// SPDX-License-Identifier: GPL-2.0-only
//! PardonIpCommand grammar and suggestions over the caller's retained IP keys.
use super::{arguments::{ArgumentValue, StringArgumentType}, dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher},
    exception::CommandSyntaxException, suggestion::{SuggestionError, Suggestions}};
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
struct Source { permission: u8, ips: Vec<String>, target: Rc<RefCell<Option<String>>> }
fn source(permission:u8,ips:&[String])->Source {Source{permission,ips:ips.to_vec(),target:Rc::new(RefCell::new(None))}}
fn dispatcher()->CommandDispatcher<Source> {
    let mut d=CommandDispatcher::new();
    d.register(ArgumentBuilder::literal("pardon-ip").requires(Rc::new(|s:&Source|s.permission>=3))
        .then(ArgumentBuilder::argument("target",Rc::new(StringArgumentType::word()))
            .suggests(Rc::new(|context:&CommandContext<Source>,builder|{
                let remaining=builder.remaining_lower_case();
                for ip in &context.source().ips {
                    let candidate=super::java_text::java_to_lower_case_root(ip);
                    if candidate.starts_with(&remaining)||candidate.char_indices().any(|(i,c)|
                        (c=='.'||c=='_')&&candidate[i+c.len_utf8()..].starts_with(&remaining)) {builder.suggest(ip);}
                }
                Ok(builder.build())
            }))
            .executes(Rc::new(|context:&CommandContext<Source>|{
                let Some(ArgumentValue::String(target))=context.argument("target") else {unreachable!("registered string")};
                *context.source().target.borrow_mut()=Some(target.clone());Ok(1)
            }))));d
}
/// Authorize and parse before the host performs IP validation, removal or IO.
pub fn parse(input:&str,permission:u8)->Result<String,CommandSyntaxException> {
    let s=source(permission,&[]);dispatcher().execute_str(input,&s)?;
    let target=s.target.borrow_mut().take().expect("registered pardon-ip executed");Ok(target)
}
pub fn suggestions(input:&str,permission:u8,ips:&[String])->Result<Suggestions,SuggestionError> {
    if permission<3{return Ok(Suggestions::empty());}
    let d=dispatcher();d.completion_suggestions_at_end(&d.parse(input,&source(permission,ips)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_parse_tree_and_live_key_suggestions() {
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/pardon-ip-command-vectors.json")).unwrap();
        let ips=["192.0.2.7","198.51.100.9","2001:db8::1"].map(String::from);
        for row in fixture["trees"].as_array().unwrap(){
            let d=dispatcher();let s=source(row["permission"].as_u64().unwrap() as u8,&ips);
            let (tree,root)=super::super::tree_packet::usable_tree(d.tree(),d.root(),&s);
            let hex:String=super::super::tree_packet::encode_commands_packet(&tree,root).unwrap().iter().map(|b|format!("{b:02x}")).collect();
            assert_eq!(hex,row["packet_hex"].as_str().unwrap());
        }
        for row in fixture["execution"].as_array().unwrap(){
            let target=parse(row["input"].as_str().unwrap(),row["permission"].as_u64().unwrap() as u8);
            assert_eq!(target.is_ok(),row["parsed"].as_bool().unwrap(),"{row}");
            if let Ok(target)=target{assert_eq!(target,row["target"].as_str().unwrap());}
        }
        for row in fixture["suggestions"].as_array().unwrap(){
            let actual=suggestions(row["input"].as_str().unwrap(),3,&ips).unwrap();
            assert_eq!(actual.range().start(),row["start"].as_u64().unwrap() as usize);
            assert_eq!(actual.range().end(),row["end"].as_u64().unwrap() as usize);
            assert_eq!(actual.list().iter().map(|v|v.text()).collect::<Vec<_>>(),row["texts"].as_array().unwrap().iter().map(|v|v.as_str().unwrap()).collect::<Vec<_>>());
        }
        assert!(suggestions("pardon-ip ",2,&ips).unwrap().list().is_empty());
    }
}
