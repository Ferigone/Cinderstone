//! Bounded inherited Entity fluid/swimming bodies on the actual Bat physical.
//!
//! This is not Entity.baseTick or LivingEntity.baseTick. The actual caller must
//! still run their profiler, portal, splash, damage, fire, void, effects, air and
//! leash phases in source order. These functions own no actor/RNG/world/cache.

use super::retained_move::BatPhysicalError;
use crate::{
    entity_base::FluidTag,
    entity_state::java_floor,
    fluid_physics::{self, FluidBody, FluidTag as QueryFluidTag, FluidWorld, RuntimeFluidWorld},
    mobs::actor::{NativeBatLease, NativeBatTypes},
    runtime_world::RuntimeWorld,
};

fn query_tag(tag: FluidTag) -> QueryFluidTag {
    match tag {
        FluidTag::Water => QueryFluidTag::Water,
        FluidTag::Lava => QueryFluidTag::Lava,
    }
}

/// Actual inherited Bat selection of Entity.updateFluidHeightAndDoFluidPushing
/// (Java3385–3450). Bat is a nonplayer and inherits isPushedByFluid=true. The
/// reused native solver preserves original fluid-loop/order and normalization.
/// It reads only the actual RuntimeWorld's native block/fluid data, and cannot
/// call back into an actor while the immutable numeric query locals exist.
///
/// The caller supplies the source water/lava factor at its actual callsite.
/// This helper changes only that tag's depth and inherited delta movement; it
/// does NOT clear the whole depth map, set wasTouchingWater, emit a splash or
/// clear fire. None depth on an unavailable-chunk early return preserves the
/// existing tag entry. No actor borrow spans any world operation.
pub fn update_fluid_height_and_do_fluid_pushing<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    world: &mut RuntimeWorld,
    tag: FluidTag,
    factor: f64,
) -> Result<bool, BatPhysicalError> {
    let body = actor.with_physical(|physical| FluidBody {
        bounds: physical.bounds,
        velocity: physical.velocity,
        pushed_by_fluid: true,
        is_player: false,
    })?;
    let update = fluid_physics::update_fluid_pushing(
        &mut RuntimeFluidWorld { world },
        body,
        query_tag(tag),
        factor,
    )?;
    if update.height_written.is_none() {
        return Ok(update.touching);
    }
    actor.with_physical(|physical| {
        // Bat inherits Entity.setDeltaMovement(Vec3), whose body is exactly the
        // canonical field store. This is not Player's movement setter.
        physical.velocity = update.velocity;
        if let Some(height) = update.height_written {
            physical.fluid_heights[tag.index()] = height;
        }
    })?;
    Ok(update.touching)
}

/// Entity's private updateFluidOnEyes1461–1478 for an actually UNMOUNTED Bat in
/// the native vanilla water/lava-tag domain. A mounted actor requires the real
/// retained vehicle/Boat owner and is rejected before executing this body.
/// `was_eye_in_water` describes the PREVIOUS eye-tag set, as in Java.
pub fn update_fluid_on_eyes_unmounted<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    world: &mut RuntimeWorld,
) -> Result<(), BatPhysicalError> {
    if actor.with_physical(|p| p.vehicle.is_some())? {
        return Err(BatPhysicalError::Unbound(
            "retained vehicle/Boat eye-fluid owner",
        ));
    }
    actor.with_physical(|physical| {
        physical.was_eye_in_water = physical.fluid_on_eyes & FluidTag::Water.mask() != 0;
        physical.fluid_on_eyes = 0;
    })?;
    let eye_y = actor.with_physical(|p| p.eye_y())?;
    let at = actor.with_physical(|p| {
        (
            java_floor(p.position.x),
            java_floor(eye_y),
            java_floor(p.position.z),
        )
    })?;
    let mut fluids = RuntimeFluidWorld { world };
    let fluid = fluids.fluid(at)?;
    // Source adds two floats BEFORE widening the surface to double.
    let surface = f64::from(at.1 as f32 + fluid_physics::fluid_height(&mut fluids, at, fluid)?);
    // Original dcmpl/ifle skips the tag branch for NaN.
    if surface > eye_y {
        let mut tags = 0;
        if fluids.matches_tag(at, fluid, QueryFluidTag::Water)? {
            tags |= FluidTag::Water.mask();
        }
        if fluids.matches_tag(at, fluid, QueryFluidTag::Lava)? {
            tags |= FluidTag::Lava.mask();
        }
        actor.with_physical(|p| p.fluid_on_eyes = tags)?;
    }
    Ok(())
}

/// Entity.updateSwimming1427–1432, inherited by Bat, with short-circuit reads
/// of the actual sprint/water/vehicle fields and real current block fluid.
/// setSwimming mutates bit4 of the same metadata0 shared flags/dirty owner.
/// The concrete RuntimeFluidWorld supports the native vanilla fluid-tag domain;
/// replaced data-pack tags require their actual owner before using this body.
pub fn update_swimming<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    world: &mut RuntimeWorld,
) -> Result<(), BatPhysicalError> {
    let swimming = actor.with_physical(|p| p.is_swimming())?;
    let value = if !actor.with_physical(|p| p.is_sprinting())? {
        false
    } else if swimming {
        actor.with_physical(|p| p.was_touching_water)?
            && actor.with_physical(|p| p.vehicle.is_none())?
    } else if actor.with_physical(|p| p.was_eye_in_water)?
        && actor.with_physical(|p| p.was_touching_water)?
        && actor.with_physical(|p| p.vehicle.is_none())?
    {
        let at = actor.with_physical(|p| p.block_position)?;
        let mut fluids = RuntimeFluidWorld { world };
        let state = fluids.fluid(at)?;
        fluids.matches_tag(at, state, QueryFluidTag::Water)?
    } else {
        false
    };
    actor.with_physical(|p| p.set_swimming(value))?;
    Ok(())
}

/// Bat inherits Entity.setRemainingFireTicks621–622 without Player's abilities
/// clamp. Keep this selected inherited body distinct from the source caller's
/// burning/ignite/clearFire dispatch and from any complete damage phase.
pub fn set_remaining_fire_ticks<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
    ticks: i32,
) -> Result<(), BatPhysicalError> {
    actor.with_physical(|p| p.remaining_fire_ticks = ticks)?;
    Ok(())
}

/// Entity.resetFallDistance2673–2674, inherited by this actual Bat. Bat's empty
/// checkFallDamage override is separate and must not replace this setter.
pub fn reset_fall_distance<T: NativeBatTypes>(
    actor: &NativeBatLease<T>,
) -> Result<(), BatPhysicalError> {
    actor.with_physical(|p| p.fall_distance = 0.)?;
    Ok(())
}
