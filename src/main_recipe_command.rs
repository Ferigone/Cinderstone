// SPDX-License-Identifier: GPL-2.0-only
//! Single-recipe commands mutate the actual retained target book/advancement owners.
use super::*;
use mc121_rust::{commands::recipe_command::{self,Operation,RecipeError},components::text::{Component,Contents,Style,TextColor},server_recipe_book::RecipeId};
impl Server{
    fn recipe_command_failure(&self,source:Option<i32>,cause:Component)->Result<()>{
        let c=Component{contents:Contents::Text(String::new()),siblings:vec![cause],style:Style{color:Some(TextColor::parse("red")?),..Style::default()}};
        self.command_component_feedback(source,c,false,true,None)
    }
    pub(super) fn execute_recipe_command(&self,line:&str,source:Option<i32>,permission:u8)->Result<()>{
        let a=match recipe_command::parse(line,permission){Ok(a)=>a,Err(e)=>{
            let text=match e{RecipeError::Syntax(e)=>e.message(),RecipeError::UnsupportedTarget=>"Recipe targets must be one online player name".into(),RecipeError::UnsupportedWildcard=>"Use one recipe ID; wildcard recipe commands are not supported".into()};
            return self.recipe_command_failure(source,Component::literal(text));
        }};
        let mut players=self.players.write().map_err(|_|anyhow!("players poisoned"))?;
        let Some(target)=players.registry.get_by_name(&a.target).cloned() else{drop(players);return self.recipe_command_failure(source,Component::from_json(&json!({"translate":"argument.entity.notfound.player"}))?);};
        let id=RecipeId::parse(&a.recipe)?;let Some(recipe)=VanillaRecipeCatalogue::get().descriptor(&id) else{
            drop(players);let c=recipe_command::unknown_recipe(&a.recipe);return self.recipe_command_failure(source,c);
        };
        let display=Component::from_json(&main_scoreboard::decorated_name(&players.gameplay.scoreboard,&target.profile))?;
        let (count,packets)=self.mutate_command_recipe_locked(&mut players,target.profile.uuid,&recipe,a.operation==Operation::Give)?;
        if !packets.is_empty(){let _=self.entity_events.send(PlayerEvent::WorldAudience{recipients:Arc::new(vec![target.entity_id]),packets:Arc::new(packets)});}
        drop(players);
        if count==0{return self.recipe_command_failure(source,recipe_command::zero_result(a.operation));}
        self.command_component_feedback(source,recipe_command::success(a.operation,display),true,false,Some(count))?;
        println!("{}",json!({"event":"recipe_command","target":target.entity_id,"recipe":a.recipe,"result":count}));Ok(())
    }
}
