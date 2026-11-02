//! LivingEntity's actual active location-dependent enchantment collection.
//!
//! The source field is Reference2ObjectArrayMap<Enchantment, Set<LocationEffect>>:
//! keys are Enchantment VALUE object references, not registry Holder objects or
//! Enchantment record equality. Values are the actual retained ObjectArraySets
//! allocated lazily by Enchantment.runLocationChangedEffects. Neither array
//! collection hashes keys/effects or owns a registry, actor, RNG or evaluator.
//!
//! This bounded nonnull domain implements the methods used by those source
//! callers. Actual registered values and pure pinned effect equality remain
//! supplied references; no source enchantment/effect object is invented here.

use std::sync::{Arc, Mutex};

/// The SAME canonical Enchantment VALUE object. Clone retains that reference;
/// reference comparison is callback-free. Holder equality, names, numeric IDs,
/// structural record equality and reconstructed metadata are not substitutes.
pub trait EnchantmentValueReference: Clone {
    fn same_object(&self, other: &Self) -> bool;
}

/// One actual nonnull EnchantmentLocationBasedEffect reference. ObjectArraySet
/// tests reference identity first, then STORED.equals(query), scanning backward.
/// java_equals is that actual virtual equality in the pinned callback-free
/// effect-value domain, including nested record/list/value semantics. Custom
/// callback-bearing equals implementations require a separate retained runner.
pub trait LocationEffectReference: Clone {
    fn same_object(&self, other: &Self) -> bool;
    fn java_equals(&self, other: &Self) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocationMapError {
    Poisoned,
    Allocation { capacity: usize },
    NoSuchElement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArrayState {
    pub size: usize,
    pub capacity: usize,
}

fn slots<T>(capacity: usize) -> Result<Vec<Option<T>>, LocationMapError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| LocationMapError::Allocation { capacity })?;
    values.resize_with(capacity, || None);
    Ok(values)
}
fn grown_capacity(size: usize) -> Result<usize, LocationMapError> {
    if size == 0 {
        Ok(2)
    } else {
        size.checked_mul(2).ok_or(LocationMapError::Allocation {
            capacity: usize::MAX,
        })
    }
}

struct ReferenceArrayMap<K, E: LocationEffectReference> {
    keys: Vec<Option<K>>,
    values: Vec<Option<LocationEffectSet<E>>>,
    size: usize,
}
impl<K: EnchantmentValueReference, E: LocationEffectReference> ReferenceArrayMap<K, E> {
    fn find_key(&self, key: &K) -> Option<usize> {
        (0..self.size).rev().find(|index| {
            self.keys[*index]
                .as_ref()
                .expect("used source key slot")
                .same_object(key)
        })
    }
}

/// The actual mutable peractor map object. Clone aliases this SAME map. A new
/// Living constructor calls new directly; no external successful factory is
/// required. Holding a map alone does not fabricate or retain a living actor.
pub struct LocationEnchantments<K: EnchantmentValueReference, E: LocationEffectReference>(
    Arc<Mutex<ReferenceArrayMap<K, E>>>,
);
impl<K: EnchantmentValueReference, E: LocationEffectReference> Clone
    for LocationEnchantments<K, E>
{
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<K: EnchantmentValueReference, E: LocationEffectReference> Default
    for LocationEnchantments<K, E>
{
    fn default() -> Self {
        Self::new()
    }
}
impl<K: EnchantmentValueReference, E: LocationEffectReference> LocationEnchantments<K, E> {
    /// Original no-argument Reference2ObjectArrayMap: zero size, empty arrays.
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ReferenceArrayMap {
            keys: Vec::new(),
            values: Vec::new(),
            size: 0,
        })))
    }
    pub fn same_object(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn state(&self) -> Result<ArrayState, LocationMapError> {
        let state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        Ok(ArrayState {
            size: state.size,
            capacity: state.keys.len(),
        })
    }
    pub fn len(&self) -> Result<usize, LocationMapError> {
        Ok(self.state()?.size)
    }
    pub fn is_empty(&self) -> Result<bool, LocationMapError> {
        Ok(self.len()? == 0)
    }
    pub fn get(&self, key: &K) -> Result<Option<LocationEffectSet<E>>, LocationMapError> {
        let state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        Ok(state
            .find_key(key)
            .and_then(|index| state.values[index].clone()))
    }
    /// Replacement preserves the existing key object and position. New entries
    /// grow/copy both original arrays, then write key, value, and finally size.
    /// Callers must source-order lazy NEW set -> map.put -> set.add -> callback.
    pub fn put(
        &self,
        key: K,
        value: LocationEffectSet<E>,
    ) -> Result<Option<LocationEffectSet<E>>, LocationMapError> {
        let mut state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        if let Some(index) = state.find_key(&key) {
            return Ok(state.values[index].replace(value));
        }
        if state.size == state.keys.len() {
            let capacity = grown_capacity(state.size)?;
            let mut keys = slots(capacity)?;
            let mut values = slots(capacity)?;
            for index in (0..state.size).rev() {
                keys[index] = state.keys[index].clone();
                values[index] = state.values[index].clone();
            }
            state.keys = keys;
            state.values = values;
        }
        let index = state.size;
        state.keys[index] = Some(key);
        state.values[index] = Some(value);
        state.size += 1;
        Ok(None)
    }
    /// Remove and return the SAME retained set BEFORE any stopped-effect
    /// callback. Later mutation/replacement of this map cannot retarget it.
    pub fn remove(&self, key: &K) -> Result<Option<LocationEffectSet<E>>, LocationMapError> {
        let mut state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        let Some(index) = state.find_key(key) else {
            return Ok(None);
        };
        let old = state.values[index].clone();
        for at in index..state.size - 1 {
            state.keys[at] = state.keys[at + 1].clone();
        }
        for at in index..state.size - 1 {
            state.values[at] = state.values[at + 1].clone();
        }
        state.size -= 1;
        let tail = state.size;
        state.keys[tail] = None;
        state.values[tail] = None;
        Ok(old)
    }
    pub fn clear(&self) -> Result<(), LocationMapError> {
        let mut state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        for index in (0..state.size).rev() {
            state.keys[index] = None;
            state.values[index] = None;
        }
        state.size = 0;
        Ok(())
    }
}

struct ObjectArraySet<E> {
    values: Vec<Option<E>>,
    size: usize,
}
impl<E: LocationEffectReference> ObjectArraySet<E> {
    fn find_key(&self, query: &E) -> Option<usize> {
        (0..self.size).rev().find(|index| {
            let stored = self.values[*index].as_ref().expect("used source set slot");
            stored.same_object(query) || stored.java_equals(query)
        })
    }
}

/// One actual native ObjectArraySet. A captured strong set survives removal of
/// its map entry, as Java's source local does. Its methods finish before any
/// gameplay callback; its equality domain is explicitly callback-free above.
pub struct LocationEffectSet<E: LocationEffectReference>(Arc<Mutex<ObjectArraySet<E>>>);
impl<E: LocationEffectReference> Clone for LocationEffectSet<E> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<E: LocationEffectReference> Default for LocationEffectSet<E> {
    fn default() -> Self {
        Self::new()
    }
}
impl<E: LocationEffectReference> LocationEffectSet<E> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ObjectArraySet {
            values: Vec::new(),
            size: 0,
        })))
    }
    pub fn same_object(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn state(&self) -> Result<ArrayState, LocationMapError> {
        let state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        Ok(ArrayState {
            size: state.size,
            capacity: state.values.len(),
        })
    }
    pub fn len(&self) -> Result<usize, LocationMapError> {
        Ok(self.state()?.size)
    }
    pub fn is_empty(&self) -> Result<bool, LocationMapError> {
        Ok(self.len()? == 0)
    }
    pub fn contains(&self, value: &E) -> Result<bool, LocationMapError> {
        let state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        Ok(state.find_key(value).is_some())
    }
    pub fn add(&self, value: E) -> Result<bool, LocationMapError> {
        let mut state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        if state.find_key(&value).is_some() {
            return Ok(false);
        }
        if state.size == state.values.len() {
            let mut values = slots(grown_capacity(state.size)?)?;
            for index in (0..state.size).rev() {
                values[index] = state.values[index].clone();
            }
            state.values = values;
        }
        let index = state.size;
        // ObjectArraySet.add's post-increment occurs before the array store.
        state.size += 1;
        state.values[index] = Some(value);
        Ok(true)
    }
    pub fn remove(&self, value: &E) -> Result<bool, LocationMapError> {
        let mut state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        let Some(index) = state.find_key(value) else {
            return Ok(false);
        };
        for at in index..state.size - 1 {
            state.values[at] = state.values[at + 1].clone();
        }
        state.size -= 1;
        let tail = state.size;
        state.values[tail] = None;
        Ok(true)
    }
    pub fn clear(&self) -> Result<(), LocationMapError> {
        let mut state = self.0.lock().map_err(|_| LocationMapError::Poisoned)?;
        for index in 0..state.size {
            state.values[index] = None;
        }
        state.size = 0;
        Ok(())
    }
    /// Source ObjectArraySet iterator is live, with no modCount/snapshot. The
    /// cursor retains this exact set; callback reentry may change its next item.
    pub fn cursor(&self) -> LocationEffectCursor<E> {
        LocationEffectCursor {
            retained: self.clone(),
            next: 0,
        }
    }
}

pub struct LocationEffectCursor<E: LocationEffectReference> {
    retained: LocationEffectSet<E>,
    next: usize,
}
impl<E: LocationEffectReference> LocationEffectCursor<E> {
    pub fn has_next(&self) -> Result<bool, LocationMapError> {
        Ok(self.next < self.retained.len()?)
    }
    pub fn next_effect(&mut self) -> Result<E, LocationMapError> {
        let state = self
            .retained
            .0
            .lock()
            .map_err(|_| LocationMapError::Poisoned)?;
        if self.next >= state.size {
            return Err(LocationMapError::NoSuchElement);
        }
        let value = state.values[self.next]
            .as_ref()
            .expect("used source set slot")
            .clone();
        self.next += 1;
        Ok(value)
    }
    pub fn retained_set(&self) -> LocationEffectSet<E> {
        self.retained.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Controlled actual reference/value handles. No registry, actor, hash,
    // source effect evaluator or successful enchantment callback is mocked.
    #[derive(Clone)]
    struct Enchantment(Arc<i32>);
    impl EnchantmentValueReference for Enchantment {
        fn same_object(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    struct EffectValue {
        value: i32,
        observations: Arc<Mutex<Vec<(i32, i32)>>>,
    }
    #[derive(Clone)]
    struct Effect(Arc<EffectValue>);
    impl LocationEffectReference for Effect {
        fn same_object(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
        fn java_equals(&self, other: &Self) -> bool {
            self.0
                .observations
                .lock()
                .unwrap()
                .push((self.0.value, other.0.value));
            self.0.value == other.0.value
        }
    }
    fn effect(value: i32, observations: &Arc<Mutex<Vec<(i32, i32)>>>) -> Effect {
        Effect(Arc::new(EffectValue {
            value,
            observations: Arc::clone(observations),
        }))
    }

    #[test]
    fn canonical_value_identity_map_replacement_growth_shift_and_retained_sets() {
        let map = LocationEnchantments::<Enchantment, Effect>::new();
        let alias = map.clone();
        assert!(map.same_object(&alias));
        assert_eq!(
            map.state().unwrap(),
            ArrayState {
                size: 0,
                capacity: 0
            }
        );
        let first = Enchantment(Arc::new(1));
        let equal_but_distinct = Enchantment(Arc::new(1));
        let third = Enchantment(Arc::new(3));
        let first_set = LocationEffectSet::new();
        let second_set = LocationEffectSet::new();
        assert!(map.put(first.clone(), first_set.clone()).unwrap().is_none());
        assert!(map.get(&equal_but_distinct).unwrap().is_none());
        map.put(equal_but_distinct.clone(), second_set.clone())
            .unwrap();
        assert_eq!(
            map.state().unwrap(),
            ArrayState {
                size: 2,
                capacity: 2
            }
        );
        map.put(third.clone(), LocationEffectSet::new()).unwrap();
        assert_eq!(
            map.state().unwrap(),
            ArrayState {
                size: 3,
                capacity: 4
            }
        );
        let replacement = LocationEffectSet::new();
        assert!(map
            .put(first.clone(), replacement.clone())
            .unwrap()
            .unwrap()
            .same_object(&first_set));
        {
            let state = map.0.lock().unwrap();
            assert!(state.keys[0].as_ref().unwrap().same_object(&first));
            assert!(state.keys[1]
                .as_ref()
                .unwrap()
                .same_object(&equal_but_distinct));
        }
        let removed = alias.remove(&first).unwrap().unwrap();
        assert!(removed.same_object(&replacement));
        assert!(map.get(&first).unwrap().is_none());
        assert!(map
            .get(&equal_but_distinct)
            .unwrap()
            .unwrap()
            .same_object(&second_set));
        let observations = Arc::new(Mutex::new(Vec::new()));
        removed.add(effect(7, &observations)).unwrap();
        assert_eq!(replacement.len().unwrap(), 1);
        assert_eq!(
            map.state().unwrap(),
            ArrayState {
                size: 2,
                capacity: 4
            }
        );
        map.clear().unwrap();
        assert_eq!(
            map.state().unwrap(),
            ArrayState {
                size: 0,
                capacity: 4
            }
        );
        assert_eq!(removed.len().unwrap(), 1);
    }

    #[test]
    fn actual_set_reverse_stored_equals_identity_shortcut_and_live_iteration() {
        let observations = Arc::new(Mutex::new(Vec::new()));
        let set = LocationEffectSet::new();
        let first = effect(1, &observations);
        let second = effect(2, &observations);
        let third = effect(3, &observations);
        set.add(first.clone()).unwrap();
        set.add(second.clone()).unwrap();
        set.add(third.clone()).unwrap();
        observations.lock().unwrap().clear();
        assert!(set.contains(&third).unwrap());
        assert!(observations.lock().unwrap().is_empty());
        assert!(!set.add(effect(2, &observations)).unwrap());
        assert_eq!(*observations.lock().unwrap(), vec![(3, 2), (2, 2)]);
        assert_eq!(
            set.state().unwrap(),
            ArrayState {
                size: 3,
                capacity: 4
            }
        );
        let mut cursor = set.cursor();
        assert!(cursor.next_effect().unwrap().same_object(&first));
        // Iterable.forEach uses this same live iterator: removing current skips
        // the element shifted into its previous slot; later appends are seen.
        set.remove(&first).unwrap();
        assert!(cursor.next_effect().unwrap().same_object(&third));
        let fourth = effect(4, &observations);
        set.add(fourth.clone()).unwrap();
        assert!(cursor.next_effect().unwrap().same_object(&fourth));
        assert!(!cursor.has_next().unwrap());
        assert!(matches!(
            cursor.next_effect(),
            Err(LocationMapError::NoSuchElement)
        ));
        let alias = cursor.retained_set();
        assert!(alias.same_object(&set));
        set.clear().unwrap();
        assert_eq!(
            set.state().unwrap(),
            ArrayState {
                size: 0,
                capacity: 4
            }
        );
        assert!(!cursor.has_next().unwrap());
    }
}
