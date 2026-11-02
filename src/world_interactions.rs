//! Staged block-item placement and structural neighbour propagation for 1.21.
//!
//! Plans read a live world plus their own overlay and never mutate the backend.
//! The host commits the ordered events while holding its world transaction lock.
//! Loot, block entities, fluid scheduling and sounds remain explicit events;
//! none are silently executed as successful no-ops here.
use crate::{
    geometry::{Axis, Direction},
    inventory::{block_state_property, block_state_with_properties},
    placement::{
        self, BlockPosition, PlacementContext, PlacementPlanResult, PlacementWorld,
        PlannedBlockEntity, PlannedBlockTick, PlannedBlockWrite, SurvivalResult,
    },
    runtime_collision::CollisionBorder,
    runtime_world::RuntimeWorld,
};
use anyhow::{anyhow, bail, ensure, Result};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

const SHAPE_ORDER: [Direction; 6] = [
    Direction::West,
    Direction::East,
    Direction::North,
    Direction::South,
    Direction::Down,
    Direction::Up,
];
const UPDATE_LIMIT: usize = 65_536;

/// Actual obstruction check. Redstone, border, brightness and support queries
/// are inherited from PlacementWorld and cannot default to fabricated values.
pub trait InteractionWorld: PlacementWorld {
    fn unobstructed(&mut self, state: u32, position: BlockPosition) -> Result<bool>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum InteractionEvent {
    Write {
        position: BlockPosition,
        old_state: u32,
        state: u32,
        flags: u8,
    },
    /// Host callback after an ordered write has committed; the fluid runtime
    /// executes actual onPlace and neighbour scheduling from both states.
    FluidBlockChanged {
        position: BlockPosition,
        old_state: u32,
        state: u32,
        flags: u8,
    },
    /// FireSimulation has already executed the primary FireBlock.onPlace.
    /// The corresponding fluid hooks remain actual pending work.
    FluidOnlyBlockChanged {
        position: BlockPosition,
        old_state: u32,
        state: u32,
        flags: u8,
    },
    CreateBlockEntity(PlannedBlockEntity),
    ScheduleBlockTick(PlannedBlockTick),
    /// The fluid scheduler supplies the real dimension-dependent tick delay.
    ScheduleWaterTick {
        position: BlockPosition,
    },
    /// Execute the original block loot context, not an unconditional block item.
    DropResources {
        position: BlockPosition,
        state: u32,
    },
    LevelEvent {
        position: BlockPosition,
        event: i32,
        data: i32,
    },
    GameEvent {
        position: BlockPosition,
        name: &'static str,
    },
    /// Consume Level.random.nextFloat for pitch, then the level's actual
    /// threadSafeRandom.nextLong when materialising the sound packet.
    ToggleSound {
        position: BlockPosition,
        sound_id: i32,
        volume: f32,
        pitch_scale: f32,
        pitch_offset: f32,
        exclude_actor: bool,
    },
    /// Fixed BlockItem placement pitch; only the actual sound seed draws from
    /// threadSafeRandom. Normal block-item families use these state sound facts.
    PlaceSound {
        position: BlockPosition,
        sound_id: i32,
        volume: f32,
        pitch: f32,
        exclude_actor: bool,
    },
    PlayerSound {
        sound_id: i32,
        volume: f32,
        pitch: f32,
    },
    /// The host consumes two Level.random floats for pitch and executes eight
    /// original Math.random smoke positions; evaporation does not write fluid.
    BucketEvaporation {
        position: BlockPosition,
        sound_id: i32,
    },
    RequiredHook {
        position: BlockPosition,
        state: u32,
        hook: &'static str,
        declaring_class: String,
    },
    NeighbourNotice {
        position: BlockPosition,
        source_state: u32,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InteractionPlan {
    pub events: Vec<InteractionEvent>,
    /// Captured block reads for conflict detection. The host must retain its
    /// world/fact locks during planning and commit; redstone/entity facts are
    /// dynamic inputs in addition to this block read set.
    pub read_set: BTreeMap<BlockPosition, u32>,
    pub changed_positions: BTreeSet<BlockPosition>,
}
impl InteractionPlan {
    pub fn writes(&self) -> impl Iterator<Item = PlannedBlockWrite> + '_ {
        self.events.iter().filter_map(|event| match event {
            InteractionEvent::Write {
                position,
                state,
                flags,
                ..
            } => Some(PlannedBlockWrite {
                position: *position,
                state: *state,
                flags: *flags,
            }),
            _ => None,
        })
    }
    pub fn has_required_hooks(&self) -> bool {
        self.events
            .iter()
            .any(|event| matches!(event, InteractionEvent::RequiredHook { .. }))
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum InteractionResult {
    Plan(InteractionPlan),
    Rejected,
    Pass,
    Unsupported(&'static str),
}

#[derive(Deserialize)]
struct BlockFacts {
    name: String,
    class: String,
    placement_class: String,
    survival_class: String,
    min: u32,
    max: u32,
}
#[derive(Deserialize)]
struct Catalogue {
    blocks: Vec<BlockFacts>,
    signal_facts: Vec<u8>,
}
fn catalogue() -> &'static Catalogue {
    static DATA: OnceLock<Catalogue> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/placement-1.21.json"))
            .expect("pinned 1.21 placement facts")
    })
}
fn blocks() -> &'static [BlockFacts] {
    &catalogue().blocks
}
fn facts(state: u32) -> Result<&'static BlockFacts> {
    let i = blocks()
        .partition_point(|b| b.min <= state)
        .checked_sub(1)
        .ok_or_else(|| anyhow!("unknown block state {state}"))?;
    let b = &blocks()[i];
    ensure!(state <= b.max, "unknown block state {state}");
    Ok(b)
}
fn step(p: BlockPosition, d: Direction) -> BlockPosition {
    let (x, y, z) = d.step();
    (
        p.0.wrapping_add(x),
        p.1.wrapping_add(y),
        p.2.wrapping_add(z),
    )
}
fn property(state: u32, key: &str) -> Result<String> {
    block_state_property(state, key).ok_or_else(|| anyhow!("missing {key} on state {state}"))
}
fn set(state: u32, key: &str, value: &str) -> Result<u32> {
    block_state_with_properties(state, &BTreeMap::from([(key.to_owned(), value.to_owned())]))
}
fn facing(state: u32) -> Result<Direction> {
    Direction::from_name(&property(state, "facing")?).ok_or_else(|| anyhow!("invalid facing"))
}
#[derive(Deserialize)]
struct ToggleFacts {
    name: String,
    can_open_by_hand: bool,
    open_sound_id: i32,
    close_sound_id: i32,
}
fn toggle_facts(state: u32) -> Result<&'static ToggleFacts> {
    static DATA: OnceLock<Vec<ToggleFacts>> = OnceLock::new();
    let name = &facts(state)?.name;
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/world-interaction-sounds.json"))
            .expect("actual BlockSetType/WoodType sound facts")
    })
    .iter()
    .find(|f| &f.name == name)
    .ok_or_else(|| anyhow!("missing toggle facts for {name}"))
}

#[derive(Deserialize)]
struct CallbackFacts {
    neighbor_changed: BTreeMap<String, String>,
    game_event_radius: BTreeMap<String, i32>,
}
fn callbacks() -> &'static CallbackFacts {
    static DATA: OnceLock<CallbackFacts> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/world-neighbor-facts.json"))
            .expect("actual neighbourChanged declaring classes and GameEvent registry")
    })
}
pub fn game_event_radius(name: &str) -> Result<i32> {
    callbacks()
        .game_event_radius
        .get(name)
        .copied()
        .ok_or_else(|| anyhow!("unknown game event {name}"))
}

/// Certify the original neighbourChanged callbacks that perform no mutation.
/// Base BlockBehaviour callbacks are established from the actual declaring
/// method. Door/trapdoor/gate callbacks additionally read the live signal fact;
/// they are quiescent only while their stored powered bit already matches it.
/// False preserves work for a further callback executor; it never swallows it.
pub fn neighbour_notice_is_quiescent(
    position: BlockPosition,
    source_state: u32,
    world: &mut impl PlacementWorld,
) -> Result<bool> {
    neighbour_notice_quiescent(position, source_state, world, false)
}
/// FluidBlockChanged has already dispatched LiquidBlock's fluid callbacks.
/// Only that one actual declaring class is excluded here; no other neighbour
/// callback is assumed to have run by the fluid scheduler.
pub fn neighbour_notice_nonfluid_is_quiescent(
    position: BlockPosition,
    source_state: u32,
    world: &mut impl PlacementWorld,
) -> Result<bool> {
    neighbour_notice_quiescent(position, source_state, world, true)
}
fn neighbour_notice_quiescent(
    position: BlockPosition,
    source_state: u32,
    world: &mut impl PlacementWorld,
    fluid_callbacks_done: bool,
) -> Result<bool> {
    let source = facts(source_state)?;
    for direction in [
        Direction::West,
        Direction::East,
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
    ] {
        let at = step(position, direction);
        let state = world.block(at)?;
        let block = facts(state)?;
        let class = callbacks()
            .neighbor_changed
            .get(&block.name)
            .ok_or_else(|| anyhow!("missing neighbour callback for {}", block.name))?;
        match class.as_str() {
            "dtb" => {}
            _ if crate::redstone::owns_neighbor_callback(state)? => {}
            "dko" if fluid_callbacks_done => {}
            "die" if source.min == block.min => {}
            "die" | "dom" | "dix" => {
                let mut signal = world.has_neighbor_signal(at)?;
                if class == "die" && !signal {
                    signal = world.has_neighbor_signal(step(
                        at,
                        if property(state, "half")? == "lower" {
                            Direction::Up
                        } else {
                            Direction::Down
                        },
                    ))?;
                }
                if property(state, "powered")? != signal.to_string() {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

struct Planner<'a, W> {
    world: &'a mut W,
    overlay: BTreeMap<BlockPosition, u32>,
    plan: InteractionPlan,
    updates: usize,
}
impl<W: InteractionWorld> PlacementWorld for Planner<'_, W> {
    fn block(&mut self, p: BlockPosition) -> Result<u32> {
        if let Some(&state) = self.overlay.get(&p) {
            return Ok(state);
        }
        let state = self.world.block(p)?;
        self.plan.read_set.entry(p).or_insert(state);
        Ok(state)
    }
    fn min_build_height(&mut self) -> Result<i32> {
        self.world.min_build_height()
    }
    fn max_build_height(&mut self) -> Result<i32> {
        self.world.max_build_height()
    }
    fn has_neighbor_signal(&mut self, p: BlockPosition) -> Result<bool> {
        self.world.has_neighbor_signal(p)
    }
    fn inside_world_border(&mut self, p: BlockPosition) -> Result<bool> {
        self.world.inside_world_border(p)
    }
    fn raw_brightness(&mut self, p: BlockPosition, d: i32) -> Result<i32> {
        self.world.raw_brightness(p, d)
    }
    fn can_see_sky(&mut self, p: BlockPosition) -> Result<bool> {
        self.world.can_see_sky(p)
    }
}
impl<'a, W: InteractionWorld> Planner<'a, W> {
    fn new(world: &'a mut W) -> Self {
        Self {
            world,
            overlay: BTreeMap::new(),
            plan: InteractionPlan::default(),
            updates: 0,
        }
    }
    fn write(&mut self, p: BlockPosition, state: u32, flags: u8, depth: u16) -> Result<()> {
        facts(state)?;
        ensure!(depth > 0, "block shape recursion limit exceeded");
        ensure!(
            p.1 >= self.min_build_height()? && p.1 < self.max_build_height()?,
            "block write outside build height"
        );
        let old = self.block(p)?;
        if old == state {
            return Ok(());
        }
        self.overlay.insert(p, state);
        self.plan.changed_positions.insert(p);
        self.plan.events.push(InteractionEvent::Write {
            position: p,
            old_state: old,
            state,
            flags,
        });
        // LevelChunk.setBlockState calls the new state's onPlace after an
        // actual changed write, including a same-Block property change. It
        // precedes Level.setBlock's neighbour/shape propagation. Preserve that
        // phase rather than appending a copied Anvil placement tick at the end.
        if let Some(delay) = crate::falling_block::block::source_delay(state)? {
            self.plan.events.push(InteractionEvent::ScheduleBlockTick(PlannedBlockTick {
                position: p,
                state,
                delay,
            }));
        }
        if flags & 1 != 0 {
            self.plan.events.push(InteractionEvent::NeighbourNotice {
                position: p,
                source_state: old,
            });
        }
        self.shape_updates(p, state, flags, depth)
    }
    fn shape_updates(&mut self, p: BlockPosition, state: u32, flags: u8, depth: u16) -> Result<()> {
        // Flag16 suppresses shape updates; client-only flag2 still runs them.
        if flags & 16 == 0 {
            for direction in SHAPE_ORDER {
                let at = step(p, direction);
                if at.1 < self.min_build_height()? || at.1 >= self.max_build_height()? {
                    continue;
                }
                self.updates += 1;
                ensure!(
                    self.updates <= UPDATE_LIMIT,
                    "neighbour update budget exceeded"
                );
                let before = self.block(at)?;
                let mut after = self.update_shape(before, at, direction.opposite(), state)?;
                if after != before {
                    let destroyed = after == 0;
                    if destroyed {
                        if crate::fire_simulation::fire_kind(before)? == 0 {
                            self.plan.events.push(InteractionEvent::LevelEvent {
                                position: at,
                                event: 2001,
                                data: before as i32,
                            });
                        }
                        if flags & 32 == 0 && !crate::fire_simulation::fire_has_empty_loot(before)? {
                            self.plan.events.push(InteractionEvent::DropResources {
                                position: at,
                                state: before,
                            });
                        }
                        after = removed_replacement(before)?;
                    }
                    self.write(
                        at,
                        after,
                        if destroyed { 3 } else { (flags & !33) | 2 },
                        depth - 1,
                    )?;
                    if destroyed {
                        self.plan.events.push(InteractionEvent::GameEvent {
                            position: at,
                            name: "minecraft:block_destroy",
                        });
                    }
                }
            }
        }
        Ok(())
    }
    fn update_shape(
        &mut self,
        state: u32,
        p: BlockPosition,
        direction: Direction,
        next: u32,
    ) -> Result<u32> {
        let b = facts(state)?;
        if let Some(delay) = crate::falling_block::block::source_delay(state)? {
            // FallingBlock.updateShape schedules in EVERY direction, even
            // when the inherited superclass returns this unchanged state.
            // A duplicate attempt still consumes the actual shared sub-tick
            // count when Main later executes this ordered obligation.
            self.plan.events.push(InteractionEvent::ScheduleBlockTick(PlannedBlockTick {
                position: p,
                state,
                delay,
            }));
            return Ok(state);
        }
        let class = b.placement_class.as_str();
        if matches!(b.survival_class.as_str(), "diy" | "dnk") {
            return crate::fire_simulation::shape_update(state, p, self);
        }
        if block_state_property(state, "waterlogged").as_deref() == Some("true") {
            self.plan
                .events
                .push(InteractionEvent::ScheduleWaterTick { position: p });
        }
        if class == "die" {
            let half = property(state, "half")?;
            if direction.axis() == Axis::Y && (half == "lower") == (direction == Direction::Up) {
                return if facts(next)?.placement_class == "die" && property(next, "half")? != half {
                    set(next, "half", &half)
                } else {
                    Ok(0)
                };
            }
            if half == "lower" && direction == Direction::Down && !survives(state, p, self)? {
                return Ok(0);
            }
            return Ok(state);
        }
        if class == "dig" {
            let half = property(state, "half")?;
            if direction.axis() == Axis::Y && (half == "lower") == (direction == Direction::Up) {
                return if facts(next)?.min == b.min && property(next, "half")? != half {
                    Ok(state)
                } else {
                    Ok(0)
                };
            }
            if half == "lower" && direction == Direction::Down && !survives(state, p, self)? {
                return Ok(0);
            }
            return Ok(state);
        }
        if matches!(class, "diw" | "djz") {
            if direction.axis() == Axis::Y {
                return Ok(state);
            }
            if let Some(all) =
                placement::update_horizontal_connections(state, p, |q| self.block(q))?
            {
                return set(state, direction.name(), &property(all, direction.name())?);
            }
            bail!("animated neighbour shape required for fence/pane");
        }
        if class == "dnt" {
            return if direction.axis() != Axis::Y {
                set(
                    state,
                    "shape",
                    placement::stairs_shape(state, p, |q| self.block(q))?,
                )
            } else {
                Ok(state)
            };
        }
        if class == "dox" {
            return placement::update_horizontal_connections(state, p, |q| self.block(q))?
                .ok_or_else(|| anyhow!("animated wall cover shape required"));
        }
        if class == "dix" {
            let front = facing(state)?;
            if direction.axis() == front.clockwise().axis() {
                let wall = placement::in_block_tag(next, "minecraft:walls")
                    || placement::in_block_tag(
                        self.block(step(p, direction.opposite()))?,
                        "minecraft:walls",
                    );
                return set(state, "in_wall", if wall { "true" } else { "false" });
            }
            return Ok(state);
        }
        let support_changed = match b.survival_class.as_str() {
            "dfo" | "dgn" | "dfm" | "dfn" | "dia" | "dfd" | "dgo" | "dnu" | "dgk" | "dgh"
            | "dmw" => direction == Direction::Down,
            "dpb" | "dme" | "dfj" | "dke" | "dex" | "doq" => direction == facing(state)?.opposite(),
            "djp" | "dgx" | "dnp" => direction == Direction::Up,
            "dis" => true,
            _ => false,
        };
        if support_changed && !survives(state, p, self)? {
            return Ok(0);
        }
        Ok(state)
    }
}
fn survives(state: u32, p: BlockPosition, w: &mut impl PlacementWorld) -> Result<bool> {
    match placement::can_survive(state, p, w)? {
        SurvivalResult::Supported(value) => Ok(value),
        SurvivalResult::Unsupported(reason) => bail!("unported survival predicate: {reason}"),
    }
}

/// The original nonempty block-item state selection plus setPlacedBy writes.
/// The caller selects replaced/adjacent context with can_replace_with_block.
pub fn plan_place(
    item_id: i32,
    context: &PlacementContext,
    world: &mut impl InteractionWorld,
) -> Result<InteractionResult> {
    let mut p = Planner::new(world);
    if context.position.1 < p.min_build_height()? || context.position.1 >= p.max_build_height()? {
        return Ok(InteractionResult::Rejected);
    }
    let selected = match placement::placement_plan_for_item(item_id, context, &mut p)? {
        PlacementPlanResult::Rejected => return Ok(InteractionResult::Rejected),
        PlacementPlanResult::Unsupported(reason) => {
            return Ok(InteractionResult::Unsupported(reason))
        }
        PlacementPlanResult::Plan(plan) => plan,
    };
    let primary = selected
        .writes
        .iter()
        .find(|write| write.position == context.position)
        .ok_or_else(|| anyhow!("placement plan has no primary write"))?;
    if !p.world.unobstructed(primary.state, primary.position)? {
        return Ok(InteractionResult::Rejected);
    }
    for write in selected.writes {
        p.write(write.position, write.state, write.flags, 512)?;
    }
    for update in selected.extra_shape_updates {
        if update.recursion_left > 0 {
            p.shape_updates(
                update.position,
                update.state,
                update.flags,
                u16::try_from(update.recursion_left).unwrap_or(u16::MAX),
            )?;
        }
    }
    p.plan.events.extend(
        selected
            .block_entities
            .into_iter()
            .map(InteractionEvent::CreateBlockEntity),
    );
    for tick in selected.scheduled_ticks {
        if let Some(delay) = crate::falling_block::block::source_delay(tick.state)? {
            ensure!(tick.delay == delay, "Anvil placement callback delay differs");
            // Planner.write already emitted this actual onPlace obligation at
            // the source write phase. Do not replay a second schedule attempt.
        } else {
            p.plan.events.push(InteractionEvent::ScheduleBlockTick(tick));
        }
    }
    p.plan.events.extend(
        selected
            .extra_neighbour_notices
            .into_iter()
            .map(|position| InteractionEvent::NeighbourNotice {
                position,
                source_state: 0,
            }),
    );
    for hook in selected.deferred_hooks {
        p.plan.events.push(InteractionEvent::RequiredHook {
            position: hook.position,
            state: hook.state,
            hook: hook.hook,
            declaring_class: hook.declaring_class.to_owned(),
        });
    }
    static SOUND_FACTS: OnceLock<Vec<(i32, f32, f32)>> = OnceLock::new();
    let sounds = SOUND_FACTS.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/world-placement-sounds.json"))
            .expect("actual 1.21 state placement sounds")
    });
    let state = p.block(context.position)?;
    let &(sound_id, volume, pitch) = sounds
        .get(state as usize)
        .ok_or_else(|| anyhow!("missing placement sound state"))?;
    p.plan.events.push(InteractionEvent::PlaceSound {
        position: context.position,
        sound_id,
        volume: (volume + 1.) / 2.,
        pitch: pitch * 0.8,
        exclude_actor: true,
    });
    p.plan.events.push(InteractionEvent::GameEvent {
        position: context.position,
        name: "minecraft:block_place",
    });
    Ok(InteractionResult::Plan(p.plan))
}

/// Plan a setBlock transition and structural consequences. Loot from the
/// primary write is a separate player-destroy policy; detached neighbours
/// generate explicit DropResources events with their original old states.
pub fn plan_edit(
    position: BlockPosition,
    state: u32,
    flags: u8,
    world: &mut impl InteractionWorld,
) -> Result<InteractionPlan> {
    let mut p = Planner::new(world);
    p.write(position, state, flags, 512)?;
    Ok(p.plan)
}

/// Ordinary hand toggles. Iron BlockSetTypes return Pass; gates turn toward
/// the player only when opening from their opposite facing.
pub fn plan_use(
    position: BlockPosition,
    player_direction: Direction,
    world: &mut impl InteractionWorld,
) -> Result<InteractionResult> {
    let mut p = Planner::new(world);
    let old = p.block(position)?;
    let class = facts(old)?.placement_class.as_str();
    if !matches!(class, "die" | "dom" | "dix") {
        return Ok(InteractionResult::Pass);
    }
    let material = toggle_facts(old)?;
    if !material.can_open_by_hand {
        return Ok(InteractionResult::Pass);
    }
    let open = property(old, "open")? != "true";
    let mut state = set(old, "open", if open { "true" } else { "false" })?;
    if class == "dix" && open && facing(old)? == player_direction.opposite() {
        state = set(state, "facing", player_direction.name())?;
    }
    p.write(position, state, if class == "dom" { 2 } else { 10 }, 512)?;
    if class == "dom" && block_state_property(state, "waterlogged").as_deref() == Some("true") {
        p.plan
            .events
            .push(InteractionEvent::ScheduleWaterTick { position });
    }
    p.plan.events.push(InteractionEvent::ToggleSound {
        position,
        sound_id: if open {
            material.open_sound_id
        } else {
            material.close_sound_id
        },
        volume: 1.,
        pitch_scale: 0.1,
        pitch_offset: 0.9,
        exclude_actor: true,
    });
    p.plan.events.push(InteractionEvent::GameEvent {
        position,
        name: if open {
            "minecraft:block_open"
        } else {
            "minecraft:block_close"
        },
    });
    Ok(InteractionResult::Plan(p.plan))
}

/// Exact FluidState.createLegacyBlock after removing a waterlogged/aquatic
/// block. Ordinary liquid blocks retain their own fluid legacy state as well.
pub fn removed_replacement(state: u32) -> Result<u32> {
    crate::fluid_simulation::legacy(crate::fluid_physics::FluidState::from_block(state)?)
}

/// Runtime adapter for ordinary building. The collision callback must inspect
/// the actual entity registry. Build bounds and border are caller-owned facts.
pub struct RuntimeInteractionWorld<'a, F> {
    world: &'a mut RuntimeWorld,
    minimum_y: i32,
    maximum_y: i32,
    border: CollisionBorder,
    unobstructed: F,
}
impl<'a, F> RuntimeInteractionWorld<'a, F> {
    pub fn new(
        world: &'a mut RuntimeWorld,
        minimum_y: i32,
        maximum_y: i32,
        border: CollisionBorder,
        unobstructed: F,
    ) -> Result<Self> {
        ensure!(minimum_y < maximum_y, "invalid build bounds");
        Ok(Self {
            world,
            minimum_y,
            maximum_y,
            border,
            unobstructed,
        })
    }
    pub fn fluid_shape(
        &mut self,
        cache: &mut crate::fluid_physics::FluidShapeCache,
        position: BlockPosition,
        state: crate::fluid_physics::FluidState,
    ) -> Result<crate::geometry::VoxelShape> {
        cache.shape(
            &mut crate::fluid_physics::RuntimeFluidWorld { world: self.world },
            position,
            state,
        )
    }
}
impl<F: FnMut(u32, BlockPosition) -> Result<bool>> PlacementWorld
    for RuntimeInteractionWorld<'_, F>
{
    fn block(&mut self, p: BlockPosition) -> Result<u32> {
        self.world.block(p.0, p.1, p.2)
    }
    fn min_build_height(&mut self) -> Result<i32> {
        Ok(self.minimum_y)
    }
    fn max_build_height(&mut self) -> Result<i32> {
        Ok(self.maximum_y)
    }
    fn inside_world_border(&mut self, p: BlockPosition) -> Result<bool> {
        Ok(f64::from(p.0) + 1. > self.border.min_x
            && f64::from(p.0) < self.border.max_x
            && f64::from(p.2) + 1. > self.border.min_z
            && f64::from(p.2) < self.border.max_z)
    }
    fn has_neighbor_signal(&mut self, p: BlockPosition) -> Result<bool> {
        crate::redstone::runtime_has_neighbor_signal(self.world, p)
    }
}
impl<F: FnMut(u32, BlockPosition) -> Result<bool>> InteractionWorld
    for RuntimeInteractionWorld<'_, F>
{
    fn unobstructed(&mut self, state: u32, p: BlockPosition) -> Result<bool> {
        (self.unobstructed)(state, p)
    }
}

/// Prove the zero result of Level.hasNeighborSignal in an unpowered ordinary
/// neighbourhood. A signal-source state or dynamic conductor is an error,
/// never an assumed false. The full redstone engine can replace this provider.
pub fn no_signal_source_nearby(world: &mut RuntimeWorld, p: BlockPosition) -> Result<bool> {
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ] {
        let at = step(p, direction);
        let state = world.block(at.0, at.1, at.2)?;
        let b = facts(state)?;
        ensure!(
            !matches!(b.class.as_str(), "dmy" | "dsu"),
            "dynamic redstone conductor context required"
        );
        let flags = catalogue().signal_facts[state as usize];
        ensure!(
            flags & 2 == 0,
            "redstone signal source requires the live signal engine"
        );
        if flags & 1 != 0 {
            for direct in [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ] {
                let q = step(at, direct);
                let other = world.block(q.0, q.1, q.2)?;
                facts(other)?;
                ensure!(
                    catalogue().signal_facts[other as usize] & 2 == 0,
                    "direct redstone source requires the live signal engine"
                );
            }
        }
    }
    Ok(false)
}
