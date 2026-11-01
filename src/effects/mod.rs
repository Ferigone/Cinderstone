//! Source-backed status-effect instance lifecycle for Minecraft Java 1.21.
//!
//! This namespace has no actor scheduler or active-effect map. The actual
//! living actor must own those and implement every synchronous tick callback.
//! Definitions, attributes, packets, persistence and production Main wiring
//! remain separate integration responsibilities.

mod instance;

pub use instance::{EffectInstance, EffectTickHost};
