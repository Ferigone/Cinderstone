//! Dedicated-server SeedCommand, including ComponentUtils.copyOnClickText.
use super::{
    dispatcher::{ArgumentBuilder, CommandDispatcher},
    exception::CommandSyntaxException,
    suggestion::{SuggestionError, Suggestions},
};
use crate::components::text::Component;
use std::{fmt, rc::Rc};

#[derive(Clone, Copy)]
struct Source { permission: u8, seed: Option<i64> }

fn dispatcher() -> CommandDispatcher<Source> {
    let mut dispatcher = CommandDispatcher::new();
    dispatcher.register(ArgumentBuilder::literal("seed")
        .requires(Rc::new(|source: &Source| source.permission >= 2))
        .executes(Rc::new(|context| Ok(context.source().seed.unwrap_or_default() as i32))));
    dispatcher
}

#[derive(Debug)]
pub enum SeedError { Syntax(CommandSyntaxException), Unavailable }
impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(error) => f.write_str(&error.message()),
            Self::Unavailable => f.write_str("The world seed is unavailable for this world source."),
        }
    }
}

pub struct SeedResponse { pub result: i32, pub component: Component }

/// No clock, world mutation or RNG is accepted by this read-only command.
pub fn execute(input: &str, permission: u8, seed: Option<i64>) -> Result<SeedResponse, SeedError> {
    let result = dispatcher().execute_str(input, &Source { permission, seed }).map_err(SeedError::Syntax)?;
    let seed = seed.ok_or(SeedError::Unavailable)?;
    let text = seed.to_string();
    let component = Component::from_json(&serde_json::json!({
        "translate":"commands.seed.success", "with":[{
            "translate":"chat.square_brackets", "with":[{
                "text":text, "color":"green", "insertion":text,
                "clickEvent":{"action":"copy_to_clipboard","value":text},
                "hoverEvent":{"action":"show_text","contents":{"translate":"chat.copy.click"}}
            }]
        }]
    })).expect("fixed original seed component with decimal integer text");
    Ok(SeedResponse { result, component })
}

pub fn suggestions(input: &str, permission: u8) -> Result<Suggestions, SuggestionError> {
    if permission < 2 { return Ok(Suggestions::empty()); }
    let dispatcher = dispatcher();
    dispatcher.completion_suggestions_at_end(&dispatcher.parse(input, &Source { permission, seed: None }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_seed_tree_results_and_rich_feedback() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/seed-command-vectors.json")).unwrap();
        for row in vectors["trees"].as_array().unwrap() {
            let permission = row["permission"].as_u64().unwrap() as u8;
            let dispatcher = dispatcher();
            let (tree, root) = super::super::tree_packet::usable_tree(dispatcher.tree(), dispatcher.root(), &Source { permission, seed: Some(0) });
            let bytes = super::super::tree_packet::encode_commands_packet(&tree, root).unwrap();
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(hex, row["packet_hex"].as_str().unwrap());
        }
        for row in vectors["execution"].as_array().unwrap() {
            let seed = row["seed"].as_i64().unwrap();
            let response = execute(row["input"].as_str().unwrap(), row["permission"].as_u64().unwrap() as u8, Some(seed));
            assert_eq!(response.is_ok(), row["ok"].as_bool().unwrap(), "{row}");
            if let Ok(response) = response {
                assert_eq!(response.result, row["result"].as_i64().unwrap() as i32, "{row}");
                let mut bytes = response.component.to_network_nbt().unwrap(); bytes.push(0);
                let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                assert_eq!(hex, row["feedback_hex"].as_str().unwrap(), "{row}");
            }
        }
        assert!(matches!(execute("seed", 2, None), Err(SeedError::Unavailable)));
        assert!(matches!(execute("seed", 1, None), Err(SeedError::Syntax(_))));
    }
}
