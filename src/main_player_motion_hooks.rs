//! Native creative packet motion and its real binary-side service boundaries.
//!
//! Main owns this persistent map. The source profile is the native player
//! constructor with no active-effect producer; imported player state cannot
//! acquire it from missing effect particles. Equipment is checked on every
//! operation. Network-listener bookkeeping remains a separate capability from
//! body simulation; the production connection phase composes the real native
//! body through main_player_body::BodyHooks rather than an empty callback here.
use super::*;
use mc121_rust::{
    entity_move::{BlockPos, ProfileEvent},
    entity_state::{java_floor, EntityPhysicalState},
    equipment::{EquipmentSlot, PlayerEquipment},
    game_event_context::{self, GameEventPost},
    item_host::RuntimeGameplayHooks,
    main_player_motion::{
        FallParticles, PlayerMotion, PlayerMotionFrame, PlayerMotionHooks, PlayerMotionOutcome,
        PlayerMoveHooks, PlayerTeleportRequest,
    },
    player_movement::{
        FloatingTickContext, MovePacket, MovementEnvironment, PreMove, TeleportCause,
        TeleportCommand,
    },
    player_stats::StatKey,
};
#[path = "main_player_motion_effects.rs"]
pub(super) mod effects;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativePlayerTickCoverage {
    /// ACK, firstGood/lastGood, listener counters and floating bookkeeping.
    /// No gravity, friction, baseTick or Player.aiStep is being substituted.
    NetworkListenerClockOnly,
}
pub(super) struct NativeMotionDispatch {
    pub position: Position,
    pub teleport_confirmed: bool,
}
#[derive(Debug)]
pub(super) struct NativeMotionUnavailable;
impl std::fmt::Display for NativeMotionUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("authoritative native player movement state is unavailable")
    }
}
impl std::error::Error for NativeMotionUnavailable {}

pub(super) struct NativePlayerMotion {
    pub(super) motion: PlayerMotion,
    pub(super) step: effects::PlayerStepState,
    pub(super) starting_to_fall: Option<Vec3>,
    pub(super) body: mc121_rust::player_body::PlayerBodyState,
}
pub(super) struct MainPlayerMotionRuntime {
    pub(super) players: BTreeMap<i32, NativePlayerMotion>,
}
impl MainPlayerMotionRuntime {
    pub fn new() -> Self {
        Self {
            players: BTreeMap::new(),
        }
    }
    pub fn contains(&self, id: i32) -> bool {
        self.players.contains_key(&id)
    }
    pub fn swing(&mut self, id: i32, hand: u8) -> Result<bool> {
        self.players
            .get_mut(&id)
            .ok_or(NativeMotionUnavailable)?
            .body
            .swing(hand)
    }
    pub fn velocity(&self, id: i32) -> Option<Vec3> {
        self.players
            .get(&id)
            .map(|entry| entry.motion.physical.velocity)
    }
    pub fn fall_distance(&self, id: i32) -> Option<f32> {
        self.players
            .get(&id)
            .map(|entry| entry.motion.physical.fall_distance)
    }
    pub fn physical(&self, id: i32) -> Option<&EntityPhysicalState> {
        self.players.get(&id).map(|entry| &entry.motion.physical)
    }
    /// Call only for a player admitted from the owned native constructor/save
    /// lineage. Velocity is mandatory; old saves without Motion stay unavailable.
    pub fn register_native(
        &mut self,
        player: &PlayerState,
        lifecycle: &PlayerLifecycle,
        authoritative_velocity: Vec3,
        constructor_math: [f64; 3],
    ) -> Result<()> {
        ensure!(
            !self.contains(player.entity_id),
            "motion player already registered"
        );
        require_creative(player, lifecycle)?;
        let mut motion = PlayerMotion::new(
            player.entity_id,
            player.profile.uuid,
            player.position,
            authoritative_velocity,
            player.metadata.pose,
        )?;
        motion.physical.fire_immune_ticks = 20;
        motion.physical.invulnerable = lifecycle.abilities.invulnerable;
        if let Some(MetadataValue::Int(air)) = player.entity_data.get(1) {
            motion.physical.air_supply = *air;
        }
        if let Some(MetadataValue::Int(frozen)) = player.entity_data.get(7) {
            motion.physical.ticks_frozen = *frozen;
        }
        motion.physical.fall_distance = lifecycle.fall_distance;
        motion.physical.remaining_fire_ticks = lifecycle.remaining_fire_ticks;
        sync_presentation(&mut motion, player, lifecycle)?;
        // Main sends this initial position packet only after its initial chunk
        // sequence. Reserving the matching state does not emit a second packet.
        let initial = motion
            .listener
            .teleport(motion.transform(), TeleportCause::External);
        ensure!(
            initial.id == 1,
            "fresh listener initial teleport ID changed"
        );
        self.players.insert(
            player.entity_id,
            NativePlayerMotion {
                motion,
                step: effects::PlayerStepState::default(),
                starting_to_fall: None,
                body: mc121_rust::player_body::PlayerBodyState::from_constructor_math(
                    constructor_math,
                )?,
            },
        );
        Ok(())
    }
    pub fn remove(&mut self, id: i32) {
        self.players.remove(&id);
    }
    pub fn set_flying(&mut self, id: i32, flying: bool) -> Result<()> {
        self.players
            .get_mut(&id)
            .ok_or(NativeMotionUnavailable)?
            .motion
            .request_flying(flying);
        Ok(())
    }
    /// ServerboundUseItem changes rotation without an Entity.move, fall check
    /// or listener movement-counter update. Caller applies its source codec
    /// validation/wrapping and mirrors the same angles into the registry.
    pub fn sync_use_item_rotation(&mut self, id: i32, yaw: f32, pitch: f32) -> Result<()> {
        ensure!(
            yaw.is_finite() && pitch.is_finite(),
            "invalid use-item rotation"
        );
        let entry = self.players.get_mut(&id).ok_or(NativeMotionUnavailable)?;
        entry.motion.physical.position.yaw = yaw;
        entry.motion.physical.position.pitch = pitch;
        Ok(())
    }
    pub fn acknowledge_block_changes_up_to(&mut self, id: i32, sequence: i32) -> Result<()> {
        self.players
            .get_mut(&id)
            .ok_or(NativeMotionUnavailable)?
            .motion
            .listener
            .acknowledge_block_changes_up_to(sequence)
    }
    /// Level.tickNonPassenger's old-position/rotation and entity tick counter.
    /// Call once at the real level-player callback, never from the network clock.
    pub fn begin_level_tick(&mut self, id: i32) -> Result<()> {
        let entry = self.players.get_mut(&id).ok_or(NativeMotionUnavailable)?;
        entry.motion.physical.begin_level_tick();
        Ok(())
    }
}

pub(super) fn require_creative(player: &PlayerState, lifecycle: &PlayerLifecycle) -> Result<()> {
    lifecycle.validate()?;
    ensure!(
        player.gamemode == 1
            && lifecycle.dimension == "minecraft:overworld"
            && lifecycle.abilities.may_fly
            && lifecycle.abilities.invulnerable
            && lifecycle.abilities.instant_build,
        "native movement currently requires the owned creative overworld profile"
    );
    ensure!(
        matches!(player.entity_data.get(10),Some(MetadataValue::Particles(p)) if p.is_empty())
            && matches!(
                player.entity_data.get(11),
                Some(MetadataValue::Boolean(false))
            ),
        "native default-effect profile has changed"
    );
    Ok(())
}
pub(super) fn sync_presentation(
    motion: &mut PlayerMotion,
    player: &PlayerState,
    lifecycle: &PlayerLifecycle,
) -> Result<()> {
    require_creative(player, lifecycle)?;
    ensure!(
        motion.physical.uuid == player.profile.uuid,
        "motion identity changed"
    );
    motion.set_pose(player.metadata.pose)?;
    let mut controls = motion.controls;
    controls.flying = lifecycle.abilities.flying;
    controls.shift = player.metadata.flags & 2 != 0;
    controls.sprinting = player.metadata.flags & 8 != 0;
    controls.fall_flying = player.metadata.flags & 0x80 != 0;
    controls.sleeping = matches!(
        player.entity_data.get(14),
        Some(MetadataValue::OptionalBlockPos(Some(_)))
    );
    controls.auto_spin_attack =
        matches!(player.entity_data.get(8),Some(MetadataValue::Byte(v)) if v&4!=0);
    motion.set_controls(controls);
    motion.physical.shared_flags = player.metadata.flags;
    motion.physical.silent = matches!(
        player.entity_data.get(4),
        Some(MetadataValue::Boolean(true))
    );
    motion.physical.no_gravity = matches!(
        player.entity_data.get(5),
        Some(MetadataValue::Boolean(true))
    );
    Ok(())
}
pub(super) fn require_equipment_defaults(
    inventory: &inventory::PlayerInventory,
) -> Result<PlayerEquipment> {
    let equipment = PlayerEquipment::from_inventory(inventory)?;
    for slot in EquipmentSlot::ALL {
        let stack = equipment.get(slot);
        if let Some(value) = stack.component_value(9)? {
            ensure!(
                value["enchantments"].as_array().is_some_and(Vec::is_empty),
                "enchanted movement/location effects require their actual owner"
            );
        }
        if let Some(value) = stack.component_value(12)? {
            for attribute in value["attributes"]
                .as_array()
                .context("invalid attribute component")?
            {
                let id = attribute["typeId"]
                    .as_u64()
                    .context("invalid equipment attribute ID")?;
                let key = effects::ATTRIBUTE_KEYS
                    .get(id as usize)
                    .context("unknown equipment attribute")?;
                ensure!(
                    !matches!(id, 11 | 13 | 14 | 20 | 21 | 23 | 24 | 25 | 27 | 30),
                    "movement attribute {key} requires actual modifier/effect composition"
                );
            }
        }
    }
    Ok(equipment)
}

/// World/registry lock stays held for the whole frame, including these FIFO
/// publications and the immutable final ObserverReconcile.
pub(super) struct MainMotionHooks<'a> {
    pub(super) id: i32,
    pub(super) uuid: [u8; 16],
    pub(super) registry: &'a EntityRegistry,
    pub(super) gameplay: &'a mut MainItemGameplay,
    pub(super) manager: &'a mc121_rust::runtime_chunk_manager::RuntimeChunkManager,
    pub(super) region: Option<main_chunk_runtime::CurrentChunkQueryRegion>,
    pub(super) fluid_shapes: &'a mut mc121_rust::fluid_physics::FluidShapeCache,
    pub(super) runtime: &'a mut RuntimeEntities,
    pub(super) entity_random: &'a mut LegacyRandomSource,
    pub(super) events: &'a broadcast::Sender<PlayerEvent>,
    pub(super) step: &'a mut effects::PlayerStepState,
    pub(super) starting_to_fall: &'a mut Option<Vec3>,
    pub(super) chunk_source_requested: bool,
}
impl MainMotionHooks<'_> {
    fn validate(&self, state: &EntityPhysicalState) -> Result<()> {
        ensure!(
            state.entity_id == self.id
                && state.uuid == self.uuid
                && self
                    .registry
                    .get(self.id)
                    .is_some_and(|p| p.profile.uuid == self.uuid),
            "native player motion callback identity changed"
        );
        Ok(())
    }
    fn private(&self, packet: Packet) {
        let _ = self.events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![self.id]),
            packets: Arc::new(vec![packet]),
        });
    }
    fn post(&mut self, state: &EntityPhysicalState, name: &str) -> Result<()> {
        self.validate(state)?;
        self.gameplay.require_supported_floor()?;
        let radius = mc121_rust::world_interactions::game_event_radius(name)?;
        let mut frame = self.gameplay.frame_current(
            self.registry,
            self.manager,
            true,
            self.region
                .context("motion game event requires a current loaded region")?,
        );
        game_event_context::post_to_empty_registries(
            &mut frame,
            GameEventPost {
                position: state.position_vec(),
                notification_radius: radius,
            },
        )?;
        Ok(())
    }
}
fn emit_motion_broadcast(
    registry: &EntityRegistry,
    events: &broadcast::Sender<PlayerEvent>,
    actor: &EntityPhysicalState,
    effect: effects::PlayerMotionBroadcast,
) -> Result<()> {
    let recipients = registry
        .snapshot()
        .into_iter()
        .filter_map(|player| {
            let position = if player.entity_id == actor.entity_id {
                actor.position_vec()
            } else {
                Vec3::new(player.position.x, player.position.y, player.position.z)
            };
            effect
                .in_range(player.entity_id, position, true)
                .then_some(player.entity_id)
        })
        .collect::<Vec<_>>();
    if !recipients.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(recipients),
            packets: Arc::new(vec![effect.packet]),
        });
    }
    Ok(())
}
impl PlayerMoveHooks for MainMotionHooks<'_> {
    fn owns_player_body(&self) -> bool {
        true
    }
    fn fluid_ray_shape(
        &mut self,
        world: &mut WorldBackend,
        p: BlockPos,
        fluid: mc121_rust::fluid_physics::FluidState,
    ) -> Result<geometry::VoxelShape> {
        self.fluid_shapes.shape(
            &mut mc121_rust::fluid_physics::RuntimeFluidWorld { world },
            p,
            fluid,
        )
    }
    fn block_entity_inside(
        &mut self,
        _world: &mut WorldBackend,
        s: &mut EntityPhysicalState,
        _p: BlockPos,
        block: u32,
    ) -> Result<()> {
        let owner = &mc121_rust::entity_move::block_facts(block)?
            .callback_classes()?
            .entity_inside;
        if owner == "dfl" {
            s.remaining_fire_ticks = s.remaining_fire_ticks.wrapping_add(1).min(1);
            if s.remaining_fire_ticks == 0 {
                s.remaining_fire_ticks = 1;
            }
            Ok(())
        } else {
            ensure!(
                owner == "dtb",
                "unported player entityInside {owner} for state {block}"
            );
            Ok(())
        }
    }
    fn fire_extinguished(&mut self, s: &EntityPhysicalState) -> Result<()> {
        let pitch =
            1.6f32 + (self.entity_random.next_f32() - self.entity_random.next_f32()) * 0.4f32;
        super::main_player_body::player_sound(
            self.registry,
            self.runtime,
            self.events,
            s,
            "iU",
            0.7,
            pitch,
        )
    }
    fn position_changed(&mut self, state: &EntityPhysicalState) -> Result<()> {
        self.validate(state)?;
        // Original setPosRaw callback updates spatial storage even at clipped
        // intermediate positions. No observer snapshot is published here.
        self.runtime.move_player(self.id, state.position_vec())
    }
    fn fall_distance_reset(&mut self, state: &EntityPhysicalState) -> Result<()> {
        self.validate(state)?;
        self.gameplay.require_supported_floor()?;
        // The native advancement owner has no FALL_FROM_HEIGHT registrations.
        // ServerPlayer.resetFallDistance clears this even for zero distance.
        *self.starting_to_fall = None;
        Ok(())
    }
    fn profile(&mut self, event: ProfileEvent) {
        self.gameplay
            .frame(self.registry, self.manager, true)
            .profile(event);
    }
    fn walking_step_sound(
        &mut self,
        world: &mut WorldBackend,
        state: &EntityPhysicalState,
        position: BlockPos,
        block: u32,
    ) -> Result<()> {
        self.validate(state)?;
        let registry = self.registry;
        let events = self.events;
        let runtime = &mut *self.runtime;
        let mut effects = effects::PlayerStepEffects {
            entity_random: self.entity_random,
            next_sound_seed: || runtime.next_sound_seed(),
            emit: |effect| emit_motion_broadcast(registry, events, state, effect),
        };
        effects::walking_step_sound(world, state, position, block, self.step, &mut effects)
    }
    fn step_game_event(&mut self, state: &EntityPhysicalState, _: u32) -> Result<()> {
        self.post(state, "minecraft:step")
    }
    fn inside_block(&mut self, state: &EntityPhysicalState, block: u32) -> Result<()> {
        self.validate(state)?;
        ensure!(block < 26684, "unknown ENTER_BLOCK context");
        // Only the recipe advancements register ENTER_BLOCK listeners (boats:
        // in_water). Matching reads the state alone, with no RNG draw.
        self.gameplay.enter_block(state.uuid, block)
    }
}
impl PlayerMotionHooks for MainMotionHooks<'_> {
    fn send_teleport(&mut self, command: TeleportCommand) -> Result<()> {
        self.private(PlayerMotion::teleport_packet(command));
        Ok(())
    }
    fn acknowledge_block_changes(&mut self, sequence: i32) -> Result<()> {
        let mut out = Writer::new();
        out.varint(sequence);
        self.private(Packet {
            id: 0x05,
            payload: out.into_inner(),
        });
        Ok(())
    }
    fn move_chunk_source(&mut self, state: &EntityPhysicalState) -> Result<()> {
        self.validate(state)?;
        // Commit the final registry/chunk input together below. Empty native
        // callbacks between this point and commit do not inspect ticket input;
        // their getChunkNow provider uses the certified current FULL futures.
        self.chunk_source_requested = true;
        Ok(())
    }
    fn award_stat(&mut self, player_id: i32, key: StatKey, amount: i32) -> Result<()> {
        ensure!(player_id == self.id, "movement statistic identity changed");
        self.gameplay.require_supported_floor()?;
        self.gameplay.stats_mut(self.uuid)?.increment(key, amount)?;
        self.gameplay.award_scoreboard_stat(self.uuid, key, amount)?;
        Ok(())
    }
    fn hit_ground(&mut self, state: &EntityPhysicalState, _: u32) -> Result<()> {
        self.post(state, "minecraft:hit_ground")
    }
    fn on_changed_block(
        &mut self,
        _: &mut WorldBackend,
        state: &EntityPhysicalState,
        _: BlockPos,
    ) -> Result<()> {
        self.validate(state)?;
        // Equipment was read from the actual menu before movement. No active
        // enchantment/location-effect producer exists in this native profile.
        self.gameplay.require_supported_floor()
    }
    fn fall_particles(
        &mut self,
        state: &EntityPhysicalState,
        particles: FallParticles,
    ) -> Result<()> {
        self.validate(state)?;
        effects::fall_particles(&particles, &mut |effect| {
            emit_motion_broadcast(self.registry, self.events, state, effect)
        })
    }
}
// This storage/effect adapter deliberately has no PlayerBodyTickHooks impl.
// The production body composition lives in main_player_body::BodyHooks; an
// independently embedded movement-only host cannot substitute an empty tick.

enum MotionOperation {
    Move(MovePacket),
    Ack(i32),
    Teleport(PlayerTeleportRequest),
}
#[derive(Clone, Copy)]
struct MotionContext {
    game_time: i64,
    border: CollisionBorder,
    requested: i32,
    server_view: i32,
}
impl LivePlayers {
    fn dispatch_native_motion(
        &mut self,
        id: i32,
        operation: MotionOperation,
        world: &mut WorldBackend,
        context: MotionContext,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<NativeMotionDispatch> {
        let mut entry = self
            .player_motion_runtime
            .players
            .remove(&id)
            .ok_or(NativeMotionUnavailable)?;
        // Retain the owned state even after an original-style partial exception.
        let result =
            self.dispatch_native_motion_entry(id, operation, world, context, events, &mut entry);
        let failed_state = result.as_ref().err().map(|_| entry.motion.physical.clone());
        self.player_motion_runtime.players.insert(id, entry);
        if let Some(state) = failed_state {
            // Entity.move may already have setPosRaw/velocity/stat effects when
            // a required callback fails. Retain that prefix for disconnect/save;
            // never combine its new velocity with an old registry position.
            if let Some(event) = self.registry.update_position(id, state.position)? {
                self.publish(event, events);
            }
            if let Some(lifecycle) = self.player_lifecycle.get_mut(&state.uuid) {
                lifecycle.motion_velocity = Some(state.velocity);
                lifecycle.fall_distance = state.fall_distance;
                lifecycle.remaining_fire_ticks = state.remaining_fire_ticks;
            }
            if let Some(disconnect) = self.disconnects.get(&id) {
                disconnect.send_replace(Some("disconnect.disconnected"));
            }
        }
        result
    }
    fn dispatch_native_motion_entry(
        &mut self,
        id: i32,
        operation: MotionOperation,
        world: &mut WorldBackend,
        context: MotionContext,
        events: &broadcast::Sender<PlayerEvent>,
        entry: &mut NativePlayerMotion,
    ) -> Result<NativeMotionDispatch> {
        if let MotionOperation::Ack(ack) = operation {
            if ack != entry.motion.listener.teleport_id {
                // The original ignores an unrelated ACK before any world or
                // player-state query. It must not cause observer reconciliation.
                return Ok(NativeMotionDispatch {
                    position: entry.motion.physical.position,
                    teleport_confirmed: entry.motion.listener.pending_teleport.is_none(),
                });
            }
            if entry.motion.listener.pending_teleport.is_none() {
                if let Some(disconnect) = self.disconnects.get(&id) {
                    disconnect.send_replace(Some(mc121_rust::player_movement::INVALID_MOVEMENT));
                }
                return Ok(NativeMotionDispatch {
                    position: entry.motion.physical.position,
                    teleport_confirmed: true,
                });
            }
        }
        let mut player = self
            .registry
            .get(id)
            .cloned()
            .ok_or(NativeMotionUnavailable)?;
        // SynchedEntityData changes immediately; only its wire drain waits for
        // the tracking phase. Pending action flags are already live input here.
        if let Some(pending) = self.pending_metadata.get(&id) {
            player.metadata = *pending;
        }
        let lifecycle = self
            .player_lifecycle
            .get(&player.profile.uuid)
            .context("motion lifecycle missing")?;
        sync_presentation(&mut entry.motion, &player, lifecycle)?;
        let frame = PlayerMotionFrame {
            game_time: context.game_time,
            environment: MovementEnvironment {
                runs_normally: true,
                disable_elytra_movement_check: false,
                singleplayer_owner: false,
                flight_allowed: false,
            },
            rain_known_absent: !self.world_context.weather().is_raining(),
            dead_or_dying: matches!(player.entity_data.get(9),Some(MetadataValue::Float(v)) if *v<=0.),
        };
        let prepared = match operation {
            MotionOperation::Move(packet) => {
                Some(entry.motion.prepare_move(packet, frame.environment))
            }
            _ => None,
        };
        if let Some(PreMove::Ignore(_) | PreMove::Disconnect(_)) = prepared {
            if let Some(PreMove::Disconnect(reason)) = prepared {
                if let Some(disconnect) = self.disconnects.get(&id) {
                    disconnect.send_replace(Some(reason));
                }
            }
            return Ok(NativeMotionDispatch {
                position: entry.motion.physical.position,
                teleport_confirmed: entry.motion.listener.pending_teleport.is_none(),
            });
        }
        if matches!(prepared, Some(PreMove::Move(_))) {
            self.gameplay.require_supported_floor()?;
            let equipment = self
                .containers
                .get(&id)
                .context("motion menu missing")?
                .read(|menu| require_equipment_defaults(&menu.inventory))??;
            entry.motion.controls.can_walk_on_powder_snow = {
                let boots = equipment.get(EquipmentSlot::Feet);
                !boots.is_empty() && inventory::item_name(boots.item_id) == Some("leather_boots")
            };
        }
        let region = if matches!(prepared, Some(PreMove::Move(_))) {
            let destination = match (&prepared, &operation) {
                (Some(PreMove::Move(plan)), _) => plan.target.position,
                (Some(PreMove::Teleport(command)), _) => command.target.position,
                (_, MotionOperation::Teleport(request)) => request.target.position,
                _ => entry.motion.physical.position_vec(),
            };
            let b = entry.motion.physical.bounds;
            let delta = destination.subtract(entry.motion.physical.position_vec());
            let query = if matches!(prepared, Some(PreMove::Move(_))) {
                b.expand_towards(delta)
            } else {
                // A teleport has no collision sweep through intervening chunks.
                b.moved(delta)
            }
            .inflate(Vec3::new(17., 17., 17.));
            let min = (
                java_floor(query.min_x).div_euclid(16),
                java_floor(query.min_z).div_euclid(16),
            );
            let max = (
                java_floor(query.max_x).div_euclid(16),
                java_floor(query.max_z).div_euclid(16),
            );
            Some(self.chunk_runtime.certify_current_query_region(
                world,
                &mut self.chunk_transports,
                events,
                context.game_time,
                min,
                max,
            )?)
        } else {
            // Teleport/ACK/passenger rotation use only setPosRaw and the
            // chunk-source callback, never collision or block/fluid queries.
            None
        };
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
        let mut hooks = super::main_player_body::BodyHooks::for_packet(
            self,
            events,
            id,
            player.profile.uuid,
            region,
            frame,
            context.border,
            &colliders,
            &mut entry.step,
            &mut entry.starting_to_fall,
        );
        let outcome = match operation {
            MotionOperation::Move(_) => entry.motion.handle_prepared_move(
                prepared.expect("prepared move"),
                world,
                &colliders,
                context.border,
                frame,
                &mut hooks,
            )?,
            MotionOperation::Teleport(request) => {
                PlayerMotionOutcome::Teleport(entry.motion.teleport(
                    request,
                    world,
                    &colliders,
                    context.border,
                    frame,
                    &mut hooks,
                )?)
            }
            MotionOperation::Ack(ack) => match entry.motion.acknowledge_teleport(
                ack,
                world,
                &colliders,
                context.border,
                frame,
                &mut hooks,
            )? {
                Some(reason) => PlayerMotionOutcome::Disconnect(reason),
                None => PlayerMotionOutcome::Accepted {
                    transform: entry.motion.transform(),
                    on_ground: entry.motion.physical.on_ground(),
                },
            },
        };
        let chunk_source = hooks.chunk_source_requested;
        drop(hooks);
        let position = entry.motion.physical.position;
        if let PlayerMotionOutcome::Disconnect(reason) = outcome {
            if let Some(disconnect) = self.disconnects.get(&id) {
                disconnect.send_replace(Some(reason));
            }
        } else if !matches!(outcome, PlayerMotionOutcome::Ignored(_)) {
            if let Some(event) = self.registry.update_position(id, position)? {
                self.publish(event, events);
            }
            if chunk_source {
                let chunk = (
                    (position.x.floor() as i32).div_euclid(16),
                    (position.z.floor() as i32).div_euclid(16),
                );
                if self.player_chunks.get(&id) != Some(&chunk) {
                    if let Some(previous) = self.player_chunks.insert(id, chunk) {
                        self.chunk_runtime
                            .manager
                            .player_move(id, previous, chunk)?;
                    }
                    self.queue_chunk_view(
                        id,
                        chunk,
                        entity_tracking::player_view_distance(
                            context.requested,
                            context.server_view,
                        ),
                        true,
                        events,
                    )?;
                }
            }
            let observer =
                self.observer_for(id, position, context.requested, context.server_view)?;
            self.observers.insert(id, observer);
            self.publish_reconcile(id, events)?;
        }
        if let Some(lifecycle) = self.player_lifecycle.get_mut(&player.profile.uuid) {
            lifecycle.fall_distance = entry.motion.physical.fall_distance;
            lifecycle.motion_velocity = Some(entry.motion.physical.velocity);
            lifecycle.remaining_fire_ticks = entry.motion.physical.remaining_fire_ticks;
        }
        Ok(NativeMotionDispatch {
            position,
            teleport_confirmed: entry.motion.listener.pending_teleport.is_none(),
        })
    }
    pub(super) fn advance_native_player_listener_clock(
        &mut self,
        id: i32,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<NativePlayerTickCoverage> {
        let Some(entry) = self.player_motion_runtime.players.get_mut(&id) else {
            return Err(NativeMotionUnavailable.into());
        };
        let listener = &mut entry.motion.listener;
        if listener.pending_block_ack > -1 {
            let mut out = Writer::new();
            out.varint(listener.pending_block_ack);
            let _ = events.send(PlayerEvent::WorldAudience {
                recipients: Arc::new(vec![id]),
                packets: Arc::new(vec![Packet {
                    id: 0x05,
                    payload: out.into_inner(),
                }]),
            });
            listener.take_block_ack();
        }
        let position = entry.motion.physical.position_vec();
        listener.begin_listener_tick(position);
        entry.motion.previous_position = position;
        // No Player.doTick is called here and no body's velocity is fabricated.
        listener.finish_listener_tick();
        let dead = matches!(self.registry.get(id).and_then(|p|p.entity_data.get(9)),Some(MetadataValue::Float(v)) if *v<=0.);
        if let Some(reason) = listener.check_floating_tick(FloatingTickContext {
            sleeping: entry.motion.controls.sleeping,
            passenger: entry.motion.physical.vehicle.is_some(),
            dead_or_dying: dead,
            gravity: entry.motion.attributes.gravity,
        }) {
            if let Some(disconnect) = self.disconnects.get(&id) {
                disconnect.send_replace(Some(reason));
            }
        }
        Ok(NativePlayerTickCoverage::NetworkListenerClockOnly)
    }
}

impl Server {
    async fn native_motion_operation(
        &self,
        id: i32,
        operation: MotionOperation,
        requested: i32,
    ) -> Result<NativeMotionDispatch> {
        let game_time = self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?
            .game_time;
        let bounds = self
            .border
            .read()
            .map_err(|_| anyhow!("world border lock poisoned"))?
            .bounds(self.epoch.elapsed().as_millis() as i64);
        let border = CollisionBorder {
            min_x: bounds.min_x,
            min_z: bounds.min_z,
            max_x: bounds.max_x,
            max_z: bounds.max_z,
        };
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        players.dispatch_native_motion(
            id,
            operation,
            &mut world,
            MotionContext {
                game_time,
                border,
                requested,
                server_view: self.options.view_distance,
            },
            &self.entity_events,
        )
    }
    pub(super) async fn handle_player_motion(
        &self,
        id: i32,
        packet: MovePacket,
        requested: i32,
    ) -> Result<NativeMotionDispatch> {
        self.native_motion_operation(id, MotionOperation::Move(packet), requested)
            .await
    }
    pub(super) async fn acknowledge_player_motion_teleport(
        &self,
        id: i32,
        ack: i32,
        requested: i32,
    ) -> Result<NativeMotionDispatch> {
        self.native_motion_operation(id, MotionOperation::Ack(ack), requested)
            .await
    }
    pub(super) async fn teleport_player_motion(
        &self,
        id: i32,
        request: PlayerTeleportRequest,
        requested: i32,
    ) -> Result<NativeMotionDispatch> {
        self.native_motion_operation(id, MotionOperation::Teleport(request), requested)
            .await
    }
    /// Call once before acquiring another player-registry lock in the caller.
    pub(super) fn tick_player_motion_network_clock(&self) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let ids = players
            .connection_admissions
            .values()
            .copied()
            .collect::<Vec<_>>();
        for id in ids {
            if players.player_motion_runtime.contains(id) {
                players.advance_native_player_listener_clock(id, &self.entity_events)?;
            }
        }
        Ok(())
    }
}
