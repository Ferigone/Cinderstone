//! Brigadier's primitive `ArgumentType`s: bool, integer, long, float, double, string.
//!
//! Derived from Mojang's Brigadier (MIT, see `licenses/brigadier-MIT.txt`). Each type's
//! `parse` follows the original: it reads through the `StringReader`, and a range violation
//! resets the cursor to the start of the argument before raising the bound exception.
//! Minecraft's own argument types (coordinates, selectors, ...) are later stages.

use super::coordinates::Coordinates;
use super::dispatcher::CommandContext;
use super::exception::{built_in, CommandArg, CommandSyntaxException};
use super::string_reader::StringReader;
use super::suggestion::{Suggestions, SuggestionsBuilder};

/// How an argument type appears in `ClientboundCommandsPacket`
/// (`ArgumentTypeInfos` registry id plus the serialized template).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentNetworkInfo {
    pub registry_id: i32,
    pub template: Vec<u8>,
}

/// `ArgumentTypeInfos` registry ids of the Brigadier types (1.21 registration order).
pub mod registry_ids {
    pub const BOOL: i32 = 0;
    pub const FLOAT: i32 = 1;
    pub const DOUBLE: i32 = 2;
    pub const INTEGER: i32 = 3;
    pub const LONG: i32 = 4;
    pub const STRING: i32 = 5;
}

/// `com.mojang.brigadier.arguments.ArgumentType<T>`: `parse` and `listSuggestions`, plus the
/// pieces of `ArgumentTypeInfo` and `equals` the command-tree packet needs.
pub trait ArgumentType {
    type Value;
    fn parse(&self, reader: &mut StringReader) -> Result<Self::Value, CommandSyntaxException>;

    /// `ArgumentTypeInfos.unpack(type)` serialized with `serializeToNetwork`; `None` for a type
    /// that has no network form yet (the original throws for an unregistered type).
    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        None
    }

    /// A key equal exactly when the original's `equals` says two instances are equal. `None`
    /// means `Object.equals`: only the very same instance is equal to itself.
    fn equality_key(&self) -> Option<String> {
        None
    }

    /// `listSuggestions(context, builder)`: the original's default offers nothing.
    fn list_suggestions<S>(
        &self,
        _context: &CommandContext<S>,
        _builder: &mut SuggestionsBuilder,
    ) -> Result<Suggestions, CommandSyntaxException> {
        Ok(Suggestions::empty())
    }
}

/// `BoolArgumentType.bool()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct BoolArgumentType;

impl ArgumentType for BoolArgumentType {
    type Value = bool;
    fn parse(&self, reader: &mut StringReader) -> Result<bool, CommandSyntaxException> {
        reader.read_boolean()
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        Some(ArgumentNetworkInfo {
            registry_id: registry_ids::BOOL,
            template: Vec::new(),
        })
    }

    /// Offers `true` and `false` (in that order) when they start with the typed text.
    fn list_suggestions<S>(
        &self,
        _context: &CommandContext<S>,
        builder: &mut SuggestionsBuilder,
    ) -> Result<Suggestions, CommandSyntaxException> {
        for word in ["true", "false"] {
            if word.encode_utf16().collect::<Vec<_>>().starts_with(builder.remaining_lower_case_units()) {
                builder.suggest(word);
            }
        }
        Ok(builder.build())
    }
}

/// `ArgumentUtils.createNumberFlags(hasMin, hasMax)`.
fn number_flags(has_minimum: bool, has_maximum: bool) -> u8 {
    u8::from(has_minimum) | (u8::from(has_maximum) << 1)
}

macro_rules! numeric_argument {
    ($name:ident, $value:ty, $read:ident, $variant:ident, $low:literal, $high:literal, $id:expr, $default_min:expr, $default_max:expr, $key:literal) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $name {
            pub minimum: $value,
            pub maximum: $value,
        }

        impl $name {
            pub fn new(minimum: $value, maximum: $value) -> Self {
                Self { minimum, maximum }
            }
        }

        impl ArgumentType for $name {
            type Value = $value;

            /// `<Type>ArgumentInfo.serializeToNetwork`: a flag byte for the non-default bounds,
            /// then the bounds that are set, big-endian.
            fn network_info(&self) -> Option<ArgumentNetworkInfo> {
                let has_minimum = self.minimum != $default_min;
                let has_maximum = self.maximum != $default_max;
                let mut template = vec![number_flags(has_minimum, has_maximum)];
                if has_minimum {
                    template.extend_from_slice(&self.minimum.to_be_bytes());
                }
                if has_maximum {
                    template.extend_from_slice(&self.maximum.to_be_bytes());
                }
                Some(ArgumentNetworkInfo {
                    registry_id: $id,
                    template,
                })
            }

            /// `equals` compares the bounds with `==` (so `0.0 == -0.0` and NaN never matches).
            fn equality_key(&self) -> Option<String> {
                if self.minimum != self.minimum || self.maximum != self.maximum {
                    return None;
                }
                let normalized = |value: $value| if value == (0 as $value) { (0 as $value).to_be_bytes() } else { value.to_be_bytes() };
                Some(format!("{}:{:?}:{:?}", $key, normalized(self.minimum), normalized(self.maximum)))
            }

            fn parse(&self, reader: &mut StringReader) -> Result<$value, CommandSyntaxException> {
                let start = reader.cursor();
                let result = reader.$read()?;
                if result < self.minimum {
                    reader.set_cursor(start);
                    return Err(built_in::range(
                        $low,
                        CommandArg::$variant(self.minimum),
                        CommandArg::$variant(result),
                        reader.units(),
                        reader.cursor(),
                    ));
                }
                if result > self.maximum {
                    reader.set_cursor(start);
                    return Err(built_in::range(
                        $high,
                        CommandArg::$variant(self.maximum),
                        CommandArg::$variant(result),
                        reader.units(),
                        reader.cursor(),
                    ));
                }
                Ok(result)
            }
        }
    };
}

numeric_argument!(
    IntegerArgumentType, i32, read_int, Integer, "argument.integer.low", "argument.integer.big",
    registry_ids::INTEGER, i32::MIN, i32::MAX, "integer"
);
numeric_argument!(
    LongArgumentType, i64, read_long, Long, "argument.long.low", "argument.long.big",
    registry_ids::LONG, i64::MIN, i64::MAX, "long"
);
numeric_argument!(
    FloatArgumentType, f32, read_float, Float, "argument.float.low", "argument.float.big",
    registry_ids::FLOAT, -f32::MAX, f32::MAX, "float"
);
numeric_argument!(
    DoubleArgumentType, f64, read_double, Double, "argument.double.low", "argument.double.big",
    registry_ids::DOUBLE, -f64::MAX, f64::MAX, "double"
);

/// `StringArgumentType.StringType`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StringType {
    /// `word()`: one unquoted word.
    SingleWord,
    /// `string()`: a quoted phrase or a single word.
    QuotablePhrase,
    /// `greedyString()`: the rest of the input.
    GreedyPhrase,
}

#[derive(Clone, Copy, Debug)]
pub struct StringArgumentType {
    pub kind: StringType,
}

impl StringArgumentType {
    pub fn word() -> Self {
        Self { kind: StringType::SingleWord }
    }

    pub fn string() -> Self {
        Self { kind: StringType::QuotablePhrase }
    }

    pub fn greedy_string() -> Self {
        Self { kind: StringType::GreedyPhrase }
    }

    /// `StringArgumentType.escapeIfRequired`.
    pub fn escape_if_required(input: &str) -> String {
        if input.encode_utf16().all(StringReader::is_allowed_in_unquoted_string) {
            return input.to_string();
        }
        let mut result = String::from("\"");
        for ch in input.chars() {
            if ch == '\\' || ch == '"' {
                result.push('\\');
            }
            result.push(ch);
        }
        result.push('"');
        result
    }
}

impl ArgumentType for StringArgumentType {
    type Value = String;

    /// `StringArgumentSerializer`: the `StringType` ordinal as a VarInt.
    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        let ordinal = match self.kind {
            StringType::SingleWord => 0,
            StringType::QuotablePhrase => 1,
            StringType::GreedyPhrase => 2,
        };
        Some(ArgumentNetworkInfo {
            registry_id: registry_ids::STRING,
            template: vec![ordinal],
        })
    }

    // StringArgumentType (like BoolArgumentType) keeps Object.equals: no equality key.

    fn parse(&self, reader: &mut StringReader) -> Result<String, CommandSyntaxException> {
        match self.kind {
            StringType::GreedyPhrase => {
                let text = reader.remaining();
                reader.set_cursor(reader.total_length());
                Ok(text)
            }
            StringType::SingleWord => Ok(reader.read_unquoted_string()),
            StringType::QuotablePhrase => reader.read_string(),
        }
    }
}

/// A parsed argument value, the Rust form of the original's `Object` result.
#[derive(Clone, Debug, PartialEq)]
pub enum ArgumentValue {
    Bool(bool),
    Integer(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    /// `Coordinates` of a position argument (stage 6).
    Coordinates(Coordinates),
}

macro_rules! argument_value_from {
    ($($value:ty => $variant:ident),* $(,)?) => {
        $(impl From<$value> for ArgumentValue {
            fn from(value: $value) -> Self {
                Self::$variant(value)
            }
        })*
    };
}

argument_value_from!(bool => Bool, i32 => Integer, i64 => Long, f32 => Float, f64 => Double, String => String, Coordinates => Coordinates);

/// Object-safe `ArgumentType<?>` for command sources of type `S`: what a tree node holds so
/// that nodes of different argument types can live in one command tree.
pub trait ErasedArgumentType<S> {
    fn parse_value(&self, reader: &mut StringReader) -> Result<ArgumentValue, CommandSyntaxException>;

    fn list_suggestions(
        &self,
        context: &CommandContext<S>,
        builder: &mut SuggestionsBuilder,
    ) -> Result<Suggestions, CommandSyntaxException>;

    fn network_info(&self) -> Option<ArgumentNetworkInfo>;

    fn equality_key(&self) -> Option<String>;
}

impl<S, T> ErasedArgumentType<S> for T
where
    T: ArgumentType,
    T::Value: Into<ArgumentValue>,
{
    fn parse_value(&self, reader: &mut StringReader) -> Result<ArgumentValue, CommandSyntaxException> {
        self.parse(reader).map(Into::into)
    }

    fn list_suggestions(
        &self,
        context: &CommandContext<S>,
        builder: &mut SuggestionsBuilder,
    ) -> Result<Suggestions, CommandSyntaxException> {
        ArgumentType::list_suggestions(self, context, builder)
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        ArgumentType::network_info(self)
    }

    fn equality_key(&self) -> Option<String> {
        ArgumentType::equality_key(self)
    }
}
