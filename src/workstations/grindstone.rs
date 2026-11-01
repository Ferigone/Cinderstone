//! Java 1.21 GrindstoneMenu computation and result ownership.
//!
//! The two inputs are owned; `preview` is derived and is never returned on close.
//! Enchantment tag/cost queries and the result's world callbacks belong to the
//! real registry and Level owners. This module does not supply replacements.
use crate::{
    crafting::RecipeBook,
    inventory::{Component, ItemStack},
    protocol::{Reader, Writer},
};
use anyhow::{ensure, Result};
use std::collections::BTreeMap;

pub const MENU_TYPE: i32 = 15;
pub const INPUT_SLOTS: usize = 2;
pub const RESULT_SLOT: usize = 2;
pub const MENU_SLOTS: usize = 39;

/// Queries against the actual Level's enchantment registry and curse tag.
/// Registry IDs refer to the same registry used to decode the item components.
/// Implementations must reject unresolved holders/tags rather than invent data.
pub trait GrindstoneEnchantments {
    fn is_curse(&self, enchantment: i32) -> Result<bool>;
    fn minimum_cost(&self, enchantment: i32, level: i32) -> Result<i32>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GrindstoneState {
    inputs: [ItemStack; INPUT_SLOTS],
    preview: ItemStack,
}

/// A destination must have accepted `stack` before drawing XP and committing.
/// No owned input or result is removed by preparing this token.
#[derive(Debug)]
pub struct PreparedGrindstoneTake {
    inputs: [ItemStack; INPUT_SLOTS],
    stack: ItemStack,
    preview: ItemStack,
    after_first_clear_preview: ItemStack,
    experience_sum: i32,
}

impl PreparedGrindstoneTake {
    /// The actual amount reserved for the destination, not an extra owned copy.
    pub fn stack(&self) -> &ItemStack {
        &self.stack
    }
}

/// Ordered source obligations for the owning transaction: award actual orbs at
/// the block centre, publish Level event 1042/data 0, clear input 0, clear input 1.
/// `commit_take` stages the input clears; publish that state only after preparing
/// the actual Level callbacks. A player XP grant cannot replace the orb award.
#[derive(Debug)]
pub struct GrindstoneTake {
    pub stack: ItemStack,
    pub experience: i32,
    pub level_event: (i32, i32),
    /// Each source setItem triggers slotsChanged/createResult/broadcastChanges.
    /// Broadcast input0-empty/input1-old with the first preview, then both-empty.
    pub input_clear_previews: [ItemStack; INPUT_SLOTS],
}

impl GrindstoneState {
    pub fn inputs(&self) -> &[ItemStack; INPUT_SLOTS] {
        &self.inputs
    }

    pub fn preview(&self) -> &ItemStack {
        &self.preview
    }

    pub fn may_place(stack: &ItemStack) -> Result<bool> {
        Ok(!stack.is_empty() && (stack.is_damageable()? || has_enchantments(stack)?))
    }

    /// Mutates an owned input and recomputes the derived result atomically. Slot
    /// capacity remains the ordinary item's maximum; a count above one is legal
    /// in an input slot but suppresses result computation.
    pub fn set_input(
        &mut self,
        slot: usize,
        stack: ItemStack,
        enchantments: &impl GrindstoneEnchantments,
    ) -> Result<()> {
        ensure!(slot < INPUT_SLOTS, "invalid grindstone input slot");
        ensure!(
            stack.is_empty() || Self::may_place(&stack)?,
            "grindstone input rejected"
        );
        ensure!(
            stack.is_empty() || stack.count <= stack.max_stack_size()?.min(99),
            "grindstone input exceeds slot limit"
        );
        let mut inputs = self.inputs.clone();
        inputs[slot] = canonical(stack);
        let preview = compute_result(&inputs[0], &inputs[1], enchantments)?;
        self.inputs = inputs;
        self.preview = preview;
        Ok(())
    }

    /// Source in-place stack mutation; deliberately no container notification.
    /// The controller separately preserves SimpleContainer/Slot callback phases.
    pub(crate) fn mutate_input_in_place(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        ensure!(slot < INPUT_SLOTS, "invalid grindstone input slot");
        self.inputs[slot] = canonical(stack);
        Ok(())
    }

    /// SimpleContainer.setItem clamps without consulting Slot.mayPlace.
    pub(crate) fn set_container_input(&mut self, slot: usize, mut stack: ItemStack) -> Result<()> {
        if !stack.is_empty() {
            stack.count = stack.count.min(stack.max_stack_size()?.min(99));
        }
        self.mutate_input_in_place(slot, stack)
    }

    /// The single ResultContainer stack, including in-place shift leftovers.
    pub(crate) fn mutate_result_in_place(&mut self, stack: ItemStack) {
        self.preview = canonical(stack);
    }

    /// Result onTake reads the CURRENT inputs, even after earlier PickupAll
    /// removals. The actual Level RNG owner supplies each positive-bound draw.
    pub(crate) fn experience_bound(
        &self,
        enchantments: &impl GrindstoneEnchantments,
    ) -> Result<i32> {
        let sum = experience_from_item(&self.inputs[0], enchantments)?
            .wrapping_add(experience_from_item(&self.inputs[1], enchantments)?);
        Ok(if sum <= 0 { 0 } else { sum / 2 + sum % 2 })
    }

    /// Used when the actual registry/tag owner changes; no input is consumed.
    pub fn recompute(&mut self, enchantments: &impl GrindstoneEnchantments) -> Result<()> {
        self.preview = compute_result(&self.inputs[0], &self.inputs[1], enchantments)?;
        Ok(())
    }

    /// `amount` is the actual destination transfer count. Ordinary extraction
    /// uses ResultContainer.removeItem and removes the WHOLE result regardless
    /// of the requested amount. A partial count is possible only during
    /// quick-move; onTake then clears both inputs and discards leftover preview.
    pub fn prepare_take(
        &self,
        amount: i32,
        enchantments: &impl GrindstoneEnchantments,
    ) -> Result<PreparedGrindstoneTake> {
        ensure!(
            !self.preview.is_empty() && amount > 0 && amount <= self.preview.count,
            "invalid grindstone result take"
        );
        let mut stack = self.preview.clone();
        stack.count = amount;
        let experience_sum = experience_from_item(&self.inputs[0], enchantments)?
            .wrapping_add(experience_from_item(&self.inputs[1], enchantments)?);
        Ok(PreparedGrindstoneTake {
            inputs: self.inputs.clone(),
            stack,
            preview: self.preview.clone(),
            after_first_clear_preview: compute_result(
                &ItemStack::empty(),
                &self.inputs[1],
                enchantments,
            )?,
            experience_sum,
        })
    }

    /// The host stages this operation together with its real Level RNG and orb
    /// transaction. The closure is called exactly once for positive XP, even
    /// when the bound is one; it is not called for zero/overflow-negative sums.
    /// Use a privately staged menu and RNG if a later world callback can fail.
    pub fn commit_take(
        &mut self,
        prepared: PreparedGrindstoneTake,
        mut next_level_random_int: impl FnMut(i32) -> Result<i32>,
    ) -> Result<GrindstoneTake> {
        ensure!(
            self.inputs == prepared.inputs && self.preview == prepared.preview,
            "grindstone inputs/result changed after preparation"
        );
        let experience = if prepared.experience_sum > 0 {
            // Java Math.ceil(intSum / 2.0), including odd i32::MAX.
            let bound = (prepared.experience_sum / 2) + (prepared.experience_sum % 2);
            let draw = next_level_random_int(bound)?;
            ensure!(
                (0..bound).contains(&draw),
                "Level RNG returned an invalid grindstone sample"
            );
            bound.wrapping_add(draw)
        } else {
            0
        };
        self.inputs = [ItemStack::empty(), ItemStack::empty()];
        self.preview = ItemStack::empty();
        Ok(GrindstoneTake {
            stack: prepared.stack,
            experience,
            level_event: (1042, 0),
            input_clear_previews: [prepared.after_first_clear_preview, ItemStack::empty()],
        })
    }

    /// AbstractContainerMenu.removed/clearContainer must return or drop only
    /// these owned inputs, after the owning menu handles its carried stack.
    pub fn drain_inputs_on_close(&mut self) -> [ItemStack; INPUT_SLOTS] {
        self.preview = ItemStack::empty();
        std::mem::take(&mut self.inputs)
    }
}

pub fn compute_result(
    first: &ItemStack,
    second: &ItemStack,
    enchantments: &impl GrindstoneEnchantments,
) -> Result<ItemStack> {
    if (first.is_empty() && second.is_empty()) || first.count > 1 || second.count > 1 {
        return Ok(ItemStack::empty());
    }
    if first.is_empty() || second.is_empty() {
        let input = if first.is_empty() { second } else { first };
        if !has_enchantments(input)? {
            return Ok(ItemStack::empty());
        }
        return remove_non_curses(input.clone(), enchantments);
    }
    if first.item_id != second.item_id {
        return Ok(ItemStack::empty());
    }
    let max_first = int_component(first, 2)?;
    let max_second = int_component(second, 2)?;
    ensure!(
        max_first >= 0 && max_second >= 0,
        "negative grindstone maximum damage"
    );
    let maximum = max_first.max(max_second);
    let durability = max_first
        .wrapping_sub(first.damage_value()?)
        .wrapping_add(max_second.wrapping_sub(second.damage_value()?))
        .wrapping_add(maximum.wrapping_mul(5) / 100);
    let mut result = first.clone();
    result.count = 1;
    if !first.is_damageable()? {
        if first.max_stack_size()? < 2 || !first.same_item_same_components(second)? {
            return Ok(ItemStack::empty());
        }
        result.count = 2;
    }
    if result.is_damageable()? {
        set_int_component(&mut result, 2, maximum);
        set_int_component(
            &mut result,
            3,
            maximum.wrapping_sub(durability).max(0).clamp(0, maximum),
        );
    }
    let component = crafting_enchantment_component(&result);
    if result.effective_component_bytes(component).is_some() {
        let mut merged = read_enchantments(&result, component)?;
        let additions = read_enchantments(second, crafting_enchantment_component(second))?;
        for (id, level) in additions.levels {
            if enchantments.is_curse(id)? && merged.levels.get(&id).copied().unwrap_or(0) != 0 {
                continue;
            }
            if level > 0 {
                let old = merged.levels.entry(id).or_default();
                *old = (*old).max(level.min(255));
            }
        }
        set_enchantments(&mut result, component, merged);
    }
    remove_non_curses(result, enchantments)
}

fn remove_non_curses(
    mut result: ItemStack,
    context: &impl GrindstoneEnchantments,
) -> Result<ItemStack> {
    let component = crafting_enchantment_component(&result);
    let mut enchantments = read_enchantments(&result, component)?;
    let mut curses = BTreeMap::new();
    for (id, level) in enchantments.levels {
        if context.is_curse(id)? {
            curses.insert(id, level);
        }
    }
    enchantments.levels = curses;
    if result.effective_component_bytes(component).is_some() {
        set_enchantments(&mut result, component, enchantments.clone());
    }
    if is_enchanted_book(&result) && enchantments.levels.is_empty() {
        // ItemStack.transmuteCopy retains the original component patch while
        // changing prototype; do not reconstruct a component-free normal book.
        result.item_id = RecipeBook::vanilla()
            .item_id("book")
            .expect("pinned book item");
    }
    let mut cost = 0_i32;
    for _ in 0..enchantments.levels.len() {
        // cqt.removeNonCurses calls AnvilMenu.calculateIncreasedRepairCost
        // (cpw.e): multiply as long, saturate at Integer.MAX_VALUE, then narrow.
        cost = (i64::from(cost) * 2 + 1).min(i64::from(i32::MAX)) as i32;
    }
    set_int_component(&mut result, 16, cost);
    Ok(result)
}

fn experience_from_item(stack: &ItemStack, context: &impl GrindstoneEnchantments) -> Result<i32> {
    let mut amount = 0_i32;
    for (id, level) in read_enchantments(stack, crafting_enchantment_component(stack))?.levels {
        if !context.is_curse(id)? {
            amount = amount.wrapping_add(context.minimum_cost(id, level)?);
        }
    }
    Ok(amount)
}

fn crafting_enchantment_component(stack: &ItemStack) -> i32 {
    if is_enchanted_book(stack) {
        23
    } else {
        9
    }
}
fn is_enchanted_book(stack: &ItemStack) -> bool {
    !stack.is_empty() && Some(stack.item_id) == RecipeBook::vanilla().item_id("enchanted_book")
}
fn has_enchantments(stack: &ItemStack) -> Result<bool> {
    Ok(!read_enchantments(stack, 9)?.levels.is_empty()
        || !read_enchantments(stack, 23)?.levels.is_empty())
}
#[derive(Clone, Debug)]
struct Enchantments {
    levels: BTreeMap<i32, i32>,
    show_in_tooltip: bool,
}
fn read_enchantments(stack: &ItemStack, component: i32) -> Result<Enchantments> {
    let Some(bytes) = stack.effective_component_bytes(component) else {
        return Ok(Enchantments {
            levels: BTreeMap::new(),
            show_in_tooltip: true,
        });
    };
    let mut reader = Reader::new(bytes);
    let count = reader.varint()?;
    ensure!(
        (0..=65_536).contains(&count),
        "invalid grindstone enchantment count"
    );
    let mut levels = BTreeMap::new();
    for _ in 0..count {
        let id = reader.varint()?;
        let level = reader.varint()?;
        ensure!(
            id >= 0 && (0..=255).contains(&level),
            "invalid grindstone enchantment holder/level"
        );
        levels.insert(id, level);
    }
    let show_in_tooltip = reader.bool()?;
    reader.finish()?;
    Ok(Enchantments {
        levels,
        show_in_tooltip,
    })
}
fn set_enchantments(stack: &mut ItemStack, component: i32, enchantments: Enchantments) {
    let mut writer = Writer::new();
    writer.varint(enchantments.levels.len() as i32);
    for (id, level) in enchantments.levels {
        writer.varint(id).varint(level);
    }
    writer.bool(enchantments.show_in_tooltip);
    set_component(stack, component, writer.into_inner());
}
fn int_component(stack: &ItemStack, id: i32) -> Result<i32> {
    stack.effective_component_bytes(id).map_or(Ok(0), |bytes| {
        let mut reader = Reader::new(bytes);
        let value = reader.varint()?;
        reader.finish()?;
        Ok(value)
    })
}
fn set_int_component(stack: &mut ItemStack, id: i32, value: i32) {
    let mut writer = Writer::new();
    writer.varint(value);
    set_component(stack, id, writer.into_inner());
}
fn set_component(stack: &mut ItemStack, id: i32, data: Vec<u8>) {
    stack.removed_components.retain(|old| *old != id);
    stack.components.retain(|old| old.type_id != id);
    stack.components.push(Component { type_id: id, data });
    stack.components.sort_by_key(|component| component.type_id);
    stack.normalize_patch();
}
fn canonical(stack: ItemStack) -> ItemStack {
    if stack.is_empty() {
        ItemStack::empty()
    } else {
        stack
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::LegacyRandomSource;

    // An explicitly supplied fixture registry for these local kernel checks;
    // it is not a production enchantment owner or an automatically bound tag.
    struct FixtureRegistry;
    fn enchantment_id(name: &str) -> i32 {
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../../tools/crafting_metadata.json")).unwrap();
        metadata["registries"]["minecraft:enchantment"][format!("minecraft:{name}")]
            .as_i64()
            .unwrap() as i32
    }
    impl GrindstoneEnchantments for FixtureRegistry {
        fn is_curse(&self, id: i32) -> Result<bool> {
            Ok(id == enchantment_id("vanishing_curse") || id == enchantment_id("binding_curse"))
        }
        fn minimum_cost(&self, id: i32, level: i32) -> Result<i32> {
            if id == enchantment_id("sharpness") {
                return Ok(1 + (level - 1) * 11);
            }
            if id == enchantment_id("unbreaking") {
                return Ok(5 + (level - 1) * 8);
            }
            anyhow::bail!("unresolved fixture enchantment {id}")
        }
    }
    fn item(name: &str) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), 1).unwrap()
    }
    fn enchanted(mut stack: ItemStack, levels: &[(&str, i32)], tooltip: bool) -> ItemStack {
        let component = crafting_enchantment_component(&stack);
        set_enchantments(
            &mut stack,
            component,
            Enchantments {
                levels: levels
                    .iter()
                    .map(|(name, level)| (enchantment_id(name), *level))
                    .collect(),
                show_in_tooltip: tooltip,
            },
        );
        stack
    }

    #[test]
    fn pinned_java_result_facts_and_real_input_close_ownership() -> Result<()> {
        let mut sword = item("iron_sword");
        set_int_component(&mut sword, 3, 200);
        // Direct invocation of unchanged Java cqt.computeResult returned empty.
        assert!(compute_result(&sword, &ItemStack::empty(), &FixtureRegistry)?.is_empty());
        let first = enchanted(sword, &[("vanishing_curse", 1), ("sharpness", 2)], false);
        let mut second = item("iron_sword");
        set_int_component(&mut second, 3, 100);
        second = enchanted(second, &[("vanishing_curse", 3), ("unbreaking", 1)], true);
        let result = compute_result(&first, &second, &FixtureRegistry)?;
        // These values were separately observed from the pinned Java method.
        assert_eq!(result.damage_value()?, 38);
        assert_eq!(int_component(&result, 16)?, 1);
        let curses = read_enchantments(&result, 9)?;
        assert_eq!(
            curses.levels,
            BTreeMap::from([(enchantment_id("vanishing_curse"), 1)])
        );
        assert!(!curses.show_in_tooltip);
        let book = enchanted(item("enchanted_book"), &[("sharpness", 2)], true);
        let result = compute_result(&book, &ItemStack::empty(), &FixtureRegistry)?;
        assert!(result.same_item_same_components(&item("book"))?);
        let mut state = GrindstoneState::default();
        state.set_input(0, first.clone(), &FixtureRegistry)?;
        state.set_input(1, second.clone(), &FixtureRegistry)?;
        assert!(!state.preview().is_empty());
        assert_eq!(state.drain_inputs_on_close(), [first, second]);
        assert!(state.preview().is_empty());
        assert_eq!(
            state.drain_inputs_on_close(),
            [ItemStack::empty(), ItemStack::empty()]
        );
        Ok(())
    }

    #[test]
    fn retained_curse_repair_cost_saturates_for_actual_holder_tag_fixture() -> Result<()> {
        // Actual registered holder IDs, with an explicitly restricted fixture
        // curse-tag assignment. This is not vanilla's two-entry curse tag or
        // an original-Java gameplay observation; tags belong to the real host.
        struct TaggedRegistry(std::collections::BTreeSet<i32>);
        impl GrindstoneEnchantments for TaggedRegistry {
            fn is_curse(&self, id: i32) -> Result<bool> {
                Ok(self.0.contains(&id))
            }
            fn minimum_cost(&self, _: i32, _: i32) -> Result<i32> {
                anyhow::bail!("fixture retained curses do not query minimum costs")
            }
        }
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../../tools/crafting_metadata.json"))?;
        let ids = metadata["registries"]["minecraft:enchantment"]
            .as_object()
            .unwrap()
            .values()
            .map(|value| value.as_i64().unwrap() as i32)
            .take(32)
            .collect::<std::collections::BTreeSet<_>>();
        ensure!(
            ids.len() == 32,
            "fixture requires32 actual registered holders"
        );
        let registry = TaggedRegistry(ids.clone());
        let mut sword = item("iron_sword");
        set_enchantments(
            &mut sword,
            9,
            Enchantments {
                levels: ids.into_iter().map(|id| (id, 1)).collect(),
                show_in_tooltip: true,
            },
        );
        let result = compute_result(&sword, &ItemStack::empty(), &registry)?;
        assert_eq!(read_enchantments(&result, 9)?.levels.len(), 32);
        assert_eq!(int_component(&result, 16)?, i32::MAX);
        Ok(())
    }

    #[test]
    fn prepared_take_checks_identity_and_consumes_bound_one_rng() -> Result<()> {
        let mut state = GrindstoneState::default();
        let first = enchanted(item("iron_sword"), &[("sharpness", 1)], true);
        state.set_input(0, first.clone(), &FixtureRegistry)?;
        let token = state.prepare_take(1, &FixtureRegistry)?;
        state.set_input(1, item("iron_sword"), &FixtureRegistry)?;
        assert!(state
            .commit_take(token, |_| panic!("stale take must not draw"))
            .is_err());
        state.set_input(1, ItemStack::empty(), &FixtureRegistry)?;
        let token = state.prepare_take(1, &FixtureRegistry)?;
        let mut random = LegacyRandomSource::new(0);
        let before = random.state48();
        let taken = state.commit_take(token, |bound| {
            assert_eq!(bound, 1);
            Ok(random.next_bounded_i32(bound)?)
        })?;
        assert_ne!(random.state48(), before);
        assert_eq!(taken.experience, 1);
        assert_eq!(taken.level_event, (1042, 0));
        assert!(state.inputs().iter().all(ItemStack::is_empty));
        assert!(state.prepare_take(1, &FixtureRegistry).is_err());
        // Java emits count two for two identical enchanted, stackable items.
        let stick = enchanted(item("stick"), &[("vanishing_curse", 1)], true);
        state.set_input(0, stick.clone(), &FixtureRegistry)?;
        state.set_input(1, stick, &FixtureRegistry)?;
        assert_eq!(state.preview().count, 2);
        let partial = state.prepare_take(1, &FixtureRegistry)?;
        let taken = state.commit_take(partial, |_| panic!("curse-only must not draw"))?;
        assert_eq!(taken.stack.count, 1);
        assert!(state.inputs().iter().all(ItemStack::is_empty));
        assert!(!taken.input_clear_previews[0].is_empty());
        assert!(taken.input_clear_previews[1].is_empty());
        Ok(())
    }
}
