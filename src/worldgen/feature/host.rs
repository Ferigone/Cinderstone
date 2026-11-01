//! Actual world and bulk-section owners required by the configured ore leaves.
use super::config::{OreConfiguration, TargetBlockState};
use crate::{
    density::DensityPoint, inventory::block_state_name, random::RandomSource,
    worldgen::generator::resolve_block_state,
};
use anyhow::{Context, Result};
use std::sync::OnceLock;

/// The original LevelAccessor/WorldGenLevel and its live chunk owners.
/// Implementations must preserve world/dimension identity, actual accessor
/// bounds, chunk dependency statuses and real state/registry identities.
/// Missing chunks or section owners are errors, never air or empty terrain.
///
/// The caller performs ConfiguredFeature's `ensure_can_write(origin)` before
/// invoking either leaf and before any RNG draw. Neither leaf advances a chunk
/// generation status. Errors may follow writes, draws or acquired sections;
/// the enclosing job must be consumed or poisoned rather than claim rollback.
pub trait OreHost {
    fn min_build_height(&self) -> i32;
    /// Exclusive maximum of the actual build accessor.
    fn max_build_height(&self) -> i32;

    fn outside_build_height(&self, y: i32) -> bool {
        y < self.min_build_height() || y >= self.max_build_height()
    }

    /// Actual WorldGenLevel permission query. A fresh WorldGenRegion checks
    /// its center/write radius; only upgrading centers also constrain Y here.
    fn ensure_can_write(&mut self, point: DensityPoint) -> Result<bool>;

    /// WorldGenRegion.getHeight(OCEAN_FLOOR_WG, x, z): first available Y, which
    /// is ChunkAccess.getHeight(type, x & 15, z & 15) + 1. Use the live owner.
    fn ocean_floor_wg(&mut self, x: i32, z: i32) -> Result<i32>;

    /// Construct one fresh BulkSectionAccess after regular-ore sphere culling.
    /// This must not eagerly acquire sections or reuse a preceding leaf's cache.
    /// Subsequent first accesses acquire the actual sections, retaining their
    /// identity/cache and source exclusion mechanism until the matching close.
    fn begin_bulk_section_access(&mut self) -> Result<()>;

    /// Release every section acquired by this invocation exactly once, on both
    /// success and error. No fallback, default no-op or image-only rollback.
    fn close_bulk_section_access(&mut self) -> Result<()>;

    /// BulkSectionAccess.getSection followed by its direct state read. `None`
    /// means ONLY that the actual section index lies outside the accessor's
    /// section array; this check precedes chunk lookup. Section-array bounds
    /// need not be inferred from build Y for an unaligned accessor. Missing
    /// in-range chunks/sections and calls outside the bulk lifecycle must fail.
    /// Repeated accesses reuse the same acquired section, without another
    /// region chunk lookup. Empty existing sections still return their states.
    fn bulk_section_block(&mut self, point: DensityPoint) -> Result<Option<u32>>;

    /// Read through the same invocation's BulkSectionAccess. The original null
    /// section case is ordinary AIR, not VOID_AIR and not a missing-chunk fallback.
    fn bulk_block(&mut self, point: DensityPoint) -> Result<u32> {
        match self.bulk_section_block(point)? {
            Some(state) => Ok(state),
            None => canonical_air_state(),
        }
    }

    /// Write through the actual already-acquired LevelChunkSection using the
    /// source `setBlockState(localX, localY, localZ, state, false)` path. Preserve
    /// its palette and section counters. Do not route through ChunkAccess or
    /// region setBlock: this bypasses heightmaps, block entities, lighting,
    /// on-place/remove callbacks, ticks and automatic postprocessing marks.
    /// A preceding air query may change the last-section cache; the originally
    /// acquired target section must nevertheless remain the write owner.
    fn direct_section_set(&mut self, point: DensityPoint, state: u32) -> Result<()>;

    /// Actual WorldGenRegion world read, distinct from bulk access. Resolve the
    /// real chunk first, even for an out-of-height position; a ProtoChunk then
    /// supplies VOID_AIR outside its build bounds. Missing chunks always fail.
    fn block(&mut self, point: DensityPoint) -> Result<u32>;

    /// Actual region write, used with flags 2 by ScatteredOreFeature. Apply the
    /// region permission check, then delegate to the actual chunk owner and
    /// preserve its live heightmaps/callbacks, POI notification, block entities
    /// and state-requested postprocessing in source order. WorldGenRegion does
    /// not derive runtime neighbor/packet notifications from these flags.
    /// The bool is region acceptance, not proof of a changed voxel; scattered
    /// ore ignores it and stops at the first successful material predicate.
    fn set_block(&mut self, point: DensityPoint, state: u32, flags: i32) -> Result<bool>;
}

/// Ordinary AIR for BulkSectionAccess's actual out-of-section-array result.
pub fn canonical_air_state() -> Result<u32> {
    static AIR: OnceLock<u32> = OnceLock::new();
    if let Some(state) = AIR.get() {
        return Ok(*state);
    }
    let state = resolve_block_state(&serde_json::json!({"Name":"minecraft:air"}))?;
    let _ = AIR.set(state);
    Ok(state)
}

/// BlockState.isAir for the pinned canonical registry. Invalid state IDs fail
/// explicitly. The actual registry must match this catalog; matching resource
/// names alone do not establish an unmodified registry or tag snapshot.
pub fn is_air(state: u32) -> Result<bool> {
    Ok(matches!(
        block_state_name(state).context("ore air predicate received unknown canonical state")?,
        "air" | "cave_air" | "void_air"
    ))
}

/// OreFeature.canPlaceOre, with the source callback determining whether air
/// queries use BulkSectionAccess or normal WorldGenRegion reads. `state` is the
/// leaf's single captured target read; the ordered target loop must not reread
/// it after a failed target. The supported deterministic RuleTests draw no RNG.
pub fn can_place_ore<R, F>(
    state: u32,
    read_block: &mut F,
    random: &mut R,
    config: &OreConfiguration,
    target: &TargetBlockState,
    point: DensityPoint,
) -> Result<bool>
where
    R: RandomSource + ?Sized,
    F: FnMut(DensityPoint) -> Result<u32> + ?Sized,
{
    if !target.target.matches(state)? {
        return Ok(false);
    }
    if should_skip_air_check(random, config.discard_chance_on_air_exposure) {
        return Ok(true);
    }
    // Feature.isAdjacentToAir visits Direction.values(): DOWN, UP, NORTH,
    // SOUTH, WEST, EAST, stopping at the first actual air state.
    for (dx, dy, dz) in [
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
        (-1, 0, 0),
        (1, 0, 0),
    ] {
        let neighbor = DensityPoint::new(
            point.x.wrapping_add(dx),
            point.y.wrapping_add(dy),
            point.z.wrapping_add(dz),
        );
        if is_air(read_block(neighbor)?)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn should_skip_air_check(random: &mut (impl RandomSource + ?Sized), chance: f32) -> bool {
    if chance <= 0.0 {
        true
    } else if chance >= 1.0 {
        false
    } else {
        random.next_f32() >= chance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{random::LegacyRandomSource, worldgen::feature::config::OreRuleTest};

    fn state(name: &str) -> u32 {
        resolve_block_state(&serde_json::json!({"Name":format!("minecraft:{name}")})).unwrap()
    }

    fn config(chance: f32) -> (OreConfiguration, TargetBlockState) {
        let target = TargetBlockState {
            target: OreRuleTest::BlockMatch(state("stone")),
            state: state("diamond_ore"),
        };
        (
            OreConfiguration {
                targets: vec![target],
                size: 8,
                discard_chance_on_air_exposure: chance,
            },
            target,
        )
    }

    #[test]
    fn failed_target_and_zero_discard_make_no_air_queries_or_draws() -> Result<()> {
        let (config, target) = config(0.0);
        let point = DensityPoint::new(0, 0, 0);
        let mut random = LegacyRandomSource::new(123);
        let mut untouched = LegacyRandomSource::new(123);
        let mut unexpected = |_| -> Result<u32> { panic!("unexpected air query") };
        assert!(!can_place_ore(
            state("dirt"),
            &mut unexpected,
            &mut random,
            &config,
            &target,
            point,
        )?);
        assert!(can_place_ore(
            state("stone"),
            &mut unexpected,
            &mut random,
            &config,
            &target,
            point,
        )?);
        assert_eq!(random.next_i64(), untouched.next_i64());
        Ok(())
    }

    #[test]
    fn certain_discard_uses_source_direction_order_and_stops_at_air() -> Result<()> {
        let (config, target) = config(1.0);
        let point = DensityPoint::new(i32::MAX, i32::MIN, 9);
        let mut random = LegacyRandomSource::new(7);
        let mut untouched = LegacyRandomSource::new(7);
        let mut seen = Vec::new();
        let accepted = can_place_ore(
            state("stone"),
            &mut |neighbor| {
                seen.push(neighbor);
                Ok(if seen.len() == 3 {
                    state("cave_air")
                } else {
                    state("stone")
                })
            },
            &mut random,
            &config,
            &target,
            point,
        )?;
        assert!(!accepted);
        assert_eq!(
            seen,
            [
                DensityPoint::new(i32::MAX, i32::MAX, 9),
                DensityPoint::new(i32::MAX, i32::MIN + 1, 9),
                DensityPoint::new(i32::MAX, i32::MIN, 8),
            ]
        );
        assert_eq!(random.next_i64(), untouched.next_i64());
        Ok(())
    }

    #[test]
    fn fractional_discard_draws_once_after_matching_target() -> Result<()> {
        let (config, target) = config(0.5);
        let mut random = LegacyRandomSource::new(123);
        let mut expected = LegacyRandomSource::new(123);
        let skip = expected.next_f32() >= 0.5;
        let mut queries = 0;
        let accepted = can_place_ore(
            state("stone"),
            &mut |_| {
                queries += 1;
                Ok(state("void_air"))
            },
            &mut random,
            &config,
            &target,
            DensityPoint::new(0, 0, 0),
        )?;
        assert_eq!(accepted, skip);
        assert_eq!(queries, usize::from(!skip));
        assert_eq!(random.next_i64(), expected.next_i64());
        assert!(is_air(state("air"))?);
        assert!(is_air(state("cave_air"))?);
        assert!(is_air(state("void_air"))?);
        assert!(!is_air(state("water"))?);
        assert!(is_air(u32::MAX).is_err());
        Ok(())
    }
}
