//! Furnace setChanged's analog-output callback in the supported native world.
//! Comparators retain an explicit boundary until their actual receiver exists.
use super::*;

impl Server {
    pub(super) fn notify_native_furnace_changed_locked(
        &self, world: &mut WorldBackend, players: &mut LivePlayers,
        position: (i32, i32, i32),
    ) -> Result<()> {
        self.notify_native_container_changed_locked(world, players, position)
    }
    pub(super) fn notify_native_container_changed_locked(
        &self, world: &mut WorldBackend, players: &mut LivePlayers,
        position: (i32, i32, i32),
    ) -> Result<()> {
        // Level.updateNeighbourForOutputSignal: HORIZONTAL encounter order,
        // hasChunkAt only for the first neighbor, then one conductor relay.
        for (dx, dz) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let near = (position.0 + dx, position.1, position.2 + dz);
            if !players.chunk_runtime.manager.current_full_ready((near.0.div_euclid(16), near.2.div_euclid(16))) {
                continue;
            }
            let state = world.block(near.0, near.1, near.2)?;
            ensure!(inventory::block_state_name(state) != Some("comparator"),
                "container output notification requires the actual comparator receiver");
            if mc121_rust::native_block_entities::static_conductor(state)? {
                let far = (near.0 + dx, near.1, near.2 + dz);
                players.chunk_runtime.manager.certify_full_region(
                    (far.0.div_euclid(16), far.2.div_euclid(16)),
                    (far.0.div_euclid(16), far.2.div_euclid(16)),
                )?;
                ensure!(inventory::block_state_name(world.block(far.0, far.1, far.2)?) != Some("comparator"),
                    "container conductor notification requires the actual comparator receiver");
            }
        }
        // No observable receiver remains. The owned inventory/timer mutation
        // itself is recorded separately in NativeEntitySource's world image.
        Ok(())
    }
}
