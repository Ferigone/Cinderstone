//! Recipe unlocks after scripted pickups (`fixtures/recipe-unlock-vectors.json`).

use super::player::{PlayerAdvancements, RecipeAdvancements};
use crate::inventory::ItemStack;
use serde_json::Value;

fn stack(spec: &Value) -> ItemStack {
    let mut stack = crate::loot::enchant::new_stack(spec["id"].as_str().unwrap()).unwrap();
    stack.count = spec["count"].as_i64().unwrap() as i32;
    stack
}

#[test]
fn vanilla_recipe_tree_loads() {
    let catalogue = RecipeAdvancements::vanilla();
    assert_eq!(catalogue.len(), 1277); // 1,276 recipe advancements and the root
    assert!(catalogue.index_of("minecraft:recipes/root").is_some());
    assert!(catalogue.index_of("minecraft:story/root").is_none());
}

/// Every pickup completes exactly the advancements the original's criteria select, and the
/// rewards give the same recipe set.
#[test]
fn scripted_pickups_unlock_what_the_original_unlocks() {
    let catalogue = RecipeAdvancements::vanilla();
    let doc: Value = serde_json::from_str(include_str!("../../fixtures/recipe-unlock-vectors.json")).unwrap();
    let mut steps = 0;
    for script in doc["scripts"].as_array().unwrap() {
        let mut player = PlayerAdvancements::new(catalogue);
        let mut recipes = std::collections::BTreeSet::new();
        for step in script["steps"].as_array().unwrap() {
            let mut inventory = vec![ItemStack::empty(); 41];
            for slot in step["inventory"].as_array().unwrap() {
                inventory[slot["slot"].as_u64().unwrap() as usize] = stack(&slot["item"]);
            }
            let mut completed = Vec::new();
            for listener in player.inventory_changed(catalogue, &inventory, &stack(&step["changed"])).unwrap() {
                if let Some(rewards) = player.award(catalogue, listener, 1_700_000_000).unwrap() {
                    recipes.extend(rewards.iter().cloned());
                    completed.push(catalogue.id(listener.advancement).to_owned());
                }
            }
            let expected: Vec<String> = serde_json::from_value(step["completed"].clone()).unwrap();
            assert_eq!(completed, expected, "step {step}");
            steps += 1;
        }
        let expected: Vec<String> = serde_json::from_value(script["recipes"].clone()).unwrap();
        assert_eq!(recipes.into_iter().collect::<Vec<_>>(), expected);
    }
    assert!(steps > 300);
}

#[test]
fn progress_survives_save_and_load() {
    let catalogue = RecipeAdvancements::vanilla();
    let mut player = PlayerAdvancements::new(catalogue);
    let tick = player.tick(catalogue);
    assert_eq!(tick.len(), 1);
    let rewards = player.award(catalogue, tick[0], 1_700_000_000).unwrap().unwrap().to_vec();
    assert_eq!(rewards, ["minecraft:crafting_table"]);
    assert!(player.tick(catalogue).is_empty());
    let text = player.save(catalogue).unwrap();
    assert!(text.contains("\"minecraft:recipes/decorations/crafting_table\""));
    let loaded = PlayerAdvancements::load(catalogue, &text).unwrap();
    assert_eq!(loaded, player);
    // The recipe's own `has_the_recipe` listener is gone with the completed advancement.
    assert!(loaded.recipe_unlocked(catalogue, "minecraft:crafting_table").is_empty());
    assert_eq!(loaded.recipe_unlocked(catalogue, "minecraft:oak_planks").len(), 1);
    // Entries of other trees are kept for writing back.
    let foreign = r#"{"minecraft:story/root":{"criteria":{"crafting_table":"2024-01-01 00:00:00 +0000"},"done":true},"DataVersion":3953}"#;
    let kept = PlayerAdvancements::load(catalogue, foreign).unwrap();
    assert!(kept.save(catalogue).unwrap().contains("minecraft:story/root"));
}
