//! Java collection semantics the wire forms depend on.
//!
//! `CompoundTag` keeps its entries in a `java.util.HashMap<String, Tag>`, so the
//! order in which a compound is written to the network is the HashMap iteration
//! order of its keys, not insertion order. This module reproduces that order.

/// `String.hashCode()` over UTF-16 code units.
pub fn string_hash(text: &str) -> i32 {
    text.encode_utf16()
        .fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(i32::from(unit)))
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
    let hashes: Vec<u32> = keys.iter().map(|key| spread(string_hash(key))).collect();
    let mut capacity = 16usize;
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); capacity];
    for (index, hash) in hashes.iter().enumerate() {
        buckets[*hash as usize & (capacity - 1)].push(index);
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
    let path_ok = path
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '/'));
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
                &["text", "color", "bold", "extra", "italic", "underlined", "strikethrough", "obfuscated", "clickEvent", "hoverEvent", "insertion", "font"],
                &[9, 8, 1, 3, 5, 10, 0, 2, 6, 4, 7, 11],
            ),
            (&["translate", "with", "fallback"], &[1, 2, 0]),
            (&["selector", "separator"], &[0, 1]),
            (&["nbt", "interpret", "separator", "entity"], &[0, 1, 2, 3]),
            (&["score"], &[0]),
            (&["name", "objective"], &[0, 1]),
            (
                &["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p"],
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
}
