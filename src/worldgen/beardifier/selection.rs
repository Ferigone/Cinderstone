//! Source StructureManager lookup and ordered Beardifier part selection.
use super::{Beardifier, BlockBox, Junction, Rigid, TerrainAdjustment};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolProjection {
    Rigid,
    TerrainMatching,
}

/// Actual structure-reference and STARTS owners used by one selection call.
///
/// Handles identify the live owners from the same StructureManager/level;
/// they are not registry names, copied pieces certified by a boolean, or
/// replacement chunks. Ordered vectors may snapshot those owners' actual
/// iterator order, but must not sort, deduplicate or synthesize absent data.
/// Dependencies must remain available and stable throughout this call.
/// Missing owners and unavailable observations are errors. Only an observed
/// null start is `None`; only an observed non-pool piece has no projection.
///
/// Every callback is mandatory and fallible. A successful collector is not
/// evidence that its caller implements WorldGenRegion dependency scheduling.
pub trait BeardifierSelectionHost {
    type ReferencesChunk;
    type Structure;
    type ReferenceSet;
    type StartsChunk;
    type Start;
    type Piece;

    /// Actual level.getChunk(x,z,STRUCTURE_REFERENCES), then getAllReferences.
    fn load_references_chunk(&mut self, chunk: (i32, i32)) -> Result<Self::ReferencesChunk>;
    /// The actual reference map's entrySet encounter order, including each
    /// structure's retained LongSet owner. Empty means observed empty.
    fn reference_entries(
        &mut self,
        chunk: &Self::ReferencesChunk,
    ) -> Result<Vec<(Self::Structure, Self::ReferenceSet)>>;
    fn terrain_adjustment(&mut self, structure: &Self::Structure) -> Result<TerrainAdjustment>;
    /// Actual LongSet iterator order, containing packed ChunkPos longs.
    fn reference_positions(&mut self, references: &Self::ReferenceSet) -> Result<Vec<i64>>;
    /// The level's actual getMinSection; read separately for every reference.
    fn min_section(&mut self) -> Result<i32>;
    /// Actual level.getChunk(x,z,STRUCTURE_STARTS), with no generated substitute.
    fn load_starts_chunk(&mut self, chunk: (i32, i32)) -> Result<Self::StartsChunk>;
    /// getStartForStructure at SectionPos(chunk.x,section_y,chunk.z).
    /// The original helper delegates to this chunk's getStartForStructure.
    fn start_for_structure(
        &mut self,
        chunk: &Self::StartsChunk,
        section_y: i32,
        structure: &Self::Structure,
    ) -> Result<Option<Self::Start>>;
    /// Actual StructureStart.isValid (its PiecesContainer is not empty).
    fn start_is_valid(&mut self, start: &Self::Start) -> Result<bool>;
    fn start_structure(&mut self, start: &Self::Start) -> Result<Self::Structure>;
    fn pieces(&mut self, start: &Self::Start) -> Result<Vec<Self::Piece>>;
    fn piece_bounds(&mut self, piece: &Self::Piece) -> Result<BlockBox>;
    /// `Some` requires an actual PoolElementStructurePiece and its element's
    /// actual projection; `None` requires an actual ordinary StructurePiece.
    fn pool_projection(&mut self, piece: &Self::Piece) -> Result<Option<PoolProjection>>;
    fn pool_ground_level_delta(&mut self, piece: &Self::Piece) -> Result<i32>;
    /// Actual pool piece getJunctions encounter order. Both projections use it.
    fn pool_junctions(&mut self, piece: &Self::Piece) -> Result<Vec<Junction>>;
}

/// Reproduce StructureManager.startsForStructure, then Beardifier's selection.
///
/// The first phase resolves all valid starts before any piece is inspected.
/// The map-key structure filters references; the start's own structure supplies
/// each selected rigid's adjustment. Repeated starts and parts stay repeated.
pub fn select_beardifier<H: BeardifierSelectionHost + ?Sized>(
    host: &mut H,
    chunk: (i32, i32),
) -> Result<Arc<Beardifier>> {
    let references = host.load_references_chunk(chunk)?;
    let mut starts = Vec::new();
    for (structure, reference_set) in host.reference_entries(&references)? {
        if host.terrain_adjustment(&structure)? == TerrainAdjustment::None {
            continue;
        }
        for packed in host.reference_positions(&reference_set)? {
            // ChunkPos(long) keeps the low signed X and high signed Z halves.
            let referenced_chunk = (packed as i32, (packed >> 32) as i32);
            let section_y = host.min_section()?;
            let starts_chunk = host.load_starts_chunk(referenced_chunk)?;
            if let Some(start) = host.start_for_structure(&starts_chunk, section_y, &structure)? {
                if host.start_is_valid(&start)? {
                    starts.push(start);
                }
            }
        }
    }

    // SectionPos.sectionToBlockCoord and subsequent Java int operations wrap.
    let min_x = chunk.0.wrapping_shl(4);
    let min_z = chunk.1.wrapping_shl(4);
    let near_min_x = min_x.wrapping_sub(12);
    let near_min_z = min_z.wrapping_sub(12);
    let near_max_x = min_x.wrapping_add(15).wrapping_add(12);
    let near_max_z = min_z.wrapping_add(15).wrapping_add(12);
    let mut rigids = Vec::new();
    let mut junctions = Vec::new();
    for start in starts {
        let structure = host.start_structure(&start)?;
        let terrain_adjustment = host.terrain_adjustment(&structure)?;
        for piece in host.pieces(&start)? {
            let bounds = host.piece_bounds(&piece)?;
            // StructurePiece.isCloseToChunk uses inclusive horizontal overlap.
            if bounds.max_x < near_min_x
                || bounds.min_x > near_max_x
                || bounds.max_z < near_min_z
                || bounds.min_z > near_max_z
            {
                continue;
            }
            match host.pool_projection(&piece)? {
                Some(projection) => {
                    if projection == PoolProjection::Rigid {
                        rigids.push(Rigid {
                            bounds: host.piece_bounds(&piece)?,
                            terrain_adjustment,
                            ground_level_delta: host.pool_ground_level_delta(&piece)?,
                        });
                    }
                    // A terrain-matching pool piece still contributes junctions.
                    for junction in host.pool_junctions(&piece)? {
                        if junction.source_x > near_min_x
                            && junction.source_z > near_min_z
                            && junction.source_x < near_max_x
                            && junction.source_z < near_max_z
                        {
                            junctions.push(junction);
                        }
                    }
                }
                None => rigids.push(Rigid {
                    bounds: host.piece_bounds(&piece)?,
                    terrain_adjustment,
                    ground_level_delta: 0,
                }),
            }
        }
    }
    Ok(Beardifier::from_ordered_parts(rigids, junctions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{bail, ensure};

    // Controlled owners for source-order and boundary regressions, not an
    // original StructureManager or a production dependency/status adapter.
    #[derive(Default)]
    struct ObservedOwners {
        trace: Vec<String>,
        unavailable_starts: bool,
    }

    impl BeardifierSelectionHost for ObservedOwners {
        type ReferencesChunk = ();
        type Structure = u8;
        type ReferenceSet = u8;
        type StartsChunk = i32;
        type Start = u8;
        type Piece = u8;

        fn load_references_chunk(&mut self, chunk: (i32, i32)) -> Result<()> {
            ensure!(chunk == (-1, 2), "unavailable reference owner");
            self.trace.push("references:-1,2".into());
            Ok(())
        }
        fn reference_entries(&mut self, _: &()) -> Result<Vec<(u8, u8)>> {
            Ok(vec![(0, 0), (1, 1), (2, 2)])
        }
        fn terrain_adjustment(&mut self, structure: &u8) -> Result<TerrainAdjustment> {
            Ok(match structure {
                0 => TerrainAdjustment::None,
                1 => TerrainAdjustment::Bury,
                2 => TerrainAdjustment::Encapsulate,
                3 => TerrainAdjustment::BeardThin,
                _ => bail!("unavailable structure owner"),
            })
        }
        fn reference_positions(&mut self, references: &u8) -> Result<Vec<i64>> {
            self.trace.push(format!("reference-set:{references}"));
            // Distinct reference sets can resolve the same retained start.
            match references {
                1 => Ok(vec![0, 1, 2]),
                2 => Ok(vec![0]),
                _ => bail!("filtered or unavailable reference set"),
            }
        }
        fn min_section(&mut self) -> Result<i32> {
            Ok(-4)
        }
        fn load_starts_chunk(&mut self, chunk: (i32, i32)) -> Result<i32> {
            self.trace.push(format!("starts:{},{}", chunk.0, chunk.1));
            ensure!(
                chunk.1 == 0 && (0..=2).contains(&chunk.0),
                "unavailable STARTS owner"
            );
            ensure!(
                !(self.unavailable_starts && chunk.0 == 1),
                "missing actual STARTS dependency"
            );
            Ok(chunk.0)
        }
        fn start_for_structure(
            &mut self,
            chunk: &i32,
            section_y: i32,
            _: &u8,
        ) -> Result<Option<u8>> {
            ensure!(section_y == -4, "wrong section owner");
            Ok(match chunk {
                0 => Some(10),
                1 => None, // An observed null, unlike an unavailable chunk.
                2 => Some(20),
                _ => bail!("unavailable STARTS owner"),
            })
        }
        fn start_is_valid(&mut self, start: &u8) -> Result<bool> {
            match start {
                10 => Ok(true),
                20 => Ok(false),
                _ => bail!("unavailable start owner"),
            }
        }
        fn start_structure(&mut self, start: &u8) -> Result<u8> {
            ensure!(*start == 10, "unexpected invalid start traversal");
            Ok(3)
        }
        fn pieces(&mut self, start: &u8) -> Result<Vec<u8>> {
            ensure!(*start == 10, "unavailable valid start");
            self.trace.push(format!("pieces:{start}"));
            Ok(vec![0, 1, 2, 3])
        }
        fn piece_bounds(&mut self, piece: &u8) -> Result<BlockBox> {
            let x = match piece {
                0 => -28, // Inclusive lower edge of chunk -1's expanded box.
                1 => -27,
                2 => 11, // Inclusive upper edge.
                3 => 12, // Outside: no pool/projection observation is allowed.
                _ => bail!("unavailable piece"),
            };
            Ok(BlockBox {
                min_x: x,
                min_y: 5000,
                min_z: 32,
                max_x: x,
                max_y: 5000,
                max_z: 32,
            })
        }
        fn pool_projection(&mut self, piece: &u8) -> Result<Option<PoolProjection>> {
            match piece {
                0 => Ok(None),
                1 => Ok(Some(PoolProjection::TerrainMatching)),
                2 => Ok(Some(PoolProjection::Rigid)),
                _ => bail!("outside or unavailable piece must not be visited"),
            }
        }
        fn pool_ground_level_delta(&mut self, piece: &u8) -> Result<i32> {
            ensure!(*piece == 2, "non-rigid pool delta must not be read");
            Ok(7)
        }
        fn pool_junctions(&mut self, piece: &u8) -> Result<Vec<Junction>> {
            let junction = |x, y, z| Junction {
                source_x: x,
                source_ground_y: y,
                source_z: z,
            };
            match piece {
                1 => Ok(vec![
                    junction(-28, 0, 33),
                    junction(-27, 10000, 33),
                    junction(11, 0, 33),
                ]),
                2 => Ok(vec![
                    junction(0, -10000, 33),
                    junction(0, 0, 20),
                    junction(0, 0, 59),
                ]),
                _ => bail!("ordinary or unavailable piece has no pool junction owner"),
            }
        }
    }

    #[test]
    fn source_two_phase_order_and_different_edge_rules() -> Result<()> {
        let mut host = ObservedOwners::default();
        let selected = select_beardifier(&mut host, (-1, 2))?;
        assert_eq!(
            host.trace,
            [
                "references:-1,2",
                "reference-set:1",
                "starts:0,0",
                "starts:1,0",
                "starts:2,0",
                "reference-set:2",
                "starts:0,0",
                "pieces:10",
                "pieces:10",
            ]
        );
        let rigids = selected.rigids();
        assert_eq!(rigids.len(), 4);
        assert_eq!([rigids[0].bounds.min_x, rigids[1].bounds.min_x], [-28, 11]);
        assert_eq!(
            [rigids[0].ground_level_delta, rigids[1].ground_level_delta],
            [0, 7]
        );
        assert!(rigids
            .iter()
            .all(|r| r.terrain_adjustment == TerrainAdjustment::BeardThin));
        assert_eq!(&rigids[..2], &rigids[2..]);
        let first = Junction {
            source_x: -27,
            source_ground_y: 10000,
            source_z: 33,
        };
        let second = Junction {
            source_x: 0,
            source_ground_y: -10000,
            source_z: 33,
        };
        assert_eq!(selected.junctions(), &[first, second, first, second]);
        Ok(())
    }

    #[test]
    fn missing_starts_owner_errors_before_any_piece_traversal() {
        let mut host = ObservedOwners {
            unavailable_starts: true,
            ..Default::default()
        };
        let error = select_beardifier(&mut host, (-1, 2)).unwrap_err();
        assert!(error
            .to_string()
            .contains("missing actual STARTS dependency"));
        assert_eq!(
            host.trace,
            [
                "references:-1,2",
                "reference-set:1",
                "starts:0,0",
                "starts:1,0"
            ]
        );
    }
}
