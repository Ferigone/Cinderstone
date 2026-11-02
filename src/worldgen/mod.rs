//! Native BIOMES, NOISE, SURFACE and AIR-carver data for pinned Java 1.21.
//!
//! These APIs return staged terrain, never a fabricated FULL/runtime chunk.
//! The integration coordinator must supply the actual world identity, biome
//! owner and generation-status dependencies before publishing their output.
pub mod aquifer;
pub mod biome_context;
pub mod biome_source;
pub mod biomes;
pub mod blending;
pub mod climate;
pub mod carver;
pub mod biome_temperature;
pub mod biome_zoom;
pub mod generator;
pub mod feature;
pub mod noise_chunk;
pub mod random_state;
pub mod surface;
// Keep the public API while avoiding build-output directory exclusion rules.
#[path = "chunk_target/mod.rs"]
pub mod target;
pub mod retrogen;

pub use generator::{
    resolve_block_state, BlendedGenerationContext, FreshGenerationContext, GenerationInfluence, NoiseFillOutcome,
    NoiseGenerator, NoiseTerrain, OwnedNoiseFillOutcome, OwnedNoiseTerrain, PreparedGeneration,
    SurfaceContext, TerrainStage,
    TargetClimateSampler, TargetGeneration, TargetGenerationPhase, TargetNoiseOutcome,
    TargetSurfaceOutcome,
};

pub use biome_context::BuiltinBiomeSurfaceContext;
pub use biome_temperature::BiomeTemperature;
pub use biome_zoom::{BiomeZoom, QuartBiomeProvider};

pub use biome_source::{
    select_end_biome, BuiltinMultiNoiseBiomeSource, ClimateSampler, EndQuartProvider, ErosionSampler,
    MultiNoisePreset, MultiNoiseQuartProvider,
};
pub use climate::{ClimateParameter, ClimateParameterList, ClimateParameterPoint, ClimateSearch};

pub use biomes::{
    BiomeImage, BiomeNeighborhood, BiomeResolver, BiomeTerrain, BuiltinBiomeResolver, PreparedBiomes,
    RetrogenContext,
};
