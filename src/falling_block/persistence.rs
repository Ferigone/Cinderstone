//! FallingBlockEntity additional Java NBT and full native resumable state.
use super::FallingBlockState;
use crate::{entity_state::EntityPhysicalState, nbt::Tag, placement::BlockPosition, random::LegacyRandomSource};
use anyhow::{ensure, Result};

/// Borrow the actual Level's Block registry and configured block tags. A state
/// ID alone is not a registry identity certificate. The decoder implements
/// NbtUtils.readBlockState, including its source AIR/default/property behavior;
/// a missing native capability must error rather than choose a different state.
pub trait FallingBlockStateCodec {
    fn write_block_state(&mut self, state: u32) -> Result<Tag>;
    fn read_block_state(&mut self, compound: &Tag) -> Result<u32>;
    fn state_is_air(&mut self, state: u32) -> Result<bool>;
    fn state_is_anvil(&mut self, state: u32) -> Result<bool>;
    fn default_sand_state(&mut self) -> Result<u32>;
}

/// Source addAdditionalSaveData only. The enclosing Entity.save/load owner
/// writes physical fields, UUID and dimension data. StartPos, forceTick and
/// constructor random state are NOT additional Java NBT fields.
pub fn add_additional_save_data(
    state: &FallingBlockState,
    output: &mut Tag,
    codec: &mut impl FallingBlockStateCodec,
) -> Result<()> {
    ensure!(matches!(output, Tag::Compound(_)), "falling save needs a compound");
    let block = codec.write_block_state(state.block_state)?;
    ensure!(matches!(&block, Tag::Compound(_)), "block-state codec returned non-compound");
    output.insert("BlockState", block)?;
    output.insert("Time", Tag::Int(state.time))?;
    output.insert("DropItem", Tag::Byte(i8::from(state.drop_item)))?;
    output.insert("HurtEntities", Tag::Byte(i8::from(state.hurt_entities)))?;
    // CompoundTag.putFloat uses FloatTag.valueOf, whose zero singleton turns
    // either sign of zero into +0 already in the logical CompoundTag.
    let amount = if state.fall_damage_per_distance == 0. { 0. } else { state.fall_damage_per_distance };
    output.insert("FallHurtAmount", Tag::Float(amount))?;
    output.insert("FallHurtMax", Tag::Int(state.fall_damage_max))?;
    if let Some(data) = &state.block_data {
        ensure!(matches!(data, Tag::Compound(_)), "TileEntityData is not a compound");
        output.insert("TileEntityData", data.clone())?;
    }
    output.insert("CancelDrop", Tag::Byte(i8::from(state.cancel_drop)))?;
    Ok(())
}

/// Literal source read order and optional-field retention. In particular,
/// absent HurtEntities changes only the boolean for actual anvils; absent
/// DropItem and non-compound TileEntityData preserve the prior object fields.
/// Missing/wrong BlockState is passed as CompoundTag.getCompound's EMPTY.
pub fn read_additional_save_data(
    state: &mut FallingBlockState,
    input: &Tag,
    codec: &mut impl FallingBlockStateCodec,
) -> Result<()> {
    ensure!(matches!(input, Tag::Compound(_)), "falling load needs a compound");
    let empty = Tag::Compound(Vec::new());
    let block = match input.get("BlockState") {
        Some(tag @ Tag::Compound(_)) => tag,
        _ => &empty,
    };
    state.block_state = codec.read_block_state(block)?;
    state.time = get_int(input.get("Time"));
    if is_numeric(input.get("HurtEntities")) {
        state.hurt_entities = get_boolean(input.get("HurtEntities"));
        state.fall_damage_per_distance = get_float(input.get("FallHurtAmount"));
        state.fall_damage_max = get_int(input.get("FallHurtMax"));
    } else if codec.state_is_anvil(state.block_state)? {
        state.hurt_entities = true;
    }
    if is_numeric(input.get("DropItem")) {
        state.drop_item = get_boolean(input.get("DropItem"));
    }
    if let Some(tag @ Tag::Compound(_)) = input.get("TileEntityData") {
        state.block_data = Some(tag.clone());
    }
    state.cancel_drop = get_boolean(input.get("CancelDrop"));
    if codec.state_is_air(state.block_state)? {
        state.block_state = codec.default_sand_state()?;
    }
    Ok(())
}

fn is_numeric(tag: Option<&Tag>) -> bool {
    matches!(tag, Some(Tag::Byte(_) | Tag::Short(_) | Tag::Int(_) | Tag::Long(_) | Tag::Float(_) | Tag::Double(_)))
}
fn java_floor(value: f64) -> i32 {
    let truncated = value as i32;
    if value < f64::from(truncated) { truncated.wrapping_sub(1) } else { truncated }
}
fn get_int(tag: Option<&Tag>) -> i32 {
    match tag {
        Some(Tag::Byte(v)) => i32::from(*v), Some(Tag::Short(v)) => i32::from(*v),
        Some(Tag::Int(v)) => *v, Some(Tag::Long(v)) => *v as i32,
        Some(Tag::Float(v)) => java_floor(f64::from(*v)),
        Some(Tag::Double(v)) => java_floor(*v), _ => 0,
    }
}
fn get_boolean(tag: Option<&Tag>) -> bool {
    // CompoundTag.getBoolean calls NumericTag.getAsByte, not `int != 0`.
    get_int(tag) as i8 != 0
}
fn get_float(tag: Option<&Tag>) -> f32 {
    match tag {
        Some(Tag::Byte(v)) => f32::from(*v), Some(Tag::Short(v)) => f32::from(*v),
        Some(Tag::Int(v)) => *v as f32, Some(Tag::Long(v)) => *v as f32,
        Some(Tag::Float(v)) => *v, Some(Tag::Double(v)) => *v as f32, _ => 0.,
    }
}

/// Explicit native checkpoint, intentionally richer than source entity NBT.
/// It retains the same physical/UUID/RNG/start-pos/teleport owners so a resume
/// need not run another constructor, draw entropy or invent an entity handle.
/// The outer checkpoint still certifies world history, global index/generation,
/// shared tick ordering, tracker baselines and actual registry identity.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FallingBlockSnapshot {
    pub physical: EntityPhysicalState,
    pub random_state48: u64,
    pub block_state: u32,
    pub time: i32,
    pub drop_item: bool,
    pub cancel_drop: bool,
    pub hurt_entities: bool,
    pub fall_damage_max: i32,
    pub fall_damage_per_distance_bits: u32,
    pub block_data: Option<Vec<u8>>,
    pub force_tick_after_teleport_to_duplicate: bool,
    pub start_pos: BlockPosition,
    pub blocks_building: bool,
}
impl FallingBlockSnapshot {
    pub fn capture(state: &FallingBlockState) -> Result<Self> {
        state.validate()?;
        Ok(Self {
            physical: state.physical.clone(), random_state48: state.random.state48(),
            block_state: state.block_state, time: state.time, drop_item: state.drop_item,
            cancel_drop: state.cancel_drop, hurt_entities: state.hurt_entities,
            fall_damage_max: state.fall_damage_max,
            fall_damage_per_distance_bits: state.fall_damage_per_distance.to_bits(),
            block_data: state.block_data.as_ref().map(Tag::encode_anonymous).transpose()?,
            force_tick_after_teleport_to_duplicate: state.force_tick_after_teleport_to_duplicate,
            start_pos: state.start_pos, blocks_building: state.blocks_building,
        })
    }
    pub fn restore(self) -> Result<FallingBlockState> {
        ensure!(self.random_state48 < (1_u64 << 48), "invalid falling random state");
        // LegacyRandomSource's constructor scrambles once. Undo that known
        // scramble to restore the recorded state without consuming any sample.
        let random = LegacyRandomSource::new((self.random_state48 ^ 25_214_903_917) as i64);
        let state = FallingBlockState {
            physical: self.physical, random, block_state: self.block_state,
            time: self.time, drop_item: self.drop_item, cancel_drop: self.cancel_drop,
            hurt_entities: self.hurt_entities, fall_damage_max: self.fall_damage_max,
            fall_damage_per_distance: f32::from_bits(self.fall_damage_per_distance_bits),
            block_data: self.block_data.as_deref().map(Tag::decode_anonymous).transpose()?,
            force_tick_after_teleport_to_duplicate: self.force_tick_after_teleport_to_duplicate,
            start_pos: self.start_pos, blocks_building: self.blocks_building,
        };
        state.validate()?;
        Ok(state)
    }
}
