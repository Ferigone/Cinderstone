//! Actual LoomMenu lifetime under the existing operation/world/player owners.
//! Banner, dye, template and cursor belong to the retained native menu. Their
//! ordered returns/drops are performed only by the real primary transaction.
use super::*;
use super::main_loom::LoomRequest;
use mc121_rust::container::MenuCloseContext;

impl Server {
    /// Standalone connected-player operation. Main's atomic current-menu close
    /// selects its actual owner while locked and uses the locked helper below.
    pub(super) async fn close_native_loom(
        &self,
        id: i32,
        window: u8,
        send_close: bool,
    ) -> Result<bool> {
        let _chat_sequence = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        self.close_loom_with_context_locked(
            &mut world, &mut players, id, window, send_close, now, border,
            MenuCloseContext::AliveConnected,
        )
    }

    /// The caller retains chat -> world -> players -> canonical-container order.
    /// Dead/EOF actor cleanup supplies its actual disconnected context after
    /// removing the retained InventoryMenu. No lock is reacquired here.
    pub(super) fn close_loom_with_context_locked(
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
        let Some(actual_window) = players.looms.window_id(id) else { return Ok(false); };
        if actual_window != window { return Ok(false); }
        self.loom_action_locked(world, players, id,
            LoomRequest::Close { window, send_close, context }, now, border)?;
        Ok(true)
    }

    /// Exactly one real per-player menu phase: broadcast changes first, then
    /// inspect the captured block with this player's fresh access and close.
    pub(super) fn tick_native_loom_menu_for_player_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        if !players.looms.has_open(id) { return Ok(()); }
        if let Err(error) = self.loom_action_locked(
            world, players, id, LoomRequest::Broadcast, now, border,
        ) {
            if error.downcast_ref::<super::main_loom_dispatch::UnsupportedMenuReach>().is_none() {
                return Err(error);
            }
            let window = players.looms.window_id(id)
                .context("unsupported loom profile lost its actual menu")?;
            ensure!(self.close_loom_with_context_locked(
                world, players, id, window, true, now, border,
                MenuCloseContext::AliveConnected,
            )?, "unsupported loom profile changed its captured menu");
            eprintln!("{}", json!({
                "event":"native_menu_profile_retired", "menu":"loom", "entity_id":id,
                "native_limit":"unsupported_block_interaction_range", "reason":error.to_string(),
            }));
            return Ok(());
        }
        let Some(open) = players.looms.menus.get(&id) else { return Ok(()); };
        let window = open.menu.window_id();
        let position = open.position;
        let player = players.registry.get(id).context("orphan loom menu owner")?.clone();
        // The retained pickup context is not a fresh block/range observation.
        // still_valid short-circuits the captured block before evaluating reach.
        let access = super::main_loom_dispatch::loom_access(world, &player, position)?;
        if !open.menu.still_valid(&access)? {
            self.close_loom_with_context_locked(
                world, players, id, window, true, now, border,
                MenuCloseContext::AliveConnected,
            )?;
        }
        Ok(())
    }

    /// Explicit native checkpoint retirement of still-connected menus. Main
    /// may use its existing shutdown checkpoint policy; ordinary autosave must
    /// retain current menus and use their separate-cursor inventory snapshots.
    /// Actual disconnected actor removal instead calls the context-aware
    /// helper after InventoryMenu removal, not this connected return policy.
    pub(super) fn close_looms_for_save_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        now: i64,
    ) -> Result<()> {
        let border = self.native_container_border()?;
        for id in players.looms.pending_players() {
            let window = players.looms.window_id(id).context("loom checkpoint owner disappeared")?;
            ensure!(self.close_loom_with_context_locked(
                world, players, id, window, true, now, border,
                MenuCloseContext::AliveConnected,
            )?, "loom checkpoint changed its actual menu");
        }
        Ok(())
    }
}
