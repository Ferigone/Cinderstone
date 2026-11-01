//! Java 1.21 Entity.move orchestration on the shared physical entity state.
//!
//! Geometric clipping uses the independently verified Entity.collide port.
//! World/entity virtual methods remain explicit, ordered callbacks: a provider
//! must execute their real effects or return an error. In particular, this core
//! does not replace inside-block, fall, step, edge-backoff or emission effects
//! with defaults. An error can follow earlier mutations, as an exception can in
//! the original method; the caller must not silently retry a partially run move.
use crate::{
    entity_state::EntityPhysicalState,
    geometry::{Aabb, EntityCollisionContext, Vec3},
    player_physics::{collide_motion, CollisionBody, WorldCollision},
};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::sync::OnceLock;

pub type EntityMoveState = EntityPhysicalState;
pub type BlockPos = (i32, i32, i32);
pub const MOVE_EPSILON_SQUARED: f64 = 1e-7;
pub const HORIZONTAL_COLLISION_EPSILON: f64 = 9.999999747378752e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoverType {
    SelfMove,
    Player,
    Piston,
    ShulkerBox,
    Shulker,
}
impl MoverType {
    pub fn from_protocol_ordinal(value: u8) -> Result<Self> {
        Ok(match value {
            0 => Self::SelfMove,
            1 => Self::Player,
            2 => Self::Piston,
            3 => Self::ShulkerBox,
            4 => Self::Shulker,
            _ => return Err(anyhow!("unknown exact1.21 MoverType {value}")),
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovementEmission {
    None,
    Sounds,
    Events,
    All,
}
#[derive(Clone, Copy, Debug)]
pub struct EntityMoveContext {
    pub step_height: f32,
    pub descending: bool,
    pub is_falling_block: bool,
    pub can_walk_on_powder_snow: bool,
    pub movement_emission: MovementEmission,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileEvent {
    PushMove,
    PopMove,
    PushRest,
    PopRest,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveEnd {
    NoPhysics,
    PistonZero,
    RemovedAfterFall,
    Complete,
}
#[derive(Clone, Copy, Debug)]
pub struct EntityMoveResult {
    pub end: MoveEnd,
    /// Input after piston/stuck/edge-backoff, before geometric clipping.
    pub requested: Vec3,
    pub collided: Vec3,
}
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct MoveBlockFacts {
    #[serde(deserialize_with = "parse_f32_bits")]
    speed_factor: u32,
    flags: u8,
    callbacks: u16,
    fall_damage_resetting: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct BlockCallbacks {
    pub fall_on: String,
    pub after_fall: String,
    pub step_on: String,
    pub entity_inside: String,
}
fn parse_f32_bits<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<u32, D::Error> {
    let value = String::deserialize(deserializer)?;
    u32::from_str_radix(&value, 16).map_err(serde::de::Error::custom)
}
#[derive(Deserialize)]
struct Facts {
    rows: Vec<MoveBlockFacts>,
    callback_classes: Vec<BlockCallbacks>,
}
fn facts() -> &'static Facts {
    static FACTS: OnceLock<Facts> = OnceLock::new();
    FACTS.get_or_init(|| {
        serde_json::from_str(include_str!("../tools/entity_move_facts.json"))
            .expect("verified exact1.21 movement block facts")
    })
}
pub fn block_facts(state: u32) -> Result<&'static MoveBlockFacts> {
    facts()
        .rows
        .get(state as usize)
        .ok_or_else(|| anyhow!("unknown exact1.21 movement block state {state}"))
}
impl MoveBlockFacts {
    /// ClipContext.Block.FALLDAMAGE_RESETTING returns a full unit cube for this
    /// tag, even when the block's ordinary collision shape is smaller/empty.
    pub fn fall_damage_resetting(self) -> bool {
        self.fall_damage_resetting
    }
    pub fn callback_classes(self) -> Result<&'static BlockCallbacks> {
        facts()
            .callback_classes
            .get(self.callbacks as usize)
            .ok_or_else(|| anyhow!("unknown movement callback class set {}", self.callbacks))
    }
    pub fn speed_factor(self) -> f32 {
        f32::from_bits(self.speed_factor)
    }
    pub fn fence(self) -> bool {
        self.flags & 1 != 0
    }
    pub fn wall(self) -> bool {
        self.flags & 2 != 0
    }
    pub fn fence_gate(self) -> bool {
        self.flags & 4 != 0
    }
    pub fn water(self) -> bool {
        self.flags & 8 != 0
    }
    pub fn bubble_column(self) -> bool {
        self.flags & 16 != 0
    }
    pub fn climbable_or_powder_snow(self) -> bool {
        self.flags & 32 != 0
    }
    pub fn air(self) -> bool {
        self.flags & 64 != 0
    }
    pub fn fire_or_lava(self) -> bool {
        self.flags & 128 != 0
    }
}
/// The original Block.updateEntityAfterFallOn multiplies, preserving −0 in Y.
pub fn default_after_fall_on_block(state: &mut EntityPhysicalState) {
    state.velocity = Vec3::new(
        state.velocity.x * 1.,
        state.velocity.y * 0.,
        state.velocity.z * 1.,
    );
}
pub fn slime_after_fall_on_block(
    state: &mut EntityPhysicalState,
    suppress_bounce: bool,
    is_living: bool,
) {
    if suppress_bounce {
        default_after_fall_on_block(state);
    } else if state.velocity.y < 0. {
        state.velocity = Vec3::new(
            state.velocity.x,
            -state.velocity.y * if is_living { 1. } else { 0.8 },
            state.velocity.z,
        );
    }
}
pub fn slime_step_on(state: &mut EntityPhysicalState, careful_stepping: bool) {
    let y = state.velocity.y.abs();
    if y < 0.1 && !careful_stepping {
        let factor = 0.4 + y * 0.2;
        state.velocity = Vec3::new(
            state.velocity.x * factor,
            state.velocity.y * 1.,
            state.velocity.z * factor,
        );
    }
}

/// Concrete world operations used by Entity.move. All gameplay callbacks are
/// required; an incomplete provider should explicitly fail the relevant method.
pub trait EntityMoveWorld: WorldCollision {
    fn profile(&mut self, event: ProfileEvent);
    fn game_time(&mut self) -> Result<i64>;
    /// Entity's virtual setPos. Base implementations can delegate to
    /// set_base_position, which preserves the movement callback/AABB ordering.
    fn set_position(&mut self, state: &mut EntityPhysicalState, position: Vec3) -> Result<()>;
    /// Invoked between setPosRaw and the rebuilding of the entity's AABB.
    fn position_changed(&mut self, state: &mut EntityPhysicalState) -> Result<()>;
    fn reset_fall_distance(&mut self, state: &mut EntityPhysicalState) -> Result<()>;
    fn set_on_ground_with_movement(
        &mut self,
        state: &mut EntityPhysicalState,
        on_ground: bool,
        movement: Vec3,
    ) -> Result<()>;
    /// Base Entity implementations delegate to block_speed_factor. LivingEntity
    /// overrides can depend on attributes/equipment and must retain that path.
    fn block_speed_factor(&mut self, state: &EntityPhysicalState) -> Result<f32>;
    fn maybe_back_off_from_edge(
        &mut self,
        state: &mut EntityPhysicalState,
        movement: Vec3,
        mover: MoverType,
    ) -> Result<Vec3>;
    fn fall_damage_resetting_ray(
        &mut self,
        state: &EntityPhysicalState,
        from: Vec3,
        to: Vec3,
    ) -> Result<bool>;
    fn horizontal_collision_minor(
        &mut self,
        state: &EntityPhysicalState,
        collided: Vec3,
    ) -> Result<bool>;
    fn supporting_block(
        &mut self,
        state: &EntityPhysicalState,
        bounds: Aabb,
    ) -> Result<Option<BlockPos>>;
    fn block_state(&mut self, position: BlockPos) -> Result<u32>;
    fn check_fall_damage(
        &mut self,
        state: &mut EntityPhysicalState,
        dy: f64,
        on_ground: bool,
        block: u32,
        position: BlockPos,
    ) -> Result<()>;
    fn after_fall_on_block(&mut self, state: &mut EntityPhysicalState, block: u32) -> Result<()>;
    fn step_on(
        &mut self,
        state: &mut EntityPhysicalState,
        block: u32,
        position: BlockPos,
    ) -> Result<()>;
    /// The original emission branch includes walk/move/fly distances, sound and
    /// vibration subroutines. NONE skips it exactly. Other modes require a real
    /// implementation here; they are not treated as silent entities.
    fn movement_emission(
        &mut self,
        state: &mut EntityPhysicalState,
        emission: MovementEmission,
        requested: Vec3,
        collided: Vec3,
        on_pos: BlockPos,
        on_block: u32,
    ) -> Result<()>;
    fn try_check_inside_blocks(&mut self, state: &mut EntityPhysicalState) -> Result<()>;
    fn any_fire_or_lava(&mut self, bounds: Aabb) -> Result<bool>;
    fn water_rain_or_bubble(&mut self, state: &EntityPhysicalState) -> Result<bool>;
    /// Must consume the entity RNG and emit the original sound when called.
    fn play_fire_extinguished_sound(&mut self, state: &mut EntityPhysicalState) -> Result<()>;
}
pub fn set_base_position(
    state: &mut EntityPhysicalState,
    position: Vec3,
    world: &mut impl EntityMoveWorld,
) -> Result<()> {
    if state.set_position_raw(position) {
        world.position_changed(state)?;
    }
    state.refresh_bounds();
    Ok(())
}
/// Entity.getOnPos(offset), including the fence/wall/gate support-position rule.
pub fn on_position(
    state: &EntityPhysicalState,
    offset: f32,
    world: &mut impl EntityMoveWorld,
) -> Result<BlockPos> {
    if let Some(support) = state.supporting_block {
        if offset > 1e-5_f32 {
            let block = world.block_state(support)?;
            let f = *block_facts(block)?;
            if (offset as f64 <= 0.5 && f.fence()) || f.wall() || f.fence_gate() {
                return Ok(support);
            }
            return Ok((
                support.0,
                java_floor(state.position.y - offset as f64),
                support.2,
            ));
        }
        return Ok(support);
    }
    Ok((
        java_floor(state.position.x),
        java_floor(state.position.y - offset as f64),
        java_floor(state.position.z),
    ))
}
/// The block containing the feet wins unless it has factor1; water and bubble
/// columns always use their own factor rather than the block below.
pub fn block_speed_factor(
    state: &EntityPhysicalState,
    world: &mut impl EntityMoveWorld,
) -> Result<f32> {
    let inside = world.block_state(state.block_position)?;
    let facts = *block_facts(inside)?;
    let factor = facts.speed_factor();
    if facts.water() || facts.bubble_column() {
        return Ok(factor);
    }
    if factor as f64 == 1. {
        let below = on_position(state, 0.500001_f32, world)?;
        return Ok(block_facts(world.block_state(below)?)?.speed_factor());
    }
    Ok(factor)
}
pub fn update_supporting_block(
    state: &mut EntityPhysicalState,
    on_ground: bool,
    movement: Option<Vec3>,
    world: &mut impl EntityMoveWorld,
) -> Result<()> {
    state.set_on_ground(on_ground);
    if on_ground {
        let b = state.bounds;
        let probe = Aabb::new(b.min_x, b.min_y - 1e-6, b.min_z, b.max_x, b.min_y, b.max_z);
        let mut supporting = world.supporting_block(state, probe)?;
        if supporting.is_some() || state.on_ground_no_blocks {
            state.supporting_block = supporting;
        } else if let Some(movement) = movement {
            let back = probe.moved(Vec3::new(-movement.x, 0., -movement.z));
            supporting = world.supporting_block(state, back)?;
            state.supporting_block = supporting;
        }
        state.on_ground_no_blocks = supporting.is_none();
    } else {
        state.on_ground_no_blocks = false;
        if state.supporting_block.is_some() {
            state.supporting_block = None;
        }
    }
    Ok(())
}
/// Original piston axis priority X,Y,Z and per-game-tick accumulated ±0.51 cap.
/// The small-vector early return occurs before reading game time or clearing the
/// previous accumulators. Vec3 equality below intentionally distinguishes ±0.
pub fn limit_piston_movement(
    state: &mut EntityPhysicalState,
    movement: Vec3,
    world: &mut impl EntityMoveWorld,
) -> Result<Vec3> {
    if movement.length_squared() <= MOVE_EPSILON_SQUARED {
        return Ok(movement);
    }
    let time = world.game_time()?;
    if time != state.piston_tick {
        state.piston_totals = [0.; 3];
        state.piston_tick = time;
    }
    for (axis, amount) in [movement.x, movement.y, movement.z].into_iter().enumerate() {
        if amount != 0. {
            let previous = state.piston_totals[axis];
            let total = java_clamp(amount + previous, -0.51, 0.51);
            let delta = total - previous;
            state.piston_totals[axis] = total;
            if delta.abs() <= HORIZONTAL_COLLISION_EPSILON {
                return Ok(Vec3::ZERO);
            }
            return Ok(match axis {
                0 => Vec3::new(delta, 0., 0.),
                1 => Vec3::new(0., delta, 0.),
                _ => Vec3::new(0., 0., delta),
            });
        }
    }
    Ok(Vec3::ZERO)
}
/// Execute one original Entity.move phase. Required callbacks observe and mutate
/// the same shared state, so removal/velocity/fire changes remain visible to the
/// following stages. Collision geometry never substitutes for those callbacks.
pub fn move_entity(
    state: &mut EntityPhysicalState,
    mover: MoverType,
    movement: Vec3,
    context: EntityMoveContext,
    world: &mut impl EntityMoveWorld,
) -> Result<EntityMoveResult> {
    if state.no_physics {
        let target = state.position_vec().plus(movement);
        world.set_position(state, target)?;
        return Ok(EntityMoveResult {
            end: MoveEnd::NoPhysics,
            requested: movement,
            collided: movement,
        });
    }
    state.was_on_fire = state.is_on_fire();
    let mut requested = movement;
    if mover == MoverType::Piston {
        requested = limit_piston_movement(state, requested, world)?;
        if requested == Vec3::ZERO {
            return Ok(EntityMoveResult {
                end: MoveEnd::PistonZero,
                requested,
                collided: Vec3::ZERO,
            });
        }
    }
    world.profile(ProfileEvent::PushMove);
    if state.stuck_speed_multiplier.length_squared() > MOVE_EPSILON_SQUARED {
        requested = Vec3::new(
            requested.x * state.stuck_speed_multiplier.x,
            requested.y * state.stuck_speed_multiplier.y,
            requested.z * state.stuck_speed_multiplier.z,
        );
        state.stuck_speed_multiplier = Vec3::ZERO;
        state.velocity = Vec3::ZERO;
    }
    requested = world.maybe_back_off_from_edge(state, requested, mover)?;
    let collided = collide_motion(
        world,
        CollisionBody {
            actor_id: state.entity_id,
            bounds: state.bounds,
            context: EntityCollisionContext {
                bottom_y: state.position.y,
                descending: context.descending,
                fall_distance: state.fall_distance,
                is_falling_block: context.is_falling_block,
                can_walk_on_powder_snow: context.can_walk_on_powder_snow,
            },
            step_height: context.step_height,
            on_ground: state.on_ground(),
        },
        requested,
    )?;
    let distance = collided.length_squared();
    if distance > MOVE_EPSILON_SQUARED {
        if state.fall_distance != 0. && distance >= 1. {
            let from = state.position_vec();
            let to = from.plus(collided);
            if world.fall_damage_resetting_ray(state, from, to)? {
                world.reset_fall_distance(state)?;
            }
        }
        let target = state.position_vec().plus(collided);
        world.set_position(state, target)?;
    }
    world.profile(ProfileEvent::PopMove);
    world.profile(ProfileEvent::PushRest);
    let clipped_x = (collided.x - requested.x)
        .abs()
        .partial_cmp(&HORIZONTAL_COLLISION_EPSILON)
        != Some(std::cmp::Ordering::Less);
    let clipped_z = (collided.z - requested.z)
        .abs()
        .partial_cmp(&HORIZONTAL_COLLISION_EPSILON)
        != Some(std::cmp::Ordering::Less);
    state.horizontal_collision = clipped_x || clipped_z;
    state.vertical_collision = requested.y != collided.y;
    state.vertical_collision_below = state.vertical_collision && requested.y < 0.;
    state.minor_horizontal_collision = if state.horizontal_collision {
        world.horizontal_collision_minor(state, collided)?
    } else {
        false
    };
    world.set_on_ground_with_movement(state, state.vertical_collision_below, collided)?;
    let on_pos = on_position(state, 0.2_f32, world)?;
    let on_block = world.block_state(on_pos)?;
    world.check_fall_damage(state, collided.y, state.on_ground(), on_block, on_pos)?;
    if state.removed {
        world.profile(ProfileEvent::PopRest);
        return Ok(EntityMoveResult {
            end: MoveEnd::RemovedAfterFall,
            requested,
            collided,
        });
    }
    if state.horizontal_collision {
        state.velocity = Vec3::new(
            if clipped_x { 0. } else { state.velocity.x },
            state.velocity.y,
            if clipped_z { 0. } else { state.velocity.z },
        );
    }
    if requested.y != collided.y {
        world.after_fall_on_block(state, on_block)?;
    }
    if state.on_ground() {
        world.step_on(state, on_block, on_pos)?;
    }
    if context.movement_emission != MovementEmission::None && state.vehicle.is_none() {
        world.movement_emission(
            state,
            context.movement_emission,
            requested,
            collided,
            on_pos,
            on_block,
        )?;
    }
    world.try_check_inside_blocks(state)?;
    let speed = world.block_speed_factor(state)? as f64;
    state.velocity = Vec3::new(
        state.velocity.x * speed,
        state.velocity.y,
        state.velocity.z * speed,
    );
    let fire_query = state.bounds.deflate(Vec3::new(1e-6, 1e-6, 1e-6));
    if !world.any_fire_or_lava(fire_query)? {
        if state.remaining_fire_ticks <= 0 {
            state.remaining_fire_ticks = state.fire_immune_ticks.wrapping_neg();
        }
        if state.was_on_fire && (state.is_in_powder_snow || world.water_rain_or_bubble(state)?) {
            world.play_fire_extinguished_sound(state)?;
        }
    }
    if state.is_on_fire() && (state.is_in_powder_snow || world.water_rain_or_bubble(state)?) {
        state.remaining_fire_ticks = state.fire_immune_ticks.wrapping_neg();
    }
    world.profile(ProfileEvent::PopRest);
    Ok(EntityMoveResult {
        end: MoveEnd::Complete,
        requested,
        collided,
    })
}
fn java_floor(value: f64) -> i32 {
    let n = value as i32;
    if value < (n as f64) {
        n.wrapping_sub(1)
    } else {
        n
    }
}
fn java_clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{entity::Position, geometry::VoxelShape, player_physics::CollisionQuery};
    use serde_json::{json, Map, Value};
    fn d(v: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
    }
    fn f(v: &Value) -> f32 {
        f32::from_bits(u32::from_str_radix(v.as_str().unwrap(), 16).unwrap())
    }
    fn vector(v: &Value) -> Vec3 {
        Vec3::new(d(&v[0]), d(&v[1]), d(&v[2]))
    }
    fn bbox(v: &Value) -> Aabb {
        Aabb::new(d(&v[0]), d(&v[1]), d(&v[2]), d(&v[3]), d(&v[4]), d(&v[5]))
    }
    fn flag(v: &Value, name: &str) -> bool {
        v[name].as_bool().unwrap_or(false)
    }
    fn integer(v: &Value, name: &str, default: i32) -> i32 {
        v[name].as_i64().map(|v| v as i32).unwrap_or(default)
    }
    fn long(v: &Value, default: i64) -> i64 {
        v.as_i64()
            .or_else(|| v.as_u64().map(|v| v as i64))
            .unwrap_or(default)
    }
    fn point(v: &Value) -> BlockPos {
        (
            v[0].as_i64().unwrap() as i32,
            v[1].as_i64().unwrap() as i32,
            v[2].as_i64().unwrap() as i32,
        )
    }
    fn db(v: f64) -> String {
        format!(
            "{:016x}",
            if v.is_nan() {
                0x7ff8000000000000
            } else {
                v.to_bits()
            }
        )
    }
    fn fb(v: f32) -> String {
        format!("{:08x}", if v.is_nan() { 0x7fc00000 } else { v.to_bits() })
    }
    fn vec_out(v: Vec3) -> Value {
        json!([db(v.x), db(v.y), db(v.z)])
    }
    fn box_out(v: Aabb) -> Value {
        json!([
            db(v.min_x),
            db(v.min_y),
            db(v.min_z),
            db(v.max_x),
            db(v.max_y),
            db(v.max_z)
        ])
    }
    fn shapes(v: &Value) -> Vec<VoxelShape> {
        v.as_array()
            .map(|rows| {
                rows.iter()
                    .map(|r| VoxelShape::from_aabb(bbox(r)))
                    .collect()
            })
            .unwrap_or_default()
    }
    struct Probe<'a> {
        query: &'a Value,
        step: &'a Value,
        events: Vec<&'static str>,
        support_queries: Vec<Value>,
        support_index: usize,
        output: Map<String, Value>,
        blocks: Vec<VoxelShape>,
        entities: Vec<VoxelShape>,
        border: Option<VoxelShape>,
        game_time: i64,
    }
    impl Probe<'_> {
        fn event(&mut self, name: &'static str) {
            self.events.push(name);
        }
        fn step_flag(&self, name: &str) -> bool {
            self.step[name]
                .as_bool()
                .unwrap_or_else(|| flag(self.query, name))
        }
        fn filtered(shapes: &[VoxelShape], bounds: Aabb) -> Vec<VoxelShape> {
            let query = VoxelShape::from_aabb(bounds);
            shapes
                .iter()
                .filter(|s| s.intersects_shape(&query))
                .cloned()
                .collect()
        }
    }
    impl WorldCollision for Probe<'_> {
        fn block_collisions(&mut self, query: CollisionQuery) -> Result<Vec<VoxelShape>> {
            self.event("block_collisions");
            Ok(Self::filtered(&self.blocks, query.bounds))
        }
        fn entity_collisions(&mut self, query: CollisionQuery) -> Result<Vec<VoxelShape>> {
            self.event("entity_collisions");
            Ok(Self::filtered(&self.entities, query.bounds))
        }
        fn border_collision(&mut self, _: CollisionQuery) -> Result<Option<VoxelShape>> {
            Ok(self.border.clone())
        }
        fn all_blocks_air(&mut self, _: Aabb) -> Result<bool> {
            anyhow::bail!("not queried by original Entity.move")
        }
    }
    impl EntityMoveWorld for Probe<'_> {
        fn profile(&mut self, event: ProfileEvent) {
            self.event(match event {
                ProfileEvent::PushMove => "profiler_push:move",
                ProfileEvent::PopMove | ProfileEvent::PopRest => "profiler_pop",
                ProfileEvent::PushRest => "profiler_push:rest",
            });
        }
        fn game_time(&mut self) -> Result<i64> {
            Ok(self.game_time)
        }
        fn set_position(&mut self, state: &mut EntityPhysicalState, position: Vec3) -> Result<()> {
            self.event("set_pos");
            set_base_position(state, position, self)
        }
        fn position_changed(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
            self.event("level_position_changed");
            Ok(())
        }
        fn reset_fall_distance(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
            self.event("reset_fall");
            state.fall_distance = 0.;
            Ok(())
        }
        fn set_on_ground_with_movement(
            &mut self,
            state: &mut EntityPhysicalState,
            on_ground: bool,
            movement: Vec3,
        ) -> Result<()> {
            self.event("set_ground_support");
            update_supporting_block(state, on_ground, Some(movement), self)
        }
        fn block_speed_factor(&mut self, state: &EntityPhysicalState) -> Result<f32> {
            self.event("speed_factor");
            super::block_speed_factor(state, self)
        }
        fn maybe_back_off_from_edge(
            &mut self,
            _: &mut EntityPhysicalState,
            movement: Vec3,
            _: MoverType,
        ) -> Result<Vec3> {
            self.event("edge_backoff");
            Ok(self
                .step
                .get("backoff")
                .or_else(|| self.query.get("backoff"))
                .map(vector)
                .unwrap_or(movement))
        }
        fn fall_damage_resetting_ray(
            &mut self,
            _: &EntityPhysicalState,
            _: Vec3,
            _: Vec3,
        ) -> Result<bool> {
            self.event("fall_reset_raycast");
            Ok(self.step_flag("ray_hit"))
        }
        fn horizontal_collision_minor(&mut self, _: &EntityPhysicalState, _: Vec3) -> Result<bool> {
            Ok(flag(self.query, "minor"))
        }
        fn supporting_block(
            &mut self,
            _: &EntityPhysicalState,
            bounds: Aabb,
        ) -> Result<Option<BlockPos>> {
            self.event("support_query");
            self.support_queries.push(box_out(bounds));
            let answers = self
                .step
                .get("supports")
                .or_else(|| self.query.get("supports"))
                .and_then(Value::as_array);
            let answer = answers
                .and_then(|a| a.get(self.support_index))
                .filter(|v| !v.is_null())
                .map(point);
            self.support_index += 1;
            Ok(answer)
        }
        fn block_state(&mut self, position: BlockPos) -> Result<u32> {
            self.event("block_state");
            let name = format!("{},{},{}", position.0, position.1, position.2);
            Ok(self.query["world_states"]
                .get(&name)
                .and_then(Value::as_u64)
                .map(|v| v as u32)
                .unwrap_or_else(|| integer(self.query, "default_state", 1) as u32))
        }
        fn check_fall_damage(
            &mut self,
            state: &mut EntityPhysicalState,
            dy: f64,
            on_ground: bool,
            block: u32,
            position: BlockPos,
        ) -> Result<()> {
            self.event("fall_check");
            self.output.insert(
                "fall".into(),
                json!([
                    db(dy),
                    on_ground,
                    block,
                    [position.0, position.1, position.2]
                ]),
            );
            if self.step_flag("remove_after_fall") {
                state.removed = true;
            }
            Ok(())
        }
        fn after_fall_on_block(
            &mut self,
            state: &mut EntityPhysicalState,
            block: u32,
        ) -> Result<()> {
            self.event("block_after_fall");
            self.output.insert(
                "block_after_fall_velocity_before".into(),
                vec_out(state.velocity),
            );
            match block_facts(block)?.callback_classes()?.after_fall.as_str() {
                "dfy" => default_after_fall_on_block(state),
                "dnd" => {
                    slime_after_fall_on_block(state, flag(self.query, "suppress_bounce"), false)
                }
                other => anyhow::bail!("unimplemented oracle-scene afterFall class{other}"),
            }
            self.output.insert(
                "block_after_fall_velocity_after".into(),
                vec_out(state.velocity),
            );
            Ok(())
        }
        fn step_on(
            &mut self,
            state: &mut EntityPhysicalState,
            block: u32,
            _: BlockPos,
        ) -> Result<()> {
            self.event("block_step_on");
            self.output.insert(
                "block_step_on_velocity_before".into(),
                vec_out(state.velocity),
            );
            match block_facts(block)?.callback_classes()?.step_on.as_str() {
                "dfy" => {}
                "dnd" => slime_step_on(state, false),
                other => anyhow::bail!("unimplemented oracle-scene stepOn class{other}"),
            }
            self.output.insert(
                "block_step_on_velocity_after".into(),
                vec_out(state.velocity),
            );
            Ok(())
        }
        fn movement_emission(
            &mut self,
            _: &mut EntityPhysicalState,
            _: MovementEmission,
            _: Vec3,
            _: Vec3,
            _: BlockPos,
            _: u32,
        ) -> Result<()> {
            anyhow::bail!("emission other than NONE is explicitly not ported")
        }
        fn try_check_inside_blocks(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
            self.event("inside_blocks");
            if let Some(v) = self.step.get("inside_velocity") {
                state.velocity = vector(v);
            }
            if let Some(v) = self.step.get("inside_stuck") {
                state.stuck_speed_multiplier = vector(v);
            }
            Ok(())
        }
        fn any_fire_or_lava(&mut self, _: Aabb) -> Result<bool> {
            self.event("fire_query");
            if let Some(states) = self.query["fire_states"].as_array() {
                for state in states {
                    if block_facts(state.as_u64().unwrap() as u32)?.fire_or_lava() {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        }
        fn water_rain_or_bubble(&mut self, _: &EntityPhysicalState) -> Result<bool> {
            Ok(flag(self.query, "wet"))
        }
        fn play_fire_extinguished_sound(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
            self.event("extinguish_sound");
            Ok(())
        }
    }
    fn run(q: &Value) -> Result<Value> {
        let position = vector(&q["position"]);
        let width = q.get("width").map(f).unwrap_or(0.6);
        let height = q.get("height").map(f).unwrap_or(1.8);
        let mut state = EntityPhysicalState::new(
            1,
            [0; 16],
            Position {
                x: position.x,
                y: position.y,
                z: position.z,
                yaw: 0.,
                pitch: 0.,
                on_ground: flag(q, "ground"),
            },
            q.get("velocity").map(vector).unwrap_or(Vec3::ZERO),
            width,
            height,
            height,
        )?;
        state.stuck_speed_multiplier = q.get("stuck").map(vector).unwrap_or(Vec3::ZERO);
        state.no_physics = flag(q, "no_physics");
        state.horizontal_collision = flag(q, "horizontal_collision");
        state.vertical_collision = flag(q, "vertical_collision");
        state.vertical_collision_below = flag(q, "vertical_collision_below");
        state.minor_horizontal_collision = flag(q, "minor_collision");
        state.on_ground_no_blocks = flag(q, "on_ground_no_blocks");
        state.was_on_fire = flag(q, "was_on_fire");
        state.fire_immune = flag(q, "fire_immune");
        state.remaining_fire_ticks = integer(q, "fire_ticks", 0);
        state.fire_immune_ticks = integer(q, "fire_immune_ticks", 1);
        state.is_in_powder_snow = flag(q, "powder_snow");
        state.fall_distance = q.get("fall_distance").map(f).unwrap_or(0.);
        state.supporting_block = q.get("supporting").filter(|v| !v.is_null()).map(point);
        state.piston_totals = q
            .get("piston_deltas")
            .map(vector)
            .map(|v| [v.x, v.y, v.z])
            .unwrap_or([0.; 3]);
        state.piston_tick = long(&q["piston_time"], 0);
        if flag(q, "passenger") {
            state.vehicle = Some(2);
        }
        let mut game_time = long(&q["game_time"], 0);
        let mut results = Vec::new();
        for step in q["moves"].as_array().unwrap() {
            if let Some(time) = step.get("game_time") {
                game_time = long(time, 0);
            }
            if let Some(removed) = step["removed"].as_bool() {
                state.removed = removed;
            }
            let mut world = Probe {
                query: q,
                step,
                events: Vec::new(),
                support_queries: Vec::new(),
                support_index: 0,
                output: Map::new(),
                blocks: shapes(&q["blocks"]),
                entities: shapes(&q["entities"]),
                border: q
                    .get("border")
                    .filter(|v| !v.is_null())
                    .map(|v| VoxelShape::from_aabb(bbox(v))),
                game_time,
            };
            move_entity(
                &mut state,
                MoverType::from_protocol_ordinal(integer(step, "kind", 0) as u8)?,
                vector(&step["movement"]),
                EntityMoveContext {
                    step_height: q.get("step").map(f).unwrap_or(0.6),
                    descending: false,
                    is_falling_block: false,
                    can_walk_on_powder_snow: false,
                    movement_emission: match integer(q, "emission", 0) {
                        0 => MovementEmission::None,
                        1 => MovementEmission::Sounds,
                        2 => MovementEmission::Events,
                        3 => MovementEmission::All,
                        _ => anyhow::bail!("unknown movement emission"),
                    },
                },
                &mut world,
            )?;
            world.output.insert("events".into(), json!(world.events));
            world
                .output
                .insert("position".into(), vec_out(state.position_vec()));
            world.output.insert("bounds".into(), box_out(state.bounds));
            world
                .output
                .insert("velocity".into(), vec_out(state.velocity));
            world
                .output
                .insert("stuck".into(), vec_out(state.stuck_speed_multiplier));
            world
                .output
                .insert("ground".into(), json!(state.on_ground()));
            world.output.insert(
                "horizontal_collision".into(),
                json!(state.horizontal_collision),
            );
            world
                .output
                .insert("vertical_collision".into(), json!(state.vertical_collision));
            world.output.insert(
                "vertical_collision_below".into(),
                json!(state.vertical_collision_below),
            );
            world.output.insert(
                "minor_collision".into(),
                json!(state.minor_horizontal_collision),
            );
            world.output.insert(
                "on_ground_no_blocks".into(),
                json!(state.on_ground_no_blocks),
            );
            world
                .output
                .insert("was_on_fire".into(), json!(state.was_on_fire));
            world
                .output
                .insert("fall_distance".into(), json!(fb(state.fall_distance)));
            world
                .output
                .insert("fire_ticks".into(), json!(state.remaining_fire_ticks));
            world.output.insert("removed".into(), json!(state.removed));
            world.output.insert(
                "supporting".into(),
                json!(state.supporting_block.map(|p| [p.0, p.1, p.2])),
            );
            world
                .output
                .insert("support_queries".into(), json!(world.support_queries));
            world
                .output
                .insert("piston_deltas".into(), json!(state.piston_totals.map(db)));
            world
                .output
                .insert("piston_time".into(), json!(state.piston_tick));
            world.output.insert(
                "block_position".into(),
                json!([
                    state.block_position.0,
                    state.block_position.1,
                    state.block_position.2
                ]),
            );
            results.push(Value::Object(world.output));
        }
        Ok(json!(results))
    }
    #[test]
    fn actual_original_entity_move_states_and_callback_order() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/entity_move_vectors.json")).unwrap();
        assert_eq!(
            fixture["oracle_inner_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let mut differences = Vec::new();
        for (i, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
            let actual = run(&row["query"]).unwrap();
            if actual != row["expected"] {
                if differences.len() < 6 {
                    eprintln!(
                        "case{i} {}: {}\nnative={actual}\njava={}",
                        row["category"], row["query"], row["expected"]
                    );
                }
                differences.push(i);
            }
        }
        assert!(
            differences.is_empty(),
            "{} Entity.move differences,first{:?}",
            differences.len(),
            &differences[..differences.len().min(6)]
        );
    }

    #[test]
    fn missing_block_facts_and_emission_effects_are_not_silently_skipped() {
        let missing = json!({"position":vec_out(Vec3::new(0.,64.,0.)),"moves":[{"movement":vec_out(Vec3::ZERO)}],"default_state":26684});
        assert!(run(&missing)
            .unwrap_err()
            .to_string()
            .contains("unknown exact1.21 movement block state"));
        let unsupported = json!({"position":vec_out(Vec3::new(0.,64.,0.)),"moves":[{"movement":vec_out(Vec3::ZERO)}],"emission":3});
        assert!(run(&unsupported)
            .unwrap_err()
            .to_string()
            .contains("emission other than NONE"));
    }
}
