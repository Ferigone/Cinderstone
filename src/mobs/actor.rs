//! Retained ordinary Bat construction and source-object ownership.
//!
//! This is a concrete aggregate of the native field/object owners. It begins
//! before Entity's virtual data definition, then runs the actual Living/Mob/Bat
//! constructor tail on that SAME object. Entity construction itself, registry
//! bootstrap, world admission, full ticking, tracking and saves remain explicit
//! embedding contracts. A physical-state value is NOT an Entity constructor.

use super::{
    bat::BatState,
    construction::{self, MobConstructionHost},
    controls::{
        body_rotation::BodyRotationState, jump_control::JumpControlState,
        look_control::LookControlState, move_control::MoveControlState,
    },
    goals::GoalSelectorState,
    navigation::{
        self, GroundConstructionHost, GroundNavigationState, NavigationError, NavigationHost,
        PathFinderState, WalkNodeEvaluatorState,
    },
    sensing::SensingState,
    state::{MobObjects, MobState, PathType, RetainedMobControl},
};
use crate::{
    effects::{
        active_map::{ActiveEffectKey, ActiveEffectMap},
        attribute_map::{self, JavaObjectKey},
        binding::{
            AttributeBindingOwner, NativeAttributeBinding, NativeAttributeInstance,
            NativeAttributeMap, NativeAttributeSupplier,
        },
        definitions::BoundAttribute,
        location_map::{EnchantmentValueReference, LocationEffectReference, LocationEnchantments},
        supplier::RegisteredAttributeHolder,
        EffectInstance,
    },
    entity_state::EntityPhysicalState,
    geometry::{Aabb, Vec3},
    living_entities::{
        brain::{DefaultBrainProvider, DefaultLivingBrain},
        metadata::{self, BatEntityData, BatMetadataHost, MetadataError, MetadataFieldError},
        state::{
            self as living_state, InternalLivingTracker, LivingConstructionFields,
            LivingEntityState, LivingObjects,
        },
    },
    metadata::MetadataValue,
    random::LegacyRandomSource,
};
use std::{
    fmt,
    sync::{Arc, Mutex, Weak},
};

/// Actual canonical external object domains. ItemStack Clone aliases one source
/// stack; holders/identity tokens use the shared registered/hash authority.
/// Level/EntityType/ProfilerSupplier Clone aliases the SAME source object.
/// No IDs, hashes, seeded RNGs or stack copies are allocated by this trait.
pub trait NativeBatTypes: Send + Sync + 'static {
    type ItemStack: Clone + Send + Sync;
    type DamageSource: Send + Sync;
    type FallLocation: Send + Sync;
    type AttributeHolder: RegisteredAttributeHolder + Send + Sync;
    type InstanceIdentity: JavaObjectKey + Send + Sync;
    type EffectHolder: ActiveEffectKey + Send + Sync;
    type EnchantmentValue: EnchantmentValueReference + Send + Sync;
    type LocationEffect: LocationEffectReference + Send + Sync;
    type EntityType: Clone + Send + Sync;
    type Level: Clone + Send + Sync;
    type ProfilerSupplier: Clone + Send + Sync;
    type LootTable: Send + Sync;
    type LeashData: Send + Sync;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatActorError {
    PoisonedState,
    ForeignActor,
    Unbound(&'static str),
    ConstructorAlreadyStarted,
    ConstructorNotComplete,
}
impl fmt::Display for BatActorError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }
}
impl std::error::Error for BatActorError {}

#[derive(Debug)]
pub enum BatConstructionError<E> {
    Actor(BatActorError),
    Services(E),
    Metadata(MetadataFieldError),
    Source { phase: &'static str, detail: String },
}
impl<E> From<BatActorError> for BatConstructionError<E> {
    fn from(error: BatActorError) -> Self {
        Self::Actor(error)
    }
}
fn source_error<E>(phase: &'static str, error: impl fmt::Debug) -> BatConstructionError<E> {
    BatConstructionError::Source {
        phase,
        detail: format!("{error:?}"),
    }
}

/// Opaque graph-INTERNAL descriptor, never a public source Actor lease.
/// Construction is private. Only a context already rooting the same frame can
/// resolve it. Source-facing Actor handles are always NativeBatLease instead.
/// This type is nameable solely for default-supplier/attribute binding types.
pub struct NativeBatBackref<T: NativeBatTypes>(Weak<NativeBatFrame<T>>);
impl<T: NativeBatTypes> Clone for NativeBatBackref<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub type BatDefaultAttributeSupplier<T> = NativeAttributeSupplier<
    <T as NativeBatTypes>::AttributeHolder,
    <T as NativeBatTypes>::InstanceIdentity,
    NativeBatBackref<T>,
>;
type BatAttributeInstance<T> = NativeAttributeInstance<
    <T as NativeBatTypes>::AttributeHolder,
    <T as NativeBatTypes>::InstanceIdentity,
    NativeBatBackref<T>,
>;
type BatAttributeMap<T> = NativeAttributeMap<
    <T as NativeBatTypes>::AttributeHolder,
    <T as NativeBatTypes>::InstanceIdentity,
    NativeBatBackref<T>,
>;
type BatEffectMap<T> = ActiveEffectMap<
    <T as NativeBatTypes>::EffectHolder,
    Arc<Mutex<EffectInstance<<T as NativeBatTypes>::EffectHolder>>>,
>;

// These domains have no object in the constructor/ordinary empty-goal Bat
// milestone. A non-null path/goal/context needs its actual algorithm owner;
// no placeholder object or invented isDone value can inhabit these enums.
#[derive(Clone)]
pub(crate) enum UnboundBatPath {}
pub(crate) enum UnboundBatGoal {}
pub(crate) enum UnboundBatNode {}
pub(crate) enum UnboundBatPathContext {}
type BatEvaluator<T> = Arc<
    Mutex<
        WalkNodeEvaluatorState<
            NativeBatBackref<T>,
            UnboundBatPathContext,
            UnboundBatNode,
            PathType,
            Aabb,
        >,
    >,
>;
type BatPathFinder<T> = Arc<Mutex<PathFinderState<BatEvaluator<T>, UnboundBatNode>>>;
type BatNavigation<T> = Arc<
    Mutex<
        GroundNavigationState<
            NativeBatBackref<T>,
            <T as NativeBatTypes>::Level,
            UnboundBatPath,
            BatEvaluator<T>,
            BatPathFinder<T>,
        >,
    >,
>;
type BatSelector<T> =
    Arc<Mutex<GoalSelectorState<<T as NativeBatTypes>::ProfilerSupplier, UnboundBatGoal>>>;
type BatControl<T, S> = Arc<Mutex<RetainedMobControl<NativeBatBackref<T>, S>>>;
type BatSensing<T> = Arc<Mutex<SensingState<NativeBatBackref<T>>>>;

pub(crate) struct BatObjects<T: NativeBatTypes>(std::marker::PhantomData<T>);
impl<T: NativeBatTypes> LivingObjects for BatObjects<T> {
    type Actor = NativeBatLease<T>;
    type ItemStack = T::ItemStack;
    type BlockPos = Arc<(i32, i32, i32)>;
    type DamageSource = T::DamageSource;
    type FallLocation = T::FallLocation;
    type AttributeHolder = T::AttributeHolder;
    type AttributeInstance = BatAttributeInstance<T>;
    type AttributeSupplier = BatDefaultAttributeSupplier<T>;
    type ActiveEffectsMap = BatEffectMap<T>;
    type Brain = DefaultLivingBrain;
    type LocationEnchantmentsMap = LocationEnchantments<T::EnchantmentValue, T::LocationEffect>;
}
impl<T: NativeBatTypes> MobObjects for BatObjects<T> {
    type GoalSelector = BatSelector<T>;
    type LookControl = BatControl<T, LookControlState>;
    type MoveControl = BatControl<T, MoveControlState>;
    type JumpControl = BatControl<T, JumpControlState>;
    type BodyRotationControl = BatControl<T, BodyRotationState>;
    type Navigation = BatNavigation<T>;
    type Sensing = BatSensing<T>;
    type LootTable = T::LootTable;
    type LeashData = T::LeashData;
}

/// Entity fields not represented by EntityPhysicalState. xo/yo/zo are distinct
/// from physical.old_position (xOld/yOld/zOld). Packet codec base is Entity's
/// own VecDeltaCodec, not a ServerEntity tracking position or client movement.
/// The embedding Entity constructor supplies the actual prefix values here.
pub struct BatEntityFields {
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    pub packet_position_base: Vec3,
    pub chunk_position: (i32, i32),
    pub blocks_building: bool,
}
impl BatEntityFields {
    pub fn jvm_zeroed() -> Self {
        Self {
            xo: 0.0,
            yo: 0.0,
            zo: 0.0,
            packet_position_base: Vec3::ZERO,
            chunk_position: (0, 0),
            blocks_building: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConstructionPhase {
    EntityPending,
    EntityComplete,
    Constructing,
    Complete,
}

pub(crate) struct NativeBatFields<T: NativeBatTypes> {
    pub physical: EntityPhysicalState,
    pub entity: BatEntityFields,
    pub living: LivingEntityState<BatObjects<T>, InternalLivingTracker<NativeBatBackref<T>>>,
    pub mob: MobState<BatObjects<T>>,
    pub metadata: Option<BatEntityData>,
    pub bat: Option<BatState>,
    pub entity_random: LegacyRandomSource,
    pub level: T::Level,
    phase: ConstructionPhase,
}
struct NativeBatFrame<T: NativeBatTypes> {
    entity_type: T::EntityType,
    constructor_level: T::Level,
    fields: Mutex<NativeBatFields<T>>,
}

/// Genuine strong root of the ENTIRE original object graph. Numeric IDs are
/// values only. Removing/replacing an index entry cannot retarget this handle.
/// Internal backlinks never own this root; every escaped component pairs its
/// same actual object handle with this lease. No self Arc cycle is introduced.
pub struct NativeBatLease<T: NativeBatTypes>(Arc<NativeBatFrame<T>>);
impl<T: NativeBatTypes> Clone for NativeBatLease<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<T: NativeBatTypes> NativeBatLease<T> {
    /// Attach Java-zero Living/Mob/Bat fields to the actual Entity prefix BEFORE
    /// its virtual metadata callback. All supplied physical/history/RNG values
    /// transfer ownership; none is cloned, reseeded or independently initialized.
    /// This is deliberately NOT permission to publish or tick an Entity.
    pub fn attach_to_entity_prefix(
        physical: EntityPhysicalState,
        entity: BatEntityFields,
        entity_random: LegacyRandomSource,
        entity_type: T::EntityType,
        level: T::Level,
    ) -> Self {
        Self(Arc::new(NativeBatFrame {
            entity_type,
            constructor_level: level.clone(),
            fields: Mutex::new(NativeBatFields {
                physical,
                entity,
                living: LivingEntityState::jvm_zeroed(),
                mob: MobState::jvm_zeroed(),
                metadata: None,
                bat: None,
                entity_random,
                level,
                phase: ConstructionPhase::EntityPending,
            }),
        }))
    }
    pub fn same_actor(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub(crate) fn backref(&self) -> NativeBatBackref<T> {
        NativeBatBackref(Arc::downgrade(&self.0))
    }
    pub(crate) fn with_fields<R>(
        &self,
        access: impl FnOnce(&mut NativeBatFields<T>) -> R,
    ) -> Result<R, BatActorError> {
        let mut fields = self
            .0
            .fields
            .lock()
            .map_err(|_| BatActorError::PoisonedState)?;
        Ok(access(&mut fields))
    }
    /// Only callback-free raw fragments are allowed under this lock.
    pub fn with_physical<R>(
        &self,
        access: impl FnOnce(&mut EntityPhysicalState) -> R,
    ) -> Result<R, BatActorError> {
        self.with_fields(|fields| access(&mut fields.physical))
    }
    pub fn set_position_raw(&self, position: Vec3) -> Result<bool, BatActorError> {
        self.with_fields(|fields| {
            let old_block = fields.physical.block_position;
            let changed = fields.physical.set_position_raw(position);
            if fields.physical.block_position != old_block {
                let at = fields.physical.block_position;
                fields.entity.chunk_position = (at.0 >> 4, at.2 >> 4);
            }
            changed
        })
    }
    /// Actual Entity constructor's built SynchedEntityData assignment point,
    /// following the inspected Entity→Living→Mob→Bat definitions0..16.
    pub fn install_defined_synched_data(&self) -> Result<(), BatActorError> {
        self.with_fields(|fields| {
            if fields.metadata.is_some() || fields.phase != ConstructionPhase::EntityPending {
                return Err(BatActorError::ConstructorAlreadyStarted);
            }
            fields.metadata = Some(BatEntityData::defined_bat_defaults());
            Ok(())
        })?
    }
    /// Explicit embedding assertion: full original Entity constructor completed
    /// ON THIS SAME OBJECT, including UUID draws, metadata and initial setPos.
    /// EntityPhysicalState::new alone does not meet this precondition.
    pub fn finish_entity_constructor(&self) -> Result<(), BatActorError> {
        self.with_fields(|fields| {
            if fields.phase != ConstructionPhase::EntityPending {
                return Err(BatActorError::ConstructorAlreadyStarted);
            }
            if fields.metadata.is_none() {
                return Err(BatActorError::Unbound("SynchedEntityData"));
            }
            fields.phase = ConstructionPhase::EntityComplete;
            Ok(())
        })?
    }
    pub fn is_constructor_complete(&self) -> Result<bool, BatActorError> {
        self.with_fields(|fields| fields.phase == ConstructionPhase::Complete)
    }
    /// Current Entity.level reference. Constructor dispatch separately retains
    /// its original Level argument even if this runtime field changes.
    pub fn level(&self) -> Result<T::Level, BatActorError> {
        self.with_fields(|fields| fields.level.clone())
    }
    pub fn metadata_lease(&self) -> NativeBatMetadataLease<T> {
        NativeBatMetadataLease {
            actor: self.clone(),
        }
    }
}

pub struct NativeBatMetadataLease<T: NativeBatTypes> {
    actor: NativeBatLease<T>,
}
impl<T: NativeBatTypes> Clone for NativeBatMetadataLease<T> {
    fn clone(&self) -> Self {
        Self {
            actor: self.actor.clone(),
        }
    }
}
impl<T: NativeBatTypes> NativeBatMetadataLease<T> {
    pub fn actor(&self) -> &NativeBatLease<T> {
        &self.actor
    }
}

/// Genuine external services only: shared canonical registries/hash authority,
/// ItemStack.EMPTY, global Math.random, retained Level operations and position/
/// dimension callbacks. Active-effect/attribute/location-enchantment maps,
/// Brain, selectors, controls, navigation,
/// sensing, metadata and constructor ordering are assembled below, not supplied
/// as successful inherited-constructor callbacks.
pub trait NativeBatServices<T: NativeBatTypes> {
    type Error: fmt::Debug;
    fn canonical_empty_stack(&mut self) -> Result<T::ItemStack, Self::Error>;
    fn canonical_zero_block_position(&mut self) -> Result<Arc<(i32, i32, i32)>, Self::Error>;
    fn default_attribute_supplier(
        &mut self,
        entity_type: &T::EntityType,
    ) -> Result<BatDefaultAttributeSupplier<T>, Self::Error>;
    fn max_health_holder(&mut self) -> Result<T::AttributeHolder, Self::Error>;
    fn follow_range_holder(&mut self) -> Result<T::AttributeHolder, Self::Error>;
    fn holder_hash_code(&mut self, holder: &T::AttributeHolder) -> Result<i32, Self::Error>;
    fn registered_attribute_name(
        &mut self,
        holder: &T::AttributeHolder,
    ) -> Result<String, Self::Error>;
    fn attribute_definition(
        &mut self,
        holder: &T::AttributeHolder,
    ) -> Result<Arc<BoundAttribute<T::AttributeHolder>>, Self::Error>;
    fn new_instance_identity(&mut self) -> Result<T::InstanceIdentity, Self::Error>;
    fn next_global_math_double(&mut self) -> Result<f64, Self::Error>;
    fn level_profiler_supplier(
        &mut self,
        level: &T::Level,
    ) -> Result<T::ProfilerSupplier, Self::Error>;
    fn level_is_client_side(&mut self, level: &T::Level) -> Result<bool, Self::Error>;
    fn position_changed(&mut self, actor: &NativeBatLease<T>) -> Result<(), Self::Error>;
    fn refresh_dimensions(&mut self, actor: &NativeBatLease<T>) -> Result<(), Self::Error>;
    fn on_attribute_updated(
        &mut self,
        actor: &NativeBatLease<T>,
        holder: &T::AttributeHolder,
    ) -> Result<(), Self::Error>;
}

/// One invocation owns a strong root before any callback. Services receive the
/// exact captured lease, including synchronous removal and numeric-ID reuse.
/// This type has no arbitrary inherited_tick callback or success implementation.
pub(crate) struct NativeBatContext<'w, T: NativeBatTypes, H: NativeBatServices<T>> {
    actor: NativeBatLease<T>,
    services: &'w mut H,
}
impl<'w, T: NativeBatTypes, H: NativeBatServices<T>> NativeBatContext<'w, T, H> {
    pub fn new(actor: NativeBatLease<T>, services: &'w mut H) -> Self {
        Self { actor, services }
    }
    fn require_actor(&self, actor: &NativeBatLease<T>) -> Result<(), BatActorError> {
        if self.actor.same_actor(actor) {
            Ok(())
        } else {
            Err(BatActorError::ForeignActor)
        }
    }
    fn require_backref(&self, actor: &NativeBatBackref<T>) -> Result<(), BatActorError> {
        if Weak::ptr_eq(&actor.0, &Arc::downgrade(&self.actor.0)) {
            Ok(())
        } else {
            Err(BatActorError::ForeignActor)
        }
    }
}

impl<T: NativeBatTypes, H: NativeBatServices<T>> BatMetadataHost for NativeBatContext<'_, T, H> {
    type Data = NativeBatMetadataLease<T>;
    type Error = BatConstructionError<H::Error>;
    fn with_metadata<R>(
        &mut self,
        data: &Self::Data,
        access: impl FnOnce(&mut BatEntityData, &mut EntityPhysicalState) -> R,
    ) -> Result<R, Self::Error> {
        self.require_actor(&data.actor)?;
        self.actor
            .with_fields(|fields| {
                let data = fields
                    .metadata
                    .as_mut()
                    .ok_or(BatActorError::Unbound("SynchedEntityData"))?;
                Ok::<R, BatActorError>(access(data, &mut fields.physical))
            })?
            .map_err(Into::into)
    }
    fn refresh_dimensions(&mut self, data: &Self::Data) -> Result<(), Self::Error> {
        self.require_actor(&data.actor)?;
        self.services
            .refresh_dimensions(&data.actor)
            .map_err(BatConstructionError::Services)
    }
}

impl<T: NativeBatTypes, H: NativeBatServices<T>> AttributeBindingOwner
    for NativeBatContext<'_, T, H>
{
    type Error = BatConstructionError<H::Error>;
    type Holder = T::AttributeHolder;
    type Identity = T::InstanceIdentity;
    type Map = NativeBatBackref<T>;
    type Actor = NativeBatLease<T>;
    fn holder_hash_code(&mut self, holder: &Self::Holder) -> Result<i32, Self::Error> {
        self.services
            .holder_hash_code(holder)
            .map_err(BatConstructionError::Services)
    }
    fn registered_name(&mut self, holder: &Self::Holder) -> Result<String, Self::Error> {
        self.services
            .registered_attribute_name(holder)
            .map_err(BatConstructionError::Services)
    }
    fn definition(
        &mut self,
        holder: &Self::Holder,
    ) -> Result<Arc<BoundAttribute<Self::Holder>>, Self::Error> {
        self.services
            .attribute_definition(holder)
            .map_err(BatConstructionError::Services)
    }
    fn new_instance_identity(&mut self) -> Result<Self::Identity, Self::Error> {
        self.services
            .new_instance_identity()
            .map_err(BatConstructionError::Services)
    }
    fn with_map<R>(
        &mut self,
        map: &Self::Map,
        access: impl FnOnce(&mut BatAttributeMap<T>) -> R,
    ) -> Result<R, Self::Error> {
        self.require_backref(map)?;
        self.actor
            .with_fields(|fields| {
                fields
                    .living
                    .attributes
                    .as_mut()
                    .map(access)
                    .ok_or(BatActorError::Unbound("LivingEntity.attributes"))
            })?
            .map_err(Into::into)
    }
    fn on_attribute_updated(
        &mut self,
        actor: &Self::Actor,
        holder: &Self::Holder,
    ) -> Result<(), Self::Error> {
        self.require_actor(actor)?;
        self.services
            .on_attribute_updated(actor, holder)
            .map_err(BatConstructionError::Services)
    }
}

fn java_min(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f32::from_bits(a.to_bits() | b.to_bits())
    } else if a <= b {
        a
    } else {
        b
    }
}
fn metadata_error<E>(error: MetadataError<BatConstructionError<E>>) -> BatConstructionError<E> {
    match error {
        MetadataError::Host(error) => error,
        MetadataError::Field(error) => BatConstructionError::Metadata(error),
    }
}

impl<T: NativeBatTypes, H: NativeBatServices<T>> NativeBatContext<'_, T, H> {
    fn attribute_value(
        &mut self,
        actor: &NativeBatLease<T>,
        holder: &T::AttributeHolder,
    ) -> Result<f64, BatConstructionError<H::Error>> {
        self.require_actor(actor)?;
        let map = actor.backref();
        let mut binding = NativeAttributeBinding::for_map(self, map.clone());
        attribute_map::value(&map, holder, &mut binding)
            .map_err(|error| source_error("AttributeMap.value", error))
    }
    fn set_health(
        &mut self,
        actor: &NativeBatLease<T>,
        health: f32,
    ) -> Result<(), BatConstructionError<H::Error>> {
        self.require_actor(actor)?;
        // Source captures its actual SynchedEntityData receiver BEFORE the
        // fresh current max-health lookup inside this setter.
        let data = actor.metadata_lease();
        let holder = self
            .services
            .max_health_holder()
            .map_err(BatConstructionError::Services)?;
        let maximum = self.attribute_value(actor, &holder)? as f32;
        let health = if health < 0.0 {
            0.0
        } else {
            java_min(health, maximum)
        };
        metadata::set(
            &data,
            metadata::DATA_HEALTH_ID,
            MetadataValue::Float(health),
            false,
            self,
        )
        .map_err(metadata_error)?;
        Ok(())
    }
    fn construct_after_entity(&mut self) -> Result<(), BatConstructionError<H::Error>> {
        let actor = self.actor.clone();
        actor.with_fields(|fields| {
            if fields.phase != ConstructionPhase::EntityComplete {
                return Err(if fields.phase == ConstructionPhase::EntityPending {
                    BatActorError::ConstructorNotComplete
                } else {
                    BatActorError::ConstructorAlreadyStarted
                });
            }
            fields.phase = ConstructionPhase::Constructing;
            Ok(())
        })??;
        let level = actor.0.constructor_level.clone();
        let entity_type = actor.0.entity_type.clone();
        living_state::construct_living_after_entity_with_tracker::<
            InternalLivingTracker<NativeBatBackref<T>>,
            _,
        >(&actor, &entity_type, actor.backref(), self)?;
        construction::construct_mob_after_living(&actor, &level, self)
            .map_err(|error| source_error("Mob constructor", error))?;
        // AmbientCreature's complete constructor is only its Mob parent.
        // Both final animation objects are attached BEFORE server setResting.
        actor.with_fields(|fields| fields.bat = Some(BatState::initialized_fields()))?;
        if !self
            .services
            .level_is_client_side(&level)
            .map_err(BatConstructionError::Services)?
        {
            let data = actor.metadata_lease();
            let flags =
                metadata::get(&data, metadata::DATA_BAT_FLAGS_ID, self).map_err(metadata_error)?;
            let MetadataValue::Byte(flags) = flags else {
                unreachable!("Bat byte accessor")
            };
            metadata::set(
                &data,
                metadata::DATA_BAT_FLAGS_ID,
                MetadataValue::Byte(flags | 1),
                false,
                self,
            )
            .map_err(metadata_error)?;
        }
        actor.with_fields(|fields| fields.phase = ConstructionPhase::Complete)?;
        Ok(())
    }
}

impl<T: NativeBatTypes, H: NativeBatServices<T>>
    LivingConstructionFields<InternalLivingTracker<NativeBatBackref<T>>>
    for NativeBatContext<'_, T, H>
{
    type Error = BatConstructionError<H::Error>;
    type Actor = NativeBatLease<T>;
    type Objects = BatObjects<T>;
    type EntityType = T::EntityType;
    fn with_living_state<R>(
        &mut self,
        actor: &Self::Actor,
        access: impl FnOnce(
            &mut LivingEntityState<Self::Objects, InternalLivingTracker<NativeBatBackref<T>>>,
        ) -> R,
    ) -> Result<R, Self::Error> {
        self.require_actor(actor)?;
        actor
            .with_fields(|fields| access(&mut fields.living))
            .map_err(Into::into)
    }
    fn new_active_effects_map(&mut self) -> Result<BatEffectMap<T>, Self::Error> {
        Ok(ActiveEffectMap::new())
    }
    fn canonical_empty_stack(&mut self) -> Result<T::ItemStack, Self::Error> {
        self.services
            .canonical_empty_stack()
            .map_err(BatConstructionError::Services)
    }
    fn new_location_enchantments_map(
        &mut self,
    ) -> Result<LocationEnchantments<T::EnchantmentValue, T::LocationEffect>, Self::Error> {
        Ok(LocationEnchantments::new())
    }
    fn default_attribute_supplier(
        &mut self,
        entity_type: &T::EntityType,
    ) -> Result<BatDefaultAttributeSupplier<T>, Self::Error> {
        self.services
            .default_attribute_supplier(entity_type)
            .map_err(BatConstructionError::Services)
    }
    fn max_health(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error> {
        let holder = self
            .services
            .max_health_holder()
            .map_err(BatConstructionError::Services)?;
        Ok(self.attribute_value(actor, &holder)? as f32)
    }
    fn set_health(&mut self, actor: &Self::Actor, value: f32) -> Result<(), Self::Error> {
        NativeBatContext::set_health(self, actor, value)
    }
    fn set_blocks_building(&mut self, actor: &Self::Actor, value: bool) -> Result<(), Self::Error> {
        self.require_actor(actor)?;
        actor
            .with_fields(|fields| fields.entity.blocks_building = value)
            .map_err(Into::into)
    }
    fn next_global_math_double(&mut self) -> Result<f64, Self::Error> {
        self.services
            .next_global_math_double()
            .map_err(BatConstructionError::Services)
    }
    fn reapply_position(&mut self, actor: &Self::Actor) -> Result<(), Self::Error> {
        self.require_actor(actor)?;
        crate::living_entities::retained_move::reapply_position(actor, |retained| {
            self.services.position_changed(retained).map_err(|error| {
                crate::living_entities::retained_move::BatPhysicalError::World(anyhow::anyhow!(
                    "{error:?}"
                ))
            })
        })
        .map_err(|error| source_error("Entity.reapplyPosition", error))
    }
    fn set_yaw(&mut self, actor: &Self::Actor, value: f32) -> Result<(), Self::Error> {
        self.require_actor(actor)?;
        if !value.is_finite() {
            return Err(BatActorError::Unbound("Entity non-finite rotation logger").into());
        }
        actor
            .with_physical(|physical| physical.position.yaw = value)
            .map_err(Into::into)
    }
    fn yaw(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error> {
        self.require_actor(actor)?;
        actor
            .with_physical(|physical| physical.position.yaw)
            .map_err(Into::into)
    }
    fn make_brain_from_empty_memories(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<DefaultLivingBrain, Self::Error> {
        self.require_actor(actor)?;
        Ok(DefaultBrainProvider::new().make_from_constructor_memories())
    }
}

impl<T: NativeBatTypes, H: NativeBatServices<T>> MobConstructionHost
    for NativeBatContext<'_, T, H>
{
    type Error = BatConstructionError<H::Error>;
    type Objects = BatObjects<T>;
    type Level = T::Level;
    type ProfilerSupplier = T::ProfilerSupplier;
    type WrappedGoal = UnboundBatGoal;
    fn with_mob_state<R>(
        &mut self,
        actor: &NativeBatLease<T>,
        access: impl FnOnce(&mut MobState<BatObjects<T>>) -> R,
    ) -> Result<R, Self::Error> {
        self.require_actor(actor)?;
        actor
            .with_fields(|fields| access(&mut fields.mob))
            .map_err(Into::into)
    }
    fn canonical_empty_stack(&mut self) -> Result<T::ItemStack, Self::Error> {
        self.services
            .canonical_empty_stack()
            .map_err(BatConstructionError::Services)
    }
    fn canonical_zero_block_position(&mut self) -> Result<Arc<(i32, i32, i32)>, Self::Error> {
        self.services
            .canonical_zero_block_position()
            .map_err(BatConstructionError::Services)
    }
    fn level_profiler_supplier(
        &mut self,
        level: &Self::Level,
    ) -> Result<Self::ProfilerSupplier, Self::Error> {
        self.services
            .level_profiler_supplier(level)
            .map_err(BatConstructionError::Services)
    }
    fn allocate_goal_selector(
        &mut self,
        state: GoalSelectorState<T::ProfilerSupplier, UnboundBatGoal>,
    ) -> Result<BatSelector<T>, Self::Error> {
        Ok(Arc::new(Mutex::new(state)))
    }
    fn allocate_look_control(
        &mut self,
        object: RetainedMobControl<NativeBatLease<T>, LookControlState>,
    ) -> Result<BatControl<T, LookControlState>, Self::Error> {
        self.require_actor(&object.mob)?;
        Ok(Arc::new(Mutex::new(RetainedMobControl {
            mob: object.mob.backref(),
            state: object.state,
        })))
    }
    fn allocate_move_control(
        &mut self,
        object: RetainedMobControl<NativeBatLease<T>, MoveControlState>,
    ) -> Result<BatControl<T, MoveControlState>, Self::Error> {
        self.require_actor(&object.mob)?;
        Ok(Arc::new(Mutex::new(RetainedMobControl {
            mob: object.mob.backref(),
            state: object.state,
        })))
    }
    fn allocate_jump_control(
        &mut self,
        object: RetainedMobControl<NativeBatLease<T>, JumpControlState>,
    ) -> Result<BatControl<T, JumpControlState>, Self::Error> {
        self.require_actor(&object.mob)?;
        Ok(Arc::new(Mutex::new(RetainedMobControl {
            mob: object.mob.backref(),
            state: object.state,
        })))
    }
    fn create_body_control(
        &mut self,
        actor: &NativeBatLease<T>,
    ) -> Result<BatControl<T, BodyRotationState>, Self::Error> {
        self.require_actor(actor)?;
        Ok(Arc::new(Mutex::new(RetainedMobControl {
            mob: actor.backref(),
            state: BodyRotationState::new(),
        })))
    }
    fn create_navigation(
        &mut self,
        actor: &NativeBatLease<T>,
        level: &Self::Level,
    ) -> Result<BatNavigation<T>, Self::Error> {
        self.require_actor(actor)?;
        navigation::construct_ground(&actor.backref(), level, self)
            .map_err(|error| source_error("GroundPathNavigation constructor", error))
    }
    fn allocate_sensing(
        &mut self,
        state: SensingState<NativeBatLease<T>>,
    ) -> Result<BatSensing<T>, Self::Error> {
        Ok(Arc::new(Mutex::new(state.map_mob(|actor| actor.backref()))))
    }
    fn level_is_client_side(&mut self, level: &Self::Level) -> Result<bool, Self::Error> {
        self.services
            .level_is_client_side(level)
            .map_err(BatConstructionError::Services)
    }
    fn register_goals(&mut self, actor: &NativeBatLease<T>) -> Result<(), Self::Error> {
        self.require_actor(actor)?;
        // Complete inherited Mob.registerGoals169 is literally empty for Bat.
        Ok(())
    }
}

impl<T: NativeBatTypes, H: NativeBatServices<T>> NavigationHost for NativeBatContext<'_, T, H> {
    type Error = BatConstructionError<H::Error>;
    type Lease = BatNavigation<T>;
    type Mob = NativeBatBackref<T>;
    type Level = T::Level;
    type Path = UnboundBatPath;
    type Evaluator = BatEvaluator<T>;
    type PathFinder = BatPathFinder<T>;
    fn with_navigation<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(
            &mut GroundNavigationState<
                Self::Mob,
                Self::Level,
                Self::Path,
                Self::Evaluator,
                Self::PathFinder,
            >,
        ) -> R,
    ) -> Result<R, Self::Error> {
        let mut state = retained.lock().map_err(|_| BatActorError::PoisonedState)?;
        self.require_backref(state.mob())?;
        Ok(access(&mut state))
    }
    fn path_is_done(&mut self, path: &Self::Path) -> Result<bool, Self::Error> {
        match *path {}
    }
}
impl<T: NativeBatTypes, H: NativeBatServices<T>> GroundConstructionHost
    for NativeBatContext<'_, T, H>
{
    type Context = UnboundBatPathContext;
    type Node = UnboundBatNode;
    type PathType = PathType;
    type CollisionBounds = Aabb;
    fn allocate_ground_navigation(
        &mut self,
        state: GroundNavigationState<
            Self::Mob,
            Self::Level,
            Self::Path,
            Self::Evaluator,
            Self::PathFinder,
        >,
    ) -> Result<Self::Lease, Self::Error> {
        self.require_backref(state.mob())?;
        Ok(Arc::new(Mutex::new(state)))
    }
    fn follow_range_attribute(&mut self, mob: &Self::Mob) -> Result<f64, Self::Error> {
        self.require_backref(mob)?;
        let actor = self.actor.clone();
        let holder = self
            .services
            .follow_range_holder()
            .map_err(BatConstructionError::Services)?;
        self.attribute_value(&actor, &holder)
    }
    fn create_path_finder(
        &mut self,
        retained: &Self::Lease,
        max_nodes: i32,
    ) -> Result<Self::PathFinder, NavigationError<Self::Error>> {
        navigation::create_ground_path_finder(retained, max_nodes, self)
    }
    fn allocate_walk_evaluator(
        &mut self,
        state: WalkNodeEvaluatorState<
            Self::Mob,
            Self::Context,
            Self::Node,
            Self::PathType,
            Self::CollisionBounds,
        >,
    ) -> Result<Self::Evaluator, Self::Error> {
        Ok(Arc::new(Mutex::new(state)))
    }
    fn evaluator_set_can_pass_doors(
        &mut self,
        evaluator: &Self::Evaluator,
        value: bool,
    ) -> Result<(), Self::Error> {
        evaluator
            .lock()
            .map_err(|_| BatActorError::PoisonedState)?
            .base_mut()
            .set_can_pass_doors(value);
        Ok(())
    }
    fn allocate_path_finder(
        &mut self,
        state: PathFinderState<Self::Evaluator, Self::Node>,
    ) -> Result<Self::PathFinder, Self::Error> {
        Ok(Arc::new(Mutex::new(state)))
    }
}

/// Run only the source Living→Mob→AmbientCreature→Bat constructor tail. The
/// explicit completed-Entity precondition above must already hold. A failure
/// retains assignments/RNG/callback prefixes, never retries or admits the actor.
pub fn construct_after_entity<T: NativeBatTypes, H: NativeBatServices<T>>(
    actor: &NativeBatLease<T>,
    services: &mut H,
) -> Result<(), BatConstructionError<H::Error>> {
    NativeBatContext::new(actor.clone(), services).construct_after_entity()
}

pub fn get_synched_data<T: NativeBatTypes, H: NativeBatServices<T>>(
    data: &NativeBatMetadataLease<T>,
    id: u8,
    services: &mut H,
) -> Result<MetadataValue, BatConstructionError<H::Error>> {
    metadata::get(
        data,
        id,
        &mut NativeBatContext::new(data.actor.clone(), services),
    )
    .map_err(metadata_error)
}
pub fn set_synched_data<T: NativeBatTypes, H: NativeBatServices<T>>(
    data: &NativeBatMetadataLease<T>,
    id: u8,
    value: MetadataValue,
    force: bool,
    services: &mut H,
) -> Result<bool, BatConstructionError<H::Error>> {
    metadata::set(
        data,
        id,
        value,
        force,
        &mut NativeBatContext::new(data.actor.clone(), services),
    )
    .map_err(metadata_error)
}

/// Escaped control objects retain both the exact control AND its original Mob,
/// even if a callback replaces the actor's current controller field.
pub struct NativeBatControlLease<T: NativeBatTypes, S> {
    actor: NativeBatLease<T>,
    object: BatControl<T, S>,
}
impl<T: NativeBatTypes, S> Clone for NativeBatControlLease<T, S> {
    fn clone(&self) -> Self {
        Self {
            actor: self.actor.clone(),
            object: Arc::clone(&self.object),
        }
    }
}
impl<T: NativeBatTypes, S> NativeBatControlLease<T, S> {
    pub fn actor(&self) -> &NativeBatLease<T> {
        &self.actor
    }
    /// Raw control-field fragments only. Never invoke game callbacks here.
    pub fn with_state<R>(&self, access: impl FnOnce(&mut S) -> R) -> Result<R, BatActorError> {
        let mut object = self
            .object
            .lock()
            .map_err(|_| BatActorError::PoisonedState)?;
        Ok(access(&mut object.state))
    }
    pub fn same_control(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.object, &other.object)
    }
}

pub struct NativeBatSensingLease<T: NativeBatTypes> {
    actor: NativeBatLease<T>,
    object: BatSensing<T>,
}
impl<T: NativeBatTypes> Clone for NativeBatSensingLease<T> {
    fn clone(&self) -> Self {
        Self {
            actor: self.actor.clone(),
            object: Arc::clone(&self.object),
        }
    }
}
impl<T: NativeBatTypes> NativeBatSensingLease<T> {
    pub fn actor(&self) -> &NativeBatLease<T> {
        &self.actor
    }
    pub fn same_sensing(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.object, &other.object)
    }
}

pub struct NativeBatNavigationLease<T: NativeBatTypes> {
    actor: NativeBatLease<T>,
    object: BatNavigation<T>,
}
impl<T: NativeBatTypes> Clone for NativeBatNavigationLease<T> {
    fn clone(&self) -> Self {
        Self {
            actor: self.actor.clone(),
            object: Arc::clone(&self.object),
        }
    }
}
impl<T: NativeBatTypes> NativeBatNavigationLease<T> {
    pub fn actor(&self) -> &NativeBatLease<T> {
        &self.actor
    }
    pub fn constructor_layout(
        &self,
    ) -> Result<navigation::NavigationConstructorLayout, BatActorError> {
        Ok(self
            .object
            .lock()
            .map_err(|_| BatActorError::PoisonedState)?
            .constructor_layout())
    }
}

/// The original final CombatTracker remains inline in this graph; it cannot be
/// replaced through this API. The escaped tracker lease roots that SAME graph.
pub struct NativeBatTrackerLease<T: NativeBatTypes> {
    actor: NativeBatLease<T>,
}
impl<T: NativeBatTypes> Clone for NativeBatTrackerLease<T> {
    fn clone(&self) -> Self {
        Self {
            actor: self.actor.clone(),
        }
    }
}
impl<T: NativeBatTypes> NativeBatTrackerLease<T> {
    pub fn actor(&self) -> &NativeBatLease<T> {
        &self.actor
    }
    /// Callback-free field observations/mutations, never a tracker callback.
    pub fn with_state<R>(
        &self,
        access: impl FnOnce(
            &mut crate::combat::tracker::CombatTracker<T::DamageSource, T::FallLocation>,
        ) -> R,
    ) -> Result<R, BatActorError> {
        self.actor.with_fields(|fields| {
            fields
                .living
                .combat_tracker
                .as_mut()
                .map(|tracker| access(&mut tracker.state))
                .ok_or(BatActorError::Unbound("CombatTracker"))
        })?
    }
}

impl<T: NativeBatTypes> NativeBatLease<T> {
    pub fn look_control_lease(
        &self,
    ) -> Result<NativeBatControlLease<T, LookControlState>, BatActorError> {
        let object = self
            .with_fields(|fields| fields.mob.look_control.clone())?
            .ok_or(BatActorError::Unbound("LookControl"))?;
        Ok(NativeBatControlLease {
            actor: self.clone(),
            object,
        })
    }
    pub fn move_control_lease(
        &self,
    ) -> Result<NativeBatControlLease<T, MoveControlState>, BatActorError> {
        let object = self
            .with_fields(|fields| fields.mob.move_control.clone())?
            .ok_or(BatActorError::Unbound("MoveControl"))?;
        Ok(NativeBatControlLease {
            actor: self.clone(),
            object,
        })
    }
    pub fn jump_control_lease(
        &self,
    ) -> Result<NativeBatControlLease<T, JumpControlState>, BatActorError> {
        let object = self
            .with_fields(|fields| fields.mob.jump_control.clone())?
            .ok_or(BatActorError::Unbound("JumpControl"))?;
        Ok(NativeBatControlLease {
            actor: self.clone(),
            object,
        })
    }
    pub fn body_control_lease(
        &self,
    ) -> Result<NativeBatControlLease<T, BodyRotationState>, BatActorError> {
        let object = self
            .with_fields(|fields| fields.mob.body_rotation_control.clone())?
            .ok_or(BatActorError::Unbound("BodyRotationControl"))?;
        Ok(NativeBatControlLease {
            actor: self.clone(),
            object,
        })
    }
    pub fn sensing_lease(&self) -> Result<NativeBatSensingLease<T>, BatActorError> {
        let object = self
            .with_fields(|fields| fields.mob.sensing.clone())?
            .ok_or(BatActorError::Unbound("Sensing"))?;
        Ok(NativeBatSensingLease {
            actor: self.clone(),
            object,
        })
    }
    pub fn navigation_lease(&self) -> Result<NativeBatNavigationLease<T>, BatActorError> {
        let object = self
            .with_fields(|fields| fields.mob.navigation.clone())?
            .ok_or(BatActorError::Unbound("GroundPathNavigation"))?;
        Ok(NativeBatNavigationLease {
            actor: self.clone(),
            object,
        })
    }
    pub fn tracker_lease(&self) -> Result<NativeBatTrackerLease<T>, BatActorError> {
        if !self.with_fields(|fields| fields.living.combat_tracker.is_some())? {
            return Err(BatActorError::Unbound("CombatTracker"));
        }
        Ok(NativeBatTrackerLease {
            actor: self.clone(),
        })
    }
    /// The SAME Entity.random supplied by the actual Entity prefix. Random
    /// fragments must not invoke world/game callbacks while this guard is held.
    pub fn with_entity_random<R>(
        &self,
        access: impl FnOnce(&mut LegacyRandomSource) -> R,
    ) -> Result<R, BatActorError> {
        self.with_fields(|fields| access(&mut fields.entity_random))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::tracker::CombatTracker, entity::Position,
        living_entities::state::LivingCombatTracker,
    };

    // Uninhabited external domains exercise aggregate ownership only. They
    // supply no fabricated registry, attribute, effect or production actor.
    #[derive(Clone, PartialEq, Eq)]
    enum ExternalObject {}
    impl JavaObjectKey for ExternalObject {
        fn java_hash_code(&self) -> i32 {
            match *self {}
        }
    }
    impl RegisteredAttributeHolder for ExternalObject {}
    impl ActiveEffectKey for ExternalObject {
        fn is_canonical_reference(&self) -> bool {
            match *self {}
        }
        fn java_hash_code(&self) -> i32 {
            match *self {}
        }
        fn same_object(&self, _: &Self) -> bool {
            match *self {}
        }
    }
    impl EnchantmentValueReference for ExternalObject {
        fn same_object(&self, _: &Self) -> bool {
            match *self {}
        }
    }
    impl LocationEffectReference for ExternalObject {
        fn same_object(&self, _: &Self) -> bool {
            match *self {}
        }
        fn java_equals(&self, _: &Self) -> bool {
            match *self {}
        }
    }
    struct OwnershipFixture;
    impl NativeBatTypes for OwnershipFixture {
        type ItemStack = Arc<()>;
        type DamageSource = ExternalObject;
        type FallLocation = ExternalObject;
        type AttributeHolder = ExternalObject;
        type InstanceIdentity = ExternalObject;
        type EffectHolder = ExternalObject;
        type EnchantmentValue = ExternalObject;
        type LocationEffect = ExternalObject;
        type EntityType = Arc<()>;
        type Level = Arc<()>;
        type ProfilerSupplier = Arc<()>;
        type LootTable = ExternalObject;
        type LeashData = ExternalObject;
    }
    fn actor(id: i32, uuid_byte: u8) -> NativeBatLease<OwnershipFixture> {
        // Explicitly a controlled Entity-prefix value, not a source Entity
        // constructor or admitted world object. No constructor completion claim.
        let physical = EntityPhysicalState::new(
            id,
            [uuid_byte; 16],
            Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: false,
            },
            Vec3::ZERO,
            0.5,
            0.9,
            0.45,
        )
        .unwrap();
        NativeBatLease::attach_to_entity_prefix(
            physical,
            BatEntityFields::jvm_zeroed(),
            LegacyRandomSource::new(7),
            Arc::new(()),
            Arc::new(()),
        )
    }
    fn attach_internal_objects(actor: &NativeBatLease<OwnershipFixture>) {
        let backref = actor.backref();
        actor
            .with_fields(|fields| {
                fields.mob.look_control = Some(Arc::new(Mutex::new(RetainedMobControl {
                    mob: backref.clone(),
                    state: LookControlState::new(),
                })));
                fields.mob.sensing = Some(Arc::new(Mutex::new(SensingState::new(backref.clone()))));
                fields.living.combat_tracker = Some(LivingCombatTracker {
                    actor: backref,
                    state: CombatTracker::new(),
                });
            })
            .unwrap();
    }

    #[test]
    fn escaped_components_retain_original_removed_actor_without_self_cycle() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<NativeBatLease<OwnershipFixture>>();
        send_sync::<NativeBatControlLease<OwnershipFixture, LookControlState>>();
        send_sync::<NativeBatSensingLease<OwnershipFixture>>();
        send_sync::<NativeBatTrackerLease<OwnershipFixture>>();
        send_sync::<NativeBatMetadataLease<OwnershipFixture>>();
        let original = actor(41, 1);
        attach_internal_objects(&original);
        let weak = Arc::downgrade(&original.0);
        let control = original.look_control_lease().unwrap();
        let sensing = original.sensing_lease().unwrap();
        let tracker = original.tracker_lease().unwrap();
        let metadata = original.metadata_lease();
        original
            .with_physical(|physical| physical.removed = true)
            .unwrap();
        let replacement = actor(41, 2);
        assert!(!original.same_actor(&replacement));
        drop(original);
        assert_eq!(
            control
                .actor()
                .with_physical(|physical| (physical.entity_id, physical.uuid, physical.removed))
                .unwrap(),
            (41, [1; 16], true)
        );
        assert!(control.actor().same_actor(sensing.actor()));
        assert!(control.actor().same_actor(tracker.actor()));
        assert!(control.actor().same_actor(metadata.actor()));
        drop(control);
        drop(sensing);
        drop(tracker);
        assert!(weak.upgrade().is_some());
        drop(metadata);
        assert!(
            weak.upgrade().is_none(),
            "internal source backedges must not form Arc cycles"
        );
        assert_eq!(
            replacement.with_physical(|physical| physical.uuid).unwrap(),
            [2; 16]
        );
    }

    #[test]
    fn escaped_control_is_not_redirected_when_current_field_is_replaced() {
        let actor = actor(42, 3);
        attach_internal_objects(&actor);
        let old = actor.look_control_lease().unwrap();
        let backref = actor.backref();
        actor
            .with_fields(|fields| {
                fields.mob.look_control = Some(Arc::new(Mutex::new(RetainedMobControl {
                    mob: backref,
                    state: LookControlState::new(),
                })))
            })
            .unwrap();
        let current = actor.look_control_lease().unwrap();
        assert!(!old.same_control(&current));
        assert!(old.actor().same_actor(current.actor()));
        drop(actor);
        old.with_state(|_| ()).unwrap();
    }

    #[test]
    fn entity_precondition_and_data_assignment_are_explicit_and_single_use() {
        let actor = actor(43, 4);
        actor
            .with_fields(|fields| {
                assert!(fields.living.active_effects.is_none());
                assert!(fields.living.combat_tracker.is_none());
                assert!(fields.living.brain.is_none());
                assert!(fields.mob.look_control.is_none());
                assert!(fields.bat.is_none());
                assert!(fields.metadata.is_none());
            })
            .unwrap();
        assert_eq!(
            actor.finish_entity_constructor(),
            Err(BatActorError::Unbound("SynchedEntityData"))
        );
        actor.install_defined_synched_data().unwrap();
        assert_eq!(
            actor.install_defined_synched_data(),
            Err(BatActorError::ConstructorAlreadyStarted)
        );
        actor.finish_entity_constructor().unwrap();
        assert!(!actor.is_constructor_complete().unwrap());
        assert_eq!(
            actor.finish_entity_constructor(),
            Err(BatActorError::ConstructorAlreadyStarted)
        );
    }

    #[test]
    fn raw_position_updates_chunk_cache_only_when_block_position_changes() {
        let actor = actor(44, 5);
        actor
            .with_fields(|fields| fields.entity.chunk_position = (99, 99))
            .unwrap();
        actor.set_position_raw(Vec3::new(0.25, 0.0, 0.25)).unwrap();
        assert_eq!(
            actor
                .with_fields(|fields| fields.entity.chunk_position)
                .unwrap(),
            (99, 99)
        );
        actor.set_position_raw(Vec3::new(16.0, 0.0, -0.1)).unwrap();
        assert_eq!(
            actor
                .with_fields(|fields| fields.entity.chunk_position)
                .unwrap(),
            (1, -1)
        );
    }
}
