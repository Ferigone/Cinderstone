//! CrafterMenu shared block inventory, preview, data and native click adapter.
//! The block's actual nine slots are borrowed, never copied into menu ownership.
//! All seven clicks preserve the source intermediate local broadcast boundaries;
//! actual callbacks/owners and bundle click overrides remain explicit contracts.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    protocol::Reader,
    workstations::{
        crafter::{CrafterOwnerIdentity, CrafterRecipes, CrafterState, MENU_SLOTS, PREVIEW_SLOT},
        menu::{MenuRemovalEvent, MenuSync, SyncEvent, WorkstationKind},
    },
};
use anyhow::{ensure, Result};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicU64, Ordering},
};

/// Implement with the actual captured source/world/listener owners. Java
/// traverses an identity HashSet of drag targets; stable native slot order is
/// permitted only when those callbacks are certified insensitive to that order.
/// Unknown or effectful order-sensitive receivers must return false/error.
pub trait CrafterMenuCallbacks: CrafterOwnerIdentity {
    /// Container.stillValidBlockEntity for this captured actual owner/player;
    /// includes world identity and reach, rather than a same-position type test.
    fn still_valid_for_actual_player(&self) -> Result<bool>;
    fn quickcraft_callbacks_are_order_independent(&self) -> Result<bool>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrafterMenu<K: Clone + Eq + std::fmt::Debug> {
    owner_key: K,
    sync: MenuSync,
    preview: ItemStack,
    instance: u64,
    revision: u64,
    drag_status: u8,
    drag_type: u8,
    drag_slots: BTreeSet<usize>,
}
/// Read-only source callback observations; no snapshot transfers item ownership.
/// Retained window0 carried state stays inside player_inventory; the current
/// external menu cursor is the separate carried field.
#[derive(Clone, Debug)]
pub struct CrafterPhase {
    pub state: CrafterState,
    pub player_inventory: PlayerInventory,
    pub carried: ItemStack,
    pub preview: ItemStack,
}
#[derive(Clone, Debug)]
pub enum CrafterMenuEvent {
    Sync {
        event: SyncEvent,
        phase: CrafterPhase,
    },
    Drop {
        index: usize,
        phase: CrafterPhase,
    },
    BlockEntityChanged {
        phase: CrafterPhase,
    },
    /// Actual Inventory.setChanged/timesChanged, distinct from slot listeners.
    PlayerInventoryChanged {
        phase: CrafterPhase,
    },
}
impl CrafterPhase {
    fn observe_return_packet(&mut self, packet: &crate::protocol::Packet) -> Result<()> {
        ensure!(packet.id == 0x15, "unsupported crafter close packet phase");
        let mut reader = Reader::new(&packet.payload);
        ensure!(
            reader.i8()? == -2 && reader.varint()? == 0,
            "crafter close requires raw inventory packet"
        );
        let raw = reader.i16()?;
        let slot = match raw {
            0..=8 => 36 + raw as usize,
            9..=35 => raw as usize,
            40 => 45,
            _ => anyhow::bail!("unsupported crafter close raw slot"),
        };
        let stack = ItemStack::decode_exact(reader.remaining())?;
        let old = self.player_inventory.slot(slot).unwrap();
        ensure!(
            stack.same_item_same_components(&self.carried)?,
            "crafter close changed cursor identity"
        );
        let old_count = if old.is_empty() {
            0
        } else {
            ensure!(
                old.same_item_same_components(&stack)?,
                "crafter close overwrote owned stack"
            );
            old.count
        };
        let added = stack.count - old_count;
        ensure!(
            added > 0 && added <= self.carried.count,
            "invalid crafter close insertion amount"
        );
        self.player_inventory.set_slot_authoritative(slot, stack);
        self.carried = counted(&self.carried, self.carried.count - added);
        Ok(())
    }
}
#[derive(Debug, Default)]
pub struct CrafterMenuOutcome {
    pub events: Vec<CrafterMenuEvent>,
    pub drops: Vec<DroppedStack>,
}
#[derive(Debug)]
pub struct CrafterMenuPlan<K: Clone + Eq + std::fmt::Debug> {
    before: CrafterMenu<K>,
    before_state: CrafterState,
    before_inventory: PlayerInventory,
    next: CrafterMenu<K>,
    next_state: CrafterState,
    next_inventory: PlayerInventory,
    outcome: CrafterMenuOutcome,
    require_order_independence: bool,
}
impl<K: Clone + Eq + std::fmt::Debug> CrafterMenuPlan<K> {
    pub fn outcome(&self) -> &CrafterMenuOutcome {
        &self.outcome
    }
    pub fn validate(
        &self,
        menu: &CrafterMenu<K>,
        state: &CrafterState,
        inventory: &PlayerInventory,
        callbacks: &impl CrafterMenuCallbacks<Key = K>,
    ) -> Result<()> {
        ensure!(
            *callbacks.owner_key() == self.before.owner_key,
            "crafter physical owner replaced"
        );
        if self.require_order_independence {
            ensure!(
                callbacks.quickcraft_callbacks_are_order_independent()?,
                "crafter drag callback-order certification changed"
            );
        }
        ensure!(
            *menu == self.before && *state == self.before_state,
            "stale crafter menu/block state"
        );
        ensure!(
            inventory.slots() == self.before_inventory.slots()
                && inventory.carried() == self.before_inventory.carried()
                && inventory.selected_hotbar() == self.before_inventory.selected_hotbar()
                && inventory.state_id() == self.before_inventory.state_id(),
            "crafter borrowed player inventory changed"
        );
        Ok(())
    }
    /// First stage/validate actual drops, dirty notifications and listeners;
    /// commit these three borrowed snapshots with the captured physical owner.
    pub fn commit(
        self,
        menu: &mut CrafterMenu<K>,
        state: &mut CrafterState,
        inventory: &mut PlayerInventory,
        callbacks: &impl CrafterMenuCallbacks<Key = K>,
    ) -> Result<CrafterMenuOutcome> {
        self.validate(menu, state, inventory, callbacks)?;
        *menu = self.next;
        *state = self.next_state;
        *inventory = self.next_inventory;
        Ok(self.outcome)
    }
}
/// The packet is slot VarInt, container VarInt, then ENABLED boolean. There is
/// no click state ID. Main must dispatch only to the actual crafter owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrafterSlotStatePacket {
    pub slot: i32,
    pub container: i32,
    pub enabled: bool,
}
impl CrafterSlotStatePacket {
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let mut r = Reader::new(payload);
        let value = Self {
            slot: r.varint()?,
            container: r.varint()?,
            enabled: r.bool()?,
        };
        r.finish()?;
        Ok(value)
    }
}
impl<K: Clone + Eq + std::fmt::Debug> CrafterMenu<K> {
    pub fn new(window: u8, callbacks: &impl CrafterMenuCallbacks<Key = K>) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let mut seen = NEXT.load(Ordering::Relaxed);
        let instance = loop {
            let next = seen
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("crafter UI identity exhausted"))?;
            match NEXT.compare_exchange_weak(seen, next, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break seen,
                Err(actual) => seen = actual,
            }
        };
        Ok(Self {
            owner_key: callbacks.owner_key().clone(),
            sync: MenuSync::new(window, WorkstationKind::Crafter)?,
            preview: ItemStack::empty(),
            instance,
            revision: 0,
            drag_status: 0,
            drag_type: 0,
            drag_slots: BTreeSet::new(),
        })
    }
    pub fn sync(&self) -> &MenuSync {
        &self.sync
    }
    pub fn preview(&self) -> &ItemStack {
        &self.preview
    }
    fn revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("crafter UI revision exhausted"))?;
        Ok(())
    }
    fn top(&self, state: &CrafterState) -> Vec<ItemStack> {
        let mut top = state.inputs().to_vec();
        top.push(self.preview.clone());
        top
    }
    fn refresh_for_changed_slots(
        &mut self,
        state: &CrafterState,
        inventory: &PlayerInventory,
        recipes: &mut impl CrafterRecipes,
    ) -> Result<()> {
        // Only an actual local-listener change authorizes shared RecipeCache
        // lookup. Remote-only/data-only resends must not touch that LRU.
        let changed = self
            .sync
            .has_local_slot_changes(&self.top(state), inventory)?;
        if changed {
            self.preview = state.result_preview(recipes)?;
        }
        Ok(())
    }
    fn phase(&self, state: &CrafterState, inventory: &PlayerInventory) -> CrafterPhase {
        CrafterPhase {
            state: state.clone(),
            player_inventory: inventory.clone(),
            carried: self.sync.carried().clone(),
            preview: self.preview.clone(),
        }
    }
    pub fn initialize(
        &mut self,
        state: &CrafterState,
        inventory: &PlayerInventory,
        callbacks: &impl CrafterMenuCallbacks<Key = K>,
        recipes: &mut impl CrafterRecipes,
    ) -> Result<Vec<CrafterMenuEvent>> {
        ensure!(
            *callbacks.owner_key() == self.owner_key,
            "crafter physical owner replaced"
        );
        let mut next = self.clone();
        next.preview = state.result_preview(recipes)?;
        let events = next
            .sync
            .initialize(&next.top(state), inventory, &state.data())?;
        let phase = next.phase(state, inventory);
        let events = events
            .into_iter()
            .map(|event| CrafterMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect();
        next.revision()?;
        *self = next;
        Ok(events)
    }
    /// Bound to nonreentrant source listeners. CrafterMenu is its own local
    /// listener and refreshes preview on changed slots before reaching slot45.
    pub fn broadcast_changes(
        &mut self,
        state: &CrafterState,
        inventory: &PlayerInventory,
        callbacks: &impl CrafterMenuCallbacks<Key = K>,
        recipes: &mut impl CrafterRecipes,
    ) -> Result<Vec<CrafterMenuEvent>> {
        ensure!(
            *callbacks.owner_key() == self.owner_key,
            "crafter physical owner replaced"
        );
        let mut next = self.clone();
        next.refresh_for_changed_slots(state, inventory, recipes)?;
        let events = next
            .sync
            .broadcast_changes(&next.top(state), inventory, &state.data())?;
        let phase = next.phase(state, inventory);
        let events = events
            .into_iter()
            .map(|event| CrafterMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect();
        next.revision()?;
        *self = next;
        Ok(events)
    }
    pub fn still_valid(&self, callbacks: &impl CrafterMenuCallbacks<Key = K>) -> Result<bool> {
        ensure!(
            *callbacks.owner_key() == self.owner_key,
            "crafter physical owner replaced"
        );
        callbacks.still_valid_for_actual_player()
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    pub fn prepare_slot_state(
        &self,
        state: &CrafterState,
        inventory: &PlayerInventory,
        packet: CrafterSlotStatePacket,
        spectator: bool,
        callbacks: &impl CrafterMenuCallbacks<Key = K>,
    ) -> Result<Option<CrafterMenuPlan<K>>> {
        ensure!(
            *callbacks.owner_key() == self.owner_key,
            "crafter physical owner replaced"
        );
        if spectator || packet.container != self.sync.window_id() as i32 {
            return Ok(None);
        }
        let mut next_state = state.clone();
        let changed = if (0..9).contains(&packet.slot) {
            next_state.set_slot_enabled(packet.slot as usize, packet.enabled)?
        } else {
            false
        };
        let mut next = self.clone();
        next.revision()?;
        Ok(Some(CrafterMenuPlan {
            before: self.clone(),
            before_state: state.clone(),
            before_inventory: inventory.clone(),
            next,
            next_state: next_state.clone(),
            next_inventory: inventory.clone(),
            require_order_independence: false,
            outcome: CrafterMenuOutcome {
                events: if changed {
                    vec![CrafterMenuEvent::BlockEntityChanged {
                        phase: self.phase(&next_state, inventory),
                    }]
                } else {
                    vec![]
                },
                drops: vec![],
            },
        }))
    }
    pub fn prepare_click(
        &self,
        state: &CrafterState,
        inventory: &PlayerInventory,
        click: &ContainerClick,
        creative: bool,
        callbacks: &impl CrafterMenuCallbacks<Key = K>,
        recipes: &mut impl CrafterRecipes,
    ) -> Result<CrafterMenuPlan<K>> {
        ensure!(
            click.window_id == self.sync.window_id(),
            "wrong crafter window"
        );
        ensure!(click.slot < MENU_SLOTS as i16, "crafter slot outside menu");
        ensure!(
            *callbacks.owner_key() == self.owner_key,
            "crafter physical owner replaced"
        );
        let mut next = self.clone();
        let mut next_state = state.clone();
        let mut inv = inventory.clone();
        let mut outcome = CrafterMenuOutcome::default();
        let initial = next.sync.state_id();
        let mut require_order_independence = false;
        next.sync.suppress_remote_updates(true);
        {
            let mut action = Action {
                menu: &mut next,
                state: &mut next_state,
                inventory: &mut inv,
                recipes,
                outcome: &mut outcome,
                callbacks,
                require_order_independence: &mut require_order_independence,
            };
            action.do_click(click.slot as i32, click.button as i32, click.kind, creative)?;
        }
        next.refresh_for_changed_slots(&next_state, &inv, recipes)?;
        let phase = next.phase(&next_state, &inv);
        outcome.events.extend(
            next.sync
                .finish_click(
                    click,
                    initial,
                    &next.top(&next_state),
                    &inv,
                    &next_state.data(),
                )?
                .into_iter()
                .map(|event| CrafterMenuEvent::Sync {
                    event,
                    phase: phase.clone(),
                }),
        );
        next.revision()?;
        Ok(CrafterMenuPlan {
            before: self.clone(),
            before_state: state.clone(),
            before_inventory: inventory.clone(),
            next,
            next_state,
            next_inventory: inv,
            outcome,
            require_order_independence,
        })
    }
    /// Only the cursor is returned/dropped. All9 block inputs survive menu close;
    /// slot45 is a derived preview and never an additional owned item.
    pub fn prepare_close(
        &self,
        state: &CrafterState,
        inventory: &PlayerInventory,
        context: MenuCloseContext,
        callbacks: &impl CrafterMenuCallbacks<Key = K>,
    ) -> Result<CrafterMenuPlan<K>> {
        ensure!(
            *callbacks.owner_key() == self.owner_key,
            "crafter physical owner replaced"
        );
        let mut next = self.clone();
        let mut inv = inventory.clone();
        let removal = next.sync.close_inputs_ordered(&mut [], &mut inv, context)?;
        let mut outcome = CrafterMenuOutcome {
            events: vec![],
            drops: removal.outcome.drops,
        };
        let mut phase = self.phase(state, inventory);
        for event in removal.events {
            match event {
                MenuRemovalEvent::Packet { index } => {
                    let packet = removal.outcome.packets[index].clone();
                    phase.observe_return_packet(&packet)?;
                    outcome.events.push(CrafterMenuEvent::Sync {
                        event: SyncEvent::Packet(packet),
                        phase: phase.clone(),
                    });
                }
                MenuRemovalEvent::Drop { index } => {
                    phase.carried = outcome.drops[index].stack.clone();
                    outcome.events.push(CrafterMenuEvent::Drop {
                        index,
                        phase: phase.clone(),
                    });
                    phase.carried = ItemStack::empty();
                }
            }
        }
        next.preview = ItemStack::empty();
        next.drag_status = 0;
        next.drag_slots.clear();
        next.revision()?;
        Ok(CrafterMenuPlan {
            before: self.clone(),
            before_state: state.clone(),
            before_inventory: inventory.clone(),
            next,
            next_state: state.clone(),
            next_inventory: inv,
            outcome,
            require_order_independence: false,
        })
    }
}

struct Action<'a, R: CrafterRecipes, C: CrafterMenuCallbacks> {
    menu: &'a mut CrafterMenu<C::Key>,
    state: &'a mut CrafterState,
    inventory: &'a mut PlayerInventory,
    recipes: &'a mut R,
    outcome: &'a mut CrafterMenuOutcome,
    callbacks: &'a C,
    require_order_independence: &'a mut bool,
}
impl<R: CrafterRecipes, C: CrafterMenuCallbacks> Action<'_, R, C> {
    fn slot(&self, slot: usize) -> ItemStack {
        if slot < 9 {
            self.state.inputs()[slot].clone()
        } else if slot == PREVIEW_SLOT {
            self.menu.preview.clone()
        } else {
            self.inventory.slot(slot).unwrap().clone()
        }
    }
    fn may_place(&self, slot: usize) -> bool {
        slot < 9 && self.state.may_place_menu(slot) || (9..45).contains(&slot)
    }
    fn phase(&self) -> CrafterPhase {
        self.menu.phase(self.state, self.inventory)
    }
    fn dirty(&mut self) {
        self.outcome
            .events
            .push(CrafterMenuEvent::BlockEntityChanged {
                phase: self.phase(),
            });
    }
    fn broadcast(&mut self) -> Result<()> {
        // The source self-listener refreshes preview while traversing changed
        // slots, before result45. Its ordinary nonreentrant owner sees the same
        // current inputs; remote updates remain suppressed during clicks.
        self.menu
            .refresh_for_changed_slots(self.state, self.inventory, self.recipes)?;
        let phase = self.phase();
        self.outcome.events.extend(
            self.menu
                .sync
                .broadcast_changes(
                    &self.menu.top(self.state),
                    self.inventory,
                    &self.state.data(),
                )?
                .into_iter()
                .map(|event| CrafterMenuEvent::Sync {
                    event,
                    phase: phase.clone(),
                }),
        );
        Ok(())
    }
    fn slot_changed(&mut self, slot: usize) -> Result<()> {
        if slot < 9 {
            self.dirty();
            self.broadcast()?;
        } else if slot < 45 {
            self.outcome
                .events
                .push(CrafterMenuEvent::PlayerInventoryChanged {
                    phase: self.phase(),
                });
        }
        Ok(())
    }
    fn raw(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        if slot < 9 {
            self.state.replace_stack_no_notify(slot, stack)?;
        } else if slot < 45 {
            self.inventory.set_slot_authoritative(slot, stack);
        }
        Ok(())
    }
    fn set(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        if slot < 9 {
            if self.state.is_slot_disabled(slot) && self.state.inputs()[slot].is_empty() {
                self.state.set_slot_enabled(slot, true)?;
                self.dirty();
            }
            self.state.set_slot(slot, stack)?;
            self.dirty();
            self.slot_changed(slot)?;
        } else if slot < 45 {
            self.inventory.set_slot_authoritative(slot, stack);
            self.slot_changed(slot)?;
        }
        Ok(())
    }
    fn try_remove(&mut self, slot: usize, amount: i32, maximum: i32) -> Result<ItemStack> {
        if slot == PREVIEW_SLOT {
            return Ok(ItemStack::empty());
        }
        let source = self.slot(slot);
        let n = source.count.min(amount).min(maximum);
        if source.is_empty() || n <= 0 {
            return Ok(ItemStack::empty());
        }
        self.raw(slot, counted(&source, source.count - n))?;
        if slot < 9 {
            self.dirty();
        }
        if self.slot(slot).is_empty() {
            self.set(slot, ItemStack::empty())?;
        }
        Ok(counted(&source, n))
    }
    fn take(&mut self, slot: usize, amount: i32, maximum: i32) -> Result<ItemStack> {
        let value = self.try_remove(slot, amount, maximum)?;
        if !value.is_empty() {
            self.slot_changed(slot)?;
        }
        Ok(value)
    }
    fn drop_stack(&mut self, stack: ItemStack) {
        if !stack.is_empty() {
            let index = self.outcome.drops.len();
            self.outcome.drops.push(DroppedStack {
                stack,
                around: false,
                trace_item: true,
            });
            self.outcome.events.push(CrafterMenuEvent::Drop {
                index,
                phase: self.phase(),
            });
        }
    }
    fn reset_drag(&mut self) {
        self.menu.drag_status = 0;
        self.menu.drag_slots.clear();
    }
    fn do_click(&mut self, slot: i32, button: i32, kind: ClickType, creative: bool) -> Result<()> {
        if kind == ClickType::QuickCraft {
            return self.drag(slot, button, creative);
        }
        if self.menu.drag_status != 0 {
            self.reset_drag();
            return Ok(());
        }
        match kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let source = self.menu.sync.carried().clone();
                    let n = if button == 0 {
                        source.count
                    } else {
                        source.count.min(1)
                    };
                    if button == 0 {
                        // Player.drop observes the original whole cursor;
                        // AbstractContainerMenu clears it only afterward.
                        self.drop_stack(counted(&source, n));
                        self.menu.sync.set_carried(ItemStack::empty());
                    } else {
                        // The source split(1) mutates carried before dropping.
                        self.menu
                            .sync
                            .set_carried(counted(&source, source.count - n));
                        self.drop_stack(counted(&source, n));
                    }
                } else if slot >= 0 {
                    if kind == ClickType::Pickup {
                        self.pickup(slot as usize, button)?;
                    } else if slot != PREVIEW_SLOT as i32 {
                        self.quick_move(slot as usize)?;
                    }
                }
            }
            ClickType::Swap if slot >= 0 && ((0..9).contains(&button) || button == 40) => {
                self.swap(slot as usize, button, creative)?
            }
            ClickType::Clone if creative && slot >= 0 && self.menu.sync.carried().is_empty() => {
                let source = self.slot(slot as usize);
                reject_bundle(&source)?;
                if !source.is_empty() {
                    self.menu
                        .sync
                        .set_carried(counted(&source, source.max_stack_size()?));
                }
            }
            ClickType::Throw if slot >= 0 && self.menu.sync.carried().is_empty() => {
                let source = self.slot(slot as usize);
                reject_bundle(&source)?;
                let amount = if button == 0 { 1 } else { source.count };
                let stack = self.take(slot as usize, amount, i32::MAX)?;
                self.drop_stack(stack);
            }
            ClickType::PickupAll if slot >= 0 => self.pickup_all(slot as usize, button)?,
            _ => {}
        }
        Ok(())
    }
    fn pickup(&mut self, slot: usize, button: i32) -> Result<()> {
        let source = self.slot(slot);
        let carried = self.menu.sync.carried().clone();
        reject_bundle(&source)?;
        reject_bundle(&carried)?;
        if source.is_empty() {
            if !carried.is_empty() && self.may_place(slot) {
                let n = (if button == 0 { carried.count } else { 1 }).min(limit(&carried)?);
                self.menu
                    .sync
                    .set_carried(counted(&carried, carried.count - n));
                self.set(slot, counted(&carried, n))?;
            }
        } else if slot != PREVIEW_SLOT {
            if carried.is_empty() {
                let n = if button == 0 {
                    source.count
                } else {
                    (source.count + 1) / 2
                };
                let taken = self.try_remove(slot, n, i32::MAX)?;
                if !taken.is_empty() {
                    self.menu.sync.set_carried(taken);
                    self.slot_changed(slot)?;
                }
            } else if self.may_place(slot) {
                if source.same_item_same_components(&carried)? {
                    // Slot.safeInsert retains its signed capacity and invokes
                    // setByPlayer even for zero acceptance. A loaded overfull
                    // target returns the excess to the carried stack before
                    // the setter/dirty/broadcast callbacks run.
                    let n = (if button == 0 { carried.count } else { 1 })
                        .min(limit(&carried)? - source.count);
                    self.menu
                        .sync
                        .set_carried(counted(&carried, carried.count.wrapping_sub(n)));
                    self.set(slot, counted(&source, source.count.wrapping_add(n)))?;
                } else if carried.count <= limit(&carried)? {
                    self.menu.sync.set_carried(source);
                    self.set(slot, carried)?;
                }
            }
        }
        self.slot_changed(slot)
    }
    fn move_to(&mut self, origin: usize, start: usize, end: usize, reverse: bool) -> Result<bool> {
        let before = self.slot(origin).count;
        let order = if reverse {
            (start..end).rev().collect::<Vec<_>>()
        } else {
            (start..end).collect()
        };
        if self.slot(origin).is_stackable()? {
            for &slot in &order {
                let source = self.slot(origin);
                if source.is_empty() {
                    break;
                }
                let old = self.slot(slot);
                if !old.is_empty() && old.same_item_same_components(&source)? {
                    let n = source.count.min((limit(&old)? - old.count).max(0));
                    if n > 0 {
                        self.raw(origin, counted(&source, source.count - n))?;
                        self.raw(slot, counted(&old, old.count + n))?;
                        self.slot_changed(slot)?;
                    }
                }
            }
        }
        if !self.slot(origin).is_empty() {
            for slot in order {
                if self.slot(slot).is_empty() && self.may_place(slot) {
                    let source = self.slot(origin);
                    let n = source.count.min(limit(&source)?);
                    self.raw(origin, counted(&source, source.count - n))?;
                    self.set(slot, counted(&source, n))?;
                    self.slot_changed(slot)?;
                    break;
                }
            }
        }
        Ok(self.slot(origin).count != before)
    }
    fn quick_move(&mut self, slot: usize) -> Result<()> {
        loop {
            let source = self.slot(slot);
            reject_bundle(&source)?;
            if source.is_empty() {
                break;
            }
            let moved = if slot < 9 {
                self.move_to(slot, 9, 45, true)?
            } else {
                self.move_to(slot, 0, 9, false)?
            };
            if !moved {
                break;
            }
            if self.slot(slot).is_empty() {
                self.set(slot, ItemStack::empty())?;
            } else {
                self.slot_changed(slot)?;
            }
            self.slot_changed(slot)?;
            if self.slot(slot).item_id != source.item_id {
                break;
            }
        }
        Ok(())
    }
    fn swap(&mut self, slot: usize, button: i32, creative: bool) -> Result<()> {
        let hotbar = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        if (9..45).contains(&slot) && slot == hotbar {
            let same = self.slot(slot);
            ensure!(
                same.is_empty() || same.count <= limit(&same)?,
                "overfull crafter self-SWAP requires shared ItemStack alias semantics"
            );
            if !same.is_empty() {
                self.slot_changed(slot)?;
                self.slot_changed(slot)?;
            }
            return Ok(());
        }
        let source = self.slot(slot);
        let held = self.inventory.slot(hotbar).unwrap().clone();
        reject_bundle(&source)?;
        reject_bundle(&held)?;
        if source.is_empty() && held.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            if slot != PREVIEW_SLOT {
                self.inventory
                    .set_slot_authoritative(hotbar, source.clone());
                self.set(slot, ItemStack::empty())?;
                self.slot_changed(slot)?;
            }
        } else if source.is_empty() {
            if self.may_place(slot) {
                let n = held.count.min(limit(&held)?);
                self.inventory
                    .set_slot_authoritative(hotbar, counted(&held, held.count - n));
                self.set(slot, counted(&held, n))?;
            }
        } else if slot != PREVIEW_SLOT && self.may_place(slot) {
            let max = limit(&held)?;
            if held.count > max {
                self.inventory
                    .set_slot_authoritative(hotbar, counted(&held, held.count - max));
                self.set(slot, counted(&held, max))?;
                self.slot_changed(slot)?;
                let mut old = source;
                self.add_inventory(&mut old, creative)?;
                self.drop_stack(old);
            } else {
                self.inventory.set_slot_authoritative(hotbar, source);
                self.set(slot, held)?;
                self.slot_changed(slot)?;
            }
        }
        Ok(())
    }
    fn add_inventory(&mut self, stack: &mut ItemStack, creative: bool) -> Result<()> {
        let order = (36..45).chain(9..36).collect::<Vec<_>>();
        if stack.is_damageable()? && stack.damage_value()? > 0 {
            if let Some(slot) = order
                .iter()
                .copied()
                .find(|i| self.inventory.slot(*i).unwrap().is_empty())
            {
                self.inventory
                    .set_slot_authoritative(slot, std::mem::take(stack));
            } else if creative {
                *stack = ItemStack::empty();
            }
            return Ok(());
        }
        let mut merges = vec![36 + self.inventory.selected_hotbar(), 45];
        merges.extend(order.iter().copied());
        for slot in merges {
            let old = self.inventory.slot(slot).unwrap();
            if !old.is_empty() && old.is_stackable()? && old.same_item_same_components(stack)? {
                let n = stack.count.min((limit(old)? - old.count).max(0));
                if n > 0 {
                    self.inventory
                        .set_slot_authoritative(slot, counted(old, old.count + n));
                    *stack = counted(stack, stack.count - n);
                }
                if stack.is_empty() {
                    return Ok(());
                }
            }
        }
        for slot in order {
            if self.inventory.slot(slot).unwrap().is_empty() {
                let n = stack.count.min(limit(stack)?);
                self.inventory
                    .set_slot_authoritative(slot, counted(stack, n));
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
    fn pickup_all(&mut self, clicked: usize, button: i32) -> Result<()> {
        let carried = self.menu.sync.carried().clone();
        reject_bundle(&carried)?;
        if carried.is_empty() || !self.slot(clicked).is_empty() && clicked != PREVIEW_SLOT {
            return Ok(());
        }
        let order = if button == 0 {
            (0..46).collect::<Vec<_>>()
        } else {
            (0..46).rev().collect()
        };
        for pass in 0..2 {
            for &slot in &order {
                let source = self.slot(slot);
                let carried = self.menu.sync.carried().clone();
                if carried.count >= carried.max_stack_size()? {
                    break;
                }
                if slot == PREVIEW_SLOT
                    || source.is_empty()
                    || source.count > carried.max_stack_size()?
                    || !source.same_item_same_components(&carried)?
                    || pass == 0 && source.count == source.max_stack_size()?
                {
                    continue;
                }
                let taken = self.take(
                    slot,
                    source.count,
                    carried.max_stack_size()? - carried.count,
                )?;
                self.menu
                    .sync
                    .set_carried(counted(&carried, carried.count + taken.count));
            }
        }
        Ok(())
    }
    fn drag(&mut self, slot: i32, button: i32, creative: bool) -> Result<()> {
        let previous = self.menu.drag_status;
        self.menu.drag_status = (button & 3) as u8;
        if !(previous == 1 && self.menu.drag_status == 2) && previous != self.menu.drag_status {
            self.reset_drag();
            return Ok(());
        }
        if self.menu.sync.carried().is_empty() {
            self.reset_drag();
            return Ok(());
        }
        reject_bundle(self.menu.sync.carried())?;
        match self.menu.drag_status {
            0 => {
                self.menu.drag_type = ((button >> 2) & 3) as u8;
                if self.menu.drag_type <= 1 || self.menu.drag_type == 2 && creative {
                    self.menu.drag_status = 1;
                    self.menu.drag_slots.clear();
                } else {
                    self.reset_drag();
                }
            }
            1 if slot >= 0 => {
                let slot = slot as usize;
                let source = self.slot(slot);
                let carried = self.menu.sync.carried();
                if self.may_place(slot)
                    && (source.is_empty()
                        || source.same_item_same_components(carried)?
                            && source.count <= carried.max_stack_size()?)
                    && (self.menu.drag_type == 2
                        || carried.count > self.menu.drag_slots.len() as i32)
                {
                    self.menu.drag_slots.insert(slot);
                }
            }
            1 => {}
            2 => {
                if self.menu.drag_slots.len() == 1 {
                    let slot = *self.menu.drag_slots.iter().next().unwrap();
                    self.reset_drag();
                    return self.do_click(
                        slot as i32,
                        self.menu.drag_type as i32,
                        ClickType::Pickup,
                        creative,
                    );
                }
                if self.menu.drag_slots.len() > 1 {
                    ensure!(self.callbacks.quickcraft_callbacks_are_order_independent()?, "multi-slot crafter drag requires actual callback-order independence or source Slot identity order");
                    *self.require_order_independence = true;
                }
                let carried = self.menu.sync.carried().clone();
                let mut left = carried.count;
                let slots = self.menu.drag_slots.iter().copied().collect::<Vec<_>>();
                let each = match self.menu.drag_type {
                    0 => carried.count / (slots.len().max(1) as i32),
                    1 => 1,
                    2 => carried.max_stack_size()?,
                    _ => 0,
                };
                for slot in slots {
                    let source = self.slot(slot);
                    if !self.may_place(slot)
                        || !source.is_empty()
                            && (source.count > carried.max_stack_size()?
                                || !source.same_item_same_components(&carried)?)
                        || self.menu.drag_type != 2
                            && carried.count < self.menu.drag_slots.len() as i32
                    {
                        continue;
                    }
                    let n = (each + source.count.max(0)).min(limit(&carried)?);
                    left -= n - source.count.max(0);
                    self.set(slot, counted(&carried, n))?;
                }
                self.menu.sync.set_carried(counted(&carried, left));
                self.reset_drag();
            }
            _ => self.reset_drag(),
        }
        Ok(())
    }
}

fn counted(stack: &ItemStack, n: i32) -> ItemStack {
    if stack.is_empty() || n <= 0 {
        ItemStack::empty()
    } else {
        let mut value = stack.clone();
        value.count = n;
        value
    }
}
fn limit(stack: &ItemStack) -> Result<i32> {
    let max = stack.max_stack_size()?.min(99);
    ensure!(max > 0, "invalid crafter slot stack limit");
    Ok(max)
}
fn reject_bundle(stack: &ItemStack) -> Result<()> {
    ensure!(
        inventory::item_name(stack.item_id) != Some("bundle"),
        "crafter bundle click override requires actual feature/component owner"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crafting::{RecipeBook, RecipeInput},
        protocol::Writer,
    };
    use std::collections::BTreeMap;
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    struct Callbacks(bool, u64);
    impl CrafterOwnerIdentity for Callbacks {
        type Key = u64;
        fn owner_key(&self) -> &u64 {
            &self.1
        }
    }
    impl CrafterMenuCallbacks for Callbacks {
        fn still_valid_for_actual_player(&self) -> Result<bool> {
            Ok(true)
        }
        fn quickcraft_callbacks_are_order_independent(&self) -> Result<bool> {
            Ok(self.0)
        }
    }
    struct Recipes;
    impl CrafterRecipes for Recipes {
        fn potential_recipe(&mut self, input: &RecipeInput) -> Result<Option<String>> {
            Ok(RecipeBook::vanilla()
                .evaluate("minecraft:oak_planks", input)?
                .map(|_| "minecraft:oak_planks".to_owned()))
        }
        fn assemble(&mut self, id: &str, input: &RecipeInput) -> Result<ItemStack> {
            Ok(RecipeBook::vanilla().evaluate(id, input)?.unwrap().stack)
        }
        fn remaining_items(&mut self, id: &str, input: &RecipeInput) -> Result<Vec<ItemStack>> {
            Ok(RecipeBook::vanilla()
                .evaluate(id, input)?
                .unwrap()
                .remaining)
        }
    }
    fn click(
        menu: &mut CrafterMenu<u64>,
        state: &mut CrafterState,
        inv: &mut PlayerInventory,
        slot: i16,
        kind: ClickType,
    ) -> Result<CrafterMenuOutcome> {
        let packet = ContainerClick {
            window_id: 3,
            state_id: menu.sync().state_id(),
            slot,
            button: 0,
            kind,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        menu.prepare_click(
            state,
            inv,
            &packet,
            false,
            &Callbacks(true, 1),
            &mut Recipes,
        )?
        .commit(menu, state, inv, &Callbacks(true, 1))
    }
    #[test]
    fn noncontiguous_preview_borrowed_block_inputs_and_close_cursor() -> Result<()> {
        let mut state = CrafterState::default();
        state.set_slot(4, item("oak_log", 1))?;
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("emerald", 3));
        let mut menu = CrafterMenu::new(3, &Callbacks(true, 1))?;
        let initial = menu.initialize(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        let packet = initial
            .iter()
            .find_map(|e| match e {
                CrafterMenuEvent::Sync {
                    event: SyncEvent::Packet(p),
                    ..
                } if p.id == 0x13 => Some(p),
                _ => None,
            })
            .unwrap();
        let mut r = Reader::new(&packet.payload);
        assert_eq!(r.u8()?, 3);
        r.varint()?;
        assert_eq!(r.varint()?, 46);
        let mut decoded = vec![];
        for _ in 0..46 {
            let (stack, n) = ItemStack::decode(r.remaining())?;
            r.bytes(n)?;
            decoded.push(stack);
        }
        assert_eq!(decoded[4].item_id, item("oak_log", 1).item_id);
        assert_eq!(decoded[9].count, 3);
        assert_eq!(decoded[45].item_id, item("oak_planks", 1).item_id);
        click(&mut menu, &mut state, &mut inv, 45, ClickType::Pickup)?;
        assert!(menu.sync().carried().is_empty());
        assert_eq!(state.inputs()[4].count, 1);
        click(&mut menu, &mut state, &mut inv, 4, ClickType::Pickup)?;
        let closed = menu
            .prepare_close(
                &state,
                &inv,
                MenuCloseContext::DeadOrDisconnected,
                &Callbacks(true, 1),
            )?
            .commit(&mut menu, &mut state, &mut inv, &Callbacks(true, 1))?;
        assert_eq!(closed.drops.len(), 1);
        assert_eq!(closed.drops[0].stack.item_id, item("oak_log", 1).item_id);
        assert_eq!(closed.drops[0].stack.count, 1);
        assert!(state.inputs().iter().all(ItemStack::is_empty));
        assert_eq!(inv.slot(9).unwrap().count, 3);
        Ok(())
    }
    #[test]
    fn empty_slot_toggle_routes_real_packet_and_shared_state_survives_close() -> Result<()> {
        let mut state = CrafterState::default();
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("oak_log", 2));
        let mut menu = CrafterMenu::new(3, &Callbacks(true, 1))?;
        menu.initialize(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        let mut bytes = Writer::new();
        bytes.varint(0).varint(3).bool(false);
        let toggle = CrafterSlotStatePacket::decode(&bytes.into_inner())?;
        assert!(menu
            .prepare_slot_state(&state, &inv, toggle, true, &Callbacks(true, 1))?
            .is_none());
        let changed = menu
            .prepare_slot_state(&state, &inv, toggle, false, &Callbacks(true, 1))?
            .unwrap()
            .commit(&mut menu, &mut state, &mut inv, &Callbacks(true, 1))?;
        assert!(matches!(
            changed.events.as_slice(),
            [CrafterMenuEvent::BlockEntityChanged { .. }]
        ));
        assert!(state.is_slot_disabled(0));
        click(&mut menu, &mut state, &mut inv, 0, ClickType::Pickup)?;
        assert!(state.is_slot_disabled(0)); // empty click must not call setItem and auto-enable
        click(&mut menu, &mut state, &mut inv, 9, ClickType::QuickMove)?;
        assert!(state.inputs()[0].is_empty());
        assert_eq!(state.inputs()[1].count, 2);
        assert_eq!(menu.preview().count, 4);
        let ignored = menu
            .prepare_slot_state(
                &state,
                &inv,
                CrafterSlotStatePacket {
                    slot: 1,
                    container: 3,
                    enabled: false,
                },
                false,
                &Callbacks(true, 1),
            )?
            .unwrap();
        assert!(ignored.outcome().events.is_empty());
        drop(ignored);
        let stale = menu.prepare_close(
            &state,
            &inv,
            MenuCloseContext::AliveConnected,
            &Callbacks(true, 1),
        )?;
        state
            .prepare_neighbor_signal(&Callbacks(true, 1), false, true)?
            .commit(&mut state, &Callbacks(true, 1))?;
        assert!(stale
            .validate(&menu, &state, &inv, &Callbacks(true, 1))
            .is_err());
        let data = menu.broadcast_changes(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        assert!(data.iter().any(|e| match e {
            CrafterMenuEvent::Sync {
                event: SyncEvent::Packet(p),
                ..
            } if p.id == 0x14 => {
                let mut r = Reader::new(&p.payload);
                r.u8().ok() == Some(3) && r.i16().ok() == Some(9) && r.i16().ok() == Some(1)
            }
            _ => false,
        }));
        let close = menu
            .prepare_close(
                &state,
                &inv,
                MenuCloseContext::AliveConnected,
                &Callbacks(true, 1),
            )?
            .commit(&mut menu, &mut state, &mut inv, &Callbacks(true, 1))?;
        assert!(close.drops.is_empty());
        assert_eq!(state.inputs()[1].count, 2);
        assert!(inv.slot(9).unwrap().is_empty());
        Ok(())
    }
    fn custom_click(
        menu: &mut CrafterMenu<u64>,
        state: &mut CrafterState,
        inv: &mut PlayerInventory,
        slot: i16,
        button: i8,
        kind: ClickType,
        creative: bool,
    ) -> Result<CrafterMenuOutcome> {
        let packet = ContainerClick {
            window_id: 3,
            state_id: menu.sync().state_id(),
            slot,
            button,
            kind,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        menu.prepare_click(
            state,
            inv,
            &packet,
            creative,
            &Callbacks(true, 1),
            &mut Recipes,
        )?
        .commit(menu, state, inv, &Callbacks(true, 1))
    }
    #[test]
    fn all_modes_keep_fake_preview_and_suppressed_local_broadcast_order() -> Result<()> {
        let mut state = CrafterState::default();
        state.set_slot(4, item("oak_log", 1))?;
        state.set_slot_enabled(0, false)?;
        let mut inv = PlayerInventory::default();
        let mut menu = CrafterMenu::new(3, &Callbacks(true, 1))?;
        menu.initialize(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            45,
            2,
            ClickType::Clone,
            false,
        )?;
        assert!(menu.sync().carried().is_empty());
        custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            45,
            2,
            ClickType::Clone,
            true,
        )?;
        assert_eq!(menu.sync().carried().item_id, item("oak_planks", 1).item_id);
        assert_eq!(menu.sync().carried().count, 64);
        assert_eq!(state.inputs()[4].count, 1);
        let copied = custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            -999,
            0,
            ClickType::Pickup,
            true,
        )?;
        assert_eq!(copied.drops[0].stack.count, 64);
        custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            45,
            0,
            ClickType::Swap,
            false,
        )?;
        assert!(inv.slot(36).unwrap().is_empty());
        assert_eq!(menu.preview().count, 4);
        inv.set_slot_authoritative(9, item("oak_log", 4));
        click(&mut menu, &mut state, &mut inv, 9, ClickType::Pickup)?;
        custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            -999,
            0,
            ClickType::QuickCraft,
            false,
        )?;
        for slot in [0, 1, 10, 45] {
            custom_click(
                &mut menu,
                &mut state,
                &mut inv,
                slot,
                1,
                ClickType::QuickCraft,
                false,
            )?;
        }
        let dragged = custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            -999,
            2,
            ClickType::QuickCraft,
            false,
        )?;
        assert!(state.inputs()[0].is_empty());
        assert!(state.is_slot_disabled(0));
        assert_eq!(state.inputs()[1].count, 2);
        assert_eq!(inv.slot(10).unwrap().count, 2);
        let local = dragged
            .events
            .iter()
            .position(|e| {
                matches!(
                    e,
                    CrafterMenuEvent::Sync {
                        event: SyncEvent::SlotListener { menu_slot: 1, .. },
                        ..
                    }
                )
            })
            .unwrap();
        let remote = dragged
            .events
            .iter()
            .position(|e| {
                matches!(
                    e,
                    CrafterMenuEvent::Sync {
                        event: SyncEvent::Packet(_),
                        ..
                    }
                )
            })
            .unwrap();
        assert!(local < remote);
        click(&mut menu, &mut state, &mut inv, 10, ClickType::Pickup)?;
        custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            10,
            0,
            ClickType::PickupAll,
            false,
        )?;
        assert_eq!(menu.sync().carried().count, 5);
        assert!(state.inputs().iter().all(ItemStack::is_empty));
        inv.set_slot_authoritative(45, item("oak_log", 3));
        custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            2,
            40,
            ClickType::Swap,
            false,
        )?;
        assert_eq!(state.inputs()[2].count, 3);
        assert!(inv.slot(45).unwrap().is_empty());
        let ignored = custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            2,
            1,
            ClickType::Throw,
            false,
        )?;
        assert!(ignored.drops.is_empty());
        custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            -999,
            0,
            ClickType::Pickup,
            false,
        )?;
        let thrown = custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            2,
            1,
            ClickType::Throw,
            false,
        )?;
        assert_eq!(thrown.drops[0].stack.count, 3);
        assert!(state.inputs()[2].is_empty());
        assert!(
            thrown
                .events
                .iter()
                .position(|e| matches!(e, CrafterMenuEvent::Drop { .. }))
                .unwrap()
                > thrown
                    .events
                    .iter()
                    .position(|e| matches!(
                        e,
                        CrafterMenuEvent::Sync {
                            event: SyncEvent::SlotListener { menu_slot: 2, .. },
                            ..
                        }
                    ))
                    .unwrap()
        );
        state.set_slot(4, item("oak_log", 1))?;
        menu.broadcast_changes(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        let taken = click(&mut menu, &mut state, &mut inv, 4, ClickType::Pickup)?;
        assert_eq!(
            taken
                .events
                .iter()
                .filter(|e| matches!(e, CrafterMenuEvent::BlockEntityChanged { .. }))
                .count(),
            5
        );
        let first_local = taken
            .events
            .iter()
            .position(|e| {
                matches!(
                    e,
                    CrafterMenuEvent::Sync {
                        event: SyncEvent::SlotListener { menu_slot: 4, .. },
                        ..
                    }
                )
            })
            .unwrap();
        let last_dirty = taken
            .events
            .iter()
            .rposition(|e| matches!(e, CrafterMenuEvent::BlockEntityChanged { .. }))
            .unwrap();
        let first_packet = taken
            .events
            .iter()
            .position(|e| {
                matches!(
                    e,
                    CrafterMenuEvent::Sync {
                        event: SyncEvent::Packet(_),
                        ..
                    }
                )
            })
            .unwrap();
        assert!(first_local < last_dirty && last_dirty < first_packet);
        Ok(())
    }
    #[test]
    fn outside_pickup_drop_observes_left_cursor_and_right_split_phases() -> Result<()> {
        for button in [0, 1] {
            let mut state = CrafterState::default();
            state.set_slot(4, item("oak_log", 2))?;
            let mut inventory = PlayerInventory::new();
            inventory.set_slot_authoritative(9, item("stone", 3));
            let callbacks = Callbacks(true, 1);
            let mut menu = CrafterMenu::new(3, &callbacks)?;
            menu.initialize(&state, &inventory, &callbacks, &mut Recipes)?;
            click(&mut menu, &mut state, &mut inventory, 9, ClickType::Pickup)?;
            let outcome = custom_click(
                &mut menu,
                &mut state,
                &mut inventory,
                -999,
                button,
                ClickType::Pickup,
                false,
            )?;
            assert_eq!(outcome.drops.len(), 1);
            let dropped = if button == 0 { 3 } else { 1 };
            assert_eq!(outcome.drops[0].stack, item("stone", dropped));
            assert!(!outcome.drops[0].around && outcome.drops[0].trace_item);
            let phase = outcome
                .events
                .iter()
                .find_map(|event| match event {
                    CrafterMenuEvent::Drop { index: 0, phase } => Some(phase),
                    _ => None,
                })
                .expect("actual drop source phase");
            assert_eq!(
                phase.carried,
                item("stone", if button == 0 { 3 } else { 2 })
            );
            assert_eq!(phase.state.inputs()[4], item("oak_log", 2));
            assert_eq!(phase.preview, item("oak_planks", 4));
            assert!(phase.player_inventory.slot(9).unwrap().is_empty());
            assert_eq!(menu.sync().carried().count, 3 - dropped);
            assert_eq!(
                menu.sync().carried().count + outcome.drops[0].stack.count,
                3
            );
            assert_eq!(state.inputs()[4], item("oak_log", 2));
        }
        Ok(())
    }

    #[test]
    fn matching_full_and_overfull_pickup_retains_source_setter_and_broadcast_phases() -> Result<()>
    {
        // crq.safeInsert sets the existing stack even for n=0, followed by
        // cpu.doClick's final Slot.setChanged. For an overfull loaded stack,
        // its signed capacity moves the excess back to the actual cursor.
        for original_count in [64, 99] {
            let mut inputs = std::array::from_fn(|_| ItemStack::empty());
            inputs[0] = item("stone", original_count);
            let mut state = CrafterState::restore(inputs, &[], 0, 0)?;
            let mut inventory = PlayerInventory::new();
            inventory.set_slot_authoritative(9, item("stone", 1));
            let callbacks = Callbacks(true, 1);
            let mut menu = CrafterMenu::new(3, &callbacks)?;
            menu.initialize(&state, &inventory, &callbacks, &mut Recipes)?;
            click(&mut menu, &mut state, &mut inventory, 9, ClickType::Pickup)?;
            // An authoritative change awaiting the next ordinary broadcast
            // makes the intermediate first broadcast observable. It must run
            // after the setter/first Slot dirty and before the final Slot dirty.
            inventory.set_slot_authoritative(10, item("quartz", 2));
            let outcome = click(&mut menu, &mut state, &mut inventory, 0, ClickType::Pickup)?;
            let dirty = outcome
                .events
                .iter()
                .enumerate()
                .filter_map(|(index, event)| {
                    matches!(event, CrafterMenuEvent::BlockEntityChanged { .. }).then_some(index)
                })
                .collect::<Vec<_>>();
            assert_eq!(dirty.len(), 3);
            let listener = outcome
                .events
                .iter()
                .position(|event| {
                    matches!(event,
                        CrafterMenuEvent::Sync {
                            event: SyncEvent::SlotListener { menu_slot: 10, stack, .. }, ..
                        } if stack == &item("quartz", 2)
                    )
                })
                .expect("pending actual inventory listener at intermediate broadcast");
            assert!(dirty[1] < listener && listener < dirty[2]);
            let expected_cursor = 1 + original_count - 64;
            for index in dirty {
                let CrafterMenuEvent::BlockEntityChanged { phase } = &outcome.events[index] else {
                    unreachable!()
                };
                assert_eq!(phase.state.inputs()[0], item("stone", 64));
                assert_eq!(phase.carried, item("stone", expected_cursor));
                assert_eq!(phase.player_inventory.slot(10), Some(&item("quartz", 2)));
                assert!(phase.preview.is_empty());
            }
            assert_eq!(state.inputs()[0], item("stone", 64));
            assert_eq!(menu.sync().carried(), &item("stone", expected_cursor));
            assert_eq!(
                state.inputs()[0].count + menu.sync().carried().count,
                original_count + 1
            );
        }
        Ok(())
    }

    #[test]
    fn unchanged_and_data_only_broadcasts_do_not_touch_shared_recipe_cache() -> Result<()> {
        struct Counting {
            lookups: usize,
        }
        impl CrafterRecipes for Counting {
            fn potential_recipe(&mut self, input: &RecipeInput) -> Result<Option<String>> {
                self.lookups += 1;
                Recipes.potential_recipe(input)
            }
            fn assemble(&mut self, id: &str, input: &RecipeInput) -> Result<ItemStack> {
                Recipes.assemble(id, input)
            }
            fn remaining_items(&mut self, id: &str, input: &RecipeInput) -> Result<Vec<ItemStack>> {
                Recipes.remaining_items(id, input)
            }
        }
        let mut recipes = Counting { lookups: 0 };
        let mut state = CrafterState::default();
        state.set_slot(4, item("oak_log", 1))?;
        let mut inv = PlayerInventory::default();
        let mut menu = CrafterMenu::new(3, &Callbacks(true, 1))?;
        menu.initialize(&state, &inv, &Callbacks(true, 1), &mut recipes)?;
        let initial = recipes.lookups;
        menu.broadcast_changes(&state, &inv, &Callbacks(true, 1), &mut recipes)?;
        assert_eq!(recipes.lookups, initial);
        menu.prepare_slot_state(
            &state,
            &inv,
            CrafterSlotStatePacket {
                slot: 0,
                container: 3,
                enabled: false,
            },
            false,
            &Callbacks(true, 1),
        )?
        .unwrap()
        .commit(&mut menu, &mut state, &mut inv, &Callbacks(true, 1))?;
        let packet = ContainerClick {
            window_id: 3,
            state_id: menu.sync().state_id(),
            slot: 0,
            button: 0,
            kind: ClickType::Pickup,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        menu.prepare_click(
            &state,
            &inv,
            &packet,
            false,
            &Callbacks(true, 1),
            &mut recipes,
        )?
        .commit(&mut menu, &mut state, &mut inv, &Callbacks(true, 1))?;
        assert_eq!(recipes.lookups, initial);
        assert!(state.is_slot_disabled(0));
        Ok(())
    }
    #[test]
    fn multi_slot_drag_requires_current_actual_callback_order_certificate() -> Result<()> {
        let mut state = CrafterState::default();
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("oak_log", 4));
        let mut menu = CrafterMenu::new(3, &Callbacks(true, 1))?;
        menu.initialize(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        click(&mut menu, &mut state, &mut inv, 9, ClickType::Pickup)?;
        custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            -999,
            0,
            ClickType::QuickCraft,
            false,
        )?;
        for slot in [1, 2] {
            custom_click(
                &mut menu,
                &mut state,
                &mut inv,
                slot,
                1,
                ClickType::QuickCraft,
                false,
            )?;
        }
        let packet = ContainerClick {
            window_id: 3,
            state_id: menu.sync().state_id(),
            slot: -999,
            button: 2,
            kind: ClickType::QuickCraft,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        assert!(menu
            .prepare_click(
                &state,
                &inv,
                &packet,
                false,
                &Callbacks(false, 1),
                &mut Recipes
            )
            .is_err());
        assert!(state.inputs().iter().all(ItemStack::is_empty));
        assert_eq!(menu.sync().carried().count, 4);
        let plan = menu.prepare_click(
            &state,
            &inv,
            &packet,
            false,
            &Callbacks(true, 1),
            &mut Recipes,
        )?;
        assert!(plan
            .validate(&menu, &state, &inv, &Callbacks(false, 1))
            .is_err());
        plan.commit(&mut menu, &mut state, &mut inv, &Callbacks(true, 1))?;
        assert_eq!(state.inputs()[1].count, 2);
        assert_eq!(state.inputs()[2].count, 2);
        assert!(menu.sync().carried().is_empty());
        Ok(())
    }
    #[test]
    fn captured_physical_owner_replacement_and_overfull_self_swap_are_rejected() -> Result<()> {
        let mut state = CrafterState::default();
        let mut inv = PlayerInventory::default();
        let mut menu = CrafterMenu::new(3, &Callbacks(true, 1))?;
        menu.initialize(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        let plan = menu.prepare_close(
            &state,
            &inv,
            MenuCloseContext::AliveConnected,
            &Callbacks(true, 1),
        )?;
        assert!(plan
            .validate(&menu, &state, &inv, &Callbacks(true, 2))
            .is_err());
        assert!(menu
            .broadcast_changes(&state, &inv, &Callbacks(true, 2), &mut Recipes)
            .is_err());
        inv.set_slot_authoritative(36, item("stone", 99));
        let before = menu.clone();
        let packet = ContainerClick {
            window_id: 3,
            state_id: menu.sync().state_id(),
            slot: 36,
            button: 0,
            kind: ClickType::Swap,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        };
        assert!(menu
            .prepare_click(
                &state,
                &inv,
                &packet,
                false,
                &Callbacks(true, 1),
                &mut Recipes
            )
            .is_err());
        assert_eq!(menu, before);
        assert_eq!(inv.slot(36).unwrap().count, 99);
        let transition = state.prepare_neighbor_signal(&Callbacks(true, 1), false, true)?;
        assert!(transition.validate(&state, &Callbacks(true, 2)).is_err());
        assert!(transition.commit(&mut state, &Callbacks(true, 2)).is_err());
        assert!(!state.triggered());
        Ok(())
    }
    #[test]
    fn actual_player_counter_callbacks_observe_inventory_and_external_cursor_phases() -> Result<()>
    {
        let mut state = CrafterState::default();
        let mut inv = PlayerInventory::default();
        inv.set_slot_authoritative(9, item("stone", 4));
        inv.set_carried_authoritative(item("emerald", 2));
        let mut menu = CrafterMenu::new(3, &Callbacks(true, 1))?;
        menu.initialize(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        let taken = click(&mut menu, &mut state, &mut inv, 9, ClickType::Pickup)?;
        let phases = taken
            .events
            .iter()
            .filter_map(|e| match e {
                CrafterMenuEvent::PlayerInventoryChanged { phase } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(phases.len(), 3);
        assert!(phases[0].carried.is_empty());
        assert_eq!(phases[1].carried.count, 4);
        assert_eq!(phases[2].carried.count, 4);
        assert!(phases.iter().all(|p| p.player_inventory.carried().item_id
            == item("emerald", 1).item_id
            && p.player_inventory.carried().count == 2
            && p.player_inventory.slot(9).unwrap().is_empty()));
        let inserted = click(&mut menu, &mut state, &mut inv, 10, ClickType::Pickup)?;
        assert_eq!(
            inserted
                .events
                .iter()
                .filter(|e| matches!(e, CrafterMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        inv.set_slot_authoritative(36, item("stone", 3));
        menu.broadcast_changes(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        let same = custom_click(
            &mut menu,
            &mut state,
            &mut inv,
            36,
            0,
            ClickType::Swap,
            false,
        )?;
        assert_eq!(
            same.events
                .iter()
                .filter(|e| matches!(e, CrafterMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        assert_eq!(inv.slot(36).unwrap().count, 3);
        inv.set_slot_authoritative(9, item("oak_log", 2));
        menu.broadcast_changes(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        let moved = click(&mut menu, &mut state, &mut inv, 9, ClickType::QuickMove)?;
        let first = moved
            .events
            .iter()
            .find_map(|e| match e {
                CrafterMenuEvent::BlockEntityChanged { phase } => Some(phase),
                _ => None,
            })
            .unwrap();
        assert_eq!(first.state.inputs()[0].count, 2);
        assert!(first.player_inventory.slot(9).unwrap().is_empty());
        assert!(first.carried.is_empty());
        assert_eq!(
            moved
                .events
                .iter()
                .filter(|e| matches!(e, CrafterMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        Ok(())
    }
    #[test]
    fn cursor_close_packet_phases_precede_remaining_overflow_drop() -> Result<()> {
        let mut state = CrafterState::default();
        state.set_slot(4, item("oak_log", 2))?;
        let mut inv = PlayerInventory::default();
        for slot in 9..45 {
            inv.set_slot_authoritative(slot, item("stone", 64));
        }
        inv.set_slot_authoritative(36, item("stone", 62));
        inv.set_slot_authoritative(37, item("stone", 63));
        inv.set_carried_authoritative(item("emerald", 2));
        let mut menu = CrafterMenu::new(3, &Callbacks(true, 1))?;
        menu.initialize(&state, &inv, &Callbacks(true, 1), &mut Recipes)?;
        menu.sync.set_carried(item("stone", 4));
        let closed = menu
            .prepare_close(
                &state,
                &inv,
                MenuCloseContext::AliveConnected,
                &Callbacks(true, 1),
            )?
            .commit(&mut menu, &mut state, &mut inv, &Callbacks(true, 1))?;
        assert_eq!(closed.events.len(), 3);
        if let CrafterMenuEvent::Sync { phase, .. } = &closed.events[0] {
            assert_eq!(phase.player_inventory.slot(36).unwrap().count, 64);
            assert_eq!(phase.player_inventory.slot(37).unwrap().count, 63);
            assert_eq!(phase.carried.count, 2);
        } else {
            panic!("first insertion missing");
        }
        if let CrafterMenuEvent::Sync { phase, .. } = &closed.events[1] {
            assert_eq!(phase.player_inventory.slot(37).unwrap().count, 64);
            assert_eq!(phase.carried.count, 1);
        } else {
            panic!("second insertion missing");
        }
        if let CrafterMenuEvent::Drop { index: 0, phase } = &closed.events[2] {
            assert_eq!(phase.carried.count, 1);
            assert_eq!(phase.player_inventory.carried().count, 2);
        } else {
            panic!("overflow drop missing");
        }
        assert_eq!(closed.drops[0].stack.count, 1);
        assert_eq!(state.inputs()[4].count, 2);
        assert!(menu.sync().carried().is_empty());
        Ok(())
    }
}
