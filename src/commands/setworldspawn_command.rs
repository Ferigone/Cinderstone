// SPDX-License-Identifier: GPL-2.0-only
//! SetWorldSpawnCommand over the existing BlockPos/Angle arguments.
use super::{
    arguments::{ArgumentNetworkInfo, ArgumentType, ArgumentValue},
    coordinates::{BlockPosArgumentType, CoordinateSource, Coordinates},
    dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher},
    exception::{CommandMessage, CommandSyntaxException},
    simple_arguments::AngleArgumentType,
    string_reader::StringReader,
    suggestion::{SuggestionError, Suggestions, SuggestionsBuilder},
};
use crate::components::text::{Arg, Component, Contents, Primitive, Style};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorldSpawn { pub x: i32, pub y: i32, pub z: i32, pub angle: f32 }
impl Default for WorldSpawn {fn default()->Self {Self{x:8,y:64,z:8,angle:0.}}}
impl WorldSpawn {
    pub fn feedback(self)->Component {
        Component { contents: Contents::Translatable {key:"commands.setworldspawn.success".into(),fallback:None,
            args:vec![Arg::Primitive(Primitive::Int(self.x)),Arg::Primitive(Primitive::Int(self.y)),
                Arg::Primitive(Primitive::Int(self.z)),Arg::Primitive(Primitive::Float(self.angle))]},
            siblings:Vec::new(),style:Style::default() }
    }
}
#[derive(Clone)]
struct Source { permission:u8, at:CoordinateSource, result:Rc<Cell<Option<WorldSpawn>>> }
struct SpawnPosition;
impl ArgumentType for SpawnPosition {
    type Value=Coordinates;
    fn parse(&self,r:&mut StringReader)->Result<Coordinates,CommandSyntaxException>{BlockPosArgumentType.parse(r)}
    fn network_info(&self)->Option<ArgumentNetworkInfo>{BlockPosArgumentType.network_info()}
    fn list_suggestions<S>(&self,_:&CommandContext<S>,b:&mut SuggestionsBuilder)->Result<Suggestions,CommandSyntaxException>{
        let remaining=b.remaining();let local=remaining.starts_with('^');let unit=if local{"^"}else{"~"};
        let parts:Vec<_>=remaining.trim_end_matches(' ').split(' ').collect();
        if parts.len()>3{return Ok(Suggestions::empty());}
        let mut full=parts.iter().map(|s|s.to_string()).collect::<Vec<_>>();
        if remaining.is_empty(){full=vec![unit.into()];}
        while full.len()<3 {full.push(unit.into());}
        if self.parse(&mut StringReader::new(&full.join(" "))).is_ok(){
            for end in parts.len()..=3 {let candidate=full[..end].join(" ");if candidate.starts_with(&remaining){b.suggest(&candidate);}}
        }
        Ok(b.build())
    }
}
fn record(c:&CommandContext<Source>,explicit:bool,angle:bool)->Result<i32,CommandSyntaxException>{
    let coordinates=if explicit {match c.argument("pos"){Some(ArgumentValue::Coordinates(v))=>*v,_=>unreachable!()}}
        else {BlockPosArgumentType.parse(&mut StringReader::new("~ ~ ~")).expect("fixed position")};
    let (x,y,z)=coordinates.block_pos(&c.source().at);
    if explicit && (!(-30_000_000..30_000_000).contains(&x)||!(-30_000_000..30_000_000).contains(&z)||!(-20_000_000..20_000_000).contains(&y)){
        return Err(CommandSyntaxException::without_context(CommandMessage::new("argument.pos.outofbounds",vec![])));
    }
    let angle=if angle {match c.argument("angle"){Some(ArgumentValue::Angle(v))=>v.angle(&c.source().at),_=>unreachable!()}}else{0.};
    c.source().result.set(Some(WorldSpawn{x,y,z,angle}));Ok(1)
}
fn dispatcher()->CommandDispatcher<Source>{
    let mut d=CommandDispatcher::new();d.register(ArgumentBuilder::literal("setworldspawn")
        .requires(Rc::new(|s:&Source|s.permission>=2)).executes(Rc::new(|c|record(c,false,false)))
        .then(ArgumentBuilder::argument("pos",Rc::new(SpawnPosition)).executes(Rc::new(|c|record(c,true,false)))
            .then(ArgumentBuilder::argument("angle",Rc::new(AngleArgumentType)).executes(Rc::new(|c|record(c,true,true))))));d
}
pub fn execute(input:&str,permission:u8,at:CoordinateSource)->Result<WorldSpawn,CommandSyntaxException>{
    let s=Source{permission,at,result:Rc::new(Cell::new(None))};dispatcher().execute_str(input,&s)?;Ok(s.result.get().expect("executed spawn command"))
}
pub fn suggestions(input:&str,permission:u8)->Result<Suggestions,SuggestionError>{
    if permission<2{return Ok(Suggestions::empty());}
    let zero=crate::geometry::Vec3::new(0.,0.,0.);let s=Source{permission,at:CoordinateSource{position:zero,anchored_position:zero,rotation:(0.,0.)},result:Rc::new(Cell::new(None))};
    let d=dispatcher();d.completion_suggestions_at_end(&d.parse(input,&s))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(bytes:&[u8])->String{bytes.iter().map(|b|format!("{b:02x}")).collect()}
    #[test]
    fn executed_original_grammar_coordinates_results_and_typed_feedback(){
        let v:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/setworldspawn-vectors.json")).unwrap();
        let at=CoordinateSource{position:crate::geometry::Vec3::new(12.5,65.,18.5),anchored_position:crate::geometry::Vec3::new(12.5,65.,18.5),rotation:(0.,-180.)};
        for row in v["grammar"].as_array().unwrap(){
            let permission=row["permission"].as_u64().unwrap() as u8;
            let d=dispatcher();let source=Source{permission,at,result:Rc::new(Cell::new(None))};
            let (tree,root)=super::super::tree_packet::usable_tree(d.tree(),d.root(),&source);
            assert_eq!(hex(&super::super::tree_packet::encode_commands_packet(&tree,root).unwrap()),row["tree_hex"].as_str().unwrap());
            for c in row["completions"].as_array().unwrap(){
                let s=suggestions(c["input"].as_str().unwrap(),permission).unwrap();
                assert_eq!(serde_json::json!({"start":s.range().start(),"end":s.range().end(),"texts":s.list().iter().map(|s|s.text()).collect::<Vec<_>>()}),
                    serde_json::json!({"start":c["start"],"end":c["end"],"texts":c["texts"]}),"{c}");
            }
        }
        for row in v["execution"].as_array().unwrap(){
            let p=&row["source_pos"];let r=&row["rotation"];
            let position=crate::geometry::Vec3::new(p[0].as_f64().unwrap(),p[1].as_f64().unwrap(),p[2].as_f64().unwrap());
            let at=CoordinateSource{position,anchored_position:position,rotation:(r[0].as_f64().unwrap() as f32,r[1].as_f64().unwrap() as f32)};
            let actual=execute(row["input"].as_str().unwrap(),row["permission"].as_u64().unwrap() as u8,at);
            assert_eq!(actual.is_ok(),row["ok"].as_bool().unwrap(),"{row}");
            if let Ok(spawn)=actual{
                assert_eq!(serde_json::json!([spawn.x,spawn.y,spawn.z]),row["pos"]);
                assert_eq!(spawn.angle.to_bits(),row["angle_bits"].as_i64().unwrap() as u32);
                let mut bytes=spawn.feedback().to_network_nbt().unwrap();bytes.push(0);
                assert_eq!(hex(&bytes),row["feedback_hex"].as_str().unwrap());
                assert_eq!(row["result"],1);
            }else if let Err(error)=actual{
                if row["error_key"]=="argument.pos.outofbounds"{assert_eq!(error.raw_message().key,"argument.pos.outofbounds");}
            }
        }
    }
}
