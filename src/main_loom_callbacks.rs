//! Source-phased Loom callbacks over actual Main gameplay/configuration owners.
//!
//! The kernel has already computed its state changes and synchronization. This
//! bridge preflights that exact observation stream; it creates no statistic,
//! recipe book, XP, inventory, listener registry or sound random owner.
use super::*;
use main_item_gameplay::MainItemGameplay;
use main_loom::NativeLoomContext;
use main_loom_patterns::{NativeLoomPatterns, LOOM_TAKE_SOUND_ID};
use mc121_rust::{
    container::LocalSlotChange,
    container_workstation_loom::{LoomMenuContext, LoomMenuEvent, LoomPhase},
    workstations::{loom::LoomSourceEvent, menu::SyncEvent},
};

pub(super) struct PreparedLoomCallbacks {
    source: BTreeMap<usize, LoomSourceEvent>,
    inventory_changes: BTreeSet<usize>,
    sounds: BTreeSet<usize>,
}

impl PreparedLoomCallbacks {
    pub(super) fn prepare(
        gameplay: &MainItemGameplay,
        patterns: &NativeLoomPatterns,
        events: &[LoomMenuEvent],
        context: &NativeLoomContext,
    ) -> Result<Self> {
        gameplay.require_supported_floor()?;
        // The real Main context has a private factory and retains the genuine
        // constructor-default/native nonreentrant listener profile. An arbitrary
        // listener boolean or another configured registry is not a capability.
        context.require_default_callbacks()?;
        ensure!(context.patterns().binding() == patterns.binding(),
            "loom callback pattern/configuration owner changed");
        let mut prepared = Self {
            source: BTreeMap::new(), inventory_changes: BTreeSet::new(), sounds: BTreeSet::new(),
        };
        for (index, event) in events.iter().enumerate() {
            loom_preflight_phase(loom_event_phase(event), patterns)?;
            match event {
                LoomMenuEvent::Source { event, .. } => {
                    match event {
                        LoomSourceEvent::InputContainerChanged
                        | LoomSourceEvent::OutputContainerChanged
                        | LoomSourceEvent::UpdateListener
                        | LoomSourceEvent::BroadcastChanges => {}
                    }
                    prepared.source.insert(index, *event);
                }
                LoomMenuEvent::PlayerInventoryChanged { .. } => {
                    prepared.inventory_changes.insert(index);
                }
                LoomMenuEvent::TakeSound { game_time, .. } => {
                    ensure!(context.world_game_time()? == *game_time,
                        "loom take sound lost its captured actual level time");
                    prepared.sounds.insert(index);
                }
                LoomMenuEvent::Sync { .. } => {
                    let _ = loom_local_changes(std::slice::from_ref(event))?;
                }
                LoomMenuEvent::Drop { .. } => {
                    // The actual indexed Drop owner validates/stages its owned
                    // stack and RNG. A phase observation grants no extra item.
                }
            }
        }
        Ok(prepared)
    }

    /// Visit only the source boundary admitted by the actual sealed context.
    /// Input/output superclass listener sets are genuinely the supported native
    /// constructor profile. UpdateListener is original cra.r(), a sole return.
    /// The input recomputation and BroadcastChanges have already been emitted
    /// by the kernel into this very stream; invoking them again is incorrect.
    pub(super) fn complete_source_at(&self, index: usize) -> Result<()> {
        match self.source.get(&index) {
            Some(LoomSourceEvent::InputContainerChanged)
            | Some(LoomSourceEvent::OutputContainerChanged)
            | Some(LoomSourceEvent::UpdateListener)
            | Some(LoomSourceEvent::BroadcastChanges) => Ok(()),
            None => bail!("unprepared loom source callback"),
        }
    }
    pub(super) fn is_player_inventory_change_at(&self, index: usize) -> bool {
        self.inventory_changes.contains(&index)
    }
    pub(super) fn is_sound_at(&self, index: usize) -> bool { self.sounds.contains(&index) }
}

pub(super) fn loom_event_phase(event: &LoomMenuEvent) -> &LoomPhase {
    match event {
        LoomMenuEvent::Source { phase, .. }
        | LoomMenuEvent::PlayerInventoryChanged { phase }
        | LoomMenuEvent::Sync { phase, .. }
        | LoomMenuEvent::TakeSound { phase, .. }
        | LoomMenuEvent::Drop { phase, .. } => phase,
    }
}

fn preflight_stack(stack: &ItemStack, patterns: &NativeLoomPatterns) -> Result<()> {
    main_workstation_components::require_supported_stack(stack)?;
    patterns.require_item_enabled(stack)?;
    if !stack.is_empty() { stack.encode_canonical()?; }
    Ok(())
}

/// Admit each actual historical source snapshot before any real callback/drop
/// or publication. Do not recompute the result or demand that an intermediate
/// pattern list already matches inputs whose changed callback is in progress.
pub(super) fn loom_preflight_phase(phase: &LoomPhase, patterns: &NativeLoomPatterns) -> Result<()> {
    for stack in &phase.inputs { preflight_stack(stack, patterns)?; }
    preflight_stack(&phase.result_preview, patterns)?;
    preflight_stack(&phase.carried, patterns)?;
    main_workstation_components::require_supported_inventory(&phase.player_inventory)?;
    for stack in phase.player_inventory.slots() { patterns.require_item_enabled(stack)?; }
    patterns.require_item_enabled(phase.player_inventory.carried())?;
    // Resolve the queries made by the real InventoryChangeTrigger scan before
    // its later infallible source-position replay. This is Inventory order.
    for slot in (36..45).chain(9..36).chain((5..9).rev()).chain(std::iter::once(45)) {
        let stack = phase.player_inventory.slot(slot).context("loom phase inventory slot absent")?;
        if !stack.is_empty() { stack.max_stack_size()?; }
    }
    for pattern in &phase.selectable_patterns {
        ensure!(pattern.registry_id >= 0
            && patterns.registry().get(pattern.registry_id as usize)
                .is_some_and(|entry| entry.registry_id == pattern.registry_id),
            "loom source phase contains a foreign banner-pattern holder");
    }
    Ok(())
}

/// Map only actual player-backed slots to InventoryMenu listener coordinates.
/// Input/result listeners retain their own Source events and never trigger a
/// fabricated player-inventory criterion. This function executes no listener.
pub(super) fn loom_local_changes(events: &[LoomMenuEvent]) -> Result<Vec<LocalSlotChange>> {
    let mut changes = Vec::new();
    let mut packet_offset = 0;
    for event in events {
        match event {
            LoomMenuEvent::Sync { event: SyncEvent::Packet(_), .. } => packet_offset += 1,
            LoomMenuEvent::Sync { event: SyncEvent::DataListener { index, value }, phase } => {
                ensure!(*index == 0 && *value == phase.selected_pattern,
                    "loom data listener lost its actual selected-pattern source phase");
            }
            LoomMenuEvent::Sync { event: SyncEvent::SlotListener {
                menu_slot, player_slot, stack,
            }, .. } => {
                main_workstation_components::require_supported_stack(stack)?;
                if let Some(slot) = player_slot {
                    ensure!((4..40).contains(menu_slot) && (9..45).contains(slot)
                        && *menu_slot + 5 == *slot,
                        "loom listener has a foreign canonical player-inventory backing");
                    changes.push(LocalSlotChange {
                        menu_slot: *slot,
                        inventory_slot: Some(if *slot < 36 { *slot } else { *slot - 36 }),
                        stack: stack.clone(), packet_offset,
                    });
                } else {
                    ensure!(*menu_slot < 4, "unknown loom backing container");
                }
            }
            LoomMenuEvent::Source { .. }
            | LoomMenuEvent::PlayerInventoryChanged { .. }
            | LoomMenuEvent::TakeSound { .. }
            | LoomMenuEvent::Drop { .. } => {
                // These source-position owners are explicitly separate from
                // the pure local-slot observation extraction above.
            }
        }
    }
    Ok(changes)
}

/// Packet extraction only. Normal transactions replay one indexed event at a
/// time, interleaving actual listeners/counters/sound/drops before publication.
/// A pickup's pure broadcast may use the whole synchronization-only stream.
pub(super) fn loom_sync_packets(events: &[LoomMenuEvent]) -> Result<Vec<Packet>> {
    let _ = loom_local_changes(events)?;
    let mut packets = Vec::new();
    for event in events {
        match event {
            LoomMenuEvent::Sync { event: SyncEvent::Packet(packet), .. } => packets.push(packet.clone()),
            LoomMenuEvent::Sync { event: SyncEvent::SlotListener { .. } | SyncEvent::DataListener { .. }, .. }
            | LoomMenuEvent::Source { .. }
            | LoomMenuEvent::PlayerInventoryChanged { .. }
            | LoomMenuEvent::TakeSound { .. }
            | LoomMenuEvent::Drop { .. } => {}
        }
    }
    Ok(packets)
}

/// Actual registered variable-range BLOCKS sound, centered on the captured
/// block. The caller draws this seed from Level.threadSafeRandom exactly at
/// the emitted TakeSound position, including when its actual audience is empty.
pub(super) fn loom_sound_packet(position: (i32, i32, i32), seed: i64) -> Packet {
    // Rust's saturating finite float->int cast matches Java's narrowing cast,
    // including a block coordinate outside the ordinary supported world slice.
    let coordinate = |value: i32| ((f64::from(value) + 0.5) * 8.) as i32;
    let mut fields = Writer::new();
    fields.varint(LOOM_TAKE_SOUND_ID + 1).varint(4)
        .i32(coordinate(position.0)).i32(coordinate(position.1)).i32(coordinate(position.2))
        .f32(1.).f32(1.).i64(seed);
    Packet { id: 0x68, payload: fields.into_inner() }
}
