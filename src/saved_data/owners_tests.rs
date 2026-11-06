//! Differential tests of `CommandStorage` and `ForcedChunks` against the original.
//!
//! `fixtures/saved-data-owners-vectors.json` comes from `tools/saved_data_owners_generate.py`
//! (see `docs/reference/saved-data.md`): scripted scenarios with the original's answer for every
//! step, and payload mutants loaded by the original.

use super::file;
use super::java_collections::to_java_order;
use super::storage::DimensionDataStorage;
use super::{CommandStorage, ForcedChunks, SavedData};
use crate::nbt::{JavaString, Tag};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Compounds compared as maps (member order inside a stored value is Java's `HashMap` order of a
/// decoded tag, which the port does not reproduce for foreign data), lists in order.
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

fn same_tag(actual: &Tag, expected_hex: &str) -> bool {
    normalized(actual) == normalized(&Tag::decode_anonymous(&unhex(expected_hex)).unwrap())
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "cinderstone-owners-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    if !file::is_gzip(bytes) {
        return bytes.to_vec();
    }
    let mut out = Vec::new();
    flate2::read::MultiGzDecoder::new(bytes)
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn longs(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect()
}

fn run_scenario(case: &Value) -> Vec<String> {
    let dir = TempDir::new();
    let mut storage = DimensionDataStorage::new(dir.path());
    let mut commands = CommandStorage::new();
    let mut failures = Vec::new();
    let mut put: HashMap<String, Vec<u8>> = HashMap::new();
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
        let java_failed = expect.get("error").is_some();
        match kind {
            "put_snbt" | "put_hex" => {
                let name = step["name"].as_str().unwrap();
                let bytes = if kind == "put_snbt" {
                    unhex(expect["hex"].as_str().unwrap())
                } else {
                    unhex(step["hex"].as_str().unwrap())
                };
                fs::write(dir.path().join(format!("{name}.dat")), &bytes).unwrap();
                put.insert(name.to_owned(), bytes);
            }
            "cs_set" => {
                // The original parses SNBT into `HashMap` order; a caller of the port builds its
                // compounds the same way.
                let tag = to_java_order(
                    &Tag::decode_anonymous(&unhex(step["hex"].as_str().unwrap())).unwrap(),
                );
                let outcome = commands.set(&mut storage, step["id"].as_str().unwrap(), tag);
                if outcome.is_err() != java_failed {
                    fail(format!("outcome {outcome:?}"));
                }
            }
            "cs_get" => {
                let outcome = commands.get(&mut storage, step["id"].as_str().unwrap());
                match (&outcome, java_failed) {
                    (Ok(tag), false) => {
                        if !same_tag(tag, expect["nbt"].as_str().unwrap()) {
                            fail(format!("value {tag:?}"));
                        }
                    }
                    (Err(_), true) => {}
                    _ => fail(format!("outcome {outcome:?}")),
                }
            }
            "cs_keys" => {
                let outcome = commands.keys(&mut storage);
                match (&outcome, java_failed) {
                    (Ok(keys), false) => {
                        let wanted: Vec<&str> = expect["keys"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|k| k.as_str().unwrap())
                            .collect();
                        if keys.iter().map(String::as_str).collect::<Vec<_>>() != wanted {
                            fail(format!("keys {keys:?}"));
                        }
                    }
                    (Err(_), true) => {}
                    _ => fail(format!("outcome {outcome:?}")),
                }
            }
            "fc_apply" => {
                let forced = storage
                    .compute_if_absent(&ForcedChunks::factory(), super::forced_chunks::FILE_NAME)
                    .unwrap();
                let changed = forced.set_forced(
                    step["pos"].as_i64().unwrap(),
                    step["forced"].as_bool().unwrap(),
                );
                if changed != expect["changed"].as_bool().unwrap()
                    || forced.is_dirty() != expect["dirty"].as_bool().unwrap()
                {
                    fail(format!("changed {changed} dirty {}", forced.is_dirty()));
                }
                if let Some(wanted) = expect.get("chunks") {
                    if forced.to_vec() != longs(wanted) {
                        fail(format!("chunks {:?}", forced.to_vec()));
                    }
                }
            }
            "get" | "compute" | "set_dirty" => {
                let name = step["name"].as_str().unwrap();
                assert_eq!(name, "chunks", "only the forced chunks use this op here");
                let make = kind != "get";
                let factory = ForcedChunks::factory();
                let found = if make {
                    Some(storage.compute_if_absent(&factory, name).unwrap())
                } else {
                    storage.get(&factory, name).unwrap()
                };
                let (present, dirty, data) = match found {
                    Some(data) => {
                        if kind == "set_dirty" {
                            data.set_dirty(true);
                        }
                        (true, data.is_dirty(), Some(data.save()))
                    }
                    None => (false, false, None),
                };
                if present != expect["present"].as_bool().unwrap() {
                    fail(format!("present {present}"));
                } else if present {
                    if dirty != expect["dirty"].as_bool().unwrap() {
                        fail(format!("dirty {dirty}"));
                    }
                    let bytes = data.unwrap().encode_anonymous().unwrap();
                    if hex(&bytes) != expect["data"].as_str().unwrap() {
                        fail(format!("data {}", hex(&bytes)));
                    }
                }
            }
            "read_tag" => {
                let name = step["name"].as_str().unwrap();
                let fix = if name == "chunks" {
                    super::DataFixType::SavedDataForcedChunks
                } else {
                    super::DataFixType::SavedDataCommandStorage
                };
                let version = step["version"].as_i64().unwrap() as i32;
                let tag = storage.read_tag_from_disk(name, fix, version).unwrap();
                if !same_tag(&tag, expect["nbt"].as_str().unwrap()) {
                    fail(format!("tag {tag:?}"));
                }
            }
            "save" => {
                storage.save();
                let mut names: Vec<String> = fs::read_dir(dir.path())
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                    .collect();
                names.sort();
                let present: Vec<String> = names
                    .iter()
                    .map(|n| n.strip_suffix(".dat").unwrap_or(n).to_owned())
                    .collect();
                let wanted = expect["files"].as_array().unwrap();
                let wanted_names: Vec<&str> =
                    wanted.iter().map(|f| f["name"].as_str().unwrap()).collect();
                if present != wanted_names {
                    fail(format!("files {present:?}"));
                    continue;
                }
                for (name, wanted) in present.iter().zip(wanted) {
                    let bytes = fs::read(dir.path().join(format!("{name}.dat"))).unwrap();
                    let rewritten = put.get(name) != Some(&bytes);
                    if rewritten != wanted["rewritten"].as_bool().unwrap() {
                        fail(format!("{name}: rewritten {rewritten}"));
                        continue;
                    }
                    if let Some(raw) = wanted.get("raw").and_then(Value::as_str) {
                        if rewritten && hex(&gunzip(&bytes)) != raw {
                            fail(format!(
                                "{name}: written stream differs\n  port {}",
                                hex(&gunzip(&bytes))
                            ));
                        }
                    }
                }
            }
            other => panic!("unknown op {other}"),
        }
    }
    failures
}

fn run_scenarios(fixture: &str) -> (usize, Vec<String>) {
    let document: Value = serde_json::from_str(fixture).expect("fixture is JSON");
    let scenarios = document["scenarios"].as_array().unwrap();
    let mut failures = Vec::new();
    for (index, case) in scenarios.iter().enumerate() {
        for failure in run_scenario(case) {
            failures.push(format!("scenario {index}: {failure}"));
        }
    }
    (scenarios.len(), failures)
}

fn run_mutants(fixture: &str) -> (usize, Vec<String>) {
    let document: Value = serde_json::from_str(fixture).expect("fixture is JSON");
    let mutants = document["mutants"].as_array().unwrap();
    let mut failures = Vec::new();
    for row in mutants {
        let name = row["name"].as_str().unwrap();
        let payload = Tag::decode_anonymous(&unhex(row["payload"].as_str().unwrap())).unwrap();
        let dir = TempDir::new();
        let root = Tag::Compound(vec![
            (JavaString::from("data"), payload),
            (
                JavaString::from("DataVersion"),
                Tag::Int(super::DATA_VERSION),
            ),
        ]);
        fs::write(
            dir.path().join(format!("{name}.dat")),
            file::write_root(&root).unwrap(),
        )
        .unwrap();
        let mut storage = DimensionDataStorage::new(dir.path());
        if name == "chunks" {
            let found = storage.get(&ForcedChunks::factory(), name).unwrap();
            let saved = found.map(|data| hex(&data.save().encode_anonymous().unwrap()));
            let wanted = row["data"].as_str().map(str::to_owned);
            if saved != wanted {
                failures.push(format!(
                    "{name} {}: {saved:?} vs {wanted:?}",
                    row["payload"]
                ));
            }
        } else {
            let namespace = name.strip_prefix("command_storage_").unwrap();
            let mut commands = CommandStorage::new();
            let values = row["values"].as_object().unwrap();
            for (key, wanted) in values {
                let tag = commands
                    .get(&mut storage, &format!("{namespace}:{key}"))
                    .unwrap();
                if !same_tag(&tag, wanted.as_str().unwrap()) {
                    failures.push(format!("{name} {} get {key}: {tag:?}", row["payload"]));
                }
            }
            let keys = commands.keys(&mut storage).unwrap();
            let wanted: Vec<&str> = row["keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|k| k.as_str().unwrap())
                .collect();
            if keys.iter().map(String::as_str).collect::<Vec<_>>() != wanted {
                failures.push(format!(
                    "{name} {} keys {keys:?} vs {wanted:?}",
                    row["payload"]
                ));
            }
        }
    }
    (mutants.len(), failures)
}

#[test]
fn the_owners_match_the_original() {
    let fixture = include_str!("../../fixtures/saved-data-owners-vectors.json");
    let (count, failures) = run_scenarios(fixture);
    assert!(count > 40, "fixture is unexpectedly small: {count}");
    assert!(
        failures.is_empty(),
        "{} failures in {count} scenarios:\n{}",
        failures.len(),
        failures
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn loaded_payload_mutants_match_the_original() {
    let fixture = include_str!("../../fixtures/saved-data-owners-vectors.json");
    let (count, failures) = run_mutants(fixture);
    assert!(count > 40, "fixture is unexpectedly small: {count}");
    assert!(
        failures.is_empty(),
        "{} failures in {count} mutants:\n{}",
        failures.len(),
        failures
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The full generated set (`--full-out`):
/// `SAVED_DATA_OWNERS_FILE=<json> cargo test --lib saved_data::owners_tests::explore -- --ignored`.
#[test]
#[ignore]
fn explore() {
    let path = std::env::var("SAVED_DATA_OWNERS_FILE").expect("SAVED_DATA_OWNERS_FILE");
    let fixture = fs::read_to_string(path).unwrap();
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
