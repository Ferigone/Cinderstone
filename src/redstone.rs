//! Native1.21 signal queries and source-derived block callbacks.
//! Unsupported declaring classes fail explicitly, including comparator BE
//! outputs and live wire topology until their actual owners are connected.
use crate::{geometry::Direction,inventory::{block_state_property,block_state_with_properties},placement::{self,PlacementWorld},runtime_world::RuntimeWorld,tick::TickPriority};
use anyhow::{anyhow,bail,ensure,Context,Result};use serde::Deserialize;use std::{collections::{BTreeMap,VecDeque},sync::OnceLock};
pub type Position=(i32,i32,i32);
pub const DIRECTIONS:[Direction;6]=[Direction::Down,Direction::Up,Direction::North,Direction::South,Direction::West,Direction::East];
pub const NEIGHBOR_ORDER:[Direction;6]=[Direction::West,Direction::East,Direction::Down,Direction::Up,Direction::North,Direction::South];
#[derive(Deserialize)]pub struct BlockFacts{pub name:String,pub min:u32,pub max:u32,pub class:String,pub signal:String,pub direct:String,pub neighbor:String,pub on_place:String,pub on_remove:String,#[serde(default)]pub pressed_time:i32,#[serde(default)]pub arrows:bool,#[serde(default)]pub mobs_only:bool,#[serde(default)]pub max_weight:i32,#[serde(default)]pub on_sound:i32,#[serde(default)]pub off_sound:i32}
#[derive(Deserialize)]struct Facts{blocks:Vec<BlockFacts>,lever_sound:i32}
fn facts()->&'static Facts{static D:OnceLock<Facts>=OnceLock::new();D.get_or_init(||serde_json::from_str(include_str!("../fixtures/redstone-facts.json")).expect("original redstone declarations"))}
pub fn block(state:u32)->Result<&'static BlockFacts>{let b=facts().blocks.partition_point(|b|b.min<=state).checked_sub(1).and_then(|i|facts().blocks.get(i)).context("unknown redstone state")?;ensure!(state<=b.max,"unknown redstone state");Ok(b)}
pub fn step(p:Position,d:Direction)->Position{let(x,y,z)=d.step();(p.0.wrapping_add(x),p.1.wrapping_add(y),p.2.wrapping_add(z))}
fn prop(s:u32,k:&str)->Result<String>{block_state_property(s,k).with_context(||format!("missing {k} at {s}"))}
fn yes(s:u32,k:&str)->Result<bool>{Ok(prop(s,k)?=="true")}
fn set(s:u32,k:&str,v:impl ToString)->Result<u32>{block_state_with_properties(s,&BTreeMap::from([(k.into(),v.to_string())]))}
fn direction(s:u32)->Result<Direction>{match prop(s,"facing")?.as_str(){"north"=>Ok(Direction::North),"south"=>Ok(Direction::South),"west"=>Ok(Direction::West),"east"=>Ok(Direction::East),"up"=>Ok(Direction::Up),"down"=>Ok(Direction::Down),_=>bail!("invalid redstone direction")}}
fn connected(s:u32)->Result<Direction>{Ok(match prop(s,"face")?.as_str(){"floor"=>Direction::Up,"ceiling"=>Direction::Down,"wall"=>direction(s)?,_=>bail!("invalid AttachFace")})}
pub trait SignalWorld{fn block(&mut self,p:Position)->Result<u32>;}
impl SignalWorld for RuntimeWorld{fn block(&mut self,p:Position)->Result<u32>{self.block(p.0,p.1,p.2)}}
#[derive(Deserialize)]struct SignalFacts{signal_facts:Vec<u8>}
fn conductor(s:u32)->Result<bool>{let b=block(s)?;ensure!(!matches!(b.class.as_str(),"dmy"|"dsu"),"dynamic redstone conductor owner required");static D:OnceLock<SignalFacts>=OnceLock::new();Ok(D.get_or_init(||serde_json::from_str(include_str!("../fixtures/placement-1.21.json")).expect("pinned signal flags")).signal_facts.get(s as usize).context("unknown signal flag")?&1!=0)}
fn plate_power(s:u32)->Result<i32>{match block_state_property(s,"power"){Some(v)=>Ok(v.parse()?),None=>Ok(if yes(s,"powered")?{15}else{0})}}
pub fn weak_signal(world:&mut impl SignalWorld,s:u32,p:Position,d:Direction)->Result<i32>{let b=block(s)?;match b.signal.as_str(){
"dtb"=>Ok(0),"dlu"=>Ok(15),"dkl"|"dgi"=>Ok(if yes(s,"powered")?{15}else{0}),"dfm"=>plate_power(s),"dhw"=>Ok(prop(s,"power")?.parse()?),
"dmd"=>Ok(if yes(s,"lit")?&&d!=Direction::Up{15}else{0}),"dme"=>Ok(if yes(s,"lit")?&&d!=direction(s)?{15}else{0}),
"dlj"=>Ok(if yes(s,"powered")?&&d==direction(s)?{15}else{0}),
"dia"=>{if !yes(s,"powered")?||d!=direction(s)?{return Ok(0)}ensure!(b.name=="minecraft:repeater","comparator output requires actual BE owner");Ok(15)},
"dmb"=>{let _=(world,p);bail!("wire signal requires current connection-shape owner")},
other=>bail!("unported getSignal class {other} for {}",b.name)}}
pub fn direct_signal(world:&mut impl SignalWorld,s:u32,p:Position,d:Direction)->Result<i32>{let b=block(s)?;match b.direct.as_str(){
"dtb"=>Ok(0),"dkl"|"dgi"=>Ok(if yes(s,"powered")?&&d==connected(s)?{15}else{0}),"dfm"=>Ok(if d==Direction::Up{plate_power(s)?}else{0}),
"dmd"=>if d==Direction::Down{weak_signal(world,s,p,d)}else{Ok(0)},"dia"|"dlj"=>weak_signal(world,s,p,d),
"dmb"=>bail!("wire direct signal requires current connection owner"),other=>bail!("unported getDirectSignal class {other}")}}
pub fn signal(world:&mut impl SignalWorld,p:Position,d:Direction)->Result<i32>{let s=world.block(p)?;let mut n=weak_signal(world,s,p,d)?;if conductor(s)?{for d in DIRECTIONS{let q=step(p,d);let s=world.block(q)?;n=n.max(direct_signal(world,s,q,d)?);if n>=15{break;}}}Ok(n)}
pub fn has_neighbor_signal(world:&mut impl SignalWorld,p:Position)->Result<bool>{for d in DIRECTIONS{if signal(world,step(p,d),d)?>0{return Ok(true)}}Ok(false)}
pub fn runtime_has_neighbor_signal(world:&mut RuntimeWorld,p:Position)->Result<bool>{has_neighbor_signal(world,p)}
pub struct PlacementSignals<'a,W>{pub world:&'a mut W}
impl<W:PlacementWorld> SignalWorld for PlacementSignals<'_,W>{fn block(&mut self,p:Position)->Result<u32>{self.world.block(p)}}
pub trait RedstoneWorld:SignalWorld{
 fn set_block(&mut self,p:Position,s:u32,flags:u32)->Result<()>;
 fn schedule(&mut self,p:Position,s:u32,delay:i32,priority:TickPriority)->Result<()>;
 fn has_scheduled(&self,p:Position,s:u32)->Result<bool>;
 fn will_tick(&mut self,p:Position,s:u32)->Result<bool>;
 fn update_neighbors(&mut self,p:Position,source:u32,except:Option<Direction>)->Result<()>;
 fn sound(&mut self,p:Position,id:i32,volume:f32,pitch:f32)->Result<()>;
 fn game_event(&mut self,p:Position,name:&'static str)->Result<()>;
 fn level_event(&mut self,p:Position,event:i32,data:i32)->Result<()>;
 fn entities_on_plate(&mut self,p:Position,mobs_only:bool)->Result<usize>;
 fn arrow_on_button(&mut self,p:Position,s:u32)->Result<bool>;
}
#[derive(Default)]pub struct RedstoneRuntime{burnout:VecDeque<(Position,i64)>}
impl RedstoneRuntime{pub fn new()->Self{Self::default()}fn recent(&mut self,p:Position,now:i64,add:bool)->bool{while self.burnout.front().is_some_and(|(_,t)|now.wrapping_sub(*t)>60){self.burnout.pop_front();}if add{self.burnout.push_back((p,now));}self.burnout.iter().filter(|(at,_)|*at==p).count()>=8}}
fn diode_input(w:&mut impl SignalWorld,p:Position,s:u32)->Result<bool>{let d=direction(s)?;let q=step(p,d);let n=signal(w,q,d)?;if n>=15{return Ok(true)}let t=w.block(q)?;Ok(n.max(if block(t)?.name=="minecraft:redstone_wire"{prop(t,"power")?.parse()?}else{0})>0)}
fn diode_locked(w:&mut impl SignalWorld,p:Position,s:u32)->Result<bool>{let f=direction(s)?;for d in [f.clockwise(),f.counter_clockwise()]{let q=step(p,d);let state=w.block(q)?;let b=block(state)?;if b.signal=="dia"&&direct_signal(w,state,q,d)?>0{return Ok(true)}}Ok(false)}
fn diode_priority(w:&mut impl SignalWorld,p:Position,s:u32)->Result<TickPriority>{let d=direction(s)?.opposite();let back=w.block(step(p,d))?;if block(back)?.signal=="dia"&&direction(back)?!=d{Ok(TickPriority::ExtremelyHigh)}else if yes(s,"powered")?{Ok(TickPriority::VeryHigh)}else{Ok(TickPriority::High)}}
fn diode_output(w:&mut impl RedstoneWorld,p:Position,s:u32)->Result<()>{let d=direction(s)?;let q=step(p,d.opposite());w.update_neighbors(q,s,Some(d))}
fn switch_neighbors(w:&mut impl RedstoneWorld,p:Position,s:u32)->Result<()>{w.update_neighbors(p,s,None)?;w.update_neighbors(step(p,connected(s)?.opposite()),s,None)}
pub fn owns_scheduled_callback(s:u32,delay:i32)->Result<bool>{let b=block(s)?;Ok(match b.class.as_str(){"dmc"=>delay==4,"dlj"=>delay==2,"dmg"=>matches!(delay,1|2|4|6|8),"dmd"|"dme"=>matches!(delay,2|160),"dgi"=>delay==b.pressed_time,"dlw"|"dpp"=>delay==b.pressed_time,_=>false})}
pub fn owns_neighbor_callback(s:u32)->Result<bool>{Ok(matches!(block(s)?.neighbor.as_str(),"dtb"|"dmc"|"dmd"|"dme")||block(s)?.name=="minecraft:repeater")}
pub fn neighbor_changed(w:&mut impl RedstoneWorld,p:Position,source:u32)->Result<bool>{let s=w.block(p)?;let b=block(s)?;match b.neighbor.as_str(){
"dtb"=>Ok(true),"dmc"=>{let on=yes(s,"lit")?;let input=has_neighbor_signal(w,p)?;if on!=input{if on{w.schedule(p,s,4,TickPriority::Normal)?}else{w.set_block(p,set(s,"lit",true)?,2)?;}}Ok(true)},
"dmd"|"dme"=>{let input=torch_input(w,p,s)?;if yes(s,"lit")?==input&&!w.will_tick(p,s)?{w.schedule(p,s,2,TickPriority::Normal)?;}Ok(true)},
"dia" if b.name=="minecraft:repeater"=>{if !diode_locked(w,p,s)?&&yes(s,"powered")?!=diode_input(w,p,s)?&&!w.will_tick(p,s)?{let priority=diode_priority(w,p,s)?;w.schedule(p,s,2*prop(s,"delay")?.parse::<i32>()?,priority)?;}Ok(true)},
_=>{let _=source;Ok(false)}}}
fn torch_input(w:&mut impl SignalWorld,p:Position,s:u32)->Result<bool>{let d=if block(s)?.class=="dme"{direction(s)?.opposite()}else{Direction::Down};Ok(signal(w,step(p,d),d)?>0)}
pub fn scheduled_tick(runtime:&mut RedstoneRuntime,w:&mut impl RedstoneWorld,p:Position,now:i64)->Result<()>{let s=w.block(p)?;let b=block(s)?;match b.class.as_str(){
"dmc"=>{if yes(s,"lit")?&&!has_neighbor_signal(w,p)?{w.set_block(p,set(s,"lit",false)?,2)?;}},
"dlj"=>{let on=yes(s,"powered")?;w.set_block(p,set(s,"powered",!on)?,2)?;if !on{w.schedule(p,s,2,TickPriority::Normal)?;}diode_output(w,p,s)?;},
"dmg"=>{if !diode_locked(w,p,s)?{let on=yes(s,"powered")?;let input=diode_input(w,p,s)?;if on&&!input{w.set_block(p,set(s,"powered",false)?,2)?;}else if !on{w.set_block(p,set(s,"powered",true)?,2)?;if !input{w.schedule(p,s,2*prop(s,"delay")?.parse::<i32>()?,TickPriority::VeryHigh)?;}}}},
"dmd"|"dme"=>{let input=torch_input(w,p,s)?;let _=runtime.recent(p,now,false);if yes(s,"lit")?&&input{w.set_block(p,set(s,"lit",false)?,3)?;if runtime.recent(p,now,true){w.level_event(p,1502,0)?;w.schedule(p,s,160,TickPriority::Normal)?;}}else if !yes(s,"lit")?&&!input&&!runtime.recent(p,now,false){w.set_block(p,set(s,"lit",true)?,3)?;}},
"dgi"=>{if yes(s,"powered")?{check_button(w,p,s)?;}},
"dlw"|"dpp"=>{if plate_power(s)?>0{check_plate(w,p,s)?;}},
_=>bail!("unported scheduled redstone block {}",b.name)}Ok(())}
fn check_button(w:&mut impl RedstoneWorld,p:Position,s:u32)->Result<()>{let b=block(s)?;let arrow=b.arrows&&w.arrow_on_button(p,s)?;let on=yes(s,"powered")?;if on!=arrow{let next=set(s,"powered",arrow)?;w.set_block(p,next,3)?;switch_neighbors(w,p,next)?;w.sound(p,if arrow{b.on_sound}else{b.off_sound},1.,1.)?;w.game_event(p,if arrow{"minecraft:block_activate"}else{"minecraft:block_deactivate"})?;}if arrow{w.schedule(p,s,b.pressed_time,TickPriority::Normal)?;}Ok(())}
fn check_plate(w:&mut impl RedstoneWorld,p:Position,s:u32)->Result<()>{let b=block(s)?;let old=plate_power(s)?;let n=w.entities_on_plate(p,b.mobs_only&&b.max_weight==0)?;let power=if b.max_weight>0{let n=n.min(b.max_weight as usize);if n==0{0}else{((n as f32/b.max_weight as f32)*15.).ceil() as i32}}else if n>0{15}else{0};if old!=power{let next=if b.max_weight>0{set(s,"power",power)?}else{set(s,"powered",power>0)?};w.set_block(p,next,2)?;w.update_neighbors(p,s,None)?;w.update_neighbors(step(p,Direction::Down),s,None)?;if old>0&&power==0{w.sound(p,b.off_sound,1.,1.)?;w.game_event(p,"minecraft:block_deactivate")?;}else if old==0&&power>0{w.sound(p,b.on_sound,1.,1.)?;w.game_event(p,"minecraft:block_activate")?;}}if power>0{w.schedule(p,s,b.pressed_time,TickPriority::Normal)?;}Ok(())}
pub fn entity_inside(w:&mut impl RedstoneWorld,p:Position)->Result<bool>{let s=w.block(p)?;let b=block(s)?;match b.class.as_str(){"dlw"|"dpp"=>{if plate_power(s)?==0{check_plate(w,p,s)?;}Ok(true)},"dgi"=>{if b.arrows&&!yes(s,"powered")?{check_button(w,p,s)?;}Ok(true)},_=>Ok(false)}}
pub fn use_block(w:&mut impl RedstoneWorld,p:Position,may_build:bool)->Result<bool>{let s=w.block(p)?;let b=block(s)?;match b.class.as_str(){
"dkl"=>{let on=!yes(s,"powered")?;let next=set(s,"powered",on)?;w.set_block(p,next,3)?;switch_neighbors(w,p,next)?;w.sound(p,facts().lever_sound,0.3,if on{0.6}else{0.5})?;w.game_event(p,if on{"minecraft:block_activate"}else{"minecraft:block_deactivate"})?;Ok(true)},
"dgi"=>{if !yes(s,"powered")?{let next=set(s,"powered",true)?;w.set_block(p,next,3)?;switch_neighbors(w,p,next)?;w.schedule(p,s,b.pressed_time,TickPriority::Normal)?;w.sound(p,b.on_sound,1.,1.)?;w.game_event(p,"minecraft:block_activate")?;}Ok(true)},
"dmg"=>{if may_build{w.set_block(p,set(s,"delay",prop(s,"delay")?.parse::<i32>()?%4+1)?,3)?;}Ok(true)},_=>Ok(false)}}
pub fn on_changed(w:&mut impl RedstoneWorld,p:Position,old:u32,new:u32)->Result<()>{on_changed_with_fluid_owner(w,p,old,new,false)}
/// The host invokes this only after its actual FluidSimulation callback has
/// executed LiquidBlock.onPlace. Old block removal still executes here.
pub fn on_changed_after_fluid(w:&mut impl RedstoneWorld,p:Position,old:u32,new:u32)->Result<()>{on_changed_with_fluid_owner(w,p,old,new,true)}
fn on_changed_with_fluid_owner(w:&mut impl RedstoneWorld,p:Position,old:u32,new:u32,fluid_done:bool)->Result<()>{let a=block(old)?;let b=block(new)?;if a.min!=b.min{match a.on_remove.as_str(){"dtb"|"dfy"=>{},"dkl"|"dgi"=>{if yes(old,"powered")?{switch_neighbors(w,p,old)?;}},"dfm"=>{if plate_power(old)?>0{w.update_neighbors(p,old,None)?;w.update_neighbors(step(p,Direction::Down),old,None)?;}},"dmd"|"dme"=>{for d in DIRECTIONS{w.update_neighbors(step(p,d),old,None)?;}},"dia" if a.name=="minecraft:repeater"=>diode_output(w,p,old)?,other=>bail!("unported onRemove {other} for {}",a.name)}}match b.on_place.as_str(){"dtb"|"dfy"=>{},"dko" if fluid_done=>{},"dia" if b.name=="minecraft:repeater"=>diode_output(w,p,new)?,"dmd"|"dme"=>{if a.min!=b.min{for d in DIRECTIONS{w.update_neighbors(step(p,d),new,None)?;}}},other=>bail!("unported onPlace {other} for {}",b.name)}Ok(())}
pub fn owns_placed_by(s:u32)->Result<bool>{Ok(block(s)?.name=="minecraft:repeater")}
pub fn placed_by(w:&mut impl RedstoneWorld,p:Position,s:u32)->Result<()>{ensure!(owns_placed_by(s)?,"unported setPlacedBy");if diode_input(w,p,s)?{w.schedule(p,s,1,TickPriority::Normal)?;}Ok(())}
pub fn static_conductor(state:u32)->Result<bool>{conductor(state)}
pub fn owns_change_callbacks(old:u32,new:u32)->Result<bool>{let a=block(old)?;let b=block(new)?;Ok((a.min==b.min||matches!(a.on_remove.as_str(),"dtb"|"dfy"|"dkl"|"dgi"|"dfm"|"dmd"|"dme")||a.name=="minecraft:repeater")&&(matches!(b.on_place.as_str(),"dtb"|"dfy"|"dmd"|"dme")||b.name=="minecraft:repeater"))}

/// Combined callback ownership when Main retains the real fluid-before-
/// redstone dispatcher. This claims only LiquidBlock's actual declaring class.
pub fn owns_change_callbacks_after_fluid(old:u32,new:u32)->Result<bool>{
 let a=block(old)?;let b=block(new)?;
 Ok((a.min==b.min||matches!(a.on_remove.as_str(),"dtb"|"dfy"|"dkl"|"dgi"|"dfm"|"dmd"|"dme")||a.name=="minecraft:repeater")
   &&(matches!(b.on_place.as_str(),"dtb"|"dfy"|"dmd"|"dme"|"dko")||b.name=="minecraft:repeater"))
}
