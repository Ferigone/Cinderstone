//! RandomState-owned factories and built-in named noise instances.
//!
//! These owners are derived from the world seed and the noise settings' random
//! algorithm. They never borrow or consume the gameplay Level random source.
//! Density router binding keeps its separate legacy climate substitutions.

use crate::{
    density::DensityGraph,
    noise::{NoiseParameters, NormalNoise},
    random::{LegacyRandomSource, PositionalRandomFactory, RandomSource, XoroshiroRandomSource},
};
use anyhow::{anyhow, bail, Context, Result};
use std::{collections::BTreeMap, sync::OnceLock};

/// The random owners belonging to one seeded noise-settings instance.
///
/// Positional factories are immutable seeds. Returning a copy does not advance
/// another consumer's stream; each `at` or `from_hash_of` creates its own source.
pub struct RandomStateRandoms {
    seed: i64,
    legacy: bool,
    positional: PositionalRandomFactory,
    aquifer: PositionalRandomFactory,
    ore: PositionalRandomFactory,
    noises: BTreeMap<String, NormalNoise>,
    positional_randoms: BTreeMap<String, PositionalRandomFactory>,
}

impl RandomStateRandoms {
    /// Mirrors RandomState's initial algorithm(seed).forkPositional().
    pub fn new(seed: i64, legacy: bool) -> Self {
        let positional = if legacy {
            PositionalRandomFactory::Legacy(LegacyRandomSource::new(seed).fork_positional())
        } else {
            PositionalRandomFactory::Xoroshiro(XoroshiroRandomSource::new(seed).fork_positional())
        };
        Self::with_factory(seed, legacy, positional)
    }

    /// Uses the exact immutable seed factory that already bound this router.
    pub fn from_graph(graph: &DensityGraph) -> Self {
        Self::with_factory(
            graph.seed(),
            graph.uses_legacy_random_source(),
            graph.positional_random_factory(),
        )
    }

    fn with_factory(seed: i64, legacy: bool, positional: PositionalRandomFactory) -> Self {
        // RandomState constructs these two independent hash sources before its
        // SurfaceSystem and router. Neither fork advances the root factory.
        let aquifer = positional
            .from_hash_of("minecraft:aquifer")
            .fork_positional();
        let ore = positional.from_hash_of("minecraft:ore").fork_positional();
        Self {
            seed,
            legacy,
            positional,
            aquifer,
            ore,
            noises: BTreeMap::new(),
            positional_randoms: BTreeMap::new(),
        }
    }

    pub fn seed(&self) -> i64 {
        self.seed
    }

    pub fn uses_legacy_random_source(&self) -> bool {
        self.legacy
    }

    pub fn positional_factory(&self) -> PositionalRandomFactory {
        self.positional
    }

    pub fn aquifer_random(&self) -> PositionalRandomFactory {
        self.aquifer
    }

    pub fn ore_random(&self) -> PositionalRandomFactory {
        self.ore
    }

    /// SurfaceSystem receives the original root factory, without another fork.
    /// Its clay bands use root.fromHashOf("minecraft:clay_bands") directly.
    pub fn surface_random(&self) -> PositionalRandomFactory {
        self.positional
    }

    /// RandomState.getOrCreateRandomFactory: hash the canonical location and
    /// fork the resulting sequential source once, caching the factory by key.
    pub fn get_or_create_random_factory(&mut self, name: &str) -> Result<PositionalRandomFactory> {
        let key = canonical_location(name)?;
        Ok(*self
            .positional_randoms
            .entry(key.clone())
            .or_insert_with(|| self.positional.from_hash_of(&key).fork_positional()))
    }

    /// RandomState.getOrCreateNoise/Noises.instantiate for the pinned built-ins.
    ///
    /// This always uses NormalNoise's modern constructor, including when the
    /// selected root factory is Legacy. RandomState's router visitor handles
    /// legacy temperature, vegetation and offset substitutions elsewhere.
    /// Dynamic registry overrides are unsupported by this built-in accessor.
    pub fn get_or_create_noise(&mut self, name: &str) -> Result<&NormalNoise> {
        let key = canonical_location(name)?;
        if !self.noises.contains_key(&key) {
            let parameters = builtin_parameters()?
                .get(&key)
                .cloned()
                .ok_or_else(|| anyhow!("unknown built-in noise parameters {key}"))?;
            let noise = NormalNoise::new(&mut self.positional.from_hash_of(&key), parameters);
            self.noises.insert(key.clone(), noise);
        }
        Ok(self.noises.get(&key).expect("named noise inserted above"))
    }
}

fn builtin_parameters() -> Result<&'static BTreeMap<String, NoiseParameters>> {
    static PARAMETERS: OnceLock<BTreeMap<String, NoiseParameters>> = OnceLock::new();
    if let Some(parameters) = PARAMETERS.get() {
        return Ok(parameters);
    }
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/noise_parameters-1.21.json"))?;
    let mut parameters = BTreeMap::new();
    for (name, value) in data["noises"]
        .as_object()
        .context("built-in noise parameters must be an object")?
    {
        let first_octave = value["first_octave"]
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .context("invalid built-in noise first octave")?;
        let amplitudes = value["amplitudes"]
            .as_array()
            .context("missing built-in noise amplitudes")?
            .iter()
            .map(|value| value.as_f64().context("invalid built-in noise amplitude"))
            .collect::<Result<Vec<_>>>()?;
        parameters.insert(
            name.clone(),
            NoiseParameters {
                first_octave,
                amplitudes,
            },
        );
    }
    // Racing first readers may parse the immutable packaged data twice; all
    // consumers still use the one published registry image.
    let _ = PARAMETERS.set(parameters);
    Ok(PARAMETERS
        .get()
        .expect("built-in noise registry initialized"))
}

fn canonical_location(name: &str) -> Result<String> {
    let (namespace, path) = match name.split_once(':') {
        Some(("", path)) => ("minecraft", path),
        Some(parts) => parts,
        None => ("minecraft", name),
    };
    if !namespace
        .bytes()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
        || !path
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"/._-".contains(&c))
    {
        bail!("invalid resource location {name:?}");
    }
    Ok(format!("{namespace}:{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values observed by reflection on the unmodified pinned 1.21 classes.
    /// See docs/reference/world-generation-random.md for the source and probe.
    #[test]
    fn owners_and_named_noise_match_original_random_state() {
        let cases: [(bool, i64, [i64; 5], u64); 6] = [
            (
                false,
                0,
                [
                    1304387774150406733,
                    5700463086516280346,
                    5425239508795256339,
                    5676388402950496251,
                    9033320074360651311,
                ],
                4578257928912495690,
            ),
            (
                false,
                123456789,
                [
                    -5310421403149361906,
                    1001923987239786307,
                    1262089544594960151,
                    154530420479643585,
                    -8927795029677759186,
                ],
                4594778221310875040,
            ),
            (
                false,
                -987654321,
                [
                    -5064218674613102254,
                    8105594289537676040,
                    -8357314478265351635,
                    2297948074235250021,
                    -8565934782939724010,
                ],
                13817490625186145424,
            ),
            (
                true,
                0,
                [
                    8244009244320322379,
                    2536321501272140556,
                    -1619033188633204071,
                    5604758905153221644,
                    6527578553016662450,
                ],
                13823424102576171313,
            ),
            (
                true,
                123456789,
                [
                    -6122145334885117312,
                    7937442417991274349,
                    -1545094964411766696,
                    639061394161206616,
                    2069125009880303052,
                ],
                4595424875162068182,
            ),
            (
                true,
                -987654321,
                [
                    8782623386521661745,
                    5258741500136428685,
                    -5164290900850364288,
                    1859783814560440307,
                    751844361114446719,
                ],
                13808501784789924260,
            ),
        ];
        for (legacy, seed, expected, surface_bits) in cases {
            let settings = if legacy { "nether" } else { "overworld" };
            let graph = DensityGraph::builtin(seed, settings).unwrap();
            let mut randoms = RandomStateRandoms::from_graph(&graph);
            assert_eq!(
                randoms.positional_factory(),
                RandomStateRandoms::new(seed, legacy).positional_factory()
            );
            let actual = [
                randoms.positional_factory().at(-17, -4, 23).next_i64(),
                randoms.aquifer_random().at(-17, -4, 23).next_i64(),
                randoms.ore_random().at(-17, -4, 23).next_i64(),
                randoms
                    .get_or_create_random_factory("bedrock_floor")
                    .unwrap()
                    .at(-17, -4, 23)
                    .next_i64(),
                randoms
                    .surface_random()
                    .from_hash_of("minecraft:clay_bands")
                    .next_i64(),
            ];
            assert_eq!(actual, expected, "{settings}, seed={seed}");
            let noise = randoms.get_or_create_noise("surface").unwrap();
            assert_eq!(
                noise.noise(-13.25, 0.0, 87.5).to_bits(),
                surface_bits,
                "{settings}, seed={seed}"
            );
            let identity = noise as *const NormalNoise;
            assert_eq!(
                identity,
                randoms.get_or_create_noise("minecraft:surface").unwrap() as *const NormalNoise
            );
            assert_eq!(randoms.noises.len(), 1);
        }
    }
}
