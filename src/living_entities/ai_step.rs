//! Source-ordered Java 1.21 LivingEntity.aiStep (`btn.m_`, 2736–2859).
//!
//! The caller retains one actual actor lifetime. This module owns no actor,
//! timer, physical state, health, effects, equipment, world, RNG or scheduler.
//! All callbacks are mandatory, synchronous and fallible; failures keep the
//! source mutation prefix and do not synthesize profiler cleanup. Virtual
//! subclass dispatch remains authoritative, including Mob's NoAI handling.
//!
//! Exact bounded helpers below implement private travelRidden2356–2369,
//! private updateFallFlying2869–2894 and default jump bodies2143–2168. Their
//! owners/virtual dependencies remain mandatory. These are not Main adapters.

use crate::geometry::{vanilla_cos, vanilla_sin, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiIntField {
    NoJumpDelay,
    LerpSteps,
    LerpHeadSteps,
    AutoSpinAttackTicks,
    TickCount,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiDoubleField {
    LerpX,
    LerpY,
    LerpZ,
    LerpYRot,
    LerpXRot,
    LerpYHeadRot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiFloatField {
    Xxa,
    Yya,
    Zza,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiFluid {
    Water,
    Lava,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiEffect {
    SlowFalling,
    Levitation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiDamage {
    Freeze,
    Drown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiProfileEvent {
    Push(&'static str),
    Pop,
}

/// Every field access targets the actor's canonical existing owner. Actor,
/// Level, Profiler, BoundingBox and source handles retain their original
/// identity/lifetime across reentry, removal and ID reuse. Vec3 is only the
/// immutable source value, never another physical-state owner. Each method
/// releases all actor/world borrows before returning or invoking callbacks.
pub trait LivingAiStepHost {
    type Error;
    type Actor;
    type Level;
    type Profiler;
    type BoundingBox;
    type DamageSources;
    type DamageSource;

    fn int_field(&mut self, actor: &Self::Actor, field: AiIntField) -> Result<i32, Self::Error>;
    fn set_int_field(
        &mut self,
        actor: &Self::Actor,
        field: AiIntField,
        value: i32,
    ) -> Result<(), Self::Error>;
    fn double_field(
        &mut self,
        actor: &Self::Actor,
        field: AiDoubleField,
    ) -> Result<f64, Self::Error>;
    fn float_field(&mut self, actor: &Self::Actor, field: AiFloatField)
        -> Result<f32, Self::Error>;
    fn set_float_field(
        &mut self,
        actor: &Self::Actor,
        field: AiFloatField,
        value: f32,
    ) -> Result<(), Self::Error>;
    fn jumping_field(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn set_jumping_field(&mut self, actor: &Self::Actor, value: bool) -> Result<(), Self::Error>;
    fn x(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn y(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn z(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    /// Actual Entity.syncPacketPositionCodec, updating its real VecDeltaCodec
    /// base; not teleporting, moving the actor or inventing a tracking owner.
    fn sync_packet_position_codec(
        &mut self,
        actor: &Self::Actor,
        position: Vec3,
    ) -> Result<(), Self::Error>;
    /// Virtual Entity/Mob/player-controller control and effective-AI queries.
    /// NoAI is not an additional guard in this executor.
    fn is_controlled_by_local_instance(&mut self, actor: &Self::Actor)
        -> Result<bool, Self::Error>;
    fn is_effective_ai(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual virtual interpolation; target order is X,Y,Z,YRot,XRot. The
    /// executor rereads the current counter after this callback.
    fn lerp_position_and_rotation_step(
        &mut self,
        actor: &Self::Actor,
        steps: i32,
        target: [f64; 5],
    ) -> Result<(), Self::Error>;
    fn lerp_head_rotation_step(
        &mut self,
        actor: &Self::Actor,
        steps: i32,
        target: f64,
    ) -> Result<(), Self::Error>;
    fn delta_movement(&mut self, actor: &Self::Actor) -> Result<Vec3, Self::Error>;
    /// Preserve the two source virtual overloads; they are not interchangeable
    /// dispatch shortcuts even when a particular actor inherits both defaults.
    fn set_delta_movement_vector(
        &mut self,
        actor: &Self::Actor,
        value: Vec3,
    ) -> Result<(), Self::Error>;
    fn set_delta_movement_components(
        &mut self,
        actor: &Self::Actor,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<(), Self::Error>;
    fn level(&mut self, actor: &Self::Actor) -> Result<Self::Level, Self::Error>;
    fn level_is_client_side(&mut self, level: &Self::Level) -> Result<bool, Self::Error>;
    fn profiler(&mut self, level: &Self::Level) -> Result<Self::Profiler, Self::Error>;
    fn profile(
        &mut self,
        profiler: &Self::Profiler,
        event: AiProfileEvent,
    ) -> Result<(), Self::Error>;
    fn is_immobile(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn server_ai_step(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn is_affected_by_fluids(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_in_lava(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_in_water(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn fluid_height(&mut self, actor: &Self::Actor, fluid: AiFluid) -> Result<f64, Self::Error>;
    fn fluid_jump_threshold(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn on_ground(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual virtual dispatch. The default helpers below cannot replace Mob /
    /// Player or other overrides simply because their state is unavailable.
    fn jump_in_liquid(&mut self, actor: &Self::Actor, fluid: AiFluid) -> Result<(), Self::Error>;
    fn jump_from_ground(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Actual private source body, e.g. update_fall_flying below with real
    /// equipment/effect/durability/metadata owners. No elytra-only approximation.
    fn update_fall_flying(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn bounding_box(&mut self, actor: &Self::Actor) -> Result<Self::BoundingBox, Self::Error>;
    fn has_effect(&mut self, actor: &Self::Actor, effect: AiEffect) -> Result<bool, Self::Error>;
    fn reset_fall_distance(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn controlling_passenger(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Option<Self::Actor>, Self::Error>;
    fn is_player(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_alive(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn travel_ridden(
        &mut self,
        actor: &Self::Actor,
        player: &Self::Actor,
        input: Vec3,
    ) -> Result<(), Self::Error>;
    /// Actual virtual ordinary travel (including its real world/movement
    /// callbacks), not a creative movement approximation or velocity-only step.
    fn travel(&mut self, actor: &Self::Actor, input: Vec3) -> Result<(), Self::Error>;
    fn is_dead_or_dying(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn ticks_frozen(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn is_in_powder_snow_field(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn can_freeze(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn ticks_required_to_freeze(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_ticks_frozen(&mut self, actor: &Self::Actor, value: i32) -> Result<(), Self::Error>;
    fn remove_frost(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn try_add_frost(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn is_fully_frozen(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn damage_sources(&mut self, actor: &Self::Actor) -> Result<Self::DamageSources, Self::Error>;
    fn damage_source(
        &mut self,
        sources: &Self::DamageSources,
        damage: AiDamage,
    ) -> Result<Self::DamageSource, Self::Error>;
    fn hurt(
        &mut self,
        actor: &Self::Actor,
        source: &Self::DamageSource,
        amount: f32,
    ) -> Result<bool, Self::Error>;
    fn check_auto_spin_attack(
        &mut self,
        actor: &Self::Actor,
        previous: &Self::BoundingBox,
        current: &Self::BoundingBox,
    ) -> Result<(), Self::Error>;
    fn push_entities(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn is_sensitive_to_water(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_in_water_rain_or_bubble(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
}

fn profile<H: LivingAiStepHost>(
    actor: &H::Actor,
    event: AiProfileEvent,
    host: &mut H,
) -> Result<(), H::Error> {
    let level = host.level(actor)?;
    let profiler = host.profiler(&level)?;
    host.profile(&profiler, event)
}
fn client_side<H: LivingAiStepHost>(actor: &H::Actor, host: &mut H) -> Result<bool, H::Error> {
    let level = host.level(actor)?;
    host.level_is_client_side(&level)
}
fn hurt<H: LivingAiStepHost>(
    actor: &H::Actor,
    damage: AiDamage,
    host: &mut H,
) -> Result<(), H::Error> {
    let sources = host.damage_sources(actor)?;
    let source = host.damage_source(&sources, damage)?;
    host.hurt(actor, &source, 1.)?;
    Ok(())
}

/// Default LivingEntity.aiStep only. A caller selecting a subclass's override
/// must execute that override's source and its actual parent callsites.
pub fn ai_step<H: LivingAiStepHost>(actor: &H::Actor, host: &mut H) -> Result<(), H::Error> {
    if host.int_field(actor, AiIntField::NoJumpDelay)? > 0 {
        let next = host
            .int_field(actor, AiIntField::NoJumpDelay)?
            .wrapping_sub(1);
        host.set_int_field(actor, AiIntField::NoJumpDelay, next)?;
    }
    if host.is_controlled_by_local_instance(actor)? {
        host.set_int_field(actor, AiIntField::LerpSteps, 0)?;
        let x = host.x(actor)?;
        let y = host.y(actor)?;
        let z = host.z(actor)?;
        host.sync_packet_position_codec(actor, Vec3::new(x, y, z))?;
    }
    if host.int_field(actor, AiIntField::LerpSteps)? > 0 {
        let count = host.int_field(actor, AiIntField::LerpSteps)?;
        let x = host.double_field(actor, AiDoubleField::LerpX)?;
        let y = host.double_field(actor, AiDoubleField::LerpY)?;
        let z = host.double_field(actor, AiDoubleField::LerpZ)?;
        let yaw = host.double_field(actor, AiDoubleField::LerpYRot)?;
        let pitch = host.double_field(actor, AiDoubleField::LerpXRot)?;
        host.lerp_position_and_rotation_step(actor, count, [x, y, z, yaw, pitch])?;
        let next = host
            .int_field(actor, AiIntField::LerpSteps)?
            .wrapping_sub(1);
        host.set_int_field(actor, AiIntField::LerpSteps, next)?;
    } else if !host.is_effective_ai(actor)? {
        let velocity = host.delta_movement(actor)?;
        host.set_delta_movement_vector(actor, velocity.scale(0.98_f64))?;
    }
    if host.int_field(actor, AiIntField::LerpHeadSteps)? > 0 {
        let count = host.int_field(actor, AiIntField::LerpHeadSteps)?;
        let target = host.double_field(actor, AiDoubleField::LerpYHeadRot)?;
        host.lerp_head_rotation_step(actor, count, target)?;
        let next = host
            .int_field(actor, AiIntField::LerpHeadSteps)?
            .wrapping_sub(1);
        host.set_int_field(actor, AiIntField::LerpHeadSteps, next)?;
    }
    let velocity = host.delta_movement(actor)?;
    let trim = |value: f64| if value.abs() < 0.003_f64 { 0. } else { value };
    host.set_delta_movement_components(
        actor,
        trim(velocity.x),
        trim(velocity.y),
        trim(velocity.z),
    )?;
    profile(actor, AiProfileEvent::Push("ai"), host)?;
    if host.is_immobile(actor)? {
        host.set_jumping_field(actor, false)?;
        host.set_float_field(actor, AiFloatField::Xxa, 0.)?;
        host.set_float_field(actor, AiFloatField::Zza, 0.)?;
    } else if host.is_effective_ai(actor)? {
        profile(actor, AiProfileEvent::Push("newAi"), host)?;
        host.server_ai_step(actor)?;
        profile(actor, AiProfileEvent::Pop, host)?;
    }
    profile(actor, AiProfileEvent::Pop, host)?;

    profile(actor, AiProfileEvent::Push("jump"), host)?;
    if host.jumping_field(actor)? && host.is_affected_by_fluids(actor)? {
        let fluid = if host.is_in_lava(actor)? {
            AiFluid::Lava
        } else {
            AiFluid::Water
        };
        let depth = host.fluid_height(actor, fluid)?;
        let water = host.is_in_water(actor)? && depth > 0.;
        let threshold = host.fluid_jump_threshold(actor)?;
        if water && (!host.on_ground(actor)? || depth > threshold) {
            host.jump_in_liquid(actor, AiFluid::Water)?;
        } else if host.is_in_lava(actor)? && (!host.on_ground(actor)? || depth > threshold) {
            host.jump_in_liquid(actor, AiFluid::Lava)?;
        } else if (host.on_ground(actor)? || (water && depth <= threshold))
            && host.int_field(actor, AiIntField::NoJumpDelay)? == 0
        {
            host.jump_from_ground(actor)?;
            host.set_int_field(actor, AiIntField::NoJumpDelay, 10)?;
        }
    } else {
        host.set_int_field(actor, AiIntField::NoJumpDelay, 0)?;
    }
    profile(actor, AiProfileEvent::Pop, host)?;

    profile(actor, AiProfileEvent::Push("travel"), host)?;
    let input = host.float_field(actor, AiFloatField::Xxa)? * 0.98_f32;
    host.set_float_field(actor, AiFloatField::Xxa, input)?;
    let input = host.float_field(actor, AiFloatField::Zza)? * 0.98_f32;
    host.set_float_field(actor, AiFloatField::Zza, input)?;
    host.update_fall_flying(actor)?;
    let old_bounds = host.bounding_box(actor)?;
    let x = host.float_field(actor, AiFloatField::Xxa)?;
    let y = host.float_field(actor, AiFloatField::Yya)?;
    let z = host.float_field(actor, AiFloatField::Zza)?;
    let input = Vec3::new(f64::from(x), f64::from(y), f64::from(z));
    if host.has_effect(actor, AiEffect::SlowFalling)?
        || host.has_effect(actor, AiEffect::Levitation)?
    {
        host.reset_fall_distance(actor)?;
    }
    let controller = host.controlling_passenger(actor)?;
    let ridden = match controller.as_ref() {
        Some(player) => host.is_player(player)? && host.is_alive(actor)?,
        None => false,
    };
    if ridden {
        host.travel_ridden(actor, controller.as_ref().expect("retained Player"), input)?;
    } else {
        host.travel(actor, input)?;
    }
    profile(actor, AiProfileEvent::Pop, host)?;

    profile(actor, AiProfileEvent::Push("freezing"), host)?;
    if !client_side(actor, host)? && !host.is_dead_or_dying(actor)? {
        let frozen = host.ticks_frozen(actor)?;
        if host.is_in_powder_snow_field(actor)? && host.can_freeze(actor)? {
            let maximum = host.ticks_required_to_freeze(actor)?;
            host.set_ticks_frozen(actor, maximum.min(frozen.wrapping_add(1)))?;
        } else {
            host.set_ticks_frozen(actor, 0.max(frozen.wrapping_sub(2)))?;
        }
    }
    host.remove_frost(actor)?;
    host.try_add_frost(actor)?;
    if !client_side(actor, host)?
        && host.int_field(actor, AiIntField::TickCount)? % 40 == 0
        && host.is_fully_frozen(actor)?
        && host.can_freeze(actor)?
    {
        hurt(actor, AiDamage::Freeze, host)?;
    }
    profile(actor, AiProfileEvent::Pop, host)?;

    profile(actor, AiProfileEvent::Push("push"), host)?;
    if host.int_field(actor, AiIntField::AutoSpinAttackTicks)? > 0 {
        let next = host
            .int_field(actor, AiIntField::AutoSpinAttackTicks)?
            .wrapping_sub(1);
        host.set_int_field(actor, AiIntField::AutoSpinAttackTicks, next)?;
        let current = host.bounding_box(actor)?;
        host.check_auto_spin_attack(actor, &old_bounds, &current)?;
    }
    host.push_entities(actor)?;
    profile(actor, AiProfileEvent::Pop, host)?;
    if !client_side(actor, host)?
        && host.is_sensitive_to_water(actor)?
        && host.is_in_water_rain_or_bubble(actor)?
    {
        hurt(actor, AiDamage::Drown, host)?;
    }
    Ok(())
}

pub trait LivingRiddenHost: LivingAiStepHost {
    fn ridden_input(
        &mut self,
        actor: &Self::Actor,
        player: &Self::Actor,
        input: Vec3,
    ) -> Result<Vec3, Self::Error>;
    fn tick_ridden(
        &mut self,
        actor: &Self::Actor,
        player: &Self::Actor,
        input: Vec3,
    ) -> Result<(), Self::Error>;
    fn ridden_speed(
        &mut self,
        actor: &Self::Actor,
        player: &Self::Actor,
    ) -> Result<f32, Self::Error>;
    fn set_speed(&mut self, actor: &Self::Actor, value: f32) -> Result<(), Self::Error>;
    fn calculate_entity_animation(
        &mut self,
        actor: &Self::Actor,
        flying: bool,
    ) -> Result<(), Self::Error>;
    fn try_check_inside_blocks(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
}

/// Private LivingEntity.travelRidden2356–2369, with the captured Player and
/// adjusted input retained across tickRidden/control callbacks.
pub fn travel_ridden<H: LivingRiddenHost>(
    actor: &H::Actor,
    player: &H::Actor,
    input: Vec3,
    host: &mut H,
) -> Result<(), H::Error> {
    let adjusted = host.ridden_input(actor, player, input)?;
    host.tick_ridden(actor, player, adjusted)?;
    if host.is_controlled_by_local_instance(actor)? {
        let speed = host.ridden_speed(actor, player)?;
        host.set_speed(actor, speed)?;
        host.travel(actor, adjusted)?;
    } else {
        host.calculate_entity_animation(actor, false)?;
        host.set_delta_movement_vector(actor, Vec3::ZERO)?;
        host.try_check_inside_blocks(actor)?;
    }
    Ok(())
}

pub trait LivingJumpHost: LivingAiStepHost {
    /// Actual virtual getJumpPower, with real block/attribute/effect owners.
    fn jump_power(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn is_sprinting(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn yaw(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn add_delta_movement(&mut self, actor: &Self::Actor, value: Vec3) -> Result<(), Self::Error>;
    fn set_has_impulse(&mut self, actor: &Self::Actor, value: bool) -> Result<(), Self::Error>;
}

/// Default LivingEntity.jumpFromGround2143–2160, using the existing original
/// Mth float sine table. Player/subclass overrides must retain their own bodies.
pub fn jump_from_ground<H: LivingJumpHost>(actor: &H::Actor, host: &mut H) -> Result<(), H::Error> {
    let power = host.jump_power(actor)?;
    // JVM fcmpg/ifgt continues on NaN: Java's <= guard is deliberately not
    // replaced with a negated positive comparison.
    if power <= 1.0e-5_f32 {
        return Ok(());
    }
    let velocity = host.delta_movement(actor)?;
    host.set_delta_movement_components(actor, velocity.x, f64::from(power), velocity.z)?;
    if host.is_sprinting(actor)? {
        let angle = host.yaw(actor)? * 0.017453292_f32;
        let boost = Vec3::new(
            f64::from(-vanilla_sin(angle)) * 0.2_f64,
            0.,
            f64::from(vanilla_cos(angle)) * 0.2_f64,
        );
        host.add_delta_movement(actor, boost)?;
    }
    host.set_has_impulse(actor, true)
}

/// Default LivingEntity.goDownInWater2163–2164.
pub fn go_down_in_water<H: LivingAiStepHost>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    let velocity = host.delta_movement(actor)?;
    host.set_delta_movement_vector(
        actor,
        velocity.plus(Vec3::new(0., -0.03999999910593033_f64, 0.)),
    )
}
/// Default LivingEntity.jumpInLiquid2167–2168 ignores its FluidTags parameter.
/// A subclass's actual override remains a separate virtual source operation.
pub fn jump_in_liquid<H: LivingAiStepHost>(
    actor: &H::Actor,
    _fluid: AiFluid,
    host: &mut H,
) -> Result<(), H::Error> {
    let velocity = host.delta_movement(actor)?;
    host.set_delta_movement_vector(
        actor,
        velocity.plus(Vec3::new(0., 0.03999999910593033_f64, 0.)),
    )
}

pub trait LivingFallFlyingHost: LivingAiStepHost {
    type ItemStack;
    fn shared_fall_flying_flag(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_passenger(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual retained CHEST stack, not a clone of the equipment state.
    fn chest_item(&mut self, actor: &Self::Actor) -> Result<Self::ItemStack, Self::Error>;
    fn item_is_elytra(&mut self, item: &Self::ItemStack) -> Result<bool, Self::Error>;
    fn elytra_is_fly_enabled(&mut self, item: &Self::ItemStack) -> Result<bool, Self::Error>;
    fn fall_fly_ticks_field(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    /// retainedStack.hurtAndBreak(1,actor,CHEST), including actual enchantment,
    /// RNG, equipment replacement, break event and item identity operations.
    fn hurt_and_break_chest_elytra(
        &mut self,
        actor: &Self::Actor,
        item: &Self::ItemStack,
    ) -> Result<(), Self::Error>;
    fn elytra_glide_game_event(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn set_shared_fall_flying_flag(
        &mut self,
        actor: &Self::Actor,
        value: bool,
    ) -> Result<(), Self::Error>;
}

/// Private LivingEntity.updateFallFlying2869–2894. The prospective counter is
/// a wrapped local only; actual fallFlyTicks advancement belongs to tick().
pub fn update_fall_flying<H: LivingFallFlyingHost>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    let mut flying = host.shared_fall_flying_flag(actor)?;
    if flying
        && !host.on_ground(actor)?
        && !host.is_passenger(actor)?
        && !host.has_effect(actor, AiEffect::Levitation)?
    {
        let item = host.chest_item(actor)?;
        if host.item_is_elytra(&item)? && host.elytra_is_fly_enabled(&item)? {
            flying = true;
            let next = host.fall_fly_ticks_field(actor)?.wrapping_add(1);
            if !client_side(actor, host)? && next % 10 == 0 {
                if (next / 10) % 2 == 0 {
                    host.hurt_and_break_chest_elytra(actor, &item)?;
                }
                host.elytra_glide_game_event(actor)?;
            }
        } else {
            flying = false;
        }
    } else {
        flying = false;
    }
    if !client_side(actor, host)? {
        host.set_shared_fall_flying_flag(actor, flying)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::{BTreeSet, VecDeque},
        rc::Rc,
    };

    // Controlled canonical-object callback fixtures, not real actors/worlds.
    type Actor = Rc<RefCell<State>>;
    struct State {
        ints: [i32; 5],
        doubles: [f64; 6],
        inputs: [f32; 3],
        position: Vec3,
        velocity: Vec3,
        bounds: usize,
        level: usize,
        controlled: bool,
        effective: bool,
        immobile: bool,
        jumping: bool,
        affected: bool,
        lava: bool,
        water: bool,
        ground: bool,
        depth: f64,
        threshold: f64,
        player: bool,
        alive: bool,
        dead: bool,
        removed: bool,
        powder: bool,
        freeze: bool,
        frozen: i32,
        required: i32,
        sensitive: bool,
        wet: bool,
        flying: bool,
        passenger: bool,
        fly_ticks: i32,
        power: f32,
        sprinting: bool,
        yaw: f32,
        impulse: bool,
        chest: Rc<Cell<i32>>,
    }
    impl State {
        fn actor() -> Actor {
            Rc::new(RefCell::new(Self {
                ints: [0, 0, 0, 0, 40],
                doubles: [1., 2., 3., 4., 5., 6.],
                inputs: [1., 2., 3.],
                position: Vec3::new(4., 5., 6.),
                velocity: Vec3::new(0.5, 0.25, -0.5),
                bounds: 1,
                level: 1,
                controlled: false,
                effective: false,
                immobile: false,
                jumping: false,
                affected: true,
                lava: false,
                water: false,
                ground: false,
                depth: 0.1,
                threshold: 0.4,
                player: false,
                alive: true,
                dead: false,
                removed: false,
                powder: false,
                freeze: true,
                frozen: 0,
                required: 140,
                sensitive: false,
                wet: false,
                flying: false,
                passenger: false,
                fly_ticks: 0,
                power: 0.42,
                sprinting: false,
                yaw: 90.,
                impulse: false,
                chest: Rc::new(Cell::new(0)),
            }))
        }
    }
    struct Host {
        actors: Vec<Actor>,
        calls: Vec<String>,
        controller: Option<Actor>,
        client_levels: BTreeSet<usize>,
        fail: Option<&'static str>,
        mutate_lerp: bool,
        change_level_in_ai: bool,
        slow: bool,
        mutate_effect: bool,
        kill_in_travel: bool,
        reenter_ai: bool,
        fail_jump_after: bool,
        mutate_jump_delay: bool,
        ground_answers: VecDeque<bool>,
        lava_answers: VecDeque<bool>,
        codec: Vec<Vec3>,
        traveled: Vec<Vec3>,
        spin: Vec<(usize, usize)>,
        adjusted: Option<Vec3>,
        control_after_ridden: Option<bool>,
        original_chest: Option<Rc<Cell<i32>>>,
        break_changes_level: bool,
    }
    impl Host {
        fn new(actor: &Actor) -> Self {
            Self {
                actors: vec![actor.clone()],
                calls: vec![],
                controller: None,
                client_levels: BTreeSet::new(),
                fail: None,
                mutate_lerp: false,
                change_level_in_ai: false,
                slow: false,
                mutate_effect: false,
                kill_in_travel: false,
                reenter_ai: false,
                fail_jump_after: false,
                mutate_jump_delay: false,
                ground_answers: VecDeque::new(),
                lava_answers: VecDeque::new(),
                codec: vec![],
                traveled: vec![],
                spin: vec![],
                adjusted: None,
                control_after_ridden: None,
                original_chest: None,
                break_changes_level: false,
            }
        }
        fn mark(&mut self, call: impl Into<String>) -> Result<(), &'static str> {
            for actor in &self.actors {
                assert!(
                    actor.try_borrow_mut().is_ok(),
                    "actor borrow held across callback"
                );
            }
            let call = call.into();
            self.calls.push(call.clone());
            if self.fail == Some(call.as_str()) {
                Err("owner failure")
            } else {
                Ok(())
            }
        }
        fn index(&self, call: &str) -> usize {
            self.calls.iter().position(|value| value == call).unwrap()
        }
        fn player(&mut self) -> Actor {
            let player = State::actor();
            player.borrow_mut().player = true;
            self.actors.push(player.clone());
            self.controller = Some(player.clone());
            player
        }
    }
    macro_rules! boolean {
        ($method:ident, $call:literal, $field:ident) => {
            fn $method(&mut self, actor: &Actor) -> Result<bool, Self::Error> {
                self.mark($call)?;
                Ok(actor.borrow().$field)
            }
        };
    }
    impl LivingAiStepHost for Host {
        type Error = &'static str;
        type Actor = Actor;
        type Level = usize;
        type Profiler = usize;
        type BoundingBox = usize;
        type DamageSources = usize;
        type DamageSource = (usize, AiDamage);
        fn int_field(&mut self, a: &Actor, f: AiIntField) -> Result<i32, Self::Error> {
            self.mark(format!("read:{f:?}"))?;
            Ok(a.borrow().ints[f as usize])
        }
        fn set_int_field(&mut self, a: &Actor, f: AiIntField, v: i32) -> Result<(), Self::Error> {
            self.mark(format!("write:{f:?}:{v}"))?;
            a.borrow_mut().ints[f as usize] = v;
            Ok(())
        }
        fn double_field(&mut self, a: &Actor, f: AiDoubleField) -> Result<f64, Self::Error> {
            self.mark(format!("read:{f:?}"))?;
            Ok(a.borrow().doubles[f as usize])
        }
        fn float_field(&mut self, a: &Actor, f: AiFloatField) -> Result<f32, Self::Error> {
            self.mark(format!("read:{f:?}"))?;
            Ok(a.borrow().inputs[f as usize])
        }
        fn set_float_field(
            &mut self,
            a: &Actor,
            f: AiFloatField,
            v: f32,
        ) -> Result<(), Self::Error> {
            self.mark(format!("write:{f:?}:{v}"))?;
            a.borrow_mut().inputs[f as usize] = v;
            Ok(())
        }
        boolean!(jumping_field, "jumping", jumping);
        fn set_jumping_field(&mut self, a: &Actor, v: bool) -> Result<(), Self::Error> {
            self.mark("set_jumping")?;
            a.borrow_mut().jumping = v;
            Ok(())
        }
        fn x(&mut self, a: &Actor) -> Result<f64, Self::Error> {
            self.mark("x")?;
            Ok(a.borrow().position.x)
        }
        fn y(&mut self, a: &Actor) -> Result<f64, Self::Error> {
            self.mark("y")?;
            Ok(a.borrow().position.y)
        }
        fn z(&mut self, a: &Actor) -> Result<f64, Self::Error> {
            self.mark("z")?;
            Ok(a.borrow().position.z)
        }
        fn sync_packet_position_codec(&mut self, _: &Actor, v: Vec3) -> Result<(), Self::Error> {
            self.mark("codec")?;
            self.codec.push(v);
            Ok(())
        }
        boolean!(is_controlled_by_local_instance, "controlled", controlled);
        boolean!(is_effective_ai, "effective", effective);
        fn lerp_position_and_rotation_step(
            &mut self,
            a: &Actor,
            count: i32,
            values: [f64; 5],
        ) -> Result<(), Self::Error> {
            self.mark("lerp")?;
            assert_eq!((count, values), (2, [1., 2., 3., 4., 5.]));
            if self.mutate_lerp {
                a.borrow_mut().ints[AiIntField::LerpSteps as usize] = 9;
            }
            Ok(())
        }
        fn lerp_head_rotation_step(
            &mut self,
            a: &Actor,
            count: i32,
            target: f64,
        ) -> Result<(), Self::Error> {
            self.mark("head_lerp")?;
            assert_eq!((count, target), (3, 6.));
            if self.mutate_lerp {
                a.borrow_mut().ints[AiIntField::LerpHeadSteps as usize] = 7;
            }
            Ok(())
        }
        fn delta_movement(&mut self, a: &Actor) -> Result<Vec3, Self::Error> {
            self.mark("velocity")?;
            Ok(a.borrow().velocity)
        }
        fn set_delta_movement_vector(&mut self, a: &Actor, v: Vec3) -> Result<(), Self::Error> {
            self.mark("set_vector")?;
            a.borrow_mut().velocity = v;
            Ok(())
        }
        fn set_delta_movement_components(
            &mut self,
            a: &Actor,
            x: f64,
            y: f64,
            z: f64,
        ) -> Result<(), Self::Error> {
            self.mark("set_components")?;
            a.borrow_mut().velocity = Vec3::new(x, y, z);
            Ok(())
        }
        fn level(&mut self, a: &Actor) -> Result<usize, Self::Error> {
            let level = a.borrow().level;
            self.mark(format!("level:{level}"))?;
            Ok(level)
        }
        fn level_is_client_side(&mut self, l: &usize) -> Result<bool, Self::Error> {
            self.mark(format!("client:{l}"))?;
            Ok(self.client_levels.contains(l))
        }
        fn profiler(&mut self, l: &usize) -> Result<usize, Self::Error> {
            self.mark(format!("profiler:{l}"))?;
            Ok(*l)
        }
        fn profile(&mut self, p: &usize, e: AiProfileEvent) -> Result<(), Self::Error> {
            self.mark(format!("profile:{p}:{e:?}"))
        }
        boolean!(is_immobile, "immobile", immobile);
        fn server_ai_step(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.mark("server_ai")?;
            if self.change_level_in_ai {
                a.borrow_mut().level = 2;
            }
            if self.reenter_ai {
                self.reenter_ai = false;
                ai_step(a, self)?;
            }
            Ok(())
        }
        boolean!(is_affected_by_fluids, "affected", affected);
        fn is_in_lava(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.mark("lava")?;
            Ok(self.lava_answers.pop_front().unwrap_or(a.borrow().lava))
        }
        boolean!(is_in_water, "water", water);
        fn fluid_height(&mut self, a: &Actor, f: AiFluid) -> Result<f64, Self::Error> {
            self.mark(format!("height:{f:?}"))?;
            Ok(a.borrow().depth)
        }
        fn fluid_jump_threshold(&mut self, a: &Actor) -> Result<f64, Self::Error> {
            self.mark("threshold")?;
            Ok(a.borrow().threshold)
        }
        fn on_ground(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.mark("ground")?;
            Ok(self.ground_answers.pop_front().unwrap_or(a.borrow().ground))
        }
        fn jump_in_liquid(&mut self, a: &Actor, f: AiFluid) -> Result<(), Self::Error> {
            self.mark(format!("jump_liquid:{f:?}"))?;
            jump_in_liquid(a, f, self)
        }
        fn jump_from_ground(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.mark("jump_ground")?;
            jump_from_ground(a, self)?;
            if self.mutate_jump_delay {
                a.borrow_mut().ints[0] = 77;
            }
            if self.fail_jump_after {
                Err("partial jump failure")
            } else {
                Ok(())
            }
        }
        fn update_fall_flying(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.mark("fall_flying")?;
            update_fall_flying(a, self)
        }
        fn bounding_box(&mut self, a: &Actor) -> Result<usize, Self::Error> {
            let bounds = a.borrow().bounds;
            self.mark(format!("bounds:{bounds}"))?;
            Ok(bounds)
        }
        fn has_effect(&mut self, a: &Actor, e: AiEffect) -> Result<bool, Self::Error> {
            self.mark(format!("effect:{e:?}"))?;
            if e == AiEffect::SlowFalling && self.mutate_effect {
                let mut state = a.borrow_mut();
                state.inputs[0] = 99.;
                state.bounds = 11;
            }
            Ok(e == AiEffect::SlowFalling && self.slow)
        }
        fn reset_fall_distance(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.mark("reset_fall")?;
            if self.mutate_effect {
                a.borrow_mut().bounds = 12;
            }
            Ok(())
        }
        fn controlling_passenger(&mut self, _: &Actor) -> Result<Option<Actor>, Self::Error> {
            self.mark("controller")?;
            Ok(self.controller.clone())
        }
        boolean!(is_player, "player", player);
        boolean!(is_alive, "alive", alive);
        fn travel_ridden(&mut self, a: &Actor, p: &Actor, v: Vec3) -> Result<(), Self::Error> {
            self.mark("ridden")?;
            travel_ridden(a, p, v, self)
        }
        fn travel(&mut self, a: &Actor, input: Vec3) -> Result<(), Self::Error> {
            self.mark("travel")?;
            self.traveled.push(input);
            if self.kill_in_travel {
                let mut state = a.borrow_mut();
                state.alive = false;
                state.dead = true;
                state.removed = true;
                state.bounds = 20;
            }
            Ok(())
        }
        boolean!(is_dead_or_dying, "dead", dead);
        fn ticks_frozen(&mut self, a: &Actor) -> Result<i32, Self::Error> {
            self.mark("frozen")?;
            Ok(a.borrow().frozen)
        }
        boolean!(is_in_powder_snow_field, "powder", powder);
        boolean!(can_freeze, "can_freeze", freeze);
        fn ticks_required_to_freeze(&mut self, a: &Actor) -> Result<i32, Self::Error> {
            self.mark("required")?;
            Ok(a.borrow().required)
        }
        fn set_ticks_frozen(&mut self, a: &Actor, v: i32) -> Result<(), Self::Error> {
            self.mark(format!("set_frozen:{v}"))?;
            a.borrow_mut().frozen = v;
            Ok(())
        }
        fn remove_frost(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.mark("remove_frost")
        }
        fn try_add_frost(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.mark("add_frost")
        }
        fn is_fully_frozen(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.mark("fully_frozen")?;
            let state = a.borrow();
            Ok(state.frozen >= state.required)
        }
        fn damage_sources(&mut self, a: &Actor) -> Result<usize, Self::Error> {
            let level = a.borrow().level;
            self.mark(format!("sources:{level}"))?;
            Ok(level)
        }
        fn damage_source(
            &mut self,
            s: &usize,
            d: AiDamage,
        ) -> Result<Self::DamageSource, Self::Error> {
            self.mark(format!("source:{s}:{d:?}"))?;
            Ok((*s, d))
        }
        fn hurt(
            &mut self,
            a: &Actor,
            s: &Self::DamageSource,
            amount: f32,
        ) -> Result<bool, Self::Error> {
            assert_eq!(amount, 1.);
            assert_eq!(s.0, a.borrow().level);
            self.mark(format!("hurt:{:?}", s.1))?;
            Ok(false)
        }
        fn check_auto_spin_attack(
            &mut self,
            a: &Actor,
            previous: &usize,
            current: &usize,
        ) -> Result<(), Self::Error> {
            assert_eq!(a.borrow().ints[3], 0);
            self.mark("spin")?;
            self.spin.push((*previous, *current));
            Ok(())
        }
        fn push_entities(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.mark("push")
        }
        boolean!(is_sensitive_to_water, "sensitive", sensitive);
        boolean!(is_in_water_rain_or_bubble, "wet", wet);
    }
    impl LivingRiddenHost for Host {
        fn ridden_input(&mut self, _: &Actor, p: &Actor, v: Vec3) -> Result<Vec3, Self::Error> {
            assert!(Rc::ptr_eq(p, self.controller.as_ref().unwrap()));
            self.mark("ridden_input")?;
            let result = v.plus(Vec3::new(1., 0., 0.));
            self.adjusted = Some(result);
            Ok(result)
        }
        fn tick_ridden(&mut self, a: &Actor, _: &Actor, v: Vec3) -> Result<(), Self::Error> {
            assert_eq!(Some(v), self.adjusted);
            self.mark("tick_ridden")?;
            if let Some(value) = self.control_after_ridden {
                a.borrow_mut().controlled = value;
            }
            Ok(())
        }
        fn ridden_speed(&mut self, _: &Actor, _: &Actor) -> Result<f32, Self::Error> {
            self.mark("ridden_speed")?;
            Ok(0.25)
        }
        fn set_speed(&mut self, _: &Actor, v: f32) -> Result<(), Self::Error> {
            assert_eq!(v, 0.25);
            self.mark("set_speed")
        }
        fn calculate_entity_animation(
            &mut self,
            _: &Actor,
            flying: bool,
        ) -> Result<(), Self::Error> {
            assert!(!flying);
            self.mark("animation")
        }
        fn try_check_inside_blocks(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.mark("inside")
        }
    }
    impl LivingJumpHost for Host {
        fn jump_power(&mut self, a: &Actor) -> Result<f32, Self::Error> {
            self.mark("power")?;
            Ok(a.borrow().power)
        }
        boolean!(is_sprinting, "sprinting", sprinting);
        fn yaw(&mut self, a: &Actor) -> Result<f32, Self::Error> {
            self.mark("yaw")?;
            Ok(a.borrow().yaw)
        }
        fn add_delta_movement(&mut self, a: &Actor, v: Vec3) -> Result<(), Self::Error> {
            self.mark("add_delta")?;
            let old = a.borrow().velocity;
            a.borrow_mut().velocity = old.plus(v);
            Ok(())
        }
        fn set_has_impulse(&mut self, a: &Actor, v: bool) -> Result<(), Self::Error> {
            self.mark("impulse")?;
            a.borrow_mut().impulse = v;
            Ok(())
        }
    }
    impl LivingFallFlyingHost for Host {
        type ItemStack = Rc<Cell<i32>>;
        boolean!(shared_fall_flying_flag, "flying_flag", flying);
        boolean!(is_passenger, "passenger", passenger);
        fn chest_item(&mut self, a: &Actor) -> Result<Self::ItemStack, Self::Error> {
            self.mark("chest")?;
            let item = a.borrow().chest.clone();
            self.original_chest = Some(item.clone());
            Ok(item)
        }
        fn item_is_elytra(&mut self, _: &Self::ItemStack) -> Result<bool, Self::Error> {
            self.mark("elytra")?;
            Ok(true)
        }
        fn elytra_is_fly_enabled(&mut self, _: &Self::ItemStack) -> Result<bool, Self::Error> {
            self.mark("fly_enabled")?;
            Ok(true)
        }
        fn fall_fly_ticks_field(&mut self, a: &Actor) -> Result<i32, Self::Error> {
            self.mark("fly_ticks")?;
            Ok(a.borrow().fly_ticks)
        }
        fn hurt_and_break_chest_elytra(
            &mut self,
            a: &Actor,
            item: &Self::ItemStack,
        ) -> Result<(), Self::Error> {
            assert!(Rc::ptr_eq(item, self.original_chest.as_ref().unwrap()));
            self.mark("break_elytra")?;
            item.set(item.get() + 1);
            if self.break_changes_level {
                let mut state = a.borrow_mut();
                state.level = 2;
                state.flying = false;
                state.chest = Rc::new(Cell::new(50));
            }
            Ok(())
        }
        fn elytra_glide_game_event(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.mark("glide_event")
        }
        fn set_shared_fall_flying_flag(&mut self, a: &Actor, v: bool) -> Result<(), Self::Error> {
            self.mark(format!("set_flying:{v}"))?;
            a.borrow_mut().flying = v;
            Ok(())
        }
    }

    #[test]
    fn source_locals_survive_effect_travel_death_and_current_profiler_changes() {
        let actor = State::actor();
        {
            let mut a = actor.borrow_mut();
            a.controlled = true;
            a.effective = true;
            a.ints[3] = 1;
            a.frozen = 140;
            a.sensitive = true;
            a.wet = true;
        }
        let mut host = Host::new(&actor);
        host.change_level_in_ai = true;
        host.slow = true;
        host.mutate_effect = true;
        host.kill_in_travel = true;
        ai_step(&actor, &mut host).unwrap();
        assert_eq!(host.codec, [Vec3::new(4., 5., 6.)]);
        assert_eq!(
            host.traveled,
            [Vec3::new(
                f64::from(0.98_f32),
                2.,
                f64::from(3_f32 * 0.98_f32)
            )]
        );
        assert_eq!(host.spin, [(1, 20)]);
        assert!(actor.borrow().removed);
        assert_eq!(actor.borrow().frozen, 140);
        assert!(host.index("profile:1:Push(\"newAi\")") < host.index("server_ai"));
        assert!(host.index("server_ai") < host.index("profile:2:Pop"));
        assert!(host.index("bounds:1") < host.index("effect:SlowFalling"));
        assert!(host.index("remove_frost") < host.index("hurt:Freeze"));
        assert!(host.index("hurt:Freeze") < host.index("spin"));
        assert!(host.index("push") < host.index("hurt:Drown"));
        assert!(!host.calls.iter().any(|c| c == "effect:Levitation"));
        assert!(!host.calls.iter().any(|c| c == "frozen")); // dead timer gate, not damage gate
    }

    #[test]
    fn interpolation_rereads_counters_and_immobile_keeps_y_input_and_nan_velocity() {
        let actor = State::actor();
        {
            let mut a = actor.borrow_mut();
            a.ints[1] = 2;
            a.ints[2] = 3;
        }
        let mut host = Host::new(&actor);
        host.mutate_lerp = true;
        ai_step(&actor, &mut host).unwrap();
        assert_eq!((actor.borrow().ints[1], actor.borrow().ints[2]), (8, 6));
        assert!(!host.calls.iter().any(|c| c == "codec"));
        let actor = State::actor();
        let payload = f64::from_bits(0xfff8000000001234);
        {
            let mut a = actor.borrow_mut();
            a.effective = true;
            a.immobile = true;
            a.velocity = Vec3::new(-0., 0.003, payload);
        }
        let mut host = Host::new(&actor);
        ai_step(&actor, &mut host).unwrap();
        let velocity = actor.borrow().velocity;
        assert_eq!(velocity.x.to_bits(), 0_f64.to_bits());
        assert_eq!(velocity.y, 0.003);
        assert_eq!(velocity.z.to_bits(), payload.to_bits());
        assert_eq!(host.traveled, [Vec3::new(0., 2., 0.)]);
        assert!(!host.calls.iter().any(|c| c == "server_ai"));
    }

    #[test]
    fn fluid_queries_are_repeated_and_jump_callback_error_keeps_its_prefix() {
        let actor = State::actor();
        {
            let mut a = actor.borrow_mut();
            a.jumping = true;
            a.water = true;
        }
        let mut host = Host::new(&actor);
        host.lava_answers = [false, true].into();
        host.ground_answers = [true, false].into();
        ai_step(&actor, &mut host).unwrap();
        assert!(host.index("height:Water") < host.index("water"));
        assert!(host.calls.iter().any(|c| c == "jump_liquid:Lava"));
        assert_eq!(host.calls.iter().filter(|c| *c == "lava").count(), 2);
        assert_eq!(host.calls.iter().filter(|c| *c == "ground").count(), 2);
        assert!(!host.calls.iter().any(|c| c == "jump_ground"));
        let actor = State::actor();
        {
            let mut a = actor.borrow_mut();
            a.jumping = true;
            a.ground = true;
            a.ints[0] = 1;
        }
        let mut host = Host::new(&actor);
        host.mutate_jump_delay = true;
        host.fail_jump_after = true;
        assert_eq!(ai_step(&actor, &mut host), Err("partial jump failure"));
        assert_eq!(actor.borrow().ints[0], 77);
        assert!(actor.borrow().impulse);
        assert!(!host
            .calls
            .iter()
            .any(|c| c == "write:NoJumpDelay:10" || c == "fall_flying"));
        host.calls.clear();
        host.fail_jump_after = false;
        actor.borrow_mut().ints[0] = 0;
        ai_step(&actor, &mut host).unwrap();
        assert_eq!(actor.borrow().ints[0], 10);
    }

    #[test]
    fn freeze_wrap_late_error_and_recursive_ai_use_the_same_object() {
        let actor = State::actor();
        {
            let mut a = actor.borrow_mut();
            a.powder = true;
            a.frozen = i32::MAX;
            a.ints[3] = 1;
        }
        let mut host = Host::new(&actor);
        host.fail = Some("spin");
        assert_eq!(ai_step(&actor, &mut host), Err("owner failure"));
        assert_eq!(actor.borrow().frozen, i32::MIN);
        assert_eq!(actor.borrow().ints[3], 0);
        assert_eq!(host.calls.last().unwrap(), "spin");
        assert!(!host.calls.iter().any(|c| c == "push")); // no synthetic pop or tail
        let actor = State::actor();
        actor.borrow_mut().effective = true;
        let mut host = Host::new(&actor);
        host.reenter_ai = true;
        ai_step(&actor, &mut host).unwrap();
        assert_eq!(host.calls.iter().filter(|c| *c == "server_ai").count(), 2);
        assert_eq!(
            actor.borrow().inputs[0].to_bits(),
            ((1_f32 * 0.98_f32) * 0.98_f32).to_bits()
        );
        assert_eq!(host.traveled.len(), 2);
    }

    #[test]
    fn ridden_adjusted_input_control_reread_and_remote_branch_order() {
        let actor = State::actor();
        actor.borrow_mut().controlled = true;
        let mut host = Host::new(&actor);
        let player = host.player();
        let input = Vec3::new(2., 3., 4.);
        travel_ridden(&actor, &player, input, &mut host).unwrap();
        assert_eq!(host.traveled, [Vec3::new(3., 3., 4.)]);
        assert_eq!(
            host.calls,
            [
                "ridden_input",
                "tick_ridden",
                "controlled",
                "ridden_speed",
                "set_speed",
                "travel"
            ]
        );
        host.calls.clear();
        host.traveled.clear();
        host.control_after_ridden = Some(false);
        travel_ridden(&actor, &player, input, &mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                "ridden_input",
                "tick_ridden",
                "controlled",
                "animation",
                "set_vector",
                "inside"
            ]
        );
        assert_eq!(actor.borrow().velocity, Vec3::ZERO);
        assert!(host.traveled.is_empty());
    }

    #[test]
    fn jump_defaults_use_source_float_math_threshold_and_vector_addition() {
        let actor = State::actor();
        {
            let mut a = actor.borrow_mut();
            a.sprinting = true;
            a.velocity.z = 0.;
        }
        let mut host = Host::new(&actor);
        jump_from_ground(&actor, &mut host).unwrap();
        let actual = actor.borrow().velocity;
        assert_eq!(actual.x, 0.3);
        assert_eq!(actual.y, 0.41999998688697815);
        assert_eq!(actual.z, 2.4492937051703357e-17);
        assert!(actor.borrow().impulse);
        for power in [1.0e-5_f32, -1.] {
            actor.borrow_mut().power = power;
            host.calls.clear();
            jump_from_ground(&actor, &mut host).unwrap();
            assert_eq!(host.calls, ["power"]);
        }
        {
            let mut a = actor.borrow_mut();
            a.power = f32::NAN;
            a.sprinting = false;
            a.impulse = false;
        }
        host.calls.clear();
        jump_from_ground(&actor, &mut host).unwrap();
        assert!(actor.borrow().velocity.y.is_nan());
        assert!(actor.borrow().impulse);
        assert_eq!(
            host.calls,
            [
                "power",
                "velocity",
                "set_components",
                "sprinting",
                "impulse"
            ]
        );
        actor.borrow_mut().velocity = Vec3::new(-0., 0., -0.);
        host.calls.clear();
        jump_in_liquid(&actor, AiFluid::Lava, &mut host).unwrap();
        let actual = actor.borrow().velocity;
        assert_eq!(actual.x.to_bits(), 0_f64.to_bits());
        assert_eq!(actual.z.to_bits(), 0_f64.to_bits());
        assert_eq!(actual.y, 0.03999999910593033);
        go_down_in_water(&actor, &mut host).unwrap();
        assert_eq!(actor.borrow().velocity.y, 0.);
    }

    #[test]
    fn fall_flying_retains_stack_counter_local_and_fresh_side_after_break() {
        let actor = State::actor();
        {
            let mut a = actor.borrow_mut();
            a.flying = true;
            a.fly_ticks = 19;
        }
        let mut host = Host::new(&actor);
        host.break_changes_level = true;
        host.client_levels.insert(2);
        let original = actor.borrow().chest.clone();
        update_fall_flying(&actor, &mut host).unwrap();
        assert_eq!(original.get(), 1);
        assert_eq!(actor.borrow().chest.get(), 50);
        assert_eq!(actor.borrow().fly_ticks, 19);
        assert!(!actor.borrow().flying);
        assert!(host.index("break_elytra") < host.index("glide_event"));
        assert_eq!(host.calls.last().unwrap(), "client:2");
        assert!(!host.calls.iter().any(|c| c.starts_with("set_flying:")));
        let actor = State::actor();
        {
            let mut a = actor.borrow_mut();
            a.flying = true;
            a.fly_ticks = 9;
        }
        let mut host = Host::new(&actor);
        update_fall_flying(&actor, &mut host).unwrap();
        assert!(!host.calls.iter().any(|c| c == "break_elytra"));
        assert_eq!(host.calls.last().unwrap(), "set_flying:true");
        host.calls.clear();
        actor.borrow_mut().fly_ticks = 19;
        host.fail = Some("glide_event");
        assert_eq!(update_fall_flying(&actor, &mut host), Err("owner failure"));
        assert_eq!(host.calls.last().unwrap(), "glide_event");
        assert_eq!(actor.borrow().chest.get(), 1);
    }
}
