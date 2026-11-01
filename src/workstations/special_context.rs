//! Recipe selection and map-crafting boundaries with caller-owned runtime state.
//!
//! These adapters do not supply a recipe manager, saved maps or registry owners.
//! A production caller must borrow those owners for the same menu transaction.

use crate::{
    crafting::{CraftResult, MapRecipeData, RecipeBook, RecipeContext, RecipeInput},
    inventory::{self, Component, ItemStack},
    protocol::Writer,
};
use anyhow::{anyhow, ensure, Result};
use std::collections::BTreeSet;

/// The actual world's saved-map lookup. `None` certifies that the requested map
/// is absent; an unavailable/unimplemented owner must return an error instead.
pub trait MapRecipeLookup {
    fn recipe_map_data(&mut self, map_id: i32) -> Result<Option<MapRecipeData>>;
}

/// Resolve only the world lookup performed by MapExtendingRecipe.matches:
/// the shaped paper/map pattern is checked before consulting saved map data.
pub fn context_for_map_extension(
    book: &RecipeBook,
    input: &RecipeInput,
    maps: &mut impl MapRecipeLookup,
) -> Result<RecipeContext> {
    let mut context = RecipeContext::default();
    let RecipeInput::Crafting {
        width,
        height,
        items,
    } = input
    else {
        return Ok(context);
    };
    ensure!(
        *width <= 3 && *height <= 3 && width.checked_mul(*height) == Some(items.len()),
        "invalid crafting grid dimensions"
    );
    if *width != 3
        || *height != 3
        || items.iter().enumerate().any(|(slot, stack)| {
            stack.is_empty()
                || book.item_id(if slot == 4 { "filled_map" } else { "paper" })
                    != Some(stack.item_id)
        })
    {
        return Ok(context);
    }
    let Some(id) = items[4].component_value(26)? else {
        return Ok(context);
    };
    let id = i32::try_from(
        id.as_i64()
            .ok_or_else(|| anyhow!("invalid map ID component"))?,
    )?;
    if let Some(data) = maps.recipe_map_data(id)? {
        context.maps.insert(id, data);
    }
    Ok(context)
}

#[derive(Clone, Debug)]
pub struct SelectedRecipe {
    pub recipe_id: String,
    pub result: CraftResult,
}

/// A view of a real recipe manager's captured `byType` holder order. This does
/// not derive order from RecipeBook's BTreeMap or from recipe IDs. The host must
/// rebuild the view when its recipe manager snapshot changes.
pub struct RecipeManagerView<'a> {
    book: &'a RecipeBook,
    ordered_ids: &'a [String],
}

impl<'a> RecipeManagerView<'a> {
    pub fn new(book: &'a RecipeBook, recipe_type: &str, ordered_ids: &'a [String]) -> Result<Self> {
        let mut seen = BTreeSet::new();
        for id in ordered_ids {
            ensure!(
                book.recipe_type(id) == Some(recipe_type),
                "recipe {id} has wrong type for captured manager view"
            );
            let canonical = if id.contains(':') {
                id.clone()
            } else {
                format!("minecraft:{id}")
            };
            ensure!(seen.insert(canonical), "duplicate recipe holder {id}");
        }
        Ok(Self { book, ordered_ids })
    }

    /// RecipeManager.getRecipeFor first tests the supplied last holder, then
    /// finds the first matching holder. A matching recipe whose assembly is
    /// empty remains selected (e.g. an identical smithing trim).
    pub fn get_recipe_for(
        &self,
        input: &RecipeInput,
        context: &RecipeContext,
        last_recipe: Option<&str>,
    ) -> Result<Option<SelectedRecipe>> {
        if let Some(last) = last_recipe {
            if let Some(id) = self.ordered_ids.iter().find(|id| id.as_str() == last) {
                if let Some(result) = self.book.evaluate_with_context(id, input, context)? {
                    return Ok(Some(SelectedRecipe {
                        recipe_id: id.clone(),
                        result,
                    }));
                }
            }
        }
        for id in self.ordered_ids {
            if let Some(result) = self.book.evaluate_with_context(id, input, context)? {
                return Ok(Some(SelectedRecipe {
                    recipe_id: id.clone(),
                    result,
                }));
            }
        }
        Ok(None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapPostProcessing {
    Lock,
    Scale,
}

/// The actual map saved-data and ID-allocation owner. An implementation looks
/// up `source_map_id`, allocates a new ID and stores the locked/scaled data,
/// in that order. It returns None for an absent source map (including no ID).
/// Errors must leave the owner's transaction uncommitted. This module never
/// invents a map record or allocates IDs independently of the world owner.
pub trait CraftedMapOwner {
    fn postprocess_map(
        &mut self,
        source_map_id: Option<i32>,
        operation: MapPostProcessing,
    ) -> Result<Option<i32>>;
}

/// Port of MapItem.onCraftedPostProcess's component/callback order. Call only
/// for an actually taken result, before publishing that result's owned stack.
/// A preview must never invoke this world mutation. Staging/rollback of the
/// borrowed real owner belongs to the enclosing inventory transaction.
pub fn postprocess_crafted_map(
    stack: &mut ItemStack,
    maps: &mut impl CraftedMapOwner,
) -> Result<bool> {
    if stack.is_empty() || inventory::item_name(stack.item_id) != Some("filled_map") {
        return Ok(false);
    }
    let Some(operation) = stack.component_value(28)? else {
        return Ok(false);
    };
    let operation = match operation.as_i64() {
        Some(0) => MapPostProcessing::Lock,
        Some(1) => MapPostProcessing::Scale,
        _ => return Err(anyhow!("invalid map post-processing component")),
    };
    let source = stack
        .component_value(26)?
        .map(|value| {
            i32::try_from(
                value
                    .as_i64()
                    .ok_or_else(|| anyhow!("invalid map ID component"))?,
            )
            .map_err(anyhow::Error::from)
        })
        .transpose()?;
    let mut result = stack.clone();
    // ItemStack.remove happens even when MapItem.getSavedData finds no map.
    result
        .components
        .retain(|component| component.type_id != 28);
    if !result.removed_components.contains(&28) {
        result.removed_components.push(28);
    }
    let new_id = match source {
        Some(id) => maps.postprocess_map(Some(id), operation)?,
        None => None,
    };
    if let Some(id) = new_id {
        let mut data = Writer::new();
        data.varint(id);
        result
            .components
            .retain(|component| component.type_id != 26);
        result
            .removed_components
            .retain(|component| *component != 26);
        result.components.push(Component {
            type_id: 26,
            data: data.into_inner(),
        });
    }
    result.normalize_patch();
    *stack = result;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer(stack: &mut ItemStack, type_id: i32, value: i32) {
        let mut data = Writer::new();
        data.varint(value);
        stack.components.push(Component {
            type_id,
            data: data.into_inner(),
        });
    }

    #[test]
    fn map_matching_reads_actual_owner_only_after_shaped_match() {
        struct Maps {
            calls: Vec<i32>,
        }
        impl MapRecipeLookup for Maps {
            fn recipe_map_data(&mut self, id: i32) -> Result<Option<MapRecipeData>> {
                self.calls.push(id);
                Ok(None)
            }
        }
        let book = RecipeBook::vanilla();
        let mut maps = Maps { calls: Vec::new() };
        let mut input = RecipeInput::crafting(
            3,
            3,
            vec![ItemStack::new(book.item_id("paper").unwrap(), 1).unwrap(); 9],
        )
        .unwrap();
        assert!(context_for_map_extension(book, &input, &mut maps)
            .unwrap()
            .maps
            .is_empty());
        assert!(maps.calls.is_empty());
        let RecipeInput::Crafting { items, .. } = &mut input else {
            unreachable!()
        };
        items[4] = ItemStack::new(book.item_id("filled_map").unwrap(), 1).unwrap();
        integer(&mut items[4], 26, 41);
        let context = context_for_map_extension(book, &input, &mut maps).unwrap();
        assert_eq!(maps.calls, vec![41]);
        // A certified missing saved map is a source nonmatch, not a dummy map.
        assert!(book
            .evaluate_with_context("map_extending", &input, &context)
            .unwrap()
            .is_none());
    }

    #[test]
    fn result_map_postprocessing_consumes_flag_and_uses_real_owner_once() {
        struct Maps {
            calls: Vec<(Option<i32>, MapPostProcessing)>,
        }
        impl CraftedMapOwner for Maps {
            fn postprocess_map(
                &mut self,
                id: Option<i32>,
                operation: MapPostProcessing,
            ) -> Result<Option<i32>> {
                self.calls.push((id, operation));
                Ok(Some(92))
            }
        }
        let mut map =
            ItemStack::new(RecipeBook::vanilla().item_id("filled_map").unwrap(), 1).unwrap();
        integer(&mut map, 26, 41);
        integer(&mut map, 28, 1);
        let mut maps = Maps { calls: Vec::new() };
        assert!(postprocess_crafted_map(&mut map, &mut maps).unwrap());
        assert_eq!(maps.calls, vec![(Some(41), MapPostProcessing::Scale)]);
        assert_eq!(map.component_value(26).unwrap().unwrap().as_i64(), Some(92));
        assert!(map.component_value(28).unwrap().is_none());
        assert!(!postprocess_crafted_map(&mut map, &mut maps).unwrap());
        assert_eq!(maps.calls.len(), 1);
    }

    #[test]
    fn manager_view_preserves_host_order_and_cached_holder_preference() {
        let book = RecipeBook::vanilla();
        let ids = vec![
            "minecraft:stone_brick_slab_from_stone_stonecutting".to_owned(),
            "minecraft:stone_bricks_from_stone_stonecutting".to_owned(),
        ];
        let view = RecipeManagerView::new(book, "stonecutting", &ids).unwrap();
        let input = RecipeInput::Single(ItemStack::new(book.item_id("stone").unwrap(), 1).unwrap());
        let context = RecipeContext::default();
        assert_eq!(
            view.get_recipe_for(&input, &context, None)
                .unwrap()
                .unwrap()
                .recipe_id,
            ids[0]
        );
        assert_eq!(
            view.get_recipe_for(&input, &context, Some(&ids[1]))
                .unwrap()
                .unwrap()
                .recipe_id,
            ids[1]
        );
    }
}
