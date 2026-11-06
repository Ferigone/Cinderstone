//! Differential tests against the original `EndDragonFight.Data` codec and gateway shuffle.
//!
//! `fixtures/level-data-vectors.json` comes from `tools/level_data_generate.py` (see
//! `docs/reference/level-data.md`).

use super::{default_gateways, DragonFightData, TimerCallback, TimerQueue};
use crate::components::java::resource_location;
use crate::nbt::{JavaString, Tag};
use serde_json::Value;

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn uuid_text(bytes: [u8; 16]) -> String {
    let digits = hex(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &digits[0..8],
        &digits[8..12],
        &digits[12..16],
        &digits[16..20],
        &digits[20..32]
    )
}

fn check_dragon_rows(rows: &[Value]) -> Vec<String> {
    let mut failures = Vec::new();
    for row in rows {
        let payload = row["payload"].as_str().unwrap();
        let tag = Tag::decode_anonymous(&unhex(payload)).unwrap();
        let parsed = DragonFightData::from_nbt(&tag);
        if parsed.is_ok() != row["ok"].as_bool().unwrap() {
            failures.push(format!("{payload}: accepted {}", parsed.is_ok()));
            continue;
        }
        let Ok(data) = parsed else { continue };
        let fields = &row["fields"];
        let mut wrong = Vec::new();
        if data.needs_state_scanning != fields["needs_state_scanning"].as_bool().unwrap() {
            wrong.push("needs_state_scanning");
        }
        if data.dragon_killed != fields["dragon_killed"].as_bool().unwrap() {
            wrong.push("dragon_killed");
        }
        if data.previously_killed != fields["previously_killed"].as_bool().unwrap() {
            wrong.push("previously_killed");
        }
        if data.is_respawning != fields["is_respawning"].as_bool().unwrap() {
            wrong.push("is_respawning");
        }
        if data.dragon_uuid.map(uuid_text).as_deref() != fields["dragon"].as_str() {
            wrong.push("dragon");
        }
        let exit: Option<Vec<i64>> = fields.get("exit").map(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_i64().unwrap())
                .collect()
        });
        if data
            .exit_portal_location
            .map(|p| p.iter().map(|n| i64::from(*n)).collect::<Vec<_>>())
            != exit
        {
            wrong.push("exit");
        }
        let gateways: Option<Vec<i64>> = fields.get("gateways").map(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_i64().unwrap())
                .collect()
        });
        if data
            .gateways
            .as_ref()
            .map(|g| g.iter().map(|n| i64::from(*n)).collect::<Vec<_>>())
            != gateways
        {
            wrong.push("gateways");
        }
        if !wrong.is_empty() {
            failures.push(format!("{payload}: {wrong:?} differ"));
        }
        let encoded = hex(&data.to_nbt().encode_anonymous().unwrap());
        if encoded != row["encoded"].as_str().unwrap() {
            failures.push(format!("{payload}: encoded {encoded}"));
        }
    }
    failures
}

fn check_gateways(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| {
            let seed = row["seed"].as_i64().unwrap();
            let wanted: Vec<i64> = row["gateways"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_i64().unwrap())
                .collect();
            let got: Vec<i64> = default_gateways(seed).into_iter().map(i64::from).collect();
            (got != wanted).then(|| format!("seed {seed}: {got:?}"))
        })
        .collect()
}

fn run_timer_scenario(case: &Value) -> Vec<String> {
    let mut queue = TimerQueue::new();
    let mut last_store = String::new();
    let mut failures = Vec::new();
    let steps = case["steps"].as_array().unwrap();
    let expected = case["expect"].as_array().unwrap();
    for (index, (step, expect)) in steps.iter().zip(expected).enumerate() {
        let kind = step["op"].as_str().unwrap();
        let mut fail = |what: String| {
            failures.push(format!(
                "step {index} {kind} {}: {what}\n  expected {expect}",
                step.to_string().chars().take(200).collect::<String>()
            ))
        };
        let mut fired: Vec<String> = Vec::new();
        match kind {
            "load" => {
                let tag = Tag::decode_anonymous(&unhex(step["hex"].as_str().unwrap())).unwrap();
                queue = TimerQueue::load(Some(&tag));
            }
            "reload" => {
                let tag = Tag::decode_anonymous(&unhex(&last_store)).unwrap();
                queue = TimerQueue::load(Some(&tag));
            }
            "schedule" => {
                let id = resource_location(step["id"].as_str().unwrap()).unwrap();
                let callback = if step["kind"].as_str().unwrap() == "function" {
                    TimerCallback::Function(id)
                } else {
                    TimerCallback::FunctionTag(id)
                };
                queue.schedule(
                    JavaString::from(step["name"].as_str().unwrap()),
                    step["time"].as_i64().unwrap(),
                    callback,
                );
            }
            "remove" => {
                let removed = queue.remove(&JavaString::from(step["name"].as_str().unwrap()));
                if removed as i64 != expect["removed"].as_i64().unwrap() {
                    fail(format!("removed {removed}"));
                }
            }
            "ids" => {
                let ids: Vec<String> = queue
                    .events_ids()
                    .iter()
                    .map(|id| id.to_string_lossy().into_owned())
                    .collect();
                let wanted: Vec<&str> = expect["ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect();
                if ids.iter().map(String::as_str).collect::<Vec<_>>() != wanted {
                    fail(format!("ids {ids:?}"));
                }
            }
            "store" => {
                let stored = hex(&queue.store().encode_anonymous().unwrap());
                if stored != expect["nbt"].as_str().unwrap() {
                    fail(format!("stored {stored}"));
                }
                last_store = expect["nbt"].as_str().unwrap().to_owned();
            }
            "tick" => {
                while let Some(event) = queue.pop_due(step["time"].as_i64().unwrap()) {
                    fired.push(match event.callback {
                        TimerCallback::Function(id) => format!("function {id}"),
                        TimerCallback::FunctionTag(id) => format!("function_tag {id}"),
                    });
                }
            }
            other => panic!("unknown op {other}"),
        }
        let wanted: Vec<&str> = expect["fired"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        if fired.iter().map(String::as_str).collect::<Vec<_>>() != wanted {
            fail(format!("fired {fired:?}"));
        }
    }
    failures
}

fn check_timer(scenarios: &[Value]) -> Vec<String> {
    scenarios
        .iter()
        .enumerate()
        .flat_map(|(index, case)| {
            run_timer_scenario(case)
                .into_iter()
                .map(move |failure| format!("scenario {index}: {failure}"))
        })
        .collect()
}

#[test]
fn the_timer_queue_matches_the_original() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/level-data-vectors.json")).unwrap();
    let scenarios = fixture["timer_queue"].as_array().unwrap();
    assert!(scenarios.len() > 40, "fixture is unexpectedly small");
    let failures = check_timer(scenarios);
    assert!(
        failures.is_empty(),
        "{} failures in {} scenarios:\n{}",
        failures.len(),
        scenarios.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_dragon_fight_data_matches_the_original() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/level-data-vectors.json")).unwrap();
    let rows = fixture["dragon_fight"].as_array().unwrap();
    assert!(rows.len() > 300, "fixture is unexpectedly small");
    let failures = check_dragon_rows(rows);
    assert!(
        failures.is_empty(),
        "{} of {} rows disagree:\n{}",
        failures.len(),
        rows.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_gateway_order_matches_the_original() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/level-data-vectors.json")).unwrap();
    let rows = fixture["gateways"].as_array().unwrap();
    assert!(rows.len() > 100);
    let failures = check_gateways(rows);
    assert!(
        failures.is_empty(),
        "{} seeds disagree:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The full generated set (`--full-out`):
/// `LEVEL_DATA_FILE=<json> cargo test --lib level_data::tests::explore -- --ignored`.
#[test]
#[ignore]
fn explore() {
    let path = std::env::var("LEVEL_DATA_FILE").expect("LEVEL_DATA_FILE");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut failures = check_dragon_rows(fixture["dragon_fight"].as_array().unwrap());
    failures.extend(check_gateways(fixture["gateways"].as_array().unwrap()));
    failures.extend(check_timer(fixture["timer_queue"].as_array().unwrap()));
    for failure in failures.iter().take(20) {
        eprintln!("{failure}");
    }
    assert!(failures.is_empty(), "{} disagreements", failures.len());
}

#[test]
fn the_default_data_is_the_codecs_default() {
    let empty = DragonFightData::from_nbt(&Tag::Compound(Vec::new())).unwrap();
    assert_eq!(empty, DragonFightData::DEFAULT);
    assert!(DragonFightData::from_nbt(&Tag::Int(1)).is_err());
}
