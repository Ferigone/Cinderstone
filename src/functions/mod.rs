//! Datapack functions (WP-E7 #28, stage 1): `CommandFunction.fromLines` (comments, line
//! continuation, the leading-slash errors, macro lines) and macro instantiation
//! (`StringTemplate`, `MacroFunction.instantiate` with its argument stringification).
//!
//! Commands are parsed by a caller-supplied function, so this module is independent of the
//! command dispatcher; the oracle test (`tools/loot_oracle.java functions`) runs the original over
//! the real 1.21 dispatcher and gives the port the dispatcher's verdict for each command string.

use crate::nbt::{JavaString, Tag};

/// Why a function file or a macro instantiation is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionError {
    /// "Line continuation at end of file".
    ContinuationAtEnd,
    /// A line starting with `//` ("use '#' not '//'").
    DoubleSlash { line: usize },
    /// A line starting with `/` ("Do not use a preceding forwards slash").
    LeadingSlash { line: usize },
    /// A line that is only `/`: the original peeks past its end
    /// (`StringIndexOutOfBoundsException`).
    SlashOnly,
    /// A (joined) line longer than 2,000,000 UTF-16 units.
    CommandTooLong,
    MacroWithoutVariables { line: usize },
    InvalidVariableName { name: String, line: usize },
    UnterminatedVariable { line: usize },
    /// The command parser rejected a line ("Whilst parsing command on line …").
    Command { line: usize, message: String },
    /// `commands.function.error.missing_arguments`: a macro called without arguments.
    MissingArguments,
    /// `commands.function.error.missing_argument`.
    MissingArgument(String),
    /// `commands.function.error.parse`: a substituted macro line does not parse.
    InstantiatedCommand { command: String, message: String },
    /// An argument value this port cannot stringify (an SNBT float inside a compound or list).
    Unsupported(String),
}

impl std::fmt::Display for FunctionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for FunctionError {}

/// `StringTemplate`: literal segments around `$(variable)` references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringTemplate {
    pub segments: Vec<String>,
    pub variables: Vec<String>,
}

/// `Character.isLetterOrDigit` (approximated by Unicode alphanumerics) or `_`.
fn valid_variable_name(name: &str) -> bool {
    name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

impl StringTemplate {
    /// `StringTemplate.fromString(text, line)`.
    pub fn from_string(text: &str, line: usize) -> Result<Self, FunctionError> {
        let mut segments = Vec::new();
        let mut variables = Vec::new();
        let mut start = 0;
        let mut search = text.find('$');
        while let Some(at) = search {
            if at != text.len() - 1 && text[at + 1..].starts_with('(') {
                segments.push(text[start..at].to_owned());
                let Some(close) = text[at + 1..].find(')').map(|i| i + at + 1) else {
                    return Err(FunctionError::UnterminatedVariable { line });
                };
                let name = &text[at + 2..close];
                if !valid_variable_name(name) {
                    return Err(FunctionError::InvalidVariableName { name: name.to_owned(), line });
                }
                variables.push(name.to_owned());
                start = close + 1;
                search = text[start..].find('$').map(|i| i + start);
            } else {
                search = text[at + 1..].find('$').map(|i| i + at + 1);
            }
        }
        if start == 0 {
            return Err(FunctionError::MacroWithoutVariables { line });
        }
        if start != text.len() {
            segments.push(text[start..].to_owned());
        }
        Ok(Self { segments, variables })
    }

    /// `StringTemplate.substitute(values)`.
    pub fn substitute(&self, values: &[String]) -> Result<String, FunctionError> {
        let mut out = String::new();
        for (index, value) in values.iter().enumerate().take(self.variables.len()) {
            out.push_str(&self.segments[index]);
            out.push_str(value);
            check_length(&out)?;
        }
        if self.segments.len() > self.variables.len() {
            out.push_str(&self.segments[self.segments.len() - 1]);
        }
        check_length(&out)?;
        Ok(out)
    }
}

fn check_length(text: &str) -> Result<(), FunctionError> {
    if text.encode_utf16().count() > 2_000_000 {
        Err(FunctionError::CommandTooLong)
    } else {
        Ok(())
    }
}

/// One entry of a macro function.
#[derive(Debug, Clone, PartialEq)]
pub enum MacroEntry<C> {
    Plain(C),
    /// A template and, for each of its variables, the index into the function's parameters.
    Template { template: StringTemplate, indices: Vec<usize> },
}

/// A parsed function: plain commands, or a macro function once any `$` line appears.
#[derive(Debug, Clone, PartialEq)]
pub enum Function<C> {
    Plain(Vec<C>),
    Macro { parameters: Vec<String>, entries: Vec<MacroEntry<C>> },
}

/// `String.trim()`: characters up to U+0020 are removed from both ends.
fn java_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}

/// `CommandFunction.fromLines(id, dispatcher, source, lines)` with `parse` standing in for
/// `CommandFunction.parseCommand` (its error message becomes [`FunctionError::Command`]).
pub fn from_lines<C>(lines: &[String], parse: &dyn Fn(&str) -> Result<C, String>) -> Result<Function<C>, FunctionError> {
    let mut plain: Option<Vec<C>> = Some(Vec::new());
    let mut macro_entries: Vec<MacroEntry<C>> = Vec::new();
    let mut parameters: Vec<String> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line_number = index + 1;
        let mut text = java_trim(&lines[index]).to_owned();
        if text.ends_with('\\') {
            loop {
                index += 1;
                if index == lines.len() {
                    return Err(FunctionError::ContinuationAtEnd);
                }
                text.pop();
                text.push_str(java_trim(&lines[index]));
                check_length(&text)?;
                if !text.ends_with('\\') {
                    break;
                }
            }
        }
        check_length(&text)?;
        index += 1;
        let Some(first) = text.chars().next() else { continue };
        if first == '#' {
            continue;
        }
        if first == '/' {
            if text.len() == 1 {
                return Err(FunctionError::SlashOnly);
            }
            if text[1..].starts_with('/') {
                return Err(FunctionError::DoubleSlash { line: line_number });
            }
            return Err(FunctionError::LeadingSlash { line: line_number });
        }
        if first == '$' {
            let template = StringTemplate::from_string(&text[1..], line_number)?;
            if let Some(commands) = plain.take() {
                macro_entries.extend(commands.into_iter().map(MacroEntry::Plain));
            }
            let indices = template
                .variables
                .iter()
                .map(|name| match parameters.iter().position(|p| p == name) {
                    Some(at) => at,
                    None => {
                        parameters.push(name.clone());
                        parameters.len() - 1
                    }
                })
                .collect();
            macro_entries.push(MacroEntry::Template { template, indices });
        } else {
            let command = parse(&text).map_err(|message| FunctionError::Command { line: line_number, message })?;
            match &mut plain {
                Some(commands) => commands.push(command),
                None => macro_entries.push(MacroEntry::Plain(command)),
            }
        }
    }
    Ok(match plain {
        Some(commands) => Function::Plain(commands),
        None => Function::Macro { parameters, entries: macro_entries },
    })
}

/// `new DecimalFormat("#")` with up to 15 fraction digits (`HALF_EVEN`, US symbols, no minimum
/// integer digits) over a double's shortest decimal form.
fn decimal_format(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "\u{221E}".to_owned() } else { "-\u{221E}".to_owned() };
    }
    let negative = value.is_sign_negative();
    // Digits and decimal exponent of the shortest representation.
    let formatted = format!("{:e}", value.abs());
    let (mantissa, exponent) = formatted.split_once('e').unwrap();
    let exponent: i64 = exponent.parse().unwrap();
    let digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    // value = 0.d1d2d3… × 10^(exponent + 1)
    let point = exponent + 1;
    let mut integer: Vec<u8> = Vec::new();
    let mut fraction: Vec<u8> = Vec::new();
    for (i, digit) in digits.iter().enumerate() {
        if (i as i64) < point {
            integer.push(*digit);
        } else {
            fraction.push(*digit);
        }
    }
    for _ in digits.len() as i64..point {
        integer.push(0);
    }
    if point < 0 {
        let mut padded = vec![0u8; (-point) as usize];
        padded.extend(fraction);
        fraction = padded;
    }
    if fraction.len() > 15 {
        let dropped = &fraction[15..];
        let first = dropped[0];
        let rest_nonzero = dropped[1..].iter().any(|d| *d != 0);
        fraction.truncate(15);
        let last_odd = fraction.last().copied().unwrap_or_else(|| integer.last().copied().unwrap_or(0)) % 2 == 1;
        let round_up = first > 5 || (first == 5 && (rest_nonzero || last_odd));
        if round_up {
            let mut all: Vec<u8> = integer.iter().chain(fraction.iter()).copied().collect();
            let mut i = all.len();
            loop {
                if i == 0 {
                    all.insert(0, 1);
                    integer.insert(0, 0);
                    break;
                }
                i -= 1;
                if all[i] == 9 {
                    all[i] = 0;
                } else {
                    all[i] += 1;
                    break;
                }
            }
            let split = integer.len();
            integer = all[..split].to_vec();
            fraction = all[split..].to_vec();
        }
    }
    while fraction.last() == Some(&0) {
        fraction.pop();
    }
    while integer.len() > 1 && integer[0] == 0 {
        integer.remove(0);
    }
    if integer.is_empty() {
        integer.push(0);
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    // The pattern `#` has no minimum integer digits: a zero integer part is left out unless
    // nothing else would be written.
    if !(integer == [0] && !fraction.is_empty()) {
        out.extend(integer.iter().map(|d| char::from(b'0' + d)));
    }
    if !fraction.is_empty() {
        out.push('.');
        out.extend(fraction.iter().map(|d| char::from(b'0' + d)));
    }
    out
}

/// `MacroFunction.stringify(tag)`.
pub fn stringify(tag: &Tag) -> Result<String, FunctionError> {
    Ok(match tag {
        Tag::Float(f) => decimal_format(f64::from(*f)),
        Tag::Double(d) => decimal_format(*d),
        Tag::Byte(n) => n.to_string(),
        Tag::Short(n) => n.to_string(),
        Tag::Int(n) => n.to_string(),
        Tag::Long(n) => n.to_string(),
        Tag::String(JavaString::Utf8(text)) => text.clone(),
        other => crate::nbt_snbt::to_snbt_without_floating(other)
            .map(|text| String::from_utf16_lossy(&text.units()))
            .map_err(|e| FunctionError::Unsupported(format!("macro argument {e}")))?,
    })
}

impl<C> Function<C> {
    /// `MacroFunction.instantiate(arguments, dispatcher)`: the commands of the instantiated
    /// function (a plain function's are its own). `arguments` is the compound passed to the
    /// function, if any.
    pub fn instantiate<'a>(
        &'a self,
        arguments: Option<&Tag>,
        parse: &dyn Fn(&str) -> Result<C, String>,
    ) -> Result<Vec<Instantiated<'a, C>>, FunctionError> {
        match self {
            Function::Plain(commands) => Ok(commands.iter().map(Instantiated::Compiled).collect()),
            Function::Macro { parameters, entries } => {
                let Some(Tag::Compound(values)) = arguments else {
                    return Err(FunctionError::MissingArguments);
                };
                let mut strings = Vec::with_capacity(parameters.len());
                for name in parameters {
                    let key = JavaString::Utf8(name.clone());
                    let value = values
                        .iter()
                        .rev()
                        .find(|(k, _)| *k == key)
                        .map(|(_, v)| v)
                        .ok_or_else(|| FunctionError::MissingArgument(name.clone()))?;
                    strings.push(stringify(value)?);
                }
                let mut out = Vec::with_capacity(entries.len());
                for entry in entries {
                    match entry {
                        MacroEntry::Plain(command) => out.push(Instantiated::Compiled(command)),
                        MacroEntry::Template { template, indices } => {
                            let values: Vec<String> = indices.iter().map(|i| strings[*i].clone()).collect();
                            let text = template.substitute(&values)?;
                            let command = parse(&text).map_err(|message| FunctionError::InstantiatedCommand { command: text.clone(), message })?;
                            out.push(Instantiated::Substituted(text, command));
                        }
                    }
                }
                Ok(out)
            }
        }
    }
}

/// One command of an instantiated function: compiled when the file was read, or substituted
/// (its text and the parsed command) when the macro was instantiated.
#[derive(Debug, Clone, PartialEq)]
pub enum Instantiated<'a, C> {
    Compiled(&'a C),
    Substituted(String, C),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn category(error: &FunctionError) -> String {
        match error {
            FunctionError::ContinuationAtEnd => "continuation".into(),
            FunctionError::DoubleSlash { line } => format!("double-slash {line}"),
            FunctionError::LeadingSlash { line } => format!("leading-slash {line}"),
            FunctionError::SlashOnly => "slash-only".into(),
            FunctionError::CommandTooLong => "too-long".into(),
            FunctionError::MacroWithoutVariables { line } => format!("no-variables {line}"),
            FunctionError::InvalidVariableName { name, line } => format!("bad-name '{name}' {line}"),
            FunctionError::UnterminatedVariable { line } => format!("unterminated {line}"),
            FunctionError::Command { line, .. } => format!("command {line}"),
            FunctionError::MissingArguments => "missing-arguments".into(),
            FunctionError::MissingArgument(name) => format!("missing-argument {name}"),
            FunctionError::InstantiatedCommand { command, .. } => format!("instantiated '{command}'"),
            FunctionError::Unsupported(what) => format!("unsupported {what}"),
        }
    }

    fn java_category(message: &str) -> String {
        let line = || message.split("on line ").nth(1).map(|rest| rest.split(|c: char| !c.is_ascii_digit()).next().unwrap().to_owned()).unwrap_or_default();
        if message.contains("Line continuation at end of file") {
            "continuation".into()
        } else if message.contains("use '#' not '//'") {
            format!("double-slash {}", line())
        } else if message.contains("Do not use a preceding forwards slash") {
            format!("leading-slash {}", line())
        } else if message.starts_with("StringIndexOutOfBoundsException") {
            "slash-only".into()
        } else if message.contains("Macro without variables") {
            format!("no-variables {}", line())
        } else if let Some(rest) = message.split("Invalid macro variable name '").nth(1) {
            format!("bad-name '{}' {}", rest.split('\'').next().unwrap(), line())
        } else if message.contains("Unterminated macro variable") {
            format!("unterminated {}", line())
        } else if message.contains("Whilst parsing command") {
            format!("command {}", line())
        } else if message.starts_with("Missing arguments to function") {
            "missing-arguments".into()
        } else if let Some(rest) = message.strip_prefix("Missing argument ") {
            format!("missing-argument {}", rest.strip_suffix(" to function test:f").unwrap())
        } else if let Some(rest) = message.split("Command '").nth(1) {
            format!("instantiated '{}'", rest.split("' caused error").next().unwrap())
        } else {
            format!("unknown {message}")
        }
    }

    /// Function files and macro instantiation against the original over the real 1.21 dispatcher
    /// (`fixtures/functions-vectors.json`); the dispatcher's verdict for each command string comes
    /// from the same run.
    #[test]
    fn functions_match_the_original() {
        let doc: Value = serde_json::from_str(include_str!("../../fixtures/functions-vectors.json")).unwrap();
        let parsed = doc["parsed"].as_object().unwrap();
        let parse = |text: &str| -> Result<String, String> {
            match parsed.get(text).and_then(Value::as_bool) {
                Some(true) => Ok(text.to_owned()),
                Some(false) => Err("rejected by the dispatcher".to_owned()),
                None => panic!("the oracle never parsed {text:?}"),
            }
        };
        let arguments: Vec<Option<Tag>> = doc["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().map(|text| crate::tag_parser::parse_compound(text).unwrap()))
            .collect();
        let mut unsupported = 0;
        for case in doc["cases"].as_array().unwrap() {
            let lines: Vec<String> = case["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect();
            let expect = &case["expect"];
            let ours = from_lines(&lines, &parse);
            if !expect["ok"].as_bool().unwrap() {
                let theirs = java_category(expect["error"].as_str().unwrap());
                assert_eq!(ours.as_ref().map_err(category).err(), Some(theirs), "{lines:?}");
                continue;
            }
            let function = ours.unwrap_or_else(|e| panic!("{lines:?}: {e}"));
            match (&function, expect["kind"].as_str().unwrap()) {
                (Function::Plain(commands), "plain") => {
                    let theirs: Vec<&str> = expect["commands"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
                    assert_eq!(commands.iter().map(String::as_str).collect::<Vec<_>>(), theirs, "{lines:?}");
                }
                (Function::Macro { parameters, entries }, "macro") => {
                    let theirs: Vec<&str> = expect["parameters"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
                    assert_eq!(parameters.iter().map(String::as_str).collect::<Vec<_>>(), theirs, "{lines:?}");
                    for (entry, theirs) in entries.iter().zip(expect["entries"].as_array().unwrap()) {
                        match entry {
                            MacroEntry::Plain(command) => assert_eq!(Some(command.as_str()), theirs["plain"].as_str(), "{lines:?}"),
                            MacroEntry::Template { template, indices } => {
                                assert_eq!(serde_json::json!(template.segments), theirs["segments"], "{lines:?}");
                                assert_eq!(serde_json::json!(template.variables), theirs["variables"], "{lines:?}");
                                assert_eq!(serde_json::json!(indices), theirs["indices"], "{lines:?}");
                            }
                        }
                    }
                    for (argument, theirs) in arguments.iter().zip(expect["instances"].as_array().unwrap()) {
                        match function.instantiate(argument.as_ref(), &parse) {
                            Err(FunctionError::Unsupported(_)) => unsupported += 1,
                            Err(error) => assert_eq!(Some(category(&error)), theirs["error"].as_str().map(java_category), "{lines:?} with {argument:?}"),
                            Ok(commands) => {
                                let ours: Vec<String> = commands
                                    .into_iter()
                                    .map(|c| match c {
                                        Instantiated::Compiled(text) => text.clone(),
                                        Instantiated::Substituted(text, _) => text,
                                    })
                                    .collect();
                                assert_eq!(serde_json::json!(ours), theirs["commands"], "{lines:?} with {argument:?}");
                            }
                        }
                    }
                }
                (ours, kind) => panic!("{lines:?}: ours {ours:?}, original {kind}"),
            }
        }
        eprintln!("functions: {unsupported} unsupported instantiations");
    }
}
