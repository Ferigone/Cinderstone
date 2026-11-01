//! Java 1.21 CartographyTableMenu over the actual captured saved-map owner.
//! Preparation stages that owner's real transaction. Its callbacks must not
//! reenter this menu; callback observations never confer additional item ownership.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    workstations::{
        cartography::{CartographyMaps, CartographyState, RESULT_SLOT},
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

/// Implement with the captured actual level/saved-map storage and ID owner.
/// stage MUST detach all mutable real state. commit_staged must atomically
/// publish that stage or leave the live owner unchanged on error. A clone of a
/// shared mutable handle is not a detached transaction. No default host exists.
pub trait CartographyMenuHost: CartographyMaps + Sized {
    type Key: Clone + Eq + Debug;
    type Revision: Clone + Eq + Debug;
    fn owner_key(&self) -> &Self::Key;
    fn revision(&self) -> Self::Revision;
    fn stage(&self) -> Result<Self>;
    fn commit_staged(&mut self, staged: Self) -> Result<()>;
    /// Certify actual vanilla map callbacks, readonly/nonreentrant observers and
    /// transaction support; reject unknown/effectful unsupported contexts.
    fn ensure_supported_context(&self) -> Result<()>;
    fn still_valid_for_actual_player(&self) -> Result<bool>;
    fn game_time(&self) -> Result<i64>;
    /// Certify ALL resulting native observations for this target set/phase:
    /// input-triggered broadcasts, slot/data listeners, player counter calls,
    /// cursor and remote synchronization, as well as actual host callbacks.
    /// Default/no-op custom listeners alone do not prove this. Return false
    /// when any source observation depends on Java's Slot HashSet encounter order.
    fn quickcraft_callbacks_are_order_independent(
        &self,
        targets: &BTreeSet<usize>,
        phase: &CartographyPhase,
    ) -> Result<bool>;
    /// Actual Item.onCraftedBy, including MapItem postprocess on this mutable
    /// argument and real SavedData/MapIndex dirty writes. It may receive EMPTY.
    /// phase is the source state before callback; it is not another owner.
    fn on_crafted_by(&mut self, stack: &mut ItemStack, phase: &CartographyPhase) -> Result<()>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CartographyMenu<K: Clone + Eq + Debug> {
    owner_key: K,
    station: CartographyState,
    sync: MenuSync,
    instance: u64,
    revision: u64,
    closed: bool,
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
}
#[derive(Clone, Debug)]
pub struct CartographyPhase {
    pub inputs: [ItemStack; 2],
    pub result_preview: ItemStack,
    pub player_inventory: PlayerInventory,
    pub carried: ItemStack,
}
#[derive(Clone, Debug)]
pub enum CartographyMenuEvent {
    Sync {
        event: SyncEvent,
        phase: CartographyPhase,
    },
    InputContainerChanged {
        phase: CartographyPhase,
    },
    PlayerInventoryChanged {
        phase: CartographyPhase,
    },
    /// Observation ONLY: this callback has already run against the staged real
    /// map owner. Do not execute it again while publishing these events.
    ItemCraftedBy {
        before: ItemStack,
        after: ItemStack,
        phase: CartographyPhase,
        after_phase: CartographyPhase,
    },
    /// Actual captured-position UI_CARTOGRAPHY_TABLE_TAKE_RESULT, BLOCKS, 1/1.
    TakeSound {
        phase: CartographyPhase,
    },
    Drop {
        index: usize,
        phase: CartographyPhase,
    },
}
impl CartographyPhase {
    /// Observe a raw per-insertion packet without sending it. Returning is an
    /// observation of either the carried object or a removed grid stack.
    fn observe_return_packet(
        &mut self,
        packet: &crate::protocol::Packet,
        returning: &mut ItemStack,
    ) -> Result<()> {
        ensure!(
            packet.id == 0x15,
            "unsupported cartography close packet phase"
        );
        let mut reader = crate::protocol::Reader::new(&packet.payload);
        ensure!(
            reader.i8()? == -2 && reader.varint()? == 0,
            "cartography close requires raw inventory packet"
        );
        let raw = reader.i16()?;
        let slot = match raw {
            0..=8 => 36 + raw as usize,
            9..=35 => raw as usize,
            40 => 45,
            _ => bail!("unsupported cartography close raw slot"),
        };
        let stack = ItemStack::decode_exact(reader.remaining())?;
        let old = self.player_inventory.slot(slot).unwrap();
        ensure!(
            stack.same_item_same_components(returning)?,
            "cartography close changed return identity"
        );
        let old_count = if old.is_empty() {
            0
        } else {
            ensure!(
                old.same_item_same_components(&stack)?,
                "cartography close overwrote owned stack"
            );
            old.count
        };
        let added = stack.count - old_count;
        ensure!(
            added > 0 && added <= returning.count,
            "invalid cartography close insertion amount"
        );
        self.player_inventory.set_slot_authoritative(slot, stack);
        *returning = counted(returning, returning.count - added);
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct CartographyOutcome {
    pub events: Vec<CartographyMenuEvent>,
    pub drops: Vec<DroppedStack>,
}
pub struct CartographyPlan<H: CartographyMenuHost> {
    before: CartographyMenu<H::Key>,
    before_inventory: PlayerInventory,
    before_host_revision: H::Revision,
    game_time: i64,
    require_order_independence: bool,
    require_validity: bool,
    next: CartographyMenu<H::Key>,
    next_inventory: PlayerInventory,
    staged_host: H,
    outcome: CartographyOutcome,
}
impl<H: CartographyMenuHost> CartographyPlan<H> {
    pub fn outcome(&self) -> &CartographyOutcome {
        &self.outcome
    }
    pub fn next_menu(&self) -> &CartographyMenu<H::Key> {
        &self.next
    }
    pub fn next_inventory(&self) -> &PlayerInventory {
        &self.next_inventory
    }
    pub fn staged_host(&self) -> &H {
        &self.staged_host
    }
    pub fn validate(
        &self,
        menu: &CartographyMenu<H::Key>,
        inventory: &PlayerInventory,
        host: &H,
    ) -> Result<()> {
        menu.check_open()?;
        menu.check_item_identity(inventory)?;
        ensure!(*menu == self.before, "stale cartography UI");
        ensure!(
            same_inventory(inventory, &self.before_inventory),
            "cartography player inventory changed"
        );
        ensure!(
            *host.owner_key() == self.before.owner_key
                && host.revision() == self.before_host_revision,
            "cartography saved-map owner changed"
        );
        ensure!(
            host.game_time()? == self.game_time,
            "cartography source game time changed"
        );
        host.ensure_supported_context()?;
        if self.require_validity {
            ensure!(
                host.still_valid_for_actual_player()?,
                "cartography table/player context became invalid"
            );
        }
        if self.require_order_independence {
            ensure!(
                host.quickcraft_callbacks_are_order_independent(
                    &self.before.quickcraft_slots,
                    &self.before.phase(inventory)
                )?,
                "cartography drag callback context changed"
            );
        }
        Ok(())
    }
    /// Validate before staging external counter/sound/drop/listener operations,
    /// then again here. Host commit is atomic; native assignments cannot fail.
    pub fn commit(
        self,
        menu: &mut CartographyMenu<H::Key>,
        inventory: &mut PlayerInventory,
        host: &mut H,
    ) -> Result<CartographyOutcome> {
        self.validate(menu, inventory, host)?;
        host.commit_staged(self.staged_host)?;
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
    ensure!(
        (1..=99).contains(&max),
        "invalid cartography menu stack limit"
    );
    Ok(max)
}

impl<K: Clone + Eq + Debug> CartographyMenu<K> {
    pub fn new(window_id: u8, host: &impl CartographyMenuHost<Key = K>) -> Result<Self> {
        static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
        host.ensure_supported_context()?;
        let instance = NEXT_INSTANCE
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| anyhow::anyhow!("cartography UI identity exhausted"))?;
        Ok(Self {
            owner_key: host.owner_key().clone(),
            station: CartographyState::default(),
            sync: MenuSync::new(window_id, WorkstationKind::Cartography)?,
            instance,
            revision: 0,
            closed: false,
            quickcraft_status: 0,
            quickcraft_type: 0,
            quickcraft_slots: BTreeSet::new(),
        })
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    fn check_open(&self) -> Result<()> {
        ensure!(!self.closed, "cartography UI is closed");
        Ok(())
    }
    fn check_item_identity(&self, inventory: &PlayerInventory) -> Result<()> {
        for stack in inventory
            .slots()
            .iter()
            .chain(std::iter::once(inventory.carried()))
            .chain(self.station.inputs().iter())
            .chain(std::iter::once(self.station.result()))
            .chain(std::iter::once(self.sync.carried()))
        {
            reject_unresolved_food_identity(stack)?;
        }
        Ok(())
    }
    pub fn station(&self) -> &CartographyState {
        &self.station
    }
    pub fn sync(&self) -> &MenuSync {
        &self.sync
    }
    pub fn inventory_snapshot(&self, inventory: &PlayerInventory) -> PlayerInventory {
        let mut snapshot = inventory.clone();
        snapshot.set_carried_authoritative(self.sync.carried().clone());
        snapshot
    }
    fn top(&self) -> [ItemStack; 3] {
        [
            self.station.inputs()[0].clone(),
            self.station.inputs()[1].clone(),
            self.station.result().clone(),
        ]
    }
    fn advance_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("cartography UI revision exhausted"))?;
        Ok(())
    }
    fn phase(&self, inventory: &PlayerInventory) -> CartographyPhase {
        CartographyPhase {
            inputs: self.station.inputs().clone(),
            result_preview: self.station.result().clone(),
            player_inventory: inventory.clone(),
            carried: self.sync.carried().clone(),
        }
    }
    fn player_changed(&self, inventory: &PlayerInventory, outcome: &mut CartographyOutcome) {
        outcome
            .events
            .push(CartographyMenuEvent::PlayerInventoryChanged {
                phase: self.phase(inventory),
            });
    }
    fn player_changed_with_source(
        &self,
        inventory: &PlayerInventory,
        outcome: &mut CartographyOutcome,
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
            .push(CartographyMenuEvent::PlayerInventoryChanged { phase });
    }
    pub fn initialize(&mut self, inventory: &PlayerInventory) -> Result<Vec<CartographyMenuEvent>> {
        self.check_open()?;
        self.check_item_identity(inventory)?;
        let mut next = self.clone();
        let phase = next.phase(inventory);
        let events = next
            .sync
            .initialize(&next.top(), inventory, &[])?
            .into_iter()
            .map(|event| CartographyMenuEvent::Sync {
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
    ) -> Result<Vec<CartographyMenuEvent>> {
        self.check_open()?;
        self.check_item_identity(inventory)?;
        let mut next = self.clone();
        let phase = next.phase(inventory);
        let events = next
            .sync
            .broadcast_changes(&next.top(), inventory, &[])?
            .into_iter()
            .map(|event| CartographyMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect();
        next.advance_revision()?;
        *self = next;
        Ok(events)
    }
    pub fn still_valid(&self, host: &impl CartographyMenuHost<Key = K>) -> Result<bool> {
        self.check_open()?;
        ensure!(
            *host.owner_key() == self.owner_key,
            "cartography captured level changed"
        );
        host.still_valid_for_actual_player()
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    pub fn prepare_click<H: CartographyMenuHost<Key = K>>(
        &self,
        click: &ContainerClick,
        inventory: &PlayerInventory,
        host: &H,
        creative: bool,
    ) -> Result<CartographyPlan<H>> {
        self.check_open()?;
        self.check_item_identity(inventory)?;
        for stack in click
            .changed_slots
            .values()
            .chain(std::iter::once(&click.carried))
        {
            reject_unresolved_food_identity(stack)?;
        }
        ensure!(
            click.window_id == self.sync.window_id(),
            "wrong cartography window"
        );
        ensure!(click.slot < 39, "cartography slot outside menu");
        host.ensure_supported_context()?;
        ensure!(
            *host.owner_key() == self.owner_key,
            "cartography actual owner changed"
        );
        ensure!(
            host.still_valid_for_actual_player()?,
            "cartography table/player context is invalid"
        );
        let before_host_revision = host.revision();
        let game_time = host.game_time()?;
        let mut staged_host = host.stage()?;
        ensure!(
            *staged_host.owner_key() == self.owner_key
                && staged_host.revision() == before_host_revision
                && staged_host.game_time()? == game_time,
            "cartography stage changed base owner/context"
        );
        staged_host.ensure_supported_context()?;
        ensure!(
            staged_host.still_valid_for_actual_player()?,
            "cartography detached stage changed validity"
        );
        let mut next = self.clone();
        let mut inv = inventory.clone();
        let mut outcome = CartographyOutcome::default();
        let require_order_independence = click.kind == ClickType::QuickCraft
            && (click.button & 3) == 2
            && next.quickcraft_status == 1
            && !next.sync.carried().is_empty()
            && next.quickcraft_slots.len() > 1;
        let state_before = next.sync.state_id();
        next.sync.suppress_remote_updates(true);
        Working {
            menu: &mut next,
            host: &mut staged_host,
        }
        .do_click(
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
                .map(|event| CartographyMenuEvent::Sync {
                    event,
                    phase: phase.clone(),
                }),
        );
        next.advance_revision()?;
        ensure!(
            *staged_host.owner_key() == self.owner_key && staged_host.game_time()? == game_time,
            "cartography callback changed captured owner/time"
        );
        staged_host.ensure_supported_context()?;
        Ok(CartographyPlan {
            before: self.clone(),
            before_inventory: inventory.clone(),
            before_host_revision,
            game_time,
            require_order_independence,
            require_validity: true,
            next,
            next_inventory: inv,
            staged_host,
            outcome,
        })
    }
    pub fn prepare_close<H: CartographyMenuHost<Key = K>>(
        &self,
        inventory: &PlayerInventory,
        host: &H,
        context: MenuCloseContext,
    ) -> Result<CartographyPlan<H>> {
        self.check_open()?;
        self.check_item_identity(inventory)?;
        host.ensure_supported_context()?;
        ensure!(
            *host.owner_key() == self.owner_key,
            "cartography actual owner changed"
        );
        let before_host_revision = host.revision();
        let game_time = host.game_time()?;
        let staged_host = host.stage()?;
        ensure!(
            *staged_host.owner_key() == self.owner_key
                && staged_host.revision() == before_host_revision
                && staged_host.game_time()? == game_time,
            "cartography close stage changed base owner/context"
        );
        let mut next = self.clone();
        let mut inv = inventory.clone();
        next.quickcraft_status = 0;
        next.quickcraft_slots.clear();
        let mut outcome = CartographyOutcome::default();
        let mut phase = self.phase(inventory);
        let mut returning = phase.carried.clone();
        let removal = next.sync.close_inputs_ordered(&mut [], &mut inv, context)?;
        Self::append_close(&mut outcome, removal, &mut phase, &mut returning, true)?;
        // Cartography.removed explicitly removes the preview AFTER cursor removal.
        next.station.remove_result_raw();
        phase.result_preview = ItemStack::empty();
        let mut inputs = next.station.drain_inputs();
        for (slot, input) in inputs.iter_mut().enumerate() {
            phase.inputs[slot] = ItemStack::empty();
            let mut returning = input.clone();
            let removal =
                next.sync
                    .close_inputs_ordered(std::slice::from_mut(input), &mut inv, context)?;
            Self::append_close(&mut outcome, removal, &mut phase, &mut returning, false)?;
        }
        next.closed = true;
        next.advance_revision()?;
        ensure!(
            *staged_host.owner_key() == self.owner_key && staged_host.game_time()? == game_time,
            "cartography callback changed captured owner/time"
        );
        staged_host.ensure_supported_context()?;
        Ok(CartographyPlan {
            before: self.clone(),
            before_inventory: inventory.clone(),
            before_host_revision,
            game_time,
            require_order_independence: false,
            require_validity: false,
            next,
            next_inventory: inv,
            staged_host,
            outcome,
        })
    }
    fn append_close(
        outcome: &mut CartographyOutcome,
        removal: crate::workstations::menu::MenuRemoval,
        phase: &mut CartographyPhase,
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
                    outcome.events.push(CartographyMenuEvent::Sync {
                        event: SyncEvent::Packet(packet),
                        phase: phase.clone(),
                    });
                }
                MenuRemovalEvent::Drop { index } => {
                    if is_cursor {
                        phase.carried = outcome.drops[drop_base + index].stack.clone();
                    }
                    outcome.events.push(CartographyMenuEvent::Drop {
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
}

struct Working<'a, K: Clone + Eq + Debug, H: CartographyMenuHost<Key = K>> {
    menu: &'a mut CartographyMenu<K>,
    host: &'a mut H,
}
impl<K: Clone + Eq + Debug, H: CartographyMenuHost<Key = K>> Deref for Working<'_, K, H> {
    type Target = CartographyMenu<K>;
    fn deref(&self) -> &Self::Target {
        self.menu
    }
}
impl<K: Clone + Eq + Debug, H: CartographyMenuHost<Key = K>> DerefMut for Working<'_, K, H> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.menu
    }
}
#[derive(Clone, Copy)]
enum TakenAlias {
    Detached,
    Cursor,
    Player(usize),
    Result,
}
impl<K: Clone + Eq + Debug, H: CartographyMenuHost<Key = K>> Working<'_, K, H> {
    fn slot(&self, slot: usize, inventory: &PlayerInventory) -> ItemStack {
        if slot < RESULT_SLOT {
            self.station.inputs()[slot].clone()
        } else if slot == RESULT_SLOT {
            self.station.result().clone()
        } else {
            inventory
                .slot(slot + 6)
                .expect("actual player inventory")
                .clone()
        }
    }
    fn put_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        inventory: &mut PlayerInventory,
        outcome: &mut CartographyOutcome,
    ) -> Result<()> {
        self.change_slot(slot, stack, inventory, outcome)?;
        // Slot.set -> container.setItem, then Slot.setChanged. Inputs invoke
        // the menu twice; ResultContainer.setItem itself does not setChanged.
        self.slot_changed(slot, inventory, outcome)
    }
    fn change_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        inventory: &mut PlayerInventory,
        outcome: &mut CartographyOutcome,
    ) -> Result<()> {
        if slot < RESULT_SLOT {
            self.station.set_input_raw(slot, stack)?;
            self.slot_changed(slot, inventory, outcome)?;
        } else if slot == RESULT_SLOT {
            self.station.set_result_raw(stack)?;
        } else {
            inventory.set_slot_authoritative(slot + 6, stack);
        }
        Ok(())
    }
    fn raw_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        inventory: &mut PlayerInventory,
    ) -> Result<()> {
        if slot < RESULT_SLOT {
            self.station.set_input_raw(slot, stack)?;
        } else if slot == RESULT_SLOT {
            self.station.set_result_raw(stack)?;
        } else {
            inventory.set_slot_authoritative(slot + 6, stack);
        }
        Ok(())
    }
    fn slot_changed(
        &mut self,
        slot: usize,
        inventory: &PlayerInventory,
        outcome: &mut CartographyOutcome,
    ) -> Result<()> {
        if slot <= RESULT_SLOT {
            let observations = self.menu.station.slots_changed_observed(self.host)?;
            for observation in observations {
                let top = [
                    observation.inputs[0].clone(),
                    observation.inputs[1].clone(),
                    observation.result.clone(),
                ];
                let mut phase = self.phase(inventory);
                phase.inputs = observation.inputs;
                phase.result_preview = observation.result;
                outcome.events.extend(
                    self.menu
                        .sync
                        .broadcast_changes(&top, inventory, &[])?
                        .into_iter()
                        .map(|event| CartographyMenuEvent::Sync {
                            event,
                            phase: phase.clone(),
                        }),
                );
            }
            if slot < RESULT_SLOT {
                outcome
                    .events
                    .push(CartographyMenuEvent::InputContainerChanged {
                        phase: self.phase(inventory),
                    });
            }
        } else {
            self.player_changed(inventory, outcome);
        }
        Ok(())
    }
    fn alias_after(
        &mut self,
        alias: TakenAlias,
        stack: &ItemStack,
        inventory: &mut PlayerInventory,
    ) -> Result<()> {
        match alias {
            TakenAlias::Detached => {}
            TakenAlias::Cursor => self.sync.set_carried(stack.clone()),
            TakenAlias::Player(slot) => inventory.set_slot_authoritative(slot, stack.clone()),
            TakenAlias::Result => {
                self.station.set_result_raw(stack.clone())?;
            }
        }
        Ok(())
    }
    fn crafted_by(
        &mut self,
        stack: &mut ItemStack,
        alias: TakenAlias,
        inventory: &mut PlayerInventory,
        outcome: &mut CartographyOutcome,
    ) -> Result<()> {
        let phase = self.phase(inventory);
        let before = stack.clone();
        self.host.on_crafted_by(stack, &phase)?;
        reject_unresolved_food_identity(stack)?;
        ensure!(
            stack.count == before.count && stack.item_id == before.item_id,
            "unsupported cartography callback item/count replacement"
        );
        stack.encode_canonical()?;
        self.alias_after(alias, stack, inventory)?;
        outcome.events.push(CartographyMenuEvent::ItemCraftedBy {
            before,
            after: stack.clone(),
            phase,
            after_phase: self.phase(inventory),
        });
        Ok(())
    }
    fn on_take(
        &mut self,
        slot: usize,
        taken: &mut ItemStack,
        alias: TakenAlias,
        inventory: &mut PlayerInventory,
        outcome: &mut CartographyOutcome,
    ) -> Result<()> {
        if slot == RESULT_SLOT {
            for input in 0..2 {
                let source = self.station.inputs()[input].clone();
                if !source.is_empty() {
                    self.change_slot(
                        input,
                        counted(&source, source.count - 1),
                        inventory,
                        outcome,
                    )?;
                }
            }
            self.crafted_by(taken, alias, inventory, outcome)?;
            let time = self.host.game_time()?;
            if self.station.take_sound(time) {
                outcome.events.push(CartographyMenuEvent::TakeSound {
                    phase: self.phase(inventory),
                });
            }
        }
        self.slot_changed(slot, inventory, outcome)
    }
    fn may_place(&self, slot: usize, stack: &ItemStack) -> Result<bool> {
        Ok(if slot == RESULT_SLOT {
            false
        } else if slot < RESULT_SLOT {
            CartographyState::may_place(slot, stack)
        } else {
            true
        })
    }
    fn reset_quickcraft(&mut self) {
        self.quickcraft_status = 0;
        self.quickcraft_slots.clear();
    }
    fn drop_stack(
        &self,
        outcome: &mut CartographyOutcome,
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
            outcome.events.push(CartographyMenuEvent::Drop {
                index,
                phase: self.phase(inventory),
            });
        }
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
        outcome: &mut CartographyOutcome,
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
        outcome: &mut CartographyOutcome,
    ) -> Result<ItemStack> {
        let source = self.slot(slot, inventory);
        if source.is_empty() || amount <= 0 || maximum <= 0 {
            return Ok(ItemStack::empty());
        }
        if slot == RESULT_SLOT && maximum < source.count {
            return Ok(ItemStack::empty());
        }
        let mut taken = if slot == RESULT_SLOT {
            self.station.remove_result_raw()
        } else {
            let count = source.count.min(amount).min(maximum);
            self.change_slot(
                slot,
                counted(&source, source.count - count),
                inventory,
                outcome,
            )?;
            counted(&source, count)
        };
        if self.slot(slot, inventory).is_empty() {
            self.put_slot(slot, ItemStack::empty(), inventory, outcome)?;
        }
        let alias = if let Some(cursor) = cursor_before_on_take {
            self.sync.set_carried(if slot == RESULT_SLOT {
                taken.clone()
            } else {
                cursor
            });
            TakenAlias::Cursor
        } else {
            TakenAlias::Detached
        };
        self.on_take(slot, &mut taken, alias, inventory, outcome)?;
        Ok(taken)
    }
    fn do_click(
        &mut self,
        slot: i32,
        button: i32,
        kind: ClickType,
        creative: bool,
        inventory: &mut PlayerInventory,
        outcome: &mut CartographyOutcome,
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
        outcome: &mut CartographyOutcome,
    ) -> Result<()> {
        let source = self.slot(slot, inventory);
        let carried = self.sync.carried().clone();
        reject_bundle(&source)?;
        reject_bundle(&carried)?;
        if slot == RESULT_SLOT {
            if !source.is_empty() {
                if carried.is_empty() {
                    self.take_slot_with_cursor(
                        slot,
                        source.count,
                        i32::MAX,
                        Some(source.clone()),
                        inventory,
                        outcome,
                    )?;
                } else if source.same_item_same_components(&carried)?
                    && source.count + carried.count <= carried.max_stack_size()?
                {
                    let mut taken = self.station.remove_result_raw();
                    self.put_slot(slot, ItemStack::empty(), inventory, outcome)?;
                    // cpu lambda$doClick$4 grows the existing cursor before
                    // onTake. Its mutable taken argument is DETACHED from it.
                    self.sync
                        .set_carried(counted(&carried, carried.count + taken.count));
                    self.on_take(slot, &mut taken, TakenAlias::Detached, inventory, outcome)?;
                }
            }
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
        // rejected/no-op insertion. The result container recomputes its preview.
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
        outcome: &mut CartographyOutcome,
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
                        self.raw_slot(source_slot, stack.clone(), inventory)?;
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
                    self.raw_slot(source_slot, stack.clone(), inventory)?;
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
        outcome: &mut CartographyOutcome,
    ) -> Result<()> {
        loop {
            let original = self.slot(slot, inventory);
            reject_bundle(&original)?;
            if original.is_empty() {
                break;
            }
            let mut moving = original.clone();
            if slot == RESULT_SLOT {
                // This committed source callback prefix precedes capacity;
                // failure still publishes its actual saved-map/ID mutation.
                self.crafted_by(&mut moving, TakenAlias::Result, inventory, outcome)?;
            }
            let Some((range, reverse)) = CartographyState::quick_move_target(slot, &moving) else {
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
            }
            self.slot_changed(slot, inventory, outcome)?;
            if moving.count == original.count {
                break;
            }
            self.on_take(slot, &mut moving, TakenAlias::Detached, inventory, outcome)?;
            let phase = self.phase(inventory);
            let top = self.top();
            outcome.events.extend(
                self.menu
                    .sync
                    .broadcast_changes(&top, inventory, &[])?
                    .into_iter()
                    .map(|event| CartographyMenuEvent::Sync {
                        event,
                        phase: phase.clone(),
                    }),
            );
            // AbstractContainerMenu repeats by Item identity, not components.
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
        outcome: &mut CartographyOutcome,
    ) -> Result<()> {
        let player_slot = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        if slot > RESULT_SLOT && slot + 6 == player_slot {
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
                    self.put_slot(slot, ItemStack::empty(), inventory, outcome)?;
                    let mut taken = source;
                    self.on_take(
                        slot,
                        &mut taken,
                        TakenAlias::Player(player_slot),
                        inventory,
                        outcome,
                    )?;
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
        outcome: &mut CartographyOutcome,
    ) -> Result<()> {
        let carried = self.sync.carried().clone();
        reject_bundle(&carried)?;
        if carried.is_empty()
            || !self.slot(clicked, inventory).is_empty() && self.may_pickup(clicked)?
        {
            return Ok(());
        }
        let order = if button == 0 {
            (0..39).collect::<Vec<_>>()
        } else {
            (0..39).rev().collect()
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
        outcome: &mut CartographyOutcome,
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
                    ensure!(
                        self.host.quickcraft_callbacks_are_order_independent(
                            &self.quickcraft_slots,
                            &self.phase(inventory)
                        )?,
                        "unsupported order-sensitive cartography drag callbacks"
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

/// Decoded FoodProperties overrides retain nested ItemStack reference identity
/// in Java. Wire/semantic equality cannot recover it. Stable item prototypes
/// have their actual shared default value and remain admitted.
fn reject_unresolved_food_identity(stack: &ItemStack) -> Result<()> {
    ensure!(
        !stack
            .components
            .iter()
            .any(|component| component.type_id == 20),
        "cartography food component nested-stack Java identity unresolved"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crafting::RecipeBook,
        inventory::Component,
        protocol::Writer,
        workstations::special_context::{
            postprocess_crafted_map, CraftedMapOwner, MapPostProcessing,
        },
    };
    use std::collections::BTreeMap;
    #[derive(Clone)]
    struct Host {
        key: u64,
        revision: u64,
        time: i64,
        maps: BTreeMap<i32, crate::workstations::cartography::CartographyMapData>,
        next_id: i32,
        dirty: Vec<i32>,
        callbacks: Vec<(ItemStack, CartographyPhase)>,
        supported: bool,
        drag: bool,
    }
    impl Host {
        fn new() -> Self {
            Self {
                key: 41,
                revision: 0,
                time: 1,
                maps: BTreeMap::from([(
                    5,
                    crate::workstations::cartography::CartographyMapData {
                        scale: 1,
                        locked: false,
                    },
                )]),
                next_id: 6,
                dirty: vec![],
                callbacks: vec![],
                supported: true,
                drag: true,
            }
        }
    }
    impl CartographyMaps for Host {
        fn map_data(
            &self,
            id: i32,
        ) -> Result<Option<crate::workstations::cartography::CartographyMapData>> {
            Ok(self.maps.get(&id).copied())
        }
    }
    impl CraftedMapOwner for Host {
        fn postprocess_map(
            &mut self,
            id: Option<i32>,
            operation: MapPostProcessing,
        ) -> Result<Option<i32>> {
            let Some(mut data) = id.and_then(|id| self.maps.get(&id).copied()) else {
                return Ok(None);
            };
            let new_id = self.next_id;
            self.next_id += 1;
            match operation {
                MapPostProcessing::Scale => data.scale = (data.scale + 1).clamp(0, 4),
                MapPostProcessing::Lock => data.locked = true,
            }
            self.maps.insert(new_id, data);
            self.dirty.push(new_id);
            self.revision += 1;
            Ok(Some(new_id))
        }
    }
    impl CartographyMenuHost for Host {
        type Key = u64;
        type Revision = u64;
        fn owner_key(&self) -> &u64 {
            &self.key
        }
        fn revision(&self) -> u64 {
            self.revision
        }
        fn stage(&self) -> Result<Self> {
            Ok(self.clone())
        }
        fn commit_staged(&mut self, stage: Self) -> Result<()> {
            ensure!(self.key == stage.key, "fixture owner replaced");
            *self = stage;
            Ok(())
        }
        fn ensure_supported_context(&self) -> Result<()> {
            ensure!(self.supported, "unsupported real owner context");
            Ok(())
        }
        fn still_valid_for_actual_player(&self) -> Result<bool> {
            Ok(true)
        }
        fn game_time(&self) -> Result<i64> {
            Ok(self.time)
        }
        fn quickcraft_callbacks_are_order_independent(
            &self,
            _targets: &BTreeSet<usize>,
            _phase: &CartographyPhase,
        ) -> Result<bool> {
            Ok(self.drag)
        }
        fn on_crafted_by(&mut self, stack: &mut ItemStack, phase: &CartographyPhase) -> Result<()> {
            self.callbacks.push((stack.clone(), phase.clone()));
            postprocess_crafted_map(stack, self)?;
            Ok(())
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn set_component(stack: &mut ItemStack, id: i32, value: i32) {
        let mut w = Writer::new();
        w.varint(value);
        stack.components.retain(|c| c.type_id != id);
        stack.components.push(Component {
            type_id: id,
            data: w.into_inner(),
        });
    }
    fn map(count: i32) -> ItemStack {
        let mut map = item("filled_map", count);
        set_component(&mut map, 26, 5);
        map
    }
    fn id(stack: &ItemStack) -> Option<i64> {
        stack
            .component_value(26)
            .unwrap()
            .map(|v| v.as_i64().unwrap())
    }
    fn click(
        menu: &mut CartographyMenu<u64>,
        inv: &mut PlayerInventory,
        host: &mut Host,
        slot: i16,
        button: i8,
        kind: ClickType,
        creative: bool,
    ) -> Result<CartographyOutcome> {
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
    fn setup(addition: &str, count: i32) -> Result<(CartographyMenu<u64>, PlayerInventory, Host)> {
        let mut host = Host::new();
        let mut menu = CartographyMenu::new(7, &host)?;
        let mut inv = PlayerInventory::default();
        inv.set_carried_authoritative(item("emerald", 2));
        inv.set_slot_authoritative(9, map(count));
        inv.set_slot_authoritative(10, item(addition, count));
        menu.initialize(&inv)?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            3,
            0,
            ClickType::QuickMove,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            4,
            0,
            ClickType::QuickMove,
            false,
        )?;
        Ok((menu, inv, host))
    }
    #[test]
    fn failed_shift_commits_real_map_prefix_once_then_retry_takes_that_id() -> Result<()> {
        let (mut menu, mut inv, mut host) = setup("paper", 1)?;
        assert!(host.dirty.is_empty());
        assert!(menu
            .station()
            .result()
            .effective_component_bytes(28)
            .is_some());
        for slot in 9..45 {
            inv.set_slot_authoritative(slot, item("stone", 64));
        }
        menu.broadcast_changes(&inv)?;
        let before = menu.clone();
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync().state_id(),
            slot: 2,
            button: 0,
            kind: ClickType::QuickMove,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        let prepared = menu.prepare_click(&packet, &inv, &host, false)?;
        assert_eq!(menu, before);
        assert!(host.dirty.is_empty());
        assert_eq!(prepared.staged_host().dirty, vec![6]);
        let failed = prepared.commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(host.dirty, vec![6]);
        assert_eq!(id(menu.station().result()), Some(6));
        assert!(menu
            .station()
            .result()
            .effective_component_bytes(28)
            .is_none());
        assert_eq!(menu.station().inputs()[0].count, 1);
        assert_eq!(menu.station().inputs()[1].count, 1);
        assert!(failed.events.iter().any(|e|matches!(e,CartographyMenuEvent::ItemCraftedBy {before,after,phase,..} if before.effective_component_bytes(28).is_some()&&id(after)==Some(6)&&phase.inputs[0].count==1)));
        assert!(!failed
            .events
            .iter()
            .any(|e| matches!(e, CartographyMenuEvent::TakeSound { .. })));
        inv.set_slot_authoritative(44, ItemStack::empty());
        menu.broadcast_changes(&inv)?;
        let taken = click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            0,
            ClickType::QuickMove,
            false,
        )?;
        assert_eq!(host.dirty, vec![6]);
        assert_eq!(id(inv.slot(44).unwrap()), Some(6));
        assert!(menu.station().inputs().iter().all(ItemStack::is_empty));
        assert!(menu.station().result().is_empty());
        assert!(taken.events.iter().any(|e|matches!(e,CartographyMenuEvent::ItemCraftedBy {before,phase,..} if before.is_empty()&&id(phase.player_inventory.slot(44).unwrap())==Some(6))));
        assert_eq!(
            taken
                .events
                .iter()
                .filter(|e| matches!(e, CartographyMenuEvent::TakeSound { .. }))
                .count(),
            1
        );
        assert_eq!(inv.carried().count, 2);
        Ok(())
    }
    #[test]
    fn whole_result_extraction_and_mutable_cursor_aliases_follow_source() -> Result<()> {
        let (mut menu, mut inv, mut host) = setup("map", 2)?;
        let taken = click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            1,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(menu.sync().carried().count, 2);
        let preview_listeners = taken
            .events
            .iter()
            .filter_map(|event| match event {
                CartographyMenuEvent::Sync {
                    event:
                        SyncEvent::SlotListener {
                            menu_slot: 2,
                            stack,
                            ..
                        },
                    phase,
                } => Some((stack.count, phase.carried.count)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(preview_listeners, vec![(0, 0), (2, 0)]);
        assert_eq!(menu.station().inputs()[0].count, 1);
        assert_eq!(menu.station().inputs()[1].count, 1);
        assert!(host.dirty.is_empty());
        assert!(taken.events.iter().any(|e|matches!(e,CartographyMenuEvent::ItemCraftedBy {phase,..} if phase.carried.count==2&&phase.result_preview.count==2)));
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            0,
            ClickType::Pickup,
            false,
        )?;
        let swapped = click(&mut menu, &mut inv, &mut host, 2, 0, ClickType::Swap, false)?;
        assert_eq!(inv.slot(36).unwrap().count, 2);
        assert!(swapped.events.iter().any(|e|matches!(e,CartographyMenuEvent::ItemCraftedBy {phase,..} if phase.player_inventory.slot(36).unwrap().count==2)));
        let (mut menu, mut inv, mut host) = setup("paper", 1)?;
        let taken = click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            0,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(id(menu.sync().carried()), Some(6));
        assert!(menu
            .sync()
            .carried()
            .effective_component_bytes(28)
            .is_none());
        assert!(taken.events.iter().any(|e|matches!(e,CartographyMenuEvent::ItemCraftedBy {phase,after_phase,..} if id(&phase.carried)==Some(5)&&id(&after_phase.carried)==Some(6))));
        let (mut menu, mut inv, mut host) = setup("paper", 2)?;
        let mut marked = map(1);
        set_component(&mut marked, 28, 1);
        inv.set_slot_authoritative(11, marked);
        menu.broadcast_changes(&inv)?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            5,
            0,
            ClickType::Pickup,
            false,
        )?;
        let merged = click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            0,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(menu.sync().carried().count, 2);
        assert_eq!(id(menu.sync().carried()), Some(5));
        assert!(menu
            .sync()
            .carried()
            .effective_component_bytes(28)
            .is_some());
        assert_eq!(host.dirty, vec![6]);
        assert!(merged.events.iter().any(|e|matches!(e,CartographyMenuEvent::ItemCraftedBy {after,phase,after_phase,..} if id(after)==Some(6)&&phase.carried.count==2&&id(&after_phase.carried)==Some(5))));
        Ok(())
    }
    #[test]
    fn partial_duplicate_shift_uses_remainder_argument_and_close_discards_preview() -> Result<()> {
        let (mut menu, mut inv, mut host) = setup("map", 1)?;
        for slot in 9..45 {
            inv.set_slot_authoritative(slot, item("stone", 64));
        }
        inv.set_slot_authoritative(44, map(63));
        menu.broadcast_changes(&inv)?;
        let taken = click(
            &mut menu,
            &mut inv,
            &mut host,
            2,
            0,
            ClickType::QuickMove,
            false,
        )?;
        let counts = taken
            .events
            .iter()
            .filter_map(|e| {
                if let CartographyMenuEvent::ItemCraftedBy { before, .. } = e {
                    Some(before.count)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(counts, vec![2, 1]);
        assert_eq!(inv.slot(44).unwrap().count, 64);
        assert!(host.dirty.is_empty());
        assert!(menu.station().inputs().iter().all(ItemStack::is_empty));
        assert!(taken.events.iter().any(|e|matches!(e,CartographyMenuEvent::PlayerInventoryChanged {phase} if phase.result_preview.count==1)));
        let (mut menu, mut inv, mut host) = setup("map", 2)?;
        inv.set_slot_authoritative(11, item("stone", 4));
        menu.broadcast_changes(&inv)?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            5,
            0,
            ClickType::Pickup,
            false,
        )?;
        let closed = menu
            .prepare_close(&inv, &host, MenuCloseContext::DeadOrDisconnected)?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert_eq!(
            closed
                .drops
                .iter()
                .map(|d| d.stack.count)
                .collect::<Vec<_>>(),
            vec![4, 2, 2]
        );
        let phases = closed
            .events
            .iter()
            .filter_map(|e| {
                if let CartographyMenuEvent::Drop { phase, .. } = e {
                    Some(phase)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(phases[0].result_preview.count, 2);
        assert_eq!(phases[0].inputs[0].count, 2);
        assert!(phases[1].result_preview.is_empty());
        assert!(phases[1].inputs[0].is_empty());
        assert_eq!(phases[1].inputs[1].count, 2);
        assert!(phases[2].inputs.iter().all(ItemStack::is_empty));
        assert!(menu.station().result().is_empty());
        Ok(())
    }
    #[test]
    fn drag_pickall_clone_throw_and_stale_owner_transactions_are_explicit() -> Result<()> {
        let mut host = Host::new();
        let mut menu = CartographyMenu::new(7, &host)?;
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("paper", 6));
        menu.initialize(&inv)?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            3,
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
            1,
            1,
            ClickType::QuickCraft,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            4,
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
        assert_eq!(menu.station().inputs()[1].count, 3);
        assert_eq!(inv.slot(10).unwrap().count, 3);
        assert!(menu.sync().carried().is_empty());
        click(
            &mut menu,
            &mut inv,
            &mut host,
            4,
            0,
            ClickType::Pickup,
            false,
        )?;
        inv.set_slot_authoritative(11, item("paper", 2));
        menu.broadcast_changes(&inv)?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            4,
            0,
            ClickType::PickupAll,
            false,
        )?;
        assert_eq!(menu.sync().carried().count, 8);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            0,
            ClickType::Pickup,
            false,
        )?;
        inv.set_slot_authoritative(12, map(1));
        menu.broadcast_changes(&inv)?;
        click(&mut menu, &mut inv, &mut host, 6, 2, ClickType::Clone, true)?;
        assert_eq!(menu.sync().carried().count, 64);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            0,
            ClickType::Pickup,
            false,
        )?;
        let thrown = click(
            &mut menu,
            &mut inv,
            &mut host,
            6,
            0,
            ClickType::Throw,
            false,
        )?;
        assert_eq!(thrown.drops[0].stack.count, 1);
        inv.set_slot_authoritative(9, item("paper", 1));
        menu.broadcast_changes(&inv)?;
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync().state_id(),
            slot: 3,
            button: 0,
            kind: ClickType::Pickup,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        let plan = menu.prepare_click(&packet, &inv, &host, false)?;
        let before = menu.clone();
        host.revision += 1;
        assert!(plan.commit(&mut menu, &mut inv, &mut host).is_err());
        assert_eq!(menu, before);
        assert_eq!(inv.slot(9).unwrap().count, 1);
        let plan = menu.prepare_click(&packet, &inv, &host, false)?;
        let mut other = CartographyMenu::new(7, &host)?;
        other.initialize(&inv)?;
        assert!(plan.commit(&mut other, &mut inv, &mut host).is_err());
        host.supported = false;
        assert!(menu.prepare_click(&packet, &inv, &host, false).is_err());
        Ok(())
    }
    #[test]
    fn player_counter_boundaries_signed_room_self_alias_and_drag_rejection() -> Result<()> {
        let mut host = Host::new();
        let mut menu = CartographyMenu::new(7, &host)?;
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("stone", 4));
        menu.initialize(&inv)?;
        let taken = click(
            &mut menu,
            &mut inv,
            &mut host,
            3,
            0,
            ClickType::Pickup,
            false,
        )?;
        let phases = taken
            .events
            .iter()
            .filter_map(|e| {
                if let CartographyMenuEvent::PlayerInventoryChanged { phase } = e {
                    Some(phase.carried.count)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(phases, vec![0, 4, 4]);
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            0,
            ClickType::Pickup,
            false,
        )?;
        inv.set_slot_authoritative(36, item("stone", 99));
        inv.set_slot_authoritative(37, item("stone", 1));
        menu.broadcast_changes(&inv)?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            31,
            0,
            ClickType::Pickup,
            false,
        )?;
        let normalized = click(
            &mut menu,
            &mut inv,
            &mut host,
            30,
            0,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(inv.slot(36).unwrap().count, 64);
        assert_eq!(menu.sync().carried().count, 36);
        assert_eq!(
            normalized
                .events
                .iter()
                .filter(|e| matches!(e, CartographyMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        let full = click(
            &mut menu,
            &mut inv,
            &mut host,
            30,
            0,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(
            full.events
                .iter()
                .filter(|e| matches!(e, CartographyMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        click(
            &mut menu,
            &mut inv,
            &mut host,
            -999,
            0,
            ClickType::Pickup,
            false,
        )?;
        inv.set_slot_authoritative(36, item("stone", 99));
        menu.broadcast_changes(&inv)?;
        let alias = click(
            &mut menu,
            &mut inv,
            &mut host,
            30,
            0,
            ClickType::Swap,
            false,
        )?;
        assert_eq!(inv.slot(36).unwrap().count, 64);
        assert_eq!(inv.slot(37).unwrap().count, 35);
        assert!(alias.drops.is_empty());
        assert_eq!(
            alias
                .events
                .iter()
                .filter(|e| matches!(e, CartographyMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        inv.set_slot_authoritative(9, item("paper", 4));
        menu.broadcast_changes(&inv)?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            3,
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
            1,
            1,
            ClickType::QuickCraft,
            false,
        )?;
        click(
            &mut menu,
            &mut inv,
            &mut host,
            4,
            1,
            ClickType::QuickCraft,
            false,
        )?;
        host.drag = false;
        let before = menu.clone();
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync().state_id(),
            slot: -999,
            button: 2,
            kind: ClickType::QuickCraft,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        assert!(menu.prepare_click(&packet, &inv, &host, false).is_err());
        assert_eq!(menu, before);
        assert_eq!(menu.sync().carried().count, 4);
        Ok(())
    }
    #[test]
    fn closed_ui_reopen_and_unresolved_food_guards_precede_inventory_equality() -> Result<()> {
        let mut host = Host::new();
        let mut menu = CartographyMenu::new(7, &host)?;
        let mut inv = PlayerInventory::default();
        // Stable prototype FoodProperties is shared source data, not an override.
        inv.set_slot_authoritative(9, item("bread", 2));
        menu.initialize(&inv)?;
        let packet = ContainerClick {
            window_id: 7,
            state_id: menu.sync().state_id(),
            slot: 3,
            button: 0,
            kind: ClickType::Pickup,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        let old = menu.prepare_click(&packet, &inv, &host, false)?;
        menu.prepare_close(&inv, &host, MenuCloseContext::AliveConnected)?
            .commit(&mut menu, &mut inv, &mut host)?;
        assert!(menu.is_closed());
        assert!(old.validate(&menu, &inv, &host).is_err());
        assert!(menu.prepare_click(&packet, &inv, &host, false).is_err());
        assert!(menu.initialize(&inv).is_err());
        assert!(menu.broadcast_changes(&inv).is_err());
        assert!(menu
            .prepare_close(&inv, &host, MenuCloseContext::AliveConnected)
            .is_err());
        let mut reopened = CartographyMenu::new(7, &host)?;
        reopened.initialize(&inv)?;
        let mut same_except_lifetime = old.before.clone();
        same_except_lifetime.instance = reopened.instance;
        assert_eq!(same_except_lifetime, reopened);
        assert!(old.commit(&mut reopened, &mut inv, &mut host).is_err());
        assert_eq!(inv.slot(9).unwrap().count, 2);
        let packet = ContainerClick {
            state_id: reopened.sync().state_id(),
            ..packet
        };
        let plan = reopened.prepare_click(&packet, &inv, &host, false)?;
        let mut overridden = item("bread", 2);
        overridden.components.push(Component {
            type_id: 20,
            data: overridden.effective_component_bytes(20).unwrap().to_vec(),
        });
        inv.set_slot_authoritative(9, overridden.clone());
        let before = reopened.clone();
        assert!(
            format!("{:#}", plan.validate(&reopened, &inv, &host).unwrap_err())
                .contains("identity unresolved")
        );
        assert!(reopened.prepare_click(&packet, &inv, &host, false).is_err());
        assert_eq!(reopened, before);
        inv.set_slot_authoritative(9, item("bread", 2));
        let mut predicted = packet.clone();
        predicted.carried = overridden.clone();
        assert!(reopened
            .prepare_click(&predicted, &inv, &host, false)
            .is_err());
        predicted.carried = ItemStack::empty();
        predicted.changed_slots.insert(3, overridden);
        assert!(reopened
            .prepare_click(&predicted, &inv, &host, false)
            .is_err());
        click(
            &mut reopened,
            &mut inv,
            &mut host,
            3,
            0,
            ClickType::Pickup,
            false,
        )?;
        assert_eq!(reopened.sync().carried().count, 2);
        assert!(host.dirty.is_empty());
        Ok(())
    }
}
