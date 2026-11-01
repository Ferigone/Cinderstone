//! Java 1.21 GrindstoneMenu over one temporary container and a borrowed player
//! inventory. All real owners are staged together: this is not a world/XP host.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    workstations::{
        grindstone::{
            GrindstoneEnchantments, GrindstoneState, INPUT_SLOTS, MENU_SLOTS, RESULT_SLOT,
        },
        menu::{MenuSync, SyncEvent, WorkstationKind},
    },
};
use anyhow::{ensure, Result};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};

/// Captured dispatch identity. Window IDs and structurally equal contents do
/// not authorize an old action against a new menu or a later revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrindstoneMenuToken {
    instance: u64,
    revision: u64,
}
#[derive(Clone, Copy, Debug)]
pub enum GrindstoneMenuAction {
    Initialize,
    Broadcast,
    FullState,
    Click(ClickType),
    Close(MenuCloseContext),
}
#[derive(Clone, Debug)]
pub struct GrindstonePhase {
    pub inputs: [ItemStack; INPUT_SLOTS],
    pub preview: ItemStack,
    pub player_inventory: PlayerInventory,
    pub carried: ItemStack,
}

/// Implement only with the actual captured ServerPlayer, Level/position/menu
/// lifetime, enchantment registry/tag owner, canonical inventory and shared
/// Level RNG/entity allocation owners. A grindstone has no block entity.
///
/// begin_stage must detach ALL mutable owners (including callback effects and
/// packet outbox, Inventory.add, RNG, orb merging/allocation and Player.drop).
/// A shallow clone of live owners is insufficient. publish_stage completes all
/// fallible preparation before its atomic live commit; failure publishes
/// nothing. Success queues the ordered output under the same outer owner lock.
/// It must NEVER drain packets, reenter callbacks or save menu/player state
/// before the controller's following infallible UI installation and outer lock
/// exit. Retained menu state/cursor and host inventory already provide the
/// actual read-only save views; phase snapshots are not another live owner.
/// Sync/dirty listeners must be nonreentrant; preflight must reject unavailable
/// owners and callback contexts rather than implement no-op world substitutes.
pub trait GrindstoneMenuHost: GrindstoneEnchantments + Sized {
    type Binding: Clone + Eq + Debug;
    type RegistrySnapshot: Clone + Eq + Debug;
    fn binding(&self) -> &Self::Binding;
    fn registry_snapshot(&self) -> Self::RegistrySnapshot;
    fn begin_stage(&self, token: GrindstoneMenuToken, action: GrindstoneMenuAction)
        -> Result<Self>;
    fn publish_stage(&mut self, staged: Self, token: GrindstoneMenuToken) -> Result<()>;
    fn preflight(&mut self, action: GrindstoneMenuAction) -> Result<()>;
    fn inventory(&self) -> &PlayerInventory;
    fn inventory_mut(&mut self) -> &mut PlayerInventory;
    fn instabuild(&self) -> bool;
    fn still_valid_grindstone(&self, additional_range: f64) -> Result<bool>;
    fn input_container_set_changed(&mut self, phase: &GrindstonePhase) -> Result<()>;
    fn player_inventory_set_changed(&mut self, phase: &GrindstonePhase) -> Result<()>;
    fn stage_sync_events(&mut self, events: Vec<SyncEvent>, phase: &GrindstonePhase) -> Result<()>;
    fn level_random_next_int(&mut self, bound: i32) -> Result<i32>;
    /// Actual ExperienceOrb.award at Vec3.atCenterOf(captured position), even
    /// for zero XP; never substitute a direct player XP balance increment.
    fn award_experience_orbs(&mut self, amount: i32, phase: &GrindstonePhase) -> Result<()>;
    fn level_event(&mut self, event: i32, data: i32, phase: &GrindstonePhase) -> Result<()>;
    fn stage_drop(&mut self, owned: DroppedStack, phase: &GrindstonePhase) -> Result<()>;
    /// Actual Inventory.add mutates this one stack to its remaining ownership.
    fn add_to_player_inventory(
        &mut self,
        stack: &mut ItemStack,
        phase: &GrindstonePhase,
    ) -> Result<bool>;
    /// Actual Inventory.placeItemBackInInventory(stack,true), including its
    /// insertion packets and overflow Player.drop(false). The same actual
    /// inventory/RNG/entity owners are used, in this call's source order.
    fn place_item_back(&mut self, stack: &mut ItemStack, phase: &GrindstonePhase) -> Result<()>;
    /// Exact actual Slot-identity HashSet encounter order, or reject.
    fn quickcraft_iteration_order(&self, selected: &[usize]) -> Result<Vec<usize>>;
}

#[derive(Debug)]
pub struct GrindstoneMenu<K, S> {
    binding: K,
    registry: S,
    token: GrindstoneMenuToken,
    active: bool,
    state: GrindstoneState,
    sync: MenuSync,
    drag_status: u8,
    drag_type: u8,
    drag_slots: BTreeSet<usize>,
}
fn counted(item: &ItemStack, count: i32) -> ItemStack {
    if count <= 0 || item.is_empty() {
        ItemStack::empty()
    } else {
        let mut item = item.clone();
        item.count = count;
        item
    }
}
impl<K: Clone + Eq + Debug, S: Clone + Eq + Debug> GrindstoneMenu<K, S> {
    pub fn new<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        window: u8,
        host: &H,
    ) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let mut seen = NEXT.load(Ordering::Relaxed);
        let instance = loop {
            let next = seen
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("grindstone UI identity exhausted"))?;
            match NEXT.compare_exchange_weak(seen, next, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break seen,
                Err(actual) => seen = actual,
            }
        };
        Ok(Self {
            binding: host.binding().clone(),
            registry: host.registry_snapshot(),
            token: GrindstoneMenuToken {
                instance,
                revision: 0,
            },
            active: true,
            state: GrindstoneState::default(),
            sync: MenuSync::new(window, WorkstationKind::Grindstone)?,
            drag_status: 0,
            drag_type: 0,
            drag_slots: BTreeSet::new(),
        })
    }
    fn staged_copy(&self) -> Self {
        Self {
            binding: self.binding.clone(),
            registry: self.registry.clone(),
            token: self.token,
            active: self.active,
            state: self.state.clone(),
            sync: self.sync.clone(),
            drag_status: self.drag_status,
            drag_type: self.drag_type,
            drag_slots: self.drag_slots.clone(),
        }
    }
    pub fn token(&self) -> GrindstoneMenuToken {
        self.token
    }
    pub fn state(&self) -> &GrindstoneState {
        &self.state
    }
    pub fn window_id(&self) -> u8 {
        self.sync.window_id()
    }
    pub fn state_id(&self) -> i32 {
        self.sync.state_id()
    }
    pub fn carried(&self) -> &ItemStack {
        self.sync.carried()
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    fn phase<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &self,
        host: &H,
    ) -> GrindstonePhase {
        GrindstonePhase {
            inputs: self.state.inputs().clone(),
            preview: self.state.preview().clone(),
            player_inventory: host.inventory().clone(),
            carried: self.carried().clone(),
        }
    }
    fn validate<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &self,
        token: GrindstoneMenuToken,
        host: &H,
    ) -> Result<()> {
        ensure!(
            self.active && self.token == token,
            "stale/closed grindstone UI action"
        );
        ensure!(
            &self.binding == host.binding() && self.registry == host.registry_snapshot(),
            "grindstone owner/registry binding changed"
        );
        Ok(())
    }
    pub fn still_valid<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &self,
        host: &H,
    ) -> Result<bool> {
        self.validate(self.token, host)?;
        host.still_valid_grindstone(4.0)
    }
    fn transaction<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        token: GrindstoneMenuToken,
        action: GrindstoneMenuAction,
        host: &mut H,
        f: impl FnOnce(&mut Self, &mut H) -> Result<()>,
    ) -> Result<()> {
        self.validate(token, host)?;
        let mut stage = host.begin_stage(token, action)?;
        self.validate(token, &stage)?;
        stage.preflight(action)?;
        let mut next = self.staged_copy();
        f(&mut next, &mut stage)?;
        ensure!(
            &next.binding == stage.binding() && next.registry == stage.registry_snapshot(),
            "grindstone binding changed during callback"
        );
        next.token.revision = next
            .token
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("grindstone revision exhausted"))?;
        host.publish_stage(stage, token)?;
        *self = next;
        Ok(())
    }
    pub fn initialize<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        self.transaction(
            self.token,
            GrindstoneMenuAction::Initialize,
            host,
            |next, host| {
                let events = next.sync.initialize(&next.top(), host.inventory(), &[])?;
                host.stage_sync_events(events, &next.phase(host))
            },
        )
    }
    pub fn broadcast_changes<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        self.transaction(
            self.token,
            GrindstoneMenuAction::Broadcast,
            host,
            |next, host| next.broadcast(host),
        )
    }
    /// Resynchronize the retained menu without recomputing its result or
    /// changing inputs, cursor or drag state. Publication uses the same actual
    /// owner transaction as other synchronization phases.
    pub fn broadcast_full_state<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        self.transaction(
            self.token,
            GrindstoneMenuAction::FullState,
            host,
            |next, host| {
                let events = next
                    .sync
                    .broadcast_full_state(&next.top(), host.inventory(), &[])?;
                host.stage_sync_events(events, &next.phase(host))
            },
        )
    }
    pub fn click<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        token: GrindstoneMenuToken,
        click: &ContainerClick,
        host: &mut H,
    ) -> Result<()> {
        ensure!(
            click.window_id == self.window_id() && click.slot < MENU_SLOTS as i16,
            "invalid grindstone click window/slot"
        );
        self.transaction(
            token,
            GrindstoneMenuAction::Click(click.kind),
            host,
            |next, host| {
                next.check_components(host)?;
                let before = next.state_id();
                next.sync.suppress_remote_updates(true);
                next.do_click(click, host)?;
                let events =
                    next.sync
                        .finish_click(click, before, &next.top(), host.inventory(), &[])?;
                host.stage_sync_events(events, &next.phase(host))
            },
        )
    }
    /// Server close-window publication precedes this removed callback. The
    /// central owner transfers window0 baselines AFTER removal. No preview is
    /// returned, and no XP is awarded simply because a menu closes.
    pub fn close<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        token: GrindstoneMenuToken,
        context: MenuCloseContext,
        host: &mut H,
    ) -> Result<()> {
        self.transaction(
            token,
            GrindstoneMenuAction::Close(context),
            host,
            |next, host| {
                next.reset_drag();
                let mut cursor = next.carried().clone();
                if !cursor.is_empty() {
                    match context {
                        MenuCloseContext::AliveConnected => {
                            host.place_item_back(&mut cursor, &next.phase(host))?
                        }
                        MenuCloseContext::DeadOrDisconnected => host.stage_drop(
                            DroppedStack {
                                stack: cursor,
                                around: false,
                                trace_item: false,
                            },
                            &next.phase(host),
                        )?,
                    }
                    next.sync.set_carried(ItemStack::empty());
                }
                for index in 0..INPUT_SLOTS {
                    // SimpleContainer.removeItemNoUpdate: no listener notification.
                    let mut item = next.state.inputs()[index].clone();
                    next.state
                        .mutate_input_in_place(index, ItemStack::empty())?;
                    match context {
                        MenuCloseContext::AliveConnected => {
                            host.place_item_back(&mut item, &next.phase(host))?
                        }
                        MenuCloseContext::DeadOrDisconnected => host.stage_drop(
                            DroppedStack {
                                stack: item,
                                around: false,
                                trace_item: false,
                            },
                            &next.phase(host),
                        )?,
                    }
                }
                next.state.mutate_result_in_place(ItemStack::empty());
                next.active = false;
                Ok(())
            },
        )
    }
    fn top(&self) -> Vec<ItemStack> {
        let mut out = self.state.inputs().to_vec();
        out.push(self.state.preview().clone());
        out
    }
    fn check_components<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &self,
        host: &H,
    ) -> Result<()> {
        for item in self
            .top()
            .iter()
            .chain(std::iter::once(self.carried()))
            .chain((9..46).filter_map(|n| host.inventory().slot(n)))
        {
            ensure!(
                inventory::item_name(item.item_id) != Some("bundle"),
                "bundle overrides require their actual owner"
            );
            ensure!(
                !item
                    .components
                    .iter()
                    .any(|component| component.type_id == 20),
                "food component nested-stack Java identity unresolved"
            );
        }
        Ok(())
    }
    fn reset_drag(&mut self) {
        self.drag_status = 0;
        self.drag_slots.clear();
    }
    fn broadcast<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        let events = self
            .sync
            .broadcast_changes(&self.top(), host.inventory(), &[])?;
        host.stage_sync_events(events, &self.phase(host))
    }
    fn slot<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &self,
        index: usize,
        host: &H,
    ) -> ItemStack {
        if index < 2 {
            self.state.inputs()[index].clone()
        } else if index == RESULT_SLOT {
            self.state.preview().clone()
        } else {
            host.inventory()
                .slot(index + 6)
                .expect("validated grindstone player slot")
                .clone()
        }
    }
    fn may_place(index: usize, item: &ItemStack) -> Result<bool> {
        if index < 2 {
            GrindstoneState::may_place(item)
        } else {
            Ok(index > RESULT_SLOT)
        }
    }
    fn limit(_index: usize, item: &ItemStack) -> Result<i32> {
        Ok(item.max_stack_size()?.min(99))
    }
    fn mutate_raw<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        index: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if index < 2 {
            self.state.mutate_input_in_place(index, item)?;
        } else if index == RESULT_SLOT {
            self.state.mutate_result_in_place(item);
        } else {
            host.inventory_mut().set_slot_authoritative(index + 6, item);
        }
        Ok(())
    }
    fn changed<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        index: usize,
        host: &mut H,
    ) -> Result<()> {
        if index < 2 {
            host.input_container_set_changed(&self.phase(host))?;
            // cqt.slotsChanged calls cpu.slotsChanged first (old preview),
            // then createResult -> another broadcast with the new preview.
            self.broadcast(host)?;
            self.state.recompute(host)?;
            self.broadcast(host)
        } else if index == RESULT_SLOT {
            Ok(())
        }
        // ResultContainer.setChanged
        else {
            host.player_inventory_set_changed(&self.phase(host))
        }
    }
    fn set_slot<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        index: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if index < 2 {
            self.state.set_container_input(index, item)?;
            self.changed(index, host)?;
        } else {
            self.mutate_raw(index, item, host)?;
        }
        self.changed(index, host)
    }
    fn on_take<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        index: usize,
        host: &mut H,
    ) -> Result<()> {
        if index != RESULT_SLOT {
            return self.changed(index, host);
        }
        let bound = self.state.experience_bound(host)?;
        let xp = if bound > 0 {
            let draw = host.level_random_next_int(bound)?;
            ensure!(
                (0..bound).contains(&draw),
                "invalid actual Level RNG sample"
            );
            bound.wrapping_add(draw)
        } else {
            0
        };
        host.award_experience_orbs(xp, &self.phase(host))?;
        host.level_event(1042, 0, &self.phase(host))?;
        for index in 0..2 {
            self.state.set_container_input(index, ItemStack::empty())?;
            self.changed(index, host)?; // container setter, not Slot.set
        }
        Ok(())
    }
    fn try_remove<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        index: usize,
        amount: i32,
        maximum: i32,
        host: &mut H,
    ) -> Result<ItemStack> {
        let source = self.slot(index, host);
        if source.is_empty() || index == RESULT_SLOT && maximum < source.count {
            return Ok(ItemStack::empty());
        }
        // ResultContainer.removeItem ignores amount, even for right-click or
        // THROW-one. Slot.tryRemove checks destination capacity first.
        let taken = if index == RESULT_SLOT {
            source.clone()
        } else {
            counted(&source, amount.min(maximum).min(source.count))
        };
        if taken.is_empty() {
            return Ok(taken);
        }
        self.mutate_raw(
            index,
            counted(&source, source.count.wrapping_sub(taken.count)),
            host,
        )?;
        if index < 2 {
            self.changed(index, host)?;
        }
        if self.slot(index, host).is_empty() {
            self.set_slot(index, ItemStack::empty(), host)?;
        }
        Ok(taken)
    }
    fn safe_take<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        index: usize,
        amount: i32,
        maximum: i32,
        host: &mut H,
    ) -> Result<ItemStack> {
        let taken = self.try_remove(index, amount, maximum, host)?;
        if !taken.is_empty() {
            self.on_take(index, host)?;
        }
        Ok(taken)
    }
    fn drop<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &self,
        stack: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if !stack.is_empty() {
            host.stage_drop(
                DroppedStack {
                    stack,
                    around: false,
                    trace_item: true,
                },
                &self.phase(host),
            )?;
        }
        Ok(())
    }
    fn do_click<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        click: &ContainerClick,
        host: &mut H,
    ) -> Result<()> {
        let slot = click.slot as i32;
        let button = click.button as i32;
        let creative = host.instabuild();
        if click.kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, creative, host);
        }
        if self.drag_status != 0 {
            self.reset_drag();
            return Ok(());
        }
        match click.kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let source = self.carried().clone();
                    let n = if button == 0 {
                        source.count
                    } else {
                        1.min(source.count)
                    };
                    if button == 1 {
                        // Source carried.split(1) precedes the right-drop call.
                        self.sync
                            .set_carried(counted(&source, source.count.wrapping_sub(n)));
                        self.drop(counted(&source, n), host)?;
                    } else {
                        self.drop(counted(&source, n), host)?;
                        self.sync.set_carried(ItemStack::empty());
                    }
                } else if slot >= 0 {
                    if click.kind == ClickType::QuickMove {
                        loop {
                            let before = self.slot(slot as usize, host);
                            if !self.quick_move(slot as usize, host)?
                                || self.slot(slot as usize, host).item_id != before.item_id
                            {
                                break;
                            }
                        }
                    } else {
                        self.pickup(slot as usize, button, host)?;
                    }
                }
            }
            ClickType::Swap if ((0..9).contains(&button) || button == 40) && slot >= 0 => {
                self.swap(slot as usize, button, host)?
            }
            ClickType::Clone if creative && slot >= 0 && self.carried().is_empty() => {
                let item = self.slot(slot as usize, host);
                self.sync
                    .set_carried(counted(&item, item.max_stack_size()?));
            }
            ClickType::Throw if slot >= 0 && self.carried().is_empty() => {
                let n = if button == 0 {
                    1
                } else {
                    self.slot(slot as usize, host).count
                };
                let taken = self.safe_take(slot as usize, n, i32::MAX, host)?;
                self.drop(taken, host)?;
            }
            ClickType::PickupAll if slot >= 0 => self.pickup_all(slot as usize, button, host)?,
            _ => {}
        }
        Ok(())
    }
    fn pickup<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        index: usize,
        button: i32,
        host: &mut H,
    ) -> Result<()> {
        let source = self.slot(index, host);
        let cursor = self.carried().clone();
        ensure!(
            inventory::item_name(source.item_id) != Some("bundle")
                && inventory::item_name(cursor.item_id) != Some("bundle"),
            "bundle click overrides require actual bundle owner"
        );
        if source.is_empty() {
            if !cursor.is_empty() && Self::may_place(index, &cursor)? {
                let n =
                    (if button == 0 { cursor.count } else { 1 }).min(Self::limit(index, &cursor)?);
                self.sync
                    .set_carried(counted(&cursor, cursor.count.wrapping_sub(n)));
                self.set_slot(index, counted(&cursor, n), host)?;
            }
        } else if cursor.is_empty() {
            let amount = if button == 0 {
                source.count
            } else {
                source.count.wrapping_add(1) / 2
            };
            let taken = self.try_remove(index, amount, i32::MAX, host)?;
            if !taken.is_empty() {
                self.sync.set_carried(taken);
                self.on_take(index, host)?;
            }
        } else if Self::may_place(index, &cursor)? {
            if source.same_item_same_components(&cursor)? {
                let n = (if button == 0 { cursor.count } else { 1 })
                    .min(Self::limit(index, &cursor)? - source.count);
                // Slot.safeInsert retains negative capacity for an admitted
                // overfull stack: shrink(cursor,n) grows that cursor while
                // grow(source,n) restores the slot to its limit. It also calls
                // setByPlayer when a full slot accepts exactly zero items.
                self.sync
                    .set_carried(counted(&cursor, cursor.count.wrapping_sub(n)));
                self.set_slot(index, counted(&source, source.count.wrapping_add(n)), host)?;
            } else if cursor.count <= Self::limit(index, &cursor)? {
                self.sync.set_carried(source);
                self.set_slot(index, cursor, host)?;
            }
        } else if source.same_item_same_components(&cursor)? {
            let taken = self.try_remove(
                index,
                source.count,
                cursor.max_stack_size()? - cursor.count,
                host,
            )?;
            if !taken.is_empty() {
                self.sync
                    .set_carried(counted(&cursor, cursor.count.wrapping_add(taken.count)));
                self.on_take(index, host)?;
            }
        }
        self.changed(index, host) // unconditional Slot.setChanged after Pickup
    }
    fn move_to<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        source_index: usize,
        stack: &mut ItemStack,
        range: std::ops::Range<usize>,
        reverse: bool,
        host: &mut H,
    ) -> Result<bool> {
        let before = stack.count;
        let order = if reverse {
            range.rev().collect::<Vec<_>>()
        } else {
            range.collect()
        };
        if stack.is_stackable()? {
            for &index in &order {
                let old = self.slot(index, host);
                if !old.is_empty() && old.same_item_same_components(stack)? {
                    let n = stack
                        .count
                        .min((Self::limit(index, &old)? - old.count).max(0));
                    if n > 0 {
                        *stack = counted(stack, stack.count.wrapping_sub(n));
                        self.mutate_raw(source_index, stack.clone(), host)?;
                        self.mutate_raw(index, counted(&old, old.count.wrapping_add(n)), host)?;
                        self.changed(index, host)?;
                    }
                }
                if stack.is_empty() {
                    break;
                }
            }
        }
        if !stack.is_empty() {
            for index in order {
                if self.slot(index, host).is_empty() && Self::may_place(index, stack)? {
                    let n = stack.count.min(Self::limit(index, stack)?);
                    let moved = counted(stack, n);
                    *stack = counted(stack, stack.count.wrapping_sub(n));
                    self.mutate_raw(source_index, stack.clone(), host)?;
                    self.set_slot(index, moved, host)?;
                    self.changed(index, host)?;
                    break;
                }
            }
        }
        Ok(before != stack.count)
    }
    fn quick_move<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        index: usize,
        host: &mut H,
    ) -> Result<bool> {
        let source = self.slot(index, host);
        let mut left = source.clone();
        let moved = if index == RESULT_SLOT {
            self.move_to(index, &mut left, 3..MENU_SLOTS, true, host)?
        } else if index < 2 {
            self.move_to(index, &mut left, 3..MENU_SLOTS, false, host)?
        } else if self.state.inputs().iter().any(ItemStack::is_empty) {
            self.move_to(index, &mut left, 0..2, false, host)?
        } else if index < 30 {
            self.move_to(index, &mut left, 30..MENU_SLOTS, false, host)?
        } else {
            self.move_to(index, &mut left, 3..30, false, host)?
        };
        if !moved {
            return Ok(false);
        }
        if left.is_empty() {
            self.set_slot(index, ItemStack::empty(), host)?;
        } else {
            self.changed(index, host)?;
        }
        self.on_take(index, host)?; // actual post-transfer result ownership
        Ok(true)
    }
    fn swap<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        index: usize,
        button: i32,
        host: &mut H,
    ) -> Result<()> {
        let player_index = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        let held = host
            .inventory()
            .slot(player_index)
            .expect("hotbar/offhand")
            .clone();
        let source = self.slot(index, host);
        if held.is_empty() && source.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            host.inventory_mut()
                .set_slot_authoritative(player_index, source);
            self.set_slot(index, ItemStack::empty(), host)?;
            self.on_take(index, host)?;
        } else if Self::may_place(index, &held)? {
            let maximum = Self::limit(index, &held)?;
            if source.is_empty() {
                host.inventory_mut().set_slot_authoritative(
                    player_index,
                    counted(&held, held.count.wrapping_sub(maximum.min(held.count))),
                );
                self.set_slot(index, counted(&held, maximum.min(held.count)), host)?;
            } else if held.count > maximum {
                host.inventory_mut().set_slot_authoritative(
                    player_index,
                    counted(&held, held.count.wrapping_sub(maximum)),
                );
                self.set_slot(index, counted(&held, maximum), host)?;
                self.on_take(index, host)?;
                let mut displaced = if index > RESULT_SLOT && index + 6 == player_index {
                    // Java source/held locals alias the pre-split stack. After
                    // split the displaced local references only its remainder.
                    counted(&source, source.count.wrapping_sub(maximum))
                } else {
                    source
                };
                if !host.add_to_player_inventory(&mut displaced, &self.phase(host))? {
                    self.drop(displaced, host)?;
                }
            } else {
                host.inventory_mut()
                    .set_slot_authoritative(player_index, source);
                self.set_slot(index, held, host)?;
                self.on_take(index, host)?;
            }
        }
        Ok(())
    }
    fn pickup_all<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        clicked: usize,
        button: i32,
        host: &mut H,
    ) -> Result<()> {
        if self.carried().is_empty() || !self.slot(clicked, host).is_empty() {
            return Ok(());
        }
        let order = if button == 0 {
            (0..MENU_SLOTS).collect::<Vec<_>>()
        } else {
            (0..MENU_SLOTS).rev().collect()
        };
        for pass in 0..2 {
            for &index in &order {
                let source = self.slot(index, host);
                let cursor = self.carried().clone();
                if cursor.count >= cursor.max_stack_size()? {
                    break;
                }
                if source.is_empty()
                    || !source.same_item_same_components(&cursor)?
                    || source.count > cursor.max_stack_size()?
                    || pass == 0 && source.count == source.max_stack_size()?
                {
                    continue;
                }
                let taken = self.safe_take(
                    index,
                    source.count,
                    cursor.max_stack_size()? - cursor.count,
                    host,
                )?;
                self.sync
                    .set_carried(counted(&cursor, cursor.count.wrapping_add(taken.count)));
            }
        }
        Ok(())
    }
    fn quickcraft<H: GrindstoneMenuHost<Binding = K, RegistrySnapshot = S>>(
        &mut self,
        slot: i32,
        button: i32,
        creative: bool,
        host: &mut H,
    ) -> Result<()> {
        let previous = self.drag_status;
        self.drag_status = (button & 3) as u8;
        if !(previous == 1 && self.drag_status == 2) && previous != self.drag_status {
            self.reset_drag();
            return Ok(());
        }
        if self.carried().is_empty() {
            self.reset_drag();
            return Ok(());
        }
        match self.drag_status {
            0 => {
                self.drag_type = ((button >> 2) & 3) as u8;
                if self.drag_type < 2 || self.drag_type == 2 && creative {
                    self.drag_status = 1;
                    self.drag_slots.clear();
                } else {
                    self.reset_drag();
                }
            }
            1 => {
                if slot >= 0 {
                    let index = slot as usize;
                    let old = self.slot(index, host);
                    let cursor = self.carried();
                    if (old.is_empty()
                        || old.same_item_same_components(cursor)?
                            && old.count <= cursor.max_stack_size()?)
                        && Self::may_place(index, cursor)?
                        && (self.drag_type == 2 || cursor.count > self.drag_slots.len() as i32)
                    {
                        self.drag_slots.insert(index);
                    }
                }
            }
            2 => {
                if self.drag_slots.is_empty() {
                    self.reset_drag();
                    return Ok(());
                }
                if self.drag_slots.len() == 1 {
                    let index = *self.drag_slots.first().unwrap();
                    let kind = self.drag_type as i32;
                    self.reset_drag();
                    // Source recursively dispatches PICKUP with this button;
                    // only 0/1 pass that dispatcher's guard. Creative type2
                    // with one selected slot consequently does nothing.
                    return if kind <= 1 {
                        self.pickup(index, kind, host)
                    } else {
                        Ok(())
                    };
                }
                let cursor = self.carried().clone();
                let mut remaining = cursor.count;
                let selected = self.drag_slots.iter().copied().collect::<Vec<_>>();
                let order = host.quickcraft_iteration_order(&selected)?;
                ensure!(
                    order.len() == selected.len()
                        && order.iter().copied().collect::<BTreeSet<_>>() == self.drag_slots,
                    "invalid actual quickcraft encounter order"
                );
                let each = match self.drag_type {
                    0 => cursor.count / self.drag_slots.len().max(1) as i32,
                    1 => 1,
                    2 => cursor.max_stack_size()?,
                    _ => 0,
                };
                for index in order {
                    let old = self.slot(index, host);
                    if (!old.is_empty()
                        && (!old.same_item_same_components(&cursor)?
                            || old.count > cursor.max_stack_size()?))
                        || !Self::may_place(index, &cursor)?
                        || self.drag_type != 2 && cursor.count < self.drag_slots.len() as i32
                    {
                        continue;
                    }
                    let count = each
                        .wrapping_add(old.count)
                        .min(Self::limit(index, &cursor)?);
                    remaining = remaining.wrapping_sub(count.wrapping_sub(old.count));
                    self.set_slot(index, counted(&cursor, count), host)?;
                }
                self.sync.set_carried(counted(&cursor, remaining));
                self.reset_drag();
            }
            _ => self.reset_drag(),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crafting::RecipeBook, inventory::Component, protocol::Writer, random::LegacyRandomSource,
    };
    use std::collections::BTreeMap;

    #[derive(Clone, Debug)]
    struct Record {
        name: &'static str,
        value: i32,
        phase: GrindstonePhase,
    }
    /// Restricted recording fixture: it exercises native controller ownership
    /// and rollback; it is NOT a production Level, registry or orb host.
    #[derive(Clone)]
    struct Host {
        inventory: PlayerInventory,
        binding: u64,
        registry: u64,
        creative: bool,
        random: LegacyRandomSource,
        records: Vec<Record>,
        generation: u64,
        base_generation: u64,
        fail_orbs: bool,
        fail_publish: bool,
        sync_events: Vec<(Vec<SyncEvent>, GrindstonePhase)>,
    }
    impl Host {
        fn new() -> Self {
            Self {
                inventory: PlayerInventory::default(),
                binding: 1,
                registry: 1,
                creative: false,
                random: LegacyRandomSource::new(0),
                records: vec![],
                generation: 0,
                base_generation: 0,
                fail_orbs: false,
                fail_publish: false,
                sync_events: vec![],
            }
        }
        fn record(&mut self, name: &'static str, value: i32, phase: &GrindstonePhase) {
            self.records.push(Record {
                name,
                value,
                phase: phase.clone(),
            });
        }
    }
    fn enchantment_id(name: &str) -> i32 {
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../tools/crafting_metadata.json")).unwrap();
        metadata["registries"]["minecraft:enchantment"][format!("minecraft:{name}")]
            .as_i64()
            .unwrap() as i32
    }
    impl GrindstoneEnchantments for Host {
        fn is_curse(&self, id: i32) -> Result<bool> {
            Ok(id == enchantment_id("vanishing_curse"))
        }
        fn minimum_cost(&self, id: i32, level: i32) -> Result<i32> {
            ensure!(
                id == enchantment_id("sharpness") && level == 1,
                "fixture unresolved cost"
            );
            Ok(1)
        }
    }
    impl GrindstoneMenuHost for Host {
        type Binding = u64;
        type RegistrySnapshot = u64;
        fn binding(&self) -> &u64 {
            &self.binding
        }
        fn registry_snapshot(&self) -> u64 {
            self.registry
        }
        fn begin_stage(&self, _: GrindstoneMenuToken, _: GrindstoneMenuAction) -> Result<Self> {
            let mut stage = self.clone();
            stage.base_generation = self.generation;
            Ok(stage)
        }
        fn publish_stage(&mut self, mut stage: Self, _: GrindstoneMenuToken) -> Result<()> {
            ensure!(!self.fail_publish, "fixture atomic publication unavailable");
            ensure!(
                stage.base_generation == self.generation,
                "fixture owner revision changed"
            );
            stage.generation += 1;
            *self = stage;
            Ok(())
        }
        fn preflight(&mut self, _: GrindstoneMenuAction) -> Result<()> {
            Ok(())
        }
        fn inventory(&self) -> &PlayerInventory {
            &self.inventory
        }
        fn inventory_mut(&mut self) -> &mut PlayerInventory {
            &mut self.inventory
        }
        fn instabuild(&self) -> bool {
            self.creative
        }
        fn still_valid_grindstone(&self, range: f64) -> Result<bool> {
            assert_eq!(range, 4.0);
            Ok(true)
        }
        fn input_container_set_changed(&mut self, phase: &GrindstonePhase) -> Result<()> {
            self.record("input", 0, phase);
            Ok(())
        }
        fn player_inventory_set_changed(&mut self, phase: &GrindstonePhase) -> Result<()> {
            self.record("inventory", 0, phase);
            Ok(())
        }
        fn stage_sync_events(
            &mut self,
            events: Vec<SyncEvent>,
            phase: &GrindstonePhase,
        ) -> Result<()> {
            self.record("broadcast", events.len() as i32, phase);
            self.sync_events.push((events, phase.clone()));
            Ok(())
        }
        fn level_random_next_int(&mut self, bound: i32) -> Result<i32> {
            // The real algorithm, isolated within this fixture transaction.
            Ok(self.random.next_bounded_i32(bound)?)
        }
        fn award_experience_orbs(&mut self, xp: i32, phase: &GrindstonePhase) -> Result<()> {
            self.record("orbs", xp, phase);
            ensure!(!self.fail_orbs, "fixture orb failure");
            Ok(())
        }
        fn level_event(&mut self, event: i32, data: i32, phase: &GrindstonePhase) -> Result<()> {
            assert_eq!((event, data), (1042, 0));
            self.record("event", event, phase);
            Ok(())
        }
        fn stage_drop(&mut self, item: DroppedStack, phase: &GrindstonePhase) -> Result<()> {
            assert!(!item.around);
            self.record(
                if item.trace_item {
                    "throw"
                } else {
                    "close-drop"
                },
                item.stack.count,
                phase,
            );
            Ok(())
        }
        fn add_to_player_inventory(
            &mut self,
            stack: &mut ItemStack,
            phase: &GrindstonePhase,
        ) -> Result<bool> {
            self.place_item_back(stack, phase)?;
            Ok(stack.is_empty())
        }
        fn place_item_back(
            &mut self,
            stack: &mut ItemStack,
            phase: &GrindstonePhase,
        ) -> Result<()> {
            self.record("return", stack.count, phase);
            let mut out = crate::container::ClickOutcome::default();
            crate::container::return_to_inventory_on_menu_close(
                &mut self.inventory,
                stack,
                &mut out,
            )?;
            for item in out.drops {
                self.stage_drop(item, phase)?;
            }
            Ok(())
        }
        fn quickcraft_iteration_order(&self, selected: &[usize]) -> Result<Vec<usize>> {
            Ok(selected.to_vec())
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn enchanted(name: &str, enchantment: &str) -> ItemStack {
        let mut item = item(name, 1);
        let mut w = Writer::new();
        w.varint(1)
            .varint(enchantment_id(enchantment))
            .varint(1)
            .bool(true);
        item.components.push(Component {
            type_id: if name == "enchanted_book" { 23 } else { 9 },
            data: w.into_inner(),
        });
        item
    }
    fn click(
        menu: &GrindstoneMenu<u64, u64>,
        slot: i16,
        button: i8,
        kind: ClickType,
    ) -> ContainerClick {
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
    fn act(
        menu: &mut GrindstoneMenu<u64, u64>,
        host: &mut Host,
        slot: i16,
        button: i8,
        kind: ClickType,
    ) -> Result<()> {
        menu.click(menu.token(), &click(menu, slot, button, kind), host)
    }
    fn pair() -> Result<(GrindstoneMenu<u64, u64>, Host)> {
        let mut host = Host::new();
        let stick = enchanted("stick", "vanishing_curse");
        host.inventory.set_slot_authoritative(9, stick.clone());
        host.inventory.set_slot_authoritative(10, stick);
        let mut menu = GrindstoneMenu::new(5, &host)?;
        menu.initialize(&mut host)?;
        act(&mut menu, &mut host, 3, 0, ClickType::QuickMove)?;
        // A second shift would merge identical stackable inputs into slot0.
        // Place it explicitly into slot1 to obtain the count-two result.
        act(&mut menu, &mut host, 4, 0, ClickType::Pickup)?;
        act(&mut menu, &mut host, 1, 0, ClickType::Pickup)?;
        assert_eq!(menu.state.preview().count, 2);
        host.records.clear();
        Ok((menu, host))
    }
    #[test]
    fn whole_right_result_and_partial_shift_preserve_preview_and_callback_ownership() -> Result<()>
    {
        let (mut menu, mut host) = pair()?;
        act(&mut menu, &mut host, 2, 1, ClickType::Pickup)?;
        assert_eq!(menu.carried().count, 2);
        assert!(menu.state.inputs().iter().all(ItemStack::is_empty));
        assert!(menu.state.preview().is_empty());
        let names = host.records.iter().map(|e| e.name).collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "orbs",
                "event",
                "input",
                "broadcast",
                "broadcast",
                "input",
                "broadcast",
                "broadcast",
                "broadcast"
            ]
        );
        let orb = &host.records[0];
        assert_eq!(orb.value, 0);
        assert_eq!(orb.phase.carried.count, 2);
        assert!(orb.phase.preview.is_empty());
        assert!(orb.phase.inputs.iter().all(|i| i.count == 1));
        // First clear broadcasts old EMPTY result, then recomputes a single
        // curse-only input. Second clear first broadcasts that old preview.
        assert!(host.records[3].phase.preview.is_empty());
        assert_eq!(host.records[4].phase.preview.count, 1);
        assert_eq!(host.records[6].phase.preview.count, 1);
        assert!(host.records[7].phase.preview.is_empty());

        let (mut menu, mut host) = pair()?;
        host.inventory
            .set_slot_authoritative(9, counted(menu.state.preview(), 63));
        act(&mut menu, &mut host, 3, 0, ClickType::Pickup)?;
        host.records.clear();
        act(&mut menu, &mut host, 2, 0, ClickType::Pickup)?;
        assert_eq!(menu.carried().count, 63); // room1 cannot accept whole result2
        assert_eq!(menu.state.preview().count, 2);
        assert!(!host.records.iter().any(|e| e.name == "orbs"));

        let (mut menu, mut host) = pair()?;
        for index in 9..45 {
            host.inventory
                .set_slot_authoritative(index, item("dirt", 64));
        }
        host.inventory
            .set_slot_authoritative(44, counted(menu.state.preview(), 63));
        act(&mut menu, &mut host, 2, 0, ClickType::QuickMove)?;
        assert_eq!(host.inventory.slot(44).unwrap().count, 64);
        let orb = host.records.iter().find(|e| e.name == "orbs").unwrap();
        assert_eq!(orb.phase.preview.count, 1); // untransferred derived remainder
        assert!(orb.phase.carried.is_empty());
        assert!(menu.state.inputs().iter().all(ItemStack::is_empty));
        assert!(menu.state.preview().is_empty());

        let (mut menu, mut host) = pair()?;
        menu.close(menu.token(), MenuCloseContext::AliveConnected, &mut host)?;
        assert_eq!(
            (9..46)
                .map(|n| host.inventory.slot(n).unwrap().count)
                .sum::<i32>(),
            2
        );
        assert!(!host.records.iter().any(|e| e.name == "orbs"));
        assert!(menu.state.preview().is_empty()); // never return the count2 preview
        Ok(())
    }
    #[test]
    fn xp_throw_and_world_failure_are_one_staged_transaction_with_stale_ui_guards() -> Result<()> {
        let mut host = Host::new();
        host.inventory
            .set_slot_authoritative(9, enchanted("enchanted_book", "sharpness"));
        let mut menu = GrindstoneMenu::new(4, &host)?;
        menu.initialize(&mut host)?;
        act(&mut menu, &mut host, 3, 0, ClickType::QuickMove)?;
        let token = menu.token();
        let random = host.random.state48();
        let inputs = menu.state.inputs().clone();
        let records = host.records.len();
        host.fail_orbs = true;
        assert!(act(&mut menu, &mut host, 2, 0, ClickType::Throw).is_err());
        assert_eq!(menu.token(), token);
        assert_eq!(menu.state.inputs(), &inputs);
        assert_eq!(host.random.state48(), random);
        assert_eq!(host.records.len(), records);
        host.fail_orbs = false;
        host.records.clear();
        act(&mut menu, &mut host, 2, 0, ClickType::Throw)?;
        assert_ne!(host.random.state48(), random); // bound1 still consumes Level RNG
        assert_eq!(host.records[0].name, "orbs");
        assert_eq!(host.records[0].value, 1);
        assert!(host.records[0].phase.carried.is_empty());
        let dropped = host.records.iter().position(|e| e.name == "throw").unwrap();
        assert!(dropped > host.records.iter().position(|e| e.name == "event").unwrap());
        assert!(host.records[dropped]
            .phase
            .inputs
            .iter()
            .all(ItemStack::is_empty));
        assert!(host.records[dropped].phase.preview.is_empty());
        assert_eq!(host.records[dropped].value, 1);
        assert!(menu
            .click(token, &click(&menu, 2, 0, ClickType::Throw), &mut host)
            .is_err());
        let mut reopened = GrindstoneMenu::new(4, &host)?;
        assert_ne!(reopened.token().instance, menu.token().instance);
        assert!(reopened
            .click(token, &click(&reopened, 2, 0, ClickType::Throw), &mut host)
            .is_err());
        host.registry = 2;
        assert!(menu.still_valid(&host).is_err());
        host.registry = 1;
        host.binding = 2;
        assert!(menu.still_valid(&host).is_err());
        Ok(())
    }
    #[test]
    fn ordinary_modes_close_and_outside_drop_keep_one_inventory_and_exact_cursor_phases(
    ) -> Result<()> {
        let (mut menu, mut host) = pair()?;
        // SWAP removes whole result into the actual empty hotbar before XP.
        act(&mut menu, &mut host, 2, 0, ClickType::Swap)?;
        assert_eq!(host.inventory.slot(36).unwrap().count, 2);
        assert_eq!(
            host.records
                .iter()
                .find(|e| e.name == "orbs")
                .unwrap()
                .phase
                .player_inventory
                .slot(36)
                .unwrap()
                .count,
            2
        );
        act(&mut menu, &mut host, 30, 0, ClickType::Pickup)?;
        act(&mut menu, &mut host, -999, 1, ClickType::Pickup)?;
        let event = host
            .records
            .iter()
            .rev()
            .find(|e| e.name == "throw")
            .unwrap();
        assert_eq!((event.value, event.phase.carried.count), (1, 1));
        act(&mut menu, &mut host, -999, 0, ClickType::Pickup)?;
        let event = host
            .records
            .iter()
            .rev()
            .find(|e| e.name == "throw")
            .unwrap();
        assert_eq!((event.value, event.phase.carried.count), (1, 1));
        assert!(menu.carried().is_empty());
        // QUICK_CRAFT uses captured encounter-order owner, then PICKUP_ALL
        // collects only real slots while no stale result is retained.
        host.inventory
            .set_slot_authoritative(9, enchanted("stick", "vanishing_curse"));
        let mut stack = host.inventory.slot(9).unwrap().clone();
        stack.count = 4;
        host.inventory.set_slot_authoritative(9, stack);
        act(&mut menu, &mut host, 3, 0, ClickType::Pickup)?;
        for (slot, button) in [(-999, 0), (0, 1), (1, 1), (-999, 2)] {
            act(&mut menu, &mut host, slot, button, ClickType::QuickCraft)?;
        }
        assert_eq!(
            menu.state
                .inputs()
                .iter()
                .map(|i| i.count)
                .collect::<Vec<_>>(),
            [2, 2]
        );
        act(&mut menu, &mut host, 0, 1, ClickType::Pickup)?;
        act(&mut menu, &mut host, 3, 0, ClickType::PickupAll)?;
        assert_eq!(menu.carried().count, 4);
        assert!(menu.state.inputs().iter().all(ItemStack::is_empty));
        // Source creative CLONE intentionally copies a preview without taking
        // inputs or awarding XP. Removal returns cursor and inputs separately.
        act(&mut menu, &mut host, 3, 0, ClickType::Pickup)?;
        act(&mut menu, &mut host, 3, 0, ClickType::QuickMove)?; // count4 input suppresses preview
        act(&mut menu, &mut host, 0, 1, ClickType::Pickup)?; // cursor2
        act(&mut menu, &mut host, 4, 0, ClickType::Pickup)?;
        host.creative = true;
        host.records.clear();
        act(&mut menu, &mut host, 0, 2, ClickType::Clone)?;
        assert_eq!(menu.carried().count, 64);
        assert_eq!(menu.state.inputs()[0].count, 2);
        assert!(!host.records.iter().any(|e| e.name == "orbs"));
        menu.close(menu.token(), MenuCloseContext::AliveConnected, &mut host)?;
        assert!(menu.carried().is_empty());
        assert!(menu.state.inputs().iter().all(ItemStack::is_empty));
        assert!(menu
            .click(
                menu.token(),
                &click(&menu, 0, 0, ClickType::Pickup),
                &mut host
            )
            .is_err());
        Ok(())
    }

    #[test]
    fn full_state_resync_retains_preview_cursor_drag_and_rolls_back_rejected_publication(
    ) -> Result<()> {
        let (mut menu, mut host) = pair()?;
        host.inventory.set_slot_authoritative(11, item("stone", 4));
        act(&mut menu, &mut host, 5, 0, ClickType::Pickup)?;
        act(&mut menu, &mut host, -999, 0, ClickType::QuickCraft)?;
        act(&mut menu, &mut host, 6, 1, ClickType::QuickCraft)?;
        let retained = menu.staged_copy();
        let inventory = host.inventory.clone();
        let random = host.random.state48();
        let generation = host.generation;
        let records = host.records.len();
        let queued_sync = host.sync_events.len();
        host.fail_publish = true;
        assert!(menu.broadcast_full_state(&mut host).is_err());
        assert_eq!(menu.token(), retained.token());
        assert_eq!(menu.state, retained.state);
        assert_eq!(menu.sync, retained.sync);
        assert_eq!(menu.drag_status, retained.drag_status);
        assert_eq!(menu.drag_type, retained.drag_type);
        assert_eq!(menu.drag_slots, retained.drag_slots);
        assert_eq!(host.inventory.slots(), inventory.slots());
        assert_eq!(host.inventory.carried(), inventory.carried());
        assert_eq!(host.generation, generation);
        assert_eq!(host.random.state48(), random);
        assert_eq!(host.records.len(), records);
        assert_eq!(host.sync_events.len(), queued_sync);
        host.fail_publish = false;
        menu.broadcast_full_state(&mut host)?;
        assert_eq!(menu.token().instance, retained.token().instance);
        assert_eq!(menu.token().revision, retained.token().revision + 1);
        assert_eq!(menu.state, retained.state);
        assert_eq!(menu.state.preview().count, 2);
        assert_eq!(menu.carried(), &item("stone", 4));
        assert_eq!(menu.drag_status, retained.drag_status);
        assert_eq!(menu.drag_type, retained.drag_type);
        assert_eq!(menu.drag_slots, retained.drag_slots);
        assert_eq!(menu.state_id(), (retained.state_id() + 1) & 32767);
        assert_eq!(host.random.state48(), random);
        assert_eq!(host.records.len(), records + 1);
        assert_eq!(host.records[records].name, "broadcast");
        assert_eq!(host.sync_events.len(), queued_sync + 1);
        let (events, phase) = host.sync_events.last().unwrap();
        assert_eq!(phase.inputs, *retained.state.inputs());
        assert_eq!(phase.preview, *retained.state.preview());
        assert_eq!(&phase.carried, retained.carried());
        assert_eq!(phase.player_inventory.slots(), inventory.slots());
        assert_eq!(phase.player_inventory.carried(), inventory.carried());
        let packet = events
            .iter()
            .find_map(|event| match event {
                SyncEvent::Packet(packet) if packet.id == 0x13 => Some(packet),
                _ => None,
            })
            .expect("retained full-content packet");
        let mut reader = crate::protocol::Reader::new(&packet.payload);
        assert_eq!(reader.u8()?, menu.window_id());
        assert_eq!(reader.varint()?, menu.state_id());
        assert_eq!(reader.varint()?, 39);
        let top = retained.top();
        for slot in 0..39 {
            let (stack, used) = ItemStack::decode(reader.remaining())?;
            reader.bytes(used)?;
            let expected = if slot < 3 {
                &top[slot]
            } else {
                inventory.slot(slot + 6).unwrap()
            };
            assert_eq!(&stack, expected);
        }
        assert_eq!(
            ItemStack::decode_exact(reader.remaining())?,
            item("stone", 4)
        );
        let old_click = click(&menu, -999, 0, ClickType::Pickup);
        assert!(menu.click(retained.token(), &old_click, &mut host).is_err());
        assert!(menu
            .close(
                retained.token(),
                MenuCloseContext::AliveConnected,
                &mut host
            )
            .is_err());
        assert_eq!(host.sync_events.len(), queued_sync + 1);
        menu.broadcast_changes(&mut host)?;
        assert!(host.sync_events.last().unwrap().0.is_empty());
        assert_eq!(menu.state, retained.state);
        assert_eq!(host.inventory.slots(), inventory.slots());
        Ok(())
    }
}
