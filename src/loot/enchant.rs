//! Enchantment reads/updates and `EnchantmentHelper` selection used by loot.
//!
//! `ItemEnchantments` is a wire component here (type 9 `enchantments`, type 23
//! `stored_enchantments`): `varint count, (varint registry ID, varint level)*,
//! bool showInTooltip`. Java keeps the map in a hash map whose iteration order
//! is not part of the observable semantics; entries are written in ascending
//! registry order, and equality in the repository's own `ItemStack` comparison
//! sorts them as well.

use super::facts::{facts, Kind};
use super::host::{EquipmentSlot, LootEntity};
use super::predicate::stack_item_id;
use super::{mth, LootError, Result};
use crate::inventory::{Component, ItemStack};
use crate::protocol::{Reader, Writer};
use crate::random::RandomSource;

const ENCHANTMENTS: i32 = 9;
const STORED_ENCHANTMENTS: i32 = 23;

fn host(error: anyhow::Error) -> LootError {
    LootError::Host(error.to_string())
}

/// `new ItemStack(item)`: count one, no component patch.
pub fn new_stack(item: &str) -> Result<ItemStack> {
    let id = crate::item_use::item_id(item).map_err(host)?;
    Ok(ItemStack {
        count: 1,
        item_id: id,
        components: Vec::new(),
        removed_components: Vec::new(),
    })
}

/// `ItemStack.getCount()`: an empty stack (count ≤ 0 or air) reports zero, so
/// arithmetic on a stack that a function drove negative restarts from zero.
pub fn count_of(stack: &ItemStack) -> i32 {
    if stack.is_empty() {
        0
    } else {
        stack.count
    }
}

/// `ItemStack.copyWithCount`.
pub fn copy_with_count(stack: &ItemStack, count: i32) -> ItemStack {
    if stack.is_empty() {
        ItemStack::empty()
    } else {
        ItemStack {
            count,
            ..stack.clone()
        }
    }
}

/// Install a component value in the patch, dropping it again when it equals the
/// item prototype (`PatchedDataComponentMap.set`).
pub fn set_component(stack: &mut ItemStack, type_id: i32, data: Vec<u8>) {
    stack.components.retain(|c| c.type_id != type_id);
    stack.removed_components.retain(|id| *id != type_id);
    stack.components.push(Component { type_id, data });
    stack.normalize_patch();
}

fn decode(bytes: &[u8]) -> Result<(Vec<(String, i32)>, bool)> {
    let mut reader = Reader::new(bytes);
    let count = reader.varint().map_err(host)?;
    let mut entries = Vec::new();
    for _ in 0..count {
        let id = reader.varint().map_err(host)?;
        let level = reader.varint().map_err(host)?;
        let name = facts()
            .registries
            .enchantment
            .get(usize::try_from(id).map_err(|_| LootError::Host("negative enchantment ID".into()))?)
            .ok_or_else(|| LootError::Host(format!("unknown enchantment ID {id}")))?;
        entries.push((name.clone(), level));
    }
    let show = reader.bool().map_err(host)?;
    reader.finish().map_err(host)?;
    Ok((entries, show))
}

fn encode(entries: &[(String, i32)], show_in_tooltip: bool) -> Result<Vec<u8>> {
    let mut sorted: Vec<(usize, i32)> = Vec::with_capacity(entries.len());
    for (name, level) in entries {
        let id = facts()
            .enchantment_id(name)
            .ok_or_else(|| LootError::Host(format!("unknown enchantment {name}")))?;
        sorted.push((id, *level));
    }
    sorted.sort_by_key(|(id, _)| *id);
    let mut writer = Writer::new();
    writer.varint(sorted.len() as i32);
    for (id, level) in sorted {
        writer.varint(id as i32).varint(level);
    }
    writer.bool(show_in_tooltip);
    Ok(writer.into_inner())
}

/// `stack.get(ENCHANTMENTS)` / `get(STORED_ENCHANTMENTS)`: `None` when the stack
/// has no such component, otherwise its entries.
fn read_component(stack: &ItemStack, stored: bool) -> Result<Option<(Vec<(String, i32)>, bool)>> {
    let type_id = if stored { STORED_ENCHANTMENTS } else { ENCHANTMENTS };
    match stack.effective_component_bytes(type_id) {
        None => Ok(None),
        Some(bytes) => decode(bytes).map(Some),
    }
}

/// Enchantments of a stack (empty when it has no such component).
pub fn item_enchantments(stack: &ItemStack, stored: bool) -> Result<Vec<(String, i32)>> {
    Ok(read_component(stack, stored)?.map(|(entries, _)| entries).unwrap_or_default())
}

/// `EnchantmentHelper.getItemEnchantmentLevel(holder, stack)`: ordinary
/// enchantments only; stored (book) enchantments do not count.
pub fn item_enchantment_level(stack: &ItemStack, enchantment: &str) -> Result<i32> {
    Ok(item_enchantments(stack, false)?
        .iter()
        .find(|(name, _)| name == enchantment)
        .map_or(0, |(_, level)| *level))
}

/// `EnchantmentHelper.getEnchantmentLevel(holder, entity)`: the maximum level on
/// the stacks in the slots the enchantment is active in.
pub fn entity_enchantment_level(entity: &dyn LootEntity, enchantment: &str) -> Result<i32> {
    let Some(definition) = facts().enchantment(enchantment) else {
        return Err(LootError::Host(format!("unknown enchantment {enchantment}")));
    };
    let mut best = 0;
    for slot in EquipmentSlot::ALL {
        if !definition.matches_slot(slot) {
            continue;
        }
        let stack = entity.equipment(slot)?;
        if stack.is_empty() {
            continue;
        }
        best = best.max(item_enchantment_level(&stack, enchantment)?);
    }
    Ok(best)
}

/// `ItemEnchantments.Mutable`.
pub struct Mutable {
    entries: Vec<(String, i32)>,
    show_in_tooltip: bool,
}

impl Mutable {
    pub fn get_level(&self, enchantment: &str) -> i32 {
        self.entries
            .iter()
            .find(|(name, _)| name == enchantment)
            .map_or(0, |(_, level)| *level)
    }

    /// `Mutable.set`: a level of zero or less removes the entry.
    pub fn set(&mut self, enchantment: &str, level: i32) {
        if level <= 0 {
            self.entries.retain(|(name, _)| name != enchantment);
        } else if let Some(entry) = self.entries.iter_mut().find(|(name, _)| name == enchantment) {
            entry.1 = level;
        } else {
            self.entries.push((enchantment.to_owned(), level));
        }
    }

    /// `Mutable.upgrade`: keep the larger level of a positive request.
    pub fn upgrade(&mut self, enchantment: &str, level: i32) {
        if level > 0 {
            let current = self.get_level(enchantment);
            self.set(enchantment, current.max(level));
        }
    }
}

/// `stack.is(Items.ENCHANTED_BOOK)`.
pub fn is_enchanted_book(stack: &ItemStack) -> bool {
    stack_item_id(stack) == "minecraft:enchanted_book"
}

/// `EnchantmentHelper.updateEnchantments`: a missing component is left alone.
pub fn update_enchantments(
    stack: &mut ItemStack,
    update: impl FnOnce(&mut Mutable) -> Result<()>,
) -> Result<()> {
    let stored = is_enchanted_book(stack);
    let Some((entries, show_in_tooltip)) = read_component(stack, stored)? else {
        return Ok(());
    };
    let mut mutable = Mutable {
        entries,
        show_in_tooltip,
    };
    update(&mut mutable)?;
    let data = encode(&mutable.entries, mutable.show_in_tooltip)?;
    set_component(stack, if stored { STORED_ENCHANTMENTS } else { ENCHANTMENTS }, data);
    Ok(())
}

/// `ItemStack.enchant(holder, level)`.
pub fn enchant(stack: &mut ItemStack, enchantment: &str, level: i32) -> Result<()> {
    update_enchantments(stack, |mutable| {
        mutable.upgrade(enchantment, level);
        Ok(())
    })
}

/// `EnchantmentInstance`: a candidate (registry index) at a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnchantmentInstance {
    pub index: usize,
    pub level: i32,
}

/// `WeightedRandom.getRandomItem(random, list)`.
fn weighted_random(
    random: &mut dyn RandomSource,
    list: &[EnchantmentInstance],
) -> Result<Option<EnchantmentInstance>> {
    let weight = |instance: &EnchantmentInstance| facts().enchantments[instance.index].weight;
    let total: i64 = list.iter().map(|instance| i64::from(weight(instance))).sum();
    if total > i64::from(i32::MAX) {
        return Err(LootError::Invalid(
            "Sum of weights must be <= 2147483647".to_owned(),
        ));
    }
    let total = total as i32;
    if total < 0 {
        return Err(LootError::Invalid("Negative total weight in getRandomItem".to_owned()));
    }
    if total == 0 {
        return Ok(None);
    }
    let mut remaining = random.next_bounded_i32(total)?;
    for instance in list {
        remaining = remaining.wrapping_sub(weight(instance));
        if remaining < 0 {
            return Ok(Some(*instance));
        }
    }
    Ok(None)
}

/// `Enchantment.areCompatible`.
fn are_compatible(first: usize, second: usize) -> bool {
    let a = &facts().enchantments[first];
    let b = &facts().enchantments[second];
    first != second && !a.excludes(&b.id) && !b.excludes(&a.id)
}

/// `EnchantmentHelper.getAvailableEnchantmentResults`.
pub fn available_results(level: i32, stack: &ItemStack, candidates: &[usize]) -> Vec<EnchantmentInstance> {
    let is_book = stack_item_id(stack) == "minecraft:book";
    let item = stack_item_id(stack);
    let mut results = Vec::new();
    for &index in candidates {
        let enchantment = &facts().enchantments[index];
        if !(enchantment.is_primary_item(&item) || is_book) {
            continue;
        }
        let mut candidate_level = enchantment.max_level;
        while candidate_level >= 1 {
            if level >= enchantment.min_cost_at(candidate_level)
                && level <= enchantment.max_cost_at(candidate_level)
            {
                results.push(EnchantmentInstance {
                    index,
                    level: candidate_level,
                });
                break;
            }
            candidate_level -= 1;
        }
    }
    results
}

/// `EnchantmentHelper.selectEnchantment`.
pub fn select_enchantment(
    random: &mut dyn RandomSource,
    stack: &ItemStack,
    level: i32,
    candidates: &[usize],
) -> Result<Vec<EnchantmentInstance>> {
    let mut chosen = Vec::new();
    let value = facts().item_enchantment_value(&stack_item_id(stack));
    if value <= 0 {
        return Ok(chosen);
    }
    let bound = (value / 4).wrapping_add(1);
    let first = random.next_bounded_i32(bound)?;
    let second = random.next_bounded_i32(bound)?;
    let mut level = level.wrapping_add(1i32.wrapping_add(first).wrapping_add(second));
    let spread = (random.next_f32() + random.next_f32() - 1.0f32) * 0.15f32;
    let scaled = level as f32 + level as f32 * spread;
    level = mth::clamp_i32(mth::round(scaled), 1, i32::MAX);
    let mut available = available_results(level, stack, candidates);
    if !available.is_empty() {
        if let Some(instance) = weighted_random(random, &available)? {
            chosen.push(instance);
        }
        while random.next_bounded_i32(50)? <= level {
            if let Some(last) = chosen.last().copied() {
                available.retain(|instance| are_compatible(last.index, instance.index));
            }
            if available.is_empty() {
                break;
            }
            if let Some(instance) = weighted_random(random, &available)? {
                chosen.push(instance);
            }
            level /= 2;
        }
    }
    Ok(chosen)
}

/// Registry indices of an `options` holder set, or every registered enchantment.
pub fn option_indices(options: Option<&super::facts::HolderSet>) -> Vec<usize> {
    match options {
        Some(set) => set
            .members(Kind::Enchantment)
            .iter()
            .filter_map(|id| facts().enchantment_id(id))
            .collect(),
        None => (0..facts().enchantments.len()).collect(),
    }
}

/// `EnchantmentHelper.enchantItem(random, stack, levels, candidates)`.
pub fn enchant_item(
    random: &mut dyn RandomSource,
    stack: ItemStack,
    levels: i32,
    candidates: &[usize],
) -> Result<ItemStack> {
    let selected = select_enchantment(random, &stack, levels, candidates)?;
    let mut stack = stack;
    if stack_item_id(&stack) == "minecraft:book" {
        stack = new_stack("minecraft:enchanted_book")?;
    }
    for instance in selected {
        enchant(&mut stack, &facts().enchantments[instance.index].id, instance.level)?;
    }
    Ok(stack)
}
