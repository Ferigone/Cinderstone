//! Bounded Java 1.21 smithing menu adapter: PICKUP and QUICK_MOVE.
//!
//! Player inventory is borrowed; inputs/cursor remain in this menu. Preparing
//! an action changes no live owner. The host must stage the recorded actual
//! player/item/world callbacks and drops, commit the one-shot plan, then publish
//! its ordered listener/packet events. Other click modes fail before mutation.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    workstations::{
        menu::{MenuSync, SyncEvent, WorkstationAccess, WorkstationKind},
        smithing::{
            SmithingEffect, SmithingRecipeOrder, SmithingState, SmithingTakeKind, RESULT_SLOT,
        },
    },
};
use anyhow::{bail, ensure, Result};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmithingMenu {
    station: SmithingState,
    sync: MenuSync,
    // This identifies only this temporary UI owner, never a block/world/player.
    instance: u64,
    revision: u64,
}
#[derive(Clone, Debug)]
pub enum SmithingMenuEvent {
    Effect(SmithingEffect),
    Sync(SyncEvent),
}
#[derive(Debug, Default)]
pub struct SmithingActionOutcome {
    /// One stream: actual smithing callbacks precede final menu sync. The
    /// source input/result setters do not broadcast intermediate changes.
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
    pub fn initialize(&mut self, inventory: &PlayerInventory) -> Result<Vec<SyncEvent>> {
        let mut next = self.clone();
        let events = next.sync.initialize(&next.top(), inventory, &[])?;
        next.advance_revision()?;
        *self = next;
        Ok(events)
    }
    pub fn broadcast_changes(&mut self, inventory: &PlayerInventory) -> Result<Vec<SyncEvent>> {
        let mut next = self.clone();
        let events = next.sync.broadcast_changes(&next.top(), inventory, &[])?;
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
    ) -> Result<SmithingActionPlan> {
        ensure!(
            click.window_id == self.sync.window_id(),
            "wrong smithing window"
        );
        ensure!(click.slot < 40, "smithing slot outside menu");
        ensure!(
            matches!(click.kind, ClickType::Pickup | ClickType::QuickMove),
            "smithing click mode is not integrated yet"
        );
        ensure!(
            click.button == 0 || click.button == 1,
            "unsupported smithing click button"
        );
        let mut next = self.clone();
        let mut inv = inventory.clone();
        let mut outcome = SmithingActionOutcome::default();
        let state_before = next.sync.state_id();
        next.sync.suppress_remote_updates(true);
        if click.slot == -999 {
            let carried = next.sync.carried().clone();
            if !carried.is_empty() {
                let n = if click.button == 0 { carried.count } else { 1 };
                outcome.drops.push(DroppedStack {
                    stack: counted(&carried, n),
                    around: false,
                    trace_item: true,
                });
                next.sync.set_carried(counted(&carried, carried.count - n));
            }
        } else if (0..40).contains(&click.slot) {
            let slot = click.slot as usize;
            if click.kind == ClickType::Pickup {
                next.pickup(slot, click.button, &mut inv, &mut outcome)?;
            } else {
                next.quick_move(slot, &mut inv, &mut outcome)?;
            }
        }
        outcome.events.extend(
            next.sync
                .finish_click(click, state_before, &next.top(), &inv, &[])?
                .into_iter()
                .map(SmithingMenuEvent::Sync),
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
        let mut inputs = next.station.drain_inputs_on_close();
        let closed = next.sync.close_inputs(&mut inputs, &mut inv, context)?;
        let outcome = SmithingActionOutcome {
            events: closed
                .packets
                .into_iter()
                .map(|p| SmithingMenuEvent::Sync(SyncEvent::Packet(p)))
                .collect(),
            drops: closed.drops,
        };
        next.advance_revision()?;
        Ok(SmithingActionPlan {
            before: self.clone(),
            before_inventory: inventory.clone(),
            next,
            next_inventory: inv,
            outcome,
        })
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
    ) -> Result<()> {
        if slot < 3 {
            self.station.set_input(slot, stack)?;
        } else if slot > RESULT_SLOT {
            inventory.set_slot_authoritative(slot + 5, stack);
        } else {
            bail!("result contents are derived");
        }
        Ok(())
    }
    fn may_place(&self, slot: usize, stack: &ItemStack) -> Result<bool> {
        if slot < 4 {
            self.station.may_place(slot, stack)
        } else {
            Ok(true)
        }
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
            let (output, effects) = self
                .station
                .prepare_take(SmithingTakeKind::Pickup)?
                .commit(&mut self.station)?;
            self.sync
                .set_carried(counted(&output, carried.count.max(0) + output.count));
            outcome
                .events
                .extend(effects.into_iter().map(SmithingMenuEvent::Effect));
        } else if carried.is_empty() {
            let n = if button == 0 {
                source.count
            } else {
                (source.count + 1) / 2
            };
            self.put_slot(slot, counted(&source, source.count - n), inventory)?;
            self.sync.set_carried(counted(&source, n));
        } else if self.may_place(slot, &carried)? {
            if source.is_empty() || source.same_item_same_components(&carried)? {
                let max = limit(&carried)?;
                let n = (if button == 0 { carried.count } else { 1 })
                    .min((max - source.count.max(0)).max(0));
                self.put_slot(slot, counted(&carried, source.count.max(0) + n), inventory)?;
                self.sync.set_carried(counted(&carried, carried.count - n));
            } else if carried.count <= limit(&carried)? {
                self.put_slot(slot, carried, inventory)?;
                self.sync.set_carried(source);
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
        inventory: &mut PlayerInventory,
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
                        self.put_slot(slot, counted(&target, target.count + amount), inventory)?;
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
                if self.slot(slot, inventory).is_empty() && self.may_place(slot, stack)? {
                    let n = stack.count.min(limit(stack)?);
                    self.put_slot(slot, counted(stack, n), inventory)?;
                    *stack = counted(stack, stack.count - n);
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
                if !self.move_to(&mut output, 4, 40, true, inventory)? {
                    break;
                }
                ensure!(
                    output.is_empty(),
                    "unsupported partial smithing output transfer"
                );
                let (_, effects) = plan.commit(&mut self.station)?;
                outcome
                    .events
                    .extend(effects.into_iter().map(SmithingMenuEvent::Effect));
            }
        } else {
            let mut moving = original;
            if slot < RESULT_SLOT {
                self.move_to(&mut moving, 4, 40, false, inventory)?;
            } else if let Some(target) = self.station.quick_move_input_slot(&moving)? {
                self.move_to(&mut moving, target, RESULT_SLOT, false, inventory)?;
            } else if slot < 31 {
                self.move_to(&mut moving, 31, 40, false, inventory)?;
            } else {
                self.move_to(&mut moving, 4, 31, false, inventory)?;
            }
            self.put_slot(slot, moving, inventory)?;
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
        menu.prepare_click(&packet, inv)?.commit(menu, inv)
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
        assert!(initialized
            .iter()
            .any(|e| matches!(e, SyncEvent::Packet(p) if p.id == 0x13)));
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
        let no_capacity = menu.prepare_click(&packet, &full)?;
        assert!(no_capacity
            .outcome()
            .events
            .iter()
            .all(|e| !matches!(e, SmithingMenuEvent::Effect(_))));
        assert_eq!(menu.station().inputs()[0].count, 2);
        drop(no_capacity);
        let taken = click(&mut menu, &mut inv, 3, ClickType::QuickMove)?;
        assert!(matches!(
            &taken.events[0],
            SmithingMenuEvent::Effect(SmithingEffect::CraftedBy { amount: 0, .. })
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
    fn stale_inventory_plan_and_unsupported_mode_do_not_mutate_live_state() -> Result<()> {
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
        let plan = menu.prepare_click(&packet, &inv)?;
        assert!(plan
            .outcome()
            .events
            .iter()
            .any(|e| matches!(e, SmithingMenuEvent::Sync(SyncEvent::Packet(p)) if p.id == 0x13)));
        assert_eq!(menu, before);
        inv.set_slot_authoritative(10, item("stone", 1));
        assert!(plan.commit(&mut menu, &mut inv).is_err());
        assert_eq!(menu, before);
        assert_eq!(inv.slot(9).unwrap().count, 1);
        packet.kind = ClickType::Swap;
        assert!(menu.prepare_click(&packet, &inv).is_err());
        assert_eq!(menu, before);
        Ok(())
    }
}
