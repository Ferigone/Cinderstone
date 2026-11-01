use super::*;
use mc121_rust::player_stats::{StatKey, CUSTOM, CUSTOM_DROP, ITEM_DROPPED};
use std::sync::Mutex as StdMutex;

fn player() -> PlayerDrop {
    PlayerDrop {
        player_uuid: [4; 16],
        position: Position {
            x: 8.5,
            y: 65.,
            z: 8.5,
            yaw: 0.,
            pitch: 0.,
            on_ground: true,
        },
        eye_height: 1.62,
        around: false,
        trace_item: false,
    }
}
fn menu(events: Arc<StdMutex<Vec<&'static str>>>) -> SharedContainer {
    let container =
        SharedContainer::with_publisher(Arc::new(move |_| events.lock().unwrap().push("menu")));
    container
        .transact(|menu| {
            menu.inventory
                .set_creative_slot(36, ItemStack::new(1, 3)?)?;
            Ok(((), vec![]))
        })
        .unwrap();
    container
}
#[test]
fn readiness_failure_preserves_menu_counter_rng_stats_and_never_calls_factory() {
    let events = Arc::new(StdMutex::new(vec![]));
    let container = menu(events.clone());
    events.lock().unwrap().clear();
    let mut runtime = RuntimeEntities::new(LegacyRandomSource::new(101));
    let mut random = LegacyRandomSource::new(202);
    let stats = PlayerStats::default();
    let world_before = runtime.world_random_mut().state48();
    let player_before = random.state48();
    let ids = AtomicI32::new(300);
    let mut factories = 0;
    let result = menu_action_with_items(
        &container,
        DropCommitOwner {
            runtime: &mut runtime,
            player_random: &mut random,
            stats: stats.clone(),
            entity_counter: &ids,
        },
        player(),
        |menu| menu.drop_selected(false),
        |_| Err(ItemRuntimeNotReady.into()),
        || {
            factories += 1;
            Ok(LegacyRandomSource::new(303))
        },
        DropCommitCallbacks {
            prepare: |_: &mut PlayerContainer, next_stats| Ok(next_stats),
            before_spawn: || panic!("must not commit"),
            commit: |_: PlayerStats| panic!("must not commit"),
        },
    );
    assert!(result.is_err());
    assert_eq!(factories, 0);
    assert_eq!(ids.load(Ordering::Relaxed), 300);
    assert_eq!(
        container
            .read(|menu| menu.inventory.slot(36).unwrap().count)
            .unwrap(),
        3
    );
    assert_eq!(runtime.item_count(), 0);
    assert_eq!(runtime.world_random_mut().state48(), world_before);
    assert_eq!(random.state48(), player_before);
    assert!(stats.values().is_empty());
    assert!(events.lock().unwrap().is_empty());
}
#[test]
fn successful_selected_drop_commits_spawn_then_menu_and_retains_drop_statistics() {
    let events = Arc::new(StdMutex::new(vec![]));
    let container = menu(events.clone());
    events.lock().unwrap().clear();
    let item_events = events.clone();
    let mut runtime =
        RuntimeEntities::new(LegacyRandomSource::new(101)).with_publisher(Arc::new(move |event| {
            assert!(matches!(event, RuntimeEntityEvent::Update { .. }));
            item_events.lock().unwrap().push("spawn");
        }));
    runtime
        .set_chunk_visibility((0, 0), Visibility::Ticking)
        .unwrap();
    let mut random = LegacyRandomSource::new(202);
    let mut stats = PlayerStats::default();
    let ids = AtomicI32::new(300);
    let mut factories = 0;
    let world_before = runtime.world_random_mut().state48();
    let player_before = random.state48();
    menu_action_with_items(
        &container,
        DropCommitOwner {
            runtime: &mut runtime,
            player_random: &mut random,
            stats: stats.clone(),
            entity_counter: &ids,
        },
        player(),
        |menu| menu.drop_selected(false),
        |drops| {
            assert_eq!(drops.len(), 1);
            Ok(())
        },
        || {
            factories += 1;
            Ok(LegacyRandomSource::new(303))
        },
        DropCommitCallbacks {
            prepare: |_: &mut PlayerContainer, next_stats| Ok(next_stats),
            before_spawn: || {},
            commit: |next_stats| stats = next_stats,
        },
    )
    .unwrap();
    assert_eq!(factories, 1);
    assert_eq!(ids.load(Ordering::Relaxed), 301);
    assert_eq!(runtime.item_count(), 1);
    assert_eq!(runtime.item(300).unwrap().stack.count, 1);
    assert_eq!(runtime.item(300).unwrap().thrower, Some([4; 16]));
    assert_eq!(
        container
            .read(|menu| menu.inventory.slot(36).unwrap().count)
            .unwrap(),
        2
    );
    assert_eq!(*events.lock().unwrap(), ["spawn", "menu"]);
    assert_ne!(random.state48(), player_before);
    assert_ne!(runtime.world_random_mut().state48(), world_before);
    assert_eq!(stats.get(StatKey::new(ITEM_DROPPED, 1).unwrap()), 1);
    assert_eq!(stats.get(StatKey::new(CUSTOM, CUSTOM_DROP).unwrap()), 1);
    assert_eq!(stats.dirty_keys().len(), 2);
}
#[test]
fn invalid_second_slot_is_validated_before_any_item_factory() {
    let events = Arc::new(StdMutex::new(vec![]));
    let container = menu(events.clone());
    events.lock().unwrap().clear();
    let mut runtime = RuntimeEntities::new(LegacyRandomSource::new(1));
    let mut random = LegacyRandomSource::new(2);
    let stats = PlayerStats::default();
    let ids = AtomicI32::new(300);
    let mut factories = 0;
    let result = menu_action_with_items(
        &container,
        DropCommitOwner {
            runtime: &mut runtime,
            player_random: &mut random,
            stats: stats.clone(),
            entity_counter: &ids,
        },
        player(),
        |menu| {
            let mut outcome = menu.drop_selected(false)?;
            outcome.drops.push(DroppedStack {
                stack: ItemStack {
                    item_id: 999999,
                    count: 1,
                    ..ItemStack::empty()
                },
                around: false,
                trace_item: true,
            });
            Ok(outcome)
        },
        |_| Ok(()),
        || {
            factories += 1;
            Ok(LegacyRandomSource::new(3))
        },
        DropCommitCallbacks {
            prepare: |_: &mut PlayerContainer, next_stats| Ok(next_stats),
            before_spawn: || panic!("must not commit"),
            commit: |_: PlayerStats| panic!("must not commit"),
        },
    );
    assert!(result.is_err());
    assert_eq!(factories, 0);
    assert_eq!(runtime.item_count(), 0);
    assert_eq!(ids.load(Ordering::Relaxed), 300);
    assert_eq!(
        container
            .read(|menu| menu.inventory.slot(36).unwrap().count)
            .unwrap(),
        3
    );
    assert!(events.lock().unwrap().is_empty());
}
