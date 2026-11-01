//! Default fastutil 8.5.12 Int2ObjectOpenHashMap layout used by ChunkMap.
//!
//! Iteration is observable in the shared entity-sender phase: the zero key is
//! first, then occupied table slots in descending order. Neither numeric ID
//! order nor insertion order preserves it after collisions and rehashing.
//! Values are opaque Copy handles into the host's authoritative entity store.
//! This subset covers put/get/remove/clear and immutable iteration; it does
//! not expose mutation through a live Java map iterator.

#[derive(Debug, Clone)]
pub struct TrackingEntityMap<H: Copy> {
    keys: Vec<i32>,
    values: Vec<Option<H>>,
    size: usize,
    has_zero: bool,
}

impl<H: Copy> Default for TrackingEntityMap<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Copy> TrackingEntityMap<H> {
    pub fn new() -> Self {
        Self {
            keys: vec![0; 33],
            values: vec![None; 33],
            size: 0,
            has_zero: false,
        }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn capacity(&self) -> usize {
        self.keys.len() - 1
    }

    pub fn get(&self, key: i32) -> Option<H> {
        self.index(key).and_then(|index| self.values[index])
    }

    pub fn contains_key(&self, key: i32) -> bool {
        self.index(key).is_some()
    }

    pub fn insert(&mut self, key: i32, value: H) -> Option<H> {
        if let Some(index) = self.index(key) {
            return self.values[index].replace(value);
        }
        let n = self.capacity();
        let index = if key == 0 {
            self.has_zero = true;
            n
        } else {
            let mut index = mix(key) & (n - 1);
            while self.keys[index] != 0 {
                index = (index + 1) & (n - 1);
            }
            self.keys[index] = key;
            index
        };
        self.values[index] = Some(value);
        let previous_size = self.size;
        self.size += 1;
        if previous_size >= max_fill(n) {
            self.rehash(((self.size + 1) * 4).div_ceil(3).next_power_of_two().max(2));
        }
        None
    }

    pub fn remove(&mut self, key: i32) -> Option<H> {
        let index = self.index(key)?;
        let previous = self.values[index].take();
        self.size -= 1;
        if key == 0 {
            self.has_zero = false;
        } else {
            self.shift_keys(index);
        }
        let n = self.capacity();
        // Default construction sets final minN=32; rehash and clear retain it.
        // The original additional n>16 guard is implied by this minN=32.
        if n > 32 && self.size < max_fill(n) / 4 {
            self.rehash(n / 2);
        }
        previous
    }

    pub fn clear(&mut self) {
        if self.size == 0 {
            return;
        }
        self.keys.fill(0);
        self.values.fill(None);
        self.size = 0;
        self.has_zero = false;
    }

    pub fn entries(&self) -> impl Iterator<Item = (i32, H)> + '_ {
        let zero = self.has_zero.then(|| {
            (
                0,
                self.values[self.capacity()].expect("zero-key value exists"),
            )
        });
        zero.into_iter().chain(
            (0..self.capacity())
                .rev()
                .filter(|&index| self.keys[index] != 0)
                .map(|index| {
                    (
                        self.keys[index],
                        self.values[index].expect("occupied-key value exists"),
                    )
                }),
        )
    }

    pub fn values(&self) -> impl Iterator<Item = H> + '_ {
        self.entries().map(|(_, value)| value)
    }

    fn index(&self, key: i32) -> Option<usize> {
        if key == 0 {
            return self.has_zero.then_some(self.capacity());
        }
        let mask = self.capacity() - 1;
        let mut index = mix(key) & mask;
        loop {
            if self.keys[index] == key {
                return Some(index);
            }
            if self.keys[index] == 0 {
                return None;
            }
            index = (index + 1) & mask;
        }
    }

    fn shift_keys(&mut self, mut hole: usize) {
        let mask = self.capacity() - 1;
        loop {
            let last = hole;
            hole = (hole + 1) & mask;
            loop {
                let current = self.keys[hole];
                if current == 0 {
                    self.keys[last] = 0;
                    self.values[last] = None;
                    return;
                }
                let slot = mix(current) & mask;
                if if last <= hole {
                    last >= slot || slot > hole
                } else {
                    last >= slot && slot > hole
                } {
                    break;
                }
                hole = (hole + 1) & mask;
            }
            self.keys[last] = self.keys[hole];
            self.values[last] = self.values[hole];
        }
    }

    fn rehash(&mut self, n: usize) {
        let previous_n = self.capacity();
        let previous_keys = std::mem::replace(&mut self.keys, vec![0; n + 1]);
        let previous_values = std::mem::replace(&mut self.values, vec![None; n + 1]);
        self.values[n] = previous_values[previous_n];
        for index in (0..previous_n).rev() {
            let key = previous_keys[index];
            if key != 0 {
                let mut slot = mix(key) & (n - 1);
                while self.keys[slot] != 0 {
                    slot = (slot + 1) & (n - 1);
                }
                self.keys[slot] = key;
                self.values[slot] = previous_values[index];
            }
        }
    }
}

fn mix(key: i32) -> usize {
    let hash = (key as u32).wrapping_mul(0x9e37_79b9);
    (hash ^ (hash >> 16)) as usize
}

fn max_fill(n: usize) -> usize {
    (n * 3).div_ceil(4).min(n - 1)
}

#[cfg(test)]
#[path = "../tools/entity_tracking_order_tests.rs"]
mod tests;
