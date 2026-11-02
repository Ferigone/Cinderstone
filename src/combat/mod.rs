//! Native damage phases with mandatory callbacks into the real actor/world.
//!
//! These runners own no actor, equipment, effect map, registry, or RNG. A host
//! must execute each source callback synchronously or return an error.

pub mod reduction;
pub mod totem;
pub mod tracker;
pub mod living_health;
