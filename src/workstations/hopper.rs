//! Java 1.21 hopper transfer kernels over actual borrowed container owners.
//! World lookup, entity selection, loot unpacking, dirty callbacks and transaction
//! rollback remain with the real level/container owners; no empty world exists here.
use crate::{
    geometry::{Aabb, Direction, Vec3},
    inventory::ItemStack,
};
use anyhow::{ensure, Result};

pub const MENU_TYPE: i32 = 16;
pub const CONTAINER_SLOTS: usize = 5;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HopperTiming {
    pub cooldown: i32,
    pub ticked_game_time: i64,
}
impl Default for HopperTiming {
    fn default() -> Self {
        Self {
            cooldown: -1,
            ticked_game_time: 0,
        }
    }
}

/// A borrow of the actual counterpart or hopper container. Methods must retain
/// that owner's identity, loot/slot callbacks and sided rules. Ordinary and
/// sided containers are distinguished by the host, not guessed from contents.
/// `set_item_count` mutates the existing stack without invoking setItem.
/// Every method is required; unsupported owners must return an explicit error.
pub trait HopperContainer {
    fn size(&self) -> usize;
    fn item(&mut self, slot: usize) -> Result<ItemStack>;
    fn set_item(&mut self, slot: usize, stack: ItemStack) -> Result<()>;
    fn set_item_count(&mut self, slot: usize, count: i32) -> Result<()>;
    fn remove_item(&mut self, slot: usize, count: i32) -> Result<ItemStack>;
    fn slots_for_face(&self, face: Direction) -> Result<Vec<usize>>;
    fn can_place_item(&mut self, slot: usize, stack: &ItemStack) -> Result<bool>;
    fn can_place_through_face(
        &mut self,
        slot: usize,
        stack: &ItemStack,
        face: Option<Direction>,
    ) -> Result<bool>;
    fn can_take_item_to(
        &mut self,
        destination: &dyn HopperContainer,
        slot: usize,
        stack: &ItemStack,
    ) -> Result<bool>;
    fn can_take_through_face(
        &mut self,
        slot: usize,
        stack: &ItemStack,
        face: Direction,
    ) -> Result<bool>;
    fn hopper_timing(&self) -> Option<HopperTiming>;
    fn set_hopper_timing(&mut self, timing: HopperTiming) -> Result<()>;
    fn set_changed(&mut self) -> Result<()>;
}

pub trait HopperItemEntity {
    fn bounding_box(&self) -> Result<Aabb>;
    fn stack(&self) -> Result<ItemStack>;
    fn set_stack(&mut self, stack: ItemStack) -> Result<()>;
    fn discard(&mut self) -> Result<()>;
}

/// Bind this to one actual level/hopper position and captured block state.
/// Container lookups implement block ContainerHolder, real block entity,
/// combined chest (ignore obstruction=true), then entity-container selection
/// using the actual level RNG and encounter order. None certifies absence;
/// unavailable context is an error. The enclosing transaction must stage all
/// involved real inventories, counters, entity effects and callbacks together.
pub trait HopperWorld {
    fn is_client_side(&self) -> bool;
    fn game_time(&self) -> i64;
    fn hopper_position(&self) -> (i32, i32, i32);
    fn enabled(&self) -> Result<bool>;
    fn facing(&self) -> Result<Direction>;
    fn output_container(&mut self) -> Result<Option<&mut dyn HopperContainer>>;
    fn source_container(&mut self) -> Result<Option<&mut dyn HopperContainer>>;
    /// Grid-aligned hopper, above state solid, not DOES_NOT_BLOCK_HOPPERS.
    fn suction_blocked(&mut self) -> Result<bool>;
    /// Actual alive Item entities intersecting the translated suction AABB, in
    /// the Level query's encounter order. These are actual shared entity IDs.
    fn suction_entities(&mut self) -> Result<Vec<i32>>;
    fn item_entity(&mut self, id: i32) -> Result<&mut dyn HopperItemEntity>;
    /// Static BlockEntity.setChanged(level,pos,state), after successful move.
    fn hopper_changed(&mut self) -> Result<()>;
}

fn empty(container: &mut dyn HopperContainer) -> Result<bool> {
    for slot in 0..container.size() {
        if !container.item(slot)?.is_empty() {
            return Ok(false);
        }
    }
    Ok(true)
}
fn full_hopper(container: &mut dyn HopperContainer) -> Result<bool> {
    for slot in 0..container.size() {
        let stack = container.item(slot)?;
        if stack.is_empty() || stack.count != stack.max_stack_size()? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn face_slots(container: &dyn HopperContainer, face: Option<Direction>) -> Result<Vec<usize>> {
    let slots = match face {
        Some(face) => container.slots_for_face(face)?,
        None => (0..container.size()).collect(),
    };
    ensure!(
        slots.iter().all(|slot| *slot < container.size()),
        "counterpart returned invalid sided slots"
    );
    Ok(slots)
}
fn full_face(container: &mut dyn HopperContainer, face: Direction) -> Result<bool> {
    for slot in face_slots(container, Some(face))? {
        let stack = container.item(slot)?;
        if stack.is_empty() || stack.count < stack.max_stack_size()? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Source addItem/tryMoveInItem. `source_timing` is taken from the actual
/// sending hopper, or None for non-hopper containers/item entities.
pub fn insert_item(
    source_timing: Option<HopperTiming>,
    target: &mut dyn HopperContainer,
    mut stack: ItemStack,
    side: Option<Direction>,
) -> Result<ItemStack> {
    for slot in face_slots(target, side)? {
        if stack.is_empty() {
            break;
        }
        let existing = target.item(slot)?;
        if !target.can_place_item(slot, &stack)?
            || !target.can_place_through_face(slot, &stack, side)?
        {
            continue;
        }
        let was_empty = empty(target)?;
        let changed;
        if existing.is_empty() {
            // Source passes the whole stack to the actual setter, whose own
            // container/item cap and callbacks remain authoritative.
            target.set_item(slot, stack)?;
            stack = ItemStack::empty();
            changed = true;
        } else if existing.count <= existing.max_stack_size()?
            && existing.same_item_same_components(&stack)?
        {
            let amount = stack.count.min(stack.max_stack_size()? - existing.count);
            stack.count -= amount;
            target.set_item_count(slot, existing.count + amount)?;
            changed = amount > 0;
        } else {
            changed = false;
        }
        if changed {
            if was_empty {
                if let Some(mut timing) = target.hopper_timing() {
                    if timing.cooldown <= 8 {
                        let adjustment = source_timing.is_some_and(|source| {
                            timing.ticked_game_time >= source.ticked_game_time
                        }) as i32;
                        timing.cooldown = 8 - adjustment;
                        target.set_hopper_timing(timing)?;
                    }
                }
            }
            target.set_changed()?;
        }
    }
    Ok(if stack.is_empty() {
        ItemStack::empty()
    } else {
        stack
    })
}

fn restore_failed_take(
    source: &mut dyn HopperContainer,
    slot: usize,
    original: ItemStack,
) -> Result<()> {
    if original.count == 1 {
        source.set_item(slot, original)
    } else {
        source.set_item_count(slot, original.count)
    }
}

/// Native ejectItems: first successful source slot wins; output insertion and
/// the extra destination setChanged occur before the enclosing hopper callback.
pub fn push_into(
    source: &mut dyn HopperContainer,
    target: &mut dyn HopperContainer,
    face: Direction,
) -> Result<bool> {
    if full_face(target, face)? {
        return Ok(false);
    }
    for slot in 0..source.size() {
        let original = source.item(slot)?;
        if original.is_empty() {
            continue;
        }
        let removed = source.remove_item(slot, 1)?;
        let remaining = insert_item(source.hopper_timing(), target, removed, Some(face))?;
        if remaining.is_empty() {
            target.set_changed()?;
            return Ok(true);
        }
        restore_failed_take(source, slot, original)?;
    }
    Ok(false)
}

/// Native tryTakeInItemFromSlot: generic canTakeItem receives the actual
/// destination object, then the actual WorldlyContainer down-face rule runs.
pub fn pull_from(
    target: &mut dyn HopperContainer,
    source: &mut dyn HopperContainer,
) -> Result<bool> {
    for slot in face_slots(source, Some(Direction::Down))? {
        let original = source.item(slot)?;
        if original.is_empty()
            || !source.can_take_item_to(target, slot, &original)?
            || !source.can_take_through_face(slot, &original, Direction::Down)?
        {
            continue;
        }
        let removed = source.remove_item(slot, 1)?;
        let remaining = insert_item(source.hopper_timing(), target, removed, None)?;
        if remaining.is_empty() {
            source.set_changed()?;
            return Ok(true);
        }
        restore_failed_take(source, slot, original)?;
    }
    Ok(false)
}

/// Partial absorption mutates the actual entity stack but returns false; only
/// complete absorption clears/discards that identity and returns true.
pub fn absorb_entity(
    target: &mut dyn HopperContainer,
    entity: &mut dyn HopperItemEntity,
) -> Result<bool> {
    let remaining = insert_item(None, target, entity.stack()?, None)?;
    let absorbed = remaining.is_empty();
    entity.set_stack(remaining)?;
    if absorbed {
        entity.discard()?;
    }
    Ok(absorbed)
}

fn suck_in(
    hopper: &mut dyn HopperContainer,
    world: &mut (impl HopperWorld + ?Sized),
) -> Result<bool> {
    if let Some(source) = world.source_container()? {
        return pull_from(hopper, source);
    }
    // A present but empty/rejecting source container never falls through to
    // loose items. A genuinely absent container permits the actual shape path.
    if world.suction_blocked()? {
        return Ok(false);
    }
    for id in world.suction_entities()? {
        if absorb_entity(hopper, world.item_entity(id)?)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn move_phase(
    hopper: &mut dyn HopperContainer,
    world: &mut impl HopperWorld,
    pull: impl FnOnce(&mut dyn HopperContainer, &mut dyn HopperWorld) -> Result<bool>,
) -> Result<bool> {
    let timing = hopper
        .hopper_timing()
        .ok_or_else(|| anyhow::anyhow!("actual hopper timing owner required"))?;
    if world.is_client_side() || timing.cooldown > 0 || !world.enabled()? {
        return Ok(false);
    }
    let mut moved = false;
    if !empty(hopper)? {
        let face = world.facing()?.opposite();
        if let Some(target) = world.output_container()? {
            moved = push_into(hopper, target, face)?;
        }
    }
    if !full_hopper(hopper)? {
        // Java uses bitwise OR: pull executes even after a successful push.
        moved |= pull(hopper, world)?;
    }
    if moved {
        let mut timing = hopper.hopper_timing().unwrap();
        timing.cooldown = 8;
        hopper.set_hopper_timing(timing)?;
        world.hopper_changed()?;
    }
    Ok(moved)
}

/// Actual server block-entity ticker phase: decrement first, record level time,
/// normalize nonpositive cooldown to zero, then push and pull.
pub fn push_items_tick(
    hopper: &mut dyn HopperContainer,
    world: &mut impl HopperWorld,
) -> Result<bool> {
    ensure!(
        hopper.size() == CONTAINER_SLOTS,
        "hopper must borrow its actual five-slot owner"
    );
    let mut timing = hopper
        .hopper_timing()
        .ok_or_else(|| anyhow::anyhow!("actual hopper timing owner required"))?;
    timing.cooldown = timing.cooldown.wrapping_sub(1);
    timing.ticked_game_time = world.game_time();
    if timing.cooldown <= 0 {
        timing.cooldown = 0;
    }
    hopper.set_hopper_timing(timing)?;
    move_phase(hopper, world, |target, world| suck_in(target, world))
}

/// The original block-aligned Hopper.SUCK_AABB translated to real world space.
pub fn suction_aabb(position: (i32, i32, i32)) -> Aabb {
    Aabb::new(0., 11. / 16., 0., 1., 2., 1.).moved(Vec3::new(
        position.0 as f64,
        position.1 as f64,
        position.2 as f64,
    ))
}

/// Entity-inside callback using the actual Item entity's bounds and translated
/// Hopper.SUCK_AABB (BooleanOp.AND). No tick decrement occurs.
pub fn entity_inside(
    hopper: &mut dyn HopperContainer,
    entity: &mut dyn HopperItemEntity,
    world: &mut impl HopperWorld,
) -> Result<bool> {
    ensure!(
        hopper.size() == CONTAINER_SLOTS,
        "hopper must borrow its actual five-slot owner"
    );
    if !entity
        .bounding_box()?
        .intersects(&suction_aabb(world.hopper_position()))
    {
        return Ok(false);
    }
    move_phase(hopper, world, |target, _| absorb_entity(target, entity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::RecipeBook;
    struct Container {
        slots: Vec<ItemStack>,
        timing: Option<HopperTiming>,
        changed: usize,
        order: Vec<usize>,
    }
    impl Container {
        fn new(size: usize, hopper: bool) -> Self {
            Self {
                slots: vec![ItemStack::empty(); size],
                timing: hopper.then(HopperTiming::default),
                changed: 0,
                order: (0..size).collect(),
            }
        }
    }
    impl HopperContainer for Container {
        fn size(&self) -> usize {
            self.slots.len()
        }
        fn item(&mut self, slot: usize) -> Result<ItemStack> {
            Ok(self.slots[slot].clone())
        }
        fn set_item(&mut self, slot: usize, stack: ItemStack) -> Result<()> {
            self.slots[slot] = stack;
            Ok(())
        }
        fn set_item_count(&mut self, slot: usize, count: i32) -> Result<()> {
            self.slots[slot].count = count;
            if count <= 0 {
                self.slots[slot] = ItemStack::empty();
            }
            Ok(())
        }
        fn remove_item(&mut self, slot: usize, count: i32) -> Result<ItemStack> {
            let mut stack = self.slots[slot].clone();
            stack.count = count.min(stack.count);
            self.slots[slot].count -= stack.count;
            if self.slots[slot].count <= 0 {
                self.slots[slot] = ItemStack::empty();
            }
            Ok(stack)
        }
        fn slots_for_face(&self, _: Direction) -> Result<Vec<usize>> {
            Ok(self.order.clone())
        }
        fn can_place_item(&mut self, _: usize, _: &ItemStack) -> Result<bool> {
            Ok(true)
        }
        fn can_place_through_face(
            &mut self,
            _: usize,
            _: &ItemStack,
            _: Option<Direction>,
        ) -> Result<bool> {
            Ok(true)
        }
        fn can_take_item_to(
            &mut self,
            destination: &dyn HopperContainer,
            _: usize,
            _: &ItemStack,
        ) -> Result<bool> {
            ensure!(
                destination.size() == 5,
                "actual destination must be hopper in this flow"
            );
            Ok(true)
        }
        fn can_take_through_face(&mut self, _: usize, _: &ItemStack, _: Direction) -> Result<bool> {
            Ok(true)
        }
        fn hopper_timing(&self) -> Option<HopperTiming> {
            self.timing
        }
        fn set_hopper_timing(&mut self, value: HopperTiming) -> Result<()> {
            ensure!(self.timing.is_some(), "not hopper");
            self.timing = Some(value);
            Ok(())
        }
        fn set_changed(&mut self) -> Result<()> {
            self.changed += 1;
            Ok(())
        }
    }
    struct Entity {
        stack: ItemStack,
        discarded: bool,
    }
    impl HopperItemEntity for Entity {
        fn bounding_box(&self) -> Result<Aabb> {
            Ok(Aabb::new(0., 0.7, 0., 0.25, 1., 0.25))
        }
        fn stack(&self) -> Result<ItemStack> {
            Ok(self.stack.clone())
        }
        fn set_stack(&mut self, stack: ItemStack) -> Result<()> {
            self.stack = stack;
            Ok(())
        }
        fn discard(&mut self) -> Result<()> {
            self.discarded = true;
            Ok(())
        }
    }
    struct World {
        output: Container,
        source: Container,
        changed: usize,
    }
    impl HopperWorld for World {
        fn is_client_side(&self) -> bool {
            false
        }
        fn game_time(&self) -> i64 {
            100
        }
        fn hopper_position(&self) -> (i32, i32, i32) {
            (0, 0, 0)
        }
        fn enabled(&self) -> Result<bool> {
            Ok(true)
        }
        fn facing(&self) -> Result<Direction> {
            Ok(Direction::East)
        }
        fn output_container(&mut self) -> Result<Option<&mut dyn HopperContainer>> {
            Ok(Some(&mut self.output))
        }
        fn source_container(&mut self) -> Result<Option<&mut dyn HopperContainer>> {
            Ok(Some(&mut self.source))
        }
        fn suction_blocked(&mut self) -> Result<bool> {
            anyhow::bail!("source exists: no shape fallback")
        }
        fn suction_entities(&mut self) -> Result<Vec<i32>> {
            anyhow::bail!("source exists: no entity fallback")
        }
        fn item_entity(&mut self, _: i32) -> Result<&mut dyn HopperItemEntity> {
            anyhow::bail!("no item query")
        }
        fn hopper_changed(&mut self) -> Result<()> {
            self.changed += 1;
            Ok(())
        }
    }
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    #[test]
    fn tick_pushes_then_pulls_and_adjusts_real_counterpart_cooldown() -> Result<()> {
        let mut hopper = Container::new(5, true);
        hopper.slots[0] = item("dirt", 1);
        let mut world = World {
            output: Container::new(5, true),
            source: Container::new(1, false),
            changed: 0,
        };
        world.output.timing = Some(HopperTiming {
            cooldown: 0,
            ticked_game_time: 100,
        });
        world.source.slots[0] = item("emerald", 1);
        assert!(push_items_tick(&mut hopper, &mut world)?);
        assert_eq!(world.output.slots[0], item("dirt", 1));
        assert_eq!(hopper.slots[0], item("emerald", 1));
        assert!(world.source.slots[0].is_empty());
        assert_eq!(world.output.timing.unwrap().cooldown, 7);
        assert_eq!(
            hopper.timing.unwrap(),
            HopperTiming {
                cooldown: 8,
                ticked_game_time: 100
            }
        );
        assert_eq!(world.changed, 1);
        assert!(!push_items_tick(&mut hopper, &mut world)?);
        assert_eq!(hopper.timing.unwrap().cooldown, 7);
        Ok(())
    }
    #[test]
    fn sided_order_and_failed_take_restore_canonical_source() -> Result<()> {
        let mut hopper = Container::new(5, true);
        hopper.slots[0] = item("dirt", 2);
        let mut target = Container::new(2, false);
        target.order = vec![1, 0];
        assert!(push_into(&mut hopper, &mut target, Direction::West)?);
        assert_eq!(target.slots[1], item("dirt", 1));
        assert!(target.slots[0].is_empty());
        let mut source = Container::new(1, false);
        source.slots[0] = item("emerald", 1);
        hopper.slots.fill(item("dirt", 64));
        assert!(!pull_from(&mut hopper, &mut source)?);
        assert_eq!(source.slots[0], item("emerald", 1));
        assert_eq!(source.changed, 0);
        Ok(())
    }
    #[test]
    fn partial_entity_absorption_is_not_reported_as_complete() -> Result<()> {
        let mut hopper = Container::new(5, true);
        hopper.slots.fill(item("dirt", 64));
        hopper.slots[0].count = 63;
        let mut entity = Entity {
            stack: item("dirt", 2),
            discarded: false,
        };
        assert!(!absorb_entity(&mut hopper, &mut entity)?);
        assert_eq!(entity.stack.count, 1);
        assert!(!entity.discarded);
        hopper.slots[1] = ItemStack::empty();
        assert!(absorb_entity(&mut hopper, &mut entity)?);
        assert!(entity.stack.is_empty());
        assert!(entity.discarded);
        Ok(())
    }
}
