//! Advancement definitions (`data/<ns>/advancement/**.json`, WP-E5 stage 1): `Advancement.CODEC`
//! with its display, rewards, criteria and requirements, and the trigger-instance codecs of all 56
//! registered triggers (`CriteriaTriggers`), whose predicates reuse the loot port's
//! (`crate::loot::parse`).
//!
//! Accept/reject follows the original's codecs over `RegistryOps<JsonOps>`; it is checked against
//! the executed original by `tools/loot_oracle.java advancement-parse`
//! (`tools/advancement_parse_generate.py`). What is parsed is kept for the trigger and tracker
//! stages; nothing evaluates a trigger yet.

use super::requirements::AdvancementRequirements;
use crate::components::text::{Component, ComponentError};
use crate::loot::condition::Condition;
use crate::loot::facts::facts;
use crate::loot::parse::{
    condition, damage_source_predicate, double_bounds, entity_predicate, int_bounds, item_predicate, location_predicate,
    resource_location, state_properties,
};
use crate::loot::predicate::{
    DamageSourcePredicate, DoubleBounds, EntityPredicate, IntBounds, ItemPredicate, LocationPredicate, StatePropertiesPredicate,
};
use crate::loot::LootError;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Why a definition is rejected, or which part of it this port cannot read yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionError {
    Codec(String),
    Unsupported(String),
}

impl std::fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(what) => write!(f, "invalid advancement: {what}"),
            Self::Unsupported(what) => write!(f, "unsupported advancement part: {what}"),
        }
    }
}

impl std::error::Error for DefinitionError {}

type Result<T> = std::result::Result<T, DefinitionError>;

fn codec<T>(what: impl Into<String>) -> Result<T> {
    Err(DefinitionError::Codec(what.into()))
}

fn from_loot(error: LootError) -> DefinitionError {
    match error {
        LootError::Unsupported(what) => DefinitionError::Unsupported(what),
        other => DefinitionError::Codec(other.to_string()),
    }
}

fn from_component(error: ComponentError) -> DefinitionError {
    match error {
        ComponentError::Codec(what) => DefinitionError::Codec(what),
        ComponentError::Unsupported(what) => DefinitionError::Unsupported(what),
    }
}

/// `JsonOps` reports a JSON `null` member as absent.
fn member<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    map.get(key).filter(|value| !value.is_null())
}

fn object<'a>(value: &'a Value, what: &str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| DefinitionError::Codec(format!("{what}: not a map")))
}

fn string<'a>(value: &'a Value, what: &str) -> Result<&'a str> {
    value.as_str().ok_or_else(|| DefinitionError::Codec(format!("{what}: not a string")))
}

fn boolean(value: &Value, what: &str) -> Result<bool> {
    value.as_bool().ok_or_else(|| DefinitionError::Codec(format!("{what}: not a boolean")))
}

/// `Codec.INT` over `JsonOps` (Gson's `intValue`).
fn int(value: &Value, what: &str) -> Result<i32> {
    match value {
        Value::Number(n) => Ok(crate::components::java::json_number_int_value(n)),
        _ => codec(format!("{what}: not a number")),
    }
}

fn location(value: &Value, what: &str) -> Result<String> {
    resource_location(string(value, what)?, what).map_err(from_loot)
}

fn holder_in(value: &Value, registry: &[String], what: &str) -> Result<String> {
    let id = location(value, what)?;
    if registry.contains(&id) {
        Ok(id)
    } else {
        codec(format!("{what}: Unknown registry key {id}"))
    }
}

fn list<T>(value: &Value, what: &str, each: impl Fn(&Value) -> Result<T>) -> Result<Vec<T>> {
    value.as_array().ok_or_else(|| DefinitionError::Codec(format!("{what}: not a list")))?.iter().map(each).collect()
}

/// `ContextAwarePredicate` as `EntityPredicate.ADVANCEMENT_CODEC` reads it: a list of loot
/// conditions, or else an entity predicate (wrapped as `entity_properties` of `this`).
#[derive(Debug, Clone, PartialEq)]
pub enum ContextAware {
    Conditions(Vec<Condition>),
    Entity(Box<EntityPredicate>),
}

fn context_aware(value: &Value) -> Result<ContextAware> {
    if let Value::Array(items) = value {
        if let Ok(conditions) = items.iter().map(condition).collect::<std::result::Result<Vec<_>, _>>() {
            return Ok(ContextAware::Conditions(conditions));
        }
        // The alternative (an entity predicate) cannot read a list either; report the list's error,
        // keeping an `Unsupported` clause distinguishable.
        let error = items.iter().map(condition).find_map(|r| r.err()).expect("one condition failed");
        return Err(from_loot(error));
    }
    entity_predicate(value).map(|p| ContextAware::Entity(Box::new(p))).map_err(from_loot)
}

/// `ContextAwarePredicate.CODEC`: a list of loot conditions only.
fn condition_list(value: &Value) -> Result<ContextAware> {
    list(value, "conditions", |v| condition(v).map_err(from_loot)).map(ContextAware::Conditions)
}

/// `DistancePredicate`.
#[derive(Debug, Clone, PartialEq)]
pub struct DistancePredicate {
    pub x: DoubleBounds,
    pub y: DoubleBounds,
    pub z: DoubleBounds,
    pub horizontal: DoubleBounds,
    pub absolute: DoubleBounds,
}

fn bounds_d(map: &Map<String, Value>, key: &str) -> Result<DoubleBounds> {
    match member(map, key) {
        Some(value) => double_bounds(value).map_err(from_loot),
        None => double_bounds(&Value::Object(Map::new())).map_err(from_loot),
    }
}

fn bounds_i(map: &Map<String, Value>, key: &str) -> Result<IntBounds> {
    match member(map, key) {
        Some(value) => int_bounds(value).map_err(from_loot),
        None => int_bounds(&Value::Object(Map::new())).map_err(from_loot),
    }
}

fn distance(value: &Value) -> Result<DistancePredicate> {
    let map = object(value, "distance")?;
    Ok(DistancePredicate {
        x: bounds_d(map, "x")?,
        y: bounds_d(map, "y")?,
        z: bounds_d(map, "z")?,
        horizontal: bounds_d(map, "horizontal")?,
        absolute: bounds_d(map, "absolute")?,
    })
}

/// `MobEffectsPredicate.MobEffectInstancePredicate`.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectPredicate {
    pub amplifier: IntBounds,
    pub duration: IntBounds,
    pub ambient: Option<bool>,
    pub visible: Option<bool>,
}

fn effects(value: &Value) -> Result<Vec<(String, EffectPredicate)>> {
    let map = object(value, "effects")?;
    let mut out = Vec::new();
    for (key, body) in map {
        let id = holder_in(&Value::String(key.clone()), &facts().registries.mob_effect, "mob effect")?;
        let body = object(body, "effect")?;
        out.push((
            id,
            EffectPredicate {
                amplifier: bounds_i(body, "amplifier")?,
                duration: bounds_i(body, "duration")?,
                ambient: member(body, "ambient").map(|v| boolean(v, "ambient")).transpose()?,
                visible: member(body, "visible").map(|v| boolean(v, "visible")).transpose()?,
            },
        ));
    }
    Ok(out)
}

/// `DamagePredicate`.
#[derive(Debug, Clone, PartialEq)]
pub struct DamagePredicate {
    pub dealt: DoubleBounds,
    pub taken: DoubleBounds,
    pub source_entity: Option<Box<EntityPredicate>>,
    pub blocked: Option<bool>,
    pub kind: Option<DamageSourcePredicate>,
}

fn damage(value: &Value) -> Result<DamagePredicate> {
    let map = object(value, "damage")?;
    Ok(DamagePredicate {
        dealt: bounds_d(map, "dealt")?,
        taken: bounds_d(map, "taken")?,
        source_entity: member(map, "source_entity").map(|v| entity_predicate(v).map(Box::new).map_err(from_loot)).transpose()?,
        blocked: member(map, "blocked").map(|v| boolean(v, "blocked")).transpose()?,
        kind: member(map, "type").map(|v| damage_source_predicate(v).map_err(from_loot)).transpose()?,
    })
}

/// One decoded trigger-instance field.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Predicate(ContextAware),
    Predicates(Vec<ContextAware>),
    Item(ItemPredicate),
    Items(Vec<ItemPredicate>),
    Location(Box<LocationPredicate>),
    Distance(DistancePredicate),
    Effects(Vec<(String, EffectPredicate)>),
    Damage(Box<DamagePredicate>),
    DamageSource(DamageSourcePredicate),
    State(StatePropertiesPredicate),
    Ints(IntBounds),
    Doubles(DoubleBounds),
    /// A holder or key by id (block, potion, dimension, loot table, recipe).
    Id(String),
    /// `InventoryChangeTrigger.TriggerInstance.Slots`: occupied, full, empty.
    Slots(IntBounds, IntBounds, IntBounds),
}

#[derive(Clone, Copy)]
enum Kind {
    /// `EntityPredicate.ADVANCEMENT_CODEC`.
    Entity,
    /// `EntityPredicate.ADVANCEMENT_CODEC.listOf()` (default empty).
    Entities,
    /// `ContextAwarePredicate.CODEC`.
    Conditions,
    Item,
    /// `ItemPredicate.CODEC.listOf()` (default empty).
    Items,
    Location,
    Distance,
    Effects,
    Damage,
    DamageSource,
    Block,
    State,
    Potion,
    /// `ResourceKey` / `ResourceLocation` of any id.
    Key,
    Ints,
    Doubles,
    Slots,
}

/// `(field, kind, required)` of every registered trigger's `TriggerInstance` codec. Every trigger
/// but `impossible` starts with an optional `player`.
fn trigger_fields(trigger: &str) -> Option<&'static [(&'static str, Kind, bool)]> {
    use Kind::*;
    const PLAYER: &[(&str, Kind, bool)] = &[("player", Entity, false)];
    Some(match trigger {
        "impossible" => &[],
        "location" | "slept_in_bed" | "tick" | "hero_of_the_village" | "voluntary_exile" | "avoid_vibration" | "started_riding" => PLAYER,
        "player_killed_entity" | "entity_killed_player" | "kill_mob_near_sculk_catalyst" => {
            &[("player", Entity, false), ("entity", Entity, false), ("killing_blow", DamageSource, false)]
        }
        "enter_block" | "slide_down_block" => &[("player", Entity, false), ("block", Block, false), ("state", State, false)],
        "inventory_changed" => &[("player", Entity, false), ("slots", Slots, false), ("items", Items, false)],
        "recipe_unlocked" => &[("player", Entity, false), ("recipe", Key, true)],
        "player_hurt_entity" => &[("player", Entity, false), ("damage", Damage, false), ("entity", Entity, false)],
        "entity_hurt_player" => &[("player", Entity, false), ("damage", Damage, false)],
        "enchanted_item" => &[("player", Entity, false), ("item", Item, false), ("levels", Ints, false)],
        "filled_bucket" | "consume_item" | "shot_crossbow" | "used_totem" | "using_item" => &[("player", Entity, false), ("item", Item, false)],
        "brewed_potion" => &[("player", Entity, false), ("potion", Potion, false)],
        "construct_beacon" => &[("player", Entity, false), ("level", Ints, false)],
        "used_ender_eye" => &[("player", Entity, false), ("distance", Doubles, false)],
        "summoned_entity" | "tame_animal" => &[("player", Entity, false), ("entity", Entity, false)],
        "bred_animals" => &[("player", Entity, false), ("parent", Entity, false), ("partner", Entity, false), ("child", Entity, false)],
        "cured_zombie_villager" => &[("player", Entity, false), ("zombie", Entity, false), ("villager", Entity, false)],
        "villager_trade" => &[("player", Entity, false), ("villager", Entity, false), ("item", Item, false)],
        "item_durability_changed" => &[("player", Entity, false), ("item", Item, false), ("durability", Ints, false), ("delta", Ints, false)],
        "levitation" => &[("player", Entity, false), ("distance", Distance, false), ("duration", Ints, false)],
        "changed_dimension" => &[("player", Entity, false), ("from", Key, false), ("to", Key, false)],
        "placed_block" | "item_used_on_block" | "allay_drop_item_on_block" | "default_block_use" | "any_block_use" => {
            &[("player", Entity, false), ("location", Conditions, false)]
        }
        "effects_changed" => &[("player", Entity, false), ("effects", Effects, false), ("source", Entity, false)],
        "nether_travel" | "fall_from_height" | "ride_entity_in_lava" => {
            &[("player", Entity, false), ("start_position", Location, false), ("distance", Distance, false)]
        }
        "fishing_rod_hooked" => &[("player", Entity, false), ("rod", Item, false), ("entity", Entity, false), ("item", Item, false)],
        "channeled_lightning" => &[("player", Entity, false), ("victims", Entities, false)],
        "killed_by_crossbow" => &[("player", Entity, false), ("victims", Entities, false), ("unique_entity_types", Ints, false)],
        "bee_nest_destroyed" => &[("player", Entity, false), ("block", Block, false), ("item", Item, false), ("num_bees_inside", Ints, false)],
        "target_hit" => &[("player", Entity, false), ("signal_strength", Ints, false), ("projectile", Entity, false)],
        "player_generates_container_loot" => &[("player", Entity, false), ("loot_table", Key, true)],
        "thrown_item_picked_up_by_entity" | "thrown_item_picked_up_by_player" => {
            &[("player", Entity, false), ("item", Item, false), ("entity", Entity, false)]
        }
        "player_interacted_with_entity" => &[("player", Entity, false), ("item", Item, false), ("entity", Entity, false)],
        "lightning_strike" => &[("player", Entity, false), ("lightning", Entity, false), ("bystander", Entity, false)],
        "recipe_crafted" | "crafter_recipe_crafted" => &[("player", Entity, false), ("recipe_id", Key, true), ("ingredients", Items, false)],
        "fall_after_explosion" => {
            &[("player", Entity, false), ("start_position", Location, false), ("distance", Distance, false), ("cause", Entity, false)]
        }
        _ => return None,
    })
}

fn decode_field(kind: Kind, value: &Value, name: &str) -> Result<Field> {
    Ok(match kind {
        Kind::Entity => Field::Predicate(context_aware(value)?),
        Kind::Entities => Field::Predicates(list(value, name, context_aware)?),
        Kind::Conditions => Field::Predicate(condition_list(value)?),
        Kind::Item => Field::Item(item_predicate(value).map_err(from_loot)?),
        Kind::Items => Field::Items(list(value, name, |v| item_predicate(v).map_err(from_loot))?),
        Kind::Location => Field::Location(Box::new(location_predicate(value).map_err(from_loot)?)),
        Kind::Distance => Field::Distance(distance(value)?),
        Kind::Effects => Field::Effects(effects(value)?),
        Kind::Damage => Field::Damage(Box::new(damage(value)?)),
        Kind::DamageSource => Field::DamageSource(damage_source_predicate(value).map_err(from_loot)?),
        Kind::Block => Field::Id(holder_in(value, &facts().registries.block, name)?),
        Kind::State => Field::State(state_properties(value).map_err(from_loot)?),
        Kind::Potion => Field::Id(holder_in(value, &facts().registries.potion, name)?),
        Kind::Key => Field::Id(location(value, name)?),
        Kind::Ints => Field::Ints(int_bounds(value).map_err(from_loot)?),
        Kind::Doubles => Field::Doubles(double_bounds(value).map_err(from_loot)?),
        Kind::Slots => {
            let map = object(value, name)?;
            Field::Slots(bounds_i(map, "occupied")?, bounds_i(map, "full")?, bounds_i(map, "empty")?)
        }
    })
}

/// A criterion: its trigger and decoded conditions (absent optional fields are left out).
#[derive(Debug, Clone, PartialEq)]
pub struct Criterion {
    pub trigger: String,
    pub conditions: BTreeMap<&'static str, Field>,
}

/// `Criterion.CODEC` (`ExtraCodecs.dispatchOptionalValue("trigger", "conditions", …)`): a missing
/// `conditions` member is an empty map.
fn criterion(value: &Value) -> Result<Criterion> {
    let map = object(value, "criterion")?;
    let trigger = location(member(map, "trigger").ok_or_else(|| DefinitionError::Codec("Input does not contain a key [trigger]".into()))?, "trigger")?;
    let short = trigger.strip_prefix("minecraft:").filter(|_| trigger.starts_with("minecraft:"));
    let Some(fields) = short.and_then(trigger_fields) else {
        return codec(format!("No key {trigger} in registry minecraft:trigger_type"));
    };
    let empty = Value::Object(Map::new());
    let conditions = member(map, "conditions").unwrap_or(&empty);
    let body = object(conditions, "conditions")?;
    let mut decoded = BTreeMap::new();
    for (name, kind, required) in fields {
        match member(body, name) {
            Some(value) => {
                decoded.insert(*name, decode_field(*kind, value, name)?);
            }
            None if *required => return codec(format!("No key {name} in MapLike")),
            None => {}
        }
    }
    Ok(Criterion { trigger, conditions: decoded })
}

/// `AdvancementType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    Task,
    Challenge,
    Goal,
}

/// `ItemStack.STRICT_CODEC` for a display icon. Component values are validated for the component
/// types vanilla icons use; any other component type answers `Unsupported`.
#[derive(Debug, Clone, PartialEq)]
pub struct Icon {
    pub item: String,
    pub count: i32,
    pub components: Vec<(String, Value)>,
}

fn icon(value: &Value) -> Result<Icon> {
    let map = object(value, "icon")?;
    let item = holder_in(member(map, "id").ok_or_else(|| DefinitionError::Codec("No key id in MapLike".into()))?, &facts().registries.item, "id")?;
    if item == "minecraft:air" {
        return codec("Item must not be minecraft:air");
    }
    // `intRange(1, 99).fieldOf("count").orElse(1)`: any unreadable or out-of-range count is 1.
    let count = member(map, "count").and_then(|v| int(v, "count").ok()).filter(|n| (1..=99).contains(n)).unwrap_or(1);
    // `ItemStack.validateStrict`: the count may not exceed the item's maximum stack size.
    let maximum = crate::loot::enchant::new_stack(&item)
        .map_err(from_loot)?
        .max_stack_size()
        .map_err(|e| DefinitionError::Codec(e.to_string()))?;
    if count > maximum {
        return codec(format!("Item stack with stack size of {count} was larger than maximum: {maximum}"));
    }
    let mut components = Vec::new();
    if let Some(patch) = member(map, "components") {
        for (key, body) in object(patch, "components")? {
            let name = key.strip_prefix('!').unwrap_or(key);
            let name = resource_location(name, "component").map_err(from_loot)?;
            if crate::loot::function::component_id(&name).is_none() || !name.starts_with("minecraft:") || key.starts_with('!') {
                if crate::loot::function::component_id(&name).is_none() {
                    return codec(format!("No key {name} in registry minecraft:data_component_type"));
                }
                return Err(DefinitionError::Unsupported(format!("icon component {key}")));
            }
            match name.as_str() {
                "minecraft:item_name" => {
                    flat_component(body)?;
                }
                "minecraft:hide_additional_tooltip" => {
                    object(body, "hide_additional_tooltip")?;
                }
                "minecraft:pot_decorations" => {
                    let items = list(body, "pot_decorations", |v| holder_in(v, &facts().registries.item, "decoration"))?;
                    if items.len() > 4 {
                        return codec(format!("List is too long: {}, expected range [0-4]", items.len()));
                    }
                }
                "minecraft:banner_patterns" => {
                    list(body, "banner_patterns", |layer| {
                        let layer = object(layer, "layer")?;
                        let pattern = member(layer, "pattern").ok_or_else(|| DefinitionError::Codec("No key pattern in MapLike".into()))?;
                        match pattern {
                            Value::String(_) => {
                                let id = location(pattern, "pattern")?;
                                let bare = id.strip_prefix("minecraft:").unwrap_or("");
                                if !crate::loot::function::BANNER_PATTERNS.contains(&bare) {
                                    return codec(format!("Failed to get element {id}"));
                                }
                            }
                            _ => return Err(DefinitionError::Unsupported("inline banner pattern in an icon".into())),
                        }
                        let color = string(member(layer, "color").ok_or_else(|| DefinitionError::Codec("No key color in MapLike".into()))?, "color")?;
                        if !crate::loot::function::DYE_COLORS.contains(&color) {
                            return codec(format!("Unknown element name:{color}"));
                        }
                        Ok(())
                    })?;
                }
                _ => return Err(DefinitionError::Unsupported(format!("icon component {name}"))),
            }
            components.push((name, body.clone()));
        }
    }
    Ok(Icon { item, count, components })
}

/// `ComponentSerialization.FLAT_CODEC` (the persistent codec of `item_name`, `custom_name`, …): a
/// string holding the component's JSON, read with the lenient Gson reader.
pub fn flat_component(value: &Value) -> Result<Component> {
    let text = string(value, "component")?;
    let json = crate::components::lenient::parse(text).map_err(DefinitionError::Codec)?;
    Component::from_json(&json).map_err(from_component)
}

/// `DisplayInfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    pub icon: Icon,
    pub title: Component,
    pub description: Component,
    pub background: Option<String>,
    pub frame: Frame,
    pub show_toast: bool,
    pub announce_to_chat: bool,
    pub hidden: bool,
}

fn display(value: &Value) -> Result<Display> {
    let map = object(value, "display")?;
    let need = |key: &str| member(map, key).ok_or_else(|| DefinitionError::Codec(format!("No key {key} in MapLike")));
    let flag = |key: &str, default: bool| -> Result<bool> { member(map, key).map(|v| boolean(v, key)).transpose().map(|v| v.unwrap_or(default)) };
    Ok(Display {
        icon: icon(need("icon")?)?,
        title: Component::from_json(need("title")?).map_err(from_component)?,
        description: Component::from_json(need("description")?).map_err(from_component)?,
        background: member(map, "background").map(|v| location(v, "background")).transpose()?,
        frame: match member(map, "frame").map(|v| string(v, "frame")).transpose()? {
            None | Some("task") => Frame::Task,
            Some("challenge") => Frame::Challenge,
            Some("goal") => Frame::Goal,
            Some(other) => return codec(format!("Unknown element name:{other}")),
        },
        show_toast: flag("show_toast", true)?,
        announce_to_chat: flag("announce_to_chat", true)?,
        hidden: flag("hidden", false)?,
    })
}

/// `AdvancementRewards`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rewards {
    pub experience: i32,
    pub loot: Vec<String>,
    pub recipes: Vec<String>,
    pub function: Option<String>,
}

fn rewards(value: &Value) -> Result<Rewards> {
    let map = object(value, "rewards")?;
    Ok(Rewards {
        experience: member(map, "experience").map(|v| int(v, "experience")).transpose()?.unwrap_or(0),
        loot: member(map, "loot").map(|v| list(v, "loot", |v| location(v, "loot"))).transpose()?.unwrap_or_default(),
        recipes: member(map, "recipes").map(|v| list(v, "recipes", |v| location(v, "recipes"))).transpose()?.unwrap_or_default(),
        function: member(map, "function").map(|v| location(v, "function")).transpose()?,
    })
}

/// `Advancement`.
#[derive(Debug, Clone, PartialEq)]
pub struct Advancement {
    pub parent: Option<String>,
    pub display: Option<Display>,
    pub rewards: Rewards,
    /// In the original's `HashMap` the order is not observable here; kept sorted by name.
    pub criteria: BTreeMap<String, Criterion>,
    pub requirements: AdvancementRequirements,
    pub sends_telemetry_event: bool,
}

/// `Advancement.CODEC` (with its `validate`) over one parsed JSON file.
pub fn parse(value: &Value) -> Result<Advancement> {
    let map = object(value, "advancement")?;
    let parent = member(map, "parent").map(|v| location(v, "parent")).transpose()?;
    let display = member(map, "display").map(display).transpose()?;
    let rewards = member(map, "rewards").map(rewards).transpose()?.unwrap_or_default();
    let criteria_value = member(map, "criteria").ok_or_else(|| DefinitionError::Codec("No key criteria in MapLike".into()))?;
    let mut criteria = BTreeMap::new();
    for (name, body) in object(criteria_value, "criteria")? {
        criteria.insert(name.clone(), criterion(body)?);
    }
    if criteria.is_empty() {
        return codec("Advancement criteria cannot be empty");
    }
    let requirements = match member(map, "requirements") {
        Some(value) => AdvancementRequirements::new(list(value, "requirements", |group| list(group, "requirement", |v| string(v, "requirement").map(str::to_owned)))?),
        None => AdvancementRequirements::all_of(criteria.keys().map(String::as_str)),
    };
    let sends_telemetry_event = member(map, "sends_telemetry_event").map(|v| boolean(v, "sends_telemetry_event")).transpose()?.unwrap_or(false);
    let names: BTreeSet<&str> = criteria.keys().map(String::as_str).collect();
    requirements.validate(&names).map_err(DefinitionError::Codec)?;
    Ok(Advancement { parent, display, rewards, criteria, requirements, sends_telemetry_event })
}
