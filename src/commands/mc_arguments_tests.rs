//! Differential check of the Minecraft argument types against
//! `fixtures/commands-mc-arguments-vectors.json`, produced by EXECUTING the unchanged argument
//! classes of the pinned 1.21 server (`tools/mc_arguments_generate.py`). Stage 6: the position
//! arguments. For every input the test compares the cursor afterwards and either the exception (key,
//! typed arguments, `getMessage()`, context, cursor) or the relative flags and, for two command
//! sources each, the position, rotation and block position the coordinates resolve to.

use super::arguments::ArgumentType;
use super::coordinates::{
    BlockPosArgumentType, ColumnPosArgumentType, CoordinateSource, Coordinates, Vec2ArgumentType, Vec3ArgumentType,
};
use super::string_reader::StringReader;
use crate::geometry::Vec3;
use serde_json::{json, Value};

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

#[test]
fn position_arguments_match_the_executed_original() {
    let document = fixture();
    let (mut parsed, mut failed, mut local, mut relative, mut centred) = (0, 0, 0, 0, 0);
    for (index, case) in document["cases"].as_array().unwrap().iter().enumerate() {
        let kind = case["type"].as_str().unwrap();
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
            (result, ok, err) => panic!("{context}: {result:?} vs fixture ok={ok:?} err={err:?}"),
        }
    }
    assert!(parsed > 2_000 && failed > 1_000 && local > 150 && relative > 300 && centred > 100, "{parsed} {failed} {local} {relative} {centred}");
}
