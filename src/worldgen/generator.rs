//! NoiseBasedChunkGenerator material composition and retained staged terrain.
//!
//! The initial supported boundary is an actually empty new ProtoChunk with
//! empty Blender and structure beardifier. Those are caller-owned facts, not
//! defaults. This does not select biomes, carve, place features or return FULL.
use super::{
    aquifer::{Aquifer, AquiferBlock, AquiferContext, AquiferNoise, FluidStatus},
    noise_chunk::NoiseChunk,
    random_state::RandomStateRandoms,
    surface::{self, SurfaceHost, SurfaceReport, SurfaceSystem},
};
use crate::{
    density::{ChunkDensity, DensityGraph, DensityPoint, DensityRouter},
    inventory::{block_state_name, block_state_with_properties},
    random::RandomSource,
};
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

/// Observations supplied by the actual generation dependency owner. This enum
/// has no default: absence of an owner is not evidence of empty input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationInfluence {
    ObservedEmpty,
    Present,
    Unavailable,
}

#[derive(Debug, Clone, Copy)]
pub struct FreshGenerationContext {
    pub initial_blocks: GenerationInfluence,
    pub blender: GenerationInfluence,
    pub structure_beardifier: GenerationInfluence,
}
impl FreshGenerationContext {
    fn validate(self) -> Result<()> {
        ensure!(self.initial_blocks == GenerationInfluence::ObservedEmpty,
            "NOISE requires an actually empty new ProtoChunk; existing/unknown blocks are unsupported");
        ensure!(
            self.blender == GenerationInfluence::ObservedEmpty,
            "nonempty or unavailable Blender context is unsupported"
        );
        ensure!(
            self.structure_beardifier == GenerationInfluence::ObservedEmpty,
            "nonempty or unavailable structure-beardifier context is unsupported"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainStage {
    Noise,
    Surface,
}

/// Actual BiomeManager/biome callbacks needed by SURFACE. The integration
/// supplies the zoom seed and real biome data; raw climate samples are not a
/// replacement for biome selection or biome temperature state.
pub trait SurfaceContext {
    fn biome(&mut self, x: i32, y: i32, z: i32) -> Result<String>;
    fn cold_enough_to_snow(&mut self, biome: &str, x: i32, y: i32, z: i32) -> Result<bool>;
    fn should_melt_frozen_ocean_iceberg_slightly(
        &mut self,
        biome: &str,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<bool>;
}

/// Per-world seeded owners. Gameplay Level RNG is deliberately not an input.
pub struct NoiseGenerator {
    graph: DensityGraph,
    router: DensityRouter,
    randoms: RandomStateRandoms,
    surface: SurfaceSystem,
    settings_name: String,
    default_block: u32,
    default_fluid: AquiferBlock,
    sea_level: i32,
    aquifers_enabled: bool,
    ore_veins_enabled: bool,
    palette: MaterialPalette,
}

impl NoiseGenerator {
    pub fn builtin(seed: i64, settings: &str) -> Result<Self> {
        let settings_name = if settings.contains(':') {
            settings.to_owned()
        } else {
            format!("minecraft:{settings}")
        };
        let graph = DensityGraph::builtin(seed, &settings_name)?;
        let router = graph.router()?;
        let mut randoms = RandomStateRandoms::from_graph(&graph);
        let settings = surface::builtin_settings(&settings_name)?;
        let default_block = resolve_block_state(&settings["default_block"])?;
        let default_fluid = match settings["default_fluid"]["Name"].as_str() {
            Some("minecraft:air") => AquiferBlock::Air,
            Some("minecraft:water") => AquiferBlock::Water,
            Some("minecraft:lava") => AquiferBlock::Lava,
            _ => bail!("unsupported built-in default fluid"),
        };
        let resolved_fluid = resolve_block_state(&settings["default_fluid"])?;
        let palette = MaterialPalette::new()?;
        ensure!(
            resolved_fluid == palette.fluid(default_fluid),
            "default fluid properties differ from canonical source state"
        );
        let sea_level = settings["sea_level"]
            .as_i64()
            .context("missing sea level")?
            .try_into()?;
        let aquifers_enabled = settings["aquifers_enabled"]
            .as_bool()
            .context("missing aquifer setting")?;
        let ore_veins_enabled = settings["ore_veins_enabled"]
            .as_bool()
            .context("missing ore setting")?;
        let surface =
            SurfaceSystem::from_settings(&settings_name, &mut randoms, resolve_block_state)?;
        Ok(Self {
            graph,
            router,
            randoms,
            surface,
            settings_name,
            default_block,
            default_fluid,
            sea_level,
            aquifers_enabled,
            ore_veins_enabled,
            palette,
        })
    }
    pub fn graph(&self) -> &DensityGraph {
        &self.graph
    }
    pub fn seed(&self) -> i64 {
        self.graph.seed()
    }
    pub fn settings_name(&self) -> &str {
        &self.settings_name
    }

    /// Fill a privately owned new terrain image in original doFill order. The
    /// returned image is only NOISE, irrespective of whether its blocks look
    /// ready to send. Clipped/custom partial noise cells stay unsupported.
    pub fn fill_noise(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        level_min_y: i32,
        level_height: i32,
        context: FreshGenerationContext,
    ) -> Result<NoiseTerrain<'_>> {
        context.validate()?;
        let level_max_y = level_min_y
            .checked_add(level_height)
            .context("level height overflows")?;
        ensure!(
            level_height > 0 && level_min_y >= -2032 && level_max_y <= 2032,
            "level height exceeds original dimension coordinate range"
        );
        let origin_x = chunk_x.checked_mul(16).context("chunk X overflows")?;
        let origin_z = chunk_z.checked_mul(16).context("chunk Z overflows")?;
        ensure!(
            (-33_554_384..=33_554_383).contains(&origin_x)
                && (-33_554_384..=33_554_383).contains(&origin_z),
            "chunk lies outside representable generation BlockPos range"
        );
        let mut noise = NoiseChunk::new(&self.graph, chunk_x, chunk_z, level_min_y, level_height)?;
        let grid = noise.grid();
        let mut image = TerrainImage::new(level_min_y, level_height, self.palette.air)?;
        let mut aquifer = if self.aquifers_enabled {
            Aquifer::noise_based(
                chunk_x,
                chunk_z,
                grid.min_y,
                grid.height,
                self.randoms.aquifer_random(),
            )?
        } else {
            Aquifer::Disabled
        };
        let block_density = self
            .graph
            .block_density()
            .context("missing wrapped block density")?;
        noise.visit_blocks(|graph, density, point| {
            let value = density.sample(graph, block_density)?;
            let mut context = MaterialContext {
                graph,
                density,
                router: self.router,
                sea_level: self.sea_level,
                default_fluid: self.default_fluid,
            };
            let substance = aquifer.compute_substance(point, value, &mut context)?;
            let state = match substance {
                Some(fluid) => self.palette.fluid(fluid),
                None if self.ore_veins_enabled => self
                    .ore_state(context.density, point)?
                    .unwrap_or(self.default_block),
                None => self.default_block,
            };
            if state != self.palette.air {
                image.set(point.x - origin_x, point.y, point.z - origin_z, state)?;
                if aquifer.should_schedule_fluid_update() && facts().has_fluid(state) {
                    image.postprocessing.push(point);
                }
            }
            Ok(())
        })?;
        Ok(NoiseTerrain {
            noise,
            graph: &self.graph,
            chunk_x,
            chunk_z,
            level_min_y,
            level_height,
            image,
            stage: TerrainStage::Noise,
        })
    }

    /// Stage surface replacements privately. An unavailable biome query leaves
    /// the NOISE block image intact; retained preliminary-cache reads are safe
    /// to reuse because they belong to this same graph and chunk.
    pub fn build_surface(
        &self,
        terrain: &mut NoiseTerrain<'_>,
        context: &mut impl SurfaceContext,
    ) -> Result<SurfaceReport> {
        ensure!(
            std::ptr::eq(terrain.graph, &self.graph),
            "terrain belongs to a different seeded generator"
        );
        ensure!(
            terrain.stage == TerrainStage::Noise,
            "surface stage already completed"
        );
        let mut staged = terrain.image.clone();
        let grid = self.graph.grid().context("missing noise settings")?;
        let mut host = TerrainSurfaceHost {
            image: &mut staged,
            density: terrain.noise.density_mut(),
            graph: &self.graph,
            context,
            chunk_x: terrain.chunk_x,
            chunk_z: terrain.chunk_z,
            min_gen_y: terrain.level_min_y.max(grid.min_y),
            gen_depth: terrain.level_height.min(grid.height),
            void_air: self.palette.void_air,
        };
        // NoiseBasedChunkGenerator.buildSurface supplies false. This is not the
        // old blending-height biome path, which requires its own dependencies.
        let report =
            self.surface
                .build_surface(&mut host, terrain.chunk_x, terrain.chunk_z, false)?;
        terrain.image = staged;
        terrain.stage = TerrainStage::Surface;
        Ok(report)
    }

    fn ore_state(&self, density: &mut ChunkDensity, p: DensityPoint) -> Result<Option<u32>> {
        let toggle = density.sample(&self.graph, self.router.vein_toggle)?;
        let (min_y, max_y, ore, raw, filler) = if toggle > 0.0 {
            (
                0,
                50,
                self.palette.copper_ore,
                self.palette.raw_copper,
                self.palette.granite,
            )
        } else {
            (
                -60,
                -8,
                self.palette.iron_ore,
                self.palette.raw_iron,
                self.palette.tuff,
            )
        };
        let top = max_y - p.y;
        let bottom = p.y - min_y;
        if top < 0 || bottom < 0 {
            return Ok(None);
        }
        let roundoff = clamped_map(f64::from(top.min(bottom)), 0.0, 20.0, -0.2, 0.0);
        let veininess = toggle.abs();
        if veininess + roundoff < f64::from(0.4_f32) {
            return Ok(None);
        }
        let mut random = self.randoms.ore_random().at(p.x, p.y, p.z);
        if random.next_f32() > 0.7_f32 {
            return Ok(None);
        }
        if density.sample(&self.graph, self.router.vein_ridged)? >= 0.0 {
            return Ok(None);
        }
        let richness = clamped_map(
            veininess,
            f64::from(0.4_f32),
            f64::from(0.6_f32),
            f64::from(0.1_f32),
            f64::from(0.3_f32),
        );
        if f64::from(random.next_f32()) < richness
            && density.sample(&self.graph, self.router.vein_gap)? > f64::from(-0.3_f32)
        {
            Ok(Some(if random.next_f32() < 0.02_f32 {
                raw
            } else {
                ore
            }))
        } else {
            Ok(Some(filler))
        }
    }
}

/// An owned intermediate image, retaining exactly the NoiseChunk sampling
/// caches used for its NOISE phase. No status/readiness conversion is implied.
pub struct NoiseTerrain<'g> {
    noise: NoiseChunk<'g>,
    graph: &'g DensityGraph,
    chunk_x: i32,
    chunk_z: i32,
    level_min_y: i32,
    level_height: i32,
    image: TerrainImage,
    stage: TerrainStage,
}
impl NoiseTerrain<'_> {
    pub fn stage(&self) -> TerrainStage {
        self.stage
    }
    pub fn chunk(&self) -> (i32, i32) {
        (self.chunk_x, self.chunk_z)
    }
    pub fn min_y(&self) -> i32 {
        self.level_min_y
    }
    pub fn height(&self) -> i32 {
        self.level_height
    }
    /// Canonical states in ascending Y, then local Z, then local X order.
    pub fn states(&self) -> &[u32] {
        &self.image.states
    }
    pub fn block(&self, local_x: i32, y: i32, local_z: i32) -> Result<u32> {
        self.image.block(local_x, y, local_z)
    }
    pub fn world_surface_wg(&self) -> &[i32; 256] {
        &self.image.world_surface
    }
    pub fn ocean_floor_wg(&self) -> &[i32; 256] {
        &self.image.ocean_floor
    }
    /// Original markPosForPostprocessing order, including repeated positions.
    pub fn postprocessing(&self) -> &[DensityPoint] {
        &self.image.postprocessing
    }
}

struct MaterialContext<'a> {
    graph: &'a DensityGraph,
    density: &'a mut ChunkDensity,
    router: DensityRouter,
    sea_level: i32,
    default_fluid: AquiferBlock,
}
impl AquiferContext for MaterialContext<'_> {
    fn global_fluid(&mut self, p: DensityPoint) -> Result<FluidStatus> {
        Ok(if p.y < (-54).min(self.sea_level) {
            FluidStatus::new(-54, AquiferBlock::Lava)
        } else {
            FluidStatus::new(self.sea_level, self.default_fluid)
        })
    }
    fn preliminary_surface_level(&mut self, x: i32, z: i32) -> Result<i32> {
        Ok(self.density.preliminary_surface_level(self.graph, x, z)?)
    }
    fn sample_at(&mut self, noise: AquiferNoise, p: DensityPoint) -> Result<f64> {
        let id = match noise {
            AquiferNoise::Floodedness => self.router.fluid_level_floodedness,
            AquiferNoise::Spread => self.router.fluid_level_spread,
            AquiferNoise::Lava => self.router.lava,
            AquiferNoise::Erosion => self.router.erosion,
            AquiferNoise::Depth => self.router.depth,
        };
        Ok(self.density.sample_external(self.graph, id, p)?)
    }
    fn sample_barrier(&mut self, p: DensityPoint) -> Result<f64> {
        ensure!(
            self.density.point() == p,
            "aquifer pressure sampled outside current NoiseChunk position"
        );
        Ok(self.density.sample(self.graph, self.router.barrier)?)
    }
}

#[derive(Clone)]
struct TerrainImage {
    min_y: i32,
    height: i32,
    states: Vec<u32>,
    /// getFirstAvailable values, as used by Heightmap's packed storage.
    world_surface: [i32; 256],
    ocean_floor: [i32; 256],
    postprocessing: Vec<DensityPoint>,
}
impl TerrainImage {
    fn new(min_y: i32, height: i32, air: u32) -> Result<Self> {
        let length = usize::try_from(height)?
            .checked_mul(256)
            .context("terrain allocation overflow")?;
        Ok(Self {
            min_y,
            height,
            states: vec![air; length],
            world_surface: [min_y; 256],
            ocean_floor: [min_y; 256],
            postprocessing: Vec::new(),
        })
    }
    fn index(&self, x: i32, y: i32, z: i32) -> Result<usize> {
        ensure!(
            (0..16).contains(&x)
                && (0..16).contains(&z)
                && (self.min_y..self.min_y + self.height).contains(&y),
            "terrain coordinate outside owned chunk"
        );
        Ok(((y - self.min_y) as usize) * 256 + (z as usize) * 16 + x as usize)
    }
    fn block(&self, x: i32, y: i32, z: i32) -> Result<u32> {
        Ok(self.states[self.index(x, y, z)?])
    }
    fn set(&mut self, x: i32, y: i32, z: i32, state: u32) -> Result<()> {
        let index = self.index(x, y, z)?;
        ensure!(
            (state as usize) < facts().solid_flags.len(),
            "unknown canonical block state"
        );
        self.states[index] = state;
        let column = (z * 16 + x) as usize;
        for ocean in [false, true] {
            let heights = if ocean {
                &mut self.ocean_floor
            } else {
                &mut self.world_surface
            };
            let matches = if ocean {
                facts().blocks_motion(state)
            } else {
                !facts().is_air(state)
            };
            if matches {
                if y + 1 > heights[column] {
                    heights[column] = y + 1;
                }
            } else if heights[column] == y + 1 {
                let mut next = y - 1;
                while next >= self.min_y {
                    let state = self.states[((next - self.min_y) as usize) * 256 + column];
                    if if ocean {
                        facts().blocks_motion(state)
                    } else {
                        !facts().is_air(state)
                    } {
                        break;
                    }
                    next -= 1;
                }
                heights[column] = next + 1;
            }
        }
        Ok(())
    }
}

struct TerrainSurfaceHost<'a, 'c, C> {
    image: &'a mut TerrainImage,
    density: &'a mut ChunkDensity,
    graph: &'a DensityGraph,
    context: &'c mut C,
    chunk_x: i32,
    chunk_z: i32,
    min_gen_y: i32,
    gen_depth: i32,
    void_air: u32,
}
impl<C: SurfaceContext> SurfaceHost for TerrainSurfaceHost<'_, '_, C> {
    fn min_y(&self) -> i32 {
        self.image.min_y
    }
    fn max_y(&self) -> i32 {
        self.image.min_y + self.image.height
    }
    fn min_gen_y(&self) -> i32 {
        self.min_gen_y
    }
    fn gen_depth(&self) -> i32 {
        self.gen_depth
    }
    fn block(&mut self, x: i32, y: i32, z: i32) -> Result<u32> {
        if y < self.min_y() || y >= self.max_y() {
            return Ok(self.void_air);
        }
        self.image
            .block(x - self.chunk_x * 16, y, z - self.chunk_z * 16)
    }
    fn set_block(&mut self, x: i32, y: i32, z: i32, state: u32) -> Result<()> {
        // ChunkAccess.setBlockState ignores writes outside its height accessor.
        if y < self.min_y() || y >= self.max_y() {
            return Ok(());
        }
        self.image
            .set(x - self.chunk_x * 16, y, z - self.chunk_z * 16, state)?;
        if facts().has_fluid(state) {
            self.image.postprocessing.push(DensityPoint::new(x, y, z));
        }
        Ok(())
    }
    fn is_air(&self, state: u32) -> bool {
        facts().is_air(state)
    }
    fn has_fluid(&self, state: u32) -> bool {
        facts().has_fluid(state)
    }
    fn is_named_block(&self, state: u32, name: &str) -> bool {
        block_state_name(state)
            .is_some_and(|actual| actual == name.strip_prefix("minecraft:").unwrap_or(name))
    }
    fn world_surface_wg_height(&mut self, x: i32, z: i32) -> Result<i32> {
        ensure!(
            (0..16).contains(&x) && (0..16).contains(&z),
            "surface height outside owned chunk"
        );
        Ok(self.image.world_surface[(z * 16 + x) as usize] - 1)
    }
    fn biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        self.context.biome(x, y, z)
    }
    fn cold_enough_to_snow(&mut self, b: &str, x: i32, y: i32, z: i32) -> Result<bool> {
        self.context.cold_enough_to_snow(b, x, y, z)
    }
    fn should_melt_frozen_ocean_iceberg_slightly(
        &mut self,
        b: &str,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<bool> {
        self.context
            .should_melt_frozen_ocean_iceberg_slightly(b, x, y, z)
    }
    fn preliminary_surface_level(&mut self, x: i32, z: i32) -> Result<i32> {
        Ok(self.density.preliminary_surface_level(self.graph, x, z)?)
    }
}

#[derive(Deserialize)]
struct BlockCatalog {
    blocks: BTreeMap<String, BlockEntry>,
}
#[derive(Deserialize)]
struct BlockEntry {
    default_state: u32,
}
/// Resolve source JSON through the existing exact-version canonical registry.
/// Arbitrary namespaces and unknown property values are explicit errors.
pub fn resolve_block_state(value: &Value) -> Result<u32> {
    static BLOCKS: OnceLock<BlockCatalog> = OnceLock::new();
    let blocks = BLOCKS.get_or_init(|| {
        serde_json::from_str(include_str!("../../fixtures/items-1.21.json"))
            .expect("canonical block registry")
    });
    let name = value["Name"].as_str().context("block state lacks Name")?;
    let key = name
        .strip_prefix("minecraft:")
        .context("non-Minecraft block namespace is unsupported")?;
    let entry = blocks
        .blocks
        .get(key)
        .with_context(|| format!("unknown block state {name}"))?;
    let Some(properties) = value.get("Properties") else {
        return Ok(entry.default_state);
    };
    let properties = properties
        .as_object()
        .context("block properties must be an object")?
        .iter()
        .map(|(k, v)| {
            Ok((
                k.clone(),
                v.as_str()
                    .context("block property must be a string")?
                    .to_owned(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    block_state_with_properties(entry.default_state, &properties)
}
#[derive(Deserialize)]
struct BlockFacts {
    solid_flags: Vec<u8>,
    fluids: Vec<u8>,
}
impl BlockFacts {
    fn is_air(&self, s: u32) -> bool {
        self.solid_flags[s as usize] & 8 != 0
    }
    fn blocks_motion(&self, s: u32) -> bool {
        self.solid_flags[s as usize] & 2 != 0
    }
    fn has_fluid(&self, s: u32) -> bool {
        self.fluids[s as usize] & 8 != 0
    }
}
fn facts() -> &'static BlockFacts {
    static DATA: OnceLock<BlockFacts> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../fixtures/placement-1.21.json"))
            .expect("canonical block facts")
    })
}
struct MaterialPalette {
    air: u32,
    void_air: u32,
    water: u32,
    lava: u32,
    copper_ore: u32,
    raw_copper: u32,
    granite: u32,
    iron_ore: u32,
    raw_iron: u32,
    tuff: u32,
}
impl MaterialPalette {
    fn new() -> Result<Self> {
        let state = |name: &str| {
            resolve_block_state(&serde_json::json!({"Name":format!("minecraft:{name}")}))
        };
        Ok(Self {
            air: state("air")?,
            void_air: state("void_air")?,
            water: state("water")?,
            lava: state("lava")?,
            copper_ore: state("copper_ore")?,
            raw_copper: state("raw_copper_block")?,
            granite: state("granite")?,
            iron_ore: state("deepslate_iron_ore")?,
            raw_iron: state("raw_iron_block")?,
            tuff: state("tuff")?,
        })
    }
    fn fluid(&self, fluid: AquiferBlock) -> u32 {
        match fluid {
            AquiferBlock::Air => self.air,
            AquiferBlock::Water => self.water,
            AquiferBlock::Lava => self.lava,
        }
    }
}
fn clamped_map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let delta = (value - from_min) / (from_max - from_min);
    if delta < 0.0 {
        to_min
    } else if delta > 1.0 {
        to_max
    } else {
        to_min + delta * (to_max - to_min)
    }
}
