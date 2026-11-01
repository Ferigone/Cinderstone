//! Authoritative bounded StonecutterMenu adapter over the shared sync owner.
//! Player inventory is borrowed per prepared transaction. Main owns dispatch,
//! captured block validity, actual callbacks and publication (see source ledger).
use crate::{
    container::{
        ClickOutcome, ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer,
    },
    inventory::{self, ItemStack, PlayerInventory},
    protocol::{Packet, Reader},
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
/// Read-only source-boundary observations; these are not additional owners.
#[derive(Clone, Debug)]
pub struct StonecutterPhase {
    pub input: ItemStack,
    pub result_preview: ItemStack,
    pub selected_recipe_index: i32,
    pub inventory: PlayerInventory,
    /// The external menu cursor, separate from retained window0 carried state.
    pub carried: ItemStack,
}
#[derive(Clone, Debug)]
pub enum StonecutterMenuEvent {
    Sync {
        event: SyncEvent,
        phase: StonecutterPhase,
    },
    Effect {
        effect: StonecutterEffect,
        phase: StonecutterPhase,
    },
    /// SimpleContainer.setChanged, its slotsChanged callback and default server
    /// update listener. The native kernel already computed its state change.
    InputChanged { phase: StonecutterPhase },
    /// Actual Inventory.setChanged/timesChanged. The actual player owns it.
    PlayerInventoryChanged { phase: StonecutterPhase },
    /// Index into publication.outcome.drops; dispatch at this source position.
    Drop {
        index: usize,
        phase: StonecutterPhase,
    },
    /// Item.onCraftedBy BEFORE even attempting output quick-move insertion.
    ItemCraftedBeforeQuickMove {
        stack: ItemStack,
        phase: StonecutterPhase,
    },
}
impl StonecutterPhase {
    fn observe_return_packet(
        &mut self,
        packet: &Packet,
        returning: &mut ItemStack,
        cursor: bool,
    ) -> Result<()> {
        ensure!(
            packet.id == 0x15,
            "unsupported stonecutter close packet phase"
        );
        let mut reader = Reader::new(&packet.payload);
        ensure!(
            reader.i8()? == -2 && reader.varint()? == 0,
            "stonecutter close requires raw Inventory slot packet"
        );
        let raw = reader.i16()?;
        let slot = match raw {
            0..=8 => 36 + raw as usize,
            9..=35 => raw as usize,
            40 => 45,
            _ => bail!("unsupported stonecutter return index"),
        };
        let stack = ItemStack::decode_exact(reader.remaining())?;
        let old = self.inventory.slot(slot).unwrap();
        ensure!(
            stack.same_item_same_components(returning)?,
            "stonecutter return changed item identity"
        );
        let previous = if old.is_empty() {
            0
        } else {
            ensure!(
                old.same_item_same_components(&stack)?,
                "stonecutter return overwrote item"
            );
            old.count
        };
        let added = stack.count.wrapping_sub(previous);
        ensure!(
            added > 0 && added <= returning.count,
            "invalid stonecutter return phase"
        );
        self.inventory.set_slot_authoritative(slot, stack);
        *returning = counted(returning, returning.count.wrapping_sub(added));
        if cursor {
            self.carried = canonical(returning);
        }
        Ok(())
    }
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
    raw_input: Option<ItemStack>,
    raw_result: Option<ItemStack>,
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
    pub fn initialize(&mut self, inventory: &PlayerInventory) -> Result<Vec<StonecutterMenuEvent>> {
        let mut next = self.clone();
        let top = next.top();
        let data = next.data();
        let out = next.sync.initialize(&top, inventory, &data)?;
        next.advance_revision()?;
        *self = next;
        let phase = self.phase(inventory);
        Ok(out
            .into_iter()
            .map(|event| StonecutterMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect())
    }
    pub fn broadcast_changes(
        &mut self,
        inventory: &PlayerInventory,
    ) -> Result<Vec<StonecutterMenuEvent>> {
        let mut next = self.clone();
        let top = next.top();
        let data = next.data();
        let out = next.sync.broadcast_changes(&top, inventory, &data)?;
        next.advance_revision()?;
        *self = next;
        let phase = self.phase(inventory);
        Ok(out
            .into_iter()
            .map(|event| StonecutterMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect())
    }
    pub fn still_valid(&self, access: &impl WorkstationAccess) -> Result<bool> {
        self.sync.still_valid(access)
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    fn phase(&self, inventory: &PlayerInventory) -> StonecutterPhase {
        StonecutterPhase {
            input: self.state.input().clone(),
            result_preview: self.state.result_preview().clone(),
            selected_recipe_index: self.state.selected_recipe_index(),
            inventory: inventory.clone(),
            carried: self.carried().clone(),
        }
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
            raw_input: None,
            raw_result: None,
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
        action.push_sync(events);
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
        ensure!(
            action.raw_input.is_none() && action.raw_result.is_none(),
            "unflushed stonecutter source-stack phase"
        );
        let top = action.next_menu.top();
        let data = action.next_menu.data();
        let events = action.next_menu.sync.finish_click(
            click,
            before,
            &top,
            &action.next_inventory,
            &data,
        )?;
        action.push_sync(events);
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
        let mut phase = self.phase(inventory);
        let input_before = phase.input.clone();
        let mut returning = phase.carried.clone();
        let mut cursor = true;
        for event in removal.events {
            if returning.is_empty() && cursor {
                cursor = false;
                returning = input_before.clone();
                phase.carried = ItemStack::empty();
                phase.input = ItemStack::empty();
                phase.result_preview = ItemStack::empty();
            }
            match event {
                MenuRemovalEvent::Packet { index } => {
                    let packet = action.outcome.packets[index].clone();
                    phase.observe_return_packet(&packet, &mut returning, cursor)?;
                    action.events.push(StonecutterMenuEvent::Sync {
                        event: SyncEvent::Packet(packet),
                        phase: phase.clone(),
                    });
                }
                MenuRemovalEvent::Drop { index } => {
                    ensure!(
                        action.outcome.drops[index].stack == returning,
                        "stonecutter close drop phase changed"
                    );
                    if cursor {
                        phase.carried = returning.clone();
                    }
                    action.events.push(StonecutterMenuEvent::Drop {
                        index,
                        phase: phase.clone(),
                    });
                    returning = ItemStack::empty();
                    if cursor {
                        phase.carried = ItemStack::empty();
                    }
                }
            }
        }
        action.outcome.packets.clear();
        action.next_menu.advance_revision()?;
        Ok(action)
    }
}
impl PreparedStonecutterAction {
    fn phase(&self) -> StonecutterPhase {
        let mut phase = self.next_menu.phase(&self.next_inventory);
        if let Some(input) = &self.raw_input {
            phase.input = canonical(input);
        }
        if let Some(result) = &self.raw_result {
            phase.result_preview = canonical(result);
        }
        phase
    }
    fn push_sync(&mut self, events: Vec<SyncEvent>) {
        let phase = self.phase();
        self.events
            .extend(events.into_iter().map(|event| StonecutterMenuEvent::Sync {
                event,
                phase: phase.clone(),
            }));
    }
    fn input_changed(&mut self) {
        self.events.push(StonecutterMenuEvent::InputChanged {
            phase: self.phase(),
        });
    }
    fn changed(&mut self, slot: usize, recipes: &impl StonecutterRecipes) -> Result<()> {
        match slot {
            0 => {
                if let Some(input) = self.raw_input.take() {
                    self.next_menu.state.replace_input(input, recipes)?;
                    if self.next_menu.state.input().is_empty() {
                        self.raw_result = None;
                    }
                }
                self.input_changed();
            }
            1 => {} // ResultContainer.setChanged is the original no-op.
            _ => self
                .events
                .push(StonecutterMenuEvent::PlayerInventoryChanged {
                    phase: self.phase(),
                }),
        }
        Ok(())
    }
    fn slot(&self, slot: usize) -> ItemStack {
        match slot {
            0 => self
                .raw_input
                .as_ref()
                .unwrap_or(self.next_menu.state.input())
                .clone(),
            1 => self
                .raw_result
                .as_ref()
                .unwrap_or(self.next_menu.state.result_preview())
                .clone(),
            _ => self
                .next_inventory
                .slot(slot + 7)
                .expect("validated menu slot")
                .clone(),
        }
    }
    /// Slot.setByPlayer: the temporary SimpleContainer setter invokes its own
    /// changed callback, then Slot.set invokes changed once more.
    fn set_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        recipes: &impl StonecutterRecipes,
    ) -> Result<()> {
        match slot {
            0 => {
                self.raw_input = None;
                self.raw_result = None;
                self.next_menu.state.replace_input(stack, recipes)?;
                self.input_changed();
                self.input_changed();
            }
            1 => self.raw_result = Some(canonical(&stack)),
            _ => {
                self.next_inventory
                    .set_slot_authoritative(slot + 7, canonical(&stack));
                self.changed(slot, recipes)?;
            }
        }
        Ok(())
    }
    fn mutate_slot(&mut self, slot: usize, stack: ItemStack) {
        match slot {
            0 => self.raw_input = Some(canonical(&stack)),
            1 => self.raw_result = Some(canonical(&stack)),
            _ => self
                .next_inventory
                .set_slot_authoritative(slot + 7, canonical(&stack)),
        }
    }
    fn sync_after_recompute(&mut self) -> Result<()> {
        let phase = self.phase();
        let events = self.next_menu.sync.broadcast_changes(
            &[phase.input, phase.result_preview],
            &self.next_inventory,
            &[phase.selected_recipe_index],
        )?;
        self.push_sync(events);
        Ok(())
    }
    fn apply_take(&mut self, take: StonecutterTake) -> Result<ItemStack> {
        let effects = take.effects().to_vec();
        // Crafted/reward callbacks precede input consumption. The result
        // container already holds the actual removed/insertion remainder.
        for effect in effects.iter().take_while(|effect| {
            !matches!(effect, StonecutterEffect::InputConsumedAndResultRecomputed)
        }) {
            self.events.push(StonecutterMenuEvent::Effect {
                effect: effect.clone(),
                phase: self.phase(),
            });
        }
        let taken = take.commit(&mut self.next_menu.state)?;
        if self.next_menu.state.input().is_empty() {
            self.raw_result = Some(ItemStack::empty());
        }
        // inputSlot.remove(1) invokes SimpleContainer.setChanged exactly once.
        self.input_changed();
        self.raw_result = None;
        self.raw_input = None;
        self.sync_after_recompute()?;
        let mut after_recompute = false;
        for effect in taken.effects {
            if effect == StonecutterEffect::InputConsumedAndResultRecomputed {
                after_recompute = true;
                continue;
            }
            if after_recompute {
                self.events.push(StonecutterMenuEvent::Effect {
                    effect,
                    phase: self.phase(),
                });
            }
        }
        Ok(taken.output)
    }
    /// Slot.tryRemove, without its caller-specific onTake/cursor ordering.
    fn try_remove_slot(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        recipes: &impl StonecutterRecipes,
    ) -> Result<ItemStack> {
        let source = self.slot(slot);
        if source.is_empty() || maximum <= 0 {
            return Ok(ItemStack::empty());
        }
        ensure!(
            slot != 1,
            "result removal requires its prepared result owner"
        );
        let amount = amount.min(maximum).max(0).min(source.count);
        if amount == 0 {
            return Ok(ItemStack::empty());
        }
        let taken = if slot == 0 {
            let taken = self.next_menu.state.remove_input(amount, recipes)?;
            self.raw_input = None;
            if self.next_menu.state.input().is_empty() {
                self.raw_result = None;
            }
            self.input_changed();
            taken
        } else {
            let taken = counted(&source, amount);
            self.next_inventory.set_slot_authoritative(
                slot + 7,
                counted(&source, source.count.wrapping_sub(amount)),
            );
            taken
        };
        if self.slot(slot).is_empty() {
            self.set_slot(slot, ItemStack::empty(), recipes)?;
        }
        Ok(taken)
    }
    /// Slot.safeTake runs onTake before returning to THROW/PICKUP_ALL callers.
    fn take_slot(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        recipes: &impl StonecutterRecipes,
        time: i64,
    ) -> Result<ItemStack> {
        if slot == 1 {
            let source = self.slot(slot);
            if source.is_empty() || source.count > maximum {
                return Ok(ItemStack::empty());
            }
            if let Some(take) = self.next_menu.state.prepare_take(recipes, time)? {
                self.raw_result = Some(ItemStack::empty());
                return self.apply_take(take);
            }
            return Ok(ItemStack::empty());
        }
        let taken = self.try_remove_slot(slot, amount, maximum, recipes)?;
        if !taken.is_empty() {
            self.changed(slot, recipes)?;
        }
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
                    if button == 1 {
                        self.next_menu
                            .sync
                            .set_carried(counted(&source, source.count.wrapping_sub(amount)));
                        self.drop_stack(counted(&source, amount));
                    } else {
                        self.drop_stack(counted(&source, amount));
                        self.next_menu.sync.set_carried(ItemStack::empty());
                    }
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
        let carried = self.next_menu.carried().clone();
        ensure!(
            inventory::item_name(source.item_id) != Some("bundle")
                && inventory::item_name(carried.item_id) != Some("bundle"),
            "bundle item click overrides require the actual bundle owner"
        );
        if slot == 1 {
            if !source.is_empty()
                && (carried.is_empty()
                    || source.same_item_same_components(&carried)?
                        && carried.max_stack_size()?.wrapping_sub(carried.count) >= source.count)
            {
                if let Some(take) = self.next_menu.state.prepare_take(recipes, time)? {
                    self.raw_result = Some(ItemStack::empty());
                    self.next_menu.sync.set_carried(if carried.is_empty() {
                        take.output_preview().clone()
                    } else {
                        counted(
                            &carried,
                            carried.count.wrapping_add(take.output_preview().count),
                        )
                    });
                    self.apply_take(take)?;
                }
            }
            return Ok(()); // final result Slot.setChanged is a no-op
        }
        if source.is_empty() {
            if !carried.is_empty() {
                let amount = (if button == 0 { carried.count } else { 1 }).min(limit(&carried)?);
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count.wrapping_sub(amount)));
                self.set_slot(slot, counted(&carried, amount), recipes)?;
            }
        } else if carried.is_empty() {
            let amount = if button == 0 {
                source.count
            } else {
                source.count.wrapping_add(1) / 2
            };
            let taken = self.try_remove_slot(slot, amount, i32::MAX, recipes)?;
            if !taken.is_empty() {
                self.next_menu.sync.set_carried(taken);
                self.changed(slot, recipes)?;
            }
        } else if source.same_item_same_components(&carried)? {
            // crq.safeInsert uses SIGNED capacity, including zero/negative.
            let amount = (if button == 0 { carried.count } else { 1 })
                .min(limit(&carried)?.wrapping_sub(source.count));
            self.next_menu
                .sync
                .set_carried(counted(&carried, carried.count.wrapping_sub(amount)));
            self.set_slot(
                slot,
                counted(&source, source.count.wrapping_add(amount)),
                recipes,
            )?;
        } else if carried.count <= limit(&carried)? {
            self.next_menu.sync.set_carried(source);
            self.set_slot(slot, carried, recipes)?;
        }
        self.changed(slot, recipes)?;
        Ok(())
    }
    fn move_to(
        &mut self,
        stack: &mut ItemStack,
        start: usize,
        end: usize,
        reverse: bool,
        source_slot: usize,
        recipes: &impl StonecutterRecipes,
    ) -> Result<bool> {
        let mut moved = false;
        let order: Vec<usize> = if reverse {
            (start..end).rev().collect()
        } else {
            (start..end).collect()
        };
        if stack.is_stackable()? {
            for &slot in &order {
                let existing = self.slot(slot);
                if !existing.is_empty() && existing.same_item_same_components(stack)? {
                    let total = existing.count.wrapping_add(stack.count);
                    let maximum = limit(&existing)?;
                    if total <= maximum {
                        *stack = ItemStack::empty();
                        self.mutate_slot(source_slot, stack.clone());
                        self.mutate_slot(slot, counted(&existing, total));
                        self.changed(slot, recipes)?;
                        moved = true;
                    } else if existing.count < maximum {
                        *stack = counted(
                            stack,
                            stack
                                .count
                                .wrapping_sub(maximum.wrapping_sub(existing.count)),
                        );
                        self.mutate_slot(source_slot, stack.clone());
                        self.mutate_slot(slot, counted(&existing, maximum));
                        self.changed(slot, recipes)?;
                        moved = true;
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
                    let inserted = counted(stack, amount);
                    *stack = counted(stack, stack.count.wrapping_sub(amount));
                    self.mutate_slot(source_slot, stack.clone());
                    self.set_slot(slot, inserted, recipes)?;
                    self.changed(slot, recipes)?;
                    moved = true;
                    break;
                }
            }
        }
        Ok(moved)
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
                    phase: self.phase(),
                });
            if !self.move_to(&mut left, 2, 38, true, slot, recipes)? {
                return Ok(false);
            }
            if left.is_empty() {
                self.set_slot(slot, ItemStack::empty(), recipes)?;
            }
            self.changed(slot, recipes)?; // ResultContainer no-op
            let take = self.next_menu.state.prepare_quick_take(
                recipes,
                time,
                source.count.wrapping_sub(left.count),
            )?;
            self.apply_take(take)?;
            self.sync_after_recompute()?;
            return Ok(true);
        }
        let moved = if slot == 0 {
            self.move_to(&mut left, 2, 38, false, slot, recipes)?
        } else if !recipes.matching_recipes(&left)?.is_empty() {
            self.move_to(&mut left, 0, 1, false, slot, recipes)?
        } else if slot < 29 {
            self.move_to(&mut left, 29, 38, false, slot, recipes)?
        } else {
            self.move_to(&mut left, 2, 29, false, slot, recipes)?
        };
        if moved {
            if left.is_empty() {
                self.set_slot(slot, ItemStack::empty(), recipes)?;
            }
            self.changed(slot, recipes)?;
            self.changed(slot, recipes)?; // Slot.setChanged then onTake
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
        let aliases_held = slot >= 2 && slot + 7 == player_slot;
        let source = self.slot(slot);
        let held = self
            .next_inventory
            .slot(player_slot)
            .expect("actual hotbar/offhand")
            .clone();
        if source.is_empty() && held.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            self.next_inventory
                .set_slot_authoritative(player_slot, source.clone());
            if slot == 1 {
                if let Some(take) = self.next_menu.state.prepare_take(recipes, time)? {
                    self.raw_result = Some(ItemStack::empty());
                    self.apply_take(take)?;
                }
            } else {
                self.set_slot(slot, ItemStack::empty(), recipes)?;
                self.changed(slot, recipes)?;
            }
        } else if source.is_empty() {
            if slot != 1 {
                let amount = held.count.min(limit(&held)?);
                self.next_inventory.set_slot_authoritative(
                    player_slot,
                    counted(&held, held.count.wrapping_sub(amount)),
                );
                self.set_slot(slot, counted(&held, amount), recipes)?;
            }
        } else if slot != 1 {
            let maximum = limit(&held)?;
            if held.count > maximum {
                let remainder = held.count.wrapping_sub(maximum);
                self.next_inventory
                    .set_slot_authoritative(player_slot, counted(&held, remainder));
                self.set_slot(slot, counted(&held, maximum), recipes)?;
                self.changed(slot, recipes)?;
                let mut displaced = if aliases_held {
                    counted(&source, remainder)
                } else {
                    source
                };
                self.add_to_inventory(&mut displaced, creative)?;
                self.drop_stack(displaced);
            } else {
                self.next_inventory
                    .set_slot_authoritative(player_slot, source);
                self.set_slot(slot, held, recipes)?;
                self.changed(slot, recipes)?;
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
                    0 => ((carried.count as f32) / (slots.len().max(1) as f32)).floor() as i32,
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
                    let count = each.wrapping_add(source.count).min(limit(&carried)?);
                    left = left.wrapping_sub(count.wrapping_sub(source.count));
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
            self.events.push(StonecutterMenuEvent::Drop {
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
                    StonecutterMenuEvent::Effect {
                        effect: StonecutterEffect::PlayTakeResultSound,
                        ..
                    }
                )
            })
            .unwrap();
        let dropped = throwing
            .events()
            .iter()
            .position(|e| matches!(e, StonecutterMenuEvent::Drop { index: 0, .. }))
            .unwrap();
        let final_sync = throwing
            .events()
            .iter()
            .rposition(|e| {
                matches!(
                    e,
                    StonecutterMenuEvent::Sync {
                        event: SyncEvent::Packet(_),
                        ..
                    }
                )
            })
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
                    StonecutterMenuEvent::Effect {
                        effect: StonecutterEffect::PlayTakeResultSound,
                        ..
                    }
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
                    StonecutterMenuEvent::Sync {
                        event: SyncEvent::SlotListener {
                            player_slot: Some(36),
                            ..
                        },
                        ..
                    }
                )
            })
            .unwrap();
        let sound = action
            .events()
            .iter()
            .position(|e| {
                matches!(
                    e,
                    StonecutterMenuEvent::Effect {
                        effect: StonecutterEffect::PlayTakeResultSound,
                        ..
                    }
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
                StonecutterMenuEvent::Drop { index: 0, .. },
                StonecutterMenuEvent::Sync {
                    event: SyncEvent::Packet(_),
                    ..
                },
                StonecutterMenuEvent::Drop { index: 1, .. }
            ]
        ));
        assert_eq!(closing.drops()[0].stack, stack("quartz", 2));
        assert_eq!(closing.drops()[1].stack, stack("stone", 2));
        let cursor_phase = closing
            .events()
            .iter()
            .find_map(|event| match event {
                StonecutterMenuEvent::Drop { index: 0, phase } => Some(phase),
                _ => None,
            })
            .unwrap();
        assert_eq!(cursor_phase.input, stack("stone", 3));
        assert_eq!(cursor_phase.carried, stack("quartz", 2));
        let input_phase = closing
            .events()
            .iter()
            .find_map(|event| match event {
                StonecutterMenuEvent::Drop { index: 1, phase } => Some(phase),
                _ => None,
            })
            .unwrap();
        assert!(input_phase.input.is_empty() && input_phase.carried.is_empty());
        assert_eq!(input_phase.inventory.slot(9).unwrap(), &stack("stone", 64));
        let publication = closing.commit(&mut menu, &mut inventory)?;
        assert!(publication.outcome.packets.is_empty());
        assert_eq!(inventory.slot(9).unwrap(), &stack("stone", 64));
        assert!(menu.carried().is_empty());
        assert!(menu.state().input().is_empty());
        Ok(())
    }
    fn apply(
        menu: &mut StonecutterMenu,
        inventory: &mut PlayerInventory,
        recipes: &impl StonecutterRecipes,
        slot: i16,
        button: i8,
        kind: ClickType,
        time: i64,
    ) -> Result<StonecutterPublication> {
        menu.prepare_click(
            &click(menu, slot, button, kind),
            inventory,
            recipes,
            time,
            false,
        )?
        .commit(menu, inventory)
    }
    fn player_changes(events: &[StonecutterMenuEvent]) -> Vec<&StonecutterPhase> {
        events
            .iter()
            .filter_map(|event| match event {
                StonecutterMenuEvent::PlayerInventoryChanged { phase } => Some(phase),
                _ => None,
            })
            .collect()
    }
    #[test]
    fn native_self_swap_signed_zero_capacity_and_actual_counter_phases() -> Result<()> {
        let recipes = PinnedStonecutterRecipes::new(|_: &ItemStack| Ok(true));
        let mut inventory = PlayerInventory::new();
        inventory.set_slot_authoritative(36, stack("stone", 1));
        let mut menu = StonecutterMenu::new(5)?;
        menu.initialize(&inventory)?;
        let swapped = apply(
            &mut menu,
            &mut inventory,
            &recipes,
            29,
            0,
            ClickType::Swap,
            30,
        )?;
        assert_eq!(player_changes(&swapped.events).len(), 2);
        assert!(player_changes(&swapped.events).iter().all(|phase| phase
            .inventory
            .slot(36)
            .unwrap()
            == &stack("stone", 1)));
        // Accepted imported/raw overfull fixture: source/held are one object.
        inventory.set_slot_authoritative(36, stack("iron_sword", 2));
        let swapped = apply(
            &mut menu,
            &mut inventory,
            &recipes,
            29,
            0,
            ClickType::Swap,
            30,
        )?;
        assert_eq!(player_changes(&swapped.events).len(), 2);
        assert_eq!(inventory.slot(36).unwrap(), &stack("iron_sword", 1));
        assert_eq!(inventory.slot(37).unwrap(), &stack("iron_sword", 1));
        assert!(swapped.outcome.drops.is_empty());
        inventory.set_slot_authoritative(9, stack("stone", 99));
        menu.sync.set_carried(stack("stone", 1));
        let inserted = apply(
            &mut menu,
            &mut inventory,
            &recipes,
            2,
            0,
            ClickType::Pickup,
            30,
        )?;
        assert_eq!(inventory.slot(9).unwrap(), &stack("stone", 64));
        assert_eq!(menu.carried(), &stack("stone", 36));
        assert_eq!(player_changes(&inserted.events).len(), 2);
        assert!(player_changes(&inserted.events)
            .iter()
            .all(|phase| phase.carried == stack("stone", 36)));
        menu.sync.set_carried(stack("stone", 1));
        let zero = apply(
            &mut menu,
            &mut inventory,
            &recipes,
            2,
            0,
            ClickType::Pickup,
            30,
        )?;
        assert_eq!(player_changes(&zero.events).len(), 2);
        assert_eq!(menu.carried(), &stack("stone", 1));
        assert_eq!(inventory.slot(9).unwrap(), &stack("stone", 64));
        menu.sync.set_carried(stack("stone", 64));
        apply(
            &mut menu,
            &mut inventory,
            &recipes,
            0,
            0,
            ClickType::Pickup,
            30,
        )?;
        menu.sync.set_carried(stack("stone", 1));
        let zero = apply(
            &mut menu,
            &mut inventory,
            &recipes,
            0,
            0,
            ClickType::Pickup,
            30,
        )?;
        assert_eq!(
            zero.events
                .iter()
                .filter(|event| matches!(event, StonecutterMenuEvent::InputChanged { .. }))
                .count(),
            3
        );
        assert_eq!(menu.state().input(), &stack("stone", 64));
        assert_eq!(menu.carried(), &stack("stone", 1));
        Ok(())
    }
    #[test]
    fn crafted_callbacks_keep_preconsumption_input_and_recomputed_result_phases() -> Result<()> {
        let recipes = PinnedStonecutterRecipes::new(|_: &ItemStack| Ok(true));
        let mut inventory = PlayerInventory::new();
        let mut menu = StonecutterMenu::new(6)?;
        menu.initialize(&inventory)?;
        menu.sync.set_carried(stack("stone", 3));
        apply(
            &mut menu,
            &mut inventory,
            &recipes,
            0,
            0,
            ClickType::Pickup,
            40,
        )?;
        let choice = menu
            .state()
            .recipes()
            .iter()
            .position(|id| id == "minecraft:stone_slab_from_stone_stonecutting")
            .unwrap() as i32;
        menu.prepare_button(choice, &inventory, &recipes)?
            .commit(&mut menu, &mut inventory)?;
        let mut input_shift_menu = menu.clone();
        let mut input_shift_inventory = inventory.clone();
        let input_shift = apply(
            &mut input_shift_menu,
            &mut input_shift_inventory,
            &recipes,
            0,
            0,
            ClickType::QuickMove,
            40,
        )?;
        let destination = player_changes(&input_shift.events);
        assert_eq!(destination.len(), 2);
        assert!(destination[0].input.is_empty());
        assert_eq!(destination[0].selected_recipe_index, choice);
        assert_eq!(destination[0].result_preview, stack("stone_slab", 2));
        let changed = input_shift
            .events
            .iter()
            .filter_map(|event| match event {
                StonecutterMenuEvent::InputChanged { phase } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 4);
        assert!(changed
            .iter()
            .all(|phase| phase.selected_recipe_index == -1 && phase.result_preview.is_empty()));
        let taken = apply(
            &mut menu,
            &mut inventory,
            &recipes,
            1,
            1,
            ClickType::Pickup,
            40,
        )?;
        let crafted = taken
            .events
            .iter()
            .find_map(|event| match event {
                StonecutterMenuEvent::Effect {
                    effect: StonecutterEffect::CraftedBy { .. },
                    phase,
                } => Some(phase),
                _ => None,
            })
            .unwrap();
        assert_eq!(crafted.input, stack("stone", 3));
        assert!(crafted.result_preview.is_empty());
        assert_eq!(crafted.carried, stack("stone_slab", 2));
        let consumed = taken
            .events
            .iter()
            .find_map(|event| match event {
                StonecutterMenuEvent::InputChanged { phase } => Some(phase),
                _ => None,
            })
            .unwrap();
        assert_eq!(consumed.input, stack("stone", 2));
        assert!(consumed.result_preview.is_empty());
        let sounded = taken
            .events
            .iter()
            .find_map(|event| match event {
                StonecutterMenuEvent::Effect {
                    effect: StonecutterEffect::PlayTakeResultSound,
                    phase,
                } => Some(phase),
                _ => None,
            })
            .unwrap();
        assert_eq!(sounded.input, stack("stone", 2));
        assert_eq!(sounded.result_preview, stack("stone_slab", 2));
        let shifted = apply(
            &mut menu,
            &mut inventory,
            &recipes,
            1,
            0,
            ClickType::QuickMove,
            41,
        )?;
        assert_eq!(player_changes(&shifted.events).len(), 3); // empty destination2, later merge1
        let crafted = shifted
            .events
            .iter()
            .filter_map(|event| match event {
                StonecutterMenuEvent::Effect {
                    effect: StonecutterEffect::CraftedBy { amount: 0, .. },
                    phase,
                } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(crafted.len(), 2);
        assert_eq!(crafted[0].input, stack("stone", 2));
        assert_eq!(crafted[1].input, stack("stone", 1));
        assert!(crafted.iter().all(|phase| phase.result_preview.is_empty()));
        assert_eq!(inventory.slot(44).unwrap(), &stack("stone_slab", 4));
        assert_eq!(menu.carried(), &stack("stone_slab", 2));
        assert!(menu.state().input().is_empty());
        Ok(())
    }
}
