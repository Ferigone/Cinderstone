//! Java 1.21 player pose, collision resolution and movement-validation core.
//!
//! World callbacks must provide the actual entity, block and border shapes for
//! their query. Unloaded chunks, missing block-entity animation and unavailable
//! entity context must return errors, never silently become empty geometry.
use crate::{
    entity::{resolve_player_pose, PlayerPoseInput},
    geometry::{self, Aabb, Axis, EntityCollisionContext, Vec3, VoxelShape},
};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::{cmp::Ordering, sync::OnceLock};

pub const POSE_FIT_DEFLATION: f64 = 1e-7;
pub const NEW_COLLISION_DEFLATION: f64 = 9.999999747378752e-6;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerDimensions {
    pub width: f32,
    pub height: f32,
    pub eye_height: f32,
    pub fixed: bool,
}
#[derive(Deserialize)]
struct PoseFact {
    dimensions: (String, String, String, bool),
}
#[derive(Deserialize)]
struct Facts {
    poses: Vec<PoseFact>,
    block_flags: Vec<u8>,
}
fn facts() -> &'static Facts {
    static FACTS: OnceLock<Facts> = OnceLock::new();
    FACTS.get_or_init(|| {
        serde_json::from_str(include_str!("../tools/player_physics_facts.json"))
            .expect("verified exact1.21 collision/pose facts")
    })
}
pub fn player_dimensions(pose: u8, scale: f32) -> Result<PlayerDimensions> {
    let row = &facts()
        .poses
        .get(pose as usize)
        .ok_or_else(|| anyhow!("unknown exact1.21 player pose{pose}"))?
        .dimensions;
    let parse =
        |value: &str| f32::from_bits(u32::from_str_radix(value, 16).expect("verified f32 bits"));
    let mut result = PlayerDimensions {
        width: parse(&row.0),
        height: parse(&row.1),
        eye_height: parse(&row.2),
        fixed: row.3,
    };
    if !result.fixed && scale != 1.0 {
        result.width *= scale;
        result.height *= scale;
        result.eye_height *= scale;
    }
    Ok(result)
}
#[derive(Clone, Copy, Debug)]
pub struct CollisionQuery {
    pub actor_id: i32,
    pub bounds: Aabb,
    pub context: EntityCollisionContext,
}
/// Return shapes as vanilla's query does: block/entity results are already
/// intersection-filtered against query.bounds. Border returns the conditional
/// border shape, whose intersection is checked by no_collision below.
pub trait WorldCollision {
    fn block_collisions(&mut self, query: CollisionQuery) -> Result<Vec<VoxelShape>>;
    fn entity_collisions(&mut self, query: CollisionQuery) -> Result<Vec<VoxelShape>>;
    fn border_collision(&mut self, query: CollisionQuery) -> Result<Option<VoxelShape>>;
    /// This is an isAir state query, not a collision query. Its bounds are
    /// supplied by floating_block_query_bounds and include nonsolid plants.
    fn all_blocks_air(&mut self, bounds: Aabb) -> Result<bool>;
}
pub fn no_collision(world: &mut impl WorldCollision, query: CollisionQuery) -> Result<bool> {
    if world.block_collisions(query)?.iter().any(|s| !s.is_empty()) {
        return Ok(false);
    }
    if !world.entity_collisions(query)?.is_empty() {
        return Ok(false);
    }
    Ok(world
        .border_collision(query)?
        .is_none_or(|shape| !shape.intersects_shape(&VoxelShape::from_aabb(query.bounds))))
}
pub fn pose_fits(
    world: &mut impl WorldCollision,
    actor_id: i32,
    feet: Vec3,
    pose: u8,
    scale: f32,
    context: EntityCollisionContext,
) -> Result<bool> {
    let dimensions = player_dimensions(pose, scale)?;
    let bounds = Aabb::player(feet, dimensions.width, dimensions.height).deflate(Vec3::new(
        POSE_FIT_DEFLATION,
        POSE_FIT_DEFLATION,
        POSE_FIT_DEFLATION,
    ));
    no_collision(
        world,
        CollisionQuery {
            actor_id,
            bounds,
            context,
        },
    )
}
pub fn update_player_pose(
    world: &mut impl WorldCollision,
    actor_id: i32,
    feet: Vec3,
    current: u8,
    input: PlayerPoseInput,
    scale: f32,
    context: EntityCollisionContext,
) -> Result<u8> {
    let mut failure = None;
    let result = resolve_player_pose(current, input, |pose| {
        if failure.is_some() {
            return false;
        }
        match pose_fits(world, actor_id, feet, pose, scale, context) {
            Ok(fits) => fits,
            Err(error) => {
                failure = Some(error);
                false
            }
        }
    });
    match failure {
        Some(error) => Err(error),
        None => Ok(result),
    }
}

/// BlockCollisions scans a one-block halo around epsilon-expanded bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionBlockRange {
    pub min: (i32, i32, i32),
    pub max: (i32, i32, i32),
}
pub fn collision_block_range(bounds: Aabb) -> CollisionBlockRange {
    let low = |v| java_floor(v - 1e-7).wrapping_sub(1);
    let high = |v| java_floor(v + 1e-7).wrapping_add(1);
    CollisionBlockRange {
        min: (low(bounds.min_x), low(bounds.min_y), low(bounds.min_z)),
        max: (high(bounds.max_x), high(bounds.max_y), high(bounds.max_z)),
    }
}
impl CollisionBlockRange {
    /// Cursor3D edge-count skip rules; a world adapter still has to obtain the
    /// actual dynamic collision shape with CollisionContext::Entity(context).
    pub fn includes(self, position: (i32, i32, i32), state: u32) -> Result<bool> {
        let flags = *facts()
            .block_flags
            .get(state as usize)
            .ok_or_else(|| anyhow!("unknown exact1.21 state{state}"))?;
        let edge = (position.0 == self.min.0 || position.0 == self.max.0) as u8
            + (position.1 == self.min.1 || position.1 == self.max.1) as u8
            + (position.2 == self.min.2 || position.2 == self.max.2) as u8;
        Ok(edge != 3 && (edge != 1 || flags & 1 != 0) && (edge != 2 || flags & 2 != 0))
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CollisionBody {
    pub actor_id: i32,
    pub bounds: Aabb,
    pub context: EntityCollisionContext,
    pub step_height: f32,
    pub on_ground: bool,
}
fn collect_motion_shapes(
    world: &mut impl WorldCollision,
    body: CollisionBody,
    bounds: Aabb,
    entities: &[VoxelShape],
) -> Result<Vec<VoxelShape>> {
    let query = CollisionQuery {
        actor_id: body.actor_id,
        bounds,
        context: body.context,
    };
    let mut shapes = entities.to_vec();
    if let Some(border) = world.border_collision(query)? {
        shapes.push(border);
    }
    shapes.extend(world.block_collisions(query)?);
    Ok(shapes)
}
/// Entity.collide including Java1.21's ascending candidate-step-height search.
/// Reuses the initial entity-collision list for candidate steps, as vanilla does.
/// This is geometric resolution; Entity.move's other block/fluid/landing effects
/// belong to the host, and are not fabricated from the collision result.
pub fn collide_motion(
    world: &mut impl WorldCollision,
    body: CollisionBody,
    movement: Vec3,
) -> Result<Vec3> {
    let query = CollisionQuery {
        actor_id: body.actor_id,
        bounds: body.bounds.expand_towards(movement),
        context: body.context,
    };
    let entities = world.entity_collisions(query)?;
    let initial = if movement.length_squared() == 0.0 {
        movement
    } else {
        let shapes = collect_motion_shapes(world, body, query.bounds, &entities)?;
        geometry::collide_with_shapes(movement, body.bounds, &shapes)
    };
    let clipped_x = movement.x != initial.x;
    let clipped_y = movement.y != initial.y;
    let clipped_z = movement.z != initial.z;
    let below = clipped_y && movement.y < 0.0;
    if body.step_height > 0.0 && (below || body.on_ground) && (clipped_x || clipped_z) {
        let base = if below {
            body.bounds.moved(Vec3::new(0.0, initial.y, 0.0))
        } else {
            body.bounds
        };
        let mut search =
            base.expand_towards(Vec3::new(movement.x, body.step_height as f64, movement.z));
        if !below {
            search = search.expand_towards(Vec3::new(0.0, -NEW_COLLISION_DEFLATION, 0.0));
        }
        let shapes = collect_motion_shapes(world, body, search, &entities)?;
        let candidates = collect_step_heights(base, &shapes, body.step_height, initial.y as f32);
        for height in candidates {
            let stepped = geometry::collide_with_shapes(
                Vec3::new(movement.x, height as f64, movement.z),
                base,
                &shapes,
            );
            if stepped.horizontal_distance_squared() > initial.horizontal_distance_squared() {
                return Ok(Vec3::new(
                    stepped.x,
                    stepped.y - (body.bounds.min_y - base.min_y),
                    stepped.z,
                ));
            }
        }
    }
    Ok(initial)
}
pub fn collect_step_heights(
    bounds: Aabb,
    shapes: &[VoxelShape],
    step_height: f32,
    baseline_y: f32,
) -> Vec<f32> {
    let mut heights = Vec::new();
    for shape in shapes {
        for &plane in shape.coordinates(Axis::Y) {
            let height = (plane - bounds.min_y) as f32;
            if height < 0.0 || height == baseline_y {
                continue;
            }
            if height > step_height {
                break;
            }
            let bits = java_float_bits(height);
            if !heights.iter().any(|&h| java_float_bits(h) == bits) {
                heights.push(height);
            }
        }
    }
    heights.sort_unstable_by(java_float_compare);
    heights
}
fn java_float_bits(value: f32) -> u32 {
    if value.is_nan() {
        0x7fc00000
    } else {
        value.to_bits()
    }
}
fn java_float_compare(a: &f32, b: &f32) -> Ordering {
    if a < b {
        Ordering::Less
    } else if a > b {
        Ordering::Greater
    } else {
        (java_float_bits(*a) as i32).cmp(&(java_float_bits(*b) as i32))
    }
}

/// The original handler rejects NaN coordinates and nonfinite rotation. Infinite
/// coordinates pass validation and are clamped afterwards; retain that behavior.
pub fn contains_invalid_values(position: Vec3, yaw: f32, pitch: f32) -> bool {
    position.x.is_nan()
        || position.y.is_nan()
        || position.z.is_nan()
        || !yaw.is_finite()
        || !pitch.is_finite()
}
pub fn clamp_position(position: Vec3) -> Vec3 {
    Vec3::new(
        java_clamp(position.x, -30_000_000.0, 30_000_000.0),
        java_clamp(position.y, -20_000_000.0, 20_000_000.0),
        java_clamp(position.z, -30_000_000.0, 30_000_000.0),
    )
}
pub fn wrap_rotation(mut value: f32) -> f32 {
    value %= 360.0;
    if value >= 180.0 {
        value -= 360.0;
    }
    if value < -180.0 {
        value += 360.0;
    }
    value
}
/// Vanilla's OR condition zeroes every non-NaN Y residual, including infinities.
/// NaN remains NaN, matching the two unordered JVM comparisons.
pub fn movement_residual(target: Vec3, actual: Vec3) -> Vec3 {
    let y = target.y - actual.y;
    Vec3::new(
        target.x - actual.x,
        if y > -0.5 || y < 0.5 { 0.0 } else { y },
        target.z - actual.z,
    )
}
pub fn floating_block_query_bounds(bounds: Aabb) -> Aabb {
    bounds
        .inflate(Vec3::new(0.0625, 0.0625, 0.0625))
        .expand_towards(Vec3::new(0.0, -0.55, 0.0))
}
/// Shapes intersecting the requested new AABB but not the old AABB trigger the
/// listener rollback. getCollisions includes entities and blocks, not the border.
pub fn colliding_with_anything_new(
    world: &mut impl WorldCollision,
    body: CollisionBody,
    current_position: Vec3,
    old_bounds: Aabb,
    target: Vec3,
) -> Result<bool> {
    let delta = Vec3::new(
        target.x - current_position.x,
        target.y - current_position.y,
        target.z - current_position.z,
    );
    let target_bounds = body.bounds.moved(delta).deflate(Vec3::new(
        NEW_COLLISION_DEFLATION,
        NEW_COLLISION_DEFLATION,
        NEW_COLLISION_DEFLATION,
    ));
    let query = CollisionQuery {
        actor_id: body.actor_id,
        bounds: target_bounds,
        context: body.context,
    };
    let mut shapes = world.entity_collisions(query)?;
    shapes.extend(world.block_collisions(query)?);
    let old_shape = VoxelShape::from_aabb(old_bounds.deflate(Vec3::new(
        NEW_COLLISION_DEFLATION,
        NEW_COLLISION_DEFLATION,
        NEW_COLLISION_DEFLATION,
    )));
    Ok(shapes.iter().any(|s| !s.intersects_shape(&old_shape)))
}
fn java_floor(value: f64) -> i32 {
    let n = value as i32;
    if value < n as f64 {
        n.wrapping_sub(1)
    } else {
        n
    }
}
fn java_clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value.is_nan() || max.is_nan() {
        f64::NAN
    } else if value == 0.0 && max == 0.0 {
        if value.is_sign_negative() || max.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else if value < max {
        value
    } else {
        max
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn d(v: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
    }
    fn f(v: &Value) -> f32 {
        f32::from_bits(u32::from_str_radix(v.as_str().unwrap(), 16).unwrap())
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
        format!("{:08x}", java_float_bits(v))
    }
    fn vector(v: &Value) -> Vec3 {
        Vec3::new(d(&v[0]), d(&v[1]), d(&v[2]))
    }
    fn bbox(v: &Value) -> Aabb {
        Aabb::new(d(&v[0]), d(&v[1]), d(&v[2]), d(&v[3]), d(&v[4]), d(&v[5]))
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
    fn context() -> EntityCollisionContext {
        EntityCollisionContext {
            bottom_y: 0.,
            descending: false,
            fall_distance: 0.,
            is_falling_block: false,
            can_walk_on_powder_snow: false,
        }
    }
    fn shapes(v: &Value) -> Vec<VoxelShape> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|v| VoxelShape::from_aabb(bbox(v)))
            .collect()
    }
    struct Scene {
        blocks: Vec<VoxelShape>,
        entities: Vec<VoxelShape>,
        border: Option<VoxelShape>,
    }
    impl Scene {
        fn from_query(q: &Value) -> Self {
            Self {
                blocks: shapes(&q["blocks"]),
                entities: shapes(&q["entities"]),
                border: q
                    .get("border")
                    .filter(|v| !v.is_null())
                    .map(|v| VoxelShape::from_aabb(bbox(v))),
            }
        }
        fn filtered(all: &[VoxelShape], query: CollisionQuery) -> Vec<VoxelShape> {
            let bounds = VoxelShape::from_aabb(query.bounds);
            all.iter()
                .filter(|s| s.intersects_shape(&bounds))
                .cloned()
                .collect()
        }
    }
    impl WorldCollision for Scene {
        fn block_collisions(&mut self, q: CollisionQuery) -> Result<Vec<VoxelShape>> {
            Ok(Self::filtered(&self.blocks, q))
        }
        fn entity_collisions(&mut self, q: CollisionQuery) -> Result<Vec<VoxelShape>> {
            Ok(Self::filtered(&self.entities, q))
        }
        fn border_collision(&mut self, _: CollisionQuery) -> Result<Option<VoxelShape>> {
            Ok(self.border.clone())
        }
        fn all_blocks_air(&mut self, _: Aabb) -> Result<bool> {
            anyhow::bail!("unused scene callback")
        }
    }
    fn run(q: &Value) -> Result<Value> {
        Ok(match q["op"].as_str().unwrap() {
            "primitive" => {
                let p = vector(&q["position"]);
                let yaw = f(&q["rotation"][0]);
                let pitch = f(&q["rotation"][1]);
                json!({"invalid": contains_invalid_values(p,yaw,pitch), "position": vec_out(clamp_position(p)), "rotation": [fb(wrap_rotation(yaw)),fb(wrap_rotation(pitch))]})
            }
            "dimensions" => {
                let v = player_dimensions(q["pose"].as_u64().unwrap() as u8, f(&q["scale"]))?;
                json!([fb(v.width), fb(v.height), fb(v.eye_height), v.fixed])
            }
            "step_heights" => {
                let mut inputs = shapes(&q["shapes"]);
                if let Some(custom) = q["custom_empty"].as_array() {
                    for row in custom {
                        let coords: Vec<Vec<f64>> = row
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|a| a.as_array().unwrap().iter().map(d).collect())
                            .collect();
                        let count = coords.iter().map(|a| a.len() - 1).product();
                        inputs.push(
                            VoxelShape::new(
                                coords[0].clone(),
                                coords[1].clone(),
                                coords[2].clone(),
                                vec![false; count],
                            )
                            .unwrap(),
                        );
                    }
                }
                let values = collect_step_heights(
                    bbox(&q["bounds"]),
                    &inputs,
                    f(&q["step"]),
                    f(&q["baseline"]),
                );
                json!(values.into_iter().map(fb).collect::<Vec<_>>())
            }
            "collide" => {
                let mut world = Scene::from_query(q);
                let body = CollisionBody {
                    actor_id: 1,
                    bounds: bbox(&q["bounds"]),
                    context: context(),
                    step_height: f(&q["step"]),
                    on_ground: q["ground"].as_bool().unwrap(),
                };
                vec_out(collide_motion(&mut world, body, vector(&q["movement"]))?)
            }
            "pose_fits" => {
                let mut world = Scene::from_query(q);
                json!(pose_fits(
                    &mut world,
                    1,
                    vector(&q["position"]),
                    q["pose"].as_u64().unwrap() as u8,
                    f(&q["scale"]),
                    context()
                )?)
            }
            "new_collision" => {
                let mut world = Scene::from_query(q);
                let body = CollisionBody {
                    actor_id: 1,
                    bounds: bbox(&q["bounds"]),
                    context: context(),
                    step_height: 0.6,
                    on_ground: false,
                };
                json!(colliding_with_anything_new(
                    &mut world,
                    body,
                    vector(&q["position"]),
                    bbox(&q["old_bounds"]),
                    vector(&q["target"])
                )?)
            }
            "floating_bounds" => {
                json!({"all_air":q["air"],"bounds":box_out(floating_block_query_bounds(bbox(&q["bounds"])))})
            }
            other => anyhow::bail!("unexpected original-player-physics op {other}"),
        })
    }
    #[test]
    fn actual_java_player_physics_vectors() {
        let fixture: Value =
            serde_json::from_str(include_str!("../tools/player_physics_vectors.json")).unwrap();
        assert_eq!(
            fixture["oracle_inner_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let mut differences = Vec::new();
        for (i, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
            let result = run(&row["query"]).unwrap();
            if result != row["expected"] {
                if differences.len() < 10 {
                    eprintln!(
                        "original Java player-physics case {i}: {}\nnative={result}\njava={}",
                        row["query"], row["expected"]
                    );
                }
                differences.push(i);
            }
        }
        assert!(
            differences.is_empty(),
            "{} original Java physics mismatches, first indices {:?}",
            differences.len(),
            &differences[..differences.len().min(10)]
        );
    }
    #[test]
    fn unavailable_world_collision_is_an_error() {
        struct Missing;
        impl WorldCollision for Missing {
            fn block_collisions(&mut self, _: CollisionQuery) -> Result<Vec<VoxelShape>> {
                anyhow::bail!("chunk unavailable")
            }
            fn entity_collisions(&mut self, _: CollisionQuery) -> Result<Vec<VoxelShape>> {
                Ok(Vec::new())
            }
            fn border_collision(&mut self, _: CollisionQuery) -> Result<Option<VoxelShape>> {
                Ok(None)
            }
            fn all_blocks_air(&mut self, _: Aabb) -> Result<bool> {
                anyhow::bail!("chunk unavailable")
            }
        }
        assert!(pose_fits(&mut Missing, 1, Vec3::ZERO, 0, 1., context()).is_err());
        assert!(update_player_pose(
            &mut Missing,
            1,
            Vec3::ZERO,
            0,
            PlayerPoseInput::default(),
            1.,
            context()
        )
        .is_err());
    }
    #[test]
    fn movement_y_residual_preserves_nan_only() {
        for y in [
            -f64::INFINITY,
            -1e100,
            -0.5,
            -0.,
            0.,
            0.5,
            1e100,
            f64::INFINITY,
        ] {
            assert_eq!(
                movement_residual(Vec3::new(1., y, 2.), Vec3::ZERO)
                    .y
                    .to_bits(),
                0
            );
        }
        assert!(movement_residual(Vec3::new(0., f64::NAN, 0.), Vec3::ZERO)
            .y
            .is_nan());
    }
}
