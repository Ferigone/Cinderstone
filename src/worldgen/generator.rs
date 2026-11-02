//! NoiseBasedChunkGenerator material composition and retained staged terrain.
//!
//! The initial supported boundary is an actually empty new ProtoChunk with
//! empty Blender and structure beardifier. Those are caller-owned facts, not
//! defaults. Paired staged adapters retain actual BIOMES and AIR carver data;
//! feature placement, scheduling and FULL publication remain external.
use super::{
    aquifer::{Aquifer, AquiferBlock, AquiferContext, AquiferNoise, FluidStatus},
    biome_temperature::BiomeTemperature,
    carver::{
        self, AirCarvingContext, CarverHost, CarvingMask, CarvingReport, CarvingTargetHost,
        RawBiomeManager,
    },
    noise_chunk::{NoiseChunk, RetainedNoiseChunk},
    random_state::RandomStateRandoms,
    surface::{self, SurfaceHost, SurfaceReport, SurfaceSystem},
};
use crate::{
    density::{
        quantize_climate_coord, ChunkDensity, ClimateTargetPoint, DensityGraph, DensityPoint,
        DensityRouter,
    },
    inventory::{block_state_name, block_state_with_properties},
    random::RandomSource,
};
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

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
    /// Native AIR-carver data only, not original ChunkStatus/readiness.
    Carvers,
}

/// The result of the source fillFromNoise entry point. A no-cell result means
/// the caller retains its actual input ProtoChunk unchanged. It contains no
/// replacement terrain, no NoiseChunk, and no completed runtime status.
pub enum NoiseFillOutcome<'g> {
    Generated(NoiseTerrain<'g>),
    NoCells {
        /// NoiseSettings.clampToHeightAccessor minimum Y.
        min_y: i32,
        /// Original clamped height, which may be negative for disjoint ranges.
        height: i32,
        /// floorDiv(clamped height, cell height), preserving negative results.
        cell_count_y: i32,
    },
}

/// Owned counterpart of `NoiseFillOutcome`. A skipped fill retains no
/// generator owner or terrain image; the actual input chunk stays with its
/// caller. The generated variant can outlive the caller's local Arc binding.
pub enum OwnedNoiseFillOutcome {
    Generated(OwnedNoiseTerrain),
    NoCells {
        min_y: i32,
        height: i32,
        cell_count_y: i32,
    },
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
        // ResourceLocation.parse uses the default namespace for both a
        // missing namespace and an explicitly empty one, such as :overworld.
        let settings_name = match settings.split_once(':') {
            Some(("", path)) => format!("minecraft:{path}"),
            Some(_) => settings.to_owned(),
            None => format!("minecraft:{settings}"),
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

    /// Prepare the actual empty-context density and aquifer owners before
    /// climate sampling. This strict API requires positive, aligned supported
    /// cells; it does not create a block image or assert a BIOMES status.
    pub fn prepare_chunk(
        self: &Arc<Self>,
        chunk_x: i32,
        chunk_z: i32,
        level_min_y: i32,
        level_height: i32,
        context: FreshGenerationContext,
    ) -> Result<PreparedGeneration> {
        let (noise, aquifer) =
            self.begin_noise(chunk_x, chunk_z, level_min_y, level_height, context)?;
        Ok(PreparedGeneration {
            generator: Arc::clone(self),
            noise: noise.into_retained(),
            aquifer,
            chunk_x,
            chunk_z,
            level_min_y,
            level_height,
        })
    }

    /// Mirror fillFromNoise's early no-cell return without inventing a terrain
    /// image. Clamping and the floor division happen before querying Blender,
    /// structures or block contents. A skipped result leaves the caller's
    /// actual input ProtoChunk untouched, regardless of unavailable dependency
    /// observations; a positive cell count delegates to the existing fill API.
    pub fn fill_from_noise(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        level_min_y: i32,
        level_height: i32,
        context: FreshGenerationContext,
    ) -> Result<NoiseFillOutcome<'_>> {
        let level_max_y = validate_level_height(level_min_y, level_height)?;
        let settings = self.graph.grid().context("missing noise settings")?;
        let min_y = settings.min_y.max(level_min_y);
        let height = (settings.min_y + settings.height).min(level_max_y) - min_y;
        let cell_count_y = height.div_euclid(settings.cell_height);
        if cell_count_y <= 0 {
            return Ok(NoiseFillOutcome::NoCells {
                min_y,
                height,
                cell_count_y,
            });
        }
        self.fill_noise(chunk_x, chunk_z, level_min_y, level_height, context)
            .map(NoiseFillOutcome::Generated)
    }

    /// Own a fill result without rebuilding any random or density-cache owner.
    /// `NoCells` keeps the source early-return behavior and retains no Arc.
    pub fn fill_from_noise_owned(
        self: &Arc<Self>,
        chunk_x: i32,
        chunk_z: i32,
        level_min_y: i32,
        level_height: i32,
        context: FreshGenerationContext,
    ) -> Result<OwnedNoiseFillOutcome> {
        match self.fill_from_noise(chunk_x, chunk_z, level_min_y, level_height, context)? {
            NoiseFillOutcome::Generated(terrain) => self
                .retain_terrain(terrain)
                .map(OwnedNoiseFillOutcome::Generated),
            NoiseFillOutcome::NoCells {
                min_y,
                height,
                cell_count_y,
            } => Ok(OwnedNoiseFillOutcome::NoCells {
                min_y,
                height,
                cell_count_y,
            }),
        }
    }

    /// Retain this exact generator with an already generated stage. Equal seed
    /// and settings on another generator are insufficient: the node IDs and
    /// mutable caches belong to the graph that produced the borrowed terrain.
    /// The existing density owner and block image are moved, never cloned,
    /// reseeded or wrapped again. No reference points inside the returned value.
    pub fn retain_terrain(
        self: &Arc<Self>,
        terrain: NoiseTerrain<'_>,
    ) -> Result<OwnedNoiseTerrain> {
        ensure!(
            std::ptr::eq(terrain.graph, &self.graph),
            "terrain belongs to a different seeded generator"
        );
        let NoiseTerrain {
            noise,
            aquifer,
            graph: _,
            chunk_x,
            chunk_z,
            level_min_y,
            level_height,
            image,
            stage,
        } = terrain;
        Ok(OwnedNoiseTerrain {
            generator: Arc::clone(self),
            density: noise.into_density(),
            _aquifer: aquifer,
            chunk_x,
            chunk_z,
            level_min_y,
            level_height,
            image,
            stage,
        })
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
        let (mut noise, mut aquifer) =
            self.begin_noise(chunk_x, chunk_z, level_min_y, level_height, context)?;
        let image = self.fill_noise_image(
            &mut noise,
            aquifer.as_mut(),
            (chunk_x, chunk_z),
            (level_min_y, level_height),
        )?;
        Ok(NoiseTerrain {
            noise,
            aquifer,
            graph: &self.graph,
            chunk_x,
            chunk_z,
            level_min_y,
            level_height,
            image,
            stage: TerrainStage::Noise,
        })
    }

    fn begin_noise(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        level_min_y: i32,
        level_height: i32,
        context: FreshGenerationContext,
    ) -> Result<(NoiseChunk<'_>, Box<Aquifer>)> {
        context.validate()?;
        validate_level_height(level_min_y, level_height)?;
        let origin_x = chunk_x.checked_mul(16).context("chunk X overflows")?;
        let origin_z = chunk_z.checked_mul(16).context("chunk Z overflows")?;
        ensure!(
            (-33_554_384..=33_554_383).contains(&origin_x)
                && (-33_554_384..=33_554_383).contains(&origin_z),
            "chunk lies outside representable generation BlockPos range"
        );
        let noise = NoiseChunk::new(&self.graph, chunk_x, chunk_z, level_min_y, level_height)?;
        let grid = noise.grid();
        let aquifer = if self.aquifers_enabled {
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
        Ok((noise, Box::new(aquifer)))
    }

    // One material traversal for the borrowed fill and the prepared owned fill.
    // The caller supplies the already created density and aquifer owners.
    fn fill_noise_image(
        &self,
        noise: &mut NoiseChunk<'_>,
        aquifer: &mut Aquifer,
        chunk: (i32, i32),
        level: (i32, i32),
    ) -> Result<TerrainImage> {
        let origin_x = chunk.0.checked_mul(16).context("chunk X overflows")?;
        let origin_z = chunk.1.checked_mul(16).context("chunk Z overflows")?;
        let mut image = TerrainImage::new(level.0, level.1, self.palette.air)?;
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
        Ok(image)
    }

    // Climate.Sampler.sample: wrapped roots, SinglePointContext, double->float,
    // then float multiplication and narrowing. No biome selection or spawn
    // search is implied by the six climate coordinates.
    fn sample_climate_from(
        &self,
        density: &mut ChunkDensity,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
    ) -> Result<ClimateTargetPoint> {
        let point = DensityPoint::new(
            quart_x.wrapping_shl(2),
            quart_y.wrapping_shl(2),
            quart_z.wrapping_shl(2),
        );
        let mut values = [0; 6];
        for (index, root) in [
            self.router.temperature,
            self.router.vegetation,
            self.router.continents,
            self.router.erosion,
            self.router.depth,
            self.router.ridges,
        ]
        .into_iter()
        .enumerate()
        {
            values[index] =
                quantize_climate_coord(density.sample_external(&self.graph, root, point)? as f32);
        }
        Ok(ClimateTargetPoint {
            temperature: values[0],
            humidity: values[1],
            continentalness: values[2],
            erosion: values[3],
            depth: values[4],
            weirdness: values[5],
        })
    }

    // The End source compares the erosion function's raw double at an already
    // converted block position. This is not the quantized climate coordinate.
    fn sample_erosion_from(&self, density: &mut ChunkDensity, point: DensityPoint) -> Result<f64> {
        Ok(density.sample_external(&self.graph, self.router.erosion, point)?)
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
        self.apply_surface(
            terrain.noise.density_mut(),
            &mut terrain.image,
            &mut terrain.stage,
            (terrain.chunk_x, terrain.chunk_z),
            context,
        )
    }

    // Both borrowed and owned stage APIs use the same source application and
    // transaction. Ownership conversion does not create another SurfaceSystem.
    fn apply_surface(
        &self,
        density: &mut ChunkDensity,
        image: &mut TerrainImage,
        stage: &mut TerrainStage,
        chunk: (i32, i32),
        context: &mut impl SurfaceContext,
    ) -> Result<SurfaceReport> {
        ensure!(
            *stage == TerrainStage::Noise,
            "surface stage already completed"
        );
        let mut staged = image.clone();
        let grid = self.graph.grid().context("missing noise settings")?;
        let mut host = TerrainSurfaceHost {
            min_gen_y: image.min_y.max(grid.min_y),
            gen_depth: image.height.min(grid.height),
            image: &mut staged,
            density,
            graph: &self.graph,
            context,
            chunk_x: chunk.0,
            chunk_z: chunk.1,
            void_air: self.palette.void_air,
        };
        // NoiseBasedChunkGenerator.buildSurface forwards the settings'
        // useLegacyRandomSource flag. It selects Y=0 for each column's initial
        // biome lookup; the later rule-context lookups retain their actual Y.
        let report = self.surface.build_surface(
            &mut host,
            chunk.0,
            chunk.1,
            self.graph.uses_legacy_random_source(),
        )?;
        *image = staged;
        *stage = TerrainStage::Surface;
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
    aquifer: Box<Aquifer>,
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

/// An Arc-bound prepared density/aquifer owner, before the first NOISE fill.
/// Sampling climate uses its existing wrapped roots and caches. It owns no block
/// image and represents no completed BIOMES status. Consuming `fill_noise`
/// continues this exact owner's one-shot traversal and produces owned NOISE.
pub struct PreparedGeneration {
    generator: Arc<NoiseGenerator>,
    noise: RetainedNoiseChunk,
    aquifer: Box<Aquifer>,
    chunk_x: i32,
    chunk_z: i32,
    level_min_y: i32,
    level_height: i32,
}
impl PreparedGeneration {
    pub fn chunk(&self) -> (i32, i32) {
        (self.chunk_x, self.chunk_z)
    }
    pub fn min_y(&self) -> i32 {
        self.level_min_y
    }
    pub fn height(&self) -> i32 {
        self.level_height
    }

    /// Accept quart coordinates with the original wrapping shift to blocks,
    /// wrapped single-point evaluation and float32 climate quantization.
    pub fn sample_climate(
        &mut self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
    ) -> Result<ClimateTargetPoint> {
        let density = self.noise.density_mut_for(self.generator.graph())?;
        self.generator
            .sample_climate_from(density, quart_x, quart_y, quart_z)
    }

    /// Sample retained wrapped erosion at block coordinates without climate
    /// float narrowing or quantization, as required by End source thresholds.
    pub fn sample_erosion(&mut self, point: DensityPoint) -> Result<f64> {
        let density = self.noise.density_mut_for(self.generator.graph())?;
        self.generator.sample_erosion_from(density, point)
    }

    /// Consume the prepared owner and run its only material traversal. No
    /// density or aquifer cache is rebuilt, cloned or rebound to another graph.
    pub fn fill_noise(self) -> Result<OwnedNoiseTerrain> {
        let Self {
            generator,
            noise,
            mut aquifer,
            chunk_x,
            chunk_z,
            level_min_y,
            level_height,
        } = self;
        let mut noise = NoiseChunk::restore(generator.graph(), noise)?;
        let image = generator.fill_noise_image(
            &mut noise,
            aquifer.as_mut(),
            (chunk_x, chunk_z),
            (level_min_y, level_height),
        )?;
        let density = noise.into_density();
        Ok(OwnedNoiseTerrain {
            generator,
            density,
            _aquifer: aquifer,
            chunk_x,
            chunk_z,
            level_min_y,
            level_height,
            image,
            stage: TerrainStage::Noise,
        })
    }
}

/// A movable NOISE or SURFACE stage with its exact immutable generator owner.
/// The original `ChunkDensity` is retained after interpolation stops, including
/// canonical cache owners and the preliminary-surface map. There is no borrowed
/// field or self-reference. This is not a BIOMES owner, scheduler or FULL chunk.
pub struct OwnedNoiseTerrain {
    generator: Arc<NoiseGenerator>,
    density: ChunkDensity,
    // The original per-chunk aquifer remains alive with its caches even though
    // SURFACE does not query it; the AIR carver adapter reuses this exact owner.
    _aquifer: Box<Aquifer>,
    chunk_x: i32,
    chunk_z: i32,
    level_min_y: i32,
    level_height: i32,
    image: TerrainImage,
    stage: TerrainStage,
}
impl OwnedNoiseTerrain {
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
    /// Absolute first-available heights, indexed by local Z * 16 + local X.
    pub fn world_surface_wg(&self) -> &[i32; 256] {
        &self.image.world_surface
    }
    pub fn ocean_floor_wg(&self) -> &[i32; 256] {
        &self.image.ocean_floor
    }
    pub fn postprocessing(&self) -> &[DensityPoint] {
        &self.image.postprocessing
    }

    /// Sample the same retained wrapped climate functions after NOISE. This is
    /// not biome lookup/storage, spawn search or a generation-status transition.
    pub fn sample_climate(
        &mut self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
    ) -> Result<ClimateTargetPoint> {
        self.generator
            .sample_climate_from(&mut self.density, quart_x, quart_y, quart_z)
    }

    /// Sample the same retained erosion function at block coordinates and
    /// preserve its raw double, without climate float narrowing/quantization.
    pub fn sample_erosion(&mut self, point: DensityPoint) -> Result<f64> {
        self.generator.sample_erosion_from(&mut self.density, point)
    }

    /// Consuming AIR invocation used only by the paired BIOMES adapter. The
    /// live image, mask and caches are mutated in source order, not privately
    /// cloned/rolled back. An error consumes the failed job; no completed image
    /// or retryable SURFACE owner is returned.
    pub(super) fn apply_air_carvers(
        mut self,
        context: AirCarvingContext<'_>,
    ) -> Result<(Self, CarvingMask, CarvingReport)> {
        ensure!(
            self.stage == TerrainStage::Surface,
            "AIR carvers require completed native SURFACE data"
        );
        let generator = Arc::clone(&self.generator);
        let AirCarvingContext {
            world_seed,
            source,
            zoom,
            temperature,
            neighbors,
            registry,
            facts,
            mut mask,
        } = context;
        ensure!(
            world_seed == generator.seed(),
            "carver world seed differs from the retained generation owner"
        );
        let chunk = (self.chunk_x, self.chunk_z);
        mask.install_before_invocation(chunk, self.level_min_y, self.level_height)?;
        // Original withDifferentSource view precedes RandomSupport's real
        // process-global uniquifier/nanoTime invocation, even for empty lists.
        let mut biomes = RawBiomeManager::new(generator.graph(), source, zoom);
        let mut random = crate::random::WorldgenRandom::new(crate::random_seed::create_random()?);
        let accessor = facts.generation_height(chunk)?;
        let upgrading = facts.is_upgrading(chunk)?;
        ensure!(
            accessor == (self.level_min_y, self.level_height),
            "actual CARVERS accessor differs from retained chunk build bounds"
        );
        ensure!(
            !upgrading,
            "upgrading/retrogen CARVERS context is unsupported"
        );
        let grid = generator
            .graph
            .grid()
            .context("missing actual noise settings")?;
        let min_gen_y = accessor.0.max(grid.min_y);
        let gen_depth = accessor.1.min(grid.height);
        let mut mask = mask.acquire_after_rng(self.level_min_y, self.level_height)?;
        let mut report = {
            let mut host = TerrainCarverHost {
                generator: &generator,
                terrain: &mut self,
                biomes: &mut biomes,
                temperature,
                min_gen_y,
                gen_depth,
                upgrading,
            };
            carver::run_air(
                &mut host,
                world_seed,
                neighbors,
                registry,
                &mut random,
                &mut mask,
            )?
        };
        report.biome_queries = biomes.into_queries();
        self.stage = TerrainStage::Carvers;
        Ok((self, mask, report))
    }

    /// Apply SURFACE using the captured generator and existing density owner.
    /// Failed dependencies leave block image, heights, posts and stage intact;
    /// successful application advances only to `TerrainStage::Surface`.
    pub fn build_surface(&mut self, context: &mut impl SurfaceContext) -> Result<SurfaceReport> {
        // Cloning an Arc retains identity without borrowing a field of `self`
        // across the mutable stage operation; no RNG or cache is reconstructed.
        let generator = Arc::clone(&self.generator);
        generator.apply_surface(
            &mut self.density,
            &mut self.image,
            &mut self.stage,
            (self.chunk_x, self.chunk_z),
            context,
        )
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

/// CARVERS density queries always use the source SinglePointContext, including
/// the pressure barrier; the NOISE current-owner barrier path remains separate.
struct CarverAquiferContext<'a>(MaterialContext<'a>);
impl AquiferContext for CarverAquiferContext<'_> {
    fn global_fluid(&mut self, p: DensityPoint) -> Result<FluidStatus> {
        self.0.global_fluid(p)
    }
    fn preliminary_surface_level(&mut self, x: i32, z: i32) -> Result<i32> {
        self.0.preliminary_surface_level(x, z)
    }
    fn sample_at(&mut self, noise: AquiferNoise, p: DensityPoint) -> Result<f64> {
        self.0.sample_at(noise, p)
    }
    fn sample_barrier(&mut self, p: DensityPoint) -> Result<f64> {
        Ok(self
            .0
            .density
            .sample_external(self.0.graph, self.0.router.barrier, p)?)
    }
}
struct RawCarverSurfaceContext<'a, 'g, 'w> {
    biomes: &'a mut RawBiomeManager<'g, 'w>,
    temperature: &'a mut BiomeTemperature,
}
impl SurfaceContext for RawCarverSurfaceContext<'_, '_, '_> {
    fn biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        let biome = self.biomes.biome(DensityPoint::new(x, y, z))?;
        Ok(self.temperature.canonical_biome(&biome)?.to_owned())
    }
    fn cold_enough_to_snow(&mut self, biome: &str, x: i32, y: i32, z: i32) -> Result<bool> {
        self.temperature
            .cold_enough_to_snow(biome, DensityPoint::new(x, y, z))
    }
    fn should_melt_frozen_ocean_iceberg_slightly(
        &mut self,
        biome: &str,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<bool> {
        self.temperature
            .should_melt_frozen_ocean_iceberg_slightly(biome, DensityPoint::new(x, y, z))
    }
}
struct TerrainCarverHost<'a, 'g, 'w> {
    generator: &'g NoiseGenerator,
    terrain: &'a mut OwnedNoiseTerrain,
    biomes: &'a mut RawBiomeManager<'g, 'w>,
    temperature: &'a mut BiomeTemperature,
    min_gen_y: i32,
    gen_depth: i32,
    upgrading: bool,
}
impl CarvingTargetHost for TerrainCarverHost<'_, '_, '_> {
    fn neighbor_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        self.biomes.neighbor_biome(x, y, z)
    }
}
impl CarverHost for TerrainCarverHost<'_, '_, '_> {
    fn chunk_pos(&self) -> (i32, i32) {
        (self.terrain.chunk_x, self.terrain.chunk_z)
    }
    fn min_gen_y(&self) -> i32 {
        self.min_gen_y
    }
    fn gen_depth(&self) -> i32 {
        self.gen_depth
    }
    fn is_upgrading(&self) -> bool {
        self.upgrading
    }
    fn block(&mut self, p: DensityPoint) -> Result<u32> {
        if p.y < self.terrain.level_min_y
            || p.y >= self.terrain.level_min_y + self.terrain.level_height
        {
            return Ok(self.generator.palette.void_air);
        }
        self.terrain.image.block(
            p.x - self.terrain.chunk_x * 16,
            p.y,
            p.z - self.terrain.chunk_z * 16,
        )
    }
    fn set_block(&mut self, p: DensityPoint, state: u32) -> Result<()> {
        // ProtoChunk ignores out-of-height writes; postprocessing is separate.
        if p.y < self.terrain.level_min_y
            || p.y >= self.terrain.level_min_y + self.terrain.level_height
        {
            return Ok(());
        }
        self.terrain.image.set(
            p.x - self.terrain.chunk_x * 16,
            p.y,
            p.z - self.terrain.chunk_z * 16,
            state,
        )
    }
    fn has_fluid(&self, state: u32) -> Result<bool> {
        ensure!(
            (state as usize) < facts().fluids.len(),
            "unknown canonical carver block state"
        );
        Ok(facts().has_fluid(state))
    }
    fn aquifer_substance(&mut self, p: DensityPoint) -> Result<Option<u32>> {
        let mut context = CarverAquiferContext(MaterialContext {
            graph: &self.generator.graph,
            density: &mut self.terrain.density,
            router: self.generator.router,
            sea_level: self.generator.sea_level,
            default_fluid: self.generator.default_fluid,
        });
        Ok(self
            .terrain
            ._aquifer
            .compute_substance(p, 0.0, &mut context)?
            .map(|fluid| self.generator.palette.fluid(fluid)))
    }
    fn aquifer_should_schedule_fluid_update(&self) -> bool {
        self.terrain._aquifer.should_schedule_fluid_update()
    }
    fn top_material(&mut self, p: DensityPoint, has_fluid: bool) -> Result<Option<u32>> {
        let mut context = RawCarverSurfaceContext {
            biomes: &mut *self.biomes,
            temperature: &mut *self.temperature,
        };
        let mut host = TerrainSurfaceHost {
            image: &mut self.terrain.image,
            density: &mut self.terrain.density,
            graph: &self.generator.graph,
            context: &mut context,
            chunk_x: self.terrain.chunk_x,
            chunk_z: self.terrain.chunk_z,
            min_gen_y: self.min_gen_y,
            gen_depth: self.gen_depth,
            void_air: self.generator.palette.void_air,
        };
        self.generator.surface.top_material(&mut host, p, has_fluid)
    }
    fn mark_postprocessing(&mut self, p: DensityPoint) -> Result<()> {
        if p.y >= self.terrain.level_min_y
            && p.y < self.terrain.level_min_y + self.terrain.level_height
        {
            self.terrain.image.postprocessing.push(p);
        }
        Ok(())
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
fn validate_level_height(min_y: i32, height: i32) -> Result<i32> {
    let max_y = min_y
        .checked_add(height)
        .context("level height overflows")?;
    ensure!(
        height > 0 && min_y >= -2032 && max_y <= 2032,
        "level height exceeds original dimension coordinate range"
    );
    Ok(max_y)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn observed_empty() -> FreshGenerationContext {
        FreshGenerationContext {
            initial_blocks: GenerationInfluence::ObservedEmpty,
            blender: GenerationInfluence::ObservedEmpty,
            structure_beardifier: GenerationInfluence::ObservedEmpty,
        }
    }

    fn unavailable() -> FreshGenerationContext {
        FreshGenerationContext {
            initial_blocks: GenerationInfluence::Unavailable,
            blender: GenerationInfluence::Unavailable,
            structure_beardifier: GenerationInfluence::Unavailable,
        }
    }

    fn nether_generator() -> Result<Arc<NoiseGenerator>> {
        Ok(Arc::new(NoiseGenerator::builtin(123456789, "nether")?))
    }

    fn nether_terrain(generator: &NoiseGenerator) -> Result<NoiseTerrain<'_>> {
        generator.fill_noise(0, 0, 0, 128, observed_empty())
    }

    fn generated(outcome: OwnedNoiseFillOutcome) -> OwnedNoiseTerrain {
        match outcome {
            OwnedNoiseFillOutcome::Generated(terrain) => terrain,
            OwnedNoiseFillOutcome::NoCells { .. } => panic!("expected a positive-cell stage"),
        }
    }

    struct EmptyEndCarvingRegion {
        caches: BTreeMap<(i32, i32), carver::CarverBiomeCache>,
    }
    impl EmptyEndCarvingRegion {
        fn new() -> Self {
            Self {
                caches: (-8..=8)
                    .flat_map(|x| {
                        (-8..=8).map(move |z| ((x, z), carver::CarverBiomeCache::observed_absent()))
                    })
                    .collect(),
            }
        }
    }
    impl carver::CarvingNeighbors for EmptyEndCarvingRegion {
        fn carver_biome_cache(
            &mut self,
            chunk: (i32, i32),
        ) -> Result<&mut carver::CarverBiomeCache> {
            self.caches
                .get_mut(&chunk)
                .context("missing actual test neighbor")
        }
    }
    struct EndCarvingFacts(Vec<&'static str>);
    impl carver::CarvingRegionFacts for EndCarvingFacts {
        fn generation_height(&mut self, target: (i32, i32)) -> Result<(i32, i32)> {
            ensure!(target == (0, 0));
            self.0.push("height");
            Ok((0, 256))
        }
        fn is_upgrading(&mut self, target: (i32, i32)) -> Result<bool> {
            ensure!(target == (0, 0));
            self.0.push("upgrading");
            Ok(false)
        }
    }

    #[test]
    fn empty_end_air_invocation_retains_live_owners_and_advances_only_its_native_stage(
    ) -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(123456789, "end")?);
        let mut terrain = generator
            .prepare_chunk(0, 0, 0, 256, observed_empty())?
            .fill_noise()?;
        let mut thermal = BiomeTemperature::builtin()?;
        // This allocation/phase test supplies the actual seeded End source
        // directly. Natural stored BIOMES/SURFACE sequencing is separately
        // exercised by the paired original-stage oracle, not inferred here.
        let mut biomes = RawBiomeManager::new(
            generator.graph(),
            carver::CarverBiomeSource::End,
            super::super::biome_zoom::BiomeZoom::from_world_seed(generator.seed()),
        );
        terrain.build_surface(&mut RawCarverSurfaceContext {
            biomes: &mut biomes,
            temperature: &mut thermal,
        })?;
        terrain
            .density
            .preliminary_surface_level(generator.graph(), 0, 0)?;
        let density = terrain.density.allocation_fingerprint();
        let aquifer = terrain._aquifer.as_ref() as *const Aquifer;
        let image_pointer = terrain.states().as_ptr();
        let posts_pointer = terrain.postprocessing().as_ptr();
        let before = terrain.image.clone();
        let mut region = EmptyEndCarvingRegion::new();
        let mut facts = EndCarvingFacts(Vec::new());
        let mut mask = CarvingMask::new(256, 0)?;
        mask.set(1, 255, 2)?;
        let (mut terrain, mask, report) = terrain.apply_air_carvers(AirCarvingContext {
            world_seed: generator.seed(),
            source: carver::CarverBiomeSource::End,
            zoom: super::super::biome_zoom::BiomeZoom::from_world_seed(generator.seed()),
            temperature: &mut thermal,
            neighbors: &mut region,
            registry: &mut carver::BuiltinCarverRegistry,
            facts: &mut facts,
            mask: carver::AirMaskOwner::existing(
                (0, 0),
                mask,
                carver::CarvingMaskFilter::ObservedNoUpdate,
            ),
        })?;
        assert_eq!(facts.0, vec!["height", "upgrading"]);
        assert_eq!(terrain.stage(), TerrainStage::Carvers);
        assert_eq!(terrain.density.allocation_fingerprint(), density);
        assert_eq!(terrain._aquifer.as_ref() as *const Aquifer, aquifer);
        assert_eq!(terrain.states().as_ptr(), image_pointer);
        assert_eq!(terrain.postprocessing().as_ptr(), posts_pointer);
        assert_same_image(&terrain.image, &before);
        assert!(mask.get(1, 255, 2)?);
        assert_eq!(report.neighbors.len(), 289);
        assert_eq!(report.biome_queries.len(), 289);
        assert!(report.attempts.is_empty());
        assert_eq!(report.random_count, 0);
        assert!(report.random_state48 < (1_u64 << 48));
        let mut unused = NetherBiomeFixture::default();
        assert!(terrain.build_surface(&mut unused).is_err());
        assert!(unused.columns.is_empty());
        // A duplicate invocation is rejected before touching new phase owners.
        let mut duplicate_facts = EndCarvingFacts(Vec::new());
        let result = terrain.apply_air_carvers(AirCarvingContext {
            world_seed: generator.seed(),
            source: carver::CarverBiomeSource::End,
            zoom: super::super::biome_zoom::BiomeZoom::from_world_seed(generator.seed()),
            temperature: &mut thermal,
            neighbors: &mut region,
            registry: &mut carver::BuiltinCarverRegistry,
            facts: &mut duplicate_facts,
            mask: carver::AirMaskOwner::existing(
                (0, 0),
                mask,
                carver::CarvingMaskFilter::ObservedNoUpdate,
            ),
        });
        assert!(result.is_err());
        assert!(duplicate_facts.0.is_empty());
        assert_eq!(
            Arc::strong_count(&generator),
            1,
            "consumed failed stage releases its owner"
        );
        Ok(())
    }

    #[test]
    fn surface_initial_biome_y_uses_the_original_settings_flag() -> Result<()> {
        struct ObserveFirstBiome(Option<DensityPoint>);
        impl SurfaceContext for ObserveFirstBiome {
            fn biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
                self.0 = Some(DensityPoint::new(x, y, z));
                bail!("observed initial surface biome query")
            }
            fn cold_enough_to_snow(&mut self, _: &str, _: i32, _: i32, _: i32) -> Result<bool> {
                bail!("initial biome failure must precede thermal queries")
            }
            fn should_melt_frozen_ocean_iceberg_slightly(
                &mut self,
                _: &str,
                _: i32,
                _: i32,
                _: i32,
            ) -> Result<bool> {
                bail!("initial biome failure must precede iceberg queries")
            }
        }
        for (settings, min_y, height, legacy) in
            [("overworld", -64, 384, false), ("nether", 0, 128, true)]
        {
            let generator = NoiseGenerator::builtin(123456789, settings)?;
            let mut terrain = generator.fill_noise(0, 0, min_y, height, observed_empty())?;
            let first_available = terrain.world_surface_wg()[0];
            assert_ne!(
                first_available, 0,
                "fixture must distinguish zero from the surface height"
            );
            let mut context = ObserveFirstBiome(None);
            let error = generator
                .build_surface(&mut terrain, &mut context)
                .unwrap_err();
            assert_eq!(error.to_string(), "observed initial surface biome query");
            assert_eq!(
                context.0,
                Some(DensityPoint::new(
                    0,
                    if legacy { 0 } else { first_available },
                    0
                ))
            );
            assert_eq!(terrain.stage(), TerrainStage::Noise);
        }
        Ok(())
    }

    fn assert_same_image(a: &TerrainImage, b: &TerrainImage) {
        assert_eq!(a.min_y, b.min_y);
        assert_eq!(a.height, b.height);
        assert_eq!(a.states, b.states);
        assert_eq!(a.world_surface, b.world_surface);
        assert_eq!(a.ocean_floor, b.ocean_floor);
        assert_eq!(a.postprocessing, b.postprocessing);
    }

    /// Controlled fixed-nether-wastes profile used by the retained original
    /// SurfaceSystem observations. This test fixture supplies only that biome
    /// identity; thermal callbacks are deliberately unavailable because this
    /// profile's original rule path must not query them. It is not a runtime
    /// BIOMES implementation or a default production context.
    #[derive(Default)]
    struct NetherBiomeFixture {
        columns: std::collections::BTreeSet<(i32, i32)>,
        fail_after_first_column: bool,
    }
    impl SurfaceContext for NetherBiomeFixture {
        fn biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
            ensure!((0..16).contains(&x) && (0..16).contains(&z));
            ensure!((0..=128).contains(&y));
            self.columns.insert((x, z));
            if self.fail_after_first_column && self.columns.len() > 1 {
                bail!("controlled biome dependency becomes unavailable after one column");
            }
            Ok("minecraft:nether_wastes".to_owned())
        }
        fn cold_enough_to_snow(&mut self, _: &str, _: i32, _: i32, _: i32) -> Result<bool> {
            bail!("unexpected thermal query in fixed nether-wastes profile")
        }
        fn should_melt_frozen_ocean_iceberg_slightly(
            &mut self,
            _: &str,
            _: i32,
            _: i32,
            _: i32,
        ) -> Result<bool> {
            bail!("unexpected iceberg query in fixed nether-wastes profile")
        }
    }

    #[test]
    fn retained_stage_outlives_its_local_arc_and_moves_between_threads() -> Result<()> {
        fn send_static<T: Send + 'static>() {}
        send_static::<OwnedNoiseTerrain>();
        send_static::<OwnedNoiseFillOutcome>();
        let (terrain, weak) = {
            let generator = nether_generator()?;
            let weak = Arc::downgrade(&generator);
            let terrain =
                generated(generator.fill_from_noise_owned(0, 0, 0, 128, observed_empty())?);
            assert_eq!(Arc::strong_count(&generator), 2);
            (terrain, weak)
        };
        assert_eq!(weak.strong_count(), 1);
        let terrain = std::thread::spawn(move || -> Result<OwnedNoiseTerrain> {
            let mut terrain = terrain;
            assert_eq!(terrain.stage(), TerrainStage::Noise);
            let report = terrain.build_surface(&mut NetherBiomeFixture::default())?;
            assert!(report.rule_replacements > 0);
            assert_eq!(terrain.stage(), TerrainStage::Surface);
            Ok(terrain)
        })
        .join()
        .expect("owned generation job panicked")?;
        assert_eq!(weak.strong_count(), 1);
        drop(terrain);
        assert!(weak.upgrade().is_none());
        Ok(())
    }

    #[test]
    fn retention_rejects_an_equal_seed_but_different_graph_owner() -> Result<()> {
        let generator = nether_generator()?;
        let other = nether_generator()?;
        assert_eq!(generator.seed(), other.seed());
        assert_eq!(generator.settings_name(), other.settings_name());
        let terrain = nether_terrain(&generator)?;
        let error = match other.retain_terrain(terrain) {
            Err(error) => error,
            Ok(_) => panic!("distinct graph owners must not share live density caches"),
        };
        assert!(error.to_string().contains("different seeded generator"));
        assert_eq!(Arc::strong_count(&other), 1);
        Ok(())
    }

    #[test]
    fn retention_moves_the_existing_cache_and_image_allocations() -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(123456789, "overworld")?);
        let mut terrain = generator.fill_noise(0, 0, -64, 384, observed_empty())?;
        let first =
            terrain
                .noise
                .density_mut()
                .preliminary_surface_level(generator.graph(), -17, 21)?;
        let second =
            terrain
                .noise
                .density_mut()
                .preliminary_surface_level(generator.graph(), 35, -29)?;
        let allocations = terrain.noise.density().allocation_fingerprint();
        assert!(
            !allocations.2.is_empty(),
            "preliminary-map pointer evidence must be nonempty"
        );
        let cache_values = format!("{:?}", terrain.noise.density());
        let state = terrain.noise.density().state();
        let states_pointer = terrain.states().as_ptr();
        let posts_pointer = terrain.postprocessing().as_ptr();
        let image = terrain.image.clone();
        let mut owned = generator.retain_terrain(terrain)?;
        assert_eq!(owned.density.allocation_fingerprint(), allocations);
        assert_eq!(format!("{:?}", owned.density), cache_values);
        assert_eq!(owned.density.state(), state);
        assert_eq!(owned.states().as_ptr(), states_pointer);
        assert_eq!(owned.postprocessing().as_ptr(), posts_pointer);
        assert_same_image(&owned.image, &image);
        assert_eq!(
            owned
                .density
                .preliminary_surface_level(generator.graph(), -17, 21)?,
            first
        );
        assert_eq!(
            owned
                .density
                .preliminary_surface_level(generator.graph(), 35, -29)?,
            second
        );
        assert_eq!(owned.density.allocation_fingerprint(), allocations);
        assert_eq!(owned.density.state(), state);
        Ok(())
    }

    #[test]
    fn owned_and_borrowed_surface_stages_produce_the_same_material_effects() -> Result<()> {
        let generator = nether_generator()?;
        let mut borrowed = nether_terrain(&generator)?;
        let mut owned =
            generated(generator.fill_from_noise_owned(0, 0, 0, 128, observed_empty())?);
        assert_same_image(&borrowed.image, &owned.image);
        let before = owned.states().to_vec();
        let borrowed_report =
            generator.build_surface(&mut borrowed, &mut NetherBiomeFixture::default())?;
        let owned_report = owned.build_surface(&mut NetherBiomeFixture::default())?;
        assert_eq!(borrowed_report, owned_report);
        assert!(owned_report.rule_replacements > 0);
        assert_ne!(owned.states(), before);
        assert_eq!(owned.stage(), TerrainStage::Surface);
        assert_eq!(borrowed.stage(), owned.stage());
        assert_eq!(borrowed.chunk(), owned.chunk());
        assert_eq!(borrowed.min_y(), owned.min_y());
        assert_eq!(borrowed.height(), owned.height());
        assert_same_image(&borrowed.image, &owned.image);
        assert_eq!(borrowed.states(), owned.states());
        assert_eq!(borrowed.world_surface_wg(), owned.world_surface_wg());
        assert_eq!(borrowed.ocean_floor_wg(), owned.ocean_floor_wg());
        assert_eq!(borrowed.postprocessing(), owned.postprocessing());
        assert_eq!(borrowed.block(3, 64, 7)?, owned.block(3, 64, 7)?);
        // Retaining an already surfaced stage must not reset its phase.
        let surfaced_pointer = borrowed.states().as_ptr();
        let mut retained_surface = generator.retain_terrain(borrowed)?;
        assert_eq!(retained_surface.stage(), TerrainStage::Surface);
        assert_eq!(retained_surface.states().as_ptr(), surfaced_pointer);
        assert_same_image(&retained_surface.image, &owned.image);
        assert!(retained_surface
            .build_surface(&mut NetherBiomeFixture::default())
            .is_err());
        Ok(())
    }

    #[test]
    fn owned_surface_failure_is_transactional_and_duplicate_application_is_rejected() -> Result<()>
    {
        let generator = nether_generator()?;
        let mut owned = generator.retain_terrain(nether_terrain(&generator)?)?;
        let before = owned.image.clone();
        let states_pointer = owned.states().as_ptr();
        let posts_pointer = owned.postprocessing().as_ptr();
        let mut failing = NetherBiomeFixture {
            fail_after_first_column: true,
            ..Default::default()
        };
        assert!(owned.build_surface(&mut failing).is_err());
        assert!(
            failing.columns.len() > 1,
            "failure must follow a processed column"
        );
        assert_eq!(owned.stage(), TerrainStage::Noise);
        assert_same_image(&owned.image, &before);
        assert_eq!(owned.states().as_ptr(), states_pointer);
        assert_eq!(owned.postprocessing().as_ptr(), posts_pointer);
        owned.build_surface(&mut NetherBiomeFixture::default())?;
        let surfaced = owned.image.clone();
        let cache = owned.density.allocation_fingerprint();
        let mut unused = NetherBiomeFixture::default();
        let error = owned.build_surface(&mut unused).unwrap_err();
        assert!(error
            .to_string()
            .contains("surface stage already completed"));
        assert!(unused.columns.is_empty());
        assert_eq!(owned.stage(), TerrainStage::Surface);
        assert_same_image(&owned.image, &surfaced);
        assert_eq!(owned.density.allocation_fingerprint(), cache);
        Ok(())
    }

    #[test]
    fn owned_no_cells_retains_neither_a_generator_owner_nor_a_terrain_image() -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(1, "overworld")?);
        let weak = Arc::downgrade(&generator);
        let outcome =
            generator.fill_from_noise_owned(i32::MAX, i32::MIN, 384, 16, unavailable())?;
        assert!(matches!(
            outcome,
            OwnedNoiseFillOutcome::NoCells {
                min_y: 384,
                height: -64,
                cell_count_y: -8,
            }
        ));
        assert_eq!(Arc::strong_count(&generator), 1);
        assert!(generator
            .fill_from_noise_owned(0, 0, -64, 384, unavailable())
            .is_err());
        drop(generator);
        assert!(weak.upgrade().is_none());
        Ok(())
    }

    #[test]
    fn prepared_climate_and_noise_keep_the_actual_density_and_aquifer_owners() -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(123456789, "overworld")?);
        let mut prepared = generator.prepare_chunk(0, 0, -64, 384, observed_empty())?;
        let aquifer_pointer = prepared.aquifer.as_ref() as *const Aquifer;
        assert!(matches!(prepared.aquifer.as_ref(), Aquifer::NoiseBased(_)));
        let level = prepared
            .noise
            .density_mut_for(generator.graph())?
            .preliminary_surface_level(generator.graph(), -17, 21)?;
        let allocations = prepared
            .noise
            .density_for(generator.graph())?
            .allocation_fingerprint();
        assert!(!allocations.2.is_empty());
        let state = prepared.noise.density_for(generator.graph())?.state();
        let points = [
            (0, -16, 0),
            (-1, 20, 7),
            (4, 40, -5),
            (i32::MAX, 0, i32::MIN),
        ];
        let before = points
            .iter()
            .map(|&(x, y, z)| prepared.sample_climate(x, y, z))
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(
            prepared.noise.density_for(generator.graph())?.state(),
            state
        );
        assert_eq!(
            prepared
                .noise
                .density_for(generator.graph())?
                .allocation_fingerprint(),
            allocations
        );
        assert_eq!(prepared.aquifer.as_ref() as *const Aquifer, aquifer_pointer);
        assert_eq!(prepared.chunk(), (0, 0));
        assert_eq!((prepared.min_y(), prepared.height()), (-64, 384));
        let mut owned = prepared.fill_noise()?;
        assert_eq!(owned._aquifer.as_ref() as *const Aquifer, aquifer_pointer);
        let after_allocations = owned.density.allocation_fingerprint();
        assert_eq!(
            (after_allocations.0, after_allocations.1),
            (allocations.0, allocations.1)
        );
        assert_eq!(
            owned
                .density
                .preliminary_surface_level(generator.graph(), -17, 21)?,
            level
        );
        assert!(!owned.density.state().interpolating);
        assert!(owned.density.state().array_counter > state.array_counter);
        let after = points
            .iter()
            .map(|&(x, y, z)| owned.sample_climate(x, y, z))
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(before, after);
        let direct = generator.fill_noise(0, 0, -64, 384, observed_empty())?;
        assert_same_image(&owned.image, &direct.image);
        assert_eq!(owned.stage(), TerrainStage::Noise);
        Ok(())
    }

    #[test]
    fn prepared_owner_moves_out_of_scope_and_through_surface_without_replacement() -> Result<()> {
        fn send_static<T: Send + 'static>() {}
        send_static::<PreparedGeneration>();
        let (prepared, weak, aquifer_pointer) = {
            let generator = nether_generator()?;
            let weak = Arc::downgrade(&generator);
            let prepared = generator.prepare_chunk(0, 0, 0, 128, observed_empty())?;
            let pointer = prepared.aquifer.as_ref() as *const Aquifer as usize;
            (prepared, weak, pointer)
        };
        assert_eq!(weak.strong_count(), 1);
        let mut owned = std::thread::spawn(move || -> Result<OwnedNoiseTerrain> {
            let mut prepared = prepared;
            let sample = prepared.sample_climate(-1, 4, 7)?;
            let mut owned = prepared.fill_noise()?;
            assert_eq!(owned.sample_climate(-1, 4, 7)?, sample);
            assert_eq!(
                owned._aquifer.as_ref() as *const Aquifer as usize,
                aquifer_pointer
            );
            let report = owned.build_surface(&mut NetherBiomeFixture::default())?;
            assert!(report.rule_replacements > 0);
            assert_eq!(
                owned._aquifer.as_ref() as *const Aquifer as usize,
                aquifer_pointer
            );
            assert_eq!(owned.sample_climate(-1, 4, 7)?, sample);
            Ok(owned)
        })
        .join()
        .expect("prepared generation job panicked")?;
        assert_eq!(owned.stage(), TerrainStage::Surface);
        assert_eq!(weak.strong_count(), 1);
        assert!(owned
            .build_surface(&mut NetherBiomeFixture::default())
            .is_err());
        drop(owned);
        assert!(weak.upgrade().is_none());
        Ok(())
    }

    #[test]
    fn prepared_fill_and_direct_fill_share_material_and_surface_semantics() -> Result<()> {
        let generator = nether_generator()?;
        let mut direct =
            generated(generator.fill_from_noise_owned(0, 0, 0, 128, observed_empty())?);
        let mut prepared = generator.prepare_chunk(0, 0, 0, 128, observed_empty())?;
        for (x, y, z) in [(0, 0, 0), (-1, 8, 4), (15, 31, 15)] {
            let _ = prepared.sample_climate(x, y, z)?;
        }
        let mut resumed = prepared.fill_noise()?;
        assert_same_image(&direct.image, &resumed.image);
        let mut failing = NetherBiomeFixture {
            fail_after_first_column: true,
            ..Default::default()
        };
        let before = resumed.image.clone();
        let aquifer_pointer = resumed._aquifer.as_ref() as *const Aquifer;
        assert!(resumed.build_surface(&mut failing).is_err());
        assert!(failing.columns.len() > 1);
        assert_eq!(resumed.stage(), TerrainStage::Noise);
        assert_same_image(&resumed.image, &before);
        assert_eq!(resumed._aquifer.as_ref() as *const Aquifer, aquifer_pointer);
        let direct_report = direct.build_surface(&mut NetherBiomeFixture::default())?;
        let resumed_report = resumed.build_surface(&mut NetherBiomeFixture::default())?;
        assert_eq!(direct_report, resumed_report);
        assert_same_image(&direct.image, &resumed.image);
        assert_eq!(direct.stage(), resumed.stage());
        Ok(())
    }

    #[test]
    fn prepared_setup_is_strict_and_private_restore_rejects_another_graph() -> Result<()> {
        let generator = nether_generator()?;
        assert!(generator
            .prepare_chunk(0, 0, 0, 128, unavailable())
            .is_err());
        assert!(generator
            .prepare_chunk(0, 0, 128, 16, observed_empty())
            .is_err());
        assert!(generator
            .prepare_chunk(0, 0, 1, 127, observed_empty())
            .is_err());
        assert!(generator
            .prepare_chunk(i32::MAX, 0, 0, 128, observed_empty())
            .is_err());
        assert_eq!(Arc::strong_count(&generator), 1);
        assert!(matches!(
            generator.fill_from_noise_owned(i32::MAX, i32::MIN, 128, 16, unavailable(),)?,
            OwnedNoiseFillOutcome::NoCells {
                min_y: 128,
                height: 0,
                cell_count_y: 0
            }
        ));
        let other = nether_generator()?;
        let mut prepared = generator.prepare_chunk(0, 0, 0, 128, observed_empty())?;
        // Only this module can replace the private captured Arc. The internal
        // graph identity guard must still reject an accidental reassociation.
        prepared.generator = Arc::clone(&other);
        assert!(prepared.sample_climate(0, 0, 0).is_err());
        assert!(prepared.fill_noise().is_err());
        assert_eq!(Arc::strong_count(&other), 1);
        Ok(())
    }

    #[test]
    fn retained_raw_erosion_matches_original_end_observations_before_and_after_noise() -> Result<()>
    {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../fixtures/world-generation-biome-source-vectors.json"
        ))?;
        for sequence in fixture["end_sequences"]
            .as_array()
            .context("End source sequences")?
        {
            let seed = sequence["seed"].as_str().context("seed")?.parse::<i64>()?;
            let settings = sequence["settings"].as_str().context("settings")?;
            let chunk = sequence["chunk"].as_array().context("chunk")?;
            let chunk_x = chunk[0].as_i64().context("chunk X")?.try_into()?;
            let chunk_z = chunk[1].as_i64().context("chunk Z")?.try_into()?;
            let generator = Arc::new(NoiseGenerator::builtin(seed, settings)?);
            let mut prepared =
                generator.prepare_chunk(chunk_x, chunk_z, 0, 128, observed_empty())?;
            let state_before = prepared.noise.density_for(generator.graph())?.state();
            let allocations = prepared
                .noise
                .density_for(generator.graph())?
                .allocation_fingerprint();
            let mut observations = Vec::new();
            for query in sequence["queries"]
                .as_array()
                .context("End source queries")?
            {
                let Some(point) = query["erosion_point"].as_array() else {
                    continue;
                };
                let point = DensityPoint::new(
                    point[0].as_i64().context("erosion X")?.try_into()?,
                    point[1].as_i64().context("erosion Y")?.try_into()?,
                    point[2].as_i64().context("erosion Z")?.try_into()?,
                );
                let expected = query["erosion_bits"]
                    .as_str()
                    .context("original erosion bits")?
                    .parse::<u64>()?;
                assert_eq!(prepared.sample_erosion(point)?.to_bits(), expected);
                observations.push((point, expected));
            }
            assert!(!observations.is_empty());
            assert_eq!(
                prepared.noise.density_for(generator.graph())?.state(),
                state_before
            );
            assert_eq!(
                prepared
                    .noise
                    .density_for(generator.graph())?
                    .allocation_fingerprint(),
                allocations
            );
            let mut owned = prepared.fill_noise()?;
            let state_after = owned.density.state();
            let after_allocations = owned.density.allocation_fingerprint();
            assert_eq!(
                (after_allocations.0, after_allocations.1),
                (allocations.0, allocations.1)
            );
            for (point, expected) in observations {
                assert_eq!(owned.sample_erosion(point)?.to_bits(), expected);
            }
            assert_eq!(owned.density.state(), state_after);
            assert_eq!(owned.density.allocation_fingerprint(), after_allocations);
            assert_eq!(owned.stage(), TerrainStage::Noise);
        }
        Ok(())
    }

    #[test]
    fn positive_fill_requires_the_actual_dependency_owners() -> Result<()> {
        let generator = NoiseGenerator::builtin(1, "overworld")?;
        let unavailable = FreshGenerationContext {
            initial_blocks: GenerationInfluence::Unavailable,
            blender: GenerationInfluence::Unavailable,
            structure_beardifier: GenerationInfluence::Unavailable,
        };
        assert!(matches!(
            generator.fill_from_noise(0, 0, 320, 16, unavailable)?,
            NoiseFillOutcome::NoCells { .. }
        ));
        let error = match generator.fill_from_noise(0, 0, -64, 384, unavailable) {
            Err(error) => error,
            Ok(_) => panic!("actual cells must not generate without dependency owners"),
        };
        assert!(error.to_string().contains("actually empty new ProtoChunk"));
        Ok(())
    }
}
