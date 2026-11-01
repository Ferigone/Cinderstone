//! Source-ordered ordinary Mob and Bat phases for Minecraft Java 1.21.
//!
//! The embedding runtime supplies actual living/physical state, world queries,
//! entity/Level RNG, selectors, navigation and controls. These modules do not
//! create a scheduler, canonical entity factory or production Main callsite.

pub mod ai_step;
pub mod bat;
pub mod retained_bat;
pub mod spawning;
