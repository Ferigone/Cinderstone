//! Original Java 1.21 ChunkHolder.updateFutures confirmation and executor state.
//!
//! Requests are real future handles supplied by the host, and executor tasks run
//! only when the host invokes them. No tick delay stands in for completion.
//! Cancellation is literal: cancelling a completed confirmation cannot remove
//! its already-enqueued thenRunAsync task. Published status can consequently
//! differ from the current ticket level until executor work runs.
use std::collections::{BTreeMap, VecDeque};

pub type FutureId = u64;
pub type TaskId = u64;
pub const MAX_LEVEL: i32 = 44;
pub const UNLOADED_FUTURE: FutureId = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum FullStatus {
    Inaccessible = 0,
    Full = 1,
    BlockTicking = 2,
    EntityTicking = 3,
}
impl FullStatus {
    pub fn from_level(level: i32) -> Self {
        if level <= 31 {
            Self::EntityTicking
        } else if level <= 32 {
            Self::BlockTicking
        } else if level <= 33 {
            Self::Full
        } else {
            Self::Inaccessible
        }
    }
    pub fn visibility(self) -> Visibility {
        match self {
            Self::Inaccessible => Visibility::Hidden,
            Self::Full | Self::BlockTicking => Visibility::Tracked,
            Self::EntityTicking => Visibility::Ticking,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Visibility {
    Hidden = 0,
    Tracked = 1,
    Ticking = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Full,
    Block,
    Entity,
}
impl Stage {
    pub fn status(self) -> FullStatus {
        match self {
            Self::Full => FullStatus::Full,
            Self::Block => FullStatus::BlockTicking,
            Self::Entity => FullStatus::EntityTicking,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Block => "block",
            Self::Entity => "entity",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Full => 0,
            Self::Block => 1,
            Self::Entity => 2,
        }
    }
}
/// A normally completed ChunkResult failure differs from an exceptional or
/// cancelled CompletableFuture. Only Success completes the promotion gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FutureState {
    Pending,
    Success,
    Failure,
    Unloaded,
    Exceptional,
    Cancelled,
}
impl FutureState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Unloaded => "unloaded",
            Self::Exceptional => "exceptional",
            Self::Cancelled => "cancelled",
        }
    }
    fn exceptional(self) -> bool {
        matches!(self, Self::Exceptional | Self::Cancelled)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmationState {
    Pending,
    Completed,
    Cancelled,
}
impl ConfirmationState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutorMode {
    Queued,
    Inline,
    Reject,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preparation {
    pub stage: Stage,
    pub future: FutureId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedFuture {
    pub future: FutureId,
    pub initial_state: FutureState,
}

/// A composed host must prepare each stage when requested and publish effects
/// synchronously. In particular it cannot prepare all three stages in advance:
/// already-completed earlier stages can enqueue work before the next prepare.
pub trait ChunkLifecycleHost {
    fn prepare(&mut self, stage: Stage) -> Result<PreparedFuture, String>;
    fn emit(&mut self, effect: LifecycleEffect);
    /// Complete the same provider future when demotion completes a pending
    /// local handle with UNLOADED. Its other registered observers must run here,
    /// before slots are replaced and before the immediate demotion callback.
    fn complete_prepared_future(&mut self, future: FutureId, state: FutureState);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleEffect {
    Prepare {
        stage: Stage,
        future: FutureId,
    },
    Enqueue {
        task: TaskId,
    },
    ExecutorRejected {
        task: TaskId,
    },
    FullStatusChange {
        status: FullStatus,
        visibility: Visibility,
    },
    /// The getter observes queue_level lazily; applying this notification later
    /// writes its new_level, even if a newer notification already ran.
    LevelChange {
        queue_level: i32,
        new_level: i32,
    },
}
#[derive(Clone, Debug)]
struct SourceFuture {
    state: FutureState,
    confirmations: Vec<usize>,
}
#[derive(Clone, Copy, Debug)]
struct Confirmation {
    state: ConfirmationState,
    status: FullStatus,
}
#[derive(Clone, Copy, Debug)]
struct ExecutorTask {
    status: FullStatus,
    finished: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub old_level: i32,
    pub ticket_level: i32,
    pub queue_level: i32,
    pub was_accessible: bool,
    pub confirmation: ConfirmationState,
    pub output_status: Option<FullStatus>,
    pub slots: [FutureId; 3],
    pub futures: Vec<(FutureId, FutureState)>,
    pub pending_tasks: Vec<TaskId>,
    pub save_done: bool,
    pub save_exceptional: bool,
    pub level_getter_values: Vec<i32>,
}

#[derive(Clone, Debug)]
pub struct ChunkLifecycle {
    old_level: i32,
    ticket_level: i32,
    queue_level: i32,
    was_accessible: bool,
    output_status: Option<FullStatus>,
    slots: [FutureId; 3],
    futures: BTreeMap<FutureId, SourceFuture>,
    confirmations: Vec<Confirmation>,
    current_confirmation: usize,
    tasks: BTreeMap<TaskId, ExecutorTask>,
    pending_tasks: VecDeque<TaskId>,
    next_task: TaskId,
    executor_mode: ExecutorMode,
    apply_queue_inline: bool,
    level_notifications: Vec<i32>,
    save_dependencies: Vec<FutureId>,
}
impl ChunkLifecycle {
    pub fn new(ticket_level: i32) -> Self {
        let mut futures = BTreeMap::new();
        futures.insert(
            UNLOADED_FUTURE,
            SourceFuture {
                state: FutureState::Unloaded,
                confirmations: Vec::new(),
            },
        );
        Self {
            old_level: MAX_LEVEL + 1,
            ticket_level,
            queue_level: MAX_LEVEL + 1,
            was_accessible: false,
            output_status: None,
            slots: [UNLOADED_FUTURE; 3],
            futures,
            confirmations: vec![Confirmation {
                state: ConfirmationState::Completed,
                status: FullStatus::Inaccessible,
            }],
            current_confirmation: 0,
            tasks: BTreeMap::new(),
            pending_tasks: VecDeque::new(),
            next_task: 1,
            executor_mode: ExecutorMode::Queued,
            apply_queue_inline: true,
            level_notifications: Vec::new(),
            save_dependencies: Vec::new(),
        }
    }
    pub fn register_future(&mut self, id: FutureId, state: FutureState) -> Result<(), String> {
        if self.futures.contains_key(&id) {
            return Err(format!("future {id} already registered"));
        }
        self.futures.insert(
            id,
            SourceFuture {
                state,
                confirmations: Vec::new(),
            },
        );
        Ok(())
    }
    pub fn set_ticket_level(&mut self, level: i32) {
        self.ticket_level = level;
    }
    pub fn set_executor_mode(&mut self, mode: ExecutorMode) {
        self.executor_mode = mode;
    }
    pub fn set_apply_queue_inline(&mut self, value: bool) {
        self.apply_queue_inline = value;
    }
    pub fn set_queue_level(&mut self, level: i32) {
        self.queue_level = level;
    }
    pub fn queue_level(&self) -> i32 {
        self.queue_level
    }
    pub fn future_state(&self, future: FutureId) -> Option<FutureState> {
        self.futures.get(&future).map(|f| f.state)
    }
    pub fn stage_future(&self, stage: Stage) -> FutureId {
        self.slots[stage.index()]
    }
    pub fn apply_level_change(&mut self, notification: usize) -> Result<(), String> {
        self.queue_level = *self
            .level_notifications
            .get(notification)
            .ok_or("unknown level notification")?;
        Ok(())
    }
    pub fn refresh_accessibility(&mut self) {
        self.was_accessible = FullStatus::from_level(self.ticket_level) >= FullStatus::Full;
    }
    pub fn output_status(&self) -> Option<FullStatus> {
        self.output_status
    }
    pub fn output_visibility(&self) -> Option<Visibility> {
        self.output_status.map(FullStatus::visibility)
    }

    fn cancel_confirmation(&mut self) {
        let confirmation = &mut self.confirmations[self.current_confirmation];
        if confirmation.state == ConfirmationState::Pending {
            confirmation.state = ConfirmationState::Cancelled;
        }
    }
    fn publish(&mut self, status: FullStatus, effects: &mut Vec<LifecycleEffect>) {
        self.output_status = Some(status);
        effects.push(LifecycleEffect::FullStatusChange {
            status,
            visibility: status.visibility(),
        });
    }
    fn complete_confirmation(&mut self, id: usize, effects: &mut Vec<LifecycleEffect>) {
        let confirmation = &mut self.confirmations[id];
        if confirmation.state != ConfirmationState::Pending {
            return;
        }
        confirmation.state = ConfirmationState::Completed;
        let task = self.next_task;
        self.next_task += 1;
        self.tasks.insert(
            task,
            ExecutorTask {
                status: confirmation.status,
                finished: false,
            },
        );
        match self.executor_mode {
            ExecutorMode::Reject => {
                self.tasks.get_mut(&task).unwrap().finished = true;
                effects.push(LifecycleEffect::ExecutorRejected { task });
            }
            ExecutorMode::Queued => {
                self.pending_tasks.push_back(task);
                effects.push(LifecycleEffect::Enqueue { task });
            }
            ExecutorMode::Inline => {
                effects.push(LifecycleEffect::Enqueue { task });
                self.run_task_inner(task, effects)
                    .expect("newly registered task");
            }
        }
    }
    fn complete_inner(
        &mut self,
        id: FutureId,
        value: FutureState,
        effects: &mut Vec<LifecycleEffect>,
    ) -> Result<bool, String> {
        if value == FutureState::Pending {
            return Err("cannot complete a future with Pending".into());
        }
        let source = self
            .futures
            .get_mut(&id)
            .ok_or_else(|| format!("unknown future {id}"))?;
        if source.state != FutureState::Pending {
            return Ok(value == FutureState::Cancelled && source.state == FutureState::Cancelled);
        }
        source.state = value;
        // CompletableFuture completion dependents are a stack, not a FIFO.
        let confirmations = source.confirmations.clone();
        if value == FutureState::Success {
            for confirmation in confirmations.into_iter().rev() {
                self.complete_confirmation(confirmation, effects);
            }
        }
        Ok(true)
    }
    pub fn complete_future(
        &mut self,
        id: FutureId,
        value: FutureState,
    ) -> Result<(bool, Vec<LifecycleEffect>), String> {
        let mut effects = Vec::new();
        let completed = self.complete_inner(id, value, &mut effects)?;
        Ok((completed, effects))
    }
    fn save_done(&self) -> bool {
        self.save_dependencies
            .iter()
            .all(|id| self.futures[id].state != FutureState::Pending)
    }
    fn save_exceptional(&self) -> bool {
        self.save_done()
            && self
                .save_dependencies
                .iter()
                .any(|id| self.futures[id].state.exceptional())
    }
    fn add_save_dependency(&mut self, id: FutureId) {
        if self.save_done() {
            self.save_dependencies.clear();
        }
        self.save_dependencies.push(id);
    }
    pub fn update_futures(
        &mut self,
        preparations: &[Preparation],
    ) -> Result<Vec<LifecycleEffect>, String> {
        let old = FullStatus::from_level(self.old_level);
        let new = FullStatus::from_level(self.ticket_level);
        let expected: Vec<_> = [Stage::Full, Stage::Block, Stage::Entity]
            .into_iter()
            .filter(|stage| old < stage.status() && new >= stage.status())
            .collect();
        if expected.len() != preparations.len()
            || expected.iter().zip(preparations).any(|(stage, plan)| {
                *stage != plan.stage || !self.futures.contains_key(&plan.future)
            })
        {
            return Err("preparation list must match entered stages and registered futures".into());
        }
        struct BatchHost<'a> {
            plans: std::slice::Iter<'a, Preparation>,
            states: BTreeMap<FutureId, FutureState>,
            effects: Vec<LifecycleEffect>,
        }
        impl ChunkLifecycleHost for BatchHost<'_> {
            fn prepare(&mut self, stage: Stage) -> Result<PreparedFuture, String> {
                let plan = self.plans.next().expect("validated preparation count");
                debug_assert_eq!(plan.stage, stage);
                Ok(PreparedFuture {
                    future: plan.future,
                    initial_state: self.states[&plan.future],
                })
            }
            fn emit(&mut self, effect: LifecycleEffect) {
                self.effects.push(effect);
            }
            fn complete_prepared_future(&mut self, _: FutureId, _: FutureState) {}
        }
        let mut host = BatchHost {
            plans: preparations.iter(),
            states: self.futures.iter().map(|(id, f)| (*id, f.state)).collect(),
            effects: Vec::new(),
        };
        self.update_futures_with_host(&mut host)?;
        Ok(host.effects)
    }
    pub fn update_futures_with_host<H: ChunkLifecycleHost>(
        &mut self,
        host: &mut H,
    ) -> Result<(), String> {
        let old = FullStatus::from_level(self.old_level);
        let new = FullStatus::from_level(self.ticket_level);
        self.was_accessible |= new >= FullStatus::Full;
        for stage in [Stage::Full, Stage::Block, Stage::Entity] {
            let target = stage.status();
            let index = stage.index();
            if old < target && new >= target {
                if stage == Stage::Entity && self.slots[index] != UNLOADED_FUTURE {
                    return Err("entity-ticking future must be the shared unloaded sentinel".into());
                }
                let prepared = host.prepare(stage)?;
                let future = prepared.future;
                if let Some(existing) = self.futures.get(&future) {
                    if existing.state != prepared.initial_state {
                        return Err(
                            "provider future completion was not delivered before preparation"
                                .into(),
                        );
                    }
                } else {
                    self.register_future(future, prepared.initial_state)?;
                }
                host.emit(LifecycleEffect::Prepare { stage, future });
                self.slots[index] = future;
                self.cancel_confirmation();
                let confirmation = self.confirmations.len();
                self.confirmations.push(Confirmation {
                    state: ConfirmationState::Pending,
                    status: target,
                });
                self.current_confirmation = confirmation;
                let source = self.futures.get_mut(&future).unwrap();
                source.confirmations.push(confirmation);
                let mut effects = Vec::new();
                if source.state == FutureState::Success {
                    self.complete_confirmation(confirmation, &mut effects);
                }
                for effect in effects {
                    host.emit(effect);
                }
                self.add_save_dependency(future);
            }
            if old >= target && new < target {
                let future = self.slots[index];
                let mut effects = Vec::new();
                if self.complete_inner(future, FutureState::Unloaded, &mut effects)? {
                    host.complete_prepared_future(future, FutureState::Unloaded);
                }
                for effect in effects {
                    host.emit(effect);
                }
                self.slots[index] = UNLOADED_FUTURE;
            }
        }
        if new < old {
            self.cancel_confirmation();
            let mut effects = Vec::new();
            self.publish(new, &mut effects);
            for effect in effects {
                host.emit(effect);
            }
        }
        host.emit(LifecycleEffect::LevelChange {
            queue_level: self.queue_level,
            new_level: self.ticket_level,
        });
        self.level_notifications.push(self.ticket_level);
        if self.apply_queue_inline {
            self.queue_level = self.ticket_level;
        }
        self.old_level = self.ticket_level;
        Ok(())
    }
    fn run_task_inner(
        &mut self,
        task: TaskId,
        effects: &mut Vec<LifecycleEffect>,
    ) -> Result<(), String> {
        let entry = self
            .tasks
            .get_mut(&task)
            .ok_or_else(|| format!("unknown executor task {task}"))?;
        if entry.finished {
            return Ok(());
        }
        entry.finished = true;
        let status = entry.status;
        self.pending_tasks.retain(|id| *id != task);
        self.publish(status, effects);
        Ok(())
    }
    pub fn run_executor_task(&mut self, task: TaskId) -> Result<Vec<LifecycleEffect>, String> {
        let mut effects = Vec::new();
        self.run_task_inner(task, &mut effects)?;
        Ok(effects)
    }
    pub fn drain_executor(&mut self, budget: usize) -> (Vec<TaskId>, Vec<LifecycleEffect>) {
        let mut ran = Vec::new();
        let mut effects = Vec::new();
        for _ in 0..budget {
            let Some(id) = self.pending_tasks.front().copied() else {
                break;
            };
            ran.push(id);
            self.run_task_inner(id, &mut effects)
                .expect("pending task is registered");
        }
        (ran, effects)
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            old_level: self.old_level,
            ticket_level: self.ticket_level,
            queue_level: self.queue_level,
            was_accessible: self.was_accessible,
            confirmation: self.confirmations[self.current_confirmation].state,
            output_status: self.output_status,
            slots: self.slots,
            futures: self
                .futures
                .iter()
                .map(|(id, future)| (*id, future.state))
                .collect(),
            pending_tasks: self.pending_tasks.iter().copied().collect(),
            save_done: self.save_done(),
            save_exceptional: self.save_exceptional(),
            level_getter_values: vec![self.queue_level; self.level_notifications.len()],
        }
    }
}

#[cfg(test)]
#[path = "../tools/chunk_lifecycle_tests.rs"]
mod tests;
