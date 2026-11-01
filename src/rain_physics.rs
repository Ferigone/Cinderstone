//! Original Level.isRainingAt and Entity.isInRain query order.
//!
//! A global rain flag alone does not establish exposure. Concrete providers
//! must supply actual sky, MOTION_BLOCKING and biome precipitation facts.
use crate::{
    entity_state::{java_floor, EntityPhysicalState},
    runtime_world::RuntimeWorld,
};
use anyhow::Result;

pub type BlockPosition = (i32, i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precipitation {
    None,
    Rain,
    Snow,
}

/// Required lower world queries. Missing light/chunk/biome information is an
/// error, never an invented false exposure result or a constant sky value.
pub trait RainWorld {
    fn is_raining(&mut self) -> Result<bool>;
    fn sky_brightness(&mut self, position: BlockPosition) -> Result<i32>;
    fn max_light_level(&mut self) -> Result<i32>;
    fn motion_blocking_heightmap_y(&mut self, position: BlockPosition) -> Result<i32>;
    fn precipitation_at(&mut self, position: BlockPosition) -> Result<Precipitation>;
}

/// BlockAndTintGetter.canSeeSky: brightness is read before the maximum level.
pub fn can_see_sky(world: &mut impl RainWorld, position: BlockPosition) -> Result<bool> {
    let brightness = world.sky_brightness(position)?;
    Ok(brightness >= world.max_light_level()?)
}

pub fn level_is_raining_at(world: &mut impl RainWorld, position: BlockPosition) -> Result<bool> {
    if !world.is_raining()? || !can_see_sky(world, position)? {
        return Ok(false);
    }
    if world.motion_blocking_heightmap_y(position)? > position.1 {
        return Ok(false);
    }
    Ok(world.precipitation_at(position)? == Precipitation::Rain)
}

/// Entity.isInRain uses its cached BlockPos and AABB.maxY. In particular the
/// second point does not use eye height or the current raw entity X/Z.
pub fn entity_is_in_rain(world: &mut impl RainWorld, state: &EntityPhysicalState) -> Result<bool> {
    if level_is_raining_at(world, state.block_position)? {
        return Ok(true);
    }
    let (x, _, z) = state.block_position;
    level_is_raining_at(world, (x, java_floor(state.bounds.max_y), z))
}

/// Additive RuntimeItemWorld capability. Implementations must validate the
/// actual supplied world/source scope at each query. This is called only when
/// actual global rain requires exposure; water short-circuits it first.
pub trait RainExposure {
    fn is_in_rain(&mut self, world: &mut RuntimeWorld, state: &EntityPhysicalState)
        -> Result<bool>;
}

#[path = "rain_fixture.rs"]
mod fixture;
pub use fixture::{FreshFlatPlainsRain, FreshFlatPlainsRainFrame, RainChunkAvailability};

#[cfg(test)]
#[path = "../tools/rain_physics_tests.rs"]
mod tests;
