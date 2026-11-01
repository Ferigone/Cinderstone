//! Certified shipped-recipe ordering and feature data, borrowing actual owners.
//!
//! This is metadata for the existing immutable RecipeBook, not another recipe
//! manager or Level. Main must pass its actual enabled-feature snapshot and
//! retain its player/world/menu identity and callback transaction.
use crate::{
    crafting::RecipeBook,
    inventory::{self, ItemStack},
    workstations::{
        smithing::SmithingRecipeOrder,
        stonecutter::{PinnedStonecutterRecipes, StonecutterRecipes},
    },
};
use anyhow::{anyhow, ensure, Result};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::OnceLock};

#[derive(Deserialize)]
struct Provenance {
    inner_server_sha256: String,
    server_mappings_sha1: String,
    recipe_catalog_sha256: String,
    item_catalog_sha256: String,
    no_server_or_world: bool,
}
#[derive(Deserialize)]
struct FeatureFlags {
    vanilla: Vec<String>,
    default: Vec<String>,
    air_required: Vec<String>,
    empty_stack_enabled_with_empty_flags: bool,
}
#[derive(Deserialize)]
struct Stonecutting {
    recipe_id: String,
    result_item_id: i32,
    result_item: String,
    description_id: String,
    required_features: Vec<String>,
}
#[derive(Deserialize)]
struct Smithing {
    recipe_id: String,
    recipe_kind: String,
    preview_result_item_id: i32,
    preview_result_item: String,
    preview_description_id: String,
    preview_required_features: Vec<String>,
}
#[derive(Deserialize)]
struct BaseItem {
    item_id: i32,
    item: String,
    required_features: Vec<String>,
}
#[derive(Deserialize)]
struct DefaultData {
    schema_version: i32,
    minecraft: String,
    protocol: i32,
    provenance: Provenance,
    feature_flags: FeatureFlags,
    stonecutting: Vec<Stonecutting>,
    smithing_input_order: Vec<String>,
    smithing_result_order: Vec<String>,
    smithing_recipes: Vec<Smithing>,
    smithing_base_items: Vec<BaseItem>,
}
fn data() -> &'static DefaultData {
    static DATA: OnceLock<DefaultData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("default_recipe_context_1_21.json"))
            .expect("checked-in original-class recipe context metadata")
    })
}
fn digest(bytes: &[u8]) -> String {
    openssl::sha::sha256(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn verify_data() -> Result<()> {
    let d = data();
    let book = RecipeBook::vanilla();
    ensure!(
        d.schema_version == 1 && d.minecraft == "1.21" && d.protocol == 767,
        "wrong default recipe context version"
    );
    ensure!(
        d.provenance.inner_server_sha256
            == "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
            && d.provenance.server_mappings_sha1 == "31c77994d96f05ba25a870ada70f47f315330437"
            && d.provenance.no_server_or_world,
        "wrong original-class recipe context provenance"
    );
    ensure!(
        digest(include_bytes!("../../fixtures/crafting-1.21.json"))
            == d.provenance.recipe_catalog_sha256
            && digest(include_bytes!("../../fixtures/items-1.21.json"))
                == d.provenance.item_catalog_sha256,
        "native catalogs differ from the observed recipe context"
    );
    ensure!(
        d.feature_flags.vanilla == ["minecraft:vanilla"]
            && d.feature_flags.default == d.feature_flags.vanilla
            && d.feature_flags.empty_stack_enabled_with_empty_flags,
        "unsupported observed feature profile"
    );
    let stone_ids: BTreeSet<_> = d
        .stonecutting
        .iter()
        .map(|r| r.recipe_id.as_str())
        .collect();
    let expected_stone: BTreeSet<_> = book
        .recipe_ids()
        .filter(|id| book.recipe_type(id) == Some("stonecutting"))
        .collect();
    ensure!(
        stone_ids.len() == d.stonecutting.len() && stone_ids == expected_stone,
        "incomplete observed stonecutting snapshot"
    );
    for r in &d.stonecutting {
        ensure!(
            book.item_id(&r.result_item) == Some(r.result_item_id)
                && inventory::item_default_block_state(r.result_item_id).is_some()
                && r.description_id
                    == format!(
                        "block.minecraft.{}",
                        inventory::item_name(r.result_item_id)
                            .ok_or_else(|| anyhow!("unknown default result item"))?
                    ),
            "stonecutting registry/description differs from observed defaults"
        );
    }
    let expected_smithing: BTreeSet<_> = book
        .recipe_ids()
        .filter(|id| {
            matches!(
                book.recipe_type(id),
                Some("smithing_trim" | "smithing_transform")
            )
        })
        .collect();
    for ids in [&d.smithing_input_order, &d.smithing_result_order] {
        let observed: BTreeSet<_> = ids.iter().map(String::as_str).collect();
        ensure!(
            observed.len() == ids.len() && observed == expected_smithing,
            "incomplete observed smithing snapshot"
        );
    }
    let previews: BTreeSet<_> = d
        .smithing_recipes
        .iter()
        .map(|r| r.recipe_id.as_str())
        .collect();
    ensure!(
        previews.len() == d.smithing_recipes.len() && previews == expected_smithing,
        "incomplete observed smithing preview data"
    );
    for r in &d.smithing_recipes {
        ensure!(
            book.recipe_type(&r.recipe_id) == Some(r.recipe_kind.as_str())
                && book.item_id(&r.preview_result_item) == Some(r.preview_result_item_id),
            "smithing registry differs from observed defaults"
        );
    }
    let mut sorted = d.smithing_input_order.clone();
    sorted.sort_by_key(|id| {
        &d.smithing_recipes
            .iter()
            .find(|r| &r.recipe_id == id)
            .unwrap()
            .preview_description_id
    });
    ensure!(
        sorted == d.smithing_result_order,
        "observed stable smithing sort differs"
    );
    for item in &d.smithing_base_items {
        ensure!(
            book.item_id(&item.item) == Some(item.item_id),
            "wrong smithing base registry"
        );
    }
    Ok(())
}
fn verified_data() -> Result<&'static DefaultData> {
    static VERIFIED: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    VERIFIED
        .get_or_init(|| verify_data().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|message| anyhow!("{message}"))?;
    Ok(data())
}

/// The borrowed flags must come from the actual native Level/configuration
/// owner, whose current supported snapshot is exactly minecraft:vanilla.
/// Alternate catalogs, registries, datapack reloads and experimental profiles
/// are rejected; this does not authorize a fabricated or always-enabled host.
pub struct PinnedDefaultRecipeContext<'a> {
    book: &'a RecipeBook,
    enabled: &'a BTreeSet<String>,
}
impl<'a> PinnedDefaultRecipeContext<'a> {
    pub fn new(book: &'a RecipeBook, enabled: &'a BTreeSet<String>) -> Result<Self> {
        ensure!(
            std::ptr::eq(book, RecipeBook::vanilla()),
            "requires the canonical shipped recipe owner"
        );
        let d = verified_data()?;
        let expected: BTreeSet<_> = d.feature_flags.default.iter().cloned().collect();
        ensure!(
            *enabled == expected,
            "unsupported actual enabled-feature snapshot"
        );
        Ok(Self { book, enabled })
    }
    pub fn smithing_order(&self) -> Result<SmithingRecipeOrder> {
        let d = verified_data()?;
        SmithingRecipeOrder::new(
            d.smithing_input_order.clone(),
            d.smithing_result_order.clone(),
        )
    }
    pub fn result_enabled(&self, result: &ItemStack) -> Result<bool> {
        // ItemStack.isItemEnabled short-circuits an empty stack before AIR's
        // required features. The remaining lookup is certified default data.
        if result.is_empty() {
            return Ok(true);
        }
        let d = verified_data()?;
        let required = if result.item_id == self.book.item_id("air").unwrap() {
            Some(&d.feature_flags.air_required)
        } else {
            d.stonecutting
                .iter()
                .find(|r| r.result_item_id == result.item_id)
                .map(|r| &r.required_features)
                .or_else(|| {
                    d.smithing_recipes
                        .iter()
                        .find(|r| r.preview_result_item_id == result.item_id)
                        .map(|r| &r.preview_required_features)
                })
                .or_else(|| {
                    d.smithing_base_items
                        .iter()
                        .find(|r| r.item_id == result.item_id)
                        .map(|r| &r.required_features)
                })
        }
        .ok_or_else(|| anyhow!("result has no certified default workstation feature data"))?;
        Ok(required.iter().all(|flag| self.enabled.contains(flag)))
    }
}
impl StonecutterRecipes for PinnedDefaultRecipeContext<'_> {
    fn matching_recipes(&self, input: &ItemStack) -> Result<Vec<String>> {
        PinnedStonecutterRecipes::new(|result: &ItemStack| self.result_enabled(result))
            .matching_recipes(input)
    }
    fn assemble(&self, recipe_id: &str, input: &ItemStack) -> Result<ItemStack> {
        PinnedStonecutterRecipes::new(|result: &ItemStack| self.result_enabled(result))
            .assemble(recipe_id, input)
    }
    fn result_enabled(&self, result: &ItemStack) -> Result<bool> {
        Self::result_enabled(self, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workstations::smithing::SmithingState;
    fn item(name: &str) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), 1).unwrap()
    }
    #[test]
    fn observed_default_context_drives_native_stonecutting_and_smithing() -> Result<()> {
        let enabled = BTreeSet::from(["minecraft:vanilla".to_owned()]);
        let context = PinnedDefaultRecipeContext::new(RecipeBook::vanilla(), &enabled)?;
        let stone = item("stone");
        let recipes = context.matching_recipes(&stone)?;
        assert!(!recipes.is_empty());
        for recipe in recipes {
            let result = context.assemble(&recipe, &stone)?;
            assert!(context.result_enabled(&result)?);
        }
        let mut smithing = SmithingState::new(context.smithing_order()?);
        smithing.set_input(0, item("netherite_upgrade_smithing_template"))?;
        smithing.set_input(1, item("diamond_axe"))?;
        smithing.set_input(2, item("netherite_ingot"))?;
        assert_eq!(smithing.result(), &item("netherite_axe"));
        assert!(context.result_enabled(smithing.result())?);
        assert!(context.result_enabled(&ItemStack::empty())?);
        assert!(context.result_enabled(&item("bundle")).is_err());
        Ok(())
    }
    #[test]
    fn alternate_feature_or_recipe_owners_are_not_silently_defaulted() -> Result<()> {
        let mut enabled = BTreeSet::new();
        assert!(PinnedDefaultRecipeContext::new(RecipeBook::vanilla(), &enabled).is_err());
        enabled.insert("minecraft:vanilla".to_owned());
        enabled.insert("minecraft:bundle".to_owned());
        assert!(PinnedDefaultRecipeContext::new(RecipeBook::vanilla(), &enabled).is_err());
        enabled.remove("minecraft:bundle");
        let different_owner: RecipeBook =
            serde_json::from_str(include_str!("../../fixtures/crafting-1.21.json"))?;
        assert!(PinnedDefaultRecipeContext::new(&different_owner, &enabled).is_err());
        Ok(())
    }
}
