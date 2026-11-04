//! Real FallingBlockEntity Level.clip(COLLIDER, SOURCE_ONLY, this) query.
//!
//! Direct pinned-JDK javap of dcc (BlockGetter), dcf (ClipContext) and exm
//! (EntityCollisionContext) confirms the retained traversal/voxel primitives,
//! block-wins-distance-ties and strictly-closer direction-only interaction
//! override. SOURCE_ONLY includes source lava; WATER is checked after the hit.
//! Original inner SHA256: 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
//! Mappings SHA1: 31c77994d96f05ba25a870ada70f47f315330437.
//!
//! This callable query is not complete trajectory admission. Animated block-
//! entity grids and overridden interaction shapes remain explicit unsupported
//! capabilities; production must establish its pre-mutation profile separately.
use super::*;
use super::{main_falling_block_runtime::FallingBlockPublication,
    main_falling_block_spawn::ActualFallingBlockEditServices};
use mc121_rust::{
    bucket_interactions,
    entity_state::java_floor,
    falling_block::FallingBlockState,
    fluid_physics::{FluidState, RuntimeFluidWorld},
    geometry::{self, BlockEntityShape, BlockHit, EntityCollisionContext},
    runtime_collision,
    runtime_entity_index::EntityKind,
};

const MAX_CLIP_CELLS: usize = 1_048_576;

fn same_components(a: Vec3, b: Vec3) -> bool {
    a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits()
        && a.z.to_bits() == b.z.to_bits()
}

/// Preserve the actual actor reference, generation, post-move collision
/// context, source world, current FULL region and existing fluid shape cache.
/// No entity, Level, cache history, Player context or readiness is constructed.
/// The caller maps a real hit to its position and only then queries WATER.
pub(super) fn clip_collider_source_only(
    services: &mut ActualFallingBlockEditServices<'_>,
    publication: &FallingBlockPublication,
    actor: &FallingBlockState,
    from: Vec3,
    to: Vec3,
) -> Result<Option<BlockHit>> {
    ensure!(same_components(from, actor.physical.old_position)
        && same_components(to, actor.physical.position_vec()),
        "falling clip endpoints are not the actual old/current actor positions");
    ensure!([from.x, from.y, from.z, to.x, to.y, to.z].into_iter()
        .all(|value| value.is_finite() && value > f64::from(i32::MIN) + 1.
            && value < f64::from(i32::MAX) - 1.),
        "falling clip is outside the finite native DDA coordinate profile");
    ensure!(actor.entity_id == publication.handle.id
        && services.items.entity_handle(actor.entity_id) == Some(publication.handle)
        && services.items.entity_kind(publication.handle) == Some(EntityKind::FallingBlock),
        "falling clip lost its actual retained entity/generation");
    let identities = services.external_entity_uuids
        .context("falling clip lacks the actual same-lock external identity view")?;
    ensure!(identities.iter().any(|(id, uuid)| *id == actor.entity_id && *uuid == actor.physical.uuid),
        "falling clip physical identity differs from its retained actor");
    ensure!(matches!(services.world, WorldBackend::Fixture(_)),
        "falling clip has no actual native imported-world owner");
    services.context.dimension.require_runtime_layout()?;
    services.source.as_ref().context("falling clip lacks its actual native source")?
        .require_current_world(services.world)?;
    let region = services.query_region.context("falling clip lacks an actual current FULL region")?;

    // ClipContext constructs EntityCollisionContext.of(this) AFTER Entity.move.
    // These are the SAME actual physical fields used by Falling movement, with
    // this nonliving Falling class's powder-snow behavior and no held boots.
    let context = EntityCollisionContext {
        bottom_y: actor.physical.position.y,
        descending: actor.physical.shared_flags & 2 != 0,
        fall_distance: actor.physical.fall_distance,
        is_falling_block: true,
        can_walk_on_powder_snow: false,
    };
    let mut visits = 0_usize;
    let mut visit = |position: (i32, i32, i32)| -> Option<Result<BlockHit>> {
        let query = (|| -> Result<Option<BlockHit>> {
            visits = visits.checked_add(1).context("falling clip traversal counter overflow")?;
            ensure!(visits <= MAX_CLIP_CELLS, "falling clip exceeds its bounded native traversal profile");
            let chunk = (position.0.div_euclid(16), position.2.div_euclid(16));
            ensure!(region.contains(chunk), "falling clip escaped its actual current query region");
            services.chunks.manager.certify_full_region(chunk, chunk)?;

            // BlockGetter reads both actual states before querying either
            // shape. The native FluidState owner is independent of the known
            // incomplete placement fixture's fluid flag array.
            let state = services.world.block(position.0, position.1, position.2)?;
            let fluid = FluidState::from_block(state)?;
            let collision = runtime_collision::state_collision_shape_local(
                state, position, context, BlockEntityShape::Unavailable)?;
            let mut block_hit = collision.clip(from, to, position);
            if let Some(hit) = block_hit.as_mut() {
                let interaction = bucket_interactions::static_interaction_shape(state)?;
                if let Some(other) = interaction.clip(from, to, position) {
                    if from.distance_squared(other.location) < from.distance_squared(hit.location) {
                        // Source clipWithInteractionOverride changes the face,
                        // retaining collision location, position and inside.
                        hit.direction = other.direction;
                    }
                }
            }

            // Never filter to water or short-circuit this shape query because
            // a block already hit. The real process-wide cache owns getShape.
            let fluid_hit = if fluid.source {
                services.interactions.fluid_shapes.shape(
                    &mut RuntimeFluidWorld { world: services.world }, position, fluid)?
                    .clip(from, to, position)
            } else { None };
            Ok(match (block_hit, fluid_hit) {
                (Some(block), Some(fluid)) => Some(
                    if from.distance_squared(block.location) <= from.distance_squared(fluid.location) {
                        block
                    } else { fluid }),
                (Some(block), None) => Some(block),
                (None, fluid) => fluid,
            })
        })();
        match query {
            Ok(Some(hit)) => Some(Ok(hit)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }
    };

    // Original Vec3.equals uses Double.compare, including signed zero. The
    // shared DDA's numerical equality fast path would otherwise omit the one
    // genuine first-cell shape/cache query in this zero-length edge case.
    let result = if from == to && !same_components(from, to) {
        let start = from.lerp(to, -1.0e-7);
        visit((java_floor(start.x), java_floor(start.y), java_floor(start.z)))
    } else {
        // Existing source DDA retains outward 1e-7 endpoint padding and strict
        // Z/Y/X tie order; VoxelShape retains its .001 inside sample and face.
        geometry::traverse_blocks(from, to, visit)
    };
    result.transpose()
}
