//! Native player-window inventory actions. Menu synchronization follows the
//! server's authoritative state and the client's predicted slot map.
//!
//! Window 0 only. Block menus, recipe-book actions, bundles enabled by custom
//! feature flags, advancement/stat side effects and item entities are separate.
use crate::{
    crafting::{CraftResult, RecipeBook, RecipeContext, RecipeInput},
    inventory::{self, ItemStack, PlayerInventory, INVENTORY_SLOTS},
    protocol::{Packet, Reader, Writer},
};
use anyhow::{bail, ensure, Result};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickType {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    QuickCraft,
    PickupAll,
}
impl TryFrom<i32> for ClickType {
    type Error = anyhow::Error;
    fn try_from(value: i32) -> Result<Self> {
        Ok(match value {
            0 => Self::Pickup,
            1 => Self::QuickMove,
            2 => Self::Swap,
            3 => Self::Clone,
            4 => Self::Throw,
            5 => Self::QuickCraft,
            6 => Self::PickupAll,
            _ => bail!("invalid container click type"),
        })
    }
}
#[derive(Clone, Debug)]
pub struct ContainerClick {
    pub window_id: u8,
    pub state_id: i32,
    pub slot: i16,
    pub button: i8,
    pub kind: ClickType,
    pub changed_slots: BTreeMap<i16, ItemStack>,
    pub carried: ItemStack,
}
impl ContainerClick {
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let mut r = Reader::new(payload);
        let window_id = r.u8()?;
        let state_id = r.varint()?;
        let slot = r.i16()?;
        let button = r.i8()?;
        let kind = ClickType::try_from(r.varint()?)?;
        let count = r.varint()?;
        ensure!(
            (0..=128).contains(&count),
            "container prediction slot count exceeds 128"
        );
        let mut changed_slots = BTreeMap::new();
        for _ in 0..count {
            let index = r.i16()?;
            let (stack, used) = ItemStack::decode(r.remaining())?;
            r.bytes(used)?;
            changed_slots.insert(index, stack);
        }
        let (carried, used) = ItemStack::decode(r.remaining())?;
        r.bytes(used)?;
        r.finish()?;
        Ok(Self {
            window_id,
            state_id,
            slot,
            button,
            kind,
            changed_slots,
            carried,
        })
    }
}
#[derive(Debug, Default)]
pub struct ClickOutcome {
    pub packets: Vec<Packet>,
    pub drops: Vec<DroppedStack>,
    pub crafted: Vec<String>,
}

/// Original Player.drop arguments retained until the host creates the entity.
/// Craft/close overflow and deliberate throwing have different trace/stat rules.
#[derive(Clone, Debug)]
pub struct DroppedStack {
    pub stack: ItemStack,
    pub around: bool,
    pub trace_item: bool,
}

/// Immutable observation of the local listener boundary, independent of client
/// prediction. Replay of mutating/reentrant listeners requires a direct host;
/// the bounded empty-criteria runtime consumes these observations before send.
#[derive(Clone, Debug)]
pub struct LocalSlotChange {
    pub menu_slot: usize,
    pub inventory_slot: Option<usize>,
    pub stack: ItemStack,
    /// Number of packets preceding the callback in this sync output vector.
    pub packet_offset: usize,
}
impl std::ops::Deref for DroppedStack {
    type Target = ItemStack;
    fn deref(&self) -> &ItemStack {
        &self.stack
    }
}
impl ClickOutcome {
    fn drop_stack(&mut self, stack: ItemStack, trace_item: bool) {
        // Every menu call uses Player.drop(ItemStack, traceItem), whose
        // original wrapper forwards around=false to the three-argument method.
        self.drops.push(DroppedStack {
            stack,
            around: false,
            trace_item,
        });
    }
}

#[derive(Clone, Debug)]
pub struct PlayerContainer {
    pub inventory: PlayerInventory,
    local_last_slots: Vec<ItemStack>,
    local_slot_listener: bool,
    local_slot_changes: Vec<LocalSlotChange>,
    remote_slots: Vec<ItemStack>,
    remote_carried: ItemStack,
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
    crafting: Option<(String, CraftResult)>,
    crafting_enabled: bool,
    creative_mode: bool,
}
impl Default for PlayerContainer {
    fn default() -> Self {
        Self::new()
    }
}
impl PlayerContainer {
    /// Mutate the authoritative held stack; the actual menu phase owns sync.
    pub fn set_hand_stack(&mut self, hand: i32, stack: ItemStack) -> Result<()> {
        let slot = match hand {
            0 => 36 + self.inventory.selected_hotbar(),
            1 => 45,
            _ => anyhow::bail!("invalid interaction hand"),
        };
        self.inventory.set_slot_authoritative(slot, stack);
        Ok(())
    }
    pub fn new() -> Self {
        Self::from_inventory(PlayerInventory::new())
    }
    pub fn from_inventory(inventory: PlayerInventory) -> Self {
        Self {
            local_last_slots: vec![ItemStack::empty(); INVENTORY_SLOTS],
            local_slot_listener: false,
            local_slot_changes: Vec::new(),
            remote_slots: inventory.slots().iter().map(copy_stack).collect(),
            remote_carried: copy_stack(inventory.carried()),
            inventory,
            quickcraft_status: 0,
            quickcraft_type: 0,
            quickcraft_slots: BTreeSet::new(),
            crafting: None,
            crafting_enabled: true,
            creative_mode: false,
        }
    }
    /// Call when an external creative-slot update has already been sent.
    pub fn record_external_sync(&mut self) {
        self.remote_slots = self.inventory.slots().iter().map(copy_stack).collect();
        self.remote_carried = copy_stack(self.inventory.carried());
    }
    /// A single externally synchronized slot must not hide unrelated changes.
    pub fn record_external_slot_sync(&mut self, slot: usize) -> Result<()> {
        ensure!(slot < INVENTORY_SLOTS, "invalid remote slot");
        self.remote_slots[slot] = copy_stack(self.inventory.slot(slot).unwrap());
        Ok(())
    }
    /// Bind the single ServerPlayer-style local observer. Like addSlotListener,
    /// first registration broadcasts changes; repeated registration is a no-op.
    pub fn enable_local_slot_events(&mut self) -> Result<Vec<Packet>> {
        if self.local_slot_listener {
            return Ok(vec![]);
        }
        self.local_slot_listener = true;
        self.broadcast_changes()
    }
    pub fn take_local_slot_changes(&mut self) -> Vec<LocalSlotChange> {
        std::mem::take(&mut self.local_slot_changes)
    }
    pub fn local_slot_baseline(&self) -> &[ItemStack] {
        &self.local_last_slots
    }
    /// sendAllDataToRemote: updates remote state without local callbacks.
    pub fn full_sync(&mut self) -> Packet {
        self.inventory.increment_state_id();
        self.record_external_sync();
        Packet {
            id: 0x13,
            payload: self.inventory.window_items_payload(),
        }
    }
    /// broadcastFullState first visits local slot listeners, then sends Initial.
    pub fn broadcast_full_state(&mut self) -> Result<Packet> {
        self.broadcast_full_state_at(0)
    }
    fn broadcast_full_state_at(&mut self, packet_offset: usize) -> Result<Packet> {
        for slot in 0..INVENTORY_SLOTS {
            self.record_local_change(slot, packet_offset)?;
        }
        Ok(self.full_sync())
    }
    pub fn swap_hands(&mut self) -> Result<Vec<Packet>> {
        let selected = 36 + self.inventory.selected_hotbar();
        let main = self.inventory.slots()[selected].clone();
        let off = self.inventory.slots()[45].clone();
        self.inventory.set_slot_authoritative(selected, off);
        self.inventory.set_slot_authoritative(45, main);
        let mut packets = Vec::new();
        self.synchronize(&mut packets)?;
        Ok(packets)
    }
    /// ServerPlayer.drop(boolean): update the predicted selected-slot cache
    /// before the host calls Player.drop(stack, around=false, traceItem=true).
    /// Q/CtrlQ already predict this removal, so this operation sends no packet.
    pub fn drop_selected(&mut self, all: bool) -> Result<ClickOutcome> {
        let slot = 36 + self.inventory.selected_hotbar();
        let source = self.inventory.slots()[slot].clone();
        let mut outcome = ClickOutcome::default();
        if !source.is_empty() {
            let count = if all { source.count } else { 1 };
            let (dropped, remaining) = take_stack(source, count);
            self.inventory.set_slot_authoritative(slot, remaining);
            outcome.drop_stack(dropped, true);
        }
        // InventoryMenu maps the selected inventory index to slot36..44.
        // setRemoteSlot copies even an empty selected stack before drop().
        self.remote_slots[slot] = copy_stack(&self.inventory.slots()[slot]);
        Ok(outcome)
    }

    /// Inventory.add used by ItemEntity.playerTouch, followed by authoritative
    /// window0 synchronization. The supplied stack retains any survival overflow.
    /// Creative mode also accepts and destroys overflow when no slot remains.
    /// Java moves a damaged stack whole into its first free slot; returning
    /// crafting inputs on menu close deliberately uses a different split path.
    pub fn accept_pickup(
        &mut self,
        stack: &mut ItemStack,
        creative: bool,
    ) -> Result<(bool, Vec<Packet>)> {
        if stack.is_empty() {
            return Ok((false, vec![]));
        }
        let accepted = self.add_pickup_to_inventory(stack, creative)?;
        let packets = self.broadcast_changes()?;
        Ok((accepted, packets))
    }
    /// The Inventory.add stage alone. Keep the remote menu baseline unchanged:
    /// ItemEntity calls ServerPlayer.take/broadcastChanges only on success.
    /// A partial survival transfer can mutate inventory while returning false.
    pub fn add_pickup_to_inventory(
        &mut self,
        stack: &mut ItemStack,
        creative: bool,
    ) -> Result<bool> {
        if stack.is_empty() {
            return Ok(false);
        }
        // Prepare both sides before committing so a component-codec failure
        // cannot consume an item without updating the player's inventory.
        let mut staged = self.clone();
        let mut remaining = stack.clone();
        let previous_creative = staged.creative_mode;
        staged.creative_mode = creative;
        staged.add_to_inventory_inner(&mut remaining, true)?;
        staged.creative_mode = previous_creative;
        // Inventory.add compares the final loop iteration's count. A survival
        // inventory that filled up partway through the stack returns false,
        // although the transferred items and reduced remainder are committed.
        let accepted = remaining.is_empty();
        if remaining.is_empty() {
            // Inventory.add shrinks the same Java object. Its item and patch
            // survive count==0; ItemEntity.playerTouch later restores the old
            // count on the discarded object. Replacing it with AIR loses that
            // observable state even though both empty stacks encode as zero.
            remaining = stack.clone();
            remaining.count = 0;
        }
        *self = staged;
        *stack = remaining;
        Ok(accepted)
    }
    /// AbstractContainerMenu.broadcastChanges at the host's actual call site.
    pub fn broadcast_changes(&mut self) -> Result<Vec<Packet>> {
        let mut packets = Vec::new();
        self.synchronize(&mut packets)?;
        Ok(packets)
    }
    pub fn click(
        &mut self,
        click: &ContainerClick,
        creative: bool,
        context: &RecipeContext,
    ) -> Result<ClickOutcome> {
        let mut outcome = ClickOutcome::default();
        if click.window_id != 0 {
            return Ok(outcome);
        }
        // ServerGamePacketListener ignores out-of-range positive slots before
        // applying client predictions. Negative pickup sentinels are no-ops.
        if click.slot >= INVENTORY_SLOTS as i16 {
            return Ok(outcome);
        }
        let stale = click.state_id != self.inventory.state_id();
        self.refresh_crafting(context)?;
        self.do_click(
            click.slot as i32,
            click.button as i32,
            click.kind,
            creative,
            context,
            &mut outcome,
        )?;
        // Client predictions become the remote baseline, not authoritative items.
        for (slot, stack) in &click.changed_slots {
            if (0..INVENTORY_SLOTS as i16).contains(slot) {
                self.remote_slots[*slot as usize] = copy_stack(stack);
            }
        }
        self.remote_carried = copy_stack(&click.carried);
        if stale {
            let packet = self.broadcast_full_state_at(outcome.packets.len())?;
            outcome.packets.push(packet);
        } else {
            self.synchronize(&mut outcome.packets)?;
        }
        Ok(outcome)
    }
    /// Close window0 for a living connected player. Carried items return first,
    /// then the four crafting inputs; unplaceable items are returned as drops.
    pub fn close(&mut self) -> Result<ClickOutcome> {
        let mut out = ClickOutcome::default();
        self.reset_quickcraft();
        let creative = self.creative_mode;
        self.creative_mode = false;
        let mut carried = self.inventory.carried().clone();
        self.inventory.set_carried_authoritative(ItemStack::empty());
        self.add_to_inventory(&mut carried)?;
        if !carried.is_empty() {
            out.drop_stack(carried, false);
        }
        self.inventory.set_slot_authoritative(0, ItemStack::empty());
        self.crafting = None;
        for slot in 1..5 {
            let mut item = self.inventory.slot(slot).unwrap().clone();
            self.inventory
                .set_slot_authoritative(slot, ItemStack::empty());
            self.add_to_inventory(&mut item)?;
            if !item.is_empty() {
                out.drop_stack(item, false);
            }
        }
        self.creative_mode = creative;
        self.synchronize(&mut out.packets)?;
        Ok(out)
    }
    /// Recalculate and send only changed slots after an external grid edit.
    pub fn crafting_changed(&mut self, context: &RecipeContext) -> Result<Vec<Packet>> {
        self.refresh_crafting(context)?;
        let mut packets = Vec::new();
        self.synchronize(&mut packets)?;
        Ok(packets)
    }
    pub fn refresh_crafting(&mut self, context: &RecipeContext) -> Result<()> {
        if !self.crafting_enabled {
            return Ok(());
        }
        let input = RecipeInput::crafting(2, 2, self.inventory.slots()[1..5].to_vec())?;
        let book = RecipeBook::vanilla();
        let mut matches = None;
        for id in book.recipe_ids() {
            if let Some(result) = book.evaluate_with_context(id, &input, context)? {
                if result.stack.is_empty() {
                    continue;
                }
                if let Some((previous, _)) = &matches {
                    bail!("ambiguous recipe-manager order: {previous} and {id}");
                }
                matches = Some((id.to_owned(), result));
            }
        }
        self.inventory.set_slot_authoritative(
            0,
            matches
                .as_ref()
                .map(|(_, r)| r.stack.clone())
                .unwrap_or_default(),
        );
        self.crafting = matches;
        Ok(())
    }
    fn synchronize(&mut self, packets: &mut Vec<Packet>) -> Result<()> {
        for slot in 0..INVENTORY_SLOTS {
            self.record_local_change(slot, packets.len())?;
            if !stack_matches(self.inventory.slot(slot).unwrap(), &self.remote_slots[slot])? {
                self.remote_slots[slot] = copy_stack(self.inventory.slot(slot).unwrap());
                self.inventory.increment_state_id();
                packets.push(Packet {
                    id: 0x15,
                    payload: self.inventory.set_slot_payload(slot)?,
                });
            }
        }
        if !stack_matches(self.inventory.carried(), &self.remote_carried)? {
            self.remote_carried = copy_stack(self.inventory.carried());
            let state = self.inventory.increment_state_id();
            let mut w = Writer::new();
            w.i8(-1)
                .varint(state)
                .i16(-1)
                .raw(&self.remote_carried.encode());
            packets.push(Packet {
                id: 0x15,
                payload: w.into_inner(),
            });
        }
        Ok(())
    }
    fn record_local_change(&mut self, slot: usize, packet_offset: usize) -> Result<()> {
        let current = self.inventory.slot(slot).expect("player menu slot");
        if !stack_matches(current, &self.local_last_slots[slot])? {
            let stack = copy_stack(current);
            self.local_last_slots[slot] = stack.clone();
            if self.local_slot_listener {
                let inventory_slot = match slot {
                    5..=8 => Some(44 - slot),
                    9..=35 => Some(slot),
                    36..=44 => Some(slot - 36),
                    45 => Some(40),
                    _ => None,
                };
                self.local_slot_changes.push(LocalSlotChange {
                    menu_slot: slot,
                    inventory_slot,
                    stack,
                    packet_offset,
                });
            }
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
                        out.drop_stack(dropped, true);
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
                        out.drop_stack(stack, true);
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
    fn may_place(&self, slot: usize, stack: &ItemStack) -> bool {
        if slot == 0 {
            return false;
        }
        if (5..9).contains(&slot) {
            equipment_slot(stack) == Some(slot)
        } else {
            true
        }
    }
    fn may_pickup(&self, slot: usize, creative: bool) -> Result<bool> {
        let stack = self.inventory.slot(slot).unwrap();
        if creative || stack.is_empty() || !(5..9).contains(&slot) {
            return Ok(true);
        }
        let value = stack.component_value(9)?;
        Ok(!value
            .and_then(|v| v["enchantments"].as_array().cloned())
            .is_some_and(|a| {
                a.iter()
                    .any(|e| e["id"].as_i64() == Some(2) && e["level"].as_i64().unwrap_or(0) > 0)
            }))
    }
    fn slot_limit(&self, slot: usize, stack: &ItemStack) -> Result<i32> {
        Ok(stack
            .max_stack_size()?
            .min(if (5..9).contains(&slot) { 1 } else { 99 }))
    }
    fn refresh_if_grid(&mut self, slot: usize, context: &RecipeContext) -> Result<()> {
        if (1..5).contains(&slot) {
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
        for i in 0..4 {
            let slot = i + 1;
            let source = self.inventory.slot(slot).unwrap().clone();
            let mut left = with_count(&source, source.count - 1);
            let mut remainder = result.remaining[i].clone();
            if !remainder.is_empty() {
                if left.is_empty() {
                    left = remainder;
                } else if left.same_item_same_components(&remainder)? {
                    remainder.count += left.count;
                    left = remainder;
                } else {
                    let mut item = remainder;
                    self.add_to_inventory(&mut item)?;
                    if !item.is_empty() {
                        out.drop_stack(item, false);
                    }
                }
            }
            self.inventory.set_slot_authoritative(slot, left);
        }
        self.refresh_crafting(context)
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
                if self.inventory.slot(slot).unwrap().is_empty() && self.may_place(slot, stack) {
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
    fn quick_move_once(
        &mut self,
        slot: usize,
        creative: bool,
        context: &RecipeContext,
        out: &mut ClickOutcome,
    ) -> Result<bool> {
        if !self.may_pickup(slot, creative)? {
            return Ok(false);
        }
        let source = self.inventory.slot(slot).unwrap().clone();
        if source.is_empty() {
            return Ok(false);
        }
        let mut stack = source.clone();
        let moved = if slot == 0 {
            self.move_to(&mut stack, 9, 45, true)?
        } else if (1..9).contains(&slot) {
            self.move_to(&mut stack, 9, 45, false)?
        } else if let Some(armor) =
            equipment_slot(&stack).filter(|i| self.inventory.slot(*i).unwrap().is_empty())
        {
            self.move_to(&mut stack, armor, armor + 1, false)?
        } else if inventory::item_name(stack.item_id) == Some("shield")
            && self.inventory.slot(45).unwrap().is_empty()
        {
            self.move_to(&mut stack, 45, 46, false)?
        } else if (9..36).contains(&slot) {
            self.move_to(&mut stack, 36, 45, false)?
        } else if (36..45).contains(&slot) {
            self.move_to(&mut stack, 9, 36, false)?
        } else {
            self.move_to(&mut stack, 9, 45, false)?
        };
        if !moved {
            return Ok(false);
        }
        if slot == 0 {
            self.consume_craft(context, out)?;
            if !stack.is_empty() {
                out.drop_stack(stack, false);
            }
        } else {
            self.inventory.set_slot_authoritative(slot, stack);
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
        let order = (36..45).chain(9..36).collect::<Vec<_>>();
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
            let selected = 36 + self.inventory.selected_hotbar();
            let mut merges = vec![selected, 45];
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
            45
        } else {
            36 + button as usize
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
                    out.drop_stack(source, true);
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
            (0..INVENTORY_SLOTS).collect::<Vec<_>>()
        } else {
            (0..INVENTORY_SLOTS).rev().collect::<Vec<_>>()
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
                }
                self.inventory
                    .set_carried_authoritative(with_count(&carried, left));
                self.reset_quickcraft();
                self.refresh_crafting(context)?;
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
fn equipment_slot(stack: &ItemStack) -> Option<usize> {
    static DATA: std::sync::OnceLock<BTreeMap<i32, String>> = std::sync::OnceLock::new();
    let data = DATA.get_or_init(|| {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../tools/container_oracle_equipment.json"))
                .expect("actual Java equipment slots");
        value
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, row)| {
                (
                    id.parse().unwrap(),
                    row["equipment_slot"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    });
    match data.get(&stack.item_id)?.as_str() {
        "HEAD" => Some(5),
        "CHEST" => Some(6),
        "LEGS" => Some(7),
        "FEET" => Some(8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("../tools/selected_drop_tests.rs");
    include!("../tools/container_local_listener_tests.rs");
    fn check_drop_arguments(actual: &[DroppedStack], expected: &serde_json::Value) {
        let flags = expected["drop_flags"]
            .as_array()
            .expect("original Java drop arguments");
        assert_eq!(actual.len(), flags.len());
        for (drop, flags) in actual.iter().zip(flags) {
            assert_eq!(drop.around, flags[0].as_bool().unwrap());
            assert_eq!(drop.trace_item, flags[1].as_bool().unwrap());
        }
    }
    fn stack(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn request(menu: &PlayerContainer, slot: i16, button: i8, kind: ClickType) -> ContainerClick {
        ContainerClick {
            window_id: 0,
            state_id: menu.inventory.state_id(),
            slot,
            button,
            kind,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        }
    }
    fn click(
        menu: &mut PlayerContainer,
        slot: i16,
        button: i8,
        kind: ClickType,
        creative: bool,
    ) -> ClickOutcome {
        menu.click(
            &request(menu, slot, button, kind),
            creative,
            &RecipeContext::default(),
        )
        .unwrap()
    }
    #[test]
    fn actual_java_inventory_menu_matches_all_click_modes() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../tools/container_vectors.json")).unwrap();
        let stacks = data["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                ItemStack::decode_exact(&STANDARD.decode(v.as_str().unwrap()).unwrap()).unwrap()
            })
            .collect::<Vec<_>>();
        let same = |actual: &ItemStack,
                    index: &serde_json::Value,
                    case: &serde_json::Value,
                    step: usize,
                    slot: &str| {
            let expected = &stacks[index.as_u64().unwrap() as usize];
            assert!(
                stack_matches(actual, expected).unwrap(),
                "{case},step{step},{slot}: actual{actual:?},expected{expected:?}"
            );
        };
        let mut checked = 0;
        for case in data["cases"].as_array().unwrap() {
            let mut menu = PlayerContainer::new();
            menu.crafting_enabled = false;
            for (i, v) in case["slots"].as_array().unwrap().iter().enumerate() {
                menu.inventory
                    .set_slot_authoritative(i, stacks[v.as_u64().unwrap() as usize].clone());
            }
            menu.inventory.set_carried_authoritative(
                stacks[case["carried"].as_u64().unwrap() as usize].clone(),
            );
            let mut out = ClickOutcome::default();
            for (step, (click, expected)) in case["clicks"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["states"].as_array().unwrap())
                .enumerate()
            {
                let kind = match click["type"].as_str().unwrap() {
                    "PICKUP" => ClickType::Pickup,
                    "QUICK_MOVE" => ClickType::QuickMove,
                    "SWAP" => ClickType::Swap,
                    "CLONE" => ClickType::Clone,
                    "THROW" => ClickType::Throw,
                    "QUICK_CRAFT" => ClickType::QuickCraft,
                    "PICKUP_ALL" => ClickType::PickupAll,
                    _ => panic!("unknown oracle click"),
                };
                menu.do_click(
                    click["slot"].as_i64().unwrap() as i32,
                    click["button"].as_i64().unwrap() as i32,
                    kind,
                    case["creative"].as_bool().unwrap(),
                    &RecipeContext::default(),
                    &mut out,
                )
                .unwrap_or_else(|e| panic!("{},step{step}: {e:#}", case["name"]));
                for (i, e) in expected["slots"].as_array().unwrap().iter().enumerate() {
                    same(
                        menu.inventory.slot(i).unwrap(),
                        e,
                        &case["name"],
                        step,
                        &i.to_string(),
                    );
                }
                same(
                    menu.inventory.carried(),
                    &expected["carried"],
                    &case["name"],
                    step,
                    "cursor",
                );
                check_drop_arguments(&out.drops, expected);
                let drops = expected["drops"].as_array().unwrap();
                assert_eq!(
                    out.drops.len(),
                    drops.len(),
                    "{},step{step}: drops",
                    case["name"]
                );
                for (a, e) in out.drops.iter().zip(drops) {
                    same(a, e, &case["name"], step, "drop");
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 6158);
    }
    #[test]
    fn actual_java_result_slot_consumption_and_partial_shift_drops() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../tools/container_result_vectors.json")).unwrap();
        let stacks = data["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                ItemStack::decode_exact(&STANDARD.decode(v.as_str().unwrap()).unwrap()).unwrap()
            })
            .collect::<Vec<_>>();
        let same = |actual: &ItemStack,
                    index: &serde_json::Value,
                    case: &serde_json::Value,
                    step: usize,
                    slot: &str| {
            let expected = &stacks[index.as_u64().unwrap() as usize];
            assert!(
                stack_matches(actual, expected).unwrap(),
                "{case},step{step},{slot}: actual{actual:?},expected{expected:?}"
            );
        };
        let mut checked = 0;
        for case in data["cases"].as_array().unwrap() {
            let mut menu = PlayerContainer::new();
            menu.crafting_enabled = true;
            for (i, v) in case["slots"].as_array().unwrap().iter().enumerate() {
                menu.inventory
                    .set_slot_authoritative(i, stacks[v.as_u64().unwrap() as usize].clone());
            }
            menu.inventory.set_carried_authoritative(
                stacks[case["carried"].as_u64().unwrap() as usize].clone(),
            );
            menu.refresh_crafting(&RecipeContext::default()).unwrap();
            let mut out = ClickOutcome::default();
            for (step, (click, expected)) in case["clicks"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["states"].as_array().unwrap())
                .enumerate()
            {
                let kind = match click["type"].as_str().unwrap() {
                    "PICKUP" => ClickType::Pickup,
                    "QUICK_MOVE" => ClickType::QuickMove,
                    "SWAP" => ClickType::Swap,
                    "CLONE" => ClickType::Clone,
                    "THROW" => ClickType::Throw,
                    "QUICK_CRAFT" => ClickType::QuickCraft,
                    "PICKUP_ALL" => ClickType::PickupAll,
                    _ => panic!("unknown oracle click"),
                };
                menu.do_click(
                    click["slot"].as_i64().unwrap() as i32,
                    click["button"].as_i64().unwrap() as i32,
                    kind,
                    case["creative"].as_bool().unwrap(),
                    &RecipeContext::default(),
                    &mut out,
                )
                .unwrap_or_else(|e| panic!("{},step{step}: {e:#}", case["name"]));
                for (i, e) in expected["slots"].as_array().unwrap().iter().enumerate() {
                    same(
                        menu.inventory.slot(i).unwrap(),
                        e,
                        &case["name"],
                        step,
                        &i.to_string(),
                    );
                }
                same(
                    menu.inventory.carried(),
                    &expected["carried"],
                    &case["name"],
                    step,
                    "cursor",
                );
                check_drop_arguments(&out.drops, expected);
                let drops = expected["drops"].as_array().unwrap();
                assert_eq!(
                    out.drops.len(),
                    drops.len(),
                    "{},step{step}: drops",
                    case["name"]
                );
                for (a, e) in out.drops.iter().zip(drops) {
                    same(a, e, &case["name"], step, "drop");
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 30);
    }
    #[test]
    fn actual_java_close_returns_cursor_and_crafting_inputs_before_dropping_overflow() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../tools/container_close_vectors.json")).unwrap();
        let stacks = data["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                ItemStack::decode_exact(&STANDARD.decode(v.as_str().unwrap()).unwrap()).unwrap()
            })
            .collect::<Vec<_>>();
        for case in data["cases"].as_array().unwrap() {
            let mut menu = PlayerContainer::new();
            for (i, v) in case["slots"].as_array().unwrap().iter().enumerate() {
                menu.inventory
                    .set_slot_authoritative(i, stacks[v.as_u64().unwrap() as usize].clone());
            }
            menu.inventory
                .set_selected_hotbar(case["selected"].as_i64().unwrap() as i16)
                .unwrap();
            menu.inventory.set_carried_authoritative(
                stacks[case["carried"].as_u64().unwrap() as usize].clone(),
            );
            menu.creative_mode = case["creative"].as_bool().unwrap();
            let outcome = menu.close().unwrap();
            let state = &case["states"][0];
            for (i, v) in state["slots"].as_array().unwrap().iter().enumerate() {
                assert!(
                    stack_matches(
                        menu.inventory.slot(i).unwrap(),
                        &stacks[v.as_u64().unwrap() as usize]
                    )
                    .unwrap(),
                    "{},slot{i}",
                    case["name"]
                );
            }
            assert!(
                stack_matches(
                    menu.inventory.carried(),
                    &stacks[state["carried"].as_u64().unwrap() as usize]
                )
                .unwrap(),
                "{}:cursor",
                case["name"]
            );
            check_drop_arguments(&outcome.drops, state);
            let drops = state["drops"].as_array().unwrap();
            assert_eq!(outcome.drops.len(), drops.len(), "{}:drops", case["name"]);
            for (a, b) in outcome.drops.iter().zip(drops) {
                assert!(
                    stack_matches(a, &stacks[b.as_u64().unwrap() as usize]).unwrap(),
                    "{}:drop",
                    case["name"]
                );
            }
        }
        assert_eq!(data["cases"].as_array().unwrap().len(), 32);
    }
    #[test]
    fn pickup_split_deposit_merge_and_clone() {
        let mut menu = PlayerContainer::new();
        menu.inventory.set_slot_authoritative(9, stack("stone", 10));
        click(&mut menu, 9, 1, ClickType::Pickup, false);
        assert_eq!(menu.inventory.carried().count, 5);
        assert_eq!(menu.inventory.slot(9).unwrap().count, 5);
        click(&mut menu, 10, 1, ClickType::Pickup, false);
        assert_eq!(menu.inventory.carried().count, 4);
        assert_eq!(menu.inventory.slot(10).unwrap().count, 1);
        click(&mut menu, 9, 0, ClickType::Pickup, false);
        assert!(menu.inventory.carried().is_empty());
        assert_eq!(menu.inventory.slot(9).unwrap().count, 9);
        click(&mut menu, 9, 2, ClickType::Clone, true);
        assert_eq!(menu.inventory.carried().count, 64);
        assert_eq!(menu.inventory.slot(9).unwrap().count, 9);
    }
    #[test]
    fn quickcraft_distributes_each_share_without_redistributing_clamped_surplus() {
        let mut menu = PlayerContainer::new();
        menu.inventory.set_carried_authoritative(stack("stone", 10));
        menu.inventory
            .set_slot_authoritative(10, stack("stone", 63));
        click(&mut menu, -999, 0, ClickType::QuickCraft, false);
        for slot in [9, 10, 11] {
            click(&mut menu, slot, 1, ClickType::QuickCraft, false);
        }
        click(&mut menu, -999, 2, ClickType::QuickCraft, false);
        assert_eq!(
            menu.inventory.slots()[9..12]
                .iter()
                .map(|s| s.count)
                .collect::<Vec<_>>(),
            vec![3, 64, 3]
        );
        assert_eq!(menu.inventory.carried().count, 3);
    }
    #[test]
    fn result_pickup_consumes_one_recipe_and_returns_entire_output_on_right_click() {
        let mut menu = PlayerContainer::new();
        menu.inventory
            .set_slot_authoritative(1, stack("oak_log", 2));
        let out = click(&mut menu, 0, 1, ClickType::Pickup, false);
        assert_eq!(menu.inventory.carried(), &stack("oak_planks", 4));
        assert_eq!(menu.inventory.slot(1).unwrap().count, 1);
        assert_eq!(menu.inventory.slot(0).unwrap(), &stack("oak_planks", 4));
        assert_eq!(out.crafted, vec!["minecraft:oak_planks"]);
    }
    #[test]
    fn shift_crafting_repeats_until_input_empty_and_moves_from_hotbar_end() {
        let mut menu = PlayerContainer::new();
        menu.inventory
            .set_slot_authoritative(1, stack("oak_log", 64));
        let out = click(&mut menu, 0, 0, ClickType::QuickMove, false);
        assert_eq!(out.crafted.len(), 64);
        assert!(menu.inventory.slot(0).unwrap().is_empty());
        assert!(menu.inventory.slot(1).unwrap().is_empty());
        assert!(menu.inventory.slots()[41..45]
            .iter()
            .all(|s| s == &stack("oak_planks", 64)));
    }
    #[test]
    fn client_prediction_never_assigns_items_and_stale_state_causes_full_sync() {
        let mut menu = PlayerContainer::new();
        menu.inventory.set_slot_authoritative(9, stack("stone", 10));
        let mut req = request(&menu, -1, 0, ClickType::Pickup);
        req.changed_slots.insert(9, stack("diamond", 64));
        let out = menu.click(&req, false, &RecipeContext::default()).unwrap();
        assert_eq!(menu.inventory.slot(9).unwrap(), &stack("stone", 10));
        assert_eq!(out.packets.len(), 1);
        assert_eq!(out.packets[0].id, 0x15);
        let mut req = request(&menu, -1, 0, ClickType::Pickup);
        req.state_id = menu.inventory.state_id() - 1;
        let state = menu.inventory.state_id();
        let out = menu.click(&req, false, &RecipeContext::default()).unwrap();
        assert_eq!(out.packets.len(), 1);
        assert_eq!(out.packets[0].id, 0x13);
        assert_eq!(menu.inventory.state_id(), state + 1);
    }
}
