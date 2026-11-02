//! The insertion/rehash/traversal subset of Fastutil 8.5.12's long-object map.
//! Blender creates default-sized maps and neither removes nor mutates their
//! keys. Preserving its slot order preserves floating-point accumulation order.

const INITIAL_CAPACITY: usize = 32;
const MAX_CAPACITY: usize = 1 << 30;
const LONG_PHI: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Debug)]
pub(super) struct SourceLongMap<T> {
    slots: Vec<Option<(i64, T)>>,
    zero: Option<T>,
    size: usize,
    max_fill: usize,
}

impl<T> SourceLongMap<T> {
    pub(super) fn new() -> Self {
        // new Long2ObjectOpenHashMap(): expected=16, load factor=.75,
        // HashCommon.arraySize gives 32 actual non-null-key hash slots.
        Self {
            slots: empty_slots(INITIAL_CAPACITY),
            zero: None,
            size: 0,
            max_fill: max_fill(INITIAL_CAPACITY),
        }
    }

    pub(super) fn insert(&mut self, key: i64, value: T) -> Option<T> {
        if key == 0 {
            let previous = self.zero.replace(value);
            if previous.is_none() {
                self.inserted();
            }
            return previous;
        }
        let mask = self.slots.len() - 1;
        let mut position = mix(key) as usize & mask;
        loop {
            match self.slots[position].as_ref() {
                Some((present, _)) if *present == key => {
                    // Replacement neither increments size nor moves the slot.
                    let (_, previous) = self.slots[position].replace((key, value)).unwrap();
                    return Some(previous);
                }
                Some(_) => position = (position + 1) & mask,
                None => {
                    self.slots[position] = Some((key, value));
                    self.inserted();
                    return None;
                }
            }
        }
    }

    pub(super) fn get(&self, key: i64) -> Option<&T> {
        if key == 0 {
            return self.zero.as_ref();
        }
        let mask = self.slots.len() - 1;
        let mut position = mix(key) as usize & mask;
        loop {
            match self.slots[position].as_ref() {
                Some((present, value)) if *present == key => return Some(value),
                Some(_) => position = (position + 1) & mask,
                None => return None,
            }
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (i64, &T)> {
        // Long2ObjectMap.forEach dispatches to MapEntrySet.fastForEach: the
        // special zero-key entry first, then occupied table slots n-1 down to 0.
        self.zero.iter().map(|value| (0, value)).chain(
            self.slots
                .iter()
                .rev()
                .filter_map(|entry| entry.as_ref().map(|(key, value)| (*key, value))),
        )
    }

    fn inserted(&mut self) {
        let previous_size = self.size;
        self.size += 1;
        // Original insert tests the pre-increment size against maxFill, then
        // rehashes using arraySize(the new size + 1, load factor).
        if previous_size >= self.max_fill {
            self.rehash(array_size(self.size + 1));
        }
    }

    fn rehash(&mut self, capacity: usize) {
        let previous = std::mem::replace(&mut self.slots, empty_slots(capacity));
        let mask = capacity - 1;
        // The source rehash visits old occupied slots in descending order.
        // Moving entries preserves T ownership without a Clone requirement.
        for entry in previous.into_iter().rev().flatten() {
            let mut position = mix(entry.0) as usize & mask;
            while self.slots[position].is_some() {
                position = (position + 1) & mask;
            }
            self.slots[position] = Some(entry);
        }
        // The zero-key object remains the same owner across table changes.
        self.max_fill = max_fill(capacity);
    }
}

fn empty_slots<T>(capacity: usize) -> Vec<Option<(i64, T)>> {
    std::iter::repeat_with(|| None).take(capacity).collect()
}

fn mix(key: i64) -> u64 {
    let mut mixed = (key as u64).wrapping_mul(LONG_PHI);
    mixed ^= mixed >> 32;
    mixed ^ (mixed >> 16)
}

fn array_size(expected: usize) -> usize {
    // HashCommon divides the int by the float load factor before widening to
    // double for Math.ceil. All Blender growth points are small exact inputs.
    let required = f64::from(expected as f32 / 0.75_f32).ceil() as usize;
    let capacity = required.max(2).next_power_of_two();
    // Blender's actual collection stencil has at most 193 entries. Retain the
    // source limit rather than permitting unsupported larger private tables.
    assert!(
        capacity <= MAX_CAPACITY,
        "source long-map capacity exceeded"
    );
    capacity
}

fn max_fill(capacity: usize) -> usize {
    ((capacity as f64 * 0.75_f64).ceil() as usize).min(capacity - 1)
}
