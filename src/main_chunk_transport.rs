//! Per-session transport ownership for the original PlayerChunkSender core.
//! The private outbox survives loss of its broadcast wake-up notifications.
use super::*;
use mc121_rust::player_chunk_sender::{
    ChunkBatchHost, ChunkDropHost, PlayerChunkSender, ReadyChunk,
};
use mc121_rust::runtime_lighting::NativeRuntimeLighting;
use std::collections::VecDeque;

pub(super) struct ChunkTransportUpdate {
    pub revision: u64,
    pub packets: Vec<Packet>,
    pub loaded: BTreeSet<(i32, i32)>,
    pub pending: BTreeSet<(i32, i32)>,
}
pub(super) struct SessionChunkTransport {
    sender: PlayerChunkSender,
    view: Option<entity_tracking::ChunkView>,
    scheduled: BTreeSet<(i32, i32)>,
    outbox: VecDeque<ChunkTransportUpdate>,
    revision: u64,
}
impl SessionChunkTransport {
    pub fn new() -> Self {
        Self {
            sender: PlayerChunkSender::new(false),
            view: None,
            scheduled: BTreeSet::new(),
            outbox: VecDeque::new(),
            revision: 0,
        }
    }
    pub fn on_ack(&mut self, desired: f32) {
        self.sender.on_ack(desired);
    }
    pub fn queue_view(
        &mut self,
        next: entity_tracking::ChunkView,
        send_center: bool,
        alive: bool,
    ) -> Result<()> {
        self.queue_view_ready(next, send_center, alive, |_| Ok(true))
    }
    pub fn queue_view_ready(
        &mut self,
        next: entity_tracking::ChunkView,
        send_center: bool,
        alive: bool,
        mut ready: impl FnMut((i32, i32)) -> Result<bool>,
    ) -> Result<()> {
        let changes = entity_tracking::chunk_view_difference(self.view, Some(next))?;
        let mut packets = Vec::new();
        if send_center {
            packets.push(Packet {
                id: 0x54,
                payload: encoded(|writer| {
                    writer.varint(next.center.0).varint(next.center.1);
                }),
            });
        }
        self.view = Some(next);
        // Preserve the original difference callback encounter order. A sorted
        // set difference would alter fastutil layout and later distance ties.
        for change in changes {
            match change {
                entity_tracking::ChunkViewChange::Add(position) => {
                    if ready(position)? {
                        self.mark_pending_if_wanted(position);
                    }
                }
                entity_tracking::ChunkViewChange::Remove(position) => {
                    self.sender.drop_chunk(
                        position,
                        &mut PacketDropHost {
                            alive,
                            packets: &mut packets,
                        },
                    )?;
                    self.scheduled.remove(&position);
                }
            }
        }
        self.publish_packets(packets);
        Ok(())
    }
    /// Actual readiness sendSync callback: enqueue for each current viewer.
    pub fn mark_pending_if_wanted(&mut self, position: (i32, i32)) -> bool {
        if !self.view.is_some_and(|view| view.contains(position)) {
            return false;
        }
        self.sender.mark_pending(position);
        true
    }
    pub fn send_next(
        &mut self,
        world: &mut WorldBackend,
        current_player_chunk: (i32, i32),
        ready: impl FnMut((i32, i32)) -> Result<bool>,
    ) -> Result<usize> {
        self.send_next_with_lighting(world, None, current_player_chunk, ready)
    }
    /// Native live transport supplies its persistent light owner. The caller
    /// processes pending source edits and publishes their light updates before
    /// this phase; packet construction must not consume those updates silently.
    /// None retains the separate legacy packet-only transport behavior.
    pub fn send_next_with_lighting(
        &mut self,
        world: &mut WorldBackend,
        lighting: Option<&mut NativeRuntimeLighting>,
        current_player_chunk: (i32, i32),
        ready: impl FnMut((i32, i32)) -> Result<bool>,
    ) -> Result<usize> {
        if self.view.is_none() {
            return Ok(0);
        }
        let mut host = PacketBatchHost {
            world,
            lighting,
            center: current_player_chunk,
            ready,
            packets: vec![],
            positions: vec![],
        };
        let count = self.sender.send_next(&mut host)?;
        if count != 0 {
            self.scheduled.extend(host.positions);
            self.publish_packets(host.packets);
        }
        Ok(count)
    }
    pub fn publish_packets(&mut self, packets: Vec<Packet>) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("chunk transport revision exhausted");
        self.outbox.push_back(ChunkTransportUpdate {
            revision: self.revision,
            packets,
            loaded: self.scheduled.clone(),
            pending: self.sender.pending_positions().collect(),
        });
    }
    pub fn latest_revision(&self) -> u64 {
        self.revision
    }
    /// A wake may release only its own immutable prefix, never future updates.
    pub fn drain_through(&mut self, cutoff: u64) -> Vec<ChunkTransportUpdate> {
        let mut updates = vec![];
        while self
            .outbox
            .front()
            .is_some_and(|update| update.revision <= cutoff)
        {
            updates.push(self.outbox.pop_front().expect("checked front"));
        }
        updates
    }
    #[cfg(test)]
    pub fn drain(&mut self) -> Vec<ChunkTransportUpdate> {
        self.outbox.drain(..).collect()
    }
}
struct PacketDropHost<'a> {
    alive: bool,
    packets: &'a mut Vec<Packet>,
}
impl ChunkDropHost for PacketDropHost<'_> {
    fn player_alive(&mut self) -> Result<bool> {
        Ok(self.alive)
    }
    fn unload_chunk(&mut self, position: (i32, i32)) -> Result<()> {
        self.packets.push(Packet {
            id: 0x21,
            payload: encoded(|writer| {
                // Protocol767 writes Z then X, both signed fixed-width i32.
                writer.i32(position.1).i32(position.0);
            }),
        });
        Ok(())
    }
}
struct PacketBatchHost<'a, F> {
    world: &'a mut WorldBackend,
    lighting: Option<&'a mut NativeRuntimeLighting>,
    center: (i32, i32),
    ready: F,
    packets: Vec<Packet>,
    positions: Vec<(i32, i32)>,
}
impl<F: FnMut((i32, i32)) -> Result<bool>> ChunkBatchHost for PacketBatchHost<'_, F> {
    type Chunk = Vec<u8>;
    fn player_chunk_position(&mut self) -> Result<(i32, i32)> {
        Ok(self.center)
    }
    fn ready_chunk(&mut self, requested: (i32, i32)) -> Result<Option<ReadyChunk<Vec<u8>>>> {
        if !(self.ready)(requested)? {
            return Ok(None);
        }
        // Actual current Block/sendSync readiness is supplied by the manager;
        // the older packet-only transport slice supplies its separate admission.
        let payload = match (&mut *self.world, self.lighting.as_deref_mut()) {
            (WorldBackend::Fixture(flat), Some(lighting)) => {
                // Detect missed invalidations too. Pending work is retained for
                // Main's update publication rather than swallowed in collection.
                lighting.synchronize_source(flat)?;
                lighting.require_current(flat)?;
                lighting.chunk_packet(flat, requested.0, requested.1)
            }
            (world, _) => world.chunk_packet(requested.0, requested.1),
        }
        .with_context(|| format!("chunk transport packet at {},{}", requested.0, requested.1))?;
        Ok(Some(ReadyChunk {
            handle: payload,
            actual_position: requested,
        }))
    }
    fn begin_batch(&mut self) -> Result<()> {
        self.packets.push(Packet {
            id: 0x0d,
            payload: vec![],
        });
        Ok(())
    }
    fn send_chunk(&mut self, chunk: ReadyChunk<Vec<u8>>) -> Result<()> {
        self.positions.push(chunk.actual_position);
        self.packets.push(Packet {
            id: 0x27,
            payload: chunk.handle,
        });
        Ok(())
    }
    fn end_batch(&mut self, count: usize) -> Result<()> {
        self.packets.push(Packet {
            id: 0x0c,
            payload: encoded(|writer| {
                writer.varint(count as i32);
            }),
        });
        Ok(())
    }
}
