//! Blender's height-derived offset and horizontal blending factor.
//! Map order and calculated old-generation samples belong to the parent owner.

use super::{Blender, BlendingOutput, CellGetter};

const HEIGHT_BLENDING_RANGE_CELLS: i32 = 27;

pub(super) fn blend_offset_and_factor(blender: &Blender, x: i32, z: i32) -> BlendingOutput {
    // QuartPos.fromBlock uses an arithmetic shift, including negative blocks.
    let qx = x >> 2;
    let qz = z >> 2;
    let direct_height = blender.get_blending_data_value(qx, 0, qz, CellGetter::Height);
    if direct_height != f64::MAX {
        return BlendingOutput {
            alpha: 0.0,
            blending_offset: height_to_offset(direct_height),
        };
    }
    let mut weight_sum = 0.0_f64;
    let mut height_sum = 0.0_f64;
    let mut nearest = f64::INFINITY;
    // This is the original Fastutil map's forEach order, followed by each
    // BlendingData owner's ascending height-array index order. Do not sort or
    // collect samples: accumulation association is part of the source result.
    for (key, data) in blender.height_and_biome_entries() {
        let chunk_x = key as i32;
        let chunk_z = (key >> 32) as i32;
        data.iterate_heights(
            chunk_x.wrapping_shl(2),
            chunk_z.wrapping_shl(2),
            &mut |sample_x, sample_z, height| {
                // Source integer subtraction precedes conversion to double.
                let dx = f64::from(qx.wrapping_sub(sample_x));
                let dz = f64::from(qz.wrapping_sub(sample_z));
                let distance = (dx * dx + dz * dz).sqrt();
                if distance > f64::from(HEIGHT_BLENDING_RANGE_CELLS) {
                    return;
                }
                if distance < nearest {
                    nearest = distance;
                }
                let weight = 1.0 / (distance * distance * distance * distance);
                height_sum += height * weight;
                weight_sum += weight;
            },
        );
    }
    if nearest == f64::INFINITY {
        return BlendingOutput {
            alpha: 1.0,
            blending_offset: 0.0,
        };
    }
    let height = height_sum / weight_sum;
    let fraction = java_clamp(
        nearest / f64::from(HEIGHT_BLENDING_RANGE_CELLS + 1),
        0.0,
        1.0,
    );
    // Preserve the source's multiply/subtract order rather than rewriting as
    // fraction² * (3 - 2 * fraction), which can change the final double bits.
    let alpha = 3.0 * fraction * fraction - 2.0 * fraction * fraction * fraction;
    BlendingOutput {
        alpha,
        blending_offset: height_to_offset(height),
    }
}

/// Original Blender.heightToOffset, including Mth.positiveModulo's two
/// remainders and the original double expression's association.
pub fn height_to_offset(height: f64) -> f64 {
    let shifted = height + 0.5;
    let remainder = ((shifted % 8.0) + 8.0) % 8.0;
    1.0 * (32.0 * (shifted - 128.0) - 3.0 * (shifted - 120.0) * remainder
        + 3.0 * remainder * remainder)
        / (128.0 * (32.0 - 3.0 * remainder))
}

fn java_clamp(value: f64, minimum: f64, maximum: f64) -> f64 {
    if value < minimum {
        minimum
    } else if value > maximum {
        maximum
    } else {
        value
    }
}
