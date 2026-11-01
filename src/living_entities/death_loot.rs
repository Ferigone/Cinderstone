//! Source-ordered Java 1.21 death-loot and Player inventory/XP fragments.
//!
//! These functions compose the real virtual dispatches in LivingEntity
//! (`btn`, lines 1431–1448), Player (`cmx`, 719–733 and 1861–1875), and Inventory
//! (`cmw`, 497–506). They own no inventory, item stack, actor state, RNG, loot
//! table, named random stream, entity index, packet queue or persistence state.
//! Every nonempty operation requires its real owner. Errors preserve the
//! completed prefix; death is not an all-or-nothing inventory transaction.

/// Actual owners for LivingEntity.dropAllDeathLoot. Handles must retain source
/// identity through callbacks and must not resolve to replacement actors/levels.
pub trait LivingDeathLootHost {
    type Error;
    type Source: ?Sized;
    type ServerLevel;
    type Entity;

    fn last_hurt_by_player_time(&mut self) -> Result<i32, Self::Error>;
    fn should_drop_loot(&mut self) -> Result<bool, Self::Error>;
    fn level_do_mob_loot(&mut self, level: &Self::ServerLevel) -> Result<bool, Self::Error>;
    /// Actual virtual dropFromLootTable: source/direct/causing/position context,
    /// server loot registry, seed and named random-sequence ownership included.
    /// Even the vanilla player table's absence of pools is not permission to
    /// omit its real context/registry/named-stream lifecycle.
    fn drop_from_loot_table(
        &mut self,
        source: &Self::Source,
        recently_hit: bool,
    ) -> Result<(), Self::Error>;
    fn drop_custom_death_loot(
        &mut self,
        level: &Self::ServerLevel,
        source: &Self::Source,
        recently_hit: bool,
    ) -> Result<(), Self::Error>;
    /// Actual virtual dropEquipment; Player dispatches to player_drop_equipment.
    fn drop_equipment(&mut self) -> Result<(), Self::Error>;
    /// DamageSource.getEntity (causing entity), read after equipment callbacks.
    fn source_causing_entity(
        &mut self,
        source: &Self::Source,
    ) -> Result<Option<Self::Entity>, Self::Error>;
    /// Actual virtual dropExperience, not an immediate player XP grant.
    fn drop_experience(&mut self, causing: Option<&Self::Entity>) -> Result<(), Self::Error>;
}

/// LivingEntity.dropAllDeathLoot, original lines 1431–1439.
pub fn living_drop_all_death_loot<H: LivingDeathLootHost + ?Sized>(
    host: &mut H,
    level: &H::ServerLevel,
    source: &H::Source,
) -> Result<(), H::Error> {
    let recently_hit = host.last_hurt_by_player_time()? > 0;
    if host.should_drop_loot()? && host.level_do_mob_loot(level)? {
        host.drop_from_loot_table(source, recently_hit)?;
        host.drop_custom_death_loot(level, source, recently_hit)?;
    }
    host.drop_equipment()?;
    let causing = host.source_causing_entity(source)?;
    host.drop_experience(causing.as_ref())
}

/// Actual contexts for LivingEntity.dropExperience. These are observations of
/// canonical fields/virtual methods, never new copies of XP or consumed flags.
pub trait LivingDeathExperienceHost {
    type Error;
    type Entity;
    type ServerLevel;
    type Position;

    /// Snapshot current Level once and retain it only if it is a ServerLevel.
    fn current_server_level(&mut self) -> Result<Option<Self::ServerLevel>, Self::Error>;
    fn was_experience_consumed(&mut self) -> Result<bool, Self::Error>;
    fn is_always_experience_dropper(&mut self) -> Result<bool, Self::Error>;
    fn last_hurt_by_player_time(&mut self) -> Result<i32, Self::Error>;
    fn should_drop_experience(&mut self) -> Result<bool, Self::Error>;
    /// This is a fresh this.level().getGameRules() query, not the retained level
    /// local from current_server_level. It is skipped for always-droppers.
    fn current_level_do_mob_loot(&mut self) -> Result<bool, Self::Error>;
    fn position(&mut self) -> Result<Self::Position, Self::Error>;
    /// Actual final LivingEntity.getExperienceReward: virtual base reward then
    /// EnchantmentHelper.processMobExperience on the causing entity's equipped
    /// enchantments, with this victim, source ServerLevel and real effect RNG.
    fn experience_reward(
        &mut self,
        level: &Self::ServerLevel,
        causing: Option<&Self::Entity>,
    ) -> Result<i32, Self::Error>;
    /// Actual ExperienceOrb.award(level, snapshotted position, amount): integer
    /// denominations, Level RNG group/query/merge order, independent orb RNG,
    /// shared IDs/index/addFreshEntity/tracking/persistence. No furnace recipe,
    /// global Math.random reward or direct player credit may substitute for it.
    fn award_experience_orbs(
        &mut self,
        level: &Self::ServerLevel,
        at: Self::Position,
        amount: i32,
    ) -> Result<(), Self::Error>;
}

/// LivingEntity.dropExperience, original lines 1445–1448. Position is captured
/// before getExperienceReward; callbacks may subsequently move the actor.
pub fn living_drop_experience<H: LivingDeathExperienceHost + ?Sized>(
    host: &mut H,
    causing: Option<&H::Entity>,
) -> Result<(), H::Error> {
    if let Some(level) = host.current_server_level()? {
        if !host.was_experience_consumed()?
            && (host.is_always_experience_dropper()?
                || (host.last_hurt_by_player_time()? > 0
                    && host.should_drop_experience()?
                    && host.current_level_do_mob_loot()?))
        {
            let position = host.position()?;
            let amount = host.experience_reward(&level, causing)?;
            // The source calls award even for zero/negative values; that owner
            // supplies its own while(amount>0) guard.
            host.award_experience_orbs(&level, position, amount)?;
        }
    }
    Ok(())
}

/// The complete original LivingEntity.dropEquipment body (line 1442) is return.
/// This is that inspected body, not a default for a missing virtual executor.
pub fn living_drop_equipment() {}

pub trait PlayerDeathEquipmentHost {
    type Error;
    fn keep_inventory(&mut self) -> Result<bool, Self::Error>;
    /// Actual virtual Player.destroyVanishingCursedItems.
    fn destroy_vanishing_cursed_items(&mut self) -> Result<(), Self::Error>;
    /// The actual actor Inventory.dropAll, after the vanishing pass completes.
    fn inventory_drop_all(&mut self) -> Result<(), Self::Error>;
}

/// Player.dropEquipment719–724: its concrete empty parent runs before the rule
/// read. Creative abilities do not provide an exemption in this source body.
pub fn player_drop_equipment<H: PlayerDeathEquipmentHost + ?Sized>(
    host: &mut H,
) -> Result<(), H::Error> {
    living_drop_equipment();
    if !host.keep_inventory()? {
        host.destroy_vanishing_cursed_items()?;
        host.inventory_drop_all()?;
    }
    Ok(())
}

/// Handles refer to live canonical ItemStacks and their actual Inventory. No
/// snapshot inventory may be published as the live owner during these calls.
pub trait PlayerVanishingHost {
    type Error;
    type Item;
    /// Inventory.getContainerSize; the loop rereads it at every condition.
    fn inventory_size(&mut self) -> Result<i32, Self::Error>;
    fn inventory_item(&mut self, index: i32) -> Result<Self::Item, Self::Error>;
    fn item_is_empty(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
    /// Exact EnchantmentHelper.has(item, PREVENT_EQUIPMENT_DROP). Query actual
    /// registered effect presence for each item enchantment; do not substitute
    /// a vanishing-curse ID, positive-level check or equipment-slot predicate.
    fn has_prevent_equipment_drop(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
    /// Source removes the current slot by index, not a retained stack snapshot.
    /// The returned ItemStack is discarded; no drop, stat or setChanged is added.
    fn remove_item_no_update(&mut self, index: i32) -> Result<(), Self::Error>;
}

/// Player.destroyVanishingCursedItems, original lines 727–733.
pub fn destroy_vanishing_cursed_items<H: PlayerVanishingHost + ?Sized>(
    host: &mut H,
) -> Result<(), H::Error> {
    let mut index = 0_i32;
    while index < host.inventory_size()? {
        let item = host.inventory_item(index)?;
        if !host.item_is_empty(&item)? && host.has_prevent_equipment_drop(&item)? {
            host.remove_item_no_update(index)?;
        }
        index = index.wrapping_add(1);
    }
    Ok(())
}

pub trait PlayerInventoryDeathHost {
    type Error;
    type Compartment;
    type Item;

    /// Retain the actual three source list objects in their immutable constructor
    /// order: items[36], armor[4] (FEET,LEGS,CHEST,HEAD), offhand[1]. Captured list
    /// handles must survive callbacks; do not retarget them to a new inventory.
    fn inventory_compartments(&mut self) -> Result<[Self::Compartment; 3], Self::Error>;
    fn compartment_size(&mut self, compartment: &Self::Compartment) -> Result<i32, Self::Error>;
    fn compartment_item(
        &mut self,
        compartment: &Self::Compartment,
        index: i32,
    ) -> Result<Self::Item, Self::Error>;
    fn item_is_empty(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
    /// Actual virtual Player.drop(item, around=true, trace_item=false), including
    /// ServerPlayer's world-add override. Preserve stack components/identity,
    /// constructor Level/entity RNG and the retained Player RNG. Death drops use
    /// pickupDelay40, no thrower and no trace-item statistic. Its nullable return
    /// is ignored, and its world-add boolean is not an extra slot-clear gate.
    fn player_drop(
        &mut self,
        item: &Self::Item,
        around: bool,
        trace_item: bool,
    ) -> Result<(), Self::Error>;
    /// Source list.set(index, ItemStack.EMPTY), only after the drop returns.
    /// Overwrite the actual current slot even if callbacks changed its value.
    /// Do not inject menu/crafting/cursor clears, packets or Inventory.setChanged.
    fn set_compartment_slot_empty(
        &mut self,
        compartment: &Self::Compartment,
        index: i32,
    ) -> Result<(), Self::Error>;
}

/// Inventory.dropAll497–506. This does not include a carried cursor, crafting
/// inputs/result or ender-chest inventory; those belong to other lifecycles.
pub fn inventory_drop_all<H: PlayerInventoryDeathHost + ?Sized>(
    host: &mut H,
) -> Result<(), H::Error> {
    let compartments = host.inventory_compartments()?;
    for compartment in &compartments {
        let mut index = 0_i32;
        while index < host.compartment_size(compartment)? {
            let item = host.compartment_item(compartment, index)?;
            if !host.item_is_empty(&item)? {
                host.player_drop(&item, true, false)?;
                host.set_compartment_slot_empty(compartment, index)?;
            }
            index = index.wrapping_add(1);
        }
    }
    Ok(())
}

pub trait PlayerBaseExperienceHost {
    type Error;
    fn keep_inventory(&mut self) -> Result<bool, Self::Error>;
    fn is_spectator(&mut self) -> Result<bool, Self::Error>;
    fn experience_level(&mut self) -> Result<i32, Self::Error>;
}

/// Player.getBaseExperienceReward1861–1869; this does not reset any XP fields.
pub fn player_base_experience_reward<H: PlayerBaseExperienceHost + ?Sized>(
    host: &mut H,
) -> Result<i32, H::Error> {
    if host.keep_inventory()? || host.is_spectator()? {
        return Ok(0);
    }
    let amount = host.experience_level()?.wrapping_mul(7);
    Ok(if amount > 100 { 100 } else { amount })
}

/// Complete source Player.isAlwaysExperienceDropper1875.
pub fn player_is_always_experience_dropper() -> bool {
    true
}

/// Java Inventory indices0..40 mapped to the existing native 46-slot menu.
/// Main inventory hotbar precedes ordinary slots; armor is FEET through HEAD.
/// This is an adapter mapping, not another inventory owner or iteration list.
pub const JAVA_INVENTORY_TO_MENU_SLOT: [usize; 41] = [
    36, 37, 38, 39, 40, 41, 42, 43, 44, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 8, 7, 6, 5, 45,
];

#[cfg(test)]
mod tests {
    use super::*;

    struct Host {
        calls: Vec<String>,
        recent: i32,
        keep: bool,
        spectator: bool,
        level: i32,
        consumed: bool,
        position: i32,
        slots: [Vec<i32>; 3],
        fail_drop: Option<i32>,
        fail_custom: bool,
    }
    impl Default for Host {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                recent: 4,
                keep: false,
                spectator: false,
                level: 3,
                consumed: false,
                position: 7,
                slots: [
                    {
                        let mut main = vec![0; 36];
                        main[0] = 1;
                        main[35] = 2;
                        main
                    },
                    vec![3, 0, 0, 0],
                    vec![4],
                ],
                fail_drop: None,
                fail_custom: false,
            }
        }
    }
    impl LivingDeathLootHost for Host {
        type Error = &'static str;
        type Source = u32;
        type ServerLevel = u32;
        type Entity = u32;
        fn last_hurt_by_player_time(&mut self) -> Result<i32, Self::Error> {
            self.calls.push("recent".into());
            Ok(self.recent)
        }
        fn should_drop_loot(&mut self) -> Result<bool, Self::Error> {
            self.calls.push("should_loot".into());
            Ok(true)
        }
        fn level_do_mob_loot(&mut self, l: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*l, 9);
            self.calls.push("mob_loot".into());
            Ok(true)
        }
        fn drop_from_loot_table(&mut self, s: &u32, r: bool) -> Result<(), Self::Error> {
            assert_eq!(*s, 767);
            self.calls.push(format!("table={r}"));
            self.recent = 0;
            Ok(())
        }
        fn drop_custom_death_loot(&mut self, l: &u32, s: &u32, r: bool) -> Result<(), Self::Error> {
            assert_eq!((*l, *s), (9, 767));
            self.calls.push(format!("custom={r}"));
            if self.fail_custom {
                Err("custom failed")
            } else {
                Ok(())
            }
        }
        fn drop_equipment(&mut self) -> Result<(), Self::Error> {
            self.calls.push("equipment".into());
            Ok(())
        }
        fn source_causing_entity(&mut self, s: &u32) -> Result<Option<u32>, Self::Error> {
            assert_eq!(*s, 767);
            self.calls.push("causer".into());
            Ok(Some(12))
        }
        fn drop_experience(&mut self, e: Option<&u32>) -> Result<(), Self::Error> {
            assert_eq!(e, Some(&12));
            self.calls.push("experience".into());
            Ok(())
        }
    }
    impl LivingDeathExperienceHost for Host {
        type Error = &'static str;
        type Entity = u32;
        type ServerLevel = u32;
        type Position = i32;
        fn current_server_level(&mut self) -> Result<Option<u32>, Self::Error> {
            self.calls.push("server_level".into());
            Ok(Some(9))
        }
        fn was_experience_consumed(&mut self) -> Result<bool, Self::Error> {
            self.calls.push("consumed".into());
            Ok(self.consumed)
        }
        fn is_always_experience_dropper(&mut self) -> Result<bool, Self::Error> {
            self.calls.push("always".into());
            Ok(player_is_always_experience_dropper())
        }
        fn last_hurt_by_player_time(&mut self) -> Result<i32, Self::Error> {
            panic!("Player always-dropper skips recent-hit query")
        }
        fn should_drop_experience(&mut self) -> Result<bool, Self::Error> {
            panic!("Player always-dropper skips should-drop query")
        }
        fn current_level_do_mob_loot(&mut self) -> Result<bool, Self::Error> {
            panic!("Player always-dropper skips doMobLoot query")
        }
        fn position(&mut self) -> Result<i32, Self::Error> {
            self.calls.push("position".into());
            Ok(self.position)
        }
        fn experience_reward(&mut self, l: &u32, c: Option<&u32>) -> Result<i32, Self::Error> {
            assert_eq!(*l, 9);
            assert_eq!(c, Some(&12));
            self.calls.push("reward".into());
            self.position = 100;
            player_base_experience_reward(self)
        }
        fn award_experience_orbs(&mut self, l: &u32, p: i32, a: i32) -> Result<(), Self::Error> {
            assert_eq!(*l, 9);
            self.calls.push(format!("award={p},{a}"));
            Ok(())
        }
    }
    impl PlayerBaseExperienceHost for Host {
        type Error = &'static str;
        fn keep_inventory(&mut self) -> Result<bool, Self::Error> {
            self.calls.push("keep".into());
            Ok(self.keep)
        }
        fn is_spectator(&mut self) -> Result<bool, Self::Error> {
            self.calls.push("spectator".into());
            Ok(self.spectator)
        }
        fn experience_level(&mut self) -> Result<i32, Self::Error> {
            self.calls.push("xp_level".into());
            Ok(self.level)
        }
    }
    impl PlayerDeathEquipmentHost for Host {
        type Error = &'static str;
        fn keep_inventory(&mut self) -> Result<bool, Self::Error> {
            self.calls.push("keep".into());
            Ok(self.keep)
        }
        fn destroy_vanishing_cursed_items(&mut self) -> Result<(), Self::Error> {
            destroy_vanishing_cursed_items(self)
        }
        fn inventory_drop_all(&mut self) -> Result<(), Self::Error> {
            inventory_drop_all(self)
        }
    }
    impl PlayerVanishingHost for Host {
        type Error = &'static str;
        type Item = (usize, usize, i32);
        fn inventory_size(&mut self) -> Result<i32, Self::Error> {
            self.calls.push("size".into());
            Ok(self.slots.iter().map(Vec::len).sum::<usize>() as i32)
        }
        fn inventory_item(&mut self, index: i32) -> Result<Self::Item, Self::Error> {
            let mut n = index as usize;
            for c in 0..3 {
                if n < self.slots[c].len() {
                    return Ok((c, n, self.slots[c][n]));
                }
                n -= self.slots[c].len();
            }
            panic!("invalid test inventory index")
        }
        fn item_is_empty(&mut self, i: &Self::Item) -> Result<bool, Self::Error> {
            Ok(i.2 == 0)
        }
        fn has_prevent_equipment_drop(&mut self, i: &Self::Item) -> Result<bool, Self::Error> {
            self.calls.push(format!("prevent={}", i.2));
            if i.2 == 2 {
                self.slots[i.0][i.1] = 20;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        fn remove_item_no_update(&mut self, index: i32) -> Result<(), Self::Error> {
            let i = PlayerVanishingHost::inventory_item(self, index)?;
            self.calls.push(format!("vanish_current={}", i.2));
            self.slots[i.0][i.1] = 0;
            Ok(())
        }
    }
    impl PlayerInventoryDeathHost for Host {
        type Error = &'static str;
        type Compartment = usize;
        type Item = (usize, usize, i32);
        fn inventory_compartments(&mut self) -> Result<[usize; 3], Self::Error> {
            self.calls.push("compartments".into());
            Ok([0, 1, 2])
        }
        fn compartment_size(&mut self, c: &usize) -> Result<i32, Self::Error> {
            Ok(self.slots[*c].len() as i32)
        }
        fn compartment_item(&mut self, c: &usize, i: i32) -> Result<Self::Item, Self::Error> {
            Ok((*c, i as usize, self.slots[*c][i as usize]))
        }
        fn item_is_empty(&mut self, i: &Self::Item) -> Result<bool, Self::Error> {
            Ok(i.2 == 0)
        }
        fn player_drop(
            &mut self,
            i: &Self::Item,
            around: bool,
            trace: bool,
        ) -> Result<(), Self::Error> {
            assert!(around);
            assert!(!trace);
            assert_eq!(self.slots[i.0][i.1], i.2);
            self.calls.push(format!("drop={}", i.2));
            self.slots[i.0][i.1] = i.2 * 10;
            if self.fail_drop == Some(i.2) {
                Err("drop failed after mutation")
            } else {
                Ok(())
            }
        }
        fn set_compartment_slot_empty(&mut self, c: &usize, i: i32) -> Result<(), Self::Error> {
            self.calls
                .push(format!("clear_current={}", self.slots[*c][i as usize]));
            self.slots[*c][i as usize] = 0;
            Ok(())
        }
    }

    #[test]
    fn loot_snapshot_equipment_then_causer_order_and_failure_prefix() {
        let mut h = Host::default();
        living_drop_all_death_loot(&mut h, &9, &767).unwrap();
        assert_eq!(
            h.calls,
            [
                "recent",
                "should_loot",
                "mob_loot",
                "table=true",
                "custom=true",
                "equipment",
                "causer",
                "experience"
            ]
        );
        let mut h = Host {
            fail_custom: true,
            ..Host::default()
        };
        assert_eq!(
            living_drop_all_death_loot(&mut h, &9, &767),
            Err("custom failed")
        );
        assert_eq!(h.recent, 0);
        assert_eq!(h.calls.last().unwrap(), "custom=true");
        assert!(!h.calls.iter().any(|s| s == "equipment"));
    }
    #[test]
    fn vanishing_then_source_compartment_drops_clear_after_each_callback() {
        let mut h = Host::default();
        player_drop_equipment(&mut h).unwrap();
        assert!(h
            .slots
            .iter()
            .all(|compartment| compartment.iter().all(|item| *item == 0)));
        assert!(h.calls.iter().any(|s| s == "vanish_current=20"));
        let drops: Vec<_> = h
            .calls
            .iter()
            .filter(|s| s.starts_with("drop=") || s.starts_with("clear_current="))
            .map(String::as_str)
            .collect();
        assert_eq!(
            drops,
            [
                "drop=1",
                "clear_current=10",
                "drop=3",
                "clear_current=30",
                "drop=4",
                "clear_current=40"
            ]
        );
        assert_eq!(h.calls.iter().filter(|s| *s == "size").count(), 42);
        let mut kept = Host {
            keep: true,
            ..Host::default()
        };
        player_drop_equipment(&mut kept).unwrap();
        assert_eq!(kept.calls, ["keep"]);
        let mut failed = Host {
            fail_drop: Some(3),
            ..Host::default()
        };
        assert_eq!(
            inventory_drop_all(&mut failed),
            Err("drop failed after mutation")
        );
        assert!(failed.slots[0].iter().all(|item| *item == 0));
        assert_eq!(failed.slots[1], vec![30, 0, 0, 0]);
        assert_eq!(failed.slots[2], vec![4]);
        assert_eq!(failed.calls.last().unwrap(), "drop=3");
    }
    #[test]
    fn player_experience_uses_old_position_and_does_not_reset_xp() {
        let mut h = Host::default();
        living_drop_experience(&mut h, Some(&12)).unwrap();
        assert_eq!(
            h.calls,
            [
                "server_level",
                "consumed",
                "always",
                "position",
                "reward",
                "keep",
                "spectator",
                "xp_level",
                "award=7,21"
            ]
        );
        assert_eq!((h.level, h.position), (3, 100));
        let mut kept = Host {
            keep: true,
            ..Host::default()
        };
        living_drop_experience(&mut kept, Some(&12)).unwrap();
        assert_eq!(kept.calls.last().unwrap(), "award=7,0");
        assert!(!kept.calls.iter().any(|s| s == "spectator"));
        let mut consumed = Host {
            consumed: true,
            ..Host::default()
        };
        living_drop_experience(&mut consumed, None).unwrap();
        assert_eq!(consumed.calls, ["server_level", "consumed"]);
        let mut overflow = Host {
            level: 306_783_379,
            ..Host::default()
        };
        assert_eq!(
            player_base_experience_reward(&mut overflow),
            Ok(-2_147_483_643)
        );
    }
}
