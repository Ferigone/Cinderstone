//! Original Java 1.21 Mob equipment mutation, death drops and base XP phases.
//!
//! Source: Mob (`btp`) 324–343, 527–529, 640–651, 975–1052 and 1224–1229.
//! These functions borrow actual actor/equipment/item/source/level/RNG owners.
//! They create no actor, inventory, loot engine, RNG stream or entity identity.
//! Errors retain completed mutations and draws: this source is not an atomic
//! inventory transaction, and detached equipment snapshots are not live owners.

use crate::equipment::EquipmentSlot;

/// The actual actor's retained Entity.random object. A handle must keep that
/// same object/lifetime through callbacks, removal and live-registry replacement.
/// Getting it must not seed, fork, clone or substitute another random stream.
pub trait MobEntityRandomHost {
    type Error;
    type Random;

    fn entity_random(&mut self) -> Result<Self::Random, Self::Error>;
    fn random_next_f32(&mut self, random: &Self::Random) -> Result<f32, Self::Error>;
    fn random_next_bounded_i32(
        &mut self,
        random: &Self::Random,
        bound: i32,
    ) -> Result<i32, Self::Error>;
}

/// Real virtual equipment/drop/enchantment/world operations on one actual Mob.
/// Item is a retained handle to the canonical ItemStack returned by getItemBySlot,
/// not a detached clone. The passed ServerLevel and current actor ServerLevel
/// are separate source values; do not assume they identify the same level.
pub trait MobEquipmentDeathHost: MobEntityRandomHost {
    type Source: ?Sized;
    type ServerLevel;
    type Item;
    type LivingEntity;

    fn item_by_slot(&mut self, slot: EquipmentSlot) -> Result<Self::Item, Self::Error>;
    fn equipment_drop_chance(&mut self, slot: EquipmentSlot) -> Result<f32, Self::Error>;
    /// DamageSource.getEntity instanceof LivingEntity, using the causing entity.
    fn causing_living_entity(
        &mut self,
        source: &Self::Source,
    ) -> Result<Option<Self::LivingEntity>, Self::Error>;
    /// Actual this.level() sampled only after a living causing entity is found.
    fn current_server_level(&mut self) -> Result<Option<Self::ServerLevel>, Self::Error>;
    /// Exact EnchantmentHelper.processEquipmentDropChance, with its real
    /// equipment/effect definitions, source context and effect RNG owners.
    fn process_equipment_drop_chance(
        &mut self,
        level: &Self::ServerLevel,
        causing: &Self::LivingEntity,
        source: &Self::Source,
        chance: f32,
    ) -> Result<f32, Self::Error>;
    fn item_is_empty(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
    /// Exact EnchantmentHelper.has(item, PREVENT_EQUIPMENT_DROP). A named curse,
    /// positive-level test or equipment-slot filter must not substitute for it.
    fn has_prevent_equipment_drop(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
    fn item_is_damageable(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
    fn item_max_damage(&mut self, item: &Self::Item) -> Result<i32, Self::Error>;
    fn item_set_damage(&mut self, item: &Self::Item, damage: i32) -> Result<(), Self::Error>;
    /// Actual Entity.spawnAtLocation on the retained stack: independent Item
    /// constructor RNG, UUID/shared ID, world-add, listeners and ordered tracking
    /// remain real owners. Its nullable return is ignored by this source body.
    fn spawn_at_location(&mut self, item: &Self::Item) -> Result<(), Self::Error>;
    /// Actual virtual setItemSlot(slot, canonical ItemStack.EMPTY), including
    /// verification and onEquip callbacks. No privately emptied image is used.
    fn set_item_slot_empty(&mut self, slot: EquipmentSlot) -> Result<(), Self::Error>;
}

/// The complete inspected LivingEntity.dropCustomDeathLoot body (line 1451)
/// is empty. This explicit parent body is not a missing-owner success default.
fn living_drop_custom_death_loot<L: ?Sized, S: ?Sized>(
    _level: &L,
    _source: &S,
    _recently_hit: bool,
) {
}

/// Mob.dropCustomDeathLoot, source lines 989–1014. The guaranteed flag uses the
/// RAW chance before enchantment modification; chance/IEEE values are unclamped.
/// Spawn precedes clearing the actual slot, including when a later callback fails.
pub fn drop_custom_death_loot<H: MobEquipmentDeathHost + ?Sized>(
    host: &mut H,
    level: &H::ServerLevel,
    source: &H::Source,
    recently_hit: bool,
) -> Result<(), H::Error> {
    living_drop_custom_death_loot(level, source, recently_hit);
    for slot in EquipmentSlot::ALL {
        let item = host.item_by_slot(slot)?;
        let mut chance = host.equipment_drop_chance(slot)?;
        if chance == 0.0 {
            continue;
        }
        let guaranteed = chance > 1.0;
        if let Some(causing) = host.causing_living_entity(source)? {
            if let Some(current_level) = host.current_server_level()? {
                chance =
                    host.process_equipment_drop_chance(&current_level, &causing, source, chance)?;
            }
        }
        if !host.item_is_empty(&item)?
            && !host.has_prevent_equipment_drop(&item)?
            && (recently_hit || guaranteed)
        {
            let random = host.entity_random()?;
            if host.random_next_f32(&random)? < chance {
                if !guaranteed && host.item_is_damageable(&item)? {
                    let maximum = host.item_max_damage(&item)?;
                    // JVM retains OUTER random receiver, then INNER receiver,
                    // before the second maxDamage read and the nested draws.
                    let outer_random = host.entity_random()?;
                    let inner_random = host.entity_random()?;
                    let inner_bound = host.item_max_damage(&item)?.wrapping_sub(3).max(1);
                    let inner = host.random_next_bounded_i32(&inner_random, inner_bound)?;
                    let outer =
                        host.random_next_bounded_i32(&outer_random, 1_i32.wrapping_add(inner))?;
                    host.item_set_damage(&item, maximum.wrapping_sub(outer))?;
                }
                host.spawn_at_location(&item)?;
                host.set_item_slot_empty(slot)?;
            }
        }
    }
    Ok(())
}

/// Actual xpReward and the two final source list objects/arrays, followed by
/// body armor. Index methods borrow the original Mob stores, not a Player menu.
pub trait MobEquipmentXpHost: MobEntityRandomHost {
    type Item;

    fn xp_reward(&mut self) -> Result<i32, Self::Error>;
    fn armor_size(&mut self) -> Result<i32, Self::Error>;
    fn armor_item(&mut self, index: i32) -> Result<Self::Item, Self::Error>;
    fn armor_drop_chance(&mut self, index: i32) -> Result<f32, Self::Error>;
    fn hand_size(&mut self) -> Result<i32, Self::Error>;
    fn hand_item(&mut self, index: i32) -> Result<Self::Item, Self::Error>;
    fn hand_drop_chance(&mut self, index: i32) -> Result<f32, Self::Error>;
    fn body_armor_item(&mut self) -> Result<Self::Item, Self::Error>;
    fn body_armor_drop_chance(&mut self) -> Result<f32, Self::Error>;
    fn item_is_empty(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
}

/// Mob.getBaseExperienceReward, source lines 324–343. Armor precedes hands and
/// body; this differs from EquipmentSlot.values death-drop order. Actual orb
/// creation and experience enchantment processing belong to their later owners.
pub fn get_base_experience_reward<H: MobEquipmentXpHost + ?Sized>(
    host: &mut H,
) -> Result<i32, H::Error> {
    if host.xp_reward()? <= 0 {
        return host.xp_reward();
    }
    let mut reward = host.xp_reward()?;
    let mut index = 0_i32;
    while index < host.armor_size()? {
        let item = host.armor_item(index)?;
        if !host.item_is_empty(&item)? && host.armor_drop_chance(index)? <= 1.0 {
            let random = host.entity_random()?;
            let extra = host.random_next_bounded_i32(&random, 3)?;
            reward = reward.wrapping_add(1_i32.wrapping_add(extra));
        }
        index = index.wrapping_add(1);
    }
    index = 0;
    while index < host.hand_size()? {
        let item = host.hand_item(index)?;
        if !host.item_is_empty(&item)? && host.hand_drop_chance(index)? <= 1.0 {
            let random = host.entity_random()?;
            let extra = host.random_next_bounded_i32(&random, 3)?;
            reward = reward.wrapping_add(1_i32.wrapping_add(extra));
        }
        index = index.wrapping_add(1);
    }
    let body = host.body_armor_item()?;
    if !host.item_is_empty(&body)? && host.body_armor_drop_chance()? <= 1.0 {
        let random = host.entity_random()?;
        let extra = host.random_next_bounded_i32(&random, 3)?;
        reward = reward.wrapping_add(1_i32.wrapping_add(extra));
    }
    Ok(reward)
}

pub trait MobLootTableOverrideHost {
    type Error;
    type Source: ?Sized;

    /// Actual LivingEntity superclass table processing, including registry,
    /// loot parameters, table seed/named random sequence and real item spawning.
    /// An empty default table does not authorize bypassing those owners.
    fn inherited_drop_from_loot_table(
        &mut self,
        source: &Self::Source,
        recently_hit: bool,
    ) -> Result<(), Self::Error>;
    /// Set only the actual Mob.lootTable field to null. Do not clear lootTableSeed
    /// or commit a privately copied actor if the inherited call fails.
    fn clear_custom_loot_table(&mut self) -> Result<(), Self::Error>;
}

/// Mob.dropFromLootTable, source lines 527–529. The real engine must complete
/// before the current custom table key is cleared; its seed is not reset here.
pub fn drop_from_loot_table<H: MobLootTableOverrideHost + ?Sized>(
    host: &mut H,
    source: &H::Source,
    recently_hit: bool,
) -> Result<(), H::Error> {
    host.inherited_drop_from_loot_table(source, recently_hit)?;
    host.clear_custom_loot_table()
}

/// Canonical primitive equipment replacement, without onEquip hidden inside it.
/// Item handles must preserve the actual old/new object identities. onEquipItem
/// performs the real attribute/effect/sound/game-event callbacks synchronously.
pub trait MobEquipmentSetterHost {
    type Error;
    type Item;

    fn verify_equipped_item(&mut self, item: &Self::Item) -> Result<(), Self::Error>;
    fn replace_hand_item(
        &mut self,
        index: usize,
        item: &Self::Item,
    ) -> Result<Self::Item, Self::Error>;
    fn replace_armor_item(
        &mut self,
        index: usize,
        item: &Self::Item,
    ) -> Result<Self::Item, Self::Error>;
    fn replace_body_armor_item(&mut self, item: &Self::Item) -> Result<Self::Item, Self::Error>;
    fn on_equip_item(
        &mut self,
        slot: EquipmentSlot,
        previous: &Self::Item,
        current: &Self::Item,
    ) -> Result<(), Self::Error>;
}

/// Base Mob.setItemSlot, source lines 975–985. Verification precedes replacement,
/// and the canonical new slot is already installed during onEquipItem.
pub fn set_item_slot<H: MobEquipmentSetterHost + ?Sized>(
    host: &mut H,
    slot: EquipmentSlot,
    item: &H::Item,
) -> Result<(), H::Error> {
    host.verify_equipped_item(item)?;
    let previous = match slot {
        EquipmentSlot::MainHand => host.replace_hand_item(0, item)?,
        EquipmentSlot::OffHand => host.replace_hand_item(1, item)?,
        EquipmentSlot::Feet => host.replace_armor_item(0, item)?,
        EquipmentSlot::Legs => host.replace_armor_item(1, item)?,
        EquipmentSlot::Chest => host.replace_armor_item(2, item)?,
        EquipmentSlot::Head => host.replace_armor_item(3, item)?,
        EquipmentSlot::Body => host.replace_body_armor_item(item)?,
    };
    host.on_equip_item(slot, &previous, item)
}

/// Raw backing field access to actual Mob hand/armor/body chances. Values are
/// neither normalized nor clamped, including NaNs, infinities and signed zero.
pub trait MobDropChanceHost {
    type Error;

    fn hand_drop_chance(&mut self, index: usize) -> Result<f32, Self::Error>;
    fn armor_drop_chance(&mut self, index: usize) -> Result<f32, Self::Error>;
    fn body_armor_drop_chance(&mut self) -> Result<f32, Self::Error>;
    fn set_hand_drop_chance(&mut self, index: usize, value: f32) -> Result<(), Self::Error>;
    fn set_armor_drop_chance(&mut self, index: usize, value: f32) -> Result<(), Self::Error>;
    fn set_body_armor_drop_chance(&mut self, value: f32) -> Result<(), Self::Error>;
}

/// Source lines 1017–1020, using the existing seven-slot enum.
pub fn get_equipment_drop_chance<H: MobDropChanceHost + ?Sized>(
    host: &mut H,
    slot: EquipmentSlot,
) -> Result<f32, H::Error> {
    match slot {
        EquipmentSlot::MainHand => host.hand_drop_chance(0),
        EquipmentSlot::OffHand => host.hand_drop_chance(1),
        EquipmentSlot::Feet => host.armor_drop_chance(0),
        EquipmentSlot::Legs => host.armor_drop_chance(1),
        EquipmentSlot::Chest => host.armor_drop_chance(2),
        EquipmentSlot::Head => host.armor_drop_chance(3),
        EquipmentSlot::Body => host.body_armor_drop_chance(),
    }
}

/// Source lines 1224–1229; setGuaranteedDrop646–651 uses the same fields with2.
pub fn set_drop_chance<H: MobDropChanceHost + ?Sized>(
    host: &mut H,
    slot: EquipmentSlot,
    value: f32,
) -> Result<(), H::Error> {
    match slot {
        EquipmentSlot::MainHand => host.set_hand_drop_chance(0, value),
        EquipmentSlot::OffHand => host.set_hand_drop_chance(1, value),
        EquipmentSlot::Feet => host.set_armor_drop_chance(0, value),
        EquipmentSlot::Legs => host.set_armor_drop_chance(1, value),
        EquipmentSlot::Chest => host.set_armor_drop_chance(2, value),
        EquipmentSlot::Head => host.set_armor_drop_chance(3, value),
        EquipmentSlot::Body => host.set_body_armor_drop_chance(value),
    }
}

pub fn set_guaranteed_drop<H: MobDropChanceHost + ?Sized>(
    host: &mut H,
    slot: EquipmentSlot,
) -> Result<(), H::Error> {
    set_drop_chance(host, slot, 2.0)
}

pub trait MobGuaranteedEquipHost {
    type Error;
    type Item;

    fn virtual_set_item_slot(
        &mut self,
        slot: EquipmentSlot,
        item: &Self::Item,
    ) -> Result<(), Self::Error>;
    fn virtual_set_guaranteed_drop(&mut self, slot: EquipmentSlot) -> Result<(), Self::Error>;
    fn set_persistence_required(&mut self, value: bool) -> Result<(), Self::Error>;
}

/// Source lines 640–643: complete actual virtual slot setter, guaranteed drop,
/// then persistenceRequired=true. A failed earlier callback stops later writes.
pub fn set_item_slot_and_drop_when_killed<H: MobGuaranteedEquipHost + ?Sized>(
    host: &mut H,
    slot: EquipmentSlot,
    item: &H::Item,
) -> Result<(), H::Error> {
    host.virtual_set_item_slot(slot, item)?;
    host.virtual_set_guaranteed_drop(slot)?;
    host.set_persistence_required(true)
}

/// The result is the actual source local HashSet of slots rejected by the
/// predicate, not an invented list of all undropped slots. Its real construction
/// and add/return semantics belong to the supplied collection owner. Rust set
/// iteration order is not substituted for Java HashSet behavior.
pub trait MobPreservedEquipmentHost {
    type Error;
    type Item;
    type SlotSet;

    fn new_slot_set(&mut self) -> Result<Self::SlotSet, Self::Error>;
    fn slot_set_add(
        &mut self,
        set: &mut Self::SlotSet,
        slot: EquipmentSlot,
    ) -> Result<(), Self::Error>;
    fn item_by_slot(&mut self, slot: EquipmentSlot) -> Result<Self::Item, Self::Error>;
    fn item_is_empty(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
    fn equipment_drop_chance(&mut self, slot: EquipmentSlot) -> Result<f32, Self::Error>;
    fn set_item_slot_empty(&mut self, slot: EquipmentSlot) -> Result<(), Self::Error>;
    fn spawn_at_location(&mut self, item: &Self::Item) -> Result<(), Self::Error>;
}

/// Mob.dropPreservedEquipment(predicate), source lines 1035–1052. The predicate
/// receives the retained actual stack and actual host for its real context. An
/// accepted chance>1 stack is cleared BEFORE spawn, unlike death equipment.
pub fn drop_preserved_equipment_if<H, P>(
    host: &mut H,
    mut predicate: P,
) -> Result<H::SlotSet, H::Error>
where
    H: MobPreservedEquipmentHost + ?Sized,
    P: FnMut(&mut H, &H::Item) -> Result<bool, H::Error>,
{
    let mut excluded = host.new_slot_set()?;
    for slot in EquipmentSlot::ALL {
        let item = host.item_by_slot(slot)?;
        if host.item_is_empty(&item)? {
            continue;
        }
        if !predicate(host, &item)? {
            host.slot_set_add(&mut excluded, slot)?;
            continue;
        }
        let chance = f64::from(host.equipment_drop_chance(slot)?);
        if chance > 1.0 {
            host.set_item_slot_empty(slot)?;
            host.spawn_at_location(&item)?;
        }
    }
    Ok(excluded)
}

/// Source lines 1025–1026. Its actual lambda always returns true, and the source
/// ignores the returned set. This does not supply a default gameplay predicate.
pub fn drop_preserved_equipment<H: MobPreservedEquipmentHost + ?Sized>(
    host: &mut H,
) -> Result<(), H::Error> {
    let _ = drop_preserved_equipment_if(host, |_, _| Ok(true))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::LegacyRandomSource;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Debug)]
    struct Stack {
        label: &'static str,
        empty: bool,
        prevent: bool,
        damageable: bool,
        maximum: i32,
        damage: i32,
    }
    type Item = Rc<RefCell<Stack>>;
    fn stack(label: &'static str) -> Item {
        Rc::new(RefCell::new(Stack {
            label,
            empty: false,
            prevent: false,
            damageable: false,
            maximum: 100,
            damage: 0,
        }))
    }

    // Controlled canonical-stack/field callback recorder, not a production
    // actor, enchantment evaluator, loot engine, world-add or HashSet engine.
    struct Host {
        slots: [Item; 7],
        empty: Item,
        chances: [f32; 7],
        random: LegacyRandomSource,
        xp: i32,
        living_causer: bool,
        server_level: bool,
        modified_chance: Option<f32>,
        persistent: bool,
        spawned: Vec<Item>,
        spawn_saw_equipped_item: Vec<bool>,
        fail_spawn_after_add: bool,
        fail: Option<String>,
        custom_table: bool,
        table_seed: i64,
        calls: Vec<String>,
    }
    impl Host {
        fn new() -> Self {
            let empty = stack("EMPTY");
            empty.borrow_mut().empty = true;
            Self {
                slots: std::array::from_fn(|_| empty.clone()),
                empty,
                chances: [0.0; 7],
                random: LegacyRandomSource::new(38),
                xp: 5,
                living_causer: true,
                server_level: true,
                modified_chance: None,
                persistent: false,
                spawned: Vec::new(),
                spawn_saw_equipped_item: Vec::new(),
                fail_spawn_after_add: false,
                fail: None,
                custom_table: true,
                table_seed: 88,
                calls: Vec::new(),
            }
        }
        fn record(&mut self, event: impl Into<String>) -> Result<(), &'static str> {
            let event = event.into();
            let fail = self.fail.as_deref() == Some(event.as_str());
            self.calls.push(event);
            if fail {
                Err("controlled owner failure")
            } else {
                Ok(())
            }
        }
        fn install(&mut self, slot: EquipmentSlot, name: &'static str, chance: f32) -> Item {
            let item = stack(name);
            self.slots[slot as usize] = item.clone();
            self.chances[slot as usize] = chance;
            item
        }
        fn replace(&mut self, slot: EquipmentSlot, item: &Item) -> Result<Item, &'static str> {
            self.record(format!("replace:{slot:?}"))?;
            Ok(std::mem::replace(
                &mut self.slots[slot as usize],
                item.clone(),
            ))
        }
    }
    impl MobEntityRandomHost for Host {
        type Error = &'static str;
        type Random = usize;
        fn entity_random(&mut self) -> Result<usize, Self::Error> {
            self.record("random_handle")?;
            Ok(333)
        }
        fn random_next_f32(&mut self, random: &usize) -> Result<f32, Self::Error> {
            assert_eq!(*random, 333);
            self.record("next_float")?;
            Ok(self.random.next_f32())
        }
        fn random_next_bounded_i32(
            &mut self,
            random: &usize,
            bound: i32,
        ) -> Result<i32, Self::Error> {
            assert_eq!(*random, 333);
            self.record(format!("next_int:{bound}"))?;
            self.random
                .next_bounded_i32(bound)
                .map_err(|_| "invalid source bound")
        }
    }
    impl MobEquipmentDeathHost for Host {
        type Source = u32;
        type ServerLevel = u32;
        type Item = Item;
        type LivingEntity = u32;
        fn item_by_slot(&mut self, slot: EquipmentSlot) -> Result<Item, Self::Error> {
            self.record(format!("item:{slot:?}"))?;
            Ok(self.slots[slot as usize].clone())
        }
        fn equipment_drop_chance(&mut self, slot: EquipmentSlot) -> Result<f32, Self::Error> {
            self.record(format!("chance:{slot:?}"))?;
            Ok(self.chances[slot as usize])
        }
        fn causing_living_entity(&mut self, source: &u32) -> Result<Option<u32>, Self::Error> {
            assert_eq!(*source, 77);
            self.record("causing_living")?;
            Ok(self.living_causer.then_some(44))
        }
        fn current_server_level(&mut self) -> Result<Option<u32>, Self::Error> {
            self.record("current_level")?;
            Ok(self.server_level.then_some(9))
        }
        fn process_equipment_drop_chance(
            &mut self,
            level: &u32,
            causing: &u32,
            source: &u32,
            chance: f32,
        ) -> Result<f32, Self::Error> {
            assert_eq!((*level, *causing, *source), (9, 44, 77));
            self.record("modify_chance")?;
            Ok(self.modified_chance.unwrap_or(chance))
        }
        fn item_is_empty(&mut self, item: &Item) -> Result<bool, Self::Error> {
            self.record(format!("empty:{}", item.borrow().label))?;
            Ok(item.borrow().empty)
        }
        fn has_prevent_equipment_drop(&mut self, item: &Item) -> Result<bool, Self::Error> {
            self.record(format!("prevent:{}", item.borrow().label))?;
            Ok(item.borrow().prevent)
        }
        fn item_is_damageable(&mut self, item: &Item) -> Result<bool, Self::Error> {
            self.record(format!("damageable:{}", item.borrow().label))?;
            Ok(item.borrow().damageable)
        }
        fn item_max_damage(&mut self, item: &Item) -> Result<i32, Self::Error> {
            self.record(format!("max:{}", item.borrow().label))?;
            Ok(item.borrow().maximum)
        }
        fn item_set_damage(&mut self, item: &Item, damage: i32) -> Result<(), Self::Error> {
            self.record(format!("damage:{}:{damage}", item.borrow().label))?;
            item.borrow_mut().damage = damage;
            Ok(())
        }
        fn spawn_at_location(&mut self, item: &Item) -> Result<(), Self::Error> {
            self.record(format!("spawn:{}", item.borrow().label))?;
            self.spawn_saw_equipped_item
                .push(self.slots.iter().any(|s| Rc::ptr_eq(s, item)));
            self.spawned.push(item.clone());
            if self.fail_spawn_after_add {
                Err("world callback failed after add")
            } else {
                Ok(())
            }
        }
        fn set_item_slot_empty(&mut self, slot: EquipmentSlot) -> Result<(), Self::Error> {
            self.record(format!("clear:{slot:?}"))?;
            let empty = self.empty.clone();
            set_item_slot(self, slot, &empty)
        }
    }
    impl MobEquipmentXpHost for Host {
        type Item = Item;
        fn xp_reward(&mut self) -> Result<i32, Self::Error> {
            self.record("xp")?;
            Ok(self.xp)
        }
        fn armor_size(&mut self) -> Result<i32, Self::Error> {
            self.record("armor_size")?;
            Ok(4)
        }
        fn armor_item(&mut self, index: i32) -> Result<Item, Self::Error> {
            self.record(format!("armor:{index}"))?;
            Ok(self.slots[index as usize + 2].clone())
        }
        fn armor_drop_chance(&mut self, index: i32) -> Result<f32, Self::Error> {
            self.record(format!("armor_chance:{index}"))?;
            Ok(self.chances[index as usize + 2])
        }
        fn hand_size(&mut self) -> Result<i32, Self::Error> {
            self.record("hand_size")?;
            Ok(2)
        }
        fn hand_item(&mut self, index: i32) -> Result<Item, Self::Error> {
            self.record(format!("hand:{index}"))?;
            Ok(self.slots[index as usize].clone())
        }
        fn hand_drop_chance(&mut self, index: i32) -> Result<f32, Self::Error> {
            self.record(format!("hand_chance:{index}"))?;
            Ok(self.chances[index as usize])
        }
        fn body_armor_item(&mut self) -> Result<Item, Self::Error> {
            self.record("body")?;
            Ok(self.slots[6].clone())
        }
        fn body_armor_drop_chance(&mut self) -> Result<f32, Self::Error> {
            self.record("body_chance")?;
            Ok(self.chances[6])
        }
        fn item_is_empty(&mut self, item: &Item) -> Result<bool, Self::Error> {
            MobEquipmentDeathHost::item_is_empty(self, item)
        }
    }
    impl MobEquipmentSetterHost for Host {
        type Error = &'static str;
        type Item = Item;
        fn verify_equipped_item(&mut self, item: &Item) -> Result<(), Self::Error> {
            self.record(format!("verify:{}", item.borrow().label))
        }
        fn replace_hand_item(&mut self, index: usize, item: &Item) -> Result<Item, Self::Error> {
            self.replace(EquipmentSlot::ALL[index], item)
        }
        fn replace_armor_item(&mut self, index: usize, item: &Item) -> Result<Item, Self::Error> {
            self.replace(EquipmentSlot::ALL[index + 2], item)
        }
        fn replace_body_armor_item(&mut self, item: &Item) -> Result<Item, Self::Error> {
            self.replace(EquipmentSlot::Body, item)
        }
        fn on_equip_item(
            &mut self,
            slot: EquipmentSlot,
            previous: &Item,
            current: &Item,
        ) -> Result<(), Self::Error> {
            assert!(Rc::ptr_eq(&self.slots[slot as usize], current));
            self.record(format!(
                "on_equip:{slot:?}:{}->{}",
                previous.borrow().label,
                current.borrow().label
            ))
        }
    }
    impl MobDropChanceHost for Host {
        type Error = &'static str;
        fn hand_drop_chance(&mut self, index: usize) -> Result<f32, Self::Error> {
            self.record(format!("raw_hand:{index}"))?;
            Ok(self.chances[index])
        }
        fn armor_drop_chance(&mut self, index: usize) -> Result<f32, Self::Error> {
            self.record(format!("raw_armor:{index}"))?;
            Ok(self.chances[index + 2])
        }
        fn body_armor_drop_chance(&mut self) -> Result<f32, Self::Error> {
            self.record("raw_body")?;
            Ok(self.chances[6])
        }
        fn set_hand_drop_chance(&mut self, index: usize, value: f32) -> Result<(), Self::Error> {
            self.record(format!("raw_set_hand:{index}"))?;
            self.chances[index] = value;
            Ok(())
        }
        fn set_armor_drop_chance(&mut self, index: usize, value: f32) -> Result<(), Self::Error> {
            self.record(format!("raw_set_armor:{index}"))?;
            self.chances[index + 2] = value;
            Ok(())
        }
        fn set_body_armor_drop_chance(&mut self, value: f32) -> Result<(), Self::Error> {
            self.record("raw_set_body")?;
            self.chances[6] = value;
            Ok(())
        }
    }
    impl MobGuaranteedEquipHost for Host {
        type Error = &'static str;
        type Item = Item;
        fn virtual_set_item_slot(
            &mut self,
            slot: EquipmentSlot,
            item: &Item,
        ) -> Result<(), Self::Error> {
            self.record("virtual_set")?;
            set_item_slot(self, slot, item)
        }
        fn virtual_set_guaranteed_drop(&mut self, slot: EquipmentSlot) -> Result<(), Self::Error> {
            self.record("virtual_guarantee")?;
            set_guaranteed_drop(self, slot)
        }
        fn set_persistence_required(&mut self, value: bool) -> Result<(), Self::Error> {
            self.record(format!("persistent:{value}"))?;
            self.persistent = value;
            Ok(())
        }
    }
    impl MobPreservedEquipmentHost for Host {
        type Error = &'static str;
        type Item = Item;
        type SlotSet = Rc<RefCell<Vec<EquipmentSlot>>>;
        fn new_slot_set(&mut self) -> Result<Self::SlotSet, Self::Error> {
            self.record("new_slot_set")?;
            Ok(Rc::new(RefCell::new(Vec::new())))
        }
        fn slot_set_add(
            &mut self,
            set: &mut Self::SlotSet,
            slot: EquipmentSlot,
        ) -> Result<(), Self::Error> {
            self.record(format!("excluded:{slot:?}"))?;
            set.borrow_mut().push(slot);
            Ok(())
        }
        fn item_by_slot(&mut self, slot: EquipmentSlot) -> Result<Item, Self::Error> {
            MobEquipmentDeathHost::item_by_slot(self, slot)
        }
        fn item_is_empty(&mut self, item: &Item) -> Result<bool, Self::Error> {
            MobEquipmentDeathHost::item_is_empty(self, item)
        }
        fn equipment_drop_chance(&mut self, slot: EquipmentSlot) -> Result<f32, Self::Error> {
            MobEquipmentDeathHost::equipment_drop_chance(self, slot)
        }
        fn set_item_slot_empty(&mut self, slot: EquipmentSlot) -> Result<(), Self::Error> {
            MobEquipmentDeathHost::set_item_slot_empty(self, slot)
        }
        fn spawn_at_location(&mut self, item: &Item) -> Result<(), Self::Error> {
            MobEquipmentDeathHost::spawn_at_location(self, item)
        }
    }
    impl MobLootTableOverrideHost for Host {
        type Error = &'static str;
        type Source = u32;
        fn inherited_drop_from_loot_table(
            &mut self,
            source: &u32,
            recently_hit: bool,
        ) -> Result<(), Self::Error> {
            assert_eq!(*source, 77);
            self.record(format!("real_loot_engine:{recently_hit}"))
        }
        fn clear_custom_loot_table(&mut self) -> Result<(), Self::Error> {
            self.record("clear_custom_table")?;
            self.custom_table = false;
            Ok(())
        }
    }

    #[test]
    fn raw_guaranteed_flag_precedes_enchantment_and_empty_checks_and_spawn_then_clear() {
        let mut host = Host::new();
        let main = host.install(EquipmentSlot::MainHand, "main", 2.0);
        main.borrow_mut().damageable = true;
        host.install(EquipmentSlot::OffHand, "off", 0.5);
        host.chances[EquipmentSlot::Feet as usize] = 0.085; // Empty still processes enchantments.
        host.install(EquipmentSlot::Head, "zero", -0.0);
        host.modified_chance = Some(2.0);
        let mut expected_random = host.random.clone();
        expected_random.next_f32();
        drop_custom_death_loot(&mut host, &555, &77, false).unwrap();
        assert_eq!(host.random.state48(), expected_random.state48());
        assert_eq!(host.spawned.len(), 1);
        assert!(Rc::ptr_eq(&host.spawned[0], &main));
        assert_eq!(host.spawn_saw_equipped_item, [true]);
        assert!(Rc::ptr_eq(&host.slots[0], &host.empty));
        assert!(!host.calls.iter().any(|c| c.starts_with("damageable:")));
        assert_eq!(
            host.calls.iter().filter(|c| *c == "modify_chance").count(),
            3
        );
        let first_empty = host.calls.iter().position(|c| c == "empty:main").unwrap();
        assert_eq!(
            &host.calls[first_empty - 3..first_empty],
            ["causing_living", "current_level", "modify_chance"]
        );
        let spawn = host.calls.iter().position(|c| c == "spawn:main").unwrap();
        assert_eq!(
            &host.calls[spawn..spawn + 5],
            [
                "spawn:main",
                "clear:MainHand",
                "verify:EMPTY",
                "replace:MainHand",
                "on_equip:MainHand:main->EMPTY"
            ]
        );
        assert_eq!(
            host.calls
                .iter()
                .filter(|c| c.starts_with("item:"))
                .cloned()
                .collect::<Vec<_>>(),
            [
                "item:MainHand",
                "item:OffHand",
                "item:Feet",
                "item:Legs",
                "item:Chest",
                "item:Head",
                "item:Body",
            ]
        );
        assert_eq!(host.calls.last().unwrap(), "chance:Body");
    }

    #[test]
    fn nested_durability_draws_and_spawn_error_keep_actual_item_and_rng_prefix() {
        let mut host = Host::new();
        let item = host.install(EquipmentSlot::MainHand, "tool", 1.0);
        item.borrow_mut().damageable = true;
        host.modified_chance = Some(2.0);
        host.fail_spawn_after_add = true;
        let mut expected = host.random.clone();
        expected.next_f32();
        let inner = expected.next_bounded_i32(97).unwrap();
        let outer = expected.next_bounded_i32(1 + inner).unwrap();
        assert_eq!(
            drop_custom_death_loot(&mut host, &555, &77, true),
            Err("world callback failed after add")
        );
        assert_eq!(host.random.state48(), expected.state48());
        assert_eq!(item.borrow().damage, 100 - outer);
        assert!(Rc::ptr_eq(&host.slots[0], &item));
        assert!(Rc::ptr_eq(&host.spawned[0], &item));
        assert_eq!(host.calls.last().unwrap(), "spawn:tool");
        let first_max = host.calls.iter().position(|c| c == "max:tool").unwrap();
        assert_eq!(
            &host.calls[first_max..first_max + 6],
            [
                "max:tool",
                "random_handle",
                "random_handle",
                "max:tool",
                "next_int:97",
                &format!("next_int:{}", 1 + inner),
            ]
        );
        assert!(!host.calls.iter().any(|c| c.starts_with("clear:")));

        let mut failing_clear = Host::new();
        let item = failing_clear.install(EquipmentSlot::MainHand, "tool", 2.0);
        failing_clear.fail = Some("on_equip:MainHand:tool->EMPTY".to_owned());
        assert_eq!(
            drop_custom_death_loot(&mut failing_clear, &555, &77, false),
            Err("controlled owner failure")
        );
        assert!(Rc::ptr_eq(&failing_clear.spawned[0], &item));
        assert!(Rc::ptr_eq(&failing_clear.slots[0], &failing_clear.empty));
        assert!(!failing_clear.calls.iter().any(|c| c == "item:OffHand"));
    }

    #[test]
    fn xp_scans_actual_armor_then_hands_then_body_and_keeps_raw_chance_semantics() {
        let mut host = Host::new();
        host.xp = i32::MAX;
        host.install(EquipmentSlot::Feet, "feet", 1.0);
        host.install(EquipmentSlot::Legs, "guaranteed", 2.0);
        host.install(EquipmentSlot::MainHand, "nan", f32::NAN);
        host.install(EquipmentSlot::OffHand, "off", 0.0);
        host.install(EquipmentSlot::Body, "body", 0.085);
        let mut expected = host.random.clone();
        let mut reward = i32::MAX;
        for _ in 0..3 {
            reward = reward.wrapping_add(1 + expected.next_bounded_i32(3).unwrap());
        }
        assert_eq!(get_base_experience_reward(&mut host), Ok(reward));
        assert_eq!(host.random.state48(), expected.state48());
        assert_eq!(
            host.calls
                .iter()
                .filter(|c| c.starts_with("armor:") || c.starts_with("hand:") || *c == "body")
                .cloned()
                .collect::<Vec<_>>(),
            ["armor:0", "armor:1", "armor:2", "armor:3", "hand:0", "hand:1", "body",]
        );
        assert_eq!(host.calls.iter().filter(|c| *c == "next_int:3").count(), 3);
        assert!(!host
            .calls
            .iter()
            .any(|c| c == "armor_chance:2" || c == "armor_chance:3"));
        for xp in [0, -7] {
            host.xp = xp;
            host.calls.clear();
            let before = host.random.state48();
            assert_eq!(get_base_experience_reward(&mut host), Ok(xp));
            assert_eq!(host.calls, ["xp", "xp"]);
            assert_eq!(host.random.state48(), before);
        }
    }

    #[test]
    fn setters_raw_chances_and_guarantee_preserve_canonical_swap_before_callbacks() {
        let mut host = Host::new();
        let old = host.install(EquipmentSlot::Body, "old", 0.085);
        let new = stack("new");
        host.fail = Some("on_equip:Body:old->new".to_owned());
        assert_eq!(
            set_item_slot(&mut host, EquipmentSlot::Body, &new),
            Err("controlled owner failure")
        );
        assert_eq!(
            host.calls,
            ["verify:new", "replace:Body", "on_equip:Body:old->new"]
        );
        assert!(Rc::ptr_eq(&host.slots[6], &new));
        assert!(!Rc::ptr_eq(&old, &new));
        host.calls.clear();
        host.fail = None;
        for slot in EquipmentSlot::ALL {
            set_drop_chance(&mut host, slot, -0.0).unwrap();
            assert_eq!(
                get_equipment_drop_chance(&mut host, slot)
                    .unwrap()
                    .to_bits(),
                (-0.0_f32).to_bits()
            );
        }
        set_drop_chance(&mut host, EquipmentSlot::Body, f32::NAN).unwrap();
        assert!(get_equipment_drop_chance(&mut host, EquipmentSlot::Body)
            .unwrap()
            .is_nan());
        host.calls.clear();
        set_item_slot_and_drop_when_killed(&mut host, EquipmentSlot::OffHand, &new).unwrap();
        assert_eq!(
            host.calls,
            [
                "virtual_set",
                "verify:new",
                "replace:OffHand",
                "on_equip:OffHand:EMPTY->new",
                "virtual_guarantee",
                "raw_set_hand:1",
                "persistent:true"
            ]
        );
        assert_eq!(host.chances[1], 2.0);
        assert!(host.persistent);
    }

    #[test]
    fn preserved_equipment_clears_before_spawn_and_returns_only_predicate_rejections() {
        let mut host = Host::new();
        let main = host.install(EquipmentSlot::MainHand, "main", 2.0);
        host.install(EquipmentSlot::OffHand, "reject", 2.0);
        let body = host.install(EquipmentSlot::Body, "ordinary", 1.0);
        let before = host.random.state48();
        let excluded = drop_preserved_equipment_if(&mut host, |host, item| {
            host.record(format!("predicate:{}", item.borrow().label))?;
            Ok(item.borrow().label != "reject")
        })
        .unwrap();
        assert_eq!(&*excluded.borrow(), &[EquipmentSlot::OffHand]);
        assert!(Rc::ptr_eq(&host.spawned[0], &main));
        assert_eq!(host.spawn_saw_equipped_item, [false]);
        assert!(Rc::ptr_eq(&host.slots[6], &body));
        assert_eq!(host.random.state48(), before);
        let clear = host
            .calls
            .iter()
            .position(|c| c == "clear:MainHand")
            .unwrap();
        let spawn = host.calls.iter().position(|c| c == "spawn:main").unwrap();
        assert!(clear < spawn);
        assert!(!host.calls.iter().any(|c| c == "chance:OffHand"));

        let mut failing = Host::new();
        failing.install(EquipmentSlot::MainHand, "main", 2.0);
        failing.fail_spawn_after_add = true;
        assert_eq!(
            drop_preserved_equipment(&mut failing),
            Err("world callback failed after add")
        );
        assert!(Rc::ptr_eq(&failing.slots[0], &failing.empty));
        assert_eq!(failing.spawn_saw_equipped_item, [false]);
    }

    #[test]
    fn custom_table_clear_follows_actual_engine_and_preserves_seed_or_failure_prefix() {
        let mut host = Host::new();
        host.fail = Some("real_loot_engine:true".to_owned());
        assert_eq!(
            drop_from_loot_table(&mut host, &77, true),
            Err("controlled owner failure")
        );
        assert!(host.custom_table);
        assert_eq!(host.table_seed, 88);
        assert_eq!(host.calls, ["real_loot_engine:true"]);
        host.calls.clear();
        host.fail = None;
        drop_from_loot_table(&mut host, &77, false).unwrap();
        assert!(!host.custom_table);
        assert_eq!(host.table_seed, 88);
        assert_eq!(host.calls, ["real_loot_engine:false", "clear_custom_table"]);
    }
}
