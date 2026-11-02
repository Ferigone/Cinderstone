//! Actual native grindstone host inside the canonical owner transaction.
use super::*;
use crate::main_experience::{NativeMenuExperienceStage, PreparedNativeExperience};
use crate::main_grindstone_recipes::{
    grindstone_experience_position, grindstone_level_event_packet,
    grindstone_preflight_phase, grindstone_preflight_sync,
    NativeGrindstoneEnchantments, NativeGrindstoneRegistrySnapshot,
};
use mc121_rust::{
    container::{return_to_inventory_on_menu_close, DroppedStack, MenuCloseContext},
    container_workstation_grindstone::{
        GrindstoneMenu, GrindstoneMenuAction, GrindstoneMenuHost, GrindstoneMenuToken, GrindstonePhase,
    },
    inventory::PlayerInventory,
    item_entity::{spawn_player_drop_with_factory, PlayerDrop},
    player_stats::{DropStatEvent, PlayerStats},
    runtime_entities::{ExperienceOrbRegistration, NativeEntityAddition, NativeEntityBatchEvent, PickupPlayer, PreparedNativeEntityBatch},
    runtime_entity_index::{EntityKind, RuntimeEntityIndex},
    workstations::{grindstone::GrindstoneEnchantments, menu::{SyncEvent, WorkstationAccess, WorkstationKind}},
};

/// Actual transient identities, never a serialized substitute for a Level.
/// Menu lifetime/revision is additionally checked by the real kernel token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GrindstoneBinding {
    actor: i32,
    uuid: [u8; 16],
    level_owner: usize,
    dimension: String,
    position: (i32, i32, i32),
}

pub(super) type NativeGrindstoneMenu = GrindstoneMenu<GrindstoneBinding, NativeGrindstoneRegistrySnapshot>;

#[derive(Debug)]
pub(super) struct OpenGrindstone {
    pub(super) position: (i32, i32, i32),
    pub(super) menu: NativeGrindstoneMenu,
    binding: GrindstoneBinding,
    enchantments: NativeGrindstoneEnchantments,
}

#[derive(Default)]
pub(super) struct GrindstoneRuntime {
    pub(super) menus: BTreeMap<i32, OpenGrindstone>,
}
impl GrindstoneRuntime {
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
        self.menus.get(&id).map_or_else(|| inventory.clone(),
            |open| inventory.snapshot_with_carried(open.menu.carried()))
    }
}

pub(super) enum GrindstoneRequest<'a> {
    Initialize { window: u8, position: (i32, i32, i32) },
    Click(&'a ContainerClick),
    Close { window: u8, send_close: bool, context: MenuCloseContext },
    Broadcast,
    Resync,
}

/// Actual retained synchronization capability at ServerPlayer.take. Broadcast
/// does not consult world validity, game time, instabuild, RNG, drops or XP.
/// Every such gameplay operation remains unavailable rather than receiving a
/// fabricated owner/value. The real pickup frame executes the returned phases.
struct InventoryBroadcastHost {
    binding: GrindstoneBinding,
    enchantments: NativeGrindstoneEnchantments,
    inventory: PlayerInventory,
    token: Option<GrindstoneMenuToken>,
    events: Vec<(Vec<SyncEvent>, GrindstonePhase)>,
}
impl GrindstoneEnchantments for InventoryBroadcastHost {
    fn is_curse(&self, id: i32) -> Result<bool> { self.enchantments.is_curse(id) }
    fn minimum_cost(&self, id: i32, level: i32) -> Result<i32> { self.enchantments.minimum_cost(id, level) }
}
impl GrindstoneMenuHost for InventoryBroadcastHost {
    type Binding = GrindstoneBinding;
    type RegistrySnapshot = NativeGrindstoneRegistrySnapshot;
    fn binding(&self) -> &Self::Binding { &self.binding }
    fn registry_snapshot(&self) -> Self::RegistrySnapshot { self.enchantments.registry_snapshot() }
    fn begin_stage(&self, token: GrindstoneMenuToken, action: GrindstoneMenuAction) -> Result<Self> {
        ensure!(matches!(action, GrindstoneMenuAction::Broadcast), "inventory broadcast cannot authorize a grindstone gameplay action");
        Ok(Self { binding: self.binding.clone(), enchantments: self.enchantments.clone(),
            inventory: self.inventory.clone(), token: Some(token), events: Vec::new() })
    }
    fn publish_stage(&mut self, staged: Self, token: GrindstoneMenuToken) -> Result<()> {
        ensure!(staged.token == Some(token) && staged.binding == self.binding
            && staged.registry_snapshot() == self.registry_snapshot(), "grindstone pickup synchronization owner changed");
        ensure!(staged.inventory.slots() == self.inventory.slots()
            && staged.inventory.carried() == self.inventory.carried()
            && staged.inventory.selected_hotbar() == self.inventory.selected_hotbar()
            && staged.inventory.state_id() == self.inventory.state_id()
            && staged.inventory.get_times_changed() == self.inventory.get_times_changed(),
            "grindstone pickup broadcast mutated canonical inventory");
        *self = staged;
        Ok(())
    }
    fn preflight(&mut self, action: GrindstoneMenuAction) -> Result<()> {
        ensure!(matches!(action, GrindstoneMenuAction::Broadcast), "unsupported grindstone pickup action");
        main_workstation_components::require_supported_inventory(&self.inventory)
    }
    fn inventory(&self) -> &PlayerInventory { &self.inventory }
    fn inventory_mut(&mut self) -> &mut PlayerInventory {
        unreachable!("source broadcast never requests a mutable inventory")
    }
    fn instabuild(&self) -> bool { unreachable!("source broadcast never queries gameplay abilities") }
    fn still_valid_grindstone(&self, _: f64) -> Result<bool> { bail!("pickup is not the grindstone validity phase") }
    fn input_container_set_changed(&mut self, _: &GrindstonePhase) -> Result<()> { bail!("pickup broadcast cannot mutate grindstone inputs") }
    fn player_inventory_set_changed(&mut self, _: &GrindstonePhase) -> Result<()> { bail!("pickup broadcast has no inventory dirty callback") }
    fn stage_sync_events(&mut self, events: Vec<SyncEvent>, phase: &GrindstonePhase) -> Result<()> {
        let _ = grindstone_preflight_sync(&events, phase)?;
        self.events.push((events, phase.clone())); Ok(())
    }
    fn level_random_next_int(&mut self, _: i32) -> Result<i32> { bail!("pickup broadcast has no Level RNG capability") }
    fn award_experience_orbs(&mut self, _: i32, _: &GrindstonePhase) -> Result<()> { bail!("pickup broadcast cannot award XP") }
    fn level_event(&mut self, _: i32, _: i32, _: &GrindstonePhase) -> Result<()> { bail!("pickup broadcast cannot emit a level event") }
    fn stage_drop(&mut self, _: DroppedStack, _: &GrindstonePhase) -> Result<()> { bail!("pickup broadcast cannot drop an item") }
    fn add_to_player_inventory(&mut self, _: &mut ItemStack, _: &GrindstonePhase) -> Result<bool> { bail!("pickup broadcast cannot repeat Inventory.add") }
    fn place_item_back(&mut self, _: &mut ItemStack, _: &GrindstonePhase) -> Result<()> { bail!("pickup broadcast cannot remove a menu") }
    fn quickcraft_iteration_order(&self, _: &[usize]) -> Result<Vec<usize>> { bail!("pickup broadcast cannot execute a drag") }
}
impl OpenGrindstone {
    pub(super) fn verify_pickup_actor(&self, player: PickupPlayer) -> Result<()> {
        ensure!(self.binding.actor == player.entity_id && self.binding.uuid == player.uuid,
            "grindstone pickup actor differs from its actual retained menu binding");
        Ok(())
    }

    pub(super) fn prepare_inventory_broadcast(&self, inventory: &PlayerInventory)
        -> Result<(NativeGrindstoneMenu, Vec<(Vec<SyncEvent>, GrindstonePhase)>)> {
        for stack in self.menu.state().inputs().iter().chain([self.menu.state().preview(), self.menu.carried()]) {
            main_workstation_components::require_supported_stack(stack)?;
        }
        let mut host = InventoryBroadcastHost { binding: self.binding.clone(), enchantments: self.enchantments.clone(),
            inventory: inventory.clone(), token: None, events: Vec::new() };
        let mut menu = self.menu.staged_copy_for_host();
        menu.broadcast_changes(&mut host)?;
        Ok((menu, host.events))
    }
}

#[derive(Clone)]
enum SourceEvent {
    Sync { events: Vec<SyncEvent>, phase: GrindstonePhase },
    InventoryChanged(GrindstonePhase),
    InputChanged(GrindstonePhase),
    Packets { packets: Vec<Packet>, phase: GrindstonePhase },
    Drop { ordinal: usize, statistics: PlayerStats, phase: GrindstonePhase },
    Experience { award: usize, start: usize, end: usize, phase: GrindstonePhase },
    LevelEvent { packet: Packet, phase: GrindstonePhase },
}
impl SourceEvent {
    fn phase(&self) -> &GrindstonePhase {
        match self {
            Self::Sync { phase, .. } | Self::Packets { phase, .. } | Self::Drop { phase, .. }
            | Self::Experience { phase, .. } | Self::LevelEvent { phase, .. }
            | Self::InventoryChanged(phase) | Self::InputChanged(phase) => phase,
        }
    }
}

/// Private proposals of the actual borrowed owners. The immutable live view is
/// used for visibility/source checks, never as a replacement runtime. No stage
/// can publish until the enclosing SharedContainer transaction prepares the
/// complete mixed entity sequence and owns its actual exclusive commit token.
struct NativeGrindstoneHost<'a> {
    owners: &'a LivePlayers,
    binding: GrindstoneBinding,
    enchantments: NativeGrindstoneEnchantments,
    container: PlayerContainer,
    player_random: LegacyRandomSource,
    level_random: LegacyRandomSource,
    index: RuntimeEntityIndex,
    experience: NativeMenuExperienceStage,
    statistics: PlayerStats,
    next_entity_id: i32,
    entries: Vec<NativeEntityAddition>,
    events: Vec<SourceEvent>,
    base_token: Option<GrindstoneMenuToken>,
    action: Option<GrindstoneMenuAction>,
    creative: bool,
    valid: bool,
    rain_available: bool,
    drop_context: PlayerDrop,
}

impl GrindstoneEnchantments for NativeGrindstoneHost<'_> {
    fn is_curse(&self, id: i32) -> Result<bool> { self.enchantments.is_curse(id) }
    fn minimum_cost(&self, id: i32, level: i32) -> Result<i32> {
        self.enchantments.minimum_cost(id, level)
    }
}

impl GrindstoneMenuHost for NativeGrindstoneHost<'_> {
    type Binding = GrindstoneBinding;
    type RegistrySnapshot = NativeGrindstoneRegistrySnapshot;
    fn binding(&self) -> &Self::Binding { &self.binding }
    fn registry_snapshot(&self) -> Self::RegistrySnapshot { self.enchantments.registry_snapshot() }
    fn begin_stage(&self, token: GrindstoneMenuToken, action: GrindstoneMenuAction) -> Result<Self> {
        Ok(Self {
            owners: self.owners, binding: self.binding.clone(), enchantments: self.enchantments.clone(),
            container: self.container.clone(), player_random: self.player_random.clone(),
            level_random: self.level_random.clone(), index: self.index.snapshot_for_preparation()?,
            experience: self.experience.clone(), statistics: self.statistics.clone(),
            next_entity_id: self.next_entity_id, entries: self.entries.clone(), events: self.events.clone(),
            base_token: Some(token), action: Some(action), creative: self.creative,
            valid: self.valid, rain_available: self.rain_available, drop_context: self.drop_context,
        })
    }
    fn publish_stage(&mut self, staged: Self, token: GrindstoneMenuToken) -> Result<()> {
        ensure!(staged.base_token == Some(token) && staged.binding == self.binding
            && staged.registry_snapshot() == self.registry_snapshot()
            && std::ptr::eq(staged.owners, self.owners), "grindstone staged actual owner changed");
        main_workstation_components::require_supported_inventory(&staged.container.inventory)?;
        // This installs only the enclosing transaction proposal. The retained
        // original menu and every live owner remain unchanged until the final
        // actual mixed-batch preparation succeeds under the same held locks.
        *self = staged;
        Ok(())
    }
    fn preflight(&mut self, _action: GrindstoneMenuAction) -> Result<()> {
        self.owners.gameplay.require_supported_floor()?;
        main_workstation_components::require_supported_inventory(&self.container.inventory)
    }
    fn inventory(&self) -> &PlayerInventory { &self.container.inventory }
    fn inventory_mut(&mut self) -> &mut PlayerInventory { &mut self.container.inventory }
    fn instabuild(&self) -> bool { self.creative }
    fn still_valid_grindstone(&self, additional_range: f64) -> Result<bool> {
        ensure!(additional_range == 4., "unexpected grindstone captured reach padding");
        Ok(self.valid)
    }
    fn input_container_set_changed(&mut self, phase: &GrindstonePhase) -> Result<()> {
        grindstone_preflight_phase(phase)?;
        // The actual constructor's SimpleContainer listener is the real menu;
        // the kernel executes its old-preview/recompute/new-preview sequence.
        // Additional/reentrant listeners are outside this admitted host.
        self.events.push(SourceEvent::InputChanged(phase.clone()));
        Ok(())
    }
    fn player_inventory_set_changed(&mut self, phase: &GrindstonePhase) -> Result<()> {
        grindstone_preflight_phase(phase)?;
        self.events.push(SourceEvent::InventoryChanged(phase.clone()));
        // Preserve the actual before-increment image and advance this same
        // detached inventory owner, so later callbacks see the source counter.
        self.container.inventory.set_changed();
        Ok(())
    }
    fn stage_sync_events(&mut self, events: Vec<SyncEvent>, phase: &GrindstonePhase) -> Result<()> {
        let _ = grindstone_preflight_sync(&events, phase)?;
        self.events.push(SourceEvent::Sync { events, phase: phase.clone() });
        Ok(())
    }
    fn level_random_next_int(&mut self, bound: i32) -> Result<i32> {
        Ok(self.level_random.next_bounded_i32(bound)?)
    }
    fn award_experience_orbs(&mut self, amount: i32, phase: &GrindstonePhase) -> Result<()> {
        grindstone_preflight_phase(phase)?;
        let award = self.experience.award_count();
        let start = self.entries.len();
        self.experience.award(self.owners, amount, grindstone_experience_position(self.binding.position),
            &mut self.next_entity_id, &mut self.index, &mut self.level_random)?;
        for state in self.experience.awards()[award].registration_states() {
            self.entries.push(NativeEntityAddition::ExperienceOrb(ExperienceOrbRegistration::from_state(state)?));
        }
        self.events.push(SourceEvent::Experience { award, start, end: self.entries.len(), phase: phase.clone() });
        Ok(())
    }
    fn level_event(&mut self, event: i32, data: i32, phase: &GrindstonePhase) -> Result<()> {
        grindstone_preflight_phase(phase)?;
        self.events.push(SourceEvent::LevelEvent {
            packet: grindstone_level_event_packet(self.binding.position, event, data)?, phase: phase.clone(),
        });
        Ok(())
    }
    fn stage_drop(&mut self, owned: DroppedStack, phase: &GrindstonePhase) -> Result<()> {
        grindstone_preflight_phase(phase)?;
        main_workstation_components::require_supported_stack(&owned.stack)?;
        if owned.stack.is_empty() { return Ok(()); }
        ensure!(self.rain_available, "grindstone drop requires actual rain context");
        let entity_id = self.next_entity_id;
        ensure!(entity_id > 0 && entity_id < i32::MAX, "grindstone entity ID exhausted");
        self.next_entity_id = entity_id.checked_add(1).context("grindstone entity IDs exhausted")?;
        let context = PlayerDrop { around: owned.around, trace_item: owned.trace_item, ..self.drop_context };
        let original_count = owned.stack.count;
        let original_item = owned.stack.item_id;
        let (state, random) = spawn_player_drop_with_factory(entity_id, context, owned.stack,
            &mut self.player_random, &mut self.level_random,
            &mut || random_seed::create_random().map_err(anyhow::Error::from))?
            .context("nonempty grindstone drop did not construct an actual item")?;
        self.index.register(entity_id, EntityKind::Item, state.position_vec(), false,
            |chunk| self.owners.item_runtime.experience_chunk_visibility(chunk))?;
        self.statistics.award_drop(DropStatEvent { constructed: true, trace_item: owned.trace_item,
            returned_item_id: Some(original_item), original_count })?;
        self.events.push(SourceEvent::Drop { ordinal: self.entries.len(), statistics: self.statistics.clone(), phase: phase.clone() });
        self.entries.push(NativeEntityAddition::Item(state, random));
        Ok(())
    }
    fn add_to_player_inventory(&mut self, stack: &mut ItemStack, phase: &GrindstonePhase) -> Result<bool> {
        grindstone_preflight_phase(phase)?;
        main_workstation_components::require_supported_stack(stack)?;
        self.container.add_pickup_to_inventory(stack, self.creative)
    }
    fn place_item_back(&mut self, stack: &mut ItemStack, phase: &GrindstonePhase) -> Result<()> {
        grindstone_preflight_phase(phase)?;
        main_workstation_components::require_supported_stack(stack)?;
        ensure!(matches!(self.action, Some(GrindstoneMenuAction::Close(_))),
            "placeItemBack requires its actual grindstone removed phase");
        let mut outcome = ClickOutcome::default();
        return_to_inventory_on_menu_close(&mut self.container.inventory, stack, &mut outcome)?;
        if !outcome.packets.is_empty() {
            let mut observed = phase.clone();
            observed.player_inventory = self.container.inventory.clone();
            self.events.push(SourceEvent::Packets { packets: outcome.packets, phase: observed });
        }
        ensure!(outcome.crafted.is_empty(), "menu return unexpectedly invoked a crafted callback");
        // Only Close calls this method, cursor first and then inputs after the
        // kernel clears its cursor. This is source call position, not a guess
        // that equal-valued stacks are the same owned object.
        for drop in outcome.drops {
            let mut observed = phase.clone();
            observed.player_inventory = self.container.inventory.clone();
            if !phase.carried.is_empty() { observed.carried = drop.stack.clone(); }
            self.stage_drop(drop, &observed)?;
        }
        Ok(())
    }
    fn quickcraft_iteration_order(&self, selected: &[usize]) -> Result<Vec<usize>> {
        ensure!(selected.len() <= 1, "multi-slot grindstone drag requires actual Slot-identity HashSet encounter order");
        Ok(selected.to_vec())
    }
}

fn publish(events: &broadcast::Sender<PlayerEvent>, id: i32, packets: Vec<Packet>) {
    if !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience { recipients: Arc::new(vec![id]), packets: Arc::new(packets) });
    }
}

struct Replay<'a> {
    events: Vec<SourceEvent>,
    awards: Vec<Option<PreparedNativeExperience>>,
    cursor: usize,
    pending: Option<(usize, usize)>, // source event index and last insertion exclusive
    gameplay: &'a mut main_item_gameplay::MainItemGameplay,
    experience: &'a mut main_experience::MainExperienceRuntime,
    canonical: &'a mut PlayerContainer,
    sender: &'a broadcast::Sender<PlayerEvent>,
    audience: Arc<Vec<i32>>,
    actor: i32,
    uuid: [u8; 16],
}
impl Replay<'_> {
    fn before(&mut self, ordinal: usize) {
        if self.pending.is_some() { return; }
        while self.cursor < self.events.len() {
            self.canonical.inventory = self.events[self.cursor].phase().player_inventory.clone();
            match &self.events[self.cursor] {
                SourceEvent::Drop { ordinal: expected, .. } => {
                    assert_eq!(*expected, ordinal);
                    self.pending = Some((self.cursor, ordinal + 1));
                    return;
                }
                SourceEvent::Experience { award, start, end, .. } => {
                    assert_eq!(*start, ordinal);
                    if start != end {
                        self.pending = Some((self.cursor, *end));
                        return;
                    }
                    self.awards[*award].take().expect("one source XP award")
                        .commit_registered_parts(self.experience, self.sender)
                        .expect("prevalidated actual zero-registration XP checkpoint");
                }
                SourceEvent::Sync { events, phase } => {
                    for event in events {
                        match event {
                            SyncEvent::SlotListener { player_slot: Some(slot), stack, .. } => {
                                self.gameplay.inventory_slot_changed(self.uuid, *slot, &phase.player_inventory, stack)
                                    .expect("prevalidated source inventory listener");
                            }
                            SyncEvent::Packet(packet) => publish(self.sender, self.actor, vec![packet.clone()]),
                            _ => {}
                        }
                    }
                }
                SourceEvent::InventoryChanged(_) => self.canonical.inventory.set_changed(),
                SourceEvent::InputChanged(phase) => {
                    // The kernel already executed the actual default menu
                    // listener's recomputation/broadcast phases while staged.
                    // This consumed validated observation is not a new dirty
                    // counter or a second SimpleContainer notification.
                    let _ = phase;
                }
                SourceEvent::Packets { packets, .. } => publish(self.sender, self.actor, packets.clone()),
                SourceEvent::LevelEvent { packet, .. } => {
                    if !self.audience.is_empty() {
                        let _ = self.sender.send(PlayerEvent::WorldAudience {
                            recipients: Arc::clone(&self.audience), packets: Arc::new(vec![packet.clone()]),
                        });
                    }
                }
            }
            self.cursor += 1;
        }
    }
    fn registered(&mut self, ordinal: usize, handle: mc121_rust::runtime_entity_index::EntityHandle, kind: EntityKind) {
        let (index, end) = self.pending.expect("actual insertion has a source event");
        match &self.events[index] {
            SourceEvent::Drop { ordinal: expected, statistics, .. } => {
                assert_eq!(*expected, ordinal);
                assert_eq!(kind, EntityKind::Item);
                *self.gameplay.stats_mut(self.uuid).expect("actual grindstone statistic owner") = statistics.clone();
            }
            SourceEvent::Experience { award, .. } => {
                assert_eq!(kind, EntityKind::ExperienceOrb);
                self.awards[*award].as_mut().expect("retained actual XP proposal")
                    .attach_registered(handle.id, handle).expect("validated actual XP registration handle");
                if ordinal + 1 == end {
                    self.awards[*award].take().expect("one source XP award")
                        .commit_registered_parts(self.experience, self.sender)
                        .expect("prevalidated actual integer XP checkpoint");
                }
            }
            _ => unreachable!(),
        }
        if ordinal + 1 == end { self.pending = None; self.cursor = index + 1; }
    }
}

struct PreparedGrindstoneCommit<'a> {
    entities: PreparedNativeEntityBatch<'a>,
    actual_player_random: &'a mut LegacyRandomSource,
    player_random: LegacyRandomSource,
    level_random: LegacyRandomSource,
    entity_counter: &'a AtomicI32,
    next_entity_id: i32,
    menus: &'a mut BTreeMap<i32, OpenGrindstone>,
    open: OpenGrindstone,
    close: bool,
    initial_packet: Option<Packet>,
    events: Vec<SourceEvent>,
    awards: Vec<PreparedNativeExperience>,
    gameplay: &'a mut main_item_gameplay::MainItemGameplay,
    experience: &'a mut main_experience::MainExperienceRuntime,
    audience: Arc<Vec<i32>>,
    final_inventory: PlayerInventory,
}
impl PreparedGrindstoneCommit<'_> {
    fn commit(self, id: i32, uuid: [u8; 16], sender: &broadcast::Sender<PlayerEvent>, canonical: &mut PlayerContainer) {
        let Self { entities, actual_player_random, player_random, level_random, entity_counter, next_entity_id,
            menus, open, close, initial_packet, events, awards, gameplay, experience, audience, final_inventory } = self;
        *actual_player_random = player_random;
        entity_counter.store(next_entity_id, Ordering::Relaxed);
        if close { menus.remove(&id); } else { menus.insert(id, open); }
        if let Some(packet) = initial_packet { publish(sender, id, vec![packet]); }
        let mut replay = Replay { events, awards: awards.into_iter().map(Some).collect(), cursor: 0, pending: None,
            gameplay, experience, canonical, sender, audience, actor: id, uuid };
        entities.commit_with_world_random_ordered(level_random, |event, _publication| match event {
            NativeEntityBatchEvent::Before { ordinal } => replay.before(ordinal),
            NativeEntityBatchEvent::Registered { ordinal, handle, kind } => replay.registered(ordinal, handle, kind),
            NativeEntityBatchEvent::Complete { len } => replay.before(len),
        });
        assert!(replay.pending.is_none() && replay.cursor == replay.events.len());
        assert!(replay.awards.iter().all(Option::is_none));
        assert_eq!(replay.canonical.inventory.get_times_changed(), final_inventory.get_times_changed(),
            "source inventory counter events match the prepared actual owner");
        replay.canonical.inventory = final_inventory;
    }
}

impl Server {
    pub(super) fn open_grindstone_menu_locked(&self, world: &mut WorldBackend, players: &mut LivePlayers,
        id: i32, position: (i32, i32, i32), window: u8, now: i64, border: CollisionBorder) -> Result<()> {
        self.grindstone_action_locked(world, players, id, GrindstoneRequest::Initialize { window, position }, now, border)
    }

    pub(super) fn grindstone_action_locked(&self, world: &mut WorldBackend, players: &mut LivePlayers,
        id: i32, request: GrindstoneRequest<'_>, now: i64, border: CollisionBorder) -> Result<()> {
        let initializing = matches!(request, GrindstoneRequest::Initialize { .. });
        let (position, previous_token) = if let GrindstoneRequest::Initialize { position, .. } = &request {
            ensure!(!players.grindstones.has_open(id), "grindstone opening has an existing actual owner");
            (*position, None)
        } else {
            let Some(open) = players.grindstones.menus.get(&id) else { return Ok(()); };
            if matches!(&request, GrindstoneRequest::Click(click) if click.window_id != open.menu.window_id())
                || matches!(&request, GrindstoneRequest::Close { window, .. } if *window != open.menu.window_id()) { return Ok(()); }
            (open.position, Some(open.menu.token()))
        };
        let player = players.registry.get(id).context("grindstone actor absent")?.clone();
        let close = matches!(request, GrindstoneRequest::Close { .. });
        if !close { main_stonecutter::require_stonecutter_actor(players, id)?; }
        let access = main_grindstone_dispatch::grindstone_access(world, &player, position)?;
        let valid = access.block_matches(WorkstationKind::Grindstone)?
            && access.player_can_interact_with_block(4.)?;
        if initializing { ensure!(valid, "opening actual grindstone access changed"); }
        if let GrindstoneRequest::Click(click) = &request {
            if player.gamemode != 3 {
                if click.slot >= 39 || !valid { return Ok(()); }
                main_workstation_components::require_supported_prediction(click)?;
            }
        }
        players.require_current_native_source(world)?;
        players.gameplay.require_supported_floor()?;
        players.entity_source.as_ref().context("grindstone actual native Level source missing")?
            .require_owned_callback_world(world)?;
        let enchantments = NativeGrindstoneEnchantments::new(&self.fixtures, &players.gameplay)?;
        let shared = players.containers.get(&id).context("grindstone canonical container absent")?.clone();
        let dimension = players.player_lifecycle.get(&player.profile.uuid).context("grindstone actual dimension missing")?.dimension.clone();
        let binding = GrindstoneBinding { actor: id, uuid: player.profile.uuid,
            level_owner: world as *const WorldBackend as usize, dimension: dimension.clone(), position };
        let facts = players.world_context.item_facts(now, border, false)?;
        let rain_available = matches!(facts.rain, mc121_rust::runtime_item_world::RainAvailability::NotRaining)
            || players.rain_context.is_some();
        if matches!(request, GrindstoneRequest::Close { .. })
            || player.gamemode != 3 && matches!(request, GrindstoneRequest::Click(_)) {
            players.chunk_runtime.certify_current_query_region(world, &mut players.chunk_transports, &self.entity_events, now,
                (((player.position.x - 18.).floor() as i32).div_euclid(16), ((player.position.z - 18.).floor() as i32).div_euclid(16)),
                (((player.position.x + 18.).floor() as i32).div_euclid(16), ((player.position.z + 18.).floor() as i32).div_euclid(16)))?;
        }
        let mut audience = Vec::new();
        let origin = Vec3::new(f64::from(position.0), f64::from(position.1), f64::from(position.2));
        for viewer in players.registry.snapshot() {
            let viewer_dimension = &players.player_lifecycle.get(&viewer.profile.uuid).context("grindstone recipient dimension missing")?.dimension;
            if viewer_dimension == &dimension && Vec3::new(viewer.position.x, viewer.position.y, viewer.position.z)
                .subtract(origin).length_squared() < 4096. { audience.push(viewer.entity_id); }
        }
        let audience = Arc::new(audience);
        let first_id = self.entities.load(Ordering::Relaxed);
        shared.transact_with_menu_commit(|canonical| {
            main_workstation_components::require_supported_inventory(&canonical.inventory)?;
            let level_random = players.item_runtime.world_random_mut().clone();
            let mut host = NativeGrindstoneHost {
                owners: &*players, binding, enchantments, container: canonical.clone(),
                player_random: players.player_random.get(&id).context("grindstone actual player RNG absent")?.clone(),
                level_random,
                index: players.item_runtime.prepare_experience_index()?,
                experience: NativeMenuExperienceStage::new(players)?,
                statistics: players.gameplay.snapshot_player_stats(player.profile.uuid)?,
                next_entity_id: first_id, entries: Vec::new(), events: Vec::new(), base_token: None,
                action: None, creative: player.gamemode == 1, valid, rain_available,
                drop_context: PlayerDrop { player_uuid: player.profile.uuid, position: player.position,
                    eye_height: player_physics::player_dimensions(player.metadata.pose, 1.)?.eye_height,
                    around: false, trace_item: false },
            };
            let mut next_menu = if let GrindstoneRequest::Initialize { window, .. } = request {
                NativeGrindstoneMenu::new(window, &host)?
            } else {
                let current = players.grindstones.menus.get(&id).context("retained grindstone menu disappeared")?;
                for stack in current.menu.state().inputs().iter()
                    .chain([current.menu.state().preview(), current.menu.carried()]) {
                    main_workstation_components::require_supported_stack(stack)?;
                }
                current.menu.staged_copy_for_host()
            };
            let initial_packet = match request {
                GrindstoneRequest::Initialize { window, .. } => {
                    next_menu.initialize(&mut host)?;
                    Some(main_grindstone_dispatch::grindstone_open_screen(window)?)
                }
                GrindstoneRequest::Click(_) if player.gamemode == 3 => { next_menu.broadcast_full_state(&mut host)?; None }
                GrindstoneRequest::Click(click) => { next_menu.click(next_menu.token(), click, &mut host)?; None }
                GrindstoneRequest::Broadcast => { next_menu.broadcast_changes(&mut host)?; None }
                GrindstoneRequest::Resync => { next_menu.broadcast_full_state(&mut host)?; None }
                GrindstoneRequest::Close { send_close, context, window } => {
                    next_menu.close(next_menu.token(), context, &mut host)?;
                    next_menu.transfer_inventory_state(&mut host.container)?;
                    if send_close {
                        let mut fields = Writer::new(); fields.u8(window);
                        Some(Packet { id: 0x12, payload: fields.into_inner() })
                    } else { None }
                }
            };
            let NativeGrindstoneHost { owners: _, binding, enchantments, container, player_random, level_random, experience,
                entries, events, next_entity_id, .. } = host;
            if let Some(expected) = previous_token {
                ensure!(players.grindstones.menus.get(&id).map(|open| open.menu.token()) == Some(expected),
                    "actual grindstone menu changed during transaction");
            } else { ensure!(!players.grindstones.has_open(id), "grindstone appeared during opening"); }
            ensure!(self.entities.load(Ordering::Relaxed) == first_id, "shared entity counter changed during grindstone action");
            let final_inventory = container.inventory.clone();
            *canonical = container;
            let LivePlayers { item_runtime, player_random: actual_random, experience_runtime, gameplay, grindstones, .. } = players;
            let entities = item_runtime.prepare_native_entities(entries)?;
            Ok((PreparedGrindstoneCommit {
                entities, actual_player_random: actual_random.get_mut(&id).context("actual grindstone RNG owner disappeared")?,
                player_random, level_random, entity_counter: &self.entities, next_entity_id,
                menus: &mut grindstones.menus, open: OpenGrindstone { position, menu: next_menu, binding, enchantments }, close,
                initial_packet, events, awards: experience.into_awards(), gameplay, experience: experience_runtime, audience, final_inventory,
            }, Vec::new()))
        }, |prepared, canonical| prepared.commit(id, player.profile.uuid, &self.entity_events, canonical))
    }
}
