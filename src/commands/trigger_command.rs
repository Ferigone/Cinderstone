// SPDX-License-Identifier: GPL-2.0-only
//! TriggerCommand grammar and mutation of the caller's actual Scoreboard owner.
use super::{arguments::{ArgumentNetworkInfo,ArgumentType,ArgumentValue,IntegerArgumentType},
    dispatcher::{ArgumentBuilder,CommandContext,CommandDispatcher},exception::CommandSyntaxException,
    string_reader::StringReader,suggestion::{SuggestionError,Suggestions}};
use crate::{components::text::{Arg,Component,Contents,HoverEvent,Primitive,Style,TextColor},scoreboard::{Scoreboard,ScoreHolder,ScoreboardError}};
use std::{cell::RefCell,rc::Rc};

#[derive(Clone,Copy)]
enum Operation { Simple,Add,Set }
struct Action {objective:String,operation:Operation,value:i32}
#[derive(Clone)]
struct Source {objectives:Vec<String>,action:Rc<RefCell<Option<Action>>>}
fn source(objectives:&[String])->Source{Source{objectives:objectives.to_vec(),action:Rc::new(RefCell::new(None))}}
struct ObjectiveArgument;
impl ArgumentType for ObjectiveArgument {
    type Value=String;
    fn parse(&self,r:&mut StringReader)->Result<String,CommandSyntaxException>{Ok(r.read_unquoted_string())}
    fn network_info(&self)->Option<ArgumentNetworkInfo>{Some(ArgumentNetworkInfo{registry_id:23,template:vec![]})}
}
fn record(c:&CommandContext<Source>,operation:Operation)->Result<i32,CommandSyntaxException>{
    let Some(ArgumentValue::String(objective))=c.argument("objective") else {unreachable!("registered objective")};
    let value=if matches!(operation,Operation::Simple){1}else{match c.argument("value"){Some(ArgumentValue::Integer(v))=>*v,_=>unreachable!("registered integer")}};
    *c.source().action.borrow_mut()=Some(Action{objective:objective.clone(),operation,value});Ok(1)
}
fn dispatcher()->CommandDispatcher<Source>{
    let mut d=CommandDispatcher::new();let mut objective=ArgumentBuilder::argument("objective",Rc::new(ObjectiveArgument))
        .suggests(Rc::new(|c:&CommandContext<Source>,b|{
            let remaining=b.remaining_lower_case();
            for name in &c.source().objectives {
                let lower=super::java_text::java_to_lower_case_root(name);
                if lower.starts_with(&remaining)||lower.char_indices().any(|(i,ch)|matches!(ch,'_'|'.')&&lower[i+1..].starts_with(&remaining)){b.suggest(name);}
            }Ok(b.build())
        })).executes(Rc::new(|c|record(c,Operation::Simple)));
    for (name,operation) in [("add",Operation::Add),("set",Operation::Set)] {
        objective=objective.then(ArgumentBuilder::literal(name).then(ArgumentBuilder::argument("value",Rc::new(IntegerArgumentType::new(i32::MIN,i32::MAX)))
            .executes(Rc::new(move |c|record(c,operation)))));
    }
    d.register(ArgumentBuilder::literal("trigger").then(objective));d
}
pub enum TriggerError {Syntax(CommandSyntaxException),Domain(Component),Owner(ScoreboardError)}
impl TriggerError {
    pub fn failure_component(self)->Component {
        let cause=match self {Self::Domain(c)=>c,Self::Syntax(e)=>Component::literal(e.message()),Self::Owner(e)=>Component::literal(e.to_string())};
        Component{contents:Contents::Text(String::new()),siblings:vec![cause],style:Style{color:Some(TextColor::parse("red").expect("fixed color")),..Style::default()}}
    }
}
fn translated(key:&str,args:Vec<Arg>)->Component {Component{contents:Contents::Translatable{key:key.into(),fallback:None,args},siblings:vec![],style:Style::default()}}
fn domain(key:&str)->TriggerError {TriggerError::Domain(translated(key,vec![]))}
pub struct TriggerResponse {pub result:i32,pub component:Component}
/// Parsing and all read-only domain checks precede lock/score changes.
pub fn execute(input:&str,holder:Option<&ScoreHolder>,board:&mut Scoreboard)->Result<TriggerResponse,TriggerError>{
    let s=source(&[]);dispatcher().execute_str(input,&s).map_err(TriggerError::Syntax)?;
    let action=s.action.borrow_mut().take().expect("executed trigger");
    let holder=holder.ok_or_else(||domain("permissions.requires.player"))?;
    let objective=board.objective(&action.objective).ok_or_else(||TriggerError::Domain(translated("arguments.objective.notFound",vec![Arg::Primitive(Primitive::Str(action.objective.clone()))])))?;
    if objective.criteria.name()!="trigger"{return Err(domain("commands.trigger.failed.invalid"));}
    if !board.player_score_info(&holder.name,&action.objective).is_some_and(|score|!score.locked){return Err(domain("commands.trigger.failed.unprimed"));}
    let mut name=objective.display_name.clone();name.style.hover_event=Some(HoverEvent::Text(Box::new(Component::literal(&objective.name))));
    let display=translated("chat.square_brackets",vec![Arg::Component(name)]);
    let mut score=board.get_or_create_player_score(holder,&action.objective,false).map_err(TriggerError::Owner)?;
    score.lock(board);
    let (result,key)=match action.operation {
        Operation::Simple=>(score.increment(board).map_err(TriggerError::Owner)?,"commands.trigger.simple.success"),
        Operation::Add=>(score.add(board,action.value).map_err(TriggerError::Owner)?,"commands.trigger.add.success"),
        Operation::Set=>{score.set(board,action.value).map_err(TriggerError::Owner)?;(action.value,"commands.trigger.set.success")},
    };
    let mut args=vec![Arg::Component(display)];if !matches!(action.operation,Operation::Simple){args.push(Arg::Primitive(Primitive::Int(action.value)));}
    Ok(TriggerResponse{result,component:translated(key,args)})
}
pub fn enabled_objectives(board:&Scoreboard,holder:&str)->Vec<String>{
    board.objectives().into_iter().filter(|o|o.criteria.name()=="trigger"&&board.player_score_info(holder,&o.name).is_some_and(|s|!s.locked)).map(|o|o.name.clone()).collect()
}
pub fn suggestions(input:&str,objectives:&[String])->Result<Suggestions,SuggestionError>{let d=dispatcher();d.completion_suggestions_at_end(&d.parse(input,&source(objectives)))}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoreboard::{DisplaySlot,ObjectiveCriteria,Outbound,ScoreboardPacket};
    fn hex(bytes:&[u8])->String{bytes.iter().map(|b|format!("{b:02x}")).collect()}
    fn fixture()->serde_json::Value{serde_json::from_str(include_str!("../../fixtures/trigger-vectors.json")).unwrap()}
    fn board(initial:i32,enabled:bool)->Scoreboard{
        let mut b=Scoreboard::new_server();let holder=ScoreHolder::named("TriggerUser");
        for name in ["click","deep_click","deep.click","locked","absent","dummy"] {
            let criteria=ObjectiveCriteria::by_name(if name=="dummy"{"dummy"}else{"trigger"}).unwrap().unwrap();
            let display=if name=="click"{Component::from_json(&serde_json::json!({"text":"Press","color":"gold","bold":true})).unwrap()}else{Component::literal(name)};
            let render=criteria.default_render_type();b.add_objective(name,criteria,display,render,false,None).unwrap();
            if name=="click"||name.starts_with("deep")||name=="dummy" {
                let mut score=b.get_or_create_player_score(&holder,name,false).unwrap();score.set(&mut b,if name=="click"{initial}else{0}).unwrap();
                if name!="click"||enabled{score.unlock(&mut b);}
            }
            if name=="click"{b.set_display_objective(DisplaySlot::by_name("sidebar").unwrap(),Some(name)).unwrap();}
        }
        b.take_outbox();b
    }
    fn state(b:&Scoreboard,name:&str)->serde_json::Value{
        b.player_score_info("TriggerUser",name).map(|s|serde_json::json!({"value":s.value,"locked":s.locked})).unwrap_or(serde_json::Value::Null)
    }
    #[test]
    fn original_tree_and_enabled_objective_suggestions(){
        let v=fixture();let d=dispatcher();let b=board(0,true);let names=enabled_objectives(&b,"TriggerUser");let s=source(&names);
        let (tree,root)=super::super::tree_packet::usable_tree(d.tree(),d.root(),&s);
        assert_eq!(hex(&super::super::tree_packet::encode_commands_packet(&tree,root).unwrap()),v["tree_hex"].as_str().unwrap());
        for row in v["suggestions"].as_array().unwrap(){let a=suggestions(row["input"].as_str().unwrap(),&names).unwrap();
            assert_eq!(serde_json::json!({"start":a.range().start(),"end":a.range().end(),"texts":a.list().iter().map(|a|a.text()).collect::<Vec<_>>()}),
                serde_json::json!({"start":row["start"],"end":row["end"],"texts":row["texts"]}),"{row}");}
    }
    #[test]
    fn original_results_typed_feedback_score_packets_and_one_use_locks(){
        let v=fixture();let holder=ScoreHolder::named("TriggerUser");
        for row in v["execution"].as_array().unwrap(){
            let mut b=board(row["initial"].as_i64().unwrap() as i32,row["enabled"].as_bool().unwrap());let input=row["input"].as_str().unwrap();let name=input.split(' ').nth(1).unwrap();
            assert_eq!(state(&b,name),row["before"],"{row}");
            let result=execute(input,(!row["console"].as_bool().unwrap()).then_some(&holder),&mut b);
            assert_eq!(result.is_ok(),row["ok"].as_bool().unwrap(),"{row}");
            match result {
                Ok(r)=>{assert_eq!(r.result,row["result"].as_i64().unwrap() as i32);let mut bytes=r.component.to_network_nbt().unwrap();bytes.push(0);assert_eq!(hex(&bytes),row["feedback"][0].as_str().unwrap());},
                Err(TriggerError::Syntax(error))=>{assert_eq!(error.raw_message().key,row["error_key"].as_str().unwrap());},
                Err(error)=>{let mut bytes=error.failure_component().to_network_nbt().unwrap();bytes.push(0);assert_eq!(hex(&bytes),row["feedback"][0].as_str().unwrap());},
            }
            assert_eq!(state(&b,name),row["after"],"{row}");
            let mut packets=vec![];for out in b.take_outbox(){let values=match out{Outbound::Broadcast(p)=>vec![p],Outbound::EachPlayer(p)=>p};for p in values{if matches!(p,ScoreboardPacket::SetScore{..}){packets.push(hex(&p.encode().unwrap()));}}}
            assert_eq!(serde_json::json!(packets),row["score_packets"],"{row}");
        }
    }
}
