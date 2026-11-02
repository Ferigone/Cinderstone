//! Differential tests against the original `ComponentSerialization.CODEC`.
//!
//! `fixtures/component-oracle-vectors.json` and `fixtures/component-parse-parity.json` come from
//! `tools/component_generate.py`; the oracle is documented in
//! `docs/reference/components-text.md`.

use super::text::{Component, ComponentError};
use serde_json::Value;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Compares two JSON documents, treating numbers by value (Java prints floats
/// with `Float.toString`, Rust with the shortest `f64` form).
fn same_json(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            a == b || (a as f32 == b as f32 && (a as f32).is_finite())
        }
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_json(x, y)),
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(key, x)| b.get(key).is_some_and(|y| same_json(x, y)))
        }
        _ => left == right,
    }
}

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).expect("fixture json")
}

#[test]
fn oracle_vectors_match() {
    let doc = fixture(include_str!("../../fixtures/component-oracle-vectors.json"));
    let cases = doc["cases"].as_array().expect("cases");
    let (mut compared, mut unsupported) = (0, 0);
    let mut failures = Vec::new();
    for case in cases {
        let input = &case["j"];
        let result = Component::from_json(input);
        let java_ok = case["ok"].as_bool().unwrap();
        match (&result, java_ok) {
            (Err(ComponentError::Unsupported(_)), _) => unsupported += 1,
            (Ok(component), true) => {
                compared += 1;
                let nbt = hex(&component.to_network_nbt().unwrap());
                if nbt != case["nbt"].as_str().unwrap() {
                    failures.push(format!("NBT {input}\n  rust {nbt}\n  java {}", case["nbt"].as_str().unwrap()));
                }
                let expected: Value = serde_json::from_str(case["json"].as_str().unwrap()).unwrap();
                if !same_json(&component.to_json(), &expected) {
                    failures.push(format!("JSON {input}\n  rust {}\n  java {expected}", component.to_json()));
                }
            }
            (Ok(component), false) => failures.push(format!(
                "accepted {input} -> {} but Java says {}",
                component.to_json(),
                case["err"]
            )),
            (Err(error), true) => failures.push(format!("rejected {input}: {error}; Java accepts")),
            (Err(_), false) => compared += 1,
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.iter().take(25).cloned().collect::<Vec<_>>().join("\n"));
    assert!(compared > 350, "compared only {compared} of {} (unsupported {unsupported})", cases.len());
}

#[test]
fn parse_parity_net_agrees() {
    check_parity(include_str!("../../fixtures/component-parse-parity.json"));
}

fn check_parity(text: &str) {
    let doc = fixture(text);
    let cases = doc["cases"].as_array().expect("cases");
    let (mut compared, mut unsupported) = (0usize, 0usize);
    let mut failures = Vec::new();
    for pair in cases {
        let input = &pair[0];
        let java_ok = pair[1].as_bool().unwrap();
        match Component::from_json(input) {
            Err(ComponentError::Unsupported(_)) => unsupported += 1,
            Ok(component) => {
                compared += 1;
                if !java_ok {
                    failures.push(format!("accepted but Java rejects: {input}"));
                } else if let Some(expected) = pair.get(2).and_then(Value::as_str) {
                    let nbt = hex(&component.to_network_nbt().unwrap());
                    if nbt != expected {
                        failures.push(format!("NBT differs for {input}\n  rust {nbt}\n  java {expected}"));
                    }
                }
            }
            Err(error) => {
                compared += 1;
                if java_ok {
                    failures.push(format!("rejected but Java accepts: {input} ({error})"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} disagree ({unsupported} unsupported):\n{}",
        failures.len(),
        cases.len(),
        failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n")
    );
    eprintln!("component parity: {compared} compared, {unsupported} unsupported (show_item/show_entity), {} cases", cases.len());
    assert!(compared * 10 > cases.len() * 9, "too many unsupported: {unsupported} of {}", cases.len());
}

/// Full-set run: `COMPONENT_PARITY_FILE=<json> cargo test --lib components::tests::explore_parity -- --ignored`
#[test]
#[ignore]
fn explore_parity() {
    let path = std::env::var("COMPONENT_PARITY_FILE").expect("COMPONENT_PARITY_FILE");
    check_parity(&std::fs::read_to_string(path).unwrap());
}

fn parse(json: &str) -> Component {
    Component::from_json(&serde_json::from_str(json).unwrap()).unwrap()
}

fn bytes(json: &str) -> String {
    hex(&parse(json).to_network_nbt().unwrap())
}

#[test]
fn plain_text_collapses_to_a_string_tag() {
    assert_eq!(bytes(r#""hello""#), "08000568656c6c6f");
    assert_eq!(bytes(r#"{"text":"hello"}"#), "08000568656c6c6f");
    assert_eq!(parse(r#"{"text":"hello"}"#).to_json(), Value::String("hello".to_owned()));
    // A style or a sibling prevents the collapse.
    assert_eq!(parse(r#"{"text":"a","extra":["b"]}"#).collapse_to_string(), None);
    assert_eq!(parse(r#"{"text":"a","bold":false}"#).collapse_to_string(), None);
}

#[test]
fn array_form_appends_siblings_to_the_first_element() {
    let component = parse(r#"["a",{"text":"b","bold":true},"c"]"#);
    assert_eq!(component.siblings.len(), 2);
    assert_eq!(component.siblings[0].style.bold, Some(true));
}

#[test]
fn colors_compare_by_value_and_keep_their_spelling() {
    let named = parse(r##"{"text":"a","color":"gold"}"##);
    let hex = parse(r##"{"text":"a","color":"#FFAA00"}"##);
    assert_eq!(named.style.color, hex.style.color);
    assert_eq!(named.to_json()["color"], "gold");
    assert_eq!(hex.to_json()["color"], "#FFAA00");
    assert_eq!(parse(r##"{"text":"a","color":"#0000ff"}"##).to_json()["color"], "#0000FF");
    for bad in ["#1000000", "#-1", "#", "#G", "RED", ""] {
        let value: Value = serde_json::json!({"text": "a", "color": bad});
        assert!(Component::from_json(&value).is_err(), "{bad}");
    }
}

#[test]
fn only_booleans_are_booleans() {
    let value: Value = serde_json::json!({"text": "a", "bold": 1});
    assert!(Component::from_json(&value).is_err());
    let value: Value = serde_json::json!({"text": "a", "bold": true});
    assert_eq!(Component::from_json(&value).unwrap().style.bold, Some(true));
}

#[test]
fn legacy_hover_value_is_used_only_when_contents_fails() {
    let legacy = parse(r#"{"text":"a","hoverEvent":{"action":"show_text","value":"old"}}"#);
    assert_eq!(legacy.to_json()["hoverEvent"]["contents"], "old");
    let both = parse(r#"{"text":"a","hoverEvent":{"action":"show_text","contents":"new","value":"old"}}"#);
    assert_eq!(both.to_json()["hoverEvent"]["contents"], "new");
    let fallback = parse(r#"{"text":"a","hoverEvent":{"action":"show_text","contents":1,"value":"old"}}"#);
    assert_eq!(fallback.to_json()["hoverEvent"]["contents"], "old");
    let neither: Value = serde_json::json!({"text": "a", "hoverEvent": {"action": "show_text", "contents": 1}});
    assert!(Component::from_json(&neither).is_err());
}

#[test]
fn item_and_entity_hovers_are_explicitly_unsupported() {
    for action in ["show_item", "show_entity"] {
        let value: Value = serde_json::json!({"text": "a", "hoverEvent": {"action": action, "contents": {"id": "minecraft:stone"}}});
        assert!(matches!(Component::from_json(&value), Err(ComponentError::Unsupported(_))), "{action}");
    }
}

#[test]
fn open_file_clicks_are_not_accepted() {
    let value: Value = serde_json::json!({"text": "a", "clickEvent": {"action": "open_file", "value": "x"}});
    assert!(matches!(Component::from_json(&value), Err(ComponentError::Codec(_))));
}

#[test]
fn translate_arguments_narrow_like_json_ops_convert_to() {
    // 1 -> byte, 1000 -> short, 100000 -> int, 2^40 -> long, 1.5 -> float, 0.1 -> double.
    let wire = bytes(r#"{"translate":"k","with":[1,"a"]}"#);
    assert!(wire.contains("0a00000002"), "bytes then string wrap as compounds: {wire}");
    // A list of bytes is a byte array, a list of ints an int array, longs a long array.
    assert!(bytes(r#"{"translate":"k","with":[1,2]}"#).contains("07"));
    assert!(bytes(r#"{"translate":"k","with":[100000,200000]}"#).contains("0b"));
    assert!(bytes(r#"{"translate":"k","with":[1099511627776,2]}"#).contains("0a00000002"));
    let json = parse(r#"{"translate":"k","with":[1.5,0.1]}"#).to_json();
    assert_eq!(json["with"], serde_json::json!([1.5, 0.1]));
}

#[test]
fn an_object_argument_holding_null_fails_like_the_original() {
    let value: Value = serde_json::json!({"translate": "k", "with": [{"text": "q", "bold": null}]});
    assert!(Component::from_json(&value).is_err());
    let value: Value = serde_json::json!({"translate": "k", "with": [{"text": "q", "extra": [{"text": "x"}]}]});
    assert!(Component::from_json(&value).is_ok());
}

#[test]
fn resource_locations_follow_the_original_split() {
    for (font, expected) in [(":x", "minecraft:x"), ("x:", "x:"), ("", "minecraft:"), ("uniform", "minecraft:uniform")] {
        let value: Value = serde_json::json!({"text": "a", "font": font});
        assert_eq!(Component::from_json(&value).unwrap().style.font.as_deref(), Some(expected), "{font}");
    }
    for bad in ["A:b", "a:B", "a b", "a:b:c"] {
        let value: Value = serde_json::json!({"text": "a", "font": bad});
        assert!(Component::from_json(&value).is_err(), "{bad}");
    }
}

#[test]
fn compound_keys_follow_hash_map_order() {
    // text(11) bold(11) strikethrough(11): the style members were inserted in reverse
    // declaration order, so strikethrough precedes bold in the bucket.
    let wire = bytes(r#"{"text":"f","bold":true,"strikethrough":true}"#);
    let strike = wire.find(&hex(b"strikethrough")).unwrap();
    let bold = wire.find(&hex(b"bold")).unwrap();
    let text = wire.find(&hex(b"text")).unwrap();
    assert!(text < strike && strike < bold, "{wire}");
}
