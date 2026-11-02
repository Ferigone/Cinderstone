//! Canonical FallingBlock objects and their real shared index/sender owners.
//! The world/addFreshEntity admission and literal tick callbacks are separate
//! mandatory capabilities; this store never creates an ItemState substitute.
use super::*;
use mc121_rust::{
    entity::{EntityMotionTracker, MotionTrackingOptions},
    entity_spatial::{self, SectionKey},
    entity_state::EntityPhysicalState,
    falling_block::{state::FallingBlockState, persistence::FallingBlockSnapshot},
    runtime_entity_index::{EntityHandle, EntityKind},
};

// Pinned original bootstrap: minecraft:falling_block == bsx.O, registry ID40,
// range10, update20, trackDeltas=true. Nonliving pairing sends explicit motion.
const ENTITY_TYPE_ID: i32 = 40;
const TRACKING_RANGE: i32 = 10 * 16;

/// One actual constructed object, including after an unsuccessful world add.
/// Moving its Box into the registered map preserves the object's address.
pub(super) struct ConstructedFallingBlock {
    id: i32,
    unregistered: Option<Box<FallingBlockState>>,
    registered: Option<EntityHandle>,
}
impl ConstructedFallingBlock {
    pub(super) fn new(state: FallingBlockState) -> Result<Self> {
        state.validate()?;
        ensure!(!state.removed, "cannot construct an already removed falling actor");
        Ok(Self { id: state.entity_id, unregistered: Some(Box::new(state)), registered: None })
    }
    pub(super) fn restore(saved: FallingBlockSnapshot) -> Result<Self> {
        Self::new(saved.restore()?)
    }
    pub(super) fn entity_id(&self) -> i32 { self.id }
    pub(super) fn registered_handle(&self) -> Option<EntityHandle> { self.registered }
}

/// Replication observations, not a second physical state or RNG. Keeping this
/// field separate lets a real host borrow canonical state during callbacks.
pub(super) struct FallingBlockPublication {
    pub(super) handle: EntityHandle,
    tracker: EntityMotionTracker,
    metadata: Option<Packet>,
    seen: BTreeSet<i32>,
    last_section: SectionKey,
}
pub(super) struct NativeFallingBlock {
    pub(super) state: Box<FallingBlockState>,
    pub(super) publication: FallingBlockPublication,
    insertion: u64,
}
#[derive(Default)]
pub(super) struct MainFallingBlockRuntime {
    entries: BTreeMap<i32, NativeFallingBlock>,
    next_insertion: u64,
}

fn emit_falling(events: &broadcast::Sender<PlayerEvent>, recipients: Vec<i32>, packets: Vec<Packet>) {
    if !recipients.is_empty() && !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(recipients), packets: Arc::new(packets),
        });
    }
}

impl MainFallingBlockRuntime {
    pub(super) fn restore_constructed(&self, saved: FallingBlockSnapshot) -> Result<ConstructedFallingBlock> {
        ConstructedFallingBlock::restore(saved)
    }
    /// The caller additionally certifies real UUID/world admission before its
    /// constructor entropy. This method reserves/publishes neither IDs nor RNG.
    pub(super) fn preflight_registration(&self, entities: &RuntimeEntities, id: i32) -> Result<()> {
        ensure!(id > 0 && !self.entries.contains_key(&id), "falling actor ID already owned");
        self.next_insertion.checked_add(1).context("falling insertion order exhausted")?;
        entities.validate_entity_registration_batch(std::iter::once(id))
    }

    /// Invoke only for an accepted actual world-add. If that add returns false,
    /// retain the unregistered token and still configure its same real object.
    pub(super) fn register_constructed(&mut self, entities: &mut RuntimeEntities,
        object: &mut ConstructedFallingBlock) -> Result<EntityHandle> {
        ensure!(object.registered.is_none(), "falling object was already registered");
        self.preflight_registration(entities, object.id)?;
        let state = object.unregistered.as_ref().context("constructed falling object is absent")?;
        state.validate()?;
        ensure!(state.entity_id == object.id && !state.removed, "constructed falling identity changed");
        let tracker = EntityMotionTracker::with_options(object.id, state.position, state.velocity,
            MotionTrackingOptions { update_interval: 20, track_delta: true, track_head: false })?;
        state.spawn_packet_at(ENTITY_TYPE_ID, tracker.spawn_position(), tracker.spawn_velocity())?;
        let metadata = state.metadata_packet()?;
        let last_section = entity_spatial::section_at(state.position_vec());
        let insertion = self.next_insertion.checked_add(1).context("falling insertion order exhausted")?;
        let handle = entities.register_falling_block(object.id, state.position_vec())?;
        let state = object.unregistered.take().expect("validated same constructed falling Box");
        self.entries.insert(object.id, NativeFallingBlock { state,
            publication: FallingBlockPublication { handle, tracker, metadata, seen: BTreeSet::new(), last_section }, insertion });
        self.next_insertion = insertion;
        object.registered = Some(handle);
        Ok(handle)
    }

    pub(super) fn object_state_mut<'a>(&'a mut self, object: &'a mut ConstructedFallingBlock)
        -> Result<&'a mut FallingBlockState> {
        if let Some(handle) = object.registered {
            let entry = self.entries.get_mut(&object.id).context("registered falling object missing")?;
            ensure!(entry.publication.handle == handle, "stale falling object generation");
            Ok(&mut entry.state)
        } else {
            Ok(object.unregistered.as_deref_mut().context("unregistered falling object missing")?)
        }
    }
    pub(super) fn object_state<'a>(&'a self, object: &'a ConstructedFallingBlock)
        -> Result<&'a FallingBlockState> {
        if let Some(handle) = object.registered {
            let entry = self.entries.get(&object.id).context("registered falling object missing")?;
            ensure!(entry.publication.handle == handle, "stale falling object generation");
            Ok(&entry.state)
        } else {
            Ok(object.unregistered.as_deref().context("unregistered falling object missing")?)
        }
    }
    pub(super) fn contains_uuid(&self, uuid: &[u8; 16]) -> bool {
        self.entries.values().any(|entry| !entry.state.removed && &entry.state.uuid == uuid)
    }
    pub(super) fn get(&self, handle: EntityHandle) -> Option<&NativeFallingBlock> {
        self.entries.get(&handle.id).filter(|entry|
            entry.publication.handle == handle && !entry.state.removed)
    }
    pub(super) fn get_mut(&mut self, handle: EntityHandle) -> Option<&mut NativeFallingBlock> {
        self.entries.get_mut(&handle.id).filter(|entry|
            entry.publication.handle == handle && !entry.state.removed)
    }
    pub(super) fn physical(&self, handle: EntityHandle) -> Option<&EntityPhysicalState> {
        self.get(handle).map(|entry| &entry.state.physical)
    }
    pub(super) fn physical_states(&self) -> impl Iterator<Item = &EntityPhysicalState> {
        self.entries.values().filter(|entry| !entry.state.removed).map(|entry| &entry.state.physical)
    }
    pub(super) fn registered(&self) -> impl Iterator<Item = (EntityHandle, &FallingBlockState)> {
        self.entries.values().filter(|entry| !entry.state.removed)
            .map(|entry| (entry.publication.handle, &*entry.state))
    }
    pub(super) fn max_entity_id(&self) -> Option<i32> { self.entries.keys().next_back().copied() }
    pub(super) fn count(&self) -> usize { self.entries.values().filter(|entry| !entry.state.removed).count() }
    pub(super) fn snapshot(&self) -> Result<Vec<FallingBlockSnapshot>> {
        let mut entries = self.entries.values().filter(|entry| !entry.state.removed).collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.insertion);
        entries.into_iter().map(|entry| FallingBlockSnapshot::capture(&entry.state)).collect()
    }
    /// Actual discard has already removed index/tick/sender ownership. Delay
    /// Box destruction until source callbacks using that removed actor finish.
    pub(super) fn reap_removed(&mut self) { self.entries.retain(|_, entry| !entry.state.removed); }
    pub(super) fn publish_one(&mut self, handle: EntityHandle, in_entity_ticking_range: bool, registry: &EntityRegistry,
        observers: &BTreeMap<i32, TrackingObserver>, transports: &BTreeMap<i32, SessionChunkTransport>,
        events: &broadcast::Sender<PlayerEvent>) -> Result<()> {
        let Some(entry) = self.get_mut(handle) else { return Ok(()); };
        entry.publication.publish(&mut entry.state, in_entity_ticking_range, registry, observers, transports, events)
    }
}

impl FallingBlockPublication {
    /// ChunkMap.broadcast uses the actual tracked audience captured by this
    /// sender owner, without a second spatial/distance query.
    pub(super) fn broadcast_tracked(&self, events: &broadcast::Sender<PlayerEvent>, packet: Packet) {
        emit_falling(events, self.seen.iter().copied().collect(), vec![packet]);
    }
    pub(super) fn position_changed(&mut self, state: &EntityPhysicalState,
        entities: &mut RuntimeEntities) -> Result<()> {
        ensure!(state.entity_id == self.handle.id, "falling position callback actor changed");
        if !state.removed { entities.move_falling_block(self.handle, state.position_vec())?; }
        Ok(())
    }
    pub(super) fn discard(&mut self, state: &mut FallingBlockState, entities: &mut RuntimeEntities,
        events: &broadcast::Sender<PlayerEvent>) -> Result<()> {
        ensure!(state.entity_id == self.handle.id, "falling discard actor changed");
        entities.remove_falling_block(self.handle)?;
        state.physical.removed = true;
        emit_falling(events, std::mem::take(&mut self.seen).into_iter().collect(),
            vec![mc121_rust::experience::remove_packet(&[self.handle.id])]);
        Ok(())
    }
    fn publish(&mut self, state: &mut FallingBlockState, in_entity_ticking_range: bool, registry: &EntityRegistry,
        observers: &BTreeMap<i32, TrackingObserver>, transports: &BTreeMap<i32, SessionChunkTransport>,
        events: &broadcast::Sender<PlayerEvent>) -> Result<()> {
        state.validate()?;
        ensure!(state.entity_id == self.handle.id, "falling sender actor changed");
        let position = state.position_vec();
        let chunk = ((position.x.floor() as i32).div_euclid(16), (position.z.floor() as i32).div_euclid(16));
        let mut next = BTreeSet::new();
        for (&id, observer) in observers {
            let target = entity_tracking::TrackingTarget { entity_id: self.handle.id, position,
                base_range_blocks: TRACKING_RANGE, indirect_passenger_ranges_chunks: &[], broadcast_allowed: true };
            if registry.get(id).is_some() && transports.get(&id).is_some_and(|transport| transport.entity_chunk_sent(chunk))
                && entity_tracking::within_tracking_range(*observer, target) { next.insert(id); }
        }
        for id in self.seen.difference(&next) {
            emit_falling(events, vec![*id], vec![mc121_rust::experience::remove_packet(&[self.handle.id])]);
        }
        for id in next.difference(&self.seen) {
            let mut packets = vec![state.spawn_packet_at(ENTITY_TYPE_ID,
                self.tracker.spawn_position(), self.tracker.spawn_velocity())?];
            if let Some(packet) = &self.metadata { packets.push(packet.clone()); }
            packets.push(entity::velocity_packet(self.handle.id, self.tracker.spawn_velocity())?);
            emit_falling(events, vec![*id], packets);
        }
        self.seen = next;
        // ChunkMap.tick calls ServerEntity.sendChanges only for a section
        // crossing or an actual entity-ticking-range chunk. Observer pairing
        // has its own callbacks and must not advance this movement clock.
        let section = entity_spatial::section_at(position);
        let changed_section = section != self.last_section;
        self.last_section = section;
        if !changed_section && !in_entity_ticking_range { return Ok(()); }
        let dirty = state.metadata_dirty;
        let mut packets = self.tracker.tick_with_velocity(state.position, state.velocity, dirty != 0, state.has_impulse)?;
        if dirty != 0 {
            if let Some(packet) = state.dirty_metadata_packet(dirty)? { packets.push(packet); }
            self.metadata = state.metadata_packet()?;
        }
        if state.hurt_marked { packets.push(entity::velocity_packet(self.handle.id, state.velocity)?); }
        state.physical.metadata_dirty = 0;
        state.physical.has_impulse = false;
        state.physical.hurt_marked = false;
        emit_falling(events, self.seen.iter().copied().collect(), packets);
        Ok(())
    }
}
