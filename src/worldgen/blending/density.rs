//! Blender's direct old-density lookup and ordered nearby-sample blend.
use super::{Blender, CellGetter};
use crate::density::DensityPoint;

pub(super) fn blend_density(blender: &Blender, point: DensityPoint, raw: f64) -> f64 {
    let quart_x = point.x >> 2;
    // The original uses integer division, not floorDiv or an arithmetic shift.
    let cell_y = point.y / 8;
    let quart_z = point.z >> 2;

    // Shared source lookup deliberately uses the broad height/biome map,
    // including its current/NW/W/N boundary ownership and exact MAX sentinel.
    let direct = blender.get_blending_data_value(quart_x, cell_y, quart_z, CellGetter::Density);
    if direct != f64::MAX {
        return direct;
    }

    let mut weight_sum = 0.0;
    let mut weighted_density = 0.0;
    let mut nearest = f64::INFINITY;
    for (key, data) in blender.density_entries() {
        let origin_x = (key as i32).wrapping_shl(2);
        let origin_z = ((key >> 32) as i32).wrapping_shl(2);
        data.iterate_densities(
            origin_x,
            origin_z,
            cell_y.wrapping_sub(1),
            cell_y.wrapping_add(1),
            |sample_x, sample_y, sample_z, density| {
                // Preserve Java's integer subtraction/multiplication before
                // widening, then Mth.length's left-associated squared sum.
                let dx = f64::from(quart_x.wrapping_sub(sample_x));
                let dy = f64::from(cell_y.wrapping_sub(sample_y).wrapping_mul(2));
                let dz = f64::from(quart_z.wrapping_sub(sample_z));
                let distance = ((dx * dx + dy * dy) + dz * dz).sqrt();
                if !(distance <= 2.0) {
                    return;
                }
                if distance < nearest {
                    nearest = distance;
                }
                let weight = 1.0 / (((distance * distance) * distance) * distance);
                weighted_density += density * weight;
                weight_sum += weight;
            },
        );
    }
    if nearest == f64::INFINITY {
        return raw;
    }

    let old_density = weighted_density / weight_sum;
    let factor = nearest / 3.0;
    let factor = if factor < 0.0 {
        0.0
    } else if factor > 1.0 {
        1.0
    } else {
        factor
    };
    // Mth.lerp, without a separate vertical fraction or height-blend smoothstep.
    old_density + factor * (raw - old_density)
}
