//! Java text semantics that Brigadier's messages and reader depend on.
//!
//! `Character.isWhitespace(char)` and `Double.toString` / `Float.toString` (the JDK 19+
//! shortest-representation algorithm, Java 21 here) are reproduced and compared with the
//! original JVM in `fixtures/commands-brigadier-vectors.json`.

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
