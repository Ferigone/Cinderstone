//! `PlayerAdvancements` for the vanilla recipe advancements (`recipes/**`, WP-E6 #27).
//!
//! The catalogue is the shipped `data/minecraft/advancement/recipes/**` tree, parsed by
//! [`super::definition::parse`]. Its criteria use five triggers: `inventory_changed`,
//! `recipe_unlocked`, `enter_block`, `tick` and `impossible` (the root). A player's listeners are
//! the criteria that are not done, of advancements that are not done; `award` grants a criterion
//! and, when that completes the advancement, returns its reward recipes, as
//! `PlayerAdvancements.award` → `AdvancementRewards.grant` → `awardRecipesByKey` does.
//!
//! Recipe advancements have no display, so they never appear in
//! `ClientboundUpdateAdvancementsPacket`; the other vanilla trees (story, nether, end, adventure,
//! husbandry) are not loaded here (WP-E5 #26).

use super::definition::{self, Advancement, Criterion};
use super::progress::{parse_progress_file, write_progress_file, AdvancementProgress, ProgressError};
use super::triggers::{self, Match};
use crate::inventory::ItemStack;
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Where one criterion listens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Listener {
    pub advancement: usize,
    criterion: usize,
}

/// The loaded recipe advancements, indexed by trigger.
#[derive(Debug)]
pub struct RecipeAdvancements {
    ids: Vec<String>,
    advancements: Vec<Advancement>,
    criterion_names: Vec<Vec<String>>,
    inventory_changed: Vec<Listener>,
    recipe_unlocked: BTreeMap<String, Vec<Listener>>,
    enter_block: Vec<Listener>,
    tick: Vec<Listener>,
}

impl RecipeAdvancements {
    /// The vanilla 1.21 recipe advancements (`fixtures/advancements-vanilla-1.21.json`).
    pub fn vanilla() -> &'static Self {
        static DATA: OnceLock<RecipeAdvancements> = OnceLock::new();
        DATA.get_or_init(|| {
            let doc: Value = serde_json::from_str(include_str!("../../fixtures/advancements-vanilla-1.21.json"))
                .expect("vanilla advancement capture");
            let files = doc["advancement"].as_object().expect("advancement map");
            let entries = files
                .iter()
                .filter(|(id, _)| id.starts_with("minecraft:recipes/"))
                .map(|(id, text)| {
                    let json = crate::components::lenient::parse(text.as_str().expect("advancement text")).expect("advancement json");
                    (id.clone(), definition::parse(&json).expect("vanilla recipe advancement"))
                });
            Self::new(entries).expect("vanilla recipe advancements")
        })
    }

    /// Index a set of advancements (sorted by id). Only the five recipe-tree triggers are
    /// accepted, and no criterion may carry a `player` predicate (no entity context is ported
    /// here).
    pub fn new(entries: impl IntoIterator<Item = (String, Advancement)>) -> Result<Self> {
        let mut sorted: Vec<(String, Advancement)> = entries.into_iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let mut out = Self {
            ids: Vec::new(),
            advancements: Vec::new(),
            criterion_names: Vec::new(),
            inventory_changed: Vec::new(),
            recipe_unlocked: BTreeMap::new(),
            enter_block: Vec::new(),
            tick: Vec::new(),
        };
        for (index, (id, advancement)) in sorted.into_iter().enumerate() {
            ensure!(advancement.unvalidated_clauses().is_empty(), "{id}: unvalidated predicate clauses");
            let mut names = Vec::new();
            for (position, (name, criterion)) in advancement.criteria.iter().enumerate() {
                ensure!(!criterion.conditions.contains_key("player"), "{id}/{name}: player predicates are not supported");
                let listener = Listener { advancement: index, criterion: position };
                match criterion.trigger.as_str() {
                    "minecraft:inventory_changed" => out.inventory_changed.push(listener),
                    "minecraft:recipe_unlocked" => match criterion.conditions.get("recipe") {
                        Some(definition::Field::Id(recipe)) => out.recipe_unlocked.entry(recipe.clone()).or_default().push(listener),
                        _ => bail!("{id}/{name}: recipe_unlocked without a recipe"),
                    },
                    "minecraft:enter_block" => out.enter_block.push(listener),
                    "minecraft:tick" => out.tick.push(listener),
                    "minecraft:impossible" => {}
                    other => bail!("{id}/{name}: trigger {other} is not supported in the recipe tree"),
                }
                names.push(name.clone());
            }
            out.ids.push(id);
            out.advancements.push(advancement);
            out.criterion_names.push(names);
        }
        Ok(out)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn id(&self, index: usize) -> &str {
        &self.ids[index]
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.ids.binary_search_by(|probe| probe.as_str().cmp(id)).ok()
    }

    pub fn advancement(&self, index: usize) -> &Advancement {
        &self.advancements[index]
    }

    pub fn criterion_name(&self, listener: Listener) -> &str {
        &self.criterion_names[listener.advancement][listener.criterion]
    }

    fn criterion(&self, listener: Listener) -> &Criterion {
        &self.advancements[listener.advancement].criteria[self.criterion_name(listener)]
    }
}

/// One player's progress over a [`RecipeAdvancements`] catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerAdvancements {
    progress: Vec<AdvancementProgress>,
    /// Entries of the loaded file for advancements this catalogue does not hold. The original
    /// drops them on load (with a warning) and so never writes them back; they are kept here so
    /// that a world's other trees survive until those trees are loaded (#26).
    foreign: Vec<(String, AdvancementProgress)>,
}

impl PlayerAdvancements {
    pub fn new(catalogue: &RecipeAdvancements) -> Self {
        let progress = catalogue
            .advancements
            .iter()
            .map(|advancement| {
                let mut progress = AdvancementProgress::new();
                progress.update(advancement.requirements.clone());
                progress
            })
            .collect();
        Self { progress, foreign: Vec::new() }
    }

    /// `PlayerAdvancements.load` of an `advancements/<uuid>.json` text.
    pub fn load(catalogue: &RecipeAdvancements, text: &str) -> Result<Self, ProgressError> {
        let mut out = Self::new(catalogue);
        for (id, mut progress) in parse_progress_file(text)? {
            match catalogue.index_of(&id) {
                Some(index) => {
                    progress.update(catalogue.advancements[index].requirements.clone());
                    out.progress[index] = progress;
                }
                None => out.foreign.push((id, progress)),
            }
        }
        Ok(out)
    }

    /// `PlayerAdvancements.save`: the document text.
    pub fn save(&self, catalogue: &RecipeAdvancements) -> Result<String, ProgressError> {
        let entries = self
            .progress
            .iter()
            .enumerate()
            .map(|(index, progress)| (catalogue.id(index), progress))
            .chain(self.foreign.iter().map(|(id, progress)| (id.as_str(), progress)));
        let document = write_progress_file(entries)?;
        Ok(serde_json::to_string_pretty(&document).expect("JSON value"))
    }

    pub fn progress(&self, index: usize) -> &AdvancementProgress {
        &self.progress[index]
    }

    /// A criterion is listening while neither it nor its advancement is done
    /// (`registerListeners` / `unregisterListeners`).
    pub fn listening(&self, catalogue: &RecipeAdvancements, listener: Listener) -> bool {
        let progress = &self.progress[listener.advancement];
        !progress.is_done() && progress.criterion(catalogue.criterion_name(listener)).is_some_and(|c| !c.is_done())
    }

    /// `InventoryChangeTrigger.trigger(player, inventory, changed)`: the listening criteria that
    /// match. `inventory` is the 41 `Inventory` slots.
    pub fn inventory_changed(&self, catalogue: &RecipeAdvancements, inventory: &[ItemStack], changed: &ItemStack) -> Result<Vec<Listener>> {
        let mut matched = Vec::new();
        for &listener in &catalogue.inventory_changed {
            if self.listening(catalogue, listener) {
                let result = triggers::inventory_changed(catalogue.criterion(listener), inventory, changed)
                    .map_err(|e| anyhow::anyhow!("{}: {e}", catalogue.id(listener.advancement)))?;
                if result == Match::Matches {
                    matched.push(listener);
                }
            }
        }
        Ok(matched)
    }

    /// `RecipeUnlockedTrigger.trigger(player, recipe)`.
    pub fn recipe_unlocked(&self, catalogue: &RecipeAdvancements, recipe: &str) -> Vec<Listener> {
        let listeners = catalogue.recipe_unlocked.get(recipe).map_or(&[][..], Vec::as_slice);
        listeners
            .iter()
            .copied()
            .filter(|&listener| self.listening(catalogue, listener))
            .filter(|&listener| triggers::recipe_unlocked(catalogue.criterion(listener), recipe) == Match::Matches)
            .collect()
    }

    /// `EnterBlockTrigger.trigger(player, state)` for one block state the player is inside.
    pub fn enter_block(&self, catalogue: &RecipeAdvancements, state: u32) -> Result<Vec<Listener>> {
        let mut matched = Vec::new();
        for &listener in &catalogue.enter_block {
            if self.listening(catalogue, listener) {
                let result = triggers::block_state(catalogue.criterion(listener), state).map_err(|e| anyhow::anyhow!("{e}"))?;
                if result == Match::Matches {
                    matched.push(listener);
                }
            }
        }
        Ok(matched)
    }

    /// Whether any `enter_block` criterion is still listening (the original only walks the
    /// blocks an entity is inside when the trigger has listeners for that player).
    pub fn has_enter_block_listeners(&self, catalogue: &RecipeAdvancements) -> bool {
        catalogue.enter_block.iter().any(|&listener| self.listening(catalogue, listener))
    }

    /// `PlayerTrigger` `tick`: every listening `tick` criterion matches.
    pub fn tick(&self, catalogue: &RecipeAdvancements) -> Vec<Listener> {
        catalogue.tick.iter().copied().filter(|&listener| self.listening(catalogue, listener)).collect()
    }

    /// `PlayerAdvancements.award(advancement, criterion)` at `now` (epoch seconds). Returns the
    /// reward recipes when this grant completes the advancement; nothing when the criterion was
    /// already done or its advancement was already complete (the listener was unregistered).
    pub fn award<'a>(&mut self, catalogue: &'a RecipeAdvancements, listener: Listener, now: i64) -> Result<Option<&'a [String]>> {
        let name = catalogue.criterion_name(listener);
        let progress = self.progress.get_mut(listener.advancement).context("advancement index")?;
        if progress.is_done() {
            return Ok(None);
        }
        if progress.grant_progress(name, now) && progress.is_done() {
            let rewards = &catalogue.advancements[listener.advancement].rewards;
            ensure!(rewards.experience == 0 && rewards.loot.is_empty() && rewards.function.is_none(),
                "{}: only recipe rewards are supported", catalogue.id(listener.advancement));
            return Ok(Some(&rewards.recipes));
        }
        Ok(None)
    }
}

/// The 41 `Inventory` slots (hotbar, main, armour feet→head, offhand) from the 46 `InventoryMenu`
/// slots.
pub fn inventory_from_menu(menu_slots: &[ItemStack]) -> Vec<ItemStack> {
    (36..45)
        .chain(9..36)
        .chain((5..9).rev())
        .chain(std::iter::once(45))
        .map(|index| menu_slots[index].clone())
        .collect()
}
