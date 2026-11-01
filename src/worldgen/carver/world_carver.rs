//! One source WorldCarver ellipsoid/material kernel shared by cave and canyon.
//! RNG sampling belongs to the caller's actual configured-carver geometry.
use super::{
    config::BaseCarverConfig, host::CarverHost, mask::CarvingMask, providers::HeightContext,
};
use crate::{
    density::DensityPoint,
    inventory::{block_state_name, block_state_property, block_state_with_properties},
    worldgen::generator::resolve_block_state,
};
use anyhow::{anyhow, Result};
use std::{collections::BTreeMap, sync::OnceLock};

pub const GENERATION_RANGE: i32 = 4;

/// NetherWorldCarver has a distinct source carveBlock override. It shares the
/// ellipsoid traversal, not the normal aquifer/debug/ground-repair callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldCarverKind {
    Overworld,
    Nether,
}

/// WorldCarver.canReach: preserve the original float radius additions before
/// widening and the double square/subtraction association.
pub fn can_reach(chunk: (i32, i32), center: [f64; 3], step: i32, end: i32, thickness: f32) -> bool {
    let middle_x = f64::from(chunk.0.wrapping_shl(4).wrapping_add(8));
    let middle_z = f64::from(chunk.1.wrapping_shl(4).wrapping_add(8));
    let x = center[0] - middle_x;
    let z = center[2] - middle_z;
    let remaining = f64::from(end.wrapping_sub(step));
    let radius = f64::from((thickness + 2.0_f32) + 16.0_f32);
    x * x + z * z - remaining * remaining <= radius * radius
}

/// WorldCarver.carveEllipsoid in original X/Z/descending-Y order. `skip` is the
/// real geometry's checker over normalized X/Y/Z and absolute block Y. Mask
/// writes occur before material replacement and are not rolled back on false.
/// Errors propagate after the source's already-performed mutations; callers
/// must consume or poison their in-progress generation job.
pub fn carve_ellipsoid(
    host: &mut impl CarverHost,
    config: &BaseCarverConfig,
    mask: &mut CarvingMask,
    kind: WorldCarverKind,
    center: [f64; 3],
    horizontal_radius: f64,
    vertical_radius: f64,
    mut skip: impl FnMut(f64, f64, f64, i32) -> bool,
) -> Result<bool> {
    let chunk = host.chunk_pos();
    let middle_x = f64::from(chunk.0.wrapping_shl(4).wrapping_add(8));
    let middle_z = f64::from(chunk.1.wrapping_shl(4).wrapping_add(8));
    let range = 16.0 + horizontal_radius * 2.0;
    if (center[0] - middle_x).abs() > range || (center[2] - middle_z).abs() > range {
        return Ok(false);
    }
    let origin_x = chunk.0.wrapping_shl(4);
    let origin_z = chunk.1.wrapping_shl(4);
    let min_x = java_floor(center[0] - horizontal_radius)
        .wrapping_sub(origin_x)
        .wrapping_sub(1)
        .max(0);
    let max_x = java_floor(center[0] + horizontal_radius)
        .wrapping_sub(origin_x)
        .min(15);
    let min_y = java_floor(center[1] - vertical_radius)
        .wrapping_sub(1)
        .max(host.min_gen_y().wrapping_add(1));
    let ceiling_margin = if host.is_upgrading() { 0 } else { 7 };
    let max_y = java_floor(center[1] + vertical_radius).wrapping_add(1).min(
        host.min_gen_y()
            .wrapping_add(host.gen_depth())
            .wrapping_sub(1)
            .wrapping_sub(ceiling_margin),
    );
    let min_z = java_floor(center[2] - horizontal_radius)
        .wrapping_sub(origin_z)
        .wrapping_sub(1)
        .max(0);
    let max_z = java_floor(center[2] + horizontal_radius)
        .wrapping_sub(origin_z)
        .min(15);
    let mut carved = false;
    for local_x in min_x..=max_x {
        let x = origin_x.wrapping_add(local_x);
        let normalized_x = (f64::from(x) + 0.5 - center[0]) / horizontal_radius;
        for local_z in min_z..=max_z {
            let z = origin_z.wrapping_add(local_z);
            let normalized_z = (f64::from(z) + 0.5 - center[2]) / horizontal_radius;
            if normalized_x * normalized_x + normalized_z * normalized_z >= 1.0 {
                continue;
            }
            let mut found_surface = false;
            // The source excludes the lower clipped Y rather than visiting it.
            let mut y = max_y;
            while y > min_y {
                let normalized_y = (f64::from(y) - 0.5 - center[1]) / vertical_radius;
                if !skip(normalized_x, normalized_y, normalized_z, y)
                    && (!mask.get(local_x, y, local_z)? || config.debug.debug_mode)
                {
                    mask.set(local_x, y, local_z)?;
                    // Boolean OR must not short circuit: every selected source
                    // position receives its material callback after the mask.
                    carved |= carve_block(
                        host,
                        config,
                        kind,
                        DensityPoint::new(x, y, z),
                        &mut found_surface,
                    )?;
                }
                y = y.wrapping_sub(1);
            }
        }
    }
    Ok(carved)
}

/// Source material callback. The enclosing ellipsoid already performed its
/// geometry/mask gate and set the bit. `found_surface` belongs to one actual
/// descending ellipsoid column and persists across failed replacement attempts.
pub fn carve_block(
    host: &mut impl CarverHost,
    config: &BaseCarverConfig,
    kind: WorldCarverKind,
    point: DensityPoint,
    found_surface: &mut bool,
) -> Result<bool> {
    let state = host.block(point)?;
    if kind == WorldCarverKind::Nether {
        // Nether's override has no debug bypass, surface tracking, aquifer
        // query or postprocessing callback, even for its direct lava writes.
        if !config.replaceable.contains(state)? {
            return Ok(false);
        }
        let palette = palette()?;
        let replacement = if point.y <= host.min_gen_y().wrapping_add(31) {
            palette.lava
        } else {
            palette.cave_air
        };
        host.set_block(point, replacement)?;
        return Ok(true);
    }
    let name = state_name(state)?;
    if name == "grass_block" || name == "mycelium" {
        *found_surface = true;
    }
    if !config.replaceable.contains(state)? && !config.debug.debug_mode {
        return Ok(false);
    }
    let Some(replacement) = carve_state(host, config, point)? else {
        return Ok(false);
    };
    host.set_block(point, replacement)?;
    if host.aquifer_should_schedule_fluid_update() && host.has_fluid(replacement)? {
        host.mark_postprocessing(point)?;
    }
    if *found_surface {
        let below = DensityPoint::new(point.x, point.y.wrapping_sub(1), point.z);
        if state_name(host.block(below)?)? == "dirt" {
            let has_fluid_above = host.has_fluid(replacement)?;
            if let Some(material) = host.top_material(below, has_fluid_above)? {
                host.set_block(below, material)?;
                if host.has_fluid(material)? {
                    host.mark_postprocessing(below)?;
                }
            }
        }
    }
    Ok(true)
}

fn carve_state(
    host: &mut impl CarverHost,
    config: &BaseCarverConfig,
    point: DensityPoint,
) -> Result<Option<u32>> {
    let lava_level = config.lava_level.resolve(HeightContext {
        min_gen_y: host.min_gen_y(),
        gen_depth: host.gen_depth(),
    });
    if point.y <= lava_level {
        // This source branch does NOT call the aquifer or clear its retained
        // update flag, and it precedes debug-state mapping.
        return Ok(Some(palette()?.lava));
    }
    let state = host.aquifer_substance(point)?;
    match state {
        None if config.debug.debug_mode => Ok(Some(config.debug.barrier_state)),
        None => Ok(None),
        Some(state) if config.debug.debug_mode => Ok(Some(debug_state(config, state)?)),
        Some(state) => Ok(Some(state)),
    }
}

fn debug_state(config: &BaseCarverConfig, state: u32) -> Result<u32> {
    match state_name(state)? {
        "air" => Ok(config.debug.air_state),
        "water" => {
            let water = config.debug.water_state;
            state_name(water)?;
            if block_state_property(water, "waterlogged").is_some() {
                block_state_with_properties(
                    water,
                    &BTreeMap::from([("waterlogged".to_owned(), "true".to_owned())]),
                )
            } else {
                Ok(water)
            }
        }
        "lava" => Ok(config.debug.lava_state),
        _ => Ok(state),
    }
}
fn state_name(state: u32) -> Result<&'static str> {
    block_state_name(state)
        .ok_or_else(|| anyhow!("carver received unknown canonical block state {state}"))
}
struct Palette {
    lava: u32,
    cave_air: u32,
}
fn palette() -> Result<&'static Palette> {
    static PALETTE: OnceLock<std::result::Result<Palette, String>> = OnceLock::new();
    PALETTE
        .get_or_init(|| {
            let state = |name: &str| {
                resolve_block_state(&serde_json::json!({"Name":format!("minecraft:{name}")}))
                    .map_err(|error| error.to_string())
            };
            Ok(Palette {
                lava: state("lava")?,
                cave_air: state("cave_air")?,
            })
        })
        .as_ref()
        .map_err(|error| anyhow!("invalid canonical carver palette: {error}"))
}
fn java_floor(value: f64) -> i32 {
    let integer = value as i32;
    if value < f64::from(integer) {
        integer.wrapping_sub(1)
    } else {
        integer
    }
}
