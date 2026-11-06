//! Advancement progress (WP-E5 core): requirements, per-criterion progress and the
//! `advancements/<uuid>.json` file. Main uses [`player`] for the vanilla recipe advancements (#27);
//! the other trees are not loaded. Evidence and limits: `docs/reference/advancements.md`.

pub mod definition;
pub mod player;
pub mod tree;
pub mod triggers;
pub mod progress;
pub mod requirements;
pub mod time;

#[cfg(test)]
mod tests;

pub use progress::{
    parse_progress_file, write_progress_file, AdvancementProgress, CriterionProgress, ProgressError,
};
pub use requirements::AdvancementRequirements;
#[cfg(test)]
mod definition_tests;
#[cfg(test)]
mod player_tests;
