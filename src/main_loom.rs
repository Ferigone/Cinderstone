//! Bound native LoomMenu transactions, source callbacks and ordered real drops.
//! Main must supply its actual operation/world/player locks; no detached host
//! or replacement inventory is installed by this module.
use super::*;
use crate::{
    main_item_gameplay::MainItemGameplay,
    main_loom_callbacks::{
        loom_event_phase, loom_local_changes, loom_sound_packet, PreparedLoomCallbacks,
    },
    main_loom_dispatch::{ActualLoomAccess, loom_access, loom_unambiguous_drag_order},
    main_loom_patterns::{LoomPatternBinding, NativeLoomPatterns},
};
use mc121_rust::{
    container::{ClickOutcome, MenuCloseContext},
    container_workstation_loom::{LoomMenu, LoomMenuContext, LoomMenuEvent, LoomPhase, LoomPublication},
    inventory::{ItemStack, PlayerInventory},
    item_entity::{spawn_player_drop_with_factory, PlayerDrop},
    player_stats::{DropStatEvent, PlayerStats},
    runtime_entities::{ItemBatchPublication, PickupPlayer, PreparedItemBatch},
    workstations::{
        loom::{BannerPattern, LoomPatterns},
        menu::{SyncEvent, WorkstationAccess, WorkstationKind},
    },
};

/// Only this native factory can mint the verified original default-listener
/// environment. Java does have registerUpdateListener; arbitrary replacements
/// have no capability in this server's supported source profile.
#[derive(Clone, Debug, PartialEq, Eq)]
enum NativeLoomCallbacks { OriginalServerDefault }

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeLoomBinding {
    level_owner: usize,
    entity_id: i32,
    uuid: [u8; 16],
    dimension: String,
    position: (i32, i32, i32),
    patterns: LoomPatternBinding,
    callbacks: NativeLoomCallbacks,
}

/// Captures actual observations under the level/player locks. The retained
/// context may be reused by synchronization alone, which queries neither
/// reach nor game time. Actions refresh these observations from their owners.
#[derive(Clone, Debug)]
pub(super) struct NativeLoomContext {
    binding: NativeLoomBinding,
    patterns: NativeLoomPatterns,
    access: ActualLoomAccess,
    game_time: i64,
}

impl NativeLoomContext {
    fn capture(
        server: &Server, world: &mut WorldBackend, players: &LivePlayers,
        id: i32, position: (i32, i32, i32), now: i64,
    ) -> Result<Self> {
        players.gameplay.require_supported_floor()?;
        let player = players.registry.get(id).context("loom context actor missing")?;
        let lifecycle = players.player_lifecycle.get(&player.profile.uuid)
            .context("loom context actual dimension owner missing")?;
        let patterns = NativeLoomPatterns::new(&server.fixtures)?;
        Ok(Self {
            binding: NativeLoomBinding {
                // The actual stable level-lock owner, not a menu counter or
                // a hash advertised as an independently owned Java level.
                level_owner: std::ptr::from_ref(&server.world) as usize,
                entity_id: id, uuid: player.profile.uuid,
                dimension: lifecycle.dimension.clone(), position,
                patterns: patterns.binding(),
                callbacks: NativeLoomCallbacks::OriginalServerDefault,
            },
            patterns, access: loom_access(world, player, position)?, game_time: now,
        })
    }

    fn refresh(
        &self, server: &Server, world: &mut WorldBackend, players: &LivePlayers,
        id: i32, now: i64, closing: bool,
    ) -> Result<Self> {
        let mut next = Self::capture(server, world, players, id, self.binding.position, now)?;
        if closing {
            // Removal belongs to the captured old level even during player
            // lifecycle cleanup. Do not test stillValid or invent reach here.
            next.binding.dimension = self.binding.dimension.clone();
        }
        ensure!(next.binding == self.binding, "loom actual captured owners changed");
        Ok(next)
    }

    pub(super) fn patterns(&self) -> &NativeLoomPatterns { &self.patterns }

    pub(super) fn require_default_callbacks(&self) -> Result<()> {
        ensure!(self.binding.callbacks == NativeLoomCallbacks::OriginalServerDefault
            && self.binding.patterns == self.patterns.binding(),
            "loom source callback/configuration capability changed");
        Ok(())
    }

    pub(super) fn verify_pickup_actor(&self, player: PickupPlayer) -> Result<()> {
        ensure!(player.entity_id == self.binding.entity_id && player.uuid == self.binding.uuid,
            "loom pickup callback belongs to another actor");
        ensure!(player.alive && !player.spectator && player.vehicle_bounds.is_none(),
            "loom pickup requires the actual admitted alive unmounted player");
        self.require_default_callbacks()
    }
}

impl LoomPatterns for NativeLoomContext {
    fn is_pattern_item(&self, stack: &ItemStack) -> bool { self.patterns.is_pattern_item(stack) }
    fn selectable_patterns(&self, stack: &ItemStack) -> Result<Vec<BannerPattern>> {
        self.patterns.selectable_patterns(stack)
    }
}
impl WorkstationAccess for NativeLoomContext {
    fn block_matches(&self, kind: WorkstationKind) -> Result<bool> { self.access.block_matches(kind) }
    fn player_can_interact_with_block(&self, additional_range: f64) -> Result<bool> {
        self.access.player_can_interact_with_block(additional_range)
    }
}
impl LoomMenuContext for NativeLoomContext {
    type Binding = NativeLoomBinding;
    fn binding(&self) -> Result<Self::Binding> { Ok(self.binding.clone()) }
    fn source_callbacks_supported(&self) -> Result<bool> {
        self.require_default_callbacks()?;
        Ok(true)
    }
    fn world_game_time(&self) -> Result<i64> { Ok(self.game_time) }
    fn quickcraft_slot_order(&self, targets: &BTreeSet<usize>) -> Result<Vec<usize>> {
        loom_unambiguous_drag_order(targets)
    }
}

#[derive(Clone, Debug)]
pub(super) struct OpenLoom {
    pub(super) position: (i32, i32, i32),
    pub(super) menu: LoomMenu<NativeLoomBinding>,
    pub(super) context: NativeLoomContext,
}
#[derive(Default)]
pub(super) struct LoomRuntime { pub(super) menus: BTreeMap<i32, OpenLoom> }
impl LoomRuntime {
    pub(super) fn window_id(&self, id: i32) -> Option<u8> {
        self.menus.get(&id).map(|open| open.menu.window_id())
    }
    pub(super) fn has_open(&self, id: i32) -> bool { self.menus.contains_key(&id) }
    pub(super) fn pending_players(&self) -> Vec<i32> { self.menus.keys().copied().collect() }
    pub(super) fn preflight_close_return_stacks(&self, id: i32,
        check: &mut dyn FnMut(&ItemStack) -> Result<()>) -> Result<()> {
        if let Some(open) = self.menus.get(&id) {
            check(open.menu.carried())?;
            for stack in open.menu.state().inputs() { check(stack)?; }
        }
        Ok(())
    }
    pub(super) fn inventory_snapshot(&self, id: i32, inventory: &PlayerInventory) -> PlayerInventory {
        self.menus.get(&id).map_or_else(|| inventory.clone(), |open| {
            inventory.snapshot_with_carried(open.menu.carried())
        })
    }
}
pub(super) enum LoomRequest<'a> {
    Click(&'a ContainerClick), Button(i32),
    Close { window: u8, send_close: bool, context: MenuCloseContext },
    Broadcast, Resync,
}

pub(super) fn require_loom_actor(players: &LivePlayers, id: i32) -> Result<()> {
    crate::main_stonecutter::require_stonecutter_actor(players, id)
}

fn phase(open: &OpenLoom, inventory: &PlayerInventory) -> LoomPhase {
    LoomPhase {
        inputs: open.menu.state().inputs().clone(),
        result_preview: open.menu.state().result().clone(),
        selected_pattern: open.menu.state().selected_pattern(),
        selectable_patterns: open.menu.state().selectable_patterns().to_vec(),
        player_inventory: inventory.clone(), carried: open.menu.carried().clone(),
    }
}
fn publish(events: &broadcast::Sender<PlayerEvent>, id: i32, packets: Vec<Packet>) {
    if !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![id]), packets: Arc::new(packets),
        });
    }
}

/// Preserve the kernel's source images while completing the transient counter
/// obligations owned by Main, without replacing a slot/cursor or firing hooks.
fn project_inventory_counter(events: &mut [LoomMenuEvent], inventory: &mut PlayerInventory, before: i32) -> Result<()> {
    ensure!(inventory.get_times_changed() == before, "loom kernel counter ownership changed");
    let mut preceding = 0_usize;
    for event in events {
        let marker = matches!(event, LoomMenuEvent::PlayerInventoryChanged { .. });
        let phase = match event {
            LoomMenuEvent::Source { phase, .. } | LoomMenuEvent::PlayerInventoryChanged { phase }
            | LoomMenuEvent::Sync { phase, .. } | LoomMenuEvent::TakeSound { phase, .. }
            | LoomMenuEvent::Drop { phase, .. } => phase,
        };
        ensure!(phase.player_inventory.get_times_changed() == before, "loom phase counter requires a new kernel handoff");
        for _ in 0..preceding { phase.player_inventory.set_changed(); }
        if marker { inventory.set_changed(); preceding += 1; }
    }
    Ok(())
}

struct PreparedLoomCommit<'a> {
    items: PreparedItemBatch<'a>,
    player_random: &'a mut LegacyRandomSource,
    next_player_random: LegacyRandomSource,
    next_world_random: LegacyRandomSource,
    entity_counter: &'a AtomicI32,
    next_entity_id: i32,
    gameplay: &'a mut MainItemGameplay,
    menus: &'a mut BTreeMap<i32, OpenLoom>,
    next_open: OpenLoom,
    close: bool,
    close_packet: Option<Packet>,
    publication: LoomPublication,
    callbacks: PreparedLoomCallbacks,
    stats_after_event: Vec<PlayerStats>,
    drop_ordinals: Vec<Option<usize>>,
    sound_recipients: Arc<Vec<i32>>,
    final_inventory: PlayerInventory,
}
impl PreparedLoomCommit<'_> {
    fn commit(self, id: i32, uuid: [u8; 16], events: &broadcast::Sender<PlayerEvent>, canonical_menu: &mut PlayerContainer) {
        let Self {
            items, player_random, next_player_random, next_world_random,
            entity_counter, next_entity_id, gameplay, menus, next_open,
            close, close_packet, publication, callbacks, stats_after_event,
            drop_ordinals, sound_recipients, final_inventory,
        } = self;
        *player_random = next_player_random;
        entity_counter.store(next_entity_id, Ordering::Relaxed);
        let position = next_open.position;
        if close { menus.remove(&id); } else { menus.insert(id, next_open); }
        if let Some(packet) = close_packet { publish(events, id, vec![packet]); }
        let mut cursor = 0;
        let mut pending_drop: Option<usize> = None;
        items.commit_with_world_random_ordered(next_world_random,
            |ordinal: usize, item_publication: &mut ItemBatchPublication<'_>| {
                if let Some(index) = pending_drop.take() {
                    *gameplay.stats_mut(uuid).expect("validated loom statistic owner") = stats_after_event[index].clone();
                    cursor = index + 1;
                }
                while cursor < publication.events.len() {
                    let index = cursor;
                    let event = &publication.events[index];
                    canonical_menu.inventory = loom_event_phase(event).player_inventory.clone();
                    if let LoomMenuEvent::Drop { index: drop_index, .. } = event {
                        if let Some(expected) = drop_ordinals[*drop_index] {
                            assert_eq!(expected, ordinal, "validated loom indexed drop order");
                            pending_drop = Some(index);
                            return;
                        }
                    }
                    *gameplay.stats_mut(uuid).expect("validated loom statistic owner") = stats_after_event[index].clone();
                    match event {
                        LoomMenuEvent::Sync { event: SyncEvent::SlotListener {
                            player_slot: Some(slot), stack, ..
                        }, phase } => {
                            gameplay.inventory_slot_changed(uuid, *slot, &phase.player_inventory, stack)
                                .expect("validated native empty-criteria inventory listener");
                        }
                        LoomMenuEvent::Sync { event: SyncEvent::Packet(packet), .. } => publish(events, id, vec![packet.clone()]),
                        LoomMenuEvent::Sync { event: SyncEvent::SlotListener { player_slot: None, .. }
                            | SyncEvent::DataListener { .. }, .. } => {},
                        LoomMenuEvent::Source { .. } => callbacks.complete_source_at(index)
                            .expect("validated original loom source/default-listener boundary"),
                        LoomMenuEvent::PlayerInventoryChanged { .. } => {
                            assert!(callbacks.is_player_inventory_change_at(index));
                            canonical_menu.inventory.set_changed();
                        }
                        LoomMenuEvent::TakeSound { .. } => {
                            assert!(callbacks.is_sound_at(index));
                            // Source Level.threadSafeRandom is independent of
                            // player/drop Level.random and draws with no audience.
                            let seed = item_publication.next_sound_seed().expect("validated actual level sound RNG");
                            let packet = loom_sound_packet(position, seed);
                            if !sound_recipients.is_empty() {
                                let _ = events.send(PlayerEvent::WorldAudience {
                                    recipients: Arc::clone(&sound_recipients), packets: Arc::new(vec![packet]),
                                });
                            }
                        }
                        LoomMenuEvent::Drop { .. } => {},
                    }
                    cursor += 1;
                }
            });
        assert!(pending_drop.is_none() && cursor == publication.events.len());
        assert_eq!(canonical_menu.inventory.get_times_changed(), final_inventory.get_times_changed(),
            "loom source counter timeline differs from its prepared owner");
        canonical_menu.inventory = final_inventory;
    }
}

impl Server {
    /// Source provider constructs the menu, sends OpenScreen, initializes real
    /// listeners/synchronizer, then installs current ownership. Caller already
    /// closed its previous external menu and allocated the actual window.
    pub(super) fn open_loom_menu_locked(
        &self, world: &mut WorldBackend, players: &mut LivePlayers, id: i32,
        position: (i32, i32, i32), window: u8, now: i64, _border: CollisionBorder,
    ) -> Result<()> {
        ensure!(!players.looms.has_open(id), "loom opening would replace an unclosed actual menu");
        players.require_current_native_source(world)?;
        require_loom_actor(players, id)?;
        let player = players.registry.get(id).context("loom opening actor missing")?.clone();
        let shared = players.containers.get(&id).context("loom canonical inventory missing")?.clone();
        let context = NativeLoomContext::capture(self, world, players, id, position, now)?;
        let mut open = OpenLoom { position, menu: LoomMenu::new(window, &context)?, context };
        publish(&self.entity_events, id, vec![crate::main_loom_dispatch::loom_open_screen(window)?]);
        let events = shared.read(|menu| open.menu.initialize(&menu.inventory, &open.context))??;
        ensure!(events.iter().all(|event| matches!(event, LoomMenuEvent::Sync { .. })),
            "loom initialization requires its actual synchronization-only source phase");
        let _ = PreparedLoomCallbacks::prepare(&players.gameplay, open.context.patterns(), &events, &open.context)?;
        let _ = loom_local_changes(&events)?;
        for event in &events {
            let one = std::slice::from_ref(event);
            let changes = loom_local_changes(one)?;
            if !changes.is_empty() {
                let stats = players.gameplay.snapshot_player_stats(player.profile.uuid)?;
                players.gameplay.prepare_menu_commit(player.profile.uuid,
                    &loom_event_phase(event).player_inventory, changes, stats)?.commit();
            }
            if let LoomMenuEvent::Sync { event: SyncEvent::Packet(packet), .. } = event {
                publish(&self.entity_events, id, vec![packet.clone()]);
            }
        }
        players.looms.menus.insert(id, open);
        Ok(())
    }

    pub(super) fn loom_action_locked(
        &self, world: &mut WorldBackend, players: &mut LivePlayers, id: i32,
        request: LoomRequest<'_>, now: i64, border: CollisionBorder,
    ) -> Result<()> {
        let Some(mut open) = players.looms.menus.get(&id).cloned() else { return Ok(()); };
        if matches!(&request, LoomRequest::Click(click) if click.window_id != open.menu.window_id())
            || matches!(&request, LoomRequest::Close { window, .. } if *window != open.menu.window_id()) {
            return Ok(());
        }
        let close = matches!(&request, LoomRequest::Close { .. });
        let player = players.registry.get(id).context("loom actor missing")?.clone();
        if !close { require_loom_actor(players, id)?; }
        let spectator = player.gamemode == 3;
        if spectator && matches!(&request, LoomRequest::Button(_)) { return Ok(()); }
        if !spectator && matches!(&request, LoomRequest::Click(_) | LoomRequest::Button(_))
            && !open.menu.still_valid(&loom_access(world, &player, open.position)?)? {
            // Packet handlers ignore invalid access. The real player phase
            // performs broadcast, its fresh stillValid gate and ordered close.
            return Ok(());
        }
        if !spectator && matches!(&request, LoomRequest::Click(click) if click.slot >= 40) { return Ok(()); }
        players.require_current_native_source(world)?;
        players.gameplay.require_supported_floor()?;
        open.context = open.context.refresh(self, world, players, id, now, close)?;
        let shared = players.containers.get(&id).context("loom canonical inventory missing")?.clone();
        let stats = players.gameplay.snapshot_player_stats(player.profile.uuid)?;
        let facts = players.world_context.item_facts(now, border, false)?;
        let rain_available = matches!(facts.rain, mc121_rust::runtime_item_world::RainAvailability::NotRaining)
            || players.rain_context.is_some();
        let drop_context = PlayerDrop {
            player_uuid: player.profile.uuid, position: player.position,
            eye_height: player_physics::player_dimensions(player.metadata.pose, 1.)?.eye_height,
            around: false, trace_item: false,
        };
        let origin = Vec3::new(open.position.0 as f64 + 0.5, open.position.1 as f64 + 0.5, open.position.2 as f64 + 0.5);
        let mut audience = Vec::new();
        for other in players.registry.snapshot() {
            let other_dimension = &players.player_lifecycle.get(&other.profile.uuid)
                .context("loom recipient dimension owner missing")?.dimension;
            if other_dimension == &open.context.binding.dimension
                && Vec3::new(other.position.x, other.position.y, other.position.z).subtract(origin).length_squared() < 256. {
                audience.push(other.entity_id);
            }
        }
        let sound_recipients = Arc::new(audience);
        let close_packet = match &request {
            LoomRequest::Close { send_close: true, .. } => {
                let mut fields = Writer::new(); fields.u8(open.menu.window_id());
                Some(Packet { id: 0x12, payload: fields.into_inner() })
            }
            _ => None,
        };
        let LivePlayers {
            item_runtime, player_random, gameplay, looms,
            chunk_runtime, chunk_transports, ..
        } = players;
        let actual_player_random = player_random.get_mut(&id).context("loom player RNG missing")?;
        shared.transact_with_menu_commit(move |menu| {
            let counter_before = menu.inventory.get_times_changed();
            let mut next_open = open;
            let full_state = matches!(&request, LoomRequest::Resync)
                || spectator && matches!(&request, LoomRequest::Click(_));
            let mut publication = match request {
                LoomRequest::Broadcast => LoomPublication {
                    events: next_open.menu.broadcast_changes(&menu.inventory, &next_open.context)?,
                    outcome: ClickOutcome::default(), accepted_button: None,
                },
                _ if full_state => {
                    let events = next_open.menu.broadcast_full_state(&menu.inventory, &next_open.context)?;
                    let phase = phase(&next_open, &menu.inventory);
                    LoomPublication {
                        events: events.into_iter().map(|event| LoomMenuEvent::Sync { event, phase: phase.clone() }).collect(),
                        outcome: ClickOutcome::default(), accepted_button: None,
                    }
                }
                action => {
                    let plan = match action {
                        LoomRequest::Click(click) => next_open.menu.prepare_click(click, &menu.inventory, &next_open.context, player.gamemode == 1)?,
                        LoomRequest::Button(button) => next_open.menu.prepare_button(button, &menu.inventory, &next_open.context)?,
                        LoomRequest::Close { context, .. } => next_open.menu.prepare_close(&menu.inventory, &next_open.context, context)?,
                        LoomRequest::Broadcast | LoomRequest::Resync => unreachable!(),
                    };
                    let current = looms.menus.get(&id).context("loom actual menu disappeared")?;
                    plan.validate(&current.menu, &menu.inventory, &next_open.context)?;
                    let _ = PreparedLoomCallbacks::prepare(gameplay, next_open.context.patterns(), plan.events(), &next_open.context)?;
                    for drop in plan.drops() { drop.stack.encode_canonical()?; }
                    plan.validate(&current.menu, &menu.inventory, &next_open.context)?;
                    plan.commit(&mut next_open.menu, &mut menu.inventory, &next_open.context)?
                }
            };
            project_inventory_counter(&mut publication.events, &mut menu.inventory, counter_before)?;
            ensure!(publication.outcome.packets.is_empty(), "loom packets have more than one publication owner");
            let callbacks = PreparedLoomCallbacks::prepare(gameplay, next_open.context.patterns(), &publication.events, &next_open.context)?;
            let _ = loom_local_changes(&publication.events)?;
            // Verify actual player-criterion owner/arguments before any live
            // packet, item add or independent sound RNG consumption occurs.
            for event in &publication.events {
                let changes = loom_local_changes(std::slice::from_ref(event))?;
                if !changes.is_empty() {
                    drop(gameplay.prepare_menu_commit(player.profile.uuid,
                        &loom_event_phase(event).player_inventory, changes, stats.clone())?);
                }
                if let LoomMenuEvent::TakeSound { game_time, .. } = event {
                    ensure!(*game_time == now, "loom sound lost its actual world game time");
                    ensure!(item_runtime.sound_random_state48().is_some(), "loom requires actual Level.threadSafeRandom");
                }
            }
            let mut seen = BTreeSet::new();
            let mut drop_order = Vec::new();
            let mut drop_ordinals = vec![None; publication.outcome.drops.len()];
            let mut next_stats = stats;
            let mut stats_after_event = Vec::with_capacity(publication.events.len());
            for event in &publication.events {
                if let LoomMenuEvent::Drop { index, .. } = event {
                    ensure!(*index < publication.outcome.drops.len() && seen.insert(*index), "invalid repeated loom owned drop");
                    let dropped = &publication.outcome.drops[*index];
                    if !dropped.stack.is_empty() {
                        drop_ordinals[*index] = Some(drop_order.len());
                        drop_order.push(*index);
                        next_stats.award_drop(DropStatEvent {
                            constructed: true, trace_item: dropped.trace_item,
                            returned_item_id: Some(dropped.stack.item_id), original_count: dropped.stack.count,
                        })?;
                    }
                }
                stats_after_event.push(next_stats.clone());
            }
            ensure!(seen.len() == publication.outcome.drops.len(), "unpublished loom owned drop");
            if !drop_order.is_empty() {
                ensure!(rain_available, "loom drop requires actual rain context");
                chunk_runtime.certify_current_query_region(world, chunk_transports, &self.entity_events, now,
                    (((player.position.x - 18.).floor() as i32).div_euclid(16), ((player.position.z - 18.).floor() as i32).div_euclid(16)),
                    (((player.position.x + 18.).floor() as i32).div_euclid(16), ((player.position.z + 18.).floor() as i32).div_euclid(16)))?;
            }
            if close { next_open.menu.transfer_inventory_state(menu)?; }
            let mut next_player_random = actual_player_random.clone();
            let mut next_world_random = item_runtime.world_random_mut().clone();
            let first_id = self.entities.load(Ordering::Relaxed);
            let next_entity_id = first_id.checked_add(i32::try_from(drop_order.len())?).context("loom drop entity IDs exhausted")?;
            let mut owned = std::mem::take(&mut publication.outcome.drops).into_iter().map(Some).collect::<Vec<_>>();
            let mut entries = Vec::with_capacity(drop_order.len());
            for (ordinal, index) in drop_order.into_iter().enumerate() {
                let dropped = owned[index].take().expect("validated one loom owned drop");
                let context = PlayerDrop { around: dropped.around, trace_item: dropped.trace_item, ..drop_context };
                let entity_id = first_id.checked_add(i32::try_from(ordinal)?).context("loom drop entity ID exhausted")?;
                entries.push(spawn_player_drop_with_factory(entity_id, context, dropped.stack,
                    &mut next_player_random, &mut next_world_random,
                    &mut || random_seed::create_random().map_err(anyhow::Error::from))?
                    .context("nonempty loom drop did not construct an item")?);
            }
            let final_inventory = menu.inventory.clone();
            let items = item_runtime.prepare_items(entries)?;
            Ok((PreparedLoomCommit {
                items, player_random: actual_player_random, next_player_random, next_world_random,
                entity_counter: &self.entities, next_entity_id, gameplay,
                menus: &mut looms.menus, next_open, close, close_packet, publication,
                callbacks, stats_after_event, drop_ordinals, sound_recipients, final_inventory,
            }, vec![]))
        }, |prepared, menu| prepared.commit(id, player.profile.uuid, &self.entity_events, menu))
    }
}
