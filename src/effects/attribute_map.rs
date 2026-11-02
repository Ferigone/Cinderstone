//! Java 1.21 AttributeMap's instance table, lazy supplier copy and dirty sets.
//!
//! Canonical holder/instance/supplier ownership is supplied by the embedding
//! runtime. This module owns the actual native table/set state, but creates no
//! actor, registry, identity-hash allocator, health field or scheduler. Null
//! keys/elements and mutable raw Java collection views remain outside this
//! state owner. Source transfer and NBT persistence are separate callers of its
//! two distinct live value traversal protocols; multimap operations are separate.

use super::attribute_instance::{
    AttributeDefinition, AttributeInstance, DirtyListener, ResourceLocationKey,
};
use std::{convert::Infallible, sync::Arc};

/// Canonical immutable nonnull Java object key. Clone retains the same actual
/// object, Eq is its source equality, and the hash is its source hashCode.
/// Holder.Reference and AttributeInstance use object identity; a registry ID,
/// arena index or Rust's randomized hash must not silently substitute for it.
pub trait JavaObjectKey: Clone + Eq {
    fn java_hash_code(&self) -> i32;
}

/// Actual immutable Attribute definition, including its source syncable flag.
/// Attribute.isClientSyncable is a field getter, not an actor callback.
pub trait MapAttributeDefinition: AttributeDefinition {
    fn is_client_syncable(&self) -> bool;
}

struct Entry<K, V> {
    key: K,
    value: V,
}

/// Source default Object2ObjectOpenHashMap, nonnull-key domain, f=.75.
struct ObjectTable<K, V> {
    table: Vec<Option<Entry<K, V>>>,
    size: usize,
}
impl<K: JavaObjectKey, V> ObjectTable<K, V> {
    const MIN_TABLE: usize = 32;
    fn new() -> Self {
        Self {
            table: (0..Self::MIN_TABLE).map(|_| None).collect(),
            size: 0,
        }
    }
    fn hash(key: &K) -> usize {
        let hash = (key.java_hash_code() as u32).wrapping_mul(0x9e37_79b9);
        (hash ^ (hash >> 16)) as usize
    }
    fn max_fill(&self) -> usize {
        self.table.len() * 3 / 4
    }
    fn find(&self, key: &K) -> Result<usize, usize> {
        let mask = self.table.len() - 1;
        let mut index = Self::hash(key) & mask;
        loop {
            match &self.table[index] {
                Some(entry) if &entry.key == key => return Ok(index),
                Some(_) => index = (index + 1) & mask,
                None => return Err(index),
            }
        }
    }
    fn get(&self, key: &K) -> Option<&V> {
        self.find(key)
            .ok()
            .map(|index| &self.table[index].as_ref().unwrap().value)
    }
    fn put(&mut self, key: K, value: V) -> Option<V> {
        match self.find(&key) {
            Ok(index) => Some(std::mem::replace(
                &mut self.table[index].as_mut().unwrap().value,
                value,
            )),
            Err(index) => {
                self.table[index] = Some(Entry { key, value });
                let old_size = self.size;
                self.size += 1;
                if old_size >= self.max_fill() {
                    let capacity = (((self.size + 1) as f32 / 0.75_f32) as f64).ceil() as usize;
                    self.rehash(capacity.next_power_of_two().max(2));
                }
                None
            }
        }
    }
    fn remove(&mut self, key: &K) -> bool {
        let Ok(index) = self.find(key) else {
            return false;
        };
        self.table[index] = None;
        self.size -= 1;
        let mask = self.table.len() - 1;
        let mut hole = index;
        loop {
            let mut scan = (hole + 1) & mask;
            loop {
                let Some(entry) = self.table[scan].as_ref() else {
                    if self.table.len() > Self::MIN_TABLE
                        && self.size < self.max_fill() / 4
                        && self.table.len() > 16
                    {
                        self.rehash(self.table.len() / 2);
                    }
                    return true;
                };
                let home = Self::hash(&entry.key) & mask;
                let crosses_hole = if hole <= scan {
                    hole >= home || home > scan
                } else {
                    hole >= home && home > scan
                };
                if crosses_hole {
                    break;
                }
                scan = (scan + 1) & mask;
            }
            self.table[hole] = self.table[scan].take();
            hole = scan;
        }
    }
    fn clear(&mut self) {
        self.size = 0;
        for entry in &mut self.table {
            *entry = None;
        }
    }
    fn rehash(&mut self, capacity: usize) {
        let previous = std::mem::replace(&mut self.table, (0..capacity).map(|_| None).collect());
        let mask = capacity - 1;
        for entry in previous.into_iter().rev().flatten() {
            let mut index = Self::hash(&entry.key) & mask;
            while self.table[index].is_some() {
                index = (index + 1) & mask;
            }
            self.table[index] = Some(entry);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirtySet {
    Update,
    Sync,
}

/// A live source ObjectOpenHashSet iterator, without iterator.remove support.
/// Captured remaining count is not increased when callbacks add dirty entries.
/// The next step reads the set's then-current table; it is not a key snapshot.
pub struct DirtyCursor {
    set: DirtySet,
    position: isize,
    remaining: usize,
}
impl DirtyCursor {
    pub fn has_next(&self) -> bool {
        self.remaining != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirtyIterationError {
    /// Source array access can fail after reentrant table shrink.
    TableIndexOutOfBounds,
    /// Without iterator.remove there is no wrapped list. External deletion can
    /// exhaust scanned slots before the captured count, causing source NPE.
    MissingWrappedEntries,
}

/// Source values().forEach captures initial table position, not size. Each
/// next fragment reads the then-current table, allowing source reentry order.
pub(crate) struct ValuesForEachCursor {
    position: isize,
}

/// Source values().iterator() captures initial size and table position. It is
/// distinct from forEach: additions do not extend its remaining element count.
pub(crate) struct ValuesIteratorCursor {
    position: isize,
    remaining: usize,
}
impl ValuesIteratorCursor {
    pub(crate) fn has_next(&self) -> bool {
        self.remaining != 0
    }
}

/// Actual state of one AttributeMap. Supplier is the real immutable bound
/// source supplier handle; instance keys retain the actual instance objects.
pub struct AttributeMapState<H: JavaObjectKey, I: JavaObjectKey, S> {
    supplier: S,
    instances: ObjectTable<H, I>,
    to_update: ObjectTable<I, ()>,
    to_sync: ObjectTable<I, ()>,
}
impl<H: JavaObjectKey, I: JavaObjectKey, S> AttributeMapState<H, I, S> {
    pub fn new(supplier: S) -> Self {
        Self {
            supplier,
            instances: ObjectTable::new(),
            to_update: ObjectTable::new(),
            to_sync: ObjectTable::new(),
        }
    }
    pub fn supplier(&self) -> &S {
        &self.supplier
    }
    pub fn instance(&self, holder: &H) -> Option<&I> {
        self.instances.get(holder)
    }
    pub fn instantiated_len(&self) -> usize {
        self.instances.size
    }
    pub(crate) fn values_for_each_cursor(&self) -> ValuesForEachCursor {
        ValuesForEachCursor {
            position: self.instances.table.len() as isize,
        }
    }
    pub(crate) fn next_for_each_value(
        &self,
        cursor: &mut ValuesForEachCursor,
    ) -> Result<Option<I>, DirtyIterationError> {
        while cursor.position != 0 {
            cursor.position -= 1;
            let position = cursor.position as usize;
            if position == self.instances.table.len() {
                continue;
            }
            let entry = self
                .instances
                .table
                .get(position)
                .ok_or(DirtyIterationError::TableIndexOutOfBounds)?;
            if let Some(entry) = entry {
                return Ok(Some(entry.value.clone()));
            }
        }
        Ok(None)
    }
    pub(crate) fn values_iterator_cursor(&self) -> ValuesIteratorCursor {
        ValuesIteratorCursor {
            position: self.instances.table.len() as isize,
            remaining: self.instances.size,
        }
    }
    pub(crate) fn next_iterator_value(
        &self,
        cursor: &mut ValuesIteratorCursor,
    ) -> Result<Option<I>, DirtyIterationError> {
        if !cursor.has_next() {
            return Ok(None);
        }
        cursor.remaining -= 1;
        loop {
            cursor.position -= 1;
            if cursor.position < 0 {
                return Err(DirtyIterationError::MissingWrappedEntries);
            }
            let position = cursor.position as usize;
            if position == self.instances.table.len() {
                continue;
            }
            let entry = self
                .instances
                .table
                .get(position)
                .ok_or(DirtyIterationError::TableIndexOutOfBounds)?;
            if let Some(entry) = entry {
                return Ok(Some(entry.value.clone()));
            }
        }
    }
    fn dirty(&self, set: DirtySet) -> &ObjectTable<I, ()> {
        match set {
            DirtySet::Update => &self.to_update,
            DirtySet::Sync => &self.to_sync,
        }
    }
    fn dirty_mut(&mut self, set: DirtySet) -> &mut ObjectTable<I, ()> {
        match set {
            DirtySet::Update => &mut self.to_update,
            DirtySet::Sync => &mut self.to_sync,
        }
    }
    pub fn dirty_len(&self, set: DirtySet) -> usize {
        self.dirty(set).size
    }
    pub fn dirty_contains(&self, set: DirtySet, instance: &I) -> bool {
        self.dirty(set).get(instance).is_some()
    }
    /// Operate on the live source set; existing iterator cursors remain live.
    pub fn remove_dirty(&mut self, set: DirtySet, instance: &I) -> bool {
        self.dirty_mut(set).remove(instance)
    }
    pub fn clear_dirty(&mut self, set: DirtySet) {
        self.dirty_mut(set).clear();
    }
    pub fn dirty_cursor(&self, set: DirtySet) -> DirtyCursor {
        let table = self.dirty(set);
        DirtyCursor {
            set,
            position: table.table.len() as isize,
            remaining: table.size,
        }
    }
    /// Use only a cursor created by this same live state/set owner.
    pub fn next_dirty(&self, cursor: &mut DirtyCursor) -> Result<Option<I>, DirtyIterationError> {
        if !cursor.has_next() {
            return Ok(None);
        }
        cursor.remaining -= 1;
        let table = &self.dirty(cursor.set).table;
        loop {
            cursor.position -= 1;
            if cursor.position < 0 {
                return Err(DirtyIterationError::MissingWrappedEntries);
            }
            // Source arrays have n+1 slots. The reserved null slot at n is
            // still a valid empty read after reentrant table shrink, although
            // null elements are outside this map's domain.
            if cursor.position as usize == table.len() {
                continue;
            }
            let entry = table
                .get(cursor.position as usize)
                .ok_or(DirtyIterationError::TableIndexOutOfBounds)?;
            if let Some(entry) = entry {
                return Ok(Some(entry.key.clone()));
            }
        }
    }
    /// Fixed source onAttributeModified body, with actual instance/definition.
    /// Registration in toUpdate precedes the syncable definition field read.
    pub fn on_attribute_modified<D, K>(&mut self, key: &I, instance: &AttributeInstance<D, K>)
    where
        D: MapAttributeDefinition<Key = H>,
        K: ResourceLocationKey,
    {
        self.to_update.put(key.clone(), ());
        if instance.attribute().is_client_syncable() {
            self.to_sync.put(key.clone(), ());
        }
    }
    /// Bind this listener only to the exact canonical instance identified by key.
    /// The runtime can split its actual instance arena and map for synchronous
    /// modifier APIs without moving either owner out or buffering dirty calls.
    pub fn listener(&mut self, key: I) -> AttributeMapListener<'_, H, I, S> {
        AttributeMapListener { map: self, key }
    }
}

pub struct AttributeMapListener<'a, H: JavaObjectKey, I: JavaObjectKey, S> {
    map: &'a mut AttributeMapState<H, I, S>,
    key: I,
}
impl<D, K, H, I, S> DirtyListener<D, K> for AttributeMapListener<'_, H, I, S>
where
    D: MapAttributeDefinition<Key = H>,
    K: ResourceLocationKey,
    H: JavaObjectKey,
    I: JavaObjectKey,
{
    type Error = Infallible;
    fn on_dirty(&mut self, instance: &mut AttributeInstance<D, K>) -> Result<(), Self::Error> {
        self.map.on_attribute_modified(&self.key, instance);
        Ok(())
    }
}

/// Real owner access. Pure closures release every map/instance borrow before
/// the next host operation. Map and Actor handles retain the same owners for
/// the whole invocation, including removal/reentry; ID reuse cannot retarget
/// them. Handles retain canonical identity/lifetime, and
/// cloning a handle must not make another source object. allocate_instance
/// must install the supplied NEW native object in the real retained arena.
/// Supplier lookup reads its actual bound default instance, without creating a
/// per-actor instance. No callbacks have numerical or empty defaults.
pub trait AttributeMapHost {
    type Error;
    type Holder: JavaObjectKey;
    type Instance: JavaObjectKey;
    type Supplier: Clone;
    type Map;
    type Actor;
    type ModifierKey: ResourceLocationKey;
    type Definition: MapAttributeDefinition<Key = Self::Holder>;

    fn with_map<R>(
        &mut self,
        map: &Self::Map,
        access: impl FnOnce(&mut AttributeMapState<Self::Holder, Self::Instance, Self::Supplier>) -> R,
    ) -> Result<R, Self::Error>;
    fn with_instance<R>(
        &mut self,
        instance: &Self::Instance,
        access: impl FnOnce(&mut AttributeInstance<Self::Definition, Self::ModifierKey>) -> R,
    ) -> Result<R, Self::Error>;
    fn supplier_instance(
        &mut self,
        supplier: &Self::Supplier,
        holder: &Self::Holder,
    ) -> Result<Option<Self::Instance>, Self::Error>;
    fn definition(&mut self, holder: &Self::Holder) -> Result<Arc<Self::Definition>, Self::Error>;
    fn allocate_instance(
        &mut self,
        instance: AttributeInstance<Self::Definition, Self::ModifierKey>,
    ) -> Result<Self::Instance, Self::Error>;
    /// LivingEntity.onAttributeUpdated: actual health/absorption owners are
    /// changed here when their current maxima decrease. Same actor lifetime.
    fn on_attribute_updated(
        &mut self,
        actor: &Self::Actor,
        holder: &Self::Holder,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum AttributeMapError<E> {
    Host(E),
    MissingAttribute,
    /// Source present actor instance followed by getModifier(...).amount on null.
    MissingInstanceModifier,
    /// Source supplier explicitly throws IllegalArgumentException for this case.
    MissingSupplierModifier,
    DirtyIteration(DirtyIterationError),
}

fn current<H: AttributeMapHost>(
    map: &H::Map,
    holder: &H::Holder,
    host: &mut H,
) -> Result<Option<H::Instance>, H::Error> {
    host.with_map(map, |map| map.instance(holder).cloned())
}
fn supplier<H: AttributeMapHost>(map: &H::Map, host: &mut H) -> Result<H::Supplier, H::Error> {
    host.with_map(map, |map| map.supplier().clone())
}

/// AttributeMap.getInstance54 plus AttributeSupplier.createInstance44–50.
/// The JDK Map.computeIfAbsent(Function) default is get -> create -> put. Its
/// final put searches the current table, replacing any reentrant same-key put.
/// Dirty registration occurs BEFORE the new instance is visible in this map.
pub fn get_instance<H: AttributeMapHost>(
    map: &H::Map,
    holder: &H::Holder,
    host: &mut H,
) -> Result<Option<H::Instance>, H::Error> {
    if let Some(instance) = current(map, holder, host)? {
        return Ok(Some(instance));
    }
    let supplier = supplier(map, host)?;
    let Some(template) = host.supplier_instance(&supplier, holder)? else {
        return Ok(None);
    };
    let definition = host.definition(holder)?;
    let instance = host.allocate_instance(AttributeInstance::new(definition))?;
    let replacement = host.with_instance(&template, |source| source.replacement_data())?;
    host.with_instance(&instance, |target| target.install_replacement(replacement))?;
    on_attribute_modified(map, &instance, host)?;
    host.with_map(map, |map| {
        map.instances.put(holder.clone(), instance.clone());
    })?;
    Ok(Some(instance))
}

/// AttributeMap.onAttributeModified34–38, split at source field access points.
pub fn on_attribute_modified<H: AttributeMapHost>(
    map: &H::Map,
    instance: &H::Instance,
    host: &mut H,
) -> Result<(), H::Error> {
    host.with_map(map, |map| {
        map.to_update.put(instance.clone(), ());
    })?;
    let definition = host.with_instance(instance, |instance| Arc::clone(instance.attribute()))?;
    if definition.is_client_syncable() {
        host.with_map(map, |map| {
            map.to_sync.put(instance.clone(), ());
        })?;
    }
    Ok(())
}

pub fn has_attribute<H: AttributeMapHost>(
    map: &H::Map,
    holder: &H::Holder,
    host: &mut H,
) -> Result<bool, H::Error> {
    if current(map, holder, host)?.is_some() {
        return Ok(true);
    }
    let supplier = supplier(map, host)?;
    Ok(host.supplier_instance(&supplier, holder)?.is_some())
}

fn selected<H: AttributeMapHost>(
    map: &H::Map,
    holder: &H::Holder,
    host: &mut H,
) -> Result<H::Instance, AttributeMapError<H::Error>> {
    if let Some(instance) = current(map, holder, host).map_err(AttributeMapError::Host)? {
        return Ok(instance);
    }
    let supplier = supplier(map, host).map_err(AttributeMapError::Host)?;
    host.supplier_instance(&supplier, holder)
        .map_err(AttributeMapError::Host)?
        .ok_or(AttributeMapError::MissingAttribute)
}

/// Queries preserve supplier fallback without materializing a per-actor copy.
pub fn value<H: AttributeMapHost>(
    map: &H::Map,
    holder: &H::Holder,
    host: &mut H,
) -> Result<f64, AttributeMapError<H::Error>> {
    let instance = selected(map, holder, host)?;
    host.with_instance(&instance, |instance| instance.value())
        .map_err(AttributeMapError::Host)
}
pub fn base_value<H: AttributeMapHost>(
    map: &H::Map,
    holder: &H::Holder,
    host: &mut H,
) -> Result<f64, AttributeMapError<H::Error>> {
    let instance = selected(map, holder, host)?;
    host.with_instance(&instance, |instance| instance.base_value())
        .map_err(AttributeMapError::Host)
}
pub fn has_modifier<H: AttributeMapHost>(
    map: &H::Map,
    holder: &H::Holder,
    modifier: &H::ModifierKey,
    host: &mut H,
) -> Result<bool, H::Error> {
    let instance = if let Some(instance) = current(map, holder, host)? {
        Some(instance)
    } else {
        let supplier = supplier(map, host)?;
        host.supplier_instance(&supplier, holder)?
    };
    match instance {
        Some(instance) => host.with_instance(&instance, |instance| instance.has_modifier(modifier)),
        None => Ok(false),
    }
}
pub fn modifier_value<H: AttributeMapHost>(
    map: &H::Map,
    holder: &H::Holder,
    modifier: &H::ModifierKey,
    host: &mut H,
) -> Result<f64, AttributeMapError<H::Error>> {
    let (instance, missing) =
        if let Some(instance) = current(map, holder, host).map_err(AttributeMapError::Host)? {
            (instance, AttributeMapError::MissingInstanceModifier)
        } else {
            let supplier = supplier(map, host).map_err(AttributeMapError::Host)?;
            let instance = host
                .supplier_instance(&supplier, holder)
                .map_err(AttributeMapError::Host)?
                .ok_or(AttributeMapError::MissingAttribute)?;
            (instance, AttributeMapError::MissingSupplierModifier)
        };
    host.with_instance(&instance, |instance| {
        instance
            .modifier(modifier)
            .map(|modifier| modifier.amount())
    })
    .map_err(AttributeMapError::Host)?
    .ok_or(missing)
}

/// LivingEntity.refreshDirtyAttributes1091–1096. Iterate the live set, release
/// all owner borrows before each actual actor callback, and clear ONLY after
/// successful traversal. New reentrant entries are neither snapshotted nor
/// automatically preserved: source clear removes them too after traversal.
pub fn refresh_dirty_attributes<H: AttributeMapHost>(
    actor: &H::Actor,
    map: &H::Map,
    host: &mut H,
) -> Result<(), AttributeMapError<H::Error>> {
    let mut cursor = host
        .with_map(map, |map| map.dirty_cursor(DirtySet::Update))
        .map_err(AttributeMapError::Host)?;
    while cursor.has_next() {
        let instance = host
            .with_map(map, |map| map.next_dirty(&mut cursor))
            .map_err(AttributeMapError::Host)?
            .map_err(AttributeMapError::DirtyIteration)?
            .expect("cursor has remaining entry");
        let holder = host
            .with_instance(&instance, |instance| instance.attribute().key().clone())
            .map_err(AttributeMapError::Host)?;
        host.on_attribute_updated(actor, &holder)
            .map_err(AttributeMapError::Host)?;
    }
    host.with_map(map, |map| map.clear_dirty(DirtySet::Update))
        .map_err(AttributeMapError::Host)
}

#[cfg(test)]
mod tests {
    use super::super::attribute_instance::{resource_location_java_hash, Modifier, Operation};
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    // Controlled canonical identity/hash providers and a real retained object
    // arena for the component checks; these are not production registry owners.
    #[derive(Clone, Debug)]
    struct ObjectKey {
        id: u32,
        hash: i32,
    }
    impl PartialEq for ObjectKey {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for ObjectKey {}
    impl JavaObjectKey for ObjectKey {
        fn java_hash_code(&self) -> i32 {
            self.hash
        }
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ModifierKey(String);
    impl ResourceLocationKey for ModifierKey {
        fn java_hash_code(&self) -> i32 {
            resource_location_java_hash("probe", &self.0)
        }
    }
    struct Definition {
        holder: ObjectKey,
        syncable: bool,
        calculations: Cell<usize>,
    }
    impl AttributeDefinition for Definition {
        type Key = ObjectKey;
        fn key(&self) -> &ObjectKey {
            &self.holder
        }
        fn default_value(&self) -> f64 {
            4.0
        }
        fn sanitize_value(&self, value: f64) -> f64 {
            self.calculations.set(self.calculations.get() + 1);
            value
        }
    }
    impl MapAttributeDefinition for Definition {
        fn is_client_syncable(&self) -> bool {
            self.syncable
        }
    }
    type Instance = AttributeInstance<Definition, ModifierKey>;
    type State = AttributeMapState<ObjectKey, ObjectKey, u32>;

    struct IgnoreDirty;
    impl DirtyListener<Definition, ModifierKey> for IgnoreDirty {
        type Error = Infallible;
        fn on_dirty(&mut self, _: &mut Instance) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    fn key(id: u32) -> ObjectKey {
        ObjectKey {
            id,
            hash: id as i32,
        }
    }
    fn hash_for_slot(slot: usize) -> i32 {
        (0..100_000)
            .find(|hash| {
                ObjectTable::<ObjectKey, ()>::hash(&ObjectKey { id: 0, hash: *hash }) & 31 == slot
            })
            .unwrap()
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Reentry {
        None,
        AddDirty,
        RemoveOther,
        Fail,
    }
    struct Host {
        map: RefCell<State>,
        objects: Vec<(ObjectKey, Rc<RefCell<Instance>>)>,
        definitions: Vec<Arc<Definition>>,
        templates: Vec<(ObjectKey, ObjectKey)>,
        next_id: u32,
        allocations: usize,
        inspect_dirty_prefix: bool,
        saw_dirty_before_put: bool,
        reenter_creation: bool,
        nested_created: Option<ObjectKey>,
        refresh_reentry: Reentry,
        other: Option<ObjectKey>,
        refreshed: Vec<u32>,
    }
    impl Host {
        fn new() -> Self {
            let mut host = Self {
                map: RefCell::new(State::new(9)),
                objects: Vec::new(),
                definitions: Vec::new(),
                templates: Vec::new(),
                next_id: 100,
                allocations: 0,
                inspect_dirty_prefix: true,
                saw_dirty_before_put: false,
                reenter_creation: false,
                nested_created: None,
                refresh_reentry: Reentry::None,
                other: None,
                refreshed: Vec::new(),
            };
            for id in 1..=3 {
                let definition = Arc::new(Definition {
                    holder: key(id),
                    syncable: id != 2,
                    calculations: Cell::new(0),
                });
                let mut instance = Instance::new(Arc::clone(&definition));
                instance
                    .add_permanent_modifier(
                        Arc::new(Modifier::new(
                            ModifierKey("base".into()),
                            1.0,
                            Operation::AddValue,
                        )),
                        &mut IgnoreDirty,
                    )
                    .unwrap();
                let object = key(10 + id);
                host.objects
                    .push((object.clone(), Rc::new(RefCell::new(instance))));
                host.templates.push((key(id), object));
                host.definitions.push(definition);
            }
            host
        }
        fn object(&self, key: &ObjectKey) -> Rc<RefCell<Instance>> {
            Rc::clone(&self.objects.iter().find(|(id, _)| id == key).unwrap().1)
        }
        fn assert_no_owner_borrows(&self) {
            drop(self.map.try_borrow_mut().expect("map held across callback"));
            for (_, object) in &self.objects {
                drop(
                    object
                        .try_borrow_mut()
                        .expect("instance held across callback"),
                );
            }
        }
    }
    impl AttributeMapHost for Host {
        type Error = &'static str;
        type Holder = ObjectKey;
        type Instance = ObjectKey;
        type Supplier = u32;
        type Map = u32;
        type Actor = u32;
        type ModifierKey = ModifierKey;
        type Definition = Definition;
        fn with_map<R>(
            &mut self,
            map: &u32,
            access: impl FnOnce(&mut State) -> R,
        ) -> Result<R, Self::Error> {
            assert_eq!(*map, 7);
            self.assert_no_owner_borrows();
            Ok(access(&mut self.map.borrow_mut()))
        }
        fn with_instance<R>(
            &mut self,
            key: &ObjectKey,
            access: impl FnOnce(&mut Instance) -> R,
        ) -> Result<R, Self::Error> {
            self.assert_no_owner_borrows();
            if self.inspect_dirty_prefix
                && key.id >= 100
                && self.map.borrow().dirty_contains(DirtySet::Update, key)
            {
                let holder = self.object(key).borrow().attribute().key().clone();
                if self.map.borrow().instance(&holder).is_none() {
                    self.saw_dirty_before_put = true;
                }
            }
            Ok(access(&mut self.object(key).borrow_mut()))
        }
        fn supplier_instance(
            &mut self,
            supplier: &u32,
            holder: &ObjectKey,
        ) -> Result<Option<ObjectKey>, Self::Error> {
            self.assert_no_owner_borrows();
            assert_eq!(*supplier, 9);
            Ok(self
                .templates
                .iter()
                .find(|(key, _)| key == holder)
                .map(|(_, instance)| instance.clone()))
        }
        fn definition(&mut self, holder: &ObjectKey) -> Result<Arc<Definition>, Self::Error> {
            self.assert_no_owner_borrows();
            Ok(Arc::clone(
                self.definitions
                    .iter()
                    .find(|definition| definition.key() == holder)
                    .unwrap(),
            ))
        }
        fn allocate_instance(&mut self, instance: Instance) -> Result<ObjectKey, Self::Error> {
            self.assert_no_owner_borrows();
            let holder = instance.attribute().key().clone();
            let slots = [20, 10, 15];
            let id = ObjectKey {
                id: self.next_id,
                hash: hash_for_slot(slots[self.allocations % 3]),
            };
            self.next_id += 1;
            self.allocations += 1;
            self.objects
                .push((id.clone(), Rc::new(RefCell::new(instance))));
            if self.reenter_creation {
                self.reenter_creation = false;
                self.nested_created = get_instance(&7, &holder, self)?;
            }
            Ok(id)
        }
        fn on_attribute_updated(
            &mut self,
            actor: &u32,
            holder: &ObjectKey,
        ) -> Result<(), Self::Error> {
            self.assert_no_owner_borrows();
            assert_eq!(*actor, 11);
            self.refreshed.push(holder.id);
            match std::mem::replace(&mut self.refresh_reentry, Reentry::None) {
                Reentry::None => {}
                Reentry::AddDirty => {
                    let other = self.other.clone().unwrap();
                    on_attribute_modified(&7, &other, self)?;
                }
                Reentry::RemoveOther => {
                    self.map
                        .borrow_mut()
                        .remove_dirty(DirtySet::Update, self.other.as_ref().unwrap());
                }
                Reentry::Fail => return Err("actual onAttributeUpdated failed"),
            }
            Ok(())
        }
    }

    #[test]
    fn supplier_queries_stay_lazy_then_actual_copy_registers_dirty_before_put() {
        let mut host = Host::new();
        assert!(has_attribute(&7, &key(1), &mut host).unwrap());
        assert!(!has_attribute(&7, &key(99), &mut host).unwrap());
        assert_eq!(value(&7, &key(1), &mut host).unwrap(), 5.0);
        assert_eq!(value(&7, &key(1), &mut host).unwrap(), 5.0);
        assert_eq!(host.definitions[0].calculations.get(), 1);
        assert_eq!(base_value(&7, &key(1), &mut host).unwrap(), 4.0);
        assert!(has_modifier(&7, &key(1), &ModifierKey("base".into()), &mut host).unwrap());
        assert_eq!(
            modifier_value(&7, &key(1), &ModifierKey("base".into()), &mut host).unwrap(),
            1.0
        );
        assert_eq!(
            modifier_value(&7, &key(1), &ModifierKey("missing".into()), &mut host),
            Err(AttributeMapError::MissingSupplierModifier)
        );
        assert_eq!(host.map.borrow().instantiated_len(), 0);
        assert_eq!(host.allocations, 0);
        assert!(get_instance(&7, &key(99), &mut host).unwrap().is_none());

        let instance = get_instance(&7, &key(1), &mut host).unwrap().unwrap();
        assert!(host.saw_dirty_before_put);
        assert_eq!(host.allocations, 1);
        assert_eq!(
            get_instance(&7, &key(1), &mut host).unwrap(),
            Some(instance.clone())
        );
        assert_eq!(host.allocations, 1);
        let template = host.object(&key(11));
        let actual = host.object(&instance);
        assert!(Arc::ptr_eq(
            actual
                .borrow()
                .modifier(&ModifierKey("base".into()))
                .unwrap(),
            template
                .borrow()
                .modifier(&ModifierKey("base".into()))
                .unwrap()
        ));
        assert!(host
            .map
            .borrow()
            .dirty_contains(DirtySet::Update, &instance));
        assert!(host.map.borrow().dirty_contains(DirtySet::Sync, &instance));
        assert_eq!(
            modifier_value(&7, &key(1), &ModifierKey("missing".into()), &mut host),
            Err(AttributeMapError::MissingInstanceModifier)
        );
        assert_eq!(
            value(&7, &key(99), &mut host),
            Err(AttributeMapError::MissingAttribute)
        );

        let nonsync = get_instance(&7, &key(2), &mut host).unwrap().unwrap();
        assert!(host.map.borrow().dirty_contains(DirtySet::Update, &nonsync));
        assert!(!host.map.borrow().dirty_contains(DirtySet::Sync, &nonsync));
        actual
            .borrow_mut()
            .set_base_value(8.0, &mut host.map.borrow_mut().listener(instance.clone()))
            .unwrap();
        assert_eq!(value(&7, &key(1), &mut host).unwrap(), 9.0);
    }

    #[test]
    fn jdk_default_compute_if_absent_reentry_returns_outer_and_replaces_nested_put() {
        let mut host = Host::new();
        host.reenter_creation = true;
        let outer = get_instance(&7, &key(1), &mut host).unwrap().unwrap();
        let inner = host.nested_created.clone().unwrap();
        assert_ne!(outer, inner);
        assert_eq!(host.allocations, 2);
        assert_eq!(host.map.borrow().instance(&key(1)), Some(&outer));
        assert!(host.map.borrow().dirty_contains(DirtySet::Update, &outer));
        assert!(host.map.borrow().dirty_contains(DirtySet::Update, &inner));
        assert_eq!(value(&7, &key(1), &mut host).unwrap(), 5.0);
    }

    #[test]
    fn live_dirty_refresh_keeps_source_count_reentry_clear_and_error_prefixes() {
        let mut host = Host::new();
        let first = get_instance(&7, &key(1), &mut host).unwrap().unwrap();
        let second = get_instance(&7, &key(2), &mut host).unwrap().unwrap();
        let added = get_instance(&7, &key(3), &mut host).unwrap().unwrap();
        host.map.borrow_mut().clear_dirty(DirtySet::Update);
        on_attribute_modified(&7, &first, &mut host).unwrap();
        on_attribute_modified(&7, &second, &mut host).unwrap();
        host.refresh_reentry = Reentry::AddDirty;
        host.other = Some(added.clone());
        refresh_dirty_attributes(&11, &7, &mut host).unwrap();
        // First slot20, then reentrant slot15, not old slot10. The captured
        // source count remains2 and final clear drops unvisited old+new keys.
        assert_eq!(host.refreshed, [1, 3]);
        assert_eq!(host.map.borrow().dirty_len(DirtySet::Update), 0);
        assert!(host.map.borrow().dirty_contains(DirtySet::Sync, &first));
        assert!(host.map.borrow().dirty_contains(DirtySet::Sync, &added));

        on_attribute_modified(&7, &first, &mut host).unwrap();
        host.refresh_reentry = Reentry::Fail;
        assert_eq!(
            refresh_dirty_attributes(&11, &7, &mut host),
            Err(AttributeMapError::Host("actual onAttributeUpdated failed"))
        );
        assert!(host.map.borrow().dirty_contains(DirtySet::Update, &first));

        host.map.borrow_mut().clear_dirty(DirtySet::Update);
        on_attribute_modified(&7, &first, &mut host).unwrap();
        on_attribute_modified(&7, &second, &mut host).unwrap();
        host.refresh_reentry = Reentry::RemoveOther;
        host.other = Some(second);
        assert_eq!(
            refresh_dirty_attributes(&11, &7, &mut host),
            Err(AttributeMapError::DirtyIteration(
                DirtyIterationError::MissingWrappedEntries
            ))
        );
        assert!(host.map.borrow().dirty_contains(DirtySet::Update, &first));
    }

    #[test]
    fn transfer_for_each_sees_added_lower_slot_but_save_iterator_keeps_initial_count() {
        let high = ObjectKey {
            id: 1,
            hash: hash_for_slot(20),
        };
        let low = ObjectKey {
            id: 2,
            hash: hash_for_slot(5),
        };
        let mut state = State::new(9);
        state.instances.put(high, key(101));
        let mut for_each = state.values_for_each_cursor();
        let mut iterator = state.values_iterator_cursor();
        assert_eq!(
            state.next_for_each_value(&mut for_each).unwrap(),
            Some(key(101))
        );
        assert_eq!(
            state.next_iterator_value(&mut iterator).unwrap(),
            Some(key(101))
        );
        // Native callbacks may insert while traversal retains only position.
        state.instances.put(low, key(102));
        assert_eq!(
            state.next_for_each_value(&mut for_each).unwrap(),
            Some(key(102))
        );
        assert_eq!(state.next_for_each_value(&mut for_each).unwrap(), None);
        assert!(!iterator.has_next());
        assert_eq!(state.next_iterator_value(&mut iterator).unwrap(), None);
    }

    #[test]
    fn live_cursor_retains_reserved_null_slot_after_reentrant_shrink() {
        // Matches the actual packaged-fastutil DirtyCursorShrinkProbe.
        let source_key = |slot| ObjectKey {
            id: slot as u32,
            hash: (0..100_000)
                .find(|hash| {
                    ObjectTable::<ObjectKey, ()>::hash(&ObjectKey { id: 0, hash: *hash }) & 127
                        == slot
                })
                .unwrap(),
        };
        let mut state = State::new(9);
        let keys: Vec<_> = (0..49).map(source_key).collect();
        for key in &keys {
            state.to_update.put(key.clone(), ());
        }
        let high = source_key(65);
        state.to_update.put(high.clone(), ());
        for key in &keys[..25] {
            state.remove_dirty(DirtySet::Update, key);
        }
        assert_eq!(state.to_update.table.len(), 128);
        assert_eq!(state.dirty_len(DirtySet::Update), 25);
        let mut cursor = state.dirty_cursor(DirtySet::Update);
        assert_eq!(state.next_dirty(&mut cursor).unwrap(), Some(high));
        assert_eq!(cursor.position, 65);
        state.remove_dirty(DirtySet::Update, &keys[25]);
        state.remove_dirty(DirtySet::Update, &keys[26]);
        assert_eq!(state.to_update.table.len(), 64);
        assert_eq!(
            state.next_dirty(&mut cursor).unwrap(),
            Some(keys[48].clone())
        );
    }
}
