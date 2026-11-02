//! `LootTable`, `LootPool` and the pool entry containers.
//!
//! Function layers (entry, pool, table, nested tables) are kept as an explicit
//! stack and applied innermost-first the moment a stack is produced, exactly as
//! Java's chain of `LootItemFunction.decorate` consumers does, so every RNG draw
//! made by a function happens before the next entry is expanded or rolled.

use super::condition::{all_of, Condition};
use super::context::{LootContext, ParamSet, Visited};
use super::enchant::{copy_with_count, count_of, new_stack};
use super::facts::{facts, Kind};
use super::function::{apply_all, Function};
use super::number::NumberProvider;
use super::{mth, LootError, Result};
use crate::inventory::ItemStack;

/// `NestedLootTable` contents: a registry reference or an inline table.
#[derive(Debug, Clone, PartialEq)]
pub enum TableRef {
    Key(String),
    Inline(Box<LootTable>),
}

/// What a singleton container produces.
#[derive(Debug, Clone, PartialEq)]
pub enum SingletonKind {
    Item(String),
    Tag { name: String, expand: bool },
    LootTable(TableRef),
    Dynamic(String),
    Empty,
}

/// `LootPoolSingletonContainer`.
#[derive(Debug, Clone, PartialEq)]
pub struct Singleton {
    pub weight: i32,
    pub quality: i32,
    pub conditions: Vec<Condition>,
    pub functions: Vec<Function>,
    pub kind: SingletonKind,
}

/// `LootPoolEntryContainer`.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Singleton(Singleton),
    Alternatives { children: Vec<Entry>, conditions: Vec<Condition> },
    Sequence { children: Vec<Entry>, conditions: Vec<Condition> },
    Group { children: Vec<Entry>, conditions: Vec<Condition> },
}

/// `LootPoolEntry`: one expanded, weighable candidate.
#[derive(Clone, Copy)]
enum Expanded<'t> {
    /// The container itself (`LootPoolSingletonContainer.entry`).
    Container(&'t Singleton),
    /// A member of an expanded tag; it bypasses the container's functions.
    TagMember { owner: &'t Singleton, item: &'t str },
}

impl Expanded<'_> {
    fn owner(&self) -> &Singleton {
        match self {
            Self::Container(owner) | Self::TagMember { owner, .. } => owner,
        }
    }

    /// `EntryBase.getWeight(luck)`.
    fn weight(&self, luck: f32) -> i32 {
        let owner = self.owner();
        mth::floor(owner.weight as f32 + owner.quality as f32 * luck).max(0)
    }
}

type Layers<'t> = Vec<&'t [Function]>;
type Sink<'s> = &'s mut dyn FnMut(ItemStack) -> Result<()>;

/// Apply the function layers innermost-first, then hand the stack on.
fn emit(
    stack: ItemStack,
    layers: &Layers<'_>,
    ctx: &mut LootContext<'_>,
    out: Sink<'_>,
) -> Result<()> {
    let mut stack = stack;
    for layer in layers.iter().rev() {
        stack = apply_all(layer, stack, ctx)?;
    }
    out(stack)
}

impl Entry {
    fn conditions(&self) -> &[Condition] {
        match self {
            Self::Singleton(singleton) => &singleton.conditions,
            Self::Alternatives { conditions, .. }
            | Self::Sequence { conditions, .. }
            | Self::Group { conditions, .. } => conditions,
        }
    }

    /// `ComposableEntryContainer.expand`.
    fn expand<'t>(&'t self, ctx: &mut LootContext<'_>, out: &mut Vec<Expanded<'t>>) -> Result<bool> {
        if !all_of(self.conditions(), ctx)? {
            return Ok(false);
        }
        match self {
            Self::Singleton(singleton) => {
                if let SingletonKind::Tag { name, expand: true } = &singleton.kind {
                    if let Some(members) = facts().tag(Kind::Item, name) {
                        for item in members {
                            out.push(Expanded::TagMember { owner: singleton, item });
                        }
                    }
                } else {
                    out.push(Expanded::Container(singleton));
                }
                Ok(true)
            }
            Self::Alternatives { children, .. } => {
                for child in children {
                    if child.expand(ctx, out)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::Sequence { children, .. } => {
                for child in children {
                    if !child.expand(ctx, out)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Self::Group { children, .. } => {
                for child in children {
                    child.expand(ctx, out)?;
                }
                Ok(true)
            }
        }
    }
}

impl Singleton {
    /// `createItemStack` of the container itself (entry functions are a layer).
    fn create<'t>(
        &'t self,
        ctx: &mut LootContext<'t>,
        layers: &mut Layers<'t>,
        out: Sink<'_>,
    ) -> Result<()> {
        match &self.kind {
            SingletonKind::Item(item) => emit(new_stack(item)?, layers, ctx, out),
            SingletonKind::Tag { name, .. } => {
                if let Some(members) = facts().tag(Kind::Item, name) {
                    for item in members {
                        emit(new_stack(item)?, layers, ctx, &mut *out)?;
                    }
                }
                Ok(())
            }
            SingletonKind::LootTable(reference) => match reference {
                TableRef::Inline(table) => table.get_random_items_raw_with(ctx, layers, out),
                TableRef::Key(key) => {
                    let resolver = ctx.resolver();
                    let table = resolver.loot_table(key).unwrap_or(&EMPTY_TABLE);
                    table.get_random_items_raw_with(ctx, layers, out)
                }
            },
            SingletonKind::Dynamic(name) => {
                let drops = ctx.dynamic_drops(name).cloned().unwrap_or_default();
                for stack in drops {
                    emit(stack, layers, ctx, &mut *out)?;
                }
                Ok(())
            }
            SingletonKind::Empty => Ok(()),
        }
    }
}

/// `LootTable.EMPTY` (used for unknown nested references).
pub static EMPTY_TABLE: LootTable = LootTable {
    param_set: &super::context::EMPTY,
    random_sequence: None,
    pools: Vec::new(),
    functions: Vec::new(),
};

/// `LootPool`.
#[derive(Debug, Clone, PartialEq)]
pub struct LootPool {
    pub entries: Vec<Entry>,
    pub conditions: Vec<Condition>,
    pub functions: Vec<Function>,
    pub rolls: NumberProvider,
    pub bonus_rolls: NumberProvider,
}

impl LootPool {
    /// `LootPool.addRandomItem`: one weighted pick.
    fn add_random_item<'t>(
        &'t self,
        ctx: &mut LootContext<'t>,
        layers: &mut Layers<'t>,
        out: Sink<'_>,
    ) -> Result<()> {
        let mut expanded: Vec<Expanded<'t>> = Vec::new();
        for entry in &self.entries {
            entry.expand(ctx, &mut expanded)?;
        }
        let luck = ctx.luck();
        expanded.retain(|candidate| candidate.weight(luck) > 0);
        let total = expanded
            .iter()
            .fold(0i32, |sum, candidate| sum.wrapping_add(candidate.weight(luck)));
        let count = expanded.len();
        if total != 0 && count != 0 {
            if count == 1 {
                return self.create(expanded[0], ctx, layers, out);
            }
            let mut remaining = ctx.random().next_bounded_i32(total)?;
            for candidate in expanded {
                remaining = remaining.wrapping_sub(candidate.weight(luck));
                if remaining < 0 {
                    return self.create(candidate, ctx, layers, out);
                }
            }
        }
        Ok(())
    }

    fn create<'t>(
        &'t self,
        candidate: Expanded<'t>,
        ctx: &mut LootContext<'t>,
        layers: &mut Layers<'t>,
        out: Sink<'_>,
    ) -> Result<()> {
        match candidate {
            Expanded::Container(owner) => {
                layers.push(&owner.functions);
                let result = owner.create(ctx, layers, out);
                layers.pop();
                result
            }
            Expanded::TagMember { item, .. } => emit(new_stack(item)?, layers, ctx, out),
        }
    }

    /// `LootPool.addRandomItems`.
    fn add_random_items<'t>(
        &'t self,
        ctx: &mut LootContext<'t>,
        layers: &mut Layers<'t>,
        out: Sink<'_>,
    ) -> Result<()> {
        if !all_of(&self.conditions, ctx)? {
            return Ok(());
        }
        layers.push(&self.functions);
        let base = self.rolls.get_int(ctx)?;
        let bonus = mth::floor(self.bonus_rolls.get_float(ctx)? * ctx.luck());
        let rolls = base.wrapping_add(bonus);
        let mut result = Ok(());
        for _ in 0..rolls.max(0) {
            result = self.add_random_item(ctx, layers, &mut *out);
            if result.is_err() {
                break;
            }
        }
        layers.pop();
        result
    }
}

/// `LootTable`.
#[derive(Debug, Clone, PartialEq)]
pub struct LootTable {
    pub param_set: &'static ParamSet,
    pub random_sequence: Option<String>,
    pub pools: Vec<LootPool>,
    pub functions: Vec<Function>,
}

impl LootTable {
    /// `LootTable.getRandomItemsRaw(LootContext, Consumer)`.
    pub fn get_random_items_raw<'t>(
        &'t self,
        ctx: &mut LootContext<'t>,
        out: Sink<'_>,
    ) -> Result<()> {
        let mut layers: Layers<'t> = Vec::new();
        self.get_random_items_raw_with(ctx, &mut layers, out)
    }

    fn get_random_items_raw_with<'t>(
        &'t self,
        ctx: &mut LootContext<'t>,
        layers: &mut Layers<'t>,
        out: Sink<'_>,
    ) -> Result<()> {
        let visited = Visited::Table(self as *const LootTable as usize);
        if !ctx.push_visited(visited.clone()) {
            // "Detected infinite loop in loot tables" is only a log line in Java.
            return Ok(());
        }
        layers.push(&self.functions);
        let mut result = Ok(());
        for pool in &self.pools {
            result = pool.add_random_items(ctx, layers, &mut *out);
            if result.is_err() {
                break;
            }
        }
        layers.pop();
        ctx.pop_visited(&visited);
        result
    }

    /// `LootTable.getRandomItems(LootContext)`: every produced stack passes the
    /// item-feature filter and is split to the item's maximum stack size.
    pub fn get_random_items<'t>(&'t self, ctx: &mut LootContext<'t>) -> Result<Vec<ItemStack>> {
        let level = ctx.level();
        let mut stacks = Vec::new();
        let mut splitter = |stack: ItemStack| -> Result<()> {
            split_stack(level, stack, &mut |part| {
                stacks.push(part);
            })
        };
        self.get_random_items_raw(ctx, &mut splitter)?;
        Ok(stacks)
    }

    /// `LootTable.fill(Container, ...)`: roll, shuffle, split and place the
    /// results into the empty slots of a container.
    pub fn fill<'t>(&'t self, ctx: &mut LootContext<'t>, container: &mut [ItemStack]) -> Result<()> {
        let mut items = self.get_random_items(ctx)?;
        let mut slots: Vec<usize> = container
            .iter()
            .enumerate()
            .filter(|(_, stack)| stack.is_empty())
            .map(|(index, _)| index)
            .collect();
        shuffle(&mut slots, ctx)?;
        shuffle_and_split_items(&mut items, slots.len() as i32, ctx)?;
        for item in items {
            let Some(slot) = slots.pop() else {
                // "Tried to over-fill a container" is only a log line in Java.
                return Ok(());
            };
            container[slot] = if item.is_empty() { ItemStack::empty() } else { item };
        }
        Ok(())
    }
}

/// `Util.shuffle(List, RandomSource)`.
fn shuffle<T>(list: &mut [T], ctx: &mut LootContext<'_>) -> Result<()> {
    let mut index = list.len();
    while index > 1 {
        let swap = ctx.random().next_bounded_i32(index as i32)? as usize;
        list.swap(swap, index - 1);
        index -= 1;
    }
    Ok(())
}

/// `LootTable.shuffleAndSplitItems`.
fn shuffle_and_split_items(
    items: &mut Vec<ItemStack>,
    slot_count: i32,
    ctx: &mut LootContext<'_>,
) -> Result<()> {
    let mut splittable: Vec<ItemStack> = Vec::new();
    let mut kept = Vec::new();
    for stack in items.drain(..) {
        if stack.is_empty() {
            continue;
        }
        if count_of(&stack) > 1 {
            splittable.push(stack);
        } else {
            kept.push(stack);
        }
    }
    *items = kept;
    while slot_count
        .wrapping_sub(items.len() as i32)
        .wrapping_sub(splittable.len() as i32)
        > 0
        && !splittable.is_empty()
    {
        let index = mth::next_int(ctx.random(), 0, splittable.len() as i32 - 1)? as usize;
        let mut stack = splittable.remove(index);
        let amount = mth::next_int(ctx.random(), 1, count_of(&stack) / 2)?;
        // `ItemStack.split`: take min(amount, count) and shrink the original.
        let taken = amount.min(count_of(&stack));
        let split = copy_with_count(&stack, taken);
        stack.count -= taken;
        if count_of(&stack) > 1 && ctx.random().next_bool() {
            splittable.push(stack);
        } else {
            items.push(stack);
        }
        if count_of(&split) > 1 && ctx.random().next_bool() {
            splittable.push(split);
        } else {
            items.push(split);
        }
    }
    items.append(&mut splittable);
    shuffle(items, ctx)
}

/// `LootTable.createStackSplitter` for one stack.
fn split_stack(
    level: &dyn super::host::LootLevel,
    stack: ItemStack,
    accept: &mut dyn FnMut(ItemStack),
) -> Result<()> {
    let item = super::predicate::stack_item_id(&stack);
    if !level.item_enabled(&item) {
        return Ok(());
    }
    let maximum = max_stack_size(&stack)?;
    if maximum <= 0 {
        // Java would split forever; a non-positive maximum is an invalid component.
        return Err(LootError::Invalid(format!("maximum stack size {maximum}")));
    }
    let count = count_of(&stack);
    if count < maximum {
        accept(stack);
    } else {
        let mut remaining = count;
        while remaining > 0 {
            let part = copy_with_count(&stack, maximum.min(remaining));
            remaining -= part.count;
            accept(part);
        }
    }
    Ok(())
}

/// `ItemStack.getMaxStackSize()` independent of the stack's count.
fn max_stack_size(stack: &ItemStack) -> Result<i32> {
    if stack.item_id == 0 {
        return Ok(64);
    }
    let mut probe = stack.clone();
    probe.count = 1;
    probe
        .max_stack_size()
        .map_err(|error| LootError::Host(error.to_string()))
}
