//! Java1.21 Player enchanting field updates over caller-owned native state.
//!
//! Callers supply the actual canonical XP fields, enchantment-seed field and
//! Player.random. This module creates, infers, seeds or persists none of them.
use crate::{player_data::ExperienceData, random::RandomSource};

/// Player.getEnchantmentSeed returns the actual field, including zero. Load's
/// separate zero-seed replacement is not performed by this getter.
pub fn get_enchantment_seed(enchantment_seed: &i32) -> i32 { *enchantment_seed }

/// Player.onEnchantmentPerformed(ItemStack,int), source1776–1783.
///
/// The original ItemStack argument is not read by this body. The actual menu
/// calls this body for instabuild players too: do not add a creative exemption
/// or replace the option's level payment with the displayed offer cost.
///
/// This is deliberately not giveExperiencePoints/giveExperienceLevels: total
/// and progress remain untouched unless the wrapped resulting level is
/// negative. It performs one unbounded nextInt after those field updates.
pub fn on_enchantment_performed<R: RandomSource + ?Sized>(
    experience: &mut ExperienceData,
    enchantment_seed: &mut i32,
    player_random: &mut R,
    levels: i32,
) {
    experience.level = experience.level.wrapping_sub(levels);
    if experience.level < 0 {
        experience.level = 0;
        experience.progress = 0.0;
        experience.total = 0;
    }
    *enchantment_seed = player_random.next_i32();
}

/// ServerPlayer's actual override calls the base body first, then invalidates
/// its real lastSentExp field. It does not send an XP packet immediately.
/// The caller must supply that existing cache owner, never a detached dummy.
/// Main must preflight the actual registered entity/UUID, known lifecycle seed,
/// lifecycle XP and retained Player RNG before borrowing these fields. This is
/// not a saved-data admission or a reconfiguration/respawn classifier.
pub fn server_on_enchantment_performed<R: RandomSource + ?Sized>(
    experience: &mut ExperienceData,
    enchantment_seed: &mut i32,
    player_random: &mut R,
    levels: i32,
    last_sent_experience: &mut i32,
) {
    on_enchantment_performed(experience, enchantment_seed, player_random, levels);
    *last_sent_experience = -1;
}
