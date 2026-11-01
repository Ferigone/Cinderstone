use super::*;
use mc121_rust::{item_entity::Pickup, runtime_entities::EffectDelivery};

fn position(x: f64) -> Position {
    Position {
        x,
        y: 65.,
        z: 8.5,
        yaw: 0.,
        pitch: 0.,
        on_ground: true,
    }
}
fn observer(id: i32) -> TrackingObserver {
    TrackingObserver {
        entity_id: id,
        position: Vec3::new(8.5, 65., 8.5),
        requested_view_distance: 2,
        server_view_distance: 2,
        is_spectator: false,
        tracking_distance_percent: 100,
    }
}
fn item(id: i32) -> ItemState {
    ItemState::new(
        id,
        [id as u8; 16],
        position(8.5),
        Vec3::ZERO,
        ItemStack::new(1, 3).unwrap(),
    )
    .unwrap()
}
fn snapshot(item: ItemState) -> EntitySnapshot<ItemState> {
    EntitySnapshot {
        spawn_position: item.position,
        spawn_velocity: Some(item.velocity),
        entity: item,
    }
}

#[test]
fn phase5_observer_pairing_item_transients_and_effect_recipients() {
    let loaded = BTreeSet::from([(0, 0)]);
    let mut trackers = ClientTrackers::new(1);
    assert!(trackers.items.sync(0, &[], &loaded).is_err());
    assert!(trackers.players.sync(0, &[], &loaded).is_err());
    trackers.set_observer(observer(1)).unwrap();
    let state = snapshot(item(10));
    let update = RuntimeEntityEvent::Update {
        revision: 1,
        snapshot: Box::new(state.clone()),
        packets: vec![],
    };
    let spawned = trackers.item_event(1, &update, &loaded).unwrap();
    assert!(spawned.iter().any(|packet| packet.id == 0x01));
    let collected = RuntimeEntityEvent::Collected {
        revision: 2,
        pickup: Pickup {
            item_entity_id: 10,
            player_entity_id: 1,
            packet_count: 3,
            accepted_count: 3,
            removed: false,
        },
    };
    let packets = trackers.item_event(1, &collected, &loaded).unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].id, 0x6f);
    assert!(trackers
        .item_event(1, &collected, &loaded)
        .unwrap()
        .is_empty());
    let mut fresh = ClientTrackers::new(2);
    fresh.set_observer(observer(2)).unwrap();
    // No unconditional collector self-send: collection uses actual seenBy.
    assert!(fresh.item_event(1, &collected, &loaded).unwrap().is_empty());
    let sound = Packet {
        id: 0x6a,
        payload: vec![7],
    };
    let effect = RuntimeEntityEvent::Effect {
        revision: 3,
        deliveries: vec![
            EffectDelivery {
                recipients: vec![1],
                packet: sound.clone(),
            },
            EffectDelivery {
                recipients: vec![2],
                packet: Packet {
                    id: 0x6a,
                    payload: vec![8],
                },
            },
        ],
    };
    let actual = trackers.item_event(1, &effect, &loaded).unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].payload, sound.payload);
    assert!(trackers.item_event(1, &effect, &loaded).unwrap().is_empty());
    let removed = RuntimeEntityEvent::Removed {
        revision: 4,
        entity_id: 10,
    };
    assert_eq!(
        trackers.item_event(1, &removed, &loaded).unwrap()[0].id,
        0x42
    );
    assert!(trackers.item_event(1, &update, &loaded).unwrap().is_empty());
}

#[test]
fn phase5_lag_snapshot_suppresses_old_state_but_preserves_visible_transient() {
    let loaded = BTreeSet::from([(0, 0)]);
    let mut trackers = ClientTrackers::new(1);
    trackers.set_observer(observer(1)).unwrap();
    let state = snapshot(item(10));
    trackers
        .items
        .sync(1, std::slice::from_ref(&state), &loaded)
        .unwrap();
    trackers
        .items
        .sync(5, std::slice::from_ref(&state), &loaded)
        .unwrap();
    let old = RuntimeEntityEvent::Update {
        revision: 4,
        snapshot: Box::new(state.clone()),
        packets: vec![Packet {
            id: 0x2e,
            payload: vec![1],
        }],
    };
    assert!(trackers.item_event(1, &old, &loaded).unwrap().is_empty());
    let collect = RuntimeEntityEvent::Collected {
        revision: 4,
        pickup: Pickup {
            item_entity_id: 10,
            player_entity_id: 1,
            packet_count: 3,
            accepted_count: 3,
            removed: false,
        },
    };
    assert_eq!(trackers.item_event(1, &collect, &loaded).unwrap().len(), 1);
    let mut joined = ClientTrackers::new(2);
    joined.set_observer(observer(2)).unwrap();
    joined
        .items
        .sync(5, std::slice::from_ref(&state), &loaded)
        .unwrap();
    assert!(joined.item_event(2, &collect, &loaded).unwrap().is_empty());
}

#[test]
fn phase5_accept_order_and_confirmations_are_separate_from_ticking_range() {
    let (sender, _) = broadcast::channel(32);
    let mut players = LivePlayers::new(
        LegacyRandomSource::new(12),
        LegacyRandomSource::new(13),
        WorldContext::new(
            DimensionFacts {
                min_y: -64,
                height: 384,
                logical_height: 384,
                ultrawarm: false,
                has_skylight: true,
                has_ceiling: false,
            },
            WorldRules::default(),
            WeatherState::default(),
        ),
        2,
        true,
        sender,
    );
    players
        .register_runtime_player(
            10,
            position(8.5),
            100,
            LegacyRandomSource::new(31),
            [10; 16],
        )
        .unwrap();
    players
        .register_runtime_player(
            20,
            position(640.5),
            50,
            LegacyRandomSource::new(32),
            [20; 16],
        )
        .unwrap();
    assert_eq!(
        players
            .connection_admissions
            .values()
            .copied()
            .collect::<Vec<_>>(),
        [20, 10]
    );
    assert_eq!(players.player_random.len(), 2);
    let mut range = mc121_rust::entity_ticking_range::EntityTickingRange::new(2);
    range.add_player((0, 0), 10);
    range.add_player((40, 0), 20);
    range.run_all_updates();
    assert!(range.in_entity_ticking_range((0, 0)));
    players
        .item_runtime
        .insert_item(item(30), LegacyRandomSource::new(44))
        .unwrap();
    assert!(
        players.item_runtime.snapshots().unwrap().1.is_empty(),
        "graph range does not confirm ChunkHolder visibility"
    );
    players
        .apply_confirmed_chunk_status((0, 0), FullStatus::EntityTicking)
        .unwrap();
    assert_eq!(players.item_runtime.snapshots().unwrap().1.len(), 1);
    players.remove_runtime_player(20).unwrap();
    range.remove_player((40, 0), 20);
    range.run_all_updates();
    assert_eq!(
        players
            .connection_admissions
            .values()
            .copied()
            .collect::<Vec<_>>(),
        [10]
    );
    assert!(!range.in_entity_ticking_range((40, 0)));
    // A literal stale queued callback is applied, independent of graph level.
    players
        .apply_confirmed_chunk_status((0, 0), FullStatus::Inaccessible)
        .unwrap();
    assert!(players.item_runtime.snapshots().unwrap().1.is_empty());
}

#[test]
fn phase5_unavailable_nonempty_drop_rolls_back_menu_and_publisher() {
    let mut menu = PlayerContainer::new();
    menu.inventory
        .set_creative_slot(36, ItemStack::new(1, 5).unwrap())
        .unwrap();
    let (container, mut updates) = SharedContainer::from_container(menu);
    let before = container
        .read(|menu| menu.inventory.selected_stack(0).unwrap().clone())
        .unwrap();
    let result = menu_without_world_drops(&container, |menu| menu.drop_selected(false));
    assert!(result
        .unwrap_err()
        .downcast_ref::<ItemRuntimeNotReady>()
        .is_some());
    let after = container
        .read(|menu| menu.inventory.selected_stack(0).unwrap().clone())
        .unwrap();
    assert_eq!(before, after);
    assert!(updates.try_recv().is_err());
}

#[test]
fn phase5_same_chunk_observer_move_discovers_previously_uncached_item() {
    let loaded = BTreeSet::from([(0, 0), (1, 0), (2, 0)]);
    let mut trackers = ClientTrackers::new(1);
    let mut view = observer(1);
    view.position.x = 0.5;
    trackers.set_observer(view).unwrap();
    let mut target = item(10);
    target.position.x = 33.;
    target.bounds = Aabb::player(Vec3::new(33., 65., 8.5), 0.25, 0.25);
    let state = snapshot(target);
    assert!(trackers
        .items
        .sync(1, std::slice::from_ref(&state), &loaded)
        .unwrap()
        .is_empty());
    view.position.x = 1.5;
    trackers.set_observer(view).unwrap();
    // Both positions are in chunk0; no item update or chunk transport occurs.
    let packets = trackers
        .items
        .sync(1, std::slice::from_ref(&state), &loaded)
        .unwrap();
    assert!(packets.iter().any(|packet| packet.id == 0x01));
}

#[test]
fn phase5_world_packets_keep_tick_order_and_recovery_revision_barrier() {
    let mut tracker = ClientTrackers::new(1);
    let time = Packet {
        id: 0x64,
        payload: vec![1; 16],
    };
    let rain = weather_packet(7, 0.5);
    let packets = tracker.world_event(2, &[time.clone(), rain.clone()]);
    assert_eq!(
        packets.iter().map(|packet| packet.id).collect::<Vec<_>>(),
        [0x64, 0x22]
    );
    tracker.world_revision = Some(5);
    assert!(tracker
        .world_event(4, std::slice::from_ref(&rain))
        .is_empty());
    assert_eq!(tracker.world_event(6, &[rain]).len(), 1);
    let clear = WorldSnapshot {
        revision: 0,
        time_payload: [0; 16],
        weather: WeatherState::default(),
    };
    assert!(clear.weather_packets(false).is_empty());
    assert_eq!(
        clear
            .recovery_packets()
            .iter()
            .map(|packet| packet.id)
            .collect::<Vec<_>>(),
        [0x64, 0x22, 0x22, 0x22]
    );
}

#[test]
fn phase5_chunk_transport_keeps_batches_when_broadcast_wakes_are_lost() {
    let mut transport = SessionChunkTransport::new();
    let view = entity_tracking::ChunkView {
        center: (0, 0),
        view_distance: 2,
    };
    transport.queue_view(view, true, true).unwrap();
    let initial = transport.drain();
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].packets[0].id, 0x54);
    assert_eq!(initial[0].pending.len(), 45);
    assert!(initial[0].loaded.is_empty());
    let mut world = WorldBackend::Fixture(FlatWorld::new(0, 1, 10, 9, 0));
    assert_eq!(
        transport
            .send_next(&mut world, (0, 0), |_| Ok(true))
            .unwrap(),
        9
    );
    assert_eq!(
        transport
            .send_next(&mut world, (0, 0), |_| Ok(true))
            .unwrap(),
        0
    );
    // No session drains the outbox. An ACK changes the original retained gate,
    // and a second batch must coexist with the first unconsumed update.
    transport.on_ack(16.);
    assert_eq!(
        transport
            .send_next(&mut world, (0, 0), |_| Ok(true))
            .unwrap(),
        16
    );
    let updates = transport.drain();
    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0].loaded.len(), 9);
    assert_eq!(updates[1].loaded.len(), 25);
    assert_eq!(updates[1].pending.len(), 20);
    for (update, count) in updates.iter().zip([9, 16]) {
        assert_eq!(update.packets.first().unwrap().id, 0x0d);
        assert_eq!(update.packets.last().unwrap().id, 0x0c);
        assert_eq!(
            update
                .packets
                .iter()
                .filter(|packet| packet.id == 0x27)
                .count(),
            count
        );
        let mut end = Reader::new(&update.packets.last().unwrap().payload);
        assert_eq!(end.varint().unwrap(), count as i32);
        end.finish().unwrap();
    }
    assert!(transport.drain().is_empty());
}

#[test]
fn phase5_chunk_view_drops_pending_without_unload_and_preserves_callback_order() {
    let old = entity_tracking::ChunkView {
        center: (0, 0),
        view_distance: 2,
    };
    let next = entity_tracking::ChunkView {
        center: (30, -30),
        view_distance: 2,
    };
    let mut transport = SessionChunkTransport::new();
    transport.queue_view(old, false, true).unwrap();
    transport.drain();
    let mut world = WorldBackend::Fixture(FlatWorld::new(0, 1, 10, 9, 0));
    transport
        .send_next(&mut world, (0, 0), |_| Ok(true))
        .unwrap();
    // Deliberately leave the first actual batch queued before the move.
    transport.queue_view(next, true, true).unwrap();
    let updates = transport.drain();
    assert_eq!(updates.len(), 2);
    let scheduled = &updates[0].loaded;
    let mut actual = vec![];
    for packet in &updates[1].packets {
        if packet.id == 0x21 {
            let mut input = Reader::new(&packet.payload);
            let z = input.i32().unwrap();
            let x = input.i32().unwrap();
            input.finish().unwrap();
            actual.push((x, z));
        }
    }
    let expected = entity_tracking::chunk_view_difference(Some(old), Some(next))
        .unwrap()
        .into_iter()
        .filter_map(|change| match change {
            entity_tracking::ChunkViewChange::Remove(position) if scheduled.contains(&position) => {
                Some(position)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 9);
    assert!(updates[1].loaded.is_empty());
    assert_eq!(updates[1].pending.len(), 45);
    assert!(!transport.mark_pending_if_wanted((0, 0)));
    assert!(transport.mark_pending_if_wanted(next.center));
    transport.publish_packets(vec![]);
    assert_eq!(transport.drain().len(), 1);
}
