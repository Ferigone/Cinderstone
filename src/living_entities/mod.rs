//! Original living-entity death phases with mandatory actual-owner operations.
//!
//! These runners create no actor or scheduler. Full native removal, death loot,
//! persistence and same-connection respawn remain real-host responsibilities.

pub mod death_tick;
pub mod death_loot;
pub mod removal;
pub mod respawn;
pub mod restore;
pub mod client_command;
pub mod server_player_death;
pub mod base_tick;
pub mod death;
pub mod tick;
pub mod ai_step;
pub mod travel;
pub mod state;
pub mod brain;
pub mod metadata;
