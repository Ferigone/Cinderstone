//! Source AIR carver invocation over actual retained target and region owners.
//! No scheduler, generated structures, LIQUID pass or FULL chunk is inferred.
pub mod canyon;
pub mod cave;
pub mod config;
pub mod host;
pub mod mask;
pub mod nether;
pub mod providers;
pub mod world_carver;

use super::{
    biome_source::{
        select_end_biome, BuiltinMultiNoiseBiomeSource, ClimateSampler, ErosionSampler,
    },
    biome_temperature::BiomeTemperature,
    biome_zoom::BiomeZoom,
    biomes::BiomeTerrain,
};
use crate::{
    density::{ClimateTargetPoint, DensityGraph, DensityPoint},
    random::{LegacyRandomSource, WorldgenRandom},
};
use anyhow::{bail, ensure, Result};
pub use config::{BaseCarverConfig, ConfiguredCarver};
pub use host::CarverHost;
pub use mask::{AdditionalCarvingMask, CarvingMask};
use serde::Serialize;

/// Actual immutable generation settings captured by a neighbor ChunkAccess.
/// Ordering is holder encounter order. An empty list must be real registry data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarverBiomeSettings {
    pub air: Vec<String>,
}

/// Real configuration/biome-generation-settings registry for this invocation.
/// Unknown or unavailable identities must fail, never become empty/defaults.
pub trait CarverRegistry {
    fn generation_settings(&mut self, biome: &str) -> Result<CarverBiomeSettings>;
    fn configured_carver(&mut self, key: &str) -> Result<&ConfiguredCarver>;
}

/// Closed original registry snapshot. Its caller must establish that the actual
/// registry/tags have not been overridden; a matching resource name is not proof.
pub struct BuiltinCarverRegistry;
impl CarverRegistry for BuiltinCarverRegistry {
    fn generation_settings(&mut self, biome: &str) -> Result<CarverBiomeSettings> {
        Ok(CarverBiomeSettings {
            air: config::builtin_biome_carvers(biome)?.to_vec(),
        })
    }
    fn configured_carver(&mut self, key: &str) -> Result<&ConfiguredCarver> {
        config::builtin_configured_carver(key)
    }
}

enum CacheState {
    ObservedAbsent,
    Cached(CarverBiomeSettings),
    Unavailable,
}
/// Persistent actual neighbor carverBiome cache. Resolution is deliberately
/// inside this owner so hits suppress both source lookup and registry callbacks.
pub struct CarverBiomeCache {
    state: CacheState,
}
impl CarverBiomeCache {
    pub fn observed_absent() -> Self {
        Self {
            state: CacheState::ObservedAbsent,
        }
    }
    pub fn existing(settings: CarverBiomeSettings) -> Self {
        Self {
            state: CacheState::Cached(settings),
        }
    }
    pub fn unavailable() -> Self {
        Self {
            state: CacheState::Unavailable,
        }
    }
    pub fn is_cached(&self) -> bool {
        matches!(self.state, CacheState::Cached(_))
    }
    fn resolve(
        &mut self,
        resolve: impl FnOnce() -> Result<(String, CarverBiomeSettings)>,
    ) -> Result<(CarverBiomeSettings, Option<String>, bool)> {
        match &self.state {
            CacheState::Unavailable => bail!("neighbor carverBiome cache owner unavailable"),
            CacheState::Cached(settings) => return Ok((settings.clone(), None, true)),
            CacheState::ObservedAbsent => {}
        }
        let (biome, settings) = resolve()?;
        self.state = CacheState::Cached(settings.clone());
        Ok((settings, Some(biome), false))
    }
}

/// The actual 17x17 region/chunk cache owners. Implementations must select the
/// requested neighbor even on a cache hit and fail if that dependency is absent.
pub trait CarvingNeighbors {
    fn carver_biome_cache(&mut self, chunk: (i32, i32)) -> Result<&mut CarverBiomeCache>;
}
/// Actual target accessor/upgrade owners, queried after invocation RNG creation.
/// The current fresh-chunk adapter rejects upgrading/retrogen rather than
/// applying ordinary full-height metadata to its different source accessor.
pub trait CarvingRegionFacts {
    fn generation_height(&mut self, target: (i32, i32)) -> Result<(i32, i32)>;
    fn is_upgrading(&mut self, target: (i32, i32)) -> Result<bool>;
}

/// Actual Blender scheduler action. ObservedNoUpdate performs NO setter and
/// preserves an existing additional predicate. It does not mean clear-to-false.
pub enum CarvingMaskFilter {
    ObservedNoUpdate,
    Install(AdditionalCarvingMask),
    Unavailable,
}
enum MaskState {
    ObservedAbsent,
    Existing(CarvingMask),
    Unavailable,
}
/// Actual AIR mask slot and pending scheduler filter for one target chunk.
pub struct AirMaskOwner {
    target: (i32, i32),
    mask: MaskState,
    filter: CarvingMaskFilter,
}
impl AirMaskOwner {
    pub fn observed_absent(target: (i32, i32), filter: CarvingMaskFilter) -> Self {
        Self {
            target,
            mask: MaskState::ObservedAbsent,
            filter,
        }
    }
    pub fn existing(target: (i32, i32), mask: CarvingMask, filter: CarvingMaskFilter) -> Self {
        Self {
            target,
            mask: MaskState::Existing(mask),
            filter,
        }
    }
    pub fn unavailable(target: (i32, i32), filter: CarvingMaskFilter) -> Self {
        Self {
            target,
            mask: MaskState::Unavailable,
            filter,
        }
    }
    fn get_or_create(&mut self, min_y: i32, height: i32) -> Result<&mut CarvingMask> {
        if matches!(self.mask, MaskState::ObservedAbsent) {
            self.mask = MaskState::Existing(CarvingMask::new(height, min_y)?);
        }
        match &mut self.mask {
            MaskState::Existing(mask) => {
                ensure!(
                    mask.min_y() == min_y,
                    "AIR mask belongs to another build-height owner"
                );
                Ok(mask)
            }
            _ => bail!("actual AIR carving mask owner unavailable"),
        }
    }
    pub(super) fn install_before_invocation(
        &mut self,
        target: (i32, i32),
        min_y: i32,
        height: i32,
    ) -> Result<()> {
        ensure!(self.target == target, "AIR mask belongs to another chunk");
        match std::mem::replace(&mut self.filter, CarvingMaskFilter::Unavailable) {
            CarvingMaskFilter::ObservedNoUpdate => Ok(()),
            CarvingMaskFilter::Install(filter) => {
                self.get_or_create(min_y, height)?
                    .set_additional_mask(filter);
                Ok(())
            }
            CarvingMaskFilter::Unavailable => {
                bail!("actual Blender carving-mask filter action unavailable")
            }
        }
    }
    pub(super) fn acquire_after_rng(mut self, min_y: i32, height: i32) -> Result<CarvingMask> {
        let _ = self.get_or_create(min_y, height)?;
        match self.mask {
            MaskState::Existing(mask) => Ok(mask),
            _ => unreachable!(),
        }
    }
}

/// Explicit actual source owner, reused on this worker across BIOMES/CARVERS
/// according to that worker's real RTree history. No selector is reconstructed.
pub enum CarverBiomeSource<'a> {
    MultiNoise(&'a mut BuiltinMultiNoiseBiomeSource),
    End,
}

/// Raw RandomState.sampler semantics for CARVERS, deliberately distinct from
/// the wrapped chunk climate owner used by BIOMES. Construction is internal and
/// tied to the target's captured generator; this is not a raw-graph fallback.
pub(super) struct RawCarverSampler<'g> {
    graph: &'g DensityGraph,
}
impl<'g> RawCarverSampler<'g> {
    fn new(graph: &'g DensityGraph) -> Self {
        Self { graph }
    }
}
impl ClimateSampler for RawCarverSampler<'_> {
    type Error = anyhow::Error;
    fn sample_climate(&mut self, x: i32, y: i32, z: i32) -> Result<ClimateTargetPoint> {
        self.graph.sample_climate(x, y, z)
    }
}
impl ErosionSampler for RawCarverSampler<'_> {
    type Error = anyhow::Error;
    fn sample_erosion(&mut self, point: DensityPoint) -> Result<f64> {
        let root = self
            .graph
            .root("erosion")
            .ok_or_else(|| anyhow::anyhow!("raw router has no erosion root"))?;
        Ok(self.graph.eval(root, point))
    }
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CarverBiomeQueryKind {
    NeighborSettings,
    TopMaterial,
}
#[derive(Debug, Clone, Serialize)]
pub struct CarverBiomeQuery {
    pub kind: CarverBiomeQueryKind,
    pub quart: (i32, i32, i32),
    pub biome: String,
}

pub(super) struct RawBiomeManager<'g, 'w> {
    sampler: RawCarverSampler<'g>,
    source: CarverBiomeSource<'w>,
    zoom: BiomeZoom,
    queries: Vec<CarverBiomeQuery>,
}
impl<'g, 'w> RawBiomeManager<'g, 'w> {
    pub(super) fn new(
        graph: &'g DensityGraph,
        source: CarverBiomeSource<'w>,
        zoom: BiomeZoom,
    ) -> Self {
        Self {
            sampler: RawCarverSampler::new(graph),
            source,
            zoom,
            queries: Vec::new(),
        }
    }
    fn query(&mut self, kind: CarverBiomeQueryKind, x: i32, y: i32, z: i32) -> Result<String> {
        let biome = match &mut self.source {
            CarverBiomeSource::MultiNoise(source) => {
                source.get_noise_biome(x, y, z, &mut self.sampler)?
            }
            CarverBiomeSource::End => select_end_biome(x, y, z, &mut self.sampler)?.to_owned(),
        };
        self.queries.push(CarverBiomeQuery {
            kind,
            quart: (x, y, z),
            biome: biome.clone(),
        });
        Ok(biome)
    }
    pub(super) fn neighbor_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        self.query(CarverBiomeQueryKind::NeighborSettings, x, y, z)
    }
    pub(super) fn biome(&mut self, point: DensityPoint) -> Result<String> {
        let (x, y, z) = self.zoom.selected_quart(point.x, point.y, point.z);
        self.query(CarverBiomeQueryKind::TopMaterial, x, y, z)
    }
    pub(super) fn into_queries(self) -> Vec<CarverBiomeQuery> {
        self.queries
    }
}

/// Mandatory synchronous invocation owners. The world seed and zoom must belong
/// to this actual level; the adapter checks the seed against its retained owner.
/// No field has a dummy/default implementation. Worker-local source/temperature
/// owners remain borrowed, never captured in the shared generator or job.
pub struct AirCarvingContext<'a> {
    pub world_seed: i64,
    pub source: CarverBiomeSource<'a>,
    pub zoom: BiomeZoom,
    pub temperature: &'a mut BiomeTemperature,
    pub neighbors: &'a mut dyn CarvingNeighbors,
    pub registry: &'a mut dyn CarverRegistry,
    pub facts: &'a mut dyn CarvingRegionFacts,
    pub mask: AirMaskOwner,
}

pub(super) trait CarvingTargetHost: CarverHost {
    fn neighbor_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String>;
}
#[derive(Debug, Serialize)]
pub struct NeighborCarvers {
    pub chunk: (i32, i32),
    pub cached: bool,
    pub resolved_biome: Option<String>,
    pub air: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct CarverAttempt {
    pub chunk: (i32, i32),
    pub index: i32,
    pub key: String,
    pub count_after_seed: i32,
    pub start: bool,
    pub count_after_start: i32,
    pub carved: Option<bool>,
    pub count_after_carve: i32,
}
#[derive(Debug, Serialize)]
pub struct CarvingReport {
    pub neighbors: Vec<NeighborCarvers>,
    pub attempts: Vec<CarverAttempt>,
    pub biome_queries: Vec<CarverBiomeQuery>,
    pub random_count: i32,
    pub random_state48: u64,
}

pub(super) fn run_air(
    host: &mut impl CarvingTargetHost,
    world_seed: i64,
    neighbors: &mut dyn CarvingNeighbors,
    registry: &mut dyn CarverRegistry,
    random: &mut WorldgenRandom<LegacyRandomSource>,
    mask: &mut CarvingMask,
) -> Result<CarvingReport> {
    let target = host.chunk_pos();
    let mut report = CarvingReport {
        neighbors: Vec::new(),
        attempts: Vec::new(),
        biome_queries: Vec::new(),
        random_count: 0,
        random_state48: 0,
    };
    for dx in -8..=8 {
        for dz in -8..=8 {
            let chunk = (target.0.wrapping_add(dx), target.1.wrapping_add(dz));
            let cache = neighbors.carver_biome_cache(chunk)?;
            let (settings, biome, cached) = cache.resolve(|| {
                let biome =
                    host.neighbor_biome(chunk.0.wrapping_shl(2), 0, chunk.1.wrapping_shl(2))?;
                let settings = registry.generation_settings(&biome)?;
                Ok((biome, settings))
            })?;
            report.neighbors.push(NeighborCarvers {
                chunk,
                cached,
                resolved_biome: biome,
                air: settings.air.clone(),
            });
            for (index, key) in settings.air.iter().enumerate() {
                let index = i32::try_from(index)?;
                let configured = registry.configured_carver(key)?;
                random.set_large_feature_seed(
                    world_seed.wrapping_add(i64::from(index)),
                    chunk.0,
                    chunk.1,
                );
                let count_after_seed = random.count();
                let start = match configured {
                    ConfiguredCarver::Cave(config) => cave::is_start_chunk(config, random),
                    ConfiguredCarver::NetherCave(config) => nether::is_start_chunk(config, random),
                    ConfiguredCarver::Canyon(config) => canyon::is_start_chunk(config, random),
                };
                let count_after_start = random.count();
                let carved = if start {
                    Some(match configured {
                        ConfiguredCarver::Cave(config) => {
                            cave::carve(host, config, random, chunk, mask)?
                        }
                        ConfiguredCarver::NetherCave(config) => {
                            nether::carve(host, config, random, chunk, mask)?
                        }
                        ConfiguredCarver::Canyon(config) => {
                            canyon::carve(host, config, random, chunk, mask)?
                        }
                    })
                } else {
                    None
                };
                report.attempts.push(CarverAttempt {
                    chunk,
                    index,
                    key: key.clone(),
                    count_after_seed,
                    start,
                    count_after_start,
                    carved,
                    count_after_carve: random.count(),
                });
            }
        }
    }
    report.random_count = random.count();
    report.random_state48 = random.source().state48();
    Ok(report)
}

/// Successful native AIR-carver data, actual mask and source observations. This
/// is not an original generation-status/readiness witness or FULL publication.
pub struct CarvedBiomeTerrain {
    terrain: BiomeTerrain,
    mask: CarvingMask,
    report: CarvingReport,
}
impl CarvedBiomeTerrain {
    pub(super) fn new(terrain: BiomeTerrain, mask: CarvingMask, report: CarvingReport) -> Self {
        Self {
            terrain,
            mask,
            report,
        }
    }
    pub fn terrain(&self) -> &BiomeTerrain {
        &self.terrain
    }
    pub fn mask(&self) -> &CarvingMask {
        &self.mask
    }
    pub fn report(&self) -> &CarvingReport {
        &self.report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn neighbor_cache_hits_suppress_source_and_registry_and_failures_stay_misses() -> Result<()> {
        let mut cache = CarverBiomeCache::observed_absent();
        let mut calls = 0;
        assert!(cache
            .resolve(|| {
                calls += 1;
                bail!("registry owner unavailable after real source lookup")
            })
            .is_err());
        assert!(!cache.is_cached());
        let expected = CarverBiomeSettings {
            air: config::builtin_biome_carvers("minecraft:plains")?.to_vec(),
        };
        let (settings, biome, cached) = cache.resolve(|| {
            calls += 1;
            Ok(("minecraft:plains".to_owned(), expected.clone()))
        })?;
        assert_eq!(settings, expected);
        assert_eq!(biome.as_deref(), Some("minecraft:plains"));
        assert!(!cached);
        let (_, biome, cached) = cache.resolve(|| panic!("hit must suppress both callbacks"))?;
        assert!(cached);
        assert!(biome.is_none());
        assert_eq!(calls, 2);
        assert!(CarverBiomeCache::unavailable()
            .resolve(|| panic!("no missing-owner fallback"))
            .is_err());
        Ok(())
    }

    #[test]
    fn no_blender_update_preserves_the_actual_additional_mask() -> Result<()> {
        let mut mask = CarvingMask::new(256, 0)?;
        mask.set(1, 20, 3)?;
        mask.set_additional_mask(Box::new(|x, y, z| Ok((x, y, z) == (4, 21, 6))));
        let mut owner = AirMaskOwner::existing((0, 0), mask, CarvingMaskFilter::ObservedNoUpdate);
        owner.install_before_invocation((0, 0), 0, 256)?;
        let mask = owner.acquire_after_rng(0, 256)?;
        assert!(mask.get(1, 20, 3)?);
        assert!(
            mask.get(4, 21, 6)?,
            "no setter must preserve the existing additional predicate"
        );
        assert!(!mask.get(4, 22, 6)?);
        let mut absent = AirMaskOwner::observed_absent(
            (0, 0),
            CarvingMaskFilter::Install(Box::new(|_, _, _| Ok(true))),
        );
        absent.install_before_invocation((0, 0), 0, 256)?;
        assert!(
            absent.acquire_after_rng(0, 256)?.get(0, -1, 0)?,
            "installed filter precedes even an invalid bit index"
        );
        let mut unknown = AirMaskOwner::unavailable((0, 0), CarvingMaskFilter::ObservedNoUpdate);
        unknown.install_before_invocation((0, 0), 0, 256)?;
        assert!(unknown.acquire_after_rng(0, 256).is_err());
        let mut wrong = AirMaskOwner::observed_absent((1, 0), CarvingMaskFilter::ObservedNoUpdate);
        assert!(wrong.install_before_invocation((0, 0), 0, 256).is_err());
        Ok(())
    }

    struct EndNeighbors {
        caches: BTreeMap<(i32, i32), CarverBiomeCache>,
        reads: Vec<(i32, i32)>,
    }
    impl EndNeighbors {
        fn new() -> Self {
            Self {
                caches: (-8..=8)
                    .flat_map(|x| {
                        (-8..=8).map(move |z| ((x, z), CarverBiomeCache::observed_absent()))
                    })
                    .collect(),
                reads: Vec::new(),
            }
        }
    }
    impl CarvingNeighbors for EndNeighbors {
        fn carver_biome_cache(&mut self, chunk: (i32, i32)) -> Result<&mut CarverBiomeCache> {
            self.reads.push(chunk);
            self.caches
                .get_mut(&chunk)
                .ok_or_else(|| anyhow::anyhow!("actual neighbor unavailable"))
        }
    }
    /// Empty actual End lists isolate orchestration and cache/history effects;
    /// material callbacks are deliberately unavailable, not production no-ops.
    struct EndHost {
        queries: Vec<(i32, i32, i32)>,
    }
    impl CarvingTargetHost for EndHost {
        fn neighbor_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
            self.queries.push((x, y, z));
            Ok("minecraft:the_end".to_owned())
        }
    }
    impl CarverHost for EndHost {
        fn chunk_pos(&self) -> (i32, i32) {
            (0, 0)
        }
        fn min_gen_y(&self) -> i32 {
            0
        }
        fn gen_depth(&self) -> i32 {
            128
        }
        fn is_upgrading(&self) -> bool {
            false
        }
        fn block(&mut self, _: DensityPoint) -> Result<u32> {
            bail!("unexpected material access")
        }
        fn set_block(&mut self, _: DensityPoint, _: u32) -> Result<()> {
            bail!("unexpected material write")
        }
        fn has_fluid(&self, _: u32) -> Result<bool> {
            bail!("unexpected fluid access")
        }
        fn aquifer_substance(&mut self, _: DensityPoint) -> Result<Option<u32>> {
            bail!("unexpected aquifer access")
        }
        fn aquifer_should_schedule_fluid_update(&self) -> bool {
            panic!("unexpected aquifer flag access")
        }
        fn top_material(&mut self, _: DensityPoint, _: bool) -> Result<Option<u32>> {
            bail!("unexpected surface access")
        }
        fn mark_postprocessing(&mut self, _: DensityPoint) -> Result<()> {
            bail!("unexpected postprocessing")
        }
    }
    #[test]
    fn real_empty_end_settings_still_visit_all_neighbors_and_hits_preserve_history() -> Result<()> {
        let mut host = EndHost {
            queries: Vec::new(),
        };
        let mut neighbors = EndNeighbors::new();
        let mut registry = BuiltinCarverRegistry;
        // This private kernel test controls RNG state; the public staged entry
        // constructs its own process-global source even when these lists empty.
        let mut random = WorldgenRandom::new(LegacyRandomSource::new(123));
        let initial_state = random.source().state48();
        let mut mask = CarvingMask::new(256, 0)?;
        let first = run_air(
            &mut host,
            123,
            &mut neighbors,
            &mut registry,
            &mut random,
            &mut mask,
        )?;
        let expected: Vec<_> = (-8..=8)
            .flat_map(|x| (-8..=8).map(move |z| (x, z)))
            .collect();
        assert_eq!(neighbors.reads, expected);
        assert_eq!(
            host.queries,
            expected
                .iter()
                .map(|&(x, z)| (x << 2, 0, z << 2))
                .collect::<Vec<_>>()
        );
        assert_eq!(first.neighbors.len(), 289);
        assert!(first
            .neighbors
            .iter()
            .all(|entry| !entry.cached && entry.air.is_empty()));
        assert!(first.attempts.is_empty());
        assert_eq!(first.random_count, 0);
        assert_eq!(first.random_state48, initial_state);
        let second = run_air(
            &mut host,
            123,
            &mut neighbors,
            &mut registry,
            &mut random,
            &mut mask,
        )?;
        assert!(second
            .neighbors
            .iter()
            .all(|entry| entry.cached && entry.resolved_biome.is_none()));
        assert_eq!(
            host.queries.len(),
            289,
            "warm neighbor owners must suppress source history changes"
        );
        assert_eq!(
            neighbors.reads.len(),
            578,
            "cache hits still select actual region chunks"
        );
        assert!(mask.to_array().is_empty());
        Ok(())
    }

    #[test]
    fn absent_region_dependency_stops_without_undoing_earlier_owner_effects() -> Result<()> {
        let mut neighbors = EndNeighbors::new();
        neighbors.caches.remove(&(-8, -6));
        let mut host = EndHost {
            queries: Vec::new(),
        };
        let mut random = WorldgenRandom::new(LegacyRandomSource::new(123));
        assert!(run_air(
            &mut host,
            123,
            &mut neighbors,
            &mut BuiltinCarverRegistry,
            &mut random,
            &mut CarvingMask::new(256, 0)?
        )
        .is_err());
        assert_eq!(neighbors.reads, vec![(-8, -8), (-8, -7), (-8, -6)]);
        assert_eq!(host.queries, vec![(-32, 0, -32), (-32, 0, -28)]);
        assert!(neighbors.caches[&(-8, -8)].is_cached());
        assert!(neighbors.caches[&(-8, -7)].is_cached());
        Ok(())
    }
}
