//! Java 1.21 `LivingEntity.tickDeath`, original source lines 571–576.
//!
//! This runner owns no actor, saved counter, liveness flag or scheduler. The
//! actual living entity calls it at its source `baseTick` death phase. The
//! pinned source evidence and separate ServerPlayer lifetime are recorded in
//! `docs/reference/entities-survival-death-respawn.md`.

/// Access to one actual entity and its existing death/removal owners.
///
/// Every operation is synchronous at the original callsite. An error preserves
/// prior mutations and effects; callers must not retry this tick as an atomic
/// transaction or replace the actor's actual saved death counter.
pub trait DeathTickHost {
    type Error;

    /// Read the canonical LivingEntity.deathTime, including its saved lineage.
    fn death_time(&mut self) -> Result<i32, Self::Error>;
    fn set_death_time(&mut self, ticks: i32) -> Result<(), Self::Error>;
    /// Query the actual current Level.isClientSide at the source predicate.
    fn level_is_client_side(&mut self) -> Result<bool, Self::Error>;
    fn is_removed(&mut self) -> Result<bool, Self::Error>;
    /// Actual Level.broadcastEntityEvent for this entity, with source audience.
    /// Resolve the current level again here, rather than retaining the level
    /// queried by the earlier logical-side predicate.
    fn broadcast_entity_event(&mut self, event: u8) -> Result<(), Self::Error>;
    /// Virtual Entity.remove with the exact KILLED reason, not a discard or
    /// connection disconnect. The real LivingEntity/Player overrides retain
    /// effect onMobRemoved, parent removal, Brain and menu/tracking callbacks.
    fn remove_killed(&mut self) -> Result<(), Self::Error>;
}

/// Advance the actor's real death counter once at LivingEntity.baseTick's
/// death callsite. Health-based eligibility belongs to that caller; this method
/// does not add another health/dead/connection predicate.
pub fn tick_death<H: DeathTickHost + ?Sized>(host: &mut H) -> Result<(), H::Error> {
    let ticks = host.death_time()?.wrapping_add(1);
    host.set_death_time(ticks)?;
    // The original reads the field again after the increment's write.
    if host.death_time()? >= 20 && !host.level_is_client_side()? && !host.is_removed()? {
        host.broadcast_entity_event(60)?;
        host.remove_killed()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        ReadDeathTime,
        SetDeathTime(i32),
        ClientSide,
        Removed,
        Event(u8),
        RemoveKilled,
    }

    struct Host {
        ticks: i32,
        client: bool,
        removed: bool,
        calls: Vec<Call>,
        fail_event: bool,
        event_changes_state: bool,
        fail_removal_after_mutation: bool,
    }

    impl Host {
        fn new(ticks: i32) -> Self {
            Self {
                ticks,
                client: false,
                removed: false,
                calls: Vec::new(),
                fail_event: false,
                event_changes_state: false,
                fail_removal_after_mutation: false,
            }
        }
    }

    impl DeathTickHost for Host {
        type Error = &'static str;

        fn death_time(&mut self) -> Result<i32, Self::Error> {
            self.calls.push(Call::ReadDeathTime);
            Ok(self.ticks)
        }
        fn set_death_time(&mut self, ticks: i32) -> Result<(), Self::Error> {
            self.calls.push(Call::SetDeathTime(ticks));
            self.ticks = ticks;
            Ok(())
        }
        fn level_is_client_side(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::ClientSide);
            Ok(self.client)
        }
        fn is_removed(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::Removed);
            Ok(self.removed)
        }
        fn broadcast_entity_event(&mut self, event: u8) -> Result<(), Self::Error> {
            self.calls.push(Call::Event(event));
            if self.event_changes_state {
                self.ticks = -50;
                self.removed = true;
            }
            if self.fail_event {
                Err("event failure")
            } else {
                Ok(())
            }
        }
        fn remove_killed(&mut self) -> Result<(), Self::Error> {
            self.calls.push(Call::RemoveKilled);
            self.removed = true;
            if self.fail_removal_after_mutation {
                Err("removal failure")
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn threshold_event_and_real_removal_order() {
        let mut host = Host::new(18);
        tick_death(&mut host).unwrap();
        assert_eq!(host.ticks, 19);
        assert_eq!(
            host.calls,
            [
                Call::ReadDeathTime,
                Call::SetDeathTime(19),
                Call::ReadDeathTime
            ]
        );
        assert!(!host.removed);

        host.calls.clear();
        tick_death(&mut host).unwrap();
        assert_eq!(host.ticks, 20);
        assert!(host.removed);
        assert_eq!(
            host.calls,
            [
                Call::ReadDeathTime,
                Call::SetDeathTime(20),
                Call::ReadDeathTime,
                Call::ClientSide,
                Call::Removed,
                Call::Event(60),
                Call::RemoveKilled
            ]
        );

        host.calls.clear();
        tick_death(&mut host).unwrap();
        assert_eq!(host.ticks, 21);
        assert_eq!(
            host.calls,
            [
                Call::ReadDeathTime,
                Call::SetDeathTime(21),
                Call::ReadDeathTime,
                Call::ClientSide,
                Call::Removed
            ]
        );
    }

    #[test]
    fn client_predicate_short_circuits_removed_query() {
        let mut host = Host::new(20);
        host.client = true;
        tick_death(&mut host).unwrap();
        assert_eq!(host.ticks, 21);
        assert_eq!(
            host.calls,
            [
                Call::ReadDeathTime,
                Call::SetDeathTime(21),
                Call::ReadDeathTime,
                Call::ClientSide
            ]
        );
        assert!(!host.removed);
    }

    #[test]
    fn java_counter_wrap_precedes_threshold_and_side_queries() {
        let mut host = Host::new(i32::MAX);
        tick_death(&mut host).unwrap();
        assert_eq!(host.ticks, i32::MIN);
        assert_eq!(
            host.calls,
            [
                Call::ReadDeathTime,
                Call::SetDeathTime(i32::MIN),
                Call::ReadDeathTime
            ]
        );
        assert!(!host.removed);
    }

    #[test]
    fn broadcast_mutation_does_not_insert_a_second_removal_predicate() {
        let mut host = Host::new(19);
        host.event_changes_state = true;
        tick_death(&mut host).unwrap();
        assert_eq!(host.ticks, -50);
        assert!(host.removed);
        assert_eq!(
            host.calls,
            [
                Call::ReadDeathTime,
                Call::SetDeathTime(20),
                Call::ReadDeathTime,
                Call::ClientSide,
                Call::Removed,
                Call::Event(60),
                Call::RemoveKilled,
            ]
        );
    }

    #[test]
    fn failure_preserves_counter_and_completed_effect_prefix() {
        let mut host = Host::new(19);
        host.fail_event = true;
        assert_eq!(tick_death(&mut host), Err("event failure"));
        assert_eq!(host.ticks, 20);
        assert!(!host.removed);
        assert_eq!(host.calls.last(), Some(&Call::Event(60)));
        assert!(!host.calls.contains(&Call::RemoveKilled));

        let mut host = Host::new(19);
        host.fail_removal_after_mutation = true;
        assert_eq!(tick_death(&mut host), Err("removal failure"));
        assert_eq!(host.ticks, 20);
        assert!(host.removed);
        assert_eq!(
            &host.calls[host.calls.len() - 2..],
            [Call::Event(60), Call::RemoveKilled]
        );
    }
}
