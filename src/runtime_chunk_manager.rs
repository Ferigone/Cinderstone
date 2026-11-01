//! Composition driver for proved ticket, range, readiness and lifecycle cores.
//!
//! Native holder/priority ordering is an explicit policy, not Java identity
//! HashSet or complete aqe/mailbox parity. Loading/network/storage availability
//! never directly publishes entity visibility.
use crate::{
    chunk_lifecycle::{
        self as life, ChunkLifecycle, ChunkLifecycleHost, FutureState, LifecycleEffect,
        PreparedFuture, Stage,
    },
    chunk_loading_tickets::{ChunkLoadingTickets, LoadingEvent},
    chunk_readiness::{
        self as ready, ChunkOutcome, ChunkReadiness, ChunkStatus, HolderRef, ReadinessEffect,
        ReadinessError, ReadinessHost, ReadinessKind, SourceSpec,
    },
    entity_spatial::Visibility,
    entity_ticking_range::{Chunk, EntityTickingRange, TicketKind},
    runtime_world::RuntimeWorld,
};
use anyhow::{anyhow, bail, ensure, Result};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const UNLOADED_LEVEL_CHUNK: &str = "Unloaded level chunk";
pub type HolderId = u64;
pub type MainTaskId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HolderOrderScope {
    ObservedReference,
    NativeStablePolicy,
}
#[derive(Clone, Debug)]
pub struct HolderOrder {
    pub ids: Vec<HolderId>,
    pub scope: HolderOrderScope,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorityOrderScope {
    ObservedReference,
    NativeStablePriorityScope,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingHolder {
    pub holder: HolderRef,
    pub ticket_level: i32,
}
#[derive(Clone, Debug)]
pub enum GenerationAvailability {
    ResidentSources,
    NoAllowedStatus,
    Pending(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyWorldFact {
    ObservedEmpty,
    Unavailable,
}
/// Facts of the newly created native flat-fixture source representation. This
/// is not an assertion about an imported vanilla world or arbitrary prehistory.
pub struct FreshFixtureFacts {
    pub inherited_source_entities: EmptyWorldFact,
    pub global_upgrade_fixers: EmptyWorldFact,
}
pub trait RuntimeChunkActions {
    /// Execute the actual viewer loop/markChunkPendingToSend. Returning Ok is
    /// acknowledgement of that enqueue, not permission to send ChunkData now.
    fn mark_chunk_pending_to_send(
        &mut self,
        chunk: Chunk,
        world: &mut RuntimeWorld,
    ) -> Result<(), ReadinessError>;
    fn try_schedule_light_update(&mut self, world: &mut RuntimeWorld) -> Result<()>;
}
struct ResidentFixtureChunk {
    chunk: Chunk,
    saved_ticks_unpacked_at: Option<i64>,
    post_processed: bool,
}
struct SourceSlot {
    outcome: ChunkOutcome,
}
/// Persistent source identities/data for one level and one manager's holder
/// identity registry; the frame adapter below only borrows it. Do not reuse
/// this store with a replacement manager whose holder IDs start over.
/// All completed resident FULL slots are retained through allowed-status
/// demotion. Pending disabled slots complete UNLOADED and are reset, as aqn does.
pub struct RuntimeWorldSources {
    facts: FreshFixtureFacts,
    data: BTreeMap<u64, ResidentFixtureChunk>,
    by_chunk: BTreeMap<Chunk, u64>,
    next_data: u64,
    slots: BTreeMap<(HolderId, u8), u64>,
    sources: BTreeMap<u64, SourceSlot>,
    next_source: u64,
    pending: BTreeMap<Chunk, String>,
}
impl RuntimeWorldSources {
    pub fn fresh_flat_fixture(facts: FreshFixtureFacts) -> Self {
        Self {
            facts,
            data: BTreeMap::new(),
            by_chunk: BTreeMap::new(),
            next_data: 1,
            slots: BTreeMap::new(),
            sources: BTreeMap::new(),
            next_source: 2,
            pending: BTreeMap::new(),
        }
    }
    pub fn data_chunk(&self, id: u64) -> Option<Chunk> {
        self.data.get(&id).map(|data| data.chunk)
    }
    pub fn pending_reason(&self, chunk: Chunk) -> Option<&str> {
        self.pending.get(&chunk).map(String::as_str)
    }
    pub fn ticks_unpacked_at(&self, id: u64) -> Option<i64> {
        self.data.get(&id)?.saved_ticks_unpacked_at
    }
    fn validate_fixture(
        &self,
        chunk: Chunk,
        world: &RuntimeWorld,
    ) -> std::result::Result<(), String> {
        let RuntimeWorld::Fixture(world) = world else {
            return Err("Anvil FULL source post-load/conversion has not been proved; disk/cache presence is not a generation future".into());
        };
        if self.facts.inherited_source_entities != EmptyWorldFact::ObservedEmpty {
            return Err("inherited source-entity data is unavailable".into());
        }
        use std::sync::OnceLock;
        static FACTS: OnceLock<Vec<i32>> = OnceLock::new();
        let facts = FACTS.get_or_init(|| {
            #[derive(serde::Deserialize)]
            struct Facts {
                states: Vec<i32>,
            }
            serde_json::from_str::<Facts>(include_str!(
                "../tools/storage_blockentity_factories.json"
            ))
            .expect("proved exact1.21 block-entity state facts")
            .states
        });
        for state in world.base_states() {
            if facts.get(state as usize).copied() != Some(-1) {
                return Err(format!("flat fixture has unavailable live/pending block-entity data for state {state} in {chunk:?}"));
            }
        }
        for change in world.changes().into_iter().filter(|c|(c.x.div_euclid(16),c.z.div_euclid(16))==chunk){
            if facts.get(change.state as usize).copied()!=Some(-1){world.native_block_entities().validate_at((change.x,change.y,change.z),change.state).map_err(|e|format!("native block entity unavailable: {e:#}"))?;}
        }
        Ok(())
    }
    fn materialize_fixture(
        &mut self,
        chunk: Chunk,
        world: &RuntimeWorld,
    ) -> std::result::Result<u64, String> {
        self.validate_fixture(chunk, world)?;
        self.pending.remove(&chunk);
        if let Some(&id) = self.by_chunk.get(&chunk) {
            return Ok(id);
        }
        let id = self.next_data;
        self.next_data = self
            .next_data
            .checked_add(1)
            .ok_or("fixture data identity overflow")?;
        self.data.insert(
            id,
            ResidentFixtureChunk {
                chunk,
                saved_ticks_unpacked_at: None,
                post_processed: false,
            },
        );
        self.by_chunk.insert(chunk, id);
        Ok(id)
    }
}
pub struct RuntimeWorldReadinessBackend<'a, A: RuntimeChunkActions> {
    sources: &'a mut RuntimeWorldSources,
    actions: &'a mut A,
    game_time: i64,
}
impl<'a, A: RuntimeChunkActions> RuntimeWorldReadinessBackend<'a, A> {
    pub fn new(
        sources: &'a mut RuntimeWorldSources,
        actions: &'a mut A,
        actual_game_time: i64,
    ) -> Self {
        Self {
            sources,
            actions,
            game_time: actual_game_time,
        }
    }
    fn data(&self, id: u64) -> std::result::Result<&ResidentFixtureChunk, ReadinessError> {
        self.sources
            .data
            .get(&id)
            .ok_or_else(|| ReadinessError::unsupported("unknown resident fixture chunk identity"))
    }
}
impl<A: RuntimeChunkActions> ReadinessBackend for RuntimeWorldReadinessBackend<'_, A> {
    fn generation_pass(
        &mut self,
        holder: HolderRef,
        allowed: Option<ChunkStatus>,
        world: &mut RuntimeWorld,
    ) -> Result<GenerationPass> {
        let disabled: Vec<_> = self
            .sources
            .slots
            .iter()
            .filter(|((id, status), source)| {
                *id == holder.id
                    && !ready::can_request(allowed, ChunkStatus::from_index(*status).unwrap())
                    && self.sources.sources[source].outcome == ChunkOutcome::Pending
            })
            .map(|(key, id)| (*key, *id))
            .collect();
        let mut completions = Vec::new();
        for (key, id) in disabled {
            let outcome = ChunkOutcome::Failure("Unloaded chunk".into());
            self.sources.sources.get_mut(&id).unwrap().outcome = outcome.clone();
            self.sources.slots.remove(&key);
            completions.push((id, outcome));
        }
        let availability = if allowed.is_none() {
            GenerationAvailability::NoAllowedStatus
        } else {
            match self.sources.materialize_fixture(holder.chunk, world) {
                Ok(_) => GenerationAvailability::ResidentSources,
                Err(reason) => {
                    self.sources.pending.insert(holder.chunk, reason.clone());
                    GenerationAvailability::Pending(reason)
                }
            }
        };
        Ok(GenerationPass {
            availability,
            completions,
        })
    }
    fn get_or_schedule(
        &mut self,
        holder: HolderRef,
        allowed: Option<ChunkStatus>,
        status: ChunkStatus,
        world: &mut RuntimeWorld,
    ) -> std::result::Result<SourceSpec, ReadinessError> {
        if !ready::can_request(allowed, status) {
            return Ok(SourceSpec {
                identity: 1,
                initial: ChunkOutcome::Failure("Unloaded chunk".into()),
            });
        }
        if let Some(&id) = self.sources.slots.get(&(holder.id, status.index())) {
            return Ok(SourceSpec {
                identity: id,
                initial: self.sources.sources[&id].outcome.clone(),
            });
        }
        let outcome = match self.sources.materialize_fixture(holder.chunk, world) {
            Ok(id) => ChunkOutcome::Success(ready::ChunkData {
                id,
                kind: ready::ChunkKind::Full,
            }),
            Err(reason) => {
                self.sources.pending.insert(holder.chunk, reason);
                ChunkOutcome::Pending
            }
        };
        let id = self.sources.next_source;
        self.sources.next_source = self
            .sources
            .next_source
            .checked_add(1)
            .ok_or_else(|| ReadinessError::unsupported("source identity overflow"))?;
        self.sources.slots.insert((holder.id, status.index()), id);
        self.sources.sources.insert(
            id,
            SourceSlot {
                outcome: outcome.clone(),
            },
        );
        Ok(SourceSpec {
            identity: id,
            initial: outcome,
        })
    }
    fn post_process_generation(
        &mut self,
        id: u64,
        world: &mut RuntimeWorld,
    ) -> std::result::Result<(), ReadinessError> {
        let chunk = self.data(id)?.chunk;
        self.sources
            .validate_fixture(chunk, world)
            .map_err(ReadinessError::unsupported)?;
        if self.sources.facts.global_upgrade_fixers != EmptyWorldFact::ObservedEmpty {
            return Err(ReadinessError::unsupported(
                "global UpgradeData fixer queues are unavailable",
            ));
        }
        // Fresh FlatWorld carries no PostProcessing, pending BE NBT or upgrade
        // data. All source states above were checked against actual BE facts.
        self.sources.data.get_mut(&id).unwrap().post_processed = true;
        Ok(())
    }
    fn start_ticking_chunk(
        &mut self,
        id: u64,
        world: &mut RuntimeWorld,
    ) -> std::result::Result<(), ReadinessError> {
        let chunk = self.data(id)?.chunk;
        self.sources
            .validate_fixture(chunk, world)
            .map_err(ReadinessError::unsupported)?;
        // This source representation has empty SavedTick containers. Unpack
        // consumes that pending state once; nonempty Anvil tick data is not
        // represented here and never silently becomes an empty list.
        self.sources
            .data
            .get_mut(&id)
            .unwrap()
            .saved_ticks_unpacked_at
            .get_or_insert(self.game_time);
        Ok(())
    }
    fn send_chunk(
        &mut self,
        id: u64,
        world: &mut RuntimeWorld,
    ) -> std::result::Result<(), ReadinessError> {
        let chunk = self.data(id)?.chunk;
        self.actions.mark_chunk_pending_to_send(chunk, world)
    }
    fn try_schedule_light_update(&mut self, world: &mut RuntimeWorld) -> Result<()> {
        self.actions.try_schedule_light_update(world)
    }
    fn certify_current_query_context(
        &mut self,
        holder: HolderRef,
        world: &mut RuntimeWorld,
    ) -> Result<()> {
        self.sources
            .validate_fixture(holder.chunk, world)
            .map_err(|error| anyhow!(error))?;
        ensure!(
            self.sources.facts.global_upgrade_fixers == EmptyWorldFact::ObservedEmpty,
            "global upgrade-fixer history is unknown"
        );
        ensure!(
            self.sources.by_chunk.contains_key(&holder.chunk),
            "no certified resident full source data"
        );
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct GenerationPass {
    pub availability: GenerationAvailability,
    pub completions: Vec<(u64, ChunkOutcome)>,
}

/// The backend implements actual source futures and world actions. A missing
/// generation/post-load/action implementation is explicit, never success from
/// an NBT Status field or from a network chunk packet.
pub trait ReadinessBackend {
    fn generation_pass(
        &mut self,
        holder: HolderRef,
        allowed: Option<ChunkStatus>,
        world: &mut RuntimeWorld,
    ) -> Result<GenerationPass>;
    fn get_or_schedule(
        &mut self,
        holder: HolderRef,
        allowed: Option<ChunkStatus>,
        status: ChunkStatus,
        world: &mut RuntimeWorld,
    ) -> Result<SourceSpec, ReadinessError>;
    fn post_process_generation(
        &mut self,
        data: ready::ChunkDataId,
        world: &mut RuntimeWorld,
    ) -> Result<(), ReadinessError>;
    fn start_ticking_chunk(
        &mut self,
        data: ready::ChunkDataId,
        world: &mut RuntimeWorld,
    ) -> Result<(), ReadinessError>;
    fn send_chunk(
        &mut self,
        data: ready::ChunkDataId,
        world: &mut RuntimeWorld,
    ) -> Result<(), ReadinessError>;
    fn try_schedule_light_update(&mut self, world: &mut RuntimeWorld) -> Result<()>;
    fn certify_current_query_context(
        &mut self,
        holder: HolderRef,
        world: &mut RuntimeWorld,
    ) -> Result<()>;
}
pub trait ChunkManagerHost {
    /// Invoke RuntimeEntities.set_chunk_visibility synchronously here. Do not
    /// discard stale completed-promotion callbacks or infer a ticket threshold.
    fn visibility_changed(
        &mut self,
        holder: HolderRef,
        status: life::FullStatus,
        visibility: Visibility,
    ) -> Result<()>;
}

#[derive(Clone, Debug)]
pub enum ManagerEvent {
    Loading(LoadingEvent),
    Lifecycle {
        holder: HolderId,
        effect: LifecycleEffect,
    },
    Readiness(ReadinessEffect),
    HolderOrder(HolderOrder),
    GenerationPending {
        holder: HolderRef,
        reason: String,
    },
    ReleaseRegistered {
        observer: u64,
        holder: HolderId,
        local_future: u64,
        readiness_future: Option<u64>,
    },
    PriorityDispatch {
        scope: PriorityOrderScope,
        holder: HolderRef,
        queue_level: i32,
        tasks: Vec<u64>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DistancePass {
    pub loading_steps: i32,
    pub holders_updated: usize,
    pub releases_registered: usize,
}
impl DistancePass {
    pub fn did_work(self) -> bool {
        self.loading_steps != 0 || self.holders_updated != 0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollOutcome {
    DistanceWork(DistancePass),
    MainRan(MainTaskId),
    Idle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorityPoll {
    LevelChanged(HolderId),
    BatchDispatched(HolderId),
    Idle,
}

struct Holder {
    reference: HolderRef,
    ticket_level: i32,
    allowed: Option<ChunkStatus>,
    core: Option<ChunkLifecycle>,
    next_local: u64,
    backend_to_local: BTreeMap<u64, u64>,
    local_to_backend: BTreeMap<u64, u64>,
    level_notifications: usize,
}
#[derive(Clone, Copy)]
enum MainWork {
    Tickets(u64),
    Lifecycle { holder: HolderId, task: u64 },
    Readiness(u64),
}
#[derive(Clone, Copy)]
enum Observer {
    Lifecycle { holder: HolderId, local: u64 },
    Release { chunk: Chunk, future: Option<u64> },
}
struct PriorityLane {
    first: u64,
    tasks: VecDeque<u64>,
}

pub struct RuntimeChunkManager {
    loading: ChunkLoadingTickets,
    range: EntityTickingRange,
    readiness: ChunkReadiness,
    holders: BTreeMap<HolderId, Holder>,
    by_chunk: BTreeMap<Chunk, HolderId>,
    dirty: BTreeSet<HolderId>,
    next_holder: u64,
    players: BTreeMap<i32, (Chunk, bool)>,
    main: VecDeque<(MainTaskId, MainWork)>,
    next_main: u64,
    observers: BTreeMap<u64, Observer>,
    next_observer: u64,
    priority: BTreeMap<HolderId, PriorityLane>,
    next_priority_order: u64,
    priority_in_flight: BTreeSet<u64>,
    level_changes: VecDeque<(HolderId, usize)>,
    events: Vec<ManagerEvent>,
    poison: Option<String>,
}
impl RuntimeChunkManager {
    pub fn new(view_distance: i32, simulation_distance: i32) -> Self {
        let mut loading = ChunkLoadingTickets::new(simulation_distance);
        loading.set_view_distance(view_distance.clamp(2, 32));
        Self {
            loading,
            range: EntityTickingRange::new(simulation_distance),
            readiness: ChunkReadiness::new(),
            holders: BTreeMap::new(),
            by_chunk: BTreeMap::new(),
            dirty: BTreeSet::new(),
            next_holder: 1,
            players: BTreeMap::new(),
            main: VecDeque::new(),
            next_main: 1,
            observers: BTreeMap::new(),
            next_observer: 1,
            priority: BTreeMap::new(),
            next_priority_order: 1,
            priority_in_flight: BTreeSet::new(),
            level_changes: VecDeque::new(),
            events: Vec::new(),
            poison: None,
        }
    }
    fn healthy(&self) -> Result<()> {
        if let Some(reason) = &self.poison {
            bail!("chunk manager is stopped after a callback error: {reason}");
        }
        Ok(())
    }
    pub fn ticking_range(&self) -> &EntityTickingRange {
        &self.range
    }
    pub fn loading(&self) -> &ChunkLoadingTickets {
        &self.loading
    }
    pub fn drain_events(&mut self) -> Vec<ManagerEvent> {
        std::mem::take(&mut self.events)
    }
    pub fn player_add(&mut self, id: i32, chunk: Chunk, ignored: bool) -> Result<()> {
        self.healthy()?;
        ensure!(
            !self.players.contains_key(&id),
            "player source already registered"
        );
        self.players.insert(id, (chunk, ignored));
        if !ignored {
            self.loading.add_player(chunk, id);
        }
        self.capture_loading();
        Ok(())
    }
    pub fn player_move(&mut self, id: i32, old: Chunk, new: Chunk) -> Result<()> {
        self.healthy()?;
        let entry = self
            .players
            .get_mut(&id)
            .ok_or_else(|| anyhow!("unknown player source"))?;
        ensure!(entry.0 == old, "stale player source position");
        if old != new {
            if !entry.1 {
                self.loading.remove_player(old, id);
                self.loading.add_player(new, id);
            }
            entry.0 = new;
        }
        self.capture_loading();
        Ok(())
    }
    pub fn player_remove(&mut self, id: i32, old: Chunk) -> Result<()> {
        self.healthy()?;
        let &(chunk, ignored) = self
            .players
            .get(&id)
            .ok_or_else(|| anyhow!("unknown player source"))?;
        ensure!(chunk == old, "stale player source removal");
        self.players.remove(&id);
        if !ignored {
            self.loading.remove_player(old, id);
        }
        self.capture_loading();
        Ok(())
    }
    pub fn player_ignored(&mut self, id: i32, ignored: bool) -> Result<()> {
        self.healthy()?;
        let entry = self
            .players
            .get_mut(&id)
            .ok_or_else(|| anyhow!("unknown player source"))?;
        if entry.1 != ignored {
            if ignored {
                self.loading.remove_player(entry.0, id);
            } else {
                self.loading.add_player(entry.0, id);
            }
            entry.1 = ignored;
        }
        self.capture_loading();
        Ok(())
    }
    pub fn set_view_distance(&mut self, distance: i32) -> Result<()> {
        self.healthy()?;
        self.loading.set_view_distance(distance.clamp(2, 32));
        self.capture_loading();
        Ok(())
    }
    pub fn set_simulation_distance(&mut self, distance: i32) -> Result<()> {
        self.healthy()?;
        self.loading.update_simulation_distance(distance);
        self.capture_loading();
        Ok(())
    }
    pub fn update_chunk_forced(&mut self, chunk: Chunk, forced: bool) -> Result<()> {
        self.healthy()?;
        self.loading.update_chunk_forced(chunk, forced);
        self.capture_loading();
        Ok(())
    }
    pub fn add_region_ticket(
        &mut self,
        kind: TicketKind,
        chunk: Chunk,
        distance: i32,
    ) -> Result<()> {
        self.healthy()?;
        self.loading.add_region_ticket(kind, chunk, distance);
        self.capture_loading();
        Ok(())
    }
    pub fn remove_region_ticket(
        &mut self,
        kind: TicketKind,
        chunk: Chunk,
        distance: i32,
    ) -> Result<()> {
        self.healthy()?;
        self.loading.remove_region_ticket(kind, chunk, distance);
        self.capture_loading();
        Ok(())
    }
    pub fn purge_stale_tickets(&mut self) -> Result<()> {
        self.healthy()?;
        self.loading.purge_stale_tickets();
        self.range.purge_stale_tickets();
        self.capture_loading();
        Ok(())
    }
    fn enqueue(&mut self, work: MainWork) {
        let id = self.next_main;
        self.next_main = self
            .next_main
            .checked_add(1)
            .expect("main task identity overflow");
        self.main.push_back((id, work));
    }
    fn capture_loading(&mut self) {
        for transfer in self.loading.take_ticking_transfers() {
            transfer.apply(&mut self.range);
        }
        for event in self.loading.take_events() {
            if let LoadingEvent::Enqueue {
                executor: "main",
                task,
            } = event
            {
                self.enqueue(MainWork::Tickets(task));
            }
            self.events.push(ManagerEvent::Loading(event));
        }
    }
    pub fn poll_sorter_one(&mut self) -> Result<bool> {
        self.healthy()?;
        let ran = self.loading.run_sorter_tasks(1) != 0;
        self.capture_loading();
        Ok(ran)
    }
    pub fn pending_main_tasks(&self) -> usize {
        self.main.len()
    }
    pub fn pending_sorter_tasks(&self) -> usize {
        self.loading.pending_sorter_tasks()
    }
    /// Runnable/graph work only. Pending external source futures do not create
    /// busy-loop work. Recheck this after poll_main_one: a distance pass can
    /// enqueue sorter work while the original main poll still returns Idle.
    pub fn has_scheduler_work(&self) -> bool {
        self.loading.pending_sorter_tasks() != 0
            || self.loading.has_distance_work()
            || self.range.has_work()
            || !self.main.is_empty()
            || !self.dirty.is_empty()
            || !self.priority.is_empty()
            || !self.level_changes.is_empty()
    }
    pub fn pending_holder_updates(&self) -> Vec<PendingHolder> {
        self.dirty
            .iter()
            .map(|id| {
                let h = &self.holders[id];
                PendingHolder {
                    holder: h.reference,
                    ticket_level: h.ticket_level,
                }
            })
            .collect()
    }
    pub fn holder(&self, chunk: Chunk) -> Option<HolderRef> {
        self.by_chunk
            .get(&chunk)
            .map(|id| self.holders[id].reference)
    }
    pub fn holder_snapshot(&self, id: HolderId) -> Option<life::Snapshot> {
        self.holders
            .get(&id)?
            .core
            .as_ref()
            .map(ChunkLifecycle::snapshot)
    }
    pub fn current_full_ready(&self, chunk: Chunk) -> bool {
        self.holder(chunk)
            .and_then(|holder| self.holders.get(&holder.id)?.core.as_ref())
            .is_some_and(|core| {
                core.future_state(core.stage_future(Stage::Full)) == Some(FutureState::Success)
            })
    }
    /// Original ChunkHolder.getChunkToSend predicate over the retained native
    /// holder registry: sendSync.isDone first, then CURRENT block future
    /// getNow(UNLOADED).orElse(null). The broader visibleChunkMap publication
    /// snapshot/unload phase is not yet represented by this registry.
    pub fn current_chunk_to_send_ready(&self, chunk: Chunk) -> Result<bool> {
        self.healthy()?;
        let Some(reference) = self.holder(chunk) else {
            return Ok(false);
        };
        if !self.readiness.send_sync_done(reference) {
            return Ok(false);
        }
        self.current_block_ticking_chunk_ready(chunk)
    }
    /// Current getTickingChunk, without getChunkToSend's sendSync prerequisite.
    pub fn current_block_ticking_chunk_ready(&self, chunk: Chunk) -> Result<bool> {
        self.healthy()?;
        let Some(reference)=self.holder(chunk) else {return Ok(false);};
        let holder = &self.holders[&reference.id];
        let core = holder
            .core
            .as_ref()
            .ok_or_else(|| anyhow!("reentrant chunk-to-send lookup"))?;
        let local = core.stage_future(Stage::Block);
        if local == life::UNLOADED_FUTURE {
            return Ok(false);
        }
        let future = *holder
            .local_to_backend
            .get(&local)
            .ok_or_else(|| anyhow!("block future has no readiness identity"))?;
        match self.readiness.outcome(future) {
            Some(ChunkOutcome::Success(data)) => {
                ensure!(
                    data.kind == ready::ChunkKind::Full,
                    "java.lang.ClassCastException: ticking future is not LevelChunk"
                );
                Ok(true)
            }
            Some(ChunkOutcome::Exceptional(error)) => Err(readiness_error(error)),
            Some(ChunkOutcome::Cancelled) => bail!("java.util.concurrent.CancellationException"),
            None | Some(ChunkOutcome::Pending | ChunkOutcome::Failure(_)) => Ok(false),
        }
    }
    /// Actual native holder insertion order, filtered before source shuffle by
    /// getTickingChunk. Java visibleChunkMap publication/unload order remains a
    /// separate capability; this does not claim to reconstruct that snapshot.
    pub fn native_resident_ticking_chunk_order(&self) -> Result<Vec<Chunk>> {
        self.healthy()?;
        let mut chunks=Vec::new();
        for holder in self.holders.values() {
            if self.by_chunk.get(&holder.reference.chunk)==Some(&holder.reference.id)
                && self.current_block_ticking_chunk_ready(holder.reference.chunk)? {
                chunks.push(holder.reference.chunk);
            }
        }
        Ok(chunks)
    }
    pub fn certify_full_region(&self, min: Chunk, max: Chunk) -> Result<()> {
        self.healthy()?;
        ensure!(
            min.0 <= max.0 && min.1 <= max.1,
            "inverted full-chunk region"
        );
        for x in min.0..=max.0 {
            for z in min.1..=max.1 {
                ensure!(
                    self.current_full_ready((x, z)),
                    "current FULL future is unavailable at {x},{z}"
                );
            }
        }
        Ok(())
    }
    /// Stronger supported-slice guard: current FULL future plus the backend's
    /// actual source/world-action facts. A visibility callback alone is not it.
    pub fn certify_query_region(
        &self,
        world: &mut RuntimeWorld,
        backend: &mut impl ReadinessBackend,
        min: Chunk,
        max: Chunk,
    ) -> Result<()> {
        self.certify_full_region(min, max)?;
        for x in min.0..=max.0 {
            for z in min.1..=max.1 {
                backend.certify_current_query_context(
                    self.holder((x, z)).expect("FULL guard found holder"),
                    world,
                )?;
            }
        }
        Ok(())
    }
    fn apply_scheduling(&mut self) -> Result<()> {
        for change in self.loading.take_scheduling_changes() {
            let id = if let Some(&id) = self.by_chunk.get(&change.chunk) {
                id
            } else {
                if change.level > life::MAX_LEVEL {
                    continue;
                }
                let id = self.next_holder;
                self.next_holder = self
                    .next_holder
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("holder identity overflow"))?;
                let mut core = ChunkLifecycle::new(change.level);
                core.set_apply_queue_inline(false);
                self.holders.insert(
                    id,
                    Holder {
                        reference: HolderRef {
                            id,
                            chunk: change.chunk,
                        },
                        ticket_level: change.level,
                        allowed: None,
                        core: Some(core),
                        next_local: 1,
                        backend_to_local: BTreeMap::new(),
                        local_to_backend: BTreeMap::new(),
                        level_notifications: 0,
                    },
                );
                self.by_chunk.insert(change.chunk, id);
                id
            };
            let holder = self.holders.get_mut(&id).unwrap();
            holder.ticket_level = change.level;
            holder
                .core
                .as_mut()
                .ok_or_else(|| anyhow!("reentrant holder scheduling"))?
                .set_ticket_level(change.level);
            self.dirty.insert(id);
        }
        Ok(())
    }
    fn next_observer(&mut self, kind: Observer) -> Result<u64> {
        let id = self.next_observer;
        self.next_observer = self
            .next_observer
            .checked_add(1)
            .ok_or_else(|| anyhow!("observer identity overflow"))?;
        self.observers.insert(id, kind);
        Ok(id)
    }
    fn outcome_state(outcome: &ChunkOutcome) -> FutureState {
        match outcome {
            ChunkOutcome::Pending => FutureState::Pending,
            ChunkOutcome::Success(_) => FutureState::Success,
            ChunkOutcome::Failure(reason) if reason == UNLOADED_LEVEL_CHUNK => {
                FutureState::Unloaded
            }
            ChunkOutcome::Failure(_) => FutureState::Failure,
            ChunkOutcome::Exceptional(_) => FutureState::Exceptional,
            ChunkOutcome::Cancelled => FutureState::Cancelled,
        }
    }
    fn emit_lifecycle(
        &mut self,
        id: HolderId,
        effect: LifecycleEffect,
        host: &mut impl ChunkManagerHost,
    ) -> Result<()> {
        match effect {
            LifecycleEffect::Enqueue { task } => {
                self.enqueue(MainWork::Lifecycle { holder: id, task })
            }
            LifecycleEffect::FullStatusChange { status, visibility } => {
                let visibility = match visibility {
                    life::Visibility::Hidden => Visibility::Hidden,
                    life::Visibility::Tracked => Visibility::Tracked,
                    life::Visibility::Ticking => Visibility::Ticking,
                };
                if let Err(error) =
                    host.visibility_changed(self.holders[&id].reference, status, visibility)
                {
                    self.poison = Some(error.to_string());
                    return Err(error);
                }
            }
            LifecycleEffect::LevelChange { .. } => {
                let holder = self.holders.get_mut(&id).unwrap();
                let notification = holder.level_notifications;
                holder.level_notifications += 1;
                self.level_changes.push_back((id, notification));
            }
            _ => {}
        }
        self.events
            .push(ManagerEvent::Lifecycle { holder: id, effect });
        Ok(())
    }
    fn accept_readiness(
        &mut self,
        effects: Vec<ReadinessEffect>,
        host: &mut impl ChunkManagerHost,
        demotion: Option<(HolderId, u64)>,
    ) -> Result<()> {
        for effect in effects {
            self.events.push(ManagerEvent::Readiness(effect.clone()));
            match &effect {
                ReadinessEffect::QueueTask {
                    task,
                    queue: ready::Queue::Main,
                } => self.enqueue(MainWork::Readiness(*task)),
                ReadinessEffect::QueueTask {
                    task,
                    queue: ready::Queue::Priority { center },
                } => {
                    let first = self.next_priority_order;
                    self.next_priority_order += 1;
                    self.priority
                        .entry(center.id)
                        .or_insert_with(|| PriorityLane {
                            first,
                            tasks: VecDeque::new(),
                        })
                        .tasks
                        .push_back(*task);
                }
                ReadinessEffect::PriorityAcknowledged { task } => {
                    self.priority_in_flight.remove(task);
                }
                ReadinessEffect::FutureCompleted {
                    observer,
                    future,
                    outcome,
                    ..
                } => {
                    let observer = self
                        .observers
                        .remove(observer)
                        .ok_or_else(|| anyhow!("unknown readiness observer"))?;
                    match observer {
                        Observer::Release {
                            chunk,
                            future: captured,
                        } => {
                            ensure!(
                                captured == Some(*future),
                                "release callback changed its captured future identity"
                            );
                            if matches!(
                                outcome,
                                ChunkOutcome::Success(_) | ChunkOutcome::Failure(_)
                            ) {
                                self.loading.normal_entity_future_completion(chunk);
                                self.capture_loading();
                            }
                        }
                        Observer::Lifecycle { holder, local } => {
                            if demotion == Some((holder, local)) {
                                continue;
                            }
                            let core=self.holders.get_mut(&holder).and_then(|h|h.core.as_mut()).ok_or_else(||anyhow!("reentrant lifecycle future completion requires an explicit driver boundary"))?;
                            let (_, effects) = core
                                .complete_future(local, Self::outcome_state(outcome))
                                .map_err(|e| anyhow!(e))?;
                            for effect in effects {
                                self.emit_lifecycle(holder, effect, host)?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn distance_futures_pass<B: ReadinessBackend, H: ChunkManagerHost>(
        &mut self,
        world: &mut RuntimeWorld,
        backend: &mut B,
        host: &mut H,
        holder_order: &mut impl FnMut(&[PendingHolder]) -> Result<HolderOrder>,
    ) -> Result<DistancePass> {
        self.healthy()?;
        self.capture_loading();
        self.range.run_all_updates();
        self.loading.run_player_distance_updates();
        let remaining = self.loading.run_loading_updates(i32::MAX);
        self.capture_loading();
        self.apply_scheduling()?;
        let steps = i32::MAX - remaining;
        let pending = self.pending_holder_updates();
        if !pending.is_empty() {
            let order = holder_order(&pending)?;
            let wanted: BTreeSet<_> = pending.iter().map(|h| h.holder.id).collect();
            ensure!(
                order.ids.len() == wanted.len()
                    && order.ids.iter().copied().collect::<BTreeSet<_>>() == wanted,
                "holder order must be an exact permutation"
            );
            if order.scope == HolderOrderScope::NativeStablePolicy {
                ensure!(
                    order.ids.windows(2).all(|pair| pair[0] < pair[1]),
                    "native stable holder policy must preserve stable identity order"
                );
            }
            self.events.push(ManagerEvent::HolderOrder(order.clone()));
            for id in &order.ids {
                let holder = self.holders.get_mut(id).unwrap();
                holder.allowed = ready::allowed_generation_status(holder.ticket_level);
                let reference = holder.reference;
                let allowed = holder.allowed;
                let pass = backend
                    .generation_pass(reference, allowed, world)
                    .inspect_err(|error| {
                        self.poison = Some(error.to_string());
                    })?;
                if let GenerationAvailability::Pending(reason) = pass.availability {
                    self.events.push(ManagerEvent::GenerationPending {
                        holder: reference,
                        reason,
                    });
                }
                for (identity, outcome) in pass.completions {
                    self.complete_source(identity, outcome, world, backend, host)?;
                }
            }
            for id in &order.ids {
                let mut core = self
                    .holders
                    .get_mut(id)
                    .unwrap()
                    .core
                    .take()
                    .ok_or_else(|| anyhow!("reentrant holder update"))?;
                let mut env = LifecycleEnvironment {
                    manager: self,
                    backend,
                    world,
                    host,
                    holder: *id,
                    error: None,
                };
                let result = core.update_futures_with_host(&mut env);
                let error = env.error.take();
                env.manager.holders.get_mut(id).unwrap().core = Some(core);
                if let Some(error) = error {
                    self.poison = Some(error.to_string());
                    return Err(error);
                }
                if let Err(error) = result {
                    self.poison = Some(error.clone());
                    bail!(error);
                }
            }
            self.dirty.clear();
            return Ok(DistancePass {
                loading_steps: steps,
                holders_updated: order.ids.len(),
                releases_registered: 0,
            });
        }
        let releases = self.loading.take_release_registrations();
        let count = releases.len();
        for chunk in releases {
            let holder = self
                .holder(chunk)
                .ok_or_else(|| anyhow!("release registration has no current holder"))?;
            let snapshot = self
                .holder_snapshot(holder.id)
                .ok_or_else(|| anyhow!("release registration during holder callback"))?;
            let local = snapshot.slots[2];
            let future = self.holders[&holder.id]
                .local_to_backend
                .get(&local)
                .copied();
            let observer = self.next_observer(Observer::Release { chunk, future })?;
            self.events.push(ManagerEvent::ReleaseRegistered {
                observer,
                holder: holder.id,
                local_future: local,
                readiness_future: future,
            });
            if let Some(future) = future {
                let Self {
                    readiness,
                    holders,
                    by_chunk,
                    ..
                } = self;
                let mut context = WorldHost {
                    holders,
                    by_chunk,
                    backend,
                    world,
                };
                let effects = readiness
                    .observe(future, observer, &mut context)
                    .map_err(readiness_error)?;
                self.accept_readiness(effects, host, None)?;
            } else {
                ensure!(
                    local == life::UNLOADED_FUTURE,
                    "missing exact readiness future identity"
                );
                self.observers.remove(&observer);
                self.loading.normal_entity_future_completion(chunk);
                self.capture_loading();
            }
        }
        Ok(DistancePass {
            loading_steps: steps,
            holders_updated: 0,
            releases_registered: count,
        })
    }
    /// Deliver completion after the backend has committed the same terminal
    /// outcome to its authoritative source storage. RuntimeWorldSources has no
    /// asynchronous source loader yet; it must not be completed through this
    /// method while retaining a conflicting Pending SourceSlot.
    pub fn complete_source<B: ReadinessBackend, H: ChunkManagerHost>(
        &mut self,
        identity: u64,
        outcome: ChunkOutcome,
        world: &mut RuntimeWorld,
        backend: &mut B,
        host: &mut H,
    ) -> Result<bool> {
        self.healthy()?;
        let Self {
            readiness,
            holders,
            by_chunk,
            ..
        } = self;
        let mut context = WorldHost {
            holders,
            by_chunk,
            backend,
            world,
        };
        let (changed, effects) = readiness
            .complete_source(identity, outcome, &mut context)
            .map_err(readiness_error)?;
        self.accept_readiness(effects, host, None)?;
        Ok(changed)
    }
    pub fn poll_main_one<B: ReadinessBackend, H: ChunkManagerHost>(
        &mut self,
        world: &mut RuntimeWorld,
        backend: &mut B,
        host: &mut H,
        holder_order: &mut impl FnMut(&[PendingHolder]) -> Result<HolderOrder>,
    ) -> Result<PollOutcome> {
        let pass = self.distance_futures_pass(world, backend, host, holder_order)?;
        if pass.did_work() {
            return Ok(PollOutcome::DistanceWork(pass));
        }
        backend.try_schedule_light_update(world)?;
        let Some((id, work)) = self.main.pop_front() else {
            return Ok(PollOutcome::Idle);
        };
        match work {
            MainWork::Tickets(expected) => {
                ensure!(
                    self.loading.run_main_tasks(1) == 1,
                    "missing queued ticket task"
                );
                let events = self.loading.take_events();
                ensure!(events.iter().any(|event|matches!(event,LoadingEvent::Run{executor:"main",task}if *task==expected)),"global ticket task identity mismatch");
                for event in events {
                    if let LoadingEvent::Enqueue {
                        executor: "main",
                        task,
                    } = event
                    {
                        self.enqueue(MainWork::Tickets(task));
                    }
                    self.events.push(ManagerEvent::Loading(event));
                }
                self.capture_loading();
            }
            MainWork::Lifecycle { holder, task } => {
                let effects = self
                    .holders
                    .get_mut(&holder)
                    .and_then(|h| h.core.as_mut())
                    .ok_or_else(|| anyhow!("unknown lifecycle task holder"))?
                    .run_executor_task(task)
                    .map_err(|e| anyhow!(e))?;
                for effect in effects {
                    self.emit_lifecycle(holder, effect, host)?;
                }
            }
            MainWork::Readiness(task) => {
                let Self {
                    readiness,
                    holders,
                    by_chunk,
                    ..
                } = self;
                let mut context = WorldHost {
                    holders,
                    by_chunk,
                    backend,
                    world,
                };
                let effects = readiness
                    .run_task(task, &mut context)
                    .map_err(readiness_error)?;
                self.accept_readiness(effects, host, None)?;
            }
        }
        Ok(PollOutcome::MainRan(id))
    }
    pub fn pending_priority_tasks(&self) -> Vec<(HolderRef, i32, Vec<u64>)> {
        self.priority
            .iter()
            .map(|(id, lane)| {
                let holder = &self.holders[id];
                let level = holder.core.as_ref().map_or(45, |c| c.queue_level());
                (
                    holder.reference,
                    level,
                    lane.tasks.iter().copied().collect(),
                )
            })
            .collect()
    }
    pub fn dispatch_priority_batch(
        &mut self,
        holder: HolderId,
        scope: PriorityOrderScope,
    ) -> Result<()> {
        self.healthy()?;
        ensure!(
            self.priority_in_flight.is_empty(),
            "priority batch acknowledgement is still pending"
        );
        ensure!(
            self.level_changes.is_empty(),
            "priority level changes must be serviced before dispatch"
        );
        let pending = self.pending_priority_tasks();
        let minimum = pending
            .iter()
            .map(|(_, level, _)| *level)
            .min()
            .ok_or_else(|| anyhow!("no pending priority tasks"))?;
        let level = pending
            .iter()
            .find(|(candidate, _, _)| candidate.id == holder)
            .ok_or_else(|| anyhow!("unknown priority holder"))?
            .1;
        ensure!(
            level == minimum,
            "priority dispatch must preserve minimum current holder queue level"
        );
        if scope == PriorityOrderScope::NativeStablePriorityScope {
            let chosen = self
                .priority
                .iter()
                .filter(|(id, _)| {
                    self.holders[id]
                        .core
                        .as_ref()
                        .is_some_and(|c| c.queue_level() == minimum)
                })
                .min_by_key(|(_, lane)| lane.first)
                .map(|(id, _)| *id)
                .unwrap();
            ensure!(
                chosen == holder,
                "native priority tie policy must preserve chunk insertion order"
            );
        }
        let lane = self.priority.remove(&holder).unwrap();
        let tasks: Vec<_> = lane.tasks.into_iter().collect();
        for task in &tasks {
            self.priority_in_flight.insert(*task);
            self.enqueue(MainWork::Readiness(*task));
        }
        self.events.push(ManagerEvent::PriorityDispatch {
            scope,
            holder: self.holders[&holder].reference,
            queue_level: level,
            tasks,
        });
        Ok(())
    }
    pub fn poll_priority_one(&mut self, scope: PriorityOrderScope) -> Result<PriorityPoll> {
        self.healthy()?;
        if let Some((holder, notification)) = self.level_changes.pop_front() {
            self.holders
                .get_mut(&holder)
                .and_then(|h| h.core.as_mut())
                .ok_or_else(|| anyhow!("unknown queue-level holder"))?
                .apply_level_change(notification)
                .map_err(|e| anyhow!(e))?;
            return Ok(PriorityPoll::LevelChanged(holder));
        }
        if !self.priority_in_flight.is_empty() || self.priority.is_empty() {
            return Ok(PriorityPoll::Idle);
        }
        ensure!(
            scope == PriorityOrderScope::NativeStablePriorityScope,
            "observed priority dispatch requires explicit holder input"
        );
        let holder = self
            .priority
            .iter()
            .min_by_key(|(id, lane)| {
                (
                    self.holders[id]
                        .core
                        .as_ref()
                        .map_or(45, |c| c.queue_level()),
                    lane.first,
                )
            })
            .map(|(id, _)| *id)
            .unwrap();
        self.dispatch_priority_batch(holder, scope)?;
        Ok(PriorityPoll::BatchDispatched(holder))
    }
}

fn readiness_error(error: ReadinessError) -> anyhow::Error {
    anyhow!("{}: {}", error.class, error.message)
}

#[cfg(test)]
#[path = "../tools/runtime_chunk_manager_tests.rs"]
mod tests;
struct WorldHost<'a, B> {
    holders: &'a BTreeMap<HolderId, Holder>,
    by_chunk: &'a BTreeMap<Chunk, HolderId>,
    backend: &'a mut B,
    world: &'a mut RuntimeWorld,
}
impl<B: ReadinessBackend> ReadinessHost for WorldHost<'_, B> {
    fn lookup_holder(&mut self, chunk: Chunk) -> Option<HolderRef> {
        self.by_chunk
            .get(&chunk)
            .map(|id| self.holders[id].reference)
    }
    fn get_or_schedule(
        &mut self,
        holder: HolderRef,
        status: ChunkStatus,
    ) -> Result<SourceSpec, ReadinessError> {
        let entry = self
            .holders
            .get(&holder.id)
            .filter(|entry| entry.reference == holder)
            .ok_or_else(|| ReadinessError::unsupported("stale generation holder identity"))?;
        self.backend
            .get_or_schedule(holder, entry.allowed, status, self.world)
    }
    fn post_process_generation(&mut self, data: u64) -> Result<(), ReadinessError> {
        self.backend.post_process_generation(data, self.world)
    }
    fn start_ticking_chunk(&mut self, data: u64) -> Result<(), ReadinessError> {
        self.backend.start_ticking_chunk(data, self.world)
    }
    fn send_chunk(&mut self, data: u64) -> Result<(), ReadinessError> {
        self.backend.send_chunk(data, self.world)
    }
}
struct LifecycleEnvironment<'a, B, H> {
    manager: &'a mut RuntimeChunkManager,
    backend: &'a mut B,
    world: &'a mut RuntimeWorld,
    host: &'a mut H,
    holder: HolderId,
    error: Option<anyhow::Error>,
}
impl<B: ReadinessBackend, H: ChunkManagerHost> ChunkLifecycleHost
    for LifecycleEnvironment<'_, B, H>
{
    fn prepare(&mut self, stage: Stage) -> Result<PreparedFuture, String> {
        if let Some(error) = &self.error {
            return Err(error.to_string());
        }
        let result = (|| -> Result<PreparedFuture> {
            let kind = match stage {
                Stage::Full => ReadinessKind::Accessible,
                Stage::Block => ReadinessKind::BlockTicking,
                Stage::Entity => ReadinessKind::EntityTicking,
            };
            let center = self.manager.holders[&self.holder].reference;
            let RuntimeChunkManager {
                readiness,
                holders,
                by_chunk,
                ..
            } = self.manager;
            let mut context = WorldHost {
                holders,
                by_chunk,
                backend: self.backend,
                world: self.world,
            };
            let (future, effects) = readiness
                .prepare(kind, center, &mut context)
                .map_err(readiness_error)?;
            self.manager.accept_readiness(effects, self.host, None)?;
            let state = self
                .manager
                .readiness
                .outcome(future)
                .as_ref()
                .map_or(FutureState::Pending, RuntimeChunkManager::outcome_state);
            let holder = self.manager.holders.get_mut(&self.holder).unwrap();
            let local = if let Some(local) = holder.backend_to_local.get(&future) {
                *local
            } else {
                let id = holder.next_local;
                holder.next_local = holder
                    .next_local
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("local future identity overflow"))?;
                holder.backend_to_local.insert(future, id);
                holder.local_to_backend.insert(id, future);
                id
            };
            if state == FutureState::Pending {
                let observer = self.manager.next_observer(Observer::Lifecycle {
                    holder: self.holder,
                    local,
                })?;
                let RuntimeChunkManager {
                    readiness,
                    holders,
                    by_chunk,
                    ..
                } = self.manager;
                let mut context = WorldHost {
                    holders,
                    by_chunk,
                    backend: self.backend,
                    world: self.world,
                };
                let effects = readiness
                    .observe(future, observer, &mut context)
                    .map_err(readiness_error)?;
                ensure!(
                    effects.is_empty(),
                    "prepared pending future completed during observer registration"
                );
            }
            Ok(PreparedFuture {
                future: local,
                initial_state: state,
            })
        })();
        result.map_err(|error| {
            let text = error.to_string();
            self.error = Some(error);
            text
        })
    }
    fn emit(&mut self, effect: LifecycleEffect) {
        if self.error.is_none() {
            if let Err(error) = self.manager.emit_lifecycle(self.holder, effect, self.host) {
                self.error = Some(error);
            }
        }
    }
    fn complete_prepared_future(&mut self, local: u64, state: FutureState) {
        if self.error.is_some() {
            return;
        }
        let result = (|| -> Result<()> {
            ensure!(
                state == FutureState::Unloaded,
                "unexpected lifecycle provider completion"
            );
            let future = *self.manager.holders[&self.holder]
                .local_to_backend
                .get(&local)
                .ok_or_else(|| anyhow!("missing demoted provider future identity"))?;
            let RuntimeChunkManager {
                readiness,
                holders,
                by_chunk,
                ..
            } = self.manager;
            let mut context = WorldHost {
                holders,
                by_chunk,
                backend: self.backend,
                world: self.world,
            };
            let (_, effects) = readiness
                .complete_future(
                    future,
                    ChunkOutcome::Failure(UNLOADED_LEVEL_CHUNK.into()),
                    &mut context,
                )
                .map_err(readiness_error)?;
            self.manager
                .accept_readiness(effects, self.host, Some((self.holder, local)))
        })();
        if let Err(error) = result {
            self.error = Some(error);
        }
    }
}
