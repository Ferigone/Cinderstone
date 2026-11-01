//! Geometry primitives ported from the pinned vanilla 1.21 `exc`, `ewx` and
//! `dcc` bytecode. Voxel collision uses the same 1e-7 boundaries as `exv`.
//! Static block collision shapes are extracted from the same original JAR.
//! Dynamic block shapes and complete entity movement rules remain caller work.

pub const EPSILON: f64 = 1.0e-7;
/// Default player attribute values from `buw` and `aqv` (creative adds 0.5).
pub const DEFAULT_BLOCK_INTERACTION_RANGE: f64 = 4.5;
pub const CREATIVE_BLOCK_INTERACTION_RANGE: f64 = 5.0;
pub const INTERACTION_VERIFICATION_BUFFER: f64 = 1.0;
pub const STANDING_EYE_HEIGHT: f32 = 1.62;
pub const CROUCHING_EYE_HEIGHT: f32 = 1.27;

const SIN_TABLE: &[u8; 262144] = include_bytes!("../fixtures/geometry-sin-1.21.bin");
fn sin_lookup(index: i32) -> f32 {
    let i = (index as usize & 65535) * 4;
    f32::from_be_bytes(SIN_TABLE[i..i + 4].try_into().expect("sine table entry"))
}
pub fn vanilla_sin(angle: f32) -> f32 {
    sin_lookup((angle * 10430.378_f32) as i32)
}
pub fn vanilla_cos(angle: f32) -> f32 {
    sin_lookup((angle * 10430.378_f32 + 16384.0_f32) as i32)
}
pub fn horizontal_look_direction(yaw: f32) -> Direction {
    match java_floor(yaw as f64 / 90. + 0.5) & 3 {
        0 => Direction::South,
        1 => Direction::West,
        2 => Direction::North,
        _ => Direction::East,
    }
}
/// Direction.orderedByNearest with the original float sine lookup table.
pub fn ordered_look_directions(pitch: f32, yaw: f32) -> [Direction; 6] {
    let pitch = pitch * 0.017453292_f32;
    let yaw = -yaw * 0.017453292_f32;
    let sp = vanilla_sin(pitch);
    let cp = vanilla_cos(pitch);
    let sy = vanilla_sin(yaw);
    let cy = vanilla_cos(yaw);
    let positive_x = sy > 0.;
    let positive_y = sp < 0.;
    let positive_z = cy > 0.;
    let ax = if positive_x { sy } else { -sy };
    let ay = if positive_y { -sp } else { sp };
    let az = if positive_z { cy } else { -cy };
    let x_horizontal = ax * cp;
    let z_horizontal = az * cp;
    let x = if positive_x {
        Direction::East
    } else {
        Direction::West
    };
    let y = if positive_y {
        Direction::Up
    } else {
        Direction::Down
    };
    let z = if positive_z {
        Direction::South
    } else {
        Direction::North
    };
    let first = if ax > az {
        if ay > x_horizontal {
            [y, x, z]
        } else if z_horizontal > ay {
            [x, z, y]
        } else {
            [x, y, z]
        }
    } else if ay > z_horizontal {
        [y, z, x]
    } else if x_horizontal > ay {
        [z, x, y]
    } else {
        [z, y, x]
    };
    [
        first[0],
        first[1],
        first[2],
        first[2].opposite(),
        first[1].opposite(),
        first[0].opposite(),
    ]
}

pub fn player_eye_position(feet: Vec3, eye_height: f32) -> Vec3 {
    Vec3::new(feet.x, feet.y + eye_height as f64, feet.z)
}

/// Player.canInteractWithBlock: eyes to the closest point on the entire unit
/// block AABB, with the server's verification buffer and a strict inequality.
pub fn can_interact_with_block(
    eyes: Vec3,
    block: (i32, i32, i32),
    attribute_range: f64,
    verification_buffer: f64,
) -> bool {
    let range = attribute_range + verification_buffer;
    Aabb::unit_block(block.0, block.1, block.2).distance_squared(eyes) < range * range
}

/// ServerGamePacketListenerImpl's additional UseItemOn hit-location check.
/// The packet reconstructs this world coordinate from its block and float cursor.
pub fn valid_use_item_hit_location(block: (i32, i32, i32), hit: Vec3) -> bool {
    let relative = hit.subtract(Vec3::new(
        block.0 as f64 + 0.5,
        block.1 as f64 + 0.5,
        block.2 as f64 + 0.5,
    ));
    relative.x.abs() < 1.0000001 && relative.y.abs() < 1.0000001 && relative.z.abs() < 1.0000001
}

type DynamicCollisionState = (u32, u8, u8, f32, f32, bool, &'static [[f64; 6]]);
include!("../fixtures/geometry-collisions-1.21.rs");

/// Inputs consumed by the original EntityCollisionContext and powder-snow block.
/// `can_walk_on_powder_snow` is the original entity-type tag OR living entity's
/// leather-boots check; callers supply that fact from their entity/equipment data.
#[derive(Clone, Copy, Debug)]
pub struct EntityCollisionContext {
    pub bottom_y: f64,
    pub descending: bool,
    pub fall_distance: f32,
    pub is_falling_block: bool,
    pub can_walk_on_powder_snow: bool,
}
#[derive(Clone, Copy, Debug)]
pub enum CollisionContext {
    Empty,
    Entity(EntityCollisionContext),
}
impl CollisionContext {
    fn above(self, block_y: i32, shape_max_y: f64, empty_default: bool) -> bool {
        match self {
            Self::Empty => empty_default,
            Self::Entity(c) => c.bottom_y > block_y as f64 + shape_max_y - 9.999999747378752e-6,
        }
    }
    fn descending(self) -> bool {
        matches!(self,Self::Entity(c) if c.descending)
    }
}
/// A caller must distinguish an absent block entity from one whose current
/// animation shape has not yet been calculated. Boxes are local block geometry.
#[derive(Clone, Copy, Debug)]
pub enum BlockEntityShape<'a> {
    Absent,
    Boxes(&'a [[f64; 6]]),
    Unavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollisionError {
    UnknownState,
    BlockEntityShapeRequired,
}

/// Exact static and context-dependent collision boxes in world coordinates.
/// Piston/shulker animation geometry comes from the explicit block-entity input.
pub fn block_collision_boxes(
    state_id: u32,
    position: (i32, i32, i32),
    context: CollisionContext,
    block_entity: BlockEntityShape<'_>,
) -> Result<Vec<Aabb>, CollisionError> {
    let &(shape, flags) = COLLISION_STATES
        .get(state_id as usize)
        .ok_or(CollisionError::UnknownState)?;
    let offset = Vec3::new(position.0 as f64, position.1 as f64, position.2 as f64);
    let mut local_offset = None;
    let boxes = if flags == 0 {
        COLLISION_SHAPES[shape as usize]
    } else {
        let index = DYNAMIC_COLLISION_STATES
            .binary_search_by_key(&state_id, |r| r.0)
            .map_err(|_| CollisionError::UnknownState)?;
        let &(_, kind, detail, horizontal, vertical, vertical_offset, raw) =
            &DYNAMIC_COLLISION_STATES[index];
        match kind {
            1 => raw,
            2 | 3 => {
                local_offset = Some(block_position_offset(
                    position,
                    horizontal,
                    vertical,
                    vertical_offset,
                ));
                raw
            }
            4 => {
                if context.above(position.1, 1., true) && !context.descending() {
                    SCAFFOLDING_STABLE
                } else if detail / 2 != 0
                    && detail & 1 != 0
                    && context.above(position.1, SCAFFOLDING_BELOW[0][4], true)
                {
                    SCAFFOLDING_BOTTOM
                } else {
                    &[]
                }
            }
            5 => match context {
                CollisionContext::Entity(c) if c.fall_distance > 2.5 => POWDER_SNOW_FALLING,
                CollisionContext::Entity(c)
                    if c.is_falling_block
                        || (c.can_walk_on_powder_snow
                            && context.above(position.1, 1., false)
                            && !c.descending) =>
                {
                    &[[0., 0., 0., 1., 1., 1.]][..]
                }
                _ => &[],
            },
            6 | 7 => match block_entity {
                BlockEntityShape::Boxes(boxes) => boxes,
                BlockEntityShape::Absent => {
                    if kind == 6 {
                        &[[0., 0., 0., 1., 1., 1.]][..]
                    } else {
                        &[]
                    }
                }
                BlockEntityShape::Unavailable => {
                    return Err(CollisionError::BlockEntityShapeRequired)
                }
            },
            _ => return Err(CollisionError::UnknownState),
        }
    };
    Ok(boxes
        .iter()
        .map(|b| {
            let b = Aabb::new(b[0], b[1], b[2], b[3], b[4], b[5]);
            let b = if let Some(offset) = local_offset {
                b.moved(offset)
            } else {
                b
            };
            b.moved(offset)
        })
        .collect())
}

/// BlockBehaviour.Properties offset function. The x*3129871 multiplication is
/// deliberately Java int-wrapping before promotion to the long seed expression.
pub fn block_position_offset(
    position: (i32, i32, i32),
    max_horizontal: f32,
    max_vertical: f32,
    vertical: bool,
) -> Vec3 {
    let seed =
        (position.0.wrapping_mul(3129871) as i64) ^ (position.2 as i64).wrapping_mul(116129781);
    let seed = seed
        .wrapping_mul(seed)
        .wrapping_mul(42317861)
        .wrapping_add(seed.wrapping_mul(11))
        >> 16;
    let nibble = |shift: i32| (((seed >> shift) & 15_i64) as f32 / 15.) as f64;
    let horizontal = |shift| {
        let n = (nibble(shift) - 0.5) * 0.5;
        let max = max_horizontal as f64;
        if n < -max {
            -max
        } else if n > max {
            max
        } else {
            n
        }
    };
    Vec3::new(
        horizontal(0),
        if vertical {
            (nibble(4) - 1.) * max_vertical as f64
        } else {
            0.
        },
        horizontal(8),
    )
}

/// Collision boxes for states that vanilla itself marks as cached and static.
/// None means that a real world/entity collision context is required, or the
/// state ID is outside the exact 1.21 registry. An empty slice is real airlike
/// collision, distinct from an unsupported dynamic shape.
pub fn static_block_collision_boxes(state_id: u32) -> Option<&'static [[f64; 6]]> {
    let &(shape, flags) = COLLISION_STATES.get(state_id as usize)?;
    (flags == 0).then(|| COLLISION_SHAPES[shape as usize])
}

/// A state-relative shape moved to its block location, using strict AABB
/// intersection just as the vanilla obstruction test does. None requests the
/// caller's context-dependent block shape implementation.
pub fn static_block_intersects(
    state_id: u32,
    position: (i32, i32, i32),
    bounds: Aabb,
) -> Option<bool> {
    let offset = Vec3::new(position.0 as f64, position.1 as f64, position.2 as f64);
    Some(static_block_collision_boxes(state_id)?.iter().any(|b| {
        Aabb::new(b[0], b[1], b[2], b[3], b[4], b[5])
            .moved(offset)
            .shape_intersects(bounds)
    }))
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl PartialEq for Vec3 {
    fn eq(&self, other: &Self) -> bool {
        double_equal(self.x, other.x)
            && double_equal(self.y, other.y)
            && double_equal(self.z, other.z)
    }
}
impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}
impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn plus(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
    pub fn subtract(self, other: Self) -> Self {
        self.plus(other.scale(-1.0))
    }
    pub fn vector_to(self, other: Self) -> Self {
        Self::new(other.x - self.x, other.y - self.y, other.z - self.z)
    }
    pub fn scale(self, factor: f64) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }
    pub fn multiply(self, other: Self) -> Self {
        Self::new(self.x * other.x, self.y * other.y, self.z * other.z)
    }
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }
    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }
    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }
    pub fn horizontal_distance_squared(self) -> f64 {
        self.x * self.x + self.z * self.z
    }
    pub fn normalize(self) -> Self {
        let length = self.length();
        if length < 1.0e-4 {
            Self::ZERO
        } else {
            Self::new(self.x / length, self.y / length, self.z / length)
        }
    }
    pub fn distance_squared(self, other: Self) -> f64 {
        self.vector_to(other).length_squared()
    }
    pub fn lerp(self, other: Self, fraction: f64) -> Self {
        Self::new(
            lerp(fraction, self.x, other.x),
            lerp(fraction, self.y, other.y),
            lerp(fraction, self.z, other.z),
        )
    }
    pub fn get(self, axis: Axis) -> f64 {
        match axis {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }
    pub fn with(self, axis: Axis, value: f64) -> Self {
        match axis {
            Axis::X => Self::new(value, self.y, self.z),
            Axis::Y => Self::new(self.x, value, self.z),
            Axis::Z => Self::new(self.x, self.y, value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    X,
    Y,
    Z,
}
impl Axis {
    fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Direction {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}
impl Direction {
    pub fn axis(self) -> Axis {
        match self {
            Self::Down | Self::Up => Axis::Y,
            Self::North | Self::South => Axis::Z,
            Self::West | Self::East => Axis::X,
        }
    }
    pub fn clockwise(self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
            _ => panic!("vertical direction has no horizontal clockwise turn"),
        }
    }
    pub fn counter_clockwise(self) -> Self {
        match self {
            Self::North => Self::West,
            Self::East => Self::North,
            Self::South => Self::East,
            Self::West => Self::South,
            _ => panic!("vertical direction has no horizontal counter-clockwise turn"),
        }
    }
    pub fn step(self) -> (i32, i32, i32) {
        match self {
            Self::Down => (0, -1, 0),
            Self::Up => (0, 1, 0),
            Self::North => (0, 0, -1),
            Self::South => (0, 0, 1),
            Self::West => (-1, 0, 0),
            Self::East => (1, 0, 0),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
            Self::North => "north",
            Self::South => "south",
            Self::West => "west",
            Self::East => "east",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "down" => Some(Self::Down),
            "up" => Some(Self::Up),
            "north" => Some(Self::North),
            "south" => Some(Self::South),
            "west" => Some(Self::West),
            "east" => Some(Self::East),
            _ => None,
        }
    }
    pub fn nearest(vector: Vec3) -> Self {
        // Direction.getNearest(double, double, double) narrows to float first.
        let [x, y, z] = [vector.x as f32, vector.y as f32, vector.z as f32];
        let mut result = Self::North;
        let mut best = f32::from_bits(1);
        for (direction, dx, dy, dz) in [
            (Self::Down, 0., -1., 0.),
            (Self::Up, 0., 1., 0.),
            (Self::North, 0., 0., -1.),
            (Self::South, 0., 0., 1.),
            (Self::West, -1., 0., 0.),
            (Self::East, 1., 0., 0.),
        ] {
            let dot = x * dx + y * dy + z * dz;
            if dot > best {
                best = dot;
                result = direction;
            }
        }
        result
    }
    pub fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}
impl PartialEq for Aabb {
    fn eq(&self, other: &Self) -> bool {
        self.min_position() == other.min_position() && self.max_position() == other.max_position()
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub location: Vec3,
    pub direction: Direction,
    pub fraction: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockHit {
    pub location: Vec3,
    pub direction: Direction,
    pub block_position: (i32, i32, i32),
    pub inside: bool,
}
impl Aabb {
    pub fn new(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> Self {
        Self {
            min_x: java_min(x1, x2),
            min_y: java_min(y1, y2),
            min_z: java_min(z1, z2),
            max_x: java_max(x1, x2),
            max_y: java_max(y1, y2),
            max_z: java_max(z1, z2),
        }
    }
    pub fn unit_block(x: i32, y: i32, z: i32) -> Self {
        Self::new(
            x as f64,
            y as f64,
            z as f64,
            x.wrapping_add(1) as f64,
            y.wrapping_add(1) as f64,
            z.wrapping_add(1) as f64,
        )
    }
    /// EntityDimensions.makeBoundingBox: the width is halved as a float first.
    pub fn player(position: Vec3, width: f32, height: f32) -> Self {
        let half = (width / 2.0) as f64;
        Self::new(
            position.x - half,
            position.y,
            position.z - half,
            position.x + half,
            position.y + height as f64,
            position.z + half,
        )
    }
    pub fn of_size(center: Vec3, x: f64, y: f64, z: f64) -> Self {
        Self::new(
            center.x - x / 2.0,
            center.y - y / 2.0,
            center.z - z / 2.0,
            center.x + x / 2.0,
            center.y + y / 2.0,
            center.z + z / 2.0,
        )
    }
    pub fn min(self, axis: Axis) -> f64 {
        self.min_position().get(axis)
    }
    pub fn max(self, axis: Axis) -> f64 {
        self.max_position().get(axis)
    }
    pub fn min_position(self) -> Vec3 {
        Vec3::new(self.min_x, self.min_y, self.min_z)
    }
    pub fn max_position(self) -> Vec3 {
        Vec3::new(self.max_x, self.max_y, self.max_z)
    }
    pub fn center(self) -> Vec3 {
        Vec3::new(
            lerp(0.5, self.min_x, self.max_x),
            lerp(0.5, self.min_y, self.max_y),
            lerp(0.5, self.min_z, self.max_z),
        )
    }
    pub fn bottom_center(self) -> Vec3 {
        Vec3::new(
            lerp(0.5, self.min_x, self.max_x),
            self.min_y,
            lerp(0.5, self.min_z, self.max_z),
        )
    }
    pub fn size(self) -> Vec3 {
        Vec3::new(
            self.max_x - self.min_x,
            self.max_y - self.min_y,
            self.max_z - self.min_z,
        )
    }
    pub fn has_nan(self) -> bool {
        [
            self.min_x, self.min_y, self.min_z, self.max_x, self.max_y, self.max_z,
        ]
        .iter()
        .any(|v| v.is_nan())
    }
    pub fn intersects(&self, other: &Self) -> bool {
        self.min_x < other.max_x
            && self.max_x > other.min_x
            && self.min_y < other.max_y
            && self.max_y > other.min_y
            && self.min_z < other.max_z
            && self.max_z > other.min_z
    }
    /// Shapes.joinIsNotEmpty(Shapes.create(this), Shapes.create(other), AND).
    /// Unlike raw AABB intersection, this preserves vanilla's voxel epsilon and
    /// canonical small-grid snapping used by entity placement obstruction.
    pub fn shape_intersects(self, other: Self) -> bool {
        VoxelShape::from_aabb(self).intersects_shape(&VoxelShape::from_aabb(other))
    }
    pub fn contains(self, p: Vec3) -> bool {
        p.x >= self.min_x
            && p.x < self.max_x
            && p.y >= self.min_y
            && p.y < self.max_y
            && p.z >= self.min_z
            && p.z < self.max_z
    }
    pub fn moved(self, v: Vec3) -> Self {
        Self::new(
            self.min_x + v.x,
            self.min_y + v.y,
            self.min_z + v.z,
            self.max_x + v.x,
            self.max_y + v.y,
            self.max_z + v.z,
        )
    }
    pub fn inflate(self, v: Vec3) -> Self {
        Self::new(
            self.min_x - v.x,
            self.min_y - v.y,
            self.min_z - v.z,
            self.max_x + v.x,
            self.max_y + v.y,
            self.max_z + v.z,
        )
    }
    pub fn deflate(self, v: Vec3) -> Self {
        self.inflate(v.scale(-1.0))
    }
    pub fn expand_towards(self, v: Vec3) -> Self {
        let mut lo = self.min_position();
        let mut hi = self.max_position();
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            let d = v.get(axis);
            if d < 0.0 {
                lo = lo.with(axis, lo.get(axis) + d);
            } else if d > 0.0 {
                hi = hi.with(axis, hi.get(axis) + d);
            }
        }
        Self::new(lo.x, lo.y, lo.z, hi.x, hi.y, hi.z)
    }
    pub fn contract(self, v: Vec3) -> Self {
        let mut lo = self.min_position();
        let mut hi = self.max_position();
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            let d = v.get(axis);
            if d < 0.0 {
                lo = lo.with(axis, lo.get(axis) - d);
            } else if d > 0.0 {
                hi = hi.with(axis, hi.get(axis) - d);
            }
        }
        Self::new(lo.x, lo.y, lo.z, hi.x, hi.y, hi.z)
    }
    // Vanilla calls the normalizing constructor even for a disjoint intersection.
    pub fn intersect(self, other: Self) -> Self {
        Self::new(
            java_max(self.min_x, other.min_x),
            java_max(self.min_y, other.min_y),
            java_max(self.min_z, other.min_z),
            java_min(self.max_x, other.max_x),
            java_min(self.max_y, other.max_y),
            java_min(self.max_z, other.max_z),
        )
    }
    pub fn minmax(self, other: Self) -> Self {
        Self::new(
            java_min(self.min_x, other.min_x),
            java_min(self.min_y, other.min_y),
            java_min(self.min_z, other.min_z),
            java_max(self.max_x, other.max_x),
            java_max(self.max_y, other.max_y),
            java_max(self.max_z, other.max_z),
        )
    }
    pub fn distance_squared(self, p: Vec3) -> f64 {
        let x = java_max(java_max(self.min_x - p.x, p.x - self.max_x), 0.0);
        let y = java_max(java_max(self.min_y - p.y, p.y - self.max_y), 0.0);
        let z = java_max(java_max(self.min_z - p.z, p.z - self.max_z), 0.0);
        x * x + y * y + z * z
    }
    pub fn clip(self, from: Vec3, to: Vec3) -> Option<RayHit> {
        self.clip_before(from, from.vector_to(to), 1.0)
    }
    fn clip_before(self, from: Vec3, delta: Vec3, mut fraction: f64) -> Option<RayHit> {
        let mut direction = None;
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            let d = delta.get(axis);
            let (face, bound) = match axis {
                Axis::X if d > EPSILON => (Direction::West, self.min_x),
                Axis::X if d < -EPSILON => (Direction::East, self.max_x),
                Axis::Y if d > EPSILON => (Direction::Down, self.min_y),
                Axis::Y if d < -EPSILON => (Direction::Up, self.max_y),
                Axis::Z if d > EPSILON => (Direction::North, self.min_z),
                Axis::Z if d < -EPSILON => (Direction::South, self.max_z),
                _ => continue,
            };
            let t = (bound - from.get(axis)) / d;
            let (a, b) = match axis {
                Axis::X => (Axis::Y, Axis::Z),
                Axis::Y => (Axis::Z, Axis::X),
                Axis::Z => (Axis::X, Axis::Y),
            };
            let pa = from.get(a) + t * delta.get(a);
            let pb = from.get(b) + t * delta.get(b);
            if 0.0 < t
                && t < fraction
                && self.min(a) - EPSILON < pa
                && pa < self.max(a) + EPSILON
                && self.min(b) - EPSILON < pb
                && pb < self.max(b) + EPSILON
            {
                fraction = t;
                direction = Some(face);
            }
        }
        direction.map(|direction| RayHit {
            location: from.plus(delta.scale(fraction)),
            direction,
            fraction,
        })
    }
}

/// Discrete voxel grid with exact coordinate planes, corresponding to an
/// ArrayVoxelShape. Occupancy is x-major, then y, then z, as in vanilla.
#[derive(Clone, Debug)]
pub struct VoxelShape {
    coordinates: [Vec<f64>; 3],
    size: [usize; 3],
    occupied: Vec<bool>,
}
impl VoxelShape {
    pub fn new(
        x: Vec<f64>,
        y: Vec<f64>,
        z: Vec<f64>,
        occupied: Vec<bool>,
    ) -> Result<Self, &'static str> {
        let coordinates = [x, y, z];
        if coordinates.iter().any(|v| {
            v.is_empty() || v.iter().any(|n| n.is_nan()) || v.windows(2).any(|w| w[0] >= w[1])
        }) {
            return Err("voxel coordinates must be strictly increasing");
        }
        let size = [
            coordinates[0].len() - 1,
            coordinates[1].len() - 1,
            coordinates[2].len() - 1,
        ];
        let len = size[0]
            .checked_mul(size[1])
            .and_then(|v| v.checked_mul(size[2]))
            .ok_or("voxel grid too large")?;
        if occupied.len() != len {
            return Err("voxel occupancy length mismatch");
        }
        Ok(Self {
            coordinates,
            size,
            occupied,
        })
    }
    /// Shapes.create(AABB), including near-grid snapping in [0,1].
    pub fn from_aabb(bounds: Aabb) -> Self {
        let axes = [Axis::X, Axis::Y, Axis::Z];
        if axes
            .iter()
            .any(|a| bounds.max(*a) - bounds.min(*a) < EPSILON)
        {
            return Self::new(vec![0.], vec![0.], vec![0.], vec![]).unwrap();
        }
        let bits = axes.map(|a| find_shape_bits(bounds.min(a), bounds.max(a)));
        if bits.iter().any(|b| b.is_none()) {
            return Self::single_box(bounds);
        }
        let dimensions = bits.map(|b| 1usize << b.unwrap());
        let coords = dimensions.map(|n| (0..=n).map(|i| i as f64 / n as f64).collect::<Vec<_>>());
        let lo = std::array::from_fn::<_, 3, _>(|i| {
            java_round_positive_grid(bounds.min(axes[i]) * dimensions[i] as f64)
        });
        let hi = std::array::from_fn::<_, 3, _>(|i| {
            java_round_positive_grid(bounds.max(axes[i]) * dimensions[i] as f64)
        });
        let mut occupied = Vec::new();
        for x in 0..dimensions[0] {
            for y in 0..dimensions[1] {
                for z in 0..dimensions[2] {
                    occupied.push(
                        x >= lo[0]
                            && x < hi[0]
                            && y >= lo[1]
                            && y < hi[1]
                            && z >= lo[2]
                            && z < hi[2],
                    );
                }
            }
        }
        Self::new(
            coords[0].clone(),
            coords[1].clone(),
            coords[2].clone(),
            occupied,
        )
        .unwrap()
    }
    /// Boolean AND occupancy with IndirectMerger's exact epsilon/index rules.
    pub fn intersects_shape(&self, other: &Self) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        let merged = std::array::from_fn::<_, 3, _>(|a| {
            merge_for_and(&self.coordinates[a], &other.coordinates[a])
        });
        for &(ax, bx) in &merged[0] {
            for &(ay, by) in &merged[1] {
                for &(az, bz) in &merged[2] {
                    if self.full_wide(ax, ay, az) && other.full_wide(bx, by, bz) {
                        return true;
                    }
                }
            }
        }
        false
    }
    pub fn single_box(b: Aabb) -> Self {
        Self::new(
            vec![b.min_x, b.max_x],
            vec![b.min_y, b.max_y],
            vec![b.min_z, b.max_z],
            vec![true],
        )
        .expect("nondegenerate finite box")
    }
    pub fn is_empty(&self) -> bool {
        !self.occupied.iter().any(|v| *v)
    }
    pub fn coordinates(&self, axis: Axis) -> &[f64] {
        &self.coordinates[axis.index()]
    }
    pub fn dimensions(&self) -> [usize; 3] {
        self.size
    }
    pub fn occupied(&self) -> &[bool] {
        &self.occupied
    }
    fn full_wide(&self, x: i32, y: i32, z: i32) -> bool {
        x >= 0
            && y >= 0
            && z >= 0
            && (x as usize) < self.size[0]
            && (y as usize) < self.size[1]
            && (z as usize) < self.size[2]
            && self.full(x as usize, y as usize, z as usize)
    }
    /// The original BitSetDiscreteVoxelShape box merging order (Y, X, Z).
    pub fn to_aabbs(&self) -> Vec<Aabb> {
        let mut remaining = self.clone();
        let mut boxes = Vec::new();
        for y in 0..self.size[1] {
            for x in 0..self.size[0] {
                let mut run = None;
                for z in 0..=self.size[2] {
                    if z < self.size[2] && remaining.full(x, y, z) {
                        if run.is_none() {
                            run = Some(z);
                        }
                    } else if let Some(z_start) = run.take() {
                        let clear = |shape: &mut Self, x: usize, y: usize| {
                            for zz in z_start..z {
                                shape.occupied[(x * shape.size[1] + y) * shape.size[2] + zz] =
                                    false;
                            }
                        };
                        clear(&mut remaining, x, y);
                        let mut x_end = x + 1;
                        while x_end < self.size[0]
                            && (z_start..z).all(|zz| remaining.full(x_end, y, zz))
                        {
                            clear(&mut remaining, x_end, y);
                            x_end += 1;
                        }
                        let mut y_end = y + 1;
                        while y_end < self.size[1]
                            && (x..x_end)
                                .all(|xx| (z_start..z).all(|zz| remaining.full(xx, y_end, zz)))
                        {
                            for xx in x..x_end {
                                clear(&mut remaining, xx, y_end);
                            }
                            y_end += 1;
                        }
                        boxes.push(Aabb::new(
                            self.coordinates[0][x],
                            self.coordinates[1][y],
                            self.coordinates[2][z_start],
                            self.coordinates[0][x_end],
                            self.coordinates[1][y_end],
                            self.coordinates[2][z],
                        ));
                    }
                }
            }
        }
        boxes
    }
    pub fn clip(&self, from: Vec3, to: Vec3, block_position: (i32, i32, i32)) -> Option<BlockHit> {
        if self.is_empty() {
            return None;
        }
        let delta = to.subtract(from);
        if delta.length_squared() < EPSILON {
            return None;
        }
        let inside_point = from.plus(delta.scale(0.001));
        let offset = Vec3::new(
            block_position.0 as f64,
            block_position.1 as f64,
            block_position.2 as f64,
        );
        let local = inside_point.subtract(offset);
        if self.full_wide(
            self.find_index(0, local.x),
            self.find_index(1, local.y),
            self.find_index(2, local.z),
        ) {
            return Some(BlockHit {
                location: inside_point,
                direction: Direction::nearest(delta).opposite(),
                block_position,
                inside: true,
            });
        }
        let mut result = None;
        let mut fraction = 1.0;
        for box_ in self.to_aabbs() {
            if let Some(hit) = box_.moved(offset).clip_before(from, delta, fraction) {
                fraction = hit.fraction;
                result = Some(BlockHit {
                    location: hit.location,
                    direction: hit.direction,
                    block_position,
                    inside: false,
                });
            }
        }
        result
    }

    fn find_index(&self, axis: usize, value: f64) -> i32 {
        // binarySearch(0,size+1, coordinate > value)-1
        let coordinates = &self.coordinates[axis];
        coordinates.partition_point(|plane| *plane <= value) as i32 - 1
    }
    fn full(&self, x: usize, y: usize, z: usize) -> bool {
        self.occupied[(x * self.size[1] + y) * self.size[2] + z]
    }
    pub fn collide(&self, axis: Axis, box_: Aabb, mut movement: f64) -> f64 {
        if self.is_empty() {
            return movement;
        }
        if movement.abs() < EPSILON {
            return 0.0;
        }
        let a = axis.index();
        let b = (a + 1) % 3;
        let c = (a + 2) % 3;
        let axes = [Axis::X, Axis::Y, Axis::Z];
        let min = box_.min(axis);
        let max = box_.max(axis);
        let min_index = self.find_index(a, min + EPSILON);
        let max_index = self.find_index(a, max - EPSILON);
        let b_start = 0.max(self.find_index(b, box_.min(axes[b]) + EPSILON));
        let b_end = (self.size[b] as i32).min(self.find_index(b, box_.max(axes[b]) - EPSILON) + 1);
        let c_start = 0.max(self.find_index(c, box_.min(axes[c]) + EPSILON));
        let c_end = (self.size[c] as i32).min(self.find_index(c, box_.max(axes[c]) - EPSILON) + 1);
        let collision_at = |i: i32| {
            if i < 0 || i >= self.size[a] as i32 {
                return false;
            }
            for j in b_start..b_end {
                for k in c_start..c_end {
                    let mut indices = [0; 3];
                    indices[a] = i as usize;
                    indices[b] = j as usize;
                    indices[c] = k as usize;
                    if self.full(indices[0], indices[1], indices[2]) {
                        return true;
                    }
                }
            }
            false
        };
        if movement > 0.0 {
            for i in max_index + 1..self.size[a] as i32 {
                if collision_at(i) {
                    let distance = self.coordinates[a][i as usize] - max;
                    if distance >= -EPSILON {
                        movement = java_min(movement, distance);
                    }
                    return movement;
                }
            }
        } else if movement < 0.0 {
            for i in (0..min_index).rev() {
                if collision_at(i) {
                    let distance = self.coordinates[a][i as usize + 1] - min;
                    if distance <= EPSILON {
                        movement = java_max(movement, distance);
                    }
                    return movement;
                }
            }
        }
        movement
    }
}

/// Entity.collideWithShapes: clip Y first, then the larger horizontal movement
/// axis, preserving the original collision list order and epsilon cutoff.
pub fn collide_with_shapes(movement: Vec3, mut box_: Aabb, shapes: &[VoxelShape]) -> Vec3 {
    if shapes.is_empty() {
        return movement;
    }
    let mut x = movement.x;
    let mut y = movement.y;
    let mut z = movement.z;
    let clip = |axis: Axis, b: Aabb, mut d: f64| {
        for shape in shapes {
            if d.abs() < EPSILON {
                return 0.0;
            }
            d = shape.collide(axis, b, d);
        }
        d
    };
    if y != 0.0 {
        y = clip(Axis::Y, box_, y);
        if y != 0.0 {
            box_ = box_.moved(Vec3::new(0.0, y, 0.0));
        }
    }
    let z_first = x.abs() < z.abs();
    if z_first && z != 0.0 {
        z = clip(Axis::Z, box_, z);
        if z != 0.0 {
            box_ = box_.moved(Vec3::new(0.0, 0.0, z));
        }
    }
    if x != 0.0 {
        x = clip(Axis::X, box_, x);
        if !z_first && x != 0.0 {
            box_ = box_.moved(Vec3::new(x, 0.0, 0.0));
        }
    }
    if !z_first && z != 0.0 {
        z = clip(Axis::Z, box_, z);
    }
    Vec3::new(x, y, z)
}

/// BlockGetter.traverseBlocks, including outward endpoint nudges and the exact
/// Z/Y/X tie order. The closure may return Some to terminate traversal early.
/// Callers must restrict non-finite or excessively long client rays beforehand.
pub fn traverse_blocks<T>(
    from: Vec3,
    to: Vec3,
    mut visit: impl FnMut((i32, i32, i32)) -> Option<T>,
) -> Option<T> {
    if from == to {
        return None;
    }
    let end = to.lerp(from, -EPSILON);
    let start = from.lerp(to, -EPSILON);
    let mut p = [
        java_floor(start.x),
        java_floor(start.y),
        java_floor(start.z),
    ];
    if let Some(value) = visit((p[0], p[1], p[2])) {
        return Some(value);
    }
    let delta = start.vector_to(end);
    let delta = [delta.x, delta.y, delta.z];
    let start = [start.x, start.y, start.z];
    let sign = delta.map(|n| {
        if n == 0.0 {
            0
        } else if n > 0.0 {
            1
        } else {
            -1
        }
    });
    let step = std::array::from_fn::<_, 3, _>(|i| {
        if sign[i] == 0 {
            f64::MAX
        } else {
            sign[i] as f64 / delta[i]
        }
    });
    let mut next = std::array::from_fn::<_, 3, _>(|i| {
        step[i]
            * if sign[i] > 0 {
                1.0 - fraction(start[i])
            } else {
                fraction(start[i])
            }
    });
    while next[0] <= 1.0 || next[1] <= 1.0 || next[2] <= 1.0 {
        let axis = if next[0] < next[1] {
            if next[0] < next[2] {
                0
            } else {
                2
            }
        } else if next[1] < next[2] {
            1
        } else {
            2
        };
        p[axis] = p[axis].wrapping_add(sign[axis]);
        next[axis] += step[axis];
        if let Some(value) = visit((p[0], p[1], p[2])) {
            return Some(value);
        }
    }
    None
}

fn java_round_positive_grid(n: f64) -> usize {
    (n + 0.5).floor() as usize
}
fn find_shape_bits(min: f64, max: f64) -> Option<u32> {
    if min < -EPSILON || max > 1.0000001 {
        return None;
    }
    (0..=3).find(|bits| {
        let size = (1u32 << bits) as f64;
        let a = min * size;
        let b = max * size;
        (a - (a + 0.5).floor()).abs() < EPSILON * size
            && (b - (b + 0.5).floor()).abs() < EPSILON * size
    })
}
fn merge_for_and(first: &[f64], second: &[f64]) -> Vec<(i32, i32)> {
    let (mut i, mut j) = (0, 0);
    let mut planes = Vec::new();
    let mut indices = Vec::new();
    let mut previous = f64::NAN;
    while i < first.len() || j < second.len() {
        let first_end = i >= first.len();
        let second_end = j >= second.len();
        let from_first = !first_end && (second_end || first[i] < second[j] + EPSILON);
        let value = if from_first {
            i += 1;
            if j == 0 || second_end {
                continue;
            }
            first[i - 1]
        } else {
            j += 1;
            if i == 0 || first_end {
                continue;
            }
            second[j - 1]
        };
        let pair = (i as i32 - 1, j as i32 - 1);
        if matches!(
            previous.partial_cmp(&(value - EPSILON)),
            None | Some(std::cmp::Ordering::Less)
        ) {
            planes.push(value);
            indices.push(pair);
            previous = value;
        } else {
            let last = indices.len() - 1;
            indices[last] = pair;
        }
    }
    indices.pop();
    indices
}
fn lerp(fraction: f64, from: f64, to: f64) -> f64 {
    from + fraction * (to - from)
}
fn fraction(n: f64) -> f64 {
    n - n.floor()
}
fn java_floor(n: f64) -> i32 {
    let integer = n as i32;
    if n < (integer as f64) {
        integer.wrapping_sub(1)
    } else {
        integer
    }
}
fn double_equal(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f64::from_bits(a.to_bits() | b.to_bits())
    } else if a <= b {
        a
    } else {
        b
    }
}
fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f64::from_bits(a.to_bits() & b.to_bits())
    } else if a >= b {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verification_reach_uses_eyes_closest_box_point_and_strict_boundary() {
        let block = (0, 0, 0);
        assert!(can_interact_with_block(
            Vec3::new(-5.99999999, 0.5, 0.5),
            block,
            5.,
            1.
        ));
        assert!(!can_interact_with_block(
            Vec3::new(-6., 0.5, 0.5),
            block,
            5.,
            1.
        ));
        assert!(can_interact_with_block(
            Vec3::new(7. - 1e-8, 0.5, 0.5),
            block,
            5.,
            1.
        ));
        assert!(!can_interact_with_block(
            Vec3::new(7., 0.5, 0.5),
            block,
            5.,
            1.
        ));
        assert_eq!(
            player_eye_position(Vec3::ZERO, STANDING_EYE_HEIGHT).y,
            1.6200000047683716
        );
        assert!(valid_use_item_hit_location(block, Vec3::new(1.5, 0.5, 0.5)));
        assert!(!valid_use_item_hit_location(
            block,
            Vec3::new(1.5000001, 0.5, 0.5)
        ));
        assert!(!valid_use_item_hit_location(
            block,
            Vec3::new(f64::NAN, 0.5, 0.5)
        ));
    }
    #[test]
    fn placement_shape_merger_suppresses_sub_epsilon_overlaps() {
        let a = Aabb::unit_block(0, 0, 0);
        let b = Aabb::new(1. - 1e-8, 0., 0., 2., 1., 1.);
        assert!(a.intersects(&b));
        assert!(!a.shape_intersects(b));
        assert!(a.shape_intersects(Aabb::new(1. - 1e-6, 0., 0., 2., 1., 1.)));
    }
    #[test]
    fn exact_state_collision_catalog() {
        assert_eq!(COLLISION_STATES.len(), 26684);
        assert_eq!(COLLISION_SHAPES.len(), 320);
        assert_eq!(
            COLLISION_STATES.iter().filter(|(_, f)| *f == 0).count(),
            26411
        );
        assert_eq!(static_block_collision_boxes(0), Some(&[][..]));
        assert_eq!(
            static_block_collision_boxes(1),
            Some(&[[0., 0., 0., 1., 1., 1.]][..][..])
        );
        assert!(static_block_collision_boxes(26684).is_none());
        for (state, (_, flags)) in COLLISION_STATES.iter().enumerate() {
            assert_eq!(
                static_block_collision_boxes(state as u32).is_some(),
                *flags == 0
            );
        }
    }
    #[test]
    fn placement_edges_are_open() {
        let block = Aabb::unit_block(0, 0, 0);
        assert!(!block.intersects(&Aabb::unit_block(1, 0, 0)));
        assert!(block.contains(Vec3::ZERO));
        assert!(!block.contains(Vec3::new(1.0, 0.0, 0.0)));
        assert!(block.intersects(&Aabb::new(0.9, 0.0, 0.0, 1.9, 1.0, 1.0)));
    }
    #[test]
    fn player_dimensions_keep_float_precision() {
        let b = Aabb::player(Vec3::new(8.5, 65.0, 8.5), 0.6, 1.8);
        assert_eq!(b.min_x, 8.5 - 0.30000001192092896);
        assert_eq!(b.max_y, 65.0 + 1.7999999523162842);
    }
    #[test]
    fn dda_ties_and_nudges() {
        let mut cells = Vec::new();
        traverse_blocks(Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.5, 1.5, 1.5), |p| {
            cells.push(p);
            None::<()>
        });
        assert_eq!(cells, [(0, 0, 0), (0, 0, 1), (0, 1, 1), (1, 1, 1)]);
        let mut cells = Vec::new();
        traverse_blocks(Vec3::new(0.0, 0.5, 0.5), Vec3::new(1.0, 0.5, 0.5), |p| {
            cells.push(p);
            None::<()>
        });
        assert_eq!(cells, [(-1, 0, 0), (0, 0, 0), (1, 0, 0)]);
    }
    #[test]
    fn clip_does_not_hit_start_or_end() {
        let b = Aabb::unit_block(0, 0, 0);
        assert!(b
            .clip(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(0.0, 0.5, 0.5))
            .is_none());
        let hit = b
            .clip(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(2.0, 0.5, 0.5))
            .unwrap();
        assert_eq!(hit.direction, Direction::West);
        assert_eq!(hit.location, Vec3::new(0.0, 0.5, 0.5));
        assert!(b
            .clip(Vec3::new(0.5, 0.5, 0.5), Vec3::new(2.0, 0.5, 0.5))
            .is_none());
    }
    #[test]
    fn normalized_small_vector_is_zero() {
        assert_eq!(Vec3::new(0.00001, 0.0, 0.0).normalize(), Vec3::ZERO);
        assert_eq!(
            Vec3::new(1.0, 0.0, 0.0).normalize(),
            Vec3::new(1.0, 0.0, 0.0)
        );
    }
    #[test]
    fn collision_epsilon() {
        let shape = VoxelShape::single_box(Aabb::unit_block(0, 0, 0));
        let b = Aabb::new(-1.0, 0.0, 0.0, -0.25, 1.0, 1.0);
        assert_eq!(shape.collide(Axis::X, b, 2.0), 0.25);
        assert_eq!(shape.collide(Axis::X, b, 0.00000001), 0.0);
        assert_eq!(shape.collide(Axis::Y, b, 2.0), 2.0);
    }
    #[test]
    fn signed_zero_and_nan_match_double_compare() {
        assert_ne!(Vec3::new(-0.0, 0.0, 0.0), Vec3::ZERO);
        assert_eq!(
            Vec3::new(f64::NAN, 0.0, 0.0),
            Vec3::new(f64::from_bits(0x7ff8000000000001), 0.0, 0.0)
        );
        let b = Aabb::new(-0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(b.min_x.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(b.max_x.to_bits(), 0.0_f64.to_bits());
    }
}
