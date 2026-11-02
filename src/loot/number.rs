//! `NumberProvider`, `IntRange` and enchantment `LevelBasedValue`.
//!
//! Evaluation order of nested providers and the RNG draw that follows them is
//! the Java argument-evaluation order (`Mth.nextInt(random, min.getInt(), max.getInt())`
//! evaluates both bounds, which may draw, before the uniform draw itself).

use super::context::{EntityTarget, LootContext, Param};
use super::{mth, unsupported, LootError, Result};

/// `ScoreboardNameProvider`.
#[derive(Debug, Clone, PartialEq)]
pub enum ScoreboardName {
    /// `FixedScoreboardNameProvider`: a literal score holder name.
    Fixed(String),
    /// `ContextScoreboardNameProvider`: the scoreboard name of a context entity.
    Context(EntityTarget),
}

/// `NumberProvider`.
#[derive(Debug, Clone, PartialEq)]
pub enum NumberProvider {
    Constant(f32),
    Uniform(Box<NumberProvider>, Box<NumberProvider>),
    /// `n` rolls with probability `p`.
    Binomial(Box<NumberProvider>, Box<NumberProvider>),
    Score {
        target: ScoreboardName,
        score: String,
        scale: f32,
    },
    /// `StorageValue`: command storage has no native owner yet.
    Storage {
        storage: String,
        path: String,
    },
    EnchantmentLevel(LevelBasedValue),
}

impl NumberProvider {
    /// `NumberProvider.getFloat`.
    pub fn get_float(&self, ctx: &mut LootContext<'_>) -> Result<f32> {
        match self {
            Self::Constant(value) => Ok(*value),
            Self::Uniform(min, max) => {
                let min = min.get_float(ctx)?;
                let max = max.get_float(ctx)?;
                Ok(mth::next_float(ctx.random(), min, max))
            }
            Self::Binomial(_, _) => Ok(self.get_int(ctx)? as f32),
            Self::Score {
                target,
                score,
                scale,
            } => {
                let holder = match target {
                    ScoreboardName::Fixed(name) => Some(name.clone()),
                    ScoreboardName::Context(which) => match ctx.entity(*which) {
                        Some(entity) => Some(entity.scoreboard_name()?),
                        None => None,
                    },
                };
                let Some(holder) = holder else {
                    return Ok(0.0);
                };
                Ok(match ctx.level().score(&holder, score)? {
                    Some(value) => value as f32 * *scale,
                    None => 0.0,
                })
            }
            Self::Storage { .. } => unsupported("number provider minecraft:storage"),
            Self::EnchantmentLevel(amount) => {
                let level = ctx
                    .params
                    .enchantment_level
                    .ok_or(LootError::MissingParam(Param::EnchantmentLevel.name()))?;
                amount.calculate(level)
            }
        }
    }

    /// `NumberProvider.getInt`: `Math.round(getFloat)` unless overridden.
    pub fn get_int(&self, ctx: &mut LootContext<'_>) -> Result<i32> {
        match self {
            Self::Uniform(min, max) => {
                let min = min.get_int(ctx)?;
                let max = max.get_int(ctx)?;
                mth::next_int(ctx.random(), min, max)
            }
            Self::Binomial(n, p) => {
                let n = n.get_int(ctx)?;
                let p = p.get_float(ctx)?;
                let mut successes = 0;
                for _ in 0..n.max(0) {
                    if ctx.random().next_f32() < p {
                        successes += 1;
                    }
                }
                Ok(successes)
            }
            Self::Storage { .. } => unsupported("number provider minecraft:storage"),
            _ => Ok(mth::round(self.get_float(ctx)?)),
        }
    }
}

/// `IntRange`.
#[derive(Debug, Clone, PartialEq)]
pub struct IntRange {
    pub min: Option<NumberProvider>,
    pub max: Option<NumberProvider>,
}

impl IntRange {
    /// `IntRange.clamp`.
    pub fn clamp(&self, ctx: &mut LootContext<'_>, value: i32) -> Result<i32> {
        Ok(match (&self.min, &self.max) {
            (None, None) => value,
            (None, Some(max)) => max.get_int(ctx)?.min(value),
            (Some(min), None) => min.get_int(ctx)?.max(value),
            (Some(min), Some(max)) => {
                let min = min.get_int(ctx)?;
                let max = max.get_int(ctx)?;
                mth::clamp_i32(value, min, max)
            }
        })
    }

    /// `IntRange.test`, with Java's short-circuit order.
    pub fn test(&self, ctx: &mut LootContext<'_>, value: i32) -> Result<bool> {
        Ok(match (&self.min, &self.max) {
            (None, None) => true,
            (None, Some(max)) => value <= max.get_int(ctx)?,
            (Some(min), None) => value >= min.get_int(ctx)?,
            (Some(min), Some(max)) => {
                value >= min.get_int(ctx)? && value <= max.get_int(ctx)?
            }
        })
    }
}

/// `net.minecraft.world.item.enchantment.LevelBasedValue`.
#[derive(Debug, Clone, PartialEq)]
pub enum LevelBasedValue {
    Constant(f32),
    Clamped {
        value: Box<LevelBasedValue>,
        min: f32,
        max: f32,
    },
    Fraction {
        numerator: Box<LevelBasedValue>,
        denominator: Box<LevelBasedValue>,
    },
    LevelsSquared(f32),
    Linear {
        base: f32,
        per_level_above_first: f32,
    },
    Lookup {
        values: Vec<f32>,
        fallback: Box<LevelBasedValue>,
    },
}

impl LevelBasedValue {
    /// `LevelBasedValue.calculate(level)`.
    pub fn calculate(&self, level: i32) -> Result<f32> {
        Ok(match self {
            Self::Constant(value) => *value,
            Self::Clamped { value, min, max } => {
                mth::clamp_f32(value.calculate(level)?, *min, *max)
            }
            Self::Fraction {
                numerator,
                denominator,
            } => {
                let denominator = denominator.calculate(level)?;
                if denominator == 0.0 {
                    0.0
                } else {
                    numerator.calculate(level)? / denominator
                }
            }
            Self::LevelsSquared(added) => level.wrapping_mul(level) as f32 + added,
            Self::Linear {
                base,
                per_level_above_first,
            } => base + per_level_above_first * (level.wrapping_sub(1) as f32),
            Self::Lookup { values, fallback } => {
                if i64::from(level) <= values.len() as i64 {
                    let index = i64::from(level) - 1;
                    if index < 0 {
                        return Err(LootError::Invalid(format!(
                            "Index {index} out of bounds for length {}",
                            values.len()
                        )));
                    }
                    values[index as usize]
                } else {
                    fallback.calculate(level)?
                }
            }
        })
    }
}
