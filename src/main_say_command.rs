// SPDX-License-Identifier: GPL-2.0-only
//! Literal broadcast commands use the actual authorized chat message and ordered audience.
use super::*;
use mc121_rust::{chat_chain::PlayerChatMessage,chat_listener::AuthorizedCommand,chat_wire::BoundChatType,
    commands::say_command::{self,SayError,MessageCommand},components::text::Component};
impl Server {
    pub(super) fn execute_say_command(&self,command:&AuthorizedCommand,sender:Option<i32>,permission:u8)->Result<()> {
        let kind=MessageCommand::from_line(&command.command).context("not a literal broadcast command")?;
        let text=match say_command::execute(&command.command,permission){
            Ok(text)=>text,
            Err(error)=>{
                let detail=match error {SayError::Syntax(e)=>e.message(),SayError::UnsupportedSelector=>"selector expansion in broadcast messages is not implemented (WP-E3)".into()};
                let mut c=Component::literal(detail);c.style.color=Some(mc121_rust::components::text::TextColor::parse("red")?);
                return self.command_component_feedback(sender,c,false,true,None);
            }
        };
        let now=main_admin_dispatch::wall_time_ms()?;
        let mut players=self.players.write().map_err(|_|anyhow!("players poisoned"))?;
        let name=if let Some(id)=sender {
            let profile=&players.registry.get(id).context("say source missing")?.profile;
            main_scoreboard::decorated_name(&players.gameplay.scoreboard,profile)
        }else{json!({"text":"Server"})};
        let bound=BoundChatType{registry_id:main_chat_keys::captured_chat_type_named(&self.fixtures.configuration,kind.chat_type())?,name,target_name:None};
        let message=match command.signed_arguments.get(kind.argument()) {
            Some(m)=>{ensure!(m.signed_content()==text,"authorized broadcast message range differs");m.clone()},
            None=>PlayerChatMessage::system(&text,now),
        };
        let outcome=players.chat.send_say_message(sender,message,bound,now)?;
        // Broadcast outcomes contain broadcasts/logs only; no source action reset.
        self.publish_chat_outcome(&mut players,sender.unwrap_or(0),outcome)?;
        println!("{}",json!({"event":if kind==MessageCommand::Say{"say_command"}else{"emote_command"},"source":sender,"result":1}));Ok(())
    }
}
