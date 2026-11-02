//! `LootContextParam`, parameter sets, `LootParams` and `LootContext`.

use super::host::{LootBlockEntity, LootDamageSource, LootEntity, LootLevel, LootResolver};
use super::{LootError, Result};
use crate::inventory::ItemStack;
use crate::random::RandomSource;
use std::collections::HashMap;

/// `LootContextParams`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Param {
    ThisEntity,
    LastDamagePlayer,
    DamageSource,
    AttackingEntity,
    DirectAttackingEntity,
    Origin,
    BlockState,
    BlockEntity,
    Tool,
    ExplosionRadius,
    EnchantmentLevel,
    EnchantmentActive,
}

impl Param {
    pub fn name(self) -> &'static str {
        match self {
            Self::ThisEntity => "minecraft:this_entity",
            Self::LastDamagePlayer => "minecraft:last_damage_player",
            Self::DamageSource => "minecraft:damage_source",
            Self::AttackingEntity => "minecraft:attacking_entity",
            Self::DirectAttackingEntity => "minecraft:direct_attacking_entity",
            Self::Origin => "minecraft:origin",
            Self::BlockState => "minecraft:block_state",
            Self::BlockEntity => "minecraft:block_entity",
            Self::Tool => "minecraft:tool",
            Self::ExplosionRadius => "minecraft:explosion_radius",
            Self::EnchantmentLevel => "minecraft:enchantment_level",
            Self::EnchantmentActive => "minecraft:enchantment_active",
        }
    }
}

/// `LootContextParamSet`: required and optional parameters of a table type.
#[derive(Debug, PartialEq, Eq)]
pub struct ParamSet {
    pub name: &'static str,
    pub required: &'static [Param],
    pub optional: &'static [Param],
}

impl ParamSet {
    pub fn allows(&self, param: Param) -> bool {
        self.required.contains(&param) || self.optional.contains(&param)
    }
}

use Param::*;

macro_rules! set {
    ($id:ident, $name:literal, [$($req:ident),*], [$($opt:ident),*]) => {
        pub static $id: ParamSet = ParamSet { name: $name, required: &[$($req),*], optional: &[$($opt),*] };
    };
}

set!(EMPTY, "minecraft:empty", [], []);
set!(CHEST, "minecraft:chest", [Origin], [ThisEntity]);
set!(COMMAND, "minecraft:command", [Origin], [ThisEntity]);
set!(SELECTOR, "minecraft:selector", [Origin, ThisEntity], []);
set!(FISHING, "minecraft:fishing", [Origin, Tool], [ThisEntity]);
set!(
    ENTITY,
    "minecraft:entity",
    [ThisEntity, Origin, DamageSource],
    [AttackingEntity, DirectAttackingEntity, LastDamagePlayer]
);
set!(EQUIPMENT, "minecraft:equipment", [Origin, ThisEntity], []);
set!(ARCHAEOLOGY, "minecraft:archaeology", [Origin], [ThisEntity]);
set!(GIFT, "minecraft:gift", [Origin, ThisEntity], []);
set!(BARTER, "minecraft:barter", [ThisEntity], []);
set!(VAULT, "minecraft:vault", [Origin], [ThisEntity]);
set!(ADVANCEMENT_REWARD, "minecraft:advancement_reward", [ThisEntity, Origin], []);
set!(ADVANCEMENT_ENTITY, "minecraft:advancement_entity", [ThisEntity, Origin], []);
set!(
    ADVANCEMENT_LOCATION,
    "minecraft:advancement_location",
    [ThisEntity, Origin, Tool, BlockState],
    []
);
set!(BLOCK_USE, "minecraft:block_use", [ThisEntity, Origin, BlockState], []);
set!(
    GENERIC,
    "minecraft:generic",
    [
        ThisEntity,
        LastDamagePlayer,
        DamageSource,
        AttackingEntity,
        DirectAttackingEntity,
        Origin,
        BlockState,
        BlockEntity,
        Tool,
        ExplosionRadius
    ],
    []
);
set!(
    BLOCK,
    "minecraft:block",
    [BlockState, Origin, Tool],
    [ThisEntity, BlockEntity, ExplosionRadius]
);
set!(SHEARING, "minecraft:shearing", [Origin], [ThisEntity]);
set!(
    ENCHANTED_DAMAGE,
    "minecraft:enchanted_damage",
    [ThisEntity, EnchantmentLevel, Origin, DamageSource],
    [DirectAttackingEntity, AttackingEntity]
);
set!(ENCHANTED_ITEM, "minecraft:enchanted_item", [Tool, EnchantmentLevel], []);
set!(
    ENCHANTED_LOCATION,
    "minecraft:enchanted_location",
    [ThisEntity, EnchantmentLevel, Origin, EnchantmentActive],
    []
);
set!(
    ENCHANTED_ENTITY,
    "minecraft:enchanted_entity",
    [ThisEntity, EnchantmentLevel, Origin],
    []
);
set!(
    HIT_BLOCK,
    "minecraft:hit_block",
    [ThisEntity, EnchantmentLevel, Origin, BlockState],
    []
);

/// `LootContextParamSets` lookup by registered name.
pub fn param_set(name: &str) -> Option<&'static ParamSet> {
    let name = super::facts::canonical(name);
    [
        &EMPTY,
        &CHEST,
        &COMMAND,
        &SELECTOR,
        &FISHING,
        &ENTITY,
        &EQUIPMENT,
        &ARCHAEOLOGY,
        &GIFT,
        &BARTER,
        &VAULT,
        &ADVANCEMENT_REWARD,
        &ADVANCEMENT_ENTITY,
        &ADVANCEMENT_LOCATION,
        &BLOCK_USE,
        &GENERIC,
        &BLOCK,
        &SHEARING,
        &ENCHANTED_DAMAGE,
        &ENCHANTED_ITEM,
        &ENCHANTED_LOCATION,
        &ENCHANTED_ENTITY,
        &HIT_BLOCK,
    ]
    .into_iter()
    .find(|set| set.name == name)
}

/// `LootContext.EntityTarget`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityTarget {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
}

impl EntityTarget {
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "this" => Some(Self::This),
            "attacker" => Some(Self::Attacker),
            "direct_attacker" => Some(Self::DirectAttacker),
            "attacking_player" => Some(Self::AttackingPlayer),
            _ => None,
        }
    }

    pub fn param(self) -> Param {
        match self {
            Self::This => Param::ThisEntity,
            Self::Attacker => Param::AttackingEntity,
            Self::DirectAttacker => Param::DirectAttackingEntity,
            Self::AttackingPlayer => Param::LastDamagePlayer,
        }
    }
}

/// `LootParams`: the parameters one loot roll runs with.
#[derive(Default)]
pub struct LootParams<'a> {
    pub origin: Option<[f64; 3]>,
    /// Global block state ID in the `inventory::block_state_*` catalog.
    pub block_state: Option<u32>,
    pub tool: Option<ItemStack>,
    pub explosion_radius: Option<f32>,
    pub this_entity: Option<&'a dyn LootEntity>,
    pub last_damage_player: Option<&'a dyn LootEntity>,
    pub attacking_entity: Option<&'a dyn LootEntity>,
    pub direct_attacking_entity: Option<&'a dyn LootEntity>,
    pub damage_source: Option<&'a dyn LootDamageSource>,
    pub block_entity: Option<&'a dyn LootBlockEntity>,
    pub enchantment_level: Option<i32>,
    pub enchantment_active: Option<bool>,
    /// `LootParams.dynamicDrops`: stacks a `dynamic` entry with that name emits.
    pub dynamic_drops: HashMap<String, Vec<ItemStack>>,
    pub luck: f32,
}

impl<'a> LootParams<'a> {
    pub fn has(&self, param: Param) -> bool {
        match param {
            Param::ThisEntity => self.this_entity.is_some(),
            Param::LastDamagePlayer => self.last_damage_player.is_some(),
            Param::DamageSource => self.damage_source.is_some(),
            Param::AttackingEntity => self.attacking_entity.is_some(),
            Param::DirectAttackingEntity => self.direct_attacking_entity.is_some(),
            Param::Origin => self.origin.is_some(),
            Param::BlockState => self.block_state.is_some(),
            Param::BlockEntity => self.block_entity.is_some(),
            Param::Tool => self.tool.is_some(),
            Param::ExplosionRadius => self.explosion_radius.is_some(),
            Param::EnchantmentLevel => self.enchantment_level.is_some(),
            Param::EnchantmentActive => self.enchantment_active.is_some(),
        }
    }

    const ALL: [Param; 12] = [
        Param::ThisEntity,
        Param::LastDamagePlayer,
        Param::DamageSource,
        Param::AttackingEntity,
        Param::DirectAttackingEntity,
        Param::Origin,
        Param::BlockState,
        Param::BlockEntity,
        Param::Tool,
        Param::ExplosionRadius,
        Param::EnchantmentLevel,
        Param::EnchantmentActive,
    ];

    /// `LootParams.Builder.create(paramSet)`: every present parameter must be
    /// allowed and every required parameter present.
    pub fn validate(&self, set: &ParamSet) -> Result<()> {
        let disallowed: Vec<&str> = Self::ALL
            .iter()
            .filter(|param| self.has(**param) && !set.allows(**param))
            .map(|param| param.name())
            .collect();
        if !disallowed.is_empty() {
            return Err(LootError::Invalid(format!(
                "Parameters not allowed in this parameter set: {disallowed:?}"
            )));
        }
        let missing: Vec<&str> = set
            .required
            .iter()
            .filter(|param| !self.has(**param))
            .map(|param| param.name())
            .collect();
        if !missing.is_empty() {
            return Err(LootError::Invalid(format!(
                "Missing required parameters: {missing:?}"
            )));
        }
        Ok(())
    }
}

/// Identity of an element on the infinite-loop guard (`LootContext.VisitedEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Visited {
    Table(usize),
    Predicate(String),
    Modifier(String),
}

/// `LootContext`: parameters, the roll's random source, the level and resolver.
pub struct LootContext<'a> {
    pub params: LootParams<'a>,
    random: &'a mut dyn RandomSource,
    level: &'a dyn LootLevel,
    resolver: &'a dyn LootResolver,
    visited: Vec<Visited>,
}

impl<'a> LootContext<'a> {
    /// The caller picks the random source exactly as `LootContext.Builder.create`
    /// does: an explicit seed, the table's `random_sequence`, or the level RNG.
    pub fn new(
        params: LootParams<'a>,
        random: &'a mut dyn RandomSource,
        level: &'a dyn LootLevel,
        resolver: &'a dyn LootResolver,
    ) -> Self {
        Self {
            params,
            random,
            level,
            resolver,
            visited: Vec::new(),
        }
    }

    pub fn random(&mut self) -> &mut dyn RandomSource {
        &mut *self.random
    }

    pub fn level(&self) -> &'a dyn LootLevel {
        self.level
    }

    pub fn resolver(&self) -> &'a dyn LootResolver {
        self.resolver
    }

    /// `LootContext.getLuck()`.
    pub fn luck(&self) -> f32 {
        self.params.luck
    }

    pub fn has_param(&self, param: Param) -> bool {
        self.params.has(param)
    }

    /// `LootContext.getParamOrNull(target.getParam())` for entity parameters.
    pub fn entity(&self, target: EntityTarget) -> Option<&'a dyn LootEntity> {
        match target {
            EntityTarget::This => self.params.this_entity,
            EntityTarget::Attacker => self.params.attacking_entity,
            EntityTarget::DirectAttacker => self.params.direct_attacking_entity,
            EntityTarget::AttackingPlayer => self.params.last_damage_player,
        }
    }

    /// `LootContext.addDynamicDrops`.
    pub fn dynamic_drops(&self, name: &str) -> Option<&Vec<ItemStack>> {
        self.params.dynamic_drops.get(name)
    }

    pub(crate) fn push_visited(&mut self, entry: Visited) -> bool {
        if self.visited.contains(&entry) {
            false
        } else {
            self.visited.push(entry);
            true
        }
    }

    pub(crate) fn pop_visited(&mut self, entry: &Visited) {
        if let Some(index) = self.visited.iter().position(|visited| visited == entry) {
            self.visited.remove(index);
        }
    }
}
