//! Complementary original LivingEntity fields for one actual Bat lifetime.
//!
//! This owns the mapped Living fields which a Bat's EntityPhysicalState does
//! not own. It has no physical state, health9, ID, RNG, clock or alive flag.
//! Existing PlayerBodyState/PlayerLifecycle remain Player owners: do not attach
//! this whole state beside them. The host supplies actual canonical reference
//! objects, and constructor fragments stop when an owner is unavailable.

use super::{
    ai_step::{AiDoubleField, AiFloatField, AiIntField},
    base_tick::{LivingFloatField, LivingIntField},
    tick::{LivingTickFields, TickFloatField, TickIntField},
};
use crate::{
    combat::tracker::CombatTracker,
    effects::attribute_map::{AttributeMapState, JavaObjectKey},
    entity_state::EntityPhysicalState,
};

/// Actual source object/holder types. Clone of an Actor or ItemStack handle is
/// a callback-free alias of the SAME object, never a new actor or stack copy.
/// Attribute keys retain source equality/hash and supplier identity. Active
/// effects, Brain and location-enchantment types must be real state owners;
/// booleans, snapshots, or successful dummy objects do not satisfy the contract.
pub trait LivingObjects {
    type Actor: Clone;
    type ItemStack: Clone;
    type BlockPos;
    type DamageSource;
    type FallLocation;
    type AttributeHolder: JavaObjectKey;
    type AttributeInstance: JavaObjectKey;
    type AttributeSupplier;
    type ActiveEffectsMap;
    type Brain;
    type LocationEnchantmentsMap;
}

/// The actual CombatTracker actor reference plus its already ported native
/// field owner. This adds no health/alive/tick state. The runtime retains this
/// source object graph and must bind every tracker callback to `actor`.
pub struct LivingCombatTracker<A, S, F> {
    pub actor: A,
    pub state: CombatTracker<S, F>,
}

/// Actual fixed NonNullList equipment-history state, not a detached inventory.
/// Both the default and all initial entries alias canonical ItemStack.EMPTY.
/// These are Living's LAST stacks, distinct from Mob's current equipment lists.
pub struct EquipmentHistory<S, const N: usize> {
    default_value: S,
    items: [S; N],
}
impl<S: Clone, const N: usize> EquipmentHistory<S, N> {
    pub fn new(canonical_empty: S) -> Self {
        let items = std::array::from_fn(|_| canonical_empty.clone());
        Self {
            default_value: canonical_empty,
            items,
        }
    }
    pub fn items(&self) -> &[S; N] {
        &self.items
    }
    pub fn get(&self, index: usize) -> &S {
        &self.items[index]
    }
    pub fn set(&mut self, index: usize, item: S) -> S {
        std::mem::replace(&mut self.items[index], item)
    }
    /// NonNullList.clear resets fixed entries to the same nonnull default.
    pub fn clear(&mut self) {
        for item in &mut self.items {
            *item = self.default_value.clone();
        }
    }
}

/// Original WalkAnimationState (`bup`, 5–37), distinct from Bat AnimationState.
/// All three fields start at Java +0. Operations preserve source f32 order.
pub struct WalkAnimationState {
    speed_old: f32,
    speed: f32,
    position: f32,
}
impl WalkAnimationState {
    pub fn new() -> Self {
        Self {
            speed_old: 0.0,
            speed: 0.0,
            position: 0.0,
        }
    }
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }
    pub fn update(&mut self, target: f32, scale: f32) {
        self.speed_old = self.speed;
        self.speed += (target - self.speed) * scale;
        self.position += self.speed;
    }
    pub fn speed(&self) -> f32 {
        self.speed
    }
    pub fn speed_at(&self, partial: f32) -> f32 {
        self.speed_old + partial * (self.speed - self.speed_old)
    }
    pub fn position(&self) -> f32 {
        self.position
    }
    pub fn position_at(&self, partial: f32) -> f32 {
        self.position - self.speed * (1.0_f32 - partial)
    }
    pub fn is_moving(&self) -> bool {
        self.speed > 1.0e-5_f32
    }
}
impl Default for WalkAnimationState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LivingHand {
    Main,
    Off,
}

/// Default tracker reference encoding. Keeping this a nominal tag avoids
/// recursively normalizing Objects::Actor in a public default type parameter.
pub struct RetainedLivingTracker;
/// Aggregate-private tracker self-reference encoding. This never changes the
/// genuine retained LivingObjects::Actor fields used for other actors.
pub struct InternalLivingTracker<A>(std::marker::PhantomData<A>);
pub trait LivingTrackerStorage<O: LivingObjects> {
    type Actor;
}
impl<O: LivingObjects> LivingTrackerStorage<O> for RetainedLivingTracker {
    type Actor = O::Actor;
}
impl<O: LivingObjects, A> LivingTrackerStorage<O> for InternalLivingTracker<A> {
    type Actor = A;
}

/// Canonical complementary Living fields. `jvm_zeroed` represents allocation
/// BEFORE Entity's constructor and virtual metadata definition. Reference fields
/// are genuinely null then; it does not allocate a map/Brain/EMPTY substitute.
/// Source field initializers and constructor callbacks are separate below.
///
/// Health is exclusively actual metadata9. Generic absorptionAmount(ci) is this
/// scalar; Player's override uses metadata15 instead. Mob metadata15 is a BYTE.
/// `dead` is ONLY source btn.be; it is never inferred from health/removal and
/// is not used as a replacement isAlive/isDeadOrDying predicate.
pub struct LivingEntityState<
    O: LivingObjects,
    Tracker: LivingTrackerStorage<O> = RetainedLivingTracker,
> {
    pub swinging: bool,
    pub swinging_arm: Option<LivingHand>,
    pub swing_time: i32,
    pub remove_arrow_time: i32,
    pub remove_stinger_time: i32,
    pub hurt_time: i32,
    pub hurt_duration: i32,
    pub death_time: i32,
    pub old_attack_anim: f32,
    pub attack_anim: f32,
    pub attack_strength_ticker: i32,
    /// Source final aV, assigned20 at the Living initializer, NOT the Entity
    /// invulnerableTime counter (which remains in EntityPhysicalState).
    pub invulnerable_duration: i32,
    pub time_offset: f32,
    pub rot_a: f32,
    pub body_yaw: f32,
    pub old_body_yaw: f32,
    pub head_yaw: f32,
    pub old_head_yaw: f32,
    pub last_hurt_by_player: Option<O::Actor>,
    pub last_hurt_by_player_time: i32,
    pub dead: bool,
    pub no_action_time: i32,
    pub old_run: f32,
    pub run: f32,
    pub animation_step: f32,
    pub old_animation_step: f32,
    pub rotation_offset: f32,
    pub death_score: i32,
    pub last_hurt: f32,
    pub jumping: bool,
    pub xxa: f32,
    pub yya: f32,
    pub zza: f32,
    pub lerp_steps: i32,
    pub lerp_x: f64,
    pub lerp_y: f64,
    pub lerp_z: f64,
    pub lerp_yaw: f64,
    pub lerp_pitch: f64,
    pub lerp_head_yaw: f64,
    pub lerp_head_steps: i32,
    pub effects_dirty: bool,
    pub last_hurt_by_mob: Option<O::Actor>,
    pub last_hurt_by_mob_timestamp: i32,
    pub last_hurt_mob: Option<O::Actor>,
    pub last_hurt_mob_timestamp: i32,
    pub speed: f32,
    pub no_jump_delay: i32,
    pub absorption_amount: f32,
    pub use_item: Option<O::ItemStack>,
    pub use_item_remaining: i32,
    pub fall_fly_ticks: i32,
    pub last_position: Option<O::BlockPos>,
    /// Outer None = Java null BEFORE initializer; Some(None) = Optional.empty.
    pub last_climbable_position: Option<Option<O::BlockPos>>,
    pub last_damage_source: Option<O::DamageSource>,
    pub last_damage_stamp: i64,
    pub auto_spin_attack_ticks: i32,
    pub auto_spin_attack_damage: f32,
    pub auto_spin_attack_item: Option<O::ItemStack>,
    pub swim_amount: f32,
    pub old_swim_amount: f32,
    pub discard_friction: bool,
    pub skip_drop_experience: bool,
    pub applied_scale: f32,
    pub combat_tracker:
        Option<LivingCombatTracker<Tracker::Actor, O::DamageSource, O::FallLocation>>,
    pub active_effects: Option<O::ActiveEffectsMap>,
    pub last_hand_items: Option<EquipmentHistory<O::ItemStack, 2>>,
    pub last_armor_items: Option<EquipmentHistory<O::ItemStack, 4>>,
    pub last_body_item: Option<O::ItemStack>,
    pub walk_animation: Option<WalkAnimationState>,
    pub attributes:
        Option<AttributeMapState<O::AttributeHolder, O::AttributeInstance, O::AttributeSupplier>>,
    pub brain: Option<O::Brain>,
    pub active_location_enchantments: Option<O::LocationEnchantmentsMap>,
}
impl<O: LivingObjects, Tracker: LivingTrackerStorage<O>> LivingEntityState<O, Tracker> {
    /// Raw Java allocation defaults only. Attach this SAME state to the actor
    /// before Entity's virtual callbacks; never interpret this as a constructor
    /// completion or fill missing source references on a successful fallback.
    pub fn jvm_zeroed() -> Self {
        Self {
            swinging: false,
            swinging_arm: None,
            swing_time: 0,
            remove_arrow_time: 0,
            remove_stinger_time: 0,
            hurt_time: 0,
            hurt_duration: 0,
            death_time: 0,
            old_attack_anim: 0.0,
            attack_anim: 0.0,
            attack_strength_ticker: 0,
            invulnerable_duration: 0,
            time_offset: 0.0,
            rot_a: 0.0,
            body_yaw: 0.0,
            old_body_yaw: 0.0,
            head_yaw: 0.0,
            old_head_yaw: 0.0,
            last_hurt_by_player: None,
            last_hurt_by_player_time: 0,
            dead: false,
            no_action_time: 0,
            old_run: 0.0,
            run: 0.0,
            animation_step: 0.0,
            old_animation_step: 0.0,
            rotation_offset: 0.0,
            death_score: 0,
            last_hurt: 0.0,
            jumping: false,
            xxa: 0.0,
            yya: 0.0,
            zza: 0.0,
            lerp_steps: 0,
            lerp_x: 0.0,
            lerp_y: 0.0,
            lerp_z: 0.0,
            lerp_yaw: 0.0,
            lerp_pitch: 0.0,
            lerp_head_yaw: 0.0,
            lerp_head_steps: 0,
            effects_dirty: false,
            last_hurt_by_mob: None,
            last_hurt_by_mob_timestamp: 0,
            last_hurt_mob: None,
            last_hurt_mob_timestamp: 0,
            speed: 0.0,
            no_jump_delay: 0,
            absorption_amount: 0.0,
            use_item: None,
            use_item_remaining: 0,
            fall_fly_ticks: 0,
            last_position: None,
            last_climbable_position: None,
            last_damage_source: None,
            last_damage_stamp: 0,
            auto_spin_attack_ticks: 0,
            auto_spin_attack_damage: 0.0,
            auto_spin_attack_item: None,
            swim_amount: 0.0,
            old_swim_amount: 0.0,
            discard_friction: false,
            skip_drop_experience: false,
            applied_scale: 0.0,
            combat_tracker: None,
            active_effects: None,
            last_hand_items: None,
            last_armor_items: None,
            last_body_item: None,
            walk_animation: None,
            attributes: None,
            brain: None,
            active_location_enchantments: None,
        }
    }

    /// Callback-free actual AI-field projection. TickCount remains physical.
    pub fn ai_int(&self, physical: &EntityPhysicalState, field: AiIntField) -> i32 {
        match field {
            AiIntField::NoJumpDelay => self.no_jump_delay,
            AiIntField::LerpSteps => self.lerp_steps,
            AiIntField::LerpHeadSteps => self.lerp_head_steps,
            AiIntField::AutoSpinAttackTicks => self.auto_spin_attack_ticks,
            AiIntField::TickCount => physical.tick_count,
        }
    }
    pub fn set_ai_int(
        &mut self,
        physical: &mut EntityPhysicalState,
        field: AiIntField,
        value: i32,
    ) {
        match field {
            AiIntField::NoJumpDelay => self.no_jump_delay = value,
            AiIntField::LerpSteps => self.lerp_steps = value,
            AiIntField::LerpHeadSteps => self.lerp_head_steps = value,
            AiIntField::AutoSpinAttackTicks => self.auto_spin_attack_ticks = value,
            AiIntField::TickCount => physical.tick_count = value,
        }
    }
    pub fn ai_double(&self, field: AiDoubleField) -> f64 {
        match field {
            AiDoubleField::LerpX => self.lerp_x,
            AiDoubleField::LerpY => self.lerp_y,
            AiDoubleField::LerpZ => self.lerp_z,
            AiDoubleField::LerpYRot => self.lerp_yaw,
            AiDoubleField::LerpXRot => self.lerp_pitch,
            AiDoubleField::LerpYHeadRot => self.lerp_head_yaw,
        }
    }
    pub fn ai_float(&self, field: AiFloatField) -> f32 {
        match field {
            AiFloatField::Xxa => self.xxa,
            AiFloatField::Yya => self.yya,
            AiFloatField::Zza => self.zza,
        }
    }
    pub fn set_ai_float(&mut self, field: AiFloatField, value: f32) {
        match field {
            AiFloatField::Xxa => self.xxa = value,
            AiFloatField::Yya => self.yya = value,
            AiFloatField::Zza => self.zza = value,
        }
    }
    pub fn base_int(&self, physical: &EntityPhysicalState, field: LivingIntField) -> i32 {
        match field {
            LivingIntField::HurtTime => self.hurt_time,
            LivingIntField::InvulnerableTime => physical.invulnerable_time,
            LivingIntField::LastHurtByPlayerTime => self.last_hurt_by_player_time,
            LivingIntField::TickCount => physical.tick_count,
            LivingIntField::LastHurtByMobTimestamp => self.last_hurt_by_mob_timestamp,
        }
    }
    pub fn set_base_int(
        &mut self,
        physical: &mut EntityPhysicalState,
        field: LivingIntField,
        value: i32,
    ) {
        match field {
            LivingIntField::HurtTime => self.hurt_time = value,
            LivingIntField::InvulnerableTime => physical.invulnerable_time = value,
            LivingIntField::LastHurtByPlayerTime => self.last_hurt_by_player_time = value,
            LivingIntField::TickCount => physical.tick_count = value,
            LivingIntField::LastHurtByMobTimestamp => self.last_hurt_by_mob_timestamp = value,
        }
    }
    pub fn base_float(&self, physical: &EntityPhysicalState, field: LivingFloatField) -> f32 {
        match field {
            LivingFloatField::AttackAnim => self.attack_anim,
            LivingFloatField::OldAttackAnim => self.old_attack_anim,
            LivingFloatField::AnimationStep => self.animation_step,
            LivingFloatField::OldAnimationStep => self.old_animation_step,
            LivingFloatField::BodyYaw => self.body_yaw,
            LivingFloatField::OldBodyYaw => self.old_body_yaw,
            LivingFloatField::HeadYaw => self.head_yaw,
            LivingFloatField::OldHeadYaw => self.old_head_yaw,
            LivingFloatField::OldYaw => physical.old_yaw,
            LivingFloatField::OldPitch => physical.old_pitch,
        }
    }
    pub fn set_base_float(
        &mut self,
        physical: &mut EntityPhysicalState,
        field: LivingFloatField,
        value: f32,
    ) {
        match field {
            LivingFloatField::AttackAnim => self.attack_anim = value,
            LivingFloatField::OldAttackAnim => self.old_attack_anim = value,
            LivingFloatField::AnimationStep => self.animation_step = value,
            LivingFloatField::OldAnimationStep => self.old_animation_step = value,
            LivingFloatField::BodyYaw => self.body_yaw = value,
            LivingFloatField::OldBodyYaw => self.old_body_yaw = value,
            LivingFloatField::HeadYaw => self.head_yaw = value,
            LivingFloatField::OldHeadYaw => self.old_head_yaw = value,
            LivingFloatField::OldYaw => physical.old_yaw = value,
            LivingFloatField::OldPitch => physical.old_pitch = value,
        }
    }
}

/// Borrowed actual Entity.xo/zo fields. A caller may bind PlayerMotion's
/// previous_position.x/z; EntityPhysicalState.old_position is xOld, NOT this.
/// A Bat's genuine Entity history owner is still a central attachment seam.
/// If that owner is unavailable, the host must error before projecting tick.
pub struct SourcePositionHistory<'a> {
    xo: &'a f64,
    zo: &'a f64,
}
impl<'a> SourcePositionHistory<'a> {
    pub fn new(actual_xo: &'a f64, actual_zo: &'a f64) -> Self {
        Self {
            xo: actual_xo,
            zo: actual_zo,
        }
    }
}

/// Short combined field view only. Never hold it across a source callback.
pub struct LivingTickProjection<
    'a,
    O: LivingObjects,
    Tracker: LivingTrackerStorage<O> = RetainedLivingTracker,
> {
    pub living: &'a mut LivingEntityState<O, Tracker>,
    pub physical: &'a mut EntityPhysicalState,
    pub history: SourcePositionHistory<'a>,
}
impl<O: LivingObjects, Tracker: LivingTrackerStorage<O>> LivingTickFields
    for LivingTickProjection<'_, O, Tracker>
{
    fn int(&self, field: TickIntField) -> i32 {
        match field {
            TickIntField::RemoveArrowTime => self.living.remove_arrow_time,
            TickIntField::RemoveStingerTime => self.living.remove_stinger_time,
            TickIntField::TickCount => self.physical.tick_count,
            TickIntField::FallFlyTicks => self.living.fall_fly_ticks,
        }
    }
    fn set_int(&mut self, field: TickIntField, value: i32) {
        match field {
            TickIntField::RemoveArrowTime => self.living.remove_arrow_time = value,
            TickIntField::RemoveStingerTime => self.living.remove_stinger_time = value,
            TickIntField::TickCount => self.physical.tick_count = value,
            TickIntField::FallFlyTicks => self.living.fall_fly_ticks = value,
        }
    }
    fn float(&self, field: TickFloatField) -> f32 {
        match field {
            TickFloatField::BodyYaw => self.living.body_yaw,
            TickFloatField::Run => self.living.run,
            TickFloatField::OldRun => self.living.old_run,
            TickFloatField::AttackAnim => self.living.attack_anim,
            TickFloatField::OldYaw => self.physical.old_yaw,
            TickFloatField::OldBodyYaw => self.living.old_body_yaw,
            TickFloatField::OldPitch => self.physical.old_pitch,
            TickFloatField::HeadYaw => self.living.head_yaw,
            TickFloatField::OldHeadYaw => self.living.old_head_yaw,
            TickFloatField::AnimationStep => self.living.animation_step,
            TickFloatField::AppliedScale => self.living.applied_scale,
            TickFloatField::SwimAmount => self.living.swim_amount,
            TickFloatField::OldSwimAmount => self.living.old_swim_amount,
        }
    }
    fn set_float(&mut self, field: TickFloatField, value: f32) {
        match field {
            TickFloatField::BodyYaw => self.living.body_yaw = value,
            TickFloatField::Run => self.living.run = value,
            TickFloatField::OldRun => self.living.old_run = value,
            TickFloatField::AttackAnim => self.living.attack_anim = value,
            TickFloatField::OldYaw => self.physical.old_yaw = value,
            TickFloatField::OldBodyYaw => self.living.old_body_yaw = value,
            TickFloatField::OldPitch => self.physical.old_pitch = value,
            TickFloatField::HeadYaw => self.living.head_yaw = value,
            TickFloatField::OldHeadYaw => self.living.old_head_yaw = value,
            TickFloatField::AnimationStep => self.living.animation_step = value,
            TickFloatField::AppliedScale => self.living.applied_scale = value,
            TickFloatField::SwimAmount => self.living.swim_amount = value,
            TickFloatField::OldSwimAmount => self.living.old_swim_amount = value,
        }
    }
    fn xo(&self) -> f64 {
        *self.history.xo
    }
    fn zo(&self) -> f64 {
        *self.history.zo
    }
}

/// Ordered Living initialization AFTER the actual Entity constructor. This
/// does not supply or pretend to execute that parent, metadata definition,
/// Mob/Bat construction, admission, or Entity.packetPositionCodec ownership.
pub trait LivingConstructionHost {
    type Error;
    type Actor: Clone;
    type Objects: LivingObjects<Actor = Self::Actor>;
    type EntityType;

    /// Same canonical state attached BEFORE Entity's virtual callbacks. Closure
    /// is callback-free and releases all actor/field borrows before returning.
    /// Errors precede access; completed fragments/draws are never rolled back.
    fn with_living_state<R>(
        &mut self,
        actor: &Self::Actor,
        access: impl FnOnce(&mut LivingEntityState<Self::Objects>) -> R,
    ) -> Result<R, Self::Error>;
    /// Actual Maps.newHashMap with source canonical keys/instances/live cursors.
    /// The current effect executors do not themselves supply this owner.
    fn new_active_effects_map(
        &mut self,
    ) -> Result<<Self::Objects as LivingObjects>::ActiveEffectsMap, Self::Error>;
    /// Actual nonnull shared ItemStack.EMPTY. Never construct a fresh empty
    /// stack/value clone, obtain it from an inventory snapshot, or consume RNG.
    fn canonical_empty_stack(
        &mut self,
    ) -> Result<<Self::Objects as LivingObjects>::ItemStack, Self::Error>;
    /// Actual Reference2ObjectArrayMap keyed by definition identity. No sorted
    /// map or `empty=true` marker can stand in for its mutable source owner.
    fn new_location_enchantments_map(
        &mut self,
    ) -> Result<<Self::Objects as LivingObjects>::LocationEnchantmentsMap, Self::Error>;
    /// DefaultAttributes.getSupplier on the RETAINED constructor type argument,
    /// not a new type lookup or peractor copy of the shared immutable template.
    fn default_attribute_supplier(
        &mut self,
        entity_type: &Self::EntityType,
    ) -> Result<<Self::Objects as LivingObjects>::AttributeSupplier, Self::Error>;
    /// Actual final getMaxHealth, including virtual getAttributeValue dispatch.
    fn max_health(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Actual virtual setHealth, current max-health lookup and metadata9 owner.
    fn set_health(&mut self, actor: &Self::Actor, health: f32) -> Result<(), Self::Error>;
    /// Actual Entity.blocksBuilding(J) field, currently a central Entity seam.
    fn set_blocks_building(&mut self, actor: &Self::Actor, value: bool) -> Result<(), Self::Error>;
    /// ONE actual global java.lang.Math.random double, independent of Entity
    /// and Level random streams. Three calls are deliberately interleaved.
    fn next_global_math_double(&mut self) -> Result<f64, Self::Error>;
    fn reapply_position(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn set_yaw(&mut self, actor: &Self::Actor, yaw: f32) -> Result<(), Self::Error>;
    fn yaw(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Actual empty-memories Dynamic -> virtual makeBrain/provider -> real NEW
    /// peractor Brain object. Missing Brain/codec owner must return an error.
    fn make_brain_from_empty_memories(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<<Self::Objects as LivingObjects>::Brain, Self::Error>;
}

/// Specialized storage projection for a graph-internal CombatTracker backlink.
/// Public Actor is still a genuine retained source handle. TrackerActor may be
/// an aggregate-private backlink ONLY when every escaped tracker lease roots
/// that same graph. No source callback may expose the backlink as an Actor.
pub trait LivingConstructionFields<Tracker>
where
    Tracker: LivingTrackerStorage<Self::Objects>,
{
    type Error;
    type Actor: Clone;
    type Objects: LivingObjects<Actor = Self::Actor>;
    type EntityType;

    /// Same canonical state attached BEFORE Entity's virtual callbacks. Closure
    /// is callback-free and releases all actor/field borrows before returning.
    /// Errors precede access; completed fragments/draws are never rolled back.
    fn with_living_state<R>(
        &mut self,
        actor: &Self::Actor,
        access: impl FnOnce(&mut LivingEntityState<Self::Objects, Tracker>) -> R,
    ) -> Result<R, Self::Error>;
    /// Actual Maps.newHashMap with source canonical keys/instances/live cursors.
    /// The current effect executors do not themselves supply this owner.
    fn new_active_effects_map(
        &mut self,
    ) -> Result<<Self::Objects as LivingObjects>::ActiveEffectsMap, Self::Error>;
    /// Actual nonnull shared ItemStack.EMPTY. Never construct a fresh empty
    /// stack/value clone, obtain it from an inventory snapshot, or consume RNG.
    fn canonical_empty_stack(
        &mut self,
    ) -> Result<<Self::Objects as LivingObjects>::ItemStack, Self::Error>;
    /// Actual Reference2ObjectArrayMap keyed by definition identity. No sorted
    /// map or `empty=true` marker can stand in for its mutable source owner.
    fn new_location_enchantments_map(
        &mut self,
    ) -> Result<<Self::Objects as LivingObjects>::LocationEnchantmentsMap, Self::Error>;
    /// DefaultAttributes.getSupplier on the RETAINED constructor type argument,
    /// not a new type lookup or peractor copy of the shared immutable template.
    fn default_attribute_supplier(
        &mut self,
        entity_type: &Self::EntityType,
    ) -> Result<<Self::Objects as LivingObjects>::AttributeSupplier, Self::Error>;
    /// Actual final getMaxHealth, including virtual getAttributeValue dispatch.
    fn max_health(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Actual virtual setHealth, current max-health lookup and metadata9 owner.
    fn set_health(&mut self, actor: &Self::Actor, health: f32) -> Result<(), Self::Error>;
    /// Actual Entity.blocksBuilding(J) field, currently a central Entity seam.
    fn set_blocks_building(&mut self, actor: &Self::Actor, value: bool) -> Result<(), Self::Error>;
    /// ONE actual global java.lang.Math.random double, independent of Entity
    /// and Level random streams. Three calls are deliberately interleaved.
    fn next_global_math_double(&mut self) -> Result<f64, Self::Error>;
    fn reapply_position(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn set_yaw(&mut self, actor: &Self::Actor, yaw: f32) -> Result<(), Self::Error>;
    fn yaw(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Actual empty-memories Dynamic -> virtual makeBrain/provider -> real NEW
    /// peractor Brain object. Missing Brain/codec owner must return an error.
    fn make_brain_from_empty_memories(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<<Self::Objects as LivingObjects>::Brain, Self::Error>;
}

// Preserve every existing implementation of LivingConstructionHost unchanged.
impl<H: LivingConstructionHost> LivingConstructionFields<RetainedLivingTracker> for H {
    type Error = H::Error;
    type Actor = H::Actor;
    type Objects = H::Objects;
    type EntityType = H::EntityType;
    fn with_living_state<R>(
        &mut self,
        a: &Self::Actor,
        f: impl FnOnce(&mut LivingEntityState<Self::Objects>) -> R,
    ) -> Result<R, Self::Error> {
        LivingConstructionHost::with_living_state(self, a, f)
    }
    fn new_active_effects_map(
        &mut self,
    ) -> Result<<Self::Objects as LivingObjects>::ActiveEffectsMap, Self::Error> {
        LivingConstructionHost::new_active_effects_map(self)
    }
    fn canonical_empty_stack(
        &mut self,
    ) -> Result<<Self::Objects as LivingObjects>::ItemStack, Self::Error> {
        LivingConstructionHost::canonical_empty_stack(self)
    }
    fn new_location_enchantments_map(
        &mut self,
    ) -> Result<<Self::Objects as LivingObjects>::LocationEnchantmentsMap, Self::Error> {
        LivingConstructionHost::new_location_enchantments_map(self)
    }
    fn default_attribute_supplier(
        &mut self,
        t: &Self::EntityType,
    ) -> Result<<Self::Objects as LivingObjects>::AttributeSupplier, Self::Error> {
        LivingConstructionHost::default_attribute_supplier(self, t)
    }
    fn max_health(&mut self, a: &Self::Actor) -> Result<f32, Self::Error> {
        LivingConstructionHost::max_health(self, a)
    }
    fn set_health(&mut self, a: &Self::Actor, v: f32) -> Result<(), Self::Error> {
        LivingConstructionHost::set_health(self, a, v)
    }
    fn set_blocks_building(&mut self, a: &Self::Actor, v: bool) -> Result<(), Self::Error> {
        LivingConstructionHost::set_blocks_building(self, a, v)
    }
    fn next_global_math_double(&mut self) -> Result<f64, Self::Error> {
        LivingConstructionHost::next_global_math_double(self)
    }
    fn reapply_position(&mut self, a: &Self::Actor) -> Result<(), Self::Error> {
        LivingConstructionHost::reapply_position(self, a)
    }
    fn set_yaw(&mut self, a: &Self::Actor, v: f32) -> Result<(), Self::Error> {
        LivingConstructionHost::set_yaw(self, a, v)
    }
    fn yaw(&mut self, a: &Self::Actor) -> Result<f32, Self::Error> {
        LivingConstructionHost::yaw(self, a)
    }
    fn make_brain_from_empty_memories(
        &mut self,
        a: &Self::Actor,
    ) -> Result<<Self::Objects as LivingObjects>::Brain, Self::Error> {
        LivingConstructionHost::make_brain_from_empty_memories(self, a)
    }
}

/// Existing retained-Actor constructor API, with its original field owner.
pub fn initialize_living_fields<H: LivingConstructionHost>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    initialize_living_fields_with_tracker::<RetainedLivingTracker, H>(actor, actor.clone(), host)
}

/// Existing constructor API. Specialized aggregates reuse the SAME source body
/// through construct_living_after_entity_with_tracker below.
pub fn construct_living_after_entity<H: LivingConstructionHost>(
    actor: &H::Actor,
    entity_type: &H::EntityType,
    host: &mut H,
) -> Result<(), H::Error> {
    construct_living_after_entity_with_tracker::<RetainedLivingTracker, H>(
        actor,
        entity_type,
        actor.clone(),
        host,
    )
}

/// Exact Living field-initializer prefix187–267 after Entity returns. A caller
/// must execute it at that source position, once, on the same staged object.
pub fn initialize_living_fields_with_tracker<Tracker, H>(
    actor: &H::Actor,
    tracker_actor: Tracker::Actor,
    host: &mut H,
) -> Result<(), H::Error>
where
    H: LivingConstructionFields<Tracker>,
    Tracker: LivingTrackerStorage<H::Objects>,
{
    let tracker = LivingCombatTracker {
        actor: tracker_actor,
        state: CombatTracker::new(),
    };
    host.with_living_state(actor, |s| s.combat_tracker = Some(tracker))?;
    let effects = host.new_active_effects_map()?;
    host.with_living_state(actor, |s| s.active_effects = Some(effects))?;
    let empty = host.canonical_empty_stack()?;
    let hands = EquipmentHistory::new(empty);
    host.with_living_state(actor, |s| s.last_hand_items = Some(hands))?;
    let empty = host.canonical_empty_stack()?;
    let armor = EquipmentHistory::new(empty);
    host.with_living_state(actor, |s| s.last_armor_items = Some(armor))?;
    let empty = host.canonical_empty_stack()?;
    host.with_living_state(actor, |s| {
        s.last_body_item = Some(empty);
        s.discard_friction = false;
        s.walk_animation = Some(WalkAnimationState::new());
        s.invulnerable_duration = 20;
        s.effects_dirty = true;
    })?;
    let empty = host.canonical_empty_stack()?;
    host.with_living_state(actor, |s| {
        s.use_item = Some(empty);
        s.last_climbable_position = Some(None);
    })?;
    let locations = host.new_location_enchantments_map()?;
    host.with_living_state(actor, |s| {
        s.active_location_enchantments = Some(locations);
        s.applied_scale = 1.0;
    })
}

/// Exact Living constructor272–284 plus its preceding field initializers.
/// The actual Entity parent must ALREADY have run on this staged actor. Native
/// AttributeMap/CombatTracker objects are constructed here, not successful
/// owner placeholders. No actor is admitted and no physical/health/RNG copy is
/// created. A partial error leaves all prior real assignments/draws committed.
pub fn construct_living_after_entity_with_tracker<Tracker, H>(
    actor: &H::Actor,
    entity_type: &H::EntityType,
    tracker_actor: Tracker::Actor,
    host: &mut H,
) -> Result<(), H::Error>
where
    H: LivingConstructionFields<Tracker>,
    Tracker: LivingTrackerStorage<H::Objects>,
{
    initialize_living_fields_with_tracker::<Tracker, H>(actor, tracker_actor, host)?;
    let supplier = host.default_attribute_supplier(entity_type)?;
    let attributes = AttributeMapState::new(supplier);
    host.with_living_state(actor, |s| s.attributes = Some(attributes))?;
    let maximum = host.max_health(actor)?;
    host.set_health(actor, maximum)?;
    host.set_blocks_building(actor, true)?;
    let draw = host.next_global_math_double()?;
    let rot_a = ((draw + 1.0_f64) * 0.009999999776482582_f64) as f32;
    host.with_living_state(actor, |s| s.rot_a = rot_a)?;
    host.reapply_position(actor)?;
    let draw = host.next_global_math_double()?;
    let time_offset = (draw as f32) * 12398.0_f32;
    host.with_living_state(actor, |s| s.time_offset = time_offset)?;
    let draw = host.next_global_math_double()?;
    host.set_yaw(actor, (draw * 6.2831854820251465_f64) as f32)?;
    let yaw = host.yaw(actor)?;
    host.with_living_state(actor, |s| s.head_yaw = yaw)?;
    let brain = host.make_brain_from_empty_memories(actor)?;
    host.with_living_state(actor, |s| s.brain = Some(brain))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    // Controlled reference owners for constructor/projection observations only.
    // These do not implement an effect map, Brain, entity or Main integration.
    struct Objects;
    type Actor = Rc<RefCell<LivingEntityState<Objects>>>;
    #[derive(Clone, PartialEq, Eq)]
    struct Key(i32);
    impl JavaObjectKey for Key {
        fn java_hash_code(&self) -> i32 {
            self.0
        }
    }
    struct Stack(u32);
    struct Supplier(u32);
    struct Effects;
    struct Brain;
    struct Locations;
    impl LivingObjects for Objects {
        type Actor = Actor;
        type ItemStack = Rc<Stack>;
        type BlockPos = (i32, i32, i32);
        type DamageSource = Rc<str>;
        type FallLocation = &'static str;
        type AttributeHolder = Key;
        type AttributeInstance = Key;
        type AttributeSupplier = Rc<Supplier>;
        type ActiveEffectsMap = Effects;
        type Brain = Brain;
        type LocationEnchantmentsMap = Locations;
    }
    fn actor() -> Actor {
        Rc::new(RefCell::new(LivingEntityState::jvm_zeroed()))
    }

    struct Host {
        trace: Vec<&'static str>,
        fail: Option<&'static str>,
        draws: Vec<f64>,
        draw_index: usize,
        empty: Rc<Stack>,
        supplier: Rc<Supplier>,
        type_argument: Rc<str>,
        health_arguments: Vec<f32>,
        yaw: f32,
        blocks_building_arguments: Vec<bool>,
        replace_live_on_reapply: bool,
        live_actor: Actor,
    }
    impl Host {
        fn new(a: &Actor) -> Self {
            Self {
                trace: Vec::new(),
                fail: None,
                draws: vec![0.125, 0.375, 0.625],
                draw_index: 0,
                empty: Rc::new(Stack(7)),
                supplier: Rc::new(Supplier(6)),
                type_argument: Rc::from("original constructor type"),
                health_arguments: Vec::new(),
                yaw: 0.0,
                blocks_building_arguments: Vec::new(),
                replace_live_on_reapply: false,
                live_actor: a.clone(),
            }
        }
        fn mark(&mut self, name: &'static str) -> Result<(), &'static str> {
            self.trace.push(name);
            if self.fail == Some(name) {
                Err(name)
            } else {
                Ok(())
            }
        }
    }
    impl LivingConstructionHost for Host {
        type Error = &'static str;
        type Actor = Actor;
        type Objects = Objects;
        type EntityType = Rc<str>;
        fn with_living_state<R>(
            &mut self,
            a: &Actor,
            f: impl FnOnce(&mut LivingEntityState<Objects>) -> R,
        ) -> Result<R, Self::Error> {
            Ok(f(&mut a.borrow_mut()))
        }
        fn new_active_effects_map(&mut self) -> Result<Effects, Self::Error> {
            self.mark("new_effects")?;
            Ok(Effects)
        }
        fn canonical_empty_stack(&mut self) -> Result<Rc<Stack>, Self::Error> {
            self.mark("empty")?;
            Ok(self.empty.clone())
        }
        fn new_location_enchantments_map(&mut self) -> Result<Locations, Self::Error> {
            self.mark("new_locations")?;
            Ok(Locations)
        }
        fn default_attribute_supplier(&mut self, t: &Rc<str>) -> Result<Rc<Supplier>, Self::Error> {
            self.mark("supplier")?;
            assert!(Rc::ptr_eq(t, &self.type_argument));
            Ok(self.supplier.clone())
        }
        fn max_health(&mut self, a: &Actor) -> Result<f32, Self::Error> {
            self.mark("max_health")?;
            let s = a.borrow();
            assert!(Rc::ptr_eq(
                s.attributes.as_ref().unwrap().supplier(),
                &self.supplier
            ));
            assert_eq!(s.attributes.as_ref().unwrap().instantiated_len(), 0);
            assert_eq!(s.attributes.as_ref().unwrap().supplier().0, 6);
            assert_eq!(s.invulnerable_duration, 20);
            assert!(s.effects_dirty);
            assert!(s.brain.is_none());
            Ok(6.0)
        }
        fn set_health(&mut self, a: &Actor, value: f32) -> Result<(), Self::Error> {
            self.mark("set_health")?;
            // Reentry observes the actual initialized object, then mutates it.
            assert!(a.borrow().combat_tracker.is_some());
            a.borrow_mut().dead = true;
            self.health_arguments.push(value);
            Ok(())
        }
        fn set_blocks_building(&mut self, _: &Actor, v: bool) -> Result<(), Self::Error> {
            self.mark("blocks_building")?;
            self.blocks_building_arguments.push(v);
            Ok(())
        }
        fn next_global_math_double(&mut self) -> Result<f64, Self::Error> {
            self.mark("global_math")?;
            let draw = self.draws[self.draw_index];
            self.draw_index += 1;
            Ok(draw)
        }
        fn reapply_position(&mut self, a: &Actor) -> Result<(), Self::Error> {
            self.mark("reapply")?;
            assert_eq!(self.draw_index, 1);
            assert_eq!(a.borrow().rot_a.to_bits(), 0x3c38_51eb);
            a.borrow_mut().time_offset = -99.0;
            if self.replace_live_on_reapply {
                self.live_actor = actor();
            }
            Ok(())
        }
        fn set_yaw(&mut self, a: &Actor, yaw: f32) -> Result<(), Self::Error> {
            self.mark("set_yaw")?;
            assert_eq!(self.draw_index, 3);
            assert_eq!(a.borrow().time_offset, 4649.25);
            self.yaw = yaw + 17.0; // Actual virtual setter mutation is observable.
            Ok(())
        }
        fn yaw(&mut self, a: &Actor) -> Result<f32, Self::Error> {
            self.mark("yaw")?;
            assert!(a.borrow().dead);
            Ok(self.yaw)
        }
        fn make_brain_from_empty_memories(&mut self, a: &Actor) -> Result<Brain, Self::Error> {
            self.mark("brain")?;
            assert_eq!(a.borrow().head_yaw, self.yaw);
            assert!(a.borrow().brain.is_none());
            Ok(Brain)
        }
    }

    #[test]
    fn java_zero_then_initializers_preserve_actual_reference_identity() {
        let a = actor();
        {
            let mut s = a.borrow_mut();
            assert!(!s.dead);
            assert_eq!(s.absorption_amount.to_bits(), 0);
            assert_eq!(s.applied_scale.to_bits(), 0);
            assert!(s.combat_tracker.is_none() && s.attributes.is_none());
            assert!(s.active_effects.is_none() && s.brain.is_none());
            assert!(s.last_hand_items.is_none() && s.walk_animation.is_none());
            assert!(s.use_item.is_none() && s.last_climbable_position.is_none());
            s.discard_friction = true;
        }
        let mut h = Host::new(&a);
        initialize_living_fields(&a, &mut h).unwrap();
        let s = a.borrow();
        assert!(Rc::ptr_eq(&s.combat_tracker.as_ref().unwrap().actor, &a));
        assert!(s
            .combat_tracker
            .as_ref()
            .unwrap()
            .state
            .entries()
            .is_empty());
        for item in s
            .last_hand_items
            .as_ref()
            .unwrap()
            .items()
            .iter()
            .chain(s.last_armor_items.as_ref().unwrap().items())
        {
            assert!(Rc::ptr_eq(item, &h.empty));
            assert_eq!(item.0, 7);
        }
        assert!(Rc::ptr_eq(s.last_body_item.as_ref().unwrap(), &h.empty));
        assert!(Rc::ptr_eq(s.use_item.as_ref().unwrap(), &h.empty));
        assert_eq!(s.last_climbable_position, Some(None));
        assert!(!s.discard_friction);
        assert!(s.effects_dirty);
        assert_eq!(s.invulnerable_duration, 20);
        assert_eq!(s.applied_scale, 1.0);
        assert!(s.auto_spin_attack_item.is_none());
        assert!(s.attributes.is_none() && s.brain.is_none());
        assert_eq!(
            h.trace,
            [
                "new_effects",
                "empty",
                "empty",
                "empty",
                "empty",
                "new_locations"
            ]
        );
    }

    #[test]
    fn construction_interleaves_global_draws_and_retains_same_actor_through_reentry() {
        let a = actor();
        let mut h = Host::new(&a);
        h.replace_live_on_reapply = true;
        let t = h.type_argument.clone();
        construct_living_after_entity(&a, &t, &mut h).unwrap();
        assert_eq!(
            h.trace,
            [
                "new_effects",
                "empty",
                "empty",
                "empty",
                "empty",
                "new_locations",
                "supplier",
                "max_health",
                "set_health",
                "blocks_building",
                "global_math",
                "reapply",
                "global_math",
                "global_math",
                "set_yaw",
                "yaw",
                "brain"
            ]
        );
        assert_eq!(h.health_arguments, [6.0]);
        assert_eq!(h.blocks_building_arguments, [true]);
        assert_eq!(h.draw_index, 3);
        assert!(!Rc::ptr_eq(&a, &h.live_actor));
        assert!(a.borrow().brain.is_some());
        assert!(a.borrow().dead); // Constructor does not recompute actual be.
        assert_eq!(
            a.borrow().head_yaw,
            (0.625_f64 * 6.2831854820251465) as f32 + 17.0
        );
        assert!(h.live_actor.borrow().attributes.is_none());
        assert_eq!(h.live_actor.borrow().head_yaw.to_bits(), 0);
    }

    #[test]
    fn errors_retain_only_the_source_constructor_prefix() {
        let a = actor();
        let mut h = Host::new(&a);
        h.fail = Some("new_effects");
        a.borrow_mut().discard_friction = true;
        assert_eq!(initialize_living_fields(&a, &mut h), Err("new_effects"));
        assert!(a.borrow().combat_tracker.is_some());
        assert!(a.borrow().active_effects.is_none());
        assert!(a.borrow().last_hand_items.is_none());
        assert!(a.borrow().discard_friction);

        let a = actor();
        let mut h = Host::new(&a);
        let t = h.type_argument.clone();
        h.fail = Some("reapply");
        assert_eq!(
            construct_living_after_entity(&a, &t, &mut h),
            Err("reapply")
        );
        assert_eq!(h.draw_index, 1);
        assert_eq!(a.borrow().rot_a.to_bits(), 0x3c38_51eb);
        assert_eq!(a.borrow().time_offset.to_bits(), 0);
        assert_eq!(a.borrow().head_yaw.to_bits(), 0);
        assert!(a.borrow().attributes.is_some() && a.borrow().brain.is_none());
        assert_eq!(h.trace.last(), Some(&"reapply"));

        let a = actor();
        let mut h = Host::new(&a);
        let t = h.type_argument.clone();
        h.fail = Some("brain");
        assert_eq!(construct_living_after_entity(&a, &t, &mut h), Err("brain"));
        assert_eq!(h.draw_index, 3);
        assert_eq!(a.borrow().head_yaw, h.yaw);
        assert!(a.borrow().brain.is_none());
    }

    #[test]
    fn history_list_clear_restores_the_same_default_object() {
        let empty = Rc::new(Stack(1));
        let replacement = Rc::new(Stack(2));
        let mut list = EquipmentHistory::<_, 2>::new(empty.clone());
        let old = list.set(0, replacement.clone());
        assert!(Rc::ptr_eq(&old, &empty));
        assert!(Rc::ptr_eq(list.get(0), &replacement));
        list.clear();
        assert!(list.items().iter().all(|item| Rc::ptr_eq(item, &empty)));
    }

    #[test]
    fn projections_write_canonical_fields_and_require_distinct_source_history() {
        use crate::{entity::Position, geometry::Vec3};
        let mut p = EntityPhysicalState::new(
            8,
            [3; 16],
            Position {
                x: 2.0,
                y: 3.0,
                z: 4.0,
                yaw: 5.0,
                pitch: 6.0,
                on_ground: false,
            },
            Vec3::ZERO,
            0.5,
            0.9,
            0.45,
        )
        .unwrap();
        let mut s = LivingEntityState::<Objects>::jvm_zeroed();
        p.old_position = Vec3::new(100.0, 200.0, 300.0); // xOld, not xo.
        p.invulnerable_time = 17;
        s.invulnerable_duration = 20;
        let (xo, zo) = (1.25, -8.5);
        {
            let mut fields = LivingTickProjection {
                living: &mut s,
                physical: &mut p,
                history: SourcePositionHistory::new(&xo, &zo),
            };
            assert_eq!((fields.xo(), fields.zo()), (1.25, -8.5));
            fields.set_int(TickIntField::TickCount, i32::MIN);
            fields.set_float(TickFloatField::OldYaw, -0.0);
            fields.set_float(TickFloatField::BodyYaw, f32::NAN);
            fields.set_int(TickIntField::FallFlyTicks, 77);
        }
        assert_eq!(p.tick_count, i32::MIN);
        assert_eq!(p.old_yaw.to_bits(), (-0.0_f32).to_bits());
        assert!(s.body_yaw.is_nan());
        assert_eq!(s.fall_fly_ticks, 77);
        assert_eq!(s.ai_int(&p, AiIntField::TickCount), i32::MIN);
        s.set_ai_int(&mut p, AiIntField::TickCount, 45);
        s.set_ai_int(&mut p, AiIntField::NoJumpDelay, 8);
        assert_eq!(p.tick_count, 45);
        assert_eq!(s.no_jump_delay, 8);
        assert_eq!(s.base_int(&p, LivingIntField::InvulnerableTime), 17);
        s.set_base_int(&mut p, LivingIntField::InvulnerableTime, 3);
        assert_eq!(p.invulnerable_time, 3);
        assert_eq!(s.invulnerable_duration, 20);
        s.set_base_float(&mut p, LivingFloatField::OldPitch, 13.0);
        assert_eq!(p.old_pitch, 13.0);
        // Reacquisition observes callback mutations in the SAME source fields.
        s.body_yaw = -9.0;
        let fields = LivingTickProjection {
            living: &mut s,
            physical: &mut p,
            history: SourcePositionHistory::new(&xo, &zo),
        };
        assert_eq!(fields.float(TickFloatField::BodyYaw), -9.0);
        assert_eq!(fields.physical.old_position.x, 100.0);
    }

    #[test]
    fn walk_animation_preserves_source_float_order_and_nan_predicate() {
        let mut w = WalkAnimationState::new();
        assert_eq!(
            (
                w.speed.to_bits(),
                w.speed_old.to_bits(),
                w.position.to_bits()
            ),
            (0, 0, 0)
        );
        w.set_speed(0.7);
        assert_eq!(w.speed_old.to_bits(), 0);
        assert_eq!(w.position.to_bits(), 0);
        w.update(-0.2, 0.3);
        assert_eq!(w.speed_old, 0.7);
        assert_eq!(w.speed().to_bits(), 0x3edc_28f5);
        assert_eq!(w.position().to_bits(), 0x3edc_28f5);
        assert_eq!(w.speed_at(0.4).to_bits(), 0x3f17_8d50);
        assert_eq!(w.position_at(0.4).to_bits(), 0x3e30_20c4);
        w.set_speed(1.0e-5);
        assert!(!w.is_moving());
        w.set_speed(f32::from_bits(1.0e-5_f32.to_bits() + 1));
        assert!(w.is_moving());
        w.set_speed(f32::NAN);
        assert!(!w.is_moving());
        w.update(1.0, 0.0);
        assert!(w.speed().is_nan() && w.position().is_nan());
    }
}
