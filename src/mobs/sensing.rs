//! Java 1.21 Sensing (`cdx`, source lines 10–39), over retained real owners.
//!
//! The private source IntOpenHashSet caches expose only int contains/add/clear.
//! Deterministic native BTreeSets preserve those observations without a random
//! hasher, an identity callback, an iteration API or an invented LOS result.
//! This state belongs to one actual Sensing object and retains its original Mob.

use super::ai_step::ProfileEvent;
use std::collections::BTreeSet;

pub struct SensingState<A> {
    seen: BTreeSet<i32>,
    unseen: BTreeSet<i32>,
    mob: A,
}
impl<A> SensingState<A> {
    /// Source constructor10–15: seen, then unseen, then the actual Mob handle.
    /// This creates cache fields, not an actor, visibility oracle or scheduler.
    pub fn new(mob: A) -> Self {
        Self {
            seen: BTreeSet::new(),
            unseen: BTreeSet::new(),
            mob,
        }
    }
    /// Callback-free storage conversion of the SAME Mob reference. Aggregate
    /// actors use this once, before publication, to encode an internal backlink.
    /// Every outward Sensing lease must retain that aggregate; this is never a
    /// substitute for a retained public source Mob handle.
    pub fn map_mob<B>(self, map: impl FnOnce(A) -> B) -> SensingState<B> {
        SensingState {
            seen: self.seen,
            unseen: self.unseen,
            mob: map(self.mob),
        }
    }
}

pub trait SensingHost {
    type Error;
    type Lease;
    /// Clone retains the exact original Mob, including after removal/reentry.
    type Mob: Clone;
    type Target;
    type Level;
    type Profiler;

    /// Access the canonical Sensing fields in place, with no game callback
    /// inside this closure. Release every state/actor borrow before returning.
    /// Lookup failure precedes the closure; completed mutations are not undone.
    fn with_sensing_state<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(&mut SensingState<Self::Mob>) -> R,
    ) -> Result<R, Self::Error>;
    /// Actual virtual Entity.getId on the retained target, sampled once.
    fn target_entity_id(&mut self, target: &Self::Target) -> Result<i32, Self::Error>;
    fn mob_level(&mut self, mob: &Self::Mob) -> Result<Self::Level, Self::Error>;
    fn level_profiler(&mut self, level: &Self::Level) -> Result<Self::Profiler, Self::Error>;
    fn profile(
        &mut self,
        profiler: &mut Self::Profiler,
        event: ProfileEvent,
    ) -> Result<(), Self::Error>;
    /// Actual LivingEntity.hasLineOfSight on this Mob and exact target object;
    /// world ray queries, distance, identity and source callbacks remain real.
    fn mob_has_line_of_sight(
        &mut self,
        mob: &Self::Mob,
        target: &Self::Target,
    ) -> Result<bool, Self::Error>;
}

/// Source18–20. The seen clear commits before the separate unseen clear.
pub fn tick<H: SensingHost>(retained: &H::Lease, host: &mut H) -> Result<(), H::Error> {
    host.with_sensing_state(retained, |state| state.seen.clear())?;
    host.with_sensing_state(retained, |state| state.unseen.clear())
}

/// Source23–39. Cache keys intentionally use the captured source int ID; target
/// callbacks still address the retained object, never a later occupant of that ID.
/// Both Level/profiler lookups are fresh. There is no source finally-pop or cache
/// insertion after an earlier failure, and no borrow spans the real LOS callback.
pub fn has_line_of_sight<H: SensingHost>(
    retained: &H::Lease,
    target: &H::Target,
    host: &mut H,
) -> Result<bool, H::Error> {
    let target_id = host.target_entity_id(target)?;
    if host.with_sensing_state(retained, |state| state.seen.contains(&target_id))? {
        return Ok(true);
    }
    if host.with_sensing_state(retained, |state| state.unseen.contains(&target_id))? {
        return Ok(false);
    }
    let mob = host.with_sensing_state(retained, |state| state.mob.clone())?;
    let level = host.mob_level(&mob)?;
    let mut profiler = host.level_profiler(&level)?;
    host.profile(&mut profiler, ProfileEvent::Push("hasLineOfSight"))?;

    let mob = host.with_sensing_state(retained, |state| state.mob.clone())?;
    let visible = host.mob_has_line_of_sight(&mob, target)?;

    let mob = host.with_sensing_state(retained, |state| state.mob.clone())?;
    let level = host.mob_level(&mob)?;
    let mut profiler = host.level_profiler(&level)?;
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    host.with_sensing_state(retained, |state| {
        if visible {
            state.seen.insert(target_id);
        } else {
            state.unseen.insert(target_id);
        }
    })?;
    Ok(visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    type Lease = Rc<RefCell<SensingState<Rc<()>>>>;
    type Target = Rc<RefCell<i32>>;
    struct Host {
        state: Lease,
        mob: Rc<()>,
        level: i32,
        visible: bool,
        reenter: bool,
        fail: Option<&'static str>,
        accesses: usize,
        fail_access: Option<usize>,
        calls: Vec<String>,
    }
    impl Host {
        fn new() -> Self {
            let mob = Rc::new(());
            Self {
                state: Rc::new(RefCell::new(SensingState::new(mob.clone()))),
                mob,
                level: 1,
                visible: true,
                reenter: false,
                fail: None,
                accesses: 0,
                fail_access: None,
                calls: Vec::new(),
            }
        }
        fn event(&mut self, event: &str) -> Result<(), &'static str> {
            // Any host callback may synchronously enter this same owner.
            drop(
                self.state
                    .try_borrow_mut()
                    .expect("state held across callback"),
            );
            self.calls.push(event.into());
            if self.fail == Some(event) {
                Err("controlled callback failure")
            } else {
                Ok(())
            }
        }
    }
    impl SensingHost for Host {
        type Error = &'static str;
        type Lease = Lease;
        type Mob = Rc<()>;
        type Target = Target;
        type Level = i32;
        type Profiler = i32;
        fn with_sensing_state<R>(
            &mut self,
            retained: &Lease,
            access: impl FnOnce(&mut SensingState<Rc<()>>) -> R,
        ) -> Result<R, Self::Error> {
            assert!(Rc::ptr_eq(retained, &self.state));
            self.accesses += 1;
            if self.fail_access == Some(self.accesses) {
                return Err("controlled state lookup failure");
            }
            Ok(access(&mut retained.borrow_mut()))
        }
        fn target_entity_id(&mut self, target: &Target) -> Result<i32, Self::Error> {
            self.event("id")?;
            Ok(*target.borrow())
        }
        fn mob_level(&mut self, mob: &Rc<()>) -> Result<i32, Self::Error> {
            assert!(Rc::ptr_eq(mob, &self.mob));
            self.event(&format!("level:{}", self.level))?;
            Ok(self.level)
        }
        fn level_profiler(&mut self, level: &i32) -> Result<i32, Self::Error> {
            self.event(&format!("profiler:{level}"))?;
            Ok(*level)
        }
        fn profile(&mut self, profiler: &mut i32, event: ProfileEvent) -> Result<(), Self::Error> {
            self.event(&format!("{event:?}:{profiler}"))
        }
        fn mob_has_line_of_sight(
            &mut self,
            mob: &Rc<()>,
            target: &Target,
        ) -> Result<bool, Self::Error> {
            assert!(Rc::ptr_eq(mob, &self.mob));
            self.event("los")?;
            if self.reenter {
                self.reenter = false;
                let lease = self.state.clone();
                tick(&lease, self)?;
                *target.borrow_mut() = 77;
                self.level = 2;
            }
            Ok(self.visible)
        }
    }

    #[test]
    fn reentrant_clear_id_mutation_and_fresh_profiler_keep_captured_cache_key() {
        let mut host = Host::new();
        let lease = host.state.clone();
        let target = Rc::new(RefCell::new(-9));
        host.state.borrow_mut().unseen.insert(99);
        host.reenter = true;
        assert!(has_line_of_sight(&lease, &target, &mut host).unwrap());
        assert_eq!(
            host.calls,
            [
                "id",
                "level:1",
                "profiler:1",
                "Push(\"hasLineOfSight\"):1",
                "los",
                "level:2",
                "profiler:2",
                "Pop:2"
            ]
        );
        assert!(lease.borrow().seen.contains(&-9));
        assert!(lease.borrow().unseen.is_empty());
        assert!(!lease.borrow().seen.contains(&77));
        host.calls.clear();
        // A different retained object with that ID deliberately hits Java's int cache.
        let same_id = Rc::new(RefCell::new(-9));
        assert!(has_line_of_sight(&lease, &same_id, &mut host).unwrap());
        assert_eq!(host.calls, ["id"]);
        for id in [0, i32::MIN, i32::MAX] {
            host.calls.clear();
            host.visible = false;
            let target = Rc::new(RefCell::new(id));
            assert!(!has_line_of_sight(&lease, &target, &mut host).unwrap());
            assert!(lease.borrow().unseen.contains(&id));
            host.calls.clear();
            host.visible = true;
            assert!(!has_line_of_sight(&lease, &target, &mut host).unwrap());
            assert_eq!(host.calls, ["id"]);
        }
    }

    #[test]
    fn failed_query_or_pop_does_not_cache_or_synthesize_cleanup_and_tick_keeps_prefix() {
        let mut host = Host::new();
        let lease = host.state.clone();
        let target = Rc::new(RefCell::new(4));
        host.fail = Some("los");
        assert_eq!(
            has_line_of_sight(&lease, &target, &mut host),
            Err("controlled callback failure")
        );
        assert_eq!(host.calls.last().unwrap(), "los");
        assert!(lease.borrow().seen.is_empty());
        host.calls.clear();
        host.fail = Some("Pop:1");
        assert!(has_line_of_sight(&lease, &target, &mut host).is_err());
        assert!(lease.borrow().seen.is_empty());
        lease.borrow_mut().seen.insert(1);
        lease.borrow_mut().unseen.insert(2);
        host.fail_access = Some(host.accesses + 2);
        assert_eq!(
            tick(&lease, &mut host),
            Err("controlled state lookup failure")
        );
        assert!(lease.borrow().seen.is_empty());
        assert!(lease.borrow().unseen.contains(&2));
    }
}
