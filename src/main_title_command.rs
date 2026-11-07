// SPDX-License-Identifier: GPL-2.0-only
//! Actual display packet transport and shared typed command feedback.
use super::*;
use mc121_rust::{commands::{title_command,tellraw_command::TellrawError},components::text::{Component,Contents,Style,TextColor}};
impl Server{
    fn title_failure(&self,source:Option<i32>,cause:Component)->Result<()>{self.command_component_feedback(source,Component{contents:Contents::Text(String::new()),siblings:vec![cause],style:Style{color:Some(TextColor::parse("red")?),..Style::default()}},false,true,None)}
    pub(super) fn execute_title_command(&self,line:&str,source:Option<i32>,permission:u8)->Result<()>{
        let a=match title_command::parse(line,permission){Ok(a)=>a,Err(e)=>{let c=match e{TellrawError::Syntax(e)=>Component::literal(e.message()),TellrawError::UnsupportedTarget=>Component::literal("Title targets must be one online player name"),TellrawError::Component(_)=>Component::literal("This text component is not supported"),TellrawError::DynamicComponent=>Component::literal("This text component requires unavailable context resolution")};return self.title_failure(source,c);}};
        let players=self.players.read().map_err(|_|anyhow!("players poisoned"))?;let Some(target)=players.registry.get_by_name(&a.target)else{drop(players);return self.title_failure(source,Component::from_json(&json!({"translate":"argument.entity.notfound.player"}))?);};
        let packet=match title_command::packet(&a){Ok(p)=>p,Err(_)=>{drop(players);return self.title_failure(source,Component::literal("This text component cannot be sent"));}};
        let display=Component::from_json(&main_scoreboard::decorated_name(&players.gameplay.scoreboard,&target.profile))?;let target_id=target.entity_id;
        let _=self.entity_events.send(PlayerEvent::WorldAudience{recipients:Arc::new(vec![target_id]),packets:Arc::new(vec![packet])});drop(players);
        self.command_component_feedback(source,title_command::success(a.kind,display),true,false,Some(1))?;
        println!("{}",json!({"event":"title_command","source":source,"target":target_id,"kind":a.kind.name(),"result":1}));Ok(())
    }
}
