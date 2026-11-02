//! Differential check of completion suggestions against
//! `fixtures/commands-brigadier-suggest-vectors.json`, produced by EXECUTING the unchanged
//! Brigadier 1.2.9 (`tools/brigadier_suggest_generate.py`):
//!
//! * seeded random command trees with custom suggestion providers, completed at several
//!   cursors (the result range, every suggestion and the exceptions for impossible cursors);
//! * direct `Suggestions.create` / `merge` / `SuggestionsBuilder` cases, including the hash
//!   code of every suggestion and `apply`;
//! * `String.compareToIgnoreCase` / `compareTo` / `hashCode` / `toLowerCase(Locale.ROOT)` and
//!   the complete `Character.toUpperCase` / `toLowerCase` table of the BMP.

use super::dispatcher::{CommandContext, CommandDispatcher, StringRange};
use super::dispatcher_tests::{build, Events};
use super::exception::{CommandMessage, CommandSyntaxException};
use super::java_text::{
    java_compare_to, java_compare_to_ignore_case, java_string_hash, java_to_lower_case_root, java_to_lower_char,
    java_to_lower_code_point, java_to_upper_char, java_to_upper_code_point,
};
use super::suggestion::{
    Suggestion, SuggestionError, SuggestionProvider, Suggestions, SuggestionsBuilder,
};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../fixtures/commands-brigadier-suggest-vectors.json")).unwrap()
}

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// The provider the Java harness installs for a `"sug"` node description.
pub(super) fn suggestion_provider(spec: &Value) -> SuggestionProvider<i32> {
    let kind = spec["kind"].as_str().unwrap().to_string();
    let words: Vec<String> = spec["words"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|word| word.as_str().unwrap().to_string())
        .collect();
    Rc::new(move |context: &CommandContext<i32>, builder: &mut SuggestionsBuilder| {
        match kind.as_str() {
            "words" => {
                for word in &words {
                    builder.suggest(word);
                }
            }
            "filter" => {
                for word in &words {
                    if units(&java_to_lower_case_root(word)).starts_with(builder.remaining_lower_case_units()) {
                        builder.suggest(word);
                    }
                }
            }
            "ctx" => {
                let text = format!(
                    "in={}|nodes={}|rng={}-{}|src={}|start={}|rem={}",
                    context.input(),
                    context.nodes().len(),
                    context.range().start(),
                    context.range().end(),
                    context.source(),
                    builder.start(),
                    builder.remaining()
                );
                builder.suggest(&text);
            }
            "throw" => {
                return Err(CommandSyntaxException::without_context(CommandMessage::new(
                    "command.unknown.argument",
                    vec![],
                )))
            }
            other => panic!("unknown provider {other}"),
        }
        Ok(builder.build())
    })
}

fn describe_suggestion(suggestion: &Suggestion, command: Option<&str>) -> Value {
    let mut value = json!({
        "r": [suggestion.range().start(), suggestion.range().end()],
        "t": suggestion.text(),
    });
    if let Some(integer) = suggestion.integer_value() {
        value["i"] = json!(integer);
    }
    if let Some(command) = command {
        value["h"] = json!(suggestion.java_hash());
        value["a"] = json!(suggestion.apply(command).unwrap());
    }
    value
}

fn describe(suggestions: &Suggestions, command: Option<&str>) -> Value {
    json!({
        "range": [suggestions.range().start(), suggestions.range().end()],
        "list": suggestions.list().iter().map(|s| describe_suggestion(s, command)).collect::<Vec<_>>(),
    })
}

fn error_name(error: SuggestionError) -> &'static str {
    match error {
        SuggestionError::NoNodeBeforeCursor => "IllegalStateException",
        SuggestionError::StringIndexOutOfBounds => "StringIndexOutOfBoundsException",
    }
}

#[test]
fn completion_suggestions_on_random_trees_match_the_executed_original() {
    let document = fixture();
    let (mut completions, mut non_empty, mut errors, mut providers) = (0, 0, 0, 0);
    for (test_index, test) in document["tests"].as_array().unwrap().iter().enumerate() {
        let events: Events = Rc::new(RefCell::new(Vec::new()));
        let names = Rc::new(Vec::new());
        let mut dispatcher = CommandDispatcher::<i32>::new();
        for tree in test["tree"].as_array().unwrap() {
            let builder = build(tree, &dispatcher, &events, &names);
            dispatcher.register(builder);
        }
        providers += test["tree"].to_string().matches("\"sug\"").count();
        for (case_index, case) in test["cases"].as_array().unwrap().iter().enumerate() {
            let input = case["input"].as_str().unwrap();
            let source = case["source"].as_i64().unwrap() as i32;
            let parse = dispatcher.parse(input, &source);
            for item in case["suggest"].as_array().unwrap() {
                let cursor = item["cursor"].as_u64().unwrap() as usize;
                let context = format!("tree {test_index} case {case_index} source={source} input={input:?} cursor={cursor}");
                completions += 1;
                match (dispatcher.completion_suggestions(&parse, cursor), item.get("throws")) {
                    (Ok(suggestions), None) => {
                        non_empty += usize::from(!suggestions.is_empty());
                        assert_eq!(describe(&suggestions, None), item["result"], "{context}");
                    }
                    (Err(error), Some(expected)) => {
                        errors += 1;
                        assert_eq!(error_name(error), expected.as_str().unwrap(), "{context}");
                    }
                    (outcome, expected) => panic!("{context}: {outcome:?} vs fixture {expected:?}"),
                }
            }
        }
    }
    assert!(completions > 4_000 && non_empty > 2_000 && errors > 50 && providers > 100, "{completions} {non_empty} {errors} {providers}");
}

fn suggestion_from(spec: &Value) -> Suggestion {
    let range = StringRange::between(spec["r"][0].as_u64().unwrap() as usize, spec["r"][1].as_u64().unwrap() as usize);
    match spec.get("i") {
        Some(value) => Suggestion::integer(range, value.as_i64().unwrap() as i32),
        None => Suggestion::new(range, spec["t"].as_str().unwrap()),
    }
}

fn suggestion_list(specs: &Value) -> Vec<Suggestion> {
    specs.as_array().unwrap().iter().map(suggestion_from).collect()
}

fn run_direct(op: &Value) -> Result<Value, SuggestionError> {
    Ok(match op["op"].as_str().unwrap() {
        "create" => {
            let command = op["command"].as_str().unwrap();
            describe(&Suggestions::create(command, &suggestion_list(&op["list"]))?, Some(command))
        }
        "merge" => {
            let command = op["command"].as_str().unwrap();
            let groups = op["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|group| Suggestions::create(command, &suggestion_list(group)))
                .collect::<Result<Vec<_>, _>>()?;
            describe(&Suggestions::merge(command, &groups)?, Some(command))
        }
        "builder" => {
            let input = op["input"].as_str().unwrap();
            let mut builder = SuggestionsBuilder::new(input, op["start"].as_u64().unwrap() as usize)?;
            for step in op["steps"].as_array().unwrap() {
                if let Some(text) = step.get("s") {
                    builder.suggest(text.as_str().unwrap());
                } else if let Some(value) = step.get("i") {
                    builder.suggest_integer(value.as_i64().unwrap() as i32);
                } else if let Some(offset) = step.get("off") {
                    builder = builder.create_offset(offset.as_u64().unwrap() as usize)?;
                } else {
                    builder = builder.restart();
                }
            }
            let mut state = describe(&builder.build(), Some(input));
            state["start"] = json!(builder.start());
            state["remaining"] = json!(builder.remaining());
            state["remainingLower"] = json!(builder.remaining_lower_case());
            state
        }
        "string" => {
            let (a, b) = (op["a"].as_str().unwrap(), op["b"].as_str().unwrap());
            json!({
                "cmp": java_compare_to_ignore_case(&units(a), &units(b)),
                "cmpCase": java_compare_to(&units(a), &units(b)),
                "hash": java_string_hash(&units(a)),
                "lower": java_to_lower_case_root(a),
            })
        }
        other => panic!("unknown direct op {other}"),
    })
}

#[test]
fn suggestion_classes_and_java_string_semantics_match_the_executed_original() {
    let document = fixture();
    let (mut ties, mut integers, mut errors) = (0, 0, 0);
    for (index, case) in document["direct"].as_array().unwrap().iter().enumerate() {
        let op = &case["op"];
        if op["op"] == "charCase" {
            continue;
        }
        let context = format!("direct {index}: {op}");
        let result = &case["result"];
        match (run_direct(op), result.get("throws")) {
            (Ok(actual), None) => {
                assert_eq!(actual, result["ok"], "{context}");
                if let Some(list) = actual.get("list").and_then(Value::as_array) {
                    integers += usize::from(list.iter().any(|s| s.get("i").is_some()));
                    // Two neighbours that differ only by case: the HashSet bucket order decided.
                    ties += list
                        .windows(2)
                        .filter(|w| w[0]["t"] != w[1]["t"] && w[0]["t"].as_str().unwrap().to_lowercase() == w[1]["t"].as_str().unwrap().to_lowercase())
                        .count();
                }
            }
            (Err(error), Some(expected)) => {
                errors += 1;
                assert_eq!(error_name(error), expected.as_str().unwrap(), "{context}");
            }
            (outcome, expected) => panic!("{context}: {outcome:?} vs fixture {expected:?}"),
        }
    }
    assert!(ties > 20 && integers > 20 && errors > 20, "ties {ties}, integers {integers}, errors {errors}");
}

#[test]
fn case_mapping_matches_every_unicode_scalar() {
    let document = fixture();
    let table = document["direct"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["op"]["op"] == "charCase")
        .unwrap()["result"]["ok"]
        .as_array()
        .unwrap();
    let mut expected: Vec<(u32, u32, u32)> = Vec::new();
    for row in table {
        let row: Vec<u64> = row.as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        expected.push((row[0] as u32, row[1] as u32, row[2] as u32));
    }
    let mut wrong = Vec::new();
    for code_point in (0..=0x10FFFFu32).filter(|cp| !(0xD800..=0xDFFF).contains(cp)) {
        let wanted = expected
            .binary_search_by_key(&code_point, |row| row.0)
            .map(|i| (expected[i].1, expected[i].2))
            .unwrap_or((code_point, code_point));
        let actual = (java_to_upper_code_point(code_point), java_to_lower_code_point(code_point));
        if actual != wanted {
            wrong.push((code_point, wanted.0, wanted.1));
        }
    }
    assert!(wrong.is_empty(), "code points where the JVM differs from the derivation: {wrong:?}");
    assert!(expected.len() > 2_500, "{}", expected.len());
    // The BMP wrappers take the same table.
    assert_eq!((java_to_upper_char(0x61), java_to_lower_char(0x41)), (0x41, 0x61));
}
