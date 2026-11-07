// SPDX-License-Identifier: GPL-2.0-only
//! TitleCommand grammar and packets; animation state belongs to the client.
use super::{arguments::ArgumentValue,dispatcher::{ArgumentBuilder,CommandContext,CommandDispatcher},msg_command::OnlinePlayerArgument,simple_arguments::TimeArgumentType,suggestion::{SuggestionError,Suggestions},tellraw_command::{self,JsonComponentArgument,TellrawError}};
use crate::{components::text::{Arg,Component,Contents},protocol::{Packet,Writer}};
use std::{cell::RefCell,rc::Rc};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Kind{Clear,Reset,Title,Subtitle,Actionbar,Times}
impl Kind{
    pub fn name(self)->&'static str{match self{Self::Clear=>"clear",Self::Reset=>"reset",Self::Title=>"title",Self::Subtitle=>"subtitle",Self::Actionbar=>"actionbar",Self::Times=>"times"}}
}
#[derive(Clone)]
struct Pending{kind:Kind,target:String,json:Option<String>,times:[i32;3]}
#[derive(Clone)]
struct Source{permission:u8,action:Rc<RefCell<Option<Pending>>>}
fn source(permission:u8)->Source{Source{permission,action:Rc::new(RefCell::new(None))}}
fn record(c:&CommandContext<Source>,kind:Kind)->Result<i32,super::exception::CommandSyntaxException>{
    let Some(ArgumentValue::String(target))=c.argument("targets") else{unreachable!("target")};let json=match c.argument("title"){Some(ArgumentValue::String(s))=>Some(s.clone()),_=>None};
    let times=["fadeIn","stay","fadeOut"].map(|n|match c.argument(n){Some(ArgumentValue::Integer(n))=>*n,_=>0});
    *c.source().action.borrow_mut()=Some(Pending{kind,target:target.clone(),json,times});Ok(1)
}
fn dispatcher(names:&[String])->CommandDispatcher<Source>{
    let mut d=CommandDispatcher::new();let mut target=ArgumentBuilder::argument("targets",Rc::new(OnlinePlayerArgument{names:names.to_vec()}));
    for kind in [Kind::Clear,Kind::Reset]{target=target.then(ArgumentBuilder::literal(kind.name()).executes(Rc::new(move |c|record(c,kind))));}
    for kind in [Kind::Title,Kind::Subtitle,Kind::Actionbar]{target=target.then(ArgumentBuilder::literal(kind.name()).then(ArgumentBuilder::argument("title",Rc::new(JsonComponentArgument)).executes(Rc::new(move |c|record(c,kind)))));}
    let times=ArgumentBuilder::literal("times").then(ArgumentBuilder::argument("fadeIn",Rc::new(TimeArgumentType::new())).then(ArgumentBuilder::argument("stay",Rc::new(TimeArgumentType::new())).then(ArgumentBuilder::argument("fadeOut",Rc::new(TimeArgumentType::new())).executes(Rc::new(|c|record(c,Kind::Times))))));
    d.register(ArgumentBuilder::literal("title").requires(Rc::new(|s:&Source|s.permission>=2)).then(target.then(times)));d
}
pub struct Action{pub kind:Kind,pub target:String,pub component:Option<Component>,pub times:[i32;3]}
pub fn parse(input:&str,permission:u8)->Result<Action,TellrawError>{
    let s=source(permission);dispatcher(&[]).execute_str(input,&s).map_err(TellrawError::Syntax)?;let a=s.action.borrow_mut().take().expect("executed title");
    if a.target.starts_with('@')||(a.target.len()==36&&[8,13,18,23].iter().all(|&i|a.target.as_bytes()[i]==b'-')){return Err(TellrawError::UnsupportedTarget);}
    let component=a.json.as_deref().map(tellraw_command::static_component).transpose()?;Ok(Action{kind:a.kind,target:a.target,component,times:a.times})
}
pub fn suggestions(input:&str,permission:u8,names:&[String])->Result<Suggestions,SuggestionError>{let d=dispatcher(names);d.completion_suggestions_at_end(&d.parse(input,&source(permission)))}
pub fn packet(a:&Action)->anyhow::Result<Packet>{
    let mut w=Writer::new();let id=match a.kind{Kind::Clear|Kind::Reset=>{w.bool(a.kind==Kind::Reset);0x0f},Kind::Times=>{for n in a.times{w.i32(n);}0x66},kind=>{w.raw(&a.component.as_ref().expect("parsed title component").to_network_nbt()?);match kind{Kind::Title=>0x65,Kind::Subtitle=>0x63,Kind::Actionbar=>0x4c,_=>unreachable!()}}};Ok(Packet{id,payload:w.into_inner()})
}
pub fn success(kind:Kind,target:Component)->Component{Component{contents:Contents::Translatable{key:format!("commands.title.{}.single",match kind{Kind::Clear=>"cleared",Kind::Reset=>"reset",Kind::Title=>"show.title",Kind::Subtitle=>"show.subtitle",Kind::Actionbar=>"show.actionbar",Kind::Times=>"times"}),fallback:None,args:vec![Arg::Component(target)]},siblings:vec![],style:Default::default()}}
#[cfg(test)]
mod tests{
    use super::*;
    fn hex(b:&[u8])->String{b.iter().map(|b|format!("{b:02x}")).collect()}
    #[test]
    fn original_title_tree_completion_packets_times_and_typed_feedback(){
        let v:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/title-vectors.json")).unwrap();let names=["TitleSource","TitleTarget","TitleOutside"].map(String::from);
        for row in v["trees"].as_array().unwrap(){let d=dispatcher(&names);let (tree,root)=super::super::tree_packet::usable_tree(d.tree(),d.root(),&source(row["permission"].as_u64().unwrap() as u8));assert_eq!(hex(&super::super::tree_packet::encode_commands_packet(&tree,root).unwrap()),row["hex"].as_str().unwrap());}
        for row in v["suggestions"].as_array().unwrap(){let a=suggestions(row["input"].as_str().unwrap(),2,&names).unwrap();assert_eq!(serde_json::json!({"start":a.range().start(),"end":a.range().end(),"texts":a.list().iter().map(|s|s.text()).collect::<Vec<_>>()}),serde_json::json!({"start":row["start"],"end":row["end"],"texts":row["texts"].as_array().unwrap().iter().filter(|v|!v.as_str().unwrap().starts_with('@')).collect::<Vec<_>>()}));}
        for row in v["execution"].as_array().unwrap(){let a=parse(row["input"].as_str().unwrap(),row["permission"].as_u64().unwrap() as u8);if row["ok"]==true{
            let a=a.unwrap();let p=packet(&a).unwrap();assert_eq!(serde_json::json!({"id":p.id,"hex":hex(&p.payload)}),row["packet"]);
            let mut board=crate::scoreboard::Scoreboard::new_server();if row["team"]==true{for line in ["team add title","team modify title color gold","team modify title prefix {\"text\":\"[T] \",\"font\":\"minecraft:uniform\"}","team modify title suffix {\"keybind\":\"key.jump\"}","team join title TitleTarget"]{crate::scoreboard::commands::execute(&mut board,line,None,4,&["TitleTarget".into()]).unwrap();}}
            let profile=crate::entity::PlayerProfile::offline(if a.target.eq_ignore_ascii_case("TitleTarget"){"TitleTarget"}else{"TitleSource"}).unwrap();let target=crate::scoreboard::names::player_holder(&board,&profile.name,profile.uuid).display_name.unwrap();let mut bytes=success(a.kind,target).to_network_nbt().unwrap();bytes.push(0);assert_eq!(hex(&bytes),row["feedback"][0].as_str().unwrap(),"{row}");
        }else if row["error_key"]!="argument.entity.notfound.player"{let Err(TellrawError::Syntax(e))=a else{panic!("syntax expected {row}")};assert_eq!(e.raw_message().key,row["error_key"].as_str().unwrap());}}
    }
}
