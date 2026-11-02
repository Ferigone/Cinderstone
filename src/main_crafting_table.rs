//! Native CraftingMenu composition over the canonical shared player inventory.
use super::*;
use mc121_rust::{crafting_table::CraftingTableMenu, inventory::PlayerInventory};
use std::cell::RefCell;
#[derive(Clone, Debug)]
struct OpenCraftingTable {
    position: (i32, i32, i32),
    menu: CraftingTableMenu,
}
#[derive(Default)]
pub(super) struct CraftingTableRuntime {
    menus: BTreeMap<i32, OpenCraftingTable>,
}
impl CraftingTableRuntime {
    pub fn window_id(&self, id: i32) -> Option<u8> {
        self.menus.get(&id).map(|m| m.menu.window_id)
    }
    pub fn has_open(&self, id: i32) -> bool {
        self.menus.contains_key(&id)
    }
    /// Observe the same canonical inventory at Player.take's current-menu phase.
    pub(super) fn broadcast_for_inventory(
        &mut self, id: i32, inventory: &PlayerInventory,
    ) -> Result<Vec<Packet>> {
        self.menus.get_mut(&id).context("current crafting table absent during pickup")?
            .menu.broadcast_changes(inventory)
    }
    pub fn pending_players(&self) -> Vec<i32> {
        self.menus.keys().copied().collect()
    }
    pub fn inventory_snapshot(&self, id: i32, inventory: &PlayerInventory) -> PlayerInventory {
        self.menus.get(&id).map_or_else(
            || inventory.clone(),
            |m| m.menu.inventory_snapshot(inventory),
        )
    }
    /// Actual removed returns cursor then the nine owned temporary inputs.
    /// Preview and retained window0 cursor do not belong to this observation.
    pub(super) fn preflight_close_return_stacks(
        &self, id: i32, check: &mut dyn FnMut(&ItemStack) -> Result<()>,
    ) -> Result<()> {
        if let Some(open) = self.menus.get(&id) {
            let inputs = open.menu.inputs();
            ensure!(inputs.len() == 9, "crafting close requires its actual nine inputs");
            check(open.menu.carried())?;
            for stack in inputs { check(stack)?; }
        }
        Ok(())
    }
}
fn send(events: &broadcast::Sender<PlayerEvent>, id: i32, packets: Vec<Packet>) {
    if !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![id]),
            packets: Arc::new(packets),
        });
    }
}
fn is_table(state: u32) -> Result<bool> {
    Ok(mc121_rust::placement::placement_block_name(state)? == "minecraft:crafting_table")
}
fn still_valid(world: &mut WorldBackend, player: &PlayerState, p: (i32, i32, i32)) -> Result<bool> {
    if !is_table(world.block(p.0, p.1, p.2)?)? {
        return Ok(false);
    }
    let eye = player_physics::player_dimensions(player.metadata.pose, 1.)?.eye_height;
    let eyes = geometry::player_eye_position(
        Vec3::new(player.position.x, player.position.y, player.position.z),
        eye,
    );
    let range = if player.gamemode == 1 {
        geometry::CREATIVE_BLOCK_INTERACTION_RANGE
    } else {
        geometry::DEFAULT_BLOCK_INTERACTION_RANGE
    };
    Ok(geometry::can_interact_with_block(eyes, p, range, 4.))
}
impl Server {
    /// Resend the retained crafting menu, its real grid/result and its own
    /// cursor through sendAllDataToRemote. Do not execute another recipe/click
    /// or replace the preserved local inventory-criterion baseline.
    pub(super) fn resync_native_crafting_table_locked(
        &self,
        _world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        _now: i64,
        _border: CollisionBorder,
    ) -> Result<()> {
        let mut open = players.crafting_tables.menus.get(&id).cloned()
            .context("resync requires the actual current crafting-table menu")?;
        let shared = players.containers.get(&id)
            .context("crafting-table resync inventory missing")?.clone();
        let packet = shared.read(|menu| open.menu.full_sync(&menu.inventory))??;
        players.crafting_tables.menus.insert(id, open);
        send(&self.entity_events, id, vec![packet]);
        Ok(())
    }

    pub(super) async fn use_native_crafting_table(
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
        if !is_table(world.block(target.0, target.1, target.2)?)? {
            return Ok(false);
        }
        ensure!((0..=1).contains(&hand), "invalid interaction hand");
        if !can_reach_block(player.position, player.metadata, target) || player.gamemode == 3 {
            return Ok(true);
        }
        let shared = players
            .containers
            .get(&id)
            .context("crafting shared inventory missing")?
            .clone();
        let empty = shared.read(|m| -> Result<_> {
            Ok(m.inventory.selected_stack(0)?.is_empty()
                && m.inventory.selected_stack(1)?.is_empty())
        })??;
        if player.metadata.flags & 2 != 0 && !empty {
            return Ok(false);
        }
        players.require_current_native_source(&world)?;
        players.gameplay.require_supported_floor()?;
        ensure!(
            players.world_context.rules.boolean("doLimitedCrafting") == Some(false),
            "limited crafting requires actual known-recipe unlock owner"
        );
        if let Some(window) = players.grindstones.window_id(id) {
            self.close_grindstone_with_context_locked(
                &mut world, &mut players, id, window, true, now, border,
                mc121_rust::container::MenuCloseContext::AliveConnected,
            )?;
        }
        if let Some(window) = players.anvils.window_id(id) {
            self.close_anvil_with_context_locked(
                &mut world, &mut players, id, window, true, now, border,
                mc121_rust::container::MenuCloseContext::AliveConnected,
            )?;
        }
        if let Some(window) = players.looms.window_id(id) {
            self.close_loom_with_context_locked(
                &mut world, &mut players, id, window, true, now, border,
                mc121_rust::container::MenuCloseContext::AliveConnected,
            )?;
        }
        if let Some(window) = players.smithing_tables.window_id(id) {
            self.close_smithing_with_context_locked(
                &mut world,
                &mut players,
                id,
                window,
                true,
                now,
                border,
                mc121_rust::container::MenuCloseContext::AliveConnected,
            )?;
        }
        if let Some(window) = players.stonecutters.window_id(id) {
            self.close_stonecutter_with_context_locked(
                &mut world,
                &mut players,
                id,
                window,
                true,
                now,
                border,
                mc121_rust::container::MenuCloseContext::AliveConnected,
            )?;
        }
        if let Some(window) = players.crafting_tables.window_id(id) {
            self.crafting_table_action_locked(
                &mut world,
                &mut players,
                id,
                None,
                Some(window),
                true,
                now,
                border,
            )?;
        }
        if let Some(window) = players.native_containers.window_id(id) {
            self.native_menu_action_locked(
                &mut world,
                &mut players,
                id,
                None,
                Some(window),
                true,
                now,
                border,
            )?;
        }
        if let Some(window) = players.furnaces.window_id(id) {
            self.furnace_menu_action_locked(&mut world, &mut players, id, None, Some(window), true, now, border)?;
        }
        // ServerPlayer.openMenu leaves the retained InventoryMenu untouched.
        // Its own cursor and 2x2 inputs are not owned by this external menu.
        let window = players.native_containers.allocate_window(id);
        let mut menu = CraftingTableMenu::new(window)?;
        let initial = shared.read(|m| menu.full_sync(&m.inventory))??;
        let stat = mc121_rust::player_stats::stat_key(
            "minecraft:custom",
            "minecraft:interact_with_crafting_table",
        )
        .context("crafting table statistic missing")?;
        players
            .gameplay
            .stats_mut(player.profile.uuid)?
            .increment(stat, 1)?;
        players.crafting_tables.menus.insert(
            id,
            OpenCraftingTable {
                position: target,
                menu,
            },
        );
        let title = Tag::Compound(vec![(
            "translate".into(),
            Tag::String("container.crafting".into()),
        )]);
        let mut w = Writer::new();
        w.varint(window as i32)
            .varint(12)
            .raw(&title.encode_network_anonymous()?);
        send(
            &self.entity_events,
            id,
            vec![
                Packet {
                    id: 0x33,
                    payload: w.into_inner(),
                },
                initial,
            ],
        );
        Ok(true)
    }
    pub(super) async fn click_native_crafting_table(
        &self,
        id: i32,
        click: &ContainerClick,
    ) -> Result<bool> {
        self.crafting_table_action(id, Some(click), None, false)
            .await
    }
    pub(super) async fn close_native_crafting_table(
        &self,
        id: i32,
        window: u8,
        send_close: bool,
    ) -> Result<bool> {
        self.crafting_table_action(id, None, Some(window), send_close)
            .await
    }
    async fn crafting_table_action(
        &self,
        id: i32,
        click: Option<&ContainerClick>,
        close: Option<u8>,
        send_close: bool,
    ) -> Result<bool> {
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
        self.crafting_table_action_locked(
            &mut world,
            &mut players,
            id,
            click,
            close,
            send_close,
            now,
            border,
        )
    }
    pub(super) fn crafting_table_action_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        click: Option<&ContainerClick>,
        close: Option<u8>,
        send_close: bool,
        now: i64,
        border: CollisionBorder,
    ) -> Result<bool> {
        self.crafting_table_action_with_close_context_locked(
            world,
            players,
            id,
            click,
            close,
            send_close,
            now,
            border,
            mc121_rust::container::MenuCloseContext::AliveConnected,
        )
    }
    pub(super) fn crafting_table_action_with_close_context_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        click: Option<&ContainerClick>,
        close: Option<u8>,
        send_close: bool,
        now: i64,
        border: CollisionBorder,
        close_context: mc121_rust::container::MenuCloseContext,
    ) -> Result<bool> {
        let Some(open) = players.crafting_tables.menus.get(&id).cloned() else {
            return Ok(false);
        };
        if click.is_some_and(|c| c.window_id != open.menu.window_id) {
            return Ok(true);
        }
        if close.is_some_and(|window| window != open.menu.window_id) {
            return Ok(false);
        }
        let player = players
            .registry
            .get(id)
            .context("crafting actor missing")?
            .clone();
        if click.is_some() && !still_valid(world, &player, open.position)? {
            return self.crafting_table_action_locked(
                world,
                players,
                id,
                None,
                Some(open.menu.window_id),
                true,
                now,
                border,
            );
        }
        players.require_current_native_source(world)?;
        let staged = RefCell::new(open);
        let staged = &staged;
        let world_slot = RefCell::new(world);
        let shared = players
            .containers
            .get(&id)
            .context("crafting shared inventory missing")?
            .clone();
        let stats = players
            .gameplay
            .snapshot_player_stats(player.profile.uuid)?;
        let facts = players.world_context.item_facts(now, border, false)?;
        let rain_available = matches!(
            facts.rain,
            mc121_rust::runtime_item_world::RainAvailability::NotRaining
        ) || players.rain_context.is_some();
        let drop = PlayerDrop {
            player_uuid: player.profile.uuid,
            position: player.position,
            eye_height: player_physics::player_dimensions(player.metadata.pose, 1.)?.eye_height,
            around: false,
            trace_item: false,
        };
        let LivePlayers {
            item_runtime,
            player_random,
            gameplay,
            chunk_runtime,
            chunk_transports,
            crafting_tables,
            ..
        } = players;
        let close_packet = if send_close && close.is_some() {
            let mut w = Writer::new();
            w.u8(staged.borrow().menu.window_id);
            Some(Packet {
                id: 0x12,
                payload: w.into_inner(),
            })
        } else {
            None
        };
        menu_action_with_items(
            &shared,
            DropCommitOwner {
                runtime: item_runtime,
                player_random: player_random
                    .get_mut(&id)
                    .context("crafting player RNG missing")?,
                stats,
                entity_counter: &self.entities,
            },
            drop,
            |m| {
                let mut s = staged.borrow_mut();
                let outcome = if close.is_some() {
                    s.menu.close_with_context(&mut m.inventory, close_context)?
                } else if player.gamemode == 3 {
                    ClickOutcome {
                        packets: vec![s.menu.full_sync(&m.inventory)?],
                        ..Default::default()
                    }
                } else {
                    s.menu.click(
                        click.context("crafting click absent")?,
                        &mut m.inventory,
                        player.gamemode == 1,
                        &RecipeContext::default(),
                    )?
                };
                if close.is_some() {
                    s.menu.transfer_inventory_remote_state(m)?;
                } else {
                    let _ = m.broadcast_changes()?;
                }
                Ok(outcome)
            },
            |_| {
                ensure!(
                    rain_available,
                    "crafting item drop requires actual rain context"
                );
                chunk_runtime.certify_current_query_region(
                    &mut world_slot.borrow_mut(),
                    chunk_transports,
                    &self.entity_events,
                    now,
                    (
                        ((player.position.x - 18.).floor() as i32).div_euclid(16),
                        ((player.position.z - 18.).floor() as i32).div_euclid(16),
                    ),
                    (
                        ((player.position.x + 18.).floor() as i32).div_euclid(16),
                        ((player.position.z + 18.).floor() as i32).div_euclid(16),
                    ),
                ).map(|_| ())
            },
            || random_seed::create_random().map_err(anyhow::Error::from),
            DropCommitCallbacks {
                prepare: move |m: &mut PlayerContainer, mut next_stats: PlayerStats| {
                    let gameplay = gameplay;
                    for (item, count) in staged.borrow_mut().menu.take_crafted_items() {
                        next_stats
                            .increment(mc121_rust::player_stats::StatKey::new(1, item)?, count)?;
                    }
                    let changes = m.take_local_slot_changes();
                    gameplay.prepare_menu_commit(
                        player.profile.uuid,
                        &m.inventory,
                        changes,
                        next_stats,
                    )
                },
                before_spawn: || {
                    if let Some(packet) = &close_packet {
                        send(&self.entity_events, id, vec![packet.clone()]);
                    }
                    let s = staged.borrow();
                    if close.is_some() {
                        crafting_tables.menus.remove(&id);
                    } else {
                        crafting_tables.menus.insert(id, s.clone());
                    }
                },
                commit: |p: main_item_gameplay::PreparedGameplayMenu<'_>| p.commit(),
            },
        )?;
        Ok(true)
    }
    pub(super) fn close_crafting_tables_for_save_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        now: i64,
    ) -> Result<()> {
        let border = self.native_container_border()?;
        for id in players.crafting_tables.pending_players() {
            let window = players.crafting_tables.window_id(id).unwrap();
            self.crafting_table_action_locked(
                world,
                players,
                id,
                None,
                Some(window),
                true,
                now,
                border,
            )?;
        }
        Ok(())
    }
    pub(super) async fn tick_native_crafting_tables(&self) -> Result<()> {
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
        for id in players.crafting_tables.pending_players() {
            let Some(open) = players.crafting_tables.menus.get(&id).cloned() else {
                continue;
            };
            let Some(player) = players.registry.get(id).cloned() else {
                bail!("orphan crafting table owner")
            };
            if !still_valid(&mut world, &player, open.position)? {
                self.crafting_table_action_locked(
                    &mut world,
                    &mut players,
                    id,
                    None,
                    Some(open.menu.window_id),
                    true,
                    now,
                    border,
                )?;
                continue;
            }
            let shared = players
                .containers
                .get(&id)
                .context("crafting shared inventory missing")?
                .clone();
            let open = players.crafting_tables.menus.get_mut(&id).unwrap();
            let packets = shared.read(|m| open.menu.broadcast_changes(&m.inventory))??;
            send(&self.entity_events, id, packets);
        }
        Ok(())
    }
}
