//! Runtime collision queries over the exact 1.21 world/block-state catalog.
//!
//! The caller supplies the actor's current position and the ordered results of
//! its entity query, including the actual canCollideWith relation. Ordinary
//! player pushing is separate: players are not solid collision shapes.
//! Missing chunks and unavailable block-entity animation return errors.
use crate::{
    geometry::{
        self, Aabb, Axis, BlockEntityShape, CollisionContext, EntityCollisionContext, Vec3,
        VoxelShape,
    },
    player_physics::{collision_block_range, CollisionQuery, WorldCollision},
    runtime_world::RuntimeWorld,
};
use anyhow::{anyhow, ensure, Context, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

pub const DEFAULT_BORDER_HALF_SIZE: f64 = 29_999_984.;
const MAX_QUERY_BLOCKS: u64 = 1_048_576;
pub type BlockCollisionEntry = ((i32, i32, i32), VoxelShape);

#[derive(Clone, Copy, Debug)]
pub struct CollisionActor {
    pub entity_id: i32,
    pub position: Vec3,
}

/// A broad-phase candidate in the world's original query order. The host
/// computes can_collide_with_actor using the real entity-type/vehicle rules.
/// For an ordinary player actor and ordinary player candidate this is false.
#[derive(Clone, Copy, Debug)]
pub struct CollisionEntity {
    pub entity_id: i32,
    pub bounds: Aabb,
    pub spectator: bool,
    pub can_collide_with_actor: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CollisionBorder {
    pub min_x: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_z: f64,
}
impl Default for CollisionBorder {
    fn default() -> Self {
        Self {
            min_x: -DEFAULT_BORDER_HALF_SIZE,
            min_z: -DEFAULT_BORDER_HALF_SIZE,
            max_x: DEFAULT_BORDER_HALF_SIZE,
            max_z: DEFAULT_BORDER_HALF_SIZE,
        }
    }
}
impl CollisionBorder {
    pub fn from_center_size(x: f64, z: f64, size: f64, absolute_max: i32) -> Result<Self> {
        ensure!(
            x.is_finite() && z.is_finite() && size.is_finite(),
            "nonfinite border"
        );
        ensure!(size > 0. && absolute_max > 0, "invalid border extent");
        let cap = f64::from(absolute_max);
        Ok(Self {
            min_x: (x - size / 2.).clamp(-cap, cap),
            min_z: (z - size / 2.).clamp(-cap, cap),
            max_x: (x + size / 2.).clamp(-cap, cap),
            max_z: (z + size / 2.).clamp(-cap, cap),
        })
    }

    pub fn is_inside_close_to_border(self, position: Vec3, bounds: Aabb) -> bool {
        let margin = (bounds.max_x - bounds.min_x)
            .abs()
            .max((bounds.max_z - bounds.min_z).abs())
            .max(1.);
        let distance = (position.x - self.min_x)
            .min(self.max_x - position.x)
            .min(position.z - self.min_z)
            .min(self.max_z - position.z);
        distance < margin * 2.
            && position.x >= self.min_x - margin
            && position.x < self.max_x + margin
            && position.z >= self.min_z - margin
            && position.z < self.max_z + margin
    }

    /// Infinite outside shape, with the original floor(min)/ceil(max) planes.
    pub fn collision_shape(self) -> Result<VoxelShape> {
        ensure!(
            [self.min_x, self.min_z, self.max_x, self.max_z]
                .iter()
                .all(|v| v.is_finite())
                && self.min_x < self.max_x
                && self.min_z < self.max_z,
            "invalid world border collision shape"
        );
        let mut occupied = Vec::with_capacity(9);
        for x in 0..3 {
            for z in 0..3 {
                occupied.push(x != 1 || z != 1);
            }
        }
        VoxelShape::new(
            vec![
                f64::NEG_INFINITY,
                self.min_x.floor(),
                self.max_x.ceil(),
                f64::INFINITY,
            ],
            vec![f64::NEG_INFINITY, f64::INFINITY],
            vec![
                f64::NEG_INFINITY,
                self.min_z.floor(),
                self.max_z.ceil(),
                f64::INFINITY,
            ],
            occupied,
        )
        .map_err(|error| anyhow!(error))
    }
}

#[derive(Deserialize)]
struct RawShape {
    coordinates: [Vec<String>; 3],
    occupied: String,
    canonical_block: bool,
}
type StateFact = (usize, u8, u8, f32, f32, bool, bool);

#[derive(Deserialize)]
struct RawCatalog {
    states: Vec<StateFact>,
    shapes: Vec<RawShape>,
    special: BTreeMap<String, usize>,
}
struct ShapeFact {
    shape: VoxelShape,
    canonical_block: bool,
}
struct Catalog {
    states: Vec<StateFact>,
    shapes: Vec<ShapeFact>,
    special: BTreeMap<String, usize>,
}
fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let data: RawCatalog = serde_json::from_str(include_str!(
            "../fixtures/runtime_collision_shapes-1.21.json"
        ))
        .expect("verified original 1.21 voxel catalog");
        Catalog {
            states: data.states,
            special: data.special,
            shapes: data
                .shapes
                .into_iter()
                .map(|raw| {
                    let coords = raw.coordinates.map(|axis| {
                        axis.into_iter()
                            .map(|v| {
                                f64::from_bits(
                                    u64::from_str_radix(&v, 16).expect("verified voxel f64"),
                                )
                            })
                            .collect::<Vec<_>>()
                    });
                    ShapeFact {
                        canonical_block: raw.canonical_block,
                        shape: VoxelShape::new(
                            coords[0].clone(),
                            coords[1].clone(),
                            coords[2].clone(),
                            raw.occupied.bytes().map(|v| v == b'1').collect(),
                        )
                        .expect("verified original voxel grid"),
                    }
                })
                .collect(),
        }
    })
}

/// The full original grid is retained: step-height collection observes unused
/// coordinate planes which cannot be reconstructed from just occupied AABBs.
pub fn state_collision_shape(
    state: u32,
    position: (i32, i32, i32),
    context: EntityCollisionContext,
    block_entity: BlockEntityShape<'_>,
) -> Result<(VoxelShape, bool)> {
    let (local, canonical_block) =
        state_collision_shape_local_details(state, position, context, block_entity)?;
    if local.is_empty() {
        return Ok((local, false));
    }
    // Keep the original second addition separate from the dynamic offset.
    // A ray clip needs the local result before this integer translation; an
    // inverse translation would lose the source's coordinate precision.
    let axes = [Axis::X, Axis::Y, Axis::Z];
    let coords = axes.map(|axis| {
        local.coordinates(axis).iter().map(|value| {
            *value + match axis {
                Axis::X => position.0 as f64,
                Axis::Y => position.1 as f64,
                Axis::Z => position.2 as f64,
            }
        }).collect::<Vec<_>>()
    });
    Ok((VoxelShape::new(coords[0].clone(), coords[1].clone(), coords[2].clone(),
        local.occupied().to_vec()).map_err(|error| anyhow!(error))?, canonical_block))
}

/// The same genuine BlockState collision grid BEFORE the BlockCollisions
/// iterator's integer position translation. State-owned dynamic offsets are
/// already applied with the original rounding/order. The caller supplies the
/// actual entity context and block-entity availability, as for movement.
/// Animated AABBs do not certify a source voxel grid and remain unsupported.
pub fn state_collision_shape_local(
    state: u32,
    position: (i32, i32, i32),
    context: EntityCollisionContext,
    block_entity: BlockEntityShape<'_>,
) -> Result<VoxelShape> {
    state_collision_shape_local_details(state, position, context, block_entity)
        .map(|(shape, _)| shape)
}

/// Pure capability inspection of the immutable source grid metadata for a
/// FallingBlockEntity. It neither evaluates a counterfactual entity context
/// nor invokes the actual shape query. The real post-move query must still
/// supply its genuine bottom/fall-distance/descending fields and position.
/// This is not world, chunk, block-entity absence or source ownership proof.
pub fn require_falling_collision_shape_capability(state: u32) -> Result<()> {
    let c = catalog();
    let &(base, kind, detail, horizontal, vertical, _, _) = c.states.get(state as usize)
        .context("unknown exact 1.21 falling collision capability state")?;
    c.shapes.get(base).context("falling collision source base grid is unavailable")?;
    let require_special = |name: &str| -> Result<()> {
        let id = c.special.get(name)
            .with_context(|| format!("falling collision selector lacks source asset {name}"))?;
        c.shapes.get(*id)
            .with_context(|| format!("falling collision source grid {name} is unavailable"))?;
        Ok(())
    };
    match kind {
        0 | 1 => {},
        2 | 3 => {
            ensure!(horizontal.is_finite() && vertical.is_finite()
                && horizontal >= 0. && vertical >= 0.,
                "falling collision source offset metadata is unavailable");
        }
        4 => {
            ensure!(detail <= 15, "falling scaffolding source selector metadata is unknown");
            // Actual post-move bottom/descending may select any of these
            // branches. Inspect the real assets rather than probing invented
            // entity heights through state_collision_shape_local.
            require_special("scaffolding_stable")?;
            require_special("scaffolding_bottom")?;
            require_special("empty")?;
        }
        5 => {
            // Falling always satisfies is_falling_block; the earlier source
            // fallDistance>2.5 branch may still select powder_falling.
            require_special("powder_falling")?;
            require_special("full")?;
            require_special("empty")?;
        }
        6 | 7 => anyhow::bail!(
            "falling collision requires an actual animated block-entity grid/absence owner"),
        _ => anyhow::bail!("unknown exact 1.21 falling collision selector kind {kind}"),
    }
    Ok(())
}

fn state_collision_shape_local_details(
    state: u32,
    position: (i32, i32, i32),
    context: EntityCollisionContext,
    block_entity: BlockEntityShape<'_>,
) -> Result<(VoxelShape, bool)> {
    let c = catalog();
    let &(base, kind, detail, horizontal, vertical, vertical_offset, _) = c
        .states
        .get(state as usize)
        .context("unknown exact 1.21 collision state")?;
    // This validates the same state/context/animation conditions used by the
    // independently verified block geometry module.
    geometry::block_collision_boxes(
        state,
        position,
        CollisionContext::Entity(context),
        block_entity,
    )
    .map_err(|error| anyhow!("unavailable block collision at {position:?}: {error:?}"))?;
    let mut id = base;
    let above =
        |height: f64| context.bottom_y > f64::from(position.1) + height - 9.999999747378752e-6;
    if kind == 4 {
        id = if above(1.) && !context.descending {
            c.special["scaffolding_stable"]
        } else if detail / 2 != 0 && detail & 1 != 0 && above(0.) {
            c.special["scaffolding_bottom"]
        } else {
            c.special["empty"]
        };
    } else if kind == 5 {
        id = if context.fall_distance > 2.5 {
            c.special["powder_falling"]
        } else if context.is_falling_block
            || (context.can_walk_on_powder_snow && above(1.) && !context.descending)
        {
            c.special["full"]
        } else {
            c.special["empty"]
        };
    } else if kind == 6 || kind == 7 {
        id = match block_entity {
            BlockEntityShape::Absent => c.special[if kind == 6 { "full" } else { "empty" }],
            // Animated AABBs do not preserve the source grid; the host must
            // provide the original animated grid through a future shape API.
            BlockEntityShape::Boxes(_) | BlockEntityShape::Unavailable => {
                anyhow::bail!("animated block-entity voxel grid unavailable at {position:?}")
            }
        };
    }
    let fact = &c.shapes[id];
    if fact.shape.is_empty() {
        return Ok((fact.shape.clone(), false));
    }
    let offset = if kind == 2 || kind == 3 {
        geometry::block_position_offset(position, horizontal, vertical, vertical_offset)
    } else {
        Vec3::ZERO
    };
    // VoxelShape.move applies the local dynamic offset first. Leave the final
    // BlockCollisions integer-block translation to state_collision_shape.
    let axes = [Axis::X, Axis::Y, Axis::Z];
    let coords = axes.map(|axis| {
        fact.shape
            .coordinates(axis)
            .iter()
            .map(|v| {
                if kind == 2 || kind == 3 {
                    *v + offset.get(axis)
                } else {
                    *v
                }
            })
            .collect::<Vec<_>>()
    });
    Ok((
        VoxelShape::new(
            coords[0].clone(),
            coords[1].clone(),
            coords[2].clone(),
            fact.shape.occupied().to_vec(),
        )
        .map_err(|error| anyhow!(error))?,
        fact.canonical_block,
    ))
}

pub struct RuntimeCollision<'a> {
    pub world: &'a mut RuntimeWorld,
    pub actor: CollisionActor,
    pub entities: &'a [CollisionEntity],
    pub border: CollisionBorder,
    fall_distance_available: bool,
}
impl<'a> RuntimeCollision<'a> {
    pub fn new(
        world: &'a mut RuntimeWorld,
        actor: CollisionActor,
        entities: &'a [CollisionEntity],
        border: CollisionBorder,
    ) -> Self {
        Self {
            world,
            actor,
            entities,
            border,
            fall_distance_available: true,
        }
    }
    /// Mark a host context whose Entity.fallDistance has not been computed.
    /// Ordinary block queries remain usable; a powder-snow candidate fails
    /// before its shape can consume an invented fall-distance value.
    pub fn without_fall_distance(mut self) -> Self {
        self.fall_distance_available = false;
        self
    }
    fn validate_query(&self, query: CollisionQuery) -> Result<()> {
        ensure!(
            query.actor_id == self.actor.entity_id,
            "collision actor mismatch"
        );
        ensure!(finite_bounds(query.bounds), "nonfinite collision bounds");
        Ok(())
    }
    fn block_entity_shape(
        &mut self,
        state: u32,
        p: (i32, i32, i32),
    ) -> Result<BlockEntityShape<'static>> {
        let kind = catalog().states[state as usize].1;
        if kind != 6 && kind != 7 {
            return Ok(BlockEntityShape::Absent);
        }
        match self.world {
            RuntimeWorld::Anvil(world) => {
                Ok(if world.block_entity_snapshot(p.0, p.1, p.2)?.is_none() {
                    BlockEntityShape::Absent
                } else {
                    BlockEntityShape::Unavailable
                })
            }
            // Fixture block-entity lifecycle is not tracked; it must not
            // fabricate an absent entity for a placed shulker or moving piston.
            _ => Ok(BlockEntityShape::Unavailable),
        }
    }
}

impl RuntimeCollision<'_> {
    /// Original BlockCollisions cursor positions and their filtered shapes.
    /// Supporting-block selection consumes these positions in the same order.
    pub fn block_collision_entries(
        &mut self,
        query: CollisionQuery,
    ) -> Result<Vec<BlockCollisionEntry>> {
        self.validate_query(query)?;
        let range = collision_block_range(query.bounds);
        checked_volume(range.min, range.max)?;
        let bounds_shape = VoxelShape::from_aabb(query.bounds);
        let mut shapes = Vec::new();
        // Cursor3D: x changes fastest, followed by y, then z. Corners are
        // skipped before chunk/state access by the original iterator.
        for z in range.min.2..=range.max.2 {
            for y in range.min.1..=range.max.1 {
                for x in range.min.0..=range.max.0 {
                    let p = (x, y, z);
                    let corner = (x == range.min.0 || x == range.max.0)
                        && (y == range.min.1 || y == range.max.1)
                        && (z == range.min.2 || z == range.max.2);
                    if corner {
                        continue;
                    }
                    let state = self.world.block(x, y, z)?;
                    if !range.includes(p, state)? {
                        continue;
                    }
                    ensure!(
                        self.fall_distance_available || catalog().states[state as usize].1 != 5,
                        "fall distance unavailable for powder snow collision at {p:?}"
                    );
                    let block_entity = self.block_entity_shape(state, p)?;
                    let (shape, canonical_block) =
                        state_collision_shape(state, p, query.context, block_entity)?;
                    let intersects = if canonical_block {
                        query.bounds.intersects(&Aabb::unit_block(x, y, z))
                    } else {
                        !shape.is_empty() && shape.intersects_shape(&bounds_shape)
                    };
                    if intersects {
                        shapes.push((p, shape));
                    }
                }
            }
        }
        Ok(shapes)
    }
}

impl WorldCollision for RuntimeCollision<'_> {
    fn block_collisions(&mut self, query: CollisionQuery) -> Result<Vec<VoxelShape>> {
        Ok(self
            .block_collision_entries(query)?
            .into_iter()
            .map(|(_, shape)| shape)
            .collect())
    }

    fn entity_collisions(&mut self, query: CollisionQuery) -> Result<Vec<VoxelShape>> {
        self.validate_query(query)?;
        let b = query.bounds;
        let size = ((b.max_x - b.min_x) + (b.max_y - b.min_y) + (b.max_z - b.min_z)) / 3.;
        if size < 1e-7 {
            return Ok(Vec::new());
        }
        let expanded = b.inflate(Vec3::new(1e-7, 1e-7, 1e-7));
        Ok(self
            .entities
            .iter()
            .filter(|e| {
                e.entity_id != query.actor_id
                    && !e.spectator
                    && e.can_collide_with_actor
                    && expanded.intersects(&e.bounds)
            })
            .map(|e| VoxelShape::from_aabb(e.bounds))
            .collect())
    }

    fn border_collision(&mut self, query: CollisionQuery) -> Result<Option<VoxelShape>> {
        self.validate_query(query)?;
        self.border
            .is_inside_close_to_border(self.actor.position, query.bounds)
            .then(|| self.border.collision_shape())
            .transpose()
    }

    fn all_blocks_air(&mut self, bounds: Aabb) -> Result<bool> {
        ensure!(finite_bounds(bounds), "nonfinite air-query bounds");
        let min = (
            java_floor(bounds.min_x),
            java_floor(bounds.min_y),
            java_floor(bounds.min_z),
        );
        let max = (
            java_floor(bounds.max_x),
            java_floor(bounds.max_y),
            java_floor(bounds.max_z),
        );
        checked_volume(min, max)?;
        for z in min.2..=max.2 {
            for y in min.1..=max.1 {
                for x in min.0..=max.0 {
                    let state = self.world.block(x, y, z)?;
                    if !catalog()
                        .states
                        .get(state as usize)
                        .context("unknown air-query state")?
                        .6
                    {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }
}
fn finite_bounds(b: Aabb) -> bool {
    [b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z]
        .iter()
        .all(|v| v.is_finite())
}
fn java_floor(v: f64) -> i32 {
    let n = v as i32;
    if v < f64::from(n) {
        n.wrapping_sub(1)
    } else {
        n
    }
}
fn checked_volume(min: (i32, i32, i32), max: (i32, i32, i32)) -> Result<()> {
    let dimensions = [
        i64::from(max.0) - i64::from(min.0) + 1,
        i64::from(max.1) - i64::from(min.1) + 1,
        i64::from(max.2) - i64::from(min.2) + 1,
    ];
    ensure!(
        dimensions.iter().all(|n| *n > 0),
        "invalid collision-query range"
    );
    let count = dimensions
        .iter()
        .try_fold(1u64, |n, &v| n.checked_mul(v as u64))
        .context("collision query volume overflow")?;
    ensure!(
        count <= MAX_QUERY_BLOCKS,
        "collision query exceeds available bounded context"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::FlatWorld;
    use serde_json::{json, Value};
    fn d(value: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
    }
    fn vec(value: &Value) -> Vec3 {
        Vec3::new(d(&value[0]), d(&value[1]), d(&value[2]))
    }
    fn bounds(value: &Value) -> Aabb {
        Aabb::new(
            d(&value[0]),
            d(&value[1]),
            d(&value[2]),
            d(&value[3]),
            d(&value[4]),
            d(&value[5]),
        )
    }
    fn bits(value: f64) -> String {
        format!("{:016x}", value.to_bits())
    }
    fn shape_value(shape: &VoxelShape, with_boxes: bool) -> Value {
        let mut out = json!({
            "coordinates":([Axis::X,Axis::Y,Axis::Z].map(|axis|shape.coordinates(axis).iter().copied().map(bits).collect::<Vec<_>>())),
            "occupied":shape.occupied().iter().map(|v|if *v{'1'}else{'0'}).collect::<String>(),
            "canonical_block":false,
        });
        if with_boxes {
            out["boxes"] = json!(shape
                .to_aabbs()
                .into_iter()
                .map(|b| [b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z].map(bits))
                .collect::<Vec<_>>());
        }
        out
    }
    fn empty_world() -> RuntimeWorld {
        RuntimeWorld::Fixture(FlatWorld::new(0, 0, 0, 0, 39))
    }

    #[test]
    fn original_java_runtime_world_queries() {
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/runtime_collision_vectors.json"))
                .unwrap();
        for (index, case) in cases["cases"].as_array().unwrap().iter().enumerate() {
            let q = &case["query"];
            let mut world = empty_world();
            if let RuntimeWorld::Fixture(flat) = &mut world {
                if let Some(blocks) = q["blocks"].as_array() {
                    for b in blocks {
                        flat.set_block(
                            b[0].as_i64().unwrap() as i32,
                            b[1].as_i64().unwrap() as i32,
                            b[2].as_i64().unwrap() as i32,
                            b[3].as_u64().unwrap() as u32,
                        );
                    }
                }
            }
            let position = if q["position"].is_null() {
                Vec3::new(0., 64., 0.)
            } else {
                vec(&q["position"])
            };
            let entities = q["entities"]
                .as_array()
                .map(|entities| {
                    entities
                        .iter()
                        .map(|e| CollisionEntity {
                            entity_id: e["id"].as_i64().unwrap() as i32,
                            bounds: bounds(&e["bounds"]),
                            spectator: e["spectator"].as_bool().unwrap(),
                            can_collide_with_actor: e["collidable"].as_bool().unwrap(),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut provider = RuntimeCollision::new(
                &mut world,
                CollisionActor {
                    entity_id: 1,
                    position,
                },
                &entities,
                CollisionBorder::default(),
            );
            let context = EntityCollisionContext {
                bottom_y: position.y,
                descending: q["descending"].as_bool().unwrap_or(false),
                fall_distance: q["fall"]
                    .as_str()
                    .map(|v| f32::from_bits(u32::from_str_radix(v, 16).unwrap()))
                    .unwrap_or(0.),
                is_falling_block: false,
                can_walk_on_powder_snow: false,
            };
            let query = CollisionQuery {
                actor_id: 1,
                bounds: bounds(&q["bounds"]),
                context,
            };
            let actual = match q["op"].as_str().unwrap() {
                "blocks" => json!(provider
                    .block_collisions(query)
                    .unwrap()
                    .iter()
                    .map(|s| shape_value(s, true))
                    .collect::<Vec<_>>()),
                "air" => json!(provider.all_blocks_air(query.bounds).unwrap()),
                "entities" => json!(provider
                    .entity_collisions(query)
                    .unwrap()
                    .iter()
                    .map(|s| shape_value(s, false))
                    .collect::<Vec<_>>()),
                "border" => {
                    let b = &q["border"];
                    provider.border =
                        CollisionBorder::from_center_size(d(&b[0]), d(&b[1]), d(&b[2]), 29_999_984)
                            .unwrap();
                    let active = provider.border_collision(query).unwrap().is_some();
                    json!({"active":active,"bounds":([provider.border.min_x,provider.border.min_z,provider.border.max_x,provider.border.max_z].map(bits)),"shape":shape_value(&provider.border.collision_shape().unwrap(),false)})
                }
                op => panic!("unknown runtime collision op {op}"),
            };
            assert_eq!(
                actual, case["expected"],
                "runtime collision case{index} query{q}"
            );
        }
    }

    #[test]
    fn missing_animation_and_missing_anvil_chunk_are_explicit_errors() {
        let shulker = catalog().states.iter().position(|row| row.1 == 6).unwrap() as u32;
        let context = EntityCollisionContext {
            bottom_y: 65.,
            descending: false,
            fall_distance: 0.,
            is_falling_block: false,
            can_walk_on_powder_snow: false,
        };
        assert!(
            state_collision_shape(shulker, (0, 64, 0), context, BlockEntityShape::Unavailable)
                .is_err()
        );
        let dir =
            std::env::temp_dir().join(format!("mc121-collision-empty-{}", rand::random::<u64>()));
        std::fs::create_dir_all(dir.join("region")).unwrap();
        let mut world = RuntimeWorld::Anvil(crate::storage::VanillaWorld::open(&dir).unwrap());
        let mut provider = RuntimeCollision::new(
            &mut world,
            CollisionActor {
                entity_id: 1,
                position: Vec3::new(0., 64., 0.),
            },
            &[],
            CollisionBorder::default(),
        );
        assert!(provider
            .block_collisions(CollisionQuery {
                actor_id: 1,
                bounds: Aabb::player(Vec3::new(0., 64., 0.), 0.6, 1.8),
                context
            })
            .is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ordinary_players_do_not_become_solid_obstacles_and_air_is_not_empty_shape() {
        let mut world = empty_world();
        let entities = [CollisionEntity {
            entity_id: 2,
            bounds: Aabb::player(Vec3::new(0., 64., 0.), 0.6, 1.8),
            spectator: false,
            can_collide_with_actor: false,
        }];
        let mut provider = RuntimeCollision::new(
            &mut world,
            CollisionActor {
                entity_id: 1,
                position: Vec3::new(0., 64., 0.),
            },
            &entities,
            CollisionBorder::default(),
        );
        let context = EntityCollisionContext {
            bottom_y: 64.,
            descending: false,
            fall_distance: 0.,
            is_falling_block: false,
            can_walk_on_powder_snow: false,
        };
        assert!(provider
            .entity_collisions(CollisionQuery {
                actor_id: 1,
                bounds: Aabb::player(Vec3::new(0., 64., 0.), 0.6, 1.8),
                context
            })
            .unwrap()
            .is_empty());
        // The air query consults the real isAir fact, not collision emptiness.
        let non_air_empty = catalog()
            .states
            .iter()
            .enumerate()
            .find(|(_, row)| !row.6 && row.1 == 0 && catalog().shapes[row.0].shape.is_empty())
            .unwrap()
            .0 as u32;
        if let RuntimeWorld::Fixture(flat) = provider.world {
            flat.set_block(0, 64, 0, non_air_empty);
        }
        assert!(!provider
            .all_blocks_air(Aabb::new(0., 64., 0., 1., 65., 1.))
            .unwrap());
    }

    #[test]
    fn unknown_fall_distance_only_blocks_queries_that_consume_it() {
        let mut world = empty_world();
        if let RuntimeWorld::Fixture(flat) = &mut world {
            flat.set_block(0, 64, 0, 1);
        }
        let mut provider = RuntimeCollision::new(
            &mut world,
            CollisionActor {
                entity_id: 1,
                position: Vec3::new(0.5, 64., 0.5),
            },
            &[],
            CollisionBorder::default(),
        )
        .without_fall_distance();
        let query = CollisionQuery {
            actor_id: 1,
            bounds: Aabb::player(Vec3::new(0.5, 64., 0.5), 0.6, 1.8),
            context: EntityCollisionContext {
                bottom_y: 64.,
                descending: false,
                // Deliberately unavailable, rather than a plausible zero.
                fall_distance: f32::NAN,
                is_falling_block: false,
                can_walk_on_powder_snow: false,
            },
        };
        assert!(!provider.block_collisions(query).unwrap().is_empty());
        let powder = catalog().states.iter().position(|row| row.1 == 5).unwrap() as u32;
        if let RuntimeWorld::Fixture(flat) = provider.world {
            flat.set_block(0, 64, 0, powder);
        }
        let error = provider.block_collisions(query).unwrap_err();
        assert!(error.to_string().contains("fall distance unavailable"));
        // isAir does not consume fallDistance, even for powder snow.
        assert!(!provider.all_blocks_air(query.bounds).unwrap());
    }
}
