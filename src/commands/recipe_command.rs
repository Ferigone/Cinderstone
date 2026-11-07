// SPDX-License-Identifier: GPL-2.0-only
//! RecipeCommand literal online-name/single-resource branch over real owners.
use super::{arguments::{ArgumentNetworkInfo,ArgumentType,ArgumentValue},dispatcher::{ArgumentBuilder,CommandContext,CommandDispatcher},
    exception::{built_in,CommandSyntaxException},msg_command::OnlinePlayerArgument,string_reader::StringReader,suggestion::{SuggestionError,Suggestions}};
use crate::{server_recipe_book::RecipeId,components::text::{Arg,Component,Contents,Primitive}};
use std::{cell::RefCell,rc::Rc};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Operation{Give,Take}
#[derive(Clone,Debug)]
pub struct Action{pub operation:Operation,pub target:String,pub recipe:String}
#[derive(Clone)]
struct Source{permission:u8,recipes:Vec<String>,action:Rc<RefCell<Option<Action>>>}
fn source(permission:u8,recipes:&[String])->Source{Source{permission,recipes:recipes.to_vec(),action:Rc::new(RefCell::new(None))}}
struct RecipeArgument;
impl ArgumentType for RecipeArgument{
    type Value=String;
    fn network_info(&self)->Option<ArgumentNetworkInfo>{Some(ArgumentNetworkInfo{registry_id:35,template:vec![]})}
    fn parse(&self,r:&mut StringReader)->Result<String,CommandSyntaxException>{
        let start=r.cursor();let raw=r.remaining().chars().take_while(|c|c.is_ascii_lowercase()||c.is_ascii_digit()||"_-./:".contains(*c)).collect::<String>();r.set_cursor(start+raw.len());
        match RecipeId::parse(&raw){Ok(id)=>Ok(id.as_str().to_owned()),Err(_)=>{r.set_cursor(start);Err(built_in::simple("argument.id.invalid",r.units(),start))}}
    }
}
fn record(c:&CommandContext<Source>,operation:Operation)->Result<i32,CommandSyntaxException>{
    let Some(ArgumentValue::String(target))=c.argument("targets") else{unreachable!("registered target")};
    let recipe=match c.argument("recipe"){Some(ArgumentValue::String(id))=>id.clone(),_=>"*".into()};
    *c.source().action.borrow_mut()=Some(Action{operation,target:target.clone(),recipe});Ok(1)
}
fn dispatcher(names:&[String])->CommandDispatcher<Source>{
    let mut d=CommandDispatcher::new();let mut root=ArgumentBuilder::literal("recipe").requires(Rc::new(|s:&Source|s.permission>=2));
    for (literal,operation) in [("give",Operation::Give),("take",Operation::Take)]{
        let id=ArgumentBuilder::argument("recipe",Rc::new(RecipeArgument)).suggests_named("minecraft:all_recipes",Rc::new(|c:&CommandContext<Source>,b|{
            let prefix=b.remaining_lower_case();for id in &c.source().recipes{if id.starts_with(&prefix)||(!prefix.contains(':')&&id.strip_prefix("minecraft:").is_some_and(|p|p.starts_with(&prefix))){b.suggest(id);}}Ok(b.build())
        })).executes(Rc::new(move |c|record(c,operation)));
        root=root.then(ArgumentBuilder::literal(literal).then(ArgumentBuilder::argument("targets",Rc::new(OnlinePlayerArgument{names:names.to_vec()}))
            .then(id).then(ArgumentBuilder::literal("*").executes(Rc::new(move |c|record(c,operation))))));
    }d.register(root);d
}
#[derive(Debug)]
pub enum RecipeError{Syntax(CommandSyntaxException),UnsupportedTarget,UnsupportedWildcard}
pub fn parse(input:&str,permission:u8)->Result<Action,RecipeError>{
    let s=source(permission,&[]);dispatcher(&[]).execute_str(input,&s).map_err(RecipeError::Syntax)?;let a=s.action.borrow_mut().take().expect("executed recipe");
    if a.target.starts_with('@')||(a.target.len()==36&&[8,13,18,23].iter().all(|&i|a.target.as_bytes()[i]==b'-')){return Err(RecipeError::UnsupportedTarget);}
    if a.recipe=="*"{return Err(RecipeError::UnsupportedWildcard);}Ok(a)
}
pub fn suggestions(input:&str,permission:u8,names:&[String],recipes:&[String])->Result<Suggestions,SuggestionError>{let d=dispatcher(names);d.completion_suggestions_at_end(&d.parse(input,&source(permission,recipes)))}
pub fn success(operation:Operation,target:Component)->Component{Component{contents:Contents::Translatable{key:match operation{Operation::Give=>"commands.recipe.give.success.single",Operation::Take=>"commands.recipe.take.success.single"}.into(),fallback:None,args:vec![Arg::Primitive(Primitive::Int(1)),Arg::Component(target)]},siblings:vec![],style:Default::default()}}
pub fn zero_result(operation:Operation)->Component{Component::from_json(&serde_json::json!({"translate":match operation{Operation::Give=>"commands.recipe.give.failed",Operation::Take=>"commands.recipe.take.failed"}})).expect("fixed feedback")}

pub fn unknown_recipe(id:&str)->Component{Component{contents:Contents::Translatable{key:"recipe.notFound".into(),fallback:None,args:vec![Arg::Primitive(Primitive::Str(id.into()))]},siblings:vec![],style:Default::default()}}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::server_recipe_book::{ServerRecipeBook,RecipeBookHost,RecipeBookPacket,RecipeDescriptor,VanillaRecipeCatalogue};
    struct Host{packets:Vec<String>}
    impl RecipeBookHost for Host{
        fn recipe_unlocked(&mut self,_:&mut ServerRecipeBook,_:&RecipeDescriptor)->anyhow::Result<()>{Ok(())}
        fn send_packet(&mut self,_:&mut ServerRecipeBook,p:RecipeBookPacket)->anyhow::Result<()>{self.packets.push(hex(&p.encode()?.payload));Ok(())}
    }
    fn hex(b:&[u8])->String{b.iter().map(|b|format!("{b:02x}")).collect()}
    fn state(b:&ServerRecipeBook,id:&RecipeId)->serde_json::Value{serde_json::json!({"known":b.contains(id),"highlight":b.will_highlight(id)})}
    #[test]
    fn original_recipe_tree_suggestions_book_packets_result_and_typed_feedback(){
        let v:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/recipe-command-vectors.json")).unwrap();let names=["RecipeSource","RecipeTarget"].map(String::from);let ids=["minecraft:beacon","minecraft:armor_dye"].map(String::from);
        for row in v["trees"].as_array().unwrap(){let d=dispatcher(&names);let s=source(row["permission"].as_u64().unwrap() as u8,&ids);let (tree,root)=super::super::tree_packet::usable_tree(d.tree(),d.root(),&s);assert_eq!(hex(&super::super::tree_packet::encode_commands_packet(&tree,root).unwrap()),row["hex"].as_str().unwrap());}
        for row in v["suggestions"].as_array().unwrap(){let a=suggestions(row["input"].as_str().unwrap(),2,&names,&ids).unwrap();assert_eq!(serde_json::json!({"start":a.range().start(),"end":a.range().end(),"texts":a.list().iter().map(|v|v.text()).collect::<Vec<_>>()}),serde_json::json!({"start":row["start"],"end":row["end"],"texts":row["texts"]}));}
        let beacon=VanillaRecipeCatalogue::get().descriptor(&RecipeId::parse("minecraft:beacon").unwrap()).unwrap();
        for row in v["execution"].as_array().unwrap(){
            let mut book=ServerRecipeBook::default();if row["initial_known"]==true{book.add_known(&beacon);book.add_highlight(beacon.id.clone());}assert_eq!(state(&book,&beacon.id),row["before"]);let mut host=Host{packets:vec![]};
            let parsed=parse(row["input"].as_str().unwrap(),row["permission"].as_u64().unwrap() as u8);let mut component=None;
            match parsed{
                Err(RecipeError::Syntax(e))=>assert_eq!(e.raw_message().key,row["error_key"].as_str().unwrap()),
                Err(e)=>panic!("unexpected {e:?}"),
                Ok(a)=>{
                    if a.target=="Missing"{component=Some(Component::from_json(&serde_json::json!({"translate":"argument.entity.notfound.player"})).unwrap());}
                    else if let Some(recipe)=VanillaRecipeCatalogue::get().descriptor(&RecipeId::parse(&a.recipe).unwrap()){
                        let count=if a.operation==Operation::Give{book.add_recipes(&[recipe],&mut host).unwrap()}else{book.remove_recipes(&[recipe],&mut host).unwrap()};
                        if count==0{component=Some(zero_result(a.operation));}else{
                            assert_eq!(count,row["result"].as_i64().unwrap() as i32);let mut board=crate::scoreboard::Scoreboard::new_server();if row["team"]==true{for line in ["team add recipe","team modify recipe color gold","team modify recipe prefix {\"text\":\"[R] \",\"font\":\"minecraft:uniform\"}","team modify recipe suffix {\"keybind\":\"key.jump\"}","team join recipe RecipeTarget"]{crate::scoreboard::commands::execute(&mut board,line,None,4,&["RecipeTarget".into()]).unwrap();}}
                            let p=crate::entity::PlayerProfile::offline("RecipeTarget").unwrap();let target=crate::scoreboard::names::player_holder(&board,&p.name,p.uuid).display_name.unwrap();component=Some(success(a.operation,target));
                        }
                    }else{component=Some(unknown_recipe(&a.recipe));}
                }
            }
            if let Some(c)=component{let c=if row["ok"]==true{c}else{Component{contents:Contents::Text(String::new()),siblings:vec![c],style:crate::components::text::Style{color:Some(crate::components::text::TextColor::parse("red").unwrap()),..Default::default()}}};let mut bytes=c.to_network_nbt().unwrap();bytes.push(0);assert_eq!(hex(&bytes),row["feedback"][0].as_str().unwrap(),"{row}");}
            assert_eq!(state(&book,&beacon.id),row["after"]);assert_eq!(serde_json::json!(host.packets),row["recipe_packets"],"{row}");
        }
        for input in ["recipe give @a beacon","recipe give RecipeTarget *","recipe take 00000000-0000-0000-0000-000000000000 beacon"]{assert!(parse(input,2).is_err());}
    }
}
