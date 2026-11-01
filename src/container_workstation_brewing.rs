//! Java 1.21 BrewingStandMenu over one actual persistent BrewingState owner.
//!
//! The menu retains only UI/cursor/synchronization state. A prepared action
//! borrows/clones the full actual state without save/load reconstruction. Real
//! potion/features, player counters, criteria, block/world and drop owners are
//! mandatory; this module supplies no production no-op callback host.
use crate::{
    container::{
        ClickOutcome, ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer,
    },
    inventory::{self, ItemStack, PlayerInventory},
    protocol::{Packet, Reader},
    workstations::{
        brewing::{self, BrewingEffect, BrewingState, PotionBrewingOwner, SLOT_COUNT},
        menu::{MenuRemovalEvent, MenuSync, SyncEvent, WorkstationKind},
    },
};
use anyhow::{ensure, Result};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_BREWING_MENU_INSTANCE: AtomicU64 = AtomicU64::new(1);
fn allocate_menu_instance() -> Result<u64> {
    let mut current = NEXT_BREWING_MENU_INSTANCE.load(Ordering::Relaxed);
    loop {
        let next = current
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("brewing UI identity exhausted"))?;
        match NEXT_BREWING_MENU_INSTANCE.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(current),
            Err(actual) => current = actual,
        }
    }
}

/// Actual captured block/world owner and the Level-owned PotionBrewing object.
/// ContextKey identifies that real immutable potion/features context; it is not
/// manufactured by the controller. State revision covers ticks AND slot edits.
/// The outer runtime also binds this UI to its actual player/session/window.
pub trait BrewingMenuOwner {
    type Key: Clone + Eq + Debug;
    type ContextKey: Clone + Eq + Debug;
    type Potions: PotionBrewingOwner;
    fn owner_key(&self) -> &Self::Key;
    fn potion_context_key(&self) -> &Self::ContextKey;
    fn potion_brewing(&self) -> &Self::Potions;
    fn state_revision(&self) -> u64;
    fn brewing_state(&self) -> &BrewingState;
    fn still_valid_for_actual_player(&self) -> Result<bool>;
    fn actual_player_is_server_player(&self) -> bool;
    /// Multi-slot drag uses native index order only with real receiver/listener
    /// independence certification; there is no permissive implementation.
    fn quickcraft_callbacks_are_order_independent(&self) -> Result<bool>;
    /// Atomically stage/commit the ENTIRE ordered event stream: actual dirty,
    /// player counter, advancement, world/drop and Sync listener/packet outbox
    /// operations, then install this same actual state's next image. Failure
    /// changes neither owners, outbox nor canonical state. Returned controller
    /// receipts are inspection-only and MUST NOT replay any event/drop.
    /// The state installer must not replay callbacks already staged here.
    /// Phase observations are read-only views, never extra inventories/items.
    fn commit_prepared_brewing_menu(
        &mut self,
        expected_revision: u64,
        expected_context: &Self::ContextKey,
        before: &BrewingState,
        after: &BrewingState,
        events: &[BrewingMenuEvent<Self::Key>],
        drops: &[DroppedStack],
    ) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct BrewingPhase {
    pub brewing: BrewingState,
    pub inventory: PlayerInventory,
    /// External cursor, distinct from retained window0 carried state.
    pub carried: ItemStack,
}
#[derive(Clone, Debug)]
pub enum BrewingMenuEvent<K> {
    Effect {
        owner: K,
        effect: BrewingEffect,
        phase: BrewingPhase,
    },
    PlayerInventoryChanged {
        phase: BrewingPhase,
    },
    /// BREWED_POTION uses the actual potion holder and actual ServerPlayer.
    PotionTaken {
        holder_id: i32,
        stack: ItemStack,
        phase: BrewingPhase,
    },
    Sync {
        event: SyncEvent,
        phase: BrewingPhase,
    },
    /// Index into the prepared drops. The atomic owner executes this position;
    /// a returned inspection receipt must never replay it.
    Drop {
        index: usize,
        phase: BrewingPhase,
    },
}
impl BrewingPhase {
    fn observe_return_packet(&mut self, packet: &Packet) -> Result<()> {
        ensure!(packet.id == 0x15, "unsupported brewing close packet phase");
        let mut reader = Reader::new(&packet.payload);
        ensure!(
            reader.i8()? == -2 && reader.varint()? == 0,
            "brewing close requires raw inventory slot packet"
        );
        let raw = reader.i16()?;
        let slot = match raw {
            0..=8 => 36 + raw as usize,
            9..=35 => raw as usize,
            40 => 45,
            _ => anyhow::bail!("unsupported brewing return inventory index"),
        };
        let stack = ItemStack::decode_exact(reader.remaining())?;
        let old = self.inventory.slot(slot).unwrap();
        ensure!(
            stack.same_item_same_components(&self.carried)?,
            "brewing return changed item identity"
        );
        let old_count = if old.is_empty() {
            0
        } else {
            ensure!(
                old.same_item_same_components(&stack)?,
                "brewing return overwrote item"
            );
            old.count
        };
        let added = stack.count.wrapping_sub(old_count);
        ensure!(
            added > 0 && added <= self.carried.count,
            "invalid brewing return phase"
        );
        self.inventory.set_slot_authoritative(slot, stack);
        self.carried = counted(&self.carried, self.carried.count.wrapping_sub(added));
        Ok(())
    }
}
/// Inspection-only receipt of events and drop transfers ALREADY committed by
/// the actual owner's atomic host method. Do not dispatch/drain these again.
#[derive(Debug)]
pub struct BrewingCommitReceipt<K> {
    pub events: Vec<BrewingMenuEvent<K>>,
    pub outcome: ClickOutcome,
}
#[derive(Clone, Debug)]
pub struct BrewingMenu<K, C> {
    instance: u64,
    sync: MenuSync,
    owner_key: K,
    context_key: C,
    revision: u64,
    active: bool,
    drag_status: u8,
    drag_type: u8,
    drag_slots: BTreeSet<usize>,
}
pub struct PreparedBrewingAction<K, C> {
    expected_menu: BrewingMenu<K, C>,
    expected_inventory: PlayerInventory,
    expected_revision: u64,
    expected_state: BrewingState,
    next_menu: BrewingMenu<K, C>,
    next_inventory: PlayerInventory,
    next_state: BrewingState,
    require_valid: bool,
    require_order_independence: bool,
    server_player: bool,
    events: Vec<BrewingMenuEvent<K>>,
    outcome: ClickOutcome,
}
fn same_inventory(a: &PlayerInventory, b: &PlayerInventory) -> bool {
    a.slots() == b.slots()
        && a.carried() == b.carried()
        && a.selected_hotbar() == b.selected_hotbar()
        && a.state_id() == b.state_id()
}
fn validate_item_identity(stack: &ItemStack) -> Result<()> {
    ensure!(
        !stack
            .components
            .iter()
            .any(|component| component.type_id == 20),
        "custom FoodProperties requires actual nested ItemStack identity context"
    );
    stack.encode_canonical()?;
    Ok(())
}
fn validate_player_items(inventory: &PlayerInventory, carried: &ItemStack) -> Result<()> {
    for slot in 9..46 {
        validate_item_identity(inventory.slot(slot).expect("actual inventory slot"))?;
    }
    validate_item_identity(carried)
}
fn validate_owner(owner: &impl BrewingMenuOwner) -> Result<()> {
    for stack in owner.brewing_state().slots() {
        validate_item_identity(stack)?;
    }
    Ok(())
}
impl<K: Clone + Eq + Debug, C: Clone + Eq + Debug> BrewingMenu<K, C> {
    /// BrewingStandMenu invokes neither startOpen nor stopOpen. Block provider
    /// locks/access and actual player/session/window binding remain with Main.
    pub fn new(
        window_id: u8,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
    ) -> Result<Self> {
        validate_owner(owner)?;
        Ok(Self {
            instance: allocate_menu_instance()?,
            sync: MenuSync::new(window_id, WorkstationKind::Brewing)?,
            owner_key: owner.owner_key().clone(),
            context_key: owner.potion_context_key().clone(),
            revision: 0,
            active: true,
            drag_status: 0,
            drag_type: 0,
            drag_slots: BTreeSet::new(),
        })
    }
    pub fn instance_id(&self) -> u64 {
        self.instance
    }
    pub fn window_id(&self) -> u8 {
        self.sync.window_id()
    }
    pub fn state_id(&self) -> i32 {
        self.sync.state_id()
    }
    pub fn owner_key(&self) -> &K {
        &self.owner_key
    }
    pub fn carried(&self) -> &ItemStack {
        self.sync.carried()
    }
    fn advance_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("brewing UI revision exhausted"))?;
        Ok(())
    }
    fn check_owner(&self, owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>) -> Result<()> {
        ensure!(self.active, "brewing menu was removed");
        validate_owner(owner)?;
        ensure!(
            owner.owner_key() == &self.owner_key && owner.potion_context_key() == &self.context_key,
            "brewing menu cannot rebind block or potion/features owner"
        );
        Ok(())
    }
    pub fn still_valid(
        &self,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
    ) -> Result<bool> {
        self.check_owner(owner)?;
        owner.still_valid_for_actual_player()
    }
    pub fn initialize(
        &mut self,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        inventory: &PlayerInventory,
    ) -> Result<Vec<BrewingMenuEvent<K>>> {
        self.check_owner(owner)?;
        validate_player_items(inventory, self.carried())?;
        let mut next = self.clone();
        let events = next.sync.initialize(
            owner.brewing_state().slots(),
            inventory,
            &owner.brewing_state().data(),
        )?;
        next.advance_revision()?;
        *self = next;
        let phase = BrewingPhase {
            brewing: owner.brewing_state().clone(),
            inventory: inventory.clone(),
            carried: self.carried().clone(),
        };
        Ok(events
            .into_iter()
            .map(|event| BrewingMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect())
    }
    pub fn broadcast_changes(
        &mut self,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        inventory: &PlayerInventory,
    ) -> Result<Vec<BrewingMenuEvent<K>>> {
        self.check_owner(owner)?;
        validate_player_items(inventory, self.carried())?;
        let mut next = self.clone();
        let events = next.sync.broadcast_changes(
            owner.brewing_state().slots(),
            inventory,
            &owner.brewing_state().data(),
        )?;
        next.advance_revision()?;
        *self = next;
        let phase = BrewingPhase {
            brewing: owner.brewing_state().clone(),
            inventory: inventory.clone(),
            carried: self.carried().clone(),
        };
        Ok(events
            .into_iter()
            .map(|event| BrewingMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect())
    }
    pub fn broadcast_full_state(
        &mut self,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        inventory: &PlayerInventory,
    ) -> Result<Vec<SyncEvent>> {
        self.check_owner(owner)?;
        validate_player_items(inventory, self.carried())?;
        let mut next = self.clone();
        let events = next.sync.broadcast_full_state(
            owner.brewing_state().slots(),
            inventory,
            &owner.brewing_state().data(),
        )?;
        next.advance_revision()?;
        *self = next;
        Ok(events)
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    fn prepare(
        &self,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        inventory: &PlayerInventory,
        require_valid: bool,
    ) -> Result<PreparedBrewingAction<K, C>> {
        self.check_owner(owner)?;
        validate_player_items(inventory, self.carried())?;
        if require_valid {
            ensure!(
                owner.still_valid_for_actual_player()?,
                "actual brewing container is no longer valid"
            );
        }
        Ok(PreparedBrewingAction {
            expected_menu: self.clone(),
            expected_inventory: inventory.clone(),
            expected_revision: owner.state_revision(),
            expected_state: owner.brewing_state().clone(),
            next_menu: self.clone(),
            next_inventory: inventory.clone(),
            next_state: owner.brewing_state().clone(),
            require_valid,
            require_order_independence: false,
            server_player: owner.actual_player_is_server_player(),
            events: vec![],
            outcome: ClickOutcome::default(),
        })
    }
    pub fn prepare_click(
        &self,
        click: &ContainerClick,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        inventory: &PlayerInventory,
        creative: bool,
    ) -> Result<PreparedBrewingAction<K, C>> {
        ensure!(
            click.window_id == self.window_id() && click.slot < 41,
            "invalid brewing click target"
        );
        let mut action = self.prepare(owner, inventory, true)?;
        let before = self.state_id();
        action.next_menu.sync.suppress_remote_updates(true);
        action.do_click(
            click.slot as i32,
            click.button as i32,
            click.kind,
            owner,
            creative,
        )?;
        let events = action.next_menu.sync.finish_click(
            click,
            before,
            action.next_state.slots(),
            &action.next_inventory,
            &action.next_state.data(),
        )?;
        action.push_sync(events);
        action.next_menu.advance_revision()?;
        Ok(action)
    }
    /// No block slots are returned; no fake stopOpen invocation is introduced.
    pub fn prepare_close(
        &self,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        inventory: &PlayerInventory,
        context: MenuCloseContext,
    ) -> Result<PreparedBrewingAction<K, C>> {
        let mut action = self.prepare(owner, inventory, false)?;
        action.next_menu.reset_drag();
        let removal = action.next_menu.sync.close_inputs_ordered(
            &mut [],
            &mut action.next_inventory,
            context,
        )?;
        action.outcome = removal.outcome;
        let mut phase = BrewingPhase {
            brewing: owner.brewing_state().clone(),
            inventory: inventory.clone(),
            carried: self.carried().clone(),
        };
        for event in removal.events {
            match event {
                MenuRemovalEvent::Packet { index } => {
                    let packet = action.outcome.packets[index].clone();
                    phase.observe_return_packet(&packet)?;
                    action.events.push(BrewingMenuEvent::Sync {
                        event: SyncEvent::Packet(packet),
                        phase: phase.clone(),
                    });
                }
                MenuRemovalEvent::Drop { index } => {
                    phase.carried = action.outcome.drops[index].stack.clone();
                    action.events.push(BrewingMenuEvent::Drop {
                        index,
                        phase: phase.clone(),
                    });
                    phase.carried = ItemStack::empty();
                }
            }
        }
        action.outcome.packets.clear();
        action.next_menu.active = false;
        action.next_menu.advance_revision()?;
        Ok(action)
    }
    fn reset_drag(&mut self) {
        self.drag_status = 0;
        self.drag_slots.clear();
    }
}
impl<K: Clone + Eq + Debug, C: Clone + Eq + Debug> PreparedBrewingAction<K, C> {
    pub fn events(&self) -> &[BrewingMenuEvent<K>] {
        &self.events
    }
    pub fn drops(&self) -> &[DroppedStack] {
        &self.outcome.drops
    }
    pub fn next_state(&self) -> &BrewingState {
        &self.next_state
    }
    pub fn next_inventory(&self) -> &PlayerInventory {
        &self.next_inventory
    }
    pub fn validate(
        &self,
        menu: &BrewingMenu<K, C>,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        inventory: &PlayerInventory,
    ) -> Result<()> {
        ensure!(
            menu.instance == self.expected_menu.instance
                && menu.owner_key == self.expected_menu.owner_key
                && menu.context_key == self.expected_menu.context_key
                && menu.revision == self.expected_menu.revision
                && menu.sync == self.expected_menu.sync
                && menu.active == self.expected_menu.active
                && menu.drag_status == self.expected_menu.drag_status
                && menu.drag_type == self.expected_menu.drag_type
                && menu.drag_slots == self.expected_menu.drag_slots,
            "brewing UI changed after preparation"
        );
        menu.check_owner(owner)?;
        validate_player_items(inventory, menu.carried())?;
        ensure!(
            owner.state_revision() == self.expected_revision
                && owner.brewing_state() == &self.expected_state,
            "actual brewing state changed after preparation"
        );
        ensure!(
            same_inventory(inventory, &self.expected_inventory),
            "actual player inventory changed after brewing preparation"
        );
        ensure!(
            owner.actual_player_is_server_player() == self.server_player,
            "actual brewing player role changed"
        );
        if self.require_valid {
            ensure!(
                owner.still_valid_for_actual_player()?,
                "actual player lost brewing access"
            );
        }
        if self.require_order_independence {
            ensure!(
                owner.quickcraft_callbacks_are_order_independent()?,
                "brewing callback receiver context changed"
            );
        }
        Ok(())
    }
    pub fn commit(
        self,
        menu: &mut BrewingMenu<K, C>,
        owner: &mut impl BrewingMenuOwner<Key = K, ContextKey = C>,
        inventory: &mut PlayerInventory,
    ) -> Result<BrewingCommitReceipt<K>> {
        self.validate(menu, owner, inventory)?;
        owner.commit_prepared_brewing_menu(
            self.expected_revision,
            &self.expected_menu.context_key,
            &self.expected_state,
            &self.next_state,
            &self.events,
            &self.outcome.drops,
        )?;
        *menu = self.next_menu;
        *inventory = self.next_inventory;
        Ok(BrewingCommitReceipt {
            events: self.events,
            outcome: self.outcome,
        })
    }
    fn top_count(&self) -> usize {
        SLOT_COUNT
    }
    fn slot_count(&self) -> usize {
        41
    }
    fn slot(&self, slot: usize) -> ItemStack {
        if slot < 5 {
            self.next_state.slots()[slot].clone()
        } else {
            self.next_inventory
                .slot(slot + 4)
                .expect("validated player slot")
                .clone()
        }
    }
    fn may_place(
        &self,
        slot: usize,
        stack: &ItemStack,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
    ) -> Result<bool> {
        if slot < 5 {
            BrewingState::menu_may_place(slot, stack, owner.potion_brewing())
        } else {
            Ok(true)
        }
    }
    fn limit(&self, slot: usize, stack: &ItemStack) -> Result<i32> {
        if slot < 3 {
            Ok(1.min(stack.max_stack_size()?))
        } else {
            Ok(99.min(stack.max_stack_size()?))
        }
    }
    fn phase(&self) -> BrewingPhase {
        BrewingPhase {
            brewing: self.next_state.clone(),
            inventory: self.next_inventory.clone(),
            carried: self.next_menu.carried().clone(),
        }
    }
    fn push_sync(&mut self, events: Vec<SyncEvent>) {
        let phase = self.phase();
        self.events
            .extend(events.into_iter().map(|event| BrewingMenuEvent::Sync {
                event,
                phase: phase.clone(),
            }));
    }
    fn effect(&mut self, effect: BrewingEffect) {
        self.events.push(BrewingMenuEvent::Effect {
            owner: self.next_menu.owner_key.clone(),
            effect,
            phase: self.phase(),
        });
    }
    fn changed(&mut self, slot: usize) {
        if slot < 5 {
            self.effect(BrewingEffect::SetChanged {
                slots: self.next_state.slots().clone(),
                data: self.next_state.data(),
            });
        } else {
            self.events.push(BrewingMenuEvent::PlayerInventoryChanged {
                phase: self.phase(),
            });
        }
    }
    fn set_by_player(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        if slot < 5 {
            let effect = self.next_state.set_slot(slot, canonical(&stack))?;
            self.effect(effect);
            self.changed(slot);
        } else {
            self.next_inventory
                .set_slot_authoritative(slot + 4, canonical(&stack));
            self.changed(slot);
        }
        Ok(())
    }
    fn mutate_stack(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        if slot < 5 {
            self.next_state.set_slot_raw(slot, canonical(&stack))?;
        } else {
            self.next_inventory
                .set_slot_authoritative(slot + 4, canonical(&stack));
        }
        Ok(())
    }
    fn try_remove(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
    ) -> Result<ItemStack> {
        let source = self.slot(slot);
        if source.is_empty()
            || maximum <= 0
            || !self.may_place(slot, &source, owner)? && maximum < source.count
        {
            return Ok(ItemStack::empty());
        }
        let amount = amount.max(0).min(maximum).min(source.count);
        let taken = counted(&source, amount);
        let left = counted(&source, source.count.wrapping_sub(amount));
        if slot < 5 {
            self.next_state.set_slot_raw(slot, left.clone())?;
            if !taken.is_empty() {
                self.changed(slot);
            }
        } else {
            self.next_inventory
                .set_slot_authoritative(slot + 4, left.clone());
        }
        if !taken.is_empty() && left.is_empty() {
            self.set_by_player(slot, ItemStack::empty())?;
        }
        Ok(taken)
    }
    fn on_take(&mut self, slot: usize, stack: &ItemStack) -> Result<()> {
        if slot < 3 && self.server_player {
            if let Some(holder_id) = brewing::potion_take_criterion(stack)? {
                self.events.push(BrewingMenuEvent::PotionTaken {
                    holder_id,
                    stack: stack.clone(),
                    phase: self.phase(),
                });
            }
        }
        self.changed(slot);
        Ok(())
    }
    fn drop_stack(&mut self, stack: ItemStack, trace_item: bool) {
        if !stack.is_empty() {
            let index = self.outcome.drops.len();
            self.outcome.drops.push(DroppedStack {
                stack,
                around: false,
                trace_item,
            });
            self.events.push(BrewingMenuEvent::Drop {
                index,
                phase: self.phase(),
            });
        }
    }
}
fn canonical(stack: &ItemStack) -> ItemStack {
    if stack.is_empty() {
        ItemStack::empty()
    } else {
        stack.clone()
    }
}
fn counted(stack: &ItemStack, count: i32) -> ItemStack {
    if stack.is_empty() || count <= 0 {
        ItemStack::empty()
    } else {
        let mut stack = stack.clone();
        stack.count = count;
        stack
    }
}
fn reject_bundle(stack: &ItemStack) -> Result<()> {
    ensure!(
        inventory::item_name(stack.item_id) != Some("bundle"),
        "bundle clicks require actual feature/item owner"
    );
    Ok(())
}

impl<K: Clone + Eq + Debug, C: Clone + Eq + Debug> PreparedBrewingAction<K, C> {
    fn do_click(
        &mut self,
        slot: i32,
        button: i32,
        kind: ClickType,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        creative: bool,
    ) -> Result<()> {
        if kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, owner, creative);
        }
        if self.next_menu.drag_status != 0 {
            self.next_menu.reset_drag();
            return Ok(());
        }
        match kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let carried = self.next_menu.carried().clone();
                    let amount = if button == 0 {
                        carried.count
                    } else {
                        1.min(carried.count)
                    };
                    if button == 1 {
                        self.next_menu
                            .sync
                            .set_carried(counted(&carried, carried.count - amount));
                        self.drop_stack(counted(&carried, amount), true);
                    } else {
                        self.drop_stack(counted(&carried, amount), true);
                        self.next_menu.sync.set_carried(ItemStack::empty());
                    }
                } else if slot >= 0 {
                    if kind == ClickType::Pickup {
                        self.pickup(slot as usize, button, owner)?;
                    } else {
                        loop {
                            let source = self.slot(slot as usize);
                            if source.is_empty()
                                || !self.quick_move_once(slot as usize, owner)?
                                || self.slot(slot as usize).item_id != source.item_id
                            {
                                break;
                            }
                        }
                    }
                }
            }
            ClickType::Swap if slot >= 0 && ((0..9).contains(&button) || button == 40) => {
                self.swap(slot as usize, button, owner, creative)?
            }
            ClickType::Clone if slot >= 0 && creative && self.next_menu.carried().is_empty() => {
                let source = self.slot(slot as usize);
                reject_bundle(&source)?;
                self.next_menu
                    .sync
                    .set_carried(counted(&source, source.max_stack_size()?));
            }
            ClickType::Throw if slot >= 0 && self.next_menu.carried().is_empty() => {
                let slot = slot as usize;
                let source = self.slot(slot);
                reject_bundle(&source)?;
                let amount = if button == 0 { 1 } else { source.count };
                let taken = self.try_remove(slot, amount, i32::MAX, owner)?;
                if !taken.is_empty() {
                    self.on_take(slot, &taken)?;
                }
                self.drop_stack(taken, true);
            }
            ClickType::PickupAll if slot >= 0 => self.pickup_all(slot as usize, button, owner)?,
            _ => {}
        }
        Ok(())
    }
    fn pickup(
        &mut self,
        slot: usize,
        button: i32,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
    ) -> Result<()> {
        let source = self.slot(slot);
        let carried = self.next_menu.carried().clone();
        reject_bundle(&source)?;
        reject_bundle(&carried)?;
        if source.is_empty() {
            if !carried.is_empty() && self.may_place(slot, &carried, owner)? {
                let amount =
                    (if button == 0 { carried.count } else { 1 }).min(self.limit(slot, &carried)?);
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count - amount));
                self.set_by_player(slot, counted(&carried, amount))?;
            }
        } else if carried.is_empty() {
            let amount = if button == 0 {
                source.count
            } else {
                source.count.wrapping_add(1) / 2
            };
            let taken = self.try_remove(slot, amount, i32::MAX, owner)?;
            if !taken.is_empty() {
                // cpu pickup lambda installs the cursor before Slot.onTake.
                self.next_menu.sync.set_carried(taken.clone());
                self.on_take(slot, &taken)?;
            }
        } else if self.may_place(slot, &carried, owner)? {
            if source.same_item_same_components(&carried)? {
                // Slot.safeInsert preserves a negative capacity for an actual
                // overfull target: shrink(-n) grows the cursor, grow(-n) lowers
                // the slot. Even zero transfer still invokes setByPlayer.
                let amount = (if button == 0 { carried.count } else { 1 })
                    .min(self.limit(slot, &carried)?.wrapping_sub(source.count));
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count.wrapping_sub(amount)));
                self.set_by_player(slot, counted(&source, source.count.wrapping_add(amount)))?;
            } else if carried.count <= self.limit(slot, &carried)? {
                self.next_menu.sync.set_carried(source);
                self.set_by_player(slot, carried)?;
            }
        } else if source.same_item_same_components(&carried)? {
            let taken = self.try_remove(
                slot,
                source.count,
                carried.max_stack_size()?.wrapping_sub(carried.count),
                owner,
            )?;
            if !taken.is_empty() {
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count.wrapping_add(taken.count)));
                self.on_take(slot, &taken)?;
            }
        }
        // AbstractContainerMenu.doClick calls Slot.setChanged after pickup,
        // even if the insertion was refused or the occupied slot was full.
        self.changed(slot);
        Ok(())
    }
    fn move_to(
        &mut self,
        stack: &mut ItemStack,
        start: usize,
        end: usize,
        reverse: bool,
        source_slot: usize,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
    ) -> Result<bool> {
        let before = stack.count;
        let order: Vec<usize> = if reverse {
            (start..end).rev().collect()
        } else {
            (start..end).collect()
        };
        if stack.is_stackable()? {
            for &slot in &order {
                let existing = self.slot(slot);
                // The source merge pass does not call Slot.mayPlace. A loaded
                // forbidden stack may therefore still merge with itself.
                if !existing.is_empty() && existing.same_item_same_components(stack)? {
                    let amount = stack
                        .count
                        .min((self.limit(slot, stack)? - existing.count).max(0));
                    if amount > 0 {
                        *stack = counted(stack, stack.count - amount);
                        self.mutate_stack(source_slot, stack.clone())?;
                        self.mutate_stack(slot, counted(&existing, existing.count + amount))?;
                        self.changed(slot);
                    }
                    if stack.is_empty() {
                        break;
                    }
                }
            }
        }
        if !stack.is_empty() {
            for slot in order {
                if self.slot(slot).is_empty() && self.may_place(slot, stack, owner)? {
                    let amount = stack.count.min(self.limit(slot, stack)?);
                    let moved = counted(stack, amount);
                    *stack = counted(stack, stack.count - amount);
                    self.mutate_stack(source_slot, stack.clone())?;
                    self.set_by_player(slot, moved)?;
                    // cpu.moveItemStackTo calls Slot.setChanged once more
                    // after setByPlayer's own Container.setItem/setChanged.
                    self.changed(slot);
                    break;
                }
            }
        }
        Ok(stack.count != before)
    }
    fn quick_move_once(
        &mut self,
        slot: usize,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
    ) -> Result<bool> {
        let original = self.slot(slot);
        reject_bundle(&original)?;
        if original.is_empty() {
            return Ok(false);
        }
        let mut left = original.clone();
        if slot < 5 {
            if !self.move_to(&mut left, 5, 41, true, slot, owner)? {
                return Ok(false);
            }
            // Slot.onQuickCraft defaults to no-op for all three brewing slots.
        } else if self.may_place(4, &original, owner)? {
            // Pinned cqa76-123: a SUCCESSFUL fuel transfer returns EMPTY before
            // source cleanup/onTake. The moved prefix is still authoritative.
            if self.move_to(&mut left, 4, 5, false, slot, owner)? {
                return Ok(false);
            }
            if self.may_place(3, &left, owner)?
                && !self.move_to(&mut left, 3, 4, false, slot, owner)?
            {
                return Ok(false);
            }
        } else if self.may_place(3, &left, owner)? {
            if !self.move_to(&mut left, 3, 4, false, slot, owner)? {
                return Ok(false);
            }
        } else if self.may_place(0, &original, owner)? {
            if !self.move_to(&mut left, 0, 3, false, slot, owner)? {
                return Ok(false);
            }
        } else if (5..32).contains(&slot) {
            if !self.move_to(&mut left, 32, 41, false, slot, owner)? {
                return Ok(false);
            }
        } else if (32..41).contains(&slot) {
            if !self.move_to(&mut left, 5, 32, false, slot, owner)? {
                return Ok(false);
            }
        } else if !self.move_to(&mut left, 5, 41, false, slot, owner)? {
            return Ok(false);
        }
        if left.is_empty() {
            self.set_by_player(slot, ItemStack::empty())?;
        } else {
            self.changed(slot);
        }
        if left.count == original.count {
            return Ok(false);
        }
        // Brewing passes the ORIGINAL copy, so fully shifted potions still
        // carry their potion component at the actual criterion callback.
        self.on_take(slot, &original)?;
        Ok(true)
    }
    fn swap(
        &mut self,
        slot: usize,
        button: i32,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        creative: bool,
    ) -> Result<()> {
        let player_slot = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        let aliases_held = slot >= self.top_count() && 9 + slot - self.top_count() == player_slot;
        let source = self.slot(slot);
        let held = self.next_inventory.slot(player_slot).unwrap().clone();
        reject_bundle(&source)?;
        reject_bundle(&held)?;
        if source.is_empty() && held.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            self.next_inventory
                .set_slot_authoritative(player_slot, source.clone());
            self.set_by_player(slot, ItemStack::empty())?;
            self.on_take(slot, &source)?;
        } else if source.is_empty() {
            if self.may_place(slot, &held, owner)? {
                let amount = held.count.min(self.limit(slot, &held)?);
                self.next_inventory
                    .set_slot_authoritative(player_slot, counted(&held, held.count - amount));
                self.set_by_player(slot, counted(&held, amount))?;
            }
        } else if self.may_place(slot, &held, owner)? {
            let maximum = self.limit(slot, &held)?;
            if held.count > maximum {
                self.next_inventory
                    .set_slot_authoritative(player_slot, counted(&held, held.count - maximum));
                self.set_by_player(slot, counted(&held, maximum))?;
                // When source and held denote the SAME canonical player slot,
                // split already shrank the old source object to its remainder.
                let mut displaced = if aliases_held {
                    counted(&source, held.count - maximum)
                } else {
                    source
                };
                self.on_take(slot, &displaced)?;
                self.add_to_inventory(&mut displaced, creative)?;
                self.drop_stack(displaced, true);
            } else {
                self.next_inventory
                    .set_slot_authoritative(player_slot, source.clone());
                self.set_by_player(slot, held)?;
                self.on_take(slot, &source)?;
            }
        }
        Ok(())
    }
    fn add_to_inventory(&mut self, stack: &mut ItemStack, creative: bool) -> Result<()> {
        let order = (36..45).chain(9..36).collect::<Vec<_>>();
        let damaged = stack.is_damageable()? && stack.damage_value()? > 0;
        if damaged {
            if let Some(slot) = order
                .iter()
                .copied()
                .find(|&slot| self.next_inventory.slot(slot).unwrap().is_empty())
            {
                self.next_inventory
                    .set_slot_authoritative(slot, std::mem::take(stack));
            } else if creative {
                *stack = ItemStack::empty();
            }
            return Ok(());
        }
        let mut merges = vec![36 + self.next_inventory.selected_hotbar(), 45];
        merges.extend(order.iter().copied());
        for slot in merges {
            let existing = self.next_inventory.slot(slot).unwrap().clone();
            if !existing.is_empty()
                && existing.is_stackable()?
                && existing.same_item_same_components(stack)?
            {
                let amount = stack
                    .count
                    .min((99.min(stack.max_stack_size()?) - existing.count).max(0));
                if amount > 0 {
                    self.next_inventory
                        .set_slot_authoritative(slot, counted(&existing, existing.count + amount));
                    *stack = counted(stack, stack.count - amount);
                }
                if stack.is_empty() {
                    return Ok(());
                }
            }
        }
        for slot in order {
            if self.next_inventory.slot(slot).unwrap().is_empty() {
                let amount = stack.count.min(99.min(stack.max_stack_size()?));
                self.next_inventory
                    .set_slot_authoritative(slot, counted(stack, amount));
                *stack = counted(stack, stack.count - amount);
                if stack.is_empty() {
                    break;
                }
            }
        }
        if creative {
            *stack = ItemStack::empty();
        }
        Ok(())
    }
    fn pickup_all(
        &mut self,
        clicked: usize,
        button: i32,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
    ) -> Result<()> {
        if self.next_menu.carried().is_empty() || !self.slot(clicked).is_empty() {
            return Ok(());
        }
        let order: Vec<usize> = if button == 0 {
            (0..self.slot_count()).collect()
        } else {
            (0..self.slot_count()).rev().collect()
        };
        for pass in 0..2 {
            for &slot in &order {
                let source = self.slot(slot);
                let carried = self.next_menu.carried().clone();
                if carried.count >= carried.max_stack_size()? {
                    break;
                }
                if source.is_empty()
                    || !source.same_item_same_components(&carried)?
                    || source.count > carried.max_stack_size()?
                    || pass == 0 && source.count == source.max_stack_size()?
                {
                    continue;
                }
                reject_bundle(&source)?;
                let taken = self.try_remove(
                    slot,
                    source.count,
                    carried.max_stack_size()?.wrapping_sub(carried.count),
                    owner,
                )?;
                if !taken.is_empty() {
                    self.on_take(slot, &taken)?;
                }
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count.wrapping_add(taken.count)));
            }
        }
        Ok(())
    }
    fn quickcraft(
        &mut self,
        slot: i32,
        button: i32,
        owner: &impl BrewingMenuOwner<Key = K, ContextKey = C>,
        creative: bool,
    ) -> Result<()> {
        let previous = self.next_menu.drag_status;
        self.next_menu.drag_status = (button & 3) as u8;
        if !(previous == 1 && self.next_menu.drag_status == 2)
            && previous != self.next_menu.drag_status
        {
            self.next_menu.reset_drag();
            return Ok(());
        }
        if self.next_menu.carried().is_empty() {
            self.next_menu.reset_drag();
            return Ok(());
        }
        reject_bundle(self.next_menu.carried())?;
        match self.next_menu.drag_status {
            0 => {
                self.next_menu.drag_type = ((button >> 2) & 3) as u8;
                if self.next_menu.drag_type <= 1 || self.next_menu.drag_type == 2 && creative {
                    self.next_menu.drag_status = 1;
                    self.next_menu.drag_slots.clear();
                } else {
                    self.next_menu.reset_drag();
                }
            }
            1 => {
                if slot >= 0 {
                    let slot = slot as usize;
                    let source = self.slot(slot);
                    let carried = self.next_menu.carried();
                    if self.may_place(slot, carried, owner)?
                        && (source.is_empty()
                            || source.same_item_same_components(carried)?
                                && source.count <= carried.max_stack_size()?)
                        && (self.next_menu.drag_type == 2
                            || carried.count > self.next_menu.drag_slots.len() as i32)
                    {
                        self.next_menu.drag_slots.insert(slot);
                    }
                }
            }
            2 => {
                if self.next_menu.drag_slots.len() == 1 {
                    let slot = *self.next_menu.drag_slots.iter().next().unwrap();
                    let kind = self.next_menu.drag_type;
                    self.next_menu.reset_drag();
                    return self.do_click(
                        slot as i32,
                        kind as i32,
                        ClickType::Pickup,
                        owner,
                        creative,
                    );
                }
                if self.next_menu.drag_slots.len() > 1
                    && self
                        .next_menu
                        .drag_slots
                        .iter()
                        .any(|&slot| slot < self.top_count())
                {
                    ensure!(owner.quickcraft_callbacks_are_order_independent()?,
                        "multi-slot brewing drag requires actual callback-order independence or a native Slot identity-order owner");
                    self.require_order_independence = true;
                }
                let carried = self.next_menu.carried().clone();
                let mut left = carried.count;
                let slots = self
                    .next_menu
                    .drag_slots
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                let each = match self.next_menu.drag_type {
                    0 => ((carried.count as f32) / (slots.len().max(1) as f32)).floor() as i32,
                    1 => 1,
                    2 => carried.max_stack_size()?,
                    _ => 0,
                };
                for slot in slots {
                    let source = self.slot(slot);
                    if !self.may_place(slot, &carried, owner)?
                        || !source.is_empty()
                            && (!source.same_item_same_components(&carried)?
                                || source.count > carried.max_stack_size()?)
                        || self.next_menu.drag_type != 2
                            && carried.count < self.next_menu.drag_slots.len() as i32
                    {
                        continue;
                    }
                    let count = each
                        .wrapping_add(source.count)
                        .min(self.limit(slot, &carried)?);
                    left = left.wrapping_sub(count.wrapping_sub(source.count));
                    self.set_by_player(slot, counted(&carried, count))?;
                }
                self.next_menu.sync.set_carried(counted(&carried, left));
                self.next_menu.reset_drag();
            }
            _ => self.next_menu.reset_drag(),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crafting::RecipeBook, inventory::Component, protocol::Writer,
        workstations::brewing::BrewingSaved,
    };
    use std::collections::BTreeMap;

    // These are explicitly isolated in-memory owners. They establish the
    // controller's transaction/source phase, not production recipes or a world.
    struct FixturePotions;
    impl PotionBrewingOwner for FixturePotions {
        fn is_ingredient(&self, stack: &ItemStack) -> Result<bool> {
            Ok(["sugar", "gunpowder", "blaze_powder"]
                .iter()
                .any(|name| Some(stack.item_id) == RecipeBook::vanilla().item_id(name)))
        }
        fn has_mix(&self, bottle: &ItemStack, _: &ItemStack) -> Result<bool> {
            Ok(Some(bottle.item_id) == RecipeBook::vanilla().item_id("potion"))
        }
        fn mix(&self, _: &ItemStack, _: &ItemStack) -> Result<ItemStack> {
            anyhow::bail!("menu must not run the actual world mix phase")
        }
    }
    struct FixtureOwner {
        key: &'static str,
        context: &'static str,
        state: BrewingState,
        revision: u64,
        potions: FixturePotions,
        reject: bool,
        independent: bool,
        events: Vec<BrewingMenuEvent<&'static str>>,
        drops: Vec<DroppedStack>,
        player_changes: usize,
    }
    impl FixtureOwner {
        fn new() -> Self {
            Self {
                key: "actual-fixture-block-a",
                context: "actual-fixture-potions-a",
                state: BrewingState::default(),
                revision: 0,
                potions: FixturePotions,
                reject: false,
                independent: true,
                events: vec![],
                drops: vec![],
                player_changes: 0,
            }
        }
    }
    impl BrewingMenuOwner for FixtureOwner {
        type Key = &'static str;
        type ContextKey = &'static str;
        type Potions = FixturePotions;
        fn owner_key(&self) -> &Self::Key {
            &self.key
        }
        fn potion_context_key(&self) -> &Self::ContextKey {
            &self.context
        }
        fn potion_brewing(&self) -> &Self::Potions {
            &self.potions
        }
        fn state_revision(&self) -> u64 {
            self.revision
        }
        fn brewing_state(&self) -> &BrewingState {
            &self.state
        }
        fn still_valid_for_actual_player(&self) -> Result<bool> {
            Ok(true)
        }
        fn actual_player_is_server_player(&self) -> bool {
            true
        }
        fn quickcraft_callbacks_are_order_independent(&self) -> Result<bool> {
            Ok(self.independent)
        }
        fn commit_prepared_brewing_menu(
            &mut self,
            expected_revision: u64,
            context: &Self::ContextKey,
            before: &BrewingState,
            after: &BrewingState,
            events: &[BrewingMenuEvent<Self::Key>],
            drops: &[DroppedStack],
        ) -> Result<()> {
            ensure!(!self.reject, "fixture refuses the entire transaction");
            ensure!(
                self.revision == expected_revision
                    && self.context == *context
                    && self.state == *before,
                "fixture owner changed"
            );
            let changed = before != after;
            // Commit the ordered stream once. The receipt is never replayed.
            for event in events {
                match event {
                    BrewingMenuEvent::PlayerInventoryChanged { .. } => self.player_changes += 1,
                    BrewingMenuEvent::Drop { index, .. } => self.drops.push(drops[*index].clone()),
                    _ => {}
                }
            }
            self.events = events.to_vec();
            self.state = after.clone();
            self.revision += u64::from(changed);
            Ok(())
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn potion() -> ItemStack {
        let mut stack = item("potion", 1);
        let mut data = Writer::new();
        data.bool(true).varint(1).bool(false).varint(0);
        stack.components.push(Component {
            type_id: 31,
            data: data.into_inner(),
        });
        stack
    }
    fn click(
        menu: &BrewingMenu<&'static str, &'static str>,
        slot: i16,
        button: i8,
        kind: ClickType,
    ) -> ContainerClick {
        ContainerClick {
            window_id: menu.window_id(),
            state_id: menu.state_id(),
            slot,
            button,
            kind,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        }
    }
    fn action(
        menu: &mut BrewingMenu<&'static str, &'static str>,
        owner: &mut FixtureOwner,
        inventory: &mut PlayerInventory,
        slot: i16,
        button: i8,
        kind: ClickType,
    ) -> Result<BrewingCommitReceipt<&'static str>> {
        menu.prepare_click(&click(menu, slot, button, kind), owner, inventory, false)?
            .commit(menu, owner, inventory)
    }
    fn dirty_count(events: &[BrewingMenuEvent<&'static str>]) -> usize {
        events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    BrewingMenuEvent::Effect {
                        effect: BrewingEffect::SetChanged { .. },
                        ..
                    }
                )
            })
            .count()
    }
    #[test]
    fn persistent_brewing_menu_criterion_fuel_prefix_cursor_and_owner_transactions() -> Result<()> {
        let mut owner = FixtureOwner::new();
        let mut slots = std::array::from_fn(|_| ItemStack::empty());
        slots[0] = potion();
        slots[3] = item("sugar", 3);
        owner.state = BrewingState::from_saved(BrewingSaved {
            slots,
            brew_time: 5,
            fuel: 9,
        });
        // Seed the fixture's legitimate post-tick transient bottle cache. This
        // is pure native planning, not an executed Java or production tick.
        owner.state = owner
            .state
            .prepare_tick(&owner.potions, true)?
            .next_state()
            .clone();
        assert_eq!(owner.state.data(), [4, 9]);
        let mut inventory = PlayerInventory::new();
        inventory.set_slot_authoritative(9, item("blaze_powder", 5));
        inventory.set_slot_authoritative(11, item("glass_bottle", 3));
        inventory.set_slot_authoritative(13, item("blaze_powder", 4));
        inventory.set_slot_authoritative(36, item("gunpowder", 2));
        let mut menu = BrewingMenu::new(5, &owner)?;
        let initial = menu.initialize(&owner, &inventory)?;
        let content = initial
            .iter()
            .find_map(|event| match event {
                BrewingMenuEvent::Sync {
                    event: SyncEvent::Packet(packet),
                    ..
                } if packet.id == 0x13 => Some(packet),
                _ => None,
            })
            .expect("full brewing content packet");
        let mut reader = Reader::new(&content.payload);
        assert_eq!(reader.u8()?, 5);
        reader.varint()?;
        assert_eq!(reader.varint()?, 41); // actual five + 36 player layout
                                          // Successful fuel shift commits the raw source mutation but returns
                                          // EMPTY before source dirty/onTake; no counterfeit player counter.
        let receipt = action(
            &mut menu,
            &mut owner,
            &mut inventory,
            5,
            0,
            ClickType::QuickMove,
        )?;
        assert_eq!(owner.state.slots()[4], item("blaze_powder", 5));
        assert!(inventory.slot(9).unwrap().is_empty());
        assert_eq!(owner.player_changes, 0);
        assert_eq!(dirty_count(&receipt.events), 3);
        assert_eq!(owner.state.data(), [4, 9]);
        assert!(!owner
            .state
            .prepare_tick(&owner.potions, true)?
            .effects()
            .iter()
            .any(|effect| matches!(effect, BrewingEffect::SetBottleProperties { .. })));
        // Fully shifted bottle takes ORIGINAL potion copy, not empty remainder.
        let receipt = action(
            &mut menu,
            &mut owner,
            &mut inventory,
            0,
            0,
            ClickType::QuickMove,
        )?;
        assert!(owner.state.slots()[0].is_empty());
        assert_eq!(inventory.slot(44).unwrap(), &potion());
        assert_eq!(dirty_count(&receipt.events), 3);
        let criterion = receipt
            .events
            .iter()
            .position(|event| {
                matches!(event,
            BrewingMenuEvent::PotionTaken { holder_id: 1, stack, .. } if stack == &potion())
            })
            .unwrap();
        assert!(matches!(
            receipt.events[criterion + 1],
            BrewingMenuEvent::Effect { .. }
        ));
        if let BrewingMenuEvent::PotionTaken { phase, .. } = &receipt.events[criterion] {
            assert!(phase.brewing.slots()[0].is_empty());
            assert_eq!(phase.inventory.slot(44).unwrap(), &potion());
            assert_eq!(phase.brewing.data(), [4, 9]);
        }
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            7,
            0,
            ClickType::QuickMove,
        )?;
        assert_eq!(
            owner.state.slots()[0..3],
            [
                item("glass_bottle", 1),
                item("glass_bottle", 1),
                item("glass_bottle", 1)
            ]
        );
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            0,
            0,
            ClickType::Pickup,
        )?;
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            0,
            0,
            ClickType::PickupAll,
        )?;
        assert_eq!(menu.carried(), &item("glass_bottle", 3));
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            1,
            1,
            ClickType::Pickup,
        )?;
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            2,
            1,
            ClickType::Pickup,
        )?;
        let dropped = action(
            &mut menu,
            &mut owner,
            &mut inventory,
            -999,
            0,
            ClickType::Pickup,
        )?;
        assert_eq!(dropped.outcome.drops[0].stack, item("glass_bottle", 1));
        assert_eq!(owner.drops.len(), 1); // receipt does not execute a second drop
        if let BrewingMenuEvent::Drop { phase, .. } = dropped
            .events
            .iter()
            .find(|event| matches!(event, BrewingMenuEvent::Drop { .. }))
            .unwrap()
        {
            assert_eq!(phase.carried, item("glass_bottle", 1)); // whole outside: drop before clear
        }
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            40,
            0,
            ClickType::QuickMove,
        )?;
        assert_eq!(owner.state.slots()[0], potion());
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            9,
            0,
            ClickType::Pickup,
        )?;
        for (slot, button) in [(-999, 0), (4, 1), (5, 1), (-999, 2)] {
            action(
                &mut menu,
                &mut owner,
                &mut inventory,
                slot,
                button,
                ClickType::QuickCraft,
            )?;
        }
        assert_eq!(owner.state.slots()[4], item("blaze_powder", 7));
        assert_eq!(inventory.slot(9).unwrap(), &item("blaze_powder", 2));
        assert!(menu.carried().is_empty());
        // Creative Clone is prepared on the same authoritative UI, then dropped
        // uncommitted so this source-flow's item conservation stays explicit.
        let cloned = menu.prepare_click(
            &click(&menu, 1, 2, ClickType::Clone),
            &owner,
            &inventory,
            true,
        )?;
        assert_eq!(cloned.next_menu.carried(), &item("glass_bottle", 64));
        assert!(menu.carried().is_empty());
        action(&mut menu, &mut owner, &mut inventory, 3, 0, ClickType::Swap)?;
        assert_eq!(owner.state.slots()[3], item("gunpowder", 2));
        assert_eq!(inventory.slot(36).unwrap(), &item("sugar", 3));
        // Menu changes preserve the ingredient cache, so actual next tick aborts.
        assert_eq!(
            owner
                .state
                .prepare_tick(&owner.potions, true)?
                .next_state()
                .data()[0],
            0
        );
        let snapshot = owner.state.clone();
        let pending = menu.prepare_click(
            &click(&menu, 1, 0, ClickType::Throw),
            &owner,
            &inventory,
            false,
        )?;
        owner.reject = true;
        assert!(pending
            .commit(&mut menu, &mut owner, &mut inventory)
            .is_err());
        assert_eq!(owner.state, snapshot);
        assert_eq!(owner.drops.len(), 1);
        owner.reject = false;
        let pending = menu.prepare_click(
            &click(&menu, 1, 0, ClickType::Throw),
            &owner,
            &inventory,
            false,
        )?;
        owner.revision += 1; // actual ticker/concurrent owner mutation invalidates UI action
        assert!(pending.validate(&menu, &owner, &inventory).is_err());
        let receipt = action(
            &mut menu,
            &mut owner,
            &mut inventory,
            0,
            0,
            ClickType::Pickup,
        )?;
        assert!(receipt
            .events
            .iter()
            .any(|event| matches!(event, BrewingMenuEvent::PotionTaken { holder_id: 1, .. })));
        let persistent = owner.state.clone();
        let old_action = menu.prepare_click(
            &click(&menu, -999, 1, ClickType::Pickup),
            &owner,
            &inventory,
            false,
        )?;
        let old_close =
            menu.prepare_close(&owner, &inventory, MenuCloseContext::DeadOrDisconnected)?;
        menu.prepare_close(&owner, &inventory, MenuCloseContext::DeadOrDisconnected)?
            .commit(&mut menu, &mut owner, &mut inventory)?;
        assert_eq!(owner.state, persistent); // cursor only; real block contents retained
        assert_eq!(owner.drops.len(), 2);
        assert_eq!(owner.drops[1].stack, potion());
        let reopened = BrewingMenu::new(5, &owner)?;
        assert_ne!(menu.instance_id(), reopened.instance_id());
        assert!(old_action.validate(&reopened, &owner, &inventory).is_err());
        assert!(old_close.validate(&reopened, &owner, &inventory).is_err());
        // No implicit block lifecycle or world tick ever appears in the stream.
        assert!(owner.events.iter().all(|event| !matches!(
            event,
            BrewingMenuEvent::Effect {
                effect: BrewingEffect::SetBottleProperties { .. },
                ..
            }
        )));
        Ok(())
    }

    #[test]
    fn signed_insert_alias_callbacks_and_component_identity_boundary() -> Result<()> {
        let mut owner = FixtureOwner::new();
        let mut inventory = PlayerInventory::new();
        inventory.set_slot_authoritative(9, item("stone", 99));
        inventory.set_slot_authoritative(36, item("stone", 99));
        let mut menu = BrewingMenu::new(6, &owner)?;
        menu.initialize(&owner, &inventory)?;
        menu.sync.set_carried(item("stone", 1));
        let receipt = action(
            &mut menu,
            &mut owner,
            &mut inventory,
            5,
            0,
            ClickType::Pickup,
        )?;
        assert_eq!(inventory.slot(9).unwrap(), &item("stone", 64));
        assert_eq!(menu.carried(), &item("stone", 36));
        assert_eq!(
            receipt
                .events
                .iter()
                .filter(|event| matches!(event, BrewingMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            -999,
            0,
            ClickType::Pickup,
        )?;
        let before = inventory
            .slots()
            .iter()
            .filter(|stack| stack.item_id == item("stone", 1).item_id)
            .map(|stack| stack.count)
            .sum::<i32>();
        let receipt = action(
            &mut menu,
            &mut owner,
            &mut inventory,
            32,
            0,
            ClickType::Swap,
        )?;
        let after = inventory
            .slots()
            .iter()
            .filter(|stack| stack.item_id == item("stone", 1).item_id)
            .map(|stack| stack.count)
            .sum::<i32>();
        assert_eq!(before, after); // overfull self-alias returns only displaced remainder
        assert_eq!(inventory.slot(36).unwrap(), &item("stone", 64));
        assert_eq!(
            receipt
                .events
                .iter()
                .filter(|event| matches!(event, BrewingMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        let receipt = action(
            &mut menu,
            &mut owner,
            &mut inventory,
            32,
            0,
            ClickType::Swap,
        )?;
        assert_eq!(
            receipt
                .events
                .iter()
                .filter(|event| matches!(event, BrewingMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        // Java FoodProperties contains Optional<ItemStack> reference identity.
        // The shared semantic JSON comparison cannot certify that owner.
        let mut food = item("apple", 1);
        food.components.push(Component {
            type_id: 20,
            data: food.effective_component_bytes(20).unwrap().to_vec(),
        });
        let snapshot = owner.state.clone();
        inventory.set_slot_authoritative(10, food.clone());
        assert!(menu
            .prepare_click(
                &click(&menu, 5, 0, ClickType::Pickup),
                &owner,
                &inventory,
                false
            )
            .is_err());
        assert_eq!(owner.state, snapshot);
        inventory.set_slot_authoritative(10, ItemStack::empty());
        menu.sync.set_carried(food.clone());
        assert!(menu
            .prepare_click(
                &click(&menu, 5, 0, ClickType::Pickup),
                &owner,
                &inventory,
                false
            )
            .is_err());
        menu.sync.set_carried(ItemStack::empty());
        owner.state.set_slot_raw(3, food)?;
        assert!(menu.broadcast_changes(&owner, &inventory).is_err());
        assert_eq!(owner.drops.len(), 1);
        Ok(())
    }
}
