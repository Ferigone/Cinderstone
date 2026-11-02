use super::{TargetDefaultStates, TargetStateFacts};
use crate::{
    chunk_readiness::ChunkStatus, density::DensityPoint, worldgen::retrogen::BelowZeroRetrogen,
};
use anyhow::{ensure, Context, Result};
use std::sync::Arc;

/// Logical build bounds, generation bounds and section-array coverage are
/// distinct source domains. A range does not certify a chunk's readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightRange {
    pub min_y: i32,
    pub height: i32,
}
impl HeightRange {
    pub fn new(min_y: i32, height: i32) -> Result<Self> {
        let range = Self { min_y, height };
        ensure!(height > 0, "height must be positive");
        range.max_y()?;
        Ok(range)
    }
    pub fn max_y(self) -> Result<i32> {
        self.min_y
            .checked_add(self.height)
            .context("height extent overflows")
    }
    pub fn contains(self, y: i32) -> bool {
        i64::from(y) >= i64::from(self.min_y)
            && i64::from(y) < i64::from(self.min_y) + i64::from(self.height)
    }
    pub fn min_section_y(self) -> i32 {
        self.min_y.div_euclid(16)
    }
    pub fn section_count(self) -> Result<usize> {
        Ok(usize::try_from(
            (self.max_y()?.wrapping_sub(1)).div_euclid(16) - self.min_section_y() + 1,
        )?)
    }
    pub fn physical_sections(self) -> Result<Self> {
        Self::new(
            self.min_section_y()
                .checked_mul(16)
                .context("section minimum overflows")?,
            i32::try_from(self.section_count()?)?
                .checked_mul(16)
                .context("section height overflows")?,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseHeightmap {
    OceanFloorWg,
    WorldSurfaceWg,
}

/// Required operations on the actual target. Implementations must retain the
/// same block/biome sections, preexisting maps and ordered per-section posts.
/// No method has a fallback implementation for unavailable owners.
pub trait GenerationTargetHost {
    /// Owned lease releases the actual acquired sections in Drop, including
    /// on failure. It must not borrow the target for the duration of a fill.
    type SectionLease;
    fn chunk_pos(&self) -> (i32, i32);
    fn build_height(&self) -> HeightRange;
    fn generation_height(&self) -> Result<HeightRange>;
    fn persisted_status(&self) -> ChunkStatus;
    fn highest_generated_status(&self) -> Result<ChunkStatus>;
    fn retrogen(&self) -> Result<Option<Arc<BelowZeroRetrogen>>>;
    fn state_facts(&self, state: u32) -> Result<TargetStateFacts>;
    fn default_states(&self) -> TargetDefaultStates;
    fn debug_void_terrain(&self) -> Result<bool>;

    /// Arguments are absolute section Y coordinates, descending inclusive.
    fn acquire_noise_sections(
        &mut self,
        top_section_y: i32,
        bottom_section_y: i32,
    ) -> Result<Self::SectionLease>;
    /// ChunkAccess.getOrCreateHeightmapUnprimed: preserve an existing map;
    /// create zero packed storage when absent, without scanning old blocks.
    fn get_or_create_wg_heightmap(&mut self, kind: NoiseHeightmap) -> Result<()>;
    /// Direct LevelChunkSection.setBlockState(..., false). Update section
    /// counts, but not heightmaps, posts, block entities, light or ticks.
    fn direct_section_set(
        &mut self,
        lease: &Self::SectionLease,
        point: DensityPoint,
        state: u32,
    ) -> Result<u32>;
    fn update_wg_heightmap(
        &mut self,
        kind: NoiseHeightmap,
        point: DensityPoint,
        state: u32,
    ) -> Result<bool>;
    fn mark_pos_for_postprocessing(&mut self, point: DensityPoint) -> Result<()>;

    /// ProtoChunk logical read: outside build Y returns VOID_AIR. X/Z are
    /// masked locally, including calls from source retrogen helpers.
    fn block(&self, point: DensityPoint) -> Result<u32>;
    /// Exact supported ProtoChunk.setBlockState(..., false), separate from
    /// direct NOISE section writes. Outside build Y returns VOID_AIR.
    fn proto_set_block(&mut self, point: DensityPoint, state: u32) -> Result<u32>;
    /// ChunkAccess.getHeight returns highest taken Y. Only an absent map is
    /// lazily primed; an existing unprimed map is not silently repaired.
    fn wg_height(&mut self, kind: NoiseHeightmap, local_x: i32, local_z: i32) -> Result<i32>;
    fn get_noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Result<String>;
    /// Actual Holder registry-key ownership, distinct from its display name.
    fn biome_registry_key(&self, identity: &str) -> Result<Option<String>>;
    /// Preserve the actual section allocation; commit its completed palette.
    fn commit_biome_section(&mut self, section_y: i32, biomes: Box<[String; 64]>) -> Result<()>;
}
