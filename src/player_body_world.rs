//! Player-specific virtual Entity.baseTick behavior over the real move world.
use crate::{
    entity_base::{
        self, BaseEffect, BoatFluidState, EntityBaseWorld, EyeFluidSample, FluidTag, FluidUpdate,
    },
    entity_state::{java_floor, EntityPhysicalState},
    fluid_physics::{self, FluidBody, FluidKind, FluidWorld, RuntimeFluidWorld},
    geometry::{Aabb, Vec3},
    main_player_motion::{PlayerMotionHooks, PlayerMoveWorld},
};
use anyhow::{bail, ensure, Result};
pub trait PlayerBodyWorldHooks: PlayerMotionHooks {
    fn body_minimum_build_height(&self) -> i32;
    fn body_ultrawarm(&self) -> bool;
    fn body_next_float(&mut self) -> Result<f32>;
    fn body_next_double(&mut self) -> Result<f64>;
    fn body_sound(
        &mut self,
        state: &EntityPhysicalState,
        key: &str,
        volume: f32,
        pitch: f32,
    ) -> Result<()>;
    fn body_sprint_particle(
        &mut self,
        world: &mut crate::runtime_world::RuntimeWorld,
        state: &EntityPhysicalState,
    ) -> Result<()>;
    fn body_base_effect(&mut self, state: &EntityPhysicalState, effect: BaseEffect) -> Result<()>;
    fn body_movement_speed(&self) -> Result<f32>;
    fn body_flying_speed(&self, sprinting: bool, flying: bool) -> Result<f32>;
}
impl<H: PlayerBodyWorldHooks> PlayerMoveWorld<'_, H> {
    pub fn body_collision_free(&mut self, s: &EntityPhysicalState, b: Aabb) -> Result<bool> {
        self.no_collision_at(s, b)
    }
    pub fn body_no_liquid(&mut self, b: Aabb) -> Result<bool> {
        for x in java_floor(b.min_x)..(b.max_x.ceil() as i32) {
            for y in java_floor(b.min_y)..(b.max_y.ceil() as i32) {
                for z in java_floor(b.min_z)..(b.max_z.ceil() as i32) {
                    if !fluid_physics::FluidState::from_block(self.world.block(x, y, z)?)?
                        .is_empty()
                    {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }
    pub fn body_on_climbable(&mut self, s: &EntityPhysicalState) -> Result<bool> {
        let b = self
            .world
            .block(s.block_position.0, s.block_position.1, s.block_position.2)?;
        ensure!(
            !crate::entity_move::block_facts(b)?.climbable_or_powder_snow(),
            "unported player climbable/powder-snow travel"
        );
        Ok(false)
    }
}
impl<H: PlayerBodyWorldHooks> EntityBaseWorld for PlayerMoveWorld<'_, H> {
    fn reset_base_fall_distance(&mut self, s: &mut EntityPhysicalState) -> Result<()> {
        self.host.fall_distance_reset(s)?;
        s.fall_distance = 0.;
        Ok(())
    }
    fn is_client_side(&self) -> bool {
        false
    }
    fn is_server_level(&self) -> bool {
        true
    }
    fn minimum_build_height(&self) -> i32 {
        self.host.body_minimum_build_height()
    }
    fn ultrawarm(&self) -> bool {
        self.host.body_ultrawarm()
    }
    fn set_remaining_fire_ticks(&mut self, s: &mut EntityPhysicalState, t: i32) {
        s.remaining_fire_ticks = t.min(1);
    }
    fn on_below_world(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        bail!("LivingEntity out-of-world damage bypasses creative invulnerability and needs the real hurt/death owner")
    }
    fn vehicle_removed(&mut self, v: i32) -> Result<bool> {
        bail!("player vehicle {v} lifecycle unavailable")
    }
    fn process_portal(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        bail!("player portal travel owner unavailable")
    }
    fn spawn_sprint_particle(&mut self, s: &mut EntityPhysicalState) -> Result<()> {
        self.host.body_sprint_particle(self.world, s)
    }
    fn boat_fluid_state(&mut self, v: i32) -> Result<Option<BoatFluidState>> {
        bail!("player boat {v} fluid owner unavailable")
    }
    fn update_swimming(&mut self, s: &mut EntityPhysicalState) -> Result<()> {
        if self.facts.flying {
            s.set_swimming(false);
            Ok(())
        } else {
            entity_base::update_swimming(s, self)
        }
    }
    fn update_fluid(
        &mut self,
        s: &EntityPhysicalState,
        t: FluidTag,
        push: f64,
    ) -> Result<FluidUpdate> {
        let r = fluid_physics::update_fluid_pushing(
            &mut RuntimeFluidWorld { world: self.world },
            FluidBody {
                bounds: s.bounds,
                velocity: s.velocity,
                pushed_by_fluid: !self.facts.flying,
                is_player: true,
            },
            match t {
                FluidTag::Water => fluid_physics::FluidTag::Water,
                FluidTag::Lava => fluid_physics::FluidTag::Lava,
            },
            push,
        )?;
        Ok(FluidUpdate {
            touching: r.touching,
            velocity: r.velocity,
            height_written: r.height_written,
        })
    }
    fn water_splash(&mut self, s: &mut EntityPhysicalState) -> Result<()> {
        let v = s.velocity;
        let raw = ((v.x * v.x * f64::from(0.2f32) + v.y * v.y + v.z * v.z * f64::from(0.2f32))
            .sqrt() as f32)
            * 0.2f32;
        let volume = raw.min(1.);
        let pitch = 1f32 + (self.host.body_next_float()? - self.host.body_next_float()?) * 0.4f32;
        self.host
            .body_sound(s, if volume < 0.25 { "us" } else { "ut" }, volume, pitch)?;
        let bound = 1f32 + s.width * 20f32;
        let mut i = 0;
        while (i as f32) < bound {
            self.host.body_next_double()?;
            self.host.body_next_double()?;
            self.host.body_next_double()?;
            i += 1;
        }
        i = 0;
        while (i as f32) < bound {
            self.host.body_next_double()?;
            self.host.body_next_double()?;
            i += 1;
        }
        Ok(())
    }
    fn eye_fluid_sample(&mut self, x: f64, y: f64, z: f64) -> Result<EyeFluidSample> {
        let p = (java_floor(x), java_floor(y), java_floor(z));
        let mut w = RuntimeFluidWorld { world: self.world };
        let f = w.fluid(p)?;
        let surface = f64::from(p.1 as f32 + fluid_physics::fluid_height(&mut w, p, f)?);
        Ok(EyeFluidSample {
            surface,
            tags: match f.kind {
                FluidKind::Empty => 0,
                FluidKind::Water => 1,
                FluidKind::Lava => 2,
            },
        })
    }
    fn block_fluid_tags(&mut self, p: (i32, i32, i32)) -> Result<u8> {
        Ok(
            match FluidStateExt::kind(self.world.block(p.0, p.1, p.2)?)? {
                FluidKind::Empty => 0,
                FluidKind::Water => 1,
                FluidKind::Lava => 2,
            },
        )
    }
    fn next_entity_float(&mut self, _: &EntityPhysicalState) -> Result<f32> {
        self.host.body_next_float()
    }
    fn effect(&mut self, s: &mut EntityPhysicalState, e: BaseEffect) -> Result<()> {
        self.host.body_base_effect(s, e)
    }
    fn tick_leash(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        bail!("player cannot acquire a leashable body")
    }
}
struct FluidStateExt;
impl FluidStateExt {
    fn kind(s: u32) -> Result<FluidKind> {
        Ok(fluid_physics::FluidState::from_block(s)?.kind)
    }
}
