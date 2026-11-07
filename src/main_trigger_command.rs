// SPDX-License-Identifier: GPL-2.0-only
//! Native trigger commands use the real retained scoreboard and its existing publisher.
use super::*;
use mc121_rust::{commands::trigger_command,scoreboard};
impl Server {
    pub(super) fn execute_trigger_command(&self,line:&str,source:Option<i32>)->Result<()> {
        let result={
            let mut players=self.players.write().map_err(|_|anyhow!("players poisoned"))?;
            let holder=source.map(|id|{
                let profile=&players.registry.get(id).context("trigger source missing")?.profile;
                Ok::<_,anyhow::Error>(scoreboard::names::player_holder(&players.gameplay.scoreboard,&profile.name,profile.uuid))
            }).transpose()?;
            let result=trigger_command::execute(line,holder.as_ref(),&mut players.gameplay.scoreboard);
            main_scoreboard::publish(&mut players,&self.entity_events)?;
            result
        };
        match result {
            Ok(response)=>self.command_component_feedback(source,response.component,true,false,Some(response.result)),
            Err(error)=>self.command_component_feedback(source,error.failure_component(),false,true,None),
        }
    }
}
