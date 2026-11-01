//! Java 1.21 dispenser/dropper control over captured real inventory/world owners.
//!
//! The nine slots, loot, Level RNG, behavior registry, destination containers
//! and Item entities belong to the host. This module never owns another copy
//! of that inventory. Every mutating entry point requires a staged transaction
//! of those real owners: an error must discard it, including RNG/world effects.

use crate::{
    geometry::{Direction, Vec3},
    inventory::ItemStack,
    random::RandomSource,
};
use anyhow::{ensure, Result};

pub const SLOT_COUNT: usize = 9;
pub const TRIGGER_DELAY: i32 = 4;
pub const DEFAULT_ACCURACY: i32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispenserKind {
    Dispenser,
    Dropper,
}

/// Selected from the actual behavior registry, including its default return
/// value. Registered behavior is an opaque host-owned capability, not an item
/// whitelist or a synthesized projectile/world implementation.
pub enum DispenseBehavior<B> {
    Noop,
    Default,
    Registered(B),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefaultItemSpawn {
    pub position: Vec3,
    pub facing: Direction,
    pub accuracy: i32,
}

/// Implement on one staged borrow of the actual captured block entity, Level,
/// scheduler, behavior registry, source/destination inventories and Item owners.
/// Pure getters are observations; they do not transfer ownership of a stack.
/// No method has an unsupported no-op default. Unavailable owners must error.
pub trait DispenserHost {
    type RegisteredBehavior;
    type Destination;

    /// Bind the real block-entity identity and establish transaction support.
    /// This is native preflight, not an additional Java world callback.
    fn preflight(&mut self, kind: DispenserKind) -> Result<()>;
    /// `false` certifies the exact expected block-entity type is absent. An
    /// unavailable world/owner is an error rather than an empty inventory.
    fn matching_block_entity(&self, kind: DispenserKind) -> Result<bool>;
    /// RandomizableContainerBlockEntity.unpackLootTable(null), before sampling.
    fn unpack_loot(&mut self) -> Result<()>;
    fn items(&self) -> &[ItemStack; SLOT_COUNT];
    fn level_random(&mut self) -> &mut dyn RandomSource;
    fn captured_position(&self) -> (i32, i32, i32);
    fn captured_facing(&self) -> Result<Direction>;
    /// Dropper reads current Level state for its destination, independently of
    /// the scheduled state's facing used by fallback DefaultDispense behavior.
    fn current_facing(&self) -> Result<Direction>;
    fn item_enabled(&self, stack: &ItemStack) -> Result<bool>;
    fn registered_behavior(
        &self,
        stack: &ItemStack,
    ) -> Result<DispenseBehavior<Self::RegisteredBehavior>>;
    /// Execute the actual registered behavior on the canonical selected stack,
    /// including its own item consumption, entity/world effects and callbacks.
    /// Return its replacement stack for the final source setItem call. An
    /// unported selected behavior must error; never silently default-eject it.
    fn execute_registered(
        &mut self,
        slot: usize,
        behavior: Self::RegisteredBehavior,
    ) -> Result<ItemStack>;
    /// Raw ItemStack.split on this canonical slot, without Container.setChanged.
    /// The returned stack has moved out; no second owned copy may be spawned.
    fn split_item_raw(&mut self, slot: usize, amount: i32) -> Result<ItemStack>;
    /// Actual ItemEntity construction, motion and addFreshEntity, in that order.
    /// The Level RNG velocity helper below runs AFTER the Item constructor (and
    /// its actual Item/entity RNG/ID work). Source ignores addFreshEntity's bool;
    /// do not turn that false return into an inventory refund. Unavailable Item,
    /// RNG/ID or world ownership must error before publishing the transaction.
    fn spawn_default_item(&mut self, item: ItemStack, spawn: DefaultItemSpawn) -> Result<()>;
    fn level_event(&mut self, id: i32, data: i32) -> Result<()>;
    /// BLOCK_ACTIVATE at source position, null entity, actual block entity's
    /// stored state as context; only the empty dispenser emits this event.
    fn empty_block_activate(&mut self) -> Result<()>;
    /// Actual HopperBlockEntity.getContainerAt at current-facing neighbor. It
    /// includes real block/entity container lookup and its actual RNG if used.
    fn front_container(&mut self, facing: Direction) -> Result<Option<Self::Destination>>;
    /// Actual HopperBlockEntity.addItem(source, destination, copyWithCount(1),
    /// oppositeFacing). Preserve sided slots/order, predicates, capacities,
    /// hopper cooldown and dirty/comparator callbacks. The one-item proposal
    /// and source decrement are committed together; it is not an extra item.
    /// Return either empty (accepted) or the unchanged one-item remainder.
    fn insert_one(
        &mut self,
        source_slot: usize,
        destination: Self::Destination,
        item: ItemStack,
        incoming_face: Direction,
    ) -> Result<ItemStack>;
    /// Actual RandomizableContainerBlockEntity.setItem: unpack loot (null
    /// player), replace/limit the canonical slot, then setChanged at this source
    /// phase. Invoke real dirty/comparator callbacks exactly once, including a
    /// refused destination insertion. The engine also applies count limits.
    fn set_item_and_changed(&mut self, slot: usize, replacement: ItemStack) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispenseOutcome {
    MissingBlockEntity,
    Empty,
    Noop { slot: usize },
    Dispensed { slot: usize },
    Inserted { slot: usize },
    DestinationRefused { slot: usize },
}

/// Reservoir sampling in ascending slot order. Even the first occupied slot
/// consumes nextInt(1); stack counts do not weight selection. Caller must first
/// resolve actual loot and borrow the Level's real RNG, never a fresh local RNG.
pub fn random_slot(
    items: &[ItemStack; SLOT_COUNT],
    random: &mut (impl RandomSource + ?Sized),
) -> Result<Option<usize>> {
    let mut selected = None;
    let mut occupied = 1;
    for (slot, stack) in items.iter().enumerate() {
        if !stack.is_empty() {
            if random.next_bounded_i32(occupied)? == 0 {
                selected = Some(slot);
            }
            occupied += 1;
        }
    }
    Ok(selected)
}

/// The scheduled BLOCK callback dispenses unconditionally. It does not cancel
/// when power/triggered changed during the four-tick delay.
pub fn scheduled_tick(
    kind: DispenserKind,
    host: &mut impl DispenserHost,
) -> Result<DispenseOutcome> {
    host.preflight(kind)?;
    if !host.matching_block_entity(kind)? {
        return Ok(DispenseOutcome::MissingBlockEntity);
    }
    host.unpack_loot()?;
    // Observational snapshot avoids aliasing the host's RNG and canonical slots.
    let items = host.items().clone();
    let Some(slot) = random_slot(&items, host.level_random())? else {
        host.level_event(1001, 0)?;
        if kind == DispenserKind::Dispenser {
            host.empty_block_activate()?;
        }
        return Ok(DispenseOutcome::Empty);
    };
    let selected = host.items()[slot].clone();
    if kind == DispenserKind::Dropper {
        // Source repeats this guard after slot sampling.
        if selected.is_empty() {
            return Ok(DispenseOutcome::Noop { slot });
        }
        let facing = host.current_facing()?;
        if let Some(destination) = host.front_container(facing)? {
            let mut one = selected.clone();
            one.count = 1;
            let remainder = host.insert_one(slot, destination, one, facing.opposite())?;
            ensure!(
                remainder.is_empty()
                    || remainder.count == 1 && remainder.same_item_same_components(&selected)?,
                "invalid dropper one-item insertion remainder"
            );
            let accepted = remainder.is_empty();
            let mut replacement = selected;
            if accepted {
                replacement.count -= 1;
            }
            write_item(host, slot, replacement)?;
            return Ok(if accepted {
                DispenseOutcome::Inserted { slot }
            } else {
                DispenseOutcome::DestinationRefused { slot }
            });
        }
        // Dropper bypasses the dispenser registry/feature branch entirely.
        default_dispense(host, slot)?;
    } else {
        // A disabled item explicitly gets the default behavior, even if that
        // item has a registered projectile or other specialized behavior.
        let behavior = if host.item_enabled(&selected)? {
            host.registered_behavior(&selected)?
        } else {
            DispenseBehavior::Default
        };
        match behavior {
            DispenseBehavior::Noop => return Ok(DispenseOutcome::Noop { slot }),
            DispenseBehavior::Default => default_dispense(host, slot)?,
            DispenseBehavior::Registered(behavior) => {
                let replacement = host.execute_registered(slot, behavior)?;
                write_item(host, slot, replacement)?;
            }
        }
    }
    Ok(DispenseOutcome::Dispensed { slot })
}

fn write_item(host: &mut impl DispenserHost, slot: usize, mut stack: ItemStack) -> Result<()> {
    if stack.is_empty() {
        stack = ItemStack::empty();
    } else {
        stack.count = stack.count.min(stack.max_stack_size()?.min(99));
    }
    host.set_item_and_changed(slot, stack)
}

fn default_dispense(host: &mut impl DispenserHost, slot: usize) -> Result<()> {
    let facing = host.captured_facing()?;
    let position = default_spawn_position(host.captured_position(), facing);
    let item = host.split_item_raw(slot, 1)?;
    host.spawn_default_item(
        item,
        DefaultItemSpawn {
            position,
            facing,
            accuracy: DEFAULT_ACCURACY,
        },
    )?;
    host.level_event(1000, 0)?;
    host.level_event(2000, facing as i32)?;
    let replacement = host.items()[slot].clone();
    write_item(host, slot, replacement)
}

/// Default behavior's dispense point: block center plus facing*0.7, followed
/// by the source Item-spawn Y adjustment (vertical 0.125, horizontal 0.15625).
pub fn default_spawn_position(position: (i32, i32, i32), facing: Direction) -> Vec3 {
    let (dx, dy, dz) = facing.step();
    let y_offset = if matches!(facing, Direction::Up | Direction::Down) {
        0.125
    } else {
        0.15625
    };
    Vec3::new(
        f64::from(position.0) + 0.5 + 0.7 * f64::from(dx),
        f64::from(position.1) + 0.5 + 0.7 * f64::from(dy) - y_offset,
        f64::from(position.2) + 0.5 + 0.7 * f64::from(dz),
    )
}

/// Call after constructing the real Item entity, with the same Level RNG.
/// One speed draw, then x/y/z triangle draws (two doubles each); Y mean is
/// always 0.2, including downward dispensing. This uses no Gaussian sampler.
pub fn default_item_velocity(
    random: &mut (impl RandomSource + ?Sized),
    facing: Direction,
    accuracy: i32,
) -> Vec3 {
    let speed = random.next_f64() * 0.1 + 0.2;
    let deviation = 0.0172275 * f64::from(accuracy);
    let (dx, _, dz) = facing.step();
    let mut triangle = |mean| mean + deviation * (random.next_f64() - random.next_f64());
    Vec3::new(
        triangle(f64::from(dx) * speed),
        triangle(0.2),
        triangle(f64::from(dz) * speed),
    )
}

/// Source analog output for the actual nine-slot container, with Java float
/// intermediates. This does not notify or fabricate a comparator receiver.
pub fn analog_output(items: &[ItemStack; SLOT_COUNT]) -> Result<i32> {
    let mut fullness = 0.0_f32;
    let mut occupied = false;
    for stack in items {
        if !stack.is_empty() {
            let maximum = stack.max_stack_size()?.min(99);
            ensure!(maximum > 0, "invalid dispenser stack maximum");
            fullness += stack.count as f32 / maximum as f32;
            occupied = true;
        }
    }
    Ok(
        ((fullness / SLOT_COUNT as f32 * 14.0_f32).floor() as i32)
            .wrapping_add(i32::from(occupied)),
    )
}

/// Captured block-state and real BLOCK-scheduler owners for neighbor changes.
pub trait DispenserTriggerHost {
    /// Native preflight requires a staged actual state/scheduler transaction.
    fn preflight_trigger(&mut self) -> Result<()>;
    fn neighbor_signal(&mut self, above: bool) -> Result<bool>;
    fn schedule_block_tick(&mut self, delay: i32) -> Result<()>;
    /// Write the captured real state's TRIGGERED property, preserving its
    /// facing and all other properties. Do not rebind to a replacement owner.
    fn set_captured_triggered(&mut self, triggered: bool, flags: i32) -> Result<()>;
}

/// Schedule first, then set TRIGGERED with flags 2. Falling power clears the
/// flag but leaves the actual scheduled tick intact. Above is short-circuited.
pub fn neighbor_changed(
    captured_triggered: bool,
    host: &mut impl DispenserTriggerHost,
) -> Result<()> {
    host.preflight_trigger()?;
    let powered = host.neighbor_signal(false)? || host.neighbor_signal(true)?;
    if powered && !captured_triggered {
        host.schedule_block_tick(TRIGGER_DELAY)?;
        host.set_captured_triggered(true, 2)?;
    } else if !powered && captured_triggered {
        host.set_captured_triggered(false, 2)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{crafting::RecipeBook, random::LegacyRandomSource};

    fn item(count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id("dirt").unwrap(), count).unwrap()
    }

    #[test]
    fn reservoir_and_default_velocity_match_unchanged_java_rng_method_facts() -> Result<()> {
        let mut slots = std::array::from_fn(|_| ItemStack::empty());
        slots[1] = item(1);
        slots[4] = item(64);
        slots[7] = item(3);
        // Direct calls to unchanged dyz/ayw methods in the pinned artifact:
        // nextInt(1..3) gives 0,1,1, then nextInt()=-742418317.
        let mut random = LegacyRandomSource::new(55);
        assert_eq!(random_slot(&slots, &mut random)?, Some(1));
        assert_eq!(random.next_i32(), -742418317);
        let mut random = LegacyRandomSource::new(55);
        let velocity = default_item_velocity(&mut random, Direction::East, 6);
        assert_eq!(
            velocity,
            Vec3::new(
                0.27177007744713355,
                0.17748820205937174,
                -0.02673956673299945
            )
        );
        assert_eq!(random.next_i32(), 559611923);
        let position = default_spawn_position((0, 5, 0), Direction::Down);
        assert_eq!(position, Vec3::new(0.5, 4.675, 0.5));
        assert_eq!(analog_output(&slots)?, 2);
        Ok(())
    }

    #[derive(Clone, Copy)]
    enum FixtureBehavior {
        Default,
        Noop,
        Unsupported,
    }
    #[derive(Clone)]
    struct Host {
        slots: [ItemStack; SLOT_COUNT],
        random: LegacyRandomSource,
        calls: Vec<String>,
        spawned: Vec<ItemStack>,
        destination: Option<ItemStack>,
        accept: bool,
        enabled: bool,
        behavior: FixtureBehavior,
        facing: Direction,
        current_facing: Direction,
        powered: bool,
        powered_above: bool,
    }
    impl Host {
        fn new() -> Self {
            Self {
                slots: std::array::from_fn(|_| ItemStack::empty()),
                random: LegacyRandomSource::new(55),
                calls: vec![],
                spawned: vec![],
                destination: None,
                accept: true,
                enabled: true,
                behavior: FixtureBehavior::Default,
                facing: Direction::East,
                current_facing: Direction::Up,
                powered: true,
                powered_above: false,
            }
        }
    }
    // Recording fixture owners are explicit isolated contract checks. They do
    // not establish production loot, behavior, hopper, Item/RNG/ID integration.
    impl DispenserHost for Host {
        type RegisteredBehavior = ();
        type Destination = ();
        fn preflight(&mut self, _: DispenserKind) -> Result<()> {
            Ok(())
        }
        fn matching_block_entity(&self, _: DispenserKind) -> Result<bool> {
            Ok(true)
        }
        fn unpack_loot(&mut self) -> Result<()> {
            self.calls.push("loot:null".into());
            Ok(())
        }
        fn items(&self) -> &[ItemStack; SLOT_COUNT] {
            &self.slots
        }
        fn level_random(&mut self) -> &mut dyn RandomSource {
            &mut self.random
        }
        fn captured_position(&self) -> (i32, i32, i32) {
            (0, 5, 0)
        }
        fn captured_facing(&self) -> Result<Direction> {
            Ok(self.facing)
        }
        fn current_facing(&self) -> Result<Direction> {
            Ok(self.current_facing)
        }
        fn item_enabled(&self, _: &ItemStack) -> Result<bool> {
            Ok(self.enabled)
        }
        fn registered_behavior(&self, _: &ItemStack) -> Result<DispenseBehavior<()>> {
            Ok(match self.behavior {
                FixtureBehavior::Default => DispenseBehavior::Default,
                FixtureBehavior::Noop => DispenseBehavior::Noop,
                FixtureBehavior::Unsupported => DispenseBehavior::Registered(()),
            })
        }
        fn execute_registered(&mut self, _: usize, _: ()) -> Result<ItemStack> {
            anyhow::bail!("fixture rejects unported selected registered behavior")
        }
        fn split_item_raw(&mut self, slot: usize, amount: i32) -> Result<ItemStack> {
            self.calls.push(format!("split:{slot}:{amount}"));
            let mut owned = self.slots[slot].clone();
            owned.count = amount.min(owned.count);
            self.slots[slot].count -= owned.count;
            if self.slots[slot].is_empty() {
                self.slots[slot] = ItemStack::empty();
            }
            Ok(owned)
        }
        fn spawn_default_item(&mut self, item: ItemStack, spawn: DefaultItemSpawn) -> Result<()> {
            self.calls.push("construct-item".into());
            let _velocity = default_item_velocity(&mut self.random, spawn.facing, spawn.accuracy);
            self.calls.push("motion/add-item".into());
            self.spawned.push(item);
            Ok(())
        }
        fn level_event(&mut self, id: i32, data: i32) -> Result<()> {
            self.calls.push(format!("event:{id}:{data}"));
            Ok(())
        }
        fn empty_block_activate(&mut self) -> Result<()> {
            self.calls.push("block-activate".into());
            Ok(())
        }
        fn front_container(&mut self, facing: Direction) -> Result<Option<()>> {
            self.calls.push(format!("front:{}", facing.name()));
            Ok(self.destination.is_some().then_some(()))
        }
        fn insert_one(
            &mut self,
            _: usize,
            _: (),
            item: ItemStack,
            face: Direction,
        ) -> Result<ItemStack> {
            self.calls.push(format!("insert:{}", face.name()));
            assert_eq!(item.count, 1);
            if self.accept {
                self.destination = Some(item);
                Ok(ItemStack::empty())
            } else {
                Ok(item)
            }
        }
        fn set_item_and_changed(&mut self, slot: usize, item: ItemStack) -> Result<()> {
            self.calls.push(format!("set/dirty:{slot}:{}", item.count));
            self.slots[slot] = item;
            Ok(())
        }
    }
    impl DispenserTriggerHost for Host {
        fn preflight_trigger(&mut self) -> Result<()> {
            Ok(())
        }
        fn neighbor_signal(&mut self, above: bool) -> Result<bool> {
            self.calls.push(format!("signal:{above}"));
            Ok(if above {
                self.powered_above
            } else {
                self.powered
            })
        }
        fn schedule_block_tick(&mut self, delay: i32) -> Result<()> {
            self.calls.push(format!("schedule:{delay}"));
            Ok(())
        }
        fn set_captured_triggered(&mut self, value: bool, flags: i32) -> Result<()> {
            self.calls.push(format!("trigger:{value}:{flags}"));
            Ok(())
        }
    }

    #[test]
    fn scheduled_ejection_and_dropper_insertion_preserve_actual_ownership_order() -> Result<()> {
        let mut host = Host::new();
        neighbor_changed(false, &mut host)?;
        assert_eq!(host.calls, ["signal:false", "schedule:4", "trigger:true:2"]);
        host.calls.clear();
        host.powered = false;
        neighbor_changed(true, &mut host)?;
        assert_eq!(
            host.calls,
            ["signal:false", "signal:true", "trigger:false:2"]
        );
        host.calls.clear();
        host.slots[4] = item(3);
        // Actual scheduled callbacks do not ask power again after it fell.
        assert_eq!(
            scheduled_tick(DispenserKind::Dispenser, &mut host)?,
            DispenseOutcome::Dispensed { slot: 4 }
        );
        assert_eq!(
            host.calls,
            [
                "loot:null",
                "split:4:1",
                "construct-item",
                "motion/add-item",
                "event:1000:0",
                "event:2000:5",
                "set/dirty:4:2"
            ]
        );
        assert_eq!(host.spawned, [item(1)]);
        assert_eq!(host.slots[4], item(2));
        host.calls.clear();
        host.destination = Some(ItemStack::empty());
        assert_eq!(
            scheduled_tick(DispenserKind::Dropper, &mut host)?,
            DispenseOutcome::Inserted { slot: 4 }
        );
        assert_eq!(
            host.calls,
            ["loot:null", "front:up", "insert:down", "set/dirty:4:1"]
        );
        assert_eq!(host.slots[4], item(1));
        assert_eq!(host.destination, Some(item(1)));
        host.calls.clear();
        host.accept = false;
        assert_eq!(
            scheduled_tick(DispenserKind::Dropper, &mut host)?,
            DispenseOutcome::DestinationRefused { slot: 4 }
        );
        assert_eq!(host.slots[4], item(1));
        assert_eq!(host.calls.last().unwrap(), "set/dirty:4:1");
        Ok(())
    }

    #[test]
    fn empty_noop_disabled_and_unported_contexts_stay_distinct() -> Result<()> {
        let mut host = Host::new();
        let rng_before = host.random.state48();
        assert_eq!(
            scheduled_tick(DispenserKind::Dispenser, &mut host)?,
            DispenseOutcome::Empty
        );
        assert_eq!(host.calls, ["loot:null", "event:1001:0", "block-activate"]);
        assert_eq!(host.random.state48(), rng_before);
        host.calls.clear();
        scheduled_tick(DispenserKind::Dropper, &mut host)?;
        assert_eq!(host.calls, ["loot:null", "event:1001:0"]);
        host.calls.clear();
        host.slots[0] = item(1);
        host.behavior = FixtureBehavior::Noop;
        assert_eq!(
            scheduled_tick(DispenserKind::Dispenser, &mut host)?,
            DispenseOutcome::Noop { slot: 0 }
        );
        assert_eq!(host.calls, ["loot:null"]);
        assert_eq!(host.slots[0], item(1));
        host.behavior = FixtureBehavior::Unsupported;
        // The caller owns rollback: an unsupported staged action is discarded.
        let mut staged = host.clone();
        assert!(scheduled_tick(DispenserKind::Dispenser, &mut staged).is_err());
        assert_eq!(host.slots[0], item(1));
        host.enabled = false;
        scheduled_tick(DispenserKind::Dispenser, &mut host)?;
        assert!(host.slots[0].is_empty()); // disabled item uses default, not registry
        assert_eq!(host.spawned, [item(1)]);
        Ok(())
    }
}
