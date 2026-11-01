//! Staged quart BIOMES data over the actual prepared density/aquifer owner.
//!
//! Resolver traversal matches ChunkAccess/LevelChunkSection, while storage is
//! canonical quart Y/Z/X. This module supplies data, never a ChunkStatus or a
//! completed runtime chunk. Retrogen and nonempty blending remain unsupported.
use super::{
    biome_context::BuiltinBiomeSurfaceContext,
    biome_source::{select_end_biome, BuiltinMultiNoiseBiomeSource},
    biome_temperature::BiomeTemperature,
    biome_zoom::{BiomeZoom, QuartBiomeProvider},
    generator::{OwnedNoiseTerrain, PreparedGeneration},
    surface::SurfaceReport,
};
use anyhow::{ensure, Context, Result};

/// An observation from the actual ProtoChunk/retrogen owner, not an inference
/// from seed, coordinates or height. No default assumes absent retrogen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrogenContext {
    ObservedAbsent,
    Present,
    Unavailable,
}

/// The actual resolver is supplied the same mutable prepared sampler for every
/// query. Missing registry/source/dependency owners must return errors. A fixed
/// biome, raw graph or a fresh sampler is not an implicit replacement.
pub trait BiomeResolver {
    fn get_noise_biome(
        &mut self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
        sampler: &mut PreparedGeneration,
    ) -> Result<String>;
}
impl BiomeResolver for BuiltinMultiNoiseBiomeSource {
    fn get_noise_biome(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        sampler: &mut PreparedGeneration,
    ) -> Result<String> {
        BuiltinMultiNoiseBiomeSource::get_noise_biome(self, x, y, z, sampler)
    }
}

/// Explicit built-in selection. Multi-noise borrows the worker's persistent
/// search cursor; the End source samples raw erosion from this prepared owner.
/// A preset is never inferred from a dimension name or terrain settings key.
pub enum BuiltinBiomeResolver<'a> {
    MultiNoise(&'a mut BuiltinMultiNoiseBiomeSource),
    End,
}
impl BiomeResolver for BuiltinBiomeResolver<'_> {
    fn get_noise_biome(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        sampler: &mut PreparedGeneration,
    ) -> Result<String> {
        match self {
            Self::MultiNoise(source) => {
                BuiltinMultiNoiseBiomeSource::get_noise_biome(*source, x, y, z, sampler)
            }
            Self::End => select_end_biome(x, y, z, sampler).map(str::to_owned),
        }
    }
}

/// Complete computed palettes for the full level-accessor section span.
/// Fields are private and no constructor fills unknown cells with a default.
/// Names belong to the caller's actual registry; built-in resolvers require the
/// pinned built-in registry contents, not merely matching preset key names.
#[derive(Debug, PartialEq, Eq)]
pub struct BiomeImage {
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: i32,
    min_quart_y: i32,
    max_quart_y: i32,
    storage_min_quart_y: i32,
    storage_quart_height: i32,
    values: Vec<String>,
}
impl BiomeImage {
    fn fill<R: BiomeResolver + ?Sized>(
        prepared: &mut PreparedGeneration,
        resolver: &mut R,
    ) -> Result<Self> {
        let (chunk_x, chunk_z) = prepared.chunk();
        let min_y = prepared.min_y();
        let height = prepared.height();
        let max_y = min_y
            .checked_add(height)
            .context("BIOMES height overflows")?;
        ensure!(height > 0, "BIOMES requires a positive level height");
        let min_section = min_y.div_euclid(16);
        let max_section = (max_y - 1).div_euclid(16) + 1;
        let storage_min_quart_y = min_section * 4;
        let storage_quart_height = (max_section - min_section) * 4;
        let min_quart_y = min_y >> 2;
        let quart_height = height >> 2;
        ensure!(
            quart_height > 0,
            "BIOMES accessor has no readable quart Y range"
        );
        let max_quart_y = min_quart_y + quart_height - 1;
        let length = usize::try_from(storage_quart_height)?
            .checked_mul(16)
            .context("BIOMES allocation overflows")?;
        // These are unresolved private slots, never a default biome image.
        let mut slots = vec![None; length];
        let base_x = chunk_x.wrapping_shl(2);
        let base_z = chunk_z.wrapping_shl(2);
        for section_y in min_section..max_section {
            for local_x in 0..4 {
                for local_y in 0..4 {
                    for local_z in 0..4 {
                        let x = base_x.wrapping_add(local_x);
                        let y = section_y * 4 + local_y;
                        let z = base_z.wrapping_add(local_z);
                        let biome = resolver.get_noise_biome(x, y, z, prepared)?;
                        ensure!(
                            !biome.is_empty(),
                            "resolver returned an empty biome identity"
                        );
                        let index =
                            ((y - storage_min_quart_y) * 16 + local_z * 4 + local_x) as usize;
                        slots[index] = Some(biome);
                    }
                }
            }
        }
        let values = slots
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                value.with_context(|| format!("unresolved BIOMES palette cell {index}"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            chunk_x,
            chunk_z,
            min_y,
            height,
            min_quart_y,
            max_quart_y,
            storage_min_quart_y,
            storage_quart_height,
            values,
        })
    }
    pub fn chunk(&self) -> (i32, i32) {
        (self.chunk_x, self.chunk_z)
    }
    pub fn min_y(&self) -> i32 {
        self.min_y
    }
    pub fn height(&self) -> i32 {
        self.height
    }
    /// First stored quart Y: floorDiv(full level minimum Y, 16) * 4.
    pub fn storage_min_quart_y(&self) -> i32 {
        self.storage_min_quart_y
    }
    /// Four quart rows for every section in the full level accessor span.
    pub fn storage_quart_height(&self) -> i32 {
        self.storage_quart_height
    }
    /// Canonical layout: ascending stored quart Y, then local quart Z and X.
    /// Within one section the index is localY * 16 + localZ * 4 + localX.
    pub fn values(&self) -> &[String] {
        &self.values
    }
    pub fn owns_quart(&self, x: i32, z: i32) -> bool {
        x.div_euclid(4) == self.chunk_x && z.div_euclid(4) == self.chunk_z
    }

    /// Global ownership view: foreign X/Z is an error. Once the caller's chunk
    /// has been selected, Y follows ChunkAccess.getNoiseBiome's exact clamp.
    /// Raw ChunkAccess masks X/Z locally; that is not neighbor routing.
    pub fn get_noise_biome(&self, x: i32, y: i32, z: i32) -> Result<&str> {
        ensure!(
            self.owns_quart(x, z),
            "quart biome query belongs to another chunk"
        );
        let y = y.clamp(self.min_quart_y, self.max_quart_y);
        let index = ((y - self.storage_min_quart_y) * 16 + (z & 3) * 4 + (x & 3)) as usize;
        self.values
            .get(index)
            .map(String::as_str)
            .context("quart biome is outside stored sections")
    }

    /// Borrow this actual image and a mandatory actual neighbor provider. Only
    /// requested reads are checked; success does not certify scheduler radii or
    /// other generation-status dependencies for the whole SURFACE phase.
    pub fn neighborhood<'a, N: QuartBiomeProvider<Biome = String> + ?Sized>(
        &'a self,
        neighbors: &'a mut N,
    ) -> BiomeNeighborhood<'a, N> {
        BiomeNeighborhood {
            own: self,
            neighbors,
        }
    }
}
impl QuartBiomeProvider for BiomeImage {
    type Biome = String;
    type Error = anyhow::Error;
    fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        BiomeImage::get_noise_biome(self, x, y, z).map(str::to_owned)
    }
}
impl QuartBiomeProvider for &BiomeImage {
    type Biome = String;
    type Error = anyhow::Error;
    fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        BiomeImage::get_noise_biome(self, x, y, z).map(str::to_owned)
    }
}

/// The center image is read directly. Foreign X/Z goes unchanged to the actual
/// neighborhood provider, whose selected image applies its own full-height Y
/// clamp. There is no X/Z wrapping, adjacent-chunk fabrication or fallback.
pub struct BiomeNeighborhood<'a, N: QuartBiomeProvider<Biome = String> + ?Sized> {
    own: &'a BiomeImage,
    neighbors: &'a mut N,
}
impl<N: QuartBiomeProvider<Biome = String> + ?Sized> QuartBiomeProvider for BiomeNeighborhood<'_, N>
where
    N::Error: Into<anyhow::Error>,
{
    type Biome = String;
    type Error = anyhow::Error;
    fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        if self.own.owns_quart(x, z) {
            self.own.get_noise_biome(x, y, z).map(str::to_owned)
        } else {
            self.neighbors.get_noise_biome(x, y, z).map_err(Into::into)
        }
    }
}

/// Computed quart palettes coupled to the same prepared sampler/aquifer owner.
/// No original ProtoChunk highest-generated status or scheduler is represented.
pub struct PreparedBiomes {
    prepared: PreparedGeneration,
    biomes: BiomeImage,
}
impl PreparedBiomes {
    /// Consume one generation attempt. Resolver/RTree and density-cache effects
    /// are not rolled back on error; no partial image is returned and no retry
    /// is silently performed with a reconstructed or reset source cursor.
    pub fn fill<R: BiomeResolver + ?Sized>(
        mut prepared: PreparedGeneration,
        retrogen: RetrogenContext,
        resolver: &mut R,
    ) -> Result<Self> {
        ensure!(retrogen == RetrogenContext::ObservedAbsent,
            "BIOMES requires observed absent BelowZeroRetrogen; present or unavailable retrogen is unsupported");
        let biomes = BiomeImage::fill(&mut prepared, resolver)?;
        Ok(Self { prepared, biomes })
    }
    pub fn biomes(&self) -> &BiomeImage {
        &self.biomes
    }

    /// Continue the SAME prepared owner through its single material traversal.
    /// The actual quart image is moved with the resulting NOISE data.
    pub fn fill_noise(self) -> Result<BiomeTerrain> {
        let Self { prepared, biomes } = self;
        let terrain = prepared.fill_noise()?;
        Ok(BiomeTerrain { terrain, biomes })
    }
}

/// Native BIOMES data and NOISE/SURFACE terrain from one original prepared job.
/// Private fields prevent replacing only one half with an unrelated chunk.
pub struct BiomeTerrain {
    terrain: OwnedNoiseTerrain,
    biomes: BiomeImage,
}
impl BiomeTerrain {
    pub fn biomes(&self) -> &BiomeImage {
        &self.biomes
    }
    pub fn terrain(&self) -> &OwnedNoiseTerrain {
        &self.terrain
    }

    /// Apply SURFACE with the real zoom seed, actual neighbor BIOMES provider
    /// and persistent worker-local thermal owner. Full BIOMES-radius-1 and
    /// STRUCTURE_STARTS-radius-8/status prerequisites remain the scheduler's
    /// responsibility; successful queried reads are not that certification.
    pub fn build_surface<N: QuartBiomeProvider<Biome = String> + ?Sized>(
        &mut self,
        zoom: BiomeZoom,
        neighbors: &mut N,
        temperature: &mut BiomeTemperature,
    ) -> Result<SurfaceReport>
    where
        N::Error: Into<anyhow::Error>,
    {
        let mut provider = self.biomes.neighborhood(neighbors);
        let mut context = BuiltinBiomeSurfaceContext::new(zoom, &mut provider, temperature);
        self.terrain.build_surface(&mut context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{
        biome_source::MultiNoisePreset,
        generator::{FreshGenerationContext, GenerationInfluence, NoiseGenerator, TerrainStage},
    };
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
    };

    fn empty_context() -> FreshGenerationContext {
        FreshGenerationContext {
            initial_blocks: GenerationInfluence::ObservedEmpty,
            blender: GenerationInfluence::ObservedEmpty,
            structure_beardifier: GenerationInfluence::ObservedEmpty,
        }
    }
    fn prepared(
        generator: &Arc<NoiseGenerator>,
        chunk: (i32, i32),
        min_y: i32,
        height: i32,
    ) -> Result<PreparedGeneration> {
        generator.prepare_chunk(chunk.0, chunk.1, min_y, height, empty_context())
    }

    struct RecordingSource {
        source: BuiltinMultiNoiseBiomeSource,
        calls: Vec<((i32, i32, i32), String)>,
    }
    impl BiomeResolver for RecordingSource {
        fn get_noise_biome(
            &mut self,
            x: i32,
            y: i32,
            z: i32,
            sampler: &mut PreparedGeneration,
        ) -> Result<String> {
            let biome = self.source.get_noise_biome(x, y, z, sampler)?;
            self.calls.push(((x, y, z), biome.clone()));
            Ok(biome)
        }
    }

    /// Controlled coordinate labels isolate section/palette indexing. Every
    /// call samples the actual prepared owner, but labels are test metadata,
    /// not a registry or a natural-biome/production fallback implementation.
    #[derive(Default)]
    struct CoordinateTrace {
        calls: Vec<(i32, i32, i32)>,
    }
    impl BiomeResolver for CoordinateTrace {
        fn get_noise_biome(
            &mut self,
            x: i32,
            y: i32,
            z: i32,
            sampler: &mut PreparedGeneration,
        ) -> Result<String> {
            let _ = sampler.sample_climate(x, y, z)?;
            self.calls.push((x, y, z));
            Ok(format!("controlled:quart_{x}_{y}_{z}"))
        }
    }
    fn label(x: i32, y: i32, z: i32) -> String {
        format!("controlled:quart_{x}_{y}_{z}")
    }

    #[test]
    fn actual_source_callback_order_is_distinct_from_canonical_palette_layout() -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(123456789, "overworld")?);
        let mut resolver = RecordingSource {
            source: BuiltinMultiNoiseBiomeSource::new(MultiNoisePreset::Overworld)?,
            calls: Vec::new(),
        };
        let stage = PreparedBiomes::fill(
            prepared(&generator, (-640, 320), -64, 384)?,
            RetrogenContext::ObservedAbsent,
            &mut resolver,
        )?;
        let calls = &resolver.calls;
        assert_eq!(calls.len(), 24 * 64);
        assert_eq!(calls[0].0, (-2560, -16, 1280));
        assert_eq!(calls[1].0, (-2560, -16, 1281));
        assert_eq!(calls[4].0, (-2560, -15, 1280));
        assert_eq!(calls[16].0, (-2559, -16, 1280));
        assert_eq!(calls[63].0, (-2557, -13, 1283));
        assert_eq!(calls[64].0, (-2560, -12, 1280));
        let unique = calls
            .iter()
            .map(|(_, biome)| biome.as_str())
            .collect::<BTreeSet<_>>();
        assert!(
            unique.len() > 1,
            "selected actual source fixture must exercise varying palettes"
        );
        let by_point = calls
            .iter()
            .map(|(point, biome)| (*point, biome.as_str()))
            .collect::<BTreeMap<_, _>>();
        let image = stage.biomes();
        for (index, biome) in image.values().iter().enumerate() {
            let y = image.storage_min_quart_y() + (index / 16) as i32;
            let z = 1280 + ((index / 4) % 4) as i32;
            let x = -2560 + (index % 4) as i32;
            assert_eq!(Some(&biome.as_str()), by_point.get(&(x, y, z)));
            assert_eq!(image.get_noise_biome(x, y, z)?, biome);
        }
        Ok(())
    }

    #[test]
    fn full_level_sections_are_filled_without_using_the_noise_grid_height() -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(0, "nether")?);
        let mut resolver = CoordinateTrace::default();
        let stage = PreparedBiomes::fill(
            prepared(&generator, (0, 0), 0, 256)?,
            RetrogenContext::ObservedAbsent,
            &mut resolver,
        )?;
        assert_eq!(stage.biomes().storage_min_quart_y(), 0);
        assert_eq!(stage.biomes().storage_quart_height(), 64);
        assert_eq!(stage.biomes().values().len(), 16 * 64);
        assert!(resolver.calls.contains(&(0, 63, 0)));
        assert_eq!(stage.biomes().get_noise_biome(0, 1000, 0)?, label(0, 63, 0));
        let pointer = stage.biomes().values().as_ptr();
        let noise = stage.fill_noise()?;
        assert_eq!(noise.biomes().values().as_ptr(), pointer);
        assert_eq!(noise.terrain().height(), 256);
        assert_eq!(noise.terrain().stage(), TerrainStage::Noise);
        // Only the source settings' lower 128 blocks receive NOISE writes.
        assert_eq!(noise.terrain().block(0, 200, 0)?, 0);
        Ok(())
    }

    #[test]
    fn partial_sections_keep_full_storage_but_use_the_original_level_y_clamp() -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(0, "nether")?);
        let mut trace = CoordinateTrace::default();
        let stage = PreparedBiomes::fill(
            prepared(&generator, (0, 0), 0, 8)?,
            RetrogenContext::ObservedAbsent,
            &mut trace,
        )?;
        let image = stage.biomes();
        assert_eq!(image.storage_quart_height(), 4);
        assert_eq!(trace.calls.len(), 64);
        assert_eq!(image.values()[3 * 16], label(0, 3, 0));
        assert_eq!(image.get_noise_biome(0, i32::MIN, 0)?, label(0, 0, 0));
        assert_eq!(image.get_noise_biome(0, i32::MAX, 0)?, label(0, 1, 0));
        assert!(image.get_noise_biome(-1, 0, 0).is_err());
        assert!(image.get_noise_biome(4, 0, 0).is_err());
        let generator = Arc::new(NoiseGenerator::builtin(0, "overworld")?);
        let stage = PreparedBiomes::fill(
            prepared(&generator, (-1, -1), -66, 386)?,
            RetrogenContext::ObservedAbsent,
            &mut trace,
        )?;
        let image = stage.biomes();
        assert_eq!(image.storage_min_quart_y(), -20);
        assert_eq!(image.storage_quart_height(), 100);
        assert_eq!(image.get_noise_biome(-4, i32::MIN, -4)?, label(-4, -17, -4));
        assert_eq!(image.get_noise_biome(-4, i32::MAX, -4)?, label(-4, 78, -4));
        assert!(image.get_noise_biome(0, 0, -4).is_err());
        Ok(())
    }

    #[test]
    fn retrogen_and_resolver_failures_return_no_partial_biome_owner() -> Result<()> {
        let generator = Arc::new(NoiseGenerator::builtin(0, "nether")?);
        let mut trace = CoordinateTrace::default();
        for context in [RetrogenContext::Present, RetrogenContext::Unavailable] {
            assert!(PreparedBiomes::fill(
                prepared(&generator, (0, 0), 0, 16)?,
                context,
                &mut trace
            )
            .is_err());
            assert!(trace.calls.is_empty());
            assert_eq!(Arc::strong_count(&generator), 1);
        }
        struct MissingRegistry {
            calls: usize,
        }
        impl BiomeResolver for MissingRegistry {
            fn get_noise_biome(
                &mut self,
                x: i32,
                y: i32,
                z: i32,
                sampler: &mut PreparedGeneration,
            ) -> Result<String> {
                self.calls += 1;
                let _ = sampler.sample_climate(x, y, z)?;
                if self.calls == 3 {
                    return Err(std::io::Error::other("actual registry unavailable").into());
                }
                Ok(label(x, y, z))
            }
        }
        let mut missing = MissingRegistry { calls: 0 };
        let error = match PreparedBiomes::fill(
            prepared(&generator, (0, 0), 0, 16)?,
            RetrogenContext::ObservedAbsent,
            &mut missing,
        ) {
            Err(error) => error,
            Ok(_) => panic!("unavailable resolver must not publish a biome image"),
        };
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        assert_eq!(
            missing.calls, 3,
            "external resolver history is not rolled back/reset"
        );
        assert_eq!(Arc::strong_count(&generator), 1);
        Ok(())
    }

    /// Only images computed by PreparedBiomes are stored here. An absent map
    /// entry is an unavailable neighbor, never a generated/fixed fallback.
    struct ImageNeighbors<'a> {
        images: BTreeMap<(i32, i32), &'a BiomeImage>,
        calls: Vec<(i32, i32, i32)>,
    }
    impl QuartBiomeProvider for ImageNeighbors<'_> {
        type Biome = String;
        type Error = anyhow::Error;
        fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
            self.calls.push((x, y, z));
            let image = self
                .images
                .get(&(x.div_euclid(4), z.div_euclid(4)))
                .context("neighbor BIOMES unavailable")?;
            image.get_noise_biome(x, y, z).map(str::to_owned)
        }
    }

    #[test]
    fn neighborhood_routes_actual_neighbor_images_without_center_y_clamp_or_xz_wrap() -> Result<()>
    {
        let generator = Arc::new(NoiseGenerator::builtin(0, "nether")?);
        let mut trace = CoordinateTrace::default();
        let center = PreparedBiomes::fill(
            prepared(&generator, (0, 0), 0, 8)?,
            RetrogenContext::ObservedAbsent,
            &mut trace,
        )?;
        let right = PreparedBiomes::fill(
            prepared(&generator, (1, 0), 0, 256)?,
            RetrogenContext::ObservedAbsent,
            &mut trace,
        )?;
        let mut neighbors = ImageNeighbors {
            images: BTreeMap::from([((1, 0), right.biomes())]),
            calls: Vec::new(),
        };
        {
            let mut view = center.biomes().neighborhood(&mut neighbors);
            assert_eq!(view.get_noise_biome(0, 55, 0)?, label(0, 1, 0));
            assert_eq!(view.get_noise_biome(4, 55, 0)?, label(4, 55, 0));
            assert!(view.get_noise_biome(-1, i32::MIN, 0).is_err());
        }
        assert_eq!(neighbors.calls, [(4, 55, 0), (-1, i32::MIN, 0)]);
        Ok(())
    }

    #[test]
    fn source_selected_biomes_move_into_noise_and_use_actual_surface_neighbors() -> Result<()> {
        fn send_static<T: Send + 'static>() {}
        send_static::<PreparedBiomes>();
        send_static::<BiomeTerrain>();
        let generator = Arc::new(NoiseGenerator::builtin(0, "nether")?);
        let weak = Arc::downgrade(&generator);
        let mut source = BuiltinMultiNoiseBiomeSource::new(MultiNoisePreset::Nether)?;
        let mut jobs = BTreeMap::new();
        for x in -1..=1 {
            for z in -1..=1 {
                let stage = PreparedBiomes::fill(
                    prepared(&generator, (x, z), 0, 256)?,
                    RetrogenContext::ObservedAbsent,
                    &mut source,
                )?;
                jobs.insert((x, z), stage);
            }
        }
        let center = jobs.remove(&(0, 0)).unwrap();
        let palette_pointer = center.biomes().values().as_ptr();
        let mut terrain = center.fill_noise()?;
        assert_eq!(terrain.biomes().values().as_ptr(), palette_pointer);
        assert_eq!(terrain.biomes().chunk(), terrain.terrain().chunk());
        let before = terrain.terrain().states().to_vec();
        let before_surface = *terrain.terrain().world_surface_wg();
        let before_floor = *terrain.terrain().ocean_floor_wg();
        let before_posts = terrain.terrain().postprocessing().to_vec();
        let mut missing = ImageNeighbors {
            images: BTreeMap::new(),
            calls: Vec::new(),
        };
        let mut thermal = BiomeTemperature::builtin()?;
        assert!(terrain
            .build_surface(BiomeZoom::from_world_seed(0), &mut missing, &mut thermal)
            .is_err());
        assert!(!missing.calls.is_empty());
        assert_eq!(terrain.terrain().stage(), TerrainStage::Noise);
        assert_eq!(terrain.terrain().states(), before);
        assert_eq!(terrain.terrain().world_surface_wg(), &before_surface);
        assert_eq!(terrain.terrain().ocean_floor_wg(), &before_floor);
        assert_eq!(terrain.terrain().postprocessing(), before_posts);
        let mut neighbors = ImageNeighbors {
            images: jobs.iter().map(|(key, job)| (*key, job.biomes())).collect(),
            calls: Vec::new(),
        };
        let report =
            terrain.build_surface(BiomeZoom::from_world_seed(0), &mut neighbors, &mut thermal)?;
        assert!(report.rule_replacements > 0);
        assert!(!neighbors.calls.is_empty());
        assert_eq!(terrain.terrain().stage(), TerrainStage::Surface);
        assert!(terrain
            .build_surface(BiomeZoom::from_world_seed(0), &mut neighbors, &mut thermal)
            .is_err());
        drop(neighbors);
        drop(jobs);
        drop(generator);
        assert_eq!(weak.strong_count(), 1);
        drop(terrain);
        assert!(weak.upgrade().is_none());
        Ok(())
    }
}
