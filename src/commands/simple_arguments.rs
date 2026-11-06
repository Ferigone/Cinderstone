//! Simple Minecraft argument types that need no other subsystem: `TimeArgument`,
//! `UuidArgument`, `EntityAnchorArgument`, `AngleArgument` and `SwizzleArgument`.
//!
//! Ported from the unchanged classes of the pinned 1.21 server, read with direct `javap`
//! inspection of the jar (remapped with the official mappings, kept outside git) and compared with
//! the executed originals in `fixtures/commands-mc-arguments-vectors.json`; their `ArgumentTypeInfos`
//! registry ids and templates are compared with the original's own `ClientboundCommandsPacket` in
//! `fixtures/commands-brigadier-tree-packet-vectors.json`. Suggestions are not part of this stage.

use super::arguments::{ArgumentNetworkInfo, ArgumentType};
use super::coordinates::{CoordinateSource, WorldCoordinate};
use super::exception::{built_in, CommandArg, CommandSyntaxException};
use super::string_reader::StringReader;

/// `ArgumentTypeInfos` registry ids (1.21 registration order).
pub mod registry_ids {
    pub const ANGLE: i32 = 27;
    pub const SWIZZLE: i32 = 31;
    pub const ENTITY_ANCHOR: i32 = 37;
    pub const TIME: i32 = 42;
    pub const UUID: i32 = 53;
}

const SPACE: u16 = b' ' as u16;

fn singleton(registry_id: i32) -> Option<ArgumentNetworkInfo> {
    Some(ArgumentNetworkInfo {
        registry_id,
        template: Vec::new(),
    })
}

/// `Math.round(float)`: `floor(a + 1/2)`, saturating, NaN to zero. The sum is exact in `f64`.
fn java_round(value: f32) -> i32 {
    (f64::from(value) + 0.5).floor() as i32
}

/// `TimeArgument.time(minimum)`: a float with a `d`, `s` or `t` (or no) unit, in ticks.
#[derive(Clone, Copy, Debug)]
pub struct TimeArgumentType {
    pub minimum: i32,
}

impl TimeArgumentType {
    /// `time()`: a minimum of zero ticks.
    pub fn new() -> Self {
        Self { minimum: 0 }
    }

    pub fn with_minimum(minimum: i32) -> Self {
        Self { minimum }
    }
}

impl Default for TimeArgumentType {
    fn default() -> Self {
        Self::new()
    }
}

impl ArgumentType for TimeArgumentType {
    type Value = i32;

    fn parse(&self, reader: &mut StringReader) -> Result<i32, CommandSyntaxException> {
        let value = reader.read_float()?;
        let unit = reader.read_unquoted_string();
        let multiplier: i32 = match unit.as_str() {
            "d" => 24000,
            "s" => 20,
            "t" | "" => 1,
            _ => return Err(built_in::simple("argument.time.invalid_unit", reader.units(), reader.cursor())),
        };
        let ticks = java_round(value * multiplier as f32);
        if ticks < self.minimum {
            return Err(built_in::range(
                "argument.time.tick_count_too_low",
                CommandArg::Integer(self.minimum),
                CommandArg::Integer(ticks),
                reader.units(),
                reader.cursor(),
            ));
        }
        Ok(ticks)
    }

    /// `TimeArgument.Info`: the minimum as a big-endian `int`.
    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        Some(ArgumentNetworkInfo {
            registry_id: registry_ids::TIME,
            template: self.minimum.to_be_bytes().to_vec(),
        })
    }

    fn list_suggestions<S>(
        &self,
        _context: &super::dispatcher::CommandContext<S>,
        builder: &mut super::suggestion::SuggestionsBuilder,
    ) -> Result<super::suggestion::Suggestions, CommandSyntaxException> {
        let mut reader = StringReader::new(&builder.remaining());
        if reader.read_float().is_err() { return Ok(super::suggestion::Suggestions::empty()); }
        let mut offset = builder.create_offset(builder.start() + reader.cursor())
            .expect("float cursor stays within the remaining input");
        for unit in ["d", "s", "t"] {
            if unit.encode_utf16().collect::<Vec<_>>().starts_with(offset.remaining_lower_case_units()) {
                offset.suggest(unit);
            }
        }
        Ok(offset.build())
    }
}

/// A `java.util.UUID`: the two signed 64-bit halves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JavaUuid {
    pub most_significant_bits: i64,
    pub least_significant_bits: i64,
}

/// `Long.parseLong(text, begin, end, 16)` for text made only of hex digits: an empty range or a value
/// above `Long.MAX_VALUE` fails (leading zeros are free).
fn parse_hex_long(digits: &[u16]) -> Option<i64> {
    if digits.is_empty() {
        return None;
    }
    let mut value: i64 = 0;
    for unit in digits {
        let digit = char::from_u32(u32::from(*unit))?.to_digit(16)?;
        value = value.checked_mul(16)?.checked_add(i64::from(digit))?;
    }
    Some(value)
}

/// `UUID.fromString` (JDK 21) for the characters `[-A-Fa-f0-9]`. The 36-character fast path gives the
/// same value as the general path, which is used for every length: five dash-separated hex numbers
/// that each fit `Long.parseLong`, whose high bits are masked off.
pub fn java_uuid_from_string(text: &[u16]) -> Option<JavaUuid> {
    if text.len() > 36 {
        return None;
    }
    let dash = b'-' as u16;
    let mut dashes = Vec::new();
    for (index, unit) in text.iter().enumerate() {
        if *unit == dash {
            dashes.push(index);
        }
    }
    if dashes.len() != 4 {
        return None;
    }
    let (d1, d2, d3, d4) = (dashes[0], dashes[1], dashes[2], dashes[3]);
    let part = |begin: usize, end: usize| parse_hex_long(&text[begin..end]);
    let mut most = part(0, d1)? & 0xffff_ffff;
    most <<= 16;
    most |= part(d1 + 1, d2)? & 0xffff;
    most <<= 16;
    most |= part(d2 + 1, d3)? & 0xffff;
    let mut least = part(d3 + 1, d4)? & 0xffff;
    least <<= 48;
    least |= part(d4 + 1, text.len())? & 0xffff_ffff_ffff;
    Some(JavaUuid {
        most_significant_bits: most,
        least_significant_bits: least,
    })
}

/// `UuidArgument.uuid()`: the longest run of hex digits and dashes, which must be a UUID.
#[derive(Clone, Copy, Debug, Default)]
pub struct UuidArgumentType;

impl ArgumentType for UuidArgumentType {
    type Value = JavaUuid;

    fn parse(&self, reader: &mut StringReader) -> Result<JavaUuid, CommandSyntaxException> {
        let start = reader.cursor();
        let allowed = |unit: u16| {
            unit == b'-' as u16
                || (b'0' as u16..=b'9' as u16).contains(&unit)
                || (b'a' as u16..=b'f' as u16).contains(&unit)
                || (b'A' as u16..=b'F' as u16).contains(&unit)
        };
        let end = reader.units()[start..]
            .iter()
            .position(|unit| !allowed(*unit))
            .map_or(reader.units().len(), |offset| start + offset);
        if end > start {
            if let Some(uuid) = java_uuid_from_string(&reader.units()[start..end]) {
                reader.set_cursor(end);
                return Ok(uuid);
            }
        }
        // The original does not move the cursor before failing.
        Err(built_in::simple("argument.uuid.invalid", reader.units(), reader.cursor()))
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        singleton(registry_ids::UUID)
    }
}

/// `EntityAnchorArgument.Anchor`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityAnchor {
    Feet,
    Eyes,
}

/// `EntityAnchorArgument.anchor()`: `feet` or `eyes`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EntityAnchorArgumentType;

impl ArgumentType for EntityAnchorArgumentType {
    type Value = EntityAnchor;

    fn parse(&self, reader: &mut StringReader) -> Result<EntityAnchor, CommandSyntaxException> {
        let start = reader.cursor();
        let name = reader.read_unquoted_string();
        match name.as_str() {
            "feet" => Ok(EntityAnchor::Feet),
            "eyes" => Ok(EntityAnchor::Eyes),
            _ => {
                reader.set_cursor(start);
                Err(built_in::string_arg("argument.anchor.invalid", name, reader.units(), reader.cursor()))
            }
        }
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        singleton(registry_ids::ENTITY_ANCHOR)
    }
}

/// `Mth.wrapDegrees(float)`.
fn wrap_degrees(value: f32) -> f32 {
    let mut wrapped = value % 360.0;
    if wrapped >= 180.0 {
        wrapped -= 360.0;
    }
    if wrapped < -180.0 {
        wrapped += 360.0;
    }
    wrapped
}

/// `AngleArgument.SingleAngle`: degrees, absolute or relative (`~`) to the source's yaw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SingleAngle {
    angle: f32,
    relative: bool,
}

impl SingleAngle {
    pub fn is_relative(&self) -> bool {
        self.relative
    }

    /// `getAngle(source)`: wrapped into `[-180, 180)`.
    pub fn angle(&self, source: &CoordinateSource) -> f32 {
        wrap_degrees(if self.relative { source.rotation.1 + self.angle } else { self.angle })
    }
}

/// `AngleArgument.angle()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct AngleArgumentType;

impl ArgumentType for AngleArgumentType {
    type Value = SingleAngle;

    fn parse(&self, reader: &mut StringReader) -> Result<SingleAngle, CommandSyntaxException> {
        if !reader.can_read() {
            return Err(built_in::simple("argument.angle.incomplete", reader.units(), reader.cursor()));
        }
        let relative = WorldCoordinate::is_relative_prefix(reader);
        let angle = if reader.can_read() && reader.peek() != Some(SPACE) {
            reader.read_float()?
        } else {
            0.0
        };
        if !angle.is_finite() {
            return Err(built_in::simple("argument.angle.invalid", reader.units(), reader.cursor()));
        }
        Ok(SingleAngle { angle, relative })
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        singleton(registry_ids::ANGLE)
    }
}

/// The `EnumSet<Direction.Axis>` a swizzle parses to.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Swizzle {
    pub x: bool,
    pub y: bool,
    pub z: bool,
}

impl Swizzle {
    /// The axes in `Axis` ordinal order as lower-case letters, like the `EnumSet` iterates.
    pub fn letters(&self) -> String {
        let mut letters = String::new();
        for (present, letter) in [(self.x, 'x'), (self.y, 'y'), (self.z, 'z')] {
            if present {
                letters.push(letter);
            }
        }
        letters
    }
}

/// `SwizzleArgument.swizzle()`: each of `x`, `y`, `z` at most once, in any order.
#[derive(Clone, Copy, Debug, Default)]
pub struct SwizzleArgumentType;

impl ArgumentType for SwizzleArgumentType {
    type Value = Swizzle;

    fn parse(&self, reader: &mut StringReader) -> Result<Swizzle, CommandSyntaxException> {
        let mut swizzle = Swizzle::default();
        while reader.can_read() && reader.peek() != Some(SPACE) {
            let symbol = reader.read();
            let axis = match symbol {
                Some(unit) if unit == b'x' as u16 => &mut swizzle.x,
                Some(unit) if unit == b'y' as u16 => &mut swizzle.y,
                Some(unit) if unit == b'z' as u16 => &mut swizzle.z,
                _ => {
                    return Err(built_in::simple("arguments.swizzle.invalid", reader.units(), reader.cursor()));
                }
            };
            if *axis {
                return Err(built_in::simple("arguments.swizzle.invalid", reader.units(), reader.cursor()));
            }
            *axis = true;
        }
        Ok(swizzle)
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        singleton(registry_ids::SWIZZLE)
    }
}
