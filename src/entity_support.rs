//! CollisionGetter.findSupportingBlock selection over the original block query.
//!
//! The query box is supplied by Entity.checkSupportingBlock. This layer neither
//! adds entity/border collisions nor changes the support fallback policy.
use crate::{geometry::Vec3, player_physics::CollisionQuery, runtime_collision::RuntimeCollision};
use anyhow::Result;

pub type BlockPosition = (i32, i32, i32);

/// Select from collision-filtered BlockCollisions candidates in their original
/// iterator order. Distances use block centers and the entity's position.
pub fn select_supporting_block(
    position: Vec3,
    candidates: impl IntoIterator<Item = BlockPosition>,
) -> Option<BlockPosition> {
    let mut selected = None;
    let mut nearest = f64::MAX;
    for candidate in candidates {
        let x = f64::from(candidate.0) + 0.5 - position.x;
        let y = f64::from(candidate.1) + 0.5 - position.y;
        let z = f64::from(candidate.2) + 0.5 - position.z;
        let distance = x * x + y * y + z * z;
        // Equality is ordinary JVM dcmp equality, not total floating ordering.
        // NaN/infinite distances never beat the initial Double.MAX_VALUE.
        if distance < nearest
            || (distance == nearest
                && selected.is_none_or(|previous| compare_positions(previous, candidate) < 0))
        {
            selected = Some(candidate);
            nearest = distance;
        }
    }
    selected
}

/// The runtime adapter preserves the iterator's block positions alongside its
/// already filtered shapes. Unknown world/geometry context remains an error.
pub fn find_supporting_block(
    collision: &mut RuntimeCollision<'_>,
    query: CollisionQuery,
) -> Result<Option<BlockPosition>> {
    let position = collision.actor.position;
    let entries = collision.block_collision_entries(query)?;
    Ok(select_supporting_block(
        position,
        entries.into_iter().map(|(p, _)| p),
    ))
}

fn compare_positions(a: BlockPosition, b: BlockPosition) -> i32 {
    // Vec3i.compareTo is Y, then Z, then X, using signed int subtraction.
    if a.1 != b.1 {
        a.1.wrapping_sub(b.1)
    } else if a.2 != b.2 {
        a.2.wrapping_sub(b.2)
    } else {
        a.0.wrapping_sub(b.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        geometry::{Aabb, EntityCollisionContext},
        runtime_collision::{CollisionActor, CollisionBorder},
        runtime_world::RuntimeWorld,
        world::FlatWorld,
    };
    use serde_json::{json, Value};

    fn double(value: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
    }
    fn vector(value: &Value) -> Vec3 {
        Vec3::new(double(&value[0]), double(&value[1]), double(&value[2]))
    }
    fn bounds(value: &Value) -> Aabb {
        Aabb::new(
            double(&value[0]),
            double(&value[1]),
            double(&value[2]),
            double(&value[3]),
            double(&value[4]),
            double(&value[5]),
        )
    }

    #[test]
    fn actual_java_candidates_and_support_selection() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/entity_support_vectors.json")).unwrap();
        for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
            let query = &case["query"];
            let mut flat = FlatWorld::new(0, 0, 0, 0, 39);
            for block in query["blocks"].as_array().unwrap() {
                flat.set_block(
                    block[0].as_i64().unwrap() as i32,
                    block[1].as_i64().unwrap() as i32,
                    block[2].as_i64().unwrap() as i32,
                    block[3].as_u64().unwrap() as u32,
                );
            }
            let position = vector(&query["position"]);
            let mut world = RuntimeWorld::Fixture(flat);
            let mut collision = RuntimeCollision::new(
                &mut world,
                CollisionActor {
                    entity_id: 1,
                    position,
                },
                &[],
                CollisionBorder::default(),
            );
            let query = CollisionQuery {
                actor_id: 1,
                bounds: bounds(&query["bounds"]),
                context: EntityCollisionContext {
                    bottom_y: position.y,
                    descending: query["descending"].as_bool().unwrap(),
                    fall_distance: f32::from_bits(
                        u32::from_str_radix(query["fall"].as_str().unwrap(), 16).unwrap(),
                    ),
                    is_falling_block: false,
                    can_walk_on_powder_snow: false,
                },
            };
            let positions = collision
                .block_collision_entries(query)
                .unwrap()
                .into_iter()
                .map(|(p, _)| p)
                .collect::<Vec<_>>();
            let actual = json!({"positions":positions,"support":find_supporting_block(&mut collision, query).unwrap()});
            assert_eq!(
                actual, case["expected"],
                "support case{index} category{}",
                case["category"]
            );
        }
    }
}
