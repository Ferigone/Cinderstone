// SPDX-License-Identifier: GPL-2.0-only
//! SaveAllCommand grammar; the caller owns the real asynchronous save transaction.
use super::{dispatcher::{ArgumentBuilder, CommandDispatcher}, exception::CommandSyntaxException,
    suggestion::{SuggestionError, Suggestions}};
use std::rc::Rc;
pub fn dispatcher() -> CommandDispatcher<u8> {
    let mut dispatcher = CommandDispatcher::new();
    dispatcher.register(ArgumentBuilder::literal("save-all")
        .requires(Rc::new(|permission: &u8| *permission >= 4))
        .executes(Rc::new(|_| Ok(0)))
        .then(ArgumentBuilder::literal("flush").executes(Rc::new(|_| Ok(1)))));
    dispatcher
}
/// Parse/authorize before saving. This internal result chooses the flush plan;
/// the completed original command's public result is always1.
pub fn parse(input: &str, permission: u8) -> Result<bool, CommandSyntaxException> {
    dispatcher().execute_str(input, &permission).map(|value| value != 0)
}
pub fn suggestions(input: &str, permission: u8) -> Result<Suggestions, SuggestionError> {
    let dispatcher = dispatcher();
    dispatcher.completion_suggestions_at_end(&dispatcher.parse(input, &permission))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_save_plan_tree_and_ordered_messages() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/save-all-command-vectors.json")).unwrap();
        for row in vectors["trees"].as_array().unwrap() {
            let dispatcher=dispatcher();let permission=row["permission"].as_u64().unwrap() as u8;
            let (tree,root)=super::super::tree_packet::usable_tree(dispatcher.tree(),dispatcher.root(),&permission);
            let hex:String=super::super::tree_packet::encode_commands_packet(&tree,root).unwrap().iter().map(|b|format!("{b:02x}")).collect();
            assert_eq!(hex,row["packet_hex"].as_str().unwrap());
        }
        for row in vectors["execution"].as_array().unwrap() {
            let plan=parse(row["input"].as_str().unwrap(),row["permission"].as_u64().unwrap() as u8);
            assert_eq!(plan.is_ok(),row["called"].as_bool().unwrap(),"{row}");
            if let Ok(flush)=plan {
                assert_eq!(flush,row["flush"].as_bool().unwrap());assert_eq!(row["quiet"],true);assert_eq!(row["force"],true);
                if row["save_success"]==true {assert_eq!(row["result"],1);}else{assert_eq!(row["error_key"],"commands.save.failed");}
            }
            for (index,message) in row["feedback"].as_array().unwrap().iter().enumerate() {
                let key=if index==0 {"commands.save.saving"}else{"commands.save.success"};
                assert_eq!(message["key"],key);assert_eq!(message["broadcast"],index==1);
                let mut bytes=crate::components::text::Component::from_json(&serde_json::json!({"translate":key})).unwrap().to_network_nbt().unwrap();bytes.push(0);
                let hex:String=bytes.iter().map(|b|format!("{b:02x}")).collect();assert_eq!(hex,message["hex"].as_str().unwrap());
            }
        }
    }
}
