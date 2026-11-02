//! `net.minecraft.network.chat.Component`: the text component model and its
//! `ComponentSerialization.CODEC` forms (JSON in, JSON and NBT out).
//!
//! Decoding follows the Java codec rules, including the ones that are easy to
//! miss: a JSON `null` member is absent, `lenientOptionalFieldOf` swallows a bad
//! value, the fuzzy contents matcher tries `text`, `translatable`, `keybind`,
//! `score`, `selector`, `nbt` in that order, and a plain component collapses to a
//! bare string. NBT output is the network form: compounds are written in
//! `java.util.HashMap` key order and a list of mixed tag types wraps each
//! non-compound element as `{"": element}`.
//!
//! Not ported and reported as [`ComponentError::Unsupported`]: `show_item` and
//! `show_entity` hover events (they need the item-stack and entity-tooltip codecs).

use super::java::hashmap_iteration_order;
use crate::nbt::{JavaString, Tag};
use serde_json::{Map, Number, Value};

/// Decode failure kinds. `Codec` is a rejection the original also makes;
/// `Unsupported` is outside the ported scope and says nothing about Java.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentError {
    Codec(String),
    Unsupported(String),
}

impl std::fmt::Display for ComponentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(what) => write!(f, "invalid component: {what}"),
            Self::Unsupported(what) => write!(f, "unsupported component form: {what}"),
        }
    }
}

impl std::error::Error for ComponentError {}

type Result<T> = std::result::Result<T, ComponentError>;

fn reject<T>(what: impl Into<String>) -> Result<T> {
    Err(ComponentError::Codec(what.into()))
}

/// `TextColor`: equality is by RGB value only, serialization keeps the name.
#[derive(Debug, Clone, Copy, Eq)]
pub struct TextColor {
    pub value: u32,
    name: Option<&'static str>,
}

impl PartialEq for TextColor {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

const NAMED_COLORS: [(&str, u32); 16] = [
    ("black", 0x000000),
    ("dark_blue", 0x0000AA),
    ("dark_green", 0x00AA00),
    ("dark_aqua", 0x00AAAA),
    ("dark_red", 0xAA0000),
    ("dark_purple", 0xAA00AA),
    ("gold", 0xFFAA00),
    ("gray", 0xAAAAAA),
    ("dark_gray", 0x555555),
    ("blue", 0x5555FF),
    ("green", 0x55FF55),
    ("aqua", 0x55FFFF),
    ("red", 0xFF5555),
    ("light_purple", 0xFF55FF),
    ("yellow", 0xFFFF55),
    ("white", 0xFFFFFF),
];

/// `Integer.parseInt(text, 16)`: optional sign, hex digits, must fit an `int`.
fn parse_int_hex(text: &str) -> Option<i32> {
    let (negative, digits) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let magnitude = i64::from_str_radix(digits, 16).ok()?;
    let value = if negative { -magnitude } else { magnitude };
    i32::try_from(value).ok()
}

impl TextColor {
    /// `TextColor.parseColor`.
    pub fn parse(text: &str) -> Result<Self> {
        if let Some(hex) = text.strip_prefix('#') {
            match parse_int_hex(hex) {
                Some(value) if (0..=0xFF_FFFF).contains(&value) => Ok(Self { value: value as u32, name: None }),
                Some(_) => reject(format!("Color value out of range: {text}")),
                None => reject(format!("Invalid color value: {text}")),
            }
        } else {
            NAMED_COLORS
                .iter()
                .find(|(name, _)| *name == text)
                .map(|(name, value)| Self { value: *value, name: Some(name) })
                .ok_or_else(|| ComponentError::Codec(format!("Invalid color name: {text}")))
        }
    }

    /// `TextColor.serialize`: the legacy name, or `#RRGGBB`.
    pub fn serialize(&self) -> String {
        match self.name {
            Some(name) => name.to_owned(),
            None => format!("#{:06X}", self.value),
        }
    }
}

/// `ClickEvent.Action` values the server may send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickAction {
    OpenUrl,
    RunCommand,
    SuggestCommand,
    ChangePage,
    CopyToClipboard,
}

impl ClickAction {
    fn name(self) -> &'static str {
        match self {
            Self::OpenUrl => "open_url",
            Self::RunCommand => "run_command",
            Self::SuggestCommand => "suggest_command",
            Self::ChangePage => "change_page",
            Self::CopyToClipboard => "copy_to_clipboard",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClickEvent {
    pub action: ClickAction,
    pub value: String,
}

/// `HoverEvent` (`show_text` only).
#[derive(Debug, Clone, PartialEq)]
pub struct HoverEvent {
    pub text: Box<Component>,
}

/// `Style`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    pub color: Option<TextColor>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
    pub obfuscated: Option<bool>,
    pub click_event: Option<ClickEvent>,
    pub hover_event: Option<HoverEvent>,
    pub insertion: Option<String>,
    pub font: Option<String>,
}

impl Style {
    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }
}

/// A primitive `translate` argument (`ExtraCodecs.JAVA` values).
#[derive(Debug, Clone)]
pub enum Primitive {
    Bool(bool),
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Str(String),
}

impl PartialEq for Primitive {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Byte(a), Self::Byte(b)) => a == b,
            (Self::Short(a), Self::Short(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Long(a), Self::Long(b)) => a == b,
            // `Float.equals`/`Double.equals` compare the raw bits.
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::Double(a), Self::Double(b)) => a.to_bits() == b.to_bits(),
            (Self::Str(a), Self::Str(b)) => a == b,
            _ => false,
        }
    }
}

/// A `translate` argument: a primitive or a component that is not collapsible.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Primitive(Primitive),
    Component(Component),
}

/// `nbt` component data source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataSource {
    Entity(String),
    Block(String),
    Storage(String),
}

/// `ComponentContents`.
#[derive(Debug, Clone, PartialEq)]
pub enum Contents {
    Text(String),
    Translatable { key: String, fallback: Option<String>, args: Vec<Arg> },
    Keybind(String),
    Score { name: String, objective: String },
    Selector { pattern: String, separator: Option<Box<Component>> },
    Nbt { path: String, interpret: bool, separator: Option<Box<Component>>, source: DataSource },
}

/// `Component` (`MutableComponent`): contents, siblings and style.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub contents: Contents,
    pub siblings: Vec<Component>,
    pub style: Style,
}

// ---------------------------------------------------------------- decoding

/// `JsonOps.JsonMapLike.get`: a `null` member is absent.
fn member<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    map.get(key).filter(|value| !value.is_null())
}

fn string_of(value: &Value, what: &str) -> Result<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        _ => reject(format!("{what}: expected a string")),
    }
}

fn required_string(map: &Map<String, Value>, key: &str) -> Result<String> {
    match member(map, key) {
        Some(value) => string_of(value, key),
        None => reject(format!("missing required field {key}")),
    }
}

/// `Codec.BOOL` over `JsonOps`: only a JSON boolean ("Not a boolean: 1" otherwise).
fn bool_of(value: &Value) -> Option<bool> {
    value.as_bool()
}

fn optional_bool(map: &Map<String, Value>, key: &str) -> Result<Option<bool>> {
    match member(map, key) {
        None => Ok(None),
        Some(value) => bool_of(value)
            .map(Some)
            .ok_or_else(|| ComponentError::Codec(format!("{key}: expected a boolean"))),
    }
}

/// `ResourceLocation.CODEC` (`bySeparator(text, ':')`): the first `:` splits namespace
/// and path, an empty namespace means `minecraft`, an empty path is legal.
fn resource_location(text: &str) -> Result<String> {
    let (namespace, path) = match text.split_once(':') {
        Some(("", path)) => ("minecraft", path),
        Some((namespace, path)) => (namespace, path),
        None => ("minecraft", text),
    };
    let namespace_ok = namespace
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.'));
    let path_ok = path
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '/'));
    if namespace_ok && path_ok {
        Ok(format!("{namespace}:{path}"))
    } else {
        reject(format!("Not a valid resource location: {text}"))
    }
}

/// `JsonOps.convertTo(JavaOps)` of a JSON number: `BigDecimal.longValueExact` narrowed to
/// byte, short, int or long; otherwise a float when `(float) d == d`, else a double.
/// `serde_json` keeps no decimal text, so an integer literal below `i64::MIN` arrives as
/// an `f64` and is indistinguishable from `-9223372036854775808.0` (read as a long).
fn number_primitive(number: &Number) -> Primitive {
    let exact_long = number.as_i64().or_else(|| {
        number.as_f64().and_then(|value| {
            (value.fract() == 0.0 && value >= -9.223_372_036_854_775_808e18 && value < 9.223_372_036_854_775_808e18)
                .then(|| value as i64)
        })
    });
    if let Some(long) = exact_long {
        if i64::from(long as i8) == long {
            return Primitive::Byte(long as i8);
        }
        if i64::from(long as i16) == long {
            return Primitive::Short(long as i16);
        }
        if i64::from(long as i32) == long {
            return Primitive::Int(long as i32);
        }
        return Primitive::Long(long);
    }
    let double = number.as_f64().unwrap_or(f64::NAN);
    if f64::from(double as f32) == double {
        Primitive::Float(double as f32)
    } else {
        Primitive::Double(double)
    }
}

/// `JavaOps.createMap` collects with `Collectors.toMap`, which throws on a null value, so
/// a translate argument holding an object with a JSON `null` member anywhere is a failure.
fn holds_null_member(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.values().any(|member| member.is_null() || holds_null_member(member)),
        Value::Array(items) => items.iter().any(holds_null_member),
        _ => false,
    }
}

fn decode_arg(value: &Value) -> Result<Arg> {
    if holds_null_member(value) {
        return reject("NullPointerException: JSON null in an object argument");
    }
    match value {
        Value::String(text) => Ok(Arg::Primitive(Primitive::Str(text.clone()))),
        Value::Bool(flag) => Ok(Arg::Primitive(Primitive::Bool(*flag))),
        Value::Number(number) => Ok(Arg::Primitive(number_primitive(number))),
        other => {
            let component = Component::from_json(other)?;
            Ok(match component.collapse_to_string() {
                Some(text) => Arg::Primitive(Primitive::Str(text.to_owned())),
                None => Arg::Component(component),
            })
        }
    }
}

type ContentsParser = fn(&Map<String, Value>) -> Result<Contents>;

const CONTENT_TYPES: [(&str, ContentsParser); 6] = [
    ("text", parse_text),
    ("translatable", parse_translatable),
    ("keybind", parse_keybind),
    ("score", parse_score),
    ("selector", parse_selector),
    ("nbt", parse_nbt),
];

fn parse_text(map: &Map<String, Value>) -> Result<Contents> {
    Ok(Contents::Text(required_string(map, "text")?))
}

fn parse_translatable(map: &Map<String, Value>) -> Result<Contents> {
    let key = required_string(map, "translate")?;
    // `lenientOptionalFieldOf`: a wrong-typed fallback is ignored.
    let fallback = member(map, "fallback").and_then(|v| v.as_str().map(str::to_owned));
    let args = match member(map, "with") {
        None => Vec::new(),
        Some(Value::Array(items)) => items.iter().map(decode_arg).collect::<Result<Vec<_>>>()?,
        Some(_) => return reject("with: expected a list"),
    };
    Ok(Contents::Translatable { key, fallback, args })
}

fn parse_keybind(map: &Map<String, Value>) -> Result<Contents> {
    Ok(Contents::Keybind(required_string(map, "keybind")?))
}

fn parse_score(map: &Map<String, Value>) -> Result<Contents> {
    let Some(Value::Object(inner)) = member(map, "score") else {
        return reject("score: expected an object");
    };
    Ok(Contents::Score {
        name: required_string(inner, "name")?,
        objective: required_string(inner, "objective")?,
    })
}

fn parse_selector(map: &Map<String, Value>) -> Result<Contents> {
    Ok(Contents::Selector {
        pattern: required_string(map, "selector")?,
        separator: match member(map, "separator") {
            Some(value) => Some(Box::new(Component::from_json(value)?)),
            None => None,
        },
    })
}

fn parse_data_source(map: &Map<String, Value>) -> Result<DataSource> {
    let entity = |map: &Map<String, Value>| required_string(map, "entity").map(DataSource::Entity);
    let block = |map: &Map<String, Value>| required_string(map, "block").map(DataSource::Block);
    let storage = |map: &Map<String, Value>| {
        required_string(map, "storage")
            .and_then(|text| resource_location(&text))
            .map(DataSource::Storage)
    };
    if let Some(kind) = member(map, "source") {
        return match kind.as_str() {
            Some("entity") => entity(map),
            Some("block") => block(map),
            Some("storage") => storage(map),
            _ => reject("source: unknown data source type"),
        };
    }
    entity(map).or_else(|_| block(map)).or_else(|_| storage(map)).or_else(|_| reject("No matching codec found"))
}

fn parse_nbt(map: &Map<String, Value>) -> Result<Contents> {
    let path = required_string(map, "nbt")?;
    // `lenientOptionalFieldOf`: bad values fall back to the default / absent.
    let interpret = member(map, "interpret").and_then(bool_of).unwrap_or(false);
    let separator = member(map, "separator")
        .and_then(|value| Component::from_json(value).ok())
        .map(Box::new);
    Ok(Contents::Nbt { path, interpret, separator, source: parse_data_source(map)? })
}

fn parse_contents(map: &Map<String, Value>) -> Result<Contents> {
    if let Some(kind) = member(map, "type") {
        let name = kind.as_str().ok_or_else(|| ComponentError::Codec("type: expected a string".to_owned()))?;
        return match CONTENT_TYPES.iter().find(|(known, _)| *known == name) {
            Some((_, parse)) => parse(map),
            None => reject(format!("Unknown component type {name}")),
        };
    }
    // `FuzzyCodec`: the first content type whose codec succeeds.
    for (_, parse) in CONTENT_TYPES {
        if let Ok(contents) = parse(map) {
            return Ok(contents);
        }
    }
    reject("No matching codec found")
}

fn parse_click_event(value: &Value) -> Result<ClickEvent> {
    let Value::Object(map) = value else {
        return reject("clickEvent: expected an object");
    };
    let action = match required_string(map, "action")?.as_str() {
        "open_url" => ClickAction::OpenUrl,
        "run_command" => ClickAction::RunCommand,
        "suggest_command" => ClickAction::SuggestCommand,
        "change_page" => ClickAction::ChangePage,
        "copy_to_clipboard" => ClickAction::CopyToClipboard,
        "open_file" => return reject("Action not allowed: OPEN_FILE"),
        other => return reject(format!("Unknown click action {other}")),
    };
    Ok(ClickEvent { action, value: required_string(map, "value")? })
}

fn parse_hover_event(value: &Value) -> Result<HoverEvent> {
    let Value::Object(map) = value else {
        return reject("hoverEvent: expected an object");
    };
    match required_string(map, "action")?.as_str() {
        "show_text" => {
            // `Codec.withAlternative(contents form, legacy value form)`: the legacy form
            // is tried only when the primary one fails.
            let primary = match member(map, "contents") {
                Some(contents) => Component::from_json(contents),
                None => reject("missing required field contents"),
            };
            let text = match (primary, member(map, "value")) {
                (Ok(text), _) => text,
                (Err(_), Some(legacy)) => Component::from_json(legacy)?,
                (Err(error), None) => return Err(error),
            };
            Ok(HoverEvent { text: Box::new(text) })
        }
        "show_item" | "show_entity" => Err(ComponentError::Unsupported("hoverEvent show_item/show_entity".to_owned())),
        other => reject(format!("Unknown hover action {other}")),
    }
}

fn parse_style(map: &Map<String, Value>) -> Result<Style> {
    Ok(Style {
        color: match member(map, "color") {
            Some(value) => Some(TextColor::parse(&string_of(value, "color")?)?),
            None => None,
        },
        bold: optional_bool(map, "bold")?,
        italic: optional_bool(map, "italic")?,
        underlined: optional_bool(map, "underlined")?,
        strikethrough: optional_bool(map, "strikethrough")?,
        obfuscated: optional_bool(map, "obfuscated")?,
        click_event: member(map, "clickEvent").map(parse_click_event).transpose()?,
        hover_event: member(map, "hoverEvent").map(parse_hover_event).transpose()?,
        insertion: member(map, "insertion").map(|v| string_of(v, "insertion")).transpose()?,
        font: match member(map, "font") {
            Some(value) => Some(resource_location(&string_of(value, "font")?)?),
            None => None,
        },
    })
}

impl Component {
    pub fn literal(text: impl Into<String>) -> Self {
        Self { contents: Contents::Text(text.into()), siblings: Vec::new(), style: Style::default() }
    }

    /// `Component.tryCollapseToString`.
    pub fn collapse_to_string(&self) -> Option<&str> {
        match &self.contents {
            Contents::Text(text) if self.siblings.is_empty() && self.style.is_empty() => Some(text),
            _ => None,
        }
    }

    /// `ComponentSerialization.CODEC` decoding of a JSON value.
    pub fn from_json(value: &Value) -> Result<Self> {
        match value {
            Value::String(text) => Ok(Self::literal(text.clone())),
            Value::Array(items) => {
                if items.is_empty() {
                    return reject("List must have contents");
                }
                let mut parts = items.iter().map(Self::from_json).collect::<Result<Vec<_>>>()?.into_iter();
                let mut first = parts.next().expect("non-empty");
                first.siblings.extend(parts);
                Ok(first)
            }
            Value::Object(map) => {
                let contents = parse_contents(map)?;
                let siblings = match member(map, "extra") {
                    None => Vec::new(),
                    Some(Value::Array(items)) if !items.is_empty() => {
                        items.iter().map(Self::from_json).collect::<Result<Vec<_>>>()?
                    }
                    Some(_) => return reject("extra: expected a non-empty list"),
                };
                Ok(Self { contents, siblings, style: parse_style(map)? })
            }
            _ => reject("expected a string, list or object"),
        }
    }
}

// ---------------------------------------------------------------- encoding

fn contents_json(contents: &Contents, out: &mut Map<String, Value>) {
    match contents {
        Contents::Text(text) => {
            out.insert("text".to_owned(), Value::String(text.clone()));
        }
        Contents::Translatable { key, fallback, args } => {
            out.insert("translate".to_owned(), Value::String(key.clone()));
            if let Some(fallback) = fallback {
                out.insert("fallback".to_owned(), Value::String(fallback.clone()));
            }
            if !args.is_empty() {
                out.insert("with".to_owned(), Value::Array(args.iter().map(arg_json).collect()));
            }
        }
        Contents::Keybind(name) => {
            out.insert("keybind".to_owned(), Value::String(name.clone()));
        }
        Contents::Score { name, objective } => {
            let mut inner = Map::new();
            inner.insert("name".to_owned(), Value::String(name.clone()));
            inner.insert("objective".to_owned(), Value::String(objective.clone()));
            out.insert("score".to_owned(), Value::Object(inner));
        }
        Contents::Selector { pattern, separator } => {
            out.insert("selector".to_owned(), Value::String(pattern.clone()));
            if let Some(separator) = separator {
                out.insert("separator".to_owned(), separator.to_json());
            }
        }
        Contents::Nbt { path, interpret, separator, source } => {
            out.insert("nbt".to_owned(), Value::String(path.clone()));
            if *interpret {
                out.insert("interpret".to_owned(), Value::Bool(true));
            }
            if let Some(separator) = separator {
                out.insert("separator".to_owned(), separator.to_json());
            }
            let (key, value) = match source {
                DataSource::Entity(text) => ("entity", text),
                DataSource::Block(text) => ("block", text),
                DataSource::Storage(text) => ("storage", text),
            };
            out.insert(key.to_owned(), Value::String(value.clone()));
        }
    }
}

fn primitive_json(primitive: &Primitive) -> Value {
    match primitive {
        Primitive::Bool(flag) => Value::Bool(*flag),
        Primitive::Byte(n) => Value::from(*n),
        Primitive::Short(n) => Value::from(*n),
        Primitive::Int(n) => Value::from(*n),
        Primitive::Long(n) => Value::from(*n),
        Primitive::Float(n) => format!("{n}")
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map_or(Value::Null, Value::Number),
        Primitive::Double(n) => Number::from_f64(*n).map_or(Value::Null, Value::Number),
        Primitive::Str(text) => Value::String(text.clone()),
    }
}

fn arg_json(arg: &Arg) -> Value {
    match arg {
        Arg::Primitive(primitive) => primitive_json(primitive),
        Arg::Component(component) => component.to_json(),
    }
}

impl Component {
    /// `ComponentSerialization.CODEC` encoding to JSON (a plain component is a bare string).
    pub fn to_json(&self) -> Value {
        if let Some(text) = self.collapse_to_string() {
            return Value::String(text.to_owned());
        }
        let mut out = Map::new();
        contents_json(&self.contents, &mut out);
        if !self.siblings.is_empty() {
            out.insert("extra".to_owned(), Value::Array(self.siblings.iter().map(Self::to_json).collect()));
        }
        let style = &self.style;
        if let Some(color) = &style.color {
            out.insert("color".to_owned(), Value::String(color.serialize()));
        }
        for (key, flag) in [
            ("bold", style.bold),
            ("italic", style.italic),
            ("underlined", style.underlined),
            ("strikethrough", style.strikethrough),
            ("obfuscated", style.obfuscated),
        ] {
            if let Some(flag) = flag {
                out.insert(key.to_owned(), Value::Bool(flag));
            }
        }
        if let Some(click) = &style.click_event {
            let mut inner = Map::new();
            inner.insert("action".to_owned(), Value::String(click.action.name().to_owned()));
            inner.insert("value".to_owned(), Value::String(click.value.clone()));
            out.insert("clickEvent".to_owned(), Value::Object(inner));
        }
        if let Some(hover) = &style.hover_event {
            let mut inner = Map::new();
            inner.insert("action".to_owned(), Value::String("show_text".to_owned()));
            inner.insert("contents".to_owned(), hover.text.to_json());
            out.insert("hoverEvent".to_owned(), Value::Object(inner));
        }
        if let Some(insertion) = &style.insertion {
            out.insert("insertion".to_owned(), Value::String(insertion.clone()));
        }
        if let Some(font) = &style.font {
            out.insert("font".to_owned(), Value::String(font.clone()));
        }
        Value::Object(out)
    }
}

fn nbt_string(text: &str) -> Tag {
    Tag::String(JavaString::Utf8(text.to_owned()))
}

/// A `CompoundTag` whose entries are stored in `java.util.HashMap` order.
fn compound(entries: Vec<(&str, Tag)>) -> Tag {
    let keys: Vec<&str> = entries.iter().map(|(key, _)| *key).collect();
    let order = hashmap_iteration_order(&keys);
    let mut slots: Vec<Option<(&str, Tag)>> = entries.into_iter().map(Some).collect();
    Tag::Compound(
        order
            .into_iter()
            .map(|index| {
                let (key, tag) = slots[index].take().expect("each index once");
                (JavaString::Utf8(key.to_owned()), tag)
            })
            .collect(),
    )
}

/// `{"": tag}`: how `NbtOps` stores an element of a mixed list.
fn wrap_element(tag: Tag) -> Tag {
    Tag::Compound(vec![(JavaString::Utf8(String::new()), tag)])
}

fn is_wrapper(tag: &Tag) -> bool {
    matches!(tag, Tag::Compound(entries) if entries.len() == 1 && entries[0].0 == "")
}

/// `HeterogenousListCollector.wrapIfNeeded`.
fn wrap_if_needed(tag: Tag) -> Tag {
    if matches!(tag, Tag::Compound(_)) && !is_wrapper(&tag) {
        tag
    } else {
        wrap_element(tag)
    }
}

/// `NbtOps.ListCollector`: the state machine behind `createList` and list
/// builders. A leading byte, int or long makes an array tag; a leading
/// compound or a type change makes a heterogeneous list whose non-compound
/// elements are wrapped; anything else stays a typed list.
enum ListCollector {
    Initial,
    Bytes(Vec<i8>),
    Ints(Vec<i32>),
    Longs(Vec<i64>),
    Homogeneous(Vec<Tag>),
    Heterogeneous(Vec<Tag>),
}

impl ListCollector {
    fn accept(self, tag: Tag) -> Self {
        match self {
            Self::Initial => match tag {
                Tag::Compound(_) => Self::Heterogeneous(vec![wrap_if_needed(tag)]),
                Tag::Byte(value) => Self::Bytes(vec![value]),
                Tag::Int(value) => Self::Ints(vec![value]),
                Tag::Long(value) => Self::Longs(vec![value]),
                other => Self::Homogeneous(vec![other]),
            },
            Self::Bytes(mut values) => match tag {
                Tag::Byte(value) => {
                    values.push(value);
                    Self::Bytes(values)
                }
                other => Self::Heterogeneous(values.into_iter().map(|v| wrap_element(Tag::Byte(v))).collect())
                    .accept(other),
            },
            Self::Ints(mut values) => match tag {
                Tag::Int(value) => {
                    values.push(value);
                    Self::Ints(values)
                }
                other => Self::Heterogeneous(values.into_iter().map(|v| wrap_element(Tag::Int(v))).collect())
                    .accept(other),
            },
            Self::Longs(mut values) => match tag {
                Tag::Long(value) => {
                    values.push(value);
                    Self::Longs(values)
                }
                other => Self::Heterogeneous(values.into_iter().map(|v| wrap_element(Tag::Long(v))).collect())
                    .accept(other),
            },
            Self::Homogeneous(mut values) => {
                if tag.tag_id() == values[0].tag_id() {
                    values.push(tag);
                    Self::Homogeneous(values)
                } else {
                    Self::Heterogeneous(values.into_iter().map(wrap_if_needed).collect()).accept(tag)
                }
            }
            Self::Heterogeneous(mut values) => {
                values.push(wrap_if_needed(tag));
                Self::Heterogeneous(values)
            }
        }
    }

    fn result(self) -> Tag {
        match self {
            Self::Initial => Tag::List { element_type: 0, values: Vec::new() },
            Self::Bytes(values) => Tag::ByteArray(values),
            Self::Ints(values) => Tag::IntArray(values),
            Self::Longs(values) => Tag::LongArray(values),
            Self::Homogeneous(values) => Tag::List { element_type: values[0].tag_id(), values },
            Self::Heterogeneous(values) => Tag::List { element_type: 10, values },
        }
    }
}

fn nbt_list(items: Vec<Tag>) -> Tag {
    items.into_iter().fold(ListCollector::Initial, ListCollector::accept).result()
}

fn primitive_nbt(primitive: &Primitive) -> Tag {
    match primitive {
        Primitive::Bool(flag) => Tag::Byte(i8::from(*flag)),
        Primitive::Byte(n) => Tag::Byte(*n),
        Primitive::Short(n) => Tag::Short(*n),
        Primitive::Int(n) => Tag::Int(*n),
        Primitive::Long(n) => Tag::Long(*n),
        Primitive::Float(n) => Tag::Float(*n),
        Primitive::Double(n) => Tag::Double(*n),
        Primitive::Str(text) => nbt_string(text),
    }
}

impl Component {
    /// `ComponentSerialization.CODEC.encodeStart(NbtOps, component)`.
    pub fn to_nbt(&self) -> Tag {
        if let Some(text) = self.collapse_to_string() {
            return nbt_string(text);
        }
        let mut entries: Vec<(&str, Tag)> = Vec::new();
        match &self.contents {
            Contents::Text(text) => entries.push(("text", nbt_string(text))),
            Contents::Translatable { key, fallback, args } => {
                entries.push(("translate", nbt_string(key)));
                if let Some(fallback) = fallback {
                    entries.push(("fallback", nbt_string(fallback)));
                }
                if !args.is_empty() {
                    let tags = args
                        .iter()
                        .map(|arg| match arg {
                            Arg::Primitive(primitive) => primitive_nbt(primitive),
                            Arg::Component(component) => component.to_nbt(),
                        })
                        .collect();
                    entries.push(("with", nbt_list(tags)));
                }
            }
            Contents::Keybind(name) => entries.push(("keybind", nbt_string(name))),
            Contents::Score { name, objective } => entries.push((
                "score",
                compound(vec![("name", nbt_string(name)), ("objective", nbt_string(objective))]),
            )),
            Contents::Selector { pattern, separator } => {
                entries.push(("selector", nbt_string(pattern)));
                if let Some(separator) = separator {
                    entries.push(("separator", separator.to_nbt()));
                }
            }
            Contents::Nbt { path, interpret, separator, source } => {
                entries.push(("nbt", nbt_string(path)));
                if *interpret {
                    entries.push(("interpret", Tag::Byte(1)));
                }
                if let Some(separator) = separator {
                    entries.push(("separator", separator.to_nbt()));
                }
                entries.push(match source {
                    DataSource::Entity(text) => ("entity", nbt_string(text)),
                    DataSource::Block(text) => ("block", nbt_string(text)),
                    DataSource::Storage(text) => ("storage", nbt_string(text)),
                });
            }
        }
        if !self.siblings.is_empty() {
            entries.push(("extra", nbt_list(self.siblings.iter().map(Self::to_nbt).collect())));
        }
        // Style members are put in reverse declaration order (verified against the original
        // for every key pair that shares a `HashMap` bucket; see docs/reference/components-text.md).
        let style = &self.style;
        if let Some(font) = &style.font {
            entries.push(("font", nbt_string(font)));
        }
        if let Some(insertion) = &style.insertion {
            entries.push(("insertion", nbt_string(insertion)));
        }
        if let Some(hover) = &style.hover_event {
            entries.push((
                "hoverEvent",
                compound(vec![("action", nbt_string("show_text")), ("contents", hover.text.to_nbt())]),
            ));
        }
        if let Some(click) = &style.click_event {
            entries.push((
                "clickEvent",
                compound(vec![("action", nbt_string(click.action.name())), ("value", nbt_string(&click.value))]),
            ));
        }
        for (key, flag) in [
            ("obfuscated", style.obfuscated),
            ("strikethrough", style.strikethrough),
            ("underlined", style.underlined),
            ("italic", style.italic),
            ("bold", style.bold),
        ] {
            if let Some(flag) = flag {
                entries.push((key, Tag::Byte(i8::from(flag))));
            }
        }
        if let Some(color) = &style.color {
            entries.push(("color", nbt_string(&color.serialize())));
        }
        compound(entries)
    }

    /// Network form (`FriendlyByteBuf.writeNbt` of the component): tag ID then payload.
    pub fn to_network_nbt(&self) -> Result<Vec<u8>> {
        self.to_nbt()
            .encode_network_anonymous()
            .map_err(|error| ComponentError::Codec(error.to_string()))
    }
}
