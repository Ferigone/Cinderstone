//! `ChatFormatting` and `NumberFormat` (blank, fixed, styled).

use crate::components::decode::{decode_component, parse_style, Val};
use crate::components::text::{
    hash_ordered_compound, nbt_string, reject, style_entries, Component, ComponentError, Result,
    Style,
};
use crate::nbt::Tag;
use crate::protocol::Writer;

/// `net.minecraft.ChatFormatting`, in declaration (ordinal) order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatFormatting {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
    Obfuscated,
    Bold,
    Strikethrough,
    Underline,
    Italic,
    Reset,
}

impl ChatFormatting {
    pub const ALL: [ChatFormatting; 22] = [
        Self::Black,
        Self::DarkBlue,
        Self::DarkGreen,
        Self::DarkAqua,
        Self::DarkRed,
        Self::DarkPurple,
        Self::Gold,
        Self::Gray,
        Self::DarkGray,
        Self::Blue,
        Self::Green,
        Self::Aqua,
        Self::Red,
        Self::LightPurple,
        Self::Yellow,
        Self::White,
        Self::Obfuscated,
        Self::Bold,
        Self::Strikethrough,
        Self::Underline,
        Self::Italic,
        Self::Reset,
    ];

    /// `Enum.ordinal()`: what `FriendlyByteBuf.writeEnum` writes.
    pub fn ordinal(self) -> i32 {
        Self::ALL.iter().position(|f| *f == self).expect("listed") as i32
    }

    /// `getName()`: the lower-case enum name.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; 22] = [
            "black",
            "dark_blue",
            "dark_green",
            "dark_aqua",
            "dark_red",
            "dark_purple",
            "gold",
            "gray",
            "dark_gray",
            "blue",
            "green",
            "aqua",
            "red",
            "light_purple",
            "yellow",
            "white",
            "obfuscated",
            "bold",
            "strikethrough",
            "underline",
            "italic",
            "reset",
        ];
        NAMES[self.ordinal() as usize]
    }

    /// `getId()`: the colour index 0–15, or -1 for formats and `RESET`.
    pub fn id(self) -> i32 {
        let ordinal = self.ordinal();
        if ordinal < 16 {
            ordinal
        } else {
            -1
        }
    }

    /// `getByName`: case-insensitive, ignoring every character outside `a-z` (`"Dark_Red"` is `dark_red`).
    pub fn by_name(name: &str) -> Option<Self> {
        let cleaned: String = name
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_lowercase())
            .collect();
        Self::ALL
            .iter()
            .copied()
            .find(|f| f.name().replace('_', "") == cleaned)
    }
}

/// `NumberFormat`.
#[derive(Debug, Clone, PartialEq)]
pub enum NumberFormat {
    Blank,
    Fixed(Component),
    Styled(Style),
}

impl NumberFormat {
    /// `NumberFormatTypes` registry id (`blank`, `styled`, `fixed` in registration order).
    pub fn type_id(&self) -> i32 {
        match self {
            Self::Blank => 0,
            Self::Styled(_) => 1,
            Self::Fixed(_) => 2,
        }
    }

    /// `format(int)`.
    pub fn format(&self, value: i32) -> Component {
        match self {
            Self::Blank => Component::literal(""),
            Self::Fixed(component) => component.clone(),
            Self::Styled(style) => {
                let mut component = Component::literal(value.to_string());
                component.style = style.clone();
                component
            }
        }
    }

    /// `NumberFormatTypes.CODEC.encodeStart(NbtOps, format)`.
    pub fn to_nbt(&self) -> Tag {
        let mut entries: Vec<(&str, Tag)> = Vec::new();
        match self {
            Self::Blank => entries.push(("type", nbt_string("minecraft:blank"))),
            Self::Fixed(component) => {
                entries.push(("type", nbt_string("minecraft:fixed")));
                entries.push(("value", component.to_nbt()));
            }
            Self::Styled(style) => {
                entries.push(("type", nbt_string("minecraft:styled")));
                entries.extend(style_entries(style));
            }
        }
        hash_ordered_compound(entries)
    }

    /// `NumberFormatTypes.CODEC.parse`. `Err` is a codec failure (the original treats it as absent).
    pub(crate) fn decode(value: &Val) -> Result<Self> {
        let kind = value
            .get("type")
            .and_then(Val::string)
            .ok_or_else(|| ComponentError::Codec("missing format type".to_owned()))?;
        let canonical = crate::components::java::resource_location(kind);
        match canonical.as_deref() {
            Some("minecraft:blank") => Ok(Self::Blank),
            Some("minecraft:fixed") => match value.get("value") {
                Some(inner) => Ok(Self::Fixed(decode_component(inner)?)),
                None => reject("missing required field value"),
            },
            Some("minecraft:styled") => Ok(Self::Styled(parse_style(value)?)),
            _ => reject(format!("Unknown registry key {kind}")),
        }
    }

    pub fn from_nbt(tag: &Tag) -> Result<Self> {
        Self::decode(&Val::from_nbt(tag)?)
    }

    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        Self::decode(&Val::from_json(value))
    }

    /// `NumberFormatTypes.STREAM_CODEC`: registry id, then the type's payload.
    pub fn write(&self, out: &mut Writer) -> Result<()> {
        out.varint(self.type_id());
        match self {
            Self::Blank => {}
            Self::Fixed(component) => {
                out.raw(&component.to_network_nbt()?);
            }
            Self::Styled(style) => {
                out.raw(&style_network_nbt(style)?);
            }
        }
        Ok(())
    }
}

/// `Style.Serializer.TRUSTED_STREAM_CODEC`: the style's members as one network compound.
fn style_network_nbt(style: &Style) -> Result<Vec<u8>> {
    let entries = style_entries(style);
    hash_ordered_compound(entries)
        .encode_network_anonymous()
        .map_err(|error| ComponentError::Codec(error.to_string()))
}

/// `ByteBufCodecs.optional(NUMBER_FORMAT)`.
pub fn write_optional_format(out: &mut Writer, format: Option<&NumberFormat>) -> Result<()> {
    out.bool(format.is_some());
    if let Some(format) = format {
        format.write(out)?;
    }
    Ok(())
}

/// `ComponentSerialization.TRUSTED_OPTIONAL_STREAM_CODEC`.
pub fn write_optional_component(out: &mut Writer, component: Option<&Component>) -> Result<()> {
    out.bool(component.is_some());
    if let Some(component) = component {
        out.raw(&component.to_network_nbt()?);
    }
    Ok(())
}
