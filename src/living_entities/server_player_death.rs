//! Source-ordered Java 1.21 `ServerPlayer.die` (`aqv.a(brk)`, lines 694–748).
//!
//! This override does not call either parent death body. It has no generic
//! removed/dead guard, dead-field write, health change or explicit DYING pose.
//! The executor owns no actor state, inventory, RNG, connection or schedule.
//! Every operation is a mandatory synchronous dispatch to its actual owner;
//! errors retain the already executed mutation/callback prefix. Loot, neutral
//! queries, scores, packets, menus/removal and respawn remain separate owners.

/// The actual team's current death-message visibility, read at each source
/// comparison. This value does not replace the team object or routing owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathMessageVisibility {
    Always,
    Never,
    HideForOtherTeams,
    HideForOwnTeam,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetStatistic {
    TimeSinceDeath,
    TimeSinceRest,
}

/// Required operations on the real ServerPlayer and retained source objects.
///
/// Opaque handles must preserve their actual lifetime/object identity. A killer,
/// team, message or callback belonging to the old actor must never resolve to a
/// replacement actor that later reuses its numeric entity ID. No callback has a
/// successful empty fallback when its actual owner is unavailable.
pub trait ServerPlayerDeathHost {
    type Error;
    type Source: ?Sized;
    type Component;
    type Team;
    type Killer;
    type EntityType;
    type ServerLevel;
    type Dimension;
    type BlockPos;

    fn entity_die_game_event(&mut self) -> Result<(), Self::Error>;
    /// Read the actual current level's showDeathMessages rule at this callsite.
    fn show_death_messages(&mut self) -> Result<bool, Self::Error>;
    fn combat_tracker_death_message(&mut self) -> Result<Self::Component, Self::Error>;
    fn entity_id(&mut self) -> Result<i32, Self::Error>;
    /// Send the initial ClientboundPlayerCombatKillPacket(id, message), with
    /// the original PacketSendListener.exceptionallySend registered on the real
    /// connection completion owner. Do not execute/prebuild the fallback here.
    ///
    /// When that original failure supplier actually runs, retain this actor and
    /// message identities: original.getString(256) -> literal/yellow Component
    /// -> death.attack.message_too_long; construct death.attack.even_more_magic
    /// with this actor's then-current display name and SHOW_TEXT hover of the
    /// prior message; reread this original actor's numeric ID for the fallback
    /// packet. A normal failed send follows that supplier's source behavior;
    /// it must not be replaced with an invented synchronous callback owner.
    /// Result::Err denotes a synchronous dispatch/owner failure. A later async
    /// completion failure invokes that retained supplier after die may already
    /// have continued or returned; it does not abort or roll back the death tail.
    fn send_combat_kill_with_fallback(
        &mut self,
        id: i32,
        message: &Self::Component,
    ) -> Result<(), Self::Error>;
    /// Actual CommonComponents.EMPTY; no death-message lookup or fallback send.
    fn empty_component(&mut self) -> Result<Self::Component, Self::Error>;
    fn send_combat_kill(&mut self, id: i32, message: &Self::Component) -> Result<(), Self::Error>;
    fn team(&mut self) -> Result<Option<Self::Team>, Self::Error>;
    fn death_message_visibility(
        &mut self,
        team: &Self::Team,
    ) -> Result<DeathMessageVisibility, Self::Error>;
    /// Actual PlayerList.broadcastSystemMessage(message, overlay).
    fn broadcast_system_message(
        &mut self,
        message: &Self::Component,
        overlay: bool,
    ) -> Result<(), Self::Error>;
    /// Actual PlayerList.broadcastSystemToTeam(this, message), including its
    /// current victim-team query and real audience. Do not substitute a saved
    /// Team handle for the source operation's Player parameter.
    fn broadcast_system_to_team(&mut self, message: &Self::Component) -> Result<(), Self::Error>;
    /// Actual PlayerList.broadcastSystemToAllExceptTeam(this, message).
    fn broadcast_system_to_all_except_team(
        &mut self,
        message: &Self::Component,
    ) -> Result<(), Self::Error>;

    /// Actual Player.removeEntitiesOnShoulder with its owned RNG/world work.
    fn remove_entities_on_shoulder(&mut self) -> Result<(), Self::Error>;
    fn forgive_dead_players(&mut self) -> Result<bool, Self::Error>;
    /// Actual tellNeutralMobsThatIDied (751–755): current block-position box
    /// inflated by (32,10,32), real Mob query/NO_SPECTATORS predicate, NeutralMob
    /// filter, then each playerDied callback in the source query order. An empty
    /// result requires the actual query's proof; there is no fabricated AI owner.
    fn tell_neutral_mobs_that_i_died(&mut self) -> Result<(), Self::Error>;
    fn is_spectator(&mut self) -> Result<bool, Self::Error>;
    fn server_level(&mut self) -> Result<Self::ServerLevel, Self::Error>;
    fn drop_all_death_loot(
        &mut self,
        level: &Self::ServerLevel,
        source: &Self::Source,
    ) -> Result<(), Self::Error>;
    /// Actual scoreboard.forAllObjectives(DEATH_COUNT, this, ScoreAccess::increment).
    fn increment_death_count_objectives(&mut self) -> Result<(), Self::Error>;
    fn kill_credit(&mut self) -> Result<Option<Self::Killer>, Self::Error>;
    fn killer_entity_type(
        &mut self,
        killer: &Self::Killer,
    ) -> Result<Self::EntityType, Self::Error>;
    /// Actual victim.awardStat(Stats.ENTITY_KILLED_BY.get(killerType)).
    fn award_entity_killed_by_stat(
        &mut self,
        entity_type: &Self::EntityType,
    ) -> Result<(), Self::Error>;
    /// Read this actor's actual deathScore only after the preceding stat callback.
    fn death_score(&mut self) -> Result<i32, Self::Error>;
    fn killer_award_kill_score(
        &mut self,
        killer: &Self::Killer,
        score: i32,
        source: &Self::Source,
    ) -> Result<(), Self::Error>;
    fn create_wither_rose(&mut self, killer: &Self::Killer) -> Result<(), Self::Error>;

    fn broadcast_entity_event(&mut self, event: u8) -> Result<(), Self::Error>;
    fn award_deaths_stat(&mut self) -> Result<(), Self::Error>;
    fn reset_stat(&mut self, statistic: ResetStatistic) -> Result<(), Self::Error>;
    fn clear_fire(&mut self) -> Result<(), Self::Error>;
    fn set_ticks_frozen(&mut self, ticks: i32) -> Result<(), Self::Error>;
    fn set_shared_flag_on_fire(&mut self, on_fire: bool) -> Result<(), Self::Error>;
    fn combat_tracker_recheck_status(&mut self) -> Result<(), Self::Error>;
    fn current_dimension(&mut self) -> Result<Self::Dimension, Self::Error>;
    fn block_position(&mut self) -> Result<Self::BlockPos, Self::Error>;
    /// Actual setLastDeathLocation(Optional.of(GlobalPos.of(dimension, position))).
    fn set_last_death_location(
        &mut self,
        dimension: Self::Dimension,
        position: Self::BlockPos,
    ) -> Result<(), Self::Error>;
}

/// Execute this override once the actual virtual dispatch selects ServerPlayer.
/// No health guard or parent die call is inserted. Later death ticks, removal,
/// inventory-menu cleanup, retained connection and replacement actor remain the
/// caller's real lifecycle obligations, not implied by this function returning.
pub fn die<H: ServerPlayerDeathHost + ?Sized>(
    host: &mut H,
    source: &H::Source,
) -> Result<(), H::Error> {
    host.entity_die_game_event()?;
    let show_messages = host.show_death_messages()?;
    if show_messages {
        let message = host.combat_tracker_death_message()?;
        let id = host.entity_id()?;
        host.send_combat_kill_with_fallback(id, &message)?;
        let team = host.team()?;
        if let Some(team) = team {
            if host.death_message_visibility(&team)? == DeathMessageVisibility::Always {
                host.broadcast_system_message(&message, false)?;
            } else if host.death_message_visibility(&team)?
                == DeathMessageVisibility::HideForOtherTeams
            {
                host.broadcast_system_to_team(&message)?;
            } else if host.death_message_visibility(&team)?
                == DeathMessageVisibility::HideForOwnTeam
            {
                host.broadcast_system_to_all_except_team(&message)?;
            }
        } else {
            host.broadcast_system_message(&message, false)?;
        }
    } else {
        let id = host.entity_id()?;
        let message = host.empty_component()?;
        host.send_combat_kill(id, &message)?;
    }
    host.remove_entities_on_shoulder()?;
    if host.forgive_dead_players()? {
        host.tell_neutral_mobs_that_i_died()?;
    }
    if !host.is_spectator()? {
        let level = host.server_level()?;
        host.drop_all_death_loot(&level, source)?;
    }
    host.increment_death_count_objectives()?;
    if let Some(killer) = host.kill_credit()? {
        let entity_type = host.killer_entity_type(&killer)?;
        host.award_entity_killed_by_stat(&entity_type)?;
        let score = host.death_score()?;
        host.killer_award_kill_score(&killer, score, source)?;
        host.create_wither_rose(&killer)?;
    }
    host.broadcast_entity_event(3)?;
    host.award_deaths_stat()?;
    host.reset_stat(ResetStatistic::TimeSinceDeath)?;
    host.reset_stat(ResetStatistic::TimeSinceRest)?;
    host.clear_fire()?;
    host.set_ticks_frozen(0)?;
    host.set_shared_flag_on_fire(false)?;
    host.combat_tracker_recheck_status()?;
    let dimension = host.current_dimension()?;
    let position = host.block_position()?;
    host.set_last_death_location(dimension, position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    // A call recorder; no real actor/network/scoreboard/loot owner is claimed.
    struct Host {
        calls: Vec<String>,
        show_messages: bool,
        has_team: bool,
        visibilities: VecDeque<DeathMessageVisibility>,
        forgive: bool,
        spectator: bool,
        killer: Option<u64>,
        score: i32,
        fail_at: Option<&'static str>,
    }
    impl Default for Host {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                show_messages: true,
                has_team: false,
                visibilities: VecDeque::new(),
                forgive: false,
                spectator: false,
                killer: None,
                score: 4,
                fail_at: None,
            }
        }
    }
    impl Host {
        fn call(&mut self, call: impl Into<String>) -> Result<(), &'static str> {
            let call = call.into();
            let fail = self.fail_at.is_some_and(|at| call == at);
            self.calls.push(call);
            if fail {
                Err("injected owner failure")
            } else {
                Ok(())
            }
        }
    }
    impl ServerPlayerDeathHost for Host {
        type Error = &'static str;
        type Source = u64;
        type Component = u32;
        type Team = u64;
        type Killer = u64;
        type EntityType = u32;
        type ServerLevel = u64;
        type Dimension = u64;
        type BlockPos = [i32; 3];
        fn entity_die_game_event(&mut self) -> Result<(), Self::Error> {
            self.call("entity_die")
        }
        fn show_death_messages(&mut self) -> Result<bool, Self::Error> {
            self.call("show_messages")?;
            Ok(self.show_messages)
        }
        fn combat_tracker_death_message(&mut self) -> Result<u32, Self::Error> {
            self.call("death_message")?;
            Ok(7)
        }
        fn entity_id(&mut self) -> Result<i32, Self::Error> {
            self.call("entity_id")?;
            Ok(42)
        }
        fn send_combat_kill_with_fallback(
            &mut self,
            id: i32,
            message: &u32,
        ) -> Result<(), Self::Error> {
            self.call(format!("send_with_fallback={id},{message}"))
        }
        fn empty_component(&mut self) -> Result<u32, Self::Error> {
            self.call("empty_component")?;
            Ok(0)
        }
        fn send_combat_kill(&mut self, id: i32, message: &u32) -> Result<(), Self::Error> {
            self.call(format!("send={id},{message}"))
        }
        fn team(&mut self) -> Result<Option<u64>, Self::Error> {
            self.call("team")?;
            Ok(self.has_team.then_some(55))
        }
        fn death_message_visibility(
            &mut self,
            team: &u64,
        ) -> Result<DeathMessageVisibility, Self::Error> {
            assert_eq!(*team, 55);
            self.call("visibility")?;
            Ok(self
                .visibilities
                .pop_front()
                .expect("source visibility read"))
        }
        fn broadcast_system_message(
            &mut self,
            message: &u32,
            overlay: bool,
        ) -> Result<(), Self::Error> {
            self.call(format!("broadcast_all={message},{overlay}"))
        }
        fn broadcast_system_to_team(&mut self, message: &u32) -> Result<(), Self::Error> {
            self.call(format!("broadcast_team={message}"))
        }
        fn broadcast_system_to_all_except_team(
            &mut self,
            message: &u32,
        ) -> Result<(), Self::Error> {
            self.call(format!("broadcast_other_teams={message}"))
        }
        fn remove_entities_on_shoulder(&mut self) -> Result<(), Self::Error> {
            self.call("shoulders")
        }
        fn forgive_dead_players(&mut self) -> Result<bool, Self::Error> {
            self.call("forgive")?;
            Ok(self.forgive)
        }
        fn tell_neutral_mobs_that_i_died(&mut self) -> Result<(), Self::Error> {
            self.call("neutral_query_and_callbacks")
        }
        fn is_spectator(&mut self) -> Result<bool, Self::Error> {
            self.call("spectator")?;
            Ok(self.spectator)
        }
        fn server_level(&mut self) -> Result<u64, Self::Error> {
            self.call("server_level")?;
            Ok(88)
        }
        fn drop_all_death_loot(&mut self, level: &u64, source: &u64) -> Result<(), Self::Error> {
            assert_eq!(*level, 88);
            assert_eq!(*source, 767);
            self.call("loot")
        }
        fn increment_death_count_objectives(&mut self) -> Result<(), Self::Error> {
            self.call("death_count_objectives")
        }
        fn kill_credit(&mut self) -> Result<Option<u64>, Self::Error> {
            self.call("kill_credit")?;
            Ok(self.killer)
        }
        fn killer_entity_type(&mut self, killer: &u64) -> Result<u32, Self::Error> {
            assert_eq!(*killer, 77);
            self.call("killer_type")?;
            Ok(90)
        }
        fn award_entity_killed_by_stat(&mut self, entity_type: &u32) -> Result<(), Self::Error> {
            assert_eq!(*entity_type, 90);
            self.call("entity_killed_by_stat")?;
            self.score = -9;
            Ok(())
        }
        fn death_score(&mut self) -> Result<i32, Self::Error> {
            self.call("death_score")?;
            Ok(self.score)
        }
        fn killer_award_kill_score(
            &mut self,
            killer: &u64,
            score: i32,
            source: &u64,
        ) -> Result<(), Self::Error> {
            assert_eq!(*killer, 77);
            assert_eq!(*source, 767);
            self.call(format!("award_kill_score={score}"))
        }
        fn create_wither_rose(&mut self, killer: &u64) -> Result<(), Self::Error> {
            assert_eq!(*killer, 77);
            self.call("wither_rose")
        }
        fn broadcast_entity_event(&mut self, event: u8) -> Result<(), Self::Error> {
            self.call(format!("event={event}"))
        }
        fn award_deaths_stat(&mut self) -> Result<(), Self::Error> {
            self.call("deaths_stat")
        }
        fn reset_stat(&mut self, statistic: ResetStatistic) -> Result<(), Self::Error> {
            self.call(format!("reset={statistic:?}"))
        }
        fn clear_fire(&mut self) -> Result<(), Self::Error> {
            self.call("clear_fire")
        }
        fn set_ticks_frozen(&mut self, ticks: i32) -> Result<(), Self::Error> {
            self.call(format!("frozen={ticks}"))
        }
        fn set_shared_flag_on_fire(&mut self, on: bool) -> Result<(), Self::Error> {
            self.call(format!("shared_fire={on}"))
        }
        fn combat_tracker_recheck_status(&mut self) -> Result<(), Self::Error> {
            self.call("combat_recheck")
        }
        fn current_dimension(&mut self) -> Result<u64, Self::Error> {
            self.call("current_dimension")?;
            Ok(11)
        }
        fn block_position(&mut self) -> Result<[i32; 3], Self::Error> {
            self.call("block_position")?;
            Ok([1, 2, 3])
        }
        fn set_last_death_location(
            &mut self,
            dimension: u64,
            position: [i32; 3],
        ) -> Result<(), Self::Error> {
            assert_eq!(dimension, 11);
            assert_eq!(position, [1, 2, 3]);
            self.call("last_death_location")
        }
    }
    #[test]
    fn full_override_preserves_message_loot_killer_and_tail_order() {
        let mut host = Host {
            forgive: true,
            killer: Some(77),
            ..Host::default()
        };
        assert_eq!(die(&mut host, &767), Ok(()));
        assert_eq!(
            host.calls,
            [
                "entity_die",
                "show_messages",
                "death_message",
                "entity_id",
                "send_with_fallback=42,7",
                "team",
                "broadcast_all=7,false",
                "shoulders",
                "forgive",
                "neutral_query_and_callbacks",
                "spectator",
                "server_level",
                "loot",
                "death_count_objectives",
                "kill_credit",
                "killer_type",
                "entity_killed_by_stat",
                "death_score",
                "award_kill_score=-9",
                "wither_rose",
                "event=3",
                "deaths_stat",
                "reset=TimeSinceDeath",
                "reset=TimeSinceRest",
                "clear_fire",
                "frozen=0",
                "shared_fire=false",
                "combat_recheck",
                "current_dimension",
                "block_position",
                "last_death_location"
            ]
        );
    }
    #[test]
    fn team_visibility_is_reread_and_disabled_message_spectator_path_has_own_gates() {
        let mut host = Host {
            has_team: true,
            visibilities: [
                DeathMessageVisibility::Never,
                DeathMessageVisibility::HideForOwnTeam,
                DeathMessageVisibility::HideForOwnTeam,
            ]
            .into(),
            ..Host::default()
        };
        die(&mut host, &767).unwrap();
        assert_eq!(
            &host.calls[5..10],
            [
                "team",
                "visibility",
                "visibility",
                "visibility",
                "broadcast_other_teams=7"
            ]
        );
        let mut hidden = Host {
            show_messages: false,
            spectator: true,
            ..Host::default()
        };
        die(&mut hidden, &767).unwrap();
        assert_eq!(
            hidden.calls,
            [
                "entity_die",
                "show_messages",
                "entity_id",
                "empty_component",
                "send=42,0",
                "shoulders",
                "forgive",
                "spectator",
                "death_count_objectives",
                "kill_credit",
                "event=3",
                "deaths_stat",
                "reset=TimeSinceDeath",
                "reset=TimeSinceRest",
                "clear_fire",
                "frozen=0",
                "shared_fire=false",
                "combat_recheck",
                "current_dimension",
                "block_position",
                "last_death_location"
            ]
        );
    }
    #[test]
    fn later_owner_failure_preserves_earlier_callbacks_and_stops_tail() {
        let mut host = Host {
            fail_at: Some("entity_killed_by_stat"),
            killer: Some(77),
            ..Host::default()
        };
        assert_eq!(die(&mut host, &767), Err("injected owner failure"));
        assert!(host.calls.iter().any(|call| call == "loot"));
        assert!(host
            .calls
            .iter()
            .any(|call| call == "death_count_objectives"));
        assert_eq!(host.calls.last().unwrap(), "entity_killed_by_stat");
        assert!(!host.calls.iter().any(|call| call == "event=3"));
        let mut network = Host {
            fail_at: Some("send_with_fallback=42,7"),
            ..Host::default()
        };
        assert_eq!(die(&mut network, &767), Err("injected owner failure"));
        assert_eq!(
            network.calls,
            [
                "entity_die",
                "show_messages",
                "death_message",
                "entity_id",
                "send_with_fallback=42,7"
            ]
        );
    }
}
