//! Native NOISE and SURFACE generation stages for the pinned Java 1.21 data.
//!
//! These APIs return staged terrain, never a fabricated FULL/runtime chunk.
//! The integration coordinator must supply the actual world identity, biome
//! owner and generation-status dependencies before publishing their output.
pub mod aquifer;
pub mod biome_context;
pub mod biome_temperature;
pub mod biome_zoom;
pub mod generator;
pub mod noise_chunk;
pub mod random_state;
pub mod surface;

pub use generator::{
    resolve_block_state, FreshGenerationContext, GenerationInfluence, NoiseFillOutcome,
    NoiseGenerator, NoiseTerrain, OwnedNoiseFillOutcome, OwnedNoiseTerrain, SurfaceContext,
    TerrainStage,
};

pub use biome_context::BuiltinBiomeSurfaceContext;
pub use biome_temperature::BiomeTemperature;
pub use biome_zoom::{BiomeZoom, QuartBiomeProvider};
