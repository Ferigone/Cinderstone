//! `net.minecraft.world.scores` and `ServerScoreboard` (WP-E4 stages 1–3): objectives, scores,
//! teams, display slots, number formats, the five scoreboard packets and `scoreboard.dat`.
//! Nothing here is called by Main. Evidence and limits: `docs/reference/scoreboard.md`.

pub mod criteria;
pub mod fastutil;
pub mod format;
pub mod hooks;
pub mod model;
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
