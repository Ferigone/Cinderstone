//! Java 1.21 stew, decorated-pot and armor-dye recipes over actual item owners.
//!
//! Primary evidence is direct JDK21 javap of pinned czx19–63, dob22–28,
//! cys18–47, cyi20–72, cxk48–101 and dqx.createDecoratedPotItem. The inner
//! server SHA256 is 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
//! This module supplies no registry, tags, Item/Block definitions or Main host.
use crate::{
    crafting::{CraftResult, RecipeBook, RecipeInput},
    inventory::{Component, ItemStack},
    protocol::Writer,
};
use anyhow::{ensure, Context, Result};
use std::fmt::Debug;

/// Source bootstrap constants, resolved by the actual item owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceItem {
    Air,
    BrownMushroom,
    RedMushroom,
    Bowl,
    SuspiciousStew,
    DecoratedPot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialItemTag {
    SmallFlowers,
    DecoratedPotIngredients,
    Dyeable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialOutput {
    SuspiciousStew,
    DecoratedPot,
}

/// The primitive fields of the actual typed DYED_COLOR component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DyedItemColor {
    pub rgb: i32,
    pub show_in_tooltip: bool,
}

/// Read-only, nonreentrant access to ONE actual item/block/tag/component owner.
///
/// Snapshot equality must certify actual owner identity AND generation, not
/// equal serialized contents. Cloned handles retain their actual objects.
/// Every unavailable, changed or unsupported owner must error. No method may
/// mutate inventory/world/actors, consume RNG or run gameplay callbacks.
///
/// The ItemStack/RecipeBook wire and prototype codec domain remains unchanged.
/// Binding names/IDs to reconstructed objects does not establish actual holder
/// or component identity. This interface is not a second registry service.
pub trait SpecialItemRecipeOwner {
    type Snapshot: Clone + Eq + Debug;
    type Item: Clone + Debug;
    type DyeItem: Clone + Debug;
    type StewEffects: Clone + Debug;

    fn snapshot(&self) -> Result<Self::Snapshot>;
    fn validate_snapshot(&self, snapshot: &Self::Snapshot) -> Result<()>;
    fn validate_recipe_book(&self, book: &RecipeBook) -> Result<()>;
    fn validate_stack(&self, stack: &ItemStack) -> Result<()>;

    /// Actual ItemStack.is(Item), including logical-empty getItem() == AIR.
    fn is_item(&self, stack: &ItemStack, item: SourceItem) -> Result<bool>;
    /// Actual source ItemTags assignment, not a compiled tag/name fallback.
    fn is_in_tag(&self, stack: &ItemStack, tag: SpecialItemTag) -> Result<bool>;
    fn stack_item(&self, stack: &ItemStack) -> Result<Self::Item>;
    /// Actual getItem() instanceof DyeItem and its retained canonical object.
    fn as_dye_item(&self, stack: &ItemStack) -> Result<Option<Self::DyeItem>>;
    fn dye_texture_diffuse_argb(&self, dye: &Self::DyeItem) -> Result<u32>;
    /// SuspiciousEffectHolder.tryGet: first a BlockItem's actual Block, then
    /// the actual Item itself. None certifies neither implements the interface.
    fn suspicious_effects(&self, item: &Self::Item) -> Result<Option<Self::StewEffects>>;
    /// Encode that SAME actual typed SuspiciousStewEffects (component32),
    /// retaining its canonical effect holders and encounter order.
    fn encode_suspicious_effects(&self, effects: &Self::StewEffects) -> Result<Component>;
    /// Stew: actual new ItemStack(SUSPICIOUS_STEW,1). Pot: actual
    /// DECORATED_POT.getDefaultInstance(), with its actual item prototype.
    fn fresh_output(&self, output: SpecialOutput) -> Result<ItemStack>;
    /// Actual PotDecorations(back,left,right,front) and its component51 codec.
    /// Each input is its actual getItem(), not its patch/count or a guessed ID.
    fn encode_pot_decorations(&self, items: &[Self::Item; 4]) -> Result<Component>;
    /// Actual ItemStack.copy(), preserving count and every actual component
    /// reference/auxiliary copy field. Certify the source copy or return Err.
    /// Byte cloning/semantic equality alone is NOT that certificate.
    fn copy_stack(&self, original: &ItemStack) -> Result<ItemStack>;
    /// Actual copyWithCount(1), with the SAME source identity requirements.
    fn copy_with_count_one(&self, original: &ItemStack) -> Result<ItemStack>;
    /// Read the actual typed DYED_COLOR; absent is distinct from black.
    fn dyed_color(&self, stack: &ItemStack) -> Result<Option<DyedItemColor>>;
    /// Actual Item craftingRemainingItem property and fresh default count1
    /// stack. Food usingConvertsTo is not this operation.
    fn crafting_remainder(&self, input: &ItemStack) -> Result<ItemStack>;
}

pub struct SpecialItemRecipeContext<'a, O: SpecialItemRecipeOwner> {
    owner: &'a O,
    snapshot: O::Snapshot,
}

impl<'a, O: SpecialItemRecipeOwner> SpecialItemRecipeContext<'a, O> {
    pub fn new(owner: &'a O) -> Result<Self> {
        let snapshot = owner.snapshot()?;
        owner.validate_snapshot(&snapshot)?;
        Ok(Self { owner, snapshot })
    }
    pub fn snapshot(&self) -> &O::Snapshot {
        &self.snapshot
    }
    pub fn validate_snapshot(&self) -> Result<()> {
        self.owner.validate_snapshot(&self.snapshot)
    }
    fn validate_stack(&self, stack: &ItemStack) -> Result<()> {
        // Explicit wire-only FOOD values do not establish nested reference
        // identity. Do not strip them or replace them with prototypes.
        crate::workstations::menu::require_component_context(stack)?;
        stack.encode_canonical()?;
        self.owner.validate_stack(stack)
    }

    /// Evaluate ONLY suspicious stew, decorated pot or armor dye. Existing
    /// default evaluators and registry-aware repair/trim remain separate.
    ///
    /// `remaining` is a read-only recipe observation, NOT a take/commit cache.
    /// The actual ResultSlot must query its current recipe manager remainder
    /// path AFTER its source crafted/recipe callbacks. This method neither
    /// consumes inputs nor makes a preview into a second owned result.
    pub fn evaluate(
        &self,
        book: &RecipeBook,
        id: &str,
        input: &RecipeInput,
    ) -> Result<Option<CraftResult>> {
        self.validate_snapshot()?;
        self.owner.validate_recipe_book(book)?;
        let kind = book
            .recipe_type(id)
            .context("unknown special-item recipe")?;
        ensure!(
            matches!(
                kind,
                "crafting_special_suspiciousstew"
                    | "crafting_decorated_pot"
                    | "crafting_special_armordye"
            ),
            "actual special-item context does not support serializer {kind}"
        );
        let RecipeInput::Crafting {
            width,
            height,
            items,
        } = input
        else {
            self.validate_snapshot()?;
            return Ok(None);
        };
        ensure!(
            *width <= i32::MAX as usize
                && *height <= i32::MAX as usize
                && width.checked_mul(*height) == Some(items.len())
                && items.len() <= i32::MAX as usize,
            "invalid source crafting input dimensions"
        );
        for stack in items {
            self.validate_stack(stack)?;
        }
        self.validate_snapshot()?;
        let stack = match kind {
            "crafting_special_suspiciousstew" => self.stew(items)?,
            "crafting_decorated_pot" => self.pot(*width, *height, items)?,
            "crafting_special_armordye" => self.armor_dye(items)?,
            _ => unreachable!(),
        };
        let Some(stack) = stack else {
            self.validate_snapshot()?;
            return Ok(None);
        };
        self.validate_stack(&stack)?;
        let mut remaining = Vec::with_capacity(items.len());
        for input in items {
            let remainder = self.owner.crafting_remainder(input)?;
            ensure!(
                remainder.is_empty() || remainder.count == 1,
                "actual Item crafting remainder must be empty or count one"
            );
            ensure!(
                !input.is_empty() || remainder.is_empty(),
                "empty/AIR input cannot invent a crafting remainder"
            );
            self.validate_stack(&remainder)?;
            remaining.push(if remainder.is_empty() {
                ItemStack::empty()
            } else {
                remainder
            });
        }
        self.validate_snapshot()?;
        Ok(Some(CraftResult { stack, remaining }))
    }

    fn fresh(&self, output: SpecialOutput) -> Result<ItemStack> {
        let stack = self.owner.fresh_output(output)?;
        let item = match output {
            SpecialOutput::SuspiciousStew => SourceItem::SuspiciousStew,
            SpecialOutput::DecoratedPot => SourceItem::DecoratedPot,
        };
        ensure!(
            !stack.is_empty() && stack.count == 1 && self.owner.is_item(&stack, item)?,
            "actual special output has wrong item/count"
        );
        self.validate_stack(&stack)?;
        Ok(stack)
    }
    fn certified_copy(&self, original: &ItemStack, count_one: bool) -> Result<ItemStack> {
        let copy = if count_one {
            self.owner.copy_with_count_one(original)?
        } else {
            self.owner.copy_stack(original)?
        };
        // This is a raw data guard AFTER the mandatory actual identity proof.
        ensure!(
            copy.item_id == original.item_id
                && copy.count == if count_one { 1 } else { original.count }
                && copy.components == original.components
                && copy.removed_components == original.removed_components,
            "certified ItemStack copy changed source fields"
        );
        self.validate_stack(&copy)?;
        Ok(copy)
    }

    fn stew(&self, items: &[ItemStack]) -> Result<Option<ItemStack>> {
        let (mut flower, mut red, mut brown, mut bowl) = (false, false, false, false);
        for stack in items.iter().filter(|stack| !stack.is_empty()) {
            // An already-filled role falls through to the next predicate.
            // Dynamic tag overlap therefore cannot be reduced to item counts.
            if self.owner.is_item(stack, SourceItem::BrownMushroom)? && !brown {
                brown = true;
            } else if self.owner.is_item(stack, SourceItem::RedMushroom)? && !red {
                red = true;
            } else if self.owner.is_in_tag(stack, SpecialItemTag::SmallFlowers)? && !flower {
                flower = true;
            } else if self.owner.is_item(stack, SourceItem::Bowl)? && !bowl {
                bowl = true;
            } else {
                return Ok(None);
            }
        }
        if !(flower && red && brown && bowl) {
            return Ok(None);
        }
        let mut output = self.fresh(SpecialOutput::SuspiciousStew)?;
        // Source scans ALL nonempty items, independently of matching's flower
        // role, and stops at the first actual interface implementation.
        for stack in items.iter().filter(|stack| !stack.is_empty()) {
            let item = self.owner.stack_item(stack)?;
            if let Some(effects) = self.owner.suspicious_effects(&item)? {
                let component = self.owner.encode_suspicious_effects(&effects)?;
                ensure!(
                    component.type_id == 32,
                    "actual stew encoder returned wrong component"
                );
                set_component(&mut output, component);
                break;
            }
        }
        Ok(Some(output))
    }

    fn pot(&self, width: usize, height: usize, items: &[ItemStack]) -> Result<Option<ItemStack>> {
        if width != 3 || height != 3 {
            return Ok(None);
        }
        for (slot, stack) in items.iter().enumerate() {
            let matches = if [1, 3, 5, 7].contains(&slot) {
                self.owner
                    .is_in_tag(stack, SpecialItemTag::DecoratedPotIngredients)?
            } else {
                self.owner.is_item(stack, SourceItem::Air)?
            };
            if !matches {
                return Ok(None);
            }
        }
        let decorations = [
            self.owner.stack_item(&items[1])?,
            self.owner.stack_item(&items[3])?,
            self.owner.stack_item(&items[5])?,
            self.owner.stack_item(&items[7])?,
        ];
        // Source constructs Decorations before getting POT's default instance.
        let component = self.owner.encode_pot_decorations(&decorations)?;
        ensure!(
            component.type_id == 51,
            "actual pot encoder returned wrong component"
        );
        let mut output = self.fresh(SpecialOutput::DecoratedPot)?;
        // This call is unconditional, even for default brick decorations.
        set_component(&mut output, component);
        Ok(Some(output))
    }

    fn armor_dye(&self, items: &[ItemStack]) -> Result<Option<ItemStack>> {
        let mut wearable = false;
        let mut has_dye = false;
        for stack in items.iter().filter(|stack| !stack.is_empty()) {
            if self.owner.is_in_tag(stack, SpecialItemTag::Dyeable)? {
                if wearable {
                    return Ok(None);
                }
                wearable = true;
            } else if self.owner.as_dye_item(stack)?.is_some() {
                has_dye = true;
            } else {
                return Ok(None);
            }
        }
        if !wearable || !has_dye {
            return Ok(None);
        }
        // ArmorDyeRecipe.assemble repeats classification, preserving source
        // tag-before-DyeItem precedence and its first ordinary countful copy.
        let mut copied = None;
        let mut dyes = Vec::new();
        for stack in items.iter().filter(|stack| !stack.is_empty()) {
            if self.owner.is_in_tag(stack, SpecialItemTag::Dyeable)? {
                if copied.is_some() {
                    return Ok(Some(ItemStack::empty()));
                }
                copied = Some(self.certified_copy(stack, false)?);
            } else if let Some(dye) = self.owner.as_dye_item(stack)? {
                dyes.push(dye);
            } else {
                return Ok(Some(ItemStack::empty()));
            }
        }
        let Some(copied) = copied else {
            return Ok(Some(ItemStack::empty()));
        };
        if dyes.is_empty() {
            return Ok(Some(ItemStack::empty()));
        }
        // DyedItemColor.applyDyes performs its OWN tag check and count1 copy.
        if !self.owner.is_in_tag(&copied, SpecialItemTag::Dyeable)? {
            return Ok(Some(ItemStack::empty()));
        }
        let mut output = self.certified_copy(&copied, true)?;
        let previous = self.owner.dyed_color(&output)?;
        let mut sums = [0_i32; 3];
        let mut brightness = 0_i32;
        let mut count = 0_i32;
        if let Some(previous) = previous {
            accumulate_color(previous.rgb as u32, &mut sums, &mut brightness);
            count = count.wrapping_add(1);
        }
        for dye in dyes {
            let color = self.owner.dye_texture_diffuse_argb(&dye)?;
            accumulate_color(color, &mut sums, &mut brightness);
            count = count.wrapping_add(1);
        }
        let mut channels = sums.map(|sum| sum / count);
        let average_brightness = brightness as f32 / count as f32;
        let maximum = *channels.iter().max().unwrap() as f32;
        for channel in &mut channels {
            // Rust's saturating float cast also maps NaN to zero, as Java's
            // f2i does. Keep source multiply then divide at f32 precision.
            *channel = (*channel as f32 * average_brightness / maximum) as i32;
        }
        let rgb = (channels[0] << 16) | (channels[1] << 8) | channels[2];
        let show = previous.map_or(true, |color| color.show_in_tooltip);
        let mut fields = Writer::new();
        fields.i32(rgb).bool(show);
        set_component(
            &mut output,
            Component {
                type_id: 24,
                data: fields.into_inner(),
            },
        );
        Ok(Some(output))
    }
}

fn accumulate_color(rgb: u32, sums: &mut [i32; 3], brightness: &mut i32) {
    let channels = [
        ((rgb >> 16) & 255) as i32,
        ((rgb >> 8) & 255) as i32,
        (rgb & 255) as i32,
    ];
    for (sum, channel) in sums.iter_mut().zip(channels) {
        *sum = sum.wrapping_add(channel);
    }
    *brightness = brightness.wrapping_add(*channels.iter().max().unwrap());
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
    stack.normalize_patch();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Reader;
    use std::{
        cell::{Cell, RefCell},
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
    };

    #[derive(Debug)]
    struct Item {
        name: &'static str,
        wire: i32,
    }
    #[derive(Debug)]
    struct Effect {
        wire: i32,
    }
    #[derive(Clone, Debug)]
    struct Effects {
        effect: Arc<Effect>,
        duration: i32,
    }
    #[derive(Clone, Debug)]
    struct Snapshot(Arc<()>);
    impl PartialEq for Snapshot {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for Snapshot {}

    // Explicit controlled native owners, not an actual configured Item/Block/
    // tag/effect registry or Main provider. Wire IDs bind INTO these fixture
    // objects; they are never their equality/identity operation.
    struct Owner {
        snapshot: RefCell<Snapshot>,
        items: BTreeMap<&'static str, Arc<Item>>,
        flowers: BTreeSet<&'static str>,
        pot: BTreeSet<&'static str>,
        dyeable: BTreeSet<&'static str>,
        dyes: BTreeMap<&'static str, u32>,
        effects: BTreeMap<&'static str, Effects>,
        trace: RefCell<Vec<String>>,
        copy_available: Cell<bool>,
        replace_on_remainder: Cell<bool>,
        missing_tag: Cell<bool>,
    }
    impl Owner {
        fn new() -> Self {
            let names = [
                "air",
                "brown_mushroom",
                "red_mushroom",
                "bowl",
                "suspicious_stew",
                "decorated_pot",
                "dandelion",
                "brick",
                "diamond",
                "emerald",
                "stick",
                "stone",
                "red_dye",
                "blue_dye",
            ];
            let items = names
                .into_iter()
                .map(|name| {
                    (
                        name,
                        Arc::new(Item {
                            name,
                            wire: RecipeBook::vanilla().item_id(name).unwrap(),
                        }),
                    )
                })
                .collect();
            let effect = Arc::new(Effect { wire: 0 });
            Self {
                snapshot: RefCell::new(Snapshot(Arc::new(()))),
                items,
                flowers: BTreeSet::from(["brown_mushroom", "dandelion"]),
                pot: BTreeSet::from(["brick", "diamond", "emerald", "stick"]),
                dyeable: BTreeSet::from(["red_dye"]),
                dyes: BTreeMap::from([("red_dye", 0xffff_0000), ("blue_dye", 0xff00_00ff)]),
                effects: BTreeMap::from([
                    (
                        "brown_mushroom",
                        Effects {
                            effect: effect.clone(),
                            duration: 41,
                        },
                    ),
                    (
                        "dandelion",
                        Effects {
                            effect,
                            duration: 82,
                        },
                    ),
                ]),
                trace: RefCell::new(vec![]),
                copy_available: Cell::new(true),
                replace_on_remainder: Cell::new(false),
                missing_tag: Cell::new(false),
            }
        }
        fn stack(&self, name: &str, count: i32) -> ItemStack {
            ItemStack::new(self.items[name].wire, count).unwrap()
        }
        fn actual(&self, stack: &ItemStack) -> Result<Arc<Item>> {
            let id = if stack.is_empty() { 0 } else { stack.item_id };
            self.items
                .values()
                .find(|item| item.wire == id)
                .cloned()
                .context("unbound controlled item wire ID")
        }
        fn check_item(&self, item: &Arc<Item>) -> Result<()> {
            ensure!(
                self.items
                    .get(item.name)
                    .is_some_and(|actual| Arc::ptr_eq(actual, item)),
                "foreign controlled item owner"
            );
            Ok(())
        }
    }
    impl SpecialItemRecipeOwner for Owner {
        type Snapshot = Snapshot;
        type Item = Arc<Item>;
        type DyeItem = Arc<Item>;
        type StewEffects = Effects;
        fn snapshot(&self) -> Result<Snapshot> {
            Ok(self.snapshot.borrow().clone())
        }
        fn validate_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
            ensure!(
                *snapshot == *self.snapshot.borrow(),
                "controlled owner replaced"
            );
            Ok(())
        }
        fn validate_recipe_book(&self, book: &RecipeBook) -> Result<()> {
            ensure!(
                std::ptr::eq(book, RecipeBook::vanilla()),
                "unbound controlled recipe owner"
            );
            Ok(())
        }
        fn validate_stack(&self, stack: &ItemStack) -> Result<()> {
            self.actual(stack).map(|_| ())
        }
        fn is_item(&self, stack: &ItemStack, item: SourceItem) -> Result<bool> {
            let key = match item {
                SourceItem::Air => "air",
                SourceItem::BrownMushroom => "brown_mushroom",
                SourceItem::RedMushroom => "red_mushroom",
                SourceItem::Bowl => "bowl",
                SourceItem::SuspiciousStew => "suspicious_stew",
                SourceItem::DecoratedPot => "decorated_pot",
            };
            Ok(Arc::ptr_eq(&self.actual(stack)?, &self.items[key]))
        }
        fn is_in_tag(&self, stack: &ItemStack, tag: SpecialItemTag) -> Result<bool> {
            ensure!(
                !self.missing_tag.get(),
                "actual controlled tag owner unavailable"
            );
            let actual = self.actual(stack)?;
            self.trace
                .borrow_mut()
                .push(format!("tag:{tag:?}:{}", actual.name));
            Ok(match tag {
                SpecialItemTag::SmallFlowers => self.flowers.contains(actual.name),
                SpecialItemTag::DecoratedPotIngredients => self.pot.contains(actual.name),
                SpecialItemTag::Dyeable => self.dyeable.contains(actual.name),
            })
        }
        fn stack_item(&self, stack: &ItemStack) -> Result<Arc<Item>> {
            self.actual(stack)
        }
        fn as_dye_item(&self, stack: &ItemStack) -> Result<Option<Arc<Item>>> {
            let item = self.actual(stack)?;
            Ok(self.dyes.contains_key(item.name).then_some(item))
        }
        fn dye_texture_diffuse_argb(&self, dye: &Arc<Item>) -> Result<u32> {
            self.check_item(dye)?;
            self.dyes
                .get(dye.name)
                .copied()
                .context("controlled item is not DyeItem")
        }
        fn suspicious_effects(&self, item: &Arc<Item>) -> Result<Option<Effects>> {
            self.check_item(item)?;
            self.trace
                .borrow_mut()
                .push(format!("effects:{}", item.name));
            Ok(self.effects.get(item.name).cloned())
        }
        fn encode_suspicious_effects(&self, effects: &Effects) -> Result<Component> {
            ensure!(
                self.effects
                    .values()
                    .any(|value| Arc::ptr_eq(&value.effect, &effects.effect)),
                "foreign controlled effect holder"
            );
            let mut fields = Writer::new();
            fields
                .varint(1)
                .varint(effects.effect.wire)
                .varint(effects.duration);
            Ok(Component {
                type_id: 32,
                data: fields.into_inner(),
            })
        }
        fn fresh_output(&self, output: SpecialOutput) -> Result<ItemStack> {
            self.trace.borrow_mut().push(format!("fresh:{output:?}"));
            Ok(self.stack(
                match output {
                    SpecialOutput::SuspiciousStew => "suspicious_stew",
                    SpecialOutput::DecoratedPot => "decorated_pot",
                },
                1,
            ))
        }
        fn encode_pot_decorations(&self, items: &[Arc<Item>; 4]) -> Result<Component> {
            self.trace
                .borrow_mut()
                .push("pot:back,left,right,front".into());
            let mut fields = Writer::new();
            fields.varint(4);
            for item in items {
                self.check_item(item)?;
                fields.varint(item.wire);
            }
            Ok(Component {
                type_id: 51,
                data: fields.into_inner(),
            })
        }
        fn copy_stack(&self, original: &ItemStack) -> Result<ItemStack> {
            ensure!(
                self.copy_available.get(),
                "actual controlled copy identity owner unavailable"
            );
            self.trace
                .borrow_mut()
                .push(format!("copy:{}", original.count));
            // This fixture certifies only its primitive/prototype values.
            crate::workstations::menu::require_component_context(original)?;
            Ok(original.clone())
        }
        fn copy_with_count_one(&self, original: &ItemStack) -> Result<ItemStack> {
            ensure!(
                self.copy_available.get(),
                "actual controlled copy identity owner unavailable"
            );
            self.trace
                .borrow_mut()
                .push(format!("copy1:{}", original.count));
            crate::workstations::menu::require_component_context(original)?;
            let mut copy = original.clone();
            copy.count = 1;
            Ok(copy)
        }
        fn dyed_color(&self, stack: &ItemStack) -> Result<Option<DyedItemColor>> {
            self.trace
                .borrow_mut()
                .push(format!("dyed:{}", stack.count));
            stack
                .effective_component_bytes(24)
                .map(|data| {
                    let mut fields = Reader::new(data);
                    let color = DyedItemColor {
                        rgb: fields.i32()?,
                        show_in_tooltip: fields.bool()?,
                    };
                    fields.finish()?;
                    Ok(color)
                })
                .transpose()
        }
        fn crafting_remainder(&self, _input: &ItemStack) -> Result<ItemStack> {
            if self.replace_on_remainder.replace(false) {
                *self.snapshot.borrow_mut() = Snapshot(Arc::new(()));
            }
            Ok(ItemStack::empty())
        }
    }
    fn recipe(kind: &str) -> String {
        let book = RecipeBook::vanilla();
        book.recipe_ids()
            .find(|id| book.recipe_type(id) == Some(kind))
            .unwrap()
            .to_owned()
    }
    fn input(width: usize, height: usize, items: Vec<ItemStack>) -> RecipeInput {
        RecipeInput::Crafting {
            width,
            height,
            items,
        }
    }

    #[test]
    fn dynamic_stew_role_overlap_and_first_nonflower_supplier_follow_source() -> Result<()> {
        let owner = Owner::new();
        let context = SpecialItemRecipeContext::new(&owner)?;
        // Pure matches has no canCraftInDimensions guard; this is not a claim
        // of an actual 1x4 native menu or an original Java runtime observation.
        let grid = input(
            1,
            4,
            vec![
                owner.stack("brown_mushroom", 1),
                owner.stack("red_mushroom", 1),
                owner.stack("dandelion", 1),
                owner.stack("bowl", 1),
            ],
        );
        let result = context
            .evaluate(
                RecipeBook::vanilla(),
                &recipe("crafting_special_suspiciousstew"),
                &grid,
            )?
            .unwrap();
        let mut effects = Reader::new(result.stack.effective_component_bytes(32).unwrap());
        assert_eq!(
            (effects.varint()?, effects.varint()?, effects.varint()?),
            (1, 0, 41)
        );
        effects.finish()?;
        assert_eq!(result.remaining.len(), 4);
        assert!(owner
            .trace
            .borrow()
            .iter()
            .any(|entry| entry == "effects:brown_mushroom"));
        assert!(!owner
            .trace
            .borrow()
            .iter()
            .any(|entry| entry == "effects:dandelion"));
        let overlap = input(
            2,
            2,
            vec![
                owner.stack("brown_mushroom", 1),
                owner.stack("brown_mushroom", 1),
                owner.stack("red_mushroom", 1),
                owner.stack("bowl", 1),
            ],
        );
        assert!(context
            .evaluate(
                RecipeBook::vanilla(),
                &recipe("crafting_special_suspiciousstew"),
                &overlap
            )?
            .is_some());
        owner.missing_tag.set(true);
        assert!(context
            .evaluate(
                RecipeBook::vanilla(),
                &recipe("crafting_special_suspiciousstew"),
                &grid
            )
            .is_err());
        Ok(())
    }

    #[test]
    fn pot_uses_actual_air_and_source_decoration_order_with_fresh_output() -> Result<()> {
        let owner = Owner::new();
        let context = SpecialItemRecipeContext::new(&owner)?;
        let mut items = vec![ItemStack::empty(); 9];
        // Source getItem observes AIR, despite this raw observation's old ID.
        items[0] = ItemStack {
            count: 0,
            item_id: owner.items["stone"].wire,
            ..ItemStack::empty()
        };
        for (slot, name) in [(1, "diamond"), (3, "emerald"), (5, "brick"), (7, "stick")] {
            items[slot] = owner.stack(name, 12);
        }
        let grid = input(3, 3, items);
        let result = context
            .evaluate(
                RecipeBook::vanilla(),
                &recipe("crafting_decorated_pot"),
                &grid,
            )?
            .unwrap();
        assert_eq!(result.stack.count, 1);
        let mut fields = Reader::new(result.stack.effective_component_bytes(51).unwrap());
        assert_eq!(fields.varint()?, 4);
        for name in ["diamond", "emerald", "brick", "stick"] {
            assert_eq!(fields.varint()?, owner.items[name].wire);
        }
        fields.finish()?;
        let trace = owner.trace.borrow();
        let encode = trace
            .iter()
            .position(|entry| entry == "pot:back,left,right,front")
            .unwrap();
        let fresh = trace
            .iter()
            .position(|entry| entry == "fresh:DecoratedPot")
            .unwrap();
        assert!(encode < fresh);
        assert!(trace.iter().all(|entry| !entry.starts_with("copy")));
        Ok(())
    }

    #[test]
    fn dye_tag_priority_two_copy_counts_and_real_owner_failures_are_explicit() -> Result<()> {
        let owner = Owner::new();
        let context = SpecialItemRecipeContext::new(&owner)?;
        let mut wearable = owner.stack("red_dye", 7);
        let mut color = Writer::new();
        color.i32(0xff0000).bool(false);
        wearable.components.push(Component {
            type_id: 24,
            data: color.into_inner(),
        });
        let grid = input(2, 1, vec![wearable, owner.stack("blue_dye", 1)]);
        let result = context
            .evaluate(
                RecipeBook::vanilla(),
                &recipe("crafting_special_armordye"),
                &grid,
            )?
            .unwrap();
        assert_eq!(result.stack.item_id, owner.items["red_dye"].wire);
        assert_eq!(result.stack.count, 1);
        assert_eq!(
            owner.dyed_color(&result.stack)?,
            Some(DyedItemColor {
                rgb: 0xff00ff,
                show_in_tooltip: false
            })
        );
        let trace = owner.trace.borrow();
        let first = trace.iter().position(|entry| entry == "copy:7").unwrap();
        let recheck = trace[first + 1..]
            .iter()
            .position(|entry| entry == "tag:Dyeable:red_dye")
            .unwrap()
            + first
            + 1;
        let second = trace.iter().position(|entry| entry == "copy1:7").unwrap();
        assert!(first < recheck && recheck < second);
        drop(trace);
        owner.copy_available.set(false);
        assert!(context
            .evaluate(
                RecipeBook::vanilla(),
                &recipe("crafting_special_armordye"),
                &grid
            )
            .is_err());
        owner.copy_available.set(true);
        owner.replace_on_remainder.set(true);
        assert!(context
            .evaluate(
                RecipeBook::vanilla(),
                &recipe("crafting_special_armordye"),
                &grid
            )
            .is_err());
        assert!(context.validate_snapshot().is_err());
        Ok(())
    }
}
