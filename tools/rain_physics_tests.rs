use super::*;
use crate::{entity::Position, geometry::Vec3};
use anyhow::bail;
use serde_json::{json, Value};
struct World<'a> {
    query: &'a Value,
    facts: &'a Value,
    trace: Vec<String>,
}
fn key(position: BlockPosition) -> String {
    format!("{},{},{}", position.0, position.1, position.2)
}
impl World<'_> {
    fn record(&mut self, name: String, phase: &str) -> Result<()> {
        self.trace.push(name);
        if self.query["fault"].as_str() == Some(phase) {
            bail!("controlled/{phase}")
        }
        Ok(())
    }
    fn cell(&self, position: BlockPosition) -> &Value {
        let key = key(position);
        self.query
            .get("cells")
            .and_then(|cells| cells.get(&key))
            .unwrap_or(&self.query["default"])
    }
}
impl RainWorld for World<'_> {
    fn is_raining(&mut self) -> Result<bool> {
        self.record("rain".into(), "rain")?;
        Ok(self.facts["raining"].as_bool().unwrap())
    }
    fn sky_brightness(&mut self, p: BlockPosition) -> Result<i32> {
        self.record(format!("sky/{}", key(p)), "sky")?;
        Ok(self.cell(p)["sky"].as_i64().unwrap() as i32)
    }
    fn max_light_level(&mut self) -> Result<i32> {
        self.record("max".into(), "max")?;
        Ok(self.query["max_light"].as_i64().unwrap_or(15) as i32)
    }
    fn motion_blocking_heightmap_y(&mut self, p: BlockPosition) -> Result<i32> {
        self.record(format!("height/4/{},{}", p.0, p.2), "height")?;
        Ok(self
            .query
            .get("heights")
            .and_then(|heights| heights.get(format!("{},{}", p.0, p.2)))
            .unwrap_or(&self.query["height"])
            .as_i64()
            .unwrap() as i32)
    }
    fn precipitation_at(&mut self, p: BlockPosition) -> Result<Precipitation> {
        self.record(format!("biome/{}", key(p)), "biome")?;
        Ok(
            match self.facts["cells"][key(p)]["precipitation"]
                .as_u64()
                .unwrap()
            {
                0 => Precipitation::None,
                1 => Precipitation::Rain,
                2 => Precipitation::Snow,
                _ => panic!("original precipitation ordinal"),
            },
        )
    }
}
#[test]
fn original_level_and_entity_rain_predicates_match_results_queries_and_fault_priority() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/rain-physics-vectors.json")).unwrap();
    assert_eq!(
        fixture["oracle_inner_sha256"],
        "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
    );
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 1000);
    let mut queries = 0;
    let mut faults = 0;
    let mut entities = 0;
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let q = &case["query"];
        let e = &case["expected"];
        let mut world = World {
            query: q,
            facts: &e["world_facts"],
            trace: Vec::new(),
        };
        let p = (
            q["position"][0].as_i64().unwrap() as i32,
            q["position"][1].as_i64().unwrap() as i32,
            q["position"][2].as_i64().unwrap() as i32,
        );
        let answer = if q["op"] == "level" {
            level_is_raining_at(&mut world, p)
        } else {
            entities += 1;
            let mut state = EntityPhysicalState::new(
                1,
                [0; 16],
                Position {
                    x: 991.25,
                    y: 64.,
                    z: -995.125,
                    yaw: 0.,
                    pitch: 0.,
                    on_ground: false,
                },
                Vec3::ZERO,
                0.25,
                0.25,
                0.2125,
            )
            .unwrap();
            state.block_position = p;
            state.bounds.max_y =
                f64::from_bits(u64::from_str_radix(q["max_y"].as_str().unwrap(), 16).unwrap());
            entity_is_in_rain(&mut world, &state)
        };
        let actual = match answer {
            Ok(answer) => json!({"accepted":true,"result":answer,"trace":world.trace}),
            Err(error) => {
                faults += 1;
                json!({"accepted":false,"error_class":"java.lang.IllegalStateException","error_message":error.to_string(),"trace":world.trace})
            }
        };
        let mut expected = e.as_object().unwrap().clone();
        expected.remove("world_facts");
        assert_eq!(
            actual,
            Value::Object(expected),
            "original rain case{index}, group{}",
            case["group"]
        );
        queries += world.trace.len();
    }
    assert_eq!((entities, faults, queries), (373, 69, 4011));
    println!(
        "RAIN_PHYSICS_CONFORMANCE: {}",
        json!({"ok":true,"original_cases":1000,"entity_cases":entities,"controlled_faults":faults,"exact_trace_calls":queries,"mismatches":0})
    );
}
