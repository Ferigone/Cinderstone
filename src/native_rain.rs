//! Live canonical-plains rain exposure after native building/fluid edits.
//! Temperature queries retain the original persistent plains cache owner;
//! light and MOTION_BLOCKING height come from the completed actual source.
use crate::{
    entity_state::EntityPhysicalState,
    rain_physics::{
        entity_is_in_rain, BlockPosition, FreshFlatPlainsRain, Precipitation, RainExposure,
        RainWorld,
    },
    runtime_chunk_manager::RuntimeChunkManager,
    runtime_lighting::NativeRuntimeLighting,
    runtime_world::RuntimeWorld,
    world::FlatWorld,
};
use anyhow::{bail, ensure, Result};

pub struct NativeFlatPlainsRainFrame<'a> {
    temperatures: &'a mut FreshFlatPlainsRain,
    lighting: &'a mut NativeRuntimeLighting,
    chunks: &'a RuntimeChunkManager,
}
impl<'a> NativeFlatPlainsRainFrame<'a> {
    pub fn new(
        temperatures: &'a mut FreshFlatPlainsRain,
        lighting: &'a mut NativeRuntimeLighting,
        chunks: &'a RuntimeChunkManager,
    ) -> Self {
        Self {
            temperatures,
            lighting,
            chunks,
        }
    }
    /// Point query for actual fire/rain callbacks. The caller invokes this
    /// frame only after reading a true current Level.isRaining weather flag.
    pub fn is_raining_at(
        &mut self,
        world: &mut RuntimeWorld,
        position: BlockPosition,
    ) -> Result<bool> {
        let RuntimeWorld::Fixture(flat) = world else {
            bail!("native rain cannot certify imported world sources");
        };
        ensure!(
            flat.base_states() == [0, 1, 10, 9],
            "native rain base source is not canonical"
        );
        ensure!(
            flat.biome_id() == 39,
            "native rain source is not canonical plains"
        );
        ensure!(
            self.lighting.sky_enabled(),
            "plains rain requires the actual skylight dimension"
        );
        self.lighting.require_current(flat)?;
        let mut queries = LiveRainQueries {
            flat,
            temperatures: self.temperatures,
            lighting: self.lighting,
            chunks: self.chunks,
        };
        crate::rain_physics::level_is_raining_at(&mut queries, position)
    }
}
struct LiveRainQueries<'a> {
    flat: &'a FlatWorld,
    temperatures: &'a mut FreshFlatPlainsRain,
    lighting: &'a mut NativeRuntimeLighting,
    chunks: &'a RuntimeChunkManager,
}
impl LiveRainQueries<'_> {
    fn require_point(&self, position: BlockPosition) -> Result<()> {
        ensure!(
            (-30_000_000..30_000_000).contains(&position.0)
                && (-30_000_000..30_000_000).contains(&position.2),
            "native rain point is outside world coordinate scope"
        );
        // Ticket input can be pending while this exact FULL future and the
        // observed light/source remain current. Queries do not run executors.
        let chunk = (position.0.div_euclid(16), position.2.div_euclid(16));
        self.chunks.certify_full_region(chunk, chunk)
    }
}
impl RainWorld for LiveRainQueries<'_> {
    fn is_raining(&mut self) -> Result<bool> {
        // RuntimeItemWorld dispatches RainExposure only after reading actual
        // current rain weather and after the original water short circuit.
        Ok(true)
    }
    fn sky_brightness(&mut self, position: BlockPosition) -> Result<i32> {
        self.require_point(position)?;
        Ok(i32::from(self.lighting.light(self.flat, position)?.0))
    }
    fn max_light_level(&mut self) -> Result<i32> {
        Ok(15)
    }
    fn motion_blocking_heightmap_y(&mut self, position: BlockPosition) -> Result<i32> {
        self.require_point(position)?;
        self.lighting.motion_blocking_height(self.flat, position)
    }
    fn precipitation_at(&mut self, position: BlockPosition) -> Result<Precipitation> {
        self.require_point(position)?;
        Ok(self.temperatures.precipitation_at(position))
    }
}
impl RainExposure for NativeFlatPlainsRainFrame<'_> {
    fn is_in_rain(
        &mut self,
        world: &mut RuntimeWorld,
        state: &EntityPhysicalState,
    ) -> Result<bool> {
        let RuntimeWorld::Fixture(flat) = world else {
            bail!("native rain cannot certify imported world sources");
        };
        ensure!(
            flat.base_states() == [0, 1, 10, 9],
            "native rain base source is not canonical"
        );
        ensure!(
            flat.biome_id() == 39,
            "native rain source is not canonical plains"
        );
        ensure!(
            self.lighting.sky_enabled(),
            "plains rain requires the actual skylight dimension"
        );
        self.lighting.require_current(flat)?;
        let mut queries = LiveRainQueries {
            flat,
            temperatures: self.temperatures,
            lighting: self.lighting,
            chunks: self.chunks,
        };
        entity_is_in_rain(&mut queries, state)
    }
}
