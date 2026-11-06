use super::progress::{
    parse_progress_file, write_progress_file, AdvancementProgress, ProgressError, DATA_VERSION,
    LAST_FIXER_VERSION,
};
use super::requirements::AdvancementRequirements;
use super::time::{format_obtained, parse_obtained};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn groups(lists: &[&[&str]]) -> AdvancementRequirements {
    AdvancementRequirements::new(
        lists
            .iter()
            .map(|list| list.iter().map(|name| (*name).to_owned()).collect())
            .collect(),
    )
}

#[test]
fn requirements_are_an_and_of_ors() {
    let requirements = groups(&[&["a", "b"], &["c"]]);
    assert!(requirements.test(|name| name == "a" || name == "c"));
    assert!(requirements.test(|name| name == "b" || name == "c"));
    assert!(!requirements.test(|name| name == "a"));
    assert!(!requirements.test(|_| false));
    assert_eq!(requirements.count(|name| name == "a"), 1);
    assert_eq!(requirements.count(|_| true), 2);
    assert_eq!(requirements.len(), 2);
    // No requirement at all is never met, an empty group can never be satisfied.
    assert!(!AdvancementRequirements::default().test(|_| true));
    assert!(!groups(&[&[]]).test(|_| true));
    assert_eq!(groups(&[&[]]).count(|_| true), 0);
    assert_eq!(
        AdvancementRequirements::all_of(["x", "y"]),
        groups(&[&["x"], &["y"]])
    );
    assert_eq!(
        AdvancementRequirements::any_of(["x", "y"]),
        groups(&[&["x", "y"]])
    );
}

#[test]
fn requirements_validate_against_the_criteria_names() {
    let criteria: BTreeSet<&str> = ["a", "b"].into_iter().collect();
    assert!(groups(&[&["a"], &["b"]]).validate(&criteria).is_ok());
    assert!(groups(&[&["a", "b"]]).validate(&criteria).is_ok());
    assert!(groups(&[&["a"]]).validate(&criteria).is_err());
    assert!(groups(&[&["a"], &["b"], &["c"]])
        .validate(&criteria)
        .is_err());
    let none = BTreeSet::new();
    assert!(AdvancementRequirements::default().validate(&none).is_ok());
    assert!(groups(&[&[]]).validate(&none).is_err());
    assert!(groups(&[&[]]).validate(&criteria).is_err());
}

#[test]
fn progress_follows_the_requirements() {
    let mut progress = AdvancementProgress::new();
    progress.update(groups(&[&["a"], &["b", "c"]]));
    assert!(!progress.is_done() && !progress.has_progress());
    assert_eq!(progress.percent(), 0.0);
    assert!(progress.grant_progress("a", 100));
    assert!(!progress.grant_progress("a", 200), "already done");
    assert!(!progress.grant_progress("zzz", 200), "unknown criterion");
    assert_eq!(progress.percent(), 0.5);
    assert!(progress.has_progress() && !progress.is_done());
    assert!(progress.grant_progress("c", 50));
    assert!(progress.is_done());
    assert_eq!(progress.percent(), 1.0);
    assert_eq!(progress.first_progress_date(), Some(50));
    assert!(progress.revoke_progress("a"));
    assert!(!progress.revoke_progress("a"));
    assert!(!progress.is_done());
    // A new requirement list keeps what it still names and forgets the rest.
    progress.update(groups(&[&["c"], &["d"]]));
    assert_eq!(progress.criterion("c").and_then(|c| c.obtained), Some(50));
    assert!(progress.criterion("a").is_none() && progress.criterion("d").is_some());
    assert_eq!(AdvancementProgress::new().percent(), 0.0);
}

#[test]
fn obtained_times_read_like_date_time_formatter() {
    assert_eq!(parse_obtained("1970-01-01 00:00:00 +0000"), Some(0));
    assert_eq!(
        parse_obtained("2024-03-04 05:06:07 +0100"),
        Some(1_709_525_167)
    );
    assert_eq!(
        parse_obtained("2024-03-04 05:06:07 -0000"),
        Some(1_709_528_767)
    );
    // SMART resolving: the day is clamped, 24:00:00 is the next midnight.
    assert_eq!(
        parse_obtained("2024-02-30 24:00:00 -0800"),
        parse_obtained("2024-03-01 00:00:00 -0800")
    );
    assert_eq!(
        parse_obtained("2023-02-31 12:00:00 +0000"),
        parse_obtained("2023-02-28 12:00:00 +0000")
    );
    assert!(parse_obtained("2024-01-01 24:00:01 +0000").is_none());
    assert!(parse_obtained("2024-01-01 24:01:00 +0000").is_none());
    assert!(parse_obtained("2024-01-01 23:59:60 +0000").is_none());
    // Years: four digits, or a sign and more than four.
    assert!(parse_obtained("+10000-01-01 00:00:00 +0000").is_some());
    assert!(parse_obtained("10000-01-01 00:00:00 +0000").is_none());
    assert!(parse_obtained("+2024-01-01 00:00:00 +0000").is_none());
    assert!(parse_obtained("0000-01-01 00:00:00 +0000").is_none());
    assert!(parse_obtained("-0001-01-01 00:00:00 +0000").is_none());
    assert!(parse_obtained("+1000000000-01-01 00:00:00 +0000").is_none());
    assert!(parse_obtained("+999999999-12-31 23:59:59 +0000").is_some());
    // Offsets: `+HHMM` only, at most 18:00.
    assert!(parse_obtained("2024-01-01 12:00:00 +1800").is_some());
    assert!(parse_obtained("2024-01-01 12:00:00 +1801").is_none());
    assert!(parse_obtained("2024-01-01 12:00:00 +01:00").is_none());
    assert!(parse_obtained("2024-01-01 12:00:00 Z").is_none());
    assert!(parse_obtained("2024-1-01 12:00:00 +0000").is_none());
}

#[test]
fn obtained_times_are_written_in_utc() {
    assert_eq!(
        format_obtained(0).as_deref(),
        Some("1970-01-01 00:00:00 +0000")
    );
    assert_eq!(
        format_obtained(1_709_525_167).as_deref(),
        Some("2024-03-04 04:06:07 +0000")
    );
    assert_eq!(
        format_obtained(-1).as_deref(),
        Some("1969-12-31 23:59:59 +0000")
    );
    // Years after 9999 carry a sign; the year of the era is printed for years before 1.
    let ten_thousand = parse_obtained("+10000-01-01 00:00:00 +0000").unwrap();
    assert_eq!(
        format_obtained(ten_thousand).as_deref(),
        Some("+10000-01-01 00:00:00 +0000")
    );
    let year_zero = parse_obtained("0001-01-01 00:00:00 +0100").unwrap();
    assert_eq!(
        format_obtained(year_zero).as_deref(),
        Some("0001-12-31 23:00:00 +0000")
    );
    // Every instant read back is written in a form that reads back to the same instant, as long as
    // the year is a CE one.
    for text in [
        "2024-03-04 05:06:07 +0100",
        "1999-12-31 23:59:59 -1800",
        "+12345-06-07 08:09:10 +0000",
    ] {
        let seconds = parse_obtained(text).unwrap();
        assert_eq!(
            parse_obtained(&format_obtained(seconds).unwrap()),
            Some(seconds)
        );
    }
}

#[test]
fn a_progress_file_round_trips_through_the_codec() {
    let text = r#"{"minecraft:story/root":{"criteria":{"crafting_table":"2024-03-04 05:06:07 +0100"},"done":false},
                   "custom:thing":{"criteria":{}},"DataVersion":3953}"#;
    let entries = parse_progress_file(text).unwrap();
    assert_eq!(entries.len(), 2);
    let mut kept = Vec::new();
    for (id, mut progress) in entries {
        progress.update(groups(&[&["crafting_table"]]));
        kept.push((id, progress));
    }
    let written =
        write_progress_file(kept.iter().map(|(id, progress)| (id.as_str(), progress))).unwrap();
    let expected: Value = serde_json::from_str(
        r#"{"minecraft:story/root":{"criteria":{"crafting_table":"2024-03-04 04:06:07 +0000"},"done":true},"DataVersion":3953}"#,
    )
    .unwrap();
    assert_eq!(written, expected);
}

#[test]
fn files_older_than_the_last_fixer_need_the_data_fixers() {
    let old = r#"{"minecraft:recipes/misc/sugar":{"criteria":{"x":"2024-03-04 05:06:07 +0100"}},"DataVersion":2000}"#;
    assert_eq!(
        parse_progress_file(old),
        Err(ProgressError::DataFixerRequired(2000))
    );
    let missing = r#"{"a:b":{"criteria":{}}}"#;
    assert_eq!(
        parse_progress_file(missing),
        Err(ProgressError::DataFixerRequired(1343))
    );
    let current = format!(r#"{{"a:b":{{"criteria":{{}}}},"DataVersion":{LAST_FIXER_VERSION}}}"#);
    assert!(parse_progress_file(&current).is_ok());
    let newer = format!(
        r#"{{"a:b":{{"criteria":{{}}}},"DataVersion":{}}}"#,
        DATA_VERSION + 500
    );
    assert!(parse_progress_file(&newer).is_ok());
}

#[test]
fn gson_reads_one_value_and_skips_a_byte_order_mark() {
    let tail = "\u{feff}{\"a:b\":{\"criteria\":{}},\"DataVersion\":3953} trailing garbage";
    assert_eq!(parse_progress_file(tail).unwrap().len(), 1);
    assert!(parse_progress_file("").is_err());
    assert!(parse_progress_file("[]").is_err());
}

/// Run one oracle row through the port and compare. Returns an error text on a mismatch.
fn check_row(row: &Value) -> Result<(), String> {
    let text = row["text"].as_str().ok_or("text")?;
    let requirements: BTreeMap<String, Vec<Vec<String>>> = row["requirements"]
        .as_object()
        .ok_or("requirements")?
        .iter()
        .map(|(id, value)| {
            let lists = value
                .as_array()
                .unwrap()
                .iter()
                .map(|group| {
                    group
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|n| n.as_str().unwrap().to_owned())
                        .collect()
                })
                .collect();
            (id.clone(), lists)
        })
        .collect();
    let java_ok = row["ok"].as_bool().ok_or("ok")?;
    let java_version = row["version"].as_i64();
    let outcome = parse_progress_file(text);
    match (&outcome, java_version) {
        (Err(ProgressError::DataFixerRequired(version)), Some(java)) => {
            if i64::from(*version) != java || java >= i64::from(LAST_FIXER_VERSION) {
                return Err(format!(
                    "fixer required at {version}, original read version {java}"
                ));
            }
            return Ok(());
        }
        (Err(ProgressError::DataFixerRequired(version)), None) => {
            return Err(format!(
                "fixer required at {version} but the original found no document"
            ));
        }
        _ => {}
    }
    if let Some(java) = java_version {
        if java < i64::from(LAST_FIXER_VERSION) && java_ok {
            return Err(format!(
                "original data-fixed a version {java} file, the port did not ask for fixers"
            ));
        }
    }
    match outcome {
        Err(ProgressError::Codec(reason)) => {
            if java_ok {
                return Err(format!("port rejected ({reason}), original accepted"));
            }
            Ok(())
        }
        Err(ProgressError::DataFixerRequired(_) | ProgressError::Encode(_)) => unreachable!(),
        Ok(entries) => {
            let mut kept = Vec::new();
            let mut done = serde_json::Map::new();
            for (id, mut progress) in entries {
                if let Some(requirement) = requirements.get(&id) {
                    progress.update(AdvancementRequirements::new(requirement.clone()));
                    done.insert(id.clone(), Value::Bool(progress.is_done()));
                    if progress.has_progress() {
                        kept.push((id, progress));
                    }
                } else {
                    done.insert(id, Value::Bool(progress.is_done()));
                }
            }
            let written = match write_progress_file(
                kept.iter().map(|(id, progress)| (id.as_str(), progress)),
            ) {
                Ok(written) => written,
                // An instant `LocalDate` cannot print: the original fails in its encoder, which its
                // row records as a rejection.
                Err(ProgressError::Encode(_)) if !java_ok => return Ok(()),
                Err(error) => {
                    return Err(format!(
                        "port cannot write ({error:?}), original ok={java_ok}"
                    ))
                }
            };
            if !java_ok {
                return Err("port accepted and wrote, original rejected".to_owned());
            }
            if written != row["json"] {
                return Err(format!(
                    "document differs: port {written} original {}",
                    row["json"]
                ));
            }
            if Value::Object(done.clone()) != row["done"] {
                return Err(format!(
                    "done differs: port {} original {}",
                    Value::Object(done),
                    row["done"]
                ));
            }
            Ok(())
        }
    }
}

fn run_rows(fixture: &str) -> (usize, Vec<String>) {
    let document: Value = serde_json::from_str(fixture).expect("fixture is JSON");
    let rows = document["rows"].as_array().expect("rows");
    let mut failures = Vec::new();
    for row in rows {
        if let Err(message) = check_row(row) {
            failures.push(format!("[{}] {:?}: {message}", row["note"], row["text"]));
        }
    }
    (rows.len(), failures)
}

#[test]
fn progress_vectors_agree_with_the_original() {
    let (count, failures) = run_rows(include_str!(
        "../../fixtures/advancement-progress-vectors.json"
    ));
    assert!(count > 400, "fixture is unexpectedly small: {count}");
    assert!(
        failures.is_empty(),
        "{} of {count} rows disagree:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The full generated set (`--full-out`), run with
/// `ADVANCEMENT_PARITY_FILE=<json> cargo test --lib advancements::tests::explore_parity -- --ignored`.
#[test]
#[ignore]
fn explore_parity() {
    let path = std::env::var("ADVANCEMENT_PARITY_FILE").expect("ADVANCEMENT_PARITY_FILE");
    let (count, failures) = run_rows(&std::fs::read_to_string(path).unwrap());
    for failure in failures.iter().take(40) {
        eprintln!("{failure}");
    }
    assert!(
        failures.is_empty(),
        "{} of {count} rows disagree",
        failures.len()
    );
}
