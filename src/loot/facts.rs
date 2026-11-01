//! Captured vanilla registry facts used by loot and its predicates.
//!
//! `fixtures/loot-registry-facts-1.21.json` is exported by `tools/loot_oracle.java
//! export-facts` from the unchanged original classes after loading the vanilla
//! data pack: enchantment definitions in registry order, the resolved tag
//! contents (nested tags already expanded in `TagLoader` order), registry ID
//! orders and `Item.getEnchantmentValue`. Nothing here is hand-written data.

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub struct Cost {
    pub base: i32,
    pub per_level_above_first: i32,
}

impl Cost {
    /// `Enchantment.Cost.calculate(level)`.
    pub fn calculate(self, level: i32) -> i32 {
        self.base
            .wrapping_add(self.per_level_above_first.wrapping_mul(level.wrapping_sub(1)))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnchantmentFact {
    pub id: String,
    pub weight: i32,
    pub max_level: i32,
    pub min_cost: Cost,
    pub max_cost: Cost,
    pub anvil_cost: i32,
    pub slots: Vec<String>,
    pub supported_items: Vec<String>,
    pub primary_items: Option<Vec<String>>,
    pub exclusive_set: Vec<String>,
    #[serde(skip)]
    supported: HashSet<String>,
    #[serde(skip)]
    primary: Option<HashSet<String>>,
    #[serde(skip)]
    exclusive: HashSet<String>,
}

impl EnchantmentFact {
    /// `Enchantment.isSupportedItem` / `canEnchant` (`ItemStack.is(HolderSet)`).
    pub fn is_supported_item(&self, item: &str) -> bool {
        self.supported.contains(item)
    }

    /// `Enchantment.isPrimaryItem`.
    pub fn is_primary_item(&self, item: &str) -> bool {
        self.is_supported_item(item)
            && self
                .primary
                .as_ref()
                .map_or(true, |primary| primary.contains(item))
    }

    /// `exclusiveSet.contains(holder)`.
    pub fn excludes(&self, other: &str) -> bool {
        self.exclusive.contains(other)
    }

    /// `Enchantment.getMinCost(level)`.
    pub fn min_cost_at(&self, level: i32) -> i32 {
        self.min_cost.calculate(level)
    }

    /// `Enchantment.getMaxCost(level)`.
    pub fn max_cost_at(&self, level: i32) -> i32 {
        self.max_cost.calculate(level)
    }

    /// `EquipmentSlotGroup.test(slot)` over the definition's slot groups.
    pub fn matches_slot(&self, slot: crate::loot::host::EquipmentSlot) -> bool {
        self.slots.iter().any(|group| slot_group_contains(group, slot))
    }
}

/// `EquipmentSlotGroup.test(EquipmentSlot)`.
pub fn slot_group_contains(group: &str, slot: crate::loot::host::EquipmentSlot) -> bool {
    use crate::loot::host::EquipmentSlot as S;
    match group {
        "any" => true,
        "mainhand" => slot == S::MainHand,
        "offhand" => slot == S::OffHand,
        "hand" => matches!(slot, S::MainHand | S::OffHand),
        "feet" => slot == S::Feet,
        "legs" => slot == S::Legs,
        "chest" => slot == S::Chest,
        "head" => slot == S::Head,
        "armor" => matches!(slot, S::Feet | S::Legs | S::Chest | S::Head),
        "body" => slot == S::Body,
        _ => false,
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct Registries {
    pub item: Vec<String>,
    pub block: Vec<String>,
    pub potion: Vec<String>,
    pub mob_effect: Vec<String>,
    pub entity_type: Vec<String>,
    pub enchantment: Vec<String>,
    pub instrument: Vec<String>,
    pub trim_material: Vec<String>,
    pub trim_pattern: Vec<String>,
    pub damage_type: Vec<String>,
    pub biome: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Tags {
    pub item: BTreeMap<String, Vec<String>>,
    pub block: BTreeMap<String, Vec<String>>,
    pub enchantment: BTreeMap<String, Vec<String>>,
    pub damage_type: BTreeMap<String, Vec<String>>,
    pub entity_type: BTreeMap<String, Vec<String>>,
    pub biome: BTreeMap<String, Vec<String>>,
    pub instrument: BTreeMap<String, Vec<String>>,
}

/// Registry a holder set or tag belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Item,
    Block,
    Enchantment,
    DamageType,
    EntityType,
    Biome,
    Instrument,
}

#[derive(Debug, Deserialize)]
pub struct Facts {
    pub enchantments: Vec<EnchantmentFact>,
    pub item_enchantment_value: HashMap<String, i32>,
    pub disabled_items: Vec<String>,
    pub instant_mob_effects: Vec<String>,
    pub registries: Registries,
    pub tags: Tags,
    #[serde(skip)]
    enchantment_index: HashMap<String, usize>,
}

impl Facts {
    /// Registry ID (also the protocol ID) of a registered enchantment.
    pub fn enchantment_id(&self, id: &str) -> Option<usize> {
        self.enchantment_index.get(id).copied()
    }

    pub fn enchantment(&self, id: &str) -> Option<&EnchantmentFact> {
        self.enchantment_id(id).map(|index| &self.enchantments[index])
    }

    pub fn enchantment_by_index(&self, index: usize) -> Option<&EnchantmentFact> {
        self.enchantments.get(index)
    }

    /// `Item.getEnchantmentValue()` of an item (zero when it has none).
    pub fn item_enchantment_value(&self, item: &str) -> i32 {
        self.item_enchantment_value.get(item).copied().unwrap_or(0)
    }

    /// Resolved tag contents in `TagLoader` order, if the tag exists.
    pub fn tag(&self, kind: Kind, name: &str) -> Option<&Vec<String>> {
        match kind {
            Kind::Item => self.tags.item.get(name),
            Kind::Block => self.tags.block.get(name),
            Kind::Enchantment => self.tags.enchantment.get(name),
            Kind::DamageType => self.tags.damage_type.get(name),
            Kind::EntityType => self.tags.entity_type.get(name),
            Kind::Biome => self.tags.biome.get(name),
            Kind::Instrument => self.tags.instrument.get(name),
        }
    }

    /// Registry ID by name for registries that appear on the wire in loot output.
    pub fn registry_id(&self, registry: &str, name: &str) -> Option<i32> {
        let list = match registry {
            "potion" => &self.registries.potion,
            "mob_effect" => &self.registries.mob_effect,
            "instrument" => &self.registries.instrument,
            "trim_material" => &self.registries.trim_material,
            "trim_pattern" => &self.registries.trim_pattern,
            "enchantment" => &self.registries.enchantment,
            "item" => &self.registries.item,
            "block" => &self.registries.block,
            "entity_type" => &self.registries.entity_type,
            _ => return None,
        };
        list.iter().position(|entry| entry == name).map(|index| index as i32)
    }

    /// `MobEffect.isInstantenous()`.
    pub fn is_instant_mob_effect(&self, effect: &str) -> bool {
        self.instant_mob_effects.iter().any(|instant| instant == effect)
    }

    pub fn is_item_enabled(&self, item: &str) -> bool {
        !self.disabled_items.iter().any(|disabled| disabled == item)
    }
}

pub fn facts() -> &'static Facts {
    static FACTS: OnceLock<Facts> = OnceLock::new();
    FACTS.get_or_init(|| {
        let mut facts: Facts = serde_json::from_str(include_str!(
            "../../fixtures/loot-registry-facts-1.21.json"
        ))
        .expect("captured loot registry facts");
        for (index, enchantment) in facts.enchantments.iter_mut().enumerate() {
            enchantment.supported = enchantment.supported_items.iter().cloned().collect();
            enchantment.primary = enchantment
                .primary_items
                .as_ref()
                .map(|items| items.iter().cloned().collect());
            enchantment.exclusive = enchantment.exclusive_set.iter().cloned().collect();
            facts
                .enchantment_index
                .insert(enchantment.id.clone(), index);
        }
        facts
    })
}

/// A `HolderSet` as written in data: a tag (`#ns:name`), one ID or a list of IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HolderSet {
    Tag(String),
    Direct(Vec<String>),
}

impl HolderSet {
    /// Members in Java iteration order. An unknown tag is the empty set, as
    /// `HolderGetter.getOrThrow(tag)` is never used for data-pack tags here.
    pub fn members(&self, kind: Kind) -> Vec<String> {
        match self {
            Self::Direct(ids) => ids.clone(),
            Self::Tag(name) => facts().tag(kind, name).cloned().unwrap_or_default(),
        }
    }

    pub fn contains(&self, kind: Kind, id: &str) -> bool {
        match self {
            Self::Direct(ids) => ids.iter().any(|entry| entry == id),
            Self::Tag(name) => facts()
                .tag(kind, name)
                .is_some_and(|members| members.iter().any(|entry| entry == id)),
        }
    }
}

/// Canonical resource location: a missing namespace means `minecraft`.
pub fn canonical(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}
