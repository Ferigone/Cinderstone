//! `it.unimi.dsi.fastutil.objects.Object2ObjectOpenHashMap` with `String` keys, down to the
//! probing, growth, shrink and back-shift deletion that decide its iteration order.
//!
//! `Scoreboard` keeps objectives, teams and score holders in these maps, and the order they are
//! listed in is observable: it is the order of packets a joining player receives and of the lists
//! in `scoreboard.dat`. Iteration walks the table from the last slot to the first.

use crate::components::java::string_hash;

const DEFAULT_INITIAL_SIZE: usize = 16;

fn mix(hash: i32) -> i32 {
    let h = hash.wrapping_mul(0x9E37_79B9_u32 as i32);
    h ^ ((h as u32) >> 16) as i32
}

fn next_power_of_two(value: u64) -> u64 {
    if value == 0 {
        return 1;
    }
    (value - 1).next_power_of_two().max(1)
}

fn array_size(expected: usize, load_factor: f32) -> usize {
    let needed = (expected as f32 / load_factor).ceil() as u64;
    next_power_of_two(needed).max(2) as usize
}

fn max_fill(n: usize, load_factor: f32) -> usize {
    ((n as f32 * load_factor).ceil() as usize).min(n - 1)
}

#[derive(Debug, Clone)]
pub struct OpenHashMap<V> {
    keys: Vec<Option<String>>,
    values: Vec<Option<V>>,
    mask: usize,
    min_n: usize,
    max_fill: usize,
    size: usize,
    load_factor: f32,
}

impl<V> OpenHashMap<V> {
    /// `new Object2ObjectOpenHashMap(expected, loadFactor)`.
    pub fn with_capacity(expected: usize, load_factor: f32) -> Self {
        let n = array_size(expected, load_factor);
        Self {
            keys: vec![None; n],
            values: (0..n).map(|_| None).collect(),
            mask: n - 1,
            min_n: n,
            max_fill: max_fill(n, load_factor),
            size: 0,
            load_factor,
        }
    }

    /// `new Object2ObjectOpenHashMap()` (16 entries, load factor 0.75).
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_INITIAL_SIZE, 0.75)
    }

    fn n(&self) -> usize {
        self.mask + 1
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    fn slot(&self, key: &str) -> usize {
        (mix(string_hash(key)) as u32 as usize) & self.mask
    }

    /// The slot holding `key`, or the empty slot where it would go.
    fn find(&self, key: &str) -> Result<usize, usize> {
        let mut pos = self.slot(key);
        loop {
            match &self.keys[pos] {
                None => return Err(pos),
                Some(existing) if existing == key => return Ok(pos),
                Some(_) => pos = (pos + 1) & self.mask,
            }
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.find(key).is_ok()
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.find(key)
            .ok()
            .and_then(|pos| self.values[pos].as_ref())
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        match self.find(key) {
            Ok(pos) => self.values[pos].as_mut(),
            Err(_) => None,
        }
    }

    /// `put`: replaces an existing value (returning it) or inserts, growing past `maxFill`.
    pub fn insert(&mut self, key: &str, value: V) -> Option<V> {
        match self.find(key) {
            Ok(pos) => self.values[pos].replace(value),
            Err(pos) => {
                self.keys[pos] = Some(key.to_owned());
                self.values[pos] = Some(value);
                let before = self.size;
                self.size += 1;
                if before >= self.max_fill {
                    self.rehash(array_size(self.size + 1, self.load_factor));
                }
                None
            }
        }
    }

    /// `tryCapacity(capacity)` (what `ObjectOpenHashSet.addAll` calls first with a load factor above
    /// one half): grow to the table `capacity` entries need, never shrink.
    pub fn try_capacity(&mut self, capacity: usize) {
        let needed = next_power_of_two((capacity as f32 / self.load_factor).ceil() as u64).max(2).min(1 << 30) as usize;
        if needed > self.n() {
            self.rehash(needed);
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        let pos = self.find(key).ok()?;
        let old = self.values[pos].take();
        self.size -= 1;
        self.shift_keys(pos);
        let n = self.n();
        if n > self.min_n && self.size < self.max_fill / 4 && n > DEFAULT_INITIAL_SIZE {
            self.rehash(n / 2);
        }
        old
    }

    fn shift_keys(&mut self, mut pos: usize) {
        loop {
            let last = pos;
            pos = (last + 1) & self.mask;
            loop {
                let Some(current) = &self.keys[pos] else {
                    self.keys[last] = None;
                    self.values[last] = None;
                    return;
                };
                let slot = self.slot(current);
                let stays = if last <= pos {
                    last >= slot || slot > pos
                } else {
                    last >= slot && slot > pos
                };
                if stays {
                    break;
                }
                pos = (pos + 1) & self.mask;
            }
            self.keys[last] = self.keys[pos].take();
            self.values[last] = self.values[pos].take();
        }
    }

    fn rehash(&mut self, new_n: usize) {
        let mask = new_n - 1;
        let mut keys: Vec<Option<String>> = vec![None; new_n];
        let mut values: Vec<Option<V>> = (0..new_n).map(|_| None).collect();
        let old_keys = std::mem::take(&mut self.keys);
        let mut old_values = std::mem::take(&mut self.values);
        // Re-insert from the last old slot to the first, as `rehash` does.
        for index in (0..old_keys.len()).rev() {
            let Some(key) = &old_keys[index] else {
                continue;
            };
            let mut pos = (mix(string_hash(key)) as u32 as usize) & mask;
            while keys[pos].is_some() {
                pos = (pos + 1) & mask;
            }
            keys[pos] = Some(key.clone());
            values[pos] = old_values[index].take();
        }
        self.keys = keys;
        self.values = values;
        self.mask = mask;
        self.max_fill = max_fill(new_n, self.load_factor);
    }

    /// Entries in the order `forEach`, `keySet()` and `values()` visit them.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        (0..self.n())
            .rev()
            .filter_map(move |pos| match (&self.keys[pos], &self.values[pos]) {
                (Some(key), Some(value)) => Some((key.as_str(), value)),
                _ => None,
            })
    }

    /// Keys as `keySet().stream()` visits them: the table from the first slot to the last
    /// (`forEach` and iterators go the other way).
    pub fn keys_ascending(&self) -> impl Iterator<Item = &str> {
        (0..self.n()).filter_map(move |pos| self.keys[pos].as_deref())
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }
}

impl<V> Default for OpenHashMap<V> {
    fn default() -> Self {
        Self::new()
    }
}
