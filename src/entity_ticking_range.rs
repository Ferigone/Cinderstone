//! Java 1.21 TickingTracker / ChunkTracker / DynamicGraphMinFixedPoint.
//!
//! Source changes enqueue graph work. Published levels change only when updates
//! run. These levels are distance-manager predicates, not confirmed ChunkHolder
//! visibility: chunk futures and lifecycle callbacks remain host responsibilities.
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub type Chunk = (i32, i32);
pub const LEVEL_COUNT: i32 = 34;
pub const DEFAULT_LEVEL: i32 = 33;
/// Original ChunkPos.INVALID_CHUNK_POS = pack(1_875_066, 1_875_066).
pub const SOURCE_CHUNK: i64 = 8_053_347_149_716_602;

pub fn packed(chunk: Chunk) -> i64 {
    ((chunk.0 as u32 as u64) | ((chunk.1 as u32 as u64) << 32)) as i64
}
pub fn unpacked(key: i64) -> Chunk {
    (key as i32, ((key as u64) >> 32) as i32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TicketKind {
    Player,
    Start,
    Forced,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Ticket {
    level: i32,
    kind: TicketKind,
}

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

#[derive(Clone, Debug)]
pub struct EntityTickingRange {
    simulation_distance: i32,
    players: BTreeMap<i64, BTreeSet<i32>>,
    tickets: BTreeMap<i64, BTreeSet<Ticket>>,
    ticket_order: LongKeyOrder,
    levels: BTreeMap<i64, u8>,
    computed: BTreeMap<i64, u8>,
    queues: [OrderedSet; 34],
    region_tickets: BTreeMap<(i64, Ticket), i64>,
    ticket_tick: i64,
}
impl EntityTickingRange {
    pub fn new(simulation_distance: i32) -> Self {
        Self {
            simulation_distance,
            players: BTreeMap::new(),
            tickets: BTreeMap::new(),
            ticket_order: LongKeyOrder::default(),
            levels: BTreeMap::new(),
            computed: BTreeMap::new(),
            queues: std::array::from_fn(|_| OrderedSet::default()),
            region_tickets: BTreeMap::new(),
            ticket_tick: 0,
        }
    }
    pub fn simulation_distance(&self) -> i32 {
        self.simulation_distance
    }
    pub fn player_ticket_level(&self) -> i32 {
        31i32.wrapping_sub(self.simulation_distance).max(0)
    }
    pub fn ticket_tick(&self) -> i64 {
        self.ticket_tick
    }
    pub fn level(&self, chunk: Chunk) -> i32 {
        self.level_key(packed(chunk))
    }
    pub fn in_entity_ticking_range(&self, chunk: Chunk) -> bool {
        self.level(chunk) <= 31
    }
    pub fn in_block_ticking_range(&self, chunk: Chunk) -> bool {
        self.level(chunk) <= 32
    }
    pub fn has_work(&self) -> bool {
        !self.computed.is_empty()
    }
    pub fn queue_size(&self) -> usize {
        self.computed.len()
    }
    pub fn stored_levels(&self) -> Vec<(Chunk, i32)> {
        self.levels
            .iter()
            .map(|(&k, &v)| (unpacked(k), v as i32))
            .collect()
    }
    pub fn computed_levels(&self) -> Vec<(Chunk, i32)> {
        self.computed
            .iter()
            .map(|(&k, &v)| (unpacked(k), v as i32))
            .collect()
    }
    pub fn queued_chunks(&self) -> [Vec<Chunk>; 34] {
        std::array::from_fn(|i| self.queues[i].keys().into_iter().map(unpacked).collect())
    }

    /// Membership is per server-player ID; the actual PLAYER ticket key is the
    /// ChunkPos, shared until the final non-ignored player leaves that chunk.
    pub fn add_player(&mut self, chunk: Chunk, player_id: i32) {
        let key = packed(chunk);
        self.players.entry(key).or_default().insert(player_id);
        self.add_ticket(TicketKind::Player, chunk, self.player_ticket_level());
    }
    pub fn remove_player(&mut self, chunk: Chunk, player_id: i32) -> bool {
        let key = packed(chunk);
        let Some(players) = self.players.get_mut(&key) else {
            return false;
        };
        let removed = players.remove(&player_id);
        if players.is_empty() {
            self.players.remove(&key);
            self.remove_ticket(TicketKind::Player, chunk, self.player_ticket_level());
        }
        removed
    }
    pub fn update_simulation_distance(&mut self, distance: i32) {
        if distance == self.simulation_distance {
            return;
        }
        self.simulation_distance = distance;
        self.replace_player_tickets_level(self.player_ticket_level());
    }
    pub fn replace_player_tickets_level(&mut self, level: i32) {
        let mut replace = Vec::new();
        for key in self.ticket_order.keys() {
            for ticket in &self.tickets[&key] {
                if ticket.kind == TicketKind::Player {
                    replace.push((key, ticket.level));
                }
            }
        }
        for (key, old) in replace {
            self.remove_ticket(TicketKind::Player, unpacked(key), old);
            self.add_ticket(TicketKind::Player, unpacked(key), level);
        }
    }
    /// Raw ara ticket API. START uses Unit; PLAYER/FORCED use this ChunkPos as
    /// their key. Arbitrary extra keys and timeout-bearing ticket kinds are not
    /// represented by this three-kind core.
    pub fn add_ticket(&mut self, kind: TicketKind, chunk: Chunk, level: i32) {
        let key = packed(chunk);
        if !self.tickets.contains_key(&key) {
            self.ticket_order.insert(key);
        }
        let tickets = self.tickets.entry(key).or_default();
        let previous = tickets.first().map_or(i32::MAX, |t| t.level);
        tickets.insert(Ticket { level, kind });
        if level < previous {
            self.check_edge(SOURCE_CHUNK, key, level, true);
        }
    }
    pub fn remove_ticket(&mut self, kind: TicketKind, chunk: Chunk, level: i32) {
        let key = packed(chunk);
        // Original getTickets is computeIfAbsent even for a missing removal.
        if !self.tickets.contains_key(&key) {
            self.ticket_order.insert(key);
        }
        let tickets = self.tickets.entry(key).or_default();
        tickets.remove(&Ticket { level, kind });
        let next = tickets.first().map_or(i32::MAX, |t| t.level);
        if tickets.is_empty() {
            self.tickets.remove(&key);
            self.ticket_order.remove(key);
        }
        self.check_edge(SOURCE_CHUNK, key, next, false);
    }
    /// DistanceManager region wrapper: FULL(33) minus region distance. Duplicate
    /// tickets refresh the host clock; the three represented kinds never expire.
    pub fn add_region_ticket(&mut self, kind: TicketKind, chunk: Chunk, distance: i32) {
        let level = 33i32.wrapping_sub(distance);
        self.region_tickets
            .insert((packed(chunk), Ticket { level, kind }), self.ticket_tick);
        self.add_ticket(kind, chunk, level);
    }
    pub fn remove_region_ticket(&mut self, kind: TicketKind, chunk: Chunk, distance: i32) {
        let level = 33i32.wrapping_sub(distance);
        self.region_tickets
            .remove(&(packed(chunk), Ticket { level, kind }));
        self.remove_ticket(kind, chunk, level);
    }
    pub fn update_chunk_forced(&mut self, chunk: Chunk, forced: bool) {
        if forced {
            self.add_region_ticket(TicketKind::Forced, chunk, 2);
        } else {
            self.remove_region_ticket(TicketKind::Forced, chunk, 2);
        }
    }
    pub fn region_ticket_created_tick(
        &self,
        kind: TicketKind,
        chunk: Chunk,
        level: i32,
    ) -> Option<i64> {
        self.region_tickets
            .get(&(packed(chunk), Ticket { level, kind }))
            .copied()
    }
    /// Host must call only when vanilla would purge. No represented ticket kind
    /// expires: START/PLAYER/FORCED all have timeout zero in exact 1.21.
    pub fn purge_stale_tickets(&mut self) {
        self.ticket_tick = self.ticket_tick.wrapping_add(1);
    }
    pub fn run_all_updates(&mut self) {
        self.run_updates(i32::MAX);
    }
    /// Original return value is remaining budget, including negative budgets.
    pub fn run_updates(&mut self, mut budget: i32) -> i32 {
        while budget > 0 {
            let Some(priority) = self.queues.iter().position(|q| q.first.is_some()) else {
                break;
            };
            budget -= 1;
            let key = self.queues[priority].pop().expect("nonempty priority");
            let old = self.level_key(key).clamp(0, 33);
            let next = self.computed.remove(&key).expect("queued computed level") as i32;
            if next < old {
                self.set_level(key, next);
                self.check_neighbors(key, next, true);
            } else if next > old {
                self.set_level(key, 33);
                if next != 33 {
                    self.enqueue(key, next.min(33));
                    self.computed.insert(key, next as u8);
                }
                self.check_neighbors(key, old, false);
            }
        }
        budget
    }
    fn level_key(&self, key: i64) -> i32 {
        self.levels.get(&key).copied().unwrap_or(33) as i32
    }
    fn set_level(&mut self, key: i64, level: i32) {
        if level >= 33 {
            self.levels.remove(&key);
        } else {
            self.levels.insert(key, level as u8);
        }
    }
    fn source_level(&self, key: i64) -> i32 {
        self.tickets
            .get(&key)
            .and_then(|s| s.first())
            .map_or(i32::MAX, |t| t.level)
    }
    fn neighbor_level(&self, from: i64, to: i64, level: i32) -> i32 {
        if from == SOURCE_CHUNK {
            self.source_level(to)
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
                level = level.min(self.neighbor_level(neighbor, key, self.level_key(neighbor)));
                if level == 0 {
                    return level;
                }
            }
        }
        level
    }
    fn enqueue(&mut self, key: i64, priority: i32) {
        self.queues[priority as usize].insert(key);
    }
    fn dequeue(&mut self, key: i64, priority: i32) {
        self.queues[priority as usize].remove(key);
    }
    fn check_edge(&mut self, from: i64, to: i64, level: i32, decreasing: bool) {
        let old = self.level_key(to);
        let computed = self.computed.get(&to).map_or(255, |v| *v as i32);
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
        let level = level.clamp(0, 33);
        let old = old.clamp(0, 33);
        let absent = computed == 255;
        let previous = if absent { old } else { computed };
        let next = if decreasing {
            previous.min(level)
        } else {
            self.calculated(to, from, level).clamp(0, 33)
        };
        let old_priority = old.min(previous).min(33);
        if old != next {
            let priority = old.min(next).min(33);
            if old_priority != priority && !absent {
                self.dequeue(to, old_priority);
            }
            self.enqueue(to, priority);
            self.computed.insert(to, next as u8);
        } else if !absent {
            self.dequeue(to, old_priority);
            self.computed.remove(&to);
        }
    }
    fn check_neighbor(&mut self, from: i64, to: i64, level: i32, decreasing: bool) {
        let computed = self.computed.get(&to).map_or(255, |v| *v as i32);
        let level = self.neighbor_level(from, to, level).clamp(0, 33);
        if decreasing {
            self.check_edge_values(from, to, level, self.level_key(to), computed, true);
        } else {
            let absent = computed == 255;
            let previous = if absent {
                self.level_key(to).clamp(0, 33)
            } else {
                computed
            };
            if level == previous {
                self.check_edge_values(
                    from,
                    to,
                    33,
                    if absent { previous } else { self.level_key(to) },
                    computed,
                    false,
                );
            }
        }
    }
    fn check_neighbors(&mut self, key: i64, level: i32, decreasing: bool) {
        if decreasing && level >= 32 {
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

impl Default for EntityTickingRange {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn chunk(value: &Value) -> Chunk {
        (
            value[0].as_i64().unwrap() as i32,
            value[1].as_i64().unwrap() as i32,
        )
    }
    fn kind(value: &Value) -> TicketKind {
        match value.as_str().unwrap() {
            "player" => TicketKind::Player,
            "start" => TicketKind::Start,
            "forced" => TicketKind::Forced,
            other => panic!("unknown fixture ticket kind: {other}"),
        }
    }
    fn kind_name(kind: TicketKind) -> &'static str {
        match kind {
            TicketKind::Player => "player",
            TicketKind::Start => "start",
            TicketKind::Forced => "forced",
        }
    }
    fn rows(values: Vec<(Chunk, i32)>) -> Value {
        Value::Array(
            values
                .into_iter()
                .map(|((x, z), level)| json!([x, z, level]))
                .collect(),
        )
    }
    fn snapshot(state: &EntityTickingRange, points: &Value, remaining: Option<i32>) -> Value {
        let mut tickets = Vec::new();
        for (&key, values) in &state.tickets {
            let (x, z) = unpacked(key);
            for ticket in values {
                tickets.push(json!([x, z, kind_name(ticket.kind), ticket.level]));
            }
        }
        tickets.sort_by_key(Value::to_string);
        let mut region_tickets = Vec::new();
        for (&(key, ticket), &tick) in &state.region_tickets {
            let (x, z) = unpacked(key);
            region_tickets.push(json!([x, z, kind_name(ticket.kind), ticket.level, tick]));
        }
        region_tickets.sort_by_key(Value::to_string);
        let queues: Vec<Value> = state
            .queued_chunks()
            .into_iter()
            .map(|q| Value::Array(q.into_iter().map(|(x, z)| json!([x, z])).collect()))
            .collect();
        json!({"levels":rows(state.stored_levels()),"computed":rows(state.computed_levels()),
            "queues":queues,"queue_size":state.queue_size(),"has_work":state.has_work(),
            "simulation_distance":state.simulation_distance(),"player_ticket_level":state.player_ticket_level(),
            "ticket_tick":state.ticket_tick(),"tickets":tickets,"region_tickets":region_tickets,
            "query_levels":points.as_array().unwrap().iter().map(|v|state.level(chunk(v))).collect::<Vec<_>>(),"remaining":remaining})
    }
    #[test]
    fn actual_original_tracker_and_distance_manager_sources() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../fixtures/entity_ticking_range_vectors.json"
        ))
        .unwrap();
        let mut total = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let query = &case["query"];
            let mut state =
                EntityTickingRange::new(query["simulation_distance"].as_i64().unwrap() as i32);
            state.ticket_tick = query["ticket_tick"].as_i64().unwrap();
            for (index, (op, expected)) in query["ops"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["expected"].as_array().unwrap())
                .enumerate()
            {
                let mut remaining = None;
                match op["op"].as_str().unwrap() {
                    "player_add" => {
                        state.add_player(chunk(&op["chunk"]), op["player"].as_i64().unwrap() as i32)
                    }
                    "player_remove" => {
                        state.remove_player(
                            chunk(&op["chunk"]),
                            op["player"].as_i64().unwrap() as i32,
                        );
                    }
                    "simulation" => {
                        state.update_simulation_distance(op["distance"].as_i64().unwrap() as i32)
                    }
                    "replace_players" => {
                        state.replace_player_tickets_level(op["level"].as_i64().unwrap() as i32)
                    }
                    "raw_add" => state.add_ticket(
                        kind(&op["kind"]),
                        chunk(&op["chunk"]),
                        op["level"].as_i64().unwrap() as i32,
                    ),
                    "raw_remove" => state.remove_ticket(
                        kind(&op["kind"]),
                        chunk(&op["chunk"]),
                        op["level"].as_i64().unwrap() as i32,
                    ),
                    "region_add" => state.add_region_ticket(
                        kind(&op["kind"]),
                        chunk(&op["chunk"]),
                        op["distance"].as_i64().unwrap() as i32,
                    ),
                    "region_remove" => state.remove_region_ticket(
                        kind(&op["kind"]),
                        chunk(&op["chunk"]),
                        op["distance"].as_i64().unwrap() as i32,
                    ),
                    "forced" => state
                        .update_chunk_forced(chunk(&op["chunk"]), op["enabled"].as_bool().unwrap()),
                    "purge" => {
                        for _ in 0..op["count"].as_i64().unwrap_or(1) {
                            state.purge_stale_tickets();
                        }
                    }
                    "run" => {
                        if let Some(budget) = op.get("budget") {
                            remaining = Some(state.run_updates(budget.as_i64().unwrap() as i32));
                        } else {
                            state.run_all_updates();
                        }
                    }
                    other => panic!("unknown fixture op: {other}"),
                }
                let actual = snapshot(&state, &query["points"], remaining);
                for (key, value) in expected.as_object().unwrap() {
                    assert_eq!(
                        &actual[key], value,
                        "case={} step={index} op={op} field={key}",
                        case["name"]
                    );
                }
                for (point, level) in query["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(expected["query_levels"].as_array().unwrap())
                {
                    assert_eq!(
                        state.in_entity_ticking_range(chunk(point)),
                        level.as_i64().unwrap() <= 31
                    );
                    assert_eq!(
                        state.in_block_ticking_range(chunk(point)),
                        level.as_i64().unwrap() <= 32
                    );
                }
                assert_eq!(
                    state.queue_size(),
                    state.queues.iter().map(|q| q.links.len()).sum::<usize>()
                );
                total += 1;
            }
        }
        println!("original TickingTracker / DistanceManager source states compared: {total}");
        assert!(total >= 1_000);
    }
}
