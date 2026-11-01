//! Default fastutil LongOpenHashSet stream order and Guava32.1.2 least(k).
//!
//! Pending chunk ordering is a stream/spliterator order, not Java Iterator
//! order. TopK's quickselect partitions can reorder equal-distance chunks.

#[derive(Clone, Debug)]
pub(super) struct PendingChunks {
    keys: Vec<i64>,
    size: usize,
    zero: bool,
}
impl Default for PendingChunks {
    fn default() -> Self {
        Self {
            keys: vec![0; 33],
            size: 0,
            zero: false,
        }
    }
}
fn long_mix(key: i64) -> usize {
    let hash = (key as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let hash = hash ^ (hash >> 32);
    (hash ^ (hash >> 16)) as usize
}
fn max_fill(n: usize) -> usize {
    (n * 3).div_ceil(4).min(n - 1)
}
impl PendingChunks {
    fn n(&self) -> usize {
        self.keys.len() - 1
    }
    fn find(&self, key: i64) -> Option<usize> {
        if key == 0 {
            return self.zero.then_some(self.n());
        }
        let mut p = long_mix(key) & (self.n() - 1);
        loop {
            if self.keys[p] == key {
                return Some(p);
            }
            if self.keys[p] == 0 {
                return None;
            }
            p = (p + 1) & (self.n() - 1);
        }
    }
    pub(super) fn insert(&mut self, key: i64) -> bool {
        if self.find(key).is_some() {
            return false;
        }
        let n = self.n();
        if key == 0 {
            self.zero = true;
        } else {
            let mut p = long_mix(key) & (n - 1);
            while self.keys[p] != 0 {
                p = (p + 1) & (n - 1);
            }
            self.keys[p] = key;
        }
        let before = self.size;
        self.size += 1;
        if before >= max_fill(n) {
            self.rehash(((self.size + 1) * 4).div_ceil(3).next_power_of_two().max(2));
        }
        true
    }
    pub(super) fn remove(&mut self, key: i64) -> bool {
        let Some(mut hole) = self.find(key) else {
            return false;
        };
        self.size -= 1;
        if key == 0 {
            self.zero = false;
        } else {
            let mask = self.n() - 1;
            loop {
                let last = hole;
                hole = (hole + 1) & mask;
                loop {
                    let current = self.keys[hole];
                    if current == 0 {
                        self.keys[last] = 0;
                        break;
                    }
                    let slot = long_mix(current) & mask;
                    if if last <= hole {
                        last >= slot || slot > hole
                    } else {
                        last >= slot && slot > hole
                    } {
                        break;
                    }
                    hole = (hole + 1) & mask;
                }
                if self.keys[hole] == 0 {
                    break;
                }
                self.keys[last] = self.keys[hole];
            }
        }
        let n = self.n();
        if n > 32 && self.size < max_fill(n) / 4 {
            self.rehash(n / 2);
        }
        true
    }
    fn rehash(&mut self, n: usize) {
        let old = std::mem::replace(&mut self.keys, vec![0; n + 1]);
        for &key in old[..old.len() - 1].iter().rev() {
            if key == 0 {
                continue;
            }
            let mut p = long_mix(key) & (n - 1);
            while self.keys[p] != 0 {
                p = (p + 1) & (n - 1);
            }
            self.keys[p] = key;
        }
    }
    /// Actual sequential stream/spliterator encounter order used by arq:
    /// zero first, then nonzero keys in ascending table-slot order. Fastutil's
    /// separate Java Iterator scans descending slots and is not used here.
    pub(super) fn iter(&self) -> impl Iterator<Item = i64> + '_ {
        self.zero.then_some(0).into_iter().chain(
            self.keys[..self.n()]
                .iter()
                .copied()
                .filter(|&key| key != 0),
        )
    }
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn contains(&self, key: i64) -> bool {
        self.find(key).is_some()
    }
    pub(super) fn len(&self) -> usize {
        self.size
    }
    pub(super) fn is_empty(&self) -> bool {
        self.size == 0
    }
}

pub(super) fn distance_squared(center: (i32, i32), chunk: (i32, i32)) -> i32 {
    let x = chunk.0.wrapping_sub(center.0);
    let z = chunk.1.wrapping_sub(center.1);
    x.wrapping_mul(x).wrapping_add(z.wrapping_mul(z))
}
fn distance(center: (i32, i32), key: i64) -> i32 {
    distance_squared(center, (key as i32, ((key as u64) >> 32) as i32))
}

/// The sequential Guava collector used by arq's limited-batch branch. At most
/// 2*k candidates are buffered; final stable sorting is on that buffer only.
/// k follows Guava's constructor precondition, <= Integer.MAX_VALUE/2.
pub(super) fn least_by_distance(
    pending: impl Iterator<Item = i64>,
    k: usize,
    center: (i32, i32),
) -> Vec<i64> {
    assert!(
        k <= i32::MAX as usize / 2,
        "Guava k exceeds Integer.MAX_VALUE/2"
    );
    let mut selector = TopK {
        k,
        center,
        buffer: Vec::with_capacity(2 * k),
        threshold: 0,
        #[cfg(test)]
        trims: 0,
        #[cfg(test)]
        fallback_sorts: 0,
    };
    for key in pending {
        selector.offer(key);
    }
    selector.buffer.sort_by_key(|&key| distance(center, key));
    selector.buffer.truncate(k);
    selector.buffer
}
struct TopK {
    k: usize,
    center: (i32, i32),
    buffer: Vec<i64>,
    threshold: i32,
    #[cfg(test)]
    trims: usize,
    #[cfg(test)]
    fallback_sorts: usize,
}
impl TopK {
    fn offer(&mut self, key: i64) {
        if self.k == 0 {
            return;
        }
        let value = distance(self.center, key);
        if self.buffer.is_empty() {
            self.buffer.push(key);
            self.threshold = value;
        } else if self.buffer.len() < self.k {
            self.buffer.push(key);
            if value > self.threshold {
                self.threshold = value;
            }
        } else if value < self.threshold {
            self.buffer.push(key);
            if self.buffer.len() == 2 * self.k {
                self.trim();
            }
        }
    }
    fn partition(&mut self, left: usize, right: usize, pivot_index: usize) -> usize {
        let pivot = self.buffer[pivot_index];
        self.buffer[pivot_index] = self.buffer[right];
        let pivot_distance = distance(self.center, pivot);
        let mut pivot_new = left;
        for i in left..right {
            if distance(self.center, self.buffer[i]) < pivot_distance {
                self.buffer.swap(pivot_new, i);
                pivot_new += 1;
            }
        }
        self.buffer[right] = self.buffer[pivot_new];
        self.buffer[pivot_new] = pivot;
        pivot_new
    }
    fn trim(&mut self) {
        #[cfg(test)]
        {
            self.trims += 1;
        }
        let mut left = 0;
        let mut right = 2 * self.k - 1;
        let mut min_threshold_position = 0;
        let mut iterations = 0;
        let max_iterations = 3 * (right - left).next_power_of_two().trailing_zeros() as usize;
        while left < right {
            let pivot_index = (left + right + 1) >> 1;
            let pivot_new = self.partition(left, right, pivot_index);
            if pivot_new > self.k {
                right = pivot_new - 1;
            } else if pivot_new < self.k {
                left = pivot_new.max(left + 1);
                min_threshold_position = pivot_new;
            } else {
                break;
            }
            iterations += 1;
            if iterations >= max_iterations {
                #[cfg(test)]
                {
                    self.fallback_sorts += 1;
                }
                let center = self.center;
                self.buffer[left..=right].sort_by_key(|&key| distance(center, key));
                break;
            }
        }
        self.buffer.truncate(self.k);
        self.threshold = distance(self.center, self.buffer[min_threshold_position]);
        for &key in &self.buffer[min_threshold_position + 1..self.k] {
            self.threshold = self.threshold.max(distance(self.center, key));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn resolve<'a>(fixture: &'a Value, value: &'a Value) -> &'a Value {
        if let Some(index) = value.get("pool") {
            &fixture["pool"][index.as_u64().unwrap() as usize]
        } else {
            value
        }
    }
    fn keys(fixture: &Value, value: &Value) -> Vec<i64> {
        resolve(fixture, value)
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().parse().unwrap())
            .collect()
    }
    fn mutate(pending: &mut PendingChunks, op: &Value) -> bool {
        let key = op["key"].as_str().unwrap().parse().unwrap();
        match op["op"].as_str().unwrap() {
            "insert" => pending.insert(key),
            "remove" => pending.remove(key),
            _ => panic!("unknown mutation"),
        }
    }
    #[test]
    fn actual_default_fastutil_layout_stream_and_iterator_order() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/player_chunk_sender_order.json"))
                .unwrap();
        let mut count = 0;
        for case in fixture["cases"].as_array().unwrap() {
            if case["query"]["kind"] != "set" {
                continue;
            }
            let mut pending = PendingChunks::new();
            for (index, (op, expected)) in case["query"]["ops"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["expected"].as_array().unwrap())
                .enumerate()
            {
                let changed = mutate(&mut pending, op);
                let key = op["key"].as_str().unwrap().parse().unwrap();
                let context = format!("{} step{index} op{op}", case["name"]);
                assert_eq!(
                    changed,
                    expected["changed"].as_bool().unwrap(),
                    "{context} changed"
                );
                assert_eq!(
                    pending.contains(key),
                    expected["contains"].as_bool().unwrap(),
                    "{context} contains"
                );
                assert_eq!(
                    pending.len(),
                    expected["size"].as_u64().unwrap() as usize,
                    "{context} size"
                );
                assert_eq!(
                    pending.is_empty(),
                    expected["size"].as_u64().unwrap() == 0,
                    "{context} empty"
                );
                assert_eq!(
                    pending.n(),
                    expected["capacity"].as_u64().unwrap() as usize,
                    "{context} capacity"
                );
                assert_eq!(expected["min_capacity"], 32);
                assert_eq!(
                    max_fill(pending.n()),
                    expected["max_fill"].as_u64().unwrap() as usize
                );
                assert_eq!(pending.zero, expected["contains_zero"].as_bool().unwrap());
                assert_eq!(
                    pending.keys,
                    keys(&fixture, &expected["key_slots"]),
                    "{context} slots"
                );
                assert_eq!(
                    pending.iter().collect::<Vec<_>>(),
                    keys(&fixture, &expected["stream"]),
                    "{context} stream"
                );
                assert_eq!(
                    pending.iter().collect::<Vec<_>>(),
                    keys(&fixture, &expected["long_stream"]),
                    "{context} long_stream"
                );
                let iterator = pending
                    .zero
                    .then_some(0)
                    .into_iter()
                    .chain(
                        pending.keys[..pending.n()]
                            .iter()
                            .rev()
                            .copied()
                            .filter(|&v| v != 0),
                    )
                    .collect::<Vec<_>>();
                assert_eq!(
                    iterator,
                    keys(&fixture, &expected["iterator"]),
                    "{context} Java iterator"
                );
                count += 1;
            }
        }
        println!("original fastutil set states compared: {count}");
        assert!(count >= 2000);
    }
    #[test]
    fn actual_guava_topk_trims_tie_order_and_original_chunk_distance() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/player_chunk_sender_order.json"))
                .unwrap();
        let mut count = 0;
        let mut trims = 0;
        let mut ties = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let query = &case["query"];
            if query["kind"] != "select" {
                continue;
            }
            let expected = &case["expected"];
            let center = (
                query["center"][0].as_i64().unwrap() as i32,
                query["center"][1].as_i64().unwrap() as i32,
            );
            let k = query["k"].as_u64().unwrap() as usize;
            let input = if let Some(values) = query.get("input") {
                keys(&fixture, values)
            } else {
                let mut pending = PendingChunks::new();
                for op in query["ops"].as_array().unwrap() {
                    mutate(&mut pending, op);
                }
                pending.iter().collect::<Vec<_>>()
            };
            assert_eq!(
                input,
                keys(&fixture, &expected["input"]),
                "{} source",
                case["name"]
            );
            let actual_distances = input
                .iter()
                .map(|&key| distance(center, key))
                .collect::<Vec<_>>();
            let expected_distances = resolve(&fixture, &expected["distances"])
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect::<Vec<_>>();
            assert_eq!(
                actual_distances, expected_distances,
                "{} distances",
                case["name"]
            );
            let actual = least_by_distance(input.iter().copied(), k, center);
            assert_eq!(
                actual,
                keys(&fixture, &expected["least"]),
                "{} TopK",
                case["name"]
            );
            let mut selector = TopK {
                k,
                center,
                buffer: Vec::with_capacity(2 * k),
                threshold: 0,
                trims: 0,
                fallback_sorts: 0,
            };
            for &key in &input {
                selector.offer(key);
            }
            assert_eq!(
                selector.trims,
                expected["trim_count"].as_u64().unwrap() as usize,
                "{} trims",
                case["name"]
            );
            if let Some(observed) = expected["fallback_observed"].as_bool() {
                assert_eq!(
                    selector.fallback_sorts > 0,
                    observed,
                    "{} fallback",
                    case["name"]
                );
            }
            trims += selector.trims;
            let mut full = input;
            full.sort_by_key(|&key| distance(center, key));
            assert_eq!(
                full,
                keys(&fixture, &expected["full_sorted"]),
                "{} full sort reference",
                case["name"]
            );
            if actual != full[..full.len().min(k)] {
                ties += 1;
            }
            count += 1;
        }
        println!("original Guava selections: {count}, actual trims: {trims}, tie orders differing from full-sort prefix: {ties}");
        assert!(count >= 150 && trims > 100 && ties > 0);
    }
}
