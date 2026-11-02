//! Canonical state for the pinned original `FallingBlockEntity` (`cjg`).
use crate::{
    entity::Position,
    entity_state::EntityPhysicalState,
    geometry::Vec3,
    nbt::Tag,
    placement::BlockPosition,
    protocol::{Packet, Writer},
    random::LegacyRandomSource,
};
use anyhow::{ensure, Result};

/// Entity's additional metadata entry8 is the falling block's start position.
pub const START_POSITION_METADATA_MASK: u16 = 1 << 8;
pub const WIDTH: f32 = 0.98;
pub const HEIGHT: f32 = 0.98;
pub const DEFAULT_GRAVITY: f64 = 0.04;

/// One authoritative physical state, carried state and retained entity RNG.
/// `Clone` is for an enclosing real transaction/snapshot; it grants neither a
/// new identity nor authority to publish a second entity.
#[derive(Clone, Debug)]
pub struct FallingBlockState {
    pub physical: EntityPhysicalState,
    pub random: LegacyRandomSource,
    pub block_state: u32,
    pub time: i32,
    pub drop_item: bool,
    pub cancel_drop: bool,
    pub hurt_entities: bool,
    pub fall_damage_max: i32,
    pub fall_damage_per_distance: f32,
    pub block_data: Option<Tag>,
    pub force_tick_after_teleport_to_duplicate: bool,
    pub start_pos: BlockPosition,
    /// Entity.blocksBuilding, set by the positioned source constructor. This
    /// is distinct from noPhysics and is not present in EntityPhysicalState.
    pub blocks_building: bool,
}

impl std::ops::Deref for FallingBlockState {
    type Target = EntityPhysicalState;
    fn deref(&self) -> &Self::Target { &self.physical }
}
impl std::ops::DerefMut for FallingBlockState {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.physical }
}

impl FallingBlockState {
    /// EntityType factory constructor, used before ordinary Entity.load. The
    /// caller supplies the actual Blocks.SAND.defaultBlockState from its bound
    /// registry. It does not perform the positioned constructor's blocksBuilding
    /// assignment or set a nonzero start position.
    pub fn new_unpositioned(
        entity_id: i32,
        random: LegacyRandomSource,
        default_sand_state: u32,
    ) -> Result<Self> {
        let mut state = Self::new(entity_id, random, default_sand_state, Vec3::ZERO)?;
        state.blocks_building = false;
        Ok(state)
    }

    /// Source positioned constructor. `random` must be the actual freshly
    /// acquired Entity constructor owner BEFORE Entity's UUID samples. No seed
    /// is synthesized here and neither Level.random nor sound RNG is borrowed.
    /// The caller resolves WATERLOGGED=false before entering this constructor.
    pub fn new(
        entity_id: i32,
        mut random: LegacyRandomSource,
        block_state: u32,
        spawn: Vec3,
    ) -> Result<Self> {
        ensure!(entity_id > 0, "falling entity ID must be positive");
        ensure!(
            [spawn.x, spawn.y, spawn.z].iter().all(|v| v.is_finite()),
            "nonfinite falling spawn position"
        );
        // Mth.createInsecureUUID(RandomSource): exactly two nextLong calls.
        let hi = (random.next_i64() as u64 & 0xffff_ffff_ffff_0fff) | 0x4000;
        let lo = (random.next_i64() as u64 & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000;
        let mut uuid = [0; 16];
        uuid[..8].copy_from_slice(&hi.to_be_bytes());
        uuid[8..].copy_from_slice(&lo.to_be_bytes());
        let mut physical = EntityPhysicalState::new(
            entity_id, uuid,
            Position { x: spawn.x, y: spawn.y, z: spawn.z, yaw: 0., pitch: 0., on_ground: false },
            Vec3::ZERO, WIDTH, HEIGHT, HEIGHT * 0.85f32,
        )?;
        physical.old_position = spawn;
        let start_pos = physical.block_position;
        // Source defineSynchedData registers ZERO; setStartPos uses .equals,
        // so a positioned entity whose actual start is ZERO stays clean.
        if start_pos != (0, 0, 0) {
            physical.metadata_dirty |= START_POSITION_METADATA_MASK;
        }
        Ok(Self {
            physical, random, block_state, time: 0, drop_item: true,
            cancel_drop: false, hurt_entities: false, fall_damage_max: 40,
            fall_damage_per_distance: 0., block_data: None,
            force_tick_after_teleport_to_duplicate: false, start_pos,
            blocks_building: true,
        })
    }

    /// Source `setStartPos`, retaining metadata dirtiness on a later reversion.
    pub fn set_start_pos(&mut self, position: BlockPosition) {
        if self.start_pos != position {
            self.start_pos = position;
            self.physical.metadata_dirty |= START_POSITION_METADATA_MASK;
        }
    }
    pub fn set_hurts_entities(&mut self, amount: f32, maximum: i32) {
        self.hurt_entities = true;
        self.fall_damage_per_distance = amount;
        self.fall_damage_max = maximum;
    }
    /// Source disableDrop sets CancelDrop; it does not clear DropItem.
    pub fn disable_drop(&mut self) { self.cancel_drop = true; }
    pub fn is_attackable(&self) -> bool { false }
    pub fn is_pickable(&self) -> bool { !self.physical.removed }
    pub fn movement_emission(&self) -> crate::entity_move::MovementEmission {
        crate::entity_move::MovementEmission::None
    }
    pub fn display_fire_animation(&self) -> bool { false }
    pub fn only_op_can_set_nbt(&self) -> bool { true }

    /// Source AddEntity carries the actual global carried BlockState ID as its
    /// data field. EntityType identity and ServerEntity's captured position /
    /// motion are mandatory caller inputs, not guessed registry constants.
    pub fn spawn_packet_at(
        &self,
        entity_type_id: i32,
        position: Position,
        velocity: Vec3,
    ) -> Result<Packet> {
        self.validate()?;
        position.validate()?;
        ensure!(entity_type_id >= 0, "unknown falling EntityType ID");
        ensure!(self.block_state <= i32::MAX as u32, "falling BlockState ID overflow");
        ensure!([velocity.x, velocity.y, velocity.z].iter().all(|v| v.is_finite()),
            "nonfinite captured falling motion");
        let mut out = Writer::new();
        out.varint(self.entity_id).uuid(self.uuid).varint(entity_type_id)
            .f64(position.x).f64(position.y).f64(position.z)
            .i8(crate::entity::angle_byte(position.pitch))
            .i8(crate::entity::angle_byte(position.yaw)).i8(0)
            .varint(self.block_state as i32);
        for component in [velocity.x, velocity.y, velocity.z] {
            out.i16((component.clamp(-3.9, 3.9) * 8000.) as i16);
        }
        Ok(Packet { id: 0x01, payload: out.into_inner() })
    }

    pub fn metadata_packet(&self) -> Result<Option<Packet>> {
        let mask = self.generic_non_default_mask()
            | if self.start_pos != (0, 0, 0) { START_POSITION_METADATA_MASK } else { 0 };
        self.dirty_metadata_packet(mask)
    }

    /// Encode an already globally drained dirty mask; never clear another
    /// observer's data here. BLOCK_POS is mandatory serializer10, not optional11.
    pub fn dirty_metadata_packet(&self, mask: u16) -> Result<Option<Packet>> {
        self.validate()?;
        let entries = self.generic_metadata_values(mask & 0xff);
        if entries.is_empty() && mask & START_POSITION_METADATA_MASK == 0 {
            return Ok(None);
        }
        let mut out = Writer::new();
        out.varint(self.entity_id);
        for (id, value) in entries {
            out.u8(id).varint(value.serializer()).raw(&value.payload()?);
        }
        if mask & START_POSITION_METADATA_MASK != 0 {
            let (x, y, z) = self.start_pos;
            let packed = ((i64::from(x) & 0x3ffffff) << 38)
                | ((i64::from(z) & 0x3ffffff) << 12) | (i64::from(y) & 0xfff);
            out.u8(8).varint(10).i64(packed);
        }
        out.u8(255);
        Ok(Some(Packet { id: 0x58, payload: out.into_inner() }))
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(self.entity_id > 0, "invalid falling entity ID");
        ensure!(self.width == WIDTH && self.height == HEIGHT && self.eye_height == HEIGHT * 0.85f32,
            "invalid native falling entity dimensions");
        self.position.validate()?;
        ensure!([self.velocity.x, self.velocity.y, self.velocity.z].iter().all(|v| v.is_finite()),
            "nonfinite falling velocity");
        ensure!(self.block_data.as_ref().is_none_or(|tag| matches!(tag, Tag::Compound(_))),
            "falling TileEntityData must be a compound");
        Ok(())
    }
}
