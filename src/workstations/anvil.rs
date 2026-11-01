//! Bounded Java 1.21 anvil result computation.
//!
//! Inputs are owned by the menu; `AnvilPreview` is derived and must never be
//! returned on close. Enchantment merging is an explicit unsupported boundary.
//! The host supplies actual item repair rules and hover-name resolution rather
//! than a second native copy of item/translation/registry ownership.

use crate::{
    inventory::{Component, ItemStack},
    nbt::{JavaString, Tag},
    protocol::{Reader, Writer},
};
use anyhow::{bail, ensure, Result};

/// Actual item and text owners consulted by `AnvilMenu.createResult`.
pub trait AnvilContext {
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

/// Compute the no-enchantment-merge branch of AnvilMenu.createResult.
/// Existing enchantments on the base are preserved, including a rename or
/// material repair of an enchanted item. An enchanted sacrifice requires the
/// actual registry/holder-compatible merge owner and is rejected explicitly.
pub fn create_result(
    inputs: &[ItemStack; 2],
    item_name: Option<&JavaString>,
    player: AnvilPlayer,
    context: &mut impl AnvilContext,
) -> Result<AnvilPreview> {
    if let Some(name) = item_name {
        ensure!(
            validate_name(name).as_ref() == Some(name),
            "anvil name was not validated"
        );
    }
    let [base, addition] = inputs;
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
        return Ok(AnvilPreview::empty());
    }
    let mut output = base.clone();
    let base_cost =
        i64::from(int_component(base, 16, 0)?) + i64::from(int_component(addition, 16, 0)?);
    let mut operation_cost: i32 = 0;
    let mut rename_cost: i32 = 0;
    let mut repair_material_count = 0;
    if !addition.is_empty() {
        if output.is_damageable()? && context.is_valid_repair_item(base, addition)? {
            let max_damage = int_component(&output, 2, 0)?;
            let mut damage = output.damage_value()?;
            let mut repaired = damage.min(max_damage / 4);
            if repaired <= 0 {
                return Ok(AnvilPreview::empty());
            }
            while repaired > 0 && repair_material_count < addition.count {
                damage = damage.wrapping_sub(repaired);
                set_int_component(&mut output, 3, damage.clamp(0, max_damage));
                operation_cost = operation_cost.wrapping_add(1);
                repair_material_count += 1;
                repaired = output.damage_value()?.min(max_damage / 4);
            }
        } else {
            // The source tests presence of STORED_ENCHANTMENTS, not nonempty
            // contents or the enchanted-book item ID, for its book branch.
            let stored = addition.effective_component_bytes(23).is_some();
            if !stored && (base.item_id != addition.item_id || !output.is_damageable()?) {
                return Ok(AnvilPreview::empty());
            }
            let addition_enchantments = if addition.item_id == enchanted_book {
                23
            } else {
                9
            };
            if stored || has_enchantments(addition, addition_enchantments)? {
                bail!("anvil enchantment combining requires actual enchantment registry/holder ownership");
            }
            if output.is_damageable()? {
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
    }
    Ok(AnvilPreview {
        stack: output,
        level_cost,
        repair_material_count,
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
        let preview = create_result(&self.inputs, self.item_name.as_ref(), player, context)?;
        self.revision = self.revision.wrapping_add(1);
        self.preview = preview;
        Ok(())
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
