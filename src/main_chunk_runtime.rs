//! Main-owned composition of confirmed chunk readiness and native fixture facts.
//! Native lighting now owns the actual stable engine, retained edit work and
//! completed network output; threaded intermediate publication is not claimed.
use super::*;
use mc121_rust::{
    chunk_readiness::{ChunkOutcome, HolderRef, ReadinessEffect, ReadinessError},
    runtime_chunk_manager::{
        ChunkManagerHost, EmptyWorldFact, FreshFixtureFacts, HolderOrder, HolderOrderScope,
        ManagerEvent, PollOutcome, PriorityOrderScope, PriorityPoll, RuntimeChunkActions,
        RuntimeChunkManager, RuntimeWorldReadinessBackend, RuntimeWorldSources,
    },
};
use std::collections::VecDeque;

const MAX_POLL_TURNS: usize = 16_384;
const MAX_DIAGNOSTIC_MESSAGES: usize = 4;
const MAX_DIAGNOSTIC_CHARS: usize = 320;

/// The host owns the stable native engine and an ordered light-publication
/// outbox. A source/mutation certificate becomes current only after observing
/// the actual world and finishing all retained edit work.
struct NativeFixtureLighting {
    engine: Option<mc121_rust::runtime_lighting::NativeRuntimeLighting>,
    pending: VecDeque<NativeLightingWork>,
    completed: VecDeque<WorldUpdate>,
}
#[derive(Clone, Copy, Debug)]
enum NativeLightingWork {
    ReconcileActualNativeSource,
}
impl NativeFixtureLighting {
    fn new(fresh_native_source: bool) -> Self {
        Self {
            // Configuration admission verifies the native fixture's actual
            // overworld dimension; imported sources never acquire this owner.
            engine: fresh_native_source
                .then(|| mc121_rust::runtime_lighting::NativeRuntimeLighting::new(true)),
            pending: VecDeque::new(),
            completed: VecDeque::new(),
        }
    }
    fn invalidate(&mut self) {
        if self.pending.is_empty() {
            self.pending
                .push_back(NativeLightingWork::ReconcileActualNativeSource);
        }
    }
    fn invalidate_block(&mut self, position: (i32, i32, i32)) -> Result<()> {
        self.invalidate();
        self.engine
            .as_mut()
            .context("native lighting source owner unavailable")?
            .invalidate(position)
    }
    fn require_supported(&self) -> Result<()> {
        ensure!(
            self.pending.is_empty(),
            "native lighting source requires reconciliation"
        );
        self.engine
            .as_ref()
            .context("native lighting source owner unavailable")?
            .require_settled()
    }
    fn try_schedule_update(&mut self, world: &WorldBackend) -> Result<()> {
        let WorldBackend::Fixture(flat) = world else {
            bail!("imported-world lighting source owner is unavailable");
        };
        // Retain the invalidation on every error. Already completed output
        // remains queued until Main publishes it; scheduler callbacks cannot
        // discard network work merely because the stable computation finished.
        self.invalidate();
        let engine = self
            .engine
            .as_mut()
            .context("native lighting source owner unavailable")?;
        let updates = engine.process_pending(flat)?;
        engine.require_current(flat)?;
        self.completed.extend(
            updates
                .into_iter()
                .filter(|update| update.changed)
                .map(|update| WorldUpdate::Light {
                    chunk: update.chunk,
                    payload: update.payload.into(),
                }),
        );
        self.pending.clear();
        Ok(())
    }
    fn engine_mut(&mut self) -> Result<&mut mc121_rust::runtime_lighting::NativeRuntimeLighting> {
        self.require_supported()?;
        self.engine
            .as_mut()
            .context("native lighting source owner unavailable")
    }
    fn drain_completed(&mut self) -> Vec<WorldUpdate> {
        self.completed.drain(..).collect()
    }
}

#[derive(Debug, Default)]
pub(super) struct ChunkRuntimePoll {
    pub turns: usize,
    pub main_tasks: usize,
    pub sorter_tasks: usize,
    pub priority_batches: usize,
    pub distance_passes: usize,
    pub holders_updated: usize,
    pub visibility_updates: usize,
    pub pending_marks: usize,
    pub generation_pending: usize,
    pub readiness_failures: usize,
    pub diagnostics: Vec<String>,
    pub quiescent: bool,
}
impl ChunkRuntimePoll {
    fn diagnostic(&mut self, message: String) {
        if self.diagnostics.len() < MAX_DIAGNOSTIC_MESSAGES {
            let message: String = message.chars().take(MAX_DIAGNOSTIC_CHARS).collect();
            if !self.diagnostics.contains(&message) {
                self.diagnostics.push(message);
            }
        }
    }
    fn collect(&mut self, manager: &mut RuntimeChunkManager) {
        for event in manager.drain_events() {
            match event {
                ManagerEvent::GenerationPending { holder, reason } => {
                    self.generation_pending += 1;
                    self.diagnostic(format!("source {:?}: {reason}", holder.chunk));
                }
                ManagerEvent::Readiness(ReadinessEffect::FutureCompleted {
                    outcome: ChunkOutcome::Exceptional(error),
                    ..
                }) => {
                    self.readiness_failures += 1;
                    self.diagnostic(format!("{}: {}", error.class, error.message));
                }
                _ => {}
            }
        }
    }
}

pub(super) struct MainChunkRuntime {
    pub manager: RuntimeChunkManager,
    sources: RuntimeWorldSources,
    lighting: NativeFixtureLighting,
}
/// An actual current-FULL/readiness proof for one locked movement operation.
/// Pending ticket input does not replace or invalidate the observed futures.
#[derive(Clone, Copy)]
pub(super) struct CurrentChunkQueryRegion {
    min: (i32, i32),
    max: (i32, i32),
}
impl CurrentChunkQueryRegion {
    pub(super) fn contains(self, chunk: (i32, i32)) -> bool {
        chunk.0 >= self.min.0
            && chunk.0 <= self.max.0
            && chunk.1 >= self.min.1
            && chunk.1 <= self.max.1
    }
}
impl MainChunkRuntime {
    pub fn new(view_distance: i32, simulation_distance: i32, fresh_unedited: bool) -> Self {
        let fact = if fresh_unedited {
            EmptyWorldFact::ObservedEmpty
        } else {
            EmptyWorldFact::Unavailable
        };
        Self {
            manager: RuntimeChunkManager::new(view_distance, simulation_distance),
            sources: RuntimeWorldSources::fresh_flat_fixture(FreshFixtureFacts {
                inherited_source_entities: fact,
                global_upgrade_fixers: fact,
            }),
            lighting: NativeFixtureLighting::new(fresh_unedited),
        }
    }
    pub fn invalidate_lighting(&mut self) {
        self.lighting.invalidate();
    }
    pub fn require_supported_lighting(&self) -> Result<()> {
        self.lighting.require_supported()
    }
    pub fn has_native_lighting_owner(&self) -> bool {
        self.lighting.engine.is_some()
    }
    pub fn certify_current_native_full(
        &self,
        world: &WorldBackend,
        min: (i32, i32),
        max: (i32, i32),
    ) -> Result<()> {
        self.require_supported_lighting()?;
        let WorldBackend::Fixture(flat) = world else {
            bail!("current native proof cannot admit imported sources");
        };
        self.lighting
            .engine
            .as_ref()
            .context("native light source owner unavailable")?
            .require_current(flat)?;
        self.manager.certify_full_region(min, max)
    }
    pub fn invalidate_block_lighting(&mut self, position: (i32, i32, i32)) -> Result<()> {
        self.lighting.invalidate_block(position)
    }
    /// Finish actual work and return the complete retained light outbox. Main
    /// publishes it after its ordered block changes while retaining world lock.
    pub fn settle_lighting(&mut self, world: &WorldBackend) -> Result<Vec<WorldUpdate>> {
        self.lighting.try_schedule_update(world)?;
        Ok(self.lighting.drain_completed())
    }
    pub fn take_lighting_updates(&mut self) -> Vec<WorldUpdate> {
        self.lighting.drain_completed()
    }
    pub fn native_lighting_mut(
        &mut self,
    ) -> Result<&mut mc121_rust::runtime_lighting::NativeRuntimeLighting> {
        self.lighting.engine_mut()
    }
    /// Split the two actual owners for item rain queries without borrowing an
    /// alias to the complete MainChunkRuntime while mutating its light cache.
    pub fn native_lighting_parts(
        &mut self,
    ) -> Result<(
        &mut mc121_rust::runtime_lighting::NativeRuntimeLighting,
        &RuntimeChunkManager,
    )> {
        Ok((self.lighting.engine_mut()?, &self.manager))
    }
    pub fn poll(
        &mut self,
        world: &mut WorldBackend,
        entities: &mut RuntimeEntities,
        transports: &mut BTreeMap<i32, SessionChunkTransport>,
        events: &broadcast::Sender<PlayerEvent>,
        game_time: i64,
    ) -> Result<ChunkRuntimePoll> {
        self.lighting.try_schedule_update(world)?;
        let mut summary = ChunkRuntimePoll::default();
        let mut visibility = VisibilityHost {
            entities,
            updates: 0,
        };
        let mut actions = MainChunkActions {
            transports,
            events,
            lighting: &mut self.lighting,
            pending_marks: 0,
        };
        let mut backend =
            RuntimeWorldReadinessBackend::new(&mut self.sources, &mut actions, game_time);
        for _ in 0..MAX_POLL_TURNS {
            summary.turns += 1;
            let sorter = self.manager.poll_sorter_one()?;
            summary.sorter_tasks += usize::from(sorter);
            let priority = self
                .manager
                .poll_priority_one(PriorityOrderScope::NativeStablePriorityScope)?;
            if matches!(priority, PriorityPoll::BatchDispatched(_)) {
                summary.priority_batches += 1;
            }
            // This call performs the original distance/futures pass itself.
            // Do not insert an extra pass that changes release-registration timing.
            let main = self.manager.poll_main_one(
                world,
                &mut backend,
                &mut visibility,
                &mut |pending| {
                    Ok(HolderOrder {
                        ids: pending.iter().map(|entry| entry.holder.id).collect(),
                        scope: HolderOrderScope::NativeStablePolicy,
                    })
                },
            )?;
            match main {
                PollOutcome::MainRan(_) => summary.main_tasks += 1,
                PollOutcome::DistanceWork(pass) => {
                    summary.distance_passes += 1;
                    summary.holders_updated += pass.holders_updated;
                }
                PollOutcome::Idle => {}
            }
            summary.collect(&mut self.manager);
            // A distance pass may enqueue sorter work even when its original
            // main-poll result is Idle. Recheck after all three executor polls.
            if !sorter
                && priority == PriorityPoll::Idle
                && main == PollOutcome::Idle
                && !self.manager.has_scheduler_work()
            {
                summary.quiescent = true;
                break;
            }
        }
        summary.visibility_updates = visibility.updates;
        summary.pending_marks = actions.pending_marks;
        Ok(summary)
    }
    pub fn certify_query_region(
        &mut self,
        world: &mut WorldBackend,
        transports: &mut BTreeMap<i32, SessionChunkTransport>,
        events: &broadcast::Sender<PlayerEvent>,
        game_time: i64,
        min: (i32, i32),
        max: (i32, i32),
    ) -> Result<()> {
        self.require_supported_lighting()?;
        ensure!(
            !self.manager.has_scheduler_work(),
            "chunk runtime has not reached scheduler quiescence"
        );
        self.certify_current_query_region(world, transports, events, game_time, min, max)
            .map(|_| ())
    }
    pub(super) fn certify_current_query_region(
        &mut self,
        world: &mut WorldBackend,
        transports: &mut BTreeMap<i32, SessionChunkTransport>,
        events: &broadcast::Sender<PlayerEvent>,
        game_time: i64,
        min: (i32, i32),
        max: (i32, i32),
    ) -> Result<CurrentChunkQueryRegion> {
        self.certify_current_native_full(world, min, max)?;
        let mut actions = MainChunkActions {
            transports,
            events,
            lighting: &mut self.lighting,
            pending_marks: 0,
        };
        let mut backend =
            RuntimeWorldReadinessBackend::new(&mut self.sources, &mut actions, game_time);
        self.manager
            .certify_query_region(world, &mut backend, min, max)?;
        Ok(CurrentChunkQueryRegion { min, max })
    }
}

struct VisibilityHost<'a> {
    entities: &'a mut RuntimeEntities,
    updates: usize,
}
impl ChunkManagerHost for VisibilityHost<'_> {
    fn visibility_changed(
        &mut self,
        holder: HolderRef,
        _: FullStatus,
        visibility: Visibility,
    ) -> Result<()> {
        self.entities
            .set_chunk_visibility(holder.chunk, visibility)?;
        self.updates += 1;
        Ok(())
    }
}
struct MainChunkActions<'a> {
    transports: &'a mut BTreeMap<i32, SessionChunkTransport>,
    events: &'a broadcast::Sender<PlayerEvent>,
    lighting: &'a mut NativeFixtureLighting,
    pending_marks: usize,
}
impl RuntimeChunkActions for MainChunkActions<'_> {
    fn mark_chunk_pending_to_send(
        &mut self,
        chunk: (i32, i32),
        _: &mut WorldBackend,
    ) -> Result<(), ReadinessError> {
        for (&id, transport) in self.transports.iter_mut() {
            if transport.mark_pending_if_wanted(chunk) {
                transport.publish_packets(vec![]);
                // The private outbox is authoritative and survives lost wakes.
                let _ = self.events.send(PlayerEvent::ChunksReady {
                    recipient: id,
                    cutoff: transport.latest_revision(),
                    reconcile: None,
                });
                self.pending_marks += 1;
            }
        }
        Ok(())
    }
    fn try_schedule_light_update(&mut self, world: &mut WorldBackend) -> Result<()> {
        self.lighting.try_schedule_update(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> WorldBackend {
        WorldBackend::Fixture(FlatWorld::new(0, 1, 10, 9, 39))
    }
    #[test]
    fn lighting_requires_observed_native_source_and_finishes_retained_work() {
        let world = fixture();
        let mut unknown = NativeFixtureLighting::new(false);
        assert!(unknown.try_schedule_update(&world).is_err());
        let mut observed = NativeFixtureLighting::new(true);
        observed.try_schedule_update(&world).unwrap();
        observed.invalidate();
        observed.invalidate();
        assert_eq!(observed.pending.len(), 1);
        assert!(observed.require_supported().is_err());
        observed.try_schedule_update(&world).unwrap();
        assert!(observed.pending.is_empty());
        observed.require_supported().unwrap();
    }

    #[test]
    fn real_send_callback_marks_current_view_and_retains_private_pending_outbox() {
        let mut near = SessionChunkTransport::new();
        let mut far = SessionChunkTransport::new();
        near.queue_view(
            entity_tracking::ChunkView {
                center: (0, 0),
                view_distance: 2,
            },
            false,
            true,
        )
        .unwrap();
        far.queue_view(
            entity_tracking::ChunkView {
                center: (100, 100),
                view_distance: 2,
            },
            false,
            true,
        )
        .unwrap();
        near.drain();
        far.drain();
        let mut transports = BTreeMap::from([(7, near), (8, far)]);
        let (events, mut receive) = broadcast::channel(1);
        let mut lighting = NativeFixtureLighting::new(true);
        let mut actions = MainChunkActions {
            transports: &mut transports,
            events: &events,
            lighting: &mut lighting,
            pending_marks: 0,
        };
        actions
            .mark_chunk_pending_to_send((0, 0), &mut fixture())
            .unwrap();
        assert_eq!(actions.pending_marks, 1);
        assert!(matches!(
            receive.try_recv().unwrap(),
            PlayerEvent::ChunksReady { recipient: 7, .. }
        ));
        let update = transports.get_mut(&7).unwrap().drain();
        assert_eq!(update.len(), 1);
        assert!(update[0].packets.is_empty());
        assert!(update[0].pending.contains(&(0, 0)));
        assert!(transports.get_mut(&8).unwrap().drain().is_empty());
    }

    #[test]
    fn confirmed_fixture_poll_settles_before_certification_and_idle_does_not_republish() {
        let mut world = fixture();
        let mut runtime = MainChunkRuntime::new(2, 2, true);
        let mut entities = RuntimeEntities::new(LegacyRandomSource::new(12345));
        let item = ItemState::new(
            41,
            [1; 16],
            Position {
                x: 8.5,
                y: 65.,
                z: 8.5,
                yaw: 0.,
                pitch: 0.,
                on_ground: false,
            },
            Vec3::ZERO,
            ItemStack::new(1, 1).unwrap(),
        )
        .unwrap();
        entities
            .insert_item(item, LegacyRandomSource::new(98765))
            .unwrap();
        assert!(entities.snapshots().unwrap().1.is_empty());
        let mut near = SessionChunkTransport::new();
        let mut far = SessionChunkTransport::new();
        near.queue_view(
            entity_tracking::ChunkView {
                center: (0, 0),
                view_distance: 2,
            },
            false,
            true,
        )
        .unwrap();
        far.queue_view(
            entity_tracking::ChunkView {
                center: (100, 100),
                view_distance: 2,
            },
            false,
            true,
        )
        .unwrap();
        near.drain();
        far.drain();
        let mut transports = BTreeMap::from([(7, near), (8, far)]);
        let (events, mut receiver) = broadcast::channel(64);
        runtime.manager.update_chunk_forced((0, 0), true).unwrap();
        let summary = runtime
            .poll(&mut world, &mut entities, &mut transports, &events, 123)
            .unwrap();
        assert!(summary.quiescent);
        assert!(!runtime.manager.has_scheduler_work());
        assert!(summary.visibility_updates > 0);
        assert_eq!(summary.pending_marks, 9);
        assert_eq!(entities.snapshots().unwrap().1.len(), 1);
        assert_eq!(transports.get_mut(&7).unwrap().drain().len(), 9);
        assert!(transports.get_mut(&8).unwrap().drain().is_empty());
        for _ in 0..9 {
            assert!(matches!(
                receiver.try_recv().unwrap(),
                PlayerEvent::ChunksReady { recipient: 7, .. }
            ));
        }
        assert!(receiver.try_recv().is_err());
        runtime
            .certify_query_region(&mut world, &mut transports, &events, 123, (-2, -2), (2, 2))
            .unwrap();
        let idle = runtime
            .poll(&mut world, &mut entities, &mut transports, &events, 124)
            .unwrap();
        assert!(idle.quiescent);
        assert_eq!(
            (idle.main_tasks, idle.pending_marks, idle.visibility_updates),
            (0, 0, 0)
        );
        assert!(transports.get_mut(&7).unwrap().drain().is_empty());
        runtime.invalidate_lighting();
        assert!(runtime.require_supported_lighting().is_err());
        assert!(runtime
            .certify_query_region(&mut world, &mut transports, &events, 125, (-2, -2), (2, 2))
            .is_err());
        runtime
            .poll(&mut world, &mut entities, &mut transports, &events, 125)
            .unwrap();
        runtime.require_supported_lighting().unwrap();
    }
}
