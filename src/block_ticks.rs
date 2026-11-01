//! A single native ServerLevel BLOCK tick queue. The key is the Block singleton
//! (its first state ID), never a property-bearing BlockState such as fire age.
use crate::{placement, tick::{BlockPos, ChunkTickQueue, LevelTickScheduler, SavedTick, ScheduledTick, TickPriority}};
use anyhow::{ensure, Result};

pub type BlockTickKey = u32;
pub type BlockTickScheduler = LevelTickScheduler<BlockTickKey>;
pub fn key(state: u32) -> Result<BlockTickKey> { placement::placement_block_min_state(state) }
pub fn pos(p:(i32,i32,i32))->BlockPos { BlockPos::new(p.0,p.1,p.2) }
pub fn schedule(queue:&mut BlockTickScheduler,p:(i32,i32,i32),state:u32,now:i64,delay:i32,priority:TickPriority,order:&mut i64)->Result<bool>{
    ensure!(queue.chunk(pos(p).chunk()).is_some(),"BLOCK tick target chunk is not loaded: {p:?}");
    let insertion=*order; *order=order.wrapping_add(1);
    Ok(queue.schedule(ScheduledTick::new(key(state)?,pos(p),now.wrapping_add(i64::from(delay)),priority,insertion)))
}
pub fn load(queue:&mut BlockTickScheduler,chunk:(i32,i32),saved:Vec<SavedTick<BlockTickKey>>,now:i64)->Result<()> {
    ensure!(queue.chunk(chunk).is_none(),"duplicate BLOCK tick chunk");
    for t in &saved {ensure!(t.pos.chunk()==chunk && key(t.kind)?==t.kind,"invalid canonical BLOCK tick key/chunk");}
    queue.add_chunk(chunk,ChunkTickQueue::from_saved_for_chunk(chunk,saved)); queue.unpack_chunk(chunk,now); Ok(())
}
/// Keep LevelTicks' collected set alive throughout callbacks, including nested
/// willTickThisTick and scheduling. On failure only the already-executed prefix
/// has run; preserve the remaining original ticks without changing their order.
pub fn tick(queue:&mut BlockTickScheduler,now:i64,budget:usize,tickable:impl FnMut((i32,i32))->bool,mut callback:impl FnMut(&mut BlockTickScheduler,&ScheduledTick<BlockTickKey>)->Result<()>)->Result<usize>{
    let mut failed=None; let mut count=0;
    queue.tick(now,budget,tickable,|queue,tick|{
        if failed.is_some(){queue.schedule(tick.clone());return;}
        match callback(queue,tick){Ok(())=>count+=1,Err(e)=>failed=Some(e)}
    });
    match failed {Some(e)=>Err(e),None=>Ok(count)}
}
