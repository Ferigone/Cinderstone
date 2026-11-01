//! Actual AnvilBlock opening and retained AnvilMenu wire dispatch.
//!
//! Main retains one canonical menu/inventory and delegates all Item, XP, RNG,
//! world, counter and ordered packet publication to main_anvil's transaction.
//! The retained InventoryMenu cursor/grid are never removed merely to open it.
use super::main_anvil::AnvilRequest;
use super::*;
use mc121_rust::{
    container::MenuCloseContext,
    nbt::JavaString,
    workstations::menu::{WorkstationAccess, WorkstationKind},
};

pub(super) fn is_anvil(state: u32) -> bool {
    mc121_rust::placement::in_block_tag(state, "minecraft:anvil")
}

/// FriendlyByteBuf.readUtf() uses the wire limit of 32767 UTF-16 units. The
/// menu's later source filter/50-unit admission is a separate operation.
pub(super) fn decode_anvil_rename(payload: &[u8]) -> Result<JavaString> {
    let mut fields = Reader::new(payload);
    let name = fields.string(32_767)?;
    fields.finish()?;
    Ok(JavaString::from(name.as_str()))
}

/// Use the actual equipped attribute owner already shared by native menu
/// dispatch. Unknown active block-range modifiers are an explicit boundary.
/// The same typed reach error lets the real lifecycle retire this menu safely.
pub(super) fn require_anvil_reach_profile(players: &LivePlayers, id: i32) -> Result<()> {
    main_stonecutter_dispatch::require_stonecutter_reach_profile(players, id)
}

/// Mutable production slice: actual native alive, unmounted creative player in
/// the Overworld, with its real default-effect/body and equipped reach profile.
/// A caller cannot manufacture this capability from a packet's creative flag.
pub(super) fn require_anvil_actor(players: &LivePlayers, id: i32) -> Result<()> {
    require_anvil_reach_profile(players, id)?;
    let player = players.registry.get(id).context("anvil actor missing")?;
    ensure!(
        matches!(player.entity_data.get(9), Some(MetadataValue::Float(health))
            if health.is_finite() && *health > 0.),
        "anvil requires its actual alive player"
    );
    let lifecycle = players
        .player_lifecycle
        .get(&player.profile.uuid)
        .context("anvil actual player lifecycle missing")?;
    main_player_motion_hooks::require_creative(player, lifecycle)?;
    let physical = players
        .player_motion_runtime
        .physical(id)
        .context("anvil native player body missing")?;
    ensure!(
        physical.entity_id == id
            && physical.uuid == player.profile.uuid
            && !physical.removed
            && physical.vehicle.is_none()
            && players.native_body_ready.contains(&id),
        "anvil requires its actual unmounted player body"
    );
    Ok(())
}

/// Read-only reach observation captured under the actual Level/player locks.
/// This is a temporary view, not a fabricated world or persistent block owner.
pub(super) struct ActualAnvilAccess {
    anvil: bool,
    position: (i32, i32, i32),
    feet: Vec3,
    pose: u8,
    gamemode: i32,
}
impl WorkstationAccess for ActualAnvilAccess {
    fn block_matches(&self, kind: WorkstationKind) -> Result<bool> {
        ensure!(
            kind == WorkstationKind::Anvil,
            "wrong captured anvil menu kind"
        );
        Ok(self.anvil)
    }
    fn player_can_interact_with_block(&self, additional_range: f64) -> Result<bool> {
        let eye = player_physics::player_dimensions(self.pose, 1.)?.eye_height;
        let eyes = geometry::player_eye_position(self.feet, eye);
        let range = if self.gamemode == 1 {
            geometry::CREATIVE_BLOCK_INTERACTION_RANGE
        } else {
            geometry::DEFAULT_BLOCK_INTERACTION_RANGE
        };
        Ok(geometry::can_interact_with_block(
            eyes,
            self.position,
            range,
            additional_range,
        ))
    }
}
pub(super) fn anvil_access(
    world: &mut WorldBackend,
    player: &PlayerState,
    position: (i32, i32, i32),
) -> Result<ActualAnvilAccess> {
    Ok(ActualAnvilAccess {
        anvil: is_anvil(world.block(position.0, position.1, position.2)?),
        position,
        feet: Vec3::new(player.position.x, player.position.y, player.position.z),
        pose: player.metadata.pose,
        gamemode: player.gamemode,
    })
}
pub(super) fn anvil_still_valid(
    world: &mut WorldBackend,
    player: &PlayerState,
    position: (i32, i32, i32),
) -> Result<bool> {
    let access = anvil_access(world, player, position)?;
    Ok(access.block_matches(WorkstationKind::Anvil)?
        && access.player_can_interact_with_block(4.)?)
}

impl Server {
    /// Actual use-without-item dispatch. Root calls this after source packet
    /// validation/teleport confirmation and before generic item/block fallback.
    pub(super) async fn use_native_anvil(
        &self,
        id: i32,
        hand: i32,
        target: (i32, i32, i32),
    ) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
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
        if self.shutting_down.load(Ordering::Acquire) {
            return Ok(false);
        }
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(false);
        };
        if !is_anvil(world.block(target.0, target.1, target.2)?) {
            return Ok(false);
        }
        ensure!((0..=1).contains(&hand), "invalid anvil interaction hand");
        // The admitted runtime currently owns the actual creative/body slice.
        // Broader player modes require their actual owners, never guessed flags.
        require_anvil_actor(&players, id)?;
        let access = anvil_access(&mut world, &player, target)?;
        if !access.player_can_interact_with_block(geometry::INTERACTION_VERIFICATION_BUFFER)? {
            return Ok(true);
        }
        if hand != 0 {
            return Ok(false);
        }
        let shared = players
            .containers
            .get(&id)
            .context("anvil canonical inventory missing")?
            .clone();
        let empty_hands = shared.read(|menu| -> Result<_> {
            Ok(menu.inventory.selected_stack(0)?.is_empty()
                && menu.inventory.selected_stack(1)?.is_empty())
        })??;
        if player.metadata.flags & 2 != 0 && !empty_hands {
            return Ok(false);
        }
        players.require_current_native_source(&world)?;
        players.gameplay.require_supported_floor()?;
        shared.read(|menu| {
            main_workstation_components::require_supported_inventory(&menu.inventory)
        })??;
        // Resolve the actual source stat owner before any screen handoff. The
        // real native empty-criteria registry is held stable by these locks.
        let stat =
            mc121_rust::player_stats::stat_key("minecraft:custom", "minecraft:interact_with_anvil")
                .context("anvil interaction statistic missing")?;
        let _ = players
            .gameplay
            .snapshot_player_stats(player.profile.uuid)?;

        enum Current {
            Anvil(u8),
            Stonecutter(u8),
            CraftingTable(u8),
            Container(u8),
            Furnace(u8),
        }
        let mut current = [
            players.anvils.window_id(id).map(Current::Anvil),
            players.stonecutters.window_id(id).map(Current::Stonecutter),
            players
                .crafting_tables
                .window_id(id)
                .map(Current::CraftingTable),
            players
                .native_containers
                .window_id(id)
                .map(Current::Container),
            players.furnaces.window_id(id).map(Current::Furnace),
        ]
        .into_iter()
        .flatten();
        let selected = current.next();
        ensure!(
            current.next().is_none(),
            "multiple actual menus during anvil open"
        );
        match selected {
            Some(Current::Anvil(window)) => self.anvil_action_locked(
                &mut world,
                &mut players,
                id,
                AnvilRequest::Close {
                    window,
                    send_close: true,
                    context: MenuCloseContext::AliveConnected,
                },
                now,
                border,
            )?,
            Some(Current::Stonecutter(window)) => self.stonecutter_action_locked(
                &mut world,
                &mut players,
                id,
                main_stonecutter::StonecutterRequest::Close {
                    window,
                    send_close: true,
                    context: MenuCloseContext::AliveConnected,
                },
                now,
                border,
            )?,
            Some(Current::CraftingTable(window)) => ensure!(
                self.crafting_table_action_locked(
                    &mut world,
                    &mut players,
                    id,
                    None,
                    Some(window),
                    true,
                    now,
                    border,
                )?,
                "actual crafting-table owner changed during anvil open"
            ),
            Some(Current::Container(window)) => ensure!(
                self.native_menu_action_locked(
                    &mut world,
                    &mut players,
                    id,
                    None,
                    Some(window),
                    true,
                    now,
                    border,
                )?,
                "actual container owner changed during anvil open"
            ),
            Some(Current::Furnace(window)) => ensure!(
                self.furnace_menu_action_locked(
                    &mut world,
                    &mut players,
                    id,
                    None,
                    Some(window),
                    true,
                    now,
                    border,
                )?,
                "actual furnace owner changed during anvil open"
            ),
            None => {}
        }
        // ServerPlayer.openMenu leaves the retained InventoryMenu alone. Its
        // cursor/2x2 inputs are distinct from the new empty external cursor.
        let window = players.native_containers.allocate_window(id);
        // Sole runtime publisher: OpenScreen(type8,container.repair), then
        // initMenu's actual listener/synchronizer sequence in the same FIFO.
        self.open_anvil_locked(&mut world, &mut players, id, target, window, now, border)?;
        players
            .gameplay
            .stats_mut(player.profile.uuid)?
            .increment(stat, 1)?;
        // DefaultBlockInteractionTrigger reads the POST-use actual block first.
        let _post_use = world.block(target.0, target.1, target.2)?;
        players
            .gameplay
            .default_block_use(player.profile.uuid, target)?;
        Ok(true)
    }

    pub(super) async fn click_native_anvil(&self, id: i32, click: &ContainerClick) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
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
        *players
            .player_last_actions
            .get_mut(&id)
            .context("anvil click last-action owner missing")? =
            self.epoch.elapsed().as_millis() as i64;
        let Some(current_window) = players.anvils.window_id(id) else {
            return Ok(false);
        };
        if current_window != click.window_id {
            // Source checks the packet against the ACTUAL current menu. An
            // ignored wrong-window packet must not fall through to window0.
            return Ok(true);
        }
        if let Err(error) = self.anvil_action_locked(
            &mut world,
            &mut players,
            id,
            AnvilRequest::Click(click),
            now,
            border,
        ) {
            self.recover_refused_anvil_candidate_locked(
                &mut world,
                &mut players,
                id,
                now,
                border,
                "container_click",
                error,
            )?;
        }
        Ok(true)
    }

    /// The source rename payload has no window/state ID. Root must route it to
    /// the actual retained current menu; the controller filters UTF-16/50 units.
    /// aru.handleRenameItem adds neither last-action reset nor extra broadcast.
    pub(super) async fn rename_native_anvil(&self, id: i32, name: &JavaString) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
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
        if !players.anvils.has_open(id) {
            return Ok(false);
        }
        if let Err(error) = self.anvil_action_locked(
            &mut world,
            &mut players,
            id,
            AnvilRequest::Rename(name),
            now,
            border,
        ) {
            self.recover_refused_anvil_candidate_locked(
                &mut world,
                &mut players,
                id,
                now,
                border,
                "rename_item",
                error,
            )?;
        }
        Ok(true)
    }

    /// Retained full-state recovery, also used by the actual spectator click
    /// branch. Never execute a second click, initialize anew or clear a cursor.
    pub(super) fn resync_native_anvil_locked(
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
        match self.anvil_action_locked(world, players, id, AnvilRequest::Resync, now, border) {
            Err(error)
                if error
                    .downcast_ref::<main_stonecutter_dispatch::UnsupportedMenuReach>()
                    .is_some() =>
            {
                self.retire_anvil_reach_profile_locked(world, players, id, now, border)?;
                eprintln!(
                    "{}",
                    json!({
                        "event":"native_menu_profile_retired", "menu":"anvil", "entity_id":id,
                        "native_limit":"unsupported_block_interaction_range",
                        "trigger":"retained_resync", "reason":error.to_string(),
                    })
                );
                Ok(())
            }
            result => result,
        }
    }

    /// A failed candidate transaction publishes nothing. The bounded native
    /// refusal policy repairs that prediction through the genuine retained UI;
    /// it does not convert a failed Item/world callback into a successful take.
    fn recover_refused_anvil_candidate_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
        trigger: &'static str,
        error: anyhow::Error,
    ) -> Result<()> {
        if error
            .downcast_ref::<main_stonecutter_dispatch::UnsupportedMenuReach>()
            .is_some()
        {
            self.retire_anvil_reach_profile_locked(world, players, id, now, border)?;
            eprintln!(
                "{}",
                json!({
                    "event":"native_menu_profile_retired", "menu":"anvil", "entity_id":id,
                    "native_limit":"unsupported_block_interaction_range", "trigger":trigger,
                    "reason":error.to_string(),
                })
            );
        } else {
            self.resync_native_anvil_locked(world, players, id, now, border)
                .with_context(|| {
                    format!("anvil candidate failed ({error:#}); retained recovery unavailable")
                })?;
            eprintln!(
                "{}",
                json!({
                    "event":"native_menu_candidate_refused", "menu":"anvil", "entity_id":id,
                    "trigger":trigger, "reason":format!("{error:#}"),
                })
            );
        }
        Ok(())
    }

    fn retire_anvil_reach_profile_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        let window = players
            .anvils
            .window_id(id)
            .context("unsupported anvil reach profile lost its actual menu")?;
        let player = players
            .registry
            .get(id)
            .context("anvil retirement actor missing")?;
        let health = match player.entity_data.get(9) {
            Some(MetadataValue::Float(value)) if value.is_finite() => *value,
            _ => bail!("anvil retirement actual health owner unavailable"),
        };
        let physical = players
            .player_motion_runtime
            .physical(id)
            .context("anvil retirement actual entity lifetime unavailable")?;
        ensure!(
            physical.entity_id == id && physical.uuid == player.profile.uuid,
            "anvil retirement actual player body changed"
        );
        let context = if health > 0. && !physical.removed {
            MenuCloseContext::AliveConnected
        } else {
            MenuCloseContext::DeadOrDisconnected
        };
        ensure!(
            self.close_anvil_with_context_locked(
                world, players, id, window, true, now, border, context,
            )?,
            "unsupported anvil profile changed its actual retained menu"
        );
        Ok(())
    }
}
