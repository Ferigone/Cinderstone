// SPDX-License-Identifier: GPL-2.0-only
//! Explicit saves reuse the actual save transaction without re-entering chat_dispatch.
use super::*;
use mc121_rust::{commands::save_all_command, components::text::Component};

impl Server {
    fn save_command_feedback(&self, source: Option<i32>, component: serde_json::Value,
        broadcast: bool, error: bool, result: Option<i32>) -> Result<()> {
        let (send_feedback, log_admin, online, visible, display) = {
            let players = self.players.read().map_err(|_|anyhow!("players poisoned"))?;
            let display = source.and_then(|id| players.registry.get(id))
                .map(|p| main_scoreboard::decorated_name(&players.gameplay.scoreboard, &p.profile))
                .unwrap_or_else(||json!({"text":"Server"}));
            let online=players.registry.snapshot();
            let visible=online.iter().filter(|p| players.chat.accepts_system_messages(p.entity_id))
                .map(|p|p.entity_id).collect::<BTreeSet<_>>();
            (players.world_context.rules.boolean("sendCommandFeedback").context("missing feedback rule")?,
                players.world_context.rules.boolean("logAdminCommands").context("missing admin log rule")?,
                online, visible, display)
        };
        let packet = |value: &serde_json::Value| -> Result<Packet> {
            let bytes = Component::from_json(value)?.to_network_nbt()?;
            Ok(Packet { id: 0x6c, payload: encoded(|w| {w.raw(&bytes).bool(false);}) })
        };
        if let Some(id) = source {
            if (error || send_feedback) && visible.contains(&id) {
                let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                    recipients: Arc::new(vec![id]), packets: Arc::new(vec![packet(&component)?]),
                });
            }
        } else {
            println!("{}",json!({"event":"administration_feedback","component":component,"result":result,"error":error}));
        }
        if broadcast {
            let admin = json!({"translate":"chat.type.admin","with":[display,component],"color":"gray","italic":true});
            if source.is_some() && log_admin { println!("{}",json!({"event":"administration_feedback","component":admin})); }
            if send_feedback {
                let owner=self.administration.read().map_err(|_|anyhow!("administration poisoned"))?;
                let recipients=online.iter().filter(|p| Some(p.entity_id)!=source && visible.contains(&p.entity_id) && owner.lists().operators.contains_key(&p.profile.uuid))
                    .map(|p|p.entity_id).collect::<Vec<_>>();
                let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                    recipients: Arc::new(recipients), packets: Arc::new(vec![packet(&admin)?]),
                });
            }
        }
        Ok(())
    }

    /// The caller retains the real chat and admission guards. No save is queued
    /// past later commands and the save body does not acquire chat_dispatch again.
    pub(super) async fn try_save_all_command(&self, line: &str, source: Option<i32>,
        sequence: &tokio::sync::MutexGuard<'_, ()>) -> Result<bool> {
        let input = if source.is_none() {line.strip_prefix('/').unwrap_or(line)} else {line};
        if input.split_whitespace().next() != Some("save-all") { return Ok(false); }
        ensure!(!mc121_rust::command_protocol::command_contains_illegal_characters(input), "illegal command characters");
        let permission = if let Some(id)=source {
            let uuid=self.players.read().map_err(|_|anyhow!("players poisoned"))?.registry.get(id).context("save command source missing")?.profile.uuid;
            self.administration.read().map_err(|_|anyhow!("administration poisoned"))?.permission_level(uuid).clamp(0,4) as u8
        } else {4};
        let flush=match save_all_command::parse(input,permission) {
            Ok(flush)=>flush,
            Err(error)=>{
                self.save_command_feedback(source,json!({"text":error.message(),"color":"red"}),false,true,None)?;
                return Ok(true);
            }
        };
        self.save_command_feedback(source,json!({"translate":"commands.save.saving"}),false,false,None)?;
        // Current native/Anvil checkpoint writers complete synchronously in both
        // modes. In particular flush cannot acknowledge queued, unfinished IO.
        match self.save_with_chat_guard(sequence).await {
            Ok(())=>self.save_command_feedback(source,json!({"translate":"commands.save.success"}),true,false,Some(1))?,
            Err(error)=>{
                eprintln!("{}",json!({"event":"explicit_save_failed","flush":flush,"reason":format!("{error:#}")}));
                self.save_command_feedback(source,json!({"text":"","extra":[{"translate":"commands.save.failed"}],"color":"red"}),false,true,None)?;
            }
        }
        Ok(true)
    }
}
