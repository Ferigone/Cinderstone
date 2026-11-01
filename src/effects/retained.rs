//! MobEffectInstance.tick with short borrows of a retained canonical object.
//!
//! The driver runs one instance; it does not own or iterate an active-effect
//! map. The real actor/collection owner must retain the object selected at the
//! source callsite before calling this function. It must preserve that object
//! even if a synchronous callback removes or replaces its map entry.

use super::instance::EffectInstance;

/// Mandatory access to the real actor, retained effect object and definitions.
///
/// `Actor` identifies and retains the same actual living actor for this
/// invocation, including through removal or reentry. An entity-ID lookup must
/// not retarget it to another actor after ID reuse.
/// `Lease` keeps the exact selected effect object alive for the entire call;
/// it must not resolve through the actor's current holder-to-instance map.
/// Removing or replacing that map entry must not retarget or invalidate the
/// lease. These types and their ownership are supplied by the embedding runtime.
///
/// `with_instance` must call its closure on that same canonical object, without
/// moving it out of the active map or substituting a snapshot. Its closure is
/// a callback-free state fragment: every such access returns before any of the
/// following actor/definition callbacks runs. The host therefore has its full
/// mutable context available for synchronous, reentrant operations, including
/// updates to this same object. No actor or instance borrow may be retained
/// across these callback boundaries. Lookup errors must occur before executing
/// the closure; an executed fragment remains committed.
///
/// `K::clone` must retain the same canonical holder identity. It must not
/// allocate another holder or replace a reference holder with a registry ID.
/// An error stops the call at that boundary without undoing earlier state or
/// world effects. No callback has an empty default implementation.
pub trait RetainedEffectTickHost<K> {
    type Error;
    type Actor;
    type Lease;

    fn with_instance<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(&mut EffectInstance<K>) -> R,
    ) -> Result<R, Self::Error>;

    /// The actual actor field, read only on the infinite-duration source branch.
    fn actor_tick_count(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;

    fn should_apply_effect_tick_this_tick(
        &mut self,
        effect: &K,
        duration_or_tick_count: i32,
        amplifier: i32,
    ) -> Result<bool, Self::Error>;

    fn apply_effect_tick(
        &mut self,
        actor: &Self::Actor,
        effect: &K,
        amplifier: i32,
    ) -> Result<bool, Self::Error>;

    /// Execute actor.removeEffect(holder) immediately. It removes the current
    /// map value selected by that holder, which may differ from the retained
    /// object after a reentrant replacement. The retained lease stays valid.
    fn remove_effect(&mut self, actor: &Self::Actor, effect: &K) -> Result<(), Self::Error>;

    /// The source Runnable captures this exact object, including after removal.
    /// Hidden details and the next link are installed before this callback.
    fn on_hidden_effect_promoted(
        &mut self,
        actor: &Self::Actor,
        retained: &Self::Lease,
    ) -> Result<(), Self::Error>;

    fn blend_duration_ticks(&mut self, effect: &K) -> Result<i32, Self::Error>;
}

/// Execute the pinned 1.21 instance tick on an already retained actual object.
///
/// Mutable fields are reread after callbacks at the same points as Java. Only
/// source locals survive callbacks: the duration/tick-count predicate argument,
/// and the first blend-duration result. Returning `false` leaves expiration
/// removal to the actual actor's tickEffects iterator and its source ordering.
/// This function neither removes an entry for ordinary expiration nor supplies
/// a scheduler, collection order, effect definition, attribute owner, or RNG.
pub fn tick_retained<K: Clone, H: RetainedEffectTickHost<K>>(
    actor: &H::Actor,
    retained: &H::Lease,
    host: &mut H,
) -> Result<bool, H::Error> {
    let has_remaining =
        host.with_instance(retained, |instance| instance.has_remaining_duration())?;
    if has_remaining {
        let infinite = host.with_instance(retained, |instance| instance.is_infinite_duration())?;
        let tick_argument = if infinite {
            host.actor_tick_count(actor)?
        } else {
            host.with_instance(retained, |instance| instance.duration())?
        };
        let (effect, amplifier) = host.with_instance(retained, |instance| {
            (instance.effect().clone(), instance.amplifier())
        })?;
        if host.should_apply_effect_tick_this_tick(&effect, tick_argument, amplifier)? {
            let (effect, amplifier) = host.with_instance(retained, |instance| {
                (instance.effect().clone(), instance.amplifier())
            })?;
            if !host.apply_effect_tick(actor, &effect, amplifier)? {
                let effect = host.with_instance(retained, |instance| instance.effect().clone())?;
                host.remove_effect(actor, &effect)?;
            }
        }
        let promoted =
            host.with_instance(retained, |instance| instance.tick_down_and_promote_hidden())?;
        if promoted {
            host.on_hidden_effect_promoted(actor, retained)?;
        }
    }

    let effect = host.with_instance(retained, |instance| {
        instance.begin_blend_tick();
        instance.effect().clone()
    })?;
    let blend_ticks = host.blend_duration_ticks(&effect)?;
    if blend_ticks == 0 {
        host.with_instance(retained, |instance| {
            instance.finish_zero_duration_blend_tick()
        })?;
    } else {
        let effect = host.with_instance(retained, |instance| instance.effect().clone())?;
        let target_blend_ticks = host.blend_duration_ticks(&effect)?;
        host.with_instance(retained, |instance| {
            let target = instance.blend_target(target_blend_ticks);
            instance.advance_blend_tick(blend_ticks, target);
        })?;
    }
    host.with_instance(retained, |instance| instance.has_remaining_duration())
}

#[cfg(test)]
mod tests {
    use super::super::instance::EffectTickHost;
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone, Debug)]
    struct Holder(Rc<u32>);
    impl PartialEq for Holder {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }

    #[derive(Clone)]
    struct Lease(Rc<RefCell<EffectInstance<Holder>>>);
    impl Lease {
        fn new(instance: EffectInstance<Holder>) -> Self {
            Self(Rc::new(RefCell::new(instance)))
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    enum Event {
        ActorTick,
        Should(i32, i32),
        Apply(i32, i32),
        Remove(Option<i32>),
        Promote(i32, i32),
        Blend,
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Mutation {
        None,
        UpdateDuringShouldAndApply,
        TickDuringApply,
        ReplaceDuringApply,
        UpdateDuringPromotion,
        UpdateDuringBlend,
    }

    struct Host {
        // This is a single-entry mock collection, not a source-order model.
        active: Option<Lease>,
        retained: Lease,
        replacement: Option<Lease>,
        events: Vec<Event>,
        should_apply: bool,
        keep_after_apply: bool,
        mutation: Mutation,
        fail_apply: bool,
        fail_remove: bool,
        fail_promote: bool,
        fail_blend_lookup: Option<usize>,
        blend_duration: i32,
        blend_lookups: usize,
    }
    impl Host {
        fn new(instance: EffectInstance<Holder>) -> Self {
            let retained = Lease::new(instance);
            Self {
                active: Some(retained.clone()),
                retained,
                replacement: None,
                events: Vec::new(),
                should_apply: true,
                keep_after_apply: true,
                mutation: Mutation::None,
                fail_apply: false,
                fail_remove: false,
                fail_promote: false,
                fail_blend_lookup: None,
                blend_duration: 0,
                blend_lookups: 0,
            }
        }

        fn assert_callback_has_full_access(&mut self) {
            // These borrows would fail if the runner held any instance borrow
            // when entering a callback. Reentrant actor operations use the same
            // host and canonical object, without a detached working copy.
            drop(
                self.retained
                    .0
                    .try_borrow_mut()
                    .expect("retained object borrowed across callback"),
            );
            if let Some(active) = &self.active {
                drop(
                    active
                        .0
                        .try_borrow_mut()
                        .expect("active object borrowed across callback"),
                );
            }
            if let Some(replacement) = &self.replacement {
                drop(
                    replacement
                        .0
                        .try_borrow_mut()
                        .expect("replacement borrowed across callback"),
                );
            }
        }

        fn update(instance: &Lease, duration: i32, amplifier: i32) {
            let effect = instance.0.borrow().effect().clone();
            instance
                .0
                .borrow_mut()
                .update(&EffectInstance::new(effect, duration, amplifier), |_, _| {
                    panic!("holder identity changed")
                });
        }
    }

    impl RetainedEffectTickHost<Holder> for Host {
        type Error = &'static str;
        type Actor = u32;
        type Lease = Lease;

        fn with_instance<R>(
            &mut self,
            retained: &Lease,
            access: impl FnOnce(&mut EffectInstance<Holder>) -> R,
        ) -> Result<R, Self::Error> {
            assert!(Rc::ptr_eq(&retained.0, &self.retained.0));
            Ok(access(&mut retained.0.borrow_mut()))
        }

        fn actor_tick_count(&mut self, actor: &u32) -> Result<i32, Self::Error> {
            assert_eq!(*actor, 7);
            self.assert_callback_has_full_access();
            self.events.push(Event::ActorTick);
            Ok(1234)
        }

        fn should_apply_effect_tick_this_tick(
            &mut self,
            effect: &Holder,
            ticks: i32,
            amplifier: i32,
        ) -> Result<bool, Self::Error> {
            self.assert_callback_has_full_access();
            assert_eq!(effect, self.retained.0.borrow().effect());
            self.events.push(Event::Should(ticks, amplifier));
            if self.mutation == Mutation::UpdateDuringShouldAndApply {
                Self::update(self.active.as_ref().unwrap(), 9, 2);
            }
            Ok(self.should_apply)
        }

        fn apply_effect_tick(
            &mut self,
            actor: &u32,
            effect: &Holder,
            amplifier: i32,
        ) -> Result<bool, Self::Error> {
            assert_eq!(*actor, 7);
            self.assert_callback_has_full_access();
            assert_eq!(effect, self.retained.0.borrow().effect());
            self.events
                .push(Event::Apply(self.retained.0.borrow().duration(), amplifier));
            match self.mutation {
                Mutation::UpdateDuringShouldAndApply => {
                    Self::update(self.active.as_ref().unwrap(), 12, 3);
                }
                Mutation::TickDuringApply => {
                    self.mutation = Mutation::None;
                    let retained = self.retained.clone();
                    tick_retained(actor, &retained, self)?;
                }
                Mutation::ReplaceDuringApply => {
                    let replacement = Lease::new(EffectInstance::new(effect.clone(), 100, 4));
                    self.active = Some(replacement.clone());
                    self.replacement = Some(replacement);
                }
                _ => {}
            }
            if self.fail_apply {
                Err("apply failed")
            } else {
                Ok(self.keep_after_apply)
            }
        }

        fn remove_effect(&mut self, actor: &u32, effect: &Holder) -> Result<(), Self::Error> {
            assert_eq!(*actor, 7);
            self.assert_callback_has_full_access();
            self.events.push(Event::Remove(
                self.active
                    .as_ref()
                    .map(|entry| entry.0.borrow().duration()),
            ));
            if self.fail_remove {
                return Err("remove failed");
            }
            if let Some(active) = &self.active {
                assert_eq!(effect, active.0.borrow().effect());
            }
            self.active = None;
            Ok(())
        }

        fn on_hidden_effect_promoted(
            &mut self,
            actor: &u32,
            retained: &Lease,
        ) -> Result<(), Self::Error> {
            assert_eq!(*actor, 7);
            self.assert_callback_has_full_access();
            assert!(Rc::ptr_eq(&retained.0, &self.retained.0));
            {
                let instance = retained.0.borrow();
                self.events
                    .push(Event::Promote(instance.duration(), instance.amplifier()));
            }
            if self.mutation == Mutation::UpdateDuringPromotion {
                Self::update(retained, -2, 255);
            }
            if self.fail_promote {
                Err("promotion failed")
            } else {
                Ok(())
            }
        }

        fn blend_duration_ticks(&mut self, effect: &Holder) -> Result<i32, Self::Error> {
            self.assert_callback_has_full_access();
            assert_eq!(effect, self.retained.0.borrow().effect());
            self.events.push(Event::Blend);
            self.blend_lookups += 1;
            if self.fail_blend_lookup == Some(self.blend_lookups) {
                return Err("blend lookup failed");
            }
            if self.mutation == Mutation::UpdateDuringBlend {
                if self.blend_lookups == 1 {
                    Self::update(&self.retained, 1, 4);
                } else {
                    Self::update(&self.retained, 100, 5);
                    return Ok(4);
                }
            }
            Ok(self.blend_duration)
        }
    }

    struct MonolithicHost {
        events: Vec<Event>,
    }
    impl EffectTickHost<Holder> for MonolithicHost {
        type Error = &'static str;
        fn should_apply_effect_tick_this_tick(
            &mut self,
            _: &Holder,
            ticks: i32,
            amplifier: i32,
        ) -> Result<bool, Self::Error> {
            self.events.push(Event::Should(ticks, amplifier));
            Ok(false)
        }
        fn apply_effect_tick(
            &mut self,
            _: &mut EffectInstance<Holder>,
            _: i32,
        ) -> Result<bool, Self::Error> {
            panic!("predicate is false")
        }
        fn remove_effect(&mut self, _: &mut EffectInstance<Holder>) -> Result<(), Self::Error> {
            panic!("predicate is false")
        }
        fn on_hidden_effect_promoted(
            &mut self,
            instance: &mut EffectInstance<Holder>,
        ) -> Result<(), Self::Error> {
            self.events
                .push(Event::Promote(instance.duration(), instance.amplifier()));
            Ok(())
        }
        fn blend_duration_ticks(&mut self, _: &Holder) -> Result<i32, Self::Error> {
            self.events.push(Event::Blend);
            Ok(2)
        }
    }

    fn holder() -> Holder {
        Holder(Rc::new(1))
    }

    #[test]
    fn retained_trace_matches_existing_source_tick_and_nested_hidden_promotion() {
        let effect = holder();
        let mut instance = EffectInstance::new(effect.clone(), 20, 0);
        instance.update(&EffectInstance::new(effect.clone(), 5, 1), |_, _| {
            panic!("same holder")
        });
        instance.update(&EffectInstance::new(effect, 3, 2), |_, _| {
            panic!("same holder")
        });
        let mut monolithic_instance = instance.clone();
        let mut monolithic_host = MonolithicHost { events: Vec::new() };
        let mut host = Host::new(instance);
        host.should_apply = false;
        host.blend_duration = 2;
        let lease = host.retained.clone();
        for tick in 1..=20 {
            let expected = monolithic_instance
                .tick(tick, &mut monolithic_host)
                .unwrap();
            assert_eq!(tick_retained(&7, &lease, &mut host).unwrap(), expected);
            let mut actual = lease.0.borrow_mut();
            assert_eq!(
                (actual.duration(), actual.amplifier()),
                (
                    monolithic_instance.duration(),
                    monolithic_instance.amplifier()
                )
            );
            assert_eq!(
                actual.blend_factor(false, 0.5),
                monolithic_instance.blend_factor(false, 0.5)
            );
        }
        assert_eq!(host.events, monolithic_host.events);
    }

    #[test]
    fn reentrant_same_instance_updates_are_seen_after_each_callback() {
        let mut host = Host::new(EffectInstance::new(holder(), 5, 0));
        host.mutation = Mutation::UpdateDuringShouldAndApply;
        let lease = host.retained.clone();
        assert!(tick_retained(&7, &lease, &mut host).unwrap());
        assert_eq!(
            host.events,
            vec![Event::Should(5, 0), Event::Apply(9, 2), Event::Blend]
        );
        let instance = lease.0.borrow();
        assert_eq!((instance.duration(), instance.amplifier()), (11, 3));
        assert!(Rc::ptr_eq(&host.active.as_ref().unwrap().0, &lease.0));
        drop(instance);

        let mut host = Host::new(EffectInstance::new(holder(), 3, 0));
        host.mutation = Mutation::TickDuringApply;
        let lease = host.retained.clone();
        assert!(tick_retained(&7, &lease, &mut host).unwrap());
        assert_eq!(lease.0.borrow().duration(), 1);
        assert_eq!(
            host.events,
            vec![
                Event::Should(3, 0),
                Event::Apply(3, 0),
                Event::Should(3, 0),
                Event::Apply(3, 0),
                Event::Blend,
                Event::Blend,
            ]
        );
    }

    #[test]
    fn replacement_and_removal_do_not_retarget_retained_object_or_promotion() {
        let effect = holder();
        let instance = EffectInstance::with_hidden(
            effect.clone(),
            2,
            3,
            false,
            true,
            true,
            Some(Box::new(EffectInstance::new(effect, 10, 1))),
        );
        let mut host = Host::new(instance);
        host.mutation = Mutation::ReplaceDuringApply;
        host.keep_after_apply = false;
        let lease = host.retained.clone();
        assert!(tick_retained(&7, &lease, &mut host).unwrap());
        assert_eq!(
            host.events,
            vec![
                Event::Should(2, 3),
                Event::Apply(2, 3),
                Event::Remove(Some(100)),
                Event::Blend
            ]
        );
        assert!(host.active.is_none());
        assert_eq!(lease.0.borrow().duration(), 1);
        assert_eq!(
            host.replacement.as_ref().unwrap().0.borrow().duration(),
            100
        );
        host.should_apply = false;
        host.mutation = Mutation::None;
        host.events.clear();
        assert!(tick_retained(&7, &lease, &mut host).unwrap());
        assert_eq!(
            (lease.0.borrow().duration(), lease.0.borrow().amplifier()),
            (8, 1)
        );
        assert_eq!(
            host.events,
            vec![Event::Should(1, 3), Event::Promote(8, 1), Event::Blend]
        );
        assert!(host.active.is_none());
    }

    #[test]
    fn failures_keep_reentrant_mutations_and_committed_promotion_prefix() {
        let mut host = Host::new(EffectInstance::new(holder(), 5, 0));
        host.mutation = Mutation::UpdateDuringShouldAndApply;
        host.fail_apply = true;
        let lease = host.retained.clone();
        assert_eq!(tick_retained(&7, &lease, &mut host), Err("apply failed"));
        assert_eq!(
            (lease.0.borrow().duration(), lease.0.borrow().amplifier()),
            (12, 3)
        );
        assert_eq!(host.events, vec![Event::Should(5, 0), Event::Apply(9, 2)]);

        host.fail_apply = false;
        host.mutation = Mutation::None;
        host.keep_after_apply = false;
        host.fail_remove = true;
        host.events.clear();
        assert_eq!(tick_retained(&7, &lease, &mut host), Err("remove failed"));
        assert_eq!(lease.0.borrow().duration(), 12);
        assert_eq!(
            host.events,
            vec![
                Event::Should(12, 3),
                Event::Apply(12, 3),
                Event::Remove(Some(12))
            ]
        );

        let effect = holder();
        let mut host = Host::new(EffectInstance::with_hidden(
            effect.clone(),
            1,
            2,
            false,
            true,
            true,
            Some(Box::new(EffectInstance::new(effect, 7, 1))),
        ));
        host.should_apply = false;
        host.mutation = Mutation::UpdateDuringPromotion;
        host.fail_promote = true;
        let lease = host.retained.clone();
        assert_eq!(
            tick_retained(&7, &lease, &mut host),
            Err("promotion failed")
        );
        assert_eq!(
            (lease.0.borrow().duration(), lease.0.borrow().amplifier()),
            (-2, 255)
        );
        assert_eq!(host.events, vec![Event::Should(1, 2), Event::Promote(6, 1)]);

        let mut host = Host::new(EffectInstance::new(holder(), 20, 0));
        host.should_apply = false;
        host.blend_duration = 2;
        let lease = host.retained.clone();
        tick_retained(&7, &lease, &mut host).unwrap();
        host.blend_lookups = 0;
        host.fail_blend_lookup = Some(2);
        host.events.clear();
        assert_eq!(
            tick_retained(&7, &lease, &mut host),
            Err("blend lookup failed")
        );
        let mut instance = lease.0.borrow_mut();
        assert_eq!(instance.duration(), 18);
        assert_eq!(instance.blend_factor(false, 0.0), 0.5);
        assert_eq!(instance.blend_factor(false, 1.0), 0.5);
        assert_eq!(
            host.events,
            vec![Event::Should(19, 0), Event::Blend, Event::Blend]
        );
    }

    #[test]
    fn infinite_counter_and_post_callback_blend_state_are_read_at_source_points() {
        let mut host = Host::new(EffectInstance::new(holder(), -1, 0));
        host.should_apply = false;
        let lease = host.retained.clone();
        assert!(tick_retained(&7, &lease, &mut host).unwrap());
        assert_eq!(
            host.events,
            vec![Event::ActorTick, Event::Should(1234, 0), Event::Blend]
        );
        assert_eq!(lease.0.borrow().duration(), -1);

        let mut host = Host::new(EffectInstance::new(holder(), 20, 0));
        host.should_apply = false;
        host.blend_duration = 2;
        host.mutation = Mutation::UpdateDuringBlend;
        let lease = host.retained.clone();
        assert!(tick_retained(&7, &lease, &mut host).unwrap());
        let mut instance = lease.0.borrow_mut();
        assert_eq!((instance.duration(), instance.amplifier()), (100, 5));
        // First lookup's 2 is the saved source local. The second lookup's 4
        // sets the target after the reentrant duration change, not the step.
        assert_eq!(instance.blend_factor(false, 1.0), 0.5);
        assert_eq!(
            host.events,
            vec![Event::Should(20, 0), Event::Blend, Event::Blend]
        );
        drop(instance);

        let mut host = Host::new(EffectInstance::new(holder(), 0, 0));
        let lease = host.retained.clone();
        assert!(!tick_retained(&7, &lease, &mut host).unwrap());
        assert_eq!(host.events, vec![Event::Blend]);
    }
}
