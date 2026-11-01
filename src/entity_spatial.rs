//! Ordered EntitySectionStorage/EntitySection queries over opaque Copy handles.
//!
//! The host supplies immutable class membership and current entity equality,
//! bounds and type-test results. Cached class lists retain the original
//! ClassInstanceMultiMap behavior, including duplicate values and ID aliases.
use crate::geometry::{Aabb, Vec3};
use anyhow::{ensure, Result};
use std::collections::BTreeMap;

pub type ClassKey = u32;
pub type SectionKey = i64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Hidden,
    Tracked,
    Ticking,
}
impl Visibility {
    pub fn accessible(self) -> bool {
        self != Self::Hidden
    }
    pub fn ticking(self) -> bool {
        self == Self::Ticking
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Continuation {
    Continue,
    Abort,
}

pub fn section_key(x: i32, y: i32, z: i32) -> SectionKey {
    (((x as u64 & 0x3f_ffff) << 42) | ((z as u64 & 0x3f_ffff) << 20) | (y as u64 & 0x0f_ffff))
        as i64
}
pub fn section_coordinates(key: SectionKey) -> (i32, i32, i32) {
    (
        (key >> 42) as i32,
        ((key << 44) >> 44) as i32,
        ((key << 22) >> 42) as i32,
    )
}
pub fn section_at(position: Vec3) -> SectionKey {
    section_key(
        section_coordinate(position.x),
        section_coordinate(position.y),
        section_coordinate(position.z),
    )
}
fn section_coordinate(value: f64) -> i32 {
    let truncated = value as i32;
    let floor = if value < f64::from(truncated) {
        truncated.wrapping_sub(1)
    } else {
        truncated
    };
    floor >> 4
}

#[derive(Debug)]
pub struct EntitySection<H: Copy> {
    base_class: ClassKey,
    visibility: Visibility,
    all: Vec<H>,
    cached: BTreeMap<ClassKey, Vec<H>>,
}
impl<H: Copy> EntitySection<H> {
    pub fn new(base_class: ClassKey, visibility: Visibility) -> Self {
        Self {
            base_class,
            visibility,
            all: Vec::new(),
            cached: BTreeMap::new(),
        }
    }
    pub fn visibility(&self) -> Visibility {
        self.visibility
    }
    pub fn set_visibility(&mut self, value: Visibility) -> Visibility {
        std::mem::replace(&mut self.visibility, value)
    }
    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }
    pub fn len(&self) -> usize {
        self.all.len()
    }
    pub fn handles(&self) -> &[H] {
        &self.all
    }
    /// Class checks are JVM-style immutable isInstance facts supplied by the
    /// host. Every applicable cached list appends the value, even a duplicate.
    pub fn add(&mut self, handle: H, mut is_instance: impl FnMut(ClassKey, H) -> bool) {
        if is_instance(self.base_class, handle) {
            self.all.push(handle);
        }
        for (&class, values) in &mut self.cached {
            if is_instance(class, handle) {
                values.push(handle);
            }
        }
    }
    /// ArrayList.remove removes its first equals-match independently from each
    /// applicable class list. Entity.equals compares current IDs, not handles.
    pub fn remove(
        &mut self,
        target: H,
        mut is_instance: impl FnMut(ClassKey, H) -> bool,
        mut equals: impl FnMut(H, H) -> bool,
    ) -> bool {
        let mut removed = false;
        if is_instance(self.base_class, target) {
            removed |= remove_first(&mut self.all, |candidate| equals(target, candidate));
        }
        for (&class, values) in &mut self.cached {
            if is_instance(class, target) {
                removed |= remove_first(values, |candidate| equals(target, candidate));
            }
        }
        removed
    }
    fn values_for(
        &mut self,
        class: Option<ClassKey>,
        is_instance: &mut impl FnMut(ClassKey, H) -> bool,
    ) -> &[H] {
        let Some(class) = class.filter(|class| *class != self.base_class) else {
            return &self.all;
        };
        let all = &self.all;
        self.cached.entry(class).or_insert_with(|| {
            all.iter()
                .copied()
                .filter(|handle| is_instance(class, *handle))
                .collect()
        })
    }
}
fn remove_first<H: Copy>(values: &mut Vec<H>, mut matches: impl FnMut(H) -> bool) -> bool {
    if let Some(index) = values.iter().position(|value| matches(*value)) {
        values.remove(index);
        true
    } else {
        false
    }
}

#[derive(Debug)]
pub struct EntitySectionStorage<H: Copy> {
    base_class: ClassKey,
    sections: BTreeMap<SectionKey, EntitySection<H>>,
}
impl<H: Copy> EntitySectionStorage<H> {
    pub fn new(base_class: ClassKey) -> Self {
        Self {
            base_class,
            sections: BTreeMap::new(),
        }
    }
    pub fn len(&self) -> usize {
        self.sections.len()
    }
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
    pub fn section(&self, key: SectionKey) -> Option<&EntitySection<H>> {
        self.sections.get(&key)
    }
    pub fn section_mut(&mut self, key: SectionKey) -> Option<&mut EntitySection<H>> {
        self.sections.get_mut(&key)
    }
    /// Initial visibility is queried once for a newly created section. The
    /// callback receives its decoded chunk X/Z, not its section Y.
    pub fn get_or_create_section(
        &mut self,
        key: SectionKey,
        initial_visibility: impl FnOnce((i32, i32)) -> Visibility,
    ) -> &mut EntitySection<H> {
        let base_class = self.base_class;
        self.sections.entry(key).or_insert_with(|| {
            let (x, _, z) = section_coordinates(key);
            EntitySection::new(base_class, initial_visibility((x, z)))
        })
    }
    pub fn remove_section(&mut self, key: SectionKey) -> Option<EntitySection<H>> {
        self.sections.remove(&key)
    }
    pub fn remove_section_if_empty(&mut self, key: SectionKey) -> bool {
        if self.sections.get(&key).is_some_and(EntitySection::is_empty) {
            self.sections.remove(&key);
            true
        } else {
            false
        }
    }
    /// Empty and hidden sections still belong to this stream. Packed Y order
    /// puts nonnegative Y before negative Y, matching the original signed set.
    pub fn chunk_sections(&self, x: i32, z: i32) -> Result<Vec<SectionKey>> {
        let start = section_key(x, 0, z);
        let end = section_key(x, -1, z).wrapping_add(1);
        ensure!(start <= end, "invalid signed section interval");
        Ok(self
            .sections
            .range(start..end)
            .map(|(&key, _)| key)
            .collect())
    }

    /// Valid class keys represent subclasses of the storage's base class.
    /// inspect supplies the EntityTypeTest.tryCast result (None rejects it) and
    /// the original entity's current bounds. Collection itself clones no entity.
    pub fn query(
        &mut self,
        bounds: Aabb,
        class: Option<ClassKey>,
        is_instance: impl FnMut(ClassKey, H) -> bool,
        inspect: impl FnMut(H) -> Result<Option<Aabb>>,
    ) -> Result<Vec<H>> {
        let mut handles = Vec::new();
        self.query_while(bounds, class, is_instance, inspect, |handle| {
            handles.push(handle);
            Ok(Continuation::Continue)
        })?;
        Ok(handles)
    }
    pub fn query_while(
        &mut self,
        bounds: Aabb,
        class: Option<ClassKey>,
        is_instance: impl FnMut(ClassKey, H) -> bool,
        inspect: impl FnMut(H) -> Result<Option<Aabb>>,
        consume: impl FnMut(H) -> Result<Continuation>,
    ) -> Result<Continuation> {
        self.query_impl(bounds, class, is_instance, inspect, |_| Ok(()), consume)
    }
    fn query_impl(
        &mut self,
        bounds: Aabb,
        class: Option<ClassKey>,
        mut is_instance: impl FnMut(ClassKey, H) -> bool,
        mut inspect: impl FnMut(H) -> Result<Option<Aabb>>,
        mut visit_section: impl FnMut(SectionKey) -> Result<()>,
        mut consume: impl FnMut(H) -> Result<Continuation>,
    ) -> Result<Continuation> {
        ensure!(
            [
                bounds.min_x,
                bounds.min_y,
                bounds.min_z,
                bounds.max_x,
                bounds.max_y,
                bounds.max_z
            ]
            .iter()
            .all(|value| value.is_finite()),
            "nonfinite spatial query bounds"
        );
        let min_x = section_coordinate(bounds.min_x - 2.);
        let max_x = section_coordinate(bounds.max_x + 2.);
        let min_y = section_coordinate(bounds.min_y - 4.);
        let max_y = section_coordinate(bounds.max_y + 0.);
        let min_z = section_coordinate(bounds.min_z - 2.);
        let max_z = section_coordinate(bounds.max_z + 2.);
        for x in min_x..=max_x {
            let start = section_key(x, 0, 0);
            let end = section_key(x, -1, -1).wrapping_add(1);
            ensure!(start <= end, "invalid signed section interval");
            for (&key, section) in self.sections.range_mut(start..end) {
                let (_, y, z) = section_coordinates(key);
                if y < min_y
                    || y > max_y
                    || z < min_z
                    || z > max_z
                    || section.is_empty()
                    || !section.visibility.accessible()
                {
                    continue;
                }
                visit_section(key)?;
                for &handle in section.values_for(class, &mut is_instance) {
                    if let Some(entity_bounds) = inspect(handle)? {
                        if entity_bounds.intersects(&bounds)
                            && consume(handle)? == Continuation::Abort
                        {
                            return Ok(Continuation::Abort);
                        }
                    }
                }
            }
        }
        Ok(Continuation::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map, Value};

    struct Entity {
        id: i32,
        kind: u32,
        bounds: Aabb,
    }
    struct Environment {
        entities: BTreeMap<i32, Entity>,
        visibility: Visibility,
        chunks: BTreeMap<(i32, i32), Visibility>,
    }
    impl Environment {
        fn is_instance(&self, class: ClassKey, handle: i32) -> bool {
            let kind = self.entities[&handle].kind;
            match class {
                0 => true,
                1 => kind == 1,
                2..=4 => kind == 2,
                _ => panic!("unknown oracle class"),
            }
        }
        fn equals(&self, target: i32, candidate: i32) -> bool {
            self.entities[&target].id == self.entities[&candidate].id
        }
        fn initial_visibility(&self, chunk: (i32, i32)) -> Visibility {
            self.chunks.get(&chunk).copied().unwrap_or(self.visibility)
        }
    }
    fn visibility(id: u64) -> Visibility {
        match id {
            0 => Visibility::Hidden,
            1 => Visibility::Tracked,
            2 => Visibility::Ticking,
            _ => panic!("unknown visibility"),
        }
    }
    fn visibility_id(value: Visibility) -> u8 {
        match value {
            Visibility::Hidden => 0,
            Visibility::Tracked => 1,
            Visibility::Ticking => 2,
        }
    }
    fn double(value: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
    }
    fn bounds(value: &Value) -> Aabb {
        Aabb::new(
            double(&value[0]),
            double(&value[1]),
            double(&value[2]),
            double(&value[3]),
            double(&value[4]),
            double(&value[5]),
        )
    }
    fn key(value: &Value) -> SectionKey {
        section_key(
            value[0].as_i64().unwrap() as i32,
            value[1].as_i64().unwrap() as i32,
            value[2].as_i64().unwrap() as i32,
        )
    }
    fn snapshot(storage: &EntitySectionStorage<i32>) -> Value {
        json!(storage.sections.iter().map(|(&key,section)| {
            let mut cached = Map::new();cached.insert(section.base_class.to_string(),json!(section.all));
            for (class,values) in &section.cached {cached.insert(class.to_string(),json!(values));}
            json!({"key":key.to_string(),"section":section_coordinates(key),"visibility":visibility_id(section.visibility),"all":section.all,"cached":cached})
        }).collect::<Vec<_>>())
    }
    fn execute(
        storage: &mut EntitySectionStorage<i32>,
        environment: &mut Environment,
        op: &Value,
    ) -> Value {
        let handle = op["entity"].as_i64().map(|value| value as i32);
        let result = match op["op"].as_str().unwrap() {
            "position_key" => {
                let p = &op["position"];
                let key = section_at(Vec3::new(double(&p[0]), double(&p[1]), double(&p[2])));
                json!({"key":key.to_string(),"section":section_coordinates(key)})
            }
            "create_section" => {
                let key = key(&op["section"]);
                let created = storage.section(key).is_none();
                storage.get_or_create_section(key, |chunk| environment.initial_visibility(chunk));
                json!(created)
            }
            "add" => {
                storage
                    .get_or_create_section(key(&op["section"]), |chunk| {
                        environment.initial_visibility(chunk)
                    })
                    .add(handle.unwrap(), |class, handle| {
                        environment.is_instance(class, handle)
                    });
                Value::Null
            }
            "remove" => json!(storage
                .section_mut(key(&op["section"]))
                .is_some_and(|section| section.remove(
                    handle.unwrap(),
                    |class, handle| environment.is_instance(class, handle),
                    |a, b| environment.equals(a, b)
                ))),
            "remove_section" => json!(storage.remove_section(key(&op["section"])).is_some()),
            "visibility" => json!(visibility_id(
                storage
                    .get_or_create_section(key(&op["section"]), |chunk| environment
                        .initial_visibility(chunk))
                    .set_visibility(visibility(op["visibility"].as_u64().unwrap()))
            )),
            "bounds" => {
                environment
                    .entities
                    .get_mut(&handle.unwrap())
                    .unwrap()
                    .bounds = bounds(&op["bounds"]);
                Value::Null
            }
            "id" => {
                environment.entities.get_mut(&handle.unwrap()).unwrap().id =
                    op["id"].as_i64().unwrap() as i32;
                Value::Null
            }
            "move" => {
                let from = key(&op["from"]);
                let to = key(&op["to"]);
                let handle = handle.unwrap();
                let removed = storage.section_mut(from).is_some_and(|section| {
                    section.remove(
                        handle,
                        |class, handle| environment.is_instance(class, handle),
                        |a, b| environment.equals(a, b),
                    )
                });
                storage
                    .get_or_create_section(to, |chunk| environment.initial_visibility(chunk))
                    .add(handle, |class, handle| {
                        environment.is_instance(class, handle)
                    });
                if op["remove_empty"].as_bool() == Some(true) {
                    storage.remove_section_if_empty(from);
                }
                json!(removed)
            }
            "chunk_sections" => json!(storage
                .chunk_sections(
                    op["chunk"][0].as_i64().unwrap() as i32,
                    op["chunk"][1].as_i64().unwrap() as i32
                )
                .unwrap()
                .into_iter()
                .map(|key| key.to_string())
                .collect::<Vec<_>>()),
            "query" => {
                let class = op["class_key"].as_u64().map(|value| value as ClassKey);
                let exact = op["exact"].as_bool().unwrap_or(false);
                let limit = op["abort_after"].as_u64().unwrap_or(0) as usize;
                let mut handles = Vec::new();
                let mut sections = Vec::new();
                let continuation = storage
                    .query_impl(
                        bounds(&op["bounds"]),
                        class,
                        |class, handle| environment.is_instance(class, handle),
                        |handle| {
                            let entity = &environment.entities[&handle];
                            let cast = class.is_none_or(|class| {
                                if exact {
                                    class == entity.kind
                                } else {
                                    environment.is_instance(class, handle)
                                }
                            });
                            Ok(cast.then_some(entity.bounds))
                        },
                        |key| {
                            sections.push(key.to_string());
                            Ok(())
                        },
                        |handle| {
                            handles.push(handle);
                            Ok(if limit > 0 && handles.len() >= limit {
                                Continuation::Abort
                            } else {
                                Continuation::Continue
                            })
                        },
                    )
                    .unwrap();
                json!({"handles":handles,"sections":sections,"aborted":continuation==Continuation::Abort})
            }
            other => panic!("unknown original spatial operation {other}"),
        };
        json!({"result":result,"sections":snapshot(storage)})
    }

    #[test]
    fn actual_java_sections_class_caches_order_visibility_moves_and_queries() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/entity_spatial_vectors.json")).unwrap();
        for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
            let scene = &case["scene"];
            let mut environment = Environment {
                entities: scene["entities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| {
                        (
                            value["handle"].as_i64().unwrap() as i32,
                            Entity {
                                id: value["id"].as_i64().unwrap() as i32,
                                kind: value["kind"].as_u64().unwrap() as u32,
                                bounds: bounds(&value["bounds"]),
                            },
                        )
                    })
                    .collect(),
                visibility: visibility(scene["initial_visibility"].as_u64().unwrap()),
                chunks: scene["chunk_visibility"]
                    .as_array()
                    .map(|values| {
                        values
                            .iter()
                            .map(|value| {
                                (
                                    (
                                        value[0].as_i64().unwrap() as i32,
                                        value[1].as_i64().unwrap() as i32,
                                    ),
                                    visibility(value[2].as_u64().unwrap()),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            let mut storage = EntitySectionStorage::new(0);
            for (step, (operation, expected)) in scene["operations"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["expected"].as_array().unwrap())
                .enumerate()
            {
                let actual = execute(&mut storage, &mut environment, operation);
                assert_eq!(
                    actual, *expected,
                    "spatial scene{index},step{step},operation{operation}"
                );
                assert_eq!(storage.len(), actual["sections"].as_array().unwrap().len());
            }
        }
    }
}
