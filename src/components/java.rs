//! Java collection semantics the wire forms depend on.
//!
//! `CompoundTag` keeps its entries in a `java.util.HashMap<String, Tag>`, so the
//! order in which a compound is written to the network is the HashMap iteration
//! order of its keys, not insertion order. This module reproduces that order.

/// `String.hashCode()` over UTF-16 code units.
pub fn string_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

/// `HashMap.hash(key)`: the hash code with its high half folded in.
fn spread(hash: i32) -> u32 {
    let hash = hash as u32;
    hash ^ (hash >> 16)
}

/// Indices of `keys` (distinct, in insertion order) in the order a default
/// `java.util.HashMap` iterates them: buckets by ascending index, each bucket in
/// insertion order, with the table doubling from 16 once the size passes 75%.
///
/// Buckets are never treeified here: that needs eight colliding keys in one
/// bucket of a table of at least 64 entries, which a text component never has.
pub fn hashmap_iteration_order(keys: &[&str]) -> Vec<usize> {
    hashmap_iteration_order_in(keys, 16)
}

/// As [`hashmap_iteration_order`] for a map whose table already has `table_size` buckets (a
/// `HashMap` never shrinks, so a set that once held many members keeps the larger table).
pub fn hashmap_iteration_order_in(keys: &[&str], table_size: usize) -> Vec<usize> {
    let hashes: Vec<i32> = keys.iter().map(|key| string_hash(key)).collect();
    hashmap_order_by_hash(&hashes, table_size.max(16))
}

/// The general form: indices of entries with the given `hashCode()`s (distinct keys, in insertion
/// order) in the order a `java.util.HashMap` whose table starts with `table_size` buckets (a power
/// of two; `HashMap(3)` starts at 4) iterates them.
pub fn hashmap_order_by_hash(hash_codes: &[i32], table_size: usize) -> Vec<usize> {
    hashmap_order_with(hash_codes, table_size, false)
}

/// As [`hashmap_order_by_hash`], for a map filled with `computeIfAbsent`: unlike `put`, which
/// appends to a bucket's chain, `HashMap.computeIfAbsent` links the new node in *front* of it, so
/// colliding keys iterate newest first.
pub fn hashmap_order_with(hash_codes: &[i32], table_size: usize, prepend: bool) -> Vec<usize> {
    let hashes: Vec<u32> = hash_codes.iter().map(|hash| spread(*hash)).collect();
    let mut capacity = table_size.max(1).next_power_of_two();
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); capacity];
    for (index, hash) in hashes.iter().enumerate() {
        let bucket = &mut buckets[*hash as usize & (capacity - 1)];
        if prepend {
            bucket.insert(0, index);
        } else {
            bucket.push(index);
        }
        if index + 1 > capacity * 3 / 4 {
            // `resize()`: the split keeps relative order, so re-bucketing in
            // iteration order is equivalent.
            capacity *= 2;
            let mut grown: Vec<Vec<usize>> = vec![Vec::new(); capacity];
            for bucket in &buckets {
                for entry in bucket {
                    grown[hashes[*entry] as usize & (capacity - 1)].push(*entry);
                }
            }
            buckets = grown;
        }
    }
    buckets.into_iter().flatten().collect()
}

/// `Number.intValue()` of a Gson `LazilyParsedNumber` (what `Codec.INT` and `JsonOps.getNumberValue`
/// callers see): integers wrap to 32 bits, decimals go through `BigDecimal.intValue()` (truncate,
/// then the low 32 bits).
///
/// `serde_json` keeps a decimal as the nearest `f64`, so the low bits of a value above 2^53 can
/// differ from the original's exact decimal (`1E30` is 2^30 there and 0 here). Above 2^127 both are
/// 0 for every exponent form, and that is what is returned.
pub fn json_number_int_value(number: &serde_json::Number) -> i32 {
    if let Some(value) = number.as_i64() {
        value as i32
    } else if let Some(value) = number.as_u64() {
        value as i32
    } else {
        number.as_f64().map_or(0, |value| {
            if value.abs() >= 2f64.powi(127) {
                0
            } else {
                value.trunc() as i128 as i32
            }
        })
    }
}

/// `ResourceLocation.hashCode()` of `namespace:path`.
pub fn resource_location_hash(id: &str) -> i32 {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    string_hash(namespace).wrapping_mul(31).wrapping_add(string_hash(path))
}

/// `NumericTag.getAsInt` (`CompoundTag.getInt`, `contains(key, 99)`): any numeric tag, with Java's
/// narrowing (`long` truncates, `float`/`double` use `Mth.floor`).
pub fn numeric_as_int(tag: &crate::nbt::Tag) -> Option<i32> {
    use crate::nbt::Tag;
    Some(match tag {
        Tag::Byte(n) => i32::from(*n),
        Tag::Short(n) => i32::from(*n),
        Tag::Int(n) => *n,
        Tag::Long(n) => *n as i32,
        Tag::Float(n) => {
            let truncated = *n as i32;
            if *n < truncated as f32 {
                truncated.wrapping_sub(1)
            } else {
                truncated
            }
        }
        Tag::Double(n) => {
            let truncated = *n as i32;
            if *n < f64::from(truncated) {
                truncated.wrapping_sub(1)
            } else {
                truncated
            }
        }
        _ => return None,
    })
}

/// `Number.longValue()` of a numeric tag as `DynamicOps.getLongStream` reads it (`float` and
/// `double` truncate toward zero, saturating).
pub fn numeric_as_long(tag: &crate::nbt::Tag) -> Option<i64> {
    use crate::nbt::Tag;
    Some(match tag {
        Tag::Byte(n) => i64::from(*n),
        Tag::Short(n) => i64::from(*n),
        Tag::Int(n) => i64::from(*n),
        Tag::Long(n) => *n,
        Tag::Float(n) => *n as i64,
        Tag::Double(n) => *n as i64,
        _ => return None,
    })
}

/// `Double.toString` (JDK 19+, shortest round-trip digits): decimal notation for magnitudes in
/// `[1e-3, 1e7)` with at least one digit after the point, otherwise `d.dddE±n`.
pub fn double_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_owned();
    }
    let mut digits = format!("{value:e}");
    // A one-digit shortest form is widened to the closest two-digit decimal that still reads back
    // as the same value (`Double.MIN_VALUE` prints as 4.9E-324, not 5E-324).
    if !digits.split('e').next().unwrap_or("").contains('.') {
        let two = format!("{value:.1e}");
        if two.parse::<f64>() == Ok(value) {
            digits = two;
        }
    }
    java_decimal(&digits, value.abs() >= 1e-3 && value.abs() < 1e7)
}

/// `Float.toString`: as [`double_to_string`] over the shortest `f32` digits.
pub fn float_to_string(value: f32) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_owned();
    }
    let mut digits = format!("{value:e}");
    if !digits.split('e').next().unwrap_or("").contains('.') {
        let two = format!("{value:.1e}");
        if two.parse::<f32>() == Ok(value) {
            digits = two;
        }
    }
    java_decimal(&digits, value.abs() >= 1e-3 && value.abs() < 1e7)
}

/// Re-formats Rust's `d.ddde±n` shortest representation the way `Double.toString` prints it.
fn java_decimal(scientific: &str, plain: bool) -> String {
    let (mantissa, exponent) = scientific.split_once('e').expect("exponent");
    let exponent: i32 = exponent.parse().expect("exponent digits");
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let digits = match digits.trim_end_matches('0') {
        "" => "0".to_owned(),
        trimmed => trimmed.to_owned(),
    };
    if !plain {
        let fraction = if digits.len() > 1 { &digits[1..] } else { "0" };
        return format!("{sign}{}.{fraction}E{exponent}", &digits[..1]);
    }
    if exponent >= 0 {
        let integer_len = exponent as usize + 1;
        if digits.len() <= integer_len {
            format!("{sign}{digits}{}.0", "0".repeat(integer_len - digits.len()))
        } else {
            format!(
                "{sign}{}.{}",
                &digits[..integer_len],
                &digits[integer_len..]
            )
        }
    } else {
        format!("{sign}0.{}{digits}", "0".repeat((-exponent - 1) as usize))
    }
}

/// `ResourceLocation.parse` / `ResourceLocation.CODEC` (`bySeparator(text, ':')`): the first `:`
/// splits namespace and path, an empty or missing namespace means `minecraft`, an empty path is
/// legal. Returns the canonical `namespace:path`, or `None` where Java throws or rejects.
pub fn resource_location(text: &str) -> Option<String> {
    let (namespace, path) = match text.split_once(':') {
        Some(("", path)) => ("minecraft", path),
        Some((namespace, path)) => (namespace, path),
        None => ("minecraft", text),
    };
    let namespace_ok = namespace
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.'));
    let path_ok = path.chars().all(|c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '/')
    });
    (namespace_ok && path_ok).then(|| format!("{namespace}:{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected orders printed by a real `java.util.HashMap<String,Integer>` (JDK 21)
    /// filled in the listed insertion order.
    #[test]
    fn hashmap_order_matches_the_jvm() {
        let cases: [(&[&str], &[usize]); 10] = [
            (
                &[
                    "text",
                    "color",
                    "bold",
                    "extra",
                    "italic",
                    "underlined",
                    "strikethrough",
                    "obfuscated",
                    "clickEvent",
                    "hoverEvent",
                    "insertion",
                    "font",
                ],
                &[9, 8, 1, 3, 5, 10, 0, 2, 6, 4, 7, 11],
            ),
            (&["translate", "with", "fallback"], &[1, 2, 0]),
            (&["selector", "separator"], &[0, 1]),
            (&["nbt", "interpret", "separator", "entity"], &[0, 1, 2, 3]),
            (&["score"], &[0]),
            (&["name", "objective"], &[0, 1]),
            (
                &[
                    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
                ],
                &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            ),
            (&["keybind"], &[0]),
            (&["action", "value"], &[0, 1]),
            (&["action", "contents"], &[1, 0]),
        ];
        for (keys, expected) in cases {
            assert_eq!(hashmap_iteration_order(keys), expected, "{keys:?}");
        }
        assert_eq!(string_hash("hello"), 99_162_322);
        assert_eq!(string_hash("é𝄞"), 233 * 31 * 31 + 0xD834 * 31 + 0xDD1E);
    }

    #[test]
    fn float_and_double_text_follows_java() {
        for (value, expected) in [
            (1.0f32, "1.0"),
            (0.1, "0.1"),
            (100.0, "100.0"),
            (1.5e7, "1.5E7"),
            (9_999_999.0, "9999999.0"),
            (1e7, "1.0E7"),
            (0.001, "0.001"),
            (0.0009, "9.0E-4"),
            (1.0e-5, "1.0E-5"),
            (123_456_789.0, "1.2345679E8"),
            (f32::MAX, "3.4028235E38"),
            (f32::from_bits(1), "1.4E-45"),
            (-2.5, "-2.5"),
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (f32::NAN, "NaN"),
            (f32::INFINITY, "Infinity"),
        ] {
            assert_eq!(float_to_string(value), expected, "{value:e}");
        }
        for (value, expected) in [
            (1.0f64, "1.0"),
            (0.1, "0.1"),
            (1e7, "1.0E7"),
            (1.0e-5, "1.0E-5"),
            (4.9e-324, "4.9E-324"),
            (f64::MAX, "1.7976931348623157E308"),
            (123_456.789, "123456.789"),
            (3.14159, "3.14159"),
            (1.0 / 3.0, "0.3333333333333333"),
        ] {
            assert_eq!(double_to_string(value), expected, "{value:e}");
        }
    }
}
