//! Minecraft 1.21 scheduled-tick primitives, chunk queues and world clock.
//!
//! Source: the pinned vanilla classes ScheduledTick (`eyt`), TickPriority
//! (`eyx`), LevelChunkTicks (`eyo`), SavedTick (`eys`), LevelTicks (`eyq`),
//! and ClientboundSetTimePacket (`afl`). Type keys must be canonical IDs: Java
//! deduplicates by type object *identity*, rather than an arbitrary value's Eq.
//! This module implements scheduling, not scheduled block/fluid behavior or the
//! chunk loader. The caller supplies tickability and actual tick callbacks.

use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
    time::Duration,
};

pub const TICKS_PER_SECOND: u32 = 20;
pub const TICK_DURATION: Duration = Duration::from_millis(50);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(from = "i32", into = "i32")]
#[repr(i8)]
pub enum TickPriority {
    ExtremelyHigh = -3,
    VeryHigh = -2,
    High = -1,
    #[default]
    Normal = 0,
    Low = 1,
    VeryLow = 2,
    ExtremelyLow = 3,
}

impl TickPriority {
    /// TickPriority.byValue saturates outside its seven enum values.
    pub fn by_value(value: i32) -> Self {
        match value {
            i32::MIN..=-3 => Self::ExtremelyHigh,
            -2 => Self::VeryHigh,
            -1 => Self::High,
            0 => Self::Normal,
            1 => Self::Low,
            2 => Self::VeryLow,
            _ => Self::ExtremelyLow,
        }
    }
    pub fn value(self) -> i32 {
        self as i8 as i32
    }
}
impl From<i32> for TickPriority {
    fn from(value: i32) -> Self {
        Self::by_value(value)
    }
}
impl From<TickPriority> for i32 {
    fn from(value: TickPriority) -> Self {
        value.value()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl BlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
    pub fn chunk(self) -> (i32, i32) {
        (self.x >> 4, self.z >> 4)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledTick<T> {
    pub kind: T,
    pub pos: BlockPos,
    pub trigger_tick: i64,
    pub priority: TickPriority,
    pub sub_tick_order: i64,
}
impl<T> ScheduledTick<T> {
    pub fn new(
        kind: T,
        pos: BlockPos,
        trigger_tick: i64,
        priority: TickPriority,
        sub_tick_order: i64,
    ) -> Self {
        Self {
            kind,
            pos,
            trigger_tick,
            priority,
            sub_tick_order,
        }
    }
    pub fn normal(kind: T, pos: BlockPos, trigger_tick: i64, sub_tick_order: i64) -> Self {
        Self::new(
            kind,
            pos,
            trigger_tick,
            TickPriority::Normal,
            sub_tick_order,
        )
    }
    /// ScheduledTick.DRAIN_ORDER: trigger time, priority, then sub-tick order.
    pub fn drain_cmp(&self, other: &Self) -> Ordering {
        self.trigger_tick
            .cmp(&other.trigger_tick)
            .then_with(|| self.priority.cmp(&other.priority))
            .then_with(|| self.sub_tick_order.cmp(&other.sub_tick_order))
    }
    /// LevelTicks compares due container heads WITHOUT comparing trigger times.
    pub fn intra_tick_cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.sub_tick_order.cmp(&other.sub_tick_order))
    }
}

/// The fields and integer priority match vanilla's saved tick NBT compound.
/// Serde's object representation is suitable for the NBT layer's `i/x/y/z/t/p`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTick<T> {
    #[serde(rename = "i")]
    pub kind: T,
    #[serde(flatten)]
    pub pos: BlockPos,
    #[serde(rename = "t")]
    pub delay: i32,
    #[serde(rename = "p")]
    pub priority: TickPriority,
}
impl<T: Clone> SavedTick<T> {
    pub fn from_scheduled(tick: &ScheduledTick<T>, now: i64) -> Self {
        Self {
            kind: tick.kind.clone(),
            pos: tick.pos,
            delay: tick.trigger_tick.wrapping_sub(now) as i32,
            priority: tick.priority,
        }
    }
    pub fn unpack(&self, now: i64, sub_tick_order: i64) -> ScheduledTick<T> {
        ScheduledTick::new(
            self.kind.clone(),
            self.pos,
            now.wrapping_add(i64::from(self.delay)),
            self.priority,
            sub_tick_order,
        )
    }
}

/// A Java PriorityQueue-compatible min-heap. In particular, equal children choose
/// the left child, and an equal newly offered element does not bubble upward.
/// Rust's BinaryHeap does not promise these observable equal-comparator rules.
#[derive(Debug, Clone)]
struct TickHeap<T> {
    values: Vec<ScheduledTick<T>>,
}
impl<T> Default for TickHeap<T> {
    fn default() -> Self {
        Self { values: Vec::new() }
    }
}
impl<T> TickHeap<T> {
    fn push(&mut self, value: ScheduledTick<T>) {
        self.values.push(value);
        let mut index = self.values.len() - 1;
        while index > 0 {
            let parent = (index - 1) >> 1;
            if self.values[index].drain_cmp(&self.values[parent]) != Ordering::Less {
                break;
            }
            self.values.swap(index, parent);
            index = parent;
        }
    }
    fn pop(&mut self) -> Option<ScheduledTick<T>> {
        let last = self.values.pop()?;
        if self.values.is_empty() {
            return Some(last);
        }
        let result = std::mem::replace(&mut self.values[0], last);
        let mut index = 0;
        let half = self.values.len() >> 1;
        while index < half {
            let mut child = (index << 1) + 1;
            let right = child + 1;
            if right < self.values.len()
                && self.values[child].drain_cmp(&self.values[right]) == Ordering::Greater
            {
                child = right;
            }
            if self.values[index].drain_cmp(&self.values[child]) != Ordering::Greater {
                break;
            }
            self.values.swap(index, child);
            index = child;
        }
        Some(result)
    }
}

impl<T: Clone> TickHeap<T> {
    /// PriorityQueue.removeAt, including its moved-up value for Iterator.remove.
    fn remove_at(&mut self, index: usize) -> (ScheduledTick<T>, Option<ScheduledTick<T>>) {
        let last = self.values.pop().expect("known queue index");
        if index == self.values.len() {
            return (last, None);
        }
        let removed = std::mem::replace(&mut self.values[index], last);
        let mut cursor = index;
        let half = self.values.len() >> 1;
        while cursor < half {
            let mut child = (cursor << 1) + 1;
            if child + 1 < self.values.len()
                && self.values[child].drain_cmp(&self.values[child + 1]) == Ordering::Greater
            {
                child += 1;
            }
            if self.values[cursor].drain_cmp(&self.values[child]) != Ordering::Greater {
                break;
            }
            self.values.swap(cursor, child);
            cursor = child;
        }
        if cursor == index {
            while cursor > 0 {
                let parent = (cursor - 1) >> 1;
                if self.values[cursor].drain_cmp(&self.values[parent]) != Ordering::Less {
                    break;
                }
                self.values.swap(cursor, parent);
                cursor = parent;
            }
            if cursor != index {
                return (removed, Some(self.values[cursor].clone()));
            }
        }
        (removed, None)
    }
}

#[derive(Debug, Clone)]
pub struct ChunkTickQueue<T> {
    queue: TickHeap<T>,
    pending: Option<Vec<SavedTick<T>>>,
    unique: HashSet<(BlockPos, T)>,
}
impl<T> Default for ChunkTickQueue<T> {
    fn default() -> Self {
        Self {
            queue: TickHeap::default(),
            pending: None,
            unique: HashSet::new(),
        }
    }
}
impl<T: Clone + Eq + Hash> ChunkTickQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }
    /// Loaded ticks are reserved immediately, but are not queued until unpack.
    pub fn from_saved(pending: Vec<SavedTick<T>>) -> Self {
        let unique = pending
            .iter()
            .map(|tick| (tick.pos, tick.kind.clone()))
            .collect();
        Self {
            queue: TickHeap::default(),
            pending: Some(pending),
            unique,
        }
    }
    /// LevelChunkTicks.load filters resolved saved ticks to their owning chunk.
    /// Unknown type IDs must already have been omitted by the registry resolver.
    pub fn from_saved_for_chunk(chunk: (i32, i32), pending: Vec<SavedTick<T>>) -> Self {
        Self::from_saved(
            pending
                .into_iter()
                .filter(|tick| tick.pos.chunk() == chunk)
                .collect(),
        )
    }
    pub fn count(&self) -> usize {
        self.queue.values.len() + self.pending.as_ref().map_or(0, Vec::len)
    }
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }
    pub fn peek(&self) -> Option<&ScheduledTick<T>> {
        self.queue.values.first()
    }
    pub fn has_scheduled_tick(&self, pos: BlockPos, kind: &T) -> bool {
        self.unique.contains(&(pos, kind.clone()))
    }
    /// The first tick for a canonical (position, type) wins, even when a later
    /// duplicate requests an earlier time or a higher priority.
    pub fn schedule(&mut self, tick: ScheduledTick<T>) -> bool {
        if !self.unique.insert((tick.pos, tick.kind.clone())) {
            return false;
        }
        self.queue.push(tick);
        true
    }
    pub fn poll(&mut self) -> Option<ScheduledTick<T>> {
        let tick = self.queue.pop()?;
        self.unique.remove(&(tick.pos, tick.kind.clone()));
        Some(tick)
    }
    pub fn poll_due(&mut self, now: i64) -> Option<ScheduledTick<T>> {
        if self.peek()?.trigger_tick > now {
            return None;
        }
        self.poll()
    }
    /// Preserve PriorityQueue iterator order, rather than sorting saved ticks.
    /// This matters because unpack assigns increasing negative sub-tick orders.
    pub fn saved_ticks(&self, now: i64) -> Vec<SavedTick<T>> {
        let mut result = self.pending.clone().unwrap_or_default();
        result.extend(
            self.queue
                .values
                .iter()
                .map(|tick| SavedTick::from_scheduled(tick, now)),
        );
        result
    }
    pub fn unpack(&mut self, now: i64) {
        if let Some(pending) = self.pending.take() {
            let mut order = (pending.len() as i32).wrapping_neg();
            for tick in pending {
                self.queue.push(tick.unpack(now, i64::from(order)));
                order = order.wrapping_add(1);
            }
        }
    }
    pub fn remove_if(&mut self, mut predicate: impl FnMut(&ScheduledTick<T>) -> bool) {
        // eyo.removeIf uses PriorityQueue's iterator, not bulk removeIf/heapify.
        let mut cursor = 0;
        let mut moved_up = VecDeque::new();
        while cursor < self.queue.values.len() {
            if predicate(&self.queue.values[cursor]) {
                let (removed, moved) = self.queue.remove_at(cursor);
                self.unique.remove(&(removed.pos, removed.kind));
                if let Some(moved) = moved {
                    moved_up.push_back(moved);
                    cursor += 1;
                }
            } else {
                cursor += 1;
            }
        }
        for tick in moved_up {
            if predicate(&tick) {
                if let Some(index) = self.queue.values.iter().position(|value| *value == tick) {
                    let (removed, _) = self.queue.remove_at(index);
                    self.unique.remove(&(removed.pos, removed.kind));
                }
            }
        }
    }
}

/// fastutil's primitive map layout is observable when equal-priority/order
/// container heads enter the Java PriorityQueue. Keep that iteration behavior.
#[derive(Debug, Clone)]
struct NextTickMap {
    keys: Vec<i64>,
    values: Vec<i64>,
    size: usize,
    has_zero: bool,
}
#[derive(Debug)]
struct MapIterator {
    position: isize,
    last: Option<usize>,
    remaining: usize,
    zero: bool,
    wrapped: Vec<i64>,
}
impl Default for NextTickMap {
    fn default() -> Self {
        Self {
            keys: vec![0; 33],
            values: vec![0; 33],
            size: 0,
            has_zero: false,
        }
    }
}
impl NextTickMap {
    fn n(&self) -> usize {
        self.keys.len() - 1
    }
    fn mix(key: i64) -> usize {
        let mut h = (key as u64).wrapping_mul(0x9e3779b97f4a7c15);
        h ^= h >> 32;
        (h ^ (h >> 16)) as usize
    }
    fn fill(n: usize) -> usize {
        (n * 3).div_ceil(4).min(n - 1)
    }
    fn index(&self, key: i64) -> Option<usize> {
        if key == 0 {
            return self.has_zero.then_some(self.n());
        }
        let mut p = Self::mix(key) & (self.n() - 1);
        loop {
            if self.keys[p] == 0 {
                return None;
            }
            if self.keys[p] == key {
                return Some(p);
            }
            p = (p + 1) & (self.n() - 1);
        }
    }
    fn put(&mut self, key: i64, value: i64) {
        if let Some(index) = self.index(key) {
            self.values[index] = value;
            return;
        }
        let n = self.n();
        let mut p = if key == 0 {
            self.has_zero = true;
            n
        } else {
            Self::mix(key) & (n - 1)
        };
        if key != 0 {
            while self.keys[p] != 0 {
                p = (p + 1) & (n - 1);
            }
            self.keys[p] = key;
        }
        self.values[p] = value;
        let previous = self.size;
        self.size += 1;
        if previous >= Self::fill(n) {
            let expected = self.size + 1;
            self.rehash((expected * 4).div_ceil(3).next_power_of_two().max(2));
        }
    }
    fn rehash(&mut self, n: usize) {
        let old_n = self.n();
        let old_keys = std::mem::replace(&mut self.keys, vec![0; n + 1]);
        let old_values = std::mem::replace(&mut self.values, vec![0; n + 1]);
        self.values[n] = old_values[old_n];
        for i in (0..old_n).rev() {
            let key = old_keys[i];
            if key != 0 {
                let mut p = Self::mix(key) & (n - 1);
                while self.keys[p] != 0 {
                    p = (p + 1) & (n - 1);
                }
                self.keys[p] = key;
                self.values[p] = old_values[i];
            }
        }
    }
    fn shift_keys(&mut self, mut hole: usize, mut wrapped: Option<&mut Vec<i64>>) {
        let mask = self.n() - 1;
        loop {
            let last = hole;
            hole = (hole + 1) & mask;
            loop {
                let current = self.keys[hole];
                if current == 0 {
                    self.keys[last] = 0;
                    return;
                }
                let slot = Self::mix(current) & mask;
                if if last <= hole {
                    last >= slot || slot > hole
                } else {
                    last >= slot && slot > hole
                } {
                    break;
                }
                hole = (hole + 1) & mask;
            }
            if hole < last {
                if let Some(keys) = wrapped.as_deref_mut() {
                    keys.push(self.keys[hole]);
                }
            }
            self.keys[last] = self.keys[hole];
            self.values[last] = self.values[hole];
        }
    }
    fn remove(&mut self, key: i64) {
        if let Some(p) = self.index(key) {
            if key == 0 {
                self.has_zero = false;
            } else {
                self.shift_keys(p, None);
            }
            self.size -= 1;
            let n = self.n();
            if n > 32 && self.size < Self::fill(n) / 4 {
                self.rehash(n / 2);
            }
        }
    }
    fn iterator(&self) -> MapIterator {
        MapIterator {
            position: self.n() as isize,
            last: None,
            remaining: self.size,
            zero: self.has_zero,
            wrapped: Vec::new(),
        }
    }
    fn next(&self, iter: &mut MapIterator) -> Option<usize> {
        if iter.remaining == 0 {
            return None;
        }
        iter.remaining -= 1;
        if iter.zero {
            iter.zero = false;
            iter.last = Some(self.n());
            return iter.last;
        }
        loop {
            iter.position -= 1;
            if iter.position < 0 {
                let key = iter.wrapped[(-iter.position - 1) as usize];
                let index = self.index(key).expect("wrapped map key");
                iter.last = Some(index);
                return Some(index);
            }
            let p = iter.position as usize;
            if self.keys[p] != 0 {
                iter.last = Some(p);
                return Some(p);
            }
        }
    }
    fn remove_current(&mut self, iter: &mut MapIterator) {
        let last = iter.last.take().expect("current map entry");
        if last == self.n() {
            self.has_zero = false;
            self.size -= 1;
        } else if iter.position >= 0 {
            self.shift_keys(last, Some(&mut iter.wrapped));
            self.size -= 1;
        } else {
            let key = iter.wrapped[(-iter.position - 1) as usize];
            self.remove(key);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockBounds {
    pub min: BlockPos,
    pub max: BlockPos,
}
impl BlockBounds {
    pub fn contains(self, pos: BlockPos) -> bool {
        pos.x >= self.min.x
            && pos.x <= self.max.x
            && pos.y >= self.min.y
            && pos.y <= self.max.y
            && pos.z >= self.min.z
            && pos.z <= self.max.z
    }
}
fn chunk_key(chunk: (i32, i32)) -> i64 {
    ((chunk.0 as u32 as u64) | ((chunk.1 as u32 as u64) << 32)) as i64
}
fn key_chunk(key: i64) -> (i32, i32) {
    (key as i32, ((key as u64) >> 32) as i32)
}

/// LevelTicks collection/run phases. A caller supplies the actual tickability
/// predicate, then implements block/fluid behavior in the callback. Rescheduling
/// from a callback is deferred to a subsequent tick, as in vanilla.
#[derive(Debug, Clone)]
pub struct LevelTickScheduler<T> {
    chunks: HashMap<i64, ChunkTickQueue<T>>,
    next_times: NextTickMap,
    containers: Vec<i64>,
    to_run: VecDeque<ScheduledTick<T>>,
    already_run: Vec<ScheduledTick<T>>,
    to_run_set: HashSet<(BlockPos, T)>,
}
impl<T> Default for LevelTickScheduler<T> {
    fn default() -> Self {
        Self {
            chunks: HashMap::new(),
            next_times: NextTickMap::default(),
            containers: Vec::new(),
            to_run: VecDeque::new(),
            already_run: Vec::new(),
            to_run_set: HashSet::new(),
        }
    }
}
impl<T: Clone + Eq + Hash> LevelTickScheduler<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn chunk(&self, chunk: (i32, i32)) -> Option<&ChunkTickQueue<T>> {
        self.chunks.get(&chunk_key(chunk))
    }
    /// Diagnostic order of the Java-compatible next-tick map, including ties.
    pub fn next_tick_deadlines(&self) -> Vec<((i32, i32), i64)> {
        let mut result = Vec::new();
        let mut iter = self.next_times.iterator();
        while let Some(p) = self.next_times.next(&mut iter) {
            let key = if p == self.next_times.n() {
                0
            } else {
                self.next_times.keys[p]
            };
            result.push((key_chunk(key), self.next_times.values[p]));
        }
        result
    }
    pub fn add_chunk(
        &mut self,
        chunk: (i32, i32),
        queue: ChunkTickQueue<T>,
    ) -> Option<ChunkTickQueue<T>> {
        let key = chunk_key(chunk);
        if let Some(tick) = queue.peek() {
            self.next_times.put(key, tick.trigger_tick);
        }
        self.chunks.insert(key, queue)
    }
    pub fn remove_chunk(&mut self, chunk: (i32, i32)) -> Option<ChunkTickQueue<T>> {
        let key = chunk_key(chunk);
        self.next_times.remove(key);
        self.chunks.remove(&key)
    }
    pub fn unpack_chunk(&mut self, chunk: (i32, i32), now: i64) {
        let key = chunk_key(chunk);
        if let Some(queue) = self.chunks.get_mut(&key) {
            let pending = queue.pending.is_some();
            queue.unpack(now);
            if pending {
                if let Some(head) = queue.peek() {
                    self.next_times.put(key, head.trigger_tick);
                }
            }
        }
    }
    pub fn count(&self) -> usize {
        self.chunks.values().map(ChunkTickQueue::count).sum()
    }
    pub fn has_scheduled_tick(&self, pos: BlockPos, kind: &T) -> bool {
        self.chunks
            .get(&chunk_key(pos.chunk()))
            .is_some_and(|q| q.has_scheduled_tick(pos, kind))
    }
    pub fn will_tick_this_tick(&mut self, pos: BlockPos, kind: &T) -> bool {
        if self.to_run_set.is_empty() && !self.to_run.is_empty() {
            self.to_run_set
                .extend(self.to_run.iter().map(|t| (t.pos, t.kind.clone())));
        }
        self.to_run_set.contains(&(pos, kind.clone()))
    }
    pub fn schedule(&mut self, tick: ScheduledTick<T>) -> bool {
        let key = chunk_key(tick.pos.chunk());
        let Some(queue) = self.chunks.get_mut(&key) else {
            return false;
        };
        if !queue.schedule(tick.clone()) {
            return false;
        }
        if queue.peek() == Some(&tick) {
            self.next_times.put(key, tick.trigger_tick);
        }
        true
    }
    fn container_cmp(&self, a: i64, b: i64) -> Ordering {
        self.chunks[&a]
            .peek()
            .expect("due container head")
            .intra_tick_cmp(self.chunks[&b].peek().expect("due container head"))
    }
    fn push_container(&mut self, key: i64) {
        self.containers.push(key);
        let mut p = self.containers.len() - 1;
        while p > 0 {
            let parent = (p - 1) >> 1;
            if self.container_cmp(self.containers[p], self.containers[parent]) != Ordering::Less {
                break;
            }
            self.containers.swap(p, parent);
            p = parent;
        }
    }
    fn pop_container(&mut self) -> Option<i64> {
        let last = self.containers.pop()?;
        if self.containers.is_empty() {
            return Some(last);
        }
        let first = std::mem::replace(&mut self.containers[0], last);
        let mut p = 0;
        let half = self.containers.len() >> 1;
        while p < half {
            let mut c = (p << 1) + 1;
            if c + 1 < self.containers.len()
                && self.container_cmp(self.containers[c], self.containers[c + 1])
                    == Ordering::Greater
            {
                c += 1;
            }
            if self.container_cmp(self.containers[p], self.containers[c]) != Ordering::Greater {
                break;
            }
            self.containers.swap(p, c);
            p = c;
        }
        Some(first)
    }
    fn collect(&mut self, now: i64, max: usize, mut tickable: impl FnMut((i32, i32)) -> bool) {
        let mut iter = self.next_times.iterator();
        while let Some(index) = self.next_times.next(&mut iter) {
            if self.next_times.values[index] > now {
                continue;
            }
            let key = if index == self.next_times.n() {
                0
            } else {
                self.next_times.keys[index]
            };
            let head = self.chunks.get(&key).and_then(ChunkTickQueue::peek);
            match head {
                None => self.next_times.remove_current(&mut iter),
                Some(tick) if tick.trigger_tick > now => {
                    self.next_times.values[index] = tick.trigger_tick
                }
                Some(_) if tickable(key_chunk(key)) => {
                    self.next_times.remove_current(&mut iter);
                    self.push_container(key);
                }
                _ => {}
            }
        }
        while self.to_run.len() < max {
            let Some(key) = self.pop_container() else {
                break;
            };
            let first = self
                .chunks
                .get_mut(&key)
                .expect("registered container")
                .poll()
                .expect("due head");
            self.to_run.push_back(first);
            let next_other = self
                .containers
                .first()
                .and_then(|k| self.chunks[k].peek())
                .cloned();
            while self.to_run.len() < max {
                let Some(head) = self.chunks[&key].peek() else {
                    break;
                };
                if head.trigger_tick > now {
                    break;
                }
                if next_other
                    .as_ref()
                    .is_some_and(|other| head.intra_tick_cmp(other) == Ordering::Greater)
                {
                    break;
                }
                let tick = self.chunks.get_mut(&key).unwrap().poll().unwrap();
                self.to_run.push_back(tick);
            }
            if let Some(head) = self.chunks[&key].peek() {
                if head.trigger_tick <= now && self.to_run.len() < max {
                    self.push_container(key);
                } else {
                    self.next_times.put(key, head.trigger_tick);
                }
            }
        }
        for &key in &self.containers {
            if let Some(head) = self.chunks[&key].peek() {
                self.next_times.put(key, head.trigger_tick);
            }
        }
    }
    pub fn tick(
        &mut self,
        now: i64,
        max: usize,
        tickable: impl FnMut((i32, i32)) -> bool,
        mut callback: impl FnMut(&mut Self, &ScheduledTick<T>),
    ) {
        self.collect(now, max, tickable);
        while let Some(tick) = self.to_run.pop_front() {
            if !self.to_run_set.is_empty() {
                self.to_run_set.remove(&(tick.pos, tick.kind.clone()));
            }
            self.already_run.push(tick.clone());
            callback(self, &tick);
        }
        self.to_run.clear();
        self.containers.clear();
        self.already_run.clear();
        self.to_run_set.clear();
    }
    pub fn drain_due(
        &mut self,
        now: i64,
        max: usize,
        tickable: impl FnMut((i32, i32)) -> bool,
    ) -> Vec<ScheduledTick<T>> {
        let mut result = Vec::new();
        self.tick(now, max, tickable, |_, tick| result.push(tick.clone()));
        result
    }
    pub fn clear_area(&mut self, bounds: BlockBounds) {
        for x in (bounds.min.x >> 4)..=(bounds.max.x >> 4) {
            for z in (bounds.min.z >> 4)..=(bounds.max.z >> 4) {
                let key = chunk_key((x, z));
                if let Some(queue) = self.chunks.get_mut(&key) {
                    let before = queue.peek().cloned();
                    queue.remove_if(|tick| bounds.contains(tick.pos));
                    let after = queue.peek();
                    if before.as_ref() != after {
                        if let Some(head) = after {
                            self.next_times.put(key, head.trigger_tick);
                        } else {
                            self.next_times.remove(key);
                        }
                    }
                }
            }
        }
        self.already_run.retain(|tick| !bounds.contains(tick.pos));
        self.to_run.retain(|tick| !bounds.contains(tick.pos));
        // Vanilla deliberately does not invalidate an already-built toRunThisTickSet.
    }
    fn area_ticks(&self, bounds: BlockBounds) -> Vec<ScheduledTick<T>> {
        let mut result = self
            .already_run
            .iter()
            .chain(self.to_run.iter())
            .filter(|t| bounds.contains(t.pos))
            .cloned()
            .collect::<Vec<_>>();
        for x in (bounds.min.x >> 4)..=(bounds.max.x >> 4) {
            for z in (bounds.min.z >> 4)..=(bounds.max.z >> 4) {
                if let Some(queue) = self.chunks.get(&chunk_key((x, z))) {
                    result.extend(
                        queue
                            .queue
                            .values
                            .iter()
                            .filter(|t| bounds.contains(t.pos))
                            .cloned(),
                    );
                }
            }
        }
        result
    }
    fn copy_ticks(&mut self, ticks: Vec<ScheduledTick<T>>, offset: BlockPos) -> usize {
        let Some(min) = ticks.iter().map(|t| t.sub_tick_order).min() else {
            return 0;
        };
        let max = ticks.iter().map(|t| t.sub_tick_order).max().unwrap();
        let mut copied = 0;
        for tick in ticks {
            let pos = BlockPos::new(
                tick.pos.x.wrapping_add(offset.x),
                tick.pos.y.wrapping_add(offset.y),
                tick.pos.z.wrapping_add(offset.z),
            );
            let order = tick
                .sub_tick_order
                .wrapping_sub(min)
                .wrapping_add(max)
                .wrapping_add(1);
            copied += usize::from(self.schedule(ScheduledTick::new(
                tick.kind,
                pos,
                tick.trigger_tick,
                tick.priority,
                order,
            )));
        }
        copied
    }
    pub fn copy_area(&mut self, bounds: BlockBounds, offset: BlockPos) -> usize {
        let ticks = self.area_ticks(bounds);
        self.copy_ticks(ticks, offset)
    }
    pub fn copy_area_from(
        &mut self,
        source: &Self,
        bounds: BlockBounds,
        offset: BlockPos,
    ) -> usize {
        self.copy_ticks(source.area_ticks(bounds), offset)
    }
}

/// Registered chunk admission and tickability gates, independent of a loader.
/// Missing containers reject scheduling. Registered non-tickable chunks retain
/// their overdue ticks until the caller makes them tickable again.
#[derive(Debug, Clone)]
pub struct LoadedChunkTicks<T> {
    chunks: HashMap<(i32, i32), ChunkTickQueue<T>>,
}
impl<T> Default for LoadedChunkTicks<T> {
    fn default() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }
}
impl<T: Clone + Eq + Hash> LoadedChunkTicks<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_chunk(
        &mut self,
        chunk: (i32, i32),
        ticks: ChunkTickQueue<T>,
    ) -> Option<ChunkTickQueue<T>> {
        self.chunks.insert(chunk, ticks)
    }
    pub fn remove_chunk(&mut self, chunk: (i32, i32)) -> Option<ChunkTickQueue<T>> {
        self.chunks.remove(&chunk)
    }
    pub fn chunk(&self, chunk: (i32, i32)) -> Option<&ChunkTickQueue<T>> {
        self.chunks.get(&chunk)
    }
    pub fn chunk_mut(&mut self, chunk: (i32, i32)) -> Option<&mut ChunkTickQueue<T>> {
        self.chunks.get_mut(&chunk)
    }
    pub fn schedule(&mut self, tick: ScheduledTick<T>) -> bool {
        self.chunks
            .get_mut(&tick.pos.chunk())
            .is_some_and(|chunk| chunk.schedule(tick))
    }
    pub fn poll_due_in_chunk(
        &mut self,
        chunk: (i32, i32),
        now: i64,
        tickable: bool,
    ) -> Option<ScheduledTick<T>> {
        if !tickable {
            return None;
        }
        self.chunks.get_mut(&chunk)?.poll_due(now)
    }
}

/// Level.getBlockRandomPos's independent 32-bit LCG. This is separate from
/// RandomSource: consuming world random samples must not advance this selector.
/// The caller retains the value between chunk/section random-tick selections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRandomSelector {
    state: i32,
}
impl BlockRandomSelector {
    pub const fn new(state: i32) -> Self {
        Self { state }
    }
    pub const fn state(&self) -> i32 {
        self.state
    }
    pub fn next_position(&mut self, origin: BlockPos, y_mask: i32) -> BlockPos {
        self.state = self.state.wrapping_mul(3).wrapping_add(1_013_904_223);
        let bits = self.state >> 2;
        BlockPos::new(
            origin.x.wrapping_add(bits & 15),
            origin.y.wrapping_add((bits >> 16) & y_mask),
            origin.z.wrapping_add((bits >> 8) & 15),
        )
    }
}

/// Route of ServerLevel's weather packets. Intensity updates go to the
/// affected dimension; threshold start/stop updates use vanilla's global path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherAudience {
    Dimension,
    AllPlayers,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherChange {
    pub audience: WeatherAudience,
    /// ClientboundGameEventPacket reason: 1 start, 2 stop, 7 rain, 8 thunder.
    pub reason: u8,
    pub value: f32,
}

/// ServerLevel weather timers and Level's interpolated weather strengths. World
/// thunder/precipitation effects (lightning rods, ice/snow and entities) require
/// the chunk and biome simulation and are separate from this state machine.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeatherState {
    pub clear_weather_time: i32,
    pub thunder_time: i32,
    pub rain_time: i32,
    pub thundering: bool,
    pub raining: bool,
    pub old_thunder_level: f32,
    pub thunder_level: f32,
    pub old_rain_level: f32,
    pub rain_level: f32,
}
impl WeatherState {
    pub fn set_weather_parameters(
        &mut self,
        clear_time: i32,
        weather_time: i32,
        raining: bool,
        thundering: bool,
    ) {
        self.clear_weather_time = clear_time;
        self.rain_time = weather_time;
        self.thunder_time = weather_time;
        self.raining = raining;
        self.thundering = thundering;
    }
    /// Sleeping resets timers and flags without snapping the interpolated levels.
    pub fn reset_weather_cycle(&mut self) {
        self.rain_time = 0;
        self.raining = false;
        self.thunder_time = 0;
        self.thundering = false;
    }
    pub fn prepare_weather(&mut self) {
        if self.raining {
            self.rain_level = 1.0;
            if self.thundering {
                self.thunder_level = 1.0;
            }
        }
    }
    pub fn rain_level_at(&self, partial_tick: f32) -> f32 {
        self.old_rain_level + partial_tick * (self.rain_level - self.old_rain_level)
    }
    pub fn thunder_level_at(&self, partial_tick: f32) -> f32 {
        (self.old_thunder_level + partial_tick * (self.thunder_level - self.old_thunder_level))
            * self.rain_level_at(partial_tick)
    }
    pub fn is_raining(&self) -> bool {
        f64::from(self.rain_level_at(1.0)) > 0.2
    }
    pub fn is_thundering(&self, has_skylight: bool, has_ceiling: bool) -> bool {
        has_skylight && !has_ceiling && f64::from(self.thunder_level_at(1.0)) > 0.9
    }
    /// `sample_inclusive` must draw a uniform integer in the inclusive interval
    /// using the world's RandomSource. Calls occur in Java's thunder/rain order.
    pub fn advance(
        &mut self,
        has_skylight: bool,
        weather_cycle: bool,
        mut sample_inclusive: impl FnMut(i32, i32) -> i32,
    ) -> Vec<WeatherChange> {
        let was_raining = self.is_raining();
        if has_skylight {
            if weather_cycle {
                if self.clear_weather_time > 0 {
                    self.clear_weather_time -= 1;
                    self.thunder_time = i32::from(!self.thundering);
                    self.rain_time = i32::from(!self.raining);
                    self.thundering = false;
                    self.raining = false;
                } else {
                    if self.thunder_time > 0 {
                        self.thunder_time -= 1;
                        if self.thunder_time == 0 {
                            self.thundering = !self.thundering;
                        }
                    } else {
                        self.thunder_time = if self.thundering {
                            sample_inclusive(3_600, 15_600)
                        } else {
                            sample_inclusive(12_000, 180_000)
                        };
                    }
                    if self.rain_time > 0 {
                        self.rain_time -= 1;
                        if self.rain_time == 0 {
                            self.raining = !self.raining;
                        }
                    } else {
                        self.rain_time = if self.raining {
                            sample_inclusive(12_000, 24_000)
                        } else {
                            sample_inclusive(12_000, 180_000)
                        };
                    }
                }
            }
            self.old_thunder_level = self.thunder_level;
            self.thunder_level = (self.thunder_level
                + if self.thundering { 0.01_f32 } else { -0.01_f32 })
            .clamp(0.0, 1.0);
            self.old_rain_level = self.rain_level;
            self.rain_level =
                (self.rain_level + if self.raining { 0.01_f32 } else { -0.01_f32 }).clamp(0.0, 1.0);
        }
        let mut changes = Vec::with_capacity(5);
        if self.old_rain_level != self.rain_level {
            changes.push(WeatherChange {
                audience: WeatherAudience::Dimension,
                reason: 7,
                value: self.rain_level,
            });
        }
        if self.old_thunder_level != self.thunder_level {
            changes.push(WeatherChange {
                audience: WeatherAudience::Dimension,
                reason: 8,
                value: self.thunder_level,
            });
        }
        if was_raining != self.is_raining() {
            changes.push(WeatherChange {
                audience: WeatherAudience::AllPlayers,
                reason: if was_raining { 2 } else { 1 },
                value: 0.0,
            });
            changes.push(WeatherChange {
                audience: WeatherAudience::AllPlayers,
                reason: 7,
                value: self.rain_level,
            });
            changes.push(WeatherChange {
                audience: WeatherAudience::AllPlayers,
                reason: 8,
                value: self.thunder_level,
            });
        }
        changes
    }
    pub fn advance_with_legacy_random(
        &mut self,
        has_skylight: bool,
        weather_cycle: bool,
        random: &mut crate::random::LegacyRandomSource,
    ) -> Vec<WeatherChange> {
        self.advance(has_skylight, weather_cycle, |min, max| {
            min + random
                .next_bounded_i32(max - min + 1)
                .expect("positive vanilla weather interval")
        })
    }
}

/// Vanilla TickRateManager (`brc`). State changes become visible to
/// runs_normally on the next tick, including the last requested step.
/// Server sprint wall-clock accounting belongs to ServerTickRateManager and is
/// deliberately not represented by this state machine.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TickRateManager {
    tick_rate: f32,
    nanoseconds_per_tick: i64,
    frozen_ticks_to_run: i32,
    run_game_elements: bool,
    frozen: bool,
}
impl Default for TickRateManager {
    fn default() -> Self {
        Self {
            tick_rate: 20.0,
            nanoseconds_per_tick: 50_000_000,
            frozen_ticks_to_run: 0,
            run_game_elements: true,
            frozen: false,
        }
    }
}
impl TickRateManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_tick_rate(&mut self, tick_rate: f32) {
        // Math.max propagates NaN, unlike Rust's f32::max. f64 -> i64 casts
        // saturate (and map NaN to zero), just like Java's d2l instruction.
        self.tick_rate = if tick_rate.is_nan() {
            tick_rate
        } else {
            tick_rate.max(1.0)
        };
        self.nanoseconds_per_tick = (1_000_000_000.0_f64 / f64::from(self.tick_rate)) as i64;
    }
    pub const fn tick_rate(&self) -> f32 {
        self.tick_rate
    }
    pub fn milliseconds_per_tick(&self) -> f32 {
        self.nanoseconds_per_tick as f32 / 1_000_000.0_f32
    }
    pub const fn nanoseconds_per_tick(&self) -> i64 {
        self.nanoseconds_per_tick
    }
    pub const fn runs_normally(&self) -> bool {
        self.run_game_elements
    }
    pub const fn is_stepping_forward(&self) -> bool {
        self.frozen_ticks_to_run > 0
    }
    pub fn set_frozen_ticks_to_run(&mut self, ticks: i32) {
        self.frozen_ticks_to_run = ticks;
    }
    pub const fn frozen_ticks_to_run(&self) -> i32 {
        self.frozen_ticks_to_run
    }
    pub fn set_frozen(&mut self, frozen: bool) {
        self.frozen = frozen;
    }
    pub const fn is_frozen(&self) -> bool {
        self.frozen
    }
    pub fn tick(&mut self) {
        self.run_game_elements = !self.frozen || self.frozen_ticks_to_run > 0;
        if self.frozen_ticks_to_run > 0 {
            self.frozen_ticks_to_run -= 1;
        }
    }
    /// Players and any vehicle with player passengers keep ticking while frozen.
    pub fn is_entity_frozen(&self, is_player: bool, player_passengers: i32) -> bool {
        !self.runs_normally() && !is_player && player_passengers <= 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickClock {
    pub game_time: i64,
    pub day_time: i64,
    pub daylight_cycle: bool,
}
impl Default for TickClock {
    fn default() -> Self {
        Self::new(0, 0, true)
    }
}
impl TickClock {
    pub const fn new(game_time: i64, day_time: i64, daylight_cycle: bool) -> Self {
        Self {
            game_time,
            day_time,
            daylight_cycle,
        }
    }
    /// Advance one world tick. Pausing the server means not calling advance.
    /// Freezing doDaylightCycle does not freeze the monotonically advancing age.
    pub fn advance(&mut self) {
        self.game_time = self.game_time.wrapping_add(1);
        if self.daylight_cycle {
            self.day_time = self.day_time.wrapping_add(1);
        }
    }
    pub fn time_update_payload(&self) -> [u8; 16] {
        let mut day = self.day_time;
        if !self.daylight_cycle {
            day = day.wrapping_neg();
            if day == 0 {
                day = -1;
            }
        }
        let mut result = [0; 16];
        result[..8].copy_from_slice(&self.game_time.to_be_bytes());
        result[8..].copy_from_slice(&day.to_be_bytes());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tick(kind: u32, x: i32, time: i64, priority: i32, order: i64) -> ScheduledTick<u32> {
        ScheduledTick::new(
            kind,
            BlockPos::new(x, 64, 0),
            time,
            TickPriority::by_value(priority),
            order,
        )
    }
    #[test]
    fn priority_clamps_and_negative_chunk_coordinates_match_java() {
        assert_eq!(TickPriority::by_value(i32::MIN).value(), -3);
        assert_eq!(TickPriority::by_value(i32::MAX).value(), 3);
        for n in -3..=3 {
            assert_eq!(TickPriority::by_value(n).value(), n);
        }
        assert_eq!(BlockPos::new(-1, 0, -17).chunk(), (-1, -2));
    }
    #[test]
    fn chunk_queue_orders_time_then_priority_then_signed_subtick() {
        let mut queue = ChunkTickQueue::new();
        for value in [
            tick(1, 1, 10, 0, 0),
            tick(1, 2, 9, 3, 10),
            tick(1, 3, 10, -3, 9),
            tick(1, 4, 10, -3, -2),
        ] {
            assert!(queue.schedule(value));
        }
        let xs: Vec<_> = std::iter::from_fn(|| queue.poll())
            .map(|t| t.pos.x)
            .collect();
        assert_eq!(xs, [2, 4, 3, 1]);
    }
    #[test]
    fn first_position_type_wins_and_poll_releases_reservation() {
        let mut queue = ChunkTickQueue::new();
        assert!(queue.schedule(tick(1, 2, 100, 3, 0)));
        assert!(!queue.schedule(tick(1, 2, 0, -3, -1)));
        assert!(queue.schedule(tick(2, 2, 0, -3, -1)));
        assert_eq!(queue.count(), 2);
        let first = queue.poll().unwrap();
        assert_eq!(first.kind, 2);
        assert!(!queue.has_scheduled_tick(first.pos, &2));
        assert!(queue.has_scheduled_tick(first.pos, &1));
        assert!(queue.schedule(tick(2, 2, 0, 0, 10)));
    }
    #[test]
    fn equal_comparator_java_heap_behavior_is_retained() {
        let mut queue = ChunkTickQueue::new();
        for x in 0..6 {
            queue.schedule(tick(1, x, 10, 0, 0));
        }
        let xs: Vec<_> = std::iter::from_fn(|| queue.poll())
            .map(|t| t.pos.x)
            .collect();
        assert_eq!(xs, [0, 5, 4, 3, 2, 1]);
    }
    #[test]
    fn loaded_pending_ticks_reserve_before_unpack_and_use_negative_orders() {
        let saved = vec![
            SavedTick {
                kind: 1,
                pos: BlockPos::new(1, 2, 3),
                delay: 5,
                priority: TickPriority::Normal,
            },
            SavedTick {
                kind: 2,
                pos: BlockPos::new(1, 2, 3),
                delay: 5,
                priority: TickPriority::Normal,
            },
        ];
        let mut queue = ChunkTickQueue::from_saved(saved.clone());
        assert_eq!(queue.count(), 2);
        assert!(queue.peek().is_none());
        assert!(!queue.schedule(ScheduledTick::normal(1, BlockPos::new(1, 2, 3), 0, 0)));
        assert_eq!(queue.saved_ticks(999), saved);
        queue.unpack(10);
        assert!(queue.poll_due(14).is_none());
        let first = queue.poll_due(15).unwrap();
        assert_eq!(
            (first.kind, first.trigger_tick, first.sub_tick_order),
            (1, 15, -2)
        );
        assert_eq!(queue.poll().unwrap().sub_tick_order, -1);
    }
    #[test]
    fn saved_delay_uses_java_long_subtract_then_int_narrowing() {
        let source = tick(1, 0, i64::MIN, 0, 7);
        assert_eq!(SavedTick::from_scheduled(&source, i64::MAX).delay, 1);
        let saved = SavedTick {
            kind: 1,
            pos: source.pos,
            delay: 1,
            priority: TickPriority::Normal,
        };
        assert_eq!(saved.unpack(i64::MAX, 0).trigger_tick, i64::MIN);
        let json = serde_json::to_value(&saved).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"i":1,"x":0,"y":64,"z":0,"t":1,"p":0})
        );
    }
    #[test]
    fn unloaded_or_not_tickable_chunks_do_not_consume_scheduled_ticks() {
        let mut loaded = LoadedChunkTicks::new();
        assert!(!loaded.schedule(tick(1, -1, 0, 0, 0)));
        loaded.add_chunk((-1, 0), ChunkTickQueue::new());
        assert!(loaded.schedule(tick(1, -1, 0, 0, 0)));
        assert!(loaded.poll_due_in_chunk((-1, 0), 100, false).is_none());
        assert_eq!(loaded.chunk((-1, 0)).unwrap().count(), 1);
        let queued = loaded.remove_chunk((-1, 0)).unwrap();
        assert!(loaded.poll_due_in_chunk((-1, 0), 100, true).is_none());
        loaded.add_chunk((-1, 0), queued);
        assert!(loaded.poll_due_in_chunk((-1, 0), 100, true).is_some());
    }
    #[test]
    fn clock_freeze_packet_zero_sentinel_and_java_long_overflow() {
        let mut clock = TickClock::new(i64::MAX, 0, false);
        clock.advance();
        assert_eq!(clock.game_time, i64::MIN);
        assert_eq!(clock.day_time, 0);
        assert_eq!(
            i64::from_be_bytes(clock.time_update_payload()[8..].try_into().unwrap()),
            -1
        );
        clock.daylight_cycle = true;
        clock.advance();
        assert_eq!(clock.day_time, 1);
        clock.daylight_cycle = false;
        assert_eq!(
            i64::from_be_bytes(clock.time_update_payload()[8..].try_into().unwrap()),
            -1
        );
        assert_eq!(TICK_DURATION, Duration::from_secs(1) / TICKS_PER_SECOND);
    }
    #[test]
    fn due_container_heads_use_intra_order_not_a_global_time_sort() {
        let mut scheduler = LevelTickScheduler::new();
        scheduler.add_chunk((0, 0), ChunkTickQueue::new());
        scheduler.add_chunk((1, 0), ChunkTickQueue::new());
        scheduler.schedule(tick(1, 1, 0, 3, 1));
        scheduler.schedule(tick(2, 2, 1, -3, 2));
        scheduler.schedule(tick(3, 16, 0, 0, 3));
        let result = scheduler
            .drain_due(10, 100, |_| true)
            .into_iter()
            .map(|t| t.kind)
            .collect::<Vec<_>>();
        assert_eq!(result, [3, 1, 2]);
    }
    #[test]
    fn callbacks_reschedule_next_invocation_and_query_only_remaining_collected_ticks() {
        let mut scheduler = LevelTickScheduler::new();
        scheduler.add_chunk((0, 0), ChunkTickQueue::new());
        scheduler.schedule(tick(1, 1, 0, 0, 0));
        scheduler.schedule(tick(2, 2, 0, 0, 1));
        let mut observed = Vec::new();
        scheduler.tick(
            0,
            2,
            |_| true,
            |state, current| {
                assert!(!state.has_scheduled_tick(current.pos, &current.kind));
                assert!(!state.will_tick_this_tick(current.pos, &current.kind));
                if current.kind == 1 {
                    assert!(state.will_tick_this_tick(BlockPos::new(2, 64, 0), &2));
                    assert!(state.schedule(tick(1, 1, 0, 0, 100)));
                }
                observed.push(current.kind);
            },
        );
        assert_eq!(observed, [1, 2]);
        assert_eq!(scheduler.count(), 1);
        assert_eq!(scheduler.drain_due(0, 100, |_| true).len(), 1);
    }
    #[test]
    fn clear_during_callback_preserves_java_cached_query_quirk_until_cleanup() {
        let mut scheduler = LevelTickScheduler::new();
        scheduler.add_chunk((0, 0), ChunkTickQueue::new());
        scheduler.schedule(tick(1, 1, 0, -3, 0));
        scheduler.schedule(tick(2, 2, 0, 3, 1));
        let bounds = BlockBounds {
            min: BlockPos::new(0, 0, 0),
            max: BlockPos::new(15, 100, 15),
        };
        let mut ran = 0;
        scheduler.tick(
            0,
            100,
            |_| true,
            |state, _| {
                ran += 1;
                assert!(state.will_tick_this_tick(BlockPos::new(2, 64, 0), &2));
                state.clear_area(bounds);
                assert!(state.will_tick_this_tick(BlockPos::new(2, 64, 0), &2));
            },
        );
        assert_eq!(ran, 1);
        assert!(!scheduler.will_tick_this_tick(BlockPos::new(2, 64, 0), &2));
    }
    #[test]
    fn copied_ticks_keep_time_and_priority_and_shift_orders_beyond_source_max() {
        let mut scheduler = LevelTickScheduler::new();
        for x in 0..=1 {
            scheduler.add_chunk((x, 0), ChunkTickQueue::new());
        }
        scheduler.schedule(tick(1, 1, 100, -1, -5));
        scheduler.schedule(tick(2, 2, 100, -1, 20));
        assert_eq!(
            scheduler.copy_area(
                BlockBounds {
                    min: BlockPos::new(0, 0, 0),
                    max: BlockPos::new(15, 100, 15)
                },
                BlockPos::new(16, 0, 0)
            ),
            2
        );
        let copied = scheduler.chunk((1, 0)).unwrap().saved_ticks(100);
        assert_eq!(
            copied
                .iter()
                .map(|t| (t.pos.x, t.delay, t.priority.value()))
                .collect::<Vec<_>>(),
            [(17, 0, -1), (18, 0, -1)]
        );
        let ticks = scheduler.drain_due(100, 100, |_| true);
        assert_eq!(
            ticks.iter().map(|t| t.sub_tick_order).collect::<Vec<_>>(),
            [-5, 20, 21, 46]
        );
    }
    #[test]
    fn frozen_steps_keep_last_step_active_until_the_following_tick() {
        let mut manager = TickRateManager::new();
        manager.set_frozen(true);
        assert!(manager.runs_normally());
        manager.tick();
        assert!(!manager.runs_normally());
        manager.set_frozen_ticks_to_run(1);
        assert!(!manager.runs_normally());
        manager.tick();
        assert_eq!(manager.frozen_ticks_to_run(), 0);
        assert!(!manager.is_stepping_forward());
        assert!(manager.runs_normally());
        manager.tick();
        assert!(!manager.runs_normally());
        assert!(!manager.is_entity_frozen(true, 0));
        assert!(!manager.is_entity_frozen(false, 1));
        assert!(manager.is_entity_frozen(false, 0));
    }
    #[test]
    fn weather_clear_window_stops_flags_but_preserves_smooth_client_transition() {
        let mut weather = WeatherState {
            clear_weather_time: 1,
            thunder_time: 50,
            rain_time: 50,
            raining: true,
            thundering: true,
            old_thunder_level: 0.21,
            thunder_level: 0.21,
            old_rain_level: 0.21,
            rain_level: 0.21,
        };
        let events = weather.advance(true, true, |_, _| panic!("clear period consumes no random"));
        assert_eq!(
            (
                weather.clear_weather_time,
                weather.rain_time,
                weather.thunder_time
            ),
            (0, 0, 0)
        );
        assert!(!weather.raining && !weather.thundering);
        assert_eq!(
            events
                .iter()
                .map(|event| (event.audience, event.reason))
                .collect::<Vec<_>>(),
            [
                (WeatherAudience::Dimension, 7),
                (WeatherAudience::Dimension, 8),
                (WeatherAudience::AllPlayers, 2),
                (WeatherAudience::AllPlayers, 7),
                (WeatherAudience::AllPlayers, 8),
            ]
        );
        assert!(weather.rain_level > 0.0);
    }
    #[test]
    fn persisted_positions_must_match_owning_chunk_before_unpack() {
        let saved = vec![
            SavedTick {
                kind: 1,
                pos: BlockPos::new(-1, 64, -1),
                delay: 0,
                priority: TickPriority::Normal,
            },
            SavedTick {
                kind: 2,
                pos: BlockPos::new(0, 64, -1),
                delay: 0,
                priority: TickPriority::Normal,
            },
        ];
        let mut scheduler = LevelTickScheduler::new();
        scheduler.add_chunk(
            (-1, -1),
            ChunkTickQueue::from_saved_for_chunk((-1, -1), saved),
        );
        assert_eq!(scheduler.count(), 1);
        assert!(scheduler.drain_due(100, 100, |_| true).is_empty());
        scheduler.unpack_chunk((-1, -1), 100);
        assert_eq!(scheduler.drain_due(100, 100, |_| true)[0].kind, 1);
    }
}
