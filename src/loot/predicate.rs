//! The `net.minecraft.advancements.critereon` predicates loot conditions use.
//!
//! Implemented from the original classes: `MinMaxBounds`, `ItemPredicate` with
//! the `enchantments`, `stored_enchantments` and `damage` sub-predicates,
//! `EntityPredicate` (type, flags, equipment, `slime`/`fishing_hook`/variant
//! sub-predicates), `DamageSourcePredicate`, `LocationPredicate`
//! (position, dimension, biomes, block, can_see_sky), `BlockPredicate` and
//! `StatePropertiesPredicate`. Every other clause is parsed into an explicit
//! [`Clause::Unsupported`] marker and fails with `LootError::Unsupported` only
//! when Java's short-circuit order actually reaches it.

use super::context::LootContext;
use super::enchant;
use super::facts::{canonical, HolderSet, Kind};
use super::host::{EquipmentSlot, LootDamageSource, LootEntity};
use super::{unsupported, Result};
use crate::inventory::{self, ItemStack};
use std::collections::BTreeMap;

/// `MinMaxBounds.Ints`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IntBounds {
    pub min: Option<i32>,
    pub max: Option<i32>,
}

impl IntBounds {
    pub const ANY: IntBounds = IntBounds { min: None, max: None };

    pub fn is_any(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// `MinMaxBounds.Ints.matches(int)`.
    pub fn matches(&self, value: i32) -> bool {
        self.min.map_or(true, |min| min <= value) && self.max.map_or(true, |max| max >= value)
    }
}

/// `MinMaxBounds.Doubles`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DoubleBounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl DoubleBounds {
    pub const ANY: DoubleBounds = DoubleBounds { min: None, max: None };

    pub fn is_any(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// `MinMaxBounds.Doubles.matches(double)`.
    pub fn matches(&self, value: f64) -> bool {
        self.min.map_or(true, |min| min <= value) && self.max.map_or(true, |max| max >= value)
    }
}

/// A clause whose Java semantics depend on an owner that does not exist yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedClause(pub String);

fn reached<T>(clause: &UnsupportedClause) -> Result<T> {
    unsupported(format!("predicate clause {}", clause.0))
}

/// `EnchantmentPredicate`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnchantmentPredicate {
    pub enchantments: Option<HolderSet>,
    pub levels: IntBounds,
}

impl EnchantmentPredicate {
    /// `EnchantmentPredicate.containedIn(ItemEnchantments)`.
    pub fn contained_in(&self, enchantments: &[(String, i32)]) -> bool {
        if let Some(set) = &self.enchantments {
            for id in set.members(Kind::Enchantment) {
                let level = enchantments
                    .iter()
                    .find(|(name, _)| *name == id)
                    .map_or(0, |(_, level)| *level);
                if level != 0 && (self.levels.is_any() || self.levels.matches(level)) {
                    return true;
                }
            }
            false
        } else if !self.levels.is_any() {
            enchantments.iter().any(|(_, level)| self.levels.matches(*level))
        } else {
            !enchantments.is_empty()
        }
    }
}

/// Entries of `ItemSubPredicate.CODEC`.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemSubPredicate {
    Enchantments(Vec<EnchantmentPredicate>),
    StoredEnchantments(Vec<EnchantmentPredicate>),
    Damage { durability: IntBounds, damage: IntBounds },
    Unsupported(UnsupportedClause),
}

/// `ItemPredicate`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemPredicate {
    pub items: Option<HolderSet>,
    pub count: IntBounds,
    /// `DataComponentPredicate` exact component matching is not ported.
    pub components: Option<UnsupportedClause>,
    pub sub_predicates: Vec<ItemSubPredicate>,
}

/// `ItemStack.is(HolderSet<Item>)`.
pub fn stack_is(stack: &ItemStack, set: &HolderSet) -> bool {
    set.contains(Kind::Item, &stack_item_id(stack))
}

/// Registered item ID of a stack (`minecraft:air` for the empty stack).
pub fn stack_item_id(stack: &ItemStack) -> String {
    let id = if stack.is_empty() { 0 } else { stack.item_id };
    canonical(inventory::item_name(id).unwrap_or("air"))
}

impl ItemPredicate {
    /// `ItemPredicate.test(ItemStack)`.
    pub fn test(&self, stack: &ItemStack) -> Result<bool> {
        if let Some(items) = &self.items {
            if !stack_is(stack, items) {
                return Ok(false);
            }
        }
        let count = if stack.is_empty() { 0 } else { stack.count };
        if !self.count.matches(count) {
            return Ok(false);
        }
        if let Some(clause) = &self.components {
            return reached(clause);
        }
        for sub in &self.sub_predicates {
            let matched = match sub {
                ItemSubPredicate::Enchantments(list) => {
                    let current = enchant::item_enchantments(stack, false)?;
                    list.iter().all(|predicate| predicate.contained_in(&current))
                }
                ItemSubPredicate::StoredEnchantments(list) => {
                    let current = enchant::item_enchantments(stack, true)?;
                    list.iter().all(|predicate| predicate.contained_in(&current))
                }
                // `SingleComponentItemPredicate`: the DAMAGE component must exist.
                ItemSubPredicate::Damage { durability, damage } => {
                    match stack.effective_component_bytes(3) {
                        None => false,
                        Some(bytes) => {
                            let current = crate::protocol::Reader::new(bytes).varint().map_err(host)?;
                            let max = stack
                                .effective_component_bytes(2)
                                .map_or(Ok(0), |bytes| crate::protocol::Reader::new(bytes).varint())
                                .map_err(host)?;
                            durability.matches(max.wrapping_sub(current)) && damage.matches(current)
                        }
                    }
                }
                ItemSubPredicate::Unsupported(clause) => return reached(clause),
            };
            if !matched {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn host(error: anyhow::Error) -> super::LootError {
    super::LootError::Host(error.to_string())
}

/// `EntityTypePredicate`.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityTypePredicate(pub HolderSet);

/// `EntityFlagsPredicate`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityFlagsPredicate {
    pub on_ground: Option<bool>,
    pub on_fire: Option<bool>,
    pub sneaking: Option<bool>,
    pub sprinting: Option<bool>,
    pub swimming: Option<bool>,
    pub flying: Option<bool>,
    pub baby: Option<bool>,
}

impl EntityFlagsPredicate {
    /// `EntityFlagsPredicate.matches(Entity)`.
    pub fn matches(&self, entity: &dyn LootEntity) -> Result<bool> {
        if let Some(expected) = self.on_ground {
            if entity.on_ground()? != expected {
                return Ok(false);
            }
        }
        if let Some(expected) = self.on_fire {
            if entity.is_on_fire()? != expected {
                return Ok(false);
            }
        }
        if let Some(expected) = self.sneaking {
            if entity.is_crouching()? != expected {
                return Ok(false);
            }
        }
        if let Some(expected) = self.sprinting {
            if entity.is_sprinting()? != expected {
                return Ok(false);
            }
        }
        if let Some(expected) = self.swimming {
            if entity.is_swimming()? != expected {
                return Ok(false);
            }
        }
        if let Some(expected) = self.flying {
            let flying = entity.is_living() && entity.is_flying()?;
            if flying != expected {
                return Ok(false);
            }
        }
        if let Some(expected) = self.baby {
            if entity.is_living() && entity.is_baby()? != expected {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// `EntityEquipmentPredicate`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityEquipmentPredicate {
    pub head: Option<ItemPredicate>,
    pub chest: Option<ItemPredicate>,
    pub legs: Option<ItemPredicate>,
    pub feet: Option<ItemPredicate>,
    pub body: Option<ItemPredicate>,
    pub mainhand: Option<ItemPredicate>,
    pub offhand: Option<ItemPredicate>,
}

impl EntityEquipmentPredicate {
    /// `EntityEquipmentPredicate.matches(Entity)`: false unless a living entity.
    pub fn matches(&self, entity: &dyn LootEntity) -> Result<bool> {
        if !entity.is_living() {
            return Ok(false);
        }
        let slots = [
            (&self.head, EquipmentSlot::Head),
            (&self.chest, EquipmentSlot::Chest),
            (&self.legs, EquipmentSlot::Legs),
            (&self.feet, EquipmentSlot::Feet),
            (&self.body, EquipmentSlot::Body),
            (&self.mainhand, EquipmentSlot::MainHand),
            (&self.offhand, EquipmentSlot::OffHand),
        ];
        for (predicate, slot) in slots {
            if let Some(predicate) = predicate {
                if !predicate.test(&entity.equipment(slot)?)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

/// `EntitySubPredicate` (`type_specific`).
#[derive(Debug, Clone, PartialEq)]
pub enum EntitySubPredicate {
    Slime { size: IntBounds },
    FishingHook { in_open_water: Option<bool> },
    /// Variant predicates (`frog`, `cat`, …): the host knows the entity class.
    Variant { kind: String, value: String },
    Unsupported(UnsupportedClause),
}

impl EntitySubPredicate {
    fn matches(&self, entity: &dyn LootEntity) -> Result<bool> {
        match self {
            Self::Slime { size } => Ok(match entity.slime_size()? {
                Some(actual) => size.matches(actual),
                None => false,
            }),
            Self::FishingHook { in_open_water } => match in_open_water {
                None => Ok(true),
                Some(expected) => Ok(match entity.fishing_hook_open_water()? {
                    Some(actual) => actual == *expected,
                    None => false,
                }),
            },
            Self::Variant { kind, value } => {
                Ok(entity.variant(kind)?.is_some_and(|actual| actual == *value))
            }
            Self::Unsupported(clause) => reached(clause),
        }
    }
}

/// `EntityPredicate`. Clause order is the order of `EntityPredicate.matches`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityPredicate {
    pub entity_type: Option<EntityTypePredicate>,
    pub distance: Option<UnsupportedClause>,
    pub movement: Option<UnsupportedClause>,
    pub located: Option<Box<LocationPredicate>>,
    pub stepping_on: Option<UnsupportedClause>,
    pub movement_affected_by: Option<UnsupportedClause>,
    pub effects: Option<UnsupportedClause>,
    pub flags: Option<EntityFlagsPredicate>,
    pub equipment: Option<EntityEquipmentPredicate>,
    pub sub_predicate: Option<EntitySubPredicate>,
    pub vehicle: Option<UnsupportedClause>,
    pub passenger: Option<UnsupportedClause>,
    pub targeted_entity: Option<UnsupportedClause>,
    pub periodic_tick: Option<UnsupportedClause>,
    pub team: Option<UnsupportedClause>,
    pub slots: Option<UnsupportedClause>,
    pub nbt: Option<UnsupportedClause>,
}

impl EntityPredicate {
    /// `EntityPredicate.matches(level, origin, entity)`.
    pub fn matches(
        &self,
        ctx: &LootContext<'_>,
        origin: Option<[f64; 3]>,
        entity: Option<&dyn LootEntity>,
    ) -> Result<bool> {
        let Some(entity) = entity else {
            return Ok(false);
        };
        if let Some(EntityTypePredicate(types)) = &self.entity_type {
            if !types.contains(Kind::EntityType, &entity.type_id()?) {
                return Ok(false);
            }
        }
        if let Some(clause) = &self.distance {
            // With no origin the Java predicate fails before measuring.
            if origin.is_none() {
                return Ok(false);
            }
            return reached(clause);
        }
        if let Some(clause) = &self.movement {
            return reached(clause);
        }
        if let Some(located) = &self.located {
            // `Entity.getX/Y/Z` is a native owner the entity trait does not expose.
            let _ = located;
            return unsupported("predicate clause location.located (entity position)");
        }
        if let Some(clause) = &self.stepping_on {
            return reached(clause);
        }
        if let Some(clause) = &self.movement_affected_by {
            return reached(clause);
        }
        if let Some(clause) = &self.effects {
            return reached(clause);
        }
        if let Some(flags) = &self.flags {
            if !flags.matches(entity)? {
                return Ok(false);
            }
        }
        if let Some(equipment) = &self.equipment {
            if !equipment.matches(entity)? {
                return Ok(false);
            }
        }
        if let Some(sub) = &self.sub_predicate {
            if !sub.matches(entity)? {
                return Ok(false);
            }
        }
        for clause in [
            &self.vehicle,
            &self.passenger,
            &self.targeted_entity,
            &self.periodic_tick,
            &self.team,
            &self.slots,
            &self.nbt,
        ]
        .into_iter()
        .flatten()
        {
            return reached(clause);
        }
        let _ = ctx;
        Ok(true)
    }
}

/// `TagPredicate<DamageType>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageTagPredicate {
    pub tag: String,
    pub expected: bool,
}

/// `DamageSourcePredicate`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DamageSourcePredicate {
    pub tags: Vec<DamageTagPredicate>,
    pub direct_entity: Option<EntityPredicate>,
    pub source_entity: Option<EntityPredicate>,
    pub is_direct: Option<bool>,
}

impl DamageSourcePredicate {
    /// `DamageSourcePredicate.matches(level, origin, source)`.
    pub fn matches(
        &self,
        ctx: &LootContext<'_>,
        origin: [f64; 3],
        source: &dyn LootDamageSource,
    ) -> Result<bool> {
        if !self.tags.is_empty() {
            let id = source.type_id()?;
            for predicate in &self.tags {
                let in_tag = super::facts::facts()
                    .tag(Kind::DamageType, &predicate.tag)
                    .is_some_and(|members| members.iter().any(|member| *member == id));
                if in_tag != predicate.expected {
                    return Ok(false);
                }
            }
        }
        if let Some(predicate) = &self.direct_entity {
            if !predicate.matches(ctx, Some(origin), source.direct_entity())? {
                return Ok(false);
            }
        }
        if let Some(predicate) = &self.source_entity {
            if !predicate.matches(ctx, Some(origin), source.source_entity())? {
                return Ok(false);
            }
        }
        match self.is_direct {
            Some(expected) => Ok(expected == source.is_direct()?),
            None => Ok(true),
        }
    }
}

/// `StatePropertiesPredicate.ValueMatcher`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueMatcher {
    Exact(String),
    Ranged { min: Option<String>, max: Option<String> },
}

/// `StatePropertiesPredicate`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatePropertiesPredicate {
    pub properties: Vec<(String, ValueMatcher)>,
}

impl StatePropertiesPredicate {
    /// `StatePropertiesPredicate.matches(BlockState)` for a catalog block state.
    pub fn matches(&self, state: u32) -> Result<bool> {
        let Some(actual) = inventory::block_state_properties(state) else {
            return Err(super::LootError::Host(format!("unknown block state {state}")));
        };
        for (name, matcher) in &self.properties {
            let Some(value) = actual.get(name) else {
                return Ok(false);
            };
            let ok = match matcher {
                ValueMatcher::Exact(expected) => values_equal(value, expected),
                ValueMatcher::Ranged { min, max } => {
                    let current: Option<i64> = value.parse().ok();
                    let Some(current) = current else {
                        return unsupported("ranged state matcher on a non-integer property");
                    };
                    let bound = |text: &Option<String>| text.as_ref().map(|t| t.parse::<i64>().ok());
                    let min_ok = match bound(min) {
                        None => true,
                        Some(None) => false,
                        Some(Some(bound)) => current >= bound,
                    };
                    let max_ok = match bound(max) {
                        None => true,
                        Some(None) => false,
                        Some(Some(bound)) => current <= bound,
                    };
                    min_ok && max_ok
                }
            };
            if !ok {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// `Property.getValue(String)` followed by `compareTo`: integer properties parse
/// numerically (`"07"` equals `7`); other properties compare their names.
fn values_equal(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    match (actual.parse::<i64>(), expected.parse::<i64>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Registered block ID of a block state (`minecraft:stone`).
pub fn block_state_block_id(state: u32) -> Result<String> {
    inventory::block_state_name(state)
        .map(canonical)
        .ok_or_else(|| super::LootError::Host(format!("unknown block state {state}")))
}

/// Property names of a block definition (`StateDefinition.getProperty`), taken
/// from the catalog's first state of that block. `None` for an unknown block.
pub fn block_property_names(block: &str) -> Option<&'static Vec<String>> {
    use std::sync::OnceLock;
    static NAMES: OnceLock<BTreeMap<String, Vec<String>>> = OnceLock::new();
    NAMES
        .get_or_init(|| {
            let mut map = BTreeMap::new();
            let mut last = String::new();
            for state in 0..u32::MAX {
                let Some(name) = inventory::block_state_name(state) else {
                    break;
                };
                if name != last {
                    last = name.to_owned();
                    let properties = inventory::block_state_properties(state)
                        .map(|properties| properties.into_keys().collect())
                        .unwrap_or_default();
                    map.insert(canonical(name), properties);
                }
            }
            map
        })
        .get(block)
}

/// `BlockPredicate`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BlockPredicate {
    pub blocks: Option<HolderSet>,
    pub state: Option<StatePropertiesPredicate>,
    pub nbt: Option<UnsupportedClause>,
}

impl BlockPredicate {
    fn matches_state(&self, state: u32) -> Result<bool> {
        if let Some(blocks) = &self.blocks {
            if !blocks.contains(Kind::Block, &block_state_block_id(state)?) {
                return Ok(false);
            }
        }
        match &self.state {
            Some(properties) => properties.matches(state),
            None => Ok(true),
        }
    }

    /// `BlockPredicate.matches(ServerLevel, BlockPos)`.
    pub fn matches(&self, ctx: &LootContext<'_>, pos: [i32; 3]) -> Result<bool> {
        if !ctx.level().is_loaded(pos)? {
            return Ok(false);
        }
        if !self.matches_state(ctx.level().block_state(pos)?)? {
            return Ok(false);
        }
        match &self.nbt {
            Some(clause) => reached(clause),
            None => Ok(true),
        }
    }
}

/// `LocationPredicate.PositionPredicate`.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionPredicate {
    pub x: DoubleBounds,
    pub y: DoubleBounds,
    pub z: DoubleBounds,
}

/// `LocationPredicate`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocationPredicate {
    pub position: Option<PositionPredicate>,
    pub biomes: Option<HolderSet>,
    pub structures: Option<UnsupportedClause>,
    pub dimension: Option<String>,
    pub smokey: Option<UnsupportedClause>,
    pub light: Option<UnsupportedClause>,
    pub block: Option<BlockPredicate>,
    pub fluid: Option<UnsupportedClause>,
    pub can_see_sky: Option<bool>,
}

/// `BlockPos.containing(x, y, z)`: `Mth.floor` of each coordinate.
pub fn block_pos_containing(x: f64, y: f64, z: f64) -> [i32; 3] {
    fn floor(value: f64) -> i32 {
        let truncated = value as i32;
        if value < f64::from(truncated) {
            truncated.wrapping_sub(1)
        } else {
            truncated
        }
    }
    [floor(x), floor(y), floor(z)]
}

impl LocationPredicate {
    /// `LocationPredicate.matches(ServerLevel, x, y, z)`.
    pub fn matches(&self, ctx: &LootContext<'_>, x: f64, y: f64, z: f64) -> Result<bool> {
        if let Some(position) = &self.position {
            if !(position.x.matches(x) && position.y.matches(y) && position.z.matches(z)) {
                return Ok(false);
            }
        }
        if let Some(dimension) = &self.dimension {
            if *dimension != ctx.level().dimension()? {
                return Ok(false);
            }
        }
        let pos = block_pos_containing(x, y, z);
        let loaded = ctx.level().is_loaded(pos)?;
        if let Some(biomes) = &self.biomes {
            if !(loaded && biomes.contains(Kind::Biome, &ctx.level().biome(pos)?)) {
                return Ok(false);
            }
        }
        if let Some(clause) = &self.structures {
            return reached(clause);
        }
        if let Some(clause) = &self.smokey {
            return reached(clause);
        }
        if let Some(clause) = &self.light {
            return reached(clause);
        }
        if let Some(block) = &self.block {
            if !block.matches(ctx, pos)? {
                return Ok(false);
            }
        }
        if let Some(clause) = &self.fluid {
            return reached(clause);
        }
        match self.can_see_sky {
            Some(expected) => Ok(expected == ctx.level().can_see_sky(pos)?),
            None => Ok(true),
        }
    }
}

/// Used by tests and parsers to build sorted property maps deterministically.
pub fn sorted_properties(map: BTreeMap<String, ValueMatcher>) -> StatePropertiesPredicate {
    StatePropertiesPredicate {
        properties: map.into_iter().collect(),
    }
}
