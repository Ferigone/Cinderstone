//! Java 1.21 enchanting offers and two-slot menu transactions.
//!
//! The item and lapis are temporary owned inputs; enchanting modifies the real
//! input in place and has no result-preview slot. Registry/holder, bookshelf,
//! player XP/seed/stat/advancement, listener and Level sound owners are supplied
//! explicitly. There is no synthetic default enchantment registry or inventory.

use crate::{
    crafting::RecipeBook, inventory::ItemStack, protocol::Reader, random::LegacyRandomSource,
};
use anyhow::{ensure, Result};
use std::fmt::Debug;

pub const MENU_TYPE: i32 = 13;
pub const ITEM_SLOT: usize = 0;
pub const LAPIS_SLOT: usize = 1;
pub const INPUT_SLOTS: usize = 2;
pub const MENU_SLOTS: usize = 38;
pub const DATA_SLOTS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnchantmentCost {
    pub base: i32,
    pub per_level_above_first: i32,
}
impl EnchantmentCost {
    pub fn at(self, level: i32) -> i32 {
        self.base.wrapping_add(
            self.per_level_above_first
                .wrapping_mul(level.wrapping_sub(1)),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnchantmentDefinition {
    pub weight: i32,
    pub max_level: i32,
    pub minimum_cost: EnchantmentCost,
    pub maximum_cost: EnchantmentCost,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnchantmentInstance<H> {
    pub holder: H,
    pub level: i32,
}

/// Actual RegistryAccess enchantment owner and item callbacks. Holder and
/// snapshot identities must belong to that owner, never a fabricated list or
/// lexicographic ordering. Queries must not consume player/Level randomness.
pub trait EnchantmentRegistry {
    type Holder: Clone + PartialEq + Debug;
    type Snapshot: Clone + PartialEq + Debug;
    fn snapshot(&self) -> Self::Snapshot;
    fn item_is_enchantable(&self, item: &ItemStack) -> Result<bool>;
    fn item_enchantment_value(&self, item: &ItemStack) -> Result<i32>;
    /// None means the actual #minecraft:in_enchanting_table tag is absent.
    /// Preserve its actual HolderSet encounter order.
    fn in_enchanting_table(&self) -> Result<Option<Vec<Self::Holder>>>;
    fn definition(&self, holder: &Self::Holder) -> Result<EnchantmentDefinition>;
    fn is_primary_item(&self, holder: &Self::Holder, item: &ItemStack) -> Result<bool>;
    /// Actual exclusive-set compatibility in both directions. Identity equality
    /// is handled before this query, matching Enchantment.areCompatible.
    fn are_compatible(&self, first: &Self::Holder, second: &Self::Holder) -> Result<bool>;
    fn protocol_id(&self, holder: &Self::Holder) -> Result<i32>;
    /// Actual ItemStack.enchant/updateEnchantments semantics, including holder
    /// map identity/order, upgrade(max old,new), and tooltip state. Unsupported
    /// component ownership must fail, not silently replace a component map.
    fn enchant_item(
        &self,
        item: &mut ItemStack,
        enchantment: &EnchantmentInstance<Self::Holder>,
    ) -> Result<()>;
}

/// Queries resolve the actual captured Level's block-state tags, relative to
/// the actual table position. Missing chunk/tag/context is an explicit error.
pub trait EnchantingBookshelves {
    fn is_power_provider(&self, offset: [i32; 3]) -> Result<bool>;
    fn is_power_transmitter(&self, offset: [i32; 3]) -> Result<bool>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnchantingPlayer {
    pub instabuild: bool,
    pub experience_level: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnchantingAction {
    ReplaceInput {
        slot: usize,
    },
    Enchant {
        option: usize,
        levels_and_lapis: i32,
    },
}

/// Implement on a staged transaction of the actual player/world/menu owners.
/// Preflight must establish every callback capability before mutation; on a
/// later error discard that entire host transaction along with the menu clone.
/// The callbacks here are source obligations, not optional no-op notifications.
pub trait EnchantingHost: EnchantmentRegistry + EnchantingBookshelves {
    fn player(&self) -> Result<EnchantingPlayer>;
    fn player_enchantment_seed(&self) -> Result<i32>;
    fn is_server_player(&self) -> bool;
    fn still_valid_table(&self, additional_range: f64) -> Result<bool>;
    fn preflight(&mut self, action: EnchantingAction) -> Result<()>;
    /// Actual Player.onEnchantmentPerformed, including the level subtraction,
    /// negative-level reset, PLAYER RNG seed refresh and ServerPlayer XP sync
    /// invalidation. It runs even for instabuild players.
    fn on_enchantment_performed(&mut self, original_item: &ItemStack, levels: i32) -> Result<()>;
    fn award_enchant_item_stat(&mut self) -> Result<()>;
    fn trigger_enchanted_item(&mut self, item: &ItemStack, levels: i32) -> Result<()>;
    /// SimpleContainer.setChanged listeners only; the native menu recomputation
    /// below follows this call. Do not recursively recompute a second preview.
    fn container_set_changed(&mut self, inputs: &[ItemStack; INPUT_SLOTS]) -> Result<()>;
    fn broadcast_changes(
        &mut self,
        inputs: &[ItemStack; INPUT_SLOTS],
        data: [i32; DATA_SLOTS],
    ) -> Result<()>;
    fn level_random_next_float(&mut self) -> Result<f32>;
    /// Actual Level.playSound(null, pos, ENCHANTMENT_TABLE_USE, BLOCKS, 1, pitch).
    fn play_enchanting_sound(&mut self, pitch: f32) -> Result<()>;
}

/// BlockPos.betweenClosed(-2,0,-2,2,1,2), x-fastest, then y, then z,
/// filtered by abs(x)==2 || abs(z)==2. All 32 positions are inspected.
pub fn bookshelf_offsets() -> Vec<[i32; 3]> {
    let mut offsets = Vec::with_capacity(32);
    for z in -2_i32..=2 {
        for y in 0..=1 {
            for x in -2_i32..=2 {
                if x.abs() == 2 || z.abs() == 2 {
                    offsets.push([x, y, z]);
                }
            }
        }
    }
    offsets
}
pub fn bookshelf_power(world: &impl EnchantingBookshelves) -> Result<i32> {
    let mut count = 0;
    for [x, y, z] in bookshelf_offsets() {
        if world.is_power_provider([x, y, z])? && world.is_power_transmitter([x / 2, y, z / 2])? {
            count += 1;
        }
    }
    Ok(count)
}

/// EnchantmentHelper.getEnchantmentCost. The bookshelf cap affects the cost,
/// not the world's actual bookshelf scan or its receiver/context validation.
pub fn enchantment_cost(
    random: &mut LegacyRandomSource,
    option: usize,
    shelves: i32,
    enchantability: i32,
) -> Result<i32> {
    ensure!(option < 3, "invalid enchanting option");
    ensure!(shelves >= 0, "negative enchanting bookshelf count");
    if enchantability <= 0 {
        return Ok(0);
    }
    let shelves = shelves.min(15);
    let base = random
        .next_bounded_i32(8)?
        .wrapping_add(1)
        .wrapping_add(shelves >> 1)
        .wrapping_add(random.next_bounded_i32(shelves + 1)?);
    Ok(match option {
        0 => (base / 3).max(1),
        1 => base.wrapping_mul(2) / 3 + 1,
        _ => base.max(shelves.wrapping_mul(2)),
    })
}

fn is_book(stack: &ItemStack) -> bool {
    !stack.is_empty() && RecipeBook::vanilla().item_id("book") == Some(stack.item_id)
}
fn is_enchantable(item: &ItemStack, registry: &impl EnchantmentRegistry) -> Result<bool> {
    if item.is_empty() || !registry.item_is_enchantable(item)? {
        return Ok(false);
    }
    let Some(bytes) = item.effective_component_bytes(9) else {
        return Ok(false);
    };
    let count = Reader::new(bytes).varint()?;
    ensure!(count >= 0, "negative enchantment component count");
    Ok(count == 0)
}

fn weighted_index<H>(
    random: &mut LegacyRandomSource,
    values: &[(EnchantmentInstance<H>, i32)],
) -> Result<Option<usize>> {
    let mut total = 0_i64;
    for (_, weight) in values {
        ensure!(*weight >= 0, "negative enchantment weight");
        total += i64::from(*weight);
    }
    ensure!(
        total <= i64::from(i32::MAX),
        "enchantment weight sum exceeds Integer.MAX_VALUE"
    );
    if total == 0 {
        return Ok(None);
    }
    let mut draw = random.next_bounded_i32(total as i32)?;
    for (index, (_, weight)) in values.iter().enumerate() {
        draw -= weight;
        if draw < 0 {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

/// Actual table tag and registry definitions feed native source selection;
/// no missing tag, holder, applicability or compatibility owner is invented.
pub fn select_enchantments<R: EnchantmentRegistry>(
    registry: &R,
    random: &mut LegacyRandomSource,
    item: &ItemStack,
    mut level: i32,
    ordered_holders: &[R::Holder],
) -> Result<Vec<EnchantmentInstance<R::Holder>>> {
    let enchantability = registry.item_enchantment_value(item)?;
    if enchantability <= 0 {
        return Ok(vec![]);
    }
    let bound = enchantability / 4 + 1;
    level = level.wrapping_add(
        1_i32
            .wrapping_add(random.next_bounded_i32(bound)?)
            .wrapping_add(random.next_bounded_i32(bound)?),
    );
    let spread = (random.next_f32() + random.next_f32() - 1.0_f32) * 0.15_f32;
    let adjusted = level as f32 + level as f32 * spread;
    // Math.round(float) rounds ties towards positive infinity; promotion before
    // adding 0.5 avoids an additional float-rounding error near a half boundary.
    level = ((f64::from(adjusted) + 0.5_f64).floor() as i32).max(1);
    let mut candidates = Vec::new();
    for holder in ordered_holders {
        let primary = registry.is_primary_item(holder, item)?;
        if !primary && !is_book(item) {
            continue;
        }
        let definition = registry.definition(holder)?;
        ensure!(
            definition.max_level >= 1 && definition.max_level <= 255,
            "invalid actual enchantment max level"
        );
        for current in (1..=definition.max_level).rev() {
            if level >= definition.minimum_cost.at(current)
                && level <= definition.maximum_cost.at(current)
            {
                candidates.push((
                    EnchantmentInstance {
                        holder: holder.clone(),
                        level: current,
                    },
                    definition.weight,
                ));
                break;
            }
        }
    }
    let mut selected = Vec::new();
    if candidates.is_empty() {
        return Ok(selected);
    }
    if let Some(index) = weighted_index(random, &candidates)? {
        selected.push(candidates[index].0.clone());
    }
    while random.next_bounded_i32(50)? <= level {
        if let Some(last) = selected.last() {
            let mut compatible = Vec::with_capacity(candidates.len());
            for candidate in candidates {
                if candidate.0.holder != last.holder
                    && registry.are_compatible(&candidate.0.holder, &last.holder)?
                {
                    compatible.push(candidate);
                }
            }
            candidates = compatible;
        }
        if candidates.is_empty() {
            break;
        }
        if let Some(index) = weighted_index(random, &candidates)? {
            selected.push(candidates[index].0.clone());
        }
        level /= 2;
    }
    Ok(selected)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnchantingQuickMove {
    ToPlayerReverse,
    ToLapis,
    OneToItem,
}

#[derive(Clone, Debug)]
pub struct EnchantingState<S> {
    snapshot: S,
    inputs: [ItemStack; INPUT_SLOTS],
    seed: i32,
    costs: [i32; 3],
    enchantment_clues: [i32; 3],
    level_clues: [i32; 3],
    random: LegacyRandomSource,
}
impl<S: Clone + PartialEq + Debug> EnchantingState<S> {
    /// Seed must be the actual player's captured getEnchantmentSeed(). The
    /// source menu RNG is reseeded before every cost/list generation; no random
    /// global/Level/player value substitutes for that seed.
    pub fn new<R: EnchantmentRegistry<Snapshot = S>>(seed: i32, registry: &R) -> Self {
        Self {
            snapshot: registry.snapshot(),
            inputs: std::array::from_fn(|_| ItemStack::empty()),
            seed,
            costs: [0; 3],
            enchantment_clues: [-1; 3],
            level_clues: [-1; 3],
            random: LegacyRandomSource::new(i64::from(seed)),
        }
    }
    pub(crate) fn install_created_menu_random(&mut self, random: LegacyRandomSource) {
        self.random = random;
    }
    pub fn inputs(&self) -> &[ItemStack; INPUT_SLOTS] {
        &self.inputs
    }
    pub fn seed(&self) -> i32 {
        self.seed
    }
    pub fn costs(&self) -> &[i32; 3] {
        &self.costs
    }
    pub fn enchantment_clues(&self) -> &[i32; 3] {
        &self.enchantment_clues
    }
    pub fn level_clues(&self) -> &[i32; 3] {
        &self.level_clues
    }
    pub fn random_state48(&self) -> u64 {
        self.random.state48()
    }
    pub fn data_values(&self) -> [i32; DATA_SLOTS] {
        [
            self.costs[0],
            self.costs[1],
            self.costs[2],
            self.seed,
            self.enchantment_clues[0],
            self.enchantment_clues[1],
            self.enchantment_clues[2],
            self.level_clues[0],
            self.level_clues[1],
            self.level_clues[2],
        ]
    }
    pub fn may_place(slot: usize, item: &ItemStack) -> bool {
        match slot {
            ITEM_SLOT => true,
            LAPIS_SLOT => {
                !item.is_empty()
                    && RecipeBook::vanilla().item_id("lapis_lazuli") == Some(item.item_id)
            }
            _ => false,
        }
    }
    pub fn slot_limit(slot: usize, item: &ItemStack) -> Result<i32> {
        Ok(match slot {
            ITEM_SLOT => 1,
            LAPIS_SLOT => item.max_stack_size()?.min(99),
            _ => 0,
        })
    }
    /// Source quickMoveStack routes: top slots to reversed player [2,38),
    /// lapis from any player slot to [1,2), or one other item into an empty
    /// slot0. It does not fall back to main-inventory/hotbar redistribution.
    pub fn quick_move_route(
        &self,
        menu_slot: usize,
        stack: &ItemStack,
    ) -> Option<EnchantingQuickMove> {
        if stack.is_empty() || menu_slot >= MENU_SLOTS {
            return None;
        }
        if menu_slot < INPUT_SLOTS {
            return Some(EnchantingQuickMove::ToPlayerReverse);
        }
        if Self::may_place(LAPIS_SLOT, stack) {
            return Some(EnchantingQuickMove::ToLapis);
        }
        self.inputs[ITEM_SLOT]
            .is_empty()
            .then_some(EnchantingQuickMove::OneToItem)
    }

    pub fn still_valid(&self, host: &impl EnchantingHost) -> Result<bool> {
        host.still_valid_table(4.0)
    }
    fn check_registry<R: EnchantmentRegistry<Snapshot = S>>(&self, registry: &R) -> Result<()> {
        ensure!(
            self.snapshot == registry.snapshot(),
            "enchanting registry snapshot changed; owner handoff required"
        );
        Ok(())
    }
    fn clear_offers(&mut self) {
        self.costs = [0; 3];
        self.enchantment_clues = [-1; 3];
        self.level_clues = [-1; 3];
    }
    fn enchantment_list<R: EnchantmentRegistry<Snapshot = S>>(
        &mut self,
        registry: &R,
        item: &ItemStack,
        option: usize,
        cost: i32,
    ) -> Result<Vec<EnchantmentInstance<R::Holder>>> {
        self.random
            .set_seed(i64::from(self.seed.wrapping_add(option as i32)));
        let Some(holders) = registry.in_enchanting_table()? else {
            return Ok(vec![]);
        };
        let mut selected = select_enchantments(registry, &mut self.random, item, cost, &holders)?;
        if is_book(item) && selected.len() > 1 {
            let index = self
                .random
                .next_bounded_i32(i32::try_from(selected.len())?)? as usize;
            selected.remove(index);
        }
        Ok(selected)
    }
    /// Return whether source slotsChanged invokes broadcastChanges. Clearing
    /// offers for an unenchantable/empty input does not broadcast inside it.
    fn compute_offers<R: EnchantmentRegistry<Snapshot = S> + EnchantingBookshelves>(
        &mut self,
        registry: &R,
    ) -> Result<bool> {
        self.check_registry(registry)?;
        let item = self.inputs[ITEM_SLOT].clone();
        if !is_enchantable(&item, registry)? {
            self.clear_offers();
            return Ok(false);
        }
        let shelves = bookshelf_power(registry)?;
        self.random.set_seed(i64::from(self.seed));
        for option in 0..3 {
            self.costs[option] = enchantment_cost(
                &mut self.random,
                option,
                shelves,
                registry.item_enchantment_value(&item)?,
            )?;
            self.enchantment_clues[option] = -1;
            self.level_clues[option] = -1;
            if self.costs[option] < option as i32 + 1 {
                self.costs[option] = 0;
            }
        }
        for option in 0..3 {
            if self.costs[option] <= 0 {
                continue;
            }
            let selected = self.enchantment_list(registry, &item, option, self.costs[option])?;
            if !selected.is_empty() {
                let index = self
                    .random
                    .next_bounded_i32(i32::try_from(selected.len())?)?
                    as usize;
                self.enchantment_clues[option] = registry.protocol_id(&selected[index].holder)?;
                self.level_clues[option] = selected[index].level;
            }
        }
        Ok(true)
    }
    fn changed<H: EnchantingHost<Snapshot = S>>(&mut self, host: &mut H) -> Result<()> {
        host.container_set_changed(&self.inputs)?;
        self.slots_changed(host)
    }
    fn slots_changed<H: EnchantingHost<Snapshot = S>>(&mut self, host: &mut H) -> Result<()> {
        if self.compute_offers(host)? {
            host.broadcast_changes(&self.inputs, self.data_values())?;
        }
        Ok(())
    }
    /// Stage a canonical slot replacement as part of the host's inventory/cursor
    /// transaction. Menu state is unchanged on error; host errors must discard
    /// the enclosing transaction too. The player's inventory remains borrowed.
    pub fn set_input<H: EnchantingHost<Snapshot = S>>(
        &mut self,
        slot: usize,
        item: ItemStack,
        host: &mut H,
    ) -> Result<()> {
        ensure!(slot < INPUT_SLOTS, "invalid enchanting input slot");
        ensure!(
            item.is_empty() || Self::may_place(slot, &item),
            "enchanting slot rejects item"
        );
        if !item.is_empty() {
            ensure!(
                item.count <= Self::slot_limit(slot, &item)?,
                "enchanting input exceeds slot capacity"
            );
            item.encode_canonical()?;
        }
        self.check_registry(host)?;
        host.preflight(EnchantingAction::ReplaceInput { slot })?;
        let mut next = self.clone();
        next.inputs[slot] = if item.is_empty() {
            ItemStack::empty()
        } else {
            item
        };
        next.changed(host)?;
        *self = next;
        Ok(())
    }
    /// In-place stack mutation for the native Slot adapter. No container dirty
    /// callback occurs until that adapter reaches the actual source boundary.
    pub(crate) fn mutate_input_in_place(&mut self, slot: usize, item: ItemStack) -> Result<()> {
        ensure!(slot < INPUT_SLOTS, "invalid enchanting input slot");
        self.inputs[slot] = if item.is_empty() {
            ItemStack::empty()
        } else {
            item
        };
        Ok(())
    }
    pub(crate) fn notify_input_changed<H: EnchantingHost<Snapshot = S>>(
        &mut self,
        host: &mut H,
    ) -> Result<()> {
        let mut next = self.clone();
        next.changed(host)?;
        *self = next;
        Ok(())
    }

    /// Native source action on a host-staged real-player/world transaction.
    /// An accepted option with an empty actual enchantment list returns true
    /// without charging levels/lapis, awarding stats or playing a sound.
    pub fn click_menu_button<H: EnchantingHost<Snapshot = S>>(
        &mut self,
        button: i32,
        host: &mut H,
    ) -> Result<bool> {
        if !(0..3).contains(&button) {
            return Ok(false);
        }
        self.check_registry(host)?;
        let option = button as usize;
        let payment = button + 1;
        let player = host.player()?;
        let lapis = &self.inputs[LAPIS_SLOT];
        if (lapis.is_empty() || lapis.count < payment) && !player.instabuild {
            return Ok(false);
        }
        if self.costs[option] <= 0
            || self.inputs[ITEM_SLOT].is_empty()
            || ((player.experience_level < payment || player.experience_level < self.costs[option])
                && !player.instabuild)
        {
            return Ok(false);
        }
        let mut next = self.clone();
        let original = next.inputs[ITEM_SLOT].clone();
        let selected = next.enchantment_list(host, &original, option, next.costs[option])?;
        if selected.is_empty() {
            *self = next;
            return Ok(true);
        }
        host.preflight(EnchantingAction::Enchant {
            option,
            levels_and_lapis: payment,
        })?;
        host.on_enchantment_performed(&original, payment)?;
        if is_book(&original) {
            let mut converted = ItemStack::new(
                RecipeBook::vanilla()
                    .item_id("enchanted_book")
                    .expect("pinned enchanted book"),
                original.count,
            )?;
            // ItemStack.transmuteCopy copies the component patch, not the old
            // prototype's entire effective component map.
            converted.components = original.components.clone();
            converted.removed_components = original.removed_components.clone();
            next.inputs[ITEM_SLOT] = converted;
            next.changed(host)?;
        }
        for enchantment in &selected {
            host.enchant_item(&mut next.inputs[ITEM_SLOT], enchantment)?;
        }
        if !host.player()?.instabuild {
            next.inputs[LAPIS_SLOT].count -= payment;
        }
        if next.inputs[LAPIS_SLOT].is_empty() {
            next.inputs[LAPIS_SLOT] = ItemStack::empty();
            next.changed(host)?;
        }
        host.award_enchant_item_stat()?;
        if host.is_server_player() {
            host.trigger_enchanted_item(&next.inputs[ITEM_SLOT], payment)?;
        }
        next.changed(host)?;
        next.seed = host.player_enchantment_seed()?;
        next.slots_changed(host)?;
        let pitch = host.level_random_next_float()? * 0.1_f32 + 0.9_f32;
        host.play_enchanting_sound(pitch)?;
        *self = next;
        Ok(true)
    }
    /// Only these two actual temporary inputs enter clearContainer. There is
    /// no output preview; the menu cursor belongs to shared close lifecycle.
    pub fn close_inputs(&mut self) -> [ItemStack; INPUT_SLOTS] {
        self.clear_offers();
        std::mem::replace(
            &mut self.inputs,
            std::array::from_fn(|_| ItemStack::empty()),
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{inventory::Component, protocol::Writer};
    use std::cell::RefCell;

    /// Restricted recording fixture, not a vanilla registry implementation.
    /// Unbreaking fields below are from the pinned packaged enchantment JSON;
    /// this deliberately supplies just one actual data entry to the algorithm.
    pub(crate) struct RecordingHost {
        snapshot: u64,
        missing_tag: bool,
        unsupported: bool,
        player: EnchantingPlayer,
        seed: i32,
        player_random: LegacyRandomSource,
        level_random: LegacyRandomSource,
        pub(crate) calls: RefCell<Vec<&'static str>>,
    }
    impl RecordingHost {
        pub(crate) fn new(instabuild: bool) -> Self {
            Self {
                snapshot: 7,
                missing_tag: false,
                unsupported: false,
                player: EnchantingPlayer {
                    instabuild,
                    experience_level: 30,
                },
                seed: 123456789,
                player_random: LegacyRandomSource::new(777),
                level_random: LegacyRandomSource::new(888),
                calls: RefCell::new(vec![]),
            }
        }
    }
    impl EnchantmentRegistry for RecordingHost {
        type Holder = &'static str;
        type Snapshot = u64;
        fn snapshot(&self) -> u64 {
            self.snapshot
        }
        fn item_is_enchantable(&self, stack: &ItemStack) -> Result<bool> {
            Ok(is_book(stack) && stack.count == 1)
        }
        fn item_enchantment_value(&self, _: &ItemStack) -> Result<i32> {
            Ok(1)
        }
        fn in_enchanting_table(&self) -> Result<Option<Vec<Self::Holder>>> {
            Ok((!self.missing_tag).then_some(vec!["minecraft:unbreaking"]))
        }
        fn definition(&self, holder: &Self::Holder) -> Result<EnchantmentDefinition> {
            assert_eq!(*holder, "minecraft:unbreaking");
            Ok(EnchantmentDefinition {
                weight: 5,
                max_level: 3,
                minimum_cost: EnchantmentCost {
                    base: 5,
                    per_level_above_first: 8,
                },
                maximum_cost: EnchantmentCost {
                    base: 55,
                    per_level_above_first: 8,
                },
            })
        }
        fn is_primary_item(&self, _: &Self::Holder, _: &ItemStack) -> Result<bool> {
            Ok(false)
        }
        fn are_compatible(&self, _: &Self::Holder, _: &Self::Holder) -> Result<bool> {
            unreachable!("single-holder fixture")
        }
        fn protocol_id(&self, _: &Self::Holder) -> Result<i32> {
            Ok(39)
        }
        fn enchant_item(
            &self,
            stack: &mut ItemStack,
            enchantment: &EnchantmentInstance<Self::Holder>,
        ) -> Result<()> {
            self.calls.borrow_mut().push("enchant");
            assert_eq!(
                RecipeBook::vanilla().item_id("enchanted_book"),
                Some(stack.item_id)
            );
            let mut wire = Writer::new();
            wire.varint(1)
                .varint(39)
                .varint(enchantment.level)
                .bool(true);
            stack.components.retain(|c| c.type_id != 23);
            stack.components.push(Component {
                type_id: 23,
                data: wire.into_inner(),
            });
            Ok(())
        }
    }
    impl EnchantingBookshelves for RecordingHost {
        fn is_power_provider(&self, _: [i32; 3]) -> Result<bool> {
            Ok(true)
        }
        fn is_power_transmitter(&self, _: [i32; 3]) -> Result<bool> {
            Ok(true)
        }
    }
    impl EnchantingHost for RecordingHost {
        fn player(&self) -> Result<EnchantingPlayer> {
            Ok(self.player)
        }
        fn player_enchantment_seed(&self) -> Result<i32> {
            self.calls.borrow_mut().push("seed");
            Ok(self.seed)
        }
        fn is_server_player(&self) -> bool {
            true
        }
        fn still_valid_table(&self, range: f64) -> Result<bool> {
            assert_eq!(range, 4.0);
            Ok(true)
        }
        fn preflight(&mut self, _: EnchantingAction) -> Result<()> {
            ensure!(!self.unsupported, "unsupported fixture owner");
            Ok(())
        }
        fn on_enchantment_performed(&mut self, _: &ItemStack, levels: i32) -> Result<()> {
            self.calls.borrow_mut().push("performed");
            self.player.experience_level = (self.player.experience_level - levels).max(0);
            self.seed = self.player_random.next_i32();
            Ok(())
        }
        fn award_enchant_item_stat(&mut self) -> Result<()> {
            self.calls.borrow_mut().push("stat");
            Ok(())
        }
        fn trigger_enchanted_item(&mut self, stack: &ItemStack, levels: i32) -> Result<()> {
            self.calls.borrow_mut().push("criterion");
            assert_eq!(levels, 3);
            assert!(stack.effective_component_bytes(23).is_some());
            Ok(())
        }
        fn container_set_changed(&mut self, _: &[ItemStack; 2]) -> Result<()> {
            self.calls.borrow_mut().push("container");
            Ok(())
        }
        fn broadcast_changes(&mut self, _: &[ItemStack; 2], _: [i32; 10]) -> Result<()> {
            self.calls.borrow_mut().push("broadcast");
            Ok(())
        }
        fn level_random_next_float(&mut self) -> Result<f32> {
            self.calls.borrow_mut().push("level_random");
            Ok(self.level_random.next_f32())
        }
        fn play_enchanting_sound(&mut self, pitch: f32) -> Result<()> {
            self.calls.borrow_mut().push("sound");
            assert!((0.9..1.0).contains(&pitch));
            Ok(())
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }

    #[test]
    fn costs_and_draw_order_match_unchanged_java_helper_observation() -> Result<()> {
        // Direct original-JAR EnchantmentHelper.getEnchantmentCost on a book,
        // fifteen shelves, followed by the same LegacyRandomSource.nextInt.
        for (seed, expected, next) in [
            (0_i64, [8, 13, 30], -1930858313),
            (123456789_i64, [8, 11, 30], -458096230),
            (-123456789_i64, [7, 18, 30], -619273243),
        ] {
            let mut random = LegacyRandomSource::new(seed);
            let actual = [
                enchantment_cost(&mut random, 0, 15, 1)?,
                enchantment_cost(&mut random, 1, 15, 1)?,
                enchantment_cost(&mut random, 2, 15, 1)?,
            ];
            assert_eq!(actual, expected);
            assert_eq!(random.next_i32(), next);
        }
        // The unchanged helper was also called with the actual bootstrap
        // Unbreaking holder in a deliberately restricted single-holder Stream.
        // This is not an observation of the complete vanilla table tag.
        let registry = RecordingHost::new(false);
        for (option, cost, level, next) in [
            (0, 8, 1, -458096230),
            (1, 11, 2, -1947763183),
            (2, 30, 3, 1412103999),
        ] {
            let mut random = LegacyRandomSource::new(i64::from(123456789 + option));
            let selected = select_enchantments(
                &registry,
                &mut random,
                &item("book", 1),
                cost,
                &["minecraft:unbreaking"],
            )?;
            assert_eq!(
                selected,
                vec![EnchantmentInstance {
                    holder: "minecraft:unbreaking",
                    level
                }]
            );
            assert_eq!(random.next_i32(), next);
        }
        Ok(())
    }

    #[test]
    fn book_enchantment_preserves_owner_callback_order_and_consumes_lapis() -> Result<()> {
        let mut host = RecordingHost::new(false);
        let mut menu = EnchantingState::new(host.seed, &host);
        menu.set_input(ITEM_SLOT, item("book", 1), &mut host)?;
        let offers = menu.data_values();
        menu.set_input(LAPIS_SLOT, item("lapis_lazuli", 3), &mut host)?;
        assert_eq!(offers, menu.data_values());
        assert_eq!(menu.enchantment_clues()[2], 39);
        assert_eq!(menu.level_clues()[2], 3);
        assert_eq!(menu.costs()[2], 30);
        host.calls.borrow_mut().clear();
        assert!(menu.click_menu_button(2, &mut host)?);
        assert_eq!(
            *host.calls.borrow(),
            [
                "performed",
                "container",
                "enchant",
                "container",
                "stat",
                "criterion",
                "container",
                "seed",
                "level_random",
                "sound"
            ]
        );
        assert_eq!(host.player.experience_level, 27);
        assert_eq!(menu.seed(), host.seed);
        assert_eq!(menu.costs(), &[0; 3]);
        assert!(menu.inputs()[LAPIS_SLOT].is_empty());
        let returned = menu.close_inputs();
        assert_eq!(
            returned[ITEM_SLOT].item_id,
            RecipeBook::vanilla().item_id("enchanted_book").unwrap()
        );
        assert_eq!(returned[ITEM_SLOT].count, 1);
        Ok(())
    }

    #[test]
    fn creative_still_runs_player_cost_callback_and_missing_tag_is_not_fabricated() -> Result<()> {
        let mut host = RecordingHost::new(true);
        let mut menu = EnchantingState::new(host.seed, &host);
        menu.set_input(ITEM_SLOT, item("book", 1), &mut host)?;
        host.player.experience_level = 1;
        host.calls.borrow_mut().clear();
        assert!(menu.click_menu_button(2, &mut host)?);
        assert_eq!(host.player.experience_level, 0);
        assert!(host.calls.borrow().contains(&"performed"));
        assert!(menu.inputs()[LAPIS_SLOT].is_empty());
        let mut host = RecordingHost::new(false);
        host.missing_tag = true;
        let mut menu = EnchantingState::new(host.seed, &host);
        menu.set_input(ITEM_SLOT, item("book", 1), &mut host)?;
        menu.set_input(LAPIS_SLOT, item("lapis_lazuli", 3), &mut host)?;
        assert_eq!(menu.enchantment_clues(), &[-1; 3]);
        host.calls.borrow_mut().clear();
        assert!(menu.click_menu_button(2, &mut host)?);
        assert!(host.calls.borrow().is_empty());
        assert_eq!(menu.inputs()[LAPIS_SLOT].count, 3);
        assert_eq!(host.player.experience_level, 30);
        Ok(())
    }

    #[test]
    fn unsupported_owner_or_changed_registry_cannot_mutate_menu() -> Result<()> {
        let mut host = RecordingHost::new(false);
        let mut menu = EnchantingState::new(host.seed, &host);
        host.unsupported = true;
        assert!(menu
            .set_input(ITEM_SLOT, item("book", 1), &mut host)
            .is_err());
        assert!(menu.inputs()[ITEM_SLOT].is_empty());
        assert!(host.calls.borrow().is_empty());
        host.unsupported = false;
        host.snapshot = 8;
        assert!(menu
            .set_input(ITEM_SLOT, item("book", 1), &mut host)
            .is_err());
        assert!(menu.inputs()[ITEM_SLOT].is_empty());
        assert_eq!(
            menu.quick_move_route(2, &item("stone", 64)),
            Some(EnchantingQuickMove::OneToItem)
        );
        assert_eq!(
            menu.quick_move_route(2, &item("lapis_lazuli", 64)),
            Some(EnchantingQuickMove::ToLapis)
        );
        assert_eq!(bookshelf_offsets().len(), 32);
        Ok(())
    }
}
