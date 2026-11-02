//! Brigadier's primitive `ArgumentType`s: bool, integer, long, float, double, string.
//!
//! Derived from Mojang's Brigadier (MIT, see `licenses/brigadier-MIT.txt`). Each type's
//! `parse` follows the original: it reads through the `StringReader`, and a range violation
//! resets the cursor to the start of the argument before raising the bound exception.
//! Minecraft's own argument types (coordinates, selectors, ...) are later stages.

use super::dispatcher::CommandContext;
use super::exception::{built_in, CommandArg, CommandSyntaxException};
use super::string_reader::StringReader;
use super::suggestion::{Suggestions, SuggestionsBuilder};

/// `com.mojang.brigadier.arguments.ArgumentType<T>`: `parse` and `listSuggestions`.
pub trait ArgumentType {
    type Value;
    fn parse(&self, reader: &mut StringReader) -> Result<Self::Value, CommandSyntaxException>;

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

macro_rules! numeric_argument {
    ($name:ident, $value:ty, $read:ident, $variant:ident, $low:literal, $high:literal) => {
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

numeric_argument!(IntegerArgumentType, i32, read_int, Integer, "argument.integer.low", "argument.integer.big");
numeric_argument!(LongArgumentType, i64, read_long, Long, "argument.long.low", "argument.long.big");
numeric_argument!(FloatArgumentType, f32, read_float, Float, "argument.float.low", "argument.float.big");
numeric_argument!(DoubleArgumentType, f64, read_double, Double, "argument.double.low", "argument.double.big");

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

argument_value_from!(bool => Bool, i32 => Integer, i64 => Long, f32 => Float, f64 => Double, String => String);

/// Object-safe `ArgumentType<?>` for command sources of type `S`: what a tree node holds so
/// that nodes of different argument types can live in one command tree.
pub trait ErasedArgumentType<S> {
    fn parse_value(&self, reader: &mut StringReader) -> Result<ArgumentValue, CommandSyntaxException>;

    fn list_suggestions(
        &self,
        context: &CommandContext<S>,
        builder: &mut SuggestionsBuilder,
    ) -> Result<Suggestions, CommandSyntaxException>;
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
}
