//! Actual block opening and wire dispatch for the native StonecutterMenu.
//! The retained InventoryMenu keeps its own cursor and 2x2 crafting inputs.
use super::*;

/// A native profile boundary, distinct from malformed data or an internal
/// transaction failure. The player phase can retire its real menu safely.
#[derive(Debug)]
pub(super) struct UnsupportedMenuReach;

impl std::fmt::Display for UnsupportedMenuReach {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("native workstation reach requires the actual equipped attribute owner")
    }
}
impl std::error::Error for UnsupportedMenuReach {}
use crate::main_stonecutter::{OpenStonecutter, StonecutterRequest};
use mc121_rust::{
    container::MenuCloseContext,
    container_workstation_stonecutter::{StonecutterMenu, StonecutterMenuEvent},
    workstations::menu::{WorkstationAccess, WorkstationKind},
};

fn is_stonecutter(state: u32) -> Result<bool> {
    Ok(mc121_rust::placement::placement_block_name(state)? == "minecraft:stonecutter")
}

/// Default reach is supported only when the actual equipped slot has no
/// applicable block-interaction-range modifier. Do not invent its resulting
/// attribute value or reject an inactive modifier in another slot group.
pub(super) fn require_stonecutter_reach_profile(players: &LivePlayers, id: i32) -> Result<()> {
    use mc121_rust::equipment::EquipmentSlot;
    players.containers.get(&id).context("stonecutter reach inventory missing")?
        .read(|menu| -> Result<()> {
            let equipment = PlayerEquipment::from_inventory(&menu.inventory)?;
            for slot in EquipmentSlot::ALL {
                let stack = equipment.get(slot);
                if stack.is_empty() { continue; }
                let Some(component) = stack.component_value(12)? else { continue; };
                for modifier in component["attributes"].as_array().context("invalid equipped attributes")? {
                    if modifier["typeId"].as_u64().context("invalid equipped attribute ID")? != 6 { continue; }
                    let applies = match modifier["slot"].as_str().context("invalid equipment slot group")? {
                        "any" => true,
                        "main_hand" => slot == EquipmentSlot::MainHand,
                        "off_hand" => slot == EquipmentSlot::OffHand,
                        "hand" => matches!(slot, EquipmentSlot::MainHand | EquipmentSlot::OffHand),
                        "feet" => slot == EquipmentSlot::Feet,
                        "legs" => slot == EquipmentSlot::Legs,
                        "chest" => slot == EquipmentSlot::Chest,
                        "head" => slot == EquipmentSlot::Head,
                        "armor" => matches!(slot, EquipmentSlot::Feet | EquipmentSlot::Legs | EquipmentSlot::Chest | EquipmentSlot::Head | EquipmentSlot::Body),
                        "body" => slot == EquipmentSlot::Body,
                        _ => bail!("unknown equipment slot group"),
                    };
                    if applies { return Err(UnsupportedMenuReach.into()); }
                }
            }
            Ok(())
        })?
}

/// A read-only snapshot captured while the actual level/player locks are held.
pub(super) struct ActualStonecutterAccess {
    stonecutter_block: bool,
    position: (i32, i32, i32),
    feet: Vec3,
    pose: u8,
    gamemode: i32,
}

impl WorkstationAccess for ActualStonecutterAccess {
    fn block_matches(&self, kind: WorkstationKind) -> Result<bool> {
        ensure!(kind == WorkstationKind::Stonecutter, "wrong captured workstation kind");
        Ok(self.stonecutter_block)
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

/// Supply the kernel's actual menu validity check with its captured level/block
/// and real actor. Stonecutters have no inventory-owning block entity.
pub(super) fn stonecutter_access(
    world: &mut WorldBackend,
    player: &PlayerState,
    position: (i32, i32, i32),
) -> Result<ActualStonecutterAccess> {
    let stonecutter_block = is_stonecutter(world.block(position.0, position.1, position.2)?)?;
    Ok(ActualStonecutterAccess {
        stonecutter_block,
        position,
        feet: Vec3::new(player.position.x, player.position.y, player.position.z),
        pose: player.metadata.pose,
        gamemode: player.gamemode,
    })
}

pub(super) fn stonecutter_still_valid(
    world: &mut WorldBackend,
    player: &PlayerState,
    position: (i32, i32, i32),
) -> Result<bool> {
    let access = stonecutter_access(world, player, position)?;
    Ok(access.block_matches(WorkstationKind::Stonecutter)?
        && access.player_can_interact_with_block(4.)?)
}

fn publish(events: &broadcast::Sender<PlayerEvent>, id: i32, packets: Vec<Packet>) {
    if !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![id]),
            packets: Arc::new(packets),
        });
    }
}

impl Server {
    pub(super) async fn use_native_stonecutter(
        &self,
        id: i32,
        hand: i32,
        target: (i32, i32, i32),
    ) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        if self.shutting_down.load(Ordering::Acquire) {
            return Ok(false);
        }
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(false);
        };
        if !is_stonecutter(world.block(target.0, target.1, target.2)?)? {
            return Ok(false);
        }
        ensure!((0..=1).contains(&hand), "invalid stonecutter interaction hand");
        require_stonecutter_reach_profile(&players, id)?;
        if !can_reach_block(player.position, player.metadata, target) {
            return Ok(true);
        }
        let shared = players.containers.get(&id)
            .context("stonecutter shared inventory missing")?.clone();
        // Spectators take the source's MenuProvider branch before ordinary
        // hand/sneak suppression, and do not execute StonecutterBlock.useWithoutItem.
        if player.gamemode != 3 {
            if hand != 0 {
                return Ok(false);
            }
            let empty_hands = shared.read(|menu| -> Result<_> {
                Ok(menu.inventory.selected_stack(0)?.is_empty()
                    && menu.inventory.selected_stack(1)?.is_empty())
            })??;
            if player.metadata.flags & 2 != 0 && !empty_hands {
                return Ok(false);
            }
        }
        players.require_current_native_source(&world)?;
        players.gameplay.require_supported_floor()?;
        let _recipes = crate::main_stonecutter_recipes::NativeStonecutterRecipes::new(
            &self.fixtures, &players.gameplay,
        )?;
        crate::main_stonecutter::require_stonecutter_actor(&players, id)?;

        if let Some(window) = players.stonecutters.window_id(id) {
            self.stonecutter_action_locked(
                &mut world, &mut players, id,
                StonecutterRequest::Close {
                    window, send_close: true, context: MenuCloseContext::AliveConnected,
                }, now, border,
            )?;
        }
        if let Some(window) = players.crafting_tables.window_id(id) {
            self.crafting_table_action_locked(
                &mut world, &mut players, id, None, Some(window), true, now, border,
            )?;
        }
        if let Some(window) = players.native_containers.window_id(id) {
            self.native_menu_action_locked(
                &mut world, &mut players, id, None, Some(window), true, now, border,
            )?;
        }
        if let Some(window) = players.furnaces.window_id(id) {
            self.furnace_menu_action_locked(
                &mut world, &mut players, id, None, Some(window), true, now, border,
            )?;
        }

        // ServerPlayer.openMenu does not remove InventoryMenu here. In
        // particular, that retained menu's cursor is not this new menu's cursor.
        let window = players.native_containers.allocate_window(id);
        let mut menu = StonecutterMenu::new(window)?;
        let title = Tag::Compound(vec![(
            "translate".into(), Tag::String("container.stonecutter".into()),
        )]);
        let mut payload = Writer::new();
        payload.varint(window as i32)
            .varint(WorkstationKind::Stonecutter.protocol_id())
            .raw(&title.encode_network_anonymous()?);
        // The source dispatches OpenScreen before initMenu installs its local
        // listener and synchronizer, rather than publishing a reversed batch.
        publish(&self.entity_events, id, vec![Packet { id: 0x33, payload: payload.into_inner() }]);
        let events = shared.read(|player_menu| menu.initialize(&player_menu.inventory))??;
        ensure!(events.iter().all(|event| matches!(event, StonecutterMenuEvent::Sync { .. })),
            "stonecutter initialization requires its actual synchronization-only source phase");
        // Resolve fallible event/phase checks before executing the actual
        // listener sequence. Each callback retains its own observed inventory,
        // rather than substituting the final menu inventory for every event.
        for event in &events {
            crate::main_stonecutter_recipes::stonecutter_preflight_phase(
                crate::main_stonecutter_recipes::stonecutter_event_phase(event),
            )?;
        }
        let _ = crate::main_stonecutter_recipes::stonecutter_local_changes(&events)?;
        for event in &events {
            let phase = crate::main_stonecutter_recipes::stonecutter_event_phase(event);
            let one = std::slice::from_ref(event);
            let changes = crate::main_stonecutter_recipes::stonecutter_local_changes(one)?;
            if !changes.is_empty() {
                let stats = players.gameplay.snapshot_player_stats(player.profile.uuid)?;
                players.gameplay.prepare_menu_commit(
                    player.profile.uuid, &phase.inventory, changes, stats,
                )?.commit();
            }
            publish(&self.entity_events, id,
                crate::main_stonecutter_recipes::stonecutter_sync_packets(one)?);
        }
        players.stonecutters.menus.insert(id, OpenStonecutter { position: target, menu });
        if player.gamemode != 3 {
            let stat = mc121_rust::player_stats::stat_key(
                "minecraft:custom", "minecraft:interact_with_stonecutter",
            ).context("stonecutter interaction statistic missing")?;
            players.gameplay.stats_mut(player.profile.uuid)?.increment(stat, 1)?;
            // DefaultBlockInteractionTrigger reads the post-use block before
            // consulting its actual player's registered criterion listeners.
            let _post_use_state = world.block(target.0, target.1, target.2)?;
            players.gameplay.default_block_use(player.profile.uuid, target)?;
        }
        Ok(true)
    }

    pub(super) async fn click_native_stonecutter(
        &self,
        id: i32,
        click: &ContainerClick,
    ) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        *players.player_last_actions.get_mut(&id)
            .context("stonecutter click last-action owner missing")? = self.epoch.elapsed().as_millis() as i64;
        if players.stonecutters.window_id(id) != Some(click.window_id) {
            return Ok(false);
        }
        self.stonecutter_action_locked(
            &mut world, &mut players, id, StonecutterRequest::Click(click), now, border,
        )?;
        Ok(true)
    }

    pub(super) async fn stonecutter_button(
        &self,
        id: i32,
        window: i32,
        button: i32,
    ) -> Result<bool> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        *players.player_last_actions.get_mut(&id)
            .context("stonecutter button last-action owner missing")? = self.epoch.elapsed().as_millis() as i64;
        if !players.stonecutters.window_id(id).is_some_and(|current| i32::from(current) == window) {
            return Ok(false);
        }
        self.stonecutter_action_locked(
            &mut world, &mut players, id, StonecutterRequest::Button(button), now, border,
        )?;
        Ok(true)
    }
}
