//! Built-in biome-source selection over mandatory retained climate owners.
//!
//! Parameter order is extracted from the original registry, never reconstructed
//! from biome names. The immutable RTree is shared; one worker keeps one search
//! cursor per preset/registry identity across chunk tasks. This selects names,
//! not BIOMES storage, generation status, blending/retrogen or spawn positions.

use super::{
    biome_zoom::QuartBiomeProvider,
    climate::{ClimateParameter, ClimateParameterList, ClimateParameterPoint, ClimateSearch},
    generator::{OwnedNoiseTerrain, PreparedGeneration},
};
use crate::density::{ClimateTargetPoint, DensityPoint};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

/// The actual chunk's wrapped Climate.Sampler. Missing owners must fail; a raw
/// DensityGraph, a fixed target or reconstructed sampler is not a substitute.
pub trait ClimateSampler {
    type Error;
    fn sample_climate(
        &mut self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
    ) -> std::result::Result<ClimateTargetPoint, Self::Error>;
}
impl ClimateSampler for PreparedGeneration {
    type Error = anyhow::Error;
    fn sample_climate(&mut self, x: i32, y: i32, z: i32) -> Result<ClimateTargetPoint> {
        PreparedGeneration::sample_climate(self, x, y, z)
    }
}
impl ClimateSampler for OwnedNoiseTerrain {
    type Error = anyhow::Error;
    fn sample_climate(&mut self, x: i32, y: i32, z: i32) -> Result<ClimateTargetPoint> {
        OwnedNoiseTerrain::sample_climate(self, x, y, z)
    }
}

/// Explicit registry preset; this is independent of noise settings and is not
/// inferred from a dimension name, terrain sample or world seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiNoisePreset {
    Overworld,
    Nether,
}
impl MultiNoisePreset {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Overworld => "minecraft:overworld",
            Self::Nether => "minecraft:nether",
        }
    }
}

#[derive(Deserialize)]
struct ParameterFixture {
    presets: BTreeMap<String, PresetRows>,
}
#[derive(Deserialize)]
struct PresetRows {
    entries: Vec<ParameterRow>,
}
#[derive(Deserialize)]
struct ParameterRow {
    parameters: [[i64; 2]; 7],
    biome: String,
}
fn builtin_lists() -> Result<&'static BTreeMap<String, ClimateParameterList<String>>> {
    static LISTS: OnceLock<
        std::result::Result<BTreeMap<String, ClimateParameterList<String>>, String>,
    > = OnceLock::new();
    LISTS
        .get_or_init(|| {
            let fixture: ParameterFixture = serde_json::from_str(include_str!(
                "../../fixtures/world-generation-multinoise-parameters.json"
            ))
            .map_err(|error| error.to_string())?;
            fixture
                .presets
                .into_iter()
                .map(|(name, rows)| {
                    let values = rows
                        .entries
                        .into_iter()
                        .map(|row| {
                            (
                                ClimateParameterPoint {
                                    parameters: row
                                        .parameters
                                        .map(|[min, max]| ClimateParameter { min, max }),
                                },
                                row.biome,
                            )
                        })
                        .collect();
                    ClimateParameterList::new(values)
                        .map(|list| (name, list))
                        .map_err(|error| error.to_string())
                })
                .collect()
        })
        .as_ref()
        .map_err(|error| anyhow!("invalid packaged multi-noise parameters: {error}"))
}

/// Persistent worker-local selection owner for one pinned built-in preset.
///
/// Keep this owner across chunks on that worker. Temporary provider views only
/// borrow the current chunk sampler, preserving this owner's previous leaf when
/// a chunk task ends. The cursor deliberately retains `!Send`/`!Sync` semantics.
/// Callers must certify the pinned registry contents; a matching name alone does
/// not prove that a datapack has not overridden that key.
pub struct BuiltinMultiNoiseBiomeSource {
    preset: MultiNoisePreset,
    search: ClimateSearch<String>,
}
impl BuiltinMultiNoiseBiomeSource {
    pub fn new(preset: MultiNoisePreset) -> Result<Self> {
        let list = builtin_lists()?
            .get(preset.name())
            .with_context(|| format!("missing built-in multi-noise preset {}", preset.name()))?;
        Ok(Self {
            preset,
            search: list.worker(),
        })
    }
    pub fn preset(&self) -> MultiNoisePreset {
        self.preset
    }

    /// Original MultiNoiseBiomeSource.getNoiseBiome: sample the supplied actual
    /// owner once, then perform the stateful RTree search on that exact target.
    pub fn get_noise_biome<S: ClimateSampler + ?Sized>(
        &mut self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
        sampler: &mut S,
    ) -> Result<String>
    where
        S::Error: Into<anyhow::Error>,
    {
        let target = sampler
            .sample_climate(quart_x, quart_y, quart_z)
            .map_err(Into::<anyhow::Error>::into)?;
        Ok(self.search.find_value(target).clone())
    }

    /// Borrow this persistent search owner and one current chunk sampler for
    /// BiomeZoom/BuiltinBiomeSurfaceContext; dropping the view drops no cursor.
    pub fn provider<'a, S: ClimateSampler + ?Sized>(
        &'a mut self,
        sampler: &'a mut S,
    ) -> MultiNoiseQuartProvider<'a, S> {
        MultiNoiseQuartProvider {
            source: self,
            sampler,
        }
    }
}

pub struct MultiNoiseQuartProvider<'a, S: ClimateSampler + ?Sized> {
    source: &'a mut BuiltinMultiNoiseBiomeSource,
    sampler: &'a mut S,
}
impl<S: ClimateSampler + ?Sized> QuartBiomeProvider for MultiNoiseQuartProvider<'_, S>
where
    S::Error: Into<anyhow::Error>,
{
    type Biome = String;
    type Error = anyhow::Error;
    fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        self.source.get_noise_biome(x, y, z, &mut *self.sampler)
    }
}

/// The End uses a raw erosion function from the same retained wrapped owner.
/// Quantized ClimateTargetPoint.erosion would change the original thresholds.
pub trait ErosionSampler {
    type Error;
    fn sample_erosion(&mut self, point: DensityPoint) -> std::result::Result<f64, Self::Error>;
}
impl ErosionSampler for PreparedGeneration {
    type Error = anyhow::Error;
    fn sample_erosion(&mut self, point: DensityPoint) -> Result<f64> {
        PreparedGeneration::sample_erosion(self, point)
    }
}
impl ErosionSampler for OwnedNoiseTerrain {
    type Error = anyhow::Error;
    fn sample_erosion(&mut self, point: DensityPoint) -> Result<f64> {
        OwnedNoiseTerrain::sample_erosion(self, point)
    }
}

/// TheEndBiomeSource.getNoiseBiome over the pinned built-in biome identities.
/// Coordinates retain the original int wrapping and arithmetic section shifts;
/// the central-island branch makes no erosion query, as in the original source.
pub fn select_end_biome<S: ErosionSampler + ?Sized>(
    quart_x: i32,
    quart_y: i32,
    quart_z: i32,
    sampler: &mut S,
) -> Result<&'static str>
where
    S::Error: Into<anyhow::Error>,
{
    let block_x = quart_x.wrapping_shl(2);
    let block_y = quart_y.wrapping_shl(2);
    let block_z = quart_z.wrapping_shl(2);
    let section_x = block_x >> 4;
    let section_z = block_z >> 4;
    if i64::from(section_x) * i64::from(section_x) + i64::from(section_z) * i64::from(section_z)
        <= 4096
    {
        return Ok("minecraft:the_end");
    }
    let point = DensityPoint::new(
        section_x.wrapping_mul(2).wrapping_add(1).wrapping_mul(8),
        block_y,
        section_z.wrapping_mul(2).wrapping_add(1).wrapping_mul(8),
    );
    let erosion = sampler
        .sample_erosion(point)
        .map_err(Into::<anyhow::Error>::into)?;
    // The source dcmpl/dcmpg branches send NaN through to end_barrens.
    Ok(if erosion > 0.25 {
        "minecraft:end_highlands"
    } else if erosion >= -0.0625 {
        "minecraft:end_midlands"
    } else if erosion < -0.21875 {
        "minecraft:small_end_islands"
    } else {
        "minecraft:end_barrens"
    })
}

/// Borrowed End source view for the actual raw erosion owner of a chunk task.
pub struct EndQuartProvider<'a, S: ErosionSampler + ?Sized> {
    sampler: &'a mut S,
}
impl<'a, S: ErosionSampler + ?Sized> EndQuartProvider<'a, S> {
    pub fn new(sampler: &'a mut S) -> Self {
        Self { sampler }
    }
}
impl<S: ErosionSampler + ?Sized> QuartBiomeProvider for EndQuartProvider<'_, S>
where
    S::Error: Into<anyhow::Error>,
{
    type Biome = String;
    type Error = anyhow::Error;
    fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        select_end_biome(x, y, z, &mut *self.sampler).map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::generator::{FreshGenerationContext, GenerationInfluence, NoiseGenerator};
    use std::sync::Arc;

    #[derive(Deserialize)]
    struct Vectors {
        sample_sequences: Vec<SampleSequence>,
        end_sequences: Vec<EndSequence>,
        end_thresholds: Vec<EndQuery>,
    }
    #[derive(Deserialize)]
    struct SampleSequence {
        settings: String,
        preset: String,
        seed: String,
        chunk: [i32; 2],
        queries: Vec<SampleQuery>,
    }
    #[derive(Debug, Deserialize)]
    struct SampleQuery {
        quart: [i32; 3],
        target: [i64; 6],
        biome: String,
    }
    #[derive(Deserialize)]
    struct EndSequence {
        settings: String,
        seed: String,
        chunk: [i32; 2],
        queries: Vec<EndQuery>,
    }
    #[derive(Deserialize)]
    struct EndQuery {
        quart: [i32; 3],
        biome: String,
        erosion_point: Option<[i32; 3]>,
        erosion_bits: Option<String>,
    }
    fn vectors() -> Result<Vectors> {
        Ok(serde_json::from_str(include_str!(
            "../../fixtures/world-generation-biome-source-vectors.json"
        ))?)
    }
    fn prepared(settings: &str, seed: &str, chunk: [i32; 2]) -> Result<PreparedGeneration> {
        let generator = Arc::new(NoiseGenerator::builtin(seed.parse()?, settings)?);
        let grid = generator.graph().grid().context("builtin noise grid")?;
        let observed = GenerationInfluence::ObservedEmpty;
        generator.prepare_chunk(
            chunk[0],
            chunk[1],
            grid.min_y,
            grid.height,
            FreshGenerationContext {
                initial_blocks: observed,
                blender: observed,
                structure_beardifier: observed,
            },
        )
    }
    struct RecordingClimate<'a, S> {
        owner: &'a mut S,
        calls: Vec<([i32; 3], ClimateTargetPoint)>,
    }
    impl<S: ClimateSampler> ClimateSampler for RecordingClimate<'_, S> {
        type Error = S::Error;
        fn sample_climate(
            &mut self,
            x: i32,
            y: i32,
            z: i32,
        ) -> std::result::Result<ClimateTargetPoint, Self::Error> {
            let target = self.owner.sample_climate(x, y, z)?;
            self.calls.push(([x, y, z], target));
            Ok(target)
        }
    }

    #[test]
    fn original_multinoise_choices_use_the_retained_wrapped_sampler_once() -> Result<()> {
        for sequence in vectors()?.sample_sequences {
            let preset = match sequence.preset.as_str() {
                "minecraft:overworld" => MultiNoisePreset::Overworld,
                "minecraft:nether" => MultiNoisePreset::Nether,
                _ => anyhow::bail!("unknown original preset"),
            };
            let mut source = BuiltinMultiNoiseBiomeSource::new(preset)?;
            let mut prepared = prepared(&sequence.settings, &sequence.seed, sequence.chunk)?;
            let mut witness = RecordingClimate {
                owner: &mut prepared,
                calls: Vec::new(),
            };
            for (index, query) in sequence.queries.iter().enumerate() {
                let [x, y, z] = query.quart;
                // New per-query views borrow the same persistent source cursor.
                let biome = source.provider(&mut witness).get_noise_biome(x, y, z)?;
                assert_eq!(biome, query.biome, "{} {query:?}", sequence.settings);
                assert_eq!(witness.calls.len(), index + 1);
                assert_eq!(witness.calls[index].0, query.quart);
                assert_eq!(
                    &witness.calls[index].1.parameter_array()[..6],
                    &query.target,
                    "original wrapped Climate.Sampler target"
                );
            }
            if sequence.settings == "minecraft:overworld" {
                // Changing chunk phase must not require replacing either this
                // persistent source or the actual chunk's retained noise owner.
                let mut terrain = prepared.fill_noise()?;
                let last = sequence.queries.last().context("original sample query")?;
                let [x, y, z] = last.quart;
                assert_eq!(source.get_noise_biome(x, y, z, &mut terrain)?, last.biome);
            }
        }
        Ok(())
    }

    struct RecordingErosion<'a, S> {
        owner: &'a mut S,
        calls: Vec<(DensityPoint, u64)>,
    }
    impl<S: ErosionSampler> ErosionSampler for RecordingErosion<'_, S> {
        type Error = S::Error;
        fn sample_erosion(&mut self, point: DensityPoint) -> std::result::Result<f64, Self::Error> {
            let value = self.owner.sample_erosion(point)?;
            self.calls.push((point, value.to_bits()));
            Ok(value)
        }
    }
    #[test]
    fn original_end_choices_keep_section_coordinates_and_raw_erosion() -> Result<()> {
        for sequence in vectors()?.end_sequences {
            let mut prepared = prepared(&sequence.settings, &sequence.seed, sequence.chunk)?;
            let mut witness = RecordingErosion {
                owner: &mut prepared,
                calls: Vec::new(),
            };
            for query in sequence.queries {
                let [x, y, z] = query.quart;
                let previous = witness.calls.len();
                assert_eq!(
                    EndQuartProvider::new(&mut witness).get_noise_biome(x, y, z)?,
                    query.biome
                );
                if let Some([x, y, z]) = query.erosion_point {
                    assert_eq!(witness.calls.len(), previous + 1);
                    let (point, bits) = witness.calls[previous];
                    assert_eq!(point, DensityPoint::new(x, y, z));
                    assert_eq!(
                        bits,
                        query
                            .erosion_bits
                            .context("original raw erosion")?
                            .parse::<u64>()?
                    );
                } else {
                    assert_eq!(
                        witness.calls.len(),
                        previous,
                        "central island samples no erosion"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn original_end_threshold_bits_and_nan_branches() -> Result<()> {
        // Controlled values executed by the unchanged original End source;
        // this witness tests its predicate and coordinates, not natural noise.
        struct ObservedErosion {
            value: f64,
            point: DensityPoint,
            calls: usize,
        }
        impl ErosionSampler for ObservedErosion {
            type Error = anyhow::Error;
            fn sample_erosion(&mut self, point: DensityPoint) -> Result<f64> {
                assert_eq!(point, self.point);
                self.calls += 1;
                Ok(self.value)
            }
        }
        for query in vectors()?.end_thresholds {
            let [x, y, z] = query
                .erosion_point
                .context("original threshold coordinate")?;
            let bits = query
                .erosion_bits
                .context("original threshold bits")?
                .parse::<u64>()?;
            let mut observed = ObservedErosion {
                value: f64::from_bits(bits),
                point: DensityPoint::new(x, y, z),
                calls: 0,
            };
            let [x, y, z] = query.quart;
            assert_eq!(
                select_end_biome(x, y, z, &mut observed)?,
                query.biome,
                "erosion bits {bits}"
            );
            assert_eq!(observed.calls, 1);
        }
        Ok(())
    }

    #[derive(Default)]
    struct MissingSampler {
        calls: usize,
    }
    impl ClimateSampler for MissingSampler {
        type Error = std::io::Error;
        fn sample_climate(
            &mut self,
            _: i32,
            _: i32,
            _: i32,
        ) -> std::io::Result<ClimateTargetPoint> {
            self.calls += 1;
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "actual climate owner unavailable",
            ))
        }
    }
    impl ErosionSampler for MissingSampler {
        type Error = std::io::Error;
        fn sample_erosion(&mut self, _: DensityPoint) -> std::io::Result<f64> {
            self.calls += 1;
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "actual erosion owner unavailable",
            ))
        }
    }
    #[test]
    fn missing_sampler_errors_propagate_without_a_replacement_biome() -> Result<()> {
        let mut source = BuiltinMultiNoiseBiomeSource::new(MultiNoisePreset::Nether)?;
        let mut missing = MissingSampler::default();
        let error = source
            .provider(&mut missing)
            .get_noise_biome(0, 0, 0)
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(missing.calls, 1);
        assert_eq!(
            select_end_biome(256, 0, 0, &mut missing)?,
            "minecraft:the_end"
        );
        assert_eq!(missing.calls, 1, "original central-island shortcut");
        let error = select_end_biome(260, 0, 0, &mut missing).unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(missing.calls, 2);
        Ok(())
    }
}
