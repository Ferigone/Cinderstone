//! LivingEntity.tickEffects (Java 1.21 lines 808–848).
//!
//! The actor's real map, fail-fast iterator, synchronized metadata, level and
//! RNG remain owned by the embedding runtime. The selected instance is retained
//! across callbacks even if its holder's map entry is removed or replaced.

use super::retained::{tick_retained, RetainedEffectTickHost};

/// An original Java exception or the exact null dereference after map.get.
#[derive(Debug, PartialEq, Eq)]
pub enum LivingEffectsTickError<E> {
    Host(E),
    NullEffect,
}

/// Mandatory access to the actual living actor's tickEffects dependencies.
///
/// Handles retain their source object identities. No method may substitute a
/// snapshot map, independent RNG or replacement actor after entity-ID reuse.
/// Iterator operations use the real live key-set iterator; in particular,
/// remove affects its selected key even when callbacks replaced that key's
/// value. No mutable actor/instance borrow may span a callback. The inherited
/// hidden-promotion callback must run virtual onEffectUpdated(instance, true,
/// null), on the same retained instance, synchronously.
pub trait LivingEffectsTickHost<K>: RetainedEffectTickHost<K> {
    type KeyCursor;
    type Level;
    type Random;
    type ParticleList;
    type Particle;

    fn effect_key_iterator(&mut self, actor: &Self::Actor) -> Result<Self::KeyCursor, Self::Error>;
    fn effect_keys_has_next(&mut self, cursor: &mut Self::KeyCursor) -> Result<bool, Self::Error>;
    fn effect_keys_next(&mut self, cursor: &mut Self::KeyCursor) -> Result<K, Self::Error>;
    fn effect_keys_remove(&mut self, cursor: &mut Self::KeyCursor) -> Result<(), Self::Error>;
    fn active_effect_get(
        &mut self,
        actor: &Self::Actor,
        key: &K,
    ) -> Result<Option<Self::Lease>, Self::Error>;

    /// Classify only actual ConcurrentModificationException, including one
    /// thrown by a nested callback. The source catches it only inside the loop.
    fn is_concurrent_modification(&self, error: &Self::Error) -> bool;

    fn actor_level(&mut self, actor: &Self::Actor) -> Result<Self::Level, Self::Error>;
    fn level_is_client_side(&mut self, level: &Self::Level) -> Result<bool, Self::Error>;
    fn on_effect_removed(
        &mut self,
        actor: &Self::Actor,
        retained: &Self::Lease,
    ) -> Result<(), Self::Error>;
    /// Virtual onEffectUpdated(instance, false, null).
    fn on_effect_updated_periodic(
        &mut self,
        actor: &Self::Actor,
        retained: &Self::Lease,
    ) -> Result<(), Self::Error>;

    fn effects_dirty(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn set_effects_dirty(&mut self, actor: &Self::Actor, value: bool) -> Result<(), Self::Error>;
    /// Actual virtual updateInvisibilityStatus, including its concrete
    /// subclass override and synchronized effect-particle metadata updates.
    fn update_invisibility_status(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Actual Entity.isCurrentlyGlowing (not just the existing shared flag).
    fn is_currently_glowing(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn shared_glowing_flag(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn set_shared_glowing_flag(&mut self, actor: &Self::Actor, value: bool) -> Result<(), Self::Error>;

    fn effect_particle_list(&mut self, actor: &Self::Actor) -> Result<Self::ParticleList, Self::Error>;
    fn particle_list_is_empty(&mut self, list: &Self::ParticleList) -> Result<bool, Self::Error>;
    fn particle_list_size(&mut self, list: &Self::ParticleList) -> Result<i32, Self::Error>;
    fn particle_list_get(
        &mut self,
        list: &Self::ParticleList,
        index: i32,
    ) -> Result<Self::Particle, Self::Error>;
    fn effect_particles_ambient(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_invisible(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn actor_random(&mut self, actor: &Self::Actor) -> Result<Self::Random, Self::Error>;
    fn random_next_int(&mut self, random: &Self::Random, bound: i32) -> Result<i32, Self::Error>;
    /// Actual Entity.getRandomX/getRandomY/getRandomZ, retaining their virtual
    /// source behavior and draws from the actor's then-current random field.
    fn random_x(&mut self, actor: &Self::Actor, scale: f64) -> Result<f64, Self::Error>;
    fn random_y(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn random_z(&mut self, actor: &Self::Actor, scale: f64) -> Result<f64, Self::Error>;
    fn add_particle(
        &mut self,
        level: &Self::Level,
        particle: &Self::Particle,
        position: [f64; 3],
        velocity: [f64; 3],
    ) -> Result<(), Self::Error>;
}

/// Run the private source updateGlowingStatus, preserving the captured result
/// of isCurrentlyGlowing across the subsequent shared-flag read.
pub fn update_glowing_status<K, H: LivingEffectsTickHost<K>>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    let glowing = host.is_currently_glowing(actor)?;
    if host.shared_glowing_flag(actor)? != glowing {
        host.set_shared_glowing_flag(actor, glowing)?;
    }
    Ok(())
}

fn tick_effect_loop<K: Clone, H: LivingEffectsTickHost<K>>(
    actor: &H::Actor,
    cursor: &mut H::KeyCursor,
    host: &mut H,
) -> Result<(), LivingEffectsTickError<H::Error>> {
    use LivingEffectsTickError::{Host, NullEffect};
    while host.effect_keys_has_next(cursor).map_err(Host)? {
        let key = host.effect_keys_next(cursor).map_err(Host)?;
        let retained = host.active_effect_get(actor, &key).map_err(Host)?.ok_or(NullEffect)?;
        if !tick_retained(actor, &retained, host).map_err(Host)? {
            let level = host.actor_level(actor).map_err(Host)?;
            if !host.level_is_client_side(&level).map_err(Host)? {
                host.effect_keys_remove(cursor).map_err(Host)?;
                host.on_effect_removed(actor, &retained).map_err(Host)?;
            }
        } else if host.with_instance(&retained, |instance| instance.duration()).map_err(Host)? % 600 == 0 {
            host.on_effect_updated_periodic(actor, &retained).map_err(Host)?;
        }
    }
    Ok(())
}

/// Execute one actual LivingEntity tickEffects call. This function does not
/// create or schedule an actor or infer an active effect map from a player ID.
/// Source ConcurrentModificationException ends only the iteration phase;
/// dirty visibility and particles still run. Other errors preserve their
/// committed prefix and stop immediately.
pub fn tick_effects<K: Clone, H: LivingEffectsTickHost<K>>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), LivingEffectsTickError<H::Error>> {
    use LivingEffectsTickError::Host;
    // Iterator creation precedes the source try range.
    let mut cursor = host.effect_key_iterator(actor).map_err(Host)?;
    match tick_effect_loop(actor, &mut cursor, host) {
        Err(Host(error)) if host.is_concurrent_modification(&error) => {}
        result => result?,
    }
    if host.effects_dirty(actor).map_err(Host)? {
        let level = host.actor_level(actor).map_err(Host)?;
        if !host.level_is_client_side(&level).map_err(Host)? {
            host.update_invisibility_status(actor).map_err(Host)?;
            update_glowing_status(actor, host).map_err(Host)?;
        }
        host.set_effects_dirty(actor, false).map_err(Host)?;
    }
    let particles = host.effect_particle_list(actor).map_err(Host)?;
    if !host.particle_list_is_empty(&particles).map_err(Host)? {
        let ambient = host.effect_particles_ambient(actor).map_err(Host)?;
        let visibility_bound = if host.is_invisible(actor).map_err(Host)? { 15 } else { 4 };
        let ambient_factor = if ambient { 5 } else { 1 };
        let random = host.actor_random(actor).map_err(Host)?;
        if host.random_next_int(&random, visibility_bound * ambient_factor).map_err(Host)? == 0 {
            // Java evaluates the Level receiver before Util.getRandom's list
            // and RNG arguments, and captures RNG before reading list.size().
            let level = host.actor_level(actor).map_err(Host)?;
            let random = host.actor_random(actor).map_err(Host)?;
            let size = host.particle_list_size(&particles).map_err(Host)?;
            let index = host.random_next_int(&random, size).map_err(Host)?;
            let particle = host.particle_list_get(&particles, index).map_err(Host)?;
            let x = host.random_x(actor, 0.5).map_err(Host)?;
            let y = host.random_y(actor).map_err(Host)?;
            let z = host.random_z(actor, 0.5).map_err(Host)?;
            host.add_particle(&level, &particle, [x, y, z], [1.0; 3]).map_err(Host)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::EffectInstance;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    #[derive(Clone)]
    struct Lease {
        id: u32,
        object: Rc<RefCell<EffectInstance<u32>>>,
    }
    impl Lease {
        fn new(id: u32, duration: i32) -> Self {
            Self {
                id,
                object: Rc::new(RefCell::new(EffectInstance::new(7, duration, 0))),
            }
        }
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Error {
        Cme,
        Fault,
    }
    struct Cursor {
        has_entry: bool,
        visited: bool,
        expected_mod_count: u64,
    }
    // One controlled key schedule and explicit structural-change boundaries.
    // This is not a native Java HashMap, metadata, Level, actor or RNG owner.
    struct Host {
        calls: Vec<String>,
        objects: Vec<Lease>,
        current: Option<Lease>,
        replacement_on_apply: Option<Lease>,
        mod_count: u64,
        null_get: bool,
        should_apply: bool,
        apply_result: bool,
        failure: Option<(&'static str, Error)>,
        dirty: bool,
        client: bool,
        glowing: bool,
        shared_glow: bool,
        change_glow_during_flag_read: bool,
        particle_empty: bool,
        ambient: bool,
        invisible: bool,
        level: u32,
        particle_list: u32,
        random: u32,
        random_reads: usize,
        draws: VecDeque<i32>,
        mutate_particle_context: bool,
    }
    impl Default for Host {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                objects: Vec::new(),
                current: None,
                replacement_on_apply: None,
                mod_count: 0,
                null_get: false,
                should_apply: false,
                apply_result: true,
                failure: None,
                dirty: false,
                client: false,
                glowing: false,
                shared_glow: false,
                change_glow_during_flag_read: false,
                particle_empty: true,
                ambient: false,
                invisible: false,
                level: 10,
                particle_list: 1,
                random: 2,
                random_reads: 0,
                draws: VecDeque::from([0, 1]),
                mutate_particle_context: false,
            }
        }
    }
    impl Host {
        fn hit(&mut self, call: impl Into<String>) -> Result<(), Error> {
            let call = call.into();
            let error = self
                .failure
                .filter(|(name, _)| *name == call)
                .map(|(_, error)| error);
            self.calls.push(call);
            error.map_or(Ok(()), Err)
        }
        fn callback(&mut self, call: impl Into<String>) -> Result<(), Error> {
            for lease in &self.objects {
                assert!(
                    lease.object.try_borrow_mut().is_ok(),
                    "instance borrowed across callback"
                );
            }
            self.hit(call)
        }
        fn install(&mut self, lease: &Lease) {
            self.objects.push(lease.clone());
            self.current = Some(lease.clone());
        }
        fn index(&self, name: &str) -> usize {
            self.calls.iter().position(|call| call == name).unwrap()
        }
    }
    impl RetainedEffectTickHost<u32> for Host {
        type Error = Error;
        type Actor = u64;
        type Lease = Lease;
        fn with_instance<R>(
            &mut self,
            retained: &Lease,
            access: impl FnOnce(&mut EffectInstance<u32>) -> R,
        ) -> Result<R, Error> {
            Ok(access(&mut retained.object.borrow_mut()))
        }
        fn actor_tick_count(&mut self, actor: &u64) -> Result<i32, Error> {
            assert_eq!(*actor, 50);
            self.callback("actor-tick")?;
            Ok(100)
        }
        fn should_apply_effect_tick_this_tick(
            &mut self,
            _: &u32,
            argument: i32,
            amplifier: i32,
        ) -> Result<bool, Error> {
            self.callback(format!("should:{argument}:{amplifier}"))?;
            Ok(self.should_apply)
        }
        fn apply_effect_tick(&mut self, _: &u64, _: &u32, amplifier: i32) -> Result<bool, Error> {
            self.callback(format!("apply:{amplifier}"))?;
            if let Some(replacement) = self.replacement_on_apply.take() {
                // Existing-key replacement is nonstructural in the source map.
                self.current = Some(replacement);
            }
            Ok(self.apply_result)
        }
        fn remove_effect(&mut self, _: &u64, _: &u32) -> Result<(), Error> {
            self.callback(format!(
                "remove-effect:{:?}",
                self.current.as_ref().map(|lease| lease.id)
            ))?;
            if self.current.take().is_some() {
                self.mod_count += 1;
            }
            Ok(())
        }
        fn on_hidden_effect_promoted(&mut self, _: &u64, retained: &Lease) -> Result<(), Error> {
            let duration = retained.object.borrow().duration();
            self.callback(format!("promoted:{}:{duration}", retained.id))
        }
        fn blend_duration_ticks(&mut self, _: &u32) -> Result<i32, Error> {
            self.callback("blend")?;
            Ok(5)
        }
    }
    impl LivingEffectsTickHost<u32> for Host {
        type KeyCursor = Cursor;
        type Level = u32;
        type Random = u32;
        type ParticleList = u32;
        type Particle = u32;
        fn effect_key_iterator(&mut self, _: &u64) -> Result<Cursor, Error> {
            self.hit("iterator")?;
            Ok(Cursor {
                has_entry: self.current.is_some(),
                visited: false,
                expected_mod_count: self.mod_count,
            })
        }
        fn effect_keys_has_next(&mut self, cursor: &mut Cursor) -> Result<bool, Error> {
            self.hit("has-next")?;
            Ok(cursor.has_entry && !cursor.visited)
        }
        fn effect_keys_next(&mut self, cursor: &mut Cursor) -> Result<u32, Error> {
            self.hit("next")?;
            if cursor.expected_mod_count != self.mod_count {
                return Err(Error::Cme);
            }
            cursor.visited = true;
            Ok(7)
        }
        fn effect_keys_remove(&mut self, cursor: &mut Cursor) -> Result<(), Error> {
            self.hit(format!(
                "iterator-remove:{:?}",
                self.current.as_ref().map(|lease| lease.id)
            ))?;
            if cursor.expected_mod_count != self.mod_count {
                return Err(Error::Cme);
            }
            self.current = None;
            self.mod_count += 1;
            cursor.expected_mod_count = self.mod_count;
            Ok(())
        }
        fn active_effect_get(&mut self, _: &u64, key: &u32) -> Result<Option<Lease>, Error> {
            self.hit(format!("get:{key}"))?;
            Ok(if self.null_get {
                None
            } else {
                self.current.clone()
            })
        }
        fn is_concurrent_modification(&self, error: &Error) -> bool {
            *error == Error::Cme
        }
        fn actor_level(&mut self, _: &u64) -> Result<u32, Error> {
            self.hit("level")?;
            Ok(self.level)
        }
        fn level_is_client_side(&mut self, level: &u32) -> Result<bool, Error> {
            self.hit(format!("client:{level}"))?;
            Ok(self.client)
        }
        fn on_effect_removed(&mut self, _: &u64, retained: &Lease) -> Result<(), Error> {
            self.callback(format!(
                "removed:{}:current{:?}",
                retained.id,
                self.current.as_ref().map(|lease| lease.id)
            ))
        }
        fn on_effect_updated_periodic(&mut self, _: &u64, retained: &Lease) -> Result<(), Error> {
            let duration = retained.object.borrow().duration();
            self.callback(format!("periodic:{}:{duration}", retained.id))
        }
        fn effects_dirty(&mut self, _: &u64) -> Result<bool, Error> {
            self.hit("dirty")?;
            Ok(self.dirty)
        }
        fn set_effects_dirty(&mut self, _: &u64, value: bool) -> Result<(), Error> {
            self.hit(format!("set-dirty:{value}"))?;
            self.dirty = value;
            Ok(())
        }
        fn update_invisibility_status(&mut self, _: &u64) -> Result<(), Error> {
            self.callback("visibility")
        }
        fn is_currently_glowing(&mut self, _: &u64) -> Result<bool, Error> {
            self.callback("currently-glowing")?;
            Ok(self.glowing)
        }
        fn shared_glowing_flag(&mut self, _: &u64) -> Result<bool, Error> {
            self.callback("shared-glow")?;
            if self.change_glow_during_flag_read {
                self.glowing = !self.glowing;
            }
            Ok(self.shared_glow)
        }
        fn set_shared_glowing_flag(&mut self, _: &u64, value: bool) -> Result<(), Error> {
            self.callback(format!("set-glow:{value}"))?;
            self.shared_glow = value;
            Ok(())
        }
        fn effect_particle_list(&mut self, _: &u64) -> Result<u32, Error> {
            self.hit("particle-list")?;
            Ok(self.particle_list)
        }
        fn particle_list_is_empty(&mut self, list: &u32) -> Result<bool, Error> {
            self.hit(format!("empty:{list}"))?;
            Ok(self.particle_empty)
        }
        fn particle_list_size(&mut self, list: &u32) -> Result<i32, Error> {
            self.hit(format!("size:{list}"))?;
            if self.mutate_particle_context {
                self.random = 9;
            }
            Ok(if *list == 1 { 2 } else { 99 })
        }
        fn particle_list_get(&mut self, list: &u32, index: i32) -> Result<u32, Error> {
            self.hit(format!("particle:{list}:{index}"))?;
            Ok(*list * 100 + index as u32)
        }
        fn effect_particles_ambient(&mut self, _: &u64) -> Result<bool, Error> {
            self.hit("ambient")?;
            Ok(self.ambient)
        }
        fn is_invisible(&mut self, _: &u64) -> Result<bool, Error> {
            self.hit("invisible")?;
            Ok(self.invisible)
        }
        fn actor_random(&mut self, _: &u64) -> Result<u32, Error> {
            self.hit("random")?;
            self.random_reads += 1;
            if self.mutate_particle_context && self.random_reads == 2 {
                self.level = 99;
                self.particle_list = 2;
            }
            Ok(self.random)
        }
        fn random_next_int(&mut self, random: &u32, bound: i32) -> Result<i32, Error> {
            self.hit(format!("draw:{random}:{bound}"))?;
            Ok(self.draws.pop_front().unwrap())
        }
        fn random_x(&mut self, _: &u64, scale: f64) -> Result<f64, Error> {
            self.hit(format!("x:{scale}:rng{}", self.random))?;
            Ok(1.0)
        }
        fn random_y(&mut self, _: &u64) -> Result<f64, Error> {
            self.hit(format!("y:rng{}", self.random))?;
            Ok(2.0)
        }
        fn random_z(&mut self, _: &u64, scale: f64) -> Result<f64, Error> {
            self.hit(format!("z:{scale}:rng{}", self.random))?;
            Ok(3.0)
        }
        fn add_particle(
            &mut self,
            level: &u32,
            particle: &u32,
            position: [f64; 3],
            velocity: [f64; 3],
        ) -> Result<(), Error> {
            self.callback(format!(
                "add-particle:{level}:{particle}:{position:?}:{velocity:?}"
            ))
        }
    }

    #[test]
    fn actual_retained_tick_expires_old_object_but_iterator_removes_current_replacement() {
        let original = Lease::new(1, 1);
        let replacement = Lease::new(2, 40);
        let mut host = Host {
            should_apply: true,
            replacement_on_apply: Some(replacement.clone()),
            ..Host::default()
        };
        host.install(&original);
        host.objects.push(replacement.clone());
        tick_effects(&50, &mut host).unwrap();
        assert_eq!(original.object.borrow().duration(), 0);
        assert_eq!(replacement.object.borrow().duration(), 40);
        assert!(host.current.is_none());
        assert!(host.index("iterator-remove:Some(2)") < host.index("removed:1:currentNone"));

        let mut client = Host {
            client: true,
            dirty: true,
            ..Host::default()
        };
        let original = Lease::new(3, 1);
        client.install(&original);
        tick_effects(&50, &mut client).unwrap();
        assert_eq!(original.object.borrow().duration(), 0);
        assert_eq!(client.current.as_ref().unwrap().id, 3);
        assert!(!client
            .calls
            .iter()
            .any(|call| call.starts_with("iterator-remove")
                || call.starts_with("removed:")
                || call == "visibility"
                || call == "currently-glowing"));
        assert!(!client.dirty);
    }

    #[test]
    fn hidden_promotion_and_periodic_six_hundred_update_use_same_retained_instance() {
        let original = Lease {
            id: 1,
            object: Rc::new(RefCell::new(EffectInstance::with_hidden(
                7,
                1,
                3,
                false,
                true,
                true,
                Some(Box::new(EffectInstance::new(7, 601, 1))),
            ))),
        };
        let mut host = Host::default();
        host.install(&original);
        tick_effects(&50, &mut host).unwrap();
        assert_eq!(original.object.borrow().duration(), 600);
        assert_eq!(original.object.borrow().amplifier(), 1);
        assert!(host.index("promoted:1:600") < host.index("periodic:1:600"));
        assert!(!host.calls.iter().any(|call| call.starts_with("removed:")));
    }

    #[test]
    fn catch_covers_only_loop_cme_and_not_creation_dirty_tail_or_null_dereference() {
        let mut creation = Host {
            failure: Some(("iterator", Error::Cme)),
            ..Host::default()
        };
        assert_eq!(
            tick_effects(&50, &mut creation),
            Err(LivingEffectsTickError::Host(Error::Cme))
        );
        assert_eq!(creation.calls, ["iterator"]);

        let original = Lease::new(1, 601);
        let mut caught = Host {
            failure: Some(("periodic:1:600", Error::Cme)),
            dirty: true,
            ..Host::default()
        };
        caught.install(&original);
        tick_effects(&50, &mut caught).unwrap();
        assert_eq!(original.object.borrow().duration(), 600);
        assert!(caught.index("periodic:1:600") < caught.index("visibility"));
        assert!(!caught.dirty);

        let mut dirty = Host {
            failure: Some(("visibility", Error::Cme)),
            dirty: true,
            ..Host::default()
        };
        assert_eq!(
            tick_effects(&50, &mut dirty),
            Err(LivingEffectsTickError::Host(Error::Cme))
        );
        assert!(dirty.dirty);
        assert_eq!(dirty.calls.last().unwrap(), "visibility");

        let mut tail = Host {
            failure: Some(("particle-list", Error::Cme)),
            ..Host::default()
        };
        assert_eq!(
            tick_effects(&50, &mut tail),
            Err(LivingEffectsTickError::Host(Error::Cme))
        );
        assert_eq!(tail.calls.last().unwrap(), "particle-list");

        let mut null = Host {
            null_get: true,
            ..Host::default()
        };
        null.install(&Lease::new(2, 10));
        assert_eq!(
            tick_effects(&50, &mut null),
            Err(LivingEffectsTickError::NullEffect)
        );
        assert_eq!(null.calls.last().unwrap(), "get:7");

        let mut other = Host {
            failure: Some(("next", Error::Fault)),
            ..Host::default()
        };
        other.install(&Lease::new(3, 10));
        assert_eq!(
            tick_effects(&50, &mut other),
            Err(LivingEffectsTickError::Host(Error::Fault))
        );
        assert_eq!(other.calls.last().unwrap(), "next");
    }

    #[test]
    fn dirty_clears_after_visibility_and_glow_using_value_captured_before_flag_read() {
        let mut host = Host {
            dirty: true,
            glowing: true,
            change_glow_during_flag_read: true,
            ..Host::default()
        };
        tick_effects(&50, &mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                "iterator",
                "has-next",
                "dirty",
                "level",
                "client:10",
                "visibility",
                "currently-glowing",
                "shared-glow",
                "set-glow:true",
                "set-dirty:false",
                "particle-list",
                "empty:1",
            ]
        );
        assert!(!host.glowing);
        assert!(host.shared_glow);
        assert!(!host.dirty);
    }

    #[test]
    fn particles_keep_source_level_list_and_rng_locals_then_read_current_coordinate_rng() {
        let mut host = Host {
            particle_empty: false,
            ambient: true,
            invisible: true,
            mutate_particle_context: true,
            ..Host::default()
        };
        tick_effects(&50, &mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                "iterator",
                "has-next",
                "dirty",
                "particle-list",
                "empty:1",
                "ambient",
                "invisible",
                "random",
                "draw:2:75",
                "level",
                "random",
                "size:1",
                "draw:2:2",
                "particle:1:1",
                "x:0.5:rng9",
                "y:rng9",
                "z:0.5:rng9",
                "add-particle:10:101:[1.0, 2.0, 3.0]:[1.0, 1.0, 1.0]",
            ]
        );
        assert_eq!(host.level, 99);
        assert_eq!(host.particle_list, 2);
    }

    #[test]
    fn particle_gate_bounds_and_empty_list_skip_unreached_draws_and_world_callbacks() {
        for (ambient, invisible, bound) in [
            (false, false, 4),
            (true, false, 20),
            (false, true, 15),
            (true, true, 75),
        ] {
            let mut host = Host {
                particle_empty: false,
                ambient,
                invisible,
                draws: VecDeque::from([1]),
                ..Host::default()
            };
            tick_effects(&50, &mut host).unwrap();
            assert_eq!(host.calls.last().unwrap(), &format!("draw:2:{bound}"));
            assert!(!host.calls.iter().any(|call| call == "level"
                || call.starts_with("size:")
                || call.starts_with("add-particle:")));
        }
        let mut empty = Host::default();
        tick_effects(&50, &mut empty).unwrap();
        assert_eq!(
            empty.calls,
            ["iterator", "has-next", "dirty", "particle-list", "empty:1"]
        );
        assert_eq!(empty.draws, [0, 1]);
    }
}
