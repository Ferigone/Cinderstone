//! Authoritative retained-menu recovery after an unsupported native candidate.
//!
//! The rejected prediction is never installed or executed. Selection and the
//! full-state operation share the normal chat/world/player ownership interval;
//! window 0 is used only when no external menu owns the current screen.
use super::*;

impl Server {
    pub(super) async fn resync_current_player_menu(&self, id: i32) -> Result<()> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        if self.shutting_down.load(Ordering::Acquire) || players.registry.get(id).is_none() {
            return Ok(());
        }
        players.require_current_native_source(&world)?;
        players.gameplay.require_supported_floor()?;
        let shared = players.containers.get(&id)
            .context("resynchronization canonical inventory missing")?.clone();
        shared.read(|menu| {
            main_workstation_components::require_supported_inventory(&menu.inventory)
        })??;

        enum CurrentMenu {
            Furnace,
            Stonecutter,
            Smithing,
            Loom,
            CraftingTable,
            NativeContainer,
        }
        let mut owners = [
            players.furnaces.window_id(id).map(|_| CurrentMenu::Furnace),
            players.stonecutters.window_id(id).map(|_| CurrentMenu::Stonecutter),
            players.smithing_tables.window_id(id).map(|_| CurrentMenu::Smithing),
            players.looms.window_id(id).map(|_| CurrentMenu::Loom),
            players.crafting_tables.window_id(id).map(|_| CurrentMenu::CraftingTable),
            players.native_containers.window_id(id).map(|_| CurrentMenu::NativeContainer),
        ].into_iter().flatten();
        let selected = owners.next();
        ensure!(owners.next().is_none(), "multiple current player menus during resynchronization");

        // Every branch targets the actual retained owner. No synthetic click,
        // initial-menu replacement or inactive InventoryMenu packet is used.
        match selected {
            Some(CurrentMenu::Furnace) => self.resync_native_furnace_locked(
                &mut world, &mut players, id, now, border,
            ),
            Some(CurrentMenu::Stonecutter) => self.resync_native_stonecutter_locked(
                &mut world, &mut players, id, now, border,
            ),
            Some(CurrentMenu::Loom) => self.resync_native_loom_locked(
                &mut world, &mut players, id, now, border,
            ),
            Some(CurrentMenu::Smithing) => self.resync_native_smithing_locked(
                &mut world, &mut players, id, now, border,
            ),
            Some(CurrentMenu::CraftingTable) => self.resync_native_crafting_table_locked(
                &mut world, &mut players, id, now, border,
            ),
            Some(CurrentMenu::NativeContainer) => self.resync_native_container_locked(
                &mut world, &mut players, id, now, border,
            ),
            None => {
                let uuid = players.registry.get(id)
                    .context("resynchronization actor disappeared")?.profile.uuid;
                let next_stats = players.gameplay.snapshot_player_stats(uuid)?;
                let gameplay = &mut players.gameplay;
                // The existing SharedContainer publisher places this packet in
                // the same PlayerEvent stream as every ordinary menu update.
                // Its rollback retains the genuine baselines on any fallible
                // preflight; the prepared actual callback token commits before
                // that publisher can observe the full-state packet.
                shared.transact_with_commit(
                    move |menu| {
                        main_workstation_components::require_supported_inventory(&menu.inventory)?;
                        let packet = menu.broadcast_full_state()?;
                        let changes = menu.take_local_slot_changes();
                        let prepared = gameplay.prepare_menu_commit(
                            uuid, &menu.inventory, changes, next_stats,
                        )?;
                        Ok((prepared, vec![packet]))
                    },
                    |prepared| prepared.commit(),
                )
            }
        }
    }
}

