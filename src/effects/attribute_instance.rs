//! Stateful Java 1.21 `AttributeInstance` (`bus`, source lines 26–163).
//!
//! Owns the actual base value, modifier indexes and dirty value cache. It owns
//! no actor, health, registry or equipment. Construction requires the immutable
//! canonical attribute definition; sanitization is its actual virtual behavior.
//! Mutations require the real dirty-listener owner synchronously, including
//! reentrant mutations and its executed prefix on error. Persistence,
//! `AttributeMap`, `replaceFrom`, and mutable Java map views are not implemented.
//!
//! `modifierById` and `permanentModifiers` are source-ordered array maps. The
//! EnumMap operation buckets are fastutil 8.5.12 Object2ObjectOpenHashMaps, with
//! their original probe/rehash/shift and descending-table iteration order. That
//! order affects floating-point accumulation. Canonical ResourceLocation keys
//! are nonnull; generic Java null-key maps are outside this module's domain.

use std::sync::Arc;

/// Immutable canonical ResourceLocation key/handle. Equality and hash must be
/// those of the actual source ResourceLocation, and cloning must retain its
/// value/identity lineage. Rust's randomized HashMap hash is not interchangeable.
pub trait ResourceLocationKey: Clone + Eq {
    fn java_hash_code(&self) -> i32;
}

/// ResourceLocation.hashCode, with Java String's UTF-16 wrapping hash. This
/// helper does not parse, validate, allocate or register a resource location.
pub fn resource_location_java_hash(namespace: &str, path: &str) -> i32 {
    fn string_hash(value: &str) -> i32 {
        value.encode_utf16().fold(0_i32, |hash, unit| {
            hash.wrapping_mul(31).wrapping_add(i32::from(unit))
        })
    }
    string_hash(namespace)
        .wrapping_mul(31)
        .wrapping_add(string_hash(path))
}

/// The actor's actual immutable attribute holder/definition, not a replacement
/// registry entry. Implementations must supply the pinned definition's default
/// and virtual sanitizeValue behavior. There are no default numeric fallbacks.
pub trait AttributeDefinition {
    type Key: ?Sized;
    fn key(&self) -> &Self::Key;
    fn default_value(&self) -> f64;
    fn sanitize_value(&self, value: f64) -> f64;
}

/// The actual EnumMap encounter order from AttributeModifier.Operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Operation {
    AddValue = 0,
    AddMultipliedBase = 1,
    AddMultipliedTotal = 2,
}

/// Immutable AttributeModifier record. Arc identity, not record equality,
/// controls addOrUpdateTransientModifier's source reference-equality shortcut.
#[derive(Debug)]
pub struct Modifier<K> {
    id: K,
    amount: f64,
    operation: Operation,
}
impl<K> Modifier<K> {
    /// The original record constructor applies no numeric clamp.
    pub fn new(id: K, amount: f64, operation: Operation) -> Self {
        Self {
            id,
            amount,
            operation,
        }
    }
    pub fn id(&self) -> &K {
        &self.id
    }
    pub fn amount(&self) -> f64 {
        self.amount
    }
    pub fn operation(&self) -> Operation {
        self.operation
    }
}

/// Required listener owned by the actual attribute-map or supplier context.
/// Reentry must use this same listener; the instance is not loaned out or cloned
/// during callbacks. A listener may call value(), clearing dirty synchronously.
pub trait DirtyListener<D: AttributeDefinition, K: ResourceLocationKey> {
    type Error;
    fn on_dirty(&mut self, instance: &mut AttributeInstance<D, K>) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationError<E> {
    /// Java IllegalArgumentException: modifier ID already applied.
    ModifierAlreadyApplied,
    Listener(E),
}

struct Entry<K> {
    key: K,
    modifier: Arc<Modifier<K>>,
}

/// The source's two Object2ObjectArrayMaps. Replacement retains position and
/// existing key handle; removal shifts later entries instead of swap-removing.
struct ArrayMap<K>(Vec<Entry<K>>);
impl<K: ResourceLocationKey> ArrayMap<K> {
    fn new() -> Self {
        Self(Vec::new())
    }
    fn get(&self, key: &K) -> Option<&Arc<Modifier<K>>> {
        self.0
            .iter()
            .find(|entry| &entry.key == key)
            .map(|entry| &entry.modifier)
    }
    fn put(&mut self, modifier: Arc<Modifier<K>>) -> Option<Arc<Modifier<K>>> {
        if let Some(entry) = self.0.iter_mut().find(|entry| entry.key == modifier.id) {
            return Some(std::mem::replace(&mut entry.modifier, modifier));
        }
        self.0.push(Entry {
            key: modifier.id.clone(),
            modifier,
        });
        None
    }
    fn remove(&mut self, key: &K) -> Option<Arc<Modifier<K>>> {
        let index = self.0.iter().position(|entry| &entry.key == key)?;
        Some(self.0.remove(index).modifier)
    }
    fn snapshot(&self) -> Vec<Arc<Modifier<K>>> {
        self.0
            .iter()
            .map(|entry| Arc::clone(&entry.modifier))
            .collect()
    }
}

/// Domain-specific port of the source operation bucket. ResourceLocation keys
/// cannot be null. There is no callback during table mutation/iteration.
struct OperationMap<K> {
    table: Vec<Option<Entry<K>>>,
    size: usize,
}
impl<K: ResourceLocationKey> OperationMap<K> {
    const MIN_TABLE: usize = 32; // expected=16, f=0.75 -> arraySize=32
    fn new() -> Self {
        Self {
            table: (0..Self::MIN_TABLE).map(|_| None).collect(),
            size: 0,
        }
    }
    fn mixed_hash(key: &K) -> usize {
        let hash = (key.java_hash_code() as u32).wrapping_mul(0x9e37_79b9);
        (hash ^ (hash >> 16)) as usize
    }
    fn max_fill(&self) -> usize {
        self.table.len() * 3 / 4
    }
    fn find(&self, key: &K) -> Result<usize, usize> {
        let mask = self.table.len() - 1;
        let mut index = Self::mixed_hash(key) & mask;
        loop {
            match &self.table[index] {
                Some(entry) if &entry.key == key => return Ok(index),
                Some(_) => index = (index + 1) & mask,
                None => return Err(index),
            }
        }
    }
    fn put(&mut self, modifier: Arc<Modifier<K>>) {
        match self.find(&modifier.id) {
            Ok(index) => {
                self.table[index].as_mut().unwrap().modifier = modifier;
            }
            Err(index) => {
                self.table[index] = Some(Entry {
                    key: modifier.id.clone(),
                    modifier,
                });
                let previous_size = self.size;
                self.size += 1;
                // Source insert tests size++ >= maxFill, then arraySize(size+1).
                if previous_size >= self.max_fill() {
                    let expected = self.size + 1;
                    let capacity = (((expected as f32 / 0.75_f32) as f64).ceil() as usize)
                        .next_power_of_two()
                        .max(2);
                    self.rehash(capacity);
                }
            }
        }
    }
    fn remove(&mut self, key: &K) {
        let Ok(index) = self.find(key) else {
            return;
        };
        self.table[index] = None;
        self.size -= 1;
        let mask = self.table.len() - 1;
        let mut hole = index;
        loop {
            let mut scan = (hole + 1) & mask;
            loop {
                let Some(entry) = self.table[scan].as_ref() else {
                    // The final hole is already None because entries are moved.
                    if self.table.len() > Self::MIN_TABLE
                        && self.size < self.max_fill() / 4
                        && self.table.len() > 16
                    {
                        self.rehash(self.table.len() / 2);
                    }
                    return;
                };
                let home = Self::mixed_hash(&entry.key) & mask;
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
    fn rehash(&mut self, capacity: usize) {
        let previous = std::mem::replace(&mut self.table, (0..capacity).map(|_| None).collect());
        let mask = capacity - 1;
        // fastutil rehash scans old slots descending, resolving collisions anew.
        for entry in previous.into_iter().rev().flatten() {
            let mut index = Self::mixed_hash(&entry.key) & mask;
            while self.table[index].is_some() {
                index = (index + 1) & mask;
            }
            self.table[index] = Some(entry);
        }
    }
    fn values(&self) -> impl Iterator<Item = &Arc<Modifier<K>>> {
        self.table
            .iter()
            .rev()
            .filter_map(|entry| entry.as_ref().map(|entry| &entry.modifier))
    }
}

/// One actual AttributeInstance state owner. It must be retained by its real
/// attribute-map/supplier owner; do not reconstruct it from a scalar total.
pub struct AttributeInstance<D: AttributeDefinition, K: ResourceLocationKey> {
    definition: Arc<D>,
    by_operation: [Option<OperationMap<K>>; 3],
    by_id: ArrayMap<K>,
    permanent: ArrayMap<K>,
    base_value: f64,
    dirty: bool,
    cached_value: f64,
}
impl<D: AttributeDefinition, K: ResourceLocationKey> AttributeInstance<D, K> {
    pub fn new(definition: Arc<D>) -> Self {
        let base_value = definition.default_value();
        Self {
            definition,
            by_operation: [None, None, None],
            by_id: ArrayMap::new(),
            permanent: ArrayMap::new(),
            base_value,
            dirty: true,
            cached_value: 0.0,
        }
    }
    pub fn attribute(&self) -> &Arc<D> {
        &self.definition
    }
    pub fn base_value(&self) -> f64 {
        self.base_value
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    /// Immutable getModifiers snapshot in source by-ID encounter order. IDs are
    /// unique, so distinct entries cannot collapse under record equality.
    pub fn modifiers(&self) -> Vec<Arc<Modifier<K>>> {
        self.by_id.snapshot()
    }
    pub fn permanent_modifiers(&self) -> Vec<Arc<Modifier<K>>> {
        self.permanent.snapshot()
    }
    pub fn modifier(&self, id: &K) -> Option<&Arc<Modifier<K>>> {
        self.by_id.get(id)
    }
    pub fn has_modifier(&self, id: &K) -> bool {
        self.by_id.get(id).is_some()
    }
    /// Read-only values from the operation bucket; this does not create a bucket.
    pub fn operation_modifiers(&self, operation: Operation) -> Vec<Arc<Modifier<K>>> {
        self.by_operation[operation as usize]
            .as_ref()
            .map(|map| map.values().map(Arc::clone).collect())
            .unwrap_or_default()
    }
    pub fn set_base_value<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        value: f64,
        listener: &mut L,
    ) -> Result<(), L::Error> {
        // Java == treats signed zeros as equal and every NaN comparison as false.
        if value == self.base_value {
            return Ok(());
        }
        self.base_value = value;
        self.set_dirty(listener)
    }
    pub fn set_dirty<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        listener: &mut L,
    ) -> Result<(), L::Error> {
        self.dirty = true;
        listener.on_dirty(self)
    }
    fn operation_mut(&mut self, operation: Operation) -> &mut OperationMap<K> {
        self.by_operation[operation as usize].get_or_insert_with(OperationMap::new)
    }
    fn add_modifier<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        modifier: Arc<Modifier<K>>,
        listener: &mut L,
    ) -> Result<(), MutationError<L::Error>> {
        if self.by_id.get(&modifier.id).is_some() {
            return Err(MutationError::ModifierAlreadyApplied);
        }
        self.by_id.put(Arc::clone(&modifier));
        self.operation_mut(modifier.operation).put(modifier);
        self.set_dirty(listener).map_err(MutationError::Listener)
    }
    pub fn add_transient_modifier<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        modifier: Arc<Modifier<K>>,
        listener: &mut L,
    ) -> Result<(), MutationError<L::Error>> {
        self.add_modifier(modifier, listener)
    }
    pub fn add_or_update_transient_modifier<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        modifier: Arc<Modifier<K>>,
        listener: &mut L,
    ) -> Result<(), L::Error> {
        let previous = self.by_id.put(Arc::clone(&modifier));
        if previous
            .as_ref()
            .is_some_and(|previous| Arc::ptr_eq(previous, &modifier))
        {
            return Ok(());
        }
        // Source does not remove an old entry from a different operation bucket.
        self.operation_mut(modifier.operation).put(modifier);
        self.set_dirty(listener)
    }
    pub fn add_permanent_modifier<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        modifier: Arc<Modifier<K>>,
        listener: &mut L,
    ) -> Result<(), MutationError<L::Error>> {
        self.add_modifier(Arc::clone(&modifier), listener)?;
        // Deliberately after the synchronous listener, including any reentry.
        self.permanent.put(modifier);
        Ok(())
    }
    pub fn add_or_replace_permanent_modifier<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        modifier: Arc<Modifier<K>>,
        listener: &mut L,
    ) -> Result<(), MutationError<L::Error>> {
        self.remove_modifier_by_id(&modifier.id, listener)
            .map_err(MutationError::Listener)?;
        self.add_modifier(Arc::clone(&modifier), listener)?;
        self.permanent.put(modifier);
        Ok(())
    }
    /// Java removeModifier(modifier) removes by ID, regardless of Arc identity.
    pub fn remove_modifier<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        modifier: &Modifier<K>,
        listener: &mut L,
    ) -> Result<(), L::Error> {
        self.remove_modifier_by_id(&modifier.id, listener)?;
        Ok(())
    }
    pub fn remove_modifier_by_id<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        id: &K,
        listener: &mut L,
    ) -> Result<bool, L::Error> {
        let Some(modifier) = self.by_id.remove(id) else {
            return Ok(false);
        };
        self.operation_mut(modifier.operation).remove(id);
        self.permanent.remove(id);
        self.set_dirty(listener)?;
        Ok(true)
    }
    pub fn remove_modifiers<L: DirtyListener<D, K> + ?Sized>(
        &mut self,
        listener: &mut L,
    ) -> Result<(), L::Error> {
        // Source copies ImmutableSet before callbacks; new reentrant modifiers
        // are not appended to this removal traversal.
        for modifier in self.modifiers() {
            self.remove_modifier(&modifier, listener)?;
        }
        Ok(())
    }
    pub fn value(&mut self) -> f64 {
        if self.dirty {
            self.cached_value = self.calculate_value();
            self.dirty = false;
        }
        self.cached_value
    }
    fn calculate_value(&self) -> f64 {
        let mut base = self.base_value;
        if let Some(map) = &self.by_operation[Operation::AddValue as usize] {
            for modifier in map.values() {
                base += modifier.amount;
            }
        }
        let mut value = base;
        if let Some(map) = &self.by_operation[Operation::AddMultipliedBase as usize] {
            for modifier in map.values() {
                value += base * modifier.amount;
            }
        }
        if let Some(map) = &self.by_operation[Operation::AddMultipliedTotal as usize] {
            for modifier in map.values() {
                value *= 1.0 + modifier.amount;
            }
        }
        self.definition.sanitize_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    // Controlled source-definition/key providers for state/callback checks.
    // These are not registrations or a production AttributeMap/actor adapter.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Key(String);
    impl ResourceLocationKey for Key {
        fn java_hash_code(&self) -> i32 {
            resource_location_java_hash("probe", &self.0)
        }
    }
    struct Definition {
        default: f64,
        sanitizations: Cell<usize>,
    }
    impl AttributeDefinition for Definition {
        type Key = str;
        fn key(&self) -> &str {
            "controlled:identity_attribute"
        }
        fn default_value(&self) -> f64 {
            self.default
        }
        fn sanitize_value(&self, value: f64) -> f64 {
            self.sanitizations.set(self.sanitizations.get() + 1);
            value
        }
    }
    type Instance = AttributeInstance<Definition, Key>;
    #[derive(Default)]
    struct Listener {
        seen: Vec<(bool, Vec<String>, Vec<String>)>,
        fail: bool,
        read_value: bool,
        remove_during_callback: Option<Key>,
    }
    impl DirtyListener<Definition, Key> for Listener {
        type Error = &'static str;
        fn on_dirty(&mut self, instance: &mut Instance) -> Result<(), Self::Error> {
            self.seen.push((
                instance.is_dirty(),
                ids(instance.modifiers()),
                ids(instance.permanent_modifiers()),
            ));
            if self.read_value {
                instance.value();
            }
            if let Some(id) = self.remove_during_callback.take() {
                instance.remove_modifier_by_id(&id, self)?;
            }
            if self.fail {
                Err("listener failed")
            } else {
                Ok(())
            }
        }
    }
    fn instance(default: f64) -> Instance {
        Instance::new(Arc::new(Definition {
            default,
            sanitizations: Cell::new(0),
        }))
    }
    fn modifier(id: &str, amount: f64, operation: Operation) -> Arc<Modifier<Key>> {
        Arc::new(Modifier::new(Key(id.into()), amount, operation))
    }
    fn ids(modifiers: Vec<Arc<Modifier<Key>>>) -> Vec<String> {
        modifiers
            .into_iter()
            .map(|modifier| modifier.id.0.clone())
            .collect()
    }
    fn operation_ids(instance: &Instance) -> Vec<String> {
        ids(instance.operation_modifiers(Operation::AddValue))
    }

    #[test]
    fn original_java_hash_order_and_nonassociative_value_are_retained() {
        // Observed from unchanged original bus/buu/akr and packaged fastutil:
        // AttributeOrderProbe, attribute-order-probe.log in external scratch.
        assert_eq!(resource_location_java_hash("probe", "a"), -979816783);
        assert_eq!(resource_location_java_hash("probe", "b"), -979816782);
        assert_eq!(resource_location_java_hash("probe", "c"), -979816781);
        let mut instance = instance(0.0);
        let mut listener = Listener::default();
        for (id, amount) in [("a", 1e16), ("b", 1.0), ("c", -1e16)] {
            instance
                .add_transient_modifier(modifier(id, amount, Operation::AddValue), &mut listener)
                .unwrap();
        }
        assert_eq!(ids(instance.modifiers()), ["a", "b", "c"]);
        assert_eq!(operation_ids(&instance), ["a", "c", "b"]);
        assert_eq!(instance.value().to_bits(), 0x3ff0_0000_0000_0000);
        assert!(!instance.is_dirty());
        assert_eq!(instance.attribute().sanitizations.get(), 1);
        assert_eq!(instance.value(), 1.0);
        assert_eq!(instance.attribute().sanitizations.get(), 1);
    }

    #[test]
    fn original_collision_shift_and_growth_order_match_java_probe() {
        let mut instance = instance(0.0);
        let mut listener = Listener::default();
        for (index, id) in ["collision2", "collision35", "collision47", "collision81"]
            .iter()
            .enumerate()
        {
            instance
                .add_transient_modifier(
                    modifier(id, (index + 1) as f64, Operation::AddValue),
                    &mut listener,
                )
                .unwrap();
        }
        assert_eq!(
            operation_ids(&instance),
            ["collision81", "collision47", "collision35", "collision2"]
        );
        instance
            .remove_modifier_by_id(&Key("collision35".into()), &mut listener)
            .unwrap();
        assert_eq!(
            operation_ids(&instance),
            ["collision81", "collision47", "collision2"]
        );
        assert_eq!(
            ids(instance.modifiers()),
            ["collision2", "collision47", "collision81"]
        );
        assert_eq!(instance.value(), 8.0);

        let mut instance = super::tests::instance(0.0);
        for index in 0..24 {
            instance
                .add_transient_modifier(
                    modifier(
                        &format!("rehash{index}"),
                        (index + 1) as f64,
                        Operation::AddValue,
                    ),
                    &mut listener,
                )
                .unwrap();
        }
        assert_eq!(instance.by_operation[0].as_ref().unwrap().table.len(), 32);
        let expected24 = [
            5, 3, 1, 7, 15, 17, 20, 13, 12, 11, 10, 9, 8, 4, 6, 2, 14, 16, 23, 18, 19, 22, 21, 0,
        ];
        assert_eq!(
            operation_ids(&instance),
            expected24.map(|index| format!("rehash{index}"))
        );
        instance
            .add_transient_modifier(
                modifier("rehash24", 25.0, Operation::AddValue),
                &mut listener,
            )
            .unwrap();
        assert_eq!(instance.by_operation[0].as_ref().unwrap().table.len(), 64);
        let expected25 = [
            5, 1, 17, 13, 9, 10, 20, 24, 21, 23, 22, 3, 7, 15, 12, 6, 11, 4, 8, 2, 14, 16, 18, 19,
            0,
        ];
        assert_eq!(
            operation_ids(&instance),
            expected25.map(|index| format!("rehash{index}"))
        );
        assert_eq!(instance.value().to_bits(), 0x4074_5000_0000_0000);
    }

    #[test]
    fn object_identity_and_operation_ghost_entries_follow_original_mutations() {
        let mut instance = instance(0.0);
        let mut listener = Listener::default();
        let original = modifier("same", 2.0, Operation::AddValue);
        instance
            .add_transient_modifier(Arc::clone(&original), &mut listener)
            .unwrap();
        listener.seen.clear();
        instance.value();
        instance
            .add_or_update_transient_modifier(Arc::clone(&original), &mut listener)
            .unwrap();
        assert!(listener.seen.is_empty());
        assert!(!instance.is_dirty());
        let equal_record = modifier("same", 2.0, Operation::AddValue);
        instance
            .add_or_update_transient_modifier(Arc::clone(&equal_record), &mut listener)
            .unwrap();
        assert_eq!(listener.seen.len(), 1);
        assert!(Arc::ptr_eq(
            instance.modifier(&Key("same".into())).unwrap(),
            &equal_record
        ));
        let replacement = modifier("same", 0.5, Operation::AddMultipliedBase);
        instance
            .add_or_update_transient_modifier(Arc::clone(&replacement), &mut listener)
            .unwrap();
        assert_eq!(instance.value(), 3.0);
        assert!(Arc::ptr_eq(
            &instance.operation_modifiers(Operation::AddValue)[0],
            &equal_record
        ));
        instance
            .remove_modifier_by_id(&Key("same".into()), &mut listener)
            .unwrap();
        assert!(!instance.has_modifier(&Key("same".into())));
        assert_eq!(instance.value(), 2.0);
        instance.remove_modifiers(&mut listener).unwrap();
        assert_eq!(instance.value(), 2.0); // snapshot has no by-ID ghost to remove
    }

    #[test]
    fn permanent_insertion_follows_listener_including_failure_and_reentry() {
        let mut instance = instance(0.0);
        let mut listener = Listener {
            read_value: true,
            ..Listener::default()
        };
        let permanent = modifier("permanent", 3.0, Operation::AddValue);
        instance
            .add_permanent_modifier(Arc::clone(&permanent), &mut listener)
            .unwrap();
        assert_eq!(
            listener.seen,
            [(true, vec!["permanent".into()], Vec::<String>::new())]
        );
        assert!(!instance.is_dirty()); // listener's real getValue cleared it
        assert!(Arc::ptr_eq(&instance.permanent_modifiers()[0], &permanent));

        let mut failing = super::tests::instance(0.0);
        let mut fail_listener = Listener {
            fail: true,
            ..Listener::default()
        };
        assert_eq!(
            failing.add_permanent_modifier(Arc::clone(&permanent), &mut fail_listener),
            Err(MutationError::Listener("listener failed"))
        );
        assert!(failing.has_modifier(permanent.id()));
        assert!(failing.is_dirty());
        assert!(failing.permanent_modifiers().is_empty());
        assert_eq!(failing.value(), 3.0);

        let mut reentrant = super::tests::instance(0.0);
        let mut reentrant_listener = Listener {
            remove_during_callback: Some(Key("permanent".into())),
            ..Listener::default()
        };
        reentrant
            .add_permanent_modifier(Arc::clone(&permanent), &mut reentrant_listener)
            .unwrap();
        assert!(!reentrant.has_modifier(permanent.id()));
        assert!(Arc::ptr_eq(&reentrant.permanent_modifiers()[0], &permanent));
        assert_eq!(reentrant.value(), 0.0);
        assert_eq!(reentrant_listener.seen.len(), 2);
    }

    #[test]
    fn duplicate_and_base_cache_boundaries_do_not_invent_notifications() {
        let mut instance = instance(0.0);
        let mut listener = Listener::default();
        assert_eq!(instance.value().to_bits(), 0.0_f64.to_bits());
        instance.set_base_value(-0.0, &mut listener).unwrap();
        assert!(listener.seen.is_empty());
        assert_eq!(instance.base_value().to_bits(), 0.0_f64.to_bits());
        assert!(!instance.is_dirty());
        instance.set_base_value(f64::NAN, &mut listener).unwrap();
        instance.set_base_value(f64::NAN, &mut listener).unwrap();
        assert_eq!(listener.seen.len(), 2);
        assert!(instance.value().is_nan());
        let applied = modifier("duplicate", 1.0, Operation::AddValue);
        instance
            .add_transient_modifier(Arc::clone(&applied), &mut listener)
            .unwrap();
        listener.seen.clear();
        assert_eq!(
            instance.add_permanent_modifier(Arc::clone(&applied), &mut listener),
            Err(MutationError::ModifierAlreadyApplied)
        );
        assert!(listener.seen.is_empty());
        assert!(instance.permanent_modifiers().is_empty());
    }
}
