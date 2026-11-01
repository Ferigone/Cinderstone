//! Production transport and actual-player bridge for native administration.
use super::*;
use main_administration::{AdminEffect, OnlineProfile};
use mc121_rust::{
    access_lists::AccessProfile,
    admission_commands::{self, CommandSource},
    admission_policy::AdmissionRejection,
    command_protocol::{self, SuggestionContext, SuggestionRequest},
};

pub(super) fn wall_time_ms() -> Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis().try_into()?)
}

/// A dedicated console reader never owns the asynchronous runtime or player
/// state. EOF disables its select branch; shutdown does not join a blocked read.
pub(super) fn console_input() -> mpsc::UnboundedReceiver<String> {
    let (sender, receiver) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => { if sender.send(line).is_err() { break; } },
                Err(_) => break,
            }
        }
    });
    receiver
}

fn permission(level: i32) -> u8 { level.clamp(0, 4) as u8 }

fn permission_packets(entity_id: i32, level: i32) -> Result<Vec<Packet>> {
    let level = permission(level);
    Ok(vec![
        Packet { id: 0x1f, payload: encoded(|w| { w.i32(entity_id).u8(24 + level); }) },
        command_protocol::administration_command_tree(level)?,
    ])
}

fn system_message(component: Vec<u8>) -> Packet {
    Packet { id: 0x6c, payload: encoded(|w| { w.raw(&component).bool(false); }) }
}

impl Server {
    pub(super) fn maximum_players(&self) -> Result<i32> {
        Ok(self.administration.read().map_err(|_| anyhow!("administration lock poisoned"))?.settings().max_players)
    }

    pub(super) fn online_administration_profiles(&self) -> Result<Vec<OnlineProfile>> {
        let players = self.players.read().map_err(|_| anyhow!("player registry lock poisoned"))?;
        players.registry.snapshot().into_iter().map(|player| {
            let address = *players.connection_addresses.get(&player.entity_id)
                .context("actual admitted connection address missing")?;
            Ok(OnlineProfile { entity_id: player.entity_id,
                profile: AccessProfile { uuid: player.profile.uuid, name: player.profile.name }, address })
        }).collect()
    }

    pub(super) async fn check_initial_admission(&self, profile: &PlayerProfile, address: SocketAddr,
        codec: &FrameCodec, writer: &mut NetworkWriter, _admission: &tokio::sync::MutexGuard<'_, ()>) -> Result<bool> {
        // Source policy runs before compression, duplicate eviction and success.
        let online = {
            let players = self.players.read().map_err(|_| anyhow!("player registry lock poisoned"))?;
            players.registry.len()
        };
        let rejection = {
            let mut owner = self.administration.write().map_err(|_| anyhow!("administration lock poisoned"))?;
            owner.remember_profile(AccessProfile { uuid: profile.uuid, name: profile.name.clone() });
            owner.can_player_login(profile.uuid, address, online, wall_time_ms()?)?
        };
        if let Some(rejection) = rejection {
            let component = rejection.json_component()?.to_string();
            send(codec, writer, 0x00, encoded(|w| { w.string(&component); })).await?;
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) async fn evict_duplicate_for_login(&self, profile: &PlayerProfile,
        admission: &tokio::sync::MutexGuard<'_, ()>) -> Result<()> {
        // Java enables negotiated compression before evicting an old owner.
        let duplicate = self.players.read().map_err(|_| anyhow!("player registry lock poisoned"))?
            .registry.get_by_uuid(&profile.uuid).map(|player| player.entity_id);
        if let Some(previous) = duplicate {
            self.cleanup_player_admitted(previous, Some("multiplayer.disconnect.duplicate_login"), admission).await?;
        }
        Ok(())
    }

    /// Called under player_admission after the CONFIGURATION finish and before
    /// allocating or constructing a new native player. Outbound is already PLAY.
    pub(super) fn check_final_admission(&self, profile: &PlayerProfile, address: SocketAddr) -> Result<Option<AdmissionRejection>> {
        let (online, duplicate) = {
            let players = self.players.read().map_err(|_| anyhow!("player registry lock poisoned"))?;
            (players.registry.len(), players.registry.get_by_uuid(&profile.uuid).is_some())
        };
        self.administration.write().map_err(|_| anyhow!("administration lock poisoned"))?
            .can_finish_configuration(duplicate, profile.uuid, address, online, wall_time_ms()?)
    }

    pub(super) fn administration_permission_packets(&self, entity_id: i32) -> Result<Vec<Packet>> {
        let uuid = self.players.read().map_err(|_| anyhow!("player registry lock poisoned"))?
            .registry.get(entity_id).context("permission recipient absent")?.profile.uuid;
        let level = self.administration.read().map_err(|_| anyhow!("administration lock poisoned"))?.permission_level(uuid);
        permission_packets(entity_id, level)
    }

    pub(super) fn administration_suggestions(&self, entity_id: i32, request: &SuggestionRequest) -> Result<Packet> {
        let online = self.online_administration_profiles()?;
        let player = online.iter().find(|player| player.entity_id == entity_id).context("command recipient absent")?;
        let owner = self.administration.read().map_err(|_| anyhow!("administration lock poisoned"))?;
        let lists = owner.lists();
        let online_names = online.iter().map(|p| p.profile.name.clone()).collect::<Vec<_>>();
        let operators = lists.operators.values().map(|p| p.profile.name.clone()).collect::<Vec<_>>();
        let banned = lists.player_bans.values().map(|p| p.profile.name.clone()).collect::<Vec<_>>();
        let banned_ips = lists.ip_bans.keys().cloned().collect::<Vec<_>>();
        let whitelist = lists.whitelist.values().map(|p| p.name.clone()).collect::<Vec<_>>();
        let op_candidates = online.iter().filter(|p| !lists.operators.contains_key(&p.profile.uuid)).map(|p| p.profile.name.clone()).collect::<Vec<_>>();
        let whitelist_add_candidates = online.iter().filter(|p| !lists.whitelist.contains_key(&p.profile.uuid)).map(|p| p.profile.name.clone()).collect::<Vec<_>>();
        command_protocol::administration_suggestions(request, permission(owner.permission_level(player.profile.uuid)), &SuggestionContext {
            online_player_names: &online_names,
            operator_names: &operators,
            banned_player_names: &banned,
            banned_ip_addresses: &banned_ips,
            whitelisted_names: &whitelist,
            op_candidates: &op_candidates,
            whitelist_add_candidates: &whitelist_add_candidates,
        })
    }

    pub(super) async fn execute_administration(&self, line: &str, source_id: Option<i32>) -> Result<()> {
        let _sequence = self.chat_dispatch.lock().await;
        // Match the source server-thread serialization: a policy mutation must
        // not fit between final admission and publication of the actual actor.
        let admission = self.player_admission.lock().await;
        self.execute_administration_locked(line, source_id, &admission)
    }

    pub(super) fn execute_administration_locked(&self, line: &str, source_id: Option<i32>,
        _admission: &tokio::sync::MutexGuard<'_, ()>) -> Result<()> {
        if command_protocol::command_contains_illegal_characters(line) {
            bail!("illegal command characters");
        }
        let online = self.online_administration_profiles()?;
        let mut owner = self.administration.write().map_err(|_| anyhow!("administration lock poisoned"))?;
        let source = match source_id {
            Some(id) => {
                let source = online.iter().find(|p| p.entity_id == id).context("command source no longer admitted")?;
                CommandSource { name: source.profile.name.clone(), permission_level: permission(owner.permission_level(source.profile.uuid)) }
            }
            None => CommandSource { name: "Server".into(), permission_level: 4 },
        };
        let outcome = owner.execute_line(line, &source, source_id, &online, wall_time_ms()?) ;
        drop(owner);
        let (send_feedback, log_admin) = {
            let players = self.players.read().map_err(|_| anyhow!("player registry lock poisoned"))?;
            (players.world_context.rules.boolean("sendCommandFeedback").context("missing sendCommandFeedback")?,
             players.world_context.rules.boolean("logAdminCommands").context("missing logAdminCommands")?)
        };
        for effect in outcome.effects {
            match effect {
                AdminEffect::PersistenceWarning { file, message } => {
                    eprintln!("{}", json!({"event":"administration_persistence_warning","file":file,"reason":message}));
                }
                AdminEffect::PermissionLevel { entity_id, uuid, level } => {
                    if online.iter().any(|p| p.entity_id == entity_id && p.profile.uuid == uuid) {
                        let _ = self.entity_events.send(PlayerEvent::WorldAudience { recipients: Arc::new(vec![entity_id]), packets: Arc::new(permission_packets(entity_id, level)?) });
                    }
                }
                AdminEffect::Disconnect { entity_id, uuid, reason } => {
                    if online.iter().any(|p| p.entity_id == entity_id && p.profile.uuid == uuid) {
                        let _ = self.entity_events.send(PlayerEvent::AdminDisconnect { recipient: entity_id, reason: reason.network_component()? });
                    }
                }
                AdminEffect::Feedback { message, broadcast_to_ops } => {
                    if source_id.is_none() {
                        println!("{}", json!({"event":"administration_feedback","component":message.json_component()}));
                    } else if send_feedback {
                        let _ = self.entity_events.send(PlayerEvent::WorldAudience { recipients: Arc::new(vec![source_id.unwrap()]), packets: Arc::new(vec![system_message(message.network_component()?)]) });
                    }
                    if broadcast_to_ops {
                        let source_display = source_id.and_then(|id| online.iter().find(|p| p.entity_id == id))
                            .map(|player| admission_commands::default_player_display_component(&player.profile))
                            .unwrap_or_else(|| json!({"text":source.name}));
                        let admin_message = json!({"translate":"chat.type.admin", "with":[source_display,message.json_component()], "color":"gray","italic":true});
                        if log_admin && source_id.is_some() {
                            println!("{}", json!({"event":"administration_feedback","component":admin_message}));
                        }
                        if send_feedback {
                            let owner=self.administration.read().map_err(|_| anyhow!("administration lock poisoned"))?;
                            let packet=system_message(admission_commands::encode_component_json(&admin_message)?);
                            for player in &online {
                                if Some(player.entity_id)!=source_id && owner.lists().operators.contains_key(&player.profile.uuid) {
                                    let _=self.entity_events.send(PlayerEvent::WorldAudience { recipients:Arc::new(vec![player.entity_id]), packets:Arc::new(vec![packet.clone()]) });
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Err(error) = outcome.result {
            if let Some(id) = source_id {
                let component = match &error {
                    admission_commands::CommandError::Message(message) => {
                        let mut component=message.json_component(); component["color"]=json!("red"); component
                    }
                    _ => json!({"text":error.to_string(),"color":"red"}),
                };
                let packet=system_message(admission_commands::encode_component_json(&component)?);
                let _=self.entity_events.send(PlayerEvent::WorldAudience {recipients:Arc::new(vec![id]), packets:Arc::new(vec![packet])});
            } else {
                println!("{}",json!({"event":"administration_command_error","reason":error.to_string()}));
            }
        }
        Ok(())
    }
}
