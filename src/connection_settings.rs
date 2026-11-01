//! Typed dedicated-server connection properties from Minecraft Java 1.21.
//!
//! The host supplies already-decoded Java Properties values. File decoding,
//! persistence and explicit native command-line overrides remain with their
//! existing owners; this module does not open or rewrite `server.properties`.

pub const RATE_LIMIT_PROPERTY: &str = "rate-limit";
pub const COMPRESSION_THRESHOLD_PROPERTY: &str = "network-compression-threshold";
pub const ENFORCE_SECURE_PROFILE_PROPERTY: &str = "enforce-secure-profile";

/// Values read by `DedicatedServerProperties` using `Settings.get`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DedicatedConnectionSettings {
    /// A nonpositive value selects ordinary Connection, without rate kicks.
    pub rate_limit: i32,
    /// A negative value disables compression; zero remains a valid threshold.
    pub network_compression_threshold: i32,
    /// Configured value only. Effective enforcement also depends on the actual
    /// online-mode and trusted-service-key owners.
    pub enforce_secure_profile: bool,
}

impl Default for DedicatedConnectionSettings {
    fn default() -> Self {
        Self {
            rate_limit: 0,
            network_compression_threshold: 256,
            enforce_secure_profile: true,
        }
    }
}

impl DedicatedConnectionSettings {
    /// Read each supported property once, in its source-constructor order.
    /// Missing/invalid integers use the source default. A present boolean is
    /// true only for an untrimmed case-insensitive `true`, even if malformed.
    pub fn from_lookup(mut lookup: impl FnMut(&'static str) -> Option<String>) -> Self {
        Self::from_values(
            lookup(RATE_LIMIT_PROPERTY).as_deref(),
            lookup(COMPRESSION_THRESHOLD_PROPERTY).as_deref(),
            lookup(ENFORCE_SECURE_PROFILE_PROPERTY).as_deref(),
        )
    }

    /// Preserve the existing file parser's I/O and invalid-escape failures.
    /// Property-file errors are not invalid integer values and must not be
    /// silently converted into defaults by the host adapter.
    pub fn try_from_lookup<E>(
        mut lookup: impl FnMut(&'static str) -> Result<Option<String>, E>,
    ) -> Result<Self, E> {
        Ok(Self::from_values(
            lookup(RATE_LIMIT_PROPERTY)?.as_deref(),
            lookup(COMPRESSION_THRESHOLD_PROPERTY)?.as_deref(),
            lookup(ENFORCE_SECURE_PROFILE_PROPERTY)?.as_deref(),
        ))
    }

    pub fn from_values(
        rate_limit: Option<&str>,
        network_compression_threshold: Option<&str>,
        enforce_secure_profile: Option<&str>,
    ) -> Self {
        let defaults = Self::default();
        Self {
            rate_limit: rate_limit
                .and_then(parse_java_i32)
                .unwrap_or(defaults.rate_limit),
            network_compression_threshold: network_compression_threshold
                .and_then(parse_java_i32)
                .unwrap_or(defaults.network_compression_threshold),
            enforce_secure_profile: enforce_secure_profile
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(defaults.enforce_secure_profile),
        }
    }

    /// `Settings.get` stores the canonical parsed/default string in its actual
    /// Properties map. Returning those strings lets that map's owner preserve
    /// the same normalization without this module inventing a file writer.
    pub fn canonical_property_values(self) -> [(&'static str, String); 3] {
        [
            (RATE_LIMIT_PROPERTY, self.rate_limit.to_string()),
            (
                COMPRESSION_THRESHOLD_PROPERTY,
                self.network_compression_threshold.to_string(),
            ),
            (
                ENFORCE_SECURE_PROFILE_PROPERTY,
                self.enforce_secure_profile.to_string(),
            ),
        ]
    }
}

/// `Integer.parseInt(String)` consumes UTF-16 chars with radix ten. In
/// particular, it accepts BMP decimal digits, ASCII signs and leading zeros,
/// rejects whitespace, supplementary digits and overflow, and accepts MIN_VALUE.
fn parse_java_i32(text: &str) -> Option<i32> {
    let mut chars = text.chars();
    let first = chars.next()?;
    let (negative, initial_digit) = match first {
        '-' => (true, None),
        '+' => (false, None),
        digit => (false, Some(java_decimal_digit(digit)?)),
    };
    let limit = if negative {
        2_147_483_648u32
    } else {
        i32::MAX as u32
    };
    let mut magnitude = initial_digit.unwrap_or(0);
    let mut has_digit = initial_digit.is_some();
    for ch in chars {
        magnitude = magnitude
            .checked_mul(10)?
            .checked_add(java_decimal_digit(ch)?)?;
        if magnitude > limit {
            return None;
        }
        has_digit = true;
    }
    if !has_digit {
        return None;
    }
    Some(if negative {
        (-(magnitude as i64)) as i32
    } else {
        magnitude as i32
    })
}

/// BMP decimal ranges accepted by JDK21 `Character.digit(char, 10)`.
/// Supplementary Unicode digits occupy surrogate pairs and are rejected by
/// the source's char-by-char Integer parser rather than treated as one digit.
fn java_decimal_digit(ch: char) -> Option<u32> {
    const ZEROES: [u32; 37] = [
        0x0030, 0x0660, 0x06f0, 0x07c0, 0x0966, 0x09e6, 0x0a66, 0x0ae6,
        0x0b66, 0x0be6, 0x0c66, 0x0ce6, 0x0d66, 0x0de6, 0x0e50, 0x0ed0,
        0x0f20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80,
        0x1a90, 0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900,
        0xa9d0, 0xa9f0, 0xaa50, 0xabf0, 0xff10,
    ];
    let code = ch as u32;
    ZEROES
        .into_iter()
        .find_map(|zero| code.checked_sub(zero).filter(|digit| *digit < 10))
}
