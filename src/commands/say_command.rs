// SPDX-License-Identifier: GPL-2.0-only
//! Literal SayCommand grammar; selector expansion is explicitly unavailable.
use super::{arguments::{ArgumentNetworkInfo,ArgumentType,ArgumentValue},dispatcher::{ArgumentBuilder,CommandDispatcher},
    exception::{CommandArg,CommandMessage,CommandSyntaxException},string_reader::StringReader};
use std::{cell::RefCell,rc::Rc};
#[derive(Clone)]
struct Source{permission:u8,message:Rc<RefCell<Option<String>>>}
struct MessageArgument;
impl ArgumentType for MessageArgument {
    type Value=String;
    fn network_info(&self)->Option<ArgumentNetworkInfo>{Some(ArgumentNetworkInfo{registry_id:19,template:vec![]})}
    fn parse(&self,r:&mut StringReader)->Result<String,CommandSyntaxException>{
        let text=r.remaining();let length=text.encode_utf16().count();
        if length>256 {return Err(CommandSyntaxException::without_context(CommandMessage::new("argument.message.too_long",vec![CommandArg::Integer(length as i32),CommandArg::Integer(256)])));}
        r.set_cursor(r.total_length());Ok(text)
    }
}
fn source(permission:u8)->Source{Source{permission,message:Rc::new(RefCell::new(None))}}
fn dispatcher()->CommandDispatcher<Source>{
    let mut d=CommandDispatcher::new();d.register(ArgumentBuilder::literal("say").requires(Rc::new(|s:&Source|s.permission>=2))
        .then(ArgumentBuilder::argument("message",Rc::new(MessageArgument)).executes(Rc::new(|c|{
            let Some(ArgumentValue::String(text))=c.argument("message") else{unreachable!("registered message")};
            *c.source().message.borrow_mut()=Some(text.clone());Ok(1)
        }))));d
}
#[derive(Debug)]
pub enum SayError{Syntax(CommandSyntaxException),UnsupportedSelector}
pub fn is_command(line:&str)->bool{line.split(' ').next()==Some("say")}
/// Parsing supplies the same unnormalized substring used for signing. No
/// selector-like input may silently become a literal signing exemption.
pub fn execute(line:&str,permission:u8)->Result<String,SayError>{
    let s=source(permission);dispatcher().execute_str(line,&s).map_err(SayError::Syntax)?;
    let text=s.message.borrow_mut().take().expect("executed say");
    if text.as_bytes().windows(2).any(|p|p[0]==b'@'&&b"paresn".contains(&p[1])){return Err(SayError::UnsupportedSelector);}
    Ok(text)
}
/// Conservative predicate for the secure-profile unsigned-command gate. Even
/// unsupported selectors must require the signed envelope at usable permission.
pub fn has_message(line:&str,permission:i32)->bool{permission>=2&&is_command(line)&&line.split_once(' ').is_some_and(|(_,v)|!v.is_empty())}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn original_say_tree_literal_ranges_result_and_bound_packets(){
        let v:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/say-vectors.json")).unwrap();
        let hex=|b:&[u8]|b.iter().map(|b|format!("{b:02x}")).collect::<String>();
        for row in v["trees"].as_array().unwrap(){let d=dispatcher();let (tree,root)=super::super::tree_packet::usable_tree(d.tree(),d.root(),&source(row["permission"].as_u64().unwrap() as u8));assert_eq!(hex(&super::super::tree_packet::encode_commands_packet(&tree,root).unwrap()),row["hex"].as_str().unwrap());}
        for row in v["execution"].as_array().unwrap(){
            let line=row["input"].as_str().unwrap();let text=execute(line,row["permission"].as_u64().unwrap() as u8);
            assert_eq!(text.is_ok(),row["ok"].as_bool().unwrap(),"{row}");
            if let Ok(text)=text{
                assert_eq!(text,row["message"].as_str().unwrap());assert_eq!(line.encode_utf16().count()-text.encode_utf16().count(),row["start_utf16"].as_u64().unwrap() as usize);assert_eq!(line.encode_utf16().count(),row["end_utf16"].as_u64().unwrap() as usize);assert_eq!(row["result"],1);assert_eq!(row["type_key"],"minecraft:say_command");
                let mut board=crate::scoreboard::Scoreboard::new_server();
                if row["team"]==true{for command in ["team add sayteam","team modify sayteam color blue","team modify sayteam prefix {\"text\":\"[S] \",\"font\":\"minecraft:uniform\"}","team modify sayteam suffix {\"keybind\":\"key.jump\"}","team join sayteam SaySource"]{crate::scoreboard::commands::execute(&mut board,command,None,4,&["SaySource".into()]).unwrap();}}
                let name=if row["console"]==true{serde_json::json!({"text":"Server"})}else{let profile=crate::entity::PlayerProfile::offline("SaySource").unwrap();crate::scoreboard::names::player_holder(&board,"SaySource",profile.uuid).display_name.unwrap().to_json()};
                let bound=crate::chat_wire::BoundChatType{registry_id:1,name,target_name:None};let p=crate::chat_wire::encode_disguised_chat(&serde_json::json!({"text":text}),&bound).unwrap();assert_eq!(hex(&p.payload),row["disguised_hex"].as_str().unwrap());
            }
        }
        assert!(matches!(execute("say @a",2),Err(SayError::UnsupportedSelector)));
    }
}
