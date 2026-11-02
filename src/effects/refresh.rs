//! Java 1.21 LivingEntity attribute refresh and scalar accessor bodies.
//!
//! Actor, AttributeMap, canonical Holder/ResourceKey identities, synchronized
//! health data, virtual absorption and body/dimensions remain actual owners.
//! This module creates no living state, scheduler, metadata mirror or registry.
//! Every source callback is synchronous and runs without a map/instance borrow.

use super::attribute_instance::AttributeDefinition;
use super::attribute_map::{self, AttributeMapError, AttributeMapHost, DirtySet};

/// Retained bindings to the actual static Attributes holders. These are not
/// names/protocol IDs, and constructing this binding must not allocate holders.
pub struct CanonicalAttributeKeys<K> {
    pub max_health: K,
    pub max_absorption: K,
    pub scale: K,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AttributeRefreshError<E> {
    Host(E),
    AttributeMap(AttributeMapError<E>),
    /// Source dereference of an uninitialized attributes field. Only getScale
    /// has a constructor-null fallback; max getters/refresh add no default map.
    NullAttributeMap,
}

/// Access to the real retained actor. All accessors refer to that SAME actor
/// and data receiver through removal/reentry; a reused entity ID cannot retarget
/// an invocation. Every state projection releases its borrow before returning.
/// No callback has an empty/default implementation or creates parallel state.
pub trait LivingAttributeHost: AttributeMapHost {
    /// A retained lease to the actual SynchedEntityData receiver, not a lookup
    /// that can later resolve another actor or a replacement data object.
    type HealthData;

    /// Actual LivingEntity.attributes field/getAttributes. None is possible
    /// during Entity-super construction, before LivingEntity installs its map.
    fn attributes(&mut self, actor: &Self::Actor) -> Result<Option<Self::Map>, Self::Error>;
    /// Actual incoming Holder.is(expected), including Reference ResourceKey
    /// identity and Direct value.equals semantics; not holder/text/ID equality.
    fn holder_is(
        &mut self,
        holder: &Self::Holder,
        expected: &Self::Holder,
    ) -> Result<bool, Self::Error>;
    /// Source virtual getAttributeValue(holder) in final max-health/absorption
    /// getters. Bind native base dispatch to attribute_value below; getScale
    /// intentionally reads its captured map directly instead of this method.
    fn virtual_attribute_value(
        &mut self,
        actor: &Self::Actor,
        holder: &Self::Holder,
    ) -> Result<f64, Self::Error>;
    /// Actual entityData field receiver for DATA_HEALTH_ID (metadata9).
    fn health_data(&mut self, actor: &Self::Actor) -> Result<Self::HealthData, Self::Error>;
    fn read_health_data(&mut self, data: &Self::HealthData) -> Result<f32, Self::Error>;
    /// Actual SynchedEntityData.set(DATA_HEALTH_ID,value), including its native
    /// equality/dirty/synchronous owner callbacks, not a private health scalar.
    fn write_health_data(&mut self, data: &Self::HealthData, value: f32)
        -> Result<(), Self::Error>;

    /// Source virtual getHealth/setHealth callsites in onAttributeUpdated.
    /// Bind the actual dispatch to health/set_health below for the base body.
    fn virtual_health(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn virtual_set_health(&mut self, actor: &Self::Actor, value: f32) -> Result<(), Self::Error>;
    /// Source virtual getAbsorptionAmount. Bat/base uses the actual raw living
    /// float; Player uses its actual synchronized DATA_PLAYER_ABSORPTION_ID15.
    fn absorption_amount(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Source virtual internalSetAbsorptionAmount AFTER the final native clamp.
    /// Base/Bat writes the canonical living float; Player writes actual data15.
    fn internal_set_absorption_amount(
        &mut self,
        actor: &Self::Actor,
        value: f32,
    ) -> Result<(), Self::Error>;
    /// Actual virtual sanitizeScale; LivingEntity/Bat's base body is identity.
    fn sanitize_scale(&mut self, actor: &Self::Actor, value: f32) -> Result<f32, Self::Error>;
}

fn attributes<H: LivingAttributeHost>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<H::Map, AttributeRefreshError<H::Error>> {
    host.attributes(actor)
        .map_err(AttributeRefreshError::Host)?
        .ok_or(AttributeRefreshError::NullAttributeMap)
}

/// LivingEntity.getAttributeValue1977. Query the real map/supplier cache; this
/// read never creates a per-actor AttributeInstance merely to obtain a value.
pub fn attribute_value<H: LivingAttributeHost>(
    actor: &H::Actor,
    holder: &H::Holder,
    host: &mut H,
) -> Result<f64, AttributeRefreshError<H::Error>> {
    let map = attributes(actor, host)?;
    attribute_map::value(&map, holder, host).map_err(AttributeRefreshError::AttributeMap)
}

/// Final getMaxHealth1791 / getMaxAbsorption1795 retain source double->float.
pub fn max_health<H: LivingAttributeHost>(
    actor: &H::Actor,
    keys: &CanonicalAttributeKeys<H::Holder>,
    host: &mut H,
) -> Result<f32, AttributeRefreshError<H::Error>> {
    Ok(host
        .virtual_attribute_value(actor, &keys.max_health)
        .map_err(AttributeRefreshError::Host)? as f32)
}

pub fn max_absorption<H: LivingAttributeHost>(
    actor: &H::Actor,
    keys: &CanonicalAttributeKeys<H::Holder>,
    host: &mut H,
) -> Result<f32, AttributeRefreshError<H::Error>> {
    Ok(host
        .virtual_attribute_value(actor, &keys.max_absorption)
        .map_err(AttributeRefreshError::Host)? as f32)
}

/// LivingEntity.getHealth1120, reading the actual synchronized health owner.
pub fn health<H: LivingAttributeHost>(
    actor: &H::Actor,
    host: &mut H,
) -> Result<f32, AttributeRefreshError<H::Error>> {
    let data = host
        .health_data(actor)
        .map_err(AttributeRefreshError::Host)?;
    host.read_health_data(&data)
        .map_err(AttributeRefreshError::Host)
}

/// LivingEntity.setHealth1124–1125. Capture the actual data receiver BEFORE
/// the current maximum lookup; write/clamp against that fresh maximum, not a
/// maximum previously captured by onAttributeUpdated or a damage routine.
pub fn set_health<H: LivingAttributeHost>(
    actor: &H::Actor,
    value: f32,
    keys: &CanonicalAttributeKeys<H::Holder>,
    host: &mut H,
) -> Result<(), AttributeRefreshError<H::Error>> {
    let data = host
        .health_data(actor)
        .map_err(AttributeRefreshError::Host)?;
    let max = max_health(actor, keys, host)?;
    host.write_health_data(&data, java_clamp(value, 0.0, max))
        .map_err(AttributeRefreshError::Host)
}

/// Final LivingEntity.setAbsorptionAmount3109–3110. Preserve the virtual
/// internal setter and its actual base/Player owner after reading current max.
pub fn set_absorption_amount<H: LivingAttributeHost>(
    actor: &H::Actor,
    value: f32,
    keys: &CanonicalAttributeKeys<H::Holder>,
    host: &mut H,
) -> Result<(), AttributeRefreshError<H::Error>> {
    let max = max_absorption(actor, keys, host)?;
    host.internal_set_absorption_amount(actor, java_clamp(value, 0.0, max))
        .map_err(AttributeRefreshError::Host)
}

/// LivingEntity.getScale554–559. The null field occurs during superclass
/// construction; it returns1 without reading SCALE or calling sanitizeScale.
pub fn scale<H: LivingAttributeHost>(
    actor: &H::Actor,
    keys: &CanonicalAttributeKeys<H::Holder>,
    host: &mut H,
) -> Result<f32, AttributeRefreshError<H::Error>> {
    let Some(map) = host
        .attributes(actor)
        .map_err(AttributeRefreshError::Host)?
    else {
        return Ok(1.0);
    };
    let value = attribute_map::value(&map, &keys.scale, host)
        .map_err(AttributeRefreshError::AttributeMap)? as f32;
    host.sanitize_scale(actor, value)
        .map_err(AttributeRefreshError::Host)
}

/// Private LivingEntity.onAttributeUpdated1099–1110. Source maxima are locals
/// across the current amount getter; the actual setter rereads current max.
/// SCALE and dimension callbacks are deliberately absent from this method.
pub fn on_attribute_updated<H: LivingAttributeHost>(
    actor: &H::Actor,
    holder: &H::Holder,
    keys: &CanonicalAttributeKeys<H::Holder>,
    host: &mut H,
) -> Result<(), AttributeRefreshError<H::Error>> {
    if host
        .holder_is(holder, &keys.max_health)
        .map_err(AttributeRefreshError::Host)?
    {
        let max = max_health(actor, keys, host)?;
        if host
            .virtual_health(actor)
            .map_err(AttributeRefreshError::Host)?
            > max
        {
            host.virtual_set_health(actor, max)
                .map_err(AttributeRefreshError::Host)?;
        }
    } else if host
        .holder_is(holder, &keys.max_absorption)
        .map_err(AttributeRefreshError::Host)?
    {
        let max = max_absorption(actor, keys, host)?;
        if host
            .absorption_amount(actor)
            .map_err(AttributeRefreshError::Host)?
            > max
        {
            set_absorption_amount(actor, max, keys, host)?;
        }
    }
    Ok(())
}

/// Private LivingEntity.refreshDirtyAttributes1091–1096. Capture the actual
/// map/set once, traverse its native live cursor, and invoke the concrete private
/// body above. Clear that SAME set only after complete successful traversal.
pub fn refresh_dirty_attributes<H: LivingAttributeHost>(
    actor: &H::Actor,
    keys: &CanonicalAttributeKeys<H::Holder>,
    host: &mut H,
) -> Result<(), AttributeRefreshError<H::Error>> {
    let map = attributes(actor, host)?;
    let mut cursor = host
        .with_map(&map, |map| map.dirty_cursor(DirtySet::Update))
        .map_err(AttributeRefreshError::Host)?;
    while cursor.has_next() {
        let instance = host
            .with_map(&map, |map| map.next_dirty(&mut cursor))
            .map_err(AttributeRefreshError::Host)?
            .map_err(|error| {
                AttributeRefreshError::AttributeMap(AttributeMapError::DirtyIteration(error))
            })?
            .expect("native cursor has remaining entry");
        let holder = host
            .with_instance(&instance, |instance| instance.attribute().key().clone())
            .map_err(AttributeRefreshError::Host)?;
        on_attribute_updated(actor, &holder, keys, host)?;
    }
    host.with_map(&map, |map| map.clear_dirty(DirtySet::Update))
        .map_err(AttributeRefreshError::Host)
}

/// The actual field/body owners needed only for LivingEntity.tick's separate
/// tail phase. These methods must not substitute a saved scale/body snapshot.
pub trait AttributeScaleTailHost: LivingAttributeHost {
    /// Source virtual getScale call in tick2590; bind native base dispatch to
    /// scale above. This is distinct from getScale's virtual sanitizeScale.
    fn virtual_scale(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn applied_scale(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn write_applied_scale(&mut self, actor: &Self::Actor, value: f32) -> Result<(), Self::Error>;
    /// Actual virtual Entity.refreshDimensions with dimensions/eyeHeight,
    /// reapplyPosition and world collision/fudge-position callbacks preserved.
    fn refresh_dimensions(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
}

/// ONLY LivingEntity.tick2588–2593, for the real tick owner to call at that
/// phase. This is not another tick/scheduler. Effect refresh does not call it.
/// Store the saved scale before dimensions; a failed callback keeps that prefix.
pub fn tick_attribute_tail<H: AttributeScaleTailHost>(
    actor: &H::Actor,
    keys: &CanonicalAttributeKeys<H::Holder>,
    host: &mut H,
) -> Result<(), AttributeRefreshError<H::Error>> {
    refresh_dirty_attributes(actor, keys, host)?;
    let scale = host
        .virtual_scale(actor)
        .map_err(AttributeRefreshError::Host)?;
    if scale
        != host
            .applied_scale(actor)
            .map_err(AttributeRefreshError::Host)?
    {
        host.write_applied_scale(actor, scale)
            .map_err(AttributeRefreshError::Host)?;
        host.refresh_dimensions(actor)
            .map_err(AttributeRefreshError::Host)?;
    }
    Ok(())
}

fn java_clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else {
        java_min(value, max)
    }
}
fn java_min(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        b
    } else if a <= b {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::super::attribute_instance::AttributeInstance;
    use super::super::attribute_map::{get_instance, on_attribute_modified};
    use super::super::attribute_persistence::ResourceLocation;
    use super::super::attribute_transfer::test_support::{
        self, key, Definition, IgnoreDirty, Key, State,
    };
    use super::*;
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    #[derive(Debug, PartialEq, Eq)]
    enum Error {
        Callback(&'static str),
        Native(Box<AttributeRefreshError<Error>>),
    }
    fn native(error: AttributeRefreshError<Error>) -> Error {
        Error::Native(Box::new(error))
    }
    #[derive(Default)]
    struct Data {
        health: f32,
        player_absorption: f32,
    }
    struct Fields {
        attributes: Option<usize>,
        data: Rc<RefCell<Data>>,
        absorption: f32,
        applied_scale: f32,
        player: bool,
    }
    type Actor = Rc<RefCell<Fields>>;
    struct Host {
        native: test_support::Host,
        actors: Vec<Actor>,
        data: Vec<Rc<RefCell<Data>>>,
        active: Actor,
        trace: Vec<String>,
        health_alias: bool,
        mutate_max_on_health_read: Option<f64>,
        mutate_max_on_absorption_read: Option<f64>,
        replace_active_on_health_read: Option<Actor>,
        replace_data_on_attributes: Option<Rc<RefCell<Data>>>,
        replace_data_on_virtual_attribute: Option<Rc<RefCell<Data>>>,
        override_attribute_value: Option<(u32, f64)>,
        dirty_absorption_after_write: bool,
        fail_health_write: bool,
        fail_dimensions: bool,
        opaque_refresh_calls: usize,
        override_scale: Option<f32>,
    }
    impl Host {
        fn new() -> Self {
            let data = Rc::new(RefCell::new(Data {
                health: 8.0,
                player_absorption: 10.0,
            }));
            let actor = Rc::new(RefCell::new(Fields {
                attributes: Some(0),
                data: Rc::clone(&data),
                absorption: 10.0,
                applied_scale: 1.0,
                player: false,
            }));
            Self {
                native: test_support::Host::new(),
                actors: vec![Rc::clone(&actor)],
                data: vec![data],
                active: actor,
                trace: Vec::new(),
                health_alias: false,
                mutate_max_on_health_read: None,
                mutate_max_on_absorption_read: None,
                replace_active_on_health_read: None,
                replace_data_on_attributes: None,
                replace_data_on_virtual_attribute: None,
                override_attribute_value: None,
                dirty_absorption_after_write: false,
                fail_health_write: false,
                fail_dimensions: false,
                opaque_refresh_calls: 0,
                override_scale: None,
            }
        }
        fn keys(&self) -> CanonicalAttributeKeys<Key> {
            CanonicalAttributeKeys {
                max_health: key(1),
                max_absorption: key(2),
                scale: key(3),
            }
        }
        fn record(&mut self, event: impl Into<String>) {
            self.native.assert_no_borrows();
            for actor in &self.actors {
                drop(
                    actor
                        .try_borrow_mut()
                        .expect("actor borrowed across callback"),
                );
            }
            for data in &self.data {
                drop(
                    data.try_borrow_mut()
                        .expect("data borrowed across callback"),
                );
            }
            self.trace.push(event.into());
        }
        fn assert_actor(&self, actor: &Actor) {
            assert!(self.actors.iter().any(|item| Rc::ptr_eq(item, actor)));
        }
        fn set_attribute(&mut self, holder: u32, value: f64) -> Key {
            let instance = get_instance(&0, &key(holder), &mut self.native)
                .unwrap()
                .unwrap();
            let object = self.native.object(&instance);
            object
                .borrow_mut()
                .replace_from_self(&mut IgnoreDirty)
                .unwrap();
            object
                .borrow_mut()
                .set_base_value(value, &mut IgnoreDirty)
                .unwrap();
            on_attribute_modified(&0, &instance, &mut self.native).unwrap();
            instance
        }
        fn clear_dirty(&mut self) {
            self.native.maps[0]
                .borrow_mut()
                .clear_dirty(DirtySet::Update);
        }
        fn prepare(&mut self) -> (Key, Key, Key) {
            let health = self.set_attribute(1, 6.0);
            let absorption = self.set_attribute(2, 4.0);
            let scale = self.set_attribute(3, 1.0);
            self.clear_dirty();
            (health, absorption, scale)
        }
        fn add_actor(&mut self, health: f32) -> Actor {
            let data = Rc::new(RefCell::new(Data {
                health,
                player_absorption: 0.0,
            }));
            let actor = Rc::new(RefCell::new(Fields {
                attributes: Some(1),
                data: Rc::clone(&data),
                absorption: 0.0,
                applied_scale: 1.0,
                player: false,
            }));
            self.data.push(data);
            self.actors.push(Rc::clone(&actor));
            actor
        }
    }
    impl AttributeMapHost for Host {
        type Error = Error;
        type Holder = Key;
        type Instance = Key;
        type Supplier = u32;
        type Map = usize;
        type Actor = Actor;
        type ModifierKey = ResourceLocation;
        type Definition = Definition;
        fn with_map<R>(
            &mut self,
            map: &usize,
            access: impl FnOnce(&mut State) -> R,
        ) -> Result<R, Error> {
            self.record("map");
            self.native.with_map(map, access).map_err(Error::Callback)
        }
        fn with_instance<R>(
            &mut self,
            instance: &Key,
            access: impl FnOnce(&mut AttributeInstance<Definition, ResourceLocation>) -> R,
        ) -> Result<R, Error> {
            self.record("instance");
            self.native
                .with_instance(instance, access)
                .map_err(Error::Callback)
        }
        fn supplier_instance(
            &mut self,
            supplier: &u32,
            holder: &Key,
        ) -> Result<Option<Key>, Error> {
            self.record("supplier");
            self.native
                .supplier_instance(supplier, holder)
                .map_err(Error::Callback)
        }
        fn definition(&mut self, holder: &Key) -> Result<Arc<Definition>, Error> {
            self.record("definition");
            self.native.definition(holder).map_err(Error::Callback)
        }
        fn allocate_instance(
            &mut self,
            instance: AttributeInstance<Definition, ResourceLocation>,
        ) -> Result<Key, Error> {
            self.record("allocate");
            self.native
                .allocate_instance(instance)
                .map_err(Error::Callback)
        }
        fn on_attribute_updated(&mut self, _: &Actor, _: &Key) -> Result<(), Error> {
            self.opaque_refresh_calls += 1;
            Err(Error::Callback("concrete private refresh body required"))
        }
    }
    impl LivingAttributeHost for Host {
        type HealthData = Rc<RefCell<Data>>;
        fn attributes(&mut self, actor: &Actor) -> Result<Option<usize>, Error> {
            self.record("attributes");
            self.assert_actor(actor);
            if let Some(data) = self.replace_data_on_attributes.take() {
                actor.borrow_mut().data = data;
            }
            Ok(actor.borrow().attributes)
        }
        fn holder_is(&mut self, holder: &Key, expected: &Key) -> Result<bool, Error> {
            self.record(format!("is:{}:{}", holder.id, expected.id));
            Ok(holder == expected || (self.health_alias && holder.id == 4 && expected.id == 1))
        }
        fn virtual_attribute_value(&mut self, actor: &Actor, holder: &Key) -> Result<f64, Error> {
            self.record(format!("virtual_attribute:{}", holder.id));
            self.assert_actor(actor);
            if let Some(data) = self.replace_data_on_virtual_attribute.take() {
                actor.borrow_mut().data = data;
            }
            if let Some((id, value)) = self.override_attribute_value {
                if holder.id == id {
                    self.override_attribute_value = None;
                    return Ok(value);
                }
            }
            attribute_value(actor, holder, self).map_err(native)
        }
        fn health_data(&mut self, actor: &Actor) -> Result<Rc<RefCell<Data>>, Error> {
            self.record("health_data");
            self.assert_actor(actor);
            Ok(Rc::clone(&actor.borrow().data))
        }
        fn read_health_data(&mut self, data: &Rc<RefCell<Data>>) -> Result<f32, Error> {
            self.record("read_health");
            if let Some(value) = self.mutate_max_on_health_read.take() {
                self.set_attribute(1, value);
            }
            if let Some(actor) = self.replace_active_on_health_read.take() {
                self.active = actor;
            }
            Ok(data.borrow().health)
        }
        fn write_health_data(&mut self, data: &Rc<RefCell<Data>>, value: f32) -> Result<(), Error> {
            self.record(format!("write_health:{value}"));
            data.borrow_mut().health = value;
            if self.dirty_absorption_after_write {
                self.dirty_absorption_after_write = false;
                self.set_attribute(2, 2.0);
            }
            if self.fail_health_write {
                Err(Error::Callback("health metadata callback failed"))
            } else {
                Ok(())
            }
        }
        fn virtual_health(&mut self, actor: &Actor) -> Result<f32, Error> {
            self.record("virtual_health");
            health(actor, self).map_err(native)
        }
        fn virtual_set_health(&mut self, actor: &Actor, value: f32) -> Result<(), Error> {
            self.record(format!("virtual_set_health:{value}"));
            let keys = self.keys();
            set_health(actor, value, &keys, self).map_err(native)
        }
        fn absorption_amount(&mut self, actor: &Actor) -> Result<f32, Error> {
            self.record("absorption");
            self.assert_actor(actor);
            if let Some(value) = self.mutate_max_on_absorption_read.take() {
                self.set_attribute(2, value);
            }
            let fields = actor.borrow();
            Ok(if fields.player {
                fields.data.borrow().player_absorption
            } else {
                fields.absorption
            })
        }
        fn internal_set_absorption_amount(
            &mut self,
            actor: &Actor,
            value: f32,
        ) -> Result<(), Error> {
            self.record(format!("internal_absorption:{value}"));
            self.assert_actor(actor);
            let mut fields = actor.borrow_mut();
            if fields.player {
                fields.data.borrow_mut().player_absorption = value;
            } else {
                fields.absorption = value;
            }
            Ok(())
        }
        fn sanitize_scale(&mut self, actor: &Actor, value: f32) -> Result<f32, Error> {
            self.record(format!("sanitize:{value}"));
            self.assert_actor(actor);
            Ok(value)
        }
    }
    impl AttributeScaleTailHost for Host {
        fn virtual_scale(&mut self, actor: &Actor) -> Result<f32, Error> {
            self.record("virtual_scale");
            if let Some(value) = self.override_scale.take() {
                return Ok(value);
            }
            let keys = self.keys();
            scale(actor, &keys, self).map_err(native)
        }
        fn applied_scale(&mut self, actor: &Actor) -> Result<f32, Error> {
            self.record("applied_scale");
            self.assert_actor(actor);
            Ok(actor.borrow().applied_scale)
        }
        fn write_applied_scale(&mut self, actor: &Actor, value: f32) -> Result<(), Error> {
            self.record(format!("store_scale:{value}"));
            self.assert_actor(actor);
            actor.borrow_mut().applied_scale = value;
            Ok(())
        }
        fn refresh_dimensions(&mut self, actor: &Actor) -> Result<(), Error> {
            let applied_scale = actor.borrow().applied_scale;
            self.record(format!("dimensions:{applied_scale}"));
            self.assert_actor(actor);
            if self.fail_dimensions {
                Err(Error::Callback("real dimensions callback failed"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn concrete_refresh_keeps_saved_max_fresh_clamp_retained_actor_and_live_terminal_clear() {
        let mut host = Host::new();
        let actor = Rc::clone(&host.active);
        let keys = host.keys();
        let (health_instance, _, _) = host.prepare();
        on_attribute_modified(&0, &health_instance, &mut host.native).unwrap();
        host.mutate_max_on_health_read = Some(3.0);
        host.dirty_absorption_after_write = true;
        let replacement = host.add_actor(99.0);
        host.replace_active_on_health_read = Some(Rc::clone(&replacement));
        refresh_dirty_attributes(&actor, &keys, &mut host).unwrap();
        assert_eq!(actor.borrow().data.borrow().health, 3.0);
        assert_eq!(replacement.borrow().data.borrow().health, 99.0);
        assert!(Rc::ptr_eq(&host.active, &replacement));
        assert!(host
            .trace
            .iter()
            .any(|event| event == "virtual_set_health:6"));
        assert!(host.trace.iter().any(|event| event == "write_health:3"));
        assert!(!host
            .trace
            .iter()
            .any(|event| event == "absorption" || event.starts_with("dimensions:")));
        assert_eq!(actor.borrow().absorption, 10.0);
        assert_eq!(host.native.maps[0].borrow().dirty_len(DirtySet::Update), 0);
        assert_eq!(host.opaque_refresh_calls, 0);
    }

    #[test]
    fn actual_holder_predicate_and_virtual_player_absorption_owner_are_required() {
        let mut host = Host::new();
        let actor = Rc::clone(&host.active);
        let keys = host.keys();
        host.prepare();
        host.health_alias = true;
        on_attribute_updated(&actor, &key(4), &keys, &mut host).unwrap();
        assert_eq!(actor.borrow().data.borrow().health, 6.0);
        assert!(host.trace.iter().any(|event| event == "is:4:1"));
        assert!(!host.trace.iter().any(|event| event == "is:4:2"));
        actor.borrow_mut().player = true;
        host.mutate_max_on_absorption_read = Some(2.0);
        on_attribute_updated(&actor, &key(2), &keys, &mut host).unwrap();
        assert_eq!(actor.borrow().data.borrow().player_absorption, 2.0);
        assert_eq!(actor.borrow().absorption, 10.0);
        assert!(host
            .trace
            .iter()
            .any(|event| event == "internal_absorption:2"));
        actor.borrow().data.borrow_mut().player_absorption = f32::NAN;
        host.trace.clear();
        on_attribute_updated(&actor, &key(2), &keys, &mut host).unwrap();
        assert!(!host
            .trace
            .iter()
            .any(|event| event.starts_with("internal_absorption:")));
    }

    #[test]
    fn health_receiver_is_captured_before_max_lookup_and_clamp_preserves_java_edges() {
        let mut host = Host::new();
        let actor = Rc::clone(&host.active);
        let keys = host.keys();
        host.prepare();
        let retained = Rc::clone(&actor.borrow().data);
        let replacement = Rc::new(RefCell::new(Data {
            health: 99.0,
            player_absorption: 0.0,
        }));
        host.data.push(Rc::clone(&replacement));
        host.replace_data_on_virtual_attribute = Some(Rc::clone(&replacement));
        host.override_attribute_value = Some((1, 6.0));
        set_health(&actor, 4.0, &keys, &mut host).unwrap();
        assert_eq!(retained.borrow().health, 4.0);
        assert_eq!(replacement.borrow().health, 99.0);
        let capture = host
            .trace
            .iter()
            .position(|event| event == "health_data")
            .unwrap();
        let lookup = host
            .trace
            .iter()
            .position(|event| event == "virtual_attribute:1")
            .unwrap();
        assert!(capture < lookup);
        for (request, expected) in [
            (f32::NEG_INFINITY, 0.0_f32),
            (-0.0, -0.0),
            (f32::INFINITY, 6.0),
        ] {
            set_health(&actor, request, &keys, &mut host).unwrap();
            assert_eq!(replacement.borrow().health.to_bits(), expected.to_bits());
        }
        let nan = f32::from_bits(0x7fc0_0017);
        set_health(&actor, nan, &keys, &mut host).unwrap();
        assert_eq!(replacement.borrow().health.to_bits(), nan.to_bits());
        host.set_attribute(1, -0.0);
        set_health(&actor, 0.0, &keys, &mut host).unwrap();
        assert_eq!(replacement.borrow().health.to_bits(), (-0.0_f32).to_bits());
        host.set_attribute(1, -3.0);
        set_health(&actor, -2.0, &keys, &mut host).unwrap();
        assert_eq!(replacement.borrow().health.to_bits(), 0.0_f32.to_bits());
    }

    #[test]
    fn max_getters_use_virtual_attribute_value_but_scale_reads_its_map_directly() {
        let mut host = Host::new();
        let actor = Rc::clone(&host.active);
        let keys = host.keys();
        host.prepare();
        host.override_attribute_value = Some((1, 12.75));
        host.trace.clear();
        assert_eq!(max_health(&actor, &keys, &mut host), Ok(12.75));
        assert_eq!(host.trace, ["virtual_attribute:1"]);
        host.override_attribute_value = Some((2, 3.5));
        assert_eq!(max_absorption(&actor, &keys, &mut host), Ok(3.5));
        host.override_attribute_value = Some((3, 99.0));
        host.trace.clear();
        assert_eq!(scale(&actor, &keys, &mut host), Ok(1.0));
        assert_eq!(host.override_attribute_value, Some((3, 99.0)));
        assert!(!host
            .trace
            .iter()
            .any(|event| event.starts_with("virtual_attribute:")));
    }

    #[test]
    fn failed_metadata_callback_keeps_written_prefix_and_prevents_terminal_clear() {
        let mut host = Host::new();
        let actor = Rc::clone(&host.active);
        let keys = host.keys();
        let (instance, _, _) = host.prepare();
        on_attribute_modified(&0, &instance, &mut host.native).unwrap();
        host.fail_health_write = true;
        assert!(refresh_dirty_attributes(&actor, &keys, &mut host).is_err());
        assert_eq!(actor.borrow().data.borrow().health, 6.0);
        assert!(host.native.maps[0]
            .borrow()
            .dirty_contains(DirtySet::Update, &instance));
        assert!(!host
            .trace
            .iter()
            .any(|event| event.starts_with("sanitize:") || event.starts_with("dimensions:")));
    }

    #[test]
    fn constructor_scale_and_tick_tail_keep_null_guard_phase_and_dimensions_failure_prefix() {
        let mut host = Host::new();
        let actor = Rc::clone(&host.active);
        let keys = host.keys();
        actor.borrow_mut().attributes = None;
        assert_eq!(scale(&actor, &keys, &mut host), Ok(1.0));
        assert_eq!(host.trace, ["attributes"]);
        assert_eq!(
            max_health(&actor, &keys, &mut host),
            Err(AttributeRefreshError::Host(Error::Native(Box::new(
                AttributeRefreshError::NullAttributeMap
            ))))
        );
        actor.borrow_mut().attributes = Some(0);
        assert_eq!(max_health(&actor, &keys, &mut host), Ok(5.0));
        assert_eq!(host.native.allocations, 0);
        host.prepare();
        host.set_attribute(3, 2.0);
        host.trace.clear();
        host.fail_dimensions = true;
        assert_eq!(
            tick_attribute_tail(&actor, &keys, &mut host),
            Err(AttributeRefreshError::Host(Error::Callback(
                "real dimensions callback failed"
            )))
        );
        assert_eq!(actor.borrow().applied_scale, 2.0);
        assert_eq!(host.native.maps[0].borrow().dirty_len(DirtySet::Update), 0);
        assert!(host.trace.ends_with(&[
            "applied_scale".into(),
            "store_scale:2".into(),
            "dimensions:2".into()
        ]));
        host.fail_dimensions = false;
        host.override_scale = Some(3.0);
        host.trace.clear();
        tick_attribute_tail(&actor, &keys, &mut host).unwrap();
        assert_eq!(actor.borrow().applied_scale, 3.0);
        assert!(host.trace.ends_with(&[
            "virtual_scale".into(),
            "applied_scale".into(),
            "store_scale:3".into(),
            "dimensions:3".into()
        ]));
    }
}
