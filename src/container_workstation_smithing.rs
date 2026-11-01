//! Java 1.21 smithing menu click adapter over the pinned default recipes.
//!
//! Player inventory is borrowed; inputs/cursor remain in this menu. Preparing
//! an action changes no live owner. The host must stage the recorded actual
//! player/item/world callbacks and drops, commit the one-shot plan, then publish
//! its ordered callback/drop/listener/packet events. Bundle overrides remain explicit.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    workstations::{
        menu::{MenuRemovalEvent, MenuSync, SyncEvent, WorkstationAccess, WorkstationKind},
        smithing::{
            SmithingEffect, SmithingRecipeOrder, SmithingState, SmithingTakeKind, SmithingTakePlan,
            RESULT_SLOT,
        },
    },
};
use anyhow::{bail, ensure, Result};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmithingMenu {
    station: SmithingState,
    sync: MenuSync,
    // This identifies only this temporary UI owner, never a block/world/player.
    instance: u64,
    revision: u64,
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
}
/// Read-only observations of actual staged owners; no snapshot creates items.
/// PlayerInventory retains window0's carried value, while carried is this
/// external menu's cursor. Inputs/result retain their source callback phase.
#[derive(Clone, Debug)]
pub struct SmithingPhase {
    pub inputs: [ItemStack; 3],
    pub result_preview: ItemStack,
    pub player_inventory: PlayerInventory,
    pub carried: ItemStack,
}
#[derive(Clone, Debug)]
pub enum SmithingMenuEvent {
    Effect {
        effect: SmithingEffect,
        phase: SmithingPhase,
    },
    Sync {
        event: SyncEvent,
        phase: SmithingPhase,
    },
    /// Actual Inventory.setChanged/timesChanged, distinct from slot listeners.
    PlayerInventoryChanged { phase: SmithingPhase },
    /// Resolve this index in outcome.drops; stage each actual drop once.
    Drop { index: usize, phase: SmithingPhase },
}
impl SmithingPhase {
    /// Observe a raw per-insertion packet without sending it. Returning is an
    /// observation of either the carried object or a removed grid stack.
    fn observe_return_packet(
        &mut self,
        packet: &crate::protocol::Packet,
        returning: &mut ItemStack,
    ) -> Result<()> {
        ensure!(packet.id == 0x15, "unsupported smithing close packet phase");
        let mut reader = crate::protocol::Reader::new(&packet.payload);
        ensure!(
            reader.i8()? == -2 && reader.varint()? == 0,
            "smithing close requires raw inventory packet"
        );
        let raw = reader.i16()?;
        let slot = match raw {
            0..=8 => 36 + raw as usize,
            9..=35 => raw as usize,
            40 => 45,
            _ => bail!("unsupported smithing close raw slot"),
        };
        let stack = ItemStack::decode_exact(reader.remaining())?;
        let old = self.player_inventory.slot(slot).unwrap();
        ensure!(
            stack.same_item_same_components(returning)?,
            "smithing close changed return identity"
        );
        let old_count = if old.is_empty() {
            0
        } else {
            ensure!(
                old.same_item_same_components(&stack)?,
                "smithing close overwrote owned stack"
            );
            old.count
        };
        let added = stack.count - old_count;
        ensure!(
            added > 0 && added <= returning.count,
            "invalid smithing close insertion amount"
        );
        self.player_inventory.set_slot_authoritative(slot, stack);
        *returning = counted(returning, returning.count - added);
        Ok(())
    }
}
#[derive(Debug, Default)]
pub struct SmithingActionOutcome {
    /// One stream of actual callbacks and source intermediate/final sync.
    /// Input changes broadcast the previous preview before createResult.
    pub events: Vec<SmithingMenuEvent>,
    pub drops: Vec<DroppedStack>,
}
#[derive(Debug)]
pub struct SmithingActionPlan {
    before: SmithingMenu,
    before_inventory: PlayerInventory,
    next: SmithingMenu,
    next_inventory: PlayerInventory,
    outcome: SmithingActionOutcome,
}
impl SmithingActionPlan {
    pub fn outcome(&self) -> &SmithingActionOutcome {
        &self.outcome
    }
    pub fn events(&self) -> &[SmithingMenuEvent] {
        &self.outcome.events
    }
    pub fn drops(&self) -> &[DroppedStack] {
        &self.outcome.drops
    }
    pub fn next_menu(&self) -> &SmithingMenu {
        &self.next
    }
    pub fn next_inventory(&self) -> &PlayerInventory {
        &self.next_inventory
    }
    /// Call before staging effects; commit checks again after staging.
    pub fn validate(&self, menu: &SmithingMenu, inventory: &PlayerInventory) -> Result<()> {
        ensure!(*menu == self.before, "stale smithing menu action");
        ensure!(
            same_inventory(inventory, &self.before_inventory),
            "borrowed player inventory changed during smithing action"
        );
        Ok(())
    }
    /// Actual callback/drop owners must be staged successfully before this
    /// commit. Publication consumes the plan and checks both borrowed owners.
    pub fn commit(
        self,
        menu: &mut SmithingMenu,
        inventory: &mut PlayerInventory,
    ) -> Result<SmithingActionOutcome> {
        self.validate(menu, inventory)?;
        *menu = self.next;
        *inventory = self.next_inventory;
        Ok(self.outcome)
    }
}
fn same_inventory(a: &PlayerInventory, b: &PlayerInventory) -> bool {
    a.slots() == b.slots()
        && a.carried() == b.carried()
        && a.selected_hotbar() == b.selected_hotbar()
        && a.state_id() == b.state_id()
}
fn counted(stack: &ItemStack, count: i32) -> ItemStack {
    if count <= 0 || stack.is_empty() {
        ItemStack::empty()
    } else {
        let mut s = stack.clone();
        s.count = count;
        s
    }
}
fn limit(stack: &ItemStack) -> Result<i32> {
    let max = stack.max_stack_size()?.min(99);
    ensure!((1..=99).contains(&max), "invalid smithing menu stack limit");
    Ok(max)
}
impl SmithingMenu {
    pub fn new(window_id: u8, order: SmithingRecipeOrder) -> Result<Self> {
        static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
        let mut observed = NEXT_INSTANCE.load(Ordering::Relaxed);
        let instance = loop {
            let next = observed
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("smithing menu identity exhausted"))?;
            match NEXT_INSTANCE.compare_exchange_weak(
                observed,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break observed,
                Err(actual) => observed = actual,
            }
        };
        Ok(Self {
            station: SmithingState::new(order),
            sync: MenuSync::new(window_id, WorkstationKind::Smithing)?,
            instance,
            revision: 0,
            quickcraft_status: 0,
            quickcraft_type: 0,
            quickcraft_slots: BTreeSet::new(),
        })
    }
    pub fn station(&self) -> &SmithingState {
        &self.station
    }
    pub fn sync(&self) -> &MenuSync {
        &self.sync
    }
    pub fn inventory_snapshot(&self, inventory: &PlayerInventory) -> PlayerInventory {
        let mut inv = inventory.clone();
        inv.set_carried_authoritative(self.sync.carried().clone());
        inv
    }
    fn top(&self) -> [ItemStack; 4] {
        let s = self.station.inputs();
        [
            s[0].clone(),
            s[1].clone(),
            s[2].clone(),
            self.station.result().clone(),
        ]
    }
    fn advance_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("smithing menu revision exhausted"))?;
        Ok(())
    }
    fn phase(&self, inventory: &PlayerInventory) -> SmithingPhase {
        SmithingPhase {
            inputs: self.station.inputs().clone(),
            result_preview: self.station.result().clone(),
            player_inventory: inventory.clone(),
            carried: self.sync.carried().clone(),
        }
    }
    fn player_changed(&self, inventory: &PlayerInventory, outcome: &mut SmithingActionOutcome) {
        outcome
            .events
            .push(SmithingMenuEvent::PlayerInventoryChanged {
                phase: self.phase(inventory),
            });
    }
    fn player_changed_with_source(
        &self,
        inventory: &PlayerInventory,
        outcome: &mut SmithingActionOutcome,
        source_slot: usize,
        source: &ItemStack,
    ) {
        let mut phase = self.phase(inventory);
        if source_slot < RESULT_SLOT {
            phase.inputs[source_slot] = source.clone();
        } else if source_slot == RESULT_SLOT {
            phase.result_preview = source.clone();
        }
        outcome
            .events
            .push(SmithingMenuEvent::PlayerInventoryChanged { phase });
    }
    pub fn initialize(&mut self, inventory: &PlayerInventory) -> Result<Vec<SmithingMenuEvent>> {
        let mut next = self.clone();
        let events = next.sync.initialize(&next.top(), inventory, &[])?;
        let phase = next.phase(inventory);
        let events = events
            .into_iter()
            .map(|event| SmithingMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect();
        next.advance_revision()?;
        *self = next;
        Ok(events)
    }
    pub fn broadcast_changes(
        &mut self,
        inventory: &PlayerInventory,
    ) -> Result<Vec<SmithingMenuEvent>> {
        let mut next = self.clone();
        let events = next.sync.broadcast_changes(&next.top(), inventory, &[])?;
        let phase = next.phase(inventory);
        let events = events
            .into_iter()
            .map(|event| SmithingMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect();
        next.advance_revision()?;
        *self = next;
        Ok(events)
    }
    /// Resynchronize this retained menu, preserving its inputs, derived result,
    /// cursor and drag state. No result recomputation or dirty callback occurs.
    pub fn broadcast_full_state(
        &mut self,
        inventory: &PlayerInventory,
    ) -> Result<Vec<SmithingMenuEvent>> {
        let mut next = self.clone();
        let events = next
            .sync
            .broadcast_full_state(&next.top(), inventory, &[])?;
        let phase = next.phase(inventory);
        let events = events
            .into_iter()
            .map(|event| SmithingMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect();
        next.advance_revision()?;
        *self = next;
        Ok(events)
    }
    pub fn still_valid(&self, access: &impl WorkstationAccess) -> Result<bool> {
        self.sync.still_valid(access)
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }

    pub fn prepare_click(
        &self,
        click: &ContainerClick,
        inventory: &PlayerInventory,
        creative: bool,
    ) -> Result<SmithingActionPlan> {
        ensure!(
            click.window_id == self.sync.window_id(),
            "wrong smithing window"
        );
        ensure!(click.slot < 40, "smithing slot outside menu");
        let mut next = self.clone();
        let mut inv = inventory.clone();
        let mut outcome = SmithingActionOutcome::default();
        let state_before = next.sync.state_id();
        next.sync.suppress_remote_updates(true);
        next.do_click(
            click.slot as i32,
            click.button as i32,
            click.kind,
            creative,
            &mut inv,
            &mut outcome,
        )?;
        let phase = next.phase(&inv);
        outcome.events.extend(
            next.sync
                .finish_click(click, state_before, &next.top(), &inv, &[])?
                .into_iter()
                .map(|event| SmithingMenuEvent::Sync {
                    event,
                    phase: phase.clone(),
                }),
        );
        next.advance_revision()?;
        Ok(SmithingActionPlan {
            before: self.clone(),
            before_inventory: inventory.clone(),
            next,
            next_inventory: inv,
            outcome,
        })
    }

    /// The server-close packet, when applicable, is published by Main before
    /// these prepared cursor/input return/drop effects. Preview is discarded.
    pub fn prepare_close(
        &self,
        inventory: &PlayerInventory,
        context: MenuCloseContext,
    ) -> Result<SmithingActionPlan> {
        let mut next = self.clone();
        let mut inv = inventory.clone();
        next.reset_quickcraft();
        let mut inputs = next.station.drain_inputs_on_close();
        let mut outcome = SmithingActionOutcome::default();
        // Preserve per-stack packet/drop interleaving: cursor first, then each
        // original input. A partial alive insertion can send raw slots before
        // its overflow drop; later inputs follow that drop, not the reverse.
        let mut phase = self.phase(inventory);
        let mut returning = phase.carried.clone();
        let removal = next.sync.close_inputs_ordered(&mut [], &mut inv, context)?;
        Self::append_close(&mut outcome, removal, &mut phase, &mut returning, true)?;
        for (slot, input) in inputs.iter_mut().enumerate() {
            // clearContainer removes only this owned input before returning it;
            // later inputs and the old derived preview still exist at callbacks.
            phase.inputs[slot] = ItemStack::empty();
            let mut returning = input.clone();
            let removal =
                next.sync
                    .close_inputs_ordered(std::slice::from_mut(input), &mut inv, context)?;
            Self::append_close(&mut outcome, removal, &mut phase, &mut returning, false)?;
        }
        next.advance_revision()?;
        Ok(SmithingActionPlan {
            before: self.clone(),
            before_inventory: inventory.clone(),
            next,
            next_inventory: inv,
            outcome,
        })
    }
    fn append_close(
        outcome: &mut SmithingActionOutcome,
        removal: crate::workstations::menu::MenuRemoval,
        phase: &mut SmithingPhase,
        returning: &mut ItemStack,
        is_cursor: bool,
    ) -> Result<()> {
        let drop_base = outcome.drops.len();
        outcome.drops.extend(removal.outcome.drops);
        for event in removal.events {
            match event {
                MenuRemovalEvent::Packet { index } => {
                    let packet = removal.outcome.packets[index].clone();
                    phase.observe_return_packet(&packet, returning)?;
                    if is_cursor {
                        phase.carried = returning.clone();
                    }
                    outcome.events.push(SmithingMenuEvent::Sync {
                        event: SyncEvent::Packet(packet),
                        phase: phase.clone(),
                    });
                }
                MenuRemovalEvent::Drop { index } => {
                    if is_cursor {
                        phase.carried = outcome.drops[drop_base + index].stack.clone();
                    }
                    outcome.events.push(SmithingMenuEvent::Drop {
                        index: drop_base + index,
                        phase: phase.clone(),
                    });
                    *returning = ItemStack::empty();
                    if is_cursor {
                        phase.carried = ItemStack::empty();
                    }
                }
            }
        }
        if is_cursor {
            phase.carried = ItemStack::empty();
        }
        Ok(())
    }
    fn slot(&self, slot: usize, inventory: &PlayerInventory) -> ItemStack {
        if slot < 3 {
            self.station.inputs()[slot].clone()
        } else if slot == RESULT_SLOT {
            self.station.result().clone()
        } else {
            inventory
                .slot(slot + 5)
                .expect("actual player inventory")
                .clone()
        }
    }
    fn put_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        self.change_slot(slot, stack, inventory, outcome)?;
        // Slot.set -> container.setItem, then Slot.setChanged. Both input
        // callbacks broadcast and recompute, even though the second may emit
        // no changed slots. Player/result container callbacks are distinct.
        self.slot_changed(slot, inventory, outcome)
    }
    fn change_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        if slot < 3 {
            let effects = self.station.set_input_observed(slot, stack)?;
            self.observe_effects(effects, inventory, outcome)?;
        } else if slot > RESULT_SLOT {
            inventory.set_slot_authoritative(slot + 5, stack);
        } else {
            bail!("result contents are derived");
        }
        Ok(())
    }
    fn slot_changed(
        &mut self,
        slot: usize,
        inventory: &PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        if slot < RESULT_SLOT {
            let effects = self.station.input_changed_observed()?;
            self.observe_effects(effects, inventory, outcome)?;
        } else if slot > RESULT_SLOT {
            self.player_changed(inventory, outcome);
        }
        Ok(())
    }
    fn observe_effects(
        &mut self,
        effects: Vec<SmithingEffect>,
        inventory: &PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        let phase = self.phase(inventory);
        self.observe_effects_from(effects, inventory, outcome, phase)
    }
    fn observe_effects_from(
        &mut self,
        effects: Vec<SmithingEffect>,
        inventory: &PlayerInventory,
        outcome: &mut SmithingActionOutcome,
        mut phase: SmithingPhase,
    ) -> Result<()> {
        for effect in effects {
            if let SmithingEffect::BeforeCreateResult {
                inputs,
                previous_result,
            } = effect
            {
                phase.inputs = inputs.clone();
                phase.result_preview = previous_result.clone();
                let [template, base, addition] = inputs;
                let top = [template, base, addition, previous_result];
                outcome.events.extend(
                    self.sync
                        .broadcast_changes(&top, inventory, &[])?
                        .into_iter()
                        .map(|event| SmithingMenuEvent::Sync {
                            event,
                            phase: phase.clone(),
                        }),
                );
            } else {
                if matches!(effect, SmithingEffect::LevelEvent { .. }) {
                    phase.inputs = self.station.inputs().clone();
                    phase.result_preview = self.station.result().clone();
                }
                outcome.events.push(SmithingMenuEvent::Effect {
                    effect,
                    phase: phase.clone(),
                });
            }
        }
        Ok(())
    }
    fn commit_take(
        &mut self,
        plan: SmithingTakePlan,
        inventory: &PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<ItemStack> {
        let mut phase = self.phase(inventory);
        phase.result_preview = ItemStack::empty();
        let (output, effects) = plan.commit(&mut self.station)?;
        self.observe_effects_from(effects, inventory, outcome, phase)?;
        Ok(output)
    }
    fn may_place(&self, slot: usize, stack: &ItemStack) -> Result<bool> {
        if slot < 4 {
            self.station.may_place(slot, stack)
        } else {
            Ok(true)
        }
    }
    fn reset_quickcraft(&mut self) {
        self.quickcraft_status = 0;
        self.quickcraft_slots.clear();
    }
    fn drop_stack(
        &self,
        outcome: &mut SmithingActionOutcome,
        stack: ItemStack,
        inventory: &PlayerInventory,
    ) {
        if !stack.is_empty() {
            let index = outcome.drops.len();
            outcome.drops.push(DroppedStack {
                stack,
                around: false,
                trace_item: true,
            });
            outcome.events.push(SmithingMenuEvent::Drop {
                index,
                phase: self.phase(inventory),
            });
        }
    }
    fn may_pickup(&self, slot: usize) -> Result<bool> {
        if slot == RESULT_SLOT {
            self.station.may_pickup()
        } else {
            Ok(true)
        }
    }
    fn take_slot(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<ItemStack> {
        self.take_slot_with_cursor(slot, amount, maximum, None, inventory, outcome)
    }
    fn take_slot_with_cursor(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        cursor_before_on_take: Option<ItemStack>,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<ItemStack> {
        let source = self.slot(slot, inventory);
        if source.is_empty() || amount <= 0 || maximum <= 0 {
            return Ok(ItemStack::empty());
        }
        if slot == RESULT_SLOT {
            if maximum < source.count || !self.station.may_pickup()? {
                return Ok(ItemStack::empty());
            }
            let plan = self.station.prepare_take(SmithingTakeKind::Pickup)?;
            if let Some(cursor) = cursor_before_on_take {
                self.sync.set_carried(cursor);
            }
            return self.commit_take(plan, inventory, outcome);
        }
        let n = source.count.min(amount).min(maximum);
        // SimpleContainer.removeItem performs one setChanged before onTake.
        self.change_slot(slot, counted(&source, source.count - n), inventory, outcome)?;
        if self.slot(slot, inventory).is_empty() {
            // Slot.tryRemove installs EMPTY through setByPlayer before onTake.
            self.put_slot(slot, ItemStack::empty(), inventory, outcome)?;
        }
        if let Some(cursor) = cursor_before_on_take {
            self.sync.set_carried(cursor);
        }
        self.slot_changed(slot, inventory, outcome)?;
        Ok(counted(&source, n))
    }
    fn do_click(
        &mut self,
        slot: i32,
        button: i32,
        kind: ClickType,
        creative: bool,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        if kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, creative, inventory, outcome);
        }
        if self.quickcraft_status != 0 {
            self.reset_quickcraft();
            return Ok(());
        }
        match kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let carried = self.sync.carried().clone();
                    let n = if button == 0 {
                        carried.count
                    } else {
                        1.min(carried.count)
                    };
                    if button == 0 {
                        self.drop_stack(outcome, counted(&carried, n), inventory);
                        self.sync.set_carried(ItemStack::empty());
                    } else {
                        self.sync.set_carried(counted(&carried, carried.count - n));
                        self.drop_stack(outcome, counted(&carried, n), inventory);
                    }
                } else if slot >= 0 {
                    if kind == ClickType::Pickup {
                        self.pickup(slot as usize, button as i8, inventory, outcome)?;
                    } else if self.may_pickup(slot as usize)? {
                        self.quick_move(slot as usize, inventory, outcome)?;
                    }
                }
            }
            ClickType::Swap if slot >= 0 && ((0..9).contains(&button) || button == 40) => {
                self.swap(slot as usize, button, creative, inventory, outcome)?
            }
            ClickType::Clone if creative && slot >= 0 && self.sync.carried().is_empty() => {
                let source = self.slot(slot as usize, inventory);
                if !source.is_empty() {
                    reject_bundle(&source)?;
                    self.sync
                        .set_carried(counted(&source, source.max_stack_size()?));
                }
            }
            ClickType::Throw
                if slot >= 0
                    && self.sync.carried().is_empty()
                    && self.may_pickup(slot as usize)? =>
            {
                let source = self.slot(slot as usize, inventory);
                reject_bundle(&source)?;
                let amount = if button == 0 { 1 } else { source.count };
                let taken = self.take_slot(slot as usize, amount, i32::MAX, inventory, outcome)?;
                self.drop_stack(outcome, taken, inventory);
            }
            ClickType::PickupAll if slot >= 0 => {
                self.pickup_all(slot as usize, button, inventory, outcome)?
            }
            _ => {}
        }
        Ok(())
    }
    fn pickup(
        &mut self,
        slot: usize,
        button: i8,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        let source = self.slot(slot, inventory);
        let carried = self.sync.carried().clone();
        reject_bundle(&source)?;
        reject_bundle(&carried)?;
        if slot == RESULT_SLOT {
            if source.is_empty() || !self.station.may_pickup()? {
                return Ok(());
            }
            if !carried.is_empty()
                && (!carried.same_item_same_components(&source)?
                    || carried.count + source.count > limit(&carried)?)
            {
                return Ok(());
            }
            let plan = self.station.prepare_take(SmithingTakeKind::Pickup)?;
            self.sync.set_carried(counted(
                plan.output(),
                carried.count.max(0) + plan.output().count,
            ));
            self.commit_take(plan, inventory, outcome)?;
        } else if carried.is_empty() {
            let n = if button == 0 {
                source.count
            } else {
                (source.count + 1) / 2
            };
            self.take_slot_with_cursor(
                slot,
                n,
                i32::MAX,
                Some(counted(&source, n)),
                inventory,
                outcome,
            )?;
        } else if self.may_place(slot, &carried)? {
            if source.is_empty() || source.same_item_same_components(&carried)? {
                let max = limit(&carried)?;
                let n =
                    (if button == 0 { carried.count } else { 1 }).min(max - source.count.max(0));
                self.sync.set_carried(counted(&carried, carried.count - n));
                self.put_slot(
                    slot,
                    counted(&carried, source.count.max(0) + n),
                    inventory,
                    outcome,
                )?;
            } else if carried.count <= limit(&carried)? {
                self.sync.set_carried(source);
                self.put_slot(slot, carried, inventory, outcome)?;
            }
        }
        // cpu.doClick ends ordinary PICKUP with Slot.setChanged, even for a
        // rejected/no-op insertion. The result container's callback is a no-op.
        self.slot_changed(slot, inventory, outcome)?;
        Ok(())
    }
    fn move_to(
        &mut self,
        stack: &mut ItemStack,
        start: usize,
        end: usize,
        reverse: bool,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
        source_slot: usize,
    ) -> Result<bool> {
        let before = stack.count;
        let order = if reverse {
            (start..end).rev().collect::<Vec<_>>()
        } else {
            (start..end).collect()
        };
        if stack.is_stackable()? {
            for &slot in &order {
                let target = self.slot(slot, inventory);
                if !target.is_empty() && target.same_item_same_components(stack)? {
                    let amount = stack.count.min((limit(stack)? - target.count).max(0));
                    if amount > 0 {
                        *stack = counted(stack, stack.count - amount);
                        if source_slot > RESULT_SLOT {
                            inventory.set_slot_authoritative(source_slot + 5, stack.clone());
                        }
                        // moveItemStackTo merges by mutating the existing stack
                        // then Slot.setChanged, not by Slot.setByPlayer.
                        self.change_slot(
                            slot,
                            counted(&target, target.count + amount),
                            inventory,
                            outcome,
                        )?;
                        if slot > RESULT_SLOT {
                            self.player_changed_with_source(inventory, outcome, source_slot, stack);
                        }
                    }
                    if stack.is_empty() {
                        break;
                    }
                }
            }
        }
        if !stack.is_empty() {
            for slot in order {
                if self.slot(slot, inventory).is_empty() && self.may_place(slot, stack)? {
                    let n = stack.count.min(limit(stack)?);
                    let inserted = counted(stack, n);
                    *stack = counted(stack, stack.count - n);
                    if source_slot > RESULT_SLOT {
                        inventory.set_slot_authoritative(source_slot + 5, stack.clone());
                    }
                    if slot > RESULT_SLOT {
                        self.change_slot(slot, inserted, inventory, outcome)?;
                        self.player_changed_with_source(inventory, outcome, source_slot, stack);
                        self.player_changed_with_source(inventory, outcome, source_slot, stack);
                    } else {
                        self.put_slot(slot, inserted, inventory, outcome)?;
                        self.slot_changed(slot, inventory, outcome)?;
                    }
                    break;
                }
            }
        }
        Ok(before != stack.count)
    }
    fn quick_move(
        &mut self,
        slot: usize,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        let original = self.slot(slot, inventory);
        reject_bundle(&original)?;
        if original.is_empty() {
            return Ok(());
        }
        if slot == RESULT_SLOT {
            // The default input slots are capped at 99. Repetition therefore
            // terminates by consumption or full player inventory.
            while !self.station.result().is_empty()
                && self.station.result().item_id == original.item_id
            {
                let plan = self.station.prepare_take(SmithingTakeKind::QuickMove)?;
                let mut output = plan.output().clone();
                if !self.move_to(&mut output, 4, 40, true, inventory, outcome, slot)? {
                    break;
                }
                ensure!(
                    output.is_empty(),
                    "unsupported partial smithing output transfer"
                );
                self.commit_take(plan, inventory, outcome)?;
            }
        } else {
            loop {
                let source = self.slot(slot, inventory);
                if source.is_empty() {
                    break;
                }
                let mut moving = source.clone();
                if slot < RESULT_SLOT {
                    self.move_to(&mut moving, 4, 40, false, inventory, outcome, slot)?;
                } else if let Some(target) = self.station.quick_move_input_slot(&moving)? {
                    self.move_to(
                        &mut moving,
                        target,
                        RESULT_SLOT,
                        false,
                        inventory,
                        outcome,
                        slot,
                    )?;
                } else if slot < 31 {
                    self.move_to(&mut moving, 31, 40, false, inventory, outcome, slot)?;
                } else {
                    self.move_to(&mut moving, 4, 31, false, inventory, outcome, slot)?;
                }
                if moving.count == source.count {
                    break;
                }
                if moving.is_empty() {
                    self.put_slot(slot, moving, inventory, outcome)?;
                } else {
                    self.change_slot(slot, moving, inventory, outcome)?;
                    if slot > RESULT_SLOT {
                        self.slot_changed(slot, inventory, outcome)?;
                    }
                }
                self.slot_changed(slot, inventory, outcome)?; // ordinary Slot.onTake
                if self.slot(slot, inventory).item_id != source.item_id {
                    break;
                }
            }
        }
        Ok(())
    }
    fn swap(
        &mut self,
        slot: usize,
        button: i32,
        creative: bool,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        let player_slot = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        if slot >= 4 && slot + 5 == player_slot {
            let source = self.slot(slot, inventory);
            if source.is_empty() {
                return Ok(());
            }
            let maximum = limit(&source)?;
            if source.count > maximum {
                // Java's held/source variables alias the SAME stack. split
                // leaves only the remainder in onTake/Inventory.add, never the
                // original overfull count copied before that mutation.
                let mut remainder = counted(&source, source.count - maximum);
                self.put_slot(slot, counted(&source, maximum), inventory, outcome)?;
                self.slot_changed(slot, inventory, outcome)?;
                Self::add_to_inventory(&mut remainder, inventory, creative)?;
                self.drop_stack(outcome, remainder, inventory);
            } else {
                self.put_slot(slot, source, inventory, outcome)?;
                self.slot_changed(slot, inventory, outcome)?;
            }
            return Ok(());
        }
        let source = self.slot(slot, inventory);
        let held = inventory
            .slot(player_slot)
            .expect("actual hotbar/offhand")
            .clone();
        reject_bundle(&source)?;
        reject_bundle(&held)?;
        if source.is_empty() && held.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            if self.may_pickup(slot)? {
                // Source installs the hotbar/offhand output before onTake.
                inventory.set_slot_authoritative(player_slot, source.clone());
                if slot == RESULT_SLOT {
                    self.take_slot(slot, source.count, i32::MAX, inventory, outcome)?;
                } else {
                    self.put_slot(slot, ItemStack::empty(), inventory, outcome)?;
                    self.slot_changed(slot, inventory, outcome)?;
                }
            }
        } else if source.is_empty() {
            if self.may_place(slot, &held)? {
                let n = held.count.min(limit(&held)?);
                inventory.set_slot_authoritative(player_slot, counted(&held, held.count - n));
                self.put_slot(slot, counted(&held, n), inventory, outcome)?;
            }
        } else if self.may_pickup(slot)? && self.may_place(slot, &held)? {
            let maximum = limit(&held)?;
            if held.count > maximum {
                inventory.set_slot_authoritative(player_slot, counted(&held, held.count - maximum));
                self.put_slot(slot, counted(&held, maximum), inventory, outcome)?;
                self.slot_changed(slot, inventory, outcome)?;
                let mut old = source;
                Self::add_to_inventory(&mut old, inventory, creative)?;
                self.drop_stack(outcome, old, inventory);
            } else {
                inventory.set_slot_authoritative(player_slot, source);
                self.put_slot(slot, held, inventory, outcome)?;
                self.slot_changed(slot, inventory, outcome)?;
            }
        }
        Ok(())
    }
    fn add_to_inventory(
        stack: &mut ItemStack,
        inventory: &mut PlayerInventory,
        creative: bool,
    ) -> Result<()> {
        let order = (36..45).chain(9..36).collect::<Vec<_>>();
        if stack.is_damageable()? && stack.damage_value()? > 0 {
            if let Some(slot) = order
                .iter()
                .copied()
                .find(|i| inventory.slot(*i).unwrap().is_empty())
            {
                inventory.set_slot_authoritative(slot, std::mem::take(stack));
            } else if creative {
                *stack = ItemStack::empty();
            }
            return Ok(());
        }
        let mut merges = vec![36 + inventory.selected_hotbar(), 45];
        merges.extend(order.iter().copied());
        for slot in merges {
            let target = inventory.slot(slot).unwrap();
            if !target.is_empty()
                && target.is_stackable()?
                && target.same_item_same_components(stack)?
            {
                let n = stack.count.min((limit(target)? - target.count).max(0));
                if n > 0 {
                    inventory.set_slot_authoritative(slot, counted(target, target.count + n));
                    *stack = counted(stack, stack.count - n);
                }
                if stack.is_empty() {
                    return Ok(());
                }
            }
        }
        for slot in order {
            if inventory.slot(slot).unwrap().is_empty() {
                let n = stack.count.min(limit(stack)?);
                inventory.set_slot_authoritative(slot, counted(stack, n));
                *stack = counted(stack, stack.count - n);
                if stack.is_empty() {
                    return Ok(());
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
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        let carried = self.sync.carried().clone();
        reject_bundle(&carried)?;
        if carried.is_empty()
            || !self.slot(clicked, inventory).is_empty() && self.may_pickup(clicked)?
        {
            return Ok(());
        }
        let order = if button == 0 {
            (0..40).collect::<Vec<_>>()
        } else {
            (0..40).rev().collect()
        };
        for pass in 0..2 {
            for &slot in &order {
                let source = self.slot(slot, inventory);
                let carried = self.sync.carried().clone();
                if carried.count >= carried.max_stack_size()? {
                    break;
                }
                if slot == RESULT_SLOT
                    || source.is_empty()
                    || source.count > carried.max_stack_size()?
                    || !source.same_item_same_components(&carried)?
                    || !self.may_pickup(slot)?
                    || pass == 0 && source.count == source.max_stack_size()?
                {
                    continue;
                }
                let taken = self.take_slot(
                    slot,
                    source.count,
                    carried.max_stack_size()? - carried.count,
                    inventory,
                    outcome,
                )?;
                self.sync
                    .set_carried(counted(&carried, carried.count + taken.count));
            }
        }
        Ok(())
    }
    fn quickcraft(
        &mut self,
        slot: i32,
        button: i32,
        creative: bool,
        inventory: &mut PlayerInventory,
        outcome: &mut SmithingActionOutcome,
    ) -> Result<()> {
        let previous = self.quickcraft_status;
        self.quickcraft_status = (button & 3) as u8;
        if !(previous == 1 && self.quickcraft_status == 2) && previous != self.quickcraft_status {
            self.reset_quickcraft();
            return Ok(());
        }
        if self.sync.carried().is_empty() {
            self.reset_quickcraft();
            return Ok(());
        }
        reject_bundle(self.sync.carried())?;
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
            1 if slot >= 0 => {
                let slot = slot as usize;
                let carried = self.sync.carried();
                let source = self.slot(slot, inventory);
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
            1 => {}
            2 => {
                if self.quickcraft_slots.len() == 1 {
                    let slot = *self.quickcraft_slots.iter().next().unwrap();
                    self.reset_quickcraft();
                    return self.do_click(
                        slot as i32,
                        self.quickcraft_type as i32,
                        ClickType::Pickup,
                        creative,
                        inventory,
                        outcome,
                    );
                }
                let carried = self.sync.carried().clone();
                let mut left = carried.count;
                let slots = self.quickcraft_slots.iter().copied().collect::<Vec<_>>();
                let each = match self.quickcraft_type {
                    0 => carried.count / (slots.len().max(1) as i32),
                    1 => 1,
                    2 => carried.max_stack_size()?,
                    _ => 0,
                };
                for slot in slots {
                    let source = self.slot(slot, inventory);
                    if !self.may_place(slot, &carried)?
                        || !source.is_empty()
                            && (source.count > carried.max_stack_size()?
                                || !source.same_item_same_components(&carried)?)
                        || self.quickcraft_type != 2
                            && carried.count < self.quickcraft_slots.len() as i32
                    {
                        continue;
                    }
                    let count = (each + source.count).min(limit(&carried)?);
                    left -= count - source.count;
                    self.put_slot(slot, counted(&carried, count), inventory, outcome)?;
                }
                self.sync.set_carried(counted(&carried, left));
                self.reset_quickcraft();
            }
            _ => self.reset_quickcraft(),
        }
        Ok(())
    }
}
fn reject_bundle(stack: &ItemStack) -> Result<()> {
    ensure!(
        inventory::item_name(stack.item_id) != Some("bundle"),
        "bundle click override requires its actual feature/component owner"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::RecipeBook;
    use std::collections::BTreeMap;
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn click(
        menu: &mut SmithingMenu,
        inv: &mut PlayerInventory,
        slot: i16,
        kind: ClickType,
    ) -> Result<SmithingActionOutcome> {
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync().state_id(),
            slot,
            button: 0,
            kind,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        menu.prepare_click(&packet, inv, false)?.commit(menu, inv)
    }
    #[test]
    fn borrowed_inventory_shift_result_and_disconnected_close_flow() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("netherite_upgrade_smithing_template", 2));
        inv.set_slot_authoritative(10, item("diamond_sword", 1));
        inv.set_slot_authoritative(11, item("netherite_ingot", 3));
        let initialized = menu.initialize(&inv)?;
        assert!(initialized.iter().any(
            |e| matches!(e, SmithingMenuEvent::Sync {event:SyncEvent::Packet(p),..} if p.id == 0x13)
        ));
        click(&mut menu, &mut inv, 4, ClickType::QuickMove)?;
        click(&mut menu, &mut inv, 5, ClickType::QuickMove)?;
        click(&mut menu, &mut inv, 6, ClickType::QuickMove)?;
        assert_eq!(
            menu.station().result().item_id,
            item("netherite_sword", 1).item_id
        );
        assert!(inv.slot(9).unwrap().is_empty());
        // The output needs actual capacity; a rejected insertion cannot consume
        // the template/base/addition or trigger callbacks.
        let mut full = inv.clone();
        for slot in 9..45 {
            full.set_slot_authoritative(slot, item("stone", 64));
        }
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync().state_id(),
            slot: 3,
            button: 0,
            kind: ClickType::QuickMove,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        let no_capacity = menu.prepare_click(&packet, &full, false)?;
        assert!(no_capacity
            .outcome()
            .events
            .iter()
            .all(|e| !matches!(e, SmithingMenuEvent::Effect { .. })));
        assert_eq!(menu.station().inputs()[0].count, 2);
        drop(no_capacity);
        let taken = click(&mut menu, &mut inv, 3, ClickType::QuickMove)?;
        assert!(matches!(
            taken
                .events
                .iter()
                .find(|event| matches!(event, SmithingMenuEvent::Effect { .. }))
                .unwrap(),
            SmithingMenuEvent::Effect {
                effect: SmithingEffect::CraftedBy { amount: 0, .. },
                ..
            }
        ));
        assert_eq!(
            inv.slot(44).unwrap().item_id,
            item("netherite_sword", 1).item_id
        );
        assert!(menu.station().result().is_empty());
        click(&mut menu, &mut inv, 39, ClickType::Pickup)?;
        assert!(inv.slot(44).unwrap().is_empty());
        let close = menu
            .prepare_close(&inv, MenuCloseContext::DeadOrDisconnected)?
            .commit(&mut menu, &mut inv)?;
        assert_eq!(
            close
                .drops
                .iter()
                .map(|s| s.stack.count)
                .collect::<Vec<_>>(),
            [1, 1, 2]
        );
        assert_eq!(
            close.drops[0].stack.item_id,
            item("netherite_sword", 1).item_id
        );
        assert_eq!(
            close.drops[1].stack.item_id,
            item("netherite_upgrade_smithing_template", 1).item_id
        );
        assert_eq!(
            close.drops[2].stack.item_id,
            item("netherite_ingot", 1).item_id
        );
        assert!(menu.sync().carried().is_empty());
        assert!(menu.station().inputs().iter().all(ItemStack::is_empty));
        Ok(())
    }
    #[test]
    fn stale_inventory_plan_and_bundle_boundary_do_not_mutate_live_state() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("diamond_sword", 1));
        menu.initialize(&inv)?;
        let fresh_ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut different_menu =
            SmithingMenu::new(7, SmithingRecipeOrder::new(fresh_ids.clone(), fresh_ids)?)?;
        different_menu.initialize(&inv)?;
        let cross_owner = menu.prepare_close(&inv, MenuCloseContext::AliveConnected)?;
        assert!(cross_owner.validate(&different_menu, &inv).is_err());
        let old_phase = menu.prepare_close(&inv, MenuCloseContext::AliveConnected)?;
        menu.broadcast_changes(&inv)?; // even an unchanged later phase invalidates a prepared action
        assert!(old_phase.validate(&menu, &inv).is_err());
        let before = menu.clone();
        let mut packet = ContainerClick {
            window_id: 7,
            state_id: -1,
            slot: 4,
            button: 0,
            kind: ClickType::Pickup,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        let plan = menu.prepare_click(&packet, &inv, false)?;
        assert!(plan.outcome().events.iter().any(
            |e| matches!(e, SmithingMenuEvent::Sync {event:SyncEvent::Packet(p),..} if p.id == 0x13)
        ));
        assert_eq!(menu, before);
        inv.set_slot_authoritative(10, item("stone", 1));
        assert!(plan.commit(&mut menu, &mut inv).is_err());
        assert_eq!(menu, before);
        assert_eq!(inv.slot(9).unwrap().count, 1);
        packet.kind = ClickType::Pickup;
        inv.set_slot_authoritative(9, item("bundle", 1));
        assert!(menu.prepare_click(&packet, &inv, false).is_err());
        assert_eq!(menu, before);
        Ok(())
    }
    fn custom_click(
        menu: &mut SmithingMenu,
        inv: &mut PlayerInventory,
        slot: i16,
        button: i8,
        kind: ClickType,
        creative: bool,
    ) -> Result<SmithingActionOutcome> {
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync().state_id(),
            slot,
            button,
            kind,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        menu.prepare_click(&packet, inv, creative)?
            .commit(menu, inv)
    }
    #[test]
    fn drag_pickall_offhand_swap_creative_clone_and_result_throw_flow() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(30, item("stone", 99));
        menu.initialize(&inv)?;
        click(&mut menu, &mut inv, 25, ClickType::QuickMove)?;
        assert!(inv.slot(30).unwrap().is_empty());
        assert_eq!(inv.slot(36).unwrap().count, 64);
        assert_eq!(inv.slot(37).unwrap().count, 35);
        inv.set_slot_authoritative(36, ItemStack::empty());
        inv.set_slot_authoritative(37, ItemStack::empty());
        inv.set_slot_authoritative(9, item("netherite_upgrade_smithing_template", 6));
        click(&mut menu, &mut inv, 4, ClickType::Pickup)?;
        custom_click(&mut menu, &mut inv, -999, 0, ClickType::QuickCraft, false)?;
        custom_click(&mut menu, &mut inv, 0, 1, ClickType::QuickCraft, false)?;
        custom_click(&mut menu, &mut inv, 5, 1, ClickType::QuickCraft, false)?;
        custom_click(&mut menu, &mut inv, 3, 1, ClickType::QuickCraft, false)?; // result cannot receive dragged items
        custom_click(&mut menu, &mut inv, -999, 2, ClickType::QuickCraft, false)?;
        assert_eq!(menu.station().inputs()[0].count, 3);
        assert_eq!(inv.slot(10).unwrap().count, 3);
        assert!(menu.sync().carried().is_empty());
        click(&mut menu, &mut inv, 5, ClickType::Pickup)?;
        inv.set_slot_authoritative(11, item("netherite_upgrade_smithing_template", 2));
        custom_click(&mut menu, &mut inv, 5, 0, ClickType::PickupAll, false)?;
        assert_eq!(menu.sync().carried().count, 8);
        assert!(menu.station().inputs()[0].is_empty());
        assert!(inv.slot(11).unwrap().is_empty());
        let dropped = custom_click(&mut menu, &mut inv, -999, 0, ClickType::Pickup, false)?;
        assert_eq!(dropped.drops[0].stack.count, 8);
        inv.set_slot_authoritative(36, item("netherite_upgrade_smithing_template", 2));
        custom_click(&mut menu, &mut inv, 0, 0, ClickType::Swap, false)?;
        inv.set_slot_authoritative(45, item("netherite_upgrade_smithing_template", 1));
        custom_click(&mut menu, &mut inv, 0, 40, ClickType::Swap, false)?;
        assert_eq!(menu.station().inputs()[0].count, 1);
        assert_eq!(inv.slot(45).unwrap().count, 2);
        inv.set_slot_authoritative(10, item("diamond_sword", 1));
        inv.set_slot_authoritative(11, item("netherite_ingot", 1));
        click(&mut menu, &mut inv, 5, ClickType::QuickMove)?;
        click(&mut menu, &mut inv, 6, ClickType::QuickMove)?;
        custom_click(&mut menu, &mut inv, 3, 2, ClickType::Clone, false)?;
        assert!(menu.sync().carried().is_empty());
        let cloned = custom_click(&mut menu, &mut inv, 3, 2, ClickType::Clone, true)?;
        assert!(cloned
            .events
            .iter()
            .all(|e| !matches!(e, SmithingMenuEvent::Effect { .. })));
        assert_eq!(
            menu.sync().carried().item_id,
            item("netherite_sword", 1).item_id
        );
        assert_eq!(menu.station().inputs()[0].count, 1);
        custom_click(&mut menu, &mut inv, -999, 0, ClickType::Pickup, true)?;
        let swapped = custom_click(&mut menu, &mut inv, 3, 1, ClickType::Swap, false)?;
        assert_eq!(
            inv.slot(37).unwrap().item_id,
            item("netherite_sword", 1).item_id
        );
        assert!(matches!(
            &swapped.events[0],
            SmithingMenuEvent::Effect {
                effect: SmithingEffect::CraftedBy { amount: 1, .. },
                ..
            }
        ));
        assert!(menu.station().result().is_empty());
        inv.set_slot_authoritative(9, item("netherite_upgrade_smithing_template", 1));
        inv.set_slot_authoritative(10, item("diamond_sword", 1));
        inv.set_slot_authoritative(11, item("netherite_ingot", 1));
        for slot in 4..7 {
            click(&mut menu, &mut inv, slot, ClickType::QuickMove)?;
        }
        let thrown = custom_click(&mut menu, &mut inv, 3, 1, ClickType::Throw, false)?;
        let drop_event = thrown
            .events
            .iter()
            .position(|e| matches!(e, SmithingMenuEvent::Drop { index: 0, .. }))
            .unwrap();
        let world_event = thrown
            .events
            .iter()
            .position(|e| {
                matches!(
                    e,
                    SmithingMenuEvent::Effect {
                        effect: SmithingEffect::LevelEvent { event: 1044, .. },
                        ..
                    }
                )
            })
            .unwrap();
        assert!(world_event < drop_event);
        assert_eq!(
            thrown.drops[0].stack.item_id,
            item("netherite_sword", 1).item_id
        );
        assert!(menu.station().inputs().iter().all(ItemStack::is_empty));
        Ok(())
    }
    #[test]
    fn pickup_all_excludes_matching_derived_result() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        menu.station
            .set_input(0, item("netherite_upgrade_smithing_template", 1))?;
        let mut base = item("diamond_sword", 1);
        base.components.push(crate::inventory::Component {
            type_id: 1,
            data: vec![64],
        });
        menu.station.set_input(1, base)?;
        menu.station.set_input(2, item("netherite_ingot", 1))?;
        inv.set_slot_authoritative(9, counted(menu.station().result(), 2));
        menu.initialize(&inv)?;
        custom_click(&mut menu, &mut inv, 4, 1, ClickType::Pickup, false)?;
        assert_eq!(menu.sync().carried().count, 1);
        let gathered = custom_click(&mut menu, &mut inv, 5, 0, ClickType::PickupAll, false)?;
        assert_eq!(menu.sync().carried().count, 2);
        assert_eq!(menu.station().result().count, 1);
        assert!(menu.station().inputs().iter().all(|s| s.count == 1));
        assert!(gathered
            .events
            .iter()
            .all(|e| !matches!(e, SmithingMenuEvent::Effect { .. })));
        Ok(())
    }
    #[test]
    fn alive_close_interleaves_raw_insertions_and_overflow_drops_per_owned_stack() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        for slot in 9..45 {
            inv.set_slot_authoritative(slot, item("stone", 64));
        }
        inv.set_slot_authoritative(36, item("netherite_upgrade_smithing_template", 63));
        inv.set_slot_authoritative(37, item("netherite_ingot", 63));
        menu.station
            .set_input(0, item("netherite_upgrade_smithing_template", 3))?;
        menu.station.set_input(1, item("diamond_sword", 1))?;
        menu.station.set_input(2, item("netherite_ingot", 2))?;
        menu.sync
            .set_carried(item("netherite_upgrade_smithing_template", 2));
        let close = menu
            .prepare_close(&inv, MenuCloseContext::AliveConnected)?
            .commit(&mut menu, &mut inv)?;
        assert!(matches!(
            close.events.as_slice(),
            [
                SmithingMenuEvent::Sync {
                    event: SyncEvent::Packet(_),
                    ..
                },
                SmithingMenuEvent::Drop { index: 0, .. },
                SmithingMenuEvent::Drop { index: 1, .. },
                SmithingMenuEvent::Drop { index: 2, .. },
                SmithingMenuEvent::Sync {
                    event: SyncEvent::Packet(_),
                    ..
                },
                SmithingMenuEvent::Drop { index: 3, .. }
            ]
        ));
        assert_eq!(
            close
                .drops
                .iter()
                .map(|d| d.stack.count)
                .collect::<Vec<_>>(),
            [1, 3, 1, 1]
        );
        assert_eq!(
            close.drops[2].stack.item_id,
            item("diamond_sword", 1).item_id
        ); // never the netherite preview
        assert_eq!(inv.slot(36).unwrap().count, 64);
        assert_eq!(inv.slot(37).unwrap().count, 64);
        assert!(close.drops.iter().all(|d| !d.around && !d.trace_item));
        Ok(())
    }

    #[test]
    fn input_callbacks_broadcast_previous_preview_before_each_result_recreation() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        menu.station
            .set_input(0, item("netherite_upgrade_smithing_template", 2))?;
        menu.station.set_input(1, item("diamond_sword", 1))?;
        inv.set_slot_authoritative(9, item("netherite_ingot", 2));
        menu.initialize(&inv)?;
        let input = custom_click(&mut menu, &mut inv, 4, 0, ClickType::QuickMove, false)?;
        let observations = |events: &[SmithingMenuEvent]| {
            events
                .iter()
                .filter_map(|event| {
                    if let SmithingMenuEvent::Sync {
                        event:
                            SyncEvent::SlotListener {
                                menu_slot, stack, ..
                            },
                        ..
                    } = event
                    {
                        Some((*menu_slot, stack.count))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        // ItemStack.split empties the actual player source BEFORE input
        // Slot.set callbacks. First broadcast sees no result; the second sees
        // createResult's new derived preview. Player inventory is not doubled.
        assert_eq!(observations(&input.events), [(2, 2), (4, 0), (3, 1)]);
        assert!(inv.slot(9).unwrap().is_empty());
        let taken = custom_click(&mut menu, &mut inv, 3, 0, ClickType::Pickup, false)?;
        // The taken output was removed before onTake. Shrink0 broadcasts EMPTY
        // before recreating a result, shrink1 broadcasts that previous result
        // even though the base just became empty, and shrink2 sees EMPTY again.
        assert_eq!(
            observations(&taken.events),
            [(0, 1), (3, 0), (1, 0), (3, 1), (2, 1), (3, 0)]
        );
        let award = taken
            .events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    SmithingMenuEvent::Effect {
                        effect: SmithingEffect::AwardRecipe { .. },
                        ..
                    }
                )
            })
            .unwrap();
        let first_listener = taken
            .events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    SmithingMenuEvent::Sync {
                        event: SyncEvent::SlotListener { .. },
                        ..
                    }
                )
            })
            .unwrap();
        let last_listener = taken
            .events
            .iter()
            .rposition(|event| {
                matches!(
                    event,
                    SmithingMenuEvent::Sync {
                        event: SyncEvent::SlotListener { .. },
                        ..
                    }
                )
            })
            .unwrap();
        let level_event = taken
            .events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    SmithingMenuEvent::Effect {
                        effect: SmithingEffect::LevelEvent { event: 1044, .. },
                        ..
                    }
                )
            })
            .unwrap();
        let first_packet = taken
            .events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    SmithingMenuEvent::Sync {
                        event: SyncEvent::Packet(_),
                        ..
                    }
                )
            })
            .unwrap();
        assert!(
            award < first_listener && last_listener < level_event && level_event < first_packet
        );
        assert_eq!(menu.sync().carried(), &item("netherite_sword", 1));
        assert_eq!(menu.station().inputs()[0].count, 1);
        assert!(menu.station().inputs()[1].is_empty());
        assert_eq!(menu.station().inputs()[2].count, 1);
        assert!(menu.station().result().is_empty());
        assert!(taken.drops.is_empty());
        Ok(())
    }
    #[test]
    fn signed_safe_insert_player_counter_phases_and_shared_self_swap_match_source() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("stone", 4));
        inv.set_carried_authoritative(item("emerald", 2));
        menu.initialize(&inv)?;
        let taken = click(&mut menu, &mut inv, 4, ClickType::Pickup)?;
        let phases = taken
            .events
            .iter()
            .filter_map(|event| match event {
                SmithingMenuEvent::PlayerInventoryChanged { phase } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(phases.len(), 3);
        assert!(phases[0].carried.is_empty());
        assert_eq!(phases[1].carried.count, 4);
        assert_eq!(phases[2].carried.count, 4);
        assert!(phases
            .iter()
            .all(|phase| phase.player_inventory.slot(9).unwrap().is_empty()
                && phase.player_inventory.carried().count == 2));
        let placed = click(&mut menu, &mut inv, 5, ClickType::Pickup)?;
        assert_eq!(
            placed
                .events
                .iter()
                .filter(|event| matches!(event, SmithingMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        inv.set_slot_authoritative(36, item("stone", 3));
        menu.broadcast_changes(&inv)?;
        let same = custom_click(&mut menu, &mut inv, 31, 0, ClickType::Swap, false)?;
        assert_eq!(
            same.events
                .iter()
                .filter(|event| matches!(event, SmithingMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        assert_eq!(inv.slot(36).unwrap().count, 3);
        // An actual owner can already hold an overfull stack. The creative
        // packet rejects this assignment, so install the fixture authoritatively.
        // Source safeInsert deliberately retains negative room.
        inv.set_slot_authoritative(36, item("stone", 99));
        inv.set_slot_authoritative(37, item("stone", 1));
        menu.broadcast_changes(&inv)?;
        click(&mut menu, &mut inv, 32, ClickType::Pickup)?;
        assert_eq!(menu.sync().carried().count, 1);
        let normalized = click(&mut menu, &mut inv, 31, ClickType::Pickup)?;
        assert_eq!(inv.slot(36).unwrap().count, 64);
        assert_eq!(menu.sync().carried().count, 36);
        let normalized_phases = normalized
            .events
            .iter()
            .filter_map(|event| match event {
                SmithingMenuEvent::PlayerInventoryChanged { phase } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(normalized_phases.len(), 2);
        assert!(normalized_phases.iter().all(|phase| phase
            .player_inventory
            .slot(36)
            .unwrap()
            .count
            == 64
            && phase.carried.count == 36));
        let full = click(&mut menu, &mut inv, 31, ClickType::Pickup)?;
        let full_phases = full
            .events
            .iter()
            .filter_map(|event| match event {
                SmithingMenuEvent::PlayerInventoryChanged { phase } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(full_phases.len(), 2);
        assert!(full_phases
            .iter()
            .all(|phase| phase.player_inventory.slot(36).unwrap().count == 64
                && phase.carried.count == 36));
        let cursor_drop = custom_click(&mut menu, &mut inv, -999, 0, ClickType::Pickup, false)?;
        assert_eq!(cursor_drop.drops[0].stack.count, 36);
        inv.set_slot_authoritative(36, item("stone", 99));
        inv.set_slot_authoritative(37, ItemStack::empty());
        inv.set_slot_authoritative(10, ItemStack::empty());
        menu.broadcast_changes(&inv)?;
        let aliased = custom_click(&mut menu, &mut inv, 31, 0, ClickType::Swap, true)?;
        assert_eq!(inv.slot(36).unwrap().count, 64);
        assert_eq!(inv.slot(37).unwrap().count, 35);
        assert_eq!(
            inv.slot(36).unwrap().count + inv.slot(37).unwrap().count,
            99
        );
        assert!(aliased.drops.is_empty());
        let alias_phases = aliased
            .events
            .iter()
            .filter_map(|event| match event {
                SmithingMenuEvent::PlayerInventoryChanged { phase } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(alias_phases.len(), 2);
        assert!(alias_phases
            .iter()
            .all(|phase| phase.player_inventory.slot(36).unwrap().count == 64
                && phase.player_inventory.slot(37).unwrap().is_empty()));
        Ok(())
    }
    #[test]
    fn repeated_result_transfer_callbacks_observe_each_actual_inventory_phase() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        inv.set_carried_authoritative(item("emerald", 2));
        menu.station
            .set_input(0, item("netherite_upgrade_smithing_template", 2))?;
        let mut swords = item("diamond_sword", 2);
        swords.components.push(crate::inventory::Component {
            type_id: 1,
            data: vec![2],
        });
        menu.station.set_input(1, swords)?;
        menu.station.set_input(2, item("netherite_ingot", 2))?;
        menu.initialize(&inv)?;
        let moved = click(&mut menu, &mut inv, 3, ClickType::QuickMove)?;
        let crafted = moved
            .events
            .iter()
            .filter_map(|event| match event {
                SmithingMenuEvent::Effect {
                    effect: SmithingEffect::CraftedBy { amount: 0, .. },
                    phase,
                } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(crafted.len(), 2);
        assert_eq!(crafted[0].inputs[0].count, 2);
        assert_eq!(crafted[1].inputs[0].count, 1);
        assert_eq!(crafted[0].player_inventory.slot(44).unwrap().count, 1);
        assert_eq!(crafted[1].player_inventory.slot(44).unwrap().count, 2);
        assert!(crafted.iter().all(|phase| phase.result_preview.is_empty()
            && phase.carried.is_empty()
            && phase.player_inventory.carried().count == 2));
        let counters = moved
            .events
            .iter()
            .filter_map(|event| match event {
                SmithingMenuEvent::PlayerInventoryChanged { phase } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(counters.len(), 3);
        assert!(counters.iter().all(|phase| phase.result_preview.is_empty()));
        assert!(menu.station().inputs().iter().all(ItemStack::is_empty));
        assert_eq!(inv.slot(44).unwrap().count, 2);
        Ok(())
    }
    #[test]
    fn close_callback_phases_retain_later_grid_inputs_and_separate_returned_items() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        for slot in 9..45 {
            inv.set_slot_authoritative(slot, item("stone", 64));
        }
        inv.set_slot_authoritative(36, item("netherite_upgrade_smithing_template", 63));
        inv.set_slot_authoritative(37, item("netherite_ingot", 63));
        inv.set_carried_authoritative(item("emerald", 2));
        menu.station
            .set_input(0, item("netherite_upgrade_smithing_template", 3))?;
        menu.station.set_input(1, item("diamond_sword", 1))?;
        menu.station.set_input(2, item("netherite_ingot", 2))?;
        menu.sync
            .set_carried(item("netherite_upgrade_smithing_template", 2));
        let closed = menu
            .prepare_close(&inv, MenuCloseContext::AliveConnected)?
            .commit(&mut menu, &mut inv)?;
        let phases = closed
            .events
            .iter()
            .map(|event| match event {
                SmithingMenuEvent::Sync { phase, .. } | SmithingMenuEvent::Drop { phase, .. } => {
                    phase
                }
                _ => panic!("unexpected close owner callback"),
            })
            .collect::<Vec<_>>();
        assert_eq!(phases.len(), 6);
        assert_eq!(phases[0].carried.count, 1);
        assert_eq!(phases[1].carried.count, 1);
        assert_eq!(
            phases[1].inputs.iter().map(|s| s.count).collect::<Vec<_>>(),
            [3, 1, 2]
        );
        assert_eq!(
            phases[2].inputs.iter().map(|s| s.count).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(phases[2].carried.is_empty());
        assert_eq!(
            phases[3].inputs.iter().map(|s| s.count).collect::<Vec<_>>(),
            [0, 0, 2]
        );
        assert!(phases[4].inputs.iter().all(ItemStack::is_empty));
        assert!(phases[4].carried.is_empty());
        assert!(phases[5].carried.is_empty());
        assert!(phases
            .iter()
            .all(|phase| phase.player_inventory.carried().count == 2
                && phase.result_preview.item_id == item("netherite_sword", 1).item_id));
        assert_eq!(
            closed
                .drops
                .iter()
                .map(|drop| drop.stack.count)
                .collect::<Vec<_>>(),
            [1, 3, 1, 1]
        );
        Ok(())
    }
    #[test]
    fn outside_whole_cursor_drop_precedes_clear_but_single_drop_observes_split() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        let mut inv = PlayerInventory::default();
        menu.initialize(&inv)?;
        menu.sync.set_carried(item("stone", 4));
        let single = custom_click(&mut menu, &mut inv, -999, 1, ClickType::Pickup, false)?;
        if let SmithingMenuEvent::Drop { phase, .. } = &single.events[0] {
            assert_eq!(phase.carried.count, 3);
        } else {
            panic!("single drop event missing");
        }
        let all = custom_click(&mut menu, &mut inv, -999, 0, ClickType::Pickup, false)?;
        if let SmithingMenuEvent::Drop { phase, .. } = &all.events[0] {
            assert_eq!(phase.carried.count, 3);
        } else {
            panic!("whole drop event missing");
        }
        assert!(menu.sync().carried().is_empty());
        assert_eq!(single.drops[0].stack.count + all.drops[0].stack.count, 4);
        Ok(())
    }

    #[test]
    fn full_state_resync_preserves_retained_owners_and_invalidates_old_plan() -> Result<()> {
        let ids = vec!["minecraft:netherite_sword_smithing".to_owned()];
        let mut menu = SmithingMenu::new(7, SmithingRecipeOrder::new(ids.clone(), ids)?)?;
        menu.station
            .set_input(0, item("netherite_upgrade_smithing_template", 2))?;
        menu.station.set_input(1, item("diamond_sword", 1))?;
        menu.station.set_input(2, item("netherite_ingot", 3))?;
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("stone", 4));
        menu.initialize(&inv)?;
        click(&mut menu, &mut inv, 4, ClickType::Pickup)?;
        custom_click(&mut menu, &mut inv, -999, 0, ClickType::QuickCraft, false)?;
        custom_click(&mut menu, &mut inv, 5, 1, ClickType::QuickCraft, false)?;
        let pending = menu.prepare_close(&inv, MenuCloseContext::AliveConnected)?;
        let retained = menu.clone();
        let inventory = inv.clone();
        let events: Vec<SmithingMenuEvent> = menu.broadcast_full_state(&inv)?;
        assert_eq!(menu.instance, retained.instance);
        assert_eq!(menu.station, retained.station);
        assert_eq!(menu.sync.carried(), &item("stone", 4));
        assert_eq!(menu.sync.carried(), retained.sync.carried());
        assert_eq!(menu.quickcraft_status, retained.quickcraft_status);
        assert_eq!(menu.quickcraft_type, retained.quickcraft_type);
        assert_eq!(menu.quickcraft_slots, retained.quickcraft_slots);
        assert_eq!(menu.revision, retained.revision + 1);
        assert_eq!(menu.sync.state_id(), (retained.sync.state_id() + 1) & 32767);
        let mut content = None;
        for event in &events {
            let SmithingMenuEvent::Sync { event, phase } = event else {
                panic!("full resync emitted an authoritative callback");
            };
            assert_eq!(phase.inputs, *retained.station.inputs());
            assert_eq!(phase.result_preview, *retained.station.result());
            assert_eq!(&phase.carried, retained.sync.carried());
            assert!(same_inventory(&phase.player_inventory, &inventory));
            if let SyncEvent::Packet(packet) = event {
                if packet.id == 0x13 {
                    content = Some(packet);
                }
            }
        }
        let packet = content.expect("retained smithing content packet");
        let mut reader = crate::protocol::Reader::new(&packet.payload);
        assert_eq!(reader.u8()?, 7);
        assert_eq!(reader.varint()?, menu.sync.state_id());
        assert_eq!(reader.varint()?, 40);
        let top = retained.top();
        for slot in 0..40 {
            let (stack, used) = ItemStack::decode(reader.remaining())?;
            reader.bytes(used)?;
            let expected = if slot < 4 {
                &top[slot]
            } else {
                inventory.slot(slot + 5).unwrap()
            };
            assert_eq!(&stack, expected);
        }
        assert_eq!(
            ItemStack::decode_exact(reader.remaining())?,
            item("stone", 4)
        );
        assert!(pending.commit(&mut menu, &mut inv).is_err());
        assert!(same_inventory(&inv, &inventory));
        assert_eq!(menu.station, retained.station);
        assert!(menu.broadcast_changes(&inv)?.is_empty());
        Ok(())
    }
}
