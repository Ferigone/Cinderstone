//! Java 1.21 AttributeSupplier14–63 and Builder66–93.
//!
//! The builder's shared frozen field and ordered records, immutable supplier
//! map, and concrete native instance operations live here. Actual registered
//! holders/definitions, retained arenas and target dirty consumers are required
//! bindings. This creates no registry, Java identity hash, actor or Main owner.
//!
//! The map domain is nonnull canonical registered Holder.Reference objects:
//! equality is actual object identity, with stable actual Object.hashCode.
//! Direct/unbound/arbitrary user keys and callback-bearing equals are outside
//! this domain. Guava32.1.2 regular-map bucket overflow is handled, including
//! its JDK fallback's first-key position/last-value and hash-call phases; the
//! logical fallback does not manufacture a Java HashMap tree/object owner.

use super::attribute_instance::{AttributeDefinition, AttributeInstance, ResourceLocationKey};
use super::attribute_map::JavaObjectKey;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Implement only for actual registered reference handles. Eq must compare
/// reference identity, not resource-key text or Holder.is; Clone retains it.
pub trait RegisteredAttributeHolder: JavaObjectKey {}

/// Actual source Object.hashCode and Holder.getRegisteredName owners. Hashing
/// may initialize the real object's identity hash; never derive it from IDs,
/// arena positions or an incidental Rust address. No unavailable-owner default.
pub trait SupplierKeyHost {
    type Error;
    type Holder: RegisteredAttributeHolder;
    fn holder_hash_code(&mut self, holder: &Self::Holder) -> Result<i32, Self::Error>;
    fn registered_name(&mut self, holder: &Self::Holder) -> Result<String, Self::Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum SupplierError<E> {
    Host(E),
    /// Source UnsupportedOperationException, after the dirty mutation.
    FrozenTemplate {
        registered_name: String,
    },
    /// Source IllegalArgumentException from getAttributeInstance19–23.
    MissingAttribute {
        registered_name: String,
    },
    /// Source IllegalArgumentException from getModifierValue35–39.
    MissingModifier {
        modifier: String,
        registered_name: String,
    },
}

/// Native storage of THIS builder's boolean, retained by every source consumer
/// capturing it. Atomic access is Rust shared ownership only; callers still
/// serialize source operations in their real owner context.
#[derive(Default)]
struct BuilderControl {
    frozen: AtomicBool,
}

/// One source lambda capturing the original builder and holder. It survives
/// builder handle release and is shared with that same template arena object.
#[derive(Clone)]
pub struct FrozenSupplierListener<H> {
    control: Arc<BuilderControl>,
    holder: H,
}
impl<H> FrozenSupplierListener<H> {
    pub fn is_frozen(&self) -> bool {
        self.control.frozen.load(Ordering::Relaxed)
    }
    /// Invoke only at the actual dirty-listener point, after source mutation.
    /// The builder/instance borrow must have ended before the name provider can
    /// reenter. Unfrozen listeners do not read the registered name at all.
    pub fn on_dirty<C: SupplierKeyHost<Holder = H> + ?Sized>(
        &self,
        context: &mut C,
    ) -> Result<(), SupplierError<C::Error>> {
        if self.is_frozen() {
            let registered_name = context
                .registered_name(&self.holder)
                .map_err(SupplierError::Host)?;
            return Err(SupplierError::FrozenTemplate { registered_name });
        }
        Ok(())
    }
}

/// The actual consumer installed with a NEW instance. Arena owners must retain
/// this descriptor with the same instance and route every later dirty call to
/// it; it is not an optional notification or an owner-free success callback.
#[derive(Clone)]
pub enum SupplierInstanceListener<H, L> {
    Frozen(FrozenSupplierListener<H>),
    Target(L),
}

#[derive(Clone)]
pub struct SupplierEntry<H, I> {
    holder: H,
    instance: I,
}
impl<H, I> SupplierEntry<H, I> {
    pub fn holder(&self) -> &H {
        &self.holder
    }
    pub fn instance(&self) -> &I {
        &self.instance
    }
}

/// Actual Guava builder records, including duplicates and failed add(base)
/// prefixes. Building never freezes this list against later appends.
pub struct SupplierBuilderState<H, I> {
    control: Arc<BuilderControl>,
    entries: Vec<SupplierEntry<H, I>>,
}
impl<H, I> SupplierBuilderState<H, I> {
    pub fn new() -> Self {
        Self {
            control: Arc::new(BuilderControl::default()),
            entries: Vec::with_capacity(4),
        }
    }
    pub fn is_frozen(&self) -> bool {
        self.control.frozen.load(Ordering::Relaxed)
    }
    /// Inspection of source pending records, not a mutable map view.
    pub fn entries(&self) -> &[SupplierEntry<H, I>] {
        &self.entries
    }
}
impl<H, I> Default for SupplierBuilderState<H, I> {
    fn default() -> Self {
        Self::new()
    }
}

/// All external object bindings are mandatory. Access closures are short native
/// fragments; none calls the host or leaves an object borrowed across a dirty
/// consumer/name provider. allocate_instance installs this supplied NEW object
/// and consumer in the actual retained arena; it must not return an alias to a
/// template or reconstruct an instance from an ID/scalar. Definitions retain
/// the same actual registered value object. Listener is the actual target
/// Consumer object/handle, not an invented actor/map or queue.
pub trait AttributeSupplierHost: SupplierKeyHost {
    type Instance: Clone;
    type Builder;
    type Definition: AttributeDefinition<Key = Self::Holder>;
    type ModifierKey: ResourceLocationKey;
    type Listener: Clone;

    fn with_builder<R>(
        &mut self,
        builder: &Self::Builder,
        access: impl FnOnce(&mut SupplierBuilderState<Self::Holder, Self::Instance>) -> R,
    ) -> Result<R, Self::Error>;
    fn definition(&mut self, holder: &Self::Holder) -> Result<Arc<Self::Definition>, Self::Error>;
    fn allocate_instance(
        &mut self,
        instance: AttributeInstance<Self::Definition, Self::ModifierKey>,
        listener: SupplierInstanceListener<Self::Holder, Self::Listener>,
    ) -> Result<Self::Instance, Self::Error>;
    fn with_instance<R>(
        &mut self,
        instance: &Self::Instance,
        access: impl FnOnce(&mut AttributeInstance<Self::Definition, Self::ModifierKey>) -> R,
    ) -> Result<R, Self::Error>;
    fn target_on_dirty(
        &mut self,
        listener: &Self::Listener,
        instance: &Self::Instance,
    ) -> Result<(), Self::Error>;
    /// Actual ResourceLocation.toString / String.valueOf(modifier), evaluated
    /// before getRegisteredName in the missing-modifier exception path.
    fn modifier_string(&mut self, modifier: &Self::ModifierKey) -> Result<String, Self::Error>;
}

pub fn notify_listener<H: AttributeSupplierHost + ?Sized>(
    listener: &SupplierInstanceListener<H::Holder, H::Listener>,
    instance: &H::Instance,
    host: &mut H,
) -> Result<(), SupplierError<H::Error>> {
    match listener {
        SupplierInstanceListener::Frozen(listener) => listener.on_dirty(host),
        SupplierInstanceListener::Target(listener) => host
            .target_on_dirty(listener, instance)
            .map_err(SupplierError::Host),
    }
}

fn create_template<H: AttributeSupplierHost + ?Sized>(
    builder: &H::Builder,
    holder: &H::Holder,
    host: &mut H,
) -> Result<(H::Instance, FrozenSupplierListener<H::Holder>), SupplierError<H::Error>> {
    let control = host
        .with_builder(builder, |builder| Arc::clone(&builder.control))
        .map_err(SupplierError::Host)?;
    let listener = FrozenSupplierListener {
        control,
        holder: holder.clone(),
    };
    let definition = host.definition(holder).map_err(SupplierError::Host)?;
    let instance = host
        .allocate_instance(
            AttributeInstance::new(definition),
            SupplierInstanceListener::Frozen(listener.clone()),
        )
        .map_err(SupplierError::Host)?;
    host.with_builder(builder, |builder| {
        builder.entries.push(SupplierEntry {
            holder: holder.clone(),
            instance: instance.clone(),
        });
    })
    .map_err(SupplierError::Host)?;
    Ok((instance, listener))
}

/// Builder.add81–82. Even a frozen builder appends a NEW dirty template; this
/// constructor does not invoke its dirty consumer.
pub fn add<H: AttributeSupplierHost + ?Sized>(
    builder: &H::Builder,
    holder: &H::Holder,
    host: &mut H,
) -> Result<(), SupplierError<H::Error>> {
    create_template(builder, holder, host)?;
    Ok(())
}

/// Builder.add86–88: create+append, then raw setBaseValue. Equal values skip
/// notification (including opposite signed zero); NaN does not. A frozen error
/// retains the appended instance, changed base and dirty flag.
pub fn add_with_base<H: AttributeSupplierHost + ?Sized>(
    builder: &H::Builder,
    holder: &H::Holder,
    base: f64,
    host: &mut H,
) -> Result<(), SupplierError<H::Error>> {
    let (instance, listener) = create_template(builder, holder, host)?;
    let dirty = host
        .with_instance(&instance, |instance| instance.set_base_value_fields(base))
        .map_err(SupplierError::Host)?;
    if dirty {
        listener.on_dirty(host)?;
    }
    Ok(())
}

#[derive(Clone)]
enum Lookup {
    Empty,
    Singleton,
    Regular { buckets: Vec<Vec<usize>> },
    JdkFallback,
}

/// Immutable map topology, retaining shared actual template instances. A
/// builder append/rebuild cannot change a previously returned map's entries;
/// a mutation through its real template lease remains observable by all users.
#[derive(Clone)]
pub struct AttributeSupplierState<H, I> {
    entries: Vec<SupplierEntry<H, I>>,
    lookup: Lookup,
}
impl<H: RegisteredAttributeHolder, I> AttributeSupplierState<H, I> {
    pub fn entries(&self) -> &[SupplierEntry<H, I>] {
        &self.entries
    }
    pub fn template<C: SupplierKeyHost<Holder = H> + ?Sized>(
        &self,
        holder: &H,
        context: &mut C,
    ) -> Result<Option<&I>, C::Error> {
        let selected = match &self.lookup {
            Lookup::Empty => None,
            Lookup::Singleton => self.entries.first().filter(|entry| entry.holder == *holder),
            Lookup::Regular { buckets } => {
                let hash = context.holder_hash_code(holder)?;
                let bucket = smear(hash) as usize & (buckets.len() - 1);
                buckets[bucket]
                    .iter()
                    .map(|index| &self.entries[*index])
                    .find(|entry| *holder == entry.holder)
            }
            Lookup::JdkFallback => {
                // JDK HashMap.get observes this hash once; stored hashes/tree
                // layout do not alter pure canonical-reference equality.
                context.holder_hash_code(holder)?;
                self.entries.iter().find(|entry| *holder == entry.holder)
            }
        };
        Ok(selected.map(|entry| &entry.instance))
    }
    pub fn has_attribute<C: SupplierKeyHost<Holder = H> + ?Sized>(
        &self,
        holder: &H,
        context: &mut C,
    ) -> Result<bool, C::Error> {
        Ok(self.template(holder, context)?.is_some())
    }
}

fn smear(hash: i32) -> u32 {
    (hash as u32)
        .wrapping_mul(0xcc9e_2d51)
        .rotate_left(15)
        .wrapping_mul(0x1b87_3593)
}
fn table_size(size: usize) -> usize {
    let size = size.max(2);
    let highest = 1_usize << (usize::BITS - 1 - size.leading_zeros());
    if size > (1.2_f64 * highest as f64) as usize {
        highest.saturating_mul(2).min(1 << 30)
    } else {
        highest
    }
}

enum ConstructionError<E> {
    Host(E),
    BucketOverflow,
}

fn regular<H: RegisteredAttributeHolder, I: Clone, C: SupplierKeyHost<Holder = H> + ?Sized>(
    input: &[SupplierEntry<H, I>],
    context: &mut C,
) -> Result<AttributeSupplierState<H, I>, ConstructionError<C::Error>> {
    let capacity = table_size(input.len());
    let mut buckets = vec![Vec::<usize>::new(); capacity];
    let mut resolved = vec![0; input.len()];
    for index in (0..input.len()).rev() {
        let hash = context
            .holder_hash_code(&input[index].holder)
            .map_err(ConstructionError::Host)?;
        let bucket = smear(hash) as usize & (capacity - 1);
        let mut duplicate = None;
        for (count, prior) in buckets[bucket].iter().enumerate() {
            if input[*prior].holder == input[index].holder {
                duplicate = Some(*prior);
                break;
            }
            // Source checks after each failed equals, and throws at >8.
            if count >= 8 {
                return Err(ConstructionError::BucketOverflow);
            }
        }
        resolved[index] = duplicate.unwrap_or(index);
        if duplicate.is_none() {
            buckets[bucket].insert(0, index);
        }
    }
    // removeDuplicates walks FORWARD over shared immutable-entry identities.
    let mut retained = Vec::new();
    for index in resolved {
        if !retained.contains(&index) {
            retained.push(index);
        }
    }
    let entries: Vec<_> = retained.iter().map(|index| input[*index].clone()).collect();
    if entries.len() != input.len() && table_size(entries.len()) != capacity {
        // Guava reconstructs the deduplicated map and hashes its entries again;
        // it does not use the singleton Builder shortcut at this inner point.
        return regular(&entries, context);
    }
    // Translate actual bucket-node references into immutable encounter indices.
    for bucket in &mut buckets {
        for index in bucket {
            *index = retained.iter().position(|entry| entry == index).unwrap();
        }
    }
    Ok(AttributeSupplierState {
        entries,
        lookup: Lookup::Regular { buckets },
    })
}

fn jdk_fallback<H: RegisteredAttributeHolder, I: Clone, C: SupplierKeyHost<Holder = H> + ?Sized>(
    input: &[SupplierEntry<H, I>],
    context: &mut C,
) -> Result<AttributeSupplierState<H, I>, C::Error> {
    let mut entries: Vec<SupplierEntry<H, I>> = Vec::new();
    let mut duplicates: Vec<(H, Option<I>)> = Vec::new();
    // Logical JDK delegateMap.put then duplicateMap.put, source FORWARD order.
    for entry in input {
        context.holder_hash_code(&entry.holder)?;
        if let Some(prior) = entries
            .iter_mut()
            .find(|prior| prior.holder == entry.holder)
        {
            prior.instance = entry.instance.clone();
            context.holder_hash_code(&entry.holder)?;
            if let Some(duplicate) = duplicates.iter_mut().find(|prior| prior.0 == entry.holder) {
                duplicate.1 = Some(entry.instance.clone());
            } else {
                duplicates.push((entry.holder.clone(), Some(entry.instance.clone())));
            }
        } else {
            entries.push(entry.clone());
        }
    }
    if !duplicates.is_empty() {
        // Source second pass: containsKey, then get, then put(null) only on the
        // first occurrence. These additional real hash observations matter.
        for entry in input {
            context.holder_hash_code(&entry.holder)?;
            if let Some(duplicate) = duplicates.iter_mut().find(|prior| prior.0 == entry.holder) {
                context.holder_hash_code(&entry.holder)?;
                if duplicate.1.is_some() {
                    context.holder_hash_code(&entry.holder)?;
                    duplicate.1 = None;
                }
            }
        }
    }
    Ok(AttributeSupplierState {
        entries,
        lookup: Lookup::JdkFallback,
    })
}

/// Builder.build92–93. Freeze BEFORE snapshot/construction/hash observations.
/// No listener/name call occurs while the builder is borrowed. Source Guava
/// no-comparator buildKeepingLast preserves the first key position but retains
/// the last-created template. Hash failures do not undo freezing.
pub fn build<H: AttributeSupplierHost + ?Sized>(
    builder: &H::Builder,
    host: &mut H,
) -> Result<AttributeSupplierState<H::Holder, H::Instance>, SupplierError<H::Error>> {
    let entries = host
        .with_builder(builder, |builder| {
            builder.control.frozen.store(true, Ordering::Relaxed);
            builder.entries.clone()
        })
        .map_err(SupplierError::Host)?;
    let lookup = match entries.len() {
        0 => Lookup::Empty,
        1 => Lookup::Singleton,
        _ => match regular(&entries, host) {
            Ok(supplier) => return Ok(supplier),
            Err(ConstructionError::Host(error)) => return Err(SupplierError::Host(error)),
            Err(ConstructionError::BucketOverflow) => {
                return jdk_fallback(&entries, host).map_err(SupplierError::Host)
            }
        },
    };
    Ok(AttributeSupplierState { entries, lookup })
}

fn required<H: AttributeSupplierHost + ?Sized>(
    supplier: &AttributeSupplierState<H::Holder, H::Instance>,
    holder: &H::Holder,
    host: &mut H,
) -> Result<H::Instance, SupplierError<H::Error>> {
    if let Some(instance) = supplier
        .template(holder, host)
        .map_err(SupplierError::Host)?
    {
        return Ok(instance.clone());
    }
    let registered_name = host.registered_name(holder).map_err(SupplierError::Host)?;
    Err(SupplierError::MissingAttribute { registered_name })
}

pub fn value<H: AttributeSupplierHost + ?Sized>(
    supplier: &AttributeSupplierState<H::Holder, H::Instance>,
    holder: &H::Holder,
    host: &mut H,
) -> Result<f64, SupplierError<H::Error>> {
    let instance = required(supplier, holder, host)?;
    host.with_instance(&instance, |instance| instance.value())
        .map_err(SupplierError::Host)
}

pub fn base_value<H: AttributeSupplierHost + ?Sized>(
    supplier: &AttributeSupplierState<H::Holder, H::Instance>,
    holder: &H::Holder,
    host: &mut H,
) -> Result<f64, SupplierError<H::Error>> {
    let instance = required(supplier, holder, host)?;
    host.with_instance(&instance, |instance| instance.base_value())
        .map_err(SupplierError::Host)
}

pub fn has_modifier<H: AttributeSupplierHost + ?Sized>(
    supplier: &AttributeSupplierState<H::Holder, H::Instance>,
    holder: &H::Holder,
    modifier: &H::ModifierKey,
    host: &mut H,
) -> Result<bool, SupplierError<H::Error>> {
    let Some(instance) = supplier
        .template(holder, host)
        .map_err(SupplierError::Host)?
    else {
        return Ok(false);
    };
    host.with_instance(instance, |instance| instance.has_modifier(modifier))
        .map_err(SupplierError::Host)
}

pub fn modifier_value<H: AttributeSupplierHost + ?Sized>(
    supplier: &AttributeSupplierState<H::Holder, H::Instance>,
    holder: &H::Holder,
    modifier: &H::ModifierKey,
    host: &mut H,
) -> Result<f64, SupplierError<H::Error>> {
    let instance = required(supplier, holder, host)?;
    let amount = host
        .with_instance(&instance, |instance| {
            instance
                .modifier(modifier)
                .map(|modifier| modifier.amount())
        })
        .map_err(SupplierError::Host)?;
    if let Some(amount) = amount {
        return Ok(amount);
    }
    let modifier = host
        .modifier_string(modifier)
        .map_err(SupplierError::Host)?;
    let registered_name = host.registered_name(holder).map_err(SupplierError::Host)?;
    Err(SupplierError::MissingModifier {
        modifier,
        registered_name,
    })
}

/// Supplier.createInstance44–50 outside AttributeMap's computeIfAbsent. Missing
/// means null, without construction/name lookup. Capture template, NEW target
/// with actual consumer, replaceFrom its current fields, then invoke consumer.
/// Insertion into an actor map is deliberately absent. Existing AttributeMap's
/// get_instance already composes this shape and its own final insertion.
pub fn create_instance<H: AttributeSupplierHost + ?Sized>(
    supplier: &AttributeSupplierState<H::Holder, H::Instance>,
    listener: H::Listener,
    holder: &H::Holder,
    host: &mut H,
) -> Result<Option<H::Instance>, SupplierError<H::Error>> {
    let Some(template) = supplier
        .template(holder, host)
        .map_err(SupplierError::Host)?
        .cloned()
    else {
        return Ok(None);
    };
    let definition = host.definition(holder).map_err(SupplierError::Host)?;
    let descriptor = SupplierInstanceListener::Target(listener);
    let instance = host
        .allocate_instance(AttributeInstance::new(definition), descriptor.clone())
        .map_err(SupplierError::Host)?;
    let replacement = host
        .with_instance(&template, |source| source.replacement_data())
        .map_err(SupplierError::Host)?;
    host.with_instance(&instance, |target| target.install_replacement(replacement))
        .map_err(SupplierError::Host)?;
    notify_listener(&descriptor, &instance, host)?;
    Ok(Some(instance))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::attribute_instance::{DirtyListener, Modifier, Operation};
    use std::convert::Infallible;

    // Controlled object/arena/context schedule, not a native registry or Main
    // proof. Key numbers name controlled objects; their hash is explicit input.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Holder(u32, i32);
    impl JavaObjectKey for Holder {
        fn java_hash_code(&self) -> i32 {
            self.1
        }
    }
    impl RegisteredAttributeHolder for Holder {}
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ModifierKey(&'static str);
    impl ResourceLocationKey for ModifierKey {
        fn java_hash_code(&self) -> i32 {
            self.0.bytes().fold(0, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(i32::from(byte))
            })
        }
    }
    struct Definition(Holder, f64);
    impl AttributeDefinition for Definition {
        type Key = Holder;
        fn key(&self) -> &Holder {
            &self.0
        }
        fn default_value(&self) -> f64 {
            self.1
        }
        fn sanitize_value(&self, value: f64) -> f64 {
            value.clamp(0.0, 100.0)
        }
    }
    type Native = AttributeInstance<Definition, ModifierKey>;
    #[derive(Debug, PartialEq, Eq)]
    enum Error {
        Hash,
        Name,
        Target,
    }
    struct BeforeFreeze;
    impl DirtyListener<Definition, ModifierKey> for BeforeFreeze {
        type Error = Infallible;
        fn on_dirty(&mut self, _: &mut Native) -> Result<(), Infallible> {
            Ok(())
        }
    }
    #[derive(Default)]
    struct Host {
        builder: Option<SupplierBuilderState<Holder, usize>>,
        instances: Vec<Native>,
        listeners: Vec<SupplierInstanceListener<Holder, u32>>,
        events: Vec<String>,
        fail_hash: bool,
        fail_name: bool,
        fail_target: bool,
        reenter_name: bool,
        replace_template_after_new: Option<usize>,
    }
    impl Host {
        fn new() -> Self {
            Self {
                builder: Some(SupplierBuilderState::new()),
                ..Self::default()
            }
        }
        fn holder(id: u32) -> Holder {
            Holder(id, id as i32)
        }
        fn last(&self) -> usize {
            self.instances.len() - 1
        }
        fn stored_dirty(&mut self, instance: usize) -> Result<(), SupplierError<Error>> {
            let listener = self.listeners[instance].clone();
            notify_listener(&listener, &instance, self)
        }
    }
    impl SupplierKeyHost for Host {
        type Error = Error;
        type Holder = Holder;
        fn holder_hash_code(&mut self, holder: &Holder) -> Result<i32, Error> {
            self.events.push(format!("hash:{}", holder.0));
            if self.fail_hash {
                return Err(Error::Hash);
            }
            Ok(holder.java_hash_code())
        }
        fn registered_name(&mut self, holder: &Holder) -> Result<String, Error> {
            self.events.push(format!("name:{}", holder.0));
            if self.reenter_name {
                self.reenter_name = false;
                // A name provider can use the canonical builder after the
                // source mutation; no whole-builder borrow spans this call.
                add(&(), &Self::holder(99), self).unwrap();
            }
            if self.fail_name {
                return Err(Error::Name);
            }
            Ok(format!("attribute.{}", holder.0))
        }
    }
    impl AttributeSupplierHost for Host {
        type Instance = usize;
        type Builder = ();
        type Definition = Definition;
        type ModifierKey = ModifierKey;
        type Listener = u32;
        fn with_builder<R>(
            &mut self,
            _: &(),
            access: impl FnOnce(&mut SupplierBuilderState<Holder, usize>) -> R,
        ) -> Result<R, Error> {
            Ok(access(self.builder.as_mut().unwrap()))
        }
        fn definition(&mut self, holder: &Holder) -> Result<Arc<Definition>, Error> {
            self.events.push(format!("definition:{}", holder.0));
            Ok(Arc::new(Definition(holder.clone(), 20.0)))
        }
        fn allocate_instance(
            &mut self,
            instance: Native,
            listener: SupplierInstanceListener<Holder, u32>,
        ) -> Result<usize, Error> {
            let index = self.instances.len();
            self.events.push(format!("new:{index}"));
            self.instances.push(instance);
            self.listeners.push(listener);
            if let Some(template) = self.replace_template_after_new.take() {
                self.instances[template].set_base_value_fields(73.0);
            }
            Ok(index)
        }
        fn with_instance<R>(
            &mut self,
            instance: &usize,
            access: impl FnOnce(&mut Native) -> R,
        ) -> Result<R, Error> {
            Ok(access(&mut self.instances[*instance]))
        }
        fn target_on_dirty(&mut self, listener: &u32, instance: &usize) -> Result<(), Error> {
            self.events.push(format!("target:{listener}:{instance}"));
            // This actual controlled consumer synchronously evaluates the new
            // object's current value, clearing its native dirty cache.
            let value = self.instances[*instance].value();
            self.events.push(format!("value:{value}"));
            if self.fail_target {
                return Err(Error::Target);
            }
            Ok(())
        }
        fn modifier_string(&mut self, modifier: &ModifierKey) -> Result<String, Error> {
            self.events.push(format!("modifier:{}", modifier.0));
            Ok(modifier.0.to_owned())
        }
    }

    #[test]
    fn duplicate_templates_keep_first_position_last_object_and_independent_map_snapshots() {
        let mut host = Host::new();
        let a = Host::holder(1);
        let b = Host::holder(2);
        add(&(), &a, &mut host).unwrap();
        add(&(), &b, &mut host).unwrap();
        add_with_base(&(), &a, 6.0, &mut host).unwrap();
        host.events.clear();
        let first = build(&(), &mut host).unwrap();
        assert_eq!(
            host.events,
            ["hash:1", "hash:2", "hash:1", "hash:2", "hash:1"]
        );
        assert_eq!(
            first
                .entries()
                .iter()
                .map(|e| e.holder.0)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(*first.template(&a, &mut host).unwrap().unwrap(), 2);
        assert_eq!(base_value(&first, &a, &mut host).unwrap(), 6.0);
        // add(default) after build succeeds. Old immutable map topology cannot
        // change, while a new build refers to the newly appended actual object.
        add(&(), &a, &mut host).unwrap();
        let second = build(&(), &mut host).unwrap();
        assert_eq!(*second.template(&a, &mut host).unwrap().unwrap(), 3);
        assert_eq!(*first.template(&a, &mut host).unwrap().unwrap(), 2);
        assert_eq!(base_value(&second, &a, &mut host).unwrap(), 20.0);
        assert_eq!(host.builder.as_ref().unwrap().entries().len(), 4);
        assert!(!host.events.iter().any(|event| event.starts_with("name:")));
    }

    #[test]
    fn frozen_add_mutates_then_names_and_reentry_and_name_failure_keep_prefix() {
        let mut host = Host::new();
        let a = Host::holder(1);
        add(&(), &a, &mut host).unwrap();
        let supplier = build(&(), &mut host).unwrap();
        host.events.clear();
        // Equal new default avoids setDirty, even though builder is frozen.
        add_with_base(&(), &a, 20.0, &mut host).unwrap();
        assert!(!host.events.iter().any(|event| event.starts_with("name:")));
        host.reenter_name = true;
        let error = add_with_base(&(), &a, -9.0, &mut host).unwrap_err();
        assert_eq!(
            error,
            SupplierError::FrozenTemplate {
                registered_name: "attribute.1".into()
            }
        );
        assert_eq!(host.instances[2].base_value(), -9.0);
        assert!(host.instances[2].is_dirty());
        assert_eq!(host.builder.as_ref().unwrap().entries().len(), 4);
        assert_eq!(base_value(&supplier, &a, &mut host).unwrap(), 20.0);
        host.fail_name = true;
        assert_eq!(
            add_with_base(&(), &a, f64::NAN, &mut host),
            Err(SupplierError::Host(Error::Name))
        );
        assert!(host.instances[host.last()].base_value().is_nan());
        let retained = 0;
        host.builder = None;
        host.instances[retained].set_base_value_fields(4.0);
        host.fail_name = false;
        assert!(matches!(
            host.stored_dirty(retained),
            Err(SupplierError::FrozenTemplate { .. })
        ));
        // Supplier reads the same retained object, after builder handle release.
        assert_eq!(base_value(&supplier, &a, &mut host).unwrap(), 4.0);
    }

    #[test]
    fn hash_error_retains_frozen_prefix_and_empty_singleton_skip_hashing() {
        let mut host = Host::new();
        let empty = build(&(), &mut host).unwrap();
        host.fail_hash = true;
        assert!(!empty.has_attribute(&Host::holder(1), &mut host).unwrap());
        add(&(), &Host::holder(1), &mut host).unwrap();
        let singleton = build(&(), &mut host).unwrap();
        assert!(singleton
            .has_attribute(&Host::holder(1), &mut host)
            .unwrap());
        assert!(!singleton
            .has_attribute(&Host::holder(2), &mut host)
            .unwrap());
        assert!(!host.events.iter().any(|event| event.starts_with("hash:")));
        add(&(), &Host::holder(2), &mut host).unwrap();
        assert!(matches!(
            build(&(), &mut host),
            Err(SupplierError::Host(Error::Hash))
        ));
        assert!(host.builder.as_ref().unwrap().is_frozen());
        assert!(matches!(
            add_with_base(&(), &Host::holder(3), 30.0, &mut host),
            Err(SupplierError::FrozenTemplate { .. })
        ));
        assert_eq!(host.instances[2].base_value(), 30.0);
    }

    #[test]
    fn bucket_overflow_falls_back_with_source_hash_phases_and_latest_duplicate() {
        let mut host = Host::new();
        for id in 1..=10 {
            add(&(), &Holder(id, 0), &mut host).unwrap();
        }
        add_with_base(&(), &Holder(1, 0), 7.0, &mut host).unwrap();
        host.events.clear();
        let supplier = build(&(), &mut host).unwrap();
        assert!(matches!(supplier.lookup, Lookup::JdkFallback));
        assert_eq!(
            supplier
                .entries()
                .iter()
                .map(|e| e.holder.0)
                .collect::<Vec<_>>(),
            (1..=10).collect::<Vec<_>>()
        );
        assert_eq!(supplier.entries()[0].instance, 10);
        // Failed regular reverse scan: final duplicate1, then10 through2.
        let mut expected = vec!["hash:1".to_owned()];
        expected.extend((2..=10).rev().map(|id| format!("hash:{id}")));
        // JDK first pass delegate.put for each, plus duplicateMap.put for last1.
        expected.extend((1..=10).map(|id| format!("hash:{id}")));
        expected.extend(["hash:1".into(), "hash:1".into()]);
        // Second pass contains for every entry; get+put(null) at first1, get at last1.
        expected.extend(["hash:1".into(), "hash:1".into(), "hash:1".into()]);
        expected.extend((2..=10).map(|id| format!("hash:{id}")));
        expected.extend(["hash:1".into(), "hash:1".into()]);
        assert_eq!(host.events, expected);
        assert_eq!(
            base_value(&supplier, &Holder(1, 0), &mut host).unwrap(),
            7.0
        );
    }

    #[test]
    fn lazy_copy_retains_modifier_objects_reads_after_construction_and_calls_consumer_before_return(
    ) {
        let mut host = Host::new();
        let a = Host::holder(1);
        add_with_base(&(), &a, 6.0, &mut host).unwrap();
        let modifier = Arc::new(Modifier::new(
            ModifierKey("bonus"),
            2.0,
            Operation::AddValue,
        ));
        host.instances[0]
            .add_transient_modifier(Arc::clone(&modifier), &mut BeforeFreeze)
            .unwrap();
        let supplier = build(&(), &mut host).unwrap();
        // New target construction precedes template replaceFrom reads. This
        // controlled arena event changes the canonical retained template there.
        host.replace_template_after_new = Some(0);
        host.events.clear();
        let target = create_instance(&supplier, 40, &a, &mut host)
            .unwrap()
            .unwrap();
        assert_eq!(target, 1);
        assert_eq!(
            host.events,
            ["definition:1", "new:1", "target:40:1", "value:75"]
        );
        assert_eq!(host.instances[target].base_value(), 73.0);
        assert!(!host.instances[target].is_dirty());
        assert!(Arc::ptr_eq(
            host.instances[target].modifier(modifier.id()).unwrap(),
            &modifier
        ));
        // Copy is a new object: later target mutation leaves template raw base.
        host.instances[target].set_base_value_fields(13.0);
        assert_eq!(base_value(&supplier, &a, &mut host).unwrap(), 73.0);
        host.fail_target = true;
        assert_eq!(
            create_instance(&supplier, 41, &a, &mut host),
            Err(SupplierError::Host(Error::Target))
        );
        assert_eq!(host.instances[2].base_value(), 73.0);
        assert!(!host.instances[2].is_dirty());
        let before = host.instances.len();
        host.events.clear();
        assert_eq!(
            create_instance(&supplier, 42, &Host::holder(2), &mut host).unwrap(),
            None
        );
        assert_eq!(host.instances.len(), before);
        assert!(host.events.is_empty());
    }

    #[test]
    fn shared_template_cache_raw_base_and_missing_query_paths_remain_distinct() {
        let mut host = Host::new();
        let a = Host::holder(1);
        add_with_base(&(), &a, 300.0, &mut host).unwrap();
        let supplier = build(&(), &mut host).unwrap();
        let same = supplier.clone();
        assert_eq!(base_value(&same, &a, &mut host).unwrap(), 300.0);
        assert!(host.instances[0].is_dirty());
        assert_eq!(value(&supplier, &a, &mut host).unwrap(), 100.0);
        assert!(!host.instances[0].is_dirty());
        let key = ModifierKey("missing");
        assert!(!has_modifier(&supplier, &Host::holder(2), &key, &mut host).unwrap());
        host.events.clear();
        assert_eq!(
            modifier_value(&supplier, &a, &key, &mut host),
            Err(SupplierError::MissingModifier {
                modifier: "missing".into(),
                registered_name: "attribute.1".into()
            })
        );
        assert_eq!(host.events, ["modifier:missing", "name:1"]);
        host.events.clear();
        assert_eq!(
            value(&supplier, &Host::holder(2), &mut host),
            Err(SupplierError::MissingAttribute {
                registered_name: "attribute.2".into()
            })
        );
        assert_eq!(host.events, ["name:2"]);
    }
}
