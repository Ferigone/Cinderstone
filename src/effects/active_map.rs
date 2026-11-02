//! Java 21 HashMap owner for ordinary LivingEntity.activeEffects.
//!
//! Implements the default constructor, nonnull canonical Holder.Reference keys,
//! nonnull retained instance handles, linked bins, stable resize/split, get/put/
//! remove/clear and actual live key/value HashIterator semantics. No actor, key
//! identity-hash allocator, RNG or scheduler is created. Tree bins and general
//! Map/view/serialization APIs are outside this bounded component.

use std::sync::{Arc, Mutex};

/// A nonnull original registry Holder.Reference. Its source Object.equals is
/// reference identity and Object.hashCode is supplied by the actual owner.
/// Methods and Clone are callback-free immutable identity/handle operations.
/// Clone must retain the SAME holder; do not deep-copy a definition or substitute
/// a registry ID, Rust pointer hash, randomized hasher or newly allocated hash.
/// Null, Direct/custom holders and callback-bearing hash/equals are unsupported.
pub trait ActiveEffectKey: Clone {
    fn is_canonical_reference(&self) -> bool;
    fn java_hash_code(&self) -> i32;
    fn same_object(&self, other: &Self) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeBoundary {
    pub capacity: usize,
    pub bucket: usize,
    pub linked_entries: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveMapError {
    UnsupportedKey,
    /// A native owner was poisoned by a prior panic; never classify as CME.
    Poisoned,
    ConcurrentModification,
    NoSuchElement,
    IllegalIteratorState,
    /// The new linked node is already visible, but treeifyBin has not run and
    /// putVal has NOT incremented size/modCount. This capability refusal is NOT
    /// a Java exception simulation. No operation may silently use a list where
    /// the original now requires a tree; clear can explicitly empty this map.
    UnsupportedTreeBin(TreeBoundary),
    /// Source threshold was assigned before allocation; earlier put mutations
    /// remain committed. This reports native allocation failure explicitly.
    Allocation {
        capacity: usize,
    },
}

type Link<K, V> = Option<Arc<Mutex<Node<K, V>>>>;
struct Node<K, V> {
    hash: u32,
    key: K,
    value: V,
    next: Link<K, V>,
}
struct Fields<K, V> {
    table: Vec<Link<K, V>>,
    size: i32,
    mod_count: i32,
    threshold: i32,
    // Diagnostic capability boundary, NOT an invented Java/gameplay field.
    // The underlying Java field prefix is retained exactly at refusal.
    tree_boundary: Option<TreeBoundary>,
}

/// A strong reference to ONE original map object, not a snapshot of its fields.
/// Cloning this handle retains that map through actor map replacement/removal.
/// Values must be nonnull retained instance handles whose Clone is callback-free
/// and never copies an EffectInstance. All borrows are internal short fragments;
/// actor/definition/world callbacks execute only AFTER these methods return.
pub struct ActiveEffectMap<K: ActiveEffectKey, V: Clone> {
    object: Arc<Mutex<Fields<K, V>>>,
}
impl<K: ActiveEffectKey, V: Clone> Clone for ActiveEffectMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            object: Arc::clone(&self.object),
        }
    }
}
impl<K: ActiveEffectKey, V: Clone> Default for ActiveEffectMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveMapState {
    pub size: i32,
    pub mod_count: i32,
    pub capacity: usize,
    pub threshold: i32,
    pub tree_boundary: Option<TreeBoundary>,
}

impl<K: ActiveEffectKey, V: Clone> ActiveEffectMap<K, V> {
    /// Source HashMap()477–479: no table allocation, threshold/size/modCount0,
    /// actual default load factor0.75 (fixed for this active-effects constructor).
    pub fn new() -> Self {
        Self {
            object: Arc::new(Mutex::new(Fields {
                table: Vec::new(),
                size: 0,
                mod_count: 0,
                threshold: 0,
                tree_boundary: None,
            })),
        }
    }
    pub fn same_map(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.object, &other.object)
    }
    pub fn len(&self) -> Result<i32, ActiveMapError> {
        Ok(self
            .object
            .lock()
            .map_err(|_| ActiveMapError::Poisoned)?
            .size)
    }
    pub fn is_empty(&self) -> Result<bool, ActiveMapError> {
        Ok(self.len()? == 0)
    }
    /// Read-only exact source fields plus explicit unsupported-domain status.
    /// This does not turn a blocked tree-prefix into a usable linked-bin map.
    pub fn state(&self) -> Result<ActiveMapState, ActiveMapError> {
        let f = self.object.lock().map_err(|_| ActiveMapError::Poisoned)?;
        Ok(ActiveMapState {
            size: f.size,
            mod_count: f.mod_count,
            capacity: f.table.len(),
            threshold: f.threshold,
            tree_boundary: f.tree_boundary,
        })
    }
    fn hash(key: &K) -> Result<u32, ActiveMapError> {
        if !key.is_canonical_reference() {
            return Err(ActiveMapError::UnsupportedKey);
        }
        let hash = key.java_hash_code() as u32;
        Ok(hash ^ (hash >> 16))
    }
    fn supported(f: &Fields<K, V>) -> Result<(), ActiveMapError> {
        if let Some(boundary) = f.tree_boundary {
            Err(ActiveMapError::UnsupportedTreeBin(boundary))
        } else {
            Ok(())
        }
    }
    fn matching(node: &Node<K, V>, hash: u32, key: &K) -> bool {
        node.hash == hash && key.same_object(&node.key)
    }
    pub fn get(&self, key: &K) -> Result<Option<V>, ActiveMapError> {
        let hash = Self::hash(key)?;
        let f = self.object.lock().map_err(|_| ActiveMapError::Poisoned)?;
        Self::supported(&f)?;
        if f.table.is_empty() {
            return Ok(None);
        }
        let mut current = f.table[(f.table.len() - 1) & hash as usize].clone();
        while let Some(node) = current {
            let n = node.lock().map_err(|_| ActiveMapError::Poisoned)?;
            if Self::matching(&n, hash, key) {
                return Ok(Some(n.value.clone()));
            }
            current = n.next.clone();
        }
        Ok(None)
    }
    pub fn contains_key(&self, key: &K) -> Result<bool, ActiveMapError> {
        Ok(self.get(key)?.is_some())
    }

    /// Source put/putVal, onlyIfAbsent=false, ordinary HashMap's empty
    /// afterNodeAccess/afterNodeInsertion bodies. Existing key/Node identity is
    /// retained on replacement; only its value changes, without modCount/resize.
    pub fn put(&self, key: K, value: V) -> Result<Option<V>, ActiveMapError> {
        let hash = Self::hash(&key)?;
        let mut f = self.object.lock().map_err(|_| ActiveMapError::Poisoned)?;
        Self::supported(&f)?;
        if f.table.is_empty() {
            Self::resize(&mut f)?;
        }
        let bucket = (f.table.len() - 1) & hash as usize;
        if let Some(first) = f.table[bucket].clone() {
            let mut current = first;
            let mut bin_count = 0usize;
            loop {
                let matches = {
                    let node = current.lock().map_err(|_| ActiveMapError::Poisoned)?;
                    Self::matching(&node, hash, &key)
                };
                if matches {
                    return Ok(Some(std::mem::replace(
                        &mut current.lock().map_err(|_| ActiveMapError::Poisoned)?.value,
                        value,
                    )));
                }
                let next = current
                    .lock()
                    .map_err(|_| ActiveMapError::Poisoned)?
                    .next
                    .clone();
                if let Some(next) = next {
                    current = next;
                    bin_count += 1;
                } else {
                    current.lock().map_err(|_| ActiveMapError::Poisoned)?.next =
                        Some(Arc::new(Mutex::new(Node {
                            hash,
                            key,
                            value,
                            next: None,
                        })));
                    if bin_count >= 7 {
                        if f.table.len() < 64 {
                            Self::resize(&mut f)?;
                        } else {
                            let boundary = TreeBoundary {
                                capacity: f.table.len(),
                                bucket,
                                linked_entries: bin_count + 2,
                            };
                            f.tree_boundary = Some(boundary);
                            return Err(ActiveMapError::UnsupportedTreeBin(boundary));
                        }
                    }
                    break;
                }
            }
        } else {
            f.table[bucket] = Some(Arc::new(Mutex::new(Node {
                hash,
                key,
                value,
                next: None,
            })));
        }
        f.mod_count = f.mod_count.wrapping_add(1);
        f.size = f.size.wrapping_add(1);
        if f.size > f.threshold {
            Self::resize(&mut f)?;
        }
        Ok(None)
    }

    /// Source resize684–754. Existing Node identity survives; each old chain
    /// splits by hash&oldCap with stable LOW/HIGH order and terminated tails.
    fn resize(f: &mut Fields<K, V>) -> Result<(), ActiveMapError> {
        const MAXIMUM: usize = 1 << 30;
        let old_capacity = f.table.len();
        let old_threshold = f.threshold;
        let mut new_threshold = 0;
        let new_capacity = if old_capacity > 0 {
            if old_capacity >= MAXIMUM {
                f.threshold = i32::MAX;
                return Ok(());
            }
            let capacity = old_capacity << 1;
            if capacity < MAXIMUM && old_capacity >= 16 {
                new_threshold = old_threshold.wrapping_shl(1);
            }
            capacity
        } else if old_threshold > 0 {
            old_threshold as usize
        } else {
            new_threshold = 12;
            16
        };
        if new_threshold == 0 {
            let product = new_capacity as f32 * 0.75_f32;
            new_threshold = if new_capacity < MAXIMUM && product < (MAXIMUM as f32) {
                product as i32
            } else {
                i32::MAX
            };
        }
        // Source updates threshold BEFORE allocating/assigning the new table.
        f.threshold = new_threshold;
        let mut new_table = Vec::new();
        new_table
            .try_reserve_exact(new_capacity)
            .map_err(|_| ActiveMapError::Allocation {
                capacity: new_capacity,
            })?;
        new_table.resize_with(new_capacity, || None);
        let mut old_table = std::mem::replace(&mut f.table, new_table);
        for (index, slot) in old_table.iter_mut().enumerate() {
            let Some(first) = slot.take() else {
                continue;
            };
            if first
                .lock()
                .map_err(|_| ActiveMapError::Poisoned)?
                .next
                .is_none()
            {
                let bucket = first.lock().map_err(|_| ActiveMapError::Poisoned)?.hash as usize
                    & (new_capacity - 1);
                f.table[bucket] = Some(first);
                continue;
            }
            let (mut low_head, mut low_tail, mut high_head, mut high_tail): (
                Link<K, V>,
                Link<K, V>,
                Link<K, V>,
                Link<K, V>,
            ) = (None, None, None, None);
            let mut current = Some(first);
            while let Some(node) = current {
                let next = node
                    .lock()
                    .map_err(|_| ActiveMapError::Poisoned)?
                    .next
                    .clone();
                if node.lock().map_err(|_| ActiveMapError::Poisoned)?.hash as usize & old_capacity
                    == 0
                {
                    if let Some(tail) = &low_tail {
                        tail.lock().map_err(|_| ActiveMapError::Poisoned)?.next =
                            Some(node.clone());
                    } else {
                        low_head = Some(node.clone());
                    }
                    low_tail = Some(node);
                } else {
                    if let Some(tail) = &high_tail {
                        tail.lock().map_err(|_| ActiveMapError::Poisoned)?.next =
                            Some(node.clone());
                    } else {
                        high_head = Some(node.clone());
                    }
                    high_tail = Some(node);
                }
                current = next;
            }
            if let Some(tail) = low_tail {
                tail.lock().map_err(|_| ActiveMapError::Poisoned)?.next = None;
                f.table[index] = low_head;
            }
            if let Some(tail) = high_tail {
                tail.lock().map_err(|_| ActiveMapError::Poisoned)?.next = None;
                f.table[index + old_capacity] = high_head;
            }
        }
        Ok(())
    }

    pub fn remove(&self, key: &K) -> Result<Option<V>, ActiveMapError> {
        let hash = Self::hash(key)?;
        let mut f = self.object.lock().map_err(|_| ActiveMapError::Poisoned)?;
        Self::supported(&f)?;
        let selected = Self::remove_node(&mut f, hash, key)?;
        match selected {
            Some(node) => Ok(Some(
                node.lock()
                    .map_err(|_| ActiveMapError::Poisoned)?
                    .value
                    .clone(),
            )),
            None => Ok(None),
        }
    }
    fn remove_node(f: &mut Fields<K, V>, hash: u32, key: &K) -> Result<Link<K, V>, ActiveMapError> {
        if f.table.is_empty() {
            return Ok(None);
        }
        let bucket = (f.table.len() - 1) & hash as usize;
        let mut current = f.table[bucket].clone();
        let mut previous: Link<K, V> = None;
        while let Some(node) = current {
            let next = node
                .lock()
                .map_err(|_| ActiveMapError::Poisoned)?
                .next
                .clone();
            let matches = {
                let entry = node.lock().map_err(|_| ActiveMapError::Poisoned)?;
                Self::matching(&entry, hash, key)
            };
            if matches {
                if let Some(previous) = previous {
                    previous.lock().map_err(|_| ActiveMapError::Poisoned)?.next = next;
                } else {
                    f.table[bucket] = next;
                }
                f.mod_count = f.mod_count.wrapping_add(1);
                f.size = f.size.wrapping_sub(1);
                // JDK does NOT clear removed Node.next or overwrite its value.
                return Ok(Some(node));
            }
            previous = Some(node);
            current = next;
        }
        Ok(None)
    }
    /// Source clear866–872 increments modCount EVEN on an empty/unallocated
    /// map, retains table/threshold, and never clears detached Nodes' links.
    /// It can explicitly recover the unsupported treeification-prefix object.
    pub fn clear(&self) -> Result<(), ActiveMapError> {
        let mut f = self.object.lock().map_err(|_| ActiveMapError::Poisoned)?;
        f.mod_count = f.mod_count.wrapping_add(1);
        if !f.table.is_empty() && f.size > 0 {
            f.size = 0;
            for slot in &mut f.table {
                *slot = None;
            }
        }
        f.tree_boundary = None;
        Ok(())
    }
    pub fn key_cursor(&self) -> Result<KeyCursor<K, V>, ActiveMapError> {
        Ok(KeyCursor {
            inner: Cursor::new(self)?,
        })
    }
    pub fn values_cursor(&self) -> Result<ValuesCursor<K, V>, ActiveMapError> {
        Ok(ValuesCursor {
            inner: Cursor::new(self)?,
        })
    }
}

struct Cursor<K: ActiveEffectKey, V: Clone> {
    owner: ActiveEffectMap<K, V>,
    next: Link<K, V>,
    current: Link<K, V>,
    expected_mod_count: i32,
    index: usize,
}
impl<K: ActiveEffectKey, V: Clone> Cursor<K, V> {
    fn new(owner: &ActiveEffectMap<K, V>) -> Result<Self, ActiveMapError> {
        let f = owner.object.lock().map_err(|_| ActiveMapError::Poisoned)?;
        ActiveEffectMap::<K, V>::supported(&f)?;
        let mut cursor = Self {
            owner: owner.clone(),
            next: None,
            current: None,
            expected_mod_count: f.mod_count,
            index: 0,
        };
        if f.size > 0 {
            while cursor.index < f.table.len() {
                cursor.next = f.table[cursor.index].clone();
                cursor.index += 1;
                if cursor.next.is_some() {
                    break;
                }
            }
        }
        Ok(cursor)
    }
    fn has_next(&self) -> bool {
        self.next.is_some()
    }
    fn next_node(&mut self) -> Result<Arc<Mutex<Node<K, V>>>, ActiveMapError> {
        let entry = self.next.clone();
        let f = self
            .owner
            .object
            .lock()
            .map_err(|_| ActiveMapError::Poisoned)?;
        // Source priority, even when next is null: CME precedes exhaustion.
        if f.mod_count != self.expected_mod_count {
            return Err(ActiveMapError::ConcurrentModification);
        }
        ActiveEffectMap::<K, V>::supported(&f)?;
        let entry = entry.ok_or(ActiveMapError::NoSuchElement)?;
        self.current = Some(entry.clone());
        self.next = entry
            .lock()
            .map_err(|_| ActiveMapError::Poisoned)?
            .next
            .clone();
        if self.next.is_none() {
            while self.index < f.table.len() {
                self.next = f.table[self.index].clone();
                self.index += 1;
                if self.next.is_some() {
                    break;
                }
            }
        }
        Ok(entry)
    }
    fn remove(&mut self) -> Result<(), ActiveMapError> {
        // Source priority: illegal state BEFORE modCount mismatch.
        let current = self
            .current
            .clone()
            .ok_or(ActiveMapError::IllegalIteratorState)?;
        let mut f = self
            .owner
            .object
            .lock()
            .map_err(|_| ActiveMapError::Poisoned)?;
        if f.mod_count != self.expected_mod_count {
            return Err(ActiveMapError::ConcurrentModification);
        }
        ActiveEffectMap::<K, V>::supported(&f)?;
        self.current = None;
        let (hash, key) = {
            let node = current.lock().map_err(|_| ActiveMapError::Poisoned)?;
            (node.hash, node.key.clone())
        };
        // JDK21 uses p.hash, NOT a recomputed key.hashCode, and matchValue=false.
        let _ = ActiveEffectMap::<K, V>::remove_node(&mut f, hash, &key)?;
        self.expected_mod_count = f.mod_count;
        Ok(())
    }
}

/// Fresh HashMap.keySet().iterator() semantics. View-object identity/cache APIs
/// are not exposed; this cursor retains the actual map and live Nodes itself.
pub struct KeyCursor<K: ActiveEffectKey, V: Clone> {
    inner: Cursor<K, V>,
}
impl<K: ActiveEffectKey, V: Clone> KeyCursor<K, V> {
    pub fn has_next(&self) -> bool {
        self.inner.has_next()
    }
    pub fn next_key(&mut self) -> Result<K, ActiveMapError> {
        let node = self.inner.next_node()?;
        let value = node
            .lock()
            .map_err(|_| ActiveMapError::Poisoned)?
            .key
            .clone();
        Ok(value)
    }
    pub fn remove(&mut self) -> Result<(), ActiveMapError> {
        self.inner.remove()
    }
    pub fn retained_map(&self) -> ActiveEffectMap<K, V> {
        self.inner.owner.clone()
    }
}
pub struct ValuesCursor<K: ActiveEffectKey, V: Clone> {
    inner: Cursor<K, V>,
}
impl<K: ActiveEffectKey, V: Clone> ValuesCursor<K, V> {
    pub fn has_next(&self) -> bool {
        self.inner.has_next()
    }
    /// Read Node.value at NEXT time, not iterator creation/prefetch. A put of
    /// the same key before this call is nonstructural and supplies its new value.
    pub fn next_value(&mut self) -> Result<V, ActiveMapError> {
        let node = self.inner.next_node()?;
        let value = node
            .lock()
            .map_err(|_| ActiveMapError::Poisoned)?
            .value
            .clone();
        Ok(value)
    }
    pub fn remove(&mut self) -> Result<(), ActiveMapError> {
        self.inner.remove()
    }
    pub fn retained_map(&self) -> ActiveEffectMap<K, V> {
        self.inner.owner.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct KeyData {
        label: i32,
        hash: i32,
        canonical: bool,
        hash_reads: AtomicUsize,
    }
    #[derive(Clone)]
    struct Key(Arc<KeyData>);
    impl ActiveEffectKey for Key {
        fn is_canonical_reference(&self) -> bool {
            self.0.canonical
        }
        fn java_hash_code(&self) -> i32 {
            self.0.hash_reads.fetch_add(1, Ordering::Relaxed);
            self.0.hash
        }
        fn same_object(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    fn key(label: i32, hash: i32) -> Key {
        Key(Arc::new(KeyData {
            label,
            hash,
            canonical: true,
            hash_reads: AtomicUsize::new(0),
        }))
    }
    type Value = Arc<i32>;
    type Map = ActiveEffectMap<Key, Value>;
    fn value(v: i32) -> Value {
        Arc::new(v)
    }
    fn labels(map: &Map) -> Vec<i32> {
        let mut cursor = map.key_cursor().unwrap();
        let mut out = vec![];
        while cursor.has_next() {
            out.push(cursor.next_key().unwrap().0.label);
        }
        out
    }

    #[test]
    fn actual_lazy_fields_empty_clear_and_exception_priority() {
        let map = Map::new();
        assert_eq!(
            map.state().unwrap(),
            ActiveMapState {
                size: 0,
                mod_count: 0,
                capacity: 0,
                threshold: 0,
                tree_boundary: None
            }
        );
        let mut cursor = map.key_cursor().unwrap();
        assert!(!cursor.has_next());
        map.clear().unwrap();
        assert_eq!(map.state().unwrap().mod_count, 1);
        assert_eq!(cursor.remove(), Err(ActiveMapError::IllegalIteratorState));
        assert!(matches!(
            cursor.next_key(),
            Err(ActiveMapError::ConcurrentModification)
        ));
        let mut fresh = map.key_cursor().unwrap();
        assert!(matches!(
            fresh.next_key(),
            Err(ActiveMapError::NoSuchElement)
        ));
        assert_eq!(map.state().unwrap().capacity, 0);
    }

    #[test]
    fn canonical_object_identity_and_unsigned_hash_spread() {
        let map = Map::new();
        let one = key(7, 0x1234_5678);
        let same_label_different_object = key(7, 0x1234_5678);
        map.put(one.clone(), value(1)).unwrap();
        map.put(same_label_different_object.clone(), value(2))
            .unwrap();
        assert_eq!(map.len().unwrap(), 2);
        assert_eq!(**map.get(&one).unwrap().as_ref().unwrap(), 1);
        assert_eq!(
            **map
                .get(&same_label_different_object)
                .unwrap()
                .as_ref()
                .unwrap(),
            2
        );
        let f = map.object.lock().unwrap();
        let bucket = (0x1234_5678_u32 ^ (0x1234_5678_u32 >> 16)) as usize & 15;
        assert!(one.same_object(&f.table[bucket].as_ref().unwrap().lock().unwrap().key));
        drop(f);
        let invalid = Key(Arc::new(KeyData {
            label: 7,
            hash: 0,
            canonical: false,
            hash_reads: AtomicUsize::new(0),
        }));
        let before = map.state().unwrap();
        assert!(matches!(
            map.put(invalid.clone(), value(9)),
            Err(ActiveMapError::UnsupportedKey)
        ));
        assert!(matches!(
            map.get(&invalid),
            Err(ActiveMapError::UnsupportedKey)
        ));
        assert!(matches!(
            map.remove(&invalid),
            Err(ActiveMapError::UnsupportedKey)
        ));
        assert_eq!(map.state().unwrap(), before);
        assert_eq!(invalid.0.hash_reads.load(Ordering::Relaxed), 0);
        let negative = key(-1, -1);
        map.put(negative.clone(), value(3)).unwrap();
        assert_eq!(*map.get(&negative).unwrap().unwrap(), 3);
    }

    #[test]
    fn replacement_is_nonstructural_prefetched_value_is_live_and_selected_value_retained() {
        let map = Map::new();
        let k = key(1, 1);
        let first = value(10);
        map.put(k.clone(), first.clone()).unwrap();
        let mut cursor = map.values_cursor().unwrap();
        let initial = map.state().unwrap();
        let replacement = value(20);
        let old = map.put(k.clone(), replacement.clone()).unwrap().unwrap();
        assert!(Arc::ptr_eq(&old, &first));
        assert_eq!(map.state().unwrap(), initial);
        let selected = cursor.next_value().unwrap();
        assert!(Arc::ptr_eq(&selected, &replacement));
        // Actual synchronous callback can access the same owner: no map/node
        // guard survives next_value, and the source selected handle stays old.
        assert!(map.object.try_lock().is_ok());
        assert!(cursor.inner.current.as_ref().unwrap().try_lock().is_ok());
        let newest = value(30);
        map.put(k.clone(), newest.clone()).unwrap();
        assert_eq!(*selected, 20);
        cursor.remove().unwrap();
        assert!(map.get(&k).unwrap().is_none());
        assert_eq!(*selected, 20);
        assert_eq!(*first, 10);
        assert_eq!(cursor.remove(), Err(ActiveMapError::IllegalIteratorState));
    }

    #[test]
    fn resize_preserves_bucket_split_order_and_actual_node_identity() {
        let map = Map::new();
        let mut keys = vec![];
        for raw in [0, 16, 32, 48, 1] {
            let k = key(raw, raw);
            map.put(k.clone(), value(raw)).unwrap();
            keys.push(k);
        }
        let mut before = map.key_cursor().unwrap();
        assert_eq!(before.next_key().unwrap().0.label, 0);
        assert_eq!(before.next_key().unwrap().0.label, 16);
        let original_node = before.inner.current.clone().unwrap();
        for raw in 2..=9 {
            let k = key(raw, raw);
            map.put(k.clone(), value(raw)).unwrap();
            keys.push(k);
        }
        assert_eq!(
            map.state().unwrap(),
            ActiveMapState {
                size: 13,
                mod_count: 13,
                capacity: 32,
                threshold: 24,
                tree_boundary: None
            }
        );
        assert_eq!(labels(&map), vec![0, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 16, 48]);
        let bucket = map.object.lock().unwrap().table[16].clone().unwrap();
        assert!(Arc::ptr_eq(&original_node, &bucket));
        assert!(before.has_next());
        assert!(matches!(
            before.next_key(),
            Err(ActiveMapError::ConcurrentModification)
        ));
    }

    #[test]
    fn live_iterator_removal_uses_stored_hash_and_allows_continuation() {
        let map = Map::new();
        let a = key(1, 0);
        let b = key(2, 0);
        let c = key(3, 0);
        for k in [&a, &b, &c] {
            map.put(k.clone(), value(k.0.label)).unwrap();
        }
        let mut cursor = map.key_cursor().unwrap();
        assert_eq!(cursor.next_key().unwrap().0.label, 1);
        cursor.remove().unwrap();
        assert_eq!(a.0.hash_reads.load(Ordering::Relaxed), 1); // no rehash in JDK21 iterator.remove
        assert_eq!(cursor.next_key().unwrap().0.label, 2);
        let removed_node = cursor.inner.current.clone().unwrap();
        cursor.remove().unwrap();
        assert!(removed_node.lock().unwrap().next.is_some());
        assert_eq!(cursor.next_key().unwrap().0.label, 3);
        cursor.remove().unwrap();
        assert!(!cursor.has_next());
        assert!(map.is_empty().unwrap());
        assert_eq!(map.state().unwrap().capacity, 16);
        assert!(matches!(
            cursor.next_key(),
            Err(ActiveMapError::NoSuchElement)
        ));
    }

    #[test]
    fn has_next_is_prefetched_only_but_next_and_remove_fail_fast() {
        let map = Map::new();
        let a = key(1, 1);
        let b = key(2, 2);
        map.put(a.clone(), value(1)).unwrap();
        let mut cursor = map.values_cursor().unwrap();
        let selected = cursor.next_value().unwrap();
        assert!(!cursor.has_next());
        map.put(b, value(2)).unwrap();
        assert!(!cursor.has_next());
        assert_eq!(cursor.remove(), Err(ActiveMapError::ConcurrentModification));
        assert!(cursor.inner.current.is_some());
        assert_eq!(
            cursor.next_value(),
            Err(ActiveMapError::ConcurrentModification)
        );
        assert_eq!(*selected, 1);
        let mut another = map.key_cursor().unwrap();
        map.clear().unwrap();
        assert!(another.has_next());
        assert!(matches!(
            another.next_key(),
            Err(ActiveMapError::ConcurrentModification)
        ));
    }

    #[test]
    fn cursor_strongly_retains_old_map_nodes_and_values_after_actor_map_replacement() {
        let old = Map::new();
        let k = key(1, 1);
        let v = value(7);
        let weak = Arc::downgrade(&v);
        old.put(k.clone(), v.clone()).unwrap();
        drop(v);
        let mut cursor = old.values_cursor().unwrap();
        let mut actor_map = Some(old.clone());
        drop(old);
        let replacement = Map::new();
        actor_map.replace(replacement.clone());
        assert!(replacement.is_empty().unwrap());
        let selected = cursor.next_value().unwrap();
        assert_eq!(*selected, 7);
        let retained = cursor.retained_map();
        assert!(!retained.same_map(&replacement));
        retained.clear().unwrap();
        drop(selected);
        drop(retained);
        assert!(weak.upgrade().is_some());
        drop(cursor);
        assert!(weak.upgrade().is_none());
        assert!(actor_map.unwrap().same_map(&replacement));
    }

    #[test]
    fn collision_growth_and_tree_refusal_preserve_exact_linked_prefix() {
        let map = Map::new();
        let mut keys = vec![];
        for label in 0..10 {
            let k = key(label, 0);
            map.put(k.clone(), value(label)).unwrap();
            keys.push(k);
        }
        assert_eq!(map.state().unwrap().capacity, 64);
        assert_eq!(map.state().unwrap().threshold, 48);
        assert_eq!(labels(&map), (0..10).collect::<Vec<_>>());
        let mut cursor = map.key_cursor().unwrap();
        let pending_value = value(10);
        let weak = Arc::downgrade(&pending_value);
        let boundary = TreeBoundary {
            capacity: 64,
            bucket: 0,
            linked_entries: 11,
        };
        assert!(
            matches!(map.put(key(10,0),pending_value),Err(ActiveMapError::UnsupportedTreeBin(b)) if b==boundary)
        );
        let state = map.state().unwrap();
        assert_eq!((state.size, state.mod_count), (10, 10));
        assert_eq!(state.tree_boundary, Some(boundary));
        assert!(weak.upgrade().is_some());
        assert!(cursor.has_next());
        assert!(
            matches!(cursor.next_key(),Err(ActiveMapError::UnsupportedTreeBin(b)) if b==boundary)
        );
        assert!(matches!(
            map.get(&keys[0]),
            Err(ActiveMapError::UnsupportedTreeBin(_))
        ));
        map.clear().unwrap();
        assert_eq!(map.state().unwrap().mod_count, 11);
        assert!(map.is_empty().unwrap());
        assert!(weak.upgrade().is_some());
        drop(cursor);
        assert!(weak.upgrade().is_none());
        map.put(keys[0].clone(), value(99)).unwrap();
        assert_eq!(*map.get(&keys[0]).unwrap().unwrap(), 99);
    }

    #[test]
    fn jvm_modcount_wrap_and_empty_clear_invalidate_exactly() {
        let map = Map::new();
        let a = key(1, 1);
        let b = key(2, 2);
        map.put(a, value(1)).unwrap();
        map.put(b, value(2)).unwrap();
        map.object.lock().unwrap().mod_count = i32::MAX;
        let mut cursor = map.key_cursor().unwrap();
        cursor.next_key().unwrap();
        cursor.remove().unwrap();
        assert_eq!(map.state().unwrap().mod_count, i32::MIN);
        assert_eq!(cursor.inner.expected_mod_count, i32::MIN);
        cursor.next_key().unwrap();
        map.clear().unwrap();
        assert!(matches!(
            cursor.next_key(),
            Err(ActiveMapError::ConcurrentModification)
        ));
        let mut empty = map.key_cursor().unwrap();
        map.clear().unwrap();
        assert!(!empty.has_next());
        assert!(matches!(
            empty.next_key(),
            Err(ActiveMapError::ConcurrentModification)
        ));
    }

    #[test]
    fn production_map_and_cursors_are_send_sync_with_send_handles() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Map>();
        assert_send_sync::<KeyCursor<Key, Value>>();
        assert_send_sync::<ValuesCursor<Key, Value>>();
    }
}
