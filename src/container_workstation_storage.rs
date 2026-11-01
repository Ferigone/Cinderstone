//! Java 1.21 HopperMenu, DispenserMenu and ShulkerBoxMenu transactions.
//!
//! This owner borrows the actual block container and player inventory. It does
//! not implement or replace chest/barrel menus, create block identities, retain
//! a second block inventory, or substitute world/RNG/lifecycle callbacks.
use crate::{
    container::{
        ClickOutcome, ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer,
    },
    inventory::{self, ItemStack, PlayerInventory},
    workstations::menu::{MenuRemovalEvent, MenuSync, SyncEvent, WorkstationKind},
};
use anyhow::{ensure, Result};
use std::{collections::BTreeSet, fmt::Debug};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageMenuKind {
    Hopper,
    Dispenser,
    Shulker,
}
impl StorageMenuKind {
    pub fn top_slots(self) -> usize {
        match self {
            Self::Hopper => 5,
            Self::Dispenser => 9,
            Self::Shulker => 27,
        }
    }
    pub fn protocol_id(self) -> i32 {
        self.sync_kind().protocol_id()
    }
    fn sync_kind(self) -> WorkstationKind {
        match self {
            Self::Hopper => WorkstationKind::Hopper,
            Self::Dispenser => WorkstationKind::Dispenser,
            Self::Shulker => WorkstationKind::Shulker,
        }
    }
}
/// Implement on the real canonical block-container owner, with the actual player
/// captured by the validity callback. Key must identify that actual object,
/// including its lifetime across replacement at identical coordinates. Revision
/// changes on content mutations; lifecycle counters have their own real owner.
///
/// install_prepared_contents is an atomic native commit of an already staged
/// image. It must not replay source callbacks: the ordered operation trace has
/// already been prepared against the real host/world owners. Failure changes
/// no contents. The integrator supplies implementations, not this menu module.
pub trait StorageContainer {
    type Key: Clone + Eq + Debug;
    fn owner_key(&self) -> &Self::Key;
    fn kind(&self) -> StorageMenuKind;
    fn content_revision(&self) -> u64;
    fn slots(&self) -> &[ItemStack];
    fn maximum_stack_size(&self) -> i32;
    fn still_valid_for_actual_player(&self) -> Result<bool>;
    fn install_prepared_contents(
        &mut self,
        expected_revision: u64,
        expected: &[ItemStack],
        next: &[ItemStack],
    ) -> Result<()>;
    /// Original ShulkerBoxSlot calls Item.canFitInsideContainerItems. Override
    /// this for an actual custom registry; the default is the pinned item set.
    fn can_fit_inside_container_items(&self, stack: &ItemStack) -> Result<bool> {
        pinned_can_fit_inside_container_items(stack)
    }
    /// Java iterates an identity-based HashSet of selected Slot objects. A
    /// native multi-slot batch uses index order only when the actual host
    /// certifies its dirty/comparator callbacks are insensitive to that order.
    /// Unknown or order-sensitive receivers must remain an explicit boundary.
    fn quickcraft_callbacks_are_order_independent(&self) -> Result<bool>;
}
/// Item returns true; BlockItem returns false only for ShulkerBoxBlock. The
/// canonical shipped block-state names identify those 17 vanilla block items.
pub fn pinned_can_fit_inside_container_items(stack: &ItemStack) -> Result<bool> {
    if stack.is_empty() {
        return Ok(true);
    }
    ensure!(
        inventory::item_name(stack.item_id).is_some(),
        "unknown item class requires actual registry owner"
    );
    if let Some(state) = inventory::item_default_block_state(stack.item_id) {
        let name = inventory::block_state_name(state)
            .ok_or_else(|| anyhow::anyhow!("unknown block item class"))?;
        return Ok(name != "shulker_box" && !name.ends_with("_shulker_box"));
    }
    Ok(true)
}
/// Source container methods, plus in-place mutations of its actual ItemStack.
/// Snapshots accompanying these operations are observations, not owned copies.
#[derive(Clone, Debug)]
pub enum StorageOperation {
    SetItem {
        slot: usize,
        stack: ItemStack,
    },
    MutateStack {
        slot: usize,
        stack: ItemStack,
    },
    RemoveItem {
        slot: usize,
        amount: i32,
        removed: ItemStack,
    },
    SetChanged,
}
#[derive(Clone, Debug)]
pub enum StorageMenuEvent<K> {
    StartOpen {
        owner: K,
    },
    StopOpen {
        owner: K,
    },
    Container {
        owner: K,
        operation: StorageOperation,
        slots_after: Vec<ItemStack>,
    },
    Sync(SyncEvent),
    /// Actual Inventory.setChanged (cmw timesChanged), distinct from the local
    /// menu listener and packet baseline. The host owns this player counter.
    PlayerInventoryChanged,
    /// Index into publication.outcome.drops; execute exactly in this position.
    Drop {
        index: usize,
    },
}
#[derive(Debug)]
pub struct StoragePublication<K> {
    pub events: Vec<StorageMenuEvent<K>>,
    pub outcome: ClickOutcome,
}
#[derive(Clone, Debug)]
pub struct StorageMenu<K> {
    sync: MenuSync,
    owner_key: K,
    kind: StorageMenuKind,
    revision: u64,
    active: bool,
    drag_status: u8,
    drag_type: u8,
    drag_slots: BTreeSet<usize>,
}
pub struct PreparedStorageOpen<K> {
    menu: StorageMenu<K>,
    source_revision: u64,
    source_slots: Vec<ItemStack>,
    events: Vec<StorageMenuEvent<K>>,
}
impl<K: Clone + Eq + Debug> PreparedStorageOpen<K> {
    pub fn events(&self) -> &[StorageMenuEvent<K>] {
        &self.events
    }
    pub fn validate(&self, owner: &impl StorageContainer<Key = K>) -> Result<()> {
        ensure!(
            owner.owner_key() == &self.menu.owner_key
                && owner.kind() == self.menu.kind
                && owner.content_revision() == self.source_revision
                && owner.slots() == self.source_slots,
            "storage owner changed during open preparation"
        );
        Ok(())
    }
    /// The host must stage StartOpen with its actual container/player/world
    /// owners before commit. Main's OpenScreen and initial sync follow it.
    pub fn commit(
        self,
        owner: &impl StorageContainer<Key = K>,
    ) -> Result<(StorageMenu<K>, Vec<StorageMenuEvent<K>>)> {
        self.validate(owner)?;
        Ok((self.menu, self.events))
    }
}
pub struct PreparedStorageAction<K> {
    expected_menu: StorageMenu<K>,
    expected_inventory: PlayerInventory,
    expected_revision: u64,
    expected_top: Vec<ItemStack>,
    next_menu: StorageMenu<K>,
    next_inventory: PlayerInventory,
    next_top: Vec<ItemStack>,
    top_maximum: i32,
    require_valid: bool,
    require_order_independence: bool,
    events: Vec<StorageMenuEvent<K>>,
    outcome: ClickOutcome,
}
impl<K: Clone + Eq + Debug> StorageMenu<K> {
    pub fn prepare_open(
        window_id: u8,
        owner: &impl StorageContainer<Key = K>,
    ) -> Result<PreparedStorageOpen<K>> {
        validate_owner(owner)?;
        let kind = owner.kind();
        let key = owner.owner_key().clone();
        let menu = Self {
            sync: MenuSync::new(window_id, kind.sync_kind())?,
            owner_key: key.clone(),
            kind,
            revision: 0,
            active: true,
            drag_status: 0,
            drag_type: 0,
            drag_slots: BTreeSet::new(),
        };
        Ok(PreparedStorageOpen {
            menu,
            source_revision: owner.content_revision(),
            source_slots: owner.slots().to_vec(),
            events: vec![StorageMenuEvent::StartOpen { owner: key }],
        })
    }
    pub fn window_id(&self) -> u8 {
        self.sync.window_id()
    }
    pub fn state_id(&self) -> i32 {
        self.sync.state_id()
    }
    pub fn kind(&self) -> StorageMenuKind {
        self.kind
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
            .ok_or_else(|| anyhow::anyhow!("storage menu revision exhausted"))?;
        Ok(())
    }
    fn check_owner(&self, owner: &impl StorageContainer<Key = K>) -> Result<()> {
        ensure!(self.active, "storage menu has been removed");
        validate_owner(owner)?;
        ensure!(
            owner.owner_key() == &self.owner_key && owner.kind() == self.kind,
            "storage menu cannot rebind to another block-container object"
        );
        Ok(())
    }
    pub fn still_valid(&self, owner: &impl StorageContainer<Key = K>) -> Result<bool> {
        self.check_owner(owner)?;
        owner.still_valid_for_actual_player()
    }
    pub fn initialize(
        &mut self,
        owner: &impl StorageContainer<Key = K>,
        inventory: &PlayerInventory,
    ) -> Result<Vec<SyncEvent>> {
        self.check_owner(owner)?;
        let mut next = self.clone();
        let out = next.sync.initialize(owner.slots(), inventory, &[])?;
        next.advance_revision()?;
        *self = next;
        Ok(out)
    }
    pub fn broadcast_changes(
        &mut self,
        owner: &impl StorageContainer<Key = K>,
        inventory: &PlayerInventory,
    ) -> Result<Vec<SyncEvent>> {
        self.check_owner(owner)?;
        let mut next = self.clone();
        let out = next.sync.broadcast_changes(owner.slots(), inventory, &[])?;
        next.advance_revision()?;
        *self = next;
        Ok(out)
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    fn prepare(
        &self,
        owner: &impl StorageContainer<Key = K>,
        inventory: &PlayerInventory,
        require_valid: bool,
    ) -> Result<PreparedStorageAction<K>> {
        self.check_owner(owner)?;
        if require_valid {
            ensure!(
                owner.still_valid_for_actual_player()?,
                "actual storage container is no longer valid for this player"
            );
        }
        Ok(PreparedStorageAction {
            expected_menu: self.clone(),
            expected_inventory: inventory.clone(),
            expected_revision: owner.content_revision(),
            expected_top: owner.slots().to_vec(),
            next_menu: self.clone(),
            next_inventory: inventory.clone(),
            next_top: owner.slots().to_vec(),
            top_maximum: owner.maximum_stack_size(),
            require_valid,
            require_order_independence: false,
            events: vec![],
            outcome: ClickOutcome::default(),
        })
    }
    pub fn prepare_click(
        &self,
        click: &ContainerClick,
        owner: &impl StorageContainer<Key = K>,
        inventory: &PlayerInventory,
        creative: bool,
    ) -> Result<PreparedStorageAction<K>> {
        ensure!(
            click.window_id == self.window_id() && click.slot < (self.kind.top_slots() + 36) as i16,
            "invalid storage menu click target"
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
            &action.next_top,
            &action.next_inventory,
            &[],
        )?;
        action
            .events
            .extend(events.into_iter().map(StorageMenuEvent::Sync));
        action.next_menu.advance_revision()?;
        Ok(action)
    }
    /// Only the actual cursor is returned/dropped; all block slots remain owned
    /// by their container. stopOpen follows super.removed's cursor effects.
    pub fn prepare_close(
        &self,
        owner: &impl StorageContainer<Key = K>,
        inventory: &PlayerInventory,
        context: MenuCloseContext,
    ) -> Result<PreparedStorageAction<K>> {
        let mut action = self.prepare(owner, inventory, false)?;
        action.next_menu.reset_drag();
        let removal = action.next_menu.sync.close_inputs_ordered(
            &mut [],
            &mut action.next_inventory,
            context,
        )?;
        action.outcome = removal.outcome;
        for event in removal.events {
            match event {
                MenuRemovalEvent::Packet { index } => action.events.push(StorageMenuEvent::Sync(
                    SyncEvent::Packet(action.outcome.packets[index].clone()),
                )),
                MenuRemovalEvent::Drop { index } => {
                    action.events.push(StorageMenuEvent::Drop { index })
                }
            }
        }
        action.outcome.packets.clear();
        action.events.push(StorageMenuEvent::StopOpen {
            owner: self.owner_key.clone(),
        });
        action.next_menu.active = false;
        action.next_menu.advance_revision()?;
        Ok(action)
    }
    fn reset_drag(&mut self) {
        self.drag_status = 0;
        self.drag_slots.clear();
    }
}
fn validate_owner(owner: &impl StorageContainer) -> Result<()> {
    ensure!(
        owner.slots().len() == owner.kind().top_slots(),
        "actual storage container size mismatch"
    );
    ensure!(
        (1..=99).contains(&owner.maximum_stack_size()),
        "unsupported actual storage stack limit"
    );
    for stack in owner.slots() {
        stack.encode_canonical()?;
    }
    Ok(())
}
impl<K: Clone + Eq + Debug> PreparedStorageAction<K> {
    pub fn events(&self) -> &[StorageMenuEvent<K>] {
        &self.events
    }
    pub fn drops(&self) -> &[DroppedStack] {
        &self.outcome.drops
    }
    pub fn next_slots(&self) -> &[ItemStack] {
        &self.next_top
    }
    pub fn next_inventory(&self) -> &PlayerInventory {
        &self.next_inventory
    }
    pub fn validate(
        &self,
        menu: &StorageMenu<K>,
        owner: &impl StorageContainer<Key = K>,
        inventory: &PlayerInventory,
    ) -> Result<()> {
        ensure!(
            menu.owner_key == self.expected_menu.owner_key
                && menu.kind == self.expected_menu.kind
                && menu.revision == self.expected_menu.revision
                && menu.sync == self.expected_menu.sync
                && menu.active == self.expected_menu.active
                && menu.drag_status == self.expected_menu.drag_status
                && menu.drag_type == self.expected_menu.drag_type
                && menu.drag_slots == self.expected_menu.drag_slots,
            "storage menu changed after action preparation"
        );
        menu.check_owner(owner)?;
        ensure!(
            owner.content_revision() == self.expected_revision
                && owner.slots() == self.expected_top
                && owner.maximum_stack_size() == self.top_maximum,
            "canonical storage container changed after preparation"
        );
        ensure!(
            inventory.slots() == self.expected_inventory.slots()
                && inventory.carried() == self.expected_inventory.carried()
                && inventory.selected_hotbar() == self.expected_inventory.selected_hotbar()
                && inventory.state_id() == self.expected_inventory.state_id(),
            "canonical player inventory changed after preparation"
        );
        if self.require_valid {
            ensure!(
                owner.still_valid_for_actual_player()?,
                "actual player lost storage access during preparation"
            );
        }
        if self.require_order_independence {
            ensure!(
                owner.quickcraft_callbacks_are_order_independent()?,
                "storage callback receiver context changed after drag preparation"
            );
        }
        Ok(())
    }
    /// Stage actual callback/world/drop owners using events first, validate,
    /// then commit once. Packets and indexed drops publish in events order.
    pub fn commit(
        self,
        menu: &mut StorageMenu<K>,
        owner: &mut impl StorageContainer<Key = K>,
        inventory: &mut PlayerInventory,
    ) -> Result<StoragePublication<K>> {
        self.validate(menu, owner, inventory)?;
        if self.expected_top != self.next_top {
            owner.install_prepared_contents(
                self.expected_revision,
                &self.expected_top,
                &self.next_top,
            )?;
        }
        *menu = self.next_menu;
        *inventory = self.next_inventory;
        Ok(StoragePublication {
            events: self.events,
            outcome: self.outcome,
        })
    }
    fn top_count(&self) -> usize {
        self.next_menu.kind.top_slots()
    }
    fn slot_count(&self) -> usize {
        self.top_count() + 36
    }
    fn slot(&self, slot: usize) -> ItemStack {
        if slot < self.top_count() {
            self.next_top[slot].clone()
        } else {
            self.next_inventory
                .slot(9 + slot - self.top_count())
                .expect("validated player menu slot")
                .clone()
        }
    }
    fn may_place(
        &self,
        slot: usize,
        stack: &ItemStack,
        owner: &impl StorageContainer<Key = K>,
    ) -> Result<bool> {
        if slot < self.top_count() && self.next_menu.kind == StorageMenuKind::Shulker {
            owner.can_fit_inside_container_items(stack)
        } else {
            Ok(true)
        }
    }
    fn limit(&self, slot: usize, stack: &ItemStack) -> Result<i32> {
        Ok((if slot < self.top_count() {
            self.top_maximum
        } else {
            99
        })
        .min(stack.max_stack_size()?))
    }
    fn operation(&mut self, operation: StorageOperation) {
        self.events.push(StorageMenuEvent::Container {
            owner: self.next_menu.owner_key.clone(),
            operation,
            slots_after: self.next_top.clone(),
        });
    }
    fn changed(&mut self, slot: usize) {
        if slot < self.top_count() {
            self.operation(StorageOperation::SetChanged);
        } else {
            self.events.push(StorageMenuEvent::PlayerInventoryChanged);
        }
    }
    fn set_by_player(&mut self, slot: usize, stack: ItemStack) {
        let stack = canonical(&stack);
        if slot < self.top_count() {
            self.next_top[slot] = stack.clone();
            self.operation(StorageOperation::SetItem { slot, stack });
            self.changed(slot);
        } else {
            self.next_inventory
                .set_slot_authoritative(9 + slot - self.top_count(), stack);
            self.changed(slot);
        }
    }
    fn mutate_stack(&mut self, slot: usize, stack: ItemStack) {
        let stack = canonical(&stack);
        if slot < self.top_count() {
            self.next_top[slot] = stack.clone();
            self.operation(StorageOperation::MutateStack { slot, stack });
        } else {
            self.next_inventory
                .set_slot_authoritative(9 + slot - self.top_count(), stack);
        }
    }
    fn try_remove(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        owner: &impl StorageContainer<Key = K>,
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
        let left = counted(&source, source.count - amount);
        if slot < self.top_count() {
            self.next_top[slot] = left.clone();
            self.operation(StorageOperation::RemoveItem {
                slot,
                amount,
                removed: taken.clone(),
            });
        } else {
            self.next_inventory
                .set_slot_authoritative(9 + slot - self.top_count(), left.clone());
        }
        if !taken.is_empty() && left.is_empty() {
            self.set_by_player(slot, ItemStack::empty());
        }
        Ok(taken)
    }
    fn drop_stack(&mut self, stack: ItemStack, trace_item: bool) {
        if !stack.is_empty() {
            let index = self.outcome.drops.len();
            self.outcome.drops.push(DroppedStack {
                stack,
                around: false,
                trace_item,
            });
            self.events.push(StorageMenuEvent::Drop { index });
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
        let mut value = stack.clone();
        value.count = count;
        value
    }
}
fn reject_bundle(stack: &ItemStack) -> Result<()> {
    ensure!(
        inventory::item_name(stack.item_id) != Some("bundle"),
        "bundle click overrides require the actual feature/item owner"
    );
    Ok(())
}
impl<K: Clone + Eq + Debug> PreparedStorageAction<K> {
    fn do_click(
        &mut self,
        slot: i32,
        button: i32,
        kind: ClickType,
        owner: &impl StorageContainer<Key = K>,
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
                    self.drop_stack(counted(&carried, amount), true);
                    self.next_menu
                        .sync
                        .set_carried(counted(&carried, carried.count - amount));
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
                    self.changed(slot);
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
        owner: &impl StorageContainer<Key = K>,
    ) -> Result<()> {
        let source = self.slot(slot);
        let carried = self.next_menu.carried().clone();
        reject_bundle(&source)?;
        reject_bundle(&carried)?;
        if source.is_empty() {
            if !carried.is_empty() && self.may_place(slot, &carried, owner)? {
                let amount =
                    (if button == 0 { carried.count } else { 1 }).min(self.limit(slot, &carried)?);
                self.set_by_player(slot, counted(&carried, amount));
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count - amount));
            }
        } else if carried.is_empty() {
            let amount = if button == 0 {
                source.count
            } else {
                (source.count + 1) / 2
            };
            let taken = self.try_remove(slot, amount, i32::MAX, owner)?;
            if !taken.is_empty() {
                // cpu pickup lambda installs the cursor before Slot.onTake.
                self.next_menu.sync.set_carried(taken);
                self.changed(slot);
            }
        } else if self.may_place(slot, &carried, owner)? {
            if source.same_item_same_components(&carried)? {
                let amount = (if button == 0 { carried.count } else { 1 })
                    .min((self.limit(slot, &carried)? - source.count).max(0));
                self.set_by_player(slot, counted(&source, source.count + amount));
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count - amount));
            } else if carried.count <= self.limit(slot, &carried)? {
                self.next_menu.sync.set_carried(source);
                self.set_by_player(slot, carried);
            }
        } else if source.same_item_same_components(&carried)? {
            let taken = self.try_remove(
                slot,
                source.count,
                carried.max_stack_size()? - carried.count,
                owner,
            )?;
            if !taken.is_empty() {
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count + taken.count));
                self.changed(slot);
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
        owner: &impl StorageContainer<Key = K>,
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
                        self.mutate_stack(slot, counted(&existing, existing.count + amount));
                        *stack = counted(stack, stack.count - amount);
                        self.mutate_stack(source_slot, stack.clone());
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
                    self.mutate_stack(source_slot, stack.clone());
                    self.set_by_player(slot, moved);
                    break;
                }
            }
        }
        Ok(stack.count != before)
    }
    fn quick_move_once(
        &mut self,
        slot: usize,
        owner: &impl StorageContainer<Key = K>,
    ) -> Result<bool> {
        let source = self.slot(slot);
        reject_bundle(&source)?;
        if source.is_empty() {
            return Ok(false);
        }
        let mut left = source.clone();
        let top = self.top_count();
        let total = self.slot_count();
        let moved = if slot < top {
            self.move_to(&mut left, top, total, true, slot, owner)?
        } else {
            self.move_to(&mut left, 0, top, false, slot, owner)?
        };
        if !moved {
            return Ok(false);
        }
        // Source mutations were recorded inside moveItemStackTo, before each
        // destination's callback. Complete the source slot's dirty boundary.
        if left.is_empty() {
            self.set_by_player(slot, ItemStack::empty());
        } else {
            self.changed(slot);
        }
        if self.next_menu.kind == StorageMenuKind::Dispenser {
            // DispenserMenu invokes the base onTake; Hopper/Shulker omit it.
            self.changed(slot);
        }
        Ok(true)
    }
    fn swap(
        &mut self,
        slot: usize,
        button: i32,
        owner: &impl StorageContainer<Key = K>,
        creative: bool,
    ) -> Result<()> {
        let player_slot = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        if slot >= self.top_count() && 9 + slot - self.top_count() == player_slot {
            return Ok(());
        }
        let source = self.slot(slot);
        let held = self.next_inventory.slot(player_slot).unwrap().clone();
        reject_bundle(&source)?;
        reject_bundle(&held)?;
        if source.is_empty() && held.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            self.next_inventory
                .set_slot_authoritative(player_slot, source);
            self.set_by_player(slot, ItemStack::empty());
            self.changed(slot);
        } else if source.is_empty() {
            if self.may_place(slot, &held, owner)? {
                let amount = held.count.min(self.limit(slot, &held)?);
                self.set_by_player(slot, counted(&held, amount));
                self.next_inventory
                    .set_slot_authoritative(player_slot, counted(&held, held.count - amount));
            }
        } else if self.may_place(slot, &held, owner)? {
            let maximum = self.limit(slot, &held)?;
            if held.count > maximum {
                self.set_by_player(slot, counted(&held, maximum));
                self.next_inventory
                    .set_slot_authoritative(player_slot, counted(&held, held.count - maximum));
                self.changed(slot);
                let mut displaced = source;
                self.add_to_inventory(&mut displaced, creative)?;
                self.drop_stack(displaced, true);
            } else {
                self.next_inventory
                    .set_slot_authoritative(player_slot, source);
                self.set_by_player(slot, held);
                self.changed(slot);
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
        owner: &impl StorageContainer<Key = K>,
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
                    carried.max_stack_size()? - carried.count,
                    owner,
                )?;
                if !taken.is_empty() {
                    self.changed(slot);
                }
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count + taken.count));
            }
        }
        Ok(())
    }
    fn quickcraft(
        &mut self,
        slot: i32,
        button: i32,
        owner: &impl StorageContainer<Key = K>,
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
                        "multi-slot storage drag requires actual callback-order independence or a native Slot identity-order owner");
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
                    0 => carried.count / slots.len().max(1) as i32,
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
                    let count = (each + source.count).min(self.limit(slot, &carried)?);
                    left -= count - source.count;
                    self.set_by_player(slot, counted(&carried, count));
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
    use crate::{crafting::RecipeBook, inventory::Component, protocol::Writer};
    use std::collections::BTreeMap;
    // This is an actual owned in-memory test container, not a production world
    // owner or a claim that Main has registered these block entities.
    struct TestContainer {
        key: &'static str,
        kind: StorageMenuKind,
        revision: u64,
        slots: Vec<ItemStack>,
        valid: bool,
        independent: bool,
    }
    impl TestContainer {
        fn new(kind: StorageMenuKind) -> Self {
            Self {
                key: "owned-test-object-a",
                kind,
                revision: 0,
                slots: vec![ItemStack::empty(); kind.top_slots()],
                valid: true,
                independent: true,
            }
        }
    }
    impl StorageContainer for TestContainer {
        type Key = &'static str;
        fn owner_key(&self) -> &Self::Key {
            &self.key
        }
        fn kind(&self) -> StorageMenuKind {
            self.kind
        }
        fn content_revision(&self) -> u64 {
            self.revision
        }
        fn slots(&self) -> &[ItemStack] {
            &self.slots
        }
        fn maximum_stack_size(&self) -> i32 {
            99
        }
        fn quickcraft_callbacks_are_order_independent(&self) -> Result<bool> {
            // Only the explicit independent fixture has no world receiver.
            Ok(self.independent)
        }
        fn still_valid_for_actual_player(&self) -> Result<bool> {
            Ok(self.valid)
        }
        fn install_prepared_contents(
            &mut self,
            revision: u64,
            expected: &[ItemStack],
            next: &[ItemStack],
        ) -> Result<()> {
            ensure!(
                self.revision == revision && self.slots == expected,
                "test owner changed"
            );
            let revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("test revision exhausted"))?;
            self.slots.clone_from_slice(next);
            self.revision = revision;
            Ok(())
        }
    }
    fn stack(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn click(
        menu: &StorageMenu<&'static str>,
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
        menu: &mut StorageMenu<&'static str>,
        owner: &mut TestContainer,
        inventory: &mut PlayerInventory,
        slot: i16,
        button: i8,
        kind: ClickType,
    ) -> Result<StoragePublication<&'static str>> {
        menu.prepare_click(&click(menu, slot, button, kind), owner, inventory, false)?
            .commit(menu, owner, inventory)
    }
    #[test]
    fn actual_borrowed_storage_flows_all_families_and_close_keeps_world_contents() -> Result<()> {
        for kind in [
            StorageMenuKind::Hopper,
            StorageMenuKind::Dispenser,
            StorageMenuKind::Shulker,
        ] {
            let mut owner = TestContainer::new(kind);
            owner.slots[0] = stack("stone", 8);
            let mut inventory = PlayerInventory::new();
            inventory.set_slot_authoritative(9, stack("emerald", 2));
            let opening = StorageMenu::prepare_open(3, &owner)?;
            assert!(matches!(
                opening.events(),
                [StorageMenuEvent::StartOpen {
                    owner: "owned-test-object-a"
                }]
            ));
            let (mut menu, _) = opening.commit(&owner)?;
            menu.initialize(&owner, &inventory)?;
            let mut stale = click(&menu, 0, 1, ClickType::Pickup);
            stale.state_id = -1;
            let publication = menu
                .prepare_click(&stale, &owner, &inventory, false)?
                .commit(&mut menu, &mut owner, &mut inventory)?;
            assert!(publication
                .events
                .iter()
                .any(|e| matches!(e,StorageMenuEvent::Sync(SyncEvent::Packet(p))if p.id==0x13)));
            assert_eq!(menu.carried(), &stack("stone", 4));
            assert_eq!(owner.slots[0], stack("stone", 4));
            for (slot, button) in [(-999, 0), (1, 1), (2, 1), (-999, 2)] {
                action(
                    &mut menu,
                    &mut owner,
                    &mut inventory,
                    slot,
                    button,
                    ClickType::QuickCraft,
                )?;
            }
            assert_eq!(owner.slots[1], stack("stone", 2));
            assert_eq!(owner.slots[2], stack("stone", 2));
            action(
                &mut menu,
                &mut owner,
                &mut inventory,
                0,
                40,
                ClickType::Swap,
            )?;
            assert_eq!(inventory.slot(45).unwrap(), &stack("stone", 4));
            action(
                &mut menu,
                &mut owner,
                &mut inventory,
                1,
                0,
                ClickType::Pickup,
            )?;
            action(
                &mut menu,
                &mut owner,
                &mut inventory,
                1,
                0,
                ClickType::PickupAll,
            )?;
            assert_eq!(menu.carried(), &stack("stone", 4));
            action(
                &mut menu,
                &mut owner,
                &mut inventory,
                3,
                0,
                ClickType::Pickup,
            )?;
            action(
                &mut menu,
                &mut owner,
                &mut inventory,
                3,
                0,
                ClickType::QuickMove,
            )?;
            let hotbar_last = (kind.top_slots() + 35) as i16;
            assert_eq!(inventory.slot(44).unwrap(), &stack("stone", 4));
            let thrown = action(
                &mut menu,
                &mut owner,
                &mut inventory,
                hotbar_last,
                0,
                ClickType::Throw,
            )?;
            assert_eq!(thrown.outcome.drops[0].stack, stack("stone", 1));
            let clone = menu.prepare_click(
                &click(&menu, hotbar_last, 0, ClickType::Clone),
                &owner,
                &inventory,
                true,
            )?;
            assert_eq!(clone.next_menu.carried(), &stack("stone", 64));
            assert!(menu.carried().is_empty()); // uncommitted creative plan owns nothing
            action(
                &mut menu,
                &mut owner,
                &mut inventory,
                hotbar_last,
                0,
                ClickType::Pickup,
            )?;
            assert_eq!(menu.carried(), &stack("stone", 3));
            // A different actor updates these actual world slots while this menu
            // remains open. Closing must retain them, never return/drop them.
            owner.slots[4] = stack("quartz", 11);
            owner.revision += 1;
            let pending = menu.prepare_click(
                &click(&menu, 4, 0, ClickType::Pickup),
                &owner,
                &inventory,
                false,
            )?;
            owner.slots[4] = stack("quartz", 12);
            owner.revision += 1;
            assert!(pending
                .commit(&mut menu, &mut owner, &mut inventory)
                .is_err());
            let old_key = owner.key;
            owner.key = "replacement-at-same-position";
            assert!(menu
                .prepare_close(&owner, &inventory, MenuCloseContext::AliveConnected)
                .is_err());
            owner.key = old_key;
            let closing =
                menu.prepare_close(&owner, &inventory, MenuCloseContext::DeadOrDisconnected)?;
            assert!(matches!(
                closing.events(),
                [
                    StorageMenuEvent::Drop { index: 0 },
                    StorageMenuEvent::StopOpen {
                        owner: "owned-test-object-a"
                    }
                ]
            ));
            let closed = closing.commit(&mut menu, &mut owner, &mut inventory)?;
            assert_eq!(closed.outcome.drops[0].stack, stack("stone", 3));
            assert_eq!(owner.slots[4], stack("quartz", 12));
            assert_eq!(inventory.slot(45).unwrap(), &stack("stone", 4));
            assert_eq!(inventory.slot(9).unwrap(), &stack("emerald", 2));
            assert!(menu
                .prepare_close(&owner, &inventory, MenuCloseContext::DeadOrDisconnected)
                .is_err());
        }
        Ok(())
    }
    #[test]
    fn shulker_class_restriction_and_actual_component_stack_limit() -> Result<()> {
        for name in [
            "shulker_box",
            "white_shulker_box",
            "orange_shulker_box",
            "magenta_shulker_box",
            "light_blue_shulker_box",
            "yellow_shulker_box",
            "lime_shulker_box",
            "pink_shulker_box",
            "gray_shulker_box",
            "light_gray_shulker_box",
            "cyan_shulker_box",
            "purple_shulker_box",
            "blue_shulker_box",
            "brown_shulker_box",
            "green_shulker_box",
            "red_shulker_box",
            "black_shulker_box",
        ] {
            assert!(!pinned_can_fit_inside_container_items(&stack(name, 1))?);
        }
        let mut owner = TestContainer::new(StorageMenuKind::Shulker);
        let mut inventory = PlayerInventory::new();
        inventory.set_slot_authoritative(9, stack("shulker_box", 1));
        let (mut menu, _) = StorageMenu::prepare_open(4, &owner)?.commit(&owner)?;
        menu.initialize(&owner, &inventory)?;
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            27,
            0,
            ClickType::QuickMove,
        )?;
        assert!(owner.slots.iter().all(ItemStack::is_empty));
        assert_eq!(inventory.slot(9).unwrap(), &stack("shulker_box", 1));
        let mut custom = stack("stone", 90);
        let mut maximum = Writer::new();
        maximum.varint(99);
        custom.components.push(Component {
            type_id: 1,
            data: maximum.into_inner(),
        });
        inventory.set_slot_authoritative(9, custom.clone());
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            27,
            0,
            ClickType::QuickMove,
        )?;
        assert_eq!(owner.slots[0], custom); // original container/item max is 99, not a hardcoded64
        assert!(inventory.slot(9).unwrap().is_empty());
        inventory.set_slot_authoritative(9, stack("emerald", 6));
        action(
            &mut menu,
            &mut owner,
            &mut inventory,
            27,
            0,
            ClickType::Pickup,
        )?;
        for (slot, button) in [(-999, 0), (1, 1), (2, 1)] {
            action(
                &mut menu,
                &mut owner,
                &mut inventory,
                slot,
                button,
                ClickType::QuickCraft,
            )?;
        }
        owner.independent = false;
        assert!(menu
            .prepare_click(
                &click(&menu, -999, 2, ClickType::QuickCraft),
                &owner,
                &inventory,
                false
            )
            .is_err());
        assert!(owner.slots[1].is_empty() && owner.slots[2].is_empty());
        assert_eq!(menu.carried(), &stack("emerald", 6));
        owner.independent = true;
        let prepared = menu.prepare_click(
            &click(&menu, -999, 2, ClickType::QuickCraft),
            &owner,
            &inventory,
            false,
        )?;
        owner.independent = false;
        assert!(prepared
            .commit(&mut menu, &mut owner, &mut inventory)
            .is_err());
        assert!(owner.slots[1].is_empty() && owner.slots[2].is_empty());
        Ok(())
    }
}
