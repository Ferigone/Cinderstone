//! Minecraft 1.21 aquifer decisions and per-chunk caches.
//!
//! The owner supplies the real global fluid picker, the wrapped router and the
//! chunk's preliminary surface query. A successful material decision does not
//! advance ChunkStatus or publish a chunk.

use crate::{
    density::DensityPoint,
    random::{PositionalRandomFactory, RandomSource},
};
use anyhow::{bail, Result};

/// Canonical built-in fluid/air materials. The generator resolves these to its
/// actual registry's states; `None` from the aquifer selects the default solid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AquiferBlock {
    Air,
    Water,
    Lava,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FluidStatus {
    pub level: i32,
    pub fluid: AquiferBlock,
}
impl FluidStatus {
    pub const fn new(level: i32, fluid: AquiferBlock) -> Self {
        Self { level, fluid }
    }
    pub fn at(self, y: i32) -> AquiferBlock {
        if y < self.level {
            self.fluid
        } else {
            AquiferBlock::Air
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AquiferNoise {
    Floodedness,
    Spread,
    Lava,
    Erosion,
    Depth,
}

/// `sample_at` is the original aquifer's SinglePointContext path. In contrast,
/// pressure samples `sample_barrier` with the current NoiseChunk context. Both
/// must use the same wrapped router that owns the terrain density.
pub trait AquiferContext {
    fn global_fluid(&mut self, point: DensityPoint) -> Result<FluidStatus>;
    fn preliminary_surface_level(&mut self, x: i32, z: i32) -> Result<i32>;
    fn sample_at(&mut self, noise: AquiferNoise, point: DensityPoint) -> Result<f64>;
    fn sample_barrier(&mut self, point: DensityPoint) -> Result<f64>;
}

pub enum Aquifer {
    Disabled,
    NoiseBased(NoiseBasedAquifer),
}
impl Aquifer {
    pub fn noise_based(
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        random: PositionalRandomFactory,
    ) -> Result<Self> {
        Ok(Self::NoiseBased(NoiseBasedAquifer::new(
            chunk_x, chunk_z, min_y, height, random,
        )?))
    }
    pub fn compute_substance(
        &mut self,
        point: DensityPoint,
        density: f64,
        context: &mut impl AquiferContext,
    ) -> Result<Option<AquiferBlock>> {
        match self {
            Self::Disabled => {
                if density > 0.0 {
                    Ok(None)
                } else {
                    Ok(Some(context.global_fluid(point)?.at(point.y)))
                }
            }
            Self::NoiseBased(aquifer) => aquifer.compute_substance(point, density, context),
        }
    }
    pub fn should_schedule_fluid_update(&self) -> bool {
        match self {
            Self::Disabled => false,
            Self::NoiseBased(a) => a.schedule_fluid_update,
        }
    }
}

/// Stateful owner of one chunk's aquifer centers and computed fluid statuses.
/// Cache dimensions and the twelve-center search follow NoiseBasedAquifer.
pub struct NoiseBasedAquifer {
    random: PositionalRandomFactory,
    min_grid_x: i32,
    min_grid_y: i32,
    min_grid_z: i32,
    grid_size_x: i32,
    grid_size_y: i32,
    grid_size_z: i32,
    locations: Vec<Option<i64>>,
    statuses: Vec<Option<FluidStatus>>,
    schedule_fluid_update: bool,
}
const WAY_BELOW_MIN_Y: i32 = -32_512; // DimensionType.MIN_Y (-2032) << 4.
const SURFACE_OFFSETS: [(i32, i32); 13] = [
    (0, 0),
    (-2, -1),
    (-1, -1),
    (0, -1),
    (1, -1),
    (-3, 0),
    (-2, 0),
    (-1, 0),
    (1, 0),
    (-2, 1),
    (-1, 1),
    (0, 1),
    (1, 1),
];

impl NoiseBasedAquifer {
    pub fn new(
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        random: PositionalRandomFactory,
    ) -> Result<Self> {
        if height <= 0 || min_y < -2032 || i64::from(min_y) + i64::from(height) > 2032 {
            bail!("aquifer height is outside the original dimension coordinate range");
        }
        let block_x = chunk_x
            .checked_mul(16)
            .ok_or_else(|| anyhow::anyhow!("aquifer chunk X overflows"))?;
        let block_z = chunk_z
            .checked_mul(16)
            .ok_or_else(|| anyhow::anyhow!("aquifer chunk Z overflows"))?;
        // BlockPos packs X/Z into 26 bits. Reject inaccessible source coordinates
        // rather than producing a different wrapped world outside that range.
        if !(-33_554_384..=33_554_383).contains(&block_x)
            || !(-33_554_384..=33_554_383).contains(&block_z)
        {
            bail!("aquifer chunk lies outside representable BlockPos coordinates");
        }
        let min_grid_x = block_x.div_euclid(16) - 1;
        let min_grid_z = block_z.div_euclid(16) - 1;
        let min_grid_y = min_y.div_euclid(12) - 1;
        let grid_size_x = (block_x + 15).div_euclid(16) + 1 - min_grid_x + 1;
        let grid_size_z = (block_z + 15).div_euclid(16) + 1 - min_grid_z + 1;
        let grid_size_y = (min_y + height).div_euclid(12) + 1 - min_grid_y + 1;
        let count = (grid_size_x * grid_size_y * grid_size_z) as usize;
        Ok(Self {
            random,
            min_grid_x,
            min_grid_y,
            min_grid_z,
            grid_size_x,
            grid_size_y,
            grid_size_z,
            locations: vec![None; count],
            statuses: vec![None; count],
            schedule_fluid_update: false,
        })
    }

    fn index(&self, x: i32, y: i32, z: i32) -> Result<usize> {
        let x = x - self.min_grid_x;
        let y = y - self.min_grid_y;
        let z = z - self.min_grid_z;
        if x < 0
            || x >= self.grid_size_x
            || y < 0
            || y >= self.grid_size_y
            || z < 0
            || z >= self.grid_size_z
        {
            bail!("aquifer sample outside its owned chunk grid");
        }
        Ok(((y * self.grid_size_z + z) * self.grid_size_x + x) as usize)
    }
    fn location(&mut self, x: i32, y: i32, z: i32) -> Result<i64> {
        let index = self.index(x, y, z)?;
        if let Some(location) = self.locations[index] {
            return Ok(location);
        }
        let mut random = self.random.at(x, y, z);
        let center = pack(DensityPoint::new(
            x * 16 + random.next_bounded_i32(10)?,
            y * 12 + random.next_bounded_i32(9)?,
            z * 16 + random.next_bounded_i32(10)?,
        ));
        self.locations[index] = Some(center);
        Ok(center)
    }
    fn status(&mut self, center: i64, context: &mut impl AquiferContext) -> Result<FluidStatus> {
        let point = unpack(center);
        let index = self.index(
            point.x.div_euclid(16),
            point.y.div_euclid(12),
            point.z.div_euclid(16),
        )?;
        if let Some(status) = self.statuses[index] {
            return Ok(status);
        }
        let status = self.compute_fluid(point, context)?;
        self.statuses[index] = Some(status);
        Ok(status)
    }
    pub fn should_schedule_fluid_update(&self) -> bool {
        self.schedule_fluid_update
    }

    pub fn compute_substance(
        &mut self,
        point: DensityPoint,
        density: f64,
        context: &mut impl AquiferContext,
    ) -> Result<Option<AquiferBlock>> {
        if density > 0.0 {
            self.schedule_fluid_update = false;
            return Ok(None);
        }
        if context.global_fluid(point)?.at(point.y) == AquiferBlock::Lava {
            self.schedule_fluid_update = false;
            return Ok(Some(AquiferBlock::Lava));
        }
        let gx = (point.x - 5).div_euclid(16);
        let gy = (point.y + 1).div_euclid(12);
        let gz = (point.z - 5).div_euclid(16);
        let mut distances = [i32::MAX; 3];
        let mut centers = [0_i64; 3];
        // Equal-distance insertion uses <=: changing this tie order changes the
        // selected fluid status at lattice boundaries.
        for dx in 0..=1 {
            for dy in -1..=1 {
                for dz in 0..=1 {
                    let center = self.location(gx + dx, gy + dy, gz + dz)?;
                    let c = unpack(center);
                    let x = c.x - point.x;
                    let y = c.y - point.y;
                    let z = c.z - point.z;
                    let distance = x * x + y * y + z * z;
                    if distance <= distances[0] {
                        distances[2] = distances[1];
                        distances[1] = distances[0];
                        distances[0] = distance;
                        centers[2] = centers[1];
                        centers[1] = centers[0];
                        centers[0] = center;
                    } else if distance <= distances[1] {
                        distances[2] = distances[1];
                        distances[1] = distance;
                        centers[2] = centers[1];
                        centers[1] = center;
                    } else if distance <= distances[2] {
                        distances[2] = distance;
                        centers[2] = center;
                    }
                }
            }
        }
        let first = self.status(centers[0], context)?;
        let similarity12 = similarity(distances[0], distances[1]);
        let material = first.at(point.y);
        if similarity12 <= 0.0 {
            self.schedule_fluid_update = similarity12 >= similarity(10 * 10, 12 * 12);
            return Ok(Some(material));
        }
        if material == AquiferBlock::Water
            && context
                .global_fluid(DensityPoint::new(point.x, point.y - 1, point.z))?
                .at(point.y - 1)
                == AquiferBlock::Lava
        {
            self.schedule_fluid_update = true;
            return Ok(Some(material));
        }
        // NaN is the original MutableDouble sentinel: a genuinely NaN barrier
        // sample remains eligible for another evaluation on the next pair.
        let mut barrier = f64::NAN;
        let second = self.status(centers[1], context)?;
        if density + similarity12 * pressure(point, first, second, &mut barrier, context)? > 0.0 {
            self.schedule_fluid_update = false;
            return Ok(None);
        }
        let third = self.status(centers[2], context)?;
        let similarity13 = similarity(distances[0], distances[2]);
        if similarity13 > 0.0
            && density
                + similarity12
                    * similarity13
                    * pressure(point, first, third, &mut barrier, context)?
                > 0.0
        {
            self.schedule_fluid_update = false;
            return Ok(None);
        }
        let similarity23 = similarity(distances[1], distances[2]);
        if similarity23 > 0.0
            && density
                + similarity12
                    * similarity23
                    * pressure(point, second, third, &mut barrier, context)?
                > 0.0
        {
            self.schedule_fluid_update = false;
            return Ok(None);
        }
        self.schedule_fluid_update = true;
        Ok(Some(material))
    }

    fn compute_fluid(
        &self,
        point: DensityPoint,
        context: &mut impl AquiferContext,
    ) -> Result<FluidStatus> {
        let global = context.global_fluid(point)?;
        let mut min_surface = i32::MAX;
        let top = point.y + 12;
        let bottom = point.y - 12;
        let mut surface_has_fluid = false;
        for (dx, dz) in SURFACE_OFFSETS {
            let x = point.x + dx * 16;
            let z = point.z + dz * 16;
            let surface = context.preliminary_surface_level(x, z)?;
            let surface_plus_8 = surface.wrapping_add(8);
            let center = dx == 0 && dz == 0;
            if center && bottom > surface_plus_8 {
                return Ok(global);
            }
            let above_surface = top > surface_plus_8;
            if above_surface || center {
                let nearby = context.global_fluid(DensityPoint::new(x, surface_plus_8, z))?;
                if nearby.at(surface_plus_8) != AquiferBlock::Air {
                    if center {
                        surface_has_fluid = true;
                    }
                    if above_surface {
                        return Ok(nearby);
                    }
                }
            }
            min_surface = min_surface.min(surface);
        }
        let level = self.surface_level(point, global, min_surface, surface_has_fluid, context)?;
        let mut fluid = global.fluid;
        if level <= -10 && level != WAY_BELOW_MIN_Y && global.fluid != AquiferBlock::Lava {
            let lava = context.sample_at(
                AquiferNoise::Lava,
                DensityPoint::new(
                    point.x.div_euclid(64),
                    point.y.div_euclid(40),
                    point.z.div_euclid(64),
                ),
            )?;
            if lava.abs() > 0.3 {
                fluid = AquiferBlock::Lava;
            }
        }
        Ok(FluidStatus::new(level, fluid))
    }
    fn surface_level(
        &self,
        point: DensityPoint,
        global: FluidStatus,
        min_surface: i32,
        surface_has_fluid: bool,
        context: &mut impl AquiferContext,
    ) -> Result<i32> {
        let erosion = context.sample_at(AquiferNoise::Erosion, point)?;
        // Original isDeepDarkRegion short-circuits depth if erosion is not low.
        let deep_dark = erosion < f64::from(-0.225_f32)
            && context.sample_at(AquiferNoise::Depth, point)? > f64::from(0.9_f32);
        let (dryness, floodedness) = if deep_dark {
            (-1.0, -1.0)
        } else {
            let difference = min_surface.wrapping_add(8).wrapping_sub(point.y);
            let near_surface = if surface_has_fluid {
                clamped_map(f64::from(difference), 0.0, 64.0, 1.0, 0.0)
            } else {
                0.0
            };
            let flooded = clamp(
                context.sample_at(AquiferNoise::Floodedness, point)?,
                -1.0,
                1.0,
            );
            let lower = map(near_surface, 1.0, 0.0, -0.3, 0.8);
            let upper = map(near_surface, 1.0, 0.0, -0.8, 0.4);
            (flooded - upper, flooded - lower)
        };
        if floodedness > 0.0 {
            return Ok(global.level);
        }
        if !(dryness > 0.0) {
            return Ok(WAY_BELOW_MIN_Y);
        }
        let spread = context.sample_at(
            AquiferNoise::Spread,
            DensityPoint::new(
                point.x.div_euclid(16),
                point.y.div_euclid(40),
                point.z.div_euclid(16),
            ),
        )? * 10.0;
        let quantized = quantize_spread(spread);
        Ok(min_surface.min(
            point
                .y
                .div_euclid(40)
                .wrapping_mul(40)
                .wrapping_add(20)
                .wrapping_add(quantized),
        ))
    }
}

// Mth.quantize(spread, 3) uses Mth.floor: narrow to int first, then
// conditionally decrement with Java int wrapping. floor() before narrowing
// differs for negative values beyond the int range, including -infinity.
fn quantize_spread(spread: f64) -> i32 {
    let scaled = spread / 3.0;
    let truncated = scaled as i32;
    let floor = if scaled < f64::from(truncated) {
        truncated.wrapping_sub(1)
    } else {
        truncated
    };
    floor.wrapping_mul(3)
}

fn similarity(first: i32, second: i32) -> f64 {
    1.0 - f64::from(second.wrapping_sub(first).wrapping_abs()) / 25.0
}
fn pressure(
    point: DensityPoint,
    first: FluidStatus,
    second: FluidStatus,
    barrier: &mut f64,
    context: &mut impl AquiferContext,
) -> Result<f64> {
    let a = first.at(point.y);
    let b = second.at(point.y);
    if (a == AquiferBlock::Lava && b == AquiferBlock::Water)
        || (a == AquiferBlock::Water && b == AquiferBlock::Lava)
    {
        return Ok(2.0);
    }
    let difference = first.level.wrapping_sub(second.level).wrapping_abs();
    if difference == 0 {
        return Ok(0.0);
    }
    let middle = 0.5 * f64::from(first.level.wrapping_add(second.level));
    let offset = f64::from(point.y) + 0.5 - middle;
    let edge = f64::from(difference) / 2.0 - offset.abs();
    let shape = if offset > 0.0 {
        let value = 0.0 + edge;
        if value > 0.0 {
            value / 1.5
        } else {
            value / 2.5
        }
    } else {
        let value = 3.0 + edge;
        if value > 0.0 {
            value / 3.0
        } else {
            value / 10.0
        }
    };
    let noise = if shape < -2.0 || shape > 2.0 {
        0.0
    } else {
        if barrier.is_nan() {
            *barrier = context.sample_barrier(point)?;
        }
        *barrier
    };
    Ok(2.0 * (noise + shape))
}
fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else {
        if value > max {
            max
        } else {
            value
        }
    }
}
fn map(value: f64, min: f64, max: f64, from: f64, to: f64) -> f64 {
    from + ((value - min) / (max - min)) * (to - from)
}
fn clamped_map(value: f64, min: f64, max: f64, from: f64, to: f64) -> f64 {
    let delta = (value - min) / (max - min);
    if delta < 0.0 {
        from
    } else if delta > 1.0 {
        to
    } else {
        from + delta * (to - from)
    }
}
fn pack(point: DensityPoint) -> i64 {
    (((i64::from(point.x) & 0x3ff_ffff) << 38)
        | ((i64::from(point.z) & 0x3ff_ffff) << 12)
        | (i64::from(point.y) & 0xfff)) as i64
}
fn unpack(value: i64) -> DensityPoint {
    DensityPoint::new(
        (value >> 38) as i32,
        ((value << 52) >> 52) as i32,
        ((value << 26) >> 38) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{density::{ChunkDensity, DensityGraph}, worldgen::random_state::RandomStateRandoms};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture { seed: i64, settings: String, chunk: [i32; 2], rows: Vec<Row> }
    #[derive(Deserialize)]
    struct Row { point: [i32; 3], density: f64, material: Option<String>, schedule_fluid_update: bool }
    struct OriginalSinglePointProbe<'a> { graph: &'a DensityGraph, chunk: ChunkDensity }
    impl AquiferContext for OriginalSinglePointProbe<'_> {
        fn global_fluid(&mut self, point: DensityPoint) -> Result<FluidStatus> {
            Ok(if point.y < -54 { FluidStatus::new(-54, AquiferBlock::Lava) }
               else { FluidStatus::new(63, AquiferBlock::Water) })
        }
        fn preliminary_surface_level(&mut self, x: i32, z: i32) -> Result<i32> {
            Ok(self.chunk.preliminary_surface_level(self.graph, x, z)?)
        }
        fn sample_at(&mut self, noise: AquiferNoise, point: DensityPoint) -> Result<f64> {
            let root = match noise {
                AquiferNoise::Floodedness => "fluid_level_floodedness",
                AquiferNoise::Spread => "fluid_level_spread", AquiferNoise::Lava => "lava",
                AquiferNoise::Erosion => "erosion", AquiferNoise::Depth => "depth",
            };
            Ok(self.chunk.sample_external(self.graph, self.graph.root(root).unwrap(), point)?)
        }
        fn sample_barrier(&mut self, point: DensityPoint) -> Result<f64> {
            // This fixture explicitly invokes the original with SinglePointContext.
            // Production passes the current NoiseChunk and uses sample() instead.
            Ok(self.chunk.sample_external(self.graph, self.graph.root("barrier").unwrap(), point)?)
        }
    }
    #[derive(Deserialize)]
    struct QuantizationFixture {
        rows: Vec<QuantizationRow>,
    }
    #[derive(Deserialize)]
    struct QuantizationRow {
        name: String,
        input_bits: String,
        quantized: i32,
    }
    #[test]
    fn spread_quantization_matches_original_java_narrowing_and_wrapping() -> Result<()> {
        let fixture: QuantizationFixture = serde_json::from_str(include_str!(
            "../../fixtures/world-generation-aquifer-quantization.json"
        ))?;
        for row in fixture.rows {
            let spread = f64::from_bits(row.input_bits.parse()?);
            assert_eq!(quantize_spread(spread), row.quantized, "{}", row.name);
        }
        Ok(())
    }
    #[test]
    fn actual_original_aquifer_decisions_and_update_flags() -> Result<()> {
        let fixture: Fixture = serde_json::from_str(include_str!("../../fixtures/world-generation-aquifer-vectors.json"))?;
        let graph = DensityGraph::builtin(fixture.seed, &fixture.settings)?;
        let grid = graph.grid().unwrap();
        let random = RandomStateRandoms::from_graph(&graph);
        let mut chunk = ChunkDensity::new(grid, fixture.chunk[0] * 16, fixture.chunk[1] * 16, (16 / grid.cell_width) as usize)?;
        chunk.wrap_router(&graph)?;
        let mut context = OriginalSinglePointProbe { graph: &graph, chunk };
        let mut aquifer = Aquifer::noise_based(fixture.chunk[0], fixture.chunk[1], grid.min_y, grid.height, random.aquifer_random())?;
        // Order matters: all calls share the original source object's two caches.
        for row in fixture.rows {
            let point = DensityPoint::new(row.point[0], row.point[1], row.point[2]);
            let expected = row.material.as_deref().map(|name| match name {
                "air" => AquiferBlock::Air, "water" => AquiferBlock::Water,
                "lava" => AquiferBlock::Lava, _ => panic!("unknown oracle material"),
            });
            assert_eq!(aquifer.compute_substance(point, row.density, &mut context)?, expected, "{point:?}, density {}", row.density);
            assert_eq!(aquifer.should_schedule_fluid_update(), row.schedule_fluid_update, "{point:?}");
        }
        Ok(())
    }
}
