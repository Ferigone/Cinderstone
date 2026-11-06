//! /weather mutates the retained WorldContext weather and uses the actual Level.random.
use super::*;
use main_administration::{AdminEffect, CommandOutcome};
use mc121_rust::admission_commands::{CommandError, CommandMessage};

impl Server {
    pub(super) fn execute_weather_command(&self, line: &str, permission: u8) -> Result<CommandOutcome> {
        let mut players = self.players.write().map_err(|_| anyhow!("player registry lock poisoned"))?;
        let LivePlayers { world_context, item_runtime, .. } = &mut *players;
        match mc121_rust::commands::weather_command::execute(
            line, permission, world_context.weather_mut(), item_runtime.world_random_mut(),
        ) {
            Ok(response) => Ok(CommandOutcome {
                result: Ok(response.result),
                effects: vec![AdminEffect::Feedback {
                    message: CommandMessage::new(response.key, vec![]), broadcast_to_ops: true,
                }],
            }),
            Err(error) => Ok(CommandOutcome {
                result: Err(CommandError::Host(error.message())), effects: vec![],
            }),
        }
    }
}
