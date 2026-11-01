//! NoiseChunk marker caches and interpolation phase state (empty Blender).
use super::*;
use std::{
    collections::{HashMap, HashSet},
    fmt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkDensityError {
    InvalidGrid,
    InterpolationAlreadyStarted,
    InterpolationNotStarted,
    SampleOutsideInterpolation,
    ArrayIndex { index: usize, length: usize },
    UnregisteredMarker,
    MissingDensityRoot(&'static str),
}
impl fmt::Display for ChunkDensityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGrid => f.write_str("invalid noise-cell grid"),
            Self::InterpolationAlreadyStarted | Self::InterpolationNotStarted => {
                f.write_str("Staring interpolation twice")
            }
            Self::SampleOutsideInterpolation => {
                f.write_str("Trying to sample interpolator outside the interpolation loop")
            }
            Self::ArrayIndex { index, length } => {
                write!(f, "Index {index} out of bounds for length {length}")
            }
            Self::UnregisteredMarker => {
                f.write_str("density marker was not wrapped for this chunk")
            }
            Self::MissingDensityRoot(name) => write!(f, "missing density root {name}"),
        }
    }
}
impl std::error::Error for ChunkDensityError {}
type ChunkResult<T> = std::result::Result<T, ChunkDensityError>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    Column,
    Cell,
    External,
}
#[derive(Debug, Clone)]
struct Interpolator {
    input: NodeId,
    left: Vec<Vec<f64>>,
    right: Vec<Vec<f64>>,
    corners: [f64; 8],
    y_values: [f64; 4],
    x_values: [f64; 2],
    value: f64,
}
#[derive(Debug, Clone)]
enum MarkerCache {
    Interpolated(Box<Interpolator>),
    Flat {
        input: NodeId,
        values: Vec<Vec<f64>>,
    },
    Cache2D {
        input: NodeId,
        key: i64,
        value: f64,
    },
    Once {
        input: NodeId,
        last_scalar: i64,
        last_array: i64,
        value: f64,
        values: Option<Vec<f64>>,
    },
    Cell {
        input: NodeId,
        values: Vec<f64>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkDensityState {
    pub interpolating: bool,
    pub selecting_cell: bool,
    pub interpolation_counter: i64,
    pub array_counter: i64,
    pub array_index: i32,
    pub base_x: i32,
    pub base_y: i32,
    pub base_z: i32,
    pub in_x: i32,
    pub in_y: i32,
    pub in_z: i32,
}
/// One chunk's mutable sampling state; share the immutable DensityGraph across
/// chunks. Call wrap_router (or wrap for custom functions) before interpolation.
#[derive(Debug)]
pub struct ChunkDensity {
    grid: NoiseGrid,
    cell_count_xz: usize,
    cell_count_y: usize,
    first_cell_x: i32,
    first_cell_z: i32,
    first_cell_y: i32,
    first_quart_x: i32,
    first_quart_z: i32,
    quart_count: usize,
    canonical: Vec<usize>,
    rewritten_arithmetic: Vec<Option<(NodeId, f64)>>,
    canonical_keys: HashMap<String, usize>,
    registered: HashSet<usize>,
    caches: Vec<Option<MarkerCache>>,
    interpolators: Vec<usize>,
    cell_caches: Vec<usize>,
    state: ChunkDensityState,
    owner_context: bool,
    provider: Provider,
    external_points: Vec<DensityPoint>,
    preliminary_surface_levels: HashMap<i64, i32>,
}
fn pack_xz(x: i32, z: i32) -> i64 {
    ((x as u32 as u64) | ((z as u32 as u64) << 32)) as i64
}
impl ChunkDensity {
    pub fn new(
        grid: NoiseGrid,
        origin_x: i32,
        origin_z: i32,
        cell_count_xz: usize,
    ) -> ChunkResult<Self> {
        if grid.cell_width <= 0
            || grid.cell_height <= 0
            || grid.height < 0
            || cell_count_xz == 0
            || cell_count_xz > 4096
        {
            return Err(ChunkDensityError::InvalidGrid);
        }
        let width = grid.cell_width;
        let height = grid.cell_height;
        Ok(Self {
            grid,
            cell_count_xz,
            cell_count_y: grid.height.div_euclid(height) as usize,
            first_cell_x: origin_x.div_euclid(width),
            first_cell_z: origin_z.div_euclid(width),
            first_cell_y: grid.min_y.div_euclid(height),
            first_quart_x: origin_x >> 2,
            first_quart_z: origin_z >> 2,
            quart_count: ((cell_count_xz as i32).wrapping_mul(width) >> 2) as usize,
            canonical: Vec::new(),
            rewritten_arithmetic: Vec::new(),
            canonical_keys: HashMap::new(),
            registered: HashSet::new(),
            caches: Vec::new(),
            interpolators: Vec::new(),
            cell_caches: Vec::new(),
            state: ChunkDensityState {
                interpolating: false,
                selecting_cell: false,
                interpolation_counter: 0,
                array_counter: 0,
                array_index: 0,
                base_x: 0,
                base_y: 0,
                base_z: 0,
                in_x: 0,
                in_y: 0,
                in_z: 0,
            },
            owner_context: false,
            provider: Provider::External,
            external_points: Vec::new(),
            preliminary_surface_levels: HashMap::new(),
        })
    }
    pub fn state(&self) -> ChunkDensityState {
        self.state
    }
    pub fn point(&self) -> DensityPoint {
        DensityPoint::new(
            self.state.base_x.wrapping_add(self.state.in_x),
            self.state.base_y.wrapping_add(self.state.in_y),
            self.state.base_z.wrapping_add(self.state.in_z),
        )
    }
    /// The vertical extent used by this chunk's interpolation and preliminary
    /// surface queries, after the generator settings have been clamped to the
    /// level height accessor.
    pub fn grid(&self) -> NoiseGrid {
        self.grid
    }

    /// NoiseChunk.preliminarySurfaceLevel, including quart-block quantization,
    /// its per-chunk cache and the original no-surface sentinel (i32::MAX).
    ///
    /// The sampled function is the wrapped initial-density root, evaluated with
    /// a SinglePointContext. It must not use the current interpolation point:
    /// aquifer queries can ask about columns outside the chunk being filled.
    pub fn preliminary_surface_level(
        &mut self,
        graph: &DensityGraph,
        x: i32,
        z: i32,
    ) -> ChunkResult<i32> {
        let x = (x >> 2).wrapping_shl(2);
        let z = (z >> 2).wrapping_shl(2);
        let key = pack_xz(x, z);
        if let Some(level) = self.preliminary_surface_levels.get(&key) {
            return Ok(*level);
        }
        let input = graph.root("initial_density_without_jaggedness").ok_or(
            ChunkDensityError::MissingDensityRoot("initial_density_without_jaggedness"),
        )?;
        let was_owner = self.owner_context;
        self.owner_context = false;
        let sampled: ChunkResult<i32> = (|| {
            let mut y = self.grid.min_y.wrapping_add(self.grid.height);
            while y >= self.grid.min_y {
                if graph.eval_sampled(input, DensityPoint::new(x, y, z), self)? > 0.390625 {
                    return Ok(y);
                }
                // Vanilla dimensions cannot underflow here. Avoid a wrapped,
                // nonterminating loop for an out-of-domain custom grid.
                let Some(next_y) = y.checked_sub(self.grid.cell_height) else {
                    break;
                };
                y = next_y;
            }
            Ok(i32::MAX)
        })();
        self.owner_context = was_owner;
        let level = sampled?;
        self.preliminary_surface_levels.insert(key, level);
        Ok(level)
    }

    fn update_canonical(&mut self, graph: &DensityGraph) {
        while self.canonical.len() < graph.nodes.len() {
            let id = self.canonical.len();
            let rewritten = graph.wrapped_constant_argument(NodeId(id));
            self.rewritten_arithmetic.push(rewritten);
            if let DensityNode::Reference(input) = graph.nodes[id] {
                // HolderHolder.mapAll visits the held function first, then
                // NoiseChunk.wrapNew returns holder.value(). Parent equality
                // therefore sees the wrapped child, not another holder layer.
                self.canonical.push(self.canonical[input.0]);
                continue;
            }
            let mut node = match (rewritten, &graph.nodes[id]) {
                (Some((input, argument)), DensityNode::Binary(kind, _, _)) => {
                    DensityNode::ConstantArgument(*kind, input, argument)
                }
                _ => graph.nodes[id].clone(),
            };
            map_children(&mut node, &mut |child| NodeId(self.canonical[child.0]));
            // Debug floats round-trip uniquely, including signed zero. This key is an
            // in-memory structural equality key, never a persisted format or hash ID.
            let key = match &node {
                DensityNode::Spline(Spline::Multipoint { identity, .. }) => {
                    format!("spline{identity}:{node:?}")
                }
                _ => format!("{node:?}"),
            };
            let next = self.caches.len();
            let canonical = *self.canonical_keys.entry(key).or_insert_with(|| {
                self.caches.push(None);
                next
            });
            self.canonical.push(canonical);
        }
    }
    pub fn wrap(&mut self, graph: &DensityGraph, id: NodeId) -> ChunkResult<()> {
        self.update_canonical(graph);
        self.register(graph, id)
    }
    pub fn wrap_router(&mut self, graph: &DensityGraph) -> ChunkResult<()> {
        for name in [
            "barrier",
            "fluid_level_floodedness",
            "fluid_level_spread",
            "lava",
            "temperature",
            "vegetation",
            "continents",
            "erosion",
            "depth",
            "ridges",
            "initial_density_without_jaggedness",
            "final_density",
            "vein_toggle",
            "vein_ridged",
            "vein_gap",
        ] {
            if let Some(id) = graph.root(name) {
                self.wrap(graph, id)?;
            }
        }
        if let Some(id) = graph.block_density() {
            self.wrap(graph, id)?;
        }
        Ok(())
    }
    fn register(&mut self, graph: &DensityGraph, id: NodeId) -> ChunkResult<()> {
        if let DensityNode::Reference(input) = graph.nodes[id.0] {
            // A reference shares its child's canonical identity. Register the
            // child before marking that identity, or its marker would be lost.
            return self.register(graph, input);
        }
        let canonical = self.canonical[id.0];
        if !self.registered.insert(canonical) {
            return Ok(());
        }
        let mut children = Vec::new();
        visit_children(&graph.nodes[id.0], &mut |child| children.push(child));
        for child in children {
            self.register(graph, child)?;
        }
        let DensityNode::Marker(kind, input) = graph.nodes[id.0] else {
            return Ok(());
        };
        let cache = match kind {
            MarkerKind::Interpolated => {
                self.interpolators.push(canonical);
                MarkerCache::Interpolated(Box::new(Interpolator {
                    input,
                    left: vec![vec![0.0; self.cell_count_y + 1]; self.cell_count_xz + 1],
                    right: vec![vec![0.0; self.cell_count_y + 1]; self.cell_count_xz + 1],
                    corners: [0.0; 8],
                    y_values: [0.0; 4],
                    x_values: [0.0; 2],
                    value: 0.0,
                }))
            }
            MarkerKind::FlatCache => {
                let mut values = vec![vec![0.0; self.quart_count + 1]; self.quart_count + 1];
                let was_owner = self.owner_context;
                self.owner_context = false;
                for (x, row) in values.iter_mut().enumerate() {
                    for (z, value) in row.iter_mut().enumerate() {
                        let point = DensityPoint::new(
                            self.first_quart_x.wrapping_add(x as i32).wrapping_shl(2),
                            0,
                            self.first_quart_z.wrapping_add(z as i32).wrapping_shl(2),
                        );
                        *value = graph.eval_sampled(input, point, self)?;
                    }
                }
                self.owner_context = was_owner;
                MarkerCache::Flat { input, values }
            }
            MarkerKind::Cache2D => MarkerCache::Cache2D {
                input,
                key: pack_xz(1_875_066, 1_875_066),
                value: 0.0,
            },
            MarkerKind::CacheOnce => MarkerCache::Once {
                input,
                last_scalar: 0,
                last_array: 0,
                value: 0.0,
                values: None,
            },
            MarkerKind::CacheAllInCell => {
                self.cell_caches.push(canonical);
                MarkerCache::Cell {
                    input,
                    values: vec![0.0; self.cell_volume()],
                }
            }
        };
        self.caches[canonical] = Some(cache);
        Ok(())
    }
    fn cell_volume(&self) -> usize {
        (self.grid.cell_width as usize)
            * (self.grid.cell_width as usize)
            * (self.grid.cell_height as usize)
    }
    fn fill_slice(&mut self, graph: &DensityGraph, left: bool, cell_x: i32) -> ChunkResult<()> {
        self.state.base_x = cell_x.wrapping_mul(self.grid.cell_width);
        self.state.in_x = 0;
        self.provider = Provider::Column;
        self.owner_context = true;
        for z in 0..=self.cell_count_xz {
            self.state.base_z = self
                .first_cell_z
                .wrapping_add(z as i32)
                .wrapping_mul(self.grid.cell_width);
            self.state.in_z = 0;
            self.state.array_counter = self.state.array_counter.wrapping_add(1);
            for index in 0..self.interpolators.len() {
                let cache = self.interpolators[index];
                let input = match self.caches[cache].as_ref().unwrap() {
                    MarkerCache::Interpolated(value) => value.input,
                    _ => unreachable!(),
                };
                let mut values = vec![0.0; self.cell_count_y + 1];
                graph.fill_sampled(input, &mut values, self)?;
                match self.caches[cache].as_mut().unwrap() {
                    MarkerCache::Interpolated(value) => {
                        if left {
                            value.left[z] = values;
                        } else {
                            value.right[z] = values;
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        self.state.array_counter = self.state.array_counter.wrapping_add(1);
        Ok(())
    }
    pub fn initialize_first_cell_x(&mut self, graph: &DensityGraph) -> ChunkResult<()> {
        if self.state.interpolating {
            return Err(ChunkDensityError::InterpolationAlreadyStarted);
        }
        self.state.interpolating = true;
        self.state.interpolation_counter = 0;
        self.fill_slice(graph, true, self.first_cell_x)
    }
    pub fn advance_cell_x(&mut self, graph: &DensityGraph, index: i32) -> ChunkResult<()> {
        self.fill_slice(
            graph,
            false,
            self.first_cell_x.wrapping_add(index).wrapping_add(1),
        )?;
        self.state.base_x = self
            .first_cell_x
            .wrapping_add(index)
            .wrapping_mul(self.grid.cell_width);
        Ok(())
    }
    pub fn select_cell_yz(
        &mut self,
        graph: &DensityGraph,
        cell_y: usize,
        cell_z: usize,
    ) -> ChunkResult<()> {
        for index in 0..self.interpolators.len() {
            let cache = self.interpolators[index];
            let value = match self.caches[cache].as_mut().unwrap() {
                MarkerCache::Interpolated(value) => value,
                _ => unreachable!(),
            };
            let get = |slice: &Vec<Vec<f64>>, z: usize, y: usize| -> ChunkResult<f64> {
                let row = slice.get(z).ok_or(ChunkDensityError::ArrayIndex {
                    index: z,
                    length: slice.len(),
                })?;
                row.get(y).copied().ok_or(ChunkDensityError::ArrayIndex {
                    index: y,
                    length: row.len(),
                })
            };
            value.corners = [
                get(&value.left, cell_z, cell_y)?,
                get(&value.right, cell_z, cell_y)?,
                get(&value.left, cell_z, cell_y + 1)?,
                get(&value.right, cell_z, cell_y + 1)?,
                get(&value.left, cell_z + 1, cell_y)?,
                get(&value.right, cell_z + 1, cell_y)?,
                get(&value.left, cell_z + 1, cell_y + 1)?,
                get(&value.right, cell_z + 1, cell_y + 1)?,
            ];
        }
        self.state.selecting_cell = true;
        self.state.base_y = self
            .first_cell_y
            .wrapping_add(cell_y as i32)
            .wrapping_mul(self.grid.cell_height);
        self.state.base_z = self
            .first_cell_z
            .wrapping_add(cell_z as i32)
            .wrapping_mul(self.grid.cell_width);
        self.state.array_counter = self.state.array_counter.wrapping_add(1);
        self.provider = Provider::Cell;
        self.owner_context = true;
        for index in 0..self.cell_caches.len() {
            let cache = self.cell_caches[index];
            let input = match self.caches[cache].as_ref().unwrap() {
                MarkerCache::Cell { input, .. } => *input,
                _ => unreachable!(),
            };
            let mut values = vec![0.0; self.cell_volume()];
            graph.fill_sampled(input, &mut values, self)?;
            match self.caches[cache].as_mut().unwrap() {
                MarkerCache::Cell { values: cached, .. } => *cached = values,
                _ => unreachable!(),
            }
        }
        self.state.array_counter = self.state.array_counter.wrapping_add(1);
        self.state.selecting_cell = false;
        Ok(())
    }
    pub fn update_for_y(&mut self, block_y: i32, fraction: f64) {
        self.state.in_y = block_y.wrapping_sub(self.state.base_y);
        for cache in &self.interpolators {
            let MarkerCache::Interpolated(value) = self.caches[*cache].as_mut().unwrap() else {
                unreachable!()
            };
            let c = value.corners;
            value.y_values = [
                lerp(fraction, c[0], c[2]),
                lerp(fraction, c[1], c[3]),
                lerp(fraction, c[4], c[6]),
                lerp(fraction, c[5], c[7]),
            ];
        }
    }
    pub fn update_for_x(&mut self, block_x: i32, fraction: f64) {
        self.state.in_x = block_x.wrapping_sub(self.state.base_x);
        for cache in &self.interpolators {
            let MarkerCache::Interpolated(value) = self.caches[*cache].as_mut().unwrap() else {
                unreachable!()
            };
            let y = value.y_values;
            value.x_values = [lerp(fraction, y[0], y[1]), lerp(fraction, y[2], y[3])];
        }
    }
    pub fn update_for_z(&mut self, block_z: i32, fraction: f64) {
        self.state.in_z = block_z.wrapping_sub(self.state.base_z);
        self.state.interpolation_counter = self.state.interpolation_counter.wrapping_add(1);
        for cache in &self.interpolators {
            let MarkerCache::Interpolated(value) = self.caches[*cache].as_mut().unwrap() else {
                unreachable!()
            };
            value.value = lerp(fraction, value.x_values[0], value.x_values[1]);
        }
    }
    pub fn stop_interpolation(&mut self) -> ChunkResult<()> {
        if !self.state.interpolating {
            return Err(ChunkDensityError::InterpolationNotStarted);
        }
        self.state.interpolating = false;
        Ok(())
    }
    pub fn swap_slices(&mut self) {
        for cache in &self.interpolators {
            let MarkerCache::Interpolated(value) = self.caches[*cache].as_mut().unwrap() else {
                unreachable!()
            };
            std::mem::swap(&mut value.left, &mut value.right);
        }
    }
    pub fn sample(&mut self, graph: &DensityGraph, id: NodeId) -> ChunkResult<f64> {
        self.owner_context = true;
        graph.eval_sampled(id, self.point(), self)
    }
    pub fn sample_external(
        &mut self,
        graph: &DensityGraph,
        id: NodeId,
        point: DensityPoint,
    ) -> ChunkResult<f64> {
        self.owner_context = false;
        graph.eval_sampled(id, point, self)
    }
    pub fn for_cell_index(&mut self, index: usize) -> DensityPoint {
        self.provider = Provider::Cell;
        self.for_index(index)
    }
    pub fn fill_column(&mut self, graph: &DensityGraph, id: NodeId) -> ChunkResult<Vec<f64>> {
        self.provider = Provider::Column;
        self.owner_context = true;
        let mut values = vec![0.0; self.cell_count_y + 1];
        graph.fill_sampled(id, &mut values, self)?;
        Ok(values)
    }
    pub fn fill_cell(&mut self, graph: &DensityGraph, id: NodeId) -> ChunkResult<Vec<f64>> {
        self.provider = Provider::Cell;
        self.owner_context = true;
        let mut values = vec![0.0; self.cell_volume()];
        graph.fill_sampled(id, &mut values, self)?;
        Ok(values)
    }
    pub fn fill_external(
        &mut self,
        graph: &DensityGraph,
        id: NodeId,
        points: &[DensityPoint],
    ) -> ChunkResult<Vec<f64>> {
        self.external_points.clear();
        self.external_points.extend_from_slice(points);
        self.provider = Provider::External;
        self.owner_context = false;
        let mut values = vec![0.0; points.len()];
        graph.fill_sampled(id, &mut values, self)?;
        Ok(values)
    }
}
impl DensityContext for ChunkDensity {
    type Error = ChunkDensityError;
    fn rewritten_constant_argument(&self, id: NodeId) -> Option<(NodeId, f64)> {
        self.rewritten_arithmetic.get(id.0).copied().flatten()
    }
    fn marker(
        &mut self,
        graph: &DensityGraph,
        id: NodeId,
        _kind: MarkerKind,
        _input: NodeId,
        point: DensityPoint,
    ) -> ChunkResult<f64> {
        let cache = *self
            .canonical
            .get(id.0)
            .ok_or(ChunkDensityError::UnregisteredMarker)?;
        match self
            .caches
            .get(cache)
            .and_then(Option::as_ref)
            .ok_or(ChunkDensityError::UnregisteredMarker)?
        {
            MarkerCache::Flat { input, values } => {
                let x = (point.x >> 2).wrapping_sub(self.first_quart_x);
                let z = (point.z >> 2).wrapping_sub(self.first_quart_z);
                if x >= 0 && z >= 0 && (x as usize) < values.len() && (z as usize) < values.len() {
                    return Ok(values[x as usize][z as usize]);
                }
                let input = *input;
                graph.eval_sampled(input, point, self)
            }
            MarkerCache::Cache2D { input, key, value } => {
                let next = pack_xz(point.x, point.z);
                if *key == next {
                    return Ok(*value);
                }
                let input = *input;
                if let Some(MarkerCache::Cache2D { key, .. }) = self.caches[cache].as_mut() {
                    *key = next;
                }
                let value = graph.eval_sampled(input, point, self)?;
                if let Some(MarkerCache::Cache2D { value: cached, .. }) =
                    self.caches[cache].as_mut()
                {
                    *cached = value;
                }
                Ok(value)
            }
            MarkerCache::Once {
                input,
                last_scalar,
                last_array,
                value,
                values,
            } => {
                let input = *input;
                if !self.owner_context {
                    return graph.eval_sampled(input, point, self);
                }
                if let Some(values) = values {
                    if *last_array == self.state.array_counter {
                        let index = self.state.array_index as usize;
                        return values
                            .get(index)
                            .copied()
                            .ok_or(ChunkDensityError::ArrayIndex {
                                index,
                                length: values.len(),
                            });
                    }
                }
                if *last_scalar == self.state.interpolation_counter {
                    return Ok(*value);
                }
                if let Some(MarkerCache::Once { last_scalar, .. }) = self.caches[cache].as_mut() {
                    *last_scalar = self.state.interpolation_counter;
                }
                let value = graph.eval_sampled(input, point, self)?;
                if let Some(MarkerCache::Once { value: cached, .. }) = self.caches[cache].as_mut() {
                    *cached = value;
                }
                Ok(value)
            }
            MarkerCache::Interpolated(value) => {
                if !self.owner_context {
                    let input = value.input;
                    return graph.eval_sampled(input, point, self);
                }
                if !self.state.interpolating {
                    return Err(ChunkDensityError::SampleOutsideInterpolation);
                }
                if self.state.selecting_cell {
                    let x = f64::from(self.state.in_x) / f64::from(self.grid.cell_width);
                    let y = f64::from(self.state.in_y) / f64::from(self.grid.cell_height);
                    let z = f64::from(self.state.in_z) / f64::from(self.grid.cell_width);
                    let c = value.corners;
                    Ok(lerp(
                        z,
                        lerp(y, lerp(x, c[0], c[1]), lerp(x, c[2], c[3])),
                        lerp(y, lerp(x, c[4], c[5]), lerp(x, c[6], c[7])),
                    ))
                } else {
                    Ok(value.value)
                }
            }
            MarkerCache::Cell { input, values } => {
                let input = *input;
                if !self.owner_context {
                    return graph.eval_sampled(input, point, self);
                }
                if !self.state.interpolating {
                    return Err(ChunkDensityError::SampleOutsideInterpolation);
                }
                let x = self.state.in_x;
                let y = self.state.in_y;
                let z = self.state.in_z;
                if x >= 0
                    && y >= 0
                    && z >= 0
                    && x < self.grid.cell_width
                    && y < self.grid.cell_height
                    && z < self.grid.cell_width
                {
                    let index = (((self.grid.cell_height - 1 - y) * self.grid.cell_width + x)
                        * self.grid.cell_width
                        + z) as usize;
                    return Ok(values[index]);
                }
                graph.eval_sampled(input, point, self)
            }
        }
    }
    fn marker_fill(
        &mut self,
        graph: &DensityGraph,
        id: NodeId,
        _kind: MarkerKind,
        _input: NodeId,
        values: &mut [f64],
    ) -> ChunkResult<()> {
        let cache = *self
            .canonical
            .get(id.0)
            .ok_or(ChunkDensityError::UnregisteredMarker)?;
        match self
            .caches
            .get(cache)
            .and_then(Option::as_ref)
            .ok_or(ChunkDensityError::UnregisteredMarker)?
        {
            MarkerCache::Once {
                input,
                last_array,
                values: cached,
                ..
            } => {
                if let Some(cached) = cached {
                    if *last_array == self.state.array_counter {
                        if values.len() > cached.len() {
                            return Err(ChunkDensityError::ArrayIndex {
                                index: values.len(),
                                length: cached.len(),
                            });
                        }
                        values.copy_from_slice(&cached[..values.len()]);
                        return Ok(());
                    }
                }
                let input = *input;
                graph.fill_sampled(input, values, self)?;
                if let Some(MarkerCache::Once {
                    last_array,
                    values: cached,
                    ..
                }) = self.caches[cache].as_mut()
                {
                    if let Some(cached) = cached {
                        if cached.len() == values.len() {
                            cached.copy_from_slice(values);
                        } else {
                            *cached = values.to_vec();
                        }
                    } else {
                        *cached = Some(values.to_vec());
                    }
                    *last_array = self.state.array_counter;
                }
                Ok(())
            }
            MarkerCache::Interpolated(value) => {
                let input = value.input;
                if self.state.selecting_cell {
                    self.fill_direct(graph, id, values)
                } else {
                    graph.fill_sampled(input, values, self)
                }
            }
            MarkerCache::Cache2D { input, .. } => {
                let input = *input;
                graph.fill_sampled(input, values, self)
            }
            MarkerCache::Flat { .. } | MarkerCache::Cell { .. } => {
                self.fill_direct(graph, id, values)
            }
        }
    }
    fn for_index(&mut self, index: usize) -> DensityPoint {
        match self.provider {
            Provider::Column => {
                self.state.base_y = (index as i32)
                    .wrapping_add(self.first_cell_y)
                    .wrapping_mul(self.grid.cell_height);
                self.state.interpolation_counter = self.state.interpolation_counter.wrapping_add(1);
                self.state.in_y = 0;
                self.state.array_index = index as i32;
                self.owner_context = true;
                self.point()
            }
            Provider::Cell => {
                let index = index as i32;
                let w = self.grid.cell_width;
                let z = index.rem_euclid(w);
                let rest = index.div_euclid(w);
                self.state.in_x = rest.rem_euclid(w);
                self.state.in_y = self.grid.cell_height - 1 - rest.div_euclid(w);
                self.state.in_z = z;
                self.state.array_index = index;
                self.owner_context = true;
                self.point()
            }
            Provider::External => {
                self.owner_context = false;
                self.external_points[index]
            }
        }
    }
    fn fill_direct(
        &mut self,
        graph: &DensityGraph,
        id: NodeId,
        values: &mut [f64],
    ) -> ChunkResult<()> {
        match self.provider {
            Provider::Column => {
                for index in 0..=self.cell_count_y {
                    let point = self.for_index(index);
                    let value = graph.eval_sampled(id, point, self)?;
                    let length = values.len();
                    *values
                        .get_mut(index)
                        .ok_or(ChunkDensityError::ArrayIndex { index, length })? = value;
                }
            }
            Provider::Cell => {
                self.state.array_index = 0;
                self.owner_context = true;
                for y in (0..self.grid.cell_height).rev() {
                    self.state.in_y = y;
                    for x in 0..self.grid.cell_width {
                        self.state.in_x = x;
                        for z in 0..self.grid.cell_width {
                            self.state.in_z = z;
                            let index = self.state.array_index as usize;
                            self.state.array_index = self.state.array_index.wrapping_add(1);
                            let value = graph.eval_sampled(id, self.point(), self)?;
                            let length = values.len();
                            *values
                                .get_mut(index)
                                .ok_or(ChunkDensityError::ArrayIndex { index, length })? = value;
                        }
                    }
                }
            }
            Provider::External => {
                for (index, value) in values.iter_mut().enumerate() {
                    let point = self.for_index(index);
                    *value = graph.eval_sampled(id, point, self)?;
                }
            }
        }
        Ok(())
    }
}
fn visit_spline(spline: &Spline, visit: &mut impl FnMut(NodeId)) {
    match spline {
        Spline::Constant(_) => {}
        Spline::Multipoint {
            coordinate, points, ..
        } => {
            visit(*coordinate);
            for point in points {
                visit_spline(&point.value, visit);
            }
        }
    }
}
fn visit_children(node: &DensityNode, visit: &mut impl FnMut(NodeId)) {
    match node {
        DensityNode::Reference(input)
        | DensityNode::Marker(_, input)
        | DensityNode::Mapped(_, input)
        | DensityNode::ConstantArgument(_, input, _)
        | DensityNode::Clamp { input, .. }
        | DensityNode::WeirdScaled { input, .. }
        | DensityNode::BlendDensity(input) => visit(*input),
        DensityNode::Binary(_, a, b) => {
            visit(*a);
            visit(*b);
        }
        DensityNode::RangeChoice {
            input,
            inside,
            outside,
            ..
        } => {
            visit(*input);
            visit(*inside);
            visit(*outside);
        }
        DensityNode::ShiftedNoise { x, y, z, .. } => {
            visit(*x);
            visit(*y);
            visit(*z);
        }
        DensityNode::Spline(spline) => visit_spline(spline, visit),
        _ => {}
    }
}
fn map_spline(spline: &mut Spline, map: &mut impl FnMut(NodeId) -> NodeId) {
    if let Spline::Multipoint {
        coordinate, points, ..
    } = spline
    {
        *coordinate = map(*coordinate);
        for point in points {
            map_spline(&mut point.value, map);
        }
    }
}
fn map_children(node: &mut DensityNode, map: &mut impl FnMut(NodeId) -> NodeId) {
    match node {
        DensityNode::Reference(input)
        | DensityNode::Marker(_, input)
        | DensityNode::Mapped(_, input)
        | DensityNode::ConstantArgument(_, input, _)
        | DensityNode::Clamp { input, .. }
        | DensityNode::WeirdScaled { input, .. }
        | DensityNode::BlendDensity(input) => *input = map(*input),
        DensityNode::Binary(_, a, b) => {
            *a = map(*a);
            *b = map(*b);
        }
        DensityNode::RangeChoice {
            input,
            inside,
            outside,
            ..
        } => {
            *input = map(*input);
            *inside = map(*inside);
            *outside = map(*outside);
        }
        DensityNode::ShiftedNoise { x, y, z, .. } => {
            *x = map(*x);
            *y = map(*y);
            *z = map(*z);
        }
        DensityNode::Spline(spline) => map_spline(spline, map),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewrite_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../fixtures/world-generation-density-rewrite.json"
        ))
        .unwrap()
    }

    fn rewrite_chunk() -> ChunkDensity {
        ChunkDensity::new(
            NoiseGrid {
                min_y: -64,
                height: 384,
                cell_width: 4,
                cell_height: 8,
            },
            0,
            0,
            4,
        )
        .unwrap()
    }

    fn observed_bits(value: &Value) -> Vec<u64> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().parse().unwrap())
            .collect()
    }

    #[test]
    fn chunk_arithmetic_rewrite_preserves_original_scalar_and_fill_bits() {
        let fixture = rewrite_fixture();
        for case in fixture["cases"].as_array().unwrap() {
            let mut graph = DensityGraph::new(0, false, BTreeMap::new(), BTreeMap::new());
            let id = graph.parse(&case["expression"]).unwrap();
            let points: Vec<_> = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    DensityPoint::new(
                        p[0].as_i64().unwrap() as i32,
                        p[1].as_i64().unwrap() as i32,
                        p[2].as_i64().unwrap() as i32,
                    )
                })
                .collect();
            let raw = observed_bits(&case["raw_fill_bits"]);
            let wrapped = observed_bits(&case["wrapped_fill_bits"]);
            let bits = |values: Vec<f64>| values.into_iter().map(f64::to_bits).collect::<Vec<_>>();
            assert_eq!(bits(graph.eval_many(id, &points)), raw);
            assert_eq!(bits(graph.fill_many(id, &points)), raw);
            let bounds = graph.bounds(id);
            assert_eq!(
                vec![bounds.min.to_bits(), bounds.max.to_bits()],
                observed_bits(&case["raw_bounds_bits"])
            );
            // Empty-context holder erasure preserves these bounds exactly;
            // MIN/MAX may retain the graph's original short-circuit metadata.
            assert_eq!(case["raw_bounds_bits"], case["wrapped_bounds_bits"]);
            let mut chunk = rewrite_chunk();
            chunk.wrap(&graph, id).unwrap();
            let sampled = points
                .iter()
                .map(|point| chunk.sample_external(&graph, id, *point).unwrap().to_bits())
                .collect::<Vec<_>>();
            assert_eq!(sampled, wrapped);
            assert_eq!(
                bits(chunk.fill_external(&graph, id, &points).unwrap()),
                wrapped
            );
        }
    }

    #[test]
    fn zero_coefficient_keeps_wrapped_input_lifecycle_errors() {
        let fixture = rewrite_fixture();
        let case = &fixture["lifecycle"];
        let mut graph = DensityGraph::new(0, false, BTreeMap::new(), BTreeMap::new());
        let id = graph.parse(&case["expression"]).unwrap();
        assert_eq!(graph.eval(id, DensityPoint::new(0, -64, 0)).to_bits(), 0);
        let mut chunk = rewrite_chunk();
        chunk.wrap(&graph, id).unwrap();
        let error = chunk.sample(&graph, id).unwrap_err();
        assert_eq!(error, ChunkDensityError::SampleOutsideInterpolation);
        assert_eq!(error.to_string(), case["error"].as_str().unwrap());
    }

    #[test]
    fn rewritten_arithmetic_keeps_the_original_marker_owner() {
        let fixture = rewrite_fixture();
        assert_eq!(fixture["marker_identity"]["same_owner"], true);
        let expression = &fixture["cases"][0]["expression"];
        let mut graph = DensityGraph::new(0, false, BTreeMap::new(), BTreeMap::new());
        let held = graph.parse(expression).unwrap();
        let constant = graph.parse(&expression["argument1"]).unwrap();
        let input = graph.parse(&expression["argument2"]).unwrap();
        let direct = graph.binary(Binary::Mul, constant, input);
        let held_marker = graph.push(
            DensityNode::Marker(MarkerKind::Interpolated, held),
            graph.bounds(held),
        );
        let direct_marker = graph.push(
            DensityNode::Marker(MarkerKind::Interpolated, direct),
            graph.bounds(direct),
        );
        let mut chunk = rewrite_chunk();
        chunk.wrap(&graph, held_marker).unwrap();
        chunk.wrap(&graph, direct_marker).unwrap();
        assert_eq!(chunk.interpolators.len(), 1);
        assert_eq!(
            chunk.canonical[held_marker.0],
            chunk.canonical[direct_marker.0]
        );
    }

    #[test]
    fn empty_namespace_uses_the_original_default_namespace() {
        let canonical = DensityGraph::builtin(123456789, "minecraft:overworld").unwrap();
        let alias = DensityGraph::builtin(123456789, ":overworld").unwrap();
        assert_eq!(
            canonical.positional_random_factory(),
            alias.positional_random_factory()
        );
        assert_eq!(
            canonical.sample_climate(-3, 17, 2).unwrap(),
            alias.sample_climate(-3, 17, 2).unwrap()
        );
    }

    #[test]
    fn holder_indirection_preserves_the_interpolator_owner() {
        // The original HolderHolder.mapAll + NoiseChunk.wrap produced one
        // identical owner for these direct/held arguments; see the source note.
        let mut graph = DensityGraph::new(0, false, BTreeMap::new(), BTreeMap::new());
        let value = graph.push(DensityNode::Constant(0.25), DensityBounds::constant(0.25));
        let held = graph.push(DensityNode::Reference(value), graph.bounds(value));
        let direct = graph.push(
            DensityNode::Marker(MarkerKind::Interpolated, value),
            graph.bounds(value),
        );
        let indirect = graph.push(
            DensityNode::Marker(MarkerKind::Interpolated, held),
            graph.bounds(value),
        );
        let mut chunk = ChunkDensity::new(
            NoiseGrid {
                min_y: 0,
                height: 16,
                cell_width: 4,
                cell_height: 8,
            },
            0,
            0,
            4,
        )
        .unwrap();
        chunk.wrap(&graph, direct).unwrap();
        chunk.wrap(&graph, indirect).unwrap();
        assert_eq!(chunk.interpolators.len(), 1);
        assert_eq!(chunk.canonical[direct.0], chunk.canonical[indirect.0]);
        chunk.initialize_first_cell_x(&graph).unwrap();
        chunk.advance_cell_x(&graph, 0).unwrap();
        chunk.select_cell_yz(&graph, 0, 0).unwrap();
        chunk.update_for_y(3, 3.0 / 8.0);
        chunk.update_for_x(2, 2.0 / 4.0);
        chunk.update_for_z(1, 1.0 / 4.0);
        assert_eq!(
            chunk.sample(&graph, direct).unwrap().to_bits(),
            0.25_f64.to_bits()
        );
        assert_eq!(
            chunk.sample(&graph, indirect).unwrap().to_bits(),
            0.25_f64.to_bits()
        );
        chunk.stop_interpolation().unwrap();
    }

    #[test]
    fn builtin_cache_owner_counts_match_the_original_constructor() {
        for (settings, expected) in [("overworld", (8, 1)), ("nether", (1, 1)), ("end", (1, 1))] {
            let graph = DensityGraph::builtin(123456789, settings).unwrap();
            let grid = graph.grid().unwrap();
            let mut chunk =
                ChunkDensity::new(grid, -32, 48, (16 / grid.cell_width) as usize).unwrap();
            chunk.wrap_router(&graph).unwrap();
            assert_eq!(
                (chunk.interpolators.len(), chunk.cell_caches.len()),
                expected,
                "{settings}"
            );
        }
    }
}
