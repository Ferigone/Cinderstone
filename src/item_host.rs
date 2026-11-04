//! Inventory and world-effect adapter for the authoritative item coordinator.
//!
//! Gameplay callbacks remain required host operations. In particular this
//! adapter does not implement ServerPlayer.tick, statistics/advancements or
//! game-event listeners. Menu publishers must share the coordinator's ordered
//! event stream so its already-published Collect precedes private menu packets.
use crate::{
    entity::{EntityRegistry, PlayerState},
    entity_move::ProfileEvent,
    entity_state::EntityPhysicalState,
    geometry::{Aabb, Vec3, VoxelShape},
    fluid_physics::FluidState,
    inventory::{ItemStack, PlayerInventory},
    item_effects,
    item_entity::{ItemState, Pickup},
    metadata::MetadataValue,
    player_inventory::SharedContainer,
    protocol::Packet,
    runtime_entities::{EffectDelivery, PickupPlayer, RuntimeEntityHost, RuntimeEntityQuerySound},
    runtime_entity_index::EntityHandle,
    runtime_item_world::RuntimeItemEffect,
    runtime_world::RuntimeWorld,
};
use anyhow::{ensure, Context, Result};
use std::collections::BTreeMap;

/// These methods have no default no-op implementations. The embedding runtime
/// must execute its actual callbacks or explicitly return an unsupported error.
pub trait RuntimeGameplayHooks {
    fn level_player_tick(&mut self, player: EntityHandle) -> Result<()>;
    fn profile(&mut self, event: ProfileEvent);
    /// The registry does not encode dimension identity. Supply the current
    /// authoritative fact instead of inferring it from loaded chunks.
    fn player_in_overworld(&self, player: &PlayerState) -> Result<bool>;
    fn hit_ground(&mut self, state: &EntityPhysicalState, affected_block: u32) -> Result<()>;
    fn splash(&mut self, state: &EntityPhysicalState) -> Result<()>;
    fn entity_damage(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
        causing_entity: Option<i32>,
    ) -> Result<()>;
    /// Spatial/tick/tracking removal has already run in the coordinator.
    fn discarded(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
    ) -> Result<()>;
    fn award_pickup_stat(
        &mut self,
        player: PickupPlayer,
        original_item_id: i32,
        original_count: i32,
    ) -> Result<()>;
    /// The InventoryMenu's local listener, independently of the predicted
    /// remote baseline. Called after Collect and before its private packets.
    fn inventory_slot_changed(
        &mut self,
        player: PickupPlayer,
        menu_slot: usize,
        inventory: &PlayerInventory,
        changed_stack: &ItemStack,
    ) -> Result<()>;
    fn on_item_pickup(
        &mut self,
        player: PickupPlayer,
        state: &ItemState,
        pickup: Pickup,
    ) -> Result<()>;
}

/// A host has one actual external-menu synchronization owner. Installing either
/// variant replaces the previous one, preventing duplicate publication.
enum ExternalMenuSync<'a, H: RuntimeGameplayHooks + ?Sized> {
    Packets {
        player: i32,
        sync: &'a mut dyn FnMut(&PlayerInventory) -> Result<Vec<Packet>>,
    },
    WithHooks {
        player: PickupPlayer,
        sync: &'a mut dyn FnMut(&mut H, &PlayerInventory) -> Result<Vec<Packet>>,
    },
    WithPickupPreflight {
        player: PickupPlayer,
        callback: &'a mut dyn FnMut(&mut H, &PlayerInventory, Option<&ItemStack>) -> Result<ExternalMenuPickupResult>,
    },
}

/// Candidate admission is distinct from the later actual take publication.
/// The embedding owner may Refuse only a known native profile boundary before
/// Inventory.add. Unexpected failures remain errors; refusal is never a late
/// replacement for an already-mutated take/synchronization callback.
pub enum ExternalMenuPickupResult {
    Admit,
    Refuse,
    Packets(Vec<Packet>),
}

/// Explicit concrete world methods borrowing the SAME gameplay owner as the
/// ordinary hooks. There are no successful defaults; a host without this
/// attachment returns unsupported at the actual source callback.
pub struct RuntimeInventoryWorldCallbacks<H: RuntimeGameplayHooks + ?Sized> {
    pub pressure_plate_inside: fn(&mut H, &mut RuntimeWorld, &mut EntityPhysicalState,
        EntityHandle, crate::placement::BlockPosition, u32,
        &mut dyn RuntimeEntityQuerySound) -> Result<()>,
    pub fluid_shape: fn(&mut H, &mut RuntimeWorld, crate::placement::BlockPosition,
        FluidState) -> Result<VoxelShape>,
    pub rain_exposure: fn(&mut H, &mut RuntimeWorld, &EntityPhysicalState) -> Result<bool>,
}

/// Internal transaction abort minted only for the explicit pre-add Refuse
/// result. The shared-container rollback also preserves its publication
/// revision: refusal is not a successful empty container transaction.
#[derive(Debug)]
struct RefusedExternalMenuPickup;
impl std::fmt::Display for RefusedExternalMenuPickup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("external menu candidate was refused before inventory add")
    }
}
impl std::error::Error for RefusedExternalMenuPickup {}

pub struct RuntimeInventoryHost<'a, H: RuntimeGameplayHooks + ?Sized> {
    players: &'a EntityRegistry,
    containers: &'a BTreeMap<i32, SharedContainer>,
    hooks: &'a mut H,
    external_menu_player: Option<i32>,
    external_menu_sync: Option<ExternalMenuSync<'a, H>>,
    world_callbacks: Option<RuntimeInventoryWorldCallbacks<H>>,
}

impl<'a, H: RuntimeGameplayHooks + ?Sized> RuntimeInventoryHost<'a, H> {
    pub fn new(
        players: &'a EntityRegistry,
        containers: &'a BTreeMap<i32, SharedContainer>,
        hooks: &'a mut H,
    ) -> Self {
        Self {
            players,
            containers,
            hooks,
            external_menu_player: None,
            external_menu_sync: None,
            world_callbacks: None,
        }
    }
    pub fn with_world_callbacks(mut self, callbacks: RuntimeInventoryWorldCallbacks<H>) -> Self {
        self.world_callbacks = Some(callbacks);
        self
    }
    /// The actual active block-menu owner already performed this player's
    /// ServerPlayer.tick menu broadcast/validity phase. Do not also broadcast
    /// the inactive inventory menu through the ordinary inventory-only host.
    pub fn with_external_player_menu(mut self, player: i32) -> Self {
        self.external_menu_player = Some(player);
        self
    }
    /// ServerPlayer.take broadcasts the actual current menu synchronously.
    /// The callback owns that menu's captured container and baselines; the
    /// ordinary inactive InventoryMenu must not be synchronized in its place.
    pub fn with_external_menu_sync(
        mut self,
        player: i32,
        sync: &'a mut dyn FnMut(&PlayerInventory) -> Result<Vec<Packet>>,
    ) -> Self {
        self.external_menu_sync = Some(ExternalMenuSync::Packets { player, sync });
        self
    }

    /// Synchronize the actual active menu with the same gameplay hook owner
    /// already borrowed by this host. The callback runs at the existing
    /// ServerPlayer.take/menu-sync phase inside the canonical shared-container
    /// transaction. It can execute that menu's local inventory listeners before
    /// returning its ordered packets, without cloning hooks or inventing a
    /// second inventory/cursor owner. The caller retains the actual player fact
    /// in its callback; the bound identity is checked before dispatch.
    pub fn with_external_menu_sync_and_hooks(
        mut self,
        player: PickupPlayer,
        sync: &'a mut dyn FnMut(&mut H, &PlayerInventory) -> Result<Vec<Packet>>,
    ) -> Self {
        self.external_menu_sync = Some(ExternalMenuSync::WithHooks { player, sync });
        self
    }

    /// One actual current-menu owner handles both pure candidate admission and
    /// source-position take synchronization. Some(stack) is called before the
    /// canonical Inventory.add mutation and returns Admit/Refuse, no packets;
    /// None is the existing take/menu-sync phase. Both borrow the same hooks
    /// and actual inventory, with no second menu, cursor or callback owner.
    pub fn with_external_menu_sync_and_pickup_preflight(
        mut self,
        player: PickupPlayer,
        callback: &'a mut dyn FnMut(&mut H, &PlayerInventory, Option<&ItemStack>) -> Result<ExternalMenuPickupResult>,
    ) -> Self {
        self.external_menu_sync = Some(ExternalMenuSync::WithPickupPreflight { player, callback });
        self
    }

    fn player(&self, player: PickupPlayer) -> Result<&PlayerState> {
        let current = self
            .players
            .get(player.entity_id)
            .context("pickup player is not registered")?;
        ensure!(
            current.profile.uuid == player.uuid,
            "pickup player UUID changed"
        );
        Ok(current)
    }

    fn container(&self, player: PickupPlayer) -> Result<&SharedContainer> {
        self.player(player)?;
        self.containers
            .get(&player.entity_id)
            .context("pickup player has no authoritative container")
    }

    fn sync_menu(&mut self, player: PickupPlayer) -> Result<()> {
        let container = self.container(player)?.clone();
        if let Some(external) = &mut self.external_menu_sync {
            match external {
                ExternalMenuSync::Packets { player: id, sync } if *id == player.entity_id => {
                    return container.transact(|menu| Ok(((), sync(&menu.inventory)?)));
                }
                ExternalMenuSync::WithHooks { player: bound, sync }
                    if bound.entity_id == player.entity_id =>
                {
                    ensure!(bound.uuid == player.uuid, "external menu player UUID changed");
                    let hooks = &mut *self.hooks;
                    return container.transact(|menu| Ok(((), sync(hooks, &menu.inventory)?)));
                }
                ExternalMenuSync::WithPickupPreflight { player: bound, callback }
                    if bound.entity_id == player.entity_id =>
                {
                    ensure!(bound.uuid == player.uuid, "external menu player UUID changed");
                    let hooks = &mut *self.hooks;
                    return container.transact(|menu| {
                        let packets = match callback(hooks, &menu.inventory, None)? {
                            ExternalMenuPickupResult::Packets(packets) => packets,
                            ExternalMenuPickupResult::Admit | ExternalMenuPickupResult::Refuse =>
                                anyhow::bail!("external menu cannot admit/refuse after canonical pickup mutation"),
                        };
                        Ok(((), packets))
                    });
                }
                _ => {}
            }
        }
        container.transact(|menu| {
            let packets = menu.broadcast_changes()?;
            for change in menu.take_local_slot_changes() {
                if change.inventory_slot.is_some() {
                    self.hooks.inventory_slot_changed(
                        player,
                        change.menu_slot,
                        &menu.inventory,
                        &change.stack,
                    )?;
                }
            }
            Ok(((), packets))
        })
    }
}

impl<H: RuntimeGameplayHooks + ?Sized> RuntimeEntityHost for RuntimeInventoryHost<'_, H> {
    fn pressure_plate_inside(&mut self, world: &mut RuntimeWorld, state: &mut EntityPhysicalState,
        handle: EntityHandle, position: crate::placement::BlockPosition, block_state: u32,
        access: &mut dyn RuntimeEntityQuerySound) -> Result<()> {
        let callback = self.world_callbacks.as_ref()
            .context("actual inventory pressure world owner is not attached")?.pressure_plate_inside;
        callback(&mut *self.hooks, world, state, handle, position, block_state, access)
    }
    fn fluid_shape(&mut self, world: &mut RuntimeWorld, position: crate::placement::BlockPosition,
        fluid: FluidState) -> Result<VoxelShape> {
        let callback = self.world_callbacks.as_ref()
            .context("actual inventory fluid shape owner is not attached")?.fluid_shape;
        callback(&mut *self.hooks, world, position, fluid)
    }
    fn rain_exposure(&mut self, world: &mut RuntimeWorld, state: &EntityPhysicalState) -> Result<bool> {
        let callback = self.world_callbacks.as_ref()
            .context("actual inventory rain owner is not attached")?.rain_exposure;
        callback(&mut *self.hooks, world, state)
    }
    fn level_player_tick(&mut self, player: EntityHandle) -> Result<()> {
        self.hooks.level_player_tick(player)?;
        if self.external_menu_player == Some(player.id) {
            return Ok(());
        }
        let current = self
            .players
            .get(player.id)
            .context("level player missing")?;
        let dimensions = crate::player_physics::player_dimensions(current.metadata.pose, 1.)?;
        let alive = match current.entity_data.get(9) {
            Some(MetadataValue::Float(health)) => *health > 0.,
            _ => anyhow::bail!("level player health data unavailable"),
        };
        // ServerPlayer.tick broadcasts the menu in the level phase. A partial
        // false Inventory.add has no take callback and is flushed here next tick.
        // This remains a bounded phase: the hook must not claim full Player.tick.
        self.sync_menu(PickupPlayer {
            entity_id: player.id,
            uuid: current.profile.uuid,
            bounds: Aabb::player(
                Vec3::new(current.position.x, current.position.y, current.position.z),
                dimensions.width,
                dimensions.height,
            ),
            alive,
            spectator: current.gamemode == 3,
            vehicle_bounds: None,
        })
    }

    fn profile(&mut self, event: ProfileEvent) {
        self.hooks.profile(event);
    }

    fn effect(
        &mut self,
        state: &EntityPhysicalState,
        effect: RuntimeItemEffect,
        sound_seed: Option<i64>,
    ) -> Result<Vec<EffectDelivery>> {
        // Validate/encode the captured state now, before it can move again.
        // Unsupported lifecycle effects remain encoder errors.
        let broadcast = item_effects::encode_effect(state, effect, sound_seed)?;
        match effect {
            RuntimeItemEffect::HitGround { affected_block } => {
                self.hooks.hit_ground(state, affected_block)?
            }
            RuntimeItemEffect::Splash => self.hooks.splash(state)?,
            // DiscardBelowWorld's removal callback already ran through
            // discarded; it has neither another game event nor a packet.
            _ => {}
        }
        let Some(broadcast) = broadcast else {
            return Ok(Vec::new());
        };
        let mut recipients = Vec::new();
        for player in self.players.snapshot() {
            let same_dimension = self.hooks.player_in_overworld(&player)?;
            let position = Vec3::new(player.position.x, player.position.y, player.position.z);
            if broadcast.in_range(position, same_dimension) {
                recipients.push(player.entity_id);
            }
        }
        Ok(vec![EffectDelivery {
            recipients,
            packet: broadcast.packet,
        }])
    }

    fn entity_damage(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
        causing_entity: Option<i32>,
    ) -> Result<()> {
        self.hooks
            .entity_damage(state, stack, health, causing_entity)
    }

    fn discarded(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
    ) -> Result<()> {
        self.hooks.discarded(state, stack, health)
    }

    fn add_to_inventory(
        &mut self,
        player: PickupPlayer,
        _item_entity_id: i32,
        _original_count: i32,
        stack: &mut ItemStack,
    ) -> Result<bool> {
        let creative = self.player(player)?.gamemode == 1;
        let container = self.container(player)?.clone();
        let result = container.transact(|menu| {
            if let Some(ExternalMenuSync::WithPickupPreflight { player: bound, callback }) = &mut self.external_menu_sync {
                if bound.entity_id == player.entity_id {
                    ensure!(bound.uuid == player.uuid, "external menu pickup player UUID changed");
                    match callback(&mut *self.hooks, &menu.inventory, Some(&*stack))? {
                        ExternalMenuPickupResult::Admit => {},
                        ExternalMenuPickupResult::Refuse => return Err(RefusedExternalMenuPickup.into()),
                        ExternalMenuPickupResult::Packets(_) =>
                            anyhow::bail!("external menu pickup preflight cannot publish packets"),
                    }
                }
            }
            let accepted = menu.add_pickup_to_inventory(stack, creative)?;
            // Inventory.add precedes Player.take. Keep the remote baseline
            // untouched and emit no private inventory packet at this stage.
            Ok((accepted, Vec::new()))
        });
        match result {
            Err(error) if error.is::<RefusedExternalMenuPickup>() => Ok(false),
            result => result,
        }
    }

    fn collect_item(
        &mut self,
        player: PickupPlayer,
        _state: &ItemState,
        _original_count: i32,
        _accepted_count: i32,
    ) -> Result<()> {
        // The coordinator has already published its one Collect event.
        self.sync_menu(player)
    }

    fn collected(
        &mut self,
        player: PickupPlayer,
        state: &ItemState,
        pickup: Pickup,
        original_item_id: i32,
    ) -> Result<()> {
        self.player(player)?;
        self.hooks
            .award_pickup_stat(player, original_item_id, pickup.packet_count)?;
        self.hooks.on_item_pickup(player, state, pickup)
    }
}

#[cfg(test)]
#[path = "../tools/item_host_tests.rs"]
mod tests;
