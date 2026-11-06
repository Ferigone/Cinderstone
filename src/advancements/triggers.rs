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

fn item_field<'a>(criterion: &'a Criterion, name: &str) -> Option<&'a crate::loot::predicate::ItemPredicate> {
    match criterion.conditions.get(name) {
        Some(Field::Item(item)) => Some(item),
        _ => None,
    }
}

fn ints(criterion: &Criterion, name: &str, value: i32) -> bool {
    match criterion.conditions.get(name) {
        Some(Field::Ints(bounds)) => bounds.matches(value),
        _ => true,
    }
}

fn optional_item(criterion: &Criterion, stack: &ItemStack) -> Result<bool, LootError> {
    match item_field(criterion, "item") {
        Some(predicate) => predicate.test(stack),
        None => Ok(true),
    }
}

/// `consume_item`, `filled_bucket`, `used_totem`, `using_item`, `shot_crossbow`: the optional
/// `item` predicate against the stack.
pub fn item_used(criterion: &Criterion, stack: &ItemStack) -> Result<Match, LootError> {
    Ok(finish(criterion, optional_item(criterion, stack)?))
}

/// `enchanted_item`: the item, then the enchantment `levels` spent.
pub fn enchanted_item(criterion: &Criterion, stack: &ItemStack, levels: i32) -> Result<Match, LootError> {
    Ok(finish(criterion, optional_item(criterion, stack)? && ints(criterion, "levels", levels)))
}

/// `item_durability_changed` with the stack after the change and its new damage value:
/// `durability` is `maxDamage - damage`, `delta` is the stack's current damage minus `damage`.
pub fn item_durability_changed(criterion: &Criterion, stack: &ItemStack, damage: i32) -> Result<Match, LootError> {
    if !optional_item(criterion, stack)? {
        return Ok(Match::NoMatch);
    }
    let host = |e: anyhow::Error| LootError::Host(e.to_string());
    let maximum = stack
        .effective_component_bytes(2)
        .map_or(Ok(0), |bytes| crate::protocol::Reader::new(bytes).varint())
        .map_err(host)?;
    let current = stack.damage_value().map_err(host)?;
    Ok(finish(criterion, ints(criterion, "durability", maximum.wrapping_sub(damage)) && ints(criterion, "delta", current.wrapping_sub(damage))))
}

/// `construct_beacon` with the beacon's level.
pub fn construct_beacon(criterion: &Criterion, level: i32) -> Match {
    finish(criterion, ints(criterion, "level", level))
}

/// `used_ender_eye` with the squared horizontal distance to the stronghold
/// (`MinMaxBounds.Doubles.matchesSqr`: the bounds are squared, sign and all).
pub fn used_ender_eye(criterion: &Criterion, distance_squared: f64) -> Match {
    let matched = match criterion.conditions.get("distance") {
        Some(Field::Doubles(bounds)) => {
            !bounds.min.is_some_and(|min| min * min > distance_squared) && !bounds.max.is_some_and(|max| max * max < distance_squared)
        }
        _ => true,
    };
    finish(criterion, matched)
}

/// `brewed_potion` with the brewed potion's id.
pub fn brewed_potion(criterion: &Criterion, potion: &str) -> Match {
    let matched = !matches!(criterion.conditions.get("potion"), Some(Field::Id(id)) if id != potion);
    finish(criterion, matched)
}

/// `changed_dimension` with the dimensions left and entered.
pub fn changed_dimension(criterion: &Criterion, from: &str, to: &str) -> Match {
    let key = |name: &str| match criterion.conditions.get(name) {
        Some(Field::Id(id)) => Some(id.as_str()),
        _ => None,
    };
    let matched = key("from").is_none_or(|id| id == from) && key("to").is_none_or(|id| id == to);
    finish(criterion, matched)
}

fn block_and_state(criterion: &Criterion, state: u32) -> Result<bool, LootError> {
    if let Some(Field::Id(block)) = criterion.conditions.get("block") {
        if crate::loot::predicate::block_state_block_id(state)? != *block {
            return Ok(false);
        }
    }
    match criterion.conditions.get("state") {
        Some(Field::State(properties)) => properties.matches(state),
        _ => Ok(true),
    }
}

/// `enter_block` / `slide_down_block` with the block state (global state id).
pub fn block_state(criterion: &Criterion, state: u32) -> Result<Match, LootError> {
    Ok(finish(criterion, block_and_state(criterion, state)?))
}

/// `bee_nest_destroyed` with the nest's state, the tool and the bees that were inside.
pub fn bee_nest_destroyed(criterion: &Criterion, state: u32, tool: &ItemStack, bees: i32) -> Result<Match, LootError> {
    let matched = block_and_state(criterion, state)? && optional_item(criterion, tool)? && ints(criterion, "num_bees_inside", bees);
    Ok(finish(criterion, matched))
}

/// `recipe_crafted` / `crafter_recipe_crafted` with the recipe id and the ingredients used: every
/// `ingredients` predicate must take a distinct ingredient (first match wins, in order).
pub fn recipe_crafted(criterion: &Criterion, recipe: &str, ingredients: &[ItemStack]) -> Result<Match, LootError> {
    if !matches!(criterion.conditions.get("recipe_id"), Some(Field::Id(id)) if id == recipe) {
        return Ok(Match::NoMatch);
    }
    let mut left: Vec<&ItemStack> = ingredients.iter().collect();
    if let Some(Field::Items(predicates)) = criterion.conditions.get("ingredients") {
        for predicate in predicates {
            let mut found = None;
            for (index, stack) in left.iter().enumerate() {
                if predicate.test(stack)? {
                    found = Some(index);
                    break;
                }
            }
            match found {
                Some(index) => {
                    left.remove(index);
                }
                None => return Ok(Match::NoMatch),
            }
        }
    }
    Ok(finish(criterion, true))
}
