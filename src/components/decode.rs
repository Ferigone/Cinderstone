//! Decoding of `ComponentSerialization.CODEC` from JSON (`JsonOps`) and from NBT (`NbtOps`).
//!
//! Both inputs are first lowered to [`Val`], a small tree that keeps what the two DFU
//! operations disagree on: a JSON number is not a boolean, an NBT numeric tag is
//! (`byteValue() != 0`); a JSON `null` member is absent; a list of compounds read through
//! `NbtOps` unwraps every `{"": element}` wrapper. Everything after that is one decoder.

use super::text::{
    reject, Arg, ClickAction, ClickEvent, Component, ComponentError, Contents, DataSource,
    HoverEvent, Primitive, Result, Style, TextColor,
};
use crate::nbt::Tag;
use serde_json::{Number, Value};

/// A decoded-input tree. Map members keep their order; lookups take the last duplicate.
#[derive(Debug, Clone)]
pub(crate) enum Val {
    Null,
    Bool(bool),
    /// A JSON number (type information is not known until it is narrowed).
    JsonNumber(Number),
    /// A typed NBT number.
    Number(Primitive),
    Str(String),
    List(Vec<Val>),
    Map(Vec<(String, Val)>),
}

impl Val {
    pub(crate) fn from_json(value: &Value) -> Val {
        match value {
            Value::Null => Val::Null,
            Value::Bool(flag) => Val::Bool(*flag),
            Value::Number(number) => Val::JsonNumber(number.clone()),
            Value::String(text) => Val::Str(text.clone()),
            Value::Array(items) => Val::List(items.iter().map(Val::from_json).collect()),
            Value::Object(map) => Val::Map(
                map.iter()
                    .map(|(key, value)| (key.clone(), Val::from_json(value)))
                    .collect(),
            ),
        }
    }

    /// `NbtOps` view of a tag. A list of compounds unwraps `{"": element}` wrappers, as
    /// `NbtOps.getStream` does; arrays are lists of numbers.
    pub(crate) fn from_nbt(tag: &Tag) -> Result<Val> {
        let text = |value: &crate::nbt::JavaString| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                ComponentError::Unsupported("a string with an unpaired UTF-16 surrogate".to_owned())
            })
        };
        Ok(match tag {
            Tag::Byte(n) => Val::Number(Primitive::Byte(*n)),
            Tag::Short(n) => Val::Number(Primitive::Short(*n)),
            Tag::Int(n) => Val::Number(Primitive::Int(*n)),
            Tag::Long(n) => Val::Number(Primitive::Long(*n)),
            Tag::Float(n) => Val::Number(Primitive::Float(*n)),
            Tag::Double(n) => Val::Number(Primitive::Double(*n)),
            Tag::String(value) => Val::Str(text(value)?),
            Tag::ByteArray(values) => Val::List(
                values
                    .iter()
                    .map(|n| Val::Number(Primitive::Byte(*n)))
                    .collect(),
            ),
            Tag::IntArray(values) => Val::List(
                values
                    .iter()
                    .map(|n| Val::Number(Primitive::Int(*n)))
                    .collect(),
            ),
            Tag::LongArray(values) => Val::List(
                values
                    .iter()
                    .map(|n| Val::Number(Primitive::Long(*n)))
                    .collect(),
            ),
            Tag::List {
                element_type,
                values,
            } => {
                let unwrap = *element_type == 10;
                Val::List(
                    values
                        .iter()
                        .map(|element| match element {
                            Tag::Compound(entries)
                                if unwrap && entries.len() == 1 && entries[0].0 == "" =>
                            {
                                Val::from_nbt(&entries[0].1)
                            }
                            other => Val::from_nbt(other),
                        })
                        .collect::<Result<_>>()?,
                )
            }
            Tag::Compound(entries) => Val::Map(
                entries
                    .iter()
                    .map(|(key, value)| Ok((text(key)?, Val::from_nbt(value)?)))
                    .collect::<Result<_>>()?,
            ),
        })
    }

    /// `MapLike.get`: the last member of that name; a JSON `null` counts as absent.
    pub(crate) fn get(&self, key: &str) -> Option<&Val> {
        match self {
            Val::Map(entries) => entries
                .iter()
                .rev()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
                .filter(|value| !matches!(value, Val::Null)),
            _ => None,
        }
    }

    pub(crate) fn string(&self) -> Option<&str> {
        match self {
            Val::Str(text) => Some(text),
            _ => None,
        }
    }

    /// `Codec.BOOL`: `JsonOps` accepts only a boolean ("Not a boolean: 1"); `NbtOps` goes
    /// through `getNumberValue` and `Number.byteValue() != 0`.
    pub(crate) fn boolean(&self) -> Option<bool> {
        match self {
            Val::Bool(flag) => Some(*flag),
            Val::Number(number) => Some(
                (match number {
                    Primitive::Bool(flag) => i8::from(*flag),
                    Primitive::Byte(n) => *n,
                    Primitive::Short(n) => *n as i8,
                    Primitive::Int(n) => *n as i8,
                    Primitive::Long(n) => *n as i8,
                    Primitive::Float(n) => (*n as i32) as i8,
                    Primitive::Double(n) => (*n as i32) as i8,
                    Primitive::Str(_) => return None,
                }) != 0,
            ),
            _ => None,
        }
    }

    /// `JavaOps.createMap` collects with `Collectors.toMap`, which throws on a null value,
    /// so an object holding a JSON `null` member anywhere makes the argument fail.
    fn holds_null_member(&self) -> bool {
        match self {
            Val::Map(entries) => entries
                .iter()
                .any(|(_, value)| matches!(value, Val::Null) || value.holds_null_member()),
            Val::List(items) => items.iter().any(Val::holds_null_member),
            _ => false,
        }
    }
}

fn required_string(map: &Val, key: &str) -> Result<String> {
    match map.get(key) {
        Some(value) => string_of(value, key),
        None => reject(format!("missing required field {key}")),
    }
}

fn string_of(value: &Val, what: &str) -> Result<String> {
    value
        .string()
        .map(str::to_owned)
        .ok_or_else(|| ComponentError::Codec(format!("{what}: expected a string")))
}

fn optional_bool(map: &Val, key: &str) -> Result<Option<bool>> {
    match map.get(key) {
        None => Ok(None),
        Some(value) => value
            .boolean()
            .map(Some)
            .ok_or_else(|| ComponentError::Codec(format!("{key}: expected a boolean"))),
    }
}

fn resource_location(text: &str) -> Result<String> {
    super::java::resource_location(text)
        .ok_or_else(|| ComponentError::Codec(format!("Not a valid resource location: {text}")))
}

/// `JsonOps.convertTo(JavaOps)` of a JSON number: `BigDecimal.longValueExact` narrowed to
/// byte, short, int or long; otherwise a float when `(float) d == d`, else a double.
/// `serde_json` keeps no decimal text, so an integer literal below `i64::MIN` arrives as
/// an `f64` and is indistinguishable from `-9223372036854775808.0` (read as a long).
fn number_primitive(number: &Number) -> Primitive {
    let exact_long = number.as_i64().or_else(|| {
        number.as_f64().and_then(|value| {
            (value.fract() == 0.0
                && value >= -9.223_372_036_854_775_808e18
                && value < 9.223_372_036_854_775_808e18)
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

/// `TranslatableContents.ARG_CODEC`: a primitive (`ExtraCodecs.JAVA` restricted to numbers,
/// booleans and strings) or else a component; a component that collapses to a plain string
/// is a string argument.
fn decode_arg(value: &Val) -> Result<Arg> {
    match value {
        Val::Str(text) => return Ok(Arg::Primitive(Primitive::Str(text.clone()))),
        Val::Bool(flag) => return Ok(Arg::Primitive(Primitive::Bool(*flag))),
        Val::JsonNumber(number) => return Ok(Arg::Primitive(number_primitive(number))),
        Val::Number(number) => return Ok(Arg::Primitive(number.clone())),
        _ => {}
    }
    if value.holds_null_member() {
        return reject("NullPointerException: JSON null in an object argument");
    }
    let component = decode_component(value)?;
    Ok(match component.collapse_to_string() {
        Some(text) => Arg::Primitive(Primitive::Str(text.to_owned())),
        None => Arg::Component(component),
    })
}

type ContentsParser = fn(&Val) -> Result<Contents>;

const CONTENT_TYPES: [(&str, ContentsParser); 6] = [
    ("text", parse_text),
    ("translatable", parse_translatable),
    ("keybind", parse_keybind),
    ("score", parse_score),
    ("selector", parse_selector),
    ("nbt", parse_nbt),
];

fn parse_text(map: &Val) -> Result<Contents> {
    Ok(Contents::Text(required_string(map, "text")?))
}

fn parse_translatable(map: &Val) -> Result<Contents> {
    let key = required_string(map, "translate")?;
    // `lenientOptionalFieldOf`: a wrong-typed fallback is ignored.
    let fallback = map
        .get("fallback")
        .and_then(|v| v.string().map(str::to_owned));
    let args = match map.get("with") {
        None => Vec::new(),
        Some(Val::List(items)) => items.iter().map(decode_arg).collect::<Result<Vec<_>>>()?,
        Some(_) => return reject("with: expected a list"),
    };
    Ok(Contents::Translatable {
        key,
        fallback,
        args,
    })
}

fn parse_keybind(map: &Val) -> Result<Contents> {
    Ok(Contents::Keybind(required_string(map, "keybind")?))
}

fn parse_score(map: &Val) -> Result<Contents> {
    let Some(inner @ Val::Map(_)) = map.get("score") else {
        return reject("score: expected an object");
    };
    Ok(Contents::Score {
        name: required_string(inner, "name")?,
        objective: required_string(inner, "objective")?,
    })
}

fn parse_selector(map: &Val) -> Result<Contents> {
    Ok(Contents::Selector {
        pattern: required_string(map, "selector")?,
        separator: match map.get("separator") {
            Some(value) => Some(Box::new(decode_component(value)?)),
            None => None,
        },
    })
}

fn parse_data_source(map: &Val) -> Result<DataSource> {
    let entity = |map: &Val| required_string(map, "entity").map(DataSource::Entity);
    let block = |map: &Val| required_string(map, "block").map(DataSource::Block);
    let storage = |map: &Val| {
        required_string(map, "storage")
            .and_then(|text| resource_location(&text))
            .map(DataSource::Storage)
    };
    if let Some(kind) = map.get("source") {
        return match kind.string() {
            Some("entity") => entity(map),
            Some("block") => block(map),
            Some("storage") => storage(map),
            _ => reject("source: unknown data source type"),
        };
    }
    entity(map)
        .or_else(|_| block(map))
        .or_else(|_| storage(map))
        .or_else(|_| reject("No matching codec found"))
}

fn parse_nbt(map: &Val) -> Result<Contents> {
    let path = required_string(map, "nbt")?;
    // `lenientOptionalFieldOf`: bad values fall back to the default / absent.
    let interpret = map.get("interpret").and_then(Val::boolean).unwrap_or(false);
    let separator = map
        .get("separator")
        .and_then(|value| decode_component(value).ok())
        .map(Box::new);
    Ok(Contents::Nbt {
        path,
        interpret,
        separator,
        source: parse_data_source(map)?,
    })
}

fn parse_contents(map: &Val) -> Result<Contents> {
    if let Some(kind) = map.get("type") {
        let name = kind
            .string()
            .ok_or_else(|| ComponentError::Codec("type: expected a string".to_owned()))?;
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

fn parse_click_event(value: &Val) -> Result<ClickEvent> {
    let Val::Map(_) = value else {
        return reject("clickEvent: expected an object");
    };
    let action = match required_string(value, "action")?.as_str() {
        "open_url" => ClickAction::OpenUrl,
        "run_command" => ClickAction::RunCommand,
        "suggest_command" => ClickAction::SuggestCommand,
        "change_page" => ClickAction::ChangePage,
        "copy_to_clipboard" => ClickAction::CopyToClipboard,
        "open_file" => return reject("Action not allowed: OPEN_FILE"),
        other => return reject(format!("Unknown click action {other}")),
    };
    Ok(ClickEvent {
        action,
        value: required_string(value, "value")?,
    })
}

fn parse_hover_event(value: &Val) -> Result<HoverEvent> {
    let Val::Map(_) = value else {
        return reject("hoverEvent: expected an object");
    };
    match required_string(value, "action")?.as_str() {
        "show_text" => {
            // `Codec.withAlternative(contents form, legacy value form)`: the legacy form
            // is tried only when the primary one fails.
            let primary = match value.get("contents") {
                Some(contents) => decode_component(contents),
                None => reject("missing required field contents"),
            };
            let text = match (primary, value.get("value")) {
                (Ok(text), _) => text,
                (Err(_), Some(legacy)) => decode_component(legacy)?,
                (Err(error), None) => return Err(error),
            };
            Ok(HoverEvent {
                text: Box::new(text),
            })
        }
        "show_item" | "show_entity" => Err(ComponentError::Unsupported(
            "hoverEvent show_item/show_entity".to_owned(),
        )),
        other => reject(format!("Unknown hover action {other}")),
    }
}

pub(crate) fn parse_style(map: &Val) -> Result<Style> {
    Ok(Style {
        color: match map.get("color") {
            Some(value) => Some(TextColor::parse(&string_of(value, "color")?)?),
            None => None,
        },
        bold: optional_bool(map, "bold")?,
        italic: optional_bool(map, "italic")?,
        underlined: optional_bool(map, "underlined")?,
        strikethrough: optional_bool(map, "strikethrough")?,
        obfuscated: optional_bool(map, "obfuscated")?,
        click_event: map.get("clickEvent").map(parse_click_event).transpose()?,
        hover_event: map.get("hoverEvent").map(parse_hover_event).transpose()?,
        insertion: map
            .get("insertion")
            .map(|v| string_of(v, "insertion"))
            .transpose()?,
        font: match map.get("font") {
            Some(value) => Some(resource_location(&string_of(value, "font")?)?),
            None => None,
        },
    })
}

pub(crate) fn decode_component(value: &Val) -> Result<Component> {
    match value {
        Val::Str(text) => Ok(Component::literal(text.clone())),
        Val::List(items) => {
            if items.is_empty() {
                return reject("List must have contents");
            }
            let mut parts = items
                .iter()
                .map(decode_component)
                .collect::<Result<Vec<_>>>()?
                .into_iter();
            let mut first = parts.next().expect("non-empty");
            first.siblings.extend(parts);
            Ok(first)
        }
        Val::Map(_) => {
            let contents = parse_contents(value)?;
            let siblings = match value.get("extra") {
                None => Vec::new(),
                Some(Val::List(items)) if !items.is_empty() => items
                    .iter()
                    .map(decode_component)
                    .collect::<Result<Vec<_>>>()?,
                Some(_) => return reject("extra: expected a non-empty list"),
            };
            Ok(Component {
                contents,
                siblings,
                style: parse_style(value)?,
            })
        }
        _ => reject("expected a string, list or object"),
    }
}

impl Component {
    /// `ComponentSerialization.CODEC.parse(JsonOps, value)`.
    pub fn from_json(value: &Value) -> Result<Self> {
        decode_component(&Val::from_json(value))
    }

    /// `ComponentSerialization.CODEC.parse(NbtOps, tag)`: the form carried by item
    /// components, block entities and packets.
    pub fn from_nbt(tag: &Tag) -> Result<Self> {
        decode_component(&Val::from_nbt(tag)?)
    }
}
