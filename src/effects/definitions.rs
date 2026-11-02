//! Java 1.21 Attribute/RangedAttribute value objects and registered bindings.
//!
//! One shared AttributeValue owns its source mutable sync/sentiment fields.
//! Registry/Holder/ResourceKey identity, Java identity hashes, network IDs and
//! registration/freeze phases remain actual external owners. No registry is
//! constructed here, and cloning an Arc never copies a source value object.

use super::attribute_instance::AttributeDefinition;
use super::attribute_map::MapAttributeDefinition;
use crate::nbt::JavaString;
use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Sentiment {
    Positive = 0,
    Neutral = 1,
    Negative = 2,
}

/// The three actual ChatFormatting results used by Attribute.Sentiment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeStyle {
    Blue,
    Gray,
    Red,
}
impl Sentiment {
    /// Source Sentiment.getStyle64–67; the argument is the source positive flag.
    pub fn style(self, positive: bool) -> AttributeStyle {
        match self {
            Self::Positive if positive => AttributeStyle::Blue,
            Self::Positive => AttributeStyle::Red,
            Self::Neutral => AttributeStyle::Gray,
            Self::Negative if positive => AttributeStyle::Red,
            Self::Negative => AttributeStyle::Blue,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NullSentiment;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeError {
    MinimumAboveMaximum,
    DefaultBelowMinimum,
    DefaultAboveMaximum,
}
impl RangeError {
    /// Original IllegalArgumentException constructor messages, in source check order.
    pub fn source_message(self) -> &'static str {
        match self {
            Self::MinimumAboveMaximum => "Minimum value cannot be bigger than maximum value!",
            Self::DefaultBelowMinimum => "Default value cannot be lower than minimum value!",
            Self::DefaultAboveMaximum => "Default value cannot be bigger than maximum value!",
        }
    }
}
impl fmt::Display for RangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.source_message())
    }
}
impl std::error::Error for RangeError {}

#[derive(Clone, Copy, Debug)]
pub struct RangedBounds {
    min_value: f64,
    max_value: f64,
}
impl RangedBounds {
    pub fn min_value(&self) -> f64 {
        self.min_value
    }
    pub fn max_value(&self) -> f64 {
        self.max_value
    }
}

/// The actual source value, initially unregistered. Base Attribute and its
/// RangedAttribute subclass share these fields; only the latter has bounds.
/// This type deliberately does not implement Clone or value equality.
#[derive(Debug)]
pub struct AttributeValue {
    default_value: f64,
    description_id: Option<JavaString>,
    syncable: AtomicBool,
    // 0/1/2 are the actual enum alternatives;3 is Java's permitted null field.
    sentiment: AtomicU8,
    bounds: Option<RangedBounds>,
}
impl AttributeValue {
    /// Attribute constructor19–24: no numeric validation or sanitization.
    pub fn new(description_id: JavaString, default_value: f64) -> Self {
        Self::with_nullable_description(Some(description_id), default_value)
    }
    /// Java's raw String constructor also permits null; the getter returns it.
    pub fn with_nullable_description(
        description_id: Option<JavaString>,
        default_value: f64,
    ) -> Self {
        Self {
            default_value,
            description_id,
            syncable: AtomicBool::new(false),
            sentiment: AtomicU8::new(Sentiment::Positive as u8),
            bounds: None,
        }
    }
    pub fn ranged(
        description_id: JavaString,
        default_value: f64,
        min_value: f64,
        max_value: f64,
    ) -> Result<Self, RangeError> {
        Self::ranged_with_nullable_description(
            Some(description_id),
            default_value,
            min_value,
            max_value,
        )
    }
    /// RangedAttribute10–23. Preserve Java comparison order, raw NaNs and both
    /// signed zeros; NaN is not an extra constructor rejection in this source.
    pub fn ranged_with_nullable_description(
        description_id: Option<JavaString>,
        default_value: f64,
        min_value: f64,
        max_value: f64,
    ) -> Result<Self, RangeError> {
        let mut value = Self::with_nullable_description(description_id, default_value);
        value.bounds = Some(RangedBounds {
            min_value,
            max_value,
        });
        if min_value > max_value {
            return Err(RangeError::MinimumAboveMaximum);
        }
        if default_value < min_value {
            return Err(RangeError::DefaultBelowMinimum);
        }
        if default_value > max_value {
            return Err(RangeError::DefaultAboveMaximum);
        }
        Ok(value)
    }
    pub fn default_value(&self) -> f64 {
        self.default_value
    }
    pub fn description_id(&self) -> Option<&JavaString> {
        self.description_id.as_ref()
    }
    pub fn bounds(&self) -> Option<&RangedBounds> {
        self.bounds.as_ref()
    }
    pub fn is_client_syncable(&self) -> bool {
        self.syncable.load(Ordering::Relaxed)
    }
    /// Source setSyncable36–37 mutates this SAME value and returns this receiver.
    /// No instance dirty/cache callback is implied by changing the flag.
    pub fn set_syncable(&self, syncable: bool) -> &Self {
        self.syncable.store(syncable, Ordering::Relaxed);
        self
    }
    pub fn sentiment(&self) -> Option<Sentiment> {
        match self.sentiment.load(Ordering::Relaxed) {
            0 => Some(Sentiment::Positive),
            1 => Some(Sentiment::Neutral),
            2 => Some(Sentiment::Negative),
            3 => None,
            _ => unreachable!("only native sentiment states can be stored"),
        }
    }
    pub fn set_sentiment(&self, sentiment: Sentiment) -> &Self {
        self.set_nullable_sentiment(Some(sentiment))
    }
    /// Source setSentiment41–42 permits null; failure occurs later at getStyle.
    pub fn set_nullable_sentiment(&self, sentiment: Option<Sentiment>) -> &Self {
        self.sentiment
            .store(sentiment.map_or(3, |value| value as u8), Ordering::Relaxed);
        self
    }
    /// Attribute.getStyle54 dereferences its current sentiment. Null is a typed
    /// source NPE boundary, not an invented fallback color or eager setter error.
    pub fn style(&self, positive: bool) -> Result<AttributeStyle, NullSentiment> {
        self.sentiment()
            .map(|sentiment| sentiment.style(positive))
            .ok_or(NullSentiment)
    }
    /// Attribute46 identity; RangedAttribute35–38 maps input NaN to raw min,
    /// otherwise original Mth.clamp(double). Raw base/default fields stay raw.
    pub fn sanitize_value(&self, value: f64) -> f64 {
        let Some(bounds) = &self.bounds else {
            return value;
        };
        if value.is_nan() {
            bounds.min_value
        } else if value < bounds.min_value {
            bounds.min_value
        } else {
            java_min(value, bounds.max_value)
        }
    }
}

fn java_min(a: f64, b: f64) -> f64 {
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

/// The real registry/holder owner resolves its actual already-registered value.
/// It must reject unbound/unregistered references as appropriate and retain the
/// same AttributeValue object; no successful generated default/ID lookup is valid.
pub trait RegisteredAttributeOwner {
    type Holder;
    type Error;
    fn registered_attribute_value(
        &mut self,
        holder: &Self::Holder,
    ) -> Result<Arc<AttributeValue>, Self::Error>;
}

/// Adapter from an actual registered holder/value binding to existing native
/// instance/map traits. It owns no registry or hash and never copies value fields.
pub struct BoundAttribute<H> {
    holder: H,
    value: Arc<AttributeValue>,
}
impl<H> BoundAttribute<H> {
    /// Resolve through the actual owner AFTER the source registration/binding
    /// phase. A raw value can exist earlier; never force an early holder binding.
    pub fn from_registered<O: RegisteredAttributeOwner<Holder = H>>(
        holder: H,
        owner: &mut O,
    ) -> Result<Self, O::Error> {
        let value = owner.registered_attribute_value(&holder)?;
        Ok(Self { holder, value })
    }
    pub fn holder(&self) -> &H {
        &self.holder
    }
    pub fn value(&self) -> &Arc<AttributeValue> {
        &self.value
    }
}
impl<H> AttributeDefinition for BoundAttribute<H> {
    type Key = H;
    fn key(&self) -> &H {
        &self.holder
    }
    fn default_value(&self) -> f64 {
        self.value.default_value()
    }
    fn sanitize_value(&self, value: f64) -> f64 {
        self.value.sanitize_value(value)
    }
}
impl<H> MapAttributeDefinition for BoundAttribute<H> {
    fn is_client_syncable(&self) -> bool {
        self.value.is_client_syncable()
    }
}

/// Raw registration VALUE data from Attributes12–42, in its real31-entry
/// registration order. These records instantiate neither a registry nor holders.
#[derive(Clone, Copy, Debug)]
pub struct AttributeSpec {
    pub name: &'static str,
    pub description_id: &'static str,
    pub default_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub syncable: bool,
    pub sentiment: Sentiment,
}
impl AttributeSpec {
    /// Construct the source value only. Actual registerForHolder/registry freeze
    /// and canonical bound adapter creation must occur in their real phases.
    pub fn create_value(&self) -> Result<AttributeValue, RangeError> {
        let value = AttributeValue::ranged(
            self.description_id.into(),
            self.default_value,
            self.min_value,
            self.max_value,
        )?;
        if self.syncable {
            value.set_syncable(true);
        }
        if self.sentiment != Sentiment::Positive {
            value.set_sentiment(self.sentiment);
        }
        Ok(value)
    }
}
macro_rules! spec {
    ($path:literal, $default:expr, $min:expr, $max:expr, $sync:expr, $sentiment:ident) => {
        AttributeSpec {
            name: concat!("minecraft:", $path),
            description_id: concat!("attribute.name.", $path),
            default_value: $default,
            min_value: $min,
            max_value: $max,
            syncable: $sync,
            sentiment: Sentiment::$sentiment,
        }
    };
}

pub const BUILTIN_ATTRIBUTE_SPECS: [AttributeSpec; 31] = [
    spec!("generic.armor", 0.0, 0.0, 30.0, true, Positive),
    spec!("generic.armor_toughness", 0.0, 0.0, 20.0, true, Positive),
    spec!("generic.attack_damage", 2.0, 0.0, 2048.0, false, Positive),
    spec!("generic.attack_knockback", 0.0, 0.0, 5.0, false, Positive),
    spec!("generic.attack_speed", 4.0, 0.0, 1024.0, true, Positive),
    spec!("player.block_break_speed", 1.0, 0.0, 1024.0, true, Positive),
    spec!(
        "player.block_interaction_range",
        4.5,
        0.0,
        64.0,
        true,
        Positive
    ),
    spec!("generic.burning_time", 1.0, 0.0, 1024.0, true, Negative),
    spec!(
        "generic.explosion_knockback_resistance",
        0.0,
        0.0,
        1.0,
        true,
        Positive
    ),
    spec!(
        "player.entity_interaction_range",
        3.0,
        0.0,
        64.0,
        true,
        Positive
    ),
    spec!(
        "generic.fall_damage_multiplier",
        1.0,
        0.0,
        100.0,
        true,
        Negative
    ),
    spec!("generic.flying_speed", 0.4, 0.0, 1024.0, true, Positive),
    spec!("generic.follow_range", 32.0, 0.0, 2048.0, false, Positive),
    spec!("generic.gravity", 0.08, -1.0, 1.0, true, Neutral),
    spec!(
        "generic.jump_strength",
        0.41999998688697815,
        0.0,
        32.0,
        true,
        Positive
    ),
    spec!(
        "generic.knockback_resistance",
        0.0,
        0.0,
        1.0,
        false,
        Positive
    ),
    spec!("generic.luck", 0.0, -1024.0, 1024.0, true, Positive),
    spec!("generic.max_absorption", 0.0, 0.0, 2048.0, true, Positive),
    spec!("generic.max_health", 20.0, 1.0, 1024.0, true, Positive),
    spec!("player.mining_efficiency", 0.0, 0.0, 1024.0, true, Positive),
    spec!("generic.movement_efficiency", 0.0, 0.0, 1.0, true, Positive),
    spec!("generic.movement_speed", 0.7, 0.0, 1024.0, true, Positive),
    spec!("generic.oxygen_bonus", 0.0, 0.0, 1024.0, true, Positive),
    spec!(
        "generic.safe_fall_distance",
        3.0,
        -1024.0,
        1024.0,
        true,
        Positive
    ),
    spec!("generic.scale", 1.0, 0.0625, 16.0, true, Neutral),
    spec!("player.sneaking_speed", 0.3, 0.0, 1.0, true, Positive),
    spec!(
        "zombie.spawn_reinforcements",
        0.0,
        0.0,
        1.0,
        false,
        Positive
    ),
    spec!("generic.step_height", 0.6, 0.0, 10.0, true, Positive),
    spec!(
        "player.submerged_mining_speed",
        0.2,
        0.0,
        20.0,
        true,
        Positive
    ),
    spec!(
        "player.sweeping_damage_ratio",
        0.0,
        0.0,
        1.0,
        true,
        Positive
    ),
    spec!(
        "generic.water_movement_efficiency",
        0.0,
        0.0,
        1.0,
        true,
        Positive
    ),
];

#[cfg(test)]
mod tests {
    use super::super::attribute_instance::{AttributeInstance, ResourceLocationKey};
    use super::super::attribute_map::{AttributeMapState, DirtySet, JavaObjectKey};
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    // Controlled actual retained holder/value bindings for component checks.
    // These fixtures allocate no production registry ID or Java identity hash.
    #[derive(Clone)]
    struct Holder(Rc<RefCell<Option<Arc<AttributeValue>>>>);
    impl PartialEq for Holder {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for Holder {}
    impl JavaObjectKey for Holder {
        fn java_hash_code(&self) -> i32 {
            17
        }
    }
    struct Owner {
        calls: usize,
    }
    impl RegisteredAttributeOwner for Owner {
        type Holder = Holder;
        type Error = &'static str;
        fn registered_attribute_value(
            &mut self,
            holder: &Holder,
        ) -> Result<Arc<AttributeValue>, Self::Error> {
            self.calls += 1;
            holder
                .0
                .borrow()
                .as_ref()
                .cloned()
                .ok_or("actual holder value is unbound")
        }
    }
    #[derive(Clone)]
    struct InstanceKey(Rc<()>);
    impl PartialEq for InstanceKey {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for InstanceKey {}
    impl JavaObjectKey for InstanceKey {
        fn java_hash_code(&self) -> i32 {
            29
        }
    }
    #[derive(Clone, PartialEq, Eq)]
    struct ModifierKey;
    impl ResourceLocationKey for ModifierKey {
        fn java_hash_code(&self) -> i32 {
            0
        }
    }

    #[test]
    fn registered_native_value_identity_controls_real_instance_cache_and_map_dirty_sync() {
        let value = Arc::new(
            AttributeValue::ranged(
                "attribute.name.generic.max_health".into(),
                20.0,
                1.0,
                1024.0,
            )
            .unwrap(),
        );
        let holder = Holder(Rc::new(RefCell::new(Some(Arc::clone(&value)))));
        let mut owner = Owner { calls: 0 };
        let first = Arc::new(BoundAttribute::from_registered(holder.clone(), &mut owner).unwrap());
        let second = Arc::new(BoundAttribute::from_registered(holder.clone(), &mut owner).unwrap());
        assert!(Arc::ptr_eq(first.value(), second.value()));
        assert!(Arc::ptr_eq(first.value(), &value));
        assert!(first.holder() == &holder);
        let mut instance: AttributeInstance<BoundAttribute<Holder>, ModifierKey> =
            AttributeInstance::new(first);
        let other: AttributeInstance<BoundAttribute<Holder>, ModifierKey> =
            AttributeInstance::new(second);
        assert_eq!(other.base_value(), 20.0);
        let instance_key = InstanceKey(Rc::new(()));
        let mut map: AttributeMapState<Holder, InstanceKey, ()> = AttributeMapState::new(());
        instance
            .set_base_value(-37.0, &mut map.listener(instance_key.clone()))
            .unwrap();
        assert_eq!(instance.base_value(), -37.0);
        assert_eq!(instance.value(), 1.0);
        assert!(!instance.is_dirty());
        assert_eq!(map.dirty_len(DirtySet::Update), 1);
        assert_eq!(map.dirty_len(DirtySet::Sync), 0);
        assert!(std::ptr::eq(value.set_syncable(true), value.as_ref()));
        assert!(other.attribute().is_client_syncable());
        assert!(!instance.is_dirty());
        instance
            .set_base_value(3.0, &mut map.listener(instance_key.clone()))
            .unwrap();
        assert_eq!(instance.value(), 3.0);
        assert_eq!(map.dirty_len(DirtySet::Sync), 1);
        value.set_syncable(false);
        // The setter does not retroactively remove an already-dirty sync entry.
        assert_eq!(map.dirty_len(DirtySet::Sync), 1);
        map.clear_dirty(DirtySet::Sync);
        instance
            .set_base_value(f64::NAN, &mut map.listener(instance_key))
            .unwrap();
        assert!(instance.base_value().is_nan());
        assert_eq!(instance.value(), 1.0);
        assert_eq!(map.dirty_len(DirtySet::Sync), 0);
    }

    #[test]
    fn raw_constructor_checks_and_sanitizers_preserve_source_nan_zero_and_order() {
        assert_eq!(
            AttributeValue::ranged("x".into(), -3.0, 2.0, 1.0).unwrap_err(),
            RangeError::MinimumAboveMaximum
        );
        assert_eq!(
            AttributeValue::ranged("x".into(), -1.0, 0.0, 10.0).unwrap_err(),
            RangeError::DefaultBelowMinimum
        );
        assert_eq!(
            AttributeValue::ranged("x".into(), 11.0, 0.0, 10.0).unwrap_err(),
            RangeError::DefaultAboveMaximum
        );
        assert_eq!(
            RangeError::MinimumAboveMaximum.source_message(),
            "Minimum value cannot be bigger than maximum value!"
        );
        let nan = f64::from_bits(0x7ff8_0000_0000_0017);
        let base = AttributeValue::new("base".into(), nan);
        assert_eq!(base.default_value().to_bits(), nan.to_bits());
        assert_eq!(base.sanitize_value(nan).to_bits(), nan.to_bits());
        assert_eq!(base.sanitize_value(-0.0).to_bits(), (-0.0_f64).to_bits());
        let ranged = AttributeValue::ranged("range".into(), nan, 1.0, 8.0).unwrap();
        assert_eq!(ranged.default_value().to_bits(), nan.to_bits());
        assert_eq!(ranged.sanitize_value(nan), 1.0);
        assert_eq!(ranged.sanitize_value(f64::NEG_INFINITY), 1.0);
        assert_eq!(ranged.sanitize_value(f64::INFINITY), 8.0);
        let zero = AttributeValue::ranged("zero".into(), 0.0, -0.0, -0.0).unwrap();
        assert_eq!(zero.default_value().to_bits(), 0.0_f64.to_bits());
        assert_eq!(
            zero.bounds().unwrap().min_value().to_bits(),
            (-0.0_f64).to_bits()
        );
        assert_eq!(zero.sanitize_value(0.0).to_bits(), (-0.0_f64).to_bits());
        let nan_max = AttributeValue::ranged("nanmax".into(), 0.0, 0.0, nan).unwrap();
        assert_eq!(nan_max.sanitize_value(1.0).to_bits(), nan.to_bits());
        assert_eq!(nan_max.sanitize_value(nan), 0.0);
        let nan_min = AttributeValue::ranged("nanmin".into(), 0.0, nan, 10.0).unwrap();
        assert_eq!(nan_min.sanitize_value(1.0), 1.0);
        assert_eq!(nan_min.sanitize_value(nan).to_bits(), nan.to_bits());
    }

    #[test]
    fn shared_sentiment_mutations_and_late_null_failure_preserve_source_receiver() {
        let description = JavaString::from_utf16_lossless(vec![0xd800, 0x61]);
        let value = Arc::new(AttributeValue::new(description.clone(), 1.0));
        let shared = Arc::clone(&value);
        assert_eq!(shared.description_id(), Some(&description));
        assert_eq!(value.style(true), Ok(AttributeStyle::Blue));
        assert_eq!(value.style(false), Ok(AttributeStyle::Red));
        assert!(std::ptr::eq(
            value.set_sentiment(Sentiment::Negative),
            value.as_ref()
        ));
        assert_eq!(shared.style(true), Ok(AttributeStyle::Red));
        assert_eq!(shared.style(false), Ok(AttributeStyle::Blue));
        value.set_sentiment(Sentiment::Neutral);
        assert_eq!(shared.style(true), Ok(AttributeStyle::Gray));
        assert_eq!(shared.style(false), Ok(AttributeStyle::Gray));
        value.set_nullable_sentiment(None);
        assert_eq!(shared.style(true), Err(NullSentiment));
        value.set_sentiment(Sentiment::Positive);
        assert_eq!(shared.style(true), Ok(AttributeStyle::Blue));
        let null_description = AttributeValue::with_nullable_description(None, 0.0);
        assert_eq!(null_description.description_id(), None);
    }

    #[test]
    fn binding_waits_for_actual_owner_and_specs_create_values_without_registry_or_bat_overrides() {
        let holder = Holder(Rc::new(RefCell::new(None)));
        let mut owner = Owner { calls: 0 };
        assert!(matches!(
            BoundAttribute::from_registered(holder.clone(), &mut owner),
            Err("actual holder value is unbound")
        ));
        assert_eq!(owner.calls, 1);
        let spec = &BUILTIN_ATTRIBUTE_SPECS[18];
        assert_eq!(spec.name, "minecraft:generic.max_health");
        let value = Arc::new(spec.create_value().unwrap());
        assert_eq!(value.default_value(), 20.0); // Bat6 is a supplier base override.
        *holder.0.borrow_mut() = Some(Arc::clone(&value));
        let bound = BoundAttribute::from_registered(holder, &mut owner).unwrap();
        assert!(Arc::ptr_eq(bound.value(), &value));
        assert!(bound.is_client_syncable());
        assert_eq!(
            BUILTIN_ATTRIBUTE_SPECS[14].default_value.to_bits(),
            0.41999998688697815_f64.to_bits()
        );
        assert_eq!(BUILTIN_ATTRIBUTE_SPECS[12].default_value, 32.0); // Mob16 is a supplier override.
        for spec in &BUILTIN_ATTRIBUTE_SPECS {
            let value = spec.create_value().unwrap();
            assert_eq!(
                value.default_value().to_bits(),
                spec.default_value.to_bits()
            );
            assert_eq!(value.is_client_syncable(), spec.syncable);
            assert_eq!(value.sentiment(), Some(spec.sentiment));
        }
    }
}
