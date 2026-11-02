//! Typed Minecraft 1.21 density graphs and RandomState seed bindings.
//!
//! Raw evaluation preserves DensityFunctions marker nodes as pass-through nodes,
//! exactly as RandomState.router() does before NoiseChunk wraps them. This is
//! not a chunk generator or a claim that interpolation/blending is complete.
mod chunk;
pub use chunk::{ChunkDensity, ChunkDensityError, ChunkDensityState};

use crate::{
    noise::{BlendedNoise, NoiseParameters, NormalNoise, SimplexNoise},
    random::{LegacyRandomSource, PositionalRandomFactory, XoroshiroRandomSource},
};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Arc, OnceLock},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DensityBounds {
    pub min: f64,
    pub max: f64,
}
impl DensityBounds {
    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }
    pub const fn constant(value: f64) -> Self {
        Self::new(value, value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DensityPoint {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl DensityPoint {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClimateTargetPoint {
    pub temperature: i64,
    pub humidity: i64,
    pub continentalness: i64,
    pub erosion: i64,
    pub depth: i64,
    pub weirdness: i64,
}
impl ClimateTargetPoint {
    pub fn parameter_array(self) -> [i64; 7] {
        [
            self.temperature,
            self.humidity,
            self.continentalness,
            self.erosion,
            self.depth,
            self.weirdness,
            0,
        ]
    }
}
pub fn quantize_climate_coord(value: f32) -> i64 {
    (value * 10_000.0_f32) as i64
}
pub fn unquantize_climate_coord(value: i64) -> f32 {
    value as f32 / 10_000.0_f32
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseGrid {
    pub min_y: i32,
    pub height: i32,
    pub cell_width: i32,
    pub cell_height: i32,
}
/// The fifteen functions of the seeded `NoiseRouter`, in its record order.
///
/// Node IDs belong to the graph that returned this router. They retain marker
/// nodes: chunk owners must sample through `ChunkDensity` for interpolation and
/// caches, rather than treating raw graph evaluation as generated terrain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DensityRouter {
    pub barrier: NodeId,
    pub fluid_level_floodedness: NodeId,
    pub fluid_level_spread: NodeId,
    pub lava: NodeId,
    pub temperature: NodeId,
    pub vegetation: NodeId,
    pub continents: NodeId,
    pub erosion: NodeId,
    pub depth: NodeId,
    pub ridges: NodeId,
    pub initial_density_without_jaggedness: NodeId,
    pub final_density: NodeId,
    pub vein_toggle: NodeId,
    pub vein_ridged: NodeId,
    pub vein_gap: NodeId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerKind {
    Interpolated,
    FlatCache,
    Cache2D,
    CacheOnce,
    CacheAllInCell,
}
#[derive(Debug, Clone, Copy)]
enum Binary {
    Add,
    Mul,
    Min,
    Max,
}
#[derive(Debug, Clone, Copy)]
enum Mapped {
    Abs,
    Square,
    Cube,
    HalfNegative,
    QuarterNegative,
    Squeeze,
}
#[derive(Debug, Clone, Copy)]
enum Shift {
    A,
    B,
    All,
}
#[derive(Debug, Clone, Copy)]
enum Rarity {
    Type1,
    Type2,
}
#[derive(Debug, Clone)]
struct SplinePoint {
    location: f32,
    value: Spline,
    derivative: f32,
}
#[derive(Debug, Clone)]
enum Spline {
    Constant(f32),
    Multipoint {
        coordinate: NodeId,
        identity: usize,
        points: Vec<SplinePoint>,
        min: f32,
        max: f32,
    },
}
impl Spline {
    fn bounds(&self) -> (f32, f32) {
        match self {
            Self::Constant(value) => (*value, *value),
            Self::Multipoint { min, max, .. } => (*min, *max),
        }
    }
}
#[derive(Debug, Clone)]
enum DensityNode {
    Constant(f64),
    Reference(NodeId),
    Marker(MarkerKind, NodeId),
    Binary(Binary, NodeId, NodeId),
    ConstantArgument(Binary, NodeId, f64),
    Mapped(Mapped, NodeId),
    Clamp {
        input: NodeId,
        min: f64,
        max: f64,
    },
    RangeChoice {
        input: NodeId,
        min: f64,
        max: f64,
        inside: NodeId,
        outside: NodeId,
    },
    YGradient {
        from_y: i32,
        to_y: i32,
        from_value: f64,
        to_value: f64,
    },
    Noise {
        noise: usize,
        xz_scale: f64,
        y_scale: f64,
    },
    Shift {
        noise: usize,
        kind: Shift,
    },
    ShiftedNoise {
        noise: usize,
        x: NodeId,
        y: NodeId,
        z: NodeId,
        xz_scale: f64,
        y_scale: f64,
    },
    WeirdScaled {
        input: NodeId,
        noise: usize,
        rarity: Rarity,
    },
    Spline(Spline),
    Blended(usize),
    EndIslands,
    BlendAlpha,
    BlendOffset,
    Beardifier,
    BlendDensity(NodeId),
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
fn min32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f32::from_bits(a.to_bits() | b.to_bits())
    } else if a <= b {
        a
    } else {
        b
    }
}
fn max32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f32::from_bits(a.to_bits() & b.to_bits())
    } else if a >= b {
        a
    } else {
        b
    }
}
fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else {
        java_min(value, max)
    }
}
fn clamp32(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else {
        min32(value, max)
    }
}
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}
fn lerp32(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}
fn clamped_lerp(a: f64, b: f64, t: f64) -> f64 {
    if t < 0.0 {
        a
    } else if t > 1.0 {
        b
    } else {
        lerp(t, a, b)
    }
}
fn apply_mapped(kind: Mapped, value: f64) -> f64 {
    match kind {
        Mapped::Abs => value.abs(),
        Mapped::Square => value * value,
        Mapped::Cube => value * value * value,
        Mapped::HalfNegative => {
            if value > 0.0 {
                value
            } else {
                value * 0.5
            }
        }
        Mapped::QuarterNegative => {
            if value > 0.0 {
                value
            } else {
                value * 0.25
            }
        }
        Mapped::Squeeze => {
            let value = clamp(value, -1.0, 1.0);
            value / 2.0 - value * value * value / 24.0
        }
    }
}
fn rarity(kind: Rarity, value: f64) -> f64 {
    match kind {
        Rarity::Type1 => {
            if value < -0.5 {
                0.75
            } else if value < 0.0 {
                1.0
            } else if value < 0.5 {
                1.5
            } else {
                2.0
            }
        }
        Rarity::Type2 => {
            if value < -0.75 {
                0.5
            } else if value < -0.5 {
                0.75
            } else if value < 0.5 {
                1.0
            } else if value < 0.75 {
                2.0
            } else {
                3.0
            }
        }
    }
}
fn canonical(name: &str) -> String {
    match name.split_once(':') {
        Some(("", path)) => format!("minecraft:{path}"),
        Some(_) => name.to_owned(),
        None => format!("minecraft:{name}"),
    }
}
fn number(value: &Value, key: &str) -> Result<f64> {
    value[key]
        .as_f64()
        .ok_or_else(|| anyhow!("density field {key} must be numeric"))
}
fn integer(value: &Value, key: &str) -> Result<i32> {
    value[key]
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| anyhow!("density field {key} must be an int"))
}
fn name<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| anyhow!("density field {key} must be a string"))
}

/// Evaluation context used when NoiseChunk replaces marker records. Raw
/// contexts delegate markers; chunk contexts own their cache/counter state.
pub trait DensityContext: Sized {
    type Error;
    /// Metadata of the actual visited function. NoiseChunk's Blender wrappers
    /// change child bounds without changing the shared RandomState graph.
    fn bounds(&self, graph: &DensityGraph, id: NodeId) -> DensityBounds {
        graph.bounds(id)
    }
    /// A visitor may replace an ADD/MUL node with MulOrAdd after erasing its
    /// holder arguments. The input is evaluated before applying the constant,
    /// even when that constant is zero. Raw RandomState contexts do not perform
    /// this NoiseChunk rewrite. This describes the wrapped function, regardless
    /// of whether its sampling FunctionContext belongs to the chunk.
    fn rewritten_constant_argument(&self, _id: NodeId) -> Option<(NodeId, f64)> {
        None
    }
    fn marker(
        &mut self,
        graph: &DensityGraph,
        id: NodeId,
        kind: MarkerKind,
        input: NodeId,
        point: DensityPoint,
    ) -> std::result::Result<f64, Self::Error>;
    fn marker_fill(
        &mut self,
        graph: &DensityGraph,
        _id: NodeId,
        _kind: MarkerKind,
        input: NodeId,
        values: &mut [f64],
    ) -> std::result::Result<(), Self::Error> {
        graph.fill_sampled(input, values, self)
    }
    fn for_index(&mut self, index: usize) -> DensityPoint;
    fn fill_direct(
        &mut self,
        graph: &DensityGraph,
        id: NodeId,
        values: &mut [f64],
    ) -> std::result::Result<(), Self::Error> {
        for (index, value) in values.iter_mut().enumerate() {
            let point = self.for_index(index);
            *value = graph.eval_sampled(id, point, self)?;
        }
        Ok(())
    }
    fn blend_density(&mut self, _point: DensityPoint, value: f64) -> f64 {
        value
    }
    fn blend_alpha(&mut self, _id: NodeId, _point: DensityPoint) -> f64 {
        1.0
    }
    fn blend_offset(&mut self, _id: NodeId, _point: DensityPoint) -> f64 {
        0.0
    }
    /// The raw BeardifierMarker fills zero without visiting provider indices.
    /// NoiseChunk replaces it with an actual Beardifier, whose SimpleFunction
    /// fill uses the provider even when the selected structure lists are empty.
    fn beardifier_fill(
        &mut self,
        _graph: &DensityGraph,
        _id: NodeId,
        values: &mut [f64],
    ) -> std::result::Result<(), Self::Error> {
        values.fill(0.0);
        Ok(())
    }
    /// Raw/empty BlendAlpha and BlendOffset use Arrays.fill, without visiting
    /// provider indices. Nonempty NoiseChunk FlatCache wrappers override these.
    fn blend_alpha_fill(
        &mut self,
        _graph: &DensityGraph,
        _id: NodeId,
        values: &mut [f64],
    ) -> std::result::Result<(), Self::Error> {
        values.fill(1.0);
        Ok(())
    }
    fn blend_offset_fill(
        &mut self,
        _graph: &DensityGraph,
        _id: NodeId,
        values: &mut [f64],
    ) -> std::result::Result<(), Self::Error> {
        values.fill(0.0);
        Ok(())
    }
}
struct RawDensityContext<'a> {
    points: &'a [DensityPoint],
}
impl DensityContext for RawDensityContext<'_> {
    type Error = std::convert::Infallible;
    fn marker(
        &mut self,
        graph: &DensityGraph,
        _id: NodeId,
        _kind: MarkerKind,
        input: NodeId,
        point: DensityPoint,
    ) -> std::result::Result<f64, Self::Error> {
        graph.eval_sampled(input, point, self)
    }
    fn for_index(&mut self, index: usize) -> DensityPoint {
        self.points[index]
    }
}

/// Immutable graph after parsing. Evaluation performs no allocations; all noise
/// instances and registry references are resolved and shared at construction.
#[derive(Debug)]
pub struct DensityGraph {
    seed: i64,
    legacy: bool,
    factory: PositionalRandomFactory,
    nodes: Vec<DensityNode>,
    bounds: Vec<DensityBounds>,
    definitions: Arc<BTreeMap<String, Value>>,
    parsed_definitions: BTreeMap<String, NodeId>,
    references: BTreeMap<String, NodeId>,
    resolving: BTreeSet<String>,
    parameters: Arc<BTreeMap<String, NoiseParameters>>,
    noises: Vec<NormalNoise>,
    noise_ids: HashMap<String, usize>,
    blends: Vec<BlendedNoise>,
    end_noise: Option<SimplexNoise>,
    roots: BTreeMap<String, NodeId>,
    grid: Option<NoiseGrid>,
    block_density: Option<NodeId>,
    next_spline_identity: usize,
}
impl DensityGraph {
    pub fn new(
        seed: i64,
        legacy: bool,
        definitions: BTreeMap<String, Value>,
        parameters: BTreeMap<String, NoiseParameters>,
    ) -> Self {
        let factory = if legacy {
            PositionalRandomFactory::Legacy(LegacyRandomSource::new(seed).fork_positional())
        } else {
            PositionalRandomFactory::Xoroshiro(XoroshiroRandomSource::new(seed).fork_positional())
        };
        Self {
            seed,
            legacy,
            factory,
            nodes: Vec::new(),
            bounds: Vec::new(),
            definitions: Arc::new(definitions),
            parsed_definitions: BTreeMap::new(),
            references: BTreeMap::new(),
            resolving: BTreeSet::new(),
            parameters: Arc::new(parameters),
            noises: Vec::new(),
            noise_ids: HashMap::new(),
            blends: Vec::new(),
            end_noise: None,
            roots: BTreeMap::new(),
            grid: None,
            block_density: None,
            next_spline_identity: 0,
        }
    }
    pub fn builtin(seed: i64, settings: &str) -> Result<Self> {
        static DATA: OnceLock<Value> = OnceLock::new();
        static PARAMETERS: OnceLock<Value> = OnceLock::new();
        let data = DATA.get_or_init(|| {
            serde_json::from_str(include_str!("../fixtures/density_builtin-1.21.json"))
                .expect("packaged density data")
        });
        let parameters = PARAMETERS.get_or_init(|| {
            serde_json::from_str(include_str!("../fixtures/noise_parameters-1.21.json"))
                .expect("packaged noise parameters")
        });
        let settings = canonical(settings);
        let setting = data["settings"]
            .get(&settings)
            .ok_or_else(|| anyhow!("unknown noise settings {settings}"))?;
        let definitions = data["definitions"]
            .as_object()
            .context("density definitions must be an object")?
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        let parameters = parameters["noises"]
            .as_object()
            .context("noise parameters must be an object")?
            .iter()
            .map(|(key, value)| {
                let first_octave = integer(value, "first_octave")?;
                let amplitudes = value["amplitudes"]
                    .as_array()
                    .context("noise amplitudes must be an array")?
                    .iter()
                    .map(|v| v.as_f64().context("invalid amplitude"))
                    .collect::<Result<Vec<_>>>()?;
                Ok((
                    key.clone(),
                    NoiseParameters {
                        first_octave,
                        amplitudes,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut graph = Self::new(
            seed,
            setting["legacy_random_source"]
                .as_bool()
                .context("missing legacy setting")?,
            definitions,
            parameters,
        );
        let noise = &setting["noise"];
        graph.grid = Some(NoiseGrid {
            min_y: integer(noise, "min_y")?,
            height: integer(noise, "height")?,
            cell_width: integer(noise, "size_horizontal")?.wrapping_mul(4),
            cell_height: integer(noise, "size_vertical")?.wrapping_mul(4),
        });
        for (key, value) in setting["noise_router"]
            .as_object()
            .context("noise router must be an object")?
        {
            let id = graph.parse(value)?;
            graph.roots.insert(key.clone(), id);
        }
        if let Some(final_density) = graph.root("final_density") {
            let beard = graph.push(DensityNode::Beardifier, DensityBounds::constant(0.0));
            let sum = graph.binary(Binary::Add, final_density, beard);
            graph.block_density = Some(graph.push(
                DensityNode::Marker(MarkerKind::CacheAllInCell, sum),
                graph.bounds(sum),
            ));
        }
        Ok(graph)
    }
    /// Climate.Sampler accepts quart coordinates and narrows each density
    /// result to float before multiplying by10000 and narrowing to long.
    pub fn sample_climate(
        &self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
    ) -> Result<ClimateTargetPoint> {
        let point = DensityPoint::new(
            quart_x.wrapping_shl(2),
            quart_y.wrapping_shl(2),
            quart_z.wrapping_shl(2),
        );
        let mut values = [0; 6];
        for (i, key) in [
            "temperature",
            "vegetation",
            "continents",
            "erosion",
            "depth",
            "ridges",
        ]
        .iter()
        .enumerate()
        {
            let node = self
                .root(key)
                .ok_or_else(|| anyhow!("missing climate density root {key}"))?;
            values[i] = quantize_climate_coord(self.eval(node, point) as f32);
        }
        Ok(ClimateTargetPoint {
            temperature: values[0],
            humidity: values[1],
            continentalness: values[2],
            erosion: values[3],
            depth: values[4],
            weirdness: values[5],
        })
    }
    pub fn roots(&self) -> &BTreeMap<String, NodeId> {
        &self.roots
    }
    /// Return all original router fields, rejecting a partial/custom graph.
    /// This does not wrap its functions for any chunk or supply biome,
    /// structure-beardifier or nonempty Blender state.
    pub fn router(&self) -> Result<DensityRouter> {
        let required = |name: &str| {
            self.root(name)
                .ok_or_else(|| anyhow!("missing noise router field {name}"))
        };
        Ok(DensityRouter {
            barrier: required("barrier")?,
            fluid_level_floodedness: required("fluid_level_floodedness")?,
            fluid_level_spread: required("fluid_level_spread")?,
            lava: required("lava")?,
            temperature: required("temperature")?,
            vegetation: required("vegetation")?,
            continents: required("continents")?,
            erosion: required("erosion")?,
            depth: required("depth")?,
            ridges: required("ridges")?,
            initial_density_without_jaggedness: required("initial_density_without_jaggedness")?,
            final_density: required("final_density")?,
            vein_toggle: required("vein_toggle")?,
            vein_ridged: required("vein_ridged")?,
            vein_gap: required("vein_gap")?,
        })
    }
    /// World seed used to bind the graph's noise holders and terrain noise.
    pub fn seed(&self) -> i64 {
        self.seed
    }
    pub fn uses_legacy_random_source(&self) -> bool {
        self.legacy
    }
    /// RandomState's positional root after the algorithm(seed) positional fork.
    /// Copying this immutable factory consumes no sequential random state.
    /// Aquifer, ore and surface owners derive their own named streams from it.
    pub fn positional_random_factory(&self) -> PositionalRandomFactory {
        self.factory
    }
    pub fn root(&self, name: &str) -> Option<NodeId> {
        self.roots.get(name).copied()
    }
    pub fn block_density(&self) -> Option<NodeId> {
        self.block_density
    }
    pub fn grid(&self) -> Option<NoiseGrid> {
        self.grid
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn bounds(&self, id: NodeId) -> DensityBounds {
        self.bounds[id.0]
    }
    pub fn marker(&self, id: NodeId) -> Option<(MarkerKind, NodeId)> {
        match self.nodes.get(id.0)? {
            DensityNode::Marker(kind, input) => Some((*kind, *input)),
            _ => None,
        }
    }
    pub fn definition(&mut self, name: &str) -> Result<NodeId> {
        let key = canonical(name);
        if let Some(id) = self.parsed_definitions.get(&key) {
            return Ok(*id);
        }
        if !self.resolving.insert(key.clone()) {
            bail!("cyclic density function reference {key}");
        }
        let value = self
            .definitions
            .get(&key)
            .cloned()
            .ok_or_else(|| anyhow!("unknown density function {key}"))?;
        let result = self.parse(&value);
        self.resolving.remove(&key);
        let id = result?;
        self.parsed_definitions.insert(key, id);
        Ok(id)
    }
    fn reference(&mut self, key: &str) -> Result<NodeId> {
        let key = canonical(key);
        if let Some(id) = self.references.get(&key) {
            return Ok(*id);
        }
        let target = self.definition(&key)?;
        let id = self.push(DensityNode::Reference(target), self.bounds(target));
        self.references.insert(key, id);
        Ok(id)
    }
    fn push(&mut self, node: DensityNode, bounds: DensityBounds) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        self.bounds.push(bounds);
        id
    }
    fn noise(&mut self, key: &str) -> Result<usize> {
        let key = canonical(key);
        let fresh = self.legacy
            && matches!(
                key.as_str(),
                "minecraft:temperature" | "minecraft:vegetation" | "minecraft:offset"
            );
        if !fresh {
            if let Some(index) = self.noise_ids.get(&key) {
                return Ok(*index);
            }
        }
        let noise =
            if self.legacy && (key == "minecraft:temperature" || key == "minecraft:vegetation") {
                let mut random = LegacyRandomSource::new(
                    self.seed
                        .wrapping_add(i64::from(key == "minecraft:vegetation")),
                );
                NormalNoise::legacy(
                    &mut random,
                    NoiseParameters {
                        first_octave: -7,
                        amplitudes: vec![1.0, 1.0],
                    },
                )?
            } else if self.legacy && key == "minecraft:offset" {
                NormalNoise::new(
                    &mut self.factory.from_hash_of(&key),
                    NoiseParameters {
                        first_octave: 0,
                        amplitudes: vec![0.0],
                    },
                )
            } else {
                let parameters = self
                    .parameters
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| anyhow!("unknown noise parameters {key}"))?;
                NormalNoise::new(&mut self.factory.from_hash_of(&key), parameters)
            };
        let id = self.noises.len();
        self.noises.push(noise);
        if !fresh {
            self.noise_ids.insert(key, id);
        }
        Ok(id)
    }
    fn noise_bounds(&self, index: usize, factor: f64) -> DensityBounds {
        let max = self.noises[index].max_value() * factor;
        DensityBounds::new(-max, max)
    }
    pub fn parse(&mut self, value: &Value) -> Result<NodeId> {
        if let Some(value) = value.as_f64() {
            return Ok(self.push(DensityNode::Constant(value), DensityBounds::constant(value)));
        }
        if let Some(key) = value.as_str() {
            return self.reference(key);
        }
        let kind = canonical(name(value, "type")?);
        let (kind, bounds) = match kind.as_str() {
            "minecraft:constant" => {
                let v = number(value, "argument")?;
                (DensityNode::Constant(v), DensityBounds::constant(v))
            }
            "minecraft:blend_alpha" => (DensityNode::BlendAlpha, DensityBounds::constant(1.0)),
            "minecraft:blend_offset" => (DensityNode::BlendOffset, DensityBounds::constant(0.0)),
            "minecraft:beardifier" => (DensityNode::Beardifier, DensityBounds::constant(0.0)),
            "minecraft:interpolated"
            | "minecraft:flat_cache"
            | "minecraft:cache_2d"
            | "minecraft:cache_once"
            | "minecraft:cache_all_in_cell" => {
                let marker = match kind.as_str() {
                    "minecraft:interpolated" => MarkerKind::Interpolated,
                    "minecraft:flat_cache" => MarkerKind::FlatCache,
                    "minecraft:cache_2d" => MarkerKind::Cache2D,
                    "minecraft:cache_once" => MarkerKind::CacheOnce,
                    _ => MarkerKind::CacheAllInCell,
                };
                let input = self.parse_argument(&value["argument"])?;
                (DensityNode::Marker(marker, input), self.bounds(input))
            }
            "minecraft:add" | "minecraft:mul" | "minecraft:min" | "minecraft:max" => {
                let kind = match kind.as_str() {
                    "minecraft:add" => Binary::Add,
                    "minecraft:mul" => Binary::Mul,
                    "minecraft:min" => Binary::Min,
                    _ => Binary::Max,
                };
                let a = self.parse_argument(&value["argument1"])?;
                let b = self.parse_argument(&value["argument2"])?;
                return Ok(self.binary(kind, a, b));
            }
            "minecraft:abs"
            | "minecraft:square"
            | "minecraft:cube"
            | "minecraft:half_negative"
            | "minecraft:quarter_negative"
            | "minecraft:squeeze" => {
                let kind = match kind.as_str() {
                    "minecraft:abs" => Mapped::Abs,
                    "minecraft:square" => Mapped::Square,
                    "minecraft:cube" => Mapped::Cube,
                    "minecraft:half_negative" => Mapped::HalfNegative,
                    "minecraft:quarter_negative" => Mapped::QuarterNegative,
                    _ => Mapped::Squeeze,
                };
                let input = self.parse_argument(&value["argument"])?;
                let before = self.bounds(input);
                let a = apply_mapped(kind, before.min);
                let b = apply_mapped(kind, before.max);
                let bounds = if matches!(kind, Mapped::Abs | Mapped::Square) {
                    DensityBounds::new(java_max(0.0, before.min), java_max(a, b))
                } else {
                    DensityBounds::new(a, b)
                };
                (DensityNode::Mapped(kind, input), bounds)
            }
            "minecraft:clamp" => {
                let input = self.parse_argument(&value["input"])?;
                let min = number(value, "min")?;
                let max = number(value, "max")?;
                (
                    DensityNode::Clamp { input, min, max },
                    DensityBounds::new(min, max),
                )
            }
            "minecraft:range_choice" => {
                let input = self.parse_argument(&value["input"])?;
                let inside = self.parse_argument(&value["when_in_range"])?;
                let outside = self.parse_argument(&value["when_out_of_range"])?;
                let a = self.bounds(inside);
                let b = self.bounds(outside);
                (
                    DensityNode::RangeChoice {
                        input,
                        min: number(value, "min_inclusive")?,
                        max: number(value, "max_exclusive")?,
                        inside,
                        outside,
                    },
                    DensityBounds::new(java_min(a.min, b.min), java_max(a.max, b.max)),
                )
            }
            "minecraft:y_clamped_gradient" => {
                let from_value = number(value, "from_value")?;
                let to_value = number(value, "to_value")?;
                (
                    DensityNode::YGradient {
                        from_y: integer(value, "from_y")?,
                        to_y: integer(value, "to_y")?,
                        from_value,
                        to_value,
                    },
                    DensityBounds::new(
                        java_min(from_value, to_value),
                        java_max(from_value, to_value),
                    ),
                )
            }
            "minecraft:noise" => {
                let noise = self.noise(name(value, "noise")?)?;
                (
                    DensityNode::Noise {
                        noise,
                        xz_scale: number(value, "xz_scale")?,
                        y_scale: number(value, "y_scale")?,
                    },
                    self.noise_bounds(noise, 1.0),
                )
            }
            "minecraft:shift_a" | "minecraft:shift_b" | "minecraft:shift" => {
                let noise = self.noise(name(value, "argument")?)?;
                let kind = match kind.as_str() {
                    "minecraft:shift_a" => Shift::A,
                    "minecraft:shift_b" => Shift::B,
                    _ => Shift::All,
                };
                (
                    DensityNode::Shift { noise, kind },
                    self.noise_bounds(noise, 4.0),
                )
            }
            "minecraft:shifted_noise" => {
                let noise = self.noise(name(value, "noise")?)?;
                let x = self.parse_argument(&value["shift_x"])?;
                let y = self.parse_argument(&value["shift_y"])?;
                let z = self.parse_argument(&value["shift_z"])?;
                (
                    DensityNode::ShiftedNoise {
                        noise,
                        x,
                        y,
                        z,
                        xz_scale: number(value, "xz_scale")?,
                        y_scale: number(value, "y_scale")?,
                    },
                    self.noise_bounds(noise, 1.0),
                )
            }
            "minecraft:weird_scaled_sampler" => {
                let input = self.parse_argument(&value["input"])?;
                let noise = self.noise(name(value, "noise")?)?;
                let rarity = match name(value, "rarity_value_mapper")? {
                    "type_1" => Rarity::Type1,
                    "type_2" => Rarity::Type2,
                    other => bail!("unknown rarity mapper {other}"),
                };
                let max = self.noises[noise].max_value()
                    * if matches!(rarity, Rarity::Type1) {
                        2.0
                    } else {
                        3.0
                    };
                (
                    DensityNode::WeirdScaled {
                        input,
                        noise,
                        rarity,
                    },
                    DensityBounds::new(0.0, max),
                )
            }
            "minecraft:spline" => {
                let spline = self.parse_spline(&value["spline"])?;
                let (a, b) = spline.bounds();
                (
                    DensityNode::Spline(spline),
                    DensityBounds::new(f64::from(a), f64::from(b)),
                )
            }
            "minecraft:old_blended_noise" => {
                let mut random = if self.legacy {
                    crate::random::RandomSourceKind::Legacy(LegacyRandomSource::new(self.seed))
                } else {
                    self.factory.from_hash_of("minecraft:terrain")
                };
                let noise = BlendedNoise::new(
                    &mut random,
                    number(value, "xz_scale")?,
                    number(value, "y_scale")?,
                    number(value, "xz_factor")?,
                    number(value, "y_factor")?,
                    number(value, "smear_scale_multiplier")?,
                );
                let bounds = DensityBounds::new(noise.min_value(), noise.max_value());
                let id = self.blends.len();
                self.blends.push(noise);
                (DensityNode::Blended(id), bounds)
            }
            "minecraft:end_islands" => {
                if self.end_noise.is_none() {
                    let mut random = LegacyRandomSource::new(self.seed);
                    random.consume_count(17_292);
                    self.end_noise = Some(SimplexNoise::new(&mut random));
                }
                (
                    DensityNode::EndIslands,
                    DensityBounds::new(-0.84375, 0.5625),
                )
            }
            "minecraft:blend_density" => {
                let input = self.parse_argument(&value["argument"])?;
                (
                    DensityNode::BlendDensity(input),
                    DensityBounds::new(f64::NEG_INFINITY, f64::INFINITY),
                )
            }
            other => bail!("unsupported density node {other}"),
        };
        Ok(self.push(kind, bounds))
    }
    // DensityFunction.HOLDER_HELPER_CODEC wraps inline values as well as named
    // references. These wrappers prevent add/mul's direct-Constant shortcut.
    fn parse_argument(&mut self, value: &Value) -> Result<NodeId> {
        let input = self.parse(value)?;
        if value.is_string() {
            Ok(input)
        } else {
            Ok(self.push(DensityNode::Reference(input), self.bounds(input)))
        }
    }
    fn binary(&mut self, kind: Binary, a: NodeId, b: NodeId) -> NodeId {
        let x = self.bounds(a);
        let y = self.bounds(b);
        let bounds = match kind {
            Binary::Add => DensityBounds::new(x.min + y.min, x.max + y.max),
            Binary::Min => DensityBounds::new(java_min(x.min, y.min), java_min(x.max, y.max)),
            Binary::Max => DensityBounds::new(java_max(x.min, y.min), java_max(x.max, y.max)),
            Binary::Mul => {
                let (min, max) = if x.min > 0.0 && y.min > 0.0 {
                    (x.min * y.min, x.max * y.max)
                } else if x.max < 0.0 && y.max < 0.0 {
                    (x.max * y.max, x.min * y.min)
                } else {
                    (
                        java_min(x.min * y.max, x.max * y.min),
                        java_max(x.min * y.min, x.max * y.max),
                    )
                };
                DensityBounds::new(min, max)
            }
        };
        let node = if matches!(kind, Binary::Add | Binary::Mul) {
            match (&self.nodes[a.0], &self.nodes[b.0]) {
                (DensityNode::Constant(value), _) => DensityNode::ConstantArgument(kind, b, *value),
                (_, DensityNode::Constant(value)) => DensityNode::ConstantArgument(kind, a, *value),
                _ => DensityNode::Binary(kind, a, b),
            }
        } else {
            DensityNode::Binary(kind, a, b)
        };
        self.push(node, bounds)
    }
    /// NoiseChunk.wrapNew erases HolderHolder after visiting its contents.
    /// Ap2.mapAll then re-runs the two-argument factory: only a resulting direct
    /// Constant permits MulOrAdd, not a marker or other function over a constant.
    fn wrapped_constant_argument(&self, id: NodeId) -> Option<(NodeId, f64)> {
        let DensityNode::Binary(Binary::Add | Binary::Mul, a, b) = self.nodes[id.0] else {
            return None;
        };
        let constant = |mut id: NodeId| loop {
            match self.nodes[id.0] {
                DensityNode::Reference(input) => id = input,
                DensityNode::Constant(value) => break Some(value),
                _ => break None,
            }
        };
        if let Some(value) = constant(a) {
            Some((b, value))
        } else {
            constant(b).map(|value| (a, value))
        }
    }
    fn parse_spline(&mut self, value: &Value) -> Result<Spline> {
        if let Some(value) = value.as_f64() {
            return Ok(Spline::Constant(value as f32));
        }
        let coordinate = if let Some(reference) = value["coordinate"].as_str() {
            self.definition(reference)?
        } else {
            self.parse(&value["coordinate"])?
        };
        let identity = self.next_spline_identity;
        self.next_spline_identity += 1;
        let points = value["points"]
            .as_array()
            .context("spline points must be an array")?;
        if points.is_empty() {
            bail!("Cannot create a multipoint spline with no points");
        }
        let mut parsed = Vec::with_capacity(points.len());
        for point in points {
            parsed.push(SplinePoint {
                location: number(point, "location")? as f32,
                value: self.parse_spline(&point["value"])?,
                derivative: number(point, "derivative")? as f32,
            });
        }
        let coord = self.bounds(coordinate);
        let cmin = coord.min as f32;
        let cmax = coord.max as f32;
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let first = &parsed[0];
        let last = &parsed[parsed.len() - 1];
        if cmin < first.location {
            let (a, b) = first.value.bounds();
            let a = extend_spline(cmin, first, a);
            let b = extend_spline(cmin, first, b);
            min = min32(min, min32(a, b));
            max = max32(max, max32(a, b));
        }
        if cmax > last.location {
            let (a, b) = last.value.bounds();
            let a = extend_spline(cmax, last, a);
            let b = extend_spline(cmax, last, b);
            min = min32(min, min32(a, b));
            max = max32(max, max32(a, b));
        }
        for point in &parsed {
            let (a, b) = point.value.bounds();
            min = min32(min, a);
            max = max32(max, b);
        }
        for pair in parsed.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            let width = b.location - a.location;
            let (amin, amax) = a.value.bounds();
            let (bmin, bmax) = b.value.bounds();
            let da = a.derivative;
            let db = b.derivative;
            if da == 0.0 && db == 0.0 {
                continue;
            }
            let da = da * width;
            let db = db * width;
            let low = min32(amin, bmin);
            let high = max32(amax, bmax);
            let e1 = da - bmax + amin;
            let e2 = da - bmin + amax;
            let e3 = -db + bmin - amax;
            let e4 = -db + bmax - amin;
            min = min32(min, low + 0.25 * min32(e1, e3));
            max = max32(max, high + 0.25 * max32(e2, e4));
        }
        Ok(Spline::Multipoint {
            coordinate,
            identity,
            points: parsed,
            min,
            max,
        })
    }
    /// Evaluate a density function before NoiseChunk wraps its markers.
    pub fn eval(&self, id: NodeId, point: DensityPoint) -> f64 {
        match self.eval_sampled(id, point, &mut RawDensityContext { points: &[] }) {
            Ok(value) => value,
            Err(never) => match never {},
        }
    }
    /// Evaluate an original density function before NoiseChunk wraps its markers.
    pub fn eval_sampled<C: DensityContext>(
        &self,
        id: NodeId,
        point: DensityPoint,
        context: &mut C,
    ) -> std::result::Result<f64, C::Error> {
        Ok(match &self.nodes[id.0] {
            DensityNode::Constant(value) => *value,
            DensityNode::Reference(input) => self.eval_sampled(*input, point, context)?,
            DensityNode::Marker(kind, input) => {
                return context.marker(self, id, *kind, *input, point)
            }
            DensityNode::BlendDensity(input) => {
                let value = self.eval_sampled(*input, point, context)?;
                context.blend_density(point, value)
            }
            DensityNode::BlendAlpha => context.blend_alpha(id, point),
            DensityNode::BlendOffset => context.blend_offset(id, point),
            DensityNode::Beardifier => 0.0,
            DensityNode::Binary(kind, a, b) => {
                if let Some((input, argument)) = context.rewritten_constant_argument(id) {
                    let value = self.eval_sampled(input, point, context)?;
                    return Ok(match kind {
                        Binary::Add => value + argument,
                        Binary::Mul => value * argument,
                        _ => unreachable!("wrapped constant optimization is add/mul only"),
                    });
                }
                let a = self.eval_sampled(*a, point, context)?;
                match kind {
                    Binary::Add => a + self.eval_sampled(*b, point, context)?,
                    Binary::Mul => {
                        if a == 0.0 {
                            0.0
                        } else {
                            a * self.eval_sampled(*b, point, context)?
                        }
                    }
                    Binary::Min => {
                        if a < context.bounds(self, *b).min {
                            a
                        } else {
                            java_min(a, self.eval_sampled(*b, point, context)?)
                        }
                    }
                    Binary::Max => {
                        if a > context.bounds(self, *b).max {
                            a
                        } else {
                            java_max(a, self.eval_sampled(*b, point, context)?)
                        }
                    }
                }
            }
            DensityNode::ConstantArgument(kind, input, argument) => {
                let value = self.eval_sampled(*input, point, context)?;
                match kind {
                    Binary::Add => value + argument,
                    Binary::Mul => value * argument,
                    _ => unreachable!("constant optimization is add/mul only"),
                }
            }
            DensityNode::Mapped(kind, input) => {
                apply_mapped(*kind, self.eval_sampled(*input, point, context)?)
            }
            DensityNode::Clamp { input, min, max } => {
                clamp(self.eval_sampled(*input, point, context)?, *min, *max)
            }
            DensityNode::RangeChoice {
                input,
                min,
                max,
                inside,
                outside,
            } => {
                let value = self.eval_sampled(*input, point, context)?;
                self.eval_sampled(
                    if value >= *min && value < *max {
                        *inside
                    } else {
                        *outside
                    },
                    point,
                    context,
                )?
            }
            DensityNode::YGradient {
                from_y,
                to_y,
                from_value,
                to_value,
            } => clamped_lerp(
                *from_value,
                *to_value,
                (f64::from(point.y) - f64::from(*from_y)) / (f64::from(*to_y) - f64::from(*from_y)),
            ),
            DensityNode::Noise {
                noise,
                xz_scale,
                y_scale,
            } => self.noises[*noise].noise(
                f64::from(point.x) * xz_scale,
                f64::from(point.y) * y_scale,
                f64::from(point.z) * xz_scale,
            ),
            DensityNode::Shift { noise, kind } => {
                let (x, y, z) = match kind {
                    Shift::A => (f64::from(point.x), 0.0, f64::from(point.z)),
                    Shift::B => (f64::from(point.z), f64::from(point.x), 0.0),
                    Shift::All => (f64::from(point.x), f64::from(point.y), f64::from(point.z)),
                };
                self.noises[*noise].noise(x * 0.25, y * 0.25, z * 0.25) * 4.0
            }
            DensityNode::ShiftedNoise {
                noise,
                x,
                y,
                z,
                xz_scale,
                y_scale,
            } => self.noises[*noise].noise(
                f64::from(point.x) * xz_scale + self.eval_sampled(*x, point, context)?,
                f64::from(point.y) * y_scale + self.eval_sampled(*y, point, context)?,
                f64::from(point.z) * xz_scale + self.eval_sampled(*z, point, context)?,
            ),
            DensityNode::WeirdScaled {
                input,
                noise,
                rarity: mapper,
            } => {
                let scale = rarity(*mapper, self.eval_sampled(*input, point, context)?);
                scale
                    * self.noises[*noise]
                        .noise(
                            f64::from(point.x) / scale,
                            f64::from(point.y) / scale,
                            f64::from(point.z) / scale,
                        )
                        .abs()
            }
            DensityNode::Spline(spline) => {
                f64::from(self.eval_spline_sampled(spline, point, context)?)
            }
            DensityNode::Blended(index) => self.blends[*index].compute(point.x, point.y, point.z),
            DensityNode::EndIslands => self.end_islands(point.x / 8, point.z / 8),
        })
    }
    pub fn eval_many(&self, id: NodeId, points: &[DensityPoint]) -> Vec<f64> {
        points.iter().map(|point| self.eval(id, *point)).collect()
    }
    pub fn fill_many(&self, id: NodeId, points: &[DensityPoint]) -> Vec<f64> {
        let mut values = vec![0.0; points.len()];
        match self.fill_sampled(id, &mut values, &mut RawDensityContext { points }) {
            Ok(()) => values,
            Err(never) => match never {},
        }
    }
    pub fn fill_sampled<C: DensityContext>(
        &self,
        id: NodeId,
        values: &mut [f64],
        context: &mut C,
    ) -> std::result::Result<(), C::Error> {
        match &self.nodes[id.0] {
            DensityNode::Constant(value) => values.fill(*value),
            DensityNode::BlendAlpha => context.blend_alpha_fill(self, id, values)?,
            DensityNode::BlendOffset => context.blend_offset_fill(self, id, values)?,
            DensityNode::Beardifier => context.beardifier_fill(self, id, values)?,
            DensityNode::Reference(input) => self.fill_sampled(*input, values, context)?,
            DensityNode::Marker(kind, input) => {
                context.marker_fill(self, id, *kind, *input, values)?
            }
            DensityNode::Mapped(kind, input) => {
                self.fill_sampled(*input, values, context)?;
                for value in values {
                    *value = apply_mapped(*kind, *value);
                }
            }
            DensityNode::Clamp { input, min, max } => {
                self.fill_sampled(*input, values, context)?;
                for value in values {
                    *value = clamp(*value, *min, *max);
                }
            }
            DensityNode::ConstantArgument(kind, input, argument) => {
                self.fill_sampled(*input, values, context)?;
                for value in values {
                    *value = match kind {
                        Binary::Add => *value + argument,
                        Binary::Mul => *value * argument,
                        _ => unreachable!("constant add/mul"),
                    };
                }
            }
            DensityNode::BlendDensity(input) => {
                self.fill_sampled(*input, values, context)?;
                for (index, value) in values.iter_mut().enumerate() {
                    let point = context.for_index(index);
                    *value = context.blend_density(point, *value);
                }
            }
            DensityNode::RangeChoice {
                input,
                min,
                max,
                inside,
                outside,
            } => {
                self.fill_sampled(*input, values, context)?;
                for (index, value) in values.iter_mut().enumerate() {
                    let point = context.for_index(index);
                    *value = self.eval_sampled(
                        if *value >= *min && *value < *max {
                            *inside
                        } else {
                            *outside
                        },
                        point,
                        context,
                    )?;
                }
            }
            DensityNode::Binary(kind, a, b) => {
                if let Some((input, argument)) = context.rewritten_constant_argument(id) {
                    self.fill_sampled(input, values, context)?;
                    for value in values {
                        *value = match kind {
                            Binary::Add => *value + argument,
                            Binary::Mul => *value * argument,
                            _ => unreachable!("wrapped constant optimization is add/mul only"),
                        };
                    }
                    return Ok(());
                }
                self.fill_sampled(*a, values, context)?;
                match kind {
                    Binary::Add => {
                        let mut other = vec![0.0; values.len()];
                        self.fill_sampled(*b, &mut other, context)?;
                        for (value, other) in values.iter_mut().zip(other) {
                            *value += other;
                        }
                    }
                    Binary::Mul => {
                        for (index, value) in values.iter_mut().enumerate() {
                            *value = if *value == 0.0 {
                                0.0
                            } else {
                                let point = context.for_index(index);
                                *value * self.eval_sampled(*b, point, context)?
                            };
                        }
                    }
                    Binary::Min => {
                        let bound = context.bounds(self, *b).min;
                        for (index, value) in values.iter_mut().enumerate() {
                            if (*value).partial_cmp(&bound) != Some(std::cmp::Ordering::Less) {
                                let point = context.for_index(index);
                                *value = java_min(*value, self.eval_sampled(*b, point, context)?);
                            }
                        }
                    }
                    Binary::Max => {
                        let bound = context.bounds(self, *b).max;
                        for (index, value) in values.iter_mut().enumerate() {
                            if (*value).partial_cmp(&bound) != Some(std::cmp::Ordering::Greater) {
                                let point = context.for_index(index);
                                *value = java_max(*value, self.eval_sampled(*b, point, context)?);
                            }
                        }
                    }
                }
            }
            _ => context.fill_direct(self, id, values)?,
        }
        Ok(())
    }
    fn eval_spline_sampled<C: DensityContext>(
        &self,
        spline: &Spline,
        point: DensityPoint,
        context: &mut C,
    ) -> std::result::Result<f32, C::Error> {
        Ok(match spline {
            Spline::Constant(value) => *value,
            Spline::Multipoint {
                coordinate, points, ..
            } => {
                let coordinate = self.eval_sampled(*coordinate, point, context)? as f32;
                let mut lo = 0;
                let mut hi = points.len();
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    if coordinate < points[mid].location {
                        hi = mid;
                    } else {
                        lo = mid + 1;
                    }
                }
                if lo == 0 {
                    return Ok(extend_spline(
                        coordinate,
                        &points[0],
                        self.eval_spline_sampled(&points[0].value, point, context)?,
                    ));
                }
                let a = &points[lo - 1];
                if lo == points.len() {
                    return Ok(extend_spline(
                        coordinate,
                        a,
                        self.eval_spline_sampled(&a.value, point, context)?,
                    ));
                }
                let b = &points[lo];
                let t = (coordinate - a.location) / (b.location - a.location);
                let av = self.eval_spline_sampled(&a.value, point, context)?;
                let bv = self.eval_spline_sampled(&b.value, point, context)?;
                let da = a.derivative * (b.location - a.location) - (bv - av);
                let db = -b.derivative * (b.location - a.location) + (bv - av);
                lerp32(t, av, bv) + t * (1.0 - t) * lerp32(t, da, db)
            }
        })
    }
    fn end_islands(&self, x: i32, z: i32) -> f64 {
        let noise = self.end_noise.as_ref().expect("end noise bound at parse");
        let cx = x / 2;
        let cz = z / 2;
        let rx = x % 2;
        let rz = z % 2;
        let squared = x.wrapping_mul(x).wrapping_add(z.wrapping_mul(z));
        let mut value = 100.0_f32 - (f64::from(squared as f32).sqrt() as f32) * 8.0;
        value = clamp32(value, -100.0, 80.0);
        for dx in -12_i32..=12 {
            for dz in -12_i32..=12 {
                let qx = i64::from(cx.wrapping_add(dx));
                let qz = i64::from(cz.wrapping_add(dz));
                if qx.wrapping_mul(qx).wrapping_add(qz.wrapping_mul(qz)) <= 4096
                    || noise.noise_2d(qx as f64, qz as f64) >= f64::from(-0.9_f32)
                {
                    continue;
                }
                let scale = ((qx as f32).abs() * 3439.0 + (qz as f32).abs() * 147.0) % 13.0 + 9.0;
                let x = (rx - dx * 2) as f32;
                let z = (rz - dz * 2) as f32;
                let candidate = 100.0 - (f64::from(x * x + z * z).sqrt() as f32) * scale;
                value = max32(value, clamp32(candidate, -100.0, 80.0));
            }
        }
        (f64::from(value) - 8.0) / 128.0
    }
}
fn extend_spline(coordinate: f32, point: &SplinePoint, value: f32) -> f32 {
    if point.derivative == 0.0 {
        value
    } else {
        value + point.derivative * (coordinate - point.location)
    }
}
