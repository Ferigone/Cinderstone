//! Owner traits for everything a loot table reads that it does not own.
//!
//! The loot engine owns table structure, weighting, RNG consumption order and
//! item mutation. The world, entities, damage sources, block entities and the
//! recipe manager belong to other owners (Level, entity runtime, damage
//! pipeline, native block entities, recipe book). They answer through these
//! traits. Every method that has no source-verified native owner yet defaults
//! to `LootError::Unsupported`, so a missing owner can never silently become a
//! wrong answer.

use super::{unsupported, LootTable, Result};
use crate::inventory::{Component, ItemStack};

/// `net.minecraft.world.entity.EquipmentSlot` in Java declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipmentSlot {
    MainHand,
    OffHand,
    Feet,
    Legs,
    Chest,
    Head,
    Body,
}

impl EquipmentSlot {
    pub const ALL: [EquipmentSlot; 7] = [
        Self::MainHand,
        Self::OffHand,
        Self::Feet,
        Self::Legs,
        Self::Chest,
        Self::Head,
        Self::Body,
    ];
}

/// The `ServerLevel` queries loot performs.
pub trait LootLevel {
    /// `Item.isEnabled(level.enabledFeatures())`. Vanilla's feature set enables
    /// every 1.21 item, which is the captured default.
    fn item_enabled(&self, item: &str) -> bool {
        super::facts::facts().is_item_enabled(item)
    }
    fn game_time(&self) -> Result<i64> {
        unsupported("level.game_time")
    }
    fn day_time(&self) -> Result<i64> {
        unsupported("level.day_time")
    }
    fn is_raining(&self) -> Result<bool> {
        unsupported("level.is_raining")
    }
    fn is_thundering(&self) -> Result<bool> {
        unsupported("level.is_thundering")
    }
    fn dimension(&self) -> Result<String> {
        unsupported("level.dimension")
    }
    fn is_loaded(&self, _pos: [i32; 3]) -> Result<bool> {
        unsupported("level.is_loaded")
    }
    /// Global block state ID (the catalog used by `inventory::block_state_*`).
    fn block_state(&self, _pos: [i32; 3]) -> Result<u32> {
        unsupported("level.block_state")
    }
    fn biome(&self, _pos: [i32; 3]) -> Result<String> {
        unsupported("level.biome")
    }
    fn can_see_sky(&self, _pos: [i32; 3]) -> Result<bool> {
        unsupported("level.can_see_sky")
    }
    /// `RecipeManager.getRecipeFor(SMELTING, SingleRecipeInput(stack), level)` and
    /// the recipe's `getResultItem`. `None` when no smelting recipe matches.
    ///
    /// The default answers from the captured vanilla default recipes in
    /// `crate::smelting`; custom data-pack recipes need a host override.
    fn smelting_result(&self, stack: &ItemStack) -> Result<Option<ItemStack>> {
        let recipe = crate::smelting::recipe_for(crate::smelting::CookingKind::Smelting, stack)
            .map_err(|error| super::LootError::Host(error.to_string()))?;
        Ok(recipe.map(|recipe| recipe.result.clone()))
    }
    /// `ExplorationMapFunction.run` tail: `findNearestMapStructure` plus map
    /// allocation. When no structure is found the owner must return `stack`
    /// unchanged (the map item that was passed in).
    #[allow(clippy::too_many_arguments)]
    fn exploration_map(
        &self,
        _origin: [f64; 3],
        _destination: &str,
        _decoration: &str,
        _zoom: i32,
        _search_radius: i32,
        _skip_known_structures: bool,
        _stack: ItemStack,
    ) -> Result<ItemStack> {
        unsupported("level.exploration_map")
    }
    /// Scoreboard value of a score holder for an objective, if tracked.
    fn score(&self, _holder: &str, _objective: &str) -> Result<Option<i32>> {
        unsupported("level.scoreboard")
    }
}

/// Entity facts read by `EntityPredicate` and enchantment slot queries.
pub trait LootEntity {
    /// Registered entity type ID such as `minecraft:zombie`.
    fn type_id(&self) -> Result<String>;
    /// `Entity.isOnFire()`.
    fn is_on_fire(&self) -> Result<bool>;
    /// `entity instanceof LivingEntity`.
    fn is_living(&self) -> bool;
    /// `LivingEntity.getItemBySlot(slot)` (empty stack when the slot is empty).
    fn equipment(&self, slot: EquipmentSlot) -> Result<ItemStack>;

    fn on_ground(&self) -> Result<bool> {
        unsupported("entity.on_ground")
    }
    fn is_crouching(&self) -> Result<bool> {
        unsupported("entity.is_crouching")
    }
    fn is_sprinting(&self) -> Result<bool> {
        unsupported("entity.is_sprinting")
    }
    fn is_swimming(&self) -> Result<bool> {
        unsupported("entity.is_swimming")
    }
    /// `LivingEntity.isFallFlying() || Player.getAbilities().flying`.
    fn is_flying(&self) -> Result<bool> {
        unsupported("entity.is_flying")
    }
    fn is_baby(&self) -> Result<bool> {
        unsupported("entity.is_baby")
    }
    /// `Entity.getScoreboardName()` (UUID string, or the player name).
    fn scoreboard_name(&self) -> Result<String> {
        unsupported("entity.scoreboard_name")
    }
    /// `Some(size)` when the entity is a `Slime` (including magma cubes).
    fn slime_size(&self) -> Result<Option<i32>> {
        Ok(None)
    }
    /// `Some(variant)` when the entity is a `FishingHook`: `isOpenWaterFishing()`.
    fn fishing_hook_open_water(&self) -> Result<Option<bool>> {
        Ok(None)
    }
    /// Variant string for `type_specific` variant predicates (`frog`, `cat`, …);
    /// `None` when the entity is not of that class.
    /// `Entity.getCustomName()` (every entity is `Nameable`) as network NBT; `None` when unnamed.
    fn custom_name_nbt(&self) -> Result<Option<Vec<u8>>> {
        unsupported("entity.custom_name")
    }
    /// `Some(bytes)` when the entity is a `Player`: `ResolvableProfile.STREAM_CODEC` of
    /// `new ResolvableProfile(player.getGameProfile())`.
    fn player_profile(&self) -> Result<Option<Vec<u8>>> {
        unsupported("entity.game_profile")
    }
    fn variant(&self, kind: &str) -> Result<Option<String>> {
        unsupported(format!("entity.variant.{kind}"))
    }
}

/// `net.minecraft.world.damagesource.DamageSource`.
pub trait LootDamageSource {
    /// Registered damage type ID such as `minecraft:player_attack`.
    fn type_id(&self) -> Result<String>;
    fn direct_entity(&self) -> Option<&dyn LootEntity>;
    /// The causing (source) entity.
    fn source_entity(&self) -> Option<&dyn LootEntity>;
    /// `DamageSource.isDirect()`: causing entity identical to the direct entity.
    fn is_direct(&self) -> Result<bool>;
}

/// `BlockEntity.collectComponents()`.
pub trait LootBlockEntity {
    fn collect_components(&self) -> Result<Vec<Component>>;
    /// `None` when the block entity is not `Nameable`; else its `getCustomName()` as network NBT.
    fn nameable_custom_name_nbt(&self) -> Result<Option<Option<Vec<u8>>>> {
        unsupported("block_entity.custom_name")
    }
}

/// Registry lookups performed through `LootContext.getResolver()`.
pub trait LootResolver {
    fn loot_table(&self, key: &str) -> Option<&LootTable>;
    fn predicate(&self, _key: &str) -> Option<&super::Condition> {
        None
    }
    fn item_modifier(&self, _key: &str) -> Option<&super::Function> {
        None
    }
}
