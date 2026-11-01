//! GenericContainerMenu and AbstractContainerMenu click rules for ordinary
//! chest/barrel slots. The canonical player inventory is borrowed per operation;
//! only the menu cursor, drag state and remote synchronization baseline live here.
use crate::{
    container::{ClickOutcome, ClickType, ContainerClick, DroppedStack, PlayerContainer},
    inventory::{ItemStack, PlayerInventory},
    protocol::{Packet, Writer},
};
use anyhow::{ensure, Result};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct GenericContainerMenu {
    pub window_id: u8,
    pub rows: usize,
    state_id: i32,
    carried: ItemStack,
    remote: Vec<ItemStack>,
    remote_carried: ItemStack,
    drag_status: u8,
    drag_type: u8,
    drag_slots: BTreeSet<usize>,
}
fn canonical(s: &ItemStack) -> ItemStack {
    if s.is_empty() {
        ItemStack::empty()
    } else {
        s.clone()
    }
}
fn counted(s: &ItemStack, n: i32) -> ItemStack {
    if n <= 0 || s.is_empty() {
        ItemStack::empty()
    } else {
        let mut s = s.clone();
        s.count = n;
        s
    }
}
fn matches(a: &ItemStack, b: &ItemStack) -> Result<bool> {
    Ok(
        (if a.is_empty() { 0 } else { a.count }) == (if b.is_empty() { 0 } else { b.count })
            && a.same_item_same_components(b)?,
    )
}
fn limit(s: &ItemStack) -> Result<i32> {
    Ok(s.max_stack_size()?.min(64))
}
fn drop_stack(out: &mut ClickOutcome, s: ItemStack, trace: bool) {
    if !s.is_empty() {
        out.drops.push(DroppedStack {
            stack: s,
            around: false,
            trace_item: trace,
        });
    }
}
fn move_to(
    slots: &mut [ItemStack],
    stack: &mut ItemStack,
    start: usize,
    end: usize,
    reverse: bool,
) -> Result<bool> {
    let before = stack.count;
    let order = if reverse {
        (start..end).rev().collect::<Vec<_>>()
    } else {
        (start..end).collect()
    };
    if stack.is_stackable()? {
        for &i in &order {
            let old = &slots[i];
            if !old.is_empty() && old.same_item_same_components(stack)? {
                let n = stack.count.min((limit(stack)? - old.count).max(0));
                if n > 0 {
                    slots[i] = counted(old, old.count + n);
                    *stack = counted(stack, stack.count - n);
                }
                if stack.is_empty() {
                    break;
                }
            }
        }
    }
    if !stack.is_empty() {
        for i in order {
            if slots[i].is_empty() {
                let n = stack.count.min(limit(stack)?);
                slots[i] = counted(stack, n);
                *stack = counted(stack, stack.count - n);
                break;
            }
        }
    }
    Ok(before != stack.count)
}
impl GenericContainerMenu {
    pub fn new(window_id: u8, rows: usize) -> Result<Self> {
        ensure!(
            (1..=100).contains(&window_id) && matches!(rows, 3 | 6),
            "unsupported native generic menu"
        );
        Ok(Self {
            window_id,
            rows,
            state_id: 0,
            carried: ItemStack::empty(),
            remote: vec![ItemStack::empty(); rows * 9 + 36],
            remote_carried: ItemStack::empty(),
            drag_status: 0,
            drag_type: 0,
            drag_slots: BTreeSet::new(),
        })
    }
    fn next_state(&mut self) -> i32 {
        self.state_id = (self.state_id + 1) & 32767;
        self.state_id
    }
    fn slots(&self, top: &[ItemStack], inventory: &PlayerInventory) -> Result<Vec<ItemStack>> {
        ensure!(top.len() == self.rows * 9, "generic container size changed");
        let mut s = top.iter().map(canonical).collect::<Vec<_>>();
        s.extend((9..45).map(|i| canonical(inventory.slot(i).expect("canonical inventory slot"))));
        Ok(s)
    }
    pub fn inventory_snapshot(&self, inventory: &PlayerInventory) -> PlayerInventory {
        let mut i = inventory.clone();
        i.set_carried_authoritative(self.carried.clone());
        i
    }
    pub fn full_sync(&mut self, top: &[ItemStack], inventory: &PlayerInventory) -> Result<Packet> {
        let slots = self.slots(top, inventory)?;
        self.remote = slots.clone();
        self.remote_carried = canonical(&self.carried);
        let state = self.next_state();
        let mut w = Writer::new();
        w.u8(self.window_id)
            .varint(state)
            .varint(slots.len() as i32);
        for s in &slots {
            w.raw(&s.encode_canonical()?);
        }
        w.raw(&self.carried.encode_canonical()?);
        Ok(Packet {
            id: 0x13,
            payload: w.into_inner(),
        })
    }
    pub fn broadcast_changes(
        &mut self,
        top: &[ItemStack],
        inventory: &PlayerInventory,
    ) -> Result<Vec<Packet>> {
        let slots = self.slots(top, inventory)?;
        let mut out = Vec::new();
        for (i, s) in slots.iter().enumerate() {
            if !matches(s, &self.remote[i])? {
                self.remote[i] = canonical(s);
                let state = self.next_state();
                let mut w = Writer::new();
                w.u8(self.window_id)
                    .varint(state)
                    .i16(i as i16)
                    .raw(&s.encode_canonical()?);
                out.push(Packet {
                    id: 0x15,
                    payload: w.into_inner(),
                });
            }
        }
        if !matches(&self.carried, &self.remote_carried)? {
            self.remote_carried = canonical(&self.carried);
            let state = self.next_state();
            let mut w = Writer::new();
            w.i8(-1)
                .varint(state)
                .i16(-1)
                .raw(&self.carried.encode_canonical()?);
            out.push(Packet {
                id: 0x15,
                payload: w.into_inner(),
            });
        }
        Ok(out)
    }
    fn reset_drag(&mut self) {
        self.drag_status = 0;
        self.drag_slots.clear();
    }
    pub fn click(
        &mut self,
        click: &ContainerClick,
        top: &mut [ItemStack],
        inventory: &mut PlayerInventory,
        creative: bool,
    ) -> Result<ClickOutcome> {
        let mut out = ClickOutcome::default();
        if click.window_id != self.window_id || click.slot >= self.remote.len() as i16 {
            return Ok(out);
        }
        let stale = click.state_id != self.state_id;
        let mut slots = self.slots(top, inventory)?;
        self.do_click(
            click.slot as i32,
            click.button as i32,
            click.kind,
            &mut slots,
            inventory,
            creative,
            &mut out,
        )?;
        let n = top.len();
        top.clone_from_slice(&slots[..n]);
        for (i, s) in slots[n..].iter().enumerate() {
            inventory.set_slot_authoritative(i + 9, canonical(s));
        }
        for (&i, s) in &click.changed_slots {
            if i >= 0 && (i as usize) < self.remote.len() {
                self.remote[i as usize] = canonical(s);
            }
        }
        self.remote_carried = canonical(&click.carried);
        if stale {
            out.packets.push(self.full_sync(top, inventory)?);
        } else {
            out.packets.extend(self.broadcast_changes(top, inventory)?);
        }
        Ok(out)
    }
    fn pickup(
        &mut self,
        slot: i32,
        button: i32,
        slots: &mut [ItemStack],
        out: &mut ClickOutcome,
    ) -> Result<()> {
        if slot == -999 {
            let n = if button == 0 { self.carried.count } else { 1 };
            drop_stack(out, counted(&self.carried, n.min(self.carried.count)), true);
            self.carried = counted(&self.carried, self.carried.count - n);
            return Ok(());
        }
        if slot < 0 {
            return Ok(());
        }
        let at = slot as usize;
        let s = slots[at].clone();
        let c = self.carried.clone();
        if c.is_empty() {
            let n = if button == 0 {
                s.count
            } else {
                (s.count + 1) / 2
            };
            self.carried = counted(&s, n);
            slots[at] = counted(&s, s.count - n);
        } else if s.is_empty() {
            let n = (if button == 0 { c.count } else { 1 }).min(limit(&c)?);
            slots[at] = counted(&c, n);
            self.carried = counted(&c, c.count - n);
        } else if s.same_item_same_components(&c)? {
            let n = (if button == 0 { c.count } else { 1 }).min((limit(&c)? - s.count).max(0));
            slots[at] = counted(&s, s.count + n);
            self.carried = counted(&c, c.count - n);
        } else if c.count <= limit(&c)? {
            slots[at] = c;
            self.carried = s;
        }
        Ok(())
    }
    fn do_click(
        &mut self,
        slot: i32,
        button: i32,
        kind: ClickType,
        slots: &mut [ItemStack],
        inventory: &mut PlayerInventory,
        creative: bool,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        if kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, slots, inventory, creative, out);
        }
        if self.drag_status != 0 {
            self.reset_drag();
            return Ok(());
        }
        match kind {
            ClickType::Pickup if button == 0 || button == 1 => {
                self.pickup(slot, button, slots, out)?
            }
            ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    self.pickup(slot, button, slots, out)?;
                } else if slot >= 0 {
                    let i = slot as usize;
                    let top = self.rows * 9;
                    for _ in 0..4096 {
                        let mut s = slots[i].clone();
                        if s.is_empty() {
                            break;
                        }
                        let moved = if i < top {
                            move_to(slots, &mut s, top, top + 36, true)?
                        } else {
                            move_to(slots, &mut s, 0, top, false)?
                        };
                        if !moved {
                            break;
                        }
                        slots[i] = s;
                    }
                }
            }
            ClickType::Swap if slot >= 0 && ((0..9).contains(&button) || button == 40) => {
                let i = slot as usize;
                let j = if button == 40 {
                    None
                } else {
                    Some(self.rows * 9 + 27 + button as usize)
                };
                if j == Some(i) {
                    return Ok(());
                }
                let source = slots[i].clone();
                let held = if let Some(j) = j {
                    slots[j].clone()
                } else {
                    inventory.slot(45).unwrap().clone()
                };
                let mut remainder = ItemStack::empty();
                if held.is_empty() {
                    slots[i] = ItemStack::empty();
                } else if held.count > limit(&held)? {
                    slots[i] = counted(&held, limit(&held)?);
                    remainder = counted(&held, held.count - limit(&held)?);
                } else {
                    slots[i] = held.clone();
                }
                if held.count > limit(&held)? && !held.is_empty() {
                    if let Some(j) = j {
                        slots[j] = remainder;
                    } else {
                        inventory.set_slot_authoritative(45, remainder);
                    }
                    let mut rest = source;
                    Self::return_to_inventory_slots(slots, self.rows * 9, inventory, &mut rest)?;
                    drop_stack(out, rest, true);
                } else if let Some(j) = j {
                    slots[j] = source;
                } else {
                    inventory.set_slot_authoritative(45, source);
                }
            }
            ClickType::Clone if creative && slot >= 0 && self.carried.is_empty() => {
                let s = &slots[slot as usize];
                self.carried = counted(s, s.max_stack_size()?);
            }
            ClickType::Throw
                if slot >= 0 && self.carried.is_empty() && (button == 0 || button == 1) =>
            {
                let i = slot as usize;
                let n = if button == 0 { 1 } else { slots[i].count };
                drop_stack(out, counted(&slots[i], n), true);
                slots[i] = counted(&slots[i], slots[i].count - n);
            }
            ClickType::PickupAll if slot >= 0 && (button == 0 || button == 1) => {
                if !self.carried.is_empty() && slots[slot as usize].is_empty() {
                    let order = if button == 0 {
                        (0..slots.len()).collect::<Vec<_>>()
                    } else {
                        (0..slots.len()).rev().collect()
                    };
                    for pass in 0..2 {
                        for &i in &order {
                            let s = slots[i].clone();
                            if s.is_empty()
                                || !s.same_item_same_components(&self.carried)?
                                || pass == 0 && s.count == s.max_stack_size()?
                            {
                                continue;
                            }
                            let n = s
                                .count
                                .min((self.carried.max_stack_size()? - self.carried.count).max(0));
                            self.carried = counted(&self.carried, self.carried.count + n);
                            slots[i] = counted(&s, s.count - n);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn return_to_inventory_slots(
        slots: &mut [ItemStack],
        top: usize,
        inventory: &mut PlayerInventory,
        stack: &mut ItemStack,
    ) -> Result<()> {
        let mut menu = PlayerContainer::from_inventory(inventory.clone());
        for i in 0..36 {
            menu.inventory
                .set_slot_authoritative(i + 9, slots[top + i].clone());
        }
        menu.add_pickup_to_inventory(stack, false)?;
        for i in 0..36 {
            slots[top + i] = menu.inventory.slot(i + 9).unwrap().clone();
        }
        inventory.set_slot_authoritative(45, menu.inventory.slot(45).unwrap().clone());
        Ok(())
    }
    fn quickcraft(
        &mut self,
        slot: i32,
        button: i32,
        slots: &mut [ItemStack],
        inventory: &mut PlayerInventory,
        creative: bool,
        out: &mut ClickOutcome,
    ) -> Result<()> {
        let previous = self.drag_status;
        self.drag_status = (button & 3) as u8;
        if !(previous == 1 && self.drag_status == 2) && previous != self.drag_status {
            self.reset_drag();
            return Ok(());
        }
        if self.carried.is_empty() {
            self.reset_drag();
            return Ok(());
        }
        match self.drag_status {
            0 => {
                self.drag_type = ((button >> 2) & 3) as u8;
                if self.drag_type <= 1 || self.drag_type == 2 && creative {
                    self.drag_status = 1;
                    self.drag_slots.clear();
                } else {
                    self.reset_drag();
                }
            }
            1 if slot >= 0 => {
                let i = slot as usize;
                let s = &slots[i];
                if (s.is_empty() || s.same_item_same_components(&self.carried)?)
                    && (self.drag_type == 2 || self.carried.count > self.drag_slots.len() as i32)
                    && s.count < limit(&self.carried)?
                {
                    self.drag_slots.insert(i);
                }
            }
            2 => {
                if self.drag_slots.len() == 1 {
                    let i = *self.drag_slots.iter().next().unwrap();
                    let kind = self.drag_type;
                    self.reset_drag();
                    return self.do_click(
                        i as i32,
                        kind as i32,
                        ClickType::Pickup,
                        slots,
                        inventory,
                        creative,
                        out,
                    );
                }
                let c = self.carried.clone();
                let each = match self.drag_type {
                    0 => c.count / (self.drag_slots.len().max(1) as i32),
                    1 => 1,
                    2 => c.max_stack_size()?,
                    _ => 0,
                };
                let mut left = c.count;
                for &i in &self.drag_slots {
                    let s = &slots[i];
                    if !s.is_empty() && !s.same_item_same_components(&c)?
                        || self.drag_type != 2 && c.count < self.drag_slots.len() as i32
                    {
                        continue;
                    }
                    let n = (each + s.count).min(limit(&c)?);
                    left -= n - s.count;
                    slots[i] = counted(&c, n);
                }
                self.carried = counted(&c, left);
                self.reset_drag();
            }
            _ => self.reset_drag(),
        }
        Ok(())
    }
    /// Same Inventory.placeItemBackInInventory owner as InventoryMenu.close.
    pub fn close(&mut self, inventory: &mut PlayerInventory) -> Result<ClickOutcome> {
        let mut c = std::mem::take(&mut self.carried);
        let mut menu = PlayerContainer::from_inventory(inventory.clone());
        menu.add_pickup_to_inventory(&mut c, false)?;
        *inventory = menu.inventory;
        self.reset_drag();
        let mut out = ClickOutcome::default();
        drop_stack(&mut out, c, false);
        Ok(out)
    }
}
