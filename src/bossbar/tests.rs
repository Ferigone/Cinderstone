//! Differential tests against the original boss bar classes.
//!
//! `fixtures/bossbar-vectors.json` comes from `tools/bossbar_generate.py` (see
//! `docs/reference/bossbar.md`): scripted scenarios with the original's state, saved NBT and the
//! frames each stub player was sent, and one-step mutants of a saved boss bar loaded by the original.

use super::{BossBarColor, BossBarOverlay, CustomBossEvents, Uuid};
use crate::components::text::Component;
use crate::nbt::Tag;
use serde_json::Value;
use std::collections::BTreeMap;

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn uuid(text: &str) -> Uuid {
    let digits: String = text.chars().filter(|c| *c != '-').collect();
    unhex(&digits).try_into().expect("16 bytes")
}

fn uuid_text(bytes: Uuid) -> String {
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

fn component(value: &Value) -> Component {
    Component::from_json(value).expect("the original accepted this name")
}

/// The fields of `bossState`, compared with the event.
fn state_differences(
    events: &CustomBossEvents,
    id: &str,
    expect: &Value,
    check_uuid: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    let Some(event) = events.get(id) else {
        return vec!["no such event".to_owned()];
    };
    let base = event.base();
    let mut check = |what: &str, ok: bool| {
        if !ok {
            out.push(format!("{what} differs"));
        }
    };
    if check_uuid {
        check(
            "uuid",
            uuid_text(base.id()) == expect["uuid"].as_str().unwrap(),
        );
    }
    check(
        "name",
        base.name().to_json_string() == expect["name"].as_str().unwrap(),
    );
    check(
        "progress",
        i64::from(base.progress().to_bits() as i32) == expect["progress_bits"].as_i64().unwrap(),
    );
    check(
        "value",
        i64::from(event.value()) == expect["value"].as_i64().unwrap(),
    );
    check(
        "max",
        i64::from(event.max()) == expect["max"].as_i64().unwrap(),
    );
    check(
        "color",
        base.color().name() == expect["color"].as_str().unwrap(),
    );
    check(
        "overlay",
        base.overlay().name() == expect["overlay"].as_str().unwrap(),
    );
    check(
        "darken",
        base.should_darken_screen() == expect["darken"].as_bool().unwrap(),
    );
    check(
        "music",
        base.should_play_boss_music() == expect["music"].as_bool().unwrap(),
    );
    check(
        "fog",
        base.should_create_world_fog() == expect["fog"].as_bool().unwrap(),
    );
    check(
        "visible",
        base.is_visible() == expect["visible"].as_bool().unwrap(),
    );
    out
}

fn drain(events: &mut CustomBossEvents) -> Vec<(Uuid, Vec<u8>)> {
    let mut out = Vec::new();
    for id in events.ids() {
        if let Some(event) = events.get_mut(&id) {
            for (player, packet) in event.base_mut().take_outbox() {
                out.push((player, packet.frame().unwrap()));
            }
        }
    }
    out
}

fn run_scenario(case: &Value, players: &BTreeMap<String, Uuid>) -> Vec<String> {
    let mut events = CustomBossEvents::new();
    let mut failures = Vec::new();
    let mut last_save = String::new();
    // The original's stub players have entity ids 1, 2, 3, ... in order of first mention, and a
    // hash set of players iterates by id.
    let mut creation: Vec<String> = Vec::new();
    let steps = case["steps"].as_array().unwrap();
    let expected = case["expect"].as_array().unwrap();
    for (index, (step, expect)) in steps.iter().zip(expected).enumerate() {
        let kind = step["op"].as_str().unwrap();
        let id = step.get("id").and_then(Value::as_str).unwrap_or("");
        let mut fail = |what: String| {
            failures.push(format!(
                "step {index} {kind} {}: {what}\n  expected {expect}",
                step.to_string().chars().take(160).collect::<String>()
            ))
        };
        let player = |name: &str| *players.get(name).expect("known player");
        if let Some(name) = step.get("player").and_then(Value::as_str) {
            if !creation.iter().any(|known| known == name) {
                creation.push(name.to_owned());
            }
        }
        if let Some(names) = step.get("players").and_then(Value::as_array) {
            for name in names {
                let name = name.as_str().unwrap();
                if !creation.iter().any(|known| known == name) {
                    creation.push(name.to_owned());
                }
            }
        }
        let mut check_state = None;
        {
            let event = events.get_mut(id);
            match kind {
                "create" => {
                    let wanted = uuid(expect["state"]["uuid"].as_str().unwrap());
                    events.create(id, wanted, component(&step["name"])).unwrap();
                    check_state = Some(true);
                }
                "remove" => events.remove(id),
                "set_name" => event.unwrap().base_mut().set_name(component(&step["name"])),
                "set_value" => {
                    event
                        .unwrap()
                        .set_value(step["value"].as_i64().unwrap() as i32);
                    check_state = Some(false);
                }
                "set_max" => {
                    event
                        .unwrap()
                        .set_max(step["value"].as_i64().unwrap() as i32);
                    check_state = Some(false);
                }
                "set_progress" => event
                    .unwrap()
                    .base_mut()
                    .set_progress(f32::from_bits(step["bits"].as_i64().unwrap() as u32)),
                "set_color" => event
                    .unwrap()
                    .base_mut()
                    .set_color(BossBarColor::by_name(step["value"].as_str().unwrap())),
                "set_overlay" => event
                    .unwrap()
                    .base_mut()
                    .set_overlay(BossBarOverlay::by_name(step["value"].as_str().unwrap())),
                "set_darken" => event
                    .unwrap()
                    .base_mut()
                    .set_darken_screen(step["value"].as_bool().unwrap()),
                "set_music" => event
                    .unwrap()
                    .base_mut()
                    .set_play_boss_music(step["value"].as_bool().unwrap()),
                "set_fog" => event
                    .unwrap()
                    .base_mut()
                    .set_create_world_fog(step["value"].as_bool().unwrap()),
                "set_visible" => event
                    .unwrap()
                    .base_mut()
                    .set_visible(step["value"].as_bool().unwrap()),
                "add_player" => event
                    .unwrap()
                    .add_player(uuid(step["uuid"].as_str().unwrap())),
                "remove_player" => event
                    .unwrap()
                    .remove_player(uuid(step["uuid"].as_str().unwrap())),
                "remove_all" => event.unwrap().remove_all_players(),
                "add_offline" => event
                    .unwrap()
                    .add_offline_player(uuid(step["uuid"].as_str().unwrap())),
                "set_players" => {
                    let mut named: Vec<&str> = step["players"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|name| name.as_str().unwrap())
                        .collect();
                    named.sort_by_key(|name| creation.iter().position(|known| known == name));
                    let online: Vec<Uuid> = named.into_iter().map(player).collect();
                    let changed = event.unwrap().set_players(&online);
                    if changed != expect["changed"].as_bool().unwrap() {
                        fail(format!("changed {changed}"));
                    }
                }
                "connect" => events.on_player_connect(uuid(step["uuid"].as_str().unwrap())),
                "disconnect" => events.on_player_disconnect(uuid(step["uuid"].as_str().unwrap())),
                "state" => check_state = Some(false),
                "ids" => {
                    let wanted: Vec<&str> = expect["ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap())
                        .collect();
                    if events.ids().iter().map(String::as_str).collect::<Vec<_>>() != wanted {
                        fail(format!("ids {:?}", events.ids()));
                    }
                }
                "save" => {
                    let saved = hex(&events.save().encode_anonymous().unwrap());
                    if saved != expect["nbt"].as_str().unwrap() {
                        fail(format!("saved {saved}"));
                    }
                    last_save = expect["nbt"].as_str().unwrap().to_owned();
                }
                "load" => {
                    let tag = Tag::decode_anonymous(&unhex(&last_save)).unwrap();
                    // The original draws a random UUID per loaded event; none is observable here.
                    events.load(&tag, |_| [0u8; 16]).unwrap();
                }
                "packet" => {
                    let base = event.unwrap().base();
                    let packet = match step["kind"].as_str().unwrap() {
                        "add" => base.add_packet(),
                        "remove" => base.remove_packet(),
                        "progress" => base.update_progress_packet(),
                        "name" => base.update_name_packet(),
                        "style" => base.update_style_packet(),
                        _ => base.update_properties_packet(),
                    };
                    let frame = hex(&packet.frame().unwrap());
                    if frame != expect["frame"].as_str().unwrap() {
                        fail(format!("frame {frame}"));
                    }
                }
                other => panic!("unknown op {other}"),
            }
        }
        if let Some(check_uuid) = check_state {
            if let Some(wanted) = expect.get("state") {
                for difference in state_differences(&events, id, wanted, check_uuid) {
                    fail(difference);
                }
            }
        }
        // The frames each player was sent, in order (the original's global order across players
        // is that of an identity hash set).
        let mut sent: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (target, frame) in drain(&mut events) {
            sent.entry(uuid_text(target)).or_default().push(hex(&frame));
        }
        let mut wanted: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for entry in expect["sent"].as_array().unwrap() {
            let name = entry["player"].as_str().unwrap();
            wanted
                .entry(uuid_text(player(name)))
                .or_default()
                .push(entry["frame"].as_str().unwrap().to_owned());
        }
        if sent != wanted {
            fail(format!("sent {sent:?}"));
        }
    }
    failures
}

fn run_scenarios(fixture: &Value) -> (usize, Vec<String>) {
    let players: BTreeMap<String, Uuid> = fixture["meta"]["players"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| (name.clone(), uuid(value.as_str().unwrap())))
        .collect();
    let scenarios = fixture["scenarios"].as_array().unwrap();
    let mut failures = Vec::new();
    for (index, case) in scenarios.iter().enumerate() {
        for failure in run_scenario(case, &players) {
            failures.push(format!("scenario {index}: {failure}"));
        }
    }
    (scenarios.len(), failures)
}

fn run_mutants(fixture: &Value) -> (usize, Vec<String>) {
    let mutants = fixture["mutants"].as_array().unwrap();
    let mut failures = Vec::new();
    for row in mutants {
        let id = row["id"].as_str().unwrap();
        let payload = Tag::decode_anonymous(&unhex(row["payload"].as_str().unwrap())).unwrap();
        let wrapper = Tag::Compound(vec![(id.into(), payload)]);
        let mut events = CustomBossEvents::new();
        let outcome = events.load(&wrapper, |_| [7u8; 16]);
        let ok = outcome.is_ok();
        if ok != row["ok"].as_bool().unwrap() {
            failures.push(format!(
                "{}: loaded {ok}, original {}",
                row["payload"], row["ok"]
            ));
            continue;
        }
        if !ok {
            continue;
        }
        let differences = state_differences(&events, id, &row["state"], false);
        if !differences.is_empty() {
            failures.push(format!("{}: {differences:?}", row["payload"]));
        }
        let saved = hex(&events.save().encode_anonymous().unwrap());
        if saved != row["nbt"].as_str().unwrap() {
            failures.push(format!("{}: saved {saved}", row["payload"]));
        }
    }
    (mutants.len(), failures)
}

#[test]
fn the_boss_bars_match_the_original() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/bossbar-vectors.json")).unwrap();
    let (count, failures) = run_scenarios(&fixture);
    assert!(count > 40, "fixture is unexpectedly small: {count}");
    assert!(
        failures.is_empty(),
        "{} failures in {count} scenarios:\n{}",
        failures.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn loaded_boss_bar_mutants_match_the_original() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/bossbar-vectors.json")).unwrap();
    let (count, failures) = run_mutants(&fixture);
    assert!(count > 100, "fixture is unexpectedly small: {count}");
    assert!(
        failures.is_empty(),
        "{} failures in {count} mutants:\n{}",
        failures.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The full generated set (`--full-out`):
/// `BOSSBAR_FILE=<json> cargo test --lib bossbar::tests::explore -- --ignored`.
#[test]
#[ignore]
fn explore() {
    let path = std::env::var("BOSSBAR_FILE").expect("BOSSBAR_FILE");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let (scenarios, mut failures) = run_scenarios(&fixture);
    let (mutants, more) = run_mutants(&fixture);
    failures.extend(more);
    for failure in failures.iter().take(20) {
        eprintln!("{failure}");
    }
    assert!(
        failures.is_empty(),
        "{} failures in {scenarios} scenarios and {mutants} mutants",
        failures.len()
    );
}
