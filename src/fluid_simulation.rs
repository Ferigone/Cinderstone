//! Native 1.21 scheduled water/lava simulation (FlowingFluid, WaterFluid,
//! LavaFluid and LiquidBlock). Effects are synchronous required host calls.
//! Lava random ignition uses explicit chunk/fire callbacks. Animation is client
//! work; custom LiquidBlockContainer callbacks are never successful no-ops.
use crate::{
    fluid_physics::{self, FluidKind, FluidState},
    geometry::{self, Axis, BlockEntityShape, CollisionContext, Direction},
    inventory::{block_state_property, block_state_with_properties},
    placement,
    random::LegacyRandomSource,
    runtime_world::{RuntimeWorld, WorldChange},
    tick::{BlockPos, ChunkTickQueue, LevelTickScheduler, SavedTick, ScheduledTick},
};
use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    sync::OnceLock,
};

pub type Position = (i32, i32, i32);
const HORIZONTAL: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];
const NEIGHBORS: [Direction; 6] = [
    Direction::West,
    Direction::East,
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
];
const SHAPE_NEIGHBORS: [Direction; 6] = [
    Direction::West,
    Direction::East,
    Direction::North,
    Direction::South,
    Direction::Down,
    Direction::Up,
];
const LAVA_CONTACT: [Direction; 5] = [
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

/// Tick identity is the singleton, not merely the water/lava family. Vanilla
/// deduplicates SOURCE_WATER and FLOWING_WATER independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScheduledFluid {
    Water,
    FlowingWater,
    Lava,
    FlowingLava,
}
impl ScheduledFluid {
    fn of(state: FluidState) -> Option<Self> {
        Some(match (state.kind, state.source) {
            (FluidKind::Water, true) => Self::Water,
            (FluidKind::Water, false) => Self::FlowingWater,
            (FluidKind::Lava, true) => Self::Lava,
            (FluidKind::Lava, false) => Self::FlowingLava,
            (FluidKind::Empty, _) => return None,
        })
    }
    fn family(self) -> FluidKind {
        match self {
            Self::Water | Self::FlowingWater => FluidKind::Water,
            _ => FluidKind::Lava,
        }
    }
    fn mask(self) -> u8 {
        match self {
            Self::Water => 1,
            Self::FlowingWater => 2,
            Self::Lava => 4,
            Self::FlowingLava => 8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FluidWorldFacts {
    pub ultrawarm: bool,
    pub water_source_conversion: bool,
    pub lava_source_conversion: bool,
}
impl FluidWorldFacts {
    pub fn delay(self, kind: ScheduledFluid) -> i32 {
        match kind.family() {
            FluidKind::Water => 5,
            _ if self.ultrawarm => 10,
            _ => 30,
        }
    }
    fn drop_off(self, family: FluidKind) -> i32 {
        if family == FluidKind::Water || self.ultrawarm {
            1
        } else {
            2
        }
    }
    fn slope_distance(self, family: FluidKind) -> i32 {
        if family == FluidKind::Water || self.ultrawarm {
            4
        } else {
            2
        }
    }
    fn converts(self, family: FluidKind) -> bool {
        if family == FluidKind::Water {
            self.water_source_conversion
        } else {
            self.lava_source_conversion
        }
    }
}

/// set_block must execute the supplied vanilla flags and return the actual
/// ordered changes. Returning success without installing the state is rejected.
/// The host owns loot/spawn integration and non-default container callbacks.
pub trait FluidSimulationHost {
    fn set_block(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        state: u32,
        flags: u32,
    ) -> Result<Vec<WorldChange>>;
    fn before_destroying_block(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        old_state: u32,
        random: &mut LegacyRandomSource,
    ) -> Result<()>;
    fn level_event(&mut self, event: i32, position: Position, data: i32) -> Result<()>;
    fn place_custom_liquid(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        state: u32,
        fluid: FluidState,
    ) -> Result<Vec<WorldChange>>;
    fn block_changes(&mut self, changes: &[WorldChange]) -> Result<()>;
}

/// Lava ignition probes distinguish Level.isLoaded from LevelReader.hasChunkAt.
/// Installing fire must execute BaseFireBlock.getState and its real onPlace
/// callbacks (including portal creation/scheduled fire ticks), then return the
/// ordered committed edits. The caller must supply the actual tick RNG.
pub trait LavaIgnitionHost: FluidSimulationHost {
    fn is_loaded(&mut self, world: &mut RuntimeWorld, position: Position) -> Result<bool>;
    fn has_chunk_at(&mut self, world: &mut RuntimeWorld, position: Position) -> Result<bool>;
    fn place_fire(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        random: &mut LegacyRandomSource,
    ) -> Result<Vec<WorldChange>>;
}

/// The same sub-tick counter must also be used by the embedding level's block
/// scheduler. The same world RNG is passed into any loot callback.
pub struct FluidTickContext<'a, H> {
    pub world: &'a mut RuntimeWorld,
    pub random: &'a mut LegacyRandomSource,
    pub sub_tick_order: &'a mut i64,
    pub host: &'a mut H,
    pub facts: FluidWorldFacts,
    pub game_time: i64,
}

#[derive(Default, Debug)]
pub struct FluidTickReport {
    pub executed: usize,
    pub stale: usize,
    pub changed_blocks: usize,
}

pub struct FluidSimulation {
    scheduler: LevelTickScheduler<ScheduledFluid>,
}
impl Default for FluidSimulation {
    fn default() -> Self {
        Self::new()
    }
}
impl FluidSimulation {
    pub fn new() -> Self {
        Self {
            scheduler: LevelTickScheduler::new(),
        }
    }
    pub fn pending_count(&self) -> usize {
        self.scheduler.count()
    }
    pub fn chunk_has_ticks(&self, chunk: (i32, i32)) -> bool {
        self.scheduler
            .chunk(chunk)
            .is_some_and(|queue| !queue.is_empty())
    }
    pub fn saved_chunk_ticks(
        &self,
        chunk: (i32, i32),
        now: i64,
    ) -> Option<Vec<SavedTick<ScheduledFluid>>> {
        self.scheduler
            .chunk(chunk)
            .map(|queue| queue.saved_ticks(now))
    }
    pub fn load_chunk(
        &mut self,
        chunk: (i32, i32),
        saved: Vec<SavedTick<ScheduledFluid>>,
        now: i64,
    ) -> Result<()> {
        ensure!(
            self.scheduler.chunk(chunk).is_none(),
            "fluid tick chunk already loaded"
        );
        self.scheduler
            .add_chunk(chunk, ChunkTickQueue::from_saved_for_chunk(chunk, saved));
        self.scheduler.unpack_chunk(chunk, now);
        Ok(())
    }
    pub fn unload_chunk(
        &mut self,
        chunk: (i32, i32),
        now: i64,
    ) -> Option<Vec<SavedTick<ScheduledFluid>>> {
        self.scheduler
            .remove_chunk(chunk)
            .map(|queue| queue.saved_ticks(now))
    }
    pub fn schedule(
        &mut self,
        kind: ScheduledFluid,
        position: Position,
        now: i64,
        delay: i32,
        sub_tick_order: &mut i64,
    ) -> Result<bool> {
        schedule(
            &mut self.scheduler,
            kind,
            position,
            now,
            delay,
            sub_tick_order,
        )
    }
    pub fn schedule_at(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        now: i64,
        facts: FluidWorldFacts,
        sub_tick_order: &mut i64,
    ) -> Result<bool> {
        let state = fluid(world, position)?;
        let Some(kind) = ScheduledFluid::of(state) else {
            return Ok(false);
        };
        self.schedule(kind, position, now, facts.delay(kind), sub_tick_order)
    }
    /// Called after an external, already-committed block write. Root and direct
    /// neighbors receive the actual liquid onPlace/neighborChanged hooks.
    pub fn on_block_changed<H: FluidSimulationHost>(
        &mut self,
        context: &mut FluidTickContext<'_, H>,
        position: Position,
        old_state: u32,
        new_state: u32,
    ) -> Result<()> {
        self.on_block_changed_with_flags(context, position, old_state, new_state, 3)
    }
    /// Preserve Level.setBlock flags: bit1 dispatches neighbourChanged, while
    /// bit16 suppresses updateShape. Waterlogged shape callbacks remain host
    /// effects because they are emitted by the host's actual block updater.
    pub fn on_block_changed_with_flags<H: FluidSimulationHost>(
        &mut self,
        context: &mut FluidTickContext<'_, H>,
        position: Position,
        old_state: u32,
        new_state: u32,
        flags: u32,
    ) -> Result<()> {
        let mut run = Run {
            scheduler: &mut self.scheduler,
            context,
            changed: 0,
        };
        run.on_place(position, old_state, new_state)?;
        if flags & 1 != 0 {
            run.notify_neighbors(position)?;
        }
        if flags & 16 == 0 {
            run.notify_liquid_shapes(position)?;
        }
        Ok(())
    }
    /// Only chunks certified tickable by the level enter this drain. Remaining
    /// collected ticks are retained if an unsupported callback stops the run.
    pub fn tick_due<H: FluidSimulationHost>(
        &mut self,
        context: &mut FluidTickContext<'_, H>,
        budget: usize,
        tickable: impl FnMut((i32, i32)) -> bool,
    ) -> Result<FluidTickReport> {
        let due = self
            .scheduler
            .drain_due(context.game_time, budget, tickable);
        let mut report = FluidTickReport::default();
        let mut run = Run {
            scheduler: &mut self.scheduler,
            context,
            changed: 0,
        };
        for (index, tick) in due.iter().enumerate() {
            let p = (tick.pos.x, tick.pos.y, tick.pos.z);
            let outcome = (|| -> Result<bool> {
                let state = fluid(run.context.world, p)?;
                if ScheduledFluid::of(state) != Some(tick.kind) {
                    return Ok(false);
                }
                run.tick(p, state)?;
                Ok(true)
            })();
            if let Err(error) = outcome {
                for rest in &due[index + 1..] {
                    run.scheduler.schedule(rest.clone());
                }
                return Err(error.context(format!("fluid {:?} tick at {p:?}", tick.kind)));
            }
            if outcome? {
                report.executed += 1;
            } else {
                report.stale += 1;
            }
        }
        report.changed_blocks = run.changed;
        Ok(report)
    }

    /// LavaFluid.randomTick, invoked by the level's actual random-block tick
    /// selection. The scheduled-fluid queue does not invent random-tick visits.
    /// Dispatch from the sampled block's captured fluid state. The original
    /// callback does not re-read lava after the preceding block random tick.
    pub fn random_tick_lava<H: LavaIgnitionHost>(
        &mut self,
        context: &mut FluidTickContext<'_, H>,
        position: Position,
        do_fire_tick: bool,
        min_build_height: i32,
        max_build_height: i32,
    ) -> Result<usize> {
        ensure!(
            min_build_height < max_build_height,
            "invalid lava ignition build bounds"
        );
        if !do_fire_tick {
            return Ok(0);
        }
        let mut changed = 0;
        let steps = context.random.next_bounded_i32(3)?;
        if steps > 0 {
            let mut at = position;
            for _ in 0..steps {
                at = (
                    at.0.wrapping_add(context.random.next_bounded_i32(3)? - 1),
                    at.1.wrapping_add(1),
                    at.2.wrapping_add(context.random.next_bounded_i32(3)? - 1),
                );
                if !context.host.is_loaded(context.world, at)? {
                    return Ok(changed);
                }
                let state = block(context.world, at)?;
                if crate::lighting::state_properties(state)?.is_air() {
                    for direction in [
                        Direction::Down,
                        Direction::Up,
                        Direction::North,
                        Direction::South,
                        Direction::West,
                        Direction::East,
                    ] {
                        if ignitable(
                            context,
                            step(at, direction),
                            min_build_height,
                            max_build_height,
                        )? {
                            changed += place_fire(context, at)?;
                            return Ok(changed);
                        }
                    }
                } else if fluid_physics::block_facts(state)?.blocks_motion {
                    return Ok(changed);
                }
            }
        } else {
            for _ in 0..3 {
                let at = (
                    position
                        .0
                        .wrapping_add(context.random.next_bounded_i32(3)? - 1),
                    position.1,
                    position
                        .2
                        .wrapping_add(context.random.next_bounded_i32(3)? - 1),
                );
                if !context.host.is_loaded(context.world, at)? {
                    return Ok(changed);
                }
                let above = step(at, Direction::Up);
                if crate::lighting::state_properties(block(context.world, above)?)?.is_air()
                    && ignitable(context, at, min_build_height, max_build_height)?
                {
                    changed += place_fire(context, above)?;
                }
            }
        }
        Ok(changed)
    }
}

fn ignitable<H: LavaIgnitionHost>(
    context: &mut FluidTickContext<'_, H>,
    at: Position,
    min: i32,
    max: i32,
) -> Result<bool> {
    if (min..max).contains(&at.1) && !context.host.has_chunk_at(context.world, at)? {
        return Ok(false);
    }
    let state = block(context.world, at)?;
    facts(state)?; // reject unknown registry states
    Ok(catalog().data.ignited_by_lava.binary_search(&state).is_ok())
}
fn place_fire<H: LavaIgnitionHost>(
    context: &mut FluidTickContext<'_, H>,
    at: Position,
) -> Result<usize> {
    let changes = context.host.place_fire(context.world, at, context.random)?;
    // A portal or failed-survival callback can synchronously consume fire.
    // Require the actual installation/removal publication rather than claiming
    // that only a retained FireBlock is a legitimate callback outcome.
    ensure!(
        changes
            .iter()
            .any(|c| matches!(c, WorldChange::Block(b) if (b.x, b.y, b.z) == at)),
        "lava fire callback returned no block operation at {at:?}"
    );
    let count = changes
        .iter()
        .filter(|c| matches!(c, WorldChange::Block(_)))
        .count();
    context.host.block_changes(&changes)?;
    Ok(count)
}

fn schedule(
    scheduler: &mut LevelTickScheduler<ScheduledFluid>,
    kind: ScheduledFluid,
    position: Position,
    now: i64,
    delay: i32,
    order: &mut i64,
) -> Result<bool> {
    let pos = BlockPos::new(position.0, position.1, position.2);
    ensure!(
        scheduler.chunk(pos.chunk()).is_some(),
        "fluid tick target chunk {:?} is not registered loaded",
        pos.chunk()
    );
    let current = *order;
    *order = order.wrapping_add(1);
    Ok(scheduler.schedule(ScheduledTick::normal(
        kind,
        pos,
        now.wrapping_add(i64::from(delay)),
        current,
    )))
}
fn step(p: Position, d: Direction) -> Position {
    let v = d.step();
    (
        p.0.wrapping_add(v.0),
        p.1.wrapping_add(v.1),
        p.2.wrapping_add(v.2),
    )
}
fn block(world: &mut RuntimeWorld, p: Position) -> Result<u32> {
    world.block(p.0, p.1, p.2)
}
fn fluid(world: &mut RuntimeWorld, p: Position) -> Result<FluidState> {
    fluid_physics::FluidState::from_block(block(world, p)?)
}
fn value(family: FluidKind, amount: i32, falling: bool, source: bool) -> FluidState {
    FluidState {
        kind: family,
        own_height: amount as f32 / 9.,
        amount,
        falling,
        source,
    }
}
fn empty() -> FluidState {
    value(FluidKind::Empty, 0, false, false)
}
fn same(a: FluidState, b: FluidState) -> bool {
    a.kind == b.kind && a.amount == b.amount && a.falling == b.falling && a.source == b.source
}
fn name(state: u32) -> Result<&'static str> {
    placement::placement_block_name(state)
}
fn named(name: &str) -> Result<u32> {
    catalog()
        .names
        .get(name)
        .copied()
        .ok_or_else(|| anyhow!("unknown fluid block {name}"))
}
pub fn legacy(state: FluidState) -> Result<u32> {
    if state.is_empty() {
        return named("minecraft:air");
    }
    let base = named(if state.kind == FluidKind::Water {
        "minecraft:water"
    } else {
        "minecraft:lava"
    })?;
    let level = if state.source {
        0
    } else {
        8 - state.amount.min(8) + if state.falling { 8 } else { 0 }
    };
    block_state_with_properties(base, &BTreeMap::from([("level".into(), level.to_string())]))
}

#[derive(Deserialize)]
struct BlockCatalog {
    states: Vec<usize>,
    rows: Vec<(bool, bool, u8, i32)>,
    ignited_by_lava: Vec<u32>,
    dynamic_shape: Vec<u32>,
}
struct Catalog {
    data: BlockCatalog,
    names: BTreeMap<String, u32>,
}
fn catalog() -> &'static Catalog {
    static DATA: OnceLock<Catalog> = OnceLock::new();
    DATA.get_or_init(|| {
        let data: BlockCatalog =
            serde_json::from_str(include_str!("../fixtures/fluid-simulation-blocks.json"))
                .expect("original fluid block contracts");
        assert_eq!(data.states.len(), 26684);
        let placement: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/placement-1.21.json"))
                .expect("original block names");
        let names = placement["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                (
                    b["name"].as_str().unwrap().to_owned(),
                    b["default"].as_u64().unwrap() as u32,
                )
            })
            .collect();
        Catalog { data, names }
    })
}
fn facts(state: u32) -> Result<(bool, bool, u8, i32)> {
    let c = catalog();
    Ok(*c
        .data
        .rows
        .get(
            *c.data
                .states
                .get(state as usize)
                .context("unknown fluid blockstate")?,
        )
        .context("invalid fluid block facts")?)
}

struct Run<'a, 'b, H> {
    scheduler: &'a mut LevelTickScheduler<ScheduledFluid>,
    context: &'a mut FluidTickContext<'b, H>,
    changed: usize,
}
impl<H: FluidSimulationHost> Run<'_, '_, H> {
    fn schedule(&mut self, p: Position, state: FluidState, delay: i32) -> Result<()> {
        if let Some(kind) = ScheduledFluid::of(state) {
            schedule(
                self.scheduler,
                kind,
                p,
                self.context.game_time,
                delay,
                self.context.sub_tick_order,
            )?;
        }
        Ok(())
    }
    fn normal_schedule(&mut self, p: Position, state: FluidState) -> Result<()> {
        if let Some(kind) = ScheduledFluid::of(state) {
            self.schedule(p, state, self.context.facts.delay(kind))?;
        }
        Ok(())
    }
    fn write(&mut self, p: Position, state: u32, flags: u32) -> Result<()> {
        // Level.setBlock rejects outside-build-height attempts. FlowingFluid
        // deliberately ignores that false return when flowing out of the world.
        if !(crate::world::MIN_Y..=crate::world::MAX_Y).contains(&p.1) {
            return Ok(());
        }
        let old = block(self.context.world, p)?;
        if old == state {
            return Ok(());
        }
        let changes = self
            .context
            .host
            .set_block(self.context.world, p, state, flags)?;
        ensure!(
            block(self.context.world, p)? == state,
            "fluid host did not install requested block at {p:?}"
        );
        self.changed += changes
            .iter()
            .filter(|c| matches!(c, WorldChange::Block(_)))
            .count();
        self.context.host.block_changes(&changes)?;
        self.on_place(p, old, state)?;
        if flags & 1 != 0 {
            self.notify_neighbors(p)?;
        }
        if flags & 16 == 0 {
            self.notify_liquid_shapes(p)?;
        }
        Ok(())
    }
    fn on_place(&mut self, p: Position, old: u32, new: u32) -> Result<()> {
        if name(old)? != name(new)? && facts(new)?.1 {
            if self.harden(p, new)? {
                let f = fluid(self.context.world, p)?;
                self.normal_schedule(p, f)?;
            }
        }
        Ok(())
    }
    fn notify_neighbors(&mut self, p: Position) -> Result<()> {
        for direction in NEIGHBORS {
            let q = step(p, direction);
            let state = block(self.context.world, q)?;
            let f = FluidState::from_block(state)?;
            if f.is_empty() {
                continue;
            }
            if facts(state)?.1 {
                if self.harden(q, state)? {
                    self.normal_schedule(q, f)?;
                }
            } else if block_state_property(state, "waterlogged").as_deref() == Some("true") {
                self.normal_schedule(q, f)?;
            }
        }
        Ok(())
    }
    /// LiquidBlock.updateShape only schedules if either participating fluid
    /// state is a source. This differs from its unconditional neighbourChanged.
    fn notify_liquid_shapes(&mut self, p: Position) -> Result<()> {
        let changed = fluid(self.context.world, p)?;
        for direction in SHAPE_NEIGHBORS {
            let q = step(p, direction);
            let state = block(self.context.world, q)?;
            if !facts(state)?.1 {
                continue;
            }
            let current = FluidState::from_block(state)?;
            if current.source || changed.source {
                self.normal_schedule(q, current)?;
            }
        }
        Ok(())
    }
    /// LiquidBlock.shouldSpreadLiquid returns false after a conversion.
    fn harden(&mut self, p: Position, state: u32) -> Result<bool> {
        let f = FluidState::from_block(state)?;
        if f.kind != FluidKind::Lava {
            return Ok(true);
        }
        let soul =
            name(block(self.context.world, step(p, Direction::Down))?)? == "minecraft:soul_soil";
        for direction in LAVA_CONTACT {
            let q = step(p, direction);
            let neighbor = block(self.context.world, q)?;
            let replacement = if FluidState::from_block(neighbor)?.kind == FluidKind::Water {
                Some(if f.source {
                    "minecraft:obsidian"
                } else {
                    "minecraft:cobblestone"
                })
            } else if soul && name(neighbor)? == "minecraft:blue_ice" {
                Some("minecraft:basalt")
            } else {
                None
            };
            if let Some(replacement) = replacement {
                self.write(p, named(replacement)?, 3)?;
                self.context.host.level_event(1501, p, 0)?;
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn tick(&mut self, p: Position, mut old: FluidState) -> Result<()> {
        if !old.source {
            let state = block(self.context.world, p)?;
            let next = self.new_liquid(p, state, old.kind)?;
            let kind = ScheduledFluid::of(old).expect("nonempty scheduled fluid");
            let mut delay = self.context.facts.delay(kind);
            if old.kind == FluidKind::Lava && !next.is_empty() && !old.falling && !next.falling {
                let above = fluid(self.context.world, step(p, Direction::Up))?;
                let old_height = if above.kind == old.kind {
                    1.
                } else {
                    old.own_height
                };
                let next_height = if above.kind == next.kind {
                    1.
                } else {
                    next.own_height
                };
                if next_height > old_height && self.context.random.next_bounded_i32(4)? != 0 {
                    delay *= 4;
                }
            }
            if next.is_empty() {
                old = next;
                self.write(p, named("minecraft:air")?, 3)?;
            } else if !same(old, next) {
                old = next;
                self.write(p, legacy(next)?, 2)?;
                self.schedule(p, next, delay)?;
                self.notify_neighbors(p)?;
            }
        }
        self.spread(p, old)
    }
    fn new_liquid(&mut self, p: Position, state: u32, family: FluidKind) -> Result<FluidState> {
        let mut level = 0;
        let mut sources = 0;
        for direction in HORIZONTAL {
            let q = step(p, direction);
            let neighbor = block(self.context.world, q)?;
            let f = FluidState::from_block(neighbor)?;
            if f.kind != family || !can_pass_wall(state, p, neighbor, q, direction)? {
                continue;
            }
            if f.source {
                sources += 1;
            }
            level = level.max(f.amount);
        }
        if self.context.facts.converts(family) && sources >= 2 {
            let below = block(self.context.world, step(p, Direction::Down))?;
            let f = FluidState::from_block(below)?;
            if facts(below)?.0 || (f.kind == family && f.source) {
                return Ok(value(family, 8, false, true));
            }
        }
        let above_p = step(p, Direction::Up);
        let above = block(self.context.world, above_p)?;
        let f = FluidState::from_block(above)?;
        if !f.is_empty()
            && f.kind == family
            && can_pass_wall(state, p, above, above_p, Direction::Up)?
        {
            return Ok(value(family, 8, true, false));
        }
        let amount = level - self.context.facts.drop_off(family);
        Ok(if amount <= 0 {
            empty()
        } else {
            value(family, amount, false, false)
        })
    }
    fn can_hold(&mut self, p: Position, state: u32, incoming: FluidState) -> Result<bool> {
        let (_, _, container, mask) = facts(state)?;
        if container != 0 {
            ensure!(
                mask >= 0,
                "world-dependent liquid container admission at {p:?}"
            );
            return Ok(
                ScheduledFluid::of(incoming).is_some_and(|f| mask & i32::from(f.mask()) != 0)
            );
        }
        let n = name(state)?;
        if n.ends_with("_door")
            || placement::in_block_tag(state, "minecraft:signs")
            || matches!(
                n,
                "minecraft:ladder"
                    | "minecraft:sugar_cane"
                    | "minecraft:bubble_column"
                    | "minecraft:nether_portal"
                    | "minecraft:end_portal"
                    | "minecraft:end_gateway"
                    | "minecraft:structure_void"
            )
        {
            return Ok(false);
        }
        Ok(!fluid_physics::block_facts(state)?.blocks_motion)
    }
    fn replaceable(
        &mut self,
        p: Position,
        current: FluidState,
        incoming: FluidState,
        direction: Direction,
    ) -> Result<bool> {
        Ok(match current.kind {
            FluidKind::Empty => true,
            FluidKind::Water => direction == Direction::Down && incoming.kind != FluidKind::Water,
            FluidKind::Lava => {
                let above = fluid(self.context.world, step(p, Direction::Up))?;
                let height = if above.kind == current.kind {
                    1.
                } else {
                    current.own_height
                };
                height >= 0.44444445f32 && incoming.kind == FluidKind::Water
            }
        })
    }
    fn can_spread(
        &mut self,
        p: Position,
        state: u32,
        d: Direction,
        q: Position,
        target: u32,
        incoming: FluidState,
    ) -> Result<bool> {
        let current = FluidState::from_block(target)?;
        Ok(self.replaceable(q, current, incoming, d)?
            && can_pass_wall(state, p, target, q, d)?
            && self.can_hold(q, target, incoming)?)
    }
    fn water_hole(
        &mut self,
        p: Position,
        state: u32,
        q: Position,
        target: u32,
        family: FluidKind,
    ) -> Result<bool> {
        if !can_pass_wall(state, p, target, q, Direction::Down)? {
            return Ok(false);
        }
        if FluidState::from_block(target)?.kind == family {
            return Ok(true);
        }
        self.can_hold(q, target, value(family, 7, false, false))
    }
    fn pass_through(
        &mut self,
        p: Position,
        state: u32,
        d: Direction,
        q: Position,
        target: u32,
        family: FluidKind,
        incoming: FluidState,
    ) -> Result<bool> {
        let f = FluidState::from_block(target)?;
        Ok(!(f.kind == family && f.source)
            && can_pass_wall(state, p, target, q, d)?
            && self.can_hold(q, target, incoming)?)
    }
    fn spread(&mut self, p: Position, current: FluidState) -> Result<()> {
        if current.is_empty() {
            return Ok(());
        }
        let state = block(self.context.world, p)?;
        let down = step(p, Direction::Down);
        let target = block(self.context.world, down)?;
        let incoming = self.new_liquid(down, target, current.kind)?;
        if self.can_spread(p, state, Direction::Down, down, target, incoming)? {
            self.spread_to(down, target, Direction::Down, incoming, current.kind)?;
            let mut count = 0;
            for d in HORIZONTAL {
                let f = fluid(self.context.world, step(p, d))?;
                if f.kind == current.kind && f.source {
                    count += 1;
                }
            }
            if count >= 3 {
                self.spread_sides(p, current, state)?;
            }
        } else if current.source || !self.water_hole(p, state, down, target, current.kind)? {
            self.spread_sides(p, current, state)?;
        }
        Ok(())
    }
    fn spread_to(
        &mut self,
        p: Position,
        old: u32,
        direction: Direction,
        new: FluidState,
        family: FluidKind,
    ) -> Result<()> {
        if family == FluidKind::Lava
            && direction == Direction::Down
            && FluidState::from_block(old)?.kind == FluidKind::Water
        {
            if facts(old)?.1 {
                self.write(p, named("minecraft:stone")?, 3)?;
            }
            self.context.host.level_event(1501, p, 0)?;
            return Ok(());
        }
        let (_, _, container, _) = facts(old)?;
        if container == 1 {
            if block_state_property(old, "waterlogged").as_deref() == Some("false")
                && new.kind == FluidKind::Water
                && new.source
            {
                let state = block_state_with_properties(
                    old,
                    &BTreeMap::from([("waterlogged".into(), "true".into())]),
                )?;
                self.write(p, state, 3)?;
                self.normal_schedule(p, new)?;
            }
        } else if container == 2 {
            let changes = self
                .context
                .host
                .place_custom_liquid(self.context.world, p, old, new)?;
            self.changed += changes
                .iter()
                .filter(|c| matches!(c, WorldChange::Block(_)))
                .count();
            self.context.host.block_changes(&changes)?;
            let actual = fluid(self.context.world, p)?;
            if actual.kind == new.kind {
                self.normal_schedule(p, actual)?;
            }
        } else {
            if !crate::lighting::state_properties(old)?.is_air() {
                if family == FluidKind::Water {
                    self.context.host.before_destroying_block(
                        self.context.world,
                        p,
                        old,
                        self.context.random,
                    )?;
                } else {
                    self.context.host.level_event(1501, p, 0)?;
                }
            }
            self.write(p, legacy(new)?, 3)?;
        }
        Ok(())
    }
    fn spread_sides(&mut self, p: Position, current: FluidState, state: u32) -> Result<()> {
        let amount = if current.falling {
            7
        } else {
            current.amount - self.context.facts.drop_off(current.kind)
        };
        if amount <= 0 {
            return Ok(());
        }
        let mut cache = BTreeMap::new();
        let mut holes = BTreeMap::new();
        let mut best = 1000;
        let mut spread = Vec::new();
        for direction in HORIZONTAL {
            let q = step(p, direction);
            let target = cached_block(self.context.world, p, q, &mut cache)?;
            let next = self.new_liquid(q, target, current.kind)?;
            if !self.pass_through(p, state, direction, q, target, current.kind, next)? {
                continue;
            }
            let key = cache_key(p, q);
            let hole = match holes.get(&key) {
                Some(v) => *v,
                None => {
                    let below = step(q, Direction::Down);
                    let b = block(self.context.world, below)?;
                    let v = self.water_hole(q, target, below, b, current.kind)?;
                    holes.insert(key, v);
                    v
                }
            };
            let distance = if hole {
                0
            } else {
                self.slope(
                    q,
                    target,
                    1,
                    direction.opposite(),
                    p,
                    current.kind,
                    &mut cache,
                    &mut holes,
                )?
            };
            if distance < best {
                spread.clear();
            }
            if distance <= best {
                spread.push((direction, next));
                best = distance;
            }
        }
        spread.sort_by_key(|(d, _)| *d as u8);
        for (direction, incoming) in spread {
            let q = step(p, direction);
            let target = block(self.context.world, q)?;
            if self.can_spread(p, state, direction, q, target, incoming)? {
                self.spread_to(q, target, direction, incoming, current.kind)?;
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn slope(
        &mut self,
        p: Position,
        state: u32,
        distance: i32,
        skip: Direction,
        origin: Position,
        family: FluidKind,
        cache: &mut BTreeMap<i16, u32>,
        holes: &mut BTreeMap<i16, bool>,
    ) -> Result<i32> {
        let mut best = 1000;
        for direction in HORIZONTAL {
            if direction == skip {
                continue;
            }
            let q = step(p, direction);
            let target = cached_block(self.context.world, origin, q, cache)?;
            if !self.pass_through(
                p,
                state,
                direction,
                q,
                target,
                family,
                value(family, 7, false, false),
            )? {
                continue;
            }
            let key = cache_key(origin, q);
            let hole = match holes.get(&key) {
                Some(v) => *v,
                None => {
                    let below = step(q, Direction::Down);
                    let b = block(self.context.world, below)?;
                    let v = self.water_hole(q, target, below, b, family)?;
                    holes.insert(key, v);
                    v
                }
            };
            if hole {
                return Ok(distance);
            }
            if distance < self.context.facts.slope_distance(family) {
                best = best.min(self.slope(
                    q,
                    target,
                    distance + 1,
                    direction.opposite(),
                    origin,
                    family,
                    cache,
                    holes,
                )?);
            }
        }
        Ok(best)
    }
}
fn cache_key(origin: Position, p: Position) -> i16 {
    (((p.0.wrapping_sub(origin.0).wrapping_add(128) & 255) << 8)
        | (p.2.wrapping_sub(origin.2).wrapping_add(128) & 255)) as i16
}
fn cached_block(
    world: &mut RuntimeWorld,
    origin: Position,
    p: Position,
    cache: &mut BTreeMap<i16, u32>,
) -> Result<u32> {
    let key = cache_key(origin, p);
    if let Some(&v) = cache.get(&key) {
        return Ok(v);
    }
    let v = block(world, p)?;
    cache.insert(key, v);
    Ok(v)
}

/// Shapes.mergedFaceOccludes over the exact Empty collision-context boxes.
/// Animated block-entity shapes deliberately require a future live provider.
type WallKey = (u32, u32, Direction);
thread_local! {
    // Original FlowingFluid's per-thread 200-entry getAndMoveToFirst cache.
    // Block state IDs are the native canonical state identities.
    static WALL_CACHE: RefCell<VecDeque<(WallKey, bool)>> = const { RefCell::new(VecDeque::new()) };
}
fn can_pass_wall(a: u32, p: Position, b: u32, q: Position, direction: Direction) -> Result<bool> {
    facts(a)?;
    facts(b)?;
    if catalog().data.dynamic_shape.binary_search(&a).is_ok()
        || catalog().data.dynamic_shape.binary_search(&b).is_ok()
    {
        return uncached_can_pass_wall(a, p, b, q, direction);
    }
    let key = (a, b, direction);
    if let Some(value) = WALL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let index = cache.iter().position(|(entry, _)| *entry == key)?;
        let entry = cache.remove(index).unwrap();
        let value = entry.1;
        cache.push_front(entry);
        Some(value)
    }) {
        return Ok(value);
    }
    let value = uncached_can_pass_wall(a, p, b, q, direction)?;
    WALL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() == 200 {
            cache.pop_back();
        }
        cache.push_front((key, value));
    });
    Ok(value)
}
fn uncached_can_pass_wall(
    a: u32,
    p: Position,
    b: u32,
    q: Position,
    direction: Direction,
) -> Result<bool> {
    let first = geometry::block_collision_boxes(
        a,
        p,
        CollisionContext::Empty,
        BlockEntityShape::Unavailable,
    )
    .map_err(|e| anyhow!("fluid collision shape at {p:?}: {e:?}"))?;
    let second = geometry::block_collision_boxes(
        b,
        q,
        CollisionContext::Empty,
        BlockEntityShape::Unavailable,
    )
    .map_err(|e| anyhow!("fluid collision shape at {q:?}: {e:?}"))?;
    let axis = direction.axis();
    let positive = matches!(
        direction,
        Direction::Up | Direction::South | Direction::East
    );
    let mut rectangles = Vec::<[f64; 4]>::new();
    for (boxes, pos, upper) in [(&first, p, positive), (&second, q, !positive)] {
        let origin = match axis {
            Axis::X => pos.0,
            Axis::Y => pos.1,
            Axis::Z => pos.2,
        } as f64;
        let endpoint = if upper { origin + 1. } else { origin };
        let bound = boxes
            .iter()
            .map(|b| if upper { b.max(axis) } else { b.min(axis) })
            .reduce(|x, y| if upper { x.max(y) } else { x.min(y) });
        if !bound.is_some_and(|v| (v - endpoint).abs() <= 1e-7) {
            continue;
        }
        for b in boxes.iter() {
            if (if upper { b.max(axis) } else { b.min(axis) } - endpoint).abs() > 1e-7 {
                continue;
            }
            let r = match axis {
                Axis::X => [
                    b.min_y - f64::from(pos.1),
                    b.min_z - f64::from(pos.2),
                    b.max_y - f64::from(pos.1),
                    b.max_z - f64::from(pos.2),
                ],
                Axis::Y => [
                    b.min_x - f64::from(pos.0),
                    b.min_z - f64::from(pos.2),
                    b.max_x - f64::from(pos.0),
                    b.max_z - f64::from(pos.2),
                ],
                Axis::Z => [
                    b.min_x - f64::from(pos.0),
                    b.min_y - f64::from(pos.1),
                    b.max_x - f64::from(pos.0),
                    b.max_y - f64::from(pos.1),
                ],
            };
            rectangles.push(r);
        }
    }
    let mut xs = vec![0., 1.];
    let mut ys = vec![0., 1.];
    for r in &rectangles {
        xs.extend([r[0].clamp(0., 1.), r[2].clamp(0., 1.)]);
        ys.extend([r[1].clamp(0., 1.), r[3].clamp(0., 1.)]);
    }
    xs.sort_by(f64::total_cmp);
    xs.dedup();
    ys.sort_by(f64::total_cmp);
    ys.dedup();
    for x in xs.windows(2) {
        for y in ys.windows(2) {
            if x[1] - x[0] < 1e-7 || y[1] - y[0] < 1e-7 {
                continue;
            }
            let u = (x[0] + x[1]) * 0.5;
            let v = (y[0] + y[1]) * 0.5;
            if !rectangles
                .iter()
                .any(|r| u >= r[0] && u <= r[2] && v >= r[1] && v <= r[3])
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
