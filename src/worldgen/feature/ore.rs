//! OreFeature's regular vein geometry and direct section-write path.
//!
//! The configured-feature caller checks origin write permission before this
//! leaf. Every draw here advances its supplied RNG; no local seed is created.

use super::{
    config::OreConfiguration,
    host::{can_place_ore, OreHost},
};
use crate::{density::DensityPoint, geometry::vanilla_sin, hotspot_math, random::RandomSource};
use anyhow::{Context, Result};
use std::f32::consts::PI;

/// Place one already-authorized configured regular-ore invocation. A true
/// result records at least one original section-write attempt, not a FEATURES
/// stage transition or a guarantee that the stored state changed.
pub fn place(
    host: &mut impl OreHost,
    config: &OreConfiguration,
    random: &mut impl RandomSource,
    origin: DensityPoint,
) -> Result<bool> {
    let angle = random.next_f32() * PI;
    let extent = config.size as f32 / 8.0_f32;
    let padding = java_ceil_f32((config.size as f32 / 16.0_f32 * 2.0_f32 + 1.0_f32) / 2.0_f32);
    // The endpoints use java.lang.Math's double trig, unlike the later float
    // Mth.sin radius lookup. This helper preserves the pinned HotSpot kernel.
    let (sin, cos) = hotspot_math::sin_cos(f64::from(angle));
    let x_start = f64::from(origin.x) + sin * f64::from(extent);
    let x_end = f64::from(origin.x) - sin * f64::from(extent);
    let z_start = f64::from(origin.z) + cos * f64::from(extent);
    let z_end = f64::from(origin.z) - cos * f64::from(extent);
    let y_start = f64::from(
        origin
            .y
            .wrapping_add(random.next_bounded_i32(3)?)
            .wrapping_sub(2),
    );
    let y_end = f64::from(
        origin
            .y
            .wrapping_add(random.next_bounded_i32(3)?)
            .wrapping_sub(2),
    );
    let extent_ceil = java_ceil_f32(extent);
    let min_x = origin.x.wrapping_sub(extent_ceil).wrapping_sub(padding);
    let min_y = origin.y.wrapping_sub(2).wrapping_sub(padding);
    let min_z = origin.z.wrapping_sub(extent_ceil).wrapping_sub(padding);
    let horizontal_span = extent_ceil.wrapping_add(padding).wrapping_mul(2);
    let vertical_span = 2_i32.wrapping_add(padding).wrapping_mul(2);
    // Original inclusive X/Z eligibility scan. It returns the result of the
    // first eligible doPlace call, even if that call makes no replacements.
    for x in min_x..=min_x.wrapping_add(horizontal_span) {
        for z in min_z..=min_z.wrapping_add(horizontal_span) {
            if min_y <= host.ocean_floor_wg(x, z)? {
                return do_place(
                    host,
                    config,
                    random,
                    [x_start, y_start, z_start],
                    [x_end, y_end, z_end],
                    DensityPoint::new(min_x, min_y, min_z),
                    horizontal_span,
                    vertical_span,
                );
            }
        }
    }
    Ok(false)
}

#[derive(Clone, Copy)]
struct Sphere {
    center: [f64; 3],
    radius: f64,
}

#[allow(clippy::too_many_arguments)]
fn do_place(
    host: &mut impl OreHost,
    config: &OreConfiguration,
    random: &mut impl RandomSource,
    start: [f64; 3],
    end: [f64; 3],
    minimum: DensityPoint,
    horizontal_span: i32,
    vertical_span: i32,
) -> Result<bool> {
    let capacity = horizontal_span
        .wrapping_mul(vertical_span)
        .wrapping_mul(horizontal_span);
    let capacity = usize::try_from(capacity).context("negative OreFeature visited-bit capacity")?;
    let mut visited = vec![0_u64; capacity.div_ceil(64)];
    let size = usize::try_from(config.size).context("negative OreFeature sphere-array length")?;
    let mut spheres = Vec::with_capacity(size);
    // Initialize every sphere and consume every size-scaled double draw before
    // culling, section acquisition, target checks or exposure random draws.
    for i in 0..size {
        let fraction = i as f32 / config.size as f32;
        let center = std::array::from_fn(|axis| {
            start[axis] + f64::from(fraction) * (end[axis] - start[axis])
        });
        let radius_draw = random.next_f64() * f64::from(config.size) / 16.0;
        let radius = (f64::from(vanilla_sin(PI * fraction) + 1.0_f32) * radius_draw + 1.0) / 2.0;
        spheres.push(Sphere { center, radius });
    }
    for i in 0..size.saturating_sub(1) {
        if spheres[i].radius <= 0.0 {
            continue;
        }
        for j in i + 1..size {
            if spheres[j].radius <= 0.0 {
                continue;
            }
            let dx = spheres[i].center[0] - spheres[j].center[0];
            let dy = spheres[i].center[1] - spheres[j].center[1];
            let dz = spheres[i].center[2] - spheres[j].center[2];
            let dr = spheres[i].radius - spheres[j].radius;
            if dr * dr > dx * dx + dy * dy + dz * dz {
                if dr > 0.0 {
                    spheres[j].radius = -1.0;
                } else {
                    spheres[i].radius = -1.0;
                }
                // Source continues the inner loop even after marking i dead.
            }
        }
    }
    // try-with-resources begins here in the source, even for zero spheres.
    // The host owns the real per-invocation section cache and acquisitions.
    host.begin_bulk_section_access()?;
    let result = place_spheres(
        host,
        config,
        random,
        &spheres,
        &mut visited,
        minimum,
        horizontal_span,
        vertical_span,
    );
    let close = host.close_bulk_section_access();
    match (result, close) {
        (Ok(placed), Ok(())) => Ok(placed),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(close_error)) => Err(error.context(format!(
            "OreFeature bulk-section close also failed (suppressed): {close_error:#}"
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
fn place_spheres(
    host: &mut impl OreHost,
    config: &OreConfiguration,
    random: &mut impl RandomSource,
    spheres: &[Sphere],
    visited: &mut Vec<u64>,
    minimum: DensityPoint,
    horizontal_span: i32,
    vertical_span: i32,
) -> Result<bool> {
    let mut writes = 0_i32;
    for sphere in spheres {
        if sphere.radius < 0.0 {
            continue;
        }
        let [cx, cy, cz] = sphere.center;
        let radius = sphere.radius;
        let min_x = java_floor(cx - radius).max(minimum.x);
        let min_y = java_floor(cy - radius).max(minimum.y);
        let min_z = java_floor(cz - radius).max(minimum.z);
        // These are Math.max, including the upper bounds, in the original.
        let max_x = java_floor(cx + radius).max(min_x);
        let max_y = java_floor(cy + radius).max(min_y);
        let max_z = java_floor(cz + radius).max(min_z);
        for x in min_x..=max_x {
            let nx = (f64::from(x) + 0.5 - cx) / radius;
            if !(nx * nx < 1.0) {
                continue;
            }
            for y in min_y..=max_y {
                let ny = (f64::from(y) + 0.5 - cy) / radius;
                if !(nx * nx + ny * ny < 1.0) {
                    continue;
                }
                for z in min_z..=max_z {
                    let nz = (f64::from(z) + 0.5 - cz) / radius;
                    if !(nx * nx + ny * ny + nz * nz < 1.0) || host.outside_build_height(y) {
                        continue;
                    }
                    let index = x
                        .wrapping_sub(minimum.x)
                        .wrapping_add(y.wrapping_sub(minimum.y).wrapping_mul(horizontal_span))
                        .wrapping_add(
                            z.wrapping_sub(minimum.z)
                                .wrapping_mul(horizontal_span)
                                .wrapping_mul(vertical_span),
                        );
                    if !visit(visited, index)? {
                        continue;
                    }
                    let point = DensityPoint::new(x, y, z);
                    if !host.ensure_can_write(point)? {
                        continue;
                    }
                    let Some(state) = host.bulk_section_block(point)? else {
                        continue;
                    };
                    for target in &config.targets {
                        if can_place_ore(
                            state,
                            &mut |point| host.bulk_block(point),
                            random,
                            config,
                            target,
                            point,
                        )? {
                            host.direct_section_set(point, target.state)?;
                            writes = writes.wrapping_add(1);
                            break;
                        }
                    }
                }
            }
        }
    }
    Ok(writes > 0)
}

/// BitSet.get followed by set, before write permission and section lookup.
fn visit(visited: &mut Vec<u64>, index: i32) -> Result<bool> {
    let index = usize::try_from(index).context("negative OreFeature visited-bit index")?;
    let word = index / 64;
    let bit = 1_u64 << (index & 63);
    if word >= visited.len() {
        // java.util.BitSet's constructor size is an initial capacity, not a cap.
        visited.resize(word + 1, 0);
    }
    let unseen = visited[word] & bit == 0;
    visited[word] |= bit;
    Ok(unseen)
}

fn java_floor(value: f64) -> i32 {
    let narrowed = value as i32;
    if value < f64::from(narrowed) {
        narrowed.wrapping_sub(1)
    } else {
        narrowed
    }
}

fn java_ceil_f32(value: f32) -> i32 {
    let narrowed = value as i32;
    if value > narrowed as f32 {
        narrowed.wrapping_add(1)
    } else {
        narrowed
    }
}
