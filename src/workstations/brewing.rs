//! Java 1.21 BrewingStandBlockEntity tick ownership and menu slot rules.
//!
//! This engine borrows the actual Level's PotionBrewing owner. It does not
//! substitute guessed potion recipes, component data, a scheduler or a world.
use crate::{crafting::RecipeBook, inventory::ItemStack, smelting};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

pub const MENU_TYPE: i32 = 11;
pub const SLOT_COUNT: usize = 5;
pub const INGREDIENT_SLOT: usize = 3;
pub const FUEL_SLOT: usize = 4;
pub const BREW_TICKS: i32 = 400;
pub const FUEL_USES: i32 = 20;

/// Actual world-owned PotionBrewing recipe/container lists and feature flags.
/// `mix` takes the ingredient first, as the original method does; it must handle
/// all three bottle slots, including unchanged or empty inputs, and reproduce
/// the source's component construction rather than copying arbitrary patches.
pub trait PotionBrewingOwner {
    fn is_ingredient(&self, ingredient: &ItemStack) -> Result<bool>;
    fn has_mix(&self, bottle: &ItemStack, ingredient: &ItemStack) -> Result<bool>;
    fn mix(&self, ingredient: &ItemStack, bottle: &ItemStack) -> Result<ItemStack>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrewingState {
    slots: [ItemStack; SLOT_COUNT],
    brew_time: i32,
    fuel: i32,
    ingredient_item: Option<i32>,
    last_bottle_bits: Option<[bool; 3]>,
}

/// Only these values are persisted by Java, in addition to base-container data.
/// The ingredient cache is reconstructed on load; lastBottleBits is transient.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrewingSaved {
    pub slots: [ItemStack; SLOT_COUNT],
    pub brew_time: i16,
    pub fuel: i8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrewingFace {
    Up,
    Down,
    Side,
}

/// Preserve callback ordering and the exact container seen by each dirty call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrewingEffect {
    SetChanged {
        slots: [ItemStack; SLOT_COUNT],
        data: [i32; 2],
    },
    /// Containers.dropItemStack at integer block x/y/z, with the actual Level
    /// RNG, independent Item RNG factories and shared entity ID owner.
    DropIngredientRemainder(ItemStack),
    LevelEvent {
        id: i32,
        data: i32,
    },
    /// Set all HAS_BOTTLE properties on the captured state, flags 2.
    SetBottleProperties {
        present: [bool; 3],
        flags: i32,
    },
}

/// The actual block/world owner must stage every effect in order, preserving
/// captured block-entity identity and all item/RNG/entity owners. The caller
/// must bind the plan to that actual owner: structural state equality cannot
/// distinguish an identical newly placed container at the same coordinates.
/// An error must publish
/// neither the callbacks nor the new state. No-op implementations are only
/// suitable for explicitly labeled isolated kernel checks.
pub trait BrewingTickHost {
    fn commit_brewing_effects(&mut self, effects: &[BrewingEffect]) -> Result<()>;
}

#[derive(Debug)]
pub struct PreparedBrewingTick {
    before: BrewingState,
    after: BrewingState,
    effects: Vec<BrewingEffect>,
}
impl PreparedBrewingTick {
    pub fn next_state(&self) -> &BrewingState {
        &self.after
    }
    pub fn effects(&self) -> &[BrewingEffect] {
        &self.effects
    }
}

impl BrewingState {
    pub fn slots(&self) -> &[ItemStack; SLOT_COUNT] {
        &self.slots
    }
    /// ContainerData indices 0 and 1, sent as signed shorts by the menu.
    pub fn data(&self) -> [i32; 2] {
        [self.brew_time, self.fuel]
    }
    pub fn save_values(&self) -> BrewingSaved {
        BrewingSaved {
            slots: self.slots.clone(),
            brew_time: self.brew_time as i16,
            fuel: self.fuel as i8,
        }
    }
    pub fn from_saved(saved: BrewingSaved) -> Self {
        let ingredient_item = (saved.brew_time > 0).then(|| {
            let stack = &saved.slots[INGREDIENT_SLOT];
            if stack.is_empty() {
                0
            } else {
                stack.item_id
            }
        });
        Self {
            slots: saved.slots.map(canonical),
            brew_time: i32::from(saved.brew_time),
            fuel: i32::from(saved.fuel),
            ingredient_item,
            last_bottle_bits: None,
        }
    }

    /// BaseContainerBlockEntity.setItem clamps the count but does not apply a
    /// menu/automation predicate. Caller commits the returned dirty effect with
    /// the actual container/world transaction; ingredient changes abort on tick.
    pub fn set_slot(&mut self, slot: usize, mut stack: ItemStack) -> Result<BrewingEffect> {
        ensure!(slot < SLOT_COUNT, "invalid brewing slot");
        if !stack.is_empty() {
            stack.count = stack.count.min(stack.max_stack_size()?.min(99));
        }
        self.slots[slot] = canonical(stack);
        Ok(self.dirty_effect())
    }

    pub fn menu_may_place(
        slot: usize,
        stack: &ItemStack,
        brewing: &impl PotionBrewingOwner,
    ) -> Result<bool> {
        ensure!(slot < SLOT_COUNT, "invalid brewing menu slot");
        if stack.is_empty() {
            return Ok(false);
        }
        match slot {
            0..=2 => Ok(is_bottle(stack)),
            INGREDIENT_SLOT => brewing.is_ingredient(stack),
            FUEL_SLOT => Ok(is_item(stack, "blaze_powder")),
            _ => unreachable!(),
        }
    }
    pub fn menu_slot_limit(slot: usize, stack: &ItemStack) -> Result<i32> {
        ensure!(slot < SLOT_COUNT, "invalid brewing menu slot");
        Ok(if slot < 3 {
            1
        } else {
            stack.max_stack_size()?.min(99)
        })
    }
    pub fn can_place_in_block(
        &self,
        slot: usize,
        stack: &ItemStack,
        brewing: &impl PotionBrewingOwner,
    ) -> Result<bool> {
        ensure!(slot < SLOT_COUNT, "invalid brewing block slot");
        Ok(Self::menu_may_place(slot, stack, brewing)?
            && (slot >= 3 || self.slots[slot].is_empty()))
    }
    pub fn slots_for_face(face: BrewingFace) -> &'static [usize] {
        match face {
            BrewingFace::Up => &[3],
            BrewingFace::Down => &[0, 1, 2, 3],
            BrewingFace::Side => &[0, 1, 2, 4],
        }
    }
    /// The face's exposed-slot list is a separate check by the transfer owner.
    pub fn can_take_through_face(slot: usize, stack: &ItemStack) -> Result<bool> {
        ensure!(slot < SLOT_COUNT, "invalid brewing extraction slot");
        Ok(slot != INGREDIENT_SLOT || is_item(stack, "glass_bottle"))
    }
    pub fn is_brewable(&self, brewing: &impl PotionBrewingOwner) -> Result<bool> {
        let ingredient = &self.slots[INGREDIENT_SLOT];
        if ingredient.is_empty() || !brewing.is_ingredient(ingredient)? {
            return Ok(false);
        }
        for bottle in &self.slots[..3] {
            if !bottle.is_empty() && brewing.has_mix(bottle, ingredient)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `captured_block_is_brewing_stand` comes from the ticker's actual state,
    /// not a guessed block at the same coordinates or a newly placed container.
    pub fn prepare_tick(
        &self,
        brewing: &impl PotionBrewingOwner,
        captured_block_is_brewing_stand: bool,
    ) -> Result<PreparedBrewingTick> {
        let mut next = self.clone();
        let mut effects = Vec::new();
        // Fuel loads even when there is currently no valid brewing recipe.
        if next.fuel <= 0 && is_item(&next.slots[FUEL_SLOT], "blaze_powder") {
            next.fuel = FUEL_USES;
            shrink(&mut next.slots[FUEL_SLOT], 1);
            effects.push(next.dirty_effect());
        }
        let brewable = next.is_brewable(brewing)?;
        if next.brew_time > 0 {
            next.brew_time -= 1;
            // Completion precedes the ingredient-identity cancellation branch.
            if next.brew_time == 0 && brewable {
                next.do_brew(brewing, &mut effects)?;
            } else {
                let current_item = if next.slots[INGREDIENT_SLOT].is_empty() {
                    0
                } else {
                    next.slots[INGREDIENT_SLOT].item_id
                };
                if !brewable || Some(current_item) != next.ingredient_item {
                    next.brew_time = 0;
                }
            }
            // The original marks changed on EVERY active tick, not just an
            // ingredient abort or the completion tick.
            effects.push(next.dirty_effect());
        } else if brewable && next.fuel > 0 {
            next.fuel -= 1;
            next.brew_time = BREW_TICKS;
            next.ingredient_item = Some(next.slots[INGREDIENT_SLOT].item_id);
            effects.push(next.dirty_effect());
        }
        let present = std::array::from_fn(|i| !next.slots[i].is_empty());
        if next.last_bottle_bits != Some(present) {
            next.last_bottle_bits = Some(present);
            if captured_block_is_brewing_stand {
                effects.push(BrewingEffect::SetBottleProperties { present, flags: 2 });
            }
        }
        Ok(PreparedBrewingTick {
            before: self.clone(),
            after: next,
            effects,
        })
    }
    pub fn commit_tick(
        &mut self,
        tick: PreparedBrewingTick,
        host: &mut impl BrewingTickHost,
    ) -> Result<()> {
        ensure!(
            *self == tick.before,
            "brewing state changed after tick preparation"
        );
        host.commit_brewing_effects(&tick.effects)?;
        *self = tick.after;
        Ok(())
    }
    fn dirty_effect(&self) -> BrewingEffect {
        BrewingEffect::SetChanged {
            slots: self.slots.clone(),
            data: self.data(),
        }
    }
    fn do_brew(
        &mut self,
        brewing: &impl PotionBrewingOwner,
        effects: &mut Vec<BrewingEffect>,
    ) -> Result<()> {
        let ingredient = self.slots[INGREDIENT_SLOT].clone();
        for slot in 0..3 {
            self.slots[slot] = canonical(brewing.mix(&ingredient, &self.slots[slot])?);
        }
        // Source queries getItem().hasCraftingRemainingItem AFTER shrinking.
        // An exhausted stack's getItem() is AIR, so the final dragon breath
        // produces no bottle remainder in this exact 1.21 artifact.
        shrink(&mut self.slots[INGREDIENT_SLOT], 1);
        // The pinned catalog covers every item's static craftingRemainingItem;
        // this helper returns empty for the post-shrink empty/AIR stack.
        let remainder = smelting::fuel_remainder(&self.slots[INGREDIENT_SLOT])?;
        if !remainder.is_empty() {
            if self.slots[INGREDIENT_SLOT].is_empty() {
                self.slots[INGREDIENT_SLOT] = remainder;
            } else {
                effects.push(BrewingEffect::DropIngredientRemainder(remainder));
            }
        }
        effects.push(BrewingEffect::LevelEvent { id: 1035, data: 0 });
        Ok(())
    }
}

/// BrewingStandMenu.PotionSlot.onTake triggers BREWED_POTION only for a present
/// potion holder and an actual ServerPlayer, before the ordinary Slot.onTake.
/// This returns its holder ID; the player/advancement owner performs the trigger.
pub fn potion_take_criterion(stack: &ItemStack) -> Result<Option<i32>> {
    let Some(contents) = stack.component_value(31)? else {
        return Ok(None);
    };
    contents
        .get("potionId")
        .filter(|value| !value.is_null())
        .map(|value| {
            let id = value
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("invalid potion holder component"))?;
            Ok(i32::try_from(id)?)
        })
        .transpose()
}
fn is_item(stack: &ItemStack, name: &str) -> bool {
    !stack.is_empty() && Some(stack.item_id) == RecipeBook::vanilla().item_id(name)
}
fn is_bottle(stack: &ItemStack) -> bool {
    [
        "potion",
        "splash_potion",
        "lingering_potion",
        "glass_bottle",
    ]
    .iter()
    .any(|name| is_item(stack, name))
}
fn shrink(stack: &mut ItemStack, amount: i32) {
    stack.count -= amount;
    if stack.is_empty() {
        *stack = ItemStack::empty();
    }
}
fn canonical(stack: ItemStack) -> ItemStack {
    if stack.is_empty() {
        ItemStack::empty()
    } else {
        stack
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    // Explicit injected fixture owner: these engine checks do not claim a
    // default potion catalog, component assembly, or actual world integration.
    #[derive(Default)]
    struct FixtureMixes {
        calls: Cell<usize>,
    }
    impl PotionBrewingOwner for FixtureMixes {
        fn is_ingredient(&self, ingredient: &ItemStack) -> Result<bool> {
            Ok(["sugar", "gunpowder", "dragon_breath"]
                .iter()
                .any(|name| is_item(ingredient, name)))
        }
        fn has_mix(&self, bottle: &ItemStack, _: &ItemStack) -> Result<bool> {
            Ok(is_item(bottle, "potion"))
        }
        fn mix(&self, _: &ItemStack, bottle: &ItemStack) -> Result<ItemStack> {
            self.calls.set(self.calls.get() + 1);
            Ok(if is_item(bottle, "potion") {
                item("splash_potion", 1)
            } else {
                bottle.clone()
            })
        }
    }
    #[derive(Default)]
    struct FixtureWorld {
        events: Vec<BrewingEffect>,
        reject: bool,
    }
    impl BrewingTickHost for FixtureWorld {
        fn commit_brewing_effects(&mut self, effects: &[BrewingEffect]) -> Result<()> {
            ensure!(!self.reject, "fixture world refuses transaction");
            self.events = effects.to_vec();
            Ok(())
        }
    }
    fn tick(state: &mut BrewingState, mixes: &FixtureMixes, host: &mut FixtureWorld) -> Result<()> {
        state.commit_tick(state.prepare_tick(mixes, true)?, host)
    }

    #[test]
    fn idle_refuel_start_countdown_abort_and_final_tick_ingredient_change() -> Result<()> {
        let mixes = FixtureMixes::default();
        let mut host = FixtureWorld::default();
        let mut state = BrewingState::default();
        let _ = state.set_slot(FUEL_SLOT, item("blaze_powder", 2))?;
        tick(&mut state, &mixes, &mut host)?;
        assert_eq!(state.data(), [0, 20]);
        assert_eq!(state.slots()[4].count, 1);
        assert!(matches!(
            host.events.as_slice(),
            [
                BrewingEffect::SetChanged { data: [0, 20], .. },
                BrewingEffect::SetBottleProperties {
                    present: [false, false, false],
                    flags: 2
                }
            ]
        ));
        let _ = state.set_slot(0, item("potion", 1))?;
        let _ = state.set_slot(3, item("sugar", 2))?;
        tick(&mut state, &mixes, &mut host)?;
        assert_eq!(state.data(), [400, 19]);
        let _ = state.set_slot(3, item("gunpowder", 2))?;
        tick(&mut state, &mixes, &mut host)?;
        assert_eq!(state.data(), [0, 19]); // no fuel refund after abort
        let _ = state.set_slot(3, item("sugar", 2))?;
        tick(&mut state, &mixes, &mut host)?;
        assert_eq!(state.data(), [400, 18]);
        for _ in 0..399 {
            tick(&mut state, &mixes, &mut host)?;
        }
        assert_eq!(state.data(), [1, 18]);
        let _ = state.set_slot(3, item("gunpowder", 2))?;
        tick(&mut state, &mixes, &mut host)?;
        // Java checks completion before ingredient-identity cancellation.
        assert_eq!(state.data(), [0, 18]);
        assert!(is_item(&state.slots()[0], "splash_potion"));
        assert_eq!(state.slots()[3].count, 1);
        assert_eq!(mixes.calls.get(), 3); // includes both empty bottle slots
        assert!(matches!(
            host.events.as_slice(),
            [
                BrewingEffect::LevelEvent { id: 1035, data: 0 },
                BrewingEffect::SetChanged { .. }
            ]
        ));
        Ok(())
    }

    #[test]
    fn remainder_effect_order_save_restore_and_atomic_world_commit() -> Result<()> {
        let mixes = FixtureMixes::default();
        let mut host = FixtureWorld::default();
        let mut slots = std::array::from_fn(|_| ItemStack::empty());
        slots[0] = item("potion", 1);
        slots[3] = item("dragon_breath", 2);
        let mut state = BrewingState::from_saved(BrewingSaved {
            slots,
            brew_time: 1,
            fuel: 7,
        });
        let before = state.clone();
        let plan = state.prepare_tick(&mixes, true)?;
        assert!(
            matches!(plan.effects(), [BrewingEffect::DropIngredientRemainder(stack), BrewingEffect::LevelEvent { id: 1035, data: 0 }, BrewingEffect::SetChanged { .. }, BrewingEffect::SetBottleProperties { .. }] if is_item(stack, "glass_bottle"))
        );
        host.reject = true;
        assert!(state.commit_tick(plan, &mut host).is_err());
        assert_eq!(state, before);
        host.reject = false;
        tick(&mut state, &mixes, &mut host)?;
        assert_eq!(state.slots()[3].count, 1);
        let saved = state.save_values();
        assert_eq!(saved.brew_time, 0);
        assert_eq!(saved.fuel, 7);
        // Java queries the post-shrink item: an exhausted stack resolves to AIR.
        let mut saved = saved;
        saved.slots[0] = item("potion", 1);
        saved.brew_time = 1;
        state = BrewingState::from_saved(saved);
        tick(&mut state, &mixes, &mut host)?;
        assert!(state.slots()[3].is_empty());
        assert!(!host
            .events
            .iter()
            .any(|event| matches!(event, BrewingEffect::DropIngredientRemainder(_))));
        let stale = state.prepare_tick(&mixes, true)?;
        let _ = state.set_slot(4, item("blaze_powder", 1))?;
        assert!(state.commit_tick(stale, &mut host).is_err());
        assert_eq!(
            BrewingState::slots_for_face(BrewingFace::Down),
            &[0, 1, 2, 3]
        );
        assert!(BrewingState::can_take_through_face(
            3,
            &item("glass_bottle", 1)
        )?);
        assert!(!BrewingState::can_take_through_face(
            3,
            &item("dragon_breath", 1)
        )?);
        assert_eq!(
            BrewingState::menu_slot_limit(0, &item("glass_bottle", 64))?,
            1
        );
        Ok(())
    }
}
