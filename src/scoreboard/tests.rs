//! Differential tests against the original `Scoreboard`, `ServerScoreboard`, the five packets and
//! `ScoreboardSaveData`. Fixtures: `fixtures/scoreboard-*.json` from `tools/scoreboard_generate.py`.

use super::format::NumberFormat;
use super::model::{CollisionRule, DisplaySlot, Outbound, Scoreboard, Visibility};
use super::save::{load_scoreboard, save_scoreboard};
use super::{ChatFormatting, ObjectiveCriteria, RenderType, ScoreHolder};
use crate::components::text::Component;
use crate::nbt::{JavaString, Tag};
use serde_json::{json, Value};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// Compounds as maps; the `PlayerScores` list as a set (the original keys one holder's scores by
/// object identity, so their order differs from run to run).
fn normalized(tag: &Tag) -> Tag {
    match tag {
        Tag::Compound(entries) => {
            let mut sorted: Vec<(JavaString, Tag)> = entries
                .iter()
                .map(|(k, v)| {
                    let mut value = normalized(v);
                    if k == "PlayerScores" {
                        if let Tag::List { values, .. } = &mut value {
                            values.sort_by_key(|score| format!("{:?}", score));
                        }
                    }
                    (k.clone(), value)
                })
                .collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            Tag::Compound(sorted)
        }
        Tag::List {
            element_type,
            values,
        } => Tag::List {
            element_type: *element_type,
            values: values.iter().map(normalized).collect(),
        },
        other => other.clone(),
    }
}

fn tag_of(hex_text: &str) -> Tag {
    Tag::decode_anonymous(&unhex(hex_text)).expect("fixture NBT")
}

fn same_tag(actual: &Tag, expected_hex: &str) -> bool {
    normalized(actual) == normalized(&tag_of(expected_hex))
}

fn component(value: &Value) -> Component {
    Component::from_json(value).expect("component")
}

fn format_of(value: &Value) -> Result<Option<NumberFormat>, String> {
    if value.is_null() {
        return Ok(None);
    }
    NumberFormat::from_json(value)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn holder(step: &Value) -> ScoreHolder {
    ScoreHolder {
        name: step["holder"].as_str().unwrap().to_owned(),
        display_name: step.get("holder_display").map(component),
    }
}

/// Runs one step; `Err` is where Java throws.
fn run_step(sb: &mut Scoreboard, step: &Value) -> Result<Value, String> {
    let text = |key: &str| step[key].as_str().unwrap().to_owned();
    let err = |e: super::model::ScoreboardError| e.to_string();
    let mut result = json!({});
    match step["op"].as_str().unwrap() {
        "objective_add" => {
            let criteria = ObjectiveCriteria::by_name(&text("criteria"))
                .map_err(|e| e.0)?
                .ok_or("no such criteria")?;
            sb.add_objective(
                &text("name"),
                criteria,
                component(&step["display"]),
                RenderType::by_id(&text("render")),
                step["auto"].as_bool().unwrap(),
                format_of(&step["format"])?,
            )
            .map_err(err)?;
        }
        "objective_remove" => sb.remove_objective(&text("name")).map_err(err)?,
        "objective_set" => {
            let name = text("name");
            if let Some(display) = step.get("display") {
                sb.set_objective_display_name(&name, component(display))
                    .map_err(err)?;
            }
            if let Some(render) = step.get("render") {
                sb.set_objective_render_type(&name, RenderType::by_id(render.as_str().unwrap()))
                    .map_err(err)?;
            }
            if let Some(auto) = step.get("auto") {
                sb.set_objective_auto_update(&name, auto.as_bool().unwrap())
                    .map_err(err)?;
            }
            if let Some(format) = step.get("format") {
                sb.set_objective_number_format(&name, format_of(format)?)
                    .map_err(err)?;
            }
        }
        "display" => {
            let slot = DisplaySlot::by_name(&text("slot")).ok_or("no such slot")?;
            // `getObjective(name)` of an unknown name is `null`, which clears the slot.
            let objective = step["objective"]
                .as_str()
                .filter(|name| sb.objective(name).is_some());
            sb.set_display_objective(slot, objective).map_err(err)?;
        }
        op @ ("score_set" | "score_add" | "score_reset" | "score_lock" | "score_unlock"
        | "score_display" | "score_format" | "score_get") => {
            let force = step.get("force").and_then(Value::as_bool).unwrap_or(false);
            let mut access = sb
                .get_or_create_player_score(&holder(step), &text("objective"), force)
                .map_err(err)?;
            match op {
                "score_set" => access
                    .set(sb, step["value"].as_i64().unwrap() as i32)
                    .map_err(err)?,
                "score_add" => {
                    result["ret"] = json!(access
                        .add(sb, step["value"].as_i64().unwrap() as i32)
                        .map_err(err)?)
                }
                "score_reset" => access.reset(sb).map_err(err)?,
                "score_lock" => access.lock(sb),
                "score_unlock" => access.unlock(sb),
                "score_display" => access.set_display(
                    sb,
                    (!step["display"].is_null()).then(|| component(&step["display"])),
                ),
                "score_format" => {
                    access.set_number_format_override(sb, format_of(&step["format"])?)
                }
                _ => result["ret"] = json!(access.get(sb)),
            }
        }
        "score_remove" => sb
            .reset_single_player_score(&holder(step), &text("objective"))
            .map_err(err)?,
        "score_remove_all" => sb.reset_all_player_scores(&holder(step)),
        "team_add" => sb.add_player_team(&text("name")),
        "team_remove" => sb.remove_player_team(&text("name")).map_err(err)?,
        "team_set" => {
            let name = text("name");
            let value = &step["value"];
            match step["field"].as_str().unwrap() {
                "display" => sb.modify_team(&name, |t| t.display_name = component(value)),
                "prefix" => sb.modify_team(&name, |t| {
                    t.prefix = if value.is_null() {
                        Component::literal("")
                    } else {
                        component(value)
                    }
                }),
                "suffix" => sb.modify_team(&name, |t| {
                    t.suffix = if value.is_null() {
                        Component::literal("")
                    } else {
                        component(value)
                    }
                }),
                "color" => sb.modify_team(&name, |t| {
                    t.color = ChatFormatting::by_name(value.as_str().unwrap())
                }),
                "friendly" => {
                    sb.modify_team(&name, |t| t.allow_friendly_fire = value.as_bool().unwrap())
                }
                "invisibles" => sb.modify_team(&name, |t| {
                    t.see_friendly_invisibles = value.as_bool().unwrap()
                }),
                "nametag" => sb.modify_team(&name, |t| {
                    t.nametag_visibility = Visibility::by_name(value.as_str().unwrap()).unwrap()
                }),
                "death" => sb.modify_team(&name, |t| {
                    t.death_message_visibility =
                        Visibility::by_name(value.as_str().unwrap()).unwrap()
                }),
                "collision" => sb.modify_team(&name, |t| {
                    t.collision_rule = CollisionRule::by_name(value.as_str().unwrap()).unwrap()
                }),
                "options" => {
                    let bits = value.as_i64().unwrap();
                    sb.modify_team(&name, |t| t.allow_friendly_fire = bits & 1 > 0)
                        .and_then(|_| {
                            sb.modify_team(&name, |t| t.see_friendly_invisibles = bits & 2 > 0)
                        })
                }
                other => panic!("unknown team field {other}"),
            }
            .map_err(err)?;
        }
        "team_join" => {
            result["ret"] = json!(sb
                .add_player_to_team(&text("player"), &text("team"))
                .map_err(err)?)
        }
        "team_leave" => {
            result["ret"] = json!(sb.remove_player_from_team(&text("player")).map_err(err)?)
        }
        "team_leave_from" => sb
            .remove_player_from_team_of(&text("player"), &text("team"))
            .map_err(err)?,
        "track" => sb
            .start_tracking_objective(&text("objective"))
            .map_err(err)?,
        "untrack" => sb
            .stop_tracking_objective(&text("objective"))
            .map_err(err)?,
        op @ ("start_packets" | "stop_packets") => {
            let name = text("objective");
            let packets = if op == "start_packets" {
                sb.start_tracking_packets(&name).map_err(err)?
            } else {
                let objective = sb.objective(&name).cloned().ok_or("unknown objective")?;
                sb.stop_tracking_packets(&objective)
            };
            result["packets"] = json!(packets
                .iter()
                .map(|p| hex(&p.encode_frame().unwrap()))
                .collect::<Vec<_>>());
        }
        "save" => result["data"] = json!(hex(&save_scoreboard(sb).encode_anonymous().unwrap())),
        "dump" => result["dump"] = dump(sb),
        other => panic!("unknown op {other}"),
    }
    Ok(result)
}

fn gson(component: &Component) -> String {
    component.to_json_string()
}

fn format_hex(format: &Option<NumberFormat>) -> Value {
    match format {
        Some(format) => json!(hex(&format.to_nbt().encode_anonymous().unwrap())),
        None => Value::Null,
    }
}

fn dump(sb: &Scoreboard) -> Value {
    let objectives: Vec<Value> = sb
        .objectives()
        .iter()
        .map(|o| {
            json!({"name": o.name, "criteria": o.criteria.name(), "display": gson(&o.display_name), "render": o.render_type.id(),
                   "auto": o.display_auto_update, "format": format_hex(&o.number_format)})
        })
        .collect();
    let mut slots = serde_json::Map::new();
    for slot in DisplaySlot::all() {
        if let Some(name) = sb.display_objective(slot) {
            slots.insert(slot.name(), json!(name));
        }
    }
    let mut scores = Vec::new();
    for o in sb.objectives() {
        for entry in sb.list_player_scores(&o.name) {
            let locked = sb.player_score_info(&entry.owner, &o.name).unwrap().locked;
            scores.push(json!({"objective": o.name, "owner": entry.owner, "value": entry.value,
                "display": entry.display.as_ref().map(gson), "format": format_hex(&entry.number_format_override), "locked": locked}));
        }
    }
    let teams: Vec<Value> = sb
        .teams()
        .iter()
        .map(|t| {
            json!({"name": t.name, "display": gson(&t.display_name), "prefix": gson(&t.prefix), "suffix": gson(&t.suffix),
                "color": t.color.map(|c| c.name()), "friendly": t.allow_friendly_fire, "invisibles": t.see_friendly_invisibles,
                "nametag": t.nametag_visibility.name(), "death": t.death_message_visibility.name(),
                "collision": t.collision_rule.name(), "players": t.players()})
        })
        .collect();
    json!({"objectives": objectives, "slots": slots, "scores": scores, "tracked": sb.tracked_players(), "teams": teams})
}

fn broadcast_frames(sb: &mut Scoreboard) -> Vec<String> {
    sb.take_outbox()
        .into_iter()
        .filter_map(|outbound| match outbound {
            Outbound::Broadcast(packet) => Some(
                packet
                    .encode_frame()
                    .map_or_else(|_| "ERROR".to_owned(), |frame| hex(&frame)),
            ),
            // Java's player list is empty in the oracle, so these reach nobody.
            Outbound::EachPlayer(_) => None,
        })
        .collect()
}

fn run_scenario(case: &Value) -> Vec<String> {
    let mut failures = Vec::new();
    let expected = case["expect"].as_array().unwrap();
    let mut index = 0;
    let mut sb = Scoreboard::new_server();
    if let Some(load) = case.get("load") {
        let java_failed = expected[0].get("error").is_some();
        match (
            load_scoreboard(&tag_of(load.as_str().unwrap())),
            java_failed,
        ) {
            (Ok(loaded), false) => sb = loaded,
            (Err(_), true) => return failures,
            (Ok(_), true) => {
                return vec![format!(
                    "load: Java throws {}, Rust loads",
                    expected[0]["error"]
                )]
            }
            (Err(error), false) => return vec![format!("load: Rust fails ({error}), Java loads")],
        }
        index = 1;
    }
    for (step, expect) in case["steps"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&expected[index..])
    {
        let outcome = run_step(&mut sb, step);
        let frames = broadcast_frames(&mut sb);
        let label = format!("{step}");
        match (&outcome, expect.get("error")) {
            (Ok(result), None) => {
                for key in ["ret", "packets", "dump"] {
                    if result.get(key) != expect.get(key) {
                        let (a, b) = (
                            result.get(key).unwrap_or(&Value::Null).to_string(),
                            expect.get(key).unwrap_or(&Value::Null).to_string(),
                        );
                        let at = a
                            .bytes()
                            .zip(b.bytes())
                            .position(|(x, y)| x != y)
                            .unwrap_or(a.len().min(b.len()));
                        let window = |text: &str| {
                            text.get(at.saturating_sub(60)..(at + 80).min(text.len()))
                                .unwrap_or("")
                                .to_owned()
                        };
                        failures.push(format!(
                            "{label}: {key} differs at byte {at}\n  rust ...{}\n  java ...{}",
                            window(&a),
                            window(&b)
                        ));
                    }
                }
                if let Some(data) = expect.get("data") {
                    if !same_tag(
                        &tag_of(result["data"].as_str().unwrap()),
                        data.as_str().unwrap(),
                    ) {
                        failures.push(format!(
                            "{label}: saved data differs\n  rust {}\n  java {data}",
                            result["data"]
                        ));
                    }
                }
            }
            (Err(_), Some(_)) => {}
            (Ok(_), Some(error)) => failures.push(format!("{label}: Java throws {error}")),
            (Err(error), None) => {
                failures.push(format!("{label}: Rust fails ({error}), Java succeeds"))
            }
        }
        let wanted: Vec<String> = expect
            .get("broadcast")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|v| v.as_str().unwrap().to_owned()).collect())
            .unwrap_or_default();
        if outcome.is_ok() == expect.get("error").is_none() && frames != wanted {
            failures.push(format!(
                "{label}: broadcast\n  rust {frames:?}\n  java {wanted:?}"
            ));
        }
    }
    failures
}

#[test]
fn oracle_scenarios_match() {
    let doc: Value =
        serde_json::from_str(include_str!("../../fixtures/scoreboard-vectors.json")).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        for failure in run_scenario(case) {
            failures.push(format!("scenario {index}: {failure}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(cases.len() >= 20, "{}", cases.len());
}

fn check_load_mutants(text: &str, minimum: usize) {
    let doc: Value = serde_json::from_str(text).unwrap();
    let rows = doc["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    for row in rows {
        let payload = tag_of(row["payload"].as_str().unwrap());
        let loaded = if matches!(payload, Tag::Compound(_)) {
            load_scoreboard(&payload)
        } else {
            Err("not a compound".to_owned())
        };
        match (loaded, row.get("error")) {
            (Ok(sb), None) => {
                if dump(&sb) != row["dump"] {
                    let (a, b) = (dump(&sb).to_string(), row["dump"].to_string());
                    let at = a
                        .bytes()
                        .zip(b.bytes())
                        .position(|(x, y)| x != y)
                        .unwrap_or(a.len().min(b.len()));
                    failures.push(format!(
                        "{}: dump differs at {at}\n  rust ...{}\n  java ...{}",
                        row["payload"],
                        &a[at.saturating_sub(50)..(at + 60).min(a.len())],
                        &b[at.saturating_sub(50)..(at + 60).min(b.len())]
                    ));
                } else if !same_tag(&save_scoreboard(&sb), row["data"].as_str().unwrap()) {
                    failures.push(format!("{}: saved data differs", row["payload"]));
                }
            }
            (Err(_), Some(_)) => {}
            (Ok(_), Some(error)) => failures.push(format!(
                "Java throws {error}, Rust loads; payload {}",
                &row["payload"].as_str().unwrap()[..row["payload"].as_str().unwrap().len().min(60)]
            )),
            (Err(error), None) => failures.push(format!(
                "Rust fails ({error}), Java loads; teams {:?}",
                match payload.get("Teams") {
                    Some(Tag::List { values, .. }) => values
                        .iter()
                        .map(|t| match t {
                            Tag::Compound(e) => e
                                .iter()
                                .filter(|(k, v)| !matches!(
                                    (k.as_str(), v),
                                    (
                                        Some(
                                            "CollisionRule"
                                                | "NameTagVisibility"
                                                | "DeathMessageVisibility"
                                                | "Players"
                                        ),
                                        _
                                    )
                                ))
                                .map(|(k, v)| format!("{}={:?}", k.to_string_lossy(), v))
                                .collect::<Vec<_>>(),
                            other => vec![format!("{other:?}")],
                        })
                        .filter(|v| v
                            .iter()
                            .any(|x| x.starts_with("DisplayName") && !x.contains("team"))
                            || v.len() != 6)
                        .collect::<Vec<_>>(),
                    other => vec![vec![format!("{other:?}")]],
                }
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} disagree:\n{}",
        failures.len(),
        rows.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(rows.len() >= minimum, "only {} rows", rows.len());
}

#[test]
fn load_mutants_agree() {
    check_load_mutants(
        include_str!("../../fixtures/scoreboard-load-mutants.json"),
        100,
    );
}

/// Full-set run: `SCOREBOARD_MUTANTS_FILE=<json> cargo test --lib scoreboard::tests::explore_load_mutants -- --ignored`
#[test]
#[ignore]
fn explore_load_mutants() {
    let path = std::env::var("SCOREBOARD_MUTANTS_FILE").expect("SCOREBOARD_MUTANTS_FILE");
    check_load_mutants(&std::fs::read_to_string(path).unwrap(), 1000);
}

#[test]
fn the_scoreboard_round_trips_through_the_saved_data_storage() {
    use super::ScoreboardData;
    use crate::saved_data::DimensionDataStorage;
    let dir = std::env::temp_dir().join(format!("cinderstone-scoreboard-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut storage = DimensionDataStorage::new(&dir);
    {
        let data = storage
            .compute_if_absent(&ScoreboardData::factory(), super::save::FILE_ID)
            .unwrap();
        assert!(
            !crate::saved_data::SavedData::is_dirty(data),
            "a new scoreboard is clean"
        );
        let sb = &mut data.scoreboard;
        sb.add_objective(
            "kills",
            ObjectiveCriteria::dummy(),
            Component::literal("Kills"),
            RenderType::Integer,
            false,
            Some(NumberFormat::Blank),
        )
        .unwrap();
        sb.set_display_objective(DisplaySlot::Sidebar, Some("kills"))
            .unwrap();
        let mut access = sb
            .get_or_create_player_score(&ScoreHolder::named("Steve"), "kills", false)
            .unwrap();
        access.set(sb, 7).unwrap();
        sb.add_player_team("red");
        sb.add_player_to_team("Steve", "red").unwrap();
        assert!(
            crate::saved_data::SavedData::is_dirty(data),
            "every change marks the data dirty"
        );
    }
    storage.save();
    let before = dump(
        &storage
            .compute_if_absent(&ScoreboardData::factory(), super::save::FILE_ID)
            .unwrap()
            .scoreboard,
    );
    let mut reloaded = DimensionDataStorage::new(&dir);
    let again = &reloaded
        .get(&ScoreboardData::factory(), super::save::FILE_ID)
        .unwrap()
        .expect("the file exists")
        .scoreboard;
    assert_eq!(dump(again), before);
    assert_eq!(again.display_objective(DisplaySlot::Sidebar), Some("kills"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn criteria_names_follow_the_original() {
    assert_eq!(
        ObjectiveCriteria::by_name("dummy").unwrap().unwrap().name(),
        "dummy"
    );
    assert!(ObjectiveCriteria::by_name("health")
        .unwrap()
        .unwrap()
        .is_read_only());
    assert_eq!(
        ObjectiveCriteria::by_name("health")
            .unwrap()
            .unwrap()
            .default_render_type(),
        RenderType::Hearts
    );
    assert_eq!(ObjectiveCriteria::custom_criteria().len(), 43);
    assert!(
        ObjectiveCriteria::by_name("minecraft.mined:minecraft.stone")
            .unwrap()
            .is_some()
    );
    assert!(
        ObjectiveCriteria::by_name("minecraft.mined:minecraft.zombie")
            .unwrap()
            .is_none()
    );
    assert!(ObjectiveCriteria::by_name("nope").unwrap().is_none());
    assert!(ObjectiveCriteria::by_name("minecraft.mined:minecraft.sto!ne").is_err());
}

#[test]
fn the_open_hash_map_keeps_fastutils_order() {
    use super::fastutil::OpenHashMap;
    let mut map = OpenHashMap::with_capacity(16, 0.5);
    for index in 0..40 {
        map.insert(&format!("player{index}"), index);
    }
    assert_eq!(map.len(), 40);
    for index in (0..40).step_by(3) {
        assert!(map.remove(&format!("player{index}")).is_some());
    }
    assert_eq!(map.len(), 26);
    assert!(map.get("player1").is_some() && map.get("player3").is_none());
    assert_eq!(map.keys().count(), 26);
    assert_eq!(map.keys_ascending().count(), 26);
}

#[test]
fn packets_carry_the_ids_of_the_protocol() {
    use super::packets::ScoreboardPacket;
    let reset = ScoreboardPacket::ResetScore {
        owner: "A".to_owned(),
        objective: None,
    };
    assert_eq!(reset.encode_frame().unwrap(), vec![0x44, 1, b'A', 0]);
    let display = ScoreboardPacket::SetDisplayObjective {
        slot: DisplaySlot::Sidebar,
        objective: "o".to_owned(),
    };
    assert_eq!(display.encode_frame().unwrap(), vec![0x57, 1, 1, b'o']);
}
