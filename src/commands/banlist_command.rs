// SPDX-License-Identifier: GPL-2.0-only
//! BanListCommands grammar; the host retains ban ownership and encounter order.
use super::{
    dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher},
    exception::CommandSyntaxException,
    suggestion::{SuggestionError, Suggestions},
};
use crate::admission_commands::{BanListItem, BanListKind, CommandMessage, MessageArg};
use std::{cell::Cell, rc::Rc};
#[derive(Clone)]
struct Source {
    permission: u8,
    kind: Rc<Cell<BanListKind>>,
}
fn source(permission: u8) -> Source {
    Source {
        permission,
        kind: Rc::new(Cell::new(BanListKind::All)),
    }
}
fn dispatcher() -> CommandDispatcher<Source> {
    let mut d = CommandDispatcher::new();
    let mut root = ArgumentBuilder::literal("banlist")
        .requires(Rc::new(|s: &Source| s.permission >= 3))
        .executes(Rc::new(|_: &CommandContext<Source>| Ok(0)));
    for (name, kind) in [("ips", BanListKind::Ips), ("players", BanListKind::Players)] {
        root = root.then(ArgumentBuilder::literal(name).executes(Rc::new(
            move |c: &CommandContext<Source>| {
                c.source().kind.set(kind);
                Ok(0)
            },
        )));
    }
    d.register(root);
    d
}
/// Parse first: failed/unauthorized commands must not query or alter any owner.
pub fn parse(input: &str, permission: u8) -> Result<BanListKind, CommandSyntaxException> {
    let s = source(permission);
    dispatcher().execute_str(input, &s)?;
    Ok(s.kind.get())
}
pub fn messages(entries: Vec<BanListItem>) -> (i32, Vec<CommandMessage>) {
    let count = entries.len() as i32;
    if entries.is_empty() {
        return (0, vec![CommandMessage::plain_key("commands.banlist.none")]);
    }
    let mut messages = vec![CommandMessage::new(
        "commands.banlist.list",
        vec![MessageArg::Integer(count)],
    )];
    messages.extend(entries.into_iter().map(|entry| {
        CommandMessage::new(
            "commands.banlist.entry",
            vec![
                MessageArg::Component(Box::new(CommandMessage::literal(entry.display_name))),
                MessageArg::Text(entry.source),
                MessageArg::Text(entry.reason),
            ],
        )
    }));
    (count, messages)
}
pub fn suggestions(input: &str, permission: u8) -> Result<Suggestions, SuggestionError> {
    if permission < 3 {
        return Ok(Suggestions::empty());
    }
    let d = dispatcher();
    d.completion_suggestions_at_end(&d.parse(input, &source(permission)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(v: &[u8]) -> String {
        v.iter().map(|b| format!("{b:02x}")).collect()
    }
    #[test]
    fn executed_original_grammar_results_and_feedback() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/banlist-command-vectors.json"))
                .unwrap();
        for level in fixture["grammar"].as_array().unwrap() {
            let permission = level["permission"].as_u64().unwrap() as u8;
            let d = dispatcher();
            let s = source(permission);
            let (tree, root) = super::super::tree_packet::usable_tree(d.tree(), d.root(), &s);
            assert_eq!(
                hex(&super::super::tree_packet::encode_commands_packet(&tree, root).unwrap()),
                level["packet_hex"].as_str().unwrap()
            );
            for row in level["cases"].as_array().unwrap() {
                let v = suggestions(row["input"].as_str().unwrap(), permission).unwrap();
                assert_eq!(
                    serde_json::json!({"start":v.range().start(),"end":v.range().end(),"texts":v.list().iter().map(|v|v.text()).collect::<Vec<_>>()}),
                    row["suggestions"],
                    "{row}"
                );
            }
        }
        for row in fixture["execution"].as_array().unwrap() {
            let parsed = parse(
                row["input"].as_str().unwrap(),
                row["permission"].as_u64().unwrap() as u8,
            );
            assert_eq!(parsed.is_ok(), row["ok"].as_bool().unwrap(), "{row}");
            if let Ok(kind) = parsed {
                assert!(matches!(
                    (row["input"].as_str().unwrap(), kind),
                    ("banlist", BanListKind::All)
                        | ("banlist players", BanListKind::Players)
                        | ("banlist ips", BanListKind::Ips)
                ));
                let entries = row["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| BanListItem {
                        display_name: v["display_name"].as_str().unwrap().into(),
                        source: v["source"].as_str().unwrap().into(),
                        reason: v["reason"].as_str().unwrap().into(),
                    })
                    .collect();
                let (result, messages) = messages(entries);
                assert_eq!(result, row["result"].as_i64().unwrap() as i32);
                let encoded = messages
                    .iter()
                    .map(|message| {
                        let tag = crate::nbt::Tag::decode_anonymous(
                            &message.network_component().unwrap(),
                        )
                        .unwrap();
                        hex(&crate::components::text::Component::from_nbt(&tag)
                            .unwrap()
                            .to_network_nbt()
                            .unwrap())
                    })
                    .collect::<Vec<_>>();
                assert_eq!(serde_json::json!(encoded), row["feedback_hex"], "{row}");
            }
        }
    }
}
