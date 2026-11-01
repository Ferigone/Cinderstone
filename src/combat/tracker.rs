//! Java 1.21 CombatTracker's damage history and combat-status lifecycle.
//!
//! Source: pinned `brh.recordDamage` (40–55), `shouldEnterCombat` (58),
//! `getCombatDuration` (163–166), `recheckStatus` (171–184), and `brf`
//! CombatEntry. This is the state owner for those original tracker fields.
//! Actor health/aliveness, tick count, fall/world facts, callbacks and damage
//! sources remain with their actual owners. No actor or scheduler is created.
//!
//! Death-message Components, most-significant-fall selection and actor kill
//! credit are outside this module's implemented scope. The retained entries
//! do not substitute for any of those callbacks or results.

pub const RESET_DAMAGE_STATUS_TIME: i32 = 100;
pub const RESET_COMBAT_STATUS_TIME: i32 = 300;

/// The original immutable CombatEntry fields, in insertion order in a tracker.
///
/// S must be a retained handle to the canonical DamageSource. Cloning S when
/// recording must retain that same source identity, direct/causing entities,
/// holder and source position; it must not reconstruct a source from an ID.
/// F is the actual FallLocation value returned by the actor/world owner.
#[derive(Debug)]
pub struct CombatEntry<S, F> {
    source: S,
    damage: f32,
    fall_location: Option<F>,
    fall_distance: f32,
}

impl<S, F> CombatEntry<S, F> {
    pub fn source(&self) -> &S {
        &self.source
    }

    pub fn damage(&self) -> f32 {
        self.damage
    }

    pub fn fall_location(&self) -> Option<&F> {
        self.fall_location.as_ref()
    }

    pub fn fall_distance(&self) -> f32 {
        self.fall_distance
    }
}

/// Required operations on the actual actor associated with this tracker.
///
/// None has a dummy implementation. `actor_is_alive` is the real
/// LivingEntity.isAlive result, not a separately maintained tracker flag.
/// current_fall_location must perform FallLocation.getCurrentFallLocation:
/// query the last climbable position's actual block first, then the actor's
/// water state only when no climbable position is present; nullable is normal.
///
/// Callbacks receive this exact tracker with the preceding source mutations
/// already committed, allowing their ordinary reads or synchronous reentry.
/// If a callback fails, earlier tracker/actor mutations are not rolled back.
pub trait CombatTrackerHost<S, F> {
    type Error;

    fn actor_is_alive(&mut self) -> Result<bool, Self::Error>;
    fn actor_tick_count(&mut self) -> Result<i32, Self::Error>;
    fn current_fall_location(&mut self) -> Result<Option<F>, Self::Error>;
    fn fall_distance(&mut self) -> Result<f32, Self::Error>;
    /// DamageSource.getEntity() instanceof LivingEntity; this uses the causing
    /// entity, not getDirectEntity and not the victim's last attacker.
    fn causing_entity_is_living(&mut self, source: &S) -> Result<bool, Self::Error>;
    fn on_enter_combat(&mut self, tracker: &mut CombatTracker<S, F>) -> Result<(), Self::Error>;
    fn on_leave_combat(&mut self, tracker: &mut CombatTracker<S, F>) -> Result<(), Self::Error>;
}

/// Attach one of these transient owners to the actual living actor. It owns
/// the original tracker fields, and must not be reused for a different actor.
/// Java does not persist these fields as an independent actor/checkpoint.
#[derive(Debug)]
pub struct CombatTracker<S, F> {
    entries: Vec<CombatEntry<S, F>>,
    last_damage_time: i32,
    combat_start_time: i32,
    combat_end_time: i32,
    in_combat: bool,
    taking_damage: bool,
}

impl<S, F> Default for CombatTracker<S, F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, F> CombatTracker<S, F> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_damage_time: 0,
            combat_start_time: 0,
            combat_end_time: 0,
            in_combat: false,
            taking_damage: false,
        }
    }

    pub fn entries(&self) -> &[CombatEntry<S, F>] {
        &self.entries
    }

    pub fn last_damage_time(&self) -> i32 {
        self.last_damage_time
    }

    pub fn combat_start_time(&self) -> i32 {
        self.combat_start_time
    }

    pub fn combat_end_time(&self) -> i32 {
        self.combat_end_time
    }

    pub fn in_combat(&self) -> bool {
        self.in_combat
    }

    pub fn taking_damage(&self) -> bool {
        self.taking_damage
    }

    /// Source lines 163–166. This getter does not call recheckStatus. Reads the
    /// actor tick only while in combat, preserving Java int subtraction wrap.
    pub fn get_combat_duration<H: CombatTrackerHost<S, F>>(
        &self,
        host: &mut H,
    ) -> Result<i32, H::Error> {
        if self.in_combat {
            Ok(host
                .actor_tick_count()?
                .wrapping_sub(self.combat_start_time))
        } else {
            Ok(self.combat_end_time.wrapping_sub(self.combat_start_time))
        }
    }

    /// Source lines 171–184. Only takingDamage triggers an actor query. The
    /// timeout uses strict greater-than, and inCombat selects 300 versus 100.
    /// Leave runs after flags/end time change and before the old entries clear.
    pub fn recheck_status<H: CombatTrackerHost<S, F>>(
        &mut self,
        host: &mut H,
    ) -> Result<(), H::Error> {
        let timeout = if self.in_combat {
            RESET_COMBAT_STATUS_TIME
        } else {
            RESET_DAMAGE_STATUS_TIME
        };
        if self.taking_damage
            && (!host.actor_is_alive()?
                || host.actor_tick_count()?.wrapping_sub(self.last_damage_time) > timeout)
        {
            let was_in_combat = self.in_combat;
            self.taking_damage = false;
            self.in_combat = false;
            self.combat_end_time = host.actor_tick_count()?;
            if was_in_combat {
                host.on_leave_combat(self)?;
            }
            self.entries.clear();
        }
        Ok(())
    }
}

impl<S: Clone, F> CombatTracker<S, F> {
    /// Source lines 40–55. Call at the actual damage path's recordDamage site,
    /// before its source-ordered health subtraction. Zero, signed-zero, NaN and
    /// negative values are not normalized here: the Java method has no guard.
    /// Source cloning must retain the canonical handle as documented above.
    pub fn record_damage<H: CombatTrackerHost<S, F>>(
        &mut self,
        source: &S,
        damage: f32,
        host: &mut H,
    ) -> Result<(), H::Error> {
        self.recheck_status(host)?;
        let fall_location = host.current_fall_location()?;
        let fall_distance = host.fall_distance()?;
        self.entries.push(CombatEntry {
            source: source.clone(),
            damage,
            fall_location,
            fall_distance,
        });
        self.last_damage_time = host.actor_tick_count()?;
        self.taking_damage = true;
        if !self.in_combat && host.actor_is_alive()? && host.causing_entity_is_living(source)? {
            self.in_combat = true;
            self.combat_start_time = host.actor_tick_count()?;
            self.combat_end_time = self.combat_start_time;
            host.on_enter_combat(self)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    struct Source {
        causing_living: bool,
    }
    type Tracker = CombatTracker<Rc<Source>, &'static str>;

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        Alive,
        Tick,
        FallLocation,
        FallDistance,
        CausingLiving,
        Enter {
            start: i32,
            end: i32,
            entries: usize,
        },
        Leave {
            duration: i32,
            entries: usize,
        },
    }

    struct Host {
        calls: Vec<Call>,
        tick: i32,
        alive: bool,
        fail_leave: bool,
        fail_tick: bool,
        reenter_on_leave: Option<Rc<Source>>,
    }

    impl Host {
        fn new(tick: i32) -> Self {
            Self {
                calls: Vec::new(),
                tick,
                alive: true,
                fail_leave: false,
                fail_tick: false,
                reenter_on_leave: None,
            }
        }
    }

    impl CombatTrackerHost<Rc<Source>, &'static str> for Host {
        type Error = &'static str;

        fn actor_is_alive(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::Alive);
            Ok(self.alive)
        }
        fn actor_tick_count(&mut self) -> Result<i32, Self::Error> {
            self.calls.push(Call::Tick);
            if self.fail_tick {
                Err("tick unavailable")
            } else {
                Ok(self.tick)
            }
        }
        fn current_fall_location(&mut self) -> Result<Option<&'static str>, Self::Error> {
            self.calls.push(Call::FallLocation);
            Ok(Some("ladder"))
        }
        fn fall_distance(&mut self) -> Result<f32, Self::Error> {
            self.calls.push(Call::FallDistance);
            Ok(4.5)
        }
        fn causing_entity_is_living(&mut self, source: &Rc<Source>) -> Result<bool, Self::Error> {
            self.calls.push(Call::CausingLiving);
            Ok(source.causing_living)
        }
        fn on_enter_combat(&mut self, tracker: &mut Tracker) -> Result<(), Self::Error> {
            assert!(tracker.in_combat() && tracker.taking_damage());
            self.calls.push(Call::Enter {
                start: tracker.combat_start_time(),
                end: tracker.combat_end_time(),
                entries: tracker.entries().len(),
            });
            Ok(())
        }
        fn on_leave_combat(&mut self, tracker: &mut Tracker) -> Result<(), Self::Error> {
            assert!(!tracker.in_combat() && !tracker.taking_damage());
            let duration = tracker.get_combat_duration(self)?;
            self.calls.push(Call::Leave {
                duration,
                entries: tracker.entries().len(),
            });
            if self.fail_leave {
                Err("leave unavailable")
            } else {
                if let Some(source) = self.reenter_on_leave.take() {
                    tracker.record_damage(&source, 9.0, self)?;
                }
                Ok(())
            }
        }
    }

    #[test]
    fn leave_callback_reenters_same_tracker_before_outer_history_clear() {
        let attacker = Rc::new(Source {
            causing_living: true,
        });
        let mut tracker = Tracker::new();
        let mut host = Host::new(40);
        tracker.record_damage(&attacker, 3.0, &mut host).unwrap();
        host.calls.clear();
        host.tick = 341;
        host.reenter_on_leave = Some(attacker);

        tracker.recheck_status(&mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                Call::Alive,
                Call::Tick,
                Call::Tick,
                Call::Leave {
                    duration: 301,
                    entries: 1
                },
                Call::FallLocation,
                Call::FallDistance,
                Call::Tick,
                Call::Alive,
                Call::CausingLiving,
                Call::Tick,
                Call::Enter {
                    start: 341,
                    end: 341,
                    entries: 2
                },
            ]
        );
        // The outer recheck clears the old and reentrant entries after leave
        // returns, while retaining the flags/timestamps set by recordDamage.
        assert!(tracker.entries().is_empty());
        assert!(tracker.in_combat() && tracker.taking_damage());
        assert_eq!(tracker.last_damage_time(), 341);
        assert_eq!(tracker.combat_start_time(), 341);
        assert_eq!(tracker.combat_end_time(), 341);
        assert_eq!(tracker.get_combat_duration(&mut host), Ok(0));
    }

    #[test]
    fn source_order_timeout_boundaries_and_callback_failure_prefix() {
        let environment = Rc::new(Source {
            causing_living: false,
        });
        let attacker = Rc::new(Source {
            causing_living: true,
        });
        let mut tracker = Tracker::new();
        let mut host = Host::new(10);
        tracker.recheck_status(&mut host).unwrap();
        assert!(host.calls.is_empty());
        tracker
            .record_damage(&environment, -0.0, &mut host)
            .unwrap();
        assert_eq!(
            host.calls,
            [
                Call::FallLocation,
                Call::FallDistance,
                Call::Tick,
                Call::Alive,
                Call::CausingLiving,
            ]
        );
        assert!(Rc::ptr_eq(tracker.entries()[0].source(), &environment));
        assert_eq!(tracker.entries()[0].damage().to_bits(), (-0.0f32).to_bits());
        assert_eq!(tracker.entries()[0].fall_location(), Some(&"ladder"));
        assert_eq!(tracker.entries()[0].fall_distance(), 4.5);
        assert!(!tracker.in_combat() && tracker.taking_damage());

        host.calls.clear();
        host.tick = 110;
        tracker.recheck_status(&mut host).unwrap();
        assert_eq!(host.calls, [Call::Alive, Call::Tick]);
        assert_eq!(tracker.entries().len(), 1);
        host.calls.clear();
        host.tick = 111;
        tracker.recheck_status(&mut host).unwrap();
        assert_eq!(host.calls, [Call::Alive, Call::Tick, Call::Tick]);
        assert!(tracker.entries().is_empty());
        assert_eq!(tracker.get_combat_duration(&mut host), Ok(111));

        host.calls.clear();
        host.tick = 200;
        tracker.record_damage(&attacker, 3.0, &mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                Call::FallLocation,
                Call::FallDistance,
                Call::Tick,
                Call::Alive,
                Call::CausingLiving,
                Call::Tick,
                Call::Enter {
                    start: 200,
                    end: 200,
                    entries: 1
                },
            ]
        );
        host.calls.clear();
        host.tick = 250;
        tracker.record_damage(&environment, 2.0, &mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                Call::Alive,
                Call::Tick,
                Call::FallLocation,
                Call::FallDistance,
                Call::Tick,
            ]
        );
        assert!(Rc::ptr_eq(tracker.entries()[0].source(), &attacker));
        assert!(Rc::ptr_eq(tracker.entries()[1].source(), &environment));
        host.calls.clear();
        host.tick = 550;
        tracker.recheck_status(&mut host).unwrap();
        assert_eq!(host.calls, [Call::Alive, Call::Tick]);
        assert_eq!(tracker.get_combat_duration(&mut host), Ok(350));
        assert!(tracker.in_combat());

        host.calls.clear();
        host.tick = 551;
        host.fail_leave = true;
        assert_eq!(tracker.recheck_status(&mut host), Err("leave unavailable"));
        assert_eq!(
            host.calls,
            [
                Call::Alive,
                Call::Tick,
                Call::Tick,
                Call::Leave {
                    duration: 351,
                    entries: 2
                },
            ]
        );
        assert!(!tracker.in_combat() && !tracker.taking_damage());
        assert_eq!(tracker.combat_end_time(), 551);
        assert_eq!(tracker.entries().len(), 2); // Clear follows the failing callback.
        host.calls.clear();
        tracker.recheck_status(&mut host).unwrap();
        assert!(host.calls.is_empty());

        let mut prefix_tracker = Tracker::new();
        let mut prefix_host = Host::new(900);
        prefix_host.fail_tick = true;
        assert_eq!(
            prefix_tracker.record_damage(&attacker, 1.0, &mut prefix_host),
            Err("tick unavailable")
        );
        assert_eq!(
            prefix_host.calls,
            [Call::FallLocation, Call::FallDistance, Call::Tick]
        );
        assert_eq!(prefix_tracker.entries().len(), 1); // Append precedes tick read.
        assert_eq!(prefix_tracker.last_damage_time(), 0);
        assert!(!prefix_tracker.taking_damage());

        let mut dead_tracker = Tracker::new();
        let mut dead_host = Host::new(1);
        dead_tracker
            .record_damage(&attacker, 1.0, &mut dead_host)
            .unwrap();
        dead_host.calls.clear();
        dead_host.alive = false;
        dead_host.tick = 2;
        dead_tracker.recheck_status(&mut dead_host).unwrap();
        assert_eq!(
            dead_host.calls,
            [
                Call::Alive,
                Call::Tick,
                Call::Leave {
                    duration: 1,
                    entries: 1
                },
            ]
        );
        assert!(dead_tracker.entries().is_empty());

        let mut wrap_tracker = Tracker::new();
        let mut wrap_host = Host::new(i32::MAX - 50);
        wrap_tracker
            .record_damage(&attacker, 1.0, &mut wrap_host)
            .unwrap();
        wrap_host.tick = (i32::MAX - 50).wrapping_add(300);
        wrap_tracker.recheck_status(&mut wrap_host).unwrap();
        assert!(wrap_tracker.in_combat());
        assert_eq!(wrap_tracker.get_combat_duration(&mut wrap_host), Ok(300));
        wrap_host.tick = wrap_host.tick.wrapping_add(1);
        wrap_tracker.recheck_status(&mut wrap_host).unwrap();
        assert!(!wrap_tracker.in_combat());
        assert_eq!(wrap_tracker.get_combat_duration(&mut wrap_host), Ok(301));
    }
}
