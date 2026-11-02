//! Java 1.21 AttributeModifier/AttributeInstance/AttributeMap NBT bodies.
//!
//! The real canonical ATTRIBUTE registry, bound holder names, modifier value
//! keys and source warning sink remain mandatory owners. This module performs
//! the native fixed codec and state mutation; it neither constructs a registry
//! nor substitutes compiled numeric IDs or actor state. Input Tags are the
//! caller's actual read-only NBT values for this invocation. It writes no files.

use super::attribute_instance::{
    resource_location_java_hash, AttributeDefinition, Modifier, Operation, ResourceLocationKey,
};
use super::attribute_map::{self, AttributeMapHost, DirtyIterationError};
use crate::nbt::{JavaString, Tag};
use std::sync::Arc;

/// The actual ResourceLocation value domain. Construction validates Java's
/// ASCII namespace/path characters; empty paths are valid in this source.
/// This is a value object, not a registry entry or Holder.Reference identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceLocation {
    namespace: String,
    path: String,
}
impl ResourceLocation {
    pub fn from_parts(namespace: &str, path: &str) -> Option<Self> {
        if !namespace.bytes().all(namespace_char) || !path.bytes().all(path_char) {
            return None;
        }
        Some(Self {
            namespace: namespace.into(),
            path: path.into(),
        })
    }
    /// Source bySeparator/tryBySeparator: first colon splits; leading colon
    /// uses default namespace, as does no colon. Invalid UTF-16 units reject.
    pub fn parse(value: &JavaString) -> Option<Self> {
        let units: Vec<_> = value.iter_units().collect();
        if units.iter().any(|unit| *unit > 127) {
            return None;
        }
        let bytes: Vec<_> = units.into_iter().map(|unit| unit as u8).collect();
        let text = std::str::from_utf8(&bytes).ok()?;
        match text.find(':') {
            Some(0) => Self::from_parts("minecraft", &text[1..]),
            Some(index) => Self::from_parts(&text[..index], &text[index + 1..]),
            None => Self::from_parts("minecraft", text),
        }
    }
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn as_string(&self) -> String {
        format!("{}:{}", self.namespace, self.path)
    }
}
impl ResourceLocationKey for ResourceLocation {
    fn java_hash_code(&self) -> i32 {
        resource_location_java_hash(&self.namespace, &self.path)
    }
}
fn namespace_char(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
}
fn path_char(byte: u8) -> bool {
    namespace_char(byte) || byte == b'/'
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModifierFieldError {
    Missing(&'static str),
    WrongType { field: &'static str, actual: u8 },
    InvalidId(JavaString),
    UnknownOperation(JavaString),
}

/// Source logging sites. Modifier issues contain every native fixed-codec
/// failure plus the original record for a real warning owner to render source
/// diagnostics. These are warnings followed by source skip, not load errors.
pub enum AttributeWarning<'a> {
    ModifierDecode {
        issues: &'a [ModifierFieldError],
        record: &'a Tag,
    },
    MalformedAttributeId(&'a JavaString),
    UnknownAttributeId(&'a ResourceLocation),
}

pub trait AttributePersistenceHost: AttributeMapHost {
    /// Actual holder.unwrapKey().map(ResourceKey.location), including None for
    /// an unbound/direct holder. Never infer this from a protocol integer ID.
    fn bound_attribute_name(
        &mut self,
        holder: &Self::Holder,
    ) -> Result<Option<ResourceLocation>, Self::Error>;
    /// Actual BuiltInRegistries.ATTRIBUTE.getHolder lookup for this identifier.
    fn registered_attribute(
        &mut self,
        name: &ResourceLocation,
    ) -> Result<Option<Self::Holder>, Self::Error>;
    /// Pure bridges to the real immutable ResourceLocation modifier value key.
    /// No registry entry, actor effect or identity-hash allocation is introduced.
    fn modifier_key(&mut self, name: ResourceLocation) -> Result<Self::ModifierKey, Self::Error>;
    fn modifier_name(&mut self, key: &Self::ModifierKey) -> Result<ResourceLocation, Self::Error>;
    fn warning(&mut self, warning: AttributeWarning<'_>) -> Result<(), Self::Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum AttributePersistenceError<E> {
    Host(E),
    /// Original save throws IllegalStateException before writing an id/base.
    UnboundAttributeHolder,
    ExpectedCompound,
    ExpectedList,
    Traversal(DirtyIterationError),
}

fn compound(entries: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(
        entries
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect(),
    )
}
fn nbt_double(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}
fn number(tag: &Tag) -> Option<f64> {
    Some(match tag {
        Tag::Byte(value) => f64::from(*value),
        Tag::Short(value) => f64::from(*value),
        Tag::Int(value) => f64::from(*value),
        Tag::Long(value) => *value as f64,
        Tag::Float(value) => f64::from(*value),
        Tag::Double(value) => *value,
        _ => return None,
    })
}
fn operation_name(operation: Operation) -> &'static str {
    match operation {
        Operation::AddValue => "add_value",
        Operation::AddMultipliedBase => "add_multiplied_base",
        Operation::AddMultipliedTotal => "add_multiplied_total",
    }
}
fn operation(value: &JavaString) -> Option<Operation> {
    if value == "add_value" {
        Some(Operation::AddValue)
    } else if value == "add_multiplied_base" {
        Some(Operation::AddMultipliedBase)
    } else if value == "add_multiplied_total" {
        Some(Operation::AddMultipliedTotal)
    } else {
        None
    }
}

/// Original fixed Modifier record codec (id, amount, operation), NbtOps with
/// compressMaps=false. Unknown fields are ignored; all three fields are required.
pub fn load_modifier<H: AttributePersistenceHost>(
    tag: &Tag,
    host: &mut H,
) -> Result<Option<Arc<Modifier<H::ModifierKey>>>, AttributePersistenceError<H::Error>> {
    if !matches!(tag, Tag::Compound(_)) {
        return Err(AttributePersistenceError::ExpectedCompound);
    }
    let mut issues = Vec::new();
    let id = match tag.get("id") {
        None => {
            issues.push(ModifierFieldError::Missing("id"));
            None
        }
        Some(Tag::String(value)) => match ResourceLocation::parse(value) {
            Some(id) => Some(id),
            None => {
                issues.push(ModifierFieldError::InvalidId(value.clone()));
                None
            }
        },
        Some(value) => {
            issues.push(ModifierFieldError::WrongType {
                field: "id",
                actual: value.tag_id(),
            });
            None
        }
    };
    let amount = match tag.get("amount") {
        None => {
            issues.push(ModifierFieldError::Missing("amount"));
            None
        }
        Some(value) => match number(value) {
            Some(amount) => Some(amount),
            None => {
                issues.push(ModifierFieldError::WrongType {
                    field: "amount",
                    actual: value.tag_id(),
                });
                None
            }
        },
    };
    let op = match tag.get("operation") {
        None => {
            issues.push(ModifierFieldError::Missing("operation"));
            None
        }
        Some(Tag::String(value)) => match operation(value) {
            Some(op) => Some(op),
            None => {
                issues.push(ModifierFieldError::UnknownOperation(value.clone()));
                None
            }
        },
        Some(value) => {
            issues.push(ModifierFieldError::WrongType {
                field: "operation",
                actual: value.tag_id(),
            });
            None
        }
    };
    if !issues.is_empty() {
        // RecordCodecBuilder applies the right-hand field errors first.
        issues.reverse();
        host.warning(AttributeWarning::ModifierDecode {
            issues: &issues,
            record: tag,
        })
        .map_err(AttributePersistenceError::Host)?;
        return Ok(None);
    }
    let key = host
        .modifier_key(id.expect("validated id"))
        .map_err(AttributePersistenceError::Host)?;
    Ok(Some(Arc::new(Modifier::new(
        key,
        amount.expect("validated amount"),
        op.expect("validated operation"),
    ))))
}

pub fn save_modifier<H: AttributePersistenceHost>(
    modifier: &Modifier<H::ModifierKey>,
    host: &mut H,
) -> Result<Tag, AttributePersistenceError<H::Error>> {
    let name = host
        .modifier_name(modifier.id())
        .map_err(AttributePersistenceError::Host)?;
    Ok(compound(vec![
        ("amount", Tag::Double(nbt_double(modifier.amount()))),
        ("id", Tag::String(name.as_string().into())),
        (
            "operation",
            Tag::String(operation_name(modifier.operation()).into()),
        ),
    ]))
}

/// Source AttributeInstance.save183–196. Does not read/calculate the value cache
/// and emits only permanent records, in their actual ArrayMap encounter order.
pub fn save_instance<H: AttributePersistenceHost>(
    instance: &H::Instance,
    host: &mut H,
) -> Result<Tag, AttributePersistenceError<H::Error>> {
    let holder = host
        .with_instance(instance, |instance| instance.attribute().key().clone())
        .map_err(AttributePersistenceError::Host)?;
    let name = host
        .bound_attribute_name(&holder)
        .map_err(AttributePersistenceError::Host)?
        .ok_or(AttributePersistenceError::UnboundAttributeHolder)?;
    let base = host
        .with_instance(instance, |instance| instance.base_value())
        .map_err(AttributePersistenceError::Host)?;
    let mut result = compound(vec![
        ("id", Tag::String(name.as_string().into())),
        ("base", Tag::Double(nbt_double(base))),
    ]);
    // Modifier codecs read immutable record values only. The snapshot retains
    // those exact Arc records and source order; no actor callback is elided.
    let permanent = host
        .with_instance(instance, |instance| instance.permanent_modifiers())
        .map_err(AttributePersistenceError::Host)?;
    if !permanent.is_empty() {
        let mut values = Vec::new();
        for modifier in permanent {
            values.push(save_modifier(&modifier, host)?);
        }
        // Source CompoundTag's actual HashMap encounter order is id,
        // modifiers,base here, although the base was written before the list.
        let Tag::Compound(entries) = &mut result else {
            unreachable!("new compound")
        };
        entries.insert(
            1,
            (
                "modifiers".into(),
                Tag::List {
                    element_type: 10,
                    values,
                },
            ),
        );
    }
    Ok(result)
}

/// Source AttributeInstance.load200–215 for a receiver owned by this real map.
/// Raw base and per-record stores leave dirty/cache untouched until the single
/// terminal callback. Existing indexes are NOT cleared before loading.
pub fn load_instance<H: AttributePersistenceHost>(
    map: &H::Map,
    instance: &H::Instance,
    tag: &Tag,
    host: &mut H,
) -> Result<(), AttributePersistenceError<H::Error>> {
    if !matches!(tag, Tag::Compound(_)) {
        return Err(AttributePersistenceError::ExpectedCompound);
    }
    let base = tag.get("base").and_then(number).unwrap_or(0.0);
    host.with_instance(instance, |instance| instance.load_base_fields(base))
        .map_err(AttributePersistenceError::Host)?;
    if let Some(Tag::List {
        element_type,
        values,
    }) = tag.get("modifiers")
    {
        // CompoundTag.getList(name,10) filters a nonempty list of another type.
        if values.is_empty() || *element_type == 10 {
            let empty = Tag::Compound(Vec::new());
            for entry in values {
                let record = if matches!(entry, Tag::Compound(_)) {
                    entry
                } else {
                    &empty
                };
                if let Some(modifier) = load_modifier(record, host)? {
                    host.with_instance(instance, |instance| {
                        instance.load_modifier_fields(modifier)
                    })
                    .map_err(AttributePersistenceError::Host)?;
                }
            }
        }
    }
    host.with_instance(instance, |instance| instance.finish_load_fields())
        .map_err(AttributePersistenceError::Host)?;
    attribute_map::on_attribute_modified(map, instance, host)
        .map_err(AttributePersistenceError::Host)
}

/// AttributeMap.save121–125 uses a captured-count live ValueIterator, not the
/// position-only forEach used by transfer and not a sorted list of holders.
pub fn save_map<H: AttributePersistenceHost>(
    map: &H::Map,
    host: &mut H,
) -> Result<Tag, AttributePersistenceError<H::Error>> {
    let mut cursor = host
        .with_map(map, |map| map.values_iterator_cursor())
        .map_err(AttributePersistenceError::Host)?;
    let mut values = Vec::new();
    while cursor.has_next() {
        let instance = host
            .with_map(map, |map| map.next_iterator_value(&mut cursor))
            .map_err(AttributePersistenceError::Host)?
            .map_err(AttributePersistenceError::Traversal)?
            .expect("iterator has remaining entry");
        values.push(save_instance(&instance, host)?);
    }
    Ok(Tag::List {
        element_type: if values.is_empty() { 0 } else { 10 },
        values,
    })
}

/// AttributeMap.load129–146. The actual global ATTRIBUTE registry lookup and
/// source warnings precede per-actor getInstance/load. No protocol ID fallback.
pub fn load_map<H: AttributePersistenceHost>(
    map: &H::Map,
    tag: &Tag,
    host: &mut H,
) -> Result<(), AttributePersistenceError<H::Error>> {
    let Tag::List { values, .. } = tag else {
        return Err(AttributePersistenceError::ExpectedList);
    };
    let empty = Tag::Compound(Vec::new());
    for entry in values {
        let record = if matches!(entry, Tag::Compound(_)) {
            entry
        } else {
            &empty
        };
        let raw = match record.get("id") {
            Some(Tag::String(value)) => value.clone(),
            _ => JavaString::default(),
        };
        let Some(name) = ResourceLocation::parse(&raw) else {
            host.warning(AttributeWarning::MalformedAttributeId(&raw))
                .map_err(AttributePersistenceError::Host)?;
            continue;
        };
        let Some(holder) = host
            .registered_attribute(&name)
            .map_err(AttributePersistenceError::Host)?
        else {
            host.warning(AttributeWarning::UnknownAttributeId(&name))
                .map_err(AttributePersistenceError::Host)?;
            continue;
        };
        if let Some(instance) = attribute_map::get_instance(map, &holder, host)
            .map_err(AttributePersistenceError::Host)?
        {
            load_instance(map, &instance, record, host)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::attribute_map::{get_instance, DirtySet};
    use super::super::attribute_transfer::test_support::*;
    use super::*;

    fn record(id: &str, amount: Tag, operation: Tag) -> Tag {
        compound(vec![
            ("id", Tag::String(id.into())),
            ("amount", amount),
            ("operation", operation),
        ])
    }
    fn serialized_operation(name: &str) -> Tag {
        Tag::String(name.into())
    }
    fn modifier_list(records: Vec<Tag>) -> Tag {
        Tag::List {
            element_type: 10,
            values: records,
        }
    }
    fn attribute_record(id: &str, base: Tag, records: Tag) -> Tag {
        compound(vec![
            ("id", Tag::String(id.into())),
            ("base", base),
            ("modifiers", records),
        ])
    }

    #[test]
    fn actual_modifier_schema_preserves_raw_load_zero_and_save_factory_normalizes() {
        let mut host = Host::new();
        for amount in [
            Tag::Byte(-2),
            Tag::Short(-3),
            Tag::Int(4),
            Tag::Long(5),
            Tag::Float(0.25),
            Tag::Double(f64::NAN),
        ] {
            let tag = record(
                "no_namespace",
                amount.clone(),
                serialized_operation("add_value"),
            );
            let loaded = load_modifier(&tag, &mut host).unwrap().unwrap();
            assert_eq!(loaded.id().as_string(), "minecraft:no_namespace");
            assert_eq!(loaded.operation(), Operation::AddValue);
            if matches!(amount, Tag::Double(value) if value.is_nan()) {
                assert!(loaded.amount().is_nan());
            } else {
                assert_eq!(loaded.amount(), number(&amount).unwrap());
            }
        }
        for amount in [Tag::Float(-0.0), Tag::Double(-0.0)] {
            let loaded = load_modifier(
                &record(":", amount, serialized_operation("add_multiplied_total")),
                &mut host,
            )
            .unwrap()
            .unwrap();
            assert_eq!(loaded.id().as_string(), "minecraft:");
            assert_eq!(loaded.amount().to_bits(), (-0.0_f64).to_bits());
            let saved = save_modifier(&loaded, &mut host).unwrap();
            let Tag::Compound(entries) = &saved else {
                panic!("modifier compound")
            };
            assert_eq!(
                entries
                    .iter()
                    .map(|(key, _)| key.to_string_lossy().into_owned())
                    .collect::<Vec<_>>(),
                ["amount", "id", "operation"]
            );
            assert_eq!(
                number(saved.get("amount").unwrap()).unwrap().to_bits(),
                0.0_f64.to_bits()
            );
        }
        for bad in [
            Tag::Int(0),
            serialized_operation("0"),
            serialized_operation("ADD_VALUE"),
        ] {
            assert!(
                load_modifier(&record("probe:id", Tag::Double(1.0), bad), &mut host)
                    .unwrap()
                    .is_none()
            );
        }
        assert!(load_modifier(&compound(Vec::new()), &mut host)
            .unwrap()
            .is_none());
        assert_eq!(host.warnings.len(), 4);
        assert_eq!(
            host.warnings[3],
            "modifier:[Missing(\"operation\"), Missing(\"amount\"), Missing(\"id\")]"
        );
        assert!(ResourceLocation::parse(&JavaString::from_utf16_lossless(vec![0xd800])).is_none());
        assert!(ResourceLocation::parse(&"Bad:id".into()).is_none());
        assert!(ResourceLocation::parse(&"a:b:c".into()).is_none());
        assert_eq!(
            ResourceLocation::parse(&"".into()).unwrap().as_string(),
            "minecraft:"
        );
    }

    #[test]
    fn load_keeps_callback_prefix_cached_value_and_duplicate_operation_ghosts() {
        let mut host = Host::new();
        let instance = get_instance(&0, &key(1), &mut host).unwrap().unwrap();
        let old_total = modifier("same", 0.1, Operation::AddMultipliedTotal);
        host.object(&instance)
            .borrow_mut()
            .add_transient_modifier(Arc::clone(&old_total), &mut IgnoreDirty)
            .unwrap();
        host.set_base(&instance, 5.0);
        let cached = 6.0 * 1.1;
        assert_eq!(host.object(&instance).borrow_mut().value(), cached);
        host.maps[0].borrow_mut().clear_dirty(DirtySet::Update);
        host.observe_warning = Some(instance.clone());
        host.fail_warning = true;
        let failed = attribute_record(
            "probe:foreign",
            Tag::Double(11.0),
            modifier_list(vec![compound(Vec::new())]),
        );
        assert_eq!(
            load_instance(&0, &instance, &failed, &mut host),
            Err(AttributePersistenceError::Host("warning failed"))
        );
        assert_eq!(host.warning_states, vec![(11.0, false, cached)]);
        assert!(!host.object(&instance).borrow().is_dirty());
        assert_eq!(host.maps[0].borrow().dirty_len(DirtySet::Update), 0);

        host.fail_warning = false;
        let tag = attribute_record(
            "probe:foreign",
            Tag::Double(11.0),
            modifier_list(vec![
                record(
                    "probe:same",
                    Tag::Double(2.0),
                    serialized_operation("add_value"),
                ),
                record(
                    "probe:same",
                    Tag::Double(0.5),
                    serialized_operation("add_multiplied_base"),
                ),
                compound(Vec::new()),
            ]),
        );
        load_instance(&0, &instance, &tag, &mut host).unwrap();
        assert_eq!(host.warning_states[1], (11.0, false, cached));
        assert!(host.object(&instance).borrow().is_dirty());
        assert_eq!(host.maps[0].borrow().dirty_len(DirtySet::Update), 1);
        let object = host.object(&instance);
        let state = object.borrow();
        assert_eq!(state.modifiers().len(), 2);
        assert_eq!(state.permanent_modifiers().len(), 2);
        assert_eq!(
            state.modifier(&location("same")).unwrap().operation(),
            Operation::AddMultipliedBase
        );
        assert!(state
            .operation_modifiers(Operation::AddValue)
            .iter()
            .any(|item| item.id() == &location("same") && item.amount() == 2.0));
        assert!(Arc::ptr_eq(
            &state.operation_modifiers(Operation::AddMultipliedTotal)[0],
            &old_total
        ));
        drop(state);
        let saved = save_instance(&instance, &mut host).unwrap();
        assert_eq!(
            saved.get("id"),
            Some(&Tag::String("probe:attribute1".into()))
        );
        let Tag::Compound(fields) = &saved else {
            panic!("instance compound")
        };
        assert_eq!(
            fields
                .iter()
                .map(|(key, _)| key.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["id", "modifiers", "base"]
        );
        let Tag::List { values, .. } = saved.get("modifiers").unwrap() else {
            panic!("modifier list")
        };
        assert_eq!(values.len(), 2);
        assert_eq!(
            values[1].get("operation"),
            Some(&serialized_operation("add_multiplied_base"))
        );

        host.maps[0].borrow_mut().clear_dirty(DirtySet::Update);
        host.fail_dirty_lookup = true;
        let final_failure = compound(vec![("base", Tag::Double(23.0))]);
        assert_eq!(
            load_instance(&0, &instance, &final_failure, &mut host),
            Err(AttributePersistenceError::Host(
                "dirty definition lookup failed"
            ))
        );
        assert_eq!(host.object(&instance).borrow().base_value(), 23.0);
        assert!(host.object(&instance).borrow().is_dirty());
        assert!(host.maps[0]
            .borrow()
            .dirty_contains(DirtySet::Update, &instance));
    }

    #[test]
    fn map_load_uses_real_registry_and_supplier_then_saves_bound_holder_raw_base() {
        let mut host = Host::new();
        let records = Tag::List {
            element_type: 10,
            values: vec![
                Tag::Int(0),
                compound(vec![("id", Tag::String("Bad:id".into()))]),
                compound(vec![("id", Tag::String("probe:missing".into()))]),
                attribute_record("probe:attribute4", Tag::Int(9), modifier_list(Vec::new())),
                attribute_record(
                    "probe:attribute1",
                    Tag::Double(-0.0),
                    Tag::List {
                        element_type: 8,
                        values: vec![Tag::String("wrong list".into())],
                    },
                ),
            ],
        };
        load_map(&0, &records, &mut host).unwrap();
        assert_eq!(host.maps[0].borrow().instantiated_len(), 1);
        assert_eq!(host.allocations, 1);
        assert_eq!(
            host.warnings,
            vec![
                "unknown:minecraft:",
                "malformed:Bad:id",
                "unknown:probe:missing"
            ]
        );
        let instance = get_instance(&0, &key(1), &mut host).unwrap().unwrap();
        assert_eq!(
            host.object(&instance).borrow().base_value().to_bits(),
            (-0.0_f64).to_bits()
        );
        assert_eq!(
            host.object(&instance).borrow().permanent_modifiers().len(),
            1
        );
        let saved = save_map(&0, &mut host).unwrap();
        let Tag::List {
            element_type,
            values,
        } = saved
        else {
            panic!("saved list")
        };
        assert_eq!(element_type, 10);
        assert_eq!(
            number(values[0].get("base").unwrap()).unwrap().to_bits(),
            0.0_f64.to_bits()
        );
        host.unbound = true;
        assert_eq!(
            save_instance(&instance, &mut host),
            Err(AttributePersistenceError::UnboundAttributeHolder)
        );
        host.unbound = false;
        load_instance(
            &0,
            &instance,
            &compound(vec![("Base", Tag::Int(99))]),
            &mut host,
        )
        .unwrap();
        assert_eq!(host.object(&instance).borrow().base_value(), 0.0);
        load_instance(
            &0,
            &instance,
            &compound(vec![("base", Tag::Double(-37.0))]),
            &mut host,
        )
        .unwrap();
        assert_eq!(
            number(
                save_instance(&instance, &mut host)
                    .unwrap()
                    .get("base")
                    .unwrap()
            ),
            Some(-37.0)
        );
        host.object(&instance)
            .borrow_mut()
            .replace_from_self(&mut IgnoreDirty)
            .unwrap();
        let Tag::Compound(fields) = save_instance(&instance, &mut host).unwrap() else {
            panic!("empty permanent instance compound")
        };
        assert_eq!(
            fields
                .iter()
                .map(|(key, _)| key.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["id", "base"]
        );
    }
}
