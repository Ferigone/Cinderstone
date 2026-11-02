//! Actual shared-level and sender entrypoints for retained falling actors.
//! The canonical Box remains in its runtime while world callbacks borrow the
//! other owners. Chunk readiness and sender range come from the real manager.

use super::*;
use super::{
    main_falling_block_runtime::MainFallingBlockRuntime,
    main_falling_block_spawn::ActualFallingBlockEditServices,
};
use mc121_rust::{
    entity_spatial,
    runtime_chunk_manager::RuntimeChunkManager,
    runtime_entity_index::{EntityHandle, EntityKind},
};

impl Server {
    pub(super) fn tick_native_falling_block_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        handle: EntityHandle,
        game_time: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        ensure!(players.item_runtime.entity_kind(handle) == Some(EntityKind::FallingBlock),
            "falling level dispatch received a stale or foreign handle");
        // This is the one installed configured holder owner, not a registry
        // reconstructed from a compiled fallback on each actor tick.
        let damage_types = players.block_runtime.falling_damage_types()?.clone();
        let physical = players.falling_blocks.physical(handle)
            .context("falling level dispatch has no actual physical owner")?;
        let (min, max) = item_query_chunk_range(physical)?;
        let query_region = players.chunk_runtime.certify_current_query_region(
            world, &mut players.chunk_transports, &self.entity_events,
            game_time, min, max,
        )?;
        // The split edit service cannot mutate the XP or falling stores. Their
        // actual UUID membership therefore stays current while this same-lock
        // view is borrowed, with the current actor's removal checked at drop.
        let external_entity_uuids = players.experience_runtime.physical_states()
            .chain(players.falling_blocks.physical_states())
            .filter(|state| !state.removed)
            .map(|state| (state.entity_id, state.uuid))
            .collect::<Vec<_>>();
        let result = {
            let LivePlayers {
                falling_blocks, item_runtime, gameplay, registry,
                player_motion_runtime, chunk_runtime, world_interactions,
                entity_source, block_runtime, fluid_runtime, random_block_ticks,
                world_revision, world_context, rain_context, ..
            } = &mut *players;
            let entry = falling_blocks.get_mut(handle)
                .context("registered falling level entry has no actual retained object")?;
            let services = ActualFallingBlockEditServices {
                world, items: item_runtime, gameplay, registry,
                player_motion: player_motion_runtime, chunks: chunk_runtime,
                interactions: world_interactions, source: entity_source,
                block_runtime, fluid_runtime,
                random_ticks: random_block_ticks, world_revision,
                context: world_context, rain: rain_context.as_mut(),
                changes: &self.changes, events: &self.entity_events,
                entities: &self.entities, now: game_time, border,
                external_entity_uuids: Some(&external_entity_uuids),
                query_region: Some(query_region),
            };
            // tick_falling_block owns the literal physical begin-level phase;
            // no second begin or detached replacement body is supplied here.
            main_falling_block_physics::tick_falling_block(
                &mut entry.state, &mut entry.publication, services, &damage_types,
            )
        };
        // Source discard already removed real index/tick/tracker ownership.
        // Destroy its Box only after all callbacks using that object returned.
        players.falling_blocks.reap_removed();
        result
    }
}

pub(super) fn publish_falling_tracking(
    runtime: &mut MainFallingBlockRuntime,
    handle: EntityHandle,
    manager: &RuntimeChunkManager,
    registry: &EntityRegistry,
    observers: &BTreeMap<i32, TrackingObserver>,
    transports: &BTreeMap<i32, SessionChunkTransport>,
    events: &broadcast::Sender<PlayerEvent>,
) -> Result<()> {
    let physical = runtime.physical(handle)
        .context("shared falling sender has no actual physical owner")?;
    let section = entity_spatial::section_at(physical.position_vec());
    let (x, _, z) = entity_spatial::section_coordinates(section);
    let in_entity_ticking_range = manager.ticking_range().in_entity_ticking_range((x, z));
    runtime.publish_one(handle, in_entity_ticking_range, registry, observers, transports, events)
}
