//! Bounded Java 1.21 AnvilMenu over its actual temporary inputs and borrowed
//! canonical player inventory. Every host owner is staged together; on error
//! discard that entire detached stage and private menu copy before publication. The result
//! is a derived preview, never a second output owner. See the source ledger.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    nbt::JavaString,
    workstations::{
        anvil::{
            finish_anvil_take, AnvilComputationChange, AnvilContext, AnvilEnchantmentSession,
            AnvilEnchantmentSnapshot, AnvilPlayer, AnvilState, AnvilWorldAccess,
        },
        menu::{
            require_component_context, MenuRemovalEvent, MenuSync, SyncEvent, WorkstationAccess,
            WorkstationKind,
        },
    },
};
use anyhow::{ensure, Result};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
pub const INPUT_SLOTS: usize = 2;
pub const RESULT_SLOT: usize = 2;
pub const MENU_SLOTS: usize = 39;

/// Captured native dispatch identity; neither a reused protocol window nor
/// equal contents can revive an action after another revision or reopening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnvilMenuToken {
    instance: u64,
    revision: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum AnvilMenuAction {
    Initialize,
    Broadcast,
    FullState,
    Rename,
    Click(ClickType),
    Close(MenuCloseContext),
}
/// Read-only actual menu phase supplied while staging a callback. This is not
/// another inventory/result owner. Host inventory getters always refer to the
/// same canonical player inventory, already at this phase of the action.
#[derive(Clone, Debug)]
pub struct AnvilPhase {
    pub inputs: [ItemStack; INPUT_SLOTS],
    pub result_preview: ItemStack,
    pub carried: ItemStack,
    pub level_cost: i32,
    pub repair_material_count: i32,
}
/// Implement using a single staged actual player/world/menu transaction. The
/// binding identifies actual player + captured Level/position + menu lifetime;
/// equal inventory contents do not establish identity. Unsupported/reentrant
/// listeners must be rejected by preflight, not replayed against final state.
pub trait AnvilMenuHost: AnvilContext + AnvilWorldAccess + WorkstationAccess + Sized {
    type Binding: Clone + Eq + Debug;
    fn binding(&self) -> &Self::Binding;
    /// Isolate EVERY mutable real owner: inventory, XP, player RNG, captured
    /// world/block, drop/entity allocation, callbacks and ordered packet outbox.
    /// A shallow Clone of live Arc/Mutex owners is not a detached transaction.
    fn begin_stage(&self, token: AnvilMenuToken, action: AnvilMenuAction) -> Result<Self>;
    /// Atomically validate the captured actual owner revision/token and publish
    /// ALL staged owners. Err must publish nothing. Do not drain/send the outbox
    /// here: dispatch it only after the outer lock/transaction and this action
    /// have returned. The controller installs its private UI infallibly after
    /// successful publication; no fallible work may follow that publication.
    fn publish_stage(&mut self, staged: Self, token: AnvilMenuToken) -> Result<()>;
    fn inventory(&self) -> &PlayerInventory;
    fn inventory_mut(&mut self) -> &mut PlayerInventory;
    fn player(&self) -> Result<AnvilPlayer>;
    /// Validate actor/window/spectator/current actual owners. Rename and Click
    /// also use the controller's actual block/reach validity check.
    fn preflight_menu_action(&mut self, action: AnvilMenuAction) -> Result<()>;
    /// Install a read-only view for actual item/text/XP/world callback owners.
    /// Do not replace live ownership or allow reentry through this view.
    fn observe_menu_phase(&mut self, phase: &AnvilPhase) -> Result<()>;
    fn stage_sync_events(&mut self, events: Vec<SyncEvent>, phase: &AnvilPhase) -> Result<()>;
    /// Actual SimpleContainer.setChanged listener phase, before slotsChanged.
    fn input_container_set_changed(&mut self, phase: &AnvilPhase) -> Result<()>;
    /// Actual Inventory.setChanged/timesChanged, distinct from SlotListener.
    fn player_inventory_set_changed(&mut self, phase: &AnvilPhase) -> Result<()>;
    /// Actual Player.giveExperienceLevels (including its actual XP fields).
    fn give_experience_levels(&mut self, delta: i32, phase: &AnvilPhase) -> Result<()>;
    /// Actual Inventory.add mutates this one owned stack to its remainder.
    fn add_to_player_inventory(
        &mut self,
        stack: &mut ItemStack,
        phase: &AnvilPhase,
    ) -> Result<bool>;
    fn stage_drop(&mut self, owned: DroppedStack, phase: &AnvilPhase) -> Result<()>;
    /// Actual Slot-identity HashSet encounter order, exactly a permutation.
    fn quickcraft_iteration_order(&self, selected: &[usize]) -> Result<Vec<usize>>;
}
#[derive(Debug)]
pub struct AnvilMenu<K> {
    binding: K,
    enchantments: Option<AnvilEnchantmentSnapshot>,
    token: AnvilMenuToken,
    active: bool,
    state: AnvilState,
    sync: MenuSync,
    drag_status: u8,
    drag_type: u8,
    drag_slots: BTreeSet<usize>,
}

/// One-shot synchronization preparation for the actual ServerPlayer.take
/// boundary. This borrows the actual canonical inventory; it does not own a
/// substitute player or invoke a full AnvilMenuHost/world transaction.
pub struct AnvilInventorySyncPlan<'inventory, K> {
    inventory: &'inventory PlayerInventory,
    expected: AnvilMenu<K>,
    next_sync: MenuSync,
    next_token: AnvilMenuToken,
    events: Vec<SyncEvent>,
    phase: AnvilPhase,
}

/// Read-only source observations and already encoded packet events. The
/// caller publishes packets once, in this event order, after adopting state.
/// This is not another inventory, cursor, input or result owner.
#[derive(Debug)]
pub struct AnvilInventorySyncPublication {
    pub events: Vec<SyncEvent>,
    pub phase: AnvilPhase,
}

/// The canonical menu is exclusively held, and the actual inventory remains
/// immutably borrowed, until commit or cancellation. All native comparison,
/// encoding, stale-state checks and revision checks have already succeeded.
/// Main must preflight ALL real readonly listener hooks before the first hook;
/// after hooks begin, adoption here requires no remaining fallible work.
pub struct ValidatedAnvilInventorySync<'menu, 'inventory, K> {
    menu: &'menu mut AnvilMenu<K>,
    plan: AnvilInventorySyncPlan<'inventory, K>,
}

impl<'inventory, K: Clone + Eq + Debug> AnvilInventorySyncPlan<'inventory, K> {
    pub fn events(&self) -> &[SyncEvent] {
        &self.events
    }
    pub fn phase(&self) -> &AnvilPhase {
        &self.phase
    }
    pub fn inventory(&self) -> &'inventory PlayerInventory {
        self.inventory
    }
    pub fn next_token(&self) -> AnvilMenuToken {
        self.next_token
    }

    pub fn validate(&self, menu: &AnvilMenu<K>) -> Result<()> {
        menu.validate_token(self.expected.token)?;
        let same_enchantment_owner = match (&menu.enchantments, &self.expected.enchantments) {
            (None, None) => true,
            (Some(actual), Some(expected)) => Arc::ptr_eq(actual, expected),
            _ => false,
        };
        ensure!(
            menu.binding == self.expected.binding
                && same_enchantment_owner
                && menu.active == self.expected.active
                && menu.sync == self.expected.sync
                && menu.state.inputs() == self.expected.state.inputs()
                && menu.state.preview() == self.expected.state.preview()
                && menu.state.item_name() == self.expected.state.item_name()
                && menu.drag_status == self.expected.drag_status
                && menu.drag_type == self.expected.drag_type
                && menu.drag_slots == self.expected.drag_slots,
            "retained anvil state changed during inventory sync preparation"
        );
        // inventory is the same immutable borrow captured before preparation;
        // no caller can replace/mutate that canonical owner while it is held.
        Ok(())
    }

    pub fn into_validated<'menu>(
        self,
        menu: &'menu mut AnvilMenu<K>,
    ) -> Result<ValidatedAnvilInventorySync<'menu, 'inventory, K>> {
        self.validate(menu)?;
        Ok(ValidatedAnvilInventorySync { menu, plan: self })
    }
}

impl<'menu, 'inventory, K> ValidatedAnvilInventorySync<'menu, 'inventory, K> {
    pub fn events(&self) -> &[SyncEvent] {
        &self.plan.events
    }
    pub fn phase(&self) -> &AnvilPhase {
        &self.plan.phase
    }
    pub fn inventory(&self) -> &'inventory PlayerInventory {
        self.plan.inventory
    }
    pub fn next_token(&self) -> AnvilMenuToken {
        self.plan.next_token
    }

    /// Adopt only prepared sync baselines/state ID and checked dispatch token.
    /// No owner callback, validation, encoding, allocation or Result remains.
    pub fn commit(self) -> AnvilInventorySyncPublication {
        let Self { menu, plan } = self;
        menu.sync = plan.next_sync;
        menu.token = plan.next_token;
        AnvilInventorySyncPublication {
            events: plan.events,
            phase: plan.phase,
        }
    }
}
struct ObservedContext<'a, H> {
    actual: &'a mut H,
    phase: AnvilPhase,
}
impl<H: AnvilMenuHost> AnvilContext for ObservedContext<'_, H> {
    fn observe_computation_change(&mut self, change: AnvilComputationChange) -> Result<()> {
        match change {
            AnvilComputationChange::CostInitialized => self.phase.level_cost = 1,
            AnvilComputationChange::RepairMaterialCount(count) => {
                self.phase.repair_material_count = count
            }
        }
        self.actual.observe_menu_phase(&self.phase)
    }
    fn enchantment_snapshot(&self) -> Result<Option<AnvilEnchantmentSnapshot>> {
        self.actual.enchantment_snapshot()
    }
    fn validate_enchantment_snapshot(
        &self,
        snapshot: Option<&AnvilEnchantmentSnapshot>,
    ) -> Result<()> {
        self.actual.validate_enchantment_snapshot(snapshot)
    }
    fn begin_enchantment_merge(
        &mut self,
        base: &ItemStack,
        addition: &ItemStack,
        snapshot: &AnvilEnchantmentSnapshot,
    ) -> Result<Box<dyn AnvilEnchantmentSession>> {
        self.actual
            .begin_enchantment_merge(base, addition, snapshot)
    }
    fn is_valid_repair_item(&mut self, base: &ItemStack, addition: &ItemStack) -> Result<bool> {
        self.actual.is_valid_repair_item(base, addition)
    }
    fn hover_name(&mut self, item: &ItemStack) -> Result<JavaString> {
        self.actual.hover_name(item)
    }
}

fn counted(item: &ItemStack, count: i32) -> ItemStack {
    if count <= 0 || item.is_empty() {
        ItemStack::empty()
    } else {
        let mut out = item.clone();
        out.count = count;
        out
    }
}
impl<K: Clone + Eq + Debug> AnvilMenu<K> {
    pub fn new<H: AnvilMenuHost<Binding = K>>(window: u8, host: &H) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let mut seen = NEXT.load(Ordering::Relaxed);
        let instance = loop {
            let next = seen
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("anvil UI identity exhausted"))?;
            match NEXT.compare_exchange_weak(seen, next, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break seen,
                Err(actual) => seen = actual,
            }
        };
        Ok(Self {
            binding: host.binding().clone(),
            enchantments: host.enchantment_snapshot()?,
            token: AnvilMenuToken {
                instance,
                revision: 0,
            },
            active: true,
            state: AnvilState::default(),
            sync: MenuSync::new(window, WorkstationKind::Anvil)?,
            drag_status: 0,
            drag_type: 0,
            drag_slots: BTreeSet::new(),
        })
    }
    fn staged_copy(&self) -> Self {
        Self {
            binding: self.binding.clone(),
            enchantments: self.enchantments.clone(),
            token: self.token,
            active: self.active,
            state: self.state.clone(),
            sync: self.sync.clone(),
            drag_status: self.drag_status,
            drag_type: self.drag_type,
            drag_slots: self.drag_slots.clone(),
        }
    }
    pub fn token(&self) -> AnvilMenuToken {
        self.token
    }
    fn advance_revision(&mut self) -> Result<()> {
        self.token.revision = self
            .token
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("anvil revision exhausted"))?;
        Ok(())
    }
    fn validate_token(&self, token: AnvilMenuToken) -> Result<()> {
        ensure!(
            self.active && token == self.token,
            "stale or closed anvil UI action"
        );
        Ok(())
    }
    fn require_components<H: AnvilMenuHost<Binding = K>>(
        &self,
        host: &H,
        click: Option<&ContainerClick>,
    ) -> Result<()> {
        self.require_inventory_sync_components(host.inventory())?;
        if let Some(click) = click {
            require_component_context(&click.carried)?;
            for item in click.changed_slots.values() {
                require_component_context(item)?;
            }
        }
        Ok(())
    }
    fn require_inventory_sync_components(&self, inventory: &PlayerInventory) -> Result<()> {
        let top = self.top();
        for item in top
            .iter()
            .chain(std::iter::once(self.carried()))
            .chain(inventory.slots().iter())
            .chain(std::iter::once(inventory.carried()))
        {
            require_component_context(item)?;
        }
        Ok(())
    }
    fn validate_publish<H: AnvilMenuHost<Binding = K>>(&self, host: &H) -> Result<()> {
        host.validate_enchantment_snapshot(self.enchantments.as_ref())?;
        ensure!(
            &self.binding == host.binding(),
            "anvil binding changed during callback"
        );
        Ok(())
    }
    pub fn state(&self) -> &AnvilState {
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
    pub fn phase(&self) -> AnvilPhase {
        AnvilPhase {
            inputs: self.state.inputs().clone(),
            result_preview: self.state.preview().stack.clone(),
            carried: self.carried().clone(),
            level_cost: self.state.preview().level_cost,
            repair_material_count: self.state.preview().repair_material_count,
        }
    }
    fn top(&self) -> [ItemStack; 3] {
        [
            self.state.inputs()[0].clone(),
            self.state.inputs()[1].clone(),
            self.state.preview().stack.clone(),
        ]
    }
    fn data(&self) -> [i32; 1] {
        [self.state.preview().level_cost]
    }
    fn validate_host(&self, host: &impl AnvilMenuHost<Binding = K>) -> Result<()> {
        host.validate_enchantment_snapshot(self.enchantments.as_ref())?;
        ensure!(
            self.active && &self.binding == host.binding(),
            "anvil player/table/menu binding changed"
        );
        Ok(())
    }
    pub fn still_valid(&self, host: &impl AnvilMenuHost<Binding = K>) -> Result<bool> {
        self.validate_host(host)?;
        self.sync.still_valid(host)
    }
    pub fn transfer_inventory_state(&self, window0: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(window0)
    }
    /// Literal current-menu broadcastChanges for ServerPlayer.take. This does
    /// not initialize a new menu or consult XP, registry, repair or world owners.
    /// The caller supplies the actual pickup's canonical inventory borrow.
    pub fn prepare_inventory_broadcast<'inventory>(
        &self,
        inventory: &'inventory PlayerInventory,
    ) -> Result<AnvilInventorySyncPlan<'inventory, K>> {
        self.prepare_inventory_sync(inventory, false)
    }
    /// Retained full-state counterpart for an actual source resend owner.
    /// It preserves the same inputs/result/name/cursor/cost/drag/lifetime.
    pub fn prepare_inventory_full_state<'inventory>(
        &self,
        inventory: &'inventory PlayerInventory,
    ) -> Result<AnvilInventorySyncPlan<'inventory, K>> {
        self.prepare_inventory_sync(inventory, true)
    }
    fn prepare_inventory_sync<'inventory>(
        &self,
        inventory: &'inventory PlayerInventory,
        full_state: bool,
    ) -> Result<AnvilInventorySyncPlan<'inventory, K>> {
        self.validate_token(self.token)?;
        self.require_inventory_sync_components(inventory)?;
        let mut next_token = self.token;
        next_token.revision = next_token
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("anvil inventory sync revision exhausted"))?;
        let mut next_sync = self.sync.clone();
        let events = if full_state {
            next_sync.broadcast_full_state(&self.top(), inventory, &self.data())?
        } else {
            next_sync.broadcast_changes(&self.top(), inventory, &self.data())?
        };
        Ok(AnvilInventorySyncPlan {
            inventory,
            expected: self.staged_copy(),
            next_sync,
            next_token,
            events,
            phase: self.phase(),
        })
    }
    fn reset_drag(&mut self) {
        self.drag_status = 0;
        self.drag_slots.clear();
    }
    fn publish_sync<H: AnvilMenuHost<Binding = K>>(
        &self,
        events: Vec<SyncEvent>,
        host: &mut H,
    ) -> Result<()> {
        host.stage_sync_events(events, &self.phase())
    }
    fn broadcast_inner<H: AnvilMenuHost<Binding = K>>(&mut self, host: &mut H) -> Result<()> {
        let events = self
            .sync
            .broadcast_changes(&self.top(), host.inventory(), &self.data())?;
        self.publish_sync(events, host)
    }
    fn transaction<H: AnvilMenuHost<Binding = K>, T>(
        &mut self,
        token: AnvilMenuToken,
        action: AnvilMenuAction,
        host: &mut H,
        operation: impl FnOnce(&mut Self, &mut H) -> Result<T>,
    ) -> Result<T> {
        self.validate_token(token)?;
        self.validate_host(host)?;
        self.require_components(host, None)?;
        let mut staged = host.begin_stage(token, action)?;
        self.validate_host(&staged)?;
        self.require_components(&staged, None)?;
        staged.preflight_menu_action(action)?;
        self.validate_host(&staged)?;
        let mut next = self.staged_copy();
        let value = operation(&mut next, &mut staged)?;
        next.require_components(&staged, None)?;
        next.validate_publish(&staged)?;
        self.validate_host(host)?;
        next.advance_revision()?;
        // Every fallible action/callback/validation is finished. Actual owner
        // publication is atomic; UI installation after it cannot fail.
        host.publish_stage(staged, token)?;
        *self = next;
        Ok(value)
    }
    pub fn initialize<H: AnvilMenuHost<Binding = K>>(&mut self, host: &mut H) -> Result<()> {
        self.transaction(
            self.token,
            AnvilMenuAction::Initialize,
            host,
            |next, staged| {
                let events = next
                    .sync
                    .initialize(&next.top(), staged.inventory(), &next.data())?;
                next.publish_sync(events, staged)
            },
        )
    }
    pub fn broadcast_changes<H: AnvilMenuHost<Binding = K>>(&mut self, host: &mut H) -> Result<()> {
        self.transaction(
            self.token,
            AnvilMenuAction::Broadcast,
            host,
            |next, staged| next.broadcast_inner(staged),
        )
    }
    /// Source broadcastFullState retains listener/change baselines and sends
    /// the current content/data, including the spectator packet-owner path.
    pub fn broadcast_full_state<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        self.transaction(
            self.token,
            AnvilMenuAction::FullState,
            host,
            |next, staged| {
                let events = next.sync.broadcast_full_state(
                    &next.top(),
                    staged.inventory(),
                    &next.data(),
                )?;
                next.publish_sync(events, staged)
            },
        )
    }
    pub fn set_item_name<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        token: AnvilMenuToken,
        name: &JavaString,
        host: &mut H,
    ) -> Result<bool> {
        self.transaction(token, AnvilMenuAction::Rename, host, |next, staged| {
            ensure!(next.still_valid(staged)?, "anvil is no longer valid");
            if !next.state.request_name_observed(name)? {
                return Ok(false);
            }
            next.recompute(staged)?;
            Ok(true)
        })
    }
    fn recompute<H: AnvilMenuHost<Binding = K>>(&mut self, host: &mut H) -> Result<()> {
        let phase = self.phase();
        let player = host.player()?;
        if self.state.recompute_observed(
            player,
            &mut ObservedContext {
                actual: host,
                phase,
            },
        )? {
            self.broadcast_inner(host)?;
        }
        Ok(())
    }
    pub fn click<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        token: AnvilMenuToken,
        click: &ContainerClick,
        host: &mut H,
    ) -> Result<()> {
        self.validate_token(token)?;
        ensure!(click.window_id == self.window_id(), "wrong anvil window");
        ensure!(click.slot < MENU_SLOTS as i16, "invalid anvil slot");
        // Prediction identity must be checked before staged click mutations.
        self.require_components(host, Some(click))?;
        self.transaction(
            token,
            AnvilMenuAction::Click(click.kind),
            host,
            |next, staged| {
                next.require_components(staged, Some(click))?;
                ensure!(next.still_valid(staged)?, "anvil is no longer valid");
                let before = next.state_id();
                next.sync.suppress_remote_updates(true);
                next.do_click(click, staged)?;
                let events = next.sync.finish_click(
                    click,
                    before,
                    &next.top(),
                    staged.inventory(),
                    &next.data(),
                )?;
                next.publish_sync(events, staged)
            },
        )
    }
    pub fn close<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        token: AnvilMenuToken,
        context: MenuCloseContext,
        host: &mut H,
    ) -> Result<()> {
        self.transaction(
            token,
            AnvilMenuAction::Close(context),
            host,
            |next, staged| {
                next.reset_drag();
                // removed returns cursor first, then each removeItemNoUpdate input.
                // Old derived preview/cost survive at each source return callback.
                next.close_return(&mut [], context, staged)?;
                for index in 0..INPUT_SLOTS {
                    let mut input = next.state.inputs()[index].clone();
                    next.state
                        .mutate_input_in_place(index, ItemStack::empty())?;
                    next.close_return(std::slice::from_mut(&mut input), context, staged)?;
                }
                next.state.close_inputs();
                next.active = false;
                Ok(())
            },
        )
    }
    fn close_return<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        inputs: &mut [ItemStack],
        context: MenuCloseContext,
        host: &mut H,
    ) -> Result<()> {
        // Shared removal uses scratch snapshots. Replay onto the SAME canonical
        // staged inventory before each actual callback; no second live owner.
        let cursor = inputs.is_empty();
        let mut returning = if cursor {
            self.carried().clone()
        } else {
            inputs[0].clone()
        };
        let before = host.inventory().clone();
        let removal = self
            .sync
            .close_inputs_ordered(inputs, host.inventory_mut(), context)?;
        let after = host.inventory().clone();
        *host.inventory_mut() = before;
        if cursor {
            self.sync.set_carried(returning.clone());
        }
        for event in removal.events {
            match event {
                MenuRemovalEvent::Packet { index } => {
                    let packet = removal.outcome.packets[index].clone();
                    let added = Self::observe_return_packet(&packet, &returning, host)?;
                    returning = counted(&returning, returning.count - added);
                    if cursor {
                        self.sync.set_carried(returning.clone());
                    }
                    self.publish_sync(vec![SyncEvent::Packet(packet)], host)?;
                }
                MenuRemovalEvent::Drop { index } => {
                    let dropped = removal.outcome.drops[index].clone();
                    ensure!(dropped.stack == returning, "anvil close drop phase changed");
                    host.stage_drop(dropped, &self.phase())?;
                    returning = ItemStack::empty();
                    if cursor {
                        self.sync.set_carried(ItemStack::empty());
                    }
                }
            }
        }
        *host.inventory_mut() = after;
        if cursor {
            self.sync.set_carried(ItemStack::empty());
        }
        Ok(())
    }
    fn observe_return_packet<H: AnvilMenuHost<Binding = K>>(
        packet: &crate::protocol::Packet,
        returning: &ItemStack,
        host: &mut H,
    ) -> Result<i32> {
        ensure!(packet.id == 0x15, "unsupported anvil close packet");
        let mut reader = crate::protocol::Reader::new(&packet.payload);
        ensure!(
            reader.i8()? == -2 && reader.varint()? == 0,
            "expected raw inventory close packet"
        );
        let raw = reader.i16()?;
        let slot = match raw {
            0..=8 => 36 + raw as usize,
            9..=35 => raw as usize,
            40 => 45,
            _ => anyhow::bail!("invalid anvil close raw slot"),
        };
        let stack = ItemStack::decode_exact(reader.remaining())?;
        ensure!(
            stack.same_item_same_components(returning)?,
            "anvil close return item changed"
        );
        let old = host.inventory().slot(slot).expect("raw inventory slot");
        let old_count = if old.is_empty() {
            0
        } else {
            ensure!(
                old.same_item_same_components(&stack)?,
                "anvil close overwrote another item"
            );
            old.count
        };
        let added = stack.count - old_count;
        ensure!(
            added > 0 && added <= returning.count,
            "invalid anvil close insertion count"
        );
        host.inventory_mut().set_slot_authoritative(slot, stack);
        Ok(added)
    }
    fn slot<H: AnvilMenuHost<Binding = K>>(&self, index: usize, host: &H) -> ItemStack {
        match index {
            0 | 1 => self.state.inputs()[index].clone(),
            RESULT_SLOT => self.state.preview().stack.clone(),
            _ => host
                .inventory()
                .slot(index + 6)
                .expect("validated anvil player slot")
                .clone(),
        }
    }
    fn may_place(index: usize, _: &ItemStack) -> bool {
        index != RESULT_SLOT
    }
    fn limit(_: usize, item: &ItemStack) -> Result<i32> {
        Ok(item.max_stack_size()?.min(99))
    }
    fn may_pickup<H: AnvilMenuHost<Binding = K>>(&self, index: usize, host: &H) -> Result<bool> {
        if index == RESULT_SLOT {
            Ok(self.state.preview().level_cost > 0 && {
                let player = host.player()?;
                player.instabuild || player.experience_level >= self.state.preview().level_cost
            })
        } else {
            Ok(true)
        }
    }
    fn mutate_raw<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        index: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        match index {
            0 | 1 => self.state.mutate_input_in_place(index, item),
            RESULT_SLOT => self.state.mutate_result_in_place(item),
            _ => {
                host.inventory_mut().set_slot_authoritative(index + 6, item);
                Ok(())
            }
        }
    }
    fn changed<H: AnvilMenuHost<Binding = K>>(&mut self, index: usize, host: &mut H) -> Result<()> {
        if index < INPUT_SLOTS {
            host.input_container_set_changed(&self.phase())?;
            self.broadcast_inner(host)?; // superclass slotsChanged precedes createResult
            self.recompute(host)
        } else if index == RESULT_SLOT {
            Ok(())
        } else {
            host.player_inventory_set_changed(&self.phase())
        }
    }
    fn set_container_input<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        index: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        let item = counted(&item, item.count.min(Self::limit(index, &item)?));
        self.mutate_raw(index, item, host)?;
        self.changed(index, host)
    }
    fn set_slot<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        index: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if index < INPUT_SLOTS {
            self.set_container_input(index, item, host)?;
        } else {
            self.mutate_raw(index, item, host)?;
        }
        self.changed(index, host)
    }
    fn on_take<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        index: usize,
        _: &ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if index != RESULT_SLOT {
            return self.changed(index, host);
        }
        let player = host.player()?;
        if !player.instabuild {
            host.give_experience_levels(-self.state.preview().level_cost, &self.phase())?;
        }
        self.set_container_input(0, ItemStack::empty(), host)?;
        let material = self.state.preview().repair_material_count;
        let addition = self.state.inputs()[1].clone();
        let remainder = if material > 0 && !addition.is_empty() && addition.count > material {
            counted(&addition, addition.count - material)
        } else {
            ItemStack::empty()
        };
        self.set_container_input(1, remainder, host)?;
        self.state.reset_cost_after_take();
        host.observe_menu_phase(&self.phase())?;
        finish_anvil_take(host.player()?.instabuild, host)
    }
    fn try_remove<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        index: usize,
        amount: i32,
        maximum: i32,
        host: &mut H,
    ) -> Result<ItemStack> {
        if !self.may_pickup(index, host)? {
            return Ok(ItemStack::empty());
        }
        let source = self.slot(index, host);
        if index == RESULT_SLOT && maximum < source.count {
            return Ok(ItemStack::empty());
        }
        // ResultContainer.removeItem ignores the requested amount, returning
        // its entire derived stack. Ordinary input/player slots split normally.
        let taken = if index == RESULT_SLOT {
            source.clone()
        } else {
            counted(&source, amount.min(maximum).min(source.count))
        };
        if taken.is_empty() {
            return Ok(taken);
        }
        self.mutate_raw(index, counted(&source, source.count - taken.count), host)?;
        if index < INPUT_SLOTS {
            self.changed(index, host)?;
        }
        if self.slot(index, host).is_empty() {
            self.set_slot(index, ItemStack::empty(), host)?;
        }
        Ok(taken)
    }
    fn safe_take<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        index: usize,
        amount: i32,
        maximum: i32,
        host: &mut H,
    ) -> Result<ItemStack> {
        let taken = self.try_remove(index, amount, maximum, host)?;
        if !taken.is_empty() {
            self.on_take(index, &taken, host)?;
        }
        Ok(taken)
    }
    fn drop<H: AnvilMenuHost<Binding = K>>(&self, stack: ItemStack, host: &mut H) -> Result<()> {
        if !stack.is_empty() {
            host.stage_drop(
                DroppedStack {
                    stack,
                    around: false,
                    trace_item: true,
                },
                &self.phase(),
            )?;
        }
        Ok(())
    }
    fn do_click<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        click: &ContainerClick,
        host: &mut H,
    ) -> Result<()> {
        let slot = click.slot as i32;
        let button = click.button as i32;
        let creative = host.player()?.instabuild;
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
                        self.sync.set_carried(counted(&source, source.count - n));
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
    fn pickup<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        index: usize,
        button: i32,
        host: &mut H,
    ) -> Result<()> {
        let source = self.slot(index, host);
        let cursor = self.carried().clone();
        if !source.is_empty() && !self.may_pickup(index, host)? {
            return Ok(());
        }
        ensure!(
            inventory::item_name(source.item_id) != Some("bundle")
                && inventory::item_name(cursor.item_id) != Some("bundle"),
            "bundle click overrides require actual bundle owner"
        );
        if source.is_empty() {
            if !cursor.is_empty() && Self::may_place(index, &cursor) {
                let n =
                    (if button == 0 { cursor.count } else { 1 }).min(Self::limit(index, &cursor)?);
                self.sync.set_carried(counted(&cursor, cursor.count - n));
                self.set_slot(index, counted(&cursor, n), host)?;
            }
        } else if cursor.is_empty() {
            let amount = if button == 0 {
                source.count
            } else {
                (source.count + 1) / 2
            };
            let taken = self.try_remove(index, amount, i32::MAX, host)?;
            if !taken.is_empty() {
                self.sync.set_carried(taken.clone());
                self.on_take(index, &self.carried().clone(), host)?;
            }
        } else if Self::may_place(index, &cursor) {
            if source.same_item_same_components(&cursor)? {
                let n = (if button == 0 { cursor.count } else { 1 })
                    .min(Self::limit(index, &cursor)? - source.count);
                // Slot.safeInsert retains negative capacity for an admitted
                // overfull stack: shrink(cursor,n) grows that cursor while
                // grow(source,n) restores the slot to its limit. It also calls
                // setByPlayer when a full slot accepts exactly zero items.
                self.sync.set_carried(counted(&cursor, cursor.count - n));
                self.set_slot(index, counted(&source, source.count + n), host)?;
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
                    .set_carried(counted(&cursor, cursor.count + taken.count));
                self.on_take(index, &taken, host)?;
            }
        }
        self.changed(index, host) // unconditional Slot.setChanged after Pickup
    }
    fn move_to<H: AnvilMenuHost<Binding = K>>(
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
                        *stack = counted(stack, stack.count - n);
                        self.mutate_raw(source_index, stack.clone(), host)?;
                        self.mutate_raw(index, counted(&old, old.count + n), host)?;
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
                if self.slot(index, host).is_empty() && Self::may_place(index, stack) {
                    let n = stack.count.min(Self::limit(index, stack)?);
                    let moved = counted(stack, n);
                    *stack = counted(stack, stack.count - n);
                    self.mutate_raw(source_index, stack.clone(), host)?;
                    self.set_slot(index, moved, host)?;
                    self.changed(index, host)?;
                    break;
                }
            }
        }
        Ok(before != stack.count)
    }
    fn quick_move<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        index: usize,
        host: &mut H,
    ) -> Result<bool> {
        if !self.may_pickup(index, host)? {
            return Ok(false);
        }
        let source = self.slot(index, host);
        if source.is_empty() {
            return Ok(false);
        }
        let mut left = source.clone();
        let moved = if index == RESULT_SLOT {
            self.move_to(index, &mut left, 3..MENU_SLOTS, true, host)?
        } else if index < INPUT_SLOTS {
            self.move_to(index, &mut left, 3..MENU_SLOTS, false, host)?
        } else {
            // Anvil inherits canMoveIntoInputSlots=true: any player item tries
            // both ordinary inputs, with no synthetic recipe classification.
            self.move_to(index, &mut left, 0..RESULT_SLOT, false, host)?
        };
        if !moved {
            return Ok(false);
        }
        if left.is_empty() {
            self.set_slot(index, ItemStack::empty(), host)?;
        } else {
            self.changed(index, host)?;
        }
        self.on_take(index, &left, host)?;
        Ok(true)
    }
    fn swap<H: AnvilMenuHost<Binding = K>>(
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
        if !self.may_pickup(index, host)? {
            return Ok(());
        }
        if held.is_empty() && source.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            host.inventory_mut()
                .set_slot_authoritative(player_index, source.clone());
            self.set_slot(index, ItemStack::empty(), host)?;
            self.on_take(index, &source, host)?;
        } else if Self::may_place(index, &held) {
            let maximum = Self::limit(index, &held)?;
            if source.is_empty() {
                host.inventory_mut().set_slot_authoritative(
                    player_index,
                    counted(&held, held.count - maximum.min(held.count)),
                );
                self.set_slot(index, counted(&held, maximum.min(held.count)), host)?;
            } else if held.count > maximum {
                host.inventory_mut()
                    .set_slot_authoritative(player_index, counted(&held, held.count - maximum));
                self.set_slot(index, counted(&held, maximum), host)?;
                self.on_take(index, &source, host)?;
                let mut displaced = if index >= INPUT_SLOTS && index + 6 == player_index {
                    // Java source/held locals alias the pre-split stack. After
                    // split the displaced local references only its remainder.
                    counted(&source, source.count - maximum)
                } else {
                    source
                };
                if !host.add_to_player_inventory(&mut displaced, &self.phase())? {
                    self.drop(displaced, host)?;
                }
            } else {
                host.inventory_mut()
                    .set_slot_authoritative(player_index, source.clone());
                self.set_slot(index, held, host)?;
                self.on_take(index, &source, host)?;
            }
        }
        Ok(())
    }
    fn pickup_all<H: AnvilMenuHost<Binding = K>>(
        &mut self,
        clicked: usize,
        button: i32,
        host: &mut H,
    ) -> Result<()> {
        if self.carried().is_empty()
            || (!self.slot(clicked, host).is_empty() && self.may_pickup(clicked, host)?)
        {
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
                    || !self.may_pickup(index, host)?
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
                    .set_carried(counted(&cursor, cursor.count + taken.count));
            }
        }
        Ok(())
    }
    fn quickcraft<H: AnvilMenuHost<Binding = K>>(
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
                        && Self::may_place(index, cursor)
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
                        || !Self::may_place(index, &cursor)
                        || self.drag_type != 2 && cursor.count < self.drag_slots.len() as i32
                    {
                        continue;
                    }
                    let count = (each + old.count).min(Self::limit(index, &cursor)?);
                    remaining -= count - old.count;
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
        crafting::RecipeBook,
        inventory::Component,
        protocol::{Reader, Writer},
    };
    use std::collections::BTreeMap;
    #[derive(Clone, Debug)]
    struct Event {
        name: &'static str,
        phase: AnvilPhase,
        inventory: PlayerInventory,
    }
    #[derive(Clone, Debug)]
    struct Host {
        inventory: PlayerInventory,
        binding: u64,
        player: AnvilPlayer,
        valid: bool,
        block: i32,
        random: f32,
        draws: usize,
        events: Vec<Event>,
        sync: Vec<SyncEvent>,
        drops: Vec<DroppedStack>,
        xp: Vec<i32>,
        current: Option<AnvilPhase>,
        enchantment_owner: Option<crate::workstations::anvil::enchantment_fixture::Owner>,
        generation: u64,
        base_generation: u64,
        staged_token: Option<AnvilMenuToken>,
        last_action: Option<AnvilMenuAction>,
        fail_world_event: bool,
        fail_publish: bool,
        fail_drop: bool,
    }
    impl Host {
        fn new() -> Self {
            Self {
                inventory: PlayerInventory::default(),
                binding: 42,
                player: AnvilPlayer {
                    instabuild: false,
                    experience_level: 100,
                },
                valid: true,
                block: 0,
                random: 0.5,
                draws: 0,
                events: vec![],
                sync: vec![],
                drops: vec![],
                xp: vec![],
                current: None,
                enchantment_owner: None,
                generation: 0,
                base_generation: 0,
                staged_token: None,
                last_action: None,
                fail_world_event: false,
                fail_publish: false,
                fail_drop: false,
            }
        }
        fn record(&mut self, name: &'static str, phase: &AnvilPhase) {
            self.events.push(Event {
                name,
                phase: phase.clone(),
                inventory: self.inventory.clone(),
            });
        }
        fn world_event(&mut self, name: &'static str) {
            self.record(
                name,
                &self.current.clone().expect("actual staged menu phase"),
            );
        }
    }
    impl AnvilContext for Host {
        fn enchantment_snapshot(&self) -> Result<Option<AnvilEnchantmentSnapshot>> {
            match &self.enchantment_owner {
                Some(owner) => owner.enchantment_snapshot(),
                None => Ok(None),
            }
        }
        fn validate_enchantment_snapshot(
            &self,
            snapshot: Option<&AnvilEnchantmentSnapshot>,
        ) -> Result<()> {
            match &self.enchantment_owner {
                Some(owner) => owner.validate_enchantment_snapshot(snapshot),
                None => {
                    ensure!(snapshot.is_none(), "fixture enchantment owner disappeared");
                    Ok(())
                }
            }
        }
        fn begin_enchantment_merge(
            &mut self,
            base: &ItemStack,
            addition: &ItemStack,
            snapshot: &AnvilEnchantmentSnapshot,
        ) -> Result<Box<dyn AnvilEnchantmentSession>> {
            self.enchantment_owner
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("fixture owner unavailable"))?
                .begin_enchantment_merge(base, addition, snapshot)
        }
        fn is_valid_repair_item(&mut self, base: &ItemStack, addition: &ItemStack) -> Result<bool> {
            self.world_event("repair-rule");
            Ok(
                base.item_id == RecipeBook::vanilla().item_id("iron_sword").unwrap()
                    && addition.item_id == RecipeBook::vanilla().item_id("iron_ingot").unwrap(),
            )
        }
        fn hover_name(&mut self, _: &ItemStack) -> Result<JavaString> {
            self.world_event("hover-name");
            Ok("Original".into())
        }
    }
    impl WorkstationAccess for Host {
        fn block_matches(&self, kind: WorkstationKind) -> Result<bool> {
            assert_eq!(kind, WorkstationKind::Anvil);
            Ok(self.valid)
        }
        fn player_can_interact_with_block(&self, range: f64) -> Result<bool> {
            assert_eq!(range, 4.0);
            Ok(self.valid)
        }
    }
    impl AnvilWorldAccess for Host {
        type BlockState = i32;
        fn block_state(&mut self) -> Result<i32> {
            self.world_event("block");
            Ok(self.block)
        }
        fn is_anvil(&self, state: &i32) -> Result<bool> {
            Ok((0..=2).contains(state))
        }
        fn player_random_next_float(&mut self) -> Result<f32> {
            self.world_event("player-random");
            self.draws += 1;
            Ok(self.random)
        }
        fn damaged_state(&self, state: &i32) -> Result<Option<i32>> {
            Ok((*state < 2).then_some(*state + 1))
        }
        fn destroy_block_without_drop(&mut self) -> Result<()> {
            self.world_event("destroy");
            self.block = -1;
            Ok(())
        }
        fn set_block(&mut self, state: i32, flags: i32) -> Result<()> {
            assert_eq!(flags, 2);
            self.world_event("set-block");
            self.block = state;
            Ok(())
        }
        fn level_event(&mut self, event: i32, data: i32) -> Result<()> {
            assert_eq!(data, 0);
            self.world_event(if event == 1029 {
                "event1029"
            } else {
                assert_eq!(event, 1030);
                "event1030"
            });
            if self.fail_world_event {
                // This candidate already changed inventory, charged XP, drew
                // player RNG and destroyed the final anvil before the failure.
                assert_eq!(self.inventory.slot(44).unwrap().count, 4);
                assert_eq!(self.xp, [-1]);
                assert_eq!(self.draws, 1);
                assert_eq!(self.block, -1);
                anyhow::bail!("fixture late world callback failure");
            }
            Ok(())
        }
    }
    impl AnvilMenuHost for Host {
        type Binding = u64;
        fn binding(&self) -> &u64 {
            &self.binding
        }
        fn begin_stage(&self, token: AnvilMenuToken, action: AnvilMenuAction) -> Result<Self> {
            // All mutable fixture owners are owned values. The shared Arc
            // holder references are immutable registry fixture data only.
            let mut stage = self.clone();
            stage.base_generation = self.generation;
            stage.staged_token = Some(token);
            stage.last_action = Some(action);
            Ok(stage)
        }
        fn publish_stage(&mut self, mut staged: Self, token: AnvilMenuToken) -> Result<()> {
            ensure!(
                staged.staged_token == Some(token) && staged.base_generation == self.generation,
                "stale fixture real-owner stage"
            );
            if self.fail_publish {
                assert_eq!(staged.inventory.slot(44).unwrap().count, 4);
                assert_eq!(staged.xp, [-1]);
                assert_eq!(staged.draws, 1);
                assert_eq!(staged.block, -1);
                assert!(
                    staged.sync.len() > self.sync.len(),
                    "candidate packet outbox was staged"
                );
                anyhow::bail!("fixture late atomic publish failure");
            }
            staged.generation = self.generation.checked_add(1).unwrap();
            staged.staged_token = None;
            *self = staged;
            Ok(())
        }
        fn inventory(&self) -> &PlayerInventory {
            &self.inventory
        }
        fn inventory_mut(&mut self) -> &mut PlayerInventory {
            &mut self.inventory
        }
        fn player(&self) -> Result<AnvilPlayer> {
            Ok(self.player)
        }
        fn preflight_menu_action(&mut self, _: AnvilMenuAction) -> Result<()> {
            Ok(())
        }
        fn observe_menu_phase(&mut self, phase: &AnvilPhase) -> Result<()> {
            self.current = Some(phase.clone());
            Ok(())
        }
        fn stage_sync_events(&mut self, events: Vec<SyncEvent>, phase: &AnvilPhase) -> Result<()> {
            self.record("sync", phase);
            self.sync.extend(events);
            Ok(())
        }
        fn input_container_set_changed(&mut self, phase: &AnvilPhase) -> Result<()> {
            self.record("input-dirty", phase);
            Ok(())
        }
        fn player_inventory_set_changed(&mut self, phase: &AnvilPhase) -> Result<()> {
            self.record("inventory-dirty", phase);
            Ok(())
        }
        fn give_experience_levels(&mut self, delta: i32, phase: &AnvilPhase) -> Result<()> {
            self.record("xp", phase);
            self.xp.push(delta);
            self.player.experience_level += delta;
            Ok(())
        }
        fn add_to_player_inventory(
            &mut self,
            stack: &mut ItemStack,
            phase: &AnvilPhase,
        ) -> Result<bool> {
            self.record("add", phase);
            for i in 9..45 {
                if self.inventory.slot(i).unwrap().is_empty() {
                    self.inventory
                        .set_slot_authoritative(i, std::mem::take(stack));
                    return Ok(true);
                }
            }
            Ok(false)
        }
        fn stage_drop(&mut self, stack: DroppedStack, phase: &AnvilPhase) -> Result<()> {
            self.record("drop", phase);
            self.drops.push(stack);
            if self.fail_drop {
                assert_eq!(self.xp, [-1]);
                assert_eq!(self.draws, 1);
                assert_eq!(self.block, -1);
                anyhow::bail!("fixture post-drop callback failure");
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
    fn int_component(stack: &mut ItemStack, id: i32, value: i32) {
        let mut w = Writer::new();
        w.varint(value);
        stack.components.retain(|c| c.type_id != id);
        stack.components.push(Component {
            type_id: id,
            data: w.into_inner(),
        });
        stack.components.sort_by_key(|c| c.type_id);
    }
    fn click(
        menu: &mut AnvilMenu<u64>,
        host: &mut Host,
        slot: i16,
        button: i8,
        kind: ClickType,
    ) -> Result<()> {
        let packet = ContainerClick {
            window_id: menu.window_id(),
            state_id: menu.state_id(),
            slot,
            button,
            kind,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        menu.click(menu.token(), &packet, host)
    }
    fn rename(menu: &mut AnvilMenu<u64>, host: &mut Host, name: &str) -> Result<()> {
        assert!(menu.set_item_name(menu.token(), &name.into(), host)?);
        Ok(())
    }
    fn renamed_stack(count: i32) -> Result<(AnvilMenu<u64>, Host)> {
        let mut host = Host::new();
        host.inventory
            .set_slot_authoritative(9, item("stone", count));
        let mut menu = AnvilMenu::new(7, &host)?;
        menu.initialize(&mut host)?;
        click(&mut menu, &mut host, 3, 0, ClickType::QuickMove)?;
        rename(&mut menu, &mut host, "Named")?;
        assert_eq!(menu.state().preview().stack.count, count);
        Ok((menu, host))
    }
    #[test]
    fn repair_take_observes_previous_preview_cursor_xp_and_actual_player_rng_order() -> Result<()> {
        let mut host = Host::new();
        let mut sword = item("iron_sword", 1);
        int_component(&mut sword, 3, 100);
        host.inventory.set_slot_authoritative(9, sword);
        host.inventory
            .set_slot_authoritative(10, item("iron_ingot", 3));
        let mut menu = AnvilMenu::new(7, &host)?;
        menu.initialize(&mut host)?;
        click(&mut menu, &mut host, 3, 0, ClickType::QuickMove)?;
        host.events.clear();
        click(&mut menu, &mut host, 4, 0, ClickType::QuickMove)?;
        let first = host
            .events
            .iter()
            .find(|e| e.name == "input-dirty")
            .unwrap();
        assert_eq!(first.phase.inputs[1].count, 3);
        assert!(
            first.inventory.slot(10).unwrap().is_empty(),
            "source shared inventory shrinks before input listener callbacks"
        );
        assert!(first.phase.result_preview.is_empty());
        assert_eq!(first.phase.level_cost, 0);
        assert_eq!(menu.state().preview().level_cost, 2);
        assert_eq!(menu.state().preview().repair_material_count, 2);
        assert!(host
            .events
            .iter()
            .filter(|e| e.name == "repair-rule")
            .all(|e| e.phase.level_cost == 1));
        host.events.clear();
        host.random = 0.0;
        host.block = 2;
        click(&mut menu, &mut host, 2, 0, ClickType::Pickup)?;
        assert_eq!(host.xp, [-2]);
        assert_eq!(host.player.experience_level, 98);
        assert_eq!(host.draws, 1);
        assert_eq!(host.block, -1);
        assert_eq!(menu.carried().damage_value()?, 0);
        assert_eq!(menu.state().inputs()[1].count, 1);
        let xp = host.events.iter().position(|e| e.name == "xp").unwrap();
        assert!(!host.events[xp].phase.inputs[0].is_empty());
        assert!(!host.events[xp].phase.carried.is_empty());
        assert!(host.events[xp].phase.result_preview.is_empty());
        assert_eq!(host.events[xp].phase.level_cost, 2);
        let base = host
            .events
            .iter()
            .position(|e| e.name == "input-dirty" && e.phase.inputs[0].is_empty())
            .unwrap();
        let addition = host
            .events
            .iter()
            .position(|e| e.name == "input-dirty" && e.phase.inputs[1].count == 1)
            .unwrap();
        let block = host.events.iter().position(|e| e.name == "block").unwrap();
        assert!(xp < base && base < addition && addition < block);
        assert_eq!(host.events[base].phase.level_cost, 2);
        assert_eq!(host.events[addition].phase.level_cost, 0);
        assert_eq!(host.events[block].phase.level_cost, 0);
        assert_eq!(
            host.events
                .iter()
                .filter(|e| matches!(e.name, "block" | "player-random" | "destroy" | "event1029"))
                .map(|e| e.name)
                .collect::<Vec<_>>(),
            ["block", "player-random", "destroy", "event1029"]
        );
        Ok(())
    }
    #[test]
    fn whole_result_right_pickup_partial_shift_and_result_pickall_follow_source() -> Result<()> {
        let (mut menu, mut host) = renamed_stack(64)?;
        click(&mut menu, &mut host, 2, 1, ClickType::Pickup)?;
        assert_eq!(menu.carried().count, 64);
        assert_eq!(host.xp, [-1]);
        assert!(menu.state().inputs()[0].is_empty());
        let (mut menu, mut host) = renamed_stack(64)?;
        for i in 9..45 {
            host.inventory.set_slot_authoritative(i, item("dirt", 64));
        }
        host.inventory
            .set_slot_authoritative(44, counted(&menu.state().preview().stack, 63));
        click(&mut menu, &mut host, 2, 0, ClickType::QuickMove)?;
        assert_eq!(host.inventory.slot(44).unwrap().count, 64);
        assert_eq!(host.xp, [-1]);
        assert!(menu.state().inputs()[0].is_empty());
        assert!(menu.state().preview().stack.is_empty());
        assert!(menu.carried().is_empty());
        // Only the one free destination item was inserted; Java consumes the
        // inputs and discards the remaining derived preview on this branch.
        let (mut menu, mut host) = renamed_stack(1)?;
        menu.sync.set_carried(menu.state().preview().stack.clone());
        host.events.clear();
        click(&mut menu, &mut host, 3, 0, ClickType::PickupAll)?;
        assert_eq!(menu.carried().count, 2);
        assert_eq!(host.xp, [-1]);
        assert_eq!(
            host.events
                .iter()
                .find(|e| e.name == "xp")
                .unwrap()
                .phase
                .carried
                .count,
            1
        );
        Ok(())
    }
    #[test]
    fn drag_swap_clone_throw_close_and_session_guards_keep_preview_unowned() -> Result<()> {
        let mut host = Host::new();
        let mut menu = AnvilMenu::new(7, &host)?;
        menu.initialize(&mut host)?;
        let content = host
            .sync
            .iter()
            .find_map(|event| match event {
                SyncEvent::Packet(packet) if packet.id == 0x13 => Some(packet),
                _ => None,
            })
            .expect("initial anvil contents packet");
        let mut reader = Reader::new(&content.payload);
        assert_eq!(reader.u8()?, 7);
        reader.varint()?;
        assert_eq!(reader.varint()?, 39);
        host.sync.clear();
        assert!(menu.set_item_name(menu.token(), &"Early".into(), &mut host)?);
        assert!(
            host.sync.is_empty(),
            "empty-base createResult returns before its tail broadcast"
        );
        menu.sync.set_carried(item("emerald", 4));
        for (slot, button) in [(-999, 0), (0, 1), (1, 1), (-999, 2)] {
            click(&mut menu, &mut host, slot, button, ClickType::QuickCraft)?;
        }
        assert_eq!(
            menu.state()
                .inputs()
                .iter()
                .map(|s| s.count)
                .collect::<Vec<_>>(),
            [2, 2]
        );
        assert!(menu.carried().is_empty());
        host.inventory.set_slot_authoritative(45, item("coal", 3));
        click(&mut menu, &mut host, 0, 40, ClickType::Swap)?;
        assert_eq!(menu.state().inputs()[0], item("coal", 3));
        assert_eq!(host.inventory.slot(45), Some(&item("emerald", 2)));
        host.player.instabuild = true;
        click(&mut menu, &mut host, 0, 0, ClickType::Clone)?;
        assert_eq!(menu.carried().count, 64);
        menu.sync.set_carried(ItemStack::empty());
        click(&mut menu, &mut host, 0, 0, ClickType::Throw)?;
        assert_eq!(host.drops.last().unwrap().stack, item("coal", 1));
        assert_eq!(menu.state().inputs()[0], item("coal", 2));
        let (mut menu, mut host) = renamed_stack(4)?;
        menu.sync.set_carried(item("coal", 2));
        host.events.clear();
        let preview = menu.state().preview().stack.clone();
        menu.close(
            menu.token(),
            MenuCloseContext::DeadOrDisconnected,
            &mut host,
        )?;
        assert_eq!(
            host.drops.iter().map(|d| d.stack.count).collect::<Vec<_>>(),
            [2, 4]
        );
        let drops = host
            .events
            .iter()
            .filter(|e| e.name == "drop")
            .collect::<Vec<_>>();
        assert_eq!(drops[0].phase.carried, item("coal", 2));
        assert!(!drops[0].phase.inputs[0].is_empty());
        assert!(drops[1].phase.carried.is_empty());
        assert!(drops[1].phase.inputs[0].is_empty());
        assert!(drops.iter().all(|e| e.phase.result_preview == preview));
        assert!(menu.broadcast_changes(&mut host).is_err());
        let (mut menu, mut host) = renamed_stack(4)?;
        let preview = menu.state().preview().stack.clone();
        for slot in 9..45 {
            host.inventory
                .set_slot_authoritative(slot, item("dirt", 64));
        }
        host.inventory.set_slot_authoritative(36, item("coal", 63));
        host.inventory.set_slot_authoritative(10, item("stone", 63));
        menu.sync.set_carried(item("coal", 2));
        host.events.clear();
        menu.close(menu.token(), MenuCloseContext::AliveConnected, &mut host)?;
        assert_eq!(
            host.drops.iter().map(|d| d.stack.count).collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(
            host.events.iter().map(|e| e.name).collect::<Vec<_>>(),
            ["sync", "drop", "sync", "drop"]
        );
        assert_eq!(host.events[0].phase.carried, item("coal", 1));
        assert_eq!(host.events[0].inventory.slot(36), Some(&item("coal", 64)));
        assert_eq!(host.events[1].phase.carried, item("coal", 1));
        assert!(host.events[2].phase.carried.is_empty());
        assert_eq!(host.events[2].inventory.slot(10), Some(&item("stone", 64)));
        assert!(host
            .events
            .iter()
            .all(|e| e.phase.result_preview == preview));
        let (mut menu, mut host) = renamed_stack(1)?;
        host.binding += 1;
        assert!(click(&mut menu, &mut host, 2, 0, ClickType::Pickup).is_err());
        assert!(!menu.state().inputs()[0].is_empty());
        host.binding -= 1;
        host.valid = false;
        assert!(rename(&mut menu, &mut host, "Other").is_err());
        assert_eq!(menu.state().item_name(), Some(&"Named".into()));
        Ok(())
    }
    #[test]
    fn repair_material_counter_changes_before_actual_repair_and_hover_callbacks() -> Result<()> {
        let mut host = Host::new();
        let mut sword = item("iron_sword", 1);
        int_component(&mut sword, 3, 100);
        host.inventory.set_slot_authoritative(9, sword);
        host.inventory
            .set_slot_authoritative(10, item("iron_ingot", 3));
        let mut menu = AnvilMenu::new(7, &host)?;
        click(&mut menu, &mut host, 3, 0, ClickType::QuickMove)?;
        click(&mut menu, &mut host, 4, 0, ClickType::QuickMove)?;
        assert_eq!(menu.state().preview().repair_material_count, 2);
        rename(&mut menu, &mut host, "Named")?;
        assert!(host
            .events
            .iter()
            .any(|e| e.name == "hover-name" && e.phase.repair_material_count == 2));
        host.events.clear();
        // Removing half of three materials leaves one; recomputation starts
        // from the previous two-material preview before reset0 and assignment1.
        click(&mut menu, &mut host, 1, 1, ClickType::Pickup)?;
        let repair = host
            .events
            .iter()
            .filter(|e| e.name == "repair-rule")
            .collect::<Vec<_>>();
        let hover = host
            .events
            .iter()
            .filter(|e| e.name == "hover-name")
            .collect::<Vec<_>>();
        assert!(!repair.is_empty() && !hover.is_empty());
        assert!(repair
            .iter()
            .all(|e| e.phase.level_cost == 1 && e.phase.repair_material_count == 0));
        assert!(hover
            .iter()
            .all(|e| e.phase.level_cost == 1 && e.phase.repair_material_count == 1));
        assert_eq!(menu.state().preview().repair_material_count, 1);
        Ok(())
    }
    #[test]
    fn unresolved_food_predictions_actual_stacks_and_reused_ui_never_publish() -> Result<()> {
        let explicit_food = |mut stack: ItemStack| {
            let data = stack.effective_component_bytes(20).unwrap().to_vec();
            stack.components.push(Component { type_id: 20, data });
            stack
        };
        let food = explicit_food(item("mushroom_stew", 1));
        let mut host = Host::new();
        host.inventory.set_slot_authoritative(9, item("stone", 1));
        let mut menu = AnvilMenu::new(7, &host)?;
        menu.initialize(&mut host)?;
        host.events.clear();
        host.sync.clear();
        let token = menu.token();
        let mut packet = ContainerClick {
            window_id: menu.window_id(),
            state_id: menu.state_id(),
            slot: 3,
            button: 0,
            kind: ClickType::Pickup,
            changed_slots: BTreeMap::new(),
            carried: food.clone(),
        };
        assert!(menu.click(token, &packet, &mut host).is_err());
        packet.carried = ItemStack::empty();
        packet.changed_slots.insert(3, food.clone());
        assert!(menu.click(token, &packet, &mut host).is_err());
        assert_eq!(menu.token(), token);
        assert!(menu.carried().is_empty());
        assert_eq!(host.inventory.slot(9), Some(&item("stone", 1)));
        assert!(host.events.is_empty() && host.sync.is_empty());
        host.inventory.set_slot_authoritative(45, food);
        assert!(menu.broadcast_changes(&mut host).is_err());
        assert!(menu.set_item_name(token, &"X".into(), &mut host).is_err());
        assert!(menu
            .close(token, MenuCloseContext::AliveConnected, &mut host)
            .is_err());
        assert_eq!(menu.token(), token);
        assert!(host.events.is_empty());
        // Primitive-only prototype elision certified by the shared helper stays
        // supported; equal wire bytes for nested ItemStacks do not grant that.
        host.inventory
            .set_slot_authoritative(45, explicit_food(item("bread", 1)));
        menu.broadcast_changes(&mut host)?;
        let now = menu.token();
        assert_ne!(now, token);
        packet.changed_slots.clear();
        assert!(menu.click(token, &packet, &mut host).is_err());
        assert!(menu.set_item_name(token, &"X".into(), &mut host).is_err());
        assert!(menu
            .close(token, MenuCloseContext::DeadOrDisconnected, &mut host)
            .is_err());
        let mut reopened = AnvilMenu::new(7, &host)?;
        assert!(reopened.click(now, &packet, &mut host).is_err());
        menu.close(now, MenuCloseContext::DeadOrDisconnected, &mut host)?;
        assert!(menu.click(menu.token(), &packet, &mut host).is_err());
        assert!(menu.initialize(&mut host).is_err());
        Ok(())
    }
    #[test]
    fn owned_book_combination_forwards_snapshot_and_commits_existing_cursor_xp_flow() -> Result<()>
    {
        use crate::workstations::anvil::enchantment_fixture::{enchanted, entries, id, Owner};
        let mut host = Host::new();
        host.enchantment_owner = Some(Owner::new());
        host.inventory.set_slot_authoritative(
            9,
            enchanted(item("iron_sword", 1), &[("sharpness", 2)], false),
        );
        host.inventory.set_slot_authoritative(
            10,
            enchanted(item("enchanted_book", 1), &[("sharpness", 2)], true),
        );
        let mut menu = AnvilMenu::new(7, &host)?;
        menu.initialize(&mut host)?;
        click(&mut menu, &mut host, 3, 0, ClickType::QuickMove)?;
        click(&mut menu, &mut host, 4, 0, ClickType::QuickMove)?;
        assert_eq!(menu.state().preview().level_cost, 3);
        assert_eq!(
            host.draws, 0,
            "preview/enchantment merging consumes no player RNG"
        );
        assert_eq!(
            entries(&menu.state().preview().stack)?,
            (vec![(id("sharpness"), 3)], false)
        );
        let token = menu.token();
        let events = host.events.len();
        host.enchantment_owner.as_mut().unwrap().epoch += 1;
        assert!(menu.broadcast_changes(&mut host).is_err());
        assert_eq!(menu.token(), token);
        assert_eq!(host.events.len(), events);
        host.enchantment_owner.as_mut().unwrap().epoch -= 1;
        click(&mut menu, &mut host, 2, 0, ClickType::Pickup)?;
        assert_eq!(host.xp, [-3]);
        assert_eq!(host.draws, 1);
        assert_eq!(entries(menu.carried())?.0, vec![(id("sharpness"), 3)]);
        assert!(menu.state().inputs().iter().all(ItemStack::is_empty));
        // A previously absent capability cannot appear under an open basic UI.
        let mut host = Host::new();
        let mut basic = AnvilMenu::new(7, &host)?;
        host.enchantment_owner = Some(Owner::new());
        assert!(basic.initialize(&mut host).is_err());
        Ok(())
    }
    #[test]
    fn detached_transaction_rolls_back_late_world_publish_and_drop_failures() -> Result<()> {
        // Direct PUBLIC actions: there is no outer cloned-menu rescue/reset.
        // The restricted fixture deep-stages every mutable field, then injects
        // failures after source XP/RNG/world/inventory/outbox effects occurred.
        for fail_publish in [false, true] {
            let (mut menu, mut host) = renamed_stack(4)?;
            host.block = 2;
            host.random = 0.0;
            host.fail_world_event = !fail_publish;
            host.fail_publish = fail_publish;
            let before = (format!("{menu:?}"), format!("{host:?}"));
            assert!(click(&mut menu, &mut host, 2, 0, ClickType::QuickMove).is_err());
            assert_eq!((format!("{menu:?}"), format!("{host:?}")), before);
            host.fail_world_event = false;
            host.fail_publish = false;
            click(&mut menu, &mut host, 2, 0, ClickType::QuickMove)?;
            assert_eq!(host.inventory.slot(44).unwrap().count, 4);
            assert_eq!(host.xp, [-1]);
            assert_eq!(host.draws, 1);
            assert_eq!(host.block, -1);
            assert!(menu.state().inputs().iter().all(ItemStack::is_empty));
        }
        let (mut menu, mut host) = renamed_stack(4)?;
        host.block = 2;
        host.random = 0.0;
        host.fail_drop = true;
        let before = (format!("{menu:?}"), format!("{host:?}"));
        assert!(click(&mut menu, &mut host, 2, 0, ClickType::Throw).is_err());
        assert_eq!((format!("{menu:?}"), format!("{host:?}")), before);
        Ok(())
    }
    #[test]
    fn full_state_resend_uses_transaction_and_retains_listener_baselines() -> Result<()> {
        let mut host = Host::new();
        let mut menu = AnvilMenu::new(7, &host)?;
        menu.initialize(&mut host)?;
        host.sync.clear();
        host.events.clear();
        host.inventory.set_slot_authoritative(9, item("stone", 3));
        let generation = host.generation;
        let before = menu.token();
        menu.broadcast_full_state(&mut host)?;
        assert!(matches!(host.last_action, Some(AnvilMenuAction::FullState)));
        assert_eq!(host.generation, generation + 1);
        assert_ne!(menu.token(), before);
        assert!(host
            .sync
            .iter()
            .any(|event| matches!(event, SyncEvent::SlotListener { menu_slot: 3, .. })));
        assert!(host
            .sync
            .iter()
            .any(|event| matches!(event,SyncEvent::Packet(packet) if packet.id==0x13)));
        assert!(host
            .sync
            .iter()
            .any(|event| matches!(event,SyncEvent::Packet(packet) if packet.id==0x14)));
        host.sync.clear();
        menu.broadcast_changes(&mut host)?;
        assert!(
            host.sync.is_empty(),
            "full state retained local and remote baselines"
        );
        assert!(host.xp.is_empty() && host.draws == 0 && host.drops.is_empty());
        Ok(())
    }
    #[test]
    fn unsupported_enchantment_transfer_discards_entire_actual_host_stage() -> Result<()> {
        let mut host = Host::new();
        host.inventory
            .set_slot_authoritative(9, item("iron_sword", 1));
        let mut menu = AnvilMenu::new(7, &host)?;
        click(&mut menu, &mut host, 3, 0, ClickType::QuickMove)?;
        let mut sacrifice = item("iron_sword", 1);
        let mut w = Writer::new();
        w.varint(1).varint(0).varint(1).bool(true);
        sacrifice.components.push(Component {
            type_id: 9,
            data: w.into_inner(),
        });
        host.inventory.set_slot_authoritative(10, sacrifice.clone());
        let before = host.events.len();
        assert!(click(&mut menu, &mut host, 4, 0, ClickType::QuickMove).is_err());
        assert_eq!(host.inventory.slot(10), Some(&sacrifice));
        assert!(menu.state().inputs()[1].is_empty());
        assert_eq!(host.events.len(), before);
        assert!(host.xp.is_empty());
        assert_eq!(host.draws, 0);
        assert!(host.events.iter().all(|e| e.inventory.slots().len() == 46));
        Ok(())
    }

    #[test]
    fn pickup_inventory_sync_prepares_packets_then_adopts_only_retained_baselines() -> Result<()> {
        let (mut menu, mut host) = renamed_stack(4)?;
        // This retained cursor/drag may already exist when an ItemEntity calls
        // ServerPlayer.take. A broadcast must not construct/reset the menu.
        menu.sync.set_carried(item("diamond", 2));
        menu.drag_status = 1;
        menu.drag_type = 1;
        menu.drag_slots.insert(3);
        host.inventory.set_slot_authoritative(9, item("apple", 3));
        host.inventory.set_carried_authoritative(item("emerald", 1));
        let actual_host = format!("{host:?}");
        let state = format!("{:?}", menu.state);
        let before_menu = format!("{menu:?}");
        let before_token = menu.token();
        let before_state_id = menu.state_id();
        let plan = menu.prepare_inventory_broadcast(&host.inventory)?;
        assert_eq!(format!("{menu:?}"), before_menu);
        assert!(std::ptr::eq(plan.inventory(), &host.inventory));
        assert_eq!(plan.phase().result_preview.count, 4);
        assert_eq!(plan.phase().carried, item("diamond", 2));
        assert!(plan.events().iter().any(|event| matches!(event,
            SyncEvent::SlotListener { menu_slot: 3, player_slot: Some(9), stack }
                if stack == &item("apple", 3))));
        let packet = plan
            .events()
            .iter()
            .find_map(|event| match event {
                SyncEvent::Packet(packet)
                    if packet.id == 0x15 && packet.payload.first() == Some(&menu.window_id()) =>
                {
                    Some(packet.clone())
                }
                _ => None,
            })
            .expect("actual retained inventory slot packet");
        let mut reader = crate::protocol::Reader::new(&packet.payload);
        assert_eq!(reader.u8()?, menu.window_id());
        assert_eq!(reader.varint()?, (before_state_id + 1) & 32767);
        assert_eq!(reader.i16()?, 3);
        assert_eq!(
            ItemStack::decode_exact(reader.remaining())?,
            item("apple", 3)
        );
        let capability = plan.into_validated(&mut menu)?;
        assert_eq!(capability.menu.token(), before_token); // not adopted yet
        let mut observed = Vec::new();
        // Actual Main must preflight its concrete listeners first. These
        // native readonly observations exercise the no-Host capability path.
        for event in capability.events() {
            if let SyncEvent::SlotListener {
                player_slot: Some(slot),
                ..
            } = event
            {
                observed.push((*slot, capability.inventory().slot(*slot).unwrap().clone()));
            }
        }
        assert!(observed.contains(&(9, item("apple", 3))));
        let next_token = capability.next_token();
        let publication = capability.commit(); // deliberately no Result/hook
        assert_eq!(menu.token(), next_token);
        assert_ne!(menu.token(), before_token);
        assert_eq!(format!("{:?}", menu.state), state);
        assert_eq!((menu.drag_status, menu.drag_type), (1, 1));
        assert_eq!(menu.drag_slots, BTreeSet::from([3]));
        assert_eq!(menu.carried(), &item("diamond", 2));
        assert_eq!(
            publication.phase.level_cost,
            menu.state.preview().level_cost
        );
        assert_eq!(format!("{host:?}"), actual_host); // no XP/RNG/world/host call
        assert!(menu
            .prepare_inventory_broadcast(&host.inventory)?
            .events()
            .is_empty());
        Ok(())
    }

    #[test]
    fn inventory_sync_cancellation_staleness_and_preparation_failure_publish_nothing() -> Result<()>
    {
        let (mut menu, mut host) = renamed_stack(4)?;
        host.inventory.set_slot_authoritative(9, item("apple", 3));
        let before = format!("{menu:?}");
        let plan = menu.prepare_inventory_broadcast(&host.inventory)?;
        assert!(!plan.events().is_empty());
        drop(plan);
        assert_eq!(format!("{menu:?}"), before);
        {
            let capability = menu
                .prepare_inventory_broadcast(&host.inventory)?
                .into_validated(&mut menu)?;
            drop(capability); // Main listener preflight refused before any hook
        }
        assert_eq!(format!("{menu:?}"), before);

        let stale = menu.prepare_inventory_broadcast(&host.inventory)?;
        menu.prepare_inventory_full_state(&host.inventory)?
            .into_validated(&mut menu)?
            .commit();
        let committed = format!("{menu:?}");
        assert!(stale.into_validated(&mut menu).is_err());
        assert_eq!(format!("{menu:?}"), committed);

        let stale = menu.prepare_inventory_broadcast(&host.inventory)?;
        let mut reopened = AnvilMenu::new(menu.window_id(), &host)?;
        assert!(stale.into_validated(&mut reopened).is_err());
        let stale = menu.prepare_inventory_broadcast(&host.inventory)?;
        menu.sync.suppress_remote_updates(true); // unexpected retained-sync drift
        assert!(stale.into_validated(&mut menu).is_err());
        menu.sync.suppress_remote_updates(false);

        let mut food = item("mushroom_stew", 1);
        let wire = food.effective_component_bytes(20).unwrap().to_vec();
        food.components.push(Component {
            type_id: 20,
            data: wire,
        });
        host.inventory.set_slot_authoritative(9, food);
        let before = format!("{menu:?}");
        assert!(menu.prepare_inventory_broadcast(&host.inventory).is_err());
        assert_eq!(format!("{menu:?}"), before);
        host.inventory.set_slot_authoritative(9, item("apple", 3));
        menu.token.revision = u64::MAX;
        let before = format!("{menu:?}");
        assert!(menu.prepare_inventory_broadcast(&host.inventory).is_err());
        assert_eq!(format!("{menu:?}"), before);
        Ok(())
    }

    #[test]
    fn inventory_full_state_sync_retains_name_cost_cursor_and_incremental_baseline() -> Result<()> {
        let (mut menu, mut host) = renamed_stack(4)?;
        menu.sync.set_carried(item("diamond", 2));
        host.inventory.set_slot_authoritative(9, item("apple", 3));
        let state = format!("{:?}", menu.state);
        let plan = menu.prepare_inventory_full_state(&host.inventory)?;
        let content = plan
            .events()
            .iter()
            .find_map(|event| match event {
                SyncEvent::Packet(packet) if packet.id == 0x13 => Some(packet),
                _ => None,
            })
            .expect("retained full content packet");
        let mut reader = crate::protocol::Reader::new(&content.payload);
        assert_eq!(reader.u8()?, menu.window_id());
        assert_eq!(reader.varint()?, (menu.state_id() + 1) & 32767);
        assert_eq!(reader.varint()?, MENU_SLOTS as i32);
        assert!(plan.events().iter().any(|event| matches!(event,
            SyncEvent::Packet(packet) if packet.id == 0x14)));
        plan.into_validated(&mut menu)?.commit();
        assert_eq!(format!("{:?}", menu.state), state);
        assert_eq!(menu.state.item_name(), Some(&JavaString::from("Named")));
        assert_eq!(menu.carried(), &item("diamond", 2));
        assert!(menu
            .prepare_inventory_broadcast(&host.inventory)?
            .events()
            .is_empty());
        Ok(())
    }
}
