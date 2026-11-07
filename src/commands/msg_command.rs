// SPDX-License-Identifier: GPL-2.0-only
//! Public literal online-name whispers; selectors/UUID resolution is unavailable.
use super::{arguments::{ArgumentNetworkInfo,ArgumentType,ArgumentValue},dispatcher::{ArgumentBuilder,CommandContext,CommandDispatcher},
    exception::{built_in,CommandSyntaxException},say_command::MessageArgument,string_reader::StringReader,suggestion::{SuggestionError,Suggestions,SuggestionsBuilder}};
use std::{cell::RefCell,rc::Rc};
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Message{pub target:String,pub text:String}
#[derive(Clone)]
struct Source{message:Rc<RefCell<Option<Message>>>}
struct OnlinePlayerArgument{names:Vec<String>}
impl ArgumentType for OnlinePlayerArgument{
    type Value=String;
    fn network_info(&self)->Option<ArgumentNetworkInfo>{Some(ArgumentNetworkInfo{registry_id:6,template:vec![2]})}
    fn parse(&self,r:&mut StringReader)->Result<String,CommandSyntaxException>{
        if r.remaining().starts_with('@'){let name=r.remaining().split(' ').next().unwrap_or("").to_owned();r.set_cursor(r.cursor()+name.encode_utf16().count());return Ok(name);}
        let name=r.read_string()?;if name.is_empty(){return Err(built_in::simple("argument.entity.invalid",r.units(),r.cursor()));}Ok(name)}
    fn list_suggestions<S>(&self,_:&CommandContext<S>,b:&mut SuggestionsBuilder)->Result<Suggestions,CommandSyntaxException>{
        let remaining=b.remaining_lower_case();for name in &self.names{let lower=super::java_text::java_to_lower_case_root(name);if lower.starts_with(&remaining)||lower.char_indices().any(|(i,ch)|matches!(ch,'_'|'.')&&lower[i+1..].starts_with(&remaining)){b.suggest(name);}}Ok(b.build())
    }
}
fn source()->Source{Source{message:Rc::new(RefCell::new(None))}}
fn dispatcher(names:&[String])->CommandDispatcher<Source>{
    let mut d=CommandDispatcher::new();let root=d.register(ArgumentBuilder::literal("msg")
        .then(ArgumentBuilder::argument("targets",Rc::new(OnlinePlayerArgument{names:names.to_vec()}))
        .then(ArgumentBuilder::argument("message",Rc::new(MessageArgument)).executes(Rc::new(|c:&CommandContext<Source>|{
            let Some(ArgumentValue::String(target))=c.argument("targets") else{unreachable!("registered target")};
            let Some(ArgumentValue::String(text))=c.argument("message") else{unreachable!("registered message")};
            *c.source().message.borrow_mut()=Some(Message{target:target.clone(),text:text.clone()});Ok(1)
        })))));
    for alias in ["tell","w"]{d.register(ArgumentBuilder::literal(alias).redirect(root));}d
}
#[derive(Debug)]
pub enum MessageError{Syntax(CommandSyntaxException),UnsupportedTarget,UnsupportedMessage}
pub fn is_command(line:&str)->bool{matches!(line.split(' ').next(),Some("msg"|"tell"|"w"))}
pub fn parse(line:&str)->Result<Message,MessageError>{
    let s=source();dispatcher(&[]).execute_str(line,&s).map_err(MessageError::Syntax)?;let m=s.message.borrow_mut().take().expect("executed whisper");
    if m.target.starts_with('@')||(m.target.len()==36&&[8,13,18,23].iter().all(|&i|m.target.as_bytes()[i]==b'-')){return Err(MessageError::UnsupportedTarget);}
    if m.text.as_bytes().windows(2).any(|p|p[0]==b'@'&&b"paresn".contains(&p[1])){return Err(MessageError::UnsupportedMessage);}
    Ok(m)
}
pub fn has_message(line:&str)->bool{is_command(line)&&line.splitn(3,' ').nth(2).is_some_and(|v|!v.is_empty())}
pub fn suggestions(line:&str,names:&[String])->Result<Suggestions,SuggestionError>{let d=dispatcher(names);d.completion_suggestions_at_end(&d.parse(line,&source()))}
pub fn console_outgoing(target:crate::components::text::Component,text:&str)->crate::components::text::Component{
    use crate::components::text::{Arg,Component,Contents,Style,TextColor};
    Component{contents:Contents::Translatable{key:"commands.message.display.outgoing".into(),fallback:None,args:vec![Arg::Component(target),Arg::Component(Component::literal(text))]},siblings:vec![],style:Style{color:Some(TextColor::parse("gray").expect("fixed color")),italic:Some(true),..Style::default()}}
}
#[cfg(test)]
mod tests{
    use super::*;
    fn fixture()->serde_json::Value{serde_json::from_str(include_str!("../../fixtures/msg-vectors.json")).unwrap()}
    fn hex(b:&[u8])->String{b.iter().map(|b|format!("{b:02x}")).collect()}
    #[test]
    fn original_whisper_tree_aliases_completions_ranges_and_console_bytes(){
        let v=fixture();let names=["MsgSource","MsgTarget","MsgOutside"].map(String::from);let d=dispatcher(&names);let s=source();let (tree,root)=super::super::tree_packet::usable_tree(d.tree(),d.root(),&s);
        assert_eq!(hex(&super::super::tree_packet::encode_commands_packet(&tree,root).unwrap()),v["tree_hex"].as_str().unwrap());
        for row in v["suggestions"].as_array().unwrap(){let a=suggestions(row["input"].as_str().unwrap(),&names).unwrap();assert_eq!(serde_json::json!({"start":a.range().start(),"end":a.range().end(),"texts":a.list().iter().map(|v|v.text()).collect::<Vec<_>>()}),serde_json::json!({"start":row["start"],"end":row["end"],"texts":row["texts"]}),"{row}");}
        for row in v["execution"].as_array().unwrap(){let line=row["input"].as_str().unwrap();let parsed=parse(line);if let Some(text)=row["text"].as_str(){let m=parsed.unwrap();assert_eq!(m.text,text);assert_eq!(line.encode_utf16().count()-text.encode_utf16().count(),row["start_utf16"].as_u64().unwrap() as usize);assert_eq!(line.encode_utf16().count(),row["end_utf16"].as_u64().unwrap() as usize);
            if row["console"]==true{let board=crate::scoreboard::Scoreboard::new_server();let profile=crate::entity::PlayerProfile::offline("MsgTarget").unwrap();let target=crate::scoreboard::names::player_holder(&board,"MsgTarget",profile.uuid).display_name.unwrap();let mut bytes=console_outgoing(target,text).to_network_nbt().unwrap();bytes.push(0);assert_eq!(hex(&bytes),row["feedback"][0].as_str().unwrap());}
        }else{let Err(MessageError::Syntax(error))=parsed else{panic!("syntax expected {row}")};assert_eq!(error.raw_message().key,row["error_key"].as_str().unwrap());}}
        for input in ["msg @a hi","tell 00000000-0000-0000-0000-000000000000 hi","w MsgTarget @a"]{assert!(parse(input).is_err());assert!(has_message(input));}
    }
}
