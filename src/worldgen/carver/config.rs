//! Closed pinned built-in configured-carver registry and typed configuration.
//! Matching a resource name is not certification that a datapack has left the
//! actual registry unchanged. Custom registries/providers remain unsupported.
use super::providers::{integer, number, FloatProvider, HeightProvider, VerticalAnchor};
use crate::{
    inventory::block_state_name, placement::in_block_tag, worldgen::generator::resolve_block_state,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

const INNER_SHA256: &str = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarverDebugSettings {
    pub debug_mode: bool,
    pub air_state: u32,
    pub water_state: u32,
    pub lava_state: u32,
    pub barrier_state: u32,
}
impl CarverDebugSettings {
    fn defaults(value: &Value) -> Result<Self> {
        Ok(Self {
            debug_mode: value["debug_mode"]
                .as_bool()
                .context("missing default debug mode")?,
            air_state: resolve_block_state(&value["air_state"])?,
            water_state: resolve_block_state(&value["water_state"])?,
            lava_state: resolve_block_state(&value["lava_state"])?,
            barrier_state: resolve_block_state(&value["barrier_state"])?,
        })
    }
    fn parse(value: Option<&Value>, defaults: Self) -> Result<Self> {
        let Some(value) = value else {
            return Ok(defaults);
        };
        ensure!(value.is_object(), "carver debug settings must be an object");
        let state = |key: &str| -> Result<u32> {
            // The original field codecs use DEFAULT.getAirState() as the
            // optional fallback for each state. An entirely absent settings
            // object instead uses the complete DEFAULT instance above.
            value
                .get(key)
                .map(resolve_block_state)
                .unwrap_or(Ok(defaults.air_state))
        };
        Ok(Self {
            debug_mode: match value.get("debug_mode") {
                Some(value) => value.as_bool().context("invalid carver debug mode")?,
                None => false,
            },
            air_state: state("air_state")?,
            water_state: state("water_state")?,
            lava_state: state("lava_state")?,
            barrier_state: state("barrier_state")?,
        })
    }
}

/// The two actual built-in resolved HolderSet<Block> tags. Membership applies
/// to a block identity, including all of that block's property states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarverReplaceable {
    Overworld,
    Nether,
}
impl CarverReplaceable {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Overworld => "minecraft:overworld_carver_replaceables",
            Self::Nether => "minecraft:nether_carver_replaceables",
        }
    }
    pub fn contains(self, state: u32) -> Result<bool> {
        validate_builtin_tags()?;
        block_state_name(state).context("carver membership received an unknown canonical state")?;
        Ok(in_block_tag(state, self.tag()))
    }
    fn parse(value: &Value) -> Result<Self> {
        let tag = value
            .as_str()
            .context("built-in carver replaceable must be a tag")?;
        let tag = tag
            .strip_prefix('#')
            .context("custom direct carver block sets are unsupported")?;
        validate_builtin_tags()?;
        match canonical(tag).as_str() {
            "minecraft:overworld_carver_replaceables" => Ok(Self::Overworld),
            "minecraft:nether_carver_replaceables" => Ok(Self::Nether),
            _ => bail!("unsupported carver replaceable tag {tag}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaseCarverConfig {
    pub probability: f32,
    pub y: HeightProvider,
    pub y_scale: FloatProvider,
    pub lava_level: VerticalAnchor,
    pub debug: CarverDebugSettings,
    pub replaceable: CarverReplaceable,
}
impl BaseCarverConfig {
    fn parse(value: &Value, defaults: CarverDebugSettings) -> Result<Self> {
        let probability = number(&value["probability"])?;
        ensure!(
            (0.0..=1.0).contains(&probability),
            "carver probability is outside [0,1]"
        );
        Ok(Self {
            probability,
            y: HeightProvider::parse(&value["y"])?,
            y_scale: FloatProvider::parse(&value["yScale"])?,
            lava_level: VerticalAnchor::parse(&value["lava_level"])?,
            debug: CarverDebugSettings::parse(value.get("debug_settings"), defaults)?,
            replaceable: CarverReplaceable::parse(&value["replaceable"])?,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaveCarverConfig {
    pub base: BaseCarverConfig,
    pub horizontal_radius_multiplier: FloatProvider,
    pub vertical_radius_multiplier: FloatProvider,
    pub floor_level: FloatProvider,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CanyonShapeConfig {
    pub distance_factor: FloatProvider,
    pub thickness: FloatProvider,
    pub width_smoothness: i32,
    pub horizontal_radius_factor: FloatProvider,
    pub vertical_radius_default_factor: f32,
    pub vertical_radius_center_factor: f32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CanyonCarverConfig {
    pub base: BaseCarverConfig,
    pub vertical_rotation: FloatProvider,
    pub shape: CanyonShapeConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfiguredCarver {
    Cave(CaveCarverConfig),
    NetherCave(CaveCarverConfig),
    Canyon(CanyonCarverConfig),
}
impl ConfiguredCarver {
    pub fn base(&self) -> &BaseCarverConfig {
        match self {
            Self::Cave(config) | Self::NetherCave(config) => &config.base,
            Self::Canyon(config) => &config.base,
        }
    }
    fn parse(value: &Value, defaults: CarverDebugSettings) -> Result<Self> {
        let kind = canonical(value["type"].as_str().context("missing carver type")?);
        let value = &value["config"];
        let base = BaseCarverConfig::parse(value, defaults)?;
        match kind.as_str() {
            "minecraft:cave" | "minecraft:nether_cave" => {
                let floor_level = FloatProvider::parse(&value["floor_level"])?;
                ensure!(
                    floor_level.min_value() >= -1.0 && floor_level.max_value() <= 1.0,
                    "cave floor provider exceeds original [-1,1] codec range"
                );
                let config = CaveCarverConfig {
                    base,
                    horizontal_radius_multiplier: FloatProvider::parse(
                        &value["horizontal_radius_multiplier"],
                    )?,
                    vertical_radius_multiplier: FloatProvider::parse(
                        &value["vertical_radius_multiplier"],
                    )?,
                    floor_level,
                };
                Ok(if kind == "minecraft:cave" {
                    Self::Cave(config)
                } else {
                    Self::NetherCave(config)
                })
            }
            "minecraft:canyon" => {
                let shape = &value["shape"];
                let width_smoothness = integer(&shape["width_smoothness"])?;
                ensure!(
                    width_smoothness >= 0,
                    "canyon width smoothness must be nonnegative"
                );
                Ok(Self::Canyon(CanyonCarverConfig {
                    base,
                    vertical_rotation: FloatProvider::parse(&value["vertical_rotation"])?,
                    shape: CanyonShapeConfig {
                        distance_factor: FloatProvider::parse(&shape["distance_factor"])?,
                        thickness: FloatProvider::parse(&shape["thickness"])?,
                        width_smoothness,
                        horizontal_radius_factor: FloatProvider::parse(
                            &shape["horizontal_radius_factor"],
                        )?,
                        vertical_radius_default_factor: number(
                            &shape["vertical_radius_default_factor"],
                        )?,
                        vertical_radius_center_factor: number(
                            &shape["vertical_radius_center_factor"],
                        )?,
                    },
                }))
            }
            _ => bail!("unsupported configured carver type {kind}"),
        }
    }
}

struct Builtins {
    carvers: BTreeMap<String, ConfiguredCarver>,
    biome_carvers: BTreeMap<String, Vec<String>>,
}
fn builtins() -> Result<&'static Builtins> {
    static BUILTINS: OnceLock<std::result::Result<Builtins, String>> = OnceLock::new();
    BUILTINS
        .get_or_init(|| load_builtins().map_err(|error| format!("{error:#}")))
        .as_ref()
        .map_err(|error| anyhow!("invalid packaged carver registry: {error}"))
}
fn load_builtins() -> Result<Builtins> {
    let value: Value = serde_json::from_str(include_str!(
        "../../../fixtures/world-generation-configured-carvers.json"
    ))?;
    ensure!(
        value["minecraft"] == "1.21" && value["oracle_inner_sha256"] == INNER_SHA256,
        "packaged carver registry has a different source identity"
    );
    let defaults = CarverDebugSettings::defaults(&value["debug_defaults"])?;
    let carvers = value["configured_carvers"]
        .as_object()
        .context("missing packaged carvers")?
        .iter()
        .map(|(name, value)| Ok((name.clone(), ConfiguredCarver::parse(value, defaults)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    ensure!(
        carvers.len() == 4
            && [
                "minecraft:cave",
                "minecraft:cave_extra_underground",
                "minecraft:nether_cave",
                "minecraft:canyon"
            ]
            .into_iter()
            .all(|name| carvers.contains_key(name)),
        "packaged carver names differ from the pinned registry"
    );
    let biome_carvers =
        serde_json::from_value::<BTreeMap<String, Vec<String>>>(value["biome_carvers"].clone())?;
    for names in biome_carvers.values() {
        for name in names {
            ensure!(
                carvers.contains_key(name),
                "biome references unavailable configured carver {name}"
            );
        }
    }
    Ok(Builtins {
        carvers,
        biome_carvers,
    })
}

/// Only the four packaged configured-carver holders are available. Callers
/// certify the actual unchanged pinned registry before using this lookup.
pub fn builtin_configured_carver(name: &str) -> Result<&'static ConfiguredCarver> {
    builtins()?
        .carvers
        .get(&canonical(name))
        .with_context(|| format!("unsupported configured carver {name}"))
}

/// Original AIR carver holder order for this exact built-in biome. An unknown
/// biome errors; an actual built-in with no carvers returns its recorded empty
/// list. This is not the neighboring ChunkAccess.carverBiome cache owner.
pub fn builtin_biome_carvers(biome: &str) -> Result<&'static [String]> {
    builtins()?
        .biome_carvers
        .get(&canonical(biome))
        .map(Vec::as_slice)
        .with_context(|| format!("unsupported carver biome {biome}"))
}
fn canonical(name: &str) -> String {
    match name.split_once(':') {
        Some(("", path)) => format!("minecraft:{path}"),
        Some(_) => name.to_owned(),
        None => format!("minecraft:{name}"),
    }
}

fn validate_builtin_tags() -> Result<()> {
    #[derive(Deserialize)]
    struct Tags {
        minecraft: String,
        jar_sha256: String,
        tags: BTreeMap<String, Vec<u32>>,
    }
    static TAGS: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    TAGS.get_or_init(|| {
        let validate = || -> Result<()> {
            let data: Tags = serde_json::from_str(include_str!(
                "../../../fixtures/placement-block-tags-1.21.json"
            ))?;
            ensure!(
                data.minecraft == "1.21" && data.jar_sha256 == INNER_SHA256,
                "carver tag source identity differs"
            );
            for tag in [CarverReplaceable::Overworld, CarverReplaceable::Nether] {
                let members = data
                    .tags
                    .get(tag.tag())
                    .with_context(|| format!("missing carver block tag {}", tag.tag()))?;
                ensure!(!members.is_empty(), "empty packaged carver block tag");
                for state in members {
                    block_state_name(*state)
                        .context("carver tag contains an unknown canonical block")?;
                }
            }
            Ok(())
        };
        validate().map_err(|error| format!("{error:#}"))
    })
    .as_ref()
    .map(|_| ())
    .map_err(|error| anyhow!("invalid packaged carver block tags: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn original_configured_debug_states_match_without_enabling_debug() -> Result<()> {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../fixtures/world-generation-configured-carvers.json"
        ))?;
        let states = fixture["resolved_debug_states"]
            .as_object()
            .context("missing original debug observations")?;
        ensure!(
            states.len() == builtins()?.carvers.len(),
            "incomplete original debug observations"
        );
        for (name, expected) in states {
            let actual = builtin_configured_carver(name)?.base().debug;
            assert_eq!(
                actual.debug_mode,
                expected["debug_mode"]
                    .as_bool()
                    .context("missing original debug mode")?
            );
            assert_eq!(
                u64::from(actual.air_state),
                expected["air_state"]
                    .as_u64()
                    .context("missing original air state")?
            );
            assert_eq!(
                u64::from(actual.water_state),
                expected["water_state"]
                    .as_u64()
                    .context("missing original water state")?
            );
            assert_eq!(
                u64::from(actual.lava_state),
                expected["lava_state"]
                    .as_u64()
                    .context("missing original lava state")?
            );
            assert_eq!(
                u64::from(actual.barrier_state),
                expected["barrier_state"]
                    .as_u64()
                    .context("missing original barrier state")?
            );
        }
        Ok(())
    }

    #[test]
    fn builtin_holder_order_and_missing_owners_are_distinct() -> Result<()> {
        assert_eq!(
            builtin_biome_carvers("plains")?,
            [
                "minecraft:cave",
                "minecraft:cave_extra_underground",
                "minecraft:canyon"
            ]
        );
        assert_eq!(
            builtin_biome_carvers(":nether_wastes")?,
            ["minecraft:nether_cave"]
        );
        assert!(builtin_biome_carvers("the_end")?.is_empty());
        assert!(builtin_biome_carvers("example:plains").is_err());
        assert!(builtin_configured_carver("example:cave").is_err());
        assert!(std::ptr::eq(
            builtin_configured_carver("cave")?,
            builtin_configured_carver(":cave")?
        ));
        Ok(())
    }

    #[test]
    fn replacement_uses_whole_block_identity_and_rejects_unknown_state() -> Result<()> {
        let water = resolve_block_state(&json!({
            "Name":"minecraft:water", "Properties":{"level":"15"}
        }))?;
        assert!(CarverReplaceable::Overworld.contains(water)?);
        assert!(!CarverReplaceable::Nether.contains(water)?);
        let bedrock = resolve_block_state(&json!({"Name":"minecraft:bedrock"}))?;
        assert!(!CarverReplaceable::Overworld.contains(bedrock)?);
        assert!(CarverReplaceable::Overworld.contains(u32::MAX).is_err());
        assert!(CarverReplaceable::parse(&json!("#example:missing")).is_err());
        Ok(())
    }
}
