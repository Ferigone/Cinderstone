//! Immediate ServerPlayer.take synchronization of the actual smithing menu.
//!
//! RuntimeInventoryHost invokes this within its canonical inventory transaction,
//! after Collect and before its private packet publication. The same actual
//! MainGameplayFrame executes source-ordered inventory criterion callbacks.
use super::*;
use main_item_gameplay::MainGameplayFrame;
use main_smithing::SmithingRuntime;
use main_smithing_recipes::{smithing_local_changes, smithing_sync_packets};
use mc121_rust::{
    container_workstation_smithing::SmithingMenuEvent,
    inventory::PlayerInventory,
    item_host::RuntimeGameplayHooks,
    runtime_entities::PickupPlayer,
    workstations::menu::SyncEvent,
};

impl SmithingRuntime {
    /// Synchronize this player's actual current menu at ServerPlayer.take.
    /// The later player tick owns block validity and closure; this callback
    /// neither replaces that phase nor broadcasts the inactive InventoryMenu.
    pub(super) fn broadcast_for_inventory(
        &mut self,
        id: i32,
        player: PickupPlayer,
        inventory: &PlayerInventory,
        hooks: &mut MainGameplayFrame<'_>,
    ) -> Result<Vec<Packet>> {
        ensure!(id == player.entity_id, "smithing pickup actor changed");
        let mut next = self.menus.get(&id).cloned()
            .context("current smithing menu absent during pickup")?;
        // Stage only the existing menu's comparison/encoding baselines. Inputs,
        // result and its separate cursor retain their actual source identity;
        // no player inventory or gameplay hook owner is replaced or cloned.
        let events = next.menu.broadcast_changes(inventory)?;
        ensure!(events.iter().all(|event| matches!(event, SmithingMenuEvent::Sync { .. })),
            "smithing pickup broadcast requires its actual synchronization-only source phase");
        let _ = smithing_local_changes(&events)?;
        let packets = smithing_sync_packets(&events)?;

        // MenuSync maps smithing menu slots 4..39 to the canonical main/hotbar
        // player slots. Three inputs and result slot3 have no player backing.
        // Every actual listener sees its own source phase, not a later shared
        // snapshot. The concrete frame validates the live UUID/source owner.
        for event in &events {
            if let SmithingMenuEvent::Sync {
                event: SyncEvent::SlotListener { player_slot: Some(slot), stack, .. }, phase,
            } = event {
                hooks.inventory_slot_changed(player, *slot, &phase.player_inventory, stack)?;
            }
        }

        // A pure broadcast does not call Inventory.setChanged. Non-sync events
        // were rejected above rather than discarding a required counter/effect.
        *self.menus.get_mut(&id)
            .expect("exclusive current smithing menu retained through pickup") = next;
        Ok(packets)
    }
}
