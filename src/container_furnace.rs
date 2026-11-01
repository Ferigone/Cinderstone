//! Native Java 1.21 AbstractFurnaceMenu and FurnaceFuel/ResultSlot rules.
//! The furnace and player inventories are borrowed transactionally. Only cursor,
//! drag state, remote synchronization and ordered result callbacks live here.
use crate::{
    container::{ClickOutcome, ClickType, ContainerClick, DroppedStack},
    furnace::{FurnaceKind, FurnaceState},
    inventory::{self, ItemStack, PlayerInventory},
    protocol::{Packet, Writer},
    smelting::{self, CookingKind},
};
use anyhow::{bail, ensure, Result};
use std::collections::BTreeSet;
const MENU_SLOTS: usize = 39;
const OFFHAND: usize = 39;

/// Actual FurnaceResultSlot.checkTakeAchievements callback. The host must
/// consume these in order with its real item-crafting, XP, recipe and RNG owners.
/// Quick-move produces a positive onQuickCraft callback followed by onTake(0).
#[derive(Clone, Debug)]
pub struct FurnaceOutputTake {
    pub stack: ItemStack,
    pub amount: i32,
    pub quick: bool,
    pub slots: [ItemStack; 3],
}
#[derive(Clone, Debug)]
pub struct FurnaceMenu {
    pub window_id: u8,
    state_id: i32,
    carried: ItemStack,
    remote: Vec<ItemStack>,
    remote_carried: ItemStack,
    remote_data: [i32; 4],
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
    output_events: Vec<FurnaceOutputTake>,
}
impl FurnaceMenu {
    pub fn new(window_id: u8) -> Result<Self> {
        ensure!((1..=100).contains(&window_id), "invalid furnace window");
        Ok(Self {
            window_id,
            state_id: 0,
            carried: ItemStack::empty(),
            remote: vec![ItemStack::empty(); MENU_SLOTS],
            remote_carried: ItemStack::empty(),
            remote_data: [0; 4],
            quickcraft_status: 0,
            quickcraft_type: 0,
            quickcraft_slots: BTreeSet::new(),
            output_events: vec![],
        })
    }
    /// IDs from the original MenuType registration order, not block entity IDs.
    pub fn menu_type(kind: FurnaceKind) -> i32 {
        match kind {
            FurnaceKind::Furnace => 14,
            FurnaceKind::BlastFurnace => 10,
            FurnaceKind::Smoker => 22,
        }
    }
    pub fn inventory_snapshot(&self, inventory: &PlayerInventory) -> PlayerInventory {
        let mut value = inventory.clone();
        value.set_carried_authoritative(self.carried.clone());
        value
    }
    pub fn full_sync(
        &mut self,
        furnace: &FurnaceState,
        inventory: &PlayerInventory,
    ) -> Result<Vec<Packet>> {
        let mut work = Engine::new(self, furnace, inventory);
        let mut packets = vec![work.full_sync()?];
        work.sync_data(furnace.data(), true, &mut packets);
        work.commit_menu(self);
        Ok(packets)
    }
    pub fn broadcast_changes(
        &mut self,
        furnace: &FurnaceState,
        inventory: &PlayerInventory,
    ) -> Result<Vec<Packet>> {
        let mut work = Engine::new(self, furnace, inventory);
        let mut packets = vec![];
        work.synchronize(&mut packets)?;
        work.sync_data(furnace.data(), false, &mut packets);
        work.commit_menu(self);
        Ok(packets)
    }
    pub fn take_output_events(&mut self) -> Vec<FurnaceOutputTake> {
        std::mem::take(&mut self.output_events)
    }
    pub fn has_output_events(&self) -> bool {
        !self.output_events.is_empty()
    }
    pub fn click(
        &mut self,
        click: &ContainerClick,
        furnace: &mut FurnaceState,
        inventory: &mut PlayerInventory,
        creative: bool,
    ) -> Result<ClickOutcome> {
        let mut out = ClickOutcome::default();
        if click.window_id != self.window_id || click.slot >= MENU_SLOTS as i16 {
            return Ok(out);
        }
        let stale = click.state_id != self.state_id;
        let mut work = Engine::new(self, furnace, inventory);
        work.do_click(
            click.slot as i32,
            click.button as i32,
            click.kind,
            creative,
            &mut out,
        )?;
        for (&index, stack) in &click.changed_slots {
            if (0..MENU_SLOTS as i16).contains(&index) {
                work.remote[index as usize] = copy_stack(stack);
            }
        }
        work.remote_carried = copy_stack(&click.carried);
        let mut next = furnace.clone();
        for slot in 0..3 {
            if !stack_matches(&furnace.slots[slot], &work.inventory.slots[slot])? {
                next.set_slot(slot, copy_stack(&work.inventory.slots[slot]))?;
            }
        }
        if stale {
            out.packets.push(work.full_sync()?);
            work.sync_data(next.data(), true, &mut out.packets);
        } else {
            work.synchronize(&mut out.packets)?;
            work.sync_data(next.data(), false, &mut out.packets);
        }
        work.commit_inventory(inventory);
        work.commit_menu(self);
        *furnace = next;
        Ok(out)
    }
    pub fn close(&mut self, inventory: &mut PlayerInventory) -> Result<ClickOutcome> {
        // AbstractContainerMenu.removed returns only the carried stack. The
        // three block-entity slots remain in the live furnace owner.
        let mut carried = self.carried.clone();
        let mut player = crate::container::PlayerContainer::from_inventory(inventory.clone());
        player.add_pickup_to_inventory(&mut carried, false)?;
        let mut out = ClickOutcome::default();
        drop_stack(&mut out, carried, false);
        *inventory = player.inventory;
        self.carried = ItemStack::empty();
        self.quickcraft_status = 0;
        self.quickcraft_slots.clear();
        Ok(out)
    }
}
#[derive(Debug)]
struct WorkingInventory {
    slots: Vec<ItemStack>,
    carried: ItemStack,
    selected_hotbar: usize,
}
impl WorkingInventory {
    fn slot(&self, index: usize) -> Option<&ItemStack> {
        self.slots.get(index)
    }
    fn set_slot_authoritative(&mut self, index: usize, stack: ItemStack) {
        self.slots[index] = stack;
    }
    fn carried(&self) -> &ItemStack {
        &self.carried
    }
    fn set_carried_authoritative(&mut self, stack: ItemStack) {
        self.carried = stack;
    }
    fn selected_hotbar(&self) -> usize {
        self.selected_hotbar
    }
}
struct Engine {
    inventory: WorkingInventory,
    kind: FurnaceKind,
    window_id: u8,
    state_id: i32,
    remote: Vec<ItemStack>,
    remote_carried: ItemStack,
    remote_data: [i32; 4],
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
    creative_mode: bool,
    output_events: Vec<FurnaceOutputTake>,
}
impl Engine {
    fn new(menu: &FurnaceMenu, furnace: &FurnaceState, inventory: &PlayerInventory) -> Self {
        let mut slots = furnace.slots.iter().map(copy_stack).collect::<Vec<_>>();
        slots.extend((9..45).map(|i| copy_stack(inventory.slot(i).unwrap())));
        slots.push(copy_stack(inventory.slot(45).unwrap()));
        Self {
            inventory: WorkingInventory {
                slots,
                carried: menu.carried.clone(),
                selected_hotbar: inventory.selected_hotbar(),
            },
            kind: furnace.kind,
            window_id: menu.window_id,
            state_id: menu.state_id,
            remote: menu.remote.clone(),
            remote_carried: menu.remote_carried.clone(),
            remote_data: menu.remote_data,
            quickcraft_status: menu.quickcraft_status,
            quickcraft_type: menu.quickcraft_type,
            quickcraft_slots: menu.quickcraft_slots.clone(),
            creative_mode: false,
            output_events: menu.output_events.clone(),
        }
    }
    fn commit_inventory(&self, inventory: &mut PlayerInventory) {
        for i in 3..39 {
            inventory.set_slot_authoritative(i + 6, self.inventory.slots[i].clone());
        }
        inventory.set_slot_authoritative(45, self.inventory.slots[OFFHAND].clone());
    }
    fn commit_menu(self, menu: &mut FurnaceMenu) {
        menu.state_id = self.state_id;
        menu.carried = self.inventory.carried;
        menu.remote = self.remote;
        menu.remote_carried = self.remote_carried;
        menu.remote_data = self.remote_data;
        menu.quickcraft_status = self.quickcraft_status;
        menu.quickcraft_type = self.quickcraft_type;
        menu.quickcraft_slots = self.quickcraft_slots;
        menu.output_events = self.output_events;
    }
    fn sync_data(&mut self, values: [i32; 4], all: bool, packets: &mut Vec<Packet>) {
        for (index, value) in values.into_iter().enumerate() {
            if all || self.remote_data[index] != value {
                self.remote_data[index] = value;
                let mut w = Writer::new();
                w.u8(self.window_id).i16(index as i16).i16(value as i16);
                packets.push(Packet {
                    id: 0x14,
                    payload: w.into_inner(),
                });
            }
        }
    }
    fn output_event(&mut self, stack: ItemStack, amount: i32, quick: bool) {
        self.output_events.push(FurnaceOutputTake {
            stack,
            amount,
            quick,
            slots: std::array::from_fn(|i| copy_stack(&self.inventory.slots[i])),
        });
    }
    fn cooking_kind(&self) -> CookingKind {
        match self.kind {
            FurnaceKind::Furnace => CookingKind::Smelting,
            FurnaceKind::BlastFurnace => CookingKind::Blasting,
            FurnaceKind::Smoker => CookingKind::Smoking,
        }
    }
    fn next_state(&mut self) -> i32 {
        self.state_id = (self.state_id + 1) & 32767;
        self.state_id
    }

    fn full_sync(&mut self) -> Result<Packet> {
        let slots = self.inventory.slots[..MENU_SLOTS]
            .iter()
            .map(copy_stack)
            .collect::<Vec<_>>();
        self.remote = slots.clone();
        self.remote_carried = copy_stack(&self.inventory.carried);
        let state = self.next_state();
        let mut w = Writer::new();
        w.u8(self.window_id).varint(state).varint(MENU_SLOTS as i32);
        for s in slots {
            w.raw(&s.encode_canonical()?);
        }
        w.raw(&self.inventory.carried.encode_canonical()?);
        Ok(Packet {
            id: 0x13,
            payload: w.into_inner(),
        })
    }

    fn synchronize(&mut self, packets: &mut Vec<Packet>) -> Result<()> {
        for slot in 0..MENU_SLOTS {
            if !stack_matches(&self.inventory.slots[slot], &self.remote[slot])? {
                self.remote[slot] = copy_stack(&self.inventory.slots[slot]);
                let state = self.next_state();
                let mut w = Writer::new();
                w.u8(self.window_id)
                    .varint(state)
                    .i16(slot as i16)
                    .raw(&self.remote[slot].encode_canonical()?);
                packets.push(Packet {
                    id: 0x15,
                    payload: w.into_inner(),
                });
            }
        }
        if !stack_matches(&self.inventory.carried, &self.remote_carried)? {
            self.remote_carried = copy_stack(&self.inventory.carried);
            let state = self.next_state();
            let mut w = Writer::new();
            w.i8(-1)
                .varint(state)
                .i16(-1)
                .raw(&self.remote_carried.encode_canonical()?);
            packets.push(Packet {
                id: 0x15,
                payload: w.into_inner(),
            });
        }
        Ok(())
    }

    fn reset_quickcraft(&mut self) {
        self.quickcraft_status = 0;
        self.quickcraft_slots.clear();
    }

    fn do_click(
        &mut self,
        slot: i32,
        button: i32,
        kind: ClickType,
        creative: bool,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        self.creative_mode = creative;
        if kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, creative, out);
        }
        if self.quickcraft_status != 0 {
            self.reset_quickcraft();
            return Ok(());
        }
        match kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let count = if button == 0 {
                        self.inventory.carried().count
                    } else {
                        1
                    };
                    let dropped = take_stack(self.inventory.carried().clone(), count).0;
                    let left = take_stack(self.inventory.carried().clone(), count).1;
                    self.inventory.set_carried_authoritative(left);
                    if !dropped.is_empty() {
                        drop_stack(out, dropped, true);
                    }
                    return Ok(());
                }
                if slot < 0 {
                    return Ok(());
                }
                let slot = slot as usize;
                if kind == ClickType::QuickMove {
                    if !self.may_pickup(slot, creative)? {
                        return Ok(());
                    }
                    for _ in 0..4096 {
                        let before = self.inventory.slot(slot).unwrap().clone();
                        let moved = self.quick_move_once(slot, creative, out)?;
                        if !moved || self.inventory.slot(slot).unwrap().item_id != before.item_id {
                            break;
                        }
                    }
                } else {
                    self.pickup(slot, button, creative, out)?;
                }
            }
            ClickType::Swap if (0..9).contains(&button) || button == 40 => {
                if slot >= 0 {
                    self.swap(slot as usize, button, creative, out)?;
                }
            }
            ClickType::Clone if creative && slot >= 0 && self.inventory.carried().is_empty() => {
                let source = self.inventory.slot(slot as usize).unwrap();
                let stack = with_count(source, source.max_stack_size()?);
                self.inventory.set_carried_authoritative(stack);
            }
            ClickType::Throw if slot >= 0 && self.inventory.carried().is_empty() => {
                let slot = slot as usize;
                if self.may_pickup(slot, creative)? {
                    let amount = if button == 0 {
                        1
                    } else {
                        self.inventory.slot(slot).unwrap().count
                    };
                    let stack = self.take_slot(slot, amount, i32::MAX, out)?;
                    if !stack.is_empty() {
                        drop_stack(out, stack, true);
                    }
                }
            }
            ClickType::PickupAll if slot >= 0 => {
                self.pickup_all(slot as usize, button, creative, out)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn pickup(
        &mut self,
        slot: usize,
        button: i32,
        creative: bool,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        let source = self.inventory.slot(slot).unwrap().clone();
        let carried = self.inventory.carried().clone();
        if inventory::item_name(source.item_id) == Some("bundle")
            || inventory::item_name(carried.item_id) == Some("bundle")
        {
            bail!("bundle item-click overrides require the bundle feature implementation");
        }
        if source.is_empty() {
            if !carried.is_empty() && self.may_place(slot, &carried)? {
                let amount = if button == 0 { carried.count } else { 1 };
                let amount = amount.min(self.slot_limit(slot, &carried)?);
                let (moved, left) = take_stack(carried, amount);
                self.inventory.set_slot_authoritative(slot, moved);
                self.inventory.set_carried_authoritative(left);
            }
            return Ok(());
        }
        if !self.may_pickup(slot, creative)? {
            return Ok(());
        }
        if carried.is_empty() {
            let amount = if button == 0 {
                source.count
            } else {
                (source.count + 1) / 2
            };
            let taken = self.take_slot(slot, amount, i32::MAX, out)?;
            self.inventory.set_carried_authoritative(taken);
        } else if self.may_place(slot, &carried)? {
            if source.same_item_same_components(&carried)? {
                let amount = if button == 0 { carried.count } else { 1 };
                let amount = amount.min((self.slot_limit(slot, &carried)? - source.count).max(0));
                if amount > 0 {
                    self.inventory
                        .set_slot_authoritative(slot, with_count(&source, source.count + amount));
                    self.inventory
                        .set_carried_authoritative(with_count(&carried, carried.count - amount));
                }
            } else if carried.count <= self.slot_limit(slot, &carried)? {
                self.inventory.set_slot_authoritative(slot, carried);
                self.inventory.set_carried_authoritative(source);
            }
        } else if source.same_item_same_components(&carried)? {
            let amount = (carried.max_stack_size()? - carried.count).max(0);
            let taken = self.take_slot(slot, source.count, amount, out)?;
            self.inventory
                .set_carried_authoritative(with_count(&carried, carried.count + taken.count));
        }
        Ok(())
    }

    fn may_pickup(&self, _slot: usize, _creative: bool) -> Result<bool> {
        Ok(true)
    }

    fn move_to(
        &mut self,
        stack: &mut ItemStack,
        start: usize,
        end: usize,
        reverse: bool,
    ) -> Result<bool> {
        let before = stack.count;
        let indices = if reverse {
            (start..end).rev().collect::<Vec<_>>()
        } else {
            (start..end).collect::<Vec<_>>()
        };
        if stack.is_stackable()? {
            for slot in &indices {
                let existing = self.inventory.slot(*slot).unwrap().clone();
                if !existing.is_empty() && existing.same_item_same_components(stack)? {
                    let amount = stack
                        .count
                        .min((self.slot_limit(*slot, stack)? - existing.count).max(0));
                    if amount > 0 {
                        self.inventory.set_slot_authoritative(
                            *slot,
                            with_count(&existing, existing.count + amount),
                        );
                        *stack = with_count(stack, stack.count - amount);
                    }
                    if stack.is_empty() {
                        break;
                    }
                }
            }
        }
        if !stack.is_empty() {
            for slot in indices {
                if self.inventory.slot(slot).unwrap().is_empty() && self.may_place(slot, stack)? {
                    let amount = stack.count.min(self.slot_limit(slot, stack)?);
                    let (moved, left) = take_stack(stack.clone(), amount);
                    self.inventory.set_slot_authoritative(slot, moved);
                    *stack = left;
                    break;
                }
            }
        }
        Ok(stack.count != before)
    }

    fn add_to_inventory(&mut self, stack: &mut ItemStack) -> Result<()> {
        self.add_to_inventory_inner(stack, false)
    }

    fn add_to_inventory_inner(
        &mut self,
        stack: &mut ItemStack,
        copy_damaged_whole: bool,
    ) -> Result<()> {
        // Inventory's internal indices are hotbar0..8, then main9..35.
        // It tries the selected stack and offhand before scanning other stacks.
        let order = (30..39).chain(3..30).collect::<Vec<_>>();
        let damaged = stack.is_damageable()? && stack.damage_value()? > 0;
        if damaged && copy_damaged_whole {
            if let Some(slot) = order
                .iter()
                .copied()
                .find(|slot| self.inventory.slot(*slot).unwrap().is_empty())
            {
                self.inventory
                    .set_slot_authoritative(slot, std::mem::take(stack));
            } else if self.creative_mode {
                *stack = ItemStack::empty();
            }
            return Ok(());
        }
        if !damaged {
            let selected = 30 + self.inventory.selected_hotbar();
            let mut merges = vec![selected, 39];
            merges.extend(order.iter().copied());
            for slot in merges {
                let existing = self.inventory.slot(slot).unwrap().clone();
                if !existing.is_empty()
                    && existing.is_stackable()?
                    && existing.same_item_same_components(stack)?
                {
                    let amount = stack
                        .count
                        .min((existing.max_stack_size()? - existing.count).max(0));
                    if amount > 0 {
                        self.inventory.set_slot_authoritative(
                            slot,
                            with_count(&existing, existing.count + amount),
                        );
                        *stack = with_count(stack, stack.count - amount);
                    }
                    if stack.is_empty() {
                        break;
                    }
                }
            }
        }
        if !stack.is_empty() {
            for slot in order {
                if self.inventory.slot(slot).unwrap().is_empty() {
                    let amount = stack.count.min(stack.max_stack_size()?);
                    let (moved, left) = take_stack(stack.clone(), amount);
                    self.inventory.set_slot_authoritative(slot, moved);
                    *stack = left;
                    if stack.is_empty() {
                        break;
                    }
                }
            }
        }
        if self.creative_mode && !stack.is_empty() {
            *stack = ItemStack::empty();
        }
        Ok(())
    }

    fn swap(
        &mut self,
        slot: usize,
        button: i32,
        creative: bool,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        let hotbar = if button == 40 {
            39
        } else {
            30 + button as usize
        };
        if slot == hotbar {
            return Ok(());
        }
        let source = self.inventory.slot(slot).unwrap().clone();
        let held = self.inventory.slot(hotbar).unwrap().clone();
        if source.is_empty() && held.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            if self.may_pickup(slot, creative)? {
                let taken = self.take_slot(slot, source.count, i32::MAX, out)?;
                self.inventory.set_slot_authoritative(hotbar, taken);
            }
        } else if source.is_empty() {
            if self.may_place(slot, &held)? {
                let limit = self.slot_limit(slot, &held)?;
                let (moved, left) = take_stack(held, limit);
                self.inventory.set_slot_authoritative(slot, moved);
                self.inventory.set_slot_authoritative(hotbar, left);
            }
        } else if self.may_pickup(slot, creative)? && self.may_place(slot, &held)? {
            let limit = self.slot_limit(slot, &held)?;
            if held.count > limit {
                let (moved, left) = take_stack(held, limit);
                self.inventory.set_slot_authoritative(slot, moved);
                self.inventory.set_slot_authoritative(hotbar, left);
                let mut source = source;
                self.add_to_inventory(&mut source)?;
                if !source.is_empty() {
                    drop_stack(out, source, true);
                }
            } else {
                self.inventory.set_slot_authoritative(slot, held);
                self.inventory.set_slot_authoritative(hotbar, source);
            }
        }
        Ok(())
    }

    fn pickup_all(
        &mut self,
        clicked: usize,
        button: i32,
        creative: bool,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        if self.inventory.carried().is_empty()
            || !self.inventory.slot(clicked).unwrap().is_empty()
                && self.may_pickup(clicked, creative)?
        {
            return Ok(());
        }
        let indices = if button == 0 {
            (0..MENU_SLOTS).collect::<Vec<_>>()
        } else {
            (0..MENU_SLOTS).rev().collect::<Vec<_>>()
        };
        for pass in 0..2 {
            for slot in &indices {
                let source = self.inventory.slot(*slot).unwrap().clone();
                let carried = self.inventory.carried().clone();
                if carried.count >= carried.max_stack_size()? {
                    break;
                }
                if source.is_empty()
                    || !source.same_item_same_components(&carried)?
                    || !self.may_pickup(*slot, creative)?
                    || pass == 0 && source.count == source.max_stack_size()?
                {
                    continue;
                }
                let taken = self.take_slot(
                    *slot,
                    source.count,
                    carried.max_stack_size()? - carried.count,
                    out,
                )?;
                self.inventory
                    .set_carried_authoritative(with_count(&carried, carried.count + taken.count));
            }
        }
        Ok(())
    }

    fn quickcraft(
        &mut self,
        slot: i32,
        button: i32,
        creative: bool,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        let previous = self.quickcraft_status;
        self.quickcraft_status = (button & 3) as u8;
        if !(previous == 1 && self.quickcraft_status == 2) && previous != self.quickcraft_status {
            self.reset_quickcraft();
            return Ok(());
        }
        if self.inventory.carried().is_empty() {
            self.reset_quickcraft();
            return Ok(());
        }
        match self.quickcraft_status {
            0 => {
                self.quickcraft_type = ((button >> 2) & 3) as u8;
                if self.quickcraft_type <= 1 || self.quickcraft_type == 2 && creative {
                    self.quickcraft_status = 1;
                    self.quickcraft_slots.clear();
                } else {
                    self.reset_quickcraft();
                }
            }
            1 => {
                if slot >= 0 {
                    let slot = slot as usize;
                    let carried = self.inventory.carried();
                    let source = self.inventory.slot(slot).unwrap();
                    if self.may_place(slot, carried)?
                        && (source.is_empty()
                            || source.same_item_same_components(carried)?
                                && source.count <= carried.max_stack_size()?)
                        && (self.quickcraft_type == 2
                            || carried.count > self.quickcraft_slots.len() as i32)
                    {
                        self.quickcraft_slots.insert(slot);
                    }
                }
            }
            2 => {
                if self.quickcraft_slots.len() == 1 {
                    let slot = *self.quickcraft_slots.iter().next().unwrap();
                    self.reset_quickcraft();
                    return self.do_click(
                        slot as i32,
                        self.quickcraft_type as i32,
                        ClickType::Pickup,
                        creative,
                        out,
                    );
                }
                let carried = self.inventory.carried().clone();
                let mut left = carried.count;
                let slots = self.quickcraft_slots.iter().copied().collect::<Vec<_>>();
                let each = match self.quickcraft_type {
                    0 => carried.count / (slots.len().max(1) as i32),
                    1 => 1,
                    2 => carried.max_stack_size()?,
                    _ => 0,
                };
                for slot in slots {
                    let source = self.inventory.slot(slot).unwrap().clone();
                    if !self.may_place(slot, &carried)?
                        || !source.is_empty() && !source.same_item_same_components(&carried)?
                        || self.quickcraft_type != 2
                            && carried.count < self.quickcraft_slots.len() as i32
                    {
                        continue;
                    }
                    let count = (each + source.count).min(self.slot_limit(slot, &carried)?);
                    left -= count - source.count;
                    self.inventory
                        .set_slot_authoritative(slot, with_count(&carried, count));
                }
                self.inventory
                    .set_carried_authoritative(with_count(&carried, left));
                self.reset_quickcraft();
            }
            _ => self.reset_quickcraft(),
        }
        Ok(())
    }

    fn may_place(&self, slot: usize, stack: &ItemStack) -> Result<bool> {
        Ok(match slot {
            2 => false,
            1 => smelting::is_fuel(stack)? || inventory::item_name(stack.item_id) == Some("bucket"),
            _ => slot < MENU_SLOTS,
        })
    }
    fn slot_limit(&self, slot: usize, stack: &ItemStack) -> Result<i32> {
        Ok(
            if slot == 1 && inventory::item_name(stack.item_id) == Some("bucket") {
                1
            } else {
                stack.max_stack_size()?.min(99)
            },
        )
    }
    fn take_slot(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        _out: &mut ClickOutcome,
    ) -> Result<ItemStack> {
        let source = self.inventory.slots[slot].clone();
        if source.is_empty() || maximum <= 0 || slot == 2 && maximum < source.count {
            return Ok(ItemStack::empty());
        }
        let (taken, left) = take_stack(source, amount.min(maximum));
        self.inventory.slots[slot] = left;
        if slot == 2 && !taken.is_empty() {
            self.output_event(taken.clone(), taken.count, false);
        }
        Ok(taken)
    }
    fn quick_move_once(
        &mut self,
        slot: usize,
        _creative: bool,
        _out: &mut ClickOutcome,
    ) -> Result<bool> {
        let source = self.inventory.slots[slot].clone();
        if source.is_empty() {
            return Ok(false);
        }
        let mut stack = source.clone();
        let moved = if slot == 2 {
            self.move_to(&mut stack, 3, 39, true)?
        } else if slot < 2 {
            self.move_to(&mut stack, 3, 39, false)?
        } else if smelting::recipe_for(self.cooking_kind(), &stack)?.is_some() {
            self.move_to(&mut stack, 0, 1, false)?
        } else if smelting::is_fuel(&stack)? {
            self.move_to(&mut stack, 1, 2, false)?
        } else if slot < 30 {
            self.move_to(&mut stack, 30, 39, false)?
        } else {
            self.move_to(&mut stack, 3, 30, false)?
        };
        if !moved {
            return Ok(false);
        }
        self.inventory.slots[slot] = stack.clone();
        if slot == 2 {
            // Slot.onQuickCraft(current,original) forwards the ORIGINAL stack.
            self.output_event(source.clone(), source.count - stack.count, true);
            // AbstractFurnaceMenu then invokes onTake with the remainder; its
            // removeCount was reset by the preceding onQuickCraft callback.
            self.output_event(stack, 0, false);
        }
        Ok(true)
    }
}

fn with_count(stack: &ItemStack, count: i32) -> ItemStack {
    if stack.is_empty() || count <= 0 {
        ItemStack::empty()
    } else {
        let mut value = stack.clone();
        value.count = count;
        value
    }
}
fn take_stack(stack: ItemStack, count: i32) -> (ItemStack, ItemStack) {
    let amount = count.max(0).min(stack.count);
    (
        with_count(&stack, amount),
        with_count(&stack, stack.count - amount),
    )
}
fn copy_stack(stack: &ItemStack) -> ItemStack {
    // ItemStack.copy() returns the shared EMPTY value for every isEmpty stack.
    // Physical inventory storage can retain a noncanonical empty object.
    if stack.is_empty() {
        ItemStack::empty()
    } else {
        stack.clone()
    }
}

fn stack_matches(a: &ItemStack, b: &ItemStack) -> Result<bool> {
    // Java compares getCount(), which returns zero for every empty stack,
    // including an in-place negative count with a retained item identity.
    let a_count = if a.is_empty() { 0 } else { a.count };
    let b_count = if b.is_empty() { 0 } else { b.count };
    Ok(a_count == b_count && a.same_item_same_components(b)?)
}

fn drop_stack(out: &mut ClickOutcome, stack: ItemStack, trace_item: bool) {
    if !stack.is_empty() {
        out.drops.push(DroppedStack {
            stack,
            around: false,
            trace_item,
        });
    }
}
