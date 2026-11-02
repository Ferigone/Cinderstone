//! Ordered Java 1.21 PotionBrewing over a required immutable actual context.
//!
//! This module supplies neither a potion/item registry nor an effect, Level,
//! feature default or component-owner fallback. Named registrations resolve
//! through the captured real owners. See workstations-potion-brewing.md.
use crate::{inventory::ItemStack, workstations::brewing::PotionBrewingOwner};
use anyhow::{ensure, Result};
use std::fmt::Debug;

/// Read-only access to ONE actual item/potion/features/component snapshot.
/// Cloning its opaque handles must retain the same actual owners. Protocol IDs,
/// names or structurally equal reconstructed registries do not prove identity.
/// All methods must reject an unavailable/changed owner and unsupported
/// component identity. They perform no gameplay callback, RNG, inventory or
/// world mutation. No method has a permissive/default implementation.
pub trait PotionBrewingContext {
    type Snapshot: Clone + Eq + Debug;
    type Item: Clone + Debug;
    type Potion: Clone + Debug;
    type Ingredient: Clone + Debug;

    fn snapshot(&self) -> Result<Self::Snapshot>;
    fn validate_snapshot(&self, snapshot: &Self::Snapshot) -> Result<()>;
    fn resolve_item(&self, key: &str) -> Result<Self::Item>;
    fn resolve_potion(&self, key: &str) -> Result<Self::Potion>;
    fn item_enabled(&self, item: &Self::Item) -> Result<bool>;
    fn potion_enabled(&self, potion: &Self::Potion) -> Result<bool>;
    /// Actual `item instanceof PotionItem`; called only AFTER feature gates.
    fn expect_potion_item(&self, item: &Self::Item) -> Result<()>;
    /// Actual Ingredient.of(single actual ItemLike), retaining its item owner.
    fn ingredient_of_item(&self, item: &Self::Item) -> Result<Self::Ingredient>;
    fn ingredient_matches(&self, ingredient: &Self::Ingredient, stack: &ItemStack) -> Result<bool>;
    /// Exact ItemStack.is(Holder<Item>): getItem().builtInRegistryHolder() ==
    /// expected, including getItem's logical-empty AIR branch. This is actual
    /// holder OBJECT identity, not Holder.is or item-name/protocol-ID equality.
    fn item_matches(&self, expected: &Self::Item, stack: &ItemStack) -> Result<bool>;
    /// Actual ItemStack.getItem binding, including logical-empty AIR. Resolve
    /// this through the real item owner rather than converting a raw ID/name
    /// into a fabricated holder. Potion mixing retains this item only.
    fn stack_item(&self, stack: &ItemStack) -> Result<Self::Item>;
    /// Directional `expected.is(actual)` for Potion holders. Reference and
    /// Direct holders have different behavior; do not use symmetric equality.
    fn potion_is(&self, expected: &Self::Potion, actual: &Self::Potion) -> Result<bool>;
    /// Actual getOrDefault(POTION_CONTENTS, EMPTY).potion(), validating the
    /// typed component/holder against this same snapshot. Absent is NOT water.
    fn stack_potion(&self, stack: &ItemStack) -> Result<Option<Self::Potion>>;
    /// Actual PotionContents.createItemStack: new count1 target ItemStack with
    /// its real prototypes and fresh PotionContents(potion, NO color, EMPTY
    /// custom effects). Copy no old patch, count or other component. Encoding
    /// must use this actual potion/component owner, not a guessed registry ID.
    fn create_potion_stack(&self, item: &Self::Item, potion: &Self::Potion) -> Result<ItemStack>;
    /// The existing owned-value trait cannot return Java's original ItemStack
    /// reference. Certify/preserve every actual nested component reference
    /// owner while making its unchanged value observation, or return Err.
    /// A byte clone, semantic equality or default prototype substitution alone
    /// is NOT this certificate. Preserve exact count/item/patch fields too.
    fn copy_unchanged_preserving_identity(&self, original: &ItemStack) -> Result<ItemStack>;
}

/// Literal source result ownership, before adapting to an owned-value API.
#[derive(Debug)]
pub enum MixResult<'stack> {
    Unchanged(&'stack ItemStack),
    Fresh(ItemStack),
}

struct ContainerMix<C: PotionBrewingContext> {
    from: C::Item,
    ingredient: C::Ingredient,
    to: C::Item,
}
struct PotionMix<C: PotionBrewingContext> {
    from: C::Potion,
    ingredient: C::Ingredient,
    to: C::Potion,
}

/// Source Builder's three independent insertion-ordered lists. Registration
/// failures leave these private lists unchanged for that registration call.
/// This stores only actual context handles, never a replacement registry.
pub struct PotionBrewingBuilder<'context, C: PotionBrewingContext> {
    context: &'context C,
    snapshot: C::Snapshot,
    containers: Vec<C::Ingredient>,
    potion_mixes: Vec<PotionMix<C>>,
    container_mixes: Vec<ContainerMix<C>>,
}

impl<'context, C: PotionBrewingContext> PotionBrewingBuilder<'context, C> {
    pub fn new(context: &'context C) -> Result<Self> {
        let snapshot = context.snapshot()?;
        context.validate_snapshot(&snapshot)?;
        Ok(Self {
            context,
            snapshot,
            containers: vec![],
            potion_mixes: vec![],
            container_mixes: vec![],
        })
    }
    fn validate(&self) -> Result<()> {
        self.context.validate_snapshot(&self.snapshot)
    }

    pub fn add_container(&mut self, item_key: &str) -> Result<()> {
        self.validate()?;
        let item = self.context.resolve_item(item_key)?;
        if !self.context.item_enabled(&item)? {
            return self.validate();
        }
        self.context.expect_potion_item(&item)?;
        let ingredient = self.context.ingredient_of_item(&item)?;
        self.validate()?;
        self.containers.push(ingredient);
        Ok(())
    }

    pub fn add_container_recipe(
        &mut self,
        from_key: &str,
        ingredient_key: &str,
        to_key: &str,
    ) -> Result<()> {
        self.validate()?;
        let from = self.context.resolve_item(from_key)?;
        let ingredient = self.context.resolve_item(ingredient_key)?;
        let to = self.context.resolve_item(to_key)?;
        if !self.context.item_enabled(&from)?
            || !self.context.item_enabled(&ingredient)?
            || !self.context.item_enabled(&to)?
        {
            return self.validate();
        }
        self.context.expect_potion_item(&from)?;
        self.context.expect_potion_item(&to)?;
        let ingredient = self.context.ingredient_of_item(&ingredient)?;
        self.validate()?;
        self.container_mixes.push(ContainerMix {
            from,
            ingredient,
            to,
        });
        Ok(())
    }

    fn prepare_mix(
        &self,
        from: C::Potion,
        ingredient: &C::Item,
        to: C::Potion,
    ) -> Result<Option<PotionMix<C>>> {
        if !self.context.potion_enabled(&from)?
            || !self.context.item_enabled(ingredient)?
            || !self.context.potion_enabled(&to)?
        {
            return Ok(None);
        }
        let ingredient = self.context.ingredient_of_item(ingredient)?;
        Ok(Some(PotionMix {
            from,
            ingredient,
            to,
        }))
    }
    pub fn add_mix(&mut self, from_key: &str, ingredient_key: &str, to_key: &str) -> Result<()> {
        self.validate()?;
        let from = self.context.resolve_potion(from_key)?;
        let ingredient = self.context.resolve_item(ingredient_key)?;
        let to = self.context.resolve_potion(to_key)?;
        let mix = self.prepare_mix(from, &ingredient, to)?;
        self.validate()?;
        if let Some(mix) = mix {
            self.potion_mixes.push(mix);
        }
        Ok(())
    }

    /// The target gate precedes BOTH generated addMix calls. Flattening this
    /// into unconditional expanded registrations changes feature filtering.
    pub fn add_start_mix(&mut self, ingredient_key: &str, to_key: &str) -> Result<()> {
        self.validate()?;
        let ingredient = self.context.resolve_item(ingredient_key)?;
        let to = self.context.resolve_potion(to_key)?;
        if !self.context.potion_enabled(&to)? {
            return self.validate();
        }
        let water = self.context.resolve_potion("minecraft:water")?;
        let mundane = self.context.resolve_potion("minecraft:mundane")?;
        let first = self.prepare_mix(water, &ingredient, mundane)?;
        let awkward = self.context.resolve_potion("minecraft:awkward")?;
        let second = self.prepare_mix(awkward, &ingredient, to)?;
        self.validate()?;
        self.potion_mixes.extend(first);
        self.potion_mixes.extend(second);
        Ok(())
    }
    pub fn build(self) -> Result<PotionBrewing<'context, C>> {
        self.validate()?;
        Ok(PotionBrewing {
            context: self.context,
            snapshot: self.snapshot,
            containers: self.containers,
            potion_mixes: self.potion_mixes,
            container_mixes: self.container_mixes,
        })
    }
}

/// A concrete source algorithm over borrowed actual immutable owners.
/// Replacing the Level's owner/context requires a newly bound view; callbacks,
/// ingredient consumption/remainders and world effects remain in their owners.
pub struct PotionBrewing<'context, C: PotionBrewingContext> {
    context: &'context C,
    snapshot: C::Snapshot,
    containers: Vec<C::Ingredient>,
    potion_mixes: Vec<PotionMix<C>>,
    container_mixes: Vec<ContainerMix<C>>,
}
impl<C: PotionBrewingContext> PotionBrewing<'_, C> {
    pub fn snapshot_key(&self) -> &C::Snapshot {
        &self.snapshot
    }
    /// Inspection only: [containers, container mixes, potion mixes].
    pub fn registration_counts(&self) -> [usize; 3] {
        [
            self.containers.len(),
            self.container_mixes.len(),
            self.potion_mixes.len(),
        ]
    }
    fn validate(&self) -> Result<()> {
        self.context.validate_snapshot(&self.snapshot)
    }
    fn is_container_inner(&self, stack: &ItemStack) -> Result<bool> {
        for ingredient in &self.containers {
            if self.context.ingredient_matches(ingredient, stack)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn is_container(&self, stack: &ItemStack) -> Result<bool> {
        self.validate()?;
        let result = self.is_container_inner(stack)?;
        self.validate()?;
        Ok(result)
    }
    fn is_container_ingredient_inner(&self, stack: &ItemStack) -> Result<bool> {
        for mix in &self.container_mixes {
            if self.context.ingredient_matches(&mix.ingredient, stack)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn is_container_ingredient(&self, stack: &ItemStack) -> Result<bool> {
        self.validate()?;
        let result = self.is_container_ingredient_inner(stack)?;
        self.validate()?;
        Ok(result)
    }
    fn is_potion_ingredient_inner(&self, stack: &ItemStack) -> Result<bool> {
        for mix in &self.potion_mixes {
            if self.context.ingredient_matches(&mix.ingredient, stack)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn is_potion_ingredient(&self, stack: &ItemStack) -> Result<bool> {
        self.validate()?;
        let result = self.is_potion_ingredient_inner(stack)?;
        self.validate()?;
        Ok(result)
    }
    pub fn is_brewable_potion(&self, potion: &C::Potion) -> Result<bool> {
        self.validate()?;
        let mut result = false;
        for mix in &self.potion_mixes {
            if self.context.potion_is(&mix.to, potion)? {
                result = true;
                break;
            }
        }
        self.validate()?;
        Ok(result)
    }
    fn has_container_mix_inner(&self, bottle: &ItemStack, ingredient: &ItemStack) -> Result<bool> {
        for mix in &self.container_mixes {
            if self.context.item_matches(&mix.from, bottle)?
                && self
                    .context
                    .ingredient_matches(&mix.ingredient, ingredient)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn has_container_mix(&self, bottle: &ItemStack, ingredient: &ItemStack) -> Result<bool> {
        self.validate()?;
        let result = self.has_container_mix_inner(bottle, ingredient)?;
        self.validate()?;
        Ok(result)
    }
    fn has_potion_mix_inner(&self, bottle: &ItemStack, ingredient: &ItemStack) -> Result<bool> {
        let Some(potion) = self.context.stack_potion(bottle)? else {
            return Ok(false);
        };
        for mix in &self.potion_mixes {
            if self.context.potion_is(&mix.from, &potion)?
                && self
                    .context
                    .ingredient_matches(&mix.ingredient, ingredient)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn has_potion_mix(&self, bottle: &ItemStack, ingredient: &ItemStack) -> Result<bool> {
        self.validate()?;
        let result = self.has_potion_mix_inner(bottle, ingredient)?;
        self.validate()?;
        Ok(result)
    }

    /// Unlike has_mix, Java mix has NO registered-container guard. A valid
    /// batch can therefore transform a raw non-container stack in another slot.
    pub fn mix_result<'stack>(
        &self,
        ingredient: &ItemStack,
        bottle: &'stack ItemStack,
    ) -> Result<MixResult<'stack>> {
        self.validate()?;
        let result = self.mix_inner(ingredient, bottle)?;
        self.validate()?;
        Ok(result)
    }
    fn mix_inner<'stack>(
        &self,
        ingredient: &ItemStack,
        bottle: &'stack ItemStack,
    ) -> Result<MixResult<'stack>> {
        if bottle.is_empty() {
            return Ok(MixResult::Unchanged(bottle));
        }
        let Some(potion) = self.context.stack_potion(bottle)? else {
            return Ok(MixResult::Unchanged(bottle));
        };
        for mix in &self.container_mixes {
            if self.context.item_matches(&mix.from, bottle)?
                && self
                    .context
                    .ingredient_matches(&mix.ingredient, ingredient)?
            {
                return self.fresh(&mix.to, &potion);
            }
        }
        for mix in &self.potion_mixes {
            if self.context.potion_is(&mix.from, &potion)?
                && self
                    .context
                    .ingredient_matches(&mix.ingredient, ingredient)?
            {
                // Source retains only the actual input ITEM, not its patch.
                // Resolve it through the real item owner via this mandatory
                // construction operation, without guessing an item holder.
                return self.fresh_for_input_item(bottle, &mix.to);
            }
        }
        Ok(MixResult::Unchanged(bottle))
    }
    fn fresh<'stack>(&self, item: &C::Item, potion: &C::Potion) -> Result<MixResult<'stack>> {
        let stack = self.context.create_potion_stack(item, potion)?;
        ensure!(
            !stack.is_empty() && stack.count == 1,
            "actual potion construction did not return source count1"
        );
        Ok(MixResult::Fresh(stack))
    }
    fn fresh_for_input_item<'stack>(
        &self,
        bottle: &ItemStack,
        potion: &C::Potion,
    ) -> Result<MixResult<'stack>> {
        let item = self.context.stack_item(bottle)?;
        self.fresh(&item, potion)
    }
}

impl<C: PotionBrewingContext> PotionBrewingOwner for PotionBrewing<'_, C> {
    fn is_ingredient(&self, ingredient: &ItemStack) -> Result<bool> {
        self.validate()?;
        let result = self.is_container_ingredient_inner(ingredient)?
            || self.is_potion_ingredient_inner(ingredient)?;
        self.validate()?;
        Ok(result)
    }
    fn has_mix(&self, bottle: &ItemStack, ingredient: &ItemStack) -> Result<bool> {
        self.validate()?;
        let result = self.is_container_inner(bottle)?
            && (self.has_container_mix_inner(bottle, ingredient)?
                || self.has_potion_mix_inner(bottle, ingredient)?);
        self.validate()?;
        Ok(result)
    }
    fn mix(&self, ingredient: &ItemStack, bottle: &ItemStack) -> Result<ItemStack> {
        match self.mix_result(ingredient, bottle)? {
            MixResult::Fresh(stack) => Ok(stack),
            MixResult::Unchanged(original) => {
                let copy = self.context.copy_unchanged_preserving_identity(original)?;
                self.validate()?;
                // This data-preservation guard is NOT the identity proof;
                // that comes only from the required real context above.
                ensure!(
                    &copy == original,
                    "unchanged potion observation altered item/count/patch"
                );
                Ok(copy)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        inventory::{self, Component},
        protocol::{Reader, Writer},
    };
    use anyhow::{bail, Context};
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
        sync::Arc,
    };

    // Controlled, explicitly nonproduction canonical owners. Wire IDs are
    // codec bindings INTO these objects, never the holder equality operation.
    // This is not a builtin registry/default feature/provider implementation.
    #[derive(Debug)]
    struct Item {
        key: &'static str,
        wire: i32,
        potion_class: bool,
        enabled: bool,
    }
    #[derive(Debug)]
    struct Potion {
        key: &'static str,
        wire: i32,
        enabled: bool,
    }
    #[derive(Clone, Debug)]
    enum Holder {
        Reference(Arc<Potion>),
        Direct(Arc<Potion>),
    }
    impl Holder {
        fn value(&self) -> &Arc<Potion> {
            match self {
                Self::Reference(p) | Self::Direct(p) => p,
            }
        }
    }
    #[derive(Clone, Debug)]
    struct Owners {
        items: BTreeMap<&'static str, Arc<Item>>,
        potions: BTreeMap<&'static str, Arc<Potion>>,
    }
    #[derive(Clone, Debug)]
    struct Snapshot(Arc<Owners>);
    impl PartialEq for Snapshot {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for Snapshot {}
    struct ControlledContext {
        actual: RefCell<Snapshot>,
        trace: RefCell<Vec<String>>,
        copy_available: Cell<bool>,
    }
    fn item_id(key: &str) -> i32 {
        let name = key.strip_prefix("minecraft:").unwrap();
        (0..2000)
            .find(|&id| inventory::item_name(id) == Some(name))
            .unwrap()
    }
    impl ControlledContext {
        fn new(
            disabled_item: Option<&str>,
            disabled_potion: Option<&str>,
            copy_available: bool,
        ) -> Self {
            let items = [
                "minecraft:air",
                "minecraft:potion",
                "minecraft:splash_potion",
                "minecraft:lingering_potion",
                "minecraft:stone",
                "minecraft:gunpowder",
                "minecraft:dragon_breath",
                "minecraft:sugar",
            ]
            .into_iter()
            .map(|key| {
                (
                    key,
                    Arc::new(Item {
                        key,
                        wire: item_id(key),
                        potion_class: matches!(
                            key,
                            "minecraft:potion"
                                | "minecraft:splash_potion"
                                | "minecraft:lingering_potion"
                        ),
                        enabled: disabled_item != Some(key),
                    }),
                )
            })
            .collect();
            let potions = [
                ("minecraft:water", 0),
                ("minecraft:mundane", 1),
                ("minecraft:awkward", 3),
                ("minecraft:night_vision", 4),
            ]
            .into_iter()
            .map(|(key, wire)| {
                (
                    key,
                    Arc::new(Potion {
                        key,
                        wire,
                        enabled: disabled_potion != Some(key),
                    }),
                )
            })
            .collect();
            Self {
                actual: RefCell::new(Snapshot(Arc::new(Owners { items, potions }))),
                trace: RefCell::new(vec![]),
                copy_available: Cell::new(copy_available),
            }
        }
        fn stack(&self, key: &str, potion: Option<&str>) -> Result<ItemStack> {
            let mut stack = ItemStack::new(item_id(key), 1)?;
            if let Some(potion) = potion {
                let holder = self.resolve_potion(potion)?;
                let mut fields = Writer::new();
                fields
                    .bool(true)
                    .varint(holder.value().wire)
                    .bool(false)
                    .varint(0);
                stack.components.push(Component {
                    type_id: 31,
                    data: fields.into_inner(),
                });
            }
            Ok(stack)
        }
        fn validate_item(&self, item: &Arc<Item>) -> Result<()> {
            ensure!(
                self.actual
                    .borrow()
                    .0
                    .items
                    .get(item.key)
                    .is_some_and(|actual| Arc::ptr_eq(actual, item)),
                "foreign test item owner"
            );
            Ok(())
        }
        fn validate_potion(&self, holder: &Holder) -> Result<()> {
            let value = holder.value();
            ensure!(
                self.actual
                    .borrow()
                    .0
                    .potions
                    .get(value.key)
                    .is_some_and(|actual| Arc::ptr_eq(actual, value)),
                "foreign test potion owner"
            );
            Ok(())
        }
    }
    impl PotionBrewingContext for ControlledContext {
        type Snapshot = Snapshot;
        type Item = Arc<Item>;
        type Potion = Holder;
        type Ingredient = Arc<Item>;
        fn snapshot(&self) -> Result<Snapshot> {
            Ok(self.actual.borrow().clone())
        }
        fn validate_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
            ensure!(
                *snapshot == *self.actual.borrow(),
                "actual test snapshot replaced"
            );
            Ok(())
        }
        fn resolve_item(&self, key: &str) -> Result<Arc<Item>> {
            self.actual
                .borrow()
                .0
                .items
                .get(key)
                .cloned()
                .context("test item absent")
        }
        fn resolve_potion(&self, key: &str) -> Result<Holder> {
            self.actual
                .borrow()
                .0
                .potions
                .get(key)
                .cloned()
                .map(Holder::Reference)
                .context("test potion absent")
        }
        fn item_enabled(&self, item: &Arc<Item>) -> Result<bool> {
            self.validate_item(item)?;
            self.trace.borrow_mut().push(format!("item:{}", item.key));
            Ok(item.enabled)
        }
        fn potion_enabled(&self, potion: &Holder) -> Result<bool> {
            self.validate_potion(potion)?;
            self.trace
                .borrow_mut()
                .push(format!("potion:{}", potion.value().key));
            Ok(potion.value().enabled)
        }
        fn expect_potion_item(&self, item: &Arc<Item>) -> Result<()> {
            self.validate_item(item)?;
            self.trace.borrow_mut().push(format!("class:{}", item.key));
            ensure!(item.potion_class, "actual test item is not PotionItem");
            Ok(())
        }
        fn ingredient_of_item(&self, item: &Arc<Item>) -> Result<Arc<Item>> {
            self.validate_item(item)?;
            Ok(item.clone())
        }
        fn ingredient_matches(&self, ingredient: &Arc<Item>, stack: &ItemStack) -> Result<bool> {
            self.validate_item(ingredient)?;
            self.trace
                .borrow_mut()
                .push(format!("ingredient:{}", ingredient.key));
            Ok(Arc::ptr_eq(ingredient, &self.stack_item(stack)?))
        }
        fn item_matches(&self, expected: &Arc<Item>, stack: &ItemStack) -> Result<bool> {
            self.validate_item(expected)?;
            Ok(Arc::ptr_eq(expected, &self.stack_item(stack)?))
        }
        fn stack_item(&self, stack: &ItemStack) -> Result<Arc<Item>> {
            let id = if stack.is_empty() { 0 } else { stack.item_id };
            self.actual
                .borrow()
                .0
                .items
                .values()
                .find(|item| item.wire == id)
                .cloned()
                .context("unbound test item codec ID")
        }
        fn potion_is(&self, expected: &Holder, actual: &Holder) -> Result<bool> {
            self.validate_potion(expected)?;
            self.validate_potion(actual)?;
            Ok(match (expected, actual) {
                (Holder::Reference(_), Holder::Direct(_)) => false,
                _ => Arc::ptr_eq(expected.value(), actual.value()),
            })
        }
        fn stack_potion(&self, stack: &ItemStack) -> Result<Option<Holder>> {
            let Some(value) = stack.component_value(31)? else {
                return Ok(None);
            };
            let Some(id) = value["potionId"].as_i64() else {
                return Ok(None);
            };
            self.actual
                .borrow()
                .0
                .potions
                .values()
                .find(|potion| i64::from(potion.wire) == id)
                .cloned()
                .map(Holder::Reference)
                .map(Some)
                .context("unbound test potion codec ID")
        }
        fn create_potion_stack(&self, item: &Arc<Item>, potion: &Holder) -> Result<ItemStack> {
            self.validate_item(item)?;
            self.validate_potion(potion)?;
            self.stack(item.key, Some(potion.value().key))
        }
        fn copy_unchanged_preserving_identity(&self, original: &ItemStack) -> Result<ItemStack> {
            ensure!(
                self.copy_available.get(),
                "actual unchanged component reference owner unavailable"
            );
            // This controlled certificate admits only prototypes and its bound
            // primitive fields; unsupported FOOD references still fail.
            crate::workstations::menu::require_component_context(original)?;
            Ok(original.clone())
        }
    }

    #[test]
    fn source_container_gate_fresh_output_and_unchanged_alias_are_distinct() -> Result<()> {
        let context = ControlledContext::new(None, None, true);
        let mut builder = PotionBrewingBuilder::new(&context)?;
        builder.add_container("minecraft:potion")?;
        builder.add_container_recipe(
            "minecraft:potion",
            "minecraft:gunpowder",
            "minecraft:splash_potion",
        )?;
        builder.add_mix("minecraft:water", "minecraft:sugar", "minecraft:awkward")?;
        builder.add_mix(
            "minecraft:water",
            "minecraft:sugar",
            "minecraft:night_vision",
        )?;
        let brewing = builder.build()?;
        let gunpowder = context.stack("minecraft:gunpowder", None)?;
        let sugar = context.stack("minecraft:sugar", None)?;
        let no_base = context.stack("minecraft:potion", None)?;
        assert!(brewing.has_mix(&no_base, &gunpowder)?);
        assert!(
            matches!(brewing.mix_result(&gunpowder, &no_base)?, MixResult::Unchanged(original) if std::ptr::eq(original, &no_base))
        );
        let mut bottle = context.stack("minecraft:potion", Some("minecraft:water"))?;
        bottle.count = 17;
        let mut contents = Writer::new();
        contents
            .bool(true)
            .varint(0)
            .bool(true)
            .i32(0x123456)
            .varint(1)
            .varint(0)
            .varint(0)
            .varint(100)
            .bool(false)
            .bool(true)
            .bool(true)
            .bool(false);
        bottle
            .components
            .retain(|component| component.type_id != 31);
        bottle.components.push(Component {
            type_id: 31,
            data: contents.into_inner(),
        });
        let mut cost = Writer::new();
        cost.varint(12);
        bottle.components.push(Component {
            type_id: 16,
            data: cost.into_inner(),
        });
        bottle.encode_canonical()?;
        let MixResult::Fresh(output) = brewing.mix_result(&gunpowder, &bottle)? else {
            bail!("expected actual fresh container mix");
        };
        assert_eq!(output.item_id, item_id("minecraft:splash_potion"));
        assert_eq!(output.count, 1);
        assert_eq!(
            output.component_value(31)?.unwrap()["customEffects"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert!(output.component_value(31)?.unwrap()["customColor"].is_null());
        assert_eq!(
            output.component_value(16)?,
            ItemStack::new(output.item_id, 1)?.component_value(16)?
        );
        let raw = context.stack("minecraft:stone", Some("minecraft:water"))?;
        assert!(!brewing.has_mix(&raw, &sugar)?);
        let MixResult::Fresh(output) = brewing.mix_result(&sugar, &raw)? else {
            bail!("mix must not repeat the hasMix container gate");
        };
        assert_eq!(output.item_id, raw.item_id);
        let actual = context.stack_potion(&output)?.unwrap();
        assert!(context.potion_is(&context.resolve_potion("minecraft:awkward")?, &actual)?);
        Ok(())
    }

    #[test]
    fn feature_shortcircuit_start_gate_and_directional_holders_follow_source() -> Result<()> {
        let context = ControlledContext::new(
            Some("minecraft:stone"),
            Some("minecraft:night_vision"),
            true,
        );
        let mut builder = PotionBrewingBuilder::new(&context)?;
        builder.add_container_recipe("minecraft:stone", "minecraft:sugar", "minecraft:potion")?;
        assert_eq!(context.trace.borrow().as_slice(), ["item:minecraft:stone"]);
        context.trace.borrow_mut().clear();
        builder.add_start_mix("minecraft:sugar", "minecraft:night_vision")?;
        assert_eq!(
            context.trace.borrow().as_slice(),
            ["potion:minecraft:night_vision"]
        );
        builder.add_mix("minecraft:water", "minecraft:sugar", "minecraft:awkward")?;
        let brewing = builder.build()?;
        assert_eq!(brewing.registration_counts(), [0, 0, 1]);
        let reference = context.resolve_potion("minecraft:awkward")?;
        assert!(brewing.is_brewable_potion(&reference)?);
        assert!(!brewing.is_brewable_potion(&Holder::Direct(reference.value().clone()))?);
        assert!(context.potion_is(&Holder::Direct(reference.value().clone()), &reference)?);
        Ok(())
    }

    #[test]
    fn value_adapter_requires_identity_certificate_and_replaced_owner_rejects() -> Result<()> {
        let context = ControlledContext::new(None, None, false);
        let brewing = PotionBrewingBuilder::new(&context)?.build()?;
        let bottle = context.stack("minecraft:stone", None)?;
        let ingredient = context.stack("minecraft:sugar", None)?;
        assert!(
            matches!(brewing.mix_result(&ingredient, &bottle)?, MixResult::Unchanged(original) if std::ptr::eq(original, &bottle))
        );
        assert!(PotionBrewingOwner::mix(&brewing, &ingredient, &bottle).is_err());
        context.copy_available.set(true);
        assert_eq!(
            PotionBrewingOwner::mix(&brewing, &ingredient, &bottle)?,
            bottle
        );
        let same_entries = (*context.actual.borrow().0).clone();
        *context.actual.borrow_mut() = Snapshot(Arc::new(same_entries));
        assert!(brewing.has_mix(&bottle, &ingredient).is_err());
        assert!(brewing.mix_result(&ingredient, &bottle).is_err());
        // There is no default-water substitution for an unknown codec ID.
        let mut fields = Writer::new();
        fields.bool(true).varint(999).bool(false).varint(0);
        let mut unknown = bottle;
        unknown.components.push(Component {
            type_id: 31,
            data: fields.into_inner(),
        });
        assert!(context.stack_potion(&unknown).is_err());
        let mut reader = Reader::new(unknown.effective_component_bytes(31).unwrap());
        assert!(reader.bool()?);
        assert_eq!(reader.varint()?, 999);
        Ok(())
    }
}
