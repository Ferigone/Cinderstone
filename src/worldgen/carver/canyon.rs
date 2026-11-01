//! CanyonWorldCarver's start draw, local path and width-dependent skip shape.
//!
//! applyCarvers owns the supplied invocation WorldgenRandom. Exactly one long
//! from it seeds a separate Legacy path source; width/radius/path draws never
//! advance that invocation owner. Material changes use only the shared kernel.

use super::{
    config::CanyonCarverConfig,
    host::CarverHost,
    mask::CarvingMask,
    providers::HeightContext,
    world_carver::{can_reach, carve_ellipsoid, WorldCarverKind, GENERATION_RANGE},
};
use crate::{
    geometry::{vanilla_cos, vanilla_sin},
    random::{LegacyRandomSource, RandomSource},
};
use anyhow::{Context, Result};
use std::f32::consts::{PI, TAU};

/// Original inclusive start probability; consumes a float even at probability 0.
pub fn is_start_chunk(config: &CanyonCarverConfig, random: &mut impl RandomSource) -> bool {
    random.next_f32() <= config.base.probability
}

pub fn carve(
    host: &mut impl CarverHost,
    config: &CanyonCarverConfig,
    random: &mut impl RandomSource,
    origin: (i32, i32),
    mask: &mut CarvingMask,
) -> Result<bool> {
    let range = (GENERATION_RANGE * 2 - 1) << 4;
    let x = f64::from(
        origin
            .0
            .wrapping_shl(4)
            .wrapping_add(random.next_bounded_i32(16)?),
    );
    let y = f64::from(config.base.y.sample(
        random,
        HeightContext {
            min_gen_y: host.min_gen_y(),
            gen_depth: host.gen_depth(),
        },
    )?);
    let z = f64::from(
        origin
            .1
            .wrapping_shl(4)
            .wrapping_add(random.next_bounded_i32(16)?),
    );
    let yaw = random.next_f32() * TAU;
    let pitch = config.vertical_rotation.sample(random);
    let y_scale = f64::from(config.base.y_scale.sample(random));
    let thickness = config.shape.thickness.sample(random);
    let end = (range as f32 * config.shape.distance_factor.sample(random)) as i32;
    let seed = random.next_i64();
    do_carve(
        host,
        config,
        mask,
        seed,
        [x, y, z],
        thickness,
        yaw,
        pitch,
        0,
        end,
        y_scale,
    )?;
    // This is the original return contract, including paths with no edits.
    // It does not certify a CARVERS status or a completed runtime chunk.
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn do_carve(
    host: &mut impl CarverHost,
    config: &CanyonCarverConfig,
    mask: &mut CarvingMask,
    seed: i64,
    mut center: [f64; 3],
    thickness: f32,
    mut yaw: f32,
    mut pitch: f32,
    start: i32,
    end: i32,
    y_scale: f64,
) -> Result<()> {
    // RandomSource.create(seed), not a fork/reseed of the invocation source.
    let mut random = LegacyRandomSource::new(seed);
    let width_factors =
        init_width_factors(host.gen_depth(), config.shape.width_smoothness, &mut random)?;
    let min_gen_y = host.min_gen_y();
    let mut yaw_delta = 0.0_f32;
    let mut pitch_delta = 0.0_f32;
    for step in start..end {
        // Every multiply/divide before f2d is a separate float operation.
        let base_radius = 1.5 + f64::from(vanilla_sin(step as f32 * PI / end as f32) * thickness);
        let initial_vertical_radius = base_radius * y_scale;
        let horizontal_radius =
            base_radius * f64::from(config.shape.horizontal_radius_factor.sample(&mut random));
        let vertical_radius = update_vertical_radius(
            config,
            &mut random,
            initial_vertical_radius,
            end as f32,
            step as f32,
        );
        let pitch_cos = vanilla_cos(pitch);
        let pitch_sin = vanilla_sin(pitch);
        center[0] += f64::from(vanilla_cos(yaw) * pitch_cos);
        center[1] += f64::from(pitch_sin);
        center[2] += f64::from(vanilla_sin(yaw) * pitch_cos);
        pitch *= 0.7_f32;
        pitch += pitch_delta * 0.05_f32;
        yaw += yaw_delta * 0.05_f32;
        pitch_delta *= 0.8_f32;
        yaw_delta *= 0.5_f32;
        pitch_delta += (random.next_f32() - random.next_f32()) * random.next_f32() * 2.0_f32;
        yaw_delta += (random.next_f32() - random.next_f32()) * random.next_f32() * 4.0_f32;
        if random.next_bounded_i32(4)? == 0 {
            continue;
        }
        if !can_reach(host.chunk_pos(), center, step, end, thickness) {
            return Ok(());
        }
        carve_ellipsoid(
            host,
            &config.base,
            mask,
            WorldCarverKind::Overworld,
            center,
            horizontal_radius,
            vertical_radius,
            |nx, ny, nz, absolute_y| {
                // The sole shared kernel retains the original minimum-Y + 1
                // clip. Do not clamp this array index or invent a width value.
                let index = absolute_y.wrapping_sub(min_gen_y).wrapping_sub(1) as usize;
                (nx * nx + nz * nz) * f64::from(width_factors[index]) + ny * ny / 6.0 >= 1.0
            },
        )?;
    }
    Ok(())
}

fn init_width_factors(
    gen_depth: i32,
    width_smoothness: i32,
    random: &mut impl RandomSource,
) -> Result<Vec<f32>> {
    let length = usize::try_from(gen_depth).context("negative Canyon width-factor array length")?;
    let mut factors = Vec::with_capacity(length);
    let mut width = 1.0_f32;
    for y in 0..length {
        // The first entry skips nextInt completely. Subsequent entries always
        // draw nextInt, drawing the two floats only when its result is zero.
        if y == 0 || random.next_bounded_i32(width_smoothness)? == 0 {
            width = 1.0_f32 + random.next_f32() * random.next_f32();
        }
        factors.push(width * width);
    }
    Ok(factors)
}

fn update_vertical_radius(
    config: &CanyonCarverConfig,
    random: &mut impl RandomSource,
    radius: f64,
    end: f32,
    step: f32,
) -> f64 {
    let center_factor = 1.0_f32 - (0.5_f32 - step / end).abs() * 2.0_f32;
    let factor = config.shape.vertical_radius_default_factor
        + config.shape.vertical_radius_center_factor * center_factor;
    let vertical = f64::from(factor) * radius;
    // Mth.randomBetween(random, .75f, 1f) samples after the radius factor.
    let jitter = random.next_f32() * (1.0_f32 - 0.75_f32) + 0.75_f32;
    vertical * f64::from(jitter)
}
