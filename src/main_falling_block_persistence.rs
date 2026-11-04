//! Bootstrap of the one checksummed native Item/XP/Falling cohort.
use super::*;
use mc121_rust::{
    entity_persistence::NativeEntityCohort,
    falling_block::{block::{self, NativeFallingClass}, persistence::FallingBlockSnapshot, FallingBlockState},
    placement,
    runtime_entity_index::EntityKind,
};

/// Class metadata alone is not native powder capability. Cold restore runs
/// AFTER the final BLOCK owner has been restored and the actual configured
/// damage/powder owners have been installed. No readiness or shape query is
/// performed here; those belong to the real first activated entity tick.
fn require_powder_owner(players: &LivePlayers, world: &WorldBackend, state: u32) -> Result<bool> {
    if block::source_class(state)? != Some(NativeFallingClass::ConcretePowder) {
        return Ok(false);
    }
    players.entity_source.as_ref().context("native powder resume has no actual source owner")?
        .require_current_world(world)?;
    players.block_runtime.falling_damage_types()?;
    let owner = players.block_runtime.concrete_powder_owner()?;
    let binding = owner.binding_for_state(state)?
        .context("native powder resume lacks its installed actual Block association")?;
    let hardened = block::concrete_powder_hardened_state(state)?
        .context("native powder resume lacks its captured source concrete default")?;
    ensure!(*binding.powder() == placement::placement_block_min_state(state)?
        && *binding.concrete() == placement::placement_block_min_state(hardened)?,
        "native powder resume owner differs from its actual source association");
    Ok(true)
}

fn require_powder_payload(state: &FallingBlockState) -> Result<()> {
    ensure!(state.physical.vehicle.is_none(), "native powder passenger/vehicle resume is unsupported");
    ensure!(state.physical.portal_process.is_none(), "native powder resume lacks its actual portal processor");
    ensure!(state.block_data.is_none(), "native powder resume lacks its actual retained block-data map owner");
    // Native continuation retains the actual physical fields, signed time,
    // StartPos and RNG. Do not reset those fields to positioned-ctor defaults
    // or guess first-tick readiness from a saved position/time.
    state.validate()
}

/// Capture only actual registered bodies under Main's existing SAME world /
/// player / clock interval. The existing atomic entity cohort remains the sole
/// persisted owner; no extra powder array, RNG or source summary is created.
pub(super) fn snapshot_native_falling_blocks(
    players: &LivePlayers, world: &WorldBackend,
) -> Result<Vec<FallingBlockSnapshot>> {
    let mut seen_uuids = BTreeSet::new();
    let mut duplicate_uuids = BTreeSet::new();
    let players_now = players.registry.snapshot();
    for uuid in players.item_runtime.item_states().filter(|state| !state.removed).map(|state| state.uuid)
        .chain(players.experience_runtime.physical_states().map(|state| state.uuid))
        .chain(players.falling_blocks.registered().map(|(_, state)| state.uuid))
        .chain(players_now.iter().map(|player| player.profile.uuid))
    {
        if !seen_uuids.insert(uuid) { duplicate_uuids.insert(uuid); }
    }
    for (handle, state) in players.falling_blocks.registered() {
        if require_powder_owner(players, world, state.block_state)? {
            ensure!(handle.id == state.entity_id
                && players.item_runtime.entity_handle(state.entity_id) == Some(handle)
                && players.item_runtime.entity_kind(handle) == Some(EntityKind::FallingBlock)
                && players.falling_blocks.get(handle).is_some_and(|actual| actual.state.uuid == state.uuid),
                "native powder snapshot is not its actual indexed Falling identity/generation");
            ensure!(!duplicate_uuids.contains(&state.uuid),
                "native powder snapshot UUID collides with an actual live entity owner");
            require_powder_payload(state)?;
        }
    }
    players.falling_blocks.snapshot()
}

/// Main calls this before publishing Server or accepting PLAY actors. The
/// checkpoint decoder already validates its full source/world/time and cohort;
/// repeat identities against the actual restored item owner before mutating
/// either external body store. Real registrations then receive a source-order
/// restore over their actual NEW handles, with chunk visibility still hidden.
pub(super) fn restore_native_entity_cohort(
    players: &mut LivePlayers,
    world: &WorldBackend,
    native: Option<NativeEntityCohort>,
    legacy_experience: Vec<mc121_rust::experience::SavedExperienceOrb>,
    next_entity_id: i32,
) -> Result<()> {
    ensure!(players.experience_runtime.count() == 0 && players.falling_blocks.count() == 0,
        "native external entity stores are not empty during bootstrap");
    let Some(native) = native else {
        // This is the pre-existing split-file migration path. Its old XP/item
        // mixed order and cross-file atomicity were never certified. Keep that
        // limit explicit; it is not an original-order reconstruction.
        for orb in &legacy_experience {
            ensure!(orb.state.entity_id > 0 && orb.state.entity_id < next_entity_id,
                "legacy XP orb exceeds the shared entity counter");
        }
        if !legacy_experience.is_empty() {
            eprintln!("{}", json!({"event":"native_entity_cohort_legacy_migration",
                "mixed_source_order":"unverified", "split_file_atomicity":"unverified"}));
        }
        players.experience_runtime = main_experience::MainExperienceRuntime::restore(
            legacy_experience, &mut players.item_runtime,
        )?;
        return Ok(());
    };

    let mut ids = BTreeSet::new();
    let mut uuids = BTreeSet::new();
    let mut entries = Vec::new();
    for item in players.item_runtime.item_states() {
        ensure!(item.physical.vehicle.is_none(), "native entity passenger/vehicle graph restore is unsupported");
        ensure!(ids.insert(item.entity_id) && uuids.insert(item.uuid),
            "duplicate actual restored item identity");
        ensure!(item.entity_id > 0 && item.entity_id < next_entity_id,
            "actual restored item exceeds shared entity counter");
        entries.push((item.entity_id, EntityKind::Item, item.position_vec()));
    }
    for orb in &native.experience_orbs {
        ensure!(orb.state.physical.vehicle.is_none(), "native entity passenger/vehicle graph restore is unsupported");
        orb.state.validate()?;
        orb.restore_random()?;
        ensure!(orb.state.entity_id > 0 && orb.state.entity_id < next_entity_id,
            "native XP orb exceeds the shared entity counter");
        ensure!(ids.insert(orb.state.entity_id) && uuids.insert(orb.state.uuid),
            "duplicate identity across actual native entity owners");
        entries.push((orb.state.entity_id, EntityKind::ExperienceOrb, orb.state.position_vec()));
    }
    let mut falling_objects = Vec::with_capacity(native.falling_blocks.len());
    for saved in native.falling_blocks {
        ensure!(saved.block_data.is_none(), "native falling restore lacks the actual retained block-data map owner");
        let is_powder = require_powder_owner(players, world, saved.block_state)?;
        // Build the actual retained Box ONCE after owner admission, then keep
        // that same object through complete cohort/index/order validation.
        // Snapshot restore inverts the RNG scramble without sampling it; it
        // invokes neither source Entity construction nor UUID generation.
        let object = players.falling_blocks.restore_constructed(saved)?;
        let state = players.falling_blocks.object_state(&object)?;
        if is_powder { require_powder_payload(state)?; }
        ensure!(state.physical.vehicle.is_none(), "native entity passenger/vehicle graph restore is unsupported");
        ensure!(state.entity_id > 0 && state.entity_id < next_entity_id,
            "native falling block exceeds the shared entity counter");
        ensure!(ids.insert(state.entity_id) && uuids.insert(state.uuid),
            "duplicate identity across actual native entity owners");
        entries.push((state.entity_id, EntityKind::FallingBlock, state.position_vec()));
        falling_objects.push(object);
    }
    native.order.validate_entries(&entries)?;
    players.item_runtime.validate_entity_registration_batch(entries.iter()
        .filter(|(_, kind, _)| *kind != EntityKind::Item).map(|(id, _, _)| *id))?;

    // Some is authoritative even when EMPTY. Never append stale legacy XP.
    players.experience_runtime = main_experience::MainExperienceRuntime::restore(
        native.experience_orbs, &mut players.item_runtime,
    )?;
    for mut object in falling_objects {
        players.falling_blocks.register_constructed(&mut players.item_runtime, &mut object)?;
    }
    players.item_runtime.restore_persistent_entity_order(&native.order)?;
    ensure!(players.falling_blocks.max_entity_id().is_none_or(|id| id < next_entity_id)
        && players.experience_runtime.max_entity_id().is_none_or(|id| id < next_entity_id),
        "restored external native entity exceeds shared entity counter");
    Ok(())
}
