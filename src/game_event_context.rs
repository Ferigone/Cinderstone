//! Original GameEventDispatcher traversal for certified empty registries.
//!
//! Only the empty-listener path is implemented. Every loaded section must supply its actual registry
//! state. Block-entity absence alone cannot certify entity/dynamic listeners.
use crate::{entity_state::java_floor, geometry::Vec3};
use anyhow::{bail, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryState {
    /// The actual immutable GameEventListenerRegistry.NOOP instance.
    Noop,
    /// Snapshot of the actual Euclidean registry, including deferred buffers.
    Euclidean {
        listeners: usize,
        pending_add: usize,
        pending_remove: usize,
        processing: bool,
    },
    /// Missing source coverage is unknown, even when no BE list was observed.
    Unknown,
}

impl RegistryState {
    fn is_quiescent_empty(self) -> bool {
        matches!(
            self,
            Self::Noop
                | Self::Euclidean {
                    listeners: 0,
                    pending_add: 0,
                    pending_remove: 0,
                    processing: false
                }
        )
    }
}

pub trait GameEventRegistryWorld {
    type Chunk: Copy;

    /// Original ServerChunkCache.getChunkNow. Return only an actually loaded
    /// chunk; this query must not load or promote a chunk.
    fn get_loaded_chunk(&mut self, x: i32, z: i32) -> Result<Option<Self::Chunk>>;

    /// Perform the actual LevelChunk.getListenerRegistry lookup/creation and
    /// report the complete current registry state. A provider must own all
    /// listener sources/registration hooks or return Unknown. In particular,
    /// incomplete entity data or absent BE parsing cannot imply emptiness.
    /// These lookups and empty-registry materialization consume no RNG.
    fn registry_state(&mut self, chunk: Self::Chunk, section_y: i32) -> Result<RegistryState>;
}

#[derive(Clone, Copy, Debug)]
pub struct GameEventPost {
    pub position: Vec3,
    /// Read from the actual GameEvent definition; do not invent a default.
    pub notification_radius: i32,
}

/// Original vanilla events emitted by the supported ItemEntity callbacks.
/// Their identity and radii are recorded from the actual GAME_EVENT registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemGameEvent {
    EntityDamage,
    HitGround,
    Splash,
}

impl ItemGameEvent {
    pub const fn registry_id(self) -> i32 {
        match self {
            Self::EntityDamage => 14,
            Self::HitGround => 26,
            Self::Splash => 40,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::EntityDamage => "minecraft:entity_damage",
            Self::HitGround => "minecraft:hit_ground",
            Self::Splash => "minecraft:splash",
        }
    }

    pub const fn notification_radius(self) -> i32 {
        16
    }

    pub fn at(self, position: Vec3) -> GameEventPost {
        GameEventPost {
            position,
            notification_radius: self.notification_radius(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmptyDispatchSummary {
    pub chunk_queries: usize,
    pub loaded_chunks: usize,
    pub section_visits: usize,
}

/// Traverse X, then Z, then section Y, including sections outside build height.
/// A real nonempty or unproven registry requires a further actual dispatcher:
/// this helper fails there and never substitutes a silent listener callback.
///
/// The empty original path reads neither GameEvent.Context nor entity getters,
/// calls no listener/debug delivery, and consumes no random numbers. An empty
/// quiescent Euclidean visit temporarily sets processing=true then false; with
/// no listeners or deferred buffers it has no remaining state mutation.
pub fn post_to_empty_registries(
    world: &mut impl GameEventRegistryWorld,
    event: GameEventPost,
) -> Result<EmptyDispatchSummary> {
    let block = [
        java_floor(event.position.x),
        java_floor(event.position.y),
        java_floor(event.position.z),
    ];
    let minimum = block.map(|coordinate| coordinate.wrapping_sub(event.notification_radius) >> 4);
    let maximum = block.map(|coordinate| coordinate.wrapping_add(event.notification_radius) >> 4);
    let mut result = EmptyDispatchSummary::default();
    // A wrapped empty X/Z rectangle cannot execute any chunk getter. Avoid
    // iterating a huge outer X interval when its inner Z interval is empty.
    // An empty Y interval still requires the original X/Z chunk queries.
    if minimum[0] > maximum[0] || minimum[2] > maximum[2] {
        return Ok(result);
    }
    for x in minimum[0]..=maximum[0] {
        for z in minimum[2]..=maximum[2] {
            result.chunk_queries += 1;
            let Some(chunk) = world.get_loaded_chunk(x, z)? else {
                continue;
            };
            result.loaded_chunks += 1;
            for y in minimum[1]..=maximum[1] {
                let registry = world.registry_state(chunk, y)?;
                if !registry.is_quiescent_empty() {
                    bail!("game-event registry at section ({x},{y},{z}) requires actual listener dispatch: {registry:?}");
                }
                result.section_visits += 1;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "../tools/game_event_context_tests.rs"]
mod tests;
