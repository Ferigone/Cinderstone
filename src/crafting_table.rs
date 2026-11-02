//! Native1.21 CraftingMenu. Canonical inventory is borrowed only for each
//! atomic menu action; the retained owner stores its grid/cursor/remote baseline.
use crate::{
    container::{ClickOutcome, ClickType, ContainerClick, DroppedStack, MenuCloseContext},
    crafting::{CraftResult, RecipeBook, RecipeContext, RecipeInput},
    inventory::{self, ItemStack, PlayerInventory},
    protocol::{Packet, Writer},
};
use anyhow::{bail, ensure, Result};
use std::collections::BTreeSet;
const MENU_SLOTS: usize = 46;
#[derive(Clone, Debug)]
pub struct CraftingTableMenu {
    pub window_id: u8,
    state_id: i32,
    inputs: Vec<ItemStack>,
    result: ItemStack,
    carried: ItemStack,
    last: Vec<ItemStack>,
    local_initialized: bool,
    remote: Vec<ItemStack>,
    remote_carried: ItemStack,
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
    crafting: Option<(String, CraftResult)>,
    crafted_items: Vec<(i32, i32)>,
}
impl CraftingTableMenu {
    pub fn new(window_id: u8) -> Result<Self> {
        ensure!((1..=100).contains(&window_id), "invalid native window");
        Ok(Self {
            window_id,
            state_id: 0,
            inputs: vec![ItemStack::empty(); 9],
            result: ItemStack::empty(),
            carried: ItemStack::empty(),
            last: vec![ItemStack::empty(); MENU_SLOTS],
            local_initialized: false,
            remote: vec![ItemStack::empty(); 46],
            remote_carried: ItemStack::empty(),
            quickcraft_status: 0,
            quickcraft_type: 0,
            quickcraft_slots: BTreeSet::new(),
            crafting: None,
            crafted_items: vec![],
        })
    }
    pub fn carried(&self) -> &ItemStack { &self.carried }
    pub fn inputs(&self) -> &[ItemStack] { &self.inputs }
    pub fn inventory_snapshot(&self, inventory: &PlayerInventory) -> PlayerInventory {
        let mut value = inventory.clone();
        value.set_carried_authoritative(self.carried.clone());
        value
    }
    pub fn full_sync(&mut self, inventory: &PlayerInventory) -> Result<Packet> {
        let mut work = Engine::new(self, inventory);
        // Opening installs the player's slot listener before the synchronizer.
        // A later sendAllDataToRemote refreshes only the remote baseline.
        if !work.local_initialized {
            work.capture_last_slots();
        }
        let packet = work.full_sync()?;
        work.commit_menu(self);
        Ok(packet)
    }
    pub fn broadcast_changes(&mut self, inventory: &PlayerInventory) -> Result<Vec<Packet>> {
        let mut work = Engine::new(self, inventory);
        let mut packets = vec![];
        work.synchronize(&mut packets)?;
        work.commit_menu(self);
        Ok(packets)
    }
    pub fn take_crafted_items(&mut self) -> Vec<(i32, i32)> {
        std::mem::take(&mut self.crafted_items)
    }
    pub fn click(
        &mut self,
        click: &ContainerClick,
        inventory: &mut PlayerInventory,
        creative: bool,
        context: &RecipeContext,
    ) -> Result<ClickOutcome> {
        let mut out = ClickOutcome::default();
        if click.window_id != self.window_id || click.slot >= MENU_SLOTS as i16 {
            return Ok(out);
        }
        let stale = click.state_id != self.state_id;
        let mut work = Engine::new(self, inventory);
        work.do_click(
            click.slot as i32,
            click.button as i32,
            click.kind,
            creative,
            context,
            &mut out,
        )?;
        out.packets.append(&mut work.pending_packets);
        for (&index, stack) in &click.changed_slots {
            if (0..MENU_SLOTS as i16).contains(&index) {
                work.remote[index as usize] = copy_stack(stack);
            }
        }
        work.remote_carried = copy_stack(&click.carried);
        if stale {
            // broadcastFullState observes local slots before sending remote data.
            work.capture_last_slots();
            out.packets.push(work.full_sync()?);
        } else {
            work.synchronize(&mut out.packets)?;
        }
        work.commit_inventory(inventory);
        work.commit_menu(self);
        Ok(out)
    }
    pub fn close(&mut self, inventory: &mut PlayerInventory) -> Result<ClickOutcome> {
        self.close_with_context(inventory, MenuCloseContext::AliveConnected)
    }
    pub fn close_with_context(
        &mut self,
        inventory: &mut PlayerInventory,
        context: MenuCloseContext,
    ) -> Result<ClickOutcome> {
        let mut work = Engine::new(self, inventory);
        let mut out = ClickOutcome::default();
        work.reset_quickcraft();
        // CraftingMenu.removed invokes AbstractContainerMenu.removed first,
        // then clearContainer on the actual nine-slot input container. The
        // result is a recipe preview, never an additional owned drop.
        let mut carried = std::mem::take(&mut work.inventory.carried);
        work.return_or_drop(&mut carried, context, &mut out)?;
        for slot in 1..10 {
            let mut stack = std::mem::take(&mut work.inventory.slots[slot]);
            work.return_or_drop(&mut stack, context, &mut out)?;
        }
        work.inventory.slots[0] = ItemStack::empty();
        work.crafting = None;
        // removed() returns the grid; its result container disappears with this menu.
        work.commit_inventory(inventory);
        work.commit_menu(self);
        Ok(out)
    }
    /// Match the old menu's actual shared container/index pairs. Only its 36
    /// inventory slots belong to InventoryMenu; the grid/result/cursor do not.
    /// Returning the grid/cursor does not overwrite these old sync baselines.
    pub fn transfer_inventory_remote_state(
        &self,
        player: &mut crate::container::PlayerContainer,
    ) -> Result<()> {
        for slot in 10..MENU_SLOTS {
            player.transfer_external_slot_state(slot - 1, &self.last[slot], &self.remote[slot])?;
        }
        Ok(())
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
    fn slots(&self) -> &[ItemStack] {
        &self.slots
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
    window_id: u8,
    state_id: i32,
    last: Vec<ItemStack>,
    local_initialized: bool,
    remote: Vec<ItemStack>,
    remote_carried: ItemStack,
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
    crafting: Option<(String, CraftResult)>,
    creative_mode: bool,
    crafted_items: Vec<(i32, i32)>,
    pending_packets: Vec<Packet>,
}
impl Engine {
    fn return_or_drop(
        &mut self,
        stack: &mut ItemStack,
        context: MenuCloseContext,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        match context {
            MenuCloseContext::AliveConnected => self.return_to_inventory(stack, out),
            MenuCloseContext::DeadOrDisconnected => {
                // Player.drop(stack, false): around=false, traceItem=false.
                // The Main host owns the real Item/RNG transaction.
                drop_stack(out, std::mem::take(stack), false);
                Ok(())
            }
        }
    }
    /// Inventory.placeItemBackInInventory(stack, true), including the explicit
    /// inventory-index packet after every successful insertion.
    fn return_to_inventory(&mut self, stack: &mut ItemStack, out: &mut ClickOutcome) -> Result<()> {
        while !stack.is_empty() {
            let order = (37..46).chain(10..37).collect::<Vec<_>>();
            let mut candidates = vec![37 + self.inventory.selected_hotbar(), 46];
            candidates.extend(order.iter().copied());
            let mut destination = None;
            for slot in candidates {
                let existing = &self.inventory.slots[slot];
                if !existing.is_empty()
                    && existing.is_stackable()?
                    && existing.same_item_same_components(stack)?
                    && existing.count < existing.max_stack_size()?.min(99)
                {
                    destination = Some(slot);
                    break;
                }
            }
            if destination.is_none() {
                destination = order
                    .into_iter()
                    .find(|slot| self.inventory.slots[*slot].is_empty());
            }
            let Some(slot) = destination else {
                drop_stack(out, std::mem::take(stack), false);
                break;
            };
            let existing = self.inventory.slots[slot].count.max(0);
            let maximum = stack.max_stack_size()?;
            ensure!((1..=99).contains(&maximum), "invalid native cursor stack limit");
            let amount = stack.count.min(maximum - existing);
            ensure!(
                amount > 0,
                "no room in selected native inventory destination"
            );
            self.inventory.slots[slot] = with_count(stack, existing + amount);
            *stack = with_count(stack, stack.count - amount);
            let index = match slot {
                37..=45 => slot - 37,
                46 => 40,
                _ => slot - 1,
            };
            let mut w = Writer::new();
            w.i8(-2)
                .varint(0)
                .i16(index as i16)
                .raw(&self.inventory.slots[slot].encode_canonical()?);
            out.packets.push(Packet {
                id: 0x15,
                payload: w.into_inner(),
            });
        }
        Ok(())
    }
    fn new(menu: &CraftingTableMenu, inventory: &PlayerInventory) -> Self {
        let mut slots = vec![copy_stack(&menu.result)];
        slots.extend(menu.inputs.iter().map(copy_stack));
        slots.extend((9..45).map(|i| copy_stack(inventory.slot(i).unwrap())));
        slots.push(inventory.slot(45).unwrap().clone());
        Self {
            inventory: WorkingInventory {
                slots,
                carried: menu.carried.clone(),
                selected_hotbar: inventory.selected_hotbar(),
            },
            window_id: menu.window_id,
            state_id: menu.state_id,
            last: menu.last.clone(),
            local_initialized: menu.local_initialized,
            remote: menu.remote.clone(),
            remote_carried: menu.remote_carried.clone(),
            quickcraft_status: menu.quickcraft_status,
            quickcraft_type: menu.quickcraft_type,
            quickcraft_slots: menu.quickcraft_slots.clone(),
            crafting: menu.crafting.clone(),
            creative_mode: false,
            crafted_items: menu.crafted_items.clone(),
            pending_packets: vec![],
        }
    }
    fn commit_inventory(&self, inventory: &mut PlayerInventory) {
        for i in 10..46 {
            inventory.set_slot_authoritative(i - 1, self.inventory.slots[i].clone());
        }
        inventory.set_slot_authoritative(45, self.inventory.slots[46].clone());
    }
    fn commit_menu(self, menu: &mut CraftingTableMenu) {
        menu.state_id = self.state_id;
        menu.inputs = self.inventory.slots[1..10].to_vec();
        menu.result = self.inventory.slots[0].clone();
        menu.carried = self.inventory.carried;
        menu.last = self.last;
        menu.local_initialized = self.local_initialized;
        menu.remote = self.remote;
        menu.remote_carried = self.remote_carried;
        menu.quickcraft_status = self.quickcraft_status;
        menu.quickcraft_type = self.quickcraft_type;
        menu.quickcraft_slots = self.quickcraft_slots;
        menu.crafting = self.crafting;
        menu.crafted_items = self.crafted_items;
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
        self.capture_last_slots();
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
    fn capture_last_slots(&mut self) {
        self.last = self.inventory.slots[..MENU_SLOTS]
            .iter()
            .map(copy_stack)
            .collect();
        self.local_initialized = true;
    }
    fn refresh_crafting(&mut self, context: &RecipeContext) -> Result<()> {
        let input = RecipeInput::crafting(3, 3, self.inventory.slots[1..10].to_vec())?;
        let book = RecipeBook::vanilla();
        if inventory::item_name(self.inventory.slots[5].item_id) == Some("filled_map")
            && (1..10).filter(|slot| *slot != 5).all(|slot| {
                inventory::item_name(self.inventory.slots[slot].item_id) == Some("paper")
            })
        {
            let value = self.inventory.slots[5].component_value(26)?;
            let map_id = value
                .as_ref()
                .and_then(serde_json::Value::as_i64)
                .map(|id| id as i32);
            ensure!(
                map_id.is_some_and(|id| context.maps.contains_key(&id)),
                "map-extension matching requires its actual saved map data"
            );
        }
        let mut matching = None;
        for id in book.recipe_ids() {
            if let Some(result) = book.evaluate_with_context(id, &input, context)? {
                if result.stack.is_empty() {
                    continue;
                }
                if let Some((previous, _)) = &matching {
                    bail!("ambiguous original recipe-manager order: {previous} and {id}");
                }
                ensure!(
                    result.stack.component_value(27)?.is_none()
                        && result.stack.component_value(28)?.is_none(),
                    "map crafting requires actual post-processing owner"
                );
                matching = Some((id.to_owned(), result));
            }
        }
        let result = matching
            .as_ref()
            .map_or_else(ItemStack::empty, |(_, r)| r.stack.clone());
        self.inventory.slots[0] = result.clone();
        self.remote[0] = copy_stack(&result);
        self.crafting = matching;
        let state = self.next_state();
        let mut w = Writer::new();
        w.u8(self.window_id)
            .varint(state)
            .i16(0)
            .raw(&result.encode_canonical()?);
        self.pending_packets.push(Packet {
            id: 0x15,
            payload: w.into_inner(),
        });
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
        context: &RecipeContext,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        self.creative_mode = creative;
        if kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, creative, context, out);
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
                        let moved = self.quick_move_once(slot, creative, context, out)?;
                        if !moved || self.inventory.slot(slot).unwrap().item_id != before.item_id {
                            break;
                        }
                    }
                } else {
                    self.pickup(slot, button, creative, context, out)?;
                }
            }
            ClickType::Swap if (0..9).contains(&button) || button == 40 => {
                if slot >= 0 {
                    self.swap(slot as usize, button, creative, context, out)?;
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
                    let stack = self.take_slot(slot, amount, i32::MAX, context, out)?;
                    if !stack.is_empty() {
                        drop_stack(out, stack, true);
                    }
                }
            }
            ClickType::PickupAll if slot >= 0 => {
                self.pickup_all(slot as usize, button, creative, context, out)?;
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
        context: &RecipeContext,
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
            if !carried.is_empty() && self.may_place(slot, &carried) {
                let amount = if button == 0 { carried.count } else { 1 };
                let amount = amount.min(self.slot_limit(slot, &carried)?);
                let (moved, left) = take_stack(carried, amount);
                self.inventory.set_slot_authoritative(slot, moved);
                self.inventory.set_carried_authoritative(left);
                self.refresh_if_grid(slot, context)?;
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
            let taken = self.take_slot(slot, amount, i32::MAX, context, out)?;
            self.inventory.set_carried_authoritative(taken);
        } else if self.may_place(slot, &carried) {
            if source.same_item_same_components(&carried)? {
                let amount = if button == 0 { carried.count } else { 1 };
                let amount = amount.min((self.slot_limit(slot, &carried)? - source.count).max(0));
                if amount > 0 {
                    self.inventory
                        .set_slot_authoritative(slot, with_count(&source, source.count + amount));
                    self.inventory
                        .set_carried_authoritative(with_count(&carried, carried.count - amount));
                    self.refresh_if_grid(slot, context)?;
                }
            } else if carried.count <= self.slot_limit(slot, &carried)? {
                self.inventory.set_slot_authoritative(slot, carried);
                self.inventory.set_carried_authoritative(source);
                self.refresh_if_grid(slot, context)?;
            }
        } else if source.same_item_same_components(&carried)? {
            let amount = (carried.max_stack_size()? - carried.count).max(0);
            let taken = self.take_slot(slot, source.count, amount, context, out)?;
            self.inventory
                .set_carried_authoritative(with_count(&carried, carried.count + taken.count));
        }
        Ok(())
    }
    fn may_place(&self, slot: usize, _stack: &ItemStack) -> bool {
        slot != 0 && slot < MENU_SLOTS
    }

    fn may_pickup(&self, _slot: usize, _creative: bool) -> Result<bool> {
        Ok(true)
    }

    fn slot_limit(&self, _slot: usize, stack: &ItemStack) -> Result<i32> {
        Ok(stack.max_stack_size()?.min(99))
    }

    fn refresh_if_grid(&mut self, slot: usize, context: &RecipeContext) -> Result<()> {
        if (1..10).contains(&slot) {
            self.refresh_crafting(context)?;
        }
        Ok(())
    }
    fn take_slot(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        context: &RecipeContext,
        out: &mut ClickOutcome,
    ) -> Result<ItemStack> {
        let source = self.inventory.slot(slot).unwrap().clone();
        if source.is_empty() || maximum <= 0 || slot == 0 && maximum < source.count {
            return Ok(ItemStack::empty());
        }
        if slot == 0 {
            self.crafted_items
                .push((source.item_id, amount.min(source.count)));
            self.inventory.set_slot_authoritative(0, ItemStack::empty());
            self.consume_craft(context, out)?;
            return Ok(source);
        }
        let (taken, left) = take_stack(source, amount.min(maximum));
        self.inventory.set_slot_authoritative(slot, left);
        self.refresh_if_grid(slot, context)?;
        Ok(taken)
    }
    fn consume_craft(&mut self, context: &RecipeContext, out: &mut ClickOutcome) -> Result<()> {
        let Some((id, result)) = self.crafting.clone() else {
            return Ok(());
        };
        out.crafted.push(id);
        let occupied = (0..9)
            .filter(|i| !self.inventory.slots[i + 1].is_empty())
            .collect::<Vec<_>>();
        if occupied.is_empty() {
            return Ok(());
        }
        let min_x = occupied.iter().map(|i| i % 3).min().unwrap();
        let max_x = occupied.iter().map(|i| i % 3).max().unwrap();
        let min_y = occupied.iter().map(|i| i / 3).min().unwrap();
        let max_y = occupied.iter().map(|i| i / 3).max().unwrap();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let index = y * 3 + x;
                let slot = index + 1;
                let source = self.inventory.slots[slot].clone();
                if !source.is_empty() {
                    self.inventory.slots[slot] = with_count(&source, source.count - 1);
                    self.refresh_crafting(context)?;
                }
                let mut remainder = result.remaining[index].clone();
                if remainder.is_empty() {
                    continue;
                }
                let left = self.inventory.slots[slot].clone();
                if left.is_empty() {
                    self.inventory.slots[slot] = remainder;
                    self.refresh_crafting(context)?;
                } else if left.same_item_same_components(&remainder)? {
                    remainder.count += left.count;
                    self.inventory.slots[slot] = remainder;
                    self.refresh_crafting(context)?;
                } else {
                    self.add_to_inventory(&mut remainder)?;
                    if !remainder.is_empty() {
                        drop_stack(out, remainder, false);
                    }
                }
            }
        }
        Ok(())
    }

    fn move_to(
        &mut self,
        stack: &mut ItemStack,
        start: usize,
        end: usize,
        reverse: bool,
        context: &RecipeContext,
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
                        self.refresh_if_grid(*slot, context)?;
                    }
                    if stack.is_empty() {
                        break;
                    }
                }
            }
        }
        if !stack.is_empty() {
            for slot in indices {
                if self.inventory.slot(slot).unwrap().is_empty() && self.may_place(slot, stack) {
                    let amount = stack.count.min(self.slot_limit(slot, stack)?);
                    let (moved, left) = take_stack(stack.clone(), amount);
                    self.inventory.set_slot_authoritative(slot, moved);
                    *stack = left;
                    self.refresh_if_grid(slot, context)?;
                    break;
                }
            }
        }
        Ok(stack.count != before)
    }
    fn quick_move_once(
        &mut self,
        slot: usize,
        _creative: bool,
        context: &RecipeContext,
        out: &mut ClickOutcome,
    ) -> Result<bool> {
        let source = self.inventory.slots[slot].clone();
        if source.is_empty() {
            return Ok(false);
        }
        let mut stack = source.clone();
        let moved = if slot == 0 {
            self.move_to(&mut stack, 10, 46, true, context)?
        } else if (10..46).contains(&slot) {
            if self.move_to(&mut stack, 1, 10, false, context)? {
                true
            } else if slot < 37 {
                self.move_to(&mut stack, 37, 46, false, context)?
            } else {
                self.move_to(&mut stack, 10, 37, false, context)?
            }
        } else {
            self.move_to(&mut stack, 10, 46, false, context)?
        };
        if !moved {
            return Ok(false);
        }
        if slot == 0 {
            // ResultSlot.onQuickCraft awards the amount actually transferred,
            // then onTake consumes one recipe and drops the output remainder.
            self.crafted_items
                .push((source.item_id, source.count - stack.count));
            self.consume_craft(context, out)?;
            if !stack.is_empty() {
                drop_stack(out, stack, false);
            }
        } else {
            self.inventory.slots[slot] = stack;
            self.refresh_if_grid(slot, context)?;
        }
        Ok(true)
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
        let order = (37..46).chain(10..37).collect::<Vec<_>>();
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
            let selected = 37 + self.inventory.selected_hotbar();
            let mut merges = vec![selected, 46];
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
        context: &RecipeContext,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        let hotbar = if button == 40 {
            46
        } else {
            37 + button as usize
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
                let taken = self.take_slot(slot, source.count, i32::MAX, context, out)?;
                self.inventory.set_slot_authoritative(hotbar, taken);
            }
        } else if source.is_empty() {
            if self.may_place(slot, &held) {
                let limit = self.slot_limit(slot, &held)?;
                let (moved, left) = take_stack(held, limit);
                self.inventory.set_slot_authoritative(slot, moved);
                self.inventory.set_slot_authoritative(hotbar, left);
                self.refresh_if_grid(slot, context)?;
            }
        } else if self.may_pickup(slot, creative)? && self.may_place(slot, &held) {
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
            self.refresh_if_grid(slot, context)?;
        }
        Ok(())
    }
    fn pickup_all(
        &mut self,
        clicked: usize,
        button: i32,
        creative: bool,
        context: &RecipeContext,
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
                if *slot == 0
                    || source.is_empty()
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
                    context,
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
        context: &RecipeContext,
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
                    if self.may_place(slot, carried)
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
                        context,
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
                    if !self.may_place(slot, &carried)
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
                    self.refresh_if_grid(slot, context)?;
                }
                self.inventory
                    .set_carried_authoritative(with_count(&carried, left));
                self.reset_quickcraft();
            }
            _ => self.reset_quickcraft(),
        }
        Ok(())
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
