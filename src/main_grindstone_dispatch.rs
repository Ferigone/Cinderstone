//! Source-backed grindstone provider admission and captured actual block access.
//! Native opening/actions use the actual transactional GrindstoneMenuHost.
//! Central Main routing/export remains a separate integration responsibility.
use super::*;
use crate::main_grindstone::GrindstoneRequest;
use crate::main_grindstone_recipes::NativeGrindstoneEnchantments;
use mc121_rust::workstations::menu::{WorkstationAccess, WorkstationKind};

pub(super) const GRINDSTONE_TITLE: &str = "container.grindstone_title";
pub(super) const GRINDSTONE_INTERACTION_STAT: &str = "minecraft:interact_with_grindstone";

// The existing admitted workstation reach contract is shared unchanged. Its
// typed boundary and actual equipped-slot applicability must not drift between
// providers, or acquire another invented attribute-composition fallback.
pub(super) use crate::main_stonecutter_dispatch::UnsupportedMenuReach;

pub(super) fn require_grindstone_reach_profile(players: &LivePlayers, id: i32) -> Result<()> {
    crate::main_stonecutter_dispatch::require_stonecutter_reach_profile(players, id)
}

fn is_grindstone(state: u32) -> Result<bool> {
    Ok(placement::placement_block_name(state)? == "minecraft:grindstone")
}

/// Only observations of the real level block and real player, captured under
/// their ownership locks. No temporary menu or block entity is created here.
pub(super) struct ActualGrindstoneAccess {
    block_matches: bool,
    position: (i32, i32, i32),
    feet: Vec3,
    pose: u8,
    gamemode: i32,
}

impl WorkstationAccess for ActualGrindstoneAccess {
    fn block_matches(&self, kind: WorkstationKind) -> Result<bool> {
        ensure!(kind == WorkstationKind::Grindstone, "wrong captured workstation kind");
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

pub(super) fn grindstone_access(
    world: &mut WorldBackend,
    player: &PlayerState,
    position: (i32, i32, i32),
) -> Result<ActualGrindstoneAccess> {
    Ok(ActualGrindstoneAccess {
        block_matches: is_grindstone(world.block(position.0, position.1, position.2)?)?,
        position,
        feet: Vec3::new(player.position.x, player.position.y, player.position.z),
        pose: player.metadata.pose,
        gamemode: player.gamemode,
    })
}

/// A checked provider decision, not an installed menu or an ownership token.
/// Its actual registry observation must be rebound/validated by the real Host.
pub(super) enum GrindstoneUseAdmission {
    Pass,
    ConsumeWithoutOpening,
    Open { spectator: bool, enchantments: NativeGrindstoneEnchantments },
}

impl Server {
    pub(super) async fn use_native_grindstone(&self, id: i32, hand: i32, target: (i32, i32, i32)) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        match self.preflight_native_grindstone_use_locked(&mut world, &mut players, id, hand, target)? {
            GrindstoneUseAdmission::Pass => return Ok(false),
            GrindstoneUseAdmission::ConsumeWithoutOpening => return Ok(true),
            GrindstoneUseAdmission::Open { spectator, enchantments } => {
                ensure!(spectator == (players.registry.get(id).context("grindstone admitted actor disappeared")?.gamemode == 3),
                    "grindstone admitted actor changed provider path");
                ensure!(enchantments.registry_snapshot()
                    == NativeGrindstoneEnchantments::new(&self.fixtures, &players.gameplay)?.registry_snapshot(),
                    "grindstone admitted registry owner changed");
            }
        }
        let active = [players.grindstones.window_id(id), players.anvils.window_id(id),
            players.looms.window_id(id), players.smithing_tables.window_id(id),
            players.stonecutters.window_id(id), players.crafting_tables.window_id(id),
            players.native_containers.window_id(id), players.furnaces.window_id(id)];
        ensure!(active.iter().flatten().count() <= 1, "grindstone opening found competing current-menu owners");
        let close_context = crate::main_grindstone_lifecycle::connected_grindstone_close_context(&players, id)?;
        if let Some(window) = players.grindstones.window_id(id) {
            self.close_grindstone_with_context_locked(&mut world, &mut players, id, window, true,
                now, border, close_context)?;
        }
        if let Some(window) = players.anvils.window_id(id) {
            self.close_anvil_with_context_locked(&mut world, &mut players, id, window, true,
                now, border, close_context)?;
        }
        if let Some(window) = players.looms.window_id(id) {
            self.close_loom_with_context_locked(&mut world, &mut players, id, window, true,
                now, border, close_context)?;
        }
        if let Some(window) = players.smithing_tables.window_id(id) {
            self.close_smithing_with_context_locked(&mut world, &mut players, id, window, true,
                now, border, close_context)?;
        }
        if let Some(window) = players.stonecutters.window_id(id) {
            self.close_stonecutter_with_context_locked(&mut world, &mut players, id, window, true,
                now, border, close_context)?;
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
        // Source openMenu removes only a prior external owner. Window0 keeps
        // its own cursor/grid; the actual shared allocator owns the new ID.
        let window = players.native_containers.allocate_window(id);
        self.open_grindstone_menu_locked(&mut world, &mut players, id, target, window, now, border)?;
        self.finish_native_grindstone_open_locked(&mut world, &mut players, id, target)?;
        Ok(true)
    }

    pub(super) async fn click_native_grindstone(&self, id: i32, click: &ContainerClick) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        *players.player_last_actions.get_mut(&id).context("grindstone click last-action owner absent")? =
            self.epoch.elapsed().as_millis() as i64;
        let Some(current_window) = players.grindstones.window_id(id) else { return Ok(false); };
        // An active current menu consumes a wrong-window request. It must not
        // fall through to the inactive retained InventoryMenu in Main.
        if current_window != click.window_id { return Ok(true); }
        if let Err(error) = self.grindstone_action_locked(&mut world, &mut players, id,
            GrindstoneRequest::Click(click), now, border) {
            self.retire_unsupported_grindstone_reach_locked(&mut world, &mut players, id, error,
                now, border, "container_click")?;
        }
        Ok(true)
    }

    pub(super) fn resync_native_grindstone_locked(&self, world: &mut WorldBackend, players: &mut LivePlayers,
        id: i32, now: i64, border: CollisionBorder) -> Result<()> {
        if let Err(error) = self.grindstone_action_locked(world, players, id, GrindstoneRequest::Resync, now, border) {
            self.retire_unsupported_grindstone_reach_locked(world, players, id, error,
                now, border, "component_refusal_resync")?;
        }
        Ok(())
    }

    pub(super) async fn resync_native_grindstone(&self, id: i32) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        if !players.grindstones.has_open(id) { return Ok(false); }
        self.resync_native_grindstone_locked(&mut world, &mut players, id, now, border)?;
        Ok(true)
    }

    fn retire_unsupported_grindstone_reach_locked(&self, world: &mut WorldBackend, players: &mut LivePlayers,
        id: i32, error: anyhow::Error, now: i64, border: CollisionBorder, trigger: &str) -> Result<()> {
        if error.downcast_ref::<UnsupportedMenuReach>().is_none() { return Err(error); }
        let window = players.grindstones.window_id(id).context("unsupported grindstone profile lost its actual menu")?;
        let close_context = crate::main_grindstone_lifecycle::connected_grindstone_close_context(players, id)?;
        ensure!(self.close_grindstone_with_context_locked(world, players, id, window, true,
            now, border, close_context)?, "unsupported grindstone profile changed its actual menu");
        eprintln!("{}", json!({ "event":"native_menu_profile_retired", "menu":"grindstone",
            "entity_id":id, "native_limit":"unsupported_block_interaction_range",
            "trigger":trigger, "reason":error.to_string() }));
        Ok(())
    }

    /// Caller holds chat_dispatch -> world -> players. This performs the source
    /// provider gates before closing any previous menu, allocating a window or
    /// sending OpenScreen. It neither creates a Host nor claims a completed open.
    pub(super) fn preflight_native_grindstone_use_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        hand: i32,
        target: (i32, i32, i32),
    ) -> Result<GrindstoneUseAdmission> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Ok(GrindstoneUseAdmission::Pass);
        }
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(GrindstoneUseAdmission::Pass);
        };
        if !is_grindstone(world.block(target.0, target.1, target.2)?)? {
            return Ok(GrindstoneUseAdmission::Pass);
        }
        ensure!((0..=1).contains(&hand), "invalid grindstone interaction hand");
        require_grindstone_reach_profile(players, id)?;
        if !can_reach_block(player.position, player.metadata, target) {
            return Ok(GrindstoneUseAdmission::ConsumeWithoutOpening);
        }
        let shared = players.containers.get(&id)
            .context("grindstone shared inventory missing")?.clone();
        let spectator = player.gamemode == 3;
        if !spectator {
            if hand != 0 { return Ok(GrindstoneUseAdmission::Pass); }
            let empty_hands = shared.read(|menu| -> Result<_> {
                Ok(menu.inventory.selected_stack(0)?.is_empty()
                    && menu.inventory.selected_stack(1)?.is_empty())
            })??;
            if player.metadata.flags & 2 != 0 && !empty_hands {
                return Ok(GrindstoneUseAdmission::Pass);
            }
        }
        players.require_current_native_source(world)?;
        // This is the existing real admitted native body/readonly spectator
        // contract, not effect state inferred solely from missing particles.
        crate::main_stonecutter::require_stonecutter_actor(players, id)?;
        let enchantments = NativeGrindstoneEnchantments::new(&self.fixtures, &players.gameplay)?;
        shared.read(|menu| {
            crate::main_workstation_components::require_supported_inventory(&menu.inventory)
        })??;
        Ok(GrindstoneUseAdmission::Open { spectator, enchantments })
    }

    /// Invoke only after the actual Host initialized and installed its menu.
    /// Spectators used the direct provider path and do not award block effects.
    pub(super) fn finish_native_grindstone_open_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        target: (i32, i32, i32),
    ) -> Result<()> {
        let player = players.registry.get(id).context("grindstone open actor missing")?;
        let uuid = player.profile.uuid;
        if player.gamemode == 3 { return Ok(()); }
        let key = mc121_rust::player_stats::stat_key("minecraft:custom", GRINDSTONE_INTERACTION_STAT)
            .context("grindstone interaction statistic missing")?;
        players.gameplay.stats_mut(uuid)?.increment(key, 1)?;
        players.gameplay.award_scoreboard_stat(uuid, key, 1)?;
        let _post_use_state = world.block(target.0, target.1, target.2)?;
        players.gameplay.default_block_use(uuid, target)
    }
}

/// Encode only the actual shared counter's window once a real menu exists.
/// The Host owner must dispatch this before initialization, not as an invented
/// standalone screen or as a substitute for binding the actual Level/player.
pub(super) fn grindstone_open_screen(window: u8) -> Result<Packet> {
    ensure!((1..=100).contains(&window), "invalid actual grindstone window");
    let title = Tag::Compound(vec![("translate".into(), Tag::String(GRINDSTONE_TITLE.into()))]);
    let mut payload = Writer::new();
    payload.varint(i32::from(window))
        .varint(WorkstationKind::Grindstone.protocol_id())
        .raw(&title.encode_network_anonymous()?);
    Ok(Packet { id: 0x33, payload: payload.into_inner() })
}
