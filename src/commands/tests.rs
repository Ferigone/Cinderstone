//! Differential check against `fixtures/commands-brigadier-vectors.json`, produced by
//! EXECUTING the unchanged Brigadier 1.2.9 with the server's exception provider
//! (`tools/brigadier_generate.py`). Every case compares the result value, the cursor
//! afterwards and, for failures, the message key, typed arguments, English text,
//! `getMessage()`, `getContext()` and the exception cursor.

use super::arguments::{
    ArgumentType, BoolArgumentType, DoubleArgumentType, FloatArgumentType, IntegerArgumentType,
    LongArgumentType, StringArgumentType,
};
use super::exception::CommandSyntaxException;
use super::java_text::{java_double_to_string, java_float_to_string, java_is_whitespace};
use super::string_reader::StringReader;
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../fixtures/commands-brigadier-vectors.json")).unwrap()
}

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn hex32(value: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(value, 16).unwrap())
}

fn hex64(value: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(value, 16).unwrap())
}

fn check_error(case: &Value, error: &CommandSyntaxException, context: &str) {
    let expected = &case["err"];
    let message = error.raw_message();
    assert_eq!(message.key, expected["key"].as_str().unwrap(), "{context}: key");
    let args: Vec<(String, String)> = message
        .args
        .iter()
        .map(|a| (a.java_type().to_string(), a.java_to_string()))
        .collect();
    let wanted: Vec<(String, String)> = expected["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["t"].as_str().unwrap().to_string(),
                a["v"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(args, wanted, "{context}: arguments");
    assert_eq!(message.english(), expected["english"].as_str().unwrap(), "{context}: english");
    assert_eq!(error.message(), expected["message"].as_str().unwrap(), "{context}: message");
    assert_eq!(
        error.context().as_deref(),
        expected["context"].as_str(),
        "{context}: context"
    );
    assert_eq!(
        error.cursor() as i64,
        expected["errCursor"].as_i64().unwrap(),
        "{context}: exception cursor"
    );
    assert_eq!(
        error.input().as_deref(),
        Some(case["input"].as_str().unwrap()),
        "{context}: exception input"
    );
}

/// Run one operation and render the success value the way the oracle did.
fn apply(case: &Value, reader: &mut StringReader) -> Result<Value, CommandSyntaxException> {
    let op = case["op"].as_str().unwrap();
    let text = |key: &str| case[key].as_str().unwrap().to_string();
    Ok(match op {
        "readInt" => Value::from(reader.read_int()?),
        "readLong" => Value::from(reader.read_long()?.to_string()),
        "readFloat" => Value::from(format!("{:x}", reader.read_float()?.to_bits())),
        "readDouble" => Value::from(format!("{:x}", reader.read_double()?.to_bits())),
        "readBoolean" => Value::from(reader.read_boolean()?),
        "readUnquotedString" => Value::from(reader.read_unquoted_string()),
        "readQuotedString" => Value::from(reader.read_quoted_string()?),
        "readString" => Value::from(reader.read_string()?),
        "readStringUntil" => {
            Value::from(reader.read_string_until(units(&text("a"))[0])?)
        }
        "skipWhitespace" => {
            reader.skip_whitespace();
            Value::from(reader.remaining())
        }
        "expect" => {
            reader.expect(units(&text("a"))[0])?;
            Value::from(true)
        }
        "views" => serde_json::json!({
            "read": reader.read_so_far(),
            "remaining": reader.remaining(),
            "remainingLength": reader.remaining_length(),
            "totalLength": reader.total_length(),
            "canRead": reader.can_read(),
            "canRead3": reader.can_read_n(3),
        }),
        "arg.bool" => Value::from(BoolArgumentType.parse(reader)?),
        "arg.int" => Value::from(
            IntegerArgumentType::new(
                case["min"].as_i64().unwrap() as i32,
                case["max"].as_i64().unwrap() as i32,
            )
            .parse(reader)?,
        ),
        "arg.long" => Value::from(
            LongArgumentType::new(
                text("min").parse().unwrap(),
                text("max").parse().unwrap(),
            )
            .parse(reader)?
            .to_string(),
        ),
        "arg.float" => Value::from(format!(
            "{:x}",
            FloatArgumentType::new(hex32(&text("min")), hex32(&text("max")))
                .parse(reader)?
                .to_bits()
        )),
        "arg.double" => Value::from(format!(
            "{:x}",
            DoubleArgumentType::new(hex64(&text("min")), hex64(&text("max")))
                .parse(reader)?
                .to_bits()
        )),
        "arg.string" => {
            let argument = match case["type"].as_str().unwrap() {
                "word" => StringArgumentType::word(),
                "string" => StringArgumentType::string(),
                "greedy" => StringArgumentType::greedy_string(),
                other => panic!("unknown string type {other}"),
            };
            Value::from(argument.parse(reader)?)
        }
        "escapeIfRequired" => Value::from(StringArgumentType::escape_if_required(&text("input"))),
        "d2s" => Value::from(java_double_to_string(hex64(&text("input")))),
        "f2s" => Value::from(java_float_to_string(hex32(&text("input")))),
        other => panic!("unknown op {other}"),
    })
}

#[test]
fn reader_and_primitive_arguments_match_the_executed_original() {
    let document = fixture();
    let cases = document["cases"].as_array().unwrap();
    assert!(cases.len() > 10_000);
    let mut errors = 0;
    for (index, case) in cases.iter().enumerate() {
        let context = format!(
            "case {index} {} input={:?} cursor={:?} min={:?} max={:?}",
            case["op"], case["input"], case["cursor"], case["min"], case["max"]
        );
        let mut reader = StringReader::new(case["input"].as_str().unwrap());
        if let Some(cursor) = case["cursor"].as_u64() {
            reader.set_cursor(cursor as usize);
        }
        if case["op"] == "read" {
            let value = reader.read();
            if case.get("throws").is_some() {
                assert!(value.is_none(), "{context}: Java threw");
            } else {
                assert_eq!(value.map(i64::from), case["ok"].as_i64(), "{context}");
            }
            assert_eq!(reader.cursor() as i64, case["cur"].as_i64().unwrap(), "{context}: cursor");
            continue;
        }
        let result = apply(case, &mut reader);
        if case.get("err").is_some() {
            let error = result.expect_err(&context);
            check_error(case, &error, &context);
            errors += 1;
        } else {
            assert_eq!(result.expect(&context), case["ok"], "{context}");
        }
        let after = case["cur"].as_i64().unwrap();
        // Operations that never move a reader (static helpers) report cursor 0.
        if !matches!(case["op"].as_str(), Some("escapeIfRequired" | "d2s" | "f2s")) {
            assert_eq!(reader.cursor() as i64, after, "{context}: cursor after");
        }
    }
    assert!(errors > 5_000, "the fixture must exercise failures: {errors}");
}

#[test]
fn character_predicates_match_every_non_surrogate_code_unit() {
    let document = fixture();
    let predicates = &document["predicates"];
    let positives = |name: &str| -> Vec<u16> {
        predicates[name]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u16)
            .collect()
    };
    let cases: [(&str, fn(u16) -> bool); 4] = [
        ("isWhitespace", java_is_whitespace),
        ("isAllowedNumber", StringReader::is_allowed_number),
        ("isAllowedInUnquotedString", StringReader::is_allowed_in_unquoted_string),
        ("isQuotedStringStart", StringReader::is_quoted_string_start),
    ];
    for (name, predicate) in cases {
        let wanted = positives(name);
        let actual: Vec<u16> = (0..=u16::MAX)
            .filter(|unit| !(0xD800..=0xDFFF).contains(unit) && predicate(*unit))
            .collect();
        assert_eq!(actual, wanted, "{name}");
    }
}
