//! Generation over the caller's same live target, without rebuilding its data.
//!
//! This module is a child of `generator`: the cached density and aquifer owners
//! use the same private material/surface implementation as the fresh APIs.
use super::{GenerationInfluence, NoiseGenerator};
use crate::{
    density::{ChunkDensity, ClimateTargetPoint, DensityPoint},
    worldgen::{
        aquifer::Aquifer,
        beardifier::Beardifier,
        biome_source::{ClimateSampler, ErosionSampler},
        biome_temperature::BiomeTemperature,
        biome_zoom::{BiomeZoom, QuartBiomeProvider},
        biomes::{self, ResolvedBiome},
        blending::Blender,
        noise_chunk::{NoiseChunk, NoiseFillPlan, RetainedNoiseChunk},
        retrogen,
        surface::SurfaceReport,
        target::{GenerationTargetHost, HeightRange, NoiseHeightmap},
    },
};
use anyhow::{ensure, Context, Result};
use std::sync::Arc;

/// Local invocation state, never an original ChunkStatus or readiness result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetGenerationPhase {
    Prepared,
    Biomes,
    NoiseFilled,
    NoiseSkipped,
    NoiseTaskFinished,
    Surface,
    SurfaceSkipped,
    /// The same target/cache prefix is retained, but cannot be resumed.
    Poisoned,
}

/// The material fill's outcome. The outer noise task still applies actual
/// retrogen fixups after either successful outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetNoiseOutcome {
    Filled,
    NoCells {
        min_y: i32,
        height: i32,
        cell_count_y: i32,
    },
}

/// Public buildSurface can return before entering SurfaceSystem for the actual
/// debug-void predicate. That is a skipped invocation, not a zero-work report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSurfaceOutcome {
    Built(SurfaceReport),
    DebugVoidSkipped,
}

struct TargetNoiseOwners {
    noise: RetainedNoiseChunk,
    aquifer: Box<Aquifer>,
    chunk: (i32, i32),
    build: HeightRange,
}

#[derive(Clone)]
enum BeardifierInput {
    Observation(GenerationInfluence),
    Actual(Option<Arc<Beardifier>>),
}
impl BeardifierInput {
    /// Resolve only when a real NoiseChunk factory is entered. Unavailable
    /// input and an observed-empty actual owner remain distinct.
    fn owner(self) -> Result<Arc<Beardifier>> {
        match self {
            Self::Observation(GenerationInfluence::ObservedEmpty) => Ok(Beardifier::empty()),
            Self::Observation(GenerationInfluence::Present) => {
                anyhow::bail!("present structure influence requires an actual beardifier owner")
            }
            Self::Observation(GenerationInfluence::Unavailable) => {
                anyhow::bail!("actual structure beardifier owner is unavailable")
            }
            Self::Actual(owner) => {
                owner.context("actual structure beardifier owner is unavailable")
            }
        }
    }
}

/// A short-lived view of the same wrapped chunk functions. It implements the
/// existing source-selector contracts without constructing a PreparedGeneration,
/// random source, density graph or second cache owner.
pub struct TargetClimateSampler<'a> {
    generator: &'a NoiseGenerator,
    density: &'a mut ChunkDensity,
}
impl ClimateSampler for TargetClimateSampler<'_> {
    type Error = anyhow::Error;
    fn sample_climate(&mut self, x: i32, y: i32, z: i32) -> Result<ClimateTargetPoint> {
        self.generator.sample_climate_from(self.density, x, y, z)
    }
}
impl ErosionSampler for TargetClimateSampler<'_> {
    type Error = anyhow::Error;
    fn sample_erosion(&mut self, point: DensityPoint) -> Result<f64> {
        self.generator.sample_erosion_from(self.density, point)
    }
}

/// Own the actual input target and its retained seeded generation owners.
///
/// Construction is deliberately lazy: even coordinates and build bounds are not
/// queried until a source phase needs them. A no-cell material fill touches only
/// the generation accessor and leaves the same Box, prior caches, blocks, maps
/// and queues untouched. There is no Clone, replacement-target constructor or
/// mutable target escape. A failed phase keeps its real mutation/cache prefix
/// and poisons this session; no image-only rollback is claimed.
pub struct TargetGeneration<T: GenerationTargetHost + ?Sized> {
    target: Box<T>,
    generator: Arc<NoiseGenerator>,
    initial_blender: Option<Arc<Blender>>,
    structure_beardifier: BeardifierInput,
    owners: Option<TargetNoiseOwners>,
    phase: TargetGenerationPhase,
}
impl<T: GenerationTargetHost + ?Sized> TargetGeneration<T> {
    pub fn new(
        generator: Arc<NoiseGenerator>,
        target: Box<T>,
        blender: Arc<Blender>,
        structure_beardifier: GenerationInfluence,
    ) -> Self {
        Self::new_with_optional_blender(generator, target, Some(blender), structure_beardifier)
    }

    /// Preserve an actually unavailable Blender without substituting EMPTY.
    /// A skipped fill can return before that owner is accessed; positive cache
    /// construction requires Some(actual_owner). This constructor is also lazy.
    pub fn new_with_optional_blender(
        generator: Arc<NoiseGenerator>,
        target: Box<T>,
        blender: Option<Arc<Blender>>,
        structure_beardifier: GenerationInfluence,
    ) -> Self {
        Self {
            target,
            generator,
            initial_blender: blender,
            structure_beardifier: BeardifierInput::Observation(structure_beardifier),
            owners: None,
            phase: TargetGenerationPhase::Prepared,
        }
    }

    /// Capture actual already-selected influences without querying them or the
    /// target. None preserves an unavailable owner; it never becomes EMPTY.
    /// Selection and public phase dependency reads remain caller-owned work.
    pub fn new_with_influences(
        generator: Arc<NoiseGenerator>,
        target: Box<T>,
        blender: Option<Arc<Blender>>,
        beardifier: Option<Arc<Beardifier>>,
    ) -> Self {
        Self {
            target,
            generator,
            initial_blender: blender,
            structure_beardifier: BeardifierInput::Actual(beardifier),
            owners: None,
            phase: TargetGenerationPhase::Prepared,
        }
    }

    pub fn target(&self) -> &T {
        &self.target
    }
    pub fn phase(&self) -> TargetGenerationPhase {
        self.phase
    }
    pub fn has_cached_noise_chunk(&self) -> bool {
        self.owners.is_some()
    }

    /// Read the immutable owner actually captured by the graph-bound cache.
    /// None means no cache exists; no constructor argument is substituted.
    pub fn captured_beardifier(&self) -> Result<Option<Arc<Beardifier>>> {
        self.owners
            .as_ref()
            .map(|owners| owners.noise.beardifier_for(self.generator.graph()))
            .transpose()
    }

    fn active(&self) -> Result<()> {
        ensure!(
            self.phase != TargetGenerationPhase::Poisoned,
            "target generation is poisoned"
        );
        Ok(())
    }
    fn unfilled(&self) -> Result<()> {
        self.active()?;
        ensure!(
            matches!(
                self.phase,
                TargetGenerationPhase::Prepared | TargetGenerationPhase::Biomes
            ),
            "target material fill already attempted"
        );
        Ok(())
    }

    fn ensure_owners(
        &mut self,
        blender: Option<Arc<Blender>>,
        beardifier: BeardifierInput,
    ) -> Result<()> {
        let chunk = self.target.chunk_pos();
        let build = self.target.build_height();
        if let Some(owners) = &self.owners {
            ensure!(
                owners.chunk == chunk && owners.build == build,
                "target coordinates or build accessor changed after NoiseChunk creation"
            );
            return Ok(());
        }
        let blender = blender.context("actual Blender owner is unavailable")?;
        let beardifier = beardifier.owner()?;
        let (noise, aquifer) = self
            .generator
            .create_target_noise_owners_with_influences(chunk, build, blender, beardifier)?;
        self.owners = Some(TargetNoiseOwners {
            noise: noise.into_retained(),
            aquifer,
            chunk,
            build,
        });
        Ok(())
    }

    /// Explicit sampling before BIOMES/NOISE creates the original full-build
    /// owners once. A subsequent skipped fill preserves those exact owners.
    pub fn with_sampler<R>(
        &mut self,
        sample: impl FnOnce(&mut TargetClimateSampler<'_>) -> Result<R>,
    ) -> Result<R> {
        self.active()?;
        let result = (|| {
            self.ensure_owners(
                self.initial_blender.clone(),
                self.structure_beardifier.clone(),
            )?;
            let density = self
                .owners
                .as_mut()
                .expect("created owners")
                .noise
                .density_mut_for(self.generator.graph())?;
            sample(&mut TargetClimateSampler {
                generator: &self.generator,
                density,
            })
        })();
        if result.is_err() {
            self.phase = TargetGenerationPhase::Poisoned;
        }
        result
    }

    /// Write actual generation-accessor biome sections. The callback borrows the
    /// caller's persistent selector and receives the same mutable sampler for
    /// every lazy fallback. Holder registry-key ownership stays explicit.
    pub fn fill_biomes(
        &mut self,
        mut resolve: impl FnMut(&mut TargetClimateSampler<'_>, i32, i32, i32) -> Result<ResolvedBiome>,
    ) -> Result<()> {
        self.active()?;
        ensure!(
            self.phase == TargetGenerationPhase::Prepared,
            "target BIOMES already attempted"
        );
        self.phase = TargetGenerationPhase::Poisoned;
        self.ensure_owners(
            self.initial_blender.clone(),
            self.structure_beardifier.clone(),
        )?;
        let owners = self.owners.as_mut().expect("created owners");
        let blender = owners.noise.blender_for(self.generator.graph())?;
        let density = owners.noise.density_mut_for(self.generator.graph())?;
        let mut sampler = TargetClimateSampler {
            generator: &self.generator,
            density,
        };
        biomes::fill_target_biomes(&mut *self.target, &blender, |x, y, z| {
            resolve(&mut sampler, x, y, z)
        })?;
        self.phase = TargetGenerationPhase::Biomes;
        Ok(())
    }

    /// Original fillFromNoise boundary only: no retrogen fixups or persisted
    /// status advancement. Those belong to the separate successful noise task.
    pub fn fill_from_noise(&mut self) -> Result<TargetNoiseOutcome> {
        self.unfilled()?;
        self.phase = TargetGenerationPhase::Poisoned;
        let generation = self.target.generation_height()?;
        let settings = self
            .generator
            .graph
            .grid()
            .context("missing noise settings")?;
        let plan = NoiseFillPlan::new(settings, generation.min_y, generation.height)?;
        if !plan.has_cells() {
            let grid = plan.grid();
            self.phase = TargetGenerationPhase::NoiseSkipped;
            return Ok(TargetNoiseOutcome::NoCells {
                min_y: grid.min_y,
                height: grid.height,
                cell_count_y: plan.cell_count_y(),
            });
        }

        // Source acquires the actual sections before doFill asks for its cached
        // NoiseChunk. The owned guard releases every acquired section on error.
        let (top, bottom) = plan.section_lock_span().expect("positive cell count");
        let lease = self.target.acquire_noise_sections(top, bottom)?;
        self.ensure_owners(
            self.initial_blender.clone(),
            self.structure_beardifier.clone(),
        )?;
        self.target
            .get_or_create_wg_heightmap(NoiseHeightmap::OceanFloorWg)?;
        self.target
            .get_or_create_wg_heightmap(NoiseHeightmap::WorldSurfaceWg)?;

        let TargetNoiseOwners {
            noise,
            mut aquifer,
            chunk,
            build,
        } = self.owners.take().expect("created owners");
        let mut noise = NoiseChunk::restore(self.generator.graph(), noise)?;
        let target = &mut *self.target;
        let generator = &self.generator;
        let result = noise.visit_blocks_with_plan(&plan, |_graph, density, point| {
            let state = generator.material_state(density, aquifer.as_mut(), point)?;
            // Generated canonical AIR is an absent write, not a request to clear
            // an old target block. Other air variants keep source identity.
            if state == generator.palette.air {
                return Ok(());
            }
            if target.debug_void_terrain()? {
                return Ok(());
            }
            target.direct_section_set(&lease, point, state)?;
            target.update_wg_heightmap(NoiseHeightmap::OceanFloorWg, point, state)?;
            target.update_wg_heightmap(NoiseHeightmap::WorldSurfaceWg, point, state)?;
            if aquifer.should_schedule_fluid_update() && target.state_facts(state)?.has_fluid {
                target.mark_pos_for_postprocessing(point)?;
            }
            Ok(())
        });
        self.owners = Some(TargetNoiseOwners {
            noise: noise.into_retained(),
            aquifer,
            chunk,
            build,
        });
        drop(lease);
        result?;
        self.phase = TargetGenerationPhase::NoiseFilled;
        Ok(TargetNoiseOutcome::Filled)
    }

    /// The original ChunkStatusTasks successful-future continuation. Fixups run
    /// even after NoCells, preserve queue prefixes and never advance status.
    pub fn finish_noise_task(&mut self) -> Result<()> {
        self.active()?;
        ensure!(
            matches!(
                self.phase,
                TargetGenerationPhase::NoiseFilled | TargetGenerationPhase::NoiseSkipped
            ),
            "noise-task fixups require a successful material fill outcome"
        );
        self.phase = TargetGenerationPhase::Poisoned;
        retrogen::finish_noise_task(&mut *self.target)?;
        self.phase = TargetGenerationPhase::NoiseTaskFinished;
        Ok(())
    }

    pub fn generate_noise_task(&mut self) -> Result<TargetNoiseOutcome> {
        let outcome = self.fill_from_noise()?;
        self.finish_noise_task()?;
        Ok(outcome)
    }

    /// Source SURFACE over the same live target and cached owners. A recollected
    /// phase Blender is only a factory input when no NoiseChunk exists; an
    /// existing cached owner is retained without an Arc-equality requirement.
    pub fn build_surface<N: QuartBiomeProvider<Biome = String> + ?Sized>(
        &mut self,
        phase_blender: Arc<Blender>,
        region_height: HeightRange,
        zoom: BiomeZoom,
        neighbors: &mut N,
        temperature: &mut BiomeTemperature,
    ) -> Result<TargetSurfaceOutcome>
    where
        N::Error: Into<anyhow::Error>,
    {
        self.build_surface_with_input(
            phase_blender,
            self.structure_beardifier.clone(),
            region_height,
            zoom,
            neighbors,
            temperature,
        )
    }

    /// The recollected phase influences are used only if the actual target has
    /// no cached NoiseChunk. A cache hit retains its initial structure owner
    /// even when the later argument is different or unavailable.
    pub fn build_surface_with_influences<N: QuartBiomeProvider<Biome = String> + ?Sized>(
        &mut self,
        phase_blender: Arc<Blender>,
        phase_beardifier: Option<Arc<Beardifier>>,
        region_height: HeightRange,
        zoom: BiomeZoom,
        neighbors: &mut N,
        temperature: &mut BiomeTemperature,
    ) -> Result<TargetSurfaceOutcome>
    where
        N::Error: Into<anyhow::Error>,
    {
        self.build_surface_with_input(
            phase_blender,
            BeardifierInput::Actual(phase_beardifier),
            region_height,
            zoom,
            neighbors,
            temperature,
        )
    }

    fn build_surface_with_input<N: QuartBiomeProvider<Biome = String> + ?Sized>(
        &mut self,
        phase_blender: Arc<Blender>,
        phase_beardifier: BeardifierInput,
        region_height: HeightRange,
        zoom: BiomeZoom,
        neighbors: &mut N,
        temperature: &mut BiomeTemperature,
    ) -> Result<TargetSurfaceOutcome>
    where
        N::Error: Into<anyhow::Error>,
    {
        self.active()?;
        ensure!(
            self.phase == TargetGenerationPhase::NoiseTaskFinished,
            "SURFACE requires the successful noise-task continuation"
        );
        self.phase = TargetGenerationPhase::Poisoned;
        if self.target.debug_void_terrain()? {
            self.phase = TargetGenerationPhase::SurfaceSkipped;
            return Ok(TargetSurfaceOutcome::DebugVoidSkipped);
        }
        self.ensure_owners(Some(phase_blender), phase_beardifier)?;
        let density = self
            .owners
            .as_mut()
            .expect("created owners")
            .noise
            .density_mut_for(self.generator.graph())?;
        let report = super::target_surface::build_surface(
            &self.generator,
            density,
            &mut *self.target,
            region_height,
            zoom,
            neighbors,
            temperature,
        )?;
        self.phase = TargetGenerationPhase::Surface;
        Ok(TargetSurfaceOutcome::Built(report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        chunk_readiness::ChunkStatus,
        worldgen::{
            beardifier::{BlockBox, Rigid, TerrainAdjustment},
            retrogen::RetrogenSlot,
            target::{
                BuiltinStateFacts, NativeProtoTarget, NativeSection, NativeTargetParts,
                SourceStateFacts,
            },
        },
    };

    /// Declared existing palettes and owners, not generated terrain or runtime
    /// status. No required world/storage callback is replaced by a fallback.
    fn target(build: HeightRange) -> Result<Box<NativeProtoTarget>> {
        let facts: Arc<dyn SourceStateFacts> = Arc::new(BuiltinStateFacts::new()?);
        let mut sections = Vec::new();
        for _ in 0..build.section_count()? {
            sections.push(NativeSection::from_palettes(
                Box::new([facts.defaults().deepslate; 4096]),
                Box::new(std::array::from_fn(|_| "minecraft:plains".to_owned())),
                facts.as_ref(),
            )?);
        }
        NativeProtoTarget::from_parts(
            NativeTargetParts {
                chunk: (0, 0),
                build,
                sections: sections.into_boxed_slice(),
                persisted_status: ChunkStatus::EMPTY,
                unsaved: false,
                retrogen: RetrogenSlot::ObservedAbsent,
                ocean_floor_wg: None,
                world_surface_wg: None,
                postprocessing: (0..build.section_count()?)
                    .map(|_| None)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                debug_void_terrain: false,
                unrelated: (),
            },
            facts,
        )
    }

    fn nonempty_owner() -> Arc<Beardifier> {
        Beardifier::from_ordered_parts(
            vec![Rigid {
                bounds: BlockBox {
                    min_x: 0,
                    min_y: 320,
                    min_z: 0,
                    max_x: 15,
                    max_y: 328,
                    max_z: 15,
                },
                terrain_adjustment: TerrainAdjustment::Bury,
                ground_level_delta: 0,
            }],
            Vec::new(),
        )
    }

    #[test]
    fn actual_owner_is_captured_once_and_survives_skipped_fill() -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(123456789, "overworld")?);
        let initial = nonempty_owner();
        let mut job = TargetGeneration::new_with_influences(
            generator,
            target(HeightRange::new(320, 64)?)?,
            Some(Blender::empty()),
            Some(Arc::clone(&initial)),
        );
        assert!(job.captured_beardifier()?.is_none());
        job.with_sampler(|_| Ok(()))?;
        assert!(Arc::ptr_eq(&job.captured_beardifier()?.unwrap(), &initial));
        let allocations = job
            .owners
            .as_ref()
            .unwrap()
            .noise
            .density_for(job.generator.graph())?
            .allocation_fingerprint();
        job.ensure_owners(
            Some(Blender::empty()),
            BeardifierInput::Actual(Some(Beardifier::empty())),
        )?;
        job.ensure_owners(None, BeardifierInput::Actual(None))?;
        assert!(Arc::ptr_eq(&job.captured_beardifier()?.unwrap(), &initial));
        assert!(matches!(
            job.fill_from_noise()?,
            TargetNoiseOutcome::NoCells {
                cell_count_y: 0,
                ..
            }
        ));
        assert_eq!(
            job.owners
                .as_ref()
                .unwrap()
                .noise
                .density_for(job.generator.graph())?
                .allocation_fingerprint(),
            allocations
        );
        job.finish_noise_task()?;
        assert!(Arc::ptr_eq(&job.captured_beardifier()?.unwrap(), &initial));
        Ok(())
    }

    #[test]
    fn unavailable_actual_owner_is_lazy_and_failed_positive_fill_releases_sections() -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(123456789, "overworld")?);
        let mut skipped = TargetGeneration::new_with_influences(
            Arc::clone(&generator),
            target(HeightRange::new(320, 64)?)?,
            None,
            None,
        );
        assert!(matches!(
            skipped.fill_from_noise()?,
            TargetNoiseOutcome::NoCells { .. }
        ));
        assert!(skipped.captured_beardifier()?.is_none());
        assert!(!skipped.has_cached_noise_chunk());

        let mut positive = TargetGeneration::new_with_influences(
            Arc::clone(&generator),
            target(HeightRange::new(0, 16)?)?,
            Some(Blender::empty()),
            None,
        );
        let error = positive.fill_from_noise().unwrap_err();
        assert!(error
            .to_string()
            .contains("beardifier owner is unavailable"));
        assert_eq!(positive.phase(), TargetGenerationPhase::Poisoned);
        assert!(positive
            .target()
            .sections()
            .iter()
            .all(|section| !section.is_acquired()));
        assert!(!positive.has_cached_noise_chunk());
        assert!(positive
            .target()
            .heightmap(NoiseHeightmap::OceanFloorWg)
            .is_none());
        assert!(positive
            .target()
            .heightmap(NoiseHeightmap::WorldSurfaceWg)
            .is_none());

        let mut observed_present = TargetGeneration::new(
            generator,
            target(HeightRange::new(0, 16)?)?,
            Blender::empty(),
            GenerationInfluence::Present,
        );
        assert!(observed_present
            .fill_from_noise()
            .unwrap_err()
            .to_string()
            .contains("requires an actual beardifier owner"));
        Ok(())
    }
}
