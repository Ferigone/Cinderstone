//! Shared workstation synchronization and temporary-container removal.
//!
//! Inputs remain in their station and the player's inventory is borrowed. The
//! only owned live stack here is this menu's cursor. `SyncEvent`s retain the
//! listener/packet order; the host must dispatch listeners and stage world/item
//! effects before publishing a transaction. Reentrant listeners are unsupported.
use crate::{
    container::{
        return_to_inventory_on_menu_close, ClickOutcome, ContainerClick, DroppedStack,
        MenuCloseContext, PlayerContainer,
    },
    inventory::{ItemStack, PlayerInventory},
    protocol::{Packet, Writer},
};
use anyhow::{ensure, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkstationKind {
    Anvil,
    Grindstone,
    Loom,
    Smithing,
    Cartography,
    Stonecutter,
    Enchanting,
    Crafter,
    Merchant,
    Hopper,
    Dispenser,
    Shulker,
    Brewing,
}
impl WorkstationKind {
    /// Original 1.21 MenuType registration IDs (protocol 767).
    pub fn protocol_id(self) -> i32 {
        match self {
            Self::Anvil => 8,
            Self::Grindstone => 15,
            Self::Loom => 18,
            Self::Smithing => 21,
            Self::Cartography => 23,
            Self::Stonecutter => 24,
            Self::Enchanting => 13,
            Self::Crafter => 7,
            Self::Merchant => 19,
            Self::Hopper => 16,
            Self::Dispenser => 6,
            Self::Shulker => 20,
            Self::Brewing => 11,
        }
    }
    pub fn top_slots(self) -> usize {
        match self {
            Self::Loom | Self::Smithing => 4,
            Self::Stonecutter | Self::Enchanting => 2,
            // Nine actual inputs plus the noninteractive preview appended
            // AFTER the player inventory in CrafterMenu's slot list.
            Self::Crafter => 10,
            Self::Hopper | Self::Brewing => 5,
            Self::Dispenser => 9,
            Self::Shulker => 27,
            _ => 3,
        }
    }
    pub fn data_slots(self) -> usize {
        match self {
            Self::Anvil | Self::Loom | Self::Stonecutter => 1,
            Self::Enchanting | Self::Crafter => 10,
            Self::Brewing => 2,
            _ => 0,
        }
    }
    /// Actual player slot positions in this menu, not a presumed contiguous
    /// top-container offset. CrafterMenu appends its preview at slot45.
    pub fn player_slots(self) -> std::ops::Range<usize> {
        let start = if self == Self::Crafter {
            9
        } else {
            self.top_slots()
        };
        start..start + 36
    }
    pub fn player_inventory_slot(self, menu_slot: usize) -> Option<usize> {
        let slots = self.player_slots();
        slots
            .contains(&menu_slot)
            .then(|| 9 + menu_slot - slots.start)
    }
}

/// Observations are deliberately ordered, rather than putting all listeners
/// after all packets. An actual ServerPlayer listener filters by backing owner.
#[derive(Clone, Debug)]
pub enum SyncEvent {
    SlotListener {
        menu_slot: usize,
        player_slot: Option<usize>,
        stack: ItemStack,
    },
    DataListener {
        index: usize,
        value: i32,
    },
    Packet(Packet),
}

/// Indices into one removal outcome, retaining ordering across per-stack
/// inventory insertion packets and actual Player.drop callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuRemovalEvent {
    Packet { index: usize },
    Drop { index: usize },
}
#[derive(Debug)]
pub struct MenuRemoval {
    pub outcome: ClickOutcome,
    pub events: Vec<MenuRemovalEvent>,
}

/// Implement with the captured ContainerLevelAccess and actual player. The
/// player's attribute owner decides reach; 1.21 passes additional range 4.0.
pub trait WorkstationAccess {
    fn block_matches(&self, kind: WorkstationKind) -> Result<bool>;
    fn player_can_interact_with_block(&self, additional_range: f64) -> Result<bool>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuSync {
    window_id: u8,
    kind: WorkstationKind,
    state_id: i32,
    carried: ItemStack,
    last: Vec<ItemStack>,
    remote: Vec<ItemStack>,
    remote_carried: ItemStack,
    last_data: Vec<i32>,
    remote_data: Vec<i32>,
    suppress_remote: bool,
}

fn copy(stack: &ItemStack) -> ItemStack {
    if stack.is_empty() {
        ItemStack::empty()
    } else {
        stack.clone()
    }
}
fn matches(a: &ItemStack, b: &ItemStack) -> Result<bool> {
    Ok(
        (if a.is_empty() { 0 } else { a.count }) == (if b.is_empty() { 0 } else { b.count })
            && a.same_item_same_components(b)?,
    )
}
impl MenuSync {
    pub fn new(window_id: u8, kind: WorkstationKind) -> Result<Self> {
        ensure!((1..=100).contains(&window_id), "invalid workstation window");
        Ok(Self {
            window_id,
            kind,
            state_id: 0,
            carried: ItemStack::empty(),
            last: vec![ItemStack::empty(); kind.top_slots() + 36],
            remote: vec![ItemStack::empty(); kind.top_slots() + 36],
            remote_carried: ItemStack::empty(),
            last_data: vec![0; kind.data_slots()],
            remote_data: vec![0; kind.data_slots()],
            suppress_remote: false,
        })
    }
    pub fn window_id(&self) -> u8 {
        self.window_id
    }
    pub fn kind(&self) -> WorkstationKind {
        self.kind
    }
    pub fn state_id(&self) -> i32 {
        self.state_id
    }
    pub fn carried(&self) -> &ItemStack {
        &self.carried
    }
    /// Called only by authoritative menu actions, never client predictions.
    pub fn set_carried(&mut self, stack: ItemStack) {
        self.carried = copy(&stack);
    }
    pub fn suppress_remote_updates(&mut self, suppress: bool) {
        self.suppress_remote = suppress;
    }
    pub fn still_valid(&self, access: &impl WorkstationAccess) -> Result<bool> {
        Ok(access.block_matches(self.kind)? && access.player_can_interact_with_block(4.0)?)
    }
    fn increment_state(&mut self) -> i32 {
        self.state_id = (self.state_id + 1) & 32767;
        self.state_id
    }
    pub fn snapshot(
        &self,
        top: &[ItemStack],
        inventory: &PlayerInventory,
    ) -> Result<Vec<ItemStack>> {
        ensure!(
            top.len() == self.kind.top_slots(),
            "workstation slot count changed"
        );
        let input_count = if self.kind == WorkstationKind::Crafter {
            9
        } else {
            top.len()
        };
        let mut slots = top[..input_count].iter().map(copy).collect::<Vec<_>>();
        slots.extend((9..45).map(|i| copy(inventory.slot(i).expect("canonical player slot"))));
        if self.kind == WorkstationKind::Crafter {
            slots.push(copy(&top[9]));
        }
        Ok(slots)
    }
    fn check_data(&self, data: &[i32]) -> Result<()> {
        ensure!(
            data.len() == self.kind.data_slots(),
            "workstation data count changed"
        );
        Ok(())
    }
    /// Read-only gate for a real local slot listener such as CrafterMenu's
    /// recipe-cache listener. Remote predictions/resends are not its baseline.
    pub fn has_local_slot_changes(
        &self,
        top: &[ItemStack],
        inventory: &PlayerInventory,
    ) -> Result<bool> {
        for (slot, stack) in self.snapshot(top, inventory)?.iter().enumerate() {
            if !matches(&self.last[slot], stack)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn slot_packet(&mut self, slot: i16, stack: &ItemStack) -> Result<Packet> {
        let state = self.increment_state();
        let mut w = Writer::new();
        w.i8(if slot == -1 { -1 } else { self.window_id as i8 })
            .varint(state)
            .i16(slot)
            .raw(&stack.encode_canonical()?);
        Ok(Packet {
            id: 0x15,
            payload: w.into_inner(),
        })
    }
    fn data_packet(&self, index: usize, value: i32) -> Packet {
        let mut w = Writer::new();
        w.u8(self.window_id).i16(index as i16).i16(value as i16);
        Packet {
            id: 0x14,
            payload: w.into_inner(),
        }
    }
    fn local_slot(
        &mut self,
        slot: usize,
        stack: &ItemStack,
        out: &mut Vec<SyncEvent>,
    ) -> Result<()> {
        if !matches(&self.last[slot], stack)? {
            self.last[slot] = copy(stack);
            out.push(SyncEvent::SlotListener {
                menu_slot: slot,
                player_slot: self.kind.player_inventory_slot(slot),
                stack: copy(stack),
            });
        }
        Ok(())
    }
    fn local_data(&mut self, index: usize, value: i32, out: &mut Vec<SyncEvent>) {
        if self.last_data[index] != value {
            self.last_data[index] = value;
            out.push(SyncEvent::DataListener { index, value });
        }
    }
    /// AbstractContainerMenu.broadcastChanges: per-slot local then remote,
    /// carried after slots, then local/remote data. Suppression affects remote
    /// changes only. Predictions never suppress a local inventory listener.
    pub fn broadcast_changes(
        &mut self,
        top: &[ItemStack],
        inventory: &PlayerInventory,
        data: &[i32],
    ) -> Result<Vec<SyncEvent>> {
        self.check_data(data)?;
        let slots = self.snapshot(top, inventory)?;
        let mut next = self.clone();
        let mut out = vec![];
        for (i, stack) in slots.iter().enumerate() {
            next.local_slot(i, stack, &mut out)?;
            if !next.suppress_remote && !matches(&next.remote[i], stack)? {
                next.remote[i] = copy(stack);
                out.push(SyncEvent::Packet(next.slot_packet(i as i16, stack)?));
            }
        }
        if !next.suppress_remote && !matches(&next.remote_carried, &next.carried)? {
            let carried = copy(&next.carried);
            next.remote_carried = carried.clone();
            out.push(SyncEvent::Packet(next.slot_packet(-1, &carried)?));
        }
        for (i, &value) in data.iter().enumerate() {
            next.local_data(i, value, &mut out);
            if !next.suppress_remote && next.remote_data[i] != value {
                next.remote_data[i] = value;
                out.push(SyncEvent::Packet(next.data_packet(i, value)));
            }
        }
        *self = next;
        Ok(out)
    }
    /// Remote-only sendAllDataToRemote; also runs when incrementals are
    /// suppressed. Opening must first install/observe the actual local listener.
    pub fn send_all_data(
        &mut self,
        top: &[ItemStack],
        inventory: &PlayerInventory,
        data: &[i32],
    ) -> Result<Vec<SyncEvent>> {
        self.check_data(data)?;
        let slots = self.snapshot(top, inventory)?;
        let mut next = self.clone();
        let mut w = Writer::new();
        w.u8(self.window_id)
            .varint(next.increment_state())
            .varint(slots.len() as i32);
        for stack in &slots {
            w.raw(&stack.encode_canonical()?);
        }
        w.raw(&next.carried.encode_canonical()?);
        next.remote = slots;
        next.remote_carried = copy(&next.carried);
        next.remote_data.clone_from_slice(data);
        let mut out = vec![SyncEvent::Packet(Packet {
            id: 0x13,
            payload: w.into_inner(),
        })];
        for (i, &value) in data.iter().enumerate() {
            out.push(SyncEvent::Packet(next.data_packet(i, value)));
        }
        *self = next;
        Ok(out)
    }
    pub fn broadcast_full_state(
        &mut self,
        top: &[ItemStack],
        inventory: &PlayerInventory,
        data: &[i32],
    ) -> Result<Vec<SyncEvent>> {
        self.check_data(data)?;
        let slots = self.snapshot(top, inventory)?;
        let mut next = self.clone();
        let mut out = vec![];
        for (i, stack) in slots.iter().enumerate() {
            next.local_slot(i, stack, &mut out)?;
        }
        for (i, &value) in data.iter().enumerate() {
            next.local_data(i, value, &mut out);
        }
        out.extend(next.send_all_data(top, inventory, data)?);
        *self = next;
        Ok(out)
    }
    /// Bounded ServerPlayer.initMenu ordering: observe the actual local
    /// listener before installing its synchronizer. There are no incremental
    /// packets/state-ID increments before the initial content packet.
    pub fn initialize(
        &mut self,
        top: &[ItemStack],
        inventory: &PlayerInventory,
        data: &[i32],
    ) -> Result<Vec<SyncEvent>> {
        self.broadcast_full_state(top, inventory, data)
    }
    /// The packet handler captures `state_before_action` before executing the
    /// authoritative click with remote updates suppressed. Predictions update
    /// only remote baselines; stale clicks still execute then receive full state.
    pub fn finish_click(
        &mut self,
        click: &ContainerClick,
        state_before_action: i32,
        top: &[ItemStack],
        inventory: &PlayerInventory,
        data: &[i32],
    ) -> Result<Vec<SyncEvent>> {
        ensure!(
            click.window_id == self.window_id,
            "wrong workstation window"
        );
        let mut next = self.clone();
        for (&index, stack) in &click.changed_slots {
            if index >= 0 && (index as usize) < next.remote.len() {
                next.remote[index as usize] = copy(stack);
            }
        }
        next.remote_carried = copy(&click.carried);
        next.suppress_remote = false;
        let out = if click.state_id == state_before_action {
            next.broadcast_changes(top, inventory, data)?
        } else {
            next.broadcast_full_state(top, inventory, data)?
        };
        *self = next;
        Ok(out)
    }
    /// Cursor first, then actual temporary input slots in container order.
    /// Never include the station's derived result preview in `inputs`.
    /// The caller stages actual drop creation before committing these snapshots.
    pub fn close_inputs(
        &mut self,
        inputs: &mut [ItemStack],
        inventory: &mut PlayerInventory,
        context: MenuCloseContext,
    ) -> Result<ClickOutcome> {
        Ok(self
            .close_inputs_ordered(inputs, inventory, context)?
            .outcome)
    }
    /// Production removal uses this ordered form: an overflow drop from the
    /// cursor or an earlier input may precede a later input's insertion packet.
    /// The aggregate compatibility method cannot express that ordering alone.
    pub fn close_inputs_ordered(
        &mut self,
        inputs: &mut [ItemStack],
        inventory: &mut PlayerInventory,
        context: MenuCloseContext,
    ) -> Result<MenuRemoval> {
        let mut next_inventory = inventory.clone();
        let mut owned = std::iter::once(copy(&self.carried))
            .chain(inputs.iter().map(copy))
            .collect::<Vec<_>>();
        let mut out = ClickOutcome::default();
        let mut events = vec![];
        for stack in &mut owned {
            let packet_start = out.packets.len();
            let drop_start = out.drops.len();
            match context {
                MenuCloseContext::AliveConnected => {
                    return_to_inventory_on_menu_close(&mut next_inventory, stack, &mut out)?
                }
                MenuCloseContext::DeadOrDisconnected if !stack.is_empty() => {
                    out.drops.push(DroppedStack {
                        stack: std::mem::take(stack),
                        around: false,
                        trace_item: false,
                    })
                }
                _ => {}
            }
            // Each placeItemBackInInventory sends its insertion packets first,
            // then drops any one remaining overflow before the next input.
            events.extend(
                (packet_start..out.packets.len()).map(|index| MenuRemovalEvent::Packet { index }),
            );
            events.extend(
                (drop_start..out.drops.len()).map(|index| MenuRemovalEvent::Drop { index }),
            );
        }
        *inventory = next_inventory;
        self.carried = ItemStack::empty();
        inputs.fill(ItemStack::empty());
        Ok(MenuRemoval {
            outcome: out,
            events,
        })
    }
    /// Preserve old menu listener and remote baselines for the actual shared
    /// inventory slots. Top slots, preview, data and cursor are never transferred.
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        for offset in 0..36 {
            let slot = self.kind.player_slots().start + offset;
            player.transfer_external_slot_state(
                9 + offset,
                &self.last[slot],
                &self.remote[slot],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{container::ClickType, crafting::RecipeBook, protocol::Reader};
    use std::collections::BTreeMap;
    fn stone(n: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id("stone").unwrap(), n).unwrap()
    }
    #[test]
    fn predictions_preserve_local_callbacks_and_stale_click_resends_data() -> Result<()> {
        let inv = PlayerInventory::default();
        let mut sync = MenuSync::new(4, WorkstationKind::Stonecutter)?;
        let top = [stone(3), ItemStack::empty()];
        sync.send_all_data(&top, &inv, &[-1])?;
        let click = ContainerClick {
            window_id: 4,
            state_id: sync.state_id(),
            slot: 0,
            button: 0,
            kind: ClickType::Pickup,
            changed_slots: BTreeMap::from([(0, stone(3))]),
            carried: ItemStack::empty(),
        };
        let out = sync.finish_click(&click, sync.state_id(), &top, &inv, &[-1])?;
        assert!(matches!(
            out.as_slice(),
            [
                SyncEvent::SlotListener { menu_slot: 0, .. },
                SyncEvent::DataListener {
                    index: 0,
                    value: -1
                }
            ]
        ));
        let out = sync.finish_click(&click, sync.state_id() + 1, &top, &inv, &[-1])?;
        assert!(matches!(&out[0], SyncEvent::Packet(p) if p.id == 0x13));
        assert!(matches!(&out[1], SyncEvent::Packet(p) if p.id == 0x14));
        Ok(())
    }
    #[test]
    fn listener_packet_order_cursor_and_state_wrap() -> Result<()> {
        let inv = PlayerInventory::default();
        let mut sync = MenuSync::new(2, WorkstationKind::Grindstone)?;
        sync.state_id = 32767;
        sync.set_carried(stone(2));
        let top = [stone(1), stone(1), ItemStack::empty()];
        let out = sync.broadcast_changes(&top, &inv, &[])?;
        assert!(matches!(
            &out[0],
            SyncEvent::SlotListener { menu_slot: 0, .. }
        ));
        assert!(matches!(
            &out[2],
            SyncEvent::SlotListener { menu_slot: 1, .. }
        ));
        if let SyncEvent::Packet(packet) = &out[1] {
            let mut r = Reader::new(&packet.payload);
            assert_eq!(r.u8()?, 2);
            assert_eq!(r.varint()?, 0);
        } else {
            panic!("slot packet missing");
        }
        assert!(matches!(&out[4], SyncEvent::Packet(p) if p.payload[0] == 255));
        assert_eq!(sync.state_id(), 2);
        Ok(())
    }
    #[test]
    fn disconnected_removal_returns_only_cursor_and_owned_inputs_in_order() -> Result<()> {
        let mut inv = PlayerInventory::default();
        let mut sync = MenuSync::new(1, WorkstationKind::Stonecutter)?;
        sync.set_carried(stone(2));
        let mut inputs = [stone(3)];
        let out = sync.close_inputs(&mut inputs, &mut inv, MenuCloseContext::DeadOrDisconnected)?;
        assert_eq!(
            out.drops.iter().map(|d| d.count).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert!(out.drops.iter().all(|d| !d.around && !d.trace_item));
        assert!(inputs[0].is_empty());
        assert!(sync.carried().is_empty());
        assert!(inv.slot(36).unwrap().is_empty());
        Ok(())
    }
    #[test]
    fn alive_overflow_drop_precedes_later_input_insertion_packet() -> Result<()> {
        let mut inv = PlayerInventory::default();
        for slot in 9..45 {
            inv.set_slot_authoritative(slot, stone(64));
        }
        let coal_id = RecipeBook::vanilla().item_id("coal").unwrap();
        inv.set_slot_authoritative(10, ItemStack::new(coal_id, 63)?);
        let mut sync = MenuSync::new(1, WorkstationKind::Stonecutter)?;
        sync.set_carried(stone(1));
        let mut inputs = [ItemStack::new(coal_id, 1)?];
        let removal =
            sync.close_inputs_ordered(&mut inputs, &mut inv, MenuCloseContext::AliveConnected)?;
        assert_eq!(
            removal.events,
            vec![
                MenuRemovalEvent::Drop { index: 0 },
                MenuRemovalEvent::Packet { index: 0 }
            ]
        );
        assert_eq!(removal.outcome.drops[0].stack, stone(1));
        assert_eq!(inv.slot(10).unwrap().count, 64);
        assert!(sync.carried().is_empty());
        assert!(inputs[0].is_empty());
        Ok(())
    }
    #[test]
    fn crafter_cache_gate_observes_local_changes_after_remote_only_resend() -> Result<()> {
        let inv = PlayerInventory::default();
        let mut top = vec![ItemStack::empty(); 10];
        let mut sync = MenuSync::new(3, WorkstationKind::Crafter)?;
        sync.initialize(&top, &inv, &[0; 10])?;
        assert!(!sync.has_local_slot_changes(&top, &inv)?);
        top[0] = stone(1);
        sync.send_all_data(&top, &inv, &[0; 10])?;
        let before = sync.clone();
        assert!(sync.has_local_slot_changes(&top, &inv)?);
        assert_eq!(sync, before);
        sync.broadcast_changes(&top, &inv, &[0; 10])?;
        assert!(!sync.has_local_slot_changes(&top, &inv)?);
        Ok(())
    }
    #[test]
    fn crafter_preview_follows_actual_player_slots_and_has_no_player_listener_owner() -> Result<()>
    {
        let mut inv = PlayerInventory::default();
        let coal = ItemStack::new(RecipeBook::vanilla().item_id("coal").unwrap(), 1)?;
        inv.set_slot_authoritative(9, coal.clone());
        inv.set_slot_authoritative(44, stone(4));
        let mut top = vec![ItemStack::empty(); 10];
        top[0] = stone(1);
        top[9] = stone(2);
        let mut sync = MenuSync::new(3, WorkstationKind::Crafter)?;
        let snapshot = sync.snapshot(&top, &inv)?;
        assert_eq!(snapshot.len(), 46);
        assert_eq!(snapshot[9], coal);
        assert_eq!(snapshot[44], stone(4));
        assert_eq!(snapshot[45], stone(2));
        let events = sync.initialize(&top, &inv, &[0; 10])?;
        assert!(events.iter().any(|event| matches!(
            event,
            SyncEvent::SlotListener {
                menu_slot: 9,
                player_slot: Some(9),
                ..
            }
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            SyncEvent::SlotListener {
                menu_slot: 44,
                player_slot: Some(44),
                ..
            }
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            SyncEvent::SlotListener {
                menu_slot: 45,
                player_slot: None,
                ..
            }
        )));
        assert_eq!(WorkstationKind::Crafter.protocol_id(), 7);
        Ok(())
    }
}
