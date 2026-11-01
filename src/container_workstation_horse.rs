//! Java1.21 HorseInventoryMenu over actual borrowed mount/container/equipment
//! and player owners. This controller creates no mount, equipment registry,
//! vehicle state or second live inventory. Every effect is staged by the host.
use crate::{
    container::{ClickType, ContainerClick, DroppedStack, MenuCloseContext, PlayerContainer},
    inventory::{self, ItemStack, PlayerInventory},
    protocol::{Packet, Writer},
    workstations::menu::{require_component_context, MenuSync, SyncEvent},
};
use anyhow::{ensure, Result};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};

pub const HORSE_SCREEN_OPEN_PACKET: i32 = 0x20;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HorseMenuToken {
    instance: u64,
    revision: u64,
}
#[derive(Clone, Copy, Debug)]
pub enum HorseMenuAction {
    Open,
    Broadcast,
    Click(ClickType),
    Close(MenuCloseContext),
}
#[derive(Clone, Debug)]
pub struct HorsePhase {
    pub horse_inventory: Vec<ItemStack>,
    pub body_armor: ItemStack,
    pub player_inventory: PlayerInventory,
    pub carried: ItemStack,
}

/// Bind the actual captured ServerPlayer/Level/live AbstractHorse lifetime,
/// captured horse SimpleContainer and distinct getBodyArmorAccess object. An
/// entity network ID or equal contents alone does not establish those owners.
///
/// begin_stage isolates every involved real owner, including equipment/drop
/// chance/persistence flags, entity metadata, registry effects, listener work,
/// sound/game events, RNG, Inventory.add/drop and packet publication.
/// publish_stage validates actual captured owner revisions and atomically
/// publishes all of that stage, or publishes nothing on failure. A shallow
/// clone of shared live state is insufficient. No production default is given.
pub trait HorseMenuHost: Sized {
    type Binding: Clone + Eq + Debug;
    /// Actual registry/equipment-effect snapshot, callback admission and Slot
    /// identity/order scope. Must certify nonreentrant read-only menu listeners
    /// and remain the same throughout each staged action, or reject the action.
    type ContextToken: Clone + Eq + Debug;
    fn binding(&self) -> &Self::Binding;
    fn context_token(&self) -> &Self::ContextToken;
    fn begin_stage(&self, menu: HorseMenuToken, action: HorseMenuAction) -> Result<Self>;
    fn publish_stage(&mut self, stage: Self, menu: HorseMenuToken) -> Result<()>;
    fn preflight(&mut self, action: HorseMenuAction, context: &Self::ContextToken) -> Result<()>;
    /// Actual AbstractHorse.getInventoryColumns at opening, not a guessed UI.
    fn inventory_columns(&self) -> Result<u8>;
    fn entity_id(&self) -> i32;
    fn inventory(&self) -> &PlayerInventory;
    fn inventory_mut(&mut self) -> &mut PlayerInventory;
    /// Captured SimpleContainer: saddle index0, then actual storage indices1+.
    fn horse_inventory(&self) -> &[ItemStack];
    fn body_armor(&self) -> &ItemStack;
    fn horse_container_maximum(&self) -> i32;
    /// Original predicate owners. They preserve horse/camel/donkey/mule/llama
    /// differences, live age/taming/alive state and actual armor/carpet tags.
    fn is_saddleable(&self) -> Result<bool>;
    fn can_use_body_slot(&self) -> Result<bool>;
    fn is_body_armor_item(&self, stack: &ItemStack) -> Result<bool>;
    fn prevents_armor_change(&self, stack: &ItemStack) -> Result<bool>;
    /// ArmorSlot checks actual isCreative, while CLONE/creative drag checks
    /// abilities.instabuild. Do not collapse these two real player predicates.
    fn player_is_creative(&self) -> bool;
    fn instabuild(&self) -> bool;
    /// Source conjunction, in order: current inventory differs from captured;
    /// captured horse container validity; BODY access validity; alive mount;
    /// actual canInteractWithEntity(mount,+4.0). Opening admission is separate.
    fn has_inventory_changed(&self) -> Result<bool>;
    fn horse_container_still_valid(&self) -> Result<bool>;
    fn body_access_still_valid(&self) -> Result<bool>;
    fn mount_is_alive(&self) -> Result<bool>;
    fn player_can_interact_with_mount(&self, additional_range: f64) -> Result<bool>;
    /// Raw source in-place stack mutation, without setter/dirty callbacks.
    fn mutate_horse_stack_in_place(&mut self, slot: usize, stack: ItemStack) -> Result<()>;
    fn mutate_body_stack_in_place(&mut self, stack: ItemStack) -> Result<()>;
    /// Actual SimpleContainer.setItem: replace that slot's stack, clamp by its
    /// real container/item maximum, and synchronously run its listeners. This
    /// differs from mutating an existing stack count without notification.
    /// Phase is callback entry; internal listeners observe the mutated actual
    /// container and this same staged player inventory/cursor.
    fn horse_container_set_item(
        &mut self,
        slot: usize,
        stack: ItemStack,
        phase: &HorsePhase,
    ) -> Result<()>;

    /// Actual body-access setTheItem -> Mob.setBodyArmorItem, including source
    /// setItemSlotAndDropWhenKilled, equipment callbacks, guaranteed drop and
    /// persistence owners. This is distinct from the prior ArmorSlot callback.
    fn set_body_armor_item(&mut self, stack: ItemStack, phase: &HorsePhase) -> Result<()>;
    /// ArmorSlot.setByPlayer invokes actual LivingEntity.onEquipItem(BODY,
    /// old,new) BEFORE the body-access container setter. Old may be the actual
    /// removed stack supplied by Slot.tryRemove, not current empty equipment.
    fn on_equip_body(&mut self, old: &ItemStack, new: &ItemStack, phase: &HorsePhase)
        -> Result<()>;
    /// Actual SimpleContainer listeners, including AbstractHorse.containerChanged
    /// saddle metadata and sound. No substitute UI-only saddle flag is created.
    fn horse_container_set_changed(&mut self, phase: &HorsePhase) -> Result<()>;
    fn body_access_set_changed(&mut self, phase: &HorsePhase) -> Result<()>;
    fn player_inventory_set_changed(&mut self, phase: &HorsePhase) -> Result<()>;
    fn horse_container_start_open(&mut self, phase: &HorsePhase) -> Result<()>;
    fn horse_container_stop_open(&mut self, phase: &HorsePhase) -> Result<()>;
    fn stage_sync_events(&mut self, events: Vec<SyncEvent>, phase: &HorsePhase) -> Result<()>;
    fn stage_drop(&mut self, item: DroppedStack, phase: &HorsePhase) -> Result<()>;
    fn add_to_player_inventory(
        &mut self,
        stack: &mut ItemStack,
        phase: &HorsePhase,
    ) -> Result<bool>;
    /// Actual placeItemBackInInventory(stack,true), its insertion packets and
    /// overflow Player.drop(false). Phase is call entry; stack is the same
    /// logical owned cursor through the callback's internal mutations.
    fn place_item_back(&mut self, stack: &mut ItemStack, phase: &HorsePhase) -> Result<()>;
    fn quickcraft_iteration_order(
        &self,
        context: &Self::ContextToken,
        selected: &[usize],
    ) -> Result<Vec<usize>>;
}

#[derive(Debug)]
pub struct HorseInventoryMenu<K, C> {
    binding: K,
    context: C,
    token: HorseMenuToken,
    active: bool,
    columns: u8,
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
impl<K: Clone + Eq + Debug, C: Clone + Eq + Debug> HorseInventoryMenu<K, C> {
    /// Caller first handles the actual previous menu and window counter. This
    /// source opening transaction emits HorseScreenOpen, then calls startOpen,
    /// then installs/initializes the menu sync. No MenuType is fabricated.
    pub fn open<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        window: u8,
        host: &mut H,
    ) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let mut seen = NEXT.load(Ordering::Relaxed);
        let instance = loop {
            let next = seen
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("horse UI identity exhausted"))?;
            match NEXT.compare_exchange_weak(seen, next, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break seen,
                Err(actual) => seen = actual,
            }
        };
        let columns = host.inventory_columns()?;
        ensure!(
            columns <= 5,
            "unsupported actual horse inventory column count"
        );
        let mut menu = Self {
            binding: host.binding().clone(),
            context: host.context_token().clone(),
            token: HorseMenuToken {
                instance,
                revision: 0,
            },
            active: true,
            columns,
            sync: MenuSync::new_horse_inventory(window, columns)?,
            drag_status: 0,
            drag_type: 0,
            drag_slots: BTreeSet::new(),
        };
        menu.transaction(menu.token, HorseMenuAction::Open, host, |menu, host| {
            let mut w = Writer::new();
            w.u8(window)
                .varint(i32::from(columns))
                .i32(host.entity_id());
            host.stage_sync_events(
                vec![SyncEvent::Packet(Packet {
                    id: HORSE_SCREEN_OPEN_PACKET,
                    payload: w.into_inner(),
                })],
                &menu.phase(host),
            )?;
            host.horse_container_start_open(&menu.phase(host))?;
            let events = menu
                .sync
                .initialize(&menu.top(host), host.inventory(), &[])?;
            host.stage_sync_events(events, &menu.phase(host))
        })?;
        Ok(menu)
    }
    fn staged_copy(&self) -> Self {
        Self {
            binding: self.binding.clone(),
            context: self.context.clone(),
            token: self.token,
            active: self.active,
            columns: self.columns,
            sync: self.sync.clone(),
            drag_status: self.drag_status,
            drag_type: self.drag_type,
            drag_slots: self.drag_slots.clone(),
        }
    }
    pub fn token(&self) -> HorseMenuToken {
        self.token
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
    pub fn inventory_columns(&self) -> u8 {
        self.columns
    }
    pub fn top_slots(&self) -> usize {
        2 + 3 * usize::from(self.columns)
    }
    pub fn menu_slots(&self) -> usize {
        self.top_slots() + 36
    }
    pub fn slot_active<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &self,
        index: usize,
        host: &H,
    ) -> Result<bool> {
        self.validate(self.token, host)?;
        ensure!(index < self.menu_slots(), "invalid horse menu slot");
        match index {
            0 => host.is_saddleable(),
            1 => host.can_use_body_slot(),
            _ => Ok(true),
        }
    }
    pub fn transfer_inventory_state(&self, window0: &mut PlayerContainer) -> Result<()> {
        self.sync.transfer_inventory_state(window0)
    }
    fn phase<H: HorseMenuHost<Binding = K, ContextToken = C>>(&self, host: &H) -> HorsePhase {
        HorsePhase {
            horse_inventory: host.horse_inventory().to_vec(),
            body_armor: host.body_armor().clone(),
            player_inventory: host.inventory().clone(),
            carried: self.carried().clone(),
        }
    }
    fn top<H: HorseMenuHost<Binding = K, ContextToken = C>>(&self, host: &H) -> Vec<ItemStack> {
        let mut out = Vec::with_capacity(self.top_slots());
        out.push(host.horse_inventory()[0].clone());
        out.push(host.body_armor().clone());
        out.extend_from_slice(&host.horse_inventory()[1..]);
        out
    }
    fn validate<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &self,
        token: HorseMenuToken,
        host: &H,
    ) -> Result<()> {
        ensure!(
            self.active && token == self.token,
            "stale/closed horse UI action"
        );
        ensure!(
            &self.binding == host.binding() && &self.context == host.context_token(),
            "horse captured owners/context changed"
        );
        ensure!(
            host.horse_inventory().len() == 1 + 3 * usize::from(self.columns),
            "captured horse container size changed"
        );
        Ok(())
    }
    fn require_components<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &self,
        host: &H,
        click: Option<&ContainerClick>,
    ) -> Result<()> {
        for item in host
            .horse_inventory()
            .iter()
            .chain(std::iter::once(host.body_armor()))
            .chain(host.inventory().slots())
            .chain(std::iter::once(host.inventory().carried()))
            .chain(std::iter::once(self.carried()))
        {
            require_component_context(item)?;
            ensure!(
                inventory::item_name(item.item_id) != Some("bundle"),
                "horse bundle overrides require actual bundle owner"
            );
        }
        if let Some(click) = click {
            require_component_context(&click.carried)?;
            for item in click.changed_slots.values() {
                require_component_context(item)?;
            }
        }
        Ok(())
    }
    fn transaction<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        token: HorseMenuToken,
        action: HorseMenuAction,
        host: &mut H,
        f: impl FnOnce(&mut Self, &mut H) -> Result<()>,
    ) -> Result<()> {
        self.validate(token, host)?;
        self.require_components(host, None)?;
        let mut stage = host.begin_stage(token, action)?;
        self.validate(token, &stage)?;
        self.require_components(&stage, None)?;
        stage.preflight(action, &self.context)?;
        let mut next = self.staged_copy();
        f(&mut next, &mut stage)?;
        ensure!(
            &next.binding == stage.binding() && &next.context == stage.context_token(),
            "horse owners/context changed during callback"
        );
        ensure!(
            stage.horse_inventory().len() == 1 + 3 * usize::from(next.columns),
            "horse container resized during callback"
        );
        next.token.revision = next
            .token
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("horse UI revision exhausted"))?;
        host.publish_stage(stage, token)?;
        *self = next;
        Ok(())
    }
    pub fn still_valid<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &self,
        host: &H,
    ) -> Result<bool> {
        self.validate(self.token, host)?;
        Ok(!host.has_inventory_changed()?
            && host.horse_container_still_valid()?
            && host.body_access_still_valid()?
            && host.mount_is_alive()?
            && host.player_can_interact_with_mount(4.0)?)
    }
    pub fn broadcast_changes<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        self.transaction(
            self.token,
            HorseMenuAction::Broadcast,
            host,
            |menu, host| menu.broadcast(host),
        )
    }
    fn broadcast<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        let events = self
            .sync
            .broadcast_changes(&self.top(host), host.inventory(), &[])?;
        host.stage_sync_events(events, &self.phase(host))
    }
    pub fn click<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        token: HorseMenuToken,
        click: &ContainerClick,
        host: &mut H,
    ) -> Result<()> {
        ensure!(
            click.window_id == self.window_id() && click.slot < self.menu_slots() as i16,
            "invalid horse window/slot"
        );
        self.validate(token, host)?;
        self.require_components(host, Some(click))?;
        self.transaction(
            token,
            HorseMenuAction::Click(click.kind),
            host,
            |menu, host| {
                ensure!(menu.still_valid(host)?, "horse menu no longer valid");
                let before = menu.state_id();
                menu.sync.suppress_remote_updates(true);
                menu.do_click(click, host)?;
                let events = menu.sync.finish_click(
                    click,
                    before,
                    &menu.top(host),
                    host.inventory(),
                    &[],
                )?;
                host.stage_sync_events(events, &menu.phase(host))
            },
        )
    }
    /// Source removed() handles cursor, then horseContainer.stopOpen. Saddle,
    /// storage and BODY equipment remain with the mount; no input drain occurs.
    /// Actual close-window packet precedes this callback and window0 baseline
    /// transfer follows it in the central player/menu owner.
    pub fn close<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        token: HorseMenuToken,
        context: MenuCloseContext,
        host: &mut H,
    ) -> Result<()> {
        self.transaction(
            token,
            HorseMenuAction::Close(context),
            host,
            |menu, host| {
                menu.reset_drag();
                let mut cursor = menu.carried().clone();
                if !cursor.is_empty() {
                    match context {
                        MenuCloseContext::AliveConnected => {
                            host.place_item_back(&mut cursor, &menu.phase(host))?
                        }
                        MenuCloseContext::DeadOrDisconnected => host.stage_drop(
                            DroppedStack {
                                stack: cursor,
                                around: false,
                                trace_item: false,
                            },
                            &menu.phase(host),
                        )?,
                    }
                    menu.sync.set_carried(ItemStack::empty());
                }
                host.horse_container_stop_open(&menu.phase(host))?;
                menu.active = false;
                Ok(())
            },
        )
    }
    fn reset_drag(&mut self) {
        self.drag_status = 0;
        self.drag_slots.clear();
    }
    fn player_slot(&self, index: usize) -> usize {
        9 + index - self.top_slots()
    }
    fn horse_slot(&self, index: usize) -> usize {
        if index == 0 {
            0
        } else {
            index - 1
        }
    }
    fn slot<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &self,
        index: usize,
        host: &H,
    ) -> ItemStack {
        if index == 1 {
            host.body_armor().clone()
        } else if index < self.top_slots() {
            host.horse_inventory()[self.horse_slot(index)].clone()
        } else {
            host.inventory()
                .slot(self.player_slot(index))
                .unwrap()
                .clone()
        }
    }
    fn may_place<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &self,
        index: usize,
        item: &ItemStack,
        host: &H,
    ) -> Result<bool> {
        match index {
            0 => Ok(inventory::item_name(item.item_id) == Some("saddle")
                && self.slot(0, host).is_empty()
                && host.is_saddleable()?),
            1 => host.is_body_armor_item(item),
            _ => Ok(true),
        }
    }
    fn may_pickup<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &self,
        index: usize,
        host: &H,
    ) -> Result<bool> {
        Ok(index != 1
            || host.body_armor().is_empty()
            || host.player_is_creative()
            || !host.prevents_armor_change(host.body_armor())?)
    }
    fn limit<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &self,
        index: usize,
        item: &ItemStack,
        host: &H,
    ) -> Result<i32> {
        let cap = if index == 1 {
            1
        } else if index < self.top_slots() {
            host.horse_container_maximum()
        } else {
            99
        };
        ensure!(cap > 0, "unsupported horse/player slot maximum");
        Ok(item.max_stack_size()?.min(cap))
    }
    fn mutate_raw<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        index: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if index == 1 {
            host.mutate_body_stack_in_place(item)
        } else if index < self.top_slots() {
            host.mutate_horse_stack_in_place(self.horse_slot(index), item)
        } else {
            host.inventory_mut()
                .set_slot_authoritative(self.player_slot(index), item);
            Ok(())
        }
    }
    fn changed<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        index: usize,
        host: &mut H,
    ) -> Result<()> {
        if index == 1 {
            host.body_access_set_changed(&self.phase(host))
        } else if index < self.top_slots() {
            host.horse_container_set_changed(&self.phase(host))
        } else {
            host.player_inventory_set_changed(&self.phase(host))
        }
    }
    fn set_slot_with_old<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        index: usize,
        item: ItemStack,
        old: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        if index == 1 {
            host.on_equip_body(&old, &item, &self.phase(host))?;
            host.set_body_armor_item(item, &self.phase(host))?;
        } else if index < self.top_slots() {
            host.horse_container_set_item(self.horse_slot(index), item, &self.phase(host))?;
        } else {
            self.mutate_raw(index, item, host)?;
        }
        self.changed(index, host) // Slot.setChanged, BODY access's source no-op
    }
    fn set_slot<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        index: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        let old = self.slot(index, host);
        self.set_slot_with_old(index, item, old, host)
    }
    fn try_remove<H: HorseMenuHost<Binding = K, ContextToken = C>>(
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
        // crq.tryRemove refuses a destination smaller than the current stack
        // when allowModification (mayPickup && mayPlace(current)) is false.
        // Occupied saddle slots reject mayPlace even for matching saddles.
        if !self.may_place(index, &source, host)? && maximum < source.count {
            return Ok(ItemStack::empty());
        }
        let taken = counted(&source, amount.min(maximum).min(source.count));
        if taken.is_empty() {
            return Ok(taken);
        }
        self.mutate_raw(
            index,
            counted(&source, source.count.wrapping_sub(taken.count)),
            host,
        )?;
        if index != 1 && index < self.top_slots() {
            self.changed(index, host)?;
        }
        if self.slot(index, host).is_empty() {
            self.set_slot_with_old(index, ItemStack::empty(), taken.clone(), host)?;
        }
        Ok(taken)
    }
    fn safe_take<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        index: usize,
        amount: i32,
        maximum: i32,
        host: &mut H,
    ) -> Result<ItemStack> {
        let taken = self.try_remove(index, amount, maximum, host)?;
        if !taken.is_empty() {
            self.changed(index, host)?;
        }
        Ok(taken)
    }
    fn drop<H: HorseMenuHost<Binding = K, ContextToken = C>>(
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
    fn do_click<H: HorseMenuHost<Binding = K, ContextToken = C>>(
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
    fn pickup<H: HorseMenuHost<Binding = K, ContextToken = C>>(
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
        if !source.is_empty() && !self.may_pickup(index, host)? {
            return self.changed(index, host);
        }
        if source.is_empty() {
            if !cursor.is_empty() && self.may_place(index, &cursor, host)? {
                let n = (if button == 0 { cursor.count } else { 1 })
                    .min(self.limit(index, &cursor, host)?);
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
                self.changed(index, host)?;
            }
        } else if self.may_place(index, &cursor, host)? {
            if source.same_item_same_components(&cursor)? {
                let n = (if button == 0 { cursor.count } else { 1 })
                    .min(self.limit(index, &cursor, host)? - source.count);
                // Slot.safeInsert retains negative capacity for an admitted
                // overfull stack: shrink(cursor,n) grows that cursor while
                // grow(source,n) restores the slot to its limit. It also calls
                // setByPlayer when a full slot accepts exactly zero items.
                self.sync
                    .set_carried(counted(&cursor, cursor.count.wrapping_sub(n)));
                // Slot.safeInsert grows the actual target object BEFORE
                // setByPlayer reads its old-stack argument. ArmorSlot's
                // onEquipItem therefore sees that already-grown BODY owner.
                let grown = counted(&source, source.count.wrapping_add(n));
                self.mutate_raw(index, grown.clone(), host)?;
                self.set_slot(index, grown, host)?;
            } else if cursor.count <= self.limit(index, &cursor, host)? {
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
                self.changed(index, host)?;
            }
        }
        self.changed(index, host) // unconditional Slot.setChanged after Pickup
    }
    fn move_to<H: HorseMenuHost<Binding = K, ContextToken = C>>(
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
                        .min((self.limit(index, &old, host)? - old.count).max(0));
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
                if self.slot(index, host).is_empty() && self.may_place(index, stack, host)? {
                    let n = stack.count.min(self.limit(index, stack, host)?);
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
    fn quick_move<H: HorseMenuHost<Binding = K, ContextToken = C>>(
        &mut self,
        index: usize,
        host: &mut H,
    ) -> Result<bool> {
        let source = self.slot(index, host);
        let mut left = source.clone();
        if source.is_empty() || !self.may_pickup(index, host)? {
            return Ok(false);
        }
        let top = self.top_slots();
        let moved = if index < top {
            self.move_to(index, &mut left, top..self.menu_slots(), true, host)?
        } else if self.may_place(1, &left, host)? && self.slot(1, host).is_empty() {
            self.move_to(index, &mut left, 1..2, false, host)?
        } else if self.may_place(0, &left, host)? {
            self.move_to(index, &mut left, 0..1, false, host)?
        } else if top > 1 && self.move_to(index, &mut left, 2..top, false, host)? {
            true
        } else {
            // Source successfully moves between player main/hotbar, then
            // RETURNS EMPTY before source Slot.set/changed or onTake. That
            // return also terminates AbstractContainerMenu's repeat loop.
            if index >= top + 27 && index < top + 36 {
                self.move_to(index, &mut left, top..top + 27, false, host)?;
            } else if index >= top && index < top + 27 {
                self.move_to(index, &mut left, top + 27..top + 36, false, host)?;
            }
            return Ok(false);
        };
        if !moved {
            return Ok(false);
        }
        if left.is_empty() {
            self.set_slot(index, ItemStack::empty(), host)?;
        } else {
            self.changed(index, host)?;
        }
        // HorseInventoryMenu does not call Slot.onTake or onQuickCraft.
        Ok(true)
    }
    fn swap<H: HorseMenuHost<Binding = K, ContextToken = C>>(
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
                .set_slot_authoritative(player_index, source);
            self.set_slot(index, ItemStack::empty(), host)?;
            self.changed(index, host)?;
        } else if self.may_place(index, &held, host)? {
            let maximum = self.limit(index, &held, host)?;
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
                self.changed(index, host)?;
                let mut displaced =
                    if index >= self.top_slots() && self.player_slot(index) == player_index {
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
                self.changed(index, host)?;
            }
        }
        Ok(())
    }
    fn pickup_all<H: HorseMenuHost<Binding = K, ContextToken = C>>(
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
            (0..self.menu_slots()).collect::<Vec<_>>()
        } else {
            (0..self.menu_slots()).rev().collect()
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
                    .set_carried(counted(&cursor, cursor.count.wrapping_add(taken.count)));
            }
        }
        Ok(())
    }
    fn quickcraft<H: HorseMenuHost<Binding = K, ContextToken = C>>(
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
                        && self.may_place(index, cursor, host)?
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
                let order = host.quickcraft_iteration_order(&self.context, &selected)?;
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
                        || !self.may_place(index, &cursor, host)?
                        || self.drag_type != 2 && cursor.count < self.drag_slots.len() as i32
                    {
                        continue;
                    }
                    let count = each
                        .wrapping_add(old.count)
                        .min(self.limit(index, &cursor, host)?);
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
    use crate::{crafting::RecipeBook, protocol::Reader, random::LegacyRandomSource};
    use std::collections::BTreeMap;

    #[derive(Clone, Copy)]
    enum Family {
        Horse,
        Donkey,
        Llama,
        Camel,
    }
    #[derive(Clone)]
    struct Record {
        name: &'static str,
        phase: HorsePhase,
        events: Vec<SyncEvent>,
        old: ItemStack,
        new: ItemStack,
    }
    /// Restricted recording owner used only by these native contract checks.
    /// It is not a production mount, item registry, Level or equipment owner.
    #[derive(Clone)]
    struct Host {
        binding: (u64, u64, u64),
        context: u64,
        family: Family,
        columns: u8,
        inventory: PlayerInventory,
        horse: Vec<ItemStack>,
        body: ItemStack,
        records: Vec<Record>,
        creative: bool,
        instabuild: bool,
        armor_locked: bool,
        replaced: bool,
        alive: bool,
        reach: bool,
        generation: u64,
        base_generation: u64,
        random: LegacyRandomSource,
        fail_body: bool,
        start: i32,
        stop: i32,
    }
    impl Host {
        fn new(family: Family, columns: u8) -> Self {
            Self {
                binding: (1, 2, 3),
                context: 7,
                family,
                columns,
                inventory: PlayerInventory::default(),
                horse: vec![ItemStack::empty(); 1 + 3 * usize::from(columns)],
                body: ItemStack::empty(),
                records: vec![],
                creative: false,
                instabuild: false,
                armor_locked: false,
                replaced: false,
                alive: true,
                reach: true,
                generation: 0,
                base_generation: 0,
                random: LegacyRandomSource::new(0),
                fail_body: false,
                start: 0,
                stop: 0,
            }
        }
        fn record(&mut self, name: &'static str, phase: &HorsePhase) {
            self.records.push(Record {
                name,
                phase: phase.clone(),
                events: vec![],
                old: ItemStack::empty(),
                new: ItemStack::empty(),
            });
        }
        fn live_phase(&self, phase: &HorsePhase) -> HorsePhase {
            HorsePhase {
                horse_inventory: self.horse.clone(),
                body_armor: self.body.clone(),
                player_inventory: self.inventory.clone(),
                carried: phase.carried.clone(),
            }
        }
    }
    impl HorseMenuHost for Host {
        type Binding = (u64, u64, u64);
        type ContextToken = u64;
        fn binding(&self) -> &Self::Binding {
            &self.binding
        }
        fn context_token(&self) -> &u64 {
            &self.context
        }
        fn begin_stage(&self, _: HorseMenuToken, _: HorseMenuAction) -> Result<Self> {
            let mut stage = self.clone();
            stage.base_generation = self.generation;
            Ok(stage)
        }
        fn publish_stage(&mut self, mut stage: Self, _: HorseMenuToken) -> Result<()> {
            ensure!(
                stage.base_generation == self.generation,
                "fixture owner revision changed"
            );
            stage.generation += 1;
            *self = stage;
            Ok(())
        }
        fn preflight(&mut self, _: HorseMenuAction, context: &u64) -> Result<()> {
            ensure!(*context == self.context, "fixture context changed");
            Ok(())
        }
        fn inventory_columns(&self) -> Result<u8> {
            Ok(self.columns)
        }
        fn entity_id(&self) -> i32 {
            123
        }
        fn inventory(&self) -> &PlayerInventory {
            &self.inventory
        }
        fn inventory_mut(&mut self) -> &mut PlayerInventory {
            &mut self.inventory
        }
        fn horse_inventory(&self) -> &[ItemStack] {
            &self.horse
        }
        fn body_armor(&self) -> &ItemStack {
            &self.body
        }
        fn horse_container_maximum(&self) -> i32 {
            99
        }
        fn is_saddleable(&self) -> Result<bool> {
            Ok(!matches!(self.family, Family::Llama))
        }
        fn can_use_body_slot(&self) -> Result<bool> {
            Ok(matches!(self.family, Family::Horse | Family::Llama))
        }
        fn is_body_armor_item(&self, stack: &ItemStack) -> Result<bool> {
            Ok(match self.family {
                Family::Horse => inventory::item_name(stack.item_id) == Some("iron_horse_armor"),
                Family::Llama => inventory::item_name(stack.item_id) == Some("white_carpet"),
                _ => false,
            })
        }
        fn prevents_armor_change(&self, _: &ItemStack) -> Result<bool> {
            Ok(self.armor_locked)
        }
        fn player_is_creative(&self) -> bool {
            self.creative
        }
        fn instabuild(&self) -> bool {
            self.instabuild
        }
        fn has_inventory_changed(&self) -> Result<bool> {
            Ok(self.replaced)
        }
        fn horse_container_still_valid(&self) -> Result<bool> {
            Ok(true)
        }
        fn body_access_still_valid(&self) -> Result<bool> {
            Ok(true)
        }
        fn mount_is_alive(&self) -> Result<bool> {
            Ok(self.alive)
        }
        fn player_can_interact_with_mount(&self, range: f64) -> Result<bool> {
            assert_eq!(range, 4.0);
            Ok(self.reach)
        }
        fn mutate_horse_stack_in_place(&mut self, index: usize, item: ItemStack) -> Result<()> {
            self.horse[index] = item;
            Ok(())
        }
        fn mutate_body_stack_in_place(&mut self, item: ItemStack) -> Result<()> {
            self.body = item;
            Ok(())
        }
        fn horse_container_set_item(
            &mut self,
            index: usize,
            mut item: ItemStack,
            phase: &HorsePhase,
        ) -> Result<()> {
            if !item.is_empty() {
                item.count = item.count.min(item.max_stack_size()?.min(99));
            }
            self.horse[index] = item;
            self.horse_container_set_changed(&self.live_phase(phase))
        }
        fn set_body_armor_item(&mut self, item: ItemStack, phase: &HorsePhase) -> Result<()> {
            self.record("body-set", phase);
            self.body = item;
            ensure!(!self.fail_body, "fixture equipment failure");
            Ok(())
        }
        fn on_equip_body(
            &mut self,
            old: &ItemStack,
            new: &ItemStack,
            phase: &HorsePhase,
        ) -> Result<()> {
            self.record("equip", phase);
            let event = self.records.last_mut().unwrap();
            event.old = old.clone();
            event.new = new.clone();
            // A real owner can consume entity RNG and world work here. This
            // labeled recording effect proves whole-stage rollback only.
            self.random.next_i64();
            Ok(())
        }
        fn horse_container_set_changed(&mut self, phase: &HorsePhase) -> Result<()> {
            self.record("horse-dirty", phase);
            Ok(())
        }
        fn body_access_set_changed(&mut self, phase: &HorsePhase) -> Result<()> {
            self.record("body-dirty", phase);
            Ok(())
        }
        fn player_inventory_set_changed(&mut self, phase: &HorsePhase) -> Result<()> {
            self.record("player-dirty", phase);
            Ok(())
        }
        fn horse_container_start_open(&mut self, phase: &HorsePhase) -> Result<()> {
            self.start += 1;
            self.record("start", phase);
            Ok(())
        }
        fn horse_container_stop_open(&mut self, phase: &HorsePhase) -> Result<()> {
            self.stop += 1;
            self.record("stop", phase);
            Ok(())
        }
        fn stage_sync_events(&mut self, events: Vec<SyncEvent>, phase: &HorsePhase) -> Result<()> {
            self.record("sync", phase);
            self.records.last_mut().unwrap().events = events;
            Ok(())
        }
        fn stage_drop(&mut self, item: DroppedStack, phase: &HorsePhase) -> Result<()> {
            self.record(
                if item.trace_item {
                    "throw"
                } else {
                    "close-drop"
                },
                phase,
            );
            self.records.last_mut().unwrap().new = item.stack;
            Ok(())
        }
        fn add_to_player_inventory(
            &mut self,
            stack: &mut ItemStack,
            phase: &HorsePhase,
        ) -> Result<bool> {
            // Restricted insertion path used by these recording scenarios.
            for index in 9..45 {
                if self.inventory.slot(index).unwrap().is_empty() {
                    self.inventory
                        .set_slot_authoritative(index, std::mem::take(stack));
                    self.record("add", phase);
                    return Ok(true);
                }
            }
            Ok(false)
        }
        fn place_item_back(&mut self, stack: &mut ItemStack, phase: &HorsePhase) -> Result<()> {
            self.record("return", phase);
            let mut outcome = crate::container::ClickOutcome::default();
            crate::container::return_to_inventory_on_menu_close(
                &mut self.inventory,
                stack,
                &mut outcome,
            )?;
            for item in outcome.drops {
                self.stage_drop(item, phase)?;
            }
            Ok(())
        }
        fn quickcraft_iteration_order(
            &self,
            context: &u64,
            selected: &[usize],
        ) -> Result<Vec<usize>> {
            ensure!(*context == self.context, "fixture identity scope changed");
            Ok(selected.iter().rev().copied().collect())
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn packet(
        menu: &HorseInventoryMenu<(u64, u64, u64), u64>,
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
        menu: &mut HorseInventoryMenu<(u64, u64, u64), u64>,
        host: &mut Host,
        slot: i16,
        button: i8,
        kind: ClickType,
    ) -> Result<()> {
        menu.click(menu.token(), &packet(menu, slot, button, kind), host)
    }
    #[test]
    fn opening_separate_equipment_owner_and_world_failure_are_atomic() -> Result<()> {
        let mut host = Host::new(Family::Horse, 0);
        host.inventory
            .set_slot_authoritative(9, item("iron_horse_armor", 1));
        host.inventory.set_slot_authoritative(10, item("saddle", 1));
        let mut menu = HorseInventoryMenu::open(4, &mut host)?;
        assert_eq!(menu.menu_slots(), 38);
        assert_eq!(host.start, 1);
        assert_eq!(
            host.records.iter().map(|e| e.name).collect::<Vec<_>>(),
            ["sync", "start", "sync"]
        );
        let open = match &host.records[0].events[0] {
            SyncEvent::Packet(packet) => packet,
            _ => panic!("expected horse opening"),
        };
        assert_eq!(open.id, 0x20);
        let mut r = Reader::new(&open.payload);
        assert_eq!(r.u8()?, 4);
        assert_eq!(r.varint()?, 0);
        assert_eq!(r.i32()?, 123);
        r.finish()?;
        let contents = host.records[2]
            .events
            .iter()
            .find_map(|e| match e {
                SyncEvent::Packet(p) if p.id == 0x13 => Some(p),
                _ => None,
            })
            .unwrap();
        let mut r = Reader::new(&contents.payload);
        r.u8()?;
        r.varint()?;
        assert_eq!(r.varint()?, 38);
        let token = menu.token();
        let generation = host.generation;
        let random = host.random.state48();
        let n = host.records.len();
        host.fail_body = true;
        assert!(act(&mut menu, &mut host, 2, 0, ClickType::QuickMove).is_err());
        assert_eq!(menu.token(), token);
        assert_eq!(host.generation, generation);
        assert_eq!(host.random.state48(), random);
        assert_eq!(host.records.len(), n);
        assert_eq!(
            host.inventory.slot(9).unwrap(),
            &item("iron_horse_armor", 1)
        );
        assert!(host.body.is_empty());
        host.fail_body = false;
        host.records.clear();
        act(&mut menu, &mut host, 2, 0, ClickType::QuickMove)?;
        let equip = host.records.iter().position(|e| e.name == "equip").unwrap();
        let set = host
            .records
            .iter()
            .position(|e| e.name == "body-set")
            .unwrap();
        assert!(equip < set);
        assert!(host.records[equip].phase.body_armor.is_empty());
        assert!(host.records[equip]
            .phase
            .player_inventory
            .slot(9)
            .unwrap()
            .is_empty());
        assert_eq!(host.body, item("iron_horse_armor", 1));
        assert!(host.horse[0].is_empty());
        act(&mut menu, &mut host, 3, 0, ClickType::QuickMove)?;
        assert_eq!(host.horse[0], item("saddle", 1));
        assert_eq!(host.body, item("iron_horse_armor", 1));
        host.armor_locked = true;
        host.records.clear();
        act(&mut menu, &mut host, 1, 0, ClickType::Pickup)?;
        assert!(menu.carried().is_empty());
        assert_eq!(host.body, item("iron_horse_armor", 1));
        host.creative = true;
        act(&mut menu, &mut host, 1, 0, ClickType::Pickup)?;
        assert_eq!(menu.carried(), &item("iron_horse_armor", 1));
        assert!(host.body.is_empty());
        // ArmorSlot gets the removed old item while the actual BODY access has
        // already been split empty, before setter/equipment callbacks run.
        let removal = host
            .records
            .iter()
            .rev()
            .find(|e| e.name == "equip")
            .unwrap();
        assert_eq!(removal.old, item("iron_horse_armor", 1));
        assert!(removal.new.is_empty());
        assert!(removal.phase.body_armor.is_empty());
        // Native observed overfull BODY: safeInsert has signed capacity -1.
        // It grows the actual target before ArmorSlot fetches its old argument.
        host.body = item("iron_horse_armor", 2);
        host.records.clear();
        act(&mut menu, &mut host, 1, 0, ClickType::Pickup)?;
        assert_eq!(menu.carried().count, 2);
        assert_eq!(host.body.count, 1);
        let equip = host.records.iter().find(|e| e.name == "equip").unwrap();
        assert_eq!(
            (
                equip.phase.body_armor.count,
                equip.old.count,
                equip.new.count
            ),
            (1, 1, 1)
        );
        Ok(())
    }
    #[test]
    fn chested_storage_family_predicates_and_close_preserve_mount_contents() -> Result<()> {
        let mut host = Host::new(Family::Llama, 3);
        host.inventory
            .set_slot_authoritative(9, item("white_carpet", 5));
        host.inventory.set_slot_authoritative(10, item("saddle", 1));
        let mut menu = HorseInventoryMenu::open(5, &mut host)?;
        assert_eq!((menu.top_slots(), menu.menu_slots()), (11, 47));
        assert!(!menu.slot_active(0, &host)?);
        assert!(menu.slot_active(1, &host)?);
        act(&mut menu, &mut host, 11, 0, ClickType::QuickMove)?;
        assert_eq!(host.body, item("white_carpet", 1));
        assert_eq!(host.horse[1], item("white_carpet", 4));
        assert!(host.inventory.slot(9).unwrap().is_empty());
        act(&mut menu, &mut host, 12, 0, ClickType::QuickMove)?;
        assert!(host.horse[0].is_empty());
        assert_eq!(host.horse[2], item("saddle", 1));
        act(&mut menu, &mut host, 2, 1, ClickType::Pickup)?;
        assert_eq!(menu.carried(), &item("white_carpet", 2));
        let mount = host.horse.clone();
        let body = host.body.clone();
        host.records.clear();
        menu.close(
            menu.token(),
            MenuCloseContext::DeadOrDisconnected,
            &mut host,
        )?;
        assert_eq!(host.horse, mount);
        assert_eq!(host.body, body);
        assert_eq!((host.start, host.stop), (1, 1));
        assert_eq!(
            host.records.iter().map(|e| e.name).collect::<Vec<_>>(),
            ["close-drop", "stop"]
        );
        assert_eq!(host.records[0].phase.carried.count, 2);
        assert!(host.records[1].phase.carried.is_empty());
        assert!(menu
            .click(
                menu.token(),
                &packet(&menu, 0, 0, ClickType::Pickup),
                &mut host
            )
            .is_err());
        // Donkey BODY remains a distinct inactive equipment access; all15
        // chest slots remain real horse-container slots, not UI-owned copies.
        let mut donkey = Host::new(Family::Donkey, 5);
        let mut menu = HorseInventoryMenu::open(6, &mut donkey)?;
        assert_eq!((menu.top_slots(), menu.menu_slots()), (17, 53));
        assert!(!menu.slot_active(1, &donkey)?);
        donkey
            .inventory
            .set_slot_authoritative(9, item("iron_horse_armor", 1));
        act(&mut menu, &mut donkey, 17, 0, ClickType::QuickMove)?;
        assert!(donkey.body.is_empty());
        assert_eq!(donkey.horse[1], item("iron_horse_armor", 1));
        Ok(())
    }
    #[test]
    fn occupied_saddle_refuses_insufficient_cursor_capacity_before_removal() -> Result<()> {
        let mut host = Host::new(Family::Horse, 0);
        let mut saddle = item("saddle", 2);
        let mut maximum = Writer::new();
        maximum.varint(99);
        saddle.components.push(crate::inventory::Component {
            type_id: 1,
            data: maximum.into_inner(),
        });
        assert_eq!(saddle.max_stack_size()?, 99);
        host.horse[0] = saddle.clone();
        host.inventory
            .set_slot_authoritative(9, counted(&saddle, 98));
        let mut menu = HorseInventoryMenu::open(8, &mut host)?;
        act(&mut menu, &mut host, 2, 0, ClickType::Pickup)?;
        host.records.clear();
        let random = host.random.state48();
        act(&mut menu, &mut host, 0, 0, ClickType::Pickup)?;
        assert_eq!(host.horse[0].count, 2);
        assert_eq!(menu.carried().count, 98);
        assert_eq!(host.random.state48(), random);
        // No split/set/equipment callback occurs. PICKUP still invokes its
        // unconditional final Slot.setChanged on the unchanged saddle owner.
        assert_eq!(
            host.records.iter().map(|e| e.name).collect::<Vec<_>>(),
            ["horse-dirty", "sync"]
        );
        assert_eq!(host.records[0].phase.horse_inventory[0].count, 2);
        assert_eq!(host.records[0].phase.carried.count, 98);
        host.records.clear();
        act(&mut menu, &mut host, 3, 0, ClickType::PickupAll)?;
        assert_eq!(host.horse[0].count, 2);
        assert_eq!(menu.carried().count, 98);
        assert!(!host.records.iter().any(|e| e.name == "horse-dirty"));
        Ok(())
    }

    #[test]
    fn player_fallback_ordinary_modes_and_reused_ui_context_guards_follow_source() -> Result<()> {
        let mut host = Host::new(Family::Camel, 0);
        host.inventory.set_slot_authoritative(9, item("dirt", 8));
        let mut menu = HorseInventoryMenu::open(7, &mut host)?;
        host.records.clear();
        act(&mut menu, &mut host, 2, 0, ClickType::QuickMove)?;
        assert_eq!(host.inventory.slot(36).unwrap(), &item("dirt", 8));
        assert!(host.inventory.slot(9).unwrap().is_empty());
        // Empty-target setter + target Slot.changed only. Source fallback
        // returns EMPTY before source cleanup/dirty and has no Slot.onTake.
        assert_eq!(
            host.records
                .iter()
                .filter(|e| e.name == "player-dirty")
                .count(),
            2
        );
        act(&mut menu, &mut host, 29, 0, ClickType::Pickup)?;
        for (slot, button) in [(-999, 0), (2, 1), (3, 1), (-999, 2)] {
            act(&mut menu, &mut host, slot, button, ClickType::QuickCraft)?;
        }
        assert_eq!(host.inventory.slot(9).unwrap().count, 4);
        assert_eq!(host.inventory.slot(10).unwrap().count, 4);
        assert!(menu.carried().is_empty());
        act(&mut menu, &mut host, 2, 1, ClickType::Pickup)?;
        act(&mut menu, &mut host, 4, 0, ClickType::PickupAll)?;
        assert_eq!(menu.carried().count, 8);
        act(&mut menu, &mut host, -999, 1, ClickType::Pickup)?;
        let drop = host
            .records
            .iter()
            .rev()
            .find(|e| e.name == "throw")
            .unwrap();
        assert_eq!((drop.new.count, drop.phase.carried.count), (1, 7));
        act(&mut menu, &mut host, 2, 0, ClickType::Pickup)?;
        act(&mut menu, &mut host, 2, 0, ClickType::Swap)?;
        assert_eq!(host.inventory.slot(36).unwrap().count, 7);
        host.instabuild = true;
        act(&mut menu, &mut host, 29, 2, ClickType::Clone)?;
        assert_eq!(menu.carried().count, 64);
        act(&mut menu, &mut host, -999, 0, ClickType::Pickup)?;
        let drop = host
            .records
            .iter()
            .rev()
            .find(|e| e.name == "throw")
            .unwrap();
        assert_eq!((drop.new.count, drop.phase.carried.count), (64, 64));
        act(&mut menu, &mut host, 29, 0, ClickType::Throw)?;
        assert_eq!(host.inventory.slot(36).unwrap().count, 6);
        // Unsupported nested ItemStack identity is refused before action or
        // world staging, including client predictions rather than only owners.
        let mut food = item("mushroom_stew", 1);
        food.components.push(crate::inventory::Component {
            type_id: 20,
            data: food.effective_component_bytes(20).unwrap().to_vec(),
        });
        let old = menu.token();
        let generation = host.generation;
        let mut prediction = packet(&menu, 0, 0, ClickType::Pickup);
        prediction.carried = food;
        assert!(menu.click(old, &prediction, &mut host).is_err());
        assert_eq!(host.generation, generation);
        assert_eq!(menu.token(), old);
        menu.broadcast_changes(&mut host)?;
        assert!(menu
            .click(old, &packet(&menu, 0, 0, ClickType::Pickup), &mut host)
            .is_err());
        let mut reopened = HorseInventoryMenu::open(7, &mut host)?;
        assert!(reopened
            .click(
                menu.token(),
                &packet(&reopened, 0, 0, ClickType::Pickup),
                &mut host
            )
            .is_err());
        host.context += 1;
        assert!(menu.still_valid(&host).is_err());
        host.context -= 1;
        host.replaced = true;
        assert!(!menu.still_valid(&host)?);
        assert!(act(&mut menu, &mut host, 0, 0, ClickType::Pickup).is_err());
        host.replaced = false;
        host.binding.1 += 1;
        assert!(menu.still_valid(&host).is_err());
        Ok(())
    }
}
