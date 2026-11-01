//! The Java 1.21 loom's input/result and selected-pattern state.
//!
//! This is the workstation owner, not a second player inventory. The external
//! menu adapter owns clicks, cursor, synchronization and ContainerLevelAccess.
use crate::{
    inventory::{self, Component, ItemStack},
    protocol::{Reader, Writer},
};
use anyhow::{ensure, Result};

pub const MENU_TYPE: i32 = 18;
pub const RESULT_SLOT: usize = 3;
pub const PLAYER_SLOTS: std::ops::Range<usize> = 4..40;

/// A holder in the actual banner-pattern registry captured by this menu.
/// The host must not substitute a made-up registry order or numeric ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BannerPattern {
    pub registry_id: i32,
}

/// Resolves `no_item_required` or the actual BannerPatternItem's pattern tag.
/// Missing tags yield an empty list. Preserve the source HolderSet encounter
/// order, and retain this registry owner across the menu's lifetime.
pub trait LoomPatterns {
    fn is_pattern_item(&self, stack: &ItemStack) -> bool;
    fn selectable_patterns(&self, template: &ItemStack) -> Result<Vec<BannerPattern>>;
}

#[derive(Clone, Debug)]
pub struct LoomState {
    inputs: [ItemStack; 3],
    result: ItemStack,
    patterns: Vec<BannerPattern>,
    selected: i32,
    // Java's long field starts at zero, including the tick-zero sound guard.
    last_sound_time: i64,
}

#[derive(Debug)]
pub struct LoomTake {
    pub stack: ItemStack,
    /// Publish UI_LOOM_TAKE_RESULT at the captured block, category BLOCKS,
    /// volume/pitch 1, after input changes and before Slot.onTake/setChanged.
    pub play_sound: bool,
}

impl Default for LoomState {
    fn default() -> Self {
        Self {
            inputs: std::array::from_fn(|_| ItemStack::empty()),
            result: ItemStack::empty(),
            patterns: vec![],
            // DataSlot.standalone starts at zero; the first empty-input
            // slotsChanged callback resets it to -1.
            selected: 0,
            last_sound_time: 0,
        }
    }
}

impl LoomState {
    pub fn inputs(&self) -> &[ItemStack; 3] {
        &self.inputs
    }
    pub fn result(&self) -> &ItemStack {
        &self.result
    }
    pub fn selected_pattern(&self) -> i32 {
        self.selected
    }
    pub fn selectable_patterns(&self) -> &[BannerPattern] {
        &self.patterns
    }

    pub fn may_place(slot: usize, stack: &ItemStack, registry: &impl LoomPatterns) -> bool {
        if stack.is_empty() {
            return slot < 3;
        }
        match slot {
            0 => inventory::item_name(stack.item_id).is_some_and(|name| name.ends_with("_banner")),
            1 => dye_color(stack).is_some(),
            2 => registry.is_pattern_item(stack),
            _ => false,
        }
    }

    /// Source quickMoveStack's one target range. Input/output coordinates are
    /// external menu coordinates; the caller supplies actual shared inventory.
    pub fn quick_move_target(
        slot: usize,
        stack: &ItemStack,
        registry: &impl LoomPatterns,
    ) -> Option<(std::ops::Range<usize>, bool)> {
        if slot == RESULT_SLOT {
            return Some((4..40, true));
        }
        if slot < RESULT_SLOT {
            return Some((4..40, false));
        }
        if slot >= 40 {
            return None;
        }
        for input in 0..3 {
            if Self::may_place(input, stack, registry) {
                return Some((input..input + 1, false));
            }
        }
        if slot < 31 {
            Some((31..40, false))
        } else {
            Some((4..31, false))
        }
    }

    /// The caller transfers ownership into this input and receives the old
    /// stack. A failed registry/component lookup leaves this state unchanged.
    pub fn set_input(
        &mut self,
        slot: usize,
        stack: ItemStack,
        registry: &impl LoomPatterns,
    ) -> Result<ItemStack> {
        ensure!(
            Self::may_place(slot, &stack, registry),
            "invalid loom input slot/item"
        );
        stack.encode_canonical()?;
        let mut next = self.clone();
        let previous = std::mem::replace(&mut next.inputs[slot], stack);
        next.update_result(registry)?;
        *self = next;
        Ok(previous)
    }

    /// Source clickMenuButton only checks list bounds. In particular, this
    /// method does not invent a second six-layer check absent from that method.
    pub fn select_pattern(&mut self, index: i32) -> Result<bool> {
        if index < 0 || index as usize >= self.patterns.len() {
            return Ok(false);
        }
        let mut next = self.clone();
        next.selected = index;
        next.setup_result(next.patterns[index as usize])?;
        *self = next;
        Ok(true)
    }

    pub fn slots_changed(&mut self, registry: &impl LoomPatterns) -> Result<()> {
        let mut next = self.clone();
        next.update_result(registry)?;
        *self = next;
        Ok(())
    }

    fn update_result(&mut self, registry: &impl LoomPatterns) -> Result<()> {
        if self.inputs[0].is_empty() || self.inputs[1].is_empty() {
            self.result = ItemStack::empty();
            self.patterns.clear();
            self.selected = -1;
            return Ok(());
        }
        let previous_pattern = usize::try_from(self.selected)
            .ok()
            .and_then(|index| self.patterns.get(index))
            .copied();
        let patterns = registry.selectable_patterns(&self.inputs[2])?;
        ensure!(
            patterns.iter().all(|pattern| pattern.registry_id >= 0),
            "unregistered loom pattern holder"
        );
        self.patterns = patterns;
        let chosen = if self.patterns.len() == 1 {
            self.selected = 0;
            self.patterns.first().copied()
        } else if let Some(pattern) = previous_pattern {
            if let Some(index) = self.patterns.iter().position(|value| *value == pattern) {
                self.selected = index as i32;
                Some(pattern)
            } else {
                self.selected = -1;
                None
            }
        } else {
            self.selected = -1;
            None
        };
        if let Some(pattern) = chosen {
            if layer_count(&self.inputs[0])? >= 6 {
                self.selected = -1;
                self.result = ItemStack::empty();
            } else {
                self.setup_result(pattern)?;
            }
        } else {
            self.result = ItemStack::empty();
        }
        Ok(())
    }

    fn setup_result(&mut self, pattern: BannerPattern) -> Result<()> {
        if self.inputs[0].is_empty() || self.inputs[1].is_empty() {
            self.result = ItemStack::empty();
            return Ok(());
        }
        let color = dye_color(&self.inputs[1])
            .ok_or_else(|| anyhow::anyhow!("loom requires actual DyeItem"))?;
        let mut output = self.inputs[0].clone();
        output.count = 1;
        let count = layer_count(&output)?;
        let mut wire = Writer::new();
        wire.varint(count + 1);
        if let Some(bytes) = output.effective_component_bytes(49) {
            let mut reader = Reader::new(bytes);
            reader.varint()?;
            wire.raw(reader.remaining());
        }
        wire.varint(
            pattern
                .registry_id
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("pattern holder ID overflow"))?,
        )
        .varint(color);
        set_component(&mut output, 49, wire.into_inner());
        output.encode_canonical()?;
        self.result = output;
        Ok(())
    }

    /// The menu adapter must first prove the entire result can enter its
    /// destination. This consumes one banner and one dye, never the template.
    /// `game_time` is the captured level's actual time, not a private counter.
    pub fn take_result(
        &mut self,
        registry: &impl LoomPatterns,
        game_time: i64,
    ) -> Result<Option<LoomTake>> {
        if self.result.is_empty() {
            return Ok(None);
        }
        let mut next = self.clone();
        let stack = std::mem::take(&mut next.result);
        shrink_one(&mut next.inputs[0]);
        next.update_result(registry)?;
        shrink_one(&mut next.inputs[1]);
        next.update_result(registry)?;
        if next.inputs[0].is_empty() || next.inputs[1].is_empty() {
            next.selected = -1;
        }
        let play_sound = next.last_sound_time != game_time;
        if play_sound {
            next.last_sound_time = game_time;
        }
        *self = next;
        Ok(Some(LoomTake { stack, play_sound }))
    }

    /// After the external AbstractContainerMenu cursor callback, return/drop
    /// only these three owned inputs. The output is a derived preview.
    pub fn drain_inputs(&mut self) -> [ItemStack; 3] {
        self.result = ItemStack::empty();
        self.patterns.clear();
        self.selected = -1;
        std::mem::replace(
            &mut self.inputs,
            std::array::from_fn(|_| ItemStack::empty()),
        )
    }
}

fn layer_count(stack: &ItemStack) -> Result<i32> {
    let Some(value) = stack.component_value(49)? else {
        return Ok(0);
    };
    let layers = value
        .get("layers")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("invalid banner-pattern layers"))?;
    Ok(layers.len() as i32)
}

fn dye_color(stack: &ItemStack) -> Option<i32> {
    const COLORS: [&str; 16] = [
        "white",
        "orange",
        "magenta",
        "light_blue",
        "yellow",
        "lime",
        "pink",
        "gray",
        "light_gray",
        "cyan",
        "purple",
        "blue",
        "brown",
        "green",
        "red",
        "black",
    ];
    if stack.is_empty() {
        return None;
    }
    let name = inventory::item_name(stack.item_id)?.strip_suffix("_dye")?;
    COLORS
        .iter()
        .position(|color| *color == name)
        .map(|index| index as i32)
}

fn shrink_one(stack: &mut ItemStack) {
    stack.count -= 1;
    if stack.count <= 0 {
        *stack = ItemStack::empty();
    }
}
fn set_component(stack: &mut ItemStack, id: i32, data: Vec<u8>) {
    stack.components.retain(|component| component.type_id != id);
    stack.removed_components.retain(|removed| *removed != id);
    stack.components.push(Component { type_id: id, data });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::RecipeBook;
    struct Registry;
    impl LoomPatterns for Registry {
        fn is_pattern_item(&self, _: &ItemStack) -> bool {
            false
        }
        fn selectable_patterns(&self, template: &ItemStack) -> Result<Vec<BannerPattern>> {
            ensure!(template.is_empty(), "fixture has no template tag");
            Ok(vec![
                BannerPattern { registry_id: 0 },
                BannerPattern { registry_id: 1 },
            ])
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    #[test]
    fn result_consumes_banner_and_dye_once_without_draining_preview() -> Result<()> {
        let mut state = LoomState::default();
        assert_eq!(state.selected_pattern(), 0);
        state.set_input(0, item("white_banner", 3), &Registry)?;
        state.set_input(1, item("red_dye", 3), &Registry)?;
        assert!(state.result().is_empty());
        assert!(state.select_pattern(1)?);
        assert_eq!(layer_count(state.result())?, 1);
        let output = state.take_result(&Registry, 12)?.unwrap();
        assert!(output.play_sound);
        assert_eq!(output.stack.count, 1);
        assert_eq!(state.inputs()[0].count, 2);
        assert_eq!(state.inputs()[1].count, 2);
        assert_eq!(state.result().count, 1);
        assert!(!state.take_result(&Registry, 12)?.unwrap().play_sound);
        let owned = state.drain_inputs();
        assert_eq!(owned[0].count, 1);
        assert_eq!(owned[1].count, 1);
        assert!(state.result().is_empty());
        assert!(state.drain_inputs().iter().all(ItemStack::is_empty));
        Ok(())
    }
}
