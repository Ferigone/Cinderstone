//! Ordered key specifications from Java 1.21 PotionBrewing.addVanillaMixes.
//!
//! These are resource keys, not registries, holders, item definitions, potion
//! effects, feature owners or recipes invented by a workstation. Every call is
//! resolved by PotionBrewingBuilder through its required actual context.
//!
//! Primary evidence: direct JDK21 javap of pinned cwt.addVanillaMixes138–223,
//! cwt$a.addStartMix267–271, cwv and official mappings. A separate temporary
//! reflection probe executed the original registry/bootstrap with the original
//! VANILLA_SET and observed the exact ordered 3 containers, 2 container mixes
//! and 63 potion mixes after StartMix expansion. This is original registry/table
//! observation, not gameplay, native Main integration or a registry substitute.
//! Server-inner SHA256:
//! 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
//! Official mappings SHA1: 31c77994d96f05ba25a870ada70f47f315330437.

use super::potion_brewing::{PotionBrewingBuilder, PotionBrewingContext};
use anyhow::Result;

/// One original builder call. StartMix stays explicit: its target-potion
/// feature gate precedes the two separately gated generated Mix calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VanillaRegistration {
    Container(&'static str),
    ContainerRecipe {
        from: &'static str,
        ingredient: &'static str,
        to: &'static str,
    },
    Mix {
        from: &'static str,
        ingredient: &'static str,
        to: &'static str,
    },
    StartMix {
        ingredient: &'static str,
        to: &'static str,
    },
}

/// Source registration order: 3 Container, 2 ContainerRecipe, 41 Mix and
/// 11 StartMix calls. With original VANILLA_SET those produce 63 potion mixes.
/// No calls are regrouped by source potion, ingredient or target.
pub const VANILLA_REGISTRATIONS: &[VanillaRegistration] = &[
    VanillaRegistration::Container("minecraft:potion"),
    VanillaRegistration::Container("minecraft:splash_potion"),
    VanillaRegistration::Container("minecraft:lingering_potion"),
    VanillaRegistration::ContainerRecipe {
        from: "minecraft:potion",
        ingredient: "minecraft:gunpowder",
        to: "minecraft:splash_potion",
    },
    VanillaRegistration::ContainerRecipe {
        from: "minecraft:splash_potion",
        ingredient: "minecraft:dragon_breath",
        to: "minecraft:lingering_potion",
    },
    VanillaRegistration::Mix {
        from: "minecraft:water",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:thick",
    },
    VanillaRegistration::Mix {
        from: "minecraft:water",
        ingredient: "minecraft:redstone",
        to: "minecraft:mundane",
    },
    VanillaRegistration::Mix {
        from: "minecraft:water",
        ingredient: "minecraft:nether_wart",
        to: "minecraft:awkward",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:breeze_rod",
        to: "minecraft:wind_charged",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:slime_block",
        to: "minecraft:oozing",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:stone",
        to: "minecraft:infested",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:cobweb",
        to: "minecraft:weaving",
    },
    VanillaRegistration::Mix {
        from: "minecraft:awkward",
        ingredient: "minecraft:golden_carrot",
        to: "minecraft:night_vision",
    },
    VanillaRegistration::Mix {
        from: "minecraft:night_vision",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_night_vision",
    },
    VanillaRegistration::Mix {
        from: "minecraft:night_vision",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:invisibility",
    },
    VanillaRegistration::Mix {
        from: "minecraft:long_night_vision",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:long_invisibility",
    },
    VanillaRegistration::Mix {
        from: "minecraft:invisibility",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_invisibility",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:magma_cream",
        to: "minecraft:fire_resistance",
    },
    VanillaRegistration::Mix {
        from: "minecraft:fire_resistance",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_fire_resistance",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:rabbit_foot",
        to: "minecraft:leaping",
    },
    VanillaRegistration::Mix {
        from: "minecraft:leaping",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_leaping",
    },
    VanillaRegistration::Mix {
        from: "minecraft:leaping",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:strong_leaping",
    },
    VanillaRegistration::Mix {
        from: "minecraft:leaping",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:slowness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:long_leaping",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:long_slowness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:slowness",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_slowness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:slowness",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:strong_slowness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:awkward",
        ingredient: "minecraft:turtle_helmet",
        to: "minecraft:turtle_master",
    },
    VanillaRegistration::Mix {
        from: "minecraft:turtle_master",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_turtle_master",
    },
    VanillaRegistration::Mix {
        from: "minecraft:turtle_master",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:strong_turtle_master",
    },
    VanillaRegistration::Mix {
        from: "minecraft:swiftness",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:slowness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:long_swiftness",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:long_slowness",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:sugar",
        to: "minecraft:swiftness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:swiftness",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_swiftness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:swiftness",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:strong_swiftness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:awkward",
        ingredient: "minecraft:pufferfish",
        to: "minecraft:water_breathing",
    },
    VanillaRegistration::Mix {
        from: "minecraft:water_breathing",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_water_breathing",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:glistering_melon_slice",
        to: "minecraft:healing",
    },
    VanillaRegistration::Mix {
        from: "minecraft:healing",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:strong_healing",
    },
    VanillaRegistration::Mix {
        from: "minecraft:healing",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:harming",
    },
    VanillaRegistration::Mix {
        from: "minecraft:strong_healing",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:strong_harming",
    },
    VanillaRegistration::Mix {
        from: "minecraft:harming",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:strong_harming",
    },
    VanillaRegistration::Mix {
        from: "minecraft:poison",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:harming",
    },
    VanillaRegistration::Mix {
        from: "minecraft:long_poison",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:harming",
    },
    VanillaRegistration::Mix {
        from: "minecraft:strong_poison",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:strong_harming",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:spider_eye",
        to: "minecraft:poison",
    },
    VanillaRegistration::Mix {
        from: "minecraft:poison",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_poison",
    },
    VanillaRegistration::Mix {
        from: "minecraft:poison",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:strong_poison",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:ghast_tear",
        to: "minecraft:regeneration",
    },
    VanillaRegistration::Mix {
        from: "minecraft:regeneration",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_regeneration",
    },
    VanillaRegistration::Mix {
        from: "minecraft:regeneration",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:strong_regeneration",
    },
    VanillaRegistration::StartMix {
        ingredient: "minecraft:blaze_powder",
        to: "minecraft:strength",
    },
    VanillaRegistration::Mix {
        from: "minecraft:strength",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_strength",
    },
    VanillaRegistration::Mix {
        from: "minecraft:strength",
        ingredient: "minecraft:glowstone_dust",
        to: "minecraft:strong_strength",
    },
    VanillaRegistration::Mix {
        from: "minecraft:water",
        ingredient: "minecraft:fermented_spider_eye",
        to: "minecraft:weakness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:weakness",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_weakness",
    },
    VanillaRegistration::Mix {
        from: "minecraft:awkward",
        ingredient: "minecraft:phantom_membrane",
        to: "minecraft:slow_falling",
    },
    VanillaRegistration::Mix {
        from: "minecraft:slow_falling",
        ingredient: "minecraft:redstone",
        to: "minecraft:long_slow_falling",
    },
];

/// All 46 original Potions declarations in pinned class declaration order.
/// The original bootstrap observation found each requiredFeatures field equal
/// to the original VANILLA_SET object. This observation does NOT replace the
/// actual context's mandatory per-call feature checks or resolve these keys.
pub const VANILLA_POTION_KEYS: &[&str] = &[
    "minecraft:water",
    "minecraft:mundane",
    "minecraft:thick",
    "minecraft:awkward",
    "minecraft:night_vision",
    "minecraft:long_night_vision",
    "minecraft:invisibility",
    "minecraft:long_invisibility",
    "minecraft:leaping",
    "minecraft:long_leaping",
    "minecraft:strong_leaping",
    "minecraft:fire_resistance",
    "minecraft:long_fire_resistance",
    "minecraft:swiftness",
    "minecraft:long_swiftness",
    "minecraft:strong_swiftness",
    "minecraft:slowness",
    "minecraft:long_slowness",
    "minecraft:strong_slowness",
    "minecraft:turtle_master",
    "minecraft:long_turtle_master",
    "minecraft:strong_turtle_master",
    "minecraft:water_breathing",
    "minecraft:long_water_breathing",
    "minecraft:healing",
    "minecraft:strong_healing",
    "minecraft:harming",
    "minecraft:strong_harming",
    "minecraft:poison",
    "minecraft:long_poison",
    "minecraft:strong_poison",
    "minecraft:regeneration",
    "minecraft:long_regeneration",
    "minecraft:strong_regeneration",
    "minecraft:strength",
    "minecraft:long_strength",
    "minecraft:strong_strength",
    "minecraft:weakness",
    "minecraft:long_weakness",
    "minecraft:luck",
    "minecraft:slow_falling",
    "minecraft:long_slow_falling",
    "minecraft:wind_charged",
    "minecraft:weaving",
    "minecraft:oozing",
    "minecraft:infested",
];

/// Submit the ordered specification to the actual-context builder. No holder,
/// registry ID, default component/effect or feature flag is supplied here. If
/// resolution/admission fails, propagate the error; never publish a partial
/// builder as though the vanilla bootstrap had completed.
pub fn add_vanilla_mixes<C: PotionBrewingContext>(
    builder: &mut PotionBrewingBuilder<'_, C>,
) -> Result<()> {
    for registration in VANILLA_REGISTRATIONS {
        match *registration {
            VanillaRegistration::Container(item) => builder.add_container(item)?,
            VanillaRegistration::ContainerRecipe {
                from,
                ingredient,
                to,
            } => {
                builder.add_container_recipe(from, ingredient, to)?;
            }
            VanillaRegistration::Mix {
                from,
                ingredient,
                to,
            } => {
                builder.add_mix(from, ingredient, to)?;
            }
            VanillaRegistration::StartMix { ingredient, to } => {
                builder.add_start_mix(ingredient, to)?;
            }
        }
    }
    Ok(())
}
