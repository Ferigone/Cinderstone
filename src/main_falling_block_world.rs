//! Borrowed production world/LevelTicks capabilities for falling-block calls.
//!
//! This frame is never an alternative Level, RNG, entity store or source
//! certificate. The caller lends the actual owners while retaining its world
//! and live-player locks. Constructing it does not grant a falling-entity owner.

use super::*;
use mc121_rust::{
    block_ticks::{self, BlockTickScheduler},
    placement::BlockPosition,
    tick::TickPriority,
    world_context::DimensionFacts,
};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct OriginalFallingStatePredicates {
    schema_version: u32,
    minecraft: String,
    inner_jar_sha256: String,
    mappings_sha1: String,
    placement_catalog_sha256: String,
    state_count: u32,
    liquid_true_ranges: Vec<[u32; 2]>,
    can_be_replaced_true_ranges: Vec<[u32; 2]>,
}

impl OriginalFallingStatePredicates {
    fn load() -> Result<Self> {
        let facts: Self = serde_json::from_str(include_str!(
            "../fixtures/falling-block-state-predicates.json"))?;
        ensure!(facts.schema_version == 1 && facts.minecraft == "1.21",
            "foreign falling-state fact schema/version");
        ensure!(facts.inner_jar_sha256 ==
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
            && facts.mappings_sha1 == "31c77994d96f05ba25a870ada70f47f315330437",
            "foreign falling-state source identity");
        let catalogue_digest = openssl::sha::sha256(include_bytes!(
            "../fixtures/placement-1.21.json"));
        let catalogue_hex = catalogue_digest.iter()
            .map(|value| format!("{value:02x}")).collect::<String>();
        ensure!(facts.placement_catalog_sha256 == catalogue_hex
            && facts.state_count == 26_684,
            "falling-state facts are not bound to the actual pinned state catalogue");
        for ranges in [&facts.liquid_true_ranges, &facts.can_be_replaced_true_ranges] {
            let mut previous_end = None;
            for &[first, last] in ranges {
                ensure!(first <= last && last < facts.state_count
                    && previous_end.is_none_or(|old| old < first),
                    "invalid falling-state predicate ranges");
                previous_end = Some(last);
            }
        }
        Ok(facts)
    }

    fn require_known(&self, state: u32) -> Result<()> {
        ensure!(state < self.state_count, "unknown falling-block state ID {state}");
        mc121_rust::placement::placement_block_name(state)?;
        Ok(())
    }

    fn contains(&self, ranges: &[[u32; 2]], state: u32) -> Result<bool> {
        self.require_known(state)?;
        let end = ranges.partition_point(|range| range[0] <= state);
        // False is an observed value inside the exhaustively queried original
        // registry; foreign/unknown states have already returned an error.
        Ok(end.checked_sub(1).is_some_and(|i| state <= ranges[i][1]))
    }
}

fn state_predicates() -> Result<&'static OriginalFallingStatePredicates> {
    static FACTS: OnceLock<std::result::Result<OriginalFallingStatePredicates, String>>
        = OnceLock::new();
    FACTS.get_or_init(|| OriginalFallingStatePredicates::load()
        .map_err(|error| format!("{error:#}")))
        .as_ref().map_err(|error| anyhow!("{error}"))
}

pub(super) fn state_is_liquid(state: u32) -> Result<bool> {
    let facts = state_predicates()?;
    facts.contains(&facts.liquid_true_ranges, state)
}

pub(super) fn state_can_be_replaced(state: u32) -> Result<bool> {
    let facts = state_predicates()?;
    facts.contains(&facts.can_be_replaced_true_ranges, state)
}

/// Preserve the original short-circuit predicate, including bubble columns'
/// liquid property and the distinction between liquid and waterlogging.
pub(super) fn falling_is_free(state: u32) -> Result<bool> {
    state_predicates()?.require_known(state)?;
    if mc121_rust::lighting::state_properties(state)?.is_air() {
        return Ok(true);
    }
    if mc121_rust::placement::in_block_tag(state, "minecraft:fire") {
        return Ok(true);
    }
    if state_is_liquid(state)? {
        return Ok(true);
    }
    state_can_be_replaced(state)
}

/// Pure carried-state operation from FallingBlockEntity.fall, before its
/// constructor. The origin replacement must still use the ORIGINAL state.
pub(super) fn carried_state_for_fall(original_state: u32) -> Result<u32> {
    mc121_rust::placement::placement_block_name(original_state)?;
    if mc121_rust::inventory::block_state_property(original_state, "waterlogged").is_some() {
        mc121_rust::inventory::block_state_with_properties(original_state,
            &BTreeMap::from([("waterlogged".into(), "false".into())]))
    } else {
        Ok(original_state)
    }
}

/// Call at the source phase AFTER constructing the actual falling entity and
/// BEFORE actual Level.setBlock(flags3). This computes data only, not a commit.
pub(super) fn original_fluid_replacement(original_state: u32) -> Result<u32> {
    mc121_rust::world_interactions::removed_replacement(original_state)
}

pub(super) fn fall_spawn_position(position: BlockPosition) -> Vec3 {
    Vec3::new(f64::from(position.0) + 0.5, f64::from(position.1),
        f64::from(position.2) + 0.5)
}

pub(super) struct MainFallingBlockWorld<'a> {
    pub(super) world: &'a mut WorldBackend,
    pub(super) source: &'a mut NativeEntitySource,
    pub(super) queue: &'a mut BlockTickScheduler,
    pub(super) sub_tick_order: &'a mut i64,
    pub(super) dimension: &'a DimensionFacts,
    pub(super) now: i64,
}

impl<'a> MainFallingBlockWorld<'a> {
    /// The real BLOCK halo must already be prepared. No caller-created queue,
    /// edited save, copied source history or guessed dimension is accepted.
    pub(super) fn new(
        world: &'a mut WorldBackend,
        source: &'a mut NativeEntitySource,
        queue: &'a mut BlockTickScheduler,
        sub_tick_order: &'a mut i64,
        dimension: &'a DimensionFacts,
        now: i64,
    ) -> Result<Self> {
        ensure!(matches!(world, WorldBackend::Fixture(_)),
            "falling-block imported-world ownership is unavailable");
        dimension.require_runtime_layout()?;
        state_predicates()?;
        source.require_owned_callback_world(world)?;
        Ok(Self { world, source, queue, sub_tick_order, dimension, now })
    }

    /// Native capability validation only. It does not sample below/minimum
    /// height, schedule a tick, allocate an entity or consume any RNG stream.
    pub(super) fn validate_callback(
        &mut self,
        position: BlockPosition,
        block: u32,
        state: u32,
    ) -> Result<()> {
        self.source.require_owned_callback_world(self.world)?;
        ensure!(mc121_rust::placement::placement_block_min_state(state)? == block,
            "falling callback state has a foreign Block singleton");
        ensure!(self.queue.chunk(block_ticks::pos(position).chunk()).is_some(),
            "falling callback has no actual loaded BLOCK queue");
        Ok(())
    }

    /// Java scheduleTick consumes the shared sub-tick count even when its
    /// Block/position key is already queued. The common scheduler owns dedup.
    pub(super) fn schedule(
        &mut self,
        position: BlockPosition,
        block: u32,
        delay: i32,
    ) -> Result<()> {
        self.source.observe_block_scheduler_use();
        block_ticks::schedule(self.queue, position, block, self.now, delay,
            TickPriority::Normal, self.sub_tick_order)?;
        Ok(())
    }

    pub(super) fn block(&mut self, position: BlockPosition) -> Result<u32> {
        self.world.block(position.0, position.1, position.2)
    }
}
