//! Blender's old-biome selection and lazy resolver view.
//!
//! The parent supplies the original ordered old-data entries. Missing old
//! columns are skipped by BlendingData, never filled with a default biome.

use super::Blender;
use crate::{
    noise::{NoiseParameters, NormalNoise},
    random::XoroshiroRandomSource,
    worldgen::{biomes::BiomeResolver, generator::PreparedGeneration},
};
use anyhow::Result;
use std::sync::OnceLock;

const HEIGHT_BLENDING_RANGE_CELLS: i32 = (7 << 2) - 1;

/// The original static Blender.SHIFT_NOISE, independent of world/Level RNGs.
/// NoiseData.DEFAULT_SHIFT is octave -3 with amplitudes [1, 1, 1, 0].
pub(super) fn shift_noise() -> &'static NormalNoise {
    static SHIFT_NOISE: OnceLock<NormalNoise> = OnceLock::new();
    SHIFT_NOISE.get_or_init(|| {
        NormalNoise::new(
            &mut XoroshiroRandomSource::new(42),
            NoiseParameters {
                first_octave: -3,
                amplitudes: vec![1.0, 1.0, 1.0, 0.0],
            },
        )
    })
}

/// Select an actual old biome for these unchanged quart coordinates, or leave
/// selection to the supplied new-generation resolver. Equal-distance points
/// retain the first original map/column observation.
pub fn blend_biome(blender: &Blender, quart_x: i32, quart_y: i32, quart_z: i32) -> Option<String> {
    let mut nearest_distance = f64::INFINITY;
    let mut nearest_biome = None;
    for (key, data) in blender.height_and_biome_entries() {
        let base_x = (key as i32).wrapping_shl(2);
        let base_z = ((key >> 32) as i32).wrapping_shl(2);
        data.iterate_biomes(base_x, quart_y, base_z, |x, z, biome| {
            // Source subtracts as Java ints before widening to double.
            let dx = f64::from(quart_x.wrapping_sub(x));
            let dz = f64::from(quart_z.wrapping_sub(z));
            let distance = (dx * dx + dz * dz).sqrt();
            if distance <= f64::from(HEIGHT_BLENDING_RANGE_CELLS) && distance < nearest_distance {
                nearest_distance = distance;
                nearest_biome = Some(biome.clone());
            }
        });
    }
    if nearest_distance == f64::INFINITY {
        return None;
    }
    let shift = shift_noise().noise(f64::from(quart_x), 0.0, f64::from(quart_z)) * 12.0;
    let factor =
        ((nearest_distance + shift) / f64::from(HEIGHT_BLENDING_RANGE_CELLS + 1)).clamp(0.0, 1.0);
    if factor <= 0.5 {
        nearest_biome
    } else {
        None
    }
}

/// A borrowed Blender.getBiomeResolver view over the caller's actual resolver.
/// The same mutable PreparedGeneration reaches the resolver only on fallback;
/// choosing an old biome does not evaluate climate or change its cache history.
pub struct BlendedBiomeResolver<'a, R: ?Sized> {
    blender: &'a Blender,
    resolver: &'a mut R,
}

impl<'a, R: ?Sized> BlendedBiomeResolver<'a, R> {
    pub fn new(blender: &'a Blender, resolver: &'a mut R) -> Self {
        Self { blender, resolver }
    }
}

impl<R: BiomeResolver + ?Sized> BiomeResolver for BlendedBiomeResolver<'_, R> {
    fn get_noise_biome(
        &mut self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
        sampler: &mut PreparedGeneration,
    ) -> Result<String> {
        match blend_biome(self.blender, quart_x, quart_y, quart_z) {
            Some(biome) => Ok(biome),
            None => self
                .resolver
                .get_noise_biome(quart_x, quart_y, quart_z, sampler),
        }
    }
}
