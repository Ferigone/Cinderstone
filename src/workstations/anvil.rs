//! Bounded Java 1.21 anvil result computation.
//!
//! Inputs are owned by the menu; `AnvilPreview` is derived and must never be
//! returned on close. Enchantment merging requires its actual registry/map owner.
//! The host supplies actual item repair rules and hover-name resolution rather
//! than a second native copy of item/translation/registry ownership.

use crate::{
    inventory::{Component, ItemStack},
    nbt::{JavaString, Tag},
    protocol::{Reader, Writer},
};
use anyhow::{bail, ensure, Result};
use std::{any::Any, fmt::Debug, sync::Arc};

/// Source menu-field mutations before actual item/text callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnvilComputationChange {
    CostInitialized,
    RepairMaterialCount(i32),
}

/// Opaque actual-owner value. Native code never fabricates protocol-ID handles
/// or treats wire equality as holder/snapshot identity. Owners downcast their
/// own canonical references and reject foreign values.
pub trait AnvilOwnerValue: Any + Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
}
impl<T: Any + Debug + Send + Sync> AnvilOwnerValue for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
pub type AnvilEnchantmentHolder = Arc<dyn AnvilOwnerValue>;
pub type AnvilEnchantmentSnapshot = Arc<dyn AnvilOwnerValue>;
#[derive(Clone, Debug)]
pub struct AnvilEnchantmentEntry {
    pub holder: AnvilEnchantmentHolder,
    pub level: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct AnvilEnchantmentDefinition {
    pub max_level: i32,
    pub anvil_cost: i32,
}
/// A source ItemEnchantments.Mutable scratch map over the ACTUAL captured
/// registry/holder/component owner. It does not own an input or result stack.
/// All queries are pure and must reject unknown/foreign holders or snapshots.
pub trait AnvilEnchantmentSession: Debug {
    /// EnchantmentHelper.getEnchantments(addition): stored only for the exact
    /// enchanted-book item, normal otherwise; preserve actual map entry order.
    fn incoming_entries(&self) -> Result<Vec<AnvilEnchantmentEntry>>;
    /// True only when the actual holder/hash/map owner certifies that order.
    /// Protocol IDs, wire order and lexicographic sorting are not certificates.
    fn incoming_encounter_order_certified(&self) -> bool;
    /// Actual current mutable-map key encounter order, after previous setters.
    fn current_holders(&self) -> Result<Vec<AnvilEnchantmentHolder>>;
    fn same_holder(&self, a: &AnvilEnchantmentHolder, b: &AnvilEnchantmentHolder) -> Result<bool>;
    fn current_level(&self, holder: &AnvilEnchantmentHolder) -> Result<i32>;
    fn definition(&self, holder: &AnvilEnchantmentHolder) -> Result<AnvilEnchantmentDefinition>;
    /// Actual Enchantment.canEnchant(base): supported_items membership.
    fn can_enchant(&self, holder: &AnvilEnchantmentHolder, base: &ItemStack) -> Result<bool>;
    /// Both actual exclusive sets; equality is skipped by the kernel first.
    fn are_compatible(
        &self,
        a: &AnvilEnchantmentHolder,
        b: &AnvilEnchantmentHolder,
    ) -> Result<bool>;
    /// Exact Mutable.set: remove <=0, otherwise put(min(level,255)), NOT upgrade.
    fn set_level(&mut self, holder: &AnvilEnchantmentHolder, level: i32) -> Result<()>;
    /// Actual EnchantmentHelper.setEnchantments on only the selected output
    /// component (9 or23), preserving the copied base tooltip flag and all
    /// unrelated components. This must encode this map's actual holder bindings.
    fn write_back(&self, output: &mut ItemStack) -> Result<()>;
}

/// Actual item and text owners consulted by `AnvilMenu.createResult`.
pub trait AnvilContext {
    /// Standalone computation has no menu observer. Production controllers
    /// override this hook and forward every change to their actual phase owner.
    fn observe_computation_change(&mut self, _: AnvilComputationChange) -> Result<()> {
        Ok(())
    }

    /// None is an explicit absent capability, not a synthetic default registry.
    fn enchantment_snapshot(&self) -> Result<Option<AnvilEnchantmentSnapshot>> {
        Ok(None)
    }
    fn validate_enchantment_snapshot(
        &self,
        snapshot: Option<&AnvilEnchantmentSnapshot>,
    ) -> Result<()> {
        ensure!(
            snapshot.is_none() && self.enchantment_snapshot()?.is_none(),
            "anvil enchantment snapshot validation requires its actual owner"
        );
        Ok(())
    }
    /// Copy the actual base map/tooltip and read the addition against this same
    /// canonical registry snapshot. Returned scratch ownership must not borrow
    /// or replace live item/player/world owners; queries consume no randomness.
    fn begin_enchantment_merge(
        &mut self,
        _: &ItemStack,
        _: &ItemStack,
        _: &AnvilEnchantmentSnapshot,
    ) -> Result<Box<dyn AnvilEnchantmentSession>> {
        bail!("anvil enchantment combining requires actual enchantment registry/holder ownership")
    }

    fn is_valid_repair_item(&mut self, base: &ItemStack, addition: &ItemStack) -> Result<bool>;
    fn hover_name(&mut self, item: &ItemStack) -> Result<JavaString>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnvilPlayer {
    /// Player.getAbilities().instabuild, not a menu-local creative assumption.
    pub instabuild: bool,
    /// Actual current experienceLevel. Re-read immediately before taking.
    pub experience_level: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnvilPreview {
    pub stack: ItemStack,
    pub level_cost: i32,
    pub repair_material_count: i32,
}

impl AnvilPreview {
    pub fn empty() -> Self {
        Self {
            stack: ItemStack::empty(),
            level_cost: 0,
            repair_material_count: 0,
        }
    }
    pub fn may_pickup(&self, player: AnvilPlayer) -> bool {
        !self.stack.is_empty()
            && self.level_cost > 0
            && (player.instabuild || player.experience_level >= self.level_cost)
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
fn set_component(stack: &mut ItemStack, id: i32, data: Vec<u8>) {
    stack.components.retain(|component| component.type_id != id);
    stack
        .removed_components
        .retain(|component| *component != id);
    stack.components.push(Component { type_id: id, data });
    stack.components.sort_by_key(|component| component.type_id);
    stack.normalize_patch();
}
fn set_int_component(stack: &mut ItemStack, id: i32, value: i32) {
    let mut writer = Writer::new();
    writer.varint(value);
    set_component(stack, id, writer.into_inner());
}
fn remove_component(stack: &mut ItemStack, id: i32) {
    stack.components.retain(|component| component.type_id != id);
    if !stack.removed_components.contains(&id) {
        stack.removed_components.push(id);
    }
    stack.removed_components.sort_unstable();
    stack.normalize_patch();
}
fn has_enchantments(stack: &ItemStack, component: i32) -> Result<bool> {
    let Some(bytes) = stack.effective_component_bytes(component) else {
        return Ok(false);
    };
    let mut reader = Reader::new(bytes);
    let count = reader.varint()?;
    ensure!(count >= 0, "negative enchantment count");
    Ok(count > 0)
}

/// Java `StringUtil.filterText`, followed by AnvilMenu's UTF-16 length limit.
/// A rejected name leaves the current request unchanged. Isolated UTF-16 units
/// are retained, matching Java strings rather than lossy Rust conversion.
pub fn validate_name(name: &JavaString) -> Option<JavaString> {
    let units: Vec<u16> = name
        .iter_units()
        .filter(|unit| *unit >= 32 && *unit != 127 && *unit != 167)
        .collect();
    (units.len() <= 50).then(|| JavaString::from_utf16_lossless(units))
}

fn is_blank(name: &JavaString) -> bool {
    // StringUtil.isBlank uses Character.isWhitespace OR isSpaceChar on Java
    // UTF-16 units. None of these predicates include supplementary code points.
    name.iter_units().all(|unit| {
        matches!(unit,
        0x0009..=0x000d | 0x001c..=0x0020 | 0x00a0 | 0x1680 |
        0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000)
    })
}

/// Source saturates the long product at Integer.MAX_VALUE, then narrows to int.
pub fn increased_repair_cost(cost: i32) -> i32 {
    ((i64::from(cost) * 2 + 1).min(i64::from(i32::MAX))) as i32
}

/// Single AnvilMenu result computation. Existing base enchantments survive
/// supported repair/rename without a merge owner. Sacrifice combining additionally
/// requires the actual registry/holder/map/component capability below.
struct AnvilComputation {
    preview: AnvilPreview,
    broadcast_after: bool,
    retain_material_count: bool,
}

/// Existing standalone result computation. Menu controllers use the observed
/// state hook below to preserve source notification and early-return phases.
pub fn create_result(
    inputs: &[ItemStack; 2],
    item_name: Option<&JavaString>,
    player: AnvilPlayer,
    context: &mut impl AnvilContext,
) -> Result<AnvilPreview> {
    Ok(compute_result(inputs, item_name, player, context)?.preview)
}

fn compute_result(
    inputs: &[ItemStack; 2],
    item_name: Option<&JavaString>,
    player: AnvilPlayer,
    context: &mut impl AnvilContext,
) -> Result<AnvilComputation> {
    if let Some(name) = item_name {
        ensure!(
            validate_name(name).as_ref() == Some(name),
            "anvil name was not validated"
        );
    }
    let [base, addition] = inputs;
    // cpw.m sets its cost DataSlot before the first-input early return.
    context.observe_computation_change(AnvilComputationChange::CostInitialized)?;
    let enchanted_book = crate::crafting::RecipeBook::vanilla()
        .item_id("enchanted_book")
        .expect("pinned enchanted book");
    let enchantment_component = if base.item_id == enchanted_book {
        23
    } else {
        9
    };
    if base.is_empty()
        || base
            .effective_component_bytes(enchantment_component)
            .is_none()
    {
        return Ok(AnvilComputation {
            preview: AnvilPreview::empty(),
            broadcast_after: false,
            retain_material_count: true,
        });
    }
    let mut output = base.clone();
    let base_cost =
        i64::from(int_component(base, 16, 0)?) + i64::from(int_component(addition, 16, 0)?);
    let mut operation_cost: i32 = 0;
    let mut rename_cost: i32 = 0;
    let mut repair_material_count = 0;
    let mut pending_enchantments: Option<(
        Box<dyn AnvilEnchantmentSession>,
        AnvilEnchantmentSnapshot,
    )> = None;
    // The source resets this field only AFTER the first-input guard and before
    // isValidRepairItem. A previous repair count must not reach that callback.
    context.observe_computation_change(AnvilComputationChange::RepairMaterialCount(0))?;
    if !addition.is_empty() {
        if output.is_damageable()? && context.is_valid_repair_item(base, addition)? {
            let max_damage = int_component(&output, 2, 0)?;
            let mut damage = output.damage_value()?;
            let mut repaired = damage.min(max_damage / 4);
            if repaired <= 0 {
                return Ok(AnvilComputation {
                    preview: AnvilPreview::empty(),
                    broadcast_after: false,
                    retain_material_count: false,
                });
            }
            while repaired > 0 && repair_material_count < addition.count {
                damage = damage.wrapping_sub(repaired);
                set_int_component(&mut output, 3, damage.clamp(0, max_damage));
                operation_cost = operation_cost.wrapping_add(1);
                repair_material_count += 1;
                repaired = output.damage_value()?.min(max_damage / 4);
            }
            // Source stores the final count before later getHoverName calls.
            context.observe_computation_change(AnvilComputationChange::RepairMaterialCount(
                repair_material_count,
            ))?;
        } else {
            // The source tests presence of STORED_ENCHANTMENTS, not nonempty
            // contents or the enchanted-book item ID, for its book branch.
            let stored = addition.effective_component_bytes(23).is_some();
            if !stored && (base.item_id != addition.item_id || !output.is_damageable()?) {
                return Ok(AnvilComputation {
                    preview: AnvilPreview::empty(),
                    broadcast_after: false,
                    retain_material_count: false,
                });
            }
            let addition_enchantments = if addition.item_id == enchanted_book {
                23
            } else {
                9
            };
            if output.is_damageable()? && !stored {
                let max_damage = int_component(&output, 2, 0)?;
                let durability_a = max_damage.wrapping_sub(base.damage_value()?);
                let durability_b =
                    int_component(addition, 2, 0)?.wrapping_sub(addition.damage_value()?);
                let bonus = max_damage.wrapping_mul(12) / 100;
                let combined = durability_a.wrapping_add(durability_b.wrapping_add(bonus));
                let damage = max_damage.wrapping_sub(combined).max(0);
                if damage < output.damage_value()? {
                    set_int_component(&mut output, 3, damage.clamp(0, max_damage));
                    operation_cost = operation_cost.wrapping_add(2);
                }
            }
            if stored || has_enchantments(addition, addition_enchantments)? {
                let snapshot = context.enchantment_snapshot()?.ok_or_else(|| anyhow::anyhow!(
                    "anvil enchantment combining requires actual enchantment registry/holder ownership"))?;
                context.validate_enchantment_snapshot(Some(&snapshot))?;
                let mut session = context.begin_enchantment_merge(base, addition, &snapshot)?;
                let entries = session.incoming_entries()?;
                ensure!(
                    entries.len() <= 1 || session.incoming_encounter_order_certified(),
                    "multiple anvil enchantments require actual holder-map encounter order"
                );
                let mut accepted = false;
                let mut rejected = false;
                for entry in entries {
                    ensure!(
                        (0..=255).contains(&entry.level),
                        "invalid actual incoming enchantment level"
                    );
                    let old = session.current_level(&entry.holder)?;
                    ensure!(
                        (0..=255).contains(&old),
                        "invalid actual base enchantment level"
                    );
                    let mut level = if old == entry.level {
                        entry.level.wrapping_add(1)
                    } else {
                        old.max(entry.level)
                    };
                    // Source calls canEnchant even when creative/book overrides
                    // its result. Compatibility remains mandatory in both cases.
                    let mut allowed = session.can_enchant(&entry.holder, base)?;
                    if player.instabuild || base.item_id == enchanted_book {
                        allowed = true;
                    }
                    for current in session.current_holders()? {
                        if !session.same_holder(&entry.holder, &current)?
                            && !session.are_compatible(&entry.holder, &current)?
                        {
                            allowed = false;
                            operation_cost = operation_cost.wrapping_add(1);
                        }
                    }
                    if !allowed {
                        rejected = true;
                        continue;
                    }
                    accepted = true;
                    let definition = session.definition(&entry.holder)?;
                    ensure!(
                        (1..=255).contains(&definition.max_level) && definition.anvil_cost >= 0,
                        "invalid actual anvil enchantment definition"
                    );
                    level = level.min(definition.max_level);
                    session.set_level(&entry.holder, level)?;
                    let multiplier = if stored {
                        (definition.anvil_cost / 2).max(1)
                    } else {
                        definition.anvil_cost
                    };
                    operation_cost = operation_cost.wrapping_add(multiplier.wrapping_mul(level));
                    if base.count > 1 {
                        operation_cost = 40;
                    }
                }
                context.validate_enchantment_snapshot(Some(&snapshot))?;
                if rejected && !accepted {
                    return Ok(AnvilComputation {
                        preview: AnvilPreview::empty(),
                        broadcast_after: false,
                        retain_material_count: false,
                    });
                }
                pending_enchantments = Some((session, snapshot));
            }
        }
    }
    if item_name.is_none_or(is_blank) {
        if base.effective_component_bytes(5).is_some() {
            rename_cost = 1;
            operation_cost = operation_cost.wrapping_add(1);
            remove_component(&mut output, 5);
        }
    } else if let Some(name) = item_name {
        if *name != context.hover_name(base)? {
            rename_cost = 1;
            operation_cost = operation_cost.wrapping_add(1);
            set_component(
                &mut output,
                5,
                Tag::String(name.clone()).encode_network_anonymous()?,
            );
        }
    }
    let mut level_cost =
        (base_cost + i64::from(operation_cost)).clamp(0, i64::from(i32::MAX)) as i32;
    if operation_cost <= 0 {
        output = ItemStack::empty();
    }
    if rename_cost == operation_cost && rename_cost > 0 && level_cost >= 40 {
        level_cost = 39;
    }
    if level_cost >= 40 && !player.instabuild {
        output = ItemStack::empty();
    }
    if !output.is_empty() {
        let mut repair_cost = int_component(&output, 16, 0)?.max(int_component(addition, 16, 0)?);
        if rename_cost != operation_cost || rename_cost == 0 {
            repair_cost = increased_repair_cost(repair_cost);
        }
        set_int_component(&mut output, 16, repair_cost);
        if let Some((session, snapshot)) = &pending_enchantments {
            context.validate_enchantment_snapshot(Some(snapshot))?;
            session.write_back(&mut output)?;
        }
    }
    if let Some((_, snapshot)) = &pending_enchantments {
        context.validate_enchantment_snapshot(Some(snapshot))?;
    }
    Ok(AnvilComputation {
        preview: AnvilPreview {
            stack: output,
            level_cost,
            repair_material_count,
        },
        broadcast_after: true,
        retain_material_count: false,
    })
}

/// State changes needed by the host's result-slot transaction. This is a plan,
/// not another owned output inventory. Its output becomes owned only when the
/// canonical menu and real player/world owners commit it once.
#[derive(Debug)]
pub struct AnvilTake {
    expected_revision: u64,
    expected_inputs: [ItemStack; 2],
    expected_preview: AnvilPreview,
    after_inputs: [ItemStack; 2],
    result: ItemStack,
    /// Apply this to the actual player before clearing the first input.
    experience_levels_delta: i32,
    /// True only for a non-instabuild player; the later actual block callback
    /// draws Player.getRandom().nextFloat only if the block is still an anvil.
    may_damage_anvil: bool,
}

impl AnvilTake {
    pub fn result(&self) -> &ItemStack {
        &self.result
    }
    pub fn experience_levels_delta(&self) -> i32 {
        self.experience_levels_delta
    }
    pub fn may_damage_anvil(&self) -> bool {
        self.may_damage_anvil
    }
    pub fn after_inputs(&self) -> &[ItemStack; 2] {
        &self.after_inputs
    }
}

#[derive(Clone, Debug)]
pub struct AnvilState {
    revision: u64,
    inputs: [ItemStack; 2],
    item_name: Option<JavaString>,
    preview: AnvilPreview,
}

impl Default for AnvilState {
    fn default() -> Self {
        Self {
            revision: 0,
            inputs: [ItemStack::empty(), ItemStack::empty()],
            item_name: None,
            preview: AnvilPreview::empty(),
        }
    }
}

impl AnvilState {
    pub fn inputs(&self) -> &[ItemStack; 2] {
        &self.inputs
    }
    pub fn preview(&self) -> &AnvilPreview {
        &self.preview
    }
    pub fn item_name(&self) -> Option<&JavaString> {
        self.item_name.as_ref()
    }
    /// Caller stages a transfer into the canonical input slot. Validation or
    /// unsupported-context errors leave the complete live menu unchanged.
    pub fn set_input(
        &mut self,
        slot: usize,
        stack: ItemStack,
        player: AnvilPlayer,
        context: &mut impl AnvilContext,
    ) -> Result<()> {
        ensure!(slot < 2, "invalid anvil input slot");
        if !stack.is_empty() {
            ensure!(
                stack.count <= stack.max_stack_size()?.min(99),
                "anvil input exceeds slot capacity"
            );
            stack.encode_canonical()?;
        }
        let mut next = self.clone();
        next.inputs[slot] = if stack.is_empty() {
            ItemStack::empty()
        } else {
            stack
        };
        next.recompute(player, context)?;
        *self = next;
        Ok(())
    }
    pub fn set_item_name(
        &mut self,
        name: &JavaString,
        player: AnvilPlayer,
        context: &mut impl AnvilContext,
    ) -> Result<bool> {
        let Some(name) = validate_name(name) else {
            return Ok(false);
        };
        if self.item_name.as_ref() == Some(&name) {
            return Ok(false);
        }
        let mut next = self.clone();
        next.item_name = Some(name);
        next.recompute(player, context)?;
        *self = next;
        Ok(true)
    }
    pub fn recompute(
        &mut self,
        player: AnvilPlayer,
        context: &mut impl AnvilContext,
    ) -> Result<()> {
        self.recompute_observed(player, context).map(|_| ())
    }
    /// Recompute once after the superclass has broadcast the old preview/cost.
    /// True means createResult reached its source tail broadcast; early returns
    /// intentionally do not broadcast again. No result logic is duplicated.
    pub fn recompute_observed(
        &mut self,
        player: AnvilPlayer,
        context: &mut impl AnvilContext,
    ) -> Result<bool> {
        let mut result = compute_result(&self.inputs, self.item_name.as_ref(), player, context)?;
        if result.retain_material_count {
            // Source returns before resetting repairItemCountCost when the
            // first input is empty or lacks its enchantment component.
            result.preview.repair_material_count = self.preview.repair_material_count;
        }
        self.revision = self.revision.wrapping_add(1);
        self.preview = result.preview;
        Ok(result.broadcast_after)
    }
    /// Mutate the actual temporary input object without notifying. This is for
    /// source in-place shrink/grow/remove phases; the controller must immediately
    /// deliver the matching container/Slot setChanged boundary. Whole enclosing
    /// host/menu staging is required, including rollback on unsupported context.
    pub fn mutate_input_in_place(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
        ensure!(slot < 2, "invalid anvil input slot");
        if !stack.is_empty() {
            stack.encode_canonical()?;
        }
        self.inputs[slot] = if stack.is_empty() {
            ItemStack::empty()
        } else {
            stack
        };
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    /// ResultContainer permits count mutation/removal of its derived preview.
    /// It never manufactures a result or an independently owned output. Only
    /// the enclosing atomic transfer plus actual onTake consumes input owners.
    pub fn mutate_result_in_place(&mut self, stack: ItemStack) -> Result<()> {
        ensure!(
            stack.is_empty()
                || (stack.count <= self.preview.stack.count
                    && stack.same_item_same_components(&self.preview.stack)?),
            "anvil result is derived and cannot be replaced"
        );
        self.preview.stack = if stack.is_empty() {
            ItemStack::empty()
        } else {
            stack
        };
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    /// Validated source setItemName phase before createResult. The existing
    /// derived result is renamed in place first; no owned item is introduced.
    pub fn request_name_observed(&mut self, name: &JavaString) -> Result<bool> {
        let Some(name) = validate_name(name) else {
            return Ok(false);
        };
        if self.item_name.as_ref() == Some(&name) {
            return Ok(false);
        }
        if !self.preview.stack.is_empty() {
            if is_blank(&name) {
                remove_component(&mut self.preview.stack, 5);
            } else {
                set_component(
                    &mut self.preview.stack,
                    5,
                    Tag::String(name.clone()).encode_network_anonymous()?,
                );
            }
        }
        self.item_name = Some(name);
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }
    /// Source DataSlot reset after both actual input-container mutations.
    pub fn reset_cost_after_take(&mut self) {
        self.preview.level_cost = 0;
        self.revision = self.revision.wrapping_add(1);
    }
    /// Call after source Slot transfer/mayPickup checks, with actual current
    /// player abilities/levels. No cursor, XP, RNG, block or input is changed.
    pub fn prepare_take(&self, player: AnvilPlayer) -> Result<AnvilTake> {
        ensure!(
            self.preview.may_pickup(player),
            "anvil result is not takeable"
        );
        let mut after_inputs = self.inputs.clone();
        after_inputs[0] = ItemStack::empty();
        if self.preview.repair_material_count > 0
            && !after_inputs[1].is_empty()
            && after_inputs[1].count > self.preview.repair_material_count
        {
            after_inputs[1].count -= self.preview.repair_material_count;
        } else {
            after_inputs[1] = ItemStack::empty();
        }
        Ok(AnvilTake {
            expected_revision: self.revision,
            expected_inputs: self.inputs.clone(),
            expected_preview: self.preview.clone(),
            after_inputs,
            result: self.preview.stack.clone(),
            experience_levels_delta: if player.instabuild {
                0
            } else {
                -self.preview.level_cost
            },
            may_damage_anvil: !player.instabuild,
        })
    }
    /// Validate before changing real host owners. Host then applies XP, commits
    /// inputs/cost, and invokes the captured anvil world callback, in that order.
    pub fn validate_take(&self, take: &AnvilTake) -> Result<()> {
        ensure!(
            self.revision == take.expected_revision
                && self.inputs == take.expected_inputs
                && self.preview == take.expected_preview,
            "stale anvil result transaction"
        );
        Ok(())
    }
    pub fn commit_take(&mut self, take: AnvilTake) -> Result<ItemStack> {
        self.validate_take(&take)?;
        self.revision = self.revision.wrapping_add(1);
        self.inputs = take.after_inputs;
        self.preview = AnvilPreview::empty();
        Ok(take.result)
    }
    /// ContainerLevelAccess.clearContainer receives only these two owned inputs;
    /// the derived result is discarded. The host still owns cursor/return/drop.
    pub fn close_inputs(&mut self) -> [ItemStack; 2] {
        self.revision = self.revision.wrapping_add(1);
        self.preview = AnvilPreview::empty();
        std::mem::replace(&mut self.inputs, [ItemStack::empty(), ItemStack::empty()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Context;
    impl AnvilContext for Context {
        fn is_valid_repair_item(&mut self, base: &ItemStack, addition: &ItemStack) -> Result<bool> {
            let book = crate::crafting::RecipeBook::vanilla();
            Ok(base.item_id == book.item_id("iron_sword").unwrap()
                && addition.item_id == book.item_id("iron_ingot").unwrap())
        }
        fn hover_name(&mut self, _: &ItemStack) -> Result<JavaString> {
            Ok("Iron Sword".into())
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(
            crate::crafting::RecipeBook::vanilla()
                .item_id(name)
                .unwrap(),
            count,
        )
        .unwrap()
    }
    const SURVIVAL: AnvilPlayer = AnvilPlayer {
        instabuild: false,
        experience_level: 100,
    };

    #[test]
    fn expensive_rename_caps_at_39_without_increasing_prior_work() -> Result<()> {
        let mut sword = item("iron_sword", 1);
        set_int_component(&mut sword, 16, 63);
        let result = create_result(
            &[sword.clone(), ItemStack::empty()],
            Some(&"Named".into()),
            SURVIVAL,
            &mut Context,
        )?;
        assert_eq!(result.level_cost, 39);
        assert_eq!(int_component(&result.stack, 16, 0)?, 63);
        assert_eq!(
            Tag::decode_anonymous(result.stack.effective_component_bytes(5).unwrap())?,
            Tag::String("Named".into())
        );
        assert!(!result.may_pickup(AnvilPlayer {
            experience_level: 38,
            ..SURVIVAL
        }));
        Ok(())
    }

    #[test]
    fn material_repair_consumes_only_needed_items_and_preview_is_not_returned() -> Result<()> {
        let mut sword = item("iron_sword", 1);
        set_int_component(&mut sword, 3, 100);
        let mut menu = AnvilState::default();
        menu.set_input(0, sword, SURVIVAL, &mut Context)?;
        menu.set_input(1, item("iron_ingot", 8), SURVIVAL, &mut Context)?;
        assert_eq!(menu.preview().repair_material_count, 2);
        assert_eq!(menu.preview().level_cost, 2);
        assert_eq!(menu.preview().stack.damage_value()?, 0);
        assert_eq!(int_component(&menu.preview().stack, 16, 0)?, 1);
        let take = menu.prepare_take(SURVIVAL)?;
        assert_eq!(take.experience_levels_delta, -2);
        assert_eq!(take.after_inputs()[1].count, 6);
        assert_eq!(menu.inputs()[1].count, 8); // prepare has no mutation
        let output = menu.commit_take(take)?;
        assert_eq!(output.count, 1);
        assert!(menu.preview().stack.is_empty());
        let returned = menu.close_inputs();
        assert!(returned[0].is_empty());
        assert_eq!(returned[1].count, 6);
        Ok(())
    }

    #[test]
    fn unsupported_transfer_leaves_inputs_and_previous_preview_unchanged() -> Result<()> {
        let mut menu = AnvilState::default();
        menu.set_input(0, item("iron_sword", 1), SURVIVAL, &mut Context)?;
        menu.set_item_name(&"Named".into(), SURVIVAL, &mut Context)?;
        let before_inputs = menu.inputs().clone();
        let before_preview = menu.preview().clone();
        let mut sacrifice = item("iron_sword", 1);
        let mut enchantments = Writer::new();
        enchantments.varint(1).varint(0).varint(1).bool(true);
        set_component(&mut sacrifice, 9, enchantments.into_inner());
        assert!(menu
            .set_input(1, sacrifice, SURVIVAL, &mut Context)
            .is_err());
        assert_eq!(menu.inputs(), &before_inputs);
        assert_eq!(menu.preview(), &before_preview);
        Ok(())
    }

    #[test]
    fn name_filter_counts_java_units_and_space_characters() {
        assert_eq!(validate_name(&"\nA§B\u{7f}".into()), Some("AB".into()));
        assert!(validate_name(&JavaString::from("😀".repeat(26))).is_none());
        assert!(is_blank(&"\u{a0}\u{2007}\u{202f}".into()));
        assert!(!is_blank(&"\u{0085}".into()));
    }
}

/// Real captured ContainerLevelAccess plus actual player random owner.
/// Implementations must preflight their world transaction before this callback:
/// failures after a draw cannot be retried with a replacement RNG or block.
pub trait AnvilWorldAccess {
    type BlockState;
    fn block_state(&mut self) -> Result<Self::BlockState>;
    fn is_anvil(&self, state: &Self::BlockState) -> Result<bool>;
    fn player_random_next_float(&mut self) -> Result<f32>;
    /// Actual AnvilBlock.damage(state), preserving the state's real properties.
    fn damaged_state(&self, state: &Self::BlockState) -> Result<Option<Self::BlockState>>;
    fn destroy_block_without_drop(&mut self) -> Result<()>;
    fn set_block(&mut self, state: Self::BlockState, flags: i32) -> Result<()>;
    fn level_event(&mut self, event: i32, data: i32) -> Result<()>;
}

/// Source AnvilMenu.lambda$onTake$2, called after XP/input/cost changes.
/// The source calls the *player's* random only when wear can occur.
pub fn finish_anvil_take(instabuild: bool, access: &mut impl AnvilWorldAccess) -> Result<()> {
    let state = access.block_state()?;
    if !instabuild && access.is_anvil(&state)? && access.player_random_next_float()? < 0.12_f32 {
        match access.damaged_state(&state)? {
            None => {
                access.destroy_block_without_drop()?;
                access.level_event(1029, 0)?;
            }
            Some(damaged) => {
                access.set_block(damaged, 2)?;
                access.level_event(1030, 0)?;
            }
        }
    } else {
        access.level_event(1030, 0)?;
    }
    Ok(())
}

/// Restricted native owner used only by focused tests. Numeric IDs come from
/// the pinned registry metadata; max-level/anvil-cost facts were read directly
/// from the original sharpness/smite/unbreaking enchantment resources. The
/// limited item/exclusive sets and map order below are explicit test fixtures,
/// never a production registry, Fastutil order certificate or gameplay oracle.
#[cfg(test)]
pub(crate) mod enchantment_fixture {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    #[derive(Debug)]
    struct Holder {
        owner: u64,
        id: i32,
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Snapshot {
        owner: u64,
        epoch: u64,
    }
    #[derive(Clone, Debug)]
    pub struct Owner {
        owner: u64,
        pub epoch: u64,
        pub certify_multi: bool,
        holders: Vec<AnvilEnchantmentHolder>,
    }
    pub fn id(name: &str) -> i32 {
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../../tools/crafting_metadata.json")).unwrap();
        metadata["registries"]["minecraft:enchantment"][format!("minecraft:{name}")]
            .as_i64()
            .unwrap() as i32
    }
    pub fn enchanted(mut stack: ItemStack, entries: &[(&str, i32)], tooltip: bool) -> ItemStack {
        let component = if Some(stack.item_id)
            == crate::crafting::RecipeBook::vanilla().item_id("enchanted_book")
        {
            23
        } else {
            9
        };
        let mut writer = Writer::new();
        writer.varint(entries.len() as i32);
        for (name, level) in entries {
            writer.varint(id(name)).varint(*level);
        }
        writer.bool(tooltip);
        set_component(&mut stack, component, writer.into_inner());
        stack
    }
    pub fn entries(stack: &ItemStack) -> Result<(Vec<(i32, i32)>, bool)> {
        let component = if Some(stack.item_id)
            == crate::crafting::RecipeBook::vanilla().item_id("enchanted_book")
        {
            23
        } else {
            9
        };
        let Some(bytes) = stack.effective_component_bytes(component) else {
            return Ok((vec![], true));
        };
        let mut reader = Reader::new(bytes);
        let count = reader.varint()?;
        ensure!(count >= 0, "fixture map length");
        let mut out = vec![];
        for _ in 0..count {
            out.push((reader.varint()?, reader.varint()?));
        }
        let tooltip = reader.bool()?;
        reader.finish()?;
        Ok((out, tooltip))
    }
    impl Owner {
        pub fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let owner = NEXT.fetch_add(1, Ordering::Relaxed);
            Self {
                owner,
                epoch: 1,
                certify_multi: false,
                holders: ["sharpness", "smite", "unbreaking"]
                    .into_iter()
                    .map(|name| {
                        Arc::new(Holder {
                            owner,
                            id: id(name),
                        }) as AnvilEnchantmentHolder
                    })
                    .collect(),
            }
        }
        fn holder(&self, id: i32) -> Result<AnvilEnchantmentHolder> {
            self.holders
                .iter()
                .find(|h| h.as_ref().as_any().downcast_ref::<Holder>().unwrap().id == id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("fixture holder unavailable"))
        }
        fn holder_id(&self, h: &AnvilEnchantmentHolder) -> Result<i32> {
            let value = h
                .as_ref()
                .as_any()
                .downcast_ref::<Holder>()
                .ok_or_else(|| anyhow::anyhow!("foreign fixture holder"))?;
            ensure!(
                value.owner == self.owner && self.holders.iter().any(|known| Arc::ptr_eq(known, h)),
                "foreign fixture holder owner"
            );
            Ok(value.id)
        }
    }
    impl AnvilContext for Owner {
        fn enchantment_snapshot(&self) -> Result<Option<AnvilEnchantmentSnapshot>> {
            Ok(Some(Arc::new(Snapshot {
                owner: self.owner,
                epoch: self.epoch,
            })))
        }
        fn validate_enchantment_snapshot(
            &self,
            snapshot: Option<&AnvilEnchantmentSnapshot>,
        ) -> Result<()> {
            let value = snapshot
                .and_then(|s| s.as_ref().as_any().downcast_ref::<Snapshot>())
                .ok_or_else(|| anyhow::anyhow!("missing fixture snapshot"))?;
            ensure!(
                *value
                    == Snapshot {
                        owner: self.owner,
                        epoch: self.epoch
                    },
                "fixture registry epoch/owner changed"
            );
            Ok(())
        }
        fn begin_enchantment_merge(
            &mut self,
            base: &ItemStack,
            addition: &ItemStack,
            snapshot: &AnvilEnchantmentSnapshot,
        ) -> Result<Box<dyn AnvilEnchantmentSession>> {
            self.validate_enchantment_snapshot(Some(snapshot))?;
            let (base_entries, tooltip) = entries(base)?;
            let (incoming, _) = entries(addition)?;
            let map = base_entries
                .into_iter()
                .map(|(id, level)| Ok((self.holder(id)?, level)))
                .collect::<Result<Vec<_>>>()?;
            let incoming = incoming
                .into_iter()
                .map(|(id, level)| {
                    Ok(AnvilEnchantmentEntry {
                        holder: self.holder(id)?,
                        level,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Box::new(Session {
                owner: self.clone(),
                map,
                incoming,
                tooltip,
            }))
        }
        fn is_valid_repair_item(&mut self, _: &ItemStack, _: &ItemStack) -> Result<bool> {
            Ok(false)
        }
        fn hover_name(&mut self, _: &ItemStack) -> Result<JavaString> {
            Ok("Original".into())
        }
    }
    #[derive(Debug)]
    struct Session {
        owner: Owner,
        map: Vec<(AnvilEnchantmentHolder, i32)>,
        incoming: Vec<AnvilEnchantmentEntry>,
        tooltip: bool,
    }
    impl AnvilEnchantmentSession for Session {
        fn incoming_entries(&self) -> Result<Vec<AnvilEnchantmentEntry>> {
            Ok(self.incoming.clone())
        }
        fn incoming_encounter_order_certified(&self) -> bool {
            self.owner.certify_multi
        }
        fn current_holders(&self) -> Result<Vec<AnvilEnchantmentHolder>> {
            Ok(self.map.iter().map(|(h, _)| h.clone()).collect())
        }
        fn same_holder(
            &self,
            a: &AnvilEnchantmentHolder,
            b: &AnvilEnchantmentHolder,
        ) -> Result<bool> {
            self.owner.holder_id(a)?;
            self.owner.holder_id(b)?;
            Ok(Arc::ptr_eq(a, b))
        }
        fn current_level(&self, holder: &AnvilEnchantmentHolder) -> Result<i32> {
            self.owner.holder_id(holder)?;
            Ok(self
                .map
                .iter()
                .find(|(h, _)| Arc::ptr_eq(h, holder))
                .map(|(_, l)| *l)
                .unwrap_or(0))
        }
        fn definition(
            &self,
            holder: &AnvilEnchantmentHolder,
        ) -> Result<AnvilEnchantmentDefinition> {
            let value = self.owner.holder_id(holder)?;
            Ok(if value == id("sharpness") {
                AnvilEnchantmentDefinition {
                    max_level: 5,
                    anvil_cost: 1,
                }
            } else if value == id("smite") {
                AnvilEnchantmentDefinition {
                    max_level: 5,
                    anvil_cost: 2,
                }
            } else {
                AnvilEnchantmentDefinition {
                    max_level: 3,
                    anvil_cost: 2,
                }
            })
        }
        fn can_enchant(&self, holder: &AnvilEnchantmentHolder, base: &ItemStack) -> Result<bool> {
            self.owner.holder_id(holder)?;
            Ok(Some(base.item_id) == crate::crafting::RecipeBook::vanilla().item_id("iron_sword"))
        }
        fn are_compatible(
            &self,
            a: &AnvilEnchantmentHolder,
            b: &AnvilEnchantmentHolder,
        ) -> Result<bool> {
            let a = self.owner.holder_id(a)?;
            let b = self.owner.holder_id(b)?;
            Ok(!((a == id("sharpness") && b == id("smite"))
                || (a == id("smite") && b == id("sharpness"))))
        }
        fn set_level(&mut self, holder: &AnvilEnchantmentHolder, level: i32) -> Result<()> {
            self.owner.holder_id(holder)?;
            if level <= 0 {
                self.map.retain(|(h, _)| !Arc::ptr_eq(h, holder));
            } else if let Some((_, old)) = self.map.iter_mut().find(|(h, _)| Arc::ptr_eq(h, holder))
            {
                *old = level.min(255);
            } else {
                self.map.push((holder.clone(), level.min(255)));
            }
            Ok(())
        }
        fn write_back(&self, output: &mut ItemStack) -> Result<()> {
            let component = if Some(output.item_id)
                == crate::crafting::RecipeBook::vanilla().item_id("enchanted_book")
            {
                23
            } else {
                9
            };
            let mut writer = Writer::new();
            writer.varint(self.map.len() as i32);
            for (holder, level) in &self.map {
                writer.varint(self.owner.holder_id(holder)?).varint(*level);
            }
            writer.bool(self.tooltip);
            set_component(output, component, writer.into_inner());
            Ok(())
        }
    }
}

#[cfg(test)]
mod enchantment_tests {
    use super::enchantment_fixture::{enchanted, entries, id, Owner};
    use super::*;
    fn item(name: &str) -> ItemStack {
        ItemStack::new(
            crate::crafting::RecipeBook::vanilla()
                .item_id(name)
                .unwrap(),
            1,
        )
        .unwrap()
    }
    const PLAYER: AnvilPlayer = AnvilPlayer {
        instabuild: false,
        experience_level: 100,
    };
    #[test]
    fn actual_owner_book_cost_level_set_tooltip_and_presence_rules() -> Result<()> {
        let mut owner = Owner::new();
        let base = enchanted(item("iron_sword"), &[("sharpness", 2)], false);
        let addition = enchanted(item("enchanted_book"), &[("sharpness", 2)], true);
        let result = create_result(&[base.clone(), addition.clone()], None, PLAYER, &mut owner)?;
        assert_eq!(result.level_cost, 3);
        assert_eq!(entries(&result.stack)?, (vec![(id("sharpness"), 3)], false));
        let mut damaged_base = base.clone();
        let mut damaged_sacrifice = enchanted(item("iron_sword"), &[("sharpness", 2)], true);
        set_int_component(&mut damaged_base, 3, 100);
        set_int_component(&mut damaged_sacrifice, 3, 100);
        let combined = create_result(&[damaged_base, damaged_sacrifice], None, PLAYER, &mut owner)?;
        assert_eq!(combined.level_cost, 5); // durability2 + normal anvil_cost1 * level3
        assert_eq!(combined.stack.damage_value()?, 0);
        assert_eq!(entries(&combined.stack)?.0, vec![(id("sharpness"), 3)]);
        let overlevel = enchanted(item("iron_sword"), &[("sharpness", 9)], false);
        let result = create_result(
            &[
                overlevel,
                enchanted(item("enchanted_book"), &[("sharpness", 1)], true),
            ],
            None,
            PLAYER,
            &mut owner,
        )?;
        assert_eq!(result.level_cost, 5);
        assert_eq!(entries(&result.stack)?.0, vec![(id("sharpness"), 5)]); // set, not upgrade
        let bookbase = enchanted(item("enchanted_book"), &[("sharpness", 2)], false);
        let result = create_result(
            &[
                bookbase,
                enchanted(item("enchanted_book"), &[("unbreaking", 1)], true),
            ],
            None,
            PLAYER,
            &mut owner,
        )?;
        assert_eq!(result.stack.item_id, item("enchanted_book").item_id);
        assert_eq!(entries(&result.stack)?.0.len(), 2);
        // Presence on an arbitrary addition enables the discount, but its
        // actual normal9 map is read; stored23 is selected only for exact book.
        let mut arbitrary = enchanted(item("stone"), &[("unbreaking", 1)], true);
        let mut empty = Writer::new();
        empty.varint(0).bool(true);
        set_component(&mut arbitrary, 23, empty.into_inner());
        let mut damaged = item("iron_sword");
        set_int_component(&mut damaged, 3, 100);
        let result = create_result(&[damaged, arbitrary], None, PLAYER, &mut owner)?;
        assert_eq!(result.level_cost, 1);
        assert_eq!(result.stack.damage_value()?, 100);
        assert_eq!(entries(&result.stack)?.0, vec![(id("unbreaking"), 1)]);
        let bad = enchanted(item("enchanted_book"), &[("smite", 1)], true);
        let result = create_result(
            &[base.clone(), bad],
            Some(&"Named".into()),
            AnvilPlayer {
                instabuild: true,
                ..PLAYER
            },
            &mut owner,
        )?;
        assert!(result.stack.is_empty());
        assert_eq!(result.level_cost, 0); // creative still incompatible; before rename
        let mut expensive = base;
        set_int_component(&mut expensive, 16, 40);
        let result = create_result(
            &[expensive.clone(), addition.clone()],
            None,
            PLAYER,
            &mut owner,
        )?;
        assert!(result.stack.is_empty());
        assert_eq!(result.level_cost, 43);
        let result = create_result(
            &[expensive, addition],
            None,
            AnvilPlayer {
                instabuild: true,
                ..PLAYER
            },
            &mut owner,
        )?;
        assert!(!result.stack.is_empty());
        assert_eq!(int_component(&result.stack, 16, 0)?, 81);
        Ok(())
    }
    #[test]
    fn only_owner_certified_multi_entry_order_can_select_conflicting_enchantments() -> Result<()> {
        let mut owner = Owner::new();
        let mut state = AnvilState::default();
        state.set_input(0, item("iron_sword"), PLAYER, &mut owner)?;
        let addition = enchanted(
            item("enchanted_book"),
            &[("sharpness", 1), ("smite", 1)],
            true,
        );
        assert!(state
            .set_input(1, addition.clone(), PLAYER, &mut owner)
            .is_err());
        assert!(state.inputs()[1].is_empty());
        owner.certify_multi = true;
        state.set_input(1, addition, PLAYER, &mut owner)?;
        assert_eq!(state.preview().level_cost, 2);
        assert_eq!(
            entries(&state.preview().stack)?.0,
            vec![(id("sharpness"), 1)]
        );
        state.set_input(
            1,
            enchanted(
                item("enchanted_book"),
                &[("smite", 1), ("sharpness", 1)],
                true,
            ),
            PLAYER,
            &mut owner,
        )?;
        assert_eq!(state.preview().level_cost, 2);
        assert_eq!(entries(&state.preview().stack)?.0, vec![(id("smite"), 1)]);
        Ok(())
    }
}
