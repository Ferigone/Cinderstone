//! `CommandStorage` (`/data ... storage`): one `data/command_storage_<namespace>.dat` per namespace,
//! each a map from path to compound.

use super::java_collections::{to_java_order, JavaHashMap};
use super::storage::{DataFixType, DimensionDataStorage, Factory, SavedData, SavedDataError};
use crate::components::java::resource_location;
use crate::nbt::{JavaString, Tag};
use std::any::Any;

const ID_PREFIX: &str = "command_storage_";

fn empty_compound() -> Tag {
    Tag::Compound(Vec::new())
}

fn is_empty_compound(tag: &Tag) -> bool {
    matches!(tag, Tag::Compound(entries) if entries.is_empty())
}

/// `CommandStorage.Container`: the compounds of one namespace, in a `HashMap` keyed by path.
#[derive(Debug, Clone, Default)]
pub struct CommandStorageContainer {
    storage: JavaHashMap<JavaString, Tag>,
    dirty: bool,
}

impl CommandStorageContainer {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Container.load`: every member of `contents` goes in as a compound (a member of another type
    /// becomes an empty compound), in the iteration order of the `contents` compound.
    pub fn load(data: &Tag) -> Self {
        let mut container = Self::new();
        if let Some(Tag::Compound(entries)) = data.get("contents") {
            let mut decoded: JavaHashMap<JavaString, Tag> = JavaHashMap::default();
            for (name, value) in entries {
                decoded.put(name.clone(), value.clone());
            }
            for (name, value) in decoded.iter_ordered() {
                let compound = match value {
                    Tag::Compound(_) => to_java_order(value),
                    _ => empty_compound(),
                };
                container.storage.put(name.clone(), compound);
            }
        }
        container
    }

    /// `Container.get`: the stored compound, or an empty one.
    pub fn get(&self, path: &JavaString) -> Tag {
        self.storage
            .get(path)
            .cloned()
            .unwrap_or_else(empty_compound)
    }

    /// `Container.put`: an empty compound removes the entry. Always marks the data dirty.
    pub fn put(&mut self, path: JavaString, tag: Tag) {
        if is_empty_compound(&tag) {
            self.storage.remove(&path);
        } else {
            self.storage.put(path, tag);
        }
        self.dirty = true;
    }

    /// The paths in `HashMap` order.
    pub fn paths(&self) -> Vec<JavaString> {
        self.storage
            .iter_ordered()
            .into_iter()
            .map(|(path, _)| path.clone())
            .collect()
    }
}

impl SavedData for CommandStorageContainer {
    /// `Container.save`: a fresh `CompoundTag` is filled in the storage's iteration order, so its
    /// own `HashMap` (sized for the entries, not for the removed ones) decides the written order.
    fn save(&self) -> Tag {
        let mut fresh: JavaHashMap<JavaString, Tag> = JavaHashMap::default();
        for (path, tag) in self.storage.iter_ordered() {
            fresh.put(path.clone(), tag.clone());
        }
        let contents = fresh
            .iter_ordered()
            .into_iter()
            .map(|(path, tag)| (path.clone(), tag.clone()))
            .collect();
        Tag::Compound(vec![(
            JavaString::from("contents"),
            Tag::Compound(contents),
        )])
    }
    fn is_dirty(&self) -> bool {
        self.dirty
    }
    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn factory() -> Factory<CommandStorageContainer> {
    Factory {
        constructor: Box::new(CommandStorageContainer::new),
        deserializer: Box::new(|data| Ok(CommandStorageContainer::load(data))),
        fix: DataFixType::SavedDataCommandStorage,
    }
}

fn split(id: &str) -> Result<(String, JavaString), SavedDataError> {
    let canonical =
        resource_location(id).ok_or_else(|| SavedDataError::InvalidName(id.to_owned()))?;
    let (namespace, path) = canonical
        .split_once(':')
        .ok_or_else(|| SavedDataError::InvalidName(id.to_owned()))?;
    Ok((namespace.to_owned(), JavaString::from(path)))
}

/// `CommandStorage`: remembers which namespaces it has loaded or created (that is what `keys()`
/// walks, in `HashMap` order), over a [`DimensionDataStorage`] the caller passes in.
#[derive(Debug, Clone, Default)]
pub struct CommandStorage {
    namespaces: JavaHashMap<JavaString, ()>,
}

impl CommandStorage {
    pub fn new() -> Self {
        Self::default()
    }

    fn register(&mut self, namespace: &str) {
        let key = JavaString::from(namespace);
        if self.namespaces.get(&key).is_none() {
            self.namespaces.put(key, ());
        }
    }

    /// `get(ResourceLocation)`: the stored compound, an empty one when there is none.
    pub fn get(
        &mut self,
        storage: &mut DimensionDataStorage,
        id: &str,
    ) -> Result<Tag, SavedDataError> {
        let (namespace, path) = split(id)?;
        let name = format!("{ID_PREFIX}{namespace}");
        let found = storage
            .get(&factory(), &name)?
            .map(|container| container.get(&path));
        match found {
            Some(tag) => {
                self.register(&namespace);
                Ok(tag)
            }
            None => Ok(empty_compound()),
        }
    }

    /// `set(ResourceLocation, CompoundTag)`.
    pub fn set(
        &mut self,
        storage: &mut DimensionDataStorage,
        id: &str,
        tag: Tag,
    ) -> Result<(), SavedDataError> {
        let (namespace, path) = split(id)?;
        let name = format!("{ID_PREFIX}{namespace}");
        storage.compute_if_absent(&factory(), &name)?.put(path, tag);
        self.register(&namespace);
        Ok(())
    }

    /// `keys()`: `namespace:path` of every entry of every namespace this instance has touched.
    /// A stored path that is not a valid resource-location path makes the original throw.
    pub fn keys(&self, storage: &mut DimensionDataStorage) -> Result<Vec<String>, SavedDataError> {
        let mut out = Vec::new();
        for (namespace, _) in self.namespaces.iter_ordered() {
            let namespace = namespace.to_string_lossy().into_owned();
            let name = format!("{ID_PREFIX}{namespace}");
            let Some(container) = storage.get(&factory(), &name)? else {
                continue;
            };
            for path in container.paths() {
                let text = format!("{namespace}:{}", path.to_string_lossy());
                if path.as_str().is_none() || resource_location(&text).as_deref() != Some(&text) {
                    return Err(SavedDataError::InvalidName(text));
                }
                out.push(text);
            }
        }
        Ok(out)
    }
}
