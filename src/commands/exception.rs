//! `CommandSyntaxException` with the dedicated server's built-in exception provider.
//!
//! Brigadier ships English literals, but the vanilla server installs
//! `net.minecraft.commands.BrigadierExceptions`, whose messages are translatable
//! components. The key and the typed arguments are therefore the identity of a message;
//! `english()` renders the `en_us` text only for logs and for differential tests.
//!
//! Derived from Mojang's Brigadier (MIT, see `licenses/brigadier-MIT.txt`).

use super::java_text::{java_double_to_string, java_float_to_string};

/// One translatable-component argument, typed as the original's `Object` argument.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandArg {
    String(String),
    Integer(i32),
    Long(i64),
    Float(f32),
    Double(f64),
}

impl CommandArg {
    /// Java `getClass().getSimpleName()` of the boxed argument.
    pub fn java_type(&self) -> &'static str {
        match self {
            Self::String(_) => "String",
            Self::Integer(_) => "Integer",
            Self::Long(_) => "Long",
            Self::Float(_) => "Float",
            Self::Double(_) => "Double",
        }
    }

    /// `String.valueOf(argument)`, which `Component.getString()` uses for `%s`.
    pub fn java_to_string(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Integer(value) => value.to_string(),
            Self::Long(value) => value.to_string(),
            Self::Float(value) => java_float_to_string(*value),
            Self::Double(value) => java_double_to_string(*value),
        }
    }
}

/// A translatable message: `Component.translatable(key, args...)`.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandMessage {
    pub key: &'static str,
    pub args: Vec<CommandArg>,
}

impl CommandMessage {
    pub fn new(key: &'static str, args: Vec<CommandArg>) -> Self {
        Self { key, args }
    }

    /// The `en_us` rendering (`Component.getString()` with the default language).
    pub fn english(&self) -> String {
        let template = english_template(self.key);
        let mut out = String::new();
        let mut args = self.args.iter();
        let mut chars = template.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '%' {
                match chars.peek() {
                    Some('s') => {
                        chars.next();
                        if let Some(arg) = args.next() {
                            out.push_str(&arg.java_to_string());
                        }
                    }
                    Some('%') => {
                        chars.next();
                        out.push('%');
                    }
                    _ => out.push('%'),
                }
            } else {
                out.push(ch);
            }
        }
        out
    }
}

/// `en_us.json` templates for the built-in keys (pinned 1.21 client/server resources).
fn english_template(key: &str) -> &'static str {
    match key {
        "command.exception" => "Could not parse command: %s",
        "parsing.expected" => "Expected '%s'",
        "parsing.float.invalid" => "Invalid float '%s'",
        "parsing.double.invalid" => "Invalid double '%s'",
        "parsing.long.invalid" => "Invalid long '%s'",
        "parsing.int.invalid" => "Invalid integer '%s'",
        "parsing.bool.invalid" => "Invalid boolean, expected 'true' or 'false' but found '%s'",
        "parsing.quote.escape" => "Invalid escape sequence '\\%s' in quoted string",
        "argument.literal.incorrect" => "Expected literal %s",
        "argument.long.big" => "Long must not be more than %s, found %s",
        "argument.long.low" => "Long must not be less than %s, found %s",
        "argument.integer.big" => "Integer must not be more than %s, found %s",
        "argument.integer.low" => "Integer must not be less than %s, found %s",
        "argument.float.big" => "Float must not be more than %s, found %s",
        "argument.float.low" => "Float must not be less than %s, found %s",
        "argument.double.big" => "Double must not be more than %s, found %s",
        "argument.double.low" => "Double must not be less than %s, found %s",
        "parsing.quote.expected.start" => "Expected quote to start a string",
        "parsing.quote.expected.end" => "Unclosed quoted string",
        "parsing.int.expected" => "Expected integer",
        "parsing.long.expected" => "Expected long",
        "parsing.double.expected" => "Expected double",
        "parsing.float.expected" => "Expected float",
        "parsing.bool.expected" => "Expected boolean",
        "argument.pos.missing.double" => "Expected a coordinate",
        "argument.pos.missing.int" => "Expected a block position",
        "argument.pos.mixed" => "Cannot mix world & local coordinates (everything must either use ^ or not)",
        "argument.pos3d.incomplete" => "Incomplete (expected 3 coordinates)",
        "argument.pos2d.incomplete" => "Incomplete (expected 2 coordinates)",
        "command.unknown.command" => "Unknown or incomplete command, see below for error",
        "command.unknown.argument" => "Incorrect argument for command",
        "command.expected.separator" => {
            "Expected whitespace to end one argument, but found trailing data"
        }
        _ => "",
    }
}

const CONTEXT_AMOUNT: usize = 10;

/// `com.mojang.brigadier.exceptions.CommandSyntaxException`.
///
/// The input is kept as UTF-16 code units because Brigadier's cursor is a Java `String`
/// index. `cursor` is `-1` when the exception was created without context.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandSyntaxException {
    message: CommandMessage,
    input: Option<Vec<u16>>,
    cursor: i32,
}

impl CommandSyntaxException {
    /// `SimpleCommandExceptionType.create()` / `DynamicCommandExceptionType.create(..)`.
    pub fn without_context(message: CommandMessage) -> Self {
        Self {
            message,
            input: None,
            cursor: -1,
        }
    }

    /// `*.createWithContext(reader, ..)`: remembers the reader's string and cursor.
    pub fn with_context(message: CommandMessage, input: &[u16], cursor: usize) -> Self {
        Self {
            message,
            input: Some(input.to_vec()),
            cursor: cursor as i32,
        }
    }

    pub fn raw_message(&self) -> &CommandMessage {
        &self.message
    }

    pub fn cursor(&self) -> i32 {
        self.cursor
    }

    pub fn input(&self) -> Option<String> {
        self.input.as_deref().map(utf16_to_string)
    }

    /// `getContext()`: up to ten code units before the cursor, then `<--[HERE]`.
    ///
    /// A cut through a surrogate pair yields a lone surrogate in Java; a Rust `String`
    /// cannot hold one, so such a unit becomes U+FFFD here.
    pub fn context(&self) -> Option<String> {
        let input = self.input.as_deref()?;
        if self.cursor < 0 {
            return None;
        }
        let cursor = input.len().min(self.cursor as usize);
        let mut out = String::new();
        if cursor > CONTEXT_AMOUNT {
            out.push_str("...");
        }
        out.push_str(&utf16_to_string(&input[cursor.saturating_sub(CONTEXT_AMOUNT)..cursor]));
        out.push_str("<--[HERE]");
        Some(out)
    }

    /// `getMessage()`: the English message, plus ` at position N: context` when known.
    pub fn message(&self) -> String {
        let mut message = self.message.english();
        if let Some(context) = self.context() {
            message.push_str(&format!(" at position {}: {}", self.cursor, context));
        }
        message
    }
}

impl std::fmt::Display for CommandSyntaxException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for CommandSyntaxException {}

pub(crate) fn utf16_to_string(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// Constructors for the provider's reader and argument exceptions (the keys and argument
/// order of `net.minecraft.commands.BrigadierExceptions`).
pub(crate) mod built_in {
    use super::{CommandArg, CommandMessage, CommandSyntaxException};

    pub fn simple(key: &'static str, input: &[u16], cursor: usize) -> CommandSyntaxException {
        CommandSyntaxException::with_context(CommandMessage::new(key, Vec::new()), input, cursor)
    }

    pub fn string_arg(
        key: &'static str,
        value: String,
        input: &[u16],
        cursor: usize,
    ) -> CommandSyntaxException {
        CommandSyntaxException::with_context(
            CommandMessage::new(key, vec![CommandArg::String(value)]),
            input,
            cursor,
        )
    }

    /// `Dynamic2CommandExceptionType.createWithContext(reader, found, bound)`: the
    /// message arguments are ordered `(bound, found)`.
    pub fn range(
        key: &'static str,
        bound: CommandArg,
        found: CommandArg,
        input: &[u16],
        cursor: usize,
    ) -> CommandSyntaxException {
        CommandSyntaxException::with_context(CommandMessage::new(key, vec![bound, found]), input, cursor)
    }
}
