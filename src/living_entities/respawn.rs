//! Java 1.21 `PlayerList.respawn`, original source lines 469–530.
//!
//! This is the ordered source executor, not a player/session implementation.
//! Construction, restoration, world membership, menus, persistence lineage and
//! output require the actual owners described by [`RespawnHost`]. There is no
//! Main adapter in this module. Pinned original-Java `javap`/official-mapping
//! evidence is recorded in `docs/reference/entities-survival-death-respawn.md`.

/// The immutable coordinates of the source transition's Vec3, or current actor
/// coordinates read for a connection teleport. This owns no physical state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Actual PlayerList, old/new ServerPlayer, world and retained-connection owners.
///
/// `Actor` is an identity-qualified handle to a real native actor lifetime, not
/// its reusable numeric entity ID. `new_server_player` must create a distinct
/// native generation through the real constructor/identity/RNG owners; that
/// constructor's allocated ID is overwritten only at the source's later
/// `set_entity_id` call. An old-generation callback must never resolve to the
/// new actor. The returned actor retains the existing network listener and TCP
/// connection. Respawn does not perform login admission or reset that listener's
/// teleport sequence, chat/signature/ACK state, transport or rate-limit owner.
///
/// Every method is mandatory, synchronous and attached to the source owner at
/// that callsite. Missing runtime support must return an explicit error rather
/// than fabricate an actor, empty query, successful restoration or packet. A
/// successful callback must perform its full documented operation. Earlier
/// effects remain committed on failure: this is not a rollback transaction and
/// must not be retried from the beginning after partial output/insertion.
pub trait RespawnHost {
    type Error;
    type Actor;
    type RemovalReason;
    type ServerLevel;
    type Transition;
    type Profile;
    type ClientInformation;
    type Connection;
    type MainArm;
    type TagIterator;
    type Tag;
    type LevelData;
    type CommonSpawnInfo;
    type BlockPos;
    type Difficulty;
    type Uuid;
    type BlockState;

    /// PlayerList.players.remove(old), retaining the session/UUID-map owner.
    fn remove_from_player_list(&mut self, old: &Self::Actor) -> Result<(), Self::Error>;
    fn server_level(&mut self, actor: &Self::Actor) -> Result<Self::ServerLevel, Self::Error>;
    /// Actual ServerLevel.removePlayerImmediately(old, reason). This includes
    /// virtual Player/LivingEntity/Entity removal, menu/effect/brain/tracking
    /// callbacks and source removal order. It is not disconnect cleanup and
    /// must remain correct if death-tick removal already marked this actor.
    fn remove_player_immediately(
        &mut self,
        level: &Self::ServerLevel,
        old: &Self::Actor,
        reason: &Self::RemovalReason,
    ) -> Result<(), Self::Error>;
    /// Actual old.findRespawnPositionAndUseSpawnBlock(alive, DO_NOTHING), source
    /// 827–841 and 857–885. Resolve the real saved dimension/position/angle/forced
    /// state, bed/anchor/default spawn queries and spawn-block mutations. Login's
    /// initial coordinate and a synthetic default transition are not substitutes.
    fn find_respawn_position_and_use_spawn_block(
        &mut self,
        old: &Self::Actor,
        alive: bool,
    ) -> Result<Self::Transition, Self::Error>;
    fn transition_level(
        &mut self,
        transition: &Self::Transition,
    ) -> Result<Self::ServerLevel, Self::Error>;
    fn profile(&mut self, actor: &Self::Actor) -> Result<Self::Profile, Self::Error>;
    fn client_information(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Self::ClientInformation, Self::Error>;
    /// Full ServerPlayer(server, destination, oldProfile, oldClientInformation),
    /// including actual player/body/menu/food/attributes/effects defaults, spawn
    /// invulnerability, global identity allocation and all constructor RNG work.
    /// Do not clone the old lifecycle, recycle its generation, duplicate the
    /// connection, or construct a field-only stand-in. The actual server is the
    /// existing PlayerList server; the new actor is not inserted into its world,
    /// PlayerList list or UUID map until the later explicit source callbacks.
    fn new_server_player(
        &mut self,
        destination: &Self::ServerLevel,
        profile: Self::Profile,
        client_information: Self::ClientInformation,
    ) -> Result<Self::Actor, Self::Error>;
    /// Read the actor's actual connection field at each source callsite.
    fn connection(&mut self, actor: &Self::Actor) -> Result<Self::Connection, Self::Error>;
    fn set_connection(
        &mut self,
        actor: &Self::Actor,
        connection: Self::Connection,
    ) -> Result<(), Self::Error>;
    /// Full ServerPlayer.restoreFrom(old, alive), source 1415–1467, on the real
    /// newly constructed actor. Reuse WardenSpawnTracker/RemoteChatSession;
    /// restore current/previous game mode and abilities; assign attribute BASE
    /// values, then health=max. For alive: replace inventory, health, FoodData,
    /// copy-constructed effects through addEffect, XP/score and portal process.
    /// For dead: keepInventory || old spectator only retains inventory/XP/score;
    /// food/effects otherwise remain constructor-owned; health already equals
    /// current maximum from the unconditional attribute/health prefix. Always
    /// carry enchantment seed, EnderChest, model customization, recipe book,
    /// credits/nether/chunk-tracking state, shoulder compounds and last-death
    /// location; reset last-sent XP/health/food to -1. Keep actual item/save
    /// lineage and mandatory effect/attribute/menu callbacks. A health-only reset,
    /// wholesale old-state clone or successful no-op is not this operation.
    fn restore_from(
        &mut self,
        actor: &Self::Actor,
        old: &Self::Actor,
        alive: bool,
    ) -> Result<(), Self::Error>;
    fn entity_id(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_entity_id(&mut self, actor: &Self::Actor, id: i32) -> Result<(), Self::Error>;
    fn main_arm(&mut self, actor: &Self::Actor) -> Result<Self::MainArm, Self::Error>;
    fn set_main_arm(&mut self, actor: &Self::Actor, arm: Self::MainArm) -> Result<(), Self::Error>;
    fn transition_missing_respawn_block(
        &mut self,
        transition: &Self::Transition,
    ) -> Result<bool, Self::Error>;
    /// Actual new.copyRespawnPosition(old), including source setter behavior.
    fn copy_respawn_position(
        &mut self,
        actor: &Self::Actor,
        old: &Self::Actor,
    ) -> Result<(), Self::Error>;
    /// The actual old.getTags().iterator(), retaining live iteration semantics.
    /// Do not replace the source iterator with a cloned/sorted tag snapshot.
    fn tags_iterator(&mut self, old: &Self::Actor) -> Result<Self::TagIterator, Self::Error>;
    /// Actual iterator hasNext/next in order, including fail-fast behavior.
    fn next_tag(
        &mut self,
        iterator: &mut Self::TagIterator,
    ) -> Result<Option<Self::Tag>, Self::Error>;
    /// Actual new.addTag(tag); its boolean result is discarded by Java.
    fn add_tag(&mut self, actor: &Self::Actor, tag: Self::Tag) -> Result<(), Self::Error>;
    fn transition_position(
        &mut self,
        transition: &Self::Transition,
    ) -> Result<Position, Self::Error>;
    fn transition_y_rot(&mut self, transition: &Self::Transition) -> Result<f32, Self::Error>;
    fn transition_x_rot(&mut self, transition: &Self::Transition) -> Result<f32, Self::Error>;
    /// Actual new.moveTo(x,y,z,yRot,xRot), with physical/world callbacks.
    fn move_to(
        &mut self,
        actor: &Self::Actor,
        position: Position,
        y_rot: f32,
        x_rot: f32,
    ) -> Result<(), Self::Error>;
    /// Send GameEventPacket.NO_RESPAWN_BLOCK_AVAILABLE, value 0, on this exact
    /// retained connection through the current ordered output owner.
    fn send_no_respawn_block(&mut self, connection: &Self::Connection) -> Result<(), Self::Error>;
    fn level_data(&mut self, level: &Self::ServerLevel) -> Result<Self::LevelData, Self::Error>;
    /// Actual actor.createCommonSpawnInfo(the retained current-level local).
    fn create_common_spawn_info(
        &mut self,
        actor: &Self::Actor,
        level: &Self::ServerLevel,
    ) -> Result<Self::CommonSpawnInfo, Self::Error>;
    fn send_respawn(
        &mut self,
        connection: &Self::Connection,
        info: Self::CommonSpawnInfo,
        keep_data: u8,
    ) -> Result<(), Self::Error>;
    fn x(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn y(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn z(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn y_rot(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn x_rot(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Existing ServerGamePacketListenerImpl.teleport(x,y,z,yRot,xRot), retaining
    /// its actual teleport-ID sequence and pending-confirmation/movement owner.
    fn connection_teleport(
        &mut self,
        connection: &Self::Connection,
        position: Position,
        y_rot: f32,
        x_rot: f32,
    ) -> Result<(), Self::Error>;
    fn shared_spawn_position(
        &mut self,
        level: &Self::ServerLevel,
    ) -> Result<Self::BlockPos, Self::Error>;
    fn shared_spawn_angle(&mut self, level: &Self::ServerLevel) -> Result<f32, Self::Error>;
    fn send_default_spawn_position(
        &mut self,
        connection: &Self::Connection,
        position: Self::BlockPos,
        angle: f32,
    ) -> Result<(), Self::Error>;
    fn difficulty(&mut self, data: &Self::LevelData) -> Result<Self::Difficulty, Self::Error>;
    fn difficulty_locked(&mut self, data: &Self::LevelData) -> Result<bool, Self::Error>;
    fn send_difficulty(
        &mut self,
        connection: &Self::Connection,
        difficulty: Self::Difficulty,
        locked: bool,
    ) -> Result<(), Self::Error>;
    fn experience_progress(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn total_experience(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn experience_level(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn send_experience(
        &mut self,
        connection: &Self::Connection,
        progress: f32,
        total: i32,
        level: i32,
    ) -> Result<(), Self::Error>;
    /// Full PlayerList.sendActivePlayerEffects(actor), source 534–541, through
    /// the actor's actual current connection and real active-effect collection.
    fn send_active_player_effects(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Full PlayerList.sendLevelInfo(actor, retainedDestination), source 714–728,
    /// with real border/time/default-spawn/weather output and the server tick-
    /// rate manager's actual joining-player update.
    fn send_level_info(
        &mut self,
        actor: &Self::Actor,
        destination: &Self::ServerLevel,
    ) -> Result<(), Self::Error>;
    /// Full PlayerList.sendPlayerPermissionLevel(actor), source 544–547/634–646.
    fn send_player_permission_level(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Actual destination.addRespawnedPlayer(actor), source 916–917, retaining
    /// world/entity-index generation, tracking, tick-list and insertion owners.
    fn add_respawned_player(
        &mut self,
        destination: &Self::ServerLevel,
        actor: &Self::Actor,
    ) -> Result<(), Self::Error>;
    fn append_to_player_list(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn uuid(&mut self, actor: &Self::Actor) -> Result<Self::Uuid, Self::Error>;
    fn replace_uuid_mapping(
        &mut self,
        uuid: Self::Uuid,
        actor: &Self::Actor,
    ) -> Result<(), Self::Error>;
    /// Actual new.initInventoryMenu, with the newly constructed actor's menu,
    /// synchronizer/listener and generation-qualified publication callbacks.
    fn init_inventory_menu(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn health(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Actual LivingEntity.setHealth, not assignment to a second health owner.
    fn set_health(&mut self, actor: &Self::Actor, health: f32) -> Result<(), Self::Error>;
    /// Actual BlockPos.containing(transition.position()), retaining Java floor
    /// and coordinate conversion behavior for the real destination query.
    fn block_position_containing(
        &mut self,
        position: Position,
    ) -> Result<Self::BlockPos, Self::Error>;
    fn block_state(
        &mut self,
        destination: &Self::ServerLevel,
        position: &Self::BlockPos,
    ) -> Result<Self::BlockState, Self::Error>;
    fn is_respawn_anchor(&mut self, state: &Self::BlockState) -> Result<bool, Self::Error>;
    fn block_x(&mut self, position: &Self::BlockPos) -> Result<i32, Self::Error>;
    fn block_y(&mut self, position: &Self::BlockPos) -> Result<i32, Self::Error>;
    fn block_z(&mut self, position: &Self::BlockPos) -> Result<i32, Self::Error>;
    /// Actual retained destination.random.nextLong(), not actor/session RNG.
    fn level_random_next_long(
        &mut self,
        destination: &Self::ServerLevel,
    ) -> Result<i64, Self::Error>;
    /// Send SoundPacket(RESPAWN_ANCHOR_DEPLETE, BLOCKS, x,y,z, 1,1, seed) through
    /// this connection. Coordinates are integer block coordinates cast to double.
    fn send_anchor_deplete_sound(
        &mut self,
        connection: &Self::Connection,
        position: Position,
        seed: i64,
    ) -> Result<(), Self::Error>;
}

/// Execute the exact PlayerList replacement order after the listener selects
/// respawn. The listener must assign the returned real actor to its player field
/// and replace its actor/menu/physical references, while retaining its existing
/// connection owners. Health eligibility, won-game/hardcore branches and the
/// listener assignment are source operations outside this PlayerList method.
pub fn respawn<H: RespawnHost + ?Sized>(
    host: &mut H,
    old: &H::Actor,
    alive: bool,
    reason: &H::RemovalReason,
) -> Result<H::Actor, H::Error> {
    host.remove_from_player_list(old)?;
    let old_level = host.server_level(old)?;
    host.remove_player_immediately(&old_level, old, reason)?;
    let transition = host.find_respawn_position_and_use_spawn_block(old, alive)?;
    let destination = host.transition_level(&transition)?;
    let profile = host.profile(old)?;
    let client_information = host.client_information(old)?;
    let actor = host.new_server_player(&destination, profile, client_information)?;
    let connection = host.connection(old)?;
    host.set_connection(&actor, connection)?;
    host.restore_from(&actor, old, alive)?;
    let id = host.entity_id(old)?;
    host.set_entity_id(&actor, id)?;
    let arm = host.main_arm(old)?;
    host.set_main_arm(&actor, arm)?;
    if !host.transition_missing_respawn_block(&transition)? {
        host.copy_respawn_position(&actor, old)?;
    }
    let mut tags = host.tags_iterator(old)?;
    while let Some(tag) = host.next_tag(&mut tags)? {
        host.add_tag(&actor, tag)?;
    }
    let position = host.transition_position(&transition)?;
    let y_rot = host.transition_y_rot(&transition)?;
    let x_rot = host.transition_x_rot(&transition)?;
    host.move_to(&actor, position, y_rot, x_rot)?;
    if host.transition_missing_respawn_block(&transition)? {
        let connection = host.connection(&actor)?;
        host.send_no_respawn_block(&connection)?;
    }
    let keep_data = u8::from(alive);
    // These are distinct source locals. Later callbacks may mutate the actor's
    // current level; default-spawn/world insertion still use destination.
    let current_level = host.server_level(&actor)?;
    let level_data = host.level_data(&current_level)?;
    let connection = host.connection(&actor)?;
    let info = host.create_common_spawn_info(&actor, &current_level)?;
    host.send_respawn(&connection, info, keep_data)?;
    let connection = host.connection(&actor)?;
    let x = host.x(&actor)?;
    let y = host.y(&actor)?;
    let z = host.z(&actor)?;
    let y_rot = host.y_rot(&actor)?;
    let x_rot = host.x_rot(&actor)?;
    host.connection_teleport(&connection, Position { x, y, z }, y_rot, x_rot)?;
    let connection = host.connection(&actor)?;
    let position = host.shared_spawn_position(&destination)?;
    let angle = host.shared_spawn_angle(&destination)?;
    host.send_default_spawn_position(&connection, position, angle)?;
    let connection = host.connection(&actor)?;
    let difficulty = host.difficulty(&level_data)?;
    let locked = host.difficulty_locked(&level_data)?;
    host.send_difficulty(&connection, difficulty, locked)?;
    let connection = host.connection(&actor)?;
    let progress = host.experience_progress(&actor)?;
    let total = host.total_experience(&actor)?;
    let level = host.experience_level(&actor)?;
    host.send_experience(&connection, progress, total, level)?;
    host.send_active_player_effects(&actor)?;
    host.send_level_info(&actor, &destination)?;
    host.send_player_permission_level(&actor)?;
    host.add_respawned_player(&destination, &actor)?;
    host.append_to_player_list(&actor)?;
    let uuid = host.uuid(&actor)?;
    host.replace_uuid_mapping(uuid, &actor)?;
    host.init_inventory_menu(&actor)?;
    let health = host.health(&actor)?;
    host.set_health(&actor, health)?;
    if !alive {
        // Source re-reads transition.position here, rather than the earlier
        // move-to local or the actor's possibly changed current position.
        let position = host.transition_position(&transition)?;
        let block_position = host.block_position_containing(position)?;
        let state = host.block_state(&destination, &block_position)?;
        if host.is_respawn_anchor(&state)? {
            let connection = host.connection(&actor)?;
            let x = f64::from(host.block_x(&block_position)?);
            let y = f64::from(host.block_y(&block_position)?);
            let z = f64::from(host.block_z(&block_position)?);
            let seed = host.level_random_next_long(&destination)?;
            host.send_anchor_deplete_sound(&connection, Position { x, y, z }, seed)?;
        }
    }
    Ok(actor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    // Controlled source-call recorder only; these are not native actor, world,
    // save, menu or packet owners and do not establish Main integration.
    struct Host {
        calls: Vec<String>,
        missing: VecDeque<bool>,
        anchor: bool,
        fail_at: Option<&'static str>,
        mutate_during_respawn_send: bool,
        new_connection: u8,
        new_position: Position,
        health: f32,
    }

    impl Default for Host {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                missing: VecDeque::from([false, false]),
                anchor: true,
                fail_at: None,
                mutate_during_respawn_send: false,
                new_connection: 0,
                new_position: Position {
                    x: 1.5,
                    y: 64.0,
                    z: -2.5,
                },
                health: 17.0,
            }
        }
    }

    impl Host {
        fn call(&mut self, call: impl Into<String>) -> Result<(), &'static str> {
            let call = call.into();
            let fail = self.fail_at == Some(call.as_str());
            self.calls.push(call);
            if fail {
                Err("owner failure")
            } else {
                Ok(())
            }
        }

        fn read<T>(&mut self, call: impl Into<String>, value: T) -> Result<T, &'static str> {
            self.call(call)?;
            Ok(value)
        }

        fn index(&self, call: &str) -> usize {
            self.calls.iter().position(|actual| actual == call).unwrap()
        }
    }

    impl RespawnHost for Host {
        type Error = &'static str;
        type Actor = (i32, u64);
        type RemovalReason = &'static str;
        type ServerLevel = u8;
        type Transition = ();
        type Profile = u8;
        type ClientInformation = u8;
        type Connection = u8;
        type MainArm = u8;
        type TagIterator = VecDeque<&'static str>;
        type Tag = &'static str;
        type LevelData = u8;
        type CommonSpawnInfo = u8;
        type BlockPos = (i32, i32, i32);
        type Difficulty = u8;
        type Uuid = u8;
        type BlockState = bool;

        fn remove_from_player_list(&mut self, old: &Self::Actor) -> Result<(), Self::Error> {
            self.call(format!("list-remove:{old:?}"))
        }
        fn server_level(&mut self, actor: &Self::Actor) -> Result<u8, Self::Error> {
            self.read(format!("level:{actor:?}"), if actor.1 == 5 { 1 } else { 3 })
        }
        fn remove_player_immediately(
            &mut self,
            level: &u8,
            old: &Self::Actor,
            reason: &&'static str,
        ) -> Result<(), Self::Error> {
            self.call(format!("retire:{level}:{old:?}:{reason}"))
        }
        fn find_respawn_position_and_use_spawn_block(
            &mut self,
            _: &Self::Actor,
            alive: bool,
        ) -> Result<(), Self::Error> {
            self.read(format!("find:{alive}"), ())
        }
        fn transition_level(&mut self, _: &()) -> Result<u8, Self::Error> {
            self.read("destination", 2)
        }
        fn profile(&mut self, _: &Self::Actor) -> Result<u8, Self::Error> {
            self.read("profile", 10)
        }
        fn client_information(&mut self, _: &Self::Actor) -> Result<u8, Self::Error> {
            self.read("client-information", 11)
        }
        fn new_server_player(
            &mut self,
            destination: &u8,
            profile: u8,
            info: u8,
        ) -> Result<Self::Actor, Self::Error> {
            self.read(
                format!("construct:{destination}:{profile}:{info}"),
                (100, 6),
            )
        }
        fn connection(&mut self, actor: &Self::Actor) -> Result<u8, Self::Error> {
            self.read(
                format!("connection:{actor:?}"),
                if actor.1 == 5 { 7 } else { self.new_connection },
            )
        }
        fn set_connection(&mut self, _: &Self::Actor, connection: u8) -> Result<(), Self::Error> {
            self.call(format!("set-connection:{connection}"))?;
            self.new_connection = connection;
            Ok(())
        }
        fn restore_from(
            &mut self,
            _: &Self::Actor,
            _: &Self::Actor,
            alive: bool,
        ) -> Result<(), Self::Error> {
            self.call(format!("restore:{alive}"))
        }
        fn entity_id(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error> {
            self.read("old-id", actor.0)
        }
        fn set_entity_id(&mut self, actor: &Self::Actor, id: i32) -> Result<(), Self::Error> {
            self.call(format!("set-id:{actor:?}:{id}"))
        }
        fn main_arm(&mut self, _: &Self::Actor) -> Result<u8, Self::Error> {
            self.read("old-arm", 1)
        }
        fn set_main_arm(&mut self, _: &Self::Actor, arm: u8) -> Result<(), Self::Error> {
            self.call(format!("set-arm:{arm}"))
        }
        fn transition_missing_respawn_block(&mut self, _: &()) -> Result<bool, Self::Error> {
            self.call("missing-block")?;
            Ok(self.missing.pop_front().unwrap())
        }
        fn copy_respawn_position(
            &mut self,
            _: &Self::Actor,
            _: &Self::Actor,
        ) -> Result<(), Self::Error> {
            self.call("copy-respawn")
        }
        fn tags_iterator(&mut self, _: &Self::Actor) -> Result<Self::TagIterator, Self::Error> {
            self.read("tags-iterator", VecDeque::from(["one", "two"]))
        }
        fn next_tag(
            &mut self,
            iterator: &mut Self::TagIterator,
        ) -> Result<Option<Self::Tag>, Self::Error> {
            self.call("next-tag")?;
            Ok(iterator.pop_front())
        }
        fn add_tag(&mut self, _: &Self::Actor, tag: Self::Tag) -> Result<(), Self::Error> {
            self.call(format!("add-tag:{tag}"))
        }
        fn transition_position(&mut self, _: &()) -> Result<Position, Self::Error> {
            self.read(
                "transition-position",
                Position {
                    x: 1.5,
                    y: 64.0,
                    z: -2.5,
                },
            )
        }
        fn transition_y_rot(&mut self, _: &()) -> Result<f32, Self::Error> {
            self.read("transition-yrot", 20.0)
        }
        fn transition_x_rot(&mut self, _: &()) -> Result<f32, Self::Error> {
            self.read("transition-xrot", 30.0)
        }
        fn move_to(
            &mut self,
            _: &Self::Actor,
            position: Position,
            y_rot: f32,
            x_rot: f32,
        ) -> Result<(), Self::Error> {
            self.call(format!("move:{position:?}:{y_rot}:{x_rot}"))?;
            self.new_position = position;
            Ok(())
        }
        fn send_no_respawn_block(&mut self, connection: &u8) -> Result<(), Self::Error> {
            self.call(format!("no-block:{connection}"))
        }
        fn level_data(&mut self, level: &u8) -> Result<u8, Self::Error> {
            self.read(format!("level-data:{level}"), *level + 10)
        }
        fn create_common_spawn_info(
            &mut self,
            _: &Self::Actor,
            level: &u8,
        ) -> Result<u8, Self::Error> {
            self.read(format!("spawn-info:{level}"), *level)
        }
        fn send_respawn(
            &mut self,
            connection: &u8,
            info: u8,
            keep_data: u8,
        ) -> Result<(), Self::Error> {
            self.call(format!("respawn:{connection}:{info}:{keep_data}"))?;
            if self.mutate_during_respawn_send {
                self.new_connection = 8;
                self.new_position = Position {
                    x: 91.0,
                    y: 92.0,
                    z: 93.0,
                };
            }
            Ok(())
        }
        fn x(&mut self, _: &Self::Actor) -> Result<f64, Self::Error> {
            self.read("x", self.new_position.x)
        }
        fn y(&mut self, _: &Self::Actor) -> Result<f64, Self::Error> {
            self.read("y", self.new_position.y)
        }
        fn z(&mut self, _: &Self::Actor) -> Result<f64, Self::Error> {
            self.read("z", self.new_position.z)
        }
        fn y_rot(&mut self, _: &Self::Actor) -> Result<f32, Self::Error> {
            self.read("yrot", 40.0)
        }
        fn x_rot(&mut self, _: &Self::Actor) -> Result<f32, Self::Error> {
            self.read("xrot", 50.0)
        }
        fn connection_teleport(
            &mut self,
            connection: &u8,
            position: Position,
            y_rot: f32,
            x_rot: f32,
        ) -> Result<(), Self::Error> {
            self.call(format!(
                "teleport:{connection}:{position:?}:{y_rot}:{x_rot}"
            ))
        }
        fn shared_spawn_position(&mut self, level: &u8) -> Result<Self::BlockPos, Self::Error> {
            self.read(format!("shared-spawn:{level}"), (0, 70, 0))
        }
        fn shared_spawn_angle(&mut self, level: &u8) -> Result<f32, Self::Error> {
            self.read(format!("shared-angle:{level}"), 60.0)
        }
        fn send_default_spawn_position(
            &mut self,
            connection: &u8,
            position: Self::BlockPos,
            angle: f32,
        ) -> Result<(), Self::Error> {
            self.call(format!("default-spawn:{connection}:{position:?}:{angle}"))
        }
        fn difficulty(&mut self, data: &u8) -> Result<u8, Self::Error> {
            self.read(format!("difficulty:{data}"), 2)
        }
        fn difficulty_locked(&mut self, data: &u8) -> Result<bool, Self::Error> {
            self.read(format!("locked:{data}"), true)
        }
        fn send_difficulty(
            &mut self,
            connection: &u8,
            difficulty: u8,
            locked: bool,
        ) -> Result<(), Self::Error> {
            self.call(format!(
                "difficulty-packet:{connection}:{difficulty}:{locked}"
            ))
        }
        fn experience_progress(&mut self, _: &Self::Actor) -> Result<f32, Self::Error> {
            self.read("xp-progress", 0.5)
        }
        fn total_experience(&mut self, _: &Self::Actor) -> Result<i32, Self::Error> {
            self.read("xp-total", 30)
        }
        fn experience_level(&mut self, _: &Self::Actor) -> Result<i32, Self::Error> {
            self.read("xp-level", 4)
        }
        fn send_experience(
            &mut self,
            connection: &u8,
            progress: f32,
            total: i32,
            level: i32,
        ) -> Result<(), Self::Error> {
            self.call(format!("xp-packet:{connection}:{progress}:{total}:{level}"))
        }
        fn send_active_player_effects(&mut self, _: &Self::Actor) -> Result<(), Self::Error> {
            self.call("active-effects")
        }
        fn send_level_info(
            &mut self,
            _: &Self::Actor,
            destination: &u8,
        ) -> Result<(), Self::Error> {
            self.call(format!("level-info:{destination}"))
        }
        fn send_player_permission_level(&mut self, _: &Self::Actor) -> Result<(), Self::Error> {
            self.call("permissions")
        }
        fn add_respawned_player(
            &mut self,
            destination: &u8,
            actor: &Self::Actor,
        ) -> Result<(), Self::Error> {
            self.call(format!("world-add:{destination}:{actor:?}"))
        }
        fn append_to_player_list(&mut self, actor: &Self::Actor) -> Result<(), Self::Error> {
            self.call(format!("list-add:{actor:?}"))
        }
        fn uuid(&mut self, _: &Self::Actor) -> Result<u8, Self::Error> {
            self.read("uuid", 10)
        }
        fn replace_uuid_mapping(
            &mut self,
            uuid: u8,
            actor: &Self::Actor,
        ) -> Result<(), Self::Error> {
            self.call(format!("uuid-map:{uuid}:{actor:?}"))
        }
        fn init_inventory_menu(&mut self, _: &Self::Actor) -> Result<(), Self::Error> {
            self.call("init-menu")
        }
        fn health(&mut self, _: &Self::Actor) -> Result<f32, Self::Error> {
            self.read("health", self.health)
        }
        fn set_health(&mut self, _: &Self::Actor, health: f32) -> Result<(), Self::Error> {
            self.call(format!("set-health:{health}"))
        }
        fn block_position_containing(
            &mut self,
            position: Position,
        ) -> Result<Self::BlockPos, Self::Error> {
            self.read(format!("block-containing:{position:?}"), (1, 64, -3))
        }
        fn block_state(
            &mut self,
            destination: &u8,
            position: &Self::BlockPos,
        ) -> Result<bool, Self::Error> {
            self.read(
                format!("block-state:{destination}:{position:?}"),
                self.anchor,
            )
        }
        fn is_respawn_anchor(&mut self, state: &bool) -> Result<bool, Self::Error> {
            self.read("is-anchor", *state)
        }
        fn block_x(&mut self, position: &Self::BlockPos) -> Result<i32, Self::Error> {
            self.read("block-x", position.0)
        }
        fn block_y(&mut self, position: &Self::BlockPos) -> Result<i32, Self::Error> {
            self.read("block-y", position.1)
        }
        fn block_z(&mut self, position: &Self::BlockPos) -> Result<i32, Self::Error> {
            self.read("block-z", position.2)
        }
        fn level_random_next_long(&mut self, destination: &u8) -> Result<i64, Self::Error> {
            self.read(format!("world-random:{destination}"), -77)
        }
        fn send_anchor_deplete_sound(
            &mut self,
            connection: &u8,
            position: Position,
            seed: i64,
        ) -> Result<(), Self::Error> {
            self.call(format!("anchor-sound:{connection}:{position:?}:{seed}"))
        }
    }

    #[test]
    fn replacement_and_output_preserve_source_order_and_new_generation() {
        let mut host = Host::default();
        let actor = respawn(&mut host, &(42, 5), false, &"KILLED").unwrap();
        assert_eq!(actor, (100, 6)); // Handle lifetime survives the wire-ID change.
        assert_eq!(
            &host.calls[..9],
            &[
                "list-remove:(42, 5)",
                "level:(42, 5)",
                "retire:1:(42, 5):KILLED",
                "find:false",
                "destination",
                "profile",
                "client-information",
                "construct:2:10:11",
                "connection:(42, 5)",
            ]
        );
        let ordered = [
            "set-connection:7",
            "restore:false",
            "old-id",
            "set-id:(100, 6):42",
            "old-arm",
            "set-arm:1",
            "copy-respawn",
            "tags-iterator",
            "add-tag:one",
            "add-tag:two",
            "transition-position",
            "level:(100, 6)",
            "level-data:3",
            "spawn-info:3",
            "respawn:7:3:0",
            "teleport:7:Position { x: 1.5, y: 64.0, z: -2.5 }:40:50",
            "default-spawn:7:(0, 70, 0):60",
            "difficulty-packet:7:2:true",
            "xp-packet:7:0.5:30:4",
            "active-effects",
            "level-info:2",
            "permissions",
            "world-add:2:(100, 6)",
            "list-add:(100, 6)",
            "uuid",
            "uuid-map:10:(100, 6)",
            "init-menu",
            "health",
            "set-health:17",
            "block-state:2:(1, 64, -3)",
            "is-anchor",
            "block-x",
            "block-y",
            "block-z",
            "world-random:2",
            "anchor-sound:7:Position { x: 1.0, y: 64.0, z: -3.0 }:-77",
        ];
        assert!(ordered
            .windows(2)
            .all(|pair| host.index(pair[0]) < host.index(pair[1])));
        assert_eq!(
            host.calls
                .iter()
                .filter(|call| *call == "transition-position")
                .count(),
            2
        );
    }

    #[test]
    fn later_packets_reread_actor_connection_and_position_but_keep_level_locals() {
        let mut host = Host {
            missing: VecDeque::from([false, true]),
            mutate_during_respawn_send: true,
            ..Host::default()
        };
        respawn(&mut host, &(42, 5), false, &"KILLED").unwrap();
        assert!(host.index("copy-respawn") < host.index("no-block:7"));
        assert!(host.index("no-block:7") < host.index("respawn:7:3:0"));
        assert!(host
            .calls
            .contains(&"teleport:8:Position { x: 91.0, y: 92.0, z: 93.0 }:40:50".into()));
        assert!(host.calls.contains(&"default-spawn:8:(0, 70, 0):60".into()));
        assert!(host.calls.contains(&"difficulty:13".into()));
        assert!(host.calls.contains(&"level-info:2".into()));
        assert!(host
            .calls
            .contains(&"anchor-sound:8:Position { x: 1.0, y: 64.0, z: -3.0 }:-77".into()));
    }

    #[test]
    fn alive_skips_dead_anchor_query_and_rng_and_uses_keep_attribute_byte() {
        let mut host = Host {
            missing: VecDeque::from([true, true]),
            ..Host::default()
        };
        respawn(&mut host, &(42, 5), true, &"CHANGED_DIMENSION").unwrap();
        assert!(host.calls.contains(&"restore:true".into()));
        assert!(host.calls.contains(&"respawn:7:3:1".into()));
        assert!(!host.calls.contains(&"copy-respawn".into()));
        assert_eq!(host.calls.last().unwrap(), "set-health:17");
        assert!(!host
            .calls
            .iter()
            .any(|call| call.starts_with("block-") || call.starts_with("world-random")));
    }

    #[test]
    fn owner_failures_leave_only_the_completed_source_prefix() {
        let mut before_factory = Host {
            fail_at: Some("find:false"),
            ..Host::default()
        };
        assert_eq!(
            respawn(&mut before_factory, &(42, 5), false, &"KILLED"),
            Err("owner failure")
        );
        assert_eq!(
            before_factory.calls,
            [
                "list-remove:(42, 5)",
                "level:(42, 5)",
                "retire:1:(42, 5):KILLED",
                "find:false"
            ]
        );

        let mut after_insertion = Host {
            fail_at: Some("init-menu"),
            ..Host::default()
        };
        assert_eq!(
            respawn(&mut after_insertion, &(42, 5), false, &"KILLED"),
            Err("owner failure")
        );
        assert!(after_insertion.calls.contains(&"respawn:7:3:0".into()));
        assert!(after_insertion
            .calls
            .contains(&"world-add:2:(100, 6)".into()));
        assert!(after_insertion
            .calls
            .contains(&"uuid-map:10:(100, 6)".into()));
        assert_eq!(after_insertion.calls.last().unwrap(), "init-menu");
        assert!(!after_insertion.calls.contains(&"health".into()));
    }

    #[test]
    fn dead_non_anchor_uses_real_query_without_advancing_random() {
        let mut host = Host {
            anchor: false,
            ..Host::default()
        };
        respawn(&mut host, &(42, 5), false, &"KILLED").unwrap();
        assert_eq!(host.calls.last().unwrap(), "is-anchor");
        assert!(!host
            .calls
            .iter()
            .any(|call| call.starts_with("world-random") || call.starts_with("anchor-sound")));
    }
}
