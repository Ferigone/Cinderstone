//! Vanilla 1.21 BucketItem use, including the server POV ray and staged writes.
use crate::{
    fluid_physics::{FluidKind, FluidState},
    geometry::{self, BlockHit, Direction, Vec3, VoxelShape},
    inventory,
    placement::{self, BlockPosition, PlacementWorld, SurvivalResult},
    world_interactions::{self, InteractionEvent, InteractionPlan, InteractionWorld},
};
use anyhow::{anyhow, ensure, Context, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BucketKind {
    Empty,
    Water,
    Lava,
}
impl BucketKind {
    pub fn from_item_id(item: i32) -> Option<Self> {
        let name = inventory::item_name(item)?;
        match name.strip_prefix("minecraft:").unwrap_or(name) {
            "bucket" => Some(Self::Empty),
            "water_bucket" => Some(Self::Water),
            "lava_bucket" => Some(Self::Lava),
            _ => None,
        }
    }
    pub const fn registry_name(self) -> &'static str {
        match self {
            Self::Empty => "minecraft:bucket",
            Self::Water => "minecraft:water_bucket",
            Self::Lava => "minecraft:lava_bucket",
        }
    }
    pub fn item_id(self) -> i32 {
        facts().bucket_ids[match self {
            Self::Empty => "qy",
            Self::Water => "qz",
            Self::Lava => "qA",
        }]
    }
    fn source_state(self) -> Option<u32> {
        match self {
            Self::Empty => None,
            Self::Water => Some(facts().source_ids["water"]),
            Self::Lava => Some(facts().source_ids["lava"]),
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    #[test]
    fn original_bucket_item_and_used_stat_ids_round_trip() {
        for (id, kind) in [
            (908, BucketKind::Empty),
            (909, BucketKind::Water),
            (910, BucketKind::Lava),
        ] {
            assert_eq!(BucketKind::from_item_id(id), Some(kind));
            assert_eq!(kind.item_id(), id);
            assert_eq!(
                crate::player_stats::stat_key("minecraft:used", kind.registry_name())
                    .unwrap()
                    .value_id,
                id
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BucketAction {
    Pickup,
    Empty,
}
#[derive(Clone, Debug)]
pub struct BucketPlan {
    pub interaction: InteractionPlan,
    /// Survival result. Creative pickup retains the held empty bucket and
    /// inserts this result only when absent. Creative emptying retains the
    /// original filled stack and inserts nothing (getEmptySuccessItem).
    pub resulting_bucket: BucketKind,
    pub action: BucketAction,
    pub used_item_id: i32,
    pub target: BlockPosition,
}
#[derive(Clone, Debug)]
pub enum BucketResult {
    Plan(BucketPlan),
    Pass,
    Rejected,
    Unsupported(&'static str),
}

/// Permission and fluid-cache queries are actual host-owned runtime facts.
/// Ordinary audited static block outlines are supplied by default; custom or
/// positional shapes require the host's actual implementation.
pub trait BucketWorld: InteractionWorld {
    fn may_interact(&mut self, position: BlockPosition) -> Result<bool>;
    fn may_use_item_at(&mut self, position: BlockPosition, face: Direction) -> Result<bool>;
    fn ultrawarm(&mut self) -> Result<bool>;
    fn fluid_shape(&mut self, position: BlockPosition, state: FluidState) -> Result<VoxelShape>;
    fn outline_shape(&mut self, state: u32, _position: BlockPosition) -> Result<VoxelShape> {
        static_outline_shape(state)
    }
    fn interaction_shape(&mut self, state: u32, _position: BlockPosition) -> Result<VoxelShape> {
        static_interaction_shape(state)
    }
}

/// Item.getPlayerPOVHitResult uses Entity.calculateViewVector's float sine
/// table, followed by the actual block-interaction-range attribute as f64.
pub fn pov_ray_end(eyes: Vec3, pitch: f32, yaw: f32, range: f64) -> Result<Vec3> {
    ensure!(
        pitch.is_finite() && yaw.is_finite() && range.is_finite() && (0. ..=64.).contains(&range),
        "invalid bucket POV attributes"
    );
    let x_angle = pitch * 0.017453292_f32;
    let y_angle = -yaw * 0.017453292_f32;
    let cp = geometry::vanilla_cos(x_angle);
    let direction = Vec3::new(
        f64::from(geometry::vanilla_sin(y_angle) * cp),
        f64::from(-geometry::vanilla_sin(x_angle)),
        f64::from(geometry::vanilla_cos(y_angle) * cp),
    );
    Ok(eyes.plus(direction.scale(range)))
}

type StateFacts = (
    i32,
    String,
    i32,
    String,
    bool,
    bool,
    bool,
    bool,
    bool,
    String,
);
#[derive(Deserialize)]
struct Facts {
    shapes: Vec<Vec<[f64; 6]>>,
    states: Vec<usize>,
    rows: Vec<StateFacts>,
    sounds: BTreeMap<String, i32>,
    bucket_ids: BTreeMap<String, i32>,
    source_ids: BTreeMap<String, u32>,
}
fn facts() -> &'static Facts {
    static DATA: OnceLock<Facts> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/bucket-interactions-1.21.json"))
            .expect("pinned BucketItem facts")
    })
}
fn state_facts(state: u32) -> Result<&'static StateFacts> {
    let index = *facts()
        .states
        .get(state as usize)
        .context("unknown bucket-ray block state")?;
    facts()
        .rows
        .get(index)
        .context("unknown bucket-ray fact row")
}
#[derive(Deserialize)]
struct ContainerFacts {
    states: Vec<usize>,
    rows: Vec<(bool, bool, u8, i32)>,
}
fn container_facts(state: u32) -> Result<(u8, i32)> {
    static DATA: OnceLock<ContainerFacts> = OnceLock::new();
    let data = DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/fluid-simulation-blocks.json"))
            .expect("pinned LiquidBlockContainer facts")
    });
    let index = *data
        .states
        .get(state as usize)
        .context("unknown bucket container state")?;
    let row = data
        .rows
        .get(index)
        .context("unknown bucket container row")?;
    Ok((row.2, row.3))
}
fn empty_shape() -> VoxelShape {
    VoxelShape::new(vec![0.], vec![0.], vec![0.], vec![]).expect("empty shape")
}
fn shape(index: i32) -> Result<VoxelShape> {
    let boxes = facts()
        .shapes
        .get(usize::try_from(index).context("live bucket ray shape required")?)
        .context("unknown outline shape")?;
    if boxes.is_empty() {
        return Ok(empty_shape());
    }
    let mut axes: [Vec<f64>; 3] =
        std::array::from_fn(|a| boxes.iter().flat_map(|b| [b[a], b[a + 3]]).collect());
    for axis in &mut axes {
        axis.sort_by(f64::total_cmp);
        axis.dedup();
    }
    let mut occupied = Vec::new();
    for x in axes[0].windows(2) {
        for y in axes[1].windows(2) {
            for z in axes[2].windows(2) {
                let p = [(x[0] + x[1]) / 2., (y[0] + y[1]) / 2., (z[0] + z[1]) / 2.];
                occupied.push(
                    boxes
                        .iter()
                        .any(|b| (0..3).all(|a| p[a] >= b[a] && p[a] < b[a + 3])),
                );
            }
        }
    }
    VoxelShape::new(axes[0].clone(), axes[1].clone(), axes[2].clone(), occupied)
        .map_err(|e| anyhow!(e))
}
pub fn static_outline_shape(state: u32) -> Result<VoxelShape> {
    let f = state_facts(state)?;
    // These getShape bytecode implementations read only state/static shape
    // tables. World-, entity- and position-sensitive owners are not admitted.
    ensure!(
        !f.4 && matches!(
            f.1.as_str(),
            "dev"
                | "dtb"
                | "dnt"
                | "dnc"
                | "dhu"
                | "dox"
                | "die"
                | "dom"
                | "dfo"
                | "dpb"
                | "dix"
                | "dni"
                | "dko"
        ),
        "live outline shape required for {}",
        f.1
    );
    shape(f.0)
}
pub fn static_interaction_shape(state: u32) -> Result<VoxelShape> {
    let f = state_facts(state)?;
    ensure!(f.3 == "dtb", "live interaction shape required for {}", f.3);
    shape(f.2)
}

struct WorkingWorld<'a, W> {
    world: &'a mut W,
    overlay: BTreeMap<BlockPosition, u32>,
    reads: BTreeMap<BlockPosition, u32>,
}
impl<W: BucketWorld> PlacementWorld for WorkingWorld<'_, W> {
    fn block(&mut self, p: BlockPosition) -> Result<u32> {
        if let Some(state) = self.overlay.get(&p) {
            return Ok(*state);
        }
        let state = self.world.block(p)?;
        self.reads.entry(p).or_insert(state);
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
    fn raw_brightness(&mut self, p: BlockPosition, n: i32) -> Result<i32> {
        self.world.raw_brightness(p, n)
    }
    fn can_see_sky(&mut self, p: BlockPosition) -> Result<bool> {
        self.world.can_see_sky(p)
    }
}
impl<W: BucketWorld> InteractionWorld for WorkingWorld<'_, W> {
    fn unobstructed(&mut self, state: u32, p: BlockPosition) -> Result<bool> {
        self.world.unobstructed(state, p)
    }
}
impl<W: BucketWorld> WorkingWorld<'_, W> {
    fn edit(
        &mut self,
        p: BlockPosition,
        state: u32,
        flags: u8,
        plan: &mut InteractionPlan,
    ) -> Result<()> {
        let next = world_interactions::plan_edit(p, state, flags, self)?;
        for e in &next.events {
            if let InteractionEvent::Write {
                position, state, ..
            } = e
            {
                self.overlay.insert(*position, *state);
            }
        }
        plan.events.extend(next.events);
        plan.changed_positions.extend(next.changed_positions);
        Ok(())
    }
    fn destroy(&mut self, p: BlockPosition, plan: &mut InteractionPlan) -> Result<()> {
        let old = self.block(p)?;
        if state_facts(old)?.5 {
            return Ok(());
        }
        plan.events.push(InteractionEvent::LevelEvent {
            position: p,
            event: 2001,
            data: old as i32,
        });
        plan.events.push(InteractionEvent::DropResources {
            position: p,
            state: old,
        });
        self.edit(p, world_interactions::removed_replacement(old)?, 3, plan)?;
        plan.events.push(InteractionEvent::GameEvent {
            position: p,
            name: "minecraft:block_destroy",
        });
        Ok(())
    }
}

fn step(p: BlockPosition, d: Direction) -> BlockPosition {
    let q = d.step();
    (
        p.0.wrapping_add(q.0),
        p.1.wrapping_add(q.1),
        p.2.wrapping_add(q.2),
    )
}

/// BlockGetter.clip with OUTLINE, the interaction-face override, and the
/// SOURCE_ONLY/NONE fluid predicates selected by BucketItem's content.
fn raycast<W: BucketWorld>(
    kind: BucketKind,
    from: Vec3,
    to: Vec3,
    world: &mut WorkingWorld<'_, W>,
) -> Result<Option<BlockHit>> {
    ensure!(
        [from.x, from.y, from.z, to.x, to.y, to.z]
            .iter()
            .all(|v| v.is_finite())
            && from.distance_squared(to) <= 64. * 64.,
        "invalid bucket POV ray"
    );
    let result = geometry::traverse_blocks(from, to, |p| -> Option<Result<BlockHit>> {
        let query = (|| -> Result<Option<BlockHit>> {
            let state = world.block(p)?;
            let mut block = world.world.outline_shape(state, p)?.clip(from, to, p);
            if let Some(hit) = &mut block {
                if let Some(other) = world.world.interaction_shape(state, p)?.clip(from, to, p) {
                    if from.distance_squared(other.location) < from.distance_squared(hit.location) {
                        hit.direction = other.direction;
                    }
                }
            }
            let fluid = FluidState::from_block(state)?;
            let fluid = if kind == BucketKind::Empty && fluid.source {
                world.world.fluid_shape(p, fluid)?.clip(from, to, p)
            } else {
                None
            };
            Ok(match (block, fluid) {
                (Some(a), Some(b)) => Some(
                    if from.distance_squared(a.location) <= from.distance_squared(b.location) {
                        a
                    } else {
                        b
                    },
                ),
                (a, None) => a,
                (None, b) => b,
            })
        })();
        match query {
            Ok(Some(hit)) => Some(Ok(hit)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    });
    result.transpose()
}

/// `from` and `to` are the actual server eye position and interaction-attribute
/// endpoint. The caller retains all world/player fact locks through commit.
pub fn plan_bucket_use(
    kind: BucketKind,
    from: Vec3,
    to: Vec3,
    world: &mut impl BucketWorld,
) -> Result<BucketResult> {
    let mut w = WorkingWorld {
        world,
        overlay: BTreeMap::new(),
        reads: BTreeMap::new(),
    };
    let Some(hit) = raycast(kind, from, to, &mut w)? else {
        return Ok(BucketResult::Pass);
    };
    let adjacent = step(hit.block_position, hit.direction);
    if !w.world.may_interact(hit.block_position)?
        || !w.world.may_use_item_at(adjacent, hit.direction)?
    {
        return Ok(BucketResult::Rejected);
    }
    let mut interaction = InteractionPlan::default();
    let (resulting_bucket, action, target) = if kind == BucketKind::Empty {
        let old = w.block(hit.block_position)?;
        let f = state_facts(old)?;
        let fluid = FluidState::from_block(old)?;
        let result = if f.9 == "dko" && fluid.source {
            if fluid.kind == FluidKind::Water {
                BucketKind::Water
            } else if fluid.kind == FluidKind::Lava {
                BucketKind::Lava
            } else {
                return Ok(BucketResult::Rejected);
            }
        } else if f.9 == "dna"
            && inventory::block_state_property(old, "waterlogged").as_deref() == Some("true")
        {
            BucketKind::Water
        } else if f.9.is_empty() {
            return Ok(BucketResult::Rejected);
        } else {
            return Ok(BucketResult::Unsupported(
                "custom BucketPickup callback required",
            ));
        };
        if f.9 == "dko" {
            w.edit(hit.block_position, 0, 11, &mut interaction)?;
        } else {
            let dry = inventory::block_state_with_properties(
                old,
                &BTreeMap::from([("waterlogged".into(), "false".into())]),
            )?;
            w.edit(hit.block_position, dry, 3, &mut interaction)?;
            match placement::can_survive(old, hit.block_position, &mut w)? {
                SurvivalResult::Supported(false) => {
                    w.destroy(hit.block_position, &mut interaction)?
                }
                SurvivalResult::Supported(true) => {}
                SurvivalResult::Unsupported(_) => {
                    return Ok(BucketResult::Unsupported(
                        "pickup survival callback required",
                    ))
                }
            }
        }
        interaction.events.push(InteractionEvent::PlayerSound {
            sound_id: facts().sounds[if result == BucketKind::Lava {
                "dl"
            } else {
                "di"
            }],
            volume: 1.,
            pitch: 1.,
        });
        interaction.events.push(InteractionEvent::GameEvent {
            position: hit.block_position,
            name: "minecraft:fluid_pickup",
        });
        (result, BucketAction::Pickup, hit.block_position)
    } else {
        let clicked = w.block(hit.block_position)?;
        let target = if kind == BucketKind::Water && container_facts(clicked)?.0 != 0 {
            hit.block_position
        } else {
            adjacent
        };
        let Some(target) = empty_contents(kind, target, Some(adjacent), &mut w, &mut interaction)?
        else {
            return Ok(BucketResult::Rejected);
        };
        (BucketKind::Empty, BucketAction::Empty, target)
    };
    interaction.read_set = w.reads;
    Ok(BucketResult::Plan(BucketPlan {
        interaction,
        resulting_bucket,
        action,
        used_item_id: kind.item_id(),
        target,
    }))
}

fn empty_contents<W: BucketWorld>(
    kind: BucketKind,
    position: BlockPosition,
    fallback: Option<BlockPosition>,
    w: &mut WorkingWorld<'_, W>,
    plan: &mut InteractionPlan,
) -> Result<Option<BlockPosition>> {
    let old = w.block(position)?;
    let f = state_facts(old)?;
    let (container, admission) = container_facts(old)?;
    let replace = if kind == BucketKind::Water { f.7 } else { f.8 };
    if admission < 0 && container != 0 {
        return Err(anyhow!(
            "world-dependent liquid container admission required"
        ));
    }
    let accepted = f.5
        || replace
        || (container != 0 && admission & (if kind == BucketKind::Water { 1 } else { 4 }) != 0);
    if !accepted {
        return match fallback {
            Some(next) => empty_contents(kind, next, None, w, plan),
            None => Ok(None),
        };
    }
    if kind == BucketKind::Water && w.world.ultrawarm()? {
        plan.events.push(InteractionEvent::BucketEvaporation {
            position,
            sound_id: facts().sounds["iU"],
        });
        return Ok(Some(position));
    }
    if position.1 < w.min_build_height()? || position.1 >= w.max_build_height()? {
        return Ok(None);
    }
    if container != 0 && kind == BucketKind::Water {
        if container != 1 {
            return Err(anyhow!("custom LiquidBlockContainer placeLiquid required"));
        }
        if inventory::block_state_property(old, "waterlogged").as_deref() == Some("false") {
            let wet = inventory::block_state_with_properties(
                old,
                &BTreeMap::from([("waterlogged".into(), "true".into())]),
            )?;
            w.edit(position, wet, 3, plan)?;
            plan.events
                .push(InteractionEvent::ScheduleWaterTick { position });
        }
    } else {
        if replace && !f.6 {
            w.destroy(position, plan)?;
        }
        w.edit(
            position,
            kind.source_state().expect("filled bucket"),
            11,
            plan,
        )?;
    }
    plan.events.push(InteractionEvent::PlaceSound {
        position,
        sound_id: facts().sounds[if kind == BucketKind::Lava { "df" } else { "dc" }],
        volume: 1.,
        pitch: 1.,
        exclude_actor: true,
    });
    plan.events.push(InteractionEvent::GameEvent {
        position,
        name: "minecraft:fluid_place",
    });
    Ok(Some(position))
}
