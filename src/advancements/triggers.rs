//! Criterion trigger matching (WP-E5 stage 2), starting with the two triggers every recipe
//! advancement uses (WP-E6 #27): `inventory_changed` and `recipe_unlocked`.
//!
//! Each function is the trigger instance's `matches` for the event the trigger fires with. The
//! `player` predicate every trigger carries is a loot context predicate over the player; it is
//! evaluated by the caller (it needs the player as a `LootEntity`), so these functions answer
//! `PlayerPredicate` when one is present and `Matches`/`NoMatch` otherwise.

use super::definition::{Criterion, Field};
use crate::inventory::ItemStack;
use crate::loot::LootError;

/// The outcome of matching one criterion against an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    Matches,
    NoMatch,
    /// The trigger-specific part matches, but the criterion's `player` predicate must still pass.
    PlayerPredicate,
}

fn finish(criterion: &Criterion, matched: bool) -> Match {
    match (matched, criterion.conditions.contains_key("player")) {
        (false, _) => Match::NoMatch,
        (true, true) => Match::PlayerPredicate,
        (true, false) => Match::Matches,
    }
}

/// `InventoryChangeTrigger.trigger(player, inventory, changed)` for one criterion: the slot counts
/// (`full` counts stacks at their maximum size), then the item predicates — none matches anything;
/// one is tested against the changed stack only; several must each be matched by some non-empty
/// slot of the inventory.
pub fn inventory_changed(criterion: &Criterion, inventory: &[ItemStack], changed: &ItemStack) -> Result<Match, LootError> {
    let mut full = 0;
    let mut empty = 0;
    let mut occupied = 0;
    for stack in inventory {
        if stack.is_empty() {
            empty += 1;
        } else {
            occupied += 1;
            let mut probe = stack.clone();
            probe.count = 1;
            let maximum = probe.max_stack_size().map_err(|e| LootError::Host(e.to_string()))?;
            if stack.count >= maximum {
                full += 1;
            }
        }
    }
    if let Some(Field::Slots(occupied_bounds, full_bounds, empty_bounds)) = criterion.conditions.get("slots") {
        if !(occupied_bounds.matches(occupied) && full_bounds.matches(full) && empty_bounds.matches(empty)) {
            return Ok(Match::NoMatch);
        }
    }
    let items = match criterion.conditions.get("items") {
        Some(Field::Items(items)) => items.as_slice(),
        _ => &[],
    };
    let matched = match items {
        [] => true,
        [only] => !changed.is_empty() && only.test(changed)?,
        several => {
            let mut remaining: Vec<_> = several.iter().collect();
            for stack in inventory {
                if remaining.is_empty() {
                    break;
                }
                if !stack.is_empty() {
                    let mut kept = Vec::with_capacity(remaining.len());
                    for predicate in remaining {
                        if !predicate.test(stack)? {
                            kept.push(predicate);
                        }
                    }
                    remaining = kept;
                }
            }
            remaining.is_empty()
        }
    };
    Ok(finish(criterion, matched))
}

/// `RecipeUnlockedTrigger.TriggerInstance.matches(recipe)`: the recipe id is the criterion's.
pub fn recipe_unlocked(criterion: &Criterion, recipe: &str) -> Match {
    let matched = matches!(criterion.conditions.get("recipe"), Some(Field::Id(id)) if id == recipe);
    finish(criterion, matched)
}
