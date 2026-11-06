//! Differential tests against the original `TagLoader`.
//!
//! `fixtures/datapack-tags-vectors.json` holds synthetic multi-pack scenarios and the original's
//! resolved tags (or its `StackOverflowError`); `fixtures/datapack-vanilla-tags-1.21.json` holds the
//! shipped vanilla tag JSON, whose resolution must reproduce the registry facts captured earlier.
//! Both come from `tools/datapack_generate.py` (see `docs/reference/datapack-tags.md`).

use super::tags::{load_and_build, PackTags};
use crate::loot::facts::facts;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

fn packs_of(case: &Value) -> Vec<PackTags> {
    case["packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pack| PackTags {
            id: pack["id"].as_str().unwrap().to_owned(),
            files: pack["files"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(id, text)| (id.clone(), text.as_str().unwrap().to_owned()))
                .collect(),
        })
        .collect()
}

#[test]
fn synthetic_scenarios_match_the_original() {
    check_scenarios(
        include_str!("../../fixtures/datapack-tags-vectors.json"),
        400,
    );
}

/// Larger run: `DATAPACK_TAGS_FILE=<json> cargo test --lib datapack::tests::explore_scenarios -- --ignored`
/// after `python3 tools/datapack_generate.py --scenarios 6000`.
#[test]
#[ignore]
fn explore_scenarios() {
    let path = std::env::var("DATAPACK_TAGS_FILE").expect("DATAPACK_TAGS_FILE");
    check_scenarios(&std::fs::read_to_string(path).unwrap(), 3000);
}

fn check_scenarios(text: &str, minimum: usize) {
    let doc: Value = serde_json::from_str(text).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    let (mut compared, mut overflows) = (0, 0);
    let mut failures = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let known: HashSet<&str> = case["known"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect();
        let result = load_and_build(&packs_of(case), "tags/test", |id| known.contains(id));
        match (&result, case.get("error")) {
            (Err(_), Some(_)) => overflows += 1,
            (Ok(built), None) => {
                compared += 1;
                let expected: BTreeMap<String, Vec<String>> = case["expect"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(id, members)| {
                        (
                            id.clone(),
                            members
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|m| m.as_str().unwrap().to_owned())
                                .collect(),
                        )
                    })
                    .collect();
                if *built != expected {
                    failures.push(format!(
                        "scenario {index}:\n  rust {built:?}\n  java {expected:?}"
                    ));
                }
            }
            (Ok(_), Some(error)) => failures.push(format!(
                "scenario {index}: Java throws {error}, Rust resolves"
            )),
            (Err(_), None) => {
                failures.push(format!("scenario {index}: Rust overflows, Java resolves"))
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        compared >= minimum,
        "compared {compared}, overflows {overflows}"
    );
    eprintln!("tag scenarios: {compared} resolved, {overflows} overflow as in the original");
}

/// The vanilla tag directories and the registry kind each one resolves against.
const KINDS: [(&str, &str); 7] = [
    ("item", "item"),
    ("block", "block"),
    ("enchantment", "enchantment"),
    ("damage_type", "damage_type"),
    ("entity_type", "entity_type"),
    ("worldgen/biome", "biome"),
    ("instrument", "instrument"),
];

#[test]
fn the_shipped_vanilla_tags_resolve_to_the_captured_facts() {
    let doc: Value = serde_json::from_str(include_str!(
        "../../fixtures/datapack-vanilla-tags-1.21.json"
    ))
    .unwrap();
    let directories = doc["directories"].as_object().unwrap();
    let facts = facts();
    for (directory, kind) in KINDS {
        let (top, nested) = directory
            .split_once('/')
            .map_or((directory, ""), |(a, b)| (a, b));
        let files = &directories[top];
        let mut pack = PackTags {
            id: "vanilla".to_owned(),
            files: Default::default(),
        };
        for (id, text) in files.as_object().unwrap() {
            // `worldgen` holds several registries: keep the ones below `worldgen/<nested>/`.
            let (namespace, path) = id.split_once(':').unwrap();
            if nested.is_empty() {
                pack.files
                    .insert(id.clone(), text.as_str().unwrap().to_owned());
            } else if let Some(rest) = path.strip_prefix(&format!("{nested}/")) {
                pack.files.insert(
                    format!("{namespace}:{rest}"),
                    text.as_str().unwrap().to_owned(),
                );
            }
        }
        let registry: HashSet<&str> = match kind {
            "item" => facts.registries.item.iter().map(String::as_str).collect(),
            "block" => facts.registries.block.iter().map(String::as_str).collect(),
            "enchantment" => facts
                .registries
                .enchantment
                .iter()
                .map(String::as_str)
                .collect(),
            "damage_type" => facts
                .registries
                .damage_type
                .iter()
                .map(String::as_str)
                .collect(),
            "entity_type" => facts
                .registries
                .entity_type
                .iter()
                .map(String::as_str)
                .collect(),
            "biome" => facts.registries.biome.iter().map(String::as_str).collect(),
            _ => facts
                .registries
                .instrument
                .iter()
                .map(String::as_str)
                .collect(),
        };
        let resolved = load_and_build(&[pack], &format!("tags/{directory}"), |id| {
            registry.contains(id)
        })
        .expect("vanilla tags are acyclic");
        let captured = match kind {
            "item" => &facts.tags.item,
            "block" => &facts.tags.block,
            "enchantment" => &facts.tags.enchantment,
            "damage_type" => &facts.tags.damage_type,
            "entity_type" => &facts.tags.entity_type,
            "biome" => &facts.tags.biome,
            _ => &facts.tags.instrument,
        };
        assert_eq!(resolved.len(), captured.len(), "{kind}: tag count");
        for (id, members) in captured {
            assert_eq!(resolved.get(id), Some(members), "{kind}: #{id}");
        }
    }
}

// ---------------------------------------------------------------- pack repository

mod packs {
    use super::super::packs::{Compatibility, PackRepository, Position};
    use base64::Engine;
    use serde_json::{json, Value};
    use std::path::PathBuf;

    struct Folder(PathBuf);

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn materialise(name: &str, files: &serde_json::Map<String, Value>) -> Folder {
        let path =
            std::env::temp_dir().join(format!("cinderstone-packs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        for (relative, content) in files {
            let target = path.join(relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            let bytes = match content {
                Value::String(text) => text.clone().into_bytes(),
                other => base64::engine::general_purpose::STANDARD
                    .decode(other["base64"].as_str().unwrap())
                    .unwrap(),
            };
            std::fs::write(target, bytes).unwrap();
        }
        Folder(path)
    }

    fn describe(repository: &PackRepository) -> Vec<Value> {
        repository
            .available_packs()
            .into_iter()
            .map(|pack| {
                json!({
                    "id": pack.id,
                    "compatibility": match pack.metadata.compatibility {
                        Compatibility::TooOld => "TOO_OLD",
                        Compatibility::TooNew => "TOO_NEW",
                        Compatibility::Compatible => "COMPATIBLE",
                    },
                    "description": pack.metadata.description.to_json_string(),
                    "title": pack.title.to_json_string(),
                    "features": pack.metadata.features,
                    "required": pack.required,
                    "fixed": pack.fixed_position,
                    "position": if pack.default_position == Position::Top { "TOP" } else { "BOTTOM" },
                    "auto": pack.add_automatically,
                })
            })
            .collect()
    }

    fn selected(repository: &PackRepository) -> Vec<String> {
        repository
            .selected_packs()
            .iter()
            .map(|pack| pack.id.clone())
            .collect()
    }

    pub fn check(text: &str) {
        let doc: Value = serde_json::from_str(text).unwrap();
        let mut failures = Vec::new();
        for case in doc["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let folder = materialise(name, case["files"].as_object().unwrap());
            let mut repository = PackRepository::new(&folder.0);
            let expected = &case["expect"];
            let found = describe(&repository);
            let wanted = expected["available"].as_array().unwrap();
            let found_ids: Vec<&str> = found.iter().map(|p| p["id"].as_str().unwrap()).collect();
            let wanted_ids: Vec<&str> = wanted.iter().map(|p| p["id"].as_str().unwrap()).collect();
            if case["name"] == "metadata" {
                // Only the sampled packs are materialised, and the original saw exactly those.
                if found_ids != wanted_ids {
                    let extra: Vec<_> = found_ids
                        .iter()
                        .filter(|id| !wanted_ids.contains(id))
                        .collect();
                    let missing: Vec<_> = wanted_ids
                        .iter()
                        .filter(|id| !found_ids.contains(id))
                        .collect();
                    failures.push(format!(
                        "{name}: Rust-only packs {extra:?}; Java-only packs {missing:?}"
                    ));
                }
            } else if found_ids != wanted_ids {
                failures.push(format!("{name}: packs {found_ids:?}, Java {wanted_ids:?}"));
            }
            for pack in &found {
                if let Some(other) = wanted.iter().find(|w| w["id"] == pack["id"]) {
                    if pack != other {
                        failures.push(format!(
                            "{name}: {} differs\n  rust {pack}\n  java {other}",
                            pack["id"]
                        ));
                    }
                }
            }
            let ids = |value: &Value| {
                value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>()
            };
            if selected(&repository) != ids(&expected["selected"]) {
                failures.push(format!(
                    "{name}: initial selection {:?}",
                    selected(&repository)
                ));
            }
            for (step, wanted) in case["steps"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected["steps"].as_array().unwrap())
            {
                match step["op"].as_str().unwrap() {
                    "select" => repository.set_selected(&ids(&step["ids"])),
                    "add" => {
                        if json!(repository.add_pack(step["id"].as_str().unwrap())) != wanted["ret"]
                        {
                            failures.push(format!("{name}: {step} return"));
                        }
                    }
                    "remove" => {
                        if json!(repository.remove_pack(step["id"].as_str().unwrap()))
                            != wanted["ret"]
                        {
                            failures.push(format!("{name}: {step} return"));
                        }
                    }
                    _ => repository.reload(),
                }
                if selected(&repository) != ids(&wanted["selected"])
                    || repository.requested_features() != ids(&wanted["features"])
                {
                    failures.push(format!(
                        "{name}: after {step}\n  rust {:?} {:?}\n  java {} {}",
                        selected(&repository),
                        repository.requested_features(),
                        wanted["selected"],
                        wanted["features"]
                    ));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} mismatches:\n{}",
            failures.len(),
            failures
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

#[test]
fn the_pack_repository_matches_the_original() {
    packs::check(include_str!("../../fixtures/datapack-packs-vectors.json"));
}

#[test]
fn the_world_data_configuration_reads_and_writes_like_the_original() {
    use super::config::WorldDataConfiguration;
    use crate::nbt::{JavaString, Tag};
    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }
    fn normalized(tag: &Tag) -> Tag {
        match tag {
            Tag::Compound(entries) => {
                let mut sorted: Vec<(JavaString, Tag)> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), normalized(v)))
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
    let doc: Value =
        serde_json::from_str(include_str!("../../fixtures/datapack-config-vectors.json")).unwrap();
    let rows = doc["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    for row in rows {
        let payload = Tag::decode_anonymous(&unhex(row[0].as_str().unwrap())).unwrap();
        let encoded = WorldDataConfiguration::from_nbt(&payload).to_nbt();
        let expected = Tag::decode_anonymous(&unhex(row[1].as_str().unwrap())).unwrap();
        // The feature list is a set in the original: compare the lists with the same order rule.
        if normalized(&encoded) != normalized(&expected) {
            failures.push(format!(
                "{}\n  rust {encoded:?}\n  java {expected:?}",
                row[0]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} disagree:\n{}",
        failures.len(),
        rows.len(),
        failures
            .iter()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(rows.len() > 300, "{}", rows.len());
}
