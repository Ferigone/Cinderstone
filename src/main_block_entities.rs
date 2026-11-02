//! Canonical native container composition under world -> players -> menu locks.
//! Generic slots and the shared player's inventory commit with real drop/RNG
//! owners; no menu keeps a permanent second copy of the player's inventory.
use super::*;
use mc121_rust::{
    container_generic::GenericContainerMenu,
    inventory::PlayerInventory,
    native_block_entities::{self, ContainerEffect, ContainerKey, ContainerKind},
};
use std::cell::RefCell;
#[derive(Deserialize)]
struct NativeContainerFacts {
    sounds: BTreeMap<String, i32>,
    block_ids: BTreeMap<String, i32>,
}
fn native_container_facts() -> &'static NativeContainerFacts {
    static FACTS: std::sync::OnceLock<NativeContainerFacts> = std::sync::OnceLock::new();
    FACTS.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/native-container-facts.json"))
            .expect("original container registry facts")
    })
}
#[derive(Clone, Debug)]
struct OpenNativeContainer {
    key: ContainerKey,
    menu: GenericContainerMenu,
    spectator: bool,
}
#[derive(Default)]
pub(super) struct NativeContainerRuntime {
    menus: BTreeMap<i32, OpenNativeContainer>,
    counters: BTreeMap<i32, u8>,
}
impl NativeContainerRuntime {
    pub fn window_id(&self, id: i32) -> Option<u8> {
        self.menus.get(&id).map(|m| m.menu.window_id)
    }
    pub fn has_open(&self, id: i32) -> bool {
        self.menus.contains_key(&id)
    }
    /// Actual candidate admission before canonical Inventory.add. Every
    /// inspected value belongs to this retained menu/BE or the borrowed player.
    pub(super) fn preflight_inventory_pickup(
        &self, world: &WorldBackend, id: i32, player: PickupPlayer,
        inventory: &PlayerInventory, candidate: &ItemStack,
        hooks: &mut main_item_gameplay::MainGameplayFrame<'_>,
    ) -> Result<()> {
        ensure!(id == player.entity_id, "native container pickup actor changed before add");
        let open = self.menus.get(&id).context("current native container absent before pickup add")?;
        let WorldBackend::Fixture(flat) = world else {
            bail!("native container pickup requires its actual block entity")
        };
        let top = flat.native_block_entities().read_slots(&open.key)?;
        main_workstation_components::require_supported_inventory(inventory)?;
        open.menu.preflight_broadcast_context(&top, inventory, main_workstation_components::require_supported_stack)?;
        main_workstation_components::require_supported_stack(candidate)?;
        for slot in (36..45).chain(9..36).chain((5..9).rev()).chain(std::iter::once(45)) {
            if let Some(stack) = inventory.slot(slot).filter(|stack| !stack.is_empty()) { stack.max_stack_size()?; }
        }
        if !candidate.is_empty() { candidate.max_stack_size()?; }
        hooks.preflight_inventory_slot_changes(player, inventory, &[])
    }
    /// Player.take synchronizes the actual current menu after Inventory.add.
    /// Container contents remain borrowed from the real block entity.
    pub(super) fn broadcast_for_inventory(
        &mut self, world: &WorldBackend, id: i32, player: PickupPlayer,
        inventory: &PlayerInventory, hooks: &mut main_item_gameplay::MainGameplayFrame<'_>,
    ) -> Result<Vec<Packet>> {
        ensure!(id == player.entity_id, "native container pickup actor changed");
        let open = self.menus.get(&id).context("current native container absent during pickup")?;
        let WorldBackend::Fixture(flat) = world else {
            bail!("native container pickup requires its actual block entity")
        };
        let top = flat.native_block_entities().read_slots(&open.key)?;
        let mut next = open.menu.clone();
        let packets = next.broadcast_changes_with_listener(&top, inventory, |_menu_slot, player_slot, stack| {
            if let Some(slot) = player_slot {
                mc121_rust::item_host::RuntimeGameplayHooks::inventory_slot_changed(
                    hooks, player, slot, inventory, stack,
                )?;
            }
            // Actual ServerPlayer slot listener ignores foreign containers.
            Ok(())
        })?;
        self.menus.get_mut(&id).expect("exclusive native container pickup owner").menu = next;
        Ok(packets)
    }
    /// Includes spectator menus, whose captured container does not increment
    /// the block entity's opener counter but still retains its actual owner.
    pub(super) fn has_menu_at(&self, position: (i32, i32, i32)) -> bool {
        self.menus.values().any(|open| open.key.0.contains(&position))
    }
    pub fn pending_players(&self) -> Vec<i32> {
        self.menus.keys().copied().collect()
    }
    pub fn allocate_window(&mut self, id: i32) -> u8 {
        let next = self.counters.get(&id).copied().unwrap_or(0) % 100 + 1;
        self.counters.insert(id, next);
        next
    }
    pub fn inventory_snapshot(&self, id: i32, inventory: &PlayerInventory) -> PlayerInventory {
        self.menus.get(&id).map_or_else(
            || inventory.clone(),
            |m| m.menu.inventory_snapshot(inventory),
        )
    }
    /// Observe only this actual menu's returned cursor before any old-menu
    /// close. Chest/barrel contents stay with their real block entity.
    pub(super) fn preflight_close_return_stacks(
        &self, id: i32, check: &mut dyn FnMut(&ItemStack) -> Result<()>,
    ) -> Result<()> {
        if let Some(open) = self.menus.get(&id) { check(open.menu.carried())?; }
        Ok(())
    }
}
fn send_private(events: &broadcast::Sender<PlayerEvent>, id: i32, packets: Vec<Packet>) {
    if !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![id]),
            packets: Arc::new(packets),
        });
    }
}
fn still_valid(world: &mut WorldBackend, player: &PlayerState, key: &ContainerKey) -> Result<bool> {
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
    let WorldBackend::Fixture(flat) = world else {
        return Ok(false);
    };
    for p in &key.0 {
        if flat
            .native_block_entities()
            .validate_at(*p, flat.block(p.0, p.1, p.2))
            .is_err()
            || !geometry::can_interact_with_block(eyes, *p, range, 4.)
        {
            return Ok(false);
        }
    }
    Ok(true)
}
impl Server {
    /// Genuine sendAllDataToRemote on the retained current block menu. This
    /// refreshes remote predictions only; it does not run a click, reinitialize
    /// local listeners or move this external menu's cursor into InventoryMenu.
    pub(super) fn resync_native_container_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        _now: i64,
        _border: CollisionBorder,
    ) -> Result<()> {
        let mut open = players.native_containers.menus.get(&id).cloned()
            .context("resync requires the actual current native container")?;
        let shared = players.containers.get(&id)
            .context("native container resync inventory missing")?.clone();
        let WorldBackend::Fixture(flat) = &*world else {
            bail!("native container resync requires its actual block-entity owner")
        };
        let top = flat.native_block_entities().read_slots(&open.key)?;
        let packet = shared.read(|menu| open.menu.full_sync(&top, &menu.inventory))??;
        players.native_containers.menus.insert(id, open);
        send_private(&self.entity_events, id, vec![packet]);
        Ok(())
    }

    pub(super) fn close_inventory_menu_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        self.close_inventory_menu_with_context_locked(
            world,
            players,
            id,
            now,
            border,
            mc121_rust::container::MenuCloseContext::AliveConnected,
        )
    }
    pub(super) fn close_inventory_menu_with_context_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
        close_context: mc121_rust::container::MenuCloseContext,
    ) -> Result<()> {
        ensure!(
            close_context == mc121_rust::container::MenuCloseContext::DeadOrDisconnected
                || (!players.native_containers.has_open(id)
                    && !players.crafting_tables.has_open(id)
                    && !players.furnaces.has_open(id)
                    && !players.stonecutters.has_open(id)
                    && !players.smithing_tables.has_open(id)
                    && !players.looms.has_open(id)
                    && !players.grindstones.has_open(id)
                    && !players.anvils.has_open(id)),
            "close actual native menu before InventoryMenu cleanup"
        );
        // Player.remove cleans the retained InventoryMenu before the actual
        // external menu. Only its own cursor and 2x2 inputs belong to this call;
        // the active menu's separate cursor is handled by its later removal.
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(());
        };
        let shared = players
            .containers
            .get(&id)
            .context("closing inventory missing")?
            .clone();
        if players.gameplay.require_supported_floor().is_err()
            || players.chunk_runtime.require_supported_lighting().is_err()
            || players.require_current_native_source(world).is_err()
        {
            return menu_without_world_drops(&shared, |m| m.close_with_context(close_context));
        }
        let facts = players.world_context.item_facts(now, border, false)?;
        let rain_available = matches!(
            facts.rain,
            mc121_rust::runtime_item_world::RainAvailability::NotRaining
        ) || players.rain_context.is_some();
        let stats = players
            .gameplay
            .snapshot_player_stats(player.profile.uuid)?;
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
            ..
        } = players;
        menu_action_with_items(
            &shared,
            DropCommitOwner {
                runtime: item_runtime,
                player_random: player_random
                    .get_mut(&id)
                    .context("closing inventory RNG missing")?,
                stats,
                entity_counter: &self.entities,
            },
            drop,
            |m| m.close_with_context(close_context),
            |_| {
                ensure!(rain_available, "closing inventory drop rain owner missing");
                chunk_runtime.certify_current_query_region(
                    world,
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
                prepare: |menu: &mut PlayerContainer, next_stats| {
                    let gameplay = gameplay;
                    let changes = menu.take_local_slot_changes();
                    gameplay.prepare_menu_commit(
                        player.profile.uuid,
                        &menu.inventory,
                        changes,
                        next_stats,
                    )
                },
                before_spawn: || {},
                commit: |p: main_item_gameplay::PreparedGameplayMenu<'_>| p.commit(),
            },
        )
    }
    pub(super) fn native_container_border(&self) -> Result<CollisionBorder> {
        let b = self
            .border
            .read()
            .map_err(|_| anyhow!("border poisoned"))?
            .bounds(self.epoch.elapsed().as_millis() as i64);
        Ok(CollisionBorder {
            min_x: b.min_x,
            min_z: b.min_z,
            max_x: b.max_x,
            max_z: b.max_z,
        })
    }
    pub(super) async fn use_native_container(
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
        let state = world.block(target.0, target.1, target.2)?;
        let Some(kind) = native_block_entities::container_kind(state) else {
            return Ok(false);
        };
        if !can_reach_block(player.position, player.metadata, target) {
            return Ok(true);
        }
        ensure!((0..=1).contains(&hand), "invalid interaction hand");
        let shared = players
            .containers
            .get(&id)
            .context("native inventory missing")?
            .clone();
        let (empty, held) = shared.read(|m| -> Result<_> {
            Ok((
                m.inventory.selected_stack(0)?.is_empty()
                    && m.inventory.selected_stack(1)?.is_empty(),
                m.inventory.selected_stack(0)?.clone(),
            ))
        })??;
        if player.metadata.flags & 2 != 0 && !empty {
            return Ok(false);
        }
        players.require_current_native_source(&world)?;
        let WorldBackend::Fixture(flat) = &*world else {
            return Ok(false);
        };
        let key = flat
            .native_block_entities()
            .resolve(target, |p| Ok(flat.block(p.0, p.1, p.2)))?;
        if kind == ContainerKind::Chest {
            for p in &key.0 {
                if native_block_entities::static_conductor(flat.block(p.0, p.1 + 1, p.2))? {
                    return Ok(true);
                }
            }
        }
        ensure!(
            flat.native_block_entities()
                .can_unlock(&key, &held, player.gamemode == 3)?,
            "locked native container requires exact held custom name"
        );
        let title = flat.native_block_entities().title(&key)?;
        let top = flat.native_block_entities().read_slots(&key)?;
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
        // Opening an external menu does not invoke InventoryMenu.removed; its
        // retained cursor and 2x2 inputs remain separate from this menu.
        let next = players.native_containers.allocate_window(id);
        let mut menu = GenericContainerMenu::new(next, top.len() / 9)?;
        let initial = shared.read(|m| menu.full_sync(&top, &m.inventory))??;
        let key_stat = mc121_rust::player_stats::stat_key(
            "minecraft:custom",
            if kind == ContainerKind::Chest {
                "minecraft:open_chest"
            } else {
                "minecraft:open_barrel"
            },
        )
        .context("container stat missing")?;
        players
            .gameplay
            .stats_mut(player.profile.uuid)?
            .increment(key_stat, 1)?;
        let WorldBackend::Fixture(flat) = &mut *world else {
            unreachable!()
        };
        let effects =
            flat.native_block_entities_mut()
                .start_open(&key, id, player.gamemode == 3, now)?;
        players.native_containers.menus.insert(
            id,
            OpenNativeContainer {
                key,
                menu,
                spectator: player.gamemode == 3,
            },
        );
        self.emit_native_container_effects(&mut world, &mut players, effects, now, border)?;
        let mut w = Writer::new();
        w.varint(next as i32)
            .varint(if top.len() == 27 { 2 } else { 5 })
            .raw(&title.encode_network_anonymous()?);
        send_private(
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
    pub(super) async fn click_native_container(
        &self,
        id: i32,
        click: &ContainerClick,
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
        self.native_menu_action_locked(
            &mut world,
            &mut players,
            id,
            Some(click),
            None,
            false,
            now,
            border,
        )
    }
    pub(super) async fn close_native_container(
        &self,
        id: i32,
        window: u8,
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
        self.native_menu_action_locked(
            &mut world,
            &mut players,
            id,
            None,
            Some(window),
            send_close,
            now,
            border,
        )
    }
    pub(super) fn native_menu_action_locked(
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
        self.native_menu_action_with_close_context_locked(
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
    pub(super) fn native_menu_action_with_close_context_locked(
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
        let Some(open) = players.native_containers.menus.get(&id).cloned() else {
            return Ok(false);
        };
        if click.is_some_and(|c| c.window_id != open.menu.window_id) {
            return Ok(true);
        }
        if close.is_some_and(|w| w != open.menu.window_id) {
            return Ok(false);
        }
        let player = players
            .registry
            .get(id)
            .context("native container actor missing")?
            .clone();
        if click.is_some() && !still_valid(world, &player, &open.key)? {
            return self.native_menu_action_locked(
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
        let WorldBackend::Fixture(flat) = &*world else {
            bail!("native menu requires owned block entity backend")
        };
        let before = flat.native_block_entities().images();
        let top = flat.native_block_entities().read_slots(&open.key)?;
        let staged = RefCell::new((open, top));
        let world_slot = RefCell::new(world);
        let effects_slot = RefCell::new(Vec::new());
        let shared = players
            .containers
            .get(&id)
            .context("native shared inventory missing")?
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
            native_containers,
            entity_source,
            ..
        } = players;
        let close_packet = if send_close && close.is_some() {
            let mut w = Writer::new();
            w.u8(staged.borrow().0.menu.window_id);
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
                    .context("actual playerRNG absent")?,
                stats,
                entity_counter: &self.entities,
            },
            drop,
            |m| {
                let mut s = staged.borrow_mut();
                let (open, top) = &mut *s;
                let outcome = if close.is_some() {
                    open.menu.close_with_context(&mut m.inventory, close_context)?
                } else if open.spectator {
                    ClickOutcome {
                        packets: vec![open.menu.full_sync(top, &m.inventory)?],
                        ..Default::default()
                    }
                } else {
                    open.menu.click(
                        click.context("native click absent")?,
                        top,
                        &mut m.inventory,
                        player.gamemode == 1,
                    )?
                };
                if close.is_some() {
                    open.menu.transfer_inventory_remote_state(m)?;
                } else {
                    let _ = m.broadcast_changes()?;
                }
                Ok(outcome)
            },
            |_| {
                ensure!(rain_available, "native menu drop requires rain exposure");
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
                prepare: |m: &mut PlayerContainer, next_stats| {
                    let gameplay = gameplay;
                    let s = staged.borrow();
                    let w = world_slot.borrow();
                    let WorldBackend::Fixture(flat) = &**w else {
                        bail!("native container backend changed")
                    };
                    flat.native_block_entities()
                        .preflight_write_slots(&s.0.key, &s.1)?;
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
                        send_private(&self.entity_events, id, vec![packet.clone()]);
                    }
                    let s = staged.borrow();
                    let mut w = world_slot.borrow_mut();
                    let WorldBackend::Fixture(flat) = &mut **w else {
                        unreachable!()
                    };
                    flat.native_block_entities_mut()
                        .write_slots(&s.0.key, &s.1)
                        .expect("validated container slots under exclusive world lock");
                    if close.is_some() {
                        *effects_slot.borrow_mut() = flat
                            .native_block_entities_mut()
                            .stop_open(&s.0.key, id, s.0.spectator, now)
                            .expect("validated open-container owner");
                        native_containers.menus.remove(&id);
                    } else {
                        native_containers.menus.insert(id, s.0.clone());
                    }
                    if let Some(source) = entity_source.as_mut() {
                        source
                            .observe_owned_container_change(&w, &before)
                            .expect("validated native container source under exclusive world lock");
                    }
                },
                commit: |p: main_item_gameplay::PreparedGameplayMenu<'_>| p.commit(),
            },
        )?;
        let _ = staged.into_inner();
        let world = world_slot.into_inner();
        let effects = effects_slot.into_inner();
        self.emit_native_container_effects(world, players, effects, now, border)?;
        self.sync_native_containers_locked(world, players)?;
        Ok(true)
    }
    fn sync_native_containers_locked(
        &self,
        world: &WorldBackend,
        players: &mut LivePlayers,
    ) -> Result<()> {
        let WorldBackend::Fixture(flat) = world else {
            return Ok(());
        };
        for id in players.native_containers.pending_players() {
            let Some(shared) = players.containers.get(&id).cloned() else {
                continue;
            };
            let open = players.native_containers.menus.get_mut(&id).unwrap();
            let top = flat.native_block_entities().read_slots(&open.key)?;
            let packets = shared.read(|m| open.menu.broadcast_changes(&top, &m.inventory))??;
            send_private(&self.entity_events, id, packets);
        }
        Ok(())
    }
    fn emit_native_container_effects(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        effects: Vec<ContainerEffect>,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        for effect in effects {
            match effect {
                ContainerEffect::ScheduleBlockTick {
                    position,
                    state,
                    delay,
                } => {
                    players.schedule_native_block_tick(
                        world,
                        &mc121_rust::placement::PlannedBlockTick {
                            position,
                            state,
                            delay: delay as i32,
                        },
                        now,
                    )?;
                }
                ContainerEffect::Sound {
                    origin,
                    sound,
                    volume,
                } => {
                    let sound_id = *native_container_facts()
                        .sounds
                        .get(sound)
                        .context("native container sound absent")?;
                    let pitch = players.item_runtime.world_random_mut().next_f32() * 0.1 + 0.9;
                    let seed = players.item_runtime.next_sound_seed()?;
                    let mut w = Writer::new();
                    w.varint(sound_id + 1)
                        .varint(4)
                        .i32((origin.x * 8.) as i32)
                        .i32((origin.y * 8.) as i32)
                        .i32((origin.z * 8.) as i32)
                        .f32(volume)
                        .f32(pitch)
                        .i64(seed);
                    let recipients = players
                        .registry
                        .snapshot()
                        .into_iter()
                        .filter(|p| {
                            let d = Vec3::new(p.position.x, p.position.y, p.position.z)
                                .subtract(origin);
                            d.x * d.x + d.y * d.y + d.z * d.z < 256.
                        })
                        .map(|p| p.entity_id)
                        .collect();
                    let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                        recipients: Arc::new(recipients),
                        packets: Arc::new(vec![Packet {
                            id: 0x68,
                            payload: w.into_inner(),
                        }]),
                    });
                }
                ContainerEffect::BlockEvent {
                    position,
                    state,
                    id,
                    value,
                } => {
                    let kind = native_block_entities::container_kind(state)
                        .context("block event owner absent")?;
                    let block = *native_container_facts()
                        .block_ids
                        .get(kind.name())
                        .context("block event ID absent")?;
                    let mut w = Writer::new();
                    w.i64(pack_position(position.0, position.1, position.2))
                        .u8(id as u8)
                        .u8(value as u8)
                        .varint(block);
                    let packet = Packet {
                        id: 0x08,
                        payload: w.into_inner(),
                    };
                    let recipients = players
                        .registry
                        .snapshot()
                        .into_iter()
                        .filter(|p| {
                            let dx = p.position.x - f64::from(position.0);
                            let dy = p.position.y - f64::from(position.1);
                            let dz = p.position.z - f64::from(position.2);
                            dx * dx + dy * dy + dz * dz < 4096.
                        })
                        .map(|p| p.entity_id)
                        .collect();
                    let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                        recipients: Arc::new(recipients),
                        packets: Arc::new(vec![packet]),
                    });
                }
                ContainerEffect::SetOpen {
                    position,
                    state,
                    open,
                } => {
                    let next = inventory::block_state_with_properties(
                        state,
                        &BTreeMap::from([("open".into(), open.to_string())]),
                    )?;
                    let plan = {
                        let mut query =
                            mc121_rust::world_interactions::RuntimeInteractionWorld::new(
                                world,
                                mc121_rust::world::MIN_Y,
                                mc121_rust::world::MAX_Y + 1,
                                border,
                                |_, _| Ok(true),
                            )?;
                        mc121_rust::world_interactions::plan_edit(position, next, 3, &mut query)?
                    };
                    ensure!(
                        self.commit_world_interaction(world, players, -1, plan)?
                            == main_world_interactions::MainInteractionResult::Applied,
                        "native barrel state rejected"
                    );
                }
                ContainerEffect::GameEvent { position, name } => {
                    let radius = mc121_rust::world_interactions::game_event_radius(name)?;
                    players.chunk_runtime.certify_query_region(
                        world,
                        &mut players.chunk_transports,
                        &self.entity_events,
                        now,
                        (
                            (position.0 - radius).div_euclid(16),
                            (position.2 - radius).div_euclid(16),
                        ),
                        (
                            (position.0 + radius).div_euclid(16),
                            (position.2 + radius).div_euclid(16),
                        ),
                    )?;
                    let mut frame = players.gameplay.frame(
                        &players.registry,
                        &players.chunk_runtime.manager,
                        true,
                    );
                    mc121_rust::game_event_context::post_to_empty_registries(
                        &mut frame,
                        mc121_rust::game_event_context::GameEventPost {
                            position: Vec3::new(
                                f64::from(position.0) + 0.5,
                                f64::from(position.1) + 0.5,
                                f64::from(position.2) + 0.5,
                            ),
                            notification_radius: radius,
                        },
                    )?;
                }
            }
        }
        let report =
            players.drain_world_interactions(world, now, border, true, &self.entity_events, 4096);
        ensure!(
            report.blocked.is_none(),
            "native container callback pending: {:?}",
            report.blocked
        );
        Ok(())
    }
    pub(super) fn close_native_containers_for_save_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        now: i64,
    ) -> Result<()> {
        let border = self.native_container_border()?;
        for id in players.native_containers.pending_players() {
            let window = players.native_containers.window_id(id).unwrap();
            self.native_menu_action_locked(
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
    /// Invoked by the canonical global BLOCK queue, never by a second timer loop.
    pub(super) fn tick_native_container_block_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        p: (i32, i32, i32),
        state: u32,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        let actual = players
            .native_containers
            .menus
            .iter()
            .filter(|(id, m)| m.key.0.contains(&p) && players.registry.get(**id).is_some())
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let WorldBackend::Fixture(flat) = world else {
            bail!("scheduled container requires actual native owner")
        };
        flat.native_block_entities().validate_at(p, state)?;
        let effects = flat
            .native_block_entities_mut()
            .recheck_at(p, now, actual)?;
        self.emit_native_container_effects(world, players, effects, now, border)
    }
    pub(super) async fn tick_native_containers(&self) -> Result<()> {
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
        for id in players.native_containers.pending_players() {
            if let (Some(player), Some(open)) = (
                players.registry.get(id).cloned(),
                players.native_containers.menus.get(&id).cloned(),
            ) {
                if !still_valid(&mut world, &player, &open.key)? {
                    self.native_menu_action_locked(
                        &mut world,
                        &mut players,
                        id,
                        None,
                        Some(open.menu.window_id),
                        true,
                        now,
                        border,
                    )?;
                }
            }
        }
        self.sync_native_containers_locked(&world, &mut players)
    }
}
