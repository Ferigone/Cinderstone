//! Minecraft 1.21 fluid height, current and entity fluid-pushing calculations.
//!
//! Flow providers retain real chunk availability and world support-face facts.
//! Uncached support faces require an explicit host implementation; they never
//! silently become nonsolid. Fluid random ticks and block replacement are
//! separate mechanics.
use crate::{
    geometry::{Aabb, Direction, Vec3, VoxelShape},
    runtime_world::RuntimeWorld,
};
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[cfg(test)]
#[path = "../tools/fluid_shape_tests.rs"]
mod shape_tests;

pub type BlockPos = (i32, i32, i32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FluidKind {
    Empty,
    Water,
    Lava,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FluidTag {
    Water,
    Lava,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidState {
    pub kind: FluidKind,
    pub own_height: f32,
    pub amount: i32,
    pub falling: bool,
    pub source: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct FluidBlockFacts {
    pub fluid: FluidState,
    pub blocks_motion: bool,
    pub ice: bool,
    pub cached_sturdy_faces: Option<u8>,
}
impl FluidState {
    pub fn from_block(state: u32) -> Result<Self> {
        Ok(block_facts(state)?.fluid)
    }
    pub fn is_empty(self) -> bool {
        self.kind == FluidKind::Empty
    }
    pub fn matches(self, tag: FluidTag) -> bool {
        matches!(
            (self.kind, tag),
            (FluidKind::Water, FluidTag::Water) | (FluidKind::Lava, FluidTag::Lava)
        )
    }
    fn affects_flow_of(self, current: FluidKind) -> bool {
        self.is_empty() || self.kind == current
    }
}

/// The four vanilla FlowingFluid singleton instances own separate shape maps.
/// Their lifetime spans every level and dimension in the same server process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FluidShapeOwner {
    SourceWater,
    FlowingWater,
    SourceLava,
    FlowingLava,
}
impl FluidShapeOwner {
    pub const ALL: [Self; 4] = [
        Self::SourceWater,
        Self::FlowingWater,
        Self::SourceLava,
        Self::FlowingLava,
    ];
    fn kind(self) -> FluidKind {
        match self {
            Self::SourceWater | Self::FlowingWater => FluidKind::Water,
            Self::SourceLava | Self::FlowingLava => FluidKind::Lava,
        }
    }
    fn source(self) -> bool {
        matches!(self, Self::SourceWater | Self::SourceLava)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FluidShapeKey {
    owner: FluidShapeOwner,
    amount: u8,
    falling: bool,
}
impl FluidShapeKey {
    fn from_state(state: FluidState) -> Result<Self> {
        let owner = match (state.kind, state.source) {
            (FluidKind::Water, true) => FluidShapeOwner::SourceWater,
            (FluidKind::Water, false) => FluidShapeOwner::FlowingWater,
            (FluidKind::Lava, true) => FluidShapeOwner::SourceLava,
            (FluidKind::Lava, false) => FluidShapeOwner::FlowingLava,
            (FluidKind::Empty, _) => bail!("empty fluid has no FlowingFluid shape-cache key"),
        };
        ensure!(
            (state.source && state.amount == 8)
                || (!state.source && (1..=8).contains(&state.amount)),
            "noncanonical vanilla fluid amount for shape cache"
        );
        ensure!(
            state.own_height.to_bits() == (state.amount as f32 / 9_f32).to_bits(),
            "noncanonical vanilla fluid own-height for shape cache"
        );
        Ok(Self {
            owner,
            amount: state.amount as u8,
            falling: state.falling,
        })
    }
    fn state(self) -> FluidState {
        FluidState {
            kind: self.owner.kind(),
            own_height: f32::from(self.amount) / 9_f32,
            amount: i32::from(self.amount),
            falling: self.falling,
            source: self.owner.source(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FluidShapeHistory {
    /// The original singleton's earlier shape queries have not been observed.
    Unknown,
    /// An actual cache snapshot established that this canonical key is absent.
    Unqueried,
    /// Height stored by the original first query, not the current cell height.
    CachedHeight(f32),
}

/// Explicit history of vanilla FlowingFluid.getShape's process-wide caches.
/// There is deliberately no Default or implicit fresh/empty runtime history.
#[derive(Debug)]
pub struct FluidShapeCache {
    entries: BTreeMap<FluidShapeKey, FluidShapeHistory>,
}
impl FluidShapeCache {
    pub fn unknown() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
    /// Import only observed facts. Omitted keys remain Unknown. The caller must
    /// retain this same cache for subsequent queries across items and worlds.
    pub fn from_observed_history(
        history: impl IntoIterator<Item = (FluidState, FluidShapeHistory)>,
    ) -> Result<Self> {
        let mut cache = Self::unknown();
        for (state, history) in history {
            let key = FluidShapeKey::from_state(state)?;
            if let FluidShapeHistory::CachedHeight(height) = history {
                ensure!(
                    height.to_bits() == state.own_height.to_bits()
                        || height.to_bits() == 1_f32.to_bits(),
                    "invalid observed vanilla fluid shape height"
                );
            }
            ensure!(
                cache.entries.insert(key, history).is_none(),
                "duplicate observed fluid shape key"
            );
        }
        Ok(cache)
    }
    /// Controlled bootstrap import: each supplied owner's actual original map
    /// must have been observed empty. This does not assert anything about
    /// omitted owners, an imported world's prehistory or an arbitrary runtime.
    pub fn from_observed_empty_owners(owners: impl IntoIterator<Item = FluidShapeOwner>) -> Self {
        let mut cache = Self::unknown();
        for owner in owners {
            for amount in 1..=8 {
                if owner.source() && amount != 8 {
                    continue;
                }
                for falling in [false, true] {
                    cache.entries.insert(
                        FluidShapeKey {
                            owner,
                            amount,
                            falling,
                        },
                        FluidShapeHistory::Unqueried,
                    );
                }
            }
        }
        cache
    }
    pub fn history(&self, state: FluidState) -> Result<FluidShapeHistory> {
        Ok(self
            .entries
            .get(&FluidShapeKey::from_state(state)?)
            .copied()
            .unwrap_or(FluidShapeHistory::Unknown))
    }
    pub fn observations(&self) -> impl Iterator<Item = (FluidState, FluidShapeHistory)> + '_ {
        self.entries
            .iter()
            .map(|(key, history)| (key.state(), *history))
    }
    /// Original getShape: the first query reads the above fluid and caches its
    /// f32 height under canonical FluidState identity. A hit makes no world
    /// query; an unknown history is an error, never a dynamic recomputation.
    pub fn shape(
        &mut self,
        world: &mut impl FluidWorld,
        position: BlockPos,
        state: FluidState,
    ) -> Result<VoxelShape> {
        if state.is_empty() {
            return Ok(VoxelShape::new(vec![0.], vec![0.], vec![0.], Vec::new())
                .expect("canonical empty voxel shape"));
        }
        let key = FluidShapeKey::from_state(state)?;
        let height = match self
            .entries
            .get(&key)
            .copied()
            .unwrap_or(FluidShapeHistory::Unknown)
        {
            FluidShapeHistory::Unknown => bail!("fluid shape cache history unknown for {key:?}"),
            FluidShapeHistory::CachedHeight(height) => height,
            FluidShapeHistory::Unqueried => {
                // An exception leaves the original computeIfAbsent map empty.
                let height = fluid_height(world, position, state)?;
                self.entries
                    .insert(key, FluidShapeHistory::CachedHeight(height));
                height
            }
        };
        Ok(VoxelShape::from_aabb(Aabb::new(
            0.,
            0.,
            0.,
            1.,
            f64::from(height),
            1.,
        )))
    }
}
type FactRow = (u8, u32, i32, bool, bool, bool, bool, u8, bool);
#[derive(Deserialize)]
struct Facts {
    states: Vec<usize>,
    rows: Vec<FactRow>,
}
fn facts() -> &'static Facts {
    static FACTS: OnceLock<Facts> = OnceLock::new();
    FACTS.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/fluid_physics_facts.json"))
            .expect("verified original fluid facts")
    })
}
pub fn block_facts(state: u32) -> Result<FluidBlockFacts> {
    let f = facts();
    let &(kind, height, amount, falling, source, blocks_motion, ice, faces, cached) = f
        .rows
        .get(
            *f.states
                .get(state as usize)
                .context("unknown exact1.21 fluid blockstate")?,
        )
        .context("invalid fluid fact index")?;
    Ok(FluidBlockFacts {
        fluid: FluidState {
            kind: match kind {
                0 => FluidKind::Empty,
                1 => FluidKind::Water,
                2 => FluidKind::Lava,
                _ => bail!("unknown fluid family"),
            },
            own_height: f32::from_bits(height),
            amount,
            falling,
            source,
        },
        blocks_motion,
        ice,
        cached_sturdy_faces: cached.then_some(faces),
    })
}
pub trait FluidWorld {
    /// Exact LevelReader.hasChunksAt rectangle, in block coordinates. This is
    /// availability, not permission to generate or lazily load missing chunks.
    fn has_chunks_at(&mut self, min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> Result<bool>;
    fn block(&mut self, position: BlockPos) -> Result<u32>;
    fn fluid(&mut self, position: BlockPos) -> Result<FluidState> {
        FluidState::from_block(self.block(position)?)
    }
    /// Default immutable vanilla fluid tags. A host with replaced datapack tags
    /// supplies their actual membership here.
    fn matches_tag(
        &mut self,
        _position: BlockPos,
        state: FluidState,
        tag: FluidTag,
    ) -> Result<bool> {
        Ok(state.matches(tag))
    }
    fn face_sturdy(
        &mut self,
        position: BlockPos,
        state: u32,
        direction: Direction,
    ) -> Result<bool> {
        let mask = block_facts(state)?
            .cached_sturdy_faces
            .with_context(|| format!("uncached live support face unavailable at {position:?}"))?;
        Ok(mask & (1 << direction as u8) != 0)
    }
    fn flow(&mut self, position: BlockPos, state: FluidState) -> Result<Vec3>
    where
        Self: Sized,
    {
        fluid_flow(self, position, state)
    }
}
pub fn fluid_height(
    world: &mut impl FluidWorld,
    position: BlockPos,
    state: FluidState,
) -> Result<f32> {
    if state.is_empty() {
        return Ok(0.);
    }
    Ok(
        if world
            .fluid((position.0, position.1.wrapping_add(1), position.2))?
            .kind
            == state.kind
        {
            1.
        } else {
            state.own_height
        },
    )
}
const HORIZONTAL: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];
fn step(p: BlockPos, direction: Direction) -> BlockPos {
    let (x, y, z) = direction.step();
    (
        p.0.wrapping_add(x),
        p.1.wrapping_add(y),
        p.2.wrapping_add(z),
    )
}
fn solid_face(
    world: &mut impl FluidWorld,
    position: BlockPos,
    direction: Direction,
    kind: FluidKind,
) -> Result<bool> {
    // Keep the original read order even though a provider may derive its fluid
    // from the same blockstate: getBlockState, then getFluidState.
    let state = world.block(position)?;
    let fluid = world.fluid(position)?;
    if fluid.kind == kind {
        return Ok(false);
    }
    if direction == Direction::Up {
        return Ok(true);
    }
    if block_facts(state)?.ice {
        return Ok(false);
    }
    world.face_sturdy(position, state, direction)
}
pub fn fluid_flow(
    world: &mut impl FluidWorld,
    position: BlockPos,
    state: FluidState,
) -> Result<Vec3> {
    if state.is_empty() {
        return Ok(Vec3::ZERO);
    }
    let mut x = 0.;
    let mut z = 0.;
    for direction in HORIZONTAL {
        let neighbor = step(position, direction);
        let next = world.fluid(neighbor)?;
        if !next.affects_flow_of(state.kind) {
            continue;
        }
        let mut height = next.own_height;
        let mut difference = 0f32;
        if height == 0. {
            if !block_facts(world.block(neighbor)?)?.blocks_motion {
                let below = world.fluid((neighbor.0, neighbor.1.wrapping_sub(1), neighbor.2))?;
                if below.affects_flow_of(state.kind) {
                    height = below.own_height;
                    if height > 0. {
                        difference = state.own_height - (height - 0.8888889f32);
                    }
                }
            }
        } else if height > 0. {
            difference = state.own_height - height;
        }
        // Multiplication happens as a float, before its promotion to double.
        if difference != 0. {
            let (dx, _, dz) = direction.step();
            x += f64::from(dx as f32 * difference);
            z += f64::from(dz as f32 * difference);
        }
    }
    let mut flow = Vec3::new(x, 0., z);
    if state.falling {
        for direction in HORIZONTAL {
            let neighbor = step(position, direction);
            if solid_face(world, neighbor, direction, state.kind)?
                || solid_face(
                    world,
                    (neighbor.0, neighbor.1.wrapping_add(1), neighbor.2),
                    direction,
                    state.kind,
                )?
            {
                flow = flow.normalize().plus(Vec3::new(0., -6., 0.));
                break;
            }
        }
    }
    Ok(flow.normalize())
}

#[derive(Clone, Copy, Debug)]
pub struct FluidBody {
    pub bounds: Aabb,
    pub velocity: Vec3,
    pub pushed_by_fluid: bool,
    pub is_player: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidPush {
    pub touching: bool,
    pub velocity: Vec3,
    /// None is the touchingUnloadedChunk early return; retain the prior map
    /// entry. A loaded empty query explicitly writes Some(0).
    pub height_written: Option<f64>,
}
/// Entity.updateFluidHeightAndDoFluidPushing, including its running maximum
/// depth, original X/Y/Z loop order, and player/nonplayer current normalization.
pub fn update_fluid_pushing(
    world: &mut impl FluidWorld,
    body: FluidBody,
    tag: FluidTag,
    factor: f64,
) -> Result<FluidPush> {
    ensure!(finite_bounds(body.bounds), "nonfinite entity fluid bounds");
    let margin = body.bounds.inflate(Vec3::new(1., 1., 1.));
    if !world.has_chunks_at(
        java_floor(margin.min_x),
        java_floor(margin.min_z),
        java_ceil(margin.max_x),
        java_ceil(margin.max_z),
    )? {
        return Ok(FluidPush {
            touching: false,
            velocity: body.velocity,
            height_written: None,
        });
    }
    let bounds = body.bounds.deflate(Vec3::new(0.001, 0.001, 0.001));
    let min = (
        java_floor(bounds.min_x),
        java_floor(bounds.min_y),
        java_floor(bounds.min_z),
    );
    let max = (
        java_ceil(bounds.max_x),
        java_ceil(bounds.max_y),
        java_ceil(bounds.max_z),
    );
    let volume = [
        i64::from(max.0) - i64::from(min.0),
        i64::from(max.1) - i64::from(min.1),
        i64::from(max.2) - i64::from(min.2),
    ]
    .into_iter()
    .try_fold(1u64, |n, v| n.checked_mul(v.max(0) as u64))
    .context("fluid query range overflow")?;
    ensure!(
        volume <= 1_048_576,
        "fluid query exceeds available bounded context"
    );
    let mut height = 0f64;
    let mut touching = false;
    let mut velocity = Vec3::ZERO;
    let mut count = 0i32;
    for x in min.0..max.0 {
        for y in min.1..max.1 {
            for z in min.2..max.2 {
                let p = (x, y, z);
                let fluid = world.fluid(p)?;
                if !world.matches_tag(p, fluid, tag)? {
                    continue;
                }
                let surface = f64::from(y as f32 + fluid_height(world, p, fluid)?);
                if surface >= bounds.min_y {
                    touching = true;
                    height = java_max(surface - bounds.min_y, height);
                    if body.pushed_by_fluid {
                        let mut flow = world.flow(p, fluid)?;
                        if height < 0.4 {
                            flow = flow.scale(height);
                        }
                        velocity = velocity.plus(flow);
                        count = count.wrapping_add(1);
                    }
                }
            }
        }
    }
    let mut current = body.velocity;
    if velocity.length() > 0. {
        if count > 0 {
            velocity = velocity.scale(1. / f64::from(count));
        }
        if !body.is_player {
            velocity = velocity.normalize();
        }
        velocity = velocity.scale(factor);
        if current.x.abs() < 0.003
            && current.z.abs() < 0.003
            && velocity.length() < 0.0045000000000000005
        {
            velocity = velocity.normalize().scale(0.0045000000000000005);
        }
        current = current.plus(velocity);
    }
    Ok(FluidPush {
        touching,
        velocity: current,
        height_written: Some(height),
    })
}
fn finite_bounds(b: Aabb) -> bool {
    [b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z]
        .iter()
        .all(|v| v.is_finite())
}
fn java_floor(v: f64) -> i32 {
    let n = v as i32;
    if v < f64::from(n) {
        n.wrapping_sub(1)
    } else {
        n
    }
}
fn java_ceil(v: f64) -> i32 {
    let n = v as i32;
    if v > f64::from(n) {
        n.wrapping_add(1)
    } else {
        n
    }
}
fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0. && b == 0. {
        if a.is_sign_positive() || b.is_sign_positive() {
            0.
        } else {
            -0.
        }
    } else if a > b {
        a
    } else {
        b
    }
}

/// Native adapter uses the loaded chunk cache for hasChunksAt. It never reads
/// an unloaded Anvil chunk merely to make a fluid query appear available.
pub struct RuntimeFluidWorld<'a> {
    pub world: &'a mut RuntimeWorld,
}
impl FluidWorld for RuntimeFluidWorld<'_> {
    fn has_chunks_at(&mut self, min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> Result<bool> {
        match self.world {
            RuntimeWorld::Fixture(_) => Ok(true),
            RuntimeWorld::Anvil(world) => {
                for x in min_x.div_euclid(16)..=max_x.div_euclid(16) {
                    for z in min_z.div_euclid(16)..=max_z.div_euclid(16) {
                        if !world.cached_chunk_is_loaded(x, z) {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
        }
    }
    fn block(&mut self, p: BlockPos) -> Result<u32> {
        match self.world {
            RuntimeWorld::Fixture(world) => Ok(world.block(p.0, p.1, p.2)),
            RuntimeWorld::Anvil(world) => world
                .loaded_block_state(p.0, p.1, p.2)
                .with_context(|| format!("live fluid chunk unavailable at {p:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    fn d(value: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
    }
    fn v(value: &Value) -> Vec3 {
        Vec3::new(d(&value[0]), d(&value[1]), d(&value[2]))
    }
    fn b(value: &Value) -> Aabb {
        Aabb::new(
            d(&value[0]),
            d(&value[1]),
            d(&value[2]),
            d(&value[3]),
            d(&value[4]),
            d(&value[5]),
        )
    }
    fn bits(value: f64) -> String {
        format!(
            "{:016x}",
            if value.is_nan() {
                f64::NAN.to_bits()
            } else {
                value.to_bits()
            }
        )
    }
    fn vector(value: Vec3) -> Value {
        json!([bits(value.x), bits(value.y), bits(value.z)])
    }
    #[derive(Default)]
    struct TestWorld {
        blocks: BTreeMap<BlockPos, u32>,
        loaded: bool,
        events: Vec<String>,
    }
    impl FluidWorld for TestWorld {
        fn has_chunks_at(&mut self, a: i32, b: i32, c: i32, d: i32) -> Result<bool> {
            self.events.push(format!("chunks:{a},{b},{c},{d}"));
            Ok(self.loaded)
        }
        fn block(&mut self, p: BlockPos) -> Result<u32> {
            self.events.push(format!("block:{},{},{}", p.0, p.1, p.2));
            Ok(self.blocks.get(&p).copied().unwrap_or(0))
        }
        fn fluid(&mut self, p: BlockPos) -> Result<FluidState> {
            self.events.push(format!("fluid:{},{},{}", p.0, p.1, p.2));
            FluidState::from_block(self.blocks.get(&p).copied().unwrap_or(0))
        }
    }
    #[test]
    fn original_java_fluid_height_current_pushing_and_world_query_order() {
        let vectors: Value =
            serde_json::from_str(include_str!("../fixtures/fluid_physics_vectors.json")).unwrap();
        for (index, case) in vectors["cases"].as_array().unwrap().iter().enumerate() {
            let q = &case["query"];
            let mut world = TestWorld {
                loaded: q["loaded"].as_bool().unwrap_or(true),
                ..TestWorld::default()
            };
            for row in q["blocks"].as_array().unwrap() {
                world.blocks.insert(
                    (
                        row[0].as_i64().unwrap() as i32,
                        row[1].as_i64().unwrap() as i32,
                        row[2].as_i64().unwrap() as i32,
                    ),
                    row[3].as_u64().unwrap() as u32,
                );
            }
            let mut out = match q["op"].as_str().unwrap() {
                "push" => {
                    let result = update_fluid_pushing(
                        &mut world,
                        FluidBody {
                            bounds: b(&q["bounds"]),
                            velocity: v(&q["velocity"]),
                            pushed_by_fluid: q["pushed"].as_bool().unwrap(),
                            is_player: q["player"].as_bool().unwrap(),
                        },
                        if q["tag"] == 1 {
                            FluidTag::Water
                        } else {
                            FluidTag::Lava
                        },
                        d(&q["factor"]),
                    )
                    .unwrap();
                    json!({"touching":result.touching,"velocity":vector(result.velocity),"height_written":result.height_written.map(bits),"stored_height":bits(result.height_written.unwrap_or_else(||d(&q["previous_height"])))})
                }
                op => {
                    let p = (
                        q["position"][0].as_i64().unwrap() as i32,
                        q["position"][1].as_i64().unwrap() as i32,
                        q["position"][2].as_i64().unwrap() as i32,
                    );
                    let state = world.fluid(p).unwrap();
                    match op {
                        "flow" => json!({"flow":vector(fluid_flow(&mut world,p,state).unwrap())}),
                        "height" => {
                            json!({"height":format!("{:08x}",fluid_height(&mut world,p,state).unwrap().to_bits())})
                        }
                        "face" => {
                            json!({"solid":solid_face(&mut world,p,[Direction::Down,Direction::Up,Direction::North,Direction::South,Direction::West,Direction::East][q["direction"].as_u64().unwrap()as usize],if q["tag"]==1{FluidKind::Water}else{FluidKind::Lava}).unwrap()})
                        }
                        _ => panic!("unknown original fluid operation {op}"),
                    }
                }
            };
            out["events"] = json!(world.events);
            assert_eq!(out, case["expected"], "original fluid case{index} query{q}");
        }
    }

    #[test]
    fn unavailable_support_faces_are_not_guessed_and_unloaded_chunks_keep_height_untouched() {
        let state = (0..26684)
            .find(|state| {
                block_facts(*state).unwrap().cached_sturdy_faces.is_none()
                    && block_facts(*state).unwrap().fluid.is_empty()
            })
            .unwrap();
        let mut world = TestWorld::default();
        world.blocks.insert((0, 64, 0), state);
        assert!(
            solid_face(&mut world, (0, 64, 0), Direction::East, FluidKind::Water)
                .unwrap_err()
                .to_string()
                .contains("uncached live support face unavailable")
        );
        world.events.clear();
        let velocity = Vec3::new(0.1, -0.2, 0.3);
        let result = update_fluid_pushing(
            &mut world,
            FluidBody {
                bounds: Aabb::player(Vec3::new(0., 64., 0.), 0.6, 1.8),
                velocity,
                pushed_by_fluid: true,
                is_player: false,
            },
            FluidTag::Water,
            0.014,
        )
        .unwrap();
        assert_eq!(
            result,
            FluidPush {
                touching: false,
                velocity,
                height_written: None
            }
        );
        assert_eq!(world.events.len(), 1);
        assert!(world.events[0].starts_with("chunks:"));
    }
}
