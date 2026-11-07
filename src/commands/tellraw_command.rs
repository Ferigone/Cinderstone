// SPDX-License-Identifier: GPL-2.0-only
//! Static JSON components to one actual online player; context-dependent content is explicit.
use super::{arguments::{ArgumentNetworkInfo,ArgumentType,ArgumentValue},dispatcher::{ArgumentBuilder,CommandContext,CommandDispatcher},exception::{built_in,CommandSyntaxException},msg_command::OnlinePlayerArgument,string_reader::StringReader,suggestion::{SuggestionError,Suggestions}};
use crate::components::text::{Arg,Component,Contents,HoverEvent};
use std::{cell::RefCell,rc::Rc};
#[derive(Clone)]
struct Source{permission:u8,action:Rc<RefCell<Option<(String,String)>>>}
fn source(permission:u8)->Source{Source{permission,action:Rc::new(RefCell::new(None))}}
struct JsonComponentArgument;
impl ArgumentType for JsonComponentArgument{
    type Value=String;
    fn network_info(&self)->Option<ArgumentNetworkInfo>{Some(ArgumentNetworkInfo{registry_id:17,template:vec![]})}
    fn parse(&self,r:&mut StringReader)->Result<String,CommandSyntaxException>{
        let input=r.remaining();let mut stream=serde_json::Deserializer::from_str(&input).into_iter::<serde_json::Value>();
        match stream.next(){Some(Ok(_))=>{},_=>return Err(built_in::simple("argument.component.invalid",r.units(),r.cursor()))}
        let json=input[..stream.byte_offset()].to_owned();
        // The full component codec is checked after Brigadier has rejected trailing syntax.
        r.set_cursor(r.cursor()+json.encode_utf16().count());Ok(json)
    }
}
fn dispatcher(names:&[String])->CommandDispatcher<Source>{
    let mut d=CommandDispatcher::new();d.register(ArgumentBuilder::literal("tellraw").requires(Rc::new(|s:&Source|s.permission>=2))
        .then(ArgumentBuilder::argument("targets",Rc::new(OnlinePlayerArgument{names:names.to_vec()}))
        .then(ArgumentBuilder::argument("message",Rc::new(JsonComponentArgument)).executes(Rc::new(|c:&CommandContext<Source>|{
            let Some(ArgumentValue::String(target))=c.argument("targets") else{unreachable!("target")};let Some(ArgumentValue::String(json))=c.argument("message") else{unreachable!("component")};
            *c.source().action.borrow_mut()=Some((target.clone(),json.clone()));Ok(1)
        })))));d
}
#[derive(Debug)]
pub enum TellrawError{Syntax(CommandSyntaxException),UnsupportedTarget,Component(crate::components::text::ComponentError),DynamicComponent}
pub struct Message{pub target:String,pub component:Component}
fn is_static(c:&Component)->bool{
    let contents=match &c.contents{Contents::Text(_)|Contents::Keybind(_)=>true,Contents::Translatable{args,..}=>args.iter().all(|a|match a{Arg::Component(c)=>is_static(c),Arg::Primitive(_)=>true}),_=>false};
    contents&&c.siblings.iter().all(is_static)&&match &c.style.hover_event{Some(HoverEvent::Text(c))=>is_static(c),Some(HoverEvent::PlayerEntity{name:Some(c),..})=>is_static(c),_=>true}
}
pub fn parse(input:&str,permission:u8)->Result<Message,TellrawError>{
    let s=source(permission);dispatcher(&[]).execute_str(input,&s).map_err(TellrawError::Syntax)?;let (target,json)=s.action.borrow_mut().take().expect("executed tellraw");
    if target.starts_with('@')||(target.len()==36&&[8,13,18,23].iter().all(|&i|target.as_bytes()[i]==b'-')){return Err(TellrawError::UnsupportedTarget);}
    let value=serde_json::from_str(&json).expect("parsed JSON");let component=Component::from_json(&value).map_err(TellrawError::Component)?;
    if !is_static(&component){return Err(TellrawError::DynamicComponent);}Ok(Message{target,component})
}
pub fn suggestions(input:&str,permission:u8,names:&[String])->Result<Suggestions,SuggestionError>{let d=dispatcher(names);d.completion_suggestions_at_end(&d.parse(input,&source(permission)))}
#[cfg(test)]
mod tests{
    use super::*;
    fn hex(b:&[u8])->String{b.iter().map(|b|format!("{b:02x}")).collect()}
    #[test]
    fn original_tellraw_trees_completions_static_json_packets_and_failures(){
        let v:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/tellraw-vectors.json")).unwrap();let names=["RawSource","RawTarget","RawOutside"].map(String::from);
        for row in v["trees"].as_array().unwrap(){let d=dispatcher(&names);let (tree,root)=super::super::tree_packet::usable_tree(d.tree(),d.root(),&source(row["permission"].as_u64().unwrap() as u8));assert_eq!(hex(&super::super::tree_packet::encode_commands_packet(&tree,root).unwrap()),row["hex"].as_str().unwrap());}
        for row in v["suggestions"].as_array().unwrap(){let a=suggestions(row["input"].as_str().unwrap(),2,&names).unwrap();assert_eq!(serde_json::json!({"start":a.range().start(),"end":a.range().end(),"texts":a.list().iter().map(|s|s.text()).collect::<Vec<_>>()}),serde_json::json!({"start":row["start"],"end":row["end"],"texts":row["texts"].as_array().unwrap().iter().filter(|v|!v.as_str().unwrap().starts_with('@')).collect::<Vec<_>>()}));}
        for row in v["execution"].as_array().unwrap(){
            let line=row["input"].as_str().unwrap();let d=dispatcher(&names);let p=d.parse(line,&source(row["permission"].as_u64().unwrap() as u8));let context=p.context.build(line);
            if let Some(arg)=context.parsed_argument("message"){assert_eq!(arg.range.start(),row["start_utf16"].as_u64().unwrap() as usize);assert_eq!(arg.range.end(),row["end_utf16"].as_u64().unwrap() as usize);}
            let parsed=parse(row["input"].as_str().unwrap(),row["permission"].as_u64().unwrap() as u8);if row["ok"]==true{
            let m=parsed.unwrap();if row["visibility"]!=2{let mut bytes=m.component.to_network_nbt().unwrap();bytes.push(0);let expected=row["packets"].as_object().unwrap().values().find_map(|a|a.as_array().unwrap().first()).unwrap().as_str().unwrap();assert_eq!(hex(&bytes),expected,"{row}");}
        }else if row["error_key"]!="argument.entity.notfound.player"{let Err(TellrawError::Syntax(e))=parsed else{panic!("syntax expected {row}")};assert_eq!(e.raw_message().key,row["error_key"].as_str().unwrap(),"{row}");}}
        for input in ["tellraw @a \"x\"","tellraw 00000000-0000-0000-0000-000000000000 \"x\"","tellraw RawTarget {\"selector\":\"@a\"}","tellraw RawTarget {\"text\":\"x\",\"hoverEvent\":{\"action\":\"show_text\",\"contents\":{\"score\":{\"name\":\"x\",\"objective\":\"y\"}}}}"]{assert!(parse(input,2).is_err(),"{input}");}
    }
}
