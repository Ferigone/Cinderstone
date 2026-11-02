//! Scalar Beardifier arithmetic from the pinned Java 1.21 `dyl` class.
use super::{Beardifier, TerrainAdjustment};
use crate::density::DensityPoint;
use std::sync::OnceLock;

const KERNEL_SIZE: i32 = 24;
const KERNEL_RADIUS: i32 = 12;
const KERNEL_VOLUME: usize = 24 * 24 * 24;

/// Preserve source encounter order: all rigid pieces, then all junctions.
pub(super) fn compute(owner: &Beardifier, point: DensityPoint) -> f64 {
    let mut value = 0.0;
    for rigid in owner.rigids() {
        let bounds = rigid.bounds;
        let dx = outside_distance(bounds.min_x, bounds.max_x, point.x);
        let dz = outside_distance(bounds.min_z, bounds.max_z, point.z);
        let ground_y = bounds.min_y.wrapping_add(rigid.ground_level_delta);
        let ground_dy = point.y.wrapping_sub(ground_y);
        let contribution = match rigid.terrain_adjustment {
            TerrainAdjustment::None => 0.0,
            TerrainAdjustment::Bury => {
                bury(f64::from(dx), f64::from(ground_dy) / 2.0, f64::from(dz))
            }
            TerrainAdjustment::BeardThin => beard(dx, ground_dy, dz, ground_dy) * 0.8,
            TerrainAdjustment::BeardBox => {
                let dy = outside_distance(ground_y, bounds.max_y, point.y);
                beard(dx, dy, dz, ground_dy) * 0.8
            }
            TerrainAdjustment::Encapsulate => {
                let dy = outside_distance(bounds.min_y, bounds.max_y, point.y);
                bury(
                    f64::from(dx) / 2.0,
                    f64::from(dy) / 2.0,
                    f64::from(dz) / 2.0,
                ) * 0.8
            }
        };
        value += contribution;
    }
    for junction in owner.junctions() {
        let dx = point.x.wrapping_sub(junction.source_x);
        let dy = point.y.wrapping_sub(junction.source_ground_y);
        let dz = point.z.wrapping_sub(junction.source_z);
        value += beard(dx, dy, dz, dy) * 0.4;
    }
    value
}

fn outside_distance(minimum: i32, maximum: i32, value: i32) -> i32 {
    0.max(minimum.wrapping_sub(value).max(value.wrapping_sub(maximum)))
}

fn length_squared(x: f64, y: f64, z: f64) -> f64 {
    (x * x + y * y) + z * z
}

fn bury(x: f64, y: f64, z: f64) -> f64 {
    let distance = length_squared(x, y, z).sqrt();
    // Mth.clampedMap(distance, 0, 6, 1, 0), retaining inverseLerp and
    // clampedLerp's arithmetic and comparison order.
    let t = (distance - 0.0) / (6.0 - 0.0);
    if t < 0.0 {
        1.0
    } else if t > 1.0 {
        0.0
    } else {
        1.0 + t * (0.0 - 1.0)
    }
}

fn beard(x: i32, y: i32, z: i32, ground_dy: i32) -> f64 {
    let ix = x.wrapping_add(KERNEL_RADIUS);
    let iy = y.wrapping_add(KERNEL_RADIUS);
    let iz = z.wrapping_add(KERNEL_RADIUS);
    if !(0..KERNEL_SIZE).contains(&ix)
        || !(0..KERNEL_SIZE).contains(&iy)
        || !(0..KERNEL_SIZE).contains(&iz)
    {
        return 0.0;
    }
    // The signed ground-plane distance controls the sign and inverse square
    // root even when BEARD_BOX uses a different nonnegative table Y index.
    let half_y = f64::from(ground_dy) + 0.5;
    let square = length_squared(f64::from(x), half_y, f64::from(z));
    let scale = (-half_y * fast_inv_sqrt(square / 2.0)) / 2.0;
    let index = (iz * KERNEL_SIZE * KERNEL_SIZE + ix * KERNEL_SIZE + iy) as usize;
    scale * f64::from(kernel()[index])
}

fn fast_inv_sqrt(mut value: f64) -> f64 {
    let half = 0.5 * value;
    let bits = value.to_bits() as i64;
    let guess = 6_910_469_410_427_058_090_i64.wrapping_sub(bits >> 1);
    value = f64::from_bits(guess as u64);
    value *= 1.5 - (half * value) * value;
    value
}

fn kernel() -> &'static [f32; KERNEL_VOLUME] {
    static KERNEL: OnceLock<[f32; KERNEL_VOLUME]> = OnceLock::new();
    KERNEL.get_or_init(|| {
        std::array::from_fn(|index| {
            let z = index as i32 / (KERNEL_SIZE * KERNEL_SIZE) - KERNEL_RADIUS;
            let x = index as i32 / KERNEL_SIZE % KERNEL_SIZE - KERNEL_RADIUS;
            let y = index as i32 % KERNEL_SIZE - KERNEL_RADIUS;
            let square = length_squared(f64::from(x), f64::from(y) + 0.5, f64::from(z));
            // Java stores only this final f32 result of Math.pow(E, exponent).
            std::f64::consts::E.powf(-square / 16.0) as f32
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct OriginalKernel {
        original_inner_sha256: String,
        bits: Vec<u32>,
    }

    #[test]
    fn native_kernel_rounds_to_every_actual_original_float() {
        let observed: OriginalKernel = serde_json::from_str(include_str!(
            "../../../fixtures/world-generation-beardifier-kernel.json"
        ))
        .unwrap();
        assert_eq!(
            observed.original_inner_sha256,
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        assert_eq!(observed.bits.len(), KERNEL_VOLUME);
        for (index, (native, actual)) in kernel().iter().zip(observed.bits).enumerate() {
            assert_eq!(native.to_bits(), actual, "original kernel index {index}");
        }
    }
}
