//! Actual StonecutterMenu lifetime under world -> players -> shared-menu locks.
//!
//! The authoritative action owner performs inventory/drop/RNG transactions and
//! ordered callback publication. This module never owns an inventory copy,
//! result-preview item, block entity, health flag or replacement world.
use super::*;
use super::main_stonecutter::StonecutterRequest;
use mc121_rust::container::MenuCloseContext;

impl Server {
    /// Close the actual current stonecutter from a connected-player operation.
    /// `window` identifies the captured server menu, not an untrusted close
    /// packet ID: vanilla's close handler closes its actual current menu.
    pub(super) async fn close_native_stonecutter(
        &self,
        id: i32,
        window: u8,
        send_close: bool,
    ) -> Result<bool> {
        let _chat_sequence = self.chat_dispatch.lock().await;
        let now = self
            .clock
            .read()
            .map_err(|_| anyhow!("clock poisoned"))?
            .game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("players poisoned"))?;
        self.close_stonecutter_with_context_locked(
            &mut world,
            &mut players,
            id,
            window,
            send_close,
            now,
            border,
            MenuCloseContext::AliveConnected,
        )
    }

    /// Compose with an existing world/player operation without acquiring those
    /// locks again. Disconnect/removal explicitly passes DeadOrDisconnected;
    /// it must not use the connected-player inventory-return branch.
    pub(super) fn close_stonecutter_with_context_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        window: u8,
        send_close: bool,
        now: i64,
        border: CollisionBorder,
        context: MenuCloseContext,
    ) -> Result<bool> {
        let Some(actual_window) = players.stonecutters.window_id(id) else {
            return Ok(false);
        };
        if actual_window != window {
            return Ok(false);
        }
        self.stonecutter_action_locked(
            world,
            players,
            id,
            StonecutterRequest::Close {
                window,
                send_close,
                context,
            },
            now,
            border,
        )?;
        Ok(true)
    }

    /// ServerPlayer.tick broadcasts the current menu before testing stillValid.
    /// Call once in that player's actual menu phase, not as a second tick of a
    /// block entity: stonecutters have no inventory-owning block entity.
    pub(super) fn tick_native_stonecutter_menu_for_player_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        if !players.stonecutters.has_open(id) {
            return Ok(());
        }
        // Broadcast uses the canonical inventory and the actual source-ordered
        // SyncEvent/listener owner, including remote-state bookkeeping.
        if let Err(error) = self.stonecutter_action_locked(
            world,
            players,
            id,
            StonecutterRequest::Broadcast,
            now,
            border,
        ) {
            if error.downcast_ref::<super::main_stonecutter_dispatch::UnsupportedMenuReach>().is_none() {
                return Err(error);
            }
            // A legal inventory/selected-slot change can leave this bounded
            // attribute profile while its menu is open. Retire that real menu
            // through its actual return/drop owner instead of aborting the
            // entire level tick or fabricating a custom interaction range.
            let window = players.stonecutters.window_id(id)
                .context("unsupported stonecutter profile lost its actual menu")?;
            ensure!(self.close_stonecutter_with_context_locked(
                world, players, id, window, true, now, border,
                MenuCloseContext::AliveConnected,
            )?, "unsupported stonecutter profile changed its captured menu");
            eprintln!("{}", json!({
                "event":"native_menu_profile_retired", "menu":"stonecutter",
                "entity_id":id, "native_limit":"unsupported_block_interaction_range",
                "reason":error.to_string(),
            }));
            return Ok(());
        }
        let Some(open) = players.stonecutters.menus.get(&id) else {
            return Ok(());
        };
        let position = open.position;
        let window = open.menu.window_id();
        let player = players
            .registry
            .get(id)
            .context("orphan stonecutter menu owner")?
            .clone();
        let access = super::main_stonecutter_dispatch::stonecutter_access(world, &player, position)?;
        if !open.menu.still_valid(&access)? {
            self.close_stonecutter_with_context_locked(
                world,
                players,
                id,
                window,
                true,
                now,
                border,
                MenuCloseContext::AliveConnected,
            )?;
        }
        Ok(())
    }

    /// Whole-phase adapter for hosts that have not already called the per-player
    /// method. Main must choose one call path, preserving actual player order.
    pub(super) async fn tick_native_stonecutters(&self) -> Result<()> {
        let _chat_sequence = self.chat_dispatch.lock().await;
        let now = self
            .clock
            .read()
            .map_err(|_| anyhow!("clock poisoned"))?
            .game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("players poisoned"))?;
        for id in players.stonecutters.pending_players() {
            self.tick_native_stonecutter_menu_for_player_locked(
                &mut world,
                &mut players,
                id,
                now,
                border,
            )?;
        }
        Ok(())
    }

    /// Native checkpointing first retires external menus through their real
    /// connected-player removal path, then snapshots the shared inventory.
    /// Owned cursor/input stacks cannot disappear in an unpersisted menu.
    pub(super) fn close_stonecutters_for_save_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        now: i64,
    ) -> Result<()> {
        let border = self.native_container_border()?;
        for id in players.stonecutters.pending_players() {
            let window = players
                .stonecutters
                .window_id(id)
                .context("stonecutter save owner disappeared")?;
            self.close_stonecutter_with_context_locked(
                world,
                players,
                id,
                window,
                true,
                now,
                border,
                MenuCloseContext::AliveConnected,
            )?;
        }
        Ok(())
    }
}
