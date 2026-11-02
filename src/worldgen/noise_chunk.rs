//! Source-ordered NOISE traversal over one chunk's real marker/cache owner.
//!
//! This owns interpolation state, not a ChunkStatus or a fabricated FULL chunk.
//! The caller supplies block material/aquifer work in the visit callback. The
//! density owner retains its actual Blender and structure Beardifier before
//! router wrapping; unavailable influences are not replaced by empty owners.
use super::{beardifier::Beardifier, blending::Blender};
use crate::density::{ChunkDensity, DensityGraph, DensityPoint, NoiseGrid};
use anyhow::{anyhow, bail, ensure, Result};
use std::sync::Arc;

/// The separate generation window passed to NoiseBasedChunkGenerator.doFill.
///
/// NoiseChunk.forChunk clamps against the chunk's build accessor, while
/// fillFromNoise clamps against getHeightAccessorForGeneration. Neither clamp
/// rounds its minimum or height to a cell boundary. This plan does not own or
/// allocate a target, NoiseChunk, aquifer, section or heightmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseFillPlan {
    grid: NoiseGrid,
    min_cell_y: i32,
    cell_count_y: i32,
}

impl NoiseFillPlan {
    /// Clamp the original noise settings to the actual generation accessor.
    /// A negative clamped height is retained for the source no-cell return.
    pub fn new(settings: NoiseGrid, generation_min_y: i32, generation_height: i32) -> Result<Self> {
        ensure!(
            generation_height >= 0,
            "generation height must not be negative"
        );
        ensure!(
            settings.height >= 0,
            "noise-settings height must not be negative"
        );
        let generation_max_y = generation_min_y
            .checked_add(generation_height)
            .ok_or_else(|| anyhow!("generation vertical extent overflows"))?;
        let settings_max_y = settings
            .min_y
            .checked_add(settings.height)
            .ok_or_else(|| anyhow!("noise-settings vertical extent overflows"))?;
        let min_y = generation_min_y.max(settings.min_y);
        let height = generation_max_y
            .min(settings_max_y)
            .checked_sub(min_y)
            .ok_or_else(|| anyhow!("clamped generation height overflows"))?;
        Self::from_grid(NoiseGrid {
            min_y,
            height,
            ..settings
        })
    }

    fn from_grid(grid: NoiseGrid) -> Result<Self> {
        ensure!(
            grid.cell_width > 0 && grid.cell_height > 0,
            "noise-cell dimensions must be positive"
        );
        Ok(Self {
            grid,
            min_cell_y: grid.min_y.div_euclid(grid.cell_height),
            cell_count_y: grid.height.div_euclid(grid.cell_height),
        })
    }

    /// Unrounded generation clamp, not the retained NoiseChunk build grid.
    pub fn grid(&self) -> NoiseGrid {
        self.grid
    }
    pub fn min_cell_y(&self) -> i32 {
        self.min_cell_y
    }
    pub fn cell_count_y(&self) -> i32 {
        self.cell_count_y
    }
    pub fn has_cells(&self) -> bool {
        self.cell_count_y > 0
    }
    /// First Y traversed geometrically; it can lie below the logical minimum.
    pub fn first_block_y(&self) -> i32 {
        self.min_cell_y.wrapping_mul(self.grid.cell_height)
    }
    /// Traversed height when positive; a nonpositive count is a no-cell plan.
    pub fn block_height(&self) -> i32 {
        self.cell_count_y.wrapping_mul(self.grid.cell_height)
    }
    /// Absolute inclusive section Ys, top then bottom, acquired descending.
    ///
    /// The source uses the unrounded clamped minimum here, rather than the
    /// first traversed block. The actual target converts these to its section
    /// indices and retains/releases those original section owners.
    pub fn section_lock_span(&self) -> Option<(i32, i32)> {
        self.has_cells().then(|| {
            let top_y = self
                .grid
                .min_y
                .wrapping_add(self.block_height())
                .wrapping_sub(1);
            (top_y >> 4, self.grid.min_y >> 4)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TraversalPhase {
    Ready,
    Visiting,
    Traversed,
    Failed,
}

/// A graph-bound sampling owner retained across NOISE and SURFACE phases.
///
/// Borrowing the graph prevents its replacement or mutation while canonical
/// marker IDs and caches are live. A traversal is one-shot: an interrupted fill
/// cannot be resumed as if every earlier block had been committed.
pub struct NoiseChunk<'g> {
    graph: &'g DensityGraph,
    density: ChunkDensity,
    grid: NoiseGrid,
    origin_x: i32,
    origin_z: i32,
    phase: TraversalPhase,
}

/// Internal move-only state used while an Arc-bound prepared job holds the
/// exact graph alive. Fields and restore operations are not public rebinding
/// APIs. The identity is compared only; it is never dereferenced or persisted.
pub(super) struct RetainedNoiseChunk {
    graph_identity: usize,
    density: ChunkDensity,
    grid: NoiseGrid,
    origin_x: i32,
    origin_z: i32,
    phase: TraversalPhase,
}
impl RetainedNoiseChunk {
    fn require_graph(&self, graph: &DensityGraph) -> Result<()> {
        ensure!(
            self.graph_identity == graph as *const DensityGraph as usize,
            "retained noise state belongs to a different density graph"
        );
        Ok(())
    }
    pub(super) fn density_mut_for(&mut self, graph: &DensityGraph) -> Result<&mut ChunkDensity> {
        self.require_graph(graph)?;
        Ok(&mut self.density)
    }
    pub(super) fn blender_for(&self, graph: &DensityGraph) -> Result<Arc<Blender>> {
        self.require_graph(graph)?;
        Ok(Arc::clone(self.density.blender()))
    }
    pub(super) fn beardifier_for(&self, graph: &DensityGraph) -> Result<Arc<Beardifier>> {
        self.require_graph(graph)?;
        Ok(Arc::clone(self.density.beardifier()))
    }
    #[cfg(test)]
    pub(super) fn density_for(&self, graph: &DensityGraph) -> Result<&ChunkDensity> {
        self.require_graph(graph)?;
        Ok(&self.density)
    }
}

impl std::fmt::Debug for NoiseChunk<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseChunk")
            .field("grid", &self.grid)
            .field("origin_x", &self.origin_x)
            .field("origin_z", &self.origin_z)
            .field("phase", &self.phase)
            .field("density_state", &self.density.state())
            .finish_non_exhaustive()
    }
}

impl<'g> NoiseChunk<'g> {
    /// Clamp the selected noise settings to the chunk's actual build accessor.
    /// The unrounded grid controls all retained interpolation/aquifer caches.
    /// The material callback must own actual physical sections: traversal of a
    /// partial bottom cell can reach below the logical accessor minimum.
    pub fn new(
        graph: &'g DensityGraph,
        chunk_x: i32,
        chunk_z: i32,
        level_min_y: i32,
        level_height: i32,
    ) -> Result<Self> {
        Self::new_with_blender(
            graph,
            chunk_x,
            chunk_z,
            level_min_y,
            level_height,
            Blender::empty(),
        )
    }

    /// Capture the actual Blender and the known empty structure influence.
    pub fn new_with_blender(
        graph: &'g DensityGraph,
        chunk_x: i32,
        chunk_z: i32,
        level_min_y: i32,
        level_height: i32,
        blender: Arc<Blender>,
    ) -> Result<Self> {
        Self::new_with_influences(
            graph,
            chunk_x,
            chunk_z,
            level_min_y,
            level_height,
            blender,
            Beardifier::empty(),
        )
    }

    /// Capture both actual influences before router wrapping and aquifer creation.
    /// The supplied owners are retained unchanged across prepared stage moves.
    pub fn new_with_influences(
        graph: &'g DensityGraph,
        chunk_x: i32,
        chunk_z: i32,
        level_min_y: i32,
        level_height: i32,
        blender: Arc<Blender>,
        beardifier: Arc<Beardifier>,
    ) -> Result<Self> {
        let settings = graph
            .grid()
            .ok_or_else(|| anyhow!("density graph has no noise settings"))?;
        ensure!(level_height > 0, "level height must be positive");
        ensure!(
            settings.cell_width > 0
                && settings.cell_width <= 16
                && 16 % settings.cell_width == 0
                && settings.cell_height > 0,
            "unsupported noise-cell dimensions"
        );
        let level_max_y = level_min_y
            .checked_add(level_height)
            .ok_or_else(|| anyhow!("level vertical extent overflows"))?;
        let settings_max_y = settings
            .min_y
            .checked_add(settings.height)
            .ok_or_else(|| anyhow!("noise-settings vertical extent overflows"))?;
        let min_y = level_min_y.max(settings.min_y);
        let max_y = level_max_y.min(settings_max_y);
        // The original forChunk constructor can retain a zero-height clamp:
        // BIOMES and SURFACE still use its real caches/aquifer even when the
        // separate fillFromNoise entry returns without material traversal.
        // Negative constructor grids remain an explicit unsupported boundary.
        ensure!(
            max_y >= min_y,
            "negative clamped NoiseChunk height is unsupported"
        );
        let grid = NoiseGrid {
            min_y,
            height: max_y - min_y,
            cell_width: settings.cell_width,
            cell_height: settings.cell_height,
        };
        let origin_x = chunk_x.wrapping_shl(4);
        let origin_z = chunk_z.wrapping_shl(4);
        let mut density = ChunkDensity::new_with_influences(
            grid,
            origin_x,
            origin_z,
            (16 / grid.cell_width) as usize,
            blender,
            beardifier,
        )?;
        // This also registers the aquifer/router functions before the first
        // slice is filled, preserving the source wrapper and cache ownership.
        density.wrap_router(graph)?;
        Ok(Self {
            graph,
            density,
            grid,
            origin_x,
            origin_z,
            phase: TraversalPhase::Ready,
        })
    }

    pub fn grid(&self) -> NoiseGrid {
        self.grid
    }

    pub fn density(&self) -> &ChunkDensity {
        &self.density
    }

    /// Access retained preliminary-surface/cache state during the SURFACE phase.
    pub fn density_mut(&mut self) -> &mut ChunkDensity {
        &mut self.density
    }

    pub fn into_density(self) -> ChunkDensity {
        self.density
    }

    pub(super) fn into_retained(self) -> RetainedNoiseChunk {
        RetainedNoiseChunk {
            graph_identity: self.graph as *const DensityGraph as usize,
            density: self.density,
            grid: self.grid,
            origin_x: self.origin_x,
            origin_z: self.origin_z,
            phase: self.phase,
        }
    }

    pub(super) fn restore(graph: &'g DensityGraph, retained: RetainedNoiseChunk) -> Result<Self> {
        retained.require_graph(graph)?;
        Ok(Self {
            graph,
            density: retained.density,
            grid: retained.grid,
            origin_x: retained.origin_x,
            origin_z: retained.origin_z,
            phase: retained.phase,
        })
    }

    /// Visit blocks in NoiseBasedChunkGenerator.doFill order, updating the
    /// interpolators before invoking the material callback at each coordinate.
    ///
    /// An error stops interpolation and poisons this traversal. The caller must
    /// discard its privately staged terrain, retaining the original error.
    pub fn visit_blocks(
        &mut self,
        visit: impl FnMut(&DensityGraph, &mut ChunkDensity, DensityPoint) -> Result<()>,
    ) -> Result<()> {
        let plan = NoiseFillPlan::from_grid(self.grid)?;
        self.visit_blocks_with_plan(&plan, visit)
    }

    /// Traverse the separate source generation window over this exact owner.
    ///
    /// Full slices and cell caches retain the chunk-build grid. Cell selection
    /// is relative from zero, while block Y uses the plan's minimum cell Y, as
    /// in doFill. No coordinate is clipped to logical or physical target bounds;
    /// the actual section owner handles writes or rejects an unavailable index.
    /// A no-cell plan leaves even an already traversed owner's state unchanged.
    pub fn visit_blocks_with_plan(
        &mut self,
        plan: &NoiseFillPlan,
        mut visit: impl FnMut(&DensityGraph, &mut ChunkDensity, DensityPoint) -> Result<()>,
    ) -> Result<()> {
        if !plan.has_cells() {
            return Ok(());
        }
        if self.phase != TraversalPhase::Ready {
            bail!("noise traversal has already started or finished");
        }
        ensure!(
            plan.grid.cell_width == self.grid.cell_width
                && plan.grid.cell_height == self.grid.cell_height,
            "generation plan belongs to different noise-cell dimensions"
        );
        self.phase = TraversalPhase::Visiting;
        let result: Result<()> = (|| {
            self.density.initialize_first_cell_x(self.graph)?;
            let width = self.grid.cell_width;
            let height = self.grid.cell_height;
            let count_xz = 16 / width;
            let count_y = plan.cell_count_y;
            let first_cell_y = plan.min_cell_y;
            for cell_x in 0..count_xz {
                self.density.advance_cell_x(self.graph, cell_x)?;
                for cell_z in 0..count_xz {
                    for cell_y in (0..count_y).rev() {
                        self.density.select_cell_yz(
                            self.graph,
                            cell_y as usize,
                            cell_z as usize,
                        )?;
                        for local_y in (0..height).rev() {
                            let y = first_cell_y
                                .wrapping_add(cell_y)
                                .wrapping_mul(height)
                                .wrapping_add(local_y);
                            self.density
                                .update_for_y(y, f64::from(local_y) / f64::from(height));
                            for local_x in 0..width {
                                let x = self.origin_x.wrapping_add(cell_x * width + local_x);
                                self.density
                                    .update_for_x(x, f64::from(local_x) / f64::from(width));
                                for local_z in 0..width {
                                    let z = self.origin_z.wrapping_add(cell_z * width + local_z);
                                    self.density
                                        .update_for_z(z, f64::from(local_z) / f64::from(width));
                                    visit(
                                        self.graph,
                                        &mut self.density,
                                        DensityPoint::new(x, y, z),
                                    )?;
                                }
                            }
                        }
                    }
                }
                self.density.swap_slices();
            }
            Ok(())
        })();
        // initialize_first_cell_x sets the flag before sampling. A sampling or
        // callback error still leaves a live interpolator that must be stopped.
        let stop = if self.density.state().interpolating {
            self.density
                .stop_interpolation()
                .map_err(anyhow::Error::from)
        } else {
            Ok(())
        };
        let result = result.and(stop);
        self.phase = if result.is_ok() {
            TraversalPhase::Traversed
        } else {
            TraversalPhase::Failed
        };
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_material_callback_stops_and_cannot_resume_a_partial_fill() {
        let graph = DensityGraph::builtin(123456789, "overworld").unwrap();
        let mut chunk = NoiseChunk::new(&graph, -2, 3, -64, 384).unwrap();
        let mut first_point = None;
        let error = chunk
            .visit_blocks(|_, _, point| {
                first_point = Some(point);
                bail!("material owner rejected staged fill")
            })
            .unwrap_err();
        assert_eq!(error.to_string(), "material owner rejected staged fill");
        assert_eq!(first_point, Some(DensityPoint::new(-32, 319, 48)));
        assert!(!chunk.density().state().interpolating);
        let mut resumed = false;
        assert!(chunk
            .visit_blocks(|_, _, _| {
                resumed = true;
                Ok(())
            })
            .is_err());
        assert!(!resumed);
    }

    #[test]
    fn retained_state_rejects_equal_seed_different_graphs() {
        let graph = DensityGraph::builtin(123456789, "nether").unwrap();
        let other = DensityGraph::builtin(123456789, "nether").unwrap();
        let chunk = NoiseChunk::new(&graph, 0, 0, 0, 8).unwrap();
        let retained = chunk.into_retained();
        assert!(NoiseChunk::restore(&other, retained).is_err());
    }

    #[test]
    fn explicit_influence_owners_survive_retained_moves_and_graph_checks() {
        let graph = DensityGraph::builtin(123456789, "nether").unwrap();
        let other = DensityGraph::builtin(123456789, "nether").unwrap();
        let blender = Blender::empty();
        let beardifier = Beardifier::empty();
        let chunk = NoiseChunk::new_with_influences(
            &graph,
            0,
            0,
            -64,
            64,
            Arc::clone(&blender),
            Arc::clone(&beardifier),
        )
        .unwrap();
        // A zero-height NoiseChunk can be retained before BIOMES/after a
        // skipped fill. Its captured influences must not be reconstructed.
        assert_eq!((chunk.grid().min_y, chunk.grid().height), (0, 0));
        assert!(Arc::ptr_eq(chunk.density().blender(), &blender));
        assert!(Arc::ptr_eq(chunk.density().beardifier(), &beardifier));
        let retained = chunk.into_retained();
        assert!(retained.beardifier_for(&other).is_err());
        assert!(Arc::ptr_eq(
            &retained.beardifier_for(&graph).unwrap(),
            &beardifier
        ));
        let mut restored = NoiseChunk::restore(&graph, retained).unwrap();
        restored
            .visit_blocks(|_, _, _| panic!("zero-height fill reached material callback"))
            .unwrap();
        assert!(Arc::ptr_eq(restored.density().blender(), &blender));
        assert!(Arc::ptr_eq(restored.density().beardifier(), &beardifier));
    }

    #[test]
    fn retained_restore_preserves_finished_and_failed_traversal_phases() {
        let graph = DensityGraph::builtin(123456789, "nether").unwrap();
        for fail in [false, true] {
            let mut chunk = NoiseChunk::new(&graph, 0, 0, 0, 8).unwrap();
            let result = chunk.visit_blocks(|_, _, _| {
                if fail {
                    bail!("controlled material failure");
                }
                Ok(())
            });
            assert_eq!(result.is_err(), fail);
            let state = chunk.density().state();
            let allocations = chunk.density().allocation_fingerprint();
            let mut restored = NoiseChunk::restore(&graph, chunk.into_retained()).unwrap();
            assert_eq!(restored.density().state(), state);
            assert_eq!(restored.density().allocation_fingerprint(), allocations);
            let mut visits = 0;
            assert!(restored
                .visit_blocks(|_, _, _| {
                    visits += 1;
                    Ok(())
                })
                .is_err());
            assert_eq!(visits, 0);
        }
    }

    #[test]
    fn partial_cells_keep_unrounded_owner_metadata_and_source_write_coordinates() {
        let graph = DensityGraph::builtin(0, "overworld").unwrap();
        let mut chunk = NoiseChunk::new(&graph, 0, 0, 1, 17).unwrap();
        assert_eq!(
            chunk.grid(),
            NoiseGrid {
                min_y: 1,
                height: 17,
                cell_width: 4,
                cell_height: 8,
            }
        );
        let mut lowest = i32::MAX;
        let mut highest = i32::MIN;
        chunk
            .visit_blocks(|_, _, point| {
                lowest = lowest.min(point.y);
                highest = highest.max(point.y);
                Ok(())
            })
            .unwrap();
        assert_eq!((lowest, highest), (0, 15));
        assert_eq!(chunk.density().grid(), chunk.grid());
    }

    #[test]
    fn generation_clamps_preserve_source_floor_division_and_distinct_lock_span() {
        let settings = NoiseGrid {
            min_y: -64,
            height: 384,
            cell_width: 4,
            cell_height: 8,
        };
        // Source fillFromNoise/doFill examples. Locks use clamped minY, while
        // writes use floorDiv(minY, cellHeight) * cellHeight.
        for (min_y, height, first, count, locks) in [
            (1, 127, 0, 15, (7, 0)),
            (-63, 127, -64, 15, (3, -4)),
            (7, 17, 0, 2, (1, 0)),
        ] {
            let plan = NoiseFillPlan::new(settings, min_y, height).unwrap();
            assert_eq!((plan.grid().min_y, plan.grid().height), (min_y, height));
            assert_eq!(plan.first_block_y(), first);
            assert_eq!(plan.cell_count_y(), count);
            assert_eq!(plan.block_height(), count * 8);
            assert_eq!(plan.section_lock_span(), Some(locks));
        }
        let disjoint = NoiseFillPlan::new(settings, 384, 16).unwrap();
        assert_eq!((disjoint.grid().min_y, disjoint.grid().height), (384, -64));
        assert_eq!(disjoint.cell_count_y(), -8);
        assert!(!disjoint.has_cells());
        assert_eq!(disjoint.section_lock_span(), None);
    }

    #[test]
    fn separate_generation_window_keeps_full_build_slices_and_relative_cache_cells() {
        let graph = DensityGraph::builtin(123456789, "nether").unwrap();
        let mut chunk = NoiseChunk::new(&graph, 0, 0, 0, 128).unwrap();
        let plan = NoiseFillPlan::new(graph.grid().unwrap(), 8, 16).unwrap();
        let allocations = chunk.density().allocation_fingerprint();
        let mut first = None;
        let mut lowest = i32::MAX;
        let mut highest = i32::MIN;
        chunk
            .visit_blocks_with_plan(&plan, |_, density, point| {
                first.get_or_insert((point, density.state().base_y, density.state().in_y));
                lowest = lowest.min(point.y);
                highest = highest.max(point.y);
                Ok(())
            })
            .unwrap();
        // Source selects cache cell 1 relative to build minCellY=0, but emits
        // the first world Y using generation minCellY=1.
        assert_eq!(first, Some((DensityPoint::new(0, 23, 0), 8, 15)));
        assert_eq!((lowest, highest), (8, 23));
        assert_eq!((chunk.grid().min_y, chunk.grid().height), (0, 128));
        assert_eq!(chunk.density().allocation_fingerprint(), allocations);
        assert!(!chunk.density().state().interpolating);
    }

    #[test]
    fn no_cell_plan_preserves_ready_and_finished_noise_owners() {
        let graph = DensityGraph::builtin(0, "nether").unwrap();
        let mut chunk = NoiseChunk::new(&graph, 0, 0, 0, 8).unwrap();
        let plan = NoiseFillPlan::new(graph.grid().unwrap(), 128, 8).unwrap();
        for finished in [false, true] {
            if finished {
                chunk.visit_blocks(|_, _, _| Ok(())).unwrap();
            }
            let state = chunk.density().state();
            let allocations = chunk.density().allocation_fingerprint();
            let phase = chunk.phase;
            chunk
                .visit_blocks_with_plan(&plan, |_, _, _| {
                    panic!("no-cell plan called the material owner")
                })
                .unwrap();
            assert_eq!(chunk.density().state(), state);
            assert_eq!(chunk.density().allocation_fingerprint(), allocations);
            assert_eq!(chunk.phase, phase);
        }
    }

    #[test]
    fn zero_height_constructor_retains_sampling_owner_across_skipped_fill() {
        // Actual original BIOMES -> NoCells -> SURFACE profiles construct and
        // retain these owners rather than rejecting their zero-height clamp.
        for (settings, min_y, height, clamped_min) in
            [("nether", -64, 64, 0), ("overworld", 320, 64, 320)]
        {
            let graph = DensityGraph::builtin(123456789, settings).unwrap();
            let mut chunk = NoiseChunk::new(&graph, 0, 0, min_y, height).unwrap();
            assert_eq!((chunk.grid().min_y, chunk.grid().height), (clamped_min, 0));
            let temperature = graph.root("temperature").unwrap();
            let point = DensityPoint::new(0, min_y, 0);
            let before = chunk
                .density_mut()
                .sample_external(&graph, temperature, point)
                .unwrap();
            let preliminary = chunk
                .density_mut()
                .preliminary_surface_level(&graph, 0, 0)
                .unwrap();
            let state = chunk.density().state();
            let allocations = chunk.density().allocation_fingerprint();
            chunk
                .visit_blocks(|_, _, _| panic!("zero-height owner traversed material cells"))
                .unwrap();
            assert_eq!(chunk.phase, TraversalPhase::Ready);
            assert_eq!(chunk.density().state(), state);
            assert_eq!(chunk.density().allocation_fingerprint(), allocations);
            let mut restored = NoiseChunk::restore(&graph, chunk.into_retained()).unwrap();
            assert_eq!(restored.density().allocation_fingerprint(), allocations);
            assert_eq!(
                restored
                    .density_mut()
                    .sample_external(&graph, temperature, point)
                    .unwrap()
                    .to_bits(),
                before.to_bits()
            );
            assert_eq!(
                restored
                    .density_mut()
                    .preliminary_surface_level(&graph, 0, 0)
                    .unwrap(),
                preliminary
            );
        }
    }

    #[test]
    fn negative_clamped_constructor_height_remains_explicitly_unsupported() {
        let graph = DensityGraph::builtin(0, "overworld").unwrap();
        let error = NoiseChunk::new(&graph, 0, 0, 321, 63).unwrap_err();
        assert_eq!(
            error.to_string(),
            "negative clamped NoiseChunk height is unsupported"
        );
    }
}
