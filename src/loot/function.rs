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
use super::predicate::{stack_item_id, ItemPredicate};
use super::{mth, unsupported, LootError, Result};
use crate::inventory::ItemStack;
use crate::nbt::{JavaString, Tag};
use crate::protocol::{Reader, Writer};

const DAMAGE: i32 = 3;
const CUSTOM_NAME: i32 = 5;
const ITEM_NAME: i32 = 6;
const CUSTOM_DATA: i32 = 0;
const LORE: i32 = 7;
const CHARGED_PROJECTILES: i32 = 29;
const BUNDLE_CONTENTS: i32 = 30;
const CONTAINER: i32 = 52;
const ATTRIBUTE_MODIFIERS: i32 = 12;
const CUSTOM_MODEL_DATA: i32 = 13;
const CONTAINER_LOOT: i32 = 56;
const FIREWORK_EXPLOSION: i32 = 45;
const PROFILE: i32 = 47;
const WRITABLE_BOOK_CONTENT: i32 = 33;
const WRITTEN_BOOK_CONTENT: i32 = 34;
const FIREWORKS: i32 = 46;
const BANNER_PATTERNS_COMPONENT: i32 = 49;
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

/// A text component (`ComponentSerialization.CODEC`), the full model of `crate::components`.
pub type TextComponent = crate::components::text::Component;

/// `ListOperation`: how a function merges its values into an existing list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListOperation {
    ReplaceAll,
    ReplaceSection { offset: i32, size: Option<i32> },
    Insert { offset: i32 },
    Append,
}

impl ListOperation {
    /// `ListOperation.apply(old, new, maxSize)`; an out-of-range operation keeps `old` (the
    /// original logs an error and does the same).
    pub fn apply<T: Clone>(&self, old: &[T], new: &[T], max: usize) -> Vec<T> {
        match *self {
            ListOperation::ReplaceAll => new.to_vec(),
            ListOperation::Append => {
                if old.len() + new.len() > max {
                    old.to_vec()
                } else {
                    old.iter().chain(new).cloned().collect()
                }
            }
            ListOperation::Insert { offset } => {
                let offset = offset as usize;
                if offset > old.len() || old.len() + new.len() > max {
                    old.to_vec()
                } else {
                    old[..offset].iter().chain(new).chain(&old[offset..]).cloned().collect()
                }
            }
            ListOperation::ReplaceSection { offset, size } => {
                let offset = offset as usize;
                if offset > old.len() {
                    return old.to_vec();
                }
                let mut out: Vec<T> = old[..offset].iter().chain(new).cloned().collect();
                let end = offset as i64 + i64::from(size.unwrap_or(new.len() as i32));
                if end < old.len() as i64 {
                    out.extend_from_slice(&old[end as usize..]);
                }
                if out.len() > max {
                    old.to_vec()
                } else {
                    out
                }
            }
        }
    }
}

/// `Registries.BANNER_PATTERN` of the vanilla data pack, in registry (load) order.
pub const BANNER_PATTERNS: [&str; 43] = [
    "base", "border", "bricks", "circle", "creeper", "cross", "curly_border", "diagonal_left",
    "diagonal_right", "diagonal_up_left", "diagonal_up_right", "flow", "flower", "globe", "gradient",
    "gradient_up", "guster", "half_horizontal", "half_horizontal_bottom", "half_vertical",
    "half_vertical_right", "mojang", "piglin", "rhombus", "skull", "small_stripes", "square_bottom_left",
    "square_bottom_right", "square_top_left", "square_top_right", "straight_cross", "stripe_bottom",
    "stripe_center", "stripe_downleft", "stripe_downright", "stripe_left", "stripe_middle",
    "stripe_right", "stripe_top", "triangle_bottom", "triangle_top", "triangles_bottom", "triangles_top",
];

/// `DyeColor`, by id.
pub const DYE_COLORS: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan",
    "purple", "blue", "brown", "green", "red", "black",
];

/// `FireworkExplosion.Shape`, by id.
pub const FIREWORK_SHAPES: [&str; 5] = ["small_ball", "large_ball", "star", "creeper", "burst"];

/// A `Holder<BannerPattern>`: a registered pattern (its registry index) or an inline one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BannerPatternRef {
    Registered(i32),
    Direct { asset_id: String, translation_key: String },
}

/// `BannerPatternLayers.Layer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BannerLayer {
    pub pattern: BannerPatternRef,
    pub color: i32,
}

impl BannerLayer {
    fn encode(&self, writer: &mut Writer) {
        match &self.pattern {
            BannerPatternRef::Registered(index) => {
                writer.varint(index + 1);
            }
            BannerPatternRef::Direct { asset_id, translation_key } => {
                writer.varint(0).string(asset_id).string(translation_key);
            }
        }
        writer.varint(self.color);
    }
}

/// `FireworkExplosion`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FireworkExplosion {
    pub shape: i32,
    pub colors: Vec<i32>,
    pub fade_colors: Vec<i32>,
    pub trail: bool,
    pub twinkle: bool,
}

impl FireworkExplosion {
    fn encode(&self, writer: &mut Writer) {
        writer.varint(self.shape).varint(self.colors.len() as i32);
        for color in &self.colors {
            writer.raw(&color.to_be_bytes());
        }
        writer.varint(self.fade_colors.len() as i32);
        for color in &self.fade_colors {
            writer.raw(&color.to_be_bytes());
        }
        writer.bool(self.trail).bool(self.twinkle);
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self> {
        let shape = reader.varint().map_err(host)?;
        let mut lists = [Vec::new(), Vec::new()];
        for list in &mut lists {
            let count = reader.varint().map_err(host)?;
            for _ in 0..count {
                let bytes = reader.bytes(4).map_err(host)?;
                list.push(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
            }
        }
        let [colors, fade_colors] = lists;
        Ok(Self { shape, colors, fade_colors, trail: reader.bool().map_err(host)?, twinkle: reader.bool().map_err(host)? })
    }
}

/// `BuiltInRegistries.ATTRIBUTE` of 1.21, in registry order.
pub const ATTRIBUTES: [&str; 31] = [
    "minecraft:generic.armor", "minecraft:generic.armor_toughness", "minecraft:generic.attack_damage",
    "minecraft:generic.attack_knockback", "minecraft:generic.attack_speed", "minecraft:player.block_break_speed",
    "minecraft:player.block_interaction_range", "minecraft:generic.burning_time",
    "minecraft:generic.explosion_knockback_resistance", "minecraft:player.entity_interaction_range",
    "minecraft:generic.fall_damage_multiplier", "minecraft:generic.flying_speed", "minecraft:generic.follow_range",
    "minecraft:generic.gravity", "minecraft:generic.jump_strength", "minecraft:generic.knockback_resistance",
    "minecraft:generic.luck", "minecraft:generic.max_absorption", "minecraft:generic.max_health",
    "minecraft:player.mining_efficiency", "minecraft:generic.movement_efficiency",
    "minecraft:generic.movement_speed", "minecraft:generic.oxygen_bonus", "minecraft:generic.safe_fall_distance",
    "minecraft:generic.scale", "minecraft:player.sneaking_speed", "minecraft:zombie.spawn_reinforcements",
    "minecraft:generic.step_height", "minecraft:player.submerged_mining_speed",
    "minecraft:player.sweeping_damage_ratio", "minecraft:generic.water_movement_efficiency",
];

/// `EquipmentSlotGroup`, by id.
pub const SLOT_GROUPS: [&str; 10] = ["any", "mainhand", "offhand", "hand", "feet", "legs", "chest", "head", "armor", "body"];

/// `AttributeModifier.Operation`, by id.
pub const ATTRIBUTE_OPERATIONS: [&str; 3] = ["add_value", "add_multiplied_base", "add_multiplied_total"];

/// `SetAttributesFunction.Modifier`.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeModifierSpec {
    pub id: String,
    pub attribute: i32,
    pub operation: i32,
    pub amount: NumberProvider,
    pub slots: Vec<i32>,
}

/// One `ItemAttributeModifiers.Entry` as it is on the wire, with the fields `matches` reads.
#[derive(Clone)]
struct AttributeEntry {
    attribute: i32,
    id: String,
    bytes: Vec<u8>,
}

fn attribute_modifiers(bytes: &[u8]) -> Result<(Vec<AttributeEntry>, bool)> {
    let mut reader = Reader::new(bytes);
    let count = reader.varint().map_err(host)?;
    let mut entries = Vec::new();
    for _ in 0..count {
        let start = reader.remaining();
        let attribute = reader.varint().map_err(host)?;
        let id = reader.string(32767).map_err(host)?;
        reader.bytes(8).map_err(host)?;
        reader.varint().map_err(host)?;
        reader.varint().map_err(host)?;
        let used = start.len() - reader.remaining().len();
        entries.push(AttributeEntry { attribute, id, bytes: start[..used].to_vec() });
    }
    let show = reader.bool().map_err(host)?;
    reader.finish().map_err(host)?;
    Ok((entries, show))
}

/// `ArmorItem` (and `AnimalArmorItem`): `getDefaultAttributeModifiers` is not the component prototype.
fn is_armor_item(id: &str) -> bool {
    let path = id.strip_prefix("minecraft:").unwrap_or(id);
    ["_helmet", "_chestplate", "_leggings", "_boots", "_horse_armor"].iter().any(|s| path.ends_with(s)) || path == "wolf_armor"
}

/// `ContainerComponentManipulators`: the component a container function reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerComponent {
    Container,
    BundleContents,
    ChargedProjectiles,
}

impl ContainerComponent {
    fn type_id(self) -> i32 {
        match self {
            Self::Container => CONTAINER,
            Self::BundleContents => BUNDLE_CONTENTS,
            Self::ChargedProjectiles => CHARGED_PROJECTILES,
        }
    }

    fn items(self, bytes: &[u8]) -> Result<Vec<ItemStack>> {
        let mut reader = Reader::new(bytes);
        let count = reader.varint().map_err(host)?;
        let mut rest = reader.remaining();
        let mut items = Vec::new();
        for _ in 0..count {
            let (stack, used) = ItemStack::decode(rest).map_err(host)?;
            items.push(stack);
            rest = &rest[used..];
        }
        if !rest.is_empty() {
            return Err(LootError::Host("trailing bytes after container items".to_owned()));
        }
        Ok(items)
    }

    /// `setContents(stack, items)`.
    fn set(self, stack: &mut ItemStack, items: Vec<ItemStack>) -> Result<()> {
        let items = match self {
            Self::BundleContents => return unsupported("bundle_contents (BundleContents.Mutable.tryInsert)"),
            // `ItemContainerContents.fromItems`: trailing empty slots are dropped.
            Self::Container => {
                let last = items.iter().rposition(|item| !item.is_empty()).map_or(0, |i| i + 1);
                if last > 256 {
                    return Err(LootError::Host(format!("Got {last} items, but maximum is 256")));
                }
                items[..last].to_vec()
            }
            Self::ChargedProjectiles => items,
        };
        let mut writer = Writer::new();
        writer.varint(items.len() as i32);
        for item in &items {
            writer.raw(&item.encode());
        }
        set_component(stack, self.type_id(), writer.into_inner());
        Ok(())
    }
}

/// `NbtProvider`: where `copy_custom_data` reads from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NbtSource {
    Entity(EntityTarget),
    BlockEntity,
    Storage(String),
}

/// `CopyCustomDataFunction.MergeStrategy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    Replace,
    Append,
    Merge,
}

/// `CopyCustomDataFunction.CopyOperation`.
#[derive(Debug, Clone, PartialEq)]
pub struct CopyOperation {
    pub source: crate::nbt_path::NbtPath,
    pub target: crate::nbt_path::NbtPath,
    pub op: MergeStrategy,
}

/// `CopyNameFunction.NameSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameSource {
    Entity(EntityTarget),
    BlockEntity,
}

/// `ItemStack.set(type, null)`: the component is removed (a prototype value is masked).
fn remove_component(stack: &mut ItemStack, type_id: i32) {
    stack.components.retain(|c| c.type_id != type_id);
    let mut probe = stack.clone();
    probe.removed_components.retain(|id| *id != type_id);
    if probe.effective_component_bytes(type_id).is_some() && !stack.removed_components.contains(&type_id) {
        stack.removed_components.push(type_id);
    }
    stack.normalize_patch();
}

/// `CompoundTag.merge`: compounds merge recursively, every other value replaces.
fn merge_compound(target: &mut Tag, patch: &Tag) {
    let (Tag::Compound(entries), Tag::Compound(patch)) = (target, patch) else { return };
    for (key, value) in patch {
        match (entries.iter_mut().find(|(k, _)| k == key), value) {
            (Some((_, existing @ Tag::Compound(_))), Tag::Compound(_)) => merge_compound(existing, value),
            (Some((_, existing)), _) => *existing = value.clone(),
            (None, _) => entries.push((key.clone(), value.clone())),
        }
    }
}

/// `Filterable<T>`: the raw value and an optional filtered one.
#[derive(Debug, Clone, PartialEq)]
pub struct Filterable<T> {
    pub raw: T,
    pub filtered: Option<T>,
}

/// One page of a written book as it is on the wire (raw and optional filtered component NBT).
type WrittenPage = Vec<u8>;

/// `WrittenBookContent` on the wire.
struct WrittenBook {
    title: Filterable<String>,
    author: String,
    generation: i32,
    pages: Vec<WrittenPage>,
    resolved: bool,
}

impl WrittenBook {
    fn decode(bytes: Option<&[u8]>) -> Result<Self> {
        let Some(bytes) = bytes else {
            return Ok(Self {
                title: Filterable { raw: String::new(), filtered: None },
                author: String::new(),
                generation: 0,
                pages: Vec::new(),
                resolved: true,
            });
        };
        let mut reader = Reader::new(bytes);
        let raw = reader.string(32).map_err(host)?;
        let filtered = if reader.bool().map_err(host)? { Some(reader.string(32).map_err(host)?) } else { None };
        let author = reader.string(32767).map_err(host)?;
        let generation = reader.varint().map_err(host)?;
        let count = reader.varint().map_err(host)?;
        let mut pages = Vec::new();
        for _ in 0..count {
            let start = reader.remaining();
            let (_, raw_len) = Tag::decode_anonymous_prefix(start).map_err(host)?;
            let mut used = raw_len;
            let has_filtered = start.get(used).copied().unwrap_or(0) != 0;
            used += 1;
            if has_filtered {
                let (_, filtered_len) = Tag::decode_anonymous_prefix(&start[used..]).map_err(host)?;
                used += filtered_len;
            }
            pages.push(start[..used].to_vec());
            reader = Reader::new(&start[used..]);
        }
        let resolved = reader.bool().map_err(host)?;
        reader.finish().map_err(host)?;
        Ok(Self { title: Filterable { raw, filtered }, author, generation, pages, resolved })
    }

    fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.string(&self.title.raw);
        match &self.title.filtered {
            Some(filtered) => writer.bool(true).string(filtered),
            None => writer.bool(false),
        };
        writer.string(&self.author).varint(self.generation).varint(self.pages.len() as i32);
        for page in &self.pages {
            writer.raw(page);
        }
        writer.bool(self.resolved);
        writer.into_inner()
    }
}

fn written_page(page: &Filterable<TextComponent>) -> Result<WrittenPage> {
    let mut bytes = text_nbt(&page.raw)?;
    match &page.filtered {
        Some(filtered) => {
            bytes.push(1);
            bytes.extend(text_nbt(filtered)?);
        }
        None => bytes.push(0),
    }
    Ok(bytes)
}

fn writable_pages(bytes: Option<&[u8]>) -> Result<Vec<Filterable<String>>> {
    let Some(bytes) = bytes else { return Ok(Vec::new()) };
    let mut reader = Reader::new(bytes);
    let count = reader.varint().map_err(host)?;
    let mut pages = Vec::new();
    for _ in 0..count {
        let raw = reader.string(1024).map_err(host)?;
        let filtered = if reader.bool().map_err(host)? { Some(reader.string(1024).map_err(host)?) } else { None };
        pages.push(Filterable { raw, filtered });
    }
    reader.finish().map_err(host)?;
    Ok(pages)
}

/// Components whose last network field is the `show_in_tooltip` flag (`ToggleTooltips.TOGGLES`).
pub const TOOLTIP_TOGGLES: [&str; 9] = [
    "trim", "dyed_color", "enchantments", "stored_enchantments", "unbreakable", "can_break",
    "can_place_on", "attribute_modifiers", "jukebox_playable",
];

/// `BuiltInRegistries.BLOCK_ENTITY_TYPE` of 1.21, in registry order.
pub const BLOCK_ENTITY_TYPES: [&str; 44] = [
    "minecraft:furnace", "minecraft:chest", "minecraft:trapped_chest", "minecraft:ender_chest",
    "minecraft:jukebox", "minecraft:dispenser", "minecraft:dropper", "minecraft:sign",
    "minecraft:hanging_sign", "minecraft:mob_spawner", "minecraft:piston", "minecraft:brewing_stand",
    "minecraft:enchanting_table", "minecraft:end_portal", "minecraft:beacon", "minecraft:skull",
    "minecraft:daylight_detector", "minecraft:hopper", "minecraft:comparator", "minecraft:banner",
    "minecraft:structure_block", "minecraft:end_gateway", "minecraft:command_block",
    "minecraft:shulker_box", "minecraft:bed", "minecraft:conduit", "minecraft:barrel", "minecraft:smoker",
    "minecraft:blast_furnace", "minecraft:lectern", "minecraft:bell", "minecraft:jigsaw",
    "minecraft:campfire", "minecraft:beehive", "minecraft:sculk_sensor",
    "minecraft:calibrated_sculk_sensor", "minecraft:sculk_catalyst", "minecraft:sculk_shrieker",
    "minecraft:chiseled_bookshelf", "minecraft:brushable_block", "minecraft:decorated_pot",
    "minecraft:crafter", "minecraft:trial_spawner", "minecraft:vault",
];

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
    SetLore { lore: Vec<TextComponent>, mode: ListOperation, entity: Option<EntityTarget> },
    /// `(component type id, show_in_tooltip)`.
    ToggleTooltips(Vec<(i32, bool)>),
    SetCustomModelData(NumberProvider),
    SetLootTable { name: String, seed: i64 },
    Filtered { filter: ItemPredicate, modifier: Box<Function> },
    SetBannerPattern { layers: Vec<BannerLayer>, append: bool },
    SetAttributes { modifiers: Vec<AttributeModifierSpec>, replace: bool },
    CopyName(NameSource),
    CopyCustomData { source: NbtSource, ops: Vec<CopyOperation> },
    FillPlayerHead(EntityTarget),
    /// `SetCustomDataFunction`: the compound merged into `minecraft:custom_data`.
    SetCustomData(Tag),
    SetContents { component: ContainerComponent, entries: Vec<super::entry::Entry> },
    ModifyContents { component: ContainerComponent, modifier: Box<Function> },
    SetBookCover { title: Option<Filterable<String>>, author: Option<String>, generation: Option<i32> },
    SetWrittenBookPages { pages: Vec<Filterable<TextComponent>>, mode: ListOperation },
    SetWritableBookPages { pages: Vec<Filterable<String>>, mode: ListOperation },
    SetFireworks { explosions: Option<(Vec<FireworkExplosion>, ListOperation)>, flight_duration: Option<i32> },
    SetFireworkExplosion {
        shape: Option<i32>,
        colors: Option<Vec<i32>>,
        fade_colors: Option<Vec<i32>>,
        trail: Option<bool>,
        twinkle: Option<bool>,
    },
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
pub fn apply_all<'t>(functions: &'t [Function], mut stack: ItemStack, ctx: &mut LootContext<'t>) -> Result<ItemStack> {
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
    component.to_network_nbt().map_err(|error| LootError::Host(error.to_string()))
}

/// The lines of a `minecraft:lore` component: a count, then each line as network NBT.
fn lore_lines(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut reader = Reader::new(bytes);
    let count = reader.varint().map_err(host)?;
    let mut lines = Vec::new();
    for _ in 0..count {
        let before = reader.remaining();
        let (_, used) = Tag::decode_anonymous_prefix(before).map_err(host)?;
        lines.push(before[..used].to_vec());
        reader = Reader::new(&before[used..]);
    }
    reader.finish().map_err(host)?;
    Ok(lines)
}

impl Function {
    /// `LootItemFunction.apply(stack, context)`.
    pub fn apply<'t>(&'t self, stack: ItemStack, ctx: &mut LootContext<'t>) -> Result<ItemStack> {
        if !all_of(&self.conditions, ctx)? {
            return Ok(stack);
        }
        self.run(stack, ctx)
    }

    fn run<'t>(&'t self, mut stack: ItemStack, ctx: &mut LootContext<'t>) -> Result<ItemStack> {
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
                // The constructor keeps only properties the function's own block defines.
                let defined = super::predicate::block_property_names(block).ok_or_else(|| {
                    LootError::Host(format!("unknown block {block}"))
                })?;
                for name in properties.iter().filter(|name| defined.contains(name)) {
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
            FunctionKind::SetLore { lore, mode, entity } => {
                if entity.is_some_and(|which| ctx.entity(which).is_some()) && !lore.is_empty() {
                    return unsupported("set_lore entity-resolved components (ComponentUtils.updateForEntity)");
                }
                let old = lore_lines(stack.effective_component_bytes(LORE).unwrap_or(&[0]))?;
                let new = lore.iter().map(text_nbt).collect::<Result<Vec<_>>>()?;
                let lines = mode.apply(&old, &new, 256);
                let mut writer = Writer::new();
                writer.varint(lines.len() as i32);
                for line in &lines {
                    writer.raw(line);
                }
                if !stack.is_empty() {
                    set_component(&mut stack, LORE, writer.into_inner());
                }
                Ok(stack)
            }
            FunctionKind::ToggleTooltips(toggles) => {
                for (type_id, show) in toggles {
                    if let Some(bytes) = stack.effective_component_bytes(*type_id) {
                        let mut bytes = bytes.to_vec();
                        if let Some(last) = bytes.last_mut() {
                            *last = u8::from(*show);
                        }
                        set_component(&mut stack, *type_id, bytes);
                    }
                }
                Ok(stack)
            }
            FunctionKind::SetCustomModelData(value) => {
                let value = value.get_int(ctx)?;
                if !stack.is_empty() {
                    set_component(&mut stack, CUSTOM_MODEL_DATA, varint_bytes(value));
                }
                Ok(stack)
            }
            FunctionKind::SetLootTable { name, seed } => {
                if !stack.is_empty() {
                    let mut entries = vec![(JavaString::Utf8("loot_table".to_owned()), Tag::String(JavaString::Utf8(name.clone())))];
                    if *seed != 0 {
                        entries.push((JavaString::Utf8("seed".to_owned()), Tag::Long(*seed)));
                    }
                    let bytes = Tag::Compound(entries).encode_network_anonymous().map_err(|e| LootError::Host(e.to_string()))?;
                    set_component(&mut stack, CONTAINER_LOOT, bytes);
                }
                Ok(stack)
            }
            FunctionKind::Filtered { filter, modifier } => {
                if filter.test(&stack)? {
                    modifier.apply(stack, ctx)
                } else {
                    Ok(stack)
                }
            }
            FunctionKind::SetBannerPattern { layers, append } => {
                let mut writer = Writer::new();
                if *append {
                    // The old layers are kept as they are on the wire.
                    let old = stack.effective_component_bytes(BANNER_PATTERNS_COMPONENT).unwrap_or(&[0]).to_vec();
                    let mut reader = Reader::new(&old);
                    let count = reader.varint().map_err(host)?;
                    writer.varint(count + layers.len() as i32).raw(reader.remaining());
                } else {
                    writer.varint(layers.len() as i32);
                }
                for layer in layers {
                    layer.encode(&mut writer);
                }
                if !stack.is_empty() {
                    set_component(&mut stack, BANNER_PATTERNS_COMPONENT, writer.into_inner());
                }
                Ok(stack)
            }
            FunctionKind::SetFireworks { explosions, flight_duration } => {
                let (old_flight, old) = match stack.effective_component_bytes(FIREWORKS) {
                    None => (0, Vec::new()),
                    Some(bytes) => {
                        let mut reader = Reader::new(bytes);
                        let flight = reader.varint().map_err(host)?;
                        let count = reader.varint().map_err(host)?;
                        let mut list = Vec::new();
                        for _ in 0..count {
                            list.push(FireworkExplosion::decode(&mut reader)?);
                        }
                        (flight, list)
                    }
                };
                let list = match explosions {
                    Some((values, operation)) => operation.apply(&old, values, usize::MAX),
                    None => old,
                };
                if list.len() > 256 {
                    return Err(LootError::Host(format!("Got {} explosions, but maximum is 256", list.len())));
                }
                let mut writer = Writer::new();
                writer.varint(flight_duration.unwrap_or(old_flight)).varint(list.len() as i32);
                for explosion in &list {
                    explosion.encode(&mut writer);
                }
                if !stack.is_empty() {
                    set_component(&mut stack, FIREWORKS, writer.into_inner());
                }
                Ok(stack)
            }
            FunctionKind::SetFireworkExplosion { shape, colors, fade_colors, trail, twinkle } => {
                let old = match stack.effective_component_bytes(FIREWORK_EXPLOSION) {
                    Some(bytes) => FireworkExplosion::decode(&mut Reader::new(bytes))?,
                    None => FireworkExplosion { shape: 0, colors: Vec::new(), fade_colors: Vec::new(), trail: false, twinkle: false },
                };
                let new = FireworkExplosion {
                    shape: shape.unwrap_or(old.shape),
                    colors: colors.clone().unwrap_or(old.colors),
                    fade_colors: fade_colors.clone().unwrap_or(old.fade_colors),
                    trail: trail.unwrap_or(old.trail),
                    twinkle: twinkle.unwrap_or(old.twinkle),
                };
                let mut writer = Writer::new();
                new.encode(&mut writer);
                if !stack.is_empty() {
                    set_component(&mut stack, FIREWORK_EXPLOSION, writer.into_inner());
                }
                Ok(stack)
            }
            FunctionKind::CopyCustomData { source, ops } => {
                let tag = match source {
                    NbtSource::Entity(target) => match ctx.entity(*target) {
                        Some(entity) => entity.nbt_to_compare()?,
                        None => return Ok(stack),
                    },
                    NbtSource::BlockEntity => match ctx.params.block_entity {
                        Some(block_entity) => block_entity.full_nbt()?,
                        None => return Ok(stack),
                    },
                    NbtSource::Storage(id) => ctx.level().command_storage(id)?,
                };
                // The item's custom data is copied lazily, on the first operation that finds
                // something to copy.
                let mut target: Option<Tag> = None;
                for operation in ops {
                    let Ok(found) = operation.source.get(&tag) else { continue };
                    if found.is_empty() {
                        continue;
                    }
                    let data = target.get_or_insert_with(|| match stack.effective_component_bytes(CUSTOM_DATA) {
                        Some(bytes) => Tag::decode_anonymous(bytes).unwrap_or(Tag::Compound(Vec::new())),
                        None => Tag::Compound(Vec::new()),
                    });
                    let result = match operation.op {
                        MergeStrategy::Replace => operation.target.set(data, found.last().expect("non-empty")).map(|_| ()),
                        MergeStrategy::Append => operation.target.get_or_create_and_modify(
                            data,
                            &|| Tag::List { element_type: 0, values: Vec::new() },
                            &mut |list| {
                                if let Tag::List { element_type, values } = list {
                                    for value in &found {
                                        // `ListTag.add` throws for a tag of another element type.
                                        if value.tag_id() == 0 || (*element_type != 0 && *element_type != value.tag_id()) {
                                            return Err(crate::nbt_path::PathError::Runtime("UnsupportedOperationException".to_owned()));
                                        }
                                        *element_type = value.tag_id();
                                        values.push(value.clone());
                                    }
                                }
                                Ok(())
                            },
                        ),
                        MergeStrategy::Merge => operation.target.get_or_create_and_modify(data, &|| Tag::Compound(Vec::new()), &mut |compound| {
                            if matches!(compound, Tag::Compound(_)) {
                                for value in &found {
                                    if matches!(value, Tag::Compound(_)) {
                                        merge_compound(compound, value);
                                    }
                                }
                            }
                            Ok(())
                        }),
                    };
                    match result {
                        Ok(()) => {}
                        // `CommandSyntaxException` is swallowed per operation; runtime exceptions escape.
                        Err(crate::nbt_path::PathError::Runtime(what)) => return Err(LootError::Host(format!("copy_custom_data: {what}"))),
                        Err(_) => {}
                    }
                }
                if let Some(data) = target {
                    if !stack.is_empty() {
                        // `CustomData.set`: an empty compound removes the component.
                        if matches!(&data, Tag::Compound(entries) if entries.is_empty()) {
                            stack.components.retain(|c| c.type_id != CUSTOM_DATA);
                        } else {
                            let bytes = data.encode_network_anonymous().map_err(host)?;
                            set_component(&mut stack, CUSTOM_DATA, bytes);
                        }
                    }
                }
                Ok(stack)
            }
            FunctionKind::CopyName(source) => {
                let name = match source {
                    NameSource::Entity(target) => match ctx.entity(*target) {
                        None => return Ok(stack),
                        Some(entity) => entity.custom_name_nbt()?,
                    },
                    NameSource::BlockEntity => match ctx.params.block_entity {
                        None => return Ok(stack),
                        Some(block_entity) => match block_entity.nameable_custom_name_nbt()? {
                            None => return Ok(stack),
                            Some(name) => name,
                        },
                    },
                };
                if !stack.is_empty() {
                    match name {
                        Some(bytes) => set_component(&mut stack, CUSTOM_NAME, bytes),
                        None => remove_component(&mut stack, CUSTOM_NAME),
                    }
                }
                Ok(stack)
            }
            FunctionKind::FillPlayerHead(target) => {
                if stack_item_id(&stack) == "minecraft:player_head" {
                    if let Some(entity) = ctx.entity(*target) {
                        if let Some(profile) = entity.player_profile()? {
                            set_component(&mut stack, PROFILE, profile);
                        }
                    }
                }
                Ok(stack)
            }
            FunctionKind::SetCustomData(patch) => {
                // `CustomData.update`: merge into a copy; an empty result removes the component.
                let mut tag = match stack.effective_component_bytes(CUSTOM_DATA) {
                    Some(bytes) => Tag::decode_anonymous(bytes).map_err(host)?,
                    None => Tag::Compound(Vec::new()),
                };
                merge_compound(&mut tag, patch);
                if !stack.is_empty() {
                    if matches!(&tag, Tag::Compound(entries) if entries.is_empty()) {
                        stack.components.retain(|c| c.type_id != CUSTOM_DATA);
                    } else {
                        let bytes = tag.encode_network_anonymous().map_err(host)?;
                        set_component(&mut stack, CUSTOM_DATA, bytes);
                    }
                }
                Ok(stack)
            }
            FunctionKind::SetContents { component, entries } => {
                if stack.is_empty() {
                    return Ok(stack);
                }
                let items = super::entry::create_contents(entries, ctx)?;
                component.set(&mut stack, items)?;
                Ok(stack)
            }
            FunctionKind::ModifyContents { component, modifier } => {
                if stack.is_empty() {
                    return Ok(stack);
                }
                let Some(bytes) = stack.effective_component_bytes(component.type_id()) else {
                    return Ok(stack);
                };
                let items = component.items(bytes)?;
                let mut modified = Vec::with_capacity(items.len());
                for item in items {
                    if item.is_empty() {
                        modified.push(item);
                        continue;
                    }
                    let mut item = modifier.apply(item, ctx)?;
                    // `limitSize(getMaxStackSize())`.
                    if !item.is_empty() {
                        let mut probe = item.clone();
                        probe.count = 1;
                        let maximum = probe.max_stack_size().map_err(host)?;
                        if item.count > maximum {
                            item.count = maximum;
                        }
                    }
                    modified.push(item);
                }
                component.set(&mut stack, modified)?;
                Ok(stack)
            }
            FunctionKind::SetAttributes { modifiers, replace } => {
                let (mut entries, show) = if *replace {
                    (Vec::new(), true)
                } else {
                    let (entries, show) = attribute_modifiers(stack.effective_component_bytes(ATTRIBUTE_MODIFIERS).unwrap_or(&[0, 1]))?;
                    if entries.is_empty() && is_armor_item(&stack_item_id(&stack)) {
                        return unsupported("set_attributes without replace on armor (ArmorItem.getDefaultAttributeModifiers)");
                    }
                    (entries, show)
                };
                for modifier in modifiers {
                    let slot = modifier.slots[ctx.random().next_bounded_i32(modifier.slots.len() as i32)? as usize];
                    let amount = f64::from(modifier.amount.get_float(ctx)?);
                    entries.retain(|entry| !(entry.attribute == modifier.attribute && entry.id == modifier.id));
                    let mut writer = Writer::new();
                    writer.varint(modifier.attribute).string(&modifier.id).raw(&amount.to_be_bytes()).varint(modifier.operation).varint(slot);
                    entries.push(AttributeEntry { attribute: modifier.attribute, id: modifier.id.clone(), bytes: writer.into_inner() });
                }
                let mut writer = Writer::new();
                writer.varint(entries.len() as i32);
                for entry in &entries {
                    writer.raw(&entry.bytes);
                }
                writer.bool(show);
                if !stack.is_empty() {
                    set_component(&mut stack, ATTRIBUTE_MODIFIERS, writer.into_inner());
                }
                Ok(stack)
            }
            FunctionKind::SetBookCover { title, author, generation } => {
                let mut book = WrittenBook::decode(stack.effective_component_bytes(WRITTEN_BOOK_CONTENT))?;
                if let Some(title) = title {
                    book.title = title.clone();
                }
                if let Some(author) = author {
                    book.author = author.clone();
                }
                if let Some(generation) = generation {
                    book.generation = *generation;
                }
                if !stack.is_empty() {
                    set_component(&mut stack, WRITTEN_BOOK_CONTENT, book.encode());
                }
                Ok(stack)
            }
            FunctionKind::SetWrittenBookPages { pages, mode } => {
                let mut book = WrittenBook::decode(stack.effective_component_bytes(WRITTEN_BOOK_CONTENT))?;
                let new = pages.iter().map(written_page).collect::<Result<Vec<_>>>()?;
                book.pages = mode.apply(&book.pages, &new, usize::MAX);
                // `withReplacedPages` marks the content unresolved.
                book.resolved = false;
                if !stack.is_empty() {
                    set_component(&mut stack, WRITTEN_BOOK_CONTENT, book.encode());
                }
                Ok(stack)
            }
            FunctionKind::SetWritableBookPages { pages, mode } => {
                let old = writable_pages(stack.effective_component_bytes(WRITABLE_BOOK_CONTENT))?;
                let list = mode.apply(&old, pages, 100);
                let mut writer = Writer::new();
                writer.varint(list.len() as i32);
                for page in &list {
                    writer.string(&page.raw);
                    match &page.filtered {
                        Some(filtered) => writer.bool(true).string(filtered),
                        None => writer.bool(false),
                    };
                }
                if !stack.is_empty() {
                    set_component(&mut stack, WRITABLE_BOOK_CONTENT, writer.into_inner());
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
