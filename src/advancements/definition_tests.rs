//! Advancement definitions against the original's `Advancement.CODEC`.

use super::definition::{parse, DefinitionError};
use serde_json::Value;

/// Every shipped advancement parses.
#[test]
fn vanilla_advancements_parse() {
    let doc: Value = serde_json::from_str(include_str!("../../fixtures/advancements-vanilla-1.21.json")).unwrap();
    let files = doc["advancement"].as_object().unwrap();
    assert_eq!(files.len(), 1399);
    let mut failures = Vec::new();
    for (id, text) in files {
        let json = crate::components::lenient::parse(text.as_str().unwrap()).unwrap();
        if let Err(error) = parse(&json) {
            failures.push(format!("{id}: {error}"));
        }
    }
    assert!(failures.is_empty(), "{} vanilla advancements fail:\n{}", failures.len(), failures[..failures.len().min(20)].join("\n"));
}

/// One-step mutants (or `ADVANCEMENT_PARSE_VECTORS`, a full local run): accepted and rejected
/// exactly as the original does; parts this port does not read answer `Unsupported`, counted and
/// bounded.
#[test]
fn advancement_mutants_match_the_original_codec() {
    let text = match std::env::var("ADVANCEMENT_PARSE_VECTORS") {
        Ok(path) => std::fs::read_to_string(path).unwrap(),
        Err(_) => include_str!("../../fixtures/advancement-parse-vectors.json").to_owned(),
    };
    let doc: Value = serde_json::from_str(&text).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    let mut unsupported = 0usize;
    for case in cases {
        let input = case[0].as_str().unwrap();
        let expected = case[1].as_bool().unwrap();
        let json = crate::components::lenient::parse(input).unwrap();
        match (parse(&json), expected) {
            (Err(DefinitionError::Unsupported(_)), _) => unsupported += 1,
            (Ok(_), true) | (Err(DefinitionError::Codec(_)), false) => {}
            (ours, _) => failures.push(format!("{input}: ours {ours:?}, original accepts: {expected}")),
        }
    }
    eprintln!("advancement mutants: {} checked, {unsupported} unsupported", cases.len());
    if let Ok(path) = std::env::var("ADVANCEMENT_FAILURES") {
        std::fs::write(path, failures.join("\n")).unwrap();
    }
    assert!(failures.is_empty(), "{} of {} differ:\n{}", failures.len(), cases.len(), failures[..failures.len().min(20)].join("\n"));
    assert!(unsupported * 50 <= cases.len(), "{unsupported} of {} hit unported parts", cases.len());
}
