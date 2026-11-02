//! Immediate real LoomMenu synchronization at ServerPlayer.take.
//! The existing canonical inventory transaction supplies the actual pickup
//! identity and the same borrowed MainGameplayFrame used for the item pickup.
use super::*;
use main_item_gameplay::MainGameplayFrame;
use main_loom::LoomRuntime;
use main_loom_callbacks::loom_preflight_phase;
use mc121_rust::{
    container_workstation_loom::LoomMenuEvent,
    inventory::PlayerInventory,
    item_host::RuntimeGameplayHooks,
    runtime_entities::PickupPlayer,
    workstations::menu::SyncEvent,
};

impl LoomRuntime {
    /// Only pure broadcast is executed here. Source block validity belongs to
    /// the later player menu tick, even when this pickup follows block removal.
    /// Retained context is used for its actual owner/registry/callback binding;
    /// broadcast never queries its captured access or world-game-time sample.
    pub(super) fn broadcast_for_inventory(
        &mut self,
        id: i32,
        player: PickupPlayer,
        inventory: &PlayerInventory,
        hooks: &mut MainGameplayFrame<'_>,
    ) -> Result<Vec<Packet>> {
        ensure!(id == player.entity_id, "loom pickup actor changed");
        let open = self.menus.get(&id).context("current loom menu absent during pickup")?;
        open.context.verify_pickup_actor(player)?;
        open.context.require_default_callbacks()?;
        let mut next_menu = open.menu.clone();
        let events = next_menu.broadcast_changes(inventory, &open.context)?;
        let mut packets = Vec::new();
        // Complete every fallible stack/listener observation before changing
        // the actual hooks or retained menu comparison/encoding baselines.
        for event in &events {
            let LoomMenuEvent::Sync { event, phase } = event else {
                bail!("loom pickup broadcast requires its actual synchronization-only source phase");
            };
            loom_preflight_phase(phase, open.context.patterns())?;
            match event {
                SyncEvent::SlotListener { menu_slot, player_slot, stack } => {
                    main_workstation_components::require_supported_stack(stack)?;
                    if let Some(slot) = player_slot {
                        ensure!((4..40).contains(menu_slot) && *slot == *menu_slot + 5,
                            "loom pickup listener lost its canonical player backing");
                    } else {
                        ensure!(*menu_slot < 4, "unknown loom pickup backing container");
                    }
                }
                SyncEvent::DataListener { index, value } => {
                    ensure!(*index == 0 && *value == phase.selected_pattern,
                        "loom pickup data listener lost its actual selected pattern");
                    // Original ServerPlayer$2.dataChanged(cpu,int,int) returns.
                    // The real MenuSync already advanced its local baseline;
                    // no player inventory dirty notification belongs here.
                }
                SyncEvent::Packet(packet) => packets.push(packet.clone()),
            }
        }
        for event in &events {
            if let LoomMenuEvent::Sync {
                event: SyncEvent::SlotListener { player_slot: Some(slot), stack, .. }, phase,
            } = event {
                hooks.inventory_slot_changed(player, *slot, &phase.player_inventory, stack)?;
            }
        }
        // Pure broadcast has no Inventory.setChanged, take sound, recipe,
        // crafted, XP or drop callback. Such events were rejected above.
        self.menus.get_mut(&id)
            .expect("exclusive current loom menu retained through pickup").menu = next_menu;
        Ok(packets)
    }
}
