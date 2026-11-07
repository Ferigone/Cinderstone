// SPDX-License-Identifier: GPL-2.0-only
//! ExperienceCommand grammar/math over the caller's real retained XP/score owner.
use super::{arguments::{ArgumentNetworkInfo,ArgumentType,ArgumentValue,IntegerArgumentType},dispatcher::{ArgumentBuilder,CommandContext,CommandDispatcher},
    exception::CommandSyntaxException,msg_command::OnlinePlayerArgument,string_reader::StringReader,suggestion::{SuggestionError,Suggestions,SuggestionsBuilder}};
use crate::{experience,player_data::ExperienceData,components::text::{Arg,Component,Contents,Primitive}};
use std::{cell::RefCell,rc::Rc};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Operation{Add,Set,Query}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Unit{Points,Levels}
impl Unit{fn name(self)->&'static str{match self{Self::Points=>"points",Self::Levels=>"levels"}}}
#[derive(Clone,Debug)]
pub struct Action{pub operation:Operation,pub unit:Unit,pub target:String,pub amount:i32}
#[derive(Clone)]
struct Source{permission:u8,action:Rc<RefCell<Option<Action>>>}
fn source(permission:u8)->Source{Source{permission,action:Rc::new(RefCell::new(None))}}
struct SinglePlayerArgument(OnlinePlayerArgument);
impl ArgumentType for SinglePlayerArgument{
    type Value=String;
    fn network_info(&self)->Option<ArgumentNetworkInfo>{Some(ArgumentNetworkInfo{registry_id:6,template:vec![3]})}
    fn parse(&self,r:&mut StringReader)->Result<String,CommandSyntaxException>{self.0.parse(r)}
    fn list_suggestions<S>(&self,c:&CommandContext<S>,b:&mut SuggestionsBuilder)->Result<Suggestions,CommandSyntaxException>{self.0.list_suggestions(c,b)}
}
fn record(c:&CommandContext<Source>,operation:Operation,unit:Unit)->Result<i32,CommandSyntaxException>{
    let Some(ArgumentValue::String(target))=c.argument("targets") else{unreachable!("registered target")};
    let amount=match c.argument("amount"){Some(ArgumentValue::Integer(n))=>*n,_=>0};
    *c.source().action.borrow_mut()=Some(Action{operation,unit,target:target.clone(),amount});Ok(1)
}
fn dispatcher(names:&[String])->CommandDispatcher<Source>{
    let mut d=CommandDispatcher::new();let mut root=ArgumentBuilder::literal("experience").requires(Rc::new(|s:&Source|s.permission>=2));
    for (name,operation,minimum) in [("add",Operation::Add,i32::MIN),("set",Operation::Set,0)]{
        let mut amount=ArgumentBuilder::argument("amount",Rc::new(IntegerArgumentType::new(minimum,i32::MAX))).executes(Rc::new(move |c|record(c,operation,Unit::Points)));
        for unit in [Unit::Points,Unit::Levels]{amount=amount.then(ArgumentBuilder::literal(unit.name()).executes(Rc::new(move |c|record(c,operation,unit))));}
        root=root.then(ArgumentBuilder::literal(name).then(ArgumentBuilder::argument("targets",Rc::new(OnlinePlayerArgument{names:names.to_vec()})).then(amount)));
    }
    let mut query=ArgumentBuilder::argument("targets",Rc::new(SinglePlayerArgument(OnlinePlayerArgument{names:names.to_vec()})));
    for unit in [Unit::Points,Unit::Levels]{query=query.then(ArgumentBuilder::literal(unit.name()).executes(Rc::new(move |c|record(c,Operation::Query,unit))));}
    let root=d.register(root.then(ArgumentBuilder::literal("query").then(query)));
    d.register(ArgumentBuilder::literal("xp").requires(Rc::new(|s:&Source|s.permission>=2)).redirect(root));d
}
#[derive(Debug)]
pub enum ExperienceError{Syntax(CommandSyntaxException),UnsupportedTarget,Domain(Component),Owner(anyhow::Error)}
pub fn parse(input:&str,permission:u8)->Result<Action,ExperienceError>{
    let s=source(permission);dispatcher(&[]).execute_str(input,&s).map_err(ExperienceError::Syntax)?;let a=s.action.borrow_mut().take().expect("executed experience");
    if a.target.starts_with('@')||(a.target.len()==36&&[8,13,18,23].iter().all(|&i|a.target.as_bytes()[i]==b'-')){return Err(ExperienceError::UnsupportedTarget);}Ok(a)
}
pub fn suggestions(input:&str,permission:u8,names:&[String])->Result<Suggestions,SuggestionError>{let d=dispatcher(names);d.completion_suggestions_at_end(&d.parse(input,&source(permission)))}
pub struct Response{pub result:i32,pub component:Component,pub dirty:bool,pub broadcast:bool,pub level_ups:Vec<i32>}
/// Call on a staged copy; the host checks actual effect owners before commit.
pub fn apply(a:&Action,xp:&mut ExperienceData,score:&mut i32,target:Component)->Result<Response,ExperienceError>{
    let mut levels=Vec::new();let query=a.operation==Operation::Query;
    let result=if query{match a.unit{Unit::Levels=>xp.level,Unit::Points=>(xp.progress*experience::xp_needed_for_level(xp.level) as f32).floor() as i32}}else{
        match (a.operation,a.unit){
            (Operation::Add,Unit::Points)=>{let mut previous=xp.level;experience::give_experience_points(xp,a.amount,|level|{if level>previous||(previous==i32::MAX&&level==0){levels.push(level);}previous=level;Ok(())}).map_err(ExperienceError::Owner)?;*score=score.wrapping_add(a.amount);},
            (Operation::Add,Unit::Levels)=>{xp.level=xp.level.wrapping_add(a.amount);if xp.level<0{xp.level=0;xp.progress=0.;xp.total=0;}if a.amount>0{levels.push(xp.level);}},
            (Operation::Set,Unit::Levels)=>xp.level=a.amount,
            (Operation::Set,Unit::Points)=>{let needed=experience::xp_needed_for_level(xp.level);if a.amount>=needed{return Err(ExperienceError::Domain(Component::from_json(&serde_json::json!({"translate":"commands.experience.set.points.invalid"})).expect("fixed feedback")));}let needed=needed as f32;xp.progress=(a.amount as f32/needed).clamp(0.,(needed-1.)/needed);},
            _=>unreachable!("query handled"),
        }1
    };
    if !xp.progress.is_finite()||!(0. ..=1.).contains(&xp.progress)||xp.level<0||xp.total<0{return Err(ExperienceError::Owner(anyhow::anyhow!("resulting XP is outside the persisted native player profile")));}
    let key=if query{format!("commands.experience.query.{}",a.unit.name())}else{format!("commands.experience.{}.{}.success.single",if a.operation==Operation::Add{"add"}else{"set"},a.unit.name())};
    let args=if query{vec![Arg::Component(target),Arg::Primitive(Primitive::Int(result))]}else{vec![Arg::Primitive(Primitive::Int(a.amount)),Arg::Component(target)]};
    Ok(Response{result,component:Component{contents:Contents::Translatable{key,fallback:None,args},siblings:vec![],style:Default::default()},dirty:!query,broadcast:!query,level_ups:levels})
}
#[cfg(test)]
mod tests{
    use super::*;
    fn hex(b:&[u8])->String{b.iter().map(|b|format!("{b:02x}")).collect()}
    fn state(xp:&ExperienceData,score:i32)->serde_json::Value{serde_json::json!({"level":xp.level,"total":xp.total,"progress_bits":xp.progress.to_bits(),"score":score})}
    #[test]
    fn original_experience_tree_completion_sequence_state_dirty_packet_and_feedback(){
        let v:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/experience-command-vectors.json")).unwrap();let names=["ExperienceSource","ExperienceTarget"].map(String::from);
        for row in v["trees"].as_array().unwrap(){let d=dispatcher(&names);let (tree,root)=super::super::tree_packet::usable_tree(d.tree(),d.root(),&source(row["permission"].as_u64().unwrap() as u8));assert_eq!(hex(&super::super::tree_packet::encode_commands_packet(&tree,root).unwrap()),row["hex"].as_str().unwrap());}
        for row in v["suggestions"].as_array().unwrap(){let a=suggestions(row["input"].as_str().unwrap(),2,&names).unwrap();assert_eq!(serde_json::json!({"start":a.range().start(),"end":a.range().end(),"texts":a.list().iter().map(|v|v.text()).collect::<Vec<_>>()}),serde_json::json!({"start":row["start"],"end":row["end"],"texts":row["texts"]}));}
        let mut xp=ExperienceData::default();let mut score=0;
        for row in v["execution"].as_array().unwrap(){
            assert_eq!(state(&xp,score),row["before"]);let parsed=parse(row["input"].as_str().unwrap(),row["permission"].as_u64().unwrap() as u8);let mut component=None;let mut dirty=false;
            match parsed{
                Err(ExperienceError::Syntax(e))=>assert_eq!(e.raw_message().key,row["error_key"].as_str().unwrap()),
                Err(e)=>panic!("unexpected parse {e:?}"),
                Ok(a)=>{
                    if a.target=="Missing"{component=Some(Component::from_json(&serde_json::json!({"translate":"argument.entity.notfound.player"})).unwrap());}
                    else{
                        let mut board=crate::scoreboard::Scoreboard::new_server();if row["team"]==true{for line in ["team add xp","team modify xp color gold","team modify xp prefix {\"text\":\"[X] \",\"font\":\"minecraft:uniform\"}","team modify xp suffix {\"keybind\":\"key.jump\"}","team join xp ExperienceTarget"]{crate::scoreboard::commands::execute(&mut board,line,None,4,&["ExperienceTarget".into()]).unwrap();}}
                        let p=crate::entity::PlayerProfile::offline("ExperienceTarget").unwrap();let target=crate::scoreboard::names::player_holder(&board,&p.name,p.uuid).display_name.unwrap();
                        match apply(&a,&mut xp,&mut score,target){
                            Ok(r)=>{assert_eq!(r.result,row["result"].as_i64().unwrap() as i32);assert_eq!(r.broadcast,a.operation!=Operation::Query);dirty=r.dirty;if !row["sounds"].as_array().unwrap().is_empty(){assert!(r.level_ups.contains(&xp.level));}if a.operation==Operation::Set||(a.operation==Operation::Add&&a.amount<0&&a.unit==Unit::Levels){assert!(r.level_ups.is_empty());}component=Some(r.component);},
                            Err(ExperienceError::Domain(c))=>{assert_eq!(row["ok"],false);component=Some(c);},
                            Err(e)=>panic!("unexpected apply {e:?}"),
                        }
                    }
                }
            }
            if let Some(c)=component{let c=if row["ok"]==true{c}else{Component{contents:Contents::Text(String::new()),siblings:vec![c],style:crate::components::text::Style{color:Some(crate::components::text::TextColor::parse("red").unwrap()),..Default::default()}}};let mut bytes=c.to_network_nbt().unwrap();bytes.push(0);assert_eq!(hex(&bytes),row["feedback"][0].as_str().unwrap(),"{row}");}
            assert_eq!(state(&xp,score),row["after"],"{row}");assert_eq!(dirty,row["dirty"].as_bool().unwrap(),"{row}");if dirty{assert_eq!(hex(&experience::experience_packet(&xp).payload),row["xp_packet"].as_str().unwrap());}
        }
        for input in ["xp query @a points","experience set 00000000-0000-0000-0000-000000000000 3 levels"]{assert!(matches!(parse(input,2),Err(ExperienceError::UnsupportedTarget)));}
    }
}
