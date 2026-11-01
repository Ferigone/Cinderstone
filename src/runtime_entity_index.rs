//! Runtime handles and immediate section updates over the verified spatial index.
//!
//! Physical bounds stay in the host's authoritative entity store. This layer
//! never copies them into a second cache. Entity IDs are immutable while a
//! handle is registered; generations prevent cleanup of an old incarnation
//! from deleting its replacement. Ticket and ticking callbacks remain explicit.
use crate::{
    entity_spatial::{self, EntitySectionStorage, SectionKey, Visibility},
    geometry::{Aabb, Vec3},
};
use anyhow::{ensure, Context, Result};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityHandle {
    pub id: i32,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Player,
    Item,
    ExperienceOrb,
}
impl EntityKind {
    fn class(self) -> u32 {
        match self {
            Self::Player => 1,
            Self::Item => 2,
            Self::ExperienceOrb => 3,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct IndexedEntity {
    pub handle: EntityHandle,
    pub kind: EntityKind,
    pub section: SectionKey,
    pub always_ticking: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct SectionTransition {
    pub entity: IndexedEntity,
    pub previous: Visibility,
    pub current: Visibility,
}
impl SectionTransition {
    pub fn was_ticking(self) -> bool {
        self.entity.always_ticking || self.previous.ticking()
    }
    pub fn is_ticking(self) -> bool {
        self.entity.always_ticking || self.current.ticking()
    }
}

pub struct RuntimeEntityIndex {
    records: BTreeMap<i32, IndexedEntity>,
    spatial: EntitySectionStorage<EntityHandle>,
    next_generation: u64,
}
impl Default for RuntimeEntityIndex {
    fn default() -> Self {
        Self::new()
    }
}
impl RuntimeEntityIndex {
    pub fn new() -> Self {
        Self {
            records: BTreeMap::new(),
            spatial: EntitySectionStorage::new(0),
            next_generation: 1,
        }
    }
    pub fn get(&self, handle: EntityHandle) -> Option<IndexedEntity> {
        self.records
            .get(&handle.id)
            .copied()
            .filter(|entry| entry.handle == handle)
    }
    pub fn by_id(&self, id: i32) -> Option<IndexedEntity> {
        self.records.get(&id).copied()
    }
    pub fn entries(&self) -> impl Iterator<Item = IndexedEntity> + '_ {
        self.records.values().copied()
    }
    /// A transaction-local snapshot of actual handles, generations, section
    /// visibility and insertion order. Physical state is still supplied by the
    /// preparing host. This is not a second live index or a sorted-ID rebuild.
    pub fn snapshot_for_preparation(&self) -> Result<Self> {
        let records = self.records.clone();
        let mut spatial = EntitySectionStorage::new(0);
        let sections: std::collections::BTreeSet<_> =
            records.values().map(|entry| entry.section).collect();
        for key in sections {
            let source = self
                .spatial
                .section(key)
                .context("indexed entity section is missing")?;
            let target = spatial.get_or_create_section(key, |_| source.visibility());
            for &handle in source.handles() {
                ensure!(
                    records
                        .get(&handle.id)
                        .is_some_and(|entry| entry.handle == handle && entry.section == key),
                    "stale section handle in index preparation"
                );
                target.add(handle, |class, h| is_instance(&records, class, h));
            }
        }
        Ok(Self {
            records,
            spatial,
            next_generation: self.next_generation,
        })
    }
    /// Validate an exclusive owner's whole insertion batch before committing
    /// inventory or RNG changes. This does not reserve or publish identities.
    pub fn validate_registration_batch(&self, ids: impl IntoIterator<Item = i32>) -> Result<()> {
        let mut unique = std::collections::BTreeSet::new();
        for id in ids {
            ensure!(
                !self.records.contains_key(&id),
                "entity ID is already indexed"
            );
            ensure!(unique.insert(id), "duplicate entity ID in insertion batch");
        }
        self.next_generation
            .checked_add(u64::try_from(unique.len())?)
            .context("runtime entity generation exhausted")?;
        Ok(())
    }
    pub fn register(
        &mut self,
        id: i32,
        kind: EntityKind,
        position: Vec3,
        always_ticking: bool,
        initial_visibility: impl FnOnce((i32, i32)) -> Visibility,
    ) -> Result<EntityHandle> {
        ensure!(
            !self.records.contains_key(&id),
            "entity ID is already indexed"
        );
        let generation = self.next_generation;
        self.next_generation = generation
            .checked_add(1)
            .context("runtime entity generation exhausted")?;
        let handle = EntityHandle { id, generation };
        let section = entity_spatial::section_at(position);
        self.records.insert(
            id,
            IndexedEntity {
                handle,
                kind,
                section,
                always_ticking,
            },
        );
        let records = &self.records;
        self.spatial
            .get_or_create_section(section, initial_visibility)
            .add(handle, |class, h| is_instance(records, class, h));
        Ok(handle)
    }
    pub fn remove(&mut self, handle: EntityHandle) -> bool {
        let Some(entry) = self.get(handle) else {
            return false;
        };
        let records = &self.records;
        if let Some(section) = self.spatial.section_mut(entry.section) {
            section.remove(
                handle,
                |class, h| is_instance(records, class, h),
                |a, b| a.id == b.id,
            );
        }
        self.spatial.remove_section_if_empty(entry.section);
        self.records.remove(&handle.id);
        true
    }
    /// Call synchronously at each setPosRaw/onMove boundary, including a later
    /// rollback. Same-section motion retains its ClassInstanceMultiMap order.
    /// A removed entity has the original null level callback: no index update.
    pub fn position_changed(
        &mut self,
        handle: EntityHandle,
        position: Vec3,
        initial_visibility: impl FnOnce((i32, i32)) -> Visibility,
    ) -> Result<Option<SectionTransition>> {
        let Some(mut entry) = self.get(handle) else {
            return Ok(None);
        };
        let next = entity_spatial::section_at(position);
        if next == entry.section {
            return Ok(None);
        }
        let records = &self.records;
        let old = self
            .spatial
            .section_mut(entry.section)
            .context("indexed entity has no section")?;
        let previous = old.visibility();
        old.remove(
            handle,
            |class, h| is_instance(records, class, h),
            |a, b| a.id == b.id,
        );
        let current = {
            let section = self.spatial.get_or_create_section(next, initial_visibility);
            section.add(handle, |class, h| is_instance(records, class, h));
            section.visibility()
        };
        self.spatial.remove_section_if_empty(entry.section);
        entry.section = next;
        self.records.insert(handle.id, entry);
        Ok(Some(SectionTransition {
            entity: entry,
            previous,
            current,
        }))
    }
    pub fn visibility(&self, handle: EntityHandle) -> Option<Visibility> {
        let entry = self.get(handle)?;
        self.spatial
            .section(entry.section)
            .map(|section| section.visibility())
    }
    /// Return transitions in the storage's signed section order and each
    /// section's insertion order. The host applies its real ticket callbacks.
    pub fn set_chunk_visibility(
        &mut self,
        chunk: (i32, i32),
        visibility: Visibility,
    ) -> Result<Vec<SectionTransition>> {
        let mut changes = Vec::new();
        for key in self.spatial.chunk_sections(chunk.0, chunk.1)? {
            let section = self.spatial.section_mut(key).expect("enumerated section");
            let previous = section.set_visibility(visibility);
            if previous != visibility {
                changes.extend(
                    section
                        .handles()
                        .iter()
                        .filter_map(|handle| self.records.get(&handle.id).copied())
                        .map(|entity| SectionTransition {
                            entity,
                            previous,
                            current: visibility,
                        }),
                );
            }
        }
        Ok(changes)
    }
    pub fn query(
        &mut self,
        bounds: Aabb,
        kind: Option<EntityKind>,
        mut inspect: impl FnMut(EntityHandle) -> Result<Option<Aabb>>,
    ) -> Result<Vec<EntityHandle>> {
        self.query_with_kind(bounds, kind, |handle, _kind| inspect(handle))
    }
    /// Query the same section/class storage while exposing the registered
    /// entity class to its authoritative bounds owner. This avoids a second
    /// kind map when a host owns more than one non-player entity store.
    pub fn query_with_kind(
        &mut self,
        bounds: Aabb,
        kind: Option<EntityKind>,
        mut inspect: impl FnMut(EntityHandle, EntityKind) -> Result<Option<Aabb>>,
    ) -> Result<Vec<EntityHandle>> {
        let records = &self.records;
        self.spatial.query(
            bounds,
            kind.map(EntityKind::class),
            |class, h| is_instance(records, class, h),
            |handle| {
                if let Some(entry) = records
                    .get(&handle.id)
                    .filter(|entry| entry.handle == handle)
                {
                    inspect(handle, entry.kind)
                } else {
                    Ok(None)
                }
            },
        )
    }
}
fn is_instance(records: &BTreeMap<i32, IndexedEntity>, class: u32, handle: EntityHandle) -> bool {
    records
        .get(&handle.id)
        .is_some_and(|entry| entry.handle == handle && (class == 0 || class == entry.kind.class()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immediate_crossings_change_query_order_but_same_section_motion_does_not() {
        let mut index = RuntimeEntityIndex::new();
        let a = index
            .register(1, EntityKind::Item, Vec3::new(1., 64., 1.), false, |_| {
                Visibility::Ticking
            })
            .unwrap();
        let b = index
            .register(2, EntityKind::Item, Vec3::new(2., 64., 1.), false, |_| {
                Visibility::Ticking
            })
            .unwrap();
        let bounds = Aabb::new(0., 63., 0., 4., 66., 4.);
        let query = |index: &mut RuntimeEntityIndex| {
            index
                .query(bounds, Some(EntityKind::Item), |_| Ok(Some(bounds)))
                .unwrap()
        };
        assert_eq!(query(&mut index), vec![a, b]);
        index
            .position_changed(a, Vec3::new(3., 64., 1.), |_| Visibility::Ticking)
            .unwrap();
        assert_eq!(query(&mut index), vec![a, b]);
        index
            .position_changed(a, Vec3::new(17., 64., 1.), |_| Visibility::Ticking)
            .unwrap();
        index
            .position_changed(a, Vec3::new(1., 64., 1.), |_| Visibility::Ticking)
            .unwrap();
        assert_eq!(query(&mut index), vec![b, a]);
    }
    #[test]
    fn old_generation_cleanup_cannot_remove_replacement_and_visibility_is_explicit() {
        let mut index = RuntimeEntityIndex::new();
        let old = index
            .register(1, EntityKind::Player, Vec3::new(1., 64., 1.), true, |_| {
                Visibility::Hidden
            })
            .unwrap();
        assert!(index.remove(old));
        let new = index
            .register(1, EntityKind::Item, Vec3::new(1., 64., 1.), false, |_| {
                Visibility::Tracked
            })
            .unwrap();
        assert!(!index.remove(old));
        assert!(index.get(new).is_some());
        let transition = index
            .set_chunk_visibility((0, 0), Visibility::Ticking)
            .unwrap();
        assert_eq!(transition.len(), 1);
        assert!(!transition[0].was_ticking());
        assert!(transition[0].is_ticking());
    }
}
