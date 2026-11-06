//! Owners of members of `level.dat` that are not the clock, the game rules or the world border
//! (WP-G2 stage 1): the data of the Ender Dragon fight, and (later) the other members.
//!
//! A library: nothing in `src/` outside this module calls it, and `src/level.rs` is untouched.
//! Evidence and limits: `docs/reference/level-data.md`.

pub mod dragon_fight;
pub mod timer_queue;

#[cfg(test)]
mod tests;

pub use dragon_fight::{default_gateways, DragonFightData};
pub use timer_queue::{FiredEvent, TimerCallback, TimerQueue};
