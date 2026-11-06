//! Advancement progress (WP-E5 core): requirements, per-criterion progress and the
//! `advancements/<uuid>.json` file. Nothing here is called by Main; the triggers that grant
//! progress are not ported. Evidence and limits: `docs/reference/advancements.md`.

pub mod progress;
pub mod requirements;
pub mod time;

#[cfg(test)]
mod tests;

pub use progress::{
    parse_progress_file, write_progress_file, AdvancementProgress, CriterionProgress, ProgressError,
};
pub use requirements::AdvancementRequirements;
