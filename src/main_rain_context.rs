//! Main's native shared Item/Player/XP level tick and persistent rain owner.
//! No imported lighting, cache history or chunk availability is inferred here.
use super::*;
use mc121_rust::native_rain::NativeFlatPlainsRainFrame;
use mc121_rust::rain_physics::{RainChunkAvailability, RainExposure};
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
    /// Player callbacks retain their existing bounded native host; XP and item
    /// bodies execute at their actual positions in the same EntityTickList pass.
    pub(super) fn simulate_item_world(
        &mut self,
        world: &mut WorldBackend,
        game_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        self.simulate_item_world_with_player_menus(world, game_time, border, events, |players, _, id| {
            ensure!(!players.furnaces.has_open(id), "active furnace requires its actual level-player menu owner");
            Ok(())
        })
    }
    pub(super) fn simulate_item_world_with_player_menus(
        &mut self,
        world: &mut WorldBackend,
        game_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
        mut menu_tick: impl FnMut(&mut LivePlayers, &mut WorldBackend, i32) -> Result<()>,
    ) -> Result<()> {
        if self.gameplay.require_supported_floor().is_err() {
            ensure!(
                self.item_runtime.item_count() == 0 && self.experience_runtime.count() == 0,
                "active items/XP require certified gameplay sources"
            );
            return Ok(());
        }
        if self.item_runtime.item_count() != 0 || self.experience_runtime.count() != 0 {
            self.require_current_native_source(world)?;
        }
        let ticking_entities = self
            .item_runtime
            .level_tick_entries()
            .any(|(_, kind)| matches!(kind, EntityKind::Item | EntityKind::ExperienceOrb));
        let facts = self.world_context.item_facts(game_time, border, false)?;
        if ticking_entities && matches!(facts.rain, RainAvailability::ExposureUnavailable) {
            ensure!(
                self.rain_context.is_some(),
                "entity rain exposure requires an actual provider"
            );
        }
        // Certify before capturing the pass, as the old complete item phase
        // did. The actual XP owner supplies its physical generation, not a
        // fabricated ItemState; only current tick-list entries need this query.
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
                if kind == EntityKind::ExperienceOrb {
                    let Self {
                        experience_runtime,
                        item_runtime,
                        gameplay,
                        registry,
                        chunk_runtime,
                        world_interactions,
                        rain_context,
                        world_context,
                        ..
                    } = self;
                    experience_runtime.tick_one(
                        handle,
                        world,
                        item_runtime,
                        gameplay,
                        registry,
                        chunk_runtime,
                        &mut world_interactions.fluid_shapes,
                        rain_context.as_mut(),
                        world_context,
                        game_time,
                        border,
                        events,
                    )?;
                } else {
                    let external_menu = kind == EntityKind::Player && self.furnaces.has_open(handle.id);
                    if kind == EntityKind::Player {
                        // ServerPlayer.tick broadcasts the current menu and
                        // then validates it at this entry's actual position in
                        // the shared Item/Player/XP level tick iteration.
                        menu_tick(self, world, handle.id)?;
                    }
                    // Release these item-host borrows before the next XP body
                    // mutates the same section index or actual world owners.
                    let Self {
                        item_runtime,
                        chunk_runtime,
                        registry,
                        containers,
                        gameplay,
                        rain_context,
                        world_interactions,
                        ..
                    } = self;
                    let (light, manager) = chunk_runtime.native_lighting_parts()?;
                    let mut exposure = rain_context
                        .as_mut()
                        .map(|owner| NativeFlatPlainsRainFrame::new(owner, light, manager));
                    let rain = exposure
                        .as_mut()
                        .map(|frame| frame as &mut dyn RainExposure);
                    let mut frame = gameplay.frame(registry, manager, true);
                    let mut host = RuntimeInventoryHost::new(registry, containers, &mut frame);
                    if external_menu { host = host.with_external_player_menu(handle.id); }
                    item_runtime.simulate_level_entry_with_context(
                        handle,
                        world,
                        facts,
                        &colliders,
                        Some(&mut world_interactions.fluid_shapes),
                        rain,
                        &mut host,
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
