//! Live, section-backed targets for native generation. No type here publishes
//! a runtime chunk or changes a source generation status automatically.
mod facts;
mod host;
mod native;

pub use facts::{BuiltinStateFacts, SourceStateFacts, TargetDefaultStates, TargetStateFacts};
pub use host::{GenerationTargetHost, HeightRange, NoiseHeightmap};
pub use native::{
    NativeHeightmap, NativeProtoTarget, NativeSection, NativeTargetParts, NoiseSectionLease,
    SectionCounts,
};
