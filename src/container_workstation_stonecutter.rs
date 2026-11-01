//! Authoritative bounded StonecutterMenu adapter over the shared sync owner.
//! Player inventory is borrowed per prepared transaction. Main owns dispatch,
//! captured block validity, actual callbacks and publication (see source ledger).
use crate::{
    container::{
        ClickOutcome, ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer,
    },
    inventory::{self, ItemStack, PlayerInventory},
    workstations::{
        menu::{MenuRemovalEvent, MenuSync, SyncEvent, WorkstationAccess, WorkstationKind},
        stonecutter::{
            StonecutterEffect, StonecutterRecipes, StonecutterState, StonecutterTake, MENU_SLOTS,
        },
    },
};
use anyhow::{bail, ensure, Result};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Debug)]
pub struct StonecutterMenu {
    sync: MenuSync,
    state: StonecutterState,
    instance: u64,
    revision: u64,
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
}
#[derive(Clone, Debug)]
pub enum StonecutterMenuEvent {
    Sync(SyncEvent),
    Effect(StonecutterEffect),
    /// Execute exactly this owned drop at its source callback position. The
    /// index refers to `StonecutterPublication.outcome.drops`, not a copy.
    Drop {
        index: usize,
    },
    /// Item.onCraftedBy(stack, level, player), before trying result quick-move.
    /// This callback occurs even when the actual inventory accepts nothing.
    ItemCraftedBeforeQuickMove {
        stack: ItemStack,
    },
}
/// A prepared action has no live effects. Inspect/stage `events` and actual item
/// drops using real owners before consuming `commit`; failed staging is abort.
/// Callback snapshots/preview getters never confer additional item ownership.
pub struct PreparedStonecutterAction {
    expected_instance: u64,
    expected_revision: u64,
    expected_inventory: PlayerInventory,
    next_menu: StonecutterMenu,
    next_inventory: PlayerInventory,
    events: Vec<StonecutterMenuEvent>,
    outcome: ClickOutcome,
}
/// Publish `events` in order. A Drop index consumes that one owned stack from
/// outcome.drops; never drain drops independently before/after this sequence.
/// Packet events are the sole packet publication owner (outcome.packets empty).
pub struct StonecutterPublication {
    pub events: Vec<StonecutterMenuEvent>,
    pub outcome: ClickOutcome,
}
impl PreparedStonecutterAction {
    pub fn events(&self) -> &[StonecutterMenuEvent] {
        &self.events
    }
    pub fn drops(&self) -> &[DroppedStack] {
        &self.outcome.drops
    }
    pub fn next_inventory(&self) -> &PlayerInventory {
        &self.next_inventory
    }
    pub fn next_menu(&self) -> &StonecutterMenu {
        &self.next_menu
    }
    pub fn validate(&self, menu: &StonecutterMenu, inventory: &PlayerInventory) -> Result<()> {
        ensure!(
            menu.instance == self.expected_instance && menu.revision == self.expected_revision,
            "stale stonecutter menu action"
        );
        ensure!(
            inventory.slots() == self.expected_inventory.slots()
                && inventory.carried() == self.expected_inventory.carried()
                && inventory.selected_hotbar() == self.expected_inventory.selected_hotbar()
                && inventory.state_id() == self.expected_inventory.state_id(),
            "stonecutter canonical inventory changed during preparation"
        );
        Ok(())
    }
    pub fn commit(
        self,
        menu: &mut StonecutterMenu,
        inventory: &mut PlayerInventory,
    ) -> Result<StonecutterPublication> {
        self.validate(menu, inventory)?;
        *menu = self.next_menu;
        *inventory = self.next_inventory;
        Ok(StonecutterPublication {
            events: self.events,
            outcome: self.outcome,
        })
    }
}
impl StonecutterMenu {
    pub fn new(window_id: u8) -> Result<Self> {
        static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
        let instance = NEXT_INSTANCE
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| anyhow::anyhow!("menu identity exhausted"))?;
        Ok(Self {
            sync: MenuSync::new(window_id, WorkstationKind::Stonecutter)?,
            state: StonecutterState::new(),
            instance,
            revision: 0,
            quickcraft_status: 0,
            quickcraft_type: 0,
            quickcraft_slots: BTreeSet::new(),
        })
    }
    pub fn window_id(&self) -> u8 {
        self.sync.window_id()
    }
    pub fn state_id(&self) -> i32 {
        self.sync.state_id()
    }
    pub fn state(&self) -> &StonecutterState {
        &self.state
    }
    pub fn carried(&self) -> &ItemStack {
        self.sync.carried()
    }
    fn top(&self) -> [ItemStack; 2] {
        [
            self.state.input().clone(),
            self.state.result_preview().clone(),
        ]
    }
    fn data(&self) -> [i32; 1] {
        [self.state.selected_recipe_index()]
    }
    fn advance_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("menu revision exhausted"))?;
        Ok(())
    }
    pub fn initialize(&mut self, inventory: &PlayerInventory) -> Result<Vec<SyncEvent>> {
        let mut next = self.clone();
        let top = next.top();
        let data = next.data();
        let out = next.sync.initialize(&top, inventory, &data)?;
        next.advance_revision()?;
        *self = next;
        Ok(out)
    }
    pub fn broadcast_changes(&mut self, inventory: &PlayerInventory) -> Result<Vec<SyncEvent>> {
        let mut next = self.clone();
        let top = next.top();
        let data = next.data();
        let out = next.sync.broadcast_changes(&top, inventory, &data)?;
        next.advance_revision()?;
        *self = next;
        Ok(out)
    }
    pub fn still_valid(&self, access: &impl WorkstationAccess) -> Result<bool> {
        self.sync.still_valid(access)
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    fn reset_quickcraft(&mut self) {
        self.quickcraft_status = 0;
        self.quickcraft_slots.clear();
    }
    fn preparation(&self, inventory: &PlayerInventory) -> PreparedStonecutterAction {
        PreparedStonecutterAction {
            expected_instance: self.instance,
            expected_revision: self.revision,
            expected_inventory: inventory.clone(),
            next_menu: self.clone(),
            next_inventory: inventory.clone(),
            events: vec![],
            outcome: ClickOutcome::default(),
        }
    }
    pub fn prepare_button(
        &self,
        index: i32,
        inventory: &PlayerInventory,
        recipes: &impl StonecutterRecipes,
    ) -> Result<PreparedStonecutterAction> {
        let mut action = self.preparation(inventory);
        action.next_menu.state.click_button(index, recipes)?;
        let top = action.next_menu.top();
        let data = action.next_menu.data();
        let events =
            action
                .next_menu
                .sync
                .broadcast_changes(&top, &action.next_inventory, &data)?;
        action
            .events
            .extend(events.into_iter().map(StonecutterMenuEvent::Sync));
        action.next_menu.advance_revision()?;
        Ok(action)
    }
    pub fn prepare_click(
        &self,
        click: &ContainerClick,
        inventory: &PlayerInventory,
        recipes: &impl StonecutterRecipes,
        world_game_time: i64,
        creative: bool,
    ) -> Result<PreparedStonecutterAction> {
        ensure!(
            click.window_id == self.window_id(),
            "wrong stonecutter window"
        );
        ensure!(click.slot < MENU_SLOTS as i16, "invalid stonecutter slot");
        let mut action = self.preparation(inventory);
        let before = self.state_id();
        action.next_menu.sync.suppress_remote_updates(true);
        action.do_click(click, recipes, world_game_time, creative)?;
        let top = action.next_menu.top();
        let data = action.next_menu.data();
        let events = action.next_menu.sync.finish_click(
            click,
            before,
            &top,
            &action.next_inventory,
            &data,
        )?;
        action
            .events
            .extend(events.into_iter().map(StonecutterMenuEvent::Sync));
        action.next_menu.advance_revision()?;
        Ok(action)
    }
    pub fn prepare_close(
        &self,
        inventory: &PlayerInventory,
        context: MenuCloseContext,
    ) -> Result<PreparedStonecutterAction> {
        let mut action = self.preparation(inventory);
        action.next_menu.reset_quickcraft();
        let input = action.next_menu.state.remove_owned_input_for_close()?;
        let removal = action.next_menu.sync.close_inputs_ordered(
            &mut [input],
            &mut action.next_inventory,
            context,
        )?;
        action.outcome = removal.outcome;
        for event in removal.events {
            match event {
                MenuRemovalEvent::Packet { index } => {
                    action
                        .events
                        .push(StonecutterMenuEvent::Sync(SyncEvent::Packet(
                            action.outcome.packets[index].clone(),
                        )))
                }
                MenuRemovalEvent::Drop { index } => {
                    action.events.push(StonecutterMenuEvent::Drop { index })
                }
            }
        }
        // Packets have a single ordered publication owner in events.
        action.outcome.packets.clear();
        action.next_menu.advance_revision()?;
        Ok(action)
    }
}
impl PreparedStonecutterAction {
    fn slot(&self, slot: usize) -> ItemStack {
        match slot {
            0 => self.next_menu.state.input().clone(),
            1 => self.next_menu.state.result_preview().clone(),
            _ => self
                .next_inventory
                .slot(slot + 7)
                .expect("validated menu slot")
                .clone(),
        }
    }
    fn set_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        recipes: &impl StonecutterRecipes,
    ) -> Result<()> {
        match slot {
            0 => self.next_menu.state.replace_input(stack, recipes)?,
            1 => bail!("derived stonecutter result cannot be assigned as owned input"),
            _ => self.next_inventory.set_slot_authoritative(slot + 7, stack),
        }
        Ok(())
    }
    fn sync_after_recompute(&mut self) -> Result<()> {
        let top = self.next_menu.top();
        let data = self.next_menu.data();
        let events = self
            .next_menu
            .sync
            .broadcast_changes(&top, &self.next_inventory, &data)?;
        self.events
            .extend(events.into_iter().map(StonecutterMenuEvent::Sync));
        Ok(())
    }
    fn apply_take(&mut self, take: StonecutterTake) -> Result<ItemStack> {
        let taken = take.commit(&mut self.next_menu.state)?;
        for effect in taken.effects {
            if effect == StonecutterEffect::InputConsumedAndResultRecomputed {
                self.sync_after_recompute()?;
            } else {
                self.events.push(StonecutterMenuEvent::Effect(effect));
            }
        }
        Ok(taken.output)
    }
    fn take_slot(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        recipes: &impl StonecutterRecipes,
        time: i64,
    ) -> Result<ItemStack> {
        let source = self.slot(slot);
        if source.is_empty() || maximum <= 0 {
            return Ok(ItemStack::empty());
        }
        if slot == 1 {
            // Slot.tryRemove forbids partial extraction when mayPlace is false.
            if source.count > maximum {
                return Ok(ItemStack::empty());
            }
            if let Some(take) = self.next_menu.state.prepare_take(recipes, time)? {
                return self.apply_take(take);
            }
            return Ok(ItemStack::empty());
        }
        let taken = counted(&source, amount.min(maximum).max(0).min(source.count));
        self.set_slot(slot, counted(&source, source.count - taken.count), recipes)?;
        Ok(taken)
    }
    fn do_click(
        &mut self,
        click: &ContainerClick,
        recipes: &impl StonecutterRecipes,
        time: i64,
        creative: bool,
    ) -> Result<()> {
        let slot = click.slot as i32;
        let button = click.button as i32;
        if click.kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, recipes, time, creative);
        }
        if self.next_menu.quickcraft_status != 0 {
            self.next_menu.reset_quickcraft();
            return Ok(());
        }
        match click.kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let source = self.next_menu.sync.carried().clone();
                    let amount = if button == 0 {
                        source.count
                    } else {
                        1.min(source.count)
                    };
                    self.next_menu
                        .sync
                        .set_carried(counted(&source, source.count - amount));
                    self.drop_stack(counted(&source, amount));
                } else if slot >= 0 {
                    let slot = slot as usize;
                    if click.kind == ClickType::QuickMove {
                        loop {
                            let before = self.slot(slot);
                            if !self.quick_move_once(slot, recipes, time)?
                                || self.slot(slot).item_id != before.item_id
                            {
                                break;
                            }
                        }
                    } else {
                        self.pickup(slot, button, recipes, time)?;
                    }
                }
            }
            ClickType::Swap if ((0..9).contains(&button) || button == 40) && slot >= 0 => {
                self.swap(slot as usize, button, recipes, time, creative)?;
            }
            ClickType::PickupAll if slot >= 0 => {
                self.pickup_all(slot as usize, button, recipes, time)?;
            }
            ClickType::Throw if slot >= 0 && self.next_menu.sync.carried().is_empty() => {
                let amount = if button == 0 {
                    1
                } else {
                    self.slot(slot as usize).count
                };
                let taken = self.take_slot(slot as usize, amount, i32::MAX, recipes, time)?;
                self.drop_stack(taken);
            }
            ClickType::Clone
                if creative && slot >= 0 && self.next_menu.sync.carried().is_empty() =>
            {
                let source = self.slot(slot as usize);
                self.next_menu
                    .sync
                    .set_carried(counted(&source, source.max_stack_size()?));
            }
            _ => {}
        }
        Ok(())
    }
    fn pickup(
        &mut self,
        slot: usize,
        button: i32,
        recipes: &impl StonecutterRecipes,
        time: i64,
    ) -> Result<()> {
        let source = self.slot(slot);
        let carried = self.next_menu.sync.carried().clone();
        ensure!(
            inventory::item_name(source.item_id) != Some("bundle")
                && inventory::item_name(carried.item_id) != Some("bundle"),
            "bundle item click overrides require the actual bundle owner"
        );
        if slot == 1 {
            if source.is_empty() {
                return Ok(());
            }
            if carried.is_empty() {
                if let Some(take) = self.next_menu.state.prepare_take(recipes, time)? {
                    // cpu.lambda$doClick$3 installs the actual cursor first.
                    self.next_menu
                        .sync
                        .set_carried(take.output_preview().clone());
                    self.apply_take(take)?;
                }
            } else if source.same_item_same_components(&carried)?
                && carried.max_stack_size()? - carried.count >= source.count
            {
                if let Some(take) = self.next_menu.state.prepare_take(recipes, time)? {
                    // lambda$doClick$4 grows that same cursor before onTake.
                    self.next_menu.sync.set_carried(counted(
                        &carried,
                        carried.count + take.output_preview().count,
                    ));
                    self.apply_take(take)?;
                }
            }
            return Ok(());
        }
        if source.is_empty() {
            if !carried.is_empty() && slot != 1 {
                let amount = (if button == 0 { carried.count } else { 1 }).min(limit(&carried)?);
                self.set_slot(slot, counted(&carried, amount), recipes)?;
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
            let taken = self.take_slot(slot, amount, i32::MAX, recipes, time)?;
            self.next_menu.sync.set_carried(taken);
        } else if slot != 1 {
            if source.same_item_same_components(&carried)? {
                let amount = (if button == 0 { carried.count } else { 1 })
                    .min((limit(&carried)? - source.count).max(0));
                if amount > 0 {
                    self.set_slot(slot, counted(&source, source.count + amount), recipes)?;
                    self.next_menu
                        .sync
                        .set_carried(counted(&carried, carried.count - amount));
                }
            } else if carried.count <= limit(&carried)? {
                self.set_slot(slot, carried, recipes)?;
                self.next_menu.sync.set_carried(source);
            }
        } else if source.same_item_same_components(&carried)? {
            let amount = (carried.max_stack_size()? - carried.count).max(0);
            let taken = self.take_slot(slot, source.count, amount, recipes, time)?;
            self.next_menu
                .sync
                .set_carried(counted(&carried, carried.count + taken.count));
        }
        Ok(())
    }
    fn move_to(
        &mut self,
        stack: &mut ItemStack,
        start: usize,
        end: usize,
        reverse: bool,
        recipes: &impl StonecutterRecipes,
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
                if !existing.is_empty() && existing.same_item_same_components(stack)? {
                    let amount = stack.count.min((limit(stack)? - existing.count).max(0));
                    if amount > 0 {
                        self.set_slot(slot, counted(&existing, existing.count + amount), recipes)?;
                        *stack = counted(stack, stack.count - amount);
                    }
                    if stack.is_empty() {
                        break;
                    }
                }
            }
        }
        if !stack.is_empty() {
            for slot in order {
                if self.slot(slot).is_empty() {
                    let amount = stack.count.min(limit(stack)?);
                    self.set_slot(slot, counted(stack, amount), recipes)?;
                    *stack = counted(stack, stack.count - amount);
                    break;
                }
            }
        }
        Ok(stack.count != before)
    }
    fn quick_move_once(
        &mut self,
        slot: usize,
        recipes: &impl StonecutterRecipes,
        time: i64,
    ) -> Result<bool> {
        let source = self.slot(slot);
        if source.is_empty() {
            return Ok(false);
        }
        let mut left = source.clone();
        if slot == 1 {
            self.events
                .push(StonecutterMenuEvent::ItemCraftedBeforeQuickMove {
                    stack: source.clone(),
                });
            if !self.move_to(&mut left, 2, 38, true, recipes)? {
                return Ok(false);
            }
            let take = self.next_menu.state.prepare_quick_take(
                recipes,
                time,
                source.count - left.count,
            )?;
            self.apply_take(take)?;
            self.sync_after_recompute()?;
            return Ok(true);
        }
        let moved = if slot == 0 {
            self.move_to(&mut left, 2, 38, false, recipes)?
        } else if !recipes.matching_recipes(&left)?.is_empty() {
            self.move_to(&mut left, 0, 1, false, recipes)?
        } else if slot < 29 {
            self.move_to(&mut left, 29, 38, false, recipes)?
        } else {
            self.move_to(&mut left, 2, 29, false, recipes)?
        };
        if moved {
            self.set_slot(slot, left, recipes)?;
            self.sync_after_recompute()?;
        }
        Ok(moved)
    }
    fn swap(
        &mut self,
        slot: usize,
        button: i32,
        recipes: &impl StonecutterRecipes,
        time: i64,
        creative: bool,
    ) -> Result<()> {
        let player_slot = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        if slot >= 2 && slot + 7 == player_slot {
            return Ok(());
        }
        let source = self.slot(slot);
        let held = self
            .next_inventory
            .slot(player_slot)
            .expect("hotbar/offhand owner")
            .clone();
        if source.is_empty() && held.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            // SWAP installs the actual hotbar/offhand output before onTake,
            // including that callback's nested broadcastChanges.
            self.next_inventory
                .set_slot_authoritative(player_slot, source.clone());
            if slot == 1 {
                if let Some(take) = self.next_menu.state.prepare_take(recipes, time)? {
                    self.apply_take(take)?;
                }
            } else {
                self.set_slot(slot, ItemStack::empty(), recipes)?;
            }
        } else if source.is_empty() {
            if slot != 1 {
                let amount = held.count.min(limit(&held)?);
                self.set_slot(slot, counted(&held, amount), recipes)?;
                self.next_inventory
                    .set_slot_authoritative(player_slot, counted(&held, held.count - amount));
            }
        } else if slot != 1 {
            let maximum = limit(&held)?;
            if held.count > maximum {
                self.set_slot(slot, counted(&held, maximum), recipes)?;
                self.next_inventory
                    .set_slot_authoritative(player_slot, counted(&held, held.count - maximum));
                let mut displaced = source;
                self.add_to_inventory(&mut displaced, creative)?;
                self.drop_stack(displaced);
            } else {
                self.next_inventory
                    .set_slot_authoritative(player_slot, source);
                self.set_slot(slot, held, recipes)?;
            }
        }
        Ok(())
    }
    /// Inventory.add: hotbar/main free slots, selected/offhand merges and the
    /// creative remainder bypass. Offhand is a merge candidate only.
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
                let amount = stack.count.min((limit(stack)? - existing.count).max(0));
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
                let amount = stack.count.min(limit(stack)?);
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
        recipes: &impl StonecutterRecipes,
        time: i64,
    ) -> Result<()> {
        if self.next_menu.sync.carried().is_empty() || !self.slot(clicked).is_empty() {
            return Ok(());
        }
        let order: Vec<usize> = if button == 0 {
            (0..MENU_SLOTS).collect()
        } else {
            (0..MENU_SLOTS).rev().collect()
        };
        for pass in 0..2 {
            for &slot in &order {
                let source = self.slot(slot);
                let carried = self.next_menu.sync.carried().clone();
                if carried.count >= carried.max_stack_size()? {
                    break;
                }
                if !StonecutterState::may_pick_all(slot)
                    || source.is_empty()
                    || !source.same_item_same_components(&carried)?
                    || source.count > carried.max_stack_size()?
                    || pass == 0 && source.count == source.max_stack_size()?
                {
                    continue;
                }
                let taken = self.take_slot(
                    slot,
                    source.count,
                    carried.max_stack_size()? - carried.count,
                    recipes,
                    time,
                )?;
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
        recipes: &impl StonecutterRecipes,
        time: i64,
        creative: bool,
    ) -> Result<()> {
        let previous = self.next_menu.quickcraft_status;
        self.next_menu.quickcraft_status = (button & 3) as u8;
        if !(previous == 1 && self.next_menu.quickcraft_status == 2)
            && previous != self.next_menu.quickcraft_status
        {
            self.next_menu.reset_quickcraft();
            return Ok(());
        }
        if self.next_menu.sync.carried().is_empty() {
            self.next_menu.reset_quickcraft();
            return Ok(());
        }
        match self.next_menu.quickcraft_status {
            0 => {
                self.next_menu.quickcraft_type = ((button >> 2) & 3) as u8;
                if self.next_menu.quickcraft_type <= 1
                    || self.next_menu.quickcraft_type == 2 && creative
                {
                    self.next_menu.quickcraft_status = 1;
                    self.next_menu.quickcraft_slots.clear();
                } else {
                    self.next_menu.reset_quickcraft();
                }
            }
            1 => {
                if slot >= 0 {
                    let slot = slot as usize;
                    let carried = self.next_menu.sync.carried();
                    let source = self.slot(slot);
                    if slot != 1
                        && (source.is_empty()
                            || source.same_item_same_components(carried)?
                                && source.count <= carried.max_stack_size()?)
                        && (self.next_menu.quickcraft_type == 2
                            || carried.count > self.next_menu.quickcraft_slots.len() as i32)
                    {
                        self.next_menu.quickcraft_slots.insert(slot);
                    }
                }
            }
            2 => {
                if self.next_menu.quickcraft_slots.len() == 1 {
                    let slot = *self.next_menu.quickcraft_slots.iter().next().unwrap();
                    let button = self.next_menu.quickcraft_type as i8;
                    self.next_menu.reset_quickcraft();
                    let click = ContainerClick {
                        window_id: self.next_menu.window_id(),
                        state_id: self.next_menu.state_id(),
                        slot: slot as i16,
                        button,
                        kind: ClickType::Pickup,
                        changed_slots: Default::default(),
                        carried: ItemStack::empty(),
                    };
                    return self.do_click(&click, recipes, time, creative);
                }
                let carried = self.next_menu.sync.carried().clone();
                let mut left = carried.count;
                let slots = self
                    .next_menu
                    .quickcraft_slots
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                let each = match self.next_menu.quickcraft_type {
                    0 => carried.count / slots.len().max(1) as i32,
                    1 => 1,
                    2 => carried.max_stack_size()?,
                    _ => 0,
                };
                for slot in slots {
                    let source = self.slot(slot);
                    if slot == 1
                        || !source.is_empty()
                            && (!source.same_item_same_components(&carried)?
                                || source.count > carried.max_stack_size()?)
                        || self.next_menu.quickcraft_type != 2
                            && carried.count < self.next_menu.quickcraft_slots.len() as i32
                    {
                        continue;
                    }
                    let count = (each + source.count).min(limit(&carried)?);
                    left -= count - source.count;
                    self.set_slot(slot, counted(&carried, count), recipes)?;
                }
                self.next_menu.sync.set_carried(counted(&carried, left));
                self.next_menu.reset_quickcraft();
            }
            _ => self.next_menu.reset_quickcraft(),
        }
        Ok(())
    }
    fn drop_stack(&mut self, stack: ItemStack) {
        if !stack.is_empty() {
            let index = self.outcome.drops.len();
            self.outcome.drops.push(DroppedStack {
                stack,
                around: false,
                trace_item: true,
            });
            self.events.push(StonecutterMenuEvent::Drop { index });
        }
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
fn limit(stack: &ItemStack) -> Result<i32> {
    Ok(99.min(stack.max_stack_size()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{crafting::RecipeBook, workstations::stonecutter::PinnedStonecutterRecipes};
    use std::collections::BTreeMap;
    fn stack(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn click(menu: &StonecutterMenu, slot: i16, button: i8, kind: ClickType) -> ContainerClick {
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
    #[test]
    fn authoritative_slab_flow_shift_result_and_stale_prepared_action() -> Result<()> {
        let recipes = PinnedStonecutterRecipes::new(|_: &ItemStack| Ok(true));
        let mut inventory = PlayerInventory::new();
        inventory.set_slot_authoritative(9, stack("stone", 3));
        let mut menu = StonecutterMenu::new(1)?;
        menu.initialize(&inventory)?;
        let action = menu.prepare_click(
            &click(&menu, 2, 0, ClickType::QuickMove),
            &inventory,
            &recipes,
            11,
            false,
        )?;
        action.commit(&mut menu, &mut inventory)?;
        assert_eq!(menu.state().input(), &stack("stone", 3));
        assert!(inventory.slot(9).unwrap().is_empty());
        let choice = menu
            .state()
            .recipes()
            .iter()
            .position(|id| id == "minecraft:stone_slab_from_stone_stonecutting")
            .unwrap() as i32;
        let action = menu.prepare_button(choice, &inventory, &recipes)?;
        action.commit(&mut menu, &mut inventory)?;
        // Right-click still takes the entire ResultContainer output. A cursor
        // with only one free slot cannot partially remove a two-slab result.
        let mut right_menu = menu.clone();
        let mut right_inventory = inventory.clone();
        let right = right_menu.prepare_click(
            &click(&right_menu, 1, 1, ClickType::Pickup),
            &right_inventory,
            &recipes,
            12,
            false,
        )?;
        right.commit(&mut right_menu, &mut right_inventory)?;
        assert_eq!(right_menu.carried(), &stack("stone_slab", 2));
        assert_eq!(right_menu.state().input(), &stack("stone", 2));
        right_menu.sync.set_carried(stack("stone_slab", 63));
        let blocked = right_menu.prepare_click(
            &click(&right_menu, 1, 0, ClickType::Pickup),
            &right_inventory,
            &recipes,
            12,
            false,
        )?;
        blocked.commit(&mut right_menu, &mut right_inventory)?;
        assert_eq!(right_menu.carried(), &stack("stone_slab", 63));
        assert_eq!(right_menu.state().input(), &stack("stone", 2));
        let throwing = menu.prepare_click(
            &click(&menu, 1, 0, ClickType::Throw),
            &inventory,
            &recipes,
            12,
            false,
        )?;
        let sound = throwing
            .events()
            .iter()
            .position(|e| {
                matches!(
                    e,
                    StonecutterMenuEvent::Effect(StonecutterEffect::PlayTakeResultSound)
                )
            })
            .unwrap();
        let dropped = throwing
            .events()
            .iter()
            .position(|e| matches!(e, StonecutterMenuEvent::Drop { index: 0 }))
            .unwrap();
        let final_sync = throwing
            .events()
            .iter()
            .rposition(|e| matches!(e, StonecutterMenuEvent::Sync(SyncEvent::Packet(_))))
            .unwrap();
        assert!(sound < dropped && dropped < final_sync);
        assert_eq!(throwing.drops()[0].stack, stack("stone_slab", 2));
        let old = menu.prepare_button(choice, &inventory, &recipes)?;
        let action = menu.prepare_click(
            &click(&menu, 1, 0, ClickType::QuickMove),
            &inventory,
            &recipes,
            12,
            false,
        )?;
        assert_eq!(
            action
                .events()
                .iter()
                .filter(|e| matches!(
                    e,
                    StonecutterMenuEvent::Effect(StonecutterEffect::PlayTakeResultSound)
                ))
                .count(),
            1
        );
        action.commit(&mut menu, &mut inventory)?;
        assert!(old.commit(&mut menu, &mut inventory).is_err());
        assert!(menu.state().input().is_empty());
        assert!(menu.state().result_preview().is_empty());
        assert_eq!(inventory.slot(44).unwrap(), &stack("stone_slab", 6));
        Ok(())
    }
    #[test]
    fn drag_swap_and_pick_all_preserve_preview_and_actual_offhand() -> Result<()> {
        let recipes = PinnedStonecutterRecipes::new(|_: &ItemStack| Ok(true));
        let mut inventory = PlayerInventory::new();
        inventory.set_slot_authoritative(9, stack("stone", 8));
        let mut menu = StonecutterMenu::new(2)?;
        menu.initialize(&inventory)?;
        for (slot, button, kind) in [
            (2, 0, ClickType::Pickup),
            (-999, 0, ClickType::QuickCraft),
            (0, 1, ClickType::QuickCraft),
            (1, 1, ClickType::QuickCraft),
            (3, 1, ClickType::QuickCraft),
            (-999, 2, ClickType::QuickCraft),
        ] {
            let action = menu.prepare_click(
                &click(&menu, slot, button, kind),
                &inventory,
                &recipes,
                21,
                false,
            )?;
            action.commit(&mut menu, &mut inventory)?;
        }
        assert_eq!(menu.state().input(), &stack("stone", 4));
        assert_eq!(inventory.slot(10).unwrap(), &stack("stone", 4));
        assert!(menu.carried().is_empty());
        let choice = menu
            .state()
            .recipes()
            .iter()
            .position(|id| id == "minecraft:stone_slab_from_stone_stonecutting")
            .unwrap() as i32;
        menu.prepare_button(choice, &inventory, &recipes)?
            .commit(&mut menu, &mut inventory)?;
        let action = menu.prepare_click(
            &click(&menu, 1, 0, ClickType::Swap),
            &inventory,
            &recipes,
            22,
            false,
        )?;
        let listener = action
            .events()
            .iter()
            .position(|e| {
                matches!(
                    e,
                    StonecutterMenuEvent::Sync(SyncEvent::SlotListener {
                        player_slot: Some(36),
                        ..
                    })
                )
            })
            .unwrap();
        let sound = action
            .events()
            .iter()
            .position(|e| {
                matches!(
                    e,
                    StonecutterMenuEvent::Effect(StonecutterEffect::PlayTakeResultSound)
                )
            })
            .unwrap();
        assert!(listener < sound);
        action.commit(&mut menu, &mut inventory)?;
        assert_eq!(inventory.slot(36).unwrap(), &stack("stone_slab", 2));
        assert_eq!(menu.state().input(), &stack("stone", 3));
        for (slot, button, kind) in [
            (29, 0, ClickType::Pickup),
            (2, 0, ClickType::PickupAll),
            (0, 40, ClickType::Swap),
        ] {
            menu.prepare_click(
                &click(&menu, slot, button, kind),
                &inventory,
                &recipes,
                23,
                false,
            )?
            .commit(&mut menu, &mut inventory)?;
        }
        assert_eq!(menu.carried(), &stack("stone_slab", 2));
        assert_eq!(inventory.slot(45).unwrap(), &stack("stone", 3));
        assert_eq!(inventory.slot(10).unwrap(), &stack("stone", 4));
        assert!(menu.state().input().is_empty());
        assert!(menu.state().result_preview().is_empty());
        // Cursor overflow must drop before a later input insertion packet.
        menu.state.replace_input(stack("stone", 3), &recipes)?;
        menu.sync.set_carried(stack("quartz", 2));
        for slot in 9..45 {
            inventory.set_slot_authoritative(slot, stack("dirt", 64));
        }
        inventory.set_slot_authoritative(9, stack("stone", 63));
        inventory.set_slot_authoritative(45, stack("dirt", 64));
        let closing = menu.prepare_close(&inventory, MenuCloseContext::AliveConnected)?;
        assert!(matches!(
            closing.events(),
            [
                StonecutterMenuEvent::Drop { index: 0 },
                StonecutterMenuEvent::Sync(SyncEvent::Packet(_)),
                StonecutterMenuEvent::Drop { index: 1 }
            ]
        ));
        assert_eq!(closing.drops()[0].stack, stack("quartz", 2));
        assert_eq!(closing.drops()[1].stack, stack("stone", 2));
        let publication = closing.commit(&mut menu, &mut inventory)?;
        assert!(publication.outcome.packets.is_empty());
        assert_eq!(inventory.slot(9).unwrap(), &stack("stone", 64));
        assert!(menu.carried().is_empty());
        assert!(menu.state().input().is_empty());
        Ok(())
    }
}
