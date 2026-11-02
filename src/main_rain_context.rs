//! Main's native shared Item/Player/XP/Falling level tick and rain owner.
//! No imported lighting, cache history or chunk availability is inferred here.
use super::*;
use mc121_rust::rain_physics::RainChunkAvailability;
use super::main_pressure_plate_entities::MainPressurePlateEntities;
use mc121_rust::runtime_entity_index::EntityKind;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

impl RainChunkAvailability for MainChunkRuntime {
    fn require_settled_unedited_fixture(&self) -> Result<()> {
        self.require_supported_lighting()?;
        ensure!(
            !self.manager.has_scheduler_work(),
            "rain queries require chunk scheduler quiescence"
        );
        Ok(())
    }
    fn require_current_full(&self, chunk: (i32, i32)) -> Result<()> {
        self.manager.certify_full_region(chunk, chunk)
    }
}

impl LivePlayers {
    /// The production level entity phase, also exercised by the main tests.
    /// Player callbacks retain their existing bounded native host. Callers with
    /// falling actors must supply their actual world/edit owner below.
    pub(super) fn simulate_item_world(
        &mut self,
        world: &mut WorldBackend,
        game_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        self.simulate_item_world_with_player_menus(world, game_time, border, events, |players, _, id| {
            ensure!(!players.furnaces.has_open(id), "active furnace requires its actual level-player menu owner");
            ensure!(!players.stonecutters.has_open(id), "active stonecutter requires its actual level-player menu owner");
            ensure!(!players.smithing_tables.has_open(id), "active smithing menu requires its actual level-player menu owner");
            ensure!(!players.looms.has_open(id), "active loom requires its actual level-player menu owner");
            ensure!(!players.grindstones.has_open(id), "active grindstone requires its actual level-player menu owner");
            ensure!(!players.anvils.has_open(id), "active anvil requires its actual level-player menu owner");
            Ok(())
        })
    }
    pub(super) fn simulate_item_world_with_player_menus(
        &mut self,
        world: &mut WorldBackend,
        game_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
        menu_tick: impl FnMut(&mut LivePlayers, &mut WorldBackend, i32) -> Result<()>,
    ) -> Result<()> {
        self.simulate_native_entities_with_player_menus(
            world, game_time, border, events, None, menu_tick,
            |_, _, _| bail!("falling level entry requires its actual world/edit owner"),
        )
    }
    /// Both external body callbacks run at their genuine positions in the same
    /// captured EntityTickList pass. A falling actor never enters Item/Player
    /// hooks, and this layer does not run its physical begin_level_tick twice.
    pub(super) fn simulate_native_entities_with_player_menus(
        &mut self,
        world: &mut WorldBackend,
        game_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
        changes: Option<&broadcast::Sender<WorldUpdate>>,
        mut menu_tick: impl FnMut(&mut LivePlayers, &mut WorldBackend, i32) -> Result<()>,
        mut falling_tick: impl FnMut(
            &mut LivePlayers, &mut WorldBackend,
            mc121_rust::runtime_entity_index::EntityHandle,
        ) -> Result<()>,
    ) -> Result<()> {
        if self.gameplay.require_supported_floor().is_err() {
            ensure!(
                self.item_runtime.item_count() == 0 && self.experience_runtime.count() == 0
                    && self.falling_blocks.count() == 0,
                "active native entity bodies require certified gameplay sources"
            );
            return Ok(());
        }
        if self.item_runtime.item_count() != 0 || self.experience_runtime.count() != 0
            || self.falling_blocks.count() != 0 {
            self.require_current_native_source(world)?;
        }
        let ticking_entities = self
            .item_runtime
            .level_tick_entries()
            .any(|(_, kind)| matches!(kind,
                EntityKind::Item | EntityKind::ExperienceOrb | EntityKind::FallingBlock));
        let facts = self.world_context.item_facts(game_time, border, false)?;
        if ticking_entities && matches!(facts.rain, RainAvailability::ExposureUnavailable) {
            ensure!(
                self.rain_context.is_some(),
                "entity rain exposure requires an actual provider"
            );
        }
        // Certify before capturing the pass, as the old complete item phase
        // did. The actual XP owner supplies its physical generation, not an
        // ItemState substitute. Falling dispatch certifies its region at the
        // actual entry, so an earlier actor's world edit cannot stale its token.
        let mut query_regions = self
            .item_runtime
            .ticking_item_states()
            .map(|state| item_query_chunk_range(&state.physical))
            .collect::<Result<Vec<_>>>()?;
        for (handle, kind) in self.item_runtime.level_tick_entries() {
            if kind == EntityKind::ExperienceOrb {
                let physical = self
                    .experience_runtime
                    .physical(handle)
                    .context("ticking XP physical owner missing")?;
                query_regions.push(item_query_chunk_range(physical)?);
            }
        }
        for (min, max) in query_regions {
            // New admission tickets may be queued between the ordinary chunk
            // poll and this locked entity phase. They do not invalidate the
            // current FULL/light/source owners used by the actual bodies.
            self.chunk_runtime.certify_current_query_region(
                world,
                &mut self.chunk_transports,
                events,
                game_time,
                min,
                max,
            )?;
        }
        let colliders = self
            .registry
            .snapshot()
            .into_iter()
            .map(|player| -> Result<_> {
                let dimensions = player_physics::player_dimensions(player.metadata.pose, 1.)?;
                Ok(CollisionEntity {
                    entity_id: player.entity_id,
                    bounds: Aabb::player(
                        Vec3::new(player.position.x, player.position.y, player.position.z),
                        dimensions.width,
                        dimensions.height,
                    ),
                    spectator: player.gamemode == 3,
                    // Ordinary players/items/orbs are not collidable vehicles.
                    can_collide_with_actor: false,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let entries = self.item_runtime.begin_level_tick_pass()?;
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
            for handle in entries {
                let Some(kind) = self.item_runtime.entity_kind(handle) else {
                    continue;
                };
                // Capture external identities from their genuine current
                // owners before the Item/XP body is loaned. The current actor's
                // mutable physical reference overrides its earlier bounds.
                let external = if matches!(kind, EntityKind::Item | EntityKind::ExperienceOrb) {
                    main_redstone::capture_external_trigger_bodies(&self.item_runtime,
                        self.experience_runtime.physical_states(),
                        self.falling_blocks.registered().map(|(_, actor)| actor))?
                } else { Vec::new() };
                let query_region = if matches!(kind, EntityKind::Item | EntityKind::ExperienceOrb) {
                    let physical = if kind == EntityKind::Item {
                        &self.item_runtime.item(handle.id).context("actual Item body is absent")?.physical
                    } else {
                        self.experience_runtime.physical(handle).context("actual XP body is absent")?
                    };
                    let (min, max) = item_query_chunk_range(physical)?;
                    Some(self.chunk_runtime.certify_current_query_region(world,
                        &mut self.chunk_transports, events, game_time, min, max)?)
                } else { None };
                if kind == EntityKind::FallingBlock {
                    falling_tick(self, world, handle)?;
                } else if kind == EntityKind::ExperienceOrb {
                    let Self { experience_runtime, item_runtime, gameplay, registry,
                        player_motion_runtime, chunk_runtime, world_interactions,
                        entity_source, block_runtime, fluid_runtime, redstone_runtime,
                        random_block_ticks, world_revision, world_context, rain_context, .. } = self;
                    let hooks = MainPressurePlateEntities {
                        gameplay, registry, player_motion: player_motion_runtime,
                        chunks: chunk_runtime, interactions: world_interactions,
                        source: entity_source, block_runtime, fluid_runtime,
                        redstone_runtime, random_ticks: random_block_ticks,
                        world_revision, context: world_context, rain: rain_context.as_mut(),
                        changes, events, external, query_region, now: game_time, border,
                    };
                    experience_runtime.tick_one(handle, world, item_runtime, hooks)?;
                } else {
                    ensure!(matches!(kind, EntityKind::Item | EntityKind::Player),
                        "shared level entry has no actual native body dispatcher");
                    // Capture the actual current owner before its menu phase:
                    // invalidity can close it there, but must not cause a second
                    // inactive InventoryMenu broadcast later in this player entry.
                    let external_menu = if kind == EntityKind::Player {
                        let owners = usize::from(self.native_containers.has_open(handle.id))
                            + usize::from(self.crafting_tables.has_open(handle.id))
                            + usize::from(self.furnaces.has_open(handle.id))
                            + usize::from(self.stonecutters.has_open(handle.id))
                            + usize::from(self.smithing_tables.has_open(handle.id))
                            + usize::from(self.looms.has_open(handle.id))
                            + usize::from(self.grindstones.has_open(handle.id))
                            + usize::from(self.anvils.has_open(handle.id));
                        ensure!(owners <= 1, "multiple current external menus during level-player phase");
                        owners != 0
                    } else {
                        false
                    };
                    if kind == EntityKind::Player {
                        // ServerPlayer.tick broadcasts the current menu and
                        // then validates it at this entry's actual position in
                        // the shared native level tick iteration.
                        menu_tick(self, world, handle.id)?;
                    }
                    // One real world owner creates its gameplay/rain/cache
                    // views only for each method, so pressure callbacks may
                    // synchronously mutate the actual source/light/BLOCK owners.
                    let Self { item_runtime, containers, gameplay, registry,
                        player_motion_runtime, chunk_runtime, world_interactions,
                        entity_source, block_runtime, fluid_runtime, redstone_runtime,
                        random_block_ticks, world_revision, world_context, rain_context, .. } = self;
                    let mut hooks = MainPressurePlateEntities {
                        gameplay, registry, player_motion: player_motion_runtime,
                        chunks: chunk_runtime, interactions: world_interactions,
                        source: entity_source, block_runtime, fluid_runtime,
                        redstone_runtime, random_ticks: random_block_ticks,
                        world_revision, context: world_context, rain: rain_context.as_mut(),
                        changes, events, external, query_region, now: game_time, border,
                    };
                    let mut host = RuntimeInventoryHost::new(registry, containers, &mut hooks)
                        .with_world_callbacks(main_pressure_plate_entities::world_callbacks());
                    if external_menu { host = host.with_external_player_menu(handle.id); }
                    item_runtime.simulate_level_entry_with_context(
                        handle, world, facts, &colliders, None, None, &mut host,
                    )?;
                }
            }
            Ok(())
        }));
        // Like EntityTickList.forEach's finally block, unwind and callback
        // errors both release the marker after all completed mutations remain.
        self.item_runtime.finish_level_tick_pass();
        match result {
            Ok(result) => result?,
            Err(panic) => resume_unwind(panic),
        }
        self.gameplay.drain_observations();
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tools/main_rain_context_tests.rs"]
mod tests;
