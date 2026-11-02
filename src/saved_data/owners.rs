//! First saved-data owners: `idcounts` (`MapIndex`) and `random_sequences` (`RandomSequences`).

use super::file::numeric_as_int;
use super::storage::{DataFixType, Factory, SavedData};
use crate::loot::sequences::RandomSequences;
use crate::nbt::{JavaString, Tag};
use std::any::Any;
use std::collections::BTreeMap;

/// `MapIndex` (`data/idcounts.dat`): the highest map id handed out, under the key `map`.
#[derive(Debug, Clone, Default)]
pub struct MapIndex {
    used_aux_ids: BTreeMap<String, i32>,
    dirty: bool,
}

impl MapIndex {
    pub const FILE_NAME: &'static str = "idcounts";

    pub fn new() -> Self {
        Self::default()
    }

    pub fn factory() -> Factory<MapIndex> {
        Factory {
            constructor: Box::new(MapIndex::new),
            deserializer: Box::new(|data| Ok(MapIndex::load(data))),
            fix: DataFixType::SavedDataMapIndex,
        }
    }

    /// `MapIndex.load`: every numeric member, converted with `getInt`.
    pub fn load(data: &Tag) -> Self {
        let mut index = Self::new();
        if let Tag::Compound(entries) = data {
            for (name, value) in entries {
                if let (Some(name), Some(number)) = (name.as_str(), numeric_as_int(value)) {
                    index.used_aux_ids.insert(name.to_owned(), number);
                }
            }
        }
        index
    }

    /// `getFreeAuxValueForMap`: the next map id (the first one is 0).
    pub fn get_free_aux_value_for_map(&mut self) -> i32 {
        let next = self.used_aux_ids.get("map").copied().unwrap_or(-1).wrapping_add(1);
        self.used_aux_ids.insert("map".to_owned(), next);
        self.dirty = true;
        next
    }

    pub fn used(&self, key: &str) -> Option<i32> {
        self.used_aux_ids.get(key).copied()
    }
}

impl SavedData for MapIndex {
    fn save(&self) -> Tag {
        Tag::Compound(
            self.used_aux_ids
                .iter()
                .map(|(name, value)| (JavaString::from(name.as_str()), Tag::Int(*value)))
                .collect(),
        )
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

impl SavedData for RandomSequences {
    fn save(&self) -> Tag {
        RandomSequences::save(self)
    }
    fn is_dirty(&self) -> bool {
        RandomSequences::is_dirty(self)
    }
    fn set_dirty(&mut self, dirty: bool) {
        RandomSequences::set_dirty(self, dirty)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `RandomSequences.factory(worldSeed)` (`data/random_sequences.dat`).
pub fn random_sequences_factory(world_seed: i64) -> Factory<RandomSequences> {
    Factory {
        constructor: Box::new(move || RandomSequences::new(world_seed)),
        deserializer: Box::new(move |data| {
            RandomSequences::load(world_seed, data).map_err(|error| error.to_string())
        }),
        fix: DataFixType::SavedDataRandomSequences,
    }
}

/// File name (without `.dat`) of the random sequences.
pub const RANDOM_SEQUENCES: &str = "random_sequences";
