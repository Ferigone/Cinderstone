//! `net.minecraft.world.scores` and `ServerScoreboard` (WP-E4 stages 1–3): objectives, scores,
//! teams, display slots, number formats, the five scoreboard packets and `scoreboard.dat`.
//! Main retains this owner for commands, native criteria, teams and persistence.
//! Evidence and limits: `docs/reference/scoreboard{,-runtime}.md`.

pub mod commands;
pub mod criteria;
pub mod fastutil;
pub mod format;
pub mod hooks;
pub mod model;
pub mod names;
pub mod packets;
pub mod save;

#[cfg(test)]
mod tests;

pub use criteria::{ObjectiveCriteria, RenderType};
pub use format::{ChatFormatting, NumberFormat};
pub use model::{
    CollisionRule, DisplaySlot, Objective, Outbound, PlayerTeam, Score, ScoreAccess, ScoreHolder,
    Scoreboard, ScoreboardError, Visibility,
};
pub use packets::ScoreboardPacket;
pub use save::ScoreboardData;
