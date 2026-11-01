use super::*;
use crate::world::FlatWorld;

struct Backend {
    ready: bool,
    next: u64,
    identities: BTreeMap<(u64, u8), u64>,
    actions: Vec<(&'static str, u64)>,
    reject_send: bool,
}
impl Backend {
    fn new(ready: bool) -> Self {
        Self {
            ready,
            next: 2,
            identities: BTreeMap::new(),
            actions: Vec::new(),
            reject_send: false,
        }
    }
}
impl ReadinessBackend for Backend {
    fn generation_pass(
        &mut self,
        _: HolderRef,
        _: Option<ChunkStatus>,
        _: &mut RuntimeWorld,
    ) -> Result<GenerationPass> {
        Ok(GenerationPass {
            availability: if self.ready {
                GenerationAvailability::ResidentSources
            } else {
                GenerationAvailability::Pending("controlled pending generation sources".into())
            },
            completions: Vec::new(),
        })
    }
    fn get_or_schedule(
        &mut self,
        holder: HolderRef,
        allowed: Option<ChunkStatus>,
        status: ChunkStatus,
        _: &mut RuntimeWorld,
    ) -> Result<SourceSpec, ReadinessError> {
        if !ready::can_request(allowed, status) {
            return Ok(SourceSpec {
                identity: 1,
                initial: ChunkOutcome::Failure("Unloaded chunk".into()),
            });
        }
        let identity = *self
            .identities
            .entry((holder.id, status.index()))
            .or_insert_with(|| {
                let id = self.next;
                self.next += 1;
                id
            });
        Ok(SourceSpec {
            identity,
            initial: if self.ready {
                ChunkOutcome::Success(ready::ChunkData {
                    id: holder.id,
                    kind: ready::ChunkKind::Full,
                })
            } else {
                ChunkOutcome::Pending
            },
        })
    }
    fn post_process_generation(
        &mut self,
        data: u64,
        _: &mut RuntimeWorld,
    ) -> Result<(), ReadinessError> {
        self.actions.push(("post", data));
        Ok(())
    }
    fn start_ticking_chunk(
        &mut self,
        data: u64,
        _: &mut RuntimeWorld,
    ) -> Result<(), ReadinessError> {
        self.actions.push(("ticks", data));
        Ok(())
    }
    fn send_chunk(&mut self, data: u64, _: &mut RuntimeWorld) -> Result<(), ReadinessError> {
        self.actions.push(("send", data));
        if self.reject_send {
            Err(ReadinessError::unsupported(
                "actual sender action unavailable",
            ))
        } else {
            Ok(())
        }
    }
    fn try_schedule_light_update(&mut self, _: &mut RuntimeWorld) -> Result<()> {
        Ok(())
    }
    fn certify_current_query_context(&mut self, _: HolderRef, _: &mut RuntimeWorld) -> Result<()> {
        ensure!(self.ready, "controlled source is not ready");
        Ok(())
    }
}
#[derive(Default)]
struct Host {
    statuses: Vec<(Chunk, life::FullStatus, Visibility)>,
}
impl ChunkManagerHost for Host {
    fn visibility_changed(
        &mut self,
        holder: HolderRef,
        status: life::FullStatus,
        visibility: Visibility,
    ) -> Result<()> {
        self.statuses.push((holder.chunk, status, visibility));
        Ok(())
    }
}
fn world() -> RuntimeWorld {
    RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39))
}
fn order(pending: &[PendingHolder]) -> Result<HolderOrder> {
    Ok(HolderOrder {
        ids: pending.iter().map(|h| h.holder.id).collect(),
        scope: HolderOrderScope::NativeStablePolicy,
    })
}
fn settle(
    manager: &mut RuntimeChunkManager,
    backend: &mut Backend,
    host: &mut Host,
    world: &mut RuntimeWorld,
) {
    for _ in 0..20000 {
        let sorter = manager.poll_sorter_one().unwrap();
        let priority = manager
            .poll_priority_one(PriorityOrderScope::NativeStablePriorityScope)
            .unwrap();
        let main = manager
            .poll_main_one(world, backend, host, &mut order)
            .unwrap();
        if !sorter
            && priority == PriorityPoll::Idle
            && main == PollOutcome::Idle
            && !manager.has_scheduler_work()
        {
            return;
        }
    }
    panic!("controlled scheduler did not settle")
}

#[test]
fn only_confirmed_callbacks_publish_and_global_poll_rechecks_distances() {
    let mut manager = RuntimeChunkManager::new(2, 2);
    let mut backend = Backend::new(true);
    let mut host = Host::default();
    let mut world = world();
    manager.update_chunk_forced((0, 0), true).unwrap();
    assert!(matches!(
        manager
            .poll_main_one(&mut world, &mut backend, &mut host, &mut order)
            .unwrap(),
        PollOutcome::DistanceWork(_)
    ));
    assert!(manager.ticking_range().in_entity_ticking_range((0, 0)));
    assert!(manager.loading().loading_level((0, 0)) <= 31);
    assert!(host.statuses.is_empty());
    assert!(!manager.current_full_ready((0, 0)));
    assert!(!manager.current_chunk_to_send_ready((0, 0)).unwrap());
    // The main entity-readiness job runs independently of priority FULL/BLOCK
    // preparation. Its already-queued promotion must survive a later demotion.
    assert!(matches!(
        manager
            .poll_main_one(&mut world, &mut backend, &mut host, &mut order)
            .unwrap(),
        PollOutcome::MainRan(_)
    ));
    manager.update_chunk_forced((0, 0), false).unwrap();
    assert!(matches!(
        manager
            .poll_main_one(&mut world, &mut backend, &mut host, &mut order)
            .unwrap(),
        PollOutcome::DistanceWork(_)
    ));
    assert!(host
        .statuses
        .iter()
        .any(|(chunk, status, _)| *chunk == (0, 0) && *status == life::FullStatus::Inaccessible));
    assert!(matches!(
        manager
            .poll_main_one(&mut world, &mut backend, &mut host, &mut order)
            .unwrap(),
        PollOutcome::MainRan(_)
    ));
    assert_eq!(
        host.statuses.last(),
        Some(&((0, 0), life::FullStatus::EntityTicking, Visibility::Ticking))
    );
    assert!(!manager.ticking_range().in_entity_ticking_range((0, 0)));
    assert!(!manager.current_full_ready((0, 0)));
    assert!(manager.certify_full_region((0, 0), (0, 0)).is_err());
    assert!(!manager.current_chunk_to_send_ready((0, 0)).unwrap());
}

#[test]
fn native_priority_preserves_current_minimum_and_batch_acknowledgement() {
    let mut manager = RuntimeChunkManager::new(2, 2);
    let mut backend = Backend::new(true);
    let mut host = Host::default();
    let mut world = world();
    manager.update_chunk_forced((0, 0), true).unwrap();
    manager
        .distance_futures_pass(&mut world, &mut backend, &mut host, &mut order)
        .unwrap();
    while let Some((holder, notification)) = manager.level_changes.pop_front() {
        manager
            .holders
            .get_mut(&holder)
            .unwrap()
            .core
            .as_mut()
            .unwrap()
            .apply_level_change(notification)
            .unwrap();
    }
    let pending = manager.pending_priority_tasks();
    let minimum = pending.iter().map(|(_, level, _)| *level).min().unwrap();
    let wrong = pending
        .iter()
        .find(|(_, level, _)| *level > minimum)
        .unwrap()
        .0
        .id;
    assert!(manager
        .dispatch_priority_batch(wrong, PriorityOrderScope::ObservedReference)
        .is_err());
    let center = manager.holder((0, 0)).unwrap();
    assert_eq!(
        manager
            .poll_priority_one(PriorityOrderScope::NativeStablePriorityScope)
            .unwrap(),
        PriorityPoll::BatchDispatched(center.id)
    );
    assert_eq!(manager.priority_in_flight.len(), 2);
    assert_eq!(
        manager
            .poll_priority_one(PriorityOrderScope::NativeStablePriorityScope)
            .unwrap(),
        PriorityPoll::Idle
    );
    settle(&mut manager, &mut backend, &mut host, &mut world);
    manager.certify_full_region((-2, -2), (2, 2)).unwrap();
    assert!(manager.current_chunk_to_send_ready((0, 0)).unwrap());
    for pair in backend.actions.chunks(3) {
        assert_eq!(pair.len(), 3);
        assert_eq!([pair[0].0, pair[1].0, pair[2].0], ["post", "ticks", "send"]);
        assert!(pair.iter().all(|(_, id)| *id == pair[0].1));
    }
}

#[test]
fn exact_future_release_observers_are_not_deduplicated_by_chunk() {
    let mut manager = RuntimeChunkManager::new(2, 2);
    let mut backend = Backend::new(false);
    let mut host = Host::default();
    let mut world = world();
    manager.player_add(17, (0, 0), false).unwrap();
    settle(&mut manager, &mut backend, &mut host, &mut world);
    let first: Vec<_> = manager
        .events
        .iter()
        .filter_map(|e| match e {
            ManagerEvent::ReleaseRegistered {
                observer,
                holder,
                readiness_future: Some(future),
                ..
            } if manager.holders[holder].reference.chunk == (0, 0) => Some((*observer, *future)),
            _ => None,
        })
        .collect();
    assert_eq!(first.len(), 1);
    manager.update_chunk_forced((0, 0), true).unwrap();
    settle(&mut manager, &mut backend, &mut host, &mut world);
    manager.player_remove(17, (0, 0)).unwrap();
    settle(&mut manager, &mut backend, &mut host, &mut world);
    manager.player_add(17, (0, 0), false).unwrap();
    settle(&mut manager, &mut backend, &mut host, &mut world);
    let registrations: Vec<_> = manager
        .events
        .iter()
        .filter_map(|e| match e {
            ManagerEvent::ReleaseRegistered {
                observer,
                holder,
                readiness_future: Some(future),
                ..
            } if manager.holders[holder].reference.chunk == (0, 0) => Some((*observer, *future)),
            _ => None,
        })
        .collect();
    assert_eq!(registrations.len(), 2);
    assert_eq!(registrations[0].1, registrations[1].1);
    assert_ne!(registrations[0].0, registrations[1].0);
    let future = registrations[0].1;
    manager.drain_events();
    let effects = {
        let RuntimeChunkManager {
            readiness,
            holders,
            by_chunk,
            ..
        } = &mut manager;
        let mut context = WorldHost {
            holders,
            by_chunk,
            backend: &mut backend,
            world: &mut world,
        };
        readiness
            .complete_future(
                future,
                ChunkOutcome::Failure("controlled normal failure".into()),
                &mut context,
            )
            .unwrap()
            .1
    };
    manager.accept_readiness(effects, &mut host, None).unwrap();
    let notifications: Vec<_> = manager
        .events
        .iter()
        .filter_map(|event| match event {
            ManagerEvent::Readiness(ReadinessEffect::FutureCompleted { observer, .. })
                if registrations.iter().any(|(id, _)| id == observer) =>
            {
                Some(*observer)
            }
            _ => None,
        })
        .collect();
    assert_eq!(notifications, [registrations[1].0, registrations[0].0]);
    assert_eq!(
        manager
            .events
            .iter()
            .filter(|event| matches!(
                event,
                ManagerEvent::Loading(LoadingEvent::Enqueue {
                    executor: "main",
                    ..
                })
            ))
            .count(),
        2
    );
}

#[test]
fn holder_permutation_is_explicit_and_unsupported_actions_do_not_become_success() {
    let mut manager = RuntimeChunkManager::new(2, 2);
    let mut backend = Backend::new(true);
    backend.reject_send = true;
    let mut host = Host::default();
    let mut world = world();
    manager.update_chunk_forced((0, 0), true).unwrap();
    let mut bad = |pending: &[PendingHolder]| {
        Ok(HolderOrder {
            ids: pending.iter().skip(1).map(|h| h.holder.id).collect(),
            scope: HolderOrderScope::ObservedReference,
        })
    };
    assert!(manager
        .distance_futures_pass(&mut world, &mut backend, &mut host, &mut bad)
        .is_err());
    assert!(host.statuses.is_empty());
    assert!(!manager.pending_holder_updates().is_empty());
    settle(&mut manager, &mut backend, &mut host, &mut world);
    let center = manager.holder((0, 0)).unwrap();
    let core = manager.holders[&center.id].core.as_ref().unwrap();
    assert_eq!(
        core.future_state(core.stage_future(Stage::Block)),
        Some(FutureState::Exceptional)
    );
    assert_eq!(
        core.future_state(core.stage_future(Stage::Entity)),
        Some(FutureState::Success)
    );
    assert!(manager.current_full_ready((0, 0)));
    assert!(manager.current_chunk_to_send_ready((0, 0)).is_err());
    assert!(manager.events.iter().any(|event| matches!(
        event,
        ManagerEvent::Readiness(ReadinessEffect::FutureCompleted {
            outcome: ChunkOutcome::Exceptional(_),
            ..
        })
    )));
}

#[test]
fn real_fixture_sources_confirm_then_tick_an_item_against_original_full_states() {
    use crate::{
        entity::Position,
        entity_move::ProfileEvent,
        entity_state::EntityPhysicalState,
        geometry::Vec3,
        inventory::ItemStack,
        item_entity::{ItemState, Pickup},
        random::LegacyRandomSource,
        runtime_collision::CollisionBorder,
        runtime_entities::{EffectDelivery, PickupPlayer, RuntimeEntities, RuntimeEntityHost},
        runtime_entity_index::EntityHandle,
        runtime_item_world::{ItemWorldFacts, RainAvailability, RuntimeItemEffect},
    };
    struct ItemHost;
    impl RuntimeEntityHost for ItemHost {
        fn level_player_tick(&mut self, _: EntityHandle) -> Result<()> {
            bail!("no controlled player entity")
        }
        fn profile(&mut self, _: ProfileEvent) {}
        fn effect(
            &mut self,
            _: &EntityPhysicalState,
            effect: RuntimeItemEffect,
            seed: Option<i64>,
        ) -> Result<Vec<EffectDelivery>> {
            ensure!(seed.is_none(), "no sound in dry original scene");
            ensure!(
                matches!(effect, RuntimeItemEffect::HitGround { .. }),
                "unexpected controlled-world effect"
            );
            Ok(Vec::new())
        }
        fn entity_damage(
            &mut self,
            _: &EntityPhysicalState,
            _: &ItemStack,
            _: i32,
            _: Option<i32>,
        ) -> Result<()> {
            bail!("no damage in controlled scene")
        }
        fn discarded(&mut self, _: &EntityPhysicalState, _: &ItemStack, _: i32) -> Result<()> {
            bail!("no removal in controlled scene")
        }
        fn add_to_inventory(
            &mut self,
            _: PickupPlayer,
            _: i32,
            _: i32,
            _: &mut ItemStack,
        ) -> Result<bool> {
            bail!("no pickup phase")
        }
        fn collect_item(&mut self, _: PickupPlayer, _: &ItemState, _: i32, _: i32) -> Result<()> {
            bail!("no pickup phase")
        }
        fn collected(&mut self, _: PickupPlayer, _: &ItemState, _: Pickup, _: i32) -> Result<()> {
            bail!("no pickup phase")
        }
    }
    struct VisibilityHost(RuntimeEntities);
    impl ChunkManagerHost for VisibilityHost {
        fn visibility_changed(
            &mut self,
            holder: HolderRef,
            _: life::FullStatus,
            visibility: Visibility,
        ) -> Result<()> {
            self.0.set_chunk_visibility(holder.chunk, visibility)
        }
    }
    #[derive(Default)]
    struct Actions {
        pending: BTreeSet<Chunk>,
    }
    impl RuntimeChunkActions for Actions {
        fn mark_chunk_pending_to_send(
            &mut self,
            chunk: Chunk,
            _: &mut RuntimeWorld,
        ) -> Result<(), ReadinessError> {
            self.pending.insert(chunk);
            Ok(())
        }
        fn try_schedule_light_update(&mut self, _: &mut RuntimeWorld) -> Result<()> {
            Ok(())
        }
    }
    let mut manager = RuntimeChunkManager::new(2, 2);
    let mut world = world();
    let mut sources = RuntimeWorldSources::fresh_flat_fixture(FreshFixtureFacts {
        inherited_source_entities: EmptyWorldFact::ObservedEmpty,
        global_upgrade_fixers: EmptyWorldFact::ObservedEmpty,
    });
    let mut actions = Actions::default();
    let mut backend = RuntimeWorldReadinessBackend::new(&mut sources, &mut actions, 123);
    let mut host = VisibilityHost(RuntimeEntities::new(LegacyRandomSource::new(98765)));
    let mut item = ItemState::new(
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
    item.pickup_delay = 40;
    item.take_metadata_dirty();
    host.0
        .insert_item(item, LegacyRandomSource::new(12345))
        .unwrap();
    manager.update_chunk_forced((0, 0), true).unwrap();
    for turn in 0..20000 {
        let sorter = manager.poll_sorter_one().unwrap();
        let priority = manager
            .poll_priority_one(PriorityOrderScope::NativeStablePriorityScope)
            .unwrap();
        let main = manager
            .poll_main_one(&mut world, &mut backend, &mut host, &mut order)
            .unwrap();
        if !sorter
            && priority == PriorityPoll::Idle
            && main == PollOutcome::Idle
            && !manager.has_scheduler_work()
        {
            break;
        }
        assert!(turn < 19999);
    }
    manager
        .certify_query_region(&mut world, &mut backend, (-2, -2), (2, 2))
        .unwrap();
    assert_eq!(
        backend.actions.pending.len(),
        9,
        "actual one-viewer pending enqueue for the nine block-ticking callbacks"
    );
    let original: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/item-full-vectors.json")).unwrap();
    let original_states = original["scenes"][0]["states"].as_array().unwrap();
    assert_eq!(original_states.len(), 80);
    let bits = |v: f64| format!("{:016x}", v.to_bits());
    let mut item_host = ItemHost;
    for (tick, expected) in original_states.iter().enumerate() {
        manager
            .certify_query_region(&mut world, &mut backend, (-2, -2), (2, 2))
            .unwrap();
        let facts = ItemWorldFacts {
            game_time: tick as i64 + 1,
            minimum_build_height: crate::world::MIN_Y,
            ultrawarm: false,
            border: CollisionBorder::default(),
            rain: RainAvailability::NotRaining,
            has_passengers: false,
        };
        host.0
            .simulate_level(&mut world, facts, &[], &mut item_host)
            .unwrap();
        let item = host.0.item(41).unwrap();
        assert_eq!(
            serde_json::json!([
                bits(item.position.x),
                bits(item.position.y),
                bits(item.position.z)
            ]),
            expected["position"],
            "original poststate position tick{tick}"
        );
        assert_eq!(
            serde_json::json!([
                bits(item.velocity.x),
                bits(item.velocity.y),
                bits(item.velocity.z)
            ]),
            expected["velocity"],
            "original poststate velocity tick{tick}"
        );
        assert_eq!(
            serde_json::json!([item.age, item.pickup_delay, item.tick_count, item.health]),
            serde_json::json!([
                expected["age"],
                expected["pickup_delay"],
                expected["tick_count"],
                expected["health"]
            ])
        );
        assert_eq!(item.on_ground(), expected["on_ground"].as_bool().unwrap());
        assert_eq!(
            format!("{:08x}", item.fall_distance.to_bits()),
            expected["fall_distance"].as_str().unwrap()
        );
    }
    println!("RUNTIME_CHUNK_MANAGER_ITEM_CONFORMANCE: {{\"ok\":true,\"original_scenes\":1,\"poststates_motion_and_counters\":80,\"pending_chunk_marks\":9,\"mismatches\":0}}");
}

#[derive(Default)]
struct GuardActions {
    marked: Vec<Chunk>,
    light_polls: usize,
}
impl RuntimeChunkActions for GuardActions {
    fn mark_chunk_pending_to_send(
        &mut self,
        chunk: Chunk,
        _: &mut RuntimeWorld,
    ) -> Result<(), ReadinessError> {
        self.marked.push(chunk);
        Ok(())
    }
    fn try_schedule_light_update(&mut self, _: &mut RuntimeWorld) -> Result<()> {
        self.light_polls += 1;
        Ok(())
    }
}
fn fixture_sources(entities: EmptyWorldFact, fixers: EmptyWorldFact) -> RuntimeWorldSources {
    RuntimeWorldSources::fresh_flat_fixture(FreshFixtureFacts {
        inherited_source_entities: entities,
        global_upgrade_fixers: fixers,
    })
}
fn reference() -> HolderRef {
    HolderRef {
        id: 72,
        chunk: (0, 0),
    }
}

#[test]
fn fixture_source_requires_observed_facts_and_rejects_null_factory_states() {
    let mut world = world();
    let mut actions = GuardActions::default();
    let mut sources = fixture_sources(EmptyWorldFact::Unavailable, EmptyWorldFact::ObservedEmpty);
    let mut backend = RuntimeWorldReadinessBackend::new(&mut sources, &mut actions, 123);
    let source = backend
        .get_or_schedule(
            reference(),
            Some(ChunkStatus::FULL),
            ChunkStatus::FULL,
            &mut world,
        )
        .unwrap();
    assert_eq!(source.initial, ChunkOutcome::Pending);
    assert!(backend
        .sources
        .pending_reason((0, 0))
        .unwrap()
        .contains("source-entity"));
    assert!(backend
        .certify_current_query_context(reference(), &mut world)
        .is_err());
    assert!(backend.actions.marked.is_empty());

    let mut sources = fixture_sources(EmptyWorldFact::ObservedEmpty, EmptyWorldFact::Unavailable);
    let mut backend = RuntimeWorldReadinessBackend::new(&mut sources, &mut actions, 123);
    let source = backend
        .get_or_schedule(
            reference(),
            Some(ChunkStatus::FULL),
            ChunkStatus::FULL,
            &mut world,
        )
        .unwrap();
    let ChunkOutcome::Success(data) = source.initial else {
        panic!("fresh full data exists")
    };
    assert!(backend
        .post_process_generation(data.id, &mut world)
        .is_err());
    assert!(!backend.sources.data[&data.id].post_processed);
    assert!(backend
        .certify_current_query_context(reference(), &mut world)
        .is_err());

    let facts: serde_json::Value =
        serde_json::from_str(include_str!("../tools/storage_blockentity_factories.json")).unwrap();
    let null_factory = facts["states"]
        .as_array()
        .unwrap()
        .iter()
        .position(|state| state.as_i64() == Some(-2))
        .unwrap() as u32;
    assert!(
        crate::storage::block_entity_factory_snapshot(null_factory, 0, 64, 0)
            .unwrap()
            .is_none()
    );
    let RuntimeWorld::Fixture(flat) = &mut world else {
        unreachable!()
    };
    flat.set_block(0, 64, 0, null_factory);
    let mut sources = fixture_sources(EmptyWorldFact::ObservedEmpty, EmptyWorldFact::ObservedEmpty);
    let mut backend = RuntimeWorldReadinessBackend::new(&mut sources, &mut actions, 123);
    assert_eq!(
        backend
            .get_or_schedule(
                reference(),
                Some(ChunkStatus::FULL),
                ChunkStatus::FULL,
                &mut world
            )
            .unwrap()
            .initial,
        ChunkOutcome::Pending
    );
    // The pending reason names the unavailable block entity; the wording moved
    // from "block-entity" to "native block entity unavailable", same outcome.
    let reason = backend.sources.pending_reason((0, 0)).unwrap();
    assert!(
        reason.contains("block-entity") || reason.contains("block entity"),
        "{reason}"
    );
    assert!(backend.sources.data.is_empty());
    let RuntimeWorld::Fixture(flat) = &mut world else {
        unreachable!()
    };
    flat.set_block(0, 64, 0, 0);
    let pass = backend
        .generation_pass(reference(), Some(ChunkStatus::FULL), &mut world)
        .unwrap();
    assert!(matches!(
        pass.availability,
        GenerationAvailability::ResidentSources
    ));
    assert!(backend.sources.pending_reason((0, 0)).is_none());
    // Materialization is not a fabricated completion of an already-pending
    // generation future. This backend has no asynchronous generation driver.
    assert_eq!(
        backend
            .get_or_schedule(
                reference(),
                Some(ChunkStatus::FULL),
                ChunkStatus::FULL,
                &mut world
            )
            .unwrap()
            .initial,
        ChunkOutcome::Pending
    );
}

#[test]
fn generation_slots_retain_completed_identity_and_reset_only_pending_disabled_sources() {
    let mut world = world();
    let mut actions = GuardActions::default();
    let mut sources = fixture_sources(EmptyWorldFact::ObservedEmpty, EmptyWorldFact::ObservedEmpty);
    let mut backend = RuntimeWorldReadinessBackend::new(&mut sources, &mut actions, 123);
    let source = backend
        .get_or_schedule(
            reference(),
            Some(ChunkStatus::FULL),
            ChunkStatus::FULL,
            &mut world,
        )
        .unwrap();
    let ChunkOutcome::Success(data) = source.initial else {
        panic!("resident full")
    };
    backend.start_ticking_chunk(data.id, &mut world).unwrap();
    assert_eq!(backend.sources.ticks_unpacked_at(data.id), Some(123));
    backend.game_time = 456;
    backend.start_ticking_chunk(data.id, &mut world).unwrap();
    assert_eq!(backend.sources.ticks_unpacked_at(data.id), Some(123));
    let disabled = backend
        .generation_pass(reference(), None, &mut world)
        .unwrap();
    assert!(disabled.completions.is_empty());
    assert!(matches!(
        disabled.availability,
        GenerationAvailability::NoAllowedStatus
    ));
    let not_allowed = backend
        .get_or_schedule(reference(), None, ChunkStatus::FULL, &mut world)
        .unwrap();
    assert_eq!(not_allowed.identity, 1);
    assert_eq!(
        not_allowed.initial,
        ChunkOutcome::Failure("Unloaded chunk".into())
    );
    let again = backend
        .get_or_schedule(
            reference(),
            Some(ChunkStatus::FULL),
            ChunkStatus::FULL,
            &mut world,
        )
        .unwrap();
    assert_eq!(again.identity, source.identity);
    assert_eq!(again.initial, ChunkOutcome::Success(data));

    let mut sources = fixture_sources(EmptyWorldFact::Unavailable, EmptyWorldFact::ObservedEmpty);
    let mut backend = RuntimeWorldReadinessBackend::new(&mut sources, &mut actions, 123);
    let pending = backend
        .get_or_schedule(
            reference(),
            Some(ChunkStatus::FULL),
            ChunkStatus::FULL,
            &mut world,
        )
        .unwrap();
    assert_eq!(pending.initial, ChunkOutcome::Pending);
    let disabled = backend
        .generation_pass(reference(), None, &mut world)
        .unwrap();
    assert_eq!(
        disabled.completions,
        [(
            pending.identity,
            ChunkOutcome::Failure("Unloaded chunk".into())
        )]
    );
    let again = backend
        .get_or_schedule(
            reference(),
            Some(ChunkStatus::FULL),
            ChunkStatus::FULL,
            &mut world,
        )
        .unwrap();
    assert_ne!(again.identity, pending.identity);
    assert_eq!(again.initial, ChunkOutcome::Pending);
}

#[test]
fn full_anvil_nbt_and_loaded_cache_do_not_substitute_for_current_generation_futures() {
    use crate::{
        anvil::{ChunkCompression, RegionFile},
        nbt::{NbtRoot, Tag},
        storage::VanillaWorld,
    };
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "mc121-manager-anvil-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(path.join("region")).unwrap();
    let root = NbtRoot {
        name: "".into(),
        tag: Tag::Compound(vec![
            ("DataVersion".into(), Tag::Int(3953)),
            ("xPos".into(), Tag::Int(0)),
            ("zPos".into(), Tag::Int(0)),
            ("yPos".into(), Tag::Int(-4)),
            ("Status".into(), Tag::String("minecraft:full".into())),
            ("isLightOn".into(), Tag::Byte(1)),
            (
                "sections".into(),
                Tag::List {
                    element_type: 10,
                    values: vec![],
                },
            ),
            (
                "block_entities".into(),
                Tag::List {
                    element_type: 10,
                    values: vec![],
                },
            ),
            ("Heightmaps".into(), Tag::Compound(vec![])),
        ]),
    };
    let mut region = RegionFile::empty(0, 0).unwrap();
    region
        .insert_chunk(0, 0, &root, ChunkCompression::Uncompressed, 0)
        .unwrap();
    region.save_atomic(path.join("region/r.0.0.mca")).unwrap();
    let mut stored = VanillaWorld::open(&path).unwrap();
    assert_eq!(stored.block(0, 64, 0).unwrap(), 0);
    assert!(stored.cached_chunk_is_loaded(0, 0));
    let mut world = RuntimeWorld::Anvil(stored);
    let mut sources = fixture_sources(EmptyWorldFact::ObservedEmpty, EmptyWorldFact::ObservedEmpty);
    let mut actions = GuardActions::default();
    let mut backend = RuntimeWorldReadinessBackend::new(&mut sources, &mut actions, 123);
    let pass = backend
        .generation_pass(reference(), Some(ChunkStatus::FULL), &mut world)
        .unwrap();
    assert!(matches!(
        pass.availability,
        GenerationAvailability::Pending(_)
    ));
    assert_eq!(
        backend
            .get_or_schedule(
                reference(),
                Some(ChunkStatus::FULL),
                ChunkStatus::FULL,
                &mut world
            )
            .unwrap()
            .initial,
        ChunkOutcome::Pending
    );
    assert!(backend
        .certify_current_query_context(reference(), &mut world)
        .is_err());
    // Source slots belong to one holder-identity registry. The independent
    // driver below starts its own registry, rather than aliasing reference72.
    let mut sources = fixture_sources(EmptyWorldFact::ObservedEmpty, EmptyWorldFact::ObservedEmpty);
    let mut backend = RuntimeWorldReadinessBackend::new(&mut sources, &mut actions, 123);
    let mut manager = RuntimeChunkManager::new(2, 2);
    let mut host = Host::default();
    manager.update_chunk_forced((0, 0), true).unwrap();
    for _ in 0..1000 {
        let sorter = manager.poll_sorter_one().unwrap();
        let priority = manager
            .poll_priority_one(PriorityOrderScope::NativeStablePriorityScope)
            .unwrap();
        let main = manager
            .poll_main_one(&mut world, &mut backend, &mut host, &mut order)
            .unwrap();
        if !sorter
            && priority == PriorityPoll::Idle
            && main == PollOutcome::Idle
            && !manager.has_scheduler_work()
        {
            break;
        }
    }
    assert!(!manager.current_full_ready((0, 0)));
    assert!(host.statuses.is_empty());
    assert!(manager.certify_full_region((0, 0), (0, 0)).is_err());
    assert!(backend.actions.marked.is_empty());
    drop(world);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn failed_visibility_callback_poison_prevents_later_tick_certification() {
    struct RejectHost;
    impl ChunkManagerHost for RejectHost {
        fn visibility_changed(
            &mut self,
            _: HolderRef,
            _: life::FullStatus,
            _: Visibility,
        ) -> Result<()> {
            bail!("controlled index update failure")
        }
    }
    let mut manager = RuntimeChunkManager::new(2, 2);
    let mut world = world();
    let mut backend = Backend::new(true);
    let mut host = RejectHost;
    manager.update_chunk_forced((0, 0), true).unwrap();
    let mut failure = None;
    for _ in 0..1000 {
        manager.poll_sorter_one().unwrap();
        manager
            .poll_priority_one(PriorityOrderScope::NativeStablePriorityScope)
            .unwrap();
        if let Err(error) = manager.poll_main_one(&mut world, &mut backend, &mut host, &mut order) {
            failure = Some(error.to_string());
            break;
        }
    }
    assert!(failure.unwrap().contains("controlled index update failure"));
    assert!(manager.certify_full_region((0, 0), (0, 0)).is_err());
    assert!(manager
        .certify_query_region(&mut world, &mut backend, (0, 0), (0, 0))
        .is_err());
    assert!(manager.player_add(71, (0, 0), false).is_err());
    assert!(manager.poll_sorter_one().is_err());
}
