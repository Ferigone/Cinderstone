//! Native Java 1.21 AbstractFurnaceBlockEntity state machine.
//!
//! Derived from the pinned server's dpv serverTick/canBurn/burn/setItem methods,
//! dqg/drv fuel-duration overrides, and Fastutil 8.5.12's Object2IntOpenHashMap.
//! World writes, menu transactions, XP orbs, Math.random and advancement/recipe
//! publication stay with their actual runtime owners. Tick results describe the
//! required lit-state write; this module never substitutes a world or RNG.
use crate::{
    inventory::{self, ItemStack},
    smelting::{self, CookingKind, CookingRecipe},
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FurnaceKind {
    Furnace,
    BlastFurnace,
    Smoker,
}
impl FurnaceKind {
    pub fn cooking_kind(self) -> CookingKind {
        match self {
            Self::Furnace => CookingKind::Smelting,
            Self::BlastFurnace => CookingKind::Blasting,
            Self::Smoker => CookingKind::Smoking,
        }
    }
    pub fn block_name(self) -> &'static str {
        match self {
            Self::Furnace => "furnace",
            Self::BlastFurnace => "blast_furnace",
            Self::Smoker => "smoker",
        }
    }
}
fn initial_recipe_slots() -> Vec<Option<String>> {
    vec![None; 32]
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FurnaceState {
    pub kind: FurnaceKind,
    pub slots: [ItemStack; 3],
    pub lit_time: i32,
    pub lit_duration: i32,
    pub cooking_progress: i32,
    pub cooking_total_time: i32,
    pub recipes_used: BTreeMap<String, i32>,
    /// Actual open-addressed key layout. Counts live in recipes_used. Capacity
    /// survives clear(), and rehash encounter order affects later XP RNG order.
    #[serde(default = "initial_recipe_slots")]
    pub recipe_hash_slots: Vec<Option<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FurnaceTick {
    /// Any owned data changed, including menu progress counters.
    pub changed: bool,
    /// AbstractFurnaceBlockEntity's setChanged decision, distinct from counters.
    pub block_entity_changed: bool,
    /// The real world owner must set the block's LIT property with flags 3.
    pub lit_changed: Option<bool>,
    pub completed_recipe: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FurnaceRecipeReward {
    pub recipe_id: String,
    pub count: i32,
    pub experience: f32,
}

impl FurnaceState {
    pub fn new(kind: FurnaceKind) -> Self {
        Self {
            kind,
            slots: std::array::from_fn(|_| ItemStack::empty()),
            lit_time: 0,
            lit_duration: 0,
            cooking_progress: 0,
            cooking_total_time: 0,
            recipes_used: BTreeMap::new(),
            recipe_hash_slots: initial_recipe_slots(),
        }
    }
    pub fn is_lit(&self) -> bool {
        self.lit_time > 0
    }
    pub fn data(&self) -> [i32; 4] {
        [
            self.lit_time,
            self.lit_duration,
            self.cooking_progress,
            self.cooking_total_time,
        ]
    }
    pub fn burn_duration(&self, stack: &ItemStack) -> Result<i32> {
        let ticks = smelting::fuel_time(stack)?;
        Ok(if self.kind == FurnaceKind::Furnace {
            ticks
        } else {
            ticks / 2
        })
    }
    pub fn total_cook_time(&self) -> Result<i32> {
        Ok(
            smelting::recipe_for(self.kind.cooking_kind(), &self.slots[0])?
                .map(|recipe| recipe.cooking_time)
                .unwrap_or(200),
        )
    }
    pub fn validate(&self) -> Result<()> {
        for stack in &self.slots {
            if !stack.is_empty() {
                ensure!(
                    inventory::item_name(stack.item_id).is_some(),
                    "unknown furnace item"
                );
                stack.encode_canonical()?;
            }
        }
        self.validate_recipe_layout()
    }
    /// Container.setItem itself does not enforce Slot.mayPlace. Menus/hoppers
    /// perform that check before committing; this method also supports result
    /// slot writes and Java-compatible loaded inventory changes.
    pub fn set_slot(&mut self, slot: usize, mut stack: ItemStack) -> Result<()> {
        ensure!(slot < 3, "furnace slot out of range");
        let unchanged_input =
            !stack.is_empty() && self.slots[slot].same_item_same_components(&stack)?;
        if !stack.is_empty() {
            stack.encode_canonical()?;
            let maximum = 99.min(stack.max_stack_size()?);
            if stack.count > maximum {
                stack.count = maximum;
            }
        }
        if stack.is_empty() {
            stack = ItemStack::empty();
        }
        let cooking_time = if slot == 0 && !unchanged_input {
            Some(
                smelting::recipe_for(self.kind.cooking_kind(), &stack)?
                    .map(|recipe| recipe.cooking_time)
                    .unwrap_or(200),
            )
        } else {
            None
        };
        self.slots[slot] = stack;
        if let Some(total) = cooking_time {
            self.cooking_total_time = total;
            self.cooking_progress = 0;
        }
        Ok(())
    }
    pub fn can_place(&self, slot: usize, stack: &ItemStack) -> Result<bool> {
        ensure!(slot < 3, "furnace slot out of range");
        Ok(match slot {
            2 => false,
            1 => {
                smelting::is_fuel(stack)?
                    || (is_item(stack, "bucket") && !is_item(&self.slots[1], "bucket"))
            }
            _ => true,
        })
    }
    pub fn can_burn(&self) -> Result<bool> {
        let recipe = smelting::recipe_for(self.kind.cooking_kind(), &self.slots[0])?;
        self.can_burn_recipe(recipe)
    }
    fn can_burn_recipe(&self, recipe: Option<&CookingRecipe>) -> Result<bool> {
        if self.slots[0].is_empty() {
            return Ok(false);
        }
        let Some(recipe) = recipe else {
            return Ok(false);
        };
        if recipe.result.is_empty() {
            return Ok(false);
        }
        let output = &self.slots[2];
        if output.is_empty() {
            return Ok(true);
        }
        if !output.same_item_same_components(&recipe.result)? {
            return Ok(false);
        }
        // Original canBurn checks the existing count, not count+result.count.
        Ok(
            (output.count < 99 && output.count < output.max_stack_size()?)
                || output.count < recipe.result.max_stack_size()?,
        )
    }
    /// Run one actual block-entity tick. Clone staging means a failed component,
    /// recipe or continuation proof cannot partly consume fuel/input.
    pub fn tick(&mut self) -> Result<FurnaceTick> {
        self.validate()?;
        let mut staged = self.clone();
        let mut event = staged.tick_inner()?;
        event.changed = staged != *self;
        *self = staged;
        Ok(event)
    }
    fn tick_inner(&mut self) -> Result<FurnaceTick> {
        let was_lit = self.is_lit();
        let mut event = FurnaceTick::default();
        if self.is_lit() {
            self.lit_time -= 1;
        }
        let has_fuel = !self.slots[1].is_empty();
        let has_input = !self.slots[0].is_empty();
        if self.is_lit() || (has_fuel && has_input) {
            let recipe = if has_input {
                smelting::recipe_for(self.kind.cooking_kind(), &self.slots[0])?
            } else {
                None
            };
            if !self.is_lit() && self.can_burn_recipe(recipe)? {
                self.lit_time = self.burn_duration(&self.slots[1])?;
                self.lit_duration = self.lit_time;
                if self.is_lit() {
                    event.block_entity_changed = true;
                    if has_fuel {
                        let remainder = smelting::fuel_remainder(&self.slots[1])?;
                        shrink_one(&mut self.slots[1]);
                        if self.slots[1].is_empty() {
                            self.slots[1] = remainder;
                        }
                    }
                }
            }
            if self.is_lit() && self.can_burn_recipe(recipe)? {
                self.cooking_progress = self.cooking_progress.wrapping_add(1);
                if self.cooking_progress == self.cooking_total_time {
                    self.cooking_progress = 0;
                    self.cooking_total_time = self.total_cook_time()?;
                    if let Some(recipe) = recipe {
                        // Empty output copies the full default recipe stack;
                        // an occupied matching result grows exactly one.
                        if self.slots[2].is_empty() {
                            self.slots[2] = recipe.result.clone();
                        } else {
                            self.slots[2].count = self.slots[2].count.wrapping_add(1);
                        }
                        if is_item(&self.slots[0], "wet_sponge")
                            && is_item(&self.slots[1], "bucket")
                        {
                            self.slots[1] =
                                ItemStack::new(crate::item_use::item_id("water_bucket")?, 1)?;
                        }
                        shrink_one(&mut self.slots[0]);
                        self.record_recipe_used(&recipe.id, 1)?;
                        event.completed_recipe = Some(recipe.id.clone());
                    }
                    event.block_entity_changed = true;
                }
            } else {
                self.cooking_progress = 0;
            }
        } else if !self.is_lit() && self.cooking_progress > 0 {
            self.cooking_progress = self
                .cooking_progress
                .wrapping_sub(2)
                .max(0)
                .min(self.cooking_total_time);
        }
        if was_lit != self.is_lit() {
            event.block_entity_changed = true;
            event.lit_changed = Some(self.is_lit());
        }
        Ok(event)
    }
    /// Snapshot in the original Fastutil entry-iterator order. The caller must
    /// use actual Math.random, XP-orb, recipe book and advancement owners before
    /// commit_recipe_rewards; output extraction is not a direct XP grant.
    pub fn pending_recipe_rewards(&self) -> Result<Vec<FurnaceRecipeReward>> {
        self.validate_recipe_layout()?;
        let mut rewards = Vec::new();
        for id in self.recipe_hash_slots.iter().rev().flatten() {
            if let Some(recipe) = smelting::recipe_by_id(id)? {
                rewards.push(FurnaceRecipeReward {
                    recipe_id: id.clone(),
                    count: self.recipes_used[id],
                    experience: recipe.experience,
                });
            } else {
                ensure!(
                    crate::crafting::RecipeBook::vanilla()
                        .recipe_type(id)
                        .is_none(),
                    "furnace RecipesUsed references a non-cooking recipe"
                );
            }
        }
        Ok(rewards)
    }
    pub fn commit_recipe_rewards(&mut self) -> Result<()> {
        self.validate_recipe_layout()?;
        self.recipes_used.clear();
        self.recipe_hash_slots.fill(None); // Fastutil.clear never shrinks n.
        Ok(())
    }
    pub fn record_recipe_used(&mut self, recipe_id: &str, count: i32) -> Result<()> {
        let id = canonical_recipe_id(recipe_id)?;
        let previous = self.recipes_used.get(&id).copied().unwrap_or(0);
        self.put_recipe_count(id, previous.wrapping_add(count))
    }
    fn put_recipe_count(&mut self, id: String, count: i32) -> Result<()> {
        self.validate_recipe_layout()?;
        if let Some(old) = self.recipes_used.get_mut(&id) {
            *old = count;
            return Ok(());
        }
        ensure!(
            self.recipe_hash_slots.len() < (1 << 21),
            "furnace recipe history capacity exceeds supported save bound"
        );
        let before = self.recipes_used.len();
        insert_recipe_key(&mut self.recipe_hash_slots, id.clone());
        self.recipes_used.insert(id, count);
        // Fastutil's condition uses size++'s OLD value. Its arraySize argument
        // is the new size plus one, with default load factor .75.
        if before >= self.recipe_hash_slots.len() * 3 / 4 {
            let mut n = 2usize;
            while n * 3 < (self.recipes_used.len() + 1) * 4 {
                n *= 2;
            }
            let mut replacement = vec![None; n];
            for key in self.recipe_hash_slots.iter().rev().flatten() {
                insert_recipe_key(&mut replacement, key.clone());
            }
            self.recipe_hash_slots = replacement;
        }
        Ok(())
    }
    fn validate_recipe_layout(&self) -> Result<()> {
        let n = self.recipe_hash_slots.len();
        ensure!(
            n >= 32 && n.is_power_of_two() && n <= (1 << 21),
            "invalid furnace recipe-table capacity"
        );
        ensure!(
            self.recipes_used.len() <= n * 3 / 4,
            "furnace recipe-table load exceeds source invariant"
        );
        let mut keys = BTreeSet::new();
        for (slot, id) in self
            .recipe_hash_slots
            .iter()
            .enumerate()
            .filter_map(|(i, id)| id.as_ref().map(|id| (i, id)))
        {
            ensure!(
                canonical_recipe_id(id)? == *id && keys.insert(id.as_str()),
                "duplicate/noncanonical furnace recipe key"
            );
            ensure!(
                self.recipes_used.contains_key(id),
                "furnace recipe-table count missing"
            );
            let mut expected = recipe_mix(id) & (n - 1);
            while expected != slot {
                ensure!(
                    self.recipe_hash_slots[expected].is_some(),
                    "furnace recipe key has no source probe chain"
                );
                expected = (expected + 1) & (n - 1);
            }
        }
        ensure!(
            keys.len() == self.recipes_used.len(),
            "furnace RecipesUsed lacks actual hash continuation"
        );
        Ok(())
    }
    /// Reconstruct CompoundTag's Java HashMap keySet encounter order, then
    /// AbstractFurnaceBlockEntity.loadAdditional's ResourceLocation-keyed puts.
    pub fn load_recipes_used(&mut self, entries: Vec<(String, i32)>) -> Result<()> {
        let entries = java_compound_entries(entries)?;
        let mut staged = self.clone();
        staged.recipes_used.clear();
        staged.recipe_hash_slots = initial_recipe_slots();
        for (id, count) in entries {
            staged.put_recipe_count(canonical_recipe_id(&id)?, count)?;
        }
        *self = staged;
        Ok(())
    }
    /// Fastutil.forEach inserts into a new CompoundTag/Java HashMap; its keySet
    /// order is the NBT writer order. It is not BTreeMap lexical order.
    pub fn recipes_used_nbt_entries(&self) -> Result<Vec<(String, i32)>> {
        self.validate_recipe_layout()?;
        java_compound_entries(
            self.recipe_hash_slots
                .iter()
                .rev()
                .flatten()
                .map(|id| (id.clone(), self.recipes_used[id]))
                .collect(),
        )
    }
}

fn shrink_one(stack: &mut ItemStack) {
    stack.count = stack.count.wrapping_sub(1);
    if stack.is_empty() {
        *stack = ItemStack::empty();
    }
}
fn is_item(stack: &ItemStack, name: &str) -> bool {
    !stack.is_empty() && inventory::item_name(stack.item_id) == Some(name)
}
fn java_string_hash(text: &str) -> i32 {
    text.encode_utf16()
        .fold(0i32, |hash, c| hash.wrapping_mul(31).wrapping_add(c as i32))
}
fn canonical_recipe_id(text: &str) -> Result<String> {
    let (namespace, path) = text.split_once(':').unwrap_or(("minecraft", text));
    let namespace = if namespace.is_empty() {
        "minecraft"
    } else {
        namespace
    };
    ensure!(
        !path.is_empty()
            && namespace.bytes().all(|c| c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, b'_' | b'.' | b'-'))
            && path.bytes().all(|c| c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, b'_' | b'.' | b'-' | b'/')),
        "invalid furnace recipe resource location"
    );
    Ok(format!("{namespace}:{path}"))
}
fn recipe_mix(id: &str) -> usize {
    let (namespace, path) = id.split_once(':').expect("validated resource location");
    let hash = java_string_hash(namespace)
        .wrapping_mul(31)
        .wrapping_add(java_string_hash(path));
    let mixed = (hash as u32).wrapping_mul(0x9e37_79b9);
    (mixed ^ (mixed >> 16)) as usize
}
fn insert_recipe_key(table: &mut [Option<String>], id: String) {
    let mask = table.len() - 1;
    let mut slot = recipe_mix(&id) & mask;
    while table[slot].is_some() {
        slot = (slot + 1) & mask;
    }
    table[slot] = Some(id);
}

/// Java 21 HashMap<String,Integer> created by CompoundTag. Tree bins are outside
/// this bounded imported-NBT path, and are rejected rather than sorted/faked.
fn java_compound_entries(entries: Vec<(String, i32)>) -> Result<Vec<(String, i32)>> {
    ensure!(
        entries.len() <= 262_144,
        "furnace recipe NBT map exceeds supported bound"
    );
    let mut table: Vec<Vec<(String, i32)>> = vec![Vec::new(); 16];
    let mut size = 0usize;
    for (key, value) in entries {
        let hash = java_string_hash(&key) as u32;
        let slot = (hash ^ (hash >> 16)) as usize & (table.len() - 1);
        if let Some(entry) = table[slot].iter_mut().find(|(id, _)| id == &key) {
            entry.1 = value;
            continue;
        }
        table[slot].push((key, value));
        size += 1;
        let collision_resize = table[slot].len() >= 9 && table.len() < 64;
        if table[slot].len() >= 9 && table.len() >= 64 {
            bail!("furnace recipe NBT requires an unported Java HashMap tree bin");
        }
        if collision_resize || size > table.len() * 3 / 4 {
            let mut replacement = vec![Vec::new(); table.len() * 2];
            let mask = replacement.len() - 1;
            for bucket in table {
                for entry in bucket {
                    let hash = java_string_hash(&entry.0) as u32;
                    replacement[(hash ^ (hash >> 16)) as usize & mask].push(entry);
                }
            }
            table = replacement;
        }
    }
    Ok(table.into_iter().flatten().collect())
}
