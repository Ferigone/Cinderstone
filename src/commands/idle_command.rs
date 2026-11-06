// SPDX-License-Identifier: GPL-2.0-only
//! SetPlayerIdleTimeoutCommand and the listener's separate action-idle deadline.
use super::{arguments::{ArgumentValue, IntegerArgumentType}, dispatcher::{ArgumentBuilder, CommandDispatcher},
    exception::CommandSyntaxException, suggestion::{SuggestionError, Suggestions}};
use std::rc::Rc;

pub fn dispatcher() -> CommandDispatcher<u8> {
    let mut dispatcher = CommandDispatcher::new();
    dispatcher.register(ArgumentBuilder::literal("setidletimeout")
        .requires(Rc::new(|permission: &u8| *permission >= 3))
        .then(ArgumentBuilder::argument("minutes", Rc::new(IntegerArgumentType::new(0, i32::MAX)))
            .executes(Rc::new(|context| match context.argument("minutes") {
                Some(ArgumentValue::Integer(value)) => Ok(*value), _ => unreachable!("registered integer"),
            }))));
    dispatcher
}
pub fn execute(input: &str, permission: u8) -> Result<i32, CommandSyntaxException> {
    dispatcher().execute_str(input, &permission)
}
pub fn suggestions(input: &str, permission: u8) -> Result<Suggestions, SuggestionError> {
    let dispatcher = dispatcher();
    dispatcher.completion_suggestions_at_end(&dispatcher.parse(input, &permission))
}
pub fn expired(now_ms: i64, last_action_ms: i64, minutes: i32) -> bool {
    last_action_ms > 0 && minutes > 0 && now_ms.wrapping_sub(last_action_ms) > i64::from(minutes) * 60_000
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_command_and_listener_deadlines() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/idle-command-vectors.json")).unwrap();
        for row in vectors["trees"].as_array().unwrap() {
            let dispatcher = dispatcher();
            let permission = row["permission"].as_u64().unwrap() as u8;
            let (tree, root) = super::super::tree_packet::usable_tree(dispatcher.tree(), dispatcher.root(), &permission);
            let bytes = super::super::tree_packet::encode_commands_packet(&tree, root).unwrap();
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(hex, row["packet_hex"].as_str().unwrap());
        }
        for row in vectors["execution"].as_array().unwrap() {
            let parsed = execute(row["input"].as_str().unwrap(), row["permission"].as_u64().unwrap() as u8);
            assert_eq!(parsed.is_ok(), row["ok"].as_bool().unwrap(), "{row}");
            assert_eq!(parsed.as_ref().copied().unwrap_or(7), row["minutes"].as_i64().unwrap() as i32);
            if let Ok(minutes) = parsed {
                assert_eq!(minutes,row["result"].as_i64().unwrap() as i32);
                let mut bytes = crate::admission_commands::CommandMessage::new("commands.setidletimeout.success",
                    vec![crate::admission_commands::MessageArg::Integer(minutes)]).network_component().unwrap();
                let tag = crate::nbt::Tag::decode_anonymous(&bytes).unwrap();
                bytes = crate::components::text::Component::from_nbt(&tag).unwrap().to_network_nbt().unwrap();
                bytes.push(0);
                let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                assert_eq!(hex,row["feedback_hex"].as_str().unwrap());
            }
        }
        for row in vectors["deadlines"].as_array().unwrap() {
            assert_eq!(expired(row["now"].as_i64().unwrap(),row["last"].as_i64().unwrap(),row["minutes"].as_i64().unwrap() as i32),row["kicked"].as_bool().unwrap(),"{row}");
        }
    }
}
