//! Original old-generation blending data and retained Blender owner.
//!
//! Region callbacks require actual chunk, storage-age and dependency owners.
//! An unavailable observation is an error, never evidence of empty blending.
pub mod biome;
mod blender;
pub mod data;
mod density;
mod ordered_map;

pub use biome::BlendedBiomeResolver;
pub use blender::height_to_offset;
pub use data::{BlendingData, Direction8, Direction8Set, OldChunkDataHost, SerializedBlendingData};

use crate::density::DensityPoint;
use anyhow::{ensure, Result};
use ordered_map::SourceLongMap;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlendingOutput {
    pub alpha: f64,
    pub blending_offset: f64,
}
impl BlendingOutput {
    pub const EMPTY: Self = Self {
        alpha: 1.0,
        blending_offset: 0.0,
    };
}

/// ChunkPos.asLong: X occupies the low unsigned half, Z the high half.
pub const fn chunk_key(x: i32, z: i32) -> i64 {
    ((x as u32 as u64) | ((z as u32 as u64) << 32)) as i64
}

/// Concrete capabilities used by Blender.of and BlendingData.getOrUpdate.
///
/// `Chunk` and `Data` identify the actual retained source owners. The final
/// calculation must update that same data owner once and return its immutable
/// calculated Arc; it must not recalculate a detached replacement per query.
/// All eight neighbors are observed even when calculation already completed.
/// Missing dependencies return Err, separately from an observed empty slot or
/// actual highest-generated status below BIOMES. No methods have empty defaults.
pub trait BlendingRegionHost {
    type Chunk;
    type Data;

    fn center_chunk(&self) -> (i32, i32);
    /// The source guard is backed by the level's actual chunk-storage/IOWorker
    /// old-generation scan. Loaded image/data-slot absence cannot replace it.
    fn is_old_chunk_around(&mut self, center: (i32, i32), radius: i32) -> Result<bool>;
    /// Original region getChunk with its real generating-step dependency cache.
    /// Neighbor-age reads can reach radius eight; this creates no replacement.
    fn chunk(&mut self, x: i32, z: i32) -> Result<Self::Chunk>;
    fn blending_data(&mut self, chunk: &Self::Chunk) -> Result<Option<Self::Data>>;
    /// Actual getHighestGeneratedStatus, including an actual retrogen target.
    fn highest_generated_at_least_biomes(&mut self, chunk: &Self::Chunk) -> Result<bool>;
    /// ChunkAccess.isOldNoiseGeneration: actual BlendingData-slot presence.
    fn is_old_noise_generation(&mut self, chunk: &Self::Chunk) -> Result<bool>;
    fn calculate_data(
        &mut self,
        chunk: &Self::Chunk,
        data: Self::Data,
        new_generation_neighbors: Direction8Set,
    ) -> Result<Arc<BlendingData>>;
}

/// Read-only calculated data retained by every stage of one NoiseChunk job.
/// The singleton identity is distinct from a constructed Blender with zero
/// maps: source wrapNew checks the identity, rather than numerical outputs.
#[derive(Debug)]
pub struct Blender {
    empty_singleton: bool,
    height_and_biome: SourceLongMap<Arc<BlendingData>>,
    density: SourceLongMap<Arc<BlendingData>>,
}

#[derive(Clone, Copy)]
pub(super) enum CellGetter {
    Height,
    Density,
}

impl Blender {
    pub fn empty() -> Arc<Self> {
        static EMPTY: OnceLock<Arc<Blender>> = OnceLock::new();
        Arc::clone(EMPTY.get_or_init(|| {
            Arc::new(Self {
                empty_singleton: true,
                height_and_biome: SourceLongMap::new(),
                density: SourceLongMap::new(),
            })
        }))
    }
    pub fn is_empty(&self) -> bool {
        self.empty_singleton
    }

    /// Retain calculated owners in the given original insertion order. This
    /// models Blender's map constructor and deliberately retains non-singleton
    /// identity even for zero entries. Ordinary region collection below has its
    /// separate source rule returning the singleton when both maps are empty.
    pub fn from_calculated_data(
        heights: impl IntoIterator<Item = (i64, Arc<BlendingData>)>,
        densities: impl IntoIterator<Item = (i64, Arc<BlendingData>)>,
    ) -> Result<Arc<Self>> {
        let mut height_and_biome = SourceLongMap::new();
        let mut density = SourceLongMap::new();
        for (key, data) in heights {
            ensure!(
                data.is_calculated(),
                "Blender requires the actual calculated old data owner"
            );
            height_and_biome.insert(key, data);
        }
        for (key, data) in densities {
            ensure!(
                data.is_calculated(),
                "Blender requires the actual calculated old data owner"
            );
            density.insert(key, data);
        }
        Ok(Arc::new(Self {
            empty_singleton: false,
            height_and_biome,
            density,
        }))
    }

    /// Source Blender.of scan, over mandatory actual storage/region owners.
    /// Errors preserve prior source cache/calculation effects and do not return
    /// an empty substitute or certify that a generation status completed.
    pub fn from_region(host: &mut impl BlendingRegionHost) -> Result<Arc<Self>> {
        let center = host.center_chunk();
        if !host.is_old_chunk_around(center, 7)? {
            return Ok(Self::empty());
        }
        let mut height_and_biome = SourceLongMap::new();
        let mut density = SourceLongMap::new();
        for dx in -7_i32..=7 {
            for dz in -7_i32..=7 {
                if dx * dx + dz * dz > 64 {
                    continue;
                }
                let x = center.0.wrapping_add(dx);
                let z = center.1.wrapping_add(dz);
                let Some(data) = Self::get_or_update_data(host, x, z)? else {
                    continue;
                };
                let key = chunk_key(x, z);
                height_and_biome.insert(key, Arc::clone(&data));
                if (-1..=1).contains(&dx) && (-1..=1).contains(&dz) {
                    density.insert(key, data);
                }
            }
        }
        if height_and_biome.is_empty() && density.is_empty() {
            return Ok(Self::empty());
        }
        Ok(Arc::new(Self {
            empty_singleton: false,
            height_and_biome,
            density,
        }))
    }

    fn get_or_update_data(
        host: &mut impl BlendingRegionHost,
        x: i32,
        z: i32,
    ) -> Result<Option<Arc<BlendingData>>> {
        let chunk = host.chunk(x, z)?;
        let Some(data) = host.blending_data(&chunk)? else {
            return Ok(None);
        };
        if !host.highest_generated_at_least_biomes(&chunk)? {
            return Ok(None);
        }
        let mut new_neighbors = Direction8Set::NONE;
        for direction in Direction8::ALL {
            let (dx, dz) = direction.offset();
            let neighbor = host.chunk(x.wrapping_add(dx), z.wrapping_add(dz))?;
            if !host.is_old_noise_generation(&neighbor)? {
                new_neighbors.insert(direction);
            }
        }
        let calculated = host.calculate_data(&chunk, data, new_neighbors)?;
        ensure!(
            calculated.is_calculated(),
            "old data owner did not finish its actual calculation"
        );
        Ok(Some(calculated))
    }

    /// Fastutil forEach order, including key zero and resize/rehash behavior.
    pub fn height_and_biome_entries(&self) -> impl Iterator<Item = (i64, &BlendingData)> {
        self.height_and_biome
            .iter()
            .map(|(key, data)| (key, data.as_ref()))
    }
    pub fn density_entries(&self) -> impl Iterator<Item = (i64, &BlendingData)> {
        self.density.iter().map(|(key, data)| (key, data.as_ref()))
    }
    pub fn height_and_biome_owner(&self, key: i64) -> Option<&Arc<BlendingData>> {
        self.height_and_biome.get(key)
    }
    pub fn density_owner(&self, key: i64) -> Option<&Arc<BlendingData>> {
        self.density.get(key)
    }

    pub fn blend_offset_and_factor(&self, x: i32, z: i32) -> BlendingOutput {
        if self.is_empty() {
            BlendingOutput::EMPTY
        } else {
            blender::blend_offset_and_factor(self, x, z)
        }
    }
    pub fn blend_density(&self, point: DensityPoint, raw: f64) -> f64 {
        if self.is_empty() {
            raw
        } else {
            density::blend_density(self, point, raw)
        }
    }
    pub fn blend_biome(&self, x: i32, y: i32, z: i32) -> Option<String> {
        if self.is_empty() {
            None
        } else {
            biome::blend_biome(self, x, y, z)
        }
    }

    pub(super) fn get_blending_data_value(
        &self,
        qx: i32,
        cell_y: i32,
        qz: i32,
        getter: CellGetter,
    ) -> f64 {
        let cx = qx >> 2;
        let cz = qz >> 2;
        let read = |x: i32, z: i32| {
            self.height_and_biome
                .get(chunk_key(x, z))
                .map_or(f64::MAX, |data| {
                    let lx = qx.wrapping_sub(x.wrapping_shl(2));
                    let lz = qz.wrapping_sub(z.wrapping_shl(2));
                    match getter {
                        CellGetter::Height => data.get_height(lx, cell_y, lz),
                        CellGetter::Density => data.get_density(lx, cell_y, lz),
                    }
                })
        };
        let value = read(cx, cz);
        if value != f64::MAX {
            return value;
        }
        let on_x = qx & 3 == 0;
        let on_z = qz & 3 == 0;
        if on_x && on_z {
            let value = read(cx.wrapping_sub(1), cz.wrapping_sub(1));
            if value != f64::MAX {
                return value;
            }
        }
        if on_x {
            let value = read(cx.wrapping_sub(1), cz);
            if value != f64::MAX {
                return value;
            }
        }
        if on_z {
            let value = read(cx, cz.wrapping_sub(1));
            if value != f64::MAX {
                return value;
            }
        }
        f64::MAX
    }
}
