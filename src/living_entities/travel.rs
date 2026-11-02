//! Ordinary Java 1.21 LivingEntity.travel (`btn`, 2184–2353) and its selected
//! movement helpers (2397–2450). Bat inherits this body, not FlyingMob travel.
//!
//! This module creates no actor, physical state, attribute/effect map, RNG,
//! navigation, scheduler or Main callsite. Actual source virtual methods and
//! world/contact/damage/animation owners are mandatory. Public helper dispatch
//! remains virtual; private helpers below run their selected ordinary bodies.

use crate::{entity_base::FluidTag, geometry::Vec3};

pub type BlockPosition = (i32, i32, i32);

/// These names select the actual canonical original holders, not numeric IDs
/// or an independently cached set of effect flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TravelEffect {
    SlowFalling,
    DolphinsGrace,
    Levitation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TravelBlock {
    PowderSnow,
    Scaffolding,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TravelError<E> {
    Host(E),
    /// Source hasEffect(LEVITATION) succeeded, but the subsequent getEffect
    /// returned null before getAmplifier. Do not substitute amplifier zero.
    NullLevitation,
}
impl<E> From<E> for TravelError<E> {
    fn from(error: E) -> Self {
        Self::Host(error)
    }
}

/// Every Actor handle retains ONE original canonical living actor lifetime,
/// including through removal, reentry, or replacement of its live registry ID.
/// Reference-like values similarly retain the exact returned source objects.
/// They must not re-resolve a replacement Level/state/effect/source by ID.
///
/// No actor/body/map/instance borrow may span a source virtual/world callback.
/// Reads/writes described as fields or FINAL accessors are short, callback-free
/// canonical accesses. Numeric vectors are immutable source value locals; no
/// independent mutable physical body is created. Errors preserve every already
/// executed mutation/callback and stop without rollback, retries or cleanup.
/// Missing owners must report errors; there are no successful dummy defaults.
pub trait LivingTravelHost {
    type Error;
    type Actor: ?Sized;
    type Level;
    type FluidState;
    type BlockState;
    type Block;
    type EffectLease;
    type Passenger;
    type Sound;
    type DamageSources;
    type DamageSource;

    fn is_controlled_by_local_instance(&mut self, actor: &Self::Actor)
        -> Result<bool, Self::Error>;
    /// Actual FINAL Entity.getGravity, including isNoGravity metadata and
    /// virtual getDefaultGravity (LivingEntity reads Attributes.GRAVITY).
    fn gravity(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn delta_movement(&mut self, actor: &Self::Actor) -> Result<Vec3, Self::Error>;
    /// Source virtual Entity.setDeltaMovement(Vec3).
    fn set_delta_movement(&mut self, actor: &Self::Actor, value: Vec3) -> Result<(), Self::Error>;
    /// Source virtual Entity.setDeltaMovement(double,double,double). Preserve
    /// this overload's actual base/override dispatch instead of bypassing it.
    fn set_delta_components(
        &mut self,
        actor: &Self::Actor,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<(), Self::Error>;
    /// Callback-free FINAL Entity.getY and actual physical collision field.
    fn y(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn horizontal_collision(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn on_ground(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn level(&mut self, actor: &Self::Actor) -> Result<Self::Level, Self::Error>;
    fn block_position(&mut self, actor: &Self::Actor) -> Result<BlockPosition, Self::Error>;
    fn fluid_state(
        &mut self,
        level: &Self::Level,
        at: BlockPosition,
    ) -> Result<Self::FluidState, Self::Error>;
    fn is_in_water(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_in_lava(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_affected_by_fluids(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn can_stand_on_fluid(
        &mut self,
        actor: &Self::Actor,
        fluid: &Self::FluidState,
    ) -> Result<bool, Self::Error>;
    fn is_sprinting(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn water_slow_down(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Actual Attributes.WATER_MOVEMENT_EFFICIENCY value.
    fn water_movement_efficiency(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn speed(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn has_effect(
        &mut self,
        actor: &Self::Actor,
        effect: TravelEffect,
    ) -> Result<bool, Self::Error>;
    fn effect(
        &mut self,
        actor: &Self::Actor,
        effect: TravelEffect,
    ) -> Result<Option<Self::EffectLease>, Self::Error>;
    /// Callback-free amplifier read on that retained actual instance.
    fn effect_amplifier(&mut self, effect: &Self::EffectLease) -> Result<i32, Self::Error>;
    fn move_relative(
        &mut self,
        actor: &Self::Actor,
        speed: f32,
        input: Vec3,
    ) -> Result<(), Self::Error>;
    /// Actual Entity.move(MoverType.SELF, movement), including subtype contact,
    /// fall, block/fluid, sound/game-event and removal callbacks and their RNG.
    fn move_self(&mut self, actor: &Self::Actor, movement: Vec3) -> Result<(), Self::Error>;
    fn on_climbable(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual virtual helper. An ordinary owner can call the selected base body
    /// below; derived overrides must not be replaced by that body.
    fn fluid_falling_adjusted_movement(
        &mut self,
        actor: &Self::Actor,
        gravity: f64,
        falling: bool,
        movement: Vec3,
    ) -> Result<Vec3, Self::Error>;
    fn is_free(&mut self, actor: &Self::Actor, x: f64, y: f64, z: f64)
        -> Result<bool, Self::Error>;
    fn fluid_height(&mut self, actor: &Self::Actor, tag: FluidTag) -> Result<f64, Self::Error>;
    fn fluid_jump_threshold(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn is_fall_flying(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn check_slow_fall_distance(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn look_angle(&mut self, actor: &Self::Actor) -> Result<Vec3, Self::Error>;
    fn pitch(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// PURE proven native equivalent of the pinned Java Math.cos operation.
    /// No gameplay callbacks/reentry/RNG. Existing hotspot_math::sin_cos covers
    /// only finite [0,f32::TAU]; it must not panic on a negative/out-of-domain
    /// pitch or silently fall back to libm. Unsupported providers must error.
    fn java_math_cos(&mut self, radians: f64) -> Result<f64, Self::Error>;
    /// Source Level.isClientSide field read on the retained Level.
    fn level_is_client_side(&mut self, level: &Self::Level) -> Result<bool, Self::Error>;
    fn fall_damage_sound(
        &mut self,
        actor: &Self::Actor,
        damage: i32,
    ) -> Result<Self::Sound, Self::Error>;
    fn play_sound(
        &mut self,
        actor: &Self::Actor,
        sound: &Self::Sound,
        volume: f32,
        pitch: f32,
    ) -> Result<(), Self::Error>;
    fn damage_sources(&mut self, actor: &Self::Actor) -> Result<Self::DamageSources, Self::Error>;
    fn fly_into_wall_source(
        &mut self,
        sources: &Self::DamageSources,
    ) -> Result<Self::DamageSource, Self::Error>;
    fn hurt(
        &mut self,
        actor: &Self::Actor,
        source: &Self::DamageSource,
        amount: f32,
    ) -> Result<bool, Self::Error>;
    fn set_shared_flag(
        &mut self,
        actor: &Self::Actor,
        flag: u8,
        value: bool,
    ) -> Result<(), Self::Error>;
    fn movement_friction_position(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<BlockPosition, Self::Error>;
    fn block_state(
        &mut self,
        level: &Self::Level,
        at: BlockPosition,
    ) -> Result<Self::BlockState, Self::Error>;
    fn state_block(&mut self, state: &Self::BlockState) -> Result<Self::Block, Self::Error>;
    fn block_friction(&mut self, block: &Self::Block) -> Result<f32, Self::Error>;
    /// Actual virtual handleRelativeFrictionAndCalculateMovement.
    fn relative_friction_movement(
        &mut self,
        actor: &Self::Actor,
        input: Vec3,
        friction: f32,
    ) -> Result<Vec3, Self::Error>;
    fn level_has_chunk_at(
        &mut self,
        level: &Self::Level,
        at: BlockPosition,
    ) -> Result<bool, Self::Error>;
    fn minimum_build_height(&mut self, level: &Self::Level) -> Result<i32, Self::Error>;
    fn should_discard_friction(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Callback-free actual instanceof FlyingAnimal (cfn), NOT a movement flag.
    /// Bat does not implement that interface despite its normal flight.
    fn is_flying_animal(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn calculate_entity_animation(
        &mut self,
        actor: &Self::Actor,
        include_vertical: bool,
    ) -> Result<(), Self::Error>;

    // Source private-helper dependencies; no replacement private-method hooks.
    fn reset_fall_distance(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn in_block_state(&mut self, actor: &Self::Actor) -> Result<Self::BlockState, Self::Error>;
    fn state_is_block(
        &mut self,
        state: &Self::BlockState,
        block: TravelBlock,
    ) -> Result<bool, Self::Error>;
    fn can_walk_on_powder_snow(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn suppresses_sliding_down_ladder(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Callback-free actual actor instanceof Player and LivingEntity.jumping.
    fn is_player(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn jumping(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn flying_speed(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn controlling_passenger(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Option<Self::Passenger>, Self::Error>;
    fn passenger_is_player(&mut self, passenger: &Self::Passenger) -> Result<bool, Self::Error>;
}

// Java Math.min/max propagate NaN and distinguish signed zero; Rust min/max
// suppress a NaN operand. These are ordinary source operations, not libm trig.
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f64::from_bits(a.to_bits() | b.to_bits())
    } else if a <= b {
        a
    } else {
        b
    }
}
fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f64::from_bits(a.to_bits() & b.to_bits())
    } else if a >= b {
        a
    } else {
        b
    }
}
fn clamp(value: f64, lower: f64, upper: f64) -> f64 {
    if value < lower {
        lower
    } else {
        java_min(value, upper)
    }
}
fn horizontal_distance(value: Vec3) -> f64 {
    (value.x * value.x + value.z * value.z).sqrt()
}

/// Original ordinary LivingEntity.travel. Calling this selected base body does
/// not authorize bypassing an actual subtype's travel override. No top-level
/// alive/removed/NoAI guard or new tick phase is introduced here.
pub fn travel<H: LivingTravelHost>(
    actor: &H::Actor,
    input: Vec3,
    host: &mut H,
) -> Result<(), TravelError<H::Error>> {
    if host.is_controlled_by_local_instance(actor)? {
        let mut gravity = host.gravity(actor)?;
        let falling = host.delta_movement(actor)?.y <= 0.0;
        if falling && host.has_effect(actor, TravelEffect::SlowFalling)? {
            gravity = java_min(gravity, 0.01);
        }
        let level = host.level(actor)?;
        let at = host.block_position(actor)?;
        let fluid = host.fluid_state(&level, at)?;
        if host.is_in_water(actor)?
            && host.is_affected_by_fluids(actor)?
            && !host.can_stand_on_fluid(actor, &fluid)?
        {
            let old_y = host.y(actor)?;
            let mut drag = if host.is_sprinting(actor)? {
                0.9_f32
            } else {
                host.water_slow_down(actor)?
            };
            let mut acceleration = 0.02_f32;
            let mut efficiency = host.water_movement_efficiency(actor)? as f32;
            if !host.on_ground(actor)? {
                efficiency *= 0.5_f32;
            }
            if efficiency > 0.0 {
                drag += (0.54600006_f32 - drag) * efficiency;
                acceleration += (host.speed(actor)? - acceleration) * efficiency;
            }
            if host.has_effect(actor, TravelEffect::DolphinsGrace)? {
                drag = 0.96_f32;
            }
            host.move_relative(actor, acceleration, input)?;
            let movement = host.delta_movement(actor)?;
            host.move_self(actor, movement)?;
            let mut velocity = host.delta_movement(actor)?;
            if host.horizontal_collision(actor)? && host.on_climbable(actor)? {
                velocity = Vec3::new(velocity.x, 0.2, velocity.z);
            }
            host.set_delta_movement(
                actor,
                Vec3::new(
                    velocity.x * f64::from(drag),
                    velocity.y * 0.800000011920929,
                    velocity.z * f64::from(drag),
                ),
            )?;
            let current = host.delta_movement(actor)?;
            let adjusted =
                host.fluid_falling_adjusted_movement(actor, gravity, falling, current)?;
            host.set_delta_movement(actor, adjusted)?;
            liquid_escape(actor, adjusted, old_y, host)?;
        } else if host.is_in_lava(actor)?
            && host.is_affected_by_fluids(actor)?
            && !host.can_stand_on_fluid(actor, &fluid)?
        {
            let old_y = host.y(actor)?;
            host.move_relative(actor, 0.02, input)?;
            let movement = host.delta_movement(actor)?;
            host.move_self(actor, movement)?;
            let fluid_height = host.fluid_height(actor, FluidTag::Lava)?;
            let threshold = host.fluid_jump_threshold(actor)?;
            if fluid_height <= threshold {
                let velocity = host.delta_movement(actor)?;
                host.set_delta_movement(
                    actor,
                    Vec3::new(
                        velocity.x * 0.5,
                        velocity.y * 0.800000011920929,
                        velocity.z * 0.5,
                    ),
                )?;
                let current = host.delta_movement(actor)?;
                let adjusted =
                    host.fluid_falling_adjusted_movement(actor, gravity, falling, current)?;
                host.set_delta_movement(actor, adjusted)?;
            } else {
                let velocity = host.delta_movement(actor)?;
                host.set_delta_movement(actor, velocity.scale(0.5))?;
            }
            if gravity != 0.0 {
                let velocity = host.delta_movement(actor)?;
                host.set_delta_movement(actor, velocity.plus(Vec3::new(0.0, -gravity / 4.0, 0.0)))?;
            }
            let adjusted = host.delta_movement(actor)?;
            liquid_escape(actor, adjusted, old_y, host)?;
        } else if host.is_fall_flying(actor)? {
            host.check_slow_fall_distance(actor)?;
            let old_velocity = host.delta_movement(actor)?;
            let look = host.look_angle(actor)?;
            let pitch = host.pitch(actor)? * 0.017453292_f32;
            let look_horizontal = (look.x * look.x + look.z * look.z).sqrt();
            let old_horizontal = horizontal_distance(old_velocity);
            let look_length = look.length();
            let lift = host.java_math_cos(f64::from(pitch))?;
            let lift = lift * lift * java_min(1.0, look_length / 0.4);
            // old_velocity is only the impact-speed local. Source rereads
            // current delta AFTER look/pitch callbacks for this first update.
            let current = host.delta_movement(actor)?;
            let mut velocity = current.plus(Vec3::new(0.0, gravity * (-1.0 + lift * 0.75), 0.0));
            if velocity.y < 0.0 && look_horizontal > 0.0 {
                let amount = velocity.y * -0.1 * lift;
                velocity = velocity.plus(Vec3::new(
                    look.x * amount / look_horizontal,
                    amount,
                    look.z * amount / look_horizontal,
                ));
            }
            if pitch < 0.0 && look_horizontal > 0.0 {
                // Negate AFTER the lookup, as in fneg. sin(-pitch) differs for
                // this discretized table and is not an equivalent expression.
                let amount =
                    old_horizontal * f64::from(-crate::geometry::vanilla_sin(pitch)) * 0.04;
                velocity = velocity.plus(Vec3::new(
                    -look.x * amount / look_horizontal,
                    amount * 3.2,
                    -look.z * amount / look_horizontal,
                ));
            }
            if look_horizontal > 0.0 {
                velocity = velocity.plus(Vec3::new(
                    (look.x / look_horizontal * old_horizontal - velocity.x) * 0.1,
                    0.0,
                    (look.z / look_horizontal * old_horizontal - velocity.z) * 0.1,
                ));
            }
            host.set_delta_movement(
                actor,
                Vec3::new(
                    velocity.x * 0.9900000095367432,
                    velocity.y * 0.9800000190734863,
                    velocity.z * 0.9900000095367432,
                ),
            )?;
            let movement = host.delta_movement(actor)?;
            host.move_self(actor, movement)?;
            if host.horizontal_collision(actor)? {
                let level = host.level(actor)?;
                if !host.level_is_client_side(&level)? {
                    let velocity = host.delta_movement(actor)?;
                    let new_horizontal = horizontal_distance(velocity);
                    let lost = old_horizontal - new_horizontal;
                    let damage = (lost * 10.0 - 3.0) as f32;
                    if damage > 0.0 {
                        let sound = host.fall_damage_sound(actor, damage as i32)?;
                        host.play_sound(actor, &sound, 1.0, 1.0)?;
                        let sources = host.damage_sources(actor)?;
                        let source = host.fly_into_wall_source(&sources)?;
                        let _ = host.hurt(actor, &source, damage)?;
                    }
                }
            }
            if host.on_ground(actor)? {
                let level = host.level(actor)?;
                if !host.level_is_client_side(&level)? {
                    host.set_shared_flag(actor, 7, false)?;
                }
            }
        } else {
            let at = host.movement_friction_position(actor)?;
            let level = host.level(actor)?;
            let state = host.block_state(&level, at)?;
            let block = host.state_block(&state)?;
            let friction = host.block_friction(&block)?;
            let drag = if host.on_ground(actor)? {
                friction * 0.91_f32
            } else {
                0.91_f32
            };
            let velocity = host.relative_friction_movement(actor, input, friction)?;
            let mut y = velocity.y;
            if host.has_effect(actor, TravelEffect::Levitation)? {
                let effect = host
                    .effect(actor, TravelEffect::Levitation)?
                    .ok_or(TravelError::NullLevitation)?;
                let amplifier = host.effect_amplifier(&effect)?;
                y += (0.05 * f64::from(amplifier.wrapping_add(1)) - velocity.y) * 0.2;
            } else {
                let level = host.level(actor)?;
                let apply_gravity = if !host.level_is_client_side(&level)? {
                    true
                } else {
                    let level = host.level(actor)?;
                    host.level_has_chunk_at(&level, at)?
                };
                if apply_gravity {
                    y -= gravity;
                } else {
                    let current_y = host.y(actor)?;
                    let level = host.level(actor)?;
                    y = if current_y > f64::from(host.minimum_build_height(&level)?) {
                        -0.1
                    } else {
                        0.0
                    };
                }
            }
            if host.should_discard_friction(actor)? {
                host.set_delta_components(actor, velocity.x, y, velocity.z)?;
            } else {
                let x = velocity.x * f64::from(drag);
                let y = if host.is_flying_animal(actor)? {
                    y * f64::from(drag)
                } else {
                    y * 0.9800000190734863
                };
                let z = velocity.z * f64::from(drag);
                host.set_delta_components(actor, x, y, z)?;
            }
        }
    }
    let include_vertical = host.is_flying_animal(actor)?;
    host.calculate_entity_animation(actor, include_vertical)?;
    Ok(())
}

fn liquid_escape<H: LivingTravelHost>(
    actor: &H::Actor,
    velocity: Vec3,
    old_y: f64,
    host: &mut H,
) -> Result<(), H::Error> {
    if host.horizontal_collision(actor)? {
        let y = velocity.y + 0.6000000238418579 - host.y(actor)? + old_y;
        if host.is_free(actor, velocity.x, y, velocity.z)? {
            host.set_delta_components(actor, velocity.x, 0.30000001192092896, velocity.z)?;
        }
    }
    Ok(())
}

/// Selected base PUBLIC VIRTUAL body2397–2406. Its callers must use actual
/// virtual dispatch; ordinary Bat owners can select this implementation.
pub fn handle_relative_friction_and_calculate_movement<H: LivingTravelHost>(
    actor: &H::Actor,
    input: Vec3,
    friction: f32,
    host: &mut H,
) -> Result<Vec3, H::Error> {
    let speed = friction_influenced_speed(actor, friction, host)?;
    host.move_relative(actor, speed, input)?;
    let current = host.delta_movement(actor)?;
    let climbable = handle_on_climbable(actor, current, host)?;
    host.set_delta_movement(actor, climbable)?;
    let movement = host.delta_movement(actor)?;
    host.move_self(actor, movement)?;
    let velocity = host.delta_movement(actor)?;
    if host.horizontal_collision(actor)? || host.jumping(actor)? {
        let rise = if host.on_climbable(actor)? {
            true
        } else {
            let state = host.in_block_state(actor)?;
            host.state_is_block(&state, TravelBlock::PowderSnow)?
                && host.can_walk_on_powder_snow(actor)?
        };
        if rise {
            return Ok(Vec3::new(velocity.x, 0.2, velocity.z));
        }
    }
    Ok(velocity)
}

/// Selected base PUBLIC VIRTUAL body2410–2420. Its input/gravity/falling are
/// source locals, including when isSprinting synchronously changes the actor.
pub fn get_fluid_falling_adjusted_movement<H: LivingTravelHost>(
    actor: &H::Actor,
    gravity: f64,
    falling: bool,
    movement: Vec3,
    host: &mut H,
) -> Result<Vec3, H::Error> {
    if gravity != 0.0 && !host.is_sprinting(actor)? {
        let y = if falling
            && (movement.y - 0.005).abs() >= 0.003
            && (movement.y - gravity / 16.0).abs() < 0.003
        {
            -0.003
        } else {
            movement.y - gravity / 16.0
        };
        Ok(Vec3::new(movement.x, y, movement.z))
    } else {
        Ok(movement)
    }
}

fn handle_on_climbable<H: LivingTravelHost>(
    actor: &H::Actor,
    movement: Vec3,
    host: &mut H,
) -> Result<Vec3, H::Error> {
    if host.on_climbable(actor)? {
        host.reset_fall_distance(actor)?;
        let x = clamp(movement.x, -0.15000000596046448, 0.15000000596046448);
        let z = clamp(movement.z, -0.15000000596046448, 0.15000000596046448);
        let mut y = java_max(movement.y, -0.15000000596046448);
        if y < 0.0 {
            let state = host.in_block_state(actor)?;
            if !host.state_is_block(&state, TravelBlock::Scaffolding)?
                && host.suppresses_sliding_down_ladder(actor)?
                && host.is_player(actor)?
            {
                y = 0.0;
            }
        }
        Ok(Vec3::new(x, y, z))
    } else {
        Ok(movement)
    }
}

fn friction_influenced_speed<H: LivingTravelHost>(
    actor: &H::Actor,
    friction: f32,
    host: &mut H,
) -> Result<f32, H::Error> {
    if host.on_ground(actor)? {
        let speed = host.speed(actor)?;
        Ok(speed * (0.21600002_f32 / (friction * friction * friction)))
    } else {
        host.flying_speed(actor)
    }
}

/// Selected protected base getFlyingSpeed2450; actual subtype dispatch remains
/// the host's responsibility. FlyingAnimal/fall-flying are unrelated predicates.
pub fn get_flying_speed<H: LivingTravelHost>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<f32, H::Error> {
    if let Some(passenger) = host.controlling_passenger(actor)? {
        if host.passenger_is_player(&passenger)? {
            return Ok(host.speed(actor)? * 0.1_f32);
        }
    }
    Ok(0.02)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    // Controlled callback owners, deliberately NOT a production actor, world,
    // effect map, fluid/collision solver or complete Java Math provider.
    struct Actor {
        id: u32,
        velocity: Vec3,
        y: f64,
        horizontal: bool,
        grounded: bool,
        jumping: bool,
        removed: bool,
        flags: u8,
    }
    type Lease = Rc<RefCell<Actor>>;
    struct Hook {
        name: &'static str,
        occurrence: usize,
        callback: fn(&Lease, &mut Host) -> Result<(), &'static str>,
    }
    struct Host {
        owner: Lease,
        live: Lease,
        events: Vec<&'static str>,
        actor_ids: Vec<u32>,
        hooks: VecDeque<Hook>,
        fail: Option<&'static str>,
        controlled: bool,
        gravity: f64,
        water: bool,
        lava: bool,
        affected: bool,
        stand: bool,
        sprinting: bool,
        efficiency: f64,
        speed: f32,
        slow_falling: bool,
        dolphins: bool,
        levitation: bool,
        effect: Option<Rc<RefCell<i32>>>,
        fluid_height: f64,
        threshold: f64,
        free: bool,
        fall_flying: bool,
        look: Vec3,
        pitch: f32,
        client: bool,
        loaded: bool,
        minimum_y: i32,
        friction: f32,
        relative_result: Option<Vec3>,
        discard_friction: bool,
        flying_animal: bool,
        climbable: bool,
        powder_snow: bool,
        scaffolding: bool,
        walk_on_powder: bool,
        suppress_sliding: bool,
        player: bool,
        player_passenger: bool,
        set_vectors: Vec<Vec3>,
        set_components: Vec<Vec3>,
        relative_speeds: Vec<f32>,
        movements: Vec<Vec3>,
        free_offsets: Vec<Vec3>,
        animation: Vec<bool>,
        damage: Vec<f32>,
        cosine_arguments: Vec<f64>,
        fluid_arguments: Vec<(f64, bool, Vec3)>,
    }
    impl Host {
        fn new(velocity: Vec3) -> Self {
            let owner = Rc::new(RefCell::new(Actor {
                id: 1,
                velocity,
                y: 0.0,
                horizontal: false,
                grounded: false,
                jumping: false,
                removed: false,
                flags: 128,
            }));
            Self {
                owner: owner.clone(),
                live: owner,
                events: vec![],
                actor_ids: vec![],
                hooks: VecDeque::new(),
                fail: None,
                controlled: true,
                gravity: 0.08,
                water: false,
                lava: false,
                affected: true,
                stand: false,
                sprinting: false,
                efficiency: 0.0,
                speed: 0.7,
                slow_falling: false,
                dolphins: false,
                levitation: false,
                effect: Some(Rc::new(RefCell::new(0))),
                fluid_height: 0.4,
                threshold: 0.4,
                free: false,
                fall_flying: false,
                look: Vec3::new(1.0, 0.0, 0.0),
                pitch: 0.0,
                client: false,
                loaded: true,
                minimum_y: -64,
                friction: 0.6,
                relative_result: None,
                discard_friction: false,
                flying_animal: false,
                climbable: false,
                powder_snow: false,
                scaffolding: false,
                walk_on_powder: false,
                suppress_sliding: false,
                player: false,
                player_passenger: false,
                set_vectors: vec![],
                set_components: vec![],
                relative_speeds: vec![],
                movements: vec![],
                free_offsets: vec![],
                animation: vec![],
                damage: vec![],
                cosine_arguments: vec![],
                fluid_arguments: vec![],
            }
        }
        fn hit(&mut self, actor: &Lease, name: &'static str) -> Result<(), &'static str> {
            assert!(
                actor.try_borrow_mut().is_ok(),
                "actor borrowed across callback {name}"
            );
            if let Some(effect) = &self.effect {
                assert!(
                    effect.try_borrow_mut().is_ok(),
                    "effect borrowed across callback {name}"
                );
            }
            self.events.push(name);
            self.actor_ids.push(actor.borrow().id);
            let count = self.events.iter().filter(|v| **v == name).count();
            if self
                .hooks
                .front()
                .is_some_and(|h| h.name == name && h.occurrence == count)
            {
                let hook = self.hooks.pop_front().unwrap();
                (hook.callback)(actor, self)?;
            }
            if self.fail == Some(name) {
                Err(name)
            } else {
                Ok(())
            }
        }
        fn hook(
            &mut self,
            name: &'static str,
            occurrence: usize,
            callback: fn(&Lease, &mut Host) -> Result<(), &'static str>,
        ) {
            self.hooks.push_back(Hook {
                name,
                occurrence,
                callback,
            });
        }
        fn index(&self, name: &str) -> usize {
            self.events.iter().position(|v| *v == name).unwrap()
        }
    }
    impl LivingTravelHost for Host {
        type Error = &'static str;
        type Actor = Lease;
        type Level = u32;
        type FluidState = u32;
        type BlockState = u32;
        type Block = u32;
        type EffectLease = Rc<RefCell<i32>>;
        type Passenger = u32;
        type Sound = u32;
        type DamageSources = u32;
        type DamageSource = u32;
        fn is_controlled_by_local_instance(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "controlled")?;
            Ok(self.controlled)
        }
        fn gravity(&mut self, a: &Lease) -> Result<f64, Self::Error> {
            self.hit(a, "gravity")?;
            Ok(self.gravity)
        }
        fn delta_movement(&mut self, a: &Lease) -> Result<Vec3, Self::Error> {
            self.hit(a, "delta")?;
            Ok(a.borrow().velocity)
        }
        fn set_delta_movement(&mut self, a: &Lease, v: Vec3) -> Result<(), Self::Error> {
            a.borrow_mut().velocity = v;
            self.set_vectors.push(v);
            self.hit(a, "set_vector")
        }
        fn set_delta_components(
            &mut self,
            a: &Lease,
            x: f64,
            y: f64,
            z: f64,
        ) -> Result<(), Self::Error> {
            let v = Vec3::new(x, y, z);
            a.borrow_mut().velocity = v;
            self.set_components.push(v);
            self.hit(a, "set_components")
        }
        fn y(&mut self, a: &Lease) -> Result<f64, Self::Error> {
            self.hit(a, "y")?;
            Ok(a.borrow().y)
        }
        fn horizontal_collision(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "horizontal")?;
            Ok(a.borrow().horizontal)
        }
        fn on_ground(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "ground")?;
            Ok(a.borrow().grounded)
        }
        fn level(&mut self, a: &Lease) -> Result<u32, Self::Error> {
            self.hit(a, "level")?;
            Ok(10)
        }
        fn block_position(&mut self, a: &Lease) -> Result<BlockPosition, Self::Error> {
            self.hit(a, "block_position")?;
            Ok((1, 2, 3))
        }
        fn fluid_state(&mut self, l: &u32, p: BlockPosition) -> Result<u32, Self::Error> {
            assert_eq!((*l, p), (10, (1, 2, 3)));
            let a = self.owner.clone();
            self.hit(&a, "fluid_state")?;
            Ok(20)
        }
        fn is_in_water(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "water")?;
            Ok(self.water)
        }
        fn is_in_lava(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "lava")?;
            Ok(self.lava)
        }
        fn is_affected_by_fluids(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "affected")?;
            Ok(self.affected)
        }
        fn can_stand_on_fluid(&mut self, a: &Lease, f: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*f, 20);
            self.hit(a, "stand")?;
            Ok(self.stand)
        }
        fn is_sprinting(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "sprinting")?;
            Ok(self.sprinting)
        }
        fn water_slow_down(&mut self, a: &Lease) -> Result<f32, Self::Error> {
            self.hit(a, "water_drag")?;
            Ok(0.8)
        }
        fn water_movement_efficiency(&mut self, a: &Lease) -> Result<f64, Self::Error> {
            self.hit(a, "water_efficiency")?;
            Ok(self.efficiency)
        }
        fn speed(&mut self, a: &Lease) -> Result<f32, Self::Error> {
            self.hit(a, "speed")?;
            Ok(self.speed)
        }
        fn has_effect(&mut self, a: &Lease, e: TravelEffect) -> Result<bool, Self::Error> {
            let (name, v) = match e {
                TravelEffect::SlowFalling => ("slow_falling", self.slow_falling),
                TravelEffect::DolphinsGrace => ("dolphins", self.dolphins),
                TravelEffect::Levitation => ("levitation", self.levitation),
            };
            self.hit(a, name)?;
            Ok(v)
        }
        fn effect(
            &mut self,
            a: &Lease,
            e: TravelEffect,
        ) -> Result<Option<Self::EffectLease>, Self::Error> {
            assert_eq!(e, TravelEffect::Levitation);
            self.hit(a, "effect")?;
            Ok(self.effect.clone())
        }
        fn effect_amplifier(&mut self, e: &Self::EffectLease) -> Result<i32, Self::Error> {
            let a = self.owner.clone();
            self.hit(&a, "amplifier")?;
            Ok(*e.borrow())
        }
        fn move_relative(
            &mut self,
            a: &Lease,
            speed: f32,
            _input: Vec3,
        ) -> Result<(), Self::Error> {
            self.relative_speeds.push(speed);
            self.hit(a, "move_relative")
        }
        fn move_self(&mut self, a: &Lease, v: Vec3) -> Result<(), Self::Error> {
            self.movements.push(v);
            a.borrow_mut().y += v.y;
            self.hit(a, "move_self")
        }
        fn on_climbable(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "climbable")?;
            Ok(self.climbable)
        }
        fn fluid_falling_adjusted_movement(
            &mut self,
            a: &Lease,
            g: f64,
            f: bool,
            v: Vec3,
        ) -> Result<Vec3, Self::Error> {
            self.fluid_arguments.push((g, f, v));
            self.hit(a, "fluid_falling")?;
            get_fluid_falling_adjusted_movement(a, g, f, v, self)
        }
        fn is_free(&mut self, a: &Lease, x: f64, y: f64, z: f64) -> Result<bool, Self::Error> {
            self.free_offsets.push(Vec3::new(x, y, z));
            self.hit(a, "free")?;
            Ok(self.free)
        }
        fn fluid_height(&mut self, a: &Lease, t: FluidTag) -> Result<f64, Self::Error> {
            assert_eq!(t, FluidTag::Lava);
            self.hit(a, "fluid_height")?;
            Ok(self.fluid_height)
        }
        fn fluid_jump_threshold(&mut self, a: &Lease) -> Result<f64, Self::Error> {
            self.hit(a, "fluid_threshold")?;
            Ok(self.threshold)
        }
        fn is_fall_flying(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "fall_flying")?;
            Ok(self.fall_flying)
        }
        fn check_slow_fall_distance(&mut self, a: &Lease) -> Result<(), Self::Error> {
            self.hit(a, "check_slow_fall")
        }
        fn look_angle(&mut self, a: &Lease) -> Result<Vec3, Self::Error> {
            self.hit(a, "look")?;
            Ok(self.look)
        }
        fn pitch(&mut self, a: &Lease) -> Result<f32, Self::Error> {
            self.hit(a, "pitch")?;
            Ok(self.pitch)
        }
        fn java_math_cos(&mut self, r: f64) -> Result<f64, Self::Error> {
            self.cosine_arguments.push(r);
            if r.is_finite() && r.abs() <= f64::from(std::f32::consts::TAU) {
                Ok(crate::hotspot_math::sin_cos(r.abs()).1)
            } else {
                Err("unproven cosine input")
            }
        }
        fn level_is_client_side(&mut self, l: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*l, 10);
            let a = self.owner.clone();
            self.hit(&a, "client")?;
            Ok(self.client)
        }
        fn fall_damage_sound(&mut self, a: &Lease, d: i32) -> Result<u32, Self::Error> {
            assert_eq!(d, 17);
            self.hit(a, "fall_sound")?;
            Ok(30)
        }
        fn play_sound(&mut self, a: &Lease, s: &u32, v: f32, p: f32) -> Result<(), Self::Error> {
            assert_eq!((*s, v, p), (30, 1.0, 1.0));
            self.hit(a, "play_sound")
        }
        fn damage_sources(&mut self, a: &Lease) -> Result<u32, Self::Error> {
            self.hit(a, "damage_sources")?;
            Ok(40)
        }
        fn fly_into_wall_source(&mut self, s: &u32) -> Result<u32, Self::Error> {
            assert_eq!(*s, 40);
            let a = self.owner.clone();
            self.hit(&a, "wall_source")?;
            Ok(50)
        }
        fn hurt(&mut self, a: &Lease, s: &u32, v: f32) -> Result<bool, Self::Error> {
            assert_eq!(*s, 50);
            self.damage.push(v);
            self.hit(a, "hurt")?;
            Ok(false)
        }
        fn set_shared_flag(&mut self, a: &Lease, f: u8, v: bool) -> Result<(), Self::Error> {
            assert_eq!((f, v), (7, false));
            a.borrow_mut().flags &= !(1 << f);
            self.hit(a, "shared_flag")
        }
        fn movement_friction_position(&mut self, a: &Lease) -> Result<BlockPosition, Self::Error> {
            self.hit(a, "friction_position")?;
            Ok((4, 5, 6))
        }
        fn block_state(&mut self, l: &u32, p: BlockPosition) -> Result<u32, Self::Error> {
            assert_eq!((*l, p), (10, (4, 5, 6)));
            let a = self.owner.clone();
            self.hit(&a, "block_state")?;
            Ok(60)
        }
        fn state_block(&mut self, s: &u32) -> Result<u32, Self::Error> {
            assert_eq!(*s, 60);
            let a = self.owner.clone();
            self.hit(&a, "state_block")?;
            Ok(70)
        }
        fn block_friction(&mut self, b: &u32) -> Result<f32, Self::Error> {
            assert_eq!(*b, 70);
            let a = self.owner.clone();
            self.hit(&a, "friction")?;
            Ok(self.friction)
        }
        fn relative_friction_movement(
            &mut self,
            a: &Lease,
            i: Vec3,
            f: f32,
        ) -> Result<Vec3, Self::Error> {
            self.hit(a, "relative_friction")?;
            if let Some(v) = self.relative_result {
                Ok(v)
            } else {
                handle_relative_friction_and_calculate_movement(a, i, f, self)
            }
        }
        fn level_has_chunk_at(&mut self, l: &u32, p: BlockPosition) -> Result<bool, Self::Error> {
            assert_eq!((*l, p), (10, (4, 5, 6)));
            let a = self.owner.clone();
            self.hit(&a, "loaded")?;
            Ok(self.loaded)
        }
        fn minimum_build_height(&mut self, l: &u32) -> Result<i32, Self::Error> {
            assert_eq!(*l, 10);
            let a = self.owner.clone();
            self.hit(&a, "minimum_y")?;
            Ok(self.minimum_y)
        }
        fn should_discard_friction(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "discard")?;
            Ok(self.discard_friction)
        }
        fn is_flying_animal(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "flying_animal")?;
            Ok(self.flying_animal)
        }
        fn calculate_entity_animation(&mut self, a: &Lease, v: bool) -> Result<(), Self::Error> {
            self.animation.push(v);
            self.hit(a, "animation")
        }
        fn reset_fall_distance(&mut self, a: &Lease) -> Result<(), Self::Error> {
            self.hit(a, "reset_fall")
        }
        fn in_block_state(&mut self, a: &Lease) -> Result<u32, Self::Error> {
            self.hit(a, "in_block")?;
            Ok(80)
        }
        fn state_is_block(&mut self, s: &u32, b: TravelBlock) -> Result<bool, Self::Error> {
            assert_eq!(*s, 80);
            let a = self.owner.clone();
            let (n, v) = match b {
                TravelBlock::PowderSnow => ("powder_snow", self.powder_snow),
                TravelBlock::Scaffolding => ("scaffolding", self.scaffolding),
            };
            self.hit(&a, n)?;
            Ok(v)
        }
        fn can_walk_on_powder_snow(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "can_walk_powder")?;
            Ok(self.walk_on_powder)
        }
        fn suppresses_sliding_down_ladder(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "suppresses_sliding")?;
            Ok(self.suppress_sliding)
        }
        fn is_player(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "is_player")?;
            Ok(self.player)
        }
        fn jumping(&mut self, a: &Lease) -> Result<bool, Self::Error> {
            self.hit(a, "jumping")?;
            Ok(a.borrow().jumping)
        }
        fn flying_speed(&mut self, a: &Lease) -> Result<f32, Self::Error> {
            self.hit(a, "flying_speed")?;
            get_flying_speed(a, self)
        }
        fn controlling_passenger(&mut self, a: &Lease) -> Result<Option<u32>, Self::Error> {
            self.hit(a, "passenger")?;
            Ok(self.player_passenger.then_some(90))
        }
        fn passenger_is_player(&mut self, p: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*p, 90);
            let a = self.owner.clone();
            self.hit(&a, "passenger_player")?;
            Ok(self.player_passenger)
        }
    }
    fn bits(v: Vec3) -> [u64; 3] {
        [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
    }

    #[test]
    fn source_control_gate_still_animates_and_bat_uses_ordinary_vertical_drag() {
        let mut h = Host::new(Vec3::new(1.0, 2.0, 3.0));
        let a = h.live.clone();
        h.controlled = false;
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        assert_eq!(h.events, vec!["controlled", "flying_animal", "animation"]);
        assert_eq!(h.animation, vec![false]);
        h.controlled = true;
        h.events.clear();
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        let expected = Vec3::new(
            f64::from(0.91_f32),
            (2.0 - 0.08) * 0.9800000190734863,
            3.0 * f64::from(0.91_f32),
        );
        assert_eq!(bits(a.borrow().velocity), bits(expected));
        // Independent unchanged-original TravelOrderProbe, ordinary dry case.
        assert_eq!(
            bits(a.borrow().velocity),
            [
                0x3fed_1eb8_6000_0000,
                0x3ffe_1b08_a3d7_0a3d,
                0x4005_d70a_4800_0000
            ]
        );
        assert!(!h.events.contains(&"loaded"));
        assert_eq!(h.animation, vec![false, false]);
        assert!(h.index("move_self") < h.index("levitation"));
        assert!(h.index("set_components") < h.index("animation"));
    }

    #[test]
    fn water_current_velocity_after_setter_and_retained_escape_vector() {
        let mut h = Host::new(Vec3::new(1.0, -0.01, 2.0));
        let a = h.live.clone();
        h.water = true;
        h.efficiency = 1.0;
        h.dolphins = true;
        h.free = true;
        a.borrow_mut().horizontal = true;
        h.hook("set_vector", 1, |a, _| {
            a.borrow_mut().velocity = Vec3::new(9.0, 0.5, 7.0);
            Ok(())
        });
        h.hook("set_vector", 2, |a, _| {
            a.borrow_mut().velocity = Vec3::new(100.0, 100.0, 100.0);
            Ok(())
        });
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        assert_eq!(
            h.relative_speeds,
            vec![0.02_f32 + (0.7_f32 - 0.02_f32) * 0.5_f32]
        );
        assert_eq!(h.set_vectors[0].x.to_bits(), f64::from(0.96_f32).to_bits());
        assert_eq!(h.fluid_arguments[0], (0.08, true, Vec3::new(9.0, 0.5, 7.0)));
        assert_eq!(h.free_offsets[0].x, 9.0);
        assert_eq!(h.free_offsets[0].z, 7.0);
        assert_eq!(h.free_offsets[0].y.to_bits(), 0x3ff1_ae14_8147_ae15);
        assert_eq!(h.relative_speeds[0].to_bits(), 0x3eb8_51ec);
        assert_eq!(
            bits(a.borrow().velocity),
            bits(Vec3::new(9.0, 0.30000001192092896, 7.0))
        );
        assert!(!h.events.contains(&"lava"));
    }

    #[test]
    fn lava_threshold_equality_and_deep_branch_have_distinct_gravity_order() {
        let mut h = Host::new(Vec3::new(1.0, -0.01, 2.0));
        let a = h.live.clone();
        h.lava = true;
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        assert_eq!(h.fluid_arguments.len(), 1);
        assert_eq!(h.set_vectors.len(), 3);
        let y: f64 = (-0.01 * 0.800000011920929 - 0.08 / 16.0) - 0.08 / 4.0;
        assert_eq!(a.borrow().velocity.y.to_bits(), y.to_bits());
        assert_eq!(a.borrow().velocity.y.to_bits(), 0xbfa0_e560_428f_5c29);
        assert!(h.index("fluid_height") < h.index("fluid_threshold"));
        let mut h = Host::new(Vec3::new(1.0, -0.01, 2.0));
        let a = h.live.clone();
        h.lava = true;
        h.fluid_height = 0.5;
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        assert!(h.fluid_arguments.is_empty());
        assert_eq!(h.set_vectors.len(), 2);
        assert_eq!(
            bits(a.borrow().velocity),
            bits(Vec3::new(0.5, -0.01 * 0.5 - 0.08 / 4.0, 1.0))
        );
        assert_eq!(a.borrow().velocity.y.to_bits(), 0xbf99_9999_9999_999a);
    }

    #[test]
    fn dry_levitation_retains_returned_vector_wraps_amplifier_and_null_fails() {
        let mut h = Host::new(Vec3::ZERO);
        let a = h.live.clone();
        let v = Vec3::new(1.0, 2.0, 3.0);
        h.relative_result = Some(v);
        h.levitation = true;
        h.discard_friction = true;
        *h.effect.as_ref().unwrap().borrow_mut() = i32::MAX;
        h.hook("effect", 1, |a, _| {
            a.borrow_mut().velocity = Vec3::new(99.0, 99.0, 99.0);
            Ok(())
        });
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        let y = 2.0 + (0.05 * f64::from(i32::MIN) - 2.0) * 0.2;
        assert_eq!(bits(a.borrow().velocity), bits(Vec3::new(1.0, y, 3.0)));
        assert!(!h.events.contains(&"client"));
        let mut h = Host::new(Vec3::ZERO);
        let a = h.live.clone();
        h.relative_result = Some(v);
        h.levitation = true;
        h.effect = None;
        assert_eq!(
            travel(&a, Vec3::ZERO, &mut h),
            Err(TravelError::NullLevitation)
        );
        assert!(!h.events.contains(&"discard"));
        assert!(h.animation.is_empty());
        assert!(h.set_components.is_empty());
    }

    #[test]
    fn unloaded_client_fallback_and_discard_friction_preserve_source_gate() {
        let mut h = Host::new(Vec3::ZERO);
        let a = h.live.clone();
        h.client = true;
        h.loaded = false;
        h.relative_result = Some(Vec3::new(1.0, 2.0, 3.0));
        h.discard_friction = true;
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        assert_eq!(a.borrow().velocity, Vec3::new(1.0, -0.1, 3.0));
        assert!(h.index("client") < h.index("loaded"));
        assert!(h.index("loaded") < h.index("minimum_y"));
    }

    #[test]
    fn fall_flying_uses_old_impact_speed_but_fresh_velocity_after_look() {
        let mut h = Host::new(Vec3::new(1.0, 0.0, 0.0));
        let a = h.live.clone();
        h.fall_flying = true;
        h.hook("look", 1, |a, _| {
            a.borrow_mut().velocity = Vec3::new(4.0, 0.0, 0.0);
            Ok(())
        });
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        assert_eq!(h.cosine_arguments, vec![0.0]);
        assert_eq!(a.borrow().velocity.x.to_bits(), 0x400d_5179_3e64_c2f7);
        assert_eq!(a.borrow().velocity.y.to_bits(), 0xbf92_1038_624d_d2f2);
        assert_eq!(h.movements[0], h.set_vectors[0]);
        // Original Java Math.cos + source table sine at negative pitch. The
        // controlled native provider only promises its explicit bounded domain.
        let mut negative = Host::new(Vec3::new(1.0, 0.0, 0.0));
        let original = negative.live.clone();
        negative.fall_flying = true;
        negative.pitch = -30.0;
        travel(&original, Vec3::ZERO, &mut negative).unwrap();
        assert_eq!(
            bits(original.borrow().velocity),
            [0x3fef_2f44_4bfb_3d1b, 0x3f9f_bb96_643a_3496, 0]
        );
        let mut h = Host::new(Vec3::new(1.0, 0.0, 0.0));
        let a = h.live.clone();
        h.fall_flying = true;
        h.pitch = f32::NAN;
        assert_eq!(
            travel(&a, Vec3::ZERO, &mut h),
            Err(TravelError::Host("unproven cosine input"))
        );
        assert!(h.set_vectors.is_empty());
        assert!(h.animation.is_empty());
    }

    #[test]
    fn fall_impact_sound_hurt_flag_order_survives_removed_registry_replacement() {
        let mut h = Host::new(Vec3::new(2.0, 0.0, 0.0));
        let a = h.live.clone();
        h.fall_flying = true;
        h.hook("move_self", 1, |a, _| {
            let mut a = a.borrow_mut();
            a.velocity = Vec3::ZERO;
            a.horizontal = true;
            a.grounded = true;
            Ok(())
        });
        h.hook("hurt", 1, |a, h| {
            a.borrow_mut().removed = true;
            h.live = Rc::new(RefCell::new(Actor {
                id: 2,
                velocity: Vec3::ZERO,
                y: 0.0,
                horizontal: false,
                grounded: false,
                jumping: false,
                removed: false,
                flags: 128,
            }));
            Ok(())
        });
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        assert_eq!(h.damage, vec![17.0]);
        assert!(a.borrow().removed);
        assert_eq!(a.borrow().flags, 0);
        assert_eq!(h.live.borrow().flags, 128);
        let selected: Vec<_> = h
            .events
            .iter()
            .copied()
            .filter(|v| {
                [
                    "fall_sound",
                    "play_sound",
                    "damage_sources",
                    "wall_source",
                    "hurt",
                    "shared_flag",
                    "animation",
                ]
                .contains(v)
            })
            .collect();
        assert_eq!(
            selected,
            vec![
                "fall_sound",
                "play_sound",
                "damage_sources",
                "wall_source",
                "hurt",
                "shared_flag",
                "animation"
            ]
        );
        assert!(h.actor_ids.iter().all(|id| *id == 1));
    }

    #[test]
    fn same_actor_nested_travel_is_available_during_move_callback() {
        let mut h = Host::new(Vec3::new(1.0, 2.0, 3.0));
        let a = h.live.clone();
        h.relative_result = Some(Vec3::new(1.0, 2.0, 3.0));
        h.water = true;
        h.hook("move_self", 1, |a, h| {
            let water = h.water;
            h.water = false;
            travel(a, Vec3::ZERO, h).map_err(|_| "nested travel")?;
            h.water = water;
            Ok(())
        });
        travel(&a, Vec3::ZERO, &mut h).unwrap();
        assert_eq!(h.animation, vec![false, false]);
        assert!(h.actor_ids.iter().all(|id| *id == 1));
    }

    #[test]
    fn private_climbable_and_powder_snow_order_and_flying_speed_owner() {
        let mut h = Host::new(Vec3::new(1.0, -1.0, -1.0));
        let a = h.live.clone();
        h.climbable = true;
        h.suppress_sliding = true;
        let v =
            handle_relative_friction_and_calculate_movement(&a, Vec3::ZERO, 0.6, &mut h).unwrap();
        assert_eq!(
            v,
            Vec3::new(
                0.15000000596046448,
                -0.15000000596046448,
                -0.15000000596046448
            )
        );
        assert!(h.index("suppresses_sliding") < h.index("is_player"));
        assert!(h.index("reset_fall") < h.index("set_vector"));
        let mut h = Host::new(Vec3::new(1.0, -1.0, 2.0));
        let a = h.live.clone();
        a.borrow_mut().jumping = true;
        h.powder_snow = true;
        h.walk_on_powder = true;
        let v =
            handle_relative_friction_and_calculate_movement(&a, Vec3::ZERO, 0.6, &mut h).unwrap();
        assert_eq!(v, Vec3::new(1.0, 0.2, 2.0));
        assert_eq!(a.borrow().velocity.y, -1.0);
        assert!(h.index("powder_snow") < h.index("can_walk_powder"));
        h.player_passenger = true;
        assert_eq!(get_flying_speed(&a, &mut h).unwrap(), 0.7_f32 * 0.1_f32);
    }

    #[test]
    fn fluid_falling_special_snap_zero_nan_and_math_boundaries() {
        let mut h = Host::new(Vec3::ZERO);
        let a = h.live.clone();
        assert_eq!(
            get_fluid_falling_adjusted_movement(&a, 0.01, true, Vec3::ZERO, &mut h)
                .unwrap()
                .y,
            -0.003
        );
        assert_eq!(
            get_fluid_falling_adjusted_movement(&a, 0.01, false, Vec3::ZERO, &mut h)
                .unwrap()
                .y,
            -0.01 / 16.0
        );
        h.events.clear();
        let v = Vec3::new(-0.0, -0.0, 0.0);
        assert_eq!(
            bits(get_fluid_falling_adjusted_movement(&a, -0.0, true, v, &mut h).unwrap()),
            bits(v)
        );
        assert!(h.events.is_empty());
        assert!(
            get_fluid_falling_adjusted_movement(&a, f64::NAN, true, v, &mut h)
                .unwrap()
                .y
                .is_nan()
        );
        assert!(clamp(f64::NAN, -0.15, 0.15).is_nan());
        assert_eq!(java_min(0.0, -0.0).to_bits(), (-0.0_f64).to_bits());
        assert_eq!(java_max(-0.0, 0.0).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn callback_error_keeps_executed_velocity_prefix_without_animation_or_damage() {
        let mut h = Host::new(Vec3::new(2.0, 0.0, 0.0));
        let a = h.live.clone();
        h.fall_flying = true;
        h.fail = Some("play_sound");
        h.hook("move_self", 1, |a, _| {
            let mut a = a.borrow_mut();
            a.velocity = Vec3::ZERO;
            a.horizontal = true;
            a.grounded = true;
            Ok(())
        });
        assert_eq!(
            travel(&a, Vec3::ZERO, &mut h),
            Err(TravelError::Host("play_sound"))
        );
        assert_eq!(a.borrow().velocity, Vec3::ZERO);
        assert!(!h.set_vectors.is_empty());
        assert!(h.damage.is_empty());
        assert!(h.animation.is_empty());
        assert!(!h.events.contains(&"shared_flag"));
    }
}
