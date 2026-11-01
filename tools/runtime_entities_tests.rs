//! Original full-tick corpus through the real storage/index/tick coordinator.
use super::*;
use crate::{
    entity::Position,
    entity_spatial,
    runtime_collision::CollisionBorder,
    runtime_item_world::RainAvailability,
    world::{FlatWorld, MIN_Y},
};
use serde_json::{json, Value};

#[derive(Default)]
struct CorpusHost {
    events: Vec<String>,
    observed: Option<(ItemState, LegacyRandomSource)>,
    observations: usize,
    removals: usize,
}
impl RuntimeEntityHost for CorpusHost {
    fn observe_item_tick(&mut self, state: &ItemState, random: &LegacyRandomSource) {
        assert!(
            self.observed.is_none(),
            "one original item tick per fixture frame"
        );
        self.observed = Some((state.clone(), random.clone()));
        self.observations += 1;
    }
    fn level_player_tick(&mut self, _: EntityHandle) -> Result<()> {
        anyhow::bail!("single-item original corpus has no player ticks")
    }
    fn profile(&mut self, _: ProfileEvent) {}
    fn effect(
        &mut self,
        _: &EntityPhysicalState,
        effect: RuntimeItemEffect,
        sound_seed: Option<i64>,
    ) -> Result<Vec<EffectDelivery>> {
        match effect {
            RuntimeItemEffect::HitGround { .. } => {
                self.events.push("game_event/minecraft:hit_ground".into())
            }
            RuntimeItemEffect::Splash => self.events.push("game_event/minecraft:splash".into()),
            RuntimeItemEffect::Sound { .. } => {
                anyhow::bail!("persistently dry/submerged original corpus has no sound callbacks")
            }
            RuntimeItemEffect::Base(_) => {}
        }
        assert!(sound_seed.is_none());
        Ok(vec![])
    }
    fn entity_damage(
        &mut self,
        _: &EntityPhysicalState,
        _: &ItemStack,
        _: i32,
        _: Option<i32>,
    ) -> Result<()> {
        anyhow::bail!("original corpus has no damage")
    }
    fn discarded(&mut self, state: &EntityPhysicalState, _: &ItemStack, _: i32) -> Result<()> {
        assert!(state.removed);
        self.removals += 1;
        Ok(())
    }
    fn add_to_inventory(
        &mut self,
        _: PickupPlayer,
        _: i32,
        _: i32,
        _: &mut ItemStack,
    ) -> Result<bool> {
        anyhow::bail!("original corpus has no inventory add")
    }
    fn collect_item(&mut self, _: PickupPlayer, _: &ItemState, _: i32, _: i32) -> Result<()> {
        anyhow::bail!("original corpus has no collection")
    }
    fn collected(&mut self, _: PickupPlayer, _: &ItemState, _: Pickup, _: i32) -> Result<()> {
        anyhow::bail!("original corpus has no collection")
    }
}
pub(super) fn db(value: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}
pub(super) fn vector(value: &Value) -> Vec3 {
    Vec3::new(db(&value[0]), db(&value[1]), db(&value[2]))
}
fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}
fn output_vector(v: Vec3) -> Value {
    json!([bits(v.x), bits(v.y), bits(v.z)])
}
pub(super) fn facts(tick: usize) -> ItemWorldFacts {
    ItemWorldFacts {
        game_time: tick as i64 + 1,
        minimum_build_height: MIN_Y,
        ultrawarm: false,
        border: CollisionBorder::default(),
        rain: RainAvailability::NotRaining,
        has_passengers: false,
    }
}
pub(super) fn compare_post_tick(
    item: &mut ItemState,
    events: &[String],
    expected: &Value,
    scene: usize,
    tick: usize,
) {
    let dirty = item.take_metadata_dirty();
    let b = item.bounds;
    let mut actual = json!({
        "position":output_vector(item.position_vec()),"velocity":output_vector(item.velocity),
        "bounds":[bits(b.min_x),bits(b.min_y),bits(b.min_z),bits(b.max_x),bits(b.max_y),bits(b.max_z)],
        "on_ground":item.on_ground(),"horizontal_collision":item.horizontal_collision,"vertical_collision":item.vertical_collision,"vertical_collision_below":item.vertical_collision_below,"minor_horizontal_collision":item.minor_horizontal_collision,
        "no_physics":item.no_physics,"has_impulse":item.has_impulse,"first_tick":item.first_tick,"touching_water":item.was_touching_water,"was_eye_in_water":item.was_eye_in_water,"on_ground_no_blocks":item.on_ground_no_blocks,
        "fire_ticks":item.remaining_fire_ticks,"tick_count":item.tick_count,"age":item.age,"pickup_delay":item.pickup_delay,"health":item.health,"fall_distance":format!("{:08x}",item.fall_distance.to_bits()),"removed":item.removed,"support":item.supporting_block,
        "fluid_depths":[bits(item.fluid_heights[0]),bits(item.fluid_heights[1])],"fluid_on_eyes":item.fluid_on_eyes,"flags":item.shared_flags,"dirty_mask":dirty,"events":events,
    });
    if expected.get("support").is_none() {
        assert!(item.supporting_block.is_none());
        actual.as_object_mut().unwrap().remove("support");
    }
    assert_eq!(
        actual, *expected,
        "original coordinator scene{scene}tick{tick}"
    );
}
fn replay(vectors: &Value) -> (usize, usize, usize) {
    let mut frames = 0;
    let mut removals = 0;
    let mut rngs = 0;
    for (scene_index, scene) in vectors["scenes"].as_array().unwrap().iter().enumerate() {
        let q = &scene["query"];
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        for block in q["blocks"].as_array().unwrap() {
            if let RuntimeWorld::Fixture(flat) = &mut world {
                flat.set_block(
                    block[0].as_i64().unwrap() as i32,
                    block[1].as_i64().unwrap() as i32,
                    block[2].as_i64().unwrap() as i32,
                    block[3].as_u64().unwrap() as u32,
                );
            }
        }
        let p = vector(&q["position"]);
        let id = q["entity_id"].as_i64().unwrap() as i32;
        let mut item = ItemState::new(
            id,
            [1; 16],
            Position {
                x: p.x,
                y: p.y,
                z: p.z,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: q["on_ground"].as_bool().unwrap(),
            },
            vector(&q["velocity"]),
            ItemStack::new(1, 1).unwrap(),
        )
        .unwrap();
        item.tick_count = q["tick_count"].as_i64().unwrap() as i32;
        item.age = q["age"].as_i64().unwrap() as i32;
        item.pickup_delay = q["pickup_delay"].as_i64().unwrap() as i32;
        item.fall_distance =
            f32::from_bits(u32::from_str_radix(q["fall_distance"].as_str().unwrap(), 16).unwrap());
        item.set_no_gravity(q["no_gravity"].as_bool().unwrap());
        if q["drain_initial_metadata"].as_bool().unwrap_or(true) {
            item.take_metadata_dirty();
        }
        let mut runtime = RuntimeEntities::new(LegacyRandomSource::new(12345));
        // Explicit ticket facts cover all fixture positions; loading a network
        // chunk is not used as a substitute for entity ticking visibility.
        for x in -8..=8 {
            for z in -8..=8 {
                runtime
                    .set_chunk_visibility((x, z), Visibility::Ticking)
                    .unwrap();
            }
        }
        runtime
            .insert_item(item, LegacyRandomSource::new(q["seed"].as_i64().unwrap()))
            .unwrap();
        let mut host = CorpusHost::default();
        let mut final_random = None;
        for (tick, expected) in scene["states"].as_array().unwrap().iter().enumerate() {
            let current = &mut runtime
                .items
                .get_mut(&id)
                .expect("live original item before tick")
                .state;
            if q["clear_has_impulse"].as_bool().unwrap_or(true) {
                current.has_impulse = false;
            }
            host.events.clear();
            runtime
                .simulate_level(&mut world, facts(tick), &[], &mut host)
                .unwrap_or_else(|error| {
                    panic!("coordinator scene{scene_index}tick{tick}: {error:#}")
                });
            let (mut observed, random) = host
                .observed
                .take()
                .expect("coordinator must expose completed original tick even after removal");
            let dirty = observed.metadata_dirty;
            compare_post_tick(&mut observed, &host.events, expected, scene_index, tick);
            final_random = Some(random);
            let removed = expected["removed"].as_bool().unwrap();
            assert_eq!(
                runtime.index.by_id(id).is_none(),
                removed,
                "index removal scene{scene_index}tick{tick}"
            );
            assert_eq!(
                runtime.ticks.contains(id),
                !removed,
                "ticklist removal scene{scene_index}tick{tick}"
            );
            if let Some(entry) = runtime.items.get_mut(&id) {
                assert!(!removed);
                assert_eq!(
                    entry.state.take_metadata_dirty(),
                    dirty,
                    "oracle packDirty drain"
                );
                assert_eq!(
                    runtime.index.get(entry.handle).unwrap().section,
                    entity_spatial::section_at(entry.state.position_vec()),
                    "immediate original section movement"
                );
            } else {
                assert!(removed);
                removals += 1;
            }
            frames += 1;
        }
        assert_eq!(
            final_random.unwrap().next_i32(),
            scene["rng_next_int"].as_i64().unwrap() as i32,
            "coordinator final RNG scene{scene_index}"
        );
        assert_eq!(host.observations, scene["states"].as_array().unwrap().len());
        assert_eq!(
            host.removals,
            usize::from(
                scene["states"].as_array().unwrap().last().unwrap()["removed"]
                    .as_bool()
                    .unwrap()
            )
        );
        rngs += 1;
    }
    (frames, removals, rngs)
}
#[test]
fn original_full_item_corpus_through_runtime_coordinator() {
    let vectors: Value =
        serde_json::from_str(include_str!("../fixtures/item-full-vectors.json")).unwrap();
    let (frames, removed, rngs) = replay(&vectors);
    assert_eq!((frames, removed, rngs), (2616, 30, 64));
    println!(
        "RUNTIME_ENTITIES_DRY_CONFORMANCE:{}",
        json!({"ok":true,"scenes":64,"exact_post_tick_states":frames,"exact_removal_ticks":removed,"exact_final_rng_states":rngs,"mismatches":0})
    );
}
#[test]
fn original_submerged_item_corpus_through_runtime_coordinator() {
    let vectors: Value =
        serde_json::from_str(include_str!("../fixtures/item-water-vectors.json")).unwrap();
    let (frames, removed, rngs) = replay(&vectors);
    assert_eq!((frames, removed, rngs), (360, 0, 18));
    println!(
        "RUNTIME_ENTITIES_WATER_CONFORMANCE:{}",
        json!({"ok":true,"scenes":18,"exact_post_tick_states":frames,"exact_removal_ticks":removed,"exact_final_rng_states":rngs,"mismatches":0})
    );
}
