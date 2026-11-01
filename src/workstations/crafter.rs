//! Java 1.21 CrafterBlockEntity state and CrafterBlock automatic-craft rules.
//!
//! The nine inputs belong to the actual block entity. Recipe/cache selection,
//! adjacent containers, item/map callbacks, RNG/entity construction, nearby
//! players and world writes must be borrowed from the real staged runtime.
use crate::{crafting::RecipeInput, geometry::Direction, inventory::ItemStack};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub const MENU_TYPE: i32 = 7;
pub const INPUT_SLOTS: usize = 9;
pub const PREVIEW_SLOT: usize = 45;
pub const MENU_SLOTS: usize = 46;
pub const DATA_SLOTS: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrafterState {
    inputs: [ItemStack; 9],
    disabled: [bool; 9],
    triggered: i32,
    crafting_ticks_remaining: i32,
    #[serde(skip)]
    revision: u64,
}
impl Default for CrafterState {
    fn default() -> Self {
        Self {
            inputs: std::array::from_fn(|_| ItemStack::empty()),
            disabled: [false; 9],
            triggered: 0,
            crafting_ticks_remaining: 0,
            revision: 0,
        }
    }
}

/// Supplied by the actual captured block-entity owner, including its lifetime
/// across same-position replacement. There is no synthesized/default identity.
pub trait CrafterOwnerIdentity {
    type Key: Clone + Eq + Debug;
    fn owner_key(&self) -> &Self::Key;
}
/// Implement with the world's shared ten-entry CrafterBlock RecipeCache and
/// its real RecipeManager/registry snapshot. A hit returns its captured holder
/// without re-running matches. Unknown/unavailable owners must return errors.
/// Remaining items are requested only after the main output has dispensed.
pub trait CrafterRecipes {
    fn potential_recipe(&mut self, input: &RecipeInput) -> Result<Option<String>>;
    fn assemble(&mut self, recipe: &str, input: &RecipeInput) -> Result<ItemStack>;
    fn remaining_items(&mut self, recipe: &str, input: &RecipeInput) -> Result<Vec<ItemStack>>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrafterDestination {
    pub is_crafter: bool,
    /// Container.getMaxStackSize(stack), for this actual output/destination.
    pub maximum: i32,
}
/// All methods act on one caller-owned staging transaction of the real world.
/// No method may publish or reenter and change these source inputs. Insertion
/// must invoke the actual HopperBlockEntity.addItem(source,destination,...)
/// owners, including sided slots, cooldown/dirty/comparator callbacks. The
/// source/destination identities and actual world position are captured by the
/// host; this module never invents substitute block/entity/advancement owners.
pub trait CrafterOutputOwner: CrafterOwnerIdentity {
    /// Install this source snapshot into the actual captured staged owner
    /// (counter already6, inputs not consumed), then write the block property.
    /// The snapshot is phase data, never an additional physical block owner.
    fn set_crafting_block(
        &mut self,
        crafting: bool,
        flags: i32,
        source: &CrafterState,
    ) -> Result<()>;
    fn crafted_by_system(&mut self, output: &mut ItemStack) -> Result<()>;
    fn destination(
        &mut self,
        output: &ItemStack,
        facing: Direction,
    ) -> Result<Option<CrafterDestination>>;
    fn insert(&mut self, stack: ItemStack, from: Direction) -> Result<ItemStack>;
    /// DefaultDispenseItemBehavior.spawnItem: accuracy6, center + facing*0.7,
    /// then y -= 0.125 (vertical) or 0.15625 (horizontal). The Item constructor
    /// evaluates TWO level nextDouble velocity arguments before its actual
    /// independent entity factory/UUID/bob/yaw. Retain those discarded draws.
    /// Then use actual level RNG speed=nextDouble()*0.1+0.2 and X/Y/Z
    /// triangle deviations0.0172275*6 (nine level nextDouble calls total).
    /// Y's mean is0.2; X/Z means are facing.step*speed. Add through real IDs.
    fn spawn_dispensed_item(&mut self, stack: ItemStack, facing: Direction) -> Result<()>;
    /// Source finds actual ServerPlayers in a block-center box of diameter17,
    /// then triggers CRAFTER_RECIPE_CRAFTED with these PRE-consumption inputs.
    fn trigger_nearby_players(&mut self, recipe: &str, inputs: &[ItemStack; 9]) -> Result<()>;
    fn level_event(&mut self, event: i32, data: i32) -> Result<()>;
    /// Install this consumed source snapshot into the actual captured staged
    /// owner before invoking its real setChanged/comparator callbacks.
    fn block_entity_changed(&mut self, source: &CrafterState) -> Result<()>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrafterBlockEffect {
    ScheduleBlockTick {
        delay: i32,
    },
    SetProperties {
        triggered: Option<bool>,
        crafting: Option<bool>,
        flags: i32,
    },
}
/// Bind this plan to the actual captured block-entity owner in the caller.
/// Its expected complete state prevents a stale snapshot consuming newer items;
/// the real block identity is an integration contract, not a generated ID here.
#[derive(Debug)]
pub struct CrafterPlan<K: Clone + Eq + Debug> {
    owner_key: K,
    before: CrafterState,
    next: CrafterState,
    effects: Vec<CrafterBlockEffect>,
    crafted_recipe: Option<String>,
}
impl<K: Clone + Eq + Debug> CrafterPlan<K> {
    pub fn validate(
        &self,
        actual: &CrafterState,
        owner: &impl CrafterOwnerIdentity<Key = K>,
    ) -> Result<()> {
        ensure!(
            *owner.owner_key() == self.owner_key,
            "crafter block owner replaced"
        );
        ensure!(*actual == self.before, "stale crafter block-entity plan");
        Ok(())
    }
    pub fn effects(&self) -> &[CrafterBlockEffect] {
        &self.effects
    }
    pub fn next_state(&self) -> &CrafterState {
        &self.next
    }
    pub fn crafted_recipe(&self) -> Option<&str> {
        self.crafted_recipe.as_deref()
    }
    /// Commit only together with the real staged output/world owners. Effects
    /// from neighbor/tick plans still require their actual scheduler/world calls.
    pub fn commit(
        self,
        actual: &mut CrafterState,
        owner: &impl CrafterOwnerIdentity<Key = K>,
    ) -> Result<()> {
        self.validate(actual, owner)?;
        *actual = self.next;
        Ok(())
    }
}
impl CrafterState {
    /// Restore the actual saved fields after the real container/loot owner has
    /// loaded items. Java ignores disabled indices that are invalid or occupied.
    /// Raw triggered integers and crafting ticks retain source NBT semantics.
    pub fn restore(
        inputs: [ItemStack; 9],
        disabled_slots: &[i32],
        triggered: i32,
        crafting_ticks_remaining: i32,
    ) -> Result<Self> {
        for stack in &inputs {
            if !stack.is_empty() {
                stack.encode_canonical()?;
            }
        }
        let mut disabled = [false; 9];
        for &slot in disabled_slots {
            if (0..9).contains(&slot) && inputs[slot as usize].is_empty() {
                disabled[slot as usize] = true;
            }
        }
        Ok(Self {
            inputs,
            disabled,
            triggered,
            crafting_ticks_remaining,
            revision: 0,
        })
    }
    pub fn inputs(&self) -> &[ItemStack; 9] {
        &self.inputs
    }
    pub fn triggered(&self) -> bool {
        self.triggered == 1
    }
    pub fn crafting_ticks_remaining(&self) -> i32 {
        self.crafting_ticks_remaining
    }
    pub fn is_slot_disabled(&self, slot: usize) -> bool {
        slot < 9 && self.disabled[slot]
    }
    pub fn data(&self) -> [i32; 10] {
        let mut data = [0; 10];
        for (index, disabled) in self.disabled.iter().enumerate() {
            data[index] = i32::from(*disabled);
        }
        data[9] = self.triggered;
        data
    }
    fn advance_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("crafter revision exhausted"))?;
        Ok(())
    }
    /// SetSlotState uses an ENABLED boolean and accepts only an empty slot,
    /// even when enabling. The packet handler invokes this actual block entity;
    /// it does not update only a menu-private copy of disabled bits.
    pub fn set_slot_enabled(&mut self, slot: usize, enabled: bool) -> Result<bool> {
        if slot >= 9 || !self.inputs[slot].is_empty() {
            return Ok(false);
        }
        self.advance_revision()?;
        self.disabled[slot] = !enabled;
        Ok(true)
    }
    /// Container.setItem auto-enables an empty disabled slot before installing
    /// the stack. Slot.mayPlace and automation's balancing rule are separate.
    pub fn set_slot(&mut self, slot: usize, mut stack: ItemStack) -> Result<()> {
        ensure!(slot < 9, "crafter input slot outside container");
        if !stack.is_empty() {
            stack.encode_canonical()?;
            stack.count = stack.count.min(stack.max_stack_size()?.min(99));
        }
        if stack.is_empty() {
            stack = ItemStack::empty();
        }
        let mut next = self.clone();
        if next.disabled[slot] && next.inputs[slot].is_empty() {
            next.disabled[slot] = false;
        }
        next.inputs[slot] = stack;
        next.advance_revision()?;
        *self = next;
        Ok(())
    }
    /// ItemStack in-place count changes made by a menu transfer. No Container
    /// setItem auto-enable/clamp or dirty callback occurs at this boundary.
    pub(crate) fn replace_stack_no_notify(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        ensure!(slot < 9, "crafter input slot outside container");
        if !stack.is_empty() {
            stack.encode_canonical()?;
        }
        self.advance_revision()?;
        self.inputs[slot] = if stack.is_empty() {
            ItemStack::empty()
        } else {
            stack
        };
        Ok(())
    }
    pub fn remove_slot(&mut self, slot: usize, amount: i32) -> Result<ItemStack> {
        ensure!(slot < 9, "crafter input slot outside container");
        let current = self.inputs[slot].clone();
        if current.is_empty() || amount <= 0 {
            return Ok(ItemStack::empty());
        }
        let n = amount.min(current.count);
        self.advance_revision()?;
        self.inputs[slot] = counted(&current, current.count - n);
        Ok(counted(&current, n))
    }
    /// UI CrafterSlot accepts any item in an enabled slot. Automation's
    /// canPlaceItem is stricter and deliberately checks the CURRENT stack,
    /// ignoring the proposed incoming item's identity (Hopper checks that).
    pub fn may_place_menu(&self, slot: usize) -> bool {
        slot < 9 && !self.disabled[slot]
    }
    pub fn may_place_automation(&self, slot: usize, _incoming: &ItemStack) -> Result<bool> {
        ensure!(slot < 9, "crafter input slot outside container");
        if self.disabled[slot] {
            return Ok(false);
        }
        let current = &self.inputs[slot];
        if current.is_empty() {
            return Ok(true);
        }
        if current.count >= current.max_stack_size()? {
            return Ok(false);
        }
        for later in slot + 1..9 {
            let other = &self.inputs[later];
            if !self.disabled[later]
                && (other.is_empty()
                    || other.count < current.count && other.same_item_same_components(current)?)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// One point per nonempty OR disabled slot; not a fractional fullness sum.
    pub fn comparator_signal(&self) -> i32 {
        (0..9)
            .filter(|i| self.disabled[*i] || !self.inputs[*i].is_empty())
            .count() as i32
    }
    pub fn recipe_input(&self) -> RecipeInput {
        let occupied = (0..9)
            .filter(|i| !self.inputs[*i].is_empty())
            .collect::<Vec<_>>();
        if occupied.is_empty() {
            return RecipeInput::Crafting {
                width: 0,
                height: 0,
                items: vec![],
            };
        }
        let left = occupied.iter().map(|i| i % 3).min().unwrap();
        let right = occupied.iter().map(|i| i % 3).max().unwrap();
        let top = occupied.iter().map(|i| i / 3).min().unwrap();
        let bottom = occupied.iter().map(|i| i / 3).max().unwrap();
        RecipeInput::Crafting {
            width: right - left + 1,
            height: bottom - top + 1,
            items: (top..=bottom)
                .flat_map(|row| (left..=right).map(move |col| self.inputs[row * 3 + col].clone()))
                .collect(),
        }
    }
    /// Preview refresh has no item/system callbacks and consumes no inputs.
    pub fn result_preview(&self, recipes: &mut impl CrafterRecipes) -> Result<ItemStack> {
        let input = self.recipe_input();
        if self.inputs.iter().all(ItemStack::is_empty) {
            return Ok(ItemStack::empty());
        }
        match recipes.potential_recipe(&input)? {
            Some(id) => recipes.assemble(&id, &input),
            None => Ok(ItemStack::empty()),
        }
    }
    /// Caller supplies the actual block state's TRIGGERED bit and neighbor
    /// signal. A rising edge schedules4 ticks BEFORE the flags2 property write;
    /// a falling edge clears TRIGGERED/CRAFTING but retains the tick counter.
    pub fn prepare_neighbor_signal<O: CrafterOwnerIdentity>(
        &self,
        owner: &O,
        actual_triggered: bool,
        powered: bool,
    ) -> Result<CrafterPlan<O::Key>> {
        let mut next = self.clone();
        let mut effects = vec![];
        if powered && !actual_triggered {
            effects.push(CrafterBlockEffect::ScheduleBlockTick { delay: 4 });
            effects.push(CrafterBlockEffect::SetProperties {
                triggered: Some(true),
                crafting: None,
                flags: 2,
            });
            next.triggered = 1;
            next.advance_revision()?;
        } else if !powered && actual_triggered {
            effects.push(CrafterBlockEffect::SetProperties {
                triggered: Some(false),
                crafting: Some(false),
                flags: 2,
            });
            next.triggered = 0;
            next.advance_revision()?;
        }
        Ok(CrafterPlan {
            owner_key: owner.owner_key().clone(),
            before: self.clone(),
            next,
            effects,
            crafted_recipe: None,
        })
    }
    pub fn prepare_server_tick<O: CrafterOwnerIdentity>(
        &self,
        owner: &O,
    ) -> Result<CrafterPlan<O::Key>> {
        let mut next = self.clone();
        let mut effects = vec![];
        let remaining = next.crafting_ticks_remaining.wrapping_sub(1);
        if remaining >= 0 {
            next.crafting_ticks_remaining = remaining;
            next.advance_revision()?;
            if remaining == 0 {
                effects.push(CrafterBlockEffect::SetProperties {
                    triggered: None,
                    crafting: Some(false),
                    flags: 3,
                });
            }
        }
        Ok(CrafterPlan {
            owner_key: owner.owner_key().clone(),
            before: self.clone(),
            next,
            effects,
            crafted_recipe: None,
        })
    }
    /// Invoke from the actual scheduled CrafterBlock tick. The source does not
    /// recheck power here: a queued tick can still craft after a falling edge.
    /// `world` MUST be a private transaction of the actual owners, never a noop.
    pub fn prepare_auto_craft<W: CrafterOutputOwner>(
        &self,
        facing: Direction,
        recipes: &mut impl CrafterRecipes,
        world: &mut W,
    ) -> Result<CrafterPlan<W::Key>> {
        let input = self.recipe_input();
        let selected = if self.inputs.iter().all(ItemStack::is_empty) {
            None
        } else {
            recipes.potential_recipe(&input)?
        };
        let mut next = self.clone();
        let Some(recipe) = selected else {
            world.level_event(1050, 0)?;
            return Ok(CrafterPlan {
                owner_key: world.owner_key().clone(),
                before: self.clone(),
                next,
                effects: vec![],
                crafted_recipe: None,
            });
        };
        let mut output = recipes.assemble(&recipe, &input)?;
        if output.is_empty() {
            world.level_event(1050, 0)?;
            return Ok(CrafterPlan {
                owner_key: world.owner_key().clone(),
                before: self.clone(),
                next,
                effects: vec![],
                crafted_recipe: None,
            });
        }
        next.crafting_ticks_remaining = 6;
        world.set_crafting_block(true, 2, &next)?;
        world.crafted_by_system(&mut output)?;
        self.dispense(output, facing, &recipe, world)?;
        for remaining in recipes.remaining_items(&recipe, &input)? {
            if !remaining.is_empty() {
                self.dispense(remaining, facing, &recipe, world)?;
            }
        }
        for stack in &mut next.inputs {
            if !stack.is_empty() {
                *stack = counted(stack, stack.count - 1);
            }
        }
        next.advance_revision()?;
        world.block_entity_changed(&next)?;
        Ok(CrafterPlan {
            owner_key: world.owner_key().clone(),
            before: self.clone(),
            next,
            effects: vec![],
            crafted_recipe: Some(recipe),
        })
    }
    fn dispense(
        &self,
        stack: ItemStack,
        facing: Direction,
        recipe: &str,
        world: &mut impl CrafterOutputOwner,
    ) -> Result<()> {
        let destination = world.destination(&stack, facing)?;
        let mut remaining = stack.clone();
        if let Some(destination) = destination {
            ensure!(
                destination.maximum > 0,
                "invalid actual crafter destination stack limit"
            );
            if destination.is_crafter || stack.count > destination.maximum {
                while !remaining.is_empty() {
                    let one = counted(&remaining, 1);
                    let left = world.insert(one.clone(), facing.opposite())?;
                    validate_leftover(&one, &left)?;
                    if !left.is_empty() {
                        break;
                    }
                    remaining = counted(&remaining, remaining.count - 1);
                }
            } else {
                while !remaining.is_empty() {
                    let left = world.insert(remaining.clone(), facing.opposite())?;
                    validate_leftover(&remaining, &left)?;
                    let unchanged = left.count == remaining.count;
                    remaining = left;
                    if unchanged {
                        break;
                    }
                }
            }
        }
        if !remaining.is_empty() {
            world.spawn_dispensed_item(remaining, facing)?;
            world.trigger_nearby_players(recipe, &self.inputs)?;
            world.level_event(1049, 0)?;
            world.level_event(2010, facing as i32)?;
        }
        Ok(())
    }
}
fn counted(stack: &ItemStack, count: i32) -> ItemStack {
    if count <= 0 || stack.is_empty() {
        ItemStack::empty()
    } else {
        let mut result = stack.clone();
        result.count = count;
        result
    }
}
fn validate_leftover(before: &ItemStack, after: &ItemStack) -> Result<()> {
    ensure!(
        after.is_empty()
            || after.count <= before.count && after.same_item_same_components(before)?,
        "actual container insertion returned unrelated or larger remainder"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::RecipeBook;
    use std::{cell::RefCell, rc::Rc};
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    struct Owner(u64);
    impl CrafterOwnerIdentity for Owner {
        type Key = u64;
        fn owner_key(&self) -> &u64 {
            &self.0
        }
    }
    struct Recipes {
        id: &'static str,
        log: Rc<RefCell<Vec<String>>>,
    }
    impl CrafterRecipes for Recipes {
        fn potential_recipe(&mut self, input: &RecipeInput) -> Result<Option<String>> {
            self.log.borrow_mut().push("recipe".into());
            Ok(RecipeBook::vanilla()
                .evaluate(self.id, input)?
                .map(|_| self.id.to_owned()))
        }
        fn assemble(&mut self, id: &str, input: &RecipeInput) -> Result<ItemStack> {
            self.log.borrow_mut().push("assemble".into());
            Ok(RecipeBook::vanilla().evaluate(id, input)?.unwrap().stack)
        }
        fn remaining_items(&mut self, id: &str, input: &RecipeInput) -> Result<Vec<ItemStack>> {
            self.log.borrow_mut().push("remainders".into());
            Ok(RecipeBook::vanilla()
                .evaluate(id, input)?
                .unwrap()
                .remaining)
        }
    }
    struct World {
        log: Rc<RefCell<Vec<String>>>,
        destination: Option<CrafterDestination>,
        capacity: i32,
        inserts: Vec<i32>,
        spawned: Vec<ItemStack>,
        criteria_inputs: Vec<[ItemStack; 9]>,
    }
    impl CrafterOwnerIdentity for World {
        type Key = u64;
        fn owner_key(&self) -> &u64 {
            static KEY: u64 = 1;
            &KEY
        }
    }
    impl CrafterOutputOwner for World {
        fn set_crafting_block(
            &mut self,
            value: bool,
            flags: i32,
            source: &CrafterState,
        ) -> Result<()> {
            assert_eq!(source.crafting_ticks_remaining(), 6);
            self.log
                .borrow_mut()
                .push(format!("crafting:{value}:{flags}"));
            Ok(())
        }
        fn crafted_by_system(&mut self, _: &mut ItemStack) -> Result<()> {
            self.log.borrow_mut().push("system".into());
            Ok(())
        }
        fn destination(
            &mut self,
            _: &ItemStack,
            _: Direction,
        ) -> Result<Option<CrafterDestination>> {
            Ok(self.destination)
        }
        fn insert(&mut self, stack: ItemStack, from: Direction) -> Result<ItemStack> {
            assert_eq!(from, Direction::West);
            self.inserts.push(stack.count);
            let accepted = self.capacity.min(stack.count);
            self.capacity -= accepted;
            Ok(counted(&stack, stack.count - accepted))
        }
        fn spawn_dispensed_item(&mut self, stack: ItemStack, facing: Direction) -> Result<()> {
            assert_eq!(facing, Direction::East);
            self.log.borrow_mut().push(format!("spawn:{}", stack.count));
            self.spawned.push(stack);
            Ok(())
        }
        fn trigger_nearby_players(&mut self, _: &str, inputs: &[ItemStack; 9]) -> Result<()> {
            self.criteria_inputs.push(inputs.clone());
            self.log.borrow_mut().push("criterion".into());
            Ok(())
        }
        fn level_event(&mut self, id: i32, data: i32) -> Result<()> {
            self.log.borrow_mut().push(format!("event:{id}:{data}"));
            Ok(())
        }
        fn block_entity_changed(&mut self, source: &CrafterState) -> Result<()> {
            assert_eq!(source.crafting_ticks_remaining(), 6);
            self.log.borrow_mut().push("changed".into());
            Ok(())
        }
    }
    #[test]
    fn cake_dispenses_result_and_three_remainders_before_consumption() -> Result<()> {
        let mut state = CrafterState::default();
        let names = [
            "milk_bucket",
            "milk_bucket",
            "milk_bucket",
            "sugar",
            "egg",
            "sugar",
            "wheat",
            "wheat",
            "wheat",
        ];
        for (i, name) in names.into_iter().enumerate() {
            state.set_slot(i, item(name, 1))?;
        }
        let before = state.inputs.clone();
        let log = Rc::new(RefCell::new(vec![]));
        let mut recipes = Recipes {
            id: "minecraft:cake",
            log: log.clone(),
        };
        assert_eq!(
            state.result_preview(&mut recipes)?.item_id,
            item("cake", 1).item_id
        );
        log.borrow_mut().clear();
        let mut world = World {
            log: log.clone(),
            destination: None,
            capacity: 0,
            inserts: vec![],
            spawned: vec![],
            criteria_inputs: vec![],
        };
        let plan = state.prepare_auto_craft(Direction::East, &mut recipes, &mut world)?;
        assert_eq!(state.inputs, before); // prepared world/state publish together later
        assert_eq!(
            world.spawned.iter().map(|i| i.item_id).collect::<Vec<_>>(),
            [
                item("cake", 1).item_id,
                item("bucket", 1).item_id,
                item("bucket", 1).item_id,
                item("bucket", 1).item_id
            ]
        );
        assert!(world.criteria_inputs.iter().all(|inputs| *inputs == before));
        let entries = log.borrow();
        assert_eq!(
            &entries[..5],
            ["recipe", "assemble", "crafting:true:2", "system", "spawn:1"]
        );
        let remainders = entries.iter().position(|s| s == "remainders").unwrap();
        assert!(remainders > entries.iter().position(|s| s == "event:2010:5").unwrap());
        assert_eq!(entries.last().unwrap(), "changed");
        drop(entries);
        plan.commit(&mut state, &Owner(1))?;
        assert!(state.inputs.iter().all(ItemStack::is_empty));
        assert_eq!(state.crafting_ticks_remaining(), 6);
        for _ in 0..5 {
            let tick = state.prepare_server_tick(&Owner(1))?;
            assert!(tick.effects().is_empty());
            tick.commit(&mut state, &Owner(1))?;
        }
        let end = state.prepare_server_tick(&Owner(1))?;
        assert_eq!(
            end.effects(),
            [CrafterBlockEffect::SetProperties {
                triggered: None,
                crafting: Some(false),
                flags: 3
            }]
        );
        end.commit(&mut state, &Owner(1))?;
        assert_eq!(state.crafting_ticks_remaining(), 0);
        Ok(())
    }
    #[test]
    fn balancing_disabled_slots_edges_and_neighbor_crafter_insertion() -> Result<()> {
        let mut state = CrafterState::default();
        state.set_slot(0, item("stone", 5))?;
        state.set_slot(1, item("stone", 3))?;
        for i in 2..9 {
            assert!(state.set_slot_enabled(i, false)?);
        }
        assert!(!state.may_place_automation(0, &item("dirt", 1))?);
        assert!(state.may_place_automation(1, &item("dirt", 1))?); // incoming identity is deliberately ignored
        assert!(!state.set_slot_enabled(0, false)?);
        assert_eq!(state.comparator_signal(), 9);
        state.set_slot(2, item("dirt", 1))?;
        assert!(!state.is_slot_disabled(2));
        let rise = state.prepare_neighbor_signal(&Owner(1), false, true)?;
        assert_eq!(
            rise.effects()[0],
            CrafterBlockEffect::ScheduleBlockTick { delay: 4 }
        );
        rise.commit(&mut state, &Owner(1))?;
        assert!(state.triggered());
        state
            .prepare_neighbor_signal(&Owner(1), true, false)?
            .commit(&mut state, &Owner(1))?;
        assert!(!state.triggered());
        let mut state = CrafterState::default();
        state.set_slot(4, item("oak_log", 1))?;
        let RecipeInput::Crafting { width, height, .. } = state.recipe_input() else {
            unreachable!()
        };
        assert_eq!((width, height), (1, 1));
        let log = Rc::new(RefCell::new(vec![]));
        let mut recipes = Recipes {
            id: "minecraft:oak_planks",
            log: log.clone(),
        };
        let mut world = World {
            log,
            destination: Some(CrafterDestination {
                is_crafter: true,
                maximum: 64,
            }),
            capacity: 2,
            inserts: vec![],
            spawned: vec![],
            criteria_inputs: vec![],
        };
        // A scheduled tick still crafts after the falling edge; no power recheck.
        let plan = state.prepare_auto_craft(Direction::East, &mut recipes, &mut world)?;
        assert_eq!(world.inserts, [1, 1, 1]);
        assert_eq!(world.spawned[0].count, 2);
        assert_eq!(world.criteria_inputs[0][4].count, 1);
        let stale = state.prepare_neighbor_signal(&Owner(1), false, true)?;
        plan.commit(&mut state, &Owner(1))?;
        assert!(stale.commit(&mut state, &Owner(1)).is_err());
        Ok(())
    }
}
