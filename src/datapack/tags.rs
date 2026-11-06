//! `net.minecraft.tags.TagLoader`: loads `tags/<directory>/**.json` from an ordered stack of packs,
//! merges them (`replace`), and resolves nested tags with `DependencySorter`.
//!
//! The results are sets, but two details make the original's `HashMap` iteration order matter:
//! a tag that is part of a reference cycle is resolved or dropped depending on the order the
//! sorter visits tags in, and an invalid file still creates an (empty) tag. Both are reproduced.

use crate::components::java::{
    hashmap_order_by_hash, hashmap_order_with, resource_location, resource_location_hash,
};
use crate::components::lenient;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

/// `TagEntry`: an element or a `#tag`, required unless written as `{ "id": .., "required": false }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagEntry {
    pub id: String,
    pub tag: bool,
    pub required: bool,
}

/// `ExtraCodecs.TAG_OR_ELEMENT_ID`.
fn tag_or_element_id(text: &str) -> Option<(String, bool)> {
    match text.strip_prefix('#') {
        Some(rest) => resource_location(rest).map(|id| (id, true)),
        None => resource_location(text).map(|id| (id, false)),
    }
}

impl TagEntry {
    /// `TagEntry.CODEC`: the string form, or the record form, tried in that order.
    fn parse(value: &Value) -> Option<Self> {
        if let Value::String(text) = value {
            let (id, tag) = tag_or_element_id(text)?;
            return Some(Self {
                id,
                tag,
                required: true,
            });
        }
        let Value::Object(map) = value else {
            return None;
        };
        let id = map.get("id").filter(|v| !v.is_null())?.as_str()?;
        let (id, tag) = tag_or_element_id(id)?;
        let required = match map.get("required").filter(|v| !v.is_null()) {
            None => true,
            Some(Value::Bool(flag)) => *flag,
            Some(_) => return None,
        };
        Some(Self { id, tag, required })
    }
}

/// `TagFile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagFile {
    pub entries: Vec<TagEntry>,
    pub replace: bool,
}

impl TagFile {
    /// `TagFile.CODEC.parse(JsonOps, json)`; `None` is a decode error.
    pub fn parse(value: &Value) -> Option<Self> {
        let Value::Object(map) = value else {
            return None;
        };
        let Value::Array(values) = map.get("values").filter(|v| !v.is_null())? else {
            return None;
        };
        let entries = values
            .iter()
            .map(TagEntry::parse)
            .collect::<Option<Vec<_>>>()?;
        let replace = match map.get("replace").filter(|v| !v.is_null()) {
            None => false,
            Some(Value::Bool(flag)) => *flag,
            Some(_) => return None,
        };
        Some(Self { entries, replace })
    }
}

/// `TagLoader.EntryWithSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryWithSource {
    pub entry: TagEntry,
    pub source: String,
}

/// One data pack's files under the tag directory: `namespace:path` of the tag and the file text.
#[derive(Debug, Clone, Default)]
pub struct PackTags {
    pub id: String,
    pub files: BTreeMap<String, String>,
}

/// The raw result of `TagLoader.load`: tag id to its entries, in `HashMap` iteration order.
pub type Loaded = Vec<(String, Vec<EntryWithSource>)>;

/// `TagLoader.load(resourceManager)` over `packs` ordered from lowest to highest priority.
/// `directory` is the path below `data/<namespace>/` (`tags/item`).
pub fn load(packs: &[PackTags], directory: &str) -> Loaded {
    // `listResourceStacks` is a `TreeMap` over the file ids: `path` first, then `namespace`.
    let mut files: BTreeMap<(String, String), String> = BTreeMap::new();
    for pack in packs {
        for id in pack.files.keys() {
            let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
            files.insert(
                (format!("{directory}/{path}.json"), namespace.to_owned()),
                id.clone(),
            );
        }
    }
    let mut tags: HashMap<String, Vec<EntryWithSource>> = HashMap::new();
    let mut created: Vec<String> = Vec::new();
    for (_, id) in &files {
        for pack in packs {
            let Some(text) = pack.files.get(id) else {
                continue;
            };
            // JsonParser.parseReader: a syntax error is logged and the file is skipped.
            let Ok(json) = lenient::parse(text) else {
                continue;
            };
            // The list is created before the codec runs, so a file the codec rejects leaves an empty tag.
            if !tags.contains_key(id) {
                tags.insert(id.clone(), Vec::new());
                created.push(id.clone());
            }
            let Some(file) = TagFile::parse(&json) else {
                continue;
            };
            let list = tags.get_mut(id).expect("created above");
            if file.replace {
                list.clear();
            }
            list.extend(file.entries.into_iter().map(|entry| EntryWithSource {
                entry,
                source: pack.id.clone(),
            }));
        }
    }
    // `loaded` is a `HashMap<ResourceLocation, ..>` filled by `computeIfAbsent` in listing order,
    // which links a colliding key in front of the older ones.
    let hashes: Vec<i32> = created
        .iter()
        .map(|id| resource_location_hash(id))
        .collect();
    hashmap_order_with(&hashes, 16, true)
        .into_iter()
        .map(|index| {
            (
                created[index].clone(),
                tags.remove(&created[index]).unwrap_or_default(),
            )
        })
        .collect()
}

/// `DependencySorter.isCyclic` never returns once a tag depends on itself and a later check walks
/// through it: the original dies with a `StackOverflowError`, which aborts the whole reload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyOverflow;

/// `DependencySorter` over the loaded tags, followed by `TagLoader.build`: tag id to its members in
/// `ImmutableSet` order. `is_known(id)` is `idToValue`. Tags that reference something required and
/// missing are absent from the result.
pub fn build(
    loaded: &Loaded,
    is_known: impl Fn(&str) -> bool,
) -> Result<BTreeMap<String, Vec<String>>, DependencyOverflow> {
    // contents: HashMap<ResourceLocation, entries>, iterated in HashMap order (insertion order = `loaded`).
    let ids: Vec<&str> = loaded.iter().map(|(id, _)| id.as_str()).collect();
    let hashes: Vec<i32> = ids.iter().map(|id| resource_location_hash(id)).collect();
    let order: Vec<usize> = hashmap_order_by_hash(&hashes, 16);
    let entries_of = |id: &str| {
        loaded
            .iter()
            .find(|(candidate, _)| candidate == id)
            .map(|(_, entries)| entries)
    };

    // HashMultimap<K, K>: each key's dependencies live in a HashSet that starts with 4 buckets.
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
    // `isCyclic(map, start, node)`: does `node` reach `start`? The recursion has no visited set, so a
    // node already on the current path (only a self-edge can cause that) never terminates.
    // A key's dependencies live in a `HashSet` that starts with 4 buckets (`HashMultimap`'s
    // expected two values per key), and `anyMatch` stops at the first hit in that order.
    fn ordered(map: &HashMap<String, Vec<String>>, node: &str) -> Vec<String> {
        let Some(set) = map.get(node) else {
            return Vec::new();
        };
        let names: Vec<&str> = set.iter().map(String::as_str).collect();
        let hashes: Vec<i32> = names
            .iter()
            .map(|name| resource_location_hash(name))
            .collect();
        hashmap_order_by_hash(&hashes, 4)
            .into_iter()
            .map(|i| names[i].to_owned())
            .collect()
    }
    // `isCyclic(map, start, node)`: does `node` reach `start`? The recursion has no visited set, so a
    // node already on the current path (only a self-edge can cause that) never terminates, unless
    // an earlier sibling already answered `true`.
    let is_cyclic = |map: &HashMap<String, Vec<String>>,
                     start: &str,
                     node: &str|
     -> Result<bool, DependencyOverflow> {
        fn walk(
            map: &HashMap<String, Vec<String>>,
            start: &str,
            node: &str,
            path: &mut Vec<String>,
        ) -> Result<bool, DependencyOverflow> {
            if path.iter().any(|seen| seen == node) {
                return Err(DependencyOverflow);
            }
            let next = ordered(map, node);
            if next.iter().any(|candidate| candidate == start) {
                return Ok(true);
            }
            path.push(node.to_owned());
            for candidate in &next {
                if walk(map, start, candidate, path)? {
                    path.pop();
                    return Ok(true);
                }
            }
            path.pop();
            Ok(false)
        }
        walk(map, start, node, &mut Vec::new())
    };
    for optional in [false, true] {
        for &index in &order {
            let id = ids[index];
            for source in entries_of(id).into_iter().flatten() {
                if source.entry.tag
                    && source.entry.required != optional
                    && !is_cyclic(&dependencies, id, &source.entry.id)?
                {
                    let set = dependencies.entry(id.to_owned()).or_default();
                    if !set.contains(&source.entry.id) {
                        set.push(source.entry.id.clone());
                    }
                }
            }
        }
    }
    let ordered_dependencies = |id: &str| -> Vec<String> { ordered(&dependencies, id) };

    let mut built: HashMap<String, Vec<String>> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    fn visit(
        id: &str,
        visited: &mut HashSet<String>,
        ordered_dependencies: &dyn Fn(&str) -> Vec<String>,
        resolve: &mut dyn FnMut(&str),
    ) {
        if !visited.insert(id.to_owned()) {
            return;
        }
        for dependency in ordered_dependencies(id) {
            visit(&dependency, visited, ordered_dependencies, resolve);
        }
        resolve(id);
    }
    let mut resolve = |id: &str| {
        let Some(entries) = entries_of(id) else {
            return;
        };
        let mut members: Vec<String> = Vec::new();
        let mut complete = true;
        for source in entries {
            let found: Option<Vec<String>> = if source.entry.tag {
                built.get(&source.entry.id).cloned()
            } else {
                is_known(&source.entry.id).then(|| vec![source.entry.id.clone()])
            };
            match found {
                Some(values) => {
                    for value in values {
                        if !members.contains(&value) {
                            members.push(value);
                        }
                    }
                }
                None => complete &= !source.entry.required,
            }
        }
        if complete {
            built.insert(id.to_owned(), members);
        }
    };
    for &index in &order {
        visit(
            ids[index],
            &mut visited,
            &ordered_dependencies,
            &mut resolve,
        );
    }
    Ok(built.into_iter().collect())
}

/// `TagLoader.loadAndBuild`.
pub fn load_and_build(
    packs: &[PackTags],
    directory: &str,
    is_known: impl Fn(&str) -> bool,
) -> Result<BTreeMap<String, Vec<String>>, DependencyOverflow> {
    build(&load(packs, directory), is_known)
}
