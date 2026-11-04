//! Java text semantics that Brigadier's messages and reader depend on.
//!
//! `Character.isWhitespace(char)` and `Double.toString` / `Float.toString` (the JDK 19+
//! shortest-representation algorithm, Java 21 here) are reproduced and compared with the
//! original JVM in `fixtures/commands-brigadier-vectors.json`. The case mapping, ordering,
//! hashing and lower-casing used by completion suggestions (stage 4) are compared in
//! `fixtures/commands-brigadier-suggest-vectors.json`.

/// `Character.isWhitespace(char)` for one UTF-16 code unit (Java 21 / Unicode 15).
///
/// Space separators except the non-breaking ones (U+00A0, U+2007, U+202F), line and
/// paragraph separators, and the ASCII controls HT, LF, VT, FF, CR and FS, GS, RS, US.
pub fn java_is_whitespace(unit: u16) -> bool {
    matches!(
        unit,
        0x09..=0x0D
            | 0x1C..=0x1F
            | 0x20
            | 0x1680
            | 0x2000..=0x2006
            | 0x2008..=0x200A
            | 0x2028
            | 0x2029
            | 0x205F
            | 0x3000
    )
}

/// `Character.toUpperCase(int codePoint)`: the simple (one-to-one) mapping.
///
/// Rust only exposes the full mappings; a code point whose full mapping is a single scalar
/// takes it, every other code point is kept, and `JAVA_CASE_EXCEPTIONS` lists the code points
/// where Java 21 (Unicode 15.0) differs. Every Unicode scalar value is compared with the JVM.
pub fn java_to_upper_code_point(code_point: u32) -> u32 {
    if has_no_java_mapping(code_point) {
        return code_point;
    }
    if let Some(&(_, upper, _)) = case_exception(code_point) {
        return upper;
    }
    single_scalar(char::from_u32(code_point).map(|c| c.to_uppercase().collect::<Vec<_>>()), code_point)
}

/// `Character.toLowerCase(int codePoint)`; see [`java_to_upper_code_point`].
pub fn java_to_lower_code_point(code_point: u32) -> u32 {
    if has_no_java_mapping(code_point) {
        return code_point;
    }
    if let Some(&(_, _, lower)) = case_exception(code_point) {
        return lower;
    }
    single_scalar(char::from_u32(code_point).map(|c| c.to_lowercase().collect::<Vec<_>>()), code_point)
}

/// `Character.toUpperCase(char)` for one UTF-16 code unit.
pub fn java_to_upper_char(unit: u16) -> u16 {
    java_to_upper_code_point(u32::from(unit)) as u16
}

/// `Character.toLowerCase(char)` for one UTF-16 code unit.
pub fn java_to_lower_char(unit: u16) -> u16 {
    java_to_lower_code_point(u32::from(unit)) as u16
}

fn single_scalar(mapped: Option<Vec<char>>, code_point: u32) -> u32 {
    match mapped.as_deref() {
        Some([single]) => u32::from(*single),
        _ => code_point,
    }
}

fn case_exception(code_point: u32) -> Option<&'static (u32, u32, u32)> {
    JAVA_CASE_EXCEPTIONS
        .binary_search_by_key(&code_point, |entry| entry.0)
        .ok()
        .map(|index| &JAVA_CASE_EXCEPTIONS[index])
}

fn has_no_java_mapping(code_point: u32) -> bool {
    JAVA_UNMAPPED.iter().any(|&(first, last)| (first..=last).contains(&code_point))
}

/// Inclusive code point ranges whose letters Rust's newer Unicode data maps (Latin and Beria
/// Erfe additions of Unicode 16) while Java 21, on Unicode 15.0, leaves them unchanged.
const JAVA_UNMAPPED: &[(u32, u32)] = &[(411, 411), (612, 612), (7305, 7306), (42955, 42959), (42962, 42965), (42970, 42972), (68944, 68965), (68976, 68997), (93856, 93880), (93883, 93907)];

/// `(code point, Character.toUpperCase, Character.toLowerCase)` where Java's simple mapping
/// is not what the derivation gives: `U+0130` (Java's simple lower case is `i`) and the Greek
/// letters whose only mapping in Rust is a multi-character one (the `ᾀ`..`ᾯ` and
/// `ᾳ`/`ῃ`/`ῳ` families). Sorted by code point.
const JAVA_CASE_EXCEPTIONS: &[(u32, u32, u32)] = &[
    (304, 304, 105),
    (8064, 8072, 8064),
    (8065, 8073, 8065),
    (8066, 8074, 8066),
    (8067, 8075, 8067),
    (8068, 8076, 8068),
    (8069, 8077, 8069),
    (8070, 8078, 8070),
    (8071, 8079, 8071),
    (8080, 8088, 8080),
    (8081, 8089, 8081),
    (8082, 8090, 8082),
    (8083, 8091, 8083),
    (8084, 8092, 8084),
    (8085, 8093, 8085),
    (8086, 8094, 8086),
    (8087, 8095, 8087),
    (8096, 8104, 8096),
    (8097, 8105, 8097),
    (8098, 8106, 8098),
    (8099, 8107, 8099),
    (8100, 8108, 8100),
    (8101, 8109, 8101),
    (8102, 8110, 8102),
    (8103, 8111, 8103),
    (8115, 8124, 8115),
    (8131, 8140, 8131),
    (8179, 8188, 8179),
];

fn code_point_at(units: &[u16], index: usize) -> (u32, usize) {
    let first = units[index];
    if (0xD800..0xDC00).contains(&first) {
        if let Some(&second) = units.get(index + 1) {
            if (0xDC00..0xE000).contains(&second) {
                return (0x10000 + ((u32::from(first) - 0xD800) << 10) + (u32::from(second) - 0xDC00), 2);
            }
        }
    }
    (u32::from(first), 1)
}

/// `String.compareToIgnoreCase` (JDK 21).
///
/// Which JDK routine runs depends on the compact-string coder of the operands: when either
/// string is Latin-1 (every unit `<= 0xFF`) the comparison walks UTF-16 units, when both are
/// UTF-16 it walks code points (`StringUTF16.compareToCI`). In both walks the first pair that
/// differs after upper-casing and then lower-casing decides by difference, otherwise the
/// UTF-16 length difference does. Compared with the JVM in the suggestion fixture.
pub fn java_compare_to_ignore_case(a: &[u16], b: &[u16]) -> i32 {
    let latin1 = |units: &[u16]| units.iter().all(|unit| *unit <= 0xFF);
    let by_code_point = !latin1(a) && !latin1(b);
    let limit = a.len().min(b.len());
    let mut k = 0;
    while k < limit {
        let ((mut c1, width), (mut c2, _)) = if by_code_point {
            (code_point_at(a, k), code_point_at(b, k))
        } else {
            ((u32::from(a[k]), 1), (u32::from(b[k]), 1))
        };
        if c1 != c2 {
            c1 = java_to_upper_code_point(c1);
            c2 = java_to_upper_code_point(c2);
            if c1 != c2 {
                c1 = java_to_lower_code_point(c1);
                c2 = java_to_lower_code_point(c2);
                if c1 != c2 {
                    return c1 as i32 - c2 as i32;
                }
            }
        }
        k += width;
    }
    a.len() as i32 - b.len() as i32
}

/// `String.compareTo`: the first differing UTF-16 unit decides by difference, otherwise the
/// length difference does.
pub fn java_compare_to(a: &[u16], b: &[u16]) -> i32 {
    let limit = a.len().min(b.len());
    for k in 0..limit {
        if a[k] != b[k] {
            return i32::from(a[k]) - i32::from(b[k]);
        }
    }
    a.len() as i32 - b.len() as i32
}

/// `String.hashCode()` over UTF-16 code units.
pub fn java_string_hash(units: &[u16]) -> i32 {
    units.iter().fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(i32::from(*unit)))
}

/// `String.toLowerCase(Locale.ROOT)`. Rust implements the same full Unicode mapping
/// including the final-sigma rule and `U+0130`; the Unicode version of the running
/// toolchain, not Java 21's 15.0, decides characters added later.
pub fn java_to_lower_case_root(text: &str) -> String {
    text.to_lowercase()
}

/// `Double.toString(double)`.
pub fn java_double_to_string(value: f64) -> String {
    if let Some(text) = special(value.is_nan(), value.is_infinite(), value == 0.0, value.is_sign_negative()) {
        return text.into();
    }
    let (digits, exponent) = java_digits(value, value, value.is_sign_negative());
    layout(value.is_sign_negative(), &digits, exponent)
}

/// `Float.toString(float)`.
pub fn java_float_to_string(value: f32) -> String {
    if let Some(text) = special(value.is_nan(), value.is_infinite(), value == 0.0, value.is_sign_negative()) {
        return text.into();
    }
    let (digits, exponent) = java_digits(value, f64::from(value), value.is_sign_negative());
    layout(value.is_sign_negative(), &digits, exponent)
}

fn special(nan: bool, infinite: bool, zero: bool, negative: bool) -> Option<&'static str> {
    if nan {
        Some("NaN")
    } else if infinite {
        Some(if negative { "-Infinity" } else { "Infinity" })
    } else if zero {
        Some(if negative { "-0.0" } else { "0.0" })
    } else {
        None
    }
}

/// The significant digits and decimal exponent Java's `Double/Float.toString` selects.
///
/// JDK 19+ prints the shortest decimal that rounds back to the value. Two details differ
/// from Rust's shortest output and are applied here: a one-digit result is replaced by the
/// closest two-digit decimal (`4.9E-324`, not `5.0E-324`), and when two shortest candidates
/// are exactly equidistant from the value the one with an even last digit wins.
fn java_digits<T>(value: T, exact: f64, negative: bool) -> (String, i32)
where
    T: Copy + PartialEq + std::str::FromStr + std::fmt::LowerExp,
{
    let (mut digits, mut exponent) = split_scientific(&format!("{value:e}"));
    if digits.len() == 1 {
        let two = format!("{value:.1e}");
        if two.parse::<T>().ok() == Some(value) {
            let (two_digits, two_exponent) = split_scientific(&two);
            digits = two_digits;
            exponent = two_exponent;
        }
    }
    // Exact binary-to-decimal expansion of the (unsigned) value, 801 significant digits.
    let expanded = format!("{:.800e}", exact.abs());
    let mantissa: String = expanded
        .split('e')
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| *c != '.')
        .collect();
    let n = digits.len();
    let bytes = mantissa.as_bytes();
    if bytes.len() > n && bytes[n] == b'5' && bytes[n + 1..].iter().all(|b| *b == b'0') {
        let down = mantissa[..n].to_string();
        if let Some(up) = increment_digits(&down) {
            let candidate_exponent = exponent;
            let round_trips = |candidate: &str| {
                let sign = if negative { "-" } else { "" };
                let text = format!("{sign}{}.{}e{}", &candidate[..1], &candidate[1..], candidate_exponent);
                text.parse::<T>().ok() == Some(value)
            };
            if round_trips(&down) && round_trips(&up) {
                let chosen = if down.as_bytes()[n - 1] % 2 == 0 { down } else { up };
                digits = chosen.trim_end_matches('0').to_string();
                if digits.is_empty() {
                    digits.push('0');
                }
            }
        }
    }
    (digits, exponent)
}

/// Add one unit in the last place; `None` when the digit string would grow (all nines).
fn increment_digits(digits: &str) -> Option<String> {
    let mut bytes = digits.as_bytes().to_vec();
    for byte in bytes.iter_mut().rev() {
        if *byte == b'9' {
            *byte = b'0';
        } else {
            *byte += 1;
            return String::from_utf8(bytes).ok();
        }
    }
    None
}

/// Split Rust's `d.ddde-5` text into its significant digits and decimal exponent.
fn split_scientific(text: &str) -> (String, i32) {
    let unsigned = text.trim_start_matches('-');
    let (mantissa, exponent) = unsigned.split_once('e').expect("scientific notation");
    let mut digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    (digits, exponent.parse().expect("decimal exponent"))
}

/// Java's layout: plain decimal for 10^-3 <= |v| < 10^7, otherwise `d.dddE[-]n`.
fn layout(negative: bool, digits: &str, exponent: i32) -> String {
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if (-3..7).contains(&exponent) {
        if exponent < 0 {
            out.push_str("0.");
            for _ in 0..(-exponent - 1) {
                out.push('0');
            }
            out.push_str(digits);
        } else {
            let integer_len = exponent as usize + 1;
            if digits.len() <= integer_len {
                out.push_str(digits);
                for _ in 0..(integer_len - digits.len()) {
                    out.push('0');
                }
                out.push_str(".0");
            } else {
                out.push_str(&digits[..integer_len]);
                out.push('.');
                out.push_str(&digits[integer_len..]);
            }
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        if digits.len() > 1 {
            out.push_str(&digits[1..]);
        } else {
            out.push('0');
        }
        out.push('E');
        out.push_str(&exponent.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_number_text_matches_known_jdk21_values() {
        assert_eq!(java_double_to_string(1.0), "1.0");
        assert_eq!(java_double_to_string(100.0), "100.0");
        assert_eq!(java_double_to_string(1.0e7), "1.0E7");
        assert_eq!(java_double_to_string(1234567.0), "1234567.0");
        assert_eq!(java_double_to_string(0.001), "0.001");
        assert_eq!(java_double_to_string(0.0001), "1.0E-4");
        assert_eq!(java_double_to_string(-0.0), "-0.0");
        assert_eq!(java_double_to_string(f64::MIN_POSITIVE * f64::EPSILON), "4.9E-324");
        assert_eq!(java_float_to_string(1.0e-5), "1.0E-5");
        assert_eq!(java_float_to_string(0.005), "0.005");
        assert_eq!(java_float_to_string(3.4028235e38), "3.4028235E38");
    }
}
