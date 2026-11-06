//! Server-thread-style sequencing around actual chat, registry and command owners.
use super::*;
use main_chat::{MainChatContext, MainChatEffect, MainChatOutcome};

impl Server {
    fn actual_chat_context(&self, entity_id: i32) -> Result<(MainChatContext, i32)> {
        let uuid = self.players.read().map_err(|_|anyhow!("players poisoned"))?
            .registry.get(entity_id).context("actual chat sender is no longer admitted")?.profile.uuid;
        let owner = self.administration.read().map_err(|_|anyhow!("administration poisoned"))?;
        Ok((MainChatContext {
            now_ms: main_admin_dispatch::wall_time_ms()?,
            enforce_secure_profile: self.current_secure_chat_enforcement(),
            is_operator: owner.lists().operators.contains_key(&uuid),
            is_singleplayer_owner: false,
        }, owner.permission_level(uuid)))
    }

    /// Effects share the existing ordered player event stream. In particular a
    /// session's INITIALIZE_CHAT cannot be overtaken by the following message,
    /// nor a spam disconnect by that sender's final accepted broadcast.
    pub(super) fn publish_chat_outcome(&self, players: &mut LivePlayers, sender: i32,
        outcome: MainChatOutcome) -> Result<(Option<mc121_rust::chat_listener::AuthorizedCommand>, bool)> {
        if outcome.reset_last_action {
            let action = players.player_last_actions.get_mut(&sender).context("actual last-action owner missing")?;
            *action = self.epoch.elapsed().as_millis() as i64;
        }
        for effect in outcome.effects {
            match effect {
                MainChatEffect::Packet { recipient, packet } => {
                    ensure!(players.registry.get(recipient).is_some(), "chat recipient no longer admitted");
                    let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                        recipients: Arc::new(vec![recipient]), packets: Arc::new(vec![packet]),
                    });
                }
                MainChatEffect::Disconnect { recipient, reason } => {
                    let _ = self.entity_events.send(PlayerEvent::ChatDisconnect { recipient, reason });
                }
                MainChatEffect::SessionUpdate { entity_id, session } => {
                    if let Some(event) = players.registry.set_chat_session(entity_id, Some(session))? {
                        players.publish(event, &self.entity_events);
                    }
                }
                MainChatEffect::Warning { entity_id, message } => {
                    eprintln!("{}",json!({"event":"native_chat_warning","entity_id":entity_id,"reason":message}));
                }
                MainChatEffect::ChatLog { sender, content, name, trusted } => {
                    println!("{}",json!({"event":"player_chat","sender":mc121_rust::access_lists::uuid_string(sender),
                        "name":name,"content":content,"trusted":trusted}));
                }
            }
        }
        if let Some(error) = outcome.error { bail!("{error}"); }
        Ok((outcome.command, outcome.charge_command_spam))
    }

    pub(super) async fn handle_chat_packet(&self, entity_id: i32, packet: &Packet) -> Result<()> {
        if packet.id == 0x07 { self.wait_first_chat_key_attempt().await?; }
        else { self.wait_secure_chat_enforcement().await?; }
        let _sequence = self.chat_dispatch.lock().await;
        let admission = self.player_admission.lock().await;
        let keys = Arc::clone(&self.chat_keys.borrow());
        let (context, permission) = self.actual_chat_context(entity_id)?;
        let (command, charge) = {
            let mut players = self.players.write().map_err(|_|anyhow!("players poisoned"))?;
            if packet.id == 0x06 {
                let profile = players.registry.get(entity_id).context("chat sender absent")?.profile.clone();
                let decorated = main_scoreboard::decorated_name(&players.gameplay.scoreboard, &profile);
                players.chat.decorated_names.insert(entity_id, decorated);
            }
            let outcome = match packet.id {
                0x03 => players.chat.handle_ack(entity_id, &packet.payload)?,
                0x04 => {
                    let command = mc121_rust::command_protocol::decode_unsigned_command(&packet.payload)?;
                    players.chat.handle_unsigned_command(entity_id, &command, permission, context)?
                }
                0x05 => players.chat.handle_signed_command(entity_id, &packet.payload, permission, context)?,
                0x06 => players.chat.handle_message(entity_id, &packet.payload, context, self.fixtures.chat_type)?,
                0x07 => players.chat.handle_session_update(entity_id, &packet.payload, Some(&keys))?,
                _ => bail!("not a chat listener packet"),
            };
            self.publish_chat_outcome(&mut players, entity_id, outcome)?
        };
        // Source command parsing, profile resolution and side effects remain
        // owned by the actual administration host, outside the player lock.
        let result = if let Some(command) = command {
            if main_team_chat::is_command(&command.command) {
                self.execute_team_message(&command, Some(entity_id))
            } else {
                self.execute_administration_locked(&command.command, Some(entity_id), &admission)
            }
        } else { Ok(()) };
        if charge {
            let (context, _) = self.actual_chat_context(entity_id)?;
            let mut players = self.players.write().map_err(|_|anyhow!("players poisoned"))?;
            let outcome = players.chat.finish_command(entity_id, context)?;
            self.publish_chat_outcome(&mut players, entity_id, outcome)?;
        }
        result
    }
}
