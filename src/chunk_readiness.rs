//! Literal ChunkMap range dependencies and CompletableFuture continuations.
//!
//! The backend supplies actual generation futures and executes required chunk
//! actions. An NBT Status string alone does not establish a successful source.
use std::collections::{BTreeMap, VecDeque};

pub type Chunk = (i32, i32);
pub type HolderId = u64;
pub type ChunkDataId = u64;
pub type FutureId = u64;
pub type TaskId = u64;
pub type ObserverId = u64;
pub const UNLOADED_RANGE: &str = "Unloaded chunks found in range";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChunkStatus(u8);
impl ChunkStatus {
    pub const EMPTY: Self = Self(0);
    pub const FULL: Self = Self(11);
    pub fn from_index(index: u8) -> Option<Self> {
        (index < 12).then_some(Self(index))
    }
    pub fn index(self) -> u8 {
        self.0
    }
    pub fn name(self) -> &'static str {
        [
            "minecraft:empty",
            "minecraft:structure_starts",
            "minecraft:structure_references",
            "minecraft:biomes",
            "minecraft:noise",
            "minecraft:surface",
            "minecraft:carvers",
            "minecraft:features",
            "minecraft:initialize_light",
            "minecraft:light",
            "minecraft:spawn",
            "minecraft:full",
        ][usize::from(self.0)]
    }
}

#[cfg(test)]
#[path = "../tools/chunk_readiness_tests.rs"]
mod tests;
/// Original ChunkLevel.generationStatus uses Java int subtraction, including
/// overflow at the i32 endpoints. Highest allowed status is a separate holder
/// generation-pass fact; callers must not infer it from NBT chunk contents.
pub fn allowed_generation_status(level: i32) -> Option<ChunkStatus> {
    let distance = level.wrapping_sub(33);
    if distance > 11 {
        None
    } else {
        Some(status_around_full(distance))
    }
}
pub fn status_around_full(distance: i32) -> ChunkStatus {
    if distance <= 0 {
        ChunkStatus::FULL
    } else if distance > 11 {
        ChunkStatus::EMPTY
    } else {
        ChunkStatus([11, 8, 6, 3, 1, 1, 1, 1, 1, 1, 1, 1][distance as usize])
    }
}
pub fn can_request(allowed: Option<ChunkStatus>, requested: ChunkStatus) -> bool {
    allowed.is_some_and(|status| status >= requested)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HolderRef {
    pub id: HolderId,
    pub chunk: Chunk,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkKind {
    Full,
    Proto,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkData {
    pub id: ChunkDataId,
    pub kind: ChunkKind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessError {
    pub class: String,
    pub message: String,
}
impl ReadinessError {
    pub fn new(class: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            class: class.into(),
            message: message.into(),
        }
    }
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new("java.lang.UnsupportedOperationException", message)
    }
    fn cancelled() -> Self {
        Self::new(
            "java.util.concurrent.CancellationException",
            "future cancelled",
        )
    }
}
impl std::fmt::Display for ReadinessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.class, self.message)
    }
}
impl std::error::Error for ReadinessError {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChunkOutcome {
    Pending,
    Success(ChunkData),
    Failure(String),
    Exceptional(ReadinessError),
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RangeOutcome {
    Success(Vec<ChunkData>),
    Failure(String),
    Exceptional(ReadinessError),
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSpec {
    /// Stable identity of one provider future. The initial value is used only
    /// when this identity is first interned; later changes use complete_source.
    pub identity: u64,
    pub initial: ChunkOutcome,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadinessKind {
    Accessible,
    BlockTicking,
    EntityTicking,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queue {
    Main,
    Priority { center: HolderRef },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutorMode {
    Queued,
    Inline,
    Reject,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadinessEffect {
    QueueTask {
        task: TaskId,
        queue: Queue,
    },
    ExecutorRejected {
        task: TaskId,
        queue: Queue,
    },
    PriorityAcknowledged {
        task: TaskId,
    },
    FutureCompleted {
        observer: ObserverId,
        future: FutureId,
        outcome: ChunkOutcome,
        generated_counter: i32,
    },
}

/// These callbacks are required operations, not optional notifications. A
/// backend that cannot perform an action returns an explicit error; the
/// corresponding preparation future then completes exceptionally.
pub trait ReadinessHost {
    fn lookup_holder(&mut self, chunk: Chunk) -> Option<HolderRef>;
    fn get_or_schedule(
        &mut self,
        holder: HolderRef,
        status: ChunkStatus,
    ) -> Result<SourceSpec, ReadinessError>;
    fn post_process_generation(&mut self, chunk: ChunkDataId) -> Result<(), ReadinessError>;
    /// Original ServerLevel.startTickingChunk: unpack both saved tick containers
    /// against the current level game time. Global tick registration is separate.
    fn start_ticking_chunk(&mut self, chunk: ChunkDataId) -> Result<(), ReadinessError>;
    fn send_chunk(&mut self, chunk: ChunkDataId) -> Result<(), ReadinessError>;
    /// Optional synchronous effect observer, useful when composing an external
    /// executor/event log. Returned effect vectors contain the same events; a
    /// host that applies events here must not apply them a second time later.
    fn observe_effect(&mut self, _effect: &ReadinessEffect) {}
}

#[derive(Clone, Debug)]
enum Value {
    Unit,
    Chunk(Result<ChunkData, String>),
    List(Result<Vec<ChunkData>, String>),
}
#[derive(Clone, Debug)]
enum State {
    Pending,
    Value(Value),
    Exceptional(ReadinessError),
    Cancelled,
}
impl State {
    fn done(&self) -> bool {
        !matches!(self, Self::Pending)
    }
    fn propagated_error(&self) -> Option<ReadinessError> {
        match self {
            Self::Exceptional(error) => Some(error.clone()),
            Self::Cancelled => Some(ReadinessError::cancelled()),
            _ => None,
        }
    }
}
#[derive(Clone, Debug)]
struct Future {
    state: State,
    payload: Payload,
    dependents: Vec<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Payload {
    Chunk,
    Range,
    Signal,
}
#[derive(Clone, Debug)]
enum Operation {
    Single,
    Sequence(Vec<FutureId>),
    Range,
    Center,
    Tick(HolderRef),
    Send(ChunkDataId),
}
#[derive(Clone, Debug)]
enum DependentKind {
    Both {
        left: FutureId,
        right: FutureId,
        target: FutureId,
    },
    Map {
        source: FutureId,
        target: FutureId,
        operation: Operation,
        queue: Option<Queue>,
        scheduled: bool,
    },
    Observe {
        source: FutureId,
        observer: ObserverId,
    },
    Counter {
        source: FutureId,
    },
}
#[derive(Clone, Debug)]
struct Dependent {
    kind: DependentKind,
    finished: bool,
}
#[derive(Clone, Copy, Debug)]
struct Task {
    dependent: usize,
    queue: Queue,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FireMode {
    Sync,
    Nested,
    Async,
}

#[derive(Clone, Debug)]
pub struct ChunkReadiness {
    futures: Vec<Future>,
    sources: BTreeMap<u64, FutureId>,
    dependents: Vec<Dependent>,
    tasks: BTreeMap<TaskId, Task>,
    pending: VecDeque<TaskId>,
    next_task: TaskId,
    main_mode: ExecutorMode,
    priority_mode: ExecutorMode,
    send_sync: BTreeMap<HolderId, FutureId>,
    generated_counter: i32,
}
impl Default for ChunkReadiness {
    fn default() -> Self {
        Self::new()
    }
}
impl ChunkReadiness {
    fn emit<H: ReadinessHost>(
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
        effect: ReadinessEffect,
    ) {
        host.observe_effect(&effect);
        effects.push(effect);
    }
    pub fn new() -> Self {
        Self {
            futures: vec![
                Future {
                    state: State::Value(Value::Unit),
                    payload: Payload::Signal,
                    dependents: Vec::new(),
                },
                Future {
                    state: State::Value(Value::List(Err(UNLOADED_RANGE.into()))),
                    payload: Payload::Range,
                    dependents: Vec::new(),
                },
            ],
            sources: BTreeMap::new(),
            dependents: Vec::new(),
            tasks: BTreeMap::new(),
            pending: VecDeque::new(),
            next_task: 1,
            main_mode: ExecutorMode::Queued,
            priority_mode: ExecutorMode::Queued,
            send_sync: BTreeMap::new(),
            generated_counter: 0,
        }
    }
    fn node(&mut self, state: State, payload: Payload) -> FutureId {
        let id = self.futures.len() as u64;
        self.futures.push(Future {
            state,
            payload,
            dependents: Vec::new(),
        });
        id
    }
    fn state(&self, id: FutureId) -> &State {
        &self.futures[id as usize].state
    }
    fn from_outcome(value: ChunkOutcome) -> State {
        match value {
            ChunkOutcome::Pending => State::Pending,
            ChunkOutcome::Success(data) => State::Value(Value::Chunk(Ok(data))),
            ChunkOutcome::Failure(reason) => State::Value(Value::Chunk(Err(reason))),
            ChunkOutcome::Exceptional(error) => State::Exceptional(error),
            ChunkOutcome::Cancelled => State::Cancelled,
        }
    }
    pub fn register_source(&mut self, spec: SourceSpec) -> FutureId {
        if let Some(id) = self.sources.get(&spec.identity) {
            return *id;
        }
        let id = self.node(Self::from_outcome(spec.initial), Payload::Chunk);
        self.sources.insert(spec.identity, id);
        id
    }
    pub fn source_future(&self, identity: u64) -> Option<FutureId> {
        self.sources.get(&identity).copied()
    }
    pub fn outcome(&self, id: FutureId) -> Option<ChunkOutcome> {
        match &self.futures.get(id as usize)?.state {
            State::Pending => None,
            State::Value(Value::Chunk(Ok(data))) => Some(ChunkOutcome::Success(*data)),
            State::Value(Value::Chunk(Err(reason))) => Some(ChunkOutcome::Failure(reason.clone())),
            State::Exceptional(error) => Some(ChunkOutcome::Exceptional(error.clone())),
            State::Cancelled => Some(ChunkOutcome::Cancelled),
            _ => None,
        }
    }
    pub fn generated_counter(&self) -> i32 {
        self.generated_counter
    }
    pub fn range_outcome(&self, id: FutureId) -> Option<RangeOutcome> {
        match &self.futures.get(id as usize)?.state {
            State::Value(Value::List(Ok(data))) => Some(RangeOutcome::Success(data.clone())),
            State::Value(Value::List(Err(reason))) => Some(RangeOutcome::Failure(reason.clone())),
            State::Exceptional(error) => Some(RangeOutcome::Exceptional(error.clone())),
            State::Cancelled => Some(RangeOutcome::Cancelled),
            _ => None,
        }
    }
    pub fn pending_tasks(&self) -> Vec<(TaskId, Queue)> {
        self.pending
            .iter()
            .map(|id| (*id, self.tasks[id].queue))
            .collect()
    }
    pub fn set_executor_mode(&mut self, queue: Queue, mode: ExecutorMode) {
        match queue {
            Queue::Main => self.main_mode = mode,
            Queue::Priority { .. } => self.priority_mode = mode,
        }
    }
    pub fn set_send_sync_future(
        &mut self,
        holder: HolderId,
        future: FutureId,
    ) -> Result<(), ReadinessError> {
        if future as usize >= self.futures.len() {
            return Err(ReadinessError::unsupported(
                "unknown send synchronization future",
            ));
        }
        self.send_sync.insert(holder, future);
        Ok(())
    }
    /// ChunkHolder.getChunkToSend checks only sendSync.isDone before reading
    /// its current ticking-chunk future. Exceptional and cancelled barriers
    /// are done too; this getter does not unwrap their result. A holder without
    /// an added dependency retains the constructor's completed-void barrier.
    pub fn send_sync_done(&self, holder: HolderRef) -> bool {
        let future = self.send_sync.get(&holder.id).copied().unwrap_or(0);
        self.state(future).done()
    }
    pub fn register_signal(&mut self, value: ChunkOutcome) -> FutureId {
        let state = match value {
            ChunkOutcome::Pending => State::Pending,
            ChunkOutcome::Exceptional(e) => State::Exceptional(e),
            ChunkOutcome::Cancelled => State::Cancelled,
            _ => State::Value(Value::Unit),
        };
        self.node(state, Payload::Signal)
    }
    fn dependent(&mut self, kind: DependentKind) -> usize {
        let id = self.dependents.len();
        self.dependents.push(Dependent {
            kind,
            finished: false,
        });
        id
    }
    fn attach(&mut self, source: FutureId, dependent: usize) {
        if !self.state(source).done() {
            self.futures[source as usize].dependents.push(dependent);
        }
    }
    fn store_state(&mut self, id: FutureId, state: State) -> bool {
        if self.state(id).done() {
            return matches!(state, State::Cancelled) && matches!(self.state(id), State::Cancelled);
        }
        self.futures[id as usize].state = state;
        true
    }
    fn complete_state<H: ReadinessHost>(
        &mut self,
        id: FutureId,
        state: State,
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) -> bool {
        let changed = self.store_state(id, state);
        if changed {
            self.post_complete(id, host, effects);
        }
        changed
    }
    /// JDK21 postComplete moves a nested future's leading completion back onto
    /// the original root when that nested future has multiple dependents. A
    /// recursive LIFO walk gives the wrong handle/counter observer order.
    fn post_complete<H: ReadinessHost>(
        &mut self,
        root: FutureId,
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) {
        let mut current = root;
        loop {
            let Some(dep) = self.futures[current as usize].dependents.pop() else {
                if current != root {
                    current = root;
                    continue;
                }
                break;
            };
            if current != root && !self.futures[current as usize].dependents.is_empty() {
                self.futures[root as usize].dependents.push(dep);
                continue;
            }
            current = self
                .fire(dep, FireMode::Nested, host, effects)
                .unwrap_or(root);
        }
    }
    fn post_fire<H: ReadinessHost>(
        &mut self,
        target: FutureId,
        sources: &[FutureId],
        mode: FireMode,
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) -> Option<FutureId> {
        if mode != FireMode::Nested {
            for source in sources {
                if !self.futures[*source as usize].dependents.is_empty()
                    && self.state(*source).done()
                {
                    self.post_complete(*source, host, effects);
                }
            }
        }
        if self.state(target).done() && !self.futures[target as usize].dependents.is_empty() {
            if mode == FireMode::Nested {
                return Some(target);
            }
            self.post_complete(target, host, effects);
        }
        None
    }
    fn map<H: ReadinessHost>(
        &mut self,
        source: FutureId,
        operation: Operation,
        queue: Option<Queue>,
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) -> FutureId {
        let payload = match operation {
            Operation::Single | Operation::Sequence(_) | Operation::Range => Payload::Range,
            Operation::Center | Operation::Tick(_) => Payload::Chunk,
            Operation::Send(_) => Payload::Signal,
        };
        let target = self.node(State::Pending, payload);
        let dep = self.dependent(DependentKind::Map {
            source,
            target,
            operation,
            queue,
            scheduled: false,
        });
        self.attach(source, dep);
        self.fire(dep, FireMode::Sync, host, effects);
        target
    }
    fn all_of<H: ReadinessHost>(
        &mut self,
        sources: &[FutureId],
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) -> FutureId {
        if sources.len() == 1 {
            return sources[0];
        }
        let mid = sources.len().div_ceil(2);
        let left = self.all_of(&sources[..mid], host, effects);
        let right = self.all_of(&sources[mid..], host, effects);
        let target = self.node(State::Pending, Payload::Signal);
        let dep = self.dependent(DependentKind::Both {
            left,
            right,
            target,
        });
        self.attach(left, dep);
        if right != left {
            self.attach(right, dep);
        }
        self.fire(dep, FireMode::Sync, host, effects);
        target
    }
    fn sequence<H: ReadinessHost>(
        &mut self,
        sources: Vec<FutureId>,
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) -> FutureId {
        if sources.is_empty() {
            return self.node(State::Value(Value::List(Ok(Vec::new()))), Payload::Range);
        }
        if sources.len() == 1 {
            return self.map(sources[0], Operation::Single, None, host, effects);
        }
        let all = self.all_of(&sources, host, effects);
        self.map(all, Operation::Sequence(sources), None, host, effects)
    }
    fn schedule<H: ReadinessHost>(
        &mut self,
        dep: usize,
        queue: Queue,
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) -> Result<(), ReadinessError> {
        let task = self.next_task;
        self.next_task += 1;
        self.tasks.insert(
            task,
            Task {
                dependent: dep,
                queue,
            },
        );
        let mode = match queue {
            Queue::Main => self.main_mode,
            Queue::Priority { .. } => self.priority_mode,
        };
        if mode == ExecutorMode::Reject {
            Self::emit(
                host,
                effects,
                ReadinessEffect::ExecutorRejected { task, queue },
            );
            return Err(ReadinessError::new(
                "java.util.concurrent.RejectedExecutionException",
                "executor rejected",
            ));
        } else {
            Self::emit(host, effects, ReadinessEffect::QueueTask { task, queue });
            if mode == ExecutorMode::Inline {
                self.run_task_inner(task, host, effects);
            } else {
                self.pending.push_back(task);
            }
        }
        Ok(())
    }
    fn operation<H: ReadinessHost>(
        &mut self,
        value: Value,
        operation: Operation,
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) -> Result<Value, ReadinessError> {
        match operation {
            Operation::Single => match value {
                Value::Chunk(Ok(data)) => Ok(Value::List(Ok(vec![data]))),
                Value::Chunk(Err(reason)) => Ok(Value::List(Err(reason))),
                _ => Err(ReadinessError::unsupported("invalid single chunk result")),
            },
            Operation::Sequence(sources) => {
                let mut data = Vec::new();
                for source in sources {
                    match self.state(source) {
                        State::Value(Value::Chunk(Ok(chunk))) => data.push(*chunk),
                        State::Value(Value::Chunk(Err(_))) => {
                            return Ok(Value::List(Err(UNLOADED_RANGE.into())))
                        }
                        _ => {
                            return Err(ReadinessError::unsupported(
                                "invalid range dependency result",
                            ))
                        }
                    }
                }
                Ok(Value::List(Ok(data)))
            }
            Operation::Range => Ok(value),
            Operation::Center => match value {
                Value::List(Err(reason)) => Ok(Value::Chunk(Err(reason))),
                Value::List(Ok(data)) => {
                    let center = data[data.len() / 2];
                    if center.kind != ChunkKind::Full {
                        return Err(ReadinessError::new(
                            "java.lang.ClassCastException",
                            "center is not a LevelChunk",
                        ));
                    }
                    Ok(Value::Chunk(Ok(center)))
                }
                _ => Err(ReadinessError::unsupported("invalid center result")),
            },
            Operation::Tick(holder) => match value {
                Value::Chunk(Err(reason)) => Ok(Value::Chunk(Err(reason))),
                Value::Chunk(Ok(data)) => {
                    host.post_process_generation(data.id)?;
                    host.start_ticking_chunk(data.id)?;
                    let send = self.send_sync.get(&holder.id).copied().unwrap_or(0);
                    if self.state(send).done() {
                        host.send_chunk(data.id)?;
                    } else {
                        self.map(
                            send,
                            Operation::Send(data.id),
                            Some(Queue::Main),
                            host,
                            effects,
                        );
                    }
                    Ok(Value::Chunk(Ok(data)))
                }
                _ => Err(ReadinessError::unsupported("invalid ticking result")),
            },
            Operation::Send(chunk) => {
                host.send_chunk(chunk)?;
                Ok(Value::Unit)
            }
        }
    }
    fn fire<H: ReadinessHost>(
        &mut self,
        id: usize,
        mode: FireMode,
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) -> Option<FutureId> {
        if self.dependents[id].finished {
            return None;
        }
        match self.dependents[id].kind.clone() {
            DependentKind::Both {
                left,
                right,
                target,
            } => {
                if !self.state(left).done() || !self.state(right).done() {
                    return None;
                }
                self.dependents[id].finished = true;
                let error = self
                    .state(left)
                    .propagated_error()
                    .or_else(|| self.state(right).propagated_error());
                self.store_state(
                    target,
                    error
                        .map(State::Exceptional)
                        .unwrap_or(State::Value(Value::Unit)),
                );
                return self.post_fire(target, &[left, right], mode, host, effects);
            }
            DependentKind::Map {
                source,
                target,
                operation,
                queue,
                scheduled,
            } => {
                if self.state(target).done() {
                    self.dependents[id].finished = true;
                    return self.post_fire(target, &[source], mode, host, effects);
                }
                if !self.state(source).done() {
                    return None;
                }
                if let Some(error) = self.state(source).propagated_error() {
                    self.dependents[id].finished = true;
                    self.store_state(target, State::Exceptional(error));
                    return self.post_fire(target, &[source], mode, host, effects);
                }
                if let Some(queue) = queue {
                    if mode != FireMode::Async {
                        if !scheduled {
                            if let DependentKind::Map { scheduled, .. } =
                                &mut self.dependents[id].kind
                            {
                                *scheduled = true;
                            }
                            if let Err(error) = self.schedule(id, queue, host, effects) {
                                self.dependents[id].finished = true;
                                self.store_state(target, State::Exceptional(error));
                                return self.post_fire(target, &[source], mode, host, effects);
                            }
                        }
                        return None;
                    }
                }
                self.dependents[id].finished = true;
                let State::Value(value) = self.state(source).clone() else {
                    unreachable!()
                };
                let result = match self.operation(value, operation, host, effects) {
                    Ok(value) => State::Value(value),
                    Err(error) => State::Exceptional(error),
                };
                self.store_state(target, result);
                return self.post_fire(target, &[source], mode, host, effects);
            }
            DependentKind::Observe { source, observer } => {
                if !self.state(source).done() {
                    return None;
                }
                self.dependents[id].finished = true;
                if let Some(outcome) = self.outcome(source) {
                    Self::emit(
                        host,
                        effects,
                        ReadinessEffect::FutureCompleted {
                            observer,
                            future: source,
                            outcome,
                            generated_counter: self.generated_counter,
                        },
                    );
                }
            }
            DependentKind::Counter { source } => {
                if self.state(source).done() {
                    self.dependents[id].finished = true;
                    self.generated_counter = self.generated_counter.wrapping_add(1);
                }
            }
        }
        None
    }
    pub fn range_future<H: ReadinessHost>(
        &mut self,
        center: HolderRef,
        radius: u8,
        statuses: &[ChunkStatus],
        host: &mut H,
    ) -> Result<(FutureId, Vec<ReadinessEffect>), ReadinessError> {
        if statuses.len() <= usize::from(radius) {
            return Err(ReadinessError::unsupported(
                "missing range status function entries",
            ));
        }
        let mut effects = Vec::new();
        if radius == 0 {
            let spec = host.get_or_schedule(center, statuses[0])?;
            let source = self.register_source(spec);
            let result = self.map(source, Operation::Single, None, host, &mut effects);
            return Ok((result, effects));
        }
        let mut sources = Vec::new();
        let r = i32::from(radius);
        for dz in -r..=r {
            for dx in -r..=r {
                let chunk = (
                    center.chunk.0.wrapping_add(dx),
                    center.chunk.1.wrapping_add(dz),
                );
                let Some(holder) = host.lookup_holder(chunk) else {
                    return Ok((1, effects));
                };
                let distance = dx.abs().max(dz.abs()) as usize;
                let spec = host.get_or_schedule(holder, statuses[distance])?;
                sources.push(self.register_source(spec));
            }
        }
        let sequence = self.sequence(sources, host, &mut effects);
        let result = self.map(sequence, Operation::Range, None, host, &mut effects);
        Ok((result, effects))
    }
    pub fn prepare<H: ReadinessHost>(
        &mut self,
        kind: ReadinessKind,
        center: HolderRef,
        host: &mut H,
    ) -> Result<(FutureId, Vec<ReadinessEffect>), ReadinessError> {
        let (radius, statuses) = match kind {
            ReadinessKind::Accessible => (1, vec![ChunkStatus::FULL, status_around_full(1)]),
            ReadinessKind::BlockTicking => (1, vec![ChunkStatus::FULL; 2]),
            ReadinessKind::EntityTicking => (2, vec![ChunkStatus::FULL; 3]),
        };
        let (range, mut effects) = self.range_future(center, radius, &statuses, host)?;
        let queue = if kind == ReadinessKind::EntityTicking {
            Queue::Main
        } else {
            Queue::Priority { center }
        };
        let mapped = self.map(range, Operation::Center, Some(queue), host, &mut effects);
        let result = if kind == ReadinessKind::BlockTicking {
            let result = self.map(
                mapped,
                Operation::Tick(center),
                Some(Queue::Main),
                host,
                &mut effects,
            );
            let counter = self.dependent(DependentKind::Counter { source: result });
            self.attach(result, counter);
            self.fire(counter, FireMode::Sync, host, &mut effects);
            result
        } else {
            mapped
        };
        Ok((result, effects))
    }
    pub fn observe<H: ReadinessHost>(
        &mut self,
        future: FutureId,
        observer: ObserverId,
        host: &mut H,
    ) -> Result<Vec<ReadinessEffect>, ReadinessError> {
        if future as usize >= self.futures.len()
            || self.futures[future as usize].payload != Payload::Chunk
        {
            return Err(ReadinessError::unsupported(
                "observer requires a chunk-result future",
            ));
        }
        let mut effects = Vec::new();
        let dep = self.dependent(DependentKind::Observe {
            source: future,
            observer,
        });
        self.attach(future, dep);
        self.fire(dep, FireMode::Sync, host, &mut effects);
        Ok(effects)
    }
    pub fn complete_future<H: ReadinessHost>(
        &mut self,
        future: FutureId,
        outcome: ChunkOutcome,
        host: &mut H,
    ) -> Result<(bool, Vec<ReadinessEffect>), ReadinessError> {
        if future as usize >= self.futures.len()
            || self.futures[future as usize].payload != Payload::Chunk
            || outcome == ChunkOutcome::Pending
        {
            return Err(ReadinessError::unsupported(
                "invalid chunk future completion",
            ));
        }
        let mut effects = Vec::new();
        let changed = self.complete_state(future, Self::from_outcome(outcome), host, &mut effects);
        Ok((changed, effects))
    }
    pub fn complete_source<H: ReadinessHost>(
        &mut self,
        identity: u64,
        outcome: ChunkOutcome,
        host: &mut H,
    ) -> Result<(bool, Vec<ReadinessEffect>), ReadinessError> {
        let id = self
            .source_future(identity)
            .ok_or_else(|| ReadinessError::unsupported("source has not been requested"))?;
        self.complete_future(id, outcome, host)
    }
    pub fn complete_signal<H: ReadinessHost>(
        &mut self,
        future: FutureId,
        outcome: ChunkOutcome,
        host: &mut H,
    ) -> Result<(bool, Vec<ReadinessEffect>), ReadinessError> {
        if future as usize >= self.futures.len()
            || self.futures[future as usize].payload != Payload::Signal
            || outcome == ChunkOutcome::Pending
        {
            return Err(ReadinessError::unsupported("invalid signal completion"));
        }
        let state = match outcome {
            ChunkOutcome::Exceptional(error) => State::Exceptional(error),
            ChunkOutcome::Cancelled => State::Cancelled,
            _ => State::Value(Value::Unit),
        };
        let mut effects = Vec::new();
        let changed = self.complete_state(future, state, host, &mut effects);
        Ok((changed, effects))
    }
    fn run_task_inner<H: ReadinessHost>(
        &mut self,
        task: TaskId,
        host: &mut H,
        effects: &mut Vec<ReadinessEffect>,
    ) {
        let entry = self.tasks[&task];
        self.pending.retain(|id| *id != task);
        self.fire(entry.dependent, FireMode::Async, host, effects);
        if matches!(entry.queue, Queue::Priority { .. }) {
            Self::emit(
                host,
                effects,
                ReadinessEffect::PriorityAcknowledged { task },
            );
        }
    }
    pub fn run_task<H: ReadinessHost>(
        &mut self,
        task: TaskId,
        host: &mut H,
    ) -> Result<Vec<ReadinessEffect>, ReadinessError> {
        if !self.tasks.contains_key(&task) {
            return Err(ReadinessError::unsupported("unknown readiness task"));
        }
        let mut effects = Vec::new();
        self.run_task_inner(task, host, &mut effects);
        Ok(effects)
    }
}
