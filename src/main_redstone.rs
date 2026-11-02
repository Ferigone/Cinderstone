//! Live native redstone callbacks, sharing actual world, entity and BLOCK owners.
use super::*;
use mc121_rust::{redstone::{self,RedstoneWorld,SignalWorld},block_ticks,tick::TickPriority,geometry::Direction};
type P=(i32,i32,i32);
#[path = "main_redstone_actors.rs"]
mod actors;
pub(super) use actors::{NativeTriggerBody,capture_external_trigger_bodies};
use actors::NativePlateQuery;
use mc121_rust::runtime_entities::RuntimeEntityQuerySound;
use mc121_rust::{entity_state::EntityPhysicalState, runtime_entity_index::{EntityHandle,EntityKind},
    world_interactions::{self,RuntimeInteractionWorld}};
use super::{main_falling_block_spawn::ActualFallingBlockEditServices,
    main_falling_block_runtime::FallingBlockPublication};

pub(super) struct MainRedstoneRuntime{pub(super) engine:redstone::RedstoneRuntime,native:bool,limit:i32,stopped:Option<String>}
impl MainRedstoneRuntime{pub fn new(native:bool)->Self{Self{engine:redstone::RedstoneRuntime::new(),native,limit:1_000_000,stopped:None}}pub fn configure_neighbor_limit(&mut self,v:i32){self.limit=v;}pub fn require_healthy(&self)->Result<()>{ensure!(self.native,"native redstone source unavailable");ensure!(self.stopped.is_none(),"redstone stopped: {:?}",self.stopped);Ok(())}}
struct NeighborTask{p:P,source:u32,next:usize,except:Option<Direction>}
struct Host<'a>{world:&'a mut WorldBackend,fluid:main_fluid_simulation::MainFluidHost<'a>,queue:&'a mut block_ticks::BlockTickScheduler,order:&'a mut i64,sounds:&'a mut RuntimeEntities,now:i64,actors:NativePlateQuery<'a>,work:NeighborWork,exclude:Option<i32>,fluid_done:bool}
impl SignalWorld for Host<'_>{fn block(&mut self,p:P)->Result<u32>{self.world.block(p.0,p.1,p.2)}}
struct NeighborWork { limit:i32,count:i32,working:bool,pending:Vec<NeighborTask> }
impl NeighborWork { fn new(limit:i32)->Self {Self {limit,count:0,working:false,pending:Vec::new()}} }
trait NeighborDispatch:RedstoneWorld {
 fn neighbor_work(&mut self)->&mut NeighborWork;
 fn fluid_callbacks_done(&self)->bool;
}
fn dispatch_neighbors(host:&mut impl NeighborDispatch,p:P,source:u32,except:Option<Direction>)->Result<()> {
 let work=host.neighbor_work();let previous=work.count;work.count=work.count.saturating_add(1);
 if work.limit>=0&&previous>=work.limit{return Ok(())}
 work.pending.push(NeighborTask {p,source,next:0,except});if work.working{return Ok(())}work.working=true;
 let result=(||{loop {
  let Some(mut task)=host.neighbor_work().pending.pop() else {break};
  if task.next>=6{continue}let direction=redstone::NEIGHBOR_ORDER[task.next];task.next+=1;
  let target=redstone::step(task.p,direction);let source=task.source;let skip=task.except==Some(direction);
  host.neighbor_work().pending.push(task);
  if !skip {let state=host.block(target)?;if host.fluid_callbacks_done()&&redstone::block(state)?.neighbor=="dko"{continue}
   ensure!(redstone::neighbor_changed(host,target,source)?,"unported neighborChanged at {target:?}");}
 }Ok(())})();let work=host.neighbor_work();work.working=false;work.count=0;result
}
impl NeighborDispatch for Host<'_> {
 fn neighbor_work(&mut self)->&mut NeighborWork {&mut self.work}
 fn fluid_callbacks_done(&self)->bool {self.fluid_done}
}
impl Host<'_> {fn notify(&mut self,p:P,source:u32,except:Option<Direction>)->Result<()> {
 dispatch_neighbors(self,p,source,except)
}}
impl RedstoneWorld for Host<'_>{
 fn set_block(&mut self,p:P,s:u32,flags:u32)->Result<()>{let old=self.world.block(p.0,p.1,p.2)?;let changes=self.fluid.set_block_with_primary_callback(self.world,p,s,flags,false)?;self.fluid.interactions.mark_redstone_primary(p,old,s);redstone::on_changed(self,p,old,s)?;mc121_rust::fluid_simulation::FluidSimulationHost::block_changes(&mut self.fluid,&changes)}
 fn schedule(&mut self,p:P,s:u32,delay:i32,priority:TickPriority)->Result<()>{block_ticks::schedule(self.queue,p,s,self.now,delay,priority,self.order)?;Ok(())}
 fn has_scheduled(&self,p:P,s:u32)->Result<bool>{Ok(self.queue.has_scheduled_tick(block_ticks::pos(p),&block_ticks::key(s)?))}
 fn will_tick(&mut self,p:P,s:u32)->Result<bool>{Ok(self.queue.will_tick_this_tick(block_ticks::pos(p),&block_ticks::key(s)?))}
 fn update_neighbors(&mut self,p:P,source:u32,except:Option<Direction>)->Result<()>{self.notify(p,source,except)}
 fn sound(&mut self,p:P,id:i32,volume:f32,pitch:f32)->Result<()>{let seed=self.sounds.next_sound_seed()?;let origin=Vec3::new(p.0 as f64+0.5,p.1 as f64+0.5,p.2 as f64+0.5);let mut w=Writer::new();w.varint(id+1).varint(4).i32((origin.x*8.)as i32).i32((origin.y*8.)as i32).i32((origin.z*8.)as i32).f32(volume).f32(pitch).i64(seed);let recipients=self.fluid.players.iter().filter(|v|Some(v.entity_id)!=self.exclude&&(v.position.x-origin.x).powi(2)+(v.position.y-origin.y).powi(2)+(v.position.z-origin.z).powi(2)<256.).map(|v|v.entity_id).collect();let _=self.fluid.events.send(PlayerEvent::WorldAudience{recipients:Arc::new(recipients),packets:Arc::new(vec![Packet{id:0x68,payload:w.into_inner()}])});Ok(())}
 fn game_event(&mut self,p:P,name:&'static str)->Result<()>{self.fluid.interactions.append(-1,[mc121_rust::world_interactions::InteractionEvent::GameEvent{position:p,name:name.into()}]);Ok(())}
 fn level_event(&mut self,p:P,event:i32,data:i32)->Result<()>{let packet=mc121_rust::item_effects::level_event_packet(event,p,data,false);let _=self.fluid.events.send(PlayerEvent::WorldAudience{recipients:Arc::new(self.fluid.players.iter().map(|v|v.entity_id).collect()),packets:Arc::new(vec![packet])});Ok(())}
 fn entities_on_plate(&mut self,p:P,mobs_only:bool)->Result<usize>{self.actors.count(self.sounds,p,mobs_only)}
 fn arrow_on_button(&mut self,_:P,_:u32)->Result<bool>{Ok(false)} // actual indexed native classes contain no Arrow.
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
  let snapshots=self.registry.snapshot();
  let external=capture_external_trigger_bodies(&self.item_runtime,
      self.experience_runtime.physical_states(),self.falling_blocks.registered().map(|(_,actor)|actor))?;
  let trigger=override_body.map(|body|->Result<_>{
      let handle=self.item_runtime.entity_handle(body.entity_id).context("actual current redstone body handle absent")?;
      ensure!(self.item_runtime.entity_kind(handle)==Some(mc121_rust::runtime_entity_index::EntityKind::Player),
          "actual current redstone body is not the expected Player");Ok((handle,body))}).transpose()?;
  let Self{block_runtime,redstone_runtime,fluid_runtime,chunk_runtime,item_runtime,gameplay,world_interactions,entity_source,world_revision,random_block_ticks,registry,player_motion_runtime,..}=self;
  block_runtime.mark_used();if let Some(source)=entity_source.as_mut(){source.observe_block_scheduler_use();}let exclude=if let Action::Use(_,_,id)=action{id}else{None};
  let fluid=main_fluid_simulation::MainFluidHost{chunks:chunk_runtime,gameplay,interactions:world_interactions,entity_source,revision:world_revision,events,players:snapshots,active_items:item_runtime.item_count(),border,random_ticks:Some(random_block_ticks)};
  let actors=NativePlateQuery {registry,motion:player_motion_runtime,external:&external,trigger};
  let mut host=Host{world,fluid,queue:&mut block_runtime.queue,order:&mut fluid_runtime.sub_tick_order,sounds:item_runtime,now,actors,work:NeighborWork::new(redstone_runtime.limit),exclude,fluid_done:matches!(action,Action::Neighbors(..))};
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

/// All actual owners needed by pressure callbacks. Item callbacks may supply
/// the genuine TickParts query/sound loan; no full runtime is reconstructed.
pub(super) struct NativePressurePlateServices<'a> {
 pub(super) world:&'a mut WorldBackend,
 pub(super) runtime:&'a mut dyn RuntimeEntityQuerySound,
 pub(super) gameplay:&'a mut MainItemGameplay,
 pub(super) registry:&'a EntityRegistry,
 pub(super) player_motion:&'a main_player_motion_hooks::MainPlayerMotionRuntime,
 pub(super) chunks:&'a mut MainChunkRuntime,
 pub(super) interactions:&'a mut main_world_interactions::WorldInteractionRuntime,
 pub(super) source:&'a mut Option<NativeEntitySource>,
 pub(super) block_runtime:&'a mut main_block_ticks::MainBlockRuntime,
 pub(super) fluid_runtime:&'a mut main_fluid_simulation::MainFluidRuntime,
 pub(super) redstone_runtime:&'a mut MainRedstoneRuntime,
 pub(super) random_ticks:&'a mut main_random_block_ticks::MainRandomBlockTicks,
 pub(super) world_revision:&'a mut u64,
 pub(super) context:&'a WorldContext,
 pub(super) changes:&'a broadcast::Sender<WorldUpdate>,
 pub(super) events:&'a broadcast::Sender<PlayerEvent>,
 pub(super) external_trigger_bodies:&'a [NativeTriggerBody],
 pub(super) query_region:main_chunk_runtime::CurrentChunkQueryRegion,
 pub(super) now:i64,
 pub(super) border:CollisionBorder,
}

fn pressure_services<'a>(services:&'a mut ActualFallingBlockEditServices<'_>)
    ->Result<NativePressurePlateServices<'a>> {
 Ok(NativePressurePlateServices {
  world:&mut *services.world,runtime:&mut *services.items,gameplay:&mut *services.gameplay,
  registry:services.registry,player_motion:services.player_motion,chunks:&mut *services.chunks,
  interactions:&mut *services.interactions,source:&mut *services.source,
  block_runtime:&mut *services.block_runtime,fluid_runtime:&mut *services.fluid_runtime,
  redstone_runtime:&mut *services.redstone_runtime,random_ticks:&mut *services.random_ticks,
  world_revision:&mut *services.world_revision,context:services.context,changes:services.changes,
  events:services.events,external_trigger_bodies:services.external_trigger_bodies
      .context("pressure callback lacks actual external body views")?,
  query_region:services.query_region.context("pressure callback lacks actual query region")?,
  now:services.now,border:services.border,
 })
}

impl NativePressurePlateServices<'_> {
 fn commit_pressure_state(&mut self,p:P,state:u32)->Result<()> {
  self.context.dimension.require_runtime_layout()?;
  self.gameplay.require_supported_floor()?;self.random_ticks.require_healthy()?;
  ensure!(self.interactions.pending_count()==0,"pressure write has earlier actual callbacks");
  let WorldBackend::Fixture(flat)=&*self.world else {bail!("pressure write lacks actual native world")};
  self.chunks.native_lighting_mut()?.require_current(flat)?;
  let source=self.source.as_ref().context("actual pressure source is absent")?;
  source.require_current_world(self.world)?;
  source.require_fluid_history(self.world,&self.interactions.fluid_shapes)?;
  let mut query=RuntimeInteractionWorld::new(self.world,world::MIN_Y,world::MAX_Y+1,
      self.border,|_,_|bail!("pressure updates do not perform entity placement"))?;
  let plan=world_interactions::plan_edit(p,state,2,&mut query)?;
  for event in &plan.events {
   match event {
    world_interactions::InteractionEvent::Write {position,old_state,state,..}=>{
     ensure!(*position==p&&redstone::block(*old_state)?.min==redstone::block(*state)?.min,
         "pressure write needs an additional actual shape/block owner");
    }
    world_interactions::InteractionEvent::ScheduleBlockTick(tick)=>{
     ensure!(self.block_runtime.owns_falling_tick(tick.state,tick.delay)?
         ||redstone::owns_scheduled_callback(tick.state,tick.delay)?,
         "pressure shape callback has an unavailable actual scheduler");
     self.block_runtime.prepare_halo(self.world,self.chunks,
         (tick.position.0.div_euclid(16),tick.position.2.div_euclid(16)),self.now)?;
    }
    _=>bail!("pressure write needs an active fluid/fire/BE/loot/effect owner"),
   }
  }
  crate::main_native_source::callbacks_can_complete(self.world,&plan)?;
  let token=self.source.as_ref().expect("validated pressure source")
      .prepare_owned_edit_with_block_ticks(self.world,&plan,self.block_runtime)?;
  ensure!(!token.requires_fire_owner()?,"pressure write requires actual fire callbacks");
  token.require_fluid_history(&self.interactions.fluid_shapes)?;
  let revision=self.world_revision.checked_add(1).context("pressure world revision exhausted")?;
  self.random_ticks.prepare_plan(self.world,&plan)?;
  let mut changes=crate::main_world_interactions::commit_fixture_plan(self.world,&plan)?
      .context("pressure write requires actual native atomic backend")?;
  self.random_ticks.observe_plan(&plan)?;
  self.source.as_mut().expect("validated pressure source").observe_owned_commit(token,self.world)?;
  for change in &changes {
   if let WorldUpdate::Block(block)=change {
    self.gameplay.observe_block_change((block.x,block.y,block.z),block.state)?;
    self.chunks.invalidate_block_lighting((block.x,block.y,block.z))?;
   }
  }
  for event in &plan.events {
   if let world_interactions::InteractionEvent::ScheduleBlockTick(tick)=event {
    self.block_runtime.mark_used();self.source.as_mut().expect("validated source").observe_block_scheduler_use();
    block_ticks::schedule(&mut self.block_runtime.queue,tick.position,tick.state,self.now,tick.delay,
        TickPriority::Normal,&mut self.fluid_runtime.sub_tick_order)?;
   }
  }
  changes.extend(self.chunks.settle_lighting(self.world)?);
  self.source.as_mut().expect("validated source").complete_owned_edits(self.world,
      self.chunks.native_lighting_mut()?,&self.interactions.fluid_shapes,self.interactions.pending_count())?;
  *self.world_revision=revision;for change in changes {let _=self.changes.send(change);}Ok(())
 }
}

/// Retained-actor-safe parts host. It shares the real kernel and neighbor
/// executor, but uses strict source/light transactions instead of the legacy
/// fluid Host's item-count-based source fallback.
struct PressureHost<'a,'level> {
 services:&'a mut NativePressurePlateServices<'level>,
 actors:NativePlateQuery<'a>,
 position:P,
 work:NeighborWork,
}
impl SignalWorld for PressureHost<'_,'_> {
 fn block(&mut self,p:P)->Result<u32>{self.services.world.block(p.0,p.1,p.2)}
}
impl NeighborDispatch for PressureHost<'_,'_> {
 fn neighbor_work(&mut self)->&mut NeighborWork {&mut self.work}
 fn fluid_callbacks_done(&self)->bool {false}
}
impl RedstoneWorld for PressureHost<'_,'_> {
 fn set_block(&mut self,p:P,state:u32,flags:u32)->Result<()> {
  ensure!(p==self.position&&flags==2,"pressure callback needs an additional actual block-write owner");
  let old=self.block(p)?;
  ensure!(redstone::block(old)?.min==redstone::block(state)?.min
    &&crate::main_falling_block_redstone::pressure_plate_profile(state)?.is_some(),
    "pressure callback changed its actual Block singleton");
  self.services.commit_pressure_state(p,state)?;
  Ok(())
 }
 fn schedule(&mut self,p:P,state:u32,delay:i32,priority:TickPriority)->Result<()> {
  ensure!(p==self.position&&redstone::owns_scheduled_callback(state,delay)?,
      "pressure callback has an unknown actual scheduled owner");
  self.services.block_runtime.mark_used();
  self.services.source.as_mut().context("actual pressure source is absent")?.observe_block_scheduler_use();
  block_ticks::schedule(&mut self.services.block_runtime.queue,p,state,self.services.now,delay,
      priority,&mut self.services.fluid_runtime.sub_tick_order)?;Ok(())
 }
 fn has_scheduled(&self,p:P,state:u32)->Result<bool>{Ok(self.services.block_runtime.queue
      .has_scheduled_tick(block_ticks::pos(p),&block_ticks::key(state)?))}
 fn will_tick(&mut self,p:P,state:u32)->Result<bool>{Ok(self.services.block_runtime.queue
      .will_tick_this_tick(block_ticks::pos(p),&block_ticks::key(state)?))}
 fn update_neighbors(&mut self,p:P,source:u32,except:Option<Direction>)->Result<()> {
  dispatch_neighbors(self,p,source,except)
 }
 fn sound(&mut self,p:P,id:i32,volume:f32,pitch:f32)->Result<()> {
  let seed=self.services.runtime.next_sound_seed()?;
  let origin=Vec3::new(f64::from(p.0)+0.5,f64::from(p.1)+0.5,f64::from(p.2)+0.5);
  let mut fields=Writer::new();fields.varint(id+1).varint(4)
      .i32((origin.x*8.)as i32).i32((origin.y*8.)as i32).i32((origin.z*8.)as i32)
      .f32(volume).f32(pitch).i64(seed);
  let radius=if volume>1.{f64::from(16.*volume)}else{16.};
  let recipients=self.services.registry.snapshot().into_iter().filter(|player|
      Vec3::new(player.position.x,player.position.y,player.position.z)
          .subtract(origin).length_squared()<radius*radius).map(|player|player.entity_id).collect();
  let _=self.services.events.send(PlayerEvent::WorldAudience {recipients:Arc::new(recipients),
      packets:Arc::new(vec![Packet{id:0x68,payload:fields.into_inner()}])});Ok(())
 }
 fn game_event(&mut self,p:P,name:&'static str)->Result<()> {
  let region=self.services.query_region;
  let radius=world_interactions::game_event_radius(name)?;
  // The actual trigger reference is retained in actors.trigger (None only for
  // a scheduled release). Original certified-empty dispatch never reads any
  // GameEvent.Context entity getter; no proxy or null trigger is substituted.
  let mut frame=self.services.gameplay.frame_current(self.services.registry,
      &self.services.chunks.manager,true,region);
  mc121_rust::game_event_context::post_to_empty_registries(&mut frame,
      mc121_rust::game_event_context::GameEventPost {position:Vec3::new(
          f64::from(p.0)+0.5,f64::from(p.1)+0.5,f64::from(p.2)+0.5),notification_radius:radius})?;
  Ok(())
 }
 fn level_event(&mut self,_:P,_:i32,_:i32)->Result<()> {
  bail!("pressure callback requires an unavailable actual level-event phase")
 }
 fn entities_on_plate(&mut self,p:P,living_only:bool)->Result<usize> {
  self.actors.count(self.services.runtime,p,living_only)
 }
 fn arrow_on_button(&mut self,_:P,_:u32)->Result<bool> {
  bail!("pressure host does not substitute for an actual Arrow/button owner")
 }
}

fn preflight_pressure_services(services:&mut NativePressurePlateServices<'_>,p:P)->Result<()> {
 services.gameplay.require_supported_floor()?;
 services.redstone_runtime.require_healthy()?;services.block_runtime.require_healthy()?;
 services.fluid_runtime.require_healthy()?;
 ensure!(services.interactions.pending_count()==0,"pressure callback cannot bypass earlier actual world work");
 let WorldBackend::Fixture(flat)=&*services.world else {bail!("native pressure callback cannot certify imported source")};
 services.chunks.native_lighting_mut()?.require_current(flat)?;
 services.source.as_ref().context("actual pressure source is absent")?.require_current_world(services.world)?;
 services.runtime.sound_random_state48().context("actual pressure Level sound RNG is unavailable")?;
 ensure!(crate::main_falling_block_redstone::pressure_plate_profile(services.world.block(p.0,p.1,p.2)?)?.is_some(),
     "pressure callback received a foreign actual Block");
 // The initial parts profile owns the complete plate operation, but admits
 // only literal inherited surrounding neighborChanged callbacks. Further
 // active controller/fluid/BE networks need their real parts executors.
 for origin in [p,redstone::step(p,Direction::Down)] {
  for direction in redstone::NEIGHBOR_ORDER {
   let at=redstone::step(origin,direction);let state=services.world.block(at.0,at.1,at.2)?;
   ensure!(redstone::block(state)?.neighbor=="dtb"
       &&mc121_rust::fluid_physics::FluidState::from_block(state)?.is_empty()
       &&!mc121_rust::fire_simulation::has_block_entity(state)?,
       "pressure callback reaches an active unported neighboring owner");
  }
 }
 let region=services.query_region;
 let radius=world_interactions::game_event_radius("minecraft:block_activate")?;
 let min=(p.0.wrapping_sub(radius).div_euclid(16),p.2.wrapping_sub(radius).div_euclid(16));
 let max=(p.0.wrapping_add(radius).div_euclid(16),p.2.wrapping_add(radius).div_euclid(16));
 ensure!(region.contains(min)&&region.contains(max),"pressure event exceeds actual query region");
 services.chunks.manager.certify_full_region(min,max)?;
 services.block_runtime.prepare_halo(services.world,services.chunks,
      (p.0.div_euclid(16),p.2.div_euclid(16)),services.now)?;
 Ok(())
}

fn run_pressure_parts(services:&mut NativePressurePlateServices<'_>,p:P,
    trigger:Option<(EntityHandle,&EntityPhysicalState)>,scheduled:bool)->Result<()> {
 NativePlateQuery {registry:services.registry,motion:services.player_motion,
     external:services.external_trigger_bodies,trigger}.validate_current_trigger(services.runtime)?;
 preflight_pressure_services(services,p)?;
 let external=services.external_trigger_bodies;
 let query=NativePlateQuery {registry:services.registry,motion:services.player_motion,external,trigger};
 let limit=services.redstone_runtime.limit;
 let mut host=PressureHost {services,actors:query,position:p,work:NeighborWork::new(limit)};
 let result=if scheduled {redstone::pressure_plate_tick(&mut host,p)} else {redstone::entity_inside(&mut host,p)};
 match result {
  Ok(true)=>{
   ensure!(host.services.interactions.pending_count()==0,"pressure callback left actual unfinished world work");Ok(())
  }
  Ok(false)=>bail!("actual pressure kernel declined its captured Block"),
  Err(error)=>{host.services.redstone_runtime.stopped=Some(format!("{error:#}"));Err(error)}
 }
}

pub(super) fn native_pressure_plate_inside_parts(services:&mut NativePressurePlateServices<'_>,
    handle:EntityHandle,kind:EntityKind,actor:&mut EntityPhysicalState,p:P)->Result<()> {
 ensure!(actor.entity_id==handle.id&&services.runtime.entity_kind(handle)==Some(kind),
     "pressure callback lost its actual physical/index generation");
 run_pressure_parts(services,p,Some((handle,&*actor)),false)
}
pub(super) fn native_pressure_plate_inside(services:&mut ActualFallingBlockEditServices<'_>,
    handle:EntityHandle,kind:EntityKind,actor:&mut EntityPhysicalState,p:P)->Result<()> {
 native_pressure_plate_inside_parts(&mut pressure_services(services)?,handle,kind,actor,p)
}

pub(super) fn falling_pressure_plate_inside(services:&mut ActualFallingBlockEditServices<'_>,
    publication:&mut FallingBlockPublication,actor:&mut EntityPhysicalState,p:P)->Result<()> {
 native_pressure_plate_inside(services,publication.handle,EntityKind::FallingBlock,actor,p)
}

impl Server {
 pub(super) fn tick_native_pressure_plate_locked(&self,world:&mut WorldBackend,
      players:&mut LivePlayers,p:P,now:i64,border:CollisionBorder)->Result<()> {
  let radius=world_interactions::game_event_radius("minecraft:block_activate")?;
  let min=(p.0.wrapping_sub(radius).div_euclid(16),p.2.wrapping_sub(radius).div_euclid(16));
  let max=(p.0.wrapping_add(radius).div_euclid(16),p.2.wrapping_add(radius).div_euclid(16));
  let region=players.chunk_runtime.certify_current_query_region(world,&mut players.chunk_transports,
       &self.entity_events,now,min,max)?;
  let external=capture_external_trigger_bodies(&players.item_runtime,
       players.experience_runtime.physical_states(),players.falling_blocks.registered().map(|(_,actor)|actor))?;
  let LivePlayers {item_runtime,gameplay,registry,player_motion_runtime,chunk_runtime,
       world_interactions,entity_source,block_runtime,fluid_runtime,redstone_runtime,
       random_block_ticks,world_revision,world_context,rain_context,..}=players;
  let mut services=ActualFallingBlockEditServices {
       world,items:item_runtime,gameplay,registry,player_motion:player_motion_runtime,
       chunks:chunk_runtime,interactions:world_interactions,source:entity_source,
       block_runtime,fluid_runtime,redstone_runtime,random_ticks:random_block_ticks,
       world_revision,context:world_context,rain:rain_context.as_mut(),changes:&self.changes,
       events:&self.entity_events,entities:&self.entities,external_entity_uuids:None,
       external_trigger_bodies:Some(&external),query_region:Some(region),now,border,
  };
  run_pressure_parts(&mut pressure_services(&mut services)?,p,None,true)
 }
}
