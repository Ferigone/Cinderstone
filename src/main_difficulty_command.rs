//! Retained difficulty updates for the supported single-level server.
use super::*;
use main_administration::{AdminEffect, CommandOutcome};
use mc121_rust::admission_commands::{CommandError, CommandMessage, MessageArg};

pub(super) fn id(value: mc121_rust::survival::Difficulty) -> i32 {
    use mc121_rust::survival::Difficulty::*;
    match value { Peaceful => 0, Easy => 1, Normal => 2, Hard => 3 }
}
impl Server {
    pub(super) fn current_difficulty_packet(&self) -> Result<Packet> {
        let players = self.players.read().map_err(|_|anyhow!("players poisoned"))?;
        Ok(Packet { id: 0x0b, payload: encoded(|w| {w.u8(id(players.native_difficulty) as u8).bool(self.difficulty_locked);}) })
    }
    pub(super) fn execute_difficulty_command(&self, line: &str, permission: u8) -> Result<CommandOutcome> {
        let mut players = self.players.write().map_err(|_|anyhow!("players poisoned"))?;
        let result = mc121_rust::commands::difficulty_command::execute(line,permission,id(players.native_difficulty),self.hardcore);
        match result {
            Ok(response) => {
                if response.changed {
                    players.fire_runtime.set_difficulty(response.effective)?;
                    players.native_difficulty = mc121_rust::survival::Difficulty::from_id(response.effective);
                    let packet = Packet { id: 0x0b, payload: encoded(|w| {w.u8(response.effective as u8).bool(self.difficulty_locked);}) };
                    let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                        recipients: Arc::new(players.registry.snapshot().iter().map(|p|p.entity_id).collect()),
                        packets: Arc::new(vec![packet]),
                    });
                }
                let name = mc121_rust::commands::difficulty_command::NAMES[response.display_id as usize];
                let key = match name { "peaceful"=>"options.difficulty.peaceful", "easy"=>"options.difficulty.easy", "normal"=>"options.difficulty.normal", _=>"options.difficulty.hard" };
                Ok(CommandOutcome { result: Ok(response.result), effects: vec![AdminEffect::Feedback {
                    message: CommandMessage::new(response.key,vec![MessageArg::Component(Box::new(CommandMessage::new(key,vec![])))]),
                    broadcast_to_ops: response.changed,
                }] })
            }
            Err(error) => {
                let error = if error.raw_message().key == "commands.difficulty.failure" {
                    CommandError::Message(CommandMessage::new("commands.difficulty.failure",vec![MessageArg::Text(error.raw_message().args[0].java_to_string())]))
                } else { CommandError::Host(error.message()) };
                Ok(CommandOutcome { result: Err(error), effects: vec![] })
            }
        }
    }
}
