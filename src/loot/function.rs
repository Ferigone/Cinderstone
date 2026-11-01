//! `LootItemFunction`: the item modifiers a table, pool or entry applies.
//!
//! Implemented from the original classes: `set_count`, `explosion_decay`,
//! `limit_count`, `enchanted_count_increase`, `apply_bonus`, `set_damage`,
//! `furnace_smelt`, `enchant_randomly`, `enchant_with_levels`,
//! `set_enchantments`, `copy_components`, `copy_state`, `set_potion`,
//! `set_components` (armor trim), `set_item`, `set_name` (translatable and
//! plain text), `set_stew_effect`, `set_ominous_bottle_amplifier`,
//! `set_instrument`, `exploration_map`, `sequence` and `reference`. The other
//! registered function types parse into [`FunctionKind::Unsupported`] and fail
//! with `LootError::Unsupported` when executed.

use super::condition::{all_of, Condition};
use super::context::{EntityTarget, LootContext, Visited};
use super::enchant::{self, copy_with_count, count_of, new_stack, set_component};
use super::facts::{facts, HolderSet, Kind};
use super::number::{IntRange, NumberProvider};
use super::predicate::stack_item_id;
use super::{mth, unsupported, LootError, Result};
use crate::inventory::ItemStack;
use crate::nbt::{JavaString, Tag};
use crate::protocol::{Reader, Writer};

const DAMAGE: i32 = 3;
const CUSTOM_NAME: i32 = 5;
const ITEM_NAME: i32 = 6;
const POTION_CONTENTS: i32 = 31;
const SUSPICIOUS_STEW_EFFECTS: i32 = 32;
const TRIM: i32 = 35;
const INSTRUMENT: i32 = 40;
const OMINOUS_BOTTLE_AMPLIFIER: i32 = 41;
const BLOCK_STATE: i32 = 53;

/// Component type names in protocol order (`DataComponents` registry IDs).
pub const COMPONENT_NAMES: [&str; 57] = [
    "custom_data", "max_stack_size", "max_damage", "damage", "unbreakable", "custom_name",
    "item_name", "lore", "rarity", "enchantments", "can_place_on", "can_break",
    "attribute_modifiers", "custom_model_data", "hide_additional_tooltip", "hide_tooltip",
    "repair_cost", "creative_slot_lock", "enchantment_glint_override", "intangible_projectile",
    "food", "fire_resistant", "tool", "stored_enchantments", "dyed_color", "map_color", "map_id",
    "map_decorations", "map_post_processing", "charged_projectiles", "bundle_contents",
    "potion_contents", "suspicious_stew_effects", "writable_book_content", "written_book_content",
    "trim", "debug_stick_state", "entity_data", "bucket_entity_data", "block_entity_data",
    "instrument", "ominous_bottle_amplifier", "jukebox_playable", "recipes", "lodestone_tracker",
    "firework_explosion", "fireworks", "profile", "note_block_sound", "banner_patterns",
    "base_color", "pot_decorations", "container", "block_state", "bees", "lock", "container_loot",
];

/// Component type ID by `minecraft:`-qualified or bare name.
pub fn component_id(name: &str) -> Option<i32> {
    let bare = name.strip_prefix("minecraft:").unwrap_or(name);
    COMPONENT_NAMES.iter().position(|n| *n == bare).map(|i| i as i32)
}

/// `ApplyBonusCount.Formula`.
#[derive(Debug, Clone, PartialEq)]
pub enum BonusFormula {
    BinomialWithBonusCount { extra_rounds: i32, probability: f32 },
    OreDrops,
    UniformBonusCount { bonus_multiplier: i32 },
}

impl BonusFormula {
    fn calculate(&self, ctx: &mut LootContext<'_>, count: i32, level: i32) -> Result<i32> {
        match self {
            Self::BinomialWithBonusCount {
                extra_rounds,
                probability,
            } => {
                let mut count = count;
                let rounds = level.wrapping_add(*extra_rounds);
                for _ in 0..rounds.max(0) {
                    if ctx.random().next_f32() < *probability {
                        count = count.wrapping_add(1);
                    }
                }
                Ok(count)
            }
            Self::OreDrops => {
                if level > 0 {
                    let mut bonus = ctx.random().next_bounded_i32(level.wrapping_add(2))?.wrapping_sub(1);
                    if bonus < 0 {
                        bonus = 0;
                    }
                    Ok(count.wrapping_mul(bonus.wrapping_add(1)))
                } else {
                    Ok(count)
                }
            }
            Self::UniformBonusCount { bonus_multiplier } => {
                let bound = bonus_multiplier.wrapping_mul(level).wrapping_add(1);
                Ok(count.wrapping_add(ctx.random().next_bounded_i32(bound)?))
            }
        }
    }
}

/// `SetNameFunction.Target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameTarget {
    CustomName,
    ItemName,
}

/// A text component this port can serialize to its network NBT form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextComponent {
    /// `{"translate": key}` with no arguments, fallback or style.
    Translate(String),
    /// A plain string component.
    Plain(String),
}

/// `SetStewEffectFunction.EffectEntry`.
#[derive(Debug, Clone, PartialEq)]
pub struct StewEffect {
    pub effect: String,
    pub duration: NumberProvider,
}

/// `CopyComponentsFunction.Source`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentSource {
    BlockEntity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionKind {
    SetCount { count: NumberProvider, add: bool },
    ExplosionDecay,
    LimitCount(IntRange),
    EnchantedCountIncrease { enchantment: String, count: NumberProvider, limit: i32 },
    ApplyBonus { enchantment: String, formula: BonusFormula },
    SetDamage { damage: NumberProvider, add: bool },
    FurnaceSmelt,
    EnchantRandomly { options: Option<HolderSet>, only_compatible: bool },
    EnchantWithLevels { levels: NumberProvider, options: Option<HolderSet> },
    SetEnchantments { enchantments: Vec<(String, NumberProvider)>, add: bool },
    CopyComponents { source: ComponentSource, include: Option<Vec<i32>>, exclude: Vec<i32> },
    CopyState { block: String, properties: Vec<String> },
    SetPotion { potion: String },
    SetTrim { material: String, pattern: String, show_in_tooltip: bool },
    SetItem { item: String },
    SetName { name: Option<TextComponent>, entity: Option<EntityTarget>, target: NameTarget },
    SetStewEffect { effects: Vec<StewEffect> },
    SetOminousBottleAmplifier { amplifier: NumberProvider },
    SetInstrument { options: HolderSet },
    ExplorationMap {
        destination: String,
        decoration: String,
        zoom: i32,
        search_radius: i32,
        skip_existing_chunks: bool,
    },
    Sequence(Vec<Function>),
    Reference(String),
    /// A registered function type that is not ported yet (its `function` name).
    Unsupported(String),
}

/// A function plus the conditions of `LootItemConditionalFunction`.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub conditions: Vec<Condition>,
    pub kind: FunctionKind,
}

/// `LootItemFunctions.compose(list)` applied to a stack.
pub fn apply_all(functions: &[Function], mut stack: ItemStack, ctx: &mut LootContext<'_>) -> Result<ItemStack> {
    for function in functions {
        stack = function.apply(stack, ctx)?;
    }
    Ok(stack)
}

fn varint_bytes(value: i32) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.varint(value);
    writer.into_inner()
}

fn text_nbt(component: &TextComponent) -> Result<Vec<u8>> {
    let tag = match component {
        TextComponent::Translate(key) => Tag::Compound(vec![(
            JavaString::Utf8("translate".to_owned()),
            Tag::String(JavaString::Utf8(key.clone())),
        )]),
        TextComponent::Plain(text) => Tag::String(JavaString::Utf8(text.clone())),
    };
    tag.encode_network_anonymous()
        .map_err(|error| LootError::Host(error.to_string()))
}

impl Function {
    /// `LootItemFunction.apply(stack, context)`.
    pub fn apply(&self, stack: ItemStack, ctx: &mut LootContext<'_>) -> Result<ItemStack> {
        if !all_of(&self.conditions, ctx)? {
            return Ok(stack);
        }
        self.run(stack, ctx)
    }

    fn run(&self, mut stack: ItemStack, ctx: &mut LootContext<'_>) -> Result<ItemStack> {
        match &self.kind {
            FunctionKind::SetCount { count, add } => {
                let base = if *add { count_of(&stack) } else { 0 };
                stack.count = base.wrapping_add(count.get_int(ctx)?);
                Ok(stack)
            }
            FunctionKind::ExplosionDecay => {
                if let Some(radius) = ctx.params.explosion_radius {
                    let chance = 1.0f32 / radius;
                    let mut kept = 0;
                    for _ in 0..count_of(&stack) {
                        if ctx.random().next_f32() <= chance {
                            kept += 1;
                        }
                    }
                    stack.count = kept;
                }
                Ok(stack)
            }
            FunctionKind::LimitCount(limit) => {
                let current = count_of(&stack);
                stack.count = limit.clamp(ctx, current)?;
                Ok(stack)
            }
            FunctionKind::EnchantedCountIncrease {
                enchantment,
                count,
                limit,
            } => {
                if let Some(attacker) = ctx.entity(EntityTarget::Attacker) {
                    if attacker.is_living() {
                        let level = enchant::entity_enchantment_level(attacker, enchantment)?;
                        if level == 0 {
                            return Ok(stack);
                        }
                        let amount = level as f32 * count.get_float(ctx)?;
                        stack.count = count_of(&stack).wrapping_add(mth::round(amount));
                        if *limit > 0 && !stack.is_empty() && stack.count > *limit {
                            stack.count = *limit;
                        }
                    }
                }
                Ok(stack)
            }
            FunctionKind::ApplyBonus { enchantment, formula } => {
                if let Some(tool) = ctx.params.tool.clone() {
                    let level = enchant::item_enchantment_level(&tool, enchantment)?;
                    let current = count_of(&stack);
                    stack.count = formula.calculate(ctx, current, level)?;
                }
                Ok(stack)
            }
            FunctionKind::SetDamage { damage, add } => {
                if stack.is_damageable().map_err(host)? {
                    let max = max_damage(&stack)?;
                    let current = damage_value(&stack)?;
                    let carried = if *add { 1.0f32 - current as f32 / max as f32 } else { 0.0 };
                    let fraction = 1.0f32 - mth::clamp_f32(damage.get_float(ctx)? + carried, 0.0, 1.0);
                    let value = mth::floor(fraction * max as f32);
                    set_damage_value(&mut stack, value)?;
                }
                Ok(stack)
            }
            FunctionKind::FurnaceSmelt => {
                if stack.is_empty() {
                    return Ok(stack);
                }
                match ctx.level().smelting_result(&stack)? {
                    Some(result) if !result.is_empty() => Ok(copy_with_count(&result, count_of(&stack))),
                    _ => Ok(stack),
                }
            }
            FunctionKind::EnchantRandomly {
                options,
                only_compatible,
            } => {
                let is_book = stack_item_id(&stack) == "minecraft:book";
                let only_compatible = !is_book && *only_compatible;
                let item = stack_item_id(&stack);
                let candidates: Vec<usize> = enchant::option_indices(options.as_ref())
                    .into_iter()
                    .filter(|index| {
                        !only_compatible || facts().enchantments[*index].is_supported_item(&item)
                    })
                    .collect();
                if candidates.is_empty() {
                    return Ok(stack);
                }
                let chosen = candidates[ctx.random().next_bounded_i32(candidates.len() as i32)? as usize];
                let definition = &facts().enchantments[chosen];
                let level = mth::next_int(ctx.random(), 1, definition.max_level)?;
                if is_book {
                    stack = new_stack("minecraft:enchanted_book")?;
                }
                enchant::enchant(&mut stack, &definition.id, level)?;
                Ok(stack)
            }
            FunctionKind::EnchantWithLevels { levels, options } => {
                let levels = levels.get_int(ctx)?;
                let candidates = enchant::option_indices(options.as_ref());
                enchant::enchant_item(ctx.random(), stack, levels, &candidates)
            }
            FunctionKind::SetEnchantments { enchantments, add } => {
                if stack_item_id(&stack) == "minecraft:book" {
                    let mut book = new_stack("minecraft:enchanted_book")?;
                    book.count = stack.count;
                    book.components = stack.components.clone();
                    book.removed_components = stack.removed_components.clone();
                    // `set(STORED, remove(ENCHANTMENTS))`: the ordinary component is
                    // removed from the stack and its (empty) value becomes the stored one.
                    book.components.retain(|c| c.type_id != 9 && c.type_id != 23);
                    if !book.removed_components.contains(&9) {
                        book.removed_components.push(9);
                    }
                    book.normalize_patch();
                    stack = book;
                }
                // `NumberProvider.getInt` runs inside the update lambda, in map order,
                // and not at all when the stack has no enchantment component.
                enchant::update_enchantments(&mut stack, |mutable| {
                    for (name, provider) in enchantments {
                        let level = provider.get_int(ctx)?;
                        let value = if *add {
                            mth::clamp_i32(mutable.get_level(name).wrapping_add(level), 0, 255)
                        } else {
                            mth::clamp_i32(level, 0, 255)
                        };
                        mutable.set(name, value);
                    }
                    Ok(())
                })?;
                Ok(stack)
            }
            FunctionKind::CopyComponents {
                source,
                include,
                exclude,
            } => {
                let components = match source {
                    ComponentSource::BlockEntity => match ctx.params.block_entity {
                        Some(entity) => entity.collect_components()?,
                        None => Vec::new(),
                    },
                };
                for component in components {
                    let included = include.as_ref().map_or(true, |list| list.contains(&component.type_id));
                    if included && !exclude.contains(&component.type_id) {
                        set_component(&mut stack, component.type_id, component.data);
                    }
                }
                Ok(stack)
            }
            FunctionKind::CopyState { block, properties } => {
                let Some(state) = ctx.params.block_state else {
                    return Ok(stack);
                };
                let actual = crate::inventory::block_state_properties(state)
                    .ok_or_else(|| LootError::Host(format!("unknown block state {state}")))?;
                let mut merged: std::collections::BTreeMap<String, String> = match stack.effective_component_bytes(BLOCK_STATE) {
                    Some(bytes) => decode_block_state_properties(bytes)?,
                    None => Default::default(),
                };
                let _ = block;
                for name in properties {
                    if let Some(value) = actual.get(name) {
                        merged.insert(name.clone(), value.clone());
                    }
                }
                let mut writer = Writer::new();
                writer.varint(merged.len() as i32);
                for (name, value) in &merged {
                    writer.string(name).string(value);
                }
                set_component(&mut stack, BLOCK_STATE, writer.into_inner());
                Ok(stack)
            }
            FunctionKind::SetPotion { potion } => {
                let id = facts()
                    .registry_id("potion", potion)
                    .ok_or_else(|| LootError::Host(format!("unknown potion {potion}")))?;
                let existing = stack
                    .effective_component_bytes(POTION_CONTENTS)
                    .map(<[u8]>::to_vec)
                    .unwrap_or_else(|| vec![0, 0, 0]);
                let mut reader = Reader::new(&existing);
                if reader.bool().map_err(host)? {
                    reader.varint().map_err(host)?;
                }
                let rest = reader.remaining();
                let mut writer = Writer::new();
                writer.bool(true).varint(id).raw(rest);
                set_component(&mut stack, POTION_CONTENTS, writer.into_inner());
                Ok(stack)
            }
            FunctionKind::SetTrim {
                material,
                pattern,
                show_in_tooltip,
            } => {
                let material = facts()
                    .registry_id("trim_material", material)
                    .ok_or_else(|| LootError::Host(format!("unknown trim material {material}")))?;
                let pattern = facts()
                    .registry_id("trim_pattern", pattern)
                    .ok_or_else(|| LootError::Host(format!("unknown trim pattern {pattern}")))?;
                let mut writer = Writer::new();
                writer.varint(material + 1).varint(pattern + 1).bool(*show_in_tooltip);
                set_component(&mut stack, TRIM, writer.into_inner());
                Ok(stack)
            }
            FunctionKind::SetItem { item } => {
                let mut replaced = new_stack(item)?;
                replaced.count = stack.count;
                replaced.components = stack.components.clone();
                replaced.removed_components = stack.removed_components.clone();
                replaced.normalize_patch();
                Ok(replaced)
            }
            FunctionKind::SetName { name, entity, target } => {
                if let Some(name) = name {
                    if entity.is_some_and(|which| ctx.entity(which).is_some()) {
                        return unsupported("set_name entity-resolved components (ComponentUtils.updateForEntity)");
                    }
                    let type_id = match target {
                        NameTarget::CustomName => CUSTOM_NAME,
                        NameTarget::ItemName => ITEM_NAME,
                    };
                    set_component(&mut stack, type_id, text_nbt(name)?);
                }
                Ok(stack)
            }
            FunctionKind::SetStewEffect { effects } => {
                if stack_item_id(&stack) == "minecraft:suspicious_stew" && !effects.is_empty() {
                    let index = ctx.random().next_bounded_i32(effects.len() as i32)? as usize;
                    let entry = &effects[index];
                    let mut duration = entry.duration.get_int(ctx)?;
                    if !facts().is_instant_mob_effect(&entry.effect) {
                        duration = duration.wrapping_mul(20);
                    }
                    let id = facts()
                        .registry_id("mob_effect", &entry.effect)
                        .ok_or_else(|| LootError::Host(format!("unknown mob effect {}", entry.effect)))?;
                    let existing = stack
                        .effective_component_bytes(SUSPICIOUS_STEW_EFFECTS)
                        .map(<[u8]>::to_vec)
                        .unwrap_or_else(|| vec![0]);
                    let mut reader = Reader::new(&existing);
                    let count = reader.varint().map_err(host)?;
                    let tail = reader.remaining();
                    let mut writer = Writer::new();
                    writer.varint(count + 1).raw(tail).varint(id).varint(duration);
                    set_component(&mut stack, SUSPICIOUS_STEW_EFFECTS, writer.into_inner());
                }
                Ok(stack)
            }
            FunctionKind::SetOminousBottleAmplifier { amplifier } => {
                let amplifier = mth::clamp_i32(amplifier.get_int(ctx)?, 0, 4);
                set_component(&mut stack, OMINOUS_BOTTLE_AMPLIFIER, varint_bytes(amplifier));
                Ok(stack)
            }
            FunctionKind::SetInstrument { options } => {
                let members = options.members(Kind::Instrument);
                if !members.is_empty() {
                    let index = ctx.random().next_bounded_i32(members.len() as i32)? as usize;
                    let id = facts()
                        .registry_id("instrument", &members[index])
                        .ok_or_else(|| LootError::Host(format!("unknown instrument {}", members[index])))?;
                    set_component(&mut stack, INSTRUMENT, varint_bytes(id + 1));
                }
                Ok(stack)
            }
            FunctionKind::ExplorationMap {
                destination,
                decoration,
                zoom,
                search_radius,
                skip_existing_chunks,
            } => {
                if stack_item_id(&stack) != "minecraft:map" {
                    return Ok(stack);
                }
                match ctx.params.origin {
                    None => Ok(stack),
                    Some(origin) => ctx.level().exploration_map(
                        origin,
                        destination,
                        decoration,
                        *zoom,
                        *search_radius,
                        *skip_existing_chunks,
                        stack,
                    ),
                }
            }
            FunctionKind::Sequence(functions) => apply_all(functions, stack, ctx),
            FunctionKind::Reference(key) => {
                let resolver = ctx.resolver();
                let Some(function) = resolver.item_modifier(key) else {
                    return Ok(stack);
                };
                let visited = Visited::Modifier(key.clone());
                if !ctx.push_visited(visited.clone()) {
                    return Ok(stack);
                }
                let result = function.apply(stack, ctx);
                ctx.pop_visited(&visited);
                result
            }
            FunctionKind::Unsupported(name) => unsupported(format!("loot function {name}")),
        }
    }
}

fn host(error: anyhow::Error) -> LootError {
    LootError::Host(error.to_string())
}

fn max_damage(stack: &ItemStack) -> Result<i32> {
    stack
        .effective_component_bytes(2)
        .map_or(Ok(0), |bytes| Reader::new(bytes).varint())
        .map_err(host)
}

fn damage_value(stack: &ItemStack) -> Result<i32> {
    stack.damage_value().map_err(host)
}

/// `ItemStack.setDamageValue`: clamp to `0..=max_damage` and store.
fn set_damage_value(stack: &mut ItemStack, value: i32) -> Result<()> {
    let clamped = mth::clamp_i32(value, 0, max_damage(stack)?);
    set_component(stack, DAMAGE, varint_bytes(clamped));
    Ok(())
}

fn decode_block_state_properties(bytes: &[u8]) -> Result<std::collections::BTreeMap<String, String>> {
    let mut reader = Reader::new(bytes);
    let count = reader.varint().map_err(host)?;
    let mut map = std::collections::BTreeMap::new();
    for _ in 0..count {
        let name = reader.string(32767).map_err(host)?;
        let value = reader.string(32767).map_err(host)?;
        map.insert(name, value);
    }
    reader.finish().map_err(host)?;
    Ok(map)
}
