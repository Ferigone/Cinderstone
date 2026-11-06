// SPDX-License-Identifier: GPL-2.0-only
//! Read-only /list from admitted actors and their actual scoreboard display names.
use super::*;
use mc121_rust::{commands::list_command::{self, ListedPlayer}, components::text::Component};

impl Server {
    pub(super) fn execute_list_command(&self, line: &str, source: Option<i32>) -> Result<()> {
        let maximum = self.maximum_players()?;
        let players = self.players.read().map_err(|_| anyhow!("players poisoned"))?;
        // Monotonically allocated entity ids retain admission order, including rejoins.
        let roster = players.registry.snapshot().iter().map(|player| Ok(ListedPlayer {
            name: player.profile.name.clone(), uuid: mc121_rust::player_data::uuid_string(player.profile.uuid),
            display: Component::from_json(&main_scoreboard::decorated_name(&players.gameplay.scoreboard, &player.profile))?,
        })).collect::<Result<Vec<_>>>()?;
        let feedback = players.world_context.rules.boolean("sendCommandFeedback").context("sendCommandFeedback missing")?;
        drop(players);
        let (component, result, error) = match list_command::execute(line, maximum, &roster) {
            Ok(response) => (response.component, Some(response.result), false),
            Err(error) => (Component::from_json(&json!({"text":error.message(),"color":"red"}))?, None, true),
        };
        if let Some(id) = source {
            if error || feedback {
                let bytes = component.to_network_nbt()?;
                let packet = Packet { id: 0x6c, payload: encoded(|writer| { writer.raw(&bytes).bool(false); }) };
                let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                    recipients: Arc::new(vec![id]), packets: Arc::new(vec![packet]),
                });
            }
        } else {
            println!("{}", json!({"event":"administration_feedback","component":component.to_json(),"result":result,"error":error}));
        }
        Ok(())
    }
}
