//! Source-ordered Java 1.21 Mob constructor128–166, after Living returns.
//!
//! The caller already owns the same staged actor, Entity metadata/physical
//! state, complete Living constructor, and retained Level argument. Allocators
//! below receive the real native objects; none is a successful dummy callback.
//! This module neither constructs that superclass nor admits an actor to Main.

use super::{
    controls::{
        body_rotation::BodyRotationState, jump_control::JumpControlState,
        look_control::LookControlState, move_control::MoveControlState,
    },
    goals::GoalSelectorState,
    sensing::SensingState,
    state::{MobItemList, MobObjects, MobState, PathfindingMalus, RetainedMobControl},
};
use crate::living_entities::state::LivingObjects;

pub type ActorOf<H> = <<H as MobConstructionHost>::Objects as LivingObjects>::Actor;
type ItemOf<H> = <<H as MobConstructionHost>::Objects as LivingObjects>::ItemStack;
type PositionOf<H> = <<H as MobConstructionHost>::Objects as LivingObjects>::BlockPos;

/// Short canonical-field access and real source-object allocation/dispatch.
/// No field lock/borrow may survive into another host operation. Allocations
/// return the same object handles attached below; outward leases root their
/// original graph while internal self references avoid ownership cycles.
pub trait MobConstructionHost {
    type Error;
    type Objects: MobObjects;
    /// Supported constructor argument is a retained NONNULL actual Level.
    /// If an embedding admits a null token, its first profiler-supplier getter
    /// must fail at the source dereference; null cannot reach the late163 guard.
    type Level;
    type ProfilerSupplier;
    type WrappedGoal;

    fn with_mob_state<R>(
        &mut self,
        actor: &ActorOf<Self>,
        access: impl FnOnce(&mut MobState<Self::Objects>) -> R,
    ) -> Result<R, Self::Error>;
    /// Actual nonnull shared ItemStack.EMPTY, not a newly created empty value.
    /// Source reads it separately for hands, armor, and body.
    fn canonical_empty_stack(&mut self) -> Result<ItemOf<Self>, Self::Error>;
    /// Actual shared BlockPos.ZERO object, preserving its canonical identity.
    fn canonical_zero_block_position(&mut self) -> Result<PositionOf<Self>, Self::Error>;
    fn level_profiler_supplier(
        &mut self,
        level: &Self::Level,
    ) -> Result<Self::ProfilerSupplier, Self::Error>;
    fn allocate_goal_selector(
        &mut self,
        state: GoalSelectorState<Self::ProfilerSupplier, Self::WrappedGoal>,
    ) -> Result<<Self::Objects as MobObjects>::GoalSelector, Self::Error>;
    fn allocate_look_control(
        &mut self,
        object: RetainedMobControl<ActorOf<Self>, LookControlState>,
    ) -> Result<<Self::Objects as MobObjects>::LookControl, Self::Error>;
    fn allocate_move_control(
        &mut self,
        object: RetainedMobControl<ActorOf<Self>, MoveControlState>,
    ) -> Result<<Self::Objects as MobObjects>::MoveControl, Self::Error>;
    fn allocate_jump_control(
        &mut self,
        object: RetainedMobControl<ActorOf<Self>, JumpControlState>,
    ) -> Result<<Self::Objects as MobObjects>::JumpControl, Self::Error>;
    /// Actual virtual Mob.createBodyControl. The base body221 constructs
    /// RetainedMobControl { mob: actor.clone(), state: BodyRotationState::new() }.
    fn create_body_control(
        &mut self,
        actor: &ActorOf<Self>,
    ) -> Result<<Self::Objects as MobObjects>::BodyRotationControl, Self::Error>;
    /// Actual virtual Mob.createNavigation. For Bat, run construct_ground on
    /// this same actor and retained ctor Level, then return its actual handle.
    /// The Mob.navigation field is assigned only AFTER that call completes.
    fn create_navigation(
        &mut self,
        actor: &ActorOf<Self>,
        level: &Self::Level,
    ) -> Result<<Self::Objects as MobObjects>::Navigation, Self::Error>;
    fn allocate_sensing(
        &mut self,
        state: SensingState<ActorOf<Self>>,
    ) -> Result<<Self::Objects as MobObjects>::Sensing, Self::Error>;
    fn level_is_client_side(&mut self, level: &Self::Level) -> Result<bool, Self::Error>;
    /// Actual virtual dispatch, after every constructor attachment/chance fill.
    /// Base Mob169 is genuinely empty; Bat/AmbientCreature inherit that body.
    fn register_goals(&mut self, actor: &ActorOf<Self>) -> Result<(), Self::Error>;
}

/// Explicit malformed partial-owner failures, never a substitute empty array.
#[derive(Debug, PartialEq)]
pub enum MobConstructionError<E> {
    Host(E),
    NullArmorDropChances,
    NullHandDropChances,
}
impl<E> From<E> for MobConstructionError<E> {
    fn from(error: E) -> Self {
        Self::Host(error)
    }
}

/// Base virtual createBodyControl221 object. The host allocates/retains it;
/// constructing this object does not attach it prematurely to Mob.
pub fn base_body_control<A: Clone>(actor: &A) -> RetainedMobControl<A, BodyRotationState> {
    RetainedMobControl {
        mob: actor.clone(),
        state: BodyRotationState::new(),
    }
}

/// Source128–145 field initializers, after the COMPLETE Living constructor.
/// Every committed assignment remains observable if a later owner fails.
pub fn initialize_mob_fields<H: MobConstructionHost + ?Sized>(
    actor: &ActorOf<H>,
    host: &mut H,
) -> Result<(), H::Error> {
    let empty = host.canonical_empty_stack()?;
    host.with_mob_state(actor, |state| {
        state.hand_items = Some(MobItemList::new(empty))
    })?;
    host.with_mob_state(actor, |state| state.hand_drop_chances = Some([0.0; 2]))?;
    let empty = host.canonical_empty_stack()?;
    host.with_mob_state(actor, |state| {
        state.armor_items = Some(MobItemList::new(empty))
    })?;
    host.with_mob_state(actor, |state| state.armor_drop_chances = Some([0.0; 4]))?;
    let empty = host.canonical_empty_stack()?;
    host.with_mob_state(actor, |state| state.body_armor_item = Some(empty))?;
    host.with_mob_state(actor, |state| {
        state.pathfinding_malus = Some(PathfindingMalus::new())
    })?;
    let zero = host.canonical_zero_block_position()?;
    host.with_mob_state(actor, |state| state.restrict_center = Some(zero))?;
    host.with_mob_state(actor, |state| state.restrict_radius = -1.0)
}

/// Exact Mob body after Living returns. Two profiler-supplier reads are not
/// merged; virtual factories see source-prefix state, including zero chances.
/// A failure preserves all completed object assignments without publication or
/// rollback. Caller must not execute this again over an already initialized Mob.
/// `level` is the retained nonnull source constructor argument. The source late
/// null guard is unreachable for null under its earlier real supplier getters.
pub fn construct_mob_after_living<H: MobConstructionHost + ?Sized>(
    actor: &ActorOf<H>,
    level: &H::Level,
    host: &mut H,
) -> Result<(), MobConstructionError<H::Error>> {
    initialize_mob_fields(actor, host)?;
    let supplier = host.level_profiler_supplier(level)?;
    let selector = host.allocate_goal_selector(GoalSelectorState::new(supplier))?;
    host.with_mob_state(actor, |state| state.goal_selector = Some(selector))?;
    let supplier = host.level_profiler_supplier(level)?;
    let selector = host.allocate_goal_selector(GoalSelectorState::new(supplier))?;
    host.with_mob_state(actor, |state| state.target_selector = Some(selector))?;
    let control = host.allocate_look_control(RetainedMobControl {
        mob: actor.clone(),
        state: LookControlState::new(),
    })?;
    host.with_mob_state(actor, |state| state.look_control = Some(control))?;
    let control = host.allocate_move_control(RetainedMobControl {
        mob: actor.clone(),
        state: MoveControlState::new(),
    })?;
    host.with_mob_state(actor, |state| state.move_control = Some(control))?;
    let control = host.allocate_jump_control(RetainedMobControl {
        mob: actor.clone(),
        state: JumpControlState::new(),
    })?;
    host.with_mob_state(actor, |state| state.jump_control = Some(control))?;
    let control = host.create_body_control(actor)?;
    host.with_mob_state(actor, |state| state.body_rotation_control = Some(control))?;
    let navigation = host.create_navigation(actor, level)?;
    host.with_mob_state(actor, |state| state.navigation = Some(navigation))?;
    let sensing = host.allocate_sensing(SensingState::new(actor.clone()))?;
    host.with_mob_state(actor, |state| state.sensing = Some(sensing))?;
    let present = host.with_mob_state(actor, |state| {
        state
            .armor_drop_chances
            .as_mut()
            .map(|chances| chances.fill(0.085_f32))
    })?;
    if present.is_none() {
        return Err(MobConstructionError::NullArmorDropChances);
    }
    let present = host.with_mob_state(actor, |state| {
        state
            .hand_drop_chances
            .as_mut()
            .map(|chances| chances.fill(0.085_f32))
    })?;
    if present.is_none() {
        return Err(MobConstructionError::NullHandDropChances);
    }
    host.with_mob_state(actor, |state| state.body_armor_drop_chance = 0.085_f32)?;
    if !host.level_is_client_side(level)? {
        host.register_goals(actor)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{effects::attribute_map::JavaObjectKey, equipment::EquipmentSlot};
    use std::{cell::RefCell, rc::Rc};

    // Controlled allocation/callback observations, not a production actor,
    // Level, Brain, canonical item service, navigation or Main admission.
    type Actor = Rc<()>;
    type Selector = Rc<GoalSelectorState<u32, ()>>;
    struct Objects;
    #[derive(Clone, PartialEq, Eq)]
    struct Key;
    impl JavaObjectKey for Key {
        fn java_hash_code(&self) -> i32 {
            0
        }
    }
    impl LivingObjects for Objects {
        type Actor = Actor;
        type ItemStack = Rc<u32>;
        type BlockPos = Rc<[i32; 3]>;
        type DamageSource = ();
        type FallLocation = ();
        type AttributeHolder = Key;
        type AttributeInstance = Key;
        type AttributeSupplier = ();
        type ActiveEffectsMap = ();
        type Brain = ();
        type LocationEnchantmentsMap = ();
    }
    impl MobObjects for Objects {
        type GoalSelector = Selector;
        type LookControl = Rc<RetainedMobControl<Actor, LookControlState>>;
        type MoveControl = Rc<RetainedMobControl<Actor, MoveControlState>>;
        type JumpControl = Rc<RetainedMobControl<Actor, JumpControlState>>;
        type BodyRotationControl = Rc<RetainedMobControl<Actor, BodyRotationState>>;
        type Navigation = Rc<()>;
        type Sensing = Rc<RefCell<SensingState<Actor>>>;
        type LootTable = Rc<str>;
        type LeashData = ();
    }
    struct Host {
        actor: Actor,
        state: MobState<Objects>,
        empty: Rc<u32>,
        zero: Rc<[i32; 3]>,
        supplier_reads: u32,
        empty_reads: u32,
        trace: Vec<&'static str>,
        fail_navigation: bool,
        fail_second_empty: bool,
        client: bool,
    }
    impl Host {
        fn new() -> Self {
            Self {
                actor: Rc::new(()),
                state: MobState::jvm_zeroed(),
                empty: Rc::new(0),
                zero: Rc::new([0; 3]),
                supplier_reads: 0,
                empty_reads: 0,
                trace: Vec::new(),
                fail_navigation: false,
                fail_second_empty: false,
                client: false,
            }
        }
        fn assert_actor(&self, actor: &Actor) {
            assert!(Rc::ptr_eq(actor, &self.actor));
        }
    }
    impl MobConstructionHost for Host {
        type Error = &'static str;
        type Objects = Objects;
        type Level = ();
        type ProfilerSupplier = u32;
        type WrappedGoal = ();
        fn with_mob_state<R>(
            &mut self,
            actor: &Actor,
            access: impl FnOnce(&mut MobState<Objects>) -> R,
        ) -> Result<R, Self::Error> {
            self.assert_actor(actor);
            let armor_before = self.state.armor_drop_chances;
            let hands_before = self.state.hand_drop_chances;
            let body_before = self.state.body_armor_drop_chance;
            let result = access(&mut self.state);
            if armor_before == Some([0.0; 4]) && self.state.armor_drop_chances == Some([0.085; 4]) {
                self.trace.push("fill.armor");
            }
            if hands_before == Some([0.0; 2]) && self.state.hand_drop_chances == Some([0.085; 2]) {
                self.trace.push("fill.hands");
            }
            if body_before == 0.0 && self.state.body_armor_drop_chance == 0.085 {
                self.trace.push("fill.body");
            }
            Ok(result)
        }
        fn canonical_empty_stack(&mut self) -> Result<Rc<u32>, Self::Error> {
            self.empty_reads += 1;
            self.trace.push("EMPTY");
            if self.fail_second_empty && self.empty_reads == 2 {
                return Err("missing EMPTY");
            }
            Ok(self.empty.clone())
        }
        fn canonical_zero_block_position(&mut self) -> Result<Rc<[i32; 3]>, Self::Error> {
            self.trace.push("ZERO");
            Ok(self.zero.clone())
        }
        fn level_profiler_supplier(&mut self, _: &()) -> Result<u32, Self::Error> {
            self.trace.push("supplier");
            self.supplier_reads += 1;
            Ok(self.supplier_reads)
        }
        fn allocate_goal_selector(
            &mut self,
            state: GoalSelectorState<u32, ()>,
        ) -> Result<Selector, Self::Error> {
            assert_eq!(state.available_goal_count(), 0);
            assert_eq!(state.locked_flag_count(), 0);
            assert_eq!(state.initial_linked_set_layout().key_slots, 33);
            if self.supplier_reads == 1 {
                assert!(self.state.goal_selector.is_none());
            } else {
                assert!(self.state.goal_selector.is_some());
                assert!(self.state.target_selector.is_none());
            }
            self.trace.push("selector");
            Ok(Rc::new(state))
        }
        fn allocate_look_control(
            &mut self,
            object: RetainedMobControl<Actor, LookControlState>,
        ) -> Result<<Objects as MobObjects>::LookControl, Self::Error> {
            self.assert_actor(&object.mob);
            assert!(self.state.target_selector.is_some());
            self.trace.push("look");
            Ok(Rc::new(object))
        }
        fn allocate_move_control(
            &mut self,
            object: RetainedMobControl<Actor, MoveControlState>,
        ) -> Result<<Objects as MobObjects>::MoveControl, Self::Error> {
            self.assert_actor(&object.mob);
            assert!(self.state.look_control.is_some());
            assert_eq!(
                object.state.operation(),
                super::super::controls::move_control::Operation::Wait
            );
            self.trace.push("move");
            Ok(Rc::new(object))
        }
        fn allocate_jump_control(
            &mut self,
            object: RetainedMobControl<Actor, JumpControlState>,
        ) -> Result<<Objects as MobObjects>::JumpControl, Self::Error> {
            self.assert_actor(&object.mob);
            assert!(self.state.move_control.is_some());
            self.trace.push("jump");
            Ok(Rc::new(object))
        }
        fn create_body_control(
            &mut self,
            actor: &Actor,
        ) -> Result<<Objects as MobObjects>::BodyRotationControl, Self::Error> {
            self.assert_actor(actor);
            assert!(self.state.jump_control.is_some());
            self.trace.push("body");
            Ok(Rc::new(base_body_control(actor)))
        }
        fn create_navigation(&mut self, actor: &Actor, _: &()) -> Result<Rc<()>, Self::Error> {
            self.assert_actor(actor);
            self.trace.push("navigation");
            assert!(self.state.body_rotation_control.is_some());
            assert!(self.state.navigation.is_none());
            assert!(self.state.sensing.is_none());
            assert_eq!(self.state.armor_drop_chances, Some([0.0; 4]));
            assert_eq!(self.state.hand_drop_chances, Some([0.0; 2]));
            assert_eq!(self.state.body_armor_drop_chance.to_bits(), 0);
            if self.fail_navigation {
                return Err("navigation failure");
            }
            Ok(Rc::new(()))
        }
        fn allocate_sensing(
            &mut self,
            state: SensingState<Actor>,
        ) -> Result<<Objects as MobObjects>::Sensing, Self::Error> {
            assert!(self.state.navigation.is_some());
            self.trace.push("sensing");
            Ok(Rc::new(RefCell::new(state)))
        }
        fn level_is_client_side(&mut self, _: &()) -> Result<bool, Self::Error> {
            self.trace.push("client");
            Ok(self.client)
        }
        fn register_goals(&mut self, actor: &Actor) -> Result<(), Self::Error> {
            self.assert_actor(actor);
            assert!(self.state.sensing.is_some());
            for slot in EquipmentSlot::ALL {
                assert_eq!(self.state.equipment_drop_chance(slot), Some(0.085));
            }
            self.trace.push("goals");
            Ok(())
        }
    }

    #[test]
    fn factories_observe_actual_attachment_prefix_before_chance_fills() {
        let mut host = Host::new();
        let actor = host.actor.clone();
        construct_mob_after_living(&actor, &(), &mut host).unwrap();
        assert_eq!(
            host.trace,
            [
                "EMPTY",
                "EMPTY",
                "EMPTY",
                "ZERO",
                "supplier",
                "selector",
                "supplier",
                "selector",
                "look",
                "move",
                "jump",
                "body",
                "navigation",
                "sensing",
                "fill.armor",
                "fill.hands",
                "fill.body",
                "client",
                "goals"
            ]
        );
        assert_eq!(host.supplier_reads, 2);
        assert!(!Rc::ptr_eq(
            host.state.goal_selector.as_ref().unwrap(),
            host.state.target_selector.as_ref().unwrap()
        ));
        for slot in EquipmentSlot::ALL {
            assert!(Rc::ptr_eq(
                host.state.item_by_slot(slot).unwrap(),
                &host.empty
            ));
        }
        assert!(Rc::ptr_eq(
            host.state.restrict_center.as_ref().unwrap(),
            &host.zero
        ));
        assert_eq!(host.state.restrict_radius, -1.0);
        assert!(host.state.pathfinding_malus.as_ref().unwrap().is_empty());
        assert_eq!(host.state.xp_reward, 0);
        assert!(host.state.target.is_none());
        assert!(!host.state.can_pick_up_loot);
        assert!(!host.state.persistence_required);
        assert!(host.state.loot_table.is_none());
        assert_eq!(host.state.loot_table_seed, 0);
        let stack = Rc::new(42);
        let previous = host
            .state
            .replace_item_by_slot(EquipmentSlot::Head, stack.clone())
            .unwrap();
        assert!(Rc::ptr_eq(&previous, &host.empty));
        assert!(Rc::ptr_eq(
            host.state.item_by_slot(EquipmentSlot::Head).unwrap(),
            &stack
        ));
        host.state.armor_items.as_mut().unwrap().clear();
        assert!(Rc::ptr_eq(
            host.state.item_by_slot(EquipmentSlot::Head).unwrap(),
            &host.empty
        ));
    }

    #[test]
    fn constructor_errors_keep_completed_fields_and_client_skips_only_goal_registration() {
        let mut host = Host::new();
        let actor = host.actor.clone();
        host.fail_navigation = true;
        assert_eq!(
            construct_mob_after_living(&actor, &(), &mut host),
            Err(MobConstructionError::Host("navigation failure"))
        );
        assert!(host.state.body_rotation_control.is_some());
        assert!(host.state.navigation.is_none());
        assert!(host.state.sensing.is_none());
        assert_eq!(host.state.hand_drop_chances, Some([0.0; 2]));
        assert_eq!(host.trace.last(), Some(&"navigation"));
        let mut host = Host::new();
        let actor = host.actor.clone();
        host.fail_second_empty = true;
        assert_eq!(
            construct_mob_after_living(&actor, &(), &mut host),
            Err(MobConstructionError::Host("missing EMPTY"))
        );
        assert!(host.state.hand_items.is_some());
        assert!(host.state.hand_drop_chances.is_some());
        assert!(host.state.armor_items.is_none());
        assert!(host.state.body_armor_item.is_none());
        assert_eq!(host.supplier_reads, 0);
        let mut host = Host::new();
        let actor = host.actor.clone();
        host.client = true;
        construct_mob_after_living(&actor, &(), &mut host).unwrap();
        assert!(host.state.sensing.is_some());
        assert_eq!(host.trace.last(), Some(&"client"));
        assert!(host.trace.contains(&"fill.body"));
    }

    #[test]
    fn closed_enum_map_retains_raw_override_without_populating_other_defaults() {
        use super::super::state::{PathType, PathfindingMalus};
        let mut map = PathfindingMalus::new();
        assert!(map.is_empty());
        assert_eq!(map.get_or_default(PathType::Water), 8.0);
        assert_eq!(map.get_or_default(PathType::DamageFire), 16.0);
        let raw_nan = f32::from_bits(0x7fc01234);
        assert_eq!(map.put(PathType::Water, raw_nan), None);
        assert_eq!(
            map.get_or_default(PathType::Water).to_bits(),
            raw_nan.to_bits()
        );
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(PathType::DamageFire), None);
        for (ordinal, kind) in PathType::ALL.into_iter().enumerate() {
            assert_eq!(kind as usize, ordinal);
        }
    }
}
