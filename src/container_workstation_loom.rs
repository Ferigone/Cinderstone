//! Java 1.21 LoomMenu controller. The station owns its three inputs and one
//! derived preview; the real player inventory is borrowed for each transaction.
//! Main must stage actual callbacks/drops and publish this ordered event stream.
use crate::{
    container::{
        ClickOutcome, ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer,
    },
    inventory::{self, ItemStack, PlayerInventory},
    protocol::{Packet, Reader},
    workstations::{
        loom::{
            require_proved_component_identity, BannerPattern, LoomObservation, LoomPatterns,
            LoomSourceEvent, LoomState,
        },
        menu::{MenuRemovalEvent, MenuSync, SyncEvent, WorkstationAccess, WorkstationKind},
    },
};
use anyhow::{bail, ensure, Result};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};

const MENU_SLOTS: usize = 40;
const RESULT: usize = 3;

/// Implement on the captured real player/world/block/registry access. Binding
/// must include their identities, banner-pattern registry and tag revision,
/// enabled-item feature admission, and the actual callback environment. It is
/// not a menu-local counter or an invented registry. Keep the captured holder
/// order for default and template tags in LoomPatterns.
pub trait LoomMenuContext: LoomPatterns + WorkstationAccess {
    type Binding: Clone + Debug + PartialEq + Eq;
    fn binding(&self) -> Result<Self::Binding>;
    /// Certify the actual source default update listener and nonreentrant
    /// container/sync listeners. No permissive default is supplied.
    fn source_callbacks_supported(&self) -> Result<bool>;
    fn world_game_time(&self) -> Result<i64>;
    /// The actual Java Slot identity HashSet encounter order for these targets,
    /// or Err if the host cannot supply it. Never substitute sorted slot order.
    /// Required only for a completed drag containing more than one target.
    fn quickcraft_slot_order(&self, targets: &BTreeSet<usize>) -> Result<Vec<usize>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoomMenu<B: Clone + Debug + PartialEq + Eq> {
    state: LoomState,
    sync: MenuSync,
    binding: B,
    instance: u64,
    revision: u64,
    closed: bool,
    quickcraft_status: u8,
    quickcraft_type: u8,
    quickcraft_slots: BTreeSet<usize>,
}
/// Source-boundary observations. The preview and snapshots do not confer item
/// ownership. External carried is separate from retained window0 carried.
#[derive(Clone, Debug)]
pub struct LoomPhase {
    pub inputs: [ItemStack; 3],
    pub result_preview: ItemStack,
    pub selected_pattern: i32,
    pub selectable_patterns: Vec<BannerPattern>,
    pub player_inventory: PlayerInventory,
    pub carried: ItemStack,
}
#[derive(Clone, Debug)]
pub enum LoomMenuEvent {
    Source {
        event: LoomSourceEvent,
        phase: LoomPhase,
    },
    PlayerInventoryChanged {
        phase: LoomPhase,
    },
    Sync {
        event: SyncEvent,
        phase: LoomPhase,
    },
    /// Actual captured-level UI_LOOM_TAKE_RESULT, BLOCKS, volume/pitch 1.
    /// Publish at this position. Actual Player.drop owns any RNG it needs.
    TakeSound {
        game_time: i64,
        phase: LoomPhase,
    },
    /// Exactly one owned drop from outcome.drops; never publish drops twice.
    Drop {
        index: usize,
        phase: LoomPhase,
    },
}
#[derive(Debug)]
pub struct LoomPublication {
    pub events: Vec<LoomMenuEvent>,
    /// Packet events above are the only publication owner; packets is empty.
    pub outcome: ClickOutcome,
    pub accepted_button: Option<bool>,
}
/// Single-use, side-effect-free preparation. Validate against the actual owners
/// before staging, then commit after all fallible callback/drop staging succeeds.
/// Staging must not reenter or mutate this menu, inventory, or captured binding.
pub struct PreparedLoomAction<B: Clone + Debug + PartialEq + Eq> {
    before: LoomMenu<B>,
    before_inventory: PlayerInventory,
    next_menu: LoomMenu<B>,
    next_inventory: PlayerInventory,
    events: Vec<LoomMenuEvent>,
    outcome: ClickOutcome,
    accepted_button: Option<bool>,
    expected_game_time: Option<i64>,
    expected_drag_order: Option<(BTreeSet<usize>, Vec<usize>)>,
    requires_validity: bool,
}
impl<B: Clone + Debug + PartialEq + Eq> PreparedLoomAction<B> {
    pub fn events(&self) -> &[LoomMenuEvent] {
        &self.events
    }
    pub fn drops(&self) -> &[DroppedStack] {
        &self.outcome.drops
    }
    pub fn next_menu(&self) -> &LoomMenu<B> {
        &self.next_menu
    }
    pub fn next_inventory(&self) -> &PlayerInventory {
        &self.next_inventory
    }
    pub fn accepted_button(&self) -> Option<bool> {
        self.accepted_button
    }
    pub fn validate(
        &self,
        menu: &LoomMenu<B>,
        inventory: &PlayerInventory,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<()> {
        ensure!(*menu == self.before, "stale loom menu action");
        ensure!(
            same_inventory(inventory, &self.before_inventory),
            "loom canonical inventory changed during preparation"
        );
        menu.check_context(context)?;
        menu.check_stack_context(inventory)?;
        if self.requires_validity {
            ensure!(
                menu.still_valid(context)?,
                "loom block/player access changed during preparation"
            );
        }
        if let Some(time) = self.expected_game_time {
            ensure!(
                context.world_game_time()? == time,
                "loom world time changed during result preparation"
            );
        }
        if let Some((targets, order)) = &self.expected_drag_order {
            ensure!(
                context.quickcraft_slot_order(targets)? == *order,
                "loom Slot encounter order changed during preparation"
            );
        }
        Ok(())
    }
    pub fn commit(
        self,
        menu: &mut LoomMenu<B>,
        inventory: &mut PlayerInventory,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<LoomPublication> {
        self.validate(menu, inventory, context)?;
        *menu = self.next_menu;
        *inventory = self.next_inventory;
        Ok(LoomPublication {
            events: self.events,
            outcome: self.outcome,
            accepted_button: self.accepted_button,
        })
    }
}
impl<B: Clone + Debug + PartialEq + Eq> LoomMenu<B> {
    pub fn new(window_id: u8, context: &impl LoomMenuContext<Binding = B>) -> Result<Self> {
        ensure!(
            context.source_callbacks_supported()?,
            "unsupported reentrant/custom loom callback context"
        );
        ensure!(
            context.block_matches(WorkstationKind::Loom)?
                && context.player_can_interact_with_block(4.0)?,
            "invalid loom opening access"
        );
        static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
        let mut seen = NEXT_INSTANCE.load(Ordering::Relaxed);
        let instance = loop {
            let next = seen
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("loom UI identity exhausted"))?;
            match NEXT_INSTANCE.compare_exchange_weak(
                seen,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break seen,
                Err(actual) => seen = actual,
            }
        };
        Ok(Self {
            state: LoomState::default(),
            sync: MenuSync::new(window_id, WorkstationKind::Loom)?,
            binding: context.binding()?,
            instance,
            revision: 0,
            closed: false,
            quickcraft_status: 0,
            quickcraft_type: 0,
            quickcraft_slots: BTreeSet::new(),
        })
    }
    pub fn state(&self) -> &LoomState {
        &self.state
    }
    pub fn carried(&self) -> &ItemStack {
        self.sync.carried()
    }
    pub fn window_id(&self) -> u8 {
        self.sync.window_id()
    }
    pub fn state_id(&self) -> i32 {
        self.sync.state_id()
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn still_valid(&self, access: &impl WorkstationAccess) -> Result<bool> {
        self.sync.still_valid(access)
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    fn check_context(&self, context: &impl LoomMenuContext<Binding = B>) -> Result<()> {
        ensure!(!self.closed, "loom menu is closed");
        ensure!(
            self.binding == context.binding()?,
            "loom captured owner/registry/tag/feature binding changed"
        );
        ensure!(
            context.source_callbacks_supported()?,
            "unsupported reentrant/custom loom callback context"
        );
        Ok(())
    }
    fn check_stack_context(&self, inventory: &PlayerInventory) -> Result<()> {
        for stack in self.top().iter().chain(std::iter::once(self.carried())) {
            require_proved_component_identity(stack)?;
        }
        // Ordinary menu sync compares main/hotbar; SWAP and return/add can also
        // use actual offhand. Retained window0 carried is a separate owner.
        for slot in 9..46 {
            require_proved_component_identity(
                inventory.slot(slot).expect("canonical loom inventory"),
            )?;
        }
        Ok(())
    }
    fn top(&self) -> [ItemStack; 4] {
        let s = self.state.inputs();
        [
            s[0].clone(),
            s[1].clone(),
            s[2].clone(),
            self.state.result().clone(),
        ]
    }
    fn data(&self) -> [i32; 1] {
        [self.state.selected_pattern()]
    }
    fn phase(&self, inventory: &PlayerInventory) -> LoomPhase {
        LoomPhase {
            inputs: self.state.inputs().clone(),
            result_preview: self.state.result().clone(),
            selected_pattern: self.state.selected_pattern(),
            selectable_patterns: self.state.selectable_patterns().to_vec(),
            player_inventory: inventory.clone(),
            carried: self.carried().clone(),
        }
    }
    fn advance_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("loom menu revision exhausted"))?;
        Ok(())
    }
    fn reset_quickcraft(&mut self) {
        self.quickcraft_status = 0;
        self.quickcraft_slots.clear();
    }
    pub fn initialize(
        &mut self,
        inventory: &PlayerInventory,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<Vec<LoomMenuEvent>> {
        self.check_context(context)?;
        self.check_stack_context(inventory)?;
        let mut next = self.clone();
        let events = next.sync.initialize(&next.top(), inventory, &next.data())?;
        next.advance_revision()?;
        *self = next;
        let phase = self.phase(inventory);
        Ok(events
            .into_iter()
            .map(|event| LoomMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect())
    }
    pub fn broadcast_changes(
        &mut self,
        inventory: &PlayerInventory,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<Vec<LoomMenuEvent>> {
        self.check_context(context)?;
        self.check_stack_context(inventory)?;
        let mut next = self.clone();
        let events = next
            .sync
            .broadcast_changes(&next.top(), inventory, &next.data())?;
        next.advance_revision()?;
        *self = next;
        let phase = self.phase(inventory);
        Ok(events
            .into_iter()
            .map(|event| LoomMenuEvent::Sync {
                event,
                phase: phase.clone(),
            })
            .collect())
    }
    pub fn broadcast_full_state(
        &mut self,
        inventory: &PlayerInventory,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<Vec<SyncEvent>> {
        self.check_context(context)?;
        self.check_stack_context(inventory)?;
        let mut next = self.clone();
        let out = next
            .sync
            .broadcast_full_state(&next.top(), inventory, &next.data())?;
        next.advance_revision()?;
        *self = next;
        Ok(out)
    }
    fn preparation(
        &self,
        inventory: &PlayerInventory,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<PreparedLoomAction<B>> {
        self.check_context(context)?;
        self.check_stack_context(inventory)?;
        Ok(PreparedLoomAction {
            before: self.clone(),
            before_inventory: inventory.clone(),
            next_menu: self.clone(),
            next_inventory: inventory.clone(),
            events: vec![],
            outcome: ClickOutcome::default(),
            accepted_button: None,
            expected_game_time: None,
            expected_drag_order: None,
            requires_validity: false,
        })
    }
    /// Includes the real ServerGamePacketListener accepted-button broadcast.
    pub fn prepare_button(
        &self,
        index: i32,
        inventory: &PlayerInventory,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<PreparedLoomAction<B>> {
        ensure!(self.still_valid(context)?, "invalid loom button access");
        let mut action = self.preparation(inventory, context)?;
        action.requires_validity = true;
        let (accepted, observations) = action.next_menu.state.select_pattern_observed(index)?;
        action.observe(observations)?;
        action.accepted_button = Some(accepted);
        if accepted {
            action.broadcast()?;
        }
        action.next_menu.advance_revision()?;
        Ok(action)
    }
    pub fn prepare_click(
        &self,
        click: &ContainerClick,
        inventory: &PlayerInventory,
        context: &impl LoomMenuContext<Binding = B>,
        creative: bool,
    ) -> Result<PreparedLoomAction<B>> {
        ensure!(self.still_valid(context)?, "invalid loom click access");
        ensure!(click.window_id == self.window_id(), "wrong loom window");
        ensure!(click.slot < MENU_SLOTS as i16, "invalid loom slot");
        require_proved_component_identity(&click.carried)?;
        for stack in click.changed_slots.values() {
            require_proved_component_identity(stack)?;
        }
        let mut action = self.preparation(inventory, context)?;
        action.requires_validity = true;
        let before = self.state_id();
        action.next_menu.sync.suppress_remote_updates(true);
        action.do_click(click, context, creative)?;
        let events = action.next_menu.sync.finish_click(
            click,
            before,
            &action.next_menu.top(),
            &action.next_inventory,
            &action.next_menu.data(),
        )?;
        action.push_sync(events);
        action.next_menu.advance_revision()?;
        Ok(action)
    }
    pub fn prepare_close(
        &self,
        inventory: &PlayerInventory,
        context: &impl LoomMenuContext<Binding = B>,
        close: MenuCloseContext,
    ) -> Result<PreparedLoomAction<B>> {
        let mut action = self.preparation(inventory, context)?;
        action.next_menu.reset_quickcraft();
        let owned = self.state.inputs().clone();
        let removal = action.next_menu.sync.close_inputs_ordered(
            &mut owned.clone(),
            &mut action.next_inventory,
            close,
        )?;
        action.outcome = removal.outcome;
        // cpu.removed returns the cursor first. clearContainer then removes
        // each input with removeItemNoUpdate. Loom does not remove the preview.
        let mut phase = self.phase(inventory);
        let mut events = removal.events.into_iter();
        for owner in 0..4 {
            let cursor = owner == 0;
            let mut returning = if cursor {
                phase.carried.clone()
            } else {
                let stack = phase.inputs[owner - 1].clone();
                phase.inputs[owner - 1] = ItemStack::empty();
                stack
            };
            while !returning.is_empty() {
                let event = events
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing loom close return phase"))?;
                match event {
                    MenuRemovalEvent::Packet { index } => {
                        let packet = action.outcome.packets[index].clone();
                        phase.observe_return_packet(&packet, &mut returning)?;
                        if cursor {
                            phase.carried = returning.clone();
                        }
                        action.events.push(LoomMenuEvent::Sync {
                            event: SyncEvent::Packet(packet),
                            phase: phase.clone(),
                        });
                    }
                    MenuRemovalEvent::Drop { index } => {
                        ensure!(
                            action.outcome.drops[index].stack == returning,
                            "loom close drop owner changed"
                        );
                        action.events.push(LoomMenuEvent::Drop {
                            index,
                            phase: phase.clone(),
                        });
                        returning = ItemStack::empty();
                    }
                }
            }
            if cursor {
                phase.carried = ItemStack::empty();
            }
        }
        ensure!(events.next().is_none(), "extra loom close return event");
        action.outcome.packets.clear();
        for slot in 0..3 {
            action
                .next_menu
                .state
                .replace_input_raw(slot, ItemStack::empty())?;
        }
        action.next_menu.closed = true;
        action.next_menu.advance_revision()?;
        Ok(action)
    }
}
impl LoomPhase {
    fn observe_return_packet(&mut self, packet: &Packet, returning: &mut ItemStack) -> Result<()> {
        ensure!(packet.id == 0x15, "unsupported loom close packet phase");
        let mut reader = Reader::new(&packet.payload);
        ensure!(
            reader.i8()? == -2 && reader.varint()? == 0,
            "loom close requires raw Inventory slot packet"
        );
        let raw = reader.i16()?;
        let slot = match raw {
            0..=8 => 36 + raw as usize,
            9..=35 => raw as usize,
            40 => 45,
            _ => bail!("unsupported loom return slot"),
        };
        let stack = ItemStack::decode_exact(reader.remaining())?;
        let old = self.player_inventory.slot(slot).unwrap();
        ensure!(
            stack.same_item_same_components(returning)?,
            "loom return changed item identity"
        );
        let previous = if old.is_empty() {
            0
        } else {
            ensure!(
                old.same_item_same_components(&stack)?,
                "loom return overwrote item"
            );
            old.count
        };
        let added = stack.count.wrapping_sub(previous);
        ensure!(
            added > 0 && added <= returning.count,
            "invalid loom return amount"
        );
        self.player_inventory.set_slot_authoritative(slot, stack);
        *returning = counted(returning, returning.count.wrapping_sub(added));
        Ok(())
    }
}
impl<B: Clone + Debug + PartialEq + Eq> PreparedLoomAction<B> {
    fn phase(&self) -> LoomPhase {
        self.next_menu.phase(&self.next_inventory)
    }
    fn push_sync(&mut self, events: Vec<SyncEvent>) {
        let phase = self.phase();
        self.events
            .extend(events.into_iter().map(|event| LoomMenuEvent::Sync {
                event,
                phase: phase.clone(),
            }));
    }
    fn broadcast(&mut self) -> Result<()> {
        let events = self.next_menu.sync.broadcast_changes(
            &self.next_menu.top(),
            &self.next_inventory,
            &self.next_menu.data(),
        )?;
        self.push_sync(events);
        Ok(())
    }
    fn observe(&mut self, observations: Vec<LoomObservation>) -> Result<()> {
        let final_state = self.next_menu.state.clone();
        for observation in observations {
            self.next_menu.state = observation.state;
            self.events.push(LoomMenuEvent::Source {
                event: observation.event,
                phase: self.phase(),
            });
            if observation.event == LoomSourceEvent::BroadcastChanges {
                self.broadcast()?;
            }
        }
        self.next_menu.state = final_state;
        Ok(())
    }
    fn changed(&mut self, slot: usize, context: &impl LoomMenuContext<Binding = B>) -> Result<()> {
        if slot < RESULT {
            let observations = self.next_menu.state.input_changed_observed(context)?;
            self.observe(observations)?;
        } else if slot == RESULT {
            self.observe(self.next_menu.state.output_changed_observed())?;
        } else {
            self.events.push(LoomMenuEvent::PlayerInventoryChanged {
                phase: self.phase(),
            });
        }
        Ok(())
    }
    fn slot(&self, slot: usize) -> ItemStack {
        match slot {
            0..=2 => self.next_menu.state.inputs()[slot].clone(),
            3 => self.next_menu.state.result().clone(),
            _ => self
                .next_inventory
                .slot(slot + 5)
                .expect("validated loom player slot")
                .clone(),
        }
    }
    fn may_place(
        &self,
        slot: usize,
        stack: &ItemStack,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> bool {
        slot >= 4 || LoomState::may_place(slot, stack, context)
    }
    fn mutate_slot(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        let stack = canonical(&stack);
        match slot {
            0..=2 => self.next_menu.state.replace_input_raw(slot, stack)?,
            3 => self.next_menu.state.replace_result_raw(stack)?,
            _ => self.next_inventory.set_slot_authoritative(slot + 5, stack),
        };
        Ok(())
    }
    fn set_slot(
        &mut self,
        slot: usize,
        stack: ItemStack,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<()> {
        self.mutate_slot(slot, stack)?;
        // SimpleContainer.setItem invokes setChanged; Inventory.setItem does not.
        if slot < 4 {
            self.changed(slot, context)?;
        }
        self.changed(slot, context)
    }
    fn try_remove_slot(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<ItemStack> {
        let source = self.slot(slot);
        if source.is_empty()
            || maximum <= 0
            || !self.may_place(slot, &source, context) && maximum < source.count
        {
            return Ok(ItemStack::empty());
        }
        let amount = amount.max(0).min(maximum).min(source.count);
        if amount == 0 {
            return Ok(ItemStack::empty());
        }
        let taken = counted(&source, amount);
        self.mutate_slot(slot, counted(&source, source.count.wrapping_sub(amount)))?;
        if slot < 4 {
            self.changed(slot, context)?;
        }
        if self.slot(slot).is_empty() {
            self.set_slot(slot, ItemStack::empty(), context)?;
        }
        Ok(taken)
    }
    fn on_take(&mut self, slot: usize, context: &impl LoomMenuContext<Binding = B>) -> Result<()> {
        if slot == RESULT {
            for input in 0..2 {
                let source = self.slot(input);
                if !source.is_empty() {
                    self.mutate_slot(input, counted(&source, source.count.wrapping_sub(1)))?;
                    // crq.remove delegates to SimpleContainer.removeItem only;
                    // it does not run tryRemove's empty setByPlayer path.
                    self.changed(input, context)?;
                }
            }
            let time = context.world_game_time()?;
            if let Some(previous) = self.expected_game_time {
                ensure!(time == previous, "loom time changed within preparation");
            }
            self.expected_game_time = Some(time);
            if self.next_menu.state.finish_take(time) {
                self.events.push(LoomMenuEvent::TakeSound {
                    game_time: time,
                    phase: self.phase(),
                });
            }
        }
        self.changed(slot, context)
    }
    fn take_slot(
        &mut self,
        slot: usize,
        amount: i32,
        maximum: i32,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<ItemStack> {
        let taken = self.try_remove_slot(slot, amount, maximum, context)?;
        if !taken.is_empty() {
            self.on_take(slot, context)?;
        }
        Ok(taken)
    }
    fn do_click(
        &mut self,
        click: &ContainerClick,
        context: &impl LoomMenuContext<Binding = B>,
        creative: bool,
    ) -> Result<()> {
        let slot = click.slot as i32;
        let button = click.button as i32;
        if click.kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, context, creative);
        }
        if self.next_menu.quickcraft_status != 0 {
            self.next_menu.reset_quickcraft();
            return Ok(());
        }
        match click.kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let source = self.next_menu.sync.carried().clone();
                    let amount = if button == 0 {
                        source.count
                    } else {
                        1.min(source.count)
                    };
                    if button == 1 {
                        self.next_menu
                            .sync
                            .set_carried(counted(&source, source.count.wrapping_sub(amount)));
                        self.drop_stack(counted(&source, amount));
                    } else {
                        self.drop_stack(counted(&source, amount));
                        self.next_menu.sync.set_carried(ItemStack::empty());
                    }
                } else if slot >= 0 {
                    let slot = slot as usize;
                    if click.kind == ClickType::QuickMove {
                        loop {
                            let before = self.slot(slot);
                            if !self.quick_move_once(slot, context)?
                                || self.slot(slot).item_id != before.item_id
                            {
                                break;
                            }
                        }
                    } else {
                        self.pickup(slot, button, context)?;
                    }
                }
            }
            ClickType::Swap if ((0..9).contains(&button) || button == 40) && slot >= 0 => {
                self.swap(slot as usize, button, context, creative)?;
            }
            ClickType::PickupAll if slot >= 0 => {
                self.pickup_all(slot as usize, button, context)?;
            }
            ClickType::Throw if slot >= 0 && self.next_menu.sync.carried().is_empty() => {
                let amount = if button == 0 {
                    1
                } else {
                    self.slot(slot as usize).count
                };
                let taken = self.take_slot(slot as usize, amount, i32::MAX, context)?;
                self.drop_stack(taken);
            }
            ClickType::Clone
                if creative && slot >= 0 && self.next_menu.sync.carried().is_empty() =>
            {
                let source = self.slot(slot as usize);
                self.next_menu
                    .sync
                    .set_carried(counted(&source, source.max_stack_size()?));
            }
            _ => {}
        }
        Ok(())
    }
    fn pickup(
        &mut self,
        slot: usize,
        button: i32,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<()> {
        let source = self.slot(slot);
        let carried = self.next_menu.carried().clone();
        reject_bundle(&source)?;
        reject_bundle(&carried)?;
        if source.is_empty() {
            if !carried.is_empty() && self.may_place(slot, &carried, context) {
                let amount = (if button == 0 { carried.count } else { 1 }).min(limit(&carried)?);
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count - amount));
                self.set_slot(slot, counted(&carried, amount), context)?;
            }
        } else if carried.is_empty() {
            let amount = if button == 0 {
                source.count
            } else {
                (source.count + 1) / 2
            };
            let taken = self.try_remove_slot(slot, amount, i32::MAX, context)?;
            if !taken.is_empty() {
                // cpu pickup lambda installs the cursor before Slot.onTake.
                self.next_menu.sync.set_carried(taken);
                self.on_take(slot, context)?;
            }
        } else if self.may_place(slot, &carried, context) {
            if source.same_item_same_components(&carried)? {
                // Slot.safeInsert preserves a negative capacity for an actual
                // overfull target: shrink(-n) grows the cursor, grow(-n) lowers
                // the slot. Even zero transfer still invokes setByPlayer.
                let amount = (if button == 0 { carried.count } else { 1 })
                    .min(limit(&carried)?.wrapping_sub(source.count));
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count.wrapping_sub(amount)));
                self.set_slot(
                    slot,
                    counted(&source, source.count.wrapping_add(amount)),
                    context,
                )?;
            } else if carried.count <= limit(&carried)? {
                self.next_menu.sync.set_carried(source);
                self.set_slot(slot, carried, context)?;
            }
        } else if source.same_item_same_components(&carried)? {
            let taken = self.try_remove_slot(
                slot,
                source.count,
                carried.max_stack_size()? - carried.count,
                context,
            )?;
            if !taken.is_empty() {
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count + taken.count));
                self.on_take(slot, context)?;
            }
        }
        // AbstractContainerMenu.doClick calls Slot.setChanged after pickup,
        // even if the insertion was refused or the occupied slot was full.
        self.changed(slot, context)?;
        Ok(())
    }
    fn move_to(
        &mut self,
        stack: &mut ItemStack,
        start: usize,
        end: usize,
        reverse: bool,
        source_slot: usize,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<bool> {
        let mut moved = false;
        let order: Vec<usize> = if reverse {
            (start..end).rev().collect()
        } else {
            (start..end).collect()
        };
        if stack.is_stackable()? {
            for &slot in &order {
                let existing = self.slot(slot);
                // cpu's merge pass does not ask mayPlace. Ranges are chosen by
                // the actual menu's item-class dispatch, not a fallback route.
                if !existing.is_empty() && existing.same_item_same_components(stack)? {
                    let total = existing.count.wrapping_add(stack.count);
                    let maximum = limit(&existing)?;
                    if total <= maximum {
                        *stack = ItemStack::empty();
                        self.mutate_slot(source_slot, stack.clone())?;
                        self.mutate_slot(slot, counted(&existing, total))?;
                        self.changed(slot, context)?;
                        moved = true;
                    } else if existing.count < maximum {
                        *stack = counted(
                            stack,
                            stack
                                .count
                                .wrapping_sub(maximum.wrapping_sub(existing.count)),
                        );
                        self.mutate_slot(source_slot, stack.clone())?;
                        self.mutate_slot(slot, counted(&existing, maximum))?;
                        self.changed(slot, context)?;
                        moved = true;
                    }
                    if stack.is_empty() {
                        break;
                    }
                }
            }
        }
        if !stack.is_empty() {
            for slot in order {
                if self.slot(slot).is_empty() && self.may_place(slot, stack, context) {
                    let amount = stack.count.min(limit(stack)?);
                    let inserted = counted(stack, amount);
                    *stack = counted(stack, stack.count.wrapping_sub(amount));
                    self.mutate_slot(source_slot, stack.clone())?;
                    self.set_slot(slot, inserted, context)?;
                    self.changed(slot, context)?; // cpu.moveItemStackTo extra dirty
                    moved = true;
                    break;
                }
            }
        }
        Ok(moved)
    }
    fn quick_move_once(
        &mut self,
        slot: usize,
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<bool> {
        let source = self.slot(slot);
        if source.is_empty() {
            return Ok(false);
        }
        let Some((range, reverse)) = LoomState::quick_move_target(slot, &source, context) else {
            return Ok(false);
        };
        let mut left = source.clone();
        if !self.move_to(&mut left, range.start, range.end, reverse, slot, context)? {
            return Ok(false);
        }
        // Loom's onQuickCraft is the base empty callback. No Item.onCraftedBy.
        if left.is_empty() {
            self.set_slot(slot, ItemStack::empty(), context)?;
        } else {
            self.changed(slot, context)?;
        }
        if left.count == source.count {
            return Ok(false);
        }
        self.on_take(slot, context)?;
        Ok(true)
    }
    fn swap(
        &mut self,
        slot: usize,
        button: i32,
        context: &impl LoomMenuContext<Binding = B>,
        creative: bool,
    ) -> Result<()> {
        let player_slot = if button == 40 {
            45
        } else {
            36 + button as usize
        };
        let aliases_held = slot >= 4 && slot + 5 == player_slot;
        let source = self.slot(slot);
        let held = self.next_inventory.slot(player_slot).unwrap().clone();
        reject_bundle(&source)?;
        reject_bundle(&held)?;
        if source.is_empty() && held.is_empty() {
            return Ok(());
        }
        if held.is_empty() {
            self.next_inventory
                .set_slot_authoritative(player_slot, source);
            self.set_slot(slot, ItemStack::empty(), context)?;
            self.on_take(slot, context)?;
        } else if source.is_empty() {
            if self.may_place(slot, &held, context) {
                let amount = held.count.min(limit(&held)?);
                self.next_inventory.set_slot_authoritative(
                    player_slot,
                    counted(&held, held.count.wrapping_sub(amount)),
                );
                self.set_slot(slot, counted(&held, amount), context)?;
            }
        } else if self.may_place(slot, &held, context) {
            let maximum = limit(&held)?;
            if held.count > maximum {
                let remainder = held.count.wrapping_sub(maximum);
                self.next_inventory
                    .set_slot_authoritative(player_slot, counted(&held, remainder));
                self.set_slot(slot, counted(&held, maximum), context)?;
                self.on_take(slot, context)?;
                let mut displaced = if aliases_held {
                    counted(&source, remainder)
                } else {
                    source
                };
                self.add_to_inventory(&mut displaced, creative)?;
                self.drop_stack(displaced);
            } else {
                self.next_inventory
                    .set_slot_authoritative(player_slot, source);
                self.set_slot(slot, held, context)?;
                self.on_take(slot, context)?;
            }
        }
        Ok(())
    }
    fn add_to_inventory(&mut self, stack: &mut ItemStack, creative: bool) -> Result<()> {
        let order = (36..45).chain(9..36).collect::<Vec<_>>();
        let damaged = stack.is_damageable()? && stack.damage_value()? > 0;
        if damaged {
            if let Some(slot) = order
                .iter()
                .copied()
                .find(|&slot| self.next_inventory.slot(slot).unwrap().is_empty())
            {
                self.next_inventory
                    .set_slot_authoritative(slot, std::mem::take(stack));
            } else if creative {
                *stack = ItemStack::empty();
            }
            return Ok(());
        }
        let mut merges = vec![36 + self.next_inventory.selected_hotbar(), 45];
        merges.extend(order.iter().copied());
        for slot in merges {
            let existing = self.next_inventory.slot(slot).unwrap().clone();
            if !existing.is_empty()
                && existing.is_stackable()?
                && existing.same_item_same_components(stack)?
            {
                let amount = stack.count.min((limit(stack)? - existing.count).max(0));
                if amount > 0 {
                    self.next_inventory
                        .set_slot_authoritative(slot, counted(&existing, existing.count + amount));
                    *stack = counted(stack, stack.count - amount);
                }
                if stack.is_empty() {
                    return Ok(());
                }
            }
        }
        for slot in order {
            if self.next_inventory.slot(slot).unwrap().is_empty() {
                let amount = stack.count.min(limit(stack)?);
                self.next_inventory
                    .set_slot_authoritative(slot, counted(stack, amount));
                *stack = counted(stack, stack.count - amount);
                if stack.is_empty() {
                    break;
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
        context: &impl LoomMenuContext<Binding = B>,
    ) -> Result<()> {
        if self.next_menu.carried().is_empty() || !self.slot(clicked).is_empty() {
            return Ok(());
        }
        let order: Vec<usize> = if button == 0 {
            (0..MENU_SLOTS).collect()
        } else {
            (0..MENU_SLOTS).rev().collect()
        };
        for pass in 0..2 {
            for &slot in &order {
                let source = self.slot(slot);
                let carried = self.next_menu.carried().clone();
                if carried.count >= carried.max_stack_size()? {
                    break;
                }
                if source.is_empty()
                    || !source.same_item_same_components(&carried)?
                    || source.count > carried.max_stack_size()?
                    || pass == 0 && source.count == source.max_stack_size()?
                {
                    continue;
                }
                reject_bundle(&source)?;
                // Loom does not override canTakeItemForPickAll. Result removal
                // therefore uses its real onTake, rather than duplicating it.
                let taken = self.take_slot(
                    slot,
                    source.count,
                    carried.max_stack_size()?.wrapping_sub(carried.count),
                    context,
                )?;
                self.next_menu
                    .sync
                    .set_carried(counted(&carried, carried.count.wrapping_add(taken.count)));
            }
        }
        Ok(())
    }
    fn quickcraft(
        &mut self,
        slot: i32,
        button: i32,
        context: &impl LoomMenuContext<Binding = B>,
        creative: bool,
    ) -> Result<()> {
        let previous = self.next_menu.quickcraft_status;
        self.next_menu.quickcraft_status = (button & 3) as u8;
        if !(previous == 1 && self.next_menu.quickcraft_status == 2)
            && previous != self.next_menu.quickcraft_status
        {
            self.next_menu.reset_quickcraft();
            return Ok(());
        }
        if self.next_menu.carried().is_empty() {
            self.next_menu.reset_quickcraft();
            return Ok(());
        }
        reject_bundle(self.next_menu.carried())?;
        match self.next_menu.quickcraft_status {
            0 => {
                self.next_menu.quickcraft_type = ((button >> 2) & 3) as u8;
                if self.next_menu.quickcraft_type <= 1
                    || self.next_menu.quickcraft_type == 2 && creative
                {
                    self.next_menu.quickcraft_status = 1;
                    self.next_menu.quickcraft_slots.clear();
                } else {
                    self.next_menu.reset_quickcraft();
                }
            }
            1 => {
                if slot >= 0 {
                    let slot = slot as usize;
                    let source = self.slot(slot);
                    let carried = self.next_menu.carried();
                    if self.may_place(slot, carried, context)
                        && (source.is_empty()
                            || source.same_item_same_components(carried)?
                                && source.count <= carried.max_stack_size()?)
                        && (self.next_menu.quickcraft_type == 2
                            || carried.count > self.next_menu.quickcraft_slots.len() as i32)
                    {
                        self.next_menu.quickcraft_slots.insert(slot);
                    }
                }
            }
            2 => {
                if self.next_menu.quickcraft_slots.len() == 1 {
                    let slot = *self.next_menu.quickcraft_slots.iter().next().unwrap();
                    let button = self.next_menu.quickcraft_type as i8;
                    self.next_menu.reset_quickcraft();
                    return if button <= 1 {
                        self.pickup(slot, button as i32, context)
                    } else {
                        Ok(())
                    };
                }
                let targets = self.next_menu.quickcraft_slots.clone();
                let slots = if targets.len() > 1 {
                    let slots = context.quickcraft_slot_order(&targets)?;
                    ensure!(
                        slots.len() == targets.len()
                            && slots.iter().copied().collect::<BTreeSet<_>>() == targets,
                        "invalid actual loom Slot encounter order"
                    );
                    self.expected_drag_order = Some((targets.clone(), slots.clone()));
                    slots
                } else {
                    vec![]
                };
                let carried = self.next_menu.carried().clone();
                let mut left = carried.count;
                let each = match self.next_menu.quickcraft_type {
                    0 => ((carried.count as f32) / (slots.len().max(1) as f32)).floor() as i32,
                    1 => 1,
                    2 => carried.max_stack_size()?,
                    _ => 0,
                };
                for slot in slots {
                    let source = self.slot(slot);
                    if !self.may_place(slot, &carried, context)
                        || !source.is_empty()
                            && (!source.same_item_same_components(&carried)?
                                || source.count > carried.max_stack_size()?)
                        || self.next_menu.quickcraft_type != 2
                            && carried.count < targets.len() as i32
                    {
                        continue;
                    }
                    let count = each.wrapping_add(source.count).min(limit(&carried)?);
                    left = left.wrapping_sub(count.wrapping_sub(source.count));
                    self.set_slot(slot, counted(&carried, count), context)?;
                }
                // cpu keeps the original cursor during all drag slot callbacks.
                self.next_menu.sync.set_carried(counted(&carried, left));
                self.next_menu.reset_quickcraft();
            }
            _ => self.next_menu.reset_quickcraft(),
        }
        Ok(())
    }
    fn drop_stack(&mut self, stack: ItemStack) {
        if !stack.is_empty() {
            let index = self.outcome.drops.len();
            self.outcome.drops.push(DroppedStack {
                stack,
                around: false,
                trace_item: true,
            });
            self.events.push(LoomMenuEvent::Drop {
                index,
                phase: self.phase(),
            });
        }
    }
}
fn canonical(stack: &ItemStack) -> ItemStack {
    if stack.is_empty() {
        ItemStack::empty()
    } else {
        stack.clone()
    }
}
fn counted(stack: &ItemStack, count: i32) -> ItemStack {
    if count <= 0 || stack.is_empty() {
        ItemStack::empty()
    } else {
        let mut stack = stack.clone();
        stack.count = count;
        stack
    }
}
fn limit(stack: &ItemStack) -> Result<i32> {
    let maximum = stack.max_stack_size()?.min(99);
    ensure!((1..=99).contains(&maximum), "invalid loom slot stack limit");
    Ok(maximum)
}
fn reject_bundle(stack: &ItemStack) -> Result<()> {
    ensure!(
        inventory::item_name(stack.item_id) != Some("bundle"),
        "bundle click overrides require the actual item owner"
    );
    Ok(())
}
fn same_inventory(a: &PlayerInventory, b: &PlayerInventory) -> bool {
    a.slots() == b.slots()
        && a.carried() == b.carried()
        && a.selected_hotbar() == b.selected_hotbar()
        && a.state_id() == b.state_id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::RecipeBook;
    use crate::inventory::Component;
    // Explicit native fixture owners. These controlled pattern/tag lists are
    // not a production default registry or an original-Java execution oracle.
    struct Context {
        key: u64,
        time: i64,
        valid: bool,
        supported: bool,
        order: Option<Vec<usize>>,
    }
    impl Default for Context {
        fn default() -> Self {
            Self {
                key: 7,
                time: 12,
                valid: true,
                supported: true,
                order: None,
            }
        }
    }
    impl LoomPatterns for Context {
        fn is_pattern_item(&self, stack: &ItemStack) -> bool {
            inventory::item_name(stack.item_id) == Some("flower_banner_pattern")
        }
        fn selectable_patterns(&self, template: &ItemStack) -> Result<Vec<BannerPattern>> {
            if template.is_empty() {
                Ok(vec![
                    BannerPattern { registry_id: 0 },
                    BannerPattern { registry_id: 1 },
                ])
            } else if self.is_pattern_item(template) {
                Ok(vec![BannerPattern { registry_id: 2 }])
            } else {
                bail!("fixture does not own this template")
            }
        }
    }
    impl WorkstationAccess for Context {
        fn block_matches(&self, kind: WorkstationKind) -> Result<bool> {
            Ok(self.valid && kind == WorkstationKind::Loom)
        }
        fn player_can_interact_with_block(&self, range: f64) -> Result<bool> {
            ensure!(range == 4.0, "source range changed");
            Ok(self.valid)
        }
    }
    impl LoomMenuContext for Context {
        type Binding = u64;
        fn binding(&self) -> Result<u64> {
            Ok(self.key)
        }
        fn source_callbacks_supported(&self) -> Result<bool> {
            Ok(self.supported)
        }
        fn world_game_time(&self) -> Result<i64> {
            Ok(self.time)
        }
        fn quickcraft_slot_order(&self, targets: &BTreeSet<usize>) -> Result<Vec<usize>> {
            self.order
                .clone()
                .filter(|order| order.iter().all(|slot| targets.contains(slot)))
                .ok_or_else(|| anyhow::anyhow!("actual Slot order unavailable"))
        }
    }
    fn stack(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn click(menu: &LoomMenu<u64>, slot: i16, button: i8, kind: ClickType) -> ContainerClick {
        ContainerClick {
            window_id: menu.window_id(),
            state_id: menu.state_id(),
            slot,
            button,
            kind,
            changed_slots: Default::default(),
            carried: ItemStack::empty(),
        }
    }
    fn apply(
        menu: &mut LoomMenu<u64>,
        inventory: &mut PlayerInventory,
        context: &Context,
        slot: i16,
        button: i8,
        kind: ClickType,
    ) -> Result<LoomPublication> {
        menu.prepare_click(&click(menu, slot, button, kind), inventory, context, false)?
            .commit(menu, inventory, context)
    }
    fn ready(
        context: &Context,
        banners: i32,
        dyes: i32,
        template: bool,
    ) -> Result<(LoomMenu<u64>, PlayerInventory)> {
        let mut menu = LoomMenu::new(1, context)?;
        let mut inventory = PlayerInventory::new();
        inventory.set_slot_authoritative(9, stack("white_banner", banners));
        inventory.set_slot_authoritative(10, stack("red_dye", dyes));
        if template {
            inventory.set_slot_authoritative(11, stack("flower_banner_pattern", 1));
        }
        menu.initialize(&inventory, context)?;
        apply(
            &mut menu,
            &mut inventory,
            context,
            4,
            0,
            ClickType::QuickMove,
        )?;
        apply(
            &mut menu,
            &mut inventory,
            context,
            5,
            0,
            ClickType::QuickMove,
        )?;
        if template {
            apply(
                &mut menu,
                &mut inventory,
                context,
                6,
                0,
                ClickType::QuickMove,
            )?;
        } else {
            menu.prepare_button(1, &inventory, context)?.commit(
                &mut menu,
                &mut inventory,
                context,
            )?;
        }
        Ok((menu, inventory))
    }
    #[test]
    fn native_shift_results_template_ownership_and_close_once() -> Result<()> {
        let context = Context::default();
        let (mut menu, mut inventory) = ready(&context, 3, 3, true)?;
        let retained = stack("diamond", 2);
        inventory.set_carried_authoritative(retained.clone());
        let out = apply(
            &mut menu,
            &mut inventory,
            &context,
            3,
            0,
            ClickType::QuickMove,
        )?;
        assert_eq!(inventory.slot(44).unwrap().count, 3);
        assert_eq!(
            inventory.slot(44).unwrap().component_value(49)?.unwrap()["layers"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(menu.state.inputs()[0].is_empty() && menu.state.inputs()[1].is_empty());
        assert_eq!(menu.state.inputs()[2], stack("flower_banner_pattern", 1));
        assert!(menu.state.result().is_empty());
        assert_eq!(inventory.carried(), &retained);
        assert_eq!(
            out.events
                .iter()
                .filter(|event| matches!(event, LoomMenuEvent::TakeSound { .. }))
                .count(),
            1
        );
        assert!(
            out.outcome.packets.is_empty()
                && out.outcome.drops.is_empty()
                && out.outcome.crafted.is_empty()
        );
        // Old plans cannot be applied to a new UI with identical window/binding.
        let action = menu.prepare_close(&inventory, &context, MenuCloseContext::AliveConnected)?;
        let clone_before = menu.clone();
        action.commit(&mut menu, &mut inventory, &context)?;
        assert!(menu.is_closed());
        assert_eq!(
            inventory.slot(36).unwrap(),
            &stack("flower_banner_pattern", 1)
        );
        assert!(menu
            .prepare_close(&inventory, &context, MenuCloseContext::AliveConnected)
            .is_err());
        let mut reopened = LoomMenu::new(1, &context)?;
        reopened.initialize(&inventory, &context)?;
        let stale = clone_before.prepare_button(0, &inventory, &context)?;
        assert!(stale.validate(&reopened, &inventory, &context).is_err());
        Ok(())
    }
    #[test]
    fn result_take_exposes_output_then_cursor_then_input_recompute_phases() -> Result<()> {
        let context = Context::default();
        let (mut menu, mut inventory) = ready(&context, 2, 2, false)?;
        let plan = menu.prepare_click(
            &click(&menu, 3, 0, ClickType::Pickup),
            &inventory,
            &context,
            false,
        )?;
        let events = plan.events();
        let output = events
            .iter()
            .find_map(|event| match event {
                LoomMenuEvent::Source {
                    event: LoomSourceEvent::OutputContainerChanged,
                    phase,
                } => Some(phase),
                _ => None,
            })
            .unwrap();
        assert!(output.result_preview.is_empty() && output.carried.is_empty());
        let inputs = events
            .iter()
            .filter_map(|event| match event {
                LoomMenuEvent::Source {
                    event: LoomSourceEvent::InputContainerChanged,
                    phase,
                } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].inputs[0].count, 1);
        assert_eq!(inputs[0].inputs[1].count, 2);
        assert_eq!(inputs[0].carried.count, 1);
        assert!(inputs[0].result_preview.is_empty());
        assert_eq!(inputs[1].inputs[0].count, 1);
        assert_eq!(inputs[1].inputs[1].count, 1);
        assert_eq!(inputs[1].result_preview.count, 1);
        let sound = events
            .iter()
            .position(|event| matches!(event, LoomMenuEvent::TakeSound { .. }))
            .unwrap();
        assert!(matches!(
            events[sound + 1],
            LoomMenuEvent::Source {
                event: LoomSourceEvent::OutputContainerChanged,
                ..
            }
        ));
        plan.commit(&mut menu, &mut inventory, &context)?;
        assert_eq!(menu.carried().count, 1);
        assert_eq!(menu.state.result().count, 1);
        // Empty-banner branch: output callbacks retain the old selection/list.
        let plan = menu.prepare_click(
            &click(&menu, 0, 0, ClickType::Throw),
            &inventory,
            &context,
            false,
        )?;
        // Throw is refused while carrying; first return the cursor through a real player slot.
        assert_eq!(plan.next_menu().state.inputs()[0].count, 1);
        apply(&mut menu, &mut inventory, &context, 4, 0, ClickType::Pickup)?;
        let out = apply(&mut menu, &mut inventory, &context, 0, 0, ClickType::Throw)?;
        let old = out
            .events
            .iter()
            .find_map(|event| match event {
                LoomMenuEvent::Source {
                    event: LoomSourceEvent::OutputContainerChanged,
                    phase,
                } => Some(phase),
                _ => None,
            })
            .unwrap();
        assert_eq!(old.selected_pattern, 1);
        assert_eq!(old.selectable_patterns.len(), 2);
        assert!(old.inputs[0].is_empty());
        assert_eq!(menu.state.selected_pattern(), -1);
        Ok(())
    }
    #[test]
    fn signed_safe_insert_zero_callbacks_self_swap_and_external_cursor() -> Result<()> {
        let context = Context::default();
        let mut menu = LoomMenu::new(1, &context)?;
        let mut inventory = PlayerInventory::new();
        menu.state.replace_input_raw(1, stack("red_dye", 99))?;
        menu.sync.set_carried(stack("red_dye", 1));
        menu.initialize(&inventory, &context)?;
        let out = apply(&mut menu, &mut inventory, &context, 1, 0, ClickType::Pickup)?;
        assert_eq!(menu.state.inputs()[1].count, 64);
        assert_eq!(menu.carried().count, 36);
        let phases = out
            .events
            .iter()
            .filter_map(|event| match event {
                LoomMenuEvent::Source {
                    event: LoomSourceEvent::InputContainerChanged,
                    phase,
                } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(phases.len(), 3);
        assert!(phases
            .iter()
            .all(|phase| phase.inputs[1].count == 64 && phase.carried.count == 36));
        let zero = apply(&mut menu, &mut inventory, &context, 1, 0, ClickType::Pickup)?;
        assert_eq!(
            zero.events
                .iter()
                .filter(|event| matches!(
                    event,
                    LoomMenuEvent::Source {
                        event: LoomSourceEvent::InputContainerChanged,
                        ..
                    }
                ))
                .count(),
            3
        );
        assert_eq!(menu.carried().count, 36);
        menu.sync.set_carried(ItemStack::empty());
        inventory.set_slot_authoritative(36, stack("stone", 4));
        let out = apply(&mut menu, &mut inventory, &context, 31, 0, ClickType::Swap)?;
        assert_eq!(inventory.slot(36).unwrap().count, 4);
        assert_eq!(
            out.events
                .iter()
                .filter(|event| matches!(event, LoomMenuEvent::PlayerInventoryChanged { .. }))
                .count(),
            2
        );
        Ok(())
    }
    #[test]
    fn borrowed_binding_time_and_actual_drag_order_are_revalidated() -> Result<()> {
        let mut context = Context::default();
        let (menu, inventory) = ready(&context, 2, 2, false)?;
        let plan = menu.prepare_click(
            &click(&menu, 3, 0, ClickType::Pickup),
            &inventory,
            &context,
            false,
        )?;
        context.time += 1;
        assert!(plan.validate(&menu, &inventory, &context).is_err());
        context.time -= 1;
        context.key += 1;
        assert!(plan.validate(&menu, &inventory, &context).is_err());
        context.key -= 1;
        context.supported = false;
        assert!(plan.validate(&menu, &inventory, &context).is_err());
        context.supported = true;
        context.valid = false;
        assert!(plan.validate(&menu, &inventory, &context).is_err());
        assert!(menu.prepare_button(0, &inventory, &context).is_err());
        assert!(menu
            .prepare_close(&inventory, &context, MenuCloseContext::DeadOrDisconnected)
            .is_ok());
        context.valid = true;
        let mut menu = LoomMenu::new(2, &context)?;
        let mut inventory = PlayerInventory::new();
        menu.sync.set_carried(stack("red_dye", 8));
        menu.initialize(&inventory, &context)?;
        for (slot, button) in [(-999, 0), (1, 1), (4, 1)] {
            apply(
                &mut menu,
                &mut inventory,
                &context,
                slot,
                button,
                ClickType::QuickCraft,
            )?;
        }
        let finish = click(&menu, -999, 2, ClickType::QuickCraft);
        assert!(menu
            .prepare_click(&finish, &inventory, &context, false)
            .is_err());
        assert_eq!(menu.carried().count, 8);
        context.order = Some(vec![4, 1]);
        let plan = menu.prepare_click(&finish, &inventory, &context, false)?;
        context.order = Some(vec![1, 4]);
        assert!(plan.validate(&menu, &inventory, &context).is_err());
        context.order = Some(vec![4, 1]);
        plan.commit(&mut menu, &mut inventory, &context)?;
        assert_eq!(inventory.slot(9).unwrap().count, 4);
        assert_eq!(menu.state.inputs()[1].count, 4);
        assert!(menu.carried().is_empty());
        // Exact same-window, same-binding, same-contents, same-sync-revision
        // replay. Only the checked lifetime differs after a real close/reopen.
        let mut empty_inventory = PlayerInventory::new();
        let mut old = LoomMenu::new(3, &context)?;
        old.initialize(&empty_inventory, &context)?;
        let stale = old.prepare_button(-1, &empty_inventory, &context)?;
        old.prepare_close(&empty_inventory, &context, MenuCloseContext::AliveConnected)?
            .commit(&mut old, &mut empty_inventory, &context)?;
        let mut reopened = LoomMenu::new(3, &context)?;
        reopened.initialize(&empty_inventory, &context)?;
        let mut same_except_lifetime = stale.before.clone();
        same_except_lifetime.instance = reopened.instance;
        assert_eq!(same_except_lifetime, reopened);
        assert!(stale
            .validate(&reopened, &empty_inventory, &context)
            .is_err());
        Ok(())
    }
    #[test]
    fn close_preserves_old_preview_and_drains_cursor_then_each_owned_input() -> Result<()> {
        let context = Context::default();
        let (mut menu, mut inventory) = ready(&context, 2, 2, true)?;
        menu.sync.set_carried(stack("diamond", 1));
        let out = menu
            .prepare_close(&inventory, &context, MenuCloseContext::DeadOrDisconnected)?
            .commit(&mut menu, &mut inventory, &context)?;
        assert_eq!(out.outcome.drops.len(), 4);
        assert_eq!(out.outcome.drops[0].stack, stack("diamond", 1));
        assert!(out
            .outcome
            .drops
            .iter()
            .all(|drop| !drop.around && !drop.trace_item));
        let phases = out
            .events
            .iter()
            .filter_map(|event| match event {
                LoomMenuEvent::Drop { phase, .. } => Some(phase),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(phases[0].inputs[0].count, 2);
        assert_eq!(phases[0].inputs[1].count, 2);
        assert_eq!(phases[0].carried.count, 1);
        assert!(phases[1].inputs[0].is_empty());
        assert_eq!(phases[1].inputs[1].count, 2);
        assert_eq!(phases[1].inputs[2].count, 1);
        assert!(phases[1].carried.is_empty());
        assert!(phases[2].inputs[0].is_empty() && phases[2].inputs[1].is_empty());
        assert_eq!(phases[2].inputs[2].count, 1);
        assert!(phases[3].inputs.iter().all(ItemStack::is_empty));
        assert!(phases.iter().all(|phase| phase.result_preview.count == 1));
        assert!(menu.state.inputs().iter().all(ItemStack::is_empty));
        assert!(menu.is_closed());
        Ok(())
    }

    #[test]
    fn unresolved_food_patch_rejects_before_mutation_but_prototype_food_is_supported() -> Result<()>
    {
        let context = Context::default();
        let mut menu = LoomMenu::new(1, &context)?;
        let mut inventory = PlayerInventory::new();
        let prototype_food = stack("mushroom_stew", 1);
        inventory.set_slot_authoritative(9, prototype_food.clone());
        menu.initialize(&inventory, &context)?;
        let mut unresolved = prototype_food.clone();
        unresolved.components.push(Component {
            type_id: 20,
            data: prototype_food
                .effective_component_bytes(20)
                .unwrap()
                .to_vec(),
        });
        unresolved.encode_canonical()?;
        inventory.set_slot_authoritative(9, unresolved.clone());
        let before_menu = menu.clone();
        let before_inventory = inventory.clone();
        let error = menu
            .prepare_click(
                &click(&menu, 4, 0, ClickType::Pickup),
                &inventory,
                &context,
                false,
            )
            .err()
            .expect("explicit nested food identity must reject");
        assert!(error.to_string().contains("reference-identity"));
        assert_eq!(menu, before_menu);
        assert!(same_inventory(&inventory, &before_inventory));
        inventory.set_slot_authoritative(9, prototype_food.clone());
        let mut prediction = click(&menu, 4, 0, ClickType::Pickup);
        prediction.changed_slots.insert(4, unresolved);
        assert!(menu
            .prepare_click(&prediction, &inventory, &context, false)
            .is_err());
        assert_eq!(menu, before_menu);
        assert_eq!(inventory.slot(9).unwrap(), &prototype_food);
        apply(&mut menu, &mut inventory, &context, 4, 0, ClickType::Pickup)?;
        assert_eq!(menu.carried(), &prototype_food);
        assert!(inventory.slot(9).unwrap().is_empty());
        Ok(())
    }
}
