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

pub(super) type Result<T> = std::result::Result<T, ComponentError>;

pub(super) fn reject<T>(what: impl Into<String>) -> Result<T> {
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

impl Component {
    pub fn literal(text: impl Into<String>) -> Self {
        Self { contents: Contents::Text(text.into()), siblings: Vec::new(), style: Style::default() }
    }

    /// `Component.tryCollapseToString`: a plain, unstyled, childless text is a bare string.
    pub fn collapse_to_string(&self) -> Option<&str> {
        match &self.contents {
            Contents::Text(text) if self.siblings.is_empty() && self.style.is_empty() => Some(text),
            _ => None,
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
