//! Original 1.21 PlayerChunkSender scheduling and pending-chunk selection.
//!
//! Host callbacks supply actual readiness and transport. This module does not
//! treat a queued chunk as loaded, encode a synthetic chunk, or acknowledge its
//! own batches. Call it in the original chunk-send phase after connection ticks.
use anyhow::Result;
#[path = "player_chunk_sender_order.rs"]
mod order;

pub type ChunkPosition = (i32, i32);

/// A ready original-level chunk may have a different position from the queried
/// key. Selection sorts/removes using this actual position in the full branch.
#[derive(Clone, Debug)]
pub struct ReadyChunk<H> {
    pub handle: H,
    pub actual_position: ChunkPosition,
}

/// Each method is required. Errors preserve the original completed prefix:
/// selected chunks are removed before begin, and quota is spent only after end.
pub trait ChunkBatchHost {
    type Chunk;
    /// Called lazily, after outstanding-batch, quota, and pending-empty gates.
    fn player_chunk_position(&mut self) -> Result<ChunkPosition>;
    fn ready_chunk(&mut self, requested: ChunkPosition) -> Result<Option<ReadyChunk<Self::Chunk>>>;
    fn begin_batch(&mut self) -> Result<()>;
    /// Must send the real chunk/light packet and its original debug hook.
    fn send_chunk(&mut self, chunk: ReadyChunk<Self::Chunk>) -> Result<()>;
    fn end_batch(&mut self, count: usize) -> Result<()>;
}
pub trait ChunkDropHost {
    /// Queried only when the position was not removed from the pending set.
    fn player_alive(&mut self) -> Result<bool>;
    fn unload_chunk(&mut self, position: ChunkPosition) -> Result<()>;
}
#[derive(Clone, Debug)]
pub struct PlayerChunkSender {
    pending: order::PendingChunks,
    memory_connection: bool,
    desired_chunks_per_tick: f32,
    batch_quota: f32,
    unacknowledged_batches: i32,
    max_unacknowledged_batches: i32,
}
impl PlayerChunkSender {
    pub fn new(memory_connection: bool) -> Self {
        Self {
            pending: order::PendingChunks::new(),
            memory_connection,
            desired_chunks_per_tick: 9.0,
            batch_quota: 0.0,
            unacknowledged_batches: 0,
            max_unacknowledged_batches: 1,
        }
    }
    pub fn mark_pending(&mut self, position: ChunkPosition) {
        self.pending.insert(pack(position));
    }
    pub fn is_pending(&self, position: ChunkPosition) -> bool {
        self.pending.contains(pack(position))
    }
    /// Original stream encounter order (not the separate fastutil Iterator).
    pub fn pending_positions(&self) -> impl Iterator<Item = ChunkPosition> + '_ {
        self.pending.iter().map(unpack)
    }
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    pub fn desired_chunks_per_tick(&self) -> f32 {
        self.desired_chunks_per_tick
    }
    pub fn batch_quota(&self) -> f32 {
        self.batch_quota
    }
    pub fn unacknowledged_batches(&self) -> i32 {
        self.unacknowledged_batches
    }
    pub fn max_unacknowledged_batches(&self) -> i32 {
        self.max_unacknowledged_batches
    }
    /// The serverbound chunk_batch_received packet is protocol-767 ID 0x08.
    /// Unsolicited acknowledgements still decrement the original signed counter.
    pub fn on_ack(&mut self, desired: f32) {
        self.unacknowledged_batches = self.unacknowledged_batches.wrapping_sub(1);
        self.desired_chunks_per_tick = if desired.is_nan() {
            0.01
        } else {
            desired.clamp(0.01, 64.0)
        };
        if self.unacknowledged_batches == 0 {
            self.batch_quota = 1.0;
        }
        self.max_unacknowledged_batches = 10;
    }
    pub fn drop_chunk(
        &mut self,
        position: ChunkPosition,
        host: &mut impl ChunkDropHost,
    ) -> Result<()> {
        if !self.pending.remove(pack(position)) && host.player_alive()? {
            host.unload_chunk(position)?;
        }
        Ok(())
    }
    pub fn send_next(&mut self, host: &mut impl ChunkBatchHost) -> Result<usize> {
        if self.unacknowledged_batches >= self.max_unacknowledged_batches {
            return Ok(0);
        }
        let limit = java_max(1.0, self.desired_chunks_per_tick);
        self.batch_quota = java_min(self.batch_quota + self.desired_chunks_per_tick, limit);
        if self.batch_quota < 1.0 || self.pending.is_empty() {
            return Ok(0);
        }
        let center = host.player_chunk_position()?;
        let chunks = self.collect_chunks(center, host)?;
        if chunks.is_empty() {
            return Ok(0);
        }
        self.unacknowledged_batches = self.unacknowledged_batches.wrapping_add(1);
        host.begin_batch()?;
        let count = chunks.len();
        for chunk in chunks {
            host.send_chunk(chunk)?;
        }
        host.end_batch(count)?;
        self.batch_quota -= count as f32;
        Ok(count)
    }
    fn collect_chunks<H: ChunkBatchHost>(
        &mut self,
        center: ChunkPosition,
        host: &mut H,
    ) -> Result<Vec<ReadyChunk<H::Chunk>>> {
        let k = java_floor(self.batch_quota);
        let all = self.memory_connection || (self.pending.len() as i32) <= k;
        let mut chunks = Vec::new();
        let mut collect_ready = |key| -> Result<()> {
            if let Some(chunk) = host.ready_chunk(unpack(key))? {
                chunks.push(chunk);
            }
            Ok(())
        };
        if all {
            for key in self.pending.iter() {
                collect_ready(key)?;
            }
            chunks.sort_by_key(|chunk| order::distance_squared(center, chunk.actual_position));
        } else {
            for key in order::least_by_distance(self.pending.iter(), k as usize, center) {
                collect_ready(key)?;
            }
        }
        for chunk in &chunks {
            self.pending.remove(pack(chunk.actual_position));
        }
        Ok(chunks)
    }
}
fn pack((x, z): ChunkPosition) -> i64 {
    ((z as i64) << 32) | (x as u32 as i64)
}
fn unpack(key: i64) -> ChunkPosition {
    (key as i32, ((key as u64) >> 32) as i32)
}
fn java_floor(value: f32) -> i32 {
    let n = value as i32;
    if value < n as f32 {
        n.wrapping_sub(1)
    } else {
        n
    }
}
fn java_min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        a + b
    } else if a == 0.0 && b == 0.0 {
        f32::from_bits(a.to_bits() | b.to_bits())
    } else if a <= b {
        a
    } else {
        b
    }
}
fn java_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        a + b
    } else if a == 0.0 && b == 0.0 {
        f32::from_bits(a.to_bits() & b.to_bits())
    } else if a >= b {
        a
    } else {
        b
    }
}

#[cfg(test)]
#[path = "../tools/player_chunk_sender_tests.rs"]
mod tests;
