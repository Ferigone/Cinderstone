//! Actual LoomBlock provider admission and captured block/player access.
//! The bound native transaction owner supplies actual initialization/actions;
//! production routing is exported separately by the central integrator.
use super::*;
use crate::main_loom_patterns::NativeLoomPatterns;
use crate::main_loom::LoomRequest;
use mc121_rust::container::MenuCloseContext;
use mc121_rust::workstations::menu::{WorkstationAccess, WorkstationKind};

pub(super) const LOOM_TITLE: &str = "container.loom";
pub(super) const LOOM_INTERACTION_STAT: &str = "minecraft:interact_with_loom";
pub(super) use crate::main_stonecutter_dispatch::UnsupportedMenuReach;

pub(super) fn require_loom_reach_profile(players: &LivePlayers, id: i32) -> Result<()> {
    crate::main_stonecutter_dispatch::require_stonecutter_reach_profile(players, id)
}

fn is_loom(state: u32) -> Result<bool> {
    Ok(placement::placement_block_name(state)? == "minecraft:loom")
}

/// Observations captured while the actual world/player owners are locked.
/// Eye/range evaluation stays lazy after the real block-identity short circuit.
#[derive(Clone, Debug)]
pub(super) struct ActualLoomAccess {
    block_matches: bool,
    position: (i32, i32, i32),
    feet: Vec3,
    pose: u8,
    gamemode: i32,
}

impl WorkstationAccess for ActualLoomAccess {
    fn block_matches(&self, kind: WorkstationKind) -> Result<bool> {
        ensure!(kind == WorkstationKind::Loom, "wrong captured workstation kind");
        Ok(self.block_matches)
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
            eyes, self.position, range, additional_range,
        ))
    }
}

pub(super) fn loom_access(
    world: &mut WorldBackend,
    player: &PlayerState,
    position: (i32, i32, i32),
) -> Result<ActualLoomAccess> {
    Ok(ActualLoomAccess {
        block_matches: is_loom(world.block(position.0, position.1, position.2)?)?,
        position,
        feet: Vec3::new(player.position.x, player.position.y, player.position.z),
        pose: player.metadata.pose,
        gamemode: player.gamemode,
    })
}

/// A provider decision and actual registry view, not an installed-menu token.
/// The future LoomMenuContext must bind that view to its real level/player.
pub(super) enum LoomUseAdmission {
    Pass,
    ConsumeWithoutOpening,
    Open { spectator: bool, patterns: NativeLoomPatterns },
}

impl Server {
    pub(super) async fn use_native_loom(
        &self, id: i32, hand: i32, target: (i32, i32, i32),
    ) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        match self.preflight_native_loom_use_locked(&mut world, &mut players, id, hand, target)? {
            LoomUseAdmission::Pass => return Ok(false),
            LoomUseAdmission::ConsumeWithoutOpening => return Ok(true),
            LoomUseAdmission::Open { spectator, patterns } => {
                ensure!(spectator == (players.registry.get(id).context("loom admitted actor disappeared")?.gamemode == 3),
                    "loom admitted actor changed its provider path");
                ensure!(patterns.binding() == NativeLoomPatterns::new(&self.fixtures)?.binding(),
                    "loom admitted configuration owner changed");
            }
        }
        // There is exactly one source current external menu, never a second
        // shadow owner. InventoryMenu keeps its separate cursor/grid.
        let active = [
            players.looms.window_id(id), players.smithing_tables.window_id(id),
            players.stonecutters.window_id(id), players.crafting_tables.window_id(id),
            players.native_containers.window_id(id), players.furnaces.window_id(id),
        ];
        ensure!(active.iter().flatten().count() <= 1, "loom opening found competing current-menu owners");
        if let Some(window) = players.looms.window_id(id) {
            self.close_loom_with_context_locked(&mut world, &mut players, id, window, true,
                now, border, MenuCloseContext::AliveConnected)?;
        }
        if let Some(window) = players.smithing_tables.window_id(id) {
            self.close_smithing_with_context_locked(&mut world, &mut players, id, window, true,
                now, border, MenuCloseContext::AliveConnected)?;
        }
        if let Some(window) = players.stonecutters.window_id(id) {
            self.close_stonecutter_with_context_locked(&mut world, &mut players, id, window, true,
                now, border, MenuCloseContext::AliveConnected)?;
        }
        if let Some(window) = players.crafting_tables.window_id(id) {
            self.crafting_table_action_locked(&mut world, &mut players, id, None, Some(window), true, now, border)?;
        }
        if let Some(window) = players.native_containers.window_id(id) {
            self.native_menu_action_locked(&mut world, &mut players, id, None, Some(window), true, now, border)?;
        }
        if let Some(window) = players.furnaces.window_id(id) {
            self.furnace_menu_action_locked(&mut world, &mut players, id, None, Some(window), true, now, border)?;
        }
        let window = players.native_containers.allocate_window(id);
        self.open_loom_menu_locked(&mut world, &mut players, id, target, window, now, border)?;
        self.finish_native_loom_open_locked(&mut world, &mut players, id, target)?;
        Ok(true)
    }

    pub(super) async fn click_native_loom(&self, id: i32, click: &ContainerClick) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        *players.player_last_actions.get_mut(&id).context("loom click last-action owner missing")? =
            self.epoch.elapsed().as_millis() as i64;
        if players.looms.window_id(id) != Some(click.window_id) { return Ok(false); }
        if let Err(error) = self.loom_action_locked(&mut world, &mut players, id,
            LoomRequest::Click(click), now, border) {
            self.retire_unsupported_loom_reach_locked(&mut world, &mut players, id,
                error, now, border, "container_click")?;
        }
        Ok(true)
    }

    pub(super) async fn loom_button(&self, id: i32, window: i32, button: i32) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        *players.player_last_actions.get_mut(&id).context("loom button last-action owner missing")? =
            self.epoch.elapsed().as_millis() as i64;
        if players.looms.window_id(id).map(i32::from) != Some(window) { return Ok(false); }
        if let Err(error) = self.loom_action_locked(&mut world, &mut players, id,
            LoomRequest::Button(button), now, border) {
            self.retire_unsupported_loom_reach_locked(&mut world, &mut players, id,
                error, now, border, "container_button")?;
        }
        Ok(true)
    }

    /// The central rejected-prediction dispatcher calls this while retaining
    /// its existing operation/world/player locks and actual current owner.
    pub(super) fn resync_native_loom_locked(
        &self, world: &mut WorldBackend, players: &mut LivePlayers, id: i32,
        now: i64, border: CollisionBorder,
    ) -> Result<()> {
        if let Err(error) = self.loom_action_locked(world, players, id, LoomRequest::Resync, now, border) {
            self.retire_unsupported_loom_reach_locked(world, players, id,
                error, now, border, "component_refusal_resync")?;
        }
        Ok(())
    }

    pub(super) async fn resync_native_loom(&self, id: i32) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        if !players.looms.has_open(id) { return Ok(false); }
        self.resync_native_loom_locked(&mut world, &mut players, id, now, border)?;
        Ok(true)
    }

    fn retire_unsupported_loom_reach_locked(
        &self, world: &mut WorldBackend, players: &mut LivePlayers, id: i32,
        error: anyhow::Error, now: i64, border: CollisionBorder, trigger: &str,
    ) -> Result<()> {
        if error.downcast_ref::<UnsupportedMenuReach>().is_none() { return Err(error); }
        let window = players.looms.window_id(id).context("unsupported loom profile lost its actual menu")?;
        ensure!(self.close_loom_with_context_locked(world, players, id, window, true,
            now, border, MenuCloseContext::AliveConnected)?, "unsupported loom profile changed its actual menu");
        eprintln!("{}", json!({
            "event":"native_menu_profile_retired", "menu":"loom", "entity_id":id,
            "native_limit":"unsupported_block_interaction_range", "trigger":trigger,
            "reason":error.to_string(),
        }));
        Ok(())
    }

    /// Caller holds chat_dispatch -> world -> players. Resolve all admission
    /// before closing a current external menu, allocating a window or sending
    /// OpenScreen. A positive decision does not construct a LoomMenuContext.
    pub(super) fn preflight_native_loom_use_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        hand: i32,
        target: (i32, i32, i32),
    ) -> Result<LoomUseAdmission> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Ok(LoomUseAdmission::Pass);
        }
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(LoomUseAdmission::Pass);
        };
        if !is_loom(world.block(target.0, target.1, target.2)?)? {
            return Ok(LoomUseAdmission::Pass);
        }
        ensure!((0..=1).contains(&hand), "invalid loom interaction hand");
        require_loom_reach_profile(players, id)?;
        if !can_reach_block(player.position, player.metadata, target) {
            return Ok(LoomUseAdmission::ConsumeWithoutOpening);
        }
        let shared = players.containers.get(&id)
            .context("loom shared inventory missing")?.clone();
        let spectator = player.gamemode == 3;
        if !spectator {
            if hand != 0 { return Ok(LoomUseAdmission::Pass); }
            let empty_hands = shared.read(|menu| -> Result<_> {
                Ok(menu.inventory.selected_stack(0)?.is_empty()
                    && menu.inventory.selected_stack(1)?.is_empty())
            })??;
            if player.metadata.flags & 2 != 0 && !empty_hands {
                return Ok(LoomUseAdmission::Pass);
            }
        }
        players.require_current_native_source(world)?;
        // Reuse the genuine admitted default creative/readonly spectator body
        // contract, not a new effect/attribute state inferred from particles.
        crate::main_stonecutter::require_stonecutter_actor(players, id)?;
        let patterns = NativeLoomPatterns::new(&self.fixtures)?;
        ensure!(patterns.enabled_features().contains("minecraft:vanilla"),
            "loom requires its actual configured vanilla feature capability");
        shared.read(|menu| {
            crate::main_workstation_components::require_supported_inventory(&menu.inventory)
        })??;
        Ok(LoomUseAdmission::Open { spectator, patterns })
    }

    /// Caller must first initialize and install the actual bound LoomMenu.
    /// The spectator provider path does not invoke LoomBlock.useWithoutItem.
    pub(super) fn finish_native_loom_open_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        target: (i32, i32, i32),
    ) -> Result<()> {
        let player = players.registry.get(id).context("loom open actor missing")?;
        let uuid = player.profile.uuid;
        if player.gamemode == 3 { return Ok(()); }
        let key = mc121_rust::player_stats::stat_key("minecraft:custom", LOOM_INTERACTION_STAT)
            .context("loom interaction statistic missing")?;
        players.gameplay.stats_mut(uuid)?.increment(key, 1)?;
        let _post_use_state = world.block(target.0, target.1, target.2)?;
        players.gameplay.default_block_use(uuid, target)
    }
}

/// The actual native context can support the unique encounter order for zero
/// or one target. Several targets require the real Java Slot-identity HashSet
/// owner; the BTreeSet's sorted indices cannot replace that encounter order.
pub(super) fn loom_unambiguous_drag_order(targets: &BTreeSet<usize>) -> Result<Vec<usize>> {
    ensure!(targets.iter().all(|slot| *slot < 40), "invalid actual loom drag target");
    ensure!(targets.len() <= 1,
        "multi-target loom drag requires its actual Slot-identity HashSet encounter owner");
    Ok(targets.iter().copied().collect())
}

/// Only encode a window obtained from the actual shared allocator, after a
/// real menu/context has been constructed and before its initializer runs.
pub(super) fn loom_open_screen(window: u8) -> Result<Packet> {
    ensure!((1..=100).contains(&window), "invalid actual loom window");
    let title = Tag::Compound(vec![("translate".into(), Tag::String(LOOM_TITLE.into()))]);
    let mut payload = Writer::new();
    payload.varint(i32::from(window))
        .varint(WorkstationKind::Loom.protocol_id())
        .raw(&title.encode_network_anonymous()?);
    Ok(Packet { id: 0x33, payload: payload.into_inner() })
}
