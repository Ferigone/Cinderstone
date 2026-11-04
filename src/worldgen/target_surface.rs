//! Live SURFACE adapter for the imported target owned by the generation session.
//!
//! This is a child of `generator` so it reuses its seeded SurfaceSystem and
//! canonical state metadata. The host's physical image and mutation prefix are
//! retained directly; there is no private replacement image or rollback here.

use super::{
    block_state_name, facts, ChunkDensity, DensityGraph, DensityPoint, NoiseGenerator, SurfaceHost,
    SurfaceReport,
};
use crate::worldgen::{
    biome_temperature::BiomeTemperature,
    biome_zoom::{BiomeZoom, QuartBiomeProvider},
    target::{GenerationTargetHost, HeightRange, NoiseHeightmap},
};
use anyhow::{ensure, Context, Result};

/// Apply the already seeded rule system to the actual target and density owner.
///
/// `region_height` is the real WorldGenRegion height accessor, rather than the
/// target's potentially narrower BelowZeroRetrogen generation accessor. The
/// caller owns phase prerequisites, the retained graph/cache identity, public
/// Blender recollection and poisoning this session on a dependency failure.
/// No replacement Blender is installed in the retained NoiseChunk.
pub(super) fn build_surface<H, N>(
    generator: &NoiseGenerator,
    density: &mut ChunkDensity,
    target: &mut H,
    region_height: HeightRange,
    zoom: BiomeZoom,
    neighbors: &mut N,
    temperature: &mut BiomeTemperature,
) -> Result<SurfaceReport>
where
    H: GenerationTargetHost + ?Sized,
    N: QuartBiomeProvider<Biome = String> + ?Sized,
    N::Error: Into<anyhow::Error>,
{
    let build_height = target.build_height();
    ensure!(build_height.height > 0, "empty target build-height range");
    let max_y = build_height
        .min_y
        .checked_add(build_height.height)
        .context("target build-height range overflows")?;
    ensure!(region_height.height > 0, "empty region height range");
    region_height
        .min_y
        .checked_add(region_height.height)
        .context("region height range overflows")?;
    let grid = generator.graph.grid().context("missing noise settings")?;
    let (chunk_x, chunk_z) = target.chunk_pos();
    let mut host = TargetSurfaceHost {
        target,
        density,
        graph: &generator.graph,
        neighbors,
        temperature,
        zoom,
        min_y: build_height.min_y,
        max_y,
        // WorldGenerationContext uses these independent max/min expressions;
        // gen_depth is not the height of the intersection of the two ranges.
        min_gen_y: region_height.min_y.max(grid.min_y),
        gen_depth: region_height.height.min(grid.height),
    };
    generator.surface.build_surface(
        &mut host,
        chunk_x,
        chunk_z,
        generator.graph.uses_legacy_random_source(),
    )
}

struct TargetSurfaceHost<'a, H: ?Sized, N: ?Sized> {
    target: &'a mut H,
    density: &'a mut ChunkDensity,
    graph: &'a DensityGraph,
    neighbors: &'a mut N,
    temperature: &'a mut BiomeTemperature,
    zoom: BiomeZoom,
    min_y: i32,
    max_y: i32,
    min_gen_y: i32,
    gen_depth: i32,
}

impl<H, N> SurfaceHost for TargetSurfaceHost<'_, H, N>
where
    H: GenerationTargetHost + ?Sized,
    N: QuartBiomeProvider<Biome = String> + ?Sized,
    N::Error: Into<anyhow::Error>,
{
    fn min_y(&self) -> i32 {
        self.min_y
    }

    fn max_y(&self) -> i32 {
        self.max_y
    }

    fn min_gen_y(&self) -> i32 {
        self.min_gen_y
    }

    fn gen_depth(&self) -> i32 {
        self.gen_depth
    }

    fn block(&mut self, x: i32, y: i32, z: i32) -> Result<u32> {
        // Reads use the actual physical chunk, including its outside-build
        // VOID_AIR behavior; retrogen's generation range does not clip reads.
        self.target.block(DensityPoint::new(x, y, z))
    }

    fn set_block(&mut self, x: i32, y: i32, z: i32, state: u32) -> Result<()> {
        // SurfaceSystem's BlockColumn asks for the CURRENT generation accessor
        // on every attempted write, before either Proto mutation or fluid post.
        let generation = self.target.generation_height()?;
        let max_y = generation
            .min_y
            .checked_add(generation.height)
            .context("target generation-height range overflows")?;
        if y < generation.min_y || y >= max_y {
            return Ok(());
        }
        let point = DensityPoint::new(x, y, z);
        self.target.proto_set_block(point, state)?;
        if facts().has_fluid(state) {
            self.target.mark_pos_for_postprocessing(point)?;
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

    fn world_surface_wg_height(&mut self, local_x: i32, local_z: i32) -> Result<i32> {
        self.target
            .wg_height(NoiseHeightmap::WorldSurfaceWg, local_x, local_z)
    }

    fn biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        let mut provider = TargetQuartProvider {
            target: &mut *self.target,
            neighbors: &mut *self.neighbors,
        };
        let supplied = self.zoom.get_biome(&mut provider, x, y, z)?;
        // A biome lookup validates only the selected name, without sampling
        // temperature or changing its worker-local packed-position cache.
        Ok(self.temperature.canonical_biome(&supplied)?.to_owned())
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

    fn preliminary_surface_level(&mut self, x: i32, z: i32) -> Result<i32> {
        Ok(self.density.preliminary_surface_level(self.graph, x, z)?)
    }
}

/// The source's center palette owner and mandatory foreign-quart provider.
/// Selecting a center coordinate never redirects through a raw biome source.
struct TargetQuartProvider<'a, H: ?Sized, N: ?Sized> {
    target: &'a mut H,
    neighbors: &'a mut N,
}

impl<H, N> QuartBiomeProvider for TargetQuartProvider<'_, H, N>
where
    H: GenerationTargetHost + ?Sized,
    N: QuartBiomeProvider<Biome = String> + ?Sized,
    N::Error: Into<anyhow::Error>,
{
    type Biome = String;
    type Error = anyhow::Error;

    fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        if (x >> 2, z >> 2) == self.target.chunk_pos() {
            self.target.get_noise_biome(x, y, z)
        } else {
            self.neighbors.get_noise_biome(x, y, z).map_err(Into::into)
        }
    }
}
