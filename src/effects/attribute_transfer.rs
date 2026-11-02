//! AttributeMap.assignAllValues/assignBaseValues from Java 1.21, 103–118.
//!
//! Both traverse the source table's live values().forEach protocol. They create
//! no actor or supplier, keep each original source object retained across target
//! lookup/copy, and run the target instance's actual map dirty listener at its
//! source point. Source and target may be the same map or instance.

use super::attribute_instance::AttributeDefinition;
use super::attribute_map::{self, AttributeMapHost, DirtyIterationError};

#[derive(Debug, PartialEq, Eq)]
pub enum AttributeTransferError<E> {
    Host(E),
    Traversal(DirtyIterationError),
}

/// Source assignBaseValues112–118. Read the source base AFTER target getInstance
/// (and any nested supplier/dirty callbacks), then source setBaseValue semantics.
pub fn assign_base_values<H: AttributeMapHost>(
    target: &H::Map,
    source: &H::Map,
    host: &mut H,
) -> Result<(), AttributeTransferError<H::Error>> {
    transfer(target, source, false, host)
}

/// Source assignAllValues103–109. Reuse actual replaceFrom field behavior; when
/// both handles identify one object, clearing the source maps is intentional.
pub fn assign_all_values<H: AttributeMapHost>(
    target: &H::Map,
    source: &H::Map,
    host: &mut H,
) -> Result<(), AttributeTransferError<H::Error>> {
    transfer(target, source, true, host)
}

fn transfer<H: AttributeMapHost>(
    target: &H::Map,
    source: &H::Map,
    all_values: bool,
    host: &mut H,
) -> Result<(), AttributeTransferError<H::Error>> {
    let mut cursor = host
        .with_map(source, |map| map.values_for_each_cursor())
        .map_err(AttributeTransferError::Host)?;
    loop {
        let source_instance = host
            .with_map(source, |map| map.next_for_each_value(&mut cursor))
            .map_err(AttributeTransferError::Host)?
            .map_err(AttributeTransferError::Traversal)?;
        let Some(source_instance) = source_instance else {
            break;
        };
        let holder = host
            .with_instance(&source_instance, |instance| {
                instance.attribute().key().clone()
            })
            .map_err(AttributeTransferError::Host)?;
        let Some(target_instance) = attribute_map::get_instance(target, &holder, host)
            .map_err(AttributeTransferError::Host)?
        else {
            continue;
        };
        let changed = if all_values {
            if target_instance == source_instance {
                host.with_instance(&target_instance, |instance| {
                    instance.replace_from_self_fields()
                })
                .map_err(AttributeTransferError::Host)?;
            } else {
                let data = host
                    .with_instance(&source_instance, |instance| instance.replacement_data())
                    .map_err(AttributeTransferError::Host)?;
                host.with_instance(&target_instance, |instance| {
                    instance.install_replacement(data)
                })
                .map_err(AttributeTransferError::Host)?;
            }
            true
        } else {
            let value = host
                .with_instance(&source_instance, |instance| instance.base_value())
                .map_err(AttributeTransferError::Host)?;
            host.with_instance(&target_instance, |instance| {
                instance.set_base_value_fields(value)
            })
            .map_err(AttributeTransferError::Host)?
        };
        if changed {
            attribute_map::on_attribute_modified(target, &target_instance, host)
                .map_err(AttributeTransferError::Host)?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::super::attribute_instance::{AttributeInstance, DirtyListener, Modifier, Operation};
    use super::super::attribute_map::{
        AttributeMapState, DirtySet, JavaObjectKey, MapAttributeDefinition,
    };
    use super::super::attribute_persistence::{
        AttributePersistenceHost, AttributeWarning, ResourceLocation,
    };
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        convert::Infallible,
        rc::Rc,
        sync::Arc,
    };

    #[derive(Clone, Debug)]
    pub struct Key {
        pub id: u32,
        hash: i32,
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Key {}
    impl JavaObjectKey for Key {
        fn java_hash_code(&self) -> i32 {
            self.hash
        }
    }
    pub fn key(id: u32) -> Key {
        Key {
            id,
            hash: id as i32,
        }
    }
    pub struct Definition {
        key: Key,
        pub calculations: Cell<usize>,
    }
    impl AttributeDefinition for Definition {
        type Key = Key;
        fn key(&self) -> &Key {
            &self.key
        }
        fn default_value(&self) -> f64 {
            4.0
        }
        fn sanitize_value(&self, value: f64) -> f64 {
            self.calculations.set(self.calculations.get() + 1);
            value
        }
    }
    impl MapAttributeDefinition for Definition {
        fn is_client_syncable(&self) -> bool {
            true
        }
    }
    pub type Instance = AttributeInstance<Definition, ResourceLocation>;
    pub type State = AttributeMapState<Key, Key, u32>;
    pub struct IgnoreDirty;
    impl DirtyListener<Definition, ResourceLocation> for IgnoreDirty {
        type Error = Infallible;
        fn on_dirty(&mut self, _: &mut Instance) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    pub fn location(path: &str) -> ResourceLocation {
        ResourceLocation::from_parts("probe", path).unwrap()
    }
    pub fn modifier(
        path: &str,
        amount: f64,
        operation: Operation,
    ) -> Arc<Modifier<ResourceLocation>> {
        Arc::new(Modifier::new(location(path), amount, operation))
    }
    pub struct Host {
        pub maps: Vec<Rc<RefCell<State>>>,
        pub objects: Vec<(Key, Rc<RefCell<Instance>>)>,
        definitions: Vec<Arc<Definition>>,
        templates: Vec<(Key, Key)>,
        next_id: u32,
        pub allocations: usize,
        pub warnings: Vec<String>,
        pub fail_warning: bool,
        pub observe_warning: Option<Key>,
        pub warning_states: Vec<(f64, bool, f64)>,
        pub unbound: bool,
        pub fail_dirty_lookup: bool,
        pub mutate_base_on_allocate: Option<(Key, f64)>,
    }
    impl Host {
        pub fn new() -> Self {
            let mut host = Self {
                maps: (0..2)
                    .map(|_| Rc::new(RefCell::new(State::new(9))))
                    .collect(),
                objects: Vec::new(),
                definitions: Vec::new(),
                templates: Vec::new(),
                next_id: 100,
                allocations: 0,
                warnings: Vec::new(),
                fail_warning: false,
                observe_warning: None,
                warning_states: Vec::new(),
                unbound: false,
                fail_dirty_lookup: false,
                mutate_base_on_allocate: None,
            };
            for id in 1..=4 {
                let definition = Arc::new(Definition {
                    key: key(id),
                    calculations: Cell::new(0),
                });
                if id != 4 {
                    let mut instance = Instance::new(Arc::clone(&definition));
                    instance
                        .add_permanent_modifier(
                            modifier("default", 1.0, Operation::AddValue),
                            &mut IgnoreDirty,
                        )
                        .unwrap();
                    host.objects
                        .push((key(id + 10), Rc::new(RefCell::new(instance))));
                    host.templates.push((key(id), key(id + 10)));
                }
                host.definitions.push(definition);
            }
            host
        }
        pub fn object(&self, id: &Key) -> Rc<RefCell<Instance>> {
            Rc::clone(&self.objects.iter().find(|(key, _)| key == id).unwrap().1)
        }
        pub fn assert_no_borrows(&self) {
            for map in &self.maps {
                drop(map.try_borrow_mut().expect("map borrowed across callback"));
            }
            for (_, object) in &self.objects {
                drop(
                    object
                        .try_borrow_mut()
                        .expect("instance borrowed across callback"),
                );
            }
        }
        pub fn set_base(&self, id: &Key, value: f64) {
            self.object(id)
                .borrow_mut()
                .set_base_value(value, &mut IgnoreDirty)
                .unwrap();
        }
    }
    impl AttributeMapHost for Host {
        type Error = &'static str;
        type Holder = Key;
        type Instance = Key;
        type Supplier = u32;
        type Map = usize;
        type Actor = u32;
        type ModifierKey = ResourceLocation;
        type Definition = Definition;
        fn with_map<R>(
            &mut self,
            map: &usize,
            access: impl FnOnce(&mut State) -> R,
        ) -> Result<R, Self::Error> {
            self.assert_no_borrows();
            Ok(access(&mut self.maps[*map].borrow_mut()))
        }
        fn with_instance<R>(
            &mut self,
            id: &Key,
            access: impl FnOnce(&mut Instance) -> R,
        ) -> Result<R, Self::Error> {
            self.assert_no_borrows();
            if self.fail_dirty_lookup
                && self
                    .maps
                    .iter()
                    .any(|map| map.borrow().dirty_contains(DirtySet::Update, id))
            {
                return Err("dirty definition lookup failed");
            }
            Ok(access(&mut self.object(id).borrow_mut()))
        }
        fn supplier_instance(
            &mut self,
            supplier: &u32,
            holder: &Key,
        ) -> Result<Option<Key>, Self::Error> {
            self.assert_no_borrows();
            assert_eq!(*supplier, 9);
            Ok(self
                .templates
                .iter()
                .find(|(id, _)| id == holder)
                .map(|(_, object)| object.clone()))
        }
        fn definition(&mut self, holder: &Key) -> Result<Arc<Definition>, Self::Error> {
            self.assert_no_borrows();
            Ok(Arc::clone(
                self.definitions
                    .iter()
                    .find(|def| def.key() == holder)
                    .unwrap(),
            ))
        }
        fn allocate_instance(&mut self, instance: Instance) -> Result<Key, Self::Error> {
            self.assert_no_borrows();
            let id = key(self.next_id);
            self.next_id += 1;
            self.allocations += 1;
            self.objects
                .push((id.clone(), Rc::new(RefCell::new(instance))));
            if let Some((source, base)) = self.mutate_base_on_allocate.take() {
                self.set_base(&source, base);
            }
            Ok(id)
        }
        fn on_attribute_updated(&mut self, _: &u32, _: &Key) -> Result<(), Self::Error> {
            panic!("transfer/load must not invent actor refresh")
        }
    }
    impl AttributePersistenceHost for Host {
        fn bound_attribute_name(
            &mut self,
            holder: &Key,
        ) -> Result<Option<ResourceLocation>, Self::Error> {
            self.assert_no_borrows();
            Ok((!self.unbound).then(|| location(&format!("attribute{}", holder.id))))
        }
        fn registered_attribute(
            &mut self,
            name: &ResourceLocation,
        ) -> Result<Option<Key>, Self::Error> {
            self.assert_no_borrows();
            Ok((1..=4)
                .find(|id| name == &location(&format!("attribute{id}")))
                .map(key))
        }
        fn modifier_key(
            &mut self,
            name: ResourceLocation,
        ) -> Result<ResourceLocation, Self::Error> {
            self.assert_no_borrows();
            Ok(name)
        }
        fn modifier_name(
            &mut self,
            name: &ResourceLocation,
        ) -> Result<ResourceLocation, Self::Error> {
            self.assert_no_borrows();
            Ok(name.clone())
        }
        fn warning(&mut self, warning: AttributeWarning<'_>) -> Result<(), Self::Error> {
            self.assert_no_borrows();
            if let Some(id) = &self.observe_warning {
                let object = self.object(id);
                let mut instance = object.borrow_mut();
                let base = instance.base_value();
                let dirty = instance.is_dirty();
                self.warning_states.push((base, dirty, instance.value()));
            }
            self.warnings.push(match warning {
                AttributeWarning::ModifierDecode { issues, .. } => format!("modifier:{issues:?}"),
                AttributeWarning::MalformedAttributeId(raw) => {
                    format!("malformed:{}", raw.to_string_lossy())
                }
                AttributeWarning::UnknownAttributeId(name) => {
                    format!("unknown:{}", name.as_string())
                }
            });
            if self.fail_warning {
                Err("warning failed")
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::attribute_instance::Operation;
    use super::super::attribute_map::{get_instance, DirtySet};
    use super::test_support::*;
    use super::*;
    use std::sync::Arc;

    #[test]
    fn base_transfer_reads_source_after_target_creation_and_obeys_java_equality() {
        let mut host = Host::new();
        let source = get_instance(&0, &key(1), &mut host).unwrap().unwrap();
        host.set_base(&source, 5.0);
        host.mutate_base_on_allocate = Some((source.clone(), 8.0));
        assign_base_values(&1, &0, &mut host).unwrap();
        let target = get_instance(&1, &key(1), &mut host).unwrap().unwrap();
        assert_eq!(host.object(&target).borrow().base_value(), 8.0);
        host.maps[1].borrow_mut().clear_dirty(DirtySet::Update);
        assign_base_values(&1, &0, &mut host).unwrap();
        assert_eq!(host.maps[1].borrow().dirty_len(DirtySet::Update), 0);
        host.set_base(&source, -0.0);
        host.set_base(&target, 0.0);
        host.maps[1].borrow_mut().clear_dirty(DirtySet::Update);
        assign_base_values(&1, &0, &mut host).unwrap();
        assert_eq!(
            host.object(&target).borrow().base_value().to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(host.maps[1].borrow().dirty_len(DirtySet::Update), 0);
    }

    #[test]
    fn full_transfer_retains_records_then_same_map_transfer_clears_source_alias() {
        let mut host = Host::new();
        let source = get_instance(&0, &key(1), &mut host).unwrap().unwrap();
        let permanent = modifier("custom", 3.0, Operation::AddMultipliedTotal);
        host.object(&source)
            .borrow_mut()
            .add_permanent_modifier(Arc::clone(&permanent), &mut IgnoreDirty)
            .unwrap();
        host.set_base(&source, 9.0);
        assign_all_values(&1, &0, &mut host).unwrap();
        let target = get_instance(&1, &key(1), &mut host).unwrap().unwrap();
        assert_ne!(source, target);
        assert_eq!(host.object(&target).borrow().base_value(), 9.0);
        assert!(Arc::ptr_eq(
            host.object(&target)
                .borrow()
                .modifier(&location("custom"))
                .unwrap(),
            &permanent
        ));
        assign_all_values(&0, &0, &mut host).unwrap();
        assert!(host.object(&source).borrow().modifiers().is_empty());
        assert!(host
            .object(&source)
            .borrow()
            .permanent_modifiers()
            .is_empty());
        assert_eq!(host.object(&source).borrow_mut().value(), 9.0);
        assert!(host
            .object(&target)
            .borrow()
            .has_modifier(&location("custom")));
    }

    #[test]
    fn copied_prefix_survives_actual_dirty_owner_failure() {
        let mut host = Host::new();
        let source = get_instance(&0, &key(1), &mut host).unwrap().unwrap();
        let target = get_instance(&1, &key(1), &mut host).unwrap().unwrap();
        host.maps[0].borrow_mut().clear_dirty(DirtySet::Update);
        host.maps[1].borrow_mut().clear_dirty(DirtySet::Update);
        host.set_base(&source, 17.0);
        host.fail_dirty_lookup = true;
        assert_eq!(
            assign_all_values(&1, &0, &mut host),
            Err(AttributeTransferError::Host(
                "dirty definition lookup failed"
            ))
        );
        assert_eq!(host.object(&target).borrow().base_value(), 17.0);
        assert!(host.object(&target).borrow().is_dirty());
        assert!(host.maps[1]
            .borrow()
            .dirty_contains(DirtySet::Update, &target));
    }
}
