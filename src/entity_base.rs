//! Entity.baseTick ordering and water/lava state transitions for Java 1.21.
//!
//! World queries and effects are explicit. An unavailable portal, fluid,
//! vehicle, particle or damage provider must return an error rather than
//! supplying an empty environmental effect for an unsupported world context.
use crate::{
    entity_state::EntityPhysicalState,
    geometry::{Aabb, Vec3},
};
use anyhow::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FluidTag {
    Water,
    Lava,
}
impl FluidTag {
    pub fn index(self) -> usize {
        match self {
            Self::Water => 0,
            Self::Lava => 1,
        }
    }
    pub fn mask(self) -> u8 {
        1 << self.index()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FluidUpdate {
    pub touching: bool,
    pub velocity: Vec3,
    /// None is the original unloaded-chunk early return: do not overwrite the
    /// existing depth entry. The combined water/lava operation clears it first.
    pub height_written: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct BoatFluidState {
    pub under_water: bool,
    pub bounds: Aabb,
}

#[derive(Clone, Copy, Debug)]
pub struct EyeFluidSample {
    pub surface: f64,
    pub tags: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseDamage {
    OnFire,
    Lava,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BaseEffect {
    StopRiding {
        vehicle: i32,
    },
    MeltFrozen {
        position: (i32, i32, i32),
        event: i32,
        data: i32,
    },
    LavaBurnSound {
        volume: f32,
        pitch: f32,
    },
    DiscardBelowWorld,
}

pub trait EntityBaseWorld {
    fn is_client_side(&self) -> bool;
    fn is_server_level(&self) -> bool;
    fn minimum_build_height(&self) -> i32;
    fn ultrawarm(&self) -> bool;
    fn reset_base_fall_distance(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
        state.fall_distance = 0.;
        Ok(())
    }
    fn set_remaining_fire_ticks(&mut self, state: &mut EntityPhysicalState, ticks: i32) {
        state.remaining_fire_ticks = ticks;
    }
    fn on_below_world(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
        state.removed = true;
        self.effect(state, BaseEffect::DiscardBelowWorld)
    }
    fn vehicle_removed(&mut self, vehicle: i32) -> Result<bool>;
    /// Entity.handlePortal is virtual. The default is the generic server-level
    /// cooldown/processor dispatch; specialised entities can supply their
    /// original override through this hook.
    fn tick_portal(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
        if self.is_server_level() {
            if state.portal_cooldown > 0 {
                state.portal_cooldown -= 1;
            }
            if state.portal_process.is_some() {
                self.process_portal(state)?;
            }
        }
        Ok(())
    }
    fn can_spawn_sprint_particle(&mut self, state: &EntityPhysicalState) -> Result<bool> {
        Ok(state.is_sprinting()
            && !state.was_touching_water
            && !state.is_spectator
            && state.pose != 5
            && !state.is_in_lava()
            && !state.removed)
    }
    fn update_swimming(&mut self, state: &mut EntityPhysicalState) -> Result<()>
    where
        Self: Sized,
    {
        update_swimming(state, self)
    }
    /// Active portal processing after the generic cooldown decrement. This is
    /// called only when a portal processor exists in a server-level context.
    fn process_portal(&mut self, state: &mut EntityPhysicalState) -> Result<()>;
    fn spawn_sprint_particle(&mut self, state: &mut EntityPhysicalState) -> Result<()>;
    fn boat_fluid_state(&mut self, vehicle: i32) -> Result<Option<BoatFluidState>>;
    fn update_fluid(
        &mut self,
        state: &EntityPhysicalState,
        tag: FluidTag,
        push: f64,
    ) -> Result<FluidUpdate>;
    fn water_splash(&mut self, state: &mut EntityPhysicalState) -> Result<()>;
    fn eye_fluid_sample(&mut self, x: f64, y: f64, z: f64) -> Result<EyeFluidSample>;
    fn block_fluid_tags(&mut self, position: (i32, i32, i32)) -> Result<u8>;
    /// The host owns this entity's actual RNG. No default seed is manufactured.
    fn next_entity_float(&mut self, state: &EntityPhysicalState) -> Result<f32>;
    fn effect(&mut self, state: &mut EntityPhysicalState, effect: BaseEffect) -> Result<()>;
    fn tick_leash(&mut self, state: &mut EntityPhysicalState) -> Result<()>;
}

pub fn apply_fluid_update(state: &mut EntityPhysicalState, tag: FluidTag, update: FluidUpdate) {
    state.velocity = update.velocity;
    if let Some(height) = update.height_written {
        state.fluid_heights[tag.index()] = height;
    }
}

/// Entity.updateInWaterStateAndDoFluidPushing, called both by baseTick and near
/// the end of ItemEntity.tick. In particular, these two updates must not be
/// coalesced into one sample of the item's starting or ending fluid cell.
pub fn update_water_and_lava(
    state: &mut EntityPhysicalState,
    world: &mut impl EntityBaseWorld,
) -> Result<bool> {
    state.fluid_heights = [0.0; 2];
    let boat = match state.vehicle {
        Some(vehicle) => world.boat_fluid_state(vehicle)?,
        None => None,
    };
    if boat.is_some_and(|boat| !boat.under_water) {
        state.was_touching_water = false;
    } else {
        let update = world.update_fluid(state, FluidTag::Water, 0.014)?;
        let touching = update.touching;
        apply_fluid_update(state, FluidTag::Water, update);
        if touching {
            if !state.was_touching_water && !state.first_tick {
                world.water_splash(state)?;
            }
            world.reset_base_fall_distance(state)?;
            state.was_touching_water = true;
            state.remaining_fire_ticks = 0;
        } else {
            state.was_touching_water = false;
        }
    }
    let push = if world.ultrawarm() {
        0.007
    } else {
        0.0023333333333333335
    };
    let update = world.update_fluid(state, FluidTag::Lava, push)?;
    let lava = update.touching;
    apply_fluid_update(state, FluidTag::Lava, update);
    Ok(state.was_touching_water || lava)
}

pub fn update_fluid_on_eyes(
    state: &mut EntityPhysicalState,
    world: &mut impl EntityBaseWorld,
) -> Result<()> {
    state.was_eye_in_water = state.fluid_on_eyes & FluidTag::Water.mask() != 0;
    state.fluid_on_eyes = 0;
    let eye = state.eye_y();
    if let Some(vehicle) = state.vehicle {
        if world.boat_fluid_state(vehicle)?.is_some_and(|boat| {
            !boat.under_water && boat.bounds.max_y >= eye && boat.bounds.min_y <= eye
        }) {
            return Ok(());
        }
    }
    let sample = world.eye_fluid_sample(state.position.x, eye, state.position.z)?;
    if sample.surface > eye {
        state.fluid_on_eyes = sample.tags;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tools/entity_base_tests.rs"]
mod tests;

pub fn update_swimming(
    state: &mut EntityPhysicalState,
    world: &mut impl EntityBaseWorld,
) -> Result<()> {
    let swimming = if state.is_swimming() {
        state.is_sprinting() && state.was_touching_water && state.vehicle.is_none()
    } else {
        state.is_sprinting()
            && state.was_eye_in_water
            && state.was_touching_water
            && state.vehicle.is_none()
            && world.block_fluid_tags(state.block_position)? & FluidTag::Water.mask() != 0
    };
    state.set_swimming(swimming);
    Ok(())
}

/// Run the original generic base tick; the level owns tickCount advancement.
/// Damage is supplied by the entity type, synchronously at the original call
/// site, so health/removal/impulse effects remain visible to following phases.
pub fn base_tick<W: EntityBaseWorld>(
    state: &mut EntityPhysicalState,
    world: &mut W,
    mut hurt: impl FnMut(&mut EntityPhysicalState, &mut W, BaseDamage, f32) -> Result<bool>,
) -> Result<()> {
    state.cached_inside_block_state = None;
    if let Some(vehicle) = state.vehicle {
        if world.vehicle_removed(vehicle)? {
            state.vehicle = None;
            world.effect(state, BaseEffect::StopRiding { vehicle })?;
        }
    }
    if state.boarding_cooldown > 0 {
        state.boarding_cooldown -= 1;
    }
    state.walk_dist_old = state.walk_dist;
    state.old_pitch = state.position.pitch;
    state.old_yaw = state.position.yaw;
    world.tick_portal(state)?;
    if world.can_spawn_sprint_particle(state)? {
        world.spawn_sprint_particle(state)?;
    }
    state.was_in_powder_snow = state.is_in_powder_snow;
    state.is_in_powder_snow = false;
    update_water_and_lava(state, world)?;
    update_fluid_on_eyes(state, world)?;
    world.update_swimming(state)?;
    if world.is_client_side() {
        state.remaining_fire_ticks = 0;
    } else if state.remaining_fire_ticks > 0 {
        if state.fire_immune {
            state.remaining_fire_ticks = state.remaining_fire_ticks.wrapping_sub(4);
            if state.remaining_fire_ticks < 0 {
                state.remaining_fire_ticks = 0;
            }
        } else {
            if state.remaining_fire_ticks % 20 == 0 && !state.is_in_lava() {
                hurt(state, world, BaseDamage::OnFire, 1.0)?;
            }
            state.remaining_fire_ticks = state.remaining_fire_ticks.wrapping_sub(1);
        }
        if state.ticks_frozen > 0 {
            state.set_ticks_frozen(0);
            world.effect(
                state,
                BaseEffect::MeltFrozen {
                    position: state.block_position,
                    event: 1009,
                    data: 1,
                },
            )?;
        }
    }
    if state.is_in_lava() {
        if !state.fire_immune {
            // igniteForSeconds(15) raises the timer only when it is lower.
            if state.remaining_fire_ticks < 300 {
                world.set_remaining_fire_ticks(state, 300);
            }
            if hurt(state, world, BaseDamage::Lava, 4.0)? {
                let pitch = 2.0 + world.next_entity_float(state)? * 0.4;
                if !state.silent {
                    world.effect(state, BaseEffect::LavaBurnSound { volume: 0.4, pitch })?;
                }
            }
        }
        state.fall_distance *= 0.5;
    }
    if state.position.y < f64::from(world.minimum_build_height().wrapping_sub(64)) {
        world.on_below_world(state)?;
    }
    if !world.is_client_side() {
        state.set_shared_flag(0, state.remaining_fire_ticks > 0 || state.visual_fire);
    }
    state.first_tick = false;
    if !world.is_client_side() && state.leashable {
        world.tick_leash(state)?;
    }
    Ok(())
}
