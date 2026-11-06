//! `ForcedChunksSavedData` (`data/chunks.dat`): the chunks kept loaded by `/forceload`.

use super::java_collections::LongOpenHashSet;
use super::storage::{DataFixType, Factory, SavedData};
use crate::nbt::{JavaString, Tag};
use std::any::Any;

pub const FILE_NAME: &str = "chunks";

/// The packed chunk positions (`ChunkPos.toLong`: x in the low 32 bits, z in the high 32) in a
/// fastutil `LongOpenHashSet`, whose iteration order is what `chunks.dat` stores.
#[derive(Debug, Clone, Default)]
pub struct ForcedChunks {
    chunks: LongOpenHashSet,
    dirty: bool,
}

impl ForcedChunks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn factory() -> Factory<ForcedChunks> {
        Factory {
            constructor: Box::new(ForcedChunks::new),
            deserializer: Box::new(|data| Ok(ForcedChunks::load(data))),
            fix: DataFixType::SavedDataForcedChunks,
        }
    }

    /// `load`: `new LongOpenHashSet(getLongArray("Forced"))`; a missing or non-long-array member is
    /// an empty array.
    pub fn load(data: &Tag) -> Self {
        let values: &[i64] = match data.get("Forced") {
            Some(Tag::LongArray(values)) => values,
            _ => &[],
        };
        Self {
            chunks: LongOpenHashSet::from_slice(values),
            dirty: false,
        }
    }

    pub fn contains(&self, packed: i64) -> bool {
        self.chunks.contains(packed)
    }

    /// The set in `toLongArray()` order.
    pub fn to_vec(&self) -> Vec<i64> {
        self.chunks.to_vec()
    }

    /// What `ServerLevel.setChunkForced` does to the set: add or remove one position and mark the
    /// data dirty when it changed. Returns whether it changed.
    pub fn set_forced(&mut self, packed: i64, forced: bool) -> bool {
        let changed = if forced {
            self.chunks.add(packed)
        } else {
            self.chunks.remove(packed)
        };
        if changed {
            self.dirty = true;
        }
        changed
    }
}

impl SavedData for ForcedChunks {
    fn save(&self) -> Tag {
        Tag::Compound(vec![(
            JavaString::from("Forced"),
            Tag::LongArray(self.chunks.to_vec()),
        )])
    }
    fn is_dirty(&self) -> bool {
        self.dirty
    }
    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
