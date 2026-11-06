//! /time uses Brigadier and the actual locked, persisted TickClock.
use super::*;
use main_administration::{AdminEffect, CommandOutcome};
use mc121_rust::admission_commands::{CommandError, CommandMessage, MessageArg};

pub(super) fn feedback_component(message: &CommandMessage) -> Result<Vec<u8>> {
    // Use the already-ported ComponentSerialization codec, including Java's
    // map iteration order. The legacy administration codec keeps its old route.
    let tag = mc121_rust::nbt::Tag::decode_anonymous(&message.network_component()?)?;
    mc121_rust::components::text::Component::from_nbt(&tag)
        .and_then(|component| component.to_network_nbt())
        .map_err(|error| anyhow!("{error}"))
}

impl Server {
    pub(super) fn execute_time_command(&self, line: &str, permission: u8) -> Result<CommandOutcome> {
        let mut clock = self.clock.write().map_err(|_| anyhow!("world clock lock poisoned"))?;
        match mc121_rust::commands::time_command::execute(line, permission, &mut clock) {
            Ok(response) => Ok(CommandOutcome {
                result: Ok(response.result),
                effects: vec![AdminEffect::Feedback {
                    message: CommandMessage::new(response.key, vec![MessageArg::Integer(response.value)]),
                    broadcast_to_ops: response.broadcast_to_ops,
                }],
            }),
            Err(error) => Ok(CommandOutcome {
                result: Err(CommandError::Host(error.message())), effects: vec![],
            }),
        }
    }
}
