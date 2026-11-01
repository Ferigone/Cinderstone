//! Owned creative-player packet movement, collision, jumping and teleport state.
//!
//! Entity storage callbacks and final chunk/observer publication are distinct.
//! The listener tick requires the embedding runtime's actual player body tick;
//! this adapter never replaces that callback with a fabricated zero velocity.
//!
//! Keep one PlayerMotion per admitted player, under the same world/registry
//! lock used for motion and ordered observer reconciliation. Supply current
//! collision entities, world border, weather and movement-check environment.
//! `position_changed` maintains raw spatial storage; only the accepted final
//! position may produce the observer reconciliation. Teleport/block-ACK hooks
//! enqueue their packets immediately, before subsequent gameplay callbacks.
use crate::{
    entity::Position,
    entity_move::{self, EntityMoveContext, EntityMoveWorld, MovementEmission, MoverType},
    entity_state::{java_floor, EntityPhysicalState},
    fluid_physics::{self, FluidBody, FluidKind, FluidTag, FluidWorld, RuntimeFluidWorld},
    geometry::{self, Vec3},
    player_movement::{
        self, FloatingTickContext, GameMode, IgnoreReason, MoveObservation, MovePacket,
        MovementEffect, MovementEnvironment, MovementState, PhysicsChecks, PlayerFlags,
        PlayerSnapshot, PostMove, PreMove, Rotation, TeleportAck, TeleportCause, TeleportCommand,
        Transform,
    },
    player_physics,
    player_stats::{self, StatKey},
    protocol::Packet,
    runtime_collision::{CollisionBorder, CollisionEntity},
    runtime_world::RuntimeWorld,
};
use anyhow::{ensure, Context, Result};
#[path = "main_player_move_world.rs"]
mod move_world;
pub use move_world::{PlayerMoveFacts, PlayerMoveHooks, PlayerMoveWorld};

#[derive(Clone, Copy, Debug)]
pub struct PlayerMotionAttributes {
    pub step_height: f32,
    pub jump_strength: f32,
    pub jump_boost_power: f32,
    pub movement_efficiency: f32,
    pub gravity: f64,
    pub safe_fall_distance: f64,
}
impl PlayerMotionAttributes {
    /// Actual fresh Player attribute/effect values; replace from the owned
    /// attribute/effect system when equipment or effects alter these values.
    pub fn fresh_player() -> Self {
        Self {
            step_height: 0.6,
            jump_strength: 0.42,
            jump_boost_power: 0.,
            movement_efficiency: 0.,
            gravity: 0.08,
            safe_fall_distance: 3.,
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct CreativeMotionControls {
    pub flying: bool,
    pub sprinting: bool,
    pub shift: bool,
    pub won_game: bool,
    pub sleeping: bool,
    pub changing_dimension: bool,
    pub fall_flying: bool,
    pub levitating: bool,
    pub auto_spin_attack: bool,
    pub can_walk_on_powder_snow: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct PlayerMotionFrame {
    pub game_time: i64,
    pub environment: MovementEnvironment,
    pub rain_known_absent: bool,
    pub dead_or_dying: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct PlayerTeleportRequest {
    pub target: Transform,
    pub relative_flags: u8,
}
#[derive(Clone, Debug)]
pub struct FallParticles {
    pub block_state: u32,
    pub position: Vec3,
    pub count: i32,
    pub offset: Vec3,
    pub speed: f64,
}
/// Required real gameplay/storage boundaries, called in original order. No
/// default implementations turn unknown statistics, criteria or sound into no-ops.
pub trait PlayerMotionHooks: PlayerMoveHooks {
    /// Enqueue the actual ClientboundPlayerPosition packet at this call site.
    /// Rollback fall/game-event effects must occur after this enqueue.
    fn send_teleport(&mut self, command: TeleportCommand) -> Result<()>;
    fn acknowledge_block_changes(&mut self, sequence: i32) -> Result<()>;
    fn move_chunk_source(&mut self, state: &EntityPhysicalState) -> Result<()>;
    fn award_stat(&mut self, player_id: i32, key: StatKey, amount: i32) -> Result<()>;
    fn hit_ground(&mut self, state: &EntityPhysicalState, affected_block: u32) -> Result<()>;
    /// ServerPlayer.onChangedBlock delegates to the actual location-based
    /// enchantment effects for a nonspectator player, before landing particles.
    fn on_changed_block(
        &mut self,
        world: &mut RuntimeWorld,
        state: &EntityPhysicalState,
        position: entity_move::BlockPos,
    ) -> Result<()>;
    fn fall_particles(
        &mut self,
        state: &EntityPhysicalState,
        particles: FallParticles,
    ) -> Result<()>;
}
/// Additional capability required only by the complete listener tick. A
/// movement-only runtime cannot accidentally provide an empty player body.
pub trait PlayerBodyTickHooks: PlayerMotionHooks {
    /// Execute actual Player.doTick/body work, including its genuine velocity
    /// changes. This is separate from the listener's firstGood restoration.
    fn player_do_tick(&mut self, motion: &mut PlayerMotion, world: &mut RuntimeWorld)
        -> Result<()>;
}
#[derive(Clone, Copy, Debug)]
pub enum PlayerMotionOutcome {
    Accepted {
        transform: Transform,
        on_ground: bool,
    },
    /// Already enqueued through PlayerMotionHooks::send_teleport.
    Teleport(TeleportCommand),
    Ignored(IgnoreReason),
    Disconnect(&'static str),
}
#[derive(Clone, Copy, Debug)]
pub struct PlayerListenerTick {
    /// Already enqueued before the player body callback; do not send twice.
    pub acknowledged_block_sequence: Option<i32>,
    pub disconnect: Option<&'static str>,
}
#[derive(Clone, Debug, Default)]
pub struct PlayerImpulseContext {
    pub reset_grace_time: i32,
    pub explosion_cause: Option<i32>,
    pub impact_position: Option<Vec3>,
    pub ignore_fall_damage: bool,
}
pub struct PlayerMotion {
    pub physical: EntityPhysicalState,
    pub listener: MovementState,
    pub controls: CreativeMotionControls,
    pub attributes: PlayerMotionAttributes,
    pub known_movement: Vec3,
    /// Entity.xo/yo/zo; distinct from EntityPhysicalState.old_position (xOld).
    pub previous_position: Vec3,
    pub impulse: PlayerImpulseContext,
    pub spawn_extra_particles_on_fall: bool,
    pub last_climbable_position: Option<entity_move::BlockPos>,
}
impl PlayerMotion {
    /// Fresh creative player's real initial values. Velocity must be supplied
    /// from construction or restored authoritative state, not reset per packet.
    pub fn new(
        entity_id: i32,
        uuid: [u8; 16],
        position: Position,
        velocity: Vec3,
        pose: u8,
    ) -> Result<Self> {
        let d = player_physics::player_dimensions(pose, 1.)?;
        let mut physical = EntityPhysicalState::new(
            entity_id,
            uuid,
            position,
            velocity,
            d.width,
            d.height,
            d.eye_height,
        )?;
        physical.pose = pose;
        Ok(Self {
            listener: MovementState::new(physical.position_vec()),
            previous_position: physical.position_vec(),
            physical,
            controls: CreativeMotionControls::default(),
            attributes: PlayerMotionAttributes::fresh_player(),
            known_movement: Vec3::ZERO,
            impulse: PlayerImpulseContext::default(),
            spawn_extra_particles_on_fall: false,
            last_climbable_position: None,
        })
    }
    pub fn transform(&self) -> Transform {
        Transform {
            position: self.physical.position_vec(),
            rotation: Rotation {
                yaw: self.physical.position.yaw,
                pitch: self.physical.position.pitch,
            },
        }
    }
    pub fn set_pose(&mut self, pose: u8) -> Result<()> {
        let d = player_physics::player_dimensions(pose, 1.)?;
        self.physical.set_pose(pose);
        self.physical.width = d.width;
        self.physical.height = d.height;
        self.physical.eye_height = d.eye_height;
        self.physical.refresh_bounds();
        Ok(())
    }
    pub fn set_velocity(&mut self, v: Vec3) -> Result<()> {
        ensure!(
            [v.x, v.y, v.z].into_iter().all(f64::is_finite),
            "invalid authoritative player velocity"
        );
        self.physical.velocity = v;
        Ok(())
    }
    pub fn set_controls(&mut self, controls: CreativeMotionControls) {
        self.controls = controls;
        self.physical.set_shared_flag(1, controls.shift);
        self.physical.set_shared_flag(3, controls.sprinting);
    }
    pub fn request_flying(&mut self, requested: bool) {
        self.controls.flying = requested;
    }
    pub fn set_ignore_fall_damage_from_impulse(&mut self, ignore: bool) {
        self.impulse.ignore_fall_damage = ignore;
        self.impulse.reset_grace_time = if ignore { 40 } else { 0 };
    }
    pub fn reset_current_impulse_context_if_required(&mut self) {
        if self.impulse.reset_grace_time == 0 {
            self.impulse = PlayerImpulseContext::default();
        }
    }
    fn flags(&self) -> PlayerFlags {
        PlayerFlags {
            won_game: self.controls.won_game,
            passenger: self.physical.vehicle.is_some(),
            sleeping: self.controls.sleeping,
            changing_dimension: self.controls.changing_dimension,
            fall_flying: self.controls.fall_flying,
            game_mode: GameMode::Creative,
            no_physics: self.physical.no_physics,
            may_fly: true,
            levitating: self.controls.levitating,
            auto_spin_attack: self.controls.auto_spin_attack,
            spectator: false,
            landed_in_liquid: self.physical.fluid_heights.iter().any(|v| *v > 0.),
            on_climbable: false,
        }
    }
    fn snapshot(&self) -> PlayerSnapshot {
        PlayerSnapshot {
            transform: self.transform(),
            bounds: self.physical.bounds,
            velocity: self.physical.velocity,
            on_ground: self.physical.on_ground(),
            flags: self.flags(),
        }
    }
    pub(crate) fn body_move_facts(&self, frame: PlayerMotionFrame) -> PlayerMoveFacts {
        self.move_facts(frame)
    }
    fn move_facts(&self, frame: PlayerMotionFrame) -> PlayerMoveFacts {
        PlayerMoveFacts {
            game_time: frame.game_time,
            flying: self.controls.flying,
            fall_flying: self.controls.fall_flying,
            shift: self.controls.shift,
            step_height: self.attributes.step_height,
            movement_efficiency: self.attributes.movement_efficiency,
            can_walk_on_powder_snow: self.controls.can_walk_on_powder_snow,
            rain_known_absent: frame.rain_known_absent,
        }
    }
    fn stat(host: &mut impl PlayerMotionHooks, id: i32, name: &str, amount: i32) -> Result<()> {
        let key = player_stats::stat_key("minecraft:custom", name)
            .context("missing original movement statistic")?;
        host.award_stat(id, key, amount)
    }
    fn move_to(&mut self, target: Transform, provider: &mut impl EntityMoveWorld) -> Result<()> {
        // The packet listener calls Entity.absMoveTo, not Entity.moveTo.
        // It changes xo/yo/zo before setPos; xOld/yOld/zOld stay untouched.
        let position = Vec3::new(
            target.position.x.clamp(-30_000_000., 30_000_000.),
            target.position.y,
            target.position.z.clamp(-30_000_000., 30_000_000.),
        );
        self.previous_position = position;
        provider.set_position(&mut self.physical, position)?;
        self.physical.position.yaw = target.rotation.yaw % 360f32;
        self.physical.position.pitch = target.rotation.pitch.clamp(-90., 90.) % 360f32;
        self.physical.old_yaw = self.physical.position.yaw;
        self.physical.old_pitch = self.physical.position.pitch;
        Ok(())
    }
    fn validate_dry_state(&self, world: &mut RuntimeWorld, body_owned: bool) -> Result<()> {
        ensure!(
            body_owned
                || (!self.physical.was_touching_water
                    && self.physical.fluid_heights.iter().all(|v| *v == 0.)),
            "player wet state requires an actual body owner"
        );
        ensure!(
            self.physical.vehicle.is_none(),
            "player vehicle movement requires its owned vehicle pipeline"
        );
        ensure!(
            !self.controls.fall_flying
                && !self.controls.levitating
                && !self.controls.auto_spin_attack,
            "elytra/effect/spin travel branch unavailable"
        );
        let b = self.physical.bounds.deflate(Vec3::new(0.001, 0.001, 0.001));
        for x in java_floor(b.min_x)..=(b.max_x.ceil() as i32).wrapping_sub(1) {
            for y in java_floor(b.min_y)..=(b.max_y.ceil() as i32).wrapping_sub(1) {
                for z in java_floor(b.min_z)..=(b.max_z.ceil() as i32).wrapping_sub(1) {
                    let block = world.block(x, y, z)?;
                    ensure!(
                        body_owned
                            || fluid_physics::block_facts(block)?.fluid.kind == FluidKind::Empty,
                        "player fluid entry requires an actual body owner"
                    );
                    ensure!(
                        !entity_move::block_facts(block)?.climbable_or_powder_snow(),
                        "player fluid/climbable/powder lifecycle requires actual travel/base tick"
                    );
                }
            }
        }
        Ok(())
    }
    fn jump<H: PlayerMotionHooks>(&mut self, provider: &mut PlayerMoveWorld<'_, H>) -> Result<()> {
        let inside = provider.block_state(self.physical.block_position)?;
        let below = entity_move::on_position(&self.physical, 0.500001, provider)?;
        let inside_factor = jump_factor(inside)?;
        let factor = if inside_factor as f64 == 1. {
            jump_factor(provider.block_state(below)?)?
        } else {
            inside_factor
        };
        let jump = self.attributes.jump_strength * 1f32 * factor + self.attributes.jump_boost_power;
        if jump > 1e-5_f32 {
            self.physical.velocity.y = f64::from(jump);
            if self.controls.sprinting {
                let angle = self.physical.position.yaw * 0.017453292f32;
                self.physical.velocity.x += f64::from(-geometry::vanilla_sin(angle)) * 0.2;
                self.physical.velocity.z += f64::from(geometry::vanilla_cos(angle)) * 0.2;
            }
            self.physical.has_impulse = true;
        }
        Self::stat(provider.host, self.physical.entity_id, "minecraft:jump", 1)
        // Creative abilities.invulnerable makes causeFoodExhaustion return.
    }
    fn fall_check<H: PlayerMotionHooks>(
        &mut self,
        origin: Vec3,
        on_ground: bool,
        provider: &mut PlayerMoveWorld<'_, H>,
    ) -> Result<()> {
        // Entity.touchingUnloadedChunk uses floor(min), ceil(max) of the
        // bounding box inflated by one, including the inclusive chunk maxima.
        let bounds = self.physical.bounds.inflate(Vec3::new(1., 1., 1.));
        if !(RuntimeFluidWorld {
            world: provider.world,
        })
        .has_chunks_at(
            java_floor(bounds.min_x),
            java_floor(bounds.min_z),
            bounds.max_x.ceil() as i32,
            bounds.max_z.ceil() as i32,
        )? {
            return Ok(());
        }
        let movement = self.physical.position_vec().subtract(origin);
        provider.set_on_ground_with_movement(&mut self.physical, on_ground, movement)?;
        let p = entity_move::on_position(&self.physical, 0.2, provider)?;
        let block = provider.block_state(p)?;
        if self.spawn_extra_particles_on_fall && on_ground && self.physical.fall_distance > 0. {
            let count = (50f32 * self.physical.fall_distance).clamp(0., 200.) as i32;
            provider.host.fall_particles(
                &self.physical,
                FallParticles {
                    block_state: block,
                    position: Vec3::new(
                        f64::from(p.0) + 0.5,
                        f64::from(p.1) + 1.,
                        f64::from(p.2) + 0.5,
                    ),
                    count,
                    offset: Vec3::new(
                        0.30000001192092896,
                        0.30000001192092896,
                        0.30000001192092896,
                    ),
                    speed: 0.15000000596046448,
                },
            )?;
            self.spawn_extra_particles_on_fall = false;
        }
        // LivingEntity.checkFallDamage samples water when not already touching.
        // Dry support is established by the real fluid scan, not a constant.
        if !self.physical.was_touching_water {
            let update = fluid_physics::update_fluid_pushing(
                &mut RuntimeFluidWorld {
                    world: provider.world,
                },
                FluidBody {
                    bounds: self.physical.bounds,
                    velocity: self.physical.velocity,
                    pushed_by_fluid: !self.controls.flying,
                    is_player: true,
                },
                FluidTag::Water,
                0.014,
            )?;
            self.physical.velocity = update.velocity;
            if let Some(height) = update.height_written {
                self.physical.fluid_heights[0] = height;
            }
        }
        if on_ground {
            let fall = self.physical.fall_distance;
            if fall > 0. {
                provider
                    .host
                    .on_changed_block(provider.world, &self.physical, p)?;
                let callbacks = entity_move::block_facts(block)?.callback_classes()?;
                ensure!(
                    callbacks.fall_on == "dfy",
                    "special player fallOn callback unavailable"
                );
                if f64::from(fall) > self.attributes.safe_fall_distance
                    && !entity_move::block_facts(block)?.air()
                {
                    let amount =
                        (f64::from(fall) - self.attributes.safe_fall_distance).ceil() as f32;
                    let spread = f64::from(0.2f32 + amount / 15f32).min(2.5);
                    let mut position = self.physical.position_vec();
                    if p.0 != self.physical.block_position.0
                        || p.2 != self.physical.block_position.2
                    {
                        let dx = position.x - f64::from(p.0) - 0.5;
                        let dz = position.z - f64::from(p.2) - 0.5;
                        let scale = dx.abs().max(dz.abs());
                        position.x = f64::from(p.0) + 0.5 + dx / scale * 0.5;
                        position.z = f64::from(p.2) + 0.5 + dz / scale * 0.5;
                    }
                    provider.host.fall_particles(
                        &self.physical,
                        FallParticles {
                            block_state: block,
                            position,
                            count: (150. * spread) as i32,
                            offset: Vec3::ZERO,
                            speed: 0.15000000596046448,
                        },
                    )?;
                }
                let affected = if let Some(support) = self.physical.supporting_block {
                    provider.block_state(support)?
                } else {
                    block
                };
                provider.host.hit_ground(&self.physical, affected)?;
            }
            provider.reset_fall_distance(&mut self.physical)?;
            self.last_climbable_position = None;
        } else if movement.y < 0. {
            self.physical.fall_distance -= movement.y as f32;
        }
        Ok(())
    }
    fn movement_statistics(&self, origin: Vec3, host: &mut impl PlayerMotionHooks) -> Result<()> {
        let delta = self.physical.position_vec().subtract(origin);
        // ServerPlayer.checkMovementStatistics first ignores passengers and an
        // exactly unchanged position, then samples the live post-move water
        // flags before onGround. The eyes flag is the actual Entity base-tick
        // fluid tag cache, not a new sample at the packet's destination.
        if self.physical.vehicle.is_some()
            || (delta.x == 0. && delta.y == 0. && delta.z == 0.)
        {
            return Ok(());
        }
        let full_distance = || {
            java_round(
                ((delta.x * delta.x + delta.y * delta.y + delta.z * delta.z).sqrt() as f32)
                    * 100f32,
            )
        };
        // Player.isSwimming additionally gates the shared bit by current
        // abilities.flying and isSpectator; a flying toggle can precede the
        // next body tick that clears the underlying bit.
        let swimming = !self.controls.flying
            && !self.physical.is_spectator
            && self.physical.is_swimming();
        let water_stat = if swimming {
            Some(("minecraft:swim_one_cm", full_distance()))
        } else if self.physical.fluid_on_eyes & 1 != 0 {
            Some(("minecraft:walk_under_water_one_cm", full_distance()))
        } else if self.physical.was_touching_water {
            Some((
                "minecraft:walk_on_water_one_cm",
                java_round(
                    ((delta.x * delta.x + delta.z * delta.z).sqrt() as f32) * 100f32,
                ),
            ))
        } else {
            None
        };
        if let Some((key, amount)) = water_stat {
            if amount > 0 {
                Self::stat(host, self.physical.entity_id, key, amount)?;
            }
            // The currently certified creative player is invulnerable, so
            // Player.causeFoodExhaustion returns without changing food state.
            // Survival's real exhaustion/damage owner remains a prerequisite
            // before broadening that admission profile.
            return Ok(());
        }
        let distance = ((delta.x * delta.x + delta.z * delta.z).sqrt() as f32) * 100f32;
        let amount = java_round(distance);
        if self.physical.on_ground() {
            if amount > 0 {
                Self::stat(
                    host,
                    self.physical.entity_id,
                    if self.controls.sprinting {
                        "minecraft:sprint_one_cm"
                    } else if self.controls.shift {
                        "minecraft:crouch_one_cm"
                    } else {
                        "minecraft:walk_one_cm"
                    },
                    amount,
                )?;
            }
        } else if amount > 25 {
            Self::stat(
                host,
                self.physical.entity_id,
                "minecraft:fly_one_cm",
                amount,
            )?;
        }
        Ok(())
    }
    pub fn handle_move<H: PlayerMotionHooks>(
        &mut self,
        packet: MovePacket,
        world: &mut RuntimeWorld,
        entities: &[CollisionEntity],
        border: CollisionBorder,
        frame: PlayerMotionFrame,
        host: &mut H,
    ) -> Result<PlayerMotionOutcome> {
        let pre = self.prepare_move(packet, frame.environment);
        self.handle_prepared_move(pre, world, entities, border, frame, host)
    }
    /// Run exactly once per packet, before acquiring its potentially large
    /// world-query region. Rejections and teleport resends need no target halo.
    pub fn prepare_move(
        &mut self,
        packet: MovePacket,
        environment: MovementEnvironment,
    ) -> PreMove {
        let snapshot = self.snapshot();
        self.listener.prepare(packet, snapshot, environment)
    }
    pub fn handle_prepared_move<H: PlayerMotionHooks>(
        &mut self,
        pre: PreMove,
        world: &mut RuntimeWorld,
        entities: &[CollisionEntity],
        border: CollisionBorder,
        frame: PlayerMotionFrame,
        host: &mut H,
    ) -> Result<PlayerMotionOutcome> {
        let plan = match pre {
            PreMove::Disconnect(key) => return Ok(PlayerMotionOutcome::Disconnect(key)),
            PreMove::Ignore(reason) => return Ok(PlayerMotionOutcome::Ignored(reason)),
            PreMove::Teleport(command) => {
                self.apply_teleport(command, world, entities, border, frame, host)?;
                return Ok(PlayerMotionOutcome::Teleport(command));
            }
            PreMove::Passenger(target) => {
                // This source branch changes only passenger rotation and the
                // chunk-source position. Vehicle movement is a separate packet.
                let mut provider = PlayerMoveWorld::new(
                    world,
                    &self.physical,
                    entities,
                    border,
                    self.move_facts(frame),
                    host,
                )?;
                self.move_to(target, &mut provider)?;
                provider.host.move_chunk_source(&self.physical)?;
                return Ok(PlayerMotionOutcome::Accepted {
                    transform: self.transform(),
                    on_ground: self.physical.on_ground(),
                });
            }
            PreMove::Move(plan) => plan,
        };
        self.validate_dry_state(world, host.owns_player_body())?;
        let facts = self.move_facts(frame);
        let mut provider =
            PlayerMoveWorld::new(world, &self.physical, entities, border, facts, host)?;
        if plan.jump_from_ground {
            self.jump(&mut provider)?;
        }
        let below = self.physical.vertical_collision_below;
        let movement_emission =
            if self.controls.flying || (self.physical.on_ground() && self.controls.shift) {
                MovementEmission::None
            } else {
                MovementEmission::All
            };
        entity_move::move_entity(
            &mut self.physical,
            MoverType::Player,
            plan.movement,
            EntityMoveContext {
                step_height: self.attributes.step_height,
                descending: self.controls.shift,
                is_falling_block: false,
                can_walk_on_powder_snow: self.controls.can_walk_on_powder_snow,
                movement_emission,
            },
            &mut provider,
        )?;
        let observed = MoveObservation {
            player: self.snapshot(),
            vertical_collision_below_before_move: below,
        };
        let context = provider.context(&self.physical);
        let mut checks = PhysicsChecks {
            world: &mut provider,
            actor_id: self.physical.entity_id,
            context,
        };
        let accepted = match self.listener.postcheck(plan, observed, &mut checks)? {
            PostMove::Rollback { teleport, .. } => {
                self.move_to(teleport.target, &mut provider)?;
                provider.host.send_teleport(teleport)?;
                if let Some((origin, ground)) = teleport.fall_check {
                    self.fall_check(origin, ground, &mut provider)?;
                }
                return Ok(PlayerMotionOutcome::Teleport(teleport));
            }
            PostMove::Accept(accepted) => accepted,
        };
        self.move_to(accepted.plan.target, &mut provider)?;
        let final_snapshot = self.snapshot();
        let context = provider.context(&self.physical);
        let effects = self.listener.finish_acceptance(
            accepted,
            final_snapshot,
            frame.environment,
            &mut PhysicsChecks {
                world: &mut provider,
                actor_id: self.physical.entity_id,
                context,
            },
        )?;
        for effect in effects.effects {
            match effect {
                MovementEffect::MoveChunkSource => {
                    provider.host.move_chunk_source(&self.physical)?
                }
                MovementEffect::SetOnGroundWithMovement {
                    on_ground,
                    movement,
                } => {
                    provider.set_on_ground_with_movement(&mut self.physical, on_ground, movement)?
                }
                MovementEffect::CheckFallDamageFrom { origin, on_ground } => {
                    self.fall_check(origin, on_ground, &mut provider)?
                }
                MovementEffect::SetKnownMovement(movement) => self.known_movement = movement,
                MovementEffect::ResetFallDistance => {
                    provider.reset_fall_distance(&mut self.physical)?;
                }
                MovementEffect::ResetImpulseIfGroundedOrSpecial {
                    packet_on_ground,
                    was_fall_flying,
                    auto_spin_attack,
                } => {
                    if player_movement::should_reset_impulse(
                        packet_on_ground,
                        was_fall_flying,
                        auto_spin_attack,
                        self.flags(),
                    ) {
                        self.reset_current_impulse_context_if_required();
                    }
                }
                MovementEffect::CheckMovementStatisticsFrom { origin } => {
                    self.movement_statistics(origin, provider.host)?
                }
                MovementEffect::CommitLastGood => {
                    self.listener.commit_last_good(self.physical.position_vec())
                }
            }
        }
        Ok(PlayerMotionOutcome::Accepted {
            transform: self.transform(),
            on_ground: self.physical.on_ground(),
        })
    }
    fn apply_teleport<H: PlayerMotionHooks>(
        &mut self,
        command: TeleportCommand,
        world: &mut RuntimeWorld,
        entities: &[CollisionEntity],
        border: CollisionBorder,
        frame: PlayerMotionFrame,
        host: &mut H,
    ) -> Result<()> {
        let mut provider = PlayerMoveWorld::new(
            world,
            &self.physical,
            entities,
            border,
            self.move_facts(frame),
            host,
        )?;
        self.move_to(command.target, &mut provider)?;
        provider.host.send_teleport(command)
    }
    pub fn teleport<H: PlayerMotionHooks>(
        &mut self,
        request: PlayerTeleportRequest,
        world: &mut RuntimeWorld,
        entities: &[CollisionEntity],
        border: CollisionBorder,
        frame: PlayerMotionFrame,
        host: &mut H,
    ) -> Result<TeleportCommand> {
        let current = self.transform();
        let command = self.listener.teleport_with_relative_flags(
            request.target,
            current,
            request.relative_flags,
            TeleportCause::External,
        )?;
        self.apply_teleport(command, world, entities, border, frame, host)?;
        Ok(command)
    }
    pub fn acknowledge_teleport<H: PlayerMotionHooks>(
        &mut self,
        id: i32,
        world: &mut RuntimeWorld,
        entities: &[CollisionEntity],
        border: CollisionBorder,
        frame: PlayerMotionFrame,
        host: &mut H,
    ) -> Result<Option<&'static str>> {
        match self.listener.acknowledge_teleport(
            id,
            self.transform().rotation,
            self.controls.changing_dimension,
        ) {
            TeleportAck::Ignore => Ok(None),
            TeleportAck::Disconnect(key) => Ok(Some(key)),
            TeleportAck::Accepted {
                target,
                finish_dimension_change,
            } => {
                let mut provider = PlayerMoveWorld::new(
                    world,
                    &self.physical,
                    entities,
                    border,
                    self.move_facts(frame),
                    host,
                )?;
                self.move_to(target, &mut provider)?;
                provider.host.move_chunk_source(&self.physical)?;
                if finish_dimension_change {
                    self.controls.changing_dimension = false;
                }
                Ok(None)
            }
        }
    }
    pub fn listener_tick<H: PlayerBodyTickHooks>(
        &mut self,
        world: &mut RuntimeWorld,
        entities: &[CollisionEntity],
        border: CollisionBorder,
        frame: PlayerMotionFrame,
        host: &mut H,
    ) -> Result<PlayerListenerTick> {
        let block_ack =
            (self.listener.pending_block_ack > -1).then_some(self.listener.pending_block_ack);
        if let Some(sequence) = block_ack {
            host.acknowledge_block_changes(sequence)?;
            self.listener.take_block_ack();
        }
        let first = self
            .listener
            .begin_listener_tick(self.physical.position_vec());
        self.previous_position = self.physical.position_vec();
        host.player_do_tick(self, world)?;
        let target = Transform {
            position: first,
            rotation: self.transform().rotation,
        };
        let mut provider = PlayerMoveWorld::new(
            world,
            &self.physical,
            entities,
            border,
            self.move_facts(frame),
            host,
        )?;
        self.move_to(target, &mut provider)?;
        self.listener.finish_listener_tick();
        let disconnect = self.listener.check_floating_tick(FloatingTickContext {
            sleeping: self.controls.sleeping,
            passenger: self.physical.vehicle.is_some(),
            dead_or_dying: frame.dead_or_dying,
            gravity: self.attributes.gravity,
        });
        Ok(PlayerListenerTick {
            acknowledged_block_sequence: block_ack,
            disconnect,
        })
    }
    pub fn teleport_packet(command: TeleportCommand) -> Packet {
        Packet {
            id: 0x40,
            payload: command.payload(),
        }
    }
}
fn java_round(value: f32) -> i32 {
    (f64::from(value) + 0.5).floor() as i32
}
fn jump_factor(block: u32) -> Result<f32> {
    // Original dfy.getJumpFactor over all 26,684 shipped states has exactly
    // one non-unit entry: honey_block state 19445, value 0.5f.
    let name = crate::inventory::block_state_name(block).context("unknown player jump block")?;
    Ok(
        if name == "honey_block" || name == "minecraft:honey_block" {
            0.5
        } else {
            1.
        },
    )
}
