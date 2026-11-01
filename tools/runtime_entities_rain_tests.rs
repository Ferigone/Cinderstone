//! Original complete rainy ItemEntity frames through the actual coordinator.
use super::original_corpus_tests::{compare_post_tick, facts, vector};
use super::*;
use crate::{
    entity::Position,
    rain_physics::{FreshFlatPlainsRain, FreshFlatPlainsRainFrame, RainChunkAvailability},
    runtime_item_world::{ItemSound, RainAvailability},
    tick::WeatherState,
    world::FlatWorld,
};
use serde_json::{json, Value};
use std::{cell::RefCell, rc::Rc};
type Trace = Rc<RefCell<Vec<Value>>>;
#[derive(Default)]
struct Host {
    events: Vec<Value>,
    observed: BTreeMap<i32, (ItemState, LegacyRandomSource)>,
    trace: Trace,
}
impl RuntimeEntityHost for Host {
    fn observe_item_tick(&mut self, s: &ItemState, r: &LegacyRandomSource) {
        self.observed.insert(s.entity_id, (s.clone(), r.clone()));
    }
    fn level_player_tick(&mut self, _: EntityHandle) -> Result<()> {
        anyhow::bail!("rain corpus has no players")
    }
    fn profile(&mut self, e: ProfileEvent) {
        self.trace
            .borrow_mut()
            .push(json!(["profile", format!("{e:?}")]));
    }
    fn effect(
        &mut self,
        s: &EntityPhysicalState,
        e: RuntimeItemEffect,
        seed: Option<i64>,
    ) -> Result<Vec<EffectDelivery>> {
        match e {
            RuntimeItemEffect::Sound {
                sound,
                volume,
                pitch,
            } => {
                assert_eq!(sound, ItemSound::GenericExtinguishFire);
                let seed = seed.expect("real sound owner seed");
                self.trace
                    .borrow_mut()
                    .push(json!(["sound", s.entity_id, seed.to_string()]));
                self.events.push(json!({"kind":"sound","name":"minecraft:entity.generic.extinguish_fire","source":8,"position":vecbits(s.position_vec()),"volume":format!("{:08x}",volume.to_bits()),"pitch":format!("{:08x}",pitch.to_bits()),"seed":seed.to_string()}));
            }
            RuntimeItemEffect::HitGround { .. } => {
                assert!(seed.is_none());
                self.events.push(json!("game_event/minecraft:hit_ground"));
            }
            RuntimeItemEffect::Base(_) => assert!(seed.is_none()),
            other => anyhow::bail!("unexpected rainy effect {other:?}"),
        };
        Ok(vec![])
    }
    fn entity_damage(
        &mut self,
        _: &EntityPhysicalState,
        _: &ItemStack,
        _: i32,
        _: Option<i32>,
    ) -> Result<()> {
        anyhow::bail!("no damage pulse in rain corpus")
    }
    fn discarded(&mut self, _: &EntityPhysicalState, _: &ItemStack, _: i32) -> Result<()> {
        anyhow::bail!("no discard in rain corpus")
    }
    fn add_to_inventory(
        &mut self,
        _: PickupPlayer,
        _: i32,
        _: i32,
        _: &mut ItemStack,
    ) -> Result<bool> {
        anyhow::bail!("no pickup")
    }
    fn collect_item(&mut self, _: PickupPlayer, _: &ItemState, _: i32, _: i32) -> Result<()> {
        anyhow::bail!("no pickup")
    }
    fn collected(&mut self, _: PickupPlayer, _: &ItemState, _: Pickup, _: i32) -> Result<()> {
        anyhow::bail!("no pickup")
    }
}
fn vecbits(v: Vec3) -> Vec<String> {
    [v.x, v.y, v.z]
        .into_iter()
        .map(|v| format!("{:016x}", v.to_bits()))
        .collect()
}
struct Chunks;
impl RainChunkAvailability for Chunks {
    fn require_settled_unedited_fixture(&self) -> Result<()> {
        Ok(())
    }
    fn require_current_full(&self, p: (i32, i32)) -> Result<()> {
        anyhow::ensure!(
            (-8..=8).contains(&p.0) && (-8..=8).contains(&p.1),
            "outside controlled current FULL range"
        );
        Ok(())
    }
}
struct Exposure<'a> {
    frame: FreshFlatPlainsRainFrame<'a>,
    trace: Trace,
}
impl RainExposure for Exposure<'_> {
    fn is_in_rain(&mut self, w: &mut RuntimeWorld, s: &EntityPhysicalState) -> Result<bool> {
        self.trace.borrow_mut().push(json!(["rain", s.entity_id]));
        self.frame.is_in_rain(w, s)
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/item-rain-vectors.json")).unwrap()
}
fn init(q: &Value) -> ItemState {
    let p = vector(&q["position"]);
    let mut s = ItemState::new(
        q["entity_id"].as_i64().unwrap() as i32,
        [1; 16],
        Position {
            x: p.x,
            y: p.y,
            z: p.z,
            yaw: 0.,
            pitch: 0.,
            on_ground: q["on_ground"].as_bool().unwrap(),
        },
        vector(&q["velocity"]),
        ItemStack::new(1, 1).unwrap(),
    )
    .unwrap();
    s.tick_count = q["tick_count"].as_i64().unwrap() as i32;
    s.age = q["age"].as_i64().unwrap() as i32;
    s.pickup_delay = q["pickup_delay"].as_i64().unwrap() as i32;
    s.fall_distance = f32bits(&q["fall_distance"]);
    s.remaining_fire_ticks = q["fire_ticks"].as_i64().unwrap() as i32;
    s.set_no_gravity(q["no_gravity"].as_bool().unwrap());
    if q["drain_initial_metadata"].as_bool().unwrap() {
        s.take_metadata_dirty();
    }
    s
}
fn f32bits(v: &Value) -> f32 {
    f32::from_bits(u32::from_str_radix(v.as_str().unwrap(), 16).unwrap())
}
fn runtime(q: &Value, sound: bool) -> RuntimeEntities {
    let mut r = RuntimeEntities::new(LegacyRandomSource::new(12345));
    if sound {
        r = r.with_sound_random(LegacyRandomSource::new(67890));
    }
    for x in -8..=8 {
        for z in -8..=8 {
            r.set_chunk_visibility((x, z), Visibility::Ticking).unwrap();
        }
    }
    r.insert_item(
        init(q),
        LegacyRandomSource::new(q["seed"].as_i64().unwrap()),
    )
    .unwrap();
    r
}
#[test]
fn actual_complete_rain_ticks_preserve_physical_state_effect_order_and_three_rng_streams() {
    let f = fixture();
    let mut count = 0;
    let mut sounds = 0;
    for (scene, s) in f["scenes"].as_array().unwrap().iter().enumerate() {
        let q = &s["query"];
        assert!(q["blocks"].as_array().unwrap().is_empty());
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        let mut r = runtime(q, true);
        let mut owner = FreshFlatPlainsRain::from_observed_empty_temperature_history();
        let mut host = Host::default();
        let mut exposure = Exposure {
            frame: owner.frame(&Chunks),
            trace: Rc::clone(&host.trace),
        };
        let mut weather = WeatherState {
            old_rain_level: f32bits(&q["weather"]["old_rain"]),
            rain_level: f32bits(&q["weather"]["rain"]),
            ..WeatherState::default()
        };
        let id = q["entity_id"].as_i64().unwrap() as i32;
        for (tick, expected) in s["expected"]["states"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            for change in q["weather_changes"].as_array().unwrap() {
                if change["tick"].as_u64().unwrap() as usize == tick {
                    weather.old_rain_level = f32bits(&change["old_rain"]);
                    weather.rain_level = f32bits(&change["rain"]);
                }
            }
            if q["clear_has_impulse"].as_bool().unwrap() {
                r.items.get_mut(&id).unwrap().state.has_impulse = false;
            }
            host.events.clear();
            let mut facts = facts(tick);
            facts.rain = if weather.is_raining() {
                RainAvailability::ExposureUnavailable
            } else {
                RainAvailability::NotRaining
            };
            r.simulate_level_with_context(
                &mut world,
                facts,
                &[],
                None,
                Some(&mut exposure),
                &mut host,
            )
            .unwrap_or_else(|e| panic!("rain scene{scene} tick{tick}: {e:#}"));
            assert_eq!(
                host.events,
                expected["events"].as_array().unwrap().clone(),
                "rain effects scene{scene} tick{tick}"
            );
            sounds += host.events.iter().filter(|v| v["kind"] == "sound").count();
            let (mut state, rng) = host.observed.remove(&id).unwrap();
            assert_eq!(rng.state48().to_string(), expected["entity_rng_state"]);
            assert_eq!(
                r.world_random.state48().to_string(),
                expected["world_rng_state"]
            );
            assert_eq!(
                r.sound_random_state48().unwrap().to_string(),
                expected["sound_rng_state"]
            );
            let mut physical = expected.clone();
            for k in ["entity_rng_state", "world_rng_state", "sound_rng_state"] {
                physical.as_object_mut().unwrap().remove(k);
            }
            physical["events"] = json!([]);
            compare_post_tick(&mut state, &[], &physical, scene, tick);
            r.items.get_mut(&id).unwrap().state.take_metadata_dirty();
            count += 1;
        }
        assert_eq!(
            r.items.get_mut(&id).unwrap().random.next_i32(),
            s["expected"]["rng_next_int"]
        );
        assert_eq!(
            r.world_random.next_i32(),
            s["expected"]["world_rng_next_int"]
        );
        assert_eq!(
            r.sound_random.as_mut().unwrap().next_i32(),
            s["expected"]["sound_rng_next_int"]
        );
    }
    assert_eq!((count, sounds), (80, 6));
    println!("RUNTIME_ENTITIES_RAIN_CONFORMANCE:80 original states,6 sounds,3 independent RNG streams; zero differences");
}
#[test]
fn two_items_share_contexts_and_actual_sound_stream_without_world_draws() {
    let f = fixture();
    let q = &f["scenes"][0]["query"];
    let mut r = runtime(q, true);
    let mut second = init(q);
    second.entity_id += 1;
    second.uuid = [2; 16];
    second.set_position(Vec3::new(12.5, 65., 8.5));
    r.insert_item(second, LegacyRandomSource::new(712102))
        .unwrap();
    let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
    let mut host = Host::default();
    let mut owner = FreshFlatPlainsRain::from_observed_empty_temperature_history();
    let mut exposure = Exposure {
        frame: owner.frame(&Chunks),
        trace: Rc::clone(&host.trace),
    };
    let mut cache = FluidShapeCache::unknown();
    let mut facts = facts(0);
    facts.rain = RainAvailability::ExposureUnavailable;
    r.simulate_level_with_context(
        &mut world,
        facts,
        &[],
        Some(&mut cache),
        Some(&mut exposure),
        &mut host,
    )
    .unwrap();
    let sounds = host
        .trace
        .borrow()
        .iter()
        .filter(|v| v[0] == "sound")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(sounds.len(), 2);
    for (index, sound) in sounds.iter().enumerate() {
        assert_eq!(
            sound[2],
            f["actual_dzr_draws_before_nextLong_after"][index][1]
        );
    }
    let ordered = host
        .trace
        .borrow()
        .iter()
        .filter(|v| v[0] == "rain" || v[0] == "sound")
        .map(|v| json!([v[0], v[1]]))
        .collect::<Vec<_>>();
    assert_eq!(
        ordered,
        json!([
            ["rain", 400],
            ["sound", 400],
            ["rain", 400],
            ["rain", 401],
            ["sound", 401],
            ["rain", 401]
        ])
        .as_array()
        .unwrap()
        .clone()
    );
    assert_eq!(
        r.sound_random_state48().unwrap().to_string(),
        f["actual_dzr_draws_before_nextLong_after"][1][2]
    );
    assert_eq!(
        r.world_random.state48(),
        LegacyRandomSource::new(12345).state48()
    );
    for id in [400, 401] {
        assert_eq!(r.item(id).unwrap().remaining_fire_ticks, -1);
        assert!(host
            .trace
            .borrow()
            .iter()
            .any(|v| v[0] == "rain" && v[1] == id));
    }
    // Reborrow the same capabilities again; this neither resets nor duplicates
    // their owners. Cache-dependent water-ray history is separately proved.
    r.simulate_level_with_context(
        &mut world,
        facts,
        &[],
        Some(&mut cache),
        Some(&mut exposure),
        &mut host,
    )
    .unwrap();
    assert_eq!(
        host.events.iter().filter(|v| v["kind"] == "sound").count(),
        2
    );
}
#[test]
fn missing_rain_or_sound_owner_fails_without_borrowing_world_rng_or_emitting_sound() {
    let f = fixture();
    let q = &f["scenes"][0]["query"];
    for has_rain in [false, true] {
        let mut r = runtime(q, false);
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        let mut host = Host::default();
        let mut owner = FreshFlatPlainsRain::from_observed_empty_temperature_history();
        let mut exposure = owner.frame(&Chunks);
        let mut facts = facts(0);
        facts.rain = RainAvailability::ExposureUnavailable;
        let error = r
            .simulate_level_with_context(
                &mut world,
                facts,
                &[],
                None,
                if has_rain { Some(&mut exposure) } else { None },
                &mut host,
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(if has_rain {
                "threadSafeRandom"
            } else {
                "rain exposure"
            }),
            "{error}"
        );
        assert_eq!(
            r.world_random.state48(),
            LegacyRandomSource::new(12345).state48()
        );
        assert!(r.sound_random_state48().is_none());
        assert!(host.events.iter().all(|v| v["kind"] != "sound"));
        assert_eq!(r.item(400).unwrap().position.y, 64.96);
    }
}

/// Controlled rain input over two edited water-ray sites. This checks the
/// composition seam, not genuine-source rain certification for edited worlds.
#[test]
fn fluid_shape_history_is_reused_between_items_while_rain_context_is_present() {
    struct ControlledRain(usize);
    impl RainExposure for ControlledRain {
        fn is_in_rain(&mut self, _: &mut RuntimeWorld, _: &EntityPhysicalState) -> Result<bool> {
            self.0 += 1;
            Ok(true)
        }
    }
    let f = fixture();
    let q = &f["scenes"][0]["query"];
    let mut r = RuntimeEntities::new(LegacyRandomSource::new(12345))
        .with_sound_random(LegacyRandomSource::new(67890));
    for x in -1..=1 {
        r.set_chunk_visibility((x, 0), Visibility::Ticking).unwrap();
    }
    for (id, z) in [(400, 0.5), (401, 4.5)] {
        let mut item = init(q);
        item.entity_id = id;
        item.uuid = [id as u8; 16];
        item.set_position(Vec3::new(-0.25, 65.94, z));
        item.set_no_gravity(true);
        item.velocity = Vec3::new(1.5, 0., 0.);
        item.fall_distance = 4.;
        r.insert_item(item, LegacyRandomSource::new(i64::from(id)))
            .unwrap();
    }
    let water: Value =
        serde_json::from_str(include_str!("../fixtures/fluid-shape-vectors.json")).unwrap();
    let water = water["aliases"]["water_source_block_state"]
        .as_u64()
        .unwrap() as u32;
    let mut flat = FlatWorld::new(0, 1, 10, 9, 39);
    flat.set_block(0, 65, 0, water);
    flat.set_block(0, 65, 4, water);
    flat.set_block(0, 66, 4, water);
    let mut world = RuntimeWorld::Fixture(flat);
    let mut cache =
        FluidShapeCache::from_observed_empty_owners(crate::fluid_physics::FluidShapeOwner::ALL);
    let mut rain = ControlledRain(0);
    let mut host = Host::default();
    let mut facts = facts(0);
    facts.rain = RainAvailability::ExposureUnavailable;
    r.simulate_level_with_context(
        &mut world,
        facts,
        &[],
        Some(&mut cache),
        Some(&mut rain),
        &mut host,
    )
    .unwrap();
    assert_eq!(rain.0, 4);
    assert_eq!(
        cache
            .observations()
            .filter(|(_, history)| matches!(
                history,
                crate::fluid_physics::FluidShapeHistory::CachedHeight(_)
            ))
            .count(),
        1,
        "one canonical source-water shape shared by both items"
    );
    for id in [400, 401] {
        assert_eq!(
            r.item(id).unwrap().fall_distance,
            4.,
            "second above-water site must reuse first shallow shape, so neither ray hits"
        );
        assert_eq!(r.item(id).unwrap().remaining_fire_ticks, -1);
    }
    assert_eq!(
        host.events
            .iter()
            .filter(|event| event["kind"] == "sound")
            .count(),
        2
    );
    assert_eq!(
        r.world_random.state48(),
        LegacyRandomSource::new(12345).state48()
    );
}
