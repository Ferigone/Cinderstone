//! Bounded Java 1.21 LecternMenu over the actual persistent book/page owner.
//! Only cursor/sync/UI state lives here. All real book, player, block, scheduler,
//! callback and drop writes run once in a detached host transaction.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    protocol::{Packet, Reader},
    workstations::{
        lectern::{self, LecternAction, LecternContainerHost, LecternHost},
        menu::{require_component_context, MenuRemovalEvent, MenuSync, SyncEvent, WorkstationKind},
    },
};
use anyhow::{bail, ensure, Result};
use std::{
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};

/// Implement on the captured actual block entity/world/player transaction.
/// No production default exists. stage must detach ALL mutable owners including
/// inventory, world/scheduler/RNG, listeners, dirty/persist and queued packets;
/// cloning shared mutable handles does not detach them. commit_staged must be
/// atomic and leave all live owners unchanged on failure. Queues may be sent
/// only after the infallible native UI assignment completes, without reentry.
pub trait LecternMenuHost: LecternContainerHost + Sized {
    type Binding: Clone + Eq + Debug;
    type Revision: Clone + Eq + Debug;
    /// Actual player/session, world, position and block-entity identities.
    fn binding(&self) -> &Self::Binding;
    /// Covers all actual mutable dependencies, including permissions/abilities,
    /// canonical inventory, book/page/count/block, callbacks and world RNG.
    fn revision(&self) -> Self::Revision;
    fn inventory(&self) -> &PlayerInventory;
    fn inventory_mut(&mut self) -> &mut PlayerInventory;
    fn stage(&self) -> Result<Self>;
    fn commit_staged(&mut self, staged: Self) -> Result<()>;
    /// Certify the actual vanilla book container and item semantics, enabled
    /// item admission, no existing cross-owner book/cursor/inventory aliases,
    /// and readonly/nonreentrant listeners. Unknown contexts must reject.
    fn ensure_supported_menu_context(&self) -> Result<()>;
    /// Read-only callback boundary, not an additional inventory or owned book.
    fn observe_phase(&mut self, phase: &LecternPhase) -> Result<()>;
    /// Stage the actual listener or packet at this source position. Packet
    /// queues are not sent during preparation. Do not execute returned records.
    fn stage_sync(&mut self, event: SyncEvent) -> Result<()>;
    /// Actual Player.drop through its real item/entity/RNG/stat owners, once.
    fn stage_drop(&mut self, owned: DroppedStack) -> Result<()>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LecternMenu<B: Clone + Eq + Debug> {
    binding: B,
    sync: MenuSync,
    instance: u64,
    revision: u64,
    closed: bool,
    drag_status: u8,
    drag_type: u8,
    drag_book: bool,
}
#[derive(Clone, Debug)]
pub struct LecternPhase {
    pub book: ItemStack,
    pub page: i32,
    pub page_count: i32,
    pub player_inventory: PlayerInventory,
    pub carried: ItemStack,
}
/// These are observations of operations already executed in the real detached
/// host. They confer no items and MUST NOT be executed again after commit.
#[derive(Clone, Debug)]
pub enum LecternOperation {
    Preflight(LecternAction),
    TakeBookRaw,
    SplitBookRaw {
        amount: i32,
    },
    WritePageData {
        page: i32,
        page_count: Option<i32>,
    },
    SetChanged,
    SetBlockProperties {
        powered: bool,
        has_book: Option<bool>,
        flags: i32,
    },
    BlockChangeWithoutSource,
    UpdateNeighborsBelow,
    ScheduleBlockTick {
        delay: i32,
    },
    LevelEvent {
        event: i32,
        data: i32,
    },
    AddToPlayerInventory,
    Drop {
        stack: ItemStack,
        around: bool,
        trace_item: bool,
    },
    /// The source bookAccess.setItem body is empty. This is an observation of
    /// that proved no-op; Slot.setChanged still follows it.
    IgnoredBookSetter {
        attempted: ItemStack,
    },
    BroadcastChanges,
}
#[derive(Clone, Debug)]
pub enum LecternEvent {
    Operation {
        operation: LecternOperation,
        before: LecternPhase,
        after: LecternPhase,
    },
    Sync {
        event: SyncEvent,
        phase: LecternPhase,
    },
}
#[derive(Debug, Default)]
pub struct LecternPublication {
    pub events: Vec<LecternEvent>,
    pub accepted_button: Option<bool>,
}
pub struct LecternPlan<H: LecternMenuHost> {
    before: LecternMenu<H::Binding>,
    before_host_revision: H::Revision,
    before_book: ItemStack,
    before_page: i32,
    before_page_count: i32,
    before_inventory: PlayerInventory,
    require_validity: bool,
    next_menu: LecternMenu<H::Binding>,
    staged_host: H,
    publication: LecternPublication,
}
impl<H: LecternMenuHost> LecternPlan<H> {
    pub fn next_menu(&self) -> &LecternMenu<H::Binding> {
        &self.next_menu
    }
    pub fn staged_host(&self) -> &H {
        &self.staged_host
    }
    pub fn observations(&self) -> &[LecternEvent] {
        &self.publication.events
    }
    pub fn accepted_button(&self) -> Option<bool> {
        self.publication.accepted_button
    }
    pub fn validate(&self, menu: &LecternMenu<H::Binding>, host: &H) -> Result<()> {
        menu.check_context(host)?;
        ensure!(*menu == self.before, "stale lectern UI plan");
        ensure!(
            host.revision() == self.before_host_revision,
            "lectern actual owner revision changed"
        );
        ensure!(
            host.book() == &self.before_book
                && host.page() == self.before_page
                && host.page_count() == self.before_page_count,
            "lectern book/page owner changed"
        );
        ensure!(
            same_inventory(host.inventory(), &self.before_inventory),
            "lectern canonical player inventory changed"
        );
        if self.require_validity {
            ensure!(
                lectern::still_valid(host)?,
                "lectern/player context became invalid"
            );
        }
        Ok(())
    }
    pub fn commit(
        self,
        menu: &mut LecternMenu<H::Binding>,
        host: &mut H,
    ) -> Result<LecternPublication> {
        self.validate(menu, host)?;
        host.commit_staged(self.staged_host)?;
        *menu = self.next_menu;
        Ok(self.publication)
    }
}
impl<B: Clone + Eq + Debug> LecternMenu<B> {
    pub fn new(window_id: u8, host: &impl LecternMenuHost<Binding = B>) -> Result<Self> {
        host.ensure_supported_menu_context()?;
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let instance = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| anyhow::anyhow!("lectern UI identity exhausted"))?;
        Ok(Self {
            binding: host.binding().clone(),
            sync: MenuSync::new(window_id, WorkstationKind::Lectern)?,
            instance,
            revision: 0,
            closed: false,
            drag_status: 0,
            drag_type: 0,
            drag_book: false,
        })
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
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn still_valid(&self, host: &impl LecternMenuHost<Binding = B>) -> Result<bool> {
        self.check_context(host)?;
        lectern::still_valid(host)
    }
    pub fn transfer_inventory_state(&self, player: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(player)
    }
    fn advance(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("lectern UI revision exhausted"))?;
        Ok(())
    }
    fn reset_drag(&mut self) {
        self.drag_status = 0;
        self.drag_book = false;
    }
    fn check_context(&self, host: &impl LecternMenuHost<Binding = B>) -> Result<()> {
        ensure!(!self.closed, "lectern UI is closed");
        ensure!(
            host.binding() == &self.binding,
            "lectern captured owner binding changed"
        );
        host.ensure_supported_menu_context()?;
        require_component_context(host.book())?;
        require_component_context(self.carried())?;
        for stack in host.inventory().slots() {
            require_component_context(stack)?;
        }
        require_component_context(host.inventory().carried())
    }
    fn prepare<H: LecternMenuHost<Binding = B>>(
        &self,
        host: &H,
        require_validity: bool,
    ) -> Result<LecternPlan<H>> {
        self.check_context(host)?;
        if require_validity {
            ensure!(
                lectern::still_valid(host)?,
                "invalid lectern/player context"
            );
        }
        let revision = host.revision();
        let staged_host = host.stage()?;
        self.check_context(&staged_host)?;
        ensure!(
            staged_host.revision() == revision
                && staged_host.book() == host.book()
                && staged_host.page() == host.page()
                && staged_host.page_count() == host.page_count()
                && same_inventory(staged_host.inventory(), host.inventory()),
            "lectern detached stage changed base owners"
        );
        Ok(LecternPlan {
            before: self.clone(),
            before_host_revision: revision,
            before_book: host.book().clone(),
            before_page: host.page(),
            before_page_count: host.page_count(),
            before_inventory: host.inventory().clone(),
            require_validity,
            next_menu: self.clone(),
            staged_host,
            publication: LecternPublication::default(),
        })
    }
    pub fn prepare_initialize<H: LecternMenuHost<Binding = B>>(
        &self,
        host: &H,
    ) -> Result<LecternPlan<H>> {
        let mut plan = self.prepare(host, false)?;
        {
            let mut w = Working::new(&mut plan);
            let events = w.menu.sync.initialize(
                &[w.host.book().clone()],
                w.host.inventory(),
                &[w.host.page()],
            )?;
            w.sync_events(events)?;
        }
        plan.finish()?;
        Ok(plan)
    }
    /// ServerPlayer.tick calls this before its stillValid/close decision.
    pub fn prepare_broadcast<H: LecternMenuHost<Binding = B>>(
        &self,
        host: &H,
    ) -> Result<LecternPlan<H>> {
        let mut plan = self.prepare(host, false)?;
        Working::new(&mut plan).broadcast()?;
        plan.finish()?;
        Ok(plan)
    }
    pub fn prepare_full_state<H: LecternMenuHost<Binding = B>>(
        &self,
        host: &H,
    ) -> Result<LecternPlan<H>> {
        let mut plan = self.prepare(host, false)?;
        {
            let mut w = Working::new(&mut plan);
            let events = w.menu.sync.broadcast_full_state(
                &[w.host.book().clone()],
                w.host.inventory(),
                &[w.host.page()],
            )?;
            w.sync_events(events)?;
        }
        plan.finish()?;
        Ok(plan)
    }
    pub fn prepare_set_data<H: LecternMenuHost<Binding = B>>(
        &self,
        index: usize,
        value: i32,
        host: &H,
    ) -> Result<LecternPlan<H>> {
        ensure!(index == 0, "lectern has one page data slot");
        let mut plan = self.prepare(host, false)?;
        lectern::set_page(&mut Working::new(&mut plan), value)?;
        plan.finish()?;
        Ok(plan)
    }
    pub fn prepare_button<H: LecternMenuHost<Binding = B>>(
        &self,
        button: i32,
        host: &H,
    ) -> Result<LecternPlan<H>> {
        let mut plan = self.prepare(host, true)?;
        let accepted = lectern::click_menu_button(&mut Working::new(&mut plan), button)?;
        plan.publication.accepted_button = Some(accepted);
        // Actual packet listener broadcasts once more for an accepted button.
        if accepted {
            Working::new(&mut plan).broadcast()?;
        }
        plan.finish()?;
        Ok(plan)
    }
    pub fn prepare_click<H: LecternMenuHost<Binding = B>>(
        &self,
        click: &ContainerClick,
        creative: bool,
        host: &H,
    ) -> Result<LecternPlan<H>> {
        ensure!(click.window_id == self.window_id(), "wrong lectern window");
        ensure!(click.slot < 1, "lectern has one actual slot");
        require_component_context(&click.carried)?;
        for stack in click.changed_slots.values() {
            require_component_context(stack)?;
        }
        let mut plan = self.prepare(host, true)?;
        let before = self.state_id();
        {
            let mut w = Working::new(&mut plan);
            w.menu.sync.suppress_remote_updates(true);
            w.do_click(click.slot as i32, click.button as i32, click.kind, creative)?;
            let events = w.menu.sync.finish_click(
                click,
                before,
                &[w.host.book().clone()],
                w.host.inventory(),
                &[w.host.page()],
            )?;
            w.sync_events(events)?;
        }
        plan.finish()?;
        Ok(plan)
    }
    pub fn prepare_close<H: LecternMenuHost<Binding = B>>(
        &self,
        context: MenuCloseContext,
        host: &H,
    ) -> Result<LecternPlan<H>> {
        let mut plan = self.prepare(host, false)?;
        Working::new(&mut plan).close(context)?;
        plan.next_menu.closed = true;
        plan.finish()?;
        Ok(plan)
    }
}
impl<H: LecternMenuHost> LecternPlan<H> {
    fn finish(&mut self) -> Result<()> {
        ensure!(
            self.staged_host.binding() == &self.before.binding,
            "lectern callback rebound actual owners"
        );
        self.staged_host.ensure_supported_menu_context()?;
        self.next_menu.advance()
    }
}
struct Working<'a, H: LecternMenuHost> {
    menu: &'a mut LecternMenu<H::Binding>,
    host: &'a mut H,
    events: &'a mut Vec<LecternEvent>,
}
impl<'a, H: LecternMenuHost> Working<'a, H> {
    fn new(plan: &'a mut LecternPlan<H>) -> Self {
        Self {
            menu: &mut plan.next_menu,
            host: &mut plan.staged_host,
            events: &mut plan.publication.events,
        }
    }
    fn phase(&self) -> LecternPhase {
        LecternPhase {
            book: self.host.book().clone(),
            page: self.host.page(),
            page_count: self.host.page_count(),
            player_inventory: self.host.inventory().clone(),
            carried: self.menu.carried().clone(),
        }
    }
    fn call<T>(
        &mut self,
        operation: LecternOperation,
        f: impl FnOnce(&mut H) -> Result<T>,
    ) -> Result<T> {
        let before = self.phase();
        self.host.observe_phase(&before)?;
        let value = f(self.host)?;
        let after = self.phase();
        self.events.push(LecternEvent::Operation {
            operation,
            before,
            after,
        });
        Ok(value)
    }
    fn sync_events(&mut self, events: Vec<SyncEvent>) -> Result<()> {
        for event in events {
            let phase = self.phase();
            self.host.observe_phase(&phase)?;
            self.host.stage_sync(event.clone())?;
            self.events.push(LecternEvent::Sync { event, phase });
        }
        Ok(())
    }
    fn broadcast(&mut self) -> Result<()> {
        let before = self.phase();
        self.host.observe_phase(&before)?;
        let events = self.menu.sync.broadcast_changes(
            &[self.host.book().clone()],
            self.host.inventory(),
            &[self.host.page()],
        )?;
        self.events.push(LecternEvent::Operation {
            operation: LecternOperation::BroadcastChanges,
            before,
            after: self.phase(),
        });
        self.sync_events(events)
    }
    fn slot_changed(&mut self) -> Result<()> {
        self.set_changed()?;
        self.broadcast()
    }
    fn ignored_setter(&mut self, attempted: ItemStack) -> Result<()> {
        let before = self.phase();
        self.events.push(LecternEvent::Operation {
            operation: LecternOperation::IgnoredBookSetter { attempted },
            before: before.clone(),
            after: before,
        });
        self.slot_changed()
    }
    fn raw_split(&mut self, amount: i32) -> Result<ItemStack> {
        self.call(LecternOperation::SplitBookRaw { amount }, |host| {
            host.split_book_raw(amount)
        })
    }
    fn try_remove(&mut self, amount: i32, maximum: i32) -> Result<ItemStack> {
        let source = self.host.book().clone();
        let amount = amount.min(maximum).min(source.count).max(0);
        if source.is_empty() || amount == 0 {
            return Ok(ItemStack::empty());
        }
        let taken = lectern::remove_book_item(self, amount)?;
        if self.host.book().is_empty() {
            self.ignored_setter(ItemStack::empty())?;
        }
        Ok(taken)
    }
    fn drop_owned(&mut self, owned: DroppedStack) -> Result<()> {
        if owned.stack.is_empty() {
            return Ok(());
        }
        let operation = LecternOperation::Drop {
            stack: owned.stack.clone(),
            around: owned.around,
            trace_item: owned.trace_item,
        };
        self.call(operation, |host| host.stage_drop(owned))
    }
    fn pickup(&mut self, button: i32) -> Result<()> {
        let source = self.host.book().clone();
        let carried = self.menu.carried().clone();
        reject_bundle(&source)?;
        reject_bundle(&carried)?;
        // cqz's Slot.mayPlace is TRUE but its actual Container.setItem is a
        // no-op. Installing the stored object in a different owner aliases it.
        if !source.is_empty()
            && !carried.is_empty()
            && !source.same_item_same_components(&carried)?
            && carried.count <= slot_limit(&carried)?
        {
            bail!("lectern replacement requires the actual cross-owner ItemStack alias owner");
        }
        if source.is_empty() {
            // This occurs only inside a source operation; normal packet
            // admission checks book validity before this method.
            if !carried.is_empty() {
                let n = (if button == 0 { carried.count } else { 1 }).min(slot_limit(&carried)?);
                self.menu
                    .sync
                    .set_carried(counted(&carried, carried.count.wrapping_sub(n)));
                self.ignored_setter(counted(&carried, n))?;
            }
        } else if carried.is_empty() {
            let n = if button == 0 {
                source.count
            } else {
                source.count.wrapping_add(1) / 2
            };
            let taken = self.try_remove(n, i32::MAX)?;
            if !taken.is_empty() {
                self.menu.sync.set_carried(taken);
                self.slot_changed()?;
            }
        } else if source.same_item_same_components(&carried)? {
            let n = (if button == 0 { carried.count } else { 1 })
                .min(slot_limit(&carried)?.wrapping_sub(source.count));
            self.menu
                .sync
                .set_carried(counted(&carried, carried.count.wrapping_sub(n)));
            // Existing source stack.grow(n) runs before the ignored setter.
            // With an occupied lectern's cap1, n can only be zero or negative.
            ensure!(n <= 0, "unsupported positive occupied lectern insertion");
            if n < 0 {
                let removed = self.raw_split(n.wrapping_neg())?;
                ensure!(
                    removed.count == n.wrapping_neg(),
                    "lectern signed mutation split mismatch"
                );
            }
            self.ignored_setter(self.host.book().clone())?;
        }
        self.slot_changed()
    }
    fn do_click(&mut self, slot: i32, button: i32, kind: ClickType, creative: bool) -> Result<()> {
        if kind == ClickType::QuickCraft {
            return self.quickcraft(slot, button, creative);
        }
        if self.menu.drag_status != 0 {
            self.menu.reset_drag();
            return Ok(());
        }
        match kind {
            ClickType::Pickup | ClickType::QuickMove if button == 0 || button == 1 => {
                if slot == -999 {
                    let carried = self.menu.carried().clone();
                    let n = if button == 0 {
                        carried.count
                    } else {
                        1.min(carried.count)
                    };
                    if button == 1 {
                        self.menu
                            .sync
                            .set_carried(counted(&carried, carried.count.wrapping_sub(n)));
                    }
                    self.drop_owned(DroppedStack {
                        stack: counted(&carried, n),
                        around: false,
                        trace_item: true,
                    })?;
                    if button == 0 {
                        self.menu.sync.set_carried(ItemStack::empty());
                    }
                } else if slot == 0 && kind == ClickType::Pickup {
                    self.pickup(button)?;
                }
                // LecternMenu.quickMoveStack always returns EMPTY.
            }
            ClickType::Swap if slot == 0 && ((0..9).contains(&button) || button == 40) => {
                if !self.host.book().is_empty() {
                    bail!("lectern SWAP requires the actual cross-owner ItemStack alias owner");
                }
            }
            ClickType::Clone if creative && slot == 0 && self.menu.carried().is_empty() => {
                let book = self.host.book().clone();
                self.menu
                    .sync
                    .set_carried(counted(&book, book.max_stack_size()?));
            }
            ClickType::Throw if slot == 0 && self.menu.carried().is_empty() => {
                let n = if button == 0 {
                    1
                } else {
                    self.host.book().count
                };
                let taken = self.try_remove(n, i32::MAX)?;
                if !taken.is_empty() {
                    self.slot_changed()?;
                    self.drop_owned(DroppedStack {
                        stack: taken,
                        around: false,
                        trace_item: true,
                    })?;
                }
            }
            // With one occupied pickup-permitted slot, cpu.PICKUP_ALL returns
            // immediately. No invented player slot or result slot is scanned.
            ClickType::PickupAll => {}
            _ => {}
        }
        Ok(())
    }
    fn quickcraft(&mut self, slot: i32, button: i32, creative: bool) -> Result<()> {
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
        reject_bundle(self.menu.carried())?;
        match self.menu.drag_status {
            0 => {
                self.menu.drag_type = ((button >> 2) & 3) as u8;
                if self.menu.drag_type <= 1 || self.menu.drag_type == 2 && creative {
                    self.menu.drag_status = 1;
                    self.menu.drag_book = false;
                } else {
                    self.menu.reset_drag();
                }
            }
            1 => {
                ensure!(slot == 0, "lectern drag target outside actual book slot");
                let book = self.host.book();
                let carried = self.menu.carried();
                if (book.is_empty()
                    || book.same_item_same_components(carried)?
                        && book.count <= carried.max_stack_size()?)
                    && (self.menu.drag_type == 2 || carried.count > i32::from(self.menu.drag_book))
                {
                    self.menu.drag_book = true;
                }
            }
            2 => {
                let selected = self.menu.drag_book;
                let button = self.menu.drag_type as i32;
                self.menu.reset_drag();
                if selected && button <= 1 {
                    self.pickup(button)?;
                }
            }
            _ => self.menu.reset_drag(),
        }
        Ok(())
    }
    fn close(&mut self, context: MenuCloseContext) -> Result<()> {
        self.menu.reset_drag();
        let original = self.menu.carried().clone();
        let mut expected_inventory = self.host.inventory().clone();
        let removal =
            self.menu
                .sync
                .close_inputs_ordered(&mut [], &mut expected_inventory, context)?;
        // Reconstruct the actual per-insertion phases on the one detached
        // canonical inventory, with the source carried object still present.
        self.menu.sync.set_carried(original.clone());
        let mut returning = original;
        for event in removal.events {
            match event {
                MenuRemovalEvent::Packet { index } => {
                    let packet = removal.outcome.packets[index].clone();
                    observe_return_packet(self.host.inventory_mut(), &packet, &mut returning)?;
                    self.menu.sync.set_carried(returning.clone());
                    self.sync_events(vec![SyncEvent::Packet(packet)])?;
                }
                MenuRemovalEvent::Drop { index } => {
                    ensure!(
                        removal.outcome.drops[index].stack == returning,
                        "lectern close return owner changed"
                    );
                    self.drop_owned(removal.outcome.drops[index].clone())?;
                    returning = ItemStack::empty();
                }
            }
        }
        ensure!(
            returning.is_empty() && same_inventory(self.host.inventory(), &expected_inventory),
            "lectern cursor return phase mismatch"
        );
        self.menu.sync.set_carried(ItemStack::empty());
        Ok(())
    }
}
impl<H: LecternMenuHost> LecternHost for Working<'_, H> {
    type BlockState = H::BlockState;
    fn book(&self) -> &ItemStack {
        self.host.book()
    }
    fn page(&self) -> i32 {
        self.host.page()
    }
    fn page_count(&self) -> i32 {
        self.host.page_count()
    }
    fn still_valid_block_entity(&self, range: f64) -> Result<bool> {
        self.host.still_valid_block_entity(range)
    }
    fn player_may_build(&self) -> Result<bool> {
        self.host.player_may_build()
    }
    fn preflight(&mut self, action: LecternAction) -> Result<()> {
        self.call(LecternOperation::Preflight(action), |host| {
            host.preflight(action)
        })
    }
    fn block_entity_state(&self) -> Result<Self::BlockState> {
        self.host.block_entity_state()
    }
    fn take_book_raw(&mut self) -> Result<ItemStack> {
        self.call(LecternOperation::TakeBookRaw, |host| host.take_book_raw())
    }
    fn write_page_data(&mut self, page: i32, page_count: Option<i32>) -> Result<()> {
        self.call(
            LecternOperation::WritePageData { page, page_count },
            |host| host.write_page_data(page, page_count),
        )
    }
    fn set_changed(&mut self) -> Result<()> {
        self.call(LecternOperation::SetChanged, |host| host.set_changed())
    }
    fn set_block_properties(
        &mut self,
        state: &Self::BlockState,
        powered: bool,
        has_book: Option<bool>,
        flags: i32,
    ) -> Result<Self::BlockState> {
        self.call(
            LecternOperation::SetBlockProperties {
                powered,
                has_book,
                flags,
            },
            |host| host.set_block_properties(state, powered, has_book, flags),
        )
    }
    fn block_change_without_source(&mut self, state: &Self::BlockState) -> Result<()> {
        self.call(LecternOperation::BlockChangeWithoutSource, |host| {
            host.block_change_without_source(state)
        })
    }
    fn update_neighbors_below(&mut self, state: &Self::BlockState) -> Result<()> {
        self.call(LecternOperation::UpdateNeighborsBelow, |host| {
            host.update_neighbors_below(state)
        })
    }
    fn schedule_block_tick(&mut self, state: &Self::BlockState, delay: i32) -> Result<()> {
        self.call(LecternOperation::ScheduleBlockTick { delay }, |host| {
            host.schedule_block_tick(state, delay)
        })
    }
    fn level_event(&mut self, event: i32, data: i32) -> Result<()> {
        self.call(LecternOperation::LevelEvent { event, data }, |host| {
            host.level_event(event, data)
        })
    }
    fn add_to_player_inventory(&mut self, book: &mut ItemStack) -> Result<bool> {
        self.call(LecternOperation::AddToPlayerInventory, |host| {
            host.add_to_player_inventory(book)
        })
    }
    fn drop_book(&mut self, remainder: ItemStack, trace_item: bool) -> Result<()> {
        self.drop_owned(DroppedStack {
            stack: remainder,
            around: false,
            trace_item,
        })
    }
    // This is the actual menu callback owner, replacing the host's standalone
    // kernel hook. Calling host.broadcast_changes too would broadcast twice.
    fn broadcast_changes(&mut self) -> Result<()> {
        self.broadcast()
    }
}
impl<H: LecternMenuHost> LecternContainerHost for Working<'_, H> {
    fn split_book_raw(&mut self, amount: i32) -> Result<ItemStack> {
        self.raw_split(amount)
    }
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
fn slot_limit(stack: &ItemStack) -> Result<i32> {
    let limit = stack.max_stack_size()?.min(1);
    ensure!(limit == 1, "invalid lectern item stack limit");
    Ok(limit)
}
fn reject_bundle(stack: &ItemStack) -> Result<()> {
    ensure!(
        inventory::item_name(stack.item_id) != Some("bundle"),
        "lectern bundle override requires actual item owner"
    );
    Ok(())
}
fn same_inventory(a: &PlayerInventory, b: &PlayerInventory) -> bool {
    a.slots() == b.slots()
        && a.carried() == b.carried()
        && a.selected_hotbar() == b.selected_hotbar()
        && a.state_id() == b.state_id()
}
fn observe_return_packet(
    inventory: &mut PlayerInventory,
    packet: &Packet,
    returning: &mut ItemStack,
) -> Result<()> {
    ensure!(packet.id == 0x15, "unsupported lectern close packet");
    let mut reader = Reader::new(&packet.payload);
    ensure!(
        reader.i8()? == -2 && reader.varint()? == 0,
        "lectern close requires raw Inventory slot packet"
    );
    let raw = reader.i16()?;
    let slot = match raw {
        0..=8 => 36 + raw as usize,
        9..=35 => raw as usize,
        40 => 45,
        _ => bail!("unsupported lectern raw inventory return slot"),
    };
    let stack = ItemStack::decode_exact(reader.remaining())?;
    let old = inventory.slot(slot).unwrap();
    require_component_context(&stack)?;
    require_component_context(returning)?;
    ensure!(
        stack.same_item_same_components(returning)?,
        "lectern return changed item identity"
    );
    let previous = if old.is_empty() {
        0
    } else {
        ensure!(
            old.same_item_same_components(&stack)?,
            "lectern return overwrote an owned item"
        );
        old.count
    };
    let added = stack.count.wrapping_sub(previous);
    ensure!(
        added > 0 && added <= returning.count,
        "invalid lectern cursor return amount"
    );
    inventory.set_slot_authoritative(slot, stack);
    *returning = counted(returning, returning.count.wrapping_sub(added));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{crafting::RecipeBook, inventory::Component, protocol::Writer};
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct State {
        powered: bool,
        has_book: bool,
        facing: u8,
    }
    // Controlled, detached native fixture owners. This is not a production
    // lectern/world persistence implementation or an original-Java oracle.
    #[derive(Clone)]
    struct Host {
        binding: (u64, u64),
        revision: u64,
        book: ItemStack,
        page: i32,
        count: i32,
        inventory: PlayerInventory,
        state: State,
        valid: bool,
        may_build: bool,
        supported: bool,
        fail_event: bool,
        fail_publish: bool,
        current: Option<LecternPhase>,
        calls: Vec<String>,
        phases: Vec<(String, LecternPhase)>,
        packets: Vec<Packet>,
        drops: Vec<DroppedStack>,
        schedule: Vec<i32>,
        dirty: u32,
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    impl Host {
        fn new() -> Self {
            let mut book = item("writable_book", 1);
            let mut content = Writer::new();
            content.varint(3);
            for text in ["one", "two", "three"] {
                content.string(text).bool(false);
            }
            book.components.push(Component {
                type_id: 33,
                data: content.into_inner(),
            });
            Self {
                binding: (31, 7),
                revision: 0,
                book,
                page: 0,
                count: 3,
                inventory: PlayerInventory::new(),
                state: State {
                    powered: false,
                    has_book: true,
                    facing: 2,
                },
                valid: true,
                may_build: true,
                supported: true,
                fail_event: false,
                fail_publish: false,
                current: None,
                calls: vec![],
                phases: vec![],
                packets: vec![],
                drops: vec![],
                schedule: vec![],
                dirty: 0,
            }
        }
        fn record(&mut self, name: impl Into<String>) {
            let name = name.into();
            self.calls.push(name.clone());
            if let Some(phase) = &self.current {
                self.phases.push((name, phase.clone()));
            }
            self.revision += 1;
        }
        fn clear_records(&mut self) {
            self.calls.clear();
            self.phases.clear();
            self.packets.clear();
            self.drops.clear();
            self.schedule.clear();
        }
    }
    impl LecternHost for Host {
        type BlockState = State;
        fn book(&self) -> &ItemStack {
            &self.book
        }
        fn page(&self) -> i32 {
            self.page
        }
        fn page_count(&self) -> i32 {
            self.count
        }
        fn still_valid_block_entity(&self, range: f64) -> Result<bool> {
            ensure!(range == 4.0, "source lectern reach");
            Ok(self.valid)
        }
        fn player_may_build(&self) -> Result<bool> {
            Ok(self.may_build)
        }
        fn preflight(&mut self, _: LecternAction) -> Result<()> {
            ensure!(self.supported, "fixture capability unsupported");
            Ok(())
        }
        fn block_entity_state(&self) -> Result<State> {
            Ok(self.state.clone())
        }
        fn take_book_raw(&mut self) -> Result<ItemStack> {
            self.record("raw_take");
            Ok(std::mem::take(&mut self.book))
        }
        fn write_page_data(&mut self, page: i32, count: Option<i32>) -> Result<()> {
            self.record(format!("page:{page}"));
            self.page = page;
            if let Some(count) = count {
                self.count = count;
            }
            Ok(())
        }
        fn set_changed(&mut self) -> Result<()> {
            self.record("dirty");
            self.dirty += 1;
            Ok(())
        }
        fn set_block_properties(
            &mut self,
            state: &State,
            powered: bool,
            has_book: Option<bool>,
            flags: i32,
        ) -> Result<State> {
            ensure!(flags == 3, "source state flags");
            self.record(format!("state:{powered}:{has_book:?}"));
            let mut next = state.clone();
            next.powered = powered;
            if let Some(value) = has_book {
                next.has_book = value;
            }
            self.state = next.clone();
            Ok(next)
        }
        fn block_change_without_source(&mut self, state: &State) -> Result<()> {
            ensure!(state == &self.state, "actual new state");
            self.record("block_change:null");
            Ok(())
        }
        fn update_neighbors_below(&mut self, state: &State) -> Result<()> {
            ensure!(state.facing == 2, "preserve real old state");
            self.record("below");
            Ok(())
        }
        fn schedule_block_tick(&mut self, _: &State, delay: i32) -> Result<()> {
            self.record(format!("schedule:{delay}"));
            self.schedule.push(delay);
            Ok(())
        }
        fn level_event(&mut self, event: i32, data: i32) -> Result<()> {
            self.record(format!("event:{event}:{data}"));
            ensure!(!self.fail_event, "fixture event stage failed");
            Ok(())
        }
        fn add_to_player_inventory(&mut self, book: &mut ItemStack) -> Result<bool> {
            self.record("inventory_add");
            // Focused source fixture: one actual book, no merge target, first
            // raw hotbar/main free slot. Full inventory returns false unchanged.
            if let Some(slot) = (36..45)
                .chain(9..36)
                .find(|&slot| self.inventory.slot(slot).unwrap().is_empty())
            {
                self.inventory
                    .set_slot_authoritative(slot, std::mem::take(book));
                Ok(true)
            } else {
                Ok(false)
            }
        }
        fn drop_book(&mut self, remainder: ItemStack, trace_item: bool) -> Result<()> {
            self.stage_drop(DroppedStack {
                stack: remainder,
                around: false,
                trace_item,
            })
        }
        fn broadcast_changes(&mut self) -> Result<()> {
            bail!("controller must own the actual menu broadcast callback")
        }
    }
    impl LecternContainerHost for Host {
        fn split_book_raw(&mut self, amount: i32) -> Result<ItemStack> {
            self.record(format!("split:{amount}"));
            let n = amount.max(0).min(self.book.count);
            let taken = counted(&self.book, n);
            self.book = counted(&self.book, self.book.count - n);
            Ok(taken)
        }
    }
    impl LecternMenuHost for Host {
        type Binding = (u64, u64);
        type Revision = u64;
        fn binding(&self) -> &(u64, u64) {
            &self.binding
        }
        fn revision(&self) -> u64 {
            self.revision
        }
        fn inventory(&self) -> &PlayerInventory {
            &self.inventory
        }
        fn inventory_mut(&mut self) -> &mut PlayerInventory {
            &mut self.inventory
        }
        fn stage(&self) -> Result<Self> {
            Ok(self.clone())
        }
        fn commit_staged(&mut self, staged: Self) -> Result<()> {
            ensure!(!self.fail_publish, "fixture atomic publish failed");
            *self = staged;
            Ok(())
        }
        fn ensure_supported_menu_context(&self) -> Result<()> {
            ensure!(self.supported, "fixture unsupported callback context");
            Ok(())
        }
        fn observe_phase(&mut self, phase: &LecternPhase) -> Result<()> {
            self.current = Some(phase.clone());
            Ok(())
        }
        fn stage_sync(&mut self, event: SyncEvent) -> Result<()> {
            match event {
                SyncEvent::Packet(packet) => {
                    self.record(format!("packet:{}", packet.id));
                    self.packets.push(packet);
                }
                SyncEvent::SlotListener {
                    menu_slot,
                    player_slot,
                    ..
                } => {
                    ensure!(
                        menu_slot == 0 && player_slot.is_none(),
                        "lectern has no appended inventory listener"
                    );
                    self.record("slot_listener");
                }
                SyncEvent::DataListener { index, .. } => {
                    ensure!(index == 0, "lectern one page data");
                    self.record("data_listener");
                }
            }
            Ok(())
        }
        fn stage_drop(&mut self, owned: DroppedStack) -> Result<()> {
            self.record(format!("drop:{}:{}", owned.around, owned.trace_item));
            self.drops.push(owned);
            Ok(())
        }
    }
    type Menu = LecternMenu<(u64, u64)>;
    fn setup() -> Result<(Menu, Host)> {
        let mut host = Host::new();
        let mut menu = Menu::new(7, &host)?;
        menu.prepare_initialize(&host)?
            .commit(&mut menu, &mut host)?;
        host.clear_records();
        Ok((menu, host))
    }
    fn packet(menu: &Menu, slot: i16, button: i8, kind: ClickType) -> ContainerClick {
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
    fn click(
        menu: &mut Menu,
        host: &mut Host,
        slot: i16,
        button: i8,
        kind: ClickType,
        creative: bool,
    ) -> Result<LecternPublication> {
        menu.prepare_click(&packet(menu, slot, button, kind), creative, host)?
            .commit(menu, host)
    }
    fn broadcasts(publication: &LecternPublication) -> usize {
        publication
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    LecternEvent::Operation {
                        operation: LecternOperation::BroadcastChanges,
                        ..
                    }
                )
            })
            .count()
    }
    #[test]
    fn live_page_owner_pulses_actual_block_and_buttons_broadcast_twice() -> Result<()> {
        let (mut menu, mut host) = setup()?;
        let original = host.book.clone();
        let publication = menu
            .prepare_button(2, &host)?
            .commit(&mut menu, &mut host)?;
        assert_eq!(publication.accepted_button, Some(true));
        assert_eq!(broadcasts(&publication), 2);
        assert_eq!(host.page, 1);
        assert_eq!(host.count, 3);
        assert_eq!(host.book, original);
        assert!(host.state.powered && host.state.has_book);
        assert_eq!(host.schedule, vec![2]);
        assert!(host.calls.starts_with(&[
            "page:1".into(),
            "dirty".into(),
            "state:true:None".into(),
            "below".into(),
            "schedule:2".into(),
            "event:1043:0".into()
        ]));
        menu.prepare_button(i32::MAX, &host)?
            .commit(&mut menu, &mut host)?;
        assert_eq!(host.page, 2);
        host.clear_records();
        let old_dirty = host.dirty;
        let no_change = menu
            .prepare_button(2, &host)?
            .commit(&mut menu, &mut host)?;
        assert_eq!(broadcasts(&no_change), 2);
        assert_eq!(host.dirty, old_dirty);
        assert!(host.schedule.is_empty());
        assert!(host.calls.is_empty());
        let direct = menu
            .prepare_set_data(0, 0, &host)?
            .commit(&mut menu, &mut host)?;
        assert_eq!(broadcasts(&direct), 1);
        assert_eq!(host.page, 0);
        let invalid = menu
            .prepare_button(0, &host)?
            .commit(&mut menu, &mut host)?;
        assert_eq!(invalid.accepted_button, Some(false));
        assert_eq!(broadcasts(&invalid), 0);
        // A failure after page/state writes discards the entire detached world.
        host.fail_event = true;
        let before = host.clone();
        let before_menu = menu.clone();
        assert!(menu.prepare_button(2, &host).is_err());
        assert_eq!(menu, before_menu);
        assert_eq!(host.page, before.page);
        assert_eq!(host.state, before.state);
        assert_eq!(host.revision, before.revision);
        Ok(())
    }
    #[test]
    fn permission_guard_and_take_book_real_block_reset_then_inventory_drop() -> Result<()> {
        let (mut menu, mut host) = setup()?;
        host.may_build = false;
        host.revision += 1;
        let refused = menu
            .prepare_button(3, &host)?
            .commit(&mut menu, &mut host)?;
        assert_eq!(refused.accepted_button, Some(false));
        assert!(host.calls.is_empty());
        assert_eq!(host.book.count, 1);
        host.may_build = true;
        host.page = 2;
        host.state.powered = true;
        for slot in 9..45 {
            host.inventory
                .set_slot_authoritative(slot, item("stone", 64));
        }
        host.revision += 1;
        let original = host.book.clone();
        let before = menu.clone();
        let plan = menu.prepare_button(3, &host)?;
        assert_eq!(menu, before);
        assert_eq!(host.book, original);
        assert!(host.drops.is_empty());
        let out = plan.commit(&mut menu, &mut host)?;
        assert_eq!(broadcasts(&out), 1);
        assert!(host.book.is_empty());
        assert_eq!((host.page, host.count), (0, 0));
        assert!(!host.state.powered && !host.state.has_book);
        assert_eq!(host.drops.len(), 1);
        assert_eq!(host.drops[0].stack, original);
        assert!(!host.drops[0].around && !host.drops[0].trace_item);
        assert!(host.calls.starts_with(&[
            "raw_take".into(),
            "page:0".into(),
            "state:false:Some(false)".into(),
            "block_change:null".into(),
            "below".into(),
            "dirty".into(),
            "inventory_add".into(),
            "drop:false:false".into()
        ]));
        assert!(menu
            .prepare_click(&packet(&menu, 0, 0, ClickType::Pickup), false, &host)
            .is_err());
        menu.prepare_broadcast(&host)?
            .commit(&mut menu, &mut host)?;
        // Failed publication changes neither actual block nor UI/player owner.
        let (mut menu, mut host) = setup()?;
        host.fail_publish = true;
        let plan = menu.prepare_button(3, &host)?;
        let old_menu = menu.clone();
        let old_book = host.book.clone();
        let revision = host.revision;
        assert!(plan.commit(&mut menu, &mut host).is_err());
        assert_eq!(menu, old_menu);
        assert_eq!(host.book, old_book);
        assert_eq!(host.revision, revision);
        assert!(host.inventory.slot(36).unwrap().is_empty());
        let (mut menu, mut host) = setup()?;
        let original = host.book.clone();
        let retained_cursor = item("diamond", 2);
        host.inventory
            .set_carried_authoritative(retained_cursor.clone());
        host.revision += 1;
        menu.prepare_button(3, &host)?
            .commit(&mut menu, &mut host)?;
        assert_eq!(host.inventory.slot(36).unwrap(), &original);
        assert_eq!(host.inventory.carried(), &retained_cursor);
        assert!(host.book.is_empty() && host.drops.is_empty());
        Ok(())
    }
    #[test]
    fn ordinary_removal_has_cursor_phases_and_close_never_drains_live_block_book() -> Result<()> {
        let (mut menu, mut host) = setup()?;
        host.may_build = false;
        host.revision += 1;
        let original = host.book.clone();
        let out = click(&mut menu, &mut host, 0, 0, ClickType::Pickup, false)?;
        assert_eq!(menu.carried(), &original);
        assert!(host.book.is_empty());
        assert_eq!(broadcasts(&out), 3);
        let dirty = out
            .events
            .iter()
            .filter_map(|event| match event {
                LecternEvent::Operation {
                    operation: LecternOperation::SetChanged,
                    before,
                    ..
                } => Some(before),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(dirty.len(), 3);
        assert!(dirty[0].carried.is_empty());
        assert_eq!(dirty[1].carried, original);
        assert_eq!(dirty[2].carried, original);
        assert!(dirty
            .iter()
            .all(|phase| phase.book.is_empty() && phase.page == 0 && phase.page_count == 0));
        menu.prepare_close(MenuCloseContext::AliveConnected, &host)?
            .commit(&mut menu, &mut host)?;
        assert!(menu.is_closed());
        assert_eq!(host.inventory.slot(36).unwrap(), &original);
        assert!(menu.carried().is_empty());
        let (mut menu, mut host) = setup()?;
        host.page = 2;
        host.revision += 1;
        let original = host.book.clone();
        click(&mut menu, &mut host, 0, 0, ClickType::Clone, true)?;
        assert_eq!(menu.carried().count, original.max_stack_size()?);
        host.clear_records();
        let out = menu
            .prepare_close(MenuCloseContext::DeadOrDisconnected, &host)?
            .commit(&mut menu, &mut host)?;
        assert_eq!(host.book, original);
        assert_eq!((host.page, host.count), (2, 3));
        assert!(host.state.has_book);
        assert_eq!(host.drops.len(), 1);
        assert!(!host.drops[0].trace_item);
        assert!(out.events.iter().any(|event|matches!(event,LecternEvent::Operation{operation:LecternOperation::Drop{..},before,..} if before.book==original && before.carried.count>0)));
        assert!(!out.events.iter().any(|event| matches!(
            event,
            LecternEvent::Operation {
                operation: LecternOperation::WritePageData { .. } | LecternOperation::SetChanged,
                ..
            }
        )));
        Ok(())
    }
    #[test]
    fn ignored_setter_signed_counts_alias_rejection_and_all_admitted_modes() -> Result<()> {
        let (mut menu, mut host) = setup()?;
        let book = host.book.clone();
        menu.sync.set_carried(item("stone", 1));
        let before = menu.clone();
        let before_host = host.clone();
        assert!(menu
            .prepare_click(&packet(&menu, 0, 0, ClickType::Pickup), false, &host)
            .is_err());
        assert_eq!(menu, before);
        assert_eq!(host.book, before_host.book);
        assert_eq!(host.revision, before_host.revision);
        assert!(menu
            .prepare_click(&packet(&menu, 0, 0, ClickType::Swap), false, &host)
            .is_err());
        menu.sync.set_carried(counted(&book, 2));
        host.clear_records();
        let out = click(&mut menu, &mut host, 0, 0, ClickType::Pickup, false)?;
        assert_eq!(broadcasts(&out), 2);
        assert_eq!(menu.carried().count, 2);
        assert_eq!(host.book, book);
        assert_eq!(
            out.events
                .iter()
                .filter(|event| matches!(
                    event,
                    LecternEvent::Operation {
                        operation: LecternOperation::IgnoredBookSetter { .. },
                        ..
                    }
                ))
                .count(),
            1
        );
        host.book.count = 2;
        host.revision += 1;
        menu.sync.set_carried(counted(&book, 1));
        click(&mut menu, &mut host, 0, 0, ClickType::Pickup, false)?;
        assert_eq!(host.book.count, 1);
        assert_eq!(menu.carried().count, 2);
        // One source Slot means one drag target. Greedy completion invokes the
        // normal right pickup, including the real ignored setter's callbacks.
        for (slot, button) in [(-999, 4), (0, 5), (-999, 6)] {
            click(
                &mut menu,
                &mut host,
                slot,
                button,
                ClickType::QuickCraft,
                false,
            )?;
        }
        assert_eq!(host.book.count, 1);
        assert_eq!(menu.carried().count, 2);
        let before_book = host.book.clone();
        let quick = click(&mut menu, &mut host, 0, 0, ClickType::QuickMove, false)?;
        assert_eq!(broadcasts(&quick), 0);
        let all = click(&mut menu, &mut host, 0, 0, ClickType::PickupAll, false)?;
        assert_eq!(broadcasts(&all), 0);
        assert_eq!(host.book, before_book);
        host.clear_records();
        let before_count = menu.carried().count;
        click(&mut menu, &mut host, -999, 1, ClickType::Pickup, false)?;
        assert_eq!(menu.carried().count, before_count - 1);
        assert_eq!(host.drops[0].stack.count, 1);
        assert!(host.drops[0].trace_item);
        let drop_phase = host
            .phases
            .iter()
            .find(|(name, _)| name == "drop:false:true")
            .unwrap();
        assert_eq!(drop_phase.1.carried.count, before_count - 1);
        menu.sync.set_carried(ItemStack::empty());
        host.clear_records();
        click(&mut menu, &mut host, 0, 0, ClickType::Throw, false)?;
        assert!(host.book.is_empty());
        assert_eq!(host.drops.len(), 1);
        assert!(host.drops[0].trace_item);
        Ok(())
    }
    #[test]
    fn retained_sync_stale_ui_owner_and_unresolved_components_reject_atomically() -> Result<()> {
        let (mut menu, mut host) = setup()?;
        let old = menu.prepare_button(2, &host)?;
        menu.prepare_full_state(&host)?
            .commit(&mut menu, &mut host)?;
        assert!(old.validate(&menu, &host).is_err());
        let plan = menu.prepare_button(2, &host)?;
        host.valid = false;
        host.revision += 1;
        assert!(plan.validate(&menu, &host).is_err());
        menu.prepare_close(MenuCloseContext::AliveConnected, &host)?
            .commit(&mut menu, &mut host)?;
        assert!(menu.is_closed());
        assert_eq!(host.book.count, 1);
        assert!(menu.prepare_broadcast(&host).is_err());
        host.valid = true;
        host.revision += 1;
        let mut reopened = Menu::new(7, &host)?;
        reopened
            .prepare_initialize(&host)?
            .commit(&mut reopened, &mut host)?;
        assert!(plan.validate(&reopened, &host).is_err());
        let food = item("mushroom_stew", 1);
        let mut unresolved = food.clone();
        unresolved.components.push(Component {
            type_id: 20,
            data: food.effective_component_bytes(20).unwrap().to_vec(),
        });
        host.inventory.set_slot_authoritative(9, unresolved);
        host.revision += 1;
        let before = reopened.clone();
        assert!(reopened.prepare_button(2, &host).is_err());
        assert_eq!(reopened, before);
        host.inventory.set_slot_authoritative(9, food);
        host.revision += 1;
        let plan = reopened.prepare_button(2, &host)?;
        plan.commit(&mut reopened, &mut host)?;
        assert_eq!(host.page, 1);
        // Exact UI replay: only the nonreused lifetime differs between these
        // same-window/binding/book/page/sync/revision menu states.
        let (mut old, mut host) = setup()?;
        let stale = old.prepare_button(-1, &host)?;
        old.prepare_close(MenuCloseContext::AliveConnected, &host)?
            .commit(&mut old, &mut host)?;
        let mut reopened = Menu::new(7, &host)?;
        reopened
            .prepare_initialize(&host)?
            .commit(&mut reopened, &mut host)?;
        let mut same_except_lifetime = stale.before.clone();
        same_except_lifetime.instance = reopened.instance;
        assert_eq!(same_except_lifetime, reopened);
        assert!(stale.validate(&reopened, &host).is_err());
        Ok(())
    }
}
