// SPDX-License-Identifier: GPL-2.0-only
//! StopCommand grammar. The host sends feedback before requesting halt(false).
use super::{
    dispatcher::{ArgumentBuilder, CommandDispatcher},
    exception::CommandSyntaxException,
    suggestion::{SuggestionError, Suggestions},
};
use std::rc::Rc;
fn dispatcher() -> CommandDispatcher<u8> {
    let mut d = CommandDispatcher::new();
    d.register(
        ArgumentBuilder::literal("stop")
            .requires(Rc::new(|p: &u8| *p >= 4))
            .executes(Rc::new(|_| Ok(1))),
    );
    d
}
pub fn parse(input: &str, permission: u8) -> Result<i32, CommandSyntaxException> {
    dispatcher().execute_str(input, &permission)
}
pub fn suggestions(input: &str, permission: u8) -> Result<Suggestions, SuggestionError> {
    if permission < 4 {
        return Ok(Suggestions::empty());
    }
    let d = dispatcher();
    d.completion_suggestions_at_end(&d.parse(input, &permission))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(v: &[u8]) -> String {
        v.iter().map(|b| format!("{b:02x}")).collect()
    }
    #[test]
    fn executed_original_stop_registration_feedback_and_halt() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/stop-command-vectors.json")).unwrap();
        for level in fixture["grammar"].as_array().unwrap() {
            let permission = level["permission"].as_u64().unwrap() as u8;
            let d = dispatcher();
            let (tree, root) =
                super::super::tree_packet::usable_tree(d.tree(), d.root(), &permission);
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
            assert_eq!(parsed.is_err(), row["running"].as_bool().unwrap(), "{row}");
            if let Ok(result) = parsed {
                assert_eq!(result, row["result"].as_i64().unwrap() as i32);
                let bytes = crate::components::text::Component::from_json(
                    &serde_json::json!({"translate":"commands.stop.stopping"}),
                )
                .unwrap()
                .to_network_nbt()
                .unwrap();
                assert_eq!(
                    serde_json::json!([hex(&bytes)]),
                    row["feedback_hex"],
                    "{row}"
                );
            } else {
                assert!(row["feedback_hex"].as_array().unwrap().is_empty());
            }
        }
    }
}
