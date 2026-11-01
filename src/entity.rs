//! Java 1.21 (protocol 767) player replication packet primitives.
//!
//! This module implements player identity, visible player entities, movement,
//! animations, metadata and removal. It does not implement entity AI, physics,
//! view-distance policy. `PlayerMotionTracker` supplies the unmounted player's
//! vanilla tick cadence and movement suppression; velocity/passengers are
//! separate channels. The
//! owner must call it at the appropriate simulation boundaries and scope which
//! observers track each player.
//!
//! Quantization is grounded in the exact 1.21 `aig` (VecDeltaCodec), `abv`
//! (AddEntity), `afw` (TeleportEntity) and `aqt` (ServerEntity) bytecode.

use std::collections::BTreeMap;

use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};

use crate::protocol::{Packet, Writer};

pub const PLAYER_ENTITY_TYPE: i32 = 128;
pub const PLAYER_INFO_ADD_ALL: u8 = 0x3f;
const MAX_PROFILE_PROPERTIES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerProfile {
    pub uuid: [u8; 16],
    pub name: String,
    #[serde(default)]
    pub properties: Vec<ProfileProperty>,
}

impl PlayerProfile {
    pub fn offline(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        validate_name(&name)?;
        ensure!(
            name.bytes().all(|b| (33..=126).contains(&b)),
            "invalid LoginStart profile name"
        );
        let mut uuid = md5::compute(format!("OfflinePlayer:{name}").as_bytes()).0;
        uuid[6] = (uuid[6] & 0x0f) | 0x30;
        uuid[8] = (uuid[8] & 0x3f) | 0x80;
        Ok(Self {
            uuid,
            name,
            properties: Vec::new(),
        })
    }

    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name)?;
        ensure!(
            self.properties.len() <= MAX_PROFILE_PROPERTIES,
            "too many profile properties"
        );
        for property in &self.properties {
            ensure!(
                property.name.encode_utf16().count() <= 64,
                "profile property name exceeds Java wire bound"
            );
            ensure!(
                property.value.encode_utf16().count() <= 32767,
                "profile property value exceeds Java wire bound"
            );
            if let Some(signature) = &property.signature {
                ensure!(
                    signature.encode_utf16().count() <= 1024,
                    "profile property signature exceeds Java wire bound"
                );
            }
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<()> {
    ensure!(
        name.encode_utf16().count() <= 16,
        "invalid Minecraft profile name"
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl Position {
    pub fn validate(self) -> Result<()> {
        ensure!(
            [self.x, self.y, self.z].iter().all(|v| v.is_finite())
                && self.yaw.is_finite()
                && self.pitch.is_finite(),
            "non-finite entity position or rotation"
        );
        ensure!(
            self.x.abs() <= 30_000_000.0
                && self.z.abs() <= 30_000_000.0
                && self.y.abs() <= 20_000_000.0,
            "entity coordinates exceed supported Java bounds"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSession {
    pub uuid: [u8; 16],
    pub expires_at: i64,
    pub public_key: Vec<u8>,
    pub key_signature: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerMetadata {
    /// Vanilla shared flags: fire 0x01, sneak 0x02, sprint 0x08, swimming
    /// 0x10, invisible 0x20, glowing 0x40 and fall flying 0x80.
    pub flags: u8,
    /// Vanilla Pose ID: standing0, fallflying1, sleeping2, swimming3,
    /// spinattack4, crouching5; the remaining IDs are other entity poses.
    pub pose: u8,
    pub skin_parts: u8,
    /// 0 is left; 1 is right.
    pub main_hand: u8,
}

/// Boolean inputs read by Player.updatePlayerPose. Collision fit checks are
/// supplied by the world and include blocks and collidable entities.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerPoseInput {
    pub fall_flying: bool,
    pub sleeping: bool,
    pub swimming: bool,
    pub auto_spin_attack: bool,
    pub shift_key_down: bool,
    pub flying: bool,
    pub spectator: bool,
    pub passenger: bool,
}

/// Exact vanilla pose precedence and fallback. Fit callbacks use each pose's
/// entity dimensions at the current position, deflated by 1e-7 on all axes.
pub fn resolve_player_pose(
    current: u8,
    input: PlayerPoseInput,
    mut fits: impl FnMut(u8) -> bool,
) -> u8 {
    if !fits(3) {
        return current;
    }
    let candidate = if input.fall_flying {
        1
    } else if input.sleeping {
        2
    } else if input.swimming {
        3
    } else if input.auto_spin_attack {
        4
    } else if input.shift_key_down && !input.flying {
        5
    } else {
        0
    };
    if input.spectator || input.passenger || fits(candidate) {
        candidate
    } else if fits(5) {
        5
    } else {
        3
    }
}

impl Default for PlayerMetadata {
    fn default() -> Self {
        Self {
            flags: 0,
            pose: 0,
            skin_parts: 0,
            main_hand: 1,
        }
    }
}

fn default_camera_is_self() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    pub entity_id: i32,
    pub profile: PlayerProfile,
    pub position: Position,
    pub gamemode: i32,
    /// Exact relation used by ServerPlayer.broadcastToPlayer: camera == this.
    #[serde(default = "default_camera_is_self")]
    pub camera_is_self: bool,
    /// Client tracking ranges of every indirect passenger, including nested riders.
    #[serde(default)]
    pub indirect_passenger_tracking_ranges_chunks: Vec<i32>,
    pub listed: bool,
    pub latency: i32,
    /// A complete anonymous network NBT text component, including its tag ID.
    /// Preserve the caller's validated component bytes without converting them
    /// through JSON or dropping styles/unknown component data.
    pub display_name: Option<Vec<u8>>,
    pub chat_session: Option<ChatSession>,
    pub metadata: PlayerMetadata,
    /// Complete inherited Entity/LivingEntity/Player data. Text fixture JSON
    /// keeps its existing basic schema; runtime clones preserve rich values.
    #[serde(skip, default = "crate::metadata::PlayerEntityData::new_player")]
    pub entity_data: crate::metadata::PlayerEntityData,
    #[serde(default)]
    pub equipment: Box<crate::equipment::PlayerEquipment>,
}

impl PlayerState {
    pub fn new(entity_id: i32, profile: PlayerProfile, position: Position) -> Result<Self> {
        let value = Self {
            entity_id,
            profile,
            position,
            gamemode: 1,
            camera_is_self: true,
            indirect_passenger_tracking_ranges_chunks: Vec::new(),
            listed: true,
            latency: 0,
            display_name: None,
            chat_session: None,
            metadata: PlayerMetadata::default(),
            entity_data: crate::metadata::PlayerEntityData::new_player(),
            equipment: Box::default(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(self.entity_id > 0, "player entity ID must be positive");
        self.profile.validate()?;
        self.position.validate()?;
        self.equipment.validate()?;
        ensure!((0..=3).contains(&self.gamemode), "invalid player game mode");
        ensure!(self.metadata.main_hand <= 1, "invalid player main hand");
        ensure!(self.metadata.pose <= 17, "invalid player pose");
        if let Some(component) = &self.display_name {
            ensure!(
                !component.is_empty() && component.len() <= 1_048_576,
                "invalid display-name NBT size"
            );
        }
        if let Some(session) = &self.chat_session {
            ensure!(
                session.public_key.len() <= 512 && session.key_signature.len() <= 4096,
                "chat session key or signature too large"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum EntityEvent {
    Spawn(Box<PlayerState>),
    ChatSession { entity_id: i32, uuid: [u8; 16], session: Option<ChatSession> },
    Move {
        entity_id: i32,
        previous: Position,
        current: Position,
    },
    Animation {
        entity_id: i32,
        animation: u8,
    },
    Metadata {
        entity_id: i32,
        metadata: PlayerMetadata,
    },
    Equipment {
        entity_id: i32,
        equipment: Box<crate::equipment::PlayerEquipment>,
        packets: Vec<Packet>,
        changes: Vec<crate::equipment::EquipmentChange>,
    },
    EntityData {
        entity_id: i32,
        data: crate::metadata::PlayerEntityData,
        packet: Packet,
    },
    Remove {
        entity_id: i32,
        uuid: [u8; 16],
    },
}

impl EntityEvent {
    pub fn entity_id(&self) -> i32 {
        match self {
            Self::Spawn(player) => player.entity_id,
            Self::Move { entity_id, .. }
            | Self::ChatSession { entity_id, .. }
            | Self::Animation { entity_id, .. }
            | Self::Metadata { entity_id, .. }
            | Self::EntityData { entity_id, .. }
            | Self::Equipment { entity_id, .. }
            | Self::Remove { entity_id, .. } => *entity_id,
        }
    }

    /// Entity tracking never sends the observer a second copy of their own
    /// player. Send self tab-list updates separately with player_info_update.
    pub fn packets_for(&self, observer_entity_id: i32) -> Result<Vec<Packet>> {
        if self.entity_id() == observer_entity_id
            && !matches!(self, Self::Metadata { .. } | Self::EntityData { .. } | Self::ChatSession { .. })
        {
            return Ok(Vec::new());
        }
        match self {
            Self::Spawn(player) => spawn_packets(player),
            Self::ChatSession { uuid, session, .. } => Ok(vec![chat_session_packet(*uuid, session.as_ref())?]),
            Self::Move {
                entity_id,
                previous,
                current,
            } => movement_packets(*entity_id, *previous, *current),
            Self::Animation {
                entity_id,
                animation,
            } => Ok(vec![animation_packet(*entity_id, *animation)?]),
            Self::Metadata {
                entity_id,
                metadata,
            } => Ok(vec![metadata_complete_packet(*entity_id, *metadata)?]),
            Self::EntityData { packet, .. } => Ok(vec![packet.clone()]),
            Self::Equipment { packets, .. } => Ok(packets.clone()),
            Self::Remove { entity_id, uuid } => Ok(vec![
                destroy_entities(&[*entity_id])?,
                remove_player_info(&[*uuid])?,
            ]),
        }
    }
}

/// Synchronization belongs to the server: take one lock to subscribe/snapshot/
/// register and publish its event before another task can change this state.
#[derive(Debug, Default)]
pub struct EntityRegistry {
    players: BTreeMap<i32, PlayerState>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn snapshot(&self) -> Vec<PlayerState> {
        self.players.values().cloned().collect()
    }
    pub fn len(&self) -> usize {
        self.players.len()
    }
    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }
    pub fn get(&self, id: i32) -> Option<&PlayerState> {
        self.players.get(&id)
    }
    pub fn get_by_uuid(&self, uuid: &[u8; 16]) -> Option<&PlayerState> {
        self.players.values().find(|p| &p.profile.uuid == uuid)
    }
    pub fn get_by_name(&self, name: &str) -> Option<&PlayerState> {
        self.players
            .values()
            .find(|p| p.profile.name.eq_ignore_ascii_case(name))
    }
    pub fn set_chat_session(&mut self, id: i32, session: Option<ChatSession>) -> Result<Option<EntityEvent>> {
        if let Some(value) = &session {
            ensure!(value.public_key.len() <= 512 && value.key_signature.len() <= 4096, "chat session exceeds wire limits");
        }
        let player = self.players.get_mut(&id).ok_or_else(|| anyhow::anyhow!("unknown player chat session owner"))?;
        if player.chat_session == session { return Ok(None); }
        player.chat_session = session.clone();
        Ok(Some(EntityEvent::ChatSession { entity_id: id, uuid: player.profile.uuid, session }))
    }
    pub fn register(&mut self, mut player: PlayerState) -> Result<EntityEvent> {
        apply_metadata(&mut player.entity_data, player.metadata)?;
        player.validate()?;
        ensure!(
            !self.players.contains_key(&player.entity_id),
            "duplicate player entity ID"
        );
        ensure!(
            self.get_by_uuid(&player.profile.uuid).is_none(),
            "duplicate player UUID"
        );
        ensure!(
            self.get_by_name(&player.profile.name).is_none(),
            "duplicate player name"
        );
        self.players.insert(player.entity_id, player.clone());
        Ok(EntityEvent::Spawn(Box::new(player)))
    }
    pub fn update_position(&mut self, id: i32, position: Position) -> Result<Option<EntityEvent>> {
        position.validate()?;
        let player = self
            .players
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown player entity ID"))?;
        let previous = player.position;
        if previous == position {
            return Ok(None);
        }
        player.position = position;
        Ok(Some(EntityEvent::Move {
            entity_id: id,
            previous,
            current: position,
        }))
    }
    /// Store the smoothed RTT; vanilla PlayerList publishes it on its separate
    /// 601-tick cadence rather than sending a PlayerInfo packet per response.
    pub fn set_latency(&mut self, id: i32, latency: i32) -> Result<()> {
        let player = self
            .players
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown player entity ID"))?;
        player.latency = latency;
        Ok(())
    }

    pub fn update_metadata(
        &mut self,
        id: i32,
        metadata: PlayerMetadata,
    ) -> Result<Option<EntityEvent>> {
        ensure!(
            metadata.main_hand <= 1 && metadata.pose <= 17,
            "invalid player metadata"
        );
        let player = self
            .players
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown player entity ID"))?;
        if player.metadata == metadata {
            return Ok(None);
        }
        apply_metadata(&mut player.entity_data, metadata)?;
        player.metadata = metadata;
        Self::take_data_event(player)
    }

    /// Queue any of the 21 inherited player fields for the next simulation
    /// tick. Cloning values preserves ParticleOptions identity.
    pub fn set_entity_data(
        &mut self,
        id: i32,
        field: u8,
        value: crate::metadata::MetadataValue,
        force: bool,
    ) -> Result<bool> {
        use crate::metadata::MetadataValue;
        if field == 18 {
            ensure!(
                matches!(&value,MetadataValue::Byte(hand) if *hand<=1),
                "invalid player main hand"
            );
        }
        let player = self
            .players
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown player entity ID"))?;
        let changed = player.entity_data.set(field, value, force)?;
        player.metadata = basic_metadata(&player.entity_data)?;
        Ok(changed)
    }

    fn take_data_event(player: &mut PlayerState) -> Result<Option<EntityEvent>> {
        Ok(player
            .entity_data
            .pack_dirty(player.entity_id)?
            .map(|packet| EntityEvent::EntityData {
                entity_id: player.entity_id,
                data: player.entity_data.clone(),
                packet,
            }))
    }

    /// Flush all dirty players, including construction-time health, once per
    /// server tick. The returned events must be published under the same lock.
    pub fn drain_entity_data(&mut self) -> Result<Vec<EntityEvent>> {
        let mut events = Vec::new();
        for player in self.players.values_mut() {
            if let Some(event) = Self::take_data_event(player)? {
                events.push(event);
            }
        }
        Ok(events)
    }
    pub fn update_equipment(
        &mut self,
        id: i32,
        inventory: &crate::inventory::PlayerInventory,
    ) -> Result<Option<EntityEvent>> {
        self.update_equipment_state(
            id,
            crate::equipment::PlayerEquipment::from_inventory(inventory)?,
        )
    }

    pub fn update_equipment_state(
        &mut self,
        id: i32,
        equipment: crate::equipment::PlayerEquipment,
    ) -> Result<Option<EntityEvent>> {
        equipment.validate()?;
        let player = self
            .players
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown player entity ID"))?;
        let update = crate::equipment::detect_equipment_changes(id, &player.equipment, &equipment)?;
        *player.equipment = equipment;
        if update.packets.is_empty() {
            return Ok(None);
        }
        Ok(Some(EntityEvent::Equipment {
            entity_id: id,
            equipment: player.equipment.clone(),
            packets: update.packets,
            changes: update.changes,
        }))
    }

    pub fn animation(&self, id: i32, hand: i32) -> Result<EntityEvent> {
        ensure!(self.players.contains_key(&id), "unknown player entity ID");
        let animation = match hand {
            0 => 0,
            1 => 3,
            _ => bail!("invalid animation hand"),
        };
        Ok(EntityEvent::Animation {
            entity_id: id,
            animation,
        })
    }
    pub fn remove(&mut self, id: i32) -> Option<EntityEvent> {
        self.players.remove(&id).map(|p| EntityEvent::Remove {
            entity_id: id,
            uuid: p.profile.uuid,
        })
    }
}

fn packet(id: i32, writer: Writer) -> Packet {
    Packet {
        id,
        payload: writer.into_inner(),
    }
}

/// All six protocol-767 PlayerInfoUpdate actions, with each action's payload
/// emitted only when its bit is present. Unrecognized future action bits fail.
pub fn chat_session_packet(uuid: [u8; 16], session: Option<&ChatSession>) -> Result<Packet> {
    let mut out = Writer::new();
    out.u8(0x02).varint(1).uuid(uuid).bool(session.is_some());
    if let Some(session) = session {
        ensure!(session.public_key.len() <= 512 && session.key_signature.len() <= 4096, "chat session exceeds wire limits");
        out.uuid(session.uuid).i64(session.expires_at)
            .varint(session.public_key.len() as i32).raw(&session.public_key)
            .varint(session.key_signature.len() as i32).raw(&session.key_signature);
    }
    Ok(packet(0x3e, out))
}

pub fn player_info_update(players: &[PlayerState], actions: u8) -> Result<Packet> {
    ensure!(
        actions & !0x3f == 0,
        "unknown protocol-767 player info actions"
    );
    ensure!(players.len() <= 20_000, "too many player info entries");
    let mut out = Writer::new();
    out.u8(actions).varint(players.len() as i32);
    for player in players {
        player.validate()?;
        out.uuid(player.profile.uuid);
        if actions & 0x01 != 0 {
            out.string(&player.profile.name)
                .varint(player.profile.properties.len() as i32);
            for property in &player.profile.properties {
                out.string(&property.name)
                    .string(&property.value)
                    .bool(property.signature.is_some());
                if let Some(signature) = &property.signature {
                    out.string(signature);
                }
            }
        }
        if actions & 0x02 != 0 {
            out.bool(player.chat_session.is_some());
            if let Some(session) = &player.chat_session {
                out.uuid(session.uuid)
                    .i64(session.expires_at)
                    .varint(session.public_key.len() as i32)
                    .raw(&session.public_key)
                    .varint(session.key_signature.len() as i32)
                    .raw(&session.key_signature);
            }
        }
        if actions & 0x04 != 0 {
            out.varint(player.gamemode);
        }
        if actions & 0x08 != 0 {
            out.varint(i32::from(player.listed));
        }
        if actions & 0x10 != 0 {
            out.varint(player.latency);
        }
        if actions & 0x20 != 0 {
            out.bool(player.display_name.is_some());
            if let Some(component) = &player.display_name {
                out.raw(component);
            }
        }
    }
    Ok(packet(0x3e, out))
}

pub fn spawn_packets(player: &PlayerState) -> Result<Vec<Packet>> {
    player.validate()?;
    let p = player.position;
    let mut spawn = Writer::new();
    spawn
        .varint(player.entity_id)
        .uuid(player.profile.uuid)
        .varint(PLAYER_ENTITY_TYPE)
        .f64(p.x)
        .f64(p.y)
        .f64(p.z)
        .i8(angle_byte(p.pitch))
        .i8(angle_byte(p.yaw))
        .i8(angle_byte(p.yaw))
        .varint(0)
        .i16(0)
        .i16(0)
        .i16(0);
    let mut packets = vec![
        player_info_update(std::slice::from_ref(player), PLAYER_INFO_ADD_ALL)?,
        packet(0x01, spawn),
        head_rotation_packet(player.entity_id, p.yaw)?,
    ];
    let mut data = player.entity_data.clone();
    apply_metadata(&mut data, player.metadata)?;
    if let Some(packet) = data.packet_non_defaults(player.entity_id)? {
        packets.push(packet);
    }
    if let Some(packet) = player.equipment.initial_packet(player.entity_id)? {
        packets.push(packet);
    }
    Ok(packets)
}

/// Vanilla VecDeltaCodec uses Math.round, whose negative half ties go toward
/// positive infinity; Rust f64::round would give different negative deltas.
fn coordinate(position: f64) -> i64 {
    let value = position * 4096.0;
    let lower = value.floor();
    // Do not implement Math.round as floor(value + 0.5): the addition itself
    // rounds 0.49999999999999994 + 0.5 to 1.0. Java's implementation avoids it.
    (lower as i64) + i64::from(value - lower >= 0.5)
}

fn angle_units(degrees: f32) -> i32 {
    // Preserve Java's f32 arithmetic, saturating float-to-int cast, and Mth.floor
    // subtraction semantics, including out-of-range finite input.
    let value = degrees * 256.0_f32 / 360.0_f32;
    let truncated = value as i32;
    if value < truncated as f32 {
        truncated.wrapping_sub(1)
    } else {
        truncated
    }
}
pub fn angle_byte(degrees: f32) -> i8 {
    angle_units(degrees) as i8
}
fn teleport_angle_byte(degrees: f32) -> i8 {
    (degrees * 256.0_f32 / 360.0_f32) as i32 as i8
}

pub fn movement_packets(
    entity_id: i32,
    previous: Position,
    current: Position,
) -> Result<Vec<Packet>> {
    ensure!(entity_id > 0, "player entity ID must be positive");
    previous.validate()?;
    current.validate()?;
    let delta = [
        coordinate(current.x) - coordinate(previous.x),
        coordinate(current.y) - coordinate(previous.y),
        coordinate(current.z) - coordinate(previous.z),
    ];
    let moved = delta.iter().any(|v| *v != 0);
    let rotated = angle_units(previous.yaw) != angle_units(current.yaw)
        || angle_units(previous.pitch) != angle_units(current.pitch);
    let mut out = Writer::new();
    out.varint(entity_id);
    let mut packets = Vec::new();
    if delta.iter().any(|v| i16::try_from(*v).is_err()) || previous.on_ground != current.on_ground {
        out.f64(current.x)
            .f64(current.y)
            .f64(current.z)
            .i8(teleport_angle_byte(current.yaw))
            .i8(teleport_angle_byte(current.pitch))
            .bool(current.on_ground);
        packets.push(packet(0x70, out));
    } else if moved {
        out.i16(delta[0] as i16)
            .i16(delta[1] as i16)
            .i16(delta[2] as i16);
        if rotated {
            out.i8(angle_byte(current.yaw))
                .i8(angle_byte(current.pitch));
        }
        out.bool(current.on_ground);
        packets.push(packet(if rotated { 0x2f } else { 0x2e }, out));
    } else if rotated {
        out.i8(angle_byte(current.yaw))
            .i8(angle_byte(current.pitch))
            .bool(current.on_ground);
        packets.push(packet(0x30, out));
    }
    if angle_units(previous.yaw) != angle_units(current.yaw) {
        packets.push(head_rotation_packet(entity_id, current.yaw)?);
    }
    Ok(packets)
}

#[derive(Debug, Clone, Copy)]
pub struct MotionTrackingOptions {
    pub update_interval: i32,
    pub track_delta: bool,
    pub track_head: bool,
}
impl MotionTrackingOptions {
    pub const PLAYER: Self = Self {
        update_interval: 2,
        track_delta: false,
        track_head: true,
    };
    pub const ITEM: Self = Self {
        update_interval: 20,
        track_delta: true,
        track_head: false,
    };
}

/// The shared vanilla `ServerEntity` movement state for one unmounted player.
///
/// Own this once per entity, rather than once per viewer. Call `tick` exactly
/// once per simulation tick; player entity types use an update interval of two.
/// Velocity, passengers and living-entity attributes are separate channels.
#[derive(Debug, Clone)]
pub struct PlayerMotionTracker {
    entity_id: i32,
    position_base: Position,
    last_yaw: i32,
    last_pitch: i32,
    last_head_yaw: i32,
    tick_count: i32,
    teleport_delay: i32,
    was_on_ground: bool,
    options: MotionTrackingOptions,
    last_sent_velocity: crate::geometry::Vec3,
}

/// The same unmounted ServerEntity state with type-specific tracking options.
pub type EntityMotionTracker = PlayerMotionTracker;

impl PlayerMotionTracker {
    pub fn new(entity_id: i32, initial: Position) -> Result<Self> {
        Self::with_options(
            entity_id,
            initial,
            crate::geometry::Vec3::ZERO,
            MotionTrackingOptions::PLAYER,
        )
    }
    pub fn with_options(
        entity_id: i32,
        initial: Position,
        initial_velocity: crate::geometry::Vec3,
        options: MotionTrackingOptions,
    ) -> Result<Self> {
        ensure!(entity_id > 0, "entity ID must be positive");
        initial.validate()?;
        ensure!(
            options.update_interval > 0,
            "tracking interval must be positive"
        );
        ensure!(
            [initial_velocity.x, initial_velocity.y, initial_velocity.z]
                .iter()
                .all(|v| v.is_finite()),
            "non-finite entity velocity"
        );
        Ok(Self {
            entity_id,
            position_base: initial,
            last_yaw: angle_units(initial.yaw),
            last_pitch: angle_units(initial.pitch),
            last_head_yaw: if options.track_head {
                angle_units(initial.yaw)
            } else {
                0
            },
            tick_count: 0,
            teleport_delay: 0,
            was_on_ground: initial.on_ground,
            options,
            last_sent_velocity: initial_velocity,
        })
    }
    pub fn spawn_velocity(&self) -> crate::geometry::Vec3 {
        self.last_sent_velocity
    }

    pub fn tracking_tick_count(&self) -> i32 {
        self.tick_count
    }

    pub fn teleport_delay(&self) -> i32 {
        self.teleport_delay
    }

    /// Position and angles used by vanilla's AddEntity constructor when an
    /// observer begins tracking. Unsent sub-threshold motion is not included.
    pub fn spawn_position(&self) -> Position {
        Position {
            yaw: self.last_yaw.wrapping_mul(360) as f32 / 256.0,
            pitch: self.last_pitch.wrapping_mul(360) as f32 / 256.0,
            on_ground: self.was_on_ground,
            ..self.position_base
        }
    }

    pub fn tick(&mut self, current: Position, force_update: bool) -> Result<Vec<Packet>> {
        ensure!(
            !self.options.track_delta,
            "delta tracking requires tick_with_velocity"
        );
        self.tick_with_velocity(current, self.last_sent_velocity, force_update, false)
    }

    /// Run once per simulation tick. Metadata dirtiness and hasImpulse both
    /// force the interval gate; only hasImpulse forces velocity tracking for
    /// entity types whose normal track_delta option is false.
    pub fn tick_with_velocity(
        &mut self,
        current: Position,
        velocity: crate::geometry::Vec3,
        metadata_dirty: bool,
        has_impulse: bool,
    ) -> Result<Vec<Packet>> {
        let packets =
            self.collect_movement_updates(current, velocity, metadata_dirty, has_impulse)?;
        self.advance_tracking_tick();
        Ok(packets)
    }

    /// Shared sendChanges hosts publish dirty metadata after these movement
    /// packets and before advancing the tracking clock. Call the advance
    /// method exactly once after every successful collection, even if empty.
    pub(crate) fn collect_movement_updates(
        &mut self,
        current: Position,
        velocity: crate::geometry::Vec3,
        metadata_dirty: bool,
        has_impulse: bool,
    ) -> Result<Vec<Packet>> {
        current.validate()?;
        ensure!(
            [velocity.x, velocity.y, velocity.z]
                .iter()
                .all(|v| v.is_finite()),
            "non-finite entity velocity"
        );
        let mut packets = Vec::new();
        if self.tick_count % self.options.update_interval == 0 || metadata_dirty || has_impulse {
            self.teleport_delay = self.teleport_delay.wrapping_add(1);
            if (self.options.track_delta || has_impulse) && self.tick_count > 0 {
                let dx = velocity.x - self.last_sent_velocity.x;
                let dy = velocity.y - self.last_sent_velocity.y;
                let dz = velocity.z - self.last_sent_velocity.z;
                let difference = dx * dx + dy * dy + dz * dz;
                let length =
                    velocity.x * velocity.x + velocity.y * velocity.y + velocity.z * velocity.z;
                if difference > 1.0e-7 || (difference > 0.0 && length == 0.0) {
                    self.last_sent_velocity = velocity;
                    packets.push(velocity_packet(self.entity_id, velocity)?);
                }
            }
            let yaw = angle_units(current.yaw);
            let pitch = angle_units(current.pitch);
            let dx = current.x - self.position_base.x;
            let dy = current.y - self.position_base.y;
            let dz = current.z - self.position_base.z;
            // Vec3.lengthSqr preserves Java evaluation order, without an FMA.
            let moved = dx * dx + dy * dy + dz * dz >= 7.62939453125E-6;
            let send_position = moved || self.tick_count % 60 == 0;
            let send_rotation = yaw.wrapping_sub(self.last_yaw).wrapping_abs() >= 1
                || pitch.wrapping_sub(self.last_pitch).wrapping_abs() >= 1;
            let delta = [
                coordinate(current.x) - coordinate(self.position_base.x),
                coordinate(current.y) - coordinate(self.position_base.y),
                coordinate(current.z) - coordinate(self.position_base.z),
            ];
            let teleport = delta.iter().any(|&d| i16::try_from(d).is_err())
                || self.teleport_delay > 400
                || self.was_on_ground != current.on_ground;
            let mut out = Writer::new();
            out.varint(self.entity_id);
            if teleport {
                out.f64(current.x)
                    .f64(current.y)
                    .f64(current.z)
                    .i8(teleport_angle_byte(current.yaw))
                    .i8(teleport_angle_byte(current.pitch))
                    .bool(current.on_ground);
                packets.push(packet(0x70, out));
                self.teleport_delay = 0;
                self.was_on_ground = current.on_ground;
            } else if send_position && send_rotation {
                out.i16(delta[0] as i16)
                    .i16(delta[1] as i16)
                    .i16(delta[2] as i16)
                    .i8(yaw as i8)
                    .i8(pitch as i8)
                    .bool(current.on_ground);
                packets.push(packet(0x2f, out));
            } else if send_position {
                out.i16(delta[0] as i16)
                    .i16(delta[1] as i16)
                    .i16(delta[2] as i16)
                    .bool(current.on_ground);
                packets.push(packet(0x2e, out));
            } else if send_rotation {
                out.i8(yaw as i8).i8(pitch as i8).bool(current.on_ground);
                packets.push(packet(0x30, out));
            }
            if teleport || send_position {
                self.position_base = current;
            }
            if teleport || send_rotation {
                self.last_yaw = yaw;
                self.last_pitch = pitch;
            }
            let head = angle_units(current.yaw);
            if self.options.track_head && head.wrapping_sub(self.last_head_yaw).wrapping_abs() >= 1
            {
                packets.push(head_rotation_packet(self.entity_id, current.yaw)?);
                self.last_head_yaw = head;
            }
        }
        Ok(packets)
    }

    pub(crate) fn advance_tracking_tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
    }
}

pub fn velocity_packet(entity_id: i32, velocity: crate::geometry::Vec3) -> Result<Packet> {
    ensure!(entity_id > 0, "entity ID must be positive");
    let mut out = Writer::new();
    out.varint(entity_id);
    for value in [velocity.x, velocity.y, velocity.z] {
        out.i16((value.clamp(-3.9, 3.9) * 8000.0) as i32 as i16);
    }
    Ok(packet(0x5a, out))
}

pub fn head_rotation_packet(entity_id: i32, yaw: f32) -> Result<Packet> {
    ensure!(entity_id > 0 && yaw.is_finite(), "invalid head rotation");
    let mut out = Writer::new();
    out.varint(entity_id).i8(angle_byte(yaw));
    Ok(packet(0x48, out))
}
pub(crate) fn basic_metadata(data: &crate::metadata::PlayerEntityData) -> Result<PlayerMetadata> {
    use crate::metadata::MetadataValue;
    let byte = |id| match data.get(id) {
        Some(MetadataValue::Byte(v)) => Ok(*v),
        _ => bail!("invalid player metadata field"),
    };
    let pose = match data.get(6) {
        Some(MetadataValue::Pose(v)) => *v as u8,
        _ => bail!("invalid player pose field"),
    };
    Ok(PlayerMetadata {
        flags: byte(0)?,
        pose,
        skin_parts: byte(17)?,
        main_hand: byte(18)?,
    })
}

pub(crate) fn prepared_entity_data(
    player: &PlayerState,
) -> Result<crate::metadata::PlayerEntityData> {
    let mut data = player.entity_data.clone();
    apply_metadata(&mut data, player.metadata)?;
    data.clear_dirty();
    Ok(data)
}

fn metadata_complete_packet(entity_id: i32, metadata: PlayerMetadata) -> Result<Packet> {
    let mut data = crate::metadata::PlayerEntityData::default();
    apply_metadata(&mut data, metadata)?;
    for id in [0, 6, 17, 18] {
        data.set(id, data.get(id).expect("known basic field").clone(), true)?;
    }
    data.pack_dirty(entity_id)?
        .ok_or_else(|| anyhow::anyhow!("explicit metadata fields unexpectedly empty"))
}

fn apply_metadata(
    data: &mut crate::metadata::PlayerEntityData,
    metadata: PlayerMetadata,
) -> Result<()> {
    use crate::metadata::MetadataValue;
    ensure!(
        metadata.main_hand <= 1 && metadata.pose <= 17,
        "invalid player metadata"
    );
    data.set(0, MetadataValue::Byte(metadata.flags), false)?;
    data.set(6, MetadataValue::Pose(i32::from(metadata.pose)), false)?;
    data.set(17, MetadataValue::Byte(metadata.skin_parts), false)?;
    data.set(18, MetadataValue::Byte(metadata.main_hand), false)?;
    Ok(())
}

/// Initial pairing data uses Java's non-default snapshot, including the
/// constructor's health20 rather than the LivingEntity registration default1.
pub fn metadata_packet(entity_id: i32, metadata: PlayerMetadata) -> Result<Packet> {
    let mut data = crate::metadata::PlayerEntityData::new_player();
    apply_metadata(&mut data, metadata)?;
    data.packet_non_defaults(entity_id)?
        .ok_or_else(|| anyhow::anyhow!("player health snapshot is unexpectedly empty"))
}

/// Only changed fields are sent during normal tracking. Returning to a default
/// must remain in this delta, or clients would keep stale crouch/skin flags.
pub fn metadata_delta_packet(
    entity_id: i32,
    previous: PlayerMetadata,
    current: PlayerMetadata,
) -> Result<Option<Packet>> {
    let mut data = crate::metadata::PlayerEntityData::new_player();
    apply_metadata(&mut data, previous)?;
    data.pack_dirty(entity_id)?;
    apply_metadata(&mut data, current)?;
    data.pack_dirty(entity_id)
}
pub fn animation_packet(entity_id: i32, animation: u8) -> Result<Packet> {
    ensure!(entity_id > 0 && animation <= 5, "invalid entity animation");
    let mut out = Writer::new();
    out.varint(entity_id).u8(animation);
    Ok(packet(0x03, out))
}
pub fn destroy_entities(ids: &[i32]) -> Result<Packet> {
    ensure!(
        ids.len() <= 20_000 && ids.iter().all(|id| *id > 0),
        "invalid destroyed entity IDs"
    );
    let mut out = Writer::new();
    out.varint(ids.len() as i32);
    for id in ids {
        out.varint(*id);
    }
    Ok(packet(0x42, out))
}
pub fn remove_player_info(uuids: &[[u8; 16]]) -> Result<Packet> {
    ensure!(uuids.len() <= 20_000, "too many removed player UUIDs");
    let mut out = Writer::new();
    out.varint(uuids.len() as i32);
    for uuid in uuids {
        out.uuid(*uuid);
    }
    Ok(packet(0x3d, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Reader;
    fn position() -> Position {
        Position {
            x: 8.5,
            y: 65.0,
            z: 8.5,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        }
    }
    fn player(id: i32, name: &str) -> PlayerState {
        PlayerState::new(id, PlayerProfile::offline(name).unwrap(), position()).unwrap()
    }
    #[test]
    fn identity_registry_rejects_duplicates_without_losing_players() {
        let mut registry = EntityRegistry::new();
        let p = player(1, "Alice");
        registry.register(p.clone()).unwrap();
        assert!(registry.register(p).is_err());
        assert!(registry.register(player(2, "ALICE")).is_err());
        let mut duplicate = player(2, "Bob");
        duplicate.profile.uuid = registry.get(1).unwrap().profile.uuid;
        assert!(registry.register(duplicate).is_err());
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get_by_name("alice").unwrap().entity_id, 1);
        assert!(registry.remove(1).is_some());
        assert!(registry.is_empty());
    }
    #[test]
    fn no_observer_receives_its_own_spawn_movement_or_removal() {
        let p = player(1, "Alice");
        let uuid = p.profile.uuid;
        for event in [
            EntityEvent::Spawn(Box::new(p)),
            EntityEvent::Move {
                entity_id: 1,
                previous: position(),
                current: Position {
                    x: 9.0,
                    ..position()
                },
            },
            EntityEvent::Remove { entity_id: 1, uuid },
        ] {
            assert!(event.packets_for(1).unwrap().is_empty());
            assert!(!event.packets_for(2).unwrap().is_empty());
        }
    }
    #[test]
    fn java_negative_half_rounding_and_distinct_rotation_casts_are_preserved() {
        assert_eq!(coordinate(-0.5 / 4096.0), 0);
        assert_eq!(coordinate(-1.5 / 4096.0), -1);
        assert_eq!(coordinate(0.5 / 4096.0), 1);
        assert_eq!(
            coordinate(f64::from_bits(0.5_f64.to_bits() - 1) / 4096.0),
            0
        );
        assert_eq!(angle_byte(-1.0), -1);
        assert_eq!(teleport_angle_byte(-1.0), 0);
        assert_eq!(angle_byte(360.0), 0);
        assert_eq!(angle_byte(-360.0), 0);
        assert_eq!(angle_byte(-f32::MAX), -1);
    }
    #[test]
    fn signed_short_boundaries_and_ground_changes_choose_absolute_teleport() {
        let from = Position {
            x: 0.0,
            ..position()
        };
        let relative = movement_packets(
            1,
            from,
            Position {
                x: 32767.0 / 4096.0,
                ..from
            },
        )
        .unwrap();
        assert_eq!(relative[0].id, 0x2e);
        let negative = movement_packets(1, from, Position { x: -8.0, ..from }).unwrap();
        assert_eq!(negative[0].id, 0x2e);
        for to in [
            Position { x: 8.0, ..from },
            Position {
                x: -8.000244140625,
                ..from
            },
            Position {
                on_ground: false,
                ..from
            },
        ] {
            assert_eq!(movement_packets(1, from, to).unwrap()[0].id, 0x70);
        }
        assert!(movement_packets(1, from, from).unwrap().is_empty());
    }
    #[test]
    fn profile_signatures_chat_keys_and_display_components_are_not_dropped() {
        let mut p = player(128, "Signed");
        p.profile.properties.push(ProfileProperty {
            name: "textures".into(),
            value: "value".into(),
            signature: Some("signature".into()),
        });
        p.chat_session = Some(ChatSession {
            uuid: [7; 16],
            expires_at: 123,
            public_key: vec![1, 2],
            key_signature: vec![3, 4],
        });
        p.display_name = Some(vec![8, 0, 4, b'N', b'a', b'm', b'e']);
        let info = player_info_update(&[p.clone()], 0x3f).unwrap();
        let mut r = Reader::new(&info.payload);
        assert_eq!(r.u8().unwrap(), 0x3f);
        assert_eq!(r.varint().unwrap(), 1);
        assert_eq!(r.uuid().unwrap(), p.profile.uuid);
        assert_eq!(r.string(16).unwrap(), "Signed");
        assert_eq!(r.varint().unwrap(), 1);
        assert_eq!(r.string(32767).unwrap(), "textures");
        assert_eq!(r.string(32767).unwrap(), "value");
        assert!(r.bool().unwrap());
        assert_eq!(r.string(32767).unwrap(), "signature");
        assert!(r.bool().unwrap());
        assert_eq!(r.uuid().unwrap(), [7; 16]);
        assert_eq!(r.i64().unwrap(), 123);
        assert_eq!(r.varint().unwrap(), 2);
        assert_eq!(r.bytes(2).unwrap(), &[1, 2]);
        assert_eq!(r.varint().unwrap(), 2);
        assert_eq!(r.bytes(2).unwrap(), &[3, 4]);
        assert_eq!(r.varint().unwrap(), 1);
        assert_eq!(r.varint().unwrap(), 1);
        assert_eq!(r.varint().unwrap(), 0);
        assert!(r.bool().unwrap());
        assert_eq!(r.remaining(), p.display_name.as_ref().unwrap());
        assert!(player_info_update(&[p], 0x40).is_err());
    }
    #[test]
    fn invalid_positions_do_not_mutate_registered_state() {
        let mut registry = EntityRegistry::new();
        registry.register(player(1, "Alice")).unwrap();
        for p in [
            Position {
                x: f64::NAN,
                ..position()
            },
            Position {
                yaw: f32::INFINITY,
                ..position()
            },
            Position {
                x: 30_000_001.0,
                ..position()
            },
        ] {
            assert!(registry.update_position(1, p).is_err());
            assert_eq!(registry.get(1).unwrap().position, position());
        }
        assert!(registry.animation(1, 2).is_err());
        assert!(registry.update_position(99, position()).is_err());
        assert!(registry.update_position(1, position()).unwrap().is_none());
    }
    #[test]
    fn exact_wire_bytes_match_independent_prismarine_serializer() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tools/entity-fixtures.json")).unwrap();
        assert_eq!(fixture["protocol"], 767);
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 23);
        for case in cases {
            let input = &case["input"];
            let decode_player =
                || serde_json::from_value::<PlayerState>(input["player"].clone()).unwrap();
            let packets = match case["kind"].as_str().unwrap() {
                "spawn" => spawn_packets(&decode_player()).unwrap(),
                "info" => vec![player_info_update(
                    &[decode_player()],
                    input["actions"].as_u64().unwrap() as u8,
                )
                .unwrap()],
                "movement" => movement_packets(
                    input["id"].as_i64().unwrap() as i32,
                    serde_json::from_value(input["previous"].clone()).unwrap(),
                    serde_json::from_value(input["current"].clone()).unwrap(),
                )
                .unwrap(),
                "animation" => vec![animation_packet(
                    input["id"].as_i64().unwrap() as i32,
                    input["animation"].as_u64().unwrap() as u8,
                )
                .unwrap()],
                "remove" => EntityEvent::Remove {
                    entity_id: input["id"].as_i64().unwrap() as i32,
                    uuid: serde_json::from_value(input["uuid"].clone()).unwrap(),
                }
                .packets_for(-1)
                .unwrap(),
                other => panic!("unknown fixture kind {other}"),
            };
            let expected = case["expected"].as_array().unwrap();
            assert_eq!(
                packets.len(),
                expected.len(),
                "{} packet count",
                case["name"]
            );
            for (observed, wanted) in packets.iter().zip(expected) {
                assert_eq!(
                    observed.id,
                    wanted["id"].as_i64().unwrap() as i32,
                    "{} packetID",
                    case["name"]
                );
                assert_eq!(
                    observed.payload,
                    STANDARD
                        .decode(wanted["payload"].as_str().unwrap())
                        .unwrap(),
                    "{} packet{}",
                    case["name"],
                    observed.id
                );
            }
        }
    }
    #[test]
    fn exact_profile_codec_bounds_and_login_names_are_preserved() {
        assert!(PlayerProfile::offline("").is_ok());
        assert!(PlayerProfile::offline("A!b~").is_ok());
        assert!(PlayerProfile::offline("with space").is_err());
        let mut p = player(1, "Alice");
        let property = ProfileProperty {
            name: "n".repeat(64),
            value: "v".repeat(32767),
            signature: Some("s".repeat(1024)),
        };
        p.profile.properties = vec![property; 16];
        assert!(p.validate().is_ok());
        p.profile.properties.push(p.profile.properties[0].clone());
        assert!(p.validate().is_err());
        p.profile.properties.truncate(16);
        p.profile.properties[0].name.push('x');
        assert!(p.validate().is_err());
        p.profile.properties[0].name.pop();
        p.profile.properties[0]
            .signature
            .as_mut()
            .unwrap()
            .push('x');
        assert!(p.validate().is_err());
    }
    #[test]
    fn quantization_matches_actual_java_classes_at_boundaries_and_random_inputs() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tools/entity-quantization.json")).unwrap();
        assert_eq!(
            fixture["jar_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 2071);
        for (index, case) in cases.iter().enumerate() {
            let expected = case["expected"].as_i64().unwrap();
            let bits = case["bits"].as_str().unwrap();
            let observed = match case["kind"].as_str().unwrap() {
                "coordinate" => coordinate(f64::from_bits(u64::from_str_radix(bits, 16).unwrap())),
                "angle" => i64::from(angle_byte(f32::from_bits(
                    u32::from_str_radix(bits, 16).unwrap(),
                ))),
                "teleport_angle" => i64::from(teleport_angle_byte(f32::from_bits(
                    u32::from_str_radix(bits, 16).unwrap(),
                ))),
                other => panic!("unknown quantization oracle kind {other}"),
            };
            assert_eq!(observed, expected, "Java oracle case{index}: {case}");
        }
    }
    #[test]
    fn shared_motion_cadence_matches_actual_server_entity_bytecode() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tools/entity-motion.json")).unwrap();
        assert_eq!(
            fixture["jar_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 2559);
        let mut tracker = None;
        for (index, case) in cases.iter().enumerate() {
            let bits = &case["bits"];
            let double = |key: &str| {
                f64::from_bits(u64::from_str_radix(bits[key].as_str().unwrap(), 16).unwrap())
            };
            let float = |key: &str| {
                f32::from_bits(u32::from_str_radix(bits[key].as_str().unwrap(), 16).unwrap())
            };
            let position = Position {
                x: double("x"),
                y: double("y"),
                z: double("z"),
                yaw: float("yaw"),
                pitch: float("pitch"),
                on_ground: case["position"]["on_ground"].as_bool().unwrap(),
            };
            let packets = if case["op"] == "new" {
                tracker = Some(PlayerMotionTracker::new(17, position).unwrap());
                Vec::new()
            } else {
                tracker
                    .as_mut()
                    .unwrap()
                    .tick(position, case["force"].as_bool().unwrap())
                    .unwrap()
            };
            let expected = case["expected"]["packets"].as_array().unwrap();
            assert_eq!(packets.len(), expected.len(), "Java tick{index}: {case}");
            for (packet, wanted) in packets.iter().zip(expected) {
                assert_eq!(
                    packet.id,
                    wanted["id"].as_i64().unwrap() as i32,
                    "Java tick{index}"
                );
                assert_eq!(
                    packet.payload,
                    STANDARD
                        .decode(wanted["payload"].as_str().unwrap())
                        .unwrap(),
                    "Java tick{index}"
                );
            }
            let spawn = tracker.as_ref().unwrap().spawn_position();
            let wanted = &case["expected"]["spawn"];
            assert_eq!(
                spawn.x,
                f64::from_bits(u64::from_str_radix(wanted["xbits"].as_str().unwrap(), 16).unwrap()),
                "Java tick{index} x"
            );
            assert_eq!(
                spawn.y,
                f64::from_bits(u64::from_str_radix(wanted["ybits"].as_str().unwrap(), 16).unwrap()),
                "Java tick{index} y"
            );
            assert_eq!(
                spawn.z,
                f64::from_bits(u64::from_str_radix(wanted["zbits"].as_str().unwrap(), 16).unwrap()),
                "Java tick{index} z"
            );
            assert_eq!(
                spawn.yaw,
                wanted["yaw"].as_f64().unwrap() as f32,
                "Java tick{index} yaw"
            );
            assert_eq!(
                spawn.pitch,
                wanted["pitch"].as_f64().unwrap() as f32,
                "Java tick{index} pitch"
            );
        }
    }
    #[test]
    fn metadata_delta_sends_pose_and_resets_without_repeating_default_fields() {
        let previous = PlayerMetadata {
            flags: 2,
            pose: 5,
            ..PlayerMetadata::default()
        };
        let packet = metadata_delta_packet(17, previous, PlayerMetadata::default())
            .unwrap()
            .unwrap();
        assert_eq!(packet.payload, vec![17, 0, 0, 0, 6, 21, 0, 255]);
        assert!(metadata_delta_packet(17, previous, previous)
            .unwrap()
            .is_none());
        let initial = metadata_packet(17, PlayerMetadata::default()).unwrap();
        assert_eq!(initial.payload, vec![17, 9, 3, 0x41, 0xa0, 0, 0, 255]);
    }
    #[test]
    fn pose_precedence_fit_fallback_and_query_order_match_actual_player() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tools/entity-pose.json")).unwrap();
        assert_eq!(
            fixture["jar_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 16384);
        for (index, case) in cases.iter().enumerate() {
            let flags = case["flags"].as_u64().unwrap();
            let mask = case["fits"].as_u64().unwrap();
            let mut calls = Vec::new();
            let input = PlayerPoseInput {
                fall_flying: flags & 1 != 0,
                sleeping: flags & 2 != 0,
                swimming: flags & 4 != 0,
                auto_spin_attack: flags & 8 != 0,
                shift_key_down: flags & 16 != 0,
                flying: flags & 32 != 0,
                spectator: flags & 64 != 0,
                passenger: flags & 128 != 0,
            };
            let pose =
                resolve_player_pose(case["current"].as_u64().unwrap() as u8, input, |pose| {
                    calls.push(u64::from(pose));
                    mask & (1 << pose) != 0
                });
            assert_eq!(
                u64::from(pose),
                case["expected"].as_u64().unwrap(),
                "actualPlayer posecase{index}"
            );
            assert_eq!(
                calls,
                case["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap())
                    .collect::<Vec<_>>(),
                "actualPlayer querycase{index}"
            );
        }
    }
}
