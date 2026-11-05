//! Differential check of the dispatcher core against
//! `fixtures/commands-brigadier-dispatch-vectors.json`, produced by EXECUTING the unchanged
//! Brigadier 1.2.9 on seeded random command trees (`tools/brigadier_dispatch_generate.py`).
//! For every input it compares the parse result (cursor, per-node failures, the context
//! chain), the order and content of every executed command/modifier, the consumer
//! notifications and the final result or exception.

use super::arguments::{
    BoolArgumentType, DoubleArgumentType, ErasedArgumentType, FloatArgumentType,
    IntegerArgumentType, LongArgumentType, StringArgumentType,
};
use super::dispatcher::{
    ArgumentBuilder, CommandContext, CommandDispatcher, NodeId, RedirectModifier,
};
use super::exception::{CommandMessage, CommandSyntaxException};
use super::arguments::ArgumentValue;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

pub(super) type Events = Rc<RefCell<Vec<String>>>;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../fixtures/commands-brigadier-dispatch-vectors.json"
    ))
    .unwrap()
}

fn hex32(value: &Value) -> f32 {
    f32::from_bits(u32::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}

fn hex64(value: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}

pub(super) fn argument_type(spec: &Value) -> Rc<dyn ErasedArgumentType<i32>> {
    match spec["k"].as_str().unwrap() {
        "bool" => Rc::new(BoolArgumentType),
        "int" => Rc::new(IntegerArgumentType::new(
            spec["min"].as_i64().unwrap() as i32,
            spec["max"].as_i64().unwrap() as i32,
        )),
        "long" => Rc::new(LongArgumentType::new(
            spec["min"].as_str().unwrap().parse().unwrap(),
            spec["max"].as_str().unwrap().parse().unwrap(),
        )),
        "float" => Rc::new(FloatArgumentType::new(hex32(&spec["min"]), hex32(&spec["max"]))),
        "double" => Rc::new(DoubleArgumentType::new(hex64(&spec["min"]), hex64(&spec["max"]))),
        "word" => Rc::new(StringArgumentType::word()),
        "string" => Rc::new(StringArgumentType::string()),
        "greedy" => Rc::new(StringArgumentType::greedy_string()),
        "blockpos" => Rc::new(super::coordinates::BlockPosArgumentType),
        "columnpos" => Rc::new(super::coordinates::ColumnPosArgumentType),
        "vec3" => Rc::new(super::coordinates::Vec3ArgumentType::new()),
        "vec2" => Rc::new(super::coordinates::Vec2ArgumentType::new()),
        "time" => Rc::new(super::simple_arguments::TimeArgumentType::with_minimum(spec["min"].as_i64().unwrap_or(0) as i32)),
        "uuid" => Rc::new(super::simple_arguments::UuidArgumentType),
        "anchor" => Rc::new(super::simple_arguments::EntityAnchorArgumentType),
        "angle" => Rc::new(super::simple_arguments::AngleArgumentType),
        "swizzle" => Rc::new(super::simple_arguments::SwizzleArgumentType),
        "int_range" => Rc::new(super::ranges::IntRangeArgumentType),
        "float_range" => Rc::new(super::ranges::FloatRangeArgumentType),
        "operation" => Rc::new(super::operation::OperationArgumentType),
        other => panic!("unknown type {other}"),
    }
}

fn value_text(value: &ArgumentValue) -> String {
    match value {
        ArgumentValue::Float(v) => format!("f{:x}", v.to_bits()),
        ArgumentValue::Double(v) => format!("d{:x}", v.to_bits()),
        ArgumentValue::Bool(v) => format!("B{v}"),
        ArgumentValue::Integer(v) => format!("I{v}"),
        ArgumentValue::Long(v) => format!("L{v}"),
        ArgumentValue::String(v) => format!("S{v}"),
        ArgumentValue::Coordinates(v) => format!("C{v:?}"),
        ArgumentValue::Uuid(v) => format!("U{v:?}"),
        ArgumentValue::EntityAnchor(v) => format!("E{v:?}"),
        ArgumentValue::Angle(v) => format!("A{v:?}"),
        ArgumentValue::Swizzle(v) => format!("W{v:?}"),
        ArgumentValue::IntRange(v) => format!("R{v:?}"),
        ArgumentValue::DoubleRange(v) => format!("Q{v:?}"),
        ArgumentValue::Operation(v) => format!("O{v:?}"),
    }
}

/// The same one-line description of a context the Java harness records.
fn digest(context: &CommandContext<i32>, names: &[String]) -> String {
    let mut out = format!(
        "src={};in={};rng={}-{};nodes=",
        context.source(),
        context.input(),
        context.range().start(),
        context.range().end()
    );
    for node in context.nodes() {
        out.push_str(&format!("{}@{}-{},", node.name, node.range.start(), node.range.end()));
    }
    out.push_str(";args=");
    for name in names {
        if let Some(value) = context.argument(name) {
            out.push_str(&format!("{name}={},", value_text(value)));
        }
    }
    out.push_str(&format!(
        ";fork={};mod={};cmd={};child={}",
        context.is_forked(),
        context.redirect_modifier().is_some(),
        context.command().is_some(),
        context.child().is_some()
    ));
    out
}

fn collect_names(spec: &Value, names: &mut Vec<String>) {
    if spec["t"] == "arg" {
        names.push(spec["name"].as_str().unwrap().to_string());
    }
    for child in spec["children"].as_array().into_iter().flatten() {
        collect_names(child, names);
    }
}

fn unknown_command() -> CommandSyntaxException {
    CommandSyntaxException::without_context(CommandMessage::new("command.unknown.command", vec![]))
}

fn unknown_argument() -> CommandSyntaxException {
    CommandSyntaxException::without_context(CommandMessage::new("command.unknown.argument", vec![]))
}

pub(super) fn build(
    spec: &Value,
    dispatcher: &CommandDispatcher<i32>,
    events: &Events,
    names: &Rc<Vec<String>>,
) -> ArgumentBuilder<i32> {
    let name = spec["name"].as_str().unwrap();
    let mut builder = if spec["t"] == "lit" {
        ArgumentBuilder::literal(name)
    } else {
        ArgumentBuilder::argument(name, argument_type(&spec["type"]))
    };
    if let Some(sug) = spec.get("sug") {
        builder = builder.suggests(super::suggestion_tests::suggestion_provider(sug));
    }
    if let Some(level) = spec["req"].as_i64() {
        builder = builder.requires(Rc::new(move |source: &i32| i64::from(*source) >= level));
    }
    if !spec["cmd"].is_null() {
        let throws = spec["cmd"].is_string();
        let base = spec["cmd"].as_i64().unwrap_or(0) as i32;
        let (events, names) = (Rc::clone(events), Rc::clone(names));
        builder = builder.executes(Rc::new(move |context| {
            events.borrow_mut().push(format!("run|{}", digest(context, &names)));
            if throws {
                return Err(unknown_command());
            }
            Ok(base + context.source() % 7)
        }));
    }
    if spec.get("children").is_some() {
        for child in spec["children"].as_array().unwrap() {
            builder = builder.then(build(child, dispatcher, events, names));
        }
    }
    if let Some(redirect) = spec.get("redirect") {
        let to = redirect["to"].as_str().unwrap();
        let target: NodeId = if to == "root" {
            dispatcher.root()
        } else {
            dispatcher.tree().child(dispatcher.root(), to).expect("redirect target registered earlier")
        };
        match redirect["kind"].as_str().unwrap() {
            "plain" => builder = builder.redirect(target),
            "single" => {
                let (events, names) = (Rc::clone(events), Rc::clone(names));
                builder = builder.redirect_single(
                    target,
                    Rc::new(move |context| {
                        events.borrow_mut().push(format!("single|{}", digest(context, &names)));
                        Ok(context.source() + 1)
                    }),
                );
            }
            _ => {
                let kind = redirect["mod"].as_str().unwrap().to_string();
                let (events, names) = (Rc::clone(events), Rc::clone(names));
                let modifier: RedirectModifier<i32> = Rc::new(move |context| {
                    events.borrow_mut().push(format!("mod|{kind}|{}", digest(context, &names)));
                    match kind.as_str() {
                        "id" => Ok(vec![*context.source()]),
                        "dup" => Ok(vec![*context.source(), context.source() + 100]),
                        "none" => Ok(vec![]),
                        _ => Err(unknown_argument()),
                    }
                });
                builder = builder.fork(target, modifier);
            }
        }
    }
    builder
}

fn describe_error(error: &CommandSyntaxException) -> Value {
    let message = error.raw_message();
    serde_json::json!({
        "key": message.key,
        "args": message.args.iter().map(|a| serde_json::json!({"t": a.java_type(), "v": a.java_to_string()})).collect::<Vec<_>>(),
        "english": message.english(),
        "message": error.message(),
        "context": error.context(),
        "errCursor": error.cursor(),
    })
}

fn expected_error(error: &Value) -> Value {
    let mut keep = serde_json::Map::new();
    for key in ["key", "args", "english", "message", "context", "errCursor"] {
        keep.insert(key.to_string(), error[key].clone());
    }
    Value::Object(keep)
}

#[test]
fn dispatcher_parse_and_execute_match_the_executed_original() {
    let document = fixture();
    let tests = document["tests"].as_array().unwrap();
    let (mut inputs, mut failures, mut forks, mut multi, mut usages) = (0, 0, 0, 0, 0);
    for (test_index, test) in tests.iter().enumerate() {
        let mut names = Vec::new();
        for tree in test["tree"].as_array().unwrap() {
            collect_names(tree, &mut names);
        }
        names.sort();
        names.dedup();
        let names = Rc::new(names);
        let events: Events = Rc::new(RefCell::new(Vec::new()));
        let mut dispatcher = CommandDispatcher::<i32>::new();
        {
            let (events, names) = (Rc::clone(&events), Rc::clone(&names));
            dispatcher.set_consumer(Rc::new(move |context, success, result| {
                let _ = &names;
                events.borrow_mut().push(format!("done|src={}|{success}|{result}", context.source()));
            }));
        }
        for tree in test["tree"].as_array().unwrap() {
            let builder = build(tree, &dispatcher, &events, &names);
            dispatcher.register(builder);
        }
        for entry in test["usage"].as_array().unwrap() {
            let mut node = dispatcher.root();
            for name in entry["path"].as_array().unwrap() {
                node = dispatcher.tree().child(node, name.as_str().unwrap()).expect("usage path exists");
            }
            let source = entry["source"].as_i64().unwrap() as i32;
            let context = format!("tree {test_index} usage path {} source {source}", entry["path"]);
            let lines = |value: Vec<String>| Value::Array(value.into_iter().map(Value::from).collect());
            assert_eq!(lines(dispatcher.all_usage(node, &source, true)), entry["allRestricted"], "{context}: restricted");
            assert_eq!(lines(dispatcher.all_usage(node, &source, false)), entry["allUnrestricted"], "{context}: unrestricted");
            let smart: Vec<Value> = dispatcher
                .smart_usage(node, &source)
                .into_iter()
                .map(|(child, usage)| serde_json::json!([dispatcher.tree().name(child), usage]))
                .collect();
            assert_eq!(Value::Array(smart), entry["smart"], "{context}: smart");
            usages += 1;
        }
        for (case_index, case) in test["cases"].as_array().unwrap().iter().enumerate() {
            let input = case["input"].as_str().unwrap();
            let source = case["source"].as_i64().unwrap() as i32;
            let context = format!("tree {test_index} case {case_index} source={source} input={input:?}");
            events.borrow_mut().clear();
            let parse = dispatcher.parse(input, &source);

            let expected_parse = &case["parse"];
            assert_eq!(parse.reader.cursor() as i64, expected_parse["cursor"].as_i64().unwrap(), "{context}: parse cursor");
            let errors: Vec<Value> = parse
                .exceptions
                .iter()
                .map(|(node, error)| {
                    let mut value = describe_error(error);
                    value["node"] = Value::from(dispatcher.tree().name(*node));
                    value
                })
                .collect();
            let wanted_errors: Vec<Value> = expected_parse["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| {
                    let mut value = expected_error(e);
                    value["node"] = e["node"].clone();
                    value
                })
                .collect();
            assert_eq!(errors, wanted_errors, "{context}: per-node failures");
            let mut chain = Vec::new();
            let built = parse.context.build(input);
            let mut link = Some(&built);
            while let Some(current) = link {
                chain.push(Value::from(digest(current, &names)));
                link = current.child();
            }
            assert_eq!(Value::Array(chain), expected_parse["ctx"], "{context}: context chain");

            events.borrow_mut().clear();
            let outcome = dispatcher.execute(&parse);
            inputs += 1;
            match (&outcome, case.get("err")) {
                (Ok(value), None) => assert_eq!(i64::from(*value), case["result"].as_i64().unwrap(), "{context}: result"),
                (Err(error), Some(expected)) => {
                    failures += 1;
                    assert_eq!(describe_error(error), expected_error(expected), "{context}: exception");
                }
                _ => panic!("{context}: outcome {outcome:?} vs fixture {:?}", case.get("err")),
            }
            let log = events.borrow().clone();
            let wanted: Vec<String> = case["events"].as_array().unwrap().iter().map(|e| e.as_str().unwrap().to_string()).collect();
            assert_eq!(log, wanted, "{context}: executed commands and notifications");
            forks += log.iter().filter(|e| e.starts_with("mod|")).count();
            multi += usize::from(log.iter().filter(|e| e.starts_with("run|")).count() > 1);
        }
    }
    assert!(inputs > 3_500, "{inputs}");
    assert!(failures > 1_000 && forks > 50 && multi > 20, "failures {failures}, forks {forks}, multi {multi}");
    assert!(usages > 1_000, "{usages}");
}
