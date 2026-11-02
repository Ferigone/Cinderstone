//! Java 1.21 Player.enchantmentSeed and its actual lifecycle boundaries.
//!
//! Construction, saved-data loading and ServerPlayer.restoreFrom are distinct
//! operations: a getter never loads
//! a seed or obtains randomness on behalf of an absent runtime owner.
use crate::{entity_state::java_floor, nbt::Tag, random::RandomSource};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

/// The canonical field owned by one actual player. No Default is provided:
/// missing saved/runtime ownership must not become a newly constructed zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerEnchantmentSeed {
    value: i32,
}

impl PlayerEnchantmentSeed {
    /// Genuine JVM Player constructor state. Neither Player's constructor nor
    /// its getter replaces this zero or draws from the player's RNG.
    pub const fn new_player() -> Self {
        Self { value: 0 }
    }

    /// Retain an actual saved source value without loading a live player or
    /// consuming RNG. Admission must distinguish this disk-load proposal from
    /// a retained restoreFrom snapshot before applying the zero-load rule.
    pub const fn from_saved_value(value: i32) -> Self {
        Self { value }
    }

    pub const fn get(&self) -> i32 {
        self.value
    }

    /// Borrow this actual field for Player.onEnchantmentPerformed. The caller
    /// must supply the same player's canonical XP fields and Player.random.
    pub fn field_mut(&mut self) -> &mut i32 {
        &mut self.value
    }

    /// Player.readAdditionalSaveData, after CompoundTag.getInt("XpSeed").
    /// The argument must be that actual saved source value, not a caller's
    /// default for an unknown old native save. Zero causes exactly one
    /// unbounded nextInt, even if its result is itself zero. Negative values
    /// are ordinary seeds and consume no randomness here.
    pub fn load_xp_seed<R: RandomSource + ?Sized>(
        &mut self,
        source_xp_seed: i32,
        player_random: &mut R,
    ) {
        self.value = source_xp_seed;
        if self.value == 0 {
            self.value = player_random.next_i32();
        }
    }

    /// Read the real source compound before mutating the field or RNG.
    pub fn read_additional_save_data<R: RandomSource + ?Sized>(
        &mut self,
        source: &Tag,
        player_random: &mut R,
    ) -> Result<()> {
        self.load_xp_seed(read_xp_seed_value(source)?, player_random);
        Ok(())
    }

    /// Player.addAdditionalSaveData stores the actual signed field as an Int,
    /// including zero; saving does not draw or replace the retained value.
    pub fn add_additional_save_data(&self, target: &mut Tag) -> Result<()> {
        target.insert("XpSeed", Tag::Int(self.value))?;
        Ok(())
    }

    /// ServerPlayer.restoreFrom copies the seed after both keep-state paths
    /// converge. It is independent of keepEverything/keepInventory and draws
    /// no randomness, including when the previous player's field is zero.
    pub fn restore_from(&mut self, previous: &Self) {
        self.value = previous.value;
    }
}

/// CompoundTag.getInt("XpSeed") over a real source compound, without loading
/// a player or consuming its RNG. Missing/non-numeric entries return zero as
/// the original getter does; a missing source compound is an explicit error.
/// Float/Double NumericTag.getAsInt uses Mth.floor, not truncation.
pub fn read_xp_seed_value(source: &Tag) -> Result<i32> {
    ensure!(matches!(source, Tag::Compound(_)), "player seed source is not an NBT compound");
    Ok(match source.get("XpSeed") {
        Some(Tag::Byte(value)) => i32::from(*value),
        Some(Tag::Short(value)) => i32::from(*value),
        Some(Tag::Int(value)) => *value,
        Some(Tag::Long(value)) => *value as i32,
        Some(Tag::Float(value)) => java_floor(f64::from(*value)),
        Some(Tag::Double(value)) => java_floor(*value),
        _ => 0,
    })
}
