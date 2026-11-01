//! Providers required by the pinned configured carvers. Sampling borrows the
//! caller's original random owner; providers never fork or reseed it.
use crate::random::RandomSource;
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;

/// The actual WorldGenerationContext values. The source computes these
/// independently as max(level min, generator min) and min(level height,
/// generator depth), rather than using the NOISE fill's intersection height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightContext {
    pub min_gen_y: i32,
    pub gen_depth: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}
impl VerticalAnchor {
    pub fn resolve(self, context: HeightContext) -> i32 {
        match self {
            Self::Absolute(y) => y,
            Self::AboveBottom(offset) => context.min_gen_y.wrapping_add(offset),
            Self::BelowTop(offset) => context
                .gen_depth
                .wrapping_sub(1)
                .wrapping_add(context.min_gen_y)
                .wrapping_sub(offset),
        }
    }

    pub(super) fn parse(value: &Value) -> Result<Self> {
        let object = value
            .as_object()
            .context("vertical anchor must be an object")?;
        ensure!(
            object.len() == 1,
            "vertical anchor must contain one coordinate"
        );
        let (kind, value) = object.iter().next().context("missing vertical anchor")?;
        let y = integer(value)?;
        // VerticalAnchor's codec uses DimensionType.MIN_Y/MAX_Y.
        ensure!(
            (-2032..=2031).contains(&y),
            "vertical anchor exceeds original codec bounds"
        );
        Ok(match kind.as_str() {
            "absolute" => Self::Absolute(y),
            "above_bottom" => Self::AboveBottom(y),
            "below_top" => Self::BelowTop(y),
            _ => bail!("unsupported vertical anchor {kind}"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightProvider {
    Constant(VerticalAnchor),
    Uniform {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
}
impl HeightProvider {
    pub fn sample(
        &self,
        random: &mut (impl RandomSource + ?Sized),
        context: HeightContext,
    ) -> Result<i32> {
        match *self {
            Self::Constant(anchor) => Ok(anchor.resolve(context)),
            Self::Uniform {
                min_inclusive,
                max_inclusive,
            } => {
                let min = min_inclusive.resolve(context);
                let max = max_inclusive.resolve(context);
                if min > max {
                    // UniformHeight returns its resolved minimum without a
                    // draw. Its warn-once logging cache has no sampling effect.
                    return Ok(min);
                }
                let bound = max.wrapping_sub(min).wrapping_add(1);
                Ok(random.next_bounded_i32(bound)?.wrapping_add(min))
            }
        }
    }

    pub(super) fn parse(value: &Value) -> Result<Self> {
        if value.get("type").is_none() {
            return VerticalAnchor::parse(value).map(Self::Constant);
        }
        Ok(match provider_kind(value)?.as_str() {
            "minecraft:constant" => Self::Constant(VerticalAnchor::parse(&value["value"])?),
            "minecraft:uniform" => Self::Uniform {
                min_inclusive: VerticalAnchor::parse(&value["min_inclusive"])?,
                max_inclusive: VerticalAnchor::parse(&value["max_inclusive"])?,
            },
            kind => bail!("unsupported carver height provider {kind}"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatProvider {
    Constant(f32),
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
    },
    Trapezoid {
        min: f32,
        max: f32,
        plateau: f32,
    },
}
impl FloatProvider {
    pub fn sample(&self, random: &mut (impl RandomSource + ?Sized)) -> f32 {
        match *self {
            Self::Constant(value) => value,
            Self::Uniform {
                min_inclusive,
                max_exclusive,
            } => {
                // Keep each original float operation and do not clamp the
                // rounded result or combine it with a fused multiply-add.
                random.next_f32() * (max_exclusive - min_inclusive) + min_inclusive
            }
            Self::Trapezoid { min, max, plateau } => {
                let range = max - min;
                let slope = (range - plateau) / 2.0_f32;
                let other = range - slope;
                let first = random.next_f32();
                let partial = min + first * other;
                let second = random.next_f32();
                partial + second * slope
            }
        }
    }

    pub fn min_value(&self) -> f32 {
        match *self {
            Self::Constant(value) => value,
            Self::Uniform { min_inclusive, .. } => min_inclusive,
            Self::Trapezoid { min, .. } => min,
        }
    }
    pub fn max_value(&self) -> f32 {
        match *self {
            Self::Constant(value) => value,
            Self::Uniform { max_exclusive, .. } => max_exclusive,
            Self::Trapezoid { max, .. } => max,
        }
    }

    pub(super) fn parse(value: &Value) -> Result<Self> {
        if value.is_number() {
            return number(value).map(Self::Constant);
        }
        Ok(match provider_kind(value)?.as_str() {
            "minecraft:constant" => Self::Constant(number(&value["value"])?),
            "minecraft:uniform" => {
                let min_inclusive = number(&value["min_inclusive"])?;
                let max_exclusive = number(&value["max_exclusive"])?;
                ensure!(
                    max_exclusive > min_inclusive,
                    "uniform float maximum must exceed minimum"
                );
                Self::Uniform {
                    min_inclusive,
                    max_exclusive,
                }
            }
            "minecraft:trapezoid" => {
                let min = number(&value["min"])?;
                let max = number(&value["max"])?;
                let plateau = number(&value["plateau"])?;
                ensure!(max >= min, "trapezoid maximum is below minimum");
                ensure!(plateau <= max - min, "trapezoid plateau exceeds range");
                Self::Trapezoid { min, max, plateau }
            }
            kind => bail!("unsupported carver float provider {kind}"),
        })
    }
}

pub(super) fn number(value: &Value) -> Result<f32> {
    let number = value.as_f64().context("missing carver float")? as f32;
    ensure!(number.is_finite(), "carver float is not finite");
    Ok(number)
}
pub(super) fn integer(value: &Value) -> Result<i32> {
    value
        .as_i64()
        .context("missing carver integer")?
        .try_into()
        .context("carver integer exceeds i32")
}
fn provider_kind(value: &Value) -> Result<String> {
    let name = value["type"].as_str().context("missing provider type")?;
    Ok(match name.split_once(':') {
        Some(("", path)) => format!("minecraft:{path}"),
        Some(_) => name.to_owned(),
        None => format!("minecraft:{name}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::LegacyRandomSource;

    #[test]
    fn original_provider_values_and_following_random_owner_match() -> Result<()> {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../fixtures/world-generation-configured-carvers.json"
        ))?;
        let rows = fixture["provider_vectors"]
            .as_array()
            .context("missing original provider vectors")?;
        ensure!(!rows.is_empty(), "empty original provider witness");
        for row in rows {
            let mut random = LegacyRandomSource::new(row["seed"].as_i64().context("missing seed")?);
            let samples = row["samples"]
                .as_array()
                .context("missing original samples")?;
            ensure!(!samples.is_empty(), "empty original provider sequence");
            match row["kind"].as_str().context("missing provider kind")? {
                "float" => {
                    let provider = FloatProvider::parse(&row["provider"])?;
                    for expected in samples {
                        assert_eq!(
                            u64::from(provider.sample(&mut random).to_bits()),
                            expected.as_u64().context("invalid original float bits")?
                        );
                    }
                }
                "height" => {
                    let provider = HeightProvider::parse(&row["provider"])?;
                    let context = HeightContext {
                        min_gen_y: integer(&row["min_gen_y"])?,
                        gen_depth: integer(&row["gen_depth"])?,
                    };
                    for expected in samples {
                        assert_eq!(
                            i64::from(provider.sample(&mut random, context)?),
                            expected.as_i64().context("invalid original height")?
                        );
                    }
                }
                kind => bail!("unsupported original provider witness {kind}"),
            }
            assert_eq!(
                random.next_i64(),
                row["next_i64"]
                    .as_i64()
                    .context("missing original next long")?
            );
        }
        Ok(())
    }
}
