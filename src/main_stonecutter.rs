//! Canonical active StonecutterMenu transactions and ordered real item drops.
use super::*;
use main_item_gameplay::MainItemGameplay;
use main_stonecutter_recipes::{
    NativeStonecutterRecipes, PreparedStonecutterCallbacks, stonecutter_sound_packet,
};
use mc121_rust::{
    container::MenuCloseContext,
    container_workstation_stonecutter::{StonecutterMenu, StonecutterMenuEvent, StonecutterPhase, StonecutterPublication},
    inventory::PlayerInventory,
    item_entity::{spawn_player_drop_with_factory, PlayerDrop},
    player_stats::{DropStatEvent, PlayerStats},
    runtime_entities::{ItemBatchPublication, PreparedItemBatch},
    workstations::menu::SyncEvent,
};

#[derive(Clone, Debug)]
pub(super) struct OpenStonecutter {
    pub(super) position: (i32, i32, i32),
    pub(super) menu: StonecutterMenu,
}

#[derive(Default)]
pub(super) struct StonecutterRuntime {
    pub(super) menus: BTreeMap<i32, OpenStonecutter>,
}

impl StonecutterRuntime {
    pub(super) fn window_id(&self, id: i32) -> Option<u8> {
        self.menus.get(&id).map(|open| open.menu.window_id())
    }
    pub(super) fn has_open(&self, id: i32) -> bool { self.menus.contains_key(&id) }
    pub(super) fn pending_players(&self) -> Vec<i32> { self.menus.keys().copied().collect() }
    pub(super) fn preflight_close_return_stacks(&self, id: i32,
        check: &mut dyn FnMut(&ItemStack) -> Result<()>) -> Result<()> {
        if let Some(open) = self.menus.get(&id) {
            check(open.menu.carried())?;
            check(open.menu.state().input())?;
        }
        Ok(())
    }
    pub(super) fn inventory_snapshot(&self, id: i32, inventory: &PlayerInventory) -> PlayerInventory {
        self.menus.get(&id).map_or_else(
            || inventory.clone(),
            |open| inventory.snapshot_with_carried(open.menu.carried()),
        )
    }
}

pub(super) enum StonecutterRequest<'a> {
    Click(&'a ContainerClick),
    Button(i32),
    Close { window: u8, send_close: bool, context: MenuCloseContext },
    /// ServerPlayer.tick broadcasts before its subsequent validity/close gate.
    Broadcast,
    /// Repair rejected client prediction through this retained menu's real
    /// full-state synchronization path, without executing another click.
    Resync,
}

/// Reuse the existing admitted native body contract for mutable workstation
/// actions. Spectators only receive the source's read-only menu/resync path.
pub(super) fn require_stonecutter_actor(players: &LivePlayers, id: i32) -> Result<()> {
    // The captured reach calculation is valid only for the actual supported
    // attribute profile; unrelated/inactive inventory modifiers are permitted.
    main_stonecutter_dispatch::require_stonecutter_reach_profile(players, id)?;
    let player = players.registry.get(id).context("stonecutter actor missing")?;
    ensure!(matches!(player.entity_data.get(9), Some(MetadataValue::Float(health)) if health.is_finite() && *health > 0.),
        "stonecutter requires an actual alive player");
    let lifecycle = players.player_lifecycle.get(&player.profile.uuid)
        .context("stonecutter player lifecycle missing")?;
    ensure!(lifecycle.dimension == "minecraft:overworld", "stonecutter level/dimension profile unavailable");
    if player.gamemode != 3 {
        main_player_motion_hooks::require_creative(player, lifecycle)?;
        let physical = players.player_motion_runtime.physical(id)
            .context("stonecutter native player body missing")?;
        ensure!(!physical.removed && physical.vehicle.is_none(), "stonecutter native body profile changed");
    }
    Ok(())
}

fn publish(events: &broadcast::Sender<PlayerEvent>, id: i32, packets: Vec<Packet>) {
    if !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![id]), packets: Arc::new(packets),
        });
    }
}

/// The source kernel emits each Inventory.setChanged boundary but leaves the
/// borrowed owner's transient counter to Main. Change only these observation
/// counters: slots, raw empty stacks and both cursor owners stay untouched.
fn project_inventory_counter(events: &mut [StonecutterMenuEvent], inventory: &mut PlayerInventory, before: i32) -> Result<()> {
    ensure!(inventory.get_times_changed() == before, "stonecutter kernel counter ownership changed");
    let mut preceding = 0_usize;
    for event in events {
        let marker = matches!(event, StonecutterMenuEvent::PlayerInventoryChanged { .. });
        let phase = match event {
            StonecutterMenuEvent::Sync { phase, .. } | StonecutterMenuEvent::Effect { phase, .. }
            | StonecutterMenuEvent::InputChanged { phase } | StonecutterMenuEvent::PlayerInventoryChanged { phase }
            | StonecutterMenuEvent::Drop { phase, .. } | StonecutterMenuEvent::ItemCraftedBeforeQuickMove { phase, .. } => phase,
        };
        ensure!(phase.inventory.get_times_changed() == before, "stonecutter phase counter requires a new kernel handoff");
        for _ in 0..preceding { phase.inventory.set_changed(); }
        if marker { inventory.set_changed(); preceding += 1; }
    }
    Ok(())
}

struct PreparedStonecutterCommit<'a> {
    items: PreparedItemBatch<'a>,
    player_random: &'a mut LegacyRandomSource,
    next_player_random: LegacyRandomSource,
    next_world_random: LegacyRandomSource,
    entity_counter: &'a AtomicI32,
    next_entity_id: i32,
    gameplay: &'a mut MainItemGameplay,
    recipe_book: &'a mut ServerRecipeBook,
    menus: &'a mut BTreeMap<i32, OpenStonecutter>,
    next_open: OpenStonecutter,
    close: bool,
    close_packet: Option<Packet>,
    publication: StonecutterPublication,
    callbacks: PreparedStonecutterCallbacks,
    stats_after_event: Vec<PlayerStats>,
    drop_ordinals: Vec<Option<usize>>,
    sound_recipients: Arc<Vec<i32>>,
    final_inventory: PlayerInventory,
}

impl PreparedStonecutterCommit<'_> {
    fn commit(self, id: i32, uuid: [u8; 16], events: &broadcast::Sender<PlayerEvent>, canonical_menu: &mut PlayerContainer) {
        let Self {
            items, player_random, next_player_random, next_world_random,
            entity_counter, next_entity_id, gameplay, recipe_book, menus,
            next_open, close, close_packet, publication, callbacks,
            stats_after_event, drop_ordinals, sound_recipients, final_inventory,
        } = self;
        *player_random = next_player_random;
        entity_counter.store(next_entity_id, Ordering::Relaxed);
        let position = next_open.position;
        if close { menus.remove(&id); } else { menus.insert(id, next_open); }
        if let Some(packet) = close_packet { publish(events, id, vec![packet]); }

        // The actual prepared store owns every ItemEntity and publishes it at
        // its indexed Drop event. Menu packets and callbacks run between those
        // insertions through the same existing FIFO, not a second item outbox.
        let mut cursor = 0;
        let mut pending_drop: Option<usize> = None;
        items.commit_with_world_random_ordered(next_world_random,
            |ordinal: usize, item_publication: &mut ItemBatchPublication<'_>| {
                if let Some(index) = pending_drop.take() {
                    // ServerPlayer.drop awards only after actual world-add.
                    gameplay.replace_player_stats(uuid, stats_after_event[index].clone()).expect("validated workstation scoreboard/statistic commit");
                    cursor = index + 1;
                }
                while cursor < publication.events.len() {
                    let index = cursor;
                    let event = &publication.events[index];
                    canonical_menu.inventory = main_stonecutter_recipes::stonecutter_event_phase(event).inventory.clone();
                    if let StonecutterMenuEvent::Drop { index: drop_index, .. } = event {
                        if let Some(expected) = drop_ordinals[*drop_index] {
                            assert_eq!(expected, ordinal, "validated indexed drop publication order");
                            pending_drop = Some(index);
                            return;
                        }
                    }
                    gameplay.replace_player_stats(uuid, stats_after_event[index].clone()).expect("validated workstation scoreboard/statistic commit");
                    if let Some(book) = callbacks.book_at(index) { *recipe_book = book.clone(); }
                    match event {
                        StonecutterMenuEvent::Sync { event: SyncEvent::SlotListener {
                            player_slot: Some(slot), stack, ..
                        }, phase } => {
                            gameplay.inventory_slot_changed(uuid, *slot, &phase.inventory, stack)
                                .expect("validated native empty-criteria inventory listener");
                        }
                        StonecutterMenuEvent::Sync { event: SyncEvent::Packet(packet), .. } => {
                            publish(events, id, vec![packet.clone()]);
                        }
                        StonecutterMenuEvent::PlayerInventoryChanged { .. } => {
                            canonical_menu.inventory.set_changed();
                        }
                        StonecutterMenuEvent::InputChanged { .. } => {
                            callbacks.complete_input_change_at(index)
                                .expect("validated original constructor-default input listener");
                        }
                        _ => {}
                    }
                    publish(events, id, callbacks.packets_at(index).to_vec());
                    if callbacks.is_sound_at(index) {
                        let seed = item_publication.next_sound_seed()
                            .expect("validated actual Level.threadSafeRandom owner");
                        let packet = stonecutter_sound_packet(position, seed);
                        if !sound_recipients.is_empty() {
                            let _ = events.send(PlayerEvent::WorldAudience {
                                recipients: Arc::clone(&sound_recipients),
                                packets: Arc::new(vec![packet]),
                            });
                        }
                    }
                    cursor += 1;
                }
            });
        assert!(pending_drop.is_none() && cursor == publication.events.len());
        assert_eq!(canonical_menu.inventory.get_times_changed(), final_inventory.get_times_changed(),
            "stonecutter source counter timeline differs from its prepared owner");
        canonical_menu.inventory = final_inventory;
        *recipe_book = callbacks.into_book();
    }
}

impl Server {
    pub(super) fn resync_native_stonecutter_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        match self.stonecutter_action_locked(world, players, id, StonecutterRequest::Resync, now, border) {
            Err(error) if error.downcast_ref::<main_stonecutter_dispatch::UnsupportedMenuReach>().is_some() => {
                let window = players.stonecutters.window_id(id)
                    .context("unsupported stonecutter resync profile lost its actual menu")?;
                ensure!(self.close_stonecutter_with_context_locked(
                    world, players, id, window, true, now, border, MenuCloseContext::AliveConnected,
                )?, "unsupported stonecutter resync profile changed its captured menu");
                eprintln!("{}", json!({
                    "event":"native_menu_profile_retired", "menu":"stonecutter",
                    "entity_id":id, "native_limit":"unsupported_block_interaction_range",
                    "trigger":"component_refusal_resync", "reason":error.to_string(),
                }));
                Ok(())
            }
            result => result,
        }
    }

    pub(super) fn stonecutter_action_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        request: StonecutterRequest<'_>,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        let Some(open) = players.stonecutters.menus.get(&id).cloned() else { return Ok(()); };
        if matches!(&request, StonecutterRequest::Click(click) if click.window_id != open.menu.window_id())
            || matches!(&request, StonecutterRequest::Close { window, .. } if *window != open.menu.window_id()) {
            return Ok(());
        }
        let player = players.registry.get(id).context("stonecutter actor missing")?.clone();
        if !matches!(request, StonecutterRequest::Close { .. }) {
            require_stonecutter_actor(players, id)?;
        }
        let spectator = player.gamemode == 3;
        if matches!(request, StonecutterRequest::Button(_)) && spectator { return Ok(()); }
        if matches!(request, StonecutterRequest::Click(_) | StonecutterRequest::Button(_))
            && !spectator
            && !open.menu.still_valid(&main_stonecutter_dispatch::stonecutter_access(world, &player, open.position)?)? {
            // The packet handler ignores invalid menu access. Its later real
            // player phase broadcasts, checks validity and closes the menu.
            return Ok(());
        }
        if !spectator && matches!(&request, StonecutterRequest::Click(click) if click.slot >= 38) { return Ok(()); }
        players.require_current_native_source(world)?;
        players.gameplay.require_supported_floor()?;
        let recipes = NativeStonecutterRecipes::new(&self.fixtures, &players.gameplay)?;
        let shared = players.containers.get(&id).context("stonecutter canonical inventory missing")?.clone();
        let stats = players.gameplay.snapshot_player_stats(player.profile.uuid)?;
        let facts = players.world_context.item_facts(now, border, false)?;
        let rain_available = matches!(facts.rain, mc121_rust::runtime_item_world::RainAvailability::NotRaining)
            || players.rain_context.is_some();
        let drop_context = PlayerDrop {
            player_uuid: player.profile.uuid, position: player.position,
            eye_height: player_physics::player_dimensions(player.metadata.pose, 1.)?.eye_height,
            around: false, trace_item: false,
        };
        let sound_origin = Vec3::new(open.position.0 as f64 + 0.5, open.position.1 as f64 + 0.5, open.position.2 as f64 + 0.5);
        let sound_recipients = Arc::new(players.registry.snapshot().into_iter().filter(|other| {
            Vec3::new(other.position.x, other.position.y, other.position.z)
                .subtract(sound_origin).length_squared() < 256.
        }).map(|other| other.entity_id).collect::<Vec<_>>());
        let close = matches!(request, StonecutterRequest::Close { .. });
        let close_packet = match &request {
            StonecutterRequest::Close { send_close: true, .. } => {
                let mut fields = Writer::new(); fields.u8(open.menu.window_id());
                Some(Packet { id: 0x12, payload: fields.into_inner() })
            }
            _ => None,
        };
        let LivePlayers {
            item_runtime, player_random, gameplay, recipe_books, stonecutters,
            chunk_runtime, chunk_transports, ..
        } = players;
        let actual_player_random = player_random.get_mut(&id).context("stonecutter player RNG missing")?;
        let book = recipe_books.get_mut(&player.profile.uuid).context("stonecutter actual recipe book missing")?;
        shared.transact_with_menu_commit(move |menu| {
            let counter_before = menu.inventory.get_times_changed();
            let mut next_open = open;
            let full_state = matches!(&request, StonecutterRequest::Resync)
                || spectator && matches!(&request, StonecutterRequest::Click(_));
            let mut publication = match request {
                StonecutterRequest::Broadcast => StonecutterPublication {
                    events: next_open.menu.broadcast_changes(&menu.inventory)?,
                    outcome: ClickOutcome::default(),
                },
                _ if full_state => {
                    let phase = StonecutterPhase {
                        input: next_open.menu.state().input().clone(),
                        result_preview: next_open.menu.state().result_preview().clone(),
                        selected_recipe_index: next_open.menu.state().selected_recipe_index(),
                        inventory: menu.inventory.clone(),
                        carried: next_open.menu.carried().clone(),
                    };
                    StonecutterPublication {
                        events: next_open.menu.broadcast_full_state(&menu.inventory)?
                            .into_iter().map(|event| StonecutterMenuEvent::Sync { event, phase: phase.clone() }).collect(),
                        outcome: ClickOutcome::default(),
                    }
                }
                action => {
                    let plan = match action {
                        StonecutterRequest::Click(click) => next_open.menu.prepare_click(click, &menu.inventory, &recipes, now, player.gamemode == 1)?,
                        StonecutterRequest::Button(button) => next_open.menu.prepare_button(button, &menu.inventory, &recipes)?,
                        StonecutterRequest::Close { context, .. } => next_open.menu.prepare_close(&menu.inventory, context)?,
                        StonecutterRequest::Broadcast | StonecutterRequest::Resync => unreachable!(),
                    };
                    let current = stonecutters.menus.get(&id).context("stonecutter menu disappeared")?;
                    plan.validate(&current.menu, &menu.inventory)?;
                    // Every item/recipe/callback capability is checked against
                    // the actual current owners before consuming the plan.
                    let _ = PreparedStonecutterCallbacks::prepare(gameplay, book, plan.events())?;
                    for drop in plan.drops() { drop.stack.encode_canonical()?; }
                    plan.validate(&current.menu, &menu.inventory)?;
                    plan.commit(&mut next_open.menu, &mut menu.inventory)?
                }
            };
            project_inventory_counter(&mut publication.events, &mut menu.inventory, counter_before)?;
            ensure!(publication.outcome.packets.is_empty(), "stonecutter has two packet publication owners");
            ensure!(publication.outcome.crafted.is_empty(), "stonecutter crafted callbacks require their indexed source events");
            let callbacks = PreparedStonecutterCallbacks::prepare(gameplay, book, &publication.events)?;
            callbacks.validate_book(book)?;
            // This validates each player-backed source listener and all 41
            // backing-stack queries in its actual phase before commit.
            let _ = main_stonecutter_recipes::stonecutter_local_changes(&publication.events)?;
            if publication.events.iter().enumerate().any(|(index, _)| callbacks.is_sound_at(index)) {
                ensure!(item_runtime.sound_random_state48().is_some(), "stonecutter level sound RNG missing");
            }
            let mut seen = BTreeSet::new();
            let mut drop_order = Vec::new();
            let mut drop_ordinals = vec![None; publication.outcome.drops.len()];
            let mut next_stats = stats;
            let mut stats_after_event = Vec::with_capacity(publication.events.len());
            for (index, event) in publication.events.iter().enumerate() {
                callbacks.apply_stat_at(index, &mut next_stats)?;
                if matches!(event, StonecutterMenuEvent::InputChanged { .. }) {
                    callbacks.complete_input_change_at(index)?;
                }
                if let StonecutterMenuEvent::Drop { index, .. } = event {
                    ensure!(*index < publication.outcome.drops.len() && seen.insert(*index), "invalid repeated stonecutter drop index");
                    let drop = &publication.outcome.drops[*index];
                    if !drop.stack.is_empty() {
                        drop_ordinals[*index] = Some(drop_order.len());
                        drop_order.push(*index);
                        next_stats.award_drop(DropStatEvent {
                            constructed: true, trace_item: drop.trace_item,
                            returned_item_id: Some(drop.stack.item_id), original_count: drop.stack.count,
                        })?;
                    }
                }
                stats_after_event.push(next_stats.clone());
            }
            ensure!(seen.len() == publication.outcome.drops.len(), "unpublished stonecutter owned drop");
            if !drop_order.is_empty() {
                ensure!(rain_available, "stonecutter drop requires actual rain context");
                chunk_runtime.certify_current_query_region(world, chunk_transports, &self.entity_events, now,
                    (((player.position.x - 18.).floor() as i32).div_euclid(16), ((player.position.z - 18.).floor() as i32).div_euclid(16)),
                    (((player.position.x + 18.).floor() as i32).div_euclid(16), ((player.position.z + 18.).floor() as i32).div_euclid(16)))?;
            }
            if close { next_open.menu.transfer_inventory_state(menu)?; }
            let mut next_player_random = actual_player_random.clone();
            let mut next_world_random = item_runtime.world_random_mut().clone();
            let first_id = self.entities.load(Ordering::Relaxed);
            let next_entity_id = first_id.checked_add(i32::try_from(drop_order.len())?).context("stonecutter drop entity IDs exhausted")?;
            // Transfer each actual prepared owned stack once. Event snapshots
            // and derived result previews never become another ItemEntity.
            let mut owned_drops = std::mem::take(&mut publication.outcome.drops)
                .into_iter().map(Some).collect::<Vec<_>>();
            let mut entries = Vec::with_capacity(drop_order.len());
            for (ordinal, index) in drop_order.into_iter().enumerate() {
                let dropped = owned_drops[index].take().expect("validated one owned drop");
                let context = PlayerDrop { around: dropped.around, trace_item: dropped.trace_item, ..drop_context };
                let entity_id = first_id.checked_add(i32::try_from(ordinal)?).context("stonecutter drop entity ID exhausted")?;
                let entry = spawn_player_drop_with_factory(entity_id, context, dropped.stack,
                    &mut next_player_random, &mut next_world_random,
                    &mut || random_seed::create_random().map_err(anyhow::Error::from))?
                    .context("nonempty stonecutter drop did not construct an item")?;
                entries.push(entry);
            }
            callbacks.validate_book(book)?;
            let final_inventory = menu.inventory.clone();
            let items = item_runtime.prepare_items(entries)?;
            Ok((PreparedStonecutterCommit {
                items, player_random: actual_player_random, next_player_random, next_world_random,
                entity_counter: &self.entities, next_entity_id, gameplay, recipe_book: book,
                menus: &mut stonecutters.menus, next_open, close, close_packet, publication,
                callbacks, stats_after_event, drop_ordinals, sound_recipients, final_inventory,
            }, vec![]))
        }, |prepared, menu| prepared.commit(id, player.profile.uuid, &self.entity_events, menu))
    }
}
