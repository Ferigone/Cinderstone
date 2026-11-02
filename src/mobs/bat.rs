//! Bounded Java 1.21 Bat constructor/tick/AI/hurt behavior (`cez`, 40–225).
//!
//! BatState owns the source targetPosition and two animation states. The host
//! retains the actual metadata, physical/living actor, inherited methods, level,
//! targeting conditions and entity RNG. This is not an actor factory, scheduler,
//! navigation implementation or Main admission. Bat inherits genuinely empty
//! goal selectors and real GroundPathNavigation; those owners must still run.

pub type BlockPosition = (i32, i32, i32);
pub type Vector = [f64; 3];

/// The fragment of original AnimationState used by Bat.setupAnimationStates.
/// Animation update/render APIs are outside this bounded server tick fragment.
#[derive(Clone, Debug)]
pub struct BatAnimationState {
    last_time: i64,
    accumulated_time: i64,
}
impl BatAnimationState {
    fn new() -> Self {
        Self {
            last_time: i64::MAX,
            accumulated_time: 0,
        }
    }
    fn stop(&mut self) {
        self.last_time = i64::MAX;
    }
    fn start_if_stopped(&mut self, tick: i32) {
        if self.last_time == i64::MAX {
            self.last_time = i64::from(tick) * 1000 / 20;
            self.accumulated_time = 0;
        }
    }
    pub fn is_started(&self) -> bool {
        self.last_time != i64::MAX
    }
    pub fn last_time(&self) -> i64 {
        self.last_time
    }
    pub fn accumulated_time(&self) -> i64 {
        self.accumulated_time
    }
}

/// Mandatory virtual/owner operations. There are no successful dummy defaults.
/// Reads and mutations refer to the same source actor, including after an
/// inherited callback. Canonical block states must not be re-resolved against
/// replacement worlds. Errors preserve the already executed mutation prefix.
pub trait BatHost {
    type Error;
    type Source;
    type BlockState;

    /// Full AmbientCreature/Mob/LivingEntity/Entity constructor prefix, including
    /// actual metadata/attribute/RNG/selector/navigation/control construction.
    /// Source Mob flags default to NoAI=false; this callback must not substitute
    /// a creative player body or an empty navigation callback.
    fn inherited_construct(&mut self) -> Result<(), Self::Error>;
    fn inherited_define_synched_data(&mut self) -> Result<(), Self::Error>;
    fn define_bat_flags(&mut self, initial: u8) -> Result<(), Self::Error>;
    fn is_client_side(&mut self) -> Result<bool, Self::Error>;
    fn bat_flags(&mut self) -> Result<u8, Self::Error>;
    /// Actual SynchedEntityData setter, retaining its equality/dirty semantics.
    fn set_bat_flags(&mut self, flags: u8) -> Result<(), Self::Error>;
    fn inherited_tick(&mut self) -> Result<(), Self::Error>;
    fn inherited_custom_server_ai_step(&mut self) -> Result<(), Self::Error>;
    fn actor_tick_count(&mut self) -> Result<i32, Self::Error>;
    fn block_position(&mut self) -> Result<BlockPosition, Self::Error>;
    fn position(&mut self) -> Result<Vector, Self::Error>;
    fn x(&mut self) -> Result<f64, Self::Error>;
    fn y(&mut self) -> Result<f64, Self::Error>;
    fn z(&mut self) -> Result<f64, Self::Error>;
    fn bounding_box_height(&mut self) -> Result<f32, Self::Error>;
    fn delta_movement(&mut self) -> Result<Vector, Self::Error>;
    fn set_delta_movement(&mut self, value: Vector) -> Result<(), Self::Error>;
    /// Actual virtual setPos, including immediate physical/index callbacks.
    fn set_position(&mut self, value: Vector) -> Result<(), Self::Error>;
    fn is_silent(&mut self) -> Result<bool, Self::Error>;
    fn block_state(&mut self, at: BlockPosition) -> Result<Self::BlockState, Self::Error>;
    fn is_redstone_conductor(
        &mut self,
        state: &Self::BlockState,
        query_position: BlockPosition,
    ) -> Result<bool, Self::Error>;
    /// Actual level.getNearestPlayer(BAT_RESTING_TARGETING, this), using the
    /// static forNonCombat().range(4) conditions and original query order.
    fn has_nearest_resting_target_player(&mut self) -> Result<bool, Self::Error>;
    fn is_empty_block(&mut self, at: BlockPosition) -> Result<bool, Self::Error>;
    fn minimum_build_height(&mut self) -> Result<i32, Self::Error>;
    fn next_entity_int(&mut self, bound: i32) -> Result<i32, Self::Error>;
    fn set_head_yaw(&mut self, yaw: f32) -> Result<(), Self::Error>;
    fn yaw(&mut self) -> Result<f32, Self::Error>;
    fn set_yaw(&mut self, yaw: f32) -> Result<(), Self::Error>;
    fn set_forward_input(&mut self, input: f32) -> Result<(), Self::Error>;
    /// Actual levelEvent with excluded-player=null; do not buffer it past later
    /// callbacks or turn it into a privately invented sound packet.
    fn level_event(&mut self, event: i32, at: BlockPosition, data: i32) -> Result<(), Self::Error>;
    fn is_invulnerable_to(&mut self, source: &Self::Source) -> Result<bool, Self::Error>;
    fn inherited_hurt(&mut self, source: &Self::Source, amount: f32) -> Result<bool, Self::Error>;
}

pub fn define_synched_data<H: BatHost>(host: &mut H) -> Result<(), H::Error> {
    host.inherited_define_synched_data()?;
    host.define_bat_flags(0)
}

pub fn is_resting<H: BatHost>(host: &mut H) -> Result<bool, H::Error> {
    Ok(host.bat_flags()? & 1 != 0)
}

pub fn set_resting<H: BatHost>(host: &mut H, resting: bool) -> Result<(), H::Error> {
    let previous = host.bat_flags()?;
    host.set_bat_flags(if resting { previous | 1 } else { previous & !1 })
}

/// Bat.isFlapping, source line 57. Java converts tickCount to float before
/// remainder; integer tick modulo is observably different at large counts.
pub fn is_flapping<H: BatHost>(host: &mut H) -> Result<bool, H::Error> {
    Ok(!is_resting(host)? && (host.actor_tick_count()? as f32) % 10.0_f32 == 0.0)
}

/// Optional sound owner for the actual Bat's inherited tick/damage dispatch.
/// Sound values are canonical registered SoundEvent handles, not protocol IDs.
/// The inherited voice-pitch callback must retain its actual entity-RNG draws.
pub trait BatSoundHost: BatHost {
    type Sound;

    fn inherited_voice_pitch(&mut self) -> Result<f32, Self::Error>;
    fn bat_ambient_sound(&mut self) -> Result<Self::Sound, Self::Error>;
    fn bat_hurt_sound(&mut self) -> Result<Self::Sound, Self::Error>;
    fn bat_death_sound(&mut self) -> Result<Self::Sound, Self::Error>;
}

/// Source line 68, independent of the actor's health or current sound.
pub fn get_sound_volume() -> f32 {
    0.1
}

/// Source line 73. Evaluate the inherited source body and its draws first.
pub fn get_voice_pitch<H: BatSoundHost>(host: &mut H) -> Result<f32, H::Error> {
    Ok(host.inherited_voice_pitch()? * 0.95_f32)
}

/// Source lines 79–82. Flight consumes no nextInt(4); while resting, three of
/// four outcomes return null before resolving the actual ambient sound handle.
pub fn get_ambient_sound<H: BatSoundHost>(host: &mut H) -> Result<Option<H::Sound>, H::Error> {
    if is_resting(host)? && host.next_entity_int(4)? != 0 {
        return Ok(None);
    }
    Ok(Some(host.bat_ambient_sound()?))
}

/// Source line 87. The original override does not inspect its damage source.
pub fn get_hurt_sound<H: BatSoundHost>(
    _source: &H::Source,
    host: &mut H,
) -> Result<H::Sound, H::Error> {
    host.bat_hurt_sound()
}

/// Source line 92.
pub fn get_death_sound<H: BatSoundHost>(host: &mut H) -> Result<H::Sound, H::Error> {
    host.bat_death_sound()
}

/// Source line 98, the actual Bat override rather than a missing collision hook.
pub fn is_pushable() -> bool {
    false
}

/// The original Bat.doPush body at line 104 is empty, including inherited calls.
pub fn do_push<E>(_other: &E) {}

/// The original Bat.pushEntities body at line 109 is empty.
pub fn push_entities() {}

/// Source line 201 returns the same movement-emission type used by the actual
/// native Entity.move path; this enables game events and disables step sounds.
pub fn get_movement_emission() -> crate::entity_move::MovementEmission {
    crate::entity_move::MovementEmission::Events
}

/// The original checkFallDamage override at line 208 is empty. This does not
/// replace Entity.move or another species' actual fall-damage callback.
pub fn check_fall_damage<B>(
    _vertical_motion: f64,
    _on_ground: bool,
    _block: &B,
    _position: BlockPosition,
) {
}

/// Source line 212. The actual Entity movement owner still dispatches this fact.
pub fn is_ignoring_block_triggers() -> bool {
    true
}

/// Actual immutable AttributeSupplier.Builder construction, not an actor's
/// AttributeMap or a stand-in MAX_HEALTH definition. All callbacks are required.
pub trait BatAttributeTemplateHost {
    type Error;
    type Builder;
    type Attribute;

    fn create_mob_attributes(&mut self) -> Result<Self::Builder, Self::Error>;
    fn max_health_attribute(&mut self) -> Result<Self::Attribute, Self::Error>;
    /// AttributeSupplier.Builder.add on the supplied actual builder and holder;
    /// return that method's actual builder result, retaining its identity.
    fn builder_add(
        &mut self,
        builder: Self::Builder,
        attribute: &Self::Attribute,
        value: f64,
    ) -> Result<Self::Builder, Self::Error>;
}

/// Bat.createAttributes, source lines 112–113. This supplies the actual Mob
/// attribute template before adding registered MAX_HEALTH with base value 6.
pub fn create_attributes<H: BatAttributeTemplateHost>(
    host: &mut H,
) -> Result<H::Builder, H::Error> {
    let builder = host.create_mob_attributes()?;
    let max_health = host.max_health_attribute()?;
    host.builder_add(builder, &max_health, 6.0)
}

/// Additional-save callbacks on the actual actor and canonical CompoundTag.
/// These do not create a save or alter shared persistence ownership. get_byte
/// must retain CompoundTag.getByte's numeric-type/narrowing/default semantics.
pub trait BatNbtHost<N>: BatHost {
    fn inherited_read_additional_save_data(&mut self, tag: &N) -> Result<(), Self::Error>;
    fn get_byte(&mut self, tag: &N, name: &str) -> Result<i8, Self::Error>;
    fn inherited_add_additional_save_data(&mut self, tag: &mut N) -> Result<(), Self::Error>;
    fn put_byte(&mut self, tag: &mut N, name: &str, value: i8) -> Result<(), Self::Error>;
}

/// Source lines 230–232. Restore the complete byte, including non-resting bits;
/// neither targetPosition nor animation state is read by this original method.
pub fn read_additional_save_data<N, H: BatNbtHost<N>>(
    tag: &N,
    host: &mut H,
) -> Result<(), H::Error> {
    host.inherited_read_additional_save_data(tag)?;
    let flags = host.get_byte(tag, "BatFlags")?;
    host.set_bat_flags(flags as u8)
}

/// Source lines 236–238. The inherited write precedes the canonical flag read.
pub fn add_additional_save_data<N, H: BatNbtHost<N>>(
    tag: &mut N,
    host: &mut H,
) -> Result<(), H::Error> {
    host.inherited_add_additional_save_data(tag)?;
    let flags = host.bat_flags()?;
    host.put_byte(tag, "BatFlags", flags as i8)
}

/// The actual host's java.time.LocalDate.now/default-zone equivalent. It must
/// use the operating-system civil date and timezone rules; world day/time and a
/// fixed calendar sample are not fallbacks. LocalDate retains the single date
/// sampled for this call, including when the civil date changes during queries.
pub trait BatLocalDateHost {
    type Error;
    type LocalDate;

    fn local_date_now(&mut self) -> Result<Self::LocalDate, Self::Error>;
    fn day_of_month(&mut self, date: &Self::LocalDate) -> Result<i32, Self::Error>;
    fn month_of_year(&mut self, date: &Self::LocalDate) -> Result<i32, Self::Error>;
}

/// The actual private Bat.isHalloween helper, source lines 261–265. DAY is read
/// before MONTH from one LocalDate.now sample; both boundary days are included.
pub fn is_halloween<H: BatLocalDateHost>(host: &mut H) -> Result<bool, H::Error> {
    let date = host.local_date_now()?;
    let day = host.day_of_month(&date)?;
    let month = host.month_of_year(&date)?;
    Ok((month == 10 && day >= 20) || (month == 11 && day <= 3))
}

/// Actual LevelAccessor light/sea level and the supplied spawn RandomSource.
/// The RNG handle is the very argument passed to this static spawn predicate;
/// it is not an independently seeded entity, Level, loot or test stream.
pub trait BatSpawnRulesHost<Q>: BatLocalDateHost {
    fn sea_level(&mut self) -> Result<i32, Self::Error>;
    fn max_local_raw_brightness(&mut self, at: BlockPosition) -> Result<i32, Self::Error>;
    fn spawn_random_next_bool(&mut self, random: &Q) -> Result<bool, Self::Error>;
    fn spawn_random_next_bounded_i32(&mut self, random: &Q, bound: i32)
        -> Result<i32, Self::Error>;
}

/// Original Bat.checkBatSpawnRules, source lines 241–257, followed by the actual
/// base Mob predicate implemented in spawning.rs. This does not replace the
/// caller's spawn-placement, obstruction, caps, construction or world-add phase.
pub fn check_bat_spawn_rules<T, R, Q, H>(
    entity_type: &T,
    reason: &R,
    position: BlockPosition,
    random: &Q,
    host: &mut H,
) -> Result<bool, <H as BatLocalDateHost>::Error>
where
    H: BatSpawnRulesHost<Q>
        + super::spawning::MobSpawnRulesHost<T, R, Error = <H as BatLocalDateHost>::Error>,
{
    if position.1 >= host.sea_level()? {
        return Ok(false);
    }
    let brightness = host.max_local_raw_brightness(position)?;
    let mut bound = 4;
    if is_halloween(host)? {
        bound = 7;
    } else if host.spawn_random_next_bool(random)? {
        return Ok(false);
    }
    if brightness > host.spawn_random_next_bounded_i32(random, bound)? {
        return Ok(false);
    }
    super::spawning::check_mob_spawn_rules(entity_type, reason, position, random, host)
}

#[derive(Clone, Debug)]
pub struct BatState {
    target_position: Option<BlockPosition>,
    fly_animation_state: BatAnimationState,
    rest_animation_state: BatAnimationState,
}
impl BatState {
    /// Source constructor. The mandatory inherited callback creates the actual
    /// actor's shared owners first; this returned value must be attached to that
    /// same actor, never used to imply an independently created live entity.
    pub fn construct<H: BatHost>(host: &mut H) -> Result<Self, H::Error> {
        host.inherited_construct()?;
        let state = Self {
            target_position: None,
            fly_animation_state: BatAnimationState::new(),
            rest_animation_state: BatAnimationState::new(),
        };
        if !host.is_client_side()? {
            set_resting(host, true)?;
        }
        Ok(state)
    }
    pub fn target_position(&self) -> Option<BlockPosition> {
        self.target_position
    }
    // Callback-free fragments used by the retained driver. No actor/state
    // borrow obtained here may survive into a host operation.
    pub(super) fn set_target_position(&mut self, target: Option<BlockPosition>) {
        self.target_position = target;
    }
    pub(super) fn stop_fly_animation(&mut self) {
        self.fly_animation_state.stop();
    }
    pub(super) fn stop_rest_animation(&mut self) {
        self.rest_animation_state.stop();
    }
    pub(super) fn start_fly_animation_if_stopped(&mut self, tick: i32) {
        self.fly_animation_state.start_if_stopped(tick);
    }
    pub(super) fn start_rest_animation_if_stopped(&mut self, tick: i32) {
        self.rest_animation_state.start_if_stopped(tick);
    }
    pub fn fly_animation_state(&self) -> &BatAnimationState {
        &self.fly_animation_state
    }
    pub fn rest_animation_state(&self) -> &BatAnimationState {
        &self.rest_animation_state
    }

    /// Full inherited tick first, then Bat's hanging-position/vertical damping
    /// tail, then its own actual animation states. Travel is not repeated here.
    /// This borrowed component API requires a non-reentrant host. The normal
    /// inherited LivingEntity -> virtual Bat AI callgraph must instead use
    /// retained_bat::tick_retained, which releases state before callbacks.
    pub fn tick<H: BatHost>(&mut self, host: &mut H) -> Result<(), H::Error> {
        host.inherited_tick()?;
        if is_resting(host)? {
            host.set_delta_movement([0.0; 3])?;
            let x = host.x()?;
            let y = f64::from(java_floor(host.y()?)) + 1.0 - f64::from(host.bounding_box_height()?);
            let z = host.z()?;
            host.set_position([x, y, z])?;
        } else {
            let velocity = host.delta_movement()?;
            host.set_delta_movement([velocity[0] * 1.0, velocity[1] * 0.6, velocity[2] * 1.0])?;
        }
        if is_resting(host)? {
            self.fly_animation_state.stop();
            let tick = host.actor_tick_count()?;
            self.rest_animation_state.start_if_stopped(tick);
        } else {
            self.rest_animation_state.stop();
            let tick = host.actor_tick_count()?;
            self.fly_animation_state.start_if_stopped(tick);
        }
        Ok(())
    }

    /// Original Bat.customServerAiStep. RNG calls are conditional and use only
    /// this actor's actual RandomSource. The target is the original Bat field;
    /// health, body, resting metadata, selectors and navigation are not copied.
    /// Use retained_bat::custom_server_ai_step_retained for callbacks that can
    /// reenter this same actor; this borrowed API cannot support that callgraph.
    pub fn custom_server_ai_step<H: BatHost>(&mut self, host: &mut H) -> Result<(), H::Error> {
        host.inherited_custom_server_ai_step()?;
        let at = host.block_position()?;
        let above = (at.0, at.1.wrapping_add(1), at.2);
        if is_resting(host)? {
            let silent = host.is_silent()?;
            let ceiling = host.block_state(above)?;
            // This source branch supplies CURRENT position with ABOVE's state.
            if host.is_redstone_conductor(&ceiling, at)? {
                if host.next_entity_int(200)? == 0 {
                    let yaw = host.next_entity_int(360)? as f32;
                    host.set_head_yaw(yaw)?;
                }
                if host.has_nearest_resting_target_player()? {
                    set_resting(host, false)?;
                    if !silent {
                        host.level_event(1025, at, 0)?;
                    }
                }
            } else {
                set_resting(host, false)?;
                if !silent {
                    host.level_event(1025, at, 0)?;
                }
            }
        } else {
            if let Some(target) = self.target_position {
                if !host.is_empty_block(target)? || target.1 <= host.minimum_build_height()? {
                    self.target_position = None;
                }
            }
            let retarget = if let Some(target) = self.target_position {
                host.next_entity_int(30)? == 0
                    || closer_to_center_than(target, host.position()?, 2.0)
            } else {
                true
            };
            if retarget {
                let x = host.x()? + f64::from(host.next_entity_int(7)?)
                    - f64::from(host.next_entity_int(7)?);
                let y = host.y()? + f64::from(host.next_entity_int(6)?) - 2.0;
                let z = host.z()? + f64::from(host.next_entity_int(7)?)
                    - f64::from(host.next_entity_int(7)?);
                self.target_position = Some((java_floor(x), java_floor(y), java_floor(z)));
            }
            // The branch above always leaves a target, including invalidating
            // the old target and then encountering a later host error.
            let target = self
                .target_position
                .expect("source retarget installs target");
            let dx = f64::from(target.0) + 0.5 - host.x()?;
            let dy = f64::from(target.1) + 0.1 - host.y()?;
            let dz = f64::from(target.2) + 0.5 - host.z()?;
            let velocity = host.delta_movement()?;
            let next = steering_velocity(velocity, [dx, dy, dz]);
            host.set_delta_movement(next)?;
            let wanted_yaw = (mth_atan2(next[2], next[0]) * 57.2957763671875) as f32 - 90.0_f32;
            let turn = wrap_degrees(wanted_yaw - host.yaw()?);
            host.set_forward_input(0.5)?;
            let yaw = host.yaw()? + turn;
            host.set_yaw(yaw)?;
            if host.next_entity_int(100)? == 0 {
                let ceiling = host.block_state(above)?;
                if host.is_redstone_conductor(&ceiling, above)? {
                    set_resting(host, true)?;
                }
            }
        }
        Ok(())
    }
}

/// Bat wakes before the inherited hurt dispatch even when that later dispatch
/// rejects the hit. This does not implement or bypass the real damage owner.
pub fn hurt<H: BatHost>(host: &mut H, source: &H::Source, amount: f32) -> Result<bool, H::Error> {
    if host.is_invulnerable_to(source)? {
        return Ok(false);
    }
    if !host.is_client_side()? && is_resting(host)? {
        set_resting(host, false)?;
    }
    host.inherited_hurt(source, amount)
}

pub(crate) fn java_floor(value: f64) -> i32 {
    let narrowed = value as i32;
    if value < f64::from(narrowed) {
        narrowed.wrapping_sub(1)
    } else {
        narrowed
    }
}
pub(super) fn closer_to_center_than(at: BlockPosition, position: Vector, distance: f64) -> bool {
    let x = f64::from(at.0) + 0.5 - position[0];
    let y = f64::from(at.1) + 0.5 - position[1];
    let z = f64::from(at.2) + 0.5 - position[2];
    x * x + y * y + z * z < distance * distance
}
fn java_signum(value: f64) -> f64 {
    if value == 0.0 || value.is_nan() {
        value
    } else if value < 0.0 {
        -1.0
    } else {
        1.0
    }
}
pub(super) fn steering_velocity(velocity: Vector, offset: Vector) -> Vector {
    [
        velocity[0] + (java_signum(offset[0]) * 0.5 - velocity[0]) * 0.10000000149011612,
        velocity[1]
            + (java_signum(offset[1]) * 0.699999988079071 - velocity[1]) * 0.10000000149011612,
        velocity[2] + (java_signum(offset[2]) * 0.5 - velocity[2]) * 0.10000000149011612,
    ]
}
pub(crate) fn wrap_degrees(value: f32) -> f32 {
    let mut wrapped = value % 360.0_f32;
    if wrapped >= 180.0 {
        wrapped -= 360.0;
    }
    if wrapped < -180.0 {
        wrapped += 360.0;
    }
    wrapped
}

/// Exact source Mth.atan2/fastInvSqrt operation order (ayo, 410–473 and 491–496).
/// These are not platform atan2/sqrt approximations. The table bits below were
/// read from unchanged original ayo's 257-entry asin/cos arrays under JDK21;
/// MobMathProbe in external scratch records that source observation separately.
pub(crate) fn mth_atan2(mut y: f64, mut x: f64) -> f64 {
    let squared_length = x * x + y * y;
    if squared_length.is_nan() {
        return f64::NAN;
    }
    let negative_y = y < 0.0;
    if negative_y {
        y = -y;
    }
    let negative_x = x < 0.0;
    if negative_x {
        x = -x;
    }
    let swap = y > x;
    if swap {
        std::mem::swap(&mut x, &mut y);
    }
    let half = 0.5 * squared_length;
    let raw = squared_length.to_bits() as i64;
    let inverse_bits = 6_910_469_410_427_058_090_i64.wrapping_sub(raw >> 1);
    let mut inverse = f64::from_bits(inverse_bits as u64);
    inverse = inverse * (1.5 - half * inverse * inverse);
    x *= inverse;
    y *= inverse;
    let fraction_bias = f64::from_bits(0x42b0_0000_0000_0000);
    let biased = fraction_bias + y;
    let index = biased.to_bits() as u32 as usize;
    let asin = f64::from_bits(ASIN_BITS[index]);
    let cosine = f64::from_bits(COS_BITS[index]);
    let fraction = biased - fraction_bias;
    let deviation = y * cosine - x * fraction;
    let correction = (6.0 + deviation * deviation) * deviation * 0.16666666666666666;
    let mut angle = asin + correction;
    if swap {
        angle = 1.5707963267948966 - angle;
    }
    if negative_x {
        angle = 3.141592653589793 - angle;
    }
    if negative_y {
        angle = -angle;
    }
    angle
}

const ASIN_BITS: [u64; 257] = [
    0x0000000000000000,
    0x3f700002aaabdddf,
    0x3f80000aaabdde0c,
    0x3f8800240091cfda,
    0x3f90002aabdde94c,
    0x3f94005358ff0bd8,
    0x3f980090091d9024,
    0x3f9c00e4be5f0304,
    0x3fa000aabde0b9c8,
    0x3fa200f3229fdf2c,
    0x3fa4014d8ffaf8af,
    0x3fa601bc0922e634,
    0x3fa8024091fdb0a9,
    0x3faa02dd2f38ebf6,
    0x3fac0393e65c2c93,
    0x3fae0466bddb929c,
    0x3fb002abde953619,
    0x3fb103347666f892,
    0x3fb203ce2b380cd3,
    0x3fb3047a02794911,
    0x3fb405390240e6fd,
    0x3fb5060c31541da5,
    0x3fb606f49730ccc5,
    0x3fb707f33c173a99,
    0x3fb809092913e52e,
    0x3fb90a3768096840,
    0x3fba0b7f03ba78ac,
    0x3fbb0ce107d3f690,
    0x3fbc0e5e80f7172d,
    0x3fbd0ff87cc3a7a5,
    0x3fbe11b009e269b5,
    0x3fbf1386380f8b9a,
    0x3fc00abe0c129e1e,
    0x3fc08bc95e132e6f,
    0x3fc10ce59ba4a8c4,
    0x3fc18e134f0178af,
    0x3fc20f530308cc20,
    0x3fc290a543442d6a,
    0x3fc3120a9bed2f46,
    0x3fc3938399f32b5c,
    0x3fc41510cb011423,
    0x3fc496b2bd835ab9,
    0x3fc5186a00ade974,
    0x3fc59a37248233ea,
    0x3fc61c1ab9d55d30,
    0x3fc69e1552567517,
    0x3fc720278094cd3c,
    0x3fc7a251d80666ab,
    0x3fc82494ed0e78fc,
    0x3fc8a6f1550413c4,
    0x3fc92967a638db38,
    0x3fc9abf877ffe0e8,
    0x3fca2ea462b4998e,
    0x3fcab16bffc1f0dd,
    0x3fcb344fe9a97c4d,
    0x3fcbb750bc0acdef,
    0x3fcc3a6f13aae84b,
    0x3fccbdab8e7bd466,
    0x3fcd4106cba45b08,
    0x3fcdc4816b87e25e,
    0x3fce481c0fce7134,
    0x3fcecbd75b6cd8f0,
    0x3fcf4fb3f2ad079b,
    0x3fcfd3b27b368330,
    0x3fd02be9ce0b87cd,
    0x3fd06e0bfee5c057,
    0x3fd0b04025245ccc,
    0x3fd0f28696826d95,
    0x3fd134dfa9805147,
    0x3fd1774bb5687cf9,
    0x3fd1b9cb12545e62,
    0x3fd1fc5e19315893,
    0x3fd23f0523c5dc2b,
    0x3fd281c08cb69be8,
    0x3fd2c490af8bde81,
    0x3fd30775e8b6eeb9,
    0x3fd34a709597aab1,
    0x3fd38d8114823369,
    0x3fd3d0a7c4c4bd9c,
    0x3fd413e506ad84ee,
    0x3fd457393b90e2aa,
    0x3fd49aa4c5cf8926,
    0x3fd4de2808dce513,
    0x3fd521c36945a5f2,
    0x3fd565774cb66f02,
    0x3fd5a9441a02b1fd,
    0x3fd5ed2a392bb50f,
    0x3fd6312a1367c57c,
    0x3fd675441329986e,
    0x3fd6b978a427db95,
    0x3fd6fdc83364f719,
    0x3fd742332f3702b4,
    0x3fd786ba074fef93,
    0x3fd7cb5d2cc5e8ed,
    0x3fd8101d121bed2d,
    0x3fd854fa2b4aa1a3,
    0x3fd899f4edc962d3,
    0x3fd8df0dd0979384,
    0x3fd924454c462cc4,
    0x3fd9699bdb019139,
    0x3fd9af11f89ba61c,
    0x3fd9f4a82296347b,
    0x3fda3a5ed82d9537,
    0x3fda80369a63aaa8,
    0x3fdac62fec0b2a92,
    0x3fdb0c4b51d33b86,
    0x3fdb5289525368ab,
    0x3fdb98ea7617ef3a,
    0x3fdbdf6f47ae6904,
    0x3fdc261853b2d785,
    0x3fdc6ce628dd132c,
    0x3fdcb3d9580ea2b8,
    0x3fdcfaf27460fe9f,
    0x3fdd4232133444ad,
    0x3fdd8998cc3e6049,
    0x3fddd127399aabe7,
    0x3fde18ddf7da106b,
    0x3fde60bda613a78f,
    0x3fdea8c6e5f5e67f,
    0x3fdef0fa5bd85625,
    0x3fdf3958aecddef4,
    0x3fdf81e288b7ae20,
    0x3fdfca989658baaf,
    0x3fe009bdc3b4f877,
    0x3fe02e46075785a1,
    0x3fe052e571060fd4,
    0x3fe0779c5d4df4b8,
    0x3fe09c6b2a636bb7,
    0x3fe0c152382d7366,
    0x3fe0e651e85229ce,
    0x3fe10b6a9e43942f,
    0x3fe1309cbf4cdb24,
    0x3fe155e8b2a00052,
    0x3fe17b4ee1641318,
    0x3fe1a0cfb6c3e9eb,
    0x3fe1c66b9ffd666d,
    0x3fe1ec230c714a96,
    0x3fe211f66db3a5a1,
    0x3fe237e6379cdfc6,
    0x3fe25df2e05b6c41,
    0x3fe2841ce0862974,
    0x3fe2aa64b32f7783,
    0x3fe2d0cad5f90e20,
    0x3fe2f74fc9289adc,
    0x3fe31df40fbd31cd,
    0x3fe344b82f859adf,
    0x3fe36b9cb13786e1,
    0x3fe392a22087b7e9,
    0x3fe3b9c90c43296d,
    0x3fe3e11206694523,
    0x3fe4087da4473296,
    0x3fe4300c7e945024,
    0x3fe457bf318fe517,
    0x3fe47f965d201d78,
    0x3fe4a792a4f26152,
    0x3fe4cfb4b09d1a3e,
    0x3fe4f7fd2bc2fb34,
    0x3fe5206cc637e012,
    0x3fe5490434275b92,
    0x3fe571c42e3d0be7,
    0x3fe59aad71ced00f,
    0x3fe5c3c0c108f95c,
    0x3fe5ecfee31c96e7,
    0x3fe61668a46ffa82,
    0x3fe63ffed6d198f6,
    0x3fe669c251ad69e7,
    0x3fe693b3f244ee17,
    0x3fe6bdd49bea05ce,
    0x3fe6e825383cc40b,
    0x3fe712a6b76c6e92,
    0x3fe73d5a107bde74,
    0x3fe76840418978a7,
    0x3fe7935a501afa78,
    0x3fe7bea9496d5a54,
    0x3fe7ea2e42c9027a,
    0x3fe815ea59dab0a2,
    0x3fe841deb5114bb4,
    0x3fe86e0c84010764,
    0x3fe89a74ffcc34a4,
    0x3fe8c7196b9225de,
    0x3fe8f3fb14e496b4,
    0x3fe9211b54441083,
    0x3fe94e7b8da3cf7a,
    0x3fe97c1d30f5b7d2,
    0x3fe9aa01babef75e,
    0x3fe9d82ab4b5fdfd,
    0x3fea0699b66a8718,
    0x3fea355065f87fa4,
    0x3fea645078c6a78c,
    0x3fea939bb451e2a0,
    0x3feac333ef06451a,
    0x3feaf31b1127022e,
    0x3feb235315c680dc,
    0x3feb53de0bcffc24,
    0x3feb84be172438ef,
    0x3febb5f571cb0571,
    0x3febe7866d3b6480,
    0x3fec197373bc7bf1,
    0x3fec4bbf09e19830,
    0x3fec7e6bd023da76,
    0x3fecb17c849c7288,
    0x3fece4f404e29b3a,
    0x3fed18d55010f295,
    0x3fed4d2388f6360b,
    0x3fed81e1f875ea8d,
    0x3fedb714101e0a0e,
    0x3fedecbd6cf77786,
    0x3fee22e1da97bb17,
    0x3fee5985567b665d,
    0x3fee90ac13b18234,
    0x3feec85a7ee191da,
    0x3fef009542b712f2,
    0x3fef39614cbef7d4,
    0x3fef72c3d2c5752a,
    0x3fefacc258c4aaf9,
    0x3fefe762b77744d5,
    0x3ff0115591d29d12,
    0x3ff02f511b223c0f,
    0x3ff04da77ac5c9e5,
    0x3ff06c5c6f8ce9cc,
    0x3ff08b73f9af1058,
    0x3ff0aaf2613700b3,
    0x3ff0cadc3d4378b1,
    0x3ff0eb367c3fd618,
    0x3ff10c066d3e6932,
    0x3ff12d51caa6b58b,
    0x3ff14f1ec67484ed,
    0x3ff1717418520341,
    0x3ff194590de7e7f6,
    0x3ff1b7d59dd40ba2,
    0x3ff1dbf27dd2221a,
    0x3ff200b93cc5a540,
    0x3ff2263461820ad8,
    0x3ff24c6f8f6affb2,
    0x3ff27377b2570a1e,
    0x3ff29b5b338b7c8d,
    0x3ff2c42a3a3c7a87,
    0x3ff2edf6fac7f5b8,
    0x3ff318d619008ed9,
    0x3ff344df237486d7,
    0x3ff3722d2feb24c8,
    0x3ff3a0dfa4bb4aff,
    0x3ff3d11b3fc3b697,
    0x3ff4030b73c55372,
    0x3ff436e4418e69c0,
    0x3ff46ce4c738c4ea,
    0x3ff4a55ae332c7a5,
    0x3ff4e0a887c40a9c,
    0x3ff51f4bd13f8591,
    0x3ff561ebd9c18cd3,
    0x3ff5a96e34bc532b,
    0x3ff5f71d7ff42c8a,
    0x3ff64cf55148366e,
    0x3ff6ae4c63222736,
    0x3ff721a5d8718655,
    0x3ff7b7d33b928c5b,
    0x3ff921fb54442d18,
];

const COS_BITS: [u64; 257] = [
    0x3ff0000000000000,
    0x3fefffeffffbfffe,
    0x3fefffbfffbfff80,
    0x3fefff6ffebbfa4e,
    0x3feffefffbffdfff,
    0x3feffe6ff63b85e7,
    0x3feffdbfebbe9360,
    0x3feffcefda786870,
    0x3feffbffbff7fec0,
    0x3feffaef996bc4e8,
    0x3feff9bf63a1740b,
    0x3feff86f1b05dfb0,
    0x3feff6febba4bfea,
    0x3feff56e412875ab,
    0x3feff3bda6d9c950,
    0x3feff1ece79fa355,
    0x3fefeffbfdfebf1f,
    0x3fefedeae41957e6,
    0x3fefebb993aecf99,
    0x3fefe968061b4fc8,
    0x3fefe6f634576477,
    0x3fefe46416f790d1,
    0x3fefe1b1a62bddad,
    0x3fefdeded9bf61d3,
    0x3fefdbeba917c3f5,
    0x3fefd8d80b34b63e,
    0x3fefd5a3f6af6b74,
    0x3fefd24f61ba0590,
    0x3fefceda421efdb5,
    0x3fefcb448d40857c,
    0x3fefc78e3817e16e,
    0x3fefc3b73734bca2,
    0x3fefbfbf7ebc755f,
    0x3fefbba7026962a7,
    0x3fefb76db58a1299,
    0x3fefb3138b008181,
    0x3fefae987541497f,
    0x3fefa9fc6652caa7,
    0x3fefa53f4fcc4b79,
    0x3fefa06122d5118c,
    0x3fef9b61d0237250,
    0x3fef964147fbdbbf,
    0x3fef90ff7a2fd4d2,
    0x3fef8b9c561cf5a1,
    0x3fef8617caabd6f6,
    0x3fef8071c64ef938,
    0x3fef7aaa3701a270,
    0x3fef74c10a46b354,
    0x3fef6eb62d27730d,
    0x3fef68898c325199,
    0x3fef623b1379a09b,
    0x3fef5bcaae92424e,
    0x3fef553848924e81,
    0x3fef4e83cc0fad49,
    0x3fef47ad231ea746,
    0x3fef40b437506b2f,
    0x3fef3998f1b1886c,
    0x3fef325b3ac85e81,
    0x3fef2afafa9380f9,
    0x3fef237818880fa4,
    0x3fef1bd27b9002c4,
    0x3fef140a0a086af1,
    0x3fef0c1ea9bfa45f,
    0x3fef04103ff37d41,
    0x3feefbdeb14f4eda,
    0x3feef389e1ea090d,
    0x3feeeb11b5442ff1,
    0x3feee2760e45cb23,
    0x3feed9b6cf3c4663,
    0x3feed0d3d9d8432f,
    0x3feec7cd0f2b5adf,
    0x3feebea24fa5d0ec,
    0x3feeb5537b1434da,
    0x3feeabe0709cf371,
    0x3feea2490ebdd6b8,
    0x3fee988d33497437,
    0x3fee8eacbb648910,
    0x3fee84a78383435d,
    0x3fee7a7d6766784b,
    0x3fee702e4218c668,
    0x3fee65b9edeba38e,
    0x3fee5b20447455cb,
    0x3fee50611e88d6b5,
    0x3fee457c543ca073,
    0x3fee3a71bcdd63de,
    0x3fee2f412eefa6f9,
    0x3fee23ea802b4b1a,
    0x3fee186d8577f9ec,
    0x3fee0cca12e97895,
    0x3fee00fffbbbe023,
    0x3fedf50f124fba75,
    0x3fede8f7282602ae,
    0x3feddcb80ddc085b,
    0x3fedd05193273445,
    0x3fedc3c386d0ae09,
    0x3fedb70db6b0e156,
    0x3fedaa2fefaae1d8,
    0x3fed9d29fda7ac9c,
    0x3fed8ffbab9145d5,
    0x3fed82a4c34db1c6,
    0x3fed75250db9c792,
    0x3fed677c52a3dc9b,
    0x3fed59aa58c6471c,
    0x3fed4baee5c1b694,
    0x3fed3d89be176072,
    0x3fed2f3aa522ff95,
    0x3fed20c15d14a4e5,
    0x3fed121da6ea5769,
    0x3fed034f42698214,
    0x3fecf455ee182d6d,
    0x3fece5316736032e,
    0x3fecd5e169b519d7,
    0x3fecc665b0328622,
    0x3fecb6bdf3eeb01f,
    0x3feca6e9ecc569b9,
    0x3fec96e95125c43e,
    0x3fec86bbd609a260,
    0x3fec76612eed0424,
    0x3fec65d90dc509f4,
    0x3fec552322f6abf5,
    0x3fec443f1d4d22af,
    0x3fec332ca9effdd0,
    0x3fec21eb7458e5cc,
    0x3fec107b264904d9,
    0x3febfedb67be13b3,
    0x3febed0bdee7064d,
    0x3febdb0c30185485,
    0x3febc8dbfdbfda88,
    0x3febb67ae8584caa,
    0x3feba3e88e5c39ec,
    0x3feb91248c38986b,
    0x3feb7e2e7c3ed68e,
    0x3feb6b05f6966b9b,
    0x3feb57aa912de205,
    0x3feb441bdfab5580,
    0x3feb3059735c5e90,
    0x3feb1c62db2564ff,
    0x3feb0837a370523a,
    0x3feaf3d7561a9c44,
    0x3feadf417a62a16d,
    0x3feaca7594d44cbd,
    0x3feab5732734fa47,
    0x3feaa039b06e926d,
    0x3fea8ac8ac79d249,
    0x3fea751f9447b724,
    0x3fea5f3dddaa0225,
    0x3fea4922fb3ac8c2,
    0x3fea32ce5c4305ef,
    0x3fea1c3f6ca01f29,
    0x3fea057594a84fc8,
    0x3fe9ee70390dec3c,
    0x3fe9d72ebac16dd6,
    0x3fe9bfb076d236eb,
    0x3fe9a7f4c64dfe15,
    0x3fe98ffafe1ece30,
    0x3fe977c26ee7878a,
    0x3fe95f4a64decda8,
    0x3fe9469227a84b4c,
    0x3fe92d98fa2c355d,
    0x3fe9145e1a6cf381,
    0x3fe8fae0c15ad38a,
    0x3fe8e12022a5ab3a,
    0x3fe8c71b6c8c49b4,
    0x3fe8acd1c7a997f1,
    0x3fe8924256bf4545,
    0x3fe8776c367dda86,
    0x3fe85c4e7d4a0bb1,
    0x3fe840e83aff1d1d,
    0x3fe8253878ae2e09,
    0x3fe8093e385a3700,
    0x3fe7ecf874b086df,
    0x3fe7d06620bd8524,
    0x3fe7b386279d7bf3,
    0x3fe796576c292765,
    0x3fe778d8c89dc27c,
    0x3fe75b090e40447a,
    0x3fe73ce704fb7b23,
    0x3fe71e716af8a794,
    0x3fe6ffa6f4323c0d,
    0x3fe6e0864a0050d6,
    0x3fe6c10e0a9e5d66,
    0x3fe6a13cc8a9b946,
    0x3fe681110a985d4d,
    0x3fe660894a2751cc,
    0x3fe63fa3f3c02962,
    0x3fe61e5f65d4d978,
    0x3fe5fcb9f031317b,
    0x3fe5dab1d341202a,
    0x3fe5b8453f4ae294,
    0x3fe59572539c229b,
    0x3fe572371da8f26e,
    0x3fe54e91981b7778,
    0x3fe52a7fa9d2f8ea,
    0x3fe505ff24d0e46b,
    0x3fe4e10dc51235a7,
    0x3fe4bba92f53830a,
    0x3fe495ceefbdc28b,
    0x3fe46f7c7879a3bc,
    0x3fe448af2027201c,
    0x3fe4216420369e50,
    0x3fe3f9989320b7f7,
    0x3fe3d14972795a11,
    0x3fe3a87394da947a,
    0x3fe37f13aba2fbb6,
    0x3fe355264082fea0,
    0x3fe32aa7b2d3fd8f,
    0x3fe2ff9434b34639,
    0x3fe2d3e7c7da540a,
    0x3fe2a79e3a2cd2e6,
    0x3fe27ab321f3dcf1,
    0x3fe24d21d9bcbd2d,
    0x3fe21ee57bd01fd8,
    0x3fe1eff8dd34fde3,
    0x3fe1c0568830ae9d,
    0x3fe18ff8b63353dc,
    0x3fe15ed9491d3888,
    0x3fe12cf1c3c6a214,
    0x3fe0fa3b41afe8ac,
    0x3fe0c6ae6dbb479d,
    0x3fe0924377cc95c0,
    0x3fe05cf20924c255,
    0x3fe026b137474aca,
    0x3fdfdeeeea5d2083,
    0x3fdf6e75051127be,
    0x3fdefbdeb14f4ed9,
    0x3fde87142982b1b2,
    0x3fde0ffbbe00c34f,
    0x3fdd96799ba6f66c,
    0x3fdd1a6f89821640,
    0x3fdc9bbc9bb903fc,
    0x3fdc1a3cd9862dfc,
    0x3fdb95c8d37c8daa,
    0x3fdb0e35269b38f4,
    0x3fda8351e7be222b,
    0x3fd9f4e9f1b5859e,
    0x3fd962c20e982fea,
    0x3fd8cc97f2912db7,
    0x3fd83220fb33564e,
    0x3fd79308a1dd964b,
    0x3fd6eeee8953be07,
    0x3fd645640568c1c3,
    0x3fd595e8ede2700c,
    0x3fd4dfe7790ba697,
    0x3fd422aeba618155,
    0x3fd35d6b2ed1914a,
    0x3fd28f1c6c540fb9,
    0x3fd1b6867c415165,
    0x3fd0d21c6aeb714f,
    0x3fcfbfbf7ebc755d,
    0x3fcdba59d10bfe0a,
    0x3fcb8cc9d3952a43,
    0x3fc92ca4f010700e,
    0x3fc689f26c6b01d5,
    0x3fc389d6226c1299,
    0x3fbfeffbfdfebf21,
    0x3fb69af589b35960,
    0x3c91a62633145c07,
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    // Controlled callback recorder; not a production mob/world implementation.
    struct Host {
        flags: u8,
        client: bool,
        position: Vector,
        velocity: Vector,
        tick: i32,
        yaw: f32,
        head_yaw: f32,
        input: f32,
        silent: bool,
        conductor: bool,
        nearest: bool,
        empty: bool,
        invulnerable: bool,
        fail: Option<&'static str>,
        draws: VecDeque<(i32, i32)>,
        trace: Vec<String>,
    }
    impl Host {
        fn new() -> Self {
            Self {
                flags: 0x80,
                client: false,
                position: [0.0, 64.0, 0.0],
                velocity: [0.0; 3],
                tick: 10,
                yaw: 0.0,
                head_yaw: 0.0,
                input: 0.0,
                silent: false,
                conductor: true,
                nearest: false,
                empty: true,
                invulnerable: false,
                fail: None,
                draws: VecDeque::new(),
                trace: vec![],
            }
        }
        fn record(&mut self, event: impl Into<String>) -> Result<(), &'static str> {
            let event = event.into();
            let fail = self.fail.is_some_and(|at| at == event);
            self.trace.push(event);
            if fail {
                Err("injected callback failure")
            } else {
                Ok(())
            }
        }
        fn draws(&mut self, draws: &[(i32, i32)]) {
            self.draws = draws.iter().copied().collect();
        }
    }
    impl BatHost for Host {
        type Error = &'static str;
        type Source = ();
        type BlockState = BlockPosition;
        fn inherited_construct(&mut self) -> Result<(), Self::Error> {
            self.record("inherited_construct")
        }
        fn inherited_define_synched_data(&mut self) -> Result<(), Self::Error> {
            self.record("inherited_define")
        }
        fn define_bat_flags(&mut self, flags: u8) -> Result<(), Self::Error> {
            self.record("define_flags")?;
            self.flags = flags;
            Ok(())
        }
        fn is_client_side(&mut self) -> Result<bool, Self::Error> {
            self.record("client")?;
            Ok(self.client)
        }
        fn bat_flags(&mut self) -> Result<u8, Self::Error> {
            self.record("flags")?;
            Ok(self.flags)
        }
        fn set_bat_flags(&mut self, flags: u8) -> Result<(), Self::Error> {
            self.record(format!("set_flags:{flags}"))?;
            self.flags = flags;
            Ok(())
        }
        fn inherited_tick(&mut self) -> Result<(), Self::Error> {
            self.record("inherited_tick")
        }
        fn inherited_custom_server_ai_step(&mut self) -> Result<(), Self::Error> {
            self.record("inherited_ai")
        }
        fn actor_tick_count(&mut self) -> Result<i32, Self::Error> {
            self.record("tick")?;
            Ok(self.tick)
        }
        fn block_position(&mut self) -> Result<BlockPosition, Self::Error> {
            self.record("block_position")?;
            Ok((
                java_floor(self.position[0]),
                java_floor(self.position[1]),
                java_floor(self.position[2]),
            ))
        }
        fn position(&mut self) -> Result<Vector, Self::Error> {
            self.record("position")?;
            Ok(self.position)
        }
        fn x(&mut self) -> Result<f64, Self::Error> {
            self.record("x")?;
            Ok(self.position[0])
        }
        fn y(&mut self) -> Result<f64, Self::Error> {
            self.record("y")?;
            Ok(self.position[1])
        }
        fn z(&mut self) -> Result<f64, Self::Error> {
            self.record("z")?;
            Ok(self.position[2])
        }
        fn bounding_box_height(&mut self) -> Result<f32, Self::Error> {
            self.record("height")?;
            Ok(0.9)
        }
        fn delta_movement(&mut self) -> Result<Vector, Self::Error> {
            self.record("velocity")?;
            Ok(self.velocity)
        }
        fn set_delta_movement(&mut self, v: Vector) -> Result<(), Self::Error> {
            self.record("set_velocity")?;
            self.velocity = v;
            Ok(())
        }
        fn set_position(&mut self, v: Vector) -> Result<(), Self::Error> {
            self.record("set_position")?;
            self.position = v;
            Ok(())
        }
        fn is_silent(&mut self) -> Result<bool, Self::Error> {
            self.record("silent")?;
            Ok(self.silent)
        }
        fn block_state(&mut self, p: BlockPosition) -> Result<Self::BlockState, Self::Error> {
            self.record(format!("block:{p:?}"))?;
            Ok(p)
        }
        fn is_redstone_conductor(
            &mut self,
            s: &BlockPosition,
            p: BlockPosition,
        ) -> Result<bool, Self::Error> {
            self.record(format!("conductor:{s:?}:{p:?}"))?;
            Ok(self.conductor)
        }
        fn has_nearest_resting_target_player(&mut self) -> Result<bool, Self::Error> {
            self.record("nearest_target_player")?;
            Ok(self.nearest)
        }
        fn is_empty_block(&mut self, p: BlockPosition) -> Result<bool, Self::Error> {
            self.record(format!("empty:{p:?}"))?;
            Ok(self.empty)
        }
        fn minimum_build_height(&mut self) -> Result<i32, Self::Error> {
            self.record("min_y")?;
            Ok(-64)
        }
        fn next_entity_int(&mut self, bound: i32) -> Result<i32, Self::Error> {
            self.record(format!("random:{bound}"))?;
            let (expected, n) = self.draws.pop_front().expect("unexpected RNG draw");
            assert_eq!(bound, expected);
            assert!((0..bound).contains(&n));
            Ok(n)
        }
        fn set_head_yaw(&mut self, y: f32) -> Result<(), Self::Error> {
            self.record(format!("head:{y}"))?;
            self.head_yaw = y;
            Ok(())
        }
        fn yaw(&mut self) -> Result<f32, Self::Error> {
            self.record("yaw")?;
            Ok(self.yaw)
        }
        fn set_yaw(&mut self, y: f32) -> Result<(), Self::Error> {
            self.record("set_yaw")?;
            self.yaw = y;
            Ok(())
        }
        fn set_forward_input(&mut self, v: f32) -> Result<(), Self::Error> {
            self.record("forward")?;
            self.input = v;
            Ok(())
        }
        fn level_event(
            &mut self,
            event: i32,
            p: BlockPosition,
            data: i32,
        ) -> Result<(), Self::Error> {
            self.record(format!("event:{event}:{p:?}:{data}"))
        }
        fn is_invulnerable_to(&mut self, _: &()) -> Result<bool, Self::Error> {
            self.record("invulnerable")?;
            Ok(self.invulnerable)
        }
        fn inherited_hurt(&mut self, _: &(), amount: f32) -> Result<bool, Self::Error> {
            self.record(format!("inherited_hurt:{amount}"))?;
            Ok(false)
        }
    }
    impl BatSoundHost for Host {
        type Sound = &'static str;

        fn inherited_voice_pitch(&mut self) -> Result<f32, Self::Error> {
            self.record("inherited_pitch")?;
            Ok(1.25)
        }
        fn bat_ambient_sound(&mut self) -> Result<Self::Sound, Self::Error> {
            self.record("ambient_handle")?;
            Ok("canonical ambient handle")
        }
        fn bat_hurt_sound(&mut self) -> Result<Self::Sound, Self::Error> {
            self.record("hurt_handle")?;
            Ok("canonical hurt handle")
        }
        fn bat_death_sound(&mut self) -> Result<Self::Sound, Self::Error> {
            self.record("death_handle")?;
            Ok("canonical death handle")
        }
    }

    #[derive(Default)]
    struct SavedTag {
        flags: i8,
        inherited_writes: i32,
    }
    impl BatNbtHost<SavedTag> for Host {
        fn inherited_read_additional_save_data(
            &mut self,
            _tag: &SavedTag,
        ) -> Result<(), Self::Error> {
            self.record("inherited_read")
        }
        fn get_byte(&mut self, tag: &SavedTag, name: &str) -> Result<i8, Self::Error> {
            assert_eq!(name, "BatFlags");
            self.record("get_byte")?;
            Ok(tag.flags)
        }
        fn inherited_add_additional_save_data(
            &mut self,
            tag: &mut SavedTag,
        ) -> Result<(), Self::Error> {
            self.record("inherited_write")?;
            tag.inherited_writes += 1;
            Ok(())
        }
        fn put_byte(
            &mut self,
            tag: &mut SavedTag,
            name: &str,
            value: i8,
        ) -> Result<(), Self::Error> {
            assert_eq!(name, "BatFlags");
            self.record("put_byte")?;
            tag.flags = value;
            Ok(())
        }
    }

    impl BatAttributeTemplateHost for Host {
        type Error = &'static str;
        type Builder = std::rc::Rc<std::cell::RefCell<Vec<(&'static str, f64)>>>;
        type Attribute = &'static str;

        fn create_mob_attributes(&mut self) -> Result<Self::Builder, Self::Error> {
            self.record("mob_attribute_builder")?;
            Ok(std::rc::Rc::new(std::cell::RefCell::new(vec![(
                "inherited Mob template",
                16.0,
            )])))
        }
        fn max_health_attribute(&mut self) -> Result<Self::Attribute, Self::Error> {
            self.record("max_health_holder")?;
            Ok("registered MAX_HEALTH holder")
        }
        fn builder_add(
            &mut self,
            builder: Self::Builder,
            attribute: &Self::Attribute,
            value: f64,
        ) -> Result<Self::Builder, Self::Error> {
            self.record("builder_add")?;
            builder.borrow_mut().push((*attribute, value));
            Ok(builder)
        }
    }

    #[test]
    fn source_sound_overrides_and_literal_push_fall_movement_bodies() {
        let mut h = Host::new();
        h.flags = 0x81;
        h.draws(&[(4, 3)]);
        assert_eq!(get_ambient_sound(&mut h), Ok(None));
        assert_eq!(h.trace, ["flags", "random:4"]);
        h.trace.clear();
        h.draws(&[(4, 0)]);
        assert_eq!(
            get_ambient_sound(&mut h),
            Ok(Some("canonical ambient handle"))
        );
        assert_eq!(h.trace, ["flags", "random:4", "ambient_handle"]);
        h.trace.clear();
        h.flags = 0x80;
        h.draws(&[]);
        assert_eq!(
            get_ambient_sound(&mut h),
            Ok(Some("canonical ambient handle"))
        );
        assert_eq!(h.trace, ["flags", "ambient_handle"]);
        h.trace.clear();
        assert_eq!(
            get_voice_pitch(&mut h).unwrap().to_bits(),
            (1.25_f32 * 0.95_f32).to_bits()
        );
        assert_eq!(get_sound_volume().to_bits(), 0.1_f32.to_bits());
        assert_eq!(get_hurt_sound(&(), &mut h), Ok("canonical hurt handle"));
        assert_eq!(get_death_sound(&mut h), Ok("canonical death handle"));
        assert_eq!(h.trace, ["inherited_pitch", "hurt_handle", "death_handle"]);
        h.trace.clear();
        h.fail = Some("inherited_pitch");
        assert_eq!(get_voice_pitch(&mut h), Err("injected callback failure"));
        assert_eq!(h.trace, ["inherited_pitch"]);

        assert!(!is_pushable());
        do_push(&());
        push_entities();
        check_fall_damage(-20.0, true, &(), (0, 0, 0));
        assert!(is_ignoring_block_triggers());
        assert_eq!(
            get_movement_emission(),
            crate::entity_move::MovementEmission::Events
        );
        h.fail = None;
        h.tick = 16_777_219; // float narrowing becomes 16_777_220, divisible by ten.
        assert!(is_flapping(&mut h).unwrap());
        assert_ne!(h.tick % 10, 0);
        h.flags |= 1;
        h.trace.clear();
        assert!(!is_flapping(&mut h).unwrap());
        assert_eq!(h.trace, ["flags"]);
    }

    #[test]
    fn attribute_template_and_flag_nbt_preserve_inherited_prefix_and_all_bits() {
        let mut h = Host::new();
        let builder = create_attributes(&mut h).unwrap();
        assert_eq!(
            h.trace,
            ["mob_attribute_builder", "max_health_holder", "builder_add"]
        );
        assert_eq!(
            &*builder.borrow(),
            &[
                ("inherited Mob template", 16.0),
                ("registered MAX_HEALTH holder", 6.0)
            ]
        );
        h.trace.clear();
        let mut tag = SavedTag {
            flags: -127,
            inherited_writes: 0,
        };
        read_additional_save_data(&tag, &mut h).unwrap();
        assert_eq!(h.flags, 0x81);
        assert_eq!(h.trace, ["inherited_read", "get_byte", "set_flags:129"]);
        h.trace.clear();
        h.flags = 0xfe;
        add_additional_save_data(&mut tag, &mut h).unwrap();
        assert_eq!(tag.flags, -2);
        assert_eq!(tag.inherited_writes, 1);
        assert_eq!(h.trace, ["inherited_write", "flags", "put_byte"]);
        h.trace.clear();
        h.fail = Some("put_byte");
        h.flags = 0x80;
        assert_eq!(
            add_additional_save_data(&mut tag, &mut h),
            Err("injected callback failure")
        );
        assert_eq!(tag.inherited_writes, 2);
        assert_eq!(tag.flags, -2);
        assert_eq!(h.trace, ["inherited_write", "flags", "put_byte"]);
    }

    fn state(host: &mut Host) -> BatState {
        let s = BatState::construct(host).unwrap();
        host.trace.clear();
        s
    }

    #[test]
    fn constructor_metadata_hurt_and_tick_keep_real_owner_order() {
        let mut h = Host::new();
        let mut bat = BatState::construct(&mut h).unwrap();
        assert_eq!(
            h.trace,
            ["inherited_construct", "client", "flags", "set_flags:129"]
        );
        assert!(bat.target_position().is_none());
        assert!(!bat.fly_animation_state().is_started());
        h.trace.clear();
        h.position[1] = 64.75;
        h.velocity = [1.0, 2.0, 3.0];
        bat.tick(&mut h).unwrap();
        assert_eq!(
            h.trace,
            [
                "inherited_tick",
                "flags",
                "set_velocity",
                "x",
                "y",
                "height",
                "z",
                "set_position",
                "flags",
                "tick"
            ]
        );
        assert_eq!(h.position, [0.0, 65.0 - f64::from(0.9_f32), 0.0]);
        assert_eq!(h.velocity, [0.0; 3]);
        assert_eq!(bat.rest_animation_state().last_time(), 500);
        assert!(!bat.fly_animation_state().is_started());
        h.tick = 11;
        bat.tick(&mut h).unwrap();
        assert_eq!(bat.rest_animation_state().last_time(), 500);
        h.trace.clear();
        assert_eq!(hurt(&mut h, &(), 3.0), Ok(false));
        assert_eq!(
            h.trace,
            [
                "invulnerable",
                "client",
                "flags",
                "flags",
                "set_flags:128",
                "inherited_hurt:3"
            ]
        );
        assert_eq!(h.flags, 0x80); // inherited rejection does not undo wake
        h.velocity = [1.0, 2.0, 3.0];
        h.tick = 12;
        bat.tick(&mut h).unwrap();
        assert_eq!(h.velocity, [1.0, 1.2, 3.0]);
        assert_eq!(bat.fly_animation_state().last_time(), 600);
        assert!(!bat.rest_animation_state().is_started());
        h.trace.clear();
        h.invulnerable = true;
        h.flags = 0x81;
        assert_eq!(hurt(&mut h, &(), 3.0), Ok(false));
        assert_eq!(h.trace, ["invulnerable"]);
        assert_eq!(h.flags, 0x81);
        h.trace.clear();
        define_synched_data(&mut h).unwrap();
        assert_eq!(h.trace, ["inherited_define", "define_flags"]);
        assert_eq!(h.flags, 0);
    }
    #[test]
    fn resting_head_rng_then_real_player_query_and_wake_event() {
        let mut h = Host::new();
        let mut bat = state(&mut h);
        h.nearest = true;
        h.draws(&[(200, 0), (360, 271)]);
        bat.custom_server_ai_step(&mut h).unwrap();
        assert_eq!(
            h.trace,
            [
                "inherited_ai",
                "block_position",
                "flags",
                "silent",
                "block:(0, 65, 0)",
                "conductor:(0, 65, 0):(0, 64, 0)",
                "random:200",
                "random:360",
                "head:271",
                "nearest_target_player",
                "flags",
                "set_flags:128",
                "event:1025:(0, 64, 0):0"
            ]
        );
        assert_eq!(h.head_yaw, 271.0);
        assert_eq!(h.flags, 0x80);
        assert!(h.draws.is_empty());
        h.flags = 0x81;
        h.trace.clear();
        h.silent = true;
        h.conductor = false;
        bat.custom_server_ai_step(&mut h).unwrap();
        assert!(!h.trace.iter().any(|e| e.starts_with("random:")
            || e.starts_with("event:")
            || e == "nearest_target_player"));
        assert_eq!(h.flags, 0x80);
    }
    #[test]
    fn flight_target_draws_steering_and_later_rest_query_are_source_ordered() {
        let mut h = Host::new();
        let mut bat = state(&mut h);
        h.flags = 0x80;
        h.draws(&[(7, 6), (7, 2), (6, 5), (7, 1), (7, 4), (100, 1)]);
        bat.custom_server_ai_step(&mut h).unwrap();
        assert_eq!(bat.target_position(), Some((4, 67, -3)));
        assert_eq!(
            h.trace,
            [
                "inherited_ai",
                "block_position",
                "flags",
                "x",
                "random:7",
                "random:7",
                "y",
                "random:6",
                "z",
                "random:7",
                "random:7",
                "x",
                "y",
                "z",
                "velocity",
                "set_velocity",
                "yaw",
                "forward",
                "yaw",
                "set_yaw",
                "random:100"
            ]
        );
        assert_eq!(h.velocity[0], 0.5 * 0.10000000149011612);
        assert_eq!(h.velocity[1], 0.699999988079071 * 0.10000000149011612);
        assert_eq!(h.velocity[2], -0.5 * 0.10000000149011612);
        assert_eq!(h.input, 0.5);
        h.trace.clear();
        h.draws(&[(30, 1), (100, 0)]);
        bat.custom_server_ai_step(&mut h).unwrap();
        assert!(h.trace.starts_with(&[
            "inherited_ai".into(),
            "block_position".into(),
            "flags".into(),
            "empty:(4, 67, -3)".into(),
            "min_y".into(),
            "random:30".into(),
            "position".into()
        ]));
        assert!(h.trace.ends_with(&[
            "random:100".into(),
            "block:(0, 65, 0)".into(),
            "conductor:(0, 65, 0):(0, 65, 0)".into(),
            "flags".into(),
            "set_flags:129".into()
        ]));
        assert_eq!(h.flags, 0x81);
    }
    #[test]
    fn failed_actor_callback_keeps_new_target_and_consumed_random_prefix() {
        let mut h = Host::new();
        let mut bat = state(&mut h);
        h.flags = 0x80;
        h.fail = Some("velocity");
        h.draws(&[(7, 6), (7, 2), (6, 5), (7, 1), (7, 4)]);
        assert_eq!(
            bat.custom_server_ai_step(&mut h),
            Err("injected callback failure")
        );
        assert_eq!(bat.target_position(), Some((4, 67, -3)));
        assert!(h.draws.is_empty());
        assert_eq!(h.velocity, [0.0; 3]);
        assert_eq!(h.trace.last().unwrap(), "velocity");
        h.fail = None;
        h.empty = false;
        h.trace.clear();
        h.draws(&[(7, 0), (7, 0), (6, 0), (7, 0), (7, 0), (100, 1)]);
        bat.custom_server_ai_step(&mut h).unwrap();
        assert_eq!(bat.target_position(), Some((0, 62, 0)));
        assert!(!h.trace.iter().any(|e| e == "min_y" || e == "random:30"));
    }
    #[test]
    fn pinned_original_mth_math_observations_match_bits() {
        // Direct unchanged ayo.d(DD) observations from external MobMathProbe.
        for (y, x, wanted) in [
            (0x0000000000000000, 0x0000000000000000, 0x0000000000000000),
            (0x8000000000000000, 0x3ff0000000000000, 0x0000000000000000),
            (0x3ff0000000000000, 0x0000000000000000, 0x3ff921fb54442d18),
            (0xbff0000000000000, 0x0000000000000000, 0xbff921fb54442d18),
            (0x3ff0000000000000, 0x3ff0000000000000, 0x3fe921fb45e6d12e),
            (0x3ff0000000000000, 0xbff0000000000000, 0x4002d97c82ca78cc),
            (0xbff0000000000000, 0xbff0000000000000, 0xc002d97c82ca78cc),
            (0x3f689374bc6a7efa, 0x3fe6666666666666, 0x3f718db3b810e173),
            (0x3fb999999999999a, 0x3fc999999999999a, 0x3fddac64ca13863b),
            (0x7ff0000000000000, 0x3ff0000000000000, 0x7ff0000000000000),
            (0x3ff0000000000000, 0x7ff0000000000000, 0xfff0000000000000),
        ] {
            assert_eq!(
                mth_atan2(f64::from_bits(y), f64::from_bits(x)).to_bits(),
                wanted
            );
        }
        assert!(mth_atan2(f64::NAN, 1.0).is_nan());
        assert_eq!(java_signum(-0.0).to_bits(), (-0.0_f64).to_bits());
        assert_eq!(java_signum(0.0).to_bits(), 0.0_f64.to_bits());
        assert_eq!(wrap_degrees(-0.0).to_bits(), (-0.0_f32).to_bits());
        for (v, wanted) in [
            (180.0, -180.0),
            (-180.0, -180.0),
            (540.0, -180.0),
            (-540.0, -180.0),
        ] {
            assert_eq!(wrap_degrees(v), wanted);
        }
        assert!(wrap_degrees(f32::NAN).is_nan());
    }
}

#[cfg(test)]
mod spawn_rule_tests {
    use super::*;
    use crate::random::LegacyRandomSource;
    use std::rc::Rc;

    // Explicit controlled clock/world callbacks, not a production date owner.
    struct SpawnHost {
        date: Rc<(i32, i32)>,
        sampled_date: Option<Rc<(i32, i32)>>,
        change_external_date_after_day: bool,
        random: LegacyRandomSource,
        sea_level: i32,
        brightness: i32,
        valid_spawn: bool,
        fail: Option<&'static str>,
        trace: Vec<String>,
    }
    impl SpawnHost {
        fn new(month: i32, day: i32, seed: i64) -> Self {
            Self {
                date: Rc::new((month, day)),
                sampled_date: None,
                change_external_date_after_day: false,
                random: LegacyRandomSource::new(seed),
                sea_level: 64,
                brightness: 0,
                valid_spawn: true,
                fail: None,
                trace: Vec::new(),
            }
        }
        fn record(&mut self, value: impl Into<String>) -> Result<(), &'static str> {
            let value = value.into();
            let fail = self.fail == Some(value.as_str());
            self.trace.push(value);
            if fail {
                Err("injected spawn owner failure")
            } else {
                Ok(())
            }
        }
    }

    impl BatLocalDateHost for SpawnHost {
        type Error = &'static str;
        type LocalDate = Rc<(i32, i32)>;
        fn local_date_now(&mut self) -> Result<Self::LocalDate, Self::Error> {
            self.record("local_date_now")?;
            self.sampled_date = Some(self.date.clone());
            Ok(self.date.clone())
        }
        fn day_of_month(&mut self, date: &Self::LocalDate) -> Result<i32, Self::Error> {
            assert!(Rc::ptr_eq(date, self.sampled_date.as_ref().unwrap()));
            self.record("day")?;
            if self.change_external_date_after_day {
                self.date = Rc::new((12, 1));
            }
            Ok(date.1)
        }
        fn month_of_year(&mut self, date: &Self::LocalDate) -> Result<i32, Self::Error> {
            assert!(Rc::ptr_eq(date, self.sampled_date.as_ref().unwrap()));
            self.record("month")?;
            Ok(date.0)
        }
    }
    impl BatSpawnRulesHost<u32> for SpawnHost {
        fn sea_level(&mut self) -> Result<i32, Self::Error> {
            self.record("sea_level")?;
            Ok(self.sea_level)
        }
        fn max_local_raw_brightness(&mut self, at: BlockPosition) -> Result<i32, Self::Error> {
            self.record(format!("brightness:{at:?}"))?;
            Ok(self.brightness)
        }
        fn spawn_random_next_bool(&mut self, random: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*random, 42);
            self.record("spawn_boolean")?;
            Ok(self.random.next_bool())
        }
        fn spawn_random_next_bounded_i32(
            &mut self,
            random: &u32,
            bound: i32,
        ) -> Result<i32, Self::Error> {
            assert_eq!(*random, 42);
            self.record(format!("spawn_random:{bound}"))?;
            self.random
                .next_bounded_i32(bound)
                .map_err(|_| "invalid source bound")
        }
    }
    impl super::super::spawning::MobSpawnRulesHost<u32, bool> for SpawnHost {
        type Error = &'static str;
        type BlockState = BlockPosition;
        fn reason_is_spawner(&mut self, reason: &bool) -> Result<bool, Self::Error> {
            self.record("exact_spawner")?;
            Ok(*reason)
        }
        fn block_state(&mut self, at: BlockPosition) -> Result<Self::BlockState, Self::Error> {
            self.record(format!("block:{at:?}"))?;
            Ok(at)
        }
        fn block_is_valid_spawn(
            &mut self,
            state: &Self::BlockState,
            at: BlockPosition,
            entity_type: &u32,
        ) -> Result<bool, Self::Error> {
            assert_eq!(*state, at);
            assert_eq!(*entity_type, 37);
            self.record(format!("valid_spawn:{at:?}"))?;
            Ok(self.valid_spawn)
        }
    }

    #[test]
    fn halloween_uses_one_retained_local_date_day_then_month_and_exact_boundaries() {
        for (month, day, expected) in [
            (10, 19, false),
            (10, 20, true),
            (10, 31, true),
            (11, 1, true),
            (11, 3, true),
            (11, 4, false),
        ] {
            let mut host = SpawnHost::new(month, day, 38);
            let before = host.random.state48();
            assert_eq!(is_halloween(&mut host), Ok(expected));
            assert_eq!(host.trace, ["local_date_now", "day", "month"]);
            assert_eq!(host.random.state48(), before);
        }
        let mut rollover = SpawnHost::new(10, 20, 38);
        rollover.change_external_date_after_day = true;
        assert_eq!(is_halloween(&mut rollover), Ok(true));
        assert_eq!(*rollover.date, (12, 1));
        assert_eq!(*rollover.sampled_date.unwrap(), (10, 20));
    }

    #[test]
    fn static_spawn_predicate_preserves_world_date_rng_order_and_short_circuits() {
        let position = (4, 63, 7);
        let mut halloween = SpawnHost::new(10, 20, 38);
        let mut expected_random = halloween.random.clone();
        halloween.brightness = expected_random.next_bounded_i32(7).unwrap();
        assert_eq!(
            check_bat_spawn_rules(&37, &false, position, &42, &mut halloween),
            Ok(true)
        );
        assert_eq!(halloween.random.state48(), expected_random.state48());
        assert_eq!(
            halloween.trace,
            [
                "sea_level",
                "brightness:(4, 63, 7)",
                "local_date_now",
                "day",
                "month",
                "spawn_random:7",
                "exact_spawner",
                "block:(4, 62, 7)",
                "valid_spawn:(4, 62, 7)",
            ]
        );

        let mut at_sea = SpawnHost::new(10, 20, 38);
        let before = at_sea.random.state48();
        assert_eq!(
            check_bat_spawn_rules(&37, &false, (4, 64, 7), &42, &mut at_sea),
            Ok(false)
        );
        assert_eq!(at_sea.trace, ["sea_level"]);
        assert_eq!(at_sea.random.state48(), before);

        let mut ordinary_reject = SpawnHost::new(1, 1, 0);
        let mut expected_random = ordinary_reject.random.clone();
        assert!(expected_random.next_bool());
        assert_eq!(
            check_bat_spawn_rules(&37, &false, position, &42, &mut ordinary_reject),
            Ok(false)
        );
        assert_eq!(ordinary_reject.random.state48(), expected_random.state48());
        assert_eq!(ordinary_reject.trace.last().unwrap(), "spawn_boolean");

        let mut ordinary_pass = SpawnHost::new(1, 1, 4096);
        let mut expected_random = ordinary_pass.random.clone();
        assert!(!expected_random.next_bool());
        ordinary_pass.brightness = expected_random.next_bounded_i32(4).unwrap();
        assert_eq!(
            check_bat_spawn_rules(&37, &true, position, &42, &mut ordinary_pass),
            Ok(true)
        );
        assert_eq!(ordinary_pass.random.state48(), expected_random.state48());
        assert_eq!(
            &ordinary_pass.trace[5..],
            ["spawn_boolean", "spawn_random:4", "exact_spawner"]
        );

        let mut light_reject = SpawnHost::new(10, 20, 38);
        light_reject.brightness = 7;
        assert_eq!(
            check_bat_spawn_rules(&37, &false, position, &42, &mut light_reject),
            Ok(false)
        );
        assert_eq!(light_reject.trace.last().unwrap(), "spawn_random:7");
        let mut failing = SpawnHost::new(10, 20, 38);
        failing.fail = Some("month");
        let before = failing.random.state48();
        assert_eq!(
            check_bat_spawn_rules(&37, &false, position, &42, &mut failing),
            Err("injected spawn owner failure")
        );
        assert_eq!(failing.random.state48(), before);
        assert_eq!(
            failing.trace,
            [
                "sea_level",
                "brightness:(4, 63, 7)",
                "local_date_now",
                "day",
                "month"
            ]
        );
    }
}
