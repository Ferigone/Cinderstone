//! Primary loading tickets and the original player-ticket throttler phases.
//!
//! Loading levels schedule ChunkHolders; they do not confirm chunk visibility.
//! The host applies scheduling changes to ChunkLifecycle, attaches normal
//! entity-ticking-future completion callbacks, and advances the two executors.
use crate::entity_ticking_range::{packed, unpacked, Chunk, TicketKind, SOURCE_CHUNK};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// An intrusive ordered set. Removal and re-insertion move to the tail; repeated
/// insertion of an existing member preserves its place, as LongLinkedOpenHashSet.
#[derive(Clone, Debug, Default)]
struct OrderedSet {
    links: HashMap<i64, (Option<i64>, Option<i64>)>,
    first: Option<i64>,
    last: Option<i64>,
}
impl OrderedSet {
    fn insert(&mut self, key: i64) {
        if self.links.contains_key(&key) {
            return;
        }
        if let Some(last) = self.last {
            self.links.get_mut(&last).expect("queue tail exists").1 = Some(key);
        } else {
            self.first = Some(key);
        }
        self.links.insert(key, (self.last, None));
        self.last = Some(key);
    }
    fn remove(&mut self, key: i64) {
        let Some((before, after)) = self.links.remove(&key) else {
            return;
        };
        if let Some(before) = before {
            self.links.get_mut(&before).unwrap().1 = after;
        } else {
            self.first = after;
        }
        if let Some(after) = after {
            self.links.get_mut(&after).unwrap().0 = before;
        } else {
            self.last = before;
        }
    }
    fn pop(&mut self) -> Option<i64> {
        let key = self.first?;
        self.remove(key);
        Some(key)
    }
    #[cfg(test)]
    fn keys(&self) -> Vec<i64> {
        let mut result = Vec::with_capacity(self.links.len());
        let mut next = self.first;
        while let Some(key) = next {
            result.push(key);
            next = self.links[&key].1;
        }
        result
    }
}

/// The source-ticket map's default fastutil Long2ObjectOpenHashMap key layout.
/// replacePlayerTicketsLevel observes this iteration order before queuing edits.
#[derive(Clone, Debug)]
struct LongKeyOrder {
    keys: Vec<i64>,
    size: usize,
    zero: bool,
}
impl Default for LongKeyOrder {
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
impl LongKeyOrder {
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
    fn insert(&mut self, key: i64) {
        if self.find(key).is_some() {
            return;
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
    }
    fn remove(&mut self, key: i64) {
        let Some(mut hole) = self.find(key) else {
            return;
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
    fn keys(&self) -> Vec<i64> {
        self.zero
            .then_some(0)
            .into_iter()
            .chain(
                self.keys[..self.n()]
                    .iter()
                    .rev()
                    .copied()
                    .filter(|k| *k != 0),
            )
            .collect()
    }
}

impl LongKeyOrder {
    fn clear(&mut self) {
        self.keys.fill(0);
        self.size = 0;
        self.zero = false;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulingChange {
    pub chunk: Chunk,
    pub old_level: i32,
    pub level: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadingEvent {
    Enqueue { executor: &'static str, task: u64 },
    Run { executor: &'static str, task: u64 },
    Schedule(SchedulingChange),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TickingTransfer {
    AddPlayer {
        chunk: Chunk,
        player_id: i32,
    },
    RemovePlayer {
        chunk: Chunk,
        player_id: i32,
    },
    SimulationDistance(i32),
    AddTicket {
        kind: TicketKind,
        chunk: Chunk,
        level: i32,
    },
    RemoveTicket {
        kind: TicketKind,
        chunk: Chunk,
        level: i32,
    },
}
impl TickingTransfer {
    pub fn apply(&self, tracker: &mut crate::entity_ticking_range::EntityTickingRange) {
        match *self {
            Self::AddPlayer { chunk, player_id } => tracker.add_player(chunk, player_id),
            Self::RemovePlayer { chunk, player_id } => {
                tracker.remove_player(chunk, player_id);
            }
            Self::SimulationDistance(distance) => tracker.update_simulation_distance(distance),
            Self::AddTicket { kind, chunk, level } => tracker.add_ticket(kind, chunk, level),
            Self::RemoveTicket { kind, chunk, level } => tracker.remove_ticket(kind, chunk, level),
        }
    }
}

#[derive(Clone, Debug)]
struct DistanceGraph {
    max: i32,
    default: i32,
    store_through: i32,
    levels: BTreeMap<i64, u8>,
    level_order: LongKeyOrder,
    sources: BTreeMap<i64, i32>,
    computed: BTreeMap<i64, u8>,
    queues: Vec<OrderedSet>,
    changes: Vec<(i64, i32, i32)>,
}
impl DistanceGraph {
    fn new(max: i32, default: i32, store_through: i32) -> Self {
        Self {
            max,
            default,
            store_through,
            levels: BTreeMap::new(),
            level_order: LongKeyOrder::default(),
            sources: BTreeMap::new(),
            computed: BTreeMap::new(),
            queues: (0..=max).map(|_| OrderedSet::default()).collect(),
            changes: Vec::new(),
        }
    }
    fn level(&self, key: i64) -> i32 {
        self.levels.get(&key).map_or(self.default, |&v| v as i32)
    }
    fn set_level(&mut self, key: i64, level: i32) {
        let old = self.level(key);
        if level > self.store_through {
            if self.levels.remove(&key).is_some() {
                self.level_order.remove(key);
            }
        } else {
            if self.levels.insert(key, level as u8).is_none() {
                self.level_order.insert(key);
            }
        }
        self.changes.push((key, old, level));
    }
    fn source(&mut self, key: i64, level: i32, decreasing: bool) {
        if level == i32::MAX {
            self.sources.remove(&key);
        } else {
            self.sources.insert(key, level);
        }
        self.check_edge(SOURCE_CHUNK, key, level, decreasing);
    }
    fn run(&mut self, mut budget: i32) -> i32 {
        while budget > 0 {
            let Some(priority) = self.queues.iter().position(|q| q.first.is_some()) else {
                break;
            };
            budget -= 1;
            let key = self.queues[priority].pop().unwrap();
            let old = self.level(key).clamp(0, self.max);
            let next = self.computed.remove(&key).unwrap() as i32;
            if next < old {
                self.set_level(key, next);
                self.check_neighbors(key, next, true);
            } else if next > old {
                self.set_level(key, self.max);
                if next != self.max {
                    self.queues[next.min(self.max) as usize].insert(key);
                    self.computed.insert(key, next as u8);
                }
                self.check_neighbors(key, old, false);
            }
        }
        budget
    }
    fn neighbor_level(&self, from: i64, to: i64, level: i32) -> i32 {
        if from == SOURCE_CHUNK {
            self.sources.get(&to).copied().unwrap_or(i32::MAX)
        } else {
            level.wrapping_add(1)
        }
    }
    fn calculated(&self, key: i64, excluded: i64, mut level: i32) -> i32 {
        let (x, z) = unpacked(key);
        for dx in -1i32..=1 {
            for dz in -1i32..=1 {
                let mut neighbor = packed((x.wrapping_add(dx), z.wrapping_add(dz)));
                if neighbor == key {
                    neighbor = SOURCE_CHUNK;
                }
                if neighbor == excluded {
                    continue;
                }
                level = level.min(self.neighbor_level(neighbor, key, self.level(neighbor)));
                if level == 0 {
                    return level;
                }
            }
        }
        level
    }
    fn check_edge(&mut self, from: i64, to: i64, level: i32, decreasing: bool) {
        let old = self.level(to);
        let computed = self.computed.get(&to).map_or(255, |&v| v as i32);
        self.check_edge_values(from, to, level, old, computed, decreasing);
    }
    fn check_edge_values(
        &mut self,
        from: i64,
        to: i64,
        level: i32,
        old: i32,
        computed: i32,
        decreasing: bool,
    ) {
        if to == SOURCE_CHUNK {
            return;
        }
        let level = level.clamp(0, self.max);
        let old = old.clamp(0, self.max);
        let absent = computed == 255;
        let previous = if absent { old } else { computed };
        let next = if decreasing {
            previous.min(level)
        } else {
            self.calculated(to, from, level).clamp(0, self.max)
        };
        let old_priority = old.min(previous).min(self.max);
        if old != next {
            let priority = old.min(next).min(self.max);
            if old_priority != priority && !absent {
                self.queues[old_priority as usize].remove(to);
            }
            self.queues[priority as usize].insert(to);
            self.computed.insert(to, next as u8);
        } else if !absent {
            self.queues[old_priority as usize].remove(to);
            self.computed.remove(&to);
        }
    }
    fn check_neighbor(&mut self, from: i64, to: i64, level: i32, decreasing: bool) {
        let computed = self.computed.get(&to).map_or(255, |&v| v as i32);
        let level = self.neighbor_level(from, to, level).clamp(0, self.max);
        if decreasing {
            self.check_edge_values(from, to, level, self.level(to), computed, true);
        } else {
            let absent = computed == 255;
            let previous = if absent {
                self.level(to).clamp(0, self.max)
            } else {
                computed
            };
            if level == previous {
                self.check_edge_values(
                    from,
                    to,
                    self.max,
                    if absent { previous } else { self.level(to) },
                    computed,
                    false,
                );
            }
        }
    }
    fn check_neighbors(&mut self, key: i64, level: i32, decreasing: bool) {
        if decreasing && level >= self.max - 1 {
            return;
        }
        let (x, z) = unpacked(key);
        for dx in -1i32..=1 {
            for dz in -1i32..=1 {
                let neighbor = packed((x.wrapping_add(dx), z.wrapping_add(dz)));
                if neighbor != key {
                    self.check_neighbor(key, neighbor, level, decreasing);
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Ticket {
    level: i32,
    kind: TicketKind,
}
#[derive(Clone, Debug)]
enum SorterMessage {
    Level {
        chunk: i64,
        level: i32,
    },
    Release {
        chunk: i64,
        clear: bool,
        remove_ticket: bool,
    },
    Submit {
        chunk: i64,
        level: i32,
    },
    Poll,
}
#[derive(Clone, Debug)]
enum MainTask {
    ProcessorAdmission { chunk: i64, batch: u64 },
    AdmissionCommit { chunk: i64 },
    RemovalCommit { chunk: i64 },
    FutureRelease { chunk: i64 },
}
#[derive(Clone, Debug, Default)]
struct PendingLevel {
    order: OrderedSet,
    entries: BTreeMap<i64, Vec<bool>>,
}
impl PendingLevel {
    fn append(&mut self, chunk: i64, messages: impl IntoIterator<Item = bool>) {
        if !self.entries.contains_key(&chunk) {
            self.order.insert(chunk);
        }
        self.entries.entry(chunk).or_default().extend(messages);
    }
    fn remove(&mut self, chunk: i64) -> Option<Vec<bool>> {
        let result = self.entries.remove(&chunk);
        if result.is_some() {
            self.order.remove(chunk);
        }
        result
    }
}

#[derive(Clone, Debug)]
pub struct ChunkLoadingTickets {
    view_distance: i32,
    simulation_distance: i32,
    players: BTreeMap<i64, BTreeSet<i32>>,
    tickets: BTreeMap<i64, BTreeMap<Ticket, i64>>,
    ticket_tick: i64,
    player: DistanceGraph,
    loading: DistanceGraph,
    queue_levels: BTreeMap<i64, i32>,
    to_update: LongKeyOrder,
    pending: Vec<PendingLevel>,
    acquired: LongKeyOrder,
    sleeping: bool,
    mailbox: [VecDeque<SorterMessage>; 4],
    sorter_scheduled: bool,
    sorter_tasks: VecDeque<u64>,
    main_tasks: VecDeque<(u64, MainTask)>,
    next_task: u64,
    next_batch: u64,
    outstanding_batches: BTreeMap<u64, usize>,
    tickets_to_release: LongKeyOrder,
    ticking_transfers: Vec<TickingTransfer>,
    events: Vec<LoadingEvent>,
    scheduling_changes: Vec<SchedulingChange>,
}
impl ChunkLoadingTickets {
    pub fn new(simulation_distance: i32) -> Self {
        Self {
            view_distance: 0,
            simulation_distance,
            players: BTreeMap::new(),
            tickets: BTreeMap::new(),
            ticket_tick: 0,
            player: DistanceGraph::new(33, 34, 32),
            loading: DistanceGraph::new(45, 45, 44),
            queue_levels: BTreeMap::new(),
            to_update: LongKeyOrder::default(),
            pending: (0..46).map(|_| PendingLevel::default()).collect(),
            acquired: LongKeyOrder::default(),
            sleeping: true,
            mailbox: std::array::from_fn(|_| VecDeque::new()),
            sorter_scheduled: false,
            sorter_tasks: VecDeque::new(),
            main_tasks: VecDeque::new(),
            next_task: 0,
            next_batch: 0,
            outstanding_batches: BTreeMap::new(),
            tickets_to_release: LongKeyOrder::default(),
            ticking_transfers: Vec::new(),
            events: Vec::new(),
            scheduling_changes: Vec::new(),
        }
    }
    pub fn view_distance(&self) -> i32 {
        self.view_distance
    }
    /// Caller supplies DistanceManager's threshold; ChunkMap normally clamps
    /// the configured server view distance to 2..32 before invoking this.
    pub fn set_view_distance(&mut self, distance: i32) {
        for chunk in self.player.level_order.keys() {
            let level = self.player.level(chunk);
            self.ticket_transition(chunk, level, level <= self.view_distance, level <= distance);
        }
        self.view_distance = distance;
    }
    pub fn add_player(&mut self, chunk: Chunk, player_id: i32) {
        let key = packed(chunk);
        self.players.entry(key).or_default().insert(player_id);
        self.player.source(key, 0, true);
        self.ticking_transfers
            .push(TickingTransfer::AddPlayer { chunk, player_id });
    }
    pub fn remove_player(&mut self, chunk: Chunk, player_id: i32) -> bool {
        let key = packed(chunk);
        let Some(players) = self.players.get_mut(&key) else {
            return false;
        };
        let removed = players.remove(&player_id);
        if players.is_empty() {
            self.players.remove(&key);
            self.player.source(key, i32::MAX, false);
        }
        self.ticking_transfers
            .push(TickingTransfer::RemovePlayer { chunk, player_id });
        removed
    }
    pub fn update_simulation_distance(&mut self, distance: i32) {
        if distance != self.simulation_distance {
            self.simulation_distance = distance;
            self.ticking_transfers
                .push(TickingTransfer::SimulationDistance(distance));
        }
    }
    pub fn take_ticking_transfers(&mut self) -> Vec<TickingTransfer> {
        std::mem::take(&mut self.ticking_transfers)
    }
    pub fn primary_level(&self, chunk: Chunk) -> i32 {
        self.source_level(packed(chunk))
    }
    pub fn loading_level(&self, chunk: Chunk) -> i32 {
        self.loading.level(packed(chunk))
    }
    pub fn player_distance(&self, chunk: Chunk) -> i32 {
        self.player.level(packed(chunk))
    }
    pub fn acquired_chunks(&self) -> Vec<Chunk> {
        self.acquired.keys().into_iter().map(unpacked).collect()
    }
    pub fn pending_sorter_tasks(&self) -> usize {
        self.sorter_tasks.len()
    }
    pub fn pending_main_tasks(&self) -> usize {
        self.main_tasks.len()
    }
    pub fn has_distance_work(&self) -> bool {
        !self.player.computed.is_empty() || !self.loading.computed.is_empty()
    }
    pub fn take_events(&mut self) -> Vec<LoadingEvent> {
        std::mem::take(&mut self.events)
    }
    pub fn take_scheduling_changes(&mut self) -> Vec<SchedulingChange> {
        std::mem::take(&mut self.scheduling_changes)
    }
    fn source_level(&self, key: i64) -> i32 {
        self.tickets
            .get(&key)
            .and_then(|s| s.first_key_value())
            .map_or(45, |(t, _)| t.level)
    }
    pub fn add_primary_ticket(&mut self, kind: TicketKind, chunk: Chunk, level: i32) {
        let key = packed(chunk);
        let old = self.source_level(key);
        self.tickets
            .entry(key)
            .or_default()
            .insert(Ticket { level, kind }, self.ticket_tick);
        // Source map can contain a ticket >=45 without queuing a decrease.
        self.loading.sources.insert(key, self.source_level(key));
        if level < old {
            self.loading.check_edge(SOURCE_CHUNK, key, level, true);
        }
    }
    pub fn remove_primary_ticket(&mut self, kind: TicketKind, chunk: Chunk, level: i32) {
        let key = packed(chunk);
        let tickets = self.tickets.entry(key).or_default();
        tickets.remove(&Ticket { level, kind });
        if tickets.is_empty() {
            self.tickets.remove(&key);
        }
        let next = self.source_level(key);
        self.loading.sources.insert(key, next);
        self.loading.check_edge(SOURCE_CHUNK, key, next, false);
    }
    pub fn add_region_ticket(&mut self, kind: TicketKind, chunk: Chunk, distance: i32) {
        let level = 33i32.wrapping_sub(distance);
        self.add_primary_ticket(kind, chunk, level);
        self.ticking_transfers
            .push(TickingTransfer::AddTicket { kind, chunk, level });
    }
    pub fn remove_region_ticket(&mut self, kind: TicketKind, chunk: Chunk, distance: i32) {
        let level = 33i32.wrapping_sub(distance);
        self.remove_primary_ticket(kind, chunk, level);
        self.ticking_transfers
            .push(TickingTransfer::RemoveTicket { kind, chunk, level });
    }
    pub fn update_chunk_forced(&mut self, chunk: Chunk, forced: bool) {
        if forced {
            self.add_region_ticket(TicketKind::Forced, chunk, 2);
        } else {
            self.remove_region_ticket(TicketKind::Forced, chunk, 2);
        }
    }
    /// All represented TicketKind variants have timeout zero. Other timeout
    /// kinds require a further port of DistanceManager's purge semantics.
    pub fn purge_stale_tickets(&mut self) {
        self.ticket_tick = self.ticket_tick.wrapping_add(1);
    }
    pub fn run_player_distance_updates(&mut self) {
        self.player.run(i32::MAX);
        for (key, _, _) in std::mem::take(&mut self.player.changes) {
            self.to_update.insert(key);
        }
        for key in self.to_update.keys() {
            let old = self.queue_levels.get(&key).copied().unwrap_or(34);
            let next = self.player.level(key);
            if old != next {
                self.tell_sorter(
                    0,
                    SorterMessage::Level {
                        chunk: key,
                        level: next,
                    },
                );
                self.ticket_transition(
                    key,
                    next,
                    old <= self.view_distance,
                    next <= self.view_distance,
                );
            }
        }
        self.to_update.clear();
    }
    pub fn run_loading_updates(&mut self, budget: i32) -> i32 {
        let remaining = self.loading.run(budget);
        for (key, old_level, level) in std::mem::take(&mut self.loading.changes) {
            let change = SchedulingChange {
                chunk: unpacked(key),
                old_level,
                level,
            };
            self.events.push(LoadingEvent::Schedule(change));
            self.scheduling_changes.push(change);
        }
        remaining
    }
    fn ticket_transition(&mut self, chunk: i64, level: i32, old: bool, new: bool) {
        if old == new {
            return;
        }
        if new {
            self.tell_sorter(2, SorterMessage::Submit { chunk, level });
        } else {
            self.tell_sorter(
                1,
                SorterMessage::Release {
                    chunk,
                    clear: true,
                    remove_ticket: true,
                },
            );
        }
    }
    fn next_task_id(&mut self) -> u64 {
        self.next_task = self.next_task.wrapping_add(1);
        self.next_task
    }
    fn enqueue_main(&mut self, task: MainTask) {
        let id = self.next_task_id();
        self.main_tasks.push_back((id, task));
        self.events.push(LoadingEvent::Enqueue {
            executor: "main",
            task: id,
        });
    }
    fn schedule_sorter(&mut self) {
        if !self.sorter_scheduled && self.mailbox.iter().any(|q| !q.is_empty()) {
            self.sorter_scheduled = true;
            let id = self.next_task_id();
            self.sorter_tasks.push_back(id);
            self.events.push(LoadingEvent::Enqueue {
                executor: "sorter",
                task: id,
            });
        }
    }
    fn tell_sorter(&mut self, priority: usize, message: SorterMessage) {
        self.mailbox[priority].push_back(message);
        self.schedule_sorter();
    }
    pub fn run_sorter_tasks(&mut self, mut budget: i32) -> usize {
        let mut used = 0;
        while budget > 0 {
            let Some(id) = self.sorter_tasks.pop_front() else {
                break;
            };
            budget -= 1;
            used += 1;
            self.events.push(LoadingEvent::Run {
                executor: "sorter",
                task: id,
            });
            let priority = self
                .mailbox
                .iter()
                .position(|q| !q.is_empty())
                .expect("scheduled mailbox has work");
            let message = self.mailbox[priority].pop_front().unwrap();
            self.run_sorter_message(message);
            self.sorter_scheduled = false;
            self.schedule_sorter();
        }
        used
    }
    fn run_sorter_message(&mut self, message: SorterMessage) {
        match message {
            SorterMessage::Level { chunk, level } => {
                let old = self.queue_levels.get(&chunk).copied().unwrap_or(34);
                if (0..46).contains(&old) {
                    if let Some(tasks) = self.pending[old as usize].remove(chunk) {
                        if !tasks.is_empty() {
                            self.pending[level as usize].append(chunk, tasks);
                        }
                    }
                }
                if level >= 34 {
                    self.queue_levels.remove(&chunk);
                } else {
                    self.queue_levels.insert(chunk, level);
                }
            }
            SorterMessage::Submit { chunk, level } => {
                self.pending[level as usize].append(chunk, [true, false]);
                if self.sleeping {
                    self.sleeping = false;
                    self.tell_sorter(3, SorterMessage::Poll);
                }
            }
            SorterMessage::Release {
                chunk,
                clear,
                remove_ticket,
            } => {
                for pending in &mut self.pending {
                    if clear {
                        pending.remove(chunk);
                    } else if let Some(messages) = pending.entries.get_mut(&chunk) {
                        messages.retain(|&task| task);
                        if messages.is_empty() {
                            pending.remove(chunk);
                        }
                    }
                }
                self.acquired.remove(chunk);
                if self.sleeping {
                    self.sleeping = false;
                    self.tell_sorter(3, SorterMessage::Poll);
                }
                if remove_ticket {
                    self.enqueue_main(MainTask::RemovalCommit { chunk });
                }
            }
            SorterMessage::Poll => {
                if self.acquired.size >= 4 {
                    self.sleeping = true;
                    return;
                }
                let Some(level) = self.pending.iter().position(|p| p.order.first.is_some()) else {
                    self.sleeping = true;
                    return;
                };
                let chunk = self.pending[level].order.first.unwrap();
                let messages = self.pending[level].remove(chunk).unwrap();
                let count = messages.iter().filter(|&&task| task).count();
                self.next_batch = self.next_batch.wrapping_add(1);
                let batch = self.next_batch;
                if count > 0 {
                    self.outstanding_batches.insert(batch, count);
                }
                for task in messages {
                    if task {
                        self.enqueue_main(MainTask::ProcessorAdmission { chunk, batch });
                    } else {
                        self.acquired.insert(chunk);
                    }
                }
                if count == 0 {
                    self.tell_sorter(3, SorterMessage::Poll);
                }
            }
        }
    }
    pub fn run_main_tasks(&mut self, mut budget: i32) -> usize {
        let mut used = 0;
        while budget > 0 {
            let Some((id, task)) = self.main_tasks.pop_front() else {
                break;
            };
            budget -= 1;
            used += 1;
            self.events.push(LoadingEvent::Run {
                executor: "main",
                task: id,
            });
            match task {
                MainTask::ProcessorAdmission { chunk, batch } => {
                    self.enqueue_main(MainTask::AdmissionCommit { chunk });
                    let left = self
                        .outstanding_batches
                        .get_mut(&batch)
                        .expect("processor batch exists");
                    *left -= 1;
                    if *left == 0 {
                        self.outstanding_batches.remove(&batch);
                        self.tell_sorter(3, SorterMessage::Poll);
                    }
                }
                MainTask::AdmissionCommit { chunk } => {
                    if self.player.level(chunk) <= self.view_distance {
                        self.add_primary_ticket(TicketKind::Player, unpacked(chunk), 31);
                        self.tickets_to_release.insert(chunk);
                    } else {
                        self.tell_sorter(
                            1,
                            SorterMessage::Release {
                                chunk,
                                clear: false,
                                remove_ticket: false,
                            },
                        );
                    }
                }
                MainTask::RemovalCommit { chunk } => {
                    self.remove_primary_ticket(TicketKind::Player, unpacked(chunk), 31)
                }
                MainTask::FutureRelease { chunk } => self.tell_sorter(
                    1,
                    SorterMessage::Release {
                        chunk,
                        clear: false,
                        remove_ticket: false,
                    },
                ),
            }
        }
        used
    }
    /// Host calls only when the registered entity-ticking future completes
    /// normally. Both successful and unloaded/failure ChunkResults trigger it;
    /// exceptional or cancelled CompletableFutures do not run thenAccept.
    pub fn normal_entity_future_completion(&mut self, chunk: Chunk) {
        self.enqueue_main(MainTask::FutureRelease {
            chunk: packed(chunk),
        });
    }
    /// Called after the host applied changed-holder futures, on vanilla's next
    /// DistanceManager pass. The host attaches one callback to each returned
    /// holder's current entity-ticking future; no completion is invented here.
    pub fn take_release_registrations(&mut self) -> Vec<Chunk> {
        let result = self
            .tickets_to_release
            .keys()
            .into_iter()
            .filter(|key| {
                self.tickets
                    .get(key)
                    .is_some_and(|ts| ts.keys().any(|t| t.kind == TicketKind::Player))
            })
            .map(unpacked)
            .collect();
        self.tickets_to_release.clear();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity_ticking_range::EntityTickingRange;
    use serde_json::{json, Value};
    fn chunk(v: &Value) -> Chunk {
        (v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32)
    }
    fn kind(v: &Value) -> TicketKind {
        match v.as_str().unwrap() {
            "player" => TicketKind::Player,
            "start" => TicketKind::Start,
            "forced" => TicketKind::Forced,
            _ => panic!("kind"),
        }
    }
    fn kind_name(v: TicketKind) -> &'static str {
        match v {
            TicketKind::Player => "player",
            TicketKind::Start => "start",
            TicketKind::Forced => "forced",
        }
    }
    fn points(keys: Vec<i64>) -> Value {
        Value::Array(
            keys.into_iter()
                .map(|k| {
                    let (x, z) = unpacked(k);
                    json!([x, z])
                })
                .collect(),
        )
    }
    fn rows(map: &BTreeMap<i64, u8>) -> Value {
        Value::Array(
            map.iter()
                .map(|(&k, &level)| {
                    let (x, z) = unpacked(k);
                    json!([x, z, level])
                })
                .collect(),
        )
    }
    fn graph(g: &DistanceGraph) -> Value {
        json!({"levels":rows(&g.levels),"computed":rows(&g.computed),"queues":g.queues.iter().map(|q|points(q.keys())).collect::<Vec<_>>(),"queue_size":g.computed.len(),"has_work":!g.computed.is_empty()})
    }
    fn ticking_graph(g: &EntityTickingRange) -> Value {
        let r = |values: Vec<(Chunk, i32)>| {
            values
                .into_iter()
                .map(|((x, z), level)| json!([x, z, level]))
                .collect::<Vec<_>>()
        };
        json!({"levels":r(g.stored_levels()),"computed":r(g.computed_levels()),"queues":g.queued_chunks().iter().map(|q|q.iter().map(|&(x,z)|json!([x,z])).collect::<Vec<_>>()).collect::<Vec<_>>(),"queue_size":g.queue_size(),"has_work":g.has_work()})
    }
    fn snapshot(s: &ChunkLoadingTickets, g: &EntityTickingRange, queries: &Value) -> Value {
        let mut primary = Vec::new();
        for (&key, values) in &s.tickets {
            let (x, z) = unpacked(key);
            for (ticket, tick) in values {
                primary.push(json!([x, z, kind_name(ticket.kind), ticket.level, tick]));
            }
        }
        primary.sort_by_key(Value::to_string);
        let pending=s.pending.iter().map(|level|level.order.keys().into_iter().map(|key|json!({"chunk":points(vec![key])[0],"messages":level.entries[&key].iter().map(|&task|if task{"task"}else{"acquire"}).collect::<Vec<_>>()})).collect::<Vec<_>>()).collect::<Vec<_>>();
        json!({"view_distance":s.view_distance,"ticket_tick":s.ticket_tick,"primary_tickets":primary,
            "player":graph(&s.player),"loading":graph(&s.loading),"ticking":ticking_graph(g),
            "queue_levels":s.queue_levels.iter().map(|(&key,&level)|{let(x,z)=unpacked(key);json!([x,z,level])}).collect::<Vec<_>>(),
            "to_update":points(s.to_update.keys()),"tickets_to_release":points(s.tickets_to_release.keys()),
            "sorter_tasks":s.sorter_tasks,"main_tasks":s.main_tasks.iter().map(|&(id,_)|id).collect::<Vec<_>>(),
            "acquired":points(s.acquired.keys()),"sleeping":s.sleeping,"mailbox_size":s.mailbox.iter().map(VecDeque::len).sum::<usize>(),"pending":pending,
            "query":queries.as_array().unwrap().iter().map(|p|json!({"player":s.player_distance(chunk(p)),"loading":s.loading_level(chunk(p)),"ticking":g.level(chunk(p))})).collect::<Vec<_>>()})
    }
    fn events(events: Vec<LoadingEvent>) -> Value {
        Value::Array(events.into_iter().map(|event|match event{
        LoadingEvent::Enqueue{executor,task}=>json!({"event":"enqueue","executor":executor,"task":task}),
        LoadingEvent::Run{executor,task}=>json!({"event":"run","executor":executor,"task":task}),
        LoadingEvent::Schedule(change)=>json!({"event":"schedule","chunk":[change.chunk.0,change.chunk.1],"old":change.old_level,"level":change.level}),
    }).collect())
    }
    #[test]
    fn original_distance_manager_sources_graphs_and_throttler() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../fixtures/chunk_loading_tickets_vectors.json"
        ))
        .unwrap();
        let mut states = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let q = &case["query"];
            let sim = q["simulation_distance"].as_i64().unwrap() as i32;
            let mut s = ChunkLoadingTickets::new(sim);
            let mut ticking = EntityTickingRange::new(sim);
            let mut future_callbacks: BTreeMap<Chunk, usize> = BTreeMap::new();
            let mut completed_futures: BTreeMap<Chunk, String> = BTreeMap::new();
            for (index, (op, expected)) in q["ops"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["expected"].as_array().unwrap())
                .enumerate()
            {
                let mut result = Value::Null;
                match op["op"].as_str().unwrap() {
                    "view" => s.set_view_distance(op["distance"].as_i64().unwrap() as i32),
                    "simulation" => {
                        s.update_simulation_distance(op["distance"].as_i64().unwrap() as i32)
                    }
                    "player_add" => {
                        s.add_player(chunk(&op["chunk"]), op["player"].as_i64().unwrap() as i32)
                    }
                    "player_remove" => {
                        s.remove_player(chunk(&op["chunk"]), op["player"].as_i64().unwrap() as i32);
                    }
                    "raw_add" => s.add_primary_ticket(
                        kind(&op["kind"]),
                        chunk(&op["chunk"]),
                        op["level"].as_i64().unwrap() as i32,
                    ),
                    "raw_remove" => s.remove_primary_ticket(
                        kind(&op["kind"]),
                        chunk(&op["chunk"]),
                        op["level"].as_i64().unwrap() as i32,
                    ),
                    "region_add" => s.add_region_ticket(
                        kind(&op["kind"]),
                        chunk(&op["chunk"]),
                        op["distance"].as_i64().unwrap() as i32,
                    ),
                    "region_remove" => s.remove_region_ticket(
                        kind(&op["kind"]),
                        chunk(&op["chunk"]),
                        op["distance"].as_i64().unwrap() as i32,
                    ),
                    "forced" => {
                        s.update_chunk_forced(chunk(&op["chunk"]), op["enabled"].as_bool().unwrap())
                    }
                    "purge" => s.purge_stale_tickets(),
                    "run_player" => s.run_player_distance_updates(),
                    "run_ticking" => ticking.run_all_updates(),
                    "run_loading" => {
                        result = json!(s.run_loading_updates(
                            op["budget"].as_i64().unwrap_or(i32::MAX as i64) as i32
                        ))
                    }
                    "run_sorter" => {
                        result = json!(s.run_sorter_tasks(
                            op["budget"].as_i64().unwrap_or(i32::MAX as i64) as i32
                        ))
                    }
                    "run_main" => {
                        result =
                            json!(s.run_main_tasks(
                                op["budget"].as_i64().unwrap_or(i32::MAX as i64) as i32
                            ))
                    }
                    "register_releases" => {
                        for pos in s.take_release_registrations() {
                            if completed_futures.get(&pos).is_some_and(|state| {
                                matches!(state.as_str(), "success" | "failure" | "unloaded")
                            }) {
                                s.normal_entity_future_completion(pos);
                            } else {
                                *future_callbacks.entry(pos).or_default() += 1;
                            }
                        }
                        result = json!(false);
                    }
                    "future_complete" => {
                        let pos = chunk(&op["chunk"]);
                        let accepted = !completed_futures.contains_key(&pos);
                        if accepted {
                            completed_futures.insert(pos, op["state"].as_str().unwrap().to_owned());
                        }
                        result = json!(accepted);
                        if accepted
                            && matches!(
                                op["state"].as_str().unwrap(),
                                "success" | "failure" | "unloaded"
                            )
                        {
                            for _ in 0..future_callbacks.remove(&pos).unwrap_or(0) {
                                s.normal_entity_future_completion(pos);
                            }
                        }
                    }
                    "release" => s.normal_entity_future_completion(chunk(&op["chunk"])),
                    _ => panic!("unknown op {op}"),
                }
                for transfer in s.take_ticking_transfers() {
                    transfer.apply(&mut ticking);
                }
                assert_eq!(
                    result, expected["result"],
                    "{} step{index} result",
                    case["name"]
                );
                assert_eq!(
                    events(s.take_events()),
                    expected["events"],
                    "{} step{index} events op{op}",
                    case["name"]
                );
                let actual = snapshot(&s, &ticking, &q["points"]);
                for (key, value) in expected["state"].as_object().unwrap() {
                    if key == "ticking_tickets" {
                        continue;
                    } // ara ticket clocks have no expiry or propagation effect.
                    let value = if let Some(id) = value.get("pool") {
                        &fixture["pool"][id.as_u64().unwrap() as usize]
                    } else {
                        value
                    };
                    assert_eq!(
                        &actual[key], value,
                        "{} step{index} field{key} op{op}",
                        case["name"]
                    );
                }
                states += 1;
            }
        }
        println!("original loading/throttler states compared: {states}");
        assert!(states >= 300);
    }
}
