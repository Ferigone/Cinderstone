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
