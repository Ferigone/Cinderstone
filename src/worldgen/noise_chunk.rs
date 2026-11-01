//! Source-ordered NOISE traversal over one chunk's real marker/cache owner.
//!
//! This owns interpolation state, not a ChunkStatus or a fabricated FULL chunk.
//! The caller supplies block material/aquifer work in the visit callback. The
//! density graph currently supports an explicitly empty Blender and beardifier.
use crate::density::{ChunkDensity, DensityGraph, DensityPoint, NoiseGrid};
use anyhow::{anyhow, bail, ensure, Result};

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
    /// Clamp the selected noise settings to the level's actual height accessor.
    ///
    /// A native delivery currently requires complete aligned cells. Rejecting a
    /// partial custom cell keeps unfilled coordinates explicit rather than
    /// publishing a truncated terrain image as complete.
    pub fn new(
        graph: &'g DensityGraph,
        chunk_x: i32,
        chunk_z: i32,
        level_min_y: i32,
        level_height: i32,
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
        ensure!(
            max_y > min_y,
            "level and noise settings have no vertical overlap"
        );
        let grid = NoiseGrid {
            min_y,
            height: max_y - min_y,
            cell_width: settings.cell_width,
            cell_height: settings.cell_height,
        };
        ensure!(
            grid.min_y.rem_euclid(grid.cell_height) == 0 && grid.height % grid.cell_height == 0,
            "partial noise cells in the level height accessor are unsupported"
        );
        let origin_x = chunk_x.wrapping_shl(4);
        let origin_z = chunk_z.wrapping_shl(4);
        let mut density =
            ChunkDensity::new(grid, origin_x, origin_z, (16 / grid.cell_width) as usize)?;
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
        mut visit: impl FnMut(&DensityGraph, &mut ChunkDensity, DensityPoint) -> Result<()>,
    ) -> Result<()> {
        if self.phase != TraversalPhase::Ready {
            bail!("noise traversal has already started or finished");
        }
        self.phase = TraversalPhase::Visiting;
        let result: Result<()> = (|| {
            self.density.initialize_first_cell_x(self.graph)?;
            let width = self.grid.cell_width;
            let height = self.grid.cell_height;
            let count_xz = 16 / width;
            let count_y = self.grid.height / height;
            let first_cell_y = self.grid.min_y.div_euclid(height);
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
                            let y = (first_cell_y + cell_y) * height + local_y;
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
    fn partial_cells_are_explicitly_unsupported() {
        let graph = DensityGraph::builtin(0, "overworld").unwrap();
        assert!(NoiseChunk::new(&graph, 0, 0, -63, 383).is_err());
        let clamped = NoiseChunk::new(&graph, 0, 0, 0, 128).unwrap();
        assert_eq!(
            clamped.grid(),
            NoiseGrid {
                min_y: 0,
                height: 128,
                cell_width: 4,
                cell_height: 8,
            }
        );
    }
}
