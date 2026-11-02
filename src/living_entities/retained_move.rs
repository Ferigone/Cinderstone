//! Bounded inherited Entity position/collision bodies on one actual Bat lease.
//!
//! These functions are NOT Entity.move. Contact, fall, emission, profiler and
//! removal dispatch still belong to the real actor/Level dispatcher. In
//! particular a caller must not replace those phases with a call to `collide`.
//! No physical-state lock survives a position callback or a world query.

use crate::{
    entity_move,
    entity_state::java_floor,
    geometry::{Aabb, EntityCollisionContext, Vec3},
    mobs::actor::{BatActorError, NativeBatLease, NativeBatTypes},
    player_physics::{collide_motion, CollisionBody, CollisionQuery},
    runtime_collision::RuntimeCollision,
    runtime_world::RuntimeWorld,
};

pub type BlockPosition = (i32, i32, i32);

#[derive(Debug)]
pub enum BatPhysicalError {
    Actor(BatActorError),
    World(anyhow::Error),
    /// Existing RuntimeCollision filters IDs, whereas source excludes objects.
    ReusedCollisionId {
        entity_id: i32,
    },
    /// An actual source owner is required before this branch can execute.
    Unbound(&'static str),
}
impl From<BatActorError> for BatPhysicalError {
    fn from(error: BatActorError) -> Self {
        Self::Actor(error)
    }
}
impl From<anyhow::Error> for BatPhysicalError {
    fn from(error: anyhow::Error) -> Self {
        Self::World(error)
    }
}
impl std::fmt::Display for BatPhysicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Actor(error) => write!(f, "actual Bat physical owner: {error:?}"),
            Self::World(error) => error.fmt(f),
            Self::ReusedCollisionId { entity_id } => write!(
                f,
                "actual collision needs object identity for reused entity ID {entity_id}"
            ),
            Self::Unbound(owner) => write!(f, "actual Bat physical owner unavailable: {owner}"),
        }
    }
}
impl std::error::Error for BatPhysicalError {}

/// Entity.setPos(double,double,double), Java456–458, selected for actual Bat.
/// `position_changed` is the genuine EntityInLevelCallback.onMove operation,
/// including index/section changes. It runs after setPosRaw and BEFORE the
/// bounding box is rebuilt. It can synchronously reenter this SAME actor.
/// An error leaves setPosRaw committed and does not rebuild the box or retry.
pub fn set_position<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    position: Vec3,
    position_changed: impl FnOnce(&NativeBatLease<T>) -> Result<(), BatPhysicalError>,
) -> Result<(), BatPhysicalError> {
    // The actual lease also owns Entity's complementary ChunkPos cache; update
    // it inside this same raw-position boundary before publishing onMove.
    let changed = actor.set_position_raw(position)?;
    if changed {
        position_changed(actor)?;
    }
    // Source makeBoundingBox reads CURRENT dimensions and position after the
    // movement callback; a reentrant setter can have changed either one.
    actor.with_physical(|physical| physical.refresh_bounds())?;
    Ok(())
}

/// Entity.reapplyPosition466–467 selects Bat's inherited setPos body. This is
/// also used at its actual Living constructor boundary after blocksBuilding.
pub fn reapply_position<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    position_changed: impl FnOnce(&NativeBatLease<T>) -> Result<(), BatPhysicalError>,
) -> Result<(), BatPhysicalError> {
    let position = actor.with_physical(|physical| physical.position_vec())?;
    set_position(actor, position, position_changed)
}

/// Actual attribute/equipment results at the source collide access boundary.
/// These are immutable query arguments, never an alternative state owner.
#[derive(Clone, Copy, Debug)]
pub struct BatCollisionFacts {
    pub step_height: f32,
    pub can_walk_on_powder_snow: bool,
}

fn collision_body<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    facts: BatCollisionFacts,
) -> Result<CollisionBody, BatPhysicalError> {
    Ok(actor.with_physical(|physical| CollisionBody {
        actor_id: physical.entity_id,
        bounds: physical.bounds,
        context: EntityCollisionContext {
            bottom_y: physical.position.y,
            descending: physical.shared_flags & 2 != 0,
            fall_distance: physical.fall_distance,
            is_falling_block: false,
            can_walk_on_powder_snow: facts.can_walk_on_powder_snow,
        },
        step_height: facts.step_height,
        on_ground: physical.on_ground(),
    })?)
}

fn bind_collision<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    collision: &mut RuntimeCollision<'_>,
) -> Result<(), BatPhysicalError> {
    let (id, position) =
        actor.with_physical(|physical| (physical.entity_id, physical.position_vec()))?;
    if collision.actor.entity_id != id {
        return Err(BatActorError::ForeignActor.into());
    }
    collision.actor.position = position;
    Ok(())
}

/// Entity.collide1028–1068 geometric body on the canonical physical fields.
/// `collision` borrows the actual RuntimeWorld/border and original ordered
/// entity candidates with their real canCollideWith relation. Unknown shapes
/// remain errors in RuntimeCollision; no empty entity/world owner is invented.
/// This returns the source movement value and does not perform Entity.move's
/// later mutations/callbacks. `facts` must come from actual Bat attributes and
/// equipment, rather than falling-block or creative-player constants.
/// Candidates must already exclude THIS actor by source object identity. A
/// candidate sharing its numeric ID therefore names another lifetime, which
/// this existing numeric collision adapter cannot represent and must reject.
pub fn collide<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    movement: Vec3,
    facts: BatCollisionFacts,
    collision: &mut RuntimeCollision<'_>,
) -> Result<Vec3, BatPhysicalError> {
    bind_collision(actor, collision)?;
    let body = collision_body(actor, facts)?;
    if collision
        .entities
        .iter()
        .any(|candidate| candidate.entity_id == body.actor_id)
    {
        return Err(BatPhysicalError::ReusedCollisionId {
            entity_id: body.actor_id,
        });
    }
    Ok(collide_motion(collision, body, movement)?)
}

/// Entity.setOnGroundWithMovement650–652 / checkSupportingBlock659–687.
/// Queries use the real ordered BlockCollisions/support selector. Bat does not
/// override these bodies. The world adapter is callback-free; nevertheless
/// canonical physical borrows are released before each query.
pub fn set_on_ground_with_movement<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    on_ground: bool,
    movement: Option<Vec3>,
    can_walk_on_powder_snow: bool,
    collision: &mut RuntimeCollision<'_>,
) -> Result<(), BatPhysicalError> {
    // Bind before changing a field: a foreign query owner is an admission
    // failure, not a successfully executed source setOnGround phase.
    bind_collision(actor, collision)?;
    actor.with_physical(|physical| physical.set_on_ground(on_ground))?;
    if !on_ground {
        actor.with_physical(|physical| {
            physical.on_ground_no_blocks = false;
            if physical.supporting_block.is_some() {
                physical.supporting_block = None;
            }
        })?;
        return Ok(());
    }
    let body = collision_body(
        actor,
        BatCollisionFacts {
            step_height: 0.,
            can_walk_on_powder_snow,
        },
    )?;
    let b = body.bounds;
    let probe = Aabb::new(b.min_x, b.min_y - 1e-6, b.min_z, b.max_x, b.min_y, b.max_z);
    let query = |bounds| CollisionQuery {
        actor_id: body.actor_id,
        bounds,
        context: body.context,
    };
    let mut supporting = crate::entity_support::find_supporting_block(collision, query(probe))?;
    if supporting.is_some() || actor.with_physical(|p| p.on_ground_no_blocks)? {
        actor.with_physical(|p| p.supporting_block = supporting)?;
    } else if let Some(movement) = movement {
        let back = probe.moved(Vec3::new(-movement.x, 0., -movement.z));
        supporting = crate::entity_support::find_supporting_block(collision, query(back))?;
        actor.with_physical(|p| p.supporting_block = supporting)?;
    }
    actor.with_physical(|p| p.on_ground_no_blocks = supporting.is_none())?;
    Ok(())
}

/// Inherited Entity.getOnPos(offset), Java934–956, preserving the retained
/// support position and fence/wall/gate test before reading current actor Y.
pub fn on_position<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    offset: f32,
    world: &mut RuntimeWorld,
) -> Result<BlockPosition, BatPhysicalError> {
    if let Some(support) = actor.with_physical(|p| p.supporting_block)? {
        // Source fcmpl/ifle skips this branch for NaN as well.
        if offset > 1e-5_f32 {
            let block = world.block(support.0, support.1, support.2)?;
            let facts = *entity_move::block_facts(block)?;
            if (f64::from(offset) <= 0.5 && facts.fence()) || facts.wall() || facts.fence_gate() {
                return Ok(support);
            }
            let y = actor.with_physical(|p| p.position.y)?;
            return Ok((support.0, java_floor(y - f64::from(offset)), support.2));
        }
        return Ok(support);
    }
    Ok(actor.with_physical(|p| {
        (
            java_floor(p.position.x),
            java_floor(p.position.y - f64::from(offset)),
            java_floor(p.position.z),
        )
    })?)
}

/// Selected Entity.getBlockSpeedFactor976–981 body. LivingEntity's virtual
/// override is separate and must still read its actual MOVEMENT_EFFICIENCY
/// attribute and interpolate this value toward1 at its source call boundary.
pub fn entity_block_speed_factor<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    world: &mut RuntimeWorld,
) -> Result<f32, BatPhysicalError> {
    let at = actor.with_physical(|p| p.block_position)?;
    let inside = world.block(at.0, at.1, at.2)?;
    let facts = *entity_move::block_facts(inside)?;
    let factor = facts.speed_factor();
    if facts.water() || facts.bubble_column() {
        return Ok(factor);
    }
    if f64::from(factor) == 1. {
        let below = on_position(actor, 0.500001_f32, world)?;
        return Ok(
            entity_move::block_facts(world.block(below.0, below.1, below.2)?)?.speed_factor(),
        );
    }
    Ok(factor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        effects::{
            active_map::ActiveEffectKey,
            attribute_map::JavaObjectKey,
            location_map::{EnchantmentValueReference, LocationEffectReference},
            supplier::RegisteredAttributeHolder,
        },
        entity::Position,
        entity_state::EntityPhysicalState,
        mobs::actor::BatEntityFields,
        random::LegacyRandomSource,
        runtime_collision::{CollisionActor, CollisionBorder, CollisionEntity},
        world::FlatWorld,
    };

    // Controlled preconstructor field fixture for physical-method checks only.
    // External object domains are uninhabited: no attribute/effect/stack/RNG
    // registry, completed Entity constructor or admitted Main actor is faked.
    #[derive(Clone, PartialEq, Eq)]
    enum NoExternalObject {}
    impl JavaObjectKey for NoExternalObject {
        fn java_hash_code(&self) -> i32 {
            match *self {}
        }
    }
    impl RegisteredAttributeHolder for NoExternalObject {}
    impl ActiveEffectKey for NoExternalObject {
        fn is_canonical_reference(&self) -> bool {
            match *self {}
        }
        fn java_hash_code(&self) -> i32 {
            match *self {}
        }
        fn same_object(&self, _: &Self) -> bool {
            match *self {}
        }
    }
    impl EnchantmentValueReference for NoExternalObject {
        fn same_object(&self, _: &Self) -> bool {
            match *self {}
        }
    }
    impl LocationEffectReference for NoExternalObject {
        fn same_object(&self, _: &Self) -> bool {
            match *self {}
        }
        fn java_equals(&self, _: &Self) -> bool {
            match *self {}
        }
    }
    struct Types;
    impl NativeBatTypes for Types {
        type ItemStack = NoExternalObject;
        type DamageSource = NoExternalObject;
        type FallLocation = NoExternalObject;
        type EnchantmentValue = NoExternalObject;
        type LocationEffect = NoExternalObject;
        type AttributeHolder = NoExternalObject;
        type InstanceIdentity = NoExternalObject;
        type EffectHolder = NoExternalObject;
        type EntityType = ();
        type Level = ();
        type ProfilerSupplier = ();
        type LootTable = NoExternalObject;
        type LeashData = NoExternalObject;
    }
    fn fixture() -> NativeBatLease<Types> {
        let physical = EntityPhysicalState::new(
            23,
            [8; 16],
            Position {
                x: 0.,
                y: 0.,
                z: 0.,
                yaw: 0.,
                pitch: 0.,
                on_ground: false,
            },
            Vec3::ZERO,
            0.5,
            0.9,
            0.45,
        )
        .unwrap();
        NativeBatLease::attach_to_entity_prefix(
            physical,
            BatEntityFields::jvm_zeroed(),
            LegacyRandomSource::new(1),
            (),
            (),
        )
    }

    #[test]
    fn same_physical_reentrant_on_move_precedes_current_aabb_rebuild() {
        let actor = fixture();
        let old = actor.with_physical(|p| p.bounds).unwrap();
        set_position(&actor, Vec3::new(17., 2., -17.), |same| {
            assert!(actor.same_actor(same));
            assert_eq!(
                same.with_physical(|p| (p.position_vec(), p.bounds))
                    .unwrap(),
                (Vec3::new(17., 2., -17.), old)
            );
            assert_eq!(
                same.with_fields(|f| f.entity.chunk_position).unwrap(),
                (1, -2)
            );
            set_position(same, Vec3::new(4., 5., 6.), |nested| {
                assert!(same.same_actor(nested));
                assert_eq!(nested.with_physical(|p| p.bounds).unwrap(), old);
                Ok(())
            })
        })
        .unwrap();
        assert_eq!(
            actor
                .with_physical(|p| (p.position_vec(), p.bounds.min_x, p.bounds.min_y))
                .unwrap(),
            (Vec3::new(4., 5., 6.), 3.75, 5.)
        );
        assert!(!actor.is_constructor_complete().unwrap());
    }

    #[test]
    fn failed_on_move_keeps_actual_raw_prefix_and_equal_zero_still_refreshes_box() {
        let actor = fixture();
        let old = actor.with_physical(|p| p.bounds).unwrap();
        let result = set_position(&actor, Vec3::new(17., 2., -17.), |_| {
            Err(BatPhysicalError::Unbound(
                "controlled actual callback failure",
            ))
        });
        assert!(matches!(result, Err(BatPhysicalError::Unbound(_))));
        assert_eq!(
            actor
                .with_physical(|p| (p.position_vec(), p.bounds))
                .unwrap(),
            (Vec3::new(17., 2., -17.), old)
        );
        assert_eq!(
            actor.with_fields(|f| f.entity.chunk_position).unwrap(),
            (1, -2)
        );

        let actor = fixture();
        actor.with_physical(|p| p.width = 1.).unwrap();
        set_position(&actor, Vec3::new(-0., -0., -0.), |_| {
            panic!("equal signed zero moved")
        })
        .unwrap();
        assert_eq!(actor.with_physical(|p| p.position.x.to_bits()).unwrap(), 0);
        assert_eq!(actor.with_physical(|p| p.bounds.min_x).unwrap(), -0.5);
    }

    #[test]
    fn source_nan_support_avoids_query_and_reused_id_collision_is_explicit_error() {
        let actor = fixture();
        let at = (20, 10, 20);
        actor
            .with_physical(|p| p.supporting_block = Some(at))
            .unwrap();
        // Deliberately unsupported fixture state: any accidental block-facts
        // query must fail, while the source NaN support branch makes none.
        let flat = FlatWorld::new(32_767, 32_767, 32_767, 32_767, 39);
        let mut world = RuntimeWorld::Fixture(flat);
        assert_eq!(on_position(&actor, f32::NAN, &mut world).unwrap(), at);
        assert!(on_position(&actor, 0.2, &mut world).is_err());

        let body = actor.with_physical(|p| p.bounds).unwrap();
        let candidates = [CollisionEntity {
            entity_id: 23,
            bounds: body,
            spectator: false,
            can_collide_with_actor: true,
        }];
        let mut collision = RuntimeCollision::new(
            &mut world,
            CollisionActor {
                entity_id: 23,
                position: Vec3::ZERO,
            },
            &candidates,
            CollisionBorder::default(),
        );
        assert!(matches!(
            collide(
                &actor,
                Vec3::ZERO,
                BatCollisionFacts {
                    step_height: 0.6,
                    can_walk_on_powder_snow: false
                },
                &mut collision
            ),
            Err(BatPhysicalError::ReusedCollisionId { entity_id: 23 })
        ));
    }
}
