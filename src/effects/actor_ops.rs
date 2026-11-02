//! Java 1.21 LivingEntity effect collection operations: removeAllEffects
//! 944–955, addEffect 980–1000, forceAddEffect 1017–1028,
//! removeEffectNoUpdate 1036 and removeEffect 1040–1045.
//!
//! These bodies own no actor, active-effects map, holder identity/hash policy,
//! instance arena, attributes, definition, RNG, packet queue or scheduler. The
//! actual runtime supplies canonical retained objects and complete virtual
//! callbacks. Collection ticking and concrete effect callbacks are separate.

use super::EffectInstance;

/// Short access to the actual target and source objects. The owner determines
/// aliasing by canonical object identity, never equality of values/map keys.
/// `Same` borrows the object only once, avoiding conflicting mutable/shared
/// borrows when Java calls existing.update(existing) or copies its own blend.
pub enum InstancePair<'a, K> {
    Same(&'a mut EffectInstance<K>),
    Distinct {
        target: &'a mut EffectInstance<K>,
        source: &'a EffectInstance<K>,
    },
}

/// Mandatory operations on one actual actor and its real active-effects map.
///
/// Actor/Entity/Lease handles retain their exact source objects through reentry,
/// map replacement/removal and reusable wire entity IDs. A Lease must not look
/// up the current map value by holder each time. Map operations adopt/return the
/// same actual instances; they never allocate copies or temporary empty maps.
/// Holder cloning retains canonical holder identity and native equality/hash.
/// Missing owners must error rather than provide successful empty behavior.
///
/// Every access closure returns before an actor/definition callback runs. Do
/// not move an instance out of its owner or hold a map/actor/instance borrow
/// across those callbacks. Errors preserve the completed mutation/callback
/// prefix; these source operations are not rollback transactions or retry plans.
pub trait ActorEffectHost<K> {
    type Error;
    type Actor: ?Sized;
    type Entity: ?Sized;
    type Lease;
    type ValuesCursor;

    /// Actual current Level.isClientSide query for removeAllEffects.
    fn is_client_side(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual virtual actor.canBeAffected(incoming). Its base ordered tag/holder
    /// checks and any real subtype override must remain native operations.
    fn can_be_affected(
        &mut self,
        actor: &Self::Actor,
        incoming: &Self::Lease,
    ) -> Result<bool, Self::Error>;
    /// Callback-free read of the exact retained instance, including after its
    /// map entry was replaced. Lookup errors precede execution of the closure.
    fn with_instance<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(&EffectInstance<K>) -> R,
    ) -> Result<R, Self::Error>;
    /// Short canonical-object access for native update/copyBlendState. `Same`
    /// must be selected iff these leases retain the same actual instance.
    /// The independent reporter is the actual synchronous mismatched-effect
    /// diagnostic used by EffectInstance.update; do not defer that warning
    /// until after mutation. Only that diagnostic may run inside this fragment;
    /// no actor/definition/world callbacks or object borrow may escape it.
    fn with_instance_pair<R>(
        &mut self,
        target: &Self::Lease,
        source: &Self::Lease,
        access: impl FnOnce(InstancePair<'_, K>, &mut dyn FnMut(&K, &K)) -> R,
    ) -> Result<R, Self::Error>;
    fn map_get(
        &mut self,
        actor: &Self::Actor,
        holder: &K,
    ) -> Result<Option<Self::Lease>, Self::Error>;
    /// Actual activeEffects.put; incoming is visible before later callbacks.
    /// Return/retain the actual previous value, not a detached snapshot.
    fn map_put(
        &mut self,
        actor: &Self::Actor,
        holder: K,
        incoming: &Self::Lease,
    ) -> Result<Option<Self::Lease>, Self::Error>;
    fn map_remove(
        &mut self,
        actor: &Self::Actor,
        holder: &K,
    ) -> Result<Option<Self::Lease>, Self::Error>;

    /// Actual virtual actor.onEffectAdded(incoming,cause), including complete
    /// LivingEntity/ServerPlayer attribute, passenger/self packet and criterion
    /// behavior. Definition instance.onEffectAdded is a separate later call.
    fn on_effect_added(
        &mut self,
        actor: &Self::Actor,
        incoming: &Self::Lease,
        cause: Option<&Self::Entity>,
    ) -> Result<(), Self::Error>;
    fn on_effect_updated(
        &mut self,
        actor: &Self::Actor,
        existing: &Self::Lease,
        reapply: bool,
        cause: Option<&Self::Entity>,
    ) -> Result<(), Self::Error>;
    fn on_effect_removed(
        &mut self,
        actor: &Self::Actor,
        retained: &Self::Lease,
    ) -> Result<(), Self::Error>;
    /// Actual virtual MobEffectInstance.onEffectAdded(actor), including the
    /// current definition/amplifier reads and its real sound/world/RNG work.
    fn instance_on_effect_added(
        &mut self,
        actor: &Self::Actor,
        incoming: &Self::Lease,
    ) -> Result<(), Self::Error>;
    /// Actual virtual MobEffectInstance.onEffectStarted(actor). addEffect calls
    /// this on INCOMING even when updating the existing object returned false.
    fn instance_on_effect_started(
        &mut self,
        actor: &Self::Actor,
        incoming: &Self::Lease,
    ) -> Result<(), Self::Error>;
    /// Preserve removeEffect's virtual call. A selected base body can invoke
    /// remove_effect_no_update below; an override must not be bypassed.
    fn virtual_remove_effect_no_update(
        &mut self,
        actor: &Self::Actor,
        holder: &K,
    ) -> Result<Option<Self::Lease>, Self::Error>;

    /// Actual activeEffects.values().iterator(), retaining its live owner.
    fn values_iterator(&mut self, actor: &Self::Actor) -> Result<Self::ValuesCursor, Self::Error>;
    /// Original HashMap iterator hasNext examines its prefetched next node;
    /// it does not check modCount. next and remove have their own source checks.
    fn iterator_has_next(&mut self, cursor: &mut Self::ValuesCursor) -> Result<bool, Self::Error>;
    fn iterator_next_retained(
        &mut self,
        cursor: &mut Self::ValuesCursor,
    ) -> Result<Self::Lease, Self::Error>;
    /// Actual iterator.remove, after onEffectRemoved. It removes the current
    /// entry/value even if a nonstructural replacement changed that value.
    /// Structural modification errors propagate; removeAllEffects catches none.
    fn iterator_remove(&mut self, cursor: &mut Self::ValuesCursor) -> Result<(), Self::Error>;
}

fn holder<K: Clone, H: ActorEffectHost<K> + ?Sized>(
    retained: &H::Lease,
    host: &mut H,
) -> Result<K, H::Error> {
    host.with_instance(retained, |instance| instance.effect().clone())
}

/// Actual existing.update(incoming), sharing the native instance arithmetic.
/// The alias case is provably false without mutation: all comparisons see the
/// same amplifier/duration/flags and the holder cannot mismatch itself. No
/// hidden/blend clone, mutable/shared alias or substitute instance is required.
fn update<K: Clone + PartialEq, H: ActorEffectHost<K> + ?Sized>(
    existing: &H::Lease,
    incoming: &H::Lease,
    host: &mut H,
) -> Result<bool, H::Error> {
    host.with_instance_pair(existing, incoming, |pair, warning| match pair {
        InstancePair::Same(_) => false,
        InstancePair::Distinct { target, source } => {
            target.update(source, |left, right| warning(left, right))
        }
    })
}

/// LivingEntity.addEffect980–1000. On rejection there is no map/start work.
/// The selected existing object remains retained if callbacks replace the map.
pub fn add_effect<K: Clone + PartialEq, H: ActorEffectHost<K> + ?Sized>(
    actor: &H::Actor,
    incoming: &H::Lease,
    cause: Option<&H::Entity>,
    host: &mut H,
) -> Result<bool, H::Error> {
    if !host.can_be_affected(actor, incoming)? {
        return Ok(false);
    }
    let key = holder(incoming, host)?;
    let existing = host.map_get(actor, &key)?;
    let mut changed = false;
    if let Some(existing) = existing {
        if update(&existing, incoming, host)? {
            host.on_effect_updated(actor, &existing, true, cause)?;
            changed = true;
        }
    } else {
        // The original reads incoming.getEffect again for the put operation.
        let key = holder(incoming, host)?;
        let _ = host.map_put(actor, key, incoming)?;
        host.on_effect_added(actor, incoming, cause)?;
        changed = true;
        host.instance_on_effect_added(actor, incoming)?;
    }
    host.instance_on_effect_started(actor, incoming)?;
    Ok(changed)
}

/// LivingEntity.forceAddEffect1017–1028. This path invokes neither instance
/// onEffectAdded nor onEffectStarted. The put precedes blend copy/virtual work.
pub fn force_add_effect<K: Clone, H: ActorEffectHost<K> + ?Sized>(
    actor: &H::Actor,
    incoming: &H::Lease,
    cause: Option<&H::Entity>,
    host: &mut H,
) -> Result<(), H::Error> {
    if !host.can_be_affected(actor, incoming)? {
        return Ok(());
    }
    let key = holder(incoming, host)?;
    if let Some(previous) = host.map_put(actor, key, incoming)? {
        host.with_instance_pair(incoming, &previous, |pair, _warning| match pair {
            InstancePair::Same(_) => (),
            InstancePair::Distinct { target, source } => target.copy_blend_state(source),
        })?;
        host.on_effect_updated(actor, incoming, true, cause)
    } else {
        host.on_effect_added(actor, incoming, cause)
    }
}

/// The selected base removeEffectNoUpdate1036 body is map removal only.
pub fn remove_effect_no_update<K, H: ActorEffectHost<K> + ?Sized>(
    actor: &H::Actor,
    holder: &K,
    host: &mut H,
) -> Result<Option<H::Lease>, H::Error> {
    host.map_remove(actor, holder)
}

/// LivingEntity.removeEffect1040–1045 removes through the actual virtual
/// removeEffectNoUpdate first, then notifies the exact returned old object.
pub fn remove_effect<K, H: ActorEffectHost<K> + ?Sized>(
    actor: &H::Actor,
    holder: &K,
    host: &mut H,
) -> Result<bool, H::Error> {
    if let Some(retained) = host.virtual_remove_effect_no_update(actor, holder)? {
        host.on_effect_removed(actor, &retained)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// LivingEntity.removeAllEffects944–955. Unlike removeEffect, the entry remains
/// visible during each virtual removal callback. The live iterator removes it
/// only after that callback succeeds; no concurrent-modification catch is added.
pub fn remove_all_effects<K, H: ActorEffectHost<K> + ?Sized>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<bool, H::Error> {
    if host.is_client_side(actor)? {
        return Ok(false);
    }
    let mut cursor = host.values_iterator(actor)?;
    let mut changed = false;
    while host.iterator_has_next(&mut cursor)? {
        let retained = host.iterator_next_retained(&mut cursor)?;
        host.on_effect_removed(actor, &retained)?;
        host.iterator_remove(&mut cursor)?;
        changed = true;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone)]
    struct Lease {
        id: u32,
        object: Rc<RefCell<EffectInstance<u32>>>,
    }
    impl Lease {
        fn new(id: u32, holder: u32, duration: i32, amplifier: i32) -> Self {
            Self {
                id,
                object: Rc::new(RefCell::new(EffectInstance::new(
                    holder, duration, amplifier,
                ))),
            }
        }
    }
    struct Cursor {
        // Controlled iteration schedule only, not a native HashMap/order claim.
        keys: Vec<u32>,
        next: usize,
        last: Option<u32>,
        expected_mod_count: u64,
    }
    struct Host {
        calls: Vec<String>,
        objects: Vec<Lease>,
        active: Vec<(u32, Lease)>,
        mod_count: u64,
        allowed: bool,
        client: bool,
        fail_at: Option<&'static str>,
        added_replacement: Option<Lease>,
        removed_replacement: Option<Lease>,
        removed_addition: Option<Lease>,
    }
    impl Default for Host {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                objects: Vec::new(),
                active: Vec::new(),
                mod_count: 0,
                allowed: true,
                client: false,
                fail_at: None,
                added_replacement: None,
                removed_replacement: None,
                removed_addition: None,
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
        fn callback(&mut self, call: impl Into<String>) -> Result<(), &'static str> {
            // Reentrant source callbacks must have access to ALL real objects.
            for lease in &self.objects {
                assert!(
                    lease.object.try_borrow_mut().is_ok(),
                    "object borrow crossed callback"
                );
            }
            self.call(call)
        }
        fn retain(&mut self, lease: &Lease) {
            self.objects.push(lease.clone());
        }
        fn current(&self, holder: u32) -> Option<u32> {
            self.active
                .iter()
                .find(|(key, _)| *key == holder)
                .map(|(_, lease)| lease.id)
        }
        fn put(&mut self, key: u32, incoming: &Lease) -> Option<Lease> {
            if let Some((_, value)) = self.active.iter_mut().find(|(holder, _)| *holder == key) {
                Some(std::mem::replace(value, incoming.clone()))
            } else {
                self.active.push((key, incoming.clone()));
                self.mod_count += 1;
                None
            }
        }
        fn remove(&mut self, holder: u32) -> Option<Lease> {
            let index = self.active.iter().position(|(key, _)| *key == holder)?;
            self.mod_count += 1;
            Some(self.active.remove(index).1)
        }
    }
    impl ActorEffectHost<u32> for Host {
        type Error = &'static str;
        type Actor = u64;
        type Entity = u64;
        type Lease = Lease;
        type ValuesCursor = Cursor;

        fn is_client_side(&mut self, actor: &u64) -> Result<bool, Self::Error> {
            assert_eq!(*actor, 50);
            self.call("client")?;
            Ok(self.client)
        }
        fn can_be_affected(&mut self, actor: &u64, incoming: &Lease) -> Result<bool, Self::Error> {
            assert_eq!(*actor, 50);
            self.callback(format!("can-affect:{}", incoming.id))?;
            Ok(self.allowed)
        }
        fn with_instance<R>(
            &mut self,
            retained: &Lease,
            access: impl FnOnce(&EffectInstance<u32>) -> R,
        ) -> Result<R, Self::Error> {
            self.call(format!("read:{}", retained.id))?;
            Ok(access(&retained.object.borrow()))
        }
        fn with_instance_pair<R>(
            &mut self,
            target: &Lease,
            source: &Lease,
            access: impl FnOnce(InstancePair<'_, u32>, &mut dyn FnMut(&u32, &u32)) -> R,
        ) -> Result<R, Self::Error> {
            self.call(format!("pair:{}:{}", target.id, source.id))?;
            let calls = &mut self.calls;
            let mut warning =
                |left: &u32, right: &u32| calls.push(format!("warning:{left}:{right}"));
            if Rc::ptr_eq(&target.object, &source.object) {
                Ok(access(
                    InstancePair::Same(&mut target.object.borrow_mut()),
                    &mut warning,
                ))
            } else {
                Ok(access(
                    InstancePair::Distinct {
                        target: &mut target.object.borrow_mut(),
                        source: &source.object.borrow(),
                    },
                    &mut warning,
                ))
            }
        }
        fn map_get(&mut self, _: &u64, holder: &u32) -> Result<Option<Lease>, Self::Error> {
            self.call(format!("get:{holder}"))?;
            Ok(self
                .active
                .iter()
                .find(|(key, _)| key == holder)
                .map(|(_, lease)| lease.clone()))
        }
        fn map_put(
            &mut self,
            _: &u64,
            holder: u32,
            incoming: &Lease,
        ) -> Result<Option<Lease>, Self::Error> {
            self.call(format!("put:{holder}:{}", incoming.id))?;
            Ok(self.put(holder, incoming))
        }
        fn map_remove(&mut self, _: &u64, holder: &u32) -> Result<Option<Lease>, Self::Error> {
            self.call(format!("map-remove:{holder}"))?;
            Ok(self.remove(*holder))
        }
        fn on_effect_added(
            &mut self,
            actor: &u64,
            incoming: &Lease,
            cause: Option<&u64>,
        ) -> Result<(), Self::Error> {
            let key = *incoming.object.borrow().effect();
            self.callback(format!(
                "added:{}:{cause:?}:visible{:?}",
                incoming.id,
                self.current(key)
            ))?;
            if let Some(replacement) = self.added_replacement.take() {
                force_add_effect(actor, &replacement, None, self)?;
            }
            Ok(())
        }
        fn on_effect_updated(
            &mut self,
            _: &u64,
            existing: &Lease,
            reapply: bool,
            cause: Option<&u64>,
        ) -> Result<(), Self::Error> {
            self.callback(format!("updated:{}:{reapply}:{cause:?}", existing.id))
        }
        fn on_effect_removed(&mut self, _: &u64, retained: &Lease) -> Result<(), Self::Error> {
            let key = *retained.object.borrow().effect();
            self.callback(format!(
                "removed:{}:visible{:?}",
                retained.id,
                self.current(key)
            ))?;
            if let Some(replacement) = self.removed_replacement.take() {
                self.put(key, &replacement);
            }
            if let Some(addition) = self.removed_addition.take() {
                let key = *addition.object.borrow().effect();
                self.put(key, &addition);
            }
            Ok(())
        }
        fn instance_on_effect_added(
            &mut self,
            _: &u64,
            incoming: &Lease,
        ) -> Result<(), Self::Error> {
            self.callback(format!("instance-added:{}", incoming.id))
        }
        fn instance_on_effect_started(
            &mut self,
            _: &u64,
            incoming: &Lease,
        ) -> Result<(), Self::Error> {
            self.callback(format!("started:{}", incoming.id))
        }
        fn virtual_remove_effect_no_update(
            &mut self,
            actor: &u64,
            holder: &u32,
        ) -> Result<Option<Lease>, Self::Error> {
            self.callback(format!("virtual-remove:{holder}"))?;
            remove_effect_no_update(actor, holder, self)
        }
        fn values_iterator(&mut self, _: &u64) -> Result<Cursor, Self::Error> {
            self.call("iterator")?;
            Ok(Cursor {
                keys: self.active.iter().map(|(key, _)| *key).collect(),
                next: 0,
                last: None,
                expected_mod_count: self.mod_count,
            })
        }
        fn iterator_has_next(&mut self, cursor: &mut Cursor) -> Result<bool, Self::Error> {
            self.call("has-next")?;
            Ok(cursor.next < cursor.keys.len())
        }
        fn iterator_next_retained(&mut self, cursor: &mut Cursor) -> Result<Lease, Self::Error> {
            self.call("next")?;
            if self.mod_count != cursor.expected_mod_count {
                return Err("concurrent modification");
            }
            let key = cursor.keys[cursor.next];
            cursor.next += 1;
            cursor.last = Some(key);
            Ok(self
                .active
                .iter()
                .find(|(holder, _)| *holder == key)
                .unwrap()
                .1
                .clone())
        }
        fn iterator_remove(&mut self, cursor: &mut Cursor) -> Result<(), Self::Error> {
            self.call("iterator-remove")?;
            if self.mod_count != cursor.expected_mod_count {
                return Err("concurrent modification");
            }
            self.remove(cursor.last.take().unwrap()).unwrap();
            cursor.expected_mod_count = self.mod_count;
            Ok(())
        }
    }

    #[test]
    fn add_visibility_reentry_keeps_original_incoming_for_instance_callbacks() {
        let incoming = Lease::new(1, 7, 20, 0);
        let replacement = Lease::new(2, 7, 80, 1);
        let mut host = Host {
            added_replacement: Some(replacement.clone()),
            ..Host::default()
        };
        host.retain(&incoming);
        host.retain(&replacement);
        assert_eq!(add_effect(&50, &incoming, Some(&60), &mut host), Ok(true));
        assert_eq!(host.current(7), Some(2));
        assert_eq!(
            host.calls,
            [
                "can-affect:1",
                "read:1",
                "get:7",
                "read:1",
                "put:7:1",
                "added:1:Some(60):visibleSome(1)",
                "can-affect:2",
                "read:2",
                "put:7:2",
                "pair:2:1",
                "updated:2:true:None",
                "instance-added:1",
                "started:1",
            ]
        );
    }

    #[test]
    fn unchanged_update_can_mutate_hidden_chain_and_starts_incoming_only() {
        let existing = Lease::new(1, 7, 100, 3);
        let incoming = Lease::new(2, 7, 200, 1);
        let mut host = Host::default();
        host.retain(&existing);
        host.retain(&incoming);
        host.put(7, &existing);
        assert_eq!(add_effect(&50, &incoming, None, &mut host), Ok(false));
        assert_eq!(host.current(7), Some(1));
        assert_eq!(
            existing.object.borrow().hidden_effect().unwrap().duration(),
            200
        );
        assert_eq!(
            host.calls,
            ["can-affect:2", "read:2", "get:7", "pair:1:2", "started:2"]
        );
    }

    #[test]
    fn same_object_add_and_force_use_single_borrow_without_hidden_clone() {
        let existing = Lease {
            id: 1,
            object: Rc::new(RefCell::new(EffectInstance::with_hidden(
                7,
                100,
                3,
                false,
                true,
                true,
                Some(Box::new(EffectInstance::new(7, 200, 1))),
            ))),
        };
        existing.object.borrow_mut().skip_blending(0);
        let hidden_address = existing.object.borrow().hidden_effect().unwrap() as *const _ as usize;
        let mut host = Host::default();
        host.retain(&existing);
        host.put(7, &existing);
        assert_eq!(add_effect(&50, &existing, None, &mut host), Ok(false));
        force_add_effect(&50, &existing, Some(&60), &mut host).unwrap();
        assert_eq!(
            existing.object.borrow().hidden_effect().unwrap() as *const _ as usize,
            hidden_address
        );
        assert_eq!(existing.object.borrow_mut().blend_factor(false, 1.0), 1.0);
        assert_eq!(
            host.calls,
            [
                "can-affect:1",
                "read:1",
                "get:7",
                "pair:1:1",
                "started:1",
                "can-affect:1",
                "read:1",
                "put:7:1",
                "pair:1:1",
                "updated:1:true:Some(60)",
            ]
        );
    }

    #[test]
    fn force_copies_previous_blend_after_put_and_skips_instance_start_callbacks() {
        let previous = Lease::new(1, 7, 100, 0);
        previous.object.borrow_mut().skip_blending(0);
        let incoming = Lease::new(2, 7, 30, 2);
        let mut host = Host::default();
        host.retain(&previous);
        host.retain(&incoming);
        host.put(7, &previous);
        force_add_effect(&50, &incoming, None, &mut host).unwrap();
        assert_eq!(host.current(7), Some(2));
        assert_eq!(incoming.object.borrow_mut().blend_factor(false, 1.0), 1.0);
        assert_eq!(
            host.calls,
            [
                "can-affect:2",
                "read:2",
                "put:7:2",
                "pair:2:1",
                "updated:2:true:None"
            ]
        );

        let rejected = Lease::new(3, 8, 10, 0);
        host.calls.clear();
        host.allowed = false;
        assert_eq!(add_effect(&50, &rejected, None, &mut host), Ok(false));
        force_add_effect(&50, &rejected, None, &mut host).unwrap();
        assert_eq!(host.calls, ["can-affect:3", "can-affect:3"]);
    }

    #[test]
    fn remove_single_uses_virtual_dispatch_and_removed_object_after_map_removal() {
        let existing = Lease::new(1, 7, 100, 0);
        let mut host = Host::default();
        host.retain(&existing);
        host.put(7, &existing);
        assert_eq!(remove_effect(&50, &7, &mut host), Ok(true));
        assert_eq!(remove_effect(&50, &7, &mut host), Ok(false));
        assert_eq!(
            host.calls,
            [
                "virtual-remove:7",
                "map-remove:7",
                "removed:1:visibleNone",
                "virtual-remove:7",
                "map-remove:7",
            ]
        );
    }

    #[test]
    fn remove_all_keeps_visible_entry_during_callback_then_removes_replacement() {
        let previous = Lease::new(1, 7, 100, 0);
        let replacement = Lease::new(2, 7, 50, 1);
        let mut host = Host {
            removed_replacement: Some(replacement.clone()),
            ..Host::default()
        };
        host.retain(&previous);
        host.retain(&replacement);
        host.put(7, &previous);
        assert_eq!(remove_all_effects(&50, &mut host), Ok(true));
        assert_eq!(host.current(7), None);
        assert_eq!(
            host.calls,
            [
                "client",
                "iterator",
                "has-next",
                "next",
                "removed:1:visibleSome(1)",
                "iterator-remove",
                "has-next",
            ]
        );
        host.calls.clear();
        assert_eq!(remove_all_effects(&50, &mut host), Ok(false));
        assert_eq!(host.calls, ["client", "iterator", "has-next"]);
        host.calls.clear();
        host.client = true;
        assert_eq!(remove_all_effects(&50, &mut host), Ok(false));
        assert_eq!(host.calls, ["client"]);
    }

    #[test]
    fn remove_all_propagates_structural_modification_after_callback_without_catch() {
        let previous = Lease::new(1, 7, 100, 0);
        let addition = Lease::new(2, 8, 50, 1);
        let mut host = Host {
            removed_addition: Some(addition.clone()),
            ..Host::default()
        };
        host.retain(&previous);
        host.retain(&addition);
        host.put(7, &previous);
        assert_eq!(
            remove_all_effects(&50, &mut host),
            Err("concurrent modification")
        );
        assert_eq!(host.current(7), Some(1));
        assert_eq!(host.current(8), Some(2));
        assert_eq!(host.calls.last().unwrap(), "iterator-remove");
    }

    #[test]
    fn failures_keep_installed_or_updated_instance_without_later_start_or_remove() {
        let incoming = Lease::new(1, 7, 100, 0);
        let mut added = Host {
            fail_at: Some("instance-added:1"),
            ..Host::default()
        };
        added.retain(&incoming);
        assert_eq!(
            add_effect(&50, &incoming, None, &mut added),
            Err("owner failure")
        );
        assert_eq!(added.current(7), Some(1));
        assert_eq!(added.calls.last().unwrap(), "instance-added:1");

        let stronger = Lease::new(2, 7, 200, 1);
        added.retain(&stronger);
        added.calls.clear();
        added.fail_at = Some("updated:1:true:None");
        assert_eq!(
            add_effect(&50, &stronger, None, &mut added),
            Err("owner failure")
        );
        assert_eq!(incoming.object.borrow().amplifier(), 1);
        assert_eq!(incoming.object.borrow().duration(), 200);
        assert_eq!(added.calls.last().unwrap(), "updated:1:true:None");

        added.calls.clear();
        added.fail_at = Some("removed:1:visibleSome(1)");
        assert_eq!(remove_all_effects(&50, &mut added), Err("owner failure"));
        assert_eq!(added.current(7), Some(1));
        assert_eq!(added.calls.last().unwrap(), "removed:1:visibleSome(1)");
    }
}
