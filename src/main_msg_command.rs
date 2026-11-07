// SPDX-License-Identifier: GPL-2.0-only
//! Literal online-name whispers over actual retained player/chat owners.
use super::*;
use mc121_rust::{chat_chain::PlayerChatMessage,chat_listener::AuthorizedCommand,chat_wire::BoundChatType,
    commands::msg_command::{self,MessageError},components::text::{Component,Contents,Style,TextColor}};
impl Server{
    fn private_message_failure(&self,sender:Option<i32>,cause:Component)->Result<()>{
        let c=Component{contents:Contents::Text(String::new()),siblings:vec![cause],style:Style{color:Some(TextColor::parse("red")?),..Style::default()}};
        self.command_component_feedback(sender,c,false,true,None)
    }
    pub(super) fn execute_private_message(&self,command:&AuthorizedCommand,sender:Option<i32>)->Result<()>{
        let parsed=match msg_command::parse(&command.command){Ok(m)=>m,Err(e)=>{
            let detail=match e{MessageError::Syntax(e)=>e.message(),MessageError::UnsupportedTarget=>"selector/UUID whisper targets are not implemented (WP-E3)".into(),MessageError::UnsupportedMessage=>"selector expansion in whispers is not implemented (WP-E3)".into()};
            return self.private_message_failure(sender,Component::literal(detail));
        }};
        let now=main_admin_dispatch::wall_time_ms()?;let mut players=self.players.write().map_err(|_|anyhow!("players poisoned"))?;
        let Some(target)=players.registry.snapshot().into_iter().find(|p|p.profile.name.eq_ignore_ascii_case(&parsed.target)) else{
            drop(players);return self.private_message_failure(sender,Component::from_json(&json!({"translate":"argument.entity.notfound.player"}))?);
        };
        let name=if let Some(id)=sender{let p=&players.registry.get(id).context("whisper source missing")?.profile;main_scoreboard::decorated_name(&players.gameplay.scoreboard,p)}else{json!({"text":"Server"})};
        let target_name=main_scoreboard::decorated_name(&players.gameplay.scoreboard,&target.profile);
        let incoming=BoundChatType{registry_id:main_chat_keys::captured_chat_type_named(&self.fixtures.configuration,"minecraft:msg_command_incoming")?,name:name.clone(),target_name:None};
        let outgoing=BoundChatType{registry_id:main_chat_keys::captured_chat_type_named(&self.fixtures.configuration,"minecraft:msg_command_outgoing")?,name,target_name:Some(target_name.clone())};
        let message=match command.signed_arguments.get("message"){Some(m)=>{ensure!(m.signed_content()==parsed.text,"authorized whisper range differs");m.clone()},None=>PlayerChatMessage::system(&parsed.text,now)};
        let outcome=players.chat.send_private_message(sender,target.entity_id,message,incoming,outgoing)?;
        self.publish_chat_outcome(&mut players,sender.unwrap_or(0),outcome)?;drop(players);
        if sender.is_none(){
            let c=msg_command::console_outgoing(Component::from_json(&target_name)?,&parsed.text);
            self.command_component_feedback(None,c,false,false,Some(1))?;
        }
        println!("{}",json!({"event":"private_message_command","source":sender,"target":target.entity_id,"result":1}));Ok(())
    }
}
