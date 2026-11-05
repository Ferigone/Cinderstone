//! `RangeArgument.Ints` / `RangeArgument.Floats`: the `1..5`, `..5`, `3..` and `4` ranges of
//! `MinMaxBounds`.
//!
//! Ported from the unchanged classes of the pinned 1.21 server, read with direct `javap` inspection
//! of the jar (remapped with the official mappings, kept outside git) and compared with the executed
//! originals in `fixtures/commands-mc-arguments-vectors.json`; their `ArgumentTypeInfos` registry ids
//! (38 and 39, no template) are compared with the original's own `ClientboundCommandsPacket` in
//! `fixtures/commands-brigadier-tree-packet-vectors.json`. Suggestions are not part of this stage
//! (the originals keep the default, which offers nothing).
//!
//! What the original does and this port reproduces:
//!
//! * an empty remainder fails with `argument.range.empty` at the cursor, before anything is read;
//! * the number characters are `0-9`, `-` and `.`, except that a `.` followed by another `.` ends the
//!   number (the `..` separator); an empty text is "no bound", otherwise it must parse with
//!   `Integer.parseInt` / `Double.parseDouble` or the argument fails with `parsing.int.invalid` /
//!   `parsing.double.invalid` carrying the text;
//! * with a `..` after the first number the second number is read the same way; without it the single
//!   number is both bounds; no bound at all is `argument.range.empty`; `min > max` is
//!   `argument.range.swapped`;
//! * every failure after the first character check restores the cursor to where the argument began
//!   and reports that cursor (the exception is recreated with the start position);
//! * the value also carries the squared bounds (`Long` for integers, `Double` for floats), which are
//!   what distance checks compare against.
//!
//! The method `matches` of the originals belongs to the selector stage and is not ported here.

use super::arguments::{ArgumentNetworkInfo, ArgumentType};
use super::exception::{built_in, CommandSyntaxException};
use super::string_reader::StringReader;

/// `ArgumentTypeInfos` registry ids (1.21 registration order).
pub mod registry_ids {
    pub const INT_RANGE: i32 = 38;
    pub const FLOAT_RANGE: i32 = 39;
}

/// `MinMaxBounds.Ints`: optional bounds, with the squares as `long`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinMaxInts {
    pub min: Option<i32>,
    pub max: Option<i32>,
}

impl MinMaxInts {
    /// `minSq`: `min * (long) min`.
    pub fn min_sq(&self) -> Option<i64> {
        self.min.map(square_int)
    }

    pub fn max_sq(&self) -> Option<i64> {
        self.max.map(square_int)
    }
}

fn square_int(value: i32) -> i64 {
    i64::from(value) * i64::from(value)
}

/// `MinMaxBounds.Doubles`: optional bounds, with the squares as `double`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinMaxDoubles {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl MinMaxDoubles {
    pub fn min_sq(&self) -> Option<f64> {
        self.min.map(|value| value * value)
    }

    pub fn max_sq(&self) -> Option<f64> {
        self.max.map(|value| value * value)
    }
}

/// `MinMaxBounds.isAllowedInputChat`: a `.` starting the `..` separator is not a number character.
fn is_number_unit(reader: &StringReader) -> bool {
    let Some(unit) = reader.peek() else {
        return false;
    };
    let dot = b'.' as u16;
    if StringReader::is_allowed_number(unit) && unit != dot {
        return true;
    }
    if unit == dot && reader.can_read_n(2) && reader.peek_at(1) == Some(dot) {
        return false;
    }
    unit == dot
}

/// `MinMaxBounds.readNumber`: no characters is no bound; text that does not parse fails with the
/// text as the message argument and the cursor after it.
fn read_number<T>(
    reader: &mut StringReader,
    parse: impl Fn(&str) -> Option<T>,
    invalid_key: &'static str,
) -> Result<Option<T>, CommandSyntaxException> {
    let begin = reader.cursor();
    while is_number_unit(reader) {
        reader.skip();
    }
    let text = String::from_utf16_lossy(&reader.units()[begin..reader.cursor()]);
    if text.is_empty() {
        return Ok(None);
    }
    match parse(&text) {
        Some(value) => Ok(Some(value)),
        None => Err(built_in::string_arg(invalid_key, text, reader.units(), reader.cursor())),
    }
}

/// `MinMaxBounds.fromReader`, shared by both bound kinds. `create` is the factory, which fails with
/// `argument.range.swapped` when the minimum exceeds the maximum.
fn bounds_from_reader<T: Copy, R>(
    reader: &mut StringReader,
    parse: impl Fn(&str) -> Option<T>,
    invalid_key: &'static str,
    exceeds: impl Fn(T, T) -> bool,
    create: impl Fn(Option<T>, Option<T>) -> R,
) -> Result<R, CommandSyntaxException> {
    if !reader.can_read() {
        return Err(built_in::simple("argument.range.empty", reader.units(), reader.cursor()));
    }
    let start = reader.cursor();
    let attempt = (|| {
        let min = read_number(reader, &parse, invalid_key)?;
        let dot = b'.' as u16;
        let max = if reader.can_read_n(2) && reader.peek() == Some(dot) && reader.peek_at(1) == Some(dot) {
            reader.skip();
            reader.skip();
            let max = read_number(reader, &parse, invalid_key)?;
            if min.is_none() && max.is_none() {
                return Err(built_in::simple("argument.range.empty", reader.units(), reader.cursor()));
            }
            max
        } else {
            min
        };
        if min.is_none() && max.is_none() {
            return Err(built_in::simple("argument.range.empty", reader.units(), reader.cursor()));
        }
        if let (Some(low), Some(high)) = (min, max) {
            if exceeds(low, high) {
                return Err(built_in::simple("argument.range.swapped", reader.units(), reader.cursor()));
            }
        }
        Ok(create(min, max))
    })();
    attempt.map_err(|error| {
        // `new CommandSyntaxException(type, message, input, start)`: same message, the start cursor.
        reader.set_cursor(start);
        CommandSyntaxException::with_context(error.raw_message().clone(), reader.units(), start)
    })
}

fn parse_int(text: &str) -> Option<i32> {
    text.parse::<i32>().ok()
}

fn parse_double(text: &str) -> Option<f64> {
    text.parse::<f64>().ok()
}

/// `RangeArgument.intRange()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct IntRangeArgumentType;

impl ArgumentType for IntRangeArgumentType {
    type Value = MinMaxInts;

    fn parse(&self, reader: &mut StringReader) -> Result<MinMaxInts, CommandSyntaxException> {
        bounds_from_reader(reader, parse_int, "parsing.int.invalid", |min, max| min > max, |min, max| MinMaxInts {
            min,
            max,
        })
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        Some(ArgumentNetworkInfo {
            registry_id: registry_ids::INT_RANGE,
            template: Vec::new(),
        })
    }
}

/// `RangeArgument.floatRange()`: `double` bounds, as the original's `MinMaxBounds.Doubles`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FloatRangeArgumentType;

impl ArgumentType for FloatRangeArgumentType {
    type Value = MinMaxDoubles;

    fn parse(&self, reader: &mut StringReader) -> Result<MinMaxDoubles, CommandSyntaxException> {
        bounds_from_reader(reader, parse_double, "parsing.double.invalid", |min, max| min > max, |min, max| {
            MinMaxDoubles { min, max }
        })
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        Some(ArgumentNetworkInfo {
            registry_id: registry_ids::FLOAT_RANGE,
            template: Vec::new(),
        })
    }
}
