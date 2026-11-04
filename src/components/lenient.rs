//! `JsonParser.parseString`: Gson's lenient JSON reader.
//!
//! `Component.Serializer.fromJson(String)` (used by `scoreboard.dat`, and by command arguments
//! through their own readers) does not require strict JSON: names and strings may be unquoted or
//! single-quoted, `=`/`=>` may separate a name from its value, `;` may separate members and
//! elements, and comments are skipped. An empty document is JSON `null`.

use serde_json::{Map, Number, Value};

struct Reader<'a> {
    chars: Vec<char>,
    position: usize,
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
                Some(' ' | '\t' | '\r' | '\n' | '\u{c}') => self.position += 1,
                Some('#') => self.skip_line(),
                Some('/') => match self.chars.get(self.position + 1) {
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
        let mut out = String::new();
        loop {
            let c = self.peek().ok_or("Unterminated string")?;
            self.position += 1;
            if c == quote {
                return Ok(out);
            }
            if c != '\\' {
                out.push(c);
                continue;
            }
            let escape = self.peek().ok_or("Unterminated escape sequence")?;
            self.position += 1;
            match escape {
                'u' => {
                    let hex: String = self
                        .chars
                        .get(self.position..self.position + 4)
                        .ok_or("Unterminated escape sequence")?
                        .iter()
                        .collect();
                    let unit = u32::from_str_radix(&hex, 16)
                        .map_err(|_| format!("Malformed escape {hex}"))?;
                    self.position += 4;
                    out.push(char::from_u32(unit).unwrap_or('\u{fffd}'));
                }
                't' => out.push('\t'),
                'b' => out.push('\u{8}'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                'f' => out.push('\u{c}'),
                '\n' | '\'' | '"' | '\\' | '/' => out.push(escape),
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

    fn literal(text: String) -> Value {
        match text.to_ascii_lowercase().as_str() {
            "null" => return Value::Null,
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
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
            if let Ok(number) = serde_json::from_str::<Number>(&text) {
                return Value::Number(number);
            }
        }
        Value::String(text)
    }

    fn value(&mut self) -> Result<Value, String> {
        self.skip_whitespace()?;
        match self.peek() {
            None => Err("Unexpected end of input".to_owned()),
            Some('{') => {
                self.position += 1;
                let mut map = Map::new();
                loop {
                    self.skip_whitespace()?;
                    match self.peek() {
                        Some('}') => {
                            self.position += 1;
                            return Ok(Value::Object(map));
                        }
                        None => return Err("Unterminated object".to_owned()),
                        _ => {}
                    }
                    let name = match self.peek() {
                        Some(quote @ ('"' | '\'')) => self.quoted(quote)?,
                        Some(c) if DELIMITERS.contains(&c) => {
                            return Err(format!("Expected name at {}", self.position))
                        }
                        _ => self.unquoted(),
                    };
                    self.skip_whitespace()?;
                    match self.peek() {
                        Some(':') => self.position += 1,
                        Some('=') => {
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
                        Some(',' | ';') => self.position += 1,
                        Some('}') => {}
                        _ => return Err("Expected ',' or '}'".to_owned()),
                    }
                }
            }
            Some('[') => {
                self.position += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_whitespace()?;
                    match self.peek() {
                        Some(']') => {
                            self.position += 1;
                            return Ok(Value::Array(items));
                        }
                        // Lenient: an empty element is `null`.
                        Some(',' | ';') => {
                            self.position += 1;
                            items.push(Value::Null);
                            continue;
                        }
                        None => return Err("Unterminated array".to_owned()),
                        _ => {}
                    }
                    items.push(self.value()?);
                    self.skip_whitespace()?;
                    match self.peek() {
                        Some(',' | ';') => self.position += 1,
                        Some(']') => {}
                        _ => return Err("Expected ',' or ']'".to_owned()),
                    }
                }
            }
            Some(quote @ ('"' | '\'')) => Ok(Value::String(self.quoted(quote)?)),
            Some(c) if DELIMITERS.contains(&c) => {
                Err(format!("Expected value at {}", self.position))
            }
            Some(_) => Ok(Self::literal(self.unquoted())),
        }
    }
}

/// `JsonParser.parseString(text)`. `Err` is a `JsonSyntaxException`.
pub fn parse(text: &str) -> Result<Value, String> {
    // The non-execute prefix some servers add.
    let body = text.strip_prefix(")]}'\n").unwrap_or(text);
    let mut reader = Reader {
        chars: body.chars().collect(),
        position: 0,
        _text: body,
    };
    reader.skip_whitespace()?;
    if reader.peek().is_none() {
        return Ok(Value::Null);
    }
    let value = reader.value()?;
    if !value.is_null() {
        reader.skip_whitespace()?;
        if reader.peek().is_some() {
            return Err("Did not consume the entire document.".to_owned());
        }
    }
    Ok(value)
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
        assert_eq!(parse("[1;2,]").unwrap(), json!([1, 2]));
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
}
