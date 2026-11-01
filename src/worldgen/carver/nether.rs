//! NetherWorldCarver's Cave geometry overrides.
//!
//! Its block replacement override belongs to the common WorldCarver kernel.
//! This module changes only the source cave bound, thickness and initial tunnel
//! vertical scale through the shared Cave implementation.

use super::{
    cave, config::CaveCarverConfig, host::CarverHost, mask::CarvingMask,
    world_carver::WorldCarverKind,
};
use crate::random::RandomSource;
use anyhow::Result;

pub fn is_start_chunk(config: &CaveCarverConfig, random: &mut impl RandomSource) -> bool {
    cave::is_start_chunk(config, random)
}

pub fn carve(
    host: &mut impl CarverHost,
    config: &CaveCarverConfig,
    random: &mut impl RandomSource,
    origin: (i32, i32),
    mask: &mut CarvingMask,
) -> Result<bool> {
    cave::carve_kind(host, config, mask, random, origin, WorldCarverKind::Nether)
}
