//! Bootstrap of the one checksummed native Item/XP/Falling cohort.
use super::*;
use mc121_rust::{
    entity_persistence::NativeEntityCohort,
    runtime_entity_index::EntityKind,
};

/// Main calls this before publishing Server or accepting PLAY actors. The
/// checkpoint decoder already validates its full source/world/time and cohort;
/// repeat identities against the actual restored item owner before mutating
/// either external body store. Real registrations then receive a source-order
/// restore over their actual NEW handles, with chunk visibility still hidden.
pub(super) fn restore_native_entity_cohort(
    players: &mut LivePlayers,
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
    for saved in &native.falling_blocks {
        ensure!(saved.block_data.is_none(), "native falling restore lacks the actual retained block-data map owner");
        let state = saved.clone().restore()?;
        ensure!(state.physical.vehicle.is_none(), "native entity passenger/vehicle graph restore is unsupported");
        ensure!(state.entity_id > 0 && state.entity_id < next_entity_id,
            "native falling block exceeds the shared entity counter");
        ensure!(ids.insert(state.entity_id) && uuids.insert(state.uuid),
            "duplicate identity across actual native entity owners");
        entries.push((state.entity_id, EntityKind::FallingBlock, state.position_vec()));
    }
    native.order.validate_entries(&entries)?;

    // Some is authoritative even when EMPTY. Never append stale legacy XP.
    players.experience_runtime = main_experience::MainExperienceRuntime::restore(
        native.experience_orbs, &mut players.item_runtime,
    )?;
    for saved in native.falling_blocks {
        let mut object = players.falling_blocks.restore_constructed(saved)?;
        players.falling_blocks.register_constructed(&mut players.item_runtime, &mut object)?;
    }
    players.item_runtime.restore_persistent_entity_order(&native.order)?;
    ensure!(players.falling_blocks.max_entity_id().is_none_or(|id| id < next_entity_id)
        && players.experience_runtime.max_entity_id().is_none_or(|id| id < next_entity_id),
        "restored external native entity exceeds shared entity counter");
    Ok(())
}
