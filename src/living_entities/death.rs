//! Original Java 1.21 generic LivingEntity.die and Player.die bodies.
//!
//! Source: `btn.a(brk)` 1377–1405 and `cmx.a(brk)` 692–715. ServerPlayer's
//! separate override never calls either body. These executors own no actor,
//! health, dead flag, inventory, RNG, world, connection or scheduler. All host
//! operations address the same retained canonical actor, even after removal.
//! No mutable actor/world borrow may survive a callback; reentry and errors
//! preserve the source mutation prefix, without rollback or deduplication.

pub trait LivingDeathHost {
    type Error;
    type Source: ?Sized;
    type Entity;
    type ServerLevel;
    type Component;

    fn is_removed(&mut self) -> Result<bool, Self::Error>;
    /// The actual LivingEntity.dead field (`be`), not health<=0, removed or a
    /// newly inferred dead/alive flag. ServerPlayer.die does not use this body.
    fn dead_field(&mut self) -> Result<bool, Self::Error>;
    /// Construct the host's error for the original null dereference at
    /// source.getEntity. Called only after both source guards have passed.
    fn null_damage_source_error(&mut self) -> Self::Error;
    /// Actual DamageSource.getEntity (causing entity, not direct entity).
    fn source_causing_entity(
        &mut self,
        source: &Self::Source,
    ) -> Result<Option<Self::Entity>, Self::Error>;
    /// Actual virtual getKillCredit, retaining its canonical actor reference.
    fn kill_credit(&mut self) -> Result<Option<Self::Entity>, Self::Error>;
    fn death_score(&mut self) -> Result<i32, Self::Error>;
    /// killer.awardKillScore(this, score, source), with this exact victim.
    fn killer_award_kill_score(
        &mut self,
        killer: &Self::Entity,
        score: i32,
        source: &Self::Source,
    ) -> Result<(), Self::Error>;
    fn is_sleeping(&mut self) -> Result<bool, Self::Error>;
    fn stop_sleeping(&mut self) -> Result<(), Self::Error>;
    /// Resolve the actor's current level at this particular source read.
    fn is_client_side(&mut self) -> Result<bool, Self::Error>;
    fn has_custom_name(&mut self) -> Result<bool, Self::Error>;
    fn combat_tracker_death_message(&mut self) -> Result<Self::Component, Self::Error>;
    fn component_get_string(&mut self, component: &Self::Component) -> Result<String, Self::Error>;
    /// Actual LOGGER.info("Named entity {} died: {}", this, text). Preserve the
    /// retained actor as formatter argument; do not substitute a saved name.
    fn log_named_entity_death(&mut self, text: &str) -> Result<(), Self::Error>;
    fn set_dead_field(&mut self, dead: bool) -> Result<(), Self::Error>;
    fn combat_tracker_recheck_status(&mut self) -> Result<(), Self::Error>;
    /// Actual current Level read and instanceof ServerLevel. Some retains that
    /// captured level across following callbacks; None is an actual nonserver
    /// level, not a fallback for an unavailable world owner.
    fn current_server_level(&mut self) -> Result<Option<Self::ServerLevel>, Self::Error>;
    /// Actual causingEntity.killedEntity(capturedLevel, this) virtual dispatch.
    fn causing_entity_killed_entity(
        &mut self,
        causer: &Self::Entity,
        level: &Self::ServerLevel,
    ) -> Result<bool, Self::Error>;
    fn entity_die_game_event(&mut self) -> Result<(), Self::Error>;
    /// Actual virtual dropAllDeathLoot, with the captured level and original
    /// source identity. None is possible in Player's tail after a parent early
    /// return: execute the actual nullable-source path, including any preceding
    /// loot/equipment mutations before its source.getEntity dereference fails.
    /// Do not reject None eagerly, return success for unavailable loot owners,
    /// or deduplicate a second call from the Player override.
    fn drop_all_death_loot(
        &mut self,
        level: &Self::ServerLevel,
        source: Option<&Self::Source>,
    ) -> Result<(), Self::Error>;
    /// Actual virtual createWitherRose(killer), including the nullable killer.
    fn create_wither_rose(&mut self, killer: Option<&Self::Entity>) -> Result<(), Self::Error>;
    /// Read the actor's CURRENT level for broadcastEntityEvent(this, event),
    /// independently of the earlier captured ServerLevel used for death loot.
    fn broadcast_entity_event(&mut self, event: u8) -> Result<(), Self::Error>;
    fn set_dying_pose(&mut self) -> Result<(), Self::Error>;
}

/// Direct generic LivingEntity.die body. It deliberately has no health guard,
/// removal, tick scheduling, or inserted Player/ServerPlayer behavior.
pub fn living_die<H: LivingDeathHost + ?Sized>(
    host: &mut H,
    source: Option<&H::Source>,
) -> Result<(), H::Error> {
    if host.is_removed()? || host.dead_field()? {
        return Ok(());
    }
    let source = match source {
        Some(source) => source,
        None => return Err(host.null_damage_source_error()),
    };
    let causer = host.source_causing_entity(source)?;
    let killer = host.kill_credit()?;
    if host.death_score()? >= 0 {
        if let Some(killer) = killer.as_ref() {
            let score = host.death_score()?;
            host.killer_award_kill_score(killer, score, source)?;
        }
    }
    if host.is_sleeping()? {
        host.stop_sleeping()?;
    }
    if !host.is_client_side()? && host.has_custom_name()? {
        let message = host.combat_tracker_death_message()?;
        let text = host.component_get_string(&message)?;
        host.log_named_entity_death(&text)?;
    }
    host.set_dead_field(true)?;
    host.combat_tracker_recheck_status()?;
    if let Some(level) = host.current_server_level()? {
        let allow_loot = match causer.as_ref() {
            Some(causer) => host.causing_entity_killed_entity(causer, &level)?,
            None => true,
        };
        if allow_loot {
            host.entity_die_game_event()?;
            host.drop_all_death_loot(&level, Some(source))?;
            host.create_wither_rose(killer.as_ref())?;
        }
        host.broadcast_entity_event(3)?;
    }
    host.set_dying_pose()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerDeathResetStatistic {
    TimeSinceDeath,
    TimeSinceRest,
}

pub trait PlayerDeathHost: LivingDeathHost {
    type Dimension;
    type BlockPos;

    fn reapply_position(&mut self) -> Result<(), Self::Error>;
    fn is_spectator(&mut self) -> Result<bool, Self::Error>;
    fn hurt_direction(&mut self) -> Result<f32, Self::Error>;
    fn y_rotation(&mut self) -> Result<f32, Self::Error>;
    /// Actual Mth.cos(float), e.g. geometry::vanilla_cos, not f64/libm cosine.
    fn mth_cos(&mut self, angle: f32) -> Result<f32, Self::Error>;
    /// Actual Mth.sin(float), e.g. geometry::vanilla_sin.
    fn mth_sin(&mut self, angle: f32) -> Result<f32, Self::Error>;
    fn set_delta_movement(&mut self, x: f64, y: f64, z: f64) -> Result<(), Self::Error>;
    fn award_deaths_stat(&mut self) -> Result<(), Self::Error>;
    fn reset_stat(&mut self, statistic: PlayerDeathResetStatistic) -> Result<(), Self::Error>;
    fn clear_fire(&mut self) -> Result<(), Self::Error>;
    fn set_shared_flag_on_fire(&mut self, on_fire: bool) -> Result<(), Self::Error>;
    fn current_dimension(&mut self) -> Result<Self::Dimension, Self::Error>;
    fn block_position(&mut self) -> Result<Self::BlockPos, Self::Error>;
    fn set_last_death_location(
        &mut self,
        dimension: Self::Dimension,
        position: Self::BlockPos,
    ) -> Result<(), Self::Error>;
}

/// Direct Player.die body. Its invokespecial parent call selects living_die,
/// while callbacks inside both bodies retain normal virtual dispatch. The
/// Player tail runs even when the parent returned for removed/dead. In the
/// ordinary server branch both parent and tail can call death loot; no native
/// callback may silently suppress the second invocation. ServerPlayer selects
/// server_player_death::die instead of entering either body here.
pub fn player_die<H: PlayerDeathHost + ?Sized>(
    host: &mut H,
    source: Option<&H::Source>,
) -> Result<(), H::Error> {
    living_die(host, source)?;
    host.reapply_position()?;
    if !host.is_spectator()? {
        if let Some(level) = host.current_server_level()? {
            host.drop_all_death_loot(&level, source)?;
        }
    }
    if source.is_some() {
        let angle = (host.hurt_direction()? + host.y_rotation()?) * 0.017453292_f32;
        let x = f64::from(-host.mth_cos(angle)? * 0.1_f32);
        // Java rereads both virtual float getters for the z expression.
        let angle = (host.hurt_direction()? + host.y_rotation()?) * 0.017453292_f32;
        let z = f64::from(-host.mth_sin(angle)? * 0.1_f32);
        host.set_delta_movement(x, 0.10000000149011612_f64, z)?;
    } else {
        host.set_delta_movement(0., 0.1_f64, 0.)?;
    }
    host.award_deaths_stat()?;
    host.reset_stat(PlayerDeathResetStatistic::TimeSinceDeath)?;
    host.reset_stat(PlayerDeathResetStatistic::TimeSinceRest)?;
    host.clear_fire()?;
    host.set_shared_flag_on_fire(false)?;
    let dimension = host.current_dimension()?;
    let position = host.block_position()?;
    host.set_last_death_location(dimension, position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, rc::Rc};

    struct Host {
        calls: Vec<String>,
        source: Rc<u32>,
        causer: Option<Rc<u32>>,
        killer: Option<Rc<u32>>,
        level: Rc<u32>,
        removed: bool,
        dead: bool,
        sleeping: bool,
        client: bool,
        named: bool,
        server: bool,
        spectator: bool,
        allow_loot: bool,
        score: i32,
        mutate: bool,
        fail: Option<&'static str>,
        drops: usize,
        angles: Vec<f32>,
        directions: VecDeque<f32>,
        rotations: VecDeque<f32>,
        velocity: Option<[f64; 3]>,
        death_location: Option<(u32, u32)>,
    }
    impl Host {
        fn new() -> Self {
            Self {
                calls: vec![],
                source: Rc::new(767),
                causer: Some(Rc::new(1)),
                killer: Some(Rc::new(2)),
                level: Rc::new(10),
                removed: false,
                dead: false,
                sleeping: true,
                client: false,
                named: true,
                server: true,
                spectator: false,
                allow_loot: true,
                score: 4,
                mutate: false,
                fail: None,
                drops: 0,
                angles: vec![],
                directions: [10., 20.].into(),
                rotations: [30., 40.].into(),
                velocity: None,
                death_location: None,
            }
        }
        fn call(&mut self, name: &str) -> Result<(), &'static str> {
            self.calls.push(name.into());
            if self.fail == Some(name) {
                Err("owner failure")
            } else {
                Ok(())
            }
        }
        fn check_source(&self, s: &Rc<u32>) {
            assert!(Rc::ptr_eq(s, &self.source));
        }
    }
    impl LivingDeathHost for Host {
        type Error = &'static str;
        type Source = Rc<u32>;
        type Entity = Rc<u32>;
        type ServerLevel = Rc<u32>;
        type Component = Rc<String>;
        fn is_removed(&mut self) -> Result<bool, Self::Error> {
            self.call("removed")?;
            Ok(self.removed)
        }
        fn dead_field(&mut self) -> Result<bool, Self::Error> {
            self.call("dead_field")?;
            Ok(self.dead)
        }
        fn null_damage_source_error(&mut self) -> Self::Error {
            self.calls.push("null_source".into());
            "null damage source"
        }
        fn source_causing_entity(
            &mut self,
            s: &Self::Source,
        ) -> Result<Option<Self::Entity>, Self::Error> {
            self.check_source(s);
            self.call("causer")?;
            Ok(self.causer.clone())
        }
        fn kill_credit(&mut self) -> Result<Option<Self::Entity>, Self::Error> {
            self.call("kill_credit")?;
            Ok(self.killer.clone())
        }
        fn death_score(&mut self) -> Result<i32, Self::Error> {
            self.call("score")?;
            Ok(self.score)
        }
        fn killer_award_kill_score(
            &mut self,
            k: &Self::Entity,
            score: i32,
            s: &Self::Source,
        ) -> Result<(), Self::Error> {
            self.check_source(s);
            assert!(Rc::ptr_eq(k, self.killer.as_ref().unwrap()));
            self.call(&format!("award_score={score}"))?;
            if self.mutate {
                self.killer = Some(Rc::new(3));
            }
            Ok(())
        }
        fn is_sleeping(&mut self) -> Result<bool, Self::Error> {
            self.call("sleeping")?;
            Ok(self.sleeping)
        }
        fn stop_sleeping(&mut self) -> Result<(), Self::Error> {
            self.call("stop_sleeping")?;
            self.sleeping = false;
            Ok(())
        }
        fn is_client_side(&mut self) -> Result<bool, Self::Error> {
            self.call("client")?;
            Ok(self.client)
        }
        fn has_custom_name(&mut self) -> Result<bool, Self::Error> {
            self.call("named")?;
            Ok(self.named)
        }
        fn combat_tracker_death_message(&mut self) -> Result<Self::Component, Self::Error> {
            self.call("death_message")?;
            Ok(Rc::new("actual component".into()))
        }
        fn component_get_string(&mut self, c: &Self::Component) -> Result<String, Self::Error> {
            self.call("get_string")?;
            Ok((**c).clone())
        }
        fn log_named_entity_death(&mut self, text: &str) -> Result<(), Self::Error> {
            assert_eq!(text, "actual component");
            self.call("log")
        }
        fn set_dead_field(&mut self, dead: bool) -> Result<(), Self::Error> {
            self.call("set_dead")?;
            self.dead = dead;
            Ok(())
        }
        fn combat_tracker_recheck_status(&mut self) -> Result<(), Self::Error> {
            assert!(self.dead);
            self.call("recheck")
        }
        fn current_server_level(&mut self) -> Result<Option<Self::ServerLevel>, Self::Error> {
            self.call("server_level")?;
            Ok(self.server.then(|| self.level.clone()))
        }
        fn causing_entity_killed_entity(
            &mut self,
            c: &Self::Entity,
            level: &Self::ServerLevel,
        ) -> Result<bool, Self::Error> {
            assert!(Rc::ptr_eq(c, self.causer.as_ref().unwrap()));
            assert!(Rc::ptr_eq(level, &self.level));
            self.call("killed_entity")?;
            if self.mutate {
                self.level = Rc::new(11);
            }
            Ok(self.allow_loot)
        }
        fn entity_die_game_event(&mut self) -> Result<(), Self::Error> {
            self.call("die_event")
        }
        fn drop_all_death_loot(
            &mut self,
            level: &Self::ServerLevel,
            s: Option<&Self::Source>,
        ) -> Result<(), Self::Error> {
            if let Some(s) = s {
                self.check_source(s);
            }
            self.call(&format!("drop={}", **level))?;
            self.drops += 1;
            if s.is_none() {
                return Err("nullable loot source");
            }
            Ok(())
        }
        fn create_wither_rose(&mut self, k: Option<&Self::Entity>) -> Result<(), Self::Error> {
            self.call(&format!("rose={:?}", k.map(|k| **k)))
        }
        fn broadcast_entity_event(&mut self, e: u8) -> Result<(), Self::Error> {
            self.call(&format!("broadcast={}:{}", **&self.level, e))
        }
        fn set_dying_pose(&mut self) -> Result<(), Self::Error> {
            self.call("dying_pose")
        }
    }
    impl PlayerDeathHost for Host {
        type Dimension = u32;
        type BlockPos = u32;
        fn reapply_position(&mut self) -> Result<(), Self::Error> {
            self.call("reapply")
        }
        fn is_spectator(&mut self) -> Result<bool, Self::Error> {
            self.call("spectator")?;
            Ok(self.spectator)
        }
        fn hurt_direction(&mut self) -> Result<f32, Self::Error> {
            self.call("hurt_direction")?;
            Ok(self.directions.pop_front().unwrap())
        }
        fn y_rotation(&mut self) -> Result<f32, Self::Error> {
            self.call("rotation")?;
            Ok(self.rotations.pop_front().unwrap())
        }
        fn mth_cos(&mut self, a: f32) -> Result<f32, Self::Error> {
            self.call("cos")?;
            self.angles.push(a);
            Ok(0.25)
        }
        fn mth_sin(&mut self, a: f32) -> Result<f32, Self::Error> {
            self.call("sin")?;
            self.angles.push(a);
            Ok(-0.5)
        }
        fn set_delta_movement(&mut self, x: f64, y: f64, z: f64) -> Result<(), Self::Error> {
            self.call("velocity")?;
            self.velocity = Some([x, y, z]);
            Ok(())
        }
        fn award_deaths_stat(&mut self) -> Result<(), Self::Error> {
            self.call("deaths")
        }
        fn reset_stat(&mut self, s: PlayerDeathResetStatistic) -> Result<(), Self::Error> {
            self.call(&format!("reset={s:?}"))
        }
        fn clear_fire(&mut self) -> Result<(), Self::Error> {
            self.call("clear_fire")
        }
        fn set_shared_flag_on_fire(&mut self, fire: bool) -> Result<(), Self::Error> {
            assert!(!fire);
            self.call("fire_flag")
        }
        fn current_dimension(&mut self) -> Result<u32, Self::Error> {
            self.call("dimension")?;
            Ok(*self.level)
        }
        fn block_position(&mut self) -> Result<u32, Self::Error> {
            self.call("block_position")?;
            Ok(19)
        }
        fn set_last_death_location(&mut self, d: u32, p: u32) -> Result<(), Self::Error> {
            self.call("last_death")?;
            self.death_location = Some((d, p));
            Ok(())
        }
    }

    #[test]
    fn generic_death_retains_killer_and_level_but_broadcast_rereads_world() {
        let mut h = Host::new();
        h.mutate = true;
        let source = h.source.clone();
        living_die(&mut h, Some(&source)).unwrap();
        assert_eq!(
            h.calls,
            [
                "removed",
                "dead_field",
                "causer",
                "kill_credit",
                "score",
                "score",
                "award_score=4",
                "sleeping",
                "stop_sleeping",
                "client",
                "named",
                "death_message",
                "get_string",
                "log",
                "set_dead",
                "recheck",
                "server_level",
                "killed_entity",
                "die_event",
                "drop=10",
                "rose=Some(2)",
                "broadcast=11:3",
                "dying_pose"
            ]
        );
        assert!(h.dead);
        assert_eq!(h.drops, 1);
    }

    #[test]
    fn veto_negative_score_nonserver_and_error_prefix() {
        let mut h = Host::new();
        h.allow_loot = false;
        h.score = -1;
        let s = h.source.clone();
        living_die(&mut h, Some(&s)).unwrap();
        assert!(!h
            .calls
            .iter()
            .any(|c| c.starts_with("award_score") || c == "die_event"));
        assert_eq!(
            &h.calls[h.calls.len() - 3..],
            ["killed_entity", "broadcast=10:3", "dying_pose"]
        );
        let mut h = Host::new();
        h.server = false;
        h.client = true;
        let s = h.source.clone();
        living_die(&mut h, Some(&s)).unwrap();
        assert!(!h
            .calls
            .iter()
            .any(|c| c == "named" || c.starts_with("broadcast=")));
        let mut h = Host::new();
        h.fail = Some("drop=10");
        let s = h.source.clone();
        assert_eq!(player_die(&mut h, Some(&s)), Err("owner failure"));
        assert!(h.dead);
        assert_eq!(h.calls.last().unwrap(), "drop=10");
        assert!(!h.calls.iter().any(|c| c == "reapply"));
    }

    #[test]
    fn player_tail_follows_parent_and_keeps_double_loot_and_float_order() {
        let mut h = Host::new();
        let s = h.source.clone();
        player_die(&mut h, Some(&s)).unwrap();
        assert_eq!(h.drops, 2);
        let parent_end = h.calls.iter().position(|c| c == "dying_pose").unwrap();
        assert_eq!(
            &h.calls[parent_end + 1..],
            [
                "reapply",
                "spectator",
                "server_level",
                "drop=10",
                "hurt_direction",
                "rotation",
                "cos",
                "hurt_direction",
                "rotation",
                "sin",
                "velocity",
                "deaths",
                "reset=TimeSinceDeath",
                "reset=TimeSinceRest",
                "clear_fire",
                "fire_flag",
                "dimension",
                "block_position",
                "last_death"
            ]
        );
        assert_eq!(
            h.angles,
            [40_f32 * 0.017453292_f32, 60_f32 * 0.017453292_f32]
        );
        assert_eq!(
            h.velocity.unwrap(),
            [
                f64::from(-0.25_f32 * 0.1_f32),
                f64::from(0.1_f32),
                f64::from(0.5_f32 * 0.1_f32)
            ]
        );
        assert_eq!(h.death_location, Some((10, 19)));
    }

    #[test]
    fn nullable_source_fails_only_at_original_site_and_parent_early_return_keeps_tail() {
        let mut h = Host::new();
        assert_eq!(living_die(&mut h, None), Err("null damage source"));
        assert_eq!(h.calls, ["removed", "dead_field", "null_source"]);
        let mut h = Host::new();
        h.removed = true;
        h.spectator = true;
        player_die(&mut h, None).unwrap();
        assert_eq!(
            &h.calls[..4],
            ["removed", "reapply", "spectator", "velocity"]
        );
        assert_eq!(h.velocity.unwrap(), [0., 0.1_f64, 0.]);
        assert!(!h
            .calls
            .iter()
            .any(|c| c == "dead_field" || c == "hurt_direction"));
        let mut h = Host::new();
        h.dead = true;
        assert_eq!(player_die(&mut h, None), Err("nullable loot source"));
        assert_eq!(
            h.calls,
            [
                "removed",
                "dead_field",
                "reapply",
                "spectator",
                "server_level",
                "drop=10"
            ]
        );
        assert_eq!(h.drops, 1);
    }
}
