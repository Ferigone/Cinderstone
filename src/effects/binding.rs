//! Retained native AttributeSupplier/AttributeMap object bindings.
//!
//! These are actual native instance, builder and supplier objects, not an ID
//! arena or second registry. The embedding owner supplies canonical registered
//! holders, definitions and the SAME shared lazy Java object-identity authority.
//! Its map projection addresses LivingEntityState.attributes in place. No map,
//! actor, health, registry, identity hash or RNG is fabricated here.
//!
//! Target consumers in this bounded binding are the actual AttributeMap method
//! reference. A map descriptor must retain its source lifetime through the real
//! invocation owner; the binding does not choose an actor heap/GC policy.

use super::{
    attribute_instance::AttributeInstance,
    attribute_map::{self, AttributeMapHost, AttributeMapState, JavaObjectKey},
    attribute_persistence::ResourceLocation,
    definitions::BoundAttribute,
    supplier::{
        self, AttributeSupplierHost, AttributeSupplierState, RegisteredAttributeHolder,
        SupplierBuilderState, SupplierError, SupplierInstanceListener, SupplierKeyHost,
    },
};
use std::sync::{Arc, Mutex};

pub type NativeAttributeState<H> = AttributeInstance<BoundAttribute<H>, ResourceLocation>;
pub type NativeAttributeMap<H, O, M> =
    AttributeMapState<H, NativeAttributeInstance<H, O, M>, NativeAttributeSupplier<H, O, M>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingObject {
    Builder,
    Instance,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BindingError<E> {
    Owner(E),
    Poisoned(BindingObject),
    /// AttributeMap allocation requires its exact actual consumer target.
    MissingAllocationMap,
}

struct InstanceObject<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> {
    identity: O,
    state: Mutex<NativeAttributeState<H>>,
    consumer: SupplierInstanceListener<H, M>,
}

/// A strong lease of THIS source AttributeInstance. Map removal, replacement
/// and entity-ID reuse cannot retarget it. Clone retains, rather than copies,
/// the object. Its object hash delegates lazily to the supplied real identity.
pub struct NativeAttributeInstance<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone>(
    Arc<InstanceObject<H, O, M>>,
);
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> Clone
    for NativeAttributeInstance<H, O, M>
{
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> PartialEq
    for NativeAttributeInstance<H, O, M>
{
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> Eq
    for NativeAttributeInstance<H, O, M>
{
}
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> JavaObjectKey
    for NativeAttributeInstance<H, O, M>
{
    fn java_hash_code(&self) -> i32 {
        self.0.identity.java_hash_code()
    }
}
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> NativeAttributeInstance<H, O, M> {
    pub fn same_object(&self, other: &Self) -> bool {
        self == other
    }
    /// Inspect the actual identity token without forcing Object.hashCode.
    pub fn identity(&self) -> &O {
        &self.0.identity
    }
}

type BuilderState<H, O, M> = SupplierBuilderState<H, NativeAttributeInstance<H, O, M>>;

/// The same mutable source Builder, including records appended after build.
/// A source callback never runs while this object's Mutex is borrowed.
pub struct NativeAttributeBuilder<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone>(
    Arc<Mutex<BuilderState<H, O, M>>>,
);
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> Clone
    for NativeAttributeBuilder<H, O, M>
{
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> NativeAttributeBuilder<H, O, M> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(SupplierBuilderState::new())))
    }
    pub fn same_object(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> Default
    for NativeAttributeBuilder<H, O, M>
{
    fn default() -> Self {
        Self::new()
    }
}

struct SupplierObject<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> {
    state: AttributeSupplierState<H, NativeAttributeInstance<H, O, M>>,
    // Java's frozen lambda captures Builder. This source default-supplier
    // bootstrap bundle retains all records while the real static supplier
    // lives, including Bat's displaced first MAX_HEALTH template. Escaped
    // templates alone do not implement a general Java tracing object graph.
    builder: NativeAttributeBuilder<H, O, M>,
}

/// One immutable supplier map with its retained source construction graph.
/// The real default-supplier bootstrap owner retains this through shutdown;
/// every actor of this source type receives the SAME shared handle. This bounded
/// static-lifetime binding does not implement arbitrary supplier destruction
/// while a template alone escapes: FrozenSupplierListener retains the original
/// frozen control, while this bundle retains the full Builder record graph.
pub struct NativeAttributeSupplier<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone>(
    Arc<SupplierObject<H, O, M>>,
);
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> Clone
    for NativeAttributeSupplier<H, O, M>
{
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<H: RegisteredAttributeHolder, O: JavaObjectKey, M: Clone> NativeAttributeSupplier<H, O, M> {
    pub fn same_object(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn state(&self) -> &AttributeSupplierState<H, NativeAttributeInstance<H, O, M>> {
        &self.0.state
    }
    pub fn builder(&self) -> &NativeAttributeBuilder<H, O, M> {
        &self.0.builder
    }
}

/// Mandatory actual external owners. This is not an implementation of global
/// registry/bootstrap/ResourceKey interning or Java's object-hash generator.
///
/// `new_instance_identity` reserves one NEW object token at allocation without
/// eagerly forcing its hash. The token's JavaObjectKey must use the shared real
/// lazy object-hash authority at source hashCode calls; IDs, addresses, private
/// counters and a new RNG do not satisfy this contract. All H values are actual
/// bound canonical Holder.Reference objects from the same registered owner.
///
/// Map/Actor are exact retained source descriptors. with_map is a callback-free
/// short projection to the SAME inline Living map, never an entity-ID lookup,
/// cloned state or second map. All actual callbacks run after that loan ends.
pub trait AttributeBindingOwner {
    type Error;
    type Holder: RegisteredAttributeHolder;
    type Identity: JavaObjectKey;
    type Map: Clone;
    type Actor;

    fn holder_hash_code(&mut self, holder: &Self::Holder) -> Result<i32, Self::Error>;
    fn registered_name(&mut self, holder: &Self::Holder) -> Result<String, Self::Error>;
    fn definition(
        &mut self,
        holder: &Self::Holder,
    ) -> Result<Arc<BoundAttribute<Self::Holder>>, Self::Error>;
    fn new_instance_identity(&mut self) -> Result<Self::Identity, Self::Error>;
    fn with_map<R>(
        &mut self,
        map: &Self::Map,
        access: impl FnOnce(&mut NativeAttributeMap<Self::Holder, Self::Identity, Self::Map>) -> R,
    ) -> Result<R, Self::Error>;
    fn on_attribute_updated(
        &mut self,
        actor: &Self::Actor,
        holder: &Self::Holder,
    ) -> Result<(), Self::Error>;
}

pub type BindingInstance<C> = NativeAttributeInstance<
    <C as AttributeBindingOwner>::Holder,
    <C as AttributeBindingOwner>::Identity,
    <C as AttributeBindingOwner>::Map,
>;
pub type BindingBuilder<C> = NativeAttributeBuilder<
    <C as AttributeBindingOwner>::Holder,
    <C as AttributeBindingOwner>::Identity,
    <C as AttributeBindingOwner>::Map,
>;
pub type BindingSupplier<C> = NativeAttributeSupplier<
    <C as AttributeBindingOwner>::Holder,
    <C as AttributeBindingOwner>::Identity,
    <C as AttributeBindingOwner>::Map,
>;
pub type BindingResult<T, C> = Result<T, BindingError<<C as AttributeBindingOwner>::Error>>;
pub type SupplierBindingResult<T, C> =
    Result<T, SupplierError<BindingError<<C as AttributeBindingOwner>::Error>>>;

/// Concrete adapter for the ported native supplier and map operations. The
/// allocation target is the actual map's Consumer, not an event queue. When
/// using AttributeMap's free get_instance function it must be the same map;
/// the inherent get_instance method binds that source target automatically.
pub struct NativeAttributeBinding<'a, C: AttributeBindingOwner> {
    owner: &'a mut C,
    allocation_map: Option<C::Map>,
}
impl<'a, C: AttributeBindingOwner> NativeAttributeBinding<'a, C> {
    pub fn for_defaults(owner: &'a mut C) -> Self {
        Self {
            owner,
            allocation_map: None,
        }
    }
    pub fn for_map(owner: &'a mut C, map: C::Map) -> Self {
        Self {
            owner,
            allocation_map: Some(map),
        }
    }
    pub fn new_builder(&self) -> BindingBuilder<C> {
        NativeAttributeBuilder::new()
    }
    pub fn build_supplier(
        &mut self,
        builder: &BindingBuilder<C>,
    ) -> SupplierBindingResult<BindingSupplier<C>, C> {
        let state = supplier::build(builder, self)?;
        Ok(NativeAttributeSupplier(Arc::new(SupplierObject {
            state,
            builder: builder.clone(),
        })))
    }
    pub fn get_instance(
        &mut self,
        map: &C::Map,
        holder: &C::Holder,
    ) -> BindingResult<Option<BindingInstance<C>>, C> {
        let previous = self.allocation_map.replace(map.clone());
        let result = attribute_map::get_instance(map, holder, self);
        self.allocation_map = previous;
        result
    }
    /// Invoke the consumer installed in this SAME retained instance. Target
    /// consumers run native onAttributeModified directly, before insertion.
    pub fn notify_instance_dirty(
        &mut self,
        instance: &BindingInstance<C>,
    ) -> SupplierBindingResult<(), C> {
        let consumer = instance.0.consumer.clone();
        supplier::notify_listener(&consumer, instance, self)
    }
    /// Source setBaseValue fields, release instance loan, then the actual
    /// installed consumer. Equal values (including signed zeros) skip it.
    pub fn set_base_value(
        &mut self,
        instance: &BindingInstance<C>,
        value: f64,
    ) -> SupplierBindingResult<(), C> {
        let dirty = <Self as AttributeSupplierHost>::with_instance(self, instance, |state| {
            state.set_base_value_fields(value)
        })
        .map_err(SupplierError::Host)?;
        if dirty {
            self.notify_instance_dirty(instance)?;
        }
        Ok(())
    }
    /// Actual replaceFrom, including the destructive self-copy case. The
    /// source/target loans end before the installed consumer is invoked.
    pub fn replace_from(
        &mut self,
        target: &BindingInstance<C>,
        source: &BindingInstance<C>,
    ) -> SupplierBindingResult<(), C> {
        if target.same_object(source) {
            <Self as AttributeSupplierHost>::with_instance(self, target, |state| {
                state.replace_from_self_fields()
            })
            .map_err(SupplierError::Host)?;
        } else {
            let replacement =
                <Self as AttributeSupplierHost>::with_instance(self, source, |state| {
                    state.replacement_data()
                })
                .map_err(SupplierError::Host)?;
            <Self as AttributeSupplierHost>::with_instance(self, target, |state| {
                state.install_replacement(replacement)
            })
            .map_err(SupplierError::Host)?;
        }
        self.notify_instance_dirty(target)
    }
    fn allocate(
        &mut self,
        state: NativeAttributeState<C::Holder>,
        consumer: SupplierInstanceListener<C::Holder, C::Map>,
    ) -> BindingResult<BindingInstance<C>, C> {
        let identity = self
            .owner
            .new_instance_identity()
            .map_err(BindingError::Owner)?;
        Ok(NativeAttributeInstance(Arc::new(InstanceObject {
            identity,
            state: Mutex::new(state),
            consumer,
        })))
    }
    fn access_instance<R>(
        &mut self,
        instance: &BindingInstance<C>,
        access: impl FnOnce(&mut NativeAttributeState<C::Holder>) -> R,
    ) -> BindingResult<R, C> {
        let mut state = instance
            .0
            .state
            .lock()
            .map_err(|_| BindingError::Poisoned(BindingObject::Instance))?;
        Ok(access(&mut state))
    }
}

impl<C: AttributeBindingOwner> SupplierKeyHost for NativeAttributeBinding<'_, C> {
    type Error = BindingError<C::Error>;
    type Holder = C::Holder;
    fn holder_hash_code(&mut self, holder: &C::Holder) -> Result<i32, Self::Error> {
        self.owner
            .holder_hash_code(holder)
            .map_err(BindingError::Owner)
    }
    fn registered_name(&mut self, holder: &C::Holder) -> Result<String, Self::Error> {
        self.owner
            .registered_name(holder)
            .map_err(BindingError::Owner)
    }
}
impl<C: AttributeBindingOwner> AttributeSupplierHost for NativeAttributeBinding<'_, C> {
    type Instance = BindingInstance<C>;
    type Builder = BindingBuilder<C>;
    type Definition = BoundAttribute<C::Holder>;
    type ModifierKey = ResourceLocation;
    type Listener = C::Map;
    fn with_builder<R>(
        &mut self,
        builder: &Self::Builder,
        access: impl FnOnce(&mut BuilderState<C::Holder, C::Identity, C::Map>) -> R,
    ) -> Result<R, Self::Error> {
        let mut state = builder
            .0
            .lock()
            .map_err(|_| BindingError::Poisoned(BindingObject::Builder))?;
        Ok(access(&mut state))
    }
    fn definition(&mut self, holder: &C::Holder) -> Result<Arc<Self::Definition>, Self::Error> {
        self.owner.definition(holder).map_err(BindingError::Owner)
    }
    fn allocate_instance(
        &mut self,
        instance: NativeAttributeState<C::Holder>,
        listener: SupplierInstanceListener<C::Holder, Self::Listener>,
    ) -> Result<Self::Instance, Self::Error> {
        self.allocate(instance, listener)
    }
    fn with_instance<R>(
        &mut self,
        instance: &Self::Instance,
        access: impl FnOnce(&mut NativeAttributeState<C::Holder>) -> R,
    ) -> Result<R, Self::Error> {
        self.access_instance(instance, access)
    }
    fn target_on_dirty(
        &mut self,
        listener: &Self::Listener,
        instance: &Self::Instance,
    ) -> Result<(), Self::Error> {
        attribute_map::on_attribute_modified(listener, instance, self)
    }
    fn modifier_string(&mut self, modifier: &ResourceLocation) -> Result<String, Self::Error> {
        Ok(modifier.as_string())
    }
}
impl<C: AttributeBindingOwner> AttributeMapHost for NativeAttributeBinding<'_, C> {
    type Error = BindingError<C::Error>;
    type Holder = C::Holder;
    type Instance = BindingInstance<C>;
    type Supplier = BindingSupplier<C>;
    type Map = C::Map;
    type Actor = C::Actor;
    type ModifierKey = ResourceLocation;
    type Definition = BoundAttribute<C::Holder>;
    fn with_map<R>(
        &mut self,
        map: &Self::Map,
        access: impl FnOnce(&mut NativeAttributeMap<C::Holder, C::Identity, C::Map>) -> R,
    ) -> Result<R, Self::Error> {
        self.owner
            .with_map(map, access)
            .map_err(BindingError::Owner)
    }
    fn with_instance<R>(
        &mut self,
        instance: &Self::Instance,
        access: impl FnOnce(&mut NativeAttributeState<C::Holder>) -> R,
    ) -> Result<R, Self::Error> {
        self.access_instance(instance, access)
    }
    fn supplier_instance(
        &mut self,
        supplier: &Self::Supplier,
        holder: &Self::Holder,
    ) -> Result<Option<Self::Instance>, Self::Error> {
        Ok(supplier.0.state.template(holder, self)?.cloned())
    }
    fn definition(&mut self, holder: &C::Holder) -> Result<Arc<Self::Definition>, Self::Error> {
        self.owner.definition(holder).map_err(BindingError::Owner)
    }
    fn allocate_instance(
        &mut self,
        instance: NativeAttributeState<C::Holder>,
    ) -> Result<Self::Instance, Self::Error> {
        let map = self
            .allocation_map
            .clone()
            .ok_or(BindingError::MissingAllocationMap)?;
        self.allocate(instance, SupplierInstanceListener::Target(map))
    }
    fn on_attribute_updated(
        &mut self,
        actor: &Self::Actor,
        holder: &C::Holder,
    ) -> Result<(), Self::Error> {
        self.owner
            .on_attribute_updated(actor, holder)
            .map_err(BindingError::Owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::{
        attribute_map::{self, DirtySet},
        default_attributes::{self, LivingAttributeHolders, MobAttributeHolders},
        definitions::{AttributeValue, RegisteredAttributeOwner, BUILTIN_ATTRIBUTE_SPECS},
    };
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Weak,
        },
    };

    // Controlled canonical objects and supplied hash observations. This fixture
    // is neither a production registry nor an implementation of Java VM hashes.
    struct HolderObject {
        name: String,
        hash: i32,
        value: Arc<AttributeValue>,
    }
    #[derive(Clone)]
    struct Holder(Arc<HolderObject>);
    impl PartialEq for Holder {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for Holder {}
    impl JavaObjectKey for Holder {
        fn java_hash_code(&self) -> i32 {
            self.0.hash
        }
    }
    impl RegisteredAttributeHolder for Holder {}
    struct TokenObject {
        supplied_hash: i32,
        observations: AtomicUsize,
    }
    #[derive(Clone)]
    struct Token(Arc<TokenObject>);
    impl PartialEq for Token {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for Token {}
    impl JavaObjectKey for Token {
        fn java_hash_code(&self) -> i32 {
            self.0.observations.fetch_add(1, Ordering::Relaxed);
            self.0.supplied_hash
        }
    }
    #[derive(Clone)]
    struct Map(Weak<MapObject>);
    struct MapObject(Mutex<Option<NativeAttributeMap<Holder, Token, Map>>>);
    struct Owner {
        definitions: Vec<Arc<BoundAttribute<Holder>>>,
        available_tokens: VecDeque<Token>,
        allocated: Vec<Token>,
        updated: Vec<Holder>,
        fail_update: bool,
    }
    impl RegisteredAttributeOwner for Owner {
        type Holder = Holder;
        type Error = &'static str;
        fn registered_attribute_value(
            &mut self,
            holder: &Holder,
        ) -> Result<Arc<AttributeValue>, Self::Error> {
            Ok(Arc::clone(&holder.0.value))
        }
    }
    impl AttributeBindingOwner for Owner {
        type Error = &'static str;
        type Holder = Holder;
        type Identity = Token;
        type Map = Map;
        type Actor = Map;
        fn holder_hash_code(&mut self, holder: &Holder) -> Result<i32, Self::Error> {
            Ok(holder.java_hash_code())
        }
        fn registered_name(&mut self, holder: &Holder) -> Result<String, Self::Error> {
            Ok(holder.0.name.clone())
        }
        fn definition(
            &mut self,
            holder: &Holder,
        ) -> Result<Arc<BoundAttribute<Holder>>, Self::Error> {
            self.definitions
                .iter()
                .find(|definition| definition.holder() == holder)
                .cloned()
                .ok_or("unregistered fixture holder")
        }
        fn new_instance_identity(&mut self) -> Result<Token, Self::Error> {
            let token = self
                .available_tokens
                .pop_front()
                .ok_or("no supplied token")?;
            self.allocated.push(token.clone());
            Ok(token)
        }
        fn with_map<R>(
            &mut self,
            map: &Map,
            access: impl FnOnce(&mut NativeAttributeMap<Holder, Token, Map>) -> R,
        ) -> Result<R, Self::Error> {
            let root = map.0.upgrade().ok_or("invocation root expired")?;
            let mut state = root.0.lock().map_err(|_| "poisoned fixture map")?;
            Ok(access(state.as_mut().ok_or("uninitialized fixture map")?))
        }
        fn on_attribute_updated(
            &mut self,
            _actor: &Map,
            holder: &Holder,
        ) -> Result<(), Self::Error> {
            self.updated.push(holder.clone());
            if self.fail_update {
                Err("actual update failure")
            } else {
                Ok(())
            }
        }
    }
    fn fixture() -> (Owner, MobAttributeHolders<Holder>, Arc<MapObject>, Map) {
        let mut owner = Owner {
            definitions: Vec::new(),
            available_tokens: (0..32)
                .map(|hash| {
                    Token(Arc::new(TokenObject {
                        supplied_hash: hash * 71 + 17,
                        observations: AtomicUsize::new(0),
                    }))
                })
                .collect(),
            allocated: Vec::new(),
            updated: Vec::new(),
            fail_update: false,
        };
        let holders = BUILTIN_ATTRIBUTE_SPECS
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                Holder(Arc::new(HolderObject {
                    name: spec.name.into(),
                    hash: index as i32 * 137 + 3,
                    value: Arc::new(spec.create_value().unwrap()),
                }))
            })
            .collect::<Vec<_>>();
        for holder in &holders {
            let definition = BoundAttribute::from_registered(holder.clone(), &mut owner).unwrap();
            owner.definitions.push(Arc::new(definition));
        }
        let get = |name| {
            holders
                .iter()
                .find(|holder| holder.0.name == name)
                .unwrap()
                .clone()
        };
        let attributes = MobAttributeHolders {
            living: LivingAttributeHolders {
                max_health: get("minecraft:generic.max_health"),
                knockback_resistance: get("minecraft:generic.knockback_resistance"),
                movement_speed: get("minecraft:generic.movement_speed"),
                armor: get("minecraft:generic.armor"),
                armor_toughness: get("minecraft:generic.armor_toughness"),
                max_absorption: get("minecraft:generic.max_absorption"),
                step_height: get("minecraft:generic.step_height"),
                scale: get("minecraft:generic.scale"),
                gravity: get("minecraft:generic.gravity"),
                safe_fall_distance: get("minecraft:generic.safe_fall_distance"),
                fall_damage_multiplier: get("minecraft:generic.fall_damage_multiplier"),
                jump_strength: get("minecraft:generic.jump_strength"),
                oxygen_bonus: get("minecraft:generic.oxygen_bonus"),
                burning_time: get("minecraft:generic.burning_time"),
                explosion_knockback_resistance: get(
                    "minecraft:generic.explosion_knockback_resistance",
                ),
                water_movement_efficiency: get("minecraft:generic.water_movement_efficiency"),
                movement_efficiency: get("minecraft:generic.movement_efficiency"),
                attack_knockback: get("minecraft:generic.attack_knockback"),
            },
            follow_range: get("minecraft:generic.follow_range"),
        };
        let root = Arc::new(MapObject(Mutex::new(None)));
        let map = Map(Arc::downgrade(&root));
        (owner, attributes, root, map)
    }

    #[test]
    fn actual_bat_defaults_lazy_instance_and_installed_map_consumer() {
        let (mut owner, holders, root, map) = fixture();
        let mut binding = NativeAttributeBinding::for_defaults(&mut owner);
        let supplier = default_attributes::build_bat_supplier(&holders, &mut binding).unwrap();
        assert_eq!(supplier.state().entries().len(), 19);
        assert!(supplier.state().entries()[0].holder() == &holders.living.max_health);
        let records = AttributeSupplierHost::with_builder(&mut binding, supplier.builder(), |b| {
            assert!(b.is_frozen());
            b.entries()
                .iter()
                .map(|e| e.instance().clone())
                .collect::<Vec<_>>()
        })
        .unwrap();
        assert_eq!(records.len(), 20);
        assert!(!records[0].same_object(&records[19]));
        assert_eq!(binding.owner.allocated.len(), 20);
        assert!(binding.owner.allocated.iter().all(|token| token
            .0
            .observations
            .load(Ordering::Relaxed)
            == 0));
        *root.0.lock().unwrap() = Some(NativeAttributeMap::new(supplier.clone()));
        assert_eq!(
            attribute_map::value(&map, &holders.living.max_health, &mut binding).unwrap(),
            6.0
        );
        assert_eq!(
            attribute_map::value(&map, &holders.follow_range, &mut binding).unwrap(),
            16.0
        );
        assert_eq!(
            AttributeMapHost::with_map(&mut binding, &map, |m| m.instantiated_len()).unwrap(),
            0
        );
        let instance = binding
            .get_instance(&map, &holders.living.max_health)
            .unwrap()
            .unwrap();
        assert_eq!(binding.owner.allocated.len(), 21);
        assert!(instance.same_object(
            &binding
                .get_instance(&map, &holders.living.max_health)
                .unwrap()
                .unwrap()
        ));
        assert!(AttributeMapHost::with_map(&mut binding, &map, |m| m
            .dirty_contains(DirtySet::Sync, &instance))
        .unwrap());
        AttributeMapHost::with_map(&mut binding, &map, |m| {
            m.clear_dirty(DirtySet::Update);
            m.clear_dirty(DirtySet::Sync);
        })
        .unwrap();
        binding.set_base_value(&instance, 4.0).unwrap();
        assert!(AttributeMapHost::with_map(&mut binding, &map, |m| m
            .dirty_contains(DirtySet::Update, &instance))
        .unwrap());
        assert_eq!(
            attribute_map::value(&map, &holders.living.max_health, &mut binding).unwrap(),
            4.0
        );
        binding.owner.fail_update = true;
        assert!(attribute_map::refresh_dirty_attributes(&map, &map, &mut binding).is_err());
        assert_eq!(
            AttributeMapHost::with_map(&mut binding, &map, |m| m.dirty_len(DirtySet::Update))
                .unwrap(),
            1
        );
        binding.owner.fail_update = false;
        attribute_map::refresh_dirty_attributes(&map, &map, &mut binding).unwrap();
        assert_eq!(
            AttributeMapHost::with_map(&mut binding, &map, |m| m.dirty_len(DirtySet::Update))
                .unwrap(),
            0
        );
        // Frozen failure occurs after mutating the SAME retained template.
        assert!(matches!(
            binding.set_base_value(&records[19], 8.0),
            Err(SupplierError::FrozenTemplate { .. })
        ));
        assert_eq!(
            AttributeSupplierHost::with_instance(&mut binding, &records[19], |s| s.base_value())
                .unwrap(),
            8.0
        );
        assert_eq!(
            AttributeSupplierHost::with_instance(&mut binding, &records[0], |s| s.base_value())
                .unwrap(),
            20.0
        );
    }
}
