use super::*;
use serde_json::{json, Value};

fn state(key: &Value) -> FluidState {
    let amount = key["amount"].as_i64().unwrap() as i32;
    FluidState {
        kind: match key["kind"].as_u64().unwrap() {
            1 => FluidKind::Water,
            2 => FluidKind::Lava,
            _ => panic!(),
        },
        own_height: amount as f32 / 9_f32,
        amount,
        falling: key["falling"].as_bool().unwrap(),
        source: key["source"].as_bool().unwrap(),
    }
}
fn key(state: FluidState) -> Value {
    json!({"kind":match state.kind{FluidKind::Water=>1,FluidKind::Lava=>2,FluidKind::Empty=>0},"amount":state.amount,"falling":state.falling,"source":state.source})
}
fn position(value: &Value) -> BlockPos {
    (
        value[0].as_i64().unwrap() as i32,
        value[1].as_i64().unwrap() as i32,
        value[2].as_i64().unwrap() as i32,
    )
}
fn double(value: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}
fn vector(value: &Value) -> Vec3 {
    Vec3::new(double(&value[0]), double(&value[1]), double(&value[2]))
}
fn boxes(shape: &VoxelShape) -> Value {
    json!(shape
        .to_aabbs()
        .iter()
        .map(|b| [b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z]
            .map(|v| format!("{:016x}", v.to_bits())))
        .collect::<Vec<_>>())
}
fn owner(state: FluidState) -> &'static str {
    match (state.kind, state.source) {
        (FluidKind::Water, true) => "minecraft:water",
        (FluidKind::Water, false) => "minecraft:flowing_water",
        (FluidKind::Lava, true) => "minecraft:lava",
        (FluidKind::Lava, false) => "minecraft:flowing_lava",
        _ => panic!(),
    }
}
fn history(snapshot: &Value, state: FluidState) -> FluidShapeHistory {
    let Some(map) = snapshot
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["owner"] == owner(state))
    else {
        return FluidShapeHistory::Unknown;
    };
    let found = map["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"] == key(state));
    match found {
        None => FluidShapeHistory::Unqueried,
        Some(entry) => {
            assert_eq!(entry["boxes"].as_array().unwrap().len(), 1);
            FluidShapeHistory::CachedHeight(double(&entry["boxes"][0][4]) as f32)
        }
    }
}
struct World {
    above: BlockPos,
    fluid: FluidState,
    fault: bool,
    trace: Vec<String>,
}
impl FluidWorld for World {
    fn has_chunks_at(&mut self, _: i32, _: i32, _: i32, _: i32) -> Result<bool> {
        bail!("unexpected shape chunk query")
    }
    fn block(&mut self, p: BlockPos) -> Result<u32> {
        self.trace.push(format!("block:{},{},{}", p.0, p.1, p.2));
        bail!("unexpected shape block query")
    }
    fn fluid(&mut self, p: BlockPos) -> Result<FluidState> {
        self.trace.push(format!("fluid:{},{},{}", p.0, p.1, p.2));
        ensure!(p == self.above, "unexpected fluid shape position");
        if self.fault {
            bail!("controlled above query fault");
        }
        Ok(self.fluid)
    }
}

#[test]
fn original_flowing_fluid_first_query_history_shapes_queries_and_faults() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/fluid-shape-vectors.json"
    ))
    .unwrap();
    let fixture: Value = serde_json::from_str(&fixture).unwrap();
    let metadata = fixture["states"].as_array().unwrap();
    assert_eq!(metadata.len(), 36);
    for row in metadata {
        assert_eq!(
            state(&row["key"]).own_height.to_bits(),
            u32::from_str_radix(row["own_height_bits"].as_str().unwrap(), 16).unwrap()
        );
    }
    let mut queries = 0;
    let mut faults = 0;
    let mut clips = 0;
    for scene in fixture["scenes"].as_array().unwrap() {
        let mut cache = FluidShapeCache::from_observed_history(metadata.iter().map(|row| {
            let state = state(&row["key"]);
            (state, history(&scene["initial_cache"], state))
        }))
        .unwrap();
        for query in scene["queries"].as_array().unwrap() {
            let fluid = if let Some(block) = query.get("block_state") {
                FluidState::from_block(block.as_u64().unwrap() as u32).unwrap()
            } else {
                state(&query["key"])
            };
            let above = if let Some(key) = query.get("above_key") {
                state(key)
            } else if let Some(block) = query.get("above_block_state") {
                FluidState::from_block(block.as_u64().unwrap() as u32).unwrap()
            } else {
                FluidState::from_block(0).unwrap()
            };
            let p = position(&query["position"]);
            let mut world = World {
                above: (p.0, p.1.wrapping_add(1), p.2),
                fluid: above,
                fault: query["fault_above"].as_bool().unwrap_or(false),
                trace: Vec::new(),
            };
            let result = cache.shape(&mut world, p, fluid);
            let expected = &query["expected"];
            assert_eq!(key(fluid), expected["resolved_key"]);
            assert_eq!(
                result.is_ok(),
                expected["accepted"].as_bool().unwrap(),
                "{} query{queries}",
                scene["name"]
            );
            if let Ok(shape) = result {
                assert_eq!(
                    boxes(&shape),
                    expected["boxes"],
                    "{} query{queries}",
                    scene["name"]
                );
                if let Some(from) = query.get("clip_from_bits") {
                    let clipped = shape
                        .clip(vector(from), vector(&query["clip_to_bits"]), p)
                        .is_some();
                    assert_eq!(clipped, expected["clip_hit"].as_bool().unwrap());
                    clips += 1;
                }
            } else {
                faults += 1;
            }
            assert_eq!(
                json!(world.trace),
                expected["trace"],
                "{} trace{queries}",
                scene["name"]
            );
            for row in metadata {
                let state = state(&row["key"]);
                assert_eq!(
                    cache.history(state).unwrap(),
                    history(&expected["cache_after"], state),
                    "{} cache{queries} {}",
                    scene["name"],
                    row["key"]
                );
            }
            queries += 1;
        }
    }
    assert!(queries >= 237);
    assert_eq!(faults, 3);
    assert_eq!(clips, 24);
}

#[test]
fn unknown_history_is_guarded_and_only_observed_owner_is_initialized() {
    let source = FluidState {
        kind: FluidKind::Water,
        source: true,
        amount: 8,
        falling: false,
        own_height: 8_f32 / 9_f32,
    };
    let mut world = World {
        above: (0, 1, 0),
        fluid: source,
        fault: false,
        trace: Vec::new(),
    };
    let mut unknown = FluidShapeCache::unknown();
    assert!(unknown.shape(&mut world, (0, 0, 0), source).is_err());
    assert!(world.trace.is_empty());
    let mut cache = FluidShapeCache::from_observed_empty_owners([FluidShapeOwner::SourceWater]);
    assert_eq!(cache.observations().count(), 2);
    let flowing = FluidState {
        source: false,
        ..source
    };
    assert!(cache.shape(&mut world, (0, 0, 0), flowing).is_err());
    assert!(world.trace.is_empty());
    cache.shape(&mut world, (0, 0, 0), source).unwrap();
    assert_eq!(world.trace.len(), 1);
    assert_eq!(
        cache.history(source).unwrap(),
        FluidShapeHistory::CachedHeight(1.)
    );
    world.fault = true;
    cache.shape(&mut world, (0, 0, 0), source).unwrap();
    assert_eq!(world.trace.len(), 1);
    let empty = FluidState::from_block(0).unwrap();
    assert!(cache
        .shape(&mut world, (0, 0, 0), empty)
        .unwrap()
        .to_aabbs()
        .is_empty());
    assert_eq!(world.trace.len(), 1);
    assert!(FluidShapeCache::from_observed_history([(
        source,
        FluidShapeHistory::CachedHeight(f32::NAN)
    )])
    .is_err());
    assert!(FluidShapeCache::from_observed_history([
        (source, FluidShapeHistory::Unqueried),
        (source, FluidShapeHistory::Unqueried)
    ])
    .is_err());
    let bad = FluidState {
        amount: 9,
        own_height: 1.,
        ..source
    };
    assert!(cache.shape(&mut world, (0, 0, 0), bad).is_err());
    assert_eq!(world.trace.len(), 1);
    // No uncontrolled fresh-cache assumption: all four owners are initialized
    // only after each original fresh JVM snapshot has been checked empty.
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/fluid-shape-vectors.json"
    ))
    .unwrap();
    let fixture: Value = serde_json::from_str(&fixture).unwrap();
    for scene in fixture["scenes"].as_array().unwrap() {
        let initial = scene["initial_cache"].as_array().unwrap();
        assert_eq!(initial.len(), 4);
        assert!(initial
            .iter()
            .all(|owner| owner["entries"].as_array().unwrap().is_empty()));
    }
    assert_eq!(
        FluidShapeCache::from_observed_empty_owners(FluidShapeOwner::ALL)
            .observations()
            .count(),
        36
    );
}
