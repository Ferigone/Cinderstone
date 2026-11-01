//! Concrete ServerPlayer Entity.move world, with owned dry-world behavior.
//!
//! Block/entity/border geometry, supporting blocks and crouch-edge reduction
//! query the real backend. Sound/game-event/criterion delivery stays required;
//! unknown wet, fire and special-block behavior returns an explicit error.
use crate::{
    entity_move::{self, BlockPos, EntityMoveWorld, MovementEmission, MoverType, ProfileEvent},
    entity_state::{java_floor, EntityPhysicalState},
    entity_support,
    fluid_physics::{self, FluidKind},
    geometry::{self, Aabb, EntityCollisionContext, Vec3, VoxelShape},
    player_physics::{self, CollisionQuery, WorldCollision},
    runtime_collision::{CollisionActor, CollisionBorder, CollisionEntity, RuntimeCollision},
    runtime_world::RuntimeWorld,
    world::{MAX_Y, MIN_Y},
};
use anyhow::{ensure, Result};

#[derive(Clone, Copy, Debug)]
pub struct PlayerMoveFacts {
    pub game_time: i64,
    pub flying: bool,
    pub fall_flying: bool,
    pub shift: bool,
    pub step_height: f32,
    /// Actual LivingEntity movement-efficiency attribute, converted to f32.
    pub movement_efficiency: f32,
    pub can_walk_on_powder_snow: bool,
    /// A required current weather fact when extinguishing reaches rain queries.
    pub rain_known_absent: bool,
}
/// Effects occur at the literal move call sites. The position callback sees
/// new raw coordinates before AABB rebuilding, as original setPosRaw does.
pub trait PlayerMoveHooks {
    fn owns_player_body(&self) -> bool {
        false
    }
    fn fluid_ray_shape(
        &mut self,
        _world: &mut RuntimeWorld,
        _p: BlockPos,
        _fluid: fluid_physics::FluidState,
    ) -> Result<VoxelShape> {
        anyhow::bail!("player fluid ray requires actual singleton shape history")
    }
    fn block_entity_inside(
        &mut self,
        _world: &mut RuntimeWorld,
        _state: &mut EntityPhysicalState,
        _p: BlockPos,
        block: u32,
    ) -> Result<()> {
        anyhow::ensure!(
            entity_move::block_facts(block)?
                .callback_classes()?
                .entity_inside
                == "dtb",
            "unported player block entityInside"
        );
        Ok(())
    }
    fn fire_extinguished(&mut self, _state: &EntityPhysicalState) -> Result<()> {
        anyhow::bail!("player extinguish sound owner missing")
    }
    fn position_changed(&mut self, state: &EntityPhysicalState) -> Result<()>;
    /// ServerPlayer.resetFallDistance clears startingToFallPosition and, for a
    /// living player with a recorded start, triggers FALL_FROM_HEIGHT first.
    /// The host owns that ServerPlayer/criteria context; state still contains
    /// the old fallDistance here. The provider clears fallDistance afterwards.
    fn fall_distance_reset(&mut self, state: &EntityPhysicalState) -> Result<()>;
    fn profile(&mut self, event: ProfileEvent);
    /// Execute Player.walkingStepSound, including special sound blocks where
    /// applicable and the actual level sound RNG. Called only on a real step.
    fn walking_step_sound(
        &mut self,
        world: &mut RuntimeWorld,
        state: &EntityPhysicalState,
        position: BlockPos,
        block: u32,
    ) -> Result<()>;
    fn step_game_event(&mut self, state: &EntityPhysicalState, affected_block: u32) -> Result<()>;
    /// ServerPlayer.onInsideBlock invokes the ENTER_BLOCK criterion, including
    /// air cells. A certified empty catalogue may implement its actual empty gate.
    fn inside_block(&mut self, state: &EntityPhysicalState, block: u32) -> Result<()>;
}

pub struct PlayerMoveWorld<'a, H: PlayerMoveHooks + ?Sized> {
    pub world: &'a mut RuntimeWorld,
    pub facts: PlayerMoveFacts,
    pub host: &'a mut H,
    actor: CollisionActor,
    entities: &'a [CollisionEntity],
    border: CollisionBorder,
}
impl<'a, H: PlayerMoveHooks + ?Sized> PlayerMoveWorld<'a, H> {
    pub fn new(
        world: &'a mut RuntimeWorld,
        state: &EntityPhysicalState,
        entities: &'a [CollisionEntity],
        border: CollisionBorder,
        facts: PlayerMoveFacts,
        host: &'a mut H,
    ) -> Result<Self> {
        ensure!(
            facts.step_height.is_finite() && facts.step_height >= 0.,
            "invalid player step-height attribute"
        );
        ensure!(
            facts.movement_efficiency.is_finite(),
            "player movement-efficiency attribute unavailable"
        );
        Ok(Self {
            world,
            facts,
            host,
            actor: CollisionActor {
                entity_id: state.entity_id,
                position: state.position_vec(),
            },
            entities,
            border,
        })
    }
    pub fn context(&self, state: &EntityPhysicalState) -> EntityCollisionContext {
        EntityCollisionContext {
            bottom_y: state.position.y,
            descending: self.facts.shift,
            fall_distance: state.fall_distance,
            is_falling_block: false,
            can_walk_on_powder_snow: self.facts.can_walk_on_powder_snow,
        }
    }
    fn collision(&mut self) -> RuntimeCollision<'_> {
        RuntimeCollision::new(self.world, self.actor, self.entities, self.border)
    }
    fn sync_actor(&mut self, state: &EntityPhysicalState) -> Result<()> {
        ensure!(
            self.actor.entity_id == state.entity_id,
            "player collision actor changed"
        );
        self.actor.position = state.position_vec();
        Ok(())
    }
    pub(crate) fn no_collision_at(
        &mut self,
        state: &EntityPhysicalState,
        bounds: Aabb,
    ) -> Result<bool> {
        self.sync_actor(state)?;
        let query = CollisionQuery {
            actor_id: state.entity_id,
            bounds,
            context: self.context(state),
        };
        player_physics::no_collision(self, query)
    }
    fn can_fall_at_least(
        &mut self,
        state: &EntityPhysicalState,
        x: f64,
        z: f64,
        down: f32,
    ) -> Result<bool> {
        let b = state.bounds;
        self.no_collision_at(
            state,
            Aabb::new(
                b.min_x + x,
                b.min_y - f64::from(down) - 9.999999747378752e-6,
                b.min_z + z,
                b.max_x + x,
                b.min_y,
                b.max_z + z,
            ),
        )
    }
    fn inherited(block: u32, hook: &str, actual: &str, expected: &str) -> Result<()> {
        ensure!(
            actual == expected,
            "unported player {hook} callback {actual} for state {block}"
        );
        Ok(())
    }
    fn cells(bounds: Aabb, inward: f64) -> Result<(BlockPos, BlockPos)> {
        ensure!(
            [
                bounds.min_x,
                bounds.min_y,
                bounds.min_z,
                bounds.max_x,
                bounds.max_y,
                bounds.max_z
            ]
            .iter()
            .all(|v| v.is_finite()),
            "nonfinite player block query"
        );
        let min = (
            java_floor(bounds.min_x + inward),
            java_floor(bounds.min_y + inward),
            java_floor(bounds.min_z + inward),
        );
        let max = (
            java_floor(bounds.max_x - inward),
            java_floor(bounds.max_y - inward),
            java_floor(bounds.max_z - inward),
        );
        let volume = [
            i64::from(max.0) - i64::from(min.0) + 1,
            i64::from(max.1) - i64::from(min.1) + 1,
            i64::from(max.2) - i64::from(min.2) + 1,
        ]
        .into_iter()
        .try_fold(1u64, |n, v| n.checked_mul(v.max(0) as u64));
        ensure!(
            volume.is_some_and(|n| n <= 1_048_576),
            "player block query exceeds available bounded context"
        );
        Ok((min, max))
    }
    fn inside_region_available(&mut self, min: BlockPos, max: BlockPos) -> Result<bool> {
        if max.1 < MIN_Y || min.1 > MAX_Y {
            return Ok(false);
        }
        if let RuntimeWorld::Anvil(world) = self.world {
            for x in min.0.div_euclid(16)..=max.0.div_euclid(16) {
                for z in min.2.div_euclid(16)..=max.2.div_euclid(16) {
                    if !world.has_chunk(x, z)? {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }
    fn block_step_effects(
        &mut self,
        state: &EntityPhysicalState,
        position: BlockPos,
        block: u32,
        sound: bool,
        event: bool,
        requested: Vec3,
    ) -> Result<bool> {
        let facts = *entity_move::block_facts(block)?;
        if facts.air() {
            return Ok(false);
        }
        let qualifies = state.on_ground()
            || facts.climbable_or_powder_snow()
            || (state.pose == 5 && requested.y == 0.);
        if qualifies && state.shared_flags & 0x10 == 0 {
            if sound {
                self.host
                    .walking_step_sound(self.world, state, position, block)?;
            }
            if event {
                self.host.step_game_event(state, block)?;
            }
            return Ok(true);
        }
        Ok(false)
    }
}
impl<H: PlayerMoveHooks + ?Sized> WorldCollision for PlayerMoveWorld<'_, H> {
    fn block_collisions(&mut self, q: CollisionQuery) -> Result<Vec<VoxelShape>> {
        self.collision().block_collisions(q)
    }
    fn entity_collisions(&mut self, q: CollisionQuery) -> Result<Vec<VoxelShape>> {
        self.collision().entity_collisions(q)
    }
    fn border_collision(&mut self, q: CollisionQuery) -> Result<Option<VoxelShape>> {
        self.collision().border_collision(q)
    }
    fn all_blocks_air(&mut self, b: Aabb) -> Result<bool> {
        self.collision().all_blocks_air(b)
    }
}
impl<H: PlayerMoveHooks + ?Sized> EntityMoveWorld for PlayerMoveWorld<'_, H> {
    fn profile(&mut self, e: ProfileEvent) {
        self.host.profile(e);
    }
    fn game_time(&mut self) -> Result<i64> {
        Ok(self.facts.game_time)
    }
    fn set_position(&mut self, s: &mut EntityPhysicalState, p: Vec3) -> Result<()> {
        entity_move::set_base_position(s, p, self)?;
        self.sync_actor(s)
    }
    fn position_changed(&mut self, s: &mut EntityPhysicalState) -> Result<()> {
        self.sync_actor(s)?;
        self.host.position_changed(s)
    }
    fn reset_fall_distance(&mut self, s: &mut EntityPhysicalState) -> Result<()> {
        self.host.fall_distance_reset(s)?;
        s.fall_distance = 0.;
        Ok(())
    }
    fn set_on_ground_with_movement(
        &mut self,
        s: &mut EntityPhysicalState,
        on: bool,
        m: Vec3,
    ) -> Result<()> {
        entity_move::update_supporting_block(s, on, Some(m), self)
    }
    fn block_speed_factor(&mut self, s: &EntityPhysicalState) -> Result<f32> {
        if self.facts.flying || self.facts.fall_flying {
            return Ok(1.);
        }
        let factor = entity_move::block_speed_factor(s, self)?;
        Ok(factor + self.facts.movement_efficiency * (1. - factor))
    }
    fn maybe_back_off_from_edge(
        &mut self,
        s: &mut EntityPhysicalState,
        m: Vec3,
        mover: MoverType,
    ) -> Result<Vec3> {
        if self.facts.flying
            || m.y > 0.
            || m.y.is_nan()
            || !matches!(mover, MoverType::SelfMove | MoverType::Player)
            || !self.facts.shift
        {
            return Ok(m);
        }
        let step = self.facts.step_height;
        let above = s.on_ground()
            || (s.fall_distance < step
                && !self.can_fall_at_least(s, 0., 0., step - s.fall_distance)?);
        if !above {
            return Ok(m);
        }
        let (mut x, mut z) = (m.x, m.z);
        let (dx, dz) = (x.signum() * 0.05, z.signum() * 0.05);
        while x != 0. && self.can_fall_at_least(s, x, 0., step)? {
            x = if x.abs() <= 0.05 { 0. } else { x - dx };
        }
        while z != 0. && self.can_fall_at_least(s, 0., z, step)? {
            z = if z.abs() <= 0.05 { 0. } else { z - dz };
        }
        while x != 0. && z != 0. && self.can_fall_at_least(s, x, z, step)? {
            x = if x.abs() <= 0.05 { 0. } else { x - dx };
            z = if z.abs() <= 0.05 { 0. } else { z - dz };
        }
        Ok(Vec3::new(x, m.y, z))
    }
    fn fall_damage_resetting_ray(
        &mut self,
        _: &EntityPhysicalState,
        from: Vec3,
        to: Vec3,
    ) -> Result<bool> {
        geometry::traverse_blocks(from, to, |p| {
            let result = (|| -> Result<bool> {
                let block = self.world.block(p.0, p.1, p.2)?;
                let fluid = fluid_physics::block_facts(block)?.fluid;
                let block_hit = entity_move::block_facts(block)?.fall_damage_resetting()
                    && VoxelShape::from_aabb(Aabb::new(0., 0., 0., 1., 1., 1.))
                        .clip(from, to, p)
                        .is_some();
                let fluid_hit = if fluid.kind == FluidKind::Water {
                    self.host
                        .fluid_ray_shape(self.world, p, fluid)?
                        .clip(from, to, p)
                        .is_some()
                } else {
                    false
                };
                Ok(block_hit || fluid_hit)
            })();
            match result {
                Ok(false) => None,
                other => Some(other),
            }
        })
        .unwrap_or(Ok(false))
    }
    fn horizontal_collision_minor(&mut self, _: &EntityPhysicalState, _: Vec3) -> Result<bool> {
        // Player/LivingEntity inherit the original Entity return-false method.
        Ok(false)
    }
    fn supporting_block(&mut self, s: &EntityPhysicalState, b: Aabb) -> Result<Option<BlockPos>> {
        self.sync_actor(s)?;
        let q = CollisionQuery {
            actor_id: s.entity_id,
            bounds: b,
            context: self.context(s),
        };
        entity_support::find_supporting_block(&mut self.collision(), q)
    }
    fn block_state(&mut self, p: BlockPos) -> Result<u32> {
        self.world.block(p.0, p.1, p.2)
    }
    fn check_fall_damage(
        &mut self,
        _: &mut EntityPhysicalState,
        _: f64,
        _: bool,
        _: u32,
        _: BlockPos,
    ) -> Result<()> {
        // ServerPlayer.checkFallDamage is literally return-only. The movement
        // listener separately invokes doCheckFallDamage after moveTo acceptance.
        Ok(())
    }
    fn after_fall_on_block(&mut self, s: &mut EntityPhysicalState, block: u32) -> Result<()> {
        let hooks = entity_move::block_facts(block)?.callback_classes()?;
        Self::inherited(block, "afterFall", &hooks.after_fall, "dfy")?;
        entity_move::default_after_fall_on_block(s);
        Ok(())
    }
    fn step_on(&mut self, _: &mut EntityPhysicalState, block: u32, _: BlockPos) -> Result<()> {
        Self::inherited(
            block,
            "stepOn",
            &entity_move::block_facts(block)?.callback_classes()?.step_on,
            "dfy",
        )
    }
    fn movement_emission(
        &mut self,
        s: &mut EntityPhysicalState,
        mode: MovementEmission,
        requested: Vec3,
        collided: Vec3,
        on_pos: BlockPos,
        on_block: u32,
    ) -> Result<()> {
        if mode == MovementEmission::None {
            return Ok(());
        }
        ensure!(
            mode == MovementEmission::All,
            "ordinary Player movement uses ALL or NONE emission only"
        );
        s.fly_dist += (collided.length() * 0.6) as f32;
        let feet = entity_move::on_position(s, 1e-5_f32, self)?;
        let feet_block = self.block_state(feet)?;
        let y = if entity_move::block_facts(feet_block)?.climbable_or_powder_snow() {
            collided.y
        } else {
            0.
        };
        s.walk_dist += collided.horizontal_distance_squared().sqrt() as f32 * 0.6_f32;
        s.move_dist +=
            (collided.x * collided.x + y * y + collided.z * collided.z).sqrt() as f32 * 0.6_f32;
        if s.move_dist > s.next_step && !entity_move::block_facts(feet_block)?.air() {
            let same = feet == on_pos;
            let mut emitted =
                self.block_step_effects(s, on_pos, on_block, true, same, requested)?;
            if !same {
                emitted |= self.block_step_effects(s, feet, feet_block, false, true, requested)?;
            }
            if emitted {
                s.next_step = (s.move_dist as i32).wrapping_add(1) as f32;
            } else if s.was_touching_water {
                anyhow::bail!("player swim movement sound/event unavailable");
            }
        }
        // Ordinary Player inherits Entity.isFlapping=false, so an air block
        // invokes no flap callback/event. No distance is silently discarded.
        Ok(())
    }
    fn try_check_inside_blocks(&mut self, s: &mut EntityPhysicalState) -> Result<()> {
        let (min, max) = Self::cells(s.bounds, 1e-7)?;
        if !self.inside_region_available(min, max)? {
            return Ok(());
        }
        for x in min.0..=max.0 {
            for y in min.1..=max.1 {
                for z in min.2..=max.2 {
                    if s.removed {
                        return Ok(());
                    }
                    let block = self.world.block(x, y, z)?;
                    let f = *entity_move::block_facts(block)?;
                    ensure!(
                        self.host.owns_player_body()
                            || fluid_physics::block_facts(block)?.fluid.is_empty(),
                        "player fluid entry needs owned baseTick/travel context at {x},{y},{z}"
                    );
                    ensure!(
                        !f.climbable_or_powder_snow(),
                        "player climb/powder-snow inside-block behavior unavailable"
                    );
                    self.host
                        .block_entity_inside(self.world, s, (x, y, z), block)?;
                    self.host.inside_block(s, block)?;
                }
            }
        }
        Ok(())
    }
    fn any_fire_or_lava(&mut self, b: Aabb) -> Result<bool> {
        let (min, max) = Self::cells(b, 0.)?;
        for z in min.2..=max.2 {
            for y in min.1..=max.1 {
                for x in min.0..=max.0 {
                    if entity_move::block_facts(self.world.block(x, y, z)?)?.fire_or_lava() {
                        if self.host.owns_player_body() {
                            return Ok(true);
                        }
                        anyhow::bail!(
                            "player fire/lava movement requires actual damage/ignition context"
                        );
                    }
                }
            }
        }
        Ok(false)
    }
    fn water_rain_or_bubble(&mut self, s: &EntityPhysicalState) -> Result<bool> {
        ensure!(
            self.host.owns_player_body() || !s.was_touching_water,
            "player water exposure context unavailable"
        );
        ensure!(
            self.facts.rain_known_absent,
            "player rain exposure context unavailable"
        );
        let p = s.block_position;
        ensure!(
            !entity_move::block_facts(self.world.block(p.0, p.1, p.2)?)?.bubble_column(),
            "player bubble-column movement context unavailable"
        );
        Ok(s.was_touching_water)
    }
    fn play_fire_extinguished_sound(&mut self, s: &mut EntityPhysicalState) -> Result<()> {
        self.host.fire_extinguished(s)
    }
}
