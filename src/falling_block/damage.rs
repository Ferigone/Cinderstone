//! Pinned Java 1.21 `FallingBlockEntity.causeFallDamage`.
use super::state::FallingBlockState;
use crate::{geometry::Aabb, entity_state::EntityPhysicalState, random::LegacyRandomSource};
use anyhow::Result;

/// A borrow of the actual actor fields. This permits Entity.move's physical
/// callback to borrow the remaining FallingBlockEntity fields without cloning
/// or swapping its canonical physical state into a substitute object.
pub trait FallingBlockDamageActor {
    fn physical(&self) -> &EntityPhysicalState;
    fn physical_mut(&mut self) -> &mut EntityPhysicalState;
    fn random_mut(&mut self) -> &mut LegacyRandomSource;
    fn block_state(&self) -> u32;
    fn set_block_state(&mut self, state: u32);
    fn hurt_entities(&self) -> bool;
    fn fall_damage_per_distance(&self) -> f32;
    fn fall_damage_max(&self) -> i32;
    fn set_cancel_drop(&mut self, value: bool);
}
impl FallingBlockDamageActor for FallingBlockState {
    fn physical(&self) -> &EntityPhysicalState { &self.physical }
    fn physical_mut(&mut self) -> &mut EntityPhysicalState { &mut self.physical }
    fn random_mut(&mut self) -> &mut LegacyRandomSource { &mut self.random }
    fn block_state(&self) -> u32 { self.block_state }
    fn set_block_state(&mut self, state: u32) { self.block_state = state; }
    fn hurt_entities(&self) -> bool { self.hurt_entities }
    fn fall_damage_per_distance(&self) -> f32 { self.fall_damage_per_distance }
    fn fall_damage_max(&self) -> i32 { self.fall_damage_max }
    fn set_cancel_drop(&mut self, value: bool) { self.cancel_drop = value; }
}

/// Targets and sources are real owner-bound handles, not unqualified IDs.
/// Query and hurt callbacks retain the same canonical falling actor, including
/// any reentrant mutations made while a target's hurt callback executes.
pub trait FallingBlockDamageHost<A: FallingBlockDamageActor = FallingBlockState> {
    type DamageSource;
    type Target;
    fn preflight_fall_damage(&mut self, actor: &A) -> Result<()>;
    /// Actual current Block's Fallable.getFallDamageSource(actor), or the
    /// actual Level.damageSources().fallingBlock(actor) for a non-Fallable.
    fn fall_damage_source(&mut self, actor: &A) -> Result<Self::DamageSource>;
    /// Exact Level.getEntities(actor, bounds,
    /// NO_CREATIVE_OR_SPECTATOR.and(LIVING_ENTITY_STILL_ALIVE)), retaining the
    /// actual query order and same-owner generation-qualified target handles.
    fn damage_targets(&mut self, actor: &A, bounds: Aabb) -> Result<Vec<Self::Target>>;
    /// Invoke actual target.hurt(source, amount); its boolean is ignored by
    /// Java. Errors must not silently substitute a successful damage callback.
    fn hurt_target(
        &mut self,
        actor: &mut A,
        target: &Self::Target,
        source: &Self::DamageSource,
        amount: f32,
    ) -> Result<()>;
    fn state_in_anvil_tag(&mut self, state: u32) -> Result<bool>;
    /// Exact AnvilBlock.damage with real properties retained. None is the
    /// original final wear stage and sets cancelDrop; it is not an error.
    fn damaged_anvil_state(&mut self, state: u32) -> Result<Option<u32>>;
}

/// The source ignores both multiplier and incoming DamageSource and always
/// returns false. Its victim damage and Anvil wear are nevertheless real.
pub fn cause_fall_damage<A: FallingBlockDamageActor, H: FallingBlockDamageHost<A>>(
    actor: &mut A,
    fall_distance: f32,
    host: &mut H,
) -> Result<bool> {
    host.preflight_fall_damage(actor)?;
    if !actor.hurt_entities() {
        return Ok(false);
    }
    let distance = java_ceil_float(fall_distance - 1.0_f32);
    if distance < 0 {
        return Ok(false);
    }
    let source = host.fall_damage_source(actor)?;
    let damage = java_floor_float(distance as f32 * actor.fall_damage_per_distance())
        .min(actor.fall_damage_max()) as f32;
    let targets = host.damage_targets(actor, actor.physical().bounds)?;
    for target in targets {
        host.hurt_target(actor, &target, &source, damage)?;
    }
    if host.state_in_anvil_tag(actor.block_state())? && damage > 0.0_f32 {
        // Exactly one actual ENTITY float; no Level/player/global RNG draw.
        let sample = actor.random_mut().next_f32();
        let threshold = 0.05_f32 + distance as f32 * 0.05_f32;
        if sample < threshold {
            match host.damaged_anvil_state(actor.block_state())? {
                Some(state) => actor.set_block_state(state),
                None => actor.set_cancel_drop(true),
            }
        }
    }
    Ok(false)
}

// Mth's cast-then-conditional arithmetic, including Java narrowing saturation,
// NaN and wrapping +/-1 at integer extremes. Using f32::floor/ceil casts would
// differ at infinities and extreme values.
fn java_floor_float(value: f32) -> i32 {
    let integer = value as i32;
    if value < integer as f32 { integer.wrapping_sub(1) } else { integer }
}
fn java_ceil_float(value: f32) -> i32 {
    let integer = value as i32;
    if value > integer as f32 { integer.wrapping_add(1) } else { integer }
}
