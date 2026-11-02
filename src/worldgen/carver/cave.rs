//! CaveWorldCarver's ordered system, room and recursive tunnel geometry.
//!
//! The invocation RNG is supplied by applyCarvers. Every tunnel creates the
//! original independent Legacy source from its supplied seed; its draws do not
//! advance the invocation WorldgenRandom or its next(bits) counter.

use super::{
    config::CaveCarverConfig,
    host::CarverHost,
    mask::CarvingMask,
    providers::HeightContext,
    world_carver::{can_reach, carve_ellipsoid, WorldCarverKind, GENERATION_RANGE},
};
use crate::{
    geometry::{vanilla_cos, vanilla_sin},
    random::{LegacyRandomSource, RandomSource},
};
use anyhow::Result;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Original inclusive probability comparison; always consumes the start draw.
pub fn is_start_chunk(config: &CaveCarverConfig, random: &mut impl RandomSource) -> bool {
    random.next_f32() <= config.base.probability
}

pub fn carve(
    host: &mut impl CarverHost,
    config: &CaveCarverConfig,
    random: &mut impl RandomSource,
    origin: (i32, i32),
    mask: &mut CarvingMask,
) -> Result<bool> {
    carve_kind(
        host,
        config,
        mask,
        random,
        origin,
        WorldCarverKind::Overworld,
    )
}

pub(super) fn carve_kind(
    host: &mut impl CarverHost,
    config: &CaveCarverConfig,
    mask: &mut CarvingMask,
    random: &mut impl RandomSource,
    origin: (i32, i32),
    kind: WorldCarverKind,
) -> Result<bool> {
    let range = (GENERATION_RANGE * 2 - 1) << 4;
    let cave_bound = match kind {
        WorldCarverKind::Overworld => 15,
        WorldCarverKind::Nether => 10,
    };
    let inner = random.next_bounded_i32(cave_bound)?;
    let middle = random.next_bounded_i32(inner + 1)?;
    let systems = random.next_bounded_i32(middle + 1)?;
    for _ in 0..systems {
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
        let horizontal_multiplier = f64::from(config.horizontal_radius_multiplier.sample(random));
        let vertical_multiplier = f64::from(config.vertical_radius_multiplier.sample(random));
        let floor_level = f64::from(config.floor_level.sample(random));
        let mut tunnels = 1;
        if random.next_bounded_i32(4)? == 0 {
            let y_scale = f64::from(config.base.y_scale.sample(random));
            let thickness = 1.0_f32 + random.next_f32() * 6.0_f32;
            create_room(
                host,
                config,
                mask,
                kind,
                [x, y, z],
                thickness,
                y_scale,
                floor_level,
            )?;
            tunnels += random.next_bounded_i32(4)?;
        }
        for _ in 0..tunnels {
            let yaw = random.next_f32() * TAU;
            let pitch = (random.next_f32() - 0.5_f32) / 4.0_f32;
            let thickness = thickness(random, kind)?;
            let end = range - random.next_bounded_i32(range / 4)?;
            let seed = random.next_i64();
            let y_scale = match kind {
                WorldCarverKind::Overworld => 1.0,
                WorldCarverKind::Nether => 5.0,
            };
            create_tunnel(
                host,
                config,
                mask,
                kind,
                seed,
                [x, y, z],
                horizontal_multiplier,
                vertical_multiplier,
                thickness,
                yaw,
                pitch,
                0,
                end,
                y_scale,
                floor_level,
            )?;
        }
    }
    // The original returns true even when systems == 0 or no block was carved.
    // This return value does not certify CARVERS status or a completed chunk.
    Ok(true)
}

fn thickness(random: &mut impl RandomSource, kind: WorldCarverKind) -> Result<f32> {
    let mut thickness = random.next_f32() * 2.0_f32 + random.next_f32();
    match kind {
        WorldCarverKind::Nether => thickness *= 2.0_f32,
        WorldCarverKind::Overworld => {
            if random.next_bounded_i32(10)? == 0 {
                thickness *= random.next_f32() * random.next_f32() * 3.0_f32 + 1.0_f32;
            }
        }
    }
    Ok(thickness)
}

fn create_room(
    host: &mut impl CarverHost,
    config: &CaveCarverConfig,
    mask: &mut CarvingMask,
    kind: WorldCarverKind,
    center: [f64; 3],
    thickness: f32,
    y_scale: f64,
    floor_level: f64,
) -> Result<()> {
    let radius = 1.5 + f64::from(vanilla_sin(FRAC_PI_2) * thickness);
    let vertical = radius * y_scale;
    carve_ellipsoid(
        host,
        &config.base,
        mask,
        kind,
        [center[0] + 1.0, center[1], center[2]],
        radius,
        vertical,
        |x, y, z, _| should_skip(x, y, z, floor_level),
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_tunnel(
    host: &mut impl CarverHost,
    config: &CaveCarverConfig,
    mask: &mut CarvingMask,
    kind: WorldCarverKind,
    seed: i64,
    mut center: [f64; 3],
    horizontal_multiplier: f64,
    vertical_multiplier: f64,
    thickness: f32,
    mut yaw: f32,
    mut pitch: f32,
    start: i32,
    end: i32,
    y_scale: f64,
    floor_level: f64,
) -> Result<()> {
    // RandomSource.create(seed) is exactly new LegacyRandomSource(seed), not a
    // WorldgenRandom wrapper, an invocation-source fork or a positional stream.
    let mut random = LegacyRandomSource::new(seed);
    let branch = random.next_bounded_i32(end / 2)? + end / 4;
    let gentle = random.next_bounded_i32(6)? == 0;
    let mut yaw_delta = 0.0_f32;
    let mut pitch_delta = 0.0_f32;
    for step in start..end {
        let radius = 1.5 + f64::from(vanilla_sin(PI * step as f32 / end as f32) * thickness);
        let vertical = radius * y_scale;
        let cos_pitch = vanilla_cos(pitch);
        center[0] += f64::from(vanilla_cos(yaw) * cos_pitch);
        center[1] += f64::from(vanilla_sin(pitch));
        center[2] += f64::from(vanilla_sin(yaw) * cos_pitch);
        pitch *= if gentle { 0.92_f32 } else { 0.7_f32 };
        pitch += pitch_delta * 0.1_f32;
        yaw += yaw_delta * 0.1_f32;
        pitch_delta *= 0.9_f32;
        yaw_delta *= 0.75_f32;
        pitch_delta += (random.next_f32() - random.next_f32()) * random.next_f32() * 2.0_f32;
        yaw_delta += (random.next_f32() - random.next_f32()) * random.next_f32() * 4.0_f32;
        if step == branch && thickness > 1.0_f32 {
            let seed = random.next_i64();
            let child_thickness = random.next_f32() * 0.5_f32 + 0.5_f32;
            create_tunnel(
                host,
                config,
                mask,
                kind,
                seed,
                center,
                horizontal_multiplier,
                vertical_multiplier,
                child_thickness,
                yaw - FRAC_PI_2,
                pitch / 3.0_f32,
                step,
                end,
                1.0,
                floor_level,
            )?;
            // The left child completes before the parent draws the right seed
            // and thickness. Children create independent local Legacy sources.
            let seed = random.next_i64();
            let child_thickness = random.next_f32() * 0.5_f32 + 0.5_f32;
            create_tunnel(
                host,
                config,
                mask,
                kind,
                seed,
                center,
                horizontal_multiplier,
                vertical_multiplier,
                child_thickness,
                yaw + FRAC_PI_2,
                pitch / 3.0_f32,
                step,
                end,
                1.0,
                floor_level,
            )?;
            return Ok(());
        }
        if random.next_bounded_i32(4)? != 0 {
            if !can_reach(host.chunk_pos(), center, step, end, thickness) {
                return Ok(());
            }
            carve_ellipsoid(
                host,
                &config.base,
                mask,
                kind,
                center,
                radius * horizontal_multiplier,
                vertical * vertical_multiplier,
                |x, y, z, _| should_skip(x, y, z, floor_level),
            )?;
        }
    }
    Ok(())
}

fn should_skip(x: f64, y: f64, z: f64, floor_level: f64) -> bool {
    y <= floor_level || x * x + y * y + z * z >= 1.0
}
