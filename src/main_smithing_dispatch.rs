//! Actual smithing-table opening and click dispatch over the canonical owners.
//! InventoryMenu retains its own cursor/grid when this external menu opens.
use super::*;
use crate::main_smithing::{OpenSmithing, SmithingRequest};
use mc121_rust::{
    container::MenuCloseContext,
    container_workstation_smithing::{SmithingMenu, SmithingMenuEvent},
    workstations::menu::{WorkstationAccess, WorkstationKind},
};

/// An unsupported native range capability, separate from malformed state or
/// a transaction failure. The lifetime owner retires only this exact boundary.
#[derive(Debug)]
pub(super) struct UnsupportedMenuReach;

impl std::fmt::Display for UnsupportedMenuReach {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("native smithing reach requires the actual equipped attribute owner")
    }
}
impl std::error::Error for UnsupportedMenuReach {}

fn is_smithing_table(state: u32) -> Result<bool> {
    Ok(placement::placement_block_name(state)? == "minecraft:smithing_table")
}

/// Reject only an applicable modifier on an actual equipped stack. The native
/// slice does not own attribute composition; zero or cancelling modifiers do
/// not justify inventing a default resulting interaction range.
pub(super) fn require_smithing_reach_profile(players: &LivePlayers, id: i32) -> Result<()> {
    use mc121_rust::equipment::EquipmentSlot;
    players.containers.get(&id).context("smithing reach inventory missing")?
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

/// Observations captured under the actual world/player locks, not substitutes
/// for a level, a player or an inventory-owning block entity.
pub(super) struct ActualSmithingAccess {
    block_matches: bool,
    position: (i32, i32, i32),
    feet: Vec3,
    pose: u8,
    gamemode: i32,
}

impl WorkstationAccess for ActualSmithingAccess {
    fn block_matches(&self, kind: WorkstationKind) -> Result<bool> {
        ensure!(kind == WorkstationKind::Smithing, "wrong captured workstation kind");
        Ok(self.block_matches)
    }

    fn player_can_interact_with_block(&self, additional_range: f64) -> Result<bool> {
        // Keep the source block-first short circuit: unsupported pose/range
        // context must not prevent closing an already replaced smithing block.
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

pub(super) fn smithing_access(
    world: &mut WorldBackend,
    player: &PlayerState,
    position: (i32, i32, i32),
) -> Result<ActualSmithingAccess> {
    Ok(ActualSmithingAccess {
        block_matches: is_smithing_table(world.block(position.0, position.1, position.2)?)?,
        position,
        feet: Vec3::new(player.position.x, player.position.y, player.position.z),
        pose: player.metadata.pose,
        gamemode: player.gamemode,
    })
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
    pub(super) async fn use_native_smithing_table(
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
        if !is_smithing_table(world.block(target.0, target.1, target.2)?)? {
            return Ok(false);
        }
        ensure!((0..=1).contains(&hand), "invalid smithing interaction hand");
        require_smithing_reach_profile(&players, id)?;
        if !can_reach_block(player.position, player.metadata, target) {
            return Ok(true);
        }
        let shared = players.containers.get(&id)
            .context("smithing shared inventory missing")?.clone();
        // The source spectator provider path precedes ordinary hand/secondary
        // use handling and does not execute SmithingTableBlock.useWithoutItem.
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
        crate::main_smithing::require_smithing_actor(&players, id)?;
        let order = {
            let recipes = crate::main_smithing_recipes::NativeSmithingRecipes::new(
                &self.fixtures, &players.gameplay,
            )?;
            recipes.order()?
        };
        shared.read(|menu| {
            crate::main_workstation_components::require_supported_inventory(&menu.inventory)
        })??;

        if let Some(window) = players.looms.window_id(id) {
            self.close_loom_with_context_locked(
                &mut world, &mut players, id, window, true, now, border,
                MenuCloseContext::AliveConnected,
            )?;
        }
        if let Some(window) = players.smithing_tables.window_id(id) {
            self.smithing_action_locked(
                &mut world, &mut players, id,
                SmithingRequest::Close {
                    window, send_close: true, context: MenuCloseContext::AliveConnected,
                }, now, border,
            )?;
        }
        if let Some(window) = players.stonecutters.window_id(id) {
            self.close_stonecutter_with_context_locked(
                &mut world, &mut players, id, window, true, now, border,
                MenuCloseContext::AliveConnected,
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

        // Opening an external menu does not remove the retained InventoryMenu.
        let window = players.native_containers.allocate_window(id);
        let mut menu = SmithingMenu::new(window, order)?;
        let title = Tag::Compound(vec![(
            "translate".into(), Tag::String("container.upgrade".into()),
        )]);
        let mut payload = Writer::new();
        payload.varint(i32::from(window))
            .varint(WorkstationKind::Smithing.protocol_id())
            .raw(&title.encode_network_anonymous()?);
        publish(&self.entity_events, id, vec![Packet { id: 0x33, payload: payload.into_inner() }]);
        let events = shared.read(|player_menu| menu.initialize(&player_menu.inventory))??;
        ensure!(events.iter().all(|event| matches!(event, SmithingMenuEvent::Sync { .. })),
            "smithing initialization requires its actual synchronization-only source phase");
        for event in &events {
            crate::main_smithing_recipes::smithing_preflight_phase(
                crate::main_smithing_recipes::smithing_event_phase(event),
            )?;
        }
        let _ = crate::main_smithing_recipes::smithing_local_changes(&events)?;
        for event in &events {
            let phase = crate::main_smithing_recipes::smithing_event_phase(event);
            let one = std::slice::from_ref(event);
            let changes = crate::main_smithing_recipes::smithing_local_changes(one)?;
            if !changes.is_empty() {
                let stats = players.gameplay.snapshot_player_stats(player.profile.uuid)?;
                players.gameplay.prepare_menu_commit(
                    player.profile.uuid, &phase.player_inventory, changes, stats,
                )?.commit();
            }
            publish(&self.entity_events, id,
                crate::main_smithing_recipes::smithing_sync_packets(one)?);
        }
        players.smithing_tables.menus.insert(id, OpenSmithing { position: target, menu });
        if player.gamemode != 3 {
            let stat = mc121_rust::player_stats::stat_key(
                "minecraft:custom", "minecraft:interact_with_smithing_table",
            ).context("smithing interaction statistic missing")?;
            players.gameplay.stats_mut(player.profile.uuid)?.increment(stat, 1)?;
            let _post_use_state = world.block(target.0, target.1, target.2)?;
            players.gameplay.default_block_use(player.profile.uuid, target)?;
        }
        Ok(true)
    }

    pub(super) async fn click_native_smithing_table(
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
            .context("smithing click last-action owner missing")? = self.epoch.elapsed().as_millis() as i64;
        if players.smithing_tables.window_id(id) != Some(click.window_id) {
            return Ok(false);
        }
        if let Err(error) = self.smithing_action_locked(
            &mut world, &mut players, id, SmithingRequest::Click(click), now, border,
        ) {
            if error.downcast_ref::<UnsupportedMenuReach>().is_none() {
                return Err(error);
            }
            let window = players.smithing_tables.window_id(id)
                .context("unsupported smithing click lost its actual menu")?;
            ensure!(self.close_smithing_with_context_locked(
                &mut world, &mut players, id, window, true, now, border,
                MenuCloseContext::AliveConnected,
            )?, "unsupported smithing click changed its captured menu");
            eprintln!("{}", json!({
                "event":"native_menu_profile_retired", "menu":"smithing",
                "entity_id":id, "native_limit":"unsupported_block_interaction_range",
                "trigger":"container_click", "reason":error.to_string(),
            }));
        }
        Ok(true)
    }
}
