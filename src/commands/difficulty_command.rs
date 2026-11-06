//! Java 1.21 DifficultyCommand over the caller's authoritative difficulty.
use super::{
    dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher},
    exception::{CommandArg, CommandMessage, CommandSyntaxException},
    suggestion::{SuggestionError, Suggestions},
};
use std::{cell::RefCell, rc::Rc};

pub const NAMES: [&str; 4] = ["peaceful", "easy", "normal", "hard"];
#[derive(Clone)]
struct Source { permission: u8, requested: Rc<RefCell<Option<i32>>> }
fn source(permission: u8) -> Source { Source { permission, requested: Rc::new(RefCell::new(None)) } }
fn dispatcher() -> CommandDispatcher<Source> {
    let mut dispatcher = CommandDispatcher::new();
    let mut root = ArgumentBuilder::literal("difficulty")
        .requires(Rc::new(|source: &Source| source.permission >= 2))
        .executes(Rc::new(|_: &CommandContext<Source>| Ok(0)));
    for (id, name) in NAMES.iter().enumerate() {
        root = root.then(ArgumentBuilder::literal(*name).executes(Rc::new(move |context: &CommandContext<Source>| {
            *context.source().requested.borrow_mut() = Some(id as i32);
            Ok(0)
        })));
    }
    dispatcher.register(root);
    dispatcher
}
#[derive(Debug, PartialEq, Eq)]
pub struct DifficultyResponse {
    pub result: i32,
    pub key: &'static str,
    /// Requested difficulty for feedback (even if hardcore forces effective Hard).
    pub display_id: i32,
    pub effective: i32,
    pub changed: bool,
}
pub fn execute(input: &str, permission: u8, current: i32, hardcore: bool)
    -> Result<DifficultyResponse, CommandSyntaxException> {
    let source = source(permission);
    dispatcher().execute_str(input, &source)?;
    let requested = *source.requested.borrow();
    if let Some(id) = requested {
        if id == current {
            return Err(CommandSyntaxException::without_context(CommandMessage::new(
                "commands.difficulty.failure", vec![CommandArg::String(NAMES[id as usize].into())])));
        }
        Ok(DifficultyResponse { result: 0, key: "commands.difficulty.success", display_id: id,
            effective: if hardcore { 3 } else { id }, changed: true })
    } else {
        Ok(DifficultyResponse { result: current, key: "commands.difficulty.query", display_id: current,
            effective: current, changed: false })
    }
}
pub fn suggestions(input: &str, permission: u8) -> Result<Suggestions, SuggestionError> {
    if permission < 2 { return Ok(Suggestions::empty()); }
    let dispatcher = dispatcher();
    dispatcher.completion_suggestions_at_end(&dispatcher.parse(input, &source(permission)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executed_original_vectors() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/difficulty-command-vectors.json")).unwrap();
        for level in fixture["grammar"].as_array().unwrap() {
            let permission = level["permission"].as_u64().unwrap() as u8;
            let d = dispatcher(); let source = source(permission);
            let (tree, root) = super::super::tree_packet::usable_tree(d.tree(),d.root(),&source);
            let bytes = super::super::tree_packet::encode_commands_packet(&tree,root).unwrap();
            assert_eq!(bytes.iter().map(|b|format!("{b:02x}")).collect::<String>(),level["packet_hex"].as_str().unwrap());
            for row in level["cases"].as_array().unwrap() {
                let input = row["input"].as_str().unwrap();
                let completion = suggestions(input,permission).unwrap();
                assert_eq!(serde_json::json!({"start":completion.range().start(),"end":completion.range().end(),
                    "texts":completion.list().iter().map(|v|v.text()).collect::<Vec<_>>()}),row["suggestions"],"{row}");
            }
        }
        for row in fixture["execution"].as_array().unwrap() {
            let response = execute(row["input"].as_str().unwrap(),row["permission"].as_u64().unwrap() as u8,
                row["current"].as_i64().unwrap() as i32,row["hardcore"].as_bool().unwrap());
            assert_eq!(response.is_ok(),row["ok"].as_bool().unwrap(),"{row}");
            match response {
                Ok(value) => {
                    assert_eq!(value.result,row["result"].as_i64().unwrap() as i32,"{row}");
                    assert_eq!(value.effective,row["effective"].as_i64().unwrap() as i32,"{row}");
                    let component=serde_json::json!({"translate":value.key,"with":[{"translate":format!("options.difficulty.{}",NAMES[value.display_id as usize])}]});
                    let encoded=crate::components::text::Component::from_json(&component).unwrap().to_network_nbt().unwrap();
                    assert_eq!(encoded.iter().map(|b|format!("{b:02x}")).collect::<String>(),row["feedback_hex"].as_str().unwrap(),"{row}");
                }
                Err(error) if row["error_key"].as_str()==Some("commands.difficulty.failure") => {
                    assert_eq!(error.raw_message().key,"commands.difficulty.failure");
                    assert_eq!(error.raw_message().args[0].java_to_string(),row["error_argument"].as_str().unwrap());
                }
                Err(_) => {}
            }
        }
    }
}
