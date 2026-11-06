// SPDX-License-Identifier: GPL-2.0-only
//! Command bridge to the retained border used by collision, login and checkpointing.
use super::*;
use main_administration::{AdminEffect, CommandOutcome};
use mc121_rust::admission_commands::{CommandError, CommandMessage, MessageArg};
impl Server {
    pub(super) fn execute_worldborder_command(
        &self,
        line: &str,
        actor: Option<i32>,
        permission: u8,
    ) -> Result<CommandOutcome> {
        let (position, recipients) = {
            let players = self
                .players
                .read()
                .map_err(|_| anyhow!("players poisoned"))?;
            let position = match actor {
                Some(id) => {
                    let p = players
                        .registry
                        .get(id)
                        .context("command player absent")?
                        .position;
                    Vec3::new(p.x, p.y, p.z)
                }
                None => self.console_command_position,
            };
            (
                position,
                players
                    .registry
                    .snapshot()
                    .iter()
                    .map(|p| p.entity_id)
                    .collect::<Vec<_>>(),
            )
        };
        let mut border = self
            .border
            .write()
            .map_err(|_| anyhow!("border poisoned"))?;
        match mc121_rust::commands::worldborder_command::execute(
            line,
            permission,
            position,
            &mut border,
            self.epoch.elapsed().as_millis() as i64,
        ) {
            Ok(response) => {
                if !response.packets.is_empty() {
                    let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                        recipients: Arc::new(recipients),
                        packets: Arc::new(response.packets),
                    });
                }
                Ok(CommandOutcome {
                    result: Ok(response.result),
                    effects: vec![AdminEffect::Feedback {
                        message: response.message,
                        broadcast_to_ops: response.broadcast,
                    }],
                })
            }
            Err(error) => {
                let raw = error.raw_message();
                let error = if raw.key.starts_with("commands.worldborder.") {
                    let args = raw
                        .args
                        .iter()
                        .map(|arg| match arg {
                            mc121_rust::commands::exception::CommandArg::Double(v) => {
                                MessageArg::Double(*v)
                            }
                            _ => MessageArg::Text(arg.java_to_string()),
                        })
                        .collect();
                    CommandError::Message(CommandMessage::new(raw.key, args))
                } else {
                    CommandError::Host(error.message())
                };
                Ok(CommandOutcome {
                    result: Err(error),
                    effects: vec![],
                })
            }
        }
    }
}
