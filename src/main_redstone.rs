//! Live native redstone callbacks, sharing actual world, entity and BLOCK owners.
use super::*;
use mc121_rust::{redstone::{self,RedstoneWorld,SignalWorld},block_ticks,tick::TickPriority,geometry::Direction};
type P=(i32,i32,i32);
pub(super) struct MainRedstoneRuntime{pub(super) engine:redstone::RedstoneRuntime,native:bool,limit:i32,stopped:Option<String>}
impl MainRedstoneRuntime{pub fn new(native:bool)->Self{Self{engine:redstone::RedstoneRuntime::new(),native,limit:1_000_000,stopped:None}}pub fn configure_neighbor_limit(&mut self,v:i32){self.limit=v;}pub fn require_healthy(&self)->Result<()>{ensure!(self.native,"native redstone source unavailable");ensure!(self.stopped.is_none(),"redstone stopped: {:?}",self.stopped);Ok(())}}
#[derive(Clone)]struct Body{bounds:Aabb,living:bool}
struct NeighborTask{p:P,source:u32,next:usize,except:Option<Direction>}
struct Host<'a>{world:&'a mut WorldBackend,fluid:main_fluid_simulation::MainFluidHost<'a>,queue:&'a mut block_ticks::BlockTickScheduler,order:&'a mut i64,sounds:&'a mut RuntimeEntities,now:i64,bodies:Vec<Body>,limit:i32,count:i32,working:bool,pending:Vec<NeighborTask>,exclude:Option<i32>,fluid_done:bool}
impl SignalWorld for Host<'_>{fn block(&mut self,p:P)->Result<u32>{self.world.block(p.0,p.1,p.2)}}
impl Host<'_>{fn notify(&mut self,p:P,source:u32,except:Option<Direction>)->Result<()>{let previous=self.count;self.count=self.count.saturating_add(1);if self.limit>=0&&previous>=self.limit{return Ok(())}self.pending.push(NeighborTask{p,source,next:0,except});if self.working{return Ok(())}self.working=true;let result=(||{while let Some(mut task)=self.pending.pop(){if task.next>=6{continue}let d=redstone::NEIGHBOR_ORDER[task.next];task.next+=1;let target=redstone::step(task.p,d);let source=task.source;let skip=task.except==Some(d);self.pending.push(task);if !skip{let state=self.world.block(target.0,target.1,target.2)?;if self.fluid_done&&redstone::block(state)?.neighbor=="dko"{continue;}ensure!(redstone::neighbor_changed(self,target,source)?,"unported neighborChanged at {target:?}");}}Ok(())})();self.working=false;self.count=0;result}}
impl RedstoneWorld for Host<'_>{
 fn set_block(&mut self,p:P,s:u32,flags:u32)->Result<()>{let old=self.world.block(p.0,p.1,p.2)?;let changes=self.fluid.set_block_with_primary_callback(self.world,p,s,flags,false)?;self.fluid.interactions.mark_redstone_primary(p,old,s);redstone::on_changed(self,p,old,s)?;mc121_rust::fluid_simulation::FluidSimulationHost::block_changes(&mut self.fluid,&changes)}
 fn schedule(&mut self,p:P,s:u32,delay:i32,priority:TickPriority)->Result<()>{block_ticks::schedule(self.queue,p,s,self.now,delay,priority,self.order)?;Ok(())}
 fn has_scheduled(&self,p:P,s:u32)->Result<bool>{Ok(self.queue.has_scheduled_tick(block_ticks::pos(p),&block_ticks::key(s)?))}
 fn will_tick(&mut self,p:P,s:u32)->Result<bool>{Ok(self.queue.will_tick_this_tick(block_ticks::pos(p),&block_ticks::key(s)?))}
 fn update_neighbors(&mut self,p:P,source:u32,except:Option<Direction>)->Result<()>{self.notify(p,source,except)}
 fn sound(&mut self,p:P,id:i32,volume:f32,pitch:f32)->Result<()>{let seed=self.sounds.next_sound_seed()?;let origin=Vec3::new(p.0 as f64+0.5,p.1 as f64+0.5,p.2 as f64+0.5);let mut w=Writer::new();w.varint(id+1).varint(4).i32((origin.x*8.)as i32).i32((origin.y*8.)as i32).i32((origin.z*8.)as i32).f32(volume).f32(pitch).i64(seed);let recipients=self.fluid.players.iter().filter(|v|Some(v.entity_id)!=self.exclude&&(v.position.x-origin.x).powi(2)+(v.position.y-origin.y).powi(2)+(v.position.z-origin.z).powi(2)<256.).map(|v|v.entity_id).collect();let _=self.fluid.events.send(PlayerEvent::WorldAudience{recipients:Arc::new(recipients),packets:Arc::new(vec![Packet{id:0x68,payload:w.into_inner()}])});Ok(())}
 fn game_event(&mut self,p:P,name:&'static str)->Result<()>{self.fluid.interactions.append(-1,[mc121_rust::world_interactions::InteractionEvent::GameEvent{position:p,name:name.into()}]);Ok(())}
 fn level_event(&mut self,p:P,event:i32,data:i32)->Result<()>{let packet=mc121_rust::item_effects::level_event_packet(event,p,data,false);let _=self.fluid.events.send(PlayerEvent::WorldAudience{recipients:Arc::new(self.fluid.players.iter().map(|v|v.entity_id).collect()),packets:Arc::new(vec![packet])});Ok(())}
 fn entities_on_plate(&mut self,p:P,mobs_only:bool)->Result<usize>{let bounds=Aabb::new(p.0 as f64+0.125,p.1 as f64,p.2 as f64+0.125,p.0 as f64+0.875,p.1 as f64+0.25,p.2 as f64+0.875);Ok(self.bodies.iter().filter(|b|(!mobs_only||b.living)&&bounds.intersects(&b.bounds)).count())}
 fn arrow_on_button(&mut self,_:P,_:u32)->Result<bool>{Ok(false)} // actual native source owns only players/items; no Arrow exists.
}
#[derive(Clone,Copy)]enum Action{Changed(P,u32,u32),Neighbors(P,u32),Tick(P),Use(P,bool,Option<i32>),Inside(P),Placed(P,u32)}
impl LivePlayers{
 fn redstone_frame(&mut self,world:&mut WorldBackend,now:i64,border:CollisionBorder,events:&broadcast::Sender<PlayerEvent>,action:Action,override_body:Option<&mc121_rust::entity_state::EntityPhysicalState>)->Result<bool>{
  if let Action::Changed(_,old,new)=action{let a=redstone::block(old)?;let b=redstone::block(new)?;if (a.min==b.min||matches!(a.on_remove.as_str(),"dtb"|"dfy"))&&matches!(b.on_place.as_str(),"dtb"|"dfy"|"dko"){return Ok(true)}}
  if let Action::Use(p,..)=action{if !matches!(redstone::block(world.block(p.0,p.1,p.2)?)?.class.as_str(),"dkl"|"dgi"|"dmg"){return Ok(false)}}
  if let Action::Inside(p)=action{if !matches!(redstone::block(world.block(p.0,p.1,p.2)?)?.class.as_str(),"dlw"|"dpp"|"dgi"){return Ok(false)}}
  self.redstone_runtime.require_healthy()?;self.gameplay.require_supported_floor()?;
  self.entity_source.as_ref().context("native redstone source owner is unavailable")?
      .require_owned_callback_world(world)
      .context("validating the actual native redstone callback world")?;
  let p=match action{Action::Changed(p,..)|Action::Neighbors(p,..)|Action::Tick(p)|Action::Use(p,..)|Action::Inside(p)|Action::Placed(p,..)=>p};
  self.block_runtime.prepare_halo(world,&self.chunk_runtime,(p.0.div_euclid(16),p.2.div_euclid(16)),now)?;
  let snapshots=self.registry.snapshot();let mut bodies=Vec::new();for v in &snapshots{if v.gamemode!=3{let d=player_physics::player_dimensions(v.metadata.pose,1.)?;bodies.push(Body{bounds:if let Some(body)=override_body.filter(|b|b.entity_id==v.entity_id){body.bounds}else{Aabb::player(Vec3::new(v.position.x,v.position.y,v.position.z),d.width,d.height)},living:true});}}
  bodies.extend(self.item_runtime.item_states().filter(|s|!s.removed).map(|s|Body{bounds:s.bounds,living:false}));
  let Self{block_runtime,redstone_runtime,fluid_runtime,chunk_runtime,item_runtime,gameplay,world_interactions,entity_source,world_revision,random_block_ticks,..}=self;
  block_runtime.mark_used();if let Some(source)=entity_source.as_mut(){source.observe_block_scheduler_use();}let exclude=if let Action::Use(_,_,id)=action{id}else{None};
  let fluid=main_fluid_simulation::MainFluidHost{chunks:chunk_runtime,gameplay,interactions:world_interactions,entity_source,revision:world_revision,events,players:snapshots,active_items:item_runtime.item_count(),border,random_ticks:Some(random_block_ticks)};
  let mut host=Host{world,fluid,queue:&mut block_runtime.queue,order:&mut fluid_runtime.sub_tick_order,sounds:item_runtime,now,bodies,limit:redstone_runtime.limit,count:0,working:false,pending:Vec::new(),exclude,fluid_done:matches!(action,Action::Neighbors(..))};
  let result=match action{Action::Changed(p,a,b)=>redstone::on_changed_after_fluid(&mut host,p,a,b).map(|_|true),Action::Neighbors(p,s)=>host.notify(p,s,None).map(|_|true),Action::Tick(p)=>redstone::scheduled_tick(&mut redstone_runtime.engine,&mut host,p,now).map(|_|true),Action::Use(p,may_build,_)=>redstone::use_block(&mut host,p,may_build),Action::Inside(p)=>redstone::entity_inside(&mut host,p),Action::Placed(p,s)=>redstone::placed_by(&mut host,p,s).map(|_|true)};
  if let Err(e)=&result{redstone_runtime.stopped=Some(format!("{e:#}"));}result
 }
 pub(super) fn observe_redstone_change(&mut self,w:&mut WorldBackend,p:P,old:u32,new:u32,now:i64,border:CollisionBorder,e:&broadcast::Sender<PlayerEvent>)->Result<()>{self.redstone_frame(w,now,border,e,Action::Changed(p,old,new),None).map(|_|())}
 pub(super) fn observe_redstone_neighbors(&mut self,w:&mut WorldBackend,p:P,source:u32,now:i64,border:CollisionBorder,e:&broadcast::Sender<PlayerEvent>)->Result<()>{self.redstone_frame(w,now,border,e,Action::Neighbors(p,source),None).map(|_|())}
 pub(super) fn tick_redstone_block(&mut self,w:&mut WorldBackend,p:P,now:i64,border:CollisionBorder,e:&broadcast::Sender<PlayerEvent>)->Result<()>{self.redstone_frame(w,now,border,e,Action::Tick(p),None).map(|_|())}
 pub(super) fn redstone_set_placed_by(&mut self,w:&mut WorldBackend,p:P,s:u32,now:i64,border:CollisionBorder,e:&broadcast::Sender<PlayerEvent>)->Result<()>{self.redstone_frame(w,now,border,e,Action::Placed(p,s),None).map(|_|())}
 pub(super) fn use_redstone_block(&mut self,w:&mut WorldBackend,p:P,id:i32,now:i64,border:CollisionBorder,e:&broadcast::Sender<PlayerEvent>)->Result<bool>{let may=self.registry.get(id).and_then(|v|self.player_lifecycle.get(&v.profile.uuid)).context("redstone actor lifecycle missing")?.abilities.may_build;self.redstone_frame(w,now,border,e,Action::Use(p,may,Some(id)),None)}
 pub(super) fn redstone_entity_inside_with_body(&mut self,w:&mut WorldBackend,p:P,body:&mc121_rust::entity_state::EntityPhysicalState,now:i64,border:CollisionBorder,e:&broadcast::Sender<PlayerEvent>)->Result<bool>{self.redstone_frame(w,now,border,e,Action::Inside(p),Some(body))}
}
