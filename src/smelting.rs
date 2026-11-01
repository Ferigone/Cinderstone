//! Pinned Java 1.21 vanilla cooking recipes, fuels and crafting remainders.
//!
//! The fixture is derived from the original server's default recipe/tag data and
//! ordered AbstractFurnaceBlockEntity.getFuel additions. Ingredient matching uses
//! the item identity, as Ingredient.test does; input components are not copied to
//! the cooked result. Runtime datapack reload/custom recipe priority is outside
//! this default-catalogue owner rather than silently approximated.
use crate::inventory::{self, ItemStack};
use anyhow::{anyhow, ensure, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CookingKind {
    Smelting,
    Blasting,
    Smoking,
}

impl CookingKind {
    fn index(self) -> usize {
        match self {
            Self::Smelting => 0,
            Self::Blasting => 1,
            Self::Smoking => 2,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct CookingRecipe {
    pub id: String,
    pub kind: CookingKind,
    pub ingredient: Vec<i32>,
    pub result: ItemStack,
    pub cooking_time: i32,
    pub experience: f32,
}

#[derive(Deserialize)]
struct Data {
    minecraft: String,
    protocol: i32,
    fuels: BTreeMap<i32, i32>,
    remainders: BTreeMap<i32, i32>,
    recipes: Vec<CookingRecipe>,
}

struct Catalog {
    recipes: Vec<CookingRecipe>,
    by_input: [Vec<Option<usize>>; 3],
    by_id: BTreeMap<String, usize>,
    fuel_times: Vec<i32>,
    remainders: BTreeMap<i32, i32>,
}

impl Catalog {
    fn load() -> Result<Self> {
        let data: Data = serde_json::from_str(include_str!("../fixtures/smelting-1.21.json"))?;
        ensure!(data.minecraft == "1.21" && data.protocol == 767, "cooking catalogue version mismatch");
        let mut by_input: [Vec<Option<usize>>; 3] = std::array::from_fn(|_| Vec::new());
        let mut by_id = BTreeMap::new();
        for (index, recipe) in data.recipes.iter().enumerate() {
            ensure!(recipe.id.starts_with("minecraft:"), "unknown cooking recipe namespace");
            ensure!(by_id.insert(recipe.id.clone(), index).is_none(), "duplicate cooking recipe ID");
            ensure!(recipe.cooking_time > 0, "invalid cooking duration");
            ensure!(recipe.experience.is_finite() && recipe.experience >= 0.0, "invalid cooking experience");
            ensure!(!recipe.ingredient.is_empty(), "empty cooking ingredient");
            ensure!(!recipe.result.is_empty() && inventory::item_name(recipe.result.item_id).is_some(), "invalid cooking result");
            let items = &mut by_input[recipe.kind.index()];
            for item in &recipe.ingredient {
                ensure!(*item > 0 && inventory::item_name(*item).is_some(), "unknown cooking ingredient item");
                let item = *item as usize;
                if items.len() <= item {
                    items.resize(item + 1, None);
                }
                ensure!(items[item].replace(index).is_none(), "ambiguous default cooking recipes");
            }
        }
        let mut fuel_times = Vec::new();
        for (item, duration) in data.fuels {
            ensure!(item > 0 && inventory::item_name(item).is_some() && duration > 0, "invalid furnace fuel");
            let item = item as usize;
            if fuel_times.len() <= item {
                fuel_times.resize(item + 1, 0);
            }
            fuel_times[item] = duration;
        }
        for (item, remainder) in &data.remainders {
            ensure!(inventory::item_name(*item).is_some() && inventory::item_name(*remainder).is_some(), "invalid cooking remainder");
        }
        Ok(Self { recipes: data.recipes, by_input, by_id, fuel_times, remainders: data.remainders })
    }
}

fn catalog() -> Result<&'static Catalog> {
    static CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::load().map_err(|error| format!("{error:#}")))
        .as_ref()
        .map_err(|error| anyhow!("invalid pinned cooking catalogue: {error}"))
}

/// The default recipe matching this input. Lookup allocates no per-tick state.
/// Java's built-in default recipes have a unique match per kind and input item;
/// the exporter and loader reject overlapping recipes rather than inventing a
/// replacement for RecipeManager/CachedCheck's custom-datapack ordering.
pub fn recipe_for(kind: CookingKind, input: &ItemStack) -> Result<Option<&'static CookingRecipe>> {
    if input.is_empty() {
        return Ok(None);
    }
    ensure!(inventory::item_name(input.item_id).is_some(), "unknown cooking input item");
    let data = catalog()?;
    Ok(data.by_input[kind.index()].get(input.item_id as usize)
        .and_then(|index| *index).map(|index| &data.recipes[index]))
}

pub fn recipe_by_id(id: &str) -> Result<Option<&'static CookingRecipe>> {
    let data = catalog()?;
    Ok(data.by_id.get(id).map(|index| &data.recipes[*index]))
}

pub fn recipes() -> Result<&'static [CookingRecipe]> {
    Ok(&catalog()?.recipes)
}

/// AbstractFurnaceBlockEntity fuel duration before a smoker/blast furnace's
/// own getBurnDuration override applies its division by two.
pub fn fuel_time(stack: &ItemStack) -> Result<i32> {
    if stack.is_empty() {
        return Ok(0);
    }
    ensure!(inventory::item_name(stack.item_id).is_some(), "unknown furnace fuel item");
    Ok(catalog()?.fuel_times.get(stack.item_id as usize).copied().unwrap_or(0))
}

pub fn is_fuel(stack: &ItemStack) -> Result<bool> {
    Ok(fuel_time(stack)? > 0)
}

/// A fresh default stack of the item's static craftingRemainingItem. Input
/// components are not transferred. The empty bucket itself is not a fuel; its
/// furnace-menu acceptance is a separate slot rule.
pub fn fuel_remainder(stack: &ItemStack) -> Result<ItemStack> {
    if stack.is_empty() {
        return Ok(ItemStack::empty());
    }
    ensure!(inventory::item_name(stack.item_id).is_some(), "unknown furnace remainder item");
    catalog()?.remainders.get(&stack.item_id)
        .map_or_else(|| Ok(ItemStack::empty()), |item| ItemStack::new(*item, 1))
}
