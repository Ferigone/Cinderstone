//! Main owns the only ServerLevel BLOCK LevelTicks, shared by fire/redstone.
use super::*;
use mc121_rust::{block_ticks::{self,BlockTickScheduler},tick::{SavedTick,TickPriority},placement::PlannedBlockTick};
#[derive(Clone,Serialize,Deserialize)]
pub(super) struct SavedBlockRuntime { chunks:Vec<((i32,i32),Vec<SavedTick<u32>>)>, stopped:Option<String>, was_used:bool }
pub(super) struct MainBlockRuntime {
    pub(super) queue:BlockTickScheduler,
    loaded:BTreeSet<(i32,i32)>, saved:BTreeMap<(i32,i32),Vec<SavedTick<u32>>>,
    stopped:Option<String>, native_source:bool, was_used:bool,
    falling_owner:Option<main_falling_block_damage::NativeFallingDamageTypes>,
}
impl MainBlockRuntime {
    pub fn new(native_source:bool)->Self{Self{queue:BlockTickScheduler::new(),loaded:BTreeSet::new(),saved:BTreeMap::new(),stopped:None,native_source,was_used:false,falling_owner:None}}
    pub(super) fn install_falling_owner(&mut self, owner:main_falling_block_damage::NativeFallingDamageTypes)->Result<()>{self.require_healthy()?;ensure!(self.falling_owner.is_none(),"falling callback owner was already installed");self.falling_owner=Some(owner);Ok(())}
    pub(super) fn falling_damage_types(&self)->Result<&main_falling_block_damage::NativeFallingDamageTypes>{self.require_healthy()?;self.falling_owner.as_ref().context("actual configured falling callback owner is unavailable")}
    pub(super) fn owns_anvil_tick(&self,state:u32,delay:i32)->Result<bool>{Ok(self.falling_owner.is_some()&&mc121_rust::placement::anvil_falling_delay(state)?==Some(delay))}
    pub fn require_healthy(&self)->Result<()>{ensure!(self.native_source,"global BLOCK source unavailable");ensure!(self.stopped.is_none(),"global BLOCK runtime stopped: {:?}",self.stopped);Ok(())}
    pub fn pending_count(&self)->usize{self.queue.count()+self.saved.values().map(Vec::len).sum::<usize>()}
    pub fn was_used(&self)->bool{self.was_used}
    pub(super) fn mark_used(&mut self){self.was_used=true;}
    pub fn checkpoint(&self,now:i64)->SavedBlockRuntime{let mut chunks=self.saved.clone();for c in &self.loaded{chunks.insert(*c,self.queue.chunk(*c).expect("owned BLOCK chunk").saved_ticks(now));}SavedBlockRuntime{chunks:chunks.into_iter().collect(),stopped:self.stopped.clone(),was_used:self.was_used}}
    pub fn from_checkpoint(s:SavedBlockRuntime,native_source:bool)->Result<Self>{let mut r=Self::new(native_source);r.stopped=s.stopped;r.was_used=s.was_used;for(c,t)in s.chunks{ensure!(t.iter().all(|v|v.pos.chunk()==c&&block_ticks::key(v.kind).ok()==Some(v.kind)),"invalid BLOCK snapshot");ensure!(r.saved.insert(c,t).is_none(),"duplicate saved BLOCK chunk");}Ok(r)}
    pub fn sync_saved(&mut self,chunks:&MainChunkRuntime,now:i64)->Result<()>{for c in self.loaded.iter().copied().collect::<Vec<_>>(){if !chunks.manager.current_full_ready(c){let q=self.queue.remove_chunk(c).context("BLOCK queue disappeared")?;self.saved.insert(c,q.saved_ticks(now));self.loaded.remove(&c);}}Ok(())}
    pub fn prepare_halo(&mut self,world:&WorldBackend,chunks:&MainChunkRuntime,c:(i32,i32),now:i64)->Result<()>{self.require_healthy()?;ensure!(matches!(world,WorldBackend::Fixture(_)),"imported BLOCK scheduler unavailable");let min=(c.0.checked_sub(2).context("BLOCK halo overflow")?,c.1.checked_sub(2).context("BLOCK halo overflow")?);let max=(c.0.checked_add(2).context("BLOCK halo overflow")?,c.1.checked_add(2).context("BLOCK halo overflow")?);chunks.manager.certify_full_region(min,max)?;for x in min.0..=max.0{for z in min.1..=max.1{let p=(x,z);if self.loaded.insert(p){block_ticks::load(&mut self.queue,p,self.saved.remove(&p).unwrap_or_default(),now)?;}}}Ok(())}
    pub fn adopt_legacy_fire(&mut self,fire:&mut main_fire_simulation::MainFireRuntime,now:i64)->Result<()>{let old=fire.take_legacy_ticks(now);ensure!(self.pending_count()==0||old.iter().all(|(_,v)|v.is_empty()),"both legacy fire and global BLOCK have work");let kind=mc121_rust::fire_simulation::normal_fire_key()?;for(c,ticks)in old{if !ticks.is_empty(){self.was_used=true;}let converted=ticks.into_iter().map(|t|SavedTick{kind,pos:t.pos,delay:t.delay,priority:t.priority}).collect::<Vec<_>>();if !converted.is_empty(){ensure!(self.saved.insert(c,converted).is_none(),"duplicate legacy BLOCK owner");}}Ok(())}
}
impl LivePlayers {
    pub(super) fn schedule_native_block_tick(&mut self,world:&WorldBackend,tick:&PlannedBlockTick,now:i64)->Result<()>{
        ensure!(main_fire_simulation::MainFireRuntime::supports_scheduled_state(tick.state)||mc121_rust::redstone::owns_scheduled_callback(tick.state,tick.delay)?||self.block_runtime.owns_anvil_tick(tick.state,tick.delay)?||(mc121_rust::native_block_entities::container_kind(tick.state).is_some()&&tick.delay==5),"unported scheduled block callback");
        self.block_runtime.prepare_halo(world,&self.chunk_runtime,(tick.position.0.div_euclid(16),tick.position.2.div_euclid(16)),now)?;
        self.block_runtime.was_used=true;
        if let Some(source)=self.entity_source.as_mut(){source.observe_block_scheduler_use();}
        block_ticks::schedule(&mut self.block_runtime.queue,tick.position,tick.state,now,tick.delay,TickPriority::Normal,&mut self.fluid_runtime.sub_tick_order)?;Ok(())
    }
}
#[derive(Default)]pub(super) struct MainBlockTickReport{pub fire:mc121_rust::fire_simulation::FireTickReport,pub executed:usize}
impl MainBlockRuntime{fn pending_chunks(&self)->Vec<(i32,i32)>{self.loaded.iter().copied().filter(|c|self.queue.chunk(*c).is_some_and(|q|!q.is_empty())).chain(self.saved.keys().copied()).collect()}}
impl Server{
 pub(super) async fn tick_block_world(&self)->Result<MainBlockTickReport>{
  let now=self.clock.read().map_err(|_|anyhow!("clock poisoned"))?.game_time;
  let b=self.border.read().map_err(|_|anyhow!("border poisoned"))?.bounds(self.epoch.elapsed().as_millis()as i64);let border=CollisionBorder{min_x:b.min_x,min_z:b.min_z,max_x:b.max_x,max_z:b.max_z};
  let mut world=self.world.write().await;let mut players=self.players.write().map_err(|_|anyhow!("players poisoned"))?;
  if players.block_runtime.was_used(){if let Some(source)=players.entity_source.as_mut(){source.observe_block_scheduler_use();}}
  if players.block_runtime.pending_count()==0{return Ok(MainBlockTickReport::default())}
  players.block_runtime.require_healthy()?;{let LivePlayers{block_runtime,chunk_runtime,..}=&mut*players;block_runtime.sync_saved(chunk_runtime,now)?;}
  let mut tickable=BTreeSet::new();for c in players.block_runtime.pending_chunks(){if main_fluid_simulation::current_block_ready(&players.chunk_runtime,c)?{let LivePlayers{block_runtime,chunk_runtime,..}=&mut*players;block_runtime.prepare_halo(&world,chunk_runtime,c,now)?;tickable.insert(c);}}
  let mut queue=std::mem::replace(&mut players.block_runtime.queue,BlockTickScheduler::new());let mut report=MainBlockTickReport::default();
  let result=block_ticks::tick(&mut queue,now,65_536,|c|tickable.contains(&c),|queue,tick|{
   let p=(tick.pos.x,tick.pos.y,tick.pos.z);let state=world.block(p.0,p.1,p.2)?;if block_ticks::key(state)?!=tick.kind{return Ok(())}
   // Move, rather than clone, the actual collected LevelTicks owner into its
   // normal field during the callback. Reentrant scheduling/queries see the
   // same toRunSet and queue; restore it even when the callback fails.
   std::mem::swap(queue,&mut players.block_runtime.queue);
   let done=(||{if main_fire_simulation::MainFireRuntime::supports_scheduled_state(state){let fire=players.tick_fire_block(&mut world,p,now,border,&self.entity_events)?;report.fire.executed+=fire.executed;report.fire.stale+=fire.stale;report.fire.changed_blocks+=fire.changed_blocks;Ok(())}
   else if players.block_runtime.owns_anvil_tick(state,2)?{self.tick_native_anvil_block_locked(&mut world,&mut players,p,state,now,border)}
   else if mc121_rust::native_block_entities::container_kind(state).is_some(){self.tick_native_container_block_locked(&mut world,&mut players,p,state,now,border)}
   else{players.tick_redstone_block(&mut world,p,now,border,&self.entity_events)}})();
   std::mem::swap(queue,&mut players.block_runtime.queue);done
  });players.block_runtime.queue=queue;
  if players.block_runtime.was_used(){if let Some(source)=players.entity_source.as_mut(){source.observe_block_scheduler_use();}}
  match result{Ok(n)=>{report.executed=n;Ok(report)},Err(e)=>{players.block_runtime.stopped=Some(format!("{e:#}"));Err(e)}}
 }
}
