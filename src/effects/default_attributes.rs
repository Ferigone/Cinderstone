//! Source Living/Mob/Bat default AttributeSupplier construction.
//!
//! The caller supplies actual Attributes static Holder.Reference objects from
//! the canonical31-entry registry. These factories do not create a registry or
//! global DefaultAttributes map. Call build_bat_supplier once in the real source
//! default-supplier bootstrap owner and share that returned supplier with all
//! Bats. It is not a per-actor constructor.

use super::{
    binding::{
        AttributeBindingOwner, BindingBuilder, BindingSupplier, NativeAttributeBinding,
        SupplierBindingResult,
    },
    supplier,
};

/// The exact source Attributes static references used by Living/Mob. These
/// names select canonical registered objects; no slot is a protocol/registry ID.
#[derive(Clone)]
pub struct LivingAttributeHolders<H> {
    pub max_health: H,
    pub knockback_resistance: H,
    pub movement_speed: H,
    pub armor: H,
    pub armor_toughness: H,
    pub max_absorption: H,
    pub step_height: H,
    pub scale: H,
    pub gravity: H,
    pub safe_fall_distance: H,
    pub fall_damage_multiplier: H,
    pub jump_strength: H,
    pub oxygen_bonus: H,
    pub burning_time: H,
    pub explosion_knockback_resistance: H,
    pub water_movement_efficiency: H,
    pub movement_efficiency: H,
    pub attack_knockback: H,
}
#[derive(Clone)]
pub struct MobAttributeHolders<H> {
    pub living: LivingAttributeHolders<H>,
    pub follow_range: H,
}

/// LivingEntity.createLivingAttributes319–337, in original add order.
pub fn create_living_attributes<C: AttributeBindingOwner>(
    holders: &LivingAttributeHolders<C::Holder>,
    binding: &mut NativeAttributeBinding<'_, C>,
) -> SupplierBindingResult<BindingBuilder<C>, C> {
    let builder = binding.new_builder();
    for holder in [
        &holders.max_health,
        &holders.knockback_resistance,
        &holders.movement_speed,
        &holders.armor,
        &holders.armor_toughness,
        &holders.max_absorption,
        &holders.step_height,
        &holders.scale,
        &holders.gravity,
        &holders.safe_fall_distance,
        &holders.fall_damage_multiplier,
        &holders.jump_strength,
        &holders.oxygen_bonus,
        &holders.burning_time,
        &holders.explosion_knockback_resistance,
        &holders.water_movement_efficiency,
        &holders.movement_efficiency,
        &holders.attack_knockback,
    ] {
        supplier::add(&builder, holder, binding)?;
    }
    Ok(builder)
}

/// Mob.createMobAttributes172–173 adds FOLLOW_RANGE with raw base16.
pub fn create_mob_attributes<C: AttributeBindingOwner>(
    holders: &MobAttributeHolders<C::Holder>,
    binding: &mut NativeAttributeBinding<'_, C>,
) -> SupplierBindingResult<BindingBuilder<C>, C> {
    let builder = create_living_attributes(&holders.living, binding)?;
    supplier::add_with_base(&builder, &holders.follow_range, 16.0, binding)?;
    Ok(builder)
}

/// Bat.createAttributes112–113 creates a NEW MAX_HEALTH6 template, appended to
/// the actual builder. buildKeepingLast later retains the FIRST health key
/// position and this LAST value:20allocated templates,19distinct attributes.
pub fn create_bat_attributes<C: AttributeBindingOwner>(
    holders: &MobAttributeHolders<C::Holder>,
    binding: &mut NativeAttributeBinding<'_, C>,
) -> SupplierBindingResult<BindingBuilder<C>, C> {
    let builder = create_mob_attributes(holders, binding)?;
    supplier::add_with_base(&builder, &holders.living.max_health, 6.0, binding)?;
    Ok(builder)
}

/// Source DefaultAttributes bootstrap's Bat factory then build. Freezing and
/// Guava hashing/duplicate behavior are the real supplier module operations.
/// Fresh actors attach an EMPTY NativeAttributeMap of this shared supplier;
/// their constructor health query reads its6-health fallback without getInstance.
pub fn build_bat_supplier<C: AttributeBindingOwner>(
    holders: &MobAttributeHolders<C::Holder>,
    binding: &mut NativeAttributeBinding<'_, C>,
) -> SupplierBindingResult<BindingSupplier<C>, C> {
    let builder = create_bat_attributes(holders, binding)?;
    binding.build_supplier(&builder)
}
