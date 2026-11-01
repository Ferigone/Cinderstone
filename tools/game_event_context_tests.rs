use super::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;

struct World {
    loaded: String,
    registry: String,
    override_state: Option<RegistryState>,
    trace: Vec<Value>,
    chunks: BTreeSet<(i32, i32)>,
    sections: BTreeSet<(i32, i32, i32)>,
}
impl World {
    fn new(loaded: &str, registry: &str) -> Self {
        Self {
            loaded: loaded.into(),
            registry: registry.into(),
            override_state: None,
            trace: Vec::new(),
            chunks: BTreeSet::new(),
            sections: BTreeSet::new(),
        }
    }
}
impl GameEventRegistryWorld for World {
    type Chunk = (i32, i32);
    fn get_loaded_chunk(&mut self, x: i32, z: i32) -> Result<Option<Self::Chunk>> {
        let present = match self.loaded.as_str() {
            "all" => true,
            "none" => false,
            "checker" => x.wrapping_mul(31).wrapping_add(z.wrapping_mul(17)) & 1 == 0,
            _ => panic!("unknown actual chunk source mode"),
        };
        self.trace.push(json!(["source"]));
        self.trace.push(json!(["chunk", x, z, present]));
        if present {
            self.chunks.insert((x, z));
            Ok(Some((x, z)))
        } else {
            Ok(None)
        }
    }
    fn registry_state(&mut self, (x, z): Self::Chunk, y: i32) -> Result<RegistryState> {
        self.trace.push(json!(["registry", x, y, z, self.registry]));
        if let Some(state) = self.override_state {
            return Ok(state);
        }
        Ok(if self.registry == "noop" {
            RegistryState::Noop
        } else {
            self.sections.insert((x, y, z));
            RegistryState::Euclidean {
                listeners: 0,
                pending_add: 0,
                pending_remove: 0,
                processing: false,
            }
        })
    }
}
fn fixture(name: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), name)).unwrap(),
    )
    .unwrap()
}

#[test]
fn original_empty_dispatch_getter_trace_registry_creation_and_rng() {
    let cases = fixture("fixtures/game_event_context.json");
    let mut calls = 0;
    let mut visits = 0;
    assert_eq!(cases["cases"].as_array().unwrap().len(), 840);
    for case in cases["cases"].as_array().unwrap() {
        let query = &case["query"];
        let expected = &case["expected"];
        let bits = |axis: usize| {
            f64::from_bits(
                u64::from_str_radix(query["position_bits"][axis].as_str().unwrap(), 16).unwrap(),
            )
        };
        let mut world = World::new(
            query["loaded"].as_str().unwrap(),
            query["registry"].as_str().unwrap(),
        );
        let post = GameEventPost {
            position: Vec3::new(bits(0), bits(1), bits(2)),
            notification_radius: query["radius"].as_i64().unwrap() as i32,
        };
        let result = post_to_empty_registries(&mut world, post).unwrap();
        assert_eq!(
            world.trace,
            *expected["trace"].as_array().unwrap(),
            "getter trace {}",
            case["name"]
        );
        assert_eq!(
            result.chunk_queries,
            world.trace.iter().filter(|call| call[0] == "chunk").count()
        );
        assert_eq!(
            result.loaded_chunks,
            expected["loaded_chunks"].as_u64().unwrap() as usize
        );
        assert_eq!(
            result.section_visits,
            world
                .trace
                .iter()
                .filter(|call| call[0] == "registry")
                .count()
        );
        assert_eq!(world.chunks.len(), result.loaded_chunks);
        assert_eq!(
            world.sections.len(),
            expected["materialized_registries"].as_u64().unwrap() as usize
        );
        let actual_sections: BTreeSet<_> = expected["registry_states"]
            .as_array()
            .unwrap()
            .iter()
            .map(|state| {
                assert_eq!(state["active"], 0);
                assert_eq!(state["pending_add"], 0);
                assert_eq!(state["pending_remove"], 0);
                assert_eq!(state["processing"], false);
                (
                    state["x"].as_i64().unwrap() as i32,
                    state["y"].as_i64().unwrap() as i32,
                    state["z"].as_i64().unwrap() as i32,
                )
            })
            .collect();
        assert_eq!(world.sections, actual_sections);
        assert_eq!(expected["world_rng_before"], expected["world_rng_after"]);
        calls += world.trace.len();
        visits += result.section_visits;
    }
    assert_eq!(calls, 22900);
    assert_eq!(visits, 11478);
    println!("actual empty GameEventDispatcher: 840 posts, {calls} getter calls, {visits} registry visits");
}

#[test]
fn nonempty_unknown_or_nonquiescent_registry_requires_actual_dispatch() {
    for state in [
        RegistryState::Unknown,
        RegistryState::Euclidean {
            listeners: 1,
            pending_add: 0,
            pending_remove: 0,
            processing: false,
        },
        RegistryState::Euclidean {
            listeners: 0,
            pending_add: 1,
            pending_remove: 0,
            processing: false,
        },
        RegistryState::Euclidean {
            listeners: 0,
            pending_add: 0,
            pending_remove: 1,
            processing: false,
        },
        RegistryState::Euclidean {
            listeners: 0,
            pending_add: 0,
            pending_remove: 0,
            processing: true,
        },
    ] {
        let mut world = World::new("all", "euclidean");
        world.override_state = Some(state);
        let error = post_to_empty_registries(
            &mut world,
            ItemGameEvent::HitGround.at(Vec3::new(0., 64., 0.)),
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("requires actual listener dispatch"));
        assert_eq!(
            world.trace.len(),
            3,
            "reject at first queried registry without suppressing the unknown context"
        );
        let mut unloaded = World::new("none", "euclidean");
        unloaded.override_state = Some(state);
        let result = post_to_empty_registries(
            &mut unloaded,
            ItemGameEvent::Splash.at(Vec3::new(0., 64., 0.)),
        )
        .unwrap();
        assert_eq!(
            result.section_visits, 0,
            "original never queries registry state of absent chunks"
        );
    }
}

#[test]
fn item_event_identifiers_and_radii_match_actual_registered_events() {
    let facts = fixture("tools/game_event_context_factories.json");
    assert_eq!(facts["game_events"].as_array().unwrap().len(), 60);
    for event in [
        ItemGameEvent::EntityDamage,
        ItemGameEvent::HitGround,
        ItemGameEvent::Splash,
    ] {
        let fact = facts["game_events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["name"] == event.name())
            .unwrap();
        assert_eq!(fact["id"], event.registry_id());
        assert_eq!(fact["radius"], event.notification_radius());
    }
    let dynamic = facts["dynamic_entity_listener_methods"].as_array().unwrap();
    assert_eq!(dynamic.len(), 2);
    for fact in dynamic {
        assert_eq!(fact["declaring_class"], "bsr");
        assert_eq!(fact["consumer_calls"], 0);
    }
}
