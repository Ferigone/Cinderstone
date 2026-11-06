//! Gson's `JsonReader`, in both of the modes the server uses.
//!
//! [`parse`] is `JsonParser.parseString` (lenient): `Component.Serializer.fromJson(String)` (used by
//! `scoreboard.dat`, and by command arguments through their own readers) does not require strict
//! JSON: names and strings may be unquoted or single-quoted, `=`/`=>` may separate a name from its
//! value, `;` may separate members and elements, and comments are skipped. An empty document is
//! JSON `null`.
//!
//! [`parse_strict_prefix`] is `Streams.parse` over a `JsonReader` with `setLenient(false)`, which
//! is how files such as `advancements/<uuid>.json` are read: strict grammar, but unlike `serde_json`
//! a raw control character inside a string is accepted, a lone surrogate escape is kept, numbers
//! keep their text whatever their size, and nothing after the first value is looked at.

use serde_json::{Map, Number, Value};

/// Nesting the readers follow before they give up. The original recurses until the JVM stack runs
/// out (`Streams.parse` turns the `StackOverflowError` into a parse failure), which takes
/// thousands of levels; this is a fixed, smaller bound.
pub const MAX_DEPTH: usize = 512;

struct Reader<'a> {
    chars: Vec<char>,
    position: usize,
    strict: bool,
    depth: usize,
    _text: &'a str,
}

const DELIMITERS: &[char] = &[
    '/', '\\', ';', '#', '=', '{', '}', '[', ']', ':', ',', ' ', '\t', '\u{c}', '\r', '\n',
];

impl<'a> Reader<'a> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn skip_whitespace(&mut self) -> Result<(), String> {
        loop {
            match self.peek() {
                Some(' ' | '\t' | '\r' | '\n') => self.position += 1,
                Some('#') if !self.strict => self.skip_line(),
                Some('/') if !self.strict => match self.chars.get(self.position + 1) {
                    Some('/') => self.skip_line(),
                    Some('*') => {
                        let start = self.position + 2;
                        let end = (start..self.chars.len().saturating_sub(1))
                            .find(|i| self.chars[*i] == '*' && self.chars[i + 1] == '/');
                        match end {
                            Some(i) => self.position = i + 2,
                            None => return Err("Unterminated comment".to_owned()),
                        }
                    }
                    _ => return Ok(()),
                },
                _ => return Ok(()),
            }
        }
    }

    fn skip_line(&mut self) {
        while let Some(c) = self.peek() {
            self.position += 1;
            if c == '\n' {
                break;
            }
        }
    }

    fn quoted(&mut self, quote: char) -> Result<String, String> {
        self.position += 1;
        // Java strings are UTF-16: an escaped surrogate pair is one character, a lone surrogate
        // (which `String` cannot hold) becomes U+FFFD.
        let mut units: Vec<u16> = Vec::new();
        loop {
            let c = self.peek().ok_or("Unterminated string")?;
            self.position += 1;
            if c == quote {
                return Ok(String::from_utf16_lossy(&units));
            }
            if c != '\\' {
                let mut buffer = [0u16; 2];
                units.extend_from_slice(c.encode_utf16(&mut buffer));
                continue;
            }
            let escape = self.peek().ok_or("Unterminated escape sequence")?;
            self.position += 1;
            match escape {
                'u' => {
                    let hex = self
                        .chars
                        .get(self.position..self.position + 4)
                        .ok_or("Unterminated escape sequence")?;
                    if !hex.iter().all(char::is_ascii_hexdigit) {
                        return Err(format!(
                            "Malformed escape {}",
                            hex.iter().collect::<String>()
                        ));
                    }
                    let digits: String = hex.iter().collect();
                    let unit = u16::from_str_radix(&digits, 16)
                        .map_err(|_| format!("Malformed escape {digits}"))?;
                    self.position += 4;
                    units.push(unit);
                }
                't' => units.push(u16::from(b'\t')),
                'b' => units.push(8),
                'n' => units.push(u16::from(b'\n')),
                'r' => units.push(u16::from(b'\r')),
                'f' => units.push(12),
                '\n' | '\'' | '"' | '\\' | '/' => {
                    let mut buffer = [0u16; 2];
                    units.extend_from_slice(escape.encode_utf16(&mut buffer));
                }
                other => return Err(format!("Invalid escape sequence {other}")),
            }
        }
    }

    fn unquoted(&mut self) -> String {
        let start = self.position;
        while let Some(c) = self.peek() {
            if DELIMITERS.contains(&c) {
                break;
            }
            self.position += 1;
        }
        self.chars[start..self.position].iter().collect()
    }

    /// An unquoted token: a keyword (either case), a number, or (only when lenient) a string.
    fn literal(&self, text: String) -> Result<Value, String> {
        match text.to_ascii_lowercase().as_str() {
            "null" => return Ok(Value::Null),
            "true" => return Ok(Value::Bool(true)),
            "false" => return Ok(Value::Bool(false)),
            _ => {}
        }
        // A number token: `-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?` and nothing else.
        let looks_numeric = {
            let bytes = text.as_bytes();
            let mut i = usize::from(bytes.first() == Some(&b'-'));
            let digits = |from: usize| {
                bytes[from..]
                    .iter()
                    .take_while(|b| b.is_ascii_digit())
                    .count()
            };
            let integer = digits(i);
            let leading_zero = integer > 1 && bytes[i] == b'0';
            i += integer;
            let mut ok = integer > 0 && !leading_zero;
            if ok && bytes.get(i) == Some(&b'.') {
                let fraction = digits(i + 1);
                ok = fraction > 0;
                i += 1 + fraction;
            }
            if ok && matches!(bytes.get(i), Some(b'e' | b'E')) {
                let mut j = i + 1;
                if matches!(bytes.get(j), Some(b'+' | b'-')) {
                    j += 1;
                }
                let exponent = digits(j);
                ok = exponent > 0;
                i = j + exponent;
            }
            ok && i == bytes.len()
        };
        if looks_numeric {
            // Gson keeps the digits of a number whatever its size. A value `f64` cannot hold
            // (`1e400`) stays as its text when lenient (a component prints it back as text) and
            // is the largest finite double of its sign when strict, where only the magnitude
            // is ever used.
            return Ok(match serde_json::from_str::<Number>(&text) {
                Ok(number) => Value::Number(number),
                Err(_) if self.strict => {
                    let largest = if text.starts_with('-') {
                        -f64::MAX
                    } else {
                        f64::MAX
                    };
                    Value::Number(Number::from_f64(largest).expect("finite"))
                }
                Err(_) => Value::String(text),
            });
        }
        if self.strict {
            return Err("Use JsonReader.setLenient(true) to accept malformed JSON".to_owned());
        }
        Ok(Value::String(text))
    }

    fn value(&mut self) -> Result<Value, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err("nesting too deep".to_owned());
        }
        let result = self.value_inner();
        self.depth -= 1;
        result
    }

    fn name(&mut self) -> Result<String, String> {
        match self.peek() {
            Some('"') => self.quoted('"'),
            Some('\'') if !self.strict => self.quoted('\''),
            Some(c) if self.strict || DELIMITERS.contains(&c) => {
                Err(format!("Expected name at {}", self.position))
            }
            _ => Ok(self.unquoted()),
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.position += 1;
        let mut map = Map::new();
        let mut first = true;
        loop {
            self.skip_whitespace()?;
            match self.peek() {
                Some('}') if first => {
                    self.position += 1;
                    return Ok(Value::Object(map));
                }
                None => return Err("Unterminated object".to_owned()),
                _ => {}
            }
            first = false;
            let name = self.name()?;
            self.skip_whitespace()?;
            match self.peek() {
                Some(':') => self.position += 1,
                Some('=') if !self.strict => {
                    self.position += 1;
                    if self.peek() == Some('>') {
                        self.position += 1;
                    }
                }
                _ => return Err("Expected ':'".to_owned()),
            }
            let value = self.value()?;
            map.insert(name, value);
            self.skip_whitespace()?;
            match self.peek() {
                Some(',') => self.position += 1,
                Some(';') if !self.strict => self.position += 1,
                Some('}') => {
                    self.position += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err("Expected ',' or '}'".to_owned()),
            }
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.position += 1;
        let mut items = Vec::new();
        let mut first = true;
        loop {
            self.skip_whitespace()?;
            match self.peek() {
                Some(']') if first => {
                    self.position += 1;
                    return Ok(Value::Array(items));
                }
                // Lenient: a separator or the closing bracket where an element is expected means
                // an empty element, which is `null` (`[1,]` is `[1, null]`, `[,1]` is `[null, 1]`).
                Some(',' | ';' | ']') if !self.strict => items.push(Value::Null),
                None => return Err("Unterminated array".to_owned()),
                _ => items.push(self.value()?),
            }
            first = false;
            self.skip_whitespace()?;
            match self.peek() {
                Some(',') => self.position += 1,
                Some(';') if !self.strict => self.position += 1,
                Some(']') => {
                    self.position += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err("Expected ',' or ']'".to_owned()),
            }
        }
    }

    fn value_inner(&mut self) -> Result<Value, String> {
        self.skip_whitespace()?;
        match self.peek() {
            None => Err("Unexpected end of input".to_owned()),
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Value::String(self.quoted('"')?)),
            Some('\'') if !self.strict => Ok(Value::String(self.quoted('\'')?)),
            Some(c) if DELIMITERS.contains(&c) || (self.strict && c == '\'') => {
                Err(format!("Expected value at {}", self.position))
            }
            Some(_) => {
                let token = self.unquoted();
                self.literal(token)
            }
        }
    }
}

fn reader(body: &str, strict: bool) -> Reader<'_> {
    // The reader skips a leading byte-order mark.
    let body = body.strip_prefix('\u{feff}').unwrap_or(body);
    Reader {
        chars: body.chars().collect(),
        position: 0,
        strict,
        depth: 0,
        _text: body,
    }
}

/// `JsonParser.parseString(text)`. `Err` is a `JsonSyntaxException`.
pub fn parse(text: &str) -> Result<Value, String> {
    // The non-execute prefix some servers add.
    let body = text.strip_prefix(")]}'\n").unwrap_or(text);
    let mut reader = reader(body, false);
    reader.skip_whitespace()?;
    if reader.peek().is_none() {
        return Ok(Value::Null);
    }
    let value = reader.value()?;
    if !value.is_null() {
        // `parseReader` has put the reader back to strict by the time it looks for the end of
        // the document, so a comment after the value is an error too.
        reader.strict = true;
        reader.skip_whitespace()?;
        if reader.peek().is_some() {
            return Err("Did not consume the entire document.".to_owned());
        }
    }
    Ok(value)
}

/// `Streams.parse(JsonReader)` with `setLenient(false)` over `text`: strict grammar, one value, and
/// whatever follows that value is never looked at. An empty document is `null`.
pub fn parse_strict_prefix(text: &str) -> Result<Value, String> {
    let mut reader = reader(text, true);
    reader.skip_whitespace()?;
    if reader.peek().is_none() {
        return Ok(Value::Null);
    }
    reader.value()
}

#[cfg(test)]
mod tests {
    use super::parse;
    use serde_json::json;

    #[test]
    fn strict_json_is_unchanged() {
        assert_eq!(
            parse(r#"{"a":[1,2.5,"x",true,null]}"#).unwrap(),
            json!({"a": [1, 2.5, "x", true, null]})
        );
    }

    #[test]
    fn lenient_forms_are_read_like_gson() {
        assert_eq!(parse("x").unwrap(), json!("x"));
        assert_eq!(parse("'single'").unwrap(), json!("single"));
        assert_eq!(parse("{text:abc}").unwrap(), json!({"text": "abc"}));
        assert_eq!(parse("{a=1;b=>true}").unwrap(), json!({"a": 1, "b": true}));
        assert_eq!(parse("[1;2,]").unwrap(), json!([1, 2, null]));
        assert_eq!(parse("[,1]").unwrap(), json!([null, 1]));
        assert_eq!(parse("[1,,2]").unwrap(), json!([1, null, 2]));
        assert_eq!(parse("123abc").unwrap(), json!("123abc"));
        assert_eq!(parse("TRUE").unwrap(), json!(true));
        assert_eq!(parse("/* c */ # d\n 7").unwrap(), json!(7));
        assert_eq!(parse("").unwrap(), json!(null));
        assert_eq!(parse("  ").unwrap(), json!(null));
    }

    #[test]
    fn trailing_content_is_rejected() {
        assert!(parse("1 2").is_err());
        assert!(parse("{").is_err());
        assert!(parse("{\"a\"}").is_err());
    }

    #[test]
    fn object_commas_and_trailing_comments_are_errors_like_gson() {
        assert!(parse(r#"{"a":1,}"#).is_err());
        assert!(parse("{a:1,}").is_err());
        assert!(parse(r#"{"a":1}//c"#).is_err());
        assert!(parse("/* c */ 1 ").is_ok());
        assert!(parse("\u{c}1").is_err(), "a form feed is not whitespace");
        assert_eq!(parse("\u{feff}{}").unwrap(), json!({}));
    }

    #[test]
    fn escaped_surrogate_pairs_combine() {
        assert_eq!(parse(r#""\ud83d\ude00""#).unwrap(), json!("\u{1f600}"));
        assert_eq!(parse(r#""\ud800""#).unwrap(), json!("\u{fffd}"));
        assert!(parse(r#""\u00zz""#).is_err());
    }

    #[test]
    fn strict_prefix_reads_one_value_like_streams_parse() {
        use super::parse_strict_prefix;
        assert_eq!(
            parse_strict_prefix(r#"{"a":1} trailing"#).unwrap(),
            json!({"a": 1})
        );
        assert_eq!(parse_strict_prefix("\u{feff}[1]]").unwrap(), json!([1]));
        assert_eq!(
            parse_strict_prefix("\"a\tb\nc\"").unwrap(),
            json!("a\tb\nc"),
            "raw control characters are accepted"
        );
        assert_eq!(
            parse_strict_prefix(r#"{"a":1,"a":2}"#).unwrap(),
            json!({"a": 2})
        );
        assert_eq!(parse_strict_prefix("TRUE").unwrap(), json!(true));
        for bad in [
            r#"{"a":1,}"#,
            "[1,]",
            "[,1]",
            r#"{"a":1;"b":2}"#,
            "{'a':1}",
            "{a:1}",
            r#"{"a"=1}"#,
            "[1;2]",
            "// c\n1",
            "/* c */1",
            "# c\n1",
            "NaN",
            "-",
            "-01",
            "1.",
            ".5",
            "1e",
            "x",
            r#"{"a":nul}"#,
            r#"{"a":1 "b":2}"#,
            r#"{"a":}"#,
            "{\"a\":1",
            ")]}'\n1",
        ] {
            assert!(parse_strict_prefix(bad).is_err(), "{bad:?}");
        }
        // Numbers keep their text whatever their size.
        assert!(parse_strict_prefix("1e400").is_ok());
        assert!(parse_strict_prefix("[1.0,1e2,-1E-2,-0]").is_ok());
        let deep = "[".repeat(super::MAX_DEPTH + 10) + &"]".repeat(super::MAX_DEPTH + 10);
        assert!(parse_strict_prefix(&deep).is_err());
        let fine = "[".repeat(100) + &"]".repeat(100);
        assert!(parse_strict_prefix(&fine).is_ok());
    }
}
