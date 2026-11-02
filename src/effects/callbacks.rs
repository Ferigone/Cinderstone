//! Java 1.21 living/player effect callbacks and synchronized visibility.
//!
//! Sources: LivingEntity 851–870, 930–941, 1049–1088, 3123–3124;
//! ServerPlayer 1471–1500, 1702–1708; MobEffectInstance 248–249, 338–339.
//! Native definitions, attributes, metadata, passenger lists, connections and
//! collection traversal remain actual runtime owners. This creates none of
//! them, and supplies no Main callsite, packet codec or scheduler.

use super::EffectInstance;

/// Immutable source packet-constructor fields, not another effect instance or
/// a packet publication/registry owner. Holder cloning retains its identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectUpdate<K> {
    pub entity_id: i32,
    pub effect: K,
    pub amplifier: i32,
    pub duration: i32,
    pub flags: u8,
}

/// ClientboundUpdateMobEffectPacket constructor 28–48. This callback-free
/// fragment reads the actual retained instance at this packet's callsite.
pub fn effect_update<K: Clone>(
    entity_id: i32,
    instance: &EffectInstance<K>,
    blend: bool,
) -> EffectUpdate<K> {
    let effect = instance.effect().clone();
    let amplifier = instance.amplifier();
    let duration = instance.duration();
    let mut flags = 0;
    if instance.is_ambient() {
        flags |= 1;
    }
    if instance.is_visible() {
        flags |= 2;
    }
    if instance.show_icon() {
        flags |= 4;
    }
    if blend {
        flags |= 8;
    }
    EffectUpdate {
        entity_id,
        effect,
        amplifier,
        duration,
        flags,
    }
}

/// Canonical owners at the original effect callback boundaries. All methods
/// are mandatory. Missing owners return errors; no successful empty definition,
/// passenger list, attribute map or packet send may stand in for them.
///
/// Actor/Entity/Lease retain their original lifetimes after removal or ID reuse.
/// with_instance is a short callback-free access to the selected object, never
/// a current-map lookup or clone. Release all actor/instance/map borrows before
/// each remaining method, permitting ordinary synchronous reentry. Errors keep
/// all completed field/modifier/output effects; callers must not retry atomically.
pub trait LivingEffectCallbacksHost<K> {
    type Error;
    type Actor: ?Sized;
    type Entity;
    type Lease;
    type Definition;
    type AttributeMap;
    type Level;
    type PassengerList;
    type PassengerCursor;
    type Player;
    type Connection;

    fn set_effects_dirty(&mut self, actor: &Self::Actor, value: bool) -> Result<(), Self::Error>;
    fn level(&mut self, actor: &Self::Actor) -> Result<Self::Level, Self::Error>;
    fn is_client_side(&mut self, level: &Self::Level) -> Result<bool, Self::Error>;
    fn with_instance<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(&EffectInstance<K>) -> R,
    ) -> Result<R, Self::Error>;
    /// Actual canonical holder.value; preserve this definition receiver when
    /// later callbacks change attributes or the selected instance's amplifier.
    fn definition(&mut self, holder: &K) -> Result<Self::Definition, Self::Error>;
    fn attributes(&mut self, actor: &Self::Actor) -> Result<Self::AttributeMap, Self::Error>;
    /// Actual definition virtual methods, including source template/map order,
    /// real modifier identities and synchronous native dirty listeners.
    fn add_attribute_modifiers(
        &mut self,
        definition: &Self::Definition,
        attributes: &Self::AttributeMap,
        amplifier: i32,
    ) -> Result<(), Self::Error>;
    fn remove_attribute_modifiers(
        &mut self,
        definition: &Self::Definition,
        attributes: &Self::AttributeMap,
    ) -> Result<(), Self::Error>;
    /// Actual private LivingEntity.refreshDirtyAttributes, using the canonical
    /// live toUpdate set and onAttributeUpdated helper; no queued replacement.
    fn refresh_dirty_attributes(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Preserve virtual dispatch. A selected base body can call the free
    /// send_effect_to_passengers below; an actual override must not be bypassed.
    fn virtual_send_effect_to_passengers(
        &mut self,
        actor: &Self::Actor,
        retained: &Self::Lease,
    ) -> Result<(), Self::Error>;
    /// Retain the actual immutable direct-passenger list once, not current
    /// passengers on each iteration, a recursive rider list or tracking viewers.
    fn passengers(&mut self, actor: &Self::Actor) -> Result<Self::PassengerList, Self::Error>;
    fn passenger_iterator(
        &mut self,
        list: &Self::PassengerList,
    ) -> Result<Self::PassengerCursor, Self::Error>;
    fn passenger_has_next(
        &mut self,
        cursor: &mut Self::PassengerCursor,
    ) -> Result<bool, Self::Error>;
    fn passenger_next(
        &mut self,
        cursor: &mut Self::PassengerCursor,
    ) -> Result<Self::Entity, Self::Error>;
    fn as_server_player(
        &mut self,
        passenger: &Self::Entity,
    ) -> Result<Option<Self::Player>, Self::Error>;
    fn passenger_connection(
        &mut self,
        player: &Self::Player,
    ) -> Result<Self::Connection, Self::Error>;
    fn entity_id(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    /// Construct/encode and send the actual protocol packet on this connection
    /// through its real ordered owner; resolve canonical holder registry data.
    /// EffectUpdate supplies only its source constructor snapshot, not wire proof.
    fn send_effect_update(
        &mut self,
        connection: &Self::Connection,
        packet: EffectUpdate<K>,
    ) -> Result<(), Self::Error>;
    fn send_effect_remove(
        &mut self,
        connection: &Self::Connection,
        entity_id: i32,
        effect: K,
    ) -> Result<(), Self::Error>;
    /// Actual virtual definition dispatch, not a generic base no-op. If the
    /// inspected base onEffectAdded113–114 is selected, read its real optional
    /// sound; when present capture Level before actor X/Y/Z, then sound source,
    /// and playSound(excludedPlayer=null, volume=1, pitch=1). That captured
    /// Level.playSound consumes its threadSafeRandom seed at423–424. Do not
    /// replace this with entity RNG or a successful missing-sound-owner result.
    fn definition_on_effect_added(
        &mut self,
        definition: &Self::Definition,
        actor: &Self::Actor,
        amplifier: i32,
    ) -> Result<(), Self::Error>;
    /// Actual selected definition method. Absorption can compose the concrete
    /// helper below; an inspected inherited base can select its literal body.
    fn definition_on_effect_started(
        &mut self,
        definition: &Self::Definition,
        actor: &Self::Actor,
        amplifier: i32,
    ) -> Result<(), Self::Error>;
}

fn definition<K: Clone, H: LivingEffectCallbacksHost<K> + ?Sized>(
    effect: &H::Lease,
    host: &mut H,
) -> Result<H::Definition, H::Error> {
    let holder = host.with_instance(effect, |instance| instance.effect().clone())?;
    host.definition(&holder)
}

fn passenger_updates<K, H: LivingEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    host: &mut H,
    mut publish: impl FnMut(&mut H, &H::Player) -> Result<(), H::Error>,
) -> Result<(), H::Error> {
    let list = host.passengers(actor)?;
    let mut cursor = host.passenger_iterator(&list)?;
    while host.passenger_has_next(&mut cursor)? {
        let passenger = host.passenger_next(&mut cursor)?;
        if let Some(player) = host.as_server_player(&passenger)? {
            publish(host, &player)?;
        }
    }
    Ok(())
}

/// LivingEntity.sendEffectToPassengers1057–1062. Each passenger packet is
/// constructed after its connection and the actor's then-current ID are read.
pub fn send_effect_to_passengers<K: Clone, H: LivingEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    effect: &H::Lease,
    host: &mut H,
) -> Result<(), H::Error> {
    passenger_updates(actor, host, |host, player| {
        let connection = host.passenger_connection(player)?;
        let id = host.entity_id(actor)?;
        let packet = host.with_instance(effect, |instance| effect_update(id, instance, false))?;
        host.send_effect_update(&connection, packet)
    })
}

/// LivingEntity.onEffectAdded1049–1054. It does not refresh dirty attributes.
pub fn living_on_effect_added<K: Clone, H: LivingEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    effect: &H::Lease,
    _cause: Option<&H::Entity>,
    host: &mut H,
) -> Result<(), H::Error> {
    host.set_effects_dirty(actor, true)?;
    let level = host.level(actor)?;
    if !host.is_client_side(&level)? {
        let definition = definition(effect, host)?;
        let attributes = host.attributes(actor)?;
        let amplifier = host.with_instance(effect, |instance| instance.amplifier())?;
        host.add_attribute_modifiers(&definition, &attributes, amplifier)?;
        host.virtual_send_effect_to_passengers(actor, effect)?;
    }
    Ok(())
}

/// LivingEntity.onEffectUpdated1065–1075. The second logical-side query and
/// attribute/amplifier rereads deliberately follow preceding callbacks.
pub fn living_on_effect_updated<K: Clone, H: LivingEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    effect: &H::Lease,
    reapply: bool,
    _cause: Option<&H::Entity>,
    host: &mut H,
) -> Result<(), H::Error> {
    host.set_effects_dirty(actor, true)?;
    if reapply {
        let level = host.level(actor)?;
        if !host.is_client_side(&level)? {
            let definition = definition(effect, host)?;
            let attributes = host.attributes(actor)?;
            host.remove_attribute_modifiers(&definition, &attributes)?;
            let attributes = host.attributes(actor)?;
            let amplifier = host.with_instance(effect, |instance| instance.amplifier())?;
            host.add_attribute_modifiers(&definition, &attributes, amplifier)?;
            host.refresh_dirty_attributes(actor)?;
        }
    }
    let level = host.level(actor)?;
    if !host.is_client_side(&level)? {
        host.virtual_send_effect_to_passengers(actor, effect)?;
    }
    Ok(())
}

/// LivingEntity.onEffectRemoved1078–1088. Collection removal belongs to the
/// calling source operation; this callback does not remove another map value.
pub fn living_on_effect_removed<K: Clone, H: LivingEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    effect: &H::Lease,
    host: &mut H,
) -> Result<(), H::Error> {
    host.set_effects_dirty(actor, true)?;
    let level = host.level(actor)?;
    if !host.is_client_side(&level)? {
        let definition = definition(effect, host)?;
        let attributes = host.attributes(actor)?;
        host.remove_attribute_modifiers(&definition, &attributes)?;
        host.refresh_dirty_attributes(actor)?;
        passenger_updates(actor, host, |host, player| {
            let connection = host.passenger_connection(player)?;
            let id = host.entity_id(actor)?;
            let holder = host.with_instance(effect, |instance| instance.effect().clone())?;
            host.send_effect_remove(&connection, id, holder)
        })?;
    }
    Ok(())
}

/// MobEffectInstance338–339 and248–249. Definition resolution precedes the
/// amplifier read; neither helper looks up the current map entry again.
pub fn instance_on_effect_added<K: Clone, H: LivingEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    effect: &H::Lease,
    host: &mut H,
) -> Result<(), H::Error> {
    let definition = definition(effect, host)?;
    let amplifier = host.with_instance(effect, |instance| instance.amplifier())?;
    host.definition_on_effect_added(&definition, actor, amplifier)
}
pub fn instance_on_effect_started<K: Clone, H: LivingEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    effect: &H::Lease,
    host: &mut H,
) -> Result<(), H::Error> {
    let definition = definition(effect, host)?;
    let amplifier = host.with_instance(effect, |instance| instance.amplifier())?;
    host.definition_on_effect_started(&definition, actor, amplifier)
}

/// Actual ServerPlayer-specific fields and operations on that same actor.
pub trait ServerPlayerEffectCallbacksHost<K>: LivingEffectCallbacksHost<K> {
    type Position;
    fn actor_connection(&mut self, actor: &Self::Actor) -> Result<Self::Connection, Self::Error>;
    /// Actual instance.is(MobEffects.LEVITATION), preserving Holder.is semantics.
    fn is_levitation(&mut self, effect: &Self::Lease) -> Result<bool, Self::Error>;
    fn actor_tick_count(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_levitation_start_time(
        &mut self,
        actor: &Self::Actor,
        tick: i32,
    ) -> Result<(), Self::Error>;
    fn position(&mut self, actor: &Self::Actor) -> Result<Self::Position, Self::Error>;
    fn set_levitation_start_position(
        &mut self,
        actor: &Self::Actor,
        position: Option<Self::Position>,
    ) -> Result<(), Self::Error>;
    fn effects_changed_criterion(
        &mut self,
        actor: &Self::Actor,
        cause: Option<&Self::Entity>,
    ) -> Result<(), Self::Error>;
}

/// ServerPlayer1471–1480. Player inherits the inspected Living callback, so
/// invokespecial parent selects that body, never this override recursively.
pub fn server_player_on_effect_added<K: Clone, H: ServerPlayerEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    effect: &H::Lease,
    cause: Option<&H::Entity>,
    host: &mut H,
) -> Result<(), H::Error> {
    living_on_effect_added(actor, effect, cause, host)?;
    let connection = host.actor_connection(actor)?;
    let id = host.entity_id(actor)?;
    let packet = host.with_instance(effect, |instance| effect_update(id, instance, true))?;
    host.send_effect_update(&connection, packet)?;
    if host.is_levitation(effect)? {
        let tick = host.actor_tick_count(actor)?;
        host.set_levitation_start_time(actor, tick)?;
        let position = host.position(actor)?;
        host.set_levitation_start_position(actor, Some(position))?;
    }
    host.effects_changed_criterion(actor, cause)
}

pub fn server_player_on_effect_updated<K: Clone, H: ServerPlayerEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    effect: &H::Lease,
    reapply: bool,
    cause: Option<&H::Entity>,
    host: &mut H,
) -> Result<(), H::Error> {
    living_on_effect_updated(actor, effect, reapply, cause, host)?;
    let connection = host.actor_connection(actor)?;
    let id = host.entity_id(actor)?;
    let packet = host.with_instance(effect, |instance| effect_update(id, instance, false))?;
    host.send_effect_update(&connection, packet)?;
    host.effects_changed_criterion(actor, cause)
}

pub fn server_player_on_effect_removed<K: Clone, H: ServerPlayerEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    effect: &H::Lease,
    host: &mut H,
) -> Result<(), H::Error> {
    living_on_effect_removed(actor, effect, host)?;
    let connection = host.actor_connection(actor)?;
    let id = host.entity_id(actor)?;
    let holder = host.with_instance(effect, |instance| instance.effect().clone())?;
    host.send_effect_remove(&connection, id, holder)?;
    if host.is_levitation(effect)? {
        host.set_levitation_start_position(actor, None)?;
    }
    host.effects_changed_criterion(actor, None)
}

/// Actual visibility/metadata providers. A source stream and ordinary values
/// iterator have different traversal/error rules; neither is a sorted snapshot.
pub trait EffectVisibilityHost<K> {
    type Error;
    type Actor: ?Sized;
    type Lease;
    type Definition;
    type Metadata;
    type Particle;
    type ParticleList;
    type Values;
    type ValuesCursor;
    fn effects_are_empty(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn has_invisibility_effect(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn set_invisible(&mut self, actor: &Self::Actor, invisible: bool) -> Result<(), Self::Error>;
    fn virtual_remove_effect_particles(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn is_spectator(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn with_instance<R>(
        &mut self,
        effect: &Self::Lease,
        access: impl FnOnce(&EffectInstance<K>) -> R,
    ) -> Result<R, Self::Error>;
    fn definition(&mut self, holder: &K) -> Result<Self::Definition, Self::Error>;
    fn create_particle_options(
        &mut self,
        definition: &Self::Definition,
        instance: &Self::Lease,
    ) -> Result<Self::Particle, Self::Error>;
    /// Actual activeEffects.values().stream().filter/map/toList provider. Invoke
    /// mapper for each selected retained source value in encounter order, keeping
    /// Java spliterator structural-modification checks/error timing and creating
    /// the real immutable resulting list. None filters an invisible instance.
    /// No owner borrow may cross mapper's actual definition callback. Do not
    /// substitute a cloned map/values list or an invented fail-fast iterator.
    fn collect_effect_particles(
        &mut self,
        actor: &Self::Actor,
        mapper: impl FnMut(&mut Self, &Self::Lease) -> Result<Option<Self::Particle>, Self::Error>,
    ) -> Result<Self::ParticleList, Self::Error>;
    fn entity_data(&mut self, actor: &Self::Actor) -> Result<Self::Metadata, Self::Error>;
    fn empty_particle_list(&mut self) -> Result<Self::ParticleList, Self::Error>;
    /// Actual SynchedEntityData.set metadata10/11, including dirty/update work.
    fn set_effect_particles(
        &mut self,
        data: &Self::Metadata,
        particles: Self::ParticleList,
    ) -> Result<(), Self::Error>;
    fn set_effect_ambient(
        &mut self,
        data: &Self::Metadata,
        ambient: bool,
    ) -> Result<(), Self::Error>;
    fn active_effect_values(&mut self, actor: &Self::Actor) -> Result<Self::Values, Self::Error>;
    fn values_iterator(&mut self, values: &Self::Values)
        -> Result<Self::ValuesCursor, Self::Error>;
    fn values_has_next(&mut self, cursor: &mut Self::ValuesCursor) -> Result<bool, Self::Error>;
    fn values_next_retained(
        &mut self,
        cursor: &mut Self::ValuesCursor,
    ) -> Result<Self::Lease, Self::Error>;
}

/// LivingEntity.areAllEffectsAmbient930–936; invisible effects do not veto.
pub fn are_all_effects_ambient<K, H: EffectVisibilityHost<K> + ?Sized>(
    values: &H::Values,
    host: &mut H,
) -> Result<bool, H::Error> {
    let mut cursor = host.values_iterator(values)?;
    while host.values_has_next(&mut cursor)? {
        let effect = host.values_next_retained(&mut cursor)?;
        if host.with_instance(&effect, |instance| {
            instance.is_visible() && !instance.is_ambient()
        })? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// LivingEntity863–870. Metadata receiver10 is read after stream completion;
/// receiver11 is captured before the fresh current values/ambient traversal.
pub fn update_synchronized_effect_particles<K: Clone, H: EffectVisibilityHost<K> + ?Sized>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    let particles = host.collect_effect_particles(actor, |host, effect| {
        if !host.with_instance(effect, |instance| instance.is_visible())? {
            return Ok(None);
        }
        let holder = host.with_instance(effect, |instance| instance.effect().clone())?;
        let definition = host.definition(&holder)?;
        host.create_particle_options(&definition, effect).map(Some)
    })?;
    let data = host.entity_data(actor)?;
    host.set_effect_particles(&data, particles)?;
    let data = host.entity_data(actor)?;
    let values = host.active_effect_values(actor)?;
    let ambient = are_all_effects_ambient(&values, host)?;
    host.set_effect_ambient(&data, ambient)
}

/// LivingEntity940–941 clears particle metadata10 only, preserving ambient11.
pub fn remove_effect_particles<K, H: EffectVisibilityHost<K> + ?Sized>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    let data = host.entity_data(actor)?;
    let particles = host.empty_particle_list()?;
    host.set_effect_particles(&data, particles)
}

pub fn living_update_invisibility_status<K: Clone, H: EffectVisibilityHost<K> + ?Sized>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    if host.effects_are_empty(actor)? {
        host.virtual_remove_effect_particles(actor)?;
        host.set_invisible(actor, false)
    } else {
        let invisible = host.has_invisibility_effect(actor)?;
        host.set_invisible(actor, invisible)?;
        update_synchronized_effect_particles(actor, host)
    }
}

pub fn server_player_update_invisibility_status<K: Clone, H: EffectVisibilityHost<K> + ?Sized>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    if host.is_spectator(actor)? {
        host.virtual_remove_effect_particles(actor)?;
        host.set_invisible(actor, true)
    } else {
        living_update_invisibility_status(actor, host)
    }
}

/// LivingEntity3123–3124, distinct from the passenger packet helper.
pub fn update_effect_visibility<K, H: LivingEffectCallbacksHost<K> + ?Sized>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    host.set_effects_dirty(actor, true)
}

/// The complete inspected MobEffect.onEffectStarted107 base body. This is not
/// a default for an unavailable definition: only actual base-method selection
/// may call it. Subclasses keep their own native definition dispatch.
pub fn base_definition_on_effect_started<A: ?Sized>(_actor: &A, _amplifier: i32) {}

pub trait AbsorptionEffectStartedHost {
    type Error;
    type Actor: ?Sized;
    fn absorption_amount(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Actual final setAbsorptionAmount with current maximum and its virtual
    /// internal setter, including Player's synchronized absorption metadata.
    fn set_absorption_amount(
        &mut self,
        actor: &Self::Actor,
        amount: f32,
    ) -> Result<(), Self::Error>;
}

/// AbsorptionMobEffect.onEffectStarted22–24. Parent base is literally empty;
/// Java wrapping integer arithmetic precedes float conversion and Math.max.
pub fn absorption_on_effect_started<H: AbsorptionEffectStartedHost + ?Sized>(
    actor: &H::Actor,
    amplifier: i32,
    host: &mut H,
) -> Result<(), H::Error> {
    base_definition_on_effect_started(actor, amplifier);
    let current = host.absorption_amount(actor)?;
    let added = 4_i32.wrapping_mul(1_i32.wrapping_add(amplifier)) as f32;
    let amount = if current.is_nan() {
        current
    } else if current == 0.0 && added == 0.0 && current.is_sign_negative() {
        added
    } else if current >= added {
        current
    } else {
        added
    };
    host.set_absorption_amount(actor, amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::attribute_instance::{
        AttributeDefinition, AttributeInstance, DirtyListener, Modifier, Operation,
        ResourceLocationKey,
    };
    use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::Arc};

    #[derive(Clone)]
    struct Lease(Rc<RefCell<EffectInstance<u32>>>);
    impl Lease {
        fn new(ambient: bool, visible: bool) -> Self {
            Self(Rc::new(RefCell::new(EffectInstance::with_flags(
                7, 30, 0, ambient, visible, true,
            ))))
        }
        fn upgrade(&self, amplifier: i32) {
            self.0
                .borrow_mut()
                .update(&EffectInstance::new(7, 100, amplifier), |_, _| {
                    panic!("unexpected mismatch")
                });
        }
    }
    struct Definition(u32);
    impl AttributeDefinition for Definition {
        type Key = u32;
        fn key(&self) -> &u32 {
            &self.0
        }
        fn default_value(&self) -> f64 {
            10.0
        }
        fn sanitize_value(&self, value: f64) -> f64 {
            value
        }
    }
    #[derive(Clone, PartialEq, Eq)]
    struct Key(u32);
    impl ResourceLocationKey for Key {
        fn java_hash_code(&self) -> i32 {
            self.0 as i32
        }
    }
    type Attribute = AttributeInstance<Definition, Key>;
    struct Listener<'a> {
        calls: &'a mut Vec<String>,
        effect: Lease,
        change_amplifier: Option<i32>,
    }
    impl DirtyListener<Definition, Key> for Listener<'_> {
        type Error = &'static str;
        fn on_dirty(&mut self, instance: &mut Attribute) -> Result<(), Self::Error> {
            // Real native attribute listener receives the same mutable object;
            // it can recompute its cache before permanent insertion completes.
            let value = instance.value();
            self.calls.push(format!(
                "attribute-dirty:{value}:permanent{}",
                instance.permanent_modifiers().len()
            ));
            if let Some(amplifier) = self.change_amplifier.take() {
                self.effect.upgrade(amplifier);
            }
            Ok(())
        }
    }
    struct Host {
        calls: Vec<String>,
        effect: Lease,
        attribute: Rc<RefCell<Attribute>>,
        dirty: bool,
        client: bool,
        id: i32,
        passengers: Rc<Vec<u32>>,
        packets: Vec<(u32, EffectUpdate<u32>)>,
        removes: Vec<u32>,
        fail: Option<&'static str>,
        mutate_first_packet: bool,
        change_on_remove: Option<i32>,
        change_on_definition: Option<i32>,
        reenter_refresh: bool,
        levitation: bool,
        start_time: i32,
        start_position: Option<u32>,
    }
    impl Host {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                effect: Lease::new(false, true),
                attribute: Rc::new(RefCell::new(Attribute::new(Arc::new(Definition(1))))),
                dirty: false,
                client: false,
                id: 42,
                passengers: Rc::new(vec![1, 99, 2]),
                packets: Vec::new(),
                removes: Vec::new(),
                fail: None,
                mutate_first_packet: false,
                change_on_remove: None,
                change_on_definition: None,
                reenter_refresh: false,
                levitation: true,
                start_time: 5,
                start_position: Some(6),
            }
        }
        fn call(&mut self, call: impl Into<String>) -> Result<(), &'static str> {
            let call = call.into();
            assert!(
                self.effect.0.try_borrow_mut().is_ok(),
                "instance borrowed across callback"
            );
            let fail = self.fail == Some(call.as_str());
            self.calls.push(call);
            if fail {
                Err("owner failure")
            } else {
                Ok(())
            }
        }
        fn index(&self, name: &str) -> usize {
            self.calls.iter().position(|call| call == name).unwrap()
        }
    }
    impl LivingEffectCallbacksHost<u32> for Host {
        type Error = &'static str;
        type Actor = u64;
        type Entity = u32;
        type Lease = Lease;
        type Definition = u32;
        type AttributeMap = Rc<RefCell<Attribute>>;
        type Level = u32;
        type PassengerList = Rc<Vec<u32>>;
        type PassengerCursor = (Rc<Vec<u32>>, usize);
        type Player = u32;
        type Connection = u32;
        fn set_effects_dirty(&mut self, _: &u64, value: bool) -> Result<(), Self::Error> {
            self.call(format!("dirty:{value}"))?;
            self.dirty = value;
            Ok(())
        }
        fn level(&mut self, _: &u64) -> Result<u32, Self::Error> {
            self.call("level")?;
            Ok(10)
        }
        fn is_client_side(&mut self, _: &u32) -> Result<bool, Self::Error> {
            self.call("client")?;
            Ok(self.client)
        }
        fn with_instance<R>(
            &mut self,
            effect: &Lease,
            access: impl FnOnce(&EffectInstance<u32>) -> R,
        ) -> Result<R, Self::Error> {
            Ok(access(&effect.0.borrow()))
        }
        fn definition(&mut self, holder: &u32) -> Result<u32, Self::Error> {
            self.call(format!("definition:{holder}"))?;
            if let Some(a) = self.change_on_definition.take() {
                self.effect.upgrade(a);
            }
            Ok(*holder)
        }
        fn attributes(&mut self, _: &u64) -> Result<Self::AttributeMap, Self::Error> {
            self.call("attributes")?;
            Ok(self.attribute.clone())
        }
        fn add_attribute_modifiers(
            &mut self,
            definition: &u32,
            attributes: &Self::AttributeMap,
            amplifier: i32,
        ) -> Result<(), Self::Error> {
            self.call(format!("add-attributes:{definition}:{amplifier}"))?;
            let mut listener = Listener {
                calls: &mut self.calls,
                effect: self.effect.clone(),
                change_amplifier: None,
            };
            let mut attribute = attributes.borrow_mut();
            attribute.remove_modifier_by_id(&Key(1), &mut listener)?;
            attribute
                .add_permanent_modifier(
                    Arc::new(Modifier::new(
                        Key(1),
                        f64::from(amplifier.wrapping_add(1)),
                        Operation::AddValue,
                    )),
                    &mut listener,
                )
                .map_err(|_| "modifier failure")
        }
        fn remove_attribute_modifiers(
            &mut self,
            definition: &u32,
            attributes: &Self::AttributeMap,
        ) -> Result<(), Self::Error> {
            self.call(format!("remove-attributes:{definition}"))?;
            let mut listener = Listener {
                calls: &mut self.calls,
                effect: self.effect.clone(),
                change_amplifier: self.change_on_remove.take(),
            };
            attributes
                .borrow_mut()
                .remove_modifier_by_id(&Key(1), &mut listener)?;
            Ok(())
        }
        fn refresh_dirty_attributes(&mut self, actor: &u64) -> Result<(), Self::Error> {
            self.call("refresh")?;
            if self.reenter_refresh {
                self.reenter_refresh = false;
                let effect = self.effect.clone();
                living_on_effect_added(actor, &effect, None, self)?;
            }
            Ok(())
        }
        fn virtual_send_effect_to_passengers(
            &mut self,
            actor: &u64,
            effect: &Lease,
        ) -> Result<(), Self::Error> {
            self.call("virtual-passengers")?;
            send_effect_to_passengers(actor, effect, self)
        }
        fn passengers(&mut self, _: &u64) -> Result<Self::PassengerList, Self::Error> {
            self.call("passengers")?;
            Ok(self.passengers.clone())
        }
        fn passenger_iterator(
            &mut self,
            list: &Self::PassengerList,
        ) -> Result<Self::PassengerCursor, Self::Error> {
            self.call("passenger-iterator")?;
            Ok((list.clone(), 0))
        }
        fn passenger_has_next(
            &mut self,
            cursor: &mut Self::PassengerCursor,
        ) -> Result<bool, Self::Error> {
            Ok(cursor.1 < cursor.0.len())
        }
        fn passenger_next(
            &mut self,
            cursor: &mut Self::PassengerCursor,
        ) -> Result<u32, Self::Error> {
            let value = cursor.0[cursor.1];
            cursor.1 += 1;
            self.call(format!("passenger:{value}"))?;
            Ok(value)
        }
        fn as_server_player(&mut self, entity: &u32) -> Result<Option<u32>, Self::Error> {
            self.call(format!("player-test:{entity}"))?;
            Ok((*entity != 99).then_some(*entity))
        }
        fn passenger_connection(&mut self, player: &u32) -> Result<u32, Self::Error> {
            self.call(format!("connection:{player}"))?;
            Ok(*player)
        }
        fn entity_id(&mut self, _: &u64) -> Result<i32, Self::Error> {
            self.call(format!("id:{}", self.id))?;
            Ok(self.id)
        }
        fn send_effect_update(
            &mut self,
            connection: &u32,
            packet: EffectUpdate<u32>,
        ) -> Result<(), Self::Error> {
            self.call(format!("update:{connection}"))?;
            self.packets.push((*connection, packet));
            if self.mutate_first_packet && self.packets.len() == 1 {
                self.effect.upgrade(3);
                self.id = 43;
                self.passengers = Rc::new(vec![]);
            }
            Ok(())
        }
        fn send_effect_remove(
            &mut self,
            connection: &u32,
            _: i32,
            effect: u32,
        ) -> Result<(), Self::Error> {
            assert_eq!(effect, 7);
            self.call(format!("remove:{connection}"))?;
            self.removes.push(*connection);
            Ok(())
        }
        fn definition_on_effect_added(
            &mut self,
            definition: &u32,
            _: &u64,
            amp: i32,
        ) -> Result<(), Self::Error> {
            self.call(format!("definition-added:{definition}:{amp}"))
        }
        fn definition_on_effect_started(
            &mut self,
            definition: &u32,
            _: &u64,
            amp: i32,
        ) -> Result<(), Self::Error> {
            self.call(format!("definition-started:{definition}:{amp}"))
        }
    }
    impl ServerPlayerEffectCallbacksHost<u32> for Host {
        type Position = u32;
        fn actor_connection(&mut self, _: &u64) -> Result<u32, Self::Error> {
            self.call("self-connection")?;
            Ok(10)
        }
        fn is_levitation(&mut self, _: &Lease) -> Result<bool, Self::Error> {
            self.call("levitation")?;
            Ok(self.levitation)
        }
        fn actor_tick_count(&mut self, _: &u64) -> Result<i32, Self::Error> {
            self.call("tick")?;
            Ok(77)
        }
        fn set_levitation_start_time(&mut self, _: &u64, tick: i32) -> Result<(), Self::Error> {
            self.call(format!("start-time:{tick}"))?;
            self.start_time = tick;
            Ok(())
        }
        fn position(&mut self, _: &u64) -> Result<u32, Self::Error> {
            self.call("position")?;
            Ok(900)
        }
        fn set_levitation_start_position(
            &mut self,
            _: &u64,
            pos: Option<u32>,
        ) -> Result<(), Self::Error> {
            self.call(format!("start-position:{pos:?}"))?;
            self.start_position = pos;
            Ok(())
        }
        fn effects_changed_criterion(
            &mut self,
            _: &u64,
            cause: Option<&u32>,
        ) -> Result<(), Self::Error> {
            self.call(format!("criterion:{cause:?}"))
        }
    }

    #[test]
    fn passenger_list_is_retained_but_each_packet_rereads_current_effect_and_id_before_self() {
        let mut host = Host::new();
        host.mutate_first_packet = true;
        let effect = host.effect.clone();
        server_player_on_effect_added(&50, &effect, Some(&60), &mut host).unwrap();
        assert_eq!(
            host.packets.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
            [1, 2, 10]
        );
        assert_eq!(
            (
                host.packets[0].1.entity_id,
                host.packets[0].1.amplifier,
                host.packets[0].1.flags
            ),
            (42, 0, 6)
        );
        assert_eq!(
            (
                host.packets[1].1.entity_id,
                host.packets[1].1.amplifier,
                host.packets[1].1.flags
            ),
            (43, 3, 6)
        );
        assert_eq!(
            (
                host.packets[2].1.entity_id,
                host.packets[2].1.amplifier,
                host.packets[2].1.flags
            ),
            (43, 3, 14)
        );
        assert!(!host.calls.iter().any(|call| call == "refresh"));
        assert_eq!(host.attribute.borrow_mut().value(), 11.0);
        assert!(host.index("update:2") < host.index("self-connection"));
        assert!(host.index("update:10") < host.index("start-time:77"));
        assert_eq!(host.calls.last().unwrap(), "criterion:Some(60)");
        assert_eq!((host.start_time, host.start_position), (77, Some(900)));
    }

    #[test]
    fn update_rereads_amplifier_after_native_attribute_removal_and_allows_refresh_reentry() {
        let mut host = Host::new();
        host.passengers = Rc::new(vec![]);
        let effect = host.effect.clone();
        living_on_effect_added(&50, &effect, None, &mut host).unwrap();
        host.calls.clear();
        host.change_on_remove = Some(3);
        host.reenter_refresh = true;
        server_player_on_effect_updated(&50, &effect, true, Some(&60), &mut host).unwrap();
        assert!(host.index("remove-attributes:7") < host.index("add-attributes:7:3"));
        assert_eq!(host.attribute.borrow_mut().value(), 14.0);
        assert_eq!(
            host.calls.iter().filter(|call| *call == "refresh").count(),
            1
        );
        assert_eq!(
            host.calls
                .iter()
                .filter(|call| *call == "attributes")
                .count(),
            3
        );
        assert_eq!(host.packets.last().unwrap().1.flags, 6);
        assert_eq!((host.start_time, host.start_position), (5, Some(6)));
        assert_eq!(host.calls.last().unwrap(), "criterion:Some(60)");
    }

    #[test]
    fn removal_refreshes_before_passengers_self_and_only_clears_levitation_position() {
        let mut host = Host::new();
        let effect = host.effect.clone();
        server_player_on_effect_removed(&50, &effect, &mut host).unwrap();
        assert_eq!(host.removes, [1, 2, 10]);
        assert!(host.index("refresh") < host.index("passengers"));
        assert!(host.index("remove:2") < host.index("self-connection"));
        assert_eq!((host.start_time, host.start_position), (5, None));
        assert_eq!(host.calls.last().unwrap(), "criterion:None");
    }

    #[test]
    fn client_periodic_definition_resolution_and_late_failures_keep_source_prefix() {
        let mut host = Host::new();
        host.client = true;
        let effect = host.effect.clone();
        server_player_on_effect_added(&50, &effect, None, &mut host).unwrap();
        assert_eq!(host.packets.len(), 1);
        assert_eq!(host.packets[0].0, 10);
        assert!(!host.calls.iter().any(|call| call == "attributes"));
        host.client = false;
        host.calls.clear();
        living_on_effect_updated(&50, &effect, false, None, &mut host).unwrap();
        assert_eq!(host.calls.iter().filter(|call| *call == "level").count(), 1);
        assert!(!host
            .calls
            .iter()
            .any(|call| call == "attributes" || call == "refresh"));
        host.calls.clear();
        host.change_on_definition = Some(4);
        instance_on_effect_started(&50, &effect, &mut host).unwrap();
        assert_eq!(host.calls, ["definition:7", "definition-started:7:4"]);
        host.calls.clear();
        host.fail = Some("update:10");
        host.packets.clear();
        assert_eq!(
            server_player_on_effect_added(&50, &effect, None, &mut host),
            Err("owner failure")
        );
        assert_eq!(
            host.packets.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(host.dirty);
        assert_eq!(host.calls.last().unwrap(), "update:10");
    }

    // Explicit stream/iterator schedule fixture, not a Java map/spliterator
    // implementation. Actual EffectInstances and metadata callback sequencing
    // test the supplied native mapper, current-values reread and held receivers.
    struct Visibility {
        calls: Vec<String>,
        schedule: VecDeque<Lease>,
        current: Rc<Vec<Lease>>,
        after_particles: Option<Rc<Vec<Lease>>>,
        particle_data: Vec<u32>,
        ambient_data: bool,
        spectator: bool,
        invisible: bool,
        fail: Option<&'static str>,
    }
    impl Visibility {
        fn new() -> Self {
            Self {
                calls: vec![],
                schedule: VecDeque::new(),
                current: Rc::new(vec![]),
                after_particles: None,
                particle_data: vec![9],
                ambient_data: false,
                spectator: false,
                invisible: false,
                fail: None,
            }
        }
        fn call(&mut self, name: impl Into<String>) -> Result<(), &'static str> {
            let name = name.into();
            let fail = self.fail == Some(name.as_str());
            self.calls.push(name);
            if fail {
                Err("visibility failure")
            } else {
                Ok(())
            }
        }
    }
    impl EffectVisibilityHost<u32> for Visibility {
        type Error = &'static str;
        type Actor = u64;
        type Lease = Lease;
        type Definition = u32;
        type Metadata = u32;
        type Particle = u32;
        type ParticleList = Vec<u32>;
        type Values = Rc<Vec<Lease>>;
        type ValuesCursor = (Rc<Vec<Lease>>, usize);
        fn effects_are_empty(&mut self, _: &u64) -> Result<bool, Self::Error> {
            self.call("map-empty")?;
            Ok(self.current.is_empty())
        }
        fn has_invisibility_effect(&mut self, _: &u64) -> Result<bool, Self::Error> {
            self.call("has-invisibility")?;
            Ok(true)
        }
        fn set_invisible(&mut self, _: &u64, value: bool) -> Result<(), Self::Error> {
            self.call(format!("invisible:{value}"))?;
            self.invisible = value;
            Ok(())
        }
        fn virtual_remove_effect_particles(&mut self, actor: &u64) -> Result<(), Self::Error> {
            self.call("virtual-clear")?;
            remove_effect_particles(actor, self)
        }
        fn is_spectator(&mut self, _: &u64) -> Result<bool, Self::Error> {
            self.call("spectator")?;
            Ok(self.spectator)
        }
        fn with_instance<R>(
            &mut self,
            effect: &Lease,
            access: impl FnOnce(&EffectInstance<u32>) -> R,
        ) -> Result<R, Self::Error> {
            Ok(access(&effect.0.borrow()))
        }
        fn definition(&mut self, holder: &u32) -> Result<u32, Self::Error> {
            self.call(format!("definition:{holder}"))?;
            Ok(*holder)
        }
        fn create_particle_options(
            &mut self,
            definition: &u32,
            effect: &Lease,
        ) -> Result<u32, Self::Error> {
            assert!(effect.0.try_borrow_mut().is_ok());
            self.call(format!("particle:{definition}"))?;
            Ok(*definition)
        }
        fn collect_effect_particles(
            &mut self,
            _: &u64,
            mut mapper: impl FnMut(&mut Self, &Lease) -> Result<Option<u32>, Self::Error>,
        ) -> Result<Vec<u32>, Self::Error> {
            self.call("stream")?;
            let mut values = vec![];
            while let Some(effect) = self.schedule.pop_front() {
                if let Some(particle) = mapper(self, &effect)? {
                    values.push(particle);
                }
            }
            Ok(values)
        }
        fn entity_data(&mut self, _: &u64) -> Result<u32, Self::Error> {
            self.call("data")?;
            Ok(1)
        }
        fn empty_particle_list(&mut self) -> Result<Vec<u32>, Self::Error> {
            self.call("empty-list")?;
            Ok(vec![])
        }
        fn set_effect_particles(
            &mut self,
            _: &u32,
            particles: Vec<u32>,
        ) -> Result<(), Self::Error> {
            self.call("set-particles")?;
            self.particle_data = particles;
            if let Some(current) = self.after_particles.take() {
                self.current = current;
            }
            Ok(())
        }
        fn set_effect_ambient(&mut self, _: &u32, ambient: bool) -> Result<(), Self::Error> {
            self.call(format!("set-ambient:{ambient}"))?;
            self.ambient_data = ambient;
            Ok(())
        }
        fn active_effect_values(&mut self, _: &u64) -> Result<Self::Values, Self::Error> {
            self.call("values")?;
            Ok(self.current.clone())
        }
        fn values_iterator(
            &mut self,
            values: &Self::Values,
        ) -> Result<Self::ValuesCursor, Self::Error> {
            self.call("values-iterator")?;
            Ok((values.clone(), 0))
        }
        fn values_has_next(
            &mut self,
            cursor: &mut Self::ValuesCursor,
        ) -> Result<bool, Self::Error> {
            Ok(cursor.1 < cursor.0.len())
        }
        fn values_next_retained(
            &mut self,
            cursor: &mut Self::ValuesCursor,
        ) -> Result<Lease, Self::Error> {
            self.call("value-next")?;
            let value = cursor.0[cursor.1].clone();
            cursor.1 += 1;
            Ok(value)
        }
    }

    #[test]
    fn visibility_uses_native_filter_and_fresh_values_after_metadata10_before_ambient11() {
        let visible = Lease::new(false, true);
        let invisible = Lease::new(false, false);
        let mut host = Visibility::new();
        host.current = Rc::new(vec![visible.clone()]);
        host.schedule = VecDeque::from([visible, invisible.clone()]);
        host.after_particles = Some(Rc::new(vec![invisible]));
        living_update_invisibility_status(&50, &mut host).unwrap();
        assert_eq!(host.particle_data, [7]);
        assert!(host.ambient_data);
        assert_eq!(
            host.calls,
            [
                "map-empty",
                "has-invisibility",
                "invisible:true",
                "stream",
                "definition:7",
                "particle:7",
                "data",
                "set-particles",
                "data",
                "values",
                "values-iterator",
                "value-next",
                "set-ambient:true"
            ]
        );
        let mut failure = Visibility::new();
        failure.current = Rc::new(vec![Lease::new(false, true)]);
        failure.schedule = VecDeque::from([Lease::new(false, true)]);
        failure.fail = Some("values");
        assert_eq!(
            living_update_invisibility_status(&50, &mut failure),
            Err("visibility failure")
        );
        assert_eq!(failure.particle_data, [7]);
        assert!(!failure.ambient_data);
        assert_eq!(failure.calls.last().unwrap(), "values");
    }

    #[test]
    fn empty_and_spectator_clear_only_particles_without_recomputing_ambient_or_stream() {
        let mut empty = Visibility::new();
        empty.ambient_data = true;
        living_update_invisibility_status(&50, &mut empty).unwrap();
        assert!(empty.particle_data.is_empty());
        assert!(empty.ambient_data);
        assert!(!empty.invisible);
        assert_eq!(
            empty.calls,
            [
                "map-empty",
                "virtual-clear",
                "data",
                "empty-list",
                "set-particles",
                "invisible:false"
            ]
        );
        let mut spectator = Visibility::new();
        spectator.spectator = true;
        spectator.ambient_data = true;
        server_player_update_invisibility_status(&50, &mut spectator).unwrap();
        assert!(spectator.invisible);
        assert!(spectator.ambient_data);
        assert_eq!(
            spectator.calls,
            [
                "spectator",
                "virtual-clear",
                "data",
                "empty-list",
                "set-particles",
                "invisible:true"
            ]
        );
    }

    #[test]
    fn absorption_started_preserves_java_float_max_and_wrapped_integer_reward() {
        struct Absorption {
            current: f32,
            sent: Option<f32>,
        }
        impl AbsorptionEffectStartedHost for Absorption {
            type Error = ();
            type Actor = u64;
            fn absorption_amount(&mut self, _: &u64) -> Result<f32, ()> {
                Ok(self.current)
            }
            fn set_absorption_amount(&mut self, _: &u64, value: f32) -> Result<(), ()> {
                self.sent = Some(value);
                Ok(())
            }
        }
        for (current, amp, expected) in [
            (2_f32, 1, 8_f32),
            (20., 0, 20.),
            (-0., -1, 0.),
            (1., i32::MAX, 1.),
        ] {
            let mut host = Absorption {
                current,
                sent: None,
            };
            absorption_on_effect_started(&50, amp, &mut host).unwrap();
            assert_eq!(host.sent.unwrap().to_bits(), expected.to_bits());
        }
        let mut nan = Absorption {
            current: f32::NAN,
            sent: None,
        };
        absorption_on_effect_started(&50, 0, &mut nan).unwrap();
        assert!(nan.sent.unwrap().is_nan());
    }
}
