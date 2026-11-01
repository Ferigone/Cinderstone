//! Compiled SurfaceRules. Pure noise/factory state is owned by RandomState;
//! mutable biome and column caches belong to one buildSurface invocation.
use super::{SurfaceHost, SurfaceSystem};
use crate::{
    noise::NormalNoise,
    random::{PositionalRandomFactory, RandomSource},
    worldgen::random_state::RandomStateRandoms,
};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::HashMap;

pub(super) enum Rule {
    State(u32),
    Bands,
    Sequence(Vec<Rule>),
    Test(Condition, Box<Rule>),
}
pub(super) enum Condition {
    Biome(Vec<String>),
    Noise {
        name: String,
        noise: NormalNoise,
        min: f64,
        max: f64,
    },
    Not(Box<Condition>),
    AbovePreliminary,
    Hole,
    Steep,
    Temperature,
    Stone {
        offset: i32,
        add_surface: bool,
        secondary: i32,
        ceiling: bool,
    },
    Water {
        offset: i32,
        multiplier: i32,
        add_stone: bool,
    },
    Y {
        anchor: Anchor,
        multiplier: i32,
        add_stone: bool,
    },
    Gradient {
        random: PositionalRandomFactory,
        lower: Anchor,
        upper: Anchor,
    },
}
pub(super) enum Anchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}
impl Anchor {
    fn parse(value: &Value) -> Result<Self> {
        let object = value
            .as_object()
            .context("vertical anchor must be an object")?;
        if object.len() != 1 {
            bail!("vertical anchor must contain one coordinate");
        }
        if value.get("absolute").is_some() {
            Ok(Self::Absolute(integer(value, "absolute")?))
        } else if value.get("above_bottom").is_some() {
            Ok(Self::AboveBottom(integer(value, "above_bottom")?))
        } else if value.get("below_top").is_some() {
            Ok(Self::BelowTop(integer(value, "below_top")?))
        } else {
            bail!("unsupported vertical anchor")
        }
    }
    fn resolve(&self, host: &impl SurfaceHost) -> i32 {
        match self {
            Self::Absolute(y) => *y,
            Self::AboveBottom(offset) => host.min_gen_y().wrapping_add(*offset),
            Self::BelowTop(offset) => host
                .min_gen_y()
                .wrapping_add(host.gen_depth())
                .wrapping_sub(1)
                .wrapping_sub(*offset),
        }
    }
}
fn integer(v: &Value, key: &str) -> Result<i32> {
    v[key]
        .as_i64()
        .with_context(|| format!("missing integer {key}"))?
        .try_into()
        .context("surface integer outside i32")
}
fn boolean(v: &Value, key: &str) -> Result<bool> {
    v[key]
        .as_bool()
        .with_context(|| format!("missing boolean {key}"))
}
fn number(v: &Value, key: &str) -> Result<f64> {
    v[key]
        .as_f64()
        .with_context(|| format!("missing number {key}"))
}
fn kind(v: &Value) -> Result<&str> {
    v["type"].as_str().context("missing surface type")
}
impl Rule {
    pub(super) fn parse(
        v: &Value,
        randoms: &mut RandomStateRandoms,
        resolve: &mut impl FnMut(&Value) -> Result<u32>,
    ) -> Result<Self> {
        Ok(match kind(v)? {
            "minecraft:block" => Self::State(resolve(&v["result_state"])?),
            "minecraft:bandlands" => Self::Bands,
            "minecraft:sequence" => {
                let sequence = v["sequence"]
                    .as_array()
                    .context("surface sequence must be array")?;
                if sequence.is_empty() {
                    bail!("surface sequence cannot be empty");
                }
                Self::Sequence(
                    sequence
                        .iter()
                        .map(|v| Self::parse(v, randoms, resolve))
                        .collect::<Result<_>>()?,
                )
            }
            "minecraft:condition" => Self::Test(
                Condition::parse(&v["if_true"], randoms)?,
                Box::new(Self::parse(&v["then_run"], randoms, resolve)?),
            ),
            other => bail!("unsupported surface rule {other}"),
        })
    }
    pub(super) fn apply(
        &self,
        c: &mut RuleContext<'_, impl SurfaceHost>,
        system: &SurfaceSystem,
    ) -> Result<Option<u32>> {
        match self {
            Self::State(state) => Ok(Some(*state)),
            Self::Bands => Ok(Some(system.band(c.x, c.y, c.z)?)),
            Self::Sequence(sequence) => {
                for rule in sequence {
                    if let Some(state) = rule.apply(c, system)? {
                        return Ok(Some(state));
                    }
                }
                Ok(None)
            }
            Self::Test(condition, followup) => {
                if condition.test(c)? {
                    followup.apply(c, system)
                } else {
                    Ok(None)
                }
            }
        }
    }
}
impl Condition {
    fn parse(v: &Value, randoms: &mut RandomStateRandoms) -> Result<Self> {
        Ok(match kind(v)? {
            "minecraft:biome" => Self::Biome(
                v["biome_is"]
                    .as_array()
                    .context("biome_is must be array")?
                    .iter()
                    .map(|v| v.as_str().map(str::to_owned).context("invalid biome ID"))
                    .collect::<Result<_>>()?,
            ),
            "minecraft:noise_threshold" => {
                let name = v["noise"]
                    .as_str()
                    .context("missing surface noise name")?
                    .to_owned();
                Self::Noise {
                    noise: randoms.get_or_create_noise(&name)?.clone(),
                    name,
                    min: number(v, "min_threshold")?,
                    max: number(v, "max_threshold")?,
                }
            }
            "minecraft:not" => Self::Not(Box::new(Self::parse(&v["invert"], randoms)?)),
            "minecraft:above_preliminary_surface" => Self::AbovePreliminary,
            "minecraft:hole" => Self::Hole,
            "minecraft:steep" => Self::Steep,
            "minecraft:temperature" => Self::Temperature,
            "minecraft:stone_depth" => {
                let ceiling = match v["surface_type"].as_str() {
                    Some("ceiling") => true,
                    Some("floor") => false,
                    _ => bail!("unknown cave surface type"),
                };
                Self::Stone {
                    offset: integer(v, "offset")?,
                    add_surface: boolean(v, "add_surface_depth")?,
                    secondary: integer(v, "secondary_depth_range")?,
                    ceiling,
                }
            }
            "minecraft:water" => Self::Water {
                offset: integer(v, "offset")?,
                multiplier: integer(v, "surface_depth_multiplier")?,
                add_stone: boolean(v, "add_stone_depth")?,
            },
            "minecraft:y_above" => Self::Y {
                anchor: Anchor::parse(&v["anchor"])?,
                multiplier: integer(v, "surface_depth_multiplier")?,
                add_stone: boolean(v, "add_stone_depth")?,
            },
            "minecraft:vertical_gradient" => Self::Gradient {
                random: randoms.get_or_create_random_factory(
                    v["random_name"]
                        .as_str()
                        .context("missing gradient random name")?,
                )?,
                lower: Anchor::parse(&v["true_at_and_below"])?,
                upper: Anchor::parse(&v["false_at_and_above"])?,
            },
            other => bail!("unsupported surface condition {other}"),
        })
    }
    fn test(&self, c: &mut RuleContext<'_, impl SurfaceHost>) -> Result<bool> {
        Ok(match self {
            Self::Biome(ids) => ids.contains(&c.biome()?.to_owned()),
            Self::Noise {
                name,
                noise,
                min,
                max,
            } => {
                let value = *c
                    .noises
                    .entry(name.clone())
                    .or_insert_with(|| noise.noise(c.x as f64, 0.0, c.z as f64));
                value >= *min && value <= *max
            }
            Self::Not(condition) => !condition.test(c)?,
            Self::AbovePreliminary => c.y >= c.min_surface_level()?,
            Self::Hole => c.depth <= 0,
            Self::Steep => c.steep()?,
            Self::Temperature => c.temperature()?,
            Self::Stone {
                offset,
                add_surface,
                secondary,
                ceiling,
            } => {
                let stone = if *ceiling {
                    c.stone_below
                } else {
                    c.stone_above
                };
                let surface = if *add_surface { c.depth } else { 0 };
                let secondary = if *secondary == 0 {
                    0
                } else {
                    let t = (c.secondary() - -1.0) / (1.0 - -1.0);
                    (t * *secondary as f64) as i32
                };
                stone
                    <= 1_i32
                        .wrapping_add(*offset)
                        .wrapping_add(surface)
                        .wrapping_add(secondary)
            }
            Self::Water {
                offset,
                multiplier,
                add_stone,
            } => {
                c.water_height == i32::MIN
                    || c.y.wrapping_add(if *add_stone { c.stone_above } else { 0 })
                        >= c.water_height
                            .wrapping_add(*offset)
                            .wrapping_add(c.depth.wrapping_mul(*multiplier))
            }
            Self::Y {
                anchor,
                multiplier,
                add_stone,
            } => {
                c.y.wrapping_add(if *add_stone { c.stone_above } else { 0 })
                    >= anchor
                        .resolve(c.host)
                        .wrapping_add(c.depth.wrapping_mul(*multiplier))
            }
            Self::Gradient {
                random,
                lower,
                upper,
            } => {
                let lower = lower.resolve(c.host);
                let upper = upper.resolve(c.host);
                if c.y <= lower {
                    true
                } else if c.y >= upper {
                    false
                } else {
                    let probability =
                        1.0 + ((c.y as f64 - lower as f64) / (upper as f64 - lower as f64)) * -1.0;
                    (random.at(c.x, c.y, c.z).next_f32() as f64) < probability
                }
            }
        })
    }
}

#[derive(Default)]
pub(super) struct PreliminarySurfaceCache {
    origin: Option<(i32, i32)>,
    corners: [i32; 4],
}

pub(super) struct RuleContext<'a, H: SurfaceHost> {
    pub(super) host: &'a mut H,
    system: &'a SurfaceSystem,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) z: i32,
    depth: i32,
    water_height: i32,
    stone_below: i32,
    stone_above: i32,
    biome: Option<String>,
    temperature: Option<bool>,
    steep: Option<bool>,
    secondary: Option<f64>,
    min_surface: Option<i32>,
    preliminary: &'a mut PreliminarySurfaceCache,
    noises: HashMap<String, f64>,
}
impl<'a, H: SurfaceHost> RuleContext<'a, H> {
    pub(super) fn new(
        host: &'a mut H,
        system: &'a SurfaceSystem,
        x: i32,
        z: i32,
        preliminary: &'a mut PreliminarySurfaceCache,
    ) -> Self {
        Self {
            host,
            system,
            x,
            z,
            y: 0,
            depth: system.surface_depth(x, z),
            water_height: i32::MIN,
            stone_below: 0,
            stone_above: 0,
            biome: None,
            temperature: None,
            steep: None,
            secondary: None,
            min_surface: None,
            preliminary,
            noises: HashMap::new(),
        }
    }
    pub(super) fn update_y(&mut self, y: i32, water: i32, below: i32, above: i32) {
        self.y = y;
        self.water_height = water;
        self.stone_below = below;
        self.stone_above = above;
        self.biome = None;
        self.temperature = None;
    }
    fn biome(&mut self) -> Result<&str> {
        if self.biome.is_none() {
            self.biome = Some(self.host.biome(self.x, self.y, self.z)?);
        }
        Ok(self.biome.as_deref().unwrap())
    }
    fn temperature(&mut self) -> Result<bool> {
        if let Some(value) = self.temperature {
            return Ok(value);
        }
        let biome = self.biome()?.to_owned();
        let value = self
            .host
            .cold_enough_to_snow(&biome, self.x, self.y, self.z)?;
        self.temperature = Some(value);
        Ok(value)
    }
    fn secondary(&mut self) -> f64 {
        *self
            .secondary
            .get_or_insert_with(|| self.system.surface_secondary(self.x, self.z))
    }
    fn steep(&mut self) -> Result<bool> {
        if let Some(value) = self.steep {
            return Ok(value);
        }
        let x = self.x & 15;
        let z = self.z & 15;
        let north = self.host.world_surface_wg_height(x, (z - 1).max(0))?;
        let south = self.host.world_surface_wg_height(x, (z + 1).min(15))?;
        let value = if south >= north.wrapping_add(4) {
            true
        } else {
            let west = self.host.world_surface_wg_height((x - 1).max(0), z)?;
            let east = self.host.world_surface_wg_height((x + 1).min(15), z)?;
            west >= east.wrapping_add(4)
        };
        self.steep = Some(value);
        Ok(value)
    }
    pub(super) fn min_surface_level(&mut self) -> Result<i32> {
        if let Some(value) = self.min_surface {
            return Ok(value);
        }
        let cell_x = self.x >> 4;
        let cell_z = self.z >> 4;
        let x0 = cell_x.wrapping_shl(4);
        let z0 = cell_z.wrapping_shl(4);
        if self.preliminary.origin != Some((cell_x, cell_z)) {
            let p00 = self.host.preliminary_surface_level(x0, z0)?;
            let p10 = self
                .host
                .preliminary_surface_level(x0.wrapping_add(16), z0)?;
            let p01 = self
                .host
                .preliminary_surface_level(x0, z0.wrapping_add(16))?;
            let p11 = self
                .host
                .preliminary_surface_level(x0.wrapping_add(16), z0.wrapping_add(16))?;
            self.preliminary.corners = [p00, p10, p01, p11];
            self.preliminary.origin = Some((cell_x, cell_z));
        }
        let [p00, p10, p01, p11] = self.preliminary.corners.map(|p| p as f64);
        let dx = (self.x & 15) as f32 / 16.0_f32;
        let dz = (self.z & 15) as f32 / 16.0_f32;
        let lower = p00 + dx as f64 * (p10 - p00);
        let upper = p01 + dx as f64 * (p11 - p01);
        let value = (lower + dz as f64 * (upper - lower)).floor() as i32;
        let value = value.wrapping_add(self.depth).wrapping_sub(8);
        self.min_surface = Some(value);
        Ok(value)
    }
}
