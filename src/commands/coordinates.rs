//! Position arguments (`net.minecraft.commands.arguments.coordinates`): `Vec3Argument`,
//! `BlockPosArgument`, `Vec2Argument` and `ColumnPosArgument` with `WorldCoordinate(s)`,
//! `LocalCoordinates` and `Coordinates`.
//!
//! Ported from the unchanged classes of the pinned 1.21 server, read with direct `javap`
//! inspection of the jar (remapped with the official mappings, kept outside git) and compared
//! with the executed originals in `fixtures/commands-mc-arguments-vectors.json`.
//!
//! What is covered: the parse of absolute, relative (`~`) and local (`^`) coordinates with
//! the original's exceptions and cursor handling, the position, rotation and block position they
//! resolve to for a command source, and the `ArgumentTypeInfos` network forms. Not covered:
//! coordinate suggestions (they need a source that exposes the block it targets), the
//! loaded / out-of-world checks of `BlockPosArgument.getLoadedBlockPos` (they need a level) and
//! `EntityAnchorArgument` (the anchored position is supplied by the caller).

use super::arguments::{ArgumentNetworkInfo, ArgumentType};
use super::exception::{built_in, CommandSyntaxException};
use super::string_reader::StringReader;
use crate::geometry::{vanilla_cos, vanilla_sin, Vec3};

const SPACE: u16 = b' ' as u16;
const TILDE: u16 = b'~' as u16;
const CARET: u16 = b'^' as u16;

/// `ArgumentTypeInfos` registry ids (1.21 registration order).
pub mod registry_ids {
    pub const BLOCK_POS: i32 = 8;
    pub const COLUMN_POS: i32 = 9;
    pub const VEC3: i32 = 10;
    pub const VEC2: i32 = 11;
}

/// What the coordinates read from a `CommandSourceStack`: its position, the position after the
/// source's `EntityAnchorArgument.Anchor` has been applied (feet, or eyes of the executing
/// entity), and its rotation as `(pitch, yaw)` like `Vec2(x, y)`.
#[derive(Clone, Copy, Debug)]
pub struct CoordinateSource {
    pub position: Vec3,
    pub anchored_position: Vec3,
    pub rotation: (f32, f32),
}

/// `Mth.floor(double)`: `(int) d`, minus one when that rounded up. Java's `d2i` saturates and
/// maps NaN to 0 like Rust's cast, and `i - 1` wraps, so a value below `Integer.MIN_VALUE`
/// floors to `Integer.MAX_VALUE`.
fn floor(value: f64) -> i32 {
    let truncated = value as i32;
    if value < f64::from(truncated) {
        truncated.wrapping_sub(1)
    } else {
        truncated
    }
}

fn units(reader: &StringReader) -> Vec<u16> {
    reader.units().to_vec()
}

/// `WorldCoordinate`: a number that is absolute, or relative (`~`) to the source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldCoordinate {
    relative: bool,
    value: f64,
}

impl WorldCoordinate {
    pub fn new(relative: bool, value: f64) -> Self {
        Self { relative, value }
    }

    pub fn is_relative(&self) -> bool {
        self.relative
    }

    /// `get(original)`.
    pub fn get(&self, original: f64) -> f64 {
        if self.relative {
            self.value + original
        } else {
            self.value
        }
    }

    pub(super) fn is_relative_prefix(reader: &mut StringReader) -> bool {
        let relative = reader.peek() == Some(TILDE);
        if relative {
            reader.skip();
        }
        relative
    }

    /// `parseDouble(reader, centerCorrect)`: an absolute integer-looking value is moved to the
    /// middle of its block (`+ 0.5`) when `center_correct` is set.
    pub fn parse_double(reader: &mut StringReader, center_correct: bool) -> Result<Self, CommandSyntaxException> {
        if reader.can_read() && reader.peek() == Some(CARET) {
            return Err(built_in::simple("argument.pos.mixed", reader.units(), reader.cursor()));
        }
        if !reader.can_read() {
            return Err(built_in::simple("argument.pos.missing.double", reader.units(), reader.cursor()));
        }
        let relative = Self::is_relative_prefix(reader);
        let start = reader.cursor();
        let mut value = if reader.can_read() && reader.peek() != Some(SPACE) {
            reader.read_double()?
        } else {
            0.0
        };
        let text = &reader.units()[start..reader.cursor()];
        if relative && text.is_empty() {
            return Ok(Self::new(true, 0.0));
        }
        if !text.contains(&(b'.' as u16)) && !relative && center_correct {
            value += 0.5;
        }
        Ok(Self::new(relative, value))
    }

    /// `parseInt(reader)`: block coordinates; a relative offset may still be fractional.
    pub fn parse_int(reader: &mut StringReader) -> Result<Self, CommandSyntaxException> {
        if reader.can_read() && reader.peek() == Some(CARET) {
            return Err(built_in::simple("argument.pos.mixed", reader.units(), reader.cursor()));
        }
        if !reader.can_read() {
            return Err(built_in::simple("argument.pos.missing.int", reader.units(), reader.cursor()));
        }
        let relative = Self::is_relative_prefix(reader);
        let value = if reader.can_read() && reader.peek() != Some(SPACE) {
            if relative {
                reader.read_double()?
            } else {
                f64::from(reader.read_int()?)
            }
        } else {
            0.0
        };
        Ok(Self::new(relative, value))
    }
}

/// `WorldCoordinates`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldCoordinates {
    x: WorldCoordinate,
    y: WorldCoordinate,
    z: WorldCoordinate,
}

impl WorldCoordinates {
    pub fn new(x: WorldCoordinate, y: WorldCoordinate, z: WorldCoordinate) -> Self {
        Self { x, y, z }
    }

    fn incomplete(reader: &mut StringReader, start: usize) -> CommandSyntaxException {
        reader.set_cursor(start);
        built_in::simple("argument.pos3d.incomplete", reader.units(), reader.cursor())
    }

    /// `parseInt(reader)`: three block coordinates.
    pub fn parse_int(reader: &mut StringReader) -> Result<Self, CommandSyntaxException> {
        let start = reader.cursor();
        let x = WorldCoordinate::parse_int(reader)?;
        if reader.can_read() && reader.peek() == Some(SPACE) {
            reader.skip();
            let y = WorldCoordinate::parse_int(reader)?;
            if reader.can_read() && reader.peek() == Some(SPACE) {
                reader.skip();
                let z = WorldCoordinate::parse_int(reader)?;
                return Ok(Self::new(x, y, z));
            }
        }
        Err(Self::incomplete(reader, start))
    }

    /// `parseDouble(reader, centerCorrect)`: three coordinates, the middle one never centred.
    pub fn parse_double(reader: &mut StringReader, center_correct: bool) -> Result<Self, CommandSyntaxException> {
        let start = reader.cursor();
        let x = WorldCoordinate::parse_double(reader, center_correct)?;
        if reader.can_read() && reader.peek() == Some(SPACE) {
            reader.skip();
            let y = WorldCoordinate::parse_double(reader, false)?;
            if reader.can_read() && reader.peek() == Some(SPACE) {
                reader.skip();
                let z = WorldCoordinate::parse_double(reader, center_correct)?;
                return Ok(Self::new(x, y, z));
            }
        }
        Err(Self::incomplete(reader, start))
    }
}

/// `LocalCoordinates`: `^left ^up ^forwards`, relative to the source's facing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalCoordinates {
    left: f64,
    up: f64,
    forwards: f64,
}

impl LocalCoordinates {
    pub fn new(left: f64, up: f64, forwards: f64) -> Self {
        Self { left, up, forwards }
    }

    fn read_double(reader: &mut StringReader, start: usize) -> Result<f64, CommandSyntaxException> {
        if !reader.can_read() {
            return Err(built_in::simple("argument.pos.missing.double", reader.units(), reader.cursor()));
        }
        if reader.peek() != Some(CARET) {
            reader.set_cursor(start);
            return Err(built_in::simple("argument.pos.mixed", reader.units(), reader.cursor()));
        }
        reader.skip();
        if reader.can_read() && reader.peek() != Some(SPACE) {
            reader.read_double()
        } else {
            Ok(0.0)
        }
    }

    pub fn parse(reader: &mut StringReader) -> Result<Self, CommandSyntaxException> {
        let start = reader.cursor();
        let left = Self::read_double(reader, start)?;
        if reader.can_read() && reader.peek() == Some(SPACE) {
            reader.skip();
            let up = Self::read_double(reader, start)?;
            if reader.can_read() && reader.peek() == Some(SPACE) {
                reader.skip();
                let forwards = Self::read_double(reader, start)?;
                return Ok(Self::new(left, up, forwards));
            }
        }
        Err(WorldCoordinates::incomplete(reader, start))
    }

    /// `getPosition(source)`: the offset is applied along the axes the source faces, computed
    /// with Minecraft's float sine table exactly as the original.
    pub fn position(&self, source: &CoordinateSource) -> Vec3 {
        const RADIANS: f32 = 0.017_453_292;
        let (pitch, yaw) = source.rotation;
        let f = vanilla_cos((yaw + 90.0) * RADIANS);
        let f1 = vanilla_sin((yaw + 90.0) * RADIANS);
        let f2 = vanilla_cos(-pitch * RADIANS);
        let f3 = vanilla_sin(-pitch * RADIANS);
        let f4 = vanilla_cos((-pitch + 90.0) * RADIANS);
        let f5 = vanilla_sin((-pitch + 90.0) * RADIANS);
        let forward = Vec3::new(f64::from(f * f2), f64::from(f3), f64::from(f1 * f2));
        let up = Vec3::new(f64::from(f * f4), f64::from(f5), f64::from(f1 * f4));
        let left = forward.cross(up).scale(-1.0);
        let anchored = source.anchored_position;
        Vec3::new(
            anchored.x + (forward.x * self.forwards + up.x * self.up + left.x * self.left),
            anchored.y + (forward.y * self.forwards + up.y * self.up + left.y * self.left),
            anchored.z + (forward.z * self.forwards + up.z * self.up + left.z * self.left),
        )
    }
}

/// `Coordinates`: what a position argument parses to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coordinates {
    World(WorldCoordinates),
    Local(LocalCoordinates),
}

impl Coordinates {
    /// `getPosition(source)`.
    pub fn position(&self, source: &CoordinateSource) -> Vec3 {
        match self {
            Self::World(world) => Vec3::new(
                world.x.get(source.position.x),
                world.y.get(source.position.y),
                world.z.get(source.position.z),
            ),
            Self::Local(local) => local.position(source),
        }
    }

    /// `getRotation(source)` as `(pitch, yaw)`; local coordinates have none (`Vec2.ZERO`).
    pub fn rotation(&self, source: &CoordinateSource) -> (f32, f32) {
        match self {
            Self::World(world) => (
                world.x.get(f64::from(source.rotation.0)) as f32,
                world.y.get(f64::from(source.rotation.1)) as f32,
            ),
            Self::Local(_) => (0.0, 0.0),
        }
    }

    pub fn is_x_relative(&self) -> bool {
        matches!(self, Self::Local(_)) || matches!(self, Self::World(world) if world.x.relative)
    }

    pub fn is_y_relative(&self) -> bool {
        matches!(self, Self::Local(_)) || matches!(self, Self::World(world) if world.y.relative)
    }

    pub fn is_z_relative(&self) -> bool {
        matches!(self, Self::Local(_)) || matches!(self, Self::World(world) if world.z.relative)
    }

    /// `getBlockPos(source)` = `BlockPos.containing(getPosition(source))`.
    pub fn block_pos(&self, source: &CoordinateSource) -> (i32, i32, i32) {
        let position = self.position(source);
        (floor(position.x), floor(position.y), floor(position.z))
    }
}

fn parse_position(reader: &mut StringReader, center_correct: bool) -> Result<Coordinates, CommandSyntaxException> {
    if reader.can_read() && reader.peek() == Some(CARET) {
        LocalCoordinates::parse(reader).map(Coordinates::Local)
    } else {
        WorldCoordinates::parse_double(reader, center_correct).map(Coordinates::World)
    }
}

fn two_dimensional_incomplete(reader: &mut StringReader, start: usize) -> CommandSyntaxException {
    reader.set_cursor(start);
    built_in::simple("argument.pos2d.incomplete", &units(reader), reader.cursor())
}

fn network(registry_id: i32) -> Option<ArgumentNetworkInfo> {
    Some(ArgumentNetworkInfo {
        registry_id,
        template: Vec::new(),
    })
}

/// `Vec3Argument.vec3(centerCorrect)`.
#[derive(Clone, Copy, Debug)]
pub struct Vec3ArgumentType {
    pub center_correct: bool,
}

impl Vec3ArgumentType {
    /// `vec3()`: whole-number coordinates are moved to the centre of the block.
    pub fn new() -> Self {
        Self { center_correct: true }
    }

    pub fn with_center_correct(center_correct: bool) -> Self {
        Self { center_correct }
    }
}

impl Default for Vec3ArgumentType {
    fn default() -> Self {
        Self::new()
    }
}

impl ArgumentType for Vec3ArgumentType {
    type Value = Coordinates;

    fn parse(&self, reader: &mut StringReader) -> Result<Coordinates, CommandSyntaxException> {
        parse_position(reader, self.center_correct)
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        network(registry_ids::VEC3)
    }
}

/// `BlockPosArgument.blockPos()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct BlockPosArgumentType;

impl ArgumentType for BlockPosArgumentType {
    type Value = Coordinates;

    fn parse(&self, reader: &mut StringReader) -> Result<Coordinates, CommandSyntaxException> {
        if reader.can_read() && reader.peek() == Some(CARET) {
            LocalCoordinates::parse(reader).map(Coordinates::Local)
        } else {
            WorldCoordinates::parse_int(reader).map(Coordinates::World)
        }
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        network(registry_ids::BLOCK_POS)
    }
}

/// `Vec2Argument.vec2(centerCorrect)`: `x z`, with a relative zero `y`.
#[derive(Clone, Copy, Debug)]
pub struct Vec2ArgumentType {
    pub center_correct: bool,
}

impl Vec2ArgumentType {
    pub fn new() -> Self {
        Self { center_correct: true }
    }

    pub fn with_center_correct(center_correct: bool) -> Self {
        Self { center_correct }
    }
}

impl Default for Vec2ArgumentType {
    fn default() -> Self {
        Self::new()
    }
}

impl ArgumentType for Vec2ArgumentType {
    type Value = Coordinates;

    fn parse(&self, reader: &mut StringReader) -> Result<Coordinates, CommandSyntaxException> {
        let start = reader.cursor();
        if !reader.can_read() {
            return Err(two_dimensional_incomplete(reader, start));
        }
        let x = WorldCoordinate::parse_double(reader, self.center_correct)?;
        if reader.can_read() && reader.peek() == Some(SPACE) {
            reader.skip();
            let z = WorldCoordinate::parse_double(reader, self.center_correct)?;
            Ok(Coordinates::World(WorldCoordinates::new(x, WorldCoordinate::new(true, 0.0), z)))
        } else {
            Err(two_dimensional_incomplete(reader, start))
        }
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        network(registry_ids::VEC2)
    }
}

/// `ColumnPosArgument.columnPos()`: `x z` block coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnPosArgumentType;

impl ArgumentType for ColumnPosArgumentType {
    type Value = Coordinates;

    fn parse(&self, reader: &mut StringReader) -> Result<Coordinates, CommandSyntaxException> {
        let start = reader.cursor();
        if !reader.can_read() {
            return Err(two_dimensional_incomplete(reader, start));
        }
        let x = WorldCoordinate::parse_int(reader)?;
        if reader.can_read() && reader.peek() == Some(SPACE) {
            reader.skip();
            let z = WorldCoordinate::parse_int(reader)?;
            Ok(Coordinates::World(WorldCoordinates::new(x, WorldCoordinate::new(true, 0.0), z)))
        } else {
            Err(two_dimensional_incomplete(reader, start))
        }
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        network(registry_ids::COLUMN_POS)
    }
}
