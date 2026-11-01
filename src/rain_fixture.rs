//! Source-validated, settled canonical native fixture rain capabilities.
use super::{entity_is_in_rain, BlockPosition, Precipitation, RainExposure, RainWorld};
use crate::{
    entity_state::{java_floor, EntityPhysicalState},
    runtime_world::RuntimeWorld,
};
use anyhow::{ensure, Result};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::OnceLock,
};

/// Current source capability, independent of storage/network cache presence.
pub trait RainChunkAvailability {
    /// Require the actual host's persistent fresh/settled source witness.
    /// An edit followed by restoration must not recreate this witness merely
    /// because sparse block diffs have become empty again.
    fn require_settled_unedited_fixture(&self) -> Result<()>;
    fn require_current_full(&self, chunk: (i32, i32)) -> Result<()>;
}
#[derive(serde::Deserialize)]
struct NoiseLevel {
    permutation: Vec<usize>,
}
#[derive(serde::Deserialize)]
struct TemperatureNoise {
    value_factor_bits: String,
    input_factor_bits: String,
    f2_bits: String,
    g2_bits: String,
    levels: Vec<NoiseLevel>,
    gradients: Vec<[i32; 3]>,
}
#[derive(serde::Deserialize)]
struct PlainsFacts {
    has_precipitation: bool,
    base_temperature_bits: String,
    modifier: String,
}
#[derive(serde::Deserialize)]
struct FixtureFacts {
    oracle_inner_sha256: String,
    base_state_ids: [u32; 4],
    plains_initial_temperature_cache_size: usize,
    plains: PlainsFacts,
    temperature_noise: TemperatureNoise,
    height_columns_x_z_highest_levelheight_lowestskysource: Vec<[i32; 5]>,
    #[serde(rename = "sky_rows_y_brightness_canSeeSky")]
    sky_rows: Vec<Vec<(i32, i32, bool)>>,
}
fn bits64(value: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(value, 16).expect("original f64 bits"))
}
fn bits32(value: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(value, 16).expect("original f32 bits"))
}
fn fixture_facts() -> &'static FixtureFacts {
    static FACTS: OnceLock<FixtureFacts> = OnceLock::new();
    FACTS.get_or_init(|| {
        let facts: FixtureFacts =
            serde_json::from_str(include_str!("../fixtures/rain-fixture-facts.json"))
                .expect("original genuine fixture rain facts");
        assert_eq!(
            facts.oracle_inner_sha256,
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        assert_eq!(facts.base_state_ids, [0, 1, 10, 9]);
        assert_eq!(facts.plains_initial_temperature_cache_size, 0);
        assert!(facts.plains.has_precipitation);
        assert_eq!(facts.plains.modifier, "NONE");
        assert_eq!(facts.temperature_noise.levels.len(), 1);
        assert_eq!(facts.temperature_noise.levels[0].permutation.len(), 512);
        assert_eq!(bits64(&facts.temperature_noise.input_factor_bits), 1.);
        assert_eq!(bits64(&facts.temperature_noise.value_factor_bits), 1.);
        assert_eq!(
            facts
                .height_columns_x_z_highest_levelheight_lowestskysource
                .len(),
            256
        );
        assert!(facts
            .height_columns_x_z_highest_levelheight_lowestskysource
            .iter()
            .all(|row| row[2..] == [63, 64, 64]));
        assert_eq!(facts.sky_rows.len(), 1);
        assert_eq!(facts.sky_rows[0].len(), 417);
        facts
    })
}
fn permutation(noise: &TemperatureNoise, index: i32) -> i32 {
    noise.levels[0].permutation[(index & 255) as usize] as i32
}
fn noise_corner(noise: &TemperatureNoise, gradient: i32, x: f64, y: f64) -> f64 {
    let mut attenuation = 0.5 - x * x - y * y - 0.0f64 * 0.0;
    if attenuation < 0.0 {
        return 0.0;
    }
    attenuation *= attenuation;
    let g = noise.gradients[gradient as usize];
    let dot = (f64::from(g[0]) * x + f64::from(g[1]) * y) + f64::from(g[2]) * 0.0;
    (attenuation * attenuation) * dot
}
fn temperature_noise(position: BlockPosition) -> f64 {
    let noise = &fixture_facts().temperature_noise;
    // Biome converts each integer through f32 division before widening.
    let x = f64::from(position.0 as f32 / 8.0);
    let y = f64::from(position.2 as f32 / 8.0);
    let skew = (x + y) * bits64(&noise.f2_bits);
    let i = java_floor(x + skew);
    let j = java_floor(y + skew);
    let g2 = bits64(&noise.g2_bits);
    let unskew = f64::from(i.wrapping_add(j)) * g2;
    let local_x = x - (f64::from(i) - unskew);
    let local_y = y - (f64::from(j) - unskew);
    let (step_x, step_y) = if local_x > local_y { (1, 0) } else { (0, 1) };
    let second_x = local_x - f64::from(step_x) + g2;
    let second_y = local_y - f64::from(step_y) + g2;
    let third_x = local_x - 1.0 + 2.0 * g2;
    let third_y = local_y - 1.0 + 2.0 * g2;
    let ii = i & 255;
    let jj = j & 255;
    let a = permutation(noise, ii + permutation(noise, jj)) % 12;
    let b = permutation(noise, ii + step_x + permutation(noise, jj + step_y)) % 12;
    let c = permutation(noise, ii + 1 + permutation(noise, jj + 1)) % 12;
    let a = noise_corner(noise, a, local_x, local_y);
    let b = noise_corner(noise, b, second_x, second_y);
    let c = noise_corner(noise, c, third_x, third_y);
    // PerlinSimplexNoise has the observed single non-null octave and factors1.
    0.0 + (70.0 * ((a + b) + c)) * 1.0
}
fn packed(position: BlockPosition) -> i64 {
    ((i64::from(position.0) & 0x3ff_ffff) << 38)
        | ((i64::from(position.2) & 0x3ff_ffff) << 12)
        | (i64::from(position.1) & 0xfff)
}
/// One actual plains temperature-cache owner. There is deliberately no Default:
/// imported/shared unknown cache prehistory cannot become an empty cache.
pub struct FreshFlatPlainsRain {
    temperatures: BTreeMap<i64, f32>,
    insertion: VecDeque<i64>,
}
impl FreshFlatPlainsRain {
    /// Observe a fresh native owner before any temperature query. The original
    /// bootstrapped plains cache was measured empty by the genuine oracle.
    pub fn from_observed_empty_temperature_history() -> Self {
        fixture_facts();
        Self {
            temperatures: BTreeMap::new(),
            insertion: VecDeque::new(),
        }
    }
    pub fn frame<'a>(
        &'a mut self,
        chunks: &'a dyn RainChunkAvailability,
    ) -> FreshFlatPlainsRainFrame<'a> {
        FreshFlatPlainsRainFrame {
            owner: self,
            chunks,
        }
    }
    /// Reuse this level owner's actual plains temperature-cache history when
    /// sky exposure and heightmaps are supplied by the live light engine.
    pub fn precipitation_at(&mut self, position: BlockPosition) -> Precipitation {
        if self.temperature(position) < 0.15 { Precipitation::Snow } else { Precipitation::Rain }
    }
    fn temperature(&mut self, position: BlockPosition) -> f32 {
        let key = packed(position);
        if let Some(&temperature) = self.temperatures.get(&key) {
            if !temperature.is_nan() {
                return temperature;
            }
        }
        let mut temperature = bits32(&fixture_facts().plains.base_temperature_bits);
        if position.1 > 80 {
            let noise = (temperature_noise(position) * 8.0) as f32;
            temperature -= (((noise + position.1 as f32) - 80.0) * 0.05) / 40.0;
        }
        if self.temperatures.len() == 1024 {
            let oldest = self
                .insertion
                .pop_front()
                .expect("actual cache insertion owner");
            self.temperatures.remove(&oldest);
        }
        if self.temperatures.insert(key, temperature).is_none() {
            self.insertion.push_back(key);
        }
        temperature
    }
}
pub struct FreshFlatPlainsRainFrame<'a> {
    owner: &'a mut FreshFlatPlainsRain,
    chunks: &'a dyn RainChunkAvailability,
}
struct FixtureRainQueries<'a> {
    owner: &'a mut FreshFlatPlainsRain,
    chunks: &'a dyn RainChunkAvailability,
}
impl FixtureRainQueries<'_> {
    fn require_point(&self, position: BlockPosition) -> Result<()> {
        ensure!(
            (-30_000_000..30_000_000).contains(&position.0)
                && (-30_000_000..30_000_000).contains(&position.2),
            "rain point is outside observed in-world horizontal source scope"
        );
        self.chunks
            .require_current_full((position.0.div_euclid(16), position.2.div_euclid(16)))
    }
}
impl RainWorld for FixtureRainQueries<'_> {
    fn is_raining(&mut self) -> Result<bool> {
        // RuntimeItemWorld calls this capability only for actual raining weather.
        Ok(true)
    }
    fn sky_brightness(&mut self, position: BlockPosition) -> Result<i32> {
        self.require_point(position)?;
        let rows = &fixture_facts().sky_rows[0];
        let index = position
            .1
            .checked_sub(rows[0].0)
            .and_then(|index| usize::try_from(index).ok());
        let row = index
            .and_then(|index| rows.get(index))
            .filter(|row| row.0 == position.1);
        Ok(row
            .ok_or_else(|| {
                anyhow::anyhow!("rain sky point is outside measured settled fixture Y window")
            })?
            .1)
    }
    fn max_light_level(&mut self) -> Result<i32> {
        Ok(15)
    }
    fn motion_blocking_heightmap_y(&mut self, position: BlockPosition) -> Result<i32> {
        self.require_point(position)?;
        Ok(fixture_facts().height_columns_x_z_highest_levelheight_lowestskysource[0][3])
    }
    fn precipitation_at(&mut self, position: BlockPosition) -> Result<Precipitation> {
        self.require_point(position)?;
        Ok(if self.owner.temperature(position) < 0.15 {
            Precipitation::Snow
        } else {
            Precipitation::Rain
        })
    }
}
impl RainExposure for FreshFlatPlainsRainFrame<'_> {
    fn is_in_rain(
        &mut self,
        world: &mut RuntimeWorld,
        state: &EntityPhysicalState,
    ) -> Result<bool> {
        self.chunks.require_settled_unedited_fixture()?;
        let RuntimeWorld::Fixture(flat) = world else {
            anyhow::bail!("rain source is not the observed fresh native fixture")
        };
        ensure!(
            flat.base_states() == fixture_facts().base_state_ids,
            "rain source has unknown base block states"
        );
        ensure!(
            flat.biome_id() == 39,
            "rain source biome is not actual canonical plains"
        );
        ensure!(
            flat.changes().is_empty(),
            "rain source has unsupported world edits"
        );
        let mut queries = FixtureRainQueries {
            owner: self.owner,
            chunks: self.chunks,
        };
        entity_is_in_rain(&mut queries, state)
    }
}

#[cfg(test)]
#[path = "../tools/rain_fixture_tests.rs"]
mod tests;
