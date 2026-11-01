//! Java 1.21 living/player removal bodies with actual-owner callbacks.
//!
//! Sources: LivingEntity (`btn`) remove 700–706 and death effects 709–713;
//! MobEffectInstance (`brz`) onMobRemoved 252–253; Player (`cmx`) remove
//! 1450–1456 and hasContainerOpen 1485. These executors create no actor,
//! removal field, active-effect map, Brain, menu, connection or scheduler.
//! Errors preserve the already executed prefix; removal is not a transaction.

use crate::effects::EffectInstance;

/// The five actual Entity.RemovalReason values. This does not synthesize a
/// removed/dead flag or choose a reason from health, connection or chunk state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalReason {
    Killed,
    Discarded,
    UnloadedToChunk,
    UnloadedWithPlayer,
    ChangedDimension,
}

/// Actual living actor operations at the original callsites. Actor must retain
/// this exact lifetime through callbacks; a numeric entity-ID lookup must not
/// retarget the call to a later actor reusing that ID. No default is provided.
pub trait LivingRemovalHost {
    type Error;
    type Actor: ?Sized;

    /// Virtual triggerOnDeathMobEffects(reason), including the actual effect
    /// collection iterator and instance/definition callbacks. A base-body
    /// implementation can call trigger_on_death_effects below.
    fn trigger_on_death_mob_effects(
        &mut self,
        actor: &Self::Actor,
        reason: RemovalReason,
    ) -> Result<(), Self::Error>;

    /// The actual invokespecial Entity.remove(reason), not a registry deletion,
    /// connection disconnect or a successful empty parent callback. Its real
    /// setRemoved owner installs the first reason only if absent; then consults
    /// the stored reason's shouldDestroy for stopRiding, stops all passengers,
    /// and invokes the actual in-level removal callback with the passed reason.
    /// Already-removed actors do not justify skipping that source callback body.
    fn entity_remove(
        &mut self,
        actor: &Self::Actor,
        reason: RemovalReason,
    ) -> Result<(), Self::Error>;

    /// Read this actor's actual current Brain after parent removal, then execute
    /// clearMemories. Missing Brain ownership is an error, not proof of emptiness.
    fn clear_brain_memories(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
}

/// LivingEntity.remove, source 700–706. The death-effect dispatch is gated only
/// by the passed reason; no extra removed, logical-side or health gate is added.
pub fn remove_living<H: LivingRemovalHost + ?Sized>(
    actor: &H::Actor,
    reason: RemovalReason,
    host: &mut H,
) -> Result<(), H::Error> {
    if matches!(reason, RemovalReason::Killed | RemovalReason::Discarded) {
        host.trigger_on_death_mob_effects(actor, reason)?;
    }
    host.entity_remove(actor, reason)?;
    host.clear_brain_memories(actor)
}

/// The real active-effects owner and its source iterator. The iterator is a
/// live values iterator, not a snapshot, sorted-ID plan or drained staging map.
/// All accesses release collection/instance borrows before actor callbacks.
///
/// The pinned HashMap iterator's hasNext tests its prefetched next node without
/// checking modCount. next checks modCount, then reads that node's current
/// value and prefetches the following node. Preserve those distinct boundaries:
/// structural changes can fail at the next next call, while a change after the
/// final prefetched value can still reach clear. This body catches no concurrent
/// modification error. Lease must retain the selected canonical instance even
/// if synchronous callbacks replace/remove the map entry for the same holder.
pub trait DeathEffectsHost {
    type Error;
    type Actor: ?Sized;
    type Cursor;
    type Lease;

    fn active_effects_iterator(&mut self, actor: &Self::Actor)
        -> Result<Self::Cursor, Self::Error>;
    fn iterator_has_next(&mut self, cursor: &mut Self::Cursor) -> Result<bool, Self::Error>;
    fn iterator_next_retained(
        &mut self,
        cursor: &mut Self::Cursor,
    ) -> Result<Self::Lease, Self::Error>;
    /// Actual virtual instance.onMobRemoved(actor, reason), on the retained
    /// object. Its base body can call notify_effect_instance_removed below;
    /// an actual override must not be silently replaced with that base body.
    fn instance_on_mob_removed(
        &mut self,
        actor: &Self::Actor,
        retained: &Self::Lease,
        reason: RemovalReason,
    ) -> Result<(), Self::Error>;
    /// Clear this actor's current activeEffects map only. This is not
    /// removeAllEffects and must not invent onEffectRemoved/attribute callbacks.
    fn clear_active_effects(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
}

/// LivingEntity.triggerOnDeathMobEffects, source 709–713. It has no reason gate
/// itself; the ordinary remove_living caller supplies its source reason gate.
pub fn trigger_on_death_effects<H: DeathEffectsHost + ?Sized>(
    actor: &H::Actor,
    reason: RemovalReason,
    host: &mut H,
) -> Result<(), H::Error> {
    let mut cursor = host.active_effects_iterator(actor)?;
    while host.iterator_has_next(&mut cursor)? {
        let retained = host.iterator_next_retained(&mut cursor)?;
        host.instance_on_mob_removed(actor, &retained, reason)?;
    }
    host.clear_active_effects(actor)
}

/// Access for the actual MobEffectInstance base callback. K::clone retains the
/// same canonical effect holder, and Definition retains its real resolved
/// value. with_instance invokes a callback-free read on the selected canonical
/// object; it must not detach it, snapshot it or resolve it again by current map
/// key. No instance/actor borrow may remain across definition callbacks.
pub trait DeathEffectInstanceHost<K> {
    type Error;
    type Actor: ?Sized;
    type Lease;
    type Definition;

    fn with_instance<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(&EffectInstance<K>) -> R,
    ) -> Result<R, Self::Error>;
    fn effect_definition(&mut self, holder: &K) -> Result<Self::Definition, Self::Error>;
    /// The actual definition.onMobRemoved(actor, amplifier, reason), including
    /// any source world queries, RNG, spawning and entity-owner callbacks.
    fn definition_on_mob_removed(
        &mut self,
        definition: &Self::Definition,
        actor: &Self::Actor,
        amplifier: i32,
        reason: RemovalReason,
    ) -> Result<(), Self::Error>;
}

/// MobEffectInstance.onMobRemoved, source 252–253. Resolve the holder's value
/// before rereading the retained instance's amplifier at its original callsite.
pub fn notify_effect_instance_removed<K: Clone, H: DeathEffectInstanceHost<K> + ?Sized>(
    actor: &H::Actor,
    retained: &H::Lease,
    reason: RemovalReason,
    host: &mut H,
) -> Result<(), H::Error> {
    let holder = host.with_instance(retained, |instance| instance.effect().clone())?;
    let definition = host.effect_definition(&holder)?;
    let amplifier = host.with_instance(retained, |instance| instance.amplifier())?;
    host.definition_on_mob_removed(&definition, actor, amplifier, reason)
}

/// Actual Player removal/menu owners. Every field query is performed after the
/// preceding callback at its source callsite; a cached menu/window snapshot or
/// an opener counter cannot substitute for object identity and virtual calls.
pub trait PlayerRemovalHost {
    type Error;
    type Actor: ?Sized;
    type InventoryMenu;

    /// Actual invokespecial LivingEntity.remove(reason), retaining its source
    /// trigger/parent/Brain behavior, not recursive virtual Player.remove.
    fn living_remove(
        &mut self,
        actor: &Self::Actor,
        reason: RemovalReason,
    ) -> Result<(), Self::Error>;
    fn inventory_menu(&mut self, actor: &Self::Actor) -> Result<Self::InventoryMenu, Self::Error>;
    fn inventory_menu_removed(
        &mut self,
        menu: &Self::InventoryMenu,
        actor: &Self::Actor,
    ) -> Result<(), Self::Error>;
    /// Fresh null test of the current containerMenu field, after removed above.
    fn container_menu_is_present(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual virtual hasContainerOpen. Player's base method compares the
    /// current containerMenu and inventoryMenu object identities, not wire IDs.
    fn has_container_open(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual virtual doCloseContainer with its real menu/packet/item owners.
    /// This does not disconnect the connection or run LOGIN/CONFIGURATION.
    fn do_close_container(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
}

/// Player.remove, source 1450–1456. Inventory removal precedes all reads used
/// by the external-menu close decision, including after synchronous reentry.
pub fn remove_player<H: PlayerRemovalHost + ?Sized>(
    actor: &H::Actor,
    reason: RemovalReason,
    host: &mut H,
) -> Result<(), H::Error> {
    host.living_remove(actor, reason)?;
    let inventory_menu = host.inventory_menu(actor)?;
    host.inventory_menu_removed(&inventory_menu, actor)?;
    if host.container_menu_is_present(actor)? && host.has_container_open(actor)? {
        host.do_close_container(actor)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone)]
    struct Actor(Rc<()>);
    #[derive(Clone)]
    struct Holder(Rc<u32>);
    impl PartialEq for Holder {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }
    type Lease = Rc<RefCell<EffectInstance<Holder>>>;
    struct Node {
        value: Lease,
        next: Option<usize>,
    }
    struct Cursor {
        next: Option<usize>,
        expected_mod_count: u32,
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Mutation {
        None,
        AppendDuringCallback,
        ReplaceDuringLookup,
        NestedOnce,
    }
    struct Host {
        actor: Actor,
        // Prescribed encounter nodes for a controlled live-iterator host, not
        // a native HashMap implementation or production collection-order proof.
        nodes: Vec<Node>,
        first: Option<usize>,
        mod_count: u32,
        calls: Vec<String>,
        mutation: Mutation,
        mutated: bool,
        fail_at: Option<&'static str>,
        inventory_menu: u64,
        container_menu: Option<u64>,
        menu_reentry: bool,
        parent_calls: usize,
    }
    impl Host {
        fn new(amplifiers: &[i32]) -> Self {
            let mut nodes: Vec<Node> = Vec::new();
            for (index, amplifier) in amplifiers.iter().enumerate() {
                let holder = Holder(Rc::new(80 + index as u32));
                nodes.push(Node {
                    value: Rc::new(RefCell::new(EffectInstance::new(holder, 100, *amplifier))),
                    next: if index + 1 < amplifiers.len() {
                        Some(index + 1)
                    } else {
                        None
                    },
                });
            }
            Self {
                actor: Actor(Rc::new(())),
                first: (!nodes.is_empty()).then_some(0),
                nodes,
                mod_count: 0,
                calls: Vec::new(),
                mutation: Mutation::None,
                mutated: false,
                fail_at: None,
                inventory_menu: 10,
                container_menu: Some(20),
                menu_reentry: false,
                parent_calls: 0,
            }
        }
        fn check_actor(&self, actor: &Actor) {
            assert!(Rc::ptr_eq(&self.actor.0, &actor.0));
        }
        fn call(&mut self, value: impl Into<String>) -> Result<(), &'static str> {
            let value = value.into();
            let fail = self.fail_at.is_some_and(|name| value == name);
            self.calls.push(value);
            if fail {
                Err("owner failure")
            } else {
                Ok(())
            }
        }
        fn append(&mut self) {
            let index = self.nodes.len();
            if let Some(previous) = self.nodes.last_mut() {
                previous.next = Some(index);
            }
            self.nodes.push(Node {
                value: Rc::new(RefCell::new(EffectInstance::new(
                    Holder(Rc::new(99)),
                    100,
                    9,
                ))),
                next: None,
            });
            self.mod_count = self.mod_count.wrapping_add(1);
            if self.first.is_none() {
                self.first = Some(index);
            }
        }
        fn assert_unborrowed(&self) {
            for node in &self.nodes {
                drop(
                    node.value
                        .try_borrow_mut()
                        .expect("effect borrowed across callback"),
                );
            }
        }
    }
    impl LivingRemovalHost for Host {
        type Error = &'static str;
        type Actor = Actor;
        fn trigger_on_death_mob_effects(
            &mut self,
            actor: &Actor,
            reason: RemovalReason,
        ) -> Result<(), Self::Error> {
            self.check_actor(actor);
            self.call(format!("trigger:{reason:?}"))?;
            trigger_on_death_effects(actor, reason, self)
        }
        fn entity_remove(
            &mut self,
            actor: &Actor,
            reason: RemovalReason,
        ) -> Result<(), Self::Error> {
            self.check_actor(actor);
            self.parent_calls += 1;
            self.call(format!("entity_remove:{reason:?}"))
        }
        fn clear_brain_memories(&mut self, actor: &Actor) -> Result<(), Self::Error> {
            self.check_actor(actor);
            self.call("brain_clear")
        }
    }
    impl DeathEffectsHost for Host {
        type Error = &'static str;
        type Actor = Actor;
        type Cursor = Cursor;
        type Lease = Lease;
        fn active_effects_iterator(&mut self, actor: &Actor) -> Result<Cursor, Self::Error> {
            self.check_actor(actor);
            self.call("iterator")?;
            Ok(Cursor {
                next: self.first,
                expected_mod_count: self.mod_count,
            })
        }
        fn iterator_has_next(&mut self, cursor: &mut Cursor) -> Result<bool, Self::Error> {
            self.call("has_next")?;
            Ok(cursor.next.is_some())
        }
        fn iterator_next_retained(&mut self, cursor: &mut Cursor) -> Result<Lease, Self::Error> {
            self.call("next")?;
            if cursor.expected_mod_count != self.mod_count {
                return Err("concurrent modification");
            }
            let node = &self.nodes[cursor.next.ok_or("no next node")?];
            cursor.next = node.next;
            Ok(Rc::clone(&node.value))
        }
        fn instance_on_mob_removed(
            &mut self,
            actor: &Actor,
            retained: &Lease,
            reason: RemovalReason,
        ) -> Result<(), Self::Error> {
            self.check_actor(actor);
            self.assert_unborrowed();
            self.call("instance_dispatch")?;
            notify_effect_instance_removed(actor, retained, reason, self)
        }
        fn clear_active_effects(&mut self, actor: &Actor) -> Result<(), Self::Error> {
            self.check_actor(actor);
            self.call("map_clear")?;
            self.first = None;
            self.mod_count = self.mod_count.wrapping_add(1);
            Ok(())
        }
    }
    impl DeathEffectInstanceHost<Holder> for Host {
        type Error = &'static str;
        type Actor = Actor;
        type Lease = Lease;
        type Definition = Rc<u32>;
        fn with_instance<R>(
            &mut self,
            retained: &Lease,
            access: impl FnOnce(&EffectInstance<Holder>) -> R,
        ) -> Result<R, Self::Error> {
            self.call("instance_read")?;
            Ok(access(&retained.borrow()))
        }
        fn effect_definition(&mut self, holder: &Holder) -> Result<Rc<u32>, Self::Error> {
            self.assert_unborrowed();
            self.call("definition_lookup")?;
            if self.mutation == Mutation::ReplaceDuringLookup && !self.mutated {
                self.mutated = true;
                // Change the selected canonical object, then replace its current
                // map entry without a structural change. The caller's old lease
                // must still read amplifier 5, not replacement amplifier 9.
                let incoming = EffectInstance::new(holder.clone(), 200, 5);
                self.nodes[0]
                    .value
                    .borrow_mut()
                    .update(&incoming, |_, _| panic!("holder changed"));
                self.nodes[0].value =
                    Rc::new(RefCell::new(EffectInstance::new(holder.clone(), 300, 9)));
            }
            Ok(Rc::clone(&holder.0))
        }
        fn definition_on_mob_removed(
            &mut self,
            definition: &Rc<u32>,
            actor: &Actor,
            amplifier: i32,
            reason: RemovalReason,
        ) -> Result<(), Self::Error> {
            self.check_actor(actor);
            self.assert_unborrowed();
            self.call(format!("definition:{definition}:{amplifier}:{reason:?}"))?;
            if !self.mutated {
                self.mutated = true;
                match self.mutation {
                    Mutation::AppendDuringCallback => self.append(),
                    Mutation::NestedOnce => remove_living(actor, reason, self)?,
                    _ => {}
                }
            }
            Ok(())
        }
    }
    impl PlayerRemovalHost for Host {
        type Error = &'static str;
        type Actor = Actor;
        type InventoryMenu = u64;
        fn living_remove(
            &mut self,
            actor: &Actor,
            reason: RemovalReason,
        ) -> Result<(), Self::Error> {
            self.call("living_parent")?;
            remove_living(actor, reason, self)
        }
        fn inventory_menu(&mut self, actor: &Actor) -> Result<u64, Self::Error> {
            self.check_actor(actor);
            self.call("inventory_menu")?;
            Ok(self.inventory_menu)
        }
        fn inventory_menu_removed(&mut self, menu: &u64, actor: &Actor) -> Result<(), Self::Error> {
            self.check_actor(actor);
            assert_eq!(*menu, self.inventory_menu);
            self.call(format!("inventory_removed:{menu}"))?;
            if self.menu_reentry {
                self.container_menu = Some(self.inventory_menu);
            }
            Ok(())
        }
        fn container_menu_is_present(&mut self, actor: &Actor) -> Result<bool, Self::Error> {
            self.check_actor(actor);
            self.call("container_present")?;
            Ok(self.container_menu.is_some())
        }
        fn has_container_open(&mut self, actor: &Actor) -> Result<bool, Self::Error> {
            self.check_actor(actor);
            self.call("has_container_open")?;
            Ok(self.container_menu != Some(self.inventory_menu))
        }
        fn do_close_container(&mut self, actor: &Actor) -> Result<(), Self::Error> {
            self.check_actor(actor);
            self.call("do_close_container")
        }
    }

    #[test]
    fn source_reason_gates_effects_and_clears_after_callbacks_before_parent_and_brain() {
        let mut host = Host::new(&[1]);
        let actor = host.actor.clone();
        remove_living(&actor, RemovalReason::Killed, &mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                "trigger:Killed",
                "iterator",
                "has_next",
                "next",
                "instance_dispatch",
                "instance_read",
                "definition_lookup",
                "instance_read",
                "definition:80:1:Killed",
                "has_next",
                "map_clear",
                "entity_remove:Killed",
                "brain_clear"
            ]
        );
        assert!(host.first.is_none());
        for reason in [
            RemovalReason::UnloadedToChunk,
            RemovalReason::UnloadedWithPlayer,
            RemovalReason::ChangedDimension,
        ] {
            let mut host = Host::new(&[1]);
            let actor = host.actor.clone();
            remove_living(&actor, reason, &mut host).unwrap();
            assert_eq!(
                host.calls,
                [format!("entity_remove:{reason:?}"), "brain_clear".into()]
            );
            assert!(host.first.is_some());
        }
        let mut discarded = Host::new(&[]);
        let actor = discarded.actor.clone();
        remove_living(&actor, RemovalReason::Discarded, &mut discarded).unwrap();
        assert_eq!(
            discarded.calls,
            [
                "trigger:Discarded",
                "iterator",
                "has_next",
                "map_clear",
                "entity_remove:Discarded",
                "brain_clear"
            ]
        );
    }

    #[test]
    fn live_iterator_failure_and_last_entry_mutation_have_different_prefixes() {
        let mut host = Host::new(&[1, 2]);
        host.mutation = Mutation::AppendDuringCallback;
        let actor = host.actor.clone();
        assert_eq!(
            remove_living(&actor, RemovalReason::Killed, &mut host),
            Err("concurrent modification")
        );
        assert_eq!(&host.calls[host.calls.len() - 2..], ["has_next", "next"]);
        assert_eq!(host.nodes.len(), 3);
        assert!(host.first.is_some());
        assert_eq!(host.parent_calls, 0);
        assert!(!host.calls.iter().any(|call| call == "map_clear"));
        let mut last = Host::new(&[1]);
        last.mutation = Mutation::AppendDuringCallback;
        let actor = last.actor.clone();
        remove_living(&actor, RemovalReason::Killed, &mut last).unwrap();
        assert_eq!(last.nodes.len(), 2);
        assert!(last.first.is_none());
        assert!(last.calls.ends_with(&[
            "has_next".into(),
            "map_clear".into(),
            "entity_remove:Killed".into(),
            "brain_clear".into()
        ]));
        assert!(!last
            .calls
            .iter()
            .any(|call| call.starts_with("definition:99")));
    }

    #[test]
    fn canonical_instance_survives_replacement_and_rereads_amplifier_after_definition_lookup() {
        let mut host = Host::new(&[1]);
        host.mutation = Mutation::ReplaceDuringLookup;
        let original = Rc::clone(&host.nodes[0].value);
        let actor = host.actor.clone();
        trigger_on_death_effects(&actor, RemovalReason::Killed, &mut host).unwrap();
        assert!(host
            .calls
            .iter()
            .any(|call| call == "definition:80:5:Killed"));
        assert_eq!(original.borrow().amplifier(), 5);
        assert_eq!(host.nodes[0].value.borrow().amplifier(), 9);
        assert!(!Rc::ptr_eq(&original, &host.nodes[0].value));
    }

    #[test]
    fn synchronous_reentry_has_full_context_and_callback_errors_do_not_run_later_owners() {
        let mut host = Host::new(&[1]);
        host.mutation = Mutation::NestedOnce;
        let actor = host.actor.clone();
        remove_living(&actor, RemovalReason::Killed, &mut host).unwrap();
        assert_eq!(host.parent_calls, 2);
        assert_eq!(
            host.calls
                .iter()
                .filter(|call| *call == "map_clear")
                .count(),
            2
        );
        let mut failed = Host::new(&[1]);
        failed.fail_at = Some("definition:80:1:Killed");
        let actor = failed.actor.clone();
        assert_eq!(
            remove_living(&actor, RemovalReason::Killed, &mut failed),
            Err("owner failure")
        );
        assert!(failed.first.is_some());
        assert_eq!(failed.parent_calls, 0);
        assert_eq!(failed.calls.last().unwrap(), "definition:80:1:Killed");
        let mut parent = Host::new(&[1]);
        parent.fail_at = Some("entity_remove:Killed");
        let actor = parent.actor.clone();
        assert_eq!(
            remove_living(&actor, RemovalReason::Killed, &mut parent),
            Err("owner failure")
        );
        assert!(parent.first.is_none());
        assert_eq!(parent.parent_calls, 1);
        assert!(!parent.calls.iter().any(|call| call == "brain_clear"));
    }

    #[test]
    fn player_menu_decision_reads_current_fields_after_inventory_callback() {
        let mut host = Host::new(&[]);
        host.menu_reentry = true;
        let actor = host.actor.clone();
        remove_player(&actor, RemovalReason::ChangedDimension, &mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                "living_parent",
                "entity_remove:ChangedDimension",
                "brain_clear",
                "inventory_menu",
                "inventory_removed:10",
                "container_present",
                "has_container_open"
            ]
        );
        let mut external = Host::new(&[]);
        let actor = external.actor.clone();
        remove_player(&actor, RemovalReason::ChangedDimension, &mut external).unwrap();
        assert_eq!(external.calls.last().unwrap(), "do_close_container");
        let mut no_container = Host::new(&[]);
        no_container.container_menu = None;
        let actor = no_container.actor.clone();
        remove_player(&actor, RemovalReason::ChangedDimension, &mut no_container).unwrap();
        assert_eq!(no_container.calls.last().unwrap(), "container_present");
        assert!(!no_container
            .calls
            .iter()
            .any(|call| call == "has_container_open"));
    }
}
