//! Original Java 1.21 LivingEntity.baseTick (395–506) and default air/void bodies.
//!
//! This executor owns no actor, health, counter, RNG, effect collection or tick
//! scheduler. Its caller retains one actual LivingEntity lifetime; every field
//! access and virtual operation below belongs to that same canonical object.
//! Host errors preserve the executed prefix, including world effects and field
//! writes. In particular, profiler pop is source work, not error cleanup.

/// Exact actor fields, rather than a second animation-state owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LivingFloatField {
    AttackAnim,
    OldAttackAnim,
    AnimationStep,
    OldAnimationStep,
    BodyYaw,
    OldBodyYaw,
    HeadYaw,
    OldHeadYaw,
    OldYaw,
    OldPitch,
}

/// These fields retain their existing LivingEntity/Entity/physical owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LivingIntField {
    HurtTime,
    InvulnerableTime,
    LastHurtByPlayerTime,
    TickCount,
    LastHurtByMobTimestamp,
}

/// Select an existing canonical source from the actual DamageSources owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentalDamage {
    InWall,
    OutsideBorder,
    Drown,
    FellOutOfWorld,
}

/// Nullable source fields are read again at their actual dereference boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NullReference {
    Vehicle,
    LastHurtMob,
    LastHurtByMob,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LivingBaseTickError<E> {
    Host(E),
    NullReference(NullReference),
}
impl<E> From<E> for LivingBaseTickError<E> {
    fn from(error: E) -> Self {
        Self::Host(error)
    }
}

/// Mandatory access to the actual actor and its real source owners.
///
/// `Actor` is a retained canonical Entity handle, initially a LivingEntity. It
/// must survive removal and ID reuse without redirecting to another lifetime.
/// Other handles retain their exact source receiver while argument callbacks
/// execute. Field access must release all actor/state borrows before returning;
/// inherited, enchantment, damage, death, effect, world and RNG callbacks get
/// the full context needed for normal virtual dispatch and synchronous reentry.
/// No method may substitute a successful no-op for an unavailable real owner.
///
/// Classification, immutable-value construction and raw field access perform
/// only their named source operation. In particular they must not manufacture
/// an actor, logical side, border, ability, air counter or effect scheduler.
pub trait LivingBaseTickHost {
    type Error;
    type Actor;
    type Level;
    type ServerLevel;
    type Border;
    type BoundingBox;
    type BlockPos;
    type BlockState;
    type Profiler;
    type Abilities;
    type DamageSources;
    type DamageSource;

    fn float_field(
        &mut self,
        actor: &Self::Actor,
        field: LivingFloatField,
    ) -> Result<f32, Self::Error>;
    fn set_float_field(
        &mut self,
        actor: &Self::Actor,
        field: LivingFloatField,
        value: f32,
    ) -> Result<(), Self::Error>;
    fn int_field(&mut self, actor: &Self::Actor, field: LivingIntField)
        -> Result<i32, Self::Error>;
    fn set_int_field(
        &mut self,
        actor: &Self::Actor,
        field: LivingIntField,
        value: i32,
    ) -> Result<(), Self::Error>;
    fn first_tick(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn sleeping_position(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Option<Self::BlockPos>, Self::Error>;
    /// Actual private setPosToBed: setPos(x+.5,y+.6875,z+.5), with its real
    /// Entity.setPos callbacks and bounding-box/position ownership.
    fn set_position_to_bed(
        &mut self,
        actor: &Self::Actor,
        position: &Self::BlockPos,
    ) -> Result<(), Self::Error>;
    fn level(&mut self, actor: &Self::Actor) -> Result<Self::Level, Self::Error>;
    /// instanceof/cast of this retained Level, not another current-level read.
    fn as_server_level(
        &mut self,
        level: &Self::Level,
    ) -> Result<Option<Self::ServerLevel>, Self::Error>;
    fn enchantment_tick_effects(
        &mut self,
        level: &Self::ServerLevel,
        actor: &Self::Actor,
    ) -> Result<(), Self::Error>;
    /// Actual invokespecial Entity.baseTick. This includes its own virtual
    /// dispatch (notably onBelowWorld); do not call a second living tick here.
    fn entity_base_tick(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn profiler(&mut self, level: &Self::Level) -> Result<Self::Profiler, Self::Error>;
    fn profiler_push(
        &mut self,
        profiler: &Self::Profiler,
        name: &'static str,
    ) -> Result<(), Self::Error>;
    fn profiler_pop(&mut self, profiler: &Self::Profiler) -> Result<(), Self::Error>;
    fn fire_immune(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn level_is_client_side(&mut self, level: &Self::Level) -> Result<bool, Self::Error>;
    /// Actual Entity.clearFire, distinct from extinguishFire below.
    fn clear_fire(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn is_alive(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_player(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_server_player(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_in_wall(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn world_border(&mut self, level: &Self::Level) -> Result<Self::Border, Self::Error>;
    /// Actual final Entity.getBoundingBox; callback-free canonical field read.
    fn bounding_box(&mut self, actor: &Self::Actor) -> Result<Self::BoundingBox, Self::Error>;
    fn border_is_within_bounds(
        &mut self,
        border: &Self::Border,
        bounds: &Self::BoundingBox,
    ) -> Result<bool, Self::Error>;
    fn distance_to_border(
        &mut self,
        border: &Self::Border,
        actor: &Self::Actor,
    ) -> Result<f64, Self::Error>;
    fn border_damage_safe_zone(&mut self, border: &Self::Border) -> Result<f64, Self::Error>;
    fn border_damage_per_block(&mut self, border: &Self::Border) -> Result<f64, Self::Error>;
    fn damage_sources(&mut self, actor: &Self::Actor) -> Result<Self::DamageSources, Self::Error>;
    fn environmental_damage_source(
        &mut self,
        sources: &Self::DamageSources,
        kind: EnvironmentalDamage,
    ) -> Result<Self::DamageSource, Self::Error>;
    /// Actual virtual hurt, including the correct Player/ServerPlayer wrapper,
    /// equipment, reduction, death and packet callbacks. Its bool is ignored
    /// at these source callsites; errors never turn into successful damage.
    fn hurt(
        &mut self,
        actor: &Self::Actor,
        source: &Self::DamageSource,
        amount: f32,
    ) -> Result<bool, Self::Error>;
    /// Actual Entity.isEyeInFluid(FluidTags.WATER).
    fn eye_in_water(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual final Entity.getX/getY/getZ; callback-free coordinate reads.
    fn x(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn y(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn eye_y(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn z(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    /// Actual immutable BlockPos.containing (Java Mth.floor semantics).
    fn block_position_containing(
        &mut self,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<Self::BlockPos, Self::Error>;
    fn block_state(
        &mut self,
        level: &Self::Level,
        position: &Self::BlockPos,
    ) -> Result<Self::BlockState, Self::Error>;
    fn is_bubble_column(&mut self, state: &Self::BlockState) -> Result<bool, Self::Error>;
    fn can_breathe_underwater(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual MobEffectUtil.hasWaterBreathing on the real effect owner.
    fn has_water_breathing(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn abilities(&mut self, player: &Self::Actor) -> Result<Self::Abilities, Self::Error>;
    fn abilities_invulnerable(&mut self, abilities: &Self::Abilities) -> Result<bool, Self::Error>;
    fn air_supply(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn max_air_supply(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_air_supply(&mut self, actor: &Self::Actor, value: i32) -> Result<(), Self::Error>;
    /// Actual virtual decreaseAirSupply/increaseAirSupply. Subclass overrides
    /// remain authoritative; default source bodies are exported below.
    fn decrease_air_supply(&mut self, actor: &Self::Actor, value: i32) -> Result<i32, Self::Error>;
    fn increase_air_supply(&mut self, actor: &Self::Actor, value: i32) -> Result<i32, Self::Error>;
    fn delta_movement(&mut self, actor: &Self::Actor) -> Result<[f64; 3], Self::Error>;
    /// One nextDouble from this exact actor's actual RandomSource.
    fn next_double(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    /// Actual Level.addParticle(BUBBLE,...), with that retained Level receiver
    /// and its source client/server behavior. Do not redirect it to sendParticles.
    fn add_bubble_particle(
        &mut self,
        level: &Self::Level,
        position: [f64; 3],
        velocity: [f64; 3],
    ) -> Result<(), Self::Error>;
    fn is_passenger(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn vehicle(&mut self, actor: &Self::Actor) -> Result<Option<Self::Actor>, Self::Error>;
    fn dismounts_underwater(&mut self, vehicle: &Self::Actor) -> Result<bool, Self::Error>;
    fn stop_riding(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn block_position(&mut self, actor: &Self::Actor) -> Result<Self::BlockPos, Self::Error>;
    fn last_position(&mut self, actor: &Self::Actor)
        -> Result<Option<Self::BlockPos>, Self::Error>;
    /// Actual Objects.equal: identity-or-BlockPos.equals, including null.
    fn block_positions_equal(
        &mut self,
        previous: Option<&Self::BlockPos>,
        current: &Self::BlockPos,
    ) -> Result<bool, Self::Error>;
    /// Store this exact immutable BlockPos in LivingEntity.lastPos, before the
    /// onChangedBlock callback; do not make an independent position owner.
    fn set_last_position(
        &mut self,
        actor: &Self::Actor,
        position: &Self::BlockPos,
    ) -> Result<(), Self::Error>;
    fn on_changed_block(
        &mut self,
        actor: &Self::Actor,
        level: &Self::ServerLevel,
        position: &Self::BlockPos,
    ) -> Result<(), Self::Error>;
    fn is_in_water_rain_or_bubble(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_in_powder_snow(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn extinguish_fire(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn is_dead_or_dying(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn should_tick_death(
        &mut self,
        level: &Self::Level,
        actor: &Self::Actor,
    ) -> Result<bool, Self::Error>;
    /// Actual virtual tickDeath, including its real counter/removal owner.
    fn tick_death(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Raw lastHurtByPlayer = null, NOT setLastHurtByPlayer (which resets time).
    fn clear_last_hurt_by_player_field(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn last_hurt_mob(&mut self, actor: &Self::Actor) -> Result<Option<Self::Actor>, Self::Error>;
    fn clear_last_hurt_mob_field(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn last_hurt_by_mob(&mut self, actor: &Self::Actor)
        -> Result<Option<Self::Actor>, Self::Error>;
    /// Actual virtual setLastHurtByMob(null), including timestamp mutation.
    fn set_last_hurt_by_mob_none(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Actual virtual LivingEntity.tickEffects at source line496, with the real
    /// live iterator, canonical instances/definitions, dirty metadata, attribute
    /// and packet owners. The earlier enchantment phase is a different call.
    fn tick_effects(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn yaw(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn pitch(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
}

fn damage<H: LivingBaseTickHost>(
    actor: &H::Actor,
    kind: EnvironmentalDamage,
    amount: f32,
    host: &mut H,
) -> Result<(), H::Error> {
    let sources = host.damage_sources(actor)?;
    let source = host.environmental_damage_source(&sources, kind)?;
    host.hurt(actor, &source, amount)?;
    Ok(())
}

/// Execute btn.aw395–506 once on the retained actual actor. No liveness check
/// is added around work which the original continues after lethal damage.
pub fn base_tick<H: LivingBaseTickHost>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), LivingBaseTickError<H::Error>> {
    let attack = host.float_field(actor, LivingFloatField::AttackAnim)?;
    host.set_float_field(actor, LivingFloatField::OldAttackAnim, attack)?;
    if host.first_tick(actor)? {
        if let Some(position) = host.sleeping_position(actor)? {
            host.set_position_to_bed(actor, &position)?;
        }
    }
    let level = host.level(actor)?;
    if let Some(server_level) = host.as_server_level(&level)? {
        host.enchantment_tick_effects(&server_level, actor)?;
    }
    host.entity_base_tick(actor)?;
    let level = host.level(actor)?;
    let profiler = host.profiler(&level)?;
    host.profiler_push(&profiler, "livingEntityBaseTick")?;
    if host.fire_immune(actor)? || {
        let level = host.level(actor)?;
        host.level_is_client_side(&level)?
    } {
        host.clear_fire(actor)?;
    }
    if host.is_alive(actor)? {
        let is_player = host.is_player(actor)?;
        let level = host.level(actor)?;
        if !host.level_is_client_side(&level)? {
            if host.is_in_wall(actor)? {
                damage(actor, EnvironmentalDamage::InWall, 1.0, host)?;
            } else if is_player {
                let level = host.level(actor)?;
                let border = host.world_border(&level)?;
                let bounds = host.bounding_box(actor)?;
                if !host.border_is_within_bounds(&border, &bounds)? {
                    let level = host.level(actor)?;
                    let border = host.world_border(&level)?;
                    let distance = host.distance_to_border(&border, actor)?;
                    let level = host.level(actor)?;
                    let border = host.world_border(&level)?;
                    let distance = distance + host.border_damage_safe_zone(&border)?;
                    if distance < 0.0 {
                        let level = host.level(actor)?;
                        let border = host.world_border(&level)?;
                        let damage_per_block = host.border_damage_per_block(&border)?;
                        if damage_per_block > 0.0 {
                            let amount = java_floor(-distance * damage_per_block).max(1) as f32;
                            damage(actor, EnvironmentalDamage::OutsideBorder, amount, host)?;
                        }
                    }
                }
            }
        }
        let submerged_outside_bubble = if host.eye_in_water(actor)? {
            let level = host.level(actor)?;
            let x = host.x(actor)?;
            let y = host.eye_y(actor)?;
            let z = host.z(actor)?;
            let position = host.block_position_containing(x, y, z)?;
            let state = host.block_state(&level, &position)?;
            !host.is_bubble_column(&state)?
        } else {
            false
        };
        if submerged_outside_bubble {
            let needs_air = !host.can_breathe_underwater(actor)?
                && !host.has_water_breathing(actor)?
                && (!is_player || {
                    let abilities = host.abilities(actor)?;
                    !host.abilities_invulnerable(&abilities)?
                });
            if needs_air {
                let air = host.air_supply(actor)?;
                let air = host.decrease_air_supply(actor, air)?;
                host.set_air_supply(actor, air)?;
                if host.air_supply(actor)? == -20 {
                    host.set_air_supply(actor, 0)?;
                    let velocity = host.delta_movement(actor)?;
                    for _ in 0..8 {
                        let x1 = host.next_double(actor)?;
                        let x2 = host.next_double(actor)?;
                        let dx = x1 - x2;
                        let y1 = host.next_double(actor)?;
                        let y2 = host.next_double(actor)?;
                        let dy = y1 - y2;
                        let z1 = host.next_double(actor)?;
                        let z2 = host.next_double(actor)?;
                        let dz = z1 - z2;
                        let level = host.level(actor)?;
                        let x = host.x(actor)? + dx;
                        let y = host.y(actor)? + dy;
                        let z = host.z(actor)? + dz;
                        host.add_bubble_particle(&level, [x, y, z], velocity)?;
                    }
                    damage(actor, EnvironmentalDamage::Drown, 2.0, host)?;
                }
            }
            let level = host.level(actor)?;
            if !host.level_is_client_side(&level)?
                && host.is_passenger(actor)?
                && host.vehicle(actor)?.is_some()
            {
                let vehicle = host
                    .vehicle(actor)?
                    .ok_or(LivingBaseTickError::NullReference(NullReference::Vehicle))?;
                if host.dismounts_underwater(&vehicle)? {
                    host.stop_riding(actor)?;
                }
            }
        } else {
            let air = host.air_supply(actor)?;
            let maximum = host.max_air_supply(actor)?;
            if air < maximum {
                let air = host.air_supply(actor)?;
                let air = host.increase_air_supply(actor, air)?;
                host.set_air_supply(actor, air)?;
            }
        }
        let level = host.level(actor)?;
        if let Some(server_level) = host.as_server_level(&level)? {
            let position = host.block_position(actor)?;
            let last = host.last_position(actor)?;
            if !host.block_positions_equal(last.as_ref(), &position)? {
                host.set_last_position(actor, &position)?;
                host.on_changed_block(actor, &server_level, &position)?;
            }
        }
    }
    if host.is_alive(actor)?
        && (host.is_in_water_rain_or_bubble(actor)? || host.is_in_powder_snow(actor)?)
    {
        host.extinguish_fire(actor)?;
    }
    if host.int_field(actor, LivingIntField::HurtTime)? > 0 {
        let value = host
            .int_field(actor, LivingIntField::HurtTime)?
            .wrapping_sub(1);
        host.set_int_field(actor, LivingIntField::HurtTime, value)?;
    }
    if host.int_field(actor, LivingIntField::InvulnerableTime)? > 0
        && !host.is_server_player(actor)?
    {
        let value = host
            .int_field(actor, LivingIntField::InvulnerableTime)?
            .wrapping_sub(1);
        host.set_int_field(actor, LivingIntField::InvulnerableTime, value)?;
    }
    if host.is_dead_or_dying(actor)? {
        let level = host.level(actor)?;
        if host.should_tick_death(&level, actor)? {
            host.tick_death(actor)?;
        }
    }
    if host.int_field(actor, LivingIntField::LastHurtByPlayerTime)? > 0 {
        let value = host
            .int_field(actor, LivingIntField::LastHurtByPlayerTime)?
            .wrapping_sub(1);
        host.set_int_field(actor, LivingIntField::LastHurtByPlayerTime, value)?;
    } else {
        host.clear_last_hurt_by_player_field(actor)?;
    }
    if host.last_hurt_mob(actor)?.is_some() {
        let other = host
            .last_hurt_mob(actor)?
            .ok_or(LivingBaseTickError::NullReference(
                NullReference::LastHurtMob,
            ))?;
        if !host.is_alive(&other)? {
            host.clear_last_hurt_mob_field(actor)?;
        }
    }
    if host.last_hurt_by_mob(actor)?.is_some() {
        let other = host
            .last_hurt_by_mob(actor)?
            .ok_or(LivingBaseTickError::NullReference(
                NullReference::LastHurtByMob,
            ))?;
        if !host.is_alive(&other)? {
            host.set_last_hurt_by_mob_none(actor)?;
        } else {
            let now = host.int_field(actor, LivingIntField::TickCount)?;
            let then = host.int_field(actor, LivingIntField::LastHurtByMobTimestamp)?;
            if now.wrapping_sub(then) > 100 {
                host.set_last_hurt_by_mob_none(actor)?;
            }
        }
    }
    host.tick_effects(actor)?;
    for (current, previous) in [
        (
            LivingFloatField::AnimationStep,
            LivingFloatField::OldAnimationStep,
        ),
        (LivingFloatField::BodyYaw, LivingFloatField::OldBodyYaw),
        (LivingFloatField::HeadYaw, LivingFloatField::OldHeadYaw),
    ] {
        let value = host.float_field(actor, current)?;
        host.set_float_field(actor, previous, value)?;
    }
    let yaw = host.yaw(actor)?;
    host.set_float_field(actor, LivingFloatField::OldYaw, yaw)?;
    let pitch = host.pitch(actor)?;
    host.set_float_field(actor, LivingFloatField::OldPitch, pitch)?;
    let level = host.level(actor)?;
    let profiler = host.profiler(&level)?;
    host.profiler_pop(&profiler)?;
    Ok(())
}

fn java_floor(value: f64) -> i32 {
    let truncated = value as i32;
    if value < f64::from(truncated) {
        truncated.wrapping_sub(1)
    } else {
        truncated
    }
}

/// Additional actual owners needed only by the default LivingEntity air body.
pub trait LivingAirSupplyHost: LivingBaseTickHost {
    type AttributeInstance;
    /// Actual getAttribute(Attributes.OXYGEN_BONUS), preserving absence.
    fn oxygen_bonus_attribute(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Option<Self::AttributeInstance>, Self::Error>;
    fn attribute_value(&mut self, attribute: &Self::AttributeInstance) -> Result<f64, Self::Error>;
}

/// Default LivingEntity.decreaseAirSupply587–600. No RNG is consumed when the
/// actual attribute is absent or its value is not positive (including NaN).
pub fn decrease_air_supply<H: LivingAirSupplyHost>(
    actor: &H::Actor,
    air: i32,
    host: &mut H,
) -> Result<i32, H::Error> {
    let bonus = match host.oxygen_bonus_attribute(actor)? {
        Some(attribute) => host.attribute_value(&attribute)?,
        None => 0.0,
    };
    if bonus > 0.0 && host.next_double(actor)? >= 1.0 / (bonus + 1.0) {
        return Ok(air);
    }
    Ok(air.wrapping_sub(1))
}

/// Default LivingEntity.increaseAirSupply604; source integer addition wraps.
pub fn increase_air_supply<H: LivingBaseTickHost>(
    actor: &H::Actor,
    air: i32,
    host: &mut H,
) -> Result<i32, H::Error> {
    Ok(air.wrapping_add(4).min(host.max_air_supply(actor)?))
}

/// LivingEntity.onBelowWorld1953–1954. The real Entity.baseTick calls this
/// virtual override at its own source phase; this is not an extra tick hook.
pub fn on_below_world<H: LivingBaseTickHost>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    damage(actor, EnvironmentalDamage::FellOutOfWorld, 4.0, host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    type Actor = Rc<RefCell<State>>;
    struct State {
        floats: [f32; 10],
        ints: [i32; 5],
        level: usize,
        first: bool,
        sleeping: Option<i32>,
        alive: bool,
        player: bool,
        server_player: bool,
        wall: bool,
        water: bool,
        breathes: bool,
        water_effect: bool,
        invulnerable_ability: bool,
        air: i32,
        max_air: i32,
        velocity: [f64; 3],
        position: [f64; 3],
        block: i32,
        last_block: Option<i32>,
        wet: bool,
        powder: bool,
        passenger: bool,
        vehicle: Option<Actor>,
        last_mob: Option<Actor>,
        last_by_mob: Option<Actor>,
        last_player: bool,
    }
    impl State {
        fn actor() -> Actor {
            Rc::new(RefCell::new(Self {
                floats: [1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0, 0.0, 0.0],
                ints: [0, 0, 0, 101, 0],
                level: 1,
                first: false,
                sleeping: None,
                alive: true,
                player: false,
                server_player: false,
                wall: false,
                water: false,
                breathes: false,
                water_effect: false,
                invulnerable_ability: false,
                air: 300,
                max_air: 300,
                velocity: [1.0, 2.0, 3.0],
                position: [10.0, 20.0, 30.0],
                block: 7,
                last_block: Some(7),
                wet: false,
                powder: false,
                passenger: false,
                vehicle: None,
                last_mob: None,
                last_by_mob: None,
                last_player: true,
            }))
        }
    }
    struct Host {
        calls: Vec<String>,
        particles: Vec<(usize, [f64; 3], [f64; 3])>,
        random_count: usize,
        rng_moves_actor: bool,
        kill_on_hurt: bool,
        enchant_changes_level: bool,
        parent_calls_void: bool,
        effects_reenter: bool,
        effects_fail: bool,
        border_inside: bool,
        distance: f64,
        safe_zone: f64,
        damage_per_block: f64,
        border_changes_level: Option<Actor>,
        vehicle_reads: usize,
        vehicle_becomes_null: bool,
        oxygen: Option<f64>,
        random_value: f64,
    }
    impl Host {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                particles: Vec::new(),
                random_count: 0,
                rng_moves_actor: false,
                kill_on_hurt: false,
                enchant_changes_level: false,
                parent_calls_void: false,
                effects_reenter: false,
                effects_fail: false,
                border_inside: true,
                distance: -2.0,
                safe_zone: 0.5,
                damage_per_block: 2.0,
                border_changes_level: None,
                vehicle_reads: 0,
                vehicle_becomes_null: false,
                oxygen: None,
                random_value: 0.5,
            }
        }
        fn call(&mut self, name: impl Into<String>) {
            self.calls.push(name.into());
        }
        fn index(&self, name: &str) -> usize {
            self.calls.iter().position(|s| s == name).unwrap()
        }
    }
    impl LivingBaseTickHost for Host {
        type Error = &'static str;
        type Actor = Actor;
        type Level = usize;
        type ServerLevel = usize;
        type Border = usize;
        type BoundingBox = ();
        type BlockPos = i32;
        type BlockState = bool;
        type Profiler = usize;
        type Abilities = bool;
        type DamageSources = ();
        type DamageSource = EnvironmentalDamage;

        fn float_field(&mut self, a: &Actor, f: LivingFloatField) -> Result<f32, Self::Error> {
            self.call(format!("read:{f:?}"));
            Ok(a.borrow().floats[f as usize])
        }
        fn set_float_field(
            &mut self,
            a: &Actor,
            f: LivingFloatField,
            v: f32,
        ) -> Result<(), Self::Error> {
            self.call(format!("write:{f:?}:{v}"));
            a.borrow_mut().floats[f as usize] = v;
            Ok(())
        }
        fn int_field(&mut self, a: &Actor, f: LivingIntField) -> Result<i32, Self::Error> {
            self.call(format!("read:{f:?}"));
            Ok(a.borrow().ints[f as usize])
        }
        fn set_int_field(
            &mut self,
            a: &Actor,
            f: LivingIntField,
            v: i32,
        ) -> Result<(), Self::Error> {
            self.call(format!("write:{f:?}:{v}"));
            a.borrow_mut().ints[f as usize] = v;
            Ok(())
        }
        fn first_tick(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("first");
            Ok(a.borrow().first)
        }
        fn sleeping_position(&mut self, a: &Actor) -> Result<Option<i32>, Self::Error> {
            self.call("sleeping");
            Ok(a.borrow().sleeping)
        }
        fn set_position_to_bed(&mut self, _: &Actor, p: &i32) -> Result<(), Self::Error> {
            self.call(format!("bed:{p}"));
            Ok(())
        }
        fn level(&mut self, a: &Actor) -> Result<usize, Self::Error> {
            let l = a.borrow().level;
            self.call(format!("level:{l}"));
            Ok(l)
        }
        fn as_server_level(&mut self, l: &usize) -> Result<Option<usize>, Self::Error> {
            self.call(format!("server:{l}"));
            Ok((*l != 0).then_some(*l))
        }
        fn enchantment_tick_effects(&mut self, l: &usize, a: &Actor) -> Result<(), Self::Error> {
            self.call(format!("enchant:{l}"));
            if self.enchant_changes_level {
                a.borrow_mut().level = 2;
            }
            Ok(())
        }
        fn entity_base_tick(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.call("entity-base");
            a.borrow_mut().floats[LivingFloatField::AttackAnim as usize] = 9.0;
            if self.parent_calls_void {
                on_below_world(a, self)?;
            }
            Ok(())
        }
        fn profiler(&mut self, l: &usize) -> Result<usize, Self::Error> {
            self.call(format!("profiler:{l}"));
            Ok(*l)
        }
        fn profiler_push(&mut self, p: &usize, n: &'static str) -> Result<(), Self::Error> {
            self.call(format!("push:{p}:{n}"));
            Ok(())
        }
        fn profiler_pop(&mut self, p: &usize) -> Result<(), Self::Error> {
            self.call(format!("pop:{p}"));
            Ok(())
        }
        fn fire_immune(&mut self, _: &Actor) -> Result<bool, Self::Error> {
            self.call("fire-immune");
            Ok(false)
        }
        fn level_is_client_side(&mut self, l: &usize) -> Result<bool, Self::Error> {
            self.call(format!("client:{l}"));
            Ok(*l == 0)
        }
        fn clear_fire(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.call("clear-fire");
            Ok(())
        }
        fn is_alive(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("alive");
            Ok(a.borrow().alive)
        }
        fn is_player(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("player");
            Ok(a.borrow().player)
        }
        fn is_server_player(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("server-player");
            Ok(a.borrow().server_player)
        }
        fn is_in_wall(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("wall");
            Ok(a.borrow().wall)
        }
        fn world_border(&mut self, l: &usize) -> Result<usize, Self::Error> {
            self.call(format!("border:{l}"));
            Ok(*l)
        }
        fn bounding_box(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.call("bounds");
            Ok(())
        }
        fn border_is_within_bounds(&mut self, b: &usize, _: &()) -> Result<bool, Self::Error> {
            self.call(format!("inside:{b}"));
            if let Some(a) = &self.border_changes_level {
                a.borrow_mut().level = 2;
            }
            Ok(self.border_inside)
        }
        fn distance_to_border(&mut self, b: &usize, _: &Actor) -> Result<f64, Self::Error> {
            self.call(format!("distance:{b}"));
            Ok(self.distance)
        }
        fn border_damage_safe_zone(&mut self, b: &usize) -> Result<f64, Self::Error> {
            self.call(format!("safe:{b}"));
            Ok(self.safe_zone)
        }
        fn border_damage_per_block(&mut self, b: &usize) -> Result<f64, Self::Error> {
            self.call(format!("per-block:{b}"));
            Ok(self.damage_per_block)
        }
        fn damage_sources(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.call("damage-sources");
            Ok(())
        }
        fn environmental_damage_source(
            &mut self,
            _: &(),
            k: EnvironmentalDamage,
        ) -> Result<EnvironmentalDamage, Self::Error> {
            self.call(format!("source:{k:?}"));
            Ok(k)
        }
        fn hurt(
            &mut self,
            a: &Actor,
            s: &EnvironmentalDamage,
            v: f32,
        ) -> Result<bool, Self::Error> {
            self.call(format!("hurt:{s:?}:{v}"));
            if self.kill_on_hurt {
                a.borrow_mut().alive = false;
            }
            Ok(false)
        }
        fn eye_in_water(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("eye-water");
            Ok(a.borrow().water)
        }
        fn x(&mut self, a: &Actor) -> Result<f64, Self::Error> {
            self.call("x");
            Ok(a.borrow().position[0])
        }
        fn y(&mut self, a: &Actor) -> Result<f64, Self::Error> {
            self.call("y");
            Ok(a.borrow().position[1])
        }
        fn eye_y(&mut self, a: &Actor) -> Result<f64, Self::Error> {
            self.call("eye-y");
            Ok(a.borrow().position[1] + 1.0)
        }
        fn z(&mut self, a: &Actor) -> Result<f64, Self::Error> {
            self.call("z");
            Ok(a.borrow().position[2])
        }
        fn block_position_containing(
            &mut self,
            _: f64,
            _: f64,
            _: f64,
        ) -> Result<i32, Self::Error> {
            self.call("containing");
            Ok(7)
        }
        fn block_state(&mut self, _: &usize, _: &i32) -> Result<bool, Self::Error> {
            self.call("block-state");
            Ok(false)
        }
        fn is_bubble_column(&mut self, _: &bool) -> Result<bool, Self::Error> {
            self.call("bubble-column");
            Ok(false)
        }
        fn can_breathe_underwater(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("breathes");
            Ok(a.borrow().breathes)
        }
        fn has_water_breathing(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("water-effect");
            Ok(a.borrow().water_effect)
        }
        fn abilities(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("abilities");
            Ok(a.borrow().invulnerable_ability)
        }
        fn abilities_invulnerable(&mut self, v: &bool) -> Result<bool, Self::Error> {
            self.call("ability-invulnerable");
            Ok(*v)
        }
        fn air_supply(&mut self, a: &Actor) -> Result<i32, Self::Error> {
            self.call("air");
            Ok(a.borrow().air)
        }
        fn max_air_supply(&mut self, a: &Actor) -> Result<i32, Self::Error> {
            self.call("max-air");
            Ok(a.borrow().max_air)
        }
        fn set_air_supply(&mut self, a: &Actor, v: i32) -> Result<(), Self::Error> {
            self.call(format!("set-air:{v}"));
            a.borrow_mut().air = v;
            Ok(())
        }
        fn decrease_air_supply(&mut self, _: &Actor, v: i32) -> Result<i32, Self::Error> {
            self.call(format!("decrease:{v}"));
            Ok(v.wrapping_sub(1))
        }
        fn increase_air_supply(&mut self, _: &Actor, v: i32) -> Result<i32, Self::Error> {
            self.call(format!("increase:{v}"));
            Ok(v.wrapping_add(4))
        }
        fn delta_movement(&mut self, a: &Actor) -> Result<[f64; 3], Self::Error> {
            self.call("velocity");
            Ok(a.borrow().velocity)
        }
        fn next_double(&mut self, a: &Actor) -> Result<f64, Self::Error> {
            self.call("random-double");
            self.random_count += 1;
            if self.rng_moves_actor {
                let mut s = a.borrow_mut();
                s.position[0] += 1.0;
                s.velocity = [99.0; 3];
            }
            Ok(self.random_value)
        }
        fn add_bubble_particle(
            &mut self,
            l: &usize,
            p: [f64; 3],
            v: [f64; 3],
        ) -> Result<(), Self::Error> {
            self.call(format!("particle:{l}"));
            self.particles.push((*l, p, v));
            Ok(())
        }
        fn is_passenger(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("passenger");
            Ok(a.borrow().passenger)
        }
        fn vehicle(&mut self, a: &Actor) -> Result<Option<Actor>, Self::Error> {
            self.call("vehicle");
            self.vehicle_reads += 1;
            if self.vehicle_becomes_null && self.vehicle_reads == 2 {
                a.borrow_mut().vehicle = None;
            }
            Ok(a.borrow().vehicle.clone())
        }
        fn dismounts_underwater(&mut self, _: &Actor) -> Result<bool, Self::Error> {
            self.call("dismounts-underwater");
            Ok(true)
        }
        fn stop_riding(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.call("stop-riding");
            a.borrow_mut().passenger = false;
            Ok(())
        }
        fn block_position(&mut self, a: &Actor) -> Result<i32, Self::Error> {
            self.call("block-position");
            Ok(a.borrow().block)
        }
        fn last_position(&mut self, a: &Actor) -> Result<Option<i32>, Self::Error> {
            self.call("last-position");
            Ok(a.borrow().last_block)
        }
        fn block_positions_equal(&mut self, p: Option<&i32>, c: &i32) -> Result<bool, Self::Error> {
            self.call("positions-equal");
            Ok(p == Some(c))
        }
        fn set_last_position(&mut self, a: &Actor, p: &i32) -> Result<(), Self::Error> {
            self.call(format!("set-last:{p}"));
            a.borrow_mut().last_block = Some(*p);
            Ok(())
        }
        fn on_changed_block(&mut self, a: &Actor, l: &usize, p: &i32) -> Result<(), Self::Error> {
            assert_eq!(a.borrow().last_block, Some(*p));
            self.call(format!("changed:{l}:{p}"));
            Ok(())
        }
        fn is_in_water_rain_or_bubble(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("wet");
            Ok(a.borrow().wet)
        }
        fn is_in_powder_snow(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("powder");
            Ok(a.borrow().powder)
        }
        fn extinguish_fire(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.call("extinguish");
            Ok(())
        }
        fn is_dead_or_dying(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.call("dead-or-dying");
            Ok(!a.borrow().alive)
        }
        fn should_tick_death(&mut self, l: &usize, _: &Actor) -> Result<bool, Self::Error> {
            self.call(format!("should-death:{l}"));
            Ok(true)
        }
        fn tick_death(&mut self, _: &Actor) -> Result<(), Self::Error> {
            self.call("death-tick");
            Ok(())
        }
        fn clear_last_hurt_by_player_field(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.call("clear-player-field");
            a.borrow_mut().last_player = false;
            Ok(())
        }
        fn last_hurt_mob(&mut self, a: &Actor) -> Result<Option<Actor>, Self::Error> {
            self.call("last-mob");
            Ok(a.borrow().last_mob.clone())
        }
        fn clear_last_hurt_mob_field(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.call("clear-mob-field");
            a.borrow_mut().last_mob = None;
            Ok(())
        }
        fn last_hurt_by_mob(&mut self, a: &Actor) -> Result<Option<Actor>, Self::Error> {
            self.call("last-by-mob");
            Ok(a.borrow().last_by_mob.clone())
        }
        fn set_last_hurt_by_mob_none(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.call("set-by-mob-none");
            let mut s = a.borrow_mut();
            s.last_by_mob = None;
            s.ints[LivingIntField::LastHurtByMobTimestamp as usize] =
                s.ints[LivingIntField::TickCount as usize];
            Ok(())
        }
        fn tick_effects(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.call("effects");
            if self.effects_fail {
                return Err("effects");
            }
            if self.effects_reenter {
                self.effects_reenter = false;
                a.borrow_mut().floats[LivingFloatField::HeadYaw as usize] = 42.0;
                base_tick(a, self).map_err(|_| "reentry")?;
            }
            Ok(())
        }
        fn yaw(&mut self, _: &Actor) -> Result<f32, Self::Error> {
            self.call("yaw");
            Ok(12.0)
        }
        fn pitch(&mut self, _: &Actor) -> Result<f32, Self::Error> {
            self.call("pitch");
            Ok(13.0)
        }
    }
    impl LivingAirSupplyHost for Host {
        type AttributeInstance = f64;
        fn oxygen_bonus_attribute(&mut self, _: &Actor) -> Result<Option<f64>, Self::Error> {
            self.call("oxygen-attribute");
            Ok(self.oxygen)
        }
        fn attribute_value(&mut self, a: &f64) -> Result<f64, Self::Error> {
            self.call("attribute-value");
            Ok(*a)
        }
    }

    #[test]
    fn lethal_prefix_continues_air_rng_block_change_death_and_effects() {
        let actor = State::actor();
        {
            let mut s = actor.borrow_mut();
            s.wall = true;
            s.water = true;
            s.air = -19;
            s.last_block = None;
            s.wet = true;
            s.ints = [3, 20, 1, 101, 0];
        }
        let mut host = Host::new();
        host.kill_on_hurt = true;
        host.rng_moves_actor = true;
        base_tick(&actor, &mut host).unwrap();
        assert_eq!(host.random_count, 48);
        assert_eq!(host.particles.len(), 8);
        for (i, (level, position, velocity)) in host.particles.iter().enumerate() {
            assert_eq!(*level, 1);
            assert_eq!(*position, [16.0 + 6.0 * i as f64, 20.0, 30.0]);
            assert_eq!(*velocity, [1.0, 2.0, 3.0]);
        }
        assert!(host.index("hurt:InWall:1") < host.index("eye-water"));
        assert!(host.index("hurt:Drown:2") < host.index("changed:1:7"));
        assert!(host.index("changed:1:7") < host.index("death-tick"));
        assert!(host.index("death-tick") < host.index("effects"));
        assert!(!host.calls.iter().any(|s| s == "wet" || s == "extinguish"));
        assert!(!host.calls.iter().any(|s| s == "clear-player-field"));
        let state = actor.borrow();
        assert_eq!(state.ints[..3], [2, 19, 0]);
        assert!(state.last_player); // one->zero does not clear until next tick.
        assert_eq!(state.air, 0);
    }

    #[test]
    fn enchantment_precedes_parent_and_real_parent_void_dispatch_can_kill() {
        let actor = State::actor();
        {
            let mut s = actor.borrow_mut();
            s.first = true;
            s.sleeping = Some(11);
            s.server_player = true;
            s.ints[1] = 20;
        }
        let mut host = Host::new();
        host.enchant_changes_level = true;
        host.parent_calls_void = true;
        host.kill_on_hurt = true;
        base_tick(&actor, &mut host).unwrap();
        assert_eq!(
            &host.calls[..9],
            [
                "read:AttackAnim",
                "write:OldAttackAnim:1",
                "first",
                "sleeping",
                "bed:11",
                "level:1",
                "server:1",
                "enchant:1",
                "entity-base"
            ]
        );
        assert!(host.index("hurt:FellOutOfWorld:4") < host.index("push:2:livingEntityBaseTick"));
        assert!(!host.calls.iter().any(|s| s == "player" || s == "eye-water"));
        assert_eq!(actor.borrow().floats[1], 1.0);
        assert_eq!(actor.borrow().ints[1], 20); // ServerPlayer's counter belongs elsewhere.
        assert_eq!(host.calls.last().unwrap(), "pop:2");
    }

    #[test]
    fn border_query_receiver_is_retained_and_next_reads_use_new_level() {
        let actor = State::actor();
        actor.borrow_mut().player = true;
        let mut host = Host::new();
        host.border_inside = false;
        host.border_changes_level = Some(actor.clone());
        base_tick(&actor, &mut host).unwrap();
        assert!(host.index("border:1") < host.index("bounds"));
        assert!(host.index("bounds") < host.index("inside:1"));
        assert!(host.index("inside:1") < host.index("distance:2"));
        assert!(host.index("distance:2") < host.index("safe:2"));
        assert!(host.index("safe:2") < host.index("per-block:2"));
        assert!(host.calls.iter().any(|s| s == "hurt:OutsideBorder:3"));
        for distance in [0.0, f64::NAN] {
            host.calls.clear();
            host.distance = distance;
            host.safe_zone = 0.0;
            base_tick(&actor, &mut host).unwrap();
            assert!(!host
                .calls
                .iter()
                .any(|s| s.starts_with("hurt:OutsideBorder")));
        }
    }

    #[test]
    fn air_gates_short_circuit_and_vehicle_is_reread_before_dereference() {
        let actor = State::actor();
        {
            let mut s = actor.borrow_mut();
            s.water = true;
            s.breathes = true;
            s.air = -19;
            s.passenger = true;
            s.vehicle = Some(State::actor());
        }
        let mut host = Host::new();
        host.vehicle_becomes_null = true;
        assert_eq!(
            base_tick(&actor, &mut host),
            Err(LivingBaseTickError::NullReference(NullReference::Vehicle))
        );
        assert_eq!(host.vehicle_reads, 2);
        assert_eq!(host.random_count, 0);
        assert!(!host.calls.iter().any(|s| s == "water-effect"
            || s == "air"
            || s == "dismounts-underwater"
            || s == "effects"
            || s.starts_with("pop:")));
        assert_eq!(actor.borrow().air, -19);
    }

    #[test]
    fn actual_field_expiration_wraps_and_reentry_uses_same_canonical_actor() {
        let actor = State::actor();
        let dead = State::actor();
        dead.borrow_mut().alive = false;
        let live = State::actor();
        {
            let mut s = actor.borrow_mut();
            s.last_mob = Some(dead);
            s.last_by_mob = Some(live);
            s.ints[3] = i32::MIN;
            s.ints[4] = i32::MAX - 100;
        }
        let mut host = Host::new();
        host.effects_reenter = true;
        base_tick(&actor, &mut host).unwrap();
        assert!(actor.borrow().last_mob.is_none());
        assert!(actor.borrow().last_by_mob.is_none()); // wrapped elapsed101.
        assert_eq!(actor.borrow().ints[4], i32::MIN); // virtual setter side effect.
        assert_eq!(actor.borrow().floats[7], 42.0);
        assert_eq!(host.calls.iter().filter(|s| *s == "effects").count(), 2);
        assert_eq!(host.calls.iter().filter(|s| *s == "pop:1").count(), 2);
    }

    #[test]
    fn effect_error_keeps_counter_prefix_without_snapshot_or_profiler_cleanup() {
        let actor = State::actor();
        actor.borrow_mut().ints = [2, 2, 2, 0, 0];
        let mut host = Host::new();
        host.effects_fail = true;
        assert_eq!(
            base_tick(&actor, &mut host),
            Err(LivingBaseTickError::Host("effects"))
        );
        assert_eq!(&actor.borrow().ints[..3], [1, 1, 1]);
        assert_eq!(host.calls.last().unwrap(), "effects");
        assert!(!host
            .calls
            .iter()
            .any(|s| s == "read:AnimationStep" || s.starts_with("pop:")));
    }

    #[test]
    fn default_air_bodies_preserve_attribute_threshold_rng_and_wrapping() {
        let actor = State::actor();
        let mut host = Host::new();
        assert_eq!(
            decrease_air_supply(&actor, i32::MIN, &mut host),
            Ok(i32::MAX)
        );
        assert_eq!(host.calls, ["oxygen-attribute"]);
        for bonus in [0.0, -1.0, f64::NAN] {
            host.calls.clear();
            host.oxygen = Some(bonus);
            assert_eq!(decrease_air_supply(&actor, 20, &mut host), Ok(19));
            assert!(!host.calls.iter().any(|s| s == "random-double"));
        }
        host.oxygen = Some(1.0);
        host.random_value = 0.5;
        assert_eq!(decrease_air_supply(&actor, 20, &mut host), Ok(20));
        host.random_value = f64::from_bits(0.5_f64.to_bits() - 1);
        assert_eq!(decrease_air_supply(&actor, 20, &mut host), Ok(19));
        host.random_value = f64::NAN;
        assert_eq!(decrease_air_supply(&actor, 20, &mut host), Ok(19));
        assert_eq!(
            increase_air_supply(&actor, i32::MAX, &mut host),
            Ok(i32::MIN + 3)
        );
        assert_eq!(increase_air_supply(&actor, 299, &mut host), Ok(300));
    }
}
