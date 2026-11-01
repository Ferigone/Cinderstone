//! Java 1.21 personal ender inventory and active block-entity binding.
//!
//! Contents belong to the actual player, never the ender-chest block entity.
//! Reuse GenericContainerMenu's existing three-row click implementation. Main
//! must register one of these owners per real player and retain actual keys.
use crate::{inventory::ItemStack, item_nbt, nbt::Tag};
use anyhow::{ensure, Context, Result};

pub const SLOT_COUNT: usize = 27;
pub const MENU_TYPE: i32 = 2;

/// Keys are supplied by actual player/block-entity ownership. A newly placed
/// chest must receive a different key even at identical coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnderChestInventory<P, K> {
    player: P,
    slots: [ItemStack; SLOT_COUNT],
    active_chest: Option<K>,
}

/// Borrow the actual player and captured EnderChestBlockEntity. Callbacks run
/// inside the enclosing staged player/world transaction and must roll back on
/// failure. They include the real opener counter, RNG/sounds, block events,
/// game events and scheduler; unsupported receivers must return an error.
pub trait EnderChestLifecycle<P, K> {
    fn player_key(&self) -> &P;
    fn chest_still_valid(&self, key: &K, additional_range: f64) -> Result<bool>;
    fn start_chest_open(&mut self, key: &K) -> Result<()>;
    fn stop_chest_open(&mut self, key: &K) -> Result<()>;
}

impl<P: PartialEq, K: Clone + PartialEq> EnderChestInventory<P, K> {
    pub fn new(actual_player: P) -> Self {
        Self {
            player: actual_player,
            slots: std::array::from_fn(|_| ItemStack::empty()),
            active_chest: None,
        }
    }
    pub fn player_key(&self) -> &P {
        &self.player
    }
    pub fn slots(&self) -> &[ItemStack; SLOT_COUNT] {
        &self.slots
    }
    pub fn active_chest(&self) -> Option<&K> {
        self.active_chest.as_ref()
    }
    pub fn is_active_chest(&self, actual_key: &K) -> bool {
        self.active_chest.as_ref() == Some(actual_key)
    }
    pub fn set_active_chest(&mut self, actual_key: K) {
        self.active_chest = Some(actual_key);
    }

    /// SimpleContainer.setItem clamps to min(container99, item maximum). The
    /// returned old stack remains owned by the caller's inventory transaction.
    pub fn replace_slot(&mut self, slot: usize, mut stack: ItemStack) -> Result<ItemStack> {
        ensure!(slot < SLOT_COUNT, "invalid personal ender slot");
        if stack.is_empty() {
            stack = ItemStack::empty();
        } else {
            stack.count = stack.count.min(stack.max_stack_size()?.min(99));
            stack.encode_canonical()?;
        }
        Ok(std::mem::replace(&mut self.slots[slot], stack))
    }
    pub fn still_valid(&self, host: &impl EnderChestLifecycle<P, K>) -> Result<bool> {
        ensure!(
            host.player_key() == &self.player,
            "different personal ender owner"
        );
        if let Some(key) = &self.active_chest {
            return host.chest_still_valid(key, 4.0);
        }
        // The actual SimpleContainer superclass has no additional validity gate.
        Ok(true)
    }
    pub fn start_open(&self, host: &mut impl EnderChestLifecycle<P, K>) -> Result<()> {
        ensure!(
            host.player_key() == &self.player,
            "different personal ender owner"
        );
        if let Some(key) = &self.active_chest {
            host.start_chest_open(key)?;
        }
        // SimpleContainer inherits the no-op Container.startOpen callback.
        Ok(())
    }
    pub fn stop_open(&mut self, host: &mut impl EnderChestLifecycle<P, K>) -> Result<()> {
        ensure!(
            host.player_key() == &self.player,
            "different personal ender owner"
        );
        if let Some(key) = &self.active_chest {
            host.stop_chest_open(key)?;
        }
        // Source clears the active binding after the stop callback and no-op
        // superclass callback. It does not clear or return any owned contents.
        self.active_chest = None;
        Ok(())
    }

    /// PlayerEnderChestContainer.fromTag clears first, processes list order and
    /// interprets Slot as unsigned byte; later duplicate entries win. Unknown
    /// native item/component codecs fail atomically instead of discarding data.
    /// The actual player persistence owner decides when to publish the result.
    pub fn load_items(&mut self, list: &Tag) -> Result<()> {
        let Tag::List {
            element_type,
            values,
        } = list
        else {
            anyhow::bail!("ender contents require an NBT list");
        };
        ensure!(
            *element_type == 10 || values.is_empty(),
            "ender contents require compound entries"
        );
        let mut slots = std::array::from_fn(|_| ItemStack::empty());
        for entry in values {
            let slot = numeric_byte(entry.get("Slot")) as u8 as usize;
            if slot >= SLOT_COUNT {
                continue;
            }
            let mut stack = item_nbt::decode_item_stack(entry)
                .context("unsupported personal ender item/component")?;
            if !stack.is_empty() {
                stack.count = stack.count.min(stack.max_stack_size()?.min(99));
            }
            slots[slot] = if stack.is_empty() {
                ItemStack::empty()
            } else {
                stack
            };
        }
        self.slots = slots;
        Ok(())
    }
    pub fn save_items(&self) -> Result<Tag> {
        let mut values = vec![];
        for (slot, stack) in self.slots.iter().enumerate() {
            if stack.is_empty() {
                continue;
            }
            let Tag::Compound(mut fields) = item_nbt::encode_item_stack(stack, None)? else {
                unreachable!("item codec compound");
            };
            fields.insert(0, ("Slot".into(), Tag::Byte(slot as i8)));
            values.push(Tag::Compound(fields));
        }
        // An empty Java ListTag has element type END, not COMPOUND.
        Ok(Tag::List {
            element_type: if values.is_empty() { 0 } else { 10 },
            values,
        })
    }
}

fn numeric_byte(tag: Option<&Tag>) -> i8 {
    match tag {
        Some(Tag::Byte(v)) => *v,
        Some(Tag::Short(v)) => *v as i8,
        Some(Tag::Int(v)) => *v as i8,
        Some(Tag::Long(v)) => *v as i8,
        Some(Tag::Float(v)) => {
            let integer = *v as i32;
            (if *v < integer as f32 {
                integer.wrapping_sub(1)
            } else {
                integer
            }) as i8
        }
        Some(Tag::Double(v)) => {
            let integer = *v as i32;
            (if *v < integer as f64 {
                integer.wrapping_sub(1)
            } else {
                integer
            }) as i8
        }
        _ => 0,
    }
}

/// Real server-side EnderChestBlock.useWithoutItem callbacks, within one
/// staged world/player transaction. The host binds the actual player's same
/// personal inventory before openMenu; its old-menu close/constructor ordering
/// must see that binding. No second owner or guessed piglin-free world is used.
pub trait EnderChestUse {
    type ChestKey;
    fn actual_chest(&mut self) -> Result<Option<Self::ChestKey>>;
    fn above_is_redstone_conductor(&mut self) -> Result<bool>;
    fn set_personal_active_chest(&mut self, key: Self::ChestKey) -> Result<()>;
    fn open_personal_three_row_menu(&mut self) -> Result<()>;
    fn award_open_stat(&mut self) -> Result<()>;
    fn anger_nearby_piglins(&mut self, guarded: bool) -> Result<()>;
}
/// Both outcomes map to server CONSUME. Missing owner/blocked lid also consume;
/// they do not fall through to a fabricated world-storage container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnderChestUseResult {
    Consumed,
    Opened,
}
pub fn use_on_server(host: &mut impl EnderChestUse) -> Result<EnderChestUseResult> {
    let Some(key) = host.actual_chest()? else {
        return Ok(EnderChestUseResult::Consumed);
    };
    if host.above_is_redstone_conductor()? {
        return Ok(EnderChestUseResult::Consumed);
    }
    host.set_personal_active_chest(key)?;
    host.open_personal_three_row_menu()?;
    host.award_open_stat()?;
    host.anger_nearby_piglins(true)?;
    Ok(EnderChestUseResult::Opened)
}

/// Actual ChestLidController animation. Openers request the target through
/// block event1; renderer easing belongs to the renderer, not this controller.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnderChestLid {
    requested_open: bool,
    openness: f32,
    previous: f32,
}
impl EnderChestLid {
    pub fn trigger_event(&mut self, id: i32, value: i32) -> bool {
        if id != 1 {
            return false;
        }
        self.requested_open = value > 0;
        true
    }
    pub fn tick(&mut self) {
        self.previous = self.openness;
        if !self.requested_open && self.openness > 0.0 {
            self.openness = (self.openness - 0.1_f32).max(0.0);
        } else if self.requested_open && self.openness < 1.0 {
            self.openness = (self.openness + 0.1_f32).min(1.0);
        }
    }
    pub fn openness(&self, partial_tick: f32) -> f32 {
        self.previous + partial_tick * (self.openness - self.previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::RecipeBook;
    struct Host {
        player: u32,
        key: u64,
        calls: Vec<(bool, u64)>,
        fail_stop: bool,
    }
    impl EnderChestLifecycle<u32, u64> for Host {
        fn player_key(&self) -> &u32 {
            &self.player
        }
        fn chest_still_valid(&self, key: &u64, range: f64) -> Result<bool> {
            assert_eq!(range, 4.0);
            Ok(*key == self.key)
        }
        fn start_chest_open(&mut self, key: &u64) -> Result<()> {
            self.calls.push((true, *key));
            Ok(())
        }
        fn stop_chest_open(&mut self, key: &u64) -> Result<()> {
            ensure!(!self.fail_stop, "unsupported stop callback");
            self.calls.push((false, *key));
            Ok(())
        }
    }
    #[test]
    fn personal_contents_survive_close_and_actual_replacement_does_not_rebind() -> Result<()> {
        let item = ItemStack::new(RecipeBook::vanilla().item_id("diamond").unwrap(), 7)?;
        let mut inventory = EnderChestInventory::new(10_u32);
        inventory.replace_slot(26, item.clone())?;
        inventory.set_active_chest(88_u64);
        let mut host = Host {
            player: 10,
            key: 88,
            calls: vec![],
            fail_stop: false,
        };
        inventory.start_open(&mut host)?;
        assert!(inventory.still_valid(&host)?);
        host.key = 89;
        assert!(!inventory.still_valid(&host)?);
        host.fail_stop = true;
        assert!(inventory.stop_open(&mut host).is_err());
        assert_eq!(inventory.active_chest(), Some(&88));
        host.fail_stop = false;
        inventory.stop_open(&mut host)?;
        assert_eq!(host.calls, vec![(true, 88), (false, 88)]);
        assert_eq!(inventory.slots()[26], item);
        assert!(inventory.active_chest().is_none());
        let tag = inventory.save_items()?;
        let mut restored = EnderChestInventory::<u32, u64>::new(10);
        restored.load_items(&tag)?;
        assert_eq!(restored.slots(), inventory.slots());
        assert!(restored.active_chest().is_none());
        let other = EnderChestInventory::<u32, u64>::new(11);
        assert!(other.slots().iter().all(ItemStack::is_empty));
        Ok(())
    }
    #[test]
    fn lid_target_changes_only_on_actual_event_and_keeps_source_linear_interpolation() {
        let mut lid = EnderChestLid::default();
        assert!(!lid.trigger_event(9, 1));
        lid.tick();
        assert_eq!(lid.openness(1.0), 0.0);
        assert!(lid.trigger_event(1, 2));
        lid.tick();
        assert_eq!(lid.openness(0.5), 0.05);
        lid.trigger_event(1, 0);
        lid.tick();
        assert_eq!(lid.openness(1.0), 0.0);
    }
    #[test]
    fn negative_fractional_nbt_slot_is_floored_then_unsigned_instead_of_overwriting_zero(
    ) -> Result<()> {
        let stone = ItemStack::new(RecipeBook::vanilla().item_id("stone").unwrap(), 3)?;
        let diamond = ItemStack::new(RecipeBook::vanilla().item_id("diamond").unwrap(), 1)?;
        let Tag::Compound(mut first) = item_nbt::encode_item_stack(&stone, None)? else {
            unreachable!()
        };
        first.push(("Slot".into(), Tag::Byte(0)));
        let Tag::Compound(mut second) = item_nbt::encode_item_stack(&diamond, None)? else {
            unreachable!()
        };
        second.push(("Slot".into(), Tag::Float(-0.5)));
        let list = Tag::List {
            element_type: 10,
            values: vec![Tag::Compound(first), Tag::Compound(second)],
        };
        let mut owner = EnderChestInventory::<u32, u64>::new(3);
        owner.load_items(&list)?;
        assert_eq!(owner.slots()[0], stone);
        assert_eq!(
            owner
                .slots()
                .iter()
                .filter(|stack| !stack.is_empty())
                .count(),
            1
        );
        Ok(())
    }
}
