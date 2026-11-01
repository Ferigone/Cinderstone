//! `RandomSequences`: the world's named random sequences (`random_sequences.dat`).
//!
//! Almost every shipped loot table names a `random_sequence`; the roll then draws
//! from that persistent Xoroshiro stream (`ServerLevel.getRandomSequence`), not
//! from the level RNG. Which source a roll uses follows `LootContext.Builder.create`:
//! an explicit non-zero seed, else the table's sequence, else the level random.
//! The saved-data file lifecycle belongs to the world-storage owner; this type
//! owns the streams and their NBT form so that owner has nothing to reinvent.

use super::{LootError, Result};
use crate::nbt::{JavaString, Tag};
use crate::random::{
    seed_from_hash_of, upgrade_seed_unmixed, InvalidBound, LegacyRandomSource,
    PositionalRandomFactory, RandomSource, RandomSourceKind, XoroshiroRandomSource,
};
use std::collections::BTreeMap;

/// `RandomSequences`.
#[derive(Debug, Clone)]
pub struct RandomSequences {
    world_seed: i64,
    salt: i32,
    include_world_seed: bool,
    include_sequence_id: bool,
    sequences: BTreeMap<String, XoroshiroRandomSource>,
    dirty: bool,
}

impl RandomSequences {
    pub fn new(world_seed: i64) -> Self {
        Self {
            world_seed,
            salt: 0,
            include_world_seed: true,
            include_sequence_id: true,
            sequences: BTreeMap::new(),
            dirty: false,
        }
    }

    /// `setSeedDefaults(salt, includeWorldSeed, includeSequenceId)`.
    pub fn set_seed_defaults(&mut self, salt: i32, include_world_seed: bool, include_sequence_id: bool) {
        self.salt = salt;
        self.include_world_seed = include_world_seed;
        self.include_sequence_id = include_sequence_id;
    }

    /// `SavedData.isDirty()`: set by any draw from, or reset of, a sequence.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    fn create(&self, id: &str, salt: i32, include_world_seed: bool, include_sequence_id: bool) -> XoroshiroRandomSource {
        let seed = (if include_world_seed { self.world_seed } else { 0 }) ^ i64::from(salt);
        let mut state = upgrade_seed_unmixed(seed);
        if include_sequence_id {
            state = state.xor(seed_from_hash_of(id));
        }
        XoroshiroRandomSource::from_seed128(state.mixed())
    }

    /// `RandomSequences.get(id)`: the persistent stream, created on first use.
    pub fn get(&mut self, id: &str) -> SequenceRandom<'_> {
        if !self.sequences.contains_key(id) {
            let created = self.create(id, self.salt, self.include_world_seed, self.include_sequence_id);
            self.sequences.insert(id.to_owned(), created);
        }
        SequenceRandom {
            dirty: &mut self.dirty,
            source: self.sequences.get_mut(id).expect("inserted above"),
        }
    }

    /// `reset(id)`: re-seed one sequence from the current defaults.
    pub fn reset(&mut self, id: &str) {
        let created = self.create(id, self.salt, self.include_world_seed, self.include_sequence_id);
        self.sequences.insert(id.to_owned(), created);
        self.dirty = true;
    }

    /// `reset(id, salt, includeWorldSeed, includeSequenceId)`.
    pub fn reset_with(&mut self, id: &str, salt: i32, include_world_seed: bool, include_sequence_id: bool) {
        let created = self.create(id, salt, include_world_seed, include_sequence_id);
        self.sequences.insert(id.to_owned(), created);
        self.dirty = true;
    }

    /// `clear()`: forget every sequence, returning how many there were.
    pub fn clear(&mut self) -> usize {
        let count = self.sequences.len();
        self.sequences.clear();
        self.dirty = true;
        count
    }

    /// Current `(seed_lo, seed_hi)` of a sequence, if it exists.
    pub fn state(&self, id: &str) -> Option<(u64, u64)> {
        self.sequences.get(id).map(|source| {
            let state = source.state();
            (state.lo, state.hi)
        })
    }

    /// `RandomSequences.save`: the `data/random_sequences.dat` payload compound.
    pub fn save(&self) -> Tag {
        let mut sequences = Vec::new();
        for (id, source) in &self.sequences {
            let state = source.state();
            let record = Tag::Compound(vec![(
                JavaString::Utf8("source".to_owned()),
                Tag::LongArray(vec![state.lo as i64, state.hi as i64]),
            )]);
            sequences.push((JavaString::Utf8(id.clone()), record));
        }
        Tag::Compound(vec![
            (JavaString::Utf8("salt".to_owned()), Tag::Int(self.salt)),
            (JavaString::Utf8("include_world_seed".to_owned()), Tag::Byte(self.include_world_seed as i8)),
            (JavaString::Utf8("include_sequence_id".to_owned()), Tag::Byte(self.include_sequence_id as i8)),
            (JavaString::Utf8("sequences".to_owned()), Tag::Compound(sequences)),
        ])
    }

    /// `RandomSequences.load`. A sequence that fails to decode is skipped, as in
    /// Java (which only logs it); the other fields default as Java's do.
    pub fn load(world_seed: i64, root: &Tag) -> Result<Self> {
        let mut loaded = Self::new(world_seed);
        let flag = |name: &str, default: bool| match root.get(name) {
            Some(Tag::Byte(value)) => *value != 0,
            _ => default,
        };
        loaded.salt = match root.get("salt") {
            Some(Tag::Int(value)) => *value,
            _ => 0,
        };
        loaded.include_world_seed = flag("include_world_seed", true);
        loaded.include_sequence_id = flag("include_sequence_id", true);
        if let Some(Tag::Compound(entries)) = root.get("sequences") {
            for (name, record) in entries {
                let Some(name) = name.as_str() else {
                    return Err(LootError::Host("random sequence id is not valid UTF-8".to_owned()));
                };
                if let Some(Tag::LongArray(state)) = record.get("source") {
                    if let [lo, hi] = state.as_slice() {
                        loaded
                            .sequences
                            .insert(name.to_owned(), XoroshiroRandomSource::from_state(*lo as u64, *hi as u64));
                    }
                }
            }
        }
        Ok(loaded)
    }
}

/// `RandomSequences.DirtyMarkingRandomSource`: every draw marks the data dirty.
pub struct SequenceRandom<'a> {
    dirty: &'a mut bool,
    source: &'a mut XoroshiroRandomSource,
}

impl RandomSource for SequenceRandom<'_> {
    fn set_seed(&mut self, seed: i64) {
        *self.dirty = true;
        self.source.set_seed(seed);
    }
    fn next_bits(&mut self, bits: i32) -> i32 {
        *self.dirty = true;
        self.source.next_bits(bits)
    }
    fn fork(&mut self) -> RandomSourceKind {
        *self.dirty = true;
        RandomSourceKind::Xoroshiro(self.source.fork())
    }
    fn next_i32(&mut self) -> i32 {
        *self.dirty = true;
        self.source.next_i32()
    }
    fn next_i64(&mut self) -> i64 {
        *self.dirty = true;
        self.source.next_i64()
    }
    fn next_bounded_i32(&mut self, bound: i32) -> std::result::Result<i32, InvalidBound> {
        *self.dirty = true;
        self.source.next_bounded_i32(bound)
    }
    fn next_bool(&mut self) -> bool {
        *self.dirty = true;
        self.source.next_bool()
    }
    fn next_f32(&mut self) -> f32 {
        *self.dirty = true;
        self.source.next_f32()
    }
    fn next_f64(&mut self) -> f64 {
        *self.dirty = true;
        self.source.next_f64()
    }
    fn consume_count(&mut self, count: i32) {
        *self.dirty = true;
        self.source.consume_count(count);
    }
    fn fork_positional(&mut self) -> PositionalRandomFactory {
        *self.dirty = true;
        PositionalRandomFactory::Xoroshiro(self.source.fork_positional())
    }
}

/// The random source of one roll, selected as `LootContext.Builder.create` does.
pub enum RollRandom<'a> {
    /// `withOptionalRandomSeed(seed)` with a non-zero seed: `RandomSource.create(seed)`.
    Seeded(LegacyRandomSource),
    /// The table's `random_sequence`.
    Sequence(SequenceRandom<'a>),
    /// `ServerLevel.getRandom()`.
    Level(&'a mut dyn RandomSource),
}

impl<'a> RollRandom<'a> {
    /// Choose the source: seed if non-zero, else the sequence, else the level RNG.
    pub fn select(
        seed: i64,
        random_sequence: Option<&str>,
        sequences: &'a mut RandomSequences,
        level_random: &'a mut dyn RandomSource,
    ) -> Self {
        if seed != 0 {
            Self::Seeded(LegacyRandomSource::new(seed))
        } else if let Some(id) = random_sequence {
            Self::Sequence(sequences.get(id))
        } else {
            Self::Level(level_random)
        }
    }

    fn inner(&mut self) -> &mut dyn RandomSource {
        match self {
            Self::Seeded(source) => source,
            Self::Sequence(source) => source,
            Self::Level(source) => &mut **source,
        }
    }
}

impl RandomSource for RollRandom<'_> {
    fn set_seed(&mut self, seed: i64) {
        self.inner().set_seed(seed)
    }
    fn next_bits(&mut self, bits: i32) -> i32 {
        self.inner().next_bits(bits)
    }
    fn fork(&mut self) -> RandomSourceKind {
        self.inner().fork()
    }
    fn next_i32(&mut self) -> i32 {
        self.inner().next_i32()
    }
    fn next_i64(&mut self) -> i64 {
        self.inner().next_i64()
    }
    fn next_bounded_i32(&mut self, bound: i32) -> std::result::Result<i32, InvalidBound> {
        self.inner().next_bounded_i32(bound)
    }
    fn next_bool(&mut self) -> bool {
        self.inner().next_bool()
    }
    fn next_f32(&mut self) -> f32 {
        self.inner().next_f32()
    }
    fn next_f64(&mut self) -> f64 {
        self.inner().next_f64()
    }
    fn consume_count(&mut self, count: i32) {
        self.inner().consume_count(count)
    }
    fn fork_positional(&mut self) -> PositionalRandomFactory {
        self.inner().fork_positional()
    }
}
