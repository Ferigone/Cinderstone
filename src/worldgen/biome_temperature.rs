//! Original built-in biome temperatures and surface thermal predicates.
//!
//! The immutable noises use Biome's three fixed legacy seeds, never a world or
//! gameplay seed. One owner belongs to one generation worker and biome-registry
//! identity: its per-biome caches mirror that worker's original ThreadLocals.

use crate::{
    density::DensityPoint,
    noise::PerlinSimplexNoise,
    random::{LegacyRandomSource, WorldgenRandom},
};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    marker::PhantomData,
    rc::Rc,
    sync::OnceLock,
};

const CACHE_CAPACITY: usize = 1024;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TemperatureModifier {
    None,
    Frozen,
}
#[derive(Clone, Copy)]
struct BiomeClimate {
    temperature: f32,
    modifier: TemperatureModifier,
}
#[derive(Deserialize)]
struct ClimateRecord {
    temperature_bits: String,
    temperature_modifier: TemperatureModifier,
}
#[derive(Deserialize)]
struct ClimateFixture {
    biomes: BTreeMap<String, ClimateRecord>,
}
fn builtin_climates() -> Result<&'static BTreeMap<String, BiomeClimate>> {
    static CLIMATES: OnceLock<std::result::Result<BTreeMap<String, BiomeClimate>, String>> =
        OnceLock::new();
    CLIMATES
        .get_or_init(|| {
            let data: ClimateFixture = serde_json::from_str(include_str!(
                "../../fixtures/world-generation-biome-climate.json"
            ))
            .map_err(|error| error.to_string())?;
            data.biomes
                .into_iter()
                .map(|(name, row)| {
                    let bits = row
                        .temperature_bits
                        .parse::<u32>()
                        .map_err(|error| error.to_string())?;
                    Ok((
                        name,
                        BiomeClimate {
                            temperature: f32::from_bits(bits),
                            modifier: row.temperature_modifier,
                        },
                    ))
                })
                .collect()
        })
        .as_ref()
        .map_err(|error| anyhow!("invalid packaged biome climate data: {error}"))
}

struct TemperatureNoises {
    temperature: PerlinSimplexNoise,
    frozen_temperature: PerlinSimplexNoise,
    biome_info: PerlinSimplexNoise,
}
fn noises() -> &'static TemperatureNoises {
    static NOISES: OnceLock<TemperatureNoises> = OnceLock::new();
    NOISES.get_or_init(|| {
        // Original Biome.<clinit> constructs these independent streams in this
        // order, each through WorldgenRandom<LegacyRandomSource>.
        let temperature =
            PerlinSimplexNoise::new(&mut WorldgenRandom::new(LegacyRandomSource::new(1234)), [0])
                .expect("original temperature octave");
        let frozen_temperature = PerlinSimplexNoise::new(
            &mut WorldgenRandom::new(LegacyRandomSource::new(3456)),
            [-2, -1, 0],
        )
        .expect("original frozen-temperature octaves");
        let biome_info =
            PerlinSimplexNoise::new(&mut WorldgenRandom::new(LegacyRandomSource::new(2345)), [0])
                .expect("original biome-info octave");
        TemperatureNoises {
            temperature,
            frozen_temperature,
            biome_info,
        }
    })
}

#[derive(Default)]
struct TemperatureCache {
    values: HashMap<i64, f32>,
    insertion_order: VecDeque<i64>,
}
impl TemperatureCache {
    fn get(&self, key: i64) -> Option<f32> {
        self.values
            .get(&key)
            .copied()
            .filter(|value| !value.is_nan())
    }
    fn put(&mut self, key: i64, value: f32) {
        // Long2FloatLinkedOpenHashMap.get does not move an entry. Biome checks
        // size == 1024 before put, even when recomputing a cached NaN entry.
        if self.values.len() == CACHE_CAPACITY {
            let first = self.insertion_order.pop_front().expect("full cache");
            self.values.remove(&first);
        }
        if self.values.insert(key, value).is_none() {
            self.insertion_order.push_back(key);
        }
    }
}

/// Thermal owner for one worker using the immutable pinned built-in registry.
///
/// Reuse it across chunks on that worker. Create separate owners for different
/// workers or registry identities, as the original Biome has a ThreadLocal map.
/// This owner is intentionally neither Send nor Sync; the immutable noises and
/// climate definitions are shared, while mutable cache prehistory stays local.
/// Dynamic/custom biome overrides require another implementation and are rejected.
pub struct BiomeTemperature {
    climates: &'static BTreeMap<String, BiomeClimate>,
    caches: HashMap<&'static str, TemperatureCache>,
    worker_local: PhantomData<Rc<()>>,
}
impl BiomeTemperature {
    pub fn builtin() -> Result<Self> {
        let climates = builtin_climates()?;
        let _ = noises();
        Ok(Self {
            climates,
            caches: HashMap::new(),
            worker_local: PhantomData,
        })
    }

    /// Pure registry validation for BiomeManager's actual provider result.
    /// This performs no temperature/noise sample and does not touch a cache.
    pub fn canonical_biome(&self, name: &str) -> Result<&'static str> {
        let canonical = match name.split_once(':') {
            Some(("", path)) => format!("minecraft:{path}"),
            Some(_) => name.to_owned(),
            None => format!("minecraft:{name}"),
        };
        let climates = self.climates;
        climates
            .get_key_value(&canonical)
            .map(|(key, _)| key.as_str())
            .with_context(|| format!("unsupported biome climate {canonical}"))
    }

    /// Biome.getTemperature, including its packed-BlockPos key, NaN miss marker,
    /// 1024-entry FIFO eviction and absence of read-touch reordering.
    pub fn temperature(&mut self, biome: &str, point: DensityPoint) -> Result<f32> {
        let biome = self.canonical_biome(biome)?;
        let climate = self.climates[biome];
        let key = packed_position(point);
        let cache = self.caches.entry(biome).or_default();
        if let Some(value) = cache.get(key) {
            return Ok(value);
        }
        let value = height_adjusted_temperature(climate, point);
        cache.put(key, value);
        Ok(value)
    }

    /// The source is !warmEnoughToRain, preserving its NaN comparison behavior.
    pub fn cold_enough_to_snow(&mut self, biome: &str, point: DensityPoint) -> Result<bool> {
        Ok(!(self.temperature(biome, point)? >= 0.15_f32))
    }

    pub fn should_melt_frozen_ocean_iceberg_slightly(
        &mut self,
        biome: &str,
        point: DensityPoint,
    ) -> Result<bool> {
        Ok(self.temperature(biome, point)? > 0.1_f32)
    }
}

fn height_adjusted_temperature(climate: BiomeClimate, point: DensityPoint) -> f32 {
    let noises = noises();
    let mut temperature = climate.temperature;
    if let TemperatureModifier::Frozen = climate.modifier {
        let frozen = noises.frozen_temperature.noise(
            f64::from(point.x) * 0.05,
            f64::from(point.z) * 0.05,
            false,
        ) * 7.0;
        let info =
            noises
                .biome_info
                .noise(f64::from(point.x) * 0.2, f64::from(point.z) * 0.2, false);
        if frozen + info < 0.3
            && noises
                .biome_info
                .noise(f64::from(point.x) * 0.09, f64::from(point.z) * 0.09, false)
                < 0.8
        {
            temperature = 0.2_f32;
        }
    }
    if point.y > 80 {
        // i2f/fdiv/f2d occurs before noise sampling; dmul/d2f narrows the
        // noise-height offset before each subsequent float arithmetic step.
        let x = f64::from(point.x as f32 / 8.0_f32);
        let z = f64::from(point.z as f32 / 8.0_f32);
        let offset = (noises.temperature.noise(x, z, false) * 8.0) as f32;
        temperature - ((offset + point.y as f32 - 80.0_f32) * 0.05_f32) / 40.0_f32
    } else {
        temperature
    }
}
fn packed_position(point: DensityPoint) -> i64 {
    ((i64::from(point.x) & 0x3ff_ffff) << 38)
        | ((i64::from(point.z) & 0x3ff_ffff) << 12)
        | (i64::from(point.y) & 0xfff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Vectors {
        points: Vec<PointVector>,
        packed_alias_sequences: Vec<AliasSequence>,
        fifo_after_full_then_read_oldest_then_insert: CacheSnapshot,
    }
    #[derive(Deserialize)]
    struct PointVector {
        biome: String,
        point: [i32; 3],
        temperature_bits: String,
        cold_enough_to_snow: bool,
        should_melt_frozen_ocean_iceberg_slightly: bool,
    }
    #[derive(Deserialize)]
    struct AliasSequence {
        biome: String,
        rows: Vec<AliasRow>,
    }
    #[derive(Deserialize)]
    struct AliasRow {
        point: [i32; 3],
        temperature_bits: String,
    }
    #[derive(Deserialize)]
    struct CacheSnapshot {
        size: usize,
        first: String,
        last: String,
    }
    fn vectors() -> Result<Vectors> {
        Ok(serde_json::from_str(include_str!(
            "../../fixtures/world-generation-biome-temperature-vectors.json"
        ))?)
    }
    fn point(value: [i32; 3]) -> DensityPoint {
        DensityPoint::new(value[0], value[1], value[2])
    }

    #[test]
    fn actual_original_temperatures_and_surface_predicates() -> Result<()> {
        let mut owner = BiomeTemperature::builtin()?;
        for row in vectors()?.points {
            let point = point(row.point);
            assert_eq!(
                owner.temperature(&row.biome, point)?.to_bits(),
                row.temperature_bits.parse::<u32>()?,
                "{} at {point:?}",
                row.biome
            );
            assert_eq!(
                owner.cold_enough_to_snow(&row.biome, point)?,
                row.cold_enough_to_snow,
                "{} at {point:?}",
                row.biome
            );
            assert_eq!(
                owner.should_melt_frozen_ocean_iceberg_slightly(&row.biome, point)?,
                row.should_melt_frozen_ocean_iceberg_slightly,
                "{} at {point:?}",
                row.biome
            );
        }
        Ok(())
    }

    #[test]
    fn actual_original_packed_alias_prehistory_and_fifo_eviction() -> Result<()> {
        let vectors = vectors()?;
        for sequence in vectors.packed_alias_sequences {
            let mut owner = BiomeTemperature::builtin()?;
            for row in sequence.rows {
                assert_eq!(
                    owner
                        .temperature(&sequence.biome, point(row.point))?
                        .to_bits(),
                    row.temperature_bits.parse::<u32>()?,
                    "packed alias prehistory at {:?}",
                    row.point
                );
            }
        }
        let mut owner = BiomeTemperature::builtin()?;
        for x in 0..1024 {
            owner.temperature("plains", DensityPoint::new(x, 63, 0))?;
        }
        owner.temperature("plains", DensityPoint::new(0, 63, 0))?;
        owner.temperature("plains", DensityPoint::new(1024, 63, 0))?;
        let cache = &owner.caches["minecraft:plains"];
        let expected = vectors.fifo_after_full_then_read_oldest_then_insert;
        assert_eq!(cache.values.len(), expected.size);
        assert_eq!(
            *cache.insertion_order.front().unwrap(),
            expected.first.parse::<i64>()?
        );
        assert_eq!(
            *cache.insertion_order.back().unwrap(),
            expected.last.parse::<i64>()?
        );
        Ok(())
    }

    #[test]
    fn pure_canonical_validation_has_no_thermal_cache_side_effect() -> Result<()> {
        let owner = BiomeTemperature::builtin()?;
        for name in ["plains", ":plains", "minecraft:plains"] {
            assert_eq!(owner.canonical_biome(name)?, "minecraft:plains");
        }
        assert!(owner.canonical_biome("custom:plains").is_err());
        assert!(owner.canonical_biome("minecraft:absent_biome").is_err());
        assert!(owner.caches.is_empty());
        Ok(())
    }
}
