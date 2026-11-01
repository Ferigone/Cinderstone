//! Java 1.21 cartography result preparation and owned input consumption.
//! Saved maps, their IDs and Item.onCraftedBy remain with the actual world host.
use crate::{
    inventory::{self, Component, ItemStack},
    protocol::{Reader, Writer},
};
use anyhow::{ensure, Result};

pub const MENU_TYPE: i32 = 23;
pub const RESULT_SLOT: usize = 2;
pub const PLAYER_SLOTS: std::ops::Range<usize> = 3..39;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CartographyMapData {
    pub scale: i8,
    pub locked: bool,
}

/// This must borrow the captured level's MapItemSavedData owner. An unavailable
/// owner is an error; None means an actual lookup found no saved data.
pub trait CartographyMaps {
    fn map_data(&self, map_id: i32) -> Result<Option<CartographyMapData>>;
}

/// The outer menu/world transaction supplies the real Item.onCraftedBy callback
/// (including MapItem.onCraftedPostProcess, new ID allocation and saved-map
/// mutation). It must roll back its own changes on error. No default/no-op host
/// is supplied. The consumed input snapshot is the source callback phase.
pub trait CartographyHost: CartographyMaps {
    fn on_crafted_by(
        &mut self,
        result: &mut ItemStack,
        consumed_inputs: &[ItemStack; 2],
    ) -> Result<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CartographyState {
    inputs: [ItemStack; 2],
    result: ItemStack,
    last_sound_time: i64,
}
/// Source broadcastChanges boundary; the result remains a derived preview.
#[derive(Clone, Debug)]
pub struct CartographyBroadcast {
    pub inputs: [ItemStack; 2],
    pub result: ItemStack,
}
#[derive(Debug)]
pub struct CartographyTake {
    pub stack: ItemStack,
    /// After Item.onCraftedBy, play UI_CARTOGRAPHY_TABLE_TAKE_RESULT at the
    /// captured block (BLOCKS, volume/pitch 1), then Slot.onTake/setChanged.
    pub play_sound: bool,
}
impl Default for CartographyState {
    fn default() -> Self {
        Self {
            inputs: std::array::from_fn(|_| ItemStack::empty()),
            result: ItemStack::empty(),
            last_sound_time: 0,
        }
    }
}
impl CartographyState {
    pub fn inputs(&self) -> &[ItemStack; 2] {
        &self.inputs
    }
    pub fn result(&self) -> &ItemStack {
        &self.result
    }
    pub fn may_place(slot: usize, stack: &ItemStack) -> bool {
        if stack.is_empty() {
            return slot < 2;
        }
        match (slot, inventory::item_name(stack.item_id)) {
            (0, Some("filled_map")) => true,
            (1, Some("paper" | "map" | "glass_pane")) => true,
            _ => false,
        }
    }
    pub fn quick_move_target(
        slot: usize,
        stack: &ItemStack,
    ) -> Option<(std::ops::Range<usize>, bool)> {
        if slot == RESULT_SLOT {
            return Some((3..39, true));
        }
        if slot < RESULT_SLOT {
            return Some((3..39, false));
        }
        if slot >= 39 {
            return None;
        }
        for input in 0..2 {
            if Self::may_place(input, stack) {
                return Some((input..input + 1, false));
            }
        }
        if slot < 30 {
            Some((30..39, false))
        } else {
            Some((3..30, false))
        }
    }
    pub fn set_input(
        &mut self,
        slot: usize,
        stack: ItemStack,
        maps: &impl CartographyMaps,
    ) -> Result<ItemStack> {
        let mut next = self.clone();
        let previous = next.set_input_raw(slot, stack)?;
        next.slots_changed(maps)?;
        *self = next;
        Ok(previous)
    }
    /// Raw source ItemStack mutation; no container/Slot.setChanged callback.
    /// The adapter must invoke slots_changed_observed at the source boundary.
    pub fn set_input_raw(&mut self, slot: usize, stack: ItemStack) -> Result<ItemStack> {
        ensure!(
            slot < 2 && Self::may_place(slot, &stack),
            "invalid cartography input slot/item"
        );
        stack.encode_canonical()?;
        Ok(std::mem::replace(&mut self.inputs[slot], stack))
    }
    /// The actual ResultContainer.setItem/removeItem operation, not another
    /// preview owner. removeItem ignores its requested amount in Java.
    pub fn set_result_raw(&mut self, stack: ItemStack) -> Result<ItemStack> {
        stack.encode_canonical()?;
        Ok(std::mem::replace(&mut self.result, stack))
    }
    pub fn remove_result_raw(&mut self) -> ItemStack {
        std::mem::take(&mut self.result)
    }
    pub fn slots_changed(&mut self, maps: &impl CartographyMaps) -> Result<()> {
        self.slots_changed_observed(maps).map(|_| ())
    }
    /// cqb.setupResultSlot broadcasts the OLD preview before comparing the
    /// desired valid result, then broadcasts again only if it changes. Missing
    /// saved data returns without either boundary; empty inputs remove the
    /// preview without broadcasting. Input/result containers use this same hook.
    pub fn slots_changed_observed(
        &mut self,
        maps: &impl CartographyMaps,
    ) -> Result<Vec<CartographyBroadcast>> {
        if self.inputs.iter().any(ItemStack::is_empty) {
            self.result = ItemStack::empty();
            return Ok(Vec::new());
        }
        let Some(map_id) = map_id(&self.inputs[0])? else {
            return Ok(Vec::new());
        };
        // Source setupResultSlot returns immediately for a genuine missing map,
        // retaining the current result. Do not invent a map record here.
        let Some(data) = maps.map_data(map_id)? else {
            return Ok(Vec::new());
        };
        let mut output = self.inputs[0].clone();
        match inventory::item_name(self.inputs[1].item_id) {
            Some("paper") if !data.locked && data.scale < 4 => {
                output.count = 1;
                set_int_component(&mut output, 28, 1); // MapPostProcessing.SCALE
            }
            Some("glass_pane") if !data.locked => {
                output.count = 1;
                set_int_component(&mut output, 28, 0); // MapPostProcessing.LOCK
            }
            Some("map") => output.count = 2,
            _ => output = ItemStack::empty(),
        }
        let mut broadcasts = Vec::new();
        if !output.is_empty() {
            broadcasts.push(self.broadcast_snapshot());
            if self.result.count != output.count
                || !self.result.same_item_same_components(&output)?
            {
                self.result = output;
                broadcasts.push(self.broadcast_snapshot());
            }
        } else {
            self.result = ItemStack::empty();
            broadcasts.push(self.broadcast_snapshot());
        }
        Ok(broadcasts)
    }
    fn broadcast_snapshot(&self) -> CartographyBroadcast {
        CartographyBroadcast {
            inputs: self.inputs.clone(),
            result: self.result.clone(),
        }
    }
    /// Slot.onTake removes one map, then one addition, each with its own input
    /// container callback. The caller owns the actual taken argument, which may
    /// be EMPTY after full quick-move or a partial transfer remainder.
    pub fn consume_take_inputs_observed(
        &mut self,
        maps: &impl CartographyMaps,
    ) -> Result<Vec<CartographyBroadcast>> {
        let mut broadcasts = Vec::new();
        for slot in 0..2 {
            if !self.inputs[slot].is_empty() {
                shrink_one(&mut self.inputs[slot]);
                broadcasts.extend(self.slots_changed_observed(maps)?);
            }
        }
        Ok(broadcasts)
    }
    /// Call after actual Item.onCraftedBy and before base Slot.onTake.
    pub fn take_sound(&mut self, game_time: i64) -> bool {
        let play = self.last_sound_time != game_time;
        if play {
            self.last_sound_time = game_time;
        }
        play
    }
    /// The caller must reserve the complete result destination and use its
    /// outer world transaction. The callback receives the actual output after
    /// both inputs have been consumed and before the sound/Slot.onTake phase.
    pub fn take_result(
        &mut self,
        host: &mut impl CartographyHost,
        game_time: i64,
    ) -> Result<Option<CartographyTake>> {
        if self.result.is_empty() {
            return Ok(None);
        }
        let mut next = self.clone();
        let mut stack = std::mem::take(&mut next.result);
        next.consume_take_inputs_observed(host)?;
        host.on_crafted_by(&mut stack, &next.inputs)?;
        let play_sound = next.take_sound(game_time);
        next.slots_changed(host)?;
        *self = next;
        Ok(Some(CartographyTake { stack, play_sound }))
    }
    /// Run after the real cursor removal callback. Discard the derived result,
    /// then return/drop the map and addition in input order exactly once.
    pub fn drain_inputs(&mut self) -> [ItemStack; 2] {
        self.result = ItemStack::empty();
        std::mem::replace(
            &mut self.inputs,
            std::array::from_fn(|_| ItemStack::empty()),
        )
    }
}
fn map_id(stack: &ItemStack) -> Result<Option<i32>> {
    let Some(bytes) = stack.effective_component_bytes(26) else {
        return Ok(None);
    };
    let mut reader = Reader::new(bytes);
    let id = reader.varint()?;
    reader.finish()?;
    Ok(Some(id))
}
fn set_int_component(stack: &mut ItemStack, id: i32, value: i32) {
    let mut writer = Writer::new();
    writer.varint(value);
    stack.components.retain(|component| component.type_id != id);
    stack.removed_components.retain(|removed| *removed != id);
    stack.components.push(Component {
        type_id: id,
        data: writer.into_inner(),
    });
}
fn shrink_one(stack: &mut ItemStack) {
    stack.count -= 1;
    if stack.count <= 0 {
        *stack = ItemStack::empty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::RecipeBook;
    struct Host {
        fail: bool,
        calls: usize,
    }
    impl CartographyMaps for Host {
        fn map_data(&self, id: i32) -> Result<Option<CartographyMapData>> {
            ensure!(id == 5, "unexpected map lookup");
            Ok(Some(CartographyMapData {
                scale: 2,
                locked: false,
            }))
        }
    }
    impl CartographyHost for Host {
        fn on_crafted_by(&mut self, result: &mut ItemStack, inputs: &[ItemStack; 2]) -> Result<()> {
            ensure!(!self.fail, "unavailable real saved-map transaction");
            self.calls += 1;
            assert_eq!(inputs[0].count, 1);
            assert_eq!(inputs[1].count, 1);
            // This flow duplicates a map without a postprocessing marker;
            // actual MapItem.onCraftedPostProcess therefore performs no write.
            assert!(result.effective_component_bytes(28).is_none());
            Ok(())
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    #[test]
    fn failed_owner_leaves_inputs_and_result_then_duplicate_consumes_once() -> Result<()> {
        let mut host = Host {
            fail: true,
            calls: 0,
        };
        let mut state = CartographyState::default();
        let mut map = item("filled_map", 2);
        set_int_component(&mut map, 26, 5);
        state.set_input(0, map, &host)?;
        state.set_input(1, item("map", 2), &host)?;
        let before_inputs = state.inputs().clone();
        let before_result = state.result().clone();
        assert_eq!(before_result.count, 2);
        assert!(state.take_result(&mut host, 8).is_err());
        assert_eq!(*state.inputs(), before_inputs);
        assert_eq!(*state.result(), before_result);
        host.fail = false;
        let taken = state.take_result(&mut host, 8)?.unwrap();
        assert_eq!(taken.stack.count, 2);
        assert!(taken.play_sound);
        assert_eq!(host.calls, 1);
        let owned = state.drain_inputs();
        assert_eq!(owned[0].count, 1);
        assert_eq!(owned[1].count, 1);
        assert!(state.result().is_empty());
        Ok(())
    }
    #[test]
    fn scale_and_lock_require_saved_map_state() -> Result<()> {
        let host = Host {
            fail: false,
            calls: 0,
        };
        let mut state = CartographyState::default();
        let mut map = item("filled_map", 1);
        set_int_component(&mut map, 26, 5);
        state.set_input(0, map, &host)?;
        state.set_input(1, item("paper", 1), &host)?;
        assert_eq!(
            state.result().effective_component_bytes(28),
            Some([1].as_slice())
        );
        state.set_input(1, item("glass_pane", 1), &host)?;
        assert_eq!(
            state.result().effective_component_bytes(28),
            Some([0].as_slice())
        );
        Ok(())
    }
}
