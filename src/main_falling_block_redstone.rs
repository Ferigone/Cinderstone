//! Pinned pressure-plate source predicates and actual Falling entry adapter.
//!
//! BasePressurePlateBlock.entityInside receives the genuine triggering Entity;
//! its count query independently filters class, spectator and trigger-ignore
//! predicates. A nonliving trigger must not suppress an actual Living query.
use super::*;
use super::{main_falling_block_runtime::FallingBlockPublication,
    main_falling_block_spawn::ActualFallingBlockEditServices};
use mc121_rust::{entity_state::EntityPhysicalState, placement::BlockPosition,
    runtime_entity_index::EntityKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PressurePlateProfile {
    Everything,
    LivingOnly,
    Weighted { maximum: i32 },
}

pub(super) fn pressure_plate_profile(state: u32) -> Result<Option<PressurePlateProfile>> {
    let block = mc121_rust::redstone::block(state)?;
    match block.class.as_str() {
        "dlw" => {
            ensure!(block.max_weight == 0 && block.pressed_time > 0,
                "actual binary pressure-plate source facts are unavailable");
            Ok(Some(if block.mobs_only { PressurePlateProfile::LivingOnly }
                else { PressurePlateProfile::Everything }))
        }
        "dpp" => {
            ensure!(block.max_weight > 0 && block.pressed_time > 0,
                "actual weighted pressure-plate source facts are unavailable");
            Ok(Some(PressurePlateProfile::Weighted { maximum: block.max_weight }))
        }
        _ => Ok(None),
    }
}

/// Original BasePressurePlateBlock.TOUCH_AABB, NOT its visual/collision shape.
/// The actual class initializer uses1/16..15/16 horizontally and height1/4.
pub(super) fn pressure_touch_bounds(position: BlockPosition) -> Aabb {
    Aabb::new(f64::from(position.0) + 0.0625, f64::from(position.1),
        f64::from(position.2) + 0.0625, f64::from(position.0) + 0.9375,
        f64::from(position.1) + 0.25, f64::from(position.2) + 0.9375)
}

/// These are the actual native runtime classes, not a default for arbitrary
/// EntityTypes. All inherit Entity.isIgnoringBlockTriggers' literal false body.
/// Future indexed classes must explicitly port their own override here.
pub(super) fn native_plate_trigger_ignores(kind: EntityKind) -> bool {
    match kind {
        EntityKind::Player | EntityKind::Item | EntityKind::ExperienceOrb
            | EntityKind::FallingBlock => false,
    }
}

/// PressurePlateBlock's MOBS sensitivity queries LivingEntity.class; health is
/// not its predicate. A still-indexed dead Player is not made nonliving, and a
/// real FallingBlock must not be represented by a dummy Player/health value.
pub(super) fn native_plate_is_living(kind: EntityKind) -> bool {
    match kind {
        EntityKind::Player => true,
        EntityKind::Item | EntityKind::ExperienceOrb | EntityKind::FallingBlock => false,
    }
}

pub(super) fn entity_inside(
    services: &mut ActualFallingBlockEditServices<'_>,
    publication: &mut FallingBlockPublication,
    actor: &mut EntityPhysicalState,
    position: BlockPosition,
) -> Result<()> {
    ensure!(actor.entity_id == publication.handle.id && !actor.removed
        && services.items.entity_kind(publication.handle) == Some(EntityKind::FallingBlock),
        "pressure-plate callback lost its actual Falling entity/generation");
    let state = services.world.block(position.0, position.1, position.2)?;
    ensure!(pressure_plate_profile(state)?.is_some(),
        "pressure-plate entityInside has an unavailable actual declaring class");
    // Do not early-return for a stone plate because the trigger is nonliving:
    // its source checkPressed may still count a real Player already on it.
    // The shared kernel owns current-power/query/set/neighbor/sound/event/
    // schedule order; the real wrapper retains this actor as event context.
    main_redstone::falling_pressure_plate_inside(services, publication, actor, position)
}
