//! Per-observer player replication with revisioned snapshot/event reconciliation.
//!
//! The registry owner publishes updates in revision order while holding its
//! registry lock. A full snapshot may overtake an older queued tail of that
//! event stream; the snapshot barrier and entity revisions prevent rollback.
//! Movement deltas always use this observer's last sent baseline, never the
//! global previous position stored in the broadcast event.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{ensure, Result};

use crate::{
    entity::{self, EntityEvent, PlayerState, Position},
    entity_tracking::{
        self, ChunkTrackingGate, TrackingDecision, TrackingObserver, TrackingTarget,
    },
    protocol::Packet,
};

#[derive(Debug, Clone)]
pub struct EntityUpdate {
    pub revision: u64,
    pub event: EntityEvent,
}

#[derive(Debug, Clone)]
struct KnownPlayer {
    player: PlayerState,
    revision: u64,
    /// Some only when an actual Add Entity has been sent to this observer.
    sent_position: Option<Position>,
    sent_entity_data: Option<crate::metadata::PlayerEntityData>,
    sent_equipment: Option<crate::equipment::PlayerEquipment>,
    /// Ignore animations that predate the observer's latest Add Entity.
    visible_since: u64,
    animation_revision: Option<u64>,
    motion_revision: u64,
}

#[derive(Debug)]
pub struct PlayerTracker {
    observer_id: i32,
    observer: Option<TrackingObserver>,
    pending_chunks: BTreeSet<(i32, i32)>,
    known: BTreeMap<i32, KnownPlayer>,
    snapshot_revision: Option<u64>,
    // Removed identities need tombstones until a full snapshot supersedes
    // their revision. Periodic full sync also bounds a long-lived observer's
    // history under repeated player joins/leaves.
    removed: BTreeMap<i32, u64>,
    scheduled_motion: bool,
    spawn_positions: BTreeMap<i32, (u64, Position)>,
}

impl PlayerTracker {
    pub fn set_observer(&mut self, observer: TrackingObserver) -> Result<()> {
        ensure!(
            observer.entity_id == self.observer_id,
            "tracking observer identity mismatch"
        );
        self.observer = Some(observer);
        Ok(())
    }
    pub fn set_pending_chunks(&mut self, pending: BTreeSet<(i32, i32)>) {
        self.pending_chunks = pending;
    }
    fn required_observer(&self) -> Result<TrackingObserver> {
        self.observer
            .ok_or_else(|| anyhow::anyhow!("tracking observer context is required"))
    }
    pub fn reconcile_view(&mut self, loaded: &BTreeSet<(i32, i32)>) -> Result<Vec<Packet>> {
        self.required_observer()?;
        let mut packets = Vec::new();
        let ids = self.known.keys().copied().collect::<Vec<_>>();
        for id in ids {
            self.reconcile_visibility(id, loaded, &mut packets)?;
        }
        Ok(packets)
    }
    pub fn new(observer_id: i32) -> Self {
        Self {
            observer_id,
            observer: None,
            pending_chunks: BTreeSet::new(),
            known: BTreeMap::new(),
            snapshot_revision: None,
            removed: BTreeMap::new(),
            scheduled_motion: false,
            spawn_positions: BTreeMap::new(),
        }
    }

    /// Use movement packets emitted by the shared per-entity vanilla tracker.
    /// Position events still maintain actual positions and visibility, but no
    /// longer emit immediate relative movement packets.
    pub fn new_scheduled(observer_id: i32) -> Self {
        Self {
            scheduled_motion: true,
            ..Self::new(observer_id)
        }
    }

    /// Install the shared ServerEntity baseline before a snapshot or move can
    /// cause this observer to spawn an entity. The actual position remains the
    /// basis for view-window visibility checks.
    pub fn set_spawn_position(
        &mut self,
        revision: u64,
        entity_id: i32,
        position: Position,
    ) -> Result<()> {
        position.validate()?;
        ensure!(entity_id > 0, "player entity ID must be positive");
        if !self
            .spawn_positions
            .get(&entity_id)
            .is_some_and(|(seen, _)| revision < *seen)
        {
            self.spawn_positions.insert(entity_id, (revision, position));
        }
        Ok(())
    }

    /// Route one shared entity's tick output only to viewers currently tracking
    /// that entity. Snapshot barriers prevent applying old queued deltas to a
    /// client already reconciled to a newer global baseline.
    pub fn motion(
        &mut self,
        revision: u64,
        entity_id: i32,
        position: Position,
        packets: &[Packet],
    ) -> Result<Vec<Packet>> {
        self.required_observer()?;
        position.validate()?;
        if self
            .snapshot_revision
            .is_some_and(|barrier| revision <= barrier)
            || self
                .removed
                .get(&entity_id)
                .is_some_and(|barrier| revision <= *barrier)
            || self
                .known
                .get(&entity_id)
                .is_some_and(|p| revision <= p.motion_revision)
        {
            return Ok(Vec::new());
        }
        self.set_spawn_position(revision, entity_id, position)?;
        let Some(known) = self.known.get_mut(&entity_id) else {
            return Ok(Vec::new());
        };
        known.motion_revision = revision;
        if entity_id == self.observer_id || known.sent_position.is_none() {
            return Ok(Vec::new());
        }
        known.sent_position = Some(position);
        Ok(packets.to_vec())
    }

    pub fn visible_entity_ids(&self) -> BTreeSet<i32> {
        self.known
            .iter()
            .filter_map(|(&id, p)| p.sent_position.map(|_| id))
            .collect()
    }

    /// Apply a complete, atomic registry snapshot and reconcile visibility to
    /// the observer's currently loaded chunks. An older snapshot never rolls
    /// back an entity already advanced by a newer event.
    pub fn sync(
        &mut self,
        revision: u64,
        players: &[PlayerState],
        loaded: &BTreeSet<(i32, i32)>,
    ) -> Result<Vec<Packet>> {
        self.required_observer()?;
        let mut ids = BTreeSet::new();
        let mut uuids = BTreeSet::new();
        for player in players {
            player.validate()?;
            ensure!(
                ids.insert(player.entity_id),
                "duplicate entity in player snapshot"
            );
            ensure!(
                uuids.insert(player.profile.uuid),
                "duplicate UUID in player snapshot"
            );
        }
        let mut packets = Vec::new();
        if !self
            .snapshot_revision
            .is_some_and(|barrier| revision < barrier)
        {
            let absent: Vec<_> = self
                .known
                .iter()
                .filter_map(|(&id, p)| (!ids.contains(&id) && p.revision <= revision).then_some(id))
                .collect();
            for id in absent {
                self.remove_known(id, &mut packets)?;
            }
            for player in players {
                if self
                    .known
                    .get(&player.entity_id)
                    .is_some_and(|p| p.revision > revision)
                    || self
                        .removed
                        .get(&player.entity_id)
                        .is_some_and(|removed| *removed > revision)
                {
                    continue;
                }
                self.upsert(revision, player.clone(), loaded, &mut packets)?;
            }
            self.snapshot_revision = Some(
                self.snapshot_revision
                    .map_or(revision, |old| old.max(revision)),
            );
            let barrier = self.snapshot_revision.expect("snapshot just installed");
            self.removed.retain(|_, removed| *removed > barrier);
        }
        // Even a stale registry snapshot can accompany a new view window.
        // Reconcile that window using our newer cached entity state.
        let known_ids: Vec<_> = self.known.keys().copied().collect();
        for id in known_ids {
            self.reconcile_visibility(id, loaded, &mut packets)?;
        }
        if self.scheduled_motion {
            // A lag-recovery snapshot can skip previously queued motion. Put
            // every visible client at the shared codec base before discarding
            // those deltas, including rotation-only changes.
            for (&id, known) in &mut self.known {
                if known.motion_revision > revision {
                    continue;
                }
                if let Some(&(base_revision, base)) = self.spawn_positions.get(&id) {
                    if base_revision > revision {
                        continue;
                    }
                    if id != self.observer_id
                        && known.sent_position.is_some_and(|sent| sent != base)
                    {
                        packets.push(baseline_teleport(id, base));
                        known.sent_position = Some(base);
                    }
                }
                known.motion_revision = revision;
            }
        }
        Ok(packets)
    }

    pub fn event(
        &mut self,
        update: &EntityUpdate,
        loaded: &BTreeSet<(i32, i32)>,
    ) -> Result<Vec<Packet>> {
        self.required_observer()?;
        let id = update.event.entity_id();
        let mut packets = Vec::new();
        // A full snapshot represents state, not transient actions. An already
        // visible player's queued arm animation may still be delivered once,
        // even if sync coalesced later position/metadata state in the meantime.
        if let EntityEvent::Animation {
            entity_id,
            animation,
        } = &update.event
        {
            if let Some(player) = self.known.get_mut(entity_id) {
                if player.sent_position.is_some()
                    && update.revision > player.visible_since
                    && !player
                        .animation_revision
                        .is_some_and(|seen| update.revision <= seen)
                {
                    packets.push(entity::animation_packet(*entity_id, *animation)?);
                    player.animation_revision = Some(update.revision);
                }
            }
            return Ok(packets);
        }
        if self
            .snapshot_revision
            .is_some_and(|barrier| update.revision <= barrier)
            || self
                .known
                .get(&id)
                .is_some_and(|p| update.revision <= p.revision)
            || self
                .removed
                .get(&id)
                .is_some_and(|removed| update.revision <= *removed)
        {
            return Ok(packets);
        }
        match &update.event {
            EntityEvent::Spawn(player) => {
                self.upsert(update.revision, (**player).clone(), loaded, &mut packets)?
            }
            EntityEvent::Move { current, .. } => {
                current.validate()?;
                if let Some(player) = self.known.get_mut(&id) {
                    player.player.position = *current;
                    player.revision = update.revision;
                    self.reconcile_visibility(id, loaded, &mut packets)?;
                }
            }
            EntityEvent::Metadata { metadata, .. } => {
                ensure!(
                    metadata.main_hand <= 1 && metadata.pose <= 17,
                    "invalid player metadata"
                );
                if let Some(player) = self.known.get_mut(&id) {
                    player.player.metadata = *metadata;
                    player.revision = update.revision;
                    self.reconcile_visibility(id, loaded, &mut packets)?;
                }
            }
            EntityEvent::EntityData { data, packet, .. } => {
                let metadata = entity::basic_metadata(data)?;
                ensure!(
                    metadata.main_hand <= 1 && metadata.pose <= 17,
                    "invalid player metadata"
                );
                if let Some(known) = self.known.get_mut(&id) {
                    known.player.entity_data = data.clone();
                    known.player.metadata = metadata;
                    known.revision = update.revision;
                    if id == self.observer_id || known.sent_position.is_some() {
                        packets.push(packet.clone());
                        known.sent_entity_data = Some(entity::prepared_entity_data(&known.player)?);
                    }
                    self.reconcile_visibility(id, loaded, &mut packets)?;
                }
            }
            EntityEvent::Equipment {
                equipment,
                packets: shared,
                ..
            } => {
                equipment.validate()?;
                if let Some(known) = self.known.get_mut(&id) {
                    known.player.equipment = equipment.clone();
                    known.revision = update.revision;
                    if id != self.observer_id && known.sent_position.is_some() {
                        packets.extend_from_slice(shared);
                        known.sent_equipment = Some((**equipment).clone());
                    }
                    self.reconcile_visibility(id, loaded, &mut packets)?;
                }
            }
            EntityEvent::Remove { .. } => {
                self.remove_known(id, &mut packets)?;
                self.removed.insert(id, update.revision);
            }
            EntityEvent::Animation { .. } => unreachable!("handled separately"),
        }
        Ok(packets)
    }

    fn upsert(
        &mut self,
        revision: u64,
        player: PlayerState,
        loaded: &BTreeSet<(i32, i32)>,
        packets: &mut Vec<Packet>,
    ) -> Result<()> {
        player.validate()?;
        let id = player.entity_id;
        if self
            .known
            .get(&id)
            .is_some_and(|old| old.player.profile.uuid != player.profile.uuid)
        {
            self.remove_known(id, packets)?;
        }
        if let Some(known) = self.known.get_mut(&id) {
            let actions = info_changes(&known.player, &player);
            if actions != 0 {
                packets.push(entity::player_info_update(
                    std::slice::from_ref(&player),
                    actions,
                )?);
            }
            known.player = player;
            known.revision = revision;
        } else {
            packets.push(entity::player_info_update(
                std::slice::from_ref(&player),
                entity::PLAYER_INFO_ADD_ALL,
            )?);
            self.known.insert(
                id,
                KnownPlayer {
                    player,
                    revision,
                    sent_position: None,
                    sent_entity_data: None,
                    sent_equipment: None,
                    visible_since: revision,
                    animation_revision: None,
                    motion_revision: revision,
                },
            );
        }
        self.removed.remove(&id);
        self.reconcile_visibility(id, loaded, packets)
    }

    fn reconcile_visibility(
        &mut self,
        id: i32,
        loaded: &BTreeSet<(i32, i32)>,
        packets: &mut Vec<Packet>,
    ) -> Result<()> {
        let observer = self.required_observer()?;
        let Some(known) = self.known.get_mut(&id) else {
            return Ok(());
        };
        let data = entity::prepared_entity_data(&known.player)?;
        if id == self.observer_id {
            let update = if let Some(previous) = &known.sent_entity_data {
                data.difference_packet(id, previous)?
            } else {
                data.packet_non_defaults(id)?
            };
            if let Some(packet) = update {
                packets.push(packet);
            }
            known.sent_entity_data = Some(data);
            return Ok(());
        }
        let position = known.player.position;
        let chunk = (
            (position.x.floor() as i32).div_euclid(16),
            (position.z.floor() as i32).div_euclid(16),
        );
        let target = TrackingTarget {
            entity_id: id,
            position: crate::geometry::Vec3::new(position.x, position.y, position.z),
            base_range_blocks: 32 * 16,
            indirect_passenger_ranges_chunks: &known
                .player
                .indirect_passenger_tracking_ranges_chunks,
            broadcast_allowed: entity_tracking::player_broadcast_to_observer(
                observer.is_spectator,
                known.player.gamemode == 3,
                known.player.camera_is_self,
            ),
        };
        let decision = entity_tracking::tracking_decision(
            observer,
            target,
            ChunkTrackingGate {
                chunk_in_tracking_view: loaded.contains(&chunk),
                pending_to_send: self.pending_chunks.contains(&chunk),
            },
            known.sent_position.is_some(),
        );
        let allowed = match decision {
            TrackingDecision::Pair => true,
            TrackingDecision::Unpair => false,
            TrackingDecision::Unchanged => known.sent_position.is_some(),
        };
        if !allowed {
            if known.sent_position.take().is_some() {
                packets.push(entity::destroy_entities(&[id])?);
                known.sent_entity_data = None;
                known.sent_equipment = None;
            }
            return Ok(());
        }
        if let Some(previous) = known.sent_position {
            if !self.scheduled_motion {
                packets.extend(entity::movement_packets(id, previous, position)?);
                known.sent_position = Some(position);
            }
            let update = if let Some(previous) = &known.sent_entity_data {
                data.difference_packet(id, previous)?
            } else {
                data.packet_non_defaults(id)?
            };
            if let Some(packet) = update {
                packets.push(packet);
            }
            known.sent_entity_data = Some(data);
            let equipment_packets = if let Some(previous) = &known.sent_equipment {
                crate::equipment::detect_equipment_changes(id, previous, &known.player.equipment)?
                    .packets
            } else {
                known
                    .player
                    .equipment
                    .initial_packet(id)?
                    .into_iter()
                    .collect()
            };
            packets.extend(equipment_packets);
            known.sent_equipment = Some((*known.player.equipment).clone());
        } else {
            let mut spawn = known.player.clone();
            if self.scheduled_motion {
                if let Some((revision, position)) = self.spawn_positions.get(&id) {
                    spawn.position = *position;
                    known.motion_revision = known.motion_revision.max(*revision);
                }
            }
            packets.extend(
                entity::spawn_packets(&spawn)?
                    .into_iter()
                    .filter(|p| p.id != 0x3e),
            );
            known.sent_position = Some(spawn.position);
            known.sent_entity_data = Some(data);
            known.sent_equipment = Some((*known.player.equipment).clone());
            known.visible_since = known.revision;
        }
        Ok(())
    }

    fn remove_known(&mut self, id: i32, packets: &mut Vec<Packet>) -> Result<()> {
        self.spawn_positions.remove(&id);
        if let Some(known) = self.known.remove(&id) {
            if known.sent_position.is_some() {
                packets.push(entity::destroy_entities(&[id])?);
            }
            packets.push(entity::remove_player_info(&[known.player.profile.uuid])?);
        }
        Ok(())
    }
}

fn baseline_teleport(entity_id: i32, position: Position) -> Packet {
    let mut out = crate::protocol::Writer::new();
    out.varint(entity_id)
        .f64(position.x)
        .f64(position.y)
        .f64(position.z)
        .i8(entity::angle_byte(position.yaw))
        .i8(entity::angle_byte(position.pitch))
        .bool(position.on_ground);
    Packet {
        id: 0x70,
        payload: out.into_inner(),
    }
}

/// Packet policy supplied by a non-player entity implementation. The shared
/// simulation owns its movement/velocity baselines; this tracker owns only
/// observer visibility, snapshot recovery and queued-event reconciliation.
pub trait ReplicatedEntity: Clone {
    fn entity_id(&self) -> i32;
    fn position(&self) -> Position;
    fn tracking_range_chunks(&self) -> i32;
    /// Return the ranges of all nested passengers; empty only for entities
    /// without passengers. A vehicle implementation must supply the full list.
    fn indirect_passenger_tracking_ranges_chunks(&self) -> Vec<i32> {
        Vec::new()
    }
    fn broadcast_to_observer(&self, _observer: TrackingObserver) -> bool {
        true
    }
    fn validate(&self) -> Result<()> {
        ensure!(self.entity_id() > 0, "entity ID must be positive");
        self.position().validate()
    }
    fn spawn_packets(&self, spawn_position: Position) -> Result<Vec<Packet>>;
    /// A delta-tracked entity's pairing may need the shared ServerEntity
    /// velocity baseline as well as its position baseline. The default keeps
    /// compatibility with entity types that do not track movement velocity.
    fn spawn_packets_with_velocity(
        &self,
        spawn_position: Position,
        _spawn_velocity: Option<crate::geometry::Vec3>,
    ) -> Result<Vec<Packet>> {
        self.spawn_packets(spawn_position)
    }
    fn validate_spawn_velocity(
        &self,
        _spawn_velocity: Option<crate::geometry::Vec3>,
    ) -> Result<()> {
        Ok(())
    }
    fn snapshot_delta(&self, previous: &Self) -> Result<Vec<Packet>>;
}

#[derive(Debug, Clone)]
pub struct EntitySnapshot<T> {
    /// Published tracker-view metadata, with the current physical position for
    /// visibility checks. Hosts must retain metadata from the last shared
    /// publication when the underlying entity has dirty, unsent metadata.
    pub entity: T,
    pub spawn_position: Position,
    /// Last velocity published by the shared entity tracker, rather than its
    /// possibly newer physical velocity. None for types without delta tracking.
    pub spawn_velocity: Option<crate::geometry::Vec3>,
}
impl<T: ReplicatedEntity> EntitySnapshot<T> {
    fn validate(&self) -> Result<()> {
        ensure!(self.entity.entity_id() > 0, "entity ID must be positive");
        self.entity.validate()?;
        self.entity.validate_spawn_velocity(self.spawn_velocity)?;
        self.spawn_position.validate()?;
        if let Some(velocity) = self.spawn_velocity {
            ensure!(
                [velocity.x, velocity.y, velocity.z]
                    .iter()
                    .all(|v| v.is_finite()),
                "non-finite entity velocity baseline"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct VisibleEntity<T> {
    snapshot: EntitySnapshot<T>,
    revision: u64,
    sent_position: Option<Position>,
    visible_since: u64,
    transient_revision: Option<u64>,
}

#[derive(Debug)]
pub struct GenericEntityTracker<T> {
    observer: Option<TrackingObserver>,
    pending_chunks: BTreeSet<(i32, i32)>,
    known: BTreeMap<i32, VisibleEntity<T>>,
    snapshot_revision: Option<u64>,
    removed: BTreeMap<i32, u64>,
}
impl<T> Default for GenericEntityTracker<T> {
    fn default() -> Self {
        Self {
            observer: None,
            pending_chunks: BTreeSet::new(),
            known: BTreeMap::new(),
            snapshot_revision: None,
            removed: BTreeMap::new(),
        }
    }
}
impl<T: ReplicatedEntity> GenericEntityTracker<T> {
    pub fn set_observer(&mut self, observer: TrackingObserver) -> Result<()> {
        ensure!(observer.entity_id > 0, "invalid tracking observer identity");
        ensure!(
            self.observer
                .is_none_or(|old| old.entity_id == observer.entity_id),
            "tracking observer identity changed"
        );
        self.observer = Some(observer);
        Ok(())
    }
    pub fn set_pending_chunks(&mut self, pending: BTreeSet<(i32, i32)>) {
        self.pending_chunks = pending;
    }
    fn required_observer(&self) -> Result<TrackingObserver> {
        self.observer
            .ok_or_else(|| anyhow::anyhow!("tracking observer context is required"))
    }
    pub fn reconcile_view(&mut self, loaded: &BTreeSet<(i32, i32)>) -> Result<Vec<Packet>> {
        let observer = self.required_observer()?;
        let mut packets = Vec::new();
        for known in self.known.values_mut() {
            Self::reconcile(known, loaded, observer, &self.pending_chunks, &mut packets)?;
        }
        Ok(packets)
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn visible_entity_ids(&self) -> BTreeSet<i32> {
        self.known
            .iter()
            .filter_map(|(&id, known)| known.sent_position.map(|_| id))
            .collect()
    }
    fn decision(
        snapshot: &EntitySnapshot<T>,
        loaded: &BTreeSet<(i32, i32)>,
        observer: TrackingObserver,
        pending: &BTreeSet<(i32, i32)>,
        previous: bool,
    ) -> TrackingDecision {
        let position = snapshot.entity.position();
        let chunk = (
            (position.x.floor() as i32).div_euclid(16),
            (position.z.floor() as i32).div_euclid(16),
        );
        let passengers = snapshot.entity.indirect_passenger_tracking_ranges_chunks();
        let target = TrackingTarget {
            entity_id: snapshot.entity.entity_id(),
            position: crate::geometry::Vec3::new(position.x, position.y, position.z),
            base_range_blocks: snapshot.entity.tracking_range_chunks().wrapping_mul(16),
            indirect_passenger_ranges_chunks: &passengers,
            broadcast_allowed: snapshot.entity.broadcast_to_observer(observer),
        };
        entity_tracking::tracking_decision(
            observer,
            target,
            ChunkTrackingGate {
                chunk_in_tracking_view: loaded.contains(&chunk),
                pending_to_send: pending.contains(&chunk),
            },
            previous,
        )
    }
    fn in_view(
        snapshot: &EntitySnapshot<T>,
        loaded: &BTreeSet<(i32, i32)>,
        observer: TrackingObserver,
        pending: &BTreeSet<(i32, i32)>,
    ) -> bool {
        Self::decision(snapshot, loaded, observer, pending, false) == TrackingDecision::Pair
    }
    fn reconcile(
        known: &mut VisibleEntity<T>,
        loaded: &BTreeSet<(i32, i32)>,
        observer: TrackingObserver,
        pending: &BTreeSet<(i32, i32)>,
        packets: &mut Vec<Packet>,
    ) -> Result<()> {
        let id = known.snapshot.entity.entity_id();
        let allowed = match Self::decision(
            &known.snapshot,
            loaded,
            observer,
            pending,
            known.sent_position.is_some(),
        ) {
            TrackingDecision::Pair => true,
            TrackingDecision::Unpair => false,
            TrackingDecision::Unchanged => known.sent_position.is_some(),
        };
        if !allowed {
            if known.sent_position.take().is_some() {
                packets.push(entity::destroy_entities(&[id])?);
            }
        } else if known.sent_position.is_none() {
            packets.extend(known.snapshot.entity.spawn_packets_with_velocity(
                known.snapshot.spawn_position,
                known.snapshot.spawn_velocity,
            )?);
            known.sent_position = Some(known.snapshot.spawn_position);
            known.visible_since = known.revision;
        }
        Ok(())
    }
    fn remove_known(&mut self, id: i32, packets: &mut Vec<Packet>) -> Result<()> {
        if self
            .known
            .remove(&id)
            .is_some_and(|known| known.sent_position.is_some())
        {
            packets.push(entity::destroy_entities(&[id])?);
        }
        Ok(())
    }
    /// A complete, atomic simulation snapshot. All entity inputs are validated
    /// before changing observer state. Visible entities are reconciled to the
    /// shared position base before queued older relative deltas are skipped.
    pub fn sync(
        &mut self,
        revision: u64,
        snapshots: &[EntitySnapshot<T>],
        loaded: &BTreeSet<(i32, i32)>,
    ) -> Result<Vec<Packet>> {
        let observer = self.required_observer()?;
        let mut ids = BTreeSet::new();
        for snapshot in snapshots {
            snapshot.validate()?;
            ensure!(
                ids.insert(snapshot.entity.entity_id()),
                "duplicate entity in snapshot"
            );
        }
        let mut packets = Vec::new();
        if !self
            .snapshot_revision
            .is_some_and(|barrier| revision < barrier)
        {
            let absent: Vec<_> = self
                .known
                .iter()
                .filter_map(|(&id, known)| {
                    (!ids.contains(&id) && known.revision <= revision).then_some(id)
                })
                .collect();
            for id in absent {
                self.remove_known(id, &mut packets)?;
            }
            for snapshot in snapshots {
                let id = snapshot.entity.entity_id();
                if self
                    .known
                    .get(&id)
                    .is_some_and(|known| known.revision > revision)
                    || self
                        .removed
                        .get(&id)
                        .is_some_and(|removed| *removed > revision)
                {
                    continue;
                }
                if !self.known.contains_key(&id)
                    && !Self::in_view(snapshot, loaded, observer, &self.pending_chunks)
                {
                    // A complete state is available again on future view sync
                    // or update. Avoid cloning every far-away world entity
                    // into every observer's cache.
                    continue;
                }
                if let Some(known) = self.known.get_mut(&id) {
                    let previous = known.snapshot.clone();
                    let was_visible = known.sent_position.is_some();
                    known.snapshot = snapshot.clone();
                    known.revision = revision;
                    Self::reconcile(known, loaded, observer, &self.pending_chunks, &mut packets)?;
                    if was_visible && known.sent_position.is_some() {
                        if snapshot.spawn_velocity != previous.spawn_velocity {
                            if let Some(velocity) = snapshot.spawn_velocity {
                                packets.push(entity::velocity_packet(id, velocity)?);
                            }
                        }
                        if known.sent_position != Some(snapshot.spawn_position) {
                            packets.push(baseline_teleport(id, snapshot.spawn_position));
                            known.sent_position = Some(snapshot.spawn_position);
                        }
                        packets.extend(snapshot.entity.snapshot_delta(&previous.entity)?);
                    }
                } else {
                    let mut known = VisibleEntity {
                        snapshot: snapshot.clone(),
                        revision,
                        sent_position: None,
                        visible_since: revision,
                        transient_revision: None,
                    };
                    Self::reconcile(
                        &mut known,
                        loaded,
                        observer,
                        &self.pending_chunks,
                        &mut packets,
                    )?;
                    self.known.insert(id, known);
                }
                self.removed.remove(&id);
            }
            self.snapshot_revision = Some(
                self.snapshot_revision
                    .map_or(revision, |old| old.max(revision)),
            );
            let barrier = self.snapshot_revision.expect("snapshot installed");
            self.removed.retain(|_, removed| *removed > barrier);
        }
        // Older snapshots may still accompany a changed observer view window.
        for known in self.known.values_mut() {
            Self::reconcile(known, loaded, observer, &self.pending_chunks, &mut packets)?;
        }
        Ok(packets)
    }
    /// One shared simulation update: complete latest state plus its exact
    /// emitted packets. A fresh view-entry spawn already incorporates this
    /// baseline, so the queued relative delta is not applied a second time.
    pub fn update(
        &mut self,
        revision: u64,
        snapshot: &EntitySnapshot<T>,
        shared_packets: &[Packet],
        loaded: &BTreeSet<(i32, i32)>,
    ) -> Result<Vec<Packet>> {
        let observer = self.required_observer()?;
        snapshot.validate()?;
        let id = snapshot.entity.entity_id();
        if self
            .snapshot_revision
            .is_some_and(|barrier| revision <= barrier)
            || self
                .known
                .get(&id)
                .is_some_and(|known| revision <= known.revision)
            || self
                .removed
                .get(&id)
                .is_some_and(|barrier| revision <= *barrier)
        {
            return Ok(Vec::new());
        }
        if !self.known.contains_key(&id)
            && !Self::in_view(snapshot, loaded, observer, &self.pending_chunks)
        {
            // Retain only an ordering barrier for never-visible entities.
            self.removed.insert(id, revision);
            return Ok(Vec::new());
        }
        let known = self.known.entry(id).or_insert_with(|| VisibleEntity {
            snapshot: snapshot.clone(),
            revision,
            sent_position: None,
            visible_since: revision,
            transient_revision: None,
        });
        let was_visible = known.sent_position.is_some();
        known.snapshot = snapshot.clone();
        known.revision = revision;
        let mut packets = Vec::new();
        Self::reconcile(known, loaded, observer, &self.pending_chunks, &mut packets)?;
        if was_visible && known.sent_position.is_some() {
            packets.extend_from_slice(shared_packets);
            known.sent_position = Some(snapshot.spawn_position);
        }
        self.removed.remove(&id);
        Ok(packets)
    }
    pub fn remove(&mut self, revision: u64, entity_id: i32) -> Result<Vec<Packet>> {
        ensure!(entity_id > 0, "entity ID must be positive");
        if self
            .snapshot_revision
            .is_some_and(|barrier| revision <= barrier)
            || self
                .known
                .get(&entity_id)
                .is_some_and(|known| revision <= known.revision)
            || self
                .removed
                .get(&entity_id)
                .is_some_and(|barrier| revision <= *barrier)
        {
            return Ok(Vec::new());
        }
        let mut packets = Vec::new();
        self.remove_known(entity_id, &mut packets)?;
        self.removed.insert(entity_id, revision);
        Ok(packets)
    }
    /// Transient effects (for example TakeItemEntity) may survive a newer state
    /// snapshot for an entity that stayed visible, but never a fresh spawn or
    /// an already processed duplicate effect.
    pub fn transient(&mut self, revision: u64, entity_id: i32, packets: &[Packet]) -> Vec<Packet> {
        let Some(known) = self.known.get_mut(&entity_id) else {
            return Vec::new();
        };
        if known.sent_position.is_none()
            || revision <= known.visible_since
            || known
                .transient_revision
                .is_some_and(|seen| revision <= seen)
        {
            return Vec::new();
        }
        known.transient_revision = Some(revision);
        packets.to_vec()
    }
}

fn info_changes(previous: &PlayerState, current: &PlayerState) -> u8 {
    let mut actions = 0;
    if previous.profile != current.profile {
        actions |= 0x01;
    }
    if previous.chat_session != current.chat_session {
        actions |= 0x02;
    }
    if previous.gamemode != current.gamemode {
        actions |= 0x04;
    }
    if previous.listed != current.listed {
        actions |= 0x08;
    }
    if previous.latency != current.latency {
        actions |= 0x10;
    }
    if previous.display_name != current.display_name {
        actions |= 0x20;
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{entity::PlayerProfile, protocol::Reader};
    fn observer(id: i32, x: f64) -> TrackingObserver {
        TrackingObserver {
            entity_id: id,
            position: crate::geometry::Vec3::new(x, 64., 8.5),
            requested_view_distance: 32,
            server_view_distance: 32,
            is_spectator: false,
            tracking_distance_percent: 100,
        }
    }
    fn player_tracker(id: i32) -> PlayerTracker {
        let mut t = PlayerTracker::new(id);
        t.set_observer(observer(id, 8.5)).unwrap();
        t
    }
    fn scheduled_player_tracker(id: i32) -> PlayerTracker {
        let mut t = PlayerTracker::new_scheduled(id);
        t.set_observer(observer(id, 8.5)).unwrap();
        t
    }
    fn generic_tracker<T: ReplicatedEntity>() -> GenericEntityTracker<T> {
        let mut t = GenericEntityTracker::new();
        t.set_observer(observer(1, 8.5)).unwrap();
        t
    }
    fn position(x: f64) -> Position {
        Position {
            x,
            y: 65.0,
            z: 8.5,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        }
    }
    fn player(id: i32, x: f64) -> PlayerState {
        PlayerState::new(
            id,
            PlayerProfile::offline(format!("Player{id}")).unwrap(),
            position(x),
        )
        .unwrap()
    }
    fn chunks() -> BTreeSet<(i32, i32)> {
        [(0, 0)].into_iter().collect()
    }
    fn revision(revision: u64, event: EntityEvent) -> EntityUpdate {
        EntityUpdate { revision, event }
    }
    fn delta(packet: &Packet) -> i16 {
        let mut r = Reader::new(&packet.payload);
        r.varint().unwrap();
        r.i16().unwrap()
    }
    #[test]
    fn late_join_uses_snapshot_baseline_and_skips_older_queued_moves() {
        let mut tracker = player_tracker(1);
        tracker.sync(10, &[player(2, 10.0)], &chunks()).unwrap();
        let old = revision(
            9,
            EntityEvent::Move {
                entity_id: 2,
                previous: position(8.0),
                current: position(9.0),
            },
        );
        assert!(tracker.event(&old, &chunks()).unwrap().is_empty());
        let latest = revision(
            11,
            EntityEvent::Move {
                entity_id: 2,
                previous: position(9.0),
                current: position(10.25),
            },
        );
        let packets = tracker.event(&latest, &chunks()).unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].id, 0x2e);
        assert_eq!(delta(&packets[0]), 1024);
    }
    #[test]
    fn future_snapshot_coalesces_motion_before_older_events_are_dropped() {
        let mut tracker = player_tracker(1);
        tracker.sync(1, &[player(2, 8.0)], &chunks()).unwrap();
        let packets = tracker.sync(10, &[player(2, 10.0)], &chunks()).unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(delta(&packets[0]), 8192);
        assert!(tracker
            .event(
                &revision(
                    8,
                    EntityEvent::Move {
                        entity_id: 2,
                        previous: position(8.0),
                        current: position(9.0)
                    }
                ),
                &chunks()
            )
            .unwrap()
            .is_empty());
        let packets = tracker
            .event(
                &revision(
                    11,
                    EntityEvent::Move {
                        entity_id: 2,
                        previous: position(9.0),
                        current: position(10.25),
                    },
                ),
                &chunks(),
            )
            .unwrap();
        assert_eq!(delta(&packets[0]), 1024);
    }
    #[test]
    fn snapshot_pruning_blocks_queued_older_spawn_and_remove_then_spawn_rollback() {
        let mut tracker = player_tracker(1);
        tracker.sync(1, &[player(2, 8.0)], &chunks()).unwrap();
        let packets = tracker.sync(10, &[], &chunks()).unwrap();
        assert_eq!(
            packets.iter().map(|p| p.id).collect::<Vec<_>>(),
            [0x42, 0x3d]
        );
        assert!(tracker
            .event(
                &revision(2, EntityEvent::Spawn(Box::new(player(2, 8.0)))),
                &chunks()
            )
            .unwrap()
            .is_empty());
        tracker
            .event(
                &revision(11, EntityEvent::Spawn(Box::new(player(2, 8.0)))),
                &chunks(),
            )
            .unwrap();
        tracker
            .event(
                &revision(
                    13,
                    EntityEvent::Remove {
                        entity_id: 2,
                        uuid: player(2, 8.0).profile.uuid,
                    },
                ),
                &chunks(),
            )
            .unwrap();
        assert!(tracker
            .event(
                &revision(12, EntityEvent::Spawn(Box::new(player(2, 8.0)))),
                &chunks()
            )
            .unwrap()
            .is_empty());
    }
    #[test]
    fn view_crossings_keep_tab_identity_and_spawn_from_latest_position() {
        let mut tracker = player_tracker(1);
        let p = player(2, 40.0);
        let first = tracker
            .sync(1, std::slice::from_ref(&p), &chunks())
            .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, 0x3e);
        let window = [(2, 0)].into_iter().collect();
        let entered = tracker.sync(1, std::slice::from_ref(&p), &window).unwrap();
        assert_eq!(
            entered.iter().map(|p| p.id).collect::<Vec<_>>(),
            [1, 0x48, 0x58]
        );
        let left = tracker.sync(1, &[p], &chunks()).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, 0x42);
        let mut moved = player(2, -0.5);
        moved.metadata.flags = 0x20;
        let update = tracker.sync(2, &[moved], &chunks()).unwrap();
        assert!(update.is_empty());
        let negative = [(-1, 0)].into_iter().collect();
        let entered = tracker.sync(2, &[player(2, -0.5)], &negative).unwrap();
        assert_eq!(entered.iter().filter(|p| p.id == 0x3e).count(), 0);
        assert_eq!(entered[0].id, 1);
    }
    #[test]
    fn own_profile_is_listed_but_self_is_never_spawned_moved_or_destroyed() {
        let mut tracker = player_tracker(1);
        let initial = tracker.sync(1, &[player(1, 8.0)], &chunks()).unwrap();
        assert_eq!(initial.iter().filter(|p| p.id == 0x3e).count(), 1);
        assert!(initial.iter().all(|p| p.id != 1));
        let movement = tracker
            .event(
                &revision(
                    2,
                    EntityEvent::Move {
                        entity_id: 1,
                        previous: position(8.0),
                        current: position(9.0),
                    },
                ),
                &chunks(),
            )
            .unwrap();
        assert!(movement.is_empty());
        let gone = tracker.sync(3, &[], &chunks()).unwrap();
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].id, 0x3d);
    }
    #[test]
    fn older_snapshot_does_not_rollback_state_but_can_reconcile_a_new_view() {
        let mut tracker = player_tracker(1);
        tracker.sync(10, &[player(2, 8.0)], &chunks()).unwrap();
        tracker
            .event(
                &revision(
                    12,
                    EntityEvent::Move {
                        entity_id: 2,
                        previous: position(8.0),
                        current: position(40.0),
                    },
                ),
                &chunks(),
            )
            .unwrap();
        let window = [(2, 0)].into_iter().collect();
        let packets = tracker.sync(9, &[player(2, 8.0)], &window).unwrap();
        assert_eq!(packets[0].id, 1);
        let mut read = Reader::new(&packets[0].payload);
        read.varint().unwrap();
        read.uuid().unwrap();
        read.varint().unwrap();
        assert_eq!(read.f64().unwrap(), 40.0);
    }
    #[test]
    fn snapshots_emit_changed_info_and_metadata_and_reject_invalid_duplicate_snapshots_atomically()
    {
        let mut tracker = player_tracker(1);
        let p = player(2, 8.0);
        tracker
            .sync(1, std::slice::from_ref(&p), &chunks())
            .unwrap();
        let mut updated = p.clone();
        updated.latency = 42;
        updated.metadata.flags = 0x20;
        let packets = tracker.sync(2, &[updated], &chunks()).unwrap();
        assert_eq!(
            packets.iter().map(|p| p.id).collect::<Vec<_>>(),
            [0x3e, 0x58]
        );
        assert!(tracker.sync(3, &[p.clone(), p], &chunks()).is_err());
        assert_eq!(tracker.visible_entity_ids(), [2].into_iter().collect());
    }
    #[test]
    fn coalesced_snapshot_does_not_lose_visible_queued_animations_or_replay_duplicates() {
        let mut tracker = player_tracker(1);
        tracker.sync(1, &[player(2, 8.0)], &chunks()).unwrap();
        tracker.sync(10, &[player(2, 9.0)], &chunks()).unwrap();
        let event = revision(
            9,
            EntityEvent::Animation {
                entity_id: 2,
                animation: 0,
            },
        );
        assert_eq!(tracker.event(&event, &chunks()).unwrap()[0].id, 3);
        assert!(tracker.event(&event, &chunks()).unwrap().is_empty());
        let mut fresh = player_tracker(1);
        fresh.sync(10, &[player(2, 9.0)], &chunks()).unwrap();
        assert!(fresh.event(&event, &chunks()).unwrap().is_empty());
    }
    #[test]
    fn scheduled_motion_uses_global_baseline_and_snapshot_barrier() {
        let mut tracker = scheduled_player_tracker(1);
        tracker.set_spawn_position(1, 2, position(8.0)).unwrap();
        tracker.sync(1, &[player(2, 8.0)], &chunks()).unwrap();
        let state = revision(
            2,
            EntityEvent::Move {
                entity_id: 2,
                previous: position(8.0),
                current: position(8.25),
            },
        );
        assert!(tracker.event(&state, &chunks()).unwrap().is_empty());
        let packets = entity::movement_packets(2, position(8.0), position(8.25)).unwrap();
        assert_eq!(
            tracker
                .motion(3, 2, position(8.25), &packets)
                .unwrap()
                .len(),
            1
        );
        assert!(tracker
            .motion(3, 2, position(8.25), &packets)
            .unwrap()
            .is_empty());
        tracker.set_spawn_position(10, 2, position(9.0)).unwrap();
        let sync = tracker.sync(10, &[player(2, 9.0)], &chunks()).unwrap();
        assert_eq!(sync.iter().map(|p| p.id).collect::<Vec<_>>(), [0x70]);
        assert!(tracker
            .motion(9, 2, position(8.5), &packets)
            .unwrap()
            .is_empty());
        tracker.set_spawn_position(9, 2, position(8.5)).unwrap();
        assert!(tracker
            .sync(10, &[player(2, 9.0)], &chunks())
            .unwrap()
            .is_empty());
        let next = entity::movement_packets(2, position(9.0), position(9.25)).unwrap();
        let sent = tracker.motion(11, 2, position(9.25), &next).unwrap();
        assert_eq!(delta(&sent[0]), 1024);
    }

    #[test]
    fn scheduled_view_entry_spawns_from_global_base_without_replaying_old_deltas() {
        let mut tracker = scheduled_player_tracker(1);
        tracker.set_spawn_position(1, 2, position(40.0)).unwrap();
        tracker.sync(1, &[player(2, 40.0)], &chunks()).unwrap();
        tracker.set_spawn_position(3, 2, position(8.0)).unwrap();
        let entered = tracker
            .event(
                &revision(
                    4,
                    EntityEvent::Move {
                        entity_id: 2,
                        previous: position(40.0),
                        current: position(8.001),
                    },
                ),
                &chunks(),
            )
            .unwrap();
        assert_eq!(entered[0].id, 1);
        let mut read = Reader::new(&entered[0].payload);
        read.varint().unwrap();
        read.uuid().unwrap();
        read.varint().unwrap();
        assert_eq!(read.f64().unwrap(), 8.0);
        assert!(tracker.motion(3, 2, position(8.0), &[]).unwrap().is_empty());
        assert!(tracker
            .event(
                &revision(
                    5,
                    EntityEvent::Move {
                        entity_id: 2,
                        previous: position(8.001),
                        current: position(8.002)
                    }
                ),
                &chunks()
            )
            .unwrap()
            .is_empty());
    }

    #[test]
    fn scheduled_motion_never_spawns_self_or_rolls_back_newer_motion_on_old_snapshot() {
        let mut tracker = scheduled_player_tracker(1);
        tracker.set_spawn_position(1, 1, position(8.0)).unwrap();
        tracker.set_spawn_position(1, 2, position(8.0)).unwrap();
        tracker
            .sync(1, &[player(1, 8.0), player(2, 8.0)], &chunks())
            .unwrap();
        let packets = entity::movement_packets(2, position(8.0), position(9.0)).unwrap();
        assert!(tracker
            .motion(2, 1, position(9.0), &packets)
            .unwrap()
            .is_empty());
        tracker.motion(5, 2, position(9.0), &packets).unwrap();
        tracker.set_spawn_position(3, 2, position(8.0)).unwrap();
        assert!(tracker
            .sync(3, &[player(1, 8.0), player(2, 8.0)], &chunks())
            .unwrap()
            .is_empty());
    }
    #[test]
    fn rich_player_data_uses_shared_dirty_packets_and_survives_view_reentry() {
        use crate::metadata::MetadataValue;
        let mut registry = entity::EntityRegistry::new();
        registry.register(player(2, 8.0)).unwrap();
        registry.drain_entity_data().unwrap();
        let mut tracker = scheduled_player_tracker(1);
        tracker.sync(1, &registry.snapshot(), &chunks()).unwrap();
        registry
            .set_entity_data(2, 1, MetadataValue::Int(123), false)
            .unwrap();
        registry
            .set_entity_data(2, 16, MetadataValue::Int(42), false)
            .unwrap();
        let event = registry.drain_entity_data().unwrap().pop().unwrap();
        let EntityEvent::EntityData { packet: shared, .. } = &event else {
            panic!("expected rich metadata");
        };
        let sent = tracker
            .event(&revision(2, event.clone()), &chunks())
            .unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].payload, shared.payload);
        assert!(tracker
            .sync(3, &registry.snapshot(), &chunks())
            .unwrap()
            .is_empty());
        registry
            .set_entity_data(2, 16, MetadataValue::Int(42), true)
            .unwrap();
        let forced = registry.drain_entity_data().unwrap().pop().unwrap();
        assert_eq!(
            tracker.event(&revision(4, forced), &chunks()).unwrap()[0].payload,
            vec![2, 16, 1, 42, 255]
        );
        let empty = BTreeSet::new();
        tracker.sync(5, &registry.snapshot(), &empty).unwrap();
        registry
            .set_entity_data(2, 15, MetadataValue::Float(2.5), false)
            .unwrap();
        let hidden = registry.drain_entity_data().unwrap().pop().unwrap();
        assert!(tracker
            .event(&revision(6, hidden), &empty)
            .unwrap()
            .is_empty());
        let returned = tracker.sync(7, &registry.snapshot(), &chunks()).unwrap();
        let actual = returned.iter().find(|packet| packet.id == 0x58).unwrap();
        let expected = registry
            .get(2)
            .unwrap()
            .entity_data
            .packet_non_defaults(2)
            .unwrap()
            .unwrap();
        assert_eq!(actual.payload, expected.payload);
    }

    #[test]
    fn cloned_rich_particle_lists_do_not_create_spurious_snapshot_updates() {
        use crate::metadata::{MetadataValue, Particle};
        let mut registry = entity::EntityRegistry::new();
        registry.register(player(2, 8.0)).unwrap();
        let particle = Particle::from_wire(&[20, 0, 0xff, 0, 0xff]).unwrap();
        registry
            .set_entity_data(2, 10, MetadataValue::Particles(vec![particle]), false)
            .unwrap();
        registry.drain_entity_data().unwrap();
        let mut tracker = player_tracker(1);
        tracker.sync(1, &registry.snapshot(), &chunks()).unwrap();
        assert!(tracker
            .sync(2, &registry.snapshot(), &chunks())
            .unwrap()
            .is_empty());
        let different_object = Particle::from_wire(&[20, 0, 0xff, 0, 0xff]).unwrap();
        registry
            .set_entity_data(
                2,
                10,
                MetadataValue::Particles(vec![different_object]),
                false,
            )
            .unwrap();
        let update = registry.drain_entity_data().unwrap().pop().unwrap();
        assert_eq!(
            tracker
                .event(&revision(3, update), &chunks())
                .unwrap()
                .len(),
            1
        );
        assert!(tracker
            .sync(4, &registry.snapshot(), &chunks())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn self_receives_rich_metadata_but_never_a_duplicate_entity() {
        use crate::metadata::MetadataValue;
        let mut registry = entity::EntityRegistry::new();
        registry.register(player(1, 8.0)).unwrap();
        registry.drain_entity_data().unwrap();
        let mut tracker = scheduled_player_tracker(1);
        tracker.sync(1, &registry.snapshot(), &chunks()).unwrap();
        registry
            .set_entity_data(1, 9, MetadataValue::Float(3.5), false)
            .unwrap();
        let event = registry.drain_entity_data().unwrap().pop().unwrap();
        let packets = tracker.event(&revision(2, event), &chunks()).unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].id, 0x58);
        assert!(tracker.visible_entity_ids().is_empty());
        let mut metadata = registry.get(1).unwrap().metadata;
        metadata.flags = 2;
        metadata.pose = 5;
        let event = registry.update_metadata(1, metadata).unwrap().unwrap();
        tracker.event(&revision(3, event), &chunks()).unwrap();
        assert_eq!(
            registry.get(1).unwrap().entity_data.get(9),
            Some(&MetadataValue::Float(3.5))
        );
        assert!(tracker
            .sync(4, &registry.snapshot(), &chunks())
            .unwrap()
            .is_empty());
    }
    #[derive(Debug, Clone)]
    struct MockEntity {
        id: i32,
        position: Position,
        marker: u8,
    }
    impl ReplicatedEntity for MockEntity {
        fn entity_id(&self) -> i32 {
            self.id
        }
        fn position(&self) -> Position {
            self.position
        }
        fn tracking_range_chunks(&self) -> i32 {
            32
        }
        fn spawn_packets(&self, base: Position) -> Result<Vec<Packet>> {
            let mut out = crate::protocol::Writer::new();
            out.varint(self.id)
                .uuid([0; 16])
                .varint(0)
                .f64(base.x)
                .f64(base.y)
                .f64(base.z)
                .i8(0)
                .i8(0)
                .i8(0)
                .varint(0)
                .i16(0)
                .i16(0)
                .i16(0);
            let mut packets = vec![Packet {
                id: 1,
                payload: out.into_inner(),
            }];
            packets.extend(crate::metadata::encode_values(
                self.id,
                &[(0, crate::metadata::MetadataValue::Byte(self.marker))],
            )?);
            Ok(packets)
        }
        fn snapshot_delta(&self, previous: &Self) -> Result<Vec<Packet>> {
            Ok(if self.marker != previous.marker {
                crate::metadata::encode_values(
                    self.id,
                    &[(0, crate::metadata::MetadataValue::Byte(self.marker))],
                )?
                .into_iter()
                .collect()
            } else {
                Vec::new()
            })
        }
    }
    fn generic_snapshot(x: f64, base: f64, marker: u8) -> EntitySnapshot<MockEntity> {
        EntitySnapshot {
            entity: MockEntity {
                id: 20,
                position: position(x),
                marker,
            },
            spawn_position: position(base),
            spawn_velocity: None,
        }
    }
    #[test]
    fn generic_snapshots_reconcile_global_baselines_and_skip_overtaken_motion() {
        let mut tracker = generic_tracker();
        let initial = generic_snapshot(8.0, 8.0, 0);
        let first = tracker
            .sync(10, std::slice::from_ref(&initial), &chunks())
            .unwrap();
        assert_eq!(first.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 0x58]);
        let raw = entity::movement_packets(20, position(8.0), position(8.25)).unwrap();
        assert!(tracker
            .update(9, &generic_snapshot(8.25, 8.25, 0), &raw, &chunks())
            .unwrap()
            .is_empty());
        assert_eq!(
            tracker
                .update(11, &generic_snapshot(8.25, 8.25, 0), &raw, &chunks())
                .unwrap()[0]
                .payload,
            raw[0].payload
        );
        let snapshot = generic_snapshot(9.0, 9.0, 2);
        let coalesced = tracker
            .sync(20, std::slice::from_ref(&snapshot), &chunks())
            .unwrap();
        assert_eq!(
            coalesced.iter().map(|p| p.id).collect::<Vec<_>>(),
            [0x70, 0x58]
        );
        assert!(tracker
            .update(19, &generic_snapshot(8.5, 8.5, 0), &raw, &chunks())
            .unwrap()
            .is_empty());
        let next = entity::movement_packets(20, position(9.0), position(9.25)).unwrap();
        let sent = tracker
            .update(21, &generic_snapshot(9.25, 9.25, 2), &next, &chunks())
            .unwrap();
        assert_eq!(delta(&sent[0]), 1024);
        assert!(tracker
            .sync(22, &[snapshot.clone(), snapshot], &chunks())
            .is_err());
        assert_eq!(tracker.visible_entity_ids(), [20].into_iter().collect());
        assert_eq!(tracker.sync(25, &[], &chunks()).unwrap()[0].id, 0x42);
        assert!(tracker
            .update(24, &initial, &[], &chunks())
            .unwrap()
            .is_empty());
        assert_eq!(
            tracker.update(26, &initial, &[], &chunks()).unwrap()[0].id,
            1
        );
        assert_eq!(tracker.remove(28, 20).unwrap()[0].id, 0x42);
        assert!(tracker
            .update(27, &initial, &[], &chunks())
            .unwrap()
            .is_empty());
    }
    #[test]
    fn actual_java_item_pairing_uses_published_position_velocity_and_metadata() {
        use crate::{geometry::Vec3, inventory::ItemStack, item_entity::ItemState};
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/item_pairing.json")).unwrap();
        let loaded = (-6..=6)
            .flat_map(|x| (-6..=6).map(move |z| (x, z)))
            .collect();
        for case in data["cases"].as_array().unwrap() {
            let p: Position = serde_json::from_value(case["position"].clone()).unwrap();
            let velocity = |value: &serde_json::Value| {
                Vec3::new(
                    value[0].as_f64().unwrap(),
                    value[1].as_f64().unwrap(),
                    value[2].as_f64().unwrap(),
                )
            };
            let v = velocity(&case["velocity"]);
            let mut uuid = [0; 16];
            uuid[6] = 0x40;
            uuid[8] = 0x80;
            uuid[15] = 1;
            let name = case["stack"]["id"].as_str().unwrap();
            let item_id = crate::crafting::RecipeBook::vanilla()
                .item_id(name)
                .unwrap();
            let stack =
                ItemStack::new(item_id, case["stack"]["count"].as_i64().unwrap() as i32).unwrap();
            let mut item = ItemState::new(17, uuid, p, v, stack).unwrap();
            item.no_gravity = case["no_gravity"].as_bool().unwrap();
            let motion = entity::EntityMotionTracker::with_options(
                17,
                p,
                v,
                entity::MotionTrackingOptions::ITEM,
            )
            .unwrap();
            if let Some(physical) = case.get("physical_position") {
                item.position = serde_json::from_value(physical.clone()).unwrap();
            }
            if let Some(physical) = case.get("physical_velocity") {
                item.velocity = velocity(physical);
            }
            // The unsent_stack is deliberately excluded: the snapshot contract
            // carries ServerEntity's cached metadata, independently of physics.
            let snapshot = EntitySnapshot {
                entity: item,
                spawn_position: motion.spawn_position(),
                spawn_velocity: Some(motion.spawn_velocity()),
            };
            let actual = generic_tracker().sync(1, &[snapshot], &loaded).unwrap();
            let expected = case["packets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|packet| Packet {
                    id: packet["id"].as_i64().unwrap() as i32,
                    payload: STANDARD
                        .decode(packet["payload"].as_str().unwrap())
                        .unwrap(),
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{}", case["name"]);
        }
        assert_eq!(data["cases"].as_array().unwrap().len(), 240);
    }
    #[test]
    fn item_lag_recovery_and_view_reentry_never_publish_unsent_physical_velocity() {
        use crate::{geometry::Vec3, inventory::ItemStack, item_entity::ItemState};
        let snapshot = |x, base, sent_velocity: f64, physical_velocity: f64| EntitySnapshot {
            entity: ItemState::new(
                17,
                [0; 16],
                position(x),
                Vec3::new(physical_velocity, 0., 0.),
                ItemStack::new(1, 1).unwrap(),
            )
            .unwrap(),
            spawn_position: position(base),
            spawn_velocity: Some(Vec3::new(sent_velocity, 0., 0.)),
        };
        let initial = snapshot(8., 8., 0.1, 9.);
        let mut tracker = generic_tracker();
        let pairing = tracker
            .sync(1, std::slice::from_ref(&initial), &chunks())
            .unwrap();
        assert_eq!(
            pairing.iter().map(|p| p.id).collect::<Vec<_>>(),
            [1, 0x58, 0x5a]
        );
        assert_eq!(
            pairing[2],
            entity::velocity_packet(17, Vec3::new(0.1, 0., 0.)).unwrap()
        );
        let unsent = snapshot(8.1, 8., 0.1, 10.);
        assert!(tracker.sync(10, &[unsent], &chunks()).unwrap().is_empty());
        let latest = snapshot(8.5, 8.25, 0.2, 20.);
        let recovered = tracker
            .sync(20, std::slice::from_ref(&latest), &chunks())
            .unwrap();
        assert_eq!(
            recovered.iter().map(|p| p.id).collect::<Vec<_>>(),
            [0x5a, 0x70]
        );
        assert_eq!(
            recovered[0],
            entity::velocity_packet(17, Vec3::new(0.2, 0., 0.)).unwrap()
        );
        let old = snapshot(8.25, 8.125, 0.15, 15.);
        let old_packet = entity::velocity_packet(17, Vec3::new(0.15, 0., 0.)).unwrap();
        assert!(tracker
            .update(19, &old, &[old_packet], &chunks())
            .unwrap()
            .is_empty());
        let next = snapshot(8.75, 8.5, 0.3, 30.);
        let packet = entity::velocity_packet(17, Vec3::new(0.3, 0., 0.)).unwrap();
        assert_eq!(
            tracker
                .update(21, &next, std::slice::from_ref(&packet), &chunks())
                .unwrap(),
            [packet]
        );
        assert_eq!(
            tracker
                .sync(21, std::slice::from_ref(&next), &BTreeSet::new())
                .unwrap()[0]
                .id,
            0x42
        );
        let reentry = tracker
            .sync(21, std::slice::from_ref(&next), &chunks())
            .unwrap();
        assert_eq!(
            reentry.iter().map(|p| p.id).collect::<Vec<_>>(),
            [1, 0x58, 0x5a]
        );
        assert_eq!(
            reentry[2],
            entity::velocity_packet(17, Vec3::new(0.3, 0., 0.)).unwrap()
        );
        assert!(tracker
            .update(21, &next, &reentry, &chunks())
            .unwrap()
            .is_empty());
        let mut missing = next;
        missing.entity.entity_id = 18;
        missing.spawn_velocity = None;
        assert!(tracker.sync(22, &[initial, missing], &chunks()).is_err());
        assert_eq!(tracker.visible_entity_ids(), [17].into_iter().collect());
    }
    #[test]
    fn generic_view_entry_drops_already_incorporated_relative_deltas() {
        let mut tracker = generic_tracker();
        let outside = generic_snapshot(40.0, 40.0, 0);
        assert!(tracker
            .sync(1, std::slice::from_ref(&outside), &chunks())
            .unwrap()
            .is_empty());
        let entered = generic_snapshot(8.5, 8.0, 1);
        let raw = entity::movement_packets(20, position(7.5), position(8.0)).unwrap();
        let packets = tracker.update(2, &entered, &raw, &chunks()).unwrap();
        assert_eq!(packets.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 0x58]);
        let mut reader = Reader::new(&packets[0].payload);
        reader.varint().unwrap();
        reader.uuid().unwrap();
        reader.varint().unwrap();
        assert_eq!(reader.f64().unwrap(), 8.0);
        let left = tracker.update(3, &outside, &raw, &chunks()).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, 0x42);
        let window = [(2, 0)].into_iter().collect();
        let returned = tracker
            .sync(4, std::slice::from_ref(&outside), &window)
            .unwrap();
        assert_eq!(returned[0].id, 1);
        assert!(returned.iter().all(|packet| packet.id != 0x3e));
        let old = tracker.sync(2, &[entered], &chunks()).unwrap();
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].id, 0x42);
    }
    #[test]
    fn generic_transient_effects_survive_snapshot_coalescing_without_duplicates() {
        let mut tracker = generic_tracker();
        let snapshot = generic_snapshot(8.0, 8.0, 0);
        tracker
            .sync(1, std::slice::from_ref(&snapshot), &chunks())
            .unwrap();
        tracker
            .sync(10, std::slice::from_ref(&snapshot), &chunks())
            .unwrap();
        let effect = vec![entity::animation_packet(20, 0).unwrap()];
        assert_eq!(tracker.transient(9, 20, &effect).len(), 1);
        assert!(tracker.transient(9, 20, &effect).is_empty());
        tracker
            .sync(11, std::slice::from_ref(&snapshot), &BTreeSet::new())
            .unwrap();
        assert!(tracker.transient(12, 20, &effect).is_empty());
        tracker.sync(13, &[snapshot], &chunks()).unwrap();
        assert!(tracker.transient(12, 20, &effect).is_empty());
    }
    #[test]
    fn equipment_initial_pairing_global_swap_and_view_return_are_replicated() {
        use crate::{
            equipment::{EquipmentSlot, PlayerEquipment},
            inventory::ItemStack,
        };
        let mut subject = player(2, 8.0);
        let stone = ItemStack::new(1, 3).unwrap();
        subject
            .equipment
            .set(EquipmentSlot::MainHand, stone.clone());
        let mut registry = entity::EntityRegistry::new();
        registry.register(subject).unwrap();
        registry.drain_entity_data().unwrap();
        let mut tracker = scheduled_player_tracker(1);
        let initial = tracker.sync(1, &registry.snapshot(), &chunks()).unwrap();
        assert_eq!(initial.iter().filter(|packet| packet.id == 0x5b).count(), 1);
        let mut next = PlayerEquipment::default();
        next.set(EquipmentSlot::OffHand, stone.clone());
        let event = registry
            .update_equipment_state(2, next.clone())
            .unwrap()
            .unwrap();
        let sent = tracker.event(&revision(2, event), &chunks()).unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].id, 0x1f);
        assert_eq!(sent[0].payload, vec![0, 0, 0, 2, 55]);
        assert!(tracker
            .sync(3, &registry.snapshot(), &chunks())
            .unwrap()
            .is_empty());
        let empty = BTreeSet::new();
        tracker.sync(4, &registry.snapshot(), &empty).unwrap();
        next.set(EquipmentSlot::Head, stone);
        let event = registry
            .update_equipment_state(2, next.clone())
            .unwrap()
            .unwrap();
        assert!(tracker
            .event(&revision(5, event), &empty)
            .unwrap()
            .is_empty());
        let returned = tracker.sync(6, &registry.snapshot(), &chunks()).unwrap();
        let packet = returned.iter().find(|packet| packet.id == 0x5b).unwrap();
        assert_eq!(
            packet.payload,
            next.initial_packet(2).unwrap().unwrap().payload
        );
    }
    #[test]
    fn equipment_snapshot_overtakes_old_events_without_replaying_hand_swap() {
        use crate::{
            equipment::{EquipmentSlot, PlayerEquipment},
            inventory::ItemStack,
        };
        let stone = ItemStack::new(1, 1).unwrap();
        let mut subject = player(2, 8.0);
        subject
            .equipment
            .set(EquipmentSlot::MainHand, stone.clone());
        let mut registry = entity::EntityRegistry::new();
        registry.register(subject).unwrap();
        registry.drain_entity_data().unwrap();
        let mut tracker = scheduled_player_tracker(1);
        tracker.sync(1, &registry.snapshot(), &chunks()).unwrap();
        let mut next = PlayerEquipment::default();
        next.set(EquipmentSlot::OffHand, stone);
        let old = registry.update_equipment_state(2, next).unwrap().unwrap();
        let snapshot = tracker.sync(10, &registry.snapshot(), &chunks()).unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].id, 0x1f);
        assert!(tracker
            .event(&revision(9, old), &chunks())
            .unwrap()
            .is_empty());
        assert!(tracker
            .sync(11, &registry.snapshot(), &chunks())
            .unwrap()
            .is_empty());
    }
    #[test]
    fn own_equipment_uses_local_inventory_and_never_observer_packets() {
        use crate::{
            equipment::{EquipmentSlot, PlayerEquipment},
            inventory::ItemStack,
        };
        let mut registry = entity::EntityRegistry::new();
        registry.register(player(1, 8.0)).unwrap();
        registry.drain_entity_data().unwrap();
        let mut tracker = scheduled_player_tracker(1);
        tracker.sync(1, &registry.snapshot(), &chunks()).unwrap();
        let mut next = PlayerEquipment::default();
        next.set(EquipmentSlot::MainHand, ItemStack::new(1, 1).unwrap());
        let event = registry.update_equipment_state(1, next).unwrap().unwrap();
        assert!(tracker
            .event(&revision(2, event), &chunks())
            .unwrap()
            .is_empty());
        assert!(tracker
            .sync(3, &registry.snapshot(), &chunks())
            .unwrap()
            .is_empty());
    }
    #[test]
    fn generic_tracker_does_not_clone_never_visible_world_entities() {
        let snapshots: Vec<_> = (1..=1000)
            .map(|id| EntitySnapshot {
                entity: MockEntity {
                    id,
                    position: position(400.0),
                    marker: 0,
                },
                spawn_position: position(400.0),
                spawn_velocity: None,
            })
            .collect();
        let mut tracker = generic_tracker();
        assert!(tracker.sync(1, &snapshots, &chunks()).unwrap().is_empty());
        assert!(tracker.known.is_empty());
        for snapshot in &snapshots {
            assert!(tracker
                .update(2, snapshot, &[], &chunks())
                .unwrap()
                .is_empty());
        }
        assert!(tracker.known.is_empty());
        assert_eq!(tracker.removed.len(), 1000);
        let entered = EntitySnapshot {
            entity: MockEntity {
                id: 1000,
                position: position(8.0),
                marker: 1,
            },
            spawn_position: position(8.0),
            spawn_velocity: None,
        };
        assert!(tracker
            .update(1, &entered, &[], &chunks())
            .unwrap()
            .is_empty());
        assert_eq!(
            tracker.update(3, &entered, &[], &chunks()).unwrap()[0].id,
            1
        );
        assert_eq!(tracker.known.len(), 1);
        tracker.sync(4, &snapshots, &chunks()).unwrap();
        assert!(tracker.removed.is_empty());
    }

    #[test]
    fn observer_context_is_required_before_any_snapshot_mutation() {
        let mut players = PlayerTracker::new(1);
        assert!(players
            .sync(1, &[player(2, 8.5)], &chunks())
            .unwrap_err()
            .to_string()
            .contains("observer context"));
        players.set_observer(observer(1, 8.5)).unwrap();
        assert!(players
            .sync(1, &[player(2, 8.5)], &chunks())
            .unwrap()
            .iter()
            .any(|p| p.id == 1));
        let mut generic = GenericEntityTracker::new();
        assert!(generic
            .sync(1, &[generic_snapshot(8.5, 8.5, 0)], &chunks())
            .is_err());
        generic.set_observer(observer(1, 8.5)).unwrap();
        assert_eq!(
            generic
                .sync(1, &[generic_snapshot(8.5, 8.5, 0)], &chunks())
                .unwrap()[0]
                .id,
            1
        );
    }
    #[test]
    fn tracking_range_and_pending_chunks_gate_already_loaded_entities() {
        let mut tracker = generic_tracker();
        let mut current = observer(1, 0.);
        current.requested_view_distance = 2;
        tracker.set_observer(current).unwrap();
        let loaded = [(2, 0)].into_iter().collect();
        assert_eq!(
            tracker
                .sync(1, &[generic_snapshot(32., 32., 0)], &loaded)
                .unwrap()[0]
                .id,
            1
        );
        assert_eq!(
            tracker
                .sync(2, &[generic_snapshot(32. + 1e-7, 32. + 1e-7, 0)], &loaded)
                .unwrap()[0]
                .id,
            0x42
        );
        current.position.x = 1.;
        tracker.set_observer(current).unwrap();
        assert_eq!(tracker.reconcile_view(&loaded).unwrap()[0].id, 1);
        tracker.set_pending_chunks([(2, 0)].into_iter().collect());
        assert_eq!(tracker.reconcile_view(&loaded).unwrap()[0].id, 0x42);
        tracker.set_pending_chunks(BTreeSet::new());
        assert_eq!(tracker.reconcile_view(&loaded).unwrap()[0].id, 1);
    }
    #[test]
    fn spectator_and_camera_policy_changes_preserve_tab_identity() {
        let mut tracker = player_tracker(1);
        let mut target = player(2, 8.5);
        target.gamemode = 3;
        let first = tracker.sync(1, &[target.clone()], &chunks()).unwrap();
        assert!(first.iter().any(|p| p.id == 0x3e));
        assert!(!first.iter().any(|p| p.id == 1));
        let mut spectator = observer(1, 8.5);
        spectator.is_spectator = true;
        tracker.set_observer(spectator).unwrap();
        assert_eq!(tracker.reconcile_view(&chunks()).unwrap()[0].id, 1);
        target.camera_is_self = false;
        let hidden = tracker.sync(2, &[target.clone()], &chunks()).unwrap();
        assert!(hidden.iter().any(|p| p.id == 0x42));
        assert!(!hidden.iter().any(|p| p.id == 0x3d));
        target.gamemode = 0;
        spectator.is_spectator = false;
        tracker.set_observer(spectator).unwrap();
        assert!(tracker
            .sync(3, &[target], &chunks())
            .unwrap()
            .iter()
            .any(|p| p.id == 1));
    }
}
