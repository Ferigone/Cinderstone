//! Source-backed status-effect instance lifecycle for Minecraft Java 1.21.
//!
//! This namespace supplies bounded source state and runners. The actual living
//! actor must attach its map and implement every synchronous tick callback;
//! this namespace creates no actor scheduler.
//! Definitions, attributes, packets, persistence and production Main wiring
//! remain separate integration responsibilities.

pub mod attribute_instance;
pub mod attribute_map;
mod instance;

pub use instance::{EffectInstance, EffectTickHost};

pub mod retained;
pub mod actor_ops;
pub mod attribute_transfer;
pub mod attribute_persistence;
pub mod tick;
pub mod callbacks;
pub mod refresh;
pub mod definitions;
pub mod supplier;
pub mod active_map;
pub mod binding;
pub mod default_attributes;
