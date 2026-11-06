//! `PackRepository` and friends: which data packs exist, which are selected, and what each declares.
//!
//! A pack is a folder or a `.zip` in the world's `datapacks` directory with a `pack.mcmeta`, or one
//! of the three packs built into the server. Evidence and limits: `docs/reference/datapack-packs.md`.

use super::zip;
use crate::components::java::{
    hashmap_order_by_hash, json_number_int_value, resource_location, resource_location_hash,
};
use crate::components::lenient::parse_strict_prefix;
use crate::components::text::Component;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// `SharedConstants.getCurrentVersion().getPackVersion(PackType.SERVER_DATA)` for 1.21.
pub const SERVER_DATA_PACK_FORMAT: i32 = 48;

/// The feature flags of the main registry, in registration order.
pub const FEATURE_FLAGS: [&str; 3] = [
    "minecraft:vanilla",
    "minecraft:bundle",
    "minecraft:trade_rebalance",
];

/// `PackCompatibility`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    TooOld,
    TooNew,
    Compatible,
}

impl Compatibility {
    /// `PackCompatibility.forVersion(range, version)`.
    pub fn for_version(min: i32, max: i32, version: i32) -> Self {
        if max < version {
            Self::TooOld
        } else if version < min {
            Self::TooNew
        } else {
            Self::Compatible
        }
    }

    pub fn is_compatible(self) -> bool {
        self == Self::Compatible
    }
}

/// `Pack.Position`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
}

/// What `pack.mcmeta` declares, as `Pack.Metadata` keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct PackMetadata {
    pub description: Component,
    pub compatibility: Compatibility,
    /// `FeatureFlagSet` as sorted names.
    pub features: Vec<String>,
    pub overlays: Vec<String>,
}

/// `InclusiveRange<Integer>` (`ExtraCodecs.intervalCodec`): a number, a pair, or `{min_inclusive, max_inclusive}`.
fn parse_range(value: &Value) -> Option<(i32, i32)> {
    let int = |v: &Value| -> Option<i32> {
        match v {
            Value::Number(n) => Some(json_number_int_value(n)),
            _ => None,
        }
    };
    let (min, max) = match value {
        Value::Array(items) if items.len() == 2 => (int(&items[0])?, int(&items[1])?),
        Value::Object(map) => (
            int(map.get("min_inclusive").filter(|v| !v.is_null())?)?,
            int(map.get("max_inclusive").filter(|v| !v.is_null())?)?,
        ),
        single => {
            let n = int(single)?;
            (n, n)
        }
    };
    (min <= max).then_some((min, max))
}

/// `FeatureFlags.CODEC`: a list of known flag names (`vanilla` means `minecraft:vanilla`).
fn parse_feature_flags(value: &Value) -> Option<Vec<String>> {
    let Value::Array(items) = value else {
        return None;
    };
    let mut found: Vec<String> = Vec::new();
    for item in items {
        let id = resource_location(item.as_str()?)?;
        if !FEATURE_FLAGS.contains(&id.as_str()) {
            return None;
        }
        if !found.contains(&id) {
            found.push(id);
        }
    }
    Some(sorted_unique(found))
}

fn sorted_unique(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}

/// `Pack.readPackMetadata` for the text of a `pack.mcmeta`; `None` is a pack the original skips.
pub fn read_metadata(mcmeta: &str, pack_version: i32) -> Option<PackMetadata> {
    // GsonHelper.parse: Gson's strict reader, an object at the top. It reads one value and never
    // looks at what follows it, skips a leading byte-order mark, and accepts raw control
    // characters inside strings.
    let Value::Object(root) = parse_strict_prefix(mcmeta).ok()? else {
        return None;
    };
    // PackMetadataSection (required): description, pack_format, lenient supported_formats.
    let Value::Object(pack) = root.get("pack")? else {
        return None;
    };
    let description =
        Component::from_json(pack.get("description").filter(|v| !v.is_null())?).ok()?;
    let pack_format = match pack.get("pack_format").filter(|v| !v.is_null())? {
        Value::Number(n) => json_number_int_value(n),
        _ => return None,
    };
    let declared = match pack
        .get("supported_formats")
        .filter(|v| !v.is_null())
        .and_then(parse_range)
    {
        Some((min, max)) if (min..=max).contains(&pack_format) => (min, max),
        _ => (pack_format, pack_format),
    };
    // features: an unreadable section is "absent", not a failure.
    let features = root
        .get("features")
        .and_then(|section| match section {
            Value::Object(map) => parse_feature_flags(map.get("enabled").filter(|v| !v.is_null())?),
            _ => None,
        })
        .unwrap_or_default();
    // overlays: entries with a range and a directory name made of `[-_a-zA-Z0-9.]`.
    let overlays = root
        .get("overlays")
        .and_then(|section| {
            let Value::Object(map) = section else {
                return None;
            };
            let Value::Array(entries) = map.get("entries").filter(|v| !v.is_null())? else {
                return None;
            };
            let mut applicable = Vec::new();
            for entry in entries {
                let Value::Object(entry) = entry else {
                    return None;
                };
                let (min, max) = parse_range(entry.get("formats").filter(|v| !v.is_null())?)?;
                let directory = entry.get("directory").filter(|v| !v.is_null())?.as_str()?;
                if directory.is_empty()
                    || !directory
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                {
                    return None;
                }
                if (min..=max).contains(&pack_version) {
                    applicable.push(directory.to_owned());
                }
            }
            Some(applicable)
        })
        .unwrap_or_default();
    Some(PackMetadata {
        description,
        compatibility: Compatibility::for_version(declared.0, declared.1, pack_version),
        features,
        overlays,
    })
}

/// A discovered or built-in pack.
#[derive(Debug, Clone, PartialEq)]
pub struct Pack {
    pub id: String,
    pub title: Component,
    pub metadata: PackMetadata,
    pub required: bool,
    pub fixed_position: bool,
    pub default_position: Position,
    /// `PackSource.shouldAddAutomatically()`: world and built-in packs are, feature packs are not.
    pub add_automatically: bool,
}

impl Pack {
    pub fn is_compatible(&self) -> bool {
        self.metadata.compatibility.is_compatible()
    }
}

/// `Pack.Position.insert`: where a required pack goes among the selected ones.
fn insert_by_position(selected: &mut Vec<Pack>, pack: Pack) {
    let position = pack.default_position;
    let fixed_here = |other: &Pack| other.fixed_position && other.default_position == position;
    match position {
        Position::Bottom => {
            let index = selected
                .iter()
                .take_while(|other| fixed_here(other))
                .count();
            selected.insert(index, pack);
        }
        Position::Top => {
            let index = selected
                .iter()
                .rposition(|other| !fixed_here(other))
                .map_or(0, |last| last + 1);
            selected.insert(index, pack);
        }
    }
}

fn builtin(
    id: &str,
    translatable_title: bool,
    source_auto: bool,
    position: Position,
    features: &[&str],
) -> Pack {
    let key = |what: &str| {
        Component::from_json(&serde_json::json!({ "translate": format!("dataPack.{id}.{what}") }))
            .expect("translatable")
    };
    Pack {
        id: id.to_owned(),
        title: if translatable_title {
            key("name")
        } else {
            Component::literal(id)
        },
        metadata: PackMetadata {
            description: key("description"),
            compatibility: Compatibility::Compatible,
            features: features.iter().map(|f| (*f).to_owned()).collect(),
            overlays: Vec::new(),
        },
        required: false,
        fixed_position: false,
        default_position: position,
        add_automatically: source_auto,
    }
}

/// The packs built into the server: `vanilla` (bottom, added automatically) and the two feature
/// packs `bundle` and `trade_rebalance` (top, never added automatically).
pub fn built_in_packs() -> Vec<Pack> {
    vec![
        builtin(
            "vanilla",
            true,
            true,
            Position::Bottom,
            &["minecraft:vanilla"],
        ),
        builtin("bundle", false, false, Position::Top, &["minecraft:bundle"]),
        builtin(
            "trade_rebalance",
            false,
            false,
            Position::Top,
            &["minecraft:trade_rebalance"],
        ),
    ]
}

/// A `TreeMap<String, _>` key: strings compare by UTF-16 code units.
fn java_key(id: &str) -> Vec<u16> {
    id.encode_utf16().collect()
}

/// `FolderRepositorySource.loadPacks`: every directory with a `pack.mcmeta` and every `.zip` with one
/// at its root, as `file/<name>`. Symbolic links are skipped (the world's directory validator
/// refuses them unless allowed), as are entries whose metadata cannot be read.
pub fn discover_folder(folder: &Path) -> Vec<Pack> {
    let _ = std::fs::create_dir_all(folder);
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut packs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if kind.file_type().is_symlink() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let text = if kind.is_dir() {
            path.join("pack.mcmeta")
                .is_file()
                .then(|| std::fs::read(path.join("pack.mcmeta")).ok())
                .flatten()
        } else if kind.is_file() && name.ends_with(".zip") {
            std::fs::read(&path)
                .ok()
                .and_then(|archive| zip::read_entry(&archive, "pack.mcmeta"))
        } else {
            None
        };
        let Some(bytes) = text else { continue };
        let Some(metadata) =
            read_metadata(&String::from_utf8_lossy(&bytes), SERVER_DATA_PACK_FORMAT)
        else {
            continue;
        };
        packs.push(Pack {
            id: format!("file/{name}"),
            title: Component::literal(name),
            metadata,
            required: false,
            fixed_position: false,
            default_position: Position::Top,
            add_automatically: true,
        });
    }
    packs
}

/// `PackRepository`.
#[derive(Debug, Clone)]
pub struct PackRepository {
    folder: Option<std::path::PathBuf>,
    available: BTreeMap<Vec<u16>, Pack>,
    selected: Vec<Pack>,
}

impl PackRepository {
    /// `new PackRepository(new ServerPacksSource(..), new FolderRepositorySource(folder, ..))`,
    /// already discovered but with nothing selected.
    pub fn new(datapacks: &Path) -> Self {
        let mut repository = Self {
            folder: Some(datapacks.to_owned()),
            available: BTreeMap::new(),
            selected: Vec::new(),
        };
        repository.reload();
        repository
    }

    /// A repository over the built-in packs only (`createVanillaTrustedRepository`).
    pub fn vanilla_only() -> Self {
        let mut repository = Self {
            folder: None,
            available: BTreeMap::new(),
            selected: Vec::new(),
        };
        repository.reload();
        repository
    }

    fn rebuild_selected(&self, ids: &[String]) -> Vec<Pack> {
        let mut selected: Vec<Pack> = ids
            .iter()
            .filter_map(|id| self.available.get(&java_key(id)).cloned())
            .collect();
        for pack in self.available.values() {
            if pack.required && !selected.iter().any(|candidate| candidate.id == pack.id) {
                insert_by_position(&mut selected, pack.clone());
            }
        }
        selected
    }

    /// `reload()`: re-discover the packs and keep the selection that still exists.
    pub fn reload(&mut self) {
        let previous: Vec<String> = self.selected.iter().map(|pack| pack.id.clone()).collect();
        let mut available = BTreeMap::new();
        for pack in built_in_packs() {
            available.insert(java_key(&pack.id), pack);
        }
        if let Some(folder) = &self.folder {
            for pack in discover_folder(folder) {
                available.insert(java_key(&pack.id), pack);
            }
        }
        self.available = available;
        self.selected = self.rebuild_selected(&previous);
    }

    /// `setSelected(ids)`: unknown ids are dropped, duplicates are kept.
    pub fn set_selected(&mut self, ids: &[String]) {
        self.selected = self.rebuild_selected(ids);
    }

    pub fn add_pack(&mut self, id: &str) -> bool {
        match self.available.get(&java_key(id)) {
            Some(pack)
                if !self
                    .selected
                    .iter()
                    .any(|candidate| candidate.id == pack.id) =>
            {
                self.selected.push(pack.clone());
                true
            }
            _ => false,
        }
    }

    pub fn remove_pack(&mut self, id: &str) -> bool {
        match self
            .selected
            .iter()
            .position(|candidate| candidate.id == id)
        {
            Some(index) if self.available.contains_key(&java_key(id)) => {
                self.selected.remove(index);
                true
            }
            _ => false,
        }
    }

    /// `getAvailablePacks()` in `TreeMap` order.
    pub fn available_packs(&self) -> Vec<&Pack> {
        self.available.values().collect()
    }

    /// `getSelectedPacks()`.
    pub fn selected_packs(&self) -> &[Pack] {
        &self.selected
    }

    /// `getSelectedIds()` (an `ImmutableSet`: first occurrence wins).
    pub fn selected_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        for pack in &self.selected {
            if !ids.contains(&pack.id) {
                ids.push(pack.id.clone());
            }
        }
        ids
    }

    pub fn pack(&self, id: &str) -> Option<&Pack> {
        self.available.get(&java_key(id))
    }

    /// `getRequestedFeatureFlags()` as sorted names.
    pub fn requested_features(&self) -> Vec<String> {
        sorted_unique(
            self.selected
                .iter()
                .flat_map(|pack| pack.metadata.features.iter().cloned())
                .collect(),
        )
    }
}

/// `FeatureFlagRegistry.toNames` iterates a `HashSet<ResourceLocation>`; this is the order it uses.
pub fn feature_names_in_set_order(names: &[String]) -> Vec<String> {
    let hashes: Vec<i32> = names
        .iter()
        .map(|name| resource_location_hash(name))
        .collect();
    hashmap_order_by_hash(&hashes, 16)
        .into_iter()
        .map(|index| names[index].clone())
        .collect()
}
