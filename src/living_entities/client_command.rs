//! Java 1.21 `ServerGamePacketListenerImpl.handleClientCommand`, source
//! 1659–1683, with mandatory actual packet/listener/player/server owners.
//!
//! This is an ordered executor, not a packet decoder, connection, clock,
//! scheduler or player implementation. Original server/mapping `javap` evidence
//! is recorded in `docs/reference/entities-survival-death-respawn.md`; no Main
//! callsite or native respawn owner is supplied by this module.

/// The original packet's already decoded enum. Wire decoding and invalid enum
/// rejection remain the existing protocol owner's responsibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientCommandAction {
    PerformRespawn,
    RequestStats,
}

/// Exact Entity.RemovalReason values selected by this source method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RespawnRemovalReason {
    ChangedDimension,
    Killed,
}

/// Access to the actual listener's current player, original decoded packet,
/// server thread/PlayerList and native statistics/game-rule owners.
///
/// `Player` identifies a real actor lifetime/generation, not just its reusable
/// wire ID. Every `player` call reads the listener field at that callsite. The
/// handler deliberately retains neither one player snapshot nor a replacement
/// player across subsequent source field reads. Respawn must construct a real
/// new actor generation with the old wire ID and retain the existing listener,
/// connection and its transport, teleport, chat/signature, ACK and rate owners.
/// No callback has a default implementation. An unsupported owner must return
/// an explicit error, never successful no-op restoration, fabricated statistics,
/// a replacement session or a health-only field patch.
///
/// Synchronous errors preserve the completed source prefix. In particular,
/// clearing wonGame or sending part of respawn is not rolled back when a later
/// owner fails. Callers must not blindly retry this as a transaction.
pub trait ClientCommandHost {
    type Error;
    type Packet;
    type Player;
    type Level;
    type PlayerList;
    type GameRules;
    type BooleanRule;
    type Stats;

    fn player(&mut self) -> Result<Self::Player, Self::Error>;
    /// Actual ServerPlayer.serverLevel(), used by the original thread check.
    fn server_level(&mut self, player: &Self::Player) -> Result<Self::Level, Self::Error>;
    /// Full PacketUtils.ensureRunningOnSameThread(packet, thisListener, level).
    /// The actual original packet/listener/thread owner must perform rescheduling
    /// and its stop-current-handler control flow when required. An error here
    /// stops before resetLastActionTime. Do not synthesize a queue or claim that
    /// this synchronous executor itself runs anything in the background.
    fn ensure_running_on_same_thread(
        &mut self,
        packet: &Self::Packet,
        level: &Self::Level,
    ) -> Result<(), Self::Error>;
    /// Actual ServerPlayer.resetLastActionTime with its existing clock owner.
    fn reset_last_action_time(&mut self, player: &Self::Player) -> Result<(), Self::Error>;
    /// Read the native packet's already decoded action field; do not decode or
    /// validate wire input a second time. Source reads this after action reset.
    fn action(&mut self, packet: &Self::Packet) -> Result<ClientCommandAction, Self::Error>;
    fn won_game(&mut self, player: &Self::Player) -> Result<bool, Self::Error>;
    fn set_won_game(&mut self, player: &Self::Player, won_game: bool) -> Result<(), Self::Error>;
    /// Actual existing server.getPlayerList(), read before the respawn actor.
    fn player_list(&mut self) -> Result<Self::PlayerList, Self::Error>;
    /// Full PlayerList.respawn(old, alive, exactReason), source 469–530. This may
    /// compose `living_entities::respawn::respawn` only with its complete actual
    /// constructor/restore/world/spawn/menu/packet/RNG owners. Return the real
    /// replacement actor after source insertion/output; never reuse the old
    /// generation or run login admission/CONFIGURATION/connection construction.
    fn respawn(
        &mut self,
        player_list: &Self::PlayerList,
        old: &Self::Player,
        alive: bool,
        reason: RespawnRemovalReason,
    ) -> Result<Self::Player, Self::Error>;
    /// Assign the returned actual actor to this listener's player field. The
    /// surrounding central adapter must replace its actor/menu/physical refs
    /// and generation-qualified callbacks, retaining the same connection owner.
    fn set_player(&mut self, player: Self::Player) -> Result<(), Self::Error>;
    /// Actual CriteriaTriggers.CHANGED_DIMENSION.trigger(currentPlayer, END,
    /// OVERWORLD), with the real advancement owner and fixed source dimensions.
    fn trigger_end_to_overworld(&mut self, player: &Self::Player) -> Result<(), Self::Error>;
    /// Read actual LivingEntity.getHealth, not a saved snapshot or alive flag.
    fn health(&mut self, player: &Self::Player) -> Result<f32, Self::Error>;
    fn server_is_hardcore(&mut self) -> Result<bool, Self::Error>;
    /// Actual ServerPlayer.setGameMode(SPECTATOR), including its side effects;
    /// the source discards this operation's boolean result.
    fn set_game_mode_spectator(&mut self, player: &Self::Player) -> Result<(), Self::Error>;
    /// Actual current Entity.level(), re-read after setGameMode callbacks.
    fn level(&mut self, player: &Self::Player) -> Result<Self::Level, Self::Error>;
    fn game_rules(&mut self, level: &Self::Level) -> Result<Self::GameRules, Self::Error>;
    fn spectators_generate_chunks_rule(
        &mut self,
        rules: &Self::GameRules,
    ) -> Result<Self::BooleanRule, Self::Error>;
    /// Actual BooleanValue.set(value, existingServer), retaining rule callbacks
    /// and packet/world consequences, rather than assigning an isolated bool.
    fn set_boolean_rule(
        &mut self,
        rule: &Self::BooleanRule,
        value: bool,
    ) -> Result<(), Self::Error>;
    fn stats(&mut self, player: &Self::Player) -> Result<Self::Stats, Self::Error>;
    /// Actual retained ServerStatsCounter.sendStats(currentPlayer), source
    /// 171–178: drain the real dirty set before connection send, including an
    /// empty delta packet, through the existing ordered output owner.
    fn send_stats(&mut self, stats: &Self::Stats, player: &Self::Player)
        -> Result<(), Self::Error>;
}

/// Handle one actual already decoded command at the original listener phase.
/// Health eligibility, credits/dimension-change and hardcore behavior are kept
/// here; death/removal/respawn implementations remain their real native owners.
pub fn handle_client_command<H: ClientCommandHost + ?Sized>(
    host: &mut H,
    packet: &H::Packet,
) -> Result<(), H::Error> {
    let player = host.player()?;
    let level = host.server_level(&player)?;
    host.ensure_running_on_same_thread(packet, &level)?;
    let player = host.player()?;
    host.reset_last_action_time(&player)?;
    match host.action(packet)? {
        ClientCommandAction::PerformRespawn => {
            let player = host.player()?;
            if host.won_game(&player)? {
                let player = host.player()?;
                host.set_won_game(&player, false)?;
                let player_list = host.player_list()?;
                let player = host.player()?;
                let replacement = host.respawn(
                    &player_list,
                    &player,
                    true,
                    RespawnRemovalReason::ChangedDimension,
                )?;
                host.set_player(replacement)?;
                let player = host.player()?;
                host.trigger_end_to_overworld(&player)?;
            } else {
                let player = host.player()?;
                if host.health(&player)? > 0.0 {
                    return Ok(());
                }
                let player_list = host.player_list()?;
                let player = host.player()?;
                let replacement =
                    host.respawn(&player_list, &player, false, RespawnRemovalReason::Killed)?;
                host.set_player(replacement)?;
                if host.server_is_hardcore()? {
                    let player = host.player()?;
                    host.set_game_mode_spectator(&player)?;
                    let player = host.player()?;
                    let level = host.level(&player)?;
                    let rules = host.game_rules(&level)?;
                    let rule = host.spectators_generate_chunks_rule(&rules)?;
                    host.set_boolean_rule(&rule, false)?;
                }
            }
        }
        ClientCommandAction::RequestStats => {
            let player = host.player()?;
            let stats = host.stats(&player)?;
            let player = host.player()?;
            host.send_stats(&stats, &player)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Controlled callback identities and effects only; no actual actor, native
    // packet, clock, thread, connection or statistics owner is supplied here.
    struct Host {
        calls: Vec<String>,
        current: u64,
        won: bool,
        health: f32,
        hardcore: bool,
        fail_at: Option<&'static str>,
        mutate_thread: bool,
        mutate_reset: bool,
        mutate_won: bool,
        mutate_set_won: bool,
        mutate_player_list: bool,
        mutate_set_player: bool,
        mutate_spectator: bool,
        mutate_stats: bool,
    }

    impl Default for Host {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                current: 10,
                won: false,
                health: 0.0,
                hardcore: false,
                fail_at: None,
                mutate_thread: false,
                mutate_reset: false,
                mutate_won: false,
                mutate_set_won: false,
                mutate_player_list: false,
                mutate_set_player: false,
                mutate_spectator: false,
                mutate_stats: false,
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
    }

    impl ClientCommandHost for Host {
        type Error = &'static str;
        type Packet = ClientCommandAction;
        type Player = u64;
        type Level = u64;
        type PlayerList = u64;
        type GameRules = u64;
        type BooleanRule = u64;
        type Stats = u64;

        fn player(&mut self) -> Result<u64, Self::Error> {
            self.read("player", self.current)
        }
        fn server_level(&mut self, player: &u64) -> Result<u64, Self::Error> {
            self.read(format!("server-level:{player}"), *player + 100)
        }
        fn ensure_running_on_same_thread(
            &mut self,
            packet: &Self::Packet,
            level: &u64,
        ) -> Result<(), Self::Error> {
            self.call(format!("thread:{packet:?}:{level}"))?;
            if self.mutate_thread {
                self.current = 11;
            }
            Ok(())
        }
        fn reset_last_action_time(&mut self, player: &u64) -> Result<(), Self::Error> {
            self.call(format!("reset:{player}"))?;
            if self.mutate_reset {
                self.current = 12;
            }
            Ok(())
        }
        fn action(&mut self, packet: &Self::Packet) -> Result<ClientCommandAction, Self::Error> {
            self.read("action", *packet)
        }
        fn won_game(&mut self, player: &u64) -> Result<bool, Self::Error> {
            self.call(format!("won:{player}"))?;
            if self.mutate_won {
                self.current = 13;
            }
            Ok(self.won)
        }
        fn set_won_game(&mut self, player: &u64, won: bool) -> Result<(), Self::Error> {
            self.call(format!("set-won:{player}:{won}"))?;
            self.won = won;
            if self.mutate_set_won {
                self.current = 14;
            }
            Ok(())
        }
        fn player_list(&mut self) -> Result<u64, Self::Error> {
            self.call("player-list")?;
            if self.mutate_player_list {
                self.current = 15;
            }
            Ok(200)
        }
        fn respawn(
            &mut self,
            list: &u64,
            player: &u64,
            alive: bool,
            reason: RespawnRemovalReason,
        ) -> Result<u64, Self::Error> {
            self.read(format!("respawn:{list}:{player}:{alive}:{reason:?}"), 20)
        }
        fn set_player(&mut self, player: u64) -> Result<(), Self::Error> {
            self.call(format!("set-player:{player}"))?;
            self.current = if self.mutate_set_player { 21 } else { player };
            Ok(())
        }
        fn trigger_end_to_overworld(&mut self, player: &u64) -> Result<(), Self::Error> {
            self.call(format!("criterion:{player}:END:OVERWORLD"))
        }
        fn health(&mut self, player: &u64) -> Result<f32, Self::Error> {
            self.read(format!("health:{player}"), self.health)
        }
        fn server_is_hardcore(&mut self) -> Result<bool, Self::Error> {
            self.read("hardcore", self.hardcore)
        }
        fn set_game_mode_spectator(&mut self, player: &u64) -> Result<(), Self::Error> {
            self.call(format!("spectator:{player}"))?;
            if self.mutate_spectator {
                self.current = 22;
            }
            Ok(())
        }
        fn level(&mut self, player: &u64) -> Result<u64, Self::Error> {
            self.read(format!("level:{player}"), *player + 100)
        }
        fn game_rules(&mut self, level: &u64) -> Result<u64, Self::Error> {
            self.read(format!("rules:{level}"), *level + 100)
        }
        fn spectators_generate_chunks_rule(&mut self, rules: &u64) -> Result<u64, Self::Error> {
            self.read(format!("spectator-rule:{rules}"), *rules + 100)
        }
        fn set_boolean_rule(&mut self, rule: &u64, value: bool) -> Result<(), Self::Error> {
            self.call(format!("set-rule:{rule}:{value}"))
        }
        fn stats(&mut self, player: &u64) -> Result<u64, Self::Error> {
            self.call(format!("stats:{player}"))?;
            if self.mutate_stats {
                self.current = 23;
            }
            Ok(*player + 500)
        }
        fn send_stats(&mut self, stats: &u64, player: &u64) -> Result<(), Self::Error> {
            self.call(format!("send-stats:{stats}:{player}"))
        }
    }

    #[test]
    fn won_game_rereads_listener_actor_and_retains_exact_alive_dimension_branch() {
        let mut host = Host {
            won: true,
            health: 20.0,
            hardcore: true,
            mutate_thread: true,
            mutate_reset: true,
            mutate_won: true,
            mutate_set_won: true,
            mutate_player_list: true,
            mutate_set_player: true,
            ..Host::default()
        };
        handle_client_command(&mut host, &ClientCommandAction::PerformRespawn).unwrap();
        assert_eq!(
            host.calls,
            [
                "player",
                "server-level:10",
                "thread:PerformRespawn:110",
                "player",
                "reset:11",
                "action",
                "player",
                "won:12",
                "player",
                "set-won:13:false",
                "player-list",
                "player",
                "respawn:200:15:true:ChangedDimension",
                "set-player:20",
                "player",
                "criterion:21:END:OVERWORLD",
            ]
        );
        assert!(!host.won);
    }

    #[test]
    fn ordinary_positive_health_returns_after_reset_without_respawn() {
        let mut host = Host {
            health: f32::MIN_POSITIVE,
            ..Host::default()
        };
        handle_client_command(&mut host, &ClientCommandAction::PerformRespawn).unwrap();
        assert_eq!(
            host.calls,
            [
                "player",
                "server-level:10",
                "thread:PerformRespawn:110",
                "player",
                "reset:10",
                "action",
                "player",
                "won:10",
                "player",
                "health:10",
            ]
        );
    }

    #[test]
    fn dead_hardcore_changes_new_actor_then_queries_fresh_current_actor_level_rule() {
        let mut host = Host {
            health: -0.0,
            hardcore: true,
            mutate_player_list: true,
            mutate_set_player: true,
            mutate_spectator: true,
            ..Host::default()
        };
        handle_client_command(&mut host, &ClientCommandAction::PerformRespawn).unwrap();
        assert_eq!(
            &host.calls[10..],
            [
                "player-list",
                "player",
                "respawn:200:15:false:Killed",
                "set-player:20",
                "hardcore",
                "player",
                "spectator:21",
                "player",
                "level:22",
                "rules:122",
                "spectator-rule:222",
                "set-rule:322:false",
            ]
        );
        assert_eq!(host.current, 22);

        let mut nan = Host {
            health: f32::NAN,
            ..Host::default()
        };
        handle_client_command(&mut nan, &ClientCommandAction::PerformRespawn).unwrap();
        assert!(nan.calls.contains(&"respawn:200:10:false:Killed".into()));
        assert_eq!(nan.calls.last().unwrap(), "hardcore");
    }

    #[test]
    fn stats_retains_first_counter_but_rereads_listener_player_for_send() {
        let mut host = Host {
            mutate_reset: true,
            mutate_stats: true,
            ..Host::default()
        };
        handle_client_command(&mut host, &ClientCommandAction::RequestStats).unwrap();
        assert_eq!(
            host.calls,
            [
                "player",
                "server-level:10",
                "thread:RequestStats:110",
                "player",
                "reset:10",
                "action",
                "player",
                "stats:12",
                "player",
                "send-stats:512:23",
            ]
        );
    }

    #[test]
    fn thread_respawn_and_rule_errors_stop_at_committed_source_prefix() {
        let mut thread = Host {
            fail_at: Some("thread:PerformRespawn:110"),
            ..Host::default()
        };
        assert_eq!(
            handle_client_command(&mut thread, &ClientCommandAction::PerformRespawn),
            Err("owner failure")
        );
        assert_eq!(
            thread.calls,
            ["player", "server-level:10", "thread:PerformRespawn:110"]
        );

        let mut respawn = Host {
            won: true,
            fail_at: Some("respawn:200:10:true:ChangedDimension"),
            ..Host::default()
        };
        assert_eq!(
            handle_client_command(&mut respawn, &ClientCommandAction::PerformRespawn),
            Err("owner failure")
        );
        assert!(!respawn.won); // Source clear survives failure; no listener replacement.
        assert_eq!(respawn.current, 10);
        assert_eq!(
            respawn.calls.last().unwrap(),
            "respawn:200:10:true:ChangedDimension"
        );

        let mut rule = Host {
            hardcore: true,
            fail_at: Some("set-rule:320:false"),
            ..Host::default()
        };
        assert_eq!(
            handle_client_command(&mut rule, &ClientCommandAction::PerformRespawn),
            Err("owner failure")
        );
        assert_eq!(rule.current, 20); // Successful actor replacement remains committed.
        assert!(rule.calls.contains(&"spectator:20".into()));
        assert_eq!(rule.calls.last().unwrap(), "set-rule:320:false");
    }
}
