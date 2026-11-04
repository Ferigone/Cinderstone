//! Composed native post-Entity constructor fixtures, not Main/world admission.
//!
//! Canonical reference objects, their supplied hashes, EMPTY, Level and the
//! three global Math inputs are controlled external bindings. Attribute values,
//! bound definitions, templates/supplier, maps, Brain, selectors, controls and
//! navigation objects run their actual native implementations. The Entity
//! prefix is explicitly fixture-asserted; no Java Entity constructor is faked.

use super::actor::{
    self, BatActorError, BatConstructionError, BatDefaultAttributeSupplier, BatEntityFields,
    NativeBatBackref, NativeBatLease, NativeBatServices, NativeBatTypes,
};
use crate::{
    effects::{
        active_map::ActiveEffectKey,
        attribute_map::JavaObjectKey,
        binding::{AttributeBindingOwner, NativeAttributeBinding, NativeAttributeMap},
        default_attributes::{self, LivingAttributeHolders, MobAttributeHolders},
        definitions::{
            AttributeValue, BoundAttribute, RegisteredAttributeOwner, BUILTIN_ATTRIBUTE_SPECS,
        },
        location_map::{EnchantmentValueReference, LocationEffectReference},
        supplier::RegisteredAttributeHolder,
    },
    entity::Position,
    entity_state::EntityPhysicalState,
    geometry::{Aabb, Vec3},
    living_entities::{brain::DefaultBrainProvider, metadata},
    metadata::MetadataValue,
    random::LegacyRandomSource,
};
use std::{collections::VecDeque, sync::Arc};

struct HolderObject {
    name: &'static str,
    supplied_hash: i32,
    value: Arc<AttributeValue>,
}
#[derive(Clone)]
struct Holder(Arc<HolderObject>);
impl PartialEq for Holder {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Holder {}
impl JavaObjectKey for Holder {
    fn java_hash_code(&self) -> i32 {
        self.0.supplied_hash
    }
}
impl RegisteredAttributeHolder for Holder {}

#[derive(Clone)]
struct Identity(Arc<i32>);
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Identity {}
impl JavaObjectKey for Identity {
    fn java_hash_code(&self) -> i32 {
        *self.0
    }
}

// No effect, enchantment value, loot reference or damage source is fabricated
// for this constructor. Their actual empty tables are still allocated natively.
#[derive(Clone)]
enum UnusedReference {}
impl ActiveEffectKey for UnusedReference {
    fn is_canonical_reference(&self) -> bool {
        match *self {}
    }
    fn java_hash_code(&self) -> i32 {
        match *self {}
    }
    fn same_object(&self, _: &Self) -> bool {
        match *self {}
    }
}
impl EnchantmentValueReference for UnusedReference {
    fn same_object(&self, _: &Self) -> bool {
        match *self {}
    }
}
impl LocationEffectReference for UnusedReference {
    fn same_object(&self, _: &Self) -> bool {
        match *self {}
    }
    fn java_equals(&self, _: &Self) -> bool {
        match *self {}
    }
}

struct ControlledEmptyStack;
struct ControlledLevel {
    client: bool,
}
struct FixtureTypes;
impl NativeBatTypes for FixtureTypes {
    type ItemStack = Arc<ControlledEmptyStack>;
    type DamageSource = UnusedReference;
    type FallLocation = UnusedReference;
    type AttributeHolder = Holder;
    type InstanceIdentity = Identity;
    type EffectHolder = UnusedReference;
    type EnchantmentValue = UnusedReference;
    type LocationEffect = UnusedReference;
    type EntityType = Arc<()>;
    type Level = Arc<ControlledLevel>;
    type ProfilerSupplier = Arc<()>;
    type LootTable = UnusedReference;
    type LeashData = UnusedReference;
}

#[derive(Debug, PartialEq)]
struct MathObservation {
    rot_a: u32,
    time_offset: u32,
    bounds: Aabb,
    health: f32,
    has_brain: bool,
    has_look_control: bool,
}

struct Fixture {
    holders: Vec<Holder>,
    definitions: Vec<Arc<BoundAttribute<Holder>>>,
    tokens: VecDeque<Identity>,
    allocated_tokens: usize,
    supplier: Option<BatDefaultAttributeSupplier<FixtureTypes>>,
    empty: Arc<ControlledEmptyStack>,
    zero: Arc<(i32, i32, i32)>,
    original_type: Arc<()>,
    original_level: Arc<ControlledLevel>,
    replacement_level: Arc<ControlledLevel>,
    profiler: Arc<()>,
    actor: Option<NativeBatLease<FixtureTypes>>,
    draws: VecDeque<f64>,
    consumed_draws: usize,
    observations: Vec<MathObservation>,
    trace: Vec<&'static str>,
    fail_second_draw: bool,
}
impl Fixture {
    fn holder(&self, name: &str) -> Holder {
        self.holders
            .iter()
            .find(|h| h.0.name == name)
            .unwrap()
            .clone()
    }
    fn mob_holders(&self) -> MobAttributeHolders<Holder> {
        let h = |name| self.holder(name);
        MobAttributeHolders {
            living: LivingAttributeHolders {
                max_health: h("minecraft:generic.max_health"),
                knockback_resistance: h("minecraft:generic.knockback_resistance"),
                movement_speed: h("minecraft:generic.movement_speed"),
                armor: h("minecraft:generic.armor"),
                armor_toughness: h("minecraft:generic.armor_toughness"),
                max_absorption: h("minecraft:generic.max_absorption"),
                step_height: h("minecraft:generic.step_height"),
                scale: h("minecraft:generic.scale"),
                gravity: h("minecraft:generic.gravity"),
                safe_fall_distance: h("minecraft:generic.safe_fall_distance"),
                fall_damage_multiplier: h("minecraft:generic.fall_damage_multiplier"),
                jump_strength: h("minecraft:generic.jump_strength"),
                oxygen_bonus: h("minecraft:generic.oxygen_bonus"),
                burning_time: h("minecraft:generic.burning_time"),
                explosion_knockback_resistance: h(
                    "minecraft:generic.explosion_knockback_resistance",
                ),
                water_movement_efficiency: h("minecraft:generic.water_movement_efficiency"),
                movement_efficiency: h("minecraft:generic.movement_efficiency"),
                attack_knockback: h("minecraft:generic.attack_knockback"),
            },
            follow_range: h("minecraft:generic.follow_range"),
        }
    }
    fn new() -> Self {
        let holders = BUILTIN_ATTRIBUTE_SPECS
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                Holder(Arc::new(HolderObject {
                    name: spec.name,
                    supplied_hash: index as i32 * 137 + 3,
                    value: Arc::new(spec.create_value().unwrap()),
                }))
            })
            .collect();
        let mut fixture = Self {
            holders,
            definitions: Vec::new(),
            // Explicit supplied test hashes, not a production hash allocator.
            tokens: (0..32)
                .map(|index| Identity(Arc::new(index * 71 + 17)))
                .collect(),
            allocated_tokens: 0,
            supplier: None,
            empty: Arc::new(ControlledEmptyStack),
            zero: Arc::new((0, 0, 0)),
            original_type: Arc::new(()),
            original_level: Arc::new(ControlledLevel { client: false }),
            replacement_level: Arc::new(ControlledLevel { client: true }),
            profiler: Arc::new(()),
            actor: None,
            draws: VecDeque::from([0.125, 0.375, 0.625]),
            consumed_draws: 0,
            observations: Vec::new(),
            trace: Vec::new(),
            fail_second_draw: false,
        };
        for holder in fixture.holders.clone() {
            let definition = BoundAttribute::from_registered(holder, &mut fixture).unwrap();
            fixture.definitions.push(Arc::new(definition));
        }
        let holders = fixture.mob_holders();
        let mut binding = NativeAttributeBinding::for_defaults(&mut fixture);
        let supplier = default_attributes::build_bat_supplier(&holders, &mut binding).unwrap();
        fixture.supplier = Some(supplier);
        fixture.trace.clear();
        fixture
    }
    fn staged_actor(&mut self) -> NativeBatLease<FixtureTypes> {
        let mut physical = EntityPhysicalState::new(
            71,
            [3; 16],
            Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: false,
            },
            Vec3::ZERO,
            0.5,
            0.9,
            0.45,
        )
        .unwrap();
        // The source reapplyPosition must actually rebuild this box between
        // the first and second Math calls, even when setPosRaw sees no move.
        physical.bounds = Aabb::new(90.0, 91.0, 92.0, 93.0, 94.0, 95.0);
        let actor = NativeBatLease::attach_to_entity_prefix(
            physical,
            BatEntityFields::jvm_zeroed(),
            LegacyRandomSource::new(0x1234),
            Arc::clone(&self.original_type),
            Arc::clone(&self.original_level),
        );
        actor.install_defined_synched_data().unwrap();
        // This asserts a controlled Entity prefix ONLY. The actual post-Entity
        // tail below runs natively; this test supplies no Entity constructor.
        actor.finish_entity_constructor().unwrap();
        // Model a current-world replacement before the tail. Source constructor
        // locals must still retain the ORIGINAL Level for Mob and Bat checks.
        actor
            .with_fields(|fields| fields.level = Arc::clone(&self.replacement_level))
            .unwrap();
        self.actor = Some(actor.clone());
        actor
    }
}

impl RegisteredAttributeOwner for Fixture {
    type Holder = Holder;
    type Error = &'static str;
    fn registered_attribute_value(
        &mut self,
        holder: &Holder,
    ) -> Result<Arc<AttributeValue>, Self::Error> {
        Ok(Arc::clone(&holder.0.value))
    }
}
impl AttributeBindingOwner for Fixture {
    type Error = &'static str;
    type Holder = Holder;
    type Identity = Identity;
    type Map = NativeBatBackref<FixtureTypes>;
    type Actor = NativeBatLease<FixtureTypes>;
    fn holder_hash_code(&mut self, holder: &Holder) -> Result<i32, Self::Error> {
        Ok(holder.java_hash_code())
    }
    fn registered_name(&mut self, holder: &Holder) -> Result<String, Self::Error> {
        Ok(holder.0.name.into())
    }
    fn definition(&mut self, holder: &Holder) -> Result<Arc<BoundAttribute<Holder>>, Self::Error> {
        self.definitions
            .iter()
            .find(|d| d.holder() == holder)
            .cloned()
            .ok_or("unknown fixture holder")
    }
    fn new_instance_identity(&mut self) -> Result<Identity, Self::Error> {
        self.allocated_tokens += 1;
        self.tokens
            .pop_front()
            .ok_or("no supplied fixture identity")
    }
    fn with_map<R>(
        &mut self,
        _: &Self::Map,
        _: impl FnOnce(&mut NativeAttributeMap<Holder, Identity, Self::Map>) -> R,
    ) -> Result<R, Self::Error> {
        Err("default-supplier bootstrap must not access an actor map")
    }
    fn on_attribute_updated(&mut self, _: &Self::Actor, _: &Holder) -> Result<(), Self::Error> {
        Err("default-supplier bootstrap must not call an actor")
    }
}

impl NativeBatServices<FixtureTypes> for Fixture {
    type Error = &'static str;
    fn canonical_empty_stack(&mut self) -> Result<Arc<ControlledEmptyStack>, Self::Error> {
        self.trace.push("EMPTY");
        Ok(Arc::clone(&self.empty))
    }
    fn canonical_zero_block_position(&mut self) -> Result<Arc<(i32, i32, i32)>, Self::Error> {
        self.trace.push("ZERO");
        Ok(Arc::clone(&self.zero))
    }
    fn default_attribute_supplier(
        &mut self,
        entity_type: &Arc<()>,
    ) -> Result<BatDefaultAttributeSupplier<FixtureTypes>, Self::Error> {
        assert!(Arc::ptr_eq(entity_type, &self.original_type));
        self.trace.push("defaults");
        Ok(self.supplier.as_ref().unwrap().clone())
    }
    fn max_health_holder(&mut self) -> Result<Holder, Self::Error> {
        self.trace.push("MAX_HEALTH");
        Ok(self.holder("minecraft:generic.max_health"))
    }
    fn follow_range_holder(&mut self) -> Result<Holder, Self::Error> {
        self.trace.push("FOLLOW_RANGE");
        Ok(self.holder("minecraft:generic.follow_range"))
    }
    fn holder_hash_code(&mut self, holder: &Holder) -> Result<i32, Self::Error> {
        AttributeBindingOwner::holder_hash_code(self, holder)
    }
    fn registered_attribute_name(&mut self, holder: &Holder) -> Result<String, Self::Error> {
        AttributeBindingOwner::registered_name(self, holder)
    }
    fn attribute_definition(
        &mut self,
        holder: &Holder,
    ) -> Result<Arc<BoundAttribute<Holder>>, Self::Error> {
        AttributeBindingOwner::definition(self, holder)
    }
    fn new_instance_identity(&mut self) -> Result<Identity, Self::Error> {
        self.trace.push("unexpected peractor attribute allocation");
        Err("constructor reads supplier values without lazy instance allocation")
    }
    fn next_global_math_double(&mut self) -> Result<f64, Self::Error> {
        let actor = self.actor.as_ref().unwrap().clone();
        let health =
            actor::get_synched_data(&actor.metadata_lease(), metadata::DATA_HEALTH_ID, self)
                .map_err(|_| "health observation failed")?;
        let MetadataValue::Float(health) = health else {
            return Err("health not Float");
        };
        let observation = actor
            .with_fields(|f| MathObservation {
                rot_a: f.living.rot_a.to_bits(),
                time_offset: f.living.time_offset.to_bits(),
                bounds: f.physical.bounds,
                health,
                has_brain: f.living.brain.is_some(),
                has_look_control: f.mob.look_control.is_some(),
            })
            .unwrap();
        self.observations.push(observation);
        if self.consumed_draws == 1 && self.fail_second_draw {
            self.trace.push("Math2 unavailable");
            return Err("second global Math owner unavailable");
        }
        self.trace
            .push(["Math1", "Math2", "Math3"][self.consumed_draws]);
        self.consumed_draws += 1;
        self.draws
            .pop_front()
            .ok_or("no controlled global Math input")
    }
    fn level_profiler_supplier(
        &mut self,
        level: &Arc<ControlledLevel>,
    ) -> Result<Arc<()>, Self::Error> {
        assert!(Arc::ptr_eq(level, &self.original_level));
        self.trace.push("profiler");
        Ok(Arc::clone(&self.profiler))
    }
    fn level_is_client_side(&mut self, level: &Arc<ControlledLevel>) -> Result<bool, Self::Error> {
        assert!(Arc::ptr_eq(level, &self.original_level));
        self.trace.push("ctorLevel.client");
        Ok(level.client)
    }
    fn position_changed(&mut self, _: &NativeBatLease<FixtureTypes>) -> Result<(), Self::Error> {
        Err("same-position reapply must not call EntityInLevelCallback.onMove")
    }
    fn refresh_dimensions(&mut self, _: &NativeBatLease<FixtureTypes>) -> Result<(), Self::Error> {
        Err("health/Bat flag changes do not refresh dimensions")
    }
    fn on_attribute_updated(
        &mut self,
        _: &NativeBatLease<FixtureTypes>,
        _: &Holder,
    ) -> Result<(), Self::Error> {
        Err("constructor fallback queries must not instantiate/dirty an attribute")
    }
}

#[test]
fn actual_composed_living_mob_bat_constructor_owns_real_state_and_source_order() {
    let mut fixture = Fixture::new();
    let templates = fixture.allocated_tokens;
    let actor = fixture.staged_actor();
    let random_before = actor.with_entity_random(|rng| rng.state48()).unwrap();
    actor::construct_after_entity(&actor, &mut fixture).unwrap();
    assert!(actor.is_constructor_complete().unwrap());
    assert_eq!(fixture.allocated_tokens, templates);
    assert_eq!(
        actor.with_entity_random(|rng| rng.state48()).unwrap(),
        random_before
    );
    assert_eq!(
        fixture.trace,
        [
            "EMPTY",
            "EMPTY",
            "EMPTY",
            "EMPTY",
            "defaults",
            "MAX_HEALTH",
            "MAX_HEALTH",
            "Math1",
            "Math2",
            "Math3",
            "EMPTY",
            "EMPTY",
            "EMPTY",
            "ZERO",
            "profiler",
            "profiler",
            "FOLLOW_RANGE",
            "ctorLevel.client",
            "ctorLevel.client",
        ]
    );
    assert_eq!(fixture.consumed_draws, 3);
    assert_eq!(fixture.observations[0].rot_a, 0);
    assert_eq!(
        fixture.observations[0].bounds,
        Aabb::new(90.0, 91.0, 92.0, 93.0, 94.0, 95.0)
    );
    assert_eq!(fixture.observations[1].rot_a, 0x3c3851eb);
    assert_eq!(fixture.observations[1].time_offset, 0);
    assert_ne!(
        fixture.observations[1].bounds,
        fixture.observations[0].bounds
    );
    assert_eq!(fixture.observations[2].time_offset, 0x45914a00);
    assert!(fixture
        .observations
        .iter()
        .all(|o| o.health == 6.0 && !o.has_brain && !o.has_look_control));
    assert_eq!(
        actor::get_synched_data(
            &actor.metadata_lease(),
            metadata::DATA_HEALTH_ID,
            &mut fixture
        )
        .unwrap(),
        MetadataValue::Float(6.0)
    );
    assert_eq!(
        actor::get_synched_data(
            &actor.metadata_lease(),
            metadata::DATA_BAT_FLAGS_ID,
            &mut fixture
        )
        .unwrap(),
        MetadataValue::Byte(1)
    );
    actor
        .with_fields(|f| {
            assert!(Arc::ptr_eq(&f.level, &fixture.replacement_level));
            assert!(f.entity.blocks_building);
            assert_eq!(f.living.rot_a.to_bits(), 0x3c3851eb);
            assert_eq!(f.living.time_offset.to_bits(), 0x45914a00);
            assert_eq!(f.physical.position.yaw.to_bits(), 0x407b53d2);
            assert_eq!(f.living.head_yaw.to_bits(), 0x407b53d2);
            assert_eq!(f.living.body_yaw.to_bits(), 0);
            assert_eq!(f.living.attributes.as_ref().unwrap().instantiated_len(), 0);
            let brain = f.living.brain.as_ref().unwrap();
            assert_eq!(
                brain.serialize_start_nbt(),
                DefaultBrainProvider::constructor_memories_nbt()
            );
            assert_eq!(brain.state_view().active_activities, 0);
            assert_eq!(brain.state_view().last_schedule_update, -9999);
            assert_eq!(
                f.living
                    .active_effects
                    .as_ref()
                    .unwrap()
                    .state()
                    .unwrap()
                    .size,
                0
            );
            assert!(f
                .living
                .active_location_enchantments
                .as_ref()
                .unwrap()
                .is_empty()
                .unwrap());
            assert!(f.living.combat_tracker.is_some());
            for selector in [&f.mob.goal_selector, &f.mob.target_selector] {
                let selector = selector.as_ref().unwrap().lock().unwrap();
                assert_eq!(selector.available_goal_count(), 0);
                assert_eq!(selector.locked_flag_count(), 0);
            }
            assert!(!Arc::ptr_eq(
                f.mob.goal_selector.as_ref().unwrap(),
                f.mob.target_selector.as_ref().unwrap()
            ));
            for item in f
                .mob
                .hand_items
                .as_ref()
                .unwrap()
                .items()
                .iter()
                .chain(f.mob.armor_items.as_ref().unwrap().items())
            {
                assert!(Arc::ptr_eq(item, &fixture.empty));
            }
            assert!(Arc::ptr_eq(
                f.mob.body_armor_item.as_ref().unwrap(),
                &fixture.empty
            ));
            assert_eq!(f.mob.hand_drop_chances, Some([0.085; 2]));
            assert_eq!(f.mob.armor_drop_chances, Some([0.085; 4]));
            assert_eq!(f.mob.body_armor_drop_chance, 0.085);
            assert!(
                f.mob.look_control.is_some()
                    && f.mob.move_control.is_some()
                    && f.mob.jump_control.is_some()
                    && f.mob.body_rotation_control.is_some()
                    && f.mob.sensing.is_some()
            );
            let nav = f.mob.navigation.as_ref().unwrap().lock().unwrap();
            assert!(Arc::ptr_eq(nav.level(), &fixture.original_level));
            let layout = nav.constructor_layout();
            assert!(!layout.has_path && layout.has_evaluator && layout.has_path_finder);
            let evaluator = nav.node_evaluator().unwrap().lock().unwrap();
            assert!(evaluator.constructor_layout().can_pass_doors);
            let finder = nav.path_finder().unwrap().lock().unwrap();
            assert_eq!(finder.max_visited_nodes(), 256);
            assert!(Arc::ptr_eq(
                nav.node_evaluator().unwrap(),
                finder.evaluator()
            ));
            let bat = f.bat.as_ref().unwrap();
            assert!(
                !bat.fly_animation_state().is_started() && !bat.rest_animation_state().is_started()
            );
        })
        .unwrap();
}

#[test]
fn unavailable_second_math_input_keeps_actual_completed_constructor_prefix() {
    let mut fixture = Fixture::new();
    fixture.fail_second_draw = true;
    let actor = fixture.staged_actor();
    let random_before = actor.with_entity_random(|rng| rng.state48()).unwrap();
    assert!(matches!(
        actor::construct_after_entity(&actor, &mut fixture),
        Err(BatConstructionError::Services(
            "second global Math owner unavailable"
        ))
    ));
    assert!(!actor.is_constructor_complete().unwrap());
    assert_eq!(fixture.consumed_draws, 1);
    assert_eq!(
        actor.with_entity_random(|rng| rng.state48()).unwrap(),
        random_before
    );
    actor
        .with_fields(|f| {
            assert!(f.entity.blocks_building);
            assert_eq!(f.living.rot_a.to_bits(), 0x3c3851eb);
            assert_eq!(f.living.time_offset.to_bits(), 0);
            assert_eq!(f.physical.bounds, fixture.observations[1].bounds);
            assert_ne!(f.physical.bounds, fixture.observations[0].bounds);
            assert!(f.living.attributes.is_some() && f.living.combat_tracker.is_some());
            assert!(f.living.brain.is_none() && f.mob.look_control.is_none() && f.bat.is_none());
        })
        .unwrap();
    let consumed = fixture.consumed_draws;
    assert!(matches!(
        actor::construct_after_entity(&actor, &mut fixture),
        Err(BatConstructionError::Actor(
            BatActorError::ConstructorAlreadyStarted
        ))
    ));
    assert_eq!(fixture.consumed_draws, consumed);
}
