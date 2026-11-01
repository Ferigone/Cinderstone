//! Insertion-ordered EntityTickList with the original copy-on-write iteration.
//!
//! Values are opaque Copy handles into the host's entity store. Entity state is
//! never cloned. Removing/replacing a key during a pass changes future passes;
//! the current pass still visits the handles captured by its original map.
use std::{
    collections::HashMap,
    convert::Infallible,
    fmt,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
};

#[derive(Clone, Copy, Debug)]
struct Node<H> {
    handle: H,
    previous: Option<i32>,
    next: Option<i32>,
}
#[derive(Debug)]
struct OrderedHandles<H> {
    entries: HashMap<i32, Node<H>>,
    first: Option<i32>,
    last: Option<i32>,
}
impl<H: Copy> OrderedHandles<H> {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            first: None,
            last: None,
        }
    }
    fn copy_from(&mut self, source: &Self) {
        // Retain the passive map allocation when possible, like the original
        // clear+put loop. Copy only linked entries and opaque handles.
        self.entries.clone_from(&source.entries);
        self.first = source.first;
        self.last = source.last;
    }
    fn insert(&mut self, id: i32, handle: H) -> Option<H> {
        if let Some(node) = self.entries.get_mut(&id) {
            return Some(std::mem::replace(&mut node.handle, handle));
        }
        self.entries.insert(
            id,
            Node {
                handle,
                previous: self.last,
                next: None,
            },
        );
        if let Some(last) = self.last {
            self.entries
                .get_mut(&last)
                .expect("linked tail exists")
                .next = Some(id);
        } else {
            self.first = Some(id);
        }
        self.last = Some(id);
        None
    }
    fn remove(&mut self, id: i32) -> Option<H> {
        let node = self.entries.remove(&id)?;
        if let Some(previous) = node.previous {
            self.entries
                .get_mut(&previous)
                .expect("linked previous exists")
                .next = node.next;
        } else {
            self.first = node.next;
        }
        if let Some(next) = node.next {
            self.entries
                .get_mut(&next)
                .expect("linked next exists")
                .previous = node.previous;
        } else {
            self.last = node.previous;
        }
        Some(node.handle)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConcurrentIteration;
impl fmt::Display for ConcurrentIteration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Only one concurrent iteration supported")
    }
}
impl std::error::Error for ConcurrentIteration {}

#[derive(Debug, PartialEq, Eq)]
pub enum TickListError<E> {
    ConcurrentIteration,
    Callback(E),
}
impl<E: fmt::Display> fmt::Display for TickListError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConcurrentIteration => ConcurrentIteration.fmt(f),
            Self::Callback(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TickListError<E> {}

#[derive(Debug)]
pub struct EntityTickList<H: Copy> {
    buffers: [OrderedHandles<H>; 2],
    active: usize,
    iterated: Option<usize>,
}
impl<H: Copy> Default for EntityTickList<H> {
    fn default() -> Self {
        Self::new()
    }
}
impl<H: Copy> EntityTickList<H> {
    pub fn new() -> Self {
        Self {
            buffers: [OrderedHandles::new(), OrderedHandles::new()],
            active: 0,
            iterated: None,
        }
    }
    fn ensure_active_is_not_iterated(&mut self) {
        if self.iterated != Some(self.active) {
            return;
        }
        let (left, right) = self.buffers.split_at_mut(1);
        if self.active == 0 {
            right[0].copy_from(&left[0]);
        } else {
            left[0].copy_from(&right[0]);
        }
        self.active = 1 - self.active;
    }
    /// Key identity is the entity ID read at this call, not handle identity.
    /// Replacing an existing ID retains its original insertion position.
    pub fn add(&mut self, id: i32, handle: H) -> Option<H> {
        self.ensure_active_is_not_iterated();
        self.buffers[self.active].insert(id, handle)
    }
    /// Even removal of an absent key triggers copy-on-write during iteration.
    pub fn remove(&mut self, id: i32) -> Option<H> {
        self.ensure_active_is_not_iterated();
        self.buffers[self.active].remove(id)
    }
    pub fn contains(&self, id: i32) -> bool {
        self.buffers[self.active].entries.contains_key(&id)
    }
    pub fn get(&self, id: i32) -> Option<H> {
        self.buffers[self.active]
            .entries
            .get(&id)
            .map(|node| node.handle)
    }
    pub fn len(&self) -> usize {
        self.buffers[self.active].entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn is_iterating(&self) -> bool {
        self.iterated.is_some()
    }
    pub fn ordered_entries(&self) -> impl Iterator<Item = (i32, H)> + '_ {
        let map = &self.buffers[self.active];
        let mut current = map.first;
        std::iter::from_fn(move || {
            let id = current?;
            let node = &map.entries[&id];
            current = node.next;
            Some((id, node.handle))
        })
    }

    /// Split-phase counterpart of try_for_each for a host that must release
    /// its entity-store borrow between different native entity bodies. Handles
    /// are captured from the actual iterated buffer; add/remove still trigger
    /// the original copy-on-write rule until finish_snapshot_pass is called.
    /// The host must clear the marker in its error/unwind finally boundary.
    pub fn begin_snapshot_pass(&mut self) -> Result<Vec<H>, ConcurrentIteration> {
        if self.iterated.is_some() {
            return Err(ConcurrentIteration);
        }
        let handles = self.ordered_entries().map(|(_, handle)| handle).collect();
        self.iterated = Some(self.active);
        Ok(handles)
    }

    pub fn finish_snapshot_pass(&mut self) {
        self.iterated = None;
    }

    pub fn for_each(
        &mut self,
        mut visitor: impl FnMut(&mut Self, H),
    ) -> Result<(), ConcurrentIteration> {
        match self.try_for_each(|list, handle| {
            visitor(list, handle);
            Ok::<(), Infallible>(())
        }) {
            Ok(()) => Ok(()),
            Err(TickListError::ConcurrentIteration) => Err(ConcurrentIteration),
            Err(TickListError::Callback(never)) => match never {},
        }
    }

    /// Callback errors retain all completed mutations and stop this pass.
    /// The iteration marker is cleared on success, callback error and unwind,
    /// like EntityTickList.forEach's Java finally block. Nested iteration is
    /// rejected even after the first mutation has switched the active map.
    pub fn try_for_each<E>(
        &mut self,
        mut visitor: impl FnMut(&mut Self, H) -> Result<(), E>,
    ) -> Result<(), TickListError<E>> {
        if self.iterated.is_some() {
            return Err(TickListError::ConcurrentIteration);
        }
        let iterated = self.active;
        self.iterated = Some(iterated);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut current = self.buffers[iterated].first;
            while let Some(id) = current {
                let node = &self.buffers[iterated].entries[&id];
                let handle = node.handle;
                current = node.next;
                visitor(self, handle).map_err(TickListError::Callback)?;
            }
            Ok(())
        }));
        self.iterated = None;
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    fn entries(map: &OrderedHandles<i32>, ids: &BTreeMap<i32, i32>) -> Value {
        let mut out = Vec::new();
        let mut next = map.first;
        while let Some(id) = next {
            let node = &map.entries[&id];
            out.push(json!([id, node.handle, ids[&node.handle]]));
            next = node.next;
        }
        Value::Array(out)
    }
    fn state(list: &EntityTickList<i32>, ids: &BTreeMap<i32, i32>) -> Value {
        json!({
            "active": entries(&list.buffers[list.active], ids),
            "passive": entries(&list.buffers[1-list.active], ids),
            "active_map": list.active,
            "iterated_map": list.iterated,
        })
    }
    fn execute(
        list: &mut EntityTickList<i32>,
        ids: &mut BTreeMap<i32, i32>,
        operation: &Value,
    ) -> Value {
        let kind = operation["op"].as_str().unwrap();
        let mut record = json!({"op":kind});
        let label = operation["entity"].as_i64().map(|value| value as i32);
        let mut error: Option<String> = None;
        match kind {
            "add" => {
                let label = label.unwrap();
                list.add(ids[&label], label);
            }
            "remove" => {
                list.remove(ids[&label.unwrap()]);
            }
            "contains" => {
                record["result"] = json!(list.contains(ids[&label.unwrap()]));
            }
            "id" => {
                ids.insert(label.unwrap(), operation["value"].as_i64().unwrap() as i32);
            }
            "throw" => error = Some("ProbeFailure: controlled".to_owned()),
            "iterate" => {
                let mut visits = Vec::new();
                let mut callbacks = Vec::new();
                let result = list.try_for_each(|list, handle| {
                    let visit = visits.len();
                    visits.push(json!([handle, ids[&handle]]));
                    if let Some(programs) = operation["on_visit"].as_array() {
                        for program in programs {
                            if program["at"].as_u64().unwrap() as usize != visit {
                                continue;
                            }
                            let mut callback = json!({"at":visit,"effects":[]});
                            for action in program["actions"].as_array().unwrap() {
                                let result = execute(list, ids, action);
                                let failure = result["error"].as_str().map(str::to_owned);
                                callback["effects"].as_array_mut().unwrap().push(result);
                                if action["propagate"].as_bool() == Some(true) {
                                    if let Some(failure) = failure {
                                        callbacks.push(callback);
                                        return Err(failure);
                                    }
                                }
                            }
                            callbacks.push(callback);
                        }
                    }
                    Ok(())
                });
                error = match result {
                    Ok(()) => None,
                    Err(TickListError::ConcurrentIteration) => Some(
                        "UnsupportedOperationException: Only one concurrent iteration supported"
                            .to_owned(),
                    ),
                    Err(TickListError::Callback(message)) => Some(message),
                };
                record["visits"] = json!(visits);
                record["callbacks"] = json!(callbacks);
            }
            other => panic!("unknown original EntityTickList oracle operation {other}"),
        }
        record["error"] = json!(error);
        record["state"] = state(list, ids);
        record
    }

    #[test]
    fn actual_java_order_identity_copy_on_write_reentrancy_and_callback_errors() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/entity_tick_vectors.json")).unwrap();
        assert_eq!(
            fixture["oracle_inner_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        for scene in fixture["scenes"].as_array().unwrap() {
            let query = &scene["query"];
            let mut list = EntityTickList::new();
            let mut ids = query["entities"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| {
                    (
                        entry[0].as_i64().unwrap() as i32,
                        entry[1].as_i64().unwrap() as i32,
                    )
                })
                .collect();
            for (index, (operation, expected)) in query["operations"]
                .as_array()
                .unwrap()
                .iter()
                .zip(scene["expected"].as_array().unwrap())
                .enumerate()
            {
                let actual = execute(&mut list, &mut ids, operation);
                assert_eq!(
                    actual, *expected,
                    "scene {}, operation{index}: {operation}",
                    query["name"]
                );
                assert!(!list.is_iterating());
                assert_eq!(list.len(), list.ordered_entries().count());
                for (id, handle) in list.ordered_entries() {
                    assert!(list.contains(id));
                    assert_eq!(list.get(id), Some(handle));
                }
            }
        }
    }

    #[test]
    fn rust_unwind_clears_iteration_marker_and_retains_completed_mutations() {
        let mut list = EntityTickList::new();
        list.add(7, 700_u64);
        list.add(-1, 900);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = list.for_each(|list, _| {
                list.remove(-1);
                list.add(0, 1200);
                panic!("controlled Rust callback unwind");
            });
        }));
        assert!(result.is_err());
        assert!(!list.is_iterating());
        assert_eq!(
            list.ordered_entries().collect::<Vec<_>>(),
            [(7, 700), (0, 1200)]
        );
        let mut next = Vec::new();
        list.for_each(|_, handle| next.push(handle)).unwrap();
        assert_eq!(next, [700, 1200]);
    }
}
