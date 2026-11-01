use super::*;
use mc121_rust::rain_physics::RainChunkAvailability;
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(include_str!("../fixtures/item-rain-vectors.json")).unwrap()
}
fn f64bits(value: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}
fn f32bits(value: &Value) -> f32 {
    f32::from_bits(u32::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}
fn vector(value: &Value) -> Vec3 {
    Vec3::new(f64bits(&value[0]), f64bits(&value[1]), f64bits(&value[2]))
}
fn vecbits(value: Vec3) -> Value {
    json!([value.x, value.y, value.z].map(|n| format!("{:016x}", n.to_bits())))
}
fn setup(
    query: &Value,
) -> (
    LivePlayers,
    WorldBackend,
    broadcast::Sender<PlayerEvent>,
    broadcast::Receiver<PlayerEvent>,
) {
    let weather = WeatherState {
        old_rain_level: f32bits(&query["weather"]["old_rain"]),
        rain_level: f32bits(&query["weather"]["rain"]),
        ..WeatherState::default()
    };
    let (sender, receiver) = broadcast::channel(64);
    let mut players = LivePlayers::new(
        LegacyRandomSource::new(12345),
        LegacyRandomSource::new(67890),
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
            weather,
        ),
        2,
        true,
        sender.clone(),
    );
    let p = vector(&query["position"]);
    let mut item = ItemState::new(
        query["entity_id"].as_i64().unwrap() as i32,
        [1; 16],
        Position {
            x: p.x,
            y: p.y,
            z: p.z,
            yaw: 0.,
            pitch: 0.,
            on_ground: query["on_ground"].as_bool().unwrap(),
        },
        vector(&query["velocity"]),
        ItemStack::new(1, 1).unwrap(),
    )
    .unwrap();
    item.tick_count = query["tick_count"].as_i64().unwrap() as i32;
    item.age = query["age"].as_i64().unwrap() as i32;
    item.pickup_delay = query["pickup_delay"].as_i64().unwrap() as i32;
    item.fall_distance = f32bits(&query["fall_distance"]);
    item.remaining_fire_ticks = query["fire_ticks"].as_i64().unwrap() as i32;
    item.set_no_gravity(query["no_gravity"].as_bool().unwrap());
    if query["drain_initial_metadata"].as_bool().unwrap() {
        item.take_metadata_dirty();
    }
    if query["clear_has_impulse"].as_bool().unwrap() {
        item.has_impulse = false;
    }
    players
        .item_runtime
        .insert_item(
            item,
            LegacyRandomSource::new(query["seed"].as_i64().unwrap()),
        )
        .unwrap();
    let world = WorldBackend::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
    // Production item-world simulation requires the complete native item
    // source. Install the real fresh-world owner (neither storage file exists)
    // instead of leaving it absent.
    let directory = std::env::temp_dir().join(format!(
        "mc121-rain-context-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = EntityDataStore::new(directory.join("entities.json"));
    players.entity_source = Some(
        store
            .fresh_source(&directory.join("world.json"), &world)
            .unwrap(),
    );
    (players, world, sender, receiver)
}
fn settle(
    players: &mut LivePlayers,
    world: &mut WorldBackend,
    sender: &broadcast::Sender<PlayerEvent>,
) {
    players
        .chunk_runtime
        .manager
        .update_chunk_forced((0, 0), true)
        .unwrap();
    let summary = players
        .chunk_runtime
        .poll(
            world,
            &mut players.item_runtime,
            &mut players.chunk_transports,
            sender,
            1,
        )
        .unwrap();
    assert!(summary.quiescent);
    assert!(summary.visibility_updates > 0);
    players
        .chunk_runtime
        .require_settled_unedited_fixture()
        .unwrap();
    players
        .chunk_runtime
        .manager
        .certify_full_region((-2, -2), (2, 2))
        .unwrap();
    assert_eq!(players.item_runtime.ticking_item_states().count(), 1);
}
fn compare_physical(actual: &ItemState, expected: &Value) {
    let b = actual.bounds;
    let result = json!({
        "position":vecbits(actual.position_vec()),"velocity":vecbits(actual.velocity),
        "bounds":([b.min_x,b.min_y,b.min_z,b.max_x,b.max_y,b.max_z].map(|n|format!("{:016x}",n.to_bits()))),
        "on_ground":actual.on_ground(),"horizontal_collision":actual.horizontal_collision,"vertical_collision":actual.vertical_collision,
        "vertical_collision_below":actual.vertical_collision_below,"minor_horizontal_collision":actual.minor_horizontal_collision,
        "no_physics":actual.no_physics,"has_impulse":actual.has_impulse,"first_tick":actual.first_tick,
        "touching_water":actual.was_touching_water,"was_eye_in_water":actual.was_eye_in_water,"on_ground_no_blocks":actual.on_ground_no_blocks,
        "fire_ticks":actual.remaining_fire_ticks,"tick_count":actual.tick_count,"age":actual.age,"pickup_delay":actual.pickup_delay,
        "health":actual.health,"fall_distance":format!("{:08x}",actual.fall_distance.to_bits()),"removed":actual.removed,
        "support":actual.supporting_block,"fluid_depths":actual.fluid_heights.map(|n|format!("{:016x}",n.to_bits())),
        "fluid_on_eyes":actual.fluid_on_eyes,"flags":actual.shared_flags,"dirty_mask":actual.clone().take_metadata_dirty(),
    });
    let mut wanted = expected.clone();
    for field in [
        "events",
        "entity_rng_state",
        "world_rng_state",
        "sound_rng_state",
    ] {
        wanted.as_object_mut().unwrap().remove(field);
    }
    assert_eq!(result, wanted, "actual original first-tick physical state");
}

#[test]
fn main_wet_and_clear_fire10_tick_matches_original_state_sound_and_independent_rngs() {
    let fixture = corpus();
    let mut cases = 0;
    let mut sounds = 0;
    for scene in fixture["scenes"].as_array().unwrap() {
        let query = &scene["query"];
        if !matches!(
            query["name"].as_str().unwrap(),
            "wet_fire10" | "clear_fire10"
        ) {
            continue;
        }
        let expected = &scene["expected"]["states"][0];
        let (mut players, mut world, sender, mut receiver) = setup(query);
        assert!(players.registry.snapshot().is_empty());
        settle(&mut players, &mut world, &sender);
        while receiver.try_recv().is_ok() {}
        let id = query["entity_id"].as_i64().unwrap() as i32;
        let world_before = players.item_runtime.world_random_mut().state48();
        players
            .simulate_item_world(&mut world, 1, CollisionBorder::default(), &sender)
            .unwrap();
        compare_physical(players.item_runtime.item(id).unwrap(), expected);
        assert_eq!(
            players.item_runtime.world_random_mut().state48(),
            world_before
        );
        assert_eq!(world_before.to_string(), expected["world_rng_state"]);
        assert_eq!(
            players
                .item_runtime
                .sound_random_state48()
                .unwrap()
                .to_string(),
            expected["sound_rng_state"]
        );
        let mut deliveries = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            if let PlayerEvent::Item(event) = event {
                if let RuntimeEntityEvent::Effect {
                    deliveries: batch, ..
                } = &*event
                {
                    deliveries.extend(batch.clone());
                }
            }
        }
        assert_eq!(
            deliveries.len(),
            expected["events"].as_array().unwrap().len()
        );
        for (delivery, golden) in deliveries
            .iter()
            .zip(expected["events"].as_array().unwrap())
        {
            // Even without recipients, the actual f stream must advance.
            assert!(delivery.recipients.is_empty());
            assert_eq!(delivery.packet.id, 0x68);
            let mut r = Reader::new(&delivery.packet.payload);
            assert_eq!(r.varint().unwrap(), 578);
            assert_eq!(r.varint().unwrap(), golden["source"]);
            for coordinate in golden["position"].as_array().unwrap() {
                assert_eq!(r.i32().unwrap(), (f64bits(coordinate) * 8.) as i32);
            }
            assert_eq!(
                r.f32().unwrap().to_bits(),
                f32bits(&golden["volume"]).to_bits()
            );
            assert_eq!(
                r.f32().unwrap().to_bits(),
                f32bits(&golden["pitch"]).to_bits()
            );
            assert_eq!(r.i64().unwrap().to_string(), golden["seed"]);
            r.finish().unwrap();
            sounds += 1;
        }
        cases += 1;
    }
    assert_eq!((cases, sounds), (2, 1));
    println!("MAIN_RAIN_CONTEXT_CONFORMANCE:2 actual original first-tick states,1 sound seed,-3561856981251203394,Level.random unchanged,no audience");
}

#[test]
fn main_rain_requires_actual_owner_and_persistent_settled_source_before_tick() {
    let fixture = corpus();
    let query = &fixture["scenes"][0]["query"];
    let (mut players, mut world, sender, _) = setup(query);
    settle(&mut players, &mut world, &sender);
    let id = query["entity_id"].as_i64().unwrap() as i32;
    let original = players.item_runtime.item(id).unwrap().clone();
    let world_state = players.item_runtime.world_random_mut().state48();
    let sound_state = players.item_runtime.sound_random_state48();
    players.rain_context = None;
    assert!(players
        .simulate_item_world(&mut world, 1, CollisionBorder::default(), &sender)
        .is_err());
    assert_eq!(
        players.item_runtime.item(id).unwrap().tick_count,
        original.tick_count
    );
    assert_eq!(
        players.item_runtime.item(id).unwrap().remaining_fire_ticks,
        10
    );
    players.rain_context = Some(
        mc121_rust::rain_physics::FreshFlatPlainsRain::from_observed_empty_temperature_history(),
    );
    players.chunk_runtime.invalidate_lighting();
    assert!(players
        .chunk_runtime
        .require_settled_unedited_fixture()
        .is_err());
    assert!(players
        .simulate_item_world(&mut world, 1, CollisionBorder::default(), &sender)
        .is_err());
    assert_eq!(
        players.item_runtime.item(id).unwrap().tick_count,
        original.tick_count
    );
    assert_eq!(
        players.item_runtime.world_random_mut().state48(),
        world_state
    );
    assert_eq!(players.item_runtime.sound_random_state48(), sound_state);
    assert!(players
        .chunk_runtime
        .require_current_full((99, 99))
        .is_err());
}
