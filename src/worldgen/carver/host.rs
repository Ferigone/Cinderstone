//! The live owners required by the source carver material kernel.
use crate::density::DensityPoint;
use anyhow::Result;

/// A CARVERS-phase target with its actual chunk image, retained NoiseChunk and
/// aquifer. The configured carver owns replaceable registry membership; the
/// host supplies actual build/context bounds, state fluids and callback owners.
///
/// `set_block` performs the ProtoChunk write and live heightmap updates, but
/// MUST NOT mark fluids for postprocessing automatically: the carver specifies
/// that callback's original order. `top_material` uses the carver's actual
/// biome function and the same retained preliminary-surface owner.
///
/// Errors can follow a mask bit, block write or cache change. The orchestration
/// owner must consume or poison that job, not publish it or claim rollback of
/// the image alone restored all masks, aquifer flags and provider histories.
pub trait CarverHost {
    fn chunk_pos(&self) -> (i32, i32);
    fn min_gen_y(&self) -> i32;
    fn gen_depth(&self) -> i32;
    fn is_upgrading(&self) -> bool;
    fn block(&mut self, point: DensityPoint) -> Result<u32>;
    fn set_block(&mut self, point: DensityPoint, state: u32) -> Result<()>;
    fn has_fluid(&self, state: u32) -> Result<bool>;
    /// Same retained aquifer, density 0, original SinglePointContext for EVERY
    /// density query including its lazy barrier sample. No interpolation owner.
    fn aquifer_substance(&mut self, point: DensityPoint) -> Result<Option<u32>>;
    fn aquifer_should_schedule_fluid_update(&self) -> bool;
    fn top_material(&mut self, point: DensityPoint, has_fluid: bool) -> Result<Option<u32>>;
    fn mark_postprocessing(&mut self, point: DensityPoint) -> Result<()>;
}
