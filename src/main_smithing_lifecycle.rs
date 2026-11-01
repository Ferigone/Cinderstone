//! Actual SmithingMenu lifetime under the shared operation and menu owners.
//! Three inputs and the cursor are owned; the result is a derived preview.
//! Their returns/drops belong to main_smithing's ordered transaction, not to a
//! block entity, a second inventory, or a synthetic removal callback here.
use super::*;
use super::main_smithing::SmithingRequest;
use mc121_rust::container::MenuCloseContext;

impl Server {
    /// Close the captured current server menu. A client close packet's window
    /// ID does not choose this owner: Main resolves its actual current window.
    pub(super) async fn close_native_smithing_table(
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
        self.close_smithing_with_context_locked(
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

    /// Existing operations retain chat/world/player ownership across this call.
    /// Actual disconnect/removal supplies DeadOrDisconnected after removing the
    /// retained InventoryMenu, preserving cursor then input0/1/2 drop ordering.
    pub(super) fn close_smithing_with_context_locked(
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
        let Some(actual_window) = players.smithing_tables.window_id(id) else {
            return Ok(false);
        };
        if actual_window != window {
            return Ok(false);
        }
        self.smithing_action_locked(
            world,
            players,
            id,
            SmithingRequest::Close {
                window,
                send_close,
                context,
            },
            now,
            border,
        )?;
        Ok(true)
    }

    /// Run once in this actual player's ServerPlayer.tick menu phase. Source
    /// order is broadcastChanges, then stillValid, then server-side close.
    pub(super) fn tick_native_smithing_menu_for_player_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        if !players.smithing_tables.has_open(id) {
            return Ok(());
        }
        if let Err(error) = self.smithing_action_locked(
            world,
            players,
            id,
            SmithingRequest::Broadcast,
            now,
            border,
        ) {
            if error.downcast_ref::<super::main_smithing_dispatch::UnsupportedMenuReach>().is_none() {
                return Err(error);
            }
            let window = players.smithing_tables.window_id(id)
                .context("unsupported smithing profile lost its actual menu")?;
            ensure!(self.close_smithing_with_context_locked(
                world, players, id, window, true, now, border,
                MenuCloseContext::AliveConnected,
            )?, "unsupported smithing profile changed its captured menu");
            eprintln!("{}", json!({
                "event":"native_menu_profile_retired", "menu":"smithing",
                "entity_id":id, "native_limit":"unsupported_block_interaction_range",
                "reason":error.to_string(),
            }));
            return Ok(());
        }
        let Some(open) = players.smithing_tables.menus.get(&id) else {
            return Ok(());
        };
        let position = open.position;
        let window = open.menu.sync().window_id();
        let player = players
            .registry
            .get(id)
            .context("orphan smithing menu owner")?
            .clone();
        let access = super::main_smithing_dispatch::smithing_access(world, &player, position)?;
        if !open.menu.still_valid(&access)? {
            self.close_smithing_with_context_locked(
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

    /// Retire actual external menus before native checkpoint snapshots. This
    /// uses the existing connected-player checkpoint policy, not a fabricated
    /// Java periodic-save behavior. Disconnect/shutdown actor removal instead
    /// uses the context-aware per-player method with its actual lifecycle fact.
    pub(super) fn close_smithing_tables_for_save_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        now: i64,
    ) -> Result<()> {
        let border = self.native_container_border()?;
        for id in players.smithing_tables.pending_players() {
            let window = players
                .smithing_tables
                .window_id(id)
                .context("smithing save owner disappeared")?;
            self.close_smithing_with_context_locked(
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
