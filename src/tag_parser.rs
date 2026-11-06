//! `net.minecraft.nbt.TagParser` (SNBT reader) of 1.21 over Brigadier's `StringReader`.
//!
//! Accept/reject behaviour and the produced tags follow the original, which the local oracle
//! checks (`tools/loot_oracle.java snbt`, `tools/tag_parser_generate.py`). Notable rules the
//! original has and this port keeps:
//!
//! * a number that matches a suffix pattern but does not fit its type (`300b`, `99999999999`) is a
//!   *string*, and no later pattern is tried;
//! * patterns are case-insensitive (`1B`, `2L`, `1.5F`, `1E3`), `true`/`false` in any case are bytes;
//! * a list must hold one tag type; `[B;`, `[I;`, `[L;` arrays hold only bytes, ints or longs;
//! * a key may be quoted (`"` or `'`, escapes `\\` and the quote) or unquoted (`0-9A-Za-z_-.+`).
//!
//! There is no depth limit (the original overflows its stack on very deep input); callers that
//! take untrusted input should bound its length.

use crate::nbt::{JavaString, Tag};
use std::fmt;

/// Why a string is not SNBT (`CommandSyntaxException`, cursor in UTF-16 units of the input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnbtError {
    pub message: String,
    pub cursor: usize,
}

impl fmt::Display for SnbtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at position {}", self.message, self.cursor)
    }
}

impl std::error::Error for SnbtError {}

type Result<T> = std::result::Result<T, SnbtError>;

/// `TagParser.parseTag`: one compound and nothing but whitespace after it.
pub fn parse_compound(text: &str) -> Result<Tag> {
    let mut parser = TagParser::new(text);
    let tag = parser.read_struct()?;
    parser.skip_whitespace();
    if parser.can_read(1) {
        return parser.error("argument.nbt.trailing");
    }
    Ok(tag)
}

/// `new TagParser(reader).readValue()`: one value; trailing text is left unread (the second
/// field is the number of characters read).
pub fn parse_value_prefix(text: &str) -> Result<(Tag, usize)> {
    let mut parser = TagParser::new(text);
    let tag = parser.read_value()?;
    Ok((tag, parser.cursor))
}

/// `new TagParser(reader).readStruct()` on `chars` from `cursor` (used by NBT paths, which embed
/// compound patterns): the compound and the cursor after it.
pub(crate) fn read_struct_at(chars: &[char], cursor: usize) -> Result<(Tag, usize)> {
    let mut parser = TagParser { chars: chars.to_vec(), cursor };
    let tag = parser.read_struct()?;
    Ok((tag, parser.cursor))
}

struct TagParser {
    chars: Vec<char>,
    cursor: usize,
}

/// `Character.isWhitespace`.
fn java_whitespace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | '\u{001C}'..='\u{001F}')
        || (c.is_whitespace() && !matches!(c, '\u{00A0}' | '\u{2007}' | '\u{202F}') && c != '\u{0085}')
}

fn allowed_in_unquoted(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+')
}

fn quoted_start(c: char) -> bool {
    c == '"' || c == '\''
}

impl TagParser {
    fn new(text: &str) -> Self {
        Self { chars: text.chars().collect(), cursor: 0 }
    }

    fn error<T>(&self, message: &str) -> Result<T> {
        let cursor = self.chars[..self.cursor.min(self.chars.len())].iter().map(|c| c.len_utf16()).sum();
        Err(SnbtError { message: message.to_owned(), cursor })
    }

    fn can_read(&self, length: usize) -> bool {
        self.cursor + length <= self.chars.len()
    }

    fn peek_at(&self, offset: usize) -> char {
        self.chars[self.cursor + offset]
    }

    fn peek(&self) -> char {
        self.peek_at(0)
    }

    fn skip_whitespace(&mut self) {
        while self.can_read(1) && java_whitespace(self.peek()) {
            self.cursor += 1;
        }
    }

    /// `StringReader.expect` after skipping whitespace.
    fn expect(&mut self, c: char) -> Result<()> {
        self.skip_whitespace();
        if !self.can_read(1) || self.peek() != c {
            return self.error(&format!("parsing.expected {c}"));
        }
        self.cursor += 1;
        Ok(())
    }

    fn read_unquoted_string(&mut self) -> String {
        let start = self.cursor;
        while self.can_read(1) && allowed_in_unquoted(self.peek()) {
            self.cursor += 1;
        }
        self.chars[start..self.cursor].iter().collect()
    }

    fn read_string_until(&mut self, terminator: char) -> Result<String> {
        let mut out = String::new();
        let mut escaped = false;
        while self.can_read(1) {
            let c = self.peek();
            self.cursor += 1;
            if escaped {
                if c == terminator || c == '\\' {
                    out.push(c);
                    escaped = false;
                } else {
                    self.cursor -= 1;
                    return self.error("parsing.quote.escape");
                }
            } else if c == '\\' {
                escaped = true;
            } else if c == terminator {
                return Ok(out);
            } else {
                out.push(c);
            }
        }
        self.error("parsing.quote.expected.end")
    }

    fn read_quoted_string(&mut self) -> Result<String> {
        if !self.can_read(1) {
            return Ok(String::new());
        }
        let next = self.peek();
        if !quoted_start(next) {
            return self.error("parsing.quote.expected.start");
        }
        self.cursor += 1;
        self.read_string_until(next)
    }

    /// `StringReader.readString`.
    fn read_string(&mut self) -> Result<String> {
        if !self.can_read(1) {
            return Ok(String::new());
        }
        let next = self.peek();
        if quoted_start(next) {
            self.cursor += 1;
            return self.read_string_until(next);
        }
        Ok(self.read_unquoted_string())
    }

    fn read_key(&mut self) -> Result<String> {
        self.skip_whitespace();
        if !self.can_read(1) {
            return self.error("argument.nbt.expected.key");
        }
        self.read_string()
    }

    fn read_typed_value(&mut self) -> Result<Tag> {
        self.skip_whitespace();
        let start = self.cursor;
        if quoted_start(self.peek()) {
            return Ok(string_tag(self.read_quoted_string()?));
        }
        let text = self.read_unquoted_string();
        if text.is_empty() {
            self.cursor = start;
            return self.error("argument.nbt.expected.value");
        }
        Ok(typed(&text))
    }

    fn read_value(&mut self) -> Result<Tag> {
        self.skip_whitespace();
        if !self.can_read(1) {
            return self.error("argument.nbt.expected.value");
        }
        match self.peek() {
            '{' => self.read_struct(),
            '[' => {
                if self.can_read(3) && !quoted_start(self.peek_at(1)) && self.peek_at(2) == ';' {
                    self.read_array_tag()
                } else {
                    self.read_list_tag()
                }
            }
            _ => self.read_typed_value(),
        }
    }

    fn has_element_separator(&mut self) -> bool {
        self.skip_whitespace();
        if self.can_read(1) && self.peek() == ',' {
            self.cursor += 1;
            self.skip_whitespace();
            true
        } else {
            false
        }
    }

    fn read_struct(&mut self) -> Result<Tag> {
        self.expect('{')?;
        let mut entries: Vec<(JavaString, Tag)> = Vec::new();
        self.skip_whitespace();
        while self.can_read(1) && self.peek() != '}' {
            let start = self.cursor;
            let key = self.read_key()?;
            if key.is_empty() {
                self.cursor = start;
                return self.error("argument.nbt.expected.key");
            }
            self.expect(':')?;
            let value = self.read_value()?;
            let key = JavaString::Utf8(key);
            // `CompoundTag.put`: a repeated key replaces the earlier value.
            match entries.iter_mut().find(|(existing, _)| *existing == key) {
                Some(slot) => slot.1 = value,
                None => entries.push((key, value)),
            }
            if !self.has_element_separator() {
                break;
            }
            if !self.can_read(1) {
                return self.error("argument.nbt.expected.key");
            }
        }
        self.expect('}')?;
        Ok(Tag::Compound(entries))
    }

    fn read_list_tag(&mut self) -> Result<Tag> {
        self.expect('[')?;
        self.skip_whitespace();
        if !self.can_read(1) {
            return self.error("argument.nbt.expected.value");
        }
        let mut values = Vec::new();
        let mut element_type: Option<u8> = None;
        while self.peek() != ']' {
            let start = self.cursor;
            let value = self.read_value()?;
            let id = value.tag_id();
            match element_type {
                None => element_type = Some(id),
                Some(existing) if existing != id => {
                    self.cursor = start;
                    return self.error("argument.nbt.list.mixed");
                }
                _ => {}
            }
            values.push(value);
            if !self.has_element_separator() {
                break;
            }
            if !self.can_read(1) {
                return self.error("argument.nbt.expected.value");
            }
        }
        self.expect(']')?;
        Ok(Tag::List { element_type: element_type.unwrap_or(0), values })
    }

    fn read_array_tag(&mut self) -> Result<Tag> {
        self.expect('[')?;
        let start = self.cursor;
        let kind = self.peek();
        self.cursor += 2;
        self.skip_whitespace();
        if !self.can_read(1) {
            return self.error("argument.nbt.expected.value");
        }
        let element = match kind {
            'B' => 1,
            'L' => 4,
            'I' => 3,
            _ => {
                self.cursor = start;
                return self.error("argument.nbt.array.invalid");
            }
        };
        let mut values = Vec::new();
        while self.peek() != ']' {
            let at = self.cursor;
            let value = self.read_value()?;
            if value.tag_id() != element {
                self.cursor = at;
                return self.error("argument.nbt.array.mixed");
            }
            values.push(value);
            if !self.has_element_separator() {
                break;
            }
            if !self.can_read(1) {
                return self.error("argument.nbt.expected.value");
            }
        }
        self.expect(']')?;
        Ok(match kind {
            'B' => Tag::ByteArray(values.into_iter().map(|v| if let Tag::Byte(b) = v { b } else { 0 }).collect()),
            'L' => Tag::LongArray(values.into_iter().map(|v| if let Tag::Long(n) = v { n } else { 0 }).collect()),
            _ => Tag::IntArray(values.into_iter().map(|v| if let Tag::Int(n) = v { n } else { 0 }).collect()),
        })
    }
}

fn string_tag(text: String) -> Tag {
    Tag::String(JavaString::Utf8(text))
}

/// The shape `[-+]?(?:[0-9]+[.]?|[0-9]*[.][0-9]+)(?:e[-+]?[0-9]+)?` (`dot_required`: the
/// no-suffix double form `[0-9]+[.]|[0-9]*[.][0-9]+`), case-insensitive.
fn decimal_shape(text: &str, dot_required: bool) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - int_start;
    let mut dot = false;
    let mut frac_digits = 0;
    if i < bytes.len() && bytes[i] == b'.' {
        dot = true;
        i += 1;
        let frac_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = i - frac_start;
    }
    let mantissa_ok = if dot { int_digits > 0 || frac_digits > 0 } else { int_digits > 0 && !dot_required };
    if !mantissa_ok {
        return false;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == bytes.len()
}

/// `[-+]?(?:0|[1-9][0-9]*)`.
fn integer_shape(text: &str) -> bool {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) && (digits == "0" || !digits.starts_with('0'))
}

fn with_suffix<'a>(text: &'a str, suffix: char) -> Option<&'a str> {
    let last = text.chars().last()?;
    if last.eq_ignore_ascii_case(&suffix) {
        Some(&text[..text.len() - 1])
    } else {
        None
    }
}

/// `TagParser.type`.
fn typed(text: &str) -> Tag {
    let parsed = (|| -> Option<Option<Tag>> {
        if let Some(body) = with_suffix(text, 'f') {
            if decimal_shape(body, false) {
                // `FloatTag.valueOf`: any zero is the shared positive zero.
                return Some(body.parse::<f32>().ok().map(|v| Tag::Float(if v == 0.0 { 0.0 } else { v })));
            }
        }
        if let Some(body) = with_suffix(text, 'b') {
            if integer_shape(body) {
                return Some(body.parse::<i8>().ok().map(Tag::Byte));
            }
        }
        if let Some(body) = with_suffix(text, 'l') {
            if integer_shape(body) {
                return Some(body.parse::<i64>().ok().map(Tag::Long));
            }
        }
        if let Some(body) = with_suffix(text, 's') {
            if integer_shape(body) {
                return Some(body.parse::<i16>().ok().map(Tag::Short));
            }
        }
        if integer_shape(text) {
            return Some(text.parse::<i32>().ok().map(Tag::Int));
        }
        if let Some(body) = with_suffix(text, 'd') {
            if decimal_shape(body, false) {
                return Some(body.parse::<f64>().ok().map(|v| Tag::Double(if v == 0.0 { 0.0 } else { v })));
            }
        }
        if decimal_shape(text, true) {
            return Some(text.parse::<f64>().ok().map(|v| Tag::Double(if v == 0.0 { 0.0 } else { v })));
        }
        if text.eq_ignore_ascii_case("true") {
            return Some(Some(Tag::Byte(1)));
        }
        if text.eq_ignore_ascii_case("false") {
            return Some(Some(Tag::Byte(0)));
        }
        None
    })();
    match parsed {
        Some(Some(tag)) => tag,
        // No pattern matched, or the matching type's parser threw `NumberFormatException`.
        _ => string_tag(text.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tags as values: compounds compare as maps (the original iterates its `HashMap`), floats by bits.
    fn same(a: &Tag, b: &Tag) -> bool {
        match (a, b) {
            (Tag::Float(x), Tag::Float(y)) => x.to_bits() == y.to_bits(),
            (Tag::Double(x), Tag::Double(y)) => x.to_bits() == y.to_bits(),
            (Tag::List { element_type: t1, values: v1 }, Tag::List { element_type: t2, values: v2 }) => {
                (t1 == t2 || v1.is_empty()) && v1.len() == v2.len() && v1.iter().zip(v2).all(|(x, y)| same(x, y))
            }
            (Tag::Compound(e1), Tag::Compound(e2)) => {
                e1.len() == e2.len() && e1.iter().all(|(k, v)| e2.iter().any(|(k2, v2)| k == k2 && same(v, v2)))
            }
            _ => a == b,
        }
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
    }

    /// Every vector of the fixture (or of `TAG_PARSER_VECTORS`, a full local run) parses or fails as
    /// the original's `TagParser.parseTag` does, to the same tag.
    #[test]
    fn snbt_matches_the_original_tag_parser() {
        let text = match std::env::var("TAG_PARSER_VECTORS") {
            Ok(path) => std::fs::read_to_string(path).unwrap(),
            Err(_) => include_str!("../fixtures/tag-parser-vectors.json").to_owned(),
        };
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        let rows = doc["vectors"].as_array().unwrap();
        let mut failures = Vec::new();
        for row in rows {
            let input = row[0].as_str().unwrap();
            let ours = parse_compound(input);
            match (&ours, row[1].as_str()) {
                (Err(_), None) => {}
                (Ok(tag), Some(expected)) => {
                    let expected = Tag::decode_anonymous(&hex(expected)).unwrap();
                    if !same(tag, &expected) {
                        failures.push(format!("{input:?}: ours {tag:?}, original {expected:?}"));
                    }
                }
                _ => failures.push(format!("{input:?}: ours {ours:?}, original {:?}", row[1])),
            }
        }
        eprintln!("tag parser vectors: {} checked", rows.len());
        assert!(failures.is_empty(), "{} of {} differ:\n{}", failures.len(), rows.len(), failures[..failures.len().min(25)].join("\n"));
    }
}
