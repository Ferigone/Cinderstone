//! Differential tests against the original `DimensionDataStorage`.
//!
//! `fixtures/saved-data-vectors.json` holds scenarios and the original's result for every step;
//! `fixtures/saved-data-mutants.json` holds payload mutants loaded through the original factories.
//! Both come from `tools/saved_data_generate.py` (see `docs/reference/saved-data.md`).

use super::file;
use super::storage::{DataFixType, DimensionDataStorage};
use super::{random_sequences_factory, MapIndex, DATA_VERSION, RANDOM_SEQUENCES};
use crate::components::java::resource_location;
use crate::nbt::{JavaString, Tag};
use crate::random::RandomSource;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// Compounds compared as maps (Java's member order is a `HashMap` order), lists in order.
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

fn decode(hex: &str) -> Tag {
    normalized(&Tag::decode_anonymous(&unhex(hex)).expect("fixture NBT"))
}

fn same_tag(actual: &Tag, expected_hex: &str) -> bool {
    normalized(actual) == decode(expected_hex)
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "cinderstone-saved-data-{}-{}",
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

fn run_scenario(case: &Value) -> Vec<String> {
    let dir = TempDir::new();
    let seed = case["world_seed"].as_i64().unwrap();
    let mut storage = DimensionDataStorage::new(dir.path());
    let mut put: HashMap<String, Vec<u8>> = HashMap::new();
    let mut failures = Vec::new();
    let steps = case["steps"].as_array().unwrap();
    let expected = case["expect"].as_array().unwrap();
    for (index, (step, expect)) in steps.iter().zip(expected).enumerate() {
        let kind = step["op"].as_str().unwrap();
        let mut fail = |what: String| {
            failures.push(format!(
                "step {index} {kind} {}: {what}\n  expected {expect}",
                step
            ))
        };
        let file_of = |name: &str| dir.path().join(format!("{name}.dat"));
        match kind {
            "put_snbt" | "put_hex" => {
                let name = step["name"].as_str().unwrap();
                let bytes = if kind == "put_snbt" {
                    unhex(expect["hex"].as_str().unwrap())
                } else {
                    unhex(step["hex"].as_str().unwrap())
                };
                fs::write(file_of(name), &bytes).unwrap();
                put.insert(name.to_owned(), bytes);
            }
            "delete" => {
                let _ = fs::remove_file(file_of(step["name"].as_str().unwrap()));
            }
            "get" | "compute" | "set_dirty" => {
                let name = step["name"].as_str().unwrap();
                let make = kind != "get";
                let (present, dirty, data): (bool, bool, Option<Tag>) = if name == RANDOM_SEQUENCES
                {
                    let factory = random_sequences_factory(seed);
                    let found = if make {
                        Some(storage.compute_if_absent(&factory, name).unwrap())
                    } else {
                        storage.get(&factory, name).unwrap()
                    };
                    match found {
                        Some(data) => {
                            if kind == "set_dirty" {
                                data.set_dirty(true);
                            }
                            (true, data.is_dirty(), Some(data.save()))
                        }
                        None => (false, false, None),
                    }
                } else {
                    let factory = MapIndex::factory();
                    let found = if make {
                        Some(storage.compute_if_absent(&factory, name).unwrap())
                    } else {
                        storage.get(&factory, name).unwrap()
                    };
                    match found {
                        Some(data) => {
                            if kind == "set_dirty" {
                                super::SavedData::set_dirty(data, true);
                            }
                            (
                                true,
                                super::SavedData::is_dirty(data),
                                Some(super::SavedData::save(data)),
                            )
                        }
                        None => (false, false, None),
                    }
                };
                if present != expect["present"].as_bool().unwrap() {
                    fail(format!("present {present}"));
                } else if present {
                    if dirty != expect["dirty"].as_bool().unwrap() {
                        fail(format!("dirty {dirty}"));
                    }
                    if !same_tag(data.as_ref().unwrap(), expect["data"].as_str().unwrap()) {
                        fail(format!("data {:?}", data.unwrap()));
                    }
                }
            }
            "free_map_id" => {
                let index = storage
                    .compute_if_absent(&MapIndex::factory(), "idcounts")
                    .unwrap();
                let id = index.get_free_aux_value_for_map();
                if i64::from(id) != expect["id"].as_i64().unwrap()
                    || !expect["dirty"].as_bool().unwrap()
                {
                    fail(format!("id {id}"));
                }
            }
            "draw" | "reset" | "clear" | "defaults" => {
                let sequences = storage
                    .compute_if_absent(&random_sequences_factory(seed), RANDOM_SEQUENCES)
                    .unwrap();
                let id = step
                    .get("id")
                    .and_then(Value::as_str)
                    .map(resource_location);
                match kind {
                    "draw" => match id.unwrap() {
                        None => {
                            if expect.get("error").is_none() {
                                fail("rejected an id Java accepts".to_owned());
                            }
                        }
                        Some(id) => {
                            let mut random = sequences.get(&id);
                            let draws: Vec<i64> = (0..step["n"].as_i64().unwrap())
                                .map(|_| random.next_i64())
                                .collect();
                            let wanted: Vec<i64> = expect["draws"]
                                .as_array()
                                .map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect())
                                .unwrap_or_default();
                            if draws != wanted
                                || sequences.is_dirty()
                                    != expect["dirty"].as_bool().unwrap_or(false)
                            {
                                fail(format!("draws {draws:?} dirty {}", sequences.is_dirty()));
                            }
                        }
                    },
                    "reset" => match id.unwrap() {
                        None => {
                            if expect.get("error").is_none() {
                                fail("rejected an id Java accepts".to_owned());
                            }
                        }
                        Some(id) => {
                            if step.get("salt").is_some() {
                                sequences.reset_with(
                                    &id,
                                    step["salt"].as_i64().unwrap() as i32,
                                    step["include_world_seed"].as_bool().unwrap(),
                                    step["include_sequence_id"].as_bool().unwrap(),
                                );
                            } else {
                                sequences.reset(&id);
                            }
                            if sequences.is_dirty() != expect["dirty"].as_bool().unwrap() {
                                fail(format!("dirty {}", sequences.is_dirty()));
                            }
                        }
                    },
                    "clear" => {
                        let cleared = sequences.clear();
                        if cleared as i64 != expect["cleared"].as_i64().unwrap() {
                            fail(format!("cleared {cleared}"));
                        }
                    }
                    _ => sequences.set_seed_defaults(
                        step["salt"].as_i64().unwrap() as i32,
                        step["include_world_seed"].as_bool().unwrap(),
                        step["include_sequence_id"].as_bool().unwrap(),
                    ),
                }
            }
            "save" => {
                storage.save();
                let mut names: Vec<String> = fs::read_dir(dir.path())
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                    .collect();
                names.sort();
                let wanted = expect["files"].as_array().unwrap();
                let present: Vec<String> = names
                    .iter()
                    .map(|n| n.strip_suffix(".dat").unwrap_or(n).to_owned())
                    .collect();
                let wanted_names: Vec<&str> =
                    wanted.iter().map(|f| f["name"].as_str().unwrap()).collect();
                if present != wanted_names {
                    fail(format!("files {present:?}"));
                    continue;
                }
                for (name, wanted) in present.iter().zip(wanted) {
                    let bytes = fs::read(file_of(name)).unwrap();
                    let rewritten = put.get(name) != Some(&bytes);
                    if rewritten != wanted["rewritten"].as_bool().unwrap() {
                        fail(format!("{name} rewritten {rewritten}"));
                    }
                    if file::is_gzip(&bytes) != wanted["gzip"].as_bool().unwrap() {
                        fail(format!("{name} gzip {}", file::is_gzip(&bytes)));
                    }
                    match (
                        file::read_root(&bytes),
                        wanted.get("nbt").and_then(Value::as_str),
                    ) {
                        (Ok(tag), Some(hex)) => {
                            if !same_tag(&tag, hex) {
                                fail(format!("{name} content {tag:?}"));
                            }
                        }
                        (Err(_), None) => {}
                        (result, _) => fail(format!("{name} readable: {}", result.is_ok())),
                    }
                }
            }
            "read_tag" => {
                let name = step["name"].as_str().unwrap();
                let fix = if name == RANDOM_SEQUENCES {
                    DataFixType::SavedDataRandomSequences
                } else {
                    DataFixType::SavedDataMapIndex
                };
                let version = step
                    .get("version")
                    .and_then(Value::as_i64)
                    .map_or(DATA_VERSION, |v| v as i32);
                match (
                    storage.read_tag_from_disk(name, fix, version),
                    expect.get("nbt").and_then(Value::as_str),
                ) {
                    (Ok(tag), Some(hex)) => {
                        if !same_tag(&tag, hex) {
                            fail(format!("content {tag:?}"));
                        }
                    }
                    (Err(_), None) => {}
                    (result, _) => fail(format!("ok {}", result.is_ok())),
                }
            }
            other => panic!("unknown op {other}"),
        }
    }
    failures
}

#[test]
fn oracle_scenarios_match() {
    let doc: Value =
        serde_json::from_str(include_str!("../../fixtures/saved-data-vectors.json")).unwrap();
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
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(cases.len() >= 50);
}

fn check_mutants(text: &str, minimum: usize) {
    let doc: Value = serde_json::from_str(text).unwrap();
    let rows = doc["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    for row in rows {
        let name = row[0].as_str().unwrap();
        let seed = row[1].as_i64().unwrap();
        let payload = Tag::decode_anonymous(&unhex(row[2].as_str().unwrap())).unwrap();
        let dir = TempDir::new();
        let root = Tag::Compound(vec![
            (JavaString::from("data"), payload),
            (JavaString::from("DataVersion"), Tag::Int(DATA_VERSION)),
        ]);
        fs::write(
            dir.path().join(format!("{name}.dat")),
            file::write_root(&root).unwrap(),
        )
        .unwrap();
        let mut storage = DimensionDataStorage::new(dir.path());
        let loaded: Option<Tag> = if name == RANDOM_SEQUENCES {
            storage
                .get(&random_sequences_factory(seed), name)
                .unwrap()
                .map(|d| d.save())
        } else {
            storage
                .get(&MapIndex::factory(), name)
                .unwrap()
                .map(|d| super::SavedData::save(d))
        };
        let wanted = row[3].as_bool().unwrap();
        match (&loaded, wanted) {
            (Some(tag), true) => {
                if !same_tag(tag, row[4].as_str().unwrap()) {
                    failures.push(format!(
                        "{name} {}: saved {tag:?}\n  expected {}",
                        row[2], row[4]
                    ));
                }
            }
            (None, false) => {}
            (loaded, wanted) => failures.push(format!(
                "{name} {}: loaded {} but Java says {wanted}",
                row[2],
                loaded.is_some()
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
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(rows.len() >= minimum, "only {} rows", rows.len());
}

#[test]
fn payload_mutants_load_like_the_original() {
    check_mutants(include_str!("../../fixtures/saved-data-mutants.json"), 600);
}

#[test]
fn a_new_object_is_not_written_until_it_is_dirty() {
    let dir = TempDir::new();
    let mut storage = DimensionDataStorage::new(dir.path());
    storage
        .compute_if_absent(&MapIndex::factory(), "idcounts")
        .unwrap();
    storage.save();
    assert!(!dir.path().join("idcounts.dat").exists());
    let index = storage
        .compute_if_absent(&MapIndex::factory(), "idcounts")
        .unwrap();
    assert_eq!(index.get_free_aux_value_for_map(), 0);
    storage.save();
    let bytes = fs::read(dir.path().join("idcounts.dat")).unwrap();
    assert!(file::is_gzip(&bytes));
    let root = file::read_root(&bytes).unwrap();
    assert_eq!(file::data_version(&root, 0), DATA_VERSION);
    assert_eq!(
        root.get("data").and_then(|data| data.get("map")),
        Some(&Tag::Int(0))
    );
    // The write goes through a sibling file that is renamed over the target.
    assert!(!dir.path().join("idcounts.dat_new").exists());
    // Saved once, clean again: a second save leaves the bytes alone.
    storage.save();
    assert_eq!(fs::read(dir.path().join("idcounts.dat")).unwrap(), bytes);
}

#[test]
fn an_unreadable_file_is_cached_as_absent_and_logged() {
    let dir = TempDir::new();
    fs::write(dir.path().join("idcounts.dat"), [0x1f]).unwrap();
    let mut storage = DimensionDataStorage::new(dir.path());
    assert!(storage
        .get(&MapIndex::factory(), "idcounts")
        .unwrap()
        .is_none());
    assert_eq!(storage.diagnostics().len(), 1);
    // Cached: replacing the file does not change the answer until the object is replaced.
    fs::write(
        dir.path().join("idcounts.dat"),
        file::write_root(&Tag::Compound(Vec::new())).unwrap(),
    )
    .unwrap();
    assert!(storage
        .get(&MapIndex::factory(), "idcounts")
        .unwrap()
        .is_none());
    assert_eq!(storage.diagnostics().len(), 1);
}

#[test]
fn files_nobody_asked_for_are_never_rewritten() {
    let dir = TempDir::new();
    let foreign = file::write_root(&Tag::Compound(vec![(
        JavaString::from("data"),
        Tag::Compound(Vec::new()),
    )]))
    .unwrap();
    fs::write(dir.path().join("scoreboard.dat"), &foreign).unwrap();
    let mut storage = DimensionDataStorage::new(dir.path());
    storage
        .compute_if_absent(&MapIndex::factory(), "idcounts")
        .unwrap()
        .get_free_aux_value_for_map();
    storage.save();
    assert_eq!(
        fs::read(dir.path().join("scoreboard.dat")).unwrap(),
        foreign
    );
}

#[test]
fn a_name_cannot_leave_the_data_folder() {
    let dir = TempDir::new();
    let mut storage = DimensionDataStorage::new(dir.path());
    for name in ["../level", "a/b", "a\\b", "", ".."] {
        assert!(storage.get(&MapIndex::factory(), name).is_err(), "{name:?}");
    }
}

#[test]
fn a_cached_object_of_another_type_is_reported() {
    let dir = TempDir::new();
    let mut storage = DimensionDataStorage::new(dir.path());
    storage
        .compute_if_absent(&MapIndex::factory(), "idcounts")
        .unwrap();
    assert!(storage
        .get(&random_sequences_factory(1), "idcounts")
        .is_err());
}

#[test]
fn random_sequences_resume_after_a_save_and_reload() {
    let dir = TempDir::new();
    let first = {
        let mut storage = DimensionDataStorage::new(dir.path());
        let sequences = storage
            .compute_if_absent(&random_sequences_factory(99), RANDOM_SEQUENCES)
            .unwrap();
        let first = sequences.get("minecraft:chests/x").next_i64();
        storage.save();
        first
    };
    let mut storage = DimensionDataStorage::new(dir.path());
    let sequences = storage
        .get(&random_sequences_factory(99), RANDOM_SEQUENCES)
        .unwrap()
        .unwrap();
    let mut continuing = crate::loot::sequences::RandomSequences::new(99);
    assert_eq!(continuing.get("minecraft:chests/x").next_i64(), first);
    assert_eq!(
        sequences.get("minecraft:chests/x").next_i64(),
        continuing.get("minecraft:chests/x").next_i64()
    );
}

#[test]
fn the_random_sequence_settings_fix_wraps_old_data() {
    let old = Tag::Compound(vec![
        (
            JavaString::from("data"),
            Tag::Compound(vec![(JavaString::from("salt"), Tag::Int(3))]),
        ),
        (JavaString::from("DataVersion"), Tag::Int(3000)),
    ]);
    let fixed = DataFixType::SavedDataRandomSequences
        .update(old.clone(), 3000, DATA_VERSION)
        .unwrap();
    assert_eq!(
        fixed
            .get("data")
            .and_then(|data| data.get("sequences"))
            .and_then(|s| s.get("salt")),
        Some(&Tag::Int(3))
    );
    assert_eq!(
        DataFixType::SavedDataRandomSequences
            .update(old.clone(), 3565, DATA_VERSION)
            .unwrap(),
        old
    );
    assert_eq!(
        DataFixType::SavedDataMapIndex
            .update(old.clone(), 1, DATA_VERSION)
            .unwrap(),
        old
    );
}
