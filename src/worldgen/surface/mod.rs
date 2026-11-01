//! The Java 1.21 SURFACE stage: rule compilation, live column traversal and
//! badlands/iceberg extensions. This stage consumes an existing noise chunk;
//! it neither selects replacement biomes nor publishes a FULL chunk.
mod rules;

use super::random_state::RandomStateRandoms;
use crate::{
    density::DensityPoint,
    noise::NormalNoise,
    random::{PositionalRandomFactory, RandomSource},
};
use anyhow::{bail, Context, Result};
use rules::{PreliminarySurfaceCache, Rule, RuleContext};
use serde_json::{json, Value};
use std::sync::OnceLock;

/// Dependencies owned by the actual ProtoChunk, biome manager and NoiseChunk.
/// Queries must fail when the owner/context is unavailable, rather than provide
/// a stand-in biome, heightmap, temperature or preliminary density surface.
/// `set_block` must update heightmaps and mark positions containing fluid for
/// postprocessing, exactly as SurfaceSystem's BlockColumn adapter does.
pub trait SurfaceHost {
    fn min_y(&self) -> i32;
    /// Exclusive upper bound of the chunk build-height range.
    fn max_y(&self) -> i32;
    /// WorldGenerationContext.getMinGenY, separate from the chunk lower bound.
    fn min_gen_y(&self) -> i32;
    fn gen_depth(&self) -> i32;
    fn block(&mut self, x: i32, y: i32, z: i32) -> Result<u32>;
    fn set_block(&mut self, x: i32, y: i32, z: i32, state: u32) -> Result<()>;
    fn is_air(&self, state: u32) -> bool;
    fn has_fluid(&self, state: u32) -> bool;
    fn is_named_block(&self, state: u32, name: &str) -> bool;
    /// Highest non-air block Y from the live WORLD_SURFACE_WG heightmap.
    fn world_surface_wg_height(&mut self, local_x: i32, local_z: i32) -> Result<i32>;
    /// The current call phase's supplied block-biome function. build_surface
    /// uses BiomeManager.getBiome, including its actual source and zoom seed.
    fn biome(&mut self, x: i32, y: i32, z: i32) -> Result<String>;
    /// Biome.coldEnoughToSnow at this actual biome/position.
    fn cold_enough_to_snow(&mut self, biome: &str, x: i32, y: i32, z: i32) -> Result<bool>;
    /// Biome.shouldMeltFrozenOceanIcebergSlightly at this position.
    fn should_melt_frozen_ocean_iceberg_slightly(
        &mut self,
        biome: &str,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<bool>;
    /// NoiseChunk.preliminarySurfaceLevel, including its own quart-cell cache.
    fn preliminary_surface_level(&mut self, x: i32, z: i32) -> Result<i32>;
}

/// Pinned packaged settings, excluding only router and spawn-target definitions
/// already owned elsewhere. Callers can reuse material and aquifer flags.
pub fn builtin_settings(name: &str) -> Result<&'static Value> {
    static DATA: OnceLock<Value> = OnceLock::new();
    let data = DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../fixtures/world-generation-surface-rules.json"
        ))
        .expect("packaged surface settings")
    });
    // ResourceLocation defaults an explicitly empty namespace as well as an
    // omitted namespace to "minecraft".
    let name = if let Some(path) = name.strip_prefix(':') {
        format!("minecraft:{path}")
    } else if name.contains(':') {
        name.to_owned()
    } else {
        format!("minecraft:{name}")
    };
    data["settings"]
        .get(&name)
        .with_context(|| format!("unknown noise settings {name}"))
}

pub struct SurfaceSystem {
    default_block: u32,
    default_block_name: String,
    sea_level: i32,
    random: PositionalRandomFactory,
    surface: NormalNoise,
    secondary: NormalNoise,
    clay_offset: NormalNoise,
    badlands_pillar: NormalNoise,
    badlands_roof: NormalNoise,
    badlands_surface: NormalNoise,
    iceberg_pillar: NormalNoise,
    iceberg_roof: NormalNoise,
    iceberg_surface: NormalNoise,
    clay_bands: [u32; 192],
    packed_ice: u32,
    snow_block: u32,
    rule: Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceReport {
    pub columns: usize,
    pub rule_replacements: usize,
}

impl SurfaceSystem {
    /// Resolve every state through the runtime's canonical registry. Missing
    /// state properties are not silently converted into a different material.
    pub fn from_settings(
        name: &str,
        randoms: &mut RandomStateRandoms,
        mut resolve: impl FnMut(&Value) -> Result<u32>,
    ) -> Result<Self> {
        let settings = builtin_settings(name)?;
        let expected_legacy = settings["legacy_random_source"]
            .as_bool()
            .context("missing legacy setting")?;
        if expected_legacy != randoms.uses_legacy_random_source() {
            bail!("surface random algorithm differs from noise settings");
        }
        let default_block = resolve(&settings["default_block"])?;
        let default_block_name = settings["default_block"]["Name"]
            .as_str()
            .context("missing default block name")?
            .to_owned();
        let sea_level = settings["sea_level"]
            .as_i64()
            .context("missing sea level")?
            .try_into()?;
        let mut palette = Vec::new();
        for name in [
            "terracotta",
            "orange_terracotta",
            "yellow_terracotta",
            "brown_terracotta",
            "red_terracotta",
            "white_terracotta",
            "light_gray_terracotta",
        ] {
            palette.push(resolve(&json!({"Name":format!("minecraft:{name}")}))?);
        }
        let random = randoms.surface_random();
        let clay_bands = generate_bands(random.from_hash_of("minecraft:clay_bands"), &palette)?;
        let rule = Rule::parse(&settings["surface_rule"], randoms, &mut resolve)?;
        Ok(Self {
            default_block,
            default_block_name,
            sea_level,
            random,
            clay_bands,
            rule,
            surface: randoms.get_or_create_noise("minecraft:surface")?.clone(),
            secondary: randoms
                .get_or_create_noise("minecraft:surface_secondary")?
                .clone(),
            clay_offset: randoms
                .get_or_create_noise("minecraft:clay_bands_offset")?
                .clone(),
            badlands_pillar: randoms
                .get_or_create_noise("minecraft:badlands_pillar")?
                .clone(),
            badlands_roof: randoms
                .get_or_create_noise("minecraft:badlands_pillar_roof")?
                .clone(),
            badlands_surface: randoms
                .get_or_create_noise("minecraft:badlands_surface")?
                .clone(),
            iceberg_pillar: randoms
                .get_or_create_noise("minecraft:iceberg_pillar")?
                .clone(),
            iceberg_roof: randoms
                .get_or_create_noise("minecraft:iceberg_pillar_roof")?
                .clone(),
            iceberg_surface: randoms
                .get_or_create_noise("minecraft:iceberg_surface")?
                .clone(),
            packed_ice: resolve(&json!({"Name":"minecraft:packed_ice"}))?,
            snow_block: resolve(&json!({"Name":"minecraft:snow_block"}))?,
        })
    }

    pub fn surface_depth(&self, x: i32, z: i32) -> i32 {
        let value = self.surface.noise(x as f64, 0.0, z as f64);
        (value * 2.75 + 3.0 + self.random.at(x, 0, z).next_f64() * 0.25) as i32
    }
    pub fn surface_secondary(&self, x: i32, z: i32) -> f64 {
        self.secondary.noise(x as f64, 0.0, z as f64)
    }
    pub fn band(&self, x: i32, y: i32, z: i32) -> Result<u32> {
        // Avoid rounding the addition itself at a value just below +0.5.
        let value = self.clay_offset.noise(x as f64, 0.0, z as f64) * 4.0;
        let integral = value.floor();
        let offset = (integral + if value - integral >= 0.5 { 1.0 } else { 0.0 }) as i32;
        let index = y.wrapping_add(offset).wrapping_add(192) % 192;
        self.clay_bands
            .get(index as usize)
            .copied()
            .context("Java clay-band index outside array")
    }

    /// SurfaceSystem.topMaterial for this owner's compiled built-in rule.
    ///
    /// Supply the actual CarvingContext bounds, biome function, current chunk
    /// heightmaps and retained NoiseChunk preliminary-surface owner through
    /// `host`. `has_fluid` describes the caller's carved replacement above the
    /// queried position. It is not inferred from the queried block's state.
    ///
    /// This evaluates one fresh rule context. It does not scan or write blocks,
    /// run badlands/iceberg extensions, or advance a generation stage. `None`
    /// is the source's empty Optional, leaving any replacement to the caller.
    pub fn top_material(
        &self,
        host: &mut impl SurfaceHost,
        point: DensityPoint,
        has_fluid: bool,
    ) -> Result<Option<u32>> {
        let mut preliminary = PreliminarySurfaceCache::default();
        let mut context = RuleContext::new(host, self, point.x, point.z, &mut preliminary);
        let water_height = if has_fluid {
            point.y.wrapping_add(1)
        } else {
            i32::MIN
        };
        context.update_y(point.y, water_height, 1, 1);
        self.rule.apply(&mut context, self)
    }

    pub fn build_surface(
        &self,
        host: &mut impl SurfaceHost,
        chunk_x: i32,
        chunk_z: i32,
        use_legacy_biome_height: bool,
    ) -> Result<SurfaceReport> {
        if host.min_y() >= host.max_y() {
            bail!("empty surface generation height range");
        }
        let mut report = SurfaceReport {
            columns: 0,
            rule_replacements: 0,
        };
        let mut preliminary_cache = PreliminarySurfaceCache::default();
        for local_x in 0..16 {
            for local_z in 0..16 {
                let x = chunk_x.wrapping_mul(16).wrapping_add(local_x);
                let z = chunk_z.wrapping_mul(16).wrapping_add(local_z);
                let initial_top = host
                    .world_surface_wg_height(local_x, local_z)?
                    .wrapping_add(1);
                let biome = host.biome(
                    x,
                    if use_legacy_biome_height {
                        0
                    } else {
                        initial_top
                    },
                    z,
                )?;
                if biome == "minecraft:eroded_badlands" {
                    self.eroded_badlands(host, x, z, initial_top)?;
                }
                let top = host
                    .world_surface_wg_height(local_x, local_z)?
                    .wrapping_add(1);
                let mut context = RuleContext::new(host, self, x, z, &mut preliminary_cache);
                let min_y = context.host.min_y();
                let mut stone_above: i32 = 0;
                let mut water_height = i32::MIN;
                let mut stone_bottom = i32::MAX;
                for y in (min_y..=top).rev() {
                    let state = context.host.block(x, y, z)?;
                    if context.host.is_air(state) {
                        stone_above = 0;
                        water_height = i32::MIN;
                        continue;
                    }
                    if context.host.has_fluid(state) {
                        if water_height == i32::MIN {
                            water_height = y.wrapping_add(1);
                        }
                        continue;
                    }
                    if stone_bottom >= y {
                        // DimensionType.WAY_BELOW_MIN_Y; only used if the run
                        // remains solid all the way through the lower boundary.
                        stone_bottom = -32512;
                        let mut below = y.wrapping_sub(1);
                        while below >= min_y.wrapping_sub(1) {
                            let state = context.host.block(x, below, z)?;
                            if context.host.is_air(state) || context.host.has_fluid(state) {
                                stone_bottom = below.wrapping_add(1);
                                break;
                            }
                            below = below.wrapping_sub(1);
                        }
                    }
                    stone_above = stone_above.wrapping_add(1);
                    let stone_below = y.wrapping_sub(stone_bottom).wrapping_add(1);
                    context.update_y(y, water_height, stone_below, stone_above);
                    if state == self.default_block {
                        if let Some(replacement) = self.rule.apply(&mut context, self)? {
                            context.host.set_block(x, y, z, replacement)?;
                            report.rule_replacements += 1;
                        }
                    }
                }
                if biome == "minecraft:frozen_ocean" || biome == "minecraft:deep_frozen_ocean" {
                    let minimum_surface = context.min_surface_level()?;
                    self.frozen_ocean(context.host, minimum_surface, &biome, x, z, initial_top)?;
                }
                report.columns += 1;
            }
        }
        Ok(report)
    }

    fn eroded_badlands(&self, host: &mut impl SurfaceHost, x: i32, z: i32, top: i32) -> Result<()> {
        let pillar = (self.badlands_surface.noise(x as f64, 0.0, z as f64) * 8.25)
            .abs()
            .min(
                self.badlands_pillar
                    .noise(x as f64 * 0.2, 0.0, z as f64 * 0.2)
                    * 15.0,
            );
        if pillar <= 0.0 {
            return Ok(());
        }
        let roof = (self
            .badlands_roof
            .noise(x as f64 * 0.75, 0.0, z as f64 * 0.75)
            * 1.5)
            .abs();
        let upper =
            (64.0 + (pillar * pillar * 2.5).min((roof * 50.0).ceil() + 24.0)).floor() as i32;
        if top > upper {
            return Ok(());
        }
        for y in (host.min_y()..=upper).rev() {
            let state = host.block(x, y, z)?;
            if host.is_named_block(state, &self.default_block_name) {
                break;
            }
            if host.is_named_block(state, "minecraft:water") {
                return Ok(());
            }
        }
        for y in (host.min_y()..=upper).rev() {
            let state = host.block(x, y, z)?;
            if !host.is_air(state) {
                break;
            }
            host.set_block(x, y, z, self.default_block)?;
        }
        Ok(())
    }

    fn frozen_ocean(
        &self,
        host: &mut impl SurfaceHost,
        minimum_surface: i32,
        biome: &str,
        x: i32,
        z: i32,
        top: i32,
    ) -> Result<()> {
        let pillar = (self.iceberg_surface.noise(x as f64, 0.0, z as f64) * 8.25)
            .abs()
            .min(
                self.iceberg_pillar
                    .noise(x as f64 * 1.28, 0.0, z as f64 * 1.28)
                    * 15.0,
            );
        if pillar <= 1.8 {
            return Ok(());
        }
        let roof = (self
            .iceberg_roof
            .noise(x as f64 * 1.17, 0.0, z as f64 * 1.17)
            * 1.5)
            .abs();
        let mut height = (pillar * pillar * 1.2).min((roof * 40.0).ceil() + 14.0);
        if host.should_melt_frozen_ocean_iceberg_slightly(biome, x, 63, z)? {
            height -= 2.0;
        }
        let bottom = if height > 2.0 {
            let bottom = self.sea_level as f64 - height - 7.0;
            height += self.sea_level as f64;
            bottom
        } else {
            height = 0.0;
            0.0
        };
        let mut random = self.random.at(x, 0, z);
        let snow_limit = 2 + random.next_bounded_i32(4)?;
        let snow_min = self
            .sea_level
            .wrapping_add(18)
            .wrapping_add(random.next_bounded_i32(10)?);
        let mut snow_count = 0;
        let start = top.max((height as i32).wrapping_add(1));
        for y in (minimum_surface..=start).rev() {
            let state = host.block(x, y, z)?;
            let replace_air = host.is_air(state) && y < height as i32 && random.next_f64() > 0.01;
            let replace_water = if replace_air {
                false
            } else {
                host.is_named_block(state, "minecraft:water")
                    && y > bottom as i32
                    && y < self.sea_level
                    && bottom != 0.0
                    && random.next_f64() > 0.15
            };
            if replace_air || replace_water {
                let replacement = if snow_count <= snow_limit && y > snow_min {
                    snow_count += 1;
                    self.snow_block
                } else {
                    self.packed_ice
                };
                host.set_block(x, y, z, replacement)?;
            }
        }
        Ok(())
    }
}

fn generate_bands(mut random: impl RandomSource, palette: &[u32]) -> Result<[u32; 192]> {
    let mut bands = [palette[0]; 192];
    let mut index: usize = 0;
    while index < bands.len() {
        index += random.next_bounded_i32(5)? as usize + 1;
        if index < bands.len() {
            bands[index] = palette[1];
        }
        index += 1;
    }
    for (minimum_length, color) in [(1, palette[2]), (2, palette[3]), (1, palette[4])] {
        let count = random.next_bounded_i32(10)? + 6;
        for _ in 0..count {
            let length = minimum_length + random.next_bounded_i32(3)? as usize;
            let start = random.next_bounded_i32(192)? as usize;
            for value in bands.iter_mut().skip(start).take(length) {
                *value = color;
            }
        }
    }
    let white_count = random.next_bounded_i32(7)? + 9;
    let mut index = 0_usize;
    for _ in 0..white_count {
        if index >= bands.len() {
            break;
        }
        bands[index] = palette[5];
        if index as i32 - 1 > 0 && random.next_bool() {
            bands[index - 1] = palette[6];
        }
        if index + 1 < bands.len() && random.next_bool() {
            bands[index + 1] = palette[6];
        }
        index += random.next_bounded_i32(16)? as usize + 4;
    }
    Ok(bands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct TopMaterialFixture {
        cases: Vec<TopMaterialCase>,
    }
    #[derive(serde::Deserialize)]
    struct TopMaterialCase {
        settings: String,
        seed: i64,
        biome: String,
        min_y: i32,
        height: i32,
        world_surface_wg: Vec<i32>,
        queries: Vec<TopMaterialQuery>,
        blocks_unchanged: bool,
    }
    #[derive(serde::Deserialize)]
    struct TopMaterialQuery {
        point: [i32; 3],
        has_fluid: bool,
        fail_biome: bool,
        material: Option<u32>,
        error: Option<String>,
        biome_queries: Vec<[i32; 3]>,
        cold_enough_to_snow: bool,
        preliminary: std::collections::BTreeMap<String, i32>,
    }
    struct ObservedTopMaterialHost<'a> {
        case: &'a TopMaterialCase,
        query: &'a TopMaterialQuery,
        biome_queries: Vec<[i32; 3]>,
        block_accesses: usize,
        writes: usize,
    }
    impl SurfaceHost for ObservedTopMaterialHost<'_> {
        fn min_y(&self) -> i32 {
            panic!("topMaterial must not start a chunk column scan")
        }
        fn max_y(&self) -> i32 {
            panic!("topMaterial must not start a chunk column scan")
        }
        fn min_gen_y(&self) -> i32 {
            self.case.min_y
        }
        fn gen_depth(&self) -> i32 {
            self.case.height
        }
        fn block(&mut self, _: i32, _: i32, _: i32) -> Result<u32> {
            self.block_accesses += 1;
            bail!("topMaterial must not inspect block state")
        }
        fn set_block(&mut self, _: i32, _: i32, _: i32, _: u32) -> Result<()> {
            self.writes += 1;
            bail!("topMaterial must not write blocks")
        }
        fn is_air(&self, _: u32) -> bool {
            panic!("unexpected block predicate")
        }
        fn has_fluid(&self, _: u32) -> bool {
            panic!("has_fluid must come from the caller")
        }
        fn is_named_block(&self, _: u32, _: &str) -> bool {
            panic!("unexpected block predicate")
        }
        fn world_surface_wg_height(&mut self, x: i32, z: i32) -> Result<i32> {
            anyhow::ensure!(
                (0..16).contains(&x) && (0..16).contains(&z),
                "unobserved heightmap position"
            );
            self.case
                .world_surface_wg
                .get((z * 16 + x) as usize)
                .copied()
                .context("unobserved heightmap position")
        }
        fn biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
            self.biome_queries.push([x, y, z]);
            if self.query.fail_biome {
                bail!("unavailable actual biome owner");
            }
            Ok(self.case.biome.clone())
        }
        fn cold_enough_to_snow(&mut self, biome: &str, x: i32, y: i32, z: i32) -> Result<bool> {
            anyhow::ensure!(
                biome == self.case.biome && [x, y, z] == self.query.point,
                "unobserved thermal position"
            );
            Ok(self.query.cold_enough_to_snow)
        }
        fn should_melt_frozen_ocean_iceberg_slightly(
            &mut self,
            _: &str,
            _: i32,
            _: i32,
            _: i32,
        ) -> Result<bool> {
            bail!("topMaterial must not run the iceberg extension")
        }
        fn preliminary_surface_level(&mut self, x: i32, z: i32) -> Result<i32> {
            self.query
                .preliminary
                .get(&format!("{x},{z}"))
                .copied()
                .context("unobserved preliminary surface owner")
        }
    }

    #[test]
    fn original_top_material_optional_water_laziness_and_error_boundaries() -> Result<()> {
        let fixture: TopMaterialFixture = serde_json::from_str(include_str!(
            "../../../fixtures/world-generation-top-material.json"
        ))?;
        for case in fixture.cases {
            assert!(
                case.blocks_unchanged,
                "original topMaterial changed its ProtoChunk"
            );
            let graph = crate::density::DensityGraph::builtin(case.seed, &case.settings)?;
            let mut randoms = RandomStateRandoms::from_graph(&graph);
            let system = SurfaceSystem::from_settings(
                &case.settings,
                &mut randoms,
                crate::worldgen::generator::resolve_block_state,
            )?;
            for query in &case.queries {
                let mut host = ObservedTopMaterialHost {
                    case: &case,
                    query,
                    biome_queries: Vec::new(),
                    block_accesses: 0,
                    writes: 0,
                };
                let [x, y, z] = query.point;
                let result =
                    system.top_material(&mut host, DensityPoint::new(x, y, z), query.has_fluid);
                if let Some(error) = &query.error {
                    assert_eq!(
                        result.unwrap_err().to_string(),
                        *error,
                        "{} at {:?}",
                        case.biome,
                        query.point
                    );
                } else {
                    assert_eq!(
                        result?, query.material,
                        "{} at {:?}, wet={}",
                        case.biome, query.point, query.has_fluid
                    );
                }
                assert_eq!(
                    host.biome_queries, query.biome_queries,
                    "original biome callback order/laziness"
                );
                assert_eq!(host.block_accesses, 0);
                assert_eq!(host.writes, 0);
            }
        }
        Ok(())
    }

    #[test]
    fn original_default_namespace_settings_aliases_share_the_packaged_owner() -> Result<()> {
        let canonical = builtin_settings("minecraft:overworld")?;
        for alias in ["overworld", ":overworld"] {
            assert!(std::ptr::eq(canonical, builtin_settings(alias)?));
        }
        assert!(builtin_settings("other:overworld").is_err());
        Ok(())
    }
}
