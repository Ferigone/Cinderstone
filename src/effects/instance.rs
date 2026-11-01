//! MobEffectInstance's active/hidden duration and blend state.
//!
//! Direct inspection of the pinned Java 1.21 `brz` class with official
//! mappings is recorded in docs/reference/entities-survival-effects.md.
//! Holder keys remain opaque; callers must retain their canonical identity.

/// Mandatory synchronous calls made by `MobEffectInstance.tick`.
///
/// The host owns the actual living actor, effect definitions and effect map.
/// In particular, `remove_effect` must execute the actor's real removeEffect
/// path immediately, even though the retained instance continues its tick.
/// Mutable instance arguments allow callbacks to change that same source
/// object; moving it out of the actor's map is not implied by this interface.
/// An error stops at that callback, retaining prior mutations and side effects.
pub trait EffectTickHost<K> {
    type Error;

    fn should_apply_effect_tick_this_tick(
        &mut self,
        effect: &K,
        duration_or_tick_count: i32,
        amplifier: i32,
    ) -> Result<bool, Self::Error>;

    fn apply_effect_tick(
        &mut self,
        instance: &mut EffectInstance<K>,
        amplifier: i32,
    ) -> Result<bool, Self::Error>;

    fn remove_effect(&mut self, instance: &mut EffectInstance<K>) -> Result<(), Self::Error>;

    /// Called after the hidden details and next hidden link have been installed.
    fn on_hidden_effect_promoted(
        &mut self,
        instance: &mut EffectInstance<K>,
    ) -> Result<(), Self::Error>;

    fn blend_duration_ticks(&mut self, effect: &K) -> Result<i32, Self::Error>;
}

#[derive(Clone, Debug)]
struct BlendState {
    factor: f32,
    previous_factor: f32,
}

/// One Java effect instance, including its independently aging hidden chain.
///
/// Rust `Clone` is a deep native snapshot. Use `copy_instance` for Java's
/// copy-constructor semantics, which omit the hidden chain and reset blending.
#[derive(Clone, Debug)]
pub struct EffectInstance<K> {
    effect: K,
    duration: i32,
    amplifier: i32,
    ambient: bool,
    visible: bool,
    show_icon: bool,
    hidden_effect: Option<Box<Self>>,
    blend_state: BlendState,
}

impl<K> EffectInstance<K> {
    pub const INFINITE_DURATION: i32 = -1;

    pub fn new(effect: K, duration: i32, amplifier: i32) -> Self {
        Self::with_flags(effect, duration, amplifier, false, true, true)
    }

    pub fn with_flags(
        effect: K,
        duration: i32,
        amplifier: i32,
        ambient: bool,
        visible: bool,
        show_icon: bool,
    ) -> Self {
        Self::with_hidden(
            effect, duration, amplifier, ambient, visible, show_icon, None,
        )
    }

    pub fn with_hidden(
        effect: K,
        duration: i32,
        amplifier: i32,
        ambient: bool,
        visible: bool,
        show_icon: bool,
        hidden_effect: Option<Box<Self>>,
    ) -> Self {
        Self {
            effect,
            duration,
            amplifier: amplifier.clamp(0, 255),
            ambient,
            visible,
            show_icon,
            hidden_effect,
            blend_state: BlendState {
                factor: 0.0,
                previous_factor: 0.0,
            },
        }
    }

    pub fn effect(&self) -> &K {
        &self.effect
    }

    pub fn duration(&self) -> i32 {
        self.duration
    }

    pub fn amplifier(&self) -> i32 {
        self.amplifier
    }

    pub fn is_ambient(&self) -> bool {
        self.ambient
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show_icon(&self) -> bool {
        self.show_icon
    }

    pub fn hidden_effect(&self) -> Option<&Self> {
        self.hidden_effect.as_deref()
    }

    pub fn is_infinite_duration(&self) -> bool {
        self.duration == Self::INFINITE_DURATION
    }

    pub fn has_remaining_duration(&self) -> bool {
        self.is_infinite_duration() || self.duration > 0
    }

    pub fn ends_within(&self, ticks: i32) -> bool {
        !self.is_infinite_duration() && self.duration <= ticks
    }

    /// Java mapDuration: zero and -1 bypass the supplied mapping function.
    pub fn map_duration(&self, map: impl FnOnce(i32) -> i32) -> i32 {
        if self.is_infinite_duration() || self.duration == 0 {
            self.duration
        } else {
            map(self.duration)
        }
    }

    fn is_shorter_duration_than(&self, other: &Self) -> bool {
        !self.is_infinite_duration()
            && (self.duration < other.duration || other.is_infinite_duration())
    }

    fn set_details_from(&mut self, other: &Self) {
        self.duration = other.duration;
        self.amplifier = other.amplifier;
        self.ambient = other.ambient;
        self.visible = other.visible;
        self.show_icon = other.show_icon;
    }

    fn tick_down_duration(&mut self) {
        if let Some(hidden) = self.hidden_effect.as_mut() {
            hidden.tick_down_duration();
        }
        self.duration = self.map_duration(|duration| duration.wrapping_sub(1));
    }

    /// Callback-free source fragment shared with the retained-handle driver.
    pub(super) fn tick_down_and_promote_hidden(&mut self) -> bool {
        self.tick_down_duration();
        if self.duration == 0 {
            if let Some(mut hidden) = self.hidden_effect.take() {
                self.set_details_from(&hidden);
                self.hidden_effect = hidden.hidden_effect.take();
                return true;
            }
        }
        false
    }

    pub(super) fn begin_blend_tick(&mut self) {
        self.blend_state.previous_factor = self.blend_state.factor;
    }

    pub(super) fn finish_zero_duration_blend_tick(&mut self) {
        self.blend_state.factor = 1.0;
    }

    pub(super) fn advance_blend_tick(&mut self, blend_ticks: i32, target: f32) {
        if self.blend_state.factor != target {
            let step = 1.0_f32 / blend_ticks as f32;
            let delta = target - self.blend_state.factor;
            let change = if delta < -step {
                -step
            } else {
                delta.min(step)
            };
            self.blend_state.factor += change;
        }
    }

    /// Execute one retained instance tick at the actor's actual effect phase.
    ///
    /// `false` means the instance has no remaining duration; the actor's
    /// tickEffects iterator still owns expiration removal and its callbacks.
    pub fn tick<H: EffectTickHost<K>>(
        &mut self,
        actor_tick_count: i32,
        host: &mut H,
    ) -> Result<bool, H::Error> {
        if self.has_remaining_duration() {
            let tick_argument = if self.is_infinite_duration() {
                actor_tick_count
            } else {
                self.duration
            };
            if host.should_apply_effect_tick_this_tick(
                &self.effect,
                tick_argument,
                self.amplifier,
            )? {
                let amplifier = self.amplifier;
                if !host.apply_effect_tick(self, amplifier)? {
                    host.remove_effect(self)?;
                }
            }
            if self.tick_down_and_promote_hidden() {
                host.on_hidden_effect_promoted(self)?;
            }
        }
        // Java performs this even when the instance entered with no duration.
        self.begin_blend_tick();
        let blend_ticks = host.blend_duration_ticks(&self.effect)?;
        if blend_ticks == 0 {
            self.finish_zero_duration_blend_tick();
        } else {
            // The source resolves getBlendDurationTicks again through its target
            // helper; retain the lookup and its position after previous_factor.
            let target = self.blend_target(host.blend_duration_ticks(&self.effect)?);
            self.advance_blend_tick(blend_ticks, target);
        }
        Ok(self.has_remaining_duration())
    }

    pub(super) fn blend_target(&self, blend_ticks: i32) -> f32 {
        if self.ends_within(blend_ticks) {
            0.0
        } else {
            1.0
        }
    }

    /// Java skipBlending resolves the definition's real blend duration first.
    pub fn skip_blending(&mut self, blend_ticks: i32) {
        self.blend_state.factor = self.blend_target(blend_ticks);
        self.blend_state.previous_factor = self.blend_state.factor;
    }

    pub fn copy_blend_state(&mut self, other: &Self) {
        self.blend_state = other.blend_state.clone();
    }

    /// Pass the actual actor's isRemoved state, as Java getBlendFactor does.
    pub fn blend_factor(&mut self, actor_removed: bool, partial_tick: f32) -> f32 {
        if actor_removed {
            self.blend_state.previous_factor = self.blend_state.factor;
        }
        self.blend_state.previous_factor
            + partial_tick * (self.blend_state.factor - self.blend_state.previous_factor)
    }
}

impl<K: Clone> EffectInstance<K> {
    /// Java's copy constructor copies details, but neither hidden nor blend state.
    pub fn copy_instance(&self) -> Self {
        Self::with_flags(
            self.effect.clone(),
            self.duration,
            self.amplifier,
            self.ambient,
            self.visible,
            self.show_icon,
        )
    }
}

impl<K: Clone + PartialEq> EffectInstance<K> {
    /// Java update, including its warning-and-continue mismatch behavior.
    ///
    /// The caller must normally select by the canonical holder key. The warning
    /// callback runs synchronously if that precondition is violated. A `false`
    /// result can still mean the hidden chain changed; it is not a no-op proof.
    pub fn update(&mut self, incoming: &Self, mut mismatched_effects: impl FnMut(&K, &K)) -> bool {
        self.update_inner(incoming, &mut mismatched_effects)
    }

    fn update_inner(
        &mut self,
        incoming: &Self,
        mismatched_effects: &mut impl FnMut(&K, &K),
    ) -> bool {
        if self.effect != incoming.effect {
            mismatched_effects(&self.effect, &incoming.effect);
        }
        let mut changed = false;
        if incoming.amplifier > self.amplifier {
            if incoming.is_shorter_duration_than(self) {
                let previous_hidden = self.hidden_effect.take();
                let mut previous = self.copy_instance();
                previous.hidden_effect = previous_hidden;
                self.hidden_effect = Some(Box::new(previous));
            }
            self.amplifier = incoming.amplifier;
            self.duration = incoming.duration;
            changed = true;
        } else if self.is_shorter_duration_than(incoming) {
            if incoming.amplifier == self.amplifier {
                self.duration = incoming.duration;
                changed = true;
            } else if let Some(hidden) = self.hidden_effect.as_mut() {
                hidden.update_inner(incoming, mismatched_effects);
            } else {
                self.hidden_effect = Some(Box::new(incoming.copy_instance()));
            }
        }
        if (!incoming.ambient && self.ambient) || changed {
            self.ambient = incoming.ambient;
            changed = true;
        }
        if incoming.visible != self.visible {
            self.visible = incoming.visible;
            changed = true;
        }
        if incoming.show_icon != self.show_icon {
            self.show_icon = incoming.show_icon;
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    enum Event {
        Should(i32, i32),
        Apply(i32, i32),
        Remove(i32, i32),
        Promote(i32, i32),
        Blend,
    }

    struct Host {
        events: Vec<Event>,
        apply: bool,
        retain: bool,
        blend: i32,
        fail_remove: bool,
    }

    impl Host {
        fn new() -> Self {
            Self {
                events: Vec::new(),
                apply: true,
                retain: true,
                blend: 0,
                fail_remove: false,
            }
        }
    }

    impl EffectTickHost<u32> for Host {
        type Error = &'static str;

        fn should_apply_effect_tick_this_tick(
            &mut self,
            _: &u32,
            ticks: i32,
            amplifier: i32,
        ) -> Result<bool, Self::Error> {
            self.events.push(Event::Should(ticks, amplifier));
            Ok(self.apply)
        }

        fn apply_effect_tick(
            &mut self,
            instance: &mut EffectInstance<u32>,
            amplifier: i32,
        ) -> Result<bool, Self::Error> {
            self.events.push(Event::Apply(instance.duration(), amplifier));
            Ok(self.retain)
        }

        fn remove_effect(&mut self, instance: &mut EffectInstance<u32>) -> Result<(), Self::Error> {
            self.events.push(Event::Remove(instance.duration(), instance.amplifier()));
            if self.fail_remove {
                Err("missing actor remove callback")
            } else {
                Ok(())
            }
        }

        fn on_hidden_effect_promoted(
            &mut self,
            instance: &mut EffectInstance<u32>,
        ) -> Result<(), Self::Error> {
            self.events.push(Event::Promote(instance.duration(), instance.amplifier()));
            Ok(())
        }

        fn blend_duration_ticks(&mut self, _: &u32) -> Result<i32, Self::Error> {
            self.events.push(Event::Blend);
            Ok(self.blend)
        }
    }

    #[test]
    fn nested_hidden_countdown_matches_the_original_java_probe() {
        // This sequence was also executed against the unchanged pinned jar.
        let mut instance = EffectInstance::new(1, 20, 0);
        assert!(instance.update(&EffectInstance::new(1, 5, 1), |_, _| {
            panic!("same holder")
        }));
        assert!(instance.update(&EffectInstance::new(1, 3, 2), |_, _| {
            panic!("same holder")
        }));
        let mut host = Host::new();
        host.apply = false;

        for tick in 1..=3 {
            assert!(instance.tick(tick, &mut host).unwrap());
        }
        assert_eq!((instance.duration(), instance.amplifier()), (2, 1));
        let hidden = instance.hidden_effect().unwrap();
        assert_eq!((hidden.duration(), hidden.amplifier()), (17, 0));
        assert!(hidden.hidden_effect().is_none());
        assert!(host.events.contains(&Event::Promote(2, 1)));

        for tick in 4..=5 {
            assert!(instance.tick(tick, &mut host).unwrap());
        }
        assert_eq!((instance.duration(), instance.amplifier()), (15, 0));
        assert!(instance.hidden_effect().is_none());
        assert!(host.events.contains(&Event::Promote(15, 0)));
        assert!(!host.events.iter().any(|event| matches!(
            event,
            Event::Apply(_, _) | Event::Remove(_, _)
        )));

        for tick in 6..=19 {
            assert!(instance.tick(tick, &mut host).unwrap());
        }
        assert!(!instance.tick(20, &mut host).unwrap());
        assert_eq!(instance.duration(), 0);
    }

    #[test]
    fn stronger_short_effect_preserves_and_ages_the_hidden_source() {
        let mut instance = EffectInstance::new(1, 100, 0);
        assert!(instance.update(&EffectInstance::new(1, 2, 3), |_, _| panic!("same holder")));
        assert_eq!(instance.hidden_effect().unwrap().duration(), 100);
        let mut host = Host::new();
        assert!(instance.tick(501, &mut host).unwrap());
        assert_eq!(instance.duration(), 1);
        assert_eq!(instance.hidden_effect().unwrap().duration(), 99);
        host.events.clear();
        assert!(instance.tick(502, &mut host).unwrap());
        assert_eq!(instance.duration(), 98);
        assert_eq!(instance.amplifier(), 0);
        assert!(instance.hidden_effect().is_none());
        assert_eq!(
            host.events,
            vec![
                Event::Should(1, 3),
                Event::Apply(1, 3),
                Event::Promote(98, 0),
                Event::Blend,
            ]
        );
    }

    #[test]
    fn hidden_only_update_returns_false_and_java_copy_drops_hidden_and_blend() {
        let mut instance = EffectInstance::new(1, 20, 2);
        instance.skip_blending(0);
        assert!(!instance.update(&EffectInstance::new(1, 80, 1), |_, _| panic!("same holder")));
        assert_eq!(instance.hidden_effect().unwrap().duration(), 80);
        let mut copied = instance.copy_instance();
        assert!(copied.hidden_effect().is_none());
        assert_eq!(copied.blend_factor(false, 1.0), 0.0);
        assert_eq!(instance.blend_factor(false, 1.0), 1.0);
        let mut deep_snapshot = instance.clone();
        assert_eq!(deep_snapshot.hidden_effect().unwrap().duration(), 80);
        assert_eq!(deep_snapshot.blend_factor(false, 1.0), 1.0);
    }

    #[test]
    fn apply_rejection_removes_before_countdown_and_propagates_callback_failure() {
        let mut instance = EffectInstance::new(1, 2, 0);
        let mut host = Host::new();
        host.retain = false;
        host.fail_remove = true;
        assert_eq!(instance.tick(9, &mut host), Err("missing actor remove callback"));
        assert_eq!(instance.duration(), 2);
        assert_eq!(
            host.events,
            vec![Event::Should(2, 0), Event::Apply(2, 0), Event::Remove(2, 0)]
        );
        host.fail_remove = false;
        host.events.clear();
        assert!(instance.tick(9, &mut host).unwrap());
        assert_eq!(instance.duration(), 1);
        assert_eq!(
            host.events,
            vec![
                Event::Should(2, 0),
                Event::Apply(2, 0),
                Event::Remove(2, 0),
                Event::Blend,
            ]
        );
    }

    #[test]
    fn infinite_tick_uses_actual_actor_counter_and_still_ages_hidden() {
        let mut instance = EffectInstance::with_hidden(1, -1, 1, false, true, true,
            Some(Box::new(EffectInstance::new(1, 8, 0))));
        let mut host = Host::new();
        assert!(instance.tick(i32::MIN, &mut host).unwrap());
        assert_eq!(instance.duration(), -1);
        assert_eq!(instance.hidden_effect().unwrap().duration(), 7);
        assert_eq!(host.events[0], Event::Should(i32::MIN, 1));
        assert!(!instance.ends_within(i32::MAX));
        assert_eq!(instance.map_duration(|_| panic!("infinite bypasses mapping")), -1);
    }

    #[test]
    fn exhausted_instance_still_ticks_blend_without_reviving_hidden() {
        let mut instance = EffectInstance::with_hidden(1, 0, 0, false, true, true,
            Some(Box::new(EffectInstance::new(1, 8, 0))));
        let mut host = Host::new();
        assert!(!instance.tick(1, &mut host).unwrap());
        assert_eq!(instance.hidden_effect().unwrap().duration(), 8);
        assert_eq!(host.events, vec![Event::Blend]);
        assert_eq!(instance.blend_factor(false, 1.0), 1.0);
    }

    #[test]
    fn duration_and_flags_follow_infinite_and_ambient_rules() {
        let mut instance = EffectInstance::with_flags(1, -1, 999, false, true, true);
        assert_eq!(instance.amplifier(), 255);
        assert!(!instance.update(&EffectInstance::with_flags(1, 5, 255, true, true, true), |_, _| {}));
        assert_eq!(instance.duration(), -1);
        assert!(!instance.is_ambient());
        assert!(instance.update(&EffectInstance::with_flags(1, -1, 255, true, false, false), |_, _| {}));
        assert!(!instance.is_ambient());
        assert!(!instance.is_visible());
        assert!(!instance.show_icon());
        let mut warnings = Vec::new();
        assert!(instance.update(&EffectInstance::new(2, 3, 0), |a, b| warnings.push((*a, *b))));
        assert_eq!(warnings, vec![(1, 2)]);
        assert_eq!(*instance.effect(), 1);
    }

    #[test]
    fn blend_uses_post_decrement_duration_and_removed_actor_collapses_interpolation() {
        let mut instance = EffectInstance::new(1, 5, 0);
        let mut host = Host::new();
        host.blend = 2;
        instance.tick(1, &mut host).unwrap();
        assert_eq!(instance.blend_factor(false, 0.5), 0.25);
        assert_eq!(
            host.events,
            vec![Event::Should(5, 0), Event::Apply(5, 0), Event::Blend, Event::Blend]
        );
        instance.tick(2, &mut host).unwrap();
        assert_eq!(instance.blend_factor(true, 0.0), 1.0);
        instance.tick(3, &mut host).unwrap();
        assert_eq!(instance.duration(), 2);
        assert_eq!(instance.blend_factor(false, 1.0), 0.5);
    }
}
