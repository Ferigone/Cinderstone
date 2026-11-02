//! Java 1.21 loot tables: pools, entries, number providers, conditions, item
//! functions and the context they run in (`commands_data.5`, partial).
//!
//! Source: the original `net.minecraft.world.level.storage.loot` classes of the
//! pinned 1.21 server (inner jar SHA-256 `8d47de9a…`), inspected through the
//! official mappings, and the `critereon` predicates they use. The vanilla data
//! pack ships 1178 tables; they use exactly 12 condition types, 19 function types
//! and 6 entry types. Those are implemented here and checked draw-for-draw
//! against the unchanged original classes (see `docs/reference/loot-tables.md`).
//!
//! **Not implemented, deliberately explicit** (`LootError::Unsupported`, never a
//! guessed substitute): scoreboard/command-storage values, `exploration_map`
//! structure search, NBT/custom-data copying, written books, fireworks, attribute
//! and container-content functions, the full `EntityPredicate` clause set
//! (distance, movement, effects, NBT, vehicle, passenger, team, slots, …),
//! `LocationPredicate` structures/light/fluid/position, and data-pack supplied
//! predicates. Hosts for those owners do not exist on master yet.
//!
//! Nothing in this module is called by Main. The caller (block-break, mob death,
//! chest opening, fishing, …) owns the world, entities, RNG and persistence and
//! supplies them through the traits in [`host`].

pub mod condition;
pub mod context;
pub mod enchant;
pub mod entry;
pub mod facts;
pub mod function;
pub mod host;
pub mod number;
pub mod parse;
pub mod predicate;
pub mod sequences;
pub mod vanilla;

use std::fmt;

pub use condition::Condition;
pub use context::{EntityTarget, LootContext, LootParams, Param, ParamSet};
pub use entry::{Entry, LootPool, LootTable};
pub use function::Function;
pub use host::{LootBlockEntity, LootDamageSource, LootEntity, LootLevel, LootResolver};
pub use number::{IntRange, LevelBasedValue, NumberProvider};

/// Failure of a loot operation. Variants mirror the *kind* of Java failure so a
/// caller can tell "outside the ported scope" from "Java would also throw".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootError {
    /// Behaviour outside the implemented, source-verified scope.
    Unsupported(String),
    /// JSON accepted by no Java codec path.
    Codec(String),
    /// A required loot parameter is absent (Java `NoSuchElementException`).
    MissingParam(&'static str),
    /// An argument Java rejects with an exception (for example `nextInt(0)`).
    Invalid(String),
    /// The host owner could not answer a query.
    Host(String),
}

pub type Result<T> = std::result::Result<T, LootError>;

impl fmt::Display for LootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported loot behaviour: {what}"),
            Self::Codec(what) => write!(f, "invalid loot JSON: {what}"),
            Self::MissingParam(name) => write!(f, "missing loot parameter {name}"),
            Self::Invalid(what) => write!(f, "invalid loot argument: {what}"),
            Self::Host(what) => write!(f, "loot host error: {what}"),
        }
    }
}

impl std::error::Error for LootError {}

pub(crate) fn unsupported<T>(what: impl Into<String>) -> Result<T> {
    Err(LootError::Unsupported(what.into()))
}

pub(crate) fn codec<T>(what: impl Into<String>) -> Result<T> {
    Err(LootError::Codec(what.into()))
}

impl From<crate::random::InvalidBound> for LootError {
    fn from(_: crate::random::InvalidBound) -> Self {
        Self::Invalid("Bound must be positive".to_owned())
    }
}

/// `net.minecraft.util.Mth` helpers with Java's exact float/int semantics.
pub(crate) mod mth {
    use crate::random::RandomSource;

    /// `Mth.floor(float)`.
    pub fn floor(value: f32) -> i32 {
        let truncated = value as i32;
        if value < truncated as f32 {
            truncated.wrapping_sub(1)
        } else {
            truncated
        }
    }

    /// `Math.round(float)`: floor(x + 1/2), saturating, NaN to zero.
    pub fn round(value: f32) -> i32 {
        if value.is_nan() {
            return 0;
        }
        (f64::from(value) + 0.5).floor() as i32
    }

    /// `Mth.clamp(int, int, int)`.
    pub fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
        if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        }
    }

    /// `Mth.clamp(float, float, float)`.
    pub fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
        if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        }
    }

    /// `Mth.nextInt(RandomSource, int, int)`.
    pub fn next_int(random: &mut dyn RandomSource, min: i32, max: i32) -> super::Result<i32> {
        if min >= max {
            Ok(min)
        } else {
            let bound = max.wrapping_sub(min).wrapping_add(1);
            Ok(random.next_bounded_i32(bound)?.wrapping_add(min))
        }
    }

    /// `Mth.nextFloat(RandomSource, float, float)`.
    pub fn next_float(random: &mut dyn RandomSource, min: f32, max: f32) -> f32 {
        if min >= max {
            min
        } else {
            random.next_f32() * (max - min) + min
        }
    }
}

#[cfg(test)]
mod tests;
