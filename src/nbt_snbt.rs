//! Java 1.21 StringTagVisitor and StringTag.quoteAndEscape text semantics.
//!
//! This pure visitor returns lossless UTF-16. It issues no CompoundTag history,
//! decoder capability or logger ownership. Floating branches require an actual
//! Java21 Float/Double.toString provider; Rust float Display is not a fallback.

use crate::nbt::{JavaString, Tag};
use anyhow::Result;
use std::{collections::BTreeMap, fmt};

/// Exact Java21 StringBuilder.append(float/double) text, without the SNBT
/// suffix. Preserve signed zero, NaN, infinities and Java's decimal selection
/// and exponent layout. Implement over the genuine shared numeric formatter;
/// an unproved Rust Display/LowerExp approximation is not this capability.
/// Formatting must not mutate the NBT input, draw RNG or emit log/world effects.
pub trait SnbtNumberFormatter {
    fn java_float_to_string(&mut self, value: f32) -> Result<JavaString>;
    fn java_double_to_string(&mut self, value: f64) -> Result<JavaString>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingSnbtNumberFormatter {
    Float { bits: u32 },
    Double { bits: u64 },
}
impl fmt::Display for MissingSnbtNumberFormatter {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Float { bits } => write!(output,
                "SNBT Float requires an actual Java21 number formatter (bits {bits:08x})"),
            Self::Double { bits } => write!(output,
                "SNBT Double requires an actual Java21 number formatter (bits {bits:016x})"),
        }
    }
}
impl std::error::Error for MissingSnbtNumberFormatter {}

struct NoFloatingPointText;
impl SnbtNumberFormatter for NoFloatingPointText {
    fn java_float_to_string(&mut self, value: f32) -> Result<JavaString> {
        Err(MissingSnbtNumberFormatter::Float { bits: value.to_bits() }.into())
    }
    fn java_double_to_string(&mut self, value: f64) -> Result<JavaString> {
        Err(MissingSnbtNumberFormatter::Double { bits: value.to_bits() }.into())
    }
}

/// Compact original StringTagVisitor output for the supplied immutable Tag.
/// Compound keys are distinct logical Java Strings, sorted by UTF-16 order;
/// duplicate occurrences in the raw Tag representation select the last value.
/// Source-decoded input can be supplied with SourceCompoundView::tag(). This
/// function neither checks nor creates that separate source identity/history.
pub fn to_snbt<N: SnbtNumberFormatter + ?Sized>(tag: &Tag, numbers: &mut N)
    -> Result<JavaString> {
    let mut output = Vec::new();
    write_tag(tag, numbers, &mut output)?;
    Ok(JavaString::from_utf16_lossless(output))
}

/// Genuine integer/string/container/array diagnostics without a floating
/// capability. Fail explicitly at a reached Float/Double; never substitute text
/// or emit a partial diagnostic. A shadowed duplicate value is not visited.
pub fn to_snbt_without_floating(tag: &Tag) -> Result<JavaString> {
    to_snbt(tag, &mut NoFloatingPointText)
}

/// StringTag.quoteAndEscape, source136–158 (`uw.b`). The first encountered
/// single/double quote chooses the opposite delimiter. Escape only backslash
/// and the chosen delimiter, leaving controls and all UTF-16 units unchanged.
pub fn quote_and_escape(value: &JavaString) -> JavaString {
    let mut output = Vec::new();
    write_quoted(value, &mut output);
    JavaString::from_utf16_lossless(output)
}

/// StringTagVisitor.handleEscape, source123–127: only a nonempty entire match
/// of [A-Za-z0-9._+-]+ is unquoted. String TAG VALUES are always quoted.
pub fn escape_key(value: &JavaString) -> JavaString {
    let mut output = Vec::new();
    write_key(value, &mut output);
    JavaString::from_utf16_lossless(output)
}

/// StringTagVisitor.visitEnd, source132–133. Current Tag has no End variant;
/// this returns the literal visitor text without constructing any NBT value.
pub fn end_tag_snbt() -> JavaString { "END".into() }

fn write_tag<N: SnbtNumberFormatter + ?Sized>(tag: &Tag, numbers: &mut N,
    output: &mut Vec<u16>,
) -> Result<()> {
    match tag {
        Tag::Byte(value) => { decimal(*value, output); output.push(b'b' as u16); }
        Tag::Short(value) => { decimal(*value, output); output.push(b's' as u16); }
        Tag::Int(value) => decimal(*value, output),
        Tag::Long(value) => { decimal(*value, output); output.push(b'L' as u16); }
        Tag::Float(value) => {
            output.extend(numbers.java_float_to_string(*value)?.iter_units());
            output.push(b'f' as u16);
        }
        Tag::Double(value) => {
            output.extend(numbers.java_double_to_string(*value)?.iter_units());
            output.push(b'd' as u16);
        }
        Tag::ByteArray(values) => {
            ascii("[B;", output);
            for (index, value) in values.iter().enumerate() {
                comma(index, output);
                decimal(*value, output);
                output.push(b'B' as u16);
            }
            output.push(b']' as u16);
        }
        Tag::String(value) => write_quoted(value, output),
        Tag::List { values, .. } => {
            output.push(b'[' as u16);
            for (index, value) in values.iter().enumerate() {
                comma(index, output);
                write_tag(value, numbers, output)?;
            }
            output.push(b']' as u16);
        }
        Tag::Compound(entries) => {
            // Source ux visits a sorted copy of the actual DISTINCT key set,
            // then CompoundTag.get for each key. This local map implements
            // precisely that sorted logical projection, NOT HashMap history.
            // JavaString::Ord compares UTF-16 units; later equal keys replace
            // the value, matching the raw decoder's logical duplicate policy.
            let mut logical = BTreeMap::new();
            for (key, value) in entries { logical.insert(key, value); }
            output.push(b'{' as u16);
            for (index, (key, value)) in logical.into_iter().enumerate() {
                comma(index, output);
                write_key(key, output);
                output.push(b':' as u16);
                write_tag(value, numbers, output)?;
            }
            output.push(b'}' as u16);
        }
        Tag::IntArray(values) => {
            ascii("[I;", output);
            for (index, value) in values.iter().enumerate() {
                comma(index, output);
                decimal(*value, output);
            }
            output.push(b']' as u16);
        }
        Tag::LongArray(values) => {
            ascii("[L;", output);
            for (index, value) in values.iter().enumerate() {
                comma(index, output);
                decimal(*value, output);
                output.push(b'L' as u16);
            }
            output.push(b']' as u16);
        }
    }
    Ok(())
}

fn write_quoted(value: &JavaString, output: &mut Vec<u16>) {
    let opening = output.len();
    output.push(0); // Source reserves its opening character, then replaces it.
    let mut quote = 0_u16;
    for unit in value.iter_units() {
        if unit == b'\\' as u16 {
            output.push(b'\\' as u16);
        } else if unit == b'"' as u16 || unit == b'\'' as u16 {
            if quote == 0 {
                quote = if unit == b'"' as u16 { b'\'' as u16 } else { b'"' as u16 };
            }
            if quote == unit { output.push(b'\\' as u16); }
        }
        output.push(unit);
    }
    if quote == 0 { quote = b'"' as u16; }
    output[opening] = quote;
    output.push(quote);
}

fn write_key(value: &JavaString, output: &mut Vec<u16>) {
    if !value.is_empty() && value.iter_units().all(|unit| matches!(unit,
        0x41..=0x5a | 0x61..=0x7a | 0x30..=0x39 | 0x2e | 0x5f | 0x2b | 0x2d)) {
        output.extend(value.iter_units());
    } else {
        write_quoted(value, output);
    }
}
fn comma(index: usize, output: &mut Vec<u16>) {
    if index != 0 { output.push(b',' as u16); }
}
fn ascii(value: &str, output: &mut Vec<u16>) { output.extend(value.encode_utf16()); }

// Only signed INTEGER tag/array branches call this helper. Their plain base10
// form has no locale, exponent or floating decimal-selection ambiguity.
fn decimal(value: impl fmt::Display, output: &mut Vec<u16>) {
    ascii(&value.to_string(), output);
}
