//! The two Java collections whose iteration order shows in the files the owners write: a
//! `java.util.HashMap<String, V>` and fastutil's `LongOpenHashSet`.

use crate::components::java::hashmap_order_by_hash;
use crate::nbt::{JavaString, Tag};

/// `String.hashCode()` of a Java string that may hold unpaired surrogates.
pub fn java_string_hash(text: &JavaString) -> i32 {
    text.iter_units().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

/// A `java.util.HashMap<String, V>` created with `Maps.newHashMap()`: entries are kept in
/// insertion order and the iteration order is derived from the hash codes and the table size.
///
/// The table starts with 16 buckets and doubles when the size passes 75%; it never shrinks, so
/// removed entries leave a larger table behind. Buckets are never treeified here (that needs eight
/// keys in one bucket of a table of at least 64 buckets).
#[derive(Debug, Clone)]
pub struct JavaHashMap<V> {
    entries: Vec<(JavaString, V)>,
    table: usize,
}

impl<V> Default for JavaHashMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> JavaHashMap<V> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            table: 16,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, key: &JavaString) -> Option<&V> {
        self.entries
            .iter()
            .find(|(existing, _)| existing == key)
            .map(|(_, value)| value)
    }

    /// `put`: replacing a value keeps the entry where it is; a new key goes last in its bucket.
    pub fn put(&mut self, key: JavaString, value: V) -> Option<V> {
        if let Some(slot) = self
            .entries
            .iter_mut()
            .find(|(existing, _)| *existing == key)
        {
            return Some(std::mem::replace(&mut slot.1, value));
        }
        self.entries.push((key, value));
        if self.entries.len() > self.table * 3 / 4 {
            self.table *= 2;
        }
        None
    }

    pub fn remove(&mut self, key: &JavaString) -> Option<V> {
        let index = self
            .entries
            .iter()
            .position(|(existing, _)| existing == key)?;
        Some(self.entries.remove(index).1)
    }

    /// The entries in the order `HashMap` iterates them.
    pub fn iter_ordered(&self) -> Vec<(&JavaString, &V)> {
        let hashes: Vec<i32> = self
            .entries
            .iter()
            .map(|(key, _)| java_string_hash(key))
            .collect();
        hashmap_order_by_hash(&hashes, self.table)
            .into_iter()
            .map(|index| (&self.entries[index].0, &self.entries[index].1))
            .collect()
    }
}

const LONG_PHI: i64 = 0x9E37_79B9_7F4A_7C15_u64 as i64;
const DEFAULT_INITIAL_SIZE: usize = 16;
const LOAD_FACTOR: f64 = 0.75;

/// `HashCommon.mix(long)`.
fn mix(value: i64) -> i64 {
    let mut hash = value.wrapping_mul(LONG_PHI);
    hash ^= ((hash as u64) >> 32) as i64;
    hash ^ (((hash as u64) >> 16) as i64)
}

/// `HashCommon.arraySize(expected, f)`.
fn array_size(expected: usize) -> usize {
    let wanted = (expected as f64 / LOAD_FACTOR).ceil() as u64;
    let size = wanted.max(1).next_power_of_two().max(2);
    size as usize
}

/// `HashCommon.maxFill(n, f)`.
fn max_fill(n: usize) -> usize {
    (((n as f64) * LOAD_FACTOR).ceil() as usize).min(n - 1)
}

/// fastutil 8.5 `LongOpenHashSet`: open addressing with linear probing, the zero key held outside
/// the table, `toLongArray()` listing the zero key first and then the table from the last slot
/// down.
#[derive(Debug, Clone)]
pub struct LongOpenHashSet {
    key: Vec<i64>,
    mask: usize,
    n: usize,
    min_n: usize,
    max_fill: usize,
    size: usize,
    contains_null: bool,
}

impl Default for LongOpenHashSet {
    /// `new LongOpenHashSet()`: an expected size of 16.
    fn default() -> Self {
        Self::with_expected(DEFAULT_INITIAL_SIZE)
    }
}

impl LongOpenHashSet {
    /// `new LongOpenHashSet(expected, 0.75f)`.
    pub fn with_expected(expected: usize) -> Self {
        let n = array_size(expected);
        Self {
            key: vec![0; n + 1],
            mask: n - 1,
            n,
            min_n: n,
            max_fill: max_fill(n),
            size: 0,
            contains_null: false,
        }
    }

    /// `new LongOpenHashSet(long[] a)`: sized for `a.length`, then every element added.
    pub fn from_slice(values: &[i64]) -> Self {
        let mut set = Self::with_expected(values.len());
        for value in values {
            set.add(*value);
        }
        set
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    fn slot(&self, value: i64) -> usize {
        (mix(value) as usize) & self.mask
    }

    pub fn contains(&self, value: i64) -> bool {
        if value == 0 {
            return self.contains_null;
        }
        let mut pos = self.slot(value);
        loop {
            let current = self.key[pos];
            if current == 0 {
                return false;
            }
            if current == value {
                return true;
            }
            pos = (pos + 1) & self.mask;
        }
    }

    pub fn add(&mut self, value: i64) -> bool {
        if value == 0 {
            if self.contains_null {
                return false;
            }
            self.contains_null = true;
        } else {
            let mut pos = self.slot(value);
            loop {
                let current = self.key[pos];
                if current == 0 {
                    break;
                }
                if current == value {
                    return false;
                }
                pos = (pos + 1) & self.mask;
            }
            self.key[pos] = value;
        }
        let before = self.size;
        self.size += 1;
        if before >= self.max_fill {
            self.rehash(array_size(self.size + 1));
        }
        true
    }

    pub fn remove(&mut self, value: i64) -> bool {
        if value == 0 {
            if !self.contains_null {
                return false;
            }
            self.contains_null = false;
            self.key[self.n] = 0;
            self.size -= 1;
            self.shrink_if_sparse();
            return true;
        }
        let mut pos = self.slot(value);
        loop {
            let current = self.key[pos];
            if current == 0 {
                return false;
            }
            if current == value {
                self.size -= 1;
                self.shift_keys(pos);
                self.shrink_if_sparse();
                return true;
            }
            pos = (pos + 1) & self.mask;
        }
    }

    fn shrink_if_sparse(&mut self) {
        if self.n > self.min_n && self.size < self.max_fill / 4 && self.n > DEFAULT_INITIAL_SIZE {
            self.rehash(self.n / 2);
        }
    }

    /// Back-shift deletion of the key at `pos`.
    fn shift_keys(&mut self, mut pos: usize) {
        loop {
            let last = pos;
            pos = (pos + 1) & self.mask;
            loop {
                let current = self.key[pos];
                if current == 0 {
                    self.key[last] = 0;
                    return;
                }
                let slot = self.slot(current);
                let movable = if last <= pos {
                    last >= slot || slot > pos
                } else {
                    last >= slot && slot > pos
                };
                if movable {
                    self.key[last] = current;
                    break;
                }
                pos = (pos + 1) & self.mask;
            }
        }
    }

    fn rehash(&mut self, new_n: usize) {
        let mask = new_n - 1;
        let mut new_key = vec![0i64; new_n + 1];
        let real_size = if self.contains_null {
            self.size - 1
        } else {
            self.size
        };
        let mut i = self.n;
        for _ in 0..real_size {
            loop {
                i -= 1;
                if self.key[i] != 0 {
                    break;
                }
            }
            let mut pos = (mix(self.key[i]) as usize) & mask;
            while new_key[pos] != 0 {
                pos = (pos + 1) & mask;
            }
            new_key[pos] = self.key[i];
        }
        new_key[new_n] = self.key[self.n];
        self.n = new_n;
        self.mask = mask;
        self.max_fill = max_fill(new_n);
        self.key = new_key;
    }

    /// `toLongArray()`: the zero key first, then the table from the last slot to the first.
    pub fn to_vec(&self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.size);
        if self.contains_null {
            out.push(0);
        }
        for pos in (0..self.n).rev() {
            if self.key[pos] != 0 {
                out.push(self.key[pos]);
            }
        }
        out
    }
}

/// The tag as Java holds it after reading or building it: every compound, at any depth, iterates
/// in the order of its `HashMap`, filled in the order the entries are given (a file lists them in
/// the order the writer's map iterated, a parser in source order).
pub fn to_java_order(tag: &Tag) -> Tag {
    match tag {
        Tag::Compound(entries) => {
            let mut map: JavaHashMap<Tag> = JavaHashMap::new();
            for (name, value) in entries {
                map.put(name.clone(), to_java_order(value));
            }
            Tag::Compound(
                map.iter_ordered()
                    .into_iter()
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect(),
            )
        }
        Tag::List {
            element_type,
            values,
        } => Tag::List {
            element_type: *element_type,
            values: values.iter().map(to_java_order).collect(),
        },
        other => other.clone(),
    }
}
