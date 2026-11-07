// SPDX-License-Identifier: GPL-2.0-only
//! Commands use actual lifecycle XP, metadata score, dirty cache and level-up owners.
use super::*;
use mc121_rust::{commands::experience_command::{self,ExperienceError},components::text::{Component,Contents,Style,TextColor}};
impl Server{
    fn experience_failure(&self,source:Option<i32>,cause:Component)->Result<()>{
        self.command_component_feedback(source,Component{contents:Contents::Text(String::new()),siblings:vec![cause],style:Style{color:Some(TextColor::parse("red")?),..Style::default()}},false,true,None)
    }
    fn experience_error(&self,source:Option<i32>,error:ExperienceError)->Result<()>{
        let cause=match error{ExperienceError::Domain(c)=>c,ExperienceError::Syntax(e)=>Component::literal(e.message()),ExperienceError::UnsupportedTarget=>Component::literal("Experience targets must be one online player name"),ExperienceError::Owner(_)=>Component::literal("This XP change is not supported for the player's current XP state")};self.experience_failure(source,cause)
    }
    pub(super) fn execute_experience_command(&self,line:&str,source:Option<i32>,permission:u8)->Result<()>{
        let a=match experience_command::parse(line,permission){Ok(a)=>a,Err(e)=>return self.experience_error(source,e)};
        let mut players=self.players.write().map_err(|_|anyhow!("players poisoned"))?;let Some(target)=players.registry.get_by_name(&a.target).cloned() else{drop(players);return self.experience_failure(source,Component::from_json(&json!({"translate":"argument.entity.notfound.player"}))?);};
        let uuid=target.profile.uuid;let mut xp=players.player_lifecycle.get(&uuid).context("XP lifecycle missing")?.experience.clone();let mut score=match target.entity_data.get(16){Some(MetadataValue::Int(n))=>*n,_=>bail!("XP score owner missing")};
        let display=Component::from_json(&main_scoreboard::decorated_name(&players.gameplay.scoreboard,&target.profile))?;
        let physical=players.player_motion_runtime.players.get(&target.entity_id).map(|p|p.motion.physical.clone());
        if a.operation!=experience_command::Operation::Query {
            let supported=self.options.anvil_world.is_none()&&target.gamemode==1&&matches!(target.entity_data.get(9),Some(MetadataValue::Float(h))if *h>0.)&&physical.is_some()&&players.native_body_ready.contains(&target.entity_id);
            if !supported{drop(players);return self.experience_failure(source,Component::literal("Experience changes are unavailable for this world/player profile"));}
            let equipment=require_experience_equipment(players.containers.get(&target.entity_id).context("XP inventory owner missing")?);
            if equipment.is_err()||players.gameplay.require_supported_floor().is_err(){drop(players);return self.experience_failure(source,Component::literal("Experience changes are unavailable for this player profile"));}
        }
        let response=match experience_command::apply(&a,&mut xp,&mut score,display){Ok(r)=>r,Err(e)=>{drop(players);return self.experience_error(source,e);}};
        let last_level_up=players.experience_runtime.last_level_up_tick(target.entity_id);let sound_before=players.item_runtime.sound_random_state48();
        if response.dirty{
            if self.options.anvil_world.is_some()||target.gamemode!=1||!matches!(target.entity_data.get(9),Some(MetadataValue::Float(h))if *h>0.)||physical.is_none(){drop(players);return self.experience_failure(source,Component::literal("Experience changes are unavailable for this world/player profile"));}
            let physical=physical.as_ref().expect("checked physical owner");
            players.experience_runtime.last_sent_experience_mut(target.entity_id)?;
            if let Err(_)=players.experience_runtime.require_level_up_sound_owner(target.entity_id,&response.level_ups,physical,&players.item_runtime){drop(players);return self.experience_failure(source,Component::literal("Experience changes are unavailable in this world profile"));}
            players.registry.set_entity_data(target.entity_id,16,MetadataValue::Int(score),false)?;
            players.player_lifecycle.get_mut(&uuid).expect("checked lifecycle").experience=xp.clone();
            *players.experience_runtime.last_sent_experience_mut(target.entity_id)?=-1;
            let LivePlayers{experience_runtime,item_runtime,registry,..}=&mut *players;
            experience_runtime.level_up_sounds(target.entity_id,&response.level_ups,physical,item_runtime,registry,&self.entity_events)?;
        }
        let sound_after=players.item_runtime.sound_random_state48();drop(players);
        self.command_component_feedback(source,response.component,response.broadcast,false,Some(response.result))?;
        println!("{}",json!({"event":"experience_command","command":line,"target":target.entity_id,"result":response.result,"dirty":response.dirty,"level":xp.level,"total":xp.total,"progress_bits":xp.progress.to_bits(),"score":score,"target_tick":physical.as_ref().map(|p|p.tick_count),"last_level_up_before":last_level_up,"sound_random_before":sound_before,"sound_random_after":sound_after}));Ok(())
    }
}

fn require_experience_equipment(container:&SharedContainer)->Result<()>{
    container.read(|menu|->Result<()>{
        main_player_motion_hooks::require_equipment_defaults(&menu.inventory)?;
        ensure!(!menu.inventory.slot(5).is_some_and(|stack|!stack.is_empty()&&inventory::item_name(stack.item_id)==Some("turtle_helmet")),"unsupported active-effect equipment");Ok(())
    })?
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn actual_container_equipment_validation_propagates_inner_failure(){
        let (empty,_)=SharedContainer::new();assert!(require_experience_equipment(&empty).is_ok());
        let turtle=(0..2000).find(|id|inventory::item_name(*id)==Some("turtle_helmet")).unwrap();
        let mut menu=PlayerContainer::new();
        menu.inventory.set_creative_slot(5,inventory::ItemStack::new(turtle,1).unwrap()).unwrap();
        let (equipped,_)=SharedContainer::from_container(menu);
        assert!(equipped.read(|m|!m.inventory.slot(5).unwrap().is_empty()).unwrap());
        assert!(require_experience_equipment(&equipped).is_err());
    }
}
