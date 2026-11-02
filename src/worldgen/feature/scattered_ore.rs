//! ScatteredOreFeature's ordered attempts over the actual caller RNG and world.
//!
//! ConfiguredFeature's origin ensureCanWrite check belongs to the dispatcher,
//! before this leaf is invoked. This leaf performs no per-attempt prefilter,
//! seed construction, visited-position deduplication or substitute placement.

use super::{
    config::OreConfiguration,
    host::{can_place_ore, OreHost},
};
use crate::{density::DensityPoint, random::RandomSource};
use anyhow::Result;

pub fn place(
    host: &mut impl OreHost,
    config: &OreConfiguration,
    random: &mut (impl RandomSource + ?Sized),
    origin: DensityPoint,
) -> Result<bool> {
    // nextInt(1) still consumes a sample for a configured size of zero.
    let attempts = random.next_bounded_i32(config.size.wrapping_add(1))?;
    for attempt in 0..attempts {
        let spread = attempt.min(7);
        // The zero-spread first attempt still consumes all six float samples.
        let dx = random_offset(random, spread);
        let dy = random_offset(random, spread);
        let dz = random_offset(random, spread);
        let point = DensityPoint::new(
            origin.x.wrapping_add(dx),
            origin.y.wrapping_add(dy),
            origin.z.wrapping_add(dz),
        );
        // Capture the state once, even for an empty target list. Failed target
        // rules/air-exposure checks use this same state in original list order.
        let state = host.block(point)?;
        for target in &config.targets {
            if can_place_ore(
                state,
                &mut |point| host.block(point),
                random,
                config,
                target,
                point,
            )? {
                // The source ignores setBlock's boolean and stops at this
                // successful target even if the actual setter returns false.
                let _ = host.set_block(point, target.state, 2)?;
                break;
            }
        }
    }
    // Source success is unconditional after the loop, including zero attempts
    // or writes. It is not a completed FEATURES or runtime-chunk assertion.
    Ok(true)
}

fn random_offset(random: &mut (impl RandomSource + ?Sized), spread: i32) -> i32 {
    let positive = random.next_f32();
    let negative = random.next_f32();
    let value = (positive - negative) * spread as f32;
    round_offset(value)
}

fn round_offset(value: f32) -> i32 {
    // Java Math.round(float) rounds half toward positive infinity. Widen the
    // already-rounded float expression before adding 0.5: doing that addition
    // in float would incorrectly round the representable value just below 0.5.
    (f64::from(value) + 0.5_f64).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::round_offset;

    #[test]
    fn java_float_rounding_preserves_half_boundaries() {
        let half = 0.5_f32.to_bits();
        let negative_half = (-0.5_f32).to_bits();
        for (bits, expected) in [
            (half - 1, 0),
            (half, 1),
            (half + 1, 1),
            (negative_half - 1, 0),
            (negative_half, 0),
            (negative_half + 1, -1),
            ((-1.5_f32).to_bits(), -1),
        ] {
            assert_eq!(round_offset(f32::from_bits(bits)), expected, "bits {bits}");
        }
    }
}
