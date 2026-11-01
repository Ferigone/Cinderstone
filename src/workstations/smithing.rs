//! Java 1.21 smithing input/result rules for the pinned default recipe data.
//!
//! This state owns three temporary inputs. `result()` is a derived preview;
//! it becomes an owned output only when a prepared take is committed. The
//! caller supplies both real RecipeManager iteration orders and owns player,
//! block access, cursor/click transactions, sync and execution of callbacks.
use crate::{
    crafting::{RecipeBook, RecipeInput},
    inventory::ItemStack,
};
use anyhow::{ensure, Result};
use std::collections::BTreeSet;

pub const TEMPLATE_SLOT: usize = 0;
pub const BASE_SLOT: usize = 1;
pub const ADDITION_SLOT: usize = 2;
pub const RESULT_SLOT: usize = 3;
pub const MENU_TYPE: i32 = 21;
pub const MENU_SLOT_COUNT: usize = 40;

/// Distinct orders from getAllRecipesFor (input rules/quick move) and
/// getRecipesFor (result selection, ordered by result description ID).
/// IDs resolve only the shipped recipe/trim registries; dynamic data is not
/// silently substituted with these defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmithingRecipeOrder {
    input_recipes: Vec<String>,
    result_recipes: Vec<String>,
}
impl SmithingRecipeOrder {
    pub fn new(input_recipes: Vec<String>, result_recipes: Vec<String>) -> Result<Self> {
        let book = RecipeBook::vanilla();
        let validate = |ids: &[String]| -> Result<BTreeSet<String>> {
            let mut seen = BTreeSet::new();
            for id in ids {
                ensure!(
                    matches!(
                        book.recipe_type(id),
                        Some("smithing_transform" | "smithing_trim")
                    ),
                    "smithing requires a known pinned smithing recipe: {id}"
                );
                ensure!(seen.insert(id.clone()), "duplicate smithing recipe: {id}");
            }
            Ok(seen)
        };
        ensure!(
            validate(&input_recipes)? == validate(&result_recipes)?,
            "smithing input and result orders must describe the same recipe snapshot"
        );
        Ok(Self {
            input_recipes,
            result_recipes,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmithingState {
    order: SmithingRecipeOrder,
    inputs: [ItemStack; 3],
    result: ItemStack,
    selected_recipe: Option<String>,
    recipe_used: Option<String>,
    revision: u64,
}

/// ItemCombinerMenu.quickMoveStack passes the result stack *after* insertion
/// to onTake. For the supported one-item smithing results that stack is empty;
/// the ItemStack.onCraftedBy amount is therefore zero on this branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmithingTakeKind {
    Pickup,
    QuickMove,
}

/// Ordered calls to the actual item/player/world owners. Input snapshots here
/// are callback arguments, not an additional inventory or owned items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SmithingEffect {
    CraftedBy {
        stack: ItemStack,
        amount: i32,
    },
    RecipeCrafted {
        recipe_id: String,
        inputs: [ItemStack; 3],
    },
    AwardRecipe {
        recipe_id: String,
    },
    LevelEvent {
        event: i32,
        data: i32,
    },
}

/// A take is prepared before the real inventory/cursor and callback owners
/// commit their transaction. It is intentionally not Clone. A stale plan
/// cannot consume a newer input state, and dropping a plan changes no inputs.
#[derive(Debug)]
pub struct SmithingTakePlan {
    source: SmithingState,
    next: SmithingState,
    output: ItemStack,
    effects: Vec<SmithingEffect>,
}
impl SmithingTakePlan {
    pub fn output(&self) -> &ItemStack {
        &self.output
    }
    pub fn effects(&self) -> &[SmithingEffect] {
        &self.effects
    }
    /// The enclosing host must stage/validate every real callback before
    /// publishing this state and execute the effects in their recorded order.
    /// In particular the recipe criterion sees the inputs before consumption.
    pub fn commit(self, state: &mut SmithingState) -> Result<(ItemStack, Vec<SmithingEffect>)> {
        ensure!(*state == self.source, "stale smithing take plan");
        *state = self.next;
        Ok((self.output, self.effects))
    }
}

impl SmithingState {
    pub fn new(order: SmithingRecipeOrder) -> Self {
        Self {
            order,
            inputs: std::array::from_fn(|_| ItemStack::empty()),
            result: ItemStack::empty(),
            selected_recipe: None,
            recipe_used: None,
            revision: 0,
        }
    }
    pub fn inputs(&self) -> &[ItemStack; 3] {
        &self.inputs
    }
    pub fn result(&self) -> &ItemStack {
        &self.result
    }
    pub fn selected_recipe(&self) -> Option<&str> {
        self.selected_recipe.as_deref()
    }

    pub fn may_place(&self, slot: usize, stack: &ItemStack) -> Result<bool> {
        if slot >= RESULT_SLOT {
            return Ok(false);
        }
        let book = RecipeBook::vanilla();
        for id in &self.order.input_recipes {
            if book.smithing_accepts_input(id, slot, stack)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A UI input replacement; the actual inventory/cursor transfer belongs
    /// to the surrounding transaction. No player inventory is retained here.
    pub fn set_input(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        ensure!(slot < RESULT_SLOT, "invalid smithing input slot");
        ensure!(
            stack.is_empty() || self.may_place(slot, &stack)?,
            "invalid smithing ingredient slot"
        );
        if !stack.is_empty() {
            stack.encode_canonical()?;
            ensure!(
                stack.count <= stack.max_stack_size()?.min(99),
                "smithing input exceeds slot limit"
            );
        }
        let mut next = self.clone();
        next.inputs[slot] = canonical_empty(stack);
        next.create_result()?;
        next.revision = next.revision.wrapping_add(1);
        *self = next;
        Ok(())
    }

    /// Source mayPickup ignores its hasItem argument and rechecks the selected
    /// recipe. Taking still additionally requires a nonempty result slot.
    pub fn may_pickup(&self) -> Result<bool> {
        let Some(id) = self.selected_recipe.as_deref() else {
            return Ok(false);
        };
        Ok(RecipeBook::vanilla()
            .evaluate(id, &self.recipe_input())?
            .is_some())
    }

    /// findSlotToQuickMoveTo scans the constructor's captured recipe list.
    /// Per recipe, template wins over base over addition; the chosen slot must
    /// be empty. A matching occupied slot is not considered a merge target.
    pub fn quick_move_input_slot(&self, stack: &ItemStack) -> Result<Option<usize>> {
        let book = RecipeBook::vanilla();
        for id in &self.order.input_recipes {
            for slot in 0..RESULT_SLOT {
                if book.smithing_accepts_input(id, slot, stack)? {
                    if self.inputs[slot].is_empty() {
                        return Ok(Some(slot));
                    }
                    break;
                }
            }
        }
        Ok(None)
    }

    /// Smithing excludes only its derived result container from PICKUP_ALL.
    pub fn can_take_for_pick_all(slot: usize) -> bool {
        slot < MENU_SLOT_COUNT && slot != RESULT_SLOT
    }

    pub fn prepare_take(&self, kind: SmithingTakeKind) -> Result<SmithingTakePlan> {
        ensure!(
            !self.result.is_empty() && self.may_pickup()?,
            "no smithing result to take"
        );
        // Shipped smithing recipes always assemble exactly one item. Partial
        // custom output transfers require their own actual recipe owner.
        ensure!(
            self.result.count == 1,
            "unsupported non-default smithing output count"
        );
        let output = self.result.clone();
        let callback_stack = if kind == SmithingTakeKind::QuickMove {
            ItemStack::empty()
        } else {
            output.clone()
        };
        let mut effects = vec![SmithingEffect::CraftedBy {
            amount: callback_stack.count,
            stack: callback_stack,
        }];
        if let Some(id) = &self.recipe_used {
            effects.push(SmithingEffect::RecipeCrafted {
                recipe_id: id.clone(),
                inputs: self.inputs.clone(),
            });
            // Both shipped smithing serializers inherit Recipe.isSpecial=false.
            effects.push(SmithingEffect::AwardRecipe {
                recipe_id: id.clone(),
            });
        }
        let mut next = self.clone();
        next.result = ItemStack::empty();
        next.recipe_used = None;
        // Each SimpleContainer.setItem synchronously invokes slotsChanged,
        // so source createResult runs between the three decrements.
        for slot in 0..RESULT_SLOT {
            if !next.inputs[slot].is_empty() {
                next.inputs[slot].count -= 1;
                next.inputs[slot] = canonical_empty(std::mem::take(&mut next.inputs[slot]));
                next.create_result()?;
            }
        }
        effects.push(SmithingEffect::LevelEvent {
            event: 1044,
            data: 0,
        });
        next.revision = next.revision.wrapping_add(1);
        Ok(SmithingTakePlan {
            source: self.clone(),
            next,
            output,
            effects,
        })
    }

    /// Called after AbstractContainerMenu.removed has handled the actual
    /// cursor. Return/drop only these inputs, in index order; never result().
    /// The caller supplies the real alive/disconnected insertion/drop owners.
    pub fn drain_inputs_on_close(&mut self) -> [ItemStack; 3] {
        let owned = std::mem::replace(
            &mut self.inputs,
            std::array::from_fn(|_| ItemStack::empty()),
        );
        self.result = ItemStack::empty();
        self.selected_recipe = None;
        self.recipe_used = None;
        self.revision = self.revision.wrapping_add(1);
        owned
    }

    fn recipe_input(&self) -> RecipeInput {
        RecipeInput::Smithing {
            template: self.inputs[0].clone(),
            base: self.inputs[1].clone(),
            addition: self.inputs[2].clone(),
        }
    }
    fn create_result(&mut self) -> Result<()> {
        let book = RecipeBook::vanilla();
        let input = self.recipe_input();
        for id in &self.order.result_recipes {
            if let Some(crafted) = book.evaluate(id, &input)? {
                // The bounded pinned catalog has only ordinary enabled 1.21
                // items. Alternate feature sets/trim registries are not inputs
                // to this API and require a real registry/feature owner.
                self.selected_recipe = Some(id.clone());
                self.recipe_used = Some(id.clone());
                self.result = crafted.stack;
                return Ok(());
            }
        }
        // Source clears only result contents on no match; its selected recipe
        // and recipeUsed can remain, with mayPickup doing a fresh match check.
        self.result = ItemStack::empty();
        Ok(())
    }
}
fn canonical_empty(stack: ItemStack) -> ItemStack {
    if stack.is_empty() {
        ItemStack::empty()
    } else {
        stack
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Component;

    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn order(ids: &[&str]) -> Result<SmithingRecipeOrder> {
        let ids = ids
            .iter()
            .map(|s| format!("minecraft:{s}"))
            .collect::<Vec<_>>();
        SmithingRecipeOrder::new(ids.clone(), ids)
    }
    #[test]
    fn transform_take_is_owned_once_and_uses_preconsumption_inputs() -> Result<()> {
        let mut state = SmithingState::new(order(&["netherite_sword_smithing"])?);
        state.set_input(0, item("netherite_upgrade_smithing_template", 2))?;
        let mut sword = item("diamond_sword", 1);
        sword.components.push(Component {
            type_id: 3,
            data: vec![7],
        });
        state.set_input(1, sword)?;
        state.set_input(2, item("netherite_ingot", 3))?;
        assert_eq!(state.result().item_id, item("netherite_sword", 1).item_id);
        assert_eq!(state.result().damage_value()?, 7);
        assert!(state
            .quick_move_input_slot(&item("netherite_ingot", 1))?
            .is_none());
        let plan = state.prepare_take(SmithingTakeKind::QuickMove)?;
        assert!(
            matches!(&plan.effects()[0], SmithingEffect::CraftedBy { stack, amount: 0 } if stack.is_empty())
        );
        assert!(
            matches!(&plan.effects()[1], SmithingEffect::RecipeCrafted { inputs, .. } if inputs[0].count == 2 && inputs[2].count == 3)
        );
        // A prepared output is still only a prospective transfer. Inputs stay
        // unchanged until the one-shot plan commits.
        assert_eq!(state.inputs()[2].count, 3);
        let stale = state.prepare_take(SmithingTakeKind::Pickup)?;
        let (owned, effects) = plan.commit(&mut state)?;
        assert_eq!(owned.count, 1);
        assert_eq!(
            effects.last(),
            Some(&SmithingEffect::LevelEvent {
                event: 1044,
                data: 0
            })
        );
        assert!(stale.commit(&mut state).is_err());
        assert!(state.result().is_empty());
        let returned = state.drain_inputs_on_close();
        assert_eq!(returned[0].count, 1);
        assert!(returned[1].is_empty());
        assert_eq!(returned[2].count, 2);
        Ok(())
    }
    #[test]
    fn repeated_trim_matches_without_a_takeable_result() -> Result<()> {
        let mut state = SmithingState::new(order(&[
            "coast_armor_trim_smithing_template_smithing_trim",
        ])?);
        state.set_input(0, item("coast_armor_trim_smithing_template", 2))?;
        state.set_input(1, item("diamond_chestplate", 1))?;
        state.set_input(2, item("redstone", 2))?;
        let trimmed = state
            .prepare_take(SmithingTakeKind::Pickup)?
            .commit(&mut state)?
            .0;
        assert!(trimmed.effective_component_bytes(35).is_some());
        state.set_input(1, trimmed)?;
        assert!(state.may_pickup()?); // recipe.matches remains true
        assert!(state.result().is_empty()); // assemble rejects identical trim
        assert!(state.prepare_take(SmithingTakeKind::Pickup).is_err());
        assert_eq!(state.inputs()[0].count, 1);
        assert_eq!(state.inputs()[2].count, 1);
        assert!(!state.may_place(3, &item("redstone", 1))?);
        Ok(())
    }
}
