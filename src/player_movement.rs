//! Minecraft Java 1.21 movement-listener decisions and explicit host operations.
//!
//! `prepare` does not simulate Entity.move. The host executes the requested jump
//! and real movement, captures the collision-below flag AFTER jumping but BEFORE
//! movement, then calls `postcheck`. Accepted movement must next execute moveTo
//! before `finish_acceptance`. The returned effects retain the original order;
//! fall/statistics and impulse queries intentionally require live host state.
//!
//! No missing entity, world, fluid, damage or game-mode context is replaced with
//! an empty/default answer. `PhysicsChecks` bridges the tested geometric queries;
//! the rest of Entity.move and the player tick remain explicit host work.
use crate::{
    geometry::{Aabb, EntityCollisionContext, Vec3},
    player_physics::{self, CollisionBody, CollisionQuery, WorldCollision},
};
use anyhow::Result;

pub const INVALID_MOVEMENT: &str = "multiplayer.disconnect.invalid_player_movement";
pub const FLYING: &str = "multiplayer.disconnect.flying";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotation {
    pub yaw: f32,
    pub pitch: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Rotation,
}
/// The four serverbound movement variants differ in field presence. Validation
/// uses zero for absent packet fields; resolution afterwards uses current values.
#[derive(Clone, Copy, Debug)]
pub struct MovePacket {
    pub position: Option<Vec3>,
    pub rotation: Option<Rotation>,
    pub on_ground: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerFlags {
    pub won_game: bool,
    pub passenger: bool,
    pub sleeping: bool,
    pub changing_dimension: bool,
    pub fall_flying: bool,
    pub game_mode: GameMode,
    pub no_physics: bool,
    pub may_fly: bool,
    pub levitating: bool,
    pub auto_spin_attack: bool,
    /// Entity.isSpectator is queried separately from the game-mode enum when
    /// resetting impulse context; the host supplies its value at that stage.
    pub spectator: bool,
    pub landed_in_liquid: bool,
    pub on_climbable: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct PlayerSnapshot {
    pub transform: Transform,
    pub bounds: Aabb,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub flags: PlayerFlags,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct MovementEnvironment {
    pub runs_normally: bool,
    pub disable_elytra_movement_check: bool,
    pub singleplayer_owner: bool,
    pub flight_allowed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingTeleport {
    pub id: i32,
    pub position: Vec3,
}
#[derive(Clone, Debug)]
pub struct MovementState {
    pub first_good: Vec3,
    pub last_good: Vec3,
    pub received_move_packets: i32,
    pub known_move_packets: i32,
    pub tick_count: i32,
    pub awaiting_teleport_time: i32,
    pub teleport_id: i32,
    pub pending_teleport: Option<PendingTeleport>,
    pub client_is_floating: bool,
    pub above_ground_ticks: i32,
    pub pending_block_ack: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleportCause {
    External,
    PendingResend,
    Sleeping,
    MovedTooQuickly,
    CollisionRollback,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportCommand {
    pub target: Transform,
    /// Clientbound position/rotation fields, relative to current values only
    /// where relative_flags requests it. The host moves to absolute target.
    pub wire_transform: Transform,
    pub relative_flags: u8,
    pub id: i32,
    pub cause: TeleportCause,
    /// Rollback alone follows teleport with doCheckFallDamage, calculated from
    /// the live post-teleport position minus this origin, and packet onGround.
    pub fall_check: Option<(Vec3, bool)>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgnoreReason {
    WonGame,
    PendingTeleport,
    Sleeping,
}
#[derive(Clone, Copy, Debug)]
pub struct MovePlan {
    pub original: Transform,
    pub old_bounds: Aabb,
    pub target: Transform,
    pub movement: Vec3,
    pub packet_on_ground: bool,
    pub jump_from_ground: bool,
    pub was_fall_flying: bool,
}
#[derive(Clone, Copy, Debug)]
pub enum PreMove {
    Disconnect(&'static str),
    Ignore(IgnoreReason),
    Teleport(TeleportCommand),
    /// Execute player.moveTo at the current position with this rotation, then
    /// chunkSource.move. No movement counters or lastGood update occur.
    Passenger(Transform),
    Move(MovePlan),
}
#[derive(Clone, Copy, Debug)]
pub struct MoveObservation {
    pub player: PlayerSnapshot,
    pub vertical_collision_below_before_move: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct AcceptedMove {
    pub plan: MovePlan,
    pub moved_wrongly: bool,
    pub vertical_collision_below_before_move: bool,
}
#[derive(Clone, Copy, Debug)]
pub enum PostMove {
    Rollback {
        teleport: TeleportCommand,
        moved_wrongly: bool,
    },
    /// The host now executes player.moveTo(plan.target), then finish_acceptance.
    Accept(AcceptedMove),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MovementEffect {
    MoveChunkSource,
    SetOnGroundWithMovement {
        on_ground: bool,
        movement: Vec3,
    },
    /// Read current position at execution time, subtract origin, then check.
    CheckFallDamageFrom {
        origin: Vec3,
        on_ground: bool,
    },
    SetKnownMovement(Vec3),
    ResetFallDistance,
    /// Evaluate current landed-in-liquid / climbable / spectator facts here,
    /// together with these already captured values. Reset if any is true.
    ResetImpulseIfGroundedOrSpecial {
        packet_on_ground: bool,
        was_fall_flying: bool,
        auto_spin_attack: bool,
    },
    /// Read current position at execution time, subtract origin, then record.
    CheckMovementStatisticsFrom {
        origin: Vec3,
    },
    /// Read current position after every previous effect and commit lastGood.
    CommitLastGood,
}
#[derive(Clone, Debug)]
pub struct AcceptedEffects {
    pub client_is_floating: bool,
    pub effects: Vec<MovementEffect>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TeleportAck {
    Ignore,
    Disconnect(&'static str),
    Accepted {
        target: Transform,
        finish_dimension_change: bool,
    },
}
#[derive(Clone, Copy, Debug)]
pub struct FloatingTickContext {
    pub sleeping: bool,
    pub passenger: bool,
    pub dead_or_dying: bool,
    pub gravity: f64,
}
impl TeleportCommand {
    pub fn payload(&self) -> Vec<u8> {
        let mut w = crate::protocol::Writer::new();
        w.f64(self.wire_transform.position.x)
            .f64(self.wire_transform.position.y)
            .f64(self.wire_transform.position.z)
            .f32(self.wire_transform.rotation.yaw)
            .f32(self.wire_transform.rotation.pitch)
            .u8(self.relative_flags)
            .varint(self.id);
        w.into_inner()
    }
    pub fn packet(&self) -> Vec<u8> {
        let mut packet = vec![0x40];
        packet.extend(self.payload());
        packet
    }
}
pub fn block_ack_packet(sequence: i32) -> Vec<u8> {
    let mut w = crate::protocol::Writer::new();
    w.varint(0x05).varint(sequence);
    w.into_inner()
}
pub fn maximum_floating_ticks(gravity: f64) -> i32 {
    if gravity < 9.999999747378752e-6 {
        return i32::MAX;
    }
    let ratio = 0.08 / gravity;
    let max = if ratio.is_nan() || ratio > 1. {
        ratio
    } else {
        1.
    };
    let value = 80. * max;
    let integer = value as i32;
    if value > integer as f64 {
        integer.wrapping_add(1)
    } else {
        integer
    }
}
/// Every method is called lazily in the original short-circuit order.
/// Providers may fail when required world data is unavailable.
pub trait MovementChecks {
    fn old_bounds_clear(&mut self, bounds: Aabb) -> Result<bool>;
    fn new_collision(
        &mut self,
        old_bounds: Aabb,
        current_bounds: Aabb,
        current_position: Vec3,
        target: Vec3,
    ) -> Result<bool>;
    fn no_blocks_around(&mut self, current_bounds: Aabb) -> Result<bool>;
}
pub struct PhysicsChecks<'a, W: WorldCollision> {
    pub world: &'a mut W,
    pub actor_id: i32,
    pub context: EntityCollisionContext,
}
impl<W: WorldCollision> MovementChecks for PhysicsChecks<'_, W> {
    fn old_bounds_clear(&mut self, bounds: Aabb) -> Result<bool> {
        player_physics::no_collision(
            self.world,
            CollisionQuery {
                actor_id: self.actor_id,
                bounds,
                context: self.context,
            },
        )
    }
    fn new_collision(
        &mut self,
        old_bounds: Aabb,
        current_bounds: Aabb,
        current_position: Vec3,
        target: Vec3,
    ) -> Result<bool> {
        player_physics::colliding_with_anything_new(
            self.world,
            CollisionBody {
                actor_id: self.actor_id,
                bounds: current_bounds,
                context: self.context,
                step_height: 0.,
                on_ground: false,
            },
            current_position,
            old_bounds,
            target,
        )
    }
    fn no_blocks_around(&mut self, current_bounds: Aabb) -> Result<bool> {
        self.world
            .all_blocks_air(player_physics::floating_block_query_bounds(current_bounds))
    }
}
impl MovementState {
    pub fn new(position: Vec3) -> Self {
        Self {
            first_good: position,
            last_good: position,
            received_move_packets: 0,
            known_move_packets: 0,
            tick_count: 0,
            awaiting_teleport_time: 0,
            teleport_id: 0,
            pending_teleport: None,
            client_is_floating: false,
            above_ground_ticks: 0,
            pending_block_ack: -1,
        }
    }
    pub fn reset_position(&mut self, position: Vec3) {
        self.first_good = position;
        self.last_good = position;
    }
    /// Call before player.doTick. The original listener restores this captured
    /// firstGood position after doTick, preserving the then-current rotation.
    pub fn begin_listener_tick(&mut self, position: Vec3) -> Vec3 {
        self.reset_position(position);
        self.first_good
    }
    /// Call after player.doTick and the position restoration. Floating-kick,
    /// vehicle, chat, keepalive and idle handling remain separate host work.
    pub fn finish_listener_tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        self.known_move_packets = self.received_move_packets;
    }
    /// At the beginning of the listener tick, before resetPosition/player.doTick.
    pub fn take_block_ack(&mut self) -> Option<i32> {
        if self.pending_block_ack > -1 {
            let sequence = self.pending_block_ack;
            self.pending_block_ack = -1;
            Some(sequence)
        } else {
            None
        }
    }
    pub fn acknowledge_block_changes_up_to(&mut self, sequence: i32) -> Result<()> {
        if sequence < 0 {
            anyhow::bail!("Expected packet sequence nr >= 0");
        }
        self.pending_block_ack = self.pending_block_ack.max(sequence);
        Ok(())
    }
    /// Run after finish_listener_tick, before vehicle/common listener work.
    /// Returning FLYING means the original tick returns immediately as well.
    pub fn check_floating_tick(&mut self, context: FloatingTickContext) -> Option<&'static str> {
        if self.client_is_floating
            && !context.sleeping
            && !context.passenger
            && !context.dead_or_dying
        {
            self.above_ground_ticks = self.above_ground_ticks.wrapping_add(1);
            if self.above_ground_ticks > maximum_floating_ticks(context.gravity) {
                return Some(FLYING);
            }
        } else {
            self.client_is_floating = false;
            self.above_ground_ticks = 0;
        }
        None
    }
    pub fn teleport(&mut self, target: Transform, cause: TeleportCause) -> TeleportCommand {
        self.teleport_inner(target, target, 0, cause)
    }
    pub fn teleport_with_relative_flags(
        &mut self,
        target: Transform,
        current: Transform,
        relative_flags: u8,
        cause: TeleportCause,
    ) -> Result<TeleportCommand> {
        if relative_flags & !0x1f != 0 {
            anyhow::bail!("unknown 1.21 relative teleport flags");
        }
        Ok(self.teleport_inner(target, current, relative_flags, cause))
    }
    fn teleport_inner(
        &mut self,
        target: Transform,
        current: Transform,
        relative_flags: u8,
        cause: TeleportCause,
    ) -> TeleportCommand {
        self.teleport_id = self.teleport_id.wrapping_add(1);
        if self.teleport_id == i32::MAX {
            self.teleport_id = 0;
        }
        self.pending_teleport = Some(PendingTeleport {
            id: self.teleport_id,
            position: target.position,
        });
        self.awaiting_teleport_time = self.tick_count;
        TeleportCommand {
            target,
            wire_transform: Transform {
                position: Vec3::new(
                    target.position.x
                        - if relative_flags & 1 != 0 {
                            current.position.x
                        } else {
                            0.
                        },
                    target.position.y
                        - if relative_flags & 2 != 0 {
                            current.position.y
                        } else {
                            0.
                        },
                    target.position.z
                        - if relative_flags & 4 != 0 {
                            current.position.z
                        } else {
                            0.
                        },
                ),
                rotation: Rotation {
                    yaw: target.rotation.yaw
                        - if relative_flags & 8 != 0 {
                            current.rotation.yaw
                        } else {
                            0.
                        },
                    pitch: target.rotation.pitch
                        - if relative_flags & 16 != 0 {
                            current.rotation.pitch
                        } else {
                            0.
                        },
                },
            },
            relative_flags,
            id: self.teleport_id,
            cause,
            fall_check: None,
        }
    }
    pub fn acknowledge_teleport(
        &mut self,
        id: i32,
        current_rotation: Rotation,
        changing_dimension: bool,
    ) -> TeleportAck {
        if id != self.teleport_id {
            return TeleportAck::Ignore;
        }
        let Some(pending) = self.pending_teleport else {
            return TeleportAck::Disconnect(INVALID_MOVEMENT);
        };
        self.last_good = pending.position;
        self.pending_teleport = None;
        TeleportAck::Accepted {
            target: Transform {
                position: pending.position,
                rotation: current_rotation,
            },
            finish_dimension_change: changing_dimension,
        }
    }
    pub fn prepare(
        &mut self,
        packet: MovePacket,
        player: PlayerSnapshot,
        env: MovementEnvironment,
    ) -> PreMove {
        let raw_position = packet.position.unwrap_or(Vec3::ZERO);
        let raw_rotation = packet.rotation.unwrap_or_default();
        if player_physics::contains_invalid_values(
            raw_position,
            raw_rotation.yaw,
            raw_rotation.pitch,
        ) {
            return PreMove::Disconnect(INVALID_MOVEMENT);
        }
        if player.flags.won_game {
            return PreMove::Ignore(IgnoreReason::WonGame);
        }
        if self.tick_count == 0 {
            self.reset_position(player.transform.position);
        }
        if let Some(pending) = self.pending_teleport {
            if self.tick_count.wrapping_sub(self.awaiting_teleport_time) > 20 {
                return PreMove::Teleport(self.teleport(
                    Transform {
                        position: pending.position,
                        rotation: player.transform.rotation,
                    },
                    TeleportCause::PendingResend,
                ));
            }
            return PreMove::Ignore(IgnoreReason::PendingTeleport);
        }
        self.awaiting_teleport_time = self.tick_count;
        let rotation = packet.rotation.unwrap_or(player.transform.rotation);
        let target = Transform {
            position: player_physics::clamp_position(
                packet.position.unwrap_or(player.transform.position),
            ),
            rotation: Rotation {
                yaw: player_physics::wrap_rotation(rotation.yaw),
                pitch: player_physics::wrap_rotation(rotation.pitch),
            },
        };
        if player.flags.passenger {
            return PreMove::Passenger(Transform {
                position: player.transform.position,
                rotation: target.rotation,
            });
        }
        let from_first = target.position.subtract(self.first_good);
        let distance_squared = from_first.length_squared();
        if player.flags.sleeping {
            return if distance_squared > 1. {
                PreMove::Teleport(self.teleport(
                    Transform {
                        position: player.transform.position,
                        rotation: target.rotation,
                    },
                    TeleportCause::Sleeping,
                ))
            } else {
                PreMove::Ignore(IgnoreReason::Sleeping)
            };
        }
        if env.runs_normally {
            self.received_move_packets = self.received_move_packets.wrapping_add(1);
            let raw_multiplier = self
                .received_move_packets
                .wrapping_sub(self.known_move_packets);
            let multiplier = if raw_multiplier > 5 {
                1
            } else {
                raw_multiplier
            };
            if !player.flags.changing_dimension
                && !(env.disable_elytra_movement_check && player.flags.fall_flying)
            {
                let threshold = (if player.flags.fall_flying {
                    300_f32
                } else {
                    100_f32
                }) * multiplier as f32;
                if distance_squared - player.velocity.length_squared() > threshold as f64
                    && !env.singleplayer_owner
                {
                    return PreMove::Teleport(
                        self.teleport(player.transform, TeleportCause::MovedTooQuickly),
                    );
                }
            }
        }
        let movement = target.position.subtract(self.last_good);
        PreMove::Move(MovePlan {
            original: player.transform,
            old_bounds: player.bounds,
            target,
            movement,
            packet_on_ground: packet.on_ground,
            jump_from_ground: player.on_ground && !packet.on_ground && movement.y > 0.,
            was_fall_flying: player.flags.fall_flying,
        })
    }
    pub fn postcheck(
        &mut self,
        plan: MovePlan,
        observed: MoveObservation,
        checks: &mut impl MovementChecks,
    ) -> Result<PostMove> {
        let player = observed.player;
        let residual =
            player_physics::movement_residual(plan.target.position, player.transform.position);
        let moved_wrongly = !player.flags.changing_dimension
            && residual.length_squared() > 0.0625
            && !player.flags.sleeping
            && player.flags.game_mode != GameMode::Creative
            && player.flags.game_mode != GameMode::Spectator;
        if !player.flags.no_physics && !player.flags.sleeping {
            let rollback = (moved_wrongly && checks.old_bounds_clear(plan.old_bounds)?)
                || checks.new_collision(
                    plan.old_bounds,
                    player.bounds,
                    player.transform.position,
                    plan.target.position,
                )?;
            if rollback {
                let mut teleport = self.teleport(
                    Transform {
                        position: plan.original.position,
                        rotation: plan.target.rotation,
                    },
                    TeleportCause::CollisionRollback,
                );
                teleport.fall_check = Some((plan.original.position, plan.packet_on_ground));
                return Ok(PostMove::Rollback {
                    teleport,
                    moved_wrongly,
                });
            }
        }
        Ok(PostMove::Accept(AcceptedMove {
            plan,
            moved_wrongly,
            vertical_collision_below_before_move: observed.vertical_collision_below_before_move,
        }))
    }
    /// Call only after the host has executed moveTo(accepted.plan.target).
    /// This stage performs the floating-air query, then yields ordered effects.
    pub fn finish_acceptance(
        &mut self,
        accepted: AcceptedMove,
        final_player: PlayerSnapshot,
        env: MovementEnvironment,
        checks: &mut impl MovementChecks,
    ) -> Result<AcceptedEffects> {
        let plan = accepted.plan;
        let flags = final_player.flags;
        let auto_spin_attack = flags.auto_spin_attack;
        let floating = plan.movement.y >= -0.03125
            && !accepted.vertical_collision_below_before_move
            && flags.game_mode != GameMode::Spectator
            && !env.flight_allowed
            && !flags.may_fly
            && !flags.levitating
            && !plan.was_fall_flying
            && !auto_spin_attack
            && checks.no_blocks_around(final_player.bounds)?;
        self.client_is_floating = floating;
        let movement = final_player
            .transform
            .position
            .subtract(plan.original.position);
        let mut effects = vec![
            MovementEffect::MoveChunkSource,
            MovementEffect::SetOnGroundWithMovement {
                on_ground: plan.packet_on_ground,
                movement,
            },
            MovementEffect::CheckFallDamageFrom {
                origin: plan.original.position,
                on_ground: plan.packet_on_ground,
            },
            MovementEffect::SetKnownMovement(movement),
        ];
        if plan.movement.y > 0. {
            effects.push(MovementEffect::ResetFallDistance);
        }
        effects.push(MovementEffect::ResetImpulseIfGroundedOrSpecial {
            packet_on_ground: plan.packet_on_ground,
            was_fall_flying: plan.was_fall_flying,
            auto_spin_attack,
        });
        effects.push(MovementEffect::CheckMovementStatisticsFrom {
            origin: plan.original.position,
        });
        effects.push(MovementEffect::CommitLastGood);
        Ok(AcceptedEffects {
            client_is_floating: floating,
            effects,
        })
    }
    pub fn commit_last_good(&mut self, live_position: Vec3) {
        self.last_good = live_position;
    }
}
/// Evaluate at the ResetImpulseIfGroundedOrSpecial effect, using live flags after
/// fall-damage, known-movement and reset-fall callbacks have executed.
pub fn should_reset_impulse(
    packet_on_ground: bool,
    was_fall_flying: bool,
    auto_spin_attack: bool,
    live: PlayerFlags,
) -> bool {
    packet_on_ground
        || live.landed_in_liquid
        || live.on_climbable
        || live.spectator
        || was_fall_flying
        || auto_spin_attack
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::VoxelShape;
    use serde_json::{json, Map, Value};
    fn d(v: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
    }
    fn f(v: &Value) -> f32 {
        f32::from_bits(u32::from_str_radix(v.as_str().unwrap(), 16).unwrap())
    }
    fn flag(q: &Value, name: &str) -> bool {
        q[name].as_bool().unwrap_or(false)
    }
    fn integer(q: &Value, name: &str, default: i32) -> i32 {
        q[name].as_i64().map(|v| v as i32).unwrap_or(default)
    }
    fn vec(v: &Value) -> Vec3 {
        Vec3::new(d(&v[0]), d(&v[1]), d(&v[2]))
    }
    fn rotation(v: &Value) -> Rotation {
        Rotation {
            yaw: f(&v[0]),
            pitch: f(&v[1]),
        }
    }
    fn db(v: f64) -> String {
        format!(
            "{:016x}",
            if v.is_nan() {
                0x7ff8000000000000
            } else {
                v.to_bits()
            }
        )
    }
    fn fb(v: f32) -> String {
        format!("{:08x}", if v.is_nan() { 0x7fc00000 } else { v.to_bits() })
    }
    fn vec_out(v: Vec3) -> Value {
        json!([db(v.x), db(v.y), db(v.z)])
    }
    fn rotation_out(v: Rotation) -> Value {
        json!([fb(v.yaw), fb(v.pitch)])
    }
    fn flags(q: &Value) -> PlayerFlags {
        PlayerFlags {
            won_game: flag(q, "won"),
            passenger: flag(q, "passenger"),
            sleeping: flag(q, "sleep"),
            changing_dimension: flag(q, "changing"),
            fall_flying: flag(q, "elytra"),
            game_mode: match integer(q, "game_mode", if flag(q, "spectator") { 3 } else { 0 }) {
                0 => GameMode::Survival,
                1 => GameMode::Creative,
                2 => GameMode::Adventure,
                3 => GameMode::Spectator,
                _ => panic!("oracle game mode"),
            },
            no_physics: flag(q, "no_physics"),
            may_fly: flag(q, "may_fly"),
            levitating: flag(q, "levitation"),
            auto_spin_attack: flag(q, "spin"),
            spectator: flag(q, "spectator"),
            landed_in_liquid: flag(q, "liquid"),
            on_climbable: flag(q, "climb"),
        }
    }
    struct Probe<'a> {
        query: &'a Value,
        output: Map<String, Value>,
        events: Vec<&'static str>,
    }
    impl Probe<'_> {
        fn event(&mut self, name: &'static str) {
            self.events.push(name);
        }
        fn move_to(&mut self, player: &mut PlayerSnapshot, target: Transform, record: bool) {
            if record {
                self.event("move_to");
            }
            player.bounds = player
                .bounds
                .moved(target.position.subtract(player.transform.position));
            player.transform = target;
        }
        fn teleport(&mut self, player: &mut PlayerSnapshot, command: TeleportCommand) {
            self.event("teleport");
            self.output.insert("teleport".into(),json!({"position":vec_out(command.target.position),"rotation":rotation_out(command.target.rotation)}));
            self.move_to(player, command.target, false);
            if let Some((origin, on_ground)) = command.fall_check {
                self.fall(player.transform.position.subtract(origin), on_ground);
            }
        }
        fn fall(&mut self, delta: Vec3, on_ground: bool) {
            self.event("fall");
            self.output.insert("fall_delta".into(), vec_out(delta));
            self.output.insert("fall_ground".into(), json!(on_ground));
        }
    }
    impl MovementChecks for Probe<'_> {
        fn old_bounds_clear(&mut self, _: Aabb) -> Result<bool> {
            self.event("old_no_collision");
            Ok(flag(self.query, "old_no_collision"))
        }
        fn new_collision(
            &mut self,
            old: Aabb,
            current: Aabb,
            position: Vec3,
            target: Vec3,
        ) -> Result<bool> {
            self.event("new_collision_query");
            let shrink = Vec3::new(
                player_physics::NEW_COLLISION_DEFLATION,
                player_physics::NEW_COLLISION_DEFLATION,
                player_physics::NEW_COLLISION_DEFLATION,
            );
            let target_shape =
                VoxelShape::from_aabb(current.moved(target.subtract(position)).deflate(shrink));
            let old_shape = VoxelShape::from_aabb(old.deflate(shrink));
            let mut candidates = 0;
            let mut result = false;
            if let Some(rows) = self.query["shapes"].as_array() {
                for row in rows {
                    let s = VoxelShape::from_aabb(Aabb::new(
                        d(&row[0]),
                        d(&row[1]),
                        d(&row[2]),
                        d(&row[3]),
                        d(&row[4]),
                        d(&row[5]),
                    ));
                    if s.intersects_shape(&target_shape) {
                        candidates += 1;
                        if !s.intersects_shape(&old_shape) {
                            result = true;
                        }
                    }
                }
            }
            self.output
                .insert("new_collision_candidates".into(), json!(candidates));
            Ok(result)
        }
        fn no_blocks_around(&mut self, _: Aabb) -> Result<bool> {
            self.event("air_query");
            Ok(flag(self.query, "air"))
        }
    }
    fn run(q: &Value) -> Result<Value> {
        let position = vec(&q["current"]);
        let mut player = PlayerSnapshot {
            transform: Transform {
                position,
                rotation: rotation(&q["current_rotation"]),
            },
            bounds: Aabb::new(
                position.x - 0.3,
                position.y,
                position.z - 0.3,
                position.x + 0.3,
                position.y + 1.8,
                position.z + 0.3,
            ),
            velocity: vec(&q["velocity"]),
            on_ground: flag(q, "ground"),
            flags: flags(q),
        };
        let env = MovementEnvironment {
            runs_normally: flag(q, "runs_normally"),
            disable_elytra_movement_check: flag(q, "elytra_rule"),
            singleplayer_owner: flag(q, "owner"),
            flight_allowed: flag(q, "flight_allowed"),
        };
        let mut state = MovementState::new(position);
        state.first_good = vec(&q["first_good"]);
        state.last_good = vec(&q["last_good"]);
        state.tick_count = integer(q, "tick", 1);
        state.received_move_packets = integer(q, "received", 0);
        state.known_move_packets = integer(q, "known", 0);
        state.awaiting_teleport_time = integer(q, "teleport_tick", 0);
        if let Some(v) = q.get("pending").filter(|v| !v.is_null()) {
            state.pending_teleport = Some(PendingTeleport {
                id: 0,
                position: vec(v),
            });
        }
        let p = &q["packet"];
        let kind = p["kind"].as_str().unwrap();
        let packet = MovePacket {
            position: matches!(kind, "both" | "position").then(|| vec(&p["position"])),
            rotation: matches!(kind, "both" | "rotation").then(|| rotation(&p["rotation"])),
            on_ground: flag(p, "ground"),
        };
        let mut probe = Probe {
            query: q,
            output: Map::new(),
            events: Vec::new(),
        };
        match state.prepare(packet, player, env) {
            PreMove::Disconnect(reason) => {
                assert_eq!(reason, INVALID_MOVEMENT);
                probe.event("disconnect");
                probe.output.insert(
                    "disconnect".into(),
                    json!("Invalid move player packet received"),
                );
            }
            PreMove::Ignore(_) => {}
            PreMove::Teleport(command) => probe.teleport(&mut player, command),
            PreMove::Passenger(target) => {
                probe.move_to(&mut player, target, true);
                probe.event("chunks");
            }
            PreMove::Move(plan) => {
                let mut vertical_below = flag(q, "vertical_collision_below");
                if plan.jump_from_ground {
                    probe.event("jump");
                    if q.get("jump_vertical_collision_below").is_some() {
                        vertical_below = flag(q, "jump_vertical_collision_below");
                    }
                }
                probe.event("move");
                probe
                    .output
                    .insert("move_delta".into(), vec_out(plan.movement));
                let resolved = q.get("resolved_delta").map(vec).unwrap_or(plan.movement);
                let target = Transform {
                    position: player.transform.position.plus(resolved),
                    rotation: player.transform.rotation,
                };
                probe.move_to(&mut player, target, false);
                match state.postcheck(
                    plan,
                    MoveObservation {
                        player,
                        vertical_collision_below_before_move: vertical_below,
                    },
                    &mut probe,
                )? {
                    PostMove::Rollback { teleport, .. } => probe.teleport(&mut player, teleport),
                    PostMove::Accept(accepted) => {
                        probe.move_to(&mut player, accepted.plan.target, true);
                        let effects = state.finish_acceptance(accepted, player, env, &mut probe)?;
                        for effect in effects.effects {
                            match effect {
                                MovementEffect::MoveChunkSource => probe.event("chunks"),
                                MovementEffect::SetOnGroundWithMovement {
                                    on_ground,
                                    movement,
                                } => {
                                    probe.event("set_ground");
                                    player.on_ground = on_ground;
                                    probe
                                        .output
                                        .insert("ground_delta".into(), vec_out(movement));
                                }
                                MovementEffect::CheckFallDamageFrom { origin, on_ground } => probe
                                    .fall(player.transform.position.subtract(origin), on_ground),
                                MovementEffect::SetKnownMovement(delta) => {
                                    probe.event("known_movement");
                                    probe.output.insert("known_delta".into(), vec_out(delta));
                                }
                                MovementEffect::ResetFallDistance => probe.event("reset_fall"),
                                MovementEffect::ResetImpulseIfGroundedOrSpecial {
                                    packet_on_ground,
                                    was_fall_flying,
                                    auto_spin_attack,
                                } => {
                                    if should_reset_impulse(
                                        packet_on_ground,
                                        was_fall_flying,
                                        auto_spin_attack,
                                        player.flags,
                                    ) {
                                        probe.event("reset_impulse");
                                    }
                                }
                                MovementEffect::CheckMovementStatisticsFrom { origin } => {
                                    probe.event("statistics");
                                    probe.output.insert(
                                        "statistics_delta".into(),
                                        vec_out(player.transform.position.subtract(origin)),
                                    );
                                }
                                MovementEffect::CommitLastGood => {
                                    state.commit_last_good(player.transform.position)
                                }
                            }
                        }
                    }
                }
            }
        }
        probe.output.insert("events".into(), json!(probe.events));
        probe
            .output
            .insert("position".into(), vec_out(player.transform.position));
        probe
            .output
            .insert("rotation".into(), rotation_out(player.transform.rotation));
        probe
            .output
            .insert("received".into(), json!(state.received_move_packets));
        probe
            .output
            .insert("known".into(), json!(state.known_move_packets));
        probe
            .output
            .insert("teleport_tick".into(), json!(state.awaiting_teleport_time));
        probe
            .output
            .insert("floating".into(), json!(state.client_is_floating));
        probe
            .output
            .insert("first_good".into(), vec_out(state.first_good));
        probe
            .output
            .insert("last_good".into(), vec_out(state.last_good));
        probe
            .output
            .insert("ground".into(), json!(player.on_ground));
        Ok(Value::Object(probe.output))
    }
    #[test]
    fn actual_original_handle_move_player_queries_and_callbacks() {
        let v: Value =
            serde_json::from_str(include_str!("../tools/player_movement_vectors.json")).unwrap();
        assert_eq!(
            v["oracle_inner_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let mut differences = Vec::new();
        for (i, row) in v["cases"].as_array().unwrap().iter().enumerate() {
            let actual = run(&row["query"]).unwrap();
            if actual != row["expected"] {
                if differences.len() < 8 {
                    eprintln!(
                        "case{i} {}: {}\nnative={actual}\njava={}",
                        row["category"], row["query"], row["expected"]
                    );
                }
                differences.push(i);
            }
        }
        assert!(
            differences.is_empty(),
            "{} originalhandler differences,first{:?}",
            differences.len(),
            &differences[..differences.len().min(8)]
        );
    }
    #[test]
    fn collision_data_errors_do_not_commit_position() {
        struct Missing;
        impl MovementChecks for Missing {
            fn old_bounds_clear(&mut self, _: Aabb) -> Result<bool> {
                anyhow::bail!("chunk unavailable")
            }
            fn new_collision(&mut self, _: Aabb, _: Aabb, _: Vec3, _: Vec3) -> Result<bool> {
                anyhow::bail!("chunk unavailable")
            }
            fn no_blocks_around(&mut self, _: Aabb) -> Result<bool> {
                anyhow::bail!("chunk unavailable")
            }
        }
        let mut state = MovementState::new(Vec3::ZERO);
        let player = PlayerSnapshot {
            transform: Transform {
                position: Vec3::ZERO,
                rotation: Rotation::default(),
            },
            bounds: Aabb::player(Vec3::ZERO, 0.6, 1.8),
            velocity: Vec3::ZERO,
            on_ground: false,
            flags: PlayerFlags::default(),
        };
        let PreMove::Move(plan) = state.prepare(
            MovePacket {
                position: Some(Vec3::new(1., 0., 0.)),
                rotation: None,
                on_ground: false,
            },
            player,
            MovementEnvironment::default(),
        ) else {
            panic!("expected movement")
        };
        assert!(state
            .postcheck(
                plan,
                MoveObservation {
                    player,
                    vertical_collision_below_before_move: false
                },
                &mut Missing
            )
            .is_err());
        assert_eq!(state.last_good, Vec3::ZERO);
        assert!(state.pending_teleport.is_none());
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|v| format!("{v:02x}")).collect()
    }
    fn bookkeeping(q: &Value) -> Result<Value> {
        let position = vec(&q["current"]);
        let mut player = PlayerSnapshot {
            transform: Transform {
                position,
                rotation: rotation(&q["current_rotation"]),
            },
            bounds: Aabb::new(
                position.x - 0.3,
                position.y,
                position.z - 0.3,
                position.x + 0.3,
                position.y + 1.8,
                position.z + 0.3,
            ),
            velocity: vec(&q["velocity"]),
            on_ground: flag(q, "ground"),
            flags: flags(q),
        };
        let mut state = MovementState::new(position);
        state.first_good = vec(&q["first_good"]);
        state.last_good = vec(&q["last_good"]);
        state.tick_count = integer(q, "tick", 1);
        state.received_move_packets = integer(q, "received", 0);
        state.known_move_packets = integer(q, "known", 0);
        state.awaiting_teleport_time = integer(q, "teleport_tick", 0);
        state.teleport_id = integer(q, "teleport_id", 0);
        state.above_ground_ticks = integer(q, "above_ground_ticks", 0);
        state.pending_block_ack = integer(q, "block_ack", -1);
        state.client_is_floating = flag(q, "floating");
        if let Some(v) = q.get("pending").filter(|v| !v.is_null()) {
            state.pending_teleport = Some(PendingTeleport {
                id: state.teleport_id,
                position: vec(v),
            });
        }
        let mut results = Vec::new();
        for step in q["steps"].as_array().unwrap() {
            let mut probe = Probe {
                query: q,
                output: Map::new(),
                events: Vec::new(),
            };
            match step["op"].as_str().unwrap() {
                "teleport" => {
                    let target = Transform {
                        position: vec(&step["position"]),
                        rotation: rotation(&step["rotation"]),
                    };
                    let command = state.teleport_with_relative_flags(
                        target,
                        player.transform,
                        integer(step, "relative_mask", 0) as u8,
                        TeleportCause::External,
                    )?;
                    probe.move_to(&mut player, command.target, true);
                    probe.event("send_packet");
                    probe.output.insert("packets".into(),json!([{"class":"aec","position":vec_out(command.wire_transform.position),"rotation":rotation_out(command.wire_transform.rotation),"id":command.id,"relative_mask":command.relative_flags,"wire":hex(&command.packet())}]));
                }
                "ack" => match state.acknowledge_teleport(
                    integer(step, "id", 0),
                    player.transform.rotation,
                    player.flags.changing_dimension,
                ) {
                    TeleportAck::Ignore => {}
                    TeleportAck::Disconnect(reason) => {
                        assert_eq!(reason, INVALID_MOVEMENT);
                        probe.event("disconnect");
                        probe.output.insert(
                            "disconnect".into(),
                            json!("Invalid move player packet received"),
                        );
                    }
                    TeleportAck::Accepted {
                        target,
                        finish_dimension_change,
                    } => {
                        probe.move_to(&mut player, target, true);
                        if finish_dimension_change {
                            player.flags.changing_dimension = false;
                        }
                    }
                },
                "reset" => state.reset_position(player.transform.position),
                "tick" => {
                    if let Some(sequence) = state.take_block_ack() {
                        probe.event("send_packet");
                        probe.output.insert("packets".into(),json!([{"class":"abz","sequence":sequence,"wire":hex(&block_ack_packet(sequence))}]));
                    }
                    let restore_position = state.begin_listener_tick(player.transform.position);
                    probe.event("player_tick");
                    if let Some(p) = q.get("tick_player_position") {
                        let target = Transform {
                            position: vec(p),
                            rotation: q
                                .get("tick_player_rotation")
                                .map(rotation)
                                .unwrap_or(player.transform.rotation),
                        };
                        probe.move_to(&mut player, target, false);
                    }
                    let restore = Transform {
                        position: restore_position,
                        rotation: player.transform.rotation,
                    };
                    probe.move_to(&mut player, restore, true);
                    state.finish_listener_tick();
                    let gravity = if flag(q, "no_gravity") {
                        0.
                    } else {
                        q.get("gravity").map(d).unwrap_or(0.08)
                    };
                    if let Some(reason) = state.check_floating_tick(FloatingTickContext {
                        sleeping: player.flags.sleeping,
                        passenger: player.flags.passenger,
                        dead_or_dying: flag(q, "dead"),
                        gravity,
                    }) {
                        assert_eq!(reason, FLYING);
                        probe.event("disconnect");
                        probe.output.insert(
                            "disconnect".into(),
                            json!("Flying is not enabled on this server"),
                        );
                    } else {
                        probe.event("common_tick");
                    }
                }
                other => anyhow::bail!("unexpected originalbookkeeping op{other}"),
            }
            probe.output.insert("events".into(), json!(probe.events));
            probe
                .output
                .insert("position".into(), vec_out(player.transform.position));
            probe
                .output
                .insert("rotation".into(), rotation_out(player.transform.rotation));
            probe.output.insert(
                "pending".into(),
                state
                    .pending_teleport
                    .map(|p| vec_out(p.position))
                    .unwrap_or(Value::Null),
            );
            probe.output.insert("tick".into(), json!(state.tick_count));
            probe
                .output
                .insert("received".into(), json!(state.received_move_packets));
            probe
                .output
                .insert("known".into(), json!(state.known_move_packets));
            probe
                .output
                .insert("teleport_tick".into(), json!(state.awaiting_teleport_time));
            probe
                .output
                .insert("teleport_id".into(), json!(state.teleport_id));
            probe
                .output
                .insert("above_ground_ticks".into(), json!(state.above_ground_ticks));
            probe
                .output
                .insert("block_ack".into(), json!(state.pending_block_ack));
            probe
                .output
                .insert("floating".into(), json!(state.client_is_floating));
            probe
                .output
                .insert("changing".into(), json!(player.flags.changing_dimension));
            probe
                .output
                .insert("first_good".into(), vec_out(state.first_good));
            probe
                .output
                .insert("last_good".into(), vec_out(state.last_good));
            results.push(Value::Object(probe.output));
        }
        Ok(json!(results))
    }
    #[test]
    fn actual_original_teleport_ack_reset_and_listener_tick_bookkeeping() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../tools/player_movement_bookkeeping_vectors.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["oracle_inner_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let mut differences = Vec::new();
        for (i, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
            let actual = bookkeeping(&row["query"]).unwrap();
            if actual != row["expected"] {
                if differences.len() < 8 {
                    eprintln!(
                        "bookkeeping{i} {}: {}\nnative={actual}\njava={}",
                        row["category"], row["query"], row["expected"]
                    );
                }
                differences.push(i);
            }
        }
        assert!(
            differences.is_empty(),
            "{} originalbookkeeping differences,first{:?}",
            differences.len(),
            &differences[..differences.len().min(8)]
        );
    }
    #[test]
    fn invalid_block_sequence_and_reserved_relative_flags_fail_before_mutation() {
        let mut state = MovementState::new(Vec3::ZERO);
        state.acknowledge_block_changes_up_to(3).unwrap();
        state.acknowledge_block_changes_up_to(2).unwrap();
        assert!(state.acknowledge_block_changes_up_to(-1).is_err());
        assert_eq!(state.take_block_ack(), Some(3));
        assert_eq!(state.take_block_ack(), None);
        let target = Transform {
            position: Vec3::ZERO,
            rotation: Rotation::default(),
        };
        assert!(state
            .teleport_with_relative_flags(target, target, 0x20, TeleportCause::External)
            .is_err());
        assert!(state.pending_teleport.is_none());
        assert_eq!(state.teleport_id, 0);
    }
}
