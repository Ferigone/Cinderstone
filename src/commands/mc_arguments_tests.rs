//! Differential check of the Minecraft argument types against
//! `fixtures/commands-mc-arguments-vectors.json`, produced by EXECUTING the unchanged argument
//! classes of the pinned 1.21 server (`tools/mc_arguments_generate.py`). Stage 6: the position
//! arguments. For every input the test compares the cursor afterwards and either the exception (key,
//! typed arguments, `getMessage()`, context, cursor) or the relative flags and, for two command
//! sources each, the position, rotation and block position the coordinates resolve to. Stage 7: time,
//! uuid, entity anchor, angle and swizzle, compared the same way on their parsed values. Stage 9: the
//! int and float range arguments, compared on their bounds and squared bounds. Stage 10: the scoreboard
//! operation argument, compared on its parse, on the score pairs the parsed operator produces and on its
//! suggestions.

use super::arguments::ArgumentType;
use super::simple_arguments::{
    AngleArgumentType, EntityAnchor, EntityAnchorArgumentType, SwizzleArgumentType, TimeArgumentType, UuidArgumentType,
};
use super::coordinates::{
    BlockPosArgumentType, ColumnPosArgumentType, CoordinateSource, Coordinates, Vec2ArgumentType, Vec3ArgumentType,
};
use super::dispatcher::{CommandContextBuilder, CommandTree};
use super::operation::OperationArgumentType;
use super::ranges::{FloatRangeArgumentType, IntRangeArgumentType};
use super::suggestion::SuggestionsBuilder;
use super::string_reader::StringReader;
use crate::geometry::Vec3;
use serde_json::{json, Value};

const POSITION_TYPES: [&str; 6] = ["vec3", "vec3_raw", "blockpos", "vec2", "vec2_raw", "columnpos"];

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../fixtures/commands-mc-arguments-vectors.json")).unwrap()
}

fn hex64(value: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}

fn hex32(value: &Value) -> f32 {
    f32::from_bits(u32::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}

fn parse(kind: &str, reader: &mut StringReader) -> Result<Coordinates, super::exception::CommandSyntaxException> {
    match kind {
        "vec3" => Vec3ArgumentType::new().parse(reader),
        "vec3_raw" => Vec3ArgumentType::with_center_correct(false).parse(reader),
        "blockpos" => BlockPosArgumentType.parse(reader),
        "vec2" => Vec2ArgumentType::new().parse(reader),
        "vec2_raw" => Vec2ArgumentType::with_center_correct(false).parse(reader),
        "columnpos" => ColumnPosArgumentType.parse(reader),
        other => panic!("unknown type {other}"),
    }
}

fn source(spec: &Value) -> CoordinateSource {
    let position = Vec3::new(hex64(&spec["pos"][0]), hex64(&spec["pos"][1]), hex64(&spec["pos"][2]));
    CoordinateSource {
        position,
        // No entity in the oracle's sources, so the anchor leaves the position unchanged.
        anchored_position: position,
        rotation: (hex32(&spec["rot"][0]), hex32(&spec["rot"][1])),
    }
}

fn bits64(value: f64) -> String {
    format!("{:x}", value.to_bits())
}

fn bits32(value: f32) -> String {
    format!("{:x}", value.to_bits())
}

fn check_error(error: &super::exception::CommandSyntaxException, expected: &Value, context: &str) {
    let args: Vec<Value> = error
        .raw_message()
        .args
        .iter()
        .map(|a| json!({"t": a.java_type(), "v": a.java_to_string()}))
        .collect();
    assert_eq!(error.raw_message().key, expected["key"].as_str().unwrap(), "{context}: key");
    assert_eq!(Value::Array(args), expected["args"], "{context}: arguments");
    assert_eq!(error.message(), expected["message"].as_str().unwrap(), "{context}: message");
    assert_eq!(error.context().as_deref(), expected["context"].as_str(), "{context}: context");
    assert_eq!(i64::from(error.cursor()), expected["errCursor"].as_i64().unwrap(), "{context}: exception cursor");
}

#[test]
fn position_arguments_match_the_executed_original() {
    let document = fixture();
    let (mut parsed, mut failed, mut local, mut relative, mut centred) = (0, 0, 0, 0, 0);
    for (index, case) in document["cases"].as_array().unwrap().iter().enumerate() {
        let kind = case["type"].as_str().unwrap();
        if !POSITION_TYPES.contains(&kind) {
            continue;
        }
        let input = case["input"].as_str().unwrap();
        let context = format!("case {index} {kind} input={input:?} start={:?}", case.get("start"));
        let mut reader = StringReader::new(input);
        if let Some(start) = case["start"].as_u64() {
            reader.set_cursor(start as usize);
        }
        let result = parse(kind, &mut reader);
        assert_eq!(reader.cursor() as i64, case["cursor"].as_i64().unwrap(), "{context}: cursor afterwards");
        match (result, case.get("ok"), case.get("err")) {
            (Ok(coordinates), Some(expected), None) => {
                parsed += 1;
                local += usize::from(matches!(coordinates, Coordinates::Local(_)));
                relative += usize::from(coordinates.is_x_relative() && matches!(coordinates, Coordinates::World(_)));
                centred += usize::from(input.contains('.') && !input.contains('~'));
                let flags = json!([coordinates.is_x_relative(), coordinates.is_y_relative(), coordinates.is_z_relative()]);
                assert_eq!(flags, expected["rel"], "{context}: relative flags");
                let resolved: Vec<Value> = case["sources"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|spec| {
                        let source = source(spec);
                        let position = coordinates.position(&source);
                        let rotation = coordinates.rotation(&source);
                        let block = coordinates.block_pos(&source);
                        json!({
                            "pos": [bits64(position.x), bits64(position.y), bits64(position.z)],
                            "rot": [bits32(rotation.0), bits32(rotation.1)],
                            "block": [block.0, block.1, block.2],
                        })
                    })
                    .collect();
                assert_eq!(Value::Array(resolved), expected["at"], "{context}: resolved coordinates");
            }
            (Err(error), None, Some(expected)) => {
                failed += 1;
                check_error(&error, expected, &context);
            }
            (result, ok, err) => panic!("{context}: {result:?} vs fixture ok={ok:?} err={err:?}"),
        }
    }
    assert!(parsed > 2_000 && failed > 1_000 && local > 150 && relative > 300 && centred > 100, "{parsed} {failed} {local} {relative} {centred}");
}

const VALUE_TYPES: [&str; 5] = ["time", "uuid", "anchor", "angle", "swizzle"];

/// Parse with a stage 7 type and render the value the way the Java harness does.
fn parse_value(case: &Value, reader: &mut StringReader) -> Result<Value, super::exception::CommandSyntaxException> {
    Ok(match case["type"].as_str().unwrap() {
        "time" => {
            let argument = match case["min"].as_i64() {
                Some(minimum) => TimeArgumentType::with_minimum(minimum as i32),
                None => TimeArgumentType::new(),
            };
            json!({ "v": argument.parse(reader)? })
        }
        "uuid" => {
            let uuid = UuidArgumentType.parse(reader)?;
            json!({ "msb": uuid.most_significant_bits.to_string(), "lsb": uuid.least_significant_bits.to_string() })
        }
        "anchor" => json!({ "v": match EntityAnchorArgumentType.parse(reader)? {
            EntityAnchor::Feet => "FEET",
            EntityAnchor::Eyes => "EYES",
        } }),
        "angle" => {
            let angle = AngleArgumentType.parse(reader)?;
            let at: Vec<Value> = case["sources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|spec| Value::from(bits32(angle.angle(&source(spec)))))
                .collect();
            json!({ "at": at })
        }
        "swizzle" => json!({ "v": SwizzleArgumentType.parse(reader)?.letters() }),
        other => panic!("unknown type {other}"),
    })
}

#[test]
fn simple_value_arguments_match_the_executed_original() {
    let document = fixture();
    let (mut parsed, mut failed, mut scaled, mut relative_angles, mut lenient_uuids) = (0, 0, 0, 0, 0);
    for (index, case) in document["cases"].as_array().unwrap().iter().enumerate() {
        let kind = case["type"].as_str().unwrap();
        if !VALUE_TYPES.contains(&kind) {
            continue;
        }
        let input = case["input"].as_str().unwrap();
        let context = format!("case {index} {kind} input={input:?} min={:?}", case.get("min"));
        let mut reader = StringReader::new(input);
        let result = parse_value(case, &mut reader);
        assert_eq!(reader.cursor() as i64, case["cursor"].as_i64().unwrap(), "{context}: cursor afterwards");
        match (result, case.get("ok"), case.get("err")) {
            (Ok(value), Some(expected), None) => {
                parsed += 1;
                assert_eq!(&value, expected, "{context}: value");
                scaled += usize::from(kind == "time" && input.ends_with(['d', 's']));
                relative_angles += usize::from(kind == "angle" && input.starts_with('~'));
                lenient_uuids += usize::from(kind == "uuid" && reader.cursor() != 36);
            }
            (Err(error), None, Some(expected)) => {
                failed += 1;
                check_error(&error, expected, &context);
            }
            (result, ok, err) => panic!("{context}: {result:?} vs fixture ok={ok:?} err={err:?}"),
        }
    }
    assert!(
        parsed > 600 && failed > 400 && scaled > 20 && relative_angles > 30 && lenient_uuids > 20,
        "{parsed} {failed} {scaled} {relative_angles} {lenient_uuids}"
    );
}

/// The bounds the Java harness records: integers as decimal text, doubles as hex bits; an absent
/// bound is an absent key (Gson drops the nulls).
fn range_value(case: &Value, reader: &mut StringReader) -> Result<Value, super::exception::CommandSyntaxException> {
    let mut out = serde_json::Map::new();
    match case["type"].as_str().unwrap() {
        "int_range" => {
            let range = IntRangeArgumentType.parse(reader)?;
            for (key, value) in [("min", range.min.map(i64::from)), ("max", range.max.map(i64::from)), ("minSq", range.min_sq()), ("maxSq", range.max_sq())] {
                if let Some(value) = value {
                    out.insert(key.to_string(), Value::from(value.to_string()));
                }
            }
        }
        "float_range" => {
            let range = FloatRangeArgumentType.parse(reader)?;
            for (key, value) in [("min", range.min), ("max", range.max), ("minSq", range.min_sq()), ("maxSq", range.max_sq())] {
                if let Some(value) = value {
                    out.insert(key.to_string(), Value::from(bits64(value)));
                }
            }
        }
        other => panic!("unknown type {other}"),
    }
    Ok(Value::Object(out))
}

#[test]
fn range_arguments_match_the_executed_original() {
    let document = fixture();
    let (mut parsed, mut failed) = (0, 0);
    let (mut single, mut open_min, mut open_max, mut both, mut empty, mut swapped, mut invalid) = (0, 0, 0, 0, 0, 0, 0);
    for (index, case) in document["cases"].as_array().unwrap().iter().enumerate() {
        let kind = case["type"].as_str().unwrap();
        if kind != "int_range" && kind != "float_range" {
            continue;
        }
        let input = case["input"].as_str().unwrap();
        let context = format!("case {index} {kind} input={input:?}");
        let mut reader = StringReader::new(input);
        let result = range_value(case, &mut reader);
        assert_eq!(reader.cursor() as i64, case["cursor"].as_i64().unwrap(), "{context}: cursor afterwards");
        match (result, case.get("ok"), case.get("err")) {
            (Ok(value), Some(expected), None) => {
                parsed += 1;
                assert_eq!(&value, expected, "{context}: bounds");
                let (has_min, has_max) = (expected.get("min").is_some(), expected.get("max").is_some());
                single += usize::from(!input.contains(".."));
                open_min += usize::from(has_min && !has_max);
                open_max += usize::from(!has_min && has_max);
                both += usize::from(has_min && has_max && input.contains(".."));
            }
            (Err(error), None, Some(expected)) => {
                failed += 1;
                check_error(&error, expected, &context);
                match expected["key"].as_str().unwrap() {
                    "argument.range.empty" => empty += 1,
                    "argument.range.swapped" => swapped += 1,
                    _ => invalid += 1,
                }
            }
            (result, ok, err) => panic!("{context}: {result:?} vs fixture ok={ok:?} err={err:?}"),
        }
    }
    assert!(
        parsed > 600 && failed > 900 && single > 100 && open_min > 50 && open_max > 50 && both > 150 && empty > 100 && swapped > 80 && invalid > 500,
        "{parsed} {failed} {single} {open_min} {open_max} {both} {empty} {swapped} {invalid}"
    );
}

/// The pairs the parsed operator produces for the case's `pairs`: the new `(target, source)` as decimal
/// text, or the exception, as the Java harness records them.
fn operation_value(case: &Value, reader: &mut StringReader) -> Result<Value, super::exception::CommandSyntaxException> {
    let operation = OperationArgumentType.parse(reader)?;
    let pairs = case.get("pairs").and_then(Value::as_array).cloned().unwrap_or_default();
    let applied: Vec<Value> = pairs
        .iter()
        .map(|pair| {
            let target: i32 = pair[0].as_str().unwrap().parse().unwrap();
            let source: i32 = pair[1].as_str().unwrap().parse().unwrap();
            match operation.apply(target, source) {
                Ok((target, source)) => json!({ "now": [target.to_string(), source.to_string()] }),
                Err(error) => {
                    let args: Vec<Value> = error.raw_message().args.iter().map(|a| json!({"t": a.java_type(), "v": a.java_to_string()})).collect();
                    let mut entry = json!({
                        "message": error.message(),
                        "errCursor": error.cursor(),
                        "key": error.raw_message().key,
                        "args": args,
                    });
                    if let Some(context) = error.context() {
                        entry["context"] = Value::from(context);
                    }
                    json!({ "err": entry })
                }
            }
        })
        .collect();
    Ok(json!({ "applied": applied }))
}

/// The suggestions of the operation argument for the case's input and builder start.
fn operation_suggestions(case: &Value) -> Value {
    let tree: CommandTree<()> = CommandTree::default();
    let mut tree = tree;
    let root = tree.new_root();
    let context = CommandContextBuilder::new((), root, 0).build(case["input"].as_str().unwrap());
    let mut builder = SuggestionsBuilder::new(case["input"].as_str().unwrap(), case["suggestAt"].as_u64().unwrap() as usize).unwrap();
    let suggestions = OperationArgumentType.list_suggestions(&context, &mut builder).unwrap();
    let list: Vec<Value> = suggestions
        .list()
        .iter()
        .map(|one| json!({ "start": one.range().start(), "end": one.range().end(), "text": one.text() }))
        .collect();
    json!({ "start": suggestions.range().start(), "end": suggestions.range().end(), "list": list })
}

#[test]
fn operation_argument_matches_the_executed_original() {
    let document = fixture();
    let (mut parsed, mut failed, mut applied, mut divisions_by_zero, mut with_context, mut without_context) = (0, 0, 0, 0, 0, 0);
    let (mut suggested, mut non_empty_suggestions, mut inside_text) = (0, 0, 0);
    for (index, case) in document["cases"].as_array().unwrap().iter().enumerate() {
        if case["type"] != "operation" {
            continue;
        }
        let input = case["input"].as_str().unwrap();
        let context = format!("case {index} operation input={input:?} start={:?} suggestAt={:?}", case.get("start"), case.get("suggestAt"));
        let mut reader = StringReader::new(input);
        if let Some(start) = case["start"].as_u64() {
            reader.set_cursor(start as usize);
        }
        let result = operation_value(case, &mut reader);
        assert_eq!(reader.cursor() as i64, case["cursor"].as_i64().unwrap(), "{context}: cursor afterwards");
        match (result, case.get("ok"), case.get("err")) {
            (Ok(value), Some(expected), None) => {
                parsed += 1;
                assert_eq!(&value, expected, "{context}: applied pairs");
                for entry in value["applied"].as_array().unwrap() {
                    applied += 1;
                    divisions_by_zero += usize::from(entry["err"]["key"] == "arguments.operation.div0");
                }
            }
            (Err(error), None, Some(expected)) => {
                failed += 1;
                check_error(&error, expected, &context);
                with_context += usize::from(error.cursor() >= 0);
                without_context += usize::from(error.cursor() < 0);
            }
            (result, ok, err) => panic!("{context}: {result:?} vs fixture ok={ok:?} err={err:?}"),
        }
        if case.get("suggestAt").is_some() {
            suggested += 1;
            let expected = &case["suggestions"];
            assert_eq!(&operation_suggestions(case), expected, "{context}: suggestions");
            non_empty_suggestions += usize::from(!expected["list"].as_array().unwrap().is_empty());
            inside_text += usize::from(case["suggestAt"].as_u64().unwrap() > 0 && case["suggestAt"].as_u64().unwrap() < input.encode_utf16().count() as u64);
        }
    }
    assert!(
        parsed > 250 && failed > 600 && applied > 3_000 && divisions_by_zero > 40 && with_context > 0 && without_context > 600 && suggested > 400 && non_empty_suggestions > 100 && inside_text > 50,
        "{parsed} {failed} {applied} {divisions_by_zero} {with_context} {without_context} {suggested} {non_empty_suggestions} {inside_text}"
    );
}
