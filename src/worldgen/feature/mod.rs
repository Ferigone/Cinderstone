//! Configured ore leaves from the pinned Java 1.21 server.
//!
//! The caller owns the actual random stream, mutable neighboring chunks and
//! generation-region checks. These functions do not choose a feature seed,
//! apply placement modifiers or advance a chunk to FEATURES. In particular,
//! callers must preserve the different direct-section and region-write paths
//! required by the two ore implementations.

pub mod config;
pub mod host;
mod ore;
mod scattered_ore;

use crate::{density::DensityPoint, random::RandomSource};
use anyhow::Result;
pub use config::{builtin_configured_ore, ConfiguredOreFeature, OreConfiguration};
pub use host::OreHost;

/// The supported `ConfiguredFeature.place` boundary, using the caller's stream.
///
/// Configuration validation consumes no world or random observations. Once
/// validated, the source checks region write permission at the origin before
/// invoking either feature, including size-zero features. A false permission
/// result returns false without a random draw. Errors may follow mutations;
/// the owning region session must handle them without claiming image rollback
/// restored callback, random or neighboring-owner state.
pub fn place_configured_ore(
    configured: &ConfiguredOreFeature,
    host: &mut impl OreHost,
    random: &mut impl RandomSource,
    origin: DensityPoint,
) -> Result<bool> {
    configured.configuration().validate()?;
    if !host.ensure_can_write(origin)? {
        return Ok(false);
    }
    match configured {
        ConfiguredOreFeature::Ore(config) => ore::place(host, config, random, origin),
        ConfiguredOreFeature::ScatteredOre(config) => {
            scattered_ore::place(host, config, random, origin)
        }
    }
}

/// Resolve a certified pinned built-in configuration and place its ore leaf.
/// A matching resource name alone does not certify an unchanged registry;
/// dynamic registries and same-key overrides need a separate actual owner.
pub fn place_builtin_configured_ore(
    key: &str,
    host: &mut impl OreHost,
    random: &mut impl RandomSource,
    origin: DensityPoint,
) -> Result<bool> {
    place_configured_ore(builtin_configured_ore(key)?, host, random, origin)
}
