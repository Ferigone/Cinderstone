//! Registry-aware Java 1.21 repair and smithing-trim assembly.
//!
//! A context borrows one actual recipe/item/registry snapshot. It contains no
//! default registry, tag assignments, recipe manager, Level or advancement owner.
//! All queries are read-only and must error when that actual context is unavailable.
use crate::{
    crafting::{CraftResult, RecipeBook, RecipeInput},
    inventory::{Component, ItemStack},
    protocol::{Reader, Writer},
};
use anyhow::{ensure, Result};
use std::fmt::Debug;

/// EnchantmentHelper.getComponentType: only the actual enchanted-book item
/// selects STORED_ENCHANTMENTS; every other item selects ENCHANTMENTS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftingEnchantments {
    Ordinary,
    Stored,
}
impl CraftingEnchantments {
    pub fn component_id(self) -> i32 {
        match self {
            Self::Ordinary => 9,
            Self::Stored => 23,
        }
    }
    fn for_item(book: &RecipeBook, stack: &ItemStack) -> Self {
        if Some(stack.item_id) == book.item_id("enchanted_book") {
            Self::Stored
        } else {
            Self::Ordinary
        }
    }
}

/// Implement against the same actual HolderLookup.Provider/item and recipe
/// owners captured by the caller. Snapshot and holder keys retain their actual
/// identities; numeric IDs alone must not manufacture replacement holders.
/// No default/no-op production implementation or pinned-registry fallback exists.
pub trait SpecialRecipeRegistry {
    type Snapshot: Clone + Eq + Debug;
    type Enchantment: Clone + Eq + Debug;
    type Material: Clone + Eq + Debug;
    type Pattern: Clone + Eq + Debug;

    fn snapshot(&self) -> Self::Snapshot;
    /// Certify this RecipeBook's decoded ingredient/item identities belong to
    /// the captured actual recipe/item snapshot. A reload or different owner
    /// must error, rather than silently reusing the compiled default registry.
    fn validate_recipe_book(&self, book: &RecipeBook) -> Result<()>;
    /// Validate actual item/component holder ownership in this same snapshot.
    fn validate_item(&self, stack: &ItemStack) -> Result<()>;

    /// Actual enchantment registry holder encounter order. The native loop
    /// tests each holder against the ACTUAL EnchantmentTags.CURSE assignment.
    fn enchantments(&self) -> Result<Vec<Self::Enchantment>>;
    fn is_curse(&self, holder: &Self::Enchantment) -> Result<bool>;
    /// Query this actual holder in the selected typed component. Missing
    /// components/entries yield zero, as EnchantmentHelper does.
    fn enchantment_level(
        &self,
        stack: &ItemStack,
        component: CraftingEnchantments,
        holder: &Self::Enchantment,
    ) -> Result<i32>;
    /// Encode the actual new ItemEnchantments map, retaining real holder/map
    /// identity/order. The selected component ID must be 9 or 23 as requested.
    fn encode_enchantments(
        &self,
        component: CraftingEnchantments,
        values: &[(Self::Enchantment, i32)],
        show_in_tooltip: bool,
    ) -> Result<Component>;

    /// Actual trim registry encounter orders. Native assembly chooses the FIRST
    /// holder whose ingredient/template predicate matches, not a sorted ID map.
    fn trim_materials(&self) -> Result<Vec<Self::Material>>;
    fn material_matches(&self, holder: &Self::Material, addition: &ItemStack) -> Result<bool>;
    fn trim_patterns(&self) -> Result<Vec<Self::Pattern>>;
    fn pattern_matches(&self, holder: &Self::Pattern, template: &ItemStack) -> Result<bool>;
    /// ArmorTrim.hasPatternAndMaterial on the actual typed existing component.
    /// Ignore showInTooltip; preserve actual reference/direct holder equality.
    fn has_pattern_and_material(
        &self,
        base: &ItemStack,
        pattern: &Self::Pattern,
        material: &Self::Material,
    ) -> Result<bool>;
    /// Actual ArmorTrim material/pattern holder component encoding (ID 35).
    fn encode_trim(
        &self,
        material: &Self::Material,
        pattern: &Self::Pattern,
        show_in_tooltip: bool,
    ) -> Result<Component>;

    /// Recipe.getRemainingItems uses the ACTUAL Item.hasCraftingRemainingItem /
    /// getCraftingRemainingItem, constructing its default count-one stack.
    /// This is not FoodProperties.usingConvertsTo or an owned input copy.
    fn crafting_remainder(&self, input: &ItemStack) -> Result<ItemStack>;
}

/// A borrow of one captured real snapshot. A changing/replaced registry is an
/// error before a computed result can be returned. No live input is mutated.
pub struct SpecialRecipeContext<'a, R: SpecialRecipeRegistry> {
    registry: &'a R,
    snapshot: R::Snapshot,
}
impl<'a, R: SpecialRecipeRegistry> SpecialRecipeContext<'a, R> {
    pub fn new(registry: &'a R) -> Self {
        Self {
            registry,
            snapshot: registry.snapshot(),
        }
    }
    pub fn snapshot(&self) -> &R::Snapshot {
        &self.snapshot
    }
    pub fn validate_snapshot(&self) -> Result<()> {
        ensure!(
            self.registry.snapshot() == self.snapshot,
            "special-recipe registry snapshot changed; actual owner handoff required"
        );
        Ok(())
    }
    pub(crate) fn validate_book(&self, book: &RecipeBook) -> Result<()> {
        self.validate_snapshot()?;
        self.registry.validate_recipe_book(book)?;
        self.validate_snapshot()
    }
    fn validate_items(&self, items: &[ItemStack]) -> Result<()> {
        for stack in items {
            // Wire snapshots do not carry custom FoodProperties reference
            // identity through ItemStack.copy; keep that boundary explicit.
            crate::workstations::menu::require_component_context(stack)?;
            stack.encode_canonical()?;
            self.registry.validate_item(stack)?;
        }
        self.validate_snapshot()
    }
    fn remaining(&self, items: &[ItemStack]) -> Result<Vec<ItemStack>> {
        let mut out = Vec::with_capacity(items.len());
        for stack in items {
            let remaining = self.registry.crafting_remainder(stack)?;
            ensure!(
                remaining.is_empty() || remaining.count == 1,
                "actual Item crafting remainder must be empty or count one"
            );
            ensure!(
                !stack.is_empty() || remaining.is_empty(),
                "empty/AIR input cannot have a crafting remainder"
            );
            remaining.encode_canonical()?;
            self.registry.validate_item(&remaining)?;
            out.push(if remaining.is_empty() {
                ItemStack::empty()
            } else {
                remaining
            });
        }
        self.validate_snapshot()?;
        Ok(out)
    }
}
fn int_component(stack: &ItemStack, id: i32, default: i32) -> Result<i32> {
    let Some(bytes) = stack.effective_component_bytes(id) else {
        return Ok(default);
    };
    let mut reader = Reader::new(bytes);
    let value = reader.varint()?;
    reader.finish()?;
    Ok(value)
}
fn set_component(stack: &mut ItemStack, component: Component) {
    stack
        .removed_components
        .retain(|id| *id != component.type_id);
    stack
        .components
        .retain(|old| old.type_id != component.type_id);
    stack.components.push(component);
    stack.components.sort_by_key(|component| component.type_id);
    // Only actual stream-codec-proved prototype elisions are performed.
    stack.normalize_patch();
}
fn set_int(stack: &mut ItemStack, id: i32, value: i32) {
    let mut writer = Writer::new();
    writer.varint(value);
    set_component(
        stack,
        Component {
            type_id: id,
            data: writer.into_inner(),
        },
    );
}

pub(crate) fn repair<R: SpecialRecipeRegistry>(
    book: &RecipeBook,
    input: &RecipeInput,
    context: &SpecialRecipeContext<'_, R>,
) -> Result<Option<CraftResult>> {
    context.validate_book(book)?;
    let RecipeInput::Crafting {
        width,
        height,
        items,
    } = input
    else {
        return Ok(None);
    };
    ensure!(
        *width <= 3 && *height <= 3 && width.checked_mul(*height) == Some(items.len()),
        "invalid crafting grid dimensions"
    );
    context.validate_items(items)?;
    let occupied = items
        .iter()
        .filter(|stack| !stack.is_empty())
        .collect::<Vec<_>>();
    if occupied.len() != 2 {
        return Ok(None);
    }
    let (a, b) = (occupied[0], occupied[1]);
    if a.item_id != b.item_id
        || a.count != 1
        || b.count != 1
        || a.effective_component_bytes(2).is_none()
        || b.effective_component_bytes(2).is_none()
        || a.effective_component_bytes(3).is_none()
        || b.effective_component_bytes(3).is_none()
    {
        return Ok(None);
    }
    let (max_a, max_b) = (int_component(a, 2, 0)?, int_component(b, 2, 0)?);
    ensure!(
        max_a > 0 && max_b > 0,
        "actual maximum-damage component must be positive"
    );
    let maximum = max_a.max(max_b);
    let damage_a = int_component(a, 3, 0)?.clamp(0, max_a);
    let damage_b = int_component(b, 3, 0)?.clamp(0, max_b);
    let durability = max_a
        .wrapping_sub(damage_a)
        .wrapping_add(max_b.wrapping_sub(damage_b))
        .wrapping_add(maximum.wrapping_mul(5) / 100);
    // Source constructs a fresh item; unrelated input patches are not copied.
    let mut output = ItemStack::new(a.item_id, 1)?;
    set_int(&mut output, 2, maximum);
    set_int(
        &mut output,
        3,
        maximum.wrapping_sub(durability).max(0).clamp(0, maximum),
    );
    let component = CraftingEnchantments::for_item(book, &output);
    if output
        .effective_component_bytes(component.component_id())
        .is_some()
    {
        let mut values = vec![];
        let mut seen = vec![];
        for holder in context.registry.enchantments()? {
            ensure!(
                !seen.contains(&holder),
                "actual enchantment registry repeated a holder"
            );
            seen.push(holder.clone());
            if context.registry.is_curse(&holder)? {
                let level = context
                    .registry
                    .enchantment_level(a, component, &holder)?
                    .max(context.registry.enchantment_level(b, component, &holder)?);
                if level > 0 {
                    values.push((holder, level.min(255)));
                }
            }
        }
        let encoded = context
            .registry
            .encode_enchantments(component, &values, true)?;
        ensure!(
            encoded.type_id == component.component_id(),
            "actual repair encoder returned wrong enchantment component"
        );
        set_component(&mut output, encoded);
    }
    output.encode_canonical()?;
    let remaining = context.remaining(items)?;
    context.validate_snapshot()?;
    Ok(Some(CraftResult {
        stack: output,
        remaining,
    }))
}

pub(crate) fn trim<R: SpecialRecipeRegistry>(
    book: &RecipeBook,
    template: &ItemStack,
    base: &ItemStack,
    addition: &ItemStack,
    matched: bool,
    context: &SpecialRecipeContext<'_, R>,
) -> Result<Option<CraftResult>> {
    context.validate_book(book)?;
    context.validate_items(&[template.clone(), base.clone(), addition.clone()])?;
    if !matched {
        return Ok(None);
    }
    let mut material = None;
    for holder in context.registry.trim_materials()? {
        if context.registry.material_matches(&holder, addition)? {
            material = Some(holder);
            break;
        }
    }
    // Source performs both lookups before testing either Optional.
    let mut pattern = None;
    for holder in context.registry.trim_patterns()? {
        if context.registry.pattern_matches(&holder, template)? {
            pattern = Some(holder);
            break;
        }
    }
    let mut output = ItemStack::empty();
    if let (Some(material), Some(pattern)) = (material, pattern) {
        if !context
            .registry
            .has_pattern_and_material(base, &pattern, &material)?
        {
            output = base.clone();
            output.count = 1;
            let encoded = context.registry.encode_trim(&material, &pattern, true)?;
            ensure!(
                encoded.type_id == 35,
                "actual trim encoder returned wrong component"
            );
            set_component(&mut output, encoded);
        }
    }
    output.encode_canonical()?;
    let remaining = context.remaining(&[template.clone(), base.clone(), addition.clone()])?;
    context.validate_snapshot()?;
    Ok(Some(CraftResult {
        stack: output,
        remaining,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Holder {
        key: u64,
        wire_id: i32,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TrimHolder {
        key: u64,
        wire_id: i32,
        ingredient: i32,
    }
    /// Explicit isolated native registry fixture. Its IDs/tag assignments and
    /// encounter order are test-owned, not a production Level or Java execution.
    struct Registry {
        revision: Cell<u64>,
        enchantments: Vec<Holder>,
        curses: Vec<u64>,
        materials: Vec<TrimHolder>,
        patterns: Vec<TrimHolder>,
        calls: RefCell<Vec<String>>,
        fail_curse: bool,
        change_during_curse: Cell<bool>,
    }
    fn item(name: &str) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), 1).unwrap()
    }
    fn enchant_id(name: &str) -> i32 {
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../../tools/crafting_metadata.json")).unwrap();
        metadata["registries"]["minecraft:enchantment"][format!("minecraft:{name}")]
            .as_i64()
            .unwrap() as i32
    }
    fn levels(stack: &ItemStack, component: i32) -> Vec<(i32, i32)> {
        let Some(value) = stack.component_value(component).unwrap() else {
            return vec![];
        };
        value["enchantments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                (
                    entry["id"].as_i64().unwrap() as i32,
                    entry["level"].as_i64().unwrap() as i32,
                )
            })
            .collect()
    }
    fn encode_enchantments(component: i32, values: &[(i32, i32)], tooltip: bool) -> Component {
        let mut writer = Writer::new();
        writer.varint(values.len() as i32);
        for (id, level) in values {
            writer.varint(*id).varint(*level);
        }
        writer.bool(tooltip);
        Component {
            type_id: component,
            data: writer.into_inner(),
        }
    }
    impl Registry {
        fn new() -> Self {
            let book = RecipeBook::vanilla();
            Self {
                revision: Cell::new(10),
                enchantments: vec![
                    Holder {
                        key: 101,
                        wire_id: enchant_id("binding_curse"),
                    },
                    Holder {
                        key: 102,
                        wire_id: enchant_id("vanishing_curse"),
                    },
                    Holder {
                        key: 103,
                        wire_id: enchant_id("sharpness"),
                    },
                ],
                curses: vec![101, 102],
                materials: vec![
                    TrimHolder {
                        key: 201,
                        wire_id: 8,
                        ingredient: book.item_id("diamond").unwrap(),
                    },
                    TrimHolder {
                        key: 202,
                        wire_id: 2,
                        ingredient: book.item_id("diamond").unwrap(),
                    },
                ],
                patterns: vec![
                    TrimHolder {
                        key: 301,
                        wire_id: 7,
                        ingredient: book.item_id("sentry_armor_trim_smithing_template").unwrap(),
                    },
                    TrimHolder {
                        key: 302,
                        wire_id: 5,
                        ingredient: book.item_id("sentry_armor_trim_smithing_template").unwrap(),
                    },
                ],
                calls: RefCell::new(vec![]),
                fail_curse: false,
                change_during_curse: Cell::new(false),
            }
        }
        fn log(&self, s: impl Into<String>) {
            self.calls.borrow_mut().push(s.into());
        }
    }
    impl SpecialRecipeRegistry for Registry {
        type Snapshot = u64;
        type Enchantment = Holder;
        type Material = TrimHolder;
        type Pattern = TrimHolder;
        fn snapshot(&self) -> u64 {
            self.revision.get()
        }
        fn validate_recipe_book(&self, book: &RecipeBook) -> Result<()> {
            ensure!(
                std::ptr::eq(book, RecipeBook::vanilla()),
                "fixture owns only its captured book"
            );
            Ok(())
        }
        fn validate_item(&self, stack: &ItemStack) -> Result<()> {
            ensure!(
                crate::inventory::item_name(stack.item_id).is_some(),
                "fixture unresolved item"
            );
            Ok(())
        }
        fn enchantments(&self) -> Result<Vec<Holder>> {
            self.log("enchantments");
            Ok(self.enchantments.clone())
        }
        fn is_curse(&self, holder: &Holder) -> Result<bool> {
            ensure!(!self.fail_curse, "actual curse-tag owner unavailable");
            self.log(format!("curse:{}", holder.key));
            if self.change_during_curse.replace(false) {
                self.revision.set(self.revision.get() + 1);
            }
            Ok(self.curses.contains(&holder.key))
        }
        fn enchantment_level(
            &self,
            stack: &ItemStack,
            component: CraftingEnchantments,
            holder: &Holder,
        ) -> Result<i32> {
            self.log(format!("level:{}:{}", component.component_id(), holder.key));
            Ok(levels(stack, component.component_id())
                .into_iter()
                .find(|(id, _)| *id == holder.wire_id)
                .map_or(0, |(_, level)| level))
        }
        fn encode_enchantments(
            &self,
            component: CraftingEnchantments,
            values: &[(Holder, i32)],
            tooltip: bool,
        ) -> Result<Component> {
            self.log("encode_enchantments");
            Ok(encode_enchantments(
                component.component_id(),
                &values
                    .iter()
                    .map(|(h, l)| (h.wire_id, *l))
                    .collect::<Vec<_>>(),
                tooltip,
            ))
        }
        fn trim_materials(&self) -> Result<Vec<TrimHolder>> {
            self.log("materials");
            Ok(self.materials.clone())
        }
        fn material_matches(&self, holder: &TrimHolder, addition: &ItemStack) -> Result<bool> {
            self.log(format!("material:{}", holder.key));
            Ok(holder.ingredient == addition.item_id)
        }
        fn trim_patterns(&self) -> Result<Vec<TrimHolder>> {
            self.log("patterns");
            Ok(self.patterns.clone())
        }
        fn pattern_matches(&self, holder: &TrimHolder, template: &ItemStack) -> Result<bool> {
            self.log(format!("pattern:{}", holder.key));
            Ok(holder.ingredient == template.item_id)
        }
        fn has_pattern_and_material(
            &self,
            base: &ItemStack,
            pattern: &TrimHolder,
            material: &TrimHolder,
        ) -> Result<bool> {
            self.log("equal_trim");
            let Some(bytes) = base.effective_component_bytes(35) else {
                return Ok(false);
            };
            let mut reader = Reader::new(bytes);
            let m = reader.varint()?;
            let p = reader.varint()?;
            ensure!(m > 0 && p > 0, "fixture has no inline holder owner");
            Ok(m == material.wire_id + 1 && p == pattern.wire_id + 1)
        }
        fn encode_trim(
            &self,
            material: &TrimHolder,
            pattern: &TrimHolder,
            tooltip: bool,
        ) -> Result<Component> {
            self.log("encode_trim");
            let mut writer = Writer::new();
            writer
                .varint(material.wire_id + 1)
                .varint(pattern.wire_id + 1)
                .bool(tooltip);
            Ok(Component {
                type_id: 35,
                data: writer.into_inner(),
            })
        }
        fn crafting_remainder(&self, input: &ItemStack) -> Result<ItemStack> {
            self.log(format!("remainder:{}", input.item_id));
            Ok(if input.item_id == item("milk_bucket").item_id {
                item("bucket")
            } else {
                ItemStack::empty()
            })
        }
    }
    fn repair_input(name: &str) -> RecipeInput {
        let mut a = item(name);
        let mut b = item(name);
        for stack in [&mut a, &mut b] {
            set_int(stack, 2, 100);
            set_int(stack, 3, 60);
        }
        RecipeInput::crafting(2, 1, vec![a, b]).unwrap()
    }
    #[test]
    fn repair_selects_stored_component_and_actual_curse_tag_and_preserves_item_remainders(
    ) -> Result<()> {
        let book = RecipeBook::vanilla();
        let mut input = repair_input("enchanted_book");
        let RecipeInput::Crafting { items, .. } = &mut input else {
            unreachable!()
        };
        set_component(
            &mut items[0],
            encode_enchantments(23, &[(enchant_id("vanishing_curse"), 1)], false),
        );
        set_component(
            &mut items[1],
            encode_enchantments(
                23,
                &[
                    (enchant_id("vanishing_curse"), 2),
                    (enchant_id("sharpness"), 5),
                ],
                false,
            ),
        );
        set_component(
            &mut items[0],
            encode_enchantments(9, &[(enchant_id("binding_curse"), 3)], false),
        );
        let before = items.clone();
        // This expected result is derived from czg/dae bytecode, not a Java
        // runtime observation of the modified-component enchanted-book input.
        let legacy = book.evaluate("repair_item", &input)?.unwrap();
        assert_eq!(legacy.stack.damage_value()?, 15);
        assert_eq!(
            levels(&legacy.stack, 23),
            vec![(enchant_id("vanishing_curse"), 2)]
        );
        assert!(levels(&legacy.stack, 9).is_empty());
        let mut registry = Registry::new();
        // Actual fixture tag changes: Sharpness is a curse, Vanishing is not.
        registry.curses = vec![103];
        let context = SpecialRecipeContext::new(&registry);
        let actual = book
            .evaluate_with_special_registry("repair_item", &input, &context)?
            .unwrap();
        assert_eq!(actual.stack.damage_value()?, 15);
        assert_eq!(
            levels(&actual.stack, 23),
            vec![(enchant_id("sharpness"), 5)]
        );
        assert_eq!(
            actual.stack.component_value(23)?.unwrap()["showInTooltip"],
            true
        );
        assert!(levels(&actual.stack, 9).is_empty());
        assert!(registry.calls.borrow().iter().any(|c| c == "level:23:103"));
        let RecipeInput::Crafting { items, .. } = &input else {
            unreachable!()
        };
        assert_eq!(items, &before);

        let buckets = repair_input("milk_bucket");
        let actual = book
            .evaluate_with_special_registry("repair_item", &buckets, &context)?
            .unwrap();
        assert_eq!(actual.remaining, vec![item("bucket"), item("bucket")]);
        assert_eq!(actual.stack.item_id, item("milk_bucket").item_id);
        Ok(())
    }
    #[test]
    fn trim_uses_first_actual_holders_owner_equality_and_preserves_base_patch() -> Result<()> {
        let book = RecipeBook::vanilla();
        let registry = Registry::new();
        let context = SpecialRecipeContext::new(&registry);
        let mut base = item("diamond_chestplate");
        set_int(&mut base, 3, 27);
        set_int(&mut base, 16, 8);
        let template = item("sentry_armor_trim_smithing_template");
        let addition = item("diamond");
        let input = RecipeInput::Smithing {
            template: template.clone(),
            base: base.clone(),
            addition: addition.clone(),
        };
        let result = book
            .evaluate_with_special_registry(
                "sentry_armor_trim_smithing_template_smithing_trim",
                &input,
                &context,
            )?
            .unwrap();
        assert_eq!(result.stack.count, 1);
        assert_eq!(result.stack.damage_value()?, 27);
        assert_eq!(result.stack.component_value(16)?, base.component_value(16)?);
        let mut trim = Reader::new(result.stack.effective_component_bytes(35).unwrap());
        assert_eq!((trim.varint()?, trim.varint()?, trim.bool()?), (9, 8, true));
        trim.finish()?;
        assert!(!registry
            .calls
            .borrow()
            .iter()
            .any(|c| c == "material:202" || c == "pattern:302"));

        let mut same = result.stack;
        let encoded = registry.encode_trim(&registry.materials[0], &registry.patterns[0], false)?;
        set_component(&mut same, encoded);
        let same_input = RecipeInput::Smithing {
            template: template.clone(),
            base: same,
            addition: addition.clone(),
        };
        let matched = book.evaluate_with_special_registry(
            "sentry_armor_trim_smithing_template_smithing_trim",
            &same_input,
            &context,
        )?;
        assert!(matched.is_some_and(|result| result.stack.is_empty())); // tooltip ignored, matches retained.

        let mut missing = Registry::new();
        missing.materials.clear();
        let missing_context = SpecialRecipeContext::new(&missing);
        let matched = book.evaluate_with_special_registry(
            "sentry_armor_trim_smithing_template_smithing_trim",
            &input,
            &missing_context,
        )?;
        assert!(matched.is_some_and(|result| result.stack.is_empty()));
        assert!(missing.calls.borrow().iter().any(|c| c == "patterns")); // both lookups happen.
        Ok(())
    }
    #[test]
    fn registry_absence_snapshot_change_nonmatch_and_other_serializer_never_default() -> Result<()>
    {
        let book = RecipeBook::vanilla();
        let input = repair_input("iron_sword");
        let mut missing = Registry::new();
        missing.fail_curse = true;
        assert!(book
            .evaluate_with_special_registry(
                "repair_item",
                &input,
                &SpecialRecipeContext::new(&missing)
            )
            .is_err());
        let changed = Registry::new();
        changed.change_during_curse.set(true);
        let captured = SpecialRecipeContext::new(&changed);
        assert!(book
            .evaluate_with_special_registry("repair_item", &input, &captured)
            .is_err());
        assert!(captured.validate_snapshot().is_err());

        let registry = Registry::new();
        let context = SpecialRecipeContext::new(&registry);
        let mismatch = RecipeInput::crafting(1, 1, vec![item("iron_sword")])?;
        assert!(book
            .evaluate_with_special_registry("repair_item", &mismatch, &context)?
            .is_none());
        assert!(registry.calls.borrow().is_empty());
        assert!(book
            .evaluate_with_special_registry("oak_planks", &mismatch, &context)
            .is_err());
        let mismatch = RecipeInput::Smithing {
            template: item("paper"),
            base: item("diamond_chestplate"),
            addition: item("diamond"),
        };
        assert!(book
            .evaluate_with_special_registry(
                "sentry_armor_trim_smithing_template_smithing_trim",
                &mismatch,
                &context
            )?
            .is_none());
        assert!(registry.calls.borrow().is_empty());
        Ok(())
    }
}
