//! Main's persistent fresh-fixture rain owner and actual item tick composition.
//! No imported lighting, cache history or chunk availability is inferred here.
use super::*;
use mc121_rust::rain_physics::{RainChunkAvailability, RainExposure};
use mc121_rust::native_rain::NativeFlatPlainsRainFrame;

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
    /// Player callbacks remain the explicitly bounded native item slice.
    pub(super) fn simulate_item_world(
        &mut self,
        world: &mut WorldBackend,
        game_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        if self.gameplay.require_supported_floor().is_err() {
            ensure!(
                self.item_runtime.item_count() == 0,
                "active items require certified gameplay sources"
            );
            return Ok(());
        }
        if self.item_runtime.item_count() != 0 {
            self.require_current_native_source(world)?;
        }
        let ticking_items = self.item_runtime.ticking_item_states().next().is_some();
        let facts = self.world_context.item_facts(game_time, border, false)?;
        if ticking_items && matches!(facts.rain, RainAvailability::ExposureUnavailable) {
            ensure!(
                self.rain_context.is_some(),
                "item rain exposure requires an actual provider"
            );
        }
        let Self {
            item_runtime,
            chunk_runtime,
            chunk_transports,
            registry,
            containers,
            gameplay,
            rain_context,
            world_interactions,
            ..
        } = self;
        // Certify scheduled items even after a stale-TICKING promotion. Rain
        // rechecks its point-specific FULL and persistent source facts as well.
        for state in item_runtime.ticking_item_states() {
            let (min, max) = item_query_chunk_range(&state.physical)?;
            chunk_runtime.certify_query_region(
                world,
                chunk_transports,
                events,
                game_time,
                min,
                max,
            )?;
        }
        let colliders = registry
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
                    // Ordinary Player/Item bodies are not collidable. This
                    // source contains no vehicles or other entity kinds.
                    can_collide_with_actor: false,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let (light, manager) = chunk_runtime.native_lighting_parts()?;
        let mut exposure = rain_context.as_mut().map(|owner| NativeFlatPlainsRainFrame::new(owner, light, manager));
        let rain = exposure
            .as_mut()
            .map(|frame| frame as &mut dyn RainExposure);
        let mut frame = gameplay.frame(registry, manager, true);
        let mut host = RuntimeInventoryHost::new(registry, containers, &mut frame);
        item_runtime
            .simulate_level_with_context(world, facts, &colliders, Some(&mut world_interactions.fluid_shapes), rain, &mut host)?;
        gameplay.drain_observations();
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tools/main_rain_context_tests.rs"]
mod tests;
