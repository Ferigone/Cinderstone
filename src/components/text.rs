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
//! Native player `show_entity` hovers support scoreboard display-name persistence.
//! `show_item`, non-player and legacy entity hovers remain explicit Unsupported contexts.

use super::java::hashmap_iteration_order;
use crate::nbt::{JavaString, Tag};
use serde_json::{Number, Value};

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

pub(crate) type Result<T> = std::result::Result<T, ComponentError>;

pub(crate) fn reject<T>(what: impl Into<String>) -> Result<T> {
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
                Some(value) if (0..=0xFF_FFFF).contains(&value) => Ok(Self {
                    value: value as u32,
                    name: None,
                }),
                Some(_) => reject(format!("Color value out of range: {text}")),
                None => reject(format!("Invalid color value: {text}")),
            }
        } else {
            NAMED_COLORS
                .iter()
                .find(|(name, _)| *name == text)
                .map(|(name, value)| Self {
                    value: *value,
                    name: Some(name),
                })
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

/// Hover forms required by text and the live player's display name.
#[derive(Debug, Clone, PartialEq)]
pub enum HoverEvent {
    Text(Box<Component>),
    PlayerEntity {
        uuid: [u8; 16],
        name: Option<Box<Component>>,
    },
}
impl HoverEvent {
    fn action(&self) -> &'static str {
        match self {
            Self::Text(_) => "show_text",
            Self::PlayerEntity { .. } => "show_entity",
        }
    }
    fn contents_json(&self) -> Json {
        match self {
            Self::Text(text) => text.json(),
            Self::PlayerEntity { uuid, name } => {
                let mut fields = Vec::new();
                fields.push((
                    "id",
                    Json::Arr(
                        uuid.chunks_exact(4)
                            .map(|b| {
                                Json::Num(i32::from_be_bytes(b.try_into().unwrap()).to_string())
                            })
                            .collect(),
                    ),
                ));
                fields.insert(0, ("type", Json::Str("minecraft:player".into())));
                if let Some(name) = name {
                    fields.push(("name", name.json()));
                }
                Json::Obj(fields)
            }
        }
    }
    fn contents_nbt(&self) -> Tag {
        match self {
            Self::Text(text) => text.to_nbt(),
            Self::PlayerEntity { uuid, name } => {
                let mut fields = Vec::new();
                if let Some(name) = name {
                    fields.push(("name", name.to_nbt()));
                }
                fields.push((
                    "id",
                    Tag::IntArray(
                        uuid.chunks_exact(4)
                            .map(|b| i32::from_be_bytes(b.try_into().unwrap()))
                            .collect(),
                    ),
                ));
                fields.push(("type", nbt_string("minecraft:player")));
                compound(fields)
            }
        }
    }
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
    Translatable {
        key: String,
        fallback: Option<String>,
        args: Vec<Arg>,
    },
    Keybind(String),
    Score {
        name: String,
        objective: String,
    },
    Selector {
        pattern: String,
        separator: Option<Box<Component>>,
    },
    Nbt {
        path: String,
        interpret: bool,
        separator: Option<Box<Component>>,
        source: DataSource,
    },
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
        Self {
            contents: Contents::Text(text.into()),
            siblings: Vec::new(),
            style: Style::default(),
        }
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

/// A JSON document that keeps member order: the order the original's record builders insert
/// members in, which `Gson` then prints (`Component.Serializer.toJson`).
#[derive(Debug, Clone)]
enum Json {
    Str(String),
    Bool(bool),
    /// A number already formatted the way `Number.toString` prints it.
    Num(String),
    Arr(Vec<Json>),
    Obj(Vec<(&'static str, Json)>),
}

impl Json {
    fn write(&self, out: &mut String) {
        match self {
            Json::Str(text) => write_json_string(text, out),
            Json::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
            Json::Num(text) => out.push_str(text),
            Json::Arr(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Obj(members) => {
                out.push('{');
                for (index, (key, value)) in members.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_json_string(key, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }

    /// The same document as an unordered `serde_json::Value`.
    fn to_value(&self) -> Value {
        match self {
            Json::Str(text) => Value::String(text.clone()),
            Json::Bool(flag) => Value::Bool(*flag),
            Json::Num(text) => serde_json::from_str(text).unwrap_or_else(|_| {
                // Java prints `1.0E10`, `NaN`, `Infinity`: not JSON for `serde_json`.
                text.parse::<f64>()
                    .ok()
                    .and_then(Number::from_f64)
                    .map_or(Value::Null, Value::Number)
            }),
            Json::Arr(items) => Value::Array(items.iter().map(Json::to_value).collect()),
            Json::Obj(members) => Value::Object(
                members
                    .iter()
                    .map(|(key, value)| ((*key).to_owned(), value.to_value()))
                    .collect(),
            ),
        }
    }
}

/// `Gson` with HTML escaping disabled: quotes, backslash, the C0 controls (short escapes for
/// `\b \t \n \f \r`) and U+2028/U+2029 are escaped; everything else is written as is.
fn write_json_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn contents_json(contents: &Contents, out: &mut Vec<(&'static str, Json)>) {
    match contents {
        Contents::Text(text) => out.push(("text", Json::Str(text.clone()))),
        Contents::Translatable {
            key,
            fallback,
            args,
        } => {
            out.push(("translate", Json::Str(key.clone())));
            if let Some(fallback) = fallback {
                out.push(("fallback", Json::Str(fallback.clone())));
            }
            if !args.is_empty() {
                out.push(("with", Json::Arr(args.iter().map(arg_json).collect())));
            }
        }
        Contents::Keybind(name) => out.push(("keybind", Json::Str(name.clone()))),
        Contents::Score { name, objective } => out.push((
            "score",
            Json::Obj(vec![
                ("name", Json::Str(name.clone())),
                ("objective", Json::Str(objective.clone())),
            ]),
        )),
        Contents::Selector { pattern, separator } => {
            out.push(("selector", Json::Str(pattern.clone())));
            if let Some(separator) = separator {
                out.push(("separator", separator.json()));
            }
        }
        Contents::Nbt {
            path,
            interpret,
            separator,
            source,
        } => {
            out.push(("nbt", Json::Str(path.clone())));
            if *interpret {
                out.push(("interpret", Json::Bool(true)));
            }
            if let Some(separator) = separator {
                out.push(("separator", separator.json()));
            }
            out.push(match source {
                DataSource::Entity(text) => ("entity", Json::Str(text.clone())),
                DataSource::Block(text) => ("block", Json::Str(text.clone())),
                DataSource::Storage(text) => ("storage", Json::Str(text.clone())),
            });
        }
    }
}

fn primitive_json(primitive: &Primitive) -> Json {
    match primitive {
        Primitive::Bool(flag) => Json::Bool(*flag),
        Primitive::Byte(n) => Json::Num(n.to_string()),
        Primitive::Short(n) => Json::Num(n.to_string()),
        Primitive::Int(n) => Json::Num(n.to_string()),
        Primitive::Long(n) => Json::Num(n.to_string()),
        Primitive::Float(n) => Json::Num(super::java::float_to_string(*n)),
        Primitive::Double(n) => Json::Num(super::java::double_to_string(*n)),
        Primitive::Str(text) => Json::Str(text.clone()),
    }
}

fn arg_json(arg: &Arg) -> Json {
    match arg {
        Arg::Primitive(primitive) => primitive_json(primitive),
        Arg::Component(component) => component.json(),
    }
}

impl Component {
    fn json(&self) -> Json {
        if let Some(text) = self.collapse_to_string() {
            return Json::Str(text.to_owned());
        }
        let mut out = Vec::new();
        contents_json(&self.contents, &mut out);
        if !self.siblings.is_empty() {
            out.push((
                "extra",
                Json::Arr(self.siblings.iter().map(Self::json).collect()),
            ));
        }
        // Style members, in the order the original's codec writes them.
        let style = &self.style;
        if let Some(hover) = &style.hover_event {
            out.push((
                "hoverEvent",
                Json::Obj(vec![
                    ("contents", hover.contents_json()),
                    ("action", Json::Str(hover.action().to_owned())),
                ]),
            ));
        }
        if let Some(insertion) = &style.insertion {
            out.push(("insertion", Json::Str(insertion.clone())));
        }
        if let Some(font) = &style.font {
            out.push(("font", Json::Str(font.clone())));
        }
        if let Some(flag) = style.obfuscated {
            out.push(("obfuscated", Json::Bool(flag)));
        }
        if let Some(click) = &style.click_event {
            out.push((
                "clickEvent",
                Json::Obj(vec![
                    ("action", Json::Str(click.action.name().to_owned())),
                    ("value", Json::Str(click.value.clone())),
                ]),
            ));
        }
        for (key, flag) in [
            ("italic", style.italic),
            ("underlined", style.underlined),
            ("strikethrough", style.strikethrough),
        ] {
            if let Some(flag) = flag {
                out.push((key, Json::Bool(flag)));
            }
        }
        if let Some(color) = &style.color {
            out.push(("color", Json::Str(color.serialize())));
        }
        if let Some(flag) = style.bold {
            out.push(("bold", Json::Bool(flag)));
        }
        Json::Obj(out)
    }

    /// `ComponentSerialization.CODEC` encoding to JSON (a plain component is a bare string).
    /// Member order is not preserved; use [`Self::to_json_string`] for the text form.
    pub fn to_json(&self) -> Value {
        self.json().to_value()
    }

    /// `Component.Serializer.toJson`: compact `Gson` text with the original's member order.
    pub fn to_json_string(&self) -> String {
        let mut out = String::new();
        self.json().write(&mut out);
        out
    }
}

/// A style's members as NBT entries, in the order the original's codec inserts them (the order
/// only matters where keys share a `HashMap` bucket; measured for every such pair).
pub(crate) fn style_entries(style: &Style) -> Vec<(&'static str, Tag)> {
    let mut out: Vec<(&'static str, Tag)> = Vec::new();
    if let Some(hover) = &style.hover_event {
        out.push((
            "hoverEvent",
            compound(vec![
                ("contents", hover.contents_nbt()),
                ("action", nbt_string(hover.action())),
            ]),
        ));
    }
    if let Some(insertion) = &style.insertion {
        out.push(("insertion", nbt_string(insertion)));
    }
    if let Some(font) = &style.font {
        out.push(("font", nbt_string(font)));
    }
    if let Some(flag) = style.obfuscated {
        out.push(("obfuscated", Tag::Byte(i8::from(flag))));
    }
    if let Some(click) = &style.click_event {
        out.push((
            "clickEvent",
            compound(vec![
                ("action", nbt_string(click.action.name())),
                ("value", nbt_string(&click.value)),
            ]),
        ));
    }
    for (key, flag) in [
        ("italic", style.italic),
        ("underlined", style.underlined),
        ("strikethrough", style.strikethrough),
    ] {
        if let Some(flag) = flag {
            out.push((key, Tag::Byte(i8::from(flag))));
        }
    }
    if let Some(color) = &style.color {
        out.push(("color", nbt_string(&color.serialize())));
    }
    if let Some(flag) = style.bold {
        out.push(("bold", Tag::Byte(i8::from(flag))));
    }
    out
}

/// A `CompoundTag` whose entries are stored in `java.util.HashMap` order.
pub(crate) fn hash_ordered_compound(entries: Vec<(&str, Tag)>) -> Tag {
    compound(entries)
}

pub(crate) fn nbt_string(text: &str) -> Tag {
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
                other => Self::Heterogeneous(
                    values
                        .into_iter()
                        .map(|v| wrap_element(Tag::Byte(v)))
                        .collect(),
                )
                .accept(other),
            },
            Self::Ints(mut values) => match tag {
                Tag::Int(value) => {
                    values.push(value);
                    Self::Ints(values)
                }
                other => Self::Heterogeneous(
                    values
                        .into_iter()
                        .map(|v| wrap_element(Tag::Int(v)))
                        .collect(),
                )
                .accept(other),
            },
            Self::Longs(mut values) => match tag {
                Tag::Long(value) => {
                    values.push(value);
                    Self::Longs(values)
                }
                other => Self::Heterogeneous(
                    values
                        .into_iter()
                        .map(|v| wrap_element(Tag::Long(v)))
                        .collect(),
                )
                .accept(other),
            },
            Self::Homogeneous(mut values) => {
                if tag.tag_id() == values[0].tag_id() {
                    values.push(tag);
                    Self::Homogeneous(values)
                } else {
                    Self::Heterogeneous(values.into_iter().map(wrap_if_needed).collect())
                        .accept(tag)
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
            Self::Initial => Tag::List {
                element_type: 0,
                values: Vec::new(),
            },
            Self::Bytes(values) => Tag::ByteArray(values),
            Self::Ints(values) => Tag::IntArray(values),
            Self::Longs(values) => Tag::LongArray(values),
            Self::Homogeneous(values) => Tag::List {
                element_type: values[0].tag_id(),
                values,
            },
            Self::Heterogeneous(values) => Tag::List {
                element_type: 10,
                values,
            },
        }
    }
}

fn nbt_list(items: Vec<Tag>) -> Tag {
    items
        .into_iter()
        .fold(ListCollector::Initial, ListCollector::accept)
        .result()
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
            Contents::Translatable {
                key,
                fallback,
                args,
            } => {
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
                compound(vec![
                    ("name", nbt_string(name)),
                    ("objective", nbt_string(objective)),
                ]),
            )),
            Contents::Selector { pattern, separator } => {
                entries.push(("selector", nbt_string(pattern)));
                if let Some(separator) = separator {
                    entries.push(("separator", separator.to_nbt()));
                }
            }
            Contents::Nbt {
                path,
                interpret,
                separator,
                source,
            } => {
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
            entries.push((
                "extra",
                nbt_list(self.siblings.iter().map(Self::to_nbt).collect()),
            ));
        }
        entries.extend(style_entries(&self.style));
        compound(entries)
    }

    /// Network form (`FriendlyByteBuf.writeNbt` of the component): tag ID then payload.
    pub fn to_network_nbt(&self) -> Result<Vec<u8>> {
        self.to_nbt()
            .encode_network_anonymous()
            .map_err(|error| ComponentError::Codec(error.to_string()))
    }
}
