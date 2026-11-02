//! Java 1.21 BeaconMenu: temporary payment/cursor and borrowed live BE data.
//! Every actual callback, packet and drop is staged on the mandatory real host;
//! publication observations must not execute these operations a second time.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    protocol::Reader,
    workstations::{
        beacon::{
            encode_effect, BeaconEffect, BeaconHost, BeaconOpenAccess, BeaconRegistry,
            BeaconSourceEvent, BeaconState, BeaconUpdate, MENU_SLOTS,
        },
        menu::{MenuRemovalEvent, MenuSync, SyncEvent, WorkstationKind},
    },
};
use anyhow::{bail, ensure, Result};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeaconMenuToken {
    instance: u64,
    revision: u64,
}
#[derive(Clone, Copy, Debug)]
pub enum BeaconAction {
    Open,
    Initialize,
    Broadcast,
    FullState,
    Click(ClickType),
    Update,
    SetData,
    Close(MenuCloseContext),
}
/// Use actual captured BeaconBlockEntity/ContainerData, level/position/player,
/// LockCode, registry/tag/feature owners and callback/RNG/drop/packet owners.
/// Key includes their identities and registry/tag revisions. Revision covers
/// mutable source owners. Staging MUST detach all mutable state; atomic publish
/// must leave live owners unchanged on error. No default/no-op host is supplied.
pub trait BeaconMenuHost: BeaconHost + BeaconOpenAccess + Sized {
    type Key: Clone + Eq + Debug;
    type Revision: Clone + Eq + Debug;
    fn owner_key(&self) -> &Self::Key;
    fn revision(&self) -> Self::Revision;
    fn ensure_supported_context(
        &self,
        token: Option<BeaconMenuToken>,
        action: BeaconAction,
    ) -> Result<()>;
    fn stage(&self, token: Option<BeaconMenuToken>, action: BeaconAction) -> Result<Self>;
    fn publish_stage(&mut self, staged: Self, token: Option<BeaconMenuToken>) -> Result<()>;
    /// Stage actual counter/container listener/sync/packet/drop operations here.
    /// Data-set/BlockEntityChanged source records are observations of operations
    /// already executed once by BeaconHost; never repeat sound/filter/dirty calls.
    fn stage_event(&mut self, event: &BeaconMenuEvent, drops: &[DroppedStack]) -> Result<()>;
    /// Certify ALL native sync/listener/counter/cursor and actual callback effects
    /// insensitive to Slot HashSet order, or reject. Custom no-op observers alone
    /// do not establish equivalence. Rechecked with the same targets/phase.
    fn quickcraft_callbacks_are_order_independent(
        &self,
        targets: &BTreeSet<usize>,
        phase: &BeaconPhase,
    ) -> Result<bool>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeaconMenu<K: Clone + Eq + Debug> {
    owner_key: K,
    station: BeaconState,
    sync: MenuSync,
    instance: u64,
    revision: u64,
    closed: bool,
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
}
#[derive(Clone, Debug)]
pub struct BeaconPhase {
    pub payment: ItemStack,
    pub data: [i32; 3],
    pub player_inventory: PlayerInventory,
    pub carried: ItemStack,
}
#[derive(Clone, Debug)]
pub enum BeaconMenuEvent {
    Source {
        event: BeaconSourceEvent,
        phase: BeaconPhase,
    },
    PlayerInventoryChanged {
        phase: BeaconPhase,
    },
    Sync {
        event: SyncEvent,
        phase: BeaconPhase,
    },
    Drop {
        index: usize,
        phase: BeaconPhase,
    },
}
#[derive(Debug, Default)]
pub struct BeaconOutcome {
    pub events: Vec<BeaconMenuEvent>,
    pub drops: Vec<DroppedStack>,
    pub update: Option<BeaconUpdate>,
}
/// Typed SET_BEACON payload: optional presence boolean and RAW holder ID twice.
/// No window/state ID; Main must dispatch against the actual active BeaconMenu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeaconUpdatePacket {
    pub primary: Option<BeaconEffect>,
    pub secondary: Option<BeaconEffect>,
}
impl BeaconUpdatePacket {
    pub fn decode(payload: &[u8], registry: &impl BeaconRegistry) -> Result<Self> {
        fn effect(
            r: &mut Reader<'_>,
            registry: &impl BeaconRegistry,
        ) -> Result<Option<BeaconEffect>> {
            if !r.bool()? {
                return Ok(None);
            }
            let id = r.varint()?;
            ensure!(
                id >= 0 && registry.effect_registered(id)?,
                "unknown beacon packet holder"
            );
            Ok(Some(BeaconEffect { registry_id: id }))
        }
        let mut r = Reader::new(payload);
        let packet = Self {
            primary: effect(&mut r, registry)?,
            secondary: effect(&mut r, registry)?,
        };
        r.finish()?;
        Ok(packet)
    }
}
impl BeaconPhase {
    /// Observe a raw per-insertion packet without sending it. Returning is an
    /// observation of either the carried object or a removed grid stack.
    fn observe_return_packet(
        &mut self,
        packet: &crate::protocol::Packet,
        returning: &mut ItemStack,
    ) -> Result<()> {
        ensure!(packet.id == 0x15, "unsupported beacon close packet phase");
        let mut reader = crate::protocol::Reader::new(&packet.payload);
        ensure!(
            reader.i8()? == -2 && reader.varint()? == 0,
            "beacon close requires raw inventory packet"
        );
        let raw = reader.i16()?;
        let slot = match raw {
            0..=8 => 36 + raw as usize,
            9..=35 => raw as usize,
            40 => 45,
            _ => bail!("unsupported beacon close raw slot"),
        };
        let stack = ItemStack::decode_exact(reader.remaining())?;
        let old = self.player_inventory.slot(slot).unwrap();
        ensure!(
            stack.same_item_same_components(returning)?,
            "beacon close changed return identity"
        );
        let old_count = if old.is_empty() {
            0
        } else {
            ensure!(
                old.same_item_same_components(&stack)?,
                "beacon close overwrote owned stack"
            );
            old.count
        };
        let added = stack.count - old_count;
        ensure!(
            added > 0 && added <= returning.count,
            "invalid beacon close insertion amount"
        );
        self.player_inventory.set_slot_authoritative(slot, stack);
        *returning = counted(returning, returning.count - added);
        Ok(())
    }
}

pub struct BeaconOpening<H: BeaconMenuHost> {
    key: H::Key,
    revision: H::Revision,
    before_data: [i32; 3],
    inventory: PlayerInventory,
    staged: H,
    next: Option<BeaconMenu<H::Key>>,
}
impl<H: BeaconMenuHost> BeaconOpening<H> {
    pub fn allowed(&self) -> bool {
        self.next.is_some()
    }
    /// Publish real unlock-denial message/sound even when no menu was created.
    pub fn commit(
        self,
        inventory: &PlayerInventory,
        host: &mut H,
    ) -> Result<Option<BeaconMenu<H::Key>>> {
        check_inventory_identity(inventory)?;
        ensure!(
            same_inventory(inventory, &self.inventory),
            "beacon opening inventory changed"
        );
        ensure!(
            *host.owner_key() == self.key
                && host.revision() == self.revision
                && host.data()? == self.before_data,
            "beacon opening owner changed"
        );
        host.ensure_supported_context(None, BeaconAction::Open)?;
        host.publish_stage(self.staged, None)?;
        Ok(self.next)
    }
}
pub struct BeaconPlan<H: BeaconMenuHost> {
    before: BeaconMenu<H::Key>,
    before_inventory: PlayerInventory,
    before_host_revision: H::Revision,
    before_data: [i32; 3],
    action: BeaconAction,
    expected_validity: Option<bool>,
    require_drag: bool,
    next: BeaconMenu<H::Key>,
    next_inventory: PlayerInventory,
    staged: H,
    outcome: BeaconOutcome,
}
impl<H: BeaconMenuHost> BeaconPlan<H> {
    pub fn outcome(&self) -> &BeaconOutcome {
        &self.outcome
    }
    pub fn next_menu(&self) -> &BeaconMenu<H::Key> {
        &self.next
    }
    pub fn next_inventory(&self) -> &PlayerInventory {
        &self.next_inventory
    }
    pub fn staged_host(&self) -> &H {
        &self.staged
    }
    pub fn validate(
        &self,
        menu: &BeaconMenu<H::Key>,
        inventory: &PlayerInventory,
        host: &H,
    ) -> Result<()> {
        menu.check_open()?;
        menu.check_identity(inventory)?;
        ensure!(*menu == self.before, "stale beacon UI");
        ensure!(
            same_inventory(inventory, &self.before_inventory),
            "beacon canonical inventory changed"
        );
        ensure!(
            *host.owner_key() == self.before.owner_key
                && host.revision() == self.before_host_revision
                && host.data()? == self.before_data,
            "beacon captured live owner/data changed"
        );
        host.ensure_supported_context(Some(menu.token()), self.action)?;
        if let Some(valid) = self.expected_validity {
            ensure!(
                host.menu_still_valid()? == valid,
                "beacon player/block validity changed"
            );
        }
        if self.require_drag {
            ensure!(
                host.quickcraft_callbacks_are_order_independent(
                    &self.before.quickcraft_slots,
                    &self.before.phase(inventory, host)?
                )?,
                "beacon drag context changed"
            );
        }
        Ok(())
    }
    pub fn commit(
        self,
        menu: &mut BeaconMenu<H::Key>,
        inventory: &mut PlayerInventory,
        host: &mut H,
    ) -> Result<BeaconOutcome> {
        self.validate(menu, inventory, host)?;
        host.publish_stage(self.staged, Some(menu.token()))?;
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
    ensure!((1..=99).contains(&max), "invalid beacon menu stack limit");
    Ok(max)
}

impl<K: Clone + Eq + Debug> BeaconMenu<K> {
    pub fn prepare_open<H: BeaconMenuHost<Key = K>>(
        window_id: u8,
        inventory: &PlayerInventory,
        host: &H,
    ) -> Result<BeaconOpening<H>> {
        check_inventory_identity(inventory)?;
        host.ensure_supported_context(None, BeaconAction::Open)?;
        let revision = host.revision();
        let before_data = host.data()?;
        let key = host.owner_key().clone();
        let mut staged = host.stage(None, BeaconAction::Open)?;
        ensure!(
            *staged.owner_key() == key
                && staged.revision() == revision
                && staged.data()? == host.data()?,
            "beacon opening stage changed base"
        );
        staged.ensure_supported_context(None, BeaconAction::Open)?;
        let next = if let Some(station) = BeaconState::open(&mut staged)? {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let instance = NEXT
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_err(|_| anyhow::anyhow!("beacon UI identity exhausted"))?;
            Some(Self {
                owner_key: key.clone(),
                station,
                sync: MenuSync::new(window_id, WorkstationKind::Beacon)?,
                instance,
                revision: 0,
                closed: false,
                quickcraft_status: 0,
                quickcraft_type: 0,
                quickcraft_slots: BTreeSet::new(),
            })
        } else {
            None
        };
        ensure!(
            *staged.owner_key() == key,
            "beacon lock callback changed captured owner"
        );
        Ok(BeaconOpening {
            key,
            revision,
            before_data,
            inventory: inventory.clone(),
            staged,
            next,
        })
    }
    pub fn token(&self) -> BeaconMenuToken {
        BeaconMenuToken {
            instance: self.instance,
            revision: self.revision,
        }
    }
    pub fn state(&self) -> &BeaconState {
        &self.station
    }
    pub fn sync(&self) -> &MenuSync {
        &self.sync
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    /// Source block type/reach validity, separate from the captured data owner.
    pub fn still_valid(&self, host: &impl BeaconMenuHost<Key = K>) -> Result<bool> {
        self.check_open()?;
        ensure!(
            *host.owner_key() == self.owner_key,
            "beacon captured data owner changed"
        );
        host.menu_still_valid()
    }
    pub fn data(&self, host: &impl BeaconMenuHost<Key = K>) -> Result<[i32; 3]> {
        self.check_open()?;
        ensure!(
            *host.owner_key() == self.owner_key,
            "beacon captured data owner changed"
        );
        host.data()
    }
    fn check_open(&self) -> Result<()> {
        ensure!(!self.closed, "beacon UI is closed");
        Ok(())
    }
    fn check_identity(&self, inventory: &PlayerInventory) -> Result<()> {
        check_inventory_identity(inventory)?;
        reject_food(self.station.payment())?;
        reject_food(self.sync.carried())?;
        Ok(())
    }
    fn phase(&self, inventory: &PlayerInventory, host: &impl BeaconHost) -> Result<BeaconPhase> {
        Ok(BeaconPhase {
            payment: self.station.payment().clone(),
            data: host.data()?,
            player_inventory: inventory.clone(),
            carried: self.sync.carried().clone(),
        })
    }
    fn top(&self) -> [ItemStack; 1] {
        [self.station.payment().clone()]
    }
    fn advance_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("beacon UI revision exhausted"))?;
        Ok(())
    }
    fn make_plan<H: BeaconMenuHost<Key = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &H,
        action: BeaconAction,
        body: impl FnOnce(
            &mut Working<'_, K, H>,
            &mut PlayerInventory,
            &mut BeaconOutcome,
        ) -> Result<()>,
    ) -> Result<BeaconPlan<H>> {
        self.check_open()?;
        self.check_identity(inventory)?;
        host.ensure_supported_context(Some(self.token()), action)?;
        ensure!(
            *host.owner_key() == self.owner_key,
            "beacon physical/data owner replaced"
        );
        let before_host_revision = host.revision();
        let before_data = host.data()?;
        let mut staged = host.stage(Some(self.token()), action)?;
        ensure!(
            *staged.owner_key() == self.owner_key
                && staged.revision() == before_host_revision
                && staged.data()? == before_data,
            "beacon stage changed actual base"
        );
        staged.ensure_supported_context(Some(self.token()), action)?;
        let expected_validity = match action {
            BeaconAction::Click(_) => {
                ensure!(
                    host.menu_still_valid()? && staged.menu_still_valid()?,
                    "invalid beacon click"
                );
                Some(true)
            }
            BeaconAction::Update => {
                let valid = host.menu_still_valid()?;
                ensure!(
                    staged.menu_still_valid()? == valid,
                    "beacon stage changed validity"
                );
                Some(valid)
            }
            _ => None,
        };
        let mut next = self.clone();
        let mut inv = inventory.clone();
        let mut outcome = BeaconOutcome::default();
        let mut require_drag = false;
        body(
            &mut Working {
                menu: &mut next,
                host: &mut staged,
                used_drag: &mut require_drag,
            },
            &mut inv,
            &mut outcome,
        )?;
        next.advance_revision()?;
        ensure!(
            *staged.owner_key() == self.owner_key,
            "beacon callbacks changed captured owner"
        );
        Ok(BeaconPlan {
            before: self.clone(),
            before_inventory: inventory.clone(),
            before_host_revision,
            before_data,
            action,
            expected_validity,
            require_drag,
            next,
            next_inventory: inv,
            staged,
            outcome,
        })
    }
    pub fn prepare_initialize<H: BeaconMenuHost<Key = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &H,
    ) -> Result<BeaconPlan<H>> {
        self.make_plan(inventory, host, BeaconAction::Initialize, |w, inv, out| {
            let data = w.host.data()?;
            let top = w.top();
            let events = w.menu.sync.initialize(&top, inv, &data)?;
            w.sync_events(events, inv, out)
        })
    }
    pub fn prepare_broadcast<H: BeaconMenuHost<Key = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &H,
    ) -> Result<BeaconPlan<H>> {
        self.make_plan(inventory, host, BeaconAction::Broadcast, |w, inv, out| {
            w.broadcast(inv, out)
        })
    }
    pub fn prepare_full_state<H: BeaconMenuHost<Key = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &H,
    ) -> Result<BeaconPlan<H>> {
        self.make_plan(inventory, host, BeaconAction::FullState, |w, inv, out| {
            let data = w.host.data()?;
            let top = w.top();
            let events = w.menu.sync.broadcast_full_state(&top, inv, &data)?;
            w.sync_events(events, inv, out)
        })
    }
    pub fn prepare_click<H: BeaconMenuHost<Key = K>>(
        &self,
        click: &ContainerClick,
        inventory: &PlayerInventory,
        host: &H,
        creative: bool,
    ) -> Result<BeaconPlan<H>> {
        ensure!(
            click.window_id == self.sync.window_id() && click.slot < MENU_SLOTS as i16,
            "invalid beacon click window/slot"
        );
        for stack in click
            .changed_slots
            .values()
            .chain(std::iter::once(&click.carried))
        {
            reject_food(stack)?;
        }
        self.make_plan(
            inventory,
            host,
            BeaconAction::Click(click.kind),
            |w, inv, out| {
                let before = w.sync.state_id();
                w.menu.sync.suppress_remote_updates(true);
                w.do_click(
                    click.slot as i32,
                    click.button as i32,
                    click.kind,
                    creative,
                    inv,
                    out,
                )?;
                let top = w.top();
                let data = w.host.data()?;
                let events = w.menu.sync.finish_click(click, before, &top, inv, &data)?;
                w.sync_events(events, inv, out)
            },
        )
    }
    pub fn prepare_update<H: BeaconMenuHost<Key = K>>(
        &self,
        packet: BeaconUpdatePacket,
        inventory: &PlayerInventory,
        host: &H,
    ) -> Result<BeaconPlan<H>> {
        // A manually constructed typed packet must satisfy the same real
        // holder admission as the wire codec, even for an ignored action.
        encode_effect(packet.primary, host)?;
        encode_effect(packet.secondary, host)?;
        self.make_plan(inventory, host, BeaconAction::Update, |w, inv, out| {
            let cursor = w.sync.carried().clone();
            let status = w.menu.station.update_effects_observed(
                packet.primary,
                packet.secondary,
                w.host,
                |event, state, host| {
                    let phase = BeaconPhase {
                        payment: state.payment().clone(),
                        data: host.data()?,
                        player_inventory: inv.clone(),
                        carried: cursor.clone(),
                    };
                    let event = BeaconMenuEvent::Source { event, phase };
                    host.stage_event(&event, &out.drops)?;
                    out.events.push(event);
                    Ok(())
                },
            )?;
            out.update = Some(status);
            Ok(())
        })
    }
    pub fn prepare_set_data<H: BeaconMenuHost<Key = K>>(
        &self,
        index: usize,
        value: i32,
        inventory: &PlayerInventory,
        host: &H,
    ) -> Result<BeaconPlan<H>> {
        ensure!(index < 3, "invalid beacon data index");
        self.make_plan(inventory, host, BeaconAction::SetData, |w, inv, out| {
            w.source(BeaconSourceEvent::BeforeDataSet { index, value }, inv, out)?;
            w.host.set_data(index, value)?;
            w.source(BeaconSourceEvent::AfterDataSet { index, value }, inv, out)?;
            w.broadcast(inv, out)
        })
    }
    pub fn prepare_close<H: BeaconMenuHost<Key = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &H,
        context: MenuCloseContext,
    ) -> Result<BeaconPlan<H>> {
        self.make_plan(
            inventory,
            host,
            BeaconAction::Close(context),
            |w, inv, out| {
                w.reset_quickcraft();
                let mut phase = w.phase(inv)?;
                let mut returning = phase.carried.clone();
                let removal = w.menu.sync.close_inputs_ordered(&mut [], inv, context)?;
                let base = out.drops.len();
                out.drops.extend(removal.outcome.drops);
                for event in removal.events {
                    let event = match event {
                        MenuRemovalEvent::Packet { index } => {
                            let packet = removal.outcome.packets[index].clone();
                            phase.observe_return_packet(&packet, &mut returning)?;
                            phase.carried = returning.clone();
                            BeaconMenuEvent::Sync {
                                event: SyncEvent::Packet(packet),
                                phase: phase.clone(),
                            }
                        }
                        MenuRemovalEvent::Drop { index } => {
                            phase.carried = out.drops[base + index].stack.clone();
                            let event = BeaconMenuEvent::Drop {
                                index: base + index,
                                phase: phase.clone(),
                            };
                            returning = ItemStack::empty();
                            phase.carried = ItemStack::empty();
                            event
                        }
                    };
                    w.emit(event, out)?;
                }
                // Payment is removed with Slot.remove, not ordinary inventory-return logic.
                let payment = w.station.payment().clone();
                if !payment.is_empty() {
                    w.menu.station.replace_payment_raw(ItemStack::empty())?;
                    w.source(BeaconSourceEvent::PaymentContainerChanged, inv, out)?;
                    let index = out.drops.len();
                    out.drops.push(DroppedStack {
                        stack: payment,
                        around: false,
                        trace_item: false,
                    });
                    w.emit(
                        BeaconMenuEvent::Drop {
                            index,
                            phase: w.phase(inv)?,
                        },
                        out,
                    )?;
                }
                w.closed = true;
                Ok(())
            },
        )
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
}
struct Working<'a, K: Clone + Eq + Debug, H: BeaconMenuHost<Key = K>> {
    menu: &'a mut BeaconMenu<K>,
    host: &'a mut H,
    used_drag: &'a mut bool,
}
impl<K: Clone + Eq + Debug, H: BeaconMenuHost<Key = K>> Deref for Working<'_, K, H> {
    type Target = BeaconMenu<K>;
    fn deref(&self) -> &Self::Target {
        self.menu
    }
}
impl<K: Clone + Eq + Debug, H: BeaconMenuHost<Key = K>> DerefMut for Working<'_, K, H> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.menu
    }
}
impl<K: Clone + Eq + Debug, H: BeaconMenuHost<Key = K>> Working<'_, K, H> {
    fn phase(&self, inv: &PlayerInventory) -> Result<BeaconPhase> {
        self.menu.phase(inv, self.host)
    }
    fn emit(&mut self, event: BeaconMenuEvent, out: &mut BeaconOutcome) -> Result<()> {
        self.host.stage_event(&event, &out.drops)?;
        out.events.push(event);
        Ok(())
    }
    fn source(
        &mut self,
        event: BeaconSourceEvent,
        inv: &PlayerInventory,
        out: &mut BeaconOutcome,
    ) -> Result<()> {
        self.emit(
            BeaconMenuEvent::Source {
                event,
                phase: self.phase(inv)?,
            },
            out,
        )
    }
    fn sync_events(
        &mut self,
        events: Vec<SyncEvent>,
        inv: &PlayerInventory,
        out: &mut BeaconOutcome,
    ) -> Result<()> {
        for event in events {
            self.emit(
                BeaconMenuEvent::Sync {
                    event,
                    phase: self.phase(inv)?,
                },
                out,
            )?;
        }
        Ok(())
    }
    fn broadcast(&mut self, inv: &PlayerInventory, out: &mut BeaconOutcome) -> Result<()> {
        let top = self.top();
        let data = self.host.data()?;
        let events = self.menu.sync.broadcast_changes(&top, inv, &data)?;
        self.sync_events(events, inv, out)
    }
    fn player_changed(&mut self, inv: &PlayerInventory, out: &mut BeaconOutcome) -> Result<()> {
        self.emit(
            BeaconMenuEvent::PlayerInventoryChanged {
                phase: self.phase(inv)?,
            },
            out,
        )
    }
    fn player_changed_with_source(
        &mut self,
        inv: &PlayerInventory,
        out: &mut BeaconOutcome,
        _slot: usize,
        _source: &ItemStack,
    ) -> Result<()> {
        self.player_changed(inv, out)
    }
    fn slot_limit(&self, slot: usize, stack: &ItemStack) -> Result<i32> {
        Ok(if slot == 0 { 1 } else { limit(stack)? })
    }
    fn slot(&self, slot: usize, inventory: &PlayerInventory) -> ItemStack {
        if slot == 0 {
            self.station.payment().clone()
        } else {
            inventory.slot(slot + 8).unwrap().clone()
        }
    }
    fn raw_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        inventory: &mut PlayerInventory,
    ) -> Result<()> {
        if slot == 0 {
            self.menu.station.replace_payment_raw(stack)?;
        } else {
            inventory.set_slot_authoritative(slot + 8, stack);
        }
        Ok(())
    }
    fn change_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        inventory: &mut PlayerInventory,
        outcome: &mut BeaconOutcome,
    ) -> Result<()> {
        self.raw_slot(slot, stack, inventory)?;
        if slot == 0 {
            self.slot_changed(slot, inventory, outcome)?;
        }
        Ok(())
    }
    fn put_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        inventory: &mut PlayerInventory,
        outcome: &mut BeaconOutcome,
    ) -> Result<()> {
        self.change_slot(slot, stack, inventory, outcome)?;
        self.slot_changed(slot, inventory, outcome)
    }
    fn slot_changed(
        &mut self,
        slot: usize,
        inventory: &PlayerInventory,
        outcome: &mut BeaconOutcome,
    ) -> Result<()> {
        if slot == 0 {
            self.source(
                BeaconSourceEvent::PaymentContainerChanged,
                inventory,
                outcome,
            )
        } else {
            self.player_changed(inventory, outcome)
        }
    }
    fn on_take(
        &mut self,
        slot: usize,
        inventory: &mut PlayerInventory,
        outcome: &mut BeaconOutcome,
    ) -> Result<()> {
        self.slot_changed(slot, inventory, outcome)
    }
    fn may_place(&self, slot: usize, stack: &ItemStack) -> Result<bool> {
        if slot == 0 {
            BeaconState::may_place(stack, self.host)
        } else {
            Ok(true)
        }
    }
    fn reset_quickcraft(&mut self) {
        self.quickcraft_status = 0;
        self.quickcraft_slots.clear();
    }
    fn drop_stack(
        &mut self,
        outcome: &mut BeaconOutcome,
        stack: ItemStack,
        inventory: &PlayerInventory,
    ) -> Result<()> {
        if !stack.is_empty() {
            let index = outcome.drops.len();
            outcome.drops.push(DroppedStack {
                stack,
                around: false,
                trace_item: true,
            });
            self.emit(
                BeaconMenuEvent::Drop {
                    index,
                    phase: self.phase(inventory)?,
                },
                outcome,
            )?;
        }
        Ok(())
    }
    fn may_pickup(&self, _slot: usize) -> Result<bool> {
        Ok(true)
    }
    fn take_slot(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        inventory: &mut PlayerInventory,
        outcome: &mut BeaconOutcome,
    ) -> Result<ItemStack> {
        self.take_slot_with_cursor(slot, amount, maximum, None, inventory, outcome)
    }
    fn take_slot_with_cursor(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        cursor: Option<ItemStack>,
        inventory: &mut PlayerInventory,
        outcome: &mut BeaconOutcome,
    ) -> Result<ItemStack> {
        let source = self.slot(slot, inventory);
        if source.is_empty() || amount <= 0 || maximum <= 0 {
            return Ok(ItemStack::empty());
        }
        let count = source.count.min(amount).min(maximum);
        self.change_slot(
            slot,
            counted(&source, source.count - count),
            inventory,
            outcome,
        )?;
        if self.slot(slot, inventory).is_empty() {
            self.put_slot(slot, ItemStack::empty(), inventory, outcome)?;
        }
        if let Some(cursor) = cursor {
            self.sync.set_carried(cursor);
        }
        self.on_take(slot, inventory, outcome)?;
        Ok(counted(&source, count))
    }
    fn do_click(
        &mut self,
        slot: i32,
        button: i32,
        kind: ClickType,
        creative: bool,
        inventory: &mut PlayerInventory,
        outcome: &mut BeaconOutcome,
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
                        self.drop_stack(outcome, counted(&carried, n), inventory)?;
                        self.sync.set_carried(ItemStack::empty());
                    } else {
                        self.sync.set_carried(counted(&carried, carried.count - n));
                        self.drop_stack(outcome, counted(&carried, n), inventory)?;
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
                self.drop_stack(outcome, taken, inventory)?;
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
        outcome: &mut BeaconOutcome,
    ) -> Result<()> {
        let source = self.slot(slot, inventory);
        let carried = self.sync.carried().clone();
        reject_bundle(&source)?;
        reject_bundle(&carried)?;
        if carried.is_empty() {
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
                let max = self.slot_limit(slot, &carried)?;
                let n =
                    (if button == 0 { carried.count } else { 1 }).min(max - source.count.max(0));
                self.sync.set_carried(counted(&carried, carried.count - n));
                self.put_slot(
                    slot,
                    counted(&carried, source.count.max(0) + n),
                    inventory,
                    outcome,
                )?;
            } else if carried.count <= self.slot_limit(slot, &carried)? {
                self.sync.set_carried(source);
                self.put_slot(slot, carried, inventory, outcome)?;
            }
        }
        // cpu.doClick ends ordinary PICKUP with Slot.setChanged, even for a
        // rejected/no-op insertion. Payment uses its plain container callback.
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
        outcome: &mut BeaconOutcome,
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
                    let amount = stack
                        .count
                        .min((self.slot_limit(slot, stack)? - target.count).max(0));
                    if amount > 0 {
                        *stack = counted(stack, stack.count - amount);
                        self.raw_slot(source_slot, stack.clone(), inventory)?;
                        // moveItemStackTo merges by mutating the existing stack
                        // then Slot.setChanged, not by Slot.setByPlayer.
                        self.change_slot(
                            slot,
                            counted(&target, target.count + amount),
                            inventory,
                            outcome,
                        )?;
                        if slot > 0 {
                            self.player_changed_with_source(
                                inventory,
                                outcome,
                                source_slot,
                                stack,
                            )?;
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
                    let n = stack.count.min(self.slot_limit(slot, stack)?);
                    let inserted = counted(stack, n);
                    *stack = counted(stack, stack.count - n);
                    self.raw_slot(source_slot, stack.clone(), inventory)?;
                    if slot > 0 {
                        self.change_slot(slot, inserted, inventory, outcome)?;
                        self.player_changed_with_source(inventory, outcome, source_slot, stack)?;
                        self.player_changed_with_source(inventory, outcome, source_slot, stack)?;
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
        outcome: &mut BeaconOutcome,
    ) -> Result<()> {
        loop {
            let original = self.slot(slot, inventory);
            reject_bundle(&original)?;
            if original.is_empty() {
                break;
            }
            let mut moving = original.clone();
            let Some((range, reverse)) =
                self.station.quick_move_target(slot, &moving, self.host)?
            else {
                break;
            };
            if !self.move_to(
                &mut moving,
                range.start,
                range.end,
                reverse,
                inventory,
                outcome,
                slot,
            )? {
                break;
            }
            if moving.is_empty() {
                self.put_slot(slot, ItemStack::empty(), inventory, outcome)?;
            } else {
                self.slot_changed(slot, inventory, outcome)?;
            }
            if moving.count == original.count {
                break;
            }
            self.on_take(slot, inventory, outcome)?;
            if self.slot(slot, inventory).item_id != original.item_id {
                break;
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
        outcome: &mut BeaconOutcome,
    ) -> Result<()> {
        let player_slot = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        if slot > 0 && slot + 8 == player_slot {
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
                self.drop_stack(outcome, remainder, inventory)?;
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
                self.put_slot(slot, ItemStack::empty(), inventory, outcome)?;
                self.on_take(slot, inventory, outcome)?;
            }
        } else if source.is_empty() {
            if self.may_place(slot, &held)? {
                let n = held.count.min(self.slot_limit(slot, &held)?);
                inventory.set_slot_authoritative(player_slot, counted(&held, held.count - n));
                self.put_slot(slot, counted(&held, n), inventory, outcome)?;
            }
        } else if self.may_pickup(slot)? && self.may_place(slot, &held)? {
            let maximum = self.slot_limit(slot, &held)?;
            if held.count > maximum {
                inventory.set_slot_authoritative(player_slot, counted(&held, held.count - maximum));
                self.put_slot(slot, counted(&held, maximum), inventory, outcome)?;
                self.slot_changed(slot, inventory, outcome)?;
                let mut old = source;
                Self::add_to_inventory(&mut old, inventory, creative)?;
                self.drop_stack(outcome, old, inventory)?;
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
        outcome: &mut BeaconOutcome,
    ) -> Result<()> {
        let carried = self.sync.carried().clone();
        reject_bundle(&carried)?;
        if carried.is_empty()
            || !self.slot(clicked, inventory).is_empty() && self.may_pickup(clicked)?
        {
            return Ok(());
        }
        let order = if button == 0 {
            (0..37).collect::<Vec<_>>()
        } else {
            (0..37).rev().collect()
        };
        for pass in 0..2 {
            for &slot in &order {
                let source = self.slot(slot, inventory);
                let carried = self.sync.carried().clone();
                if carried.count >= carried.max_stack_size()? {
                    break;
                }
                if source.is_empty()
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
        outcome: &mut BeaconOutcome,
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
                if self.quickcraft_slots.len() > 1 {
                    *self.used_drag = true;
                    ensure!(
                        self.host.quickcraft_callbacks_are_order_independent(
                            &self.quickcraft_slots,
                            &self.phase(inventory)?
                        )?,
                        "unsupported order-sensitive beacon drag callbacks"
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
                    let count = (each + source.count).min(self.slot_limit(slot, &carried)?);
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

/// Decoded FoodProperties overrides retain nested ItemStack reference identity
/// in Java. Wire/semantic equality cannot recover it. Stable item prototypes
/// have their actual shared default value and remain admitted.
fn reject_food(stack: &ItemStack) -> Result<()> {
    ensure!(
        !stack
            .components
            .iter()
            .any(|component| component.type_id == 20),
        "beacon food component nested-stack Java identity unresolved"
    );
    Ok(())
}

fn check_inventory_identity(inventory: &PlayerInventory) -> Result<()> {
    for item in inventory
        .slots()
        .iter()
        .chain(std::iter::once(inventory.carried()))
    {
        reject_food(item)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::RecipeBook;
    use std::collections::BTreeMap;
    #[derive(Clone)]
    struct Host {
        key: u64,
        revision: u64,
        data: [i32; 3],
        valid: bool,
        unlock: bool,
        beam: bool,
        supported: bool,
        drag: bool,
        fail_event: bool,
        fail_publish: bool,
        fail_after_primary: bool,
        events: Vec<BeaconMenuEvent>,
        calls: Vec<String>,
        dirty: usize,
    }
    impl Host {
        fn new() -> Self {
            Self {
                key: 73,
                revision: 0,
                data: [0; 3],
                valid: true,
                unlock: true,
                beam: true,
                supported: true,
                drag: true,
                fail_event: false,
                fail_publish: false,
                fail_after_primary: false,
                events: vec![],
                calls: vec![],
                dirty: 0,
            }
        }
    }
    impl BeaconRegistry for Host {
        fn is_payment_item(&self, stack: &ItemStack) -> Result<bool> {
            Ok(inventory::item_name(stack.item_id) == Some("emerald"))
        }
        fn effect_registered(&self, id: i32) -> Result<bool> {
            Ok((0..12).contains(&id))
        }
    }
    impl BeaconOpenAccess for Host {
        fn can_unlock(&mut self) -> Result<bool> {
            if !self.unlock {
                self.calls.push("actual-lock-denial".into());
                self.revision += 1;
            }
            Ok(self.unlock)
        }
    }
    impl BeaconHost for Host {
        fn menu_still_valid(&self) -> Result<bool> {
            Ok(self.valid)
        }
        fn data(&self) -> Result<[i32; 3]> {
            Ok(self.data)
        }
        fn set_data(&mut self, index: usize, value: i32) -> Result<()> {
            if index == 1 && self.beam {
                self.calls.push(format!("sound-before:{}", self.data[1]));
            }
            self.calls.push(format!("data:{index}:{value}"));
            self.data[index] = if index == 0 || [0, 1, 3, 5].contains(&value) {
                value
            } else {
                0
            };
            self.revision += 1;
            Ok(())
        }
        fn block_entity_changed(&mut self, payment: &ItemStack) -> Result<()> {
            assert!(payment.is_empty());
            self.calls.push("loaded-chunk-unsaved".into());
            self.dirty += 1;
            self.revision += 1;
            Ok(())
        }
    }
    impl BeaconMenuHost for Host {
        type Key = u64;
        type Revision = u64;
        fn owner_key(&self) -> &u64 {
            &self.key
        }
        fn revision(&self) -> u64 {
            self.revision
        }
        fn ensure_supported_context(
            &self,
            _token: Option<BeaconMenuToken>,
            _action: BeaconAction,
        ) -> Result<()> {
            ensure!(self.supported, "actual beacon owner unavailable");
            Ok(())
        }
        fn stage(&self, _token: Option<BeaconMenuToken>, _action: BeaconAction) -> Result<Self> {
            Ok(self.clone())
        }
        fn publish_stage(&mut self, stage: Self, _token: Option<BeaconMenuToken>) -> Result<()> {
            ensure!(!self.fail_publish, "actual atomic publish unavailable");
            *self = stage;
            Ok(())
        }
        fn stage_event(&mut self, event: &BeaconMenuEvent, _drops: &[DroppedStack]) -> Result<()> {
            ensure!(!self.fail_event, "actual callback staging failed");
            ensure!(
                !(self.fail_after_primary
                    && matches!(
                        event,
                        BeaconMenuEvent::Source {
                            event: BeaconSourceEvent::AfterDataSet { index: 1, .. },
                            ..
                        }
                    )),
                "actual callback failed after primary setter"
            );
            self.events.push(event.clone());
            self.revision += 1;
            Ok(())
        }
        fn quickcraft_callbacks_are_order_independent(
            &self,
            _targets: &BTreeSet<usize>,
            _phase: &BeaconPhase,
        ) -> Result<bool> {
            Ok(self.drag)
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn ready() -> Result<(BeaconMenu<u64>, PlayerInventory, Host)> {
        let mut host = Host::new();
        let mut inv = PlayerInventory::default();
        inv.set_carried_authoritative(item("gold_ingot", 2));
        let mut menu = BeaconMenu::prepare_open(7, &inv, &host)?
            .commit(&inv, &mut host)?
            .unwrap();
        menu.prepare_initialize(&inv, &host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        Ok((menu, inv, host))
    }
    fn click(
        menu: &mut BeaconMenu<u64>,
        inv: &mut PlayerInventory,
        host: &mut Host,
        slot: i16,
        button: i8,
        kind: ClickType,
        creative: bool,
    ) -> Result<BeaconOutcome> {
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync().state_id(),
            slot,
            button,
            kind,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        menu.prepare_click(&packet, inv, host, creative)?
            .commit(menu, inv, host)
    }
    #[test]
    fn live_effect_updates_preserve_payment_sound_filter_dirty_and_deferred_sync() -> Result<()> {
        let (mut menu, mut inv, mut host) = ready()?;
        inv.set_slot_authoritative(9, item("emerald", 4));
        menu.prepare_broadcast(&inv, &host)?
            .commit(&mut menu, &mut inv, &mut host)?;
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
            0,
            0,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(menu.state().payment().count, 1);
        assert_eq!(menu.sync().carried().count, 3);
        assert_eq!(host.dirty, 0);
        let packet = BeaconUpdatePacket::decode(&[1, 4, 1, 0], &host)?;
        assert_eq!(packet.primary, Some(BeaconEffect { registry_id: 4 }));
        assert_eq!(packet.secondary, Some(BeaconEffect { registry_id: 0 }));
        let before = host.calls.len();
        let plan = menu.prepare_update(packet, &inv, &host)?;
        assert!(menu.state().has_payment());
        assert_eq!(host.data, [0; 3]);
        assert_eq!(plan.staged_host().data, [0, 5, 1]);
        let out = plan.commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(out.update, Some(BeaconUpdate::Updated));
        assert!(!menu.is_closed());
        assert!(!menu.state().has_payment());
        assert_eq!(
            &host.calls[before..],
            [
                "sound-before:0",
                "data:1:5",
                "data:2:1",
                "loaded-chunk-unsaved"
            ]
        );
        assert_eq!(host.dirty, 1);
        assert!(!out
            .events
            .iter()
            .any(|e| matches!(e, BeaconMenuEvent::Sync { .. })));
        let phases = out
            .events
            .iter()
            .filter_map(|e| {
                if let BeaconMenuEvent::Source { event, phase } = e {
                    Some((event, phase))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(phases.len(), 6);
        assert_eq!(phases[0].1.payment.count, 1);
        assert_eq!(phases[1].1.data, [0, 5, 0]);
        assert_eq!(phases[3].1.data, [0, 5, 1]);
        assert!(phases[4].1.payment.is_empty());
        assert!(phases[5].1.payment.is_empty());
        let sync = menu
            .prepare_broadcast(&inv, &host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert!(sync.events.iter().any(|e| matches!(
            e,
            BeaconMenuEvent::Sync {
                event: SyncEvent::DataListener { index: 1, value: 5 },
                ..
            }
        )));
        let out = menu
            .prepare_update(
                BeaconUpdatePacket {
                    primary: None,
                    secondary: None,
                },
                &inv,
                &host,
            )?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(out.update, Some(BeaconUpdate::IgnoredNoPayment));
        assert!(out.events.is_empty());
        click(
            &mut menu,
            &mut inv,
            &mut host,
            0,
            0,
            ClickType::Pickup,
            false,
        )?;
        let out = menu
            .prepare_update(
                BeaconUpdatePacket {
                    primary: None,
                    secondary: None,
                },
                &inv,
                &host,
            )?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(out.update, Some(BeaconUpdate::Updated));
        assert_eq!(host.data, [0, 0, 0]);
        assert_eq!(host.dirty, 2);
        let out = menu
            .prepare_set_data(0, 3, &inv, &host)?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(host.data[0], 3);
        assert!(out.events.iter().any(|e| matches!(
            e,
            BeaconMenuEvent::Sync {
                event: SyncEvent::DataListener { index: 0, value: 3 },
                ..
            }
        )));
        assert_eq!(host.dirty, 2);
        assert_eq!(inv.carried().count, 2);
        Ok(())
    }
    #[test]
    fn payment_shift_count1_all_clicks_and_cursor_then_always_drop_close() -> Result<()> {
        let (mut menu, mut inv, mut host) = ready()?;
        inv.set_slot_authoritative(9, item("emerald", 4));
        click(
            &mut menu,
            &mut inv,
            &mut host,
            1,
            0,
            ClickType::QuickMove,
            false,
        )?;
        assert!(!menu.state().has_payment());
        assert_eq!(inv.slot(36).unwrap().count, 4);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            28,
            0,
            ClickType::Pickup,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            0,
            ClickType::QuickCraft,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            0,
            1,
            ClickType::QuickCraft,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            1,
            ClickType::QuickCraft,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            2,
            ClickType::QuickCraft,
            false,
        )?;
        assert_eq!(menu.state().payment().count, 1);
        assert_eq!(inv.slot(10).unwrap().count, 2);
        assert_eq!(menu.sync().carried().count, 1);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            1,
            0,
            ClickType::PickupAll,
            false,
        )?;
        assert_eq!(menu.sync().carried().count, 4);
        assert!(!menu.state().has_payment());
        assert!(inv.slot(10).unwrap().is_empty());
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            0,
            ClickType::Pickup,
            false,
        )?;
        inv.set_slot_authoritative(9, item("emerald", 1));
        click(
            &mut menu,
            &mut inv,
            &mut host,
            1,
            0,
            ClickType::QuickMove,
            false,
        )?;
        assert!(menu.state().has_payment());
        click(
            &mut menu,
            &mut inv,
            &mut host,
            0,
            40,
            ClickType::Swap,
            false,
        )?;
        assert_eq!(inv.slot(45).unwrap().count, 1);
        assert!(!menu.state().has_payment());
        click(
            &mut menu,
            &mut inv,
            &mut host,
            0,
            40,
            ClickType::Swap,
            false,
        )?;
        let thrown = click(
            &mut menu,
            &mut inv,
            &mut host,
            0,
            0,
            ClickType::Throw,
            false,
        )?;
        assert_eq!(thrown.drops[0].stack.count, 1);
        assert!(thrown.drops[0].trace_item);
        inv.set_slot_authoritative(9, item("emerald", 1));
        click(&mut menu, &mut inv, &mut host, 1, 2, ClickType::Clone, true)?;
        assert_eq!(menu.sync().carried().count, 64);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            0,
            ClickType::Pickup,
            true,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            1,
            0,
            ClickType::QuickMove,
            false,
        )?;
        inv.set_slot_authoritative(9, item("stone", 3));
        click(
            &mut menu,
            &mut inv,
            &mut host,
            1,
            0,
            ClickType::Pickup,
            false,
        )?;
        let out = menu
            .prepare_close(&inv, &host, MenuCloseContext::AliveConnected)?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert!(menu.is_closed());
        assert_eq!(out.drops.len(), 1);
        assert_eq!(out.drops[0].stack, item("emerald", 1));
        assert!(!out.drops[0].trace_item);
        assert_eq!(inv.slot(36).unwrap(), &item("stone", 3));
        let drop_phase = out
            .events
            .iter()
            .find_map(|e| {
                if let BeaconMenuEvent::Drop { phase, .. } = e {
                    Some(phase)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(drop_phase.payment.is_empty() && drop_phase.carried.is_empty());
        assert_eq!(drop_phase.player_inventory.slot(36).unwrap().count, 3);
        assert_eq!(host.dirty, 0);
        Ok(())
    }
    #[test]
    fn guards_lock_denial_food_failures_live_data_and_reopened_ui() -> Result<()> {
        let mut host = Host::new();
        host.unlock = false;
        let inv = PlayerInventory::default();
        let denied = BeaconMenu::prepare_open(7, &inv, &host)?;
        assert!(!denied.allowed());
        assert!(host.calls.is_empty());
        assert!(denied.commit(&inv, &mut host)?.is_none());
        assert_eq!(host.calls, vec!["actual-lock-denial"]);
        let (mut menu, mut inv, mut host) = ready()?;
        let old = menu.prepare_full_state(&inv, &host)?;
        menu.prepare_close(&inv, &host, MenuCloseContext::DeadOrDisconnected)?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert!(old.validate(&menu, &inv, &host).is_err());
        assert!(menu
            .prepare_close(&inv, &host, MenuCloseContext::AliveConnected)
            .is_err());
        let mut reopened = BeaconMenu::prepare_open(7, &inv, &host)?
            .commit(&inv, &mut host)?
            .unwrap();
        reopened
            .prepare_initialize(&inv, &host)?
            .commit(&mut reopened, &mut inv, &mut host)?;
        let mut same = old.before.clone();
        same.instance = reopened.instance;
        assert_eq!(same, reopened);
        assert!(old.validate(&reopened, &inv, &host).is_err());
        let plan = reopened.prepare_full_state(&inv, &host)?;
        host.data[0] = 4;
        assert!(plan.validate(&reopened, &inv, &host).is_err());
        host.data[0] = 0;
        host.fail_event = true;
        let before = reopened.clone();
        assert!(reopened.prepare_full_state(&inv, &host).is_err());
        assert_eq!(reopened, before);
        host.fail_event = false;
        let plan = reopened.prepare_full_state(&inv, &host)?;
        host.fail_publish = true;
        assert!(plan.commit(&mut reopened, &mut inv, &mut host).is_err());
        assert_eq!(reopened, before);
        host.fail_publish = false;
        inv.set_slot_authoritative(9, item("bread", 2));
        reopened
            .prepare_broadcast(&inv, &host)?
            .commit(&mut reopened, &mut inv, &mut host)?;
        let old = reopened.prepare_full_state(&inv, &host)?;
        let mut food = item("bread", 2);
        food.components.push(crate::inventory::Component {
            type_id: 20,
            data: food.effective_component_bytes(20).unwrap().to_vec(),
        });
        inv.set_slot_authoritative(9, food);
        assert!(
            format!("{:#}", old.validate(&reopened, &inv, &host).unwrap_err())
                .contains("identity unresolved")
        );
        assert!(BeaconUpdatePacket::decode(&[1, 0, 0], &host)
            .unwrap()
            .primary
            .is_some());
        assert!(BeaconUpdatePacket::decode(&[1, 99, 0], &host).is_err());
        assert!(BeaconUpdatePacket::decode(&[0, 0, 1], &host).is_err());
        Ok(())
    }
    #[test]
    fn primary_prefix_failure_is_private_and_invalid_menu_filter_and_counters_are_exact(
    ) -> Result<()> {
        let (mut menu, mut inv, mut host) = ready()?;
        inv.set_slot_authoritative(9, item("emerald", 4));
        let pickup = click(
            &mut menu,
            &mut inv,
            &mut host,
            1,
            0,
            ClickType::Pickup,
            false,
        )?;
        let cursors = pickup
            .events
            .iter()
            .filter_map(|e| {
                if let BeaconMenuEvent::PlayerInventoryChanged { phase } = e {
                    Some(phase.carried.count)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(cursors, vec![0, 4, 4]);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            0,
            0,
            ClickType::Pickup,
            false,
        )?;
        let full = click(
            &mut menu,
            &mut inv,
            &mut host,
            0,
            0,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(
            full.events
                .iter()
                .filter(|e| matches!(
                    e,
                    BeaconMenuEvent::Source {
                        event: BeaconSourceEvent::PaymentContainerChanged,
                        ..
                    }
                ))
                .count(),
            3
        );
        assert_eq!(menu.sync().carried().count, 3);
        assert_eq!(host.dirty, 0);
        let before = menu.clone();
        let calls = host.calls.clone();
        let data = host.data;
        host.fail_after_primary = true;
        assert!(menu
            .prepare_update(
                BeaconUpdatePacket {
                    primary: Some(BeaconEffect { registry_id: 0 }),
                    secondary: None
                },
                &inv,
                &host
            )
            .is_err());
        assert_eq!(menu, before);
        assert_eq!(host.calls, calls);
        assert_eq!(host.data, data);
        assert_eq!(host.dirty, 0);
        host.fail_after_primary = false;
        host.valid = false;
        let ignored = menu
            .prepare_update(
                BeaconUpdatePacket {
                    primary: None,
                    secondary: None,
                },
                &inv,
                &host,
            )?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(ignored.update, Some(BeaconUpdate::IgnoredInvalidMenu));
        assert!(ignored.events.is_empty());
        assert!(menu.state().has_payment());
        host.valid = true;
        host.beam = false;
        let selected = menu
            .prepare_update(
                BeaconUpdatePacket {
                    primary: Some(BeaconEffect { registry_id: 9 }),
                    secondary: None,
                },
                &inv,
                &host,
            )?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(selected.update, Some(BeaconUpdate::Updated));
        assert_eq!(host.data, [0, 0, 0]);
        assert_eq!(host.dirty, 1);
        assert!(!host.calls.iter().any(|c| c.starts_with("sound-before:")));
        assert!(BeaconUpdatePacket::decode(&[0, 0], &host)?
            .primary
            .is_none());
        Ok(())
    }
}
