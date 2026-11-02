//! Immediate actual current GrindstoneMenu synchronization at Player.take.
//! The canonical inventory transaction supplies the real actor and same hooks;
//! the primary module supplies its retained binding/registry-only sync Host.
use super::*;
use main_grindstone::GrindstoneRuntime;
use main_grindstone_recipes::grindstone_preflight_sync;
use main_item_gameplay::MainGameplayFrame;
use mc121_rust::{
    inventory::PlayerInventory,
    item_host::RuntimeGameplayHooks,
    runtime_entities::PickupPlayer,
    workstations::menu::SyncEvent,
};

impl GrindstoneRuntime {
    pub(super) fn broadcast_for_inventory(
        &mut self, id: i32, player: PickupPlayer, inventory: &PlayerInventory,
        hooks: &mut MainGameplayFrame<'_>,
    ) -> Result<Vec<Packet>> {
        ensure!(id == player.entity_id, "grindstone pickup actor changed");
        let open = self.menus.get(&id).context("current grindstone menu absent during pickup")?;
        open.verify_pickup_actor(player)?;
        // This genuine kernel broadcast queries only its retained actual
        // binding/registry and the borrowed inventory. No Level random, valid
        // flag, clock, entity proposal, XP or drop owner is fabricated here.
        let (next_menu, phases) = open.prepare_inventory_broadcast(inventory)?;
        for (events, phase) in &phases { let _ = grindstone_preflight_sync(events, phase)?; }
        let mut packets = Vec::new();
        for (events, phase) in &phases {
            for event in events {
                match event {
                    SyncEvent::SlotListener { player_slot: Some(slot), stack, .. } => {
                        hooks.inventory_slot_changed(player, *slot, &phase.player_inventory, stack)?;
                    }
                    SyncEvent::Packet(packet) => packets.push(packet.clone()),
                    SyncEvent::SlotListener { player_slot: None, .. } => {}
                    // Preflight rejected data slots before any callback.
                    SyncEvent::DataListener { .. } => unreachable!("validated grindstone has no data slots"),
                }
            }
        }
        self.menus.get_mut(&id)
            .expect("exclusive retained grindstone menu through pickup").menu = next_menu;
        // Pure broadcast has no Inventory.setChanged, XP, take-level-event,
        // crafted/recipe or drop action and never broadcasts window0 again.
        Ok(packets)
    }
}
