//! Immediate ServerPlayer.take synchronization of the actual stonecutter menu.
//!
//! The existing RuntimeInventoryHost calls this inside its canonical shared
//! inventory transaction, after Collect and before its private packet batch.
//! The actual MainGameplayFrame owns the inventory-criterion callbacks.
use super::*;
use main_item_gameplay::MainGameplayFrame;
use main_stonecutter::StonecutterRuntime;
use main_stonecutter_recipes::{stonecutter_local_changes, stonecutter_sync_packets};
use mc121_rust::{
    container_workstation_stonecutter::StonecutterMenuEvent,
    inventory::PlayerInventory,
    item_host::RuntimeGameplayHooks,
    runtime_entities::PickupPlayer,
    workstations::menu::SyncEvent,
};

impl StonecutterRuntime {
    /// Broadcast the actual current menu immediately after an item is taken.
    /// This is not the later ServerPlayer.tick validity/close phase: even a
    /// removed block does not replace this callback with an InventoryMenu sync.
    /// The menu's own cursor remains its synchronization owner throughout.
    pub(super) fn broadcast_for_inventory(
        &mut self,
        id: i32,
        player: PickupPlayer,
        inventory: &PlayerInventory,
        hooks: &mut MainGameplayFrame<'_>,
    ) -> Result<Vec<Packet>> {
        ensure!(id == player.entity_id, "stonecutter pickup actor changed");
        let mut next = self.menus.get(&id).cloned()
            .context("current stonecutter menu absent during pickup")?;
        // Prepare all menu comparisons/encodings before changing its actual
        // local or remote baselines. The shared inventory is borrowed, never
        // replaced with an additional menu-local player inventory.
        let events = next.menu.broadcast_changes(inventory)?;
        ensure!(events.iter().all(|event| matches!(event, StonecutterMenuEvent::Sync { .. })),
            "stonecutter pickup broadcast requires its actual synchronization-only source phase");
        let _ = stonecutter_local_changes(&events)?;
        let packets = stonecutter_sync_packets(&events)?;

        // MenuSync supplies the source slot-listener encounter order and the
        // inventory index mapping. The concrete actual frame rejects unknown
        // player/source/criterion contexts; foreign or reentrant hook owners
        // cannot be installed through this adapter's signature.
        for event in &events {
            if let StonecutterMenuEvent::Sync {
                event: SyncEvent::SlotListener { player_slot: Some(slot), stack, .. }, phase,
            } = event {
                hooks.inventory_slot_changed(player, *slot, &phase.inventory, stack)?;
            }
        }

        // All fallible work is finished. No callback above can replace this
        // menu under the distinct exclusive world/player/menu owner borrows.
        *self.menus.get_mut(&id)
            .expect("exclusive current stonecutter menu retained through pickup") = next;
        Ok(packets)
    }
}
