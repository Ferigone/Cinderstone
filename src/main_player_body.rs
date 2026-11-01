//! Real native Player body composition; the connection listener owns its clock.
use super::main_player_motion_hooks as motion_host;
use super::*;
use mc121_rust::item_host::RuntimeGameplayHooks;
use mc121_rust::{
    entity_base::BaseEffect,
    entity_move::{self, EntityMoveWorld, ProfileEvent},
    entity_state::EntityPhysicalState,
    main_player_motion::{
        FallParticles, PlayerBodyTickHooks, PlayerMotion, PlayerMotionFrame, PlayerMotionHooks,
        PlayerMoveHooks, PlayerTeleportRequest,
    },
    player_body::{self, PlayerBodyHooks, PlayerBodyPhase, PlayerBodyState},
    player_body_world::PlayerBodyWorldHooks,
    player_movement::{MovementEnvironment, TeleportCommand},
    player_stats::StatKey,
};

pub(super) fn player_sound(
    registry: &EntityRegistry,
    runtime: &mut RuntimeEntities,
    events: &broadcast::Sender<PlayerEvent>,
    s: &EntityPhysicalState,
    key: &str,
    volume: f32,
    pitch: f32,
) -> Result<()> {
    let id = player_body::sound_id(key)?;
    let seed = runtime.next_sound_seed()?;
    let p = s.position_vec();
    let mut w = Writer::new();
    w.varint(id + 1)
        .varint(7)
        .i32((p.x * 8.) as i32)
        .i32((p.y * 8.) as i32)
        .i32((p.z * 8.) as i32)
        .f32(volume)
        .f32(pitch)
        .i64(seed);
    let radius = f64::from(if volume > 1. { volume * 16. } else { 16. });
    let recipients = registry
        .snapshot()
        .into_iter()
        .filter(|q| q.entity_id != s.entity_id)
        .filter(|q| {
            Vec3::new(q.position.x, q.position.y, q.position.z)
                .subtract(p)
                .length_squared()
                < radius * radius
        })
        .map(|q| q.entity_id)
        .collect::<Vec<_>>();
    if !recipients.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(recipients),
            packets: Arc::new(vec![Packet {
                id: 0x68,
                payload: w.into_inner(),
            }]),
        });
    }
    Ok(())
}
pub(super) struct BodyHooks<'a> {
    id: i32,
    uuid: [u8; 16],
    players: &'a mut LivePlayers,
    events: &'a broadcast::Sender<PlayerEvent>,
    region: Option<main_chunk_runtime::CurrentChunkQueryRegion>,
    frame: PlayerMotionFrame,
    border: CollisionBorder,
    colliders: &'a [CollisionEntity],
    step: &'a mut motion_host::effects::PlayerStepState,
    starting_to_fall: &'a mut Option<Vec3>,
    body: Option<&'a mut PlayerBodyState>,
    pub(super) chunk_source_requested: bool,
}
impl<'a> BodyHooks<'a> {
    pub(super) fn for_packet(
        players: &'a mut LivePlayers,
        events: &'a broadcast::Sender<PlayerEvent>,
        id: i32,
        uuid: [u8; 16],
        region: Option<main_chunk_runtime::CurrentChunkQueryRegion>,
        frame: PlayerMotionFrame,
        border: CollisionBorder,
        colliders: &'a [CollisionEntity],
        step: &'a mut motion_host::effects::PlayerStepState,
        starting_to_fall: &'a mut Option<Vec3>,
    ) -> Self {
        Self {
            id,
            uuid,
            players,
            events,
            region,
            frame,
            border,
            colliders,
            step,
            starting_to_fall,
            body: None,
            chunk_source_requested: false,
        }
    }

    fn motion<T>(
        &mut self,
        f: impl FnOnce(&mut motion_host::MainMotionHooks<'_>) -> Result<T>,
    ) -> Result<T> {
        let LivePlayers {
            registry,
            gameplay,
            chunk_runtime,
            item_runtime,
            player_random,
            world_interactions,
            ..
        } = &mut *self.players;
        f(&mut motion_host::MainMotionHooks {
            id: self.id,
            uuid: self.uuid,
            registry,
            gameplay,
            manager: &chunk_runtime.manager,
            region: self.region,
            fluid_shapes: &mut world_interactions.fluid_shapes,
            runtime: item_runtime,
            entity_random: player_random
                .get_mut(&self.id)
                .context("native body RNG missing")?,
            events: self.events,
            step: self.step,
            starting_to_fall: self.starting_to_fall,
            chunk_source_requested: false,
        })
    }
    fn food(&mut self, phase: PlayerBodyPhase, m: &PlayerMotion) -> Result<()> {
        let facts = main_player_food::MainFoodFacts {
            difficulty: self.players.native_difficulty,
            natural_regeneration: self
                .players
                .world_context
                .rules
                .boolean("naturalRegeneration")
                .context("native body naturalRegeneration rule missing")?,
            tick_count: m.physical.tick_count,
            max_health: 20.,
        };
        let LivePlayers {
            registry,
            gameplay,
            player_lifecycle,
            player_food_runtime,
            ..
        } = &mut *self.players;
        let lifecycle = player_lifecycle
            .get_mut(&self.uuid)
            .context("native body lifecycle missing")?;
        match phase {
            PlayerBodyPhase::PeacefulFood => player_food_runtime.peaceful_ai_step(
                self.id,
                lifecycle,
                facts,
                main_player_food::MainFoodServices {
                    registry,
                    gameplay,
                    events: self.events,
                },
            ),
            _ => {
                player_food_runtime.player_tick_food_and_stats(
                    self.id,
                    lifecycle,
                    facts,
                    main_player_food::MainFoodServices {
                        registry,
                        gameplay,
                        events: self.events,
                    },
                )?;
                player_food_runtime.send_health_changes(
                    self.id,
                    lifecycle,
                    facts,
                    main_player_food::MainFoodServices {
                        registry,
                        gameplay,
                        events: self.events,
                    },
                )?;
                Ok(())
            }
        }
    }
    fn push_players(&mut self, m: &mut PlayerMotion) -> Result<()> {
        let registry = &self.players.registry;
        let handles =
            self.players
                .item_runtime
                .native_body_query(m.physical.bounds, self.id, |h| {
                    let Some(p) = registry.get(h.id) else {
                        return Ok(None);
                    };
                    if p.gamemode == 3 {
                        return Ok(None);
                    }
                    let d = player_physics::player_dimensions(p.metadata.pose, 1.)?;
                    Ok(Some(Aabb::player(
                        Vec3::new(p.position.x, p.position.y, p.position.z),
                        d.width,
                        d.height,
                    )))
                })?;
        // Native profiles have no teams, vehicles, passengers or non-player pushable
        // bodies. Exact section order above is retained, including the cramming RNG.
        let limit = self
            .players
            .world_context
            .rules
            .integer("maxEntityCramming")
            .context("maxEntityCramming rule missing")?;
        if limit > 0 && handles.len() > (limit - 1) as usize {
            self.players
                .player_random
                .get_mut(&self.id)
                .context("body RNG missing")?
                .next_bounded_i32(4)?;
        }
        for h in handles {
            let Some(other) = self.players.player_motion_runtime.players.get_mut(&h.id) else {
                bail!("pushable player lacks native physical owner")
            };
            if other.motion.physical.no_physics || m.physical.no_physics {
                continue;
            }
            let mut dx = m.physical.position.x - other.motion.physical.position.x;
            let mut dz = m.physical.position.z - other.motion.physical.position.z;
            let length = dx.abs().max(dz.abs());
            if length < 0.009999999776482582 {
                continue;
            }
            let root = length.sqrt();
            dx /= root;
            dz /= root;
            let factor = (1. / root).min(1.);
            dx *= factor;
            dz *= factor;
            dx *= 0.05000000074505806;
            dz *= 0.05000000074505806;
            other.motion.physical.velocity.x -= dx;
            other.motion.physical.velocity.z -= dz;
            other.motion.physical.has_impulse = true;
            m.physical.velocity.x += dx;
            m.physical.velocity.z += dz;
            m.physical.has_impulse = true;
            if let Some(life) = self
                .players
                .player_lifecycle
                .get_mut(&other.motion.physical.uuid)
            {
                life.motion_velocity = Some(other.motion.physical.velocity);
            }
        }
        Ok(())
    }
}
impl PlayerMoveHooks for BodyHooks<'_> {
    fn owns_player_body(&self) -> bool {
        true
    }
    fn fluid_ray_shape(
        &mut self,
        w: &mut WorldBackend,
        p: entity_move::BlockPos,
        f: mc121_rust::fluid_physics::FluidState,
    ) -> Result<geometry::VoxelShape> {
        self.motion(|h| h.fluid_ray_shape(w, p, f))
    }
    fn block_entity_inside(
        &mut self,
        w: &mut WorldBackend,
        s: &mut EntityPhysicalState,
        p: entity_move::BlockPos,
        b: u32,
    ) -> Result<()> {
        if self.players.redstone_entity_inside_with_body(
            w,
            p,
            s,
            self.frame.game_time,
            self.border,
            self.events,
        )? {
            return Ok(());
        }
        self.motion(|h| h.block_entity_inside(w, s, p, b))
    }
    fn fire_extinguished(&mut self, s: &EntityPhysicalState) -> Result<()> {
        self.motion(|h| h.fire_extinguished(s))
    }
    fn position_changed(&mut self, s: &EntityPhysicalState) -> Result<()> {
        self.motion(|h| h.position_changed(s))
    }
    fn fall_distance_reset(&mut self, s: &EntityPhysicalState) -> Result<()> {
        self.motion(|h| h.fall_distance_reset(s))
    }
    fn profile(&mut self, e: ProfileEvent) {
        self.players
            .gameplay
            .frame(
                &self.players.registry,
                &self.players.chunk_runtime.manager,
                true,
            )
            .profile(e)
    }
    fn walking_step_sound(
        &mut self,
        w: &mut WorldBackend,
        s: &EntityPhysicalState,
        p: entity_move::BlockPos,
        b: u32,
    ) -> Result<()> {
        self.motion(|h| h.walking_step_sound(w, s, p, b))
    }
    fn step_game_event(&mut self, s: &EntityPhysicalState, b: u32) -> Result<()> {
        self.motion(|h| h.step_game_event(s, b))
    }
    fn inside_block(&mut self, s: &EntityPhysicalState, b: u32) -> Result<()> {
        self.motion(|h| h.inside_block(s, b))
    }
}
impl PlayerMotionHooks for BodyHooks<'_> {
    fn send_teleport(&mut self, c: TeleportCommand) -> Result<()> {
        self.motion(|h| h.send_teleport(c))
    }
    fn acknowledge_block_changes(&mut self, n: i32) -> Result<()> {
        self.motion(|h| h.acknowledge_block_changes(n))
    }
    fn move_chunk_source(&mut self, s: &EntityPhysicalState) -> Result<()> {
        self.motion(|h| h.move_chunk_source(s))?;
        self.chunk_source_requested = true;
        Ok(())
    }
    fn award_stat(&mut self, id: i32, k: StatKey, n: i32) -> Result<()> {
        self.motion(|h| h.award_stat(id, k, n))
    }
    fn hit_ground(&mut self, s: &EntityPhysicalState, b: u32) -> Result<()> {
        self.motion(|h| h.hit_ground(s, b))
    }
    fn on_changed_block(
        &mut self,
        w: &mut WorldBackend,
        s: &EntityPhysicalState,
        p: entity_move::BlockPos,
    ) -> Result<()> {
        self.motion(|h| h.on_changed_block(w, s, p))
    }
    fn fall_particles(&mut self, s: &EntityPhysicalState, p: FallParticles) -> Result<()> {
        self.motion(|h| h.fall_particles(s, p))
    }
}
impl PlayerBodyWorldHooks for BodyHooks<'_> {
    fn body_minimum_build_height(&self) -> i32 {
        self.players.world_context.dimension.min_y
    }
    fn body_ultrawarm(&self) -> bool {
        self.players.world_context.dimension.ultrawarm
    }
    fn body_next_float(&mut self) -> Result<f32> {
        Ok(self
            .players
            .player_random
            .get_mut(&self.id)
            .context("body entity RNG absent")?
            .next_f32())
    }
    fn body_next_double(&mut self) -> Result<f64> {
        Ok(self
            .players
            .player_random
            .get_mut(&self.id)
            .context("body entity RNG absent")?
            .next_f64())
    }
    fn body_sound(&mut self, s: &EntityPhysicalState, key: &str, v: f32, p: f32) -> Result<()> {
        player_sound(
            &self.players.registry,
            &mut self.players.item_runtime,
            self.events,
            s,
            key,
            v,
            p,
        )
    }
    fn body_sprint_particle(
        &mut self,
        w: &mut WorldBackend,
        s: &EntityPhysicalState,
    ) -> Result<()> {
        let p = if let Some(support) = s.supporting_block {
            let f = *entity_move::block_facts(w.block(support.0, support.1, support.2)?)?;
            if f.fence() || f.wall() || f.fence_gate() {
                support
            } else {
                (
                    support.0,
                    (s.position.y - f64::from(0.2f32)).floor() as i32,
                    support.2,
                )
            }
        } else {
            (
                s.block_position.0,
                (s.position.y - f64::from(0.2f32)).floor() as i32,
                s.block_position.2,
            )
        };
        let b = w.block(p.0, p.1, p.2)?;
        if player_body::sprint_render_visible(b)? {
            self.body_next_double()?;
            self.body_next_double()?;
        }
        Ok(())
    }
    fn body_base_effect(&mut self, s: &EntityPhysicalState, e: BaseEffect) -> Result<()> {
        match e {
            BaseEffect::MeltFrozen {
                position,
                event,
                data,
            } => {
                let mut w = Writer::new();
                w.i32(event)
                    .i64(pack_position(position.0, position.1, position.2))
                    .i32(data)
                    .bool(false);
                let recipients = self
                    .players
                    .registry
                    .snapshot()
                    .into_iter()
                    .filter(|p| {
                        Vec3::new(p.position.x, p.position.y, p.position.z)
                            .subtract(s.position_vec())
                            .length_squared()
                            < 4096.
                    })
                    .map(|p| p.entity_id)
                    .collect::<Vec<_>>();
                let _ = self.events.send(PlayerEvent::WorldAudience {
                    recipients: Arc::new(recipients),
                    packets: Arc::new(vec![Packet {
                        id: 0x28,
                        payload: w.into_inner(),
                    }]),
                });
                Ok(())
            }
            _ => bail!("unported Player base effect {e:?}"),
        }
    }
    fn body_movement_speed(&self) -> Result<f32> {
        Ok(
            if self
                .players
                .registry
                .get(self.id)
                .context("body player absent")?
                .metadata
                .flags
                & 8
                != 0
            {
                0.1f32 * 1.3f32
            } else {
                0.1f32
            },
        )
    }
    fn body_flying_speed(&self, sprinting: bool, flying: bool) -> Result<f32> {
        let life = self
            .players
            .player_lifecycle
            .get(&self.uuid)
            .context("body abilities absent")?;
        Ok(if flying {
            life.abilities.flying_speed * if sprinting { 2. } else { 1. }
        } else if sprinting {
            0.025999999
        } else {
            0.02
        })
    }
}
impl PlayerBodyHooks for BodyHooks<'_> {
    fn phase(
        &mut self,
        p: PlayerBodyPhase,
        m: &mut PlayerMotion,
        w: &mut WorldBackend,
        body: &mut PlayerBodyState,
    ) -> Result<()> {
        match p {
            PlayerBodyPhase::ItemUse => self.players.tick_item_use_body(
                self.id,
                &m.physical,
                self.region.context("body use requires loaded region")?,
                self.events,
            ),
            PlayerBodyPhase::PeacefulFood | PlayerBodyPhase::FoodAndHealth => self.food(p, m),
            PlayerBodyPhase::InventoryTick => self
                .players
                .containers
                .get(&self.id)
                .context("body inventory absent")?
                .read(|menu| -> Result<()> {
                    for stack in &menu.inventory.slots()[5..] {
                        if !stack.is_empty() {
                            ensure!(
                                player_body::inventory_tick_is_default(stack.item_id),
                                "special Item.inventoryTick requires actual implementation"
                            );
                        }
                    }
                    Ok(())
                })?,
            PlayerBodyPhase::PushEntities => self.push_players(m),
            PlayerBodyPhase::Pickup => {
                self.players.gameplay.connection_item_phase(self.id)?;
                let LivePlayers {
                    registry,
                    gameplay,
                    item_runtime,
                    containers,
                    chunk_runtime,
                    ..
                } = &mut *self.players;
                let mut frame = gameplay.frame_current(
                    registry,
                    &chunk_runtime.manager,
                    true,
                    self.region.context("body pickup requires loaded region")?,
                );
                let mut host = RuntimeInventoryHost::new(registry, containers, &mut frame);
                item_runtime.player_touch(
                    PickupPlayer {
                        entity_id: self.id,
                        uuid: self.uuid,
                        bounds: m.physical.bounds,
                        alive: true,
                        spectator: false,
                        vehicle_bounds: None,
                    },
                    &mut host,
                )?;
                Ok(())
            }
            PlayerBodyPhase::FinishPlayerTick => {
                let stack = self
                    .players
                    .containers
                    .get(&self.id)
                    .context("body inventory absent")?
                    .read(|menu| menu.inventory.selected_stack(0).cloned())??;
                if body.last_main_hand.count != stack.count
                    || !body.last_main_hand.same_item_same_components(&stack)?
                {
                    if body.last_main_hand.item_id != stack.item_id {
                        body.attack_strength_ticker = 0;
                    }
                    body.last_main_hand = stack;
                }
                let mut collision = RuntimeCollision::new(
                    w,
                    CollisionActor {
                        entity_id: self.id,
                        position: m.physical.position_vec(),
                    },
                    self.colliders,
                    self.border,
                );
                let input = PlayerPoseInput {
                    shift_key_down: m.controls.shift,
                    flying: m.controls.flying,
                    spectator: false,
                    fall_flying: false,
                    swimming: m.physical.is_swimming(),
                    sleeping: false,
                    auto_spin_attack: false,
                    passenger: false,
                };
                let pose = player_physics::update_player_pose(
                    &mut collision,
                    self.id,
                    m.physical.position_vec(),
                    m.physical.pose,
                    input,
                    1.,
                    EntityCollisionContext {
                        bottom_y: m.physical.position.y,
                        descending: m.controls.shift,
                        fall_distance: m.physical.fall_distance,
                        is_falling_block: false,
                        can_walk_on_powder_snow: m.controls.can_walk_on_powder_snow,
                    },
                )?;
                m.set_pose(pose)?;
                Ok(())
            }
        }
    }
}
impl PlayerBodyTickHooks for BodyHooks<'_> {
    fn player_do_tick(&mut self, m: &mut PlayerMotion, w: &mut WorldBackend) -> Result<()> {
        let body = self.body.take().context("reentrant native Player.doTick")?;
        let colliders = self.colliders;
        let result = player_body::tick(body, m, w, colliders, self.border, self.frame, self);
        self.body = Some(body);
        result
    }
}

impl LivePlayers {
    pub(super) fn tick_native_player_body(
        &mut self,
        id: i32,
        world: &mut WorldBackend,
        game_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        let mut entry = self
            .player_motion_runtime
            .players
            .remove(&id)
            .context("native body motion owner missing")?;
        let result = (|| -> Result<()> {
            let mut player = self
                .registry
                .get(id)
                .cloned()
                .context("native body player missing")?;
            if let Some(metadata) = self.pending_metadata.get(&id) {
                player.metadata = *metadata;
            }
            let lifecycle = self
                .player_lifecycle
                .get(&player.profile.uuid)
                .context("native body lifecycle missing")?;
            motion_host::sync_presentation(&mut entry.motion, &player, lifecycle)?;
            ensure!(
                matches!(player.entity_data.get(9),Some(MetadataValue::Float(v))if *v>0.),
                "dead Player body requires real death owner"
            );
            self.containers
                .get(&id)
                .context("native body inventory missing")?
                .read(|menu| ->Result<()> {
                    motion_host::require_equipment_defaults(&menu.inventory)?;
                    ensure!(!menu.inventory.slot(5).is_some_and(|stack|!stack.is_empty()&&inventory::item_name(stack.item_id)==Some("turtle_helmet")), "turtle helmet tick requires actual active-effect owner");
                    for stack in &menu.inventory.slots()[5..] {
                        if let Some(value)=stack.component_value(12)? {
                            for attribute in value["attributes"].as_array().context("body attribute component malformed")? {
                                ensure!(attribute["typeId"].as_u64()!=Some(18), "Player body MAX_HEALTH modifiers require their actual attribute owner");
                            }
                        }
                    }
                    Ok(())
                })??;
            let b = entry
                .motion
                .physical
                .bounds
                .inflate(Vec3::new(17., 17., 17.));
            let min = (
                (b.min_x.floor() as i32).div_euclid(16),
                (b.min_z.floor() as i32).div_euclid(16),
            );
            let max = (
                (b.max_x.floor() as i32).div_euclid(16),
                (b.max_z.floor() as i32).div_euclid(16),
            );
            let region = self.chunk_runtime.certify_current_query_region(
                world,
                &mut self.chunk_transports,
                events,
                game_time,
                min,
                max,
            )?;
            let colliders = self
                .registry
                .snapshot()
                .into_iter()
                .map(|p| -> Result<_> {
                    let d = player_physics::player_dimensions(p.metadata.pose, 1.)?;
                    Ok(CollisionEntity {
                        entity_id: p.entity_id,
                        bounds: Aabb::player(
                            Vec3::new(p.position.x, p.position.y, p.position.z),
                            d.width,
                            d.height,
                        ),
                        spectator: p.gamemode == 3,
                        can_collide_with_actor: false,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let frame = PlayerMotionFrame {
                game_time,
                environment: MovementEnvironment {
                    runs_normally: true,
                    disable_elytra_movement_check: false,
                    singleplayer_owner: false,
                    flight_allowed: false,
                },
                rain_known_absent: !self.world_context.weather().is_raining(),
                dead_or_dying: false,
            };
            let mut host = BodyHooks {
                id,
                uuid: player.profile.uuid,
                players: self,
                events,
                region: Some(region),
                frame,
                border,
                colliders: &colliders,
                step: &mut entry.step,
                starting_to_fall: &mut entry.starting_to_fall,
                body: Some(&mut entry.body),
                chunk_source_requested: false,
            };
            let tick = entry
                .motion
                .listener_tick(world, &colliders, border, frame, &mut host)?;
            if let Some(reason) = tick.disconnect {
                if let Some(disconnect) = host.players.disconnects.get(&id) {
                    disconnect.send_replace(Some(reason));
                }
            }
            Ok(())
        })();
        let s = &entry.motion.physical;
        if let Some(lifecycle) = self.player_lifecycle.get_mut(&s.uuid) {
            lifecycle.motion_velocity = Some(s.velocity);
            lifecycle.fall_distance = s.fall_distance;
            lifecycle.remaining_fire_ticks = s.remaining_fire_ticks;
        }
        let sync_result = (|| -> Result<()> {
            if let Some(event) = self.registry.update_position(id, s.position)? {
                self.publish(event, events);
            }
            self.registry
                .set_entity_data(id, 0, MetadataValue::Byte(s.shared_flags), false)?;
            self.registry
                .set_entity_data(id, 1, MetadataValue::Int(s.air_supply), false)?;
            self.registry
                .set_entity_data(id, 7, MetadataValue::Int(s.ticks_frozen), false)?;
            self.registry
                .set_entity_data(id, 6, MetadataValue::Pose(i32::from(s.pose)), false)?;
            if let Some(pending) = self.pending_metadata.get_mut(&id) {
                pending.flags = s.shared_flags;
                pending.pose = s.pose;
            }
            Ok(())
        })();
        let result = result.and(sync_result);
        if result.is_err() {
            if let Some(disconnect) = self.disconnects.get(&id) {
                disconnect.send_replace(Some("disconnect.disconnected"));
            }
        }
        self.player_motion_runtime.players.insert(id, entry);
        result
    }
}
