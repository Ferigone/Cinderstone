//! `com.mojang.brigadier.StringReader` (Brigadier 1.2.9).
//!
//! The cursor is an index into the original Java `String`, i.e. UTF-16 code units, so the
//! reader keeps the input as `u16` units. Behavior and message keys are compared with the
//! unchanged original classes in `fixtures/commands-brigadier-vectors.json`.
//!
//! Derived from Mojang's Brigadier (MIT, see `licenses/brigadier-MIT.txt`).

use super::exception::{built_in, utf16_to_string, CommandSyntaxException};
use super::java_text::java_is_whitespace;

const SYNTAX_ESCAPE: u16 = b'\\' as u16;
const SYNTAX_DOUBLE_QUOTE: u16 = b'"' as u16;
const SYNTAX_SINGLE_QUOTE: u16 = b'\'' as u16;

#[derive(Clone, Debug)]
pub struct StringReader {
    units: Vec<u16>,
    cursor: usize,
}

impl StringReader {
    pub fn new(string: &str) -> Self {
        Self {
            units: string.encode_utf16().collect(),
            cursor: 0,
        }
    }

    /// The whole input as UTF-16 code units (what exceptions keep as their input).
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    pub fn string(&self) -> String {
        utf16_to_string(&self.units)
    }

    pub fn total_length(&self) -> usize {
        self.units.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// `setCursor`. Java accepts any int; later reads then fail with an unchecked
    /// exception. A cursor past the end is clamped to the end here.
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.units.len());
    }

    pub fn remaining_length(&self) -> usize {
        self.units.len().saturating_sub(self.cursor)
    }

    /// `getRead()`. Java's `substring` throws past the end; this clamps instead.
    pub fn read_so_far(&self) -> String {
        utf16_to_string(&self.units[..self.cursor.min(self.units.len())])
    }

    /// `getRemaining()`; clamped like `read_so_far`.
    pub fn remaining(&self) -> String {
        utf16_to_string(&self.units[self.cursor.min(self.units.len())..])
    }

    pub fn can_read_n(&self, length: usize) -> bool {
        self.cursor + length <= self.units.len()
    }

    pub fn can_read(&self) -> bool {
        self.can_read_n(1)
    }

    pub fn peek(&self) -> Option<u16> {
        self.units.get(self.cursor).copied()
    }

    pub fn peek_at(&self, offset: usize) -> Option<u16> {
        self.units.get(self.cursor + offset).copied()
    }

    /// `read()`: Java evaluates `string.charAt(cursor++)`, so the cursor advances even
    /// when the access throws `StringIndexOutOfBoundsException` (returned here as `None`).
    pub fn read(&mut self) -> Option<u16> {
        let unit = self.units.get(self.cursor).copied();
        self.cursor += 1;
        unit
    }

    pub fn skip(&mut self) {
        self.cursor += 1;
    }

    pub fn is_allowed_number(unit: u16) -> bool {
        (b'0' as u16..=b'9' as u16).contains(&unit) || unit == b'.' as u16 || unit == b'-' as u16
    }

    pub fn is_quoted_string_start(unit: u16) -> bool {
        unit == SYNTAX_DOUBLE_QUOTE || unit == SYNTAX_SINGLE_QUOTE
    }

    pub fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(java_is_whitespace) {
            self.skip();
        }
    }

    fn error(&self, key: &'static str) -> CommandSyntaxException {
        built_in::simple(key, &self.units, self.cursor)
    }

    fn error_with(&self, key: &'static str, value: String) -> CommandSyntaxException {
        built_in::string_arg(key, value, &self.units, self.cursor)
    }

    fn read_number_text(&mut self) -> String {
        let start = self.cursor;
        while self.peek().is_some_and(Self::is_allowed_number) {
            self.skip();
        }
        utf16_to_string(&self.units[start..self.cursor])
    }

    pub fn read_int(&mut self) -> Result<i32, CommandSyntaxException> {
        let start = self.cursor;
        let number = self.read_number_text();
        if number.is_empty() {
            return Err(self.error("parsing.int.expected"));
        }
        number.parse::<i32>().map_err(|_| {
            self.cursor = start;
            self.error_with("parsing.int.invalid", number)
        })
    }

    pub fn read_long(&mut self) -> Result<i64, CommandSyntaxException> {
        let start = self.cursor;
        let number = self.read_number_text();
        if number.is_empty() {
            return Err(self.error("parsing.long.expected"));
        }
        number.parse::<i64>().map_err(|_| {
            self.cursor = start;
            self.error_with("parsing.long.invalid", number)
        })
    }

    pub fn read_double(&mut self) -> Result<f64, CommandSyntaxException> {
        let start = self.cursor;
        let number = self.read_number_text();
        if number.is_empty() {
            return Err(self.error("parsing.double.expected"));
        }
        number.parse::<f64>().map_err(|_| {
            self.cursor = start;
            self.error_with("parsing.double.invalid", number)
        })
    }

    pub fn read_float(&mut self) -> Result<f32, CommandSyntaxException> {
        let start = self.cursor;
        let number = self.read_number_text();
        if number.is_empty() {
            return Err(self.error("parsing.float.expected"));
        }
        number.parse::<f32>().map_err(|_| {
            self.cursor = start;
            self.error_with("parsing.float.invalid", number)
        })
    }

    pub fn is_allowed_in_unquoted_string(unit: u16) -> bool {
        (b'0' as u16..=b'9' as u16).contains(&unit)
            || (b'A' as u16..=b'Z' as u16).contains(&unit)
            || (b'a' as u16..=b'z' as u16).contains(&unit)
            || unit == b'_' as u16
            || unit == b'-' as u16
            || unit == b'.' as u16
            || unit == b'+' as u16
    }

    pub fn read_unquoted_string(&mut self) -> String {
        let start = self.cursor;
        while self.peek().is_some_and(Self::is_allowed_in_unquoted_string) {
            self.skip();
        }
        utf16_to_string(&self.units[start..self.cursor])
    }

    pub fn read_quoted_string(&mut self) -> Result<String, CommandSyntaxException> {
        let Some(next) = self.peek() else {
            return Ok(String::new());
        };
        if !Self::is_quoted_string_start(next) {
            return Err(self.error("parsing.quote.expected.start"));
        }
        self.skip();
        self.read_string_until(next)
    }

    pub fn read_string_until(&mut self, terminator: u16) -> Result<String, CommandSyntaxException> {
        let mut result: Vec<u16> = Vec::new();
        let mut escaped = false;
        while self.can_read() {
            let unit = self.read().expect("can_read checked the bounds");
            if escaped {
                if unit == terminator || unit == SYNTAX_ESCAPE {
                    result.push(unit);
                    escaped = false;
                } else {
                    self.cursor -= 1;
                    return Err(self.error_with("parsing.quote.escape", utf16_to_string(&[unit])));
                }
            } else if unit == SYNTAX_ESCAPE {
                escaped = true;
            } else if unit == terminator {
                return Ok(utf16_to_string(&result));
            } else {
                result.push(unit);
            }
        }
        Err(self.error("parsing.quote.expected.end"))
    }

    pub fn read_string(&mut self) -> Result<String, CommandSyntaxException> {
        let Some(next) = self.peek() else {
            return Ok(String::new());
        };
        if Self::is_quoted_string_start(next) {
            self.skip();
            return self.read_string_until(next);
        }
        Ok(self.read_unquoted_string())
    }

    pub fn read_boolean(&mut self) -> Result<bool, CommandSyntaxException> {
        let start = self.cursor;
        let value = self.read_string()?;
        if value.is_empty() {
            return Err(self.error("parsing.bool.expected"));
        }
        match value.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => {
                self.cursor = start;
                Err(self.error_with("parsing.bool.invalid", value))
            }
        }
    }

    pub fn expect(&mut self, symbol: u16) -> Result<(), CommandSyntaxException> {
        if self.peek() != Some(symbol) {
            return Err(self.error_with("parsing.expected", utf16_to_string(&[symbol])));
        }
        self.skip();
        Ok(())
    }
}
