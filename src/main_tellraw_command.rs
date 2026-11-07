// SPDX-License-Identifier: GPL-2.0-only
//! Ordered SystemChat delivery to the actual target and current visibility owner.
use super::*;
use mc121_rust::{commands::tellraw_command::{self,TellrawError},components::text::{Component,Contents,Style,TextColor}};
impl Server{
    fn tellraw_failure(&self,source:Option<i32>,cause:Component)->Result<()>{
        self.command_component_feedback(source,Component{contents:Contents::Text(String::new()),siblings:vec![cause],style:Style{color:Some(TextColor::parse("red")?),..Style::default()}},false,true,None)
    }
    pub(super) fn execute_tellraw_command(&self,line:&str,source:Option<i32>,permission:u8)->Result<()>{
        let message=match tellraw_command::parse(line,permission){Ok(m)=>m,Err(error)=>{
            let cause=match error{TellrawError::Syntax(e)=>Component::literal(e.message()),TellrawError::UnsupportedTarget=>Component::literal("Tellraw targets must be one online player name"),TellrawError::Component(_)=>Component::literal("This text component is not supported"),TellrawError::DynamicComponent=>Component::literal("This text component requires unavailable context resolution")};return self.tellraw_failure(source,cause);
        }};
        let players=self.players.read().map_err(|_|anyhow!("players poisoned"))?;
        let Some(target)=players.registry.get_by_name(&message.target) else{drop(players);return self.tellraw_failure(source,Component::from_json(&json!({"translate":"argument.entity.notfound.player"}))?);};
        let mut payload=match message.component.to_network_nbt(){Ok(p)=>p,Err(_)=>{drop(players);return self.tellraw_failure(source,Component::literal("This text component cannot be sent"));}};payload.push(0);let visible=players.chat.accepts_system_messages(target.entity_id);
        if visible{let _=self.entity_events.send(PlayerEvent::WorldAudience{recipients:Arc::new(vec![target.entity_id]),packets:Arc::new(vec![Packet{id:0x6c,payload}])});}
        println!("{}",json!({"event":"tellraw_command","source":source,"target":target.entity_id,"result":1,"delivered":visible}));Ok(())
    }
}
