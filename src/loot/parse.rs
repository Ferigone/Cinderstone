//! JSON → loot structures, following the Java codecs' accept/reject behaviour.
//!
//! Unknown object members are ignored (record codecs do), required members are
//! required, and optional members take the codec default. A registered type that
//! this port does not implement still parses and fails only when it is executed.
//! Object member order is serde_json's (sorted); that is observable only if two
//! sibling number providers both consume randomness.

use super::condition::Condition;
use super::context::{param_set, EntityTarget, ParamSet, GENERIC};
use super::entry::{Entry, LootPool, LootTable, SingletonKind, TableRef};
use super::facts::{canonical, facts, HolderSet, Kind};
use super::function::{
    component_id, BonusFormula, ComponentSource, Function, FunctionKind, ListOperation, NameTarget, StewEffect,
    TextComponent, AttributeModifierSpec, ContainerComponent, CopyOperation, MergeStrategy, NameSource, NbtSource, BannerLayer, Filterable, ATTRIBUTES, ATTRIBUTE_OPERATIONS, SLOT_GROUPS, BannerPatternRef, FireworkExplosion, BANNER_PATTERNS, BLOCK_ENTITY_TYPES,
    DYE_COLORS, FIREWORK_SHAPES, TOOLTIP_TOGGLES,
};
use super::number::{IntRange, LevelBasedValue, NumberProvider, ScoreboardName};
use super::predicate::*;
use super::{codec, LootError, Result};
use serde_json::{Map, Value};

const CONDITION_TYPES: [&str; 19] = [
    "inverted", "any_of", "all_of", "random_chance", "random_chance_with_enchanted_bonus",
    "entity_properties", "killed_by_player", "entity_scores", "block_state_property", "match_tool",
    "table_bonus", "survives_explosion", "damage_source_properties", "location_check",
    "weather_check", "reference", "time_check", "value_check", "enchantment_active_check",
];

const FUNCTION_TYPES: [&str; 40] = [
    "set_count", "set_item", "enchant_with_levels", "enchant_randomly", "set_enchantments",
    "set_custom_data", "set_components", "furnace_smelt", "enchanted_count_increase", "set_damage",
    "set_attributes", "set_name", "exploration_map", "set_stew_effect", "copy_name", "set_contents",
    "modify_contents", "filtered", "limit_count", "apply_bonus", "set_loot_table",
    "explosion_decay", "set_lore", "fill_player_head", "copy_custom_data", "copy_state",
    "set_banner_pattern", "set_potion", "set_instrument", "reference", "sequence",
    "copy_components", "set_fireworks", "set_firework_explosion", "set_book_cover",
    "set_written_book_pages", "set_writable_book_pages", "toggle_tooltips",
    "set_ominous_bottle_amplifier", "set_custom_model_data",
];

fn object<'a>(value: &'a Value, what: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| LootError::Codec(format!("{what}: expected an object")))
}

/// `JsonOps.JsonMapLike.get` reports a JSON `null` member as absent, so every
/// optional member treats `null` as missing and every required member rejects it.
fn member<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    map.get(key).filter(|value| !value.is_null())
}

fn required<'a>(map: &'a Map<String, Value>, key: &str, what: &str) -> Result<&'a Value> {
    member(map, key)
        .ok_or_else(|| LootError::Codec(format!("{what}: missing required field \"{key}\"")))
}

fn float(value: &Value, what: &str) -> Result<f32> {
    value
        .as_f64()
        .map(|number| number as f32)
        .ok_or_else(|| LootError::Codec(format!("{what}: expected a number")))
}

fn double(value: &Value, what: &str) -> Result<f64> {
    value
        .as_f64()
        .ok_or_else(|| LootError::Codec(format!("{what}: expected a number")))
}

/// `Codec.INT`: any JSON number, narrowed with `Number.intValue()`.
fn int(value: &Value, what: &str) -> Result<i32> {
    if let Some(integer) = value.as_i64() {
        Ok(integer as i32)
    } else if let Some(number) = value.as_f64() {
        Ok(number as i32)
    } else {
        codec(format!("{what}: expected an integer"))
    }
}

/// `Codec.LONG` over `JsonOps` (Gson's `longValue`).
fn long(value: &Value, what: &str) -> Result<i64> {
    match value {
        Value::Number(n) => Ok(crate::components::java::json_number_long_value(n)),
        _ => codec(format!("{what}: expected a long")),
    }
}

fn boolean(value: &Value, what: &str) -> Result<bool> {
    value
        .as_bool()
        .ok_or_else(|| LootError::Codec(format!("{what}: expected a boolean")))
}

fn string<'a>(value: &'a Value, what: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| LootError::Codec(format!("{what}: expected a string")))
}

fn optional<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    member(map, key)
}

/// `Codec.BOOL.fieldOf(key).orElse(default)`: a missing, null or wrong-typed
/// member is the default (`MapCodec.orElse` turns every decode error into it).
fn bool_or_else(map: &Map<String, Value>, key: &str, default: bool) -> bool {
    member(map, key).and_then(Value::as_bool).unwrap_or(default)
}

/// `ResourceLocation.CODEC`: validate and canonicalise.
pub fn resource_location(text: &str, what: &str) -> Result<String> {
    let (namespace, path) = match text.split_once(':') {
        Some((namespace, path)) => (namespace, path),
        None => ("minecraft", text),
    };
    let namespace_ok = !namespace.is_empty()
        && namespace
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.'));
    let path_ok = !path.is_empty()
        && path
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '/'));
    if namespace_ok && path_ok {
        Ok(format!("{namespace}:{path}"))
    } else {
        codec(format!("{what}: invalid resource location \"{text}\""))
    }
}

fn id(value: &Value, what: &str) -> Result<String> {
    resource_location(string(value, what)?, what)
}

/// A registered holder by name (`RegistryFixedCodec` / `holderByNameCodec`).
fn registered(value: &Value, kind: Kind, what: &str) -> Result<String> {
    let name = id(value, what)?;
    let known = match kind {
        Kind::Item => facts().registries.item.contains(&name),
        Kind::Block => facts().registries.block.contains(&name),
        Kind::Enchantment => facts().enchantment_id(&name).is_some(),
        Kind::EntityType => facts().registries.entity_type.contains(&name),
        Kind::Biome => facts().registries.biome.contains(&name),
        Kind::DamageType => facts().registries.damage_type.contains(&name),
        Kind::Instrument => facts().registries.instrument.contains(&name),
    };
    if known {
        Ok(name)
    } else {
        codec(format!("{what}: unknown registry entry {name}"))
    }
}

/// `RegistryCodecs.homogeneousList`: `#tag`, one ID, or a list of IDs.
fn holder_set(value: &Value, kind: Kind, what: &str) -> Result<HolderSet> {
    match value {
        Value::String(text) => match text.strip_prefix('#') {
            Some(tag) => {
                // `HolderSetCodec` resolves the tag through the registry lookup, which
                // fails with "Missing tag" for a tag the data pack does not define.
                let tag = resource_location(tag, what)?;
                if facts().tag(kind, &tag).is_none() {
                    return codec(format!("{what}: missing tag {tag}"));
                }
                Ok(HolderSet::Tag(tag))
            }
            None => Ok(HolderSet::Direct(vec![registered(value, kind, what)?])),
        },
        Value::Array(items) => items
            .iter()
            .map(|item| registered(item, kind, what))
            .collect::<Result<Vec<_>>>()
            .map(HolderSet::Direct),
        _ => codec(format!("{what}: expected a holder set")),
    }
}

/// `TagKey.hashedCodec`.
fn hashed_tag(value: &Value, what: &str) -> Result<HolderSet> {
    let text = string(value, what)?;
    match text.strip_prefix('#') {
        Some(tag) => Ok(HolderSet::Tag(resource_location(tag, what)?)),
        None => codec(format!("{what}: expected a #tag")),
    }
}

// ---------------------------------------------------------------- number providers

fn entity_target(value: &Value) -> Result<EntityTarget> {
    let text = string(value, "entity target")?;
    EntityTarget::by_name(text)
        .ok_or_else(|| LootError::Codec(format!("unknown entity target {text}")))
}

fn type_name(map: &Map<String, Value>, key: &str, what: &str) -> Result<String> {
    id(required(map, key, what)?, what)
}

fn scoreboard_name(value: &Value) -> Result<ScoreboardName> {
    if let Value::String(_) = value {
        return Ok(ScoreboardName::Context(entity_target(value)?));
    }
    let map = object(value, "scoreboard name provider")?;
    match type_name(map, "type", "scoreboard name provider")?.as_str() {
        "minecraft:fixed" => Ok(ScoreboardName::Fixed(
            string(required(map, "name", "fixed score holder")?, "name")?.to_owned(),
        )),
        "minecraft:context" => Ok(ScoreboardName::Context(entity_target(required(
            map,
            "target",
            "context score holder",
        )?)?)),
        other => codec(format!("unknown scoreboard name provider {other}")),
    }
}

fn typed_number_provider(map: &Map<String, Value>) -> Result<NumberProvider> {
    match type_name(map, "type", "number provider")?.as_str() {
        "minecraft:constant" => Ok(NumberProvider::Constant(float(
            required(map, "value", "constant")?,
            "value",
        )?)),
        "minecraft:uniform" => uniform_provider(map),
        "minecraft:binomial" => Ok(NumberProvider::Binomial(
            Box::new(number_provider(required(map, "n", "binomial")?)?),
            Box::new(number_provider(required(map, "p", "binomial")?)?),
        )),
        "minecraft:score" => Ok(NumberProvider::Score {
            target: scoreboard_name(required(map, "target", "score provider")?)?,
            score: string(required(map, "score", "score provider")?, "score")?.to_owned(),
            scale: member(map, "scale")
                .and_then(Value::as_f64)
                .map_or(1.0, |scale| scale as f32),
        }),
        "minecraft:storage" => Ok(NumberProvider::Storage {
            storage: id(required(map, "storage", "storage provider")?, "storage")?,
            path: string(required(map, "path", "storage provider")?, "path")?.to_owned(),
        }),
        "minecraft:enchantment_level" => Ok(NumberProvider::EnchantmentLevel(
            level_based_value(required(map, "amount", "enchantment_level")?)?,
        )),
        other => codec(format!("unknown number provider type {other}")),
    }
}

fn uniform_provider(map: &Map<String, Value>) -> Result<NumberProvider> {
    Ok(NumberProvider::Uniform(
        Box::new(number_provider(required(map, "min", "uniform")?)?),
        Box::new(number_provider(required(map, "max", "uniform")?)?),
    ))
}

/// `NumberProviders.CODEC`: a bare number is a constant; an object is a typed
/// provider, or a `uniform` when it has no usable `type`.
pub fn number_provider(value: &Value) -> Result<NumberProvider> {
    match value {
        Value::Number(_) => Ok(NumberProvider::Constant(float(value, "number provider")?)),
        Value::Object(map) => match typed_number_provider(map) {
            Ok(provider) => Ok(provider),
            Err(typed_error) => {
                if member(map, "min").is_some() && member(map, "max").is_some() {
                    uniform_provider(map).or(Err(typed_error))
                } else {
                    Err(typed_error)
                }
            }
        },
        _ => codec("number provider: expected a number or object"),
    }
}

pub fn level_based_value(value: &Value) -> Result<LevelBasedValue> {
    if value.is_number() {
        return Ok(LevelBasedValue::Constant(float(value, "level based value")?));
    }
    let map = object(value, "level based value")?;
    match type_name(map, "type", "level based value")?.as_str() {
        // `LevelBasedValue.bootstrap` registers no "constant" type: a constant is
        // only the bare-number form handled above, and the original rejects
        // {"type":"minecraft:constant"} (differential case custom24).
        "minecraft:linear" => Ok(LevelBasedValue::Linear {
            base: float(required(map, "base", "linear")?, "base")?,
            per_level_above_first: float(
                required(map, "per_level_above_first", "linear")?,
                "per_level_above_first",
            )?,
        }),
        "minecraft:levels_squared" => Ok(LevelBasedValue::LevelsSquared(float(
            required(map, "added", "levels_squared")?,
            "added",
        )?)),
        "minecraft:fraction" => Ok(LevelBasedValue::Fraction {
            numerator: Box::new(level_based_value(required(map, "numerator", "fraction")?)?),
            denominator: Box::new(level_based_value(required(map, "denominator", "fraction")?)?),
        }),
        "minecraft:clamped" => {
            let min = float(required(map, "min", "clamped")?, "min")?;
            let max = float(required(map, "max", "clamped")?, "max")?;
            if max <= min {
                return codec(format!("Max must be larger than min, min: {min}, max: {max}"));
            }
            Ok(LevelBasedValue::Clamped {
                value: Box::new(level_based_value(required(map, "value", "clamped")?)?),
                min,
                max,
            })
        }
        "minecraft:lookup" => {
            let values = required(map, "values", "lookup")?
                .as_array()
                .ok_or_else(|| LootError::Codec("lookup values: expected a list".to_owned()))?
                .iter()
                .map(|v| float(v, "lookup value"))
                .collect::<Result<Vec<_>>>()?;
            Ok(LevelBasedValue::Lookup {
                values,
                fallback: Box::new(level_based_value(required(map, "fallback", "lookup")?)?),
            })
        }
        other => codec(format!("unknown level based value type {other}")),
    }
}

/// `IntRange.CODEC`.
pub fn int_range(value: &Value) -> Result<IntRange> {
    if value.is_number() {
        let constant = NumberProvider::Constant(int(value, "int range")? as f32);
        return Ok(IntRange {
            min: Some(constant.clone()),
            max: Some(constant),
        });
    }
    let map = object(value, "int range")?;
    Ok(IntRange {
        min: optional(map, "min").map(number_provider).transpose()?,
        max: optional(map, "max").map(number_provider).transpose()?,
    })
}

// ---------------------------------------------------------------- predicates

pub fn int_bounds(value: &Value) -> Result<IntBounds> {
    if value.is_number() {
        let exact = int(value, "bounds")?;
        return Ok(IntBounds {
            min: Some(exact),
            max: Some(exact),
        });
    }
    let map = object(value, "bounds")?;
    Ok(IntBounds {
        min: optional(map, "min").map(|v| int(v, "min")).transpose()?,
        max: optional(map, "max").map(|v| int(v, "max")).transpose()?,
    })
}

pub fn double_bounds(value: &Value) -> Result<DoubleBounds> {
    if value.is_number() {
        let exact = double(value, "bounds")?;
        return Ok(DoubleBounds {
            min: Some(exact),
            max: Some(exact),
        });
    }
    let map = object(value, "bounds")?;
    Ok(DoubleBounds {
        min: optional(map, "min").map(|v| double(v, "min")).transpose()?,
        max: optional(map, "max").map(|v| double(v, "max")).transpose()?,
    })
}

fn unsupported_clause(name: &str) -> UnsupportedClause {
    UnsupportedClause(name.to_owned())
}

fn enchantment_predicate(value: &Value) -> Result<EnchantmentPredicate> {
    let map = object(value, "enchantment predicate")?;
    Ok(EnchantmentPredicate {
        enchantments: optional(map, "enchantments")
            .map(|v| holder_set(v, Kind::Enchantment, "enchantments"))
            .transpose()?,
        levels: match optional(map, "levels") {
            Some(levels) => int_bounds(levels)?,
            None => IntBounds::ANY,
        },
    })
}

fn enchantment_predicates(value: &Value) -> Result<Vec<EnchantmentPredicate>> {
    value
        .as_array()
        .ok_or_else(|| LootError::Codec("enchantments predicate: expected a list".to_owned()))?
        .iter()
        .map(enchantment_predicate)
        .collect()
}

pub fn item_predicate(value: &Value) -> Result<ItemPredicate> {
    let map = object(value, "item predicate")?;
    let mut sub_predicates = Vec::new();
    if let Some(sub) = optional(map, "predicates") {
        for (name, body) in object(sub, "item sub predicates")? {
            let name = resource_location(name, "item sub predicate")?;
            sub_predicates.push(match name.as_str() {
                "minecraft:enchantments" => {
                    ItemSubPredicate::Enchantments(enchantment_predicates(body)?)
                }
                "minecraft:stored_enchantments" => {
                    ItemSubPredicate::StoredEnchantments(enchantment_predicates(body)?)
                }
                "minecraft:damage" => {
                    let damage = object(body, "damage predicate")?;
                    ItemSubPredicate::Damage {
                        durability: match optional(damage, "durability") {
                            Some(v) => int_bounds(v)?,
                            None => IntBounds::ANY,
                        },
                        damage: match optional(damage, "damage") {
                            Some(v) => int_bounds(v)?,
                            None => IntBounds::ANY,
                        },
                    }
                }
                "minecraft:potion_contents" | "minecraft:custom_data" | "minecraft:container"
                | "minecraft:bundle_contents" | "minecraft:firework_explosion"
                | "minecraft:fireworks" | "minecraft:writable_book_content"
                | "minecraft:written_book_content" | "minecraft:attribute_modifiers"
                | "minecraft:trim" | "minecraft:jukebox_playable" => {
                    ItemSubPredicate::Unsupported(unsupported_clause(&format!("item.{name}")))
                }
                other => return codec(format!("unknown item sub predicate {other}")),
            });
        }
    }
    let components = optional(map, "components")
        .filter(|v| v.as_object().is_some_and(|object| !object.is_empty()))
        .map(|_| unsupported_clause("item.components"));
    Ok(ItemPredicate {
        items: optional(map, "items")
            .map(|v| holder_set(v, Kind::Item, "items"))
            .transpose()?,
        count: match optional(map, "count") {
            Some(count) => int_bounds(count)?,
            None => IntBounds::ANY,
        },
        components,
        sub_predicates,
    })
}

fn flags_predicate(value: &Value) -> Result<EntityFlagsPredicate> {
    let map = object(value, "entity flags")?;
    let flag = |key: &str| -> Result<Option<bool>> {
        optional(map, key).map(|v| boolean(v, key)).transpose()
    };
    Ok(EntityFlagsPredicate {
        on_ground: flag("is_on_ground")?,
        on_fire: flag("is_on_fire")?,
        sneaking: flag("is_sneaking")?,
        sprinting: flag("is_sprinting")?,
        swimming: flag("is_swimming")?,
        flying: flag("is_flying")?,
        baby: flag("is_baby")?,
    })
}

fn equipment_predicate(value: &Value) -> Result<EntityEquipmentPredicate> {
    let map = object(value, "entity equipment")?;
    let slot = |key: &str| -> Result<Option<ItemPredicate>> {
        optional(map, key).map(item_predicate).transpose()
    };
    Ok(EntityEquipmentPredicate {
        head: slot("head")?,
        chest: slot("chest")?,
        legs: slot("legs")?,
        feet: slot("feet")?,
        body: slot("body")?,
        mainhand: slot("mainhand")?,
        offhand: slot("offhand")?,
    })
}

fn entity_sub_predicate(value: &Value) -> Result<EntitySubPredicate> {
    let map = object(value, "type specific predicate")?;
    let kind = type_name(map, "type", "type specific predicate")?;
    let short = kind.strip_prefix("minecraft:").unwrap_or(&kind).to_owned();
    match short.as_str() {
        "slime" => Ok(EntitySubPredicate::Slime {
            size: match optional(map, "size") {
                Some(size) => int_bounds(size)?,
                None => IntBounds::ANY,
            },
        }),
        "fishing_hook" => Ok(EntitySubPredicate::FishingHook {
            in_open_water: optional(map, "in_open_water")
                .map(|v| boolean(v, "in_open_water"))
                .transpose()?,
        }),
        "axolotl" | "boat" | "fox" | "mooshroom" | "rabbit" | "horse" | "llama" | "villager"
        | "parrot" | "tropical_fish" | "painting" | "cat" | "frog" | "wolf" => {
            Ok(EntitySubPredicate::Variant {
                kind: short,
                value: id(required(map, "variant", "variant predicate")?, "variant")?,
            })
        }
        "lightning" | "player" | "raider" => Ok(EntitySubPredicate::Unsupported(unsupported_clause(
            &format!("type_specific.{short}"),
        ))),
        other => codec(format!("unknown entity sub predicate {other}")),
    }
}

pub fn entity_predicate(value: &Value) -> Result<EntityPredicate> {
    let map = object(value, "entity predicate")?;
    let clause = |key: &str| -> Option<UnsupportedClause> {
        optional(map, key).map(|_| unsupported_clause(&format!("entity.{key}")))
    };
    Ok(EntityPredicate {
        entity_type: optional(map, "type")
            .map(|v| holder_set(v, Kind::EntityType, "type").map(EntityTypePredicate))
            .transpose()?,
        distance: clause("distance"),
        movement: clause("movement"),
        located: optional(map, "location")
            .map(|v| location_predicate(v).map(Box::new))
            .transpose()?,
        stepping_on: clause("stepping_on"),
        movement_affected_by: clause("movement_affected_by"),
        effects: clause("effects"),
        flags: optional(map, "flags").map(flags_predicate).transpose()?,
        equipment: optional(map, "equipment").map(equipment_predicate).transpose()?,
        sub_predicate: optional(map, "type_specific")
            .map(entity_sub_predicate)
            .transpose()?,
        vehicle: clause("vehicle"),
        passenger: clause("passenger"),
        targeted_entity: clause("targeted_entity"),
        periodic_tick: clause("periodic_tick"),
        team: clause("team"),
        slots: clause("slots"),
        nbt: clause("nbt"),
    })
}

pub fn damage_source_predicate(value: &Value) -> Result<DamageSourcePredicate> {
    let map = object(value, "damage source predicate")?;
    let mut tags = Vec::new();
    if let Some(list) = optional(map, "tags") {
        for entry in list
            .as_array()
            .ok_or_else(|| LootError::Codec("damage tags: expected a list".to_owned()))?
        {
            let entry = object(entry, "damage tag predicate")?;
            tags.push(DamageTagPredicate {
                tag: id(required(entry, "id", "damage tag predicate")?, "id")?,
                expected: boolean(required(entry, "expected", "damage tag predicate")?, "expected")?,
            });
        }
    }
    Ok(DamageSourcePredicate {
        tags,
        direct_entity: optional(map, "direct_entity").map(entity_predicate).transpose()?,
        source_entity: optional(map, "source_entity").map(entity_predicate).transpose()?,
        is_direct: optional(map, "is_direct").map(|v| boolean(v, "is_direct")).transpose()?,
    })
}

pub fn state_properties(value: &Value) -> Result<StatePropertiesPredicate> {
    let map = object(value, "state properties predicate")?;
    let mut properties = Vec::new();
    for (name, matcher) in map {
        properties.push((
            name.clone(),
            match matcher {
                Value::String(text) => ValueMatcher::Exact(text.clone()),
                Value::Object(range) => ValueMatcher::Ranged {
                    min: optional(range, "min").map(|v| string(v, "min").map(str::to_owned)).transpose()?,
                    max: optional(range, "max").map(|v| string(v, "max").map(str::to_owned)).transpose()?,
                },
                _ => return codec("state property matcher: expected a string or range"),
            },
        ));
    }
    Ok(StatePropertiesPredicate { properties })
}

fn block_predicate(value: &Value) -> Result<BlockPredicate> {
    let map = object(value, "block predicate")?;
    Ok(BlockPredicate {
        blocks: optional(map, "blocks")
            .map(|v| holder_set(v, Kind::Block, "blocks"))
            .transpose()?,
        state: optional(map, "state").map(state_properties).transpose()?,
        nbt: optional(map, "nbt").map(|_| unsupported_clause("block.nbt")),
    })
}

pub fn location_predicate(value: &Value) -> Result<LocationPredicate> {
    let map = object(value, "location predicate")?;
    let clause = |key: &str| -> Option<UnsupportedClause> {
        optional(map, key).map(|_| unsupported_clause(&format!("location.{key}")))
    };
    let position = match optional(map, "position") {
        None => None,
        Some(position) => {
            let position = object(position, "position predicate")?;
            let axis = |key: &str| -> Result<DoubleBounds> {
                match optional(position, key) {
                    Some(bounds) => double_bounds(bounds),
                    None => Ok(DoubleBounds::ANY),
                }
            };
            let (x, y, z) = (axis("x")?, axis("y")?, axis("z")?);
            // `PositionPredicate.of` is not applied by the codec; keep what is written.
            Some(PositionPredicate { x, y, z })
        }
    };
    Ok(LocationPredicate {
        position,
        biomes: optional(map, "biomes")
            .map(|v| holder_set(v, Kind::Biome, "biomes"))
            .transpose()?,
        structures: clause("structures"),
        dimension: optional(map, "dimension").map(|v| id(v, "dimension")).transpose()?,
        smokey: clause("smokey"),
        light: clause("light"),
        block: optional(map, "block").map(block_predicate).transpose()?,
        fluid: clause("fluid"),
        can_see_sky: optional(map, "can_see_sky")
            .map(|v| boolean(v, "can_see_sky"))
            .transpose()?,
    })
}

// ---------------------------------------------------------------- conditions

fn condition_list(value: &Value, what: &str) -> Result<Vec<Condition>> {
    value
        .as_array()
        .ok_or_else(|| LootError::Codec(format!("{what}: expected a list of conditions")))?
        .iter()
        .map(condition)
        .collect()
}

fn conditions_field(map: &Map<String, Value>) -> Result<Vec<Condition>> {
    match optional(map, "conditions") {
        Some(list) => condition_list(list, "conditions"),
        None => Ok(Vec::new()),
    }
}

fn unit_interval(value: &Value, what: &str) -> Result<f32> {
    let number = float(value, what)?;
    if (0.0..=1.0).contains(&number) {
        Ok(number)
    } else {
        codec(format!("{what}: value {number} outside of range [0.0:1.0]"))
    }
}

/// `LootItemCondition.DIRECT_CODEC`: a list is an inline `all_of`.
pub fn condition(value: &Value) -> Result<Condition> {
    if let Value::Array(_) = value {
        return Ok(Condition::AllOf(condition_list(value, "all_of")?));
    }
    let map = object(value, "condition")?;
    let name = type_name(map, "condition", "condition")?;
    let short = name.strip_prefix("minecraft:").unwrap_or(&name);
    if !CONDITION_TYPES.contains(&short) || !name.starts_with("minecraft:") {
        return codec(format!("unknown condition type {name}"));
    }
    Ok(match short {
        "inverted" => Condition::Inverted(Box::new(condition(required(map, "term", "inverted")?)?)),
        "any_of" => Condition::AnyOf(condition_list(required(map, "terms", "any_of")?, "terms")?),
        "all_of" => Condition::AllOf(condition_list(required(map, "terms", "all_of")?, "terms")?),
        "random_chance" => Condition::RandomChance(number_provider(required(map, "chance", "random_chance")?)?),
        "random_chance_with_enchanted_bonus" => Condition::RandomChanceWithEnchantedBonus {
            unenchanted_chance: unit_interval(
                required(map, "unenchanted_chance", "random_chance_with_enchanted_bonus")?,
                "unenchanted_chance",
            )?,
            enchanted_chance: level_based_value(required(
                map,
                "enchanted_chance",
                "random_chance_with_enchanted_bonus",
            )?)?,
            enchantment: registered(
                required(map, "enchantment", "random_chance_with_enchanted_bonus")?,
                Kind::Enchantment,
                "enchantment",
            )?,
        },
        "entity_properties" => Condition::EntityProperties {
            predicate: optional(map, "predicate").map(entity_predicate).transpose()?,
            entity: entity_target(required(map, "entity", "entity_properties")?)?,
        },
        "killed_by_player" => Condition::KilledByPlayer,
        "entity_scores" => {
            let mut scores = Vec::new();
            for (objective, range) in object(required(map, "scores", "entity_scores")?, "scores")? {
                scores.push((objective.clone(), int_range(range)?));
            }
            Condition::EntityScores {
                entity: entity_target(required(map, "entity", "entity_scores")?)?,
                scores,
            }
        }
        "block_state_property" => {
            let block = registered(required(map, "block", "block_state_property")?, Kind::Block, "block")?;
            let properties = optional(map, "properties").map(state_properties).transpose()?;
            // `LootItemBlockStatePropertyCondition.validate`: every named property
            // must exist on the block ("Block X has no property Y").
            if let Some(properties) = &properties {
                let defined = super::predicate::block_property_names(&block)
                    .ok_or_else(|| LootError::Codec(format!("unknown block {block}")))?;
                if let Some((missing, _)) = properties.properties.iter().find(|(name, _)| !defined.contains(name)) {
                    return codec(format!("Block {block} has no property {missing}"));
                }
            }
            Condition::BlockStateProperty { block, properties }
        }
        "match_tool" => Condition::MatchTool(optional(map, "predicate").map(item_predicate).transpose()?),
        "table_bonus" => {
            let values = required(map, "chances", "table_bonus")?
                .as_array()
                .ok_or_else(|| LootError::Codec("chances: expected a list".to_owned()))?
                .iter()
                .map(|v| float(v, "chance"))
                .collect::<Result<Vec<_>>>()?;
            if values.is_empty() {
                return codec("chances: List must have contents");
            }
            Condition::TableBonus {
                enchantment: registered(required(map, "enchantment", "table_bonus")?, Kind::Enchantment, "enchantment")?,
                values,
            }
        }
        "survives_explosion" => Condition::SurvivesExplosion,
        "damage_source_properties" => Condition::DamageSourceProperties(
            optional(map, "predicate").map(damage_source_predicate).transpose()?,
        ),
        "location_check" => {
            let offset = |key: &str| -> Result<i32> {
                match optional(map, key) {
                    Some(v) => int(v, key),
                    None => Ok(0),
                }
            };
            Condition::LocationCheck {
                predicate: optional(map, "predicate").map(location_predicate).transpose()?,
                offset: [offset("offsetX")?, offset("offsetY")?, offset("offsetZ")?],
            }
        }
        "weather_check" => Condition::WeatherCheck {
            raining: optional(map, "raining").map(|v| boolean(v, "raining")).transpose()?,
            thundering: optional(map, "thundering").map(|v| boolean(v, "thundering")).transpose()?,
        },
        "reference" => Condition::Reference(id(required(map, "name", "reference")?, "name")?),
        "time_check" => Condition::TimeCheck {
            period: optional(map, "period")
                .map(|v| {
                    v.as_i64()
                        .or_else(|| v.as_f64().map(|number| number as i64))
                        .ok_or_else(|| LootError::Codec("period: expected a long".to_owned()))
                })
                .transpose()?,
            value: int_range(required(map, "value", "time_check")?)?,
        },
        "value_check" => Condition::ValueCheck {
            provider: number_provider(required(map, "value", "value_check")?)?,
            range: int_range(required(map, "range", "value_check")?)?,
        },
        "enchantment_active_check" => {
            Condition::EnchantmentActiveCheck(boolean(required(map, "active", "enchantment_active_check")?, "active")?)
        }
        other => return codec(format!("condition type {other} has no decoder")),
    })
}

// ---------------------------------------------------------------- functions

fn function_list(value: &Value, what: &str) -> Result<Vec<Function>> {
    value
        .as_array()
        .ok_or_else(|| LootError::Codec(format!("{what}: expected a list of functions")))?
        .iter()
        .map(function)
        .collect()
}

fn functions_field(map: &Map<String, Value>) -> Result<Vec<Function>> {
    match optional(map, "functions") {
        Some(list) => function_list(list, "functions"),
        None => Ok(Vec::new()),
    }
}

/// `ComponentSerialization.CODEC` over `JsonOps`.
fn text_component(value: &Value) -> Result<TextComponent> {
    use crate::components::text::ComponentError;
    TextComponent::from_json(value).map_err(|error| match error {
        ComponentError::Codec(what) => LootError::Codec(format!("text component: {what}")),
        ComponentError::Unsupported(what) => LootError::Unsupported(format!("text component: {what}")),
    })
}

/// `TagParser.LENIENT_CODEC`: SNBT text, or else any JSON value converted by `JsonOps` into
/// `NbtOps` that turns out to be a compound.
fn lenient_compound(value: &Value) -> Result<crate::nbt::Tag> {
    use crate::nbt::Tag;
    if let Value::String(text) = value {
        return crate::tag_parser::parse_compound(text).map_err(|e| LootError::Codec(format!("tag: {e}")));
    }
    match json_to_nbt(value)? {
        tag @ Tag::Compound(_) => Ok(tag),
        _ => codec("tag: Not a compound tag"),
    }
}

/// `JsonOps.convertTo(NbtOps)`: numbers as narrow as they are exact, booleans bytes, lists collected as
/// `NbtOps.createList` does (byte, int and long runs become arrays; mixed lists wrap their
/// elements in `{"": value}` compounds).
fn json_to_nbt(value: &Value) -> Result<crate::nbt::Tag> {
    use crate::nbt::{JavaString, Tag};
    Ok(match value {
        Value::Null => return Err(LootError::Unsupported("tag: JSON null (NbtOps.empty, an END tag)".to_owned())),
        Value::Bool(flag) => Tag::Byte(i8::from(*flag)),
        // `JsonOps.convertTo`: an exact integer is the narrowest of byte/short/int/long
        // (`BigDecimal.longValueExact`), anything else a float when the float is exact, else a double.
        Value::Number(n) => {
            let exact = n.as_i64().or_else(|| {
                n.as_f64().filter(|v| v.fract() == 0.0 && *v >= -9.223_372_036_854_775_808e18 && *v < 9.223_372_036_854_775_808e18).map(|v| v as i64)
            });
            match exact {
                Some(l) if i64::from(l as i8) == l => Tag::Byte(l as i8),
                Some(l) if i64::from(l as i16) == l => Tag::Short(l as i16),
                Some(l) if i64::from(l as i32) == l => Tag::Int(l as i32),
                Some(l) => Tag::Long(l),
                None => {
                    let d = n.as_f64().unwrap_or(0.0);
                    if f64::from(d as f32) == d {
                        Tag::Float(if d == 0.0 { 0.0 } else { d as f32 })
                    } else {
                        Tag::Double(d)
                    }
                }
            }
        }
        Value::String(text) => Tag::String(JavaString::Utf8(text.clone())),
        Value::Object(map) => {
            let mut entries: Vec<(JavaString, Tag)> = Vec::new();
            for (key, value) in map {
                entries.push((JavaString::Utf8(key.clone()), json_to_nbt(value)?));
            }
            Tag::Compound(entries)
        }
        Value::Array(items) => {
            let tags = items.iter().map(json_to_nbt).collect::<Result<Vec<_>>>()?;
            let Some(first) = tags.first() else {
                return Ok(Tag::List { element_type: 0, values: Vec::new() });
            };
            let same = tags.iter().all(|t| t.tag_id() == first.tag_id());
            match (first, same) {
                (Tag::Byte(_), true) => Tag::ByteArray(tags.iter().map(|t| if let Tag::Byte(b) = t { *b } else { 0 }).collect()),
                (Tag::Int(_), true) => Tag::IntArray(tags.iter().map(|t| if let Tag::Int(n) = t { *n } else { 0 }).collect()),
                (Tag::Long(_), true) => Tag::LongArray(tags.iter().map(|t| if let Tag::Long(n) = t { *n } else { 0 }).collect()),
                (Tag::Compound(_), _) | (_, false) => {
                    let wrap = |t: &Tag| -> Tag {
                        match t {
                            Tag::Compound(e) if !(e.len() == 1 && e[0].0 == JavaString::Utf8(String::new())) => t.clone(),
                            other => Tag::Compound(vec![(JavaString::Utf8(String::new()), other.clone())]),
                        }
                    };
                    if same {
                        // All compounds: a homogeneous list unless one of them is a wrapper.
                        let values: Vec<Tag> = tags.iter().map(wrap).collect();
                        Tag::List { element_type: 10, values }
                    } else {
                        Tag::List { element_type: 10, values: tags.iter().map(wrap).collect() }
                    }
                }
                _ => Tag::List { element_type: first.tag_id(), values: tags },
            }
        }
    })
}

/// `ContextNbtProvider.GETTER_CODEC`: `block_entity` or an entity target name (any other name
/// makes `EntityTarget.getByName` throw).
fn context_nbt_target(name: &str) -> Result<NbtSource> {
    Ok(match name {
        "block_entity" => NbtSource::BlockEntity,
        "this" => NbtSource::Entity(EntityTarget::This),
        "attacker" => NbtSource::Entity(EntityTarget::Attacker),
        "direct_attacker" => NbtSource::Entity(EntityTarget::DirectAttacker),
        "attacking_player" => NbtSource::Entity(EntityTarget::AttackingPlayer),
        other => return codec(format!("Invalid entity target {other}")),
    })
}

/// `ContainerComponentManipulators.CODEC`.
fn container_component(value: &Value) -> Result<ContainerComponent> {
    let name = id(value, "component")?;
    if component_id(&name).is_none() || !name.starts_with("minecraft:") {
        return codec(format!("Unknown registry key in minecraft:data_component_type: {name}"));
    }
    match name.as_str() {
        "minecraft:container" => Ok(ContainerComponent::Container),
        "minecraft:bundle_contents" => Ok(ContainerComponent::BundleContents),
        "minecraft:charged_projectiles" => Ok(ContainerComponent::ChargedProjectiles),
        _ => codec("No items in component"),
    }
}

/// `holderByNameCodec` over a fixed registry given in id order: the index of the id.
fn id_in(value: &Value, registry: &[&str], what: &str) -> Result<i32> {
    let name = id(value, what)?;
    registry
        .iter()
        .position(|n| *n == name)
        .map(|i| i as i32)
        .ok_or_else(|| LootError::Codec(format!("Unknown registry key in {what}: {name}")))
}

/// `Codec.string(0, max)`: the length is counted in UTF-16 units.
fn bounded_string(value: &Value, max: usize, what: &str) -> Result<String> {
    let text = string(value, what)?;
    let length = text.encode_utf16().count();
    if length > max {
        return codec(format!("{what}: String \"{text}\" is too long: {length} > {max}"));
    }
    Ok(text.to_owned())
}

/// `Filterable.codec(inner)`: `{raw, filtered?}`, or else a bare value.
fn filterable<T>(value: &Value, inner: impl Fn(&Value) -> Result<T>) -> Result<Filterable<T>> {
    if let Value::Object(map) = value {
        if let Some(raw) = optional(map, "raw") {
            if let Ok(raw) = inner(raw) {
                let filtered = optional(map, "filtered").map(&inner).transpose();
                if let Ok(filtered) = filtered {
                    return Ok(Filterable { raw, filtered });
                }
            }
        }
    }
    inner(value).map(|raw| Filterable { raw, filtered: None })
}

/// `SetWrittenBookPagesFunction.PAGE_CODEC`: a component whose flat JSON form fits 32767 characters.
fn book_page(value: &Value) -> Result<TextComponent> {
    let component = text_component(value)?;
    let length = component.to_json_string().encode_utf16().count();
    if length > 32767 {
        return codec(format!("page: String is too long: {length} > 32767"));
    }
    Ok(component)
}

/// A `StringRepresentable` enum by name, as its index.
fn named_index(value: &Value, names: &[&str], what: &str) -> Result<i32> {
    let name = string(value, what)?;
    names
        .iter()
        .position(|n| *n == name)
        .map(|i| i as i32)
        .ok_or_else(|| LootError::Codec(format!("{what}: unknown element name {name}")))
}

/// `Codec.INT.listOf()`.
fn int_list(value: &Value, what: &str) -> Result<Vec<i32>> {
    value
        .as_array()
        .ok_or_else(|| LootError::Codec(format!("{what}: expected a list")))?
        .iter()
        .map(|v| int(v, what))
        .collect()
}

/// `ExtraCodecs.UNSIGNED_BYTE`: `Codec.BYTE` (`Number.byteValue`) read as unsigned.
fn unsigned_byte(value: &Value) -> Result<i32> {
    match value {
        Value::Number(n) => Ok(i32::from(crate::components::java::json_number_int_value(n) as u8)),
        _ => codec("unsigned byte: expected a number"),
    }
}

/// `BannerPattern.CODEC` (`RegistryFileCodec`): a registered id, or an inline pattern.
fn banner_pattern(value: &Value) -> Result<BannerPatternRef> {
    if let Value::String(text) = value {
        if let Ok(id) = resource_location(text, "pattern") {
            let path = id.strip_prefix("minecraft:");
            return match path.and_then(|p| BANNER_PATTERNS.iter().position(|n| *n == p)) {
                Some(index) => Ok(BannerPatternRef::Registered(index as i32)),
                None => codec(format!("Failed to get element {id}")),
            };
        }
    }
    let map = object(value, "pattern")?;
    Ok(BannerPatternRef::Direct {
        asset_id: id(required(map, "asset_id", "pattern")?, "asset_id")?,
        translation_key: string(required(map, "translation_key", "pattern")?, "translation_key")?.to_owned(),
    })
}

/// `FireworkExplosion.CODEC`.
fn firework_explosion(value: &Value) -> Result<FireworkExplosion> {
    let map = object(value, "explosion")?;
    Ok(FireworkExplosion {
        shape: named_index(required(map, "shape", "explosion")?, &FIREWORK_SHAPES, "shape")?,
        colors: optional(map, "colors").map(|v| int_list(v, "colors")).transpose()?.unwrap_or_default(),
        fade_colors: optional(map, "fade_colors").map(|v| int_list(v, "fade_colors")).transpose()?.unwrap_or_default(),
        trail: optional(map, "has_trail").map(|v| boolean(v, "has_trail")).transpose()?.unwrap_or(false),
        twinkle: optional(map, "has_twinkle").map(|v| boolean(v, "has_twinkle")).transpose()?.unwrap_or(false),
    })
}

/// `ListOperation.codec(max)`: the `mode` member and its fields, inlined in the function.
fn list_operation(map: &Map<String, Value>, max: i32) -> Result<ListOperation> {
    let non_negative = |key: &str| -> Result<Option<i32>> {
        match optional(map, key) {
            None => Ok(None),
            Some(value) => {
                let n = int(value, key)?;
                if n < 0 {
                    return codec(format!("{key}: Value must be non-negative: {n}"));
                }
                Ok(Some(n))
            }
        }
    };
    let operation = match string(required(map, "mode", "list operation")?, "mode")? {
        "replace_all" => ListOperation::ReplaceAll,
        "append" => ListOperation::Append,
        "insert" => ListOperation::Insert { offset: non_negative("offset")?.unwrap_or(0) },
        "replace_section" => {
            let offset = non_negative("offset")?.unwrap_or(0);
            let size = non_negative("size")?;
            if let Some(size) = size {
                if size > max {
                    return codec(format!("Size value too large: {size}, max size is {max}"));
                }
            }
            ListOperation::ReplaceSection { offset, size }
        }
        other => return codec(format!("Unknown list operation {other}")),
    };
    Ok(operation)
}

/// `LootItemFunctions.ROOT_CODEC`: a list is an inline `sequence`.
pub fn function(value: &Value) -> Result<Function> {
    if let Value::Array(_) = value {
        return Ok(Function {
            conditions: Vec::new(),
            kind: FunctionKind::Sequence(function_list(value, "sequence")?),
        });
    }
    let map = object(value, "function")?;
    let name = type_name(map, "function", "function")?;
    let short = name.strip_prefix("minecraft:").unwrap_or(&name);
    if !FUNCTION_TYPES.contains(&short) || !name.starts_with("minecraft:") {
        return codec(format!("unknown function type {name}"));
    }
    if short == "sequence" {
        return Ok(Function {
            conditions: Vec::new(),
            kind: FunctionKind::Sequence(function_list(required(map, "functions", "sequence")?, "functions")?),
        });
    }
    let conditions = conditions_field(map)?;
    let kind = match short {
        "set_count" => FunctionKind::SetCount {
            count: number_provider(required(map, "count", "set_count")?)?,
            add: bool_or_else(map, "add", false),
        },
        "explosion_decay" => FunctionKind::ExplosionDecay,
        "limit_count" => FunctionKind::LimitCount(int_range(required(map, "limit", "limit_count")?)?),
        "enchanted_count_increase" => FunctionKind::EnchantedCountIncrease {
            enchantment: registered(required(map, "enchantment", "enchanted_count_increase")?, Kind::Enchantment, "enchantment")?,
            count: number_provider(required(map, "count", "enchanted_count_increase")?)?,
            limit: optional(map, "limit").map(|v| int(v, "limit")).transpose()?.unwrap_or(0),
        },
        "apply_bonus" => {
            let formula_id = id(required(map, "formula", "apply_bonus")?, "formula")?;
            let parameters = optional(map, "parameters");
            let formula = match formula_id.as_str() {
                "minecraft:ore_drops" => BonusFormula::OreDrops,
                "minecraft:uniform_bonus_count" => {
                    let parameters = object(
                        parameters.ok_or_else(|| LootError::Codec("apply_bonus: missing parameters".to_owned()))?,
                        "parameters",
                    )?;
                    BonusFormula::UniformBonusCount {
                        bonus_multiplier: int(required(parameters, "bonusMultiplier", "uniform_bonus_count")?, "bonusMultiplier")?,
                    }
                }
                "minecraft:binomial_with_bonus_count" => {
                    let parameters = object(
                        parameters.ok_or_else(|| LootError::Codec("apply_bonus: missing parameters".to_owned()))?,
                        "parameters",
                    )?;
                    BonusFormula::BinomialWithBonusCount {
                        extra_rounds: int(required(parameters, "extra", "binomial_with_bonus_count")?, "extra")?,
                        probability: float(required(parameters, "probability", "binomial_with_bonus_count")?, "probability")?,
                    }
                }
                other => return codec(format!("No formula type with id: '{other}'")),
            };
            FunctionKind::ApplyBonus {
                enchantment: registered(required(map, "enchantment", "apply_bonus")?, Kind::Enchantment, "enchantment")?,
                formula,
            }
        }
        "set_damage" => FunctionKind::SetDamage {
            damage: number_provider(required(map, "damage", "set_damage")?)?,
            add: bool_or_else(map, "add", false),
        },
        "furnace_smelt" => FunctionKind::FurnaceSmelt,
        "enchant_randomly" => FunctionKind::EnchantRandomly {
            options: optional(map, "options").map(|v| holder_set(v, Kind::Enchantment, "options")).transpose()?,
            only_compatible: optional(map, "only_compatible")
                .map(|v| boolean(v, "only_compatible"))
                .transpose()?
                .unwrap_or(true),
        },
        "enchant_with_levels" => FunctionKind::EnchantWithLevels {
            levels: number_provider(required(map, "levels", "enchant_with_levels")?)?,
            options: optional(map, "options").map(|v| holder_set(v, Kind::Enchantment, "options")).transpose()?,
        },
        "set_enchantments" => {
            let mut enchantments = Vec::new();
            if let Some(object_value) = optional(map, "enchantments") {
                for (name, provider) in object(object_value, "enchantments")? {
                    let name = registered(&Value::String(name.clone()), Kind::Enchantment, "enchantment")?;
                    enchantments.push((name, number_provider(provider)?));
                }
            }
            FunctionKind::SetEnchantments {
                enchantments,
                add: bool_or_else(map, "add", false),
            }
        }
        "copy_components" => {
            let source = match string(required(map, "source", "copy_components")?, "source")? {
                "block_entity" => ComponentSource::BlockEntity,
                other => return codec(format!("Unknown component source {other}")),
            };
            let types = |value: &Value| -> Result<Vec<i32>> {
                value
                    .as_array()
                    .ok_or_else(|| LootError::Codec("component list: expected a list".to_owned()))?
                    .iter()
                    .map(|v| {
                        let name = string(v, "component type")?;
                        component_id(name).ok_or_else(|| LootError::Codec(format!("unknown component type {name}")))
                    })
                    .collect()
            };
            FunctionKind::CopyComponents {
                source,
                include: optional(map, "include").map(types).transpose()?,
                exclude: optional(map, "exclude").map(types).transpose()?.unwrap_or_default(),
            }
        }
        "copy_state" => FunctionKind::CopyState {
            block: registered(required(map, "block", "copy_state")?, Kind::Block, "block")?,
            properties: required(map, "properties", "copy_state")?
                .as_array()
                .ok_or_else(|| LootError::Codec("properties: expected a list".to_owned()))?
                .iter()
                .map(|v| string(v, "property").map(str::to_owned))
                .collect::<Result<Vec<_>>>()?,
        },
        "set_potion" => {
            let potion = id(required(map, "id", "set_potion")?, "id")?;
            if !facts().registries.potion.contains(&potion) {
                return codec(format!("unknown potion {potion}"));
            }
            FunctionKind::SetPotion { potion }
        }
        "set_components" => {
            let components = object(required(map, "components", "set_components")?, "components")?;
            // `DataComponentPatch.CODEC`: every key (optionally `!`-prefixed for a
            // removal) must be a registered component type.
            for name in components.keys() {
                let bare = name.strip_prefix('!').unwrap_or(name);
                if component_id(&resource_location(bare, "component type")?).is_none() {
                    return codec(format!("unknown component type {name}"));
                }
            }
            let mut trim = None;
            let mut other = Vec::new();
            for (name, body) in components {
                if name == "minecraft:trim" {
                    let body = object(body, "trim")?;
                    trim = Some(FunctionKind::SetTrim {
                        material: registered_in(required(body, "material", "trim")?, &facts().registries.trim_material, "trim material")?,
                        pattern: registered_in(required(body, "pattern", "trim")?, &facts().registries.trim_pattern, "trim pattern")?,
                        show_in_tooltip: optional(body, "show_in_tooltip")
                            .map(|v| boolean(v, "show_in_tooltip"))
                            .transpose()?
                            .unwrap_or(true),
                    });
                } else {
                    other.push(name.clone());
                }
            }
            match (trim, other.is_empty(), components.len()) {
                (Some(trim), true, 1) => trim,
                _ => FunctionKind::Unsupported(format!("set_components with {:?}", components.keys().collect::<Vec<_>>())),
            }
        }
        "set_item" => FunctionKind::SetItem {
            item: registered(required(map, "item", "set_item")?, Kind::Item, "item")?,
        },
        "set_name" => FunctionKind::SetName {
            name: optional(map, "name").map(text_component).transpose()?,
            entity: optional(map, "entity").map(entity_target).transpose()?,
            target: match optional(map, "target").map(|v| string(v, "target")).transpose()? {
                None | Some("custom_name") => NameTarget::CustomName,
                Some("item_name") => NameTarget::ItemName,
                Some(other) => return codec(format!("unknown set_name target {other}")),
            },
        },
        "set_stew_effect" => {
            let mut effects: Vec<StewEffect> = Vec::new();
            if let Some(list) = optional(map, "effects") {
                for entry in list
                    .as_array()
                    .ok_or_else(|| LootError::Codec("effects: expected a list".to_owned()))?
                {
                    let entry = object(entry, "stew effect")?;
                    let effect = registered_in(required(entry, "type", "stew effect")?, &facts().registries.mob_effect, "mob effect")?;
                    if effects.iter().any(|existing| existing.effect == effect) {
                        return codec(format!("Encountered duplicate mob effect: '{effect}'"));
                    }
                    effects.push(StewEffect {
                        effect,
                        duration: number_provider(required(entry, "duration", "stew effect")?)?,
                    });
                }
            }
            FunctionKind::SetStewEffect { effects }
        }
        "set_ominous_bottle_amplifier" => FunctionKind::SetOminousBottleAmplifier {
            amplifier: number_provider(required(map, "amplifier", "set_ominous_bottle_amplifier")?)?,
        },
        "set_instrument" => FunctionKind::SetInstrument {
            options: hashed_tag(required(map, "options", "set_instrument")?, "options")?,
        },
        "exploration_map" => FunctionKind::ExplorationMap {
            destination: match optional(map, "destination") {
                Some(v) => id(v, "destination")?,
                None => "minecraft:on_treasure_maps".to_owned(),
            },
            decoration: match optional(map, "decoration") {
                Some(v) => registered_in(v, &facts().registries.map_decoration_type, "decoration")?,
                None => "minecraft:mansion".to_owned(),
            },
            zoom: optional(map, "zoom").map(|v| int(v, "zoom")).transpose()?.unwrap_or(2),
            search_radius: optional(map, "search_radius").map(|v| int(v, "search_radius")).transpose()?.unwrap_or(50),
            skip_existing_chunks: optional(map, "skip_existing_chunks")
                .map(|v| boolean(v, "skip_existing_chunks"))
                .transpose()?
                .unwrap_or(true),
        },
        "reference" => FunctionKind::Reference(id(required(map, "name", "reference")?, "name")?),
        "set_lore" => {
            let lines = required(map, "lore", "set_lore")?
                .as_array()
                .ok_or_else(|| LootError::Codec("lore: expected a list".to_owned()))?;
            if lines.len() > 256 {
                return codec(format!("lore: List is too long: {}, expected range [0-256]", lines.len()));
            }
            FunctionKind::SetLore {
                lore: lines.iter().map(text_component).collect::<Result<Vec<_>>>()?,
                mode: list_operation(map, 256)?,
                entity: optional(map, "entity").map(entity_target).transpose()?,
            }
        }
        "toggle_tooltips" => {
            let mut toggles = Vec::new();
            for (name, show) in object(required(map, "toggles", "toggle_tooltips")?, "toggles")? {
                let name = resource_location(name, "component type")?;
                let type_id = component_id(&name).ok_or_else(|| LootError::Codec(format!("unknown component type {name}")))?;
                let bare = name.strip_prefix("minecraft:").unwrap_or(&name);
                if !TOOLTIP_TOGGLES.contains(&bare) || !name.starts_with("minecraft:") {
                    return codec(format!("Can't toggle tooltip visiblity for {name}"));
                }
                toggles.push((type_id, boolean(show, "toggle")?));
            }
            FunctionKind::ToggleTooltips(toggles)
        }
        "set_custom_model_data" => FunctionKind::SetCustomModelData(number_provider(required(map, "value", "set_custom_model_data")?)?),
        "set_loot_table" => {
            let name = id(required(map, "name", "set_loot_table")?, "name")?;
            let seed = match optional(map, "seed") {
                None => 0,
                Some(value) => long(value, "seed")?,
            };
            let kind = id(required(map, "type", "set_loot_table")?, "type")?;
            if !BLOCK_ENTITY_TYPES.contains(&kind.as_str()) {
                return codec(format!("Unknown registry key in minecraft:block_entity_type: {kind}"));
            }
            FunctionKind::SetLootTable { name, seed }
        }
        "set_banner_pattern" => {
            let list = required(map, "patterns", "set_banner_pattern")?
                .as_array()
                .ok_or_else(|| LootError::Codec("patterns: expected a list".to_owned()))?;
            let mut layers = Vec::new();
            for layer in list {
                let layer = object(layer, "banner layer")?;
                layers.push(BannerLayer {
                    pattern: banner_pattern(required(layer, "pattern", "banner layer")?)?,
                    color: named_index(required(layer, "color", "banner layer")?, &DYE_COLORS, "color")?,
                });
            }
            FunctionKind::SetBannerPattern { layers, append: boolean(required(map, "append", "set_banner_pattern")?, "append")? }
        }
        "set_fireworks" => {
            let explosions = match optional(map, "explosions") {
                None => None,
                Some(value) => {
                    let standalone = object(value, "explosions")?;
                    let values = required(standalone, "values", "explosions")?
                        .as_array()
                        .ok_or_else(|| LootError::Codec("values: expected a list".to_owned()))?;
                    if values.len() > 256 {
                        return codec(format!("values: List is too long: {}, expected range [0-256]", values.len()));
                    }
                    let values = values.iter().map(firework_explosion).collect::<Result<Vec<_>>>()?;
                    Some((values, list_operation(standalone, 256)?))
                }
            };
            FunctionKind::SetFireworks {
                explosions,
                flight_duration: optional(map, "flight_duration").map(unsigned_byte).transpose()?,
            }
        }
        "set_firework_explosion" => FunctionKind::SetFireworkExplosion {
            shape: optional(map, "shape").map(|v| named_index(v, &FIREWORK_SHAPES, "shape")).transpose()?,
            colors: optional(map, "colors").map(|v| int_list(v, "colors")).transpose()?,
            fade_colors: optional(map, "fade_colors").map(|v| int_list(v, "fade_colors")).transpose()?,
            trail: optional(map, "trail").map(|v| boolean(v, "trail")).transpose()?,
            twinkle: optional(map, "twinkle").map(|v| boolean(v, "twinkle")).transpose()?,
        },
        "copy_custom_data" => {
            let source = match required(map, "source", "copy_custom_data")? {
                Value::String(name) => context_nbt_target(name)?,
                Value::Object(provider) => match id(required(provider, "type", "nbt provider")?, "type")?.as_str() {
                    "minecraft:context" => match required(provider, "target", "nbt provider")? {
                        Value::String(name) => context_nbt_target(name)?,
                        _ => return codec("target: expected a string"),
                    },
                    "minecraft:storage" => NbtSource::Storage(id(required(provider, "source", "nbt provider")?, "source")?),
                    other => return codec(format!("Unknown registry key in minecraft:loot_nbt_provider_type: {other}")),
                },
                _ => return codec("source: expected a string or an object"),
            };
            let mut ops = Vec::new();
            for op in required(map, "ops", "copy_custom_data")?
                .as_array()
                .ok_or_else(|| LootError::Codec("ops: expected a list".to_owned()))?
            {
                let op = object(op, "copy operation")?;
                let path = |key: &str| -> Result<crate::nbt_path::NbtPath> {
                    let text = string(required(op, key, "copy operation")?, key)?;
                    crate::nbt_path::NbtPath::of(text).map_err(|e| LootError::Codec(format!("Failed to parse path {text}: {e}")))
                };
                ops.push(CopyOperation {
                    source: path("source")?,
                    target: path("target")?,
                    op: match string(required(op, "op", "copy operation")?, "op")? {
                        "replace" => MergeStrategy::Replace,
                        "append" => MergeStrategy::Append,
                        "merge" => MergeStrategy::Merge,
                        other => return codec(format!("Unknown element name: {other}")),
                    },
                });
            }
            FunctionKind::CopyCustomData { source, ops }
        }
        "copy_name" => FunctionKind::CopyName(match string(required(map, "source", "copy_name")?, "source")? {
            "this" => NameSource::Entity(EntityTarget::This),
            "attacking_entity" => NameSource::Entity(EntityTarget::Attacker),
            "last_damage_player" => NameSource::Entity(EntityTarget::AttackingPlayer),
            "block_entity" => NameSource::BlockEntity,
            other => return codec(format!("Unknown element name: {other}")),
        }),
        "fill_player_head" => FunctionKind::FillPlayerHead(entity_target(required(map, "entity", "fill_player_head")?)?),
        "set_custom_data" => FunctionKind::SetCustomData(lenient_compound(required(map, "tag", "set_custom_data")?)?),
        "set_contents" => FunctionKind::SetContents {
            component: container_component(required(map, "component", "set_contents")?)?,
            entries: entry_list(required(map, "entries", "set_contents")?, "entries")?,
        },
        "modify_contents" => FunctionKind::ModifyContents {
            component: container_component(required(map, "component", "modify_contents")?)?,
            modifier: Box::new(function(required(map, "modifier", "modify_contents")?)?),
        },
        "set_attributes" => {
            let list = required(map, "modifiers", "set_attributes")?
                .as_array()
                .ok_or_else(|| LootError::Codec("modifiers: expected a list".to_owned()))?;
            let mut modifiers = Vec::new();
            for modifier in list {
                let modifier = object(modifier, "modifier")?;
                let id = id(required(modifier, "id", "modifier")?, "id")?;
                let attribute = id_in(required(modifier, "attribute", "modifier")?, &ATTRIBUTES, "attribute")?;
                let operation = named_index(required(modifier, "operation", "modifier")?, &ATTRIBUTE_OPERATIONS, "operation")?;
                let amount = number_provider(required(modifier, "amount", "modifier")?)?;
                let slots = match required(modifier, "slot", "modifier")? {
                    Value::Array(items) => items.iter().map(|v| named_index(v, &SLOT_GROUPS, "slot")).collect::<Result<Vec<_>>>()?,
                    other => vec![named_index(other, &SLOT_GROUPS, "slot")?],
                };
                if slots.is_empty() {
                    return codec("slot: List must have contents");
                }
                modifiers.push(AttributeModifierSpec { id, attribute, operation, amount, slots });
            }
            FunctionKind::SetAttributes {
                modifiers,
                replace: optional(map, "replace").map(|v| boolean(v, "replace")).transpose()?.unwrap_or(true),
            }
        }
        "set_book_cover" => FunctionKind::SetBookCover {
            title: optional(map, "title").map(|v| filterable(v, |v| bounded_string(v, 32, "title"))).transpose()?,
            author: optional(map, "author").map(|v| string(v, "author").map(str::to_owned)).transpose()?,
            generation: match optional(map, "generation") {
                None => None,
                Some(v) => {
                    let n = int(v, "generation")?;
                    if !(0..=3).contains(&n) {
                        return codec(format!("Value {n} outside of range [0:3]"));
                    }
                    Some(n)
                }
            },
        },
        "set_written_book_pages" => FunctionKind::SetWrittenBookPages {
            pages: required(map, "pages", "set_written_book_pages")?
                .as_array()
                .ok_or_else(|| LootError::Codec("pages: expected a list".to_owned()))?
                .iter()
                .map(|v| filterable(v, book_page))
                .collect::<Result<Vec<_>>>()?,
            mode: list_operation(map, i32::MAX)?,
        },
        "set_writable_book_pages" => {
            let pages = required(map, "pages", "set_writable_book_pages")?
                .as_array()
                .ok_or_else(|| LootError::Codec("pages: expected a list".to_owned()))?;
            if pages.len() > 100 {
                return codec(format!("pages: List is too long: {}, expected range [0-100]", pages.len()));
            }
            FunctionKind::SetWritableBookPages {
                pages: pages.iter().map(|v| filterable(v, |v| bounded_string(v, 1024, "page"))).collect::<Result<Vec<_>>>()?,
                mode: list_operation(map, 100)?,
            }
        }
        "filtered" => FunctionKind::Filtered {
            filter: item_predicate(required(map, "item_filter", "filtered")?)?,
            modifier: Box::new(function(required(map, "modifier", "filtered")?)?),
        },
        other => FunctionKind::Unsupported(other.to_owned()),
    };
    Ok(Function { conditions, kind })
}

fn registered_in(value: &Value, registry: &[String], what: &str) -> Result<String> {
    let name = id(value, what)?;
    if registry.contains(&name) {
        Ok(name)
    } else {
        codec(format!("{what}: unknown registry entry {name}"))
    }
}

// ---------------------------------------------------------------- entries, pools, tables

fn entry_list(value: &Value, what: &str) -> Result<Vec<Entry>> {
    value
        .as_array()
        .ok_or_else(|| LootError::Codec(format!("{what}: expected a list of entries")))?
        .iter()
        .map(entry)
        .collect()
}

pub fn entry(value: &Value) -> Result<Entry> {
    let map = object(value, "loot pool entry")?;
    let name = type_name(map, "type", "loot pool entry")?;
    let conditions = conditions_field(map)?;
    let singleton = |kind: SingletonKind| -> Result<Entry> {
        Ok(Entry::Singleton(super::entry::Singleton {
            weight: optional(map, "weight").map(|v| int(v, "weight")).transpose()?.unwrap_or(1),
            quality: optional(map, "quality").map(|v| int(v, "quality")).transpose()?.unwrap_or(0),
            conditions: conditions.clone(),
            functions: functions_field(map)?,
            kind,
        }))
    };
    match name.as_str() {
        "minecraft:empty" => singleton(SingletonKind::Empty),
        "minecraft:item" => singleton(SingletonKind::Item(registered(
            required(map, "name", "item entry")?,
            Kind::Item,
            "name",
        )?)),
        "minecraft:tag" => {
            let tag = resource_location(string(required(map, "name", "tag entry")?, "name")?, "name")?;
            let expand = boolean(required(map, "expand", "tag entry")?, "expand")?;
            singleton(SingletonKind::Tag { name: tag, expand })
        }
        "minecraft:loot_table" => {
            let value_field = required(map, "value", "loot_table entry")?;
            let reference = match value_field {
                Value::String(_) => TableRef::Key(id(value_field, "value")?),
                Value::Object(_) => TableRef::Inline(Box::new(table(value_field)?)),
                _ => return codec("loot_table entry value: expected a key or an inline table"),
            };
            singleton(SingletonKind::LootTable(reference))
        }
        "minecraft:dynamic" => singleton(SingletonKind::Dynamic(id(
            required(map, "name", "dynamic entry")?,
            "name",
        )?)),
        "minecraft:alternatives" => Ok(Entry::Alternatives {
            children: match optional(map, "children") {
                Some(children) => entry_list(children, "children")?,
                None => Vec::new(),
            },
            conditions,
        }),
        "minecraft:sequence" => Ok(Entry::Sequence {
            children: match optional(map, "children") {
                Some(children) => entry_list(children, "children")?,
                None => Vec::new(),
            },
            conditions,
        }),
        "minecraft:group" => Ok(Entry::Group {
            children: match optional(map, "children") {
                Some(children) => entry_list(children, "children")?,
                None => Vec::new(),
            },
            conditions,
        }),
        other => codec(format!("unknown loot pool entry type {other}")),
    }
}

fn pool(value: &Value) -> Result<LootPool> {
    let map = object(value, "loot pool")?;
    Ok(LootPool {
        entries: entry_list(required(map, "entries", "loot pool")?, "entries")?,
        conditions: conditions_field(map)?,
        functions: functions_field(map)?,
        rolls: number_provider(required(map, "rolls", "loot pool")?)?,
        // `NumberProviders.CODEC.fieldOf("bonus_rolls").orElse(0)` swallows decode errors.
        bonus_rolls: optional(map, "bonus_rolls")
            .and_then(|value| number_provider(value).ok())
            .unwrap_or(NumberProvider::Constant(0.0)),
    })
}

/// `LootTable.DIRECT_CODEC`.
pub fn table(value: &Value) -> Result<LootTable> {
    let map = object(value, "loot table")?;
    // `lenientOptionalFieldOf`: an unknown parameter set falls back to the default.
    let param_set: &'static ParamSet = optional(map, "type")
        .and_then(Value::as_str)
        .and_then(param_set)
        .unwrap_or(&GENERIC);
    Ok(LootTable {
        param_set,
        random_sequence: optional(map, "random_sequence").map(|v| id(v, "random_sequence")).transpose()?,
        pools: match optional(map, "pools") {
            Some(pools) => pools
                .as_array()
                .ok_or_else(|| LootError::Codec("pools: expected a list".to_owned()))?
                .iter()
                .map(pool)
                .collect::<Result<Vec<_>>>()?,
            None => Vec::new(),
        },
        functions: functions_field(map)?,
    })
}

/// Canonical form of a table key as written in references.
pub fn table_key(key: &str) -> String {
    canonical(key)
}
