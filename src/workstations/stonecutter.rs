//! Java 1.21 StonecutterMenu's recipe/input/result ownership.
//!
//! This is the workstation transaction core, not a second player inventory or
//! packet menu. The host owns the cursor, shared inventory, sync baselines and
//! actual block/player/recipe-reward callbacks. See the source ledger.
use crate::{
    crafting::{RecipeBook, RecipeInput},
    inventory::{self, ItemStack},
};
use anyhow::{anyhow, ensure, Result};

pub const MENU_TYPE: i32 = 24;
pub const INPUT_SLOT: usize = 0;
pub const RESULT_SLOT: usize = 1;
pub const MENU_SLOTS: usize = 38;

/// The actual recipe manager/registry and enabled-feature owners implement this
/// contract. Matching must return RecipeManager.getRecipesFor's description-ID
/// order, including its encounter-order tie behavior. This stage accepts the
/// ordinary, non-special stonecutting recipe serializer only.
pub trait StonecutterRecipes {
    fn matching_recipes(&self, input: &ItemStack) -> Result<Vec<String>>;
    fn assemble(&self, recipe_id: &str, input: &ItemStack) -> Result<ItemStack>;
    fn result_enabled(&self, result: &ItemStack) -> Result<bool>;
}

/// Pinned default-data adapter. Every shipped stonecutting result is a block
/// item, so its description ID is `block.minecraft.<item name>`. Equal result
/// keys are rejected rather than inventing recipe-manager encounter order.
/// `enabled` must consult the real level feature set in production.
pub struct PinnedStonecutterRecipes<F> {
    enabled: F,
}
impl<F> PinnedStonecutterRecipes<F> {
    pub fn new(enabled: F) -> Self {
        Self { enabled }
    }
}
impl<F: Fn(&ItemStack) -> Result<bool>> StonecutterRecipes for PinnedStonecutterRecipes<F> {
    fn matching_recipes(&self, input: &ItemStack) -> Result<Vec<String>> {
        if input.is_empty() {
            return Ok(vec![]);
        }
        let book = RecipeBook::vanilla();
        let recipe_input = RecipeInput::Single(input.clone());
        let mut matches = vec![];
        for id in book.recipe_ids() {
            if book.recipe_type(id) != Some("stonecutting") {
                continue;
            }
            let Some(result) = book.evaluate(id, &recipe_input)? else {
                continue;
            };
            ensure!(
                inventory::item_default_block_state(result.stack.item_id).is_some(),
                "non-block stonecutting result requires actual description-ID owner"
            );
            let name = inventory::item_name(result.stack.item_id)
                .ok_or_else(|| anyhow!("unknown stonecutting result item"))?;
            matches.push((format!("block.minecraft.{name}"), id.to_owned()));
        }
        matches.sort_by(|left, right| left.0.cmp(&right.0));
        ensure!(
            !matches.windows(2).any(|pair| pair[0].0 == pair[1].0),
            "equal stonecutting result descriptions require actual recipe-manager order"
        );
        Ok(matches.into_iter().map(|(_, id)| id).collect())
    }
    fn assemble(&self, recipe_id: &str, input: &ItemStack) -> Result<ItemStack> {
        let book = RecipeBook::vanilla();
        ensure!(
            book.recipe_type(recipe_id) == Some("stonecutting"),
            "not a pinned stonecutting recipe"
        );
        book.evaluate(recipe_id, &RecipeInput::Single(input.clone()))?
            .map(|result| result.stack)
            .ok_or_else(|| anyhow!("selected stonecutting recipe no longer matches"))
    }
    fn result_enabled(&self, result: &ItemStack) -> Result<bool> {
        (self.enabled)(result)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StonecutterState {
    input: ItemStack,
    // ResultContainer is a derived preview; only a successful take owns output.
    result: ItemStack,
    recipes: Vec<String>,
    selected: i32,
    remembered_item: i32,
    recipe_used: Option<String>,
    last_sound_time: i64,
    revision: u64,
}
impl Default for StonecutterState {
    fn default() -> Self {
        Self::new()
    }
}
impl StonecutterState {
    pub fn new() -> Self {
        Self {
            input: ItemStack::empty(),
            result: ItemStack::empty(),
            recipes: vec![],
            // DataSlot.standalone starts at Java's default zero. setupRecipeList
            // sets -1 when the first nonempty input changes its item identity.
            selected: 0,
            remembered_item: 0,
            recipe_used: None,
            last_sound_time: 0,
            revision: 0,
        }
    }
    pub fn input(&self) -> &ItemStack {
        &self.input
    }
    pub fn result_preview(&self) -> &ItemStack {
        &self.result
    }
    pub fn recipes(&self) -> &[String] {
        &self.recipes
    }
    pub fn selected_recipe_index(&self) -> i32 {
        self.selected
    }
    pub fn has_input_item(&self) -> bool {
        !self.input.is_empty() && !self.recipes.is_empty()
    }
    /// The ordinary input Slot accepts any stack; output never accepts inserts.
    pub fn may_place(slot: usize) -> bool {
        slot == INPUT_SLOT
    }
    /// Pick-all excludes ResultContainer even when its item matches the cursor.
    pub fn may_pick_all(slot: usize) -> bool {
        slot != RESULT_SLOT && slot < MENU_SLOTS
    }
    pub fn replace_input(
        &mut self,
        mut input: ItemStack,
        recipes: &impl StonecutterRecipes,
    ) -> Result<()> {
        if input.is_empty() {
            input = ItemStack::empty();
        } else {
            input.encode_canonical()?;
            input.count = input.count.min(99.min(input.max_stack_size()?));
        }
        let mut next = self.clone();
        next.input = input;
        next.slots_changed(recipes)?;
        next.advance_revision()?;
        *self = next;
        Ok(())
    }
    pub fn remove_input(
        &mut self,
        amount: i32,
        recipes: &impl StonecutterRecipes,
    ) -> Result<ItemStack> {
        let mut next = self.clone();
        let removed = next.remove_input_inner(amount, recipes)?;
        if !removed.is_empty() {
            next.advance_revision()?;
        }
        *self = next;
        Ok(removed)
    }
    fn remove_input_inner(
        &mut self,
        amount: i32,
        recipes: &impl StonecutterRecipes,
    ) -> Result<ItemStack> {
        if amount <= 0 || self.input.is_empty() {
            return Ok(ItemStack::empty());
        }
        let mut removed = self.input.clone();
        removed.count = amount.min(self.input.count);
        self.input.count -= removed.count;
        if self.input.is_empty() {
            self.input = ItemStack::empty();
        }
        self.slots_changed(recipes)?;
        Ok(removed)
    }
    fn slots_changed(&mut self, recipes: &impl StonecutterRecipes) -> Result<()> {
        let item = if self.input.is_empty() {
            0
        } else {
            self.input.item_id
        };
        // The original compares Item identity, not count or component equality.
        if item != self.remembered_item {
            self.remembered_item = item;
            self.recipes.clear();
            self.selected = -1;
            self.result = ItemStack::empty();
            if !self.input.is_empty() {
                self.recipes = recipes.matching_recipes(&self.input)?;
            }
        }
        Ok(())
    }
    /// Source clickMenuButton returns true even for an ignored invalid index.
    pub fn click_button(&mut self, index: i32, recipes: &impl StonecutterRecipes) -> Result<bool> {
        if index >= 0 && (index as usize) < self.recipes.len() {
            let mut next = self.clone();
            next.selected = index;
            next.setup_result(recipes)?;
            next.advance_revision()?;
            *self = next;
        }
        Ok(true)
    }
    fn setup_result(&mut self, recipes: &impl StonecutterRecipes) -> Result<()> {
        if let Some(id) = self.recipes.get(self.selected as usize) {
            let result = recipes.assemble(id, &self.input)?;
            if recipes.result_enabled(&result)? {
                self.recipe_used = Some(id.clone());
                self.result = result;
            } else {
                self.result = ItemStack::empty();
            }
        } else {
            self.result = ItemStack::empty();
        }
        Ok(())
    }
    /// Prepare a normal result-slot removal. ResultContainer.removeItem ignores
    /// requested count and removes the whole preview, including a right click.
    /// No player/world effects occur here: the host stages the ordered effects
    /// with its real owners, then commits `next` and the one owned output.
    pub fn prepare_take(
        &self,
        recipes: &impl StonecutterRecipes,
        world_game_time: i64,
    ) -> Result<Option<StonecutterTake>> {
        if self.result.is_empty() {
            return Ok(None);
        }
        let output = self.result.clone();
        self.prepare_take_inner(recipes, world_game_time, output.clone(), output)
            .map(Some)
    }
    /// Complete a successful quick-move after actual inventory insertion. The
    /// output preview may have a remainder. The source's onTake receives that
    /// remainder (possibly count zero), while exactly one input is consumed.
    /// `moved` must be the actual successful insertion count. Before attempting
    /// insertion, even if it fails, the host must run Item.onCraftedBy once.
    pub fn prepare_quick_take(
        &self,
        recipes: &impl StonecutterRecipes,
        world_game_time: i64,
        moved: i32,
    ) -> Result<StonecutterTake> {
        ensure!(
            moved > 0 && moved <= self.result.count && !self.result.is_empty(),
            "invalid stonecutter quick-move result count"
        );
        let mut output = self.result.clone();
        output.count = moved;
        let mut callback_stack = self.result.clone();
        callback_stack.count -= moved;
        self.prepare_take_inner(recipes, world_game_time, output, callback_stack)
    }
    fn prepare_take_inner(
        &self,
        recipes: &impl StonecutterRecipes,
        world_game_time: i64,
        output: ItemStack,
        callback_stack: ItemStack,
    ) -> Result<StonecutterTake> {
        ensure!(
            !self.input.is_empty(),
            "stonecutter result has no actual input"
        );
        let mut next = self.clone();
        next.result = ItemStack::empty();
        let mut effects = vec![StonecutterEffect::CraftedBy {
            amount: callback_stack.count,
            stack: callback_stack,
        }];
        if let Some(recipe_id) = next.recipe_used.take() {
            effects.push(StonecutterEffect::AwardUsedRecipe {
                recipe_id,
                input_before_consumption: self.input.clone(),
            });
        }
        next.remove_input_inner(1, recipes)?;
        next.setup_result(recipes)?;
        effects.push(StonecutterEffect::InputConsumedAndResultRecomputed);
        if next.last_sound_time != world_game_time {
            effects.push(StonecutterEffect::PlayTakeResultSound);
            next.last_sound_time = world_game_time;
        }
        next.advance_revision()?;
        Ok(StonecutterTake {
            expected: self.clone(),
            next,
            output,
            effects,
        })
    }
    /// Called after AbstractContainerMenu.removed handles the actual cursor.
    /// The host returns/drops this one input; it must never return the preview.
    pub fn remove_owned_input_for_close(&mut self) -> Result<ItemStack> {
        self.advance_revision()?;
        let input = std::mem::take(&mut self.input);
        self.result = ItemStack::empty();
        self.recipes.clear();
        self.selected = -1;
        self.remembered_item = 0;
        Ok(input)
    }
    fn advance_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow!("stonecutter revision exhausted"))?;
        Ok(())
    }
}

/// Callback snapshots are observational arguments, not additional owned items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StonecutterEffect {
    /// ItemStack.onCraftedBy(level, player, amount), including the count-zero
    /// callback reached by a successful full quick-move.
    CraftedBy { stack: ItemStack, amount: i32 },
    /// RecipeCrafted criterion, then non-special recipe-book award/clear.
    AwardUsedRecipe {
        recipe_id: String,
        input_before_consumption: ItemStack,
    },
    /// Publication boundary after the one owned input is consumed and the
    /// derived preview is recomputed, before the world sound callback.
    InputConsumedAndResultRecomputed,
    /// Actual ContainerLevelAccess world/block position; volume and pitch 1,
    /// BLOCKS sound category, no excluded player. No RNG draw is used.
    PlayTakeResultSound,
}
#[derive(Clone, Debug)]
pub struct StonecutterTake {
    expected: StonecutterState,
    next: StonecutterState,
    output: ItemStack,
    effects: Vec<StonecutterEffect>,
}
impl StonecutterTake {
    pub fn next_state(&self) -> &StonecutterState {
        &self.next
    }
    pub fn output_preview(&self) -> &ItemStack {
        &self.output
    }
    pub fn effects(&self) -> &[StonecutterEffect] {
        &self.effects
    }
    /// Consumes this plan once and rejects stale or replayed state. Callers
    /// stage real callback owners before committing the actual live state.
    pub fn commit(self, state: &mut StonecutterState) -> Result<StonecutterCommittedTake> {
        ensure!(*state == self.expected, "stale stonecutter take plan");
        *state = self.next;
        Ok(StonecutterCommittedTake {
            output: self.output,
            effects: self.effects,
        })
    }
}
pub struct StonecutterCommittedTake {
    pub output: ItemStack,
    pub effects: Vec<StonecutterEffect>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stack(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    #[test]
    fn selected_slab_take_consumes_one_input_and_close_never_owns_preview() {
        // The enabled predicate is a local fixture, not a production world.
        let recipes = PinnedStonecutterRecipes::new(|_: &ItemStack| Ok(true));
        let mut state = StonecutterState::new();
        assert_eq!(state.selected_recipe_index(), 0);
        state.replace_input(stack("stone", 3), &recipes).unwrap();
        assert_eq!(state.selected_recipe_index(), -1);
        let choice = state
            .recipes()
            .iter()
            .position(|id| id == "minecraft:stone_slab_from_stone_stonecutting")
            .unwrap() as i32;
        state.click_button(choice, &recipes).unwrap();
        state.replace_input(stack("stone", 2), &recipes).unwrap();
        assert_eq!(state.selected_recipe_index(), choice);
        let take = state.prepare_take(&recipes, 17).unwrap().unwrap();
        let mut committed_state = state.clone();
        take.clone().commit(&mut committed_state).unwrap();
        assert!(take.clone().commit(&mut committed_state).is_err());
        assert_eq!(take.output, stack("stone_slab", 2));
        assert_eq!(take.next.input(), &stack("stone", 1));
        assert!(matches!(
            take.effects.last(),
            Some(StonecutterEffect::PlayTakeResultSound)
        ));
        let second = take.next.prepare_take(&recipes, 17).unwrap().unwrap();
        assert!(!second
            .effects
            .contains(&StonecutterEffect::PlayTakeResultSound));
        assert!(second.next.input().is_empty());
        assert!(second.next.result_preview().is_empty());
        let mut closing = take.next;
        assert_eq!(
            closing.remove_owned_input_for_close().unwrap(),
            stack("stone", 1)
        );
        assert!(closing.result_preview().is_empty());
    }
}
