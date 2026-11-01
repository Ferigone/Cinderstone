//! Native Java 1.21 MerchantMenu over real staged merchant/player owners.
//!
//! Each prepare method uses a PRIVATE host transaction. Host callbacks run in
//! source order and see phase observations of the same canonical owners.
//! Commit publishes that stage once under its owner lock, then installs the
//! checked UI/inventory snapshots. Main dispatch and offers transport are separate.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    protocol::Reader,
    workstations::{
        menu::{MenuRemovalEvent, MenuSync, SyncEvent, WorkstationKind},
        merchant::{ItemCost, MerchantStage, MerchantState, MENU_SLOTS},
    },
};
use anyhow::{bail, ensure, Result};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug)]
pub enum MerchantMenuAction {
    Initialize,
    Broadcast,
    SlotsChanged,
    Click(ClickType),
    SelectTrade(i32),
    Close(MenuCloseContext),
}
/// Read-only callback arguments, never another owned result/inventory.
#[derive(Clone, Debug)]
pub struct MerchantPhase<B, K> {
    pub trade: MerchantState<B, K>,
    pub inventory: PlayerInventory,
    pub carried: ItemStack,
    pub returning: Option<ItemStack>,
}
/// Implement on a private transaction of the ACTUAL merchant, offer objects,
/// trading player, inventory, Item/RNG, advancement/stat and output owners.
/// No callback has a no-op/default implementation. Observe binds read-only
/// source-phase facts for subsequent callbacks; it must not create live copies.
/// publish (inherited) validates all real owners atomically; failure publishes
/// nothing. A common lock covers publication and native snapshot installation.
/// Reentrant callbacks and rebinding an old UI to a replacement owner are unsupported.
pub trait MerchantMenuHost: MerchantStage {
    fn preflight_menu_action(&mut self, action: MerchantMenuAction) -> Result<()>;
    /// Certify this actual callback/criteria context cannot mutate the menu,
    /// its inventory/cursor or a crafted stack behind this controller. Such
    /// callbacks require a direct reentrant owner bridge and must reject here.
    fn callbacks_preserve_menu_owners(&self) -> Result<bool>;
    fn observe_menu_phase(
        &mut self,
        phase: MerchantPhase<Self::Binding, Self::OfferKey>,
    ) -> Result<()>;
    fn stage_sync(&mut self, event: SyncEvent) -> Result<()>;
    fn inventory_set_changed(&mut self) -> Result<()>;
    fn stage_drop(&mut self, owned: DroppedStack) -> Result<()>;
    /// Actual Inventory.add on this same private canonical inventory; retain its
    /// remainder/creative behavior. This is not placeItemBackInInventory.
    fn inventory_add(
        &mut self,
        inventory: &mut PlayerInventory,
        stack: &mut ItemStack,
        creative: bool,
    ) -> Result<()>;
    /// Resolve actual component/reference identity or explicitly reject it.
    /// In particular wire equality cannot certify custom FoodProperties stacks,
    /// and an Item.onCraftedBy that mutates its output requires a direct bridge.
    fn require_stack_context(&self, stack: &ItemStack) -> Result<()>;
    /// Actual Slot-identity HashSet encounter order, exactly this permutation.
    fn quickcraft_iteration_order(&self, selected: &[usize]) -> Result<Vec<usize>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MerchantMenu<B, K> {
    state: MerchantState<B, K>,
    sync: MenuSync,
    instance: u64,
    revision: u64,
    active: bool,
    drag_status: u8,
    drag_type: u8,
    drag_slots: BTreeSet<usize>,
}
#[derive(Debug)]
pub struct MerchantMenuPlan<B, K, T> {
    before: MerchantMenu<B, K>,
    inventory_before: PlayerInventory,
    after: MerchantMenu<B, K>,
    inventory_after: PlayerInventory,
    stage_key: T,
}
impl<B: Clone + Eq + Debug, K: Clone + Eq + Debug, T: Eq> MerchantMenuPlan<B, K, T> {
    pub fn next_menu(&self) -> &MerchantMenu<B, K> {
        &self.after
    }
    pub fn next_inventory(&self) -> &PlayerInventory {
        &self.inventory_after
    }
    pub fn validate<H: MerchantMenuHost<Binding = B, OfferKey = K, StageKey = T>>(
        &self,
        menu: &MerchantMenu<B, K>,
        inventory: &PlayerInventory,
        host: &H,
    ) -> Result<()> {
        ensure!(
            *menu == self.before && same_inventory(inventory, &self.inventory_before),
            "stale merchant UI/inventory plan"
        );
        ensure!(
            host.binding() == self.before.state_binding() && host.stage_key() == self.stage_key,
            "merchant owner or private transaction changed"
        );
        ensure!(
            host.callbacks_preserve_menu_owners()?,
            "merchant reentrant callback context changed"
        );
        Ok(())
    }
    /// Effects, drops and sync are already PRIVATE in host; publish them once.
    /// Returned preview/phase observations must never be separately published.
    pub fn commit<H: MerchantMenuHost<Binding = B, OfferKey = K, StageKey = T>>(
        self,
        menu: &mut MerchantMenu<B, K>,
        inventory: &mut PlayerInventory,
        host: &mut H,
    ) -> Result<()> {
        self.validate(menu, inventory, host)?;
        host.publish()?;
        *menu = self.after;
        *inventory = self.inventory_after;
        Ok(())
    }
}
fn same_inventory(a: &PlayerInventory, b: &PlayerInventory) -> bool {
    a.slots() == b.slots()
        && a.carried() == b.carried()
        && a.selected_hotbar() == b.selected_hotbar()
        && a.state_id() == b.state_id()
}
fn counted(stack: &ItemStack, count: i32) -> ItemStack {
    if stack.is_empty() || count <= 0 {
        ItemStack::empty()
    } else {
        let mut stack = stack.clone();
        stack.count = count;
        stack
    }
}
fn maximum(stack: &ItemStack) -> Result<i32> {
    let max = stack.max_stack_size()?.min(99);
    ensure!((1..=99).contains(&max), "invalid merchant slot maximum");
    Ok(max)
}
fn check_stack<H: MerchantMenuHost>(host: &H, stack: &ItemStack) -> Result<()> {
    stack.encode_canonical()?;
    ensure!(
        inventory::item_name(stack.item_id) != Some("bundle"),
        "bundle click override requires its actual feature/component owner"
    );
    host.require_stack_context(stack)
}
fn phase<B: Clone, K: Clone>(
    state: &MerchantState<B, K>,
    inv: &PlayerInventory,
    carried: &ItemStack,
) -> MerchantPhase<B, K> {
    MerchantPhase {
        trade: state.clone(),
        inventory: inv.clone(),
        carried: carried.clone(),
        returning: None,
    }
}
impl<B: Clone + Eq + Debug, K: Clone + Eq + Debug> MerchantMenu<B, K> {
    pub fn new(window_id: u8, actual_binding: B) -> Result<Self> {
        static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
        let instance = NEXT_INSTANCE
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| anyhow::anyhow!("merchant UI identity exhausted"))?;
        Ok(Self {
            state: MerchantState::new(actual_binding),
            sync: MenuSync::new(window_id, WorkstationKind::Merchant)?,
            instance,
            revision: 0,
            active: true,
            drag_status: 0,
            drag_type: 0,
            drag_slots: BTreeSet::new(),
        })
    }
    fn state_binding(&self) -> B {
        self.state.menu_binding()
    }
    pub fn state(&self) -> &MerchantState<B, K> {
        &self.state
    }
    pub fn sync(&self) -> &MenuSync {
        &self.sync
    }
    pub fn carried(&self) -> &ItemStack {
        self.sync.carried()
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn still_valid<H: MerchantMenuHost<Binding = B, OfferKey = K>>(
        &self,
        host: &H,
    ) -> Result<bool> {
        Ok(self.active && self.state.still_valid(host)?)
    }
    pub fn inventory_snapshot(&self, inventory: &PlayerInventory) -> PlayerInventory {
        let mut snapshot = inventory.clone();
        snapshot.set_carried_authoritative(self.carried().clone());
        snapshot
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    fn top(&self) -> [ItemStack; 3] {
        [
            self.state.payments()[0].clone(),
            self.state.payments()[1].clone(),
            self.state.preview().clone(),
        ]
    }
    fn advance(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("merchant UI revision exhausted"))?;
        Ok(())
    }
    fn reset_drag(&mut self) {
        self.drag_status = 0;
        self.drag_slots.clear();
    }
    fn action<'a, H: MerchantMenuHost<Binding = B, OfferKey = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &'a mut H,
        action: MerchantMenuAction,
    ) -> Result<Engine<'a, B, K, H>> {
        ensure!(
            self.active && host.binding() == self.state_binding(),
            "inactive/rebound merchant UI"
        );
        ensure!(
            host.callbacks_preserve_menu_owners()?,
            "unsupported reentrant merchant callback context"
        );
        // ServerPlayer's phase broadcasts BEFORE testing stillValid/closing.
        // Only packet actions add the actual handler's validity guard.
        if matches!(
            action,
            MerchantMenuAction::Click(_) | MerchantMenuAction::SelectTrade(_)
        ) {
            ensure!(
                self.still_valid(host)?,
                "actual merchant trading player is no longer valid"
            );
        }
        for stack in inventory.slots().iter().chain([
            inventory.carried(),
            self.carried(),
            &self.state.payments()[0],
            &self.state.payments()[1],
            self.state.preview(),
        ]) {
            check_stack(host, stack)?;
        }
        let stage_key = host.stage_key();
        host.preflight_menu_action(action)?;
        ensure!(
            host.stage_key() == stage_key && host.binding() == self.state_binding(),
            "merchant private transaction rebound during preflight"
        );
        Ok(Engine {
            menu: self.clone(),
            inv: inventory.clone(),
            stage_key,
            host,
        })
    }
    fn finish<H: MerchantMenuHost<Binding = B, OfferKey = K>>(
        &self,
        inventory: &PlayerInventory,
        mut engine: Engine<'_, B, K, H>,
    ) -> Result<MerchantMenuPlan<B, K, H::StageKey>> {
        ensure!(
            engine.host.stage_key() == engine.stage_key
                && engine.host.binding() == self.state_binding(),
            "merchant private transaction rebound during source callbacks"
        );
        ensure!(
            engine.host.callbacks_preserve_menu_owners()?,
            "merchant callback context changed during preparation"
        );
        engine.menu.advance()?;
        Ok(MerchantMenuPlan {
            before: self.clone(),
            inventory_before: inventory.clone(),
            after: engine.menu,
            inventory_after: engine.inv,
            stage_key: engine.stage_key,
        })
    }
    pub fn prepare_initialize<H: MerchantMenuHost<Binding = B, OfferKey = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &mut H,
    ) -> Result<MerchantMenuPlan<B, K, H::StageKey>> {
        let mut e = self.action(inventory, host, MerchantMenuAction::Initialize)?;
        let events = e.menu.sync.initialize(&e.menu.top(), &e.inv, &[])?;
        e.sync_events(events)?;
        self.finish(inventory, e)
    }
    pub fn prepare_broadcast<H: MerchantMenuHost<Binding = B, OfferKey = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &mut H,
    ) -> Result<MerchantMenuPlan<B, K, H::StageKey>> {
        let mut e = self.action(inventory, host, MerchantMenuAction::Broadcast)?;
        e.broadcast()?;
        self.finish(inventory, e)
    }
    /// Actual MerchantMenu.slotsChanged refreshes BEFORE super.broadcastChanges.
    pub fn prepare_slots_changed<H: MerchantMenuHost<Binding = B, OfferKey = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &mut H,
    ) -> Result<MerchantMenuPlan<B, K, H::StageKey>> {
        let mut e = self.action(inventory, host, MerchantMenuAction::SlotsChanged)?;
        e.refresh()?;
        e.broadcast()?;
        self.finish(inventory, e)
    }
    pub fn prepare_click<H: MerchantMenuHost<Binding = B, OfferKey = K>>(
        &self,
        click: &ContainerClick,
        inventory: &PlayerInventory,
        creative: bool,
        host: &mut H,
    ) -> Result<MerchantMenuPlan<B, K, H::StageKey>> {
        ensure!(
            click.window_id == self.sync.window_id(),
            "wrong merchant window"
        );
        ensure!(click.slot < MENU_SLOTS as i16, "merchant slot outside menu");
        let mut e = self.action(inventory, host, MerchantMenuAction::Click(click.kind))?;
        let before = e.menu.sync.state_id();
        e.menu.sync.suppress_remote_updates(true);
        e.do_click(click.slot as i32, click.button as i32, click.kind, creative)?;
        let events = e
            .menu
            .sync
            .finish_click(click, before, &e.menu.top(), &e.inv, &[])?;
        e.sync_events(events)?;
        self.finish(inventory, e)
    }
    /// SelectTrade has only an index, no container/state ID. Main must establish
    /// actual current MerchantMenu identity before dispatch. No invented spectator
    /// gate or immediate broadcast is added to the source handler.
    pub fn prepare_select_trade<H: MerchantMenuHost<Binding = B, OfferKey = K>>(
        &self,
        index: i32,
        inventory: &PlayerInventory,
        host: &mut H,
    ) -> Result<MerchantMenuPlan<B, K, H::StageKey>> {
        let mut e = self.action(inventory, host, MerchantMenuAction::SelectTrade(index))?;
        e.select_trade(index)?;
        self.finish(inventory, e)
    }
    /// Actual close-screen packet, if source requires one, precedes this action.
    pub fn prepare_close<H: MerchantMenuHost<Binding = B, OfferKey = K>>(
        &self,
        inventory: &PlayerInventory,
        context: MenuCloseContext,
        host: &mut H,
    ) -> Result<MerchantMenuPlan<B, K, H::StageKey>> {
        let mut e = self.action(inventory, host, MerchantMenuAction::Close(context))?;
        e.close(context)?;
        self.finish(inventory, e)
    }
}
struct Engine<'a, B, K, H: MerchantMenuHost> {
    menu: MerchantMenu<B, K>,
    inv: PlayerInventory,
    stage_key: H::StageKey,
    host: &'a mut H,
}
impl<
        B: Clone + Eq + Debug,
        K: Clone + Eq + Debug,
        H: MerchantMenuHost<Binding = B, OfferKey = K>,
    > Engine<'_, B, K, H>
{
    fn observe(&mut self) -> Result<()> {
        self.host
            .observe_menu_phase(phase(&self.menu.state, &self.inv, self.menu.carried()))
    }
    fn sync_events(&mut self, events: Vec<SyncEvent>) -> Result<()> {
        for event in events {
            self.observe()?;
            self.host.stage_sync(event)?;
        }
        Ok(())
    }
    fn broadcast(&mut self) -> Result<()> {
        let events = self
            .menu
            .sync
            .broadcast_changes(&self.menu.top(), &self.inv, &[])?;
        self.sync_events(events)
    }
    fn refresh(&mut self) -> Result<()> {
        let inv = &self.inv;
        let carried = self.menu.carried().clone();
        self.menu.state.menu_refresh(self.host, &mut |state, host| {
            host.observe_menu_phase(phase(state, inv, &carried))
        })
    }
    fn select_hint(&mut self, index: i32) -> Result<()> {
        let inv = &self.inv;
        let carried = self.menu.carried().clone();
        self.menu
            .state
            .menu_select(index, self.host, &mut |state, host| {
                host.observe_menu_phase(phase(state, inv, &carried))
            })
    }
    fn dirty(&mut self) -> Result<()> {
        self.observe()?;
        self.host.inventory_set_changed()
    }
    fn slot(&self, slot: usize) -> ItemStack {
        if slot < 2 {
            self.menu.state.payments()[slot].clone()
        } else if slot == 2 {
            self.menu.state.preview().clone()
        } else {
            self.inv
                .slot(slot + 6)
                .expect("validated merchant player slot")
                .clone()
        }
    }
    fn may_place(slot: usize) -> bool {
        slot != 2
    }
    fn raw(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        if slot < 3 {
            self.menu.state.menu_set_raw(slot, stack)
        } else {
            self.inv.set_slot_authoritative(slot + 6, stack);
            Ok(())
        }
    }
    fn changed(&mut self, slot: usize) -> Result<()> {
        if slot < 3 {
            self.refresh()
        } else {
            self.dirty()
        }
    }
    /// Slot.setByPlayer -> container.setItem -> Slot.setChanged.
    fn put(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        check_stack(self.host, &stack)?;
        if slot < 3 {
            let inv = &self.inv;
            let carried = self.menu.carried().clone();
            self.menu
                .state
                .menu_set_slot(slot, stack, self.host, &mut |state, host| {
                    host.observe_menu_phase(phase(state, inv, &carried))
                })?;
        } else {
            self.raw(slot, stack)?;
        }
        self.changed(slot)
    }
    fn mutate(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        self.raw(slot, stack)?;
        self.changed(slot)
    }
    fn drop(&mut self, stack: ItemStack, trace_item: bool) -> Result<()> {
        if !stack.is_empty() {
            self.observe()?;
            self.host.stage_drop(DroppedStack {
                stack,
                around: false,
                trace_item,
            })?;
        }
        Ok(())
    }
    fn on_take(&mut self, slot: usize, stack: &ItemStack, crafted: i32) -> Result<()> {
        if slot == 2 {
            let inv = &self.inv;
            let carried = self.menu.carried().clone();
            self.menu
                .state
                .menu_finish_take(stack, crafted, self.host, &mut |state, host| {
                    host.observe_menu_phase(phase(state, inv, &carried))
                })
        } else {
            self.changed(slot)
        }
    }
    /// Slot.tryRemove then optional cursor install then Slot.onTake.
    fn take(
        &mut self,
        slot: usize,
        requested: i32,
        max: i32,
        cursor: Option<ItemStack>,
    ) -> Result<ItemStack> {
        let source = self.slot(slot);
        if source.is_empty() || requested <= 0 || max <= 0 || slot == 2 && max < source.count {
            return Ok(ItemStack::empty());
        }
        if slot == 2 {
            ensure!(
                self.menu.state.active_offer().is_some(),
                "unsupported stale merchant preview without real offer"
            );
        }
        let amount = requested.min(max);
        let taken = if slot < 3 {
            let inv = &self.inv;
            let carried = self.menu.carried().clone();
            self.menu
                .state
                .menu_remove(slot, amount, self.host, &mut |state, host| {
                    host.observe_menu_phase(phase(state, inv, &carried))
                })?
        } else {
            let n = amount.min(source.count);
            self.raw(slot, counted(&source, source.count - n))?;
            counted(&source, n)
        };
        if self.slot(slot).is_empty() {
            self.put(slot, ItemStack::empty())?;
        }
        if let Some(cursor) = cursor {
            self.menu.sync.set_carried(cursor);
        }
        self.on_take(slot, &taken, amount.min(source.count))?;
        Ok(taken)
    }
    fn pickup(&mut self, slot: usize, button: i32) -> Result<()> {
        let source = self.slot(slot);
        let carried = self.menu.carried().clone();
        if carried.is_empty() {
            let requested = if button == 0 {
                source.count
            } else {
                (source.count + 1) / 2
            };
            let actual = if slot == 2 { source.count } else { requested };
            self.take(slot, requested, i32::MAX, Some(counted(&source, actual)))?;
        } else if Self::may_place(slot) {
            if source.is_empty() || source.same_item_same_components(&carried)? {
                // Slot.safeInsert retains signed capacity: an overfull existing
                // stack can return its excess into the cursor.
                let n = (if button == 0 { carried.count } else { 1 })
                    .min(maximum(&carried)? - source.count);
                self.menu
                    .sync
                    .set_carried(counted(&carried, carried.count - n));
                self.put(slot, counted(&carried, source.count + n))?;
            } else if carried.count <= maximum(&carried)? {
                self.menu.sync.set_carried(source);
                self.put(slot, carried)?;
            }
        } else if !source.is_empty() && source.same_item_same_components(&carried)? {
            let capacity = maximum(&carried)? - carried.count;
            if source.count <= capacity {
                self.take(
                    slot,
                    source.count,
                    capacity,
                    Some(counted(&carried, carried.count + source.count)),
                )?;
            }
        }
        self.changed(slot)
    }
    /// Source mutation happens before destination Slot callbacks (same object).
    fn move_to(
        &mut self,
        moving: &mut ItemStack,
        start: usize,
        end: usize,
        reverse: bool,
        source: usize,
    ) -> Result<bool> {
        let before = moving.count;
        let slots = if reverse {
            (start..end).rev().collect::<Vec<_>>()
        } else {
            (start..end).collect()
        };
        if moving.is_stackable()? {
            for &slot in &slots {
                let target = self.slot(slot);
                if !target.is_empty() && target.same_item_same_components(moving)? {
                    let n = moving.count.min((maximum(moving)? - target.count).max(0));
                    if n > 0 {
                        *moving = counted(moving, moving.count - n);
                        self.raw(source, moving.clone())?;
                        self.mutate(slot, counted(&target, target.count + n))?;
                    }
                    if moving.is_empty() {
                        break;
                    }
                }
            }
        }
        if !moving.is_empty() {
            for slot in slots {
                if self.slot(slot).is_empty() && Self::may_place(slot) {
                    let n = moving.count.min(maximum(moving)?);
                    let inserted = counted(moving, n);
                    *moving = counted(moving, moving.count - n);
                    self.raw(source, moving.clone())?;
                    self.put(slot, inserted)?;
                    self.changed(slot)?;
                    break;
                }
            }
        }
        Ok(moving.count != before)
    }
    fn quick_move(&mut self, slot: usize) -> Result<()> {
        let original = self.slot(slot);
        loop {
            let source = self.slot(slot);
            if source.is_empty() || source.item_id != original.item_id {
                break;
            }
            if slot == 2 {
                ensure!(
                    self.menu.state.active_offer().is_some(),
                    "unsupported stale merchant result owner"
                );
            }
            let mut moving = source.clone();
            let (start, end, reverse) = if slot < 3 {
                (3, 39, slot == 2)
            } else if slot < 30 {
                (30, 39, false)
            } else {
                (3, 30, false)
            };
            if !self.move_to(&mut moving, start, end, reverse, slot)? {
                break;
            }
            if slot == 2 {
                self.observe()?;
                self.host
                    .item_crafted_by(&source, source.count - moving.count)?;
                self.observe()?;
                self.host.play_trade_sound()?;
            }
            if moving.is_empty() {
                self.put(slot, ItemStack::empty())?;
            } else {
                self.changed(slot)?;
            }
            self.on_take(slot, &moving, 0)?;
        }
        Ok(())
    }
    fn swap(&mut self, slot: usize, button: i32, creative: bool) -> Result<()> {
        let player = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        let source = self.slot(slot);
        let held = self.inv.slot(player).unwrap().clone();
        if source.is_empty() && held.is_empty() {
            return Ok(());
        }
        if slot >= 3 && slot + 6 == player {
            if source.count > maximum(&source)? {
                let cap = maximum(&source)?;
                let mut remainder = counted(&source, source.count - cap);
                self.put(slot, counted(&source, cap))?;
                self.on_take(slot, &remainder, 0)?;
                self.observe()?;
                self.host
                    .inventory_add(&mut self.inv, &mut remainder, creative)?;
                self.drop(remainder, true)?;
            } else {
                self.put(slot, source.clone())?;
                self.on_take(slot, &source, 0)?;
            }
            return Ok(());
        }
        if held.is_empty() {
            if slot == 2 {
                ensure!(
                    self.menu.state.active_offer().is_some(),
                    "unsupported stale merchant result owner"
                );
            }
            self.inv.set_slot_authoritative(player, source.clone());
            // SWAP uses setByPlayer, not remove; MerchantResultSlot's removeCount stays zero.
            self.put(slot, ItemStack::empty())?;
            self.on_take(slot, &source, 0)?;
        } else if source.is_empty() && Self::may_place(slot) {
            let n = held.count.min(maximum(&held)?);
            self.inv
                .set_slot_authoritative(player, counted(&held, held.count - n));
            self.put(slot, counted(&held, n))?;
        } else if Self::may_place(slot) {
            let cap = maximum(&held)?;
            if held.count > cap {
                self.inv
                    .set_slot_authoritative(player, counted(&held, held.count - cap));
                self.put(slot, counted(&held, cap))?;
                self.on_take(slot, &source, 0)?;
                let mut old = source;
                self.observe()?;
                self.host.inventory_add(&mut self.inv, &mut old, creative)?;
                self.drop(old, true)?;
            } else {
                self.inv.set_slot_authoritative(player, source.clone());
                self.put(slot, held)?;
                self.on_take(slot, &source, 0)?;
            }
        }
        Ok(())
    }
    fn drag(&mut self, slot: i32, button: i32, creative: bool) -> Result<()> {
        let previous = self.menu.drag_status;
        self.menu.drag_status = (button & 3) as u8;
        if !(previous == 1 && self.menu.drag_status == 2) && previous != self.menu.drag_status {
            self.menu.reset_drag();
            return Ok(());
        }
        if self.menu.carried().is_empty() {
            self.menu.reset_drag();
            return Ok(());
        }
        match self.menu.drag_status {
            0 => {
                self.menu.drag_type = ((button >> 2) & 3) as u8;
                if self.menu.drag_type <= 1 || self.menu.drag_type == 2 && creative {
                    self.menu.drag_status = 1;
                    self.menu.drag_slots.clear();
                } else {
                    self.menu.reset_drag();
                }
            }
            1 if slot >= 0 => {
                let target = self.slot(slot as usize);
                let carried = self.menu.carried();
                if Self::may_place(slot as usize)
                    && (target.is_empty()
                        || target.same_item_same_components(carried)?
                            && target.count <= carried.max_stack_size()?)
                    && (self.menu.drag_type == 2
                        || carried.count > self.menu.drag_slots.len() as i32)
                {
                    self.menu.drag_slots.insert(slot as usize);
                }
            }
            1 => {}
            2 => {
                if self.menu.drag_slots.len() == 1 {
                    let slot = *self.menu.drag_slots.iter().next().unwrap();
                    let button = self.menu.drag_type;
                    self.menu.reset_drag();
                    return self.do_click(slot as i32, button as i32, ClickType::Pickup, creative);
                }
                let selected = self.menu.drag_slots.iter().copied().collect::<Vec<_>>();
                let order = self.host.quickcraft_iteration_order(&selected)?;
                ensure!(
                    order.len() == selected.len()
                        && order.iter().copied().collect::<BTreeSet<_>>() == self.menu.drag_slots,
                    "actual merchant drag encounter order is not a permutation"
                );
                let carried = self.menu.carried().clone();
                let mut left = carried.count;
                let each = match self.menu.drag_type {
                    0 => carried.count / (selected.len().max(1) as i32),
                    1 => 1,
                    2 => carried.max_stack_size()?,
                    _ => 0,
                };
                for slot in order {
                    let old = self.slot(slot);
                    if !Self::may_place(slot)
                        || !old.is_empty()
                            && (!old.same_item_same_components(&carried)?
                                || old.count > carried.max_stack_size()?)
                        || self.menu.drag_type != 2 && carried.count < selected.len() as i32
                    {
                        continue;
                    }
                    let count = each.wrapping_add(old.count).min(maximum(&carried)?);
                    left = left.wrapping_sub(count.wrapping_sub(old.count));
                    self.put(slot, counted(&carried, count))?;
                }
                self.menu.sync.set_carried(counted(&carried, left));
                self.menu.reset_drag();
            }
            _ => self.menu.reset_drag(),
        }
        Ok(())
    }
    fn do_click(&mut self, slot: i32, button: i32, kind: ClickType, creative: bool) -> Result<()> {
        if kind == ClickType::QuickCraft {
            return self.drag(slot, button, creative);
        }
        if self.menu.drag_status != 0 {
            self.menu.reset_drag();
            return Ok(());
        }
        match kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let carried = self.menu.carried().clone();
                    if button == 0 {
                        self.drop(carried, true)?;
                        self.menu.sync.set_carried(ItemStack::empty());
                    } else {
                        let n = carried.count.min(1);
                        self.menu
                            .sync
                            .set_carried(counted(&carried, carried.count - n));
                        self.drop(counted(&carried, n), true)?;
                    }
                } else if slot >= 0 {
                    if kind == ClickType::Pickup {
                        self.pickup(slot as usize, button)?;
                    } else {
                        self.quick_move(slot as usize)?;
                    }
                }
            }
            ClickType::Swap if slot >= 0 && ((0..9).contains(&button) || button == 40) => {
                self.swap(slot as usize, button, creative)?
            }
            ClickType::Clone if creative && slot >= 0 && self.menu.carried().is_empty() => {
                let source = self.slot(slot as usize);
                if !source.is_empty() {
                    self.menu
                        .sync
                        .set_carried(counted(&source, source.max_stack_size()?));
                }
            }
            ClickType::Throw if slot >= 0 && self.menu.carried().is_empty() => {
                let source = self.slot(slot as usize);
                let amount = if button == 0 { 1 } else { source.count };
                let taken = self.take(slot as usize, amount, i32::MAX, None)?;
                self.drop(taken, true)?;
            }
            // MerchantMenu.canTakeItemForPickAll returns false for EVERY slot.
            ClickType::PickupAll => {}
            _ => {}
        }
        Ok(())
    }
    fn select_trade(&mut self, index: i32) -> Result<()> {
        self.select_hint(index)?;
        let offers = self.host.offers()?;
        if index < 0 || index as usize >= offers.len() {
            return Ok(());
        }
        for slot in 0..2 {
            let mut old = self.slot(slot);
            if !old.is_empty() {
                if !self.move_to(&mut old, 3, 39, true, slot)? {
                    return Ok(());
                }
                let inv = &self.inv;
                let carried = self.menu.carried().clone();
                self.menu
                    .state
                    .menu_set_slot(slot, old, self.host, &mut |state, host| {
                        host.observe_menu_phase(phase(state, inv, &carried))
                    })?;
            }
        }
        if self.menu.state.payments().iter().all(ItemStack::is_empty) {
            let current = self.host.offers()?;
            let offer = &current
                .get(index as usize)
                .ok_or_else(|| anyhow::anyhow!("merchant offers changed during autofill"))?
                .offer;
            self.fill_payment(0, &offer.cost_a)?;
            if let Some(cost) = &offer.cost_b {
                self.fill_payment(1, cost)?;
            }
        }
        Ok(())
    }
    fn fill_payment(&mut self, slot: usize, cost: &ItemCost) -> Result<()> {
        for player in 9..45 {
            let source = self.inv.slot(player).unwrap().clone();
            let old = self.slot(slot);
            if source.is_empty()
                || !cost.test(&source)?
                || !old.is_empty() && !source.same_item_same_components(&old)?
            {
                continue;
            }
            let cap = source.max_stack_size()?;
            let n = (cap - old.count).min(source.count);
            ensure!(n >= 0, "unsupported overfull merchant autofill");
            self.inv
                .set_slot_authoritative(player, counted(&source, source.count - n));
            let inv = &self.inv;
            let carried = self.menu.carried().clone();
            self.menu.state.menu_set_slot(
                slot,
                counted(&source, old.count + n),
                self.host,
                &mut |state, host| host.observe_menu_phase(phase(state, inv, &carried)),
            )?;
            if self.slot(slot).count >= cap {
                break;
            }
        }
        Ok(())
    }
    fn close_return(
        &mut self,
        owned: ItemStack,
        cursor: bool,
        context: MenuCloseContext,
    ) -> Result<()> {
        let inv_before = self.inv.clone();
        let removal = if cursor {
            self.menu
                .sync
                .close_inputs_ordered(&mut [], &mut self.inv, context)?
        } else {
            self.menu
                .sync
                .close_inputs_ordered(&mut [owned.clone()], &mut self.inv, context)?
        };
        let mut phase = phase(&self.menu.state, &inv_before, &self.menu.sync.carried());
        let mut returning = owned;
        for event in removal.events {
            match event {
                MenuRemovalEvent::Packet { index } => {
                    let packet = removal.outcome.packets[index].clone();
                    ensure!(packet.id == 0x15, "unsupported merchant return packet");
                    let mut reader = Reader::new(&packet.payload);
                    ensure!(
                        reader.i8()? == -2 && reader.varint()? == 0,
                        "merchant return requires raw inventory packet"
                    );
                    let raw = reader.i16()?;
                    let slot = match raw {
                        0..=8 => 36 + raw as usize,
                        9..=35 => raw as usize,
                        40 => 45,
                        _ => bail!("invalid merchant raw return slot"),
                    };
                    let stack = ItemStack::decode_exact(reader.remaining())?;
                    let old = phase.inventory.slot(slot).unwrap();
                    ensure!(
                        stack.same_item_same_components(&returning)?,
                        "merchant close changed return identity"
                    );
                    let count = if old.is_empty() {
                        0
                    } else {
                        ensure!(
                            old.same_item_same_components(&stack)?,
                            "merchant close overwrote owned item"
                        );
                        old.count
                    };
                    let added = stack.count - count;
                    ensure!(
                        added > 0 && added <= returning.count,
                        "invalid merchant close insertion"
                    );
                    phase.inventory.set_slot_authoritative(slot, stack);
                    returning = counted(&returning, returning.count - added);
                    phase.carried = if cursor {
                        returning.clone()
                    } else {
                        ItemStack::empty()
                    };
                    phase.returning = Some(returning.clone());
                    self.host.observe_menu_phase(phase.clone())?;
                    self.host.stage_sync(SyncEvent::Packet(packet))?;
                }
                MenuRemovalEvent::Drop { index } => {
                    let drop = removal.outcome.drops[index].clone();
                    phase.carried = if cursor {
                        returning.clone()
                    } else {
                        ItemStack::empty()
                    };
                    phase.returning = Some(drop.stack.clone());
                    self.host.observe_menu_phase(phase.clone())?;
                    self.host.stage_drop(drop)?;
                    returning = ItemStack::empty();
                }
            }
        }
        Ok(())
    }
    fn close(&mut self, context: MenuCloseContext) -> Result<()> {
        self.menu.reset_drag();
        let carried = self.menu.carried().clone();
        self.close_return(carried, true, context)?;
        self.observe()?;
        self.host.clear_trading_player()?;
        for slot in 0..2 {
            let stack = self.menu.state.menu_remove_no_update(slot)?;
            self.close_return(stack, false, context)?;
        }
        self.menu.state.menu_set_raw(2, ItemStack::empty())?;
        self.menu.active = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crafting::RecipeBook,
        workstations::merchant::{MerchantOffer, OfferEntry},
    };
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    /// Isolated transaction receiver, not a native villager or production owner.
    #[derive(Clone)]
    struct Host {
        binding: (u64, u64),
        key: u64,
        trading: bool,
        offer: MerchantOffer,
        xp: i32,
        phase: Option<MerchantPhase<(u64, u64), u64>>,
        calls: Vec<(String, MerchantPhase<(u64, u64), u64>)>,
        drops: Vec<DroppedStack>,
        fail_trade: bool,
        packets: usize,
    }
    impl Host {
        fn new() -> Self {
            Self {
                binding: (11, 22),
                key: 1,
                trading: true,
                offer: MerchantOffer {
                    cost_a: ItemCost::new(item("emerald", 1).item_id, 2, vec![]).unwrap(),
                    cost_b: None,
                    result: item("bread", 3),
                    uses: 0,
                    max_uses: 8,
                    reward_exp: true,
                    special_price_diff: 0,
                    demand: 0,
                    price_multiplier: 0.,
                    xp: 4,
                },
                xp: 0,
                phase: None,
                calls: vec![],
                drops: vec![],
                fail_trade: false,
                packets: 0,
            }
        }
        fn record(&mut self, name: String) {
            self.calls
                .push((name, self.phase.clone().expect("source phase")));
        }
    }
    impl MerchantStage for Host {
        type Binding = (u64, u64);
        type OfferKey = u64;
        type StageKey = u64;
        fn binding(&self) -> Self::Binding {
            self.binding
        }
        fn stage_key(&self) -> u64 {
            self.key
        }
        fn is_trading_player(&self) -> Result<bool> {
            Ok(self.trading)
        }
        fn offers(&self) -> Result<Vec<OfferEntry<u64>>> {
            Ok(vec![OfferEntry {
                key: 7,
                offer: self.offer.clone(),
            }])
        }
        fn offer(&self, key: &u64) -> Result<MerchantOffer> {
            ensure!(*key == 7, "actual offer key");
            Ok(self.offer.clone())
        }
        fn villager_xp(&self) -> Result<i32> {
            Ok(self.xp)
        }
        fn item_crafted_by(&mut self, stack: &ItemStack, amount: i32) -> Result<()> {
            self.record(format!("crafted:{}:{amount}", stack.count));
            Ok(())
        }
        fn notify_trade(&mut self, key: &u64) -> Result<()> {
            ensure!(*key == 7, "actual offer");
            ensure!(
                !self.fail_trade,
                "fixture actual trade callback unavailable"
            );
            self.record("trade".into());
            self.offer.uses += 1;
            Ok(())
        }
        fn notify_trade_updated(&mut self, result: &ItemStack) -> Result<()> {
            self.record(format!("updated:{}", result.count));
            Ok(())
        }
        fn award_traded_stat(&mut self) -> Result<()> {
            self.record("stat".into());
            Ok(())
        }
        fn override_xp(&mut self, xp: i32) -> Result<()> {
            self.record(format!("xp:{xp}"));
            self.xp = xp;
            Ok(())
        }
        fn play_trade_sound(&mut self) -> Result<()> {
            self.record("sound".into());
            Ok(())
        }
        fn clear_trading_player(&mut self) -> Result<()> {
            self.record("detach".into());
            self.trading = false;
            Ok(())
        }
        fn publish(&mut self) -> Result<()> {
            self.key += 1;
            Ok(())
        }
    }
    impl MerchantMenuHost for Host {
        fn preflight_menu_action(&mut self, _: MerchantMenuAction) -> Result<()> {
            Ok(())
        }
        fn callbacks_preserve_menu_owners(&self) -> Result<bool> {
            Ok(true)
        }
        fn observe_menu_phase(&mut self, phase: MerchantPhase<(u64, u64), u64>) -> Result<()> {
            self.phase = Some(phase);
            Ok(())
        }
        fn stage_sync(&mut self, event: SyncEvent) -> Result<()> {
            if matches!(event, SyncEvent::Packet(_)) {
                self.packets += 1;
            }
            self.record("sync".into());
            Ok(())
        }
        fn inventory_set_changed(&mut self) -> Result<()> {
            self.record("dirty".into());
            Ok(())
        }
        fn stage_drop(&mut self, owned: DroppedStack) -> Result<()> {
            self.record(format!("drop:{}", owned.stack.count));
            self.drops.push(owned);
            Ok(())
        }
        fn inventory_add(
            &mut self,
            _: &mut PlayerInventory,
            _: &mut ItemStack,
            _: bool,
        ) -> Result<()> {
            bail!("this fixture does not supply overflow Inventory.add")
        }
        fn require_stack_context(&self, stack: &ItemStack) -> Result<()> {
            ensure!(
                !stack.components.iter().any(|c| c.type_id == 20),
                "custom food reference identity unavailable"
            );
            Ok(())
        }
        fn quickcraft_iteration_order(&self, selected: &[usize]) -> Result<Vec<usize>> {
            // This isolated receiver's actual selected Slot encounter order.
            Ok(selected.iter().rev().copied().collect())
        }
    }
    type Menu = MerchantMenu<(u64, u64), u64>;
    fn setup() -> Result<(Menu, PlayerInventory, Host)> {
        let mut menu = Menu::new(7, (11, 22))?;
        let mut inv = PlayerInventory::default();
        let mut host = Host::new();
        menu.prepare_initialize(&inv, &mut host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        Ok((menu, inv, host))
    }
    fn click(
        menu: &mut Menu,
        inv: &mut PlayerInventory,
        host: &mut Host,
        slot: i16,
        button: i8,
        kind: ClickType,
        creative: bool,
    ) -> Result<()> {
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync.state_id(),
            slot,
            button,
            kind,
            changed_slots: Default::default(),
            carried: ItemStack::empty(),
        };
        menu.prepare_click(&packet, inv, creative, host)?
            .commit(menu, inv, host)
    }
    #[test]
    fn whole_result_pickup_throw_cursor_callbacks_and_stale_ui_guards() -> Result<()> {
        let (mut menu, mut inv, mut host) = setup()?;
        inv.set_slot_authoritative(9, item("emerald", 4));
        menu.prepare_select_trade(0, &inv, &mut host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(menu.state.preview().count, 3);
        assert_eq!(menu.state.payments()[0].count, 4);
        host.calls.clear();
        click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            1,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(menu.carried().count, 3);
        assert_eq!(host.offer.uses, 1);
        assert_eq!(host.xp, 4);
        let crafted = host
            .calls
            .iter()
            .find(|(name, _)| name == "crafted:3:2")
            .unwrap();
        assert_eq!(crafted.1.carried.count, 3);
        let trade = host.calls.iter().find(|(name, _)| name == "trade").unwrap();
        assert_eq!(trade.1.trade.payments()[0].count, 2);
        assert_eq!(trade.1.carried.count, 3);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            4,
            0,
            ClickType::Pickup,
            false,
        )?;
        host.calls.clear();
        click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            0,
            ClickType::Throw,
            false,
        )?;
        assert_eq!(host.drops.last().unwrap().stack.count, 3);
        assert!(host.drops.last().unwrap().trace_item);
        assert!(host.calls.iter().any(|(name, _)| name == "crafted:3:1"));
        assert_eq!(host.offer.uses, 2);
        assert!(menu.state.preview().is_empty());

        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync.state_id(),
            slot: 0,
            button: 0,
            kind: ClickType::PickupAll,
            changed_slots: Default::default(),
            carried: ItemStack::empty(),
        };
        let first = menu.prepare_click(&packet, &inv, false, &mut host)?;
        let second = menu.prepare_click(&packet, &inv, false, &mut host)?;
        let replacement = Menu::new(7, (11, 22))?;
        assert!(first.validate(&replacement, &inv, &host).is_err());
        first.commit(&mut menu, &mut inv, &mut host)?;
        assert!(second.validate(&menu, &inv, &host).is_err());
        host.trading = false;
        assert!(!menu.still_valid(&host)?);
        menu.prepare_broadcast(&inv, &mut host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert!(menu.prepare_click(&packet, &inv, false, &mut host).is_err());
        Ok(())
    }
    #[test]
    fn partial_shift_keeps_original_and_zero_crafted_phases_and_actual_trade_owner() -> Result<()> {
        let (mut menu, mut inv, mut host) = setup()?;
        inv.set_slot_authoritative(9, item("emerald", 2));
        let packets = host.packets;
        menu.prepare_select_trade(0, &inv, &mut host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(packets, host.packets); // Handler does not invent immediate broadcast.
        for slot in 9..45 {
            inv.set_slot_authoritative(slot, item("stone", 64));
        }
        inv.set_slot_authoritative(44, item("bread", 63));
        host.calls.clear();
        click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            0,
            ClickType::QuickMove,
            false,
        )?;
        assert_eq!(inv.slot(44).unwrap().count, 64);
        assert!(menu.state.payments()[0].is_empty());
        assert!(menu.state.preview().is_empty());
        assert_eq!(host.offer.uses, 1);
        let first = host
            .calls
            .iter()
            .position(|(name, _)| name == "crafted:3:1")
            .unwrap();
        let sound = host
            .calls
            .iter()
            .position(|(name, _)| name == "sound")
            .unwrap();
        let zero = host
            .calls
            .iter()
            .position(|(name, _)| name == "crafted:2:0")
            .unwrap();
        assert!(first < sound && sound < zero);
        assert_eq!(host.calls[first].1.trade.preview().count, 2);
        assert_eq!(host.calls[zero].1.trade.preview().count, 3);
        assert_eq!(host.calls[zero].1.inventory.slot(44).unwrap().count, 64);
        Ok(())
    }
    #[test]
    fn result_swap_signed_safe_insert_and_failed_actual_callbacks_leave_live_owners_intact(
    ) -> Result<()> {
        let (mut menu, mut inv, mut host) = setup()?;
        inv.set_slot_authoritative(9, item("emerald", 2));
        menu.prepare_select_trade(0, &inv, &mut host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        let before = menu.clone();
        let inventory_before = inv.clone();
        let mut rejected = host.clone();
        rejected.fail_trade = true;
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync.state_id(),
            slot: 2,
            button: 0,
            kind: ClickType::Pickup,
            changed_slots: Default::default(),
            carried: ItemStack::empty(),
        };
        assert!(menu
            .prepare_click(&packet, &inv, false, &mut rejected)
            .is_err());
        assert_eq!(menu, before);
        assert!(same_inventory(&inv, &inventory_before));
        assert_eq!(host.offer.uses, 0);
        host.calls.clear();
        click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            40,
            ClickType::Swap,
            false,
        )?;
        assert_eq!(inv.slot(45).unwrap(), &item("bread", 3));
        let crafted = host
            .calls
            .iter()
            .find(|(name, _)| name == "crafted:3:0")
            .unwrap();
        assert_eq!(crafted.1.inventory.slot(45).unwrap(), &item("bread", 3));
        assert!(crafted.1.carried.is_empty());
        assert_eq!(host.offer.uses, 1);

        inv.set_slot_authoritative(9, item("stone", 65));
        inv.set_slot_authoritative(10, item("stone", 1));
        click(
            &mut menu,
            &mut inv,
            &mut host,
            4,
            0,
            ClickType::Pickup,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            3,
            0,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(inv.slot(9).unwrap().count, 64);
        assert_eq!(menu.carried().count, 2);
        let mut food = item("bread", 1);
        food.components.push(crate::inventory::Component {
            type_id: 20,
            data: food.effective_component_bytes(20).unwrap().to_vec(),
        });
        inv.set_slot_authoritative(11, food);
        assert!(menu.prepare_click(&packet, &inv, false, &mut host).is_err());
        Ok(())
    }
    #[test]
    fn drag_clone_pickall_swap_player_shift_and_source_ordered_close() -> Result<()> {
        let (mut menu, mut inv, mut host) = setup()?;
        inv.set_slot_authoritative(9, item("emerald", 8));
        inv.set_slot_authoritative(10, item("coal", 8));
        click(
            &mut menu,
            &mut inv,
            &mut host,
            3,
            0,
            ClickType::Pickup,
            false,
        )?;
        for (slot, button) in [(-999, 0), (0, 1), (1, 1), (-999, 2)] {
            click(
                &mut menu,
                &mut inv,
                &mut host,
                slot,
                button,
                ClickType::QuickCraft,
                false,
            )?;
        }
        assert_eq!(menu.state.payments()[0].count, 4);
        assert_eq!(menu.state.payments()[1].count, 4);
        menu.prepare_select_trade(0, &inv, &mut host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        click(&mut menu, &mut inv, &mut host, 2, 0, ClickType::Clone, true)?;
        assert_eq!(menu.carried().count, 64);
        assert_eq!(host.offer.uses, 0);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            1,
            ClickType::Pickup,
            false,
        )?;
        inv.set_slot_authoritative(11, item("bread", 5));
        click(
            &mut menu,
            &mut inv,
            &mut host,
            5,
            0,
            ClickType::PickupAll,
            false,
        )?;
        assert_eq!(menu.carried().count, 63);
        assert_eq!(inv.slot(11).unwrap().count, 5);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            6,
            0,
            ClickType::Pickup,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            0,
            40,
            ClickType::Swap,
            false,
        )?;
        assert_eq!(inv.slot(45).unwrap().count, 8);
        assert!(menu.state.payments()[0].is_empty());
        click(
            &mut menu,
            &mut inv,
            &mut host,
            4,
            0,
            ClickType::QuickMove,
            false,
        )?;
        assert_eq!(inv.slot(36).unwrap(), &item("coal", 8));

        // Actual close cursor precedes detach; payment removal follows detach.
        click(
            &mut menu,
            &mut inv,
            &mut host,
            30,
            0,
            ClickType::Pickup,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            1,
            0,
            ClickType::Pickup,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            5,
            0,
            ClickType::Pickup,
            false,
        )?;
        host.calls.clear();
        host.drops.clear();
        menu.prepare_close(&inv, MenuCloseContext::DeadOrDisconnected, &mut host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        let drop0 = host
            .calls
            .iter()
            .position(|(name, _)| name.starts_with("drop:"))
            .unwrap();
        let detach = host
            .calls
            .iter()
            .position(|(name, _)| name == "detach")
            .unwrap();
        let drop1 = host
            .calls
            .iter()
            .rposition(|(name, _)| name.starts_with("drop:"))
            .unwrap();
        assert!(drop0 < detach && detach < drop1);
        assert!(host.drops.iter().all(|d| !d.around && !d.trace_item));
        assert_eq!(host.drops.iter().map(|d| d.stack.count).sum::<i32>(), 13);
        assert!(!menu.is_active());
        assert!(menu.state.payments().iter().all(ItemStack::is_empty));
        Ok(())
    }
}
