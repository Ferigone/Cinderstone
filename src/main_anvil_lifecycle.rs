//! Actual AnvilMenu lifetime under world -> players -> shared-inventory locks.
//!
//! The runtime action owner stages the actual cursor/inputs, inventory, RNG,
//! drops and ordered callbacks together. This layer never clones a menu, moves
//! a preview, clears retained window0 state or manufactures a player/world.
use super::main_anvil::AnvilRequest;
use super::*;
use mc121_rust::container::MenuCloseContext;

/// Connected operations still use actual Player.isAlive semantics. Explicit
/// disconnect/removal callers pass DeadOrDisconnected to the locked close hook
/// instead of deriving connection state from a still-registered player record.
pub(super) fn connected_anvil_close_context(players: &LivePlayers, id: i32) -> Result<MenuCloseContext> {
    let player = players
        .registry
        .get(id)
        .context("anvil close player owner missing")?;
    let health = match player.entity_data.get(9) {
        Some(MetadataValue::Float(value)) if value.is_finite() => *value,
        _ => bail!("anvil close actual health owner unavailable"),
    };
    let physical = players
        .player_motion_runtime
        .physical(id)
        .context("anvil close actual entity lifetime owner missing")?;
    ensure!(
        physical.entity_id == id && physical.uuid == player.profile.uuid,
        "anvil close actual entity lifetime identity changed"
    );
    Ok(if health > 0. && !physical.removed {
        MenuCloseContext::AliveConnected
    } else {
        MenuCloseContext::DeadOrDisconnected
    })
}

impl Server {
    /// Connected-player close. The window comes from the actual retained menu;
    /// vanilla's close handler does not select an owner by an untrusted packet
    /// ID. Dead connected players take the actual drop branch on removal.
    pub(super) async fn close_native_anvil(
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
        if players.anvils.window_id(id) != Some(window) {
            return Ok(false);
        }
        let context = connected_anvil_close_context(&players, id)?;
        self.close_anvil_with_context_locked(
            &mut world,
            &mut players,
            id,
            window,
            send_close,
            now,
            border,
            context,
        )
    }

    /// Compose with an already-owned world/player operation without acquiring
    /// those locks again. Disconnect/removal explicitly supplies the dead or
    /// disconnected context BEFORE deleting any real player/inventory/RNG owner.
    /// The action owner publishes close-window before cursor then input0/input1
    /// removal, discards preview, and transfers inventory baselines afterward.
    pub(super) fn close_anvil_with_context_locked(
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
        let Some(actual_window) = players.anvils.window_id(id) else {
            return Ok(false);
        };
        if actual_window != window {
            return Ok(false);
        }
        self.anvil_action_locked(
            world,
            players,
            id,
            AnvilRequest::Close {
                window,
                send_close,
                context,
            },
            now,
            border,
        )?;
        Ok(true)
    }

    /// Call exactly once in this actual player's ServerPlayer.tick menu phase.
    /// Source broadcast precedes stillValid and resulting close. The captured
    /// position and actual player are borrowed; there is no substitute host or
    /// second read/write inventory and no menu clone for this validity check.
    pub(super) fn tick_native_anvil_menu_for_player_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        if !players.anvils.has_open(id) {
            return Ok(());
        }
        if let Err(error) =
            self.anvil_action_locked(world, players, id, AnvilRequest::Broadcast, now, border)
        {
            if error
                .downcast_ref::<super::main_stonecutter_dispatch::UnsupportedMenuReach>()
                .is_none()
            {
                return Err(error);
            }
            // A real equipped modifier can move this actor out of the admitted
            // reach profile. Retire through the source return/drop owner;
            // neither guess the attribute nor reset the current menu.
            let window = players
                .anvils
                .window_id(id)
                .context("unsupported anvil profile lost its actual menu")?;
            let context = connected_anvil_close_context(players, id)?;
            ensure!(
                self.close_anvil_with_context_locked(
                    world, players, id, window, true, now, border, context,
                )?,
                "unsupported anvil profile changed its captured menu"
            );
            eprintln!(
                "{}",
                json!({
                    "event":"native_menu_profile_retired", "menu":"anvil",
                    "entity_id":id, "native_limit":"unsupported_block_interaction_range",
                    "reason":error.to_string(),
                })
            );
            return Ok(());
        }
        // This is the actual supported-actor admission predicate, separate
        // from executing a candidate kernel/world action. A later game mode,
        // effect, mount or body change must retire this bounded UI even when
        // the captured anvil/reach predicate would still be true. Broadcast
        // above remains first, and unrelated action errors are never caught.
        if let Err(profile) = super::main_anvil_dispatch::require_anvil_actor(players, id) {
            let window = players
                .anvils
                .window_id(id)
                .context("unsupported anvil actor lost its actual menu")?;
            let context = connected_anvil_close_context(players, id)?;
            ensure!(
                self.close_anvil_with_context_locked(
                    world, players, id, window, true, now, border, context,
                )?,
                "unsupported anvil actor changed its captured menu"
            );
            eprintln!(
                "{}",
                json!({
                    "event":"native_menu_profile_retired", "menu":"anvil", "entity_id":id,
                    "native_limit":"unsupported_actor_profile", "reason":profile.to_string(),
                })
            );
            return Ok(());
        }
        let Some(open) = players.anvils.menus.get(&id) else {
            return Ok(());
        };
        let position = open.position;
        let window = open.menu.window_id();
        let player = players
            .registry
            .get(id)
            .context("orphan anvil menu owner")?;
        let valid = super::main_anvil_dispatch::anvil_still_valid(world, player, position)?;
        if !valid {
            let context = connected_anvil_close_context(players, id)?;
            ensure!(
                self.close_anvil_with_context_locked(
                    world, players, id, window, true, now, border, context,
                )?,
                "invalid anvil changed its captured menu before close"
            );
        }
        Ok(())
    }

    /// Native shutdown retirement, not ordinary autosave. The actual save
    /// caller already owns chat/save/world/player sequencing. Runtime's detached
    /// inventory_snapshot captures an active cursor during live autosave while
    /// this method deliberately leaves the actual menu/input/cursor owners live.
    /// Shutdown first returns/drops those owners, then Root captures player data.
    pub(super) fn close_anvils_for_save_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        now: i64,
    ) -> Result<()> {
        if !self.shutting_down.load(Ordering::Acquire) {
            return Ok(());
        }
        let border = self.native_container_border()?;
        // Preserve the actual player registry's save order. A separate sorted
        // UI map must not define real drop/RNG/entity-allocation ordering.
        let ids = players
            .registry
            .snapshot()
            .into_iter()
            .map(|player| player.entity_id)
            .filter(|id| players.anvils.has_open(*id))
            .collect::<Vec<_>>();
        ensure!(
            ids.len() == players.anvils.pending_players().len(),
            "shutdown found an orphan anvil menu owner"
        );
        // This native shutdown checkpoint is an explicitly connected-alive
        // retirement policy. Validate every owner before publishing any close;
        // Dead/EOF removal retains its separate InventoryMenu-first caller.
        for id in &ids {
            ensure!(matches!(connected_anvil_close_context(players, *id)?, MenuCloseContext::AliveConnected),
                "anvil shutdown checkpoint requires its actual connected alive owner");
        }
        for id in ids {
            let window = players
                .anvils
                .window_id(id)
                .context("anvil shutdown owner disappeared")?;
            ensure!(
                self.close_anvil_with_context_locked(
                    world, players, id, window, true, now, border, MenuCloseContext::AliveConnected,
                )?,
                "anvil shutdown owner changed before removal"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc121_rust::{entity::PlayerProfile, world_rules::WorldRules};

    /// Controlled native owners for the real read-only admission predicate.
    /// The dimension facts come from the pinned public capture, and player,
    /// inventory and physical state use their actual native constructors. This
    /// does not fabricate a world host or claim a Main game-mode command flow.
    fn native_profile_players(dimension: DimensionFacts, mode: i32) -> Result<(LivePlayers, i32)> {
        let (events, _) = broadcast::channel(16);
        let mut players = LivePlayers::new(
            LegacyRandomSource::new(11),
            LegacyRandomSource::new(12),
            WorldContext::new(dimension, WorldRules::default(), WeatherState::default()),
            2,
            true,
            events,
        );
        let id = 42;
        let mut player = PlayerState::new(
            id,
            PlayerProfile::offline("LifecycleProbe")?,
            Position {
                x: 0.5,
                y: 64.,
                z: 0.5,
                yaw: 0.,
                pitch: 0.,
                on_ground: true,
            },
        )?;
        let initial = PlayerLifecycle::new("minecraft:overworld", 1)?;
        players.player_motion_runtime.register_native(
            &player,
            &initial,
            Vec3::ZERO,
            [0.1, 0.2, 0.3],
        )?;
        // These are explicit predicate-input states; they do not simulate a
        // transition by replacing a live menu, World or owner transaction.
        player.gamemode = mode;
        let uuid = player.profile.uuid;
        players.registry.register(player)?;
        players
            .player_lifecycle
            .insert(uuid, PlayerLifecycle::new("minecraft:overworld", mode)?);
        players.containers.insert(id, SharedContainer::new().0);
        players.native_body_ready.insert(id);
        Ok((players, id))
    }

    #[tokio::test]
    async fn actual_actor_admission_and_close_context_cover_profile_changes() -> Result<()> {
        let capture = fixtures(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/configuration.json"),
        )
        .await?;
        for mode in [0, 1, 3] {
            let (players, id) = native_profile_players(capture.dimension_facts, mode)?;
            let admitted = super::super::main_anvil_dispatch::require_anvil_actor(&players, id);
            assert_eq!(admitted.is_ok(), mode == 1);
            // Survival/spectator admission refusal does not invent a dead
            // player: actual alive owners still use inventory return on close.
            assert_eq!(
                connected_anvil_close_context(&players, id)?,
                MenuCloseContext::AliveConnected
            );
        }
        let (mut players, id) = native_profile_players(capture.dimension_facts, 1)?;
        players
            .registry
            .set_entity_data(id, 11, MetadataValue::Boolean(true), true)?;
        assert!(super::super::main_anvil_dispatch::require_anvil_actor(&players, id).is_err());
        assert_eq!(
            connected_anvil_close_context(&players, id)?,
            MenuCloseContext::AliveConnected
        );
        players
            .registry
            .set_entity_data(id, 11, MetadataValue::Boolean(false), true)?;
        players
            .player_motion_runtime
            .players
            .get_mut(&id)
            .unwrap()
            .motion
            .physical
            .vehicle = Some(77);
        assert!(super::super::main_anvil_dispatch::require_anvil_actor(&players, id).is_err());
        assert_eq!(
            connected_anvil_close_context(&players, id)?,
            MenuCloseContext::AliveConnected
        );
        let physical = &mut players
            .player_motion_runtime
            .players
            .get_mut(&id)
            .unwrap()
            .motion
            .physical;
        physical.vehicle = None;
        physical.removed = true;
        assert!(super::super::main_anvil_dispatch::require_anvil_actor(&players, id).is_err());
        assert_eq!(
            connected_anvil_close_context(&players, id)?,
            MenuCloseContext::DeadOrDisconnected
        );
        players
            .player_motion_runtime
            .players
            .get_mut(&id)
            .unwrap()
            .motion
            .physical
            .removed = false;
        players
            .registry
            .set_entity_data(id, 9, MetadataValue::Float(0.), true)?;
        assert!(super::super::main_anvil_dispatch::require_anvil_actor(&players, id).is_err());
        assert_eq!(
            connected_anvil_close_context(&players, id)?,
            MenuCloseContext::DeadOrDisconnected
        );
        players.player_motion_runtime.remove(id);
        assert!(connected_anvil_close_context(&players, id).is_err());
        Ok(())
    }
}
