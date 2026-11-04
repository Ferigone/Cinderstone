//! One-tick capability admission for the actual falling COLLIDER/SOURCE_ONLY ray.
//!
//! This does not choose a trajectory or hit. It admits the capabilities for a
//! conservative region containing every next SELF-move endpoint and padded ray
//! cell; the real move, callbacks and later clip retain their source phases.
use super::*;
use super::{
    main_falling_block_runtime::FallingBlockPublication,
    main_falling_block_spawn::ActualFallingBlockEditServices,
};
use mc121_rust::{
    bucket_interactions,
    entity_move::MOVE_EPSILON_SQUARED,
    entity_state::java_floor,
    falling_block::FallingBlockState,
    fluid_physics::{FluidShapeHistory, FluidState},
    runtime_collision,
    runtime_entity_index::{EntityHandle, EntityKind},
};

const MAX_ADMISSION_CELLS: u64 = 1_048_576;

/// One genuine preflight invocation. Private fields and no Clone/default
/// prevent fabricating or replaying the successful capability scan. The Main
/// caller retains the same services and canonical actor across begin_level_tick.
pub(super) struct PreparedNextTickClip {
    handle: EntityHandle,
    uuid: [u8; 16],
    world_revision: u64,
    now: i64,
    block_state: u32,
    time: i32,
    next_tick_count: i32,
    inputs: MovementInputs,
    min_chunk: (i32, i32),
    max_chunk: (i32, i32),
}

#[derive(PartialEq, Eq)]
struct MovementInputs {
    position: [u64; 3],
    velocity: [u64; 3],
    stuck: [u64; 3],
    bounds: [u64; 6],
    no_gravity: bool,
    no_physics: bool,
    on_ground: bool,
    shared_flags: u8,
    fall_distance: u32,
}
impl MovementInputs {
    fn capture(actor: &FallingBlockState) -> Self {
        let body = &actor.physical;
        let bounds = body.bounds;
        Self {
            position: vector_bits(body.position_vec()),
            velocity: vector_bits(body.velocity),
            stuck: vector_bits(body.stuck_speed_multiplier),
            bounds: [bounds.min_x.to_bits(), bounds.min_y.to_bits(), bounds.min_z.to_bits(),
                bounds.max_x.to_bits(), bounds.max_y.to_bits(), bounds.max_z.to_bits()],
            no_gravity: body.no_gravity,
            no_physics: body.no_physics,
            on_ground: body.on_ground(),
            shared_flags: body.shared_flags,
            fall_distance: body.fall_distance.to_bits(),
        }
    }
}
impl PreparedNextTickClip {
    /// Consume once at the core's first host preflight, immediately after the
    /// actual begin_level_tick and BEFORE time/gravity/movement. This checks
    /// the real generation and existing revision certificate, with no second
    /// world/source/shape/cache scan or replacement source identity.
    pub(super) fn validate_after_begin(
        self,
        services: &ActualFallingBlockEditServices<'_>,
        publication: &FallingBlockPublication,
        actor: &FallingBlockState,
    ) -> Result<()> {
        ensure!(!actor.removed && actor.entity_id == self.handle.id
            && publication.handle == self.handle
            && services.items.entity_handle(actor.entity_id) == Some(self.handle)
            && services.items.entity_kind(self.handle) == Some(EntityKind::FallingBlock)
            && actor.physical.uuid == self.uuid,
            "falling clip invocation lost its actual actor/generation/UUID");
        ensure!(*services.world_revision == self.world_revision && services.now == self.now,
            "falling clip invocation changed its actual world revision/phase");
        ensure!(actor.block_state == self.block_state && actor.time == self.time
            && actor.physical.tick_count == self.next_tick_count
            && vector_bits(actor.physical.old_position) == self.inputs.position
            && MovementInputs::capture(actor) == self.inputs
            && actor.vehicle.is_none() && actor.portal_process.is_none(),
            "falling clip invocation differs from its admitted begin-level transition");
        let region = services.query_region.context("falling clip invocation lost its query region")?;
        ensure!(region.contains(self.min_chunk) && region.contains(self.max_chunk),
            "falling clip invocation lost its admitted actual region");
        Ok(())
    }
}

fn vector_bits(value: Vec3) -> [u64; 3] {
    [value.x.to_bits(), value.y.to_bits(), value.z.to_bits()]
}

/// Call before begin_level_tick, time/gravity and Entity.move. This certifies
/// only this tick's possible clip cells and provider branches, never a complete
/// future falling path or successful execution of all movement callbacks.
pub(super) fn preflight_next_tick_clip(
    services: &mut ActualFallingBlockEditServices<'_>,
    publication: &FallingBlockPublication,
    actor: &FallingBlockState,
) -> Result<PreparedNextTickClip> {
    actor.validate()?;
    ensure!(!actor.removed && actor.entity_id == publication.handle.id
        && services.items.entity_handle(actor.entity_id) == Some(publication.handle)
        && services.items.entity_kind(publication.handle) == Some(EntityKind::FallingBlock),
        "falling clip admission lacks its actual live generation");
    let identities = services.external_entity_uuids
        .context("falling clip admission lacks actual same-lock external identities")?;
    ensure!(identities.iter().any(|(id, uuid)| *id == actor.entity_id && *uuid == actor.physical.uuid),
        "falling clip admission differs from the retained physical actor");
    ensure!(actor.vehicle.is_none() && actor.portal_process.is_none(),
        "falling clip admission cannot bound an active vehicle/portal owner");
    ensure!(matches!(services.world, WorldBackend::Fixture(_)),
        "falling clip admission requires the actual native backend");
    services.context.dimension.require_runtime_layout()?;
    services.gameplay.require_supported_floor()?;
    let source = services.source.as_ref().context("falling clip admission lacks its native source")?;
    source.require_current_world(services.world)?;
    source.require_fluid_history(services.world, &services.interactions.fluid_shapes)?;
    let region = services.query_region.context("falling clip admission lacks its current FULL region")?;

    // FallingMoveWorld always calls SELF, identity edge backoff and step0.
    // Thus no piston accumulation or step-up branch can enlarge displacement.
    // Account for the ACTUAL retained stuck multiplier; noPhysics bypasses that
    // branch exactly as Entity.move does. No actor field is changed here.
    let velocity = if actor.physical.no_gravity { actor.physical.velocity }
        else { actor.physical.velocity.plus(Vec3::new(0., -0.04, 0.)) };
    let stuck = actor.physical.stuck_speed_multiplier;
    let requested = if !actor.physical.no_physics
        && stuck.length_squared() > MOVE_EPSILON_SQUARED {
        Vec3::new(velocity.x * stuck.x, velocity.y * stuck.y, velocity.z * stuck.z)
    } else { velocity };
    let start = actor.physical.position_vec();
    let requested_end = start.plus(requested);
    ensure!([start.x, start.y, start.z, requested.x, requested.y, requested.z,
        requested_end.x, requested_end.y, requested_end.z].into_iter().all(f64::is_finite),
        "falling clip admission has a nonfinite actual movement envelope");
    let point_envelope = Aabb::new(start.x.min(requested_end.x), start.y.min(requested_end.y),
        start.z.min(requested_end.z), start.x.max(requested_end.x),
        start.y.max(requested_end.y), start.z.max(requested_end.z));
    // Zero-step collision clips each axis toward zero, with the source's tiny
    // epsilon allowing an opposite residual. Two blocks cover that residual,
    // collision-neighbor probes and the above-fluid read. The proportional
    // term also covers both outward1e-7 DDA endpoint extensions, at any admitted
    // finite velocity. Neither endpoint is asserted to be the later result.
    let padding = Vec3::new(2. + requested.x.abs() * 1e-7,
        2. + requested.y.abs() * 1e-7, 2. + requested.z.abs() * 1e-7);
    let envelope = actor.physical.bounds.expand_towards(requested)
        .minmax(point_envelope).inflate(padding);
    let (min, max) = bounded_cells(envelope)?;
    let min_chunk = (min.0.div_euclid(16), min.2.div_euclid(16));
    let max_chunk = (max.0.div_euclid(16), max.2.div_euclid(16));
    ensure!(region.contains(min_chunk) && region.contains(max_chunk),
        "falling clip admission escapes its actual query-region certificate");
    // Read the actual positive current-FULL capability. Admission never runs
    // the chunk scheduler or a readiness/transport/publication callback.
    services.chunks.manager.certify_full_region(min_chunk, max_chunk)?;

    let mut checked_states = BTreeSet::new();
    for x in min.0..=max.0 {
        for y in min.1..=max.1 {
            for z in min.2..=max.2 {
                let position = (x, y, z);
                let state = services.world.block(x, y, z)?;
                let block = mc121_rust::redstone::block(state)?;
                if matches!(block.class.as_str(), "dlw" | "dpp") {
                    // The current successful pressure parts profile can only
                    // change this SAME actual Block's power, with inherited
                    // neighboring callbacks. Admit its entire captured state
                    // domain, not a guessed powered state or a fake callback.
                    ensure!(main_falling_block_redstone::pressure_plate_profile(state)?.is_some(),
                        "falling clip admission has an unowned pressure Block");
                    for possible in block.min..=block.max {
                        admit_state(services, position, possible, &mut checked_states)?;
                    }
                } else {
                    admit_state(services, position, state, &mut checked_states)?;
                }
            }
        }
    }
    Ok(PreparedNextTickClip {
        handle: publication.handle,
        uuid: actor.physical.uuid,
        world_revision: *services.world_revision,
        now: services.now,
        block_state: actor.block_state,
        time: actor.time,
        next_tick_count: actor.physical.tick_count.wrapping_add(1),
        inputs: MovementInputs::capture(actor),
        min_chunk,
        max_chunk,
    })
}

fn admit_state(
    services: &mut ActualFallingBlockEditServices<'_>,
    position: (i32, i32, i32),
    state: u32,
    checked_states: &mut BTreeSet<u32>,
) -> Result<()> {
    if checked_states.insert(state) {
        // The pure metadata inspector validates ALL source selectors/assets:
        // scaffold above1/not-descending, above0/bottom, empty, powder-snow
        // fallDistance>2.5 versus Falling-full, and coordinate offset grids.
        // No premove/counterfactual actor context or shape is passed to an
        // actual-query API. Animated kinds6/7 remain explicitly unsupported.
        runtime_collision::require_falling_collision_shape_capability(state)?;
        // Conservative: require the actual immutable interaction provider even
        // for a cell whose eventual collider may miss. Custom/animated
        // interaction grids remain explicit unsupported capabilities.
        let _ = bucket_interactions::static_interaction_shape(state)?;
    }
    let fluid = FluidState::from_block(state)?;
    if fluid.source {
        match services.interactions.fluid_shapes.history(fluid)? {
            FluidShapeHistory::Unknown => bail!(
                "falling clip admission lacks actual fluid shape history at {position:?}"),
            FluidShapeHistory::Unqueried => {
                // Merely certify the real above-cell read. In particular do
                // NOT call shape/fluid_height or cache a speculative height:
                // actual first-query history belongs to the source ray phase.
                let above = (position.0, position.1.checked_add(1)
                    .context("falling clip above-fluid coordinate overflow")?, position.2);
                services.chunks.manager.certify_full_region(
                    (above.0.div_euclid(16), above.2.div_euclid(16)),
                    (above.0.div_euclid(16), above.2.div_euclid(16)))?;
                FluidState::from_block(services.world.block(above.0, above.1, above.2)?)?;
            }
            FluidShapeHistory::CachedHeight(_) => {}
        }
    }
    Ok(())
}

fn bounded_cells(bounds: Aabb) -> Result<((i32, i32, i32), (i32, i32, i32))> {
    ensure!([bounds.min_x, bounds.min_y, bounds.min_z, bounds.max_x, bounds.max_y, bounds.max_z]
        .into_iter().all(|value| value.is_finite() && value > f64::from(i32::MIN) + 2.
            && value < f64::from(i32::MAX) - 2.),
        "falling clip admission is outside its finite native coordinate profile");
    let min = (java_floor(bounds.min_x), java_floor(bounds.min_y), java_floor(bounds.min_z));
    let max = (java_floor(bounds.max_x), java_floor(bounds.max_y), java_floor(bounds.max_z));
    let volume = [i64::from(max.0) - i64::from(min.0) + 1,
        i64::from(max.1) - i64::from(min.1) + 1, i64::from(max.2) - i64::from(min.2) + 1]
        .into_iter().try_fold(1_u64, |volume, width|
            u64::try_from(width).ok().and_then(|width| volume.checked_mul(width)));
    ensure!(volume.is_some_and(|value| value <= MAX_ADMISSION_CELLS),
        "falling clip admission exceeds its one-tick bounded cell profile");
    Ok((min, max))
}
