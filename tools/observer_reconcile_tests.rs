use super::*;
use mc121_rust::item_entity::Pickup;
use std::collections::VecDeque;

const OBSERVER: i32 = 1;
const ITEM: i32 = 20;

fn observer(x: f64) -> TrackingObserver {
    TrackingObserver {
        entity_id: OBSERVER,
        position: Vec3::new(x, 65., 8.5),
        requested_view_distance: 2,
        server_view_distance: 2,
        is_spectator: false,
        tracking_distance_percent: 100,
    }
}

fn item() -> EntitySnapshot<ItemState> {
    let position = Position {
        x: 8.5,
        y: 65.,
        z: 8.5,
        yaw: 0.,
        pitch: 0.,
        on_ground: true,
    };
    EntitySnapshot {
        entity: ItemState::new(
            ITEM,
            [20; 16],
            position,
            Vec3::ZERO,
            ItemStack::new(1, 3).unwrap(),
        )
        .unwrap(),
        spawn_position: position,
        spawn_velocity: Some(Vec3::ZERO),
    }
}

fn reconcile(
    observer: TrackingObserver,
    revision: u64,
    items: Vec<EntitySnapshot<ItemState>>,
) -> Arc<ObserverReconcile> {
    Arc::new(ObserverReconcile {
        observer,
        snapshot: LiveSnapshot {
            player_revision: revision,
            players: vec![],
            player_baselines: BTreeMap::new(),
            item_revision: revision,
            items,
        },
    })
}

fn marker(reconcile: Arc<ObserverReconcile>) -> PlayerEvent {
    PlayerEvent::ObserverReconcile {
        recipient: OBSERVER,
        reconcile,
    }
}

fn pickup() -> Pickup {
    Pickup {
        item_entity_id: ITEM,
        player_entity_id: OBSERVER,
        packet_count: 3,
        accepted_count: 3,
        removed: false,
    }
}

fn collected() -> PlayerEvent {
    PlayerEvent::Item(Arc::new(RuntimeEntityEvent::Collected {
        revision: 2,
        pickup: pickup(),
    }))
}

fn removed() -> PlayerEvent {
    PlayerEvent::Item(Arc::new(RuntimeEntityEvent::Removed {
        revision: 3,
        entity_id: ITEM,
    }))
}

fn paired(loaded: &BTreeSet<(i32, i32)>) -> ClientTrackers {
    let mut trackers = ClientTrackers::new(OBSERVER);
    let packets = trackers
        .apply_reconcile(&reconcile(observer(8.5), 1, vec![item()]), loaded)
        .unwrap();
    assert!(packets.iter().any(|packet| packet.id == 0x01));
    assert_eq!(trackers.items.visible_entity_ids(), BTreeSet::from([ITEM]));
    trackers
}

/// Replay the session's relevant FIFO arms without network scheduling. Chunk
/// eligibility is installed after its packet batch, then its captured marker.
fn deliver(
    trackers: &mut ClientTrackers,
    transport: &mut SessionChunkTransport,
    loaded: &mut BTreeSet<(i32, i32)>,
    event: PlayerEvent,
) -> Vec<Packet> {
    match event {
        PlayerEvent::Item(event) => trackers.item_event(OBSERVER, &event, loaded).unwrap(),
        PlayerEvent::ObserverReconcile {
            recipient,
            reconcile,
        } => {
            assert_eq!(recipient, OBSERVER);
            trackers.apply_reconcile(&reconcile, loaded).unwrap()
        }
        PlayerEvent::ChunksReady {
            recipient,
            cutoff,
            reconcile,
        } => {
            assert_eq!(recipient, OBSERVER);
            let mut packets = vec![];
            for update in transport.drain_through(cutoff) {
                assert!(update.revision <= cutoff);
                packets.extend(update.packets);
                *loaded = update.loaded;
                trackers.players.set_pending_chunks(update.pending.clone());
                trackers.items.set_pending_chunks(update.pending);
            }
            if let Some(reconcile) = reconcile {
                packets.extend(trackers.apply_reconcile(&reconcile, loaded).unwrap());
            }
            packets
        }
        _ => panic!("unexpected event in observer ordering test"),
    }
}

#[test]
fn collected_and_removed_precede_later_absent_observer_snapshot() {
    let mut loaded = BTreeSet::from([(0, 0)]);
    let mut trackers = paired(&loaded);
    let mut transport = SessionChunkTransport::new();
    // The server has already removed the item when it captures this marker.
    // Its earlier Collect and Remove publications still wait in the session.
    let absent = reconcile(observer(8.75), 3, vec![]);
    let mut queue = VecDeque::from([collected(), removed(), marker(absent)]);
    let mut packets = vec![];
    while let Some(event) = queue.pop_front() {
        packets.extend(deliver(&mut trackers, &mut transport, &mut loaded, event));
    }
    assert_eq!(
        packets,
        vec![
            pickup().packet(),
            entity::destroy_entities(&[ITEM]).unwrap()
        ]
    );
    assert!(trackers.items.visible_entity_ids().is_empty());
    assert!(trackers
        .apply_reconcile(&reconcile(observer(8.5), 3, vec![]), &loaded)
        .unwrap()
        .is_empty());
}

#[test]
fn earlier_out_of_range_marker_suppresses_collect_and_no_later_respawn() {
    let mut loaded = BTreeSet::from([(0, 0)]);
    let mut trackers = paired(&loaded);
    let mut transport = SessionChunkTransport::new();
    let outside = reconcile(observer(1000.5), 1, vec![item()]);
    let returned = reconcile(observer(8.5), 3, vec![]);
    let mut queue = VecDeque::from([marker(outside), collected(), removed(), marker(returned)]);
    let mut packets = vec![];
    while let Some(event) = queue.pop_front() {
        packets.extend(deliver(&mut trackers, &mut transport, &mut loaded, event));
    }
    assert_eq!(packets, vec![entity::destroy_entities(&[ITEM]).unwrap()]);
    assert!(trackers.items.visible_entity_ids().is_empty());
}

#[test]
fn stale_chunk_wake_cannot_drain_future_view_or_its_absent_reconcile() {
    let mut transport = SessionChunkTransport::new();
    let near = entity_tracking::ChunkView {
        center: (0, 0),
        view_distance: 2,
    };
    transport.queue_view(near, true, true).unwrap();
    transport.drain_through(transport.latest_revision());
    let mut world = WorldBackend::Fixture(FlatWorld::new(0, 1, 10, 9, 0));
    assert_eq!(
        transport
            .send_next(&mut world, near.center, |_| Ok(true))
            .unwrap(),
        9
    );
    let batches = transport.drain_through(transport.latest_revision());
    assert_eq!(batches.len(), 1);
    let mut loaded = batches[0].loaded.clone();
    assert_eq!(loaded.len(), 9);
    assert!(loaded.contains(&(0, 0)));
    let mut trackers = paired(&loaded);
    trackers
        .players
        .set_pending_chunks(batches[0].pending.clone());
    trackers
        .items
        .set_pending_chunks(batches[0].pending.clone());

    let old_packet = Packet {
        id: 0x64,
        payload: vec![1; 16],
    };
    transport.publish_packets(vec![old_packet.clone()]);
    let old_cutoff = transport.latest_revision();
    let old_wake = PlayerEvent::ChunksReady {
        recipient: OBSERVER,
        cutoff: old_cutoff,
        reconcile: Some(reconcile(observer(8.5), 1, vec![item()])),
    };

    // Future transport and world state already exist in memory, but their wake
    // belongs after Collect. An old wake must not fetch either future state.
    transport
        .queue_view(
            entity_tracking::ChunkView {
                center: (64, 0),
                view_distance: 2,
            },
            true,
            true,
        )
        .unwrap();
    let future_cutoff = transport.latest_revision();
    assert!(future_cutoff > old_cutoff);
    let future_wake = PlayerEvent::ChunksReady {
        recipient: OBSERVER,
        cutoff: future_cutoff,
        reconcile: Some(reconcile(observer(1032.5), 3, vec![])),
    };

    let old_packets = deliver(&mut trackers, &mut transport, &mut loaded, old_wake);
    assert_eq!(old_packets, vec![old_packet]);
    assert_eq!(loaded, batches[0].loaded);
    assert_eq!(trackers.items.visible_entity_ids(), BTreeSet::from([ITEM]));
    assert!(transport.drain_through(old_cutoff).is_empty());

    let collect = deliver(&mut trackers, &mut transport, &mut loaded, collected());
    assert_eq!(collect, vec![pickup().packet()]);
    let remove = deliver(&mut trackers, &mut transport, &mut loaded, removed());
    assert_eq!(remove, vec![entity::destroy_entities(&[ITEM]).unwrap()]);
    let future_packets = deliver(&mut trackers, &mut transport, &mut loaded, future_wake);
    assert!(future_packets.iter().any(|packet| packet.id == 0x21));
    assert!(future_packets
        .iter()
        .all(|packet| packet.id != 0x01 && packet.id != 0x6f));
    assert!(loaded.is_empty());
    assert!(transport.drain_through(future_cutoff).is_empty());
    assert!(trackers.items.visible_entity_ids().is_empty());
}

#[test]
fn accepted_out_and_in_markers_remain_distinct_after_future_world_mutation() {
    let mut loaded = BTreeSet::from([(0, 0)]);
    let mut trackers = paired(&loaded);
    let mut transport = SessionChunkTransport::new();
    let mut live_items = vec![item()];
    let outside = reconcile(observer(1000.5), 1, live_items.clone());
    let inside = reconcile(observer(8.5), 1, live_items.clone());
    live_items.clear();
    let future = reconcile(observer(8.5), 3, live_items);

    let unpair = deliver(&mut trackers, &mut transport, &mut loaded, marker(outside));
    assert_eq!(unpair, vec![entity::destroy_entities(&[ITEM]).unwrap()]);
    let pair = deliver(&mut trackers, &mut transport, &mut loaded, marker(inside));
    // This captured marker still contains the item even though live state was
    // removed before either accepted observer transition was delivered.
    assert_eq!(pair, item().entity.spawn_packets().unwrap());
    assert_eq!(trackers.items.visible_entity_ids(), BTreeSet::from([ITEM]));
    let collect = deliver(&mut trackers, &mut transport, &mut loaded, collected());
    assert_eq!(collect, vec![pickup().packet()]);
    let remove = deliver(&mut trackers, &mut transport, &mut loaded, removed());
    assert_eq!(remove, vec![entity::destroy_entities(&[ITEM]).unwrap()]);
    assert!(deliver(&mut trackers, &mut transport, &mut loaded, marker(future)).is_empty());
    assert!(trackers.items.visible_entity_ids().is_empty());
}

#[tokio::test]
async fn bounded_entity_history_loss_is_terminal_before_applying_snapshot_tail() {
    let mut loaded = BTreeSet::from([(0, 0)]);
    let mut trackers = paired(&loaded);
    let mut transport = SessionChunkTransport::new();
    let (sender, mut receiver) = broadcast::channel(1);
    assert!(sender.send(collected()).is_ok());
    assert!(sender.send(removed()).is_ok());
    assert!(sender
        .send(marker(reconcile(observer(8.75), 3, vec![])))
        .is_ok());

    // The real bounded receiver has discarded both ordered transient events.
    // Applying the retained absent-state marker would fabricate a Destroy
    // without the earlier Collect. Production classifies that gap as terminal.
    let mut packets = vec![];
    let terminal = match ordered_entity_input(receiver.recv().await) {
        OrderedEntityInput::DisconnectLostHistory { skipped } => {
            assert!(skipped > 0);
            true
        }
        OrderedEntityInput::Event(event) => {
            packets.extend(deliver(&mut trackers, &mut transport, &mut loaded, event));
            false
        }
        OrderedEntityInput::Closed => panic!("sender is still alive"),
    };
    assert!(
        terminal,
        "lost history must not be replaced with a snapshot"
    );
    assert!(packets.is_empty());
    assert_eq!(trackers.items.visible_entity_ids(), BTreeSet::from([ITEM]));

    // The marker was retained by broadcast, but the terminal decision above
    // has not delivered it or changed the observer's previously visible state.
    assert!(matches!(
        receiver.try_recv(),
        Ok(PlayerEvent::ObserverReconcile {
            recipient: OBSERVER,
            ..
        })
    ));
    assert_eq!(trackers.items.visible_entity_ids(), BTreeSet::from([ITEM]));
}
