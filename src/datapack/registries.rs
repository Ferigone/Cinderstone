//! The synchronized dynamic registries (WP-E1 stage 2): loading their JSON from the selected
//! packs as `RegistryDataLoader` does, encoding each element with its network codec as
//! `RegistrySynchronization` does, and the `ClientboundRegistryDataPacket` payload.
//!
//! The codecs are written out per registry; a value the original's codec rejects is a
//! [`RegistryError`]. `minecraft:enchantment` is loaded (ids and order) but its element codec is not
//! ported: an enchantment whose data has to be sent (a client that does not know the pack it came
//! from) is [`RegistryError::Unsupported`]. Evidence and limits: `docs/reference/datapack-registries.md`.

use crate::components::decode::{parse_style, Val};
use crate::components::java::{hashmap_iteration_order, json_number_int_value, resource_location};
use crate::components::text::{style_entries, Component};
use crate::nbt::{JavaString, Tag};
use crate::protocol::Writer;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// `RegistryDataLoader.SYNCHRONIZED_REGISTRIES`, in the order they are sent.
pub const SYNCHRONIZED: [&str; 11] = [
    "worldgen/biome",
    "chat_type",
    "trim_pattern",
    "trim_material",
    "wolf_variant",
    "painting_variant",
    "dimension_type",
    "damage_type",
    "banner_pattern",
    "enchantment",
    "jukebox_song",
];

/// Registries a synchronized element may refer to by id.
#[derive(Debug, Clone, Default)]
pub struct KnownIds {
    pub items: BTreeSet<String>,
    pub sound_events: BTreeSet<String>,
    pub biomes: BTreeSet<String>,
    pub armor_materials: BTreeSet<String>,
    pub particle_types: BTreeSet<String>,
    /// Particle types without options (`SimpleParticleType`).
    pub simple_particle_types: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The codec rejects the value.
    Codec(String),
    /// A form the original accepts that this port does not encode.
    Unsupported(String),
}

type Result<T> = std::result::Result<T, RegistryError>;

fn err<T>(what: impl Into<String>) -> Result<T> {
    Err(RegistryError::Codec(what.into()))
}

/// A compound written by a codec: `entries` in the order the codec inserts them (for a
/// `RecordCodecBuilder` this is not the declaration order; each order below was read off the
/// original's `JsonOps` output, where insertion order is visible). The compound then iterates in
/// its `HashMap` order, which differs from insertion order only for keys that share a bucket.
/// `compound` with the entries sorted into the codec's insertion order `order`.
fn compound_in(mut entries: Vec<(&str, Tag)>, order: &[&str]) -> Tag {
    entries.sort_by_key(|(key, _)| order.iter().position(|k| k == key).unwrap_or(usize::MAX));
    compound(entries)
}

fn compound(entries: Vec<(&str, Tag)>) -> Tag {
    let keys: Vec<&str> = entries.iter().map(|(k, _)| *k).collect();
    let order = hashmap_iteration_order(&keys);
    let mut slots: Vec<Option<(&str, Tag)>> = entries.into_iter().map(Some).collect();
    Tag::Compound(
        order
            .into_iter()
            .map(|i| {
                let (k, v) = slots[i].take().expect("each once");
                (JavaString::from(k), v)
            })
            .collect(),
    )
}

fn string_tag(text: &str) -> Tag {
    Tag::String(JavaString::from(text))
}

/// A JSON object read through `JsonOps`: a `null` member is absent.
struct Obj<'a> {
    map: &'a Map<String, Value>,
    path: String,
}

impl<'a> Obj<'a> {
    fn of(value: &'a Value, path: &str) -> Result<Self> {
        match value {
            Value::Object(map) => Ok(Self { map, path: path.to_owned() }),
            other => err(format!("{path}: not a map: {other}")),
        }
    }

    fn get(&self, key: &str) -> Option<&'a Value> {
        self.map.get(key).filter(|v| !v.is_null())
    }

    fn need(&self, key: &str) -> Result<&'a Value> {
        self.get(key)
            .ok_or_else(|| RegistryError::Codec(format!("{}: no key {key}", self.path)))
    }

    fn at(&self, key: &str) -> String {
        format!("{}.{key}", self.path)
    }
}

fn string(value: &Value, path: &str) -> Result<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        other => err(format!("{path}: not a string: {other}")),
    }
}

fn boolean(value: &Value, path: &str) -> Result<bool> {
    match value {
        Value::Bool(flag) => Ok(*flag),
        other => err(format!("{path}: not a boolean: {other}")),
    }
}

fn int(value: &Value, path: &str) -> Result<i32> {
    match value {
        Value::Number(n) => Ok(json_number_int_value(n)),
        other => err(format!("{path}: not a number: {other}")),
    }
}

fn int_range(value: &Value, path: &str, min: i32, max: i32) -> Result<i32> {
    let n = int(value, path)?;
    if n < min || n > max {
        return err(format!("{path}: value {n} outside of range [{min}:{max}]"));
    }
    Ok(n)
}

fn long(value: &Value, path: &str) -> Result<i64> {
    match value {
        Value::Number(n) => Ok(crate::components::java::json_number_long_value(n)),
        other => err(format!("{path}: not a number: {other}")),
    }
}

fn float(value: &Value, path: &str) -> Result<f32> {
    match value {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0) as f32),
        other => err(format!("{path}: not a number: {other}")),
    }
}

fn double(value: &Value, path: &str) -> Result<f64> {
    match value {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
        other => err(format!("{path}: not a number: {other}")),
    }
}

/// `ResourceLocation.CODEC`.
fn location(value: &Value, path: &str) -> Result<String> {
    let text = string(value, path)?;
    resource_location(&text).ok_or_else(|| RegistryError::Codec(format!("{path}: not a valid resource location: {text}")))
}

/// A holder by id in a fixed registry (`holderByNameCodec` / `RegistryFixedCodec`).
fn holder(value: &Value, path: &str, known: &BTreeSet<String>, what: &str) -> Result<String> {
    let id = location(value, path)?;
    if !known.contains(&id) {
        return err(format!("{path}: unknown {what} {id}"));
    }
    Ok(id)
}

/// A `StringRepresentable` enum by name.
fn named(value: &Value, path: &str, names: &[&str]) -> Result<String> {
    let text = string(value, path)?;
    if names.contains(&text.as_str()) {
        Ok(text)
    } else {
        err(format!("{path}: unknown element name {text}"))
    }
}

fn component(value: &Value, path: &str) -> Result<Tag> {
    let parsed = Component::from_json(value)
        .map_err(|e| RegistryError::Codec(format!("{path}: {e}")))?;
    Ok(parsed.to_nbt())
}

/// `SoundEvent.CODEC`: a registered id, or an inline `{sound_id, range?}`.
fn sound(value: &Value, path: &str, known: &KnownIds) -> Result<Tag> {
    match value {
        Value::Object(_) => {
            let obj = Obj::of(value, path)?;
            let id = location(obj.need("sound_id")?, &obj.at("sound_id"))?;
            let mut entries = vec![("sound_id", string_tag(&id))];
            if let Some(range) = obj.get("range") {
                entries.push(("range", Tag::Float(float(range, &obj.at("range"))?)));
            }
            Ok(compound(entries))
        }
        _ => Ok(string_tag(&holder(value, path, &known.sound_events, "sound event")?)),
    }
}

fn bool_tag(flag: bool) -> Tag {
    Tag::Byte(i8::from(flag))
}

fn banner_pattern(value: &Value) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let asset = location(obj.need("asset_id")?, &obj.at("asset_id"))?;
    let key = string(obj.need("translation_key")?, &obj.at("translation_key"))?;
    Ok(compound(vec![("asset_id", string_tag(&asset)), ("translation_key", string_tag(&key))]))
}

fn painting_variant(value: &Value) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let width = int_range(obj.need("width")?, &obj.at("width"), 1, 16)?;
    let height = int_range(obj.need("height")?, &obj.at("height"), 1, 16)?;
    let asset = location(obj.need("asset_id")?, &obj.at("asset_id"))?;
    Ok(compound(vec![("width", Tag::Int(width)), ("height", Tag::Int(height)), ("asset_id", string_tag(&asset))]))
}

fn damage_type(value: &Value) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let message = string(obj.need("message_id")?, &obj.at("message_id"))?;
    let scaling = named(obj.need("scaling")?, &obj.at("scaling"), &["never", "when_caused_by_living_non_player", "always"])?;
    let exhaustion = float(obj.need("exhaustion")?, &obj.at("exhaustion"))?;
    let mut entries = vec![("exhaustion", Tag::Float(exhaustion))];
    if let Some(effects) = obj.get("effects") {
        let effects = named(effects, &obj.at("effects"), &["hurt", "thorns", "drowning", "burning", "poking", "freezing"])?;
        if effects != "hurt" {
            entries.push(("effects", string_tag(&effects)));
        }
    }
    if let Some(kind) = obj.get("death_message_type") {
        let kind = named(kind, &obj.at("death_message_type"), &["default", "fall_variants", "intentional_game_design"])?;
        if kind != "default" {
            entries.push(("death_message_type", string_tag(&kind)));
        }
    }
    entries.push(("message_id", string_tag(&message)));
    entries.push(("scaling", string_tag(&scaling)));
    Ok(compound(entries))
}

fn chat_decoration(value: &Value, path: &str) -> Result<Tag> {
    let obj = Obj::of(value, path)?;
    let key = string(obj.need("translation_key")?, &obj.at("translation_key"))?;
    let Value::Array(items) = obj.need("parameters")? else {
        return err(format!("{}: not a list", obj.at("parameters")));
    };
    let mut parameters = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let name = named(item, &format!("{}[{index}]", obj.at("parameters")), &["sender", "target", "content"])?;
        parameters.push(string_tag(&name));
    }
    let mut entries = vec![
        ("translation_key", string_tag(&key)),
        ("parameters", Tag::List { element_type: if parameters.is_empty() { 0 } else { 8 }, values: parameters }),
    ];
    if let Some(style) = obj.get("style") {
        if !style.is_object() {
            return err(format!("{}: Not a map: {style}", obj.at("style")));
        }
        let parsed = parse_style(&Val::from_json(style)).map_err(|e| RegistryError::Codec(format!("{}: {e}", obj.at("style"))))?;
        let style_tags = style_entries(&parsed);
        if !style_tags.is_empty() {
            entries.push(("style", compound(style_tags)));
        }
    }
    Ok(compound(entries))
}

fn chat_type(value: &Value) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let chat = chat_decoration(obj.need("chat")?, &obj.at("chat"))?;
    let narration = chat_decoration(obj.need("narration")?, &obj.at("narration"))?;
    Ok(compound(vec![("chat", chat), ("narration", narration)]))
}

fn trim_pattern(value: &Value, known: &KnownIds) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let asset = location(obj.need("asset_id")?, &obj.at("asset_id"))?;
    let item = holder(obj.need("template_item")?, &obj.at("template_item"), &known.items, "item")?;
    let description = component(obj.need("description")?, &obj.at("description"))?;
    // `fieldOf("decal").orElse(false)`: a missing or unreadable value is false.
    let decal = obj.get("decal").and_then(|v| v.as_bool()).unwrap_or(false);
    Ok(compound(vec![
        ("asset_id", string_tag(&asset)),
        ("template_item", string_tag(&item)),
        ("description", description),
        ("decal", bool_tag(decal)),
    ]))
}

/// `ExtraCodecs.RESOURCE_PATH_CODEC`: the characters a resource-location path allows.
fn resource_path(value: &Value, path: &str) -> Result<String> {
    let text = string(value, path)?;
    if text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '/')) {
        Ok(text)
    } else {
        err(format!("{path}: invalid path {text}"))
    }
}

fn trim_material(value: &Value, known: &KnownIds) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let asset = resource_path(obj.need("asset_name")?, &obj.at("asset_name"))?;
    let item = holder(obj.need("ingredient")?, &obj.at("ingredient"), &known.items, "item")?;
    let index = float(obj.need("item_model_index")?, &obj.at("item_model_index"))?;
    let mut entries = vec![("item_model_index", Tag::Float(index))];
    if let Some(overrides) = obj.get("override_armor_materials") {
        let map = Obj::of(overrides, &obj.at("override_armor_materials"))?;
        let mut pairs: Vec<(String, String)> = Vec::new();
        for (key, value) in map.map {
            let path = map.at(key);
            let id = holder(&Value::String(key.clone()), &path, &known.armor_materials, "armor material")?;
            let name = string(value, &path)?;
            pairs.push((id, name));
        }
        if !pairs.is_empty() {
            let refs: Vec<(&str, Tag)> = pairs.iter().map(|(k, v)| (k.as_str(), string_tag(v))).collect();
            entries.push(("override_armor_materials", compound(refs)));
        }
    }
    entries.push(("description", component(obj.need("description")?, &obj.at("description"))?));
    entries.push(("asset_name", string_tag(&asset)));
    entries.push(("ingredient", string_tag(&item)));
    Ok(compound(entries))
}

/// `RegistryCodecs.homogeneousList(BIOME)`: `#tag`, an id, or a list of ids.
fn biome_set(value: &Value, path: &str, known: &KnownIds) -> Result<Tag> {
    match value {
        Value::String(text) if text.starts_with('#') => {
            let tag = resource_location(&text[1..]).ok_or_else(|| RegistryError::Codec(format!("{path}: invalid tag {text}")))?;
            Ok(string_tag(&format!("#{tag}")))
        }
        Value::Array(items) => {
            let mut ids = Vec::new();
            for (index, item) in items.iter().enumerate() {
                ids.push(holder(item, &format!("{path}[{index}]"), &known.biomes, "biome")?);
            }
            if ids.len() == 1 {
                Ok(string_tag(&ids[0]))
            } else {
                Ok(Tag::List { element_type: if ids.is_empty() { 0 } else { 8 }, values: ids.iter().map(|id| string_tag(id)).collect() })
            }
        }
        _ => Ok(string_tag(&holder(value, path, &known.biomes, "biome")?)),
    }
}

fn wolf_variant(value: &Value, known: &KnownIds) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let wild = location(obj.need("wild_texture")?, &obj.at("wild_texture"))?;
    let tame = location(obj.need("tame_texture")?, &obj.at("tame_texture"))?;
    let angry = location(obj.need("angry_texture")?, &obj.at("angry_texture"))?;
    let biomes = biome_set(obj.need("biomes")?, &obj.at("biomes"), known)?;
    Ok(compound(vec![
        ("wild_texture", string_tag(&wild)),
        ("tame_texture", string_tag(&tame)),
        ("angry_texture", string_tag(&angry)),
        ("biomes", biomes),
    ]))
}

fn jukebox_song(value: &Value, known: &KnownIds) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let event = sound(obj.need("sound_event")?, &obj.at("sound_event"), known)?;
    let description = component(obj.need("description")?, &obj.at("description"))?;
    let length = float(obj.need("length_in_seconds")?, &obj.at("length_in_seconds"))?;
    // `ExtraCodecs.POSITIVE_FLOAT`: (0, Float.MAX_VALUE], so an infinite value is rejected too.
    if !(length > 0.0 && length <= f32::MAX) {
        return err(format!("{}: Value must be positive: {length}", obj.at("length_in_seconds")));
    }
    let output = int_range(obj.need("comparator_output")?, &obj.at("comparator_output"), 0, 15)?;
    Ok(compound(vec![
        ("sound_event", event),
        ("description", description),
        ("length_in_seconds", Tag::Float(length)),
        ("comparator_output", Tag::Int(output)),
    ]))
}

/// `IntProvider.codec(min, max)`: an int, or a typed provider whose bounds fit the range.
fn int_provider(value: &Value, path: &str, min: i32, max: i32) -> Result<Tag> {
    if let Value::Number(_) = value {
        let n = int(value, path)?;
        if n < min || n > max {
            return err(format!("{path}: Value provider too low/high: {n}"));
        }
        return Ok(Tag::Int(n));
    }
    let obj = Obj::of(value, path)?;
    let kind = location(obj.need("type")?, &obj.at("type"))?;
    let bounds = |lo: i32, hi: i32| -> Result<()> {
        if lo < min || hi > max {
            err(format!("{path}: Value provider too low/high"))
        } else {
            Ok(())
        }
    };
    match kind.as_str() {
        "minecraft:constant" => {
            let n = int(obj.need("value")?, &obj.at("value"))?;
            bounds(n, n)?;
            // A constant provider is written in its compact form.
            Ok(Tag::Int(n))
        }
        "minecraft:uniform" | "minecraft:biased_to_bottom" => {
            let lo = int(obj.need("min_inclusive")?, &obj.at("min_inclusive"))?;
            let hi = int(obj.need("max_inclusive")?, &obj.at("max_inclusive"))?;
            if hi < lo {
                return err(format!("{path}: Max must be at least min, min_inclusive: {lo}, max_inclusive: {hi}"));
            }
            bounds(lo, hi)?;
            Ok(compound(vec![("min_inclusive", Tag::Int(lo)), ("max_inclusive", Tag::Int(hi)), ("type", string_tag(&kind))]))
        }
        _ => Err(RegistryError::Unsupported(format!("{path}: int provider {kind}"))),
    }
}

/// The order `DimensionType.DIRECT_CODEC` inserts its fields in.
const DIMENSION_TYPE_ORDER: [&str; 18] = [
    "infiniburn",
    "effects",
    "ambient_light",
    "piglin_safe",
    "has_raids",
    "monster_spawn_light_level",
    "monster_spawn_block_light_limit",
    "respawn_anchor_works",
    "min_y",
    "height",
    "logical_height",
    "ultrawarm",
    "natural",
    "coordinate_scale",
    "bed_works",
    "fixed_time",
    "has_skylight",
    "has_ceiling",
];

/// The order `BiomeSpecialEffects.CODEC` inserts its fields in.
const BIOME_EFFECTS_ORDER: [&str; 12] = [
    "mood_sound",
    "additions_sound",
    "music",
    "grass_color_modifier",
    "particle",
    "ambient_sound",
    "sky_color",
    "foliage_color",
    "grass_color",
    "fog_color",
    "water_color",
    "water_fog_color",
];

const MIN_Y: i32 = -2032;
const MAX_Y: i32 = 2031;
const Y_SIZE: i32 = 4064;

fn dimension_type(value: &Value) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let mut entries: Vec<(&str, Tag)> = Vec::new();
    // `lenientOptionalFieldOf("fixed_time")`.
    if let Some(Value::Number(n)) = obj.get("fixed_time") {
        let _ = n;
        entries.push(("fixed_time", Tag::Long(long(obj.get("fixed_time").unwrap(), "fixed_time")?)));
    }
    for key in ["has_skylight", "has_ceiling", "ultrawarm", "natural"] {
        entries.push((key, bool_tag(boolean(obj.need(key)?, &obj.at(key))?)));
    }
    let scale = double(obj.need("coordinate_scale")?, &obj.at("coordinate_scale"))?;
    if !(scale >= 1.0e-5f32 as f64 && scale <= 3.0e7) {
        return err(format!("{}: Value {scale} outside of range", obj.at("coordinate_scale")));
    }
    entries.push(("coordinate_scale", Tag::Double(scale)));
    for key in ["bed_works", "respawn_anchor_works"] {
        entries.push((key, bool_tag(boolean(obj.need(key)?, &obj.at(key))?)));
    }
    let min_y = int_range(obj.need("min_y")?, &obj.at("min_y"), MIN_Y, MAX_Y)?;
    let height = int_range(obj.need("height")?, &obj.at("height"), 16, Y_SIZE)?;
    let logical = int_range(obj.need("logical_height")?, &obj.at("logical_height"), 0, Y_SIZE)?;
    entries.push(("min_y", Tag::Int(min_y)));
    entries.push(("height", Tag::Int(height)));
    entries.push(("logical_height", Tag::Int(logical)));
    let infiniburn = string(obj.need("infiniburn")?, &obj.at("infiniburn"))?;
    let Some(tag) = infiniburn.strip_prefix('#').and_then(resource_location) else {
        return err(format!("{}: Not a tag id {infiniburn}", obj.at("infiniburn")));
    };
    entries.push(("infiniburn", string_tag(&format!("#{tag}"))));
    // `fieldOf("effects").orElse(overworld)`.
    let effects = obj
        .get("effects")
        .and_then(|v| v.as_str().and_then(resource_location))
        .unwrap_or_else(|| "minecraft:overworld".to_owned());
    entries.push(("effects", string_tag(&effects)));
    entries.push(("ambient_light", Tag::Float(float(obj.need("ambient_light")?, &obj.at("ambient_light"))?)));
    for key in ["piglin_safe", "has_raids"] {
        entries.push((key, bool_tag(boolean(obj.need(key)?, &obj.at(key))?)));
    }
    entries.push((
        "monster_spawn_light_level",
        int_provider(obj.need("monster_spawn_light_level")?, &obj.at("monster_spawn_light_level"), 0, 15)?,
    ));
    entries.push((
        "monster_spawn_block_light_limit",
        Tag::Int(int_range(obj.need("monster_spawn_block_light_limit")?, &obj.at("monster_spawn_block_light_limit"), 0, 15)?),
    ));
    // `DimensionType.guardY`.
    if height % 16 != 0 {
        return err("height has to be multiple of 16");
    }
    if min_y % 16 != 0 {
        return err("min_y has to be a multiple of 16");
    }
    if min_y + height > MAX_Y + 1 {
        return err(format!("min_y + height cannot be higher than: {}", MAX_Y + 1));
    }
    if logical > height {
        return err("logical_height cannot be higher than height");
    }
    Ok(compound_in(entries, &DIMENSION_TYPE_ORDER))
}

/// `Biome.NETWORK_CODEC`: the climate and the special effects (the generation settings and mob
/// settings that `Biome.DIRECT_CODEC` also reads are not validated here).
fn biome(value: &Value, known: &KnownIds) -> Result<Tag> {
    let obj = Obj::of(value, "")?;
    let mut entries = vec![
        ("has_precipitation", bool_tag(boolean(obj.need("has_precipitation")?, &obj.at("has_precipitation"))?)),
        ("temperature", Tag::Float(float(obj.need("temperature")?, &obj.at("temperature"))?)),
    ];
    if let Some(modifier) = obj.get("temperature_modifier") {
        let modifier = named(modifier, &obj.at("temperature_modifier"), &["none", "frozen"])?;
        if modifier != "none" {
            entries.push(("temperature_modifier", string_tag(&modifier)));
        }
    }
    entries.push(("downfall", Tag::Float(float(obj.need("downfall")?, &obj.at("downfall"))?)));
    let fx = Obj::of(obj.need("effects")?, &obj.at("effects"))?;
    let mut effects = Vec::new();
    for key in ["fog_color", "water_color", "water_fog_color", "sky_color"] {
        effects.push((key, Tag::Int(int(fx.need(key)?, &fx.at(key))?)));
    }
    for key in ["foliage_color", "grass_color"] {
        if let Some(color) = fx.get(key) {
            effects.push((key, Tag::Int(int(color, &fx.at(key))?)));
        }
    }
    if let Some(modifier) = fx.get("grass_color_modifier") {
        let modifier = named(modifier, &fx.at("grass_color_modifier"), &["none", "dark_forest", "swamp"])?;
        if modifier != "none" {
            effects.push(("grass_color_modifier", string_tag(&modifier)));
        }
    }
    if let Some(particle) = fx.get("particle") {
        let p = Obj::of(particle, &fx.at("particle"))?;
        let options = Obj::of(p.need("options")?, &p.at("options"))?;
        let kind = holder(options.need("type")?, &options.at("type"), &known.particle_types, "particle type")?;
        if !known.simple_particle_types.contains(&kind) {
            return Err(RegistryError::Unsupported(format!("{}: particle options of {kind}", p.at("options"))));
        }
        let probability = float(p.need("probability")?, &p.at("probability"))?;
        effects.push((
            "particle",
            compound(vec![("options", compound(vec![("type", string_tag(&kind))])), ("probability", Tag::Float(probability))]),
        ));
    }
    if let Some(ambient) = fx.get("ambient_sound") {
        effects.push(("ambient_sound", sound(ambient, &fx.at("ambient_sound"), known)?));
    }
    if let Some(mood) = fx.get("mood_sound") {
        let m = Obj::of(mood, &fx.at("mood_sound"))?;
        effects.push((
            "mood_sound",
            compound(vec![
                ("sound", sound(m.need("sound")?, &m.at("sound"), known)?),
                ("tick_delay", Tag::Int(int(m.need("tick_delay")?, &m.at("tick_delay"))?)),
                ("block_search_extent", Tag::Int(int(m.need("block_search_extent")?, &m.at("block_search_extent"))?)),
                ("offset", Tag::Double(double(m.need("offset")?, &m.at("offset"))?)),
            ]),
        ));
    }
    if let Some(additions) = fx.get("additions_sound") {
        let a = Obj::of(additions, &fx.at("additions_sound"))?;
        effects.push((
            "additions_sound",
            compound(vec![
                ("sound", sound(a.need("sound")?, &a.at("sound"), known)?),
                ("tick_chance", Tag::Double(double(a.need("tick_chance")?, &a.at("tick_chance"))?)),
            ]),
        ));
    }
    if let Some(music) = fx.get("music") {
        let m = Obj::of(music, &fx.at("music"))?;
        effects.push((
            "music",
            compound(vec![
                ("sound", sound(m.need("sound")?, &m.at("sound"), known)?),
                ("min_delay", Tag::Int(int(m.need("min_delay")?, &m.at("min_delay"))?)),
                ("max_delay", Tag::Int(int(m.need("max_delay")?, &m.at("max_delay"))?)),
                ("replace_current_music", bool_tag(boolean(m.need("replace_current_music")?, &m.at("replace_current_music"))?)),
            ]),
        ));
    }
    entries.push(("effects", compound_in(effects, &BIOME_EFFECTS_ORDER)));
    Ok(compound(entries))
}

/// The network form of one element of a synchronized registry.
pub fn encode_element(registry: &str, value: &Value, known: &KnownIds) -> Result<Tag> {
    match registry {
        "worldgen/biome" => biome(value, known),
        "chat_type" => chat_type(value),
        "trim_pattern" => trim_pattern(value, known),
        "trim_material" => trim_material(value, known),
        "wolf_variant" => wolf_variant(value, known),
        "painting_variant" => painting_variant(value),
        "dimension_type" => dimension_type(value),
        "damage_type" => damage_type(value),
        "banner_pattern" => banner_pattern(value),
        "jukebox_song" => jukebox_song(value, known),
        other => Err(RegistryError::Unsupported(format!("element codec of {other}"))),
    }
}

/// One pack's files for one registry directory: id → JSON text.
#[derive(Debug, Clone, Default)]
pub struct PackRegistryFiles {
    pub pack_id: String,
    pub files: BTreeMap<String, String>,
}

/// A loaded element: its id, the pack that supplied it, and its parsed JSON.
#[derive(Debug, Clone)]
pub struct LoadedElement {
    pub id: String,
    pub pack_id: String,
    pub json: Value,
}

/// `ResourceLocation.compareTo`: the path first, then the namespace.
fn location_order(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |id: &str| -> (String, String) {
        let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
        (ns.to_owned(), path.to_owned())
    };
    let (an, ap) = split(a);
    let (bn, bp) = split(b);
    ap.cmp(&bp).then(an.cmp(&bn))
}

/// `RegistryDataLoader.loadContentsFromManager` for one registry: the files of the packs
/// (lowest priority first; a later pack's file replaces an earlier one's), in the order of their
/// locations, each read with `JsonParser.parseReader`. Errors are collected per element as the
/// original collects them; any error fails the whole load.
pub fn load_registry(packs: &[PackRegistryFiles]) -> std::result::Result<Vec<LoadedElement>, Vec<(String, String)>> {
    let mut chosen: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    for pack in packs {
        for (id, text) in &pack.files {
            chosen.insert(id.clone(), (pack.pack_id.as_str(), text.as_str()));
        }
    }
    let mut ids: Vec<&String> = chosen.keys().collect();
    ids.sort_by(|a, b| location_order(a, b));
    let mut elements = Vec::new();
    let mut errors = Vec::new();
    for id in ids {
        let (pack, text) = chosen[id];
        match crate::components::lenient::parse(text) {
            Ok(json) => elements.push(LoadedElement { id: id.clone(), pack_id: pack.to_owned(), json }),
            Err(error) => errors.push((id.clone(), error)),
        }
    }
    if errors.is_empty() {
        Ok(elements)
    } else {
        Err(errors)
    }
}

/// The payload of `ClientboundRegistryDataPacket` for one registry. An element whose pack the
/// client already knows (`omit`) is sent without data, as `RegistrySynchronization` does.
pub fn registry_data_payload(
    registry: &str,
    elements: &[LoadedElement],
    known: &KnownIds,
    omit: impl Fn(&LoadedElement) -> bool,
) -> Result<Vec<u8>> {
    let mut writer = Writer::new();
    writer.string(&format!("minecraft:{registry}"));
    writer.varint(elements.len() as i32);
    for element in elements {
        writer.string(&element.id);
        if omit(element) {
            writer.bool(false);
        } else {
            let tag = encode_element(registry, &element.json, known)?;
            writer.bool(true);
            writer.raw(&tag.encode_network_anonymous().map_err(|e| RegistryError::Codec(e.to_string()))?);
        }
    }
    Ok(writer.into_inner())
}
