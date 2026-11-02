//! Concrete, synchronous ItemEntity physics over RuntimeWorld.
//!
//! The host owns the real entity RNG, entity-section callbacks and world sound
//! RNG. Sound seeds come from the level's separate threadSafeRandom stream.
//! Ordinary inherited Block callbacks execute here; concrete overrides
//! that have not been ported return errors at their original call sites.
use crate::{
    entity_base::{
        BaseDamage, BaseEffect, BoatFluidState, EntityBaseWorld, EyeFluidSample, FluidTag,
        FluidUpdate,
    },
    entity_move::{
        self, EntityMoveContext, EntityMoveWorld, MovementEmission, MoverType, ProfileEvent,
    },
    entity_state::{java_floor, EntityPhysicalState},
    entity_support,
    fluid_physics::{self, FluidBody, FluidKind, FluidShapeCache, FluidWorld, RuntimeFluidWorld},
    geometry::{self, Aabb, EntityCollisionContext, Vec3, VoxelShape},
    inventory::ItemStack,
    item_damage::{self, DamageSourceFacts, ItemDamageWorld},
    item_entity::{self, FullItemWorld},
    placement,
    player_physics::{self, CollisionQuery, WorldCollision},
    rain_physics::RainExposure,
    random::RandomSource,
    runtime_collision::{CollisionActor, CollisionBorder, CollisionEntity, RuntimeCollision},
    runtime_world::RuntimeWorld,
    world::{MAX_Y, MIN_Y},
};
use anyhow::{bail, ensure, Context, Result};
type BlockPosition = (i32, i32, i32);
type BlockRange = (BlockPosition, BlockPosition);

#[derive(Clone, Copy, Debug)]
pub enum RainAvailability {
    /// The world's actual global raining flag is false.
    NotRaining,
    /// Actual Level.isRaining is true; sky/heightmap, precipitation and biome
    /// exposure require a supplied capability or remain unavailable.
    ExposureUnavailable,
}

#[cfg(test)]
#[path = "../tools/runtime_item_world_damage_tests.rs"]
mod damage_tests;

#[cfg(test)]
#[path = "../tools/runtime_item_world_fluid_shape_tests.rs"]
mod fluid_shape_tests;
#[cfg(test)]
#[path = "../tools/runtime_item_rain_tests.rs"]
mod rain_tests;
#[derive(Clone, Copy, Debug)]
pub struct ItemWorldFacts {
    pub game_time: i64,
    pub minimum_build_height: i32,
    pub ultrawarm: bool,
    pub border: CollisionBorder,
    pub rain: RainAvailability,
    pub has_passengers: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemSound {
    GenericSplash,
    GenericExtinguishFire,
    GenericBurn,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RuntimeItemEffect {
    Base(BaseEffect),
    HitGround {
        affected_block: u32,
    },
    Splash,
    /// ItemEntity uses SoundSource.AMBIENT. The host executes Level.playSound
    /// synchronously, including its one actual threadSafeRandom nextLong seed.
    Sound {
        sound: ItemSound,
        volume: f32,
        pitch: f32,
    },
}
pub trait RuntimeItemHost {
    /// Optional concrete world capabilities. Absence is an explicit error,
    /// never an empty shape, dry weather or ignored pressure callback.
    fn fluid_shape(&mut self, _world: &mut RuntimeWorld, _position: BlockPosition,
        _fluid: fluid_physics::FluidState) -> Result<VoxelShape> {
        bail!("actual method-local fluid shape owner is unavailable")
    }
    fn rain_exposure(&mut self, _world: &mut RuntimeWorld,
        _state: &EntityPhysicalState) -> Result<bool> {
        bail!("actual method-local rain exposure owner is unavailable")
    }
    fn pressure_plate_inside(&mut self, _world: &mut RuntimeWorld,
        _state: &mut EntityPhysicalState, _position: BlockPosition,
        _block_state: u32) -> Result<()> {
        bail!("actual pressure-plate entityInside owner is unavailable")
    }
    /// Original EntityInLevelCallback.onMove: update the entity-section index
    /// now, before a later merge query. Called before the AABB is rebuilt.
    /// Do not retain a host borrow/guard after this synchronous call returns.
    fn position_changed(&mut self, state: &EntityPhysicalState) -> Result<()>;
    fn profile(&mut self, event: ProfileEvent);
    /// Gameplay listeners and world RNG execute at this point. A host may
    /// return typed effects to a packet sink, but must preserve their order.
    fn effect(&mut self, state: &EntityPhysicalState, effect: RuntimeItemEffect) -> Result<()>;
    /// Synchronous ENTITY_DAMAGE after markHurt and the health mutation, before
    /// destruction/removal. This is a game event, not an ENTITY_DIE event or a
    /// damage packet. A host must implement active listeners or return an error.
    fn entity_damage(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
        causing_entity: Option<i32>,
    ) -> Result<()>;
    /// Original level callback onRemove(DISCARDED), with removed=true, after the
    /// stack destruction hook. Remove spatial/tick registration synchronously;
    /// the current original item tick may still execute its remaining phases.
    fn discarded(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
    ) -> Result<()>;
}

/// Source guard shares this literal implemented-dispatch policy. This classifies
/// source states, not an attached runtime capability: dfm still requires the
/// actual synchronous host (and otherwise returns an unsupported error).
/// It does not allow arbitrary overrides merely because a block has a static shape.
pub fn supports_block_callbacks(state: u32) -> Result<bool> {
    let callbacks = entity_move::block_facts(state)?.callback_classes()?;
    Ok(callbacks.fall_on == "dfy"
        && callbacks.after_fall == "dfy"
        && callbacks.step_on == "dfy"
        && (callbacks.entity_inside == "dtb"
            || (callbacks.entity_inside == "dfm" && pressure_plate_inside_state(state)?)
            || (callbacks.entity_inside == "dfl" && fire_inside_damage(state)?.is_some())))
}
fn pressure_plate_inside_state(state: u32) -> Result<bool> {
    let block = crate::redstone::block(state)?;
    Ok(block.pressed_time > 0 && match block.class.as_str() {
        "dlw" => block.max_weight == 0,
        "dpp" => block.max_weight > 0,
        _ => false,
    })
}
fn fire_inside_damage(state: u32) -> Result<Option<f32>> {
    Ok(match placement::placement_block_name(state)? {
        "minecraft:fire" => Some(1.),
        "minecraft:soul_fire" => Some(2.),
        _ => None,
    })
}
struct ItemMoveDamageContext<'a> {
    stack: &'a mut ItemStack,
    health: &'a mut i32,
}

pub struct RuntimeItemWorld<'a, R: RandomSource, H: RuntimeItemHost> {
    pub world: &'a mut RuntimeWorld,
    pub random: &'a mut R,
    pub host: &'a mut H,
    pub facts: ItemWorldFacts,
    pub entities: &'a [CollisionEntity],
    actor: CollisionActor,
    fluid_shape_cache: Option<&'a mut FluidShapeCache>,
    rain_exposure: Option<&'a mut dyn RainExposure>,
    item_move_damage: Option<ItemMoveDamageContext<'a>>,
    experience_orb: bool,
    orb_move_health: Option<&'a mut i32>,
}
impl<'a, R: RandomSource, H: RuntimeItemHost> RuntimeItemWorld<'a, R, H> {
    pub fn new(
        world: &'a mut RuntimeWorld,
        state: &EntityPhysicalState,
        random: &'a mut R,
        host: &'a mut H,
        facts: ItemWorldFacts,
        entities: &'a [CollisionEntity],
    ) -> Result<Self> {
        ensure!(
            facts.minimum_build_height == MIN_Y,
            "runtime overworld build-height facts mismatch"
        );
        ensure!(
            state.width == 0.25 && state.height == 0.25 && state.eye_height == 0.2125,
            "provider requires the actual ItemEntity dimensions"
        );
        Ok(Self {
            world,
            random,
            host,
            facts,
            entities,
            actor: CollisionActor {
                entity_id: state.entity_id,
                position: state.position_vec(),
            },
            fluid_shape_cache: None,
            rain_exposure: None,
            item_move_damage: None,
            experience_orb: false,
            orb_move_health: None,
        })
    }
    /// The inherited Entity base/move implementation with the actual XP-orb
    /// dimensions and its empty doWaterSplashEffect override. This does not
    /// create an ItemState or substitute an item inventory/health owner.
    pub fn new_experience_orb(
        world: &'a mut RuntimeWorld,
        state: &EntityPhysicalState,
        random: &'a mut R,
        host: &'a mut H,
        facts: ItemWorldFacts,
        entities: &'a [CollisionEntity],
    ) -> Result<Self> {
        ensure!(facts.minimum_build_height == MIN_Y, "runtime XP build-height facts mismatch");
        ensure!(state.width == 0.5 && state.height == 0.5 && state.eye_height == 0.425f32,
            "provider requires the actual ExperienceOrb dimensions");
        Ok(Self { world, random, host, facts, entities,
            actor: CollisionActor { entity_id: state.entity_id, position: state.position_vec() },
            fluid_shape_cache: None, rain_exposure: None, item_move_damage: None,
            experience_orb: true, orb_move_health: None })
    }
    pub fn with_experience_orb_health(mut self, health: &'a mut i32) -> Result<Self> {
        ensure!(self.experience_orb, "XP health supplied to an item world");
        self.orb_move_health = Some(health);
        Ok(self)
    }
    /// The same observed process-wide cache must be retained across items,
    /// ticks and dimensions. Omitting it keeps water-ray queries unsupported.
    pub fn with_fluid_shape_cache(mut self, cache: &'a mut FluidShapeCache) -> Self {
        self.fluid_shape_cache = Some(cache);
        self
    }
    /// Supply actual source-validated rain exposure. Global weather remains in
    /// ItemWorldFacts; water and known non-raining weather short-circuit first.
    pub fn with_rain_exposure(mut self, exposure: &'a mut dyn RainExposure) -> Self {
        self.rain_exposure = Some(exposure);
        self
    }
    fn sync_actor(&mut self, state: &EntityPhysicalState) -> Result<()> {
        ensure!(
            self.actor.entity_id == state.entity_id,
            "runtime item actor mismatch"
        );
        self.actor.position = state.position_vec();
        Ok(())
    }
    fn collision(&mut self) -> RuntimeCollision<'_> {
        RuntimeCollision::new(self.world, self.actor, self.entities, self.facts.border)
    }
    fn context(state: &EntityPhysicalState) -> EntityCollisionContext {
        EntityCollisionContext {
            bottom_y: state.position.y,
            descending: state.shared_flags & 2 != 0,
            fall_distance: state.fall_distance,
            is_falling_block: false,
            can_walk_on_powder_snow: false,
        }
    }
    fn fluid_world(&mut self) -> RuntimeFluidWorld<'_> {
        RuntimeFluidWorld { world: self.world }
    }
    fn inherited_hook(&self, block: u32, hook: &str, class: &str, expected: &str) -> Result<()> {
        ensure!(
            class == expected,
            "unported {hook} callback {class} for block state {block}"
        );
        Ok(())
    }
    fn play_sound(
        &mut self,
        state: &EntityPhysicalState,
        sound: ItemSound,
        volume: f32,
        pitch: f32,
    ) -> Result<()> {
        if !state.silent {
            self.host.effect(
                state,
                RuntimeItemEffect::Sound {
                    sound,
                    volume,
                    pitch,
                },
            )?;
        }
        Ok(())
    }
    fn region_available(&mut self, min: (i32, i32, i32), max: (i32, i32, i32)) -> Result<bool> {
        if max.1 < self.facts.minimum_build_height || min.1 > MAX_Y {
            return Ok(false);
        }
        self.fluid_world().has_chunks_at(min.0, min.2, max.0, max.2)
    }
    fn cells(bounds: Aabb, inward: f64) -> Result<BlockRange> {
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
            "nonfinite item block query"
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
            "item block query exceeds available bounded context"
        );
        Ok((min, max))
    }
}
impl<R: RandomSource, H: RuntimeItemHost> WorldCollision for RuntimeItemWorld<'_, R, H> {
    fn block_collisions(&mut self, query: CollisionQuery) -> Result<Vec<VoxelShape>> {
        self.collision().block_collisions(query)
    }
    fn entity_collisions(&mut self, query: CollisionQuery) -> Result<Vec<VoxelShape>> {
        self.collision().entity_collisions(query)
    }
    fn border_collision(&mut self, query: CollisionQuery) -> Result<Option<VoxelShape>> {
        self.collision().border_collision(query)
    }
    fn all_blocks_air(&mut self, bounds: Aabb) -> Result<bool> {
        self.collision().all_blocks_air(bounds)
    }
}
impl<R: RandomSource, H: RuntimeItemHost> EntityBaseWorld for RuntimeItemWorld<'_, R, H> {
    fn is_client_side(&self) -> bool {
        false
    }
    fn is_server_level(&self) -> bool {
        true
    }
    fn minimum_build_height(&self) -> i32 {
        self.facts.minimum_build_height
    }
    fn ultrawarm(&self) -> bool {
        self.facts.ultrawarm
    }
    fn vehicle_removed(&mut self, vehicle: i32) -> Result<bool> {
        bail!("item vehicle lifecycle unavailable for entity {vehicle}")
    }
    fn process_portal(&mut self, _state: &mut EntityPhysicalState) -> Result<()> {
        bail!("active item portal processing unavailable")
    }
    fn spawn_sprint_particle(&mut self, _state: &mut EntityPhysicalState) -> Result<()> {
        bail!("item sprint particle callback unavailable")
    }
    fn boat_fluid_state(&mut self, vehicle: i32) -> Result<Option<BoatFluidState>> {
        bail!("item boat fluid context unavailable for vehicle {vehicle}")
    }
    fn update_fluid(
        &mut self,
        state: &EntityPhysicalState,
        tag: FluidTag,
        push: f64,
    ) -> Result<FluidUpdate> {
        self.sync_actor(state)?;
        let result = fluid_physics::update_fluid_pushing(
            &mut self.fluid_world(),
            FluidBody {
                bounds: state.bounds,
                velocity: state.velocity,
                pushed_by_fluid: true,
                is_player: false,
            },
            match tag {
                FluidTag::Water => fluid_physics::FluidTag::Water,
                FluidTag::Lava => fluid_physics::FluidTag::Lava,
            },
            push,
        )?;
        Ok(FluidUpdate {
            touching: result.touching,
            velocity: result.velocity,
            height_written: result.height_written,
        })
    }
    fn water_splash(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
        if self.experience_orb { return Ok(()); }
        ensure!(
            !self.facts.has_passengers,
            "item splash controlling-passenger context unavailable"
        );
        let velocity = state.velocity;
        let speed = (velocity.x * velocity.x * f64::from(0.2f32)
            + velocity.y * velocity.y
            + velocity.z * velocity.z * f64::from(0.2f32))
        .sqrt() as f32
            * 0.2f32;
        let volume = if speed.is_nan() {
            f32::NAN
        } else {
            speed.min(1.)
        };
        let pitch = 1f32 + (self.random.next_f32() - self.random.next_f32()) * 0.4f32;
        self.play_sound(state, ItemSound::GenericSplash, volume, pitch)?;
        let bound = 1f32 + state.width * 20f32;
        let mut i = 0;
        while (i as f32) < bound {
            self.random.next_f64();
            self.random.next_f64();
            self.random.next_f64();
            i += 1;
        }
        i = 0;
        while (i as f32) < bound {
            self.random.next_f64();
            self.random.next_f64();
            i += 1;
        }
        // ServerLevel inherits the empty Level.addParticle overload called by
        // this method. Preserve draws without inventing particle packets.
        self.host.effect(state, RuntimeItemEffect::Splash)
    }
    fn eye_fluid_sample(&mut self, x: f64, y: f64, z: f64) -> Result<EyeFluidSample> {
        let p = (java_floor(x), java_floor(y), java_floor(z));
        let mut world = self.fluid_world();
        let fluid = world.fluid(p)?;
        let surface = f64::from(p.1 as f32 + fluid_physics::fluid_height(&mut world, p, fluid)?);
        Ok(EyeFluidSample {
            surface,
            tags: match fluid.kind {
                FluidKind::Empty => 0,
                FluidKind::Water => 1,
                FluidKind::Lava => 2,
            },
        })
    }
    fn block_fluid_tags(&mut self, p: (i32, i32, i32)) -> Result<u8> {
        Ok(match self.fluid_world().fluid(p)?.kind {
            FluidKind::Empty => 0,
            FluidKind::Water => 1,
            FluidKind::Lava => 2,
        })
    }
    fn next_entity_float(&mut self, _state: &EntityPhysicalState) -> Result<f32> {
        Ok(self.random.next_f32())
    }
    fn effect(&mut self, state: &mut EntityPhysicalState, effect: BaseEffect) -> Result<()> {
        match effect {
            BaseEffect::LavaBurnSound { volume, pitch } => {
                self.play_sound(state, ItemSound::GenericBurn, volume, pitch)
            }
            other => self.host.effect(state, RuntimeItemEffect::Base(other)),
        }
    }
    fn tick_leash(&mut self, _state: &mut EntityPhysicalState) -> Result<()> {
        bail!("leashable item lifecycle unavailable")
    }
}
impl<R: RandomSource, H: RuntimeItemHost> EntityMoveWorld for RuntimeItemWorld<'_, R, H> {
    fn profile(&mut self, event: ProfileEvent) {
        self.host.profile(event);
    }
    fn game_time(&mut self) -> Result<i64> {
        Ok(self.facts.game_time)
    }
    fn set_position(&mut self, state: &mut EntityPhysicalState, position: Vec3) -> Result<()> {
        entity_move::set_base_position(state, position, self)?;
        self.sync_actor(state)
    }
    fn position_changed(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
        self.sync_actor(state)?;
        self.host.position_changed(state)
    }
    fn reset_fall_distance(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
        state.fall_distance = 0.;
        Ok(())
    }
    fn set_on_ground_with_movement(
        &mut self,
        state: &mut EntityPhysicalState,
        on_ground: bool,
        movement: Vec3,
    ) -> Result<()> {
        entity_move::update_supporting_block(state, on_ground, Some(movement), self)
    }
    fn block_speed_factor(&mut self, state: &EntityPhysicalState) -> Result<f32> {
        entity_move::block_speed_factor(state, self)
    }
    fn maybe_back_off_from_edge(
        &mut self,
        _state: &mut EntityPhysicalState,
        movement: Vec3,
        _mover: MoverType,
    ) -> Result<Vec3> {
        Ok(movement)
    }
    fn fall_damage_resetting_ray(
        &mut self,
        _state: &EntityPhysicalState,
        from: Vec3,
        to: Vec3,
    ) -> Result<bool> {
        geometry::traverse_blocks(from, to, |p| {
            let result = (|| -> Result<bool> {
                let block = self.world.block(p.0, p.1, p.2)?;
                let resetting = entity_move::block_facts(block)?.fall_damage_resetting();
                let fluid = self.fluid_world().fluid(p)?;
                let block_hit = resetting
                    && VoxelShape::from_aabb(Aabb::new(0., 0., 0., 1., 1., 1.))
                        .clip(from, to, p)
                        .is_some();
                // BlockGetter.clip obtains both shapes before selecting a hit;
                // a resetting block must not short circuit the fluid query.
                let fluid_hit = if fluid.kind == FluidKind::Water {
                    let shape = if let Some(cache) = self.fluid_shape_cache.as_deref_mut() {
                        cache.shape(&mut RuntimeFluidWorld { world: self.world }, p, fluid)?
                    } else {
                        self.host.fluid_shape(self.world, p, fluid)
                            .with_context(|| format!("fluid ray shape cache history unavailable at {p:?}"))?
                    };
                    shape
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
    fn horizontal_collision_minor(
        &mut self,
        _state: &EntityPhysicalState,
        _collided: Vec3,
    ) -> Result<bool> {
        Ok(false)
    }
    fn supporting_block(
        &mut self,
        state: &EntityPhysicalState,
        bounds: Aabb,
    ) -> Result<Option<(i32, i32, i32)>> {
        self.sync_actor(state)?;
        let query = CollisionQuery {
            actor_id: state.entity_id,
            bounds,
            context: Self::context(state),
        };
        entity_support::find_supporting_block(&mut self.collision(), query)
    }
    fn block_state(&mut self, p: (i32, i32, i32)) -> Result<u32> {
        self.world.block(p.0, p.1, p.2)
    }
    fn check_fall_damage(
        &mut self,
        state: &mut EntityPhysicalState,
        dy: f64,
        on_ground: bool,
        block: u32,
        _position: (i32, i32, i32),
    ) -> Result<()> {
        if on_ground {
            if state.fall_distance > 0. {
                let callbacks = entity_move::block_facts(block)?.callback_classes()?;
                self.inherited_hook(block, "fallOn", &callbacks.fall_on, "dfy")?;
                ensure!(
                    !self.facts.has_passengers,
                    "item passenger fall damage unavailable"
                );
                // Ordinary ItemEntity inherits Entity.causeFallDamage=false.
                let affected_block = if let Some(p) = state.supporting_block {
                    self.world.block(p.0, p.1, p.2)?
                } else {
                    block
                };
                self.host
                    .effect(state, RuntimeItemEffect::HitGround { affected_block })?;
            }
            self.reset_fall_distance(state)?;
        } else if dy < 0. {
            state.fall_distance -= dy as f32;
        }
        Ok(())
    }
    fn after_fall_on_block(&mut self, state: &mut EntityPhysicalState, block: u32) -> Result<()> {
        let callbacks = entity_move::block_facts(block)?.callback_classes()?;
        self.inherited_hook(block, "afterFall", &callbacks.after_fall, "dfy")?;
        entity_move::default_after_fall_on_block(state);
        Ok(())
    }
    fn step_on(
        &mut self,
        _state: &mut EntityPhysicalState,
        block: u32,
        _position: (i32, i32, i32),
    ) -> Result<()> {
        let callbacks = entity_move::block_facts(block)?.callback_classes()?;
        self.inherited_hook(block, "stepOn", &callbacks.step_on, "dfy")
    }
    fn movement_emission(
        &mut self,
        _state: &mut EntityPhysicalState,
        emission: MovementEmission,
        _requested: Vec3,
        _collided: Vec3,
        _on_pos: (i32, i32, i32),
        _on_block: u32,
    ) -> Result<()> {
        ensure!(
            emission == MovementEmission::None,
            "non-ItemEntity movement emission unavailable"
        );
        Ok(())
    }
    fn try_check_inside_blocks(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
        let (min, max) = Self::cells(state.bounds, 1e-7)?;
        if !self.region_available(min, max)? {
            return Ok(());
        }
        for x in min.0..=max.0 {
            for y in min.1..=max.1 {
                for z in min.2..=max.2 {
                    if state.removed {
                        return Ok(());
                    }
                    let block = self.world.block(x, y, z)?;
                    let callbacks = entity_move::block_facts(block)?.callback_classes()?;
                    if callbacks.entity_inside == "dfm" {
                        self.host.pressure_plate_inside(self.world, state, (x, y, z), block)?;
                        // The concrete owner executes BasePressurePlateBlock
                        // here; the inherited Item/XP onInsideBlock is empty.
                    } else if callbacks.entity_inside == "dfl" {
                        let amount = fire_inside_damage(block)?
                            .context("unported BaseFireBlock entityInside type")?;
                        if self.experience_orb {
                            let health = self.orb_move_health.as_deref_mut()
                                .context("fire requires actual mutable XP orb health")?;
                            if !state.fire_immune {
                                state.remaining_fire_ticks = state.remaining_fire_ticks.wrapping_add(1);
                                if state.remaining_fire_ticks == 0 && state.remaining_fire_ticks < 160 {
                                    state.remaining_fire_ticks = 160;
                                }
                            }
                            state.hurt_marked = true;
                            *health = (*health as f32 - amount) as i32;
                            if *health <= 0 { state.removed = true; }
                            continue;
                        }
                        let damage = self.item_move_damage.take().context(
                            "fire entityInside requires the canonical mutable item stack/health",
                        )?;
                        // BaseFireBlock has no additional voxel-shape test. The
                        // inward1e-7 cell traversal above is its collision gate.
                        if !item_damage::item_fire_immune(damage.stack) {
                            state.remaining_fire_ticks = state.remaining_fire_ticks.wrapping_add(1);
                            if state.remaining_fire_ticks == 0 {
                                // Entity.igniteForSeconds(8) -> igniteForTicks160;
                                // ItemEntity inherits the generic Entity method.
                                if state.remaining_fire_ticks < 160 {
                                    state.remaining_fire_ticks = 160;
                                }
                            }
                        }
                        // hurt(IN_FIRE,fireDamage) is unconditional, including
                        // fire-immune items; ItemEntity.hurt owns that rejection.
                        let result = item_damage::hurt_item(
                            state,
                            damage.stack,
                            damage.health,
                            &mut DamageSourceFacts {
                                fire: true,
                                ..Default::default()
                            },
                            amount,
                            self,
                        );
                        self.item_move_damage = Some(damage);
                        result?;
                        // BaseFireBlock's super entityInside and the following
                        // inherited ItemEntity.onInsideBlock are empty methods.
                    } else {
                        self.inherited_hook(
                            block,
                            "entityInside",
                            &callbacks.entity_inside,
                            "dtb",
                        )?;
                        // Both inherited BlockBehaviour.entityInside and the
                        // following ItemEntity.onInsideBlock are empty methods.
                    }
                }
            }
        }
        Ok(())
    }
    fn any_fire_or_lava(&mut self, bounds: Aabb) -> Result<bool> {
        let (min, max) = Self::cells(bounds, 0.)?;
        // BlockGetter.getBlockStates uses BlockPos.betweenClosed: x fastest.
        for z in min.2..=max.2 {
            for y in min.1..=max.1 {
                for x in min.0..=max.0 {
                    if entity_move::block_facts(self.world.block(x, y, z)?)?.fire_or_lava() {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
    fn water_rain_or_bubble(&mut self, state: &EntityPhysicalState) -> Result<bool> {
        if state.was_touching_water {
            return Ok(true);
        }
        match self.facts.rain {
            RainAvailability::NotRaining => {}
            RainAvailability::ExposureUnavailable => {
                let exposed = if let Some(exposure) = self.rain_exposure.as_deref_mut() {
                    exposure.is_in_rain(self.world, state)?
                } else {
                    self.host.rain_exposure(self.world, state)
                        .context("item rain exposure unavailable")?
                };
                if exposed {
                    return Ok(true);
                }
            }
        }
        let p = state.block_position;
        Ok(entity_move::block_facts(self.world.block(p.0, p.1, p.2)?)?.bubble_column())
    }
    fn play_fire_extinguished_sound(&mut self, state: &mut EntityPhysicalState) -> Result<()> {
        let pitch = 1.6f32 + (self.random.next_f32() - self.random.next_f32()) * 0.4f32;
        self.play_sound(state, ItemSound::GenericExtinguishFire, 0.7, pitch)
    }
}
impl<R: RandomSource, H: RuntimeItemHost> FullItemWorld for RuntimeItemWorld<'_, R, H> {
    fn item_no_collision(&mut self, state: &EntityPhysicalState, bounds: Aabb) -> Result<bool> {
        self.sync_actor(state)?;
        player_physics::no_collision(
            self,
            CollisionQuery {
                actor_id: state.entity_id,
                bounds,
                context: Self::context(state),
            },
        )
    }
    fn escape_block_is_full_collision(&mut self, p: (i32, i32, i32)) -> Result<bool> {
        let block = self.world.block(p.0, p.1, p.2)?;
        let Some(boxes) = geometry::static_block_collision_boxes(block) else {
            bail!("uncached item escape full-collision shape unavailable at {p:?}");
        };
        Ok(boxes == [[0., 0., 0., 1., 1., 1.]])
    }
    fn move_item_full(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &mut ItemStack,
        health: &mut i32,
        movement: Vec3,
    ) -> Result<()> {
        ensure!(!self.experience_orb, "XP must use its physical/health owner, not ItemEntity.move");
        self.sync_actor(state)?;
        // Reborrow the genuine backend/RNG/capabilities for exactly this move.
        // The canonical item context cannot outlive its physical move call.
        let mut scoped = RuntimeItemWorld {
            world: &mut *self.world,
            random: &mut *self.random,
            host: &mut *self.host,
            facts: self.facts,
            entities: self.entities,
            actor: self.actor,
            fluid_shape_cache: self.fluid_shape_cache.as_deref_mut(),
            rain_exposure: self
                .rain_exposure
                .as_mut()
                .map(|exposure| &mut **exposure as &mut dyn RainExposure),
            item_move_damage: Some(ItemMoveDamageContext { stack, health }),
            experience_orb: false,
            orb_move_health: None,
        };
        let result = entity_move::move_entity(
            state,
            MoverType::SelfMove,
            movement,
            EntityMoveContext {
                step_height: 0.,
                descending: state.shared_flags & 2 != 0,
                is_falling_block: false,
                can_walk_on_powder_snow: false,
                movement_emission: MovementEmission::None,
            },
            &mut scoped,
        );
        self.actor = scoped.actor;
        result?;
        Ok(())
    }
    fn item_ground_friction(&mut self, state: &EntityPhysicalState) -> Result<f32> {
        let p = entity_move::on_position(state, 0.999999f32, self)?;
        Ok(item_entity::block_physics(self.world.block(p.0, p.1, p.2)?)?.friction)
    }
    fn hurt_item(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &mut ItemStack,
        health: &mut i32,
        damage: BaseDamage,
        amount: f32,
    ) -> Result<bool> {
        item_damage::hurt_item(
            state,
            stack,
            health,
            &mut DamageSourceFacts::from(damage),
            amount,
            self,
        )
    }
}

impl<R: RandomSource, H: RuntimeItemHost> ItemDamageWorld for RuntimeItemWorld<'_, R, H> {
    fn is_client_side(&self) -> bool {
        false
    }
    fn entity_damage(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &ItemStack,
        health: &mut i32,
        causing_entity: Option<i32>,
    ) -> Result<()> {
        self.host
            .entity_damage(state, stack, *health, causing_entity)
    }
    fn on_destroyed(
        &mut self,
        _state: &mut EntityPhysicalState,
        stack: &mut ItemStack,
        _health: &mut i32,
    ) -> Result<()> {
        // This mutates BlockItem's CONTAINER patch in place. ItemStack's
        // metadata reference and dirty slot 8 are deliberately untouched.
        item_damage::apply_supported_destruction(stack)
    }
    fn discarded(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &ItemStack,
        health: &mut i32,
    ) -> Result<()> {
        ensure!(
            state.vehicle.is_none() && !self.facts.has_passengers,
            "item discard riding/passenger callbacks unavailable"
        );
        ensure!(state.removed, "item discard requires the removed flag");
        self.host.discarded(state, stack, *health)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{entity::Position, random::LegacyRandomSource, world::FlatWorld};
    use serde_json::{json, Value};
    struct Host {
        events: Vec<String>,
    }
    impl RuntimeItemHost for Host {
        fn position_changed(&mut self, _state: &EntityPhysicalState) -> Result<()> {
            Ok(())
        }
        fn profile(&mut self, _event: ProfileEvent) {}
        fn effect(
            &mut self,
            _state: &EntityPhysicalState,
            effect: RuntimeItemEffect,
        ) -> Result<()> {
            match effect {
                RuntimeItemEffect::HitGround { .. } => {
                    self.events.push("game_event/minecraft:hit_ground".into())
                }
                RuntimeItemEffect::Splash => self.events.push("game_event/minecraft:splash".into()),
                RuntimeItemEffect::Sound { .. } => {
                    bail!("dry oracle host has no configured world sound RNG")
                }
                RuntimeItemEffect::Base(_) => {}
            }
            Ok(())
        }
        fn entity_damage(
            &mut self,
            _state: &EntityPhysicalState,
            _stack: &ItemStack,
            _health: i32,
            _causing_entity: Option<i32>,
        ) -> Result<()> {
            bail!("undamaged tick oracle host has no entity-damage callback")
        }
        fn discarded(
            &mut self,
            _state: &EntityPhysicalState,
            _stack: &ItemStack,
            _health: i32,
        ) -> Result<()> {
            bail!("undamaged tick oracle host has no damage-removal callback")
        }
    }
    fn facts(game_time: i64) -> ItemWorldFacts {
        ItemWorldFacts {
            game_time,
            minimum_build_height: MIN_Y,
            ultrawarm: false,
            border: CollisionBorder::default(),
            rain: RainAvailability::NotRaining,
            has_passengers: false,
        }
    }
    fn item(position: Vec3, velocity: Vec3) -> item_entity::ItemState {
        item_entity::ItemState::new(
            17,
            [1; 16],
            Position {
                x: position.x,
                y: position.y,
                z: position.z,
                yaw: 0.,
                pitch: 0.,
                on_ground: false,
            },
            velocity,
            ItemStack::new(1, 1).unwrap(),
        )
        .unwrap()
    }
    fn double(value: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
    }
    fn input_vector(value: &Value) -> Vec3 {
        Vec3::new(double(&value[0]), double(&value[1]), double(&value[2]))
    }
    fn double_bits(value: f64) -> String {
        format!("{:016x}", value.to_bits())
    }
    fn output_vector(value: Vec3) -> Value {
        json!([
            double_bits(value.x),
            double_bits(value.y),
            double_bits(value.z)
        ])
    }
    fn verify_full_item_scenes(vectors: &Value, anvil_world: Option<std::ffi::OsString>) {
        let limit = if anvil_world.is_some() { 8 } else { usize::MAX };
        for (scene_index, scene) in vectors["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .take(limit)
            .enumerate()
        {
            let q = &scene["query"];
            let mut world = if let Some(path) = &anvil_world {
                let mut anvil = crate::storage::VanillaWorld::open(path).unwrap();
                let p = input_vector(&q["position"]);
                let cx = java_floor(p.x).div_euclid(16);
                let cz = java_floor(p.z).div_euclid(16);
                for x in cx - 3..=cx + 3 {
                    for z in cz - 3..=cz + 3 {
                        // Physics needs the full chunk's block cache, not a
                        // network packet that also requests neighbour light.
                        anvil.block(x * 16, 64, z * 16).unwrap();
                    }
                }
                RuntimeWorld::Anvil(anvil)
            } else {
                RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39))
            };
            for block in q["blocks"].as_array().unwrap() {
                let (x, y, z, state) = (
                    block[0].as_i64().unwrap() as i32,
                    block[1].as_i64().unwrap() as i32,
                    block[2].as_i64().unwrap() as i32,
                    block[3].as_u64().unwrap() as u32,
                );
                match &mut world {
                    RuntimeWorld::Fixture(flat) => {
                        flat.set_block(x, y, z, state);
                    }
                    RuntimeWorld::Anvil(anvil) => {
                        anvil.set_block(x, y, z, state).unwrap();
                    }
                }
            }
            let mut item = item(input_vector(&q["position"]), input_vector(&q["velocity"]));
            item.entity_id = q["entity_id"].as_i64().unwrap() as i32;
            item.tick_count = q["tick_count"].as_i64().unwrap() as i32;
            item.age = q["age"].as_i64().unwrap() as i32;
            item.pickup_delay = q["pickup_delay"].as_i64().unwrap() as i32;
            item.set_on_ground(q["on_ground"].as_bool().unwrap());
            item.fall_distance = f32::from_bits(
                u32::from_str_radix(q["fall_distance"].as_str().unwrap(), 16).unwrap(),
            );
            item.set_no_gravity(q["no_gravity"].as_bool().unwrap());
            if q["drain_initial_metadata"].as_bool().unwrap_or(true) {
                item.take_metadata_dirty();
            }
            let mut random = LegacyRandomSource::new(q["seed"].as_i64().unwrap());
            let mut host = Host { events: Vec::new() };
            for (tick, expected) in scene["states"].as_array().unwrap().iter().enumerate() {
                assert!(
                    !item.removed,
                    "removed before original scene{scene_index}tick{tick}"
                );
                if q["clear_has_impulse"].as_bool().unwrap_or(true) {
                    item.has_impulse = false;
                }
                host.events.clear();
                let mut provider = RuntimeItemWorld::new(
                    &mut world,
                    &item.physical,
                    &mut random,
                    &mut host,
                    facts(tick as i64 + 1),
                    &[],
                )
                .unwrap();
                item.tick_full(&mut provider).unwrap_or_else(|error| {
                    panic!("concrete item scene{scene_index}tick{tick}: {error:#}")
                });
                let dirty = item.take_metadata_dirty();
                let b = item.bounds;
                let mut actual = json!({
                    "position":output_vector(item.position_vec()),"velocity":output_vector(item.velocity),
                    "bounds":[double_bits(b.min_x),double_bits(b.min_y),double_bits(b.min_z),double_bits(b.max_x),double_bits(b.max_y),double_bits(b.max_z)],
                    "on_ground":item.on_ground(),"horizontal_collision":item.horizontal_collision,"vertical_collision":item.vertical_collision,"vertical_collision_below":item.vertical_collision_below,"minor_horizontal_collision":item.minor_horizontal_collision,
                    "no_physics":item.no_physics,"has_impulse":item.has_impulse,"first_tick":item.first_tick,"touching_water":item.was_touching_water,"was_eye_in_water":item.was_eye_in_water,"on_ground_no_blocks":item.on_ground_no_blocks,
                    "fire_ticks":item.remaining_fire_ticks,"tick_count":item.tick_count,"age":item.age,"pickup_delay":item.pickup_delay,"health":item.health,"fall_distance":format!("{:08x}",item.fall_distance.to_bits()),"removed":item.removed,"support":item.supporting_block,
                    "fluid_depths":[double_bits(item.fluid_heights[0]),double_bits(item.fluid_heights[1])],"fluid_on_eyes":item.fluid_on_eyes,"flags":item.shared_flags,"dirty_mask":dirty,"events":host.events,
                });
                // Gson omits explicit null-valued fields in its default mode.
                if expected.get("support").is_none() {
                    assert!(item.supporting_block.is_none());
                    actual.as_object_mut().unwrap().remove("support");
                }
                assert_eq!(
                    actual, *expected,
                    "full original item scene{scene_index}tick{tick}"
                );
            }
            assert_eq!(
                random.next_i32(),
                scene["rng_next_int"].as_i64().unwrap() as i32,
                "item RNG after scene{scene_index}"
            );
        }
    }
    #[test]
    fn full_original_item_ticks_in_concrete_runtime_world() {
        let vectors: Value =
            serde_json::from_str(include_str!("../fixtures/item-full-vectors.json")).unwrap();
        // The integration runner supplies a private copy of the official flat
        // Anvil world. Normal unit runs remain self-contained fixture checks.
        verify_full_item_scenes(&vectors, std::env::var_os("MC121_ITEM_ANVIL_WORLD"));
    }
    #[test]
    fn full_original_item_ticks_while_submerged_in_water() {
        let vectors: Value =
            serde_json::from_str(include_str!("../fixtures/item-water-vectors.json")).unwrap();
        verify_full_item_scenes(&vectors, None);
    }
    #[test]
    fn cached_escape_full_collision_matches_original_all_states() {
        let original: Value =
            serde_json::from_str(include_str!("../fixtures/runtime_item_world_shapes.json"))
                .unwrap();
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        let item = item(Vec3::new(0.5, 65., 0.5), Vec3::ZERO);
        let mut random = LegacyRandomSource::new(901);
        let mut host = Host { events: Vec::new() };
        let mut provider = RuntimeItemWorld::new(
            &mut world,
            &item.physical,
            &mut random,
            &mut host,
            facts(1),
            &[],
        )
        .unwrap();
        let mut compared = 0;
        for state in 0..26684 {
            if geometry::static_block_collision_boxes(state).is_none() {
                continue;
            }
            if let RuntimeWorld::Fixture(flat) = provider.world {
                flat.set_block(0, 70, 0, state);
            }
            assert_eq!(
                provider.escape_block_is_full_collision((0, 70, 0)).unwrap(),
                original["full"][state as usize].as_bool().unwrap(),
                "escape state{state}"
            );
            compared += 1;
        }
        assert_eq!(compared, 26411);
    }
    #[test]
    fn fire_inside_borrows_canonical_item_damage_and_ignites_before_callbacks() {
        struct FireHost(Vec<(bool, i32, i32)>);
        impl RuntimeItemHost for FireHost {
            fn position_changed(&mut self, _: &EntityPhysicalState) -> Result<()> {
                bail!("zero-displacement fire smoke must not change position")
            }
            fn profile(&mut self, _: ProfileEvent) {}
            fn effect(&mut self, _: &EntityPhysicalState, _: RuntimeItemEffect) -> Result<()> {
                bail!("unexpected fire-move effect")
            }
            fn entity_damage(
                &mut self,
                state: &EntityPhysicalState,
                _: &ItemStack,
                health: i32,
                cause: Option<i32>,
            ) -> Result<()> {
                assert!(cause.is_none());
                assert!(state.hurt_marked);
                self.0
                    .push((state.removed, health, state.remaining_fire_ticks));
                Ok(())
            }
            fn discarded(
                &mut self,
                state: &EntityPhysicalState,
                _: &ItemStack,
                health: i32,
            ) -> Result<()> {
                self.0
                    .push((state.removed, health, state.remaining_fire_ticks));
                Ok(())
            }
        }
        let state_named = |name: &str| {
            (0..26_684)
                .find(|state| placement::placement_block_name(*state).unwrap() == name)
                .unwrap()
        };
        let fire = state_named("minecraft:fire");
        let soul = state_named("minecraft:soul_fire");
        let resistant = (1..1500)
            .find(|id| crate::inventory::item_name(*id) == Some("netherite_scrap"))
            .unwrap();
        for (block, item_id, initial_health, expected_health, expected_fire) in [
            (fire, 1, 5, 4, 160),
            (soul, 1, 5, 3, 160),
            (fire, resistant, 5, 5, -1),
            (fire, 1, 1, 0, 160),
        ] {
            assert!(supports_block_callbacks(block).unwrap());
            let mut flat = FlatWorld::new(0, 1, 10, 9, 39);
            flat.set_block(0, 65, 0, block);
            let mut world = RuntimeWorld::Fixture(flat);
            let mut item = item(Vec3::new(0.5, 65., 0.5), Vec3::ZERO);
            item.set_stack(ItemStack::new(item_id, 1).unwrap());
            item.health = initial_health;
            let immune = item_damage::item_fire_immune(&item.stack);
            let mut random = LegacyRandomSource::new(17);
            let initial_random = random.state48();
            let mut host = FireHost(Vec::new());
            {
                let mut provider = RuntimeItemWorld::new(
                    &mut world,
                    &item.physical,
                    &mut random,
                    &mut host,
                    facts(1),
                    &[],
                )
                .unwrap();
                provider
                    .move_item_full(
                        &mut item.physical,
                        &mut item.stack,
                        &mut item.health,
                        Vec3::ZERO,
                    )
                    .unwrap();
            }
            assert_eq!(
                (item.health, item.remaining_fire_ticks),
                (expected_health, expected_fire)
            );
            assert_eq!(item.removed, expected_health == 0);
            assert_eq!(random.state48(), initial_random);
            if immune {
                assert!(host.0.is_empty());
            } else {
                assert_eq!(host.0[0], (false, expected_health, 160));
                if item.removed {
                    assert_eq!(host.0[1], (true, 0, 160));
                }
            }
        }
    }
    #[test]
    fn unsupported_inside_callback_and_water_ray_cache_are_explicit() {
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        let mut item = item(Vec3::new(0.5, 65., 0.5), Vec3::ZERO);
        let mut random = LegacyRandomSource::new(902);
        let mut host = Host { events: Vec::new() };
        let mut provider = RuntimeItemWorld::new(
            &mut world,
            &item.physical,
            &mut random,
            &mut host,
            facts(1),
            &[],
        )
        .unwrap();
        let custom = (0..26684)
            .find(|state| {
                entity_move::block_facts(*state)
                    .unwrap()
                    .callback_classes()
                    .unwrap()
                    .entity_inside
                    != "dtb"
            })
            .unwrap();
        if let RuntimeWorld::Fixture(flat) = provider.world {
            flat.set_block(0, 65, 0, custom);
        }
        assert!(provider
            .try_check_inside_blocks(&mut item.physical)
            .unwrap_err()
            .to_string()
            .contains("unported entityInside callback"));
        let water = (0..26684)
            .find(|state| {
                fluid_physics::block_facts(*state).unwrap().fluid.kind == FluidKind::Water
            })
            .unwrap();
        if let RuntimeWorld::Fixture(flat) = provider.world {
            flat.set_block(0, 65, 0, water);
        }
        assert!(provider
            .fall_damage_resetting_ray(
                &item.physical,
                Vec3::new(0.5, 66., 0.5),
                Vec3::new(0.5, 64., 0.5)
            )
            .unwrap_err()
            .to_string()
            .contains("fluid ray shape cache history unavailable"));
    }
}
