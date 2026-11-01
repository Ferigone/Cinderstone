//! Exact-version vanilla recipe data and native matching/assembly.
//!
//! Evaluation is by recipe ID. Recipe-manager ordering, menu transactions,
//! recipe-book unlocks and world-dependent special recipes are separate layers.
//! Tags are recursively expanded from the unchanged 1.21 server artifact.

use crate::{
    inventory::{self, Component, ItemStack},
    protocol::{Reader, Writer},
};
use anyhow::{anyhow, bail, ensure, Result};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Debug)]
pub enum RecipeInput {
    Crafting {
        width: usize,
        height: usize,
        items: Vec<ItemStack>,
    },
    Single(ItemStack),
    Smithing {
        template: ItemStack,
        base: ItemStack,
        addition: ItemStack,
    },
}

impl RecipeInput {
    pub fn crafting(width: usize, height: usize, items: Vec<ItemStack>) -> Result<Self> {
        ensure!(
            width <= 3 && height <= 3 && width.checked_mul(height) == Some(items.len()),
            "invalid crafting grid dimensions"
        );
        Ok(Self::Crafting {
            width,
            height,
            items,
        })
    }
    fn items(&self) -> &[ItemStack] {
        match self {
            Self::Crafting { items, .. } => items,
            Self::Single(item) => std::slice::from_ref(item),
            Self::Smithing { .. } => &[],
        }
    }
}

/// World data consulted by recipes; map IDs refer to saved map data, not
/// merely the item component. Populate this from the world's map storage.
#[derive(Clone, Debug, Default)]
pub struct RecipeContext {
    pub maps: BTreeMap<i32, MapRecipeData>,
}
#[derive(Clone, Copy, Debug)]
pub struct MapRecipeData {
    pub scale: i8,
    pub exploration: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CraftResult {
    pub stack: ItemStack,
    /// One entry per original input slot, before the occupied rectangle is trimmed.
    pub remaining: Vec<ItemStack>,
}

#[derive(Clone, Debug, Deserialize)]
struct RecipeResult {
    item_id: i32,
    count: i32,
}
#[derive(Clone, Debug, Deserialize)]
struct Recipe {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    key: BTreeMap<String, Vec<i32>>,
    #[serde(default)]
    pattern: Vec<String>,
    #[serde(default)]
    ingredients: Vec<Vec<i32>>,
    #[serde(default)]
    ingredient: Vec<i32>,
    #[serde(default)]
    template: Vec<i32>,
    #[serde(default)]
    base: Vec<i32>,
    #[serde(default)]
    addition: Vec<i32>,
    result: Option<RecipeResult>,
    #[serde(default)]
    experience: Option<f32>,
    #[serde(default)]
    cookingtime: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct RecipeBook {
    recipes: BTreeMap<String, Recipe>,
    remainders: BTreeMap<i32, i32>,
    item_ids: BTreeMap<String, i32>,
    tags: BTreeMap<String, Vec<i32>>,
}

impl RecipeBook {
    pub fn vanilla() -> &'static Self {
        static BOOK: OnceLock<RecipeBook> = OnceLock::new();
        BOOK.get_or_init(|| {
            serde_json::from_str(include_str!("../fixtures/crafting-1.21.json"))
                .expect("pinned vanilla recipe data")
        })
    }
    pub fn len(&self) -> usize {
        self.recipes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }
    pub fn recipe_ids(&self) -> impl Iterator<Item = &str> {
        self.recipes.keys().map(String::as_str)
    }
    pub fn recipe_type(&self, id: &str) -> Option<&str> {
        self.recipes.get(&canonical(id)).map(|r| r.kind.as_str())
    }
    pub fn item_id(&self, id: &str) -> Option<i32> {
        self.item_ids.get(&canonical(id)).copied()
    }
    pub fn cooking_parameters(&self, id: &str) -> Option<(f32, i32)> {
        let r = self.recipes.get(&canonical(id))?;
        Some((r.experience?, r.cookingtime?))
    }
    /// Ingredient membership for a captured smithing recipe, before the other
    /// two inputs are available. Menu routing must retain the recipe manager's
    /// encounter order rather than using this book's sorted storage order.
    pub fn smithing_accepts_input(
        &self,
        id: &str,
        slot: usize,
        stack: &ItemStack,
    ) -> Result<bool> {
        ensure!(slot < 3, "smithing input slot must be 0, 1 or 2");
        let recipe = self
            .recipes
            .get(&canonical(id))
            .ok_or_else(|| anyhow!("unknown recipe {id}"))?;
        if !matches!(recipe.kind.as_str(), "smithing_transform" | "smithing_trim") {
            return Ok(false);
        }
        Ok(accepts(
            match slot {
                0 => &recipe.template,
                1 => &recipe.base,
                _ => &recipe.addition,
            },
            stack,
        ))
    }
    /// Pinned-default-catalog predicate. A menu holding a different recipe
    /// snapshot must use `smithing_accepts_input` over its own captured list.
    pub fn smithing_accepts_template(&self, stack: &ItemStack) -> bool {
        self.smithing_accepts_any(0, stack)
    }
    pub fn smithing_accepts_base(&self, stack: &ItemStack) -> bool {
        self.smithing_accepts_any(1, stack)
    }
    pub fn smithing_accepts_addition(&self, stack: &ItemStack) -> bool {
        self.smithing_accepts_any(2, stack)
    }
    fn smithing_accepts_any(&self, slot: usize, stack: &ItemStack) -> bool {
        self.recipes.iter().any(|(id, _)| {
            self.smithing_accepts_input(id, slot, stack)
                .expect("known recipe and smithing slot")
        })
    }
    pub fn evaluate(&self, id: &str, input: &RecipeInput) -> Result<Option<CraftResult>> {
        self.evaluate_with_context(id, input, &RecipeContext::default())
    }
    pub fn evaluate_with_context(
        &self,
        id: &str,
        input: &RecipeInput,
        context: &RecipeContext,
    ) -> Result<Option<CraftResult>> {
        let recipe = self
            .recipes
            .get(&canonical(id))
            .ok_or_else(|| anyhow!("unknown recipe {id}"))?;
        if recipe.kind == "crafting_special_mapextending" {
            let RecipeInput::Crafting {
                width,
                height,
                items,
            } = input
            else {
                return Ok(None);
            };
            validate_grid(*width, *height, items)?;
            if *width != 3
                || *height != 3
                || items.iter().enumerate().any(|(i, s)| {
                    s.is_empty()
                        || self.item_id(if i == 4 { "filled_map" } else { "paper" })
                            != Some(s.item_id)
                })
            {
                return Ok(None);
            }
            let Some(data) = component(&items[4], 26)? else {
                return Ok(None);
            };
            let map_id = Reader::new(&data).varint()?;
            let Some(map) = context.maps.get(&map_id) else {
                return Ok(None);
            };
            if map.scale >= 4 || map.exploration {
                return Ok(None);
            }
            let mut stack = items[4].clone();
            stack.count = 1;
            set_int_component(&mut stack, 28, 1);
            return Ok(Some(CraftResult {
                stack,
                remaining: items
                    .iter()
                    .map(|s| self.remainder(s))
                    .collect::<Result<_>>()?,
            }));
        }
        if recipe.kind.starts_with("crafting_special_") || recipe.kind == "crafting_decorated_pot" {
            return self.evaluate_special(&recipe.kind, input);
        }
        let matched = match (&*recipe.kind, input) {
            (
                "crafting_shaped",
                RecipeInput::Crafting {
                    width,
                    height,
                    items,
                },
            ) => {
                validate_grid(*width, *height, items)?;
                shaped_matches(recipe, *width, *height, items)
            }
            (
                "crafting_shapeless",
                RecipeInput::Crafting {
                    width,
                    height,
                    items,
                },
            ) => {
                validate_grid(*width, *height, items)?;
                shapeless_matches(&recipe.ingredients, items)
            }
            (
                "stonecutting" | "smelting" | "smoking" | "blasting" | "campfire_cooking",
                RecipeInput::Single(item),
            ) => accepts(&recipe.ingredient, item),
            (
                "smithing_transform",
                RecipeInput::Smithing {
                    template,
                    base,
                    addition,
                },
            ) => {
                accepts(&recipe.template, template)
                    && accepts(&recipe.base, base)
                    && accepts(&recipe.addition, addition)
            }
            (
                "smithing_trim",
                RecipeInput::Smithing {
                    template,
                    base,
                    addition,
                },
            ) => {
                if !accepts(&recipe.template, template)
                    || !accepts(&recipe.base, base)
                    || !accepts(&recipe.addition, addition)
                {
                    return Ok(None);
                }
                let mut stack = ItemStack::empty();
                if let (Some(material), Some(pattern)) = (
                    metadata()["trim_material_items"][addition.item_id.to_string()].as_i64(),
                    metadata()["trim_pattern_items"][template.item_id.to_string()].as_i64(),
                ) {
                    let mut w = Writer::new();
                    w.varint(material as i32 + 1)
                        .varint(pattern as i32 + 1)
                        .bool(true);
                    let data = w.into_inner();
                    // Tooltip visibility is ignored by ArmorTrim.hasPatternAndMaterial.
                    let same = component(base, 35)?.is_some_and(|old| {
                        let mut r = Reader::new(&old);
                        r.varint().ok() == Some(material as i32 + 1)
                            && r.varint().ok() == Some(pattern as i32 + 1)
                    });
                    if !same {
                        stack = base.clone();
                        stack.count = 1;
                        set_component(&mut stack, 35, data);
                    }
                }
                return Ok(Some(CraftResult {
                    stack,
                    remaining: [template, base, addition]
                        .into_iter()
                        .map(|i| self.remainder(i))
                        .collect::<Result<_>>()?,
                }));
            }
            (kind, RecipeInput::Crafting { .. })
                if kind.starts_with("crafting_special_") || kind == "crafting_decorated_pot" =>
            {
                bail!("special recipe serializer {kind} is not implemented yet")
            }
            _ => false,
        };
        if !matched {
            return Ok(None);
        }
        let result = recipe
            .result
            .as_ref()
            .ok_or_else(|| anyhow!("recipe has no static result"))?;
        let mut stack = match input {
            RecipeInput::Smithing { base, .. } => {
                // ItemStack.transmuteCopy keeps the base's component patch while
                // replacing its item prototype and count. It does not copy the
                // old prototype's defaults into the new item.
                let mut stack = base.clone();
                stack.item_id = result.item_id;
                stack.count = result.count;
                stack
            }
            _ => ItemStack::new(result.item_id, result.count)?,
        };
        if stack.is_empty() {
            stack = ItemStack::empty();
        }
        let remaining = match input {
            RecipeInput::Smithing {
                template,
                base,
                addition,
            } => [template, base, addition]
                .into_iter()
                .map(|s| self.remainder(s))
                .collect::<Result<_>>()?,
            _ => input
                .items()
                .iter()
                .map(|s| self.remainder(s))
                .collect::<Result<_>>()?,
        };
        Ok(Some(CraftResult { stack, remaining }))
    }
    fn remainder(&self, stack: &ItemStack) -> Result<ItemStack> {
        if stack.is_empty() {
            return Ok(ItemStack::empty());
        }
        self.remainders
            .get(&stack.item_id)
            .map_or_else(|| Ok(ItemStack::empty()), |id| ItemStack::new(*id, 1))
    }
}

impl RecipeBook {
    fn evaluate_special(&self, kind: &str, input: &RecipeInput) -> Result<Option<CraftResult>> {
        let RecipeInput::Crafting {
            width,
            height,
            items,
        } = input
        else {
            return Ok(None);
        };
        validate_grid(*width, *height, items)?;
        let nonempty: Vec<(usize, &ItemStack)> = items
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_empty())
            .collect();
        let mut remaining = items
            .iter()
            .map(|s| self.remainder(s))
            .collect::<Result<Vec<_>>>()?;
        let id = |name: &str| self.item_id(name).expect("pinned item exists");
        let named = |s: &ItemStack, name: &str| s.item_id == id(name);
        let no_match = || Ok(None);
        let mut result = match kind {
            "crafting_special_bookcloning" => {
                let originals = nonempty
                    .iter()
                    .filter(|(_, s)| named(s, "written_book"))
                    .collect::<Vec<_>>();
                let blanks = nonempty
                    .iter()
                    .filter(|(_, s)| named(s, "writable_book"))
                    .count();
                if originals.len() != 1 || blanks == 0 || blanks + 1 != nonempty.len() {
                    return no_match();
                }
                let (index, original) = *originals[0];
                let mut source = original.clone();
                source.count = 1;
                remaining[index] = source;
                let Some(data) = component(original, 34)? else {
                    return Ok(Some(CraftResult {
                        stack: ItemStack::empty(),
                        remaining,
                    }));
                };
                let mut r = Reader::new(&data);
                r.string(32767)?;
                if r.bool()? {
                    r.string(32767)?;
                }
                r.string(32767)?;
                let before = data.len() - r.remaining().len();
                let generation = r.varint()?;
                if generation >= 2 {
                    return Ok(Some(CraftResult {
                        stack: ItemStack::empty(),
                        remaining,
                    }));
                }
                let after = data.len() - r.remaining().len();
                let mut w = Writer::new();
                w.raw(&data[..before])
                    .varint(generation + 1)
                    .raw(&data[after..]);
                let mut output = original.clone();
                output.count = blanks as i32;
                set_component(&mut output, 34, w.into_inner());
                output
            }
            "crafting_special_suspiciousstew" => {
                let flowers = nonempty
                    .iter()
                    .filter(|(_, s)| self.tags["small_flowers"].contains(&s.item_id))
                    .collect::<Vec<_>>();
                if nonempty.len() != 4
                    || flowers.len() != 1
                    || !["red_mushroom", "brown_mushroom", "bowl"]
                        .into_iter()
                        .all(|n| nonempty.iter().filter(|(_, s)| named(s, n)).count() == 1)
                {
                    return no_match();
                }
                let mut output = ItemStack::new(id("suspicious_stew"), 1)?;
                if let Some(effects) = metadata()["stew"]
                    .get(format!("minecraft:{}", item_name(flowers[0].1)))
                    .and_then(Value::as_array)
                {
                    let mut w = Writer::new();
                    w.varint(effects.len() as i32);
                    for effect in effects {
                        let name = effect["id"].as_str().unwrap();
                        let effect_id = metadata()["mob_effects"][name]["protocol_id"]
                            .as_i64()
                            .unwrap() as i32;
                        w.varint(effect_id)
                            .varint(effect["duration"].as_i64().unwrap_or(160) as i32);
                    }
                    set_component(&mut output, 32, w.into_inner());
                }
                output
            }
            "crafting_special_repairitem" => {
                if nonempty.len() != 2 {
                    return no_match();
                }
                let (a, b) = (nonempty[0].1, nonempty[1].1);
                if a.item_id != b.item_id
                    || a.count != 1
                    || b.count != 1
                    || component(a, 2)?.is_none()
                    || component(b, 2)?.is_none()
                    || component(a, 3)?.is_none()
                    || component(b, 3)?.is_none()
                {
                    return no_match();
                }
                let (max_a, max_b) = (int_component(a, 2, 0)?, int_component(b, 2, 0)?);
                let maximum = max_a.max(max_b);
                let (damage_a, damage_b) = (
                    int_component(a, 3, 0)?.clamp(0, max_a),
                    int_component(b, 3, 0)?.clamp(0, max_b),
                );
                let durability = max_a
                    .wrapping_sub(damage_a)
                    .wrapping_add(max_b.wrapping_sub(damage_b))
                    .wrapping_add(maximum.wrapping_mul(5) / 100);
                let mut output = ItemStack::new(a.item_id, 1)?;
                set_int_component(&mut output, 2, maximum);
                set_int_component(
                    &mut output,
                    3,
                    maximum.wrapping_sub(durability).max(0).clamp(0, maximum),
                );
                let mut curses = BTreeMap::<i32, i32>::new();
                let curse_ids = [
                    metadata()["registries"]["minecraft:enchantment"]["minecraft:binding_curse"]
                        .as_i64()
                        .unwrap() as i32,
                    metadata()["registries"]["minecraft:enchantment"]["minecraft:vanishing_curse"]
                        .as_i64()
                        .unwrap() as i32,
                ];
                for input in [a, b] {
                    if let Some(data) = component(input, 9)? {
                        let mut r = Reader::new(&data);
                        let count = r.varint()?;
                        ensure!(count >= 0, "negative enchantment length");
                        for _ in 0..count {
                            let id = r.varint()?;
                            let level = r.varint()?;
                            if curse_ids.contains(&id) && level > 0 {
                                let old = curses.entry(id).or_default();
                                *old = (*old).max(level.min(255));
                            }
                        }
                    }
                }
                if !curses.is_empty() {
                    let mut w = Writer::new();
                    w.varint(curses.len() as i32);
                    for (id, level) in curses {
                        w.varint(id).varint(level);
                    }
                    w.bool(true);
                    set_component(&mut output, 9, w.into_inner());
                }
                output
            }
            "crafting_special_shulkerboxcoloring" => {
                if nonempty.len() != 2 {
                    return no_match();
                }
                let Some((_, box_stack)) = nonempty
                    .iter()
                    .find(|(_, s)| item_name(s).ends_with("shulker_box"))
                else {
                    return no_match();
                };
                let Some(color) = nonempty.iter().find_map(|(_, s)| dye(s)) else {
                    return no_match();
                };
                let mut output = (*box_stack).clone();
                output.item_id = id(&format!("{color}_shulker_box"));
                output.count = 1;
                output
            }
            "crafting_special_mapcloning" => {
                let maps: Vec<_> = nonempty
                    .iter()
                    .filter(|(_, s)| named(s, "filled_map"))
                    .collect();
                let blanks = nonempty.iter().filter(|(_, s)| named(s, "map")).count();
                if maps.len() != 1 || blanks == 0 || blanks + 1 != nonempty.len() {
                    return no_match();
                }
                let mut output = maps[0].1.clone();
                output.count = blanks as i32 + 1;
                output
            }
            "crafting_decorated_pot" => {
                if *width != 3 || *height != 3 {
                    return no_match();
                }
                let allowed = &self.tags["decorated_pot_ingredients"];
                if (0..9).any(|i| {
                    if [1, 3, 5, 7].contains(&i) {
                        !accepts(allowed, &items[i])
                    } else {
                        !items[i].is_empty()
                    }
                }) {
                    return no_match();
                }
                let mut output = ItemStack::new(id("decorated_pot"), 1)?;
                let mut w = Writer::new();
                w.varint(4);
                for i in [1, 3, 5, 7] {
                    w.varint(items[i].item_id);
                }
                set_component(&mut output, 51, w.into_inner());
                output
            }
            "crafting_special_tippedarrow" => {
                if *width != 3
                    || *height != 3
                    || items
                        .iter()
                        .enumerate()
                        .any(|(i, s)| !named(s, if i == 4 { "lingering_potion" } else { "arrow" }))
                {
                    return no_match();
                }
                let mut output = ItemStack::new(id("tipped_arrow"), 8)?;
                match component(&items[4], 31)? {
                    Some(data) => set_component(&mut output, 31, data),
                    None => remove_component(&mut output, 31),
                };
                output
            }
            "crafting_special_bannerduplicate" => {
                if nonempty.len() != 2
                    || nonempty
                        .iter()
                        .any(|(_, s)| !item_name(s).ends_with("_banner"))
                    || nonempty[0].1.item_id != nonempty[1].1.item_id
                {
                    return no_match();
                }
                let counts = nonempty
                    .iter()
                    .map(|(_, s)| component_list_len(s, 49))
                    .collect::<Result<Vec<_>>>()?;
                let source = if (1..=6).contains(&counts[0]) && counts[1] == 0 {
                    0
                } else if (1..=6).contains(&counts[1]) && counts[0] == 0 {
                    1
                } else {
                    return no_match();
                };
                let mut output = nonempty[source].1.clone();
                output.count = 1;
                remaining[nonempty[source].0] = output.clone();
                output
            }
            "crafting_special_shielddecoration" => {
                if nonempty.len() != 2 {
                    return no_match();
                }
                let Some((_, shield)) = nonempty.iter().find(|(_, s)| named(s, "shield")) else {
                    return no_match();
                };
                let Some((_, banner)) = nonempty
                    .iter()
                    .find(|(_, s)| item_name(s).ends_with("_banner"))
                else {
                    return no_match();
                };
                if component_list_len(shield, 49)? != 0 {
                    return no_match();
                }
                let mut output = (*shield).clone();
                match component(banner, 49)? {
                    Some(data) => set_component(&mut output, 49, data),
                    None => remove_component(&mut output, 49),
                };
                let color = item_name(banner).strip_suffix("_banner").unwrap();
                let color_id = metadata()["dyes"][color]["id"].as_i64().unwrap() as i32;
                set_int_component(&mut output, 50, color_id);
                output
            }
            "crafting_special_armordye" => {
                let wearable = nonempty
                    .iter()
                    .filter(|(_, s)| self.tags["dyeable"].contains(&s.item_id))
                    .collect::<Vec<_>>();
                let dyes = nonempty
                    .iter()
                    .filter_map(|(_, s)| dye(s))
                    .collect::<Vec<_>>();
                if wearable.len() != 1 || dyes.is_empty() || dyes.len() + 1 != nonempty.len() {
                    return no_match();
                }
                let mut output = wearable[0].1.clone();
                output.count = 1;
                let existing = component(&output, 24)?;
                let mut colors = Vec::new();
                let mut show = true;
                if let Some(data) = &existing {
                    let mut r = Reader::new(data);
                    colors.push(r.i32()?);
                    show = r.bool()?;
                }
                colors.extend(
                    dyes.iter()
                        .map(|name| metadata()["dyes"][name]["texture"].as_i64().unwrap() as i32),
                );
                let mut sums = [0i32; 3];
                let mut brightness = 0i32;
                for rgb in &colors {
                    let channels = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255];
                    for i in 0..3 {
                        sums[i] += channels[i];
                    }
                    brightness += *channels.iter().max().unwrap();
                }
                let count = colors.len() as i32;
                let mut channels = sums.map(|sum| sum / count);
                let avg = brightness as f32 / count as f32;
                let max = *channels.iter().max().unwrap() as f32;
                for v in &mut channels {
                    *v = (*v as f32 * avg / max) as i32;
                }
                let rgb = (channels[0] << 16) | (channels[1] << 8) | channels[2];
                let mut w = Writer::new();
                w.i32(rgb).bool(show);
                set_component(&mut output, 24, w.into_inner());
                output
            }
            "crafting_special_firework_rocket" => {
                let paper = nonempty.iter().filter(|(_, s)| named(s, "paper")).count();
                let powder = nonempty
                    .iter()
                    .filter(|(_, s)| named(s, "gunpowder"))
                    .count();
                let stars = nonempty
                    .iter()
                    .filter(|(_, s)| named(s, "firework_star"))
                    .collect::<Vec<_>>();
                if paper != 1
                    || !(1..=3).contains(&powder)
                    || paper + powder + stars.len() != nonempty.len()
                {
                    return no_match();
                }
                let explosions = stars
                    .iter()
                    .filter_map(|(_, s)| component(s, 45).transpose())
                    .collect::<Result<Vec<_>>>()?;
                let mut output = ItemStack::new(id("firework_rocket"), 3)?;
                let mut w = Writer::new();
                w.varint(powder as i32).varint(explosions.len() as i32);
                for explosion in explosions {
                    w.raw(&explosion);
                }
                set_component(&mut output, 46, w.into_inner());
                output
            }
            "crafting_special_firework_star" => {
                let mut powder = 0;
                let mut trail = false;
                let mut twinkle = false;
                let mut shape = None;
                let mut colors = Vec::new();
                for (_, s) in &nonempty {
                    if named(s, "gunpowder") {
                        powder += 1;
                    } else if named(s, "diamond") {
                        if trail {
                            return no_match();
                        }
                        trail = true;
                    } else if named(s, "glowstone_dust") {
                        if twinkle {
                            return no_match();
                        }
                        twinkle = true;
                    } else if let Some(color) = dye(s) {
                        colors.push(metadata()["dyes"][color]["firework"].as_i64().unwrap() as i32);
                    } else {
                        let n = item_name(s);
                        let v = match n {
                            "fire_charge" => 1,
                            "gold_nugget" => 2,
                            "feather" => 4,
                            "skeleton_skull"
                            | "wither_skeleton_skull"
                            | "creeper_head"
                            | "player_head"
                            | "dragon_head"
                            | "zombie_head"
                            | "piglin_head" => 3,
                            _ => return no_match(),
                        };
                        if shape.replace(v).is_some() {
                            return no_match();
                        }
                    }
                }
                if powder != 1 || colors.is_empty() {
                    return no_match();
                }
                let mut output = ItemStack::new(id("firework_star"), 1)?;
                let mut w = Writer::new();
                w.varint(shape.unwrap_or(0)).varint(colors.len() as i32);
                for color in colors {
                    w.i32(color);
                }
                w.varint(0).bool(trail).bool(twinkle);
                set_component(&mut output, 45, w.into_inner());
                output
            }
            "crafting_special_firework_star_fade" => {
                let stars = nonempty
                    .iter()
                    .filter(|(_, s)| named(s, "firework_star"))
                    .collect::<Vec<_>>();
                let colors = nonempty
                    .iter()
                    .filter_map(|(_, s)| dye(s))
                    .map(|name| metadata()["dyes"][name]["firework"].as_i64().unwrap() as i32)
                    .collect::<Vec<_>>();
                if stars.len() != 1 || colors.is_empty() || colors.len() + 1 != nonempty.len() {
                    return no_match();
                }
                let Some(data) = component(stars[0].1, 45)? else {
                    return Ok(Some(CraftResult {
                        stack: ItemStack::empty(),
                        remaining,
                    }));
                };
                let mut r = Reader::new(&data);
                let shape = r.varint()?;
                let count = r.varint()?;
                ensure!(count >= 0, "negative explosion colors");
                let original = r.bytes(count as usize * 4)?.to_vec();
                let fades = r.varint()?;
                ensure!(fades >= 0, "negative fade colors");
                r.bytes(fades as usize * 4)?;
                let trail = r.bool()?;
                let twinkle = r.bool()?;
                r.finish()?;
                let mut output = stars[0].1.clone();
                output.count = 1;
                let mut w = Writer::new();
                w.varint(shape)
                    .varint(count)
                    .raw(&original)
                    .varint(colors.len() as i32);
                for color in colors {
                    w.i32(color);
                }
                w.bool(trail).bool(twinkle);
                set_component(&mut output, 45, w.into_inner());
                output
            }
            _ => bail!("special recipe serializer {kind} is not implemented yet"),
        };
        if result.is_empty() {
            result = ItemStack::empty();
        }
        Ok(Some(CraftResult {
            stack: result,
            remaining,
        }))
    }
}
fn metadata() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../tools/crafting_metadata.json"))
            .expect("pinned crafting metadata")
    })
}
fn item_name(stack: &ItemStack) -> &'static str {
    inventory::item_name(stack.item_id).unwrap_or("")
}
fn dye(stack: &ItemStack) -> Option<&'static str> {
    let color = item_name(stack).strip_suffix("_dye")?;
    metadata()["dyes"].get(color).map(|_| color)
}
fn component(stack: &ItemStack, id: i32) -> Result<Option<Vec<u8>>> {
    Ok(stack.effective_component_bytes(id).map(<[u8]>::to_vec))
}
fn int_component(stack: &ItemStack, id: i32, default: i32) -> Result<i32> {
    component(stack, id)?.map_or(Ok(default), |data| Reader::new(&data).varint())
}
fn component_list_len(stack: &ItemStack, id: i32) -> Result<i32> {
    component(stack, id)?.map_or(Ok(0), |data| Reader::new(&data).varint())
}
fn set_component(stack: &mut ItemStack, id: i32, data: Vec<u8>) {
    stack.removed_components.retain(|v| *v != id);
    stack.components.retain(|c| c.type_id != id);
    let mut default = stack.clone();
    default.components.clear();
    default.removed_components.clear();
    if component(&default, id).ok().flatten().as_ref() != Some(&data) {
        stack.components.push(Component { type_id: id, data });
        stack.components.sort_by_key(|c| c.type_id);
    }
}
fn remove_component(stack: &mut ItemStack, id: i32) {
    stack.components.retain(|c| c.type_id != id);
    if !stack.removed_components.contains(&id) {
        stack.removed_components.push(id);
        stack.removed_components.sort_unstable();
    }
}
fn set_int_component(stack: &mut ItemStack, id: i32, value: i32) {
    let mut w = Writer::new();
    w.varint(value);
    set_component(stack, id, w.into_inner());
}

fn canonical(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}
fn validate_grid(width: usize, height: usize, items: &[ItemStack]) -> Result<()> {
    ensure!(
        width <= 3 && height <= 3 && width.checked_mul(height) == Some(items.len()),
        "invalid crafting grid dimensions"
    );
    Ok(())
}
fn accepts(ingredient: &[i32], stack: &ItemStack) -> bool {
    !stack.is_empty() && ingredient.contains(&stack.item_id)
}

fn occupied_bounds(
    width: usize,
    height: usize,
    items: &[ItemStack],
) -> Option<(usize, usize, usize, usize)> {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    for y in 0..height {
        for x in 0..width {
            if !items[y * width + x].is_empty() {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    (min_x < width && min_y < height).then_some((min_x, min_y, max_x + 1, max_y + 1))
}
fn shaped_matches(recipe: &Recipe, width: usize, height: usize, items: &[ItemStack]) -> bool {
    let Some((min_x, min_y, max_x, max_y)) = occupied_bounds(width, height, items) else {
        return false;
    };
    let pattern: Vec<Vec<char>> = recipe.pattern.iter().map(|r| r.chars().collect()).collect();
    let Some(first) = pattern.first() else {
        return false;
    };
    let (pw, ph) = (first.len(), pattern.len());
    if max_x - min_x != pw || max_y - min_y != ph || pattern.iter().any(|row| row.len() != pw) {
        return false;
    }
    [false, true].into_iter().any(|mirror| {
        (0..ph).all(|y| {
            (0..pw).all(|x| {
                let symbol = pattern[y][if mirror { pw - x - 1 } else { x }];
                let stack = &items[(min_y + y) * width + min_x + x];
                if symbol == ' ' {
                    stack.is_empty()
                } else {
                    recipe
                        .key
                        .get(&symbol.to_string())
                        .is_some_and(|i| accepts(i, stack))
                }
            })
        })
    })
}
fn shapeless_matches(ingredients: &[Vec<i32>], items: &[ItemStack]) -> bool {
    let stacks: Vec<&ItemStack> = items.iter().filter(|s| !s.is_empty()).collect();
    if stacks.len() != ingredients.len() || ingredients.is_empty() {
        return false;
    }
    // A greedy match is incorrect for intersecting item tags. Find a full
    // bipartite matching with augmenting paths, exactly one item per slot.
    fn augment(
        i: usize,
        ingredients: &[Vec<i32>],
        stacks: &[&ItemStack],
        seen: &mut [bool],
        assignments: &mut [Option<usize>],
    ) -> bool {
        for s in 0..stacks.len() {
            if seen[s] || !accepts(&ingredients[i], stacks[s]) {
                continue;
            }
            seen[s] = true;
            if assignments[s].is_none()
                || augment(
                    assignments[s].unwrap(),
                    ingredients,
                    stacks,
                    seen,
                    assignments,
                )
            {
                assignments[s] = Some(i);
                return true;
            }
        }
        false
    }
    let mut assignments = vec![None; stacks.len()];
    (0..ingredients.len()).all(|i| {
        augment(
            i,
            ingredients,
            &stacks,
            &mut vec![false; stacks.len()],
            &mut assignments,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Component;
    use serde_json::Value;
    fn stack(id: i32) -> ItemStack {
        ItemStack::new(id, 1).unwrap()
    }
    #[test]
    fn official_recipe_data_count_and_recursive_tags() {
        let book = RecipeBook::vanilla();
        assert_eq!(book.len(), 1290);
        let oak = book.item_id("oak_log").unwrap();
        let result = book
            .evaluate(
                "oak_planks",
                &RecipeInput::crafting(
                    2,
                    2,
                    vec![
                        stack(oak),
                        ItemStack::empty(),
                        ItemStack::empty(),
                        ItemStack::empty(),
                    ],
                )
                .unwrap(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            result.stack,
            ItemStack::new(book.item_id("oak_planks").unwrap(), 4).unwrap()
        );
    }
    #[test]
    fn shaped_translation_mirror_and_outer_occupied_rectangle() {
        let book = RecipeBook::vanilla();
        let wood = stack(book.item_id("oak_planks").unwrap());
        let stick = stack(book.item_id("stick").unwrap());
        let empty = ItemStack::empty();
        for mirror in [false, true] {
            let row = if mirror {
                vec![empty.clone(), stick.clone(), wood.clone()]
            } else {
                vec![empty.clone(), wood.clone(), stick.clone()]
            };
            let mut slots = vec![empty.clone(); 3];
            slots.extend(row.clone());
            slots.extend(row);
            assert!(book
                .evaluate("wooden_axe", &RecipeInput::crafting(3, 3, slots).unwrap())
                .unwrap()
                .is_none());
        }
        // A two by two stick recipe can be translated within the 3x3 grid.
        let mut slots = vec![empty; 9];
        slots[5] = wood.clone();
        slots[8] = wood;
        assert_eq!(
            book.evaluate("stick", &RecipeInput::crafting(3, 3, slots).unwrap())
                .unwrap()
                .unwrap()
                .stack
                .count,
            4
        );
    }
    #[test]
    fn overlapping_shapeless_ingredients_need_augmenting_matching() {
        assert!(shapeless_matches(
            &[vec![1, 2], vec![1]],
            &[stack(1), stack(2)]
        ));
        assert!(!shapeless_matches(
            &[vec![1], vec![1]],
            &[stack(1), stack(2)]
        ));
    }
    #[test]
    fn smithing_keeps_component_patch_and_cake_returns_buckets() {
        let book = RecipeBook::vanilla();
        let mut base = stack(book.item_id("diamond_sword").unwrap());
        base.components.push(Component {
            type_id: 3,
            data: vec![7],
        });
        let output = book
            .evaluate(
                "netherite_sword_smithing",
                &RecipeInput::Smithing {
                    template: stack(book.item_id("netherite_upgrade_smithing_template").unwrap()),
                    base: base.clone(),
                    addition: stack(book.item_id("netherite_ingot").unwrap()),
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(output.stack.components, base.components);
        assert_eq!(
            output.stack.item_id,
            book.item_id("netherite_sword").unwrap()
        );
        let items = [
            "milk_bucket",
            "milk_bucket",
            "milk_bucket",
            "sugar",
            "egg",
            "sugar",
            "wheat",
            "wheat",
            "wheat",
        ]
        .map(|n| stack(book.item_id(n).unwrap()))
        .to_vec();
        let output = book
            .evaluate("cake", &RecipeInput::crafting(3, 3, items).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(output.remaining.iter().filter(|i| !i.is_empty()).count(), 3);
        assert!(output.remaining[..3]
            .iter()
            .all(|i| i.item_id == book.item_id("bucket").unwrap()));
    }
    #[test]
    fn all_special_serializers_and_smithing_trims_match_actual_java() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let data: Value =
            serde_json::from_str(include_str!("../tools/crafting_special_vectors.json")).unwrap();
        assert!(
            data["errors"].as_array().unwrap().is_empty(),
            "{}",
            data["errors"]
        );
        let stacks = data["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                ItemStack::decode_exact(&STANDARD.decode(v.as_str().unwrap()).unwrap()).unwrap()
            })
            .collect::<Vec<_>>();
        let book = RecipeBook::vanilla();
        let equal = |a: &ItemStack, b: &ItemStack, name: &Value| {
            assert_eq!(a.count, b.count, "{name}: count");
            assert_eq!(a.item_id, b.item_id, "{name}: item");
            for id in 0..57 {
                assert_eq!(
                    a.component_value(id)
                        .unwrap_or_else(|e| panic!("{name}: actual component{id}: {e:#}")),
                    b.component_value(id)
                        .unwrap_or_else(|e| panic!("{name}: expected component{id}: {e:#}")),
                    "{name}: component {id}"
                );
            }
        };
        for v in data["vectors"].as_array().unwrap() {
            let items = v["input_indices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| stacks[i.as_u64().unwrap() as usize].clone())
                .collect::<Vec<_>>();
            let recipe_id = v["recipe_id"].as_str().unwrap();
            let input = if book.recipe_type(recipe_id) == Some("smithing_trim") {
                RecipeInput::Smithing {
                    template: items[0].clone(),
                    base: items[1].clone(),
                    addition: items[2].clone(),
                }
            } else {
                RecipeInput::crafting(
                    v["width"].as_u64().unwrap() as usize,
                    v["height"].as_u64().unwrap() as usize,
                    items,
                )
                .unwrap()
            };
            let mut context = RecipeContext::default();
            if let Some(info) = v.get("map_context") {
                if !info["missing"].as_bool().unwrap() {
                    context.maps.insert(
                        123,
                        MapRecipeData {
                            scale: info["scale"].as_i64().unwrap() as i8,
                            exploration: info["exploration"].as_bool().unwrap(),
                        },
                    );
                }
            }
            let actual = book
                .evaluate_with_context(recipe_id, &input, &context)
                .unwrap_or_else(|e| panic!("{}: {e:#}", v["name"]));
            assert_eq!(
                actual.is_some(),
                v["expected"]["matches"].as_bool().unwrap(),
                "{}",
                v["name"]
            );
            if let Some(actual) = actual {
                equal(
                    &actual.stack,
                    &stacks[v["result_index"].as_u64().unwrap() as usize],
                    &v["name"],
                );
                let remaining = if let RecipeInput::Crafting {
                    width,
                    height,
                    items,
                } = &input
                {
                    let (x0, y0, x1, y1) = occupied_bounds(*width, *height, items).unwrap();
                    let mut slots = Vec::new();
                    for y in y0..y1 {
                        for x in x0..x1 {
                            slots.push(actual.remaining[y * width + x].clone());
                        }
                    }
                    slots
                } else {
                    actual.remaining
                };
                let expected = v["remaining_indices"].as_array().unwrap();
                assert_eq!(remaining.len(), expected.len(), "{}", v["name"]);
                for (a, e) in remaining.iter().zip(expected) {
                    equal(a, &stacks[e.as_u64().unwrap() as usize], &v["name"]);
                }
            }
        }
        assert_eq!(data["vectors"].as_array().unwrap().len(), 835);
    }
    #[test]
    fn unchanged_mojang_recipe_classes_match_generated_vectors() {
        let data: Value =
            serde_json::from_str(include_str!("../tools/crafting_vectors.json")).unwrap();
        assert!(
            data["errors"].as_array().unwrap().is_empty(),
            "oracle errors: {}",
            data["errors"]
        );
        let book = RecipeBook::vanilla();
        let mut checked = 0;
        for v in data["vectors"].as_array().unwrap() {
            let i = &v["input"];
            let items = i["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| {
                    ItemStack::new(
                        s["item_id"].as_i64().unwrap() as i32,
                        s["count"].as_i64().unwrap() as i32,
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let input = match i["kind"].as_str().unwrap() {
                "crafting" => RecipeInput::crafting(
                    i["width"].as_u64().unwrap() as usize,
                    i["height"].as_u64().unwrap() as usize,
                    items,
                )
                .unwrap(),
                "smithing" => RecipeInput::Smithing {
                    template: items[0].clone(),
                    base: items[1].clone(),
                    addition: items[2].clone(),
                },
                _ => RecipeInput::Single(items[0].clone()),
            };
            let actual = book
                .evaluate(v["recipe_id"].as_str().unwrap(), &input)
                .unwrap();
            let e = &v["expected"];
            assert_eq!(
                actual.is_some(),
                e["matches"].as_bool().unwrap(),
                "{}",
                v["name"]
            );
            if let Some(actual) = actual {
                let result = &e["result"];
                assert_eq!(
                    actual.stack.item_id,
                    book.item_id(result["id"].as_str().unwrap()).unwrap(),
                    "{}",
                    v["name"]
                );
                assert_eq!(
                    actual.stack.count,
                    result["count"].as_i64().unwrap_or(1) as i32,
                    "{}",
                    v["name"]
                );
                // Oracle CraftingInput.of trims empty outer rows/columns.
                let original = input.items();
                let remaining = if let RecipeInput::Crafting {
                    width,
                    height,
                    items,
                } = &input
                {
                    let (min_x, min_y, max_x, max_y) =
                        occupied_bounds(*width, *height, items).unwrap();
                    {
                        let mut slots = Vec::new();
                        for y in min_y..max_y {
                            for x in min_x..max_x {
                                slots.push(actual.remaining[y * width + x].clone());
                            }
                        }
                        slots
                    }
                } else {
                    actual.remaining.clone()
                };
                let _ = original;
                let expected = e["remaining"].as_array().unwrap();
                assert_eq!(remaining.len(), expected.len(), "{}", v["name"]);
                for (a, e) in remaining.iter().zip(expected) {
                    assert_eq!(a.is_empty(), e.is_null(), "{}", v["name"]);
                    if !e.is_null() {
                        assert_eq!(
                            a.item_id,
                            book.item_id(e["id"].as_str().unwrap()).unwrap(),
                            "{}",
                            v["name"]
                        );
                    }
                }
            }
            checked += 1;
        }
        assert_eq!(checked, 3150);
    }
}
