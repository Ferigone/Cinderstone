//! Actual retained GrindstoneMenu lifetime and context-aware removal.
//! The primary transaction owns cursor/input returns, real drops and mixed XP
//! publication. No validity observation here constructs a substitute Host.
use super::*;
use super::main_grindstone::GrindstoneRequest;
use mc121_rust::{
    container::MenuCloseContext,
    workstations::menu::{WorkstationAccess, WorkstationKind},
};

/// A connected operation has an actual connection caller. Its Player.isAlive
/// branch still comes from canonical health/removal owners, not reach, game
/// mode, an admission flag or mere presence in the player registry. Explicit
/// EOF/removal callers instead supply DeadOrDisconnected after InventoryMenu.
pub(super) fn connected_grindstone_close_context(
    players: &LivePlayers, id: i32,
) -> Result<MenuCloseContext> {
    let player = players.registry.get(id).context("grindstone close player owner missing")?;
    let health = match player.entity_data.get(9) {
        Some(MetadataValue::Float(health)) if health.is_finite() => *health,
        _ => bail!("grindstone close actual health owner unavailable"),
    };
    let physical = players.player_motion_runtime.physical(id)
        .context("grindstone close actual entity lifetime missing")?;
    ensure!(physical.entity_id == id && physical.uuid == player.profile.uuid,
        "grindstone close actual lifetime identity changed");
    Ok(if health > 0. && !physical.removed {
        MenuCloseContext::AliveConnected
    } else {
        MenuCloseContext::DeadOrDisconnected
    })
}

impl Server {
    pub(super) async fn close_native_grindstone(
        &self, id: i32, window: u8, send_close: bool,
    ) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        if players.grindstones.window_id(id) != Some(window) { return Ok(false); }
        let context = connected_grindstone_close_context(&players, id)?;
        self.close_grindstone_with_context_locked(
            &mut world, &mut players, id, window, send_close, now, border, context,
        )
    }

    /// Caller retains chat -> world -> players -> shared-container ownership.
    /// Current-menu selection is atomic in Main; a packet's supplied window
    /// does not authorize a stale menu. No outer lock is reacquired here.
    pub(super) fn close_grindstone_with_context_locked(
        &self, world: &mut WorldBackend, players: &mut LivePlayers,
        id: i32, window: u8, send_close: bool, now: i64, border: CollisionBorder,
        context: MenuCloseContext,
    ) -> Result<bool> {
        if players.grindstones.window_id(id) != Some(window) { return Ok(false); }
        self.grindstone_action_locked(world, players, id,
            GrindstoneRequest::Close { window, send_close, context }, now, border)?;
        Ok(true)
    }

    pub(super) fn tick_native_grindstone_menu_for_player_locked(
        &self, world: &mut WorldBackend, players: &mut LivePlayers,
        id: i32, now: i64, border: CollisionBorder,
    ) -> Result<()> {
        if !players.grindstones.has_open(id) { return Ok(()); }
        if let Err(error) = self.grindstone_action_locked(
            world, players, id, GrindstoneRequest::Broadcast, now, border,
        ) {
            if error.downcast_ref::<super::main_grindstone_dispatch::UnsupportedMenuReach>().is_none() {
                return Err(error);
            }
            let window = players.grindstones.window_id(id)
                .context("unsupported grindstone profile lost its actual menu")?;
            let context = connected_grindstone_close_context(players, id)?;
            ensure!(self.close_grindstone_with_context_locked(
                world, players, id, window, true, now, border, context,
            )?, "unsupported grindstone profile changed its captured menu");
            eprintln!("{}", json!({"event":"native_menu_profile_retired", "menu":"grindstone",
                "entity_id":id, "native_limit":"unsupported_block_interaction_range",
                "reason":error.to_string()}));
            return Ok(());
        }
        let Some(open) = players.grindstones.menus.get(&id) else { return Ok(()); };
        let position = open.position;
        let window = open.menu.window_id();
        let player = players.registry.get(id).context("orphan grindstone menu owner")?.clone();
        // Broadcast has just kernel-validated this actual retained binding and
        // registry under the same nonreentrant operation. The subsequent source
        // stillValid reads the real captured block, then this fresh player reach.
        let access = super::main_grindstone_dispatch::grindstone_access(world, &player, position)?;
        if !(access.block_matches(WorkstationKind::Grindstone)?
            && access.player_can_interact_with_block(4.)?) {
            let context = connected_grindstone_close_context(players, id)?;
            self.close_grindstone_with_context_locked(
                world, players, id, window, true, now, border, context,
            )?;
        }
        Ok(())
    }

    /// Explicit connected native shutdown checkpoint only. Autosave retains
    /// menus and snapshots their separate cursor. Dead/disconnected cleanup
    /// must first remove InventoryMenu and call the context-aware helper.
    pub(super) fn close_grindstones_for_save_locked(
        &self, world: &mut WorldBackend, players: &mut LivePlayers, now: i64,
    ) -> Result<()> {
        if !self.shutting_down.load(Ordering::Acquire) { return Ok(()); }
        let ids = players.registry.snapshot().into_iter().map(|player| player.entity_id)
            .filter(|id| players.grindstones.has_open(*id)).collect::<Vec<_>>();
        ensure!(ids.len() == players.grindstones.pending_players().len(),
            "shutdown found an orphan grindstone menu owner");
        // Prove every connected return branch before publishing any checkpoint
        // close. A dead owner is not dropped here ahead of its InventoryMenu.
        for &id in &ids {
            ensure!(connected_grindstone_close_context(players, id)? == MenuCloseContext::AliveConnected,
                "grindstone shutdown retirement requires an actual alive connected owner");
        }
        let border = self.native_container_border()?;
        for id in ids {
            let window = players.grindstones.window_id(id).context("grindstone shutdown owner disappeared")?;
            ensure!(self.close_grindstone_with_context_locked(
                world, players, id, window, true, now, border, MenuCloseContext::AliveConnected,
            )?, "grindstone shutdown changed its actual menu");
        }
        Ok(())
    }
}
