//! Differential check of the command-tree packet against
//! `fixtures/commands-brigadier-tree-packet-vectors.json`: the bytes the dedicated server's own
//! `ClientboundCommandsPacket` writes for seeded random Brigadier trees
//! (`tools/brigadier_tree_packet_generate.py`). Trees contain every Brigadier argument type,
//! redirects, custom suggestion providers (registered and unregistered) and planted copies of
//! subtrees, so nodes that `CommandNode.equals` considers equal must share one index.

use super::arguments::{ArgumentNetworkInfo, ArgumentType};
use super::dispatcher::{ArgumentBuilder, Command, CommandDispatcher, NodeId};
use super::suggestion::SuggestionProvider;
use super::dispatcher_tests::argument_type;
use super::tree_packet::{encode_commands_packet, CommandTreePacketError};
use super::string_reader::StringReader;
use super::exception::CommandSyntaxException;
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../fixtures/commands-brigadier-tree-packet-vectors.json")).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct Builders {
    commands: HashMap<String, Command<i32>>,
}

fn build(spec: &Value, dispatcher: &CommandDispatcher<i32>, builders: &mut Builders) -> ArgumentBuilder<i32> {
    let name = spec["name"].as_str().unwrap();
    let mut builder = if spec["t"] == "lit" {
        ArgumentBuilder::literal(name)
    } else {
        ArgumentBuilder::argument(name, argument_type(&spec["type"]))
    };
    if let Some(sug) = spec.get("sug") {
        let provider: SuggestionProvider<i32> = Rc::new(|_context, builder| Ok(builder.build()));
        builder = match sug.get("name") {
            Some(registered) => builder.suggests_named(registered.as_str().unwrap(), provider),
            None => builder.suggests(provider),
        };
    }
    if let Some(cmd) = spec.get("cmd").filter(|c| !c.is_null()) {
        // The same JSON value is the same Command instance, as in the Java harness.
        let key = cmd.as_str().map_or_else(|| cmd.as_i64().unwrap().to_string(), str::to_string);
        let command = builders
            .commands
            .entry(key.clone())
            .or_insert_with(|| Rc::new(move |_context| Ok(key.len() as i32)))
            .clone();
        builder = builder.executes(command);
    }
    for child in spec["children"].as_array().into_iter().flatten() {
        builder = builder.then(build(child, dispatcher, builders));
    }
    if let Some(redirect) = spec.get("redirect") {
        let to = redirect["to"].as_str().unwrap();
        let target: NodeId = if to == "root" {
            dispatcher.root()
        } else {
            dispatcher.tree().child(dispatcher.root(), to).expect("redirect target registered earlier")
        };
        builder = builder.redirect(target);
    }
    builder
}

fn reachable(dispatcher: &CommandDispatcher<i32>) -> usize {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([dispatcher.root()]);
    while let Some(node) = queue.pop_front() {
        if seen.insert(node) {
            queue.extend(dispatcher.tree().children(node));
            queue.extend(dispatcher.tree().redirect(node));
        }
    }
    seen.len()
}

fn read_varint(bytes: &[u8]) -> usize {
    let (mut value, mut shift) = (0usize, 0);
    for byte in bytes {
        value |= usize::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    value
}

#[test]
fn command_tree_packets_match_the_executed_original() {
    let document = fixture();
    let (mut merged, mut redirects, mut custom, mut registered, mut total) = (0, 0, 0, 0, 0);
    for (index, case) in document["tests"].as_array().unwrap().iter().enumerate() {
        let mut dispatcher = CommandDispatcher::<i32>::new();
        let mut builders = Builders { commands: HashMap::new() };
        for tree in case["tree"].as_array().unwrap() {
            let builder = build(tree, &dispatcher, &mut builders);
            dispatcher.register(builder);
        }
        let packet = encode_commands_packet(dispatcher.tree(), dispatcher.root()).unwrap();
        assert_eq!(hex(&packet), case["packet"].as_str().unwrap(), "tree {index}: {}", case["tree"]);
        total += 1;
        merged += usize::from(read_varint(&packet) < reachable(&dispatcher));
        let text = case["tree"].to_string();
        redirects += usize::from(text.contains("\"redirect\""));
        custom += usize::from(text.contains("\"sug\""));
        registered += usize::from(text.contains("minecraft:"));
    }
    assert!(total >= 160 && merged > 20 && redirects > 40 && custom > 40 && registered > 10, "{total} {merged} {redirects} {custom} {registered}");
}

#[test]
fn a_type_without_a_network_form_is_rejected() {
    struct Opaque;
    impl ArgumentType for Opaque {
        type Value = String;
        fn parse(&self, reader: &mut StringReader) -> Result<String, CommandSyntaxException> {
            Ok(reader.read_unquoted_string())
        }
    }
    assert_eq!(ArgumentType::network_info(&Opaque), None::<ArgumentNetworkInfo>);
    let mut dispatcher = CommandDispatcher::<i32>::new();
    dispatcher.register(ArgumentBuilder::literal("x").then(ArgumentBuilder::argument("v", Rc::new(Opaque))));
    assert_eq!(
        encode_commands_packet(dispatcher.tree(), dispatcher.root()),
        Err(CommandTreePacketError::UnsupportedArgumentType { node: "v".to_string() })
    );
}
