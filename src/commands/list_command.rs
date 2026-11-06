// SPDX-License-Identifier: GPL-2.0-only
//! ListPlayersCommand over a caller's ordered roster and current display components.
use super::{dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher}, exception::CommandSyntaxException,
    suggestion::{SuggestionError, Suggestions}};
use crate::components::text::{Arg, Component, Contents, Primitive};
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
struct Source { uuids: Rc<Cell<bool>> }
fn source() -> Source { Source { uuids: Rc::new(Cell::new(false)) } }
fn dispatcher() -> CommandDispatcher<Source> {
    let mut dispatcher = CommandDispatcher::new();
    dispatcher.register(ArgumentBuilder::literal("list")
        .executes(Rc::new(|_| Ok(0)))
        .then(ArgumentBuilder::literal("uuids").executes(Rc::new(|context: &CommandContext<Source>| {
            context.source().uuids.set(true); Ok(0)
        }))));
    dispatcher
}

pub struct ListedPlayer { pub name: String, pub uuid: String, pub display: Component }
pub struct ListResponse { pub result: i32, pub component: Component }

pub fn execute(input: &str, maximum: i32, players: &[ListedPlayer]) -> Result<ListResponse, CommandSyntaxException> {
    let source = source();
    dispatcher().execute_str(input, &source)?;
    let mut names = players.iter().map(|player| if source.uuids.get() {
        serde_json::json!({"translate":"commands.list.nameAndId", "with":[{"text":player.name},{"text":player.uuid}]})
    } else { player.display.to_json() }).collect::<Vec<_>>();
    let names = if names.len() == 1 { names.pop().unwrap() } else {
        let mut extra = Vec::new();
        for name in names {
            if !extra.is_empty() { extra.push(serde_json::json!({"text":", ","color":"gray"})); }
            extra.push(name);
        }
        if extra.is_empty() { serde_json::json!({"text":""}) }
        else { serde_json::json!({"text":"","extra":extra}) }
    };
    let result = players.len() as i32;
    let mut component = Component::literal("");
    component.contents = Contents::Translatable {
        key: "commands.list.players".into(), fallback: None,
        // Java passes boxed Integer values, not JSON-number values narrowed by JsonOps.
        args: vec![Arg::Primitive(Primitive::Int(result)), Arg::Primitive(Primitive::Int(maximum)),
            Arg::Component(Component::from_json(&names).expect("canonical display components"))],
    };
    Ok(ListResponse { result, component })
}

pub fn suggestions(input: &str) -> Result<Suggestions, SuggestionError> {
    let dispatcher = dispatcher();
    dispatcher.completion_suggestions_at_end(&dispatcher.parse(input, &source()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_list_tree_results_and_current_display_components() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/list-command-vectors.json")).unwrap();
        let dispatcher = dispatcher();
        let (tree, root) = super::super::tree_packet::usable_tree(dispatcher.tree(), dispatcher.root(), &source());
        let hex: String = super::super::tree_packet::encode_commands_packet(&tree, root).unwrap().iter().map(|b| format!("{b:02x}")).collect();
        for row in vectors["trees"].as_array().unwrap() { assert_eq!(hex, row["packet_hex"].as_str().unwrap()); }
        for row in vectors["execution"].as_array().unwrap() {
            let mut board = crate::scoreboard::Scoreboard::new_server();
            if row["team"] == true {
                crate::scoreboard::commands::execute(&mut board, "team add test", None, 4, &[]).unwrap();
                crate::scoreboard::commands::execute(&mut board, "team modify test color red", None, 4, &[]).unwrap();
                crate::scoreboard::commands::execute(&mut board, "team modify test prefix {\"text\":\"[T] \"}", None, 4, &[]).unwrap();
                crate::scoreboard::commands::execute(&mut board, "team modify test suffix {\"text\":\"!\"}", None, 4, &[]).unwrap();
                crate::scoreboard::commands::execute(&mut board, "team join test Zulu", None, 4, &["Zulu".into()]).unwrap();
            }
            let players = (0..row["count"].as_u64().unwrap()).map(|i| {
                let name = if i==0 {"Zulu"} else {"Alpha"};
                let mut uuid = [0;16]; uuid[15] = (i+1) as u8;
                ListedPlayer { name: name.into(), uuid: crate::player_data::uuid_string(uuid),
                    display: crate::scoreboard::names::player_holder(&board, name, uuid).display_name.unwrap() }
            }).collect::<Vec<_>>();
            let response = execute(row["input"].as_str().unwrap(), 37, &players);
            assert_eq!(response.is_ok(), row["ok"].as_bool().unwrap(), "{row}");
            if let Ok(response) = response {
                assert_eq!(response.result, row["result"].as_i64().unwrap() as i32);
                let mut bytes = response.component.to_network_nbt().unwrap(); bytes.push(0);
                let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                assert_eq!(hex, row["feedback_hex"].as_str().unwrap(), "{row}");
            }
        }
    }
}
