//! EnchantingMenu transport over canonical temporary inputs and borrowed player
//! inventory. The host stages every real owner and ordered effect; no duplicate
//! player inventory is retained by this controller. Whole-host rollback is
//! required on error. Bundles and reentrant listeners remain explicit boundaries.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    random::LegacyRandomSource,
    workstations::{
        enchanting::{
            EnchantingAction, EnchantingBookshelves, EnchantingHost, EnchantingPlayer,
            EnchantingQuickMove, EnchantingState, EnchantmentDefinition, EnchantmentInstance,
            EnchantmentRegistry, DATA_SLOTS, INPUT_SLOTS, MENU_SLOTS,
        },
        menu::{MenuRemovalEvent, MenuSync, SyncEvent, WorkstationKind},
    },
};
use anyhow::{ensure, Result};
use std::{collections::BTreeSet, fmt::Debug};

#[derive(Clone, Copy, Debug)]
pub enum EnchantingMenuAction {
    Initialize,
    Broadcast,
    Button(i32),
    Click(ClickType),
    Close(MenuCloseContext),
}

/// The inventory getters MUST expose the SAME canonical player inventory used
/// by advancement rewards, inventory insertion and player callbacks. The native
/// MenuSync below owns broadcasts in this adapter; its ordered events replace
/// the core host's standalone broadcast_changes implementation.
pub trait EnchantingMenuHost: EnchantingHost {
    /// Actual player and captured Level/position/menu lifetime binding. The
    /// block check follows ContainerLevelAccess semantics, not an invented
    /// persistent enchanting-table block-entity identity.
    type Binding: Clone + Eq + Debug;
    fn binding(&self) -> &Self::Binding;
    /// Actual RandomSource.create factory/seed-uniquifier owner on opening.
    fn create_menu_random_source(&mut self) -> Result<LegacyRandomSource>;
    fn inventory(&self) -> &PlayerInventory;
    fn inventory_mut(&mut self) -> &mut PlayerInventory;
    fn preflight_menu_action(&mut self, action: EnchantingMenuAction) -> Result<()>;
    fn stage_sync_events(&mut self, events: Vec<SyncEvent>) -> Result<()>;
    /// Actual Inventory.setChanged owner (timesChanged), not a packet substitute.
    fn player_inventory_set_changed(&mut self) -> Result<()>;
    /// Actual Inventory.add; mutates this one owned stack to its remainder.
    fn add_to_player_inventory(&mut self, stack: &mut ItemStack) -> Result<bool>;
    fn stage_drop(&mut self, owned: DroppedStack) -> Result<()>;
    /// Actual slot-identity HashSet encounter order. Membership is menu-owned;
    /// return exactly a permutation of it, or reject the unsupported context.
    fn quickcraft_iteration_order(&self, selected: &[usize]) -> Result<Vec<usize>>;
}

struct SyncHost<'a, H> {
    actual: &'a mut H,
    sync: &'a mut MenuSync,
}
impl<H: EnchantingMenuHost> EnchantmentRegistry for SyncHost<'_, H> {
    type Holder = H::Holder;
    type Snapshot = H::Snapshot;
    fn snapshot(&self) -> Self::Snapshot {
        self.actual.snapshot()
    }
    fn item_is_enchantable(&self, item: &ItemStack) -> Result<bool> {
        self.actual.item_is_enchantable(item)
    }
    fn item_enchantment_value(&self, item: &ItemStack) -> Result<i32> {
        self.actual.item_enchantment_value(item)
    }
    fn in_enchanting_table(&self) -> Result<Option<Vec<Self::Holder>>> {
        self.actual.in_enchanting_table()
    }
    fn definition(&self, holder: &Self::Holder) -> Result<EnchantmentDefinition> {
        self.actual.definition(holder)
    }
    fn is_primary_item(&self, holder: &Self::Holder, item: &ItemStack) -> Result<bool> {
        self.actual.is_primary_item(holder, item)
    }
    fn are_compatible(&self, first: &Self::Holder, second: &Self::Holder) -> Result<bool> {
        self.actual.are_compatible(first, second)
    }
    fn protocol_id(&self, holder: &Self::Holder) -> Result<i32> {
        self.actual.protocol_id(holder)
    }
    fn enchant_item(
        &self,
        item: &mut ItemStack,
        enchantment: &EnchantmentInstance<Self::Holder>,
    ) -> Result<()> {
        self.actual.enchant_item(item, enchantment)
    }
}
impl<H: EnchantingMenuHost> EnchantingBookshelves for SyncHost<'_, H> {
    fn is_power_provider(&self, offset: [i32; 3]) -> Result<bool> {
        self.actual.is_power_provider(offset)
    }
    fn is_power_transmitter(&self, offset: [i32; 3]) -> Result<bool> {
        self.actual.is_power_transmitter(offset)
    }
}
impl<H: EnchantingMenuHost> EnchantingHost for SyncHost<'_, H> {
    fn player(&self) -> Result<EnchantingPlayer> {
        self.actual.player()
    }
    fn player_enchantment_seed(&self) -> Result<i32> {
        self.actual.player_enchantment_seed()
    }
    fn is_server_player(&self) -> bool {
        self.actual.is_server_player()
    }
    fn still_valid_table(&self, range: f64) -> Result<bool> {
        self.actual.still_valid_table(range)
    }
    fn preflight(&mut self, action: EnchantingAction) -> Result<()> {
        self.actual.preflight(action)
    }
    fn on_enchantment_performed(&mut self, original: &ItemStack, levels: i32) -> Result<()> {
        self.actual.on_enchantment_performed(original, levels)
    }
    fn award_enchant_item_stat(&mut self) -> Result<()> {
        self.actual.award_enchant_item_stat()
    }
    fn trigger_enchanted_item(&mut self, item: &ItemStack, levels: i32) -> Result<()> {
        self.actual.trigger_enchanted_item(item, levels)
    }
    fn container_set_changed(&mut self, inputs: &[ItemStack; INPUT_SLOTS]) -> Result<()> {
        self.actual.container_set_changed(inputs)
    }
    fn broadcast_changes(
        &mut self,
        inputs: &[ItemStack; INPUT_SLOTS],
        data: [i32; DATA_SLOTS],
    ) -> Result<()> {
        let events = self
            .sync
            .broadcast_changes(inputs, self.actual.inventory(), &data)?;
        self.actual.stage_sync_events(events)
    }
    fn level_random_next_float(&mut self) -> Result<f32> {
        self.actual.level_random_next_float()
    }
    fn play_enchanting_sound(&mut self, pitch: f32) -> Result<()> {
        self.actual.play_enchanting_sound(pitch)
    }
}

#[derive(Clone, Debug)]
pub struct EnchantingMenu<S, K> {
    binding: K,
    active: bool,
    state: EnchantingState<S>,
    sync: MenuSync,
    drag_status: u8,
    drag_type: u8,
    drag_slots: BTreeSet<usize>,
}
fn counted(item: &ItemStack, count: i32) -> ItemStack {
    if count <= 0 || item.is_empty() {
        ItemStack::empty()
    } else {
        let mut out = item.clone();
        out.count = count;
        out
    }
}
impl<S: Clone + PartialEq + Debug, K: Clone + Eq + Debug> EnchantingMenu<S, K> {
    pub fn new<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        window: u8,
        host: &mut H,
    ) -> Result<Self> {
        let random = host.create_menu_random_source()?;
        let mut state = EnchantingState::new(host.player_enchantment_seed()?, host);
        state.install_created_menu_random(random);
        Ok(Self {
            binding: host.binding().clone(),
            active: true,
            state,
            sync: MenuSync::new(window, WorkstationKind::Enchanting)?,
            drag_status: 0,
            drag_type: 0,
            drag_slots: BTreeSet::new(),
        })
    }
    pub fn state(&self) -> &EnchantingState<S> {
        &self.state
    }
    pub fn window_id(&self) -> u8 {
        self.sync.window_id()
    }
    pub fn state_id(&self) -> i32 {
        self.sync.state_id()
    }
    pub fn carried(&self) -> &ItemStack {
        self.sync.carried()
    }
    fn validate_host(&self, host: &impl EnchantingMenuHost<Binding = K>) -> Result<()> {
        ensure!(
            self.active && &self.binding == host.binding(),
            "enchanting player/table/menu binding changed"
        );
        Ok(())
    }
    pub fn still_valid(&self, host: &impl EnchantingMenuHost<Binding = K>) -> Result<bool> {
        self.validate_host(host)?;
        self.state.still_valid(host)
    }
    pub fn transfer_inventory_state(&self, window0: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(window0)
    }
    fn reset_drag(&mut self) {
        self.drag_status = 0;
        self.drag_slots.clear();
    }
    pub fn initialize<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        self.validate_host(host)?;
        host.preflight_menu_action(EnchantingMenuAction::Initialize)?;
        let mut next = self.clone();
        let events = next.sync.initialize(
            next.state.inputs(),
            host.inventory(),
            &next.state.data_values(),
        )?;
        host.stage_sync_events(events)?;
        *self = next;
        Ok(())
    }
    pub fn broadcast_changes<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        self.validate_host(host)?;
        host.preflight_menu_action(EnchantingMenuAction::Broadcast)?;
        let mut next = self.clone();
        let events = next.sync.broadcast_changes(
            next.state.inputs(),
            host.inventory(),
            &next.state.data_values(),
        )?;
        host.stage_sync_events(events)?;
        *self = next;
        Ok(())
    }
    pub fn click_menu_button<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        button: i32,
        host: &mut H,
    ) -> Result<bool> {
        self.validate_host(host)?;
        host.preflight_menu_action(EnchantingMenuAction::Button(button))?;
        let mut next = self.clone();
        let accepted = next.state.click_menu_button(
            button,
            &mut SyncHost {
                actual: host,
                sync: &mut next.sync,
            },
        )?;
        // Source packet handler broadcasts again on accepted buttons, after
        // the action's sound, including empty-list accepted no-op actions.
        if accepted {
            let events = next.sync.broadcast_changes(
                next.state.inputs(),
                host.inventory(),
                &next.state.data_values(),
            )?;
            host.stage_sync_events(events)?;
        }
        *self = next;
        Ok(accepted)
    }
    pub fn click<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        click: &ContainerClick,
        host: &mut H,
    ) -> Result<()> {
        ensure!(
            click.window_id == self.window_id(),
            "wrong enchanting window"
        );
        ensure!(click.slot < MENU_SLOTS as i16, "invalid enchanting slot");
        self.validate_host(host)?;
        host.preflight_menu_action(EnchantingMenuAction::Click(click.kind))?;
        let mut next = self.clone();
        let before = next.state_id();
        next.sync.suppress_remote_updates(true);
        next.do_click(click, host)?;
        let events = next.sync.finish_click(
            click,
            before,
            next.state.inputs(),
            host.inventory(),
            &next.state.data_values(),
        )?;
        host.stage_sync_events(events)?;
        *self = next;
        Ok(())
    }
    pub fn close<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        context: MenuCloseContext,
        host: &mut H,
    ) -> Result<()> {
        self.validate_host(host)?;
        host.preflight_menu_action(EnchantingMenuAction::Close(context))?;
        let mut next = self.clone();
        next.reset_drag();
        let mut inputs = next.state.close_inputs();
        let removed = next
            .sync
            .close_inputs_ordered(&mut inputs, host.inventory_mut(), context)?;
        let mut drops = removed
            .outcome
            .drops
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        for event in removed.events {
            match event {
                MenuRemovalEvent::Packet { index } => {
                    host.stage_sync_events(vec![SyncEvent::Packet(
                        removed.outcome.packets[index].clone(),
                    )])?
                }
                MenuRemovalEvent::Drop { index } => {
                    host.stage_drop(drops[index].take().expect("unique removal drop"))?
                }
            }
        }
        next.active = false;
        *self = next;
        Ok(())
    }
    fn slot<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &self,
        index: usize,
        host: &H,
    ) -> ItemStack {
        if index < INPUT_SLOTS {
            self.state.inputs()[index].clone()
        } else {
            host.inventory()
                .slot(index + 7)
                .expect("validated enchanting player slot")
                .clone()
        }
    }
    fn may_place(index: usize, item: &ItemStack) -> bool {
        index >= INPUT_SLOTS || EnchantingState::<S>::may_place(index, item)
    }
    fn limit(index: usize, item: &ItemStack) -> Result<i32> {
        if index < INPUT_SLOTS {
            EnchantingState::<S>::slot_limit(index, item)
        } else {
            Ok(item.max_stack_size()?.min(99))
        }
    }
    fn mutate_raw<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        index: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if index < INPUT_SLOTS {
            self.state.mutate_input_in_place(index, item)?;
        } else {
            host.inventory_mut().set_slot_authoritative(index + 7, item);
        }
        Ok(())
    }
    fn changed<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        index: usize,
        host: &mut H,
    ) -> Result<()> {
        if index < INPUT_SLOTS {
            self.state.notify_input_changed(&mut SyncHost {
                actual: host,
                sync: &mut self.sync,
            })
        } else {
            host.player_inventory_set_changed()
        }
    }
    /// Slot.setByPlayer -> container.setItem -> Slot.setChanged. The top
    /// SimpleContainer setter itself also invokes setChanged: preserve both.
    fn set_slot<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        index: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if index < INPUT_SLOTS {
            self.state.set_input(
                index,
                item,
                &mut SyncHost {
                    actual: host,
                    sync: &mut self.sync,
                },
            )?;
        } else {
            host.inventory_mut().set_slot_authoritative(index + 7, item);
        }
        self.changed(index, host)
    }
    /// Slot.tryRemove, excluding onTake so Pickup can install cursor first.
    fn try_remove<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        index: usize,
        amount: i32,
        maximum: i32,
        host: &mut H,
    ) -> Result<ItemStack> {
        let source = self.slot(index, host);
        let taken = counted(&source, amount.min(maximum).max(0).min(source.count));
        if taken.is_empty() {
            return Ok(taken);
        }
        self.mutate_raw(index, counted(&source, source.count - taken.count), host)?;
        if index < INPUT_SLOTS {
            self.changed(index, host)?;
        } // SimpleContainer.removeItem
        if self.slot(index, host).is_empty() {
            self.set_slot(index, ItemStack::empty(), host)?;
        }
        Ok(taken)
    }
    fn safe_take<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        index: usize,
        amount: i32,
        maximum: i32,
        host: &mut H,
    ) -> Result<ItemStack> {
        let taken = self.try_remove(index, amount, maximum, host)?;
        if !taken.is_empty() {
            self.changed(index, host)?;
        } // Slot.onTake
        Ok(taken)
    }
    fn drop<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        stack: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if !stack.is_empty() {
            host.stage_drop(DroppedStack {
                stack,
                around: false,
                trace_item: true,
            })?;
        }
        Ok(())
    }
    fn do_click<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        click: &ContainerClick,
        host: &mut H,
    ) -> Result<()> {
        let slot = click.slot as i32;
        let button = click.button as i32;
        let creative = host.player()?.instabuild;
        if click.kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, creative, host);
        }
        if self.drag_status != 0 {
            self.reset_drag();
            return Ok(());
        }
        match click.kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let source = self.carried().clone();
                    let n = if button == 0 {
                        source.count
                    } else {
                        1.min(source.count)
                    };
                    if button == 1 {
                        // Source carried.split(1) precedes the right-drop call.
                        self.sync.set_carried(counted(&source, source.count - n));
                        Self::drop(counted(&source, n), host)?;
                    } else {
                        Self::drop(counted(&source, n), host)?;
                        self.sync.set_carried(ItemStack::empty());
                    }
                } else if slot >= 0 {
                    if click.kind == ClickType::QuickMove {
                        loop {
                            let before = self.slot(slot as usize, host);
                            if !self.quick_move(slot as usize, host)?
                                || self.slot(slot as usize, host).item_id != before.item_id
                            {
                                break;
                            }
                        }
                    } else {
                        self.pickup(slot as usize, button, host)?;
                    }
                }
            }
            ClickType::Swap if ((0..9).contains(&button) || button == 40) && slot >= 0 => {
                self.swap(slot as usize, button, host)?
            }
            ClickType::Clone if creative && slot >= 0 && self.carried().is_empty() => {
                let item = self.slot(slot as usize, host);
                self.sync
                    .set_carried(counted(&item, item.max_stack_size()?));
            }
            ClickType::Throw if slot >= 0 && self.carried().is_empty() => {
                let n = if button == 0 {
                    1
                } else {
                    self.slot(slot as usize, host).count
                };
                let taken = self.safe_take(slot as usize, n, i32::MAX, host)?;
                Self::drop(taken, host)?;
            }
            ClickType::PickupAll if slot >= 0 => self.pickup_all(slot as usize, button, host)?,
            _ => {}
        }
        Ok(())
    }
    fn pickup<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        index: usize,
        button: i32,
        host: &mut H,
    ) -> Result<()> {
        let source = self.slot(index, host);
        let cursor = self.carried().clone();
        ensure!(
            inventory::item_name(source.item_id) != Some("bundle")
                && inventory::item_name(cursor.item_id) != Some("bundle"),
            "bundle click overrides require actual bundle owner"
        );
        if source.is_empty() {
            if !cursor.is_empty() && Self::may_place(index, &cursor) {
                let n =
                    (if button == 0 { cursor.count } else { 1 }).min(Self::limit(index, &cursor)?);
                self.sync.set_carried(counted(&cursor, cursor.count - n));
                self.set_slot(index, counted(&cursor, n), host)?;
            }
        } else if cursor.is_empty() {
            let amount = if button == 0 {
                source.count
            } else {
                (source.count + 1) / 2
            };
            let taken = self.try_remove(index, amount, i32::MAX, host)?;
            if !taken.is_empty() {
                self.sync.set_carried(taken);
                self.changed(index, host)?;
            }
        } else if Self::may_place(index, &cursor) {
            if source.same_item_same_components(&cursor)? {
                let n = (if button == 0 { cursor.count } else { 1 })
                    .min((Self::limit(index, &cursor)? - source.count).max(0));
                // Slot.safeInsert calls setByPlayer for matching stacks
                // even when the full slot accepts zero items.
                self.sync.set_carried(counted(&cursor, cursor.count - n));
                self.set_slot(index, counted(&source, source.count + n), host)?;
            } else if cursor.count <= Self::limit(index, &cursor)? {
                self.sync.set_carried(source);
                self.set_slot(index, cursor, host)?;
            }
        } else if source.same_item_same_components(&cursor)? {
            let taken = self.try_remove(
                index,
                source.count,
                cursor.max_stack_size()? - cursor.count,
                host,
            )?;
            if !taken.is_empty() {
                self.sync
                    .set_carried(counted(&cursor, cursor.count + taken.count));
                self.changed(index, host)?;
            }
        }
        self.changed(index, host) // unconditional Slot.setChanged after Pickup
    }
    fn move_to<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        source_index: usize,
        stack: &mut ItemStack,
        range: std::ops::Range<usize>,
        reverse: bool,
        host: &mut H,
    ) -> Result<bool> {
        let before = stack.count;
        let order = if reverse {
            range.rev().collect::<Vec<_>>()
        } else {
            range.collect()
        };
        if stack.is_stackable()? {
            for &index in &order {
                let old = self.slot(index, host);
                if !old.is_empty() && old.same_item_same_components(stack)? {
                    let n = stack
                        .count
                        .min((Self::limit(index, &old)? - old.count).max(0));
                    if n > 0 {
                        *stack = counted(stack, stack.count - n);
                        self.mutate_raw(source_index, stack.clone(), host)?;
                        self.mutate_raw(index, counted(&old, old.count + n), host)?;
                        self.changed(index, host)?;
                    }
                }
                if stack.is_empty() {
                    break;
                }
            }
        }
        if !stack.is_empty() {
            for index in order {
                if self.slot(index, host).is_empty() && Self::may_place(index, stack) {
                    let n = stack.count.min(Self::limit(index, stack)?);
                    let moved = counted(stack, n);
                    *stack = counted(stack, stack.count - n);
                    self.mutate_raw(source_index, stack.clone(), host)?;
                    self.set_slot(index, moved, host)?;
                    self.changed(index, host)?;
                    break;
                }
            }
        }
        Ok(before != stack.count)
    }
    fn quick_move<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        index: usize,
        host: &mut H,
    ) -> Result<bool> {
        let source = self.slot(index, host);
        let mut left = source.clone();
        let moved = match self.state.quick_move_route(index, &source) {
            Some(EnchantingQuickMove::ToPlayerReverse) => {
                self.move_to(index, &mut left, 2..MENU_SLOTS, true, host)?
            }
            Some(EnchantingQuickMove::ToLapis) => {
                self.move_to(index, &mut left, 1..2, true, host)?
            }
            Some(EnchantingQuickMove::OneToItem) => {
                left = counted(&source, source.count - 1);
                self.mutate_raw(index, left.clone(), host)?;
                self.set_slot(0, counted(&source, 1), host)?;
                true
            }
            None => false,
        };
        if !moved {
            return Ok(false);
        }
        if left.is_empty() {
            self.set_slot(index, ItemStack::empty(), host)?;
        } else {
            self.changed(index, host)?;
        }
        self.changed(index, host)?; // Slot.onTake with post-transfer source stack
        Ok(true)
    }
    fn swap<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        index: usize,
        button: i32,
        host: &mut H,
    ) -> Result<()> {
        let player_index = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        let held = host
            .inventory()
            .slot(player_index)
            .expect("hotbar/offhand")
            .clone();
        let source = self.slot(index, host);
        if held.is_empty() && source.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            host.inventory_mut()
                .set_slot_authoritative(player_index, source);
            self.set_slot(index, ItemStack::empty(), host)?;
            self.changed(index, host)?;
        } else if Self::may_place(index, &held) {
            let maximum = Self::limit(index, &held)?;
            if source.is_empty() {
                host.inventory_mut().set_slot_authoritative(
                    player_index,
                    counted(&held, held.count - maximum.min(held.count)),
                );
                self.set_slot(index, counted(&held, maximum.min(held.count)), host)?;
            } else if held.count > maximum {
                host.inventory_mut()
                    .set_slot_authoritative(player_index, counted(&held, held.count - maximum));
                self.set_slot(index, counted(&held, maximum), host)?;
                self.changed(index, host)?;
                let mut displaced = if index >= INPUT_SLOTS && index + 7 == player_index {
                    // Java source/held locals alias the pre-split stack. After
                    // split the displaced local references only its remainder.
                    counted(&source, source.count - maximum)
                } else {
                    source
                };
                if !host.add_to_player_inventory(&mut displaced)? {
                    Self::drop(displaced, host)?;
                }
            } else {
                host.inventory_mut()
                    .set_slot_authoritative(player_index, source);
                self.set_slot(index, held, host)?;
                self.changed(index, host)?;
            }
        }
        Ok(())
    }
    fn pickup_all<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        clicked: usize,
        button: i32,
        host: &mut H,
    ) -> Result<()> {
        if self.carried().is_empty() || !self.slot(clicked, host).is_empty() {
            return Ok(());
        }
        let order = if button == 0 {
            (0..MENU_SLOTS).collect::<Vec<_>>()
        } else {
            (0..MENU_SLOTS).rev().collect()
        };
        for pass in 0..2 {
            for &index in &order {
                let source = self.slot(index, host);
                let cursor = self.carried().clone();
                if cursor.count >= cursor.max_stack_size()? {
                    break;
                }
                if source.is_empty()
                    || !source.same_item_same_components(&cursor)?
                    || source.count > cursor.max_stack_size()?
                    || pass == 0 && source.count == source.max_stack_size()?
                {
                    continue;
                }
                let taken = self.safe_take(
                    index,
                    source.count,
                    cursor.max_stack_size()? - cursor.count,
                    host,
                )?;
                self.sync
                    .set_carried(counted(&cursor, cursor.count + taken.count));
            }
        }
        Ok(())
    }
    fn quickcraft<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
        &mut self,
        slot: i32,
        button: i32,
        creative: bool,
        host: &mut H,
    ) -> Result<()> {
        let previous = self.drag_status;
        self.drag_status = (button & 3) as u8;
        if !(previous == 1 && self.drag_status == 2) && previous != self.drag_status {
            self.reset_drag();
            return Ok(());
        }
        if self.carried().is_empty() {
            self.reset_drag();
            return Ok(());
        }
        match self.drag_status {
            0 => {
                self.drag_type = ((button >> 2) & 3) as u8;
                if self.drag_type < 2 || self.drag_type == 2 && creative {
                    self.drag_status = 1;
                    self.drag_slots.clear();
                } else {
                    self.reset_drag();
                }
            }
            1 => {
                if slot >= 0 {
                    let index = slot as usize;
                    let old = self.slot(index, host);
                    let cursor = self.carried();
                    if (old.is_empty()
                        || old.same_item_same_components(cursor)?
                            && old.count <= cursor.max_stack_size()?)
                        && Self::may_place(index, cursor)
                        && (self.drag_type == 2 || cursor.count > self.drag_slots.len() as i32)
                    {
                        self.drag_slots.insert(index);
                    }
                }
            }
            2 => {
                if self.drag_slots.is_empty() {
                    self.reset_drag();
                    return Ok(());
                }
                if self.drag_slots.len() == 1 {
                    let index = *self.drag_slots.first().unwrap();
                    let kind = self.drag_type as i32;
                    self.reset_drag();
                    // Source recursively dispatches PICKUP with this button;
                    // only 0/1 pass that dispatcher's guard. Creative type2
                    // with one selected slot consequently does nothing.
                    return if kind <= 1 {
                        self.pickup(index, kind, host)
                    } else {
                        Ok(())
                    };
                }
                let cursor = self.carried().clone();
                let mut remaining = cursor.count;
                let selected = self.drag_slots.iter().copied().collect::<Vec<_>>();
                let order = host.quickcraft_iteration_order(&selected)?;
                ensure!(
                    order.len() == selected.len()
                        && order.iter().copied().collect::<BTreeSet<_>>() == self.drag_slots,
                    "invalid actual quickcraft encounter order"
                );
                let each = match self.drag_type {
                    0 => cursor.count / self.drag_slots.len().max(1) as i32,
                    1 => 1,
                    2 => cursor.max_stack_size()?,
                    _ => 0,
                };
                for index in order {
                    let old = self.slot(index, host);
                    if (!old.is_empty()
                        && (!old.same_item_same_components(&cursor)?
                            || old.count > cursor.max_stack_size()?))
                        || !Self::may_place(index, &cursor)
                        || self.drag_type != 2 && cursor.count < self.drag_slots.len() as i32
                    {
                        continue;
                    }
                    let count = (each + old.count).min(Self::limit(index, &cursor)?);
                    remaining -= count - old.count;
                    self.set_slot(index, counted(&cursor, count), host)?;
                }
                self.sync.set_carried(counted(&cursor, remaining));
                self.reset_drag();
            }
            _ => self.reset_drag(),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{protocol::Reader, workstations::enchanting::tests::RecordingHost};
    use std::collections::BTreeMap;
    struct Host {
        core: RecordingHost,
        inventory: PlayerInventory,
        binding: u64,
        sync_events: Vec<SyncEvent>,
        drops: Vec<DroppedStack>,
        inventory_changes: usize,
        added_counts: Vec<i32>,
    }
    impl Host {
        fn new(creative: bool) -> Self {
            Self {
                core: RecordingHost::new(creative),
                inventory: PlayerInventory::default(),
                binding: 7,
                sync_events: vec![],
                drops: vec![],
                inventory_changes: 0,
                added_counts: vec![],
            }
        }
    }
    impl EnchantmentRegistry for Host {
        type Holder = &'static str;
        type Snapshot = u64;
        fn snapshot(&self) -> u64 {
            self.core.snapshot()
        }
        fn item_is_enchantable(&self, s: &ItemStack) -> Result<bool> {
            self.core.item_is_enchantable(s)
        }
        fn item_enchantment_value(&self, s: &ItemStack) -> Result<i32> {
            self.core.item_enchantment_value(s)
        }
        fn in_enchanting_table(&self) -> Result<Option<Vec<Self::Holder>>> {
            self.core.in_enchanting_table()
        }
        fn definition(&self, h: &Self::Holder) -> Result<EnchantmentDefinition> {
            self.core.definition(h)
        }
        fn is_primary_item(&self, h: &Self::Holder, s: &ItemStack) -> Result<bool> {
            self.core.is_primary_item(h, s)
        }
        fn are_compatible(&self, a: &Self::Holder, b: &Self::Holder) -> Result<bool> {
            self.core.are_compatible(a, b)
        }
        fn protocol_id(&self, h: &Self::Holder) -> Result<i32> {
            self.core.protocol_id(h)
        }
        fn enchant_item(
            &self,
            s: &mut ItemStack,
            e: &EnchantmentInstance<Self::Holder>,
        ) -> Result<()> {
            self.core.enchant_item(s, e)
        }
    }
    impl EnchantingBookshelves for Host {
        fn is_power_provider(&self, p: [i32; 3]) -> Result<bool> {
            self.core.is_power_provider(p)
        }
        fn is_power_transmitter(&self, p: [i32; 3]) -> Result<bool> {
            self.core.is_power_transmitter(p)
        }
    }
    impl EnchantingHost for Host {
        fn player(&self) -> Result<EnchantingPlayer> {
            self.core.player()
        }
        fn player_enchantment_seed(&self) -> Result<i32> {
            self.core.player_enchantment_seed()
        }
        fn is_server_player(&self) -> bool {
            self.core.is_server_player()
        }
        fn still_valid_table(&self, r: f64) -> Result<bool> {
            self.core.still_valid_table(r)
        }
        fn preflight(&mut self, a: EnchantingAction) -> Result<()> {
            self.core.preflight(a)
        }
        fn on_enchantment_performed(&mut self, s: &ItemStack, n: i32) -> Result<()> {
            self.core.on_enchantment_performed(s, n)
        }
        fn award_enchant_item_stat(&mut self) -> Result<()> {
            self.core.award_enchant_item_stat()
        }
        fn trigger_enchanted_item(&mut self, s: &ItemStack, n: i32) -> Result<()> {
            self.core.trigger_enchanted_item(s, n)
        }
        fn container_set_changed(&mut self, s: &[ItemStack; 2]) -> Result<()> {
            self.core.container_set_changed(s)
        }
        fn broadcast_changes(&mut self, _: &[ItemStack; 2], _: [i32; 10]) -> Result<()> {
            unreachable!("controller supplies actual MenuSync owner")
        }
        fn level_random_next_float(&mut self) -> Result<f32> {
            self.core.level_random_next_float()
        }
        fn play_enchanting_sound(&mut self, p: f32) -> Result<()> {
            self.core.play_enchanting_sound(p)
        }
    }
    impl EnchantingMenuHost for Host {
        type Binding = u64;
        fn binding(&self) -> &u64 {
            &self.binding
        }
        fn create_menu_random_source(&mut self) -> Result<LegacyRandomSource> {
            self.core.calls.borrow_mut().push("factory");
            Ok(LegacyRandomSource::new(42))
        }
        fn inventory(&self) -> &PlayerInventory {
            &self.inventory
        }
        fn inventory_mut(&mut self) -> &mut PlayerInventory {
            &mut self.inventory
        }
        fn preflight_menu_action(&mut self, _: EnchantingMenuAction) -> Result<()> {
            Ok(())
        }
        fn stage_sync_events(&mut self, e: Vec<SyncEvent>) -> Result<()> {
            self.core.calls.borrow_mut().push("sync");
            self.sync_events.extend(e);
            Ok(())
        }
        fn player_inventory_set_changed(&mut self) -> Result<()> {
            self.inventory_changes += 1;
            Ok(())
        }
        fn add_to_player_inventory(&mut self, s: &mut ItemStack) -> Result<bool> {
            // Recording insertion sink for the alias regression, not a
            // production Inventory.add implementation.
            self.added_counts.push(s.count);
            ensure!(
                self.inventory.slot(9).unwrap().is_empty(),
                "recording sink occupied"
            );
            self.inventory.set_slot_authoritative(9, std::mem::take(s));
            Ok(true)
        }
        fn stage_drop(&mut self, s: DroppedStack) -> Result<()> {
            self.drops.push(s);
            Ok(())
        }
        fn quickcraft_iteration_order(&self, s: &[usize]) -> Result<Vec<usize>> {
            Ok(s.to_vec())
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn click(
        menu: &EnchantingMenu<u64, u64>,
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
    use crate::crafting::RecipeBook;
    #[test]
    fn borrowed_inventory_shift_button_sync_and_disconnect_conserve_one_owned_book() -> Result<()> {
        let mut host = Host::new(false);
        host.inventory.set_slot_authoritative(9, item("book", 1));
        host.inventory
            .set_slot_authoritative(10, item("lapis_lazuli", 3));
        let mut menu = EnchantingMenu::new(5, &mut host)?;
        assert_eq!(*host.core.calls.borrow(), ["factory", "seed"]);
        menu.initialize(&mut host)?;
        let content = host
            .sync_events
            .iter()
            .find_map(|e| match e {
                SyncEvent::Packet(p) if p.id == 0x13 => Some(p),
                _ => None,
            })
            .unwrap();
        let mut reader = Reader::new(&content.payload);
        assert_eq!(reader.u8()?, 5);
        reader.varint()?;
        assert_eq!(reader.varint()?, 38);
        host.sync_events.clear();
        menu.click(&click(&menu, 2, 0, ClickType::QuickMove), &mut host)?;
        assert!(host.inventory.slot(9).unwrap().is_empty());
        // Intermediate cost recomputation must observe the player source
        // already decremented, not an input + unchanged player duplicate.
        assert!(!host.sync_events.iter().any(
            |e| matches!(e,SyncEvent::SlotListener{menu_slot:2,stack,..}if !stack.is_empty())
        ));
        menu.click(&click(&menu, 3, 0, ClickType::QuickMove), &mut host)?;
        host.core.calls.borrow_mut().clear();
        host.sync_events.clear();
        assert!(menu.click_menu_button(2, &mut host)?);
        assert_eq!(host.player()?.experience_level, 27);
        assert_eq!(host.core.calls.borrow().last(), Some(&"sync"));
        let sound = host
            .core
            .calls
            .borrow()
            .iter()
            .position(|v| *v == "sound")
            .unwrap();
        assert_eq!(host.core.calls.borrow()[sound + 1], "sync"); // packet-handler final broadcast
        assert!(host.sync_events.iter().any(|e|matches!(e,SyncEvent::SlotListener{menu_slot:0,stack,..}if RecipeBook::vanilla().item_id("enchanted_book")==Some(stack.item_id))));
        menu.click(&click(&menu, 0, 0, ClickType::Pickup), &mut host)?;
        assert!(menu.state.inputs()[0].is_empty());
        assert_eq!(menu.carried().count, 1);
        menu.close(MenuCloseContext::DeadOrDisconnected, &mut host)?;
        assert_eq!(host.drops.len(), 1);
        assert_eq!(host.drops[0].count, 1);
        assert!(!host.drops[0].trace_item);
        assert!(menu.carried().is_empty());
        assert!(menu.state.inputs().iter().all(ItemStack::is_empty));
        assert!(menu.broadcast_changes(&mut host).is_err());
        Ok(())
    }
    #[test]
    fn overfull_self_swap_keeps_only_split_remainder_and_creative_single_drag_is_noop() -> Result<()>
    {
        let mut host = Host::new(true);
        host.inventory
            .set_slot_authoritative(36, item("emerald", 99));
        let mut menu = EnchantingMenu::new(5, &mut host)?;
        menu.click(&click(&menu, 29, 0, ClickType::Swap), &mut host)?;
        assert_eq!(host.added_counts, [35]);
        assert_eq!(host.inventory.slot(36).unwrap().count, 64);
        assert_eq!(host.inventory.slot(9).unwrap().count, 35);
        // Creative clone, then drag type2 into one selected empty input.
        menu.click(&click(&menu, 29, 0, ClickType::Clone), &mut host)?;
        let cursor = menu.carried().clone();
        for (slot, button) in [(-999, 8), (0, 9), (-999, 10)] {
            menu.click(
                &click(&menu, slot, button, ClickType::QuickCraft),
                &mut host,
            )?;
        }
        assert!(menu.state.inputs()[0].is_empty());
        assert_eq!(menu.carried(), &cursor);
        // Slot.safeInsert on an already-full matching lapis slot still
        // invokes container.setItem + Slot.setChanged + outer changed.
        let mut full_host = Host::new(true);
        full_host
            .inventory
            .set_slot_authoritative(9, item("lapis_lazuli", 64));
        let mut full_menu = EnchantingMenu::new(6, &mut full_host)?;
        full_menu.click(
            &click(&full_menu, 2, 0, ClickType::QuickMove),
            &mut full_host,
        )?;
        full_menu.click(&click(&full_menu, 1, 0, ClickType::Clone), &mut full_host)?;
        full_host.core.calls.borrow_mut().clear();
        full_menu.click(&click(&full_menu, 1, 0, ClickType::Pickup), &mut full_host)?;
        assert_eq!(
            *full_host.core.calls.borrow(),
            ["container", "container", "container", "sync"]
        );
        assert_eq!(full_menu.state.inputs()[1].count, 64);
        assert_eq!(full_menu.carried().count, 64);
        host.binding = 8;
        assert!(menu
            .click(&click(&menu, 0, 0, ClickType::Pickup), &mut host)
            .is_err());
        Ok(())
    }
}
