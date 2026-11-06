//! `WorldDataConfiguration`: the `DataPacks` and `enabled_features` of `level.dat`'s `Data` compound.

use super::packs::{feature_names_in_set_order, FEATURE_FLAGS};
use crate::components::decode::Val;
use crate::components::java::resource_location;
use crate::components::text::{hash_ordered_compound, nbt_string};
use crate::nbt::Tag;

/// `WorldDataConfiguration(DataPackConfig(enabled, disabled), enabledFeatures)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldDataConfiguration {
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
    /// Feature flag names (`minecraft:vanilla`, ...), in no particular order.
    pub features: Vec<String>,
    /// `DataPackConfig` has no `equals`, so the codec leaves `DataPacks` out only for the very
    /// `DEFAULT` instance it falls back to, never for an equal list that was read from the file.
    packs_are_default_instance: bool,
}

impl Default for WorldDataConfiguration {
    /// `WorldDataConfiguration.DEFAULT`: the `vanilla` pack and the `vanilla` feature.
    fn default() -> Self {
        Self {
            enabled: vec!["vanilla".to_owned()],
            disabled: Vec::new(),
            features: vec!["minecraft:vanilla".to_owned()],
            packs_are_default_instance: true,
        }
    }
}

fn strings(value: &Val) -> Option<Vec<String>> {
    let Val::List(items) = value else { return None };
    items
        .iter()
        .map(|item| {
            if let Val::Str(text) = item {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect()
}

fn data_packs(value: &Val) -> Option<(Vec<String>, Vec<String>)> {
    Some((
        strings(value.get("Enabled")?)?,
        strings(value.get("Disabled")?)?,
    ))
}

fn features(value: &Val) -> Option<Vec<String>> {
    let mut found: Vec<String> = Vec::new();
    for name in strings(value)? {
        let id = resource_location(&name)?;
        if !FEATURE_FLAGS.contains(&id.as_str()) {
            return None;
        }
        if !found.contains(&id) {
            found.push(id);
        }
    }
    Some(found)
}

impl WorldDataConfiguration {
    /// A configuration chosen by the caller (never the `DEFAULT` instance).
    pub fn new(enabled: Vec<String>, disabled: Vec<String>, features: Vec<String>) -> Self {
        Self {
            enabled,
            disabled,
            features,
            packs_are_default_instance: false,
        }
    }

    /// `LevelStorageSource.readDataConfig`: `CODEC.parse(..).resultOrPartial(..).orElse(DEFAULT)`.
    /// Each member is lenient on its own (a bad `DataPacks` keeps the default packs and still reads
    /// the features); anything that is not a compound is the default.
    pub fn from_nbt(tag: &Tag) -> Self {
        let Ok(value) = Val::from_nbt(tag) else {
            return Self::default();
        };
        if !matches!(value, Val::Map(_)) {
            return Self::default();
        }
        let mut config = Self::default();
        if let Some((enabled, disabled)) = value.get("DataPacks").and_then(data_packs) {
            config.enabled = enabled;
            config.disabled = disabled;
            config.packs_are_default_instance = false;
        }
        if let Some(found) = value.get("enabled_features").and_then(features) {
            config.features = found;
        }
        config
    }

    /// `CODEC.encodeStart(NbtOps, ..)`: what `PrimaryLevelData` merges into `Data`.
    pub fn to_nbt(&self) -> Tag {
        let list = |items: &[String]| -> Tag {
            if items.is_empty() {
                Tag::List {
                    element_type: 0,
                    values: Vec::new(),
                }
            } else {
                Tag::List {
                    element_type: 8,
                    values: items.iter().map(|text| nbt_string(text)).collect(),
                }
            }
        };
        let known: Vec<String> = FEATURE_FLAGS
            .iter()
            .filter(|flag| self.features.iter().any(|f| f == *flag))
            .map(|f| (*f).to_owned())
            .collect();
        let mut members: Vec<(&str, Tag)> = Vec::new();
        if !self.packs_are_default_instance {
            members.push((
                "DataPacks",
                hash_ordered_compound(vec![
                    ("Enabled", list(&self.enabled)),
                    ("Disabled", list(&self.disabled)),
                ]),
            ));
        }
        // `FeatureFlagSet.equals`: the default set is {vanilla}.
        if known != ["minecraft:vanilla"] {
            members.push((
                "enabled_features",
                list(&feature_names_in_set_order(&known)),
            ));
        }
        hash_ordered_compound(members)
    }
}
