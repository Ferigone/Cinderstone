//! Immutable, ordered terrain influence from actual structure observations.
//! Direct parts are data ingress, not a StructureManager provenance certificate.
use crate::density::DensityPoint;
use std::sync::Arc;

mod kernel;
pub mod selection;

/// Original inclusive integer BoundingBox fields without coordinate rewriting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockBox {
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainAdjustment {
    None,
    Bury,
    BeardThin,
    BeardBox,
    Encapsulate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rigid {
    pub bounds: BlockBox,
    pub terrain_adjustment: TerrainAdjustment,
    pub ground_level_delta: i32,
}

/// The source coordinates consumed by the original Beardifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Junction {
    pub source_x: i32,
    pub source_ground_y: i32,
    pub source_z: i32,
}

/// Contributions retain actual encounter order, including duplicates.
#[derive(Debug)]
pub struct Beardifier {
    rigids: Vec<Rigid>,
    junctions: Vec<Junction>,
}

impl Beardifier {
    /// An observed-empty influence, freshly allocated for each owner.
    pub fn empty() -> Arc<Self> {
        Self::from_ordered_parts(Vec::new(), Vec::new())
    }

    /// All supplied fields and their ordering must come from actual owners.
    /// Missing or unavailable observations cannot be represented by this call.
    pub fn from_ordered_parts(rigids: Vec<Rigid>, junctions: Vec<Junction>) -> Arc<Self> {
        Arc::new(Self { rigids, junctions })
    }

    pub fn rigids(&self) -> &[Rigid] {
        &self.rigids
    }
    pub fn junctions(&self) -> &[Junction] {
        &self.junctions
    }
    pub fn compute(&self, point: DensityPoint) -> f64 {
        kernel::compute(self, point)
    }
}
