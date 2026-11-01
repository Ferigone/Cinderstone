//! Closed pinned configured-ore catalog. Registry names alone do not certify
//! unchanged actual registry/tag owners; explicit snapshot checks reject data
//! overrides. Other configured features and random rule predicates are absent.
use crate::{
    inventory::block_state_name, placement::in_block_tag, worldgen::generator::resolve_block_state,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

const INNER_SHA256: &str = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
const CONFIGS: &str = include_str!("../../../fixtures/world-generation-ore-configs.json");

/// Original TagMatchTest identities used by the pinned ore configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OreTargetTag {
    BaseStoneOverworld,
    BaseStoneNether,
    StoneOreReplaceables,
    DeepslateOreReplaceables,
}
impl OreTargetTag {
    pub fn name(self) -> &'static str {
        match self {
            Self::BaseStoneOverworld => "minecraft:base_stone_overworld",
            Self::BaseStoneNether => "minecraft:base_stone_nether",
            Self::StoneOreReplaceables => "minecraft:stone_ore_replaceables",
            Self::DeepslateOreReplaceables => "minecraft:deepslate_ore_replaceables",
        }
    }
    pub fn contains(self, state: u32) -> Result<bool> {
        validate_builtin_tags()?;
        block_state_name(state).context("ore tag predicate received unknown canonical state")?;
        Ok(in_block_tag(state, self.name()))
    }
    fn parse(name: &str) -> Result<Self> {
        Ok(match canonical(name).as_str() {
            "minecraft:base_stone_overworld" => Self::BaseStoneOverworld,
            "minecraft:base_stone_nether" => Self::BaseStoneNether,
            "minecraft:stone_ore_replaceables" => Self::StoneOreReplaceables,
            "minecraft:deepslate_ore_replaceables" => Self::DeepslateOreReplaceables,
            _ => bail!("unsupported ore target tag {name}"),
        })
    }
}

/// The two deterministic RuleTest implementations in the actual closed catalog.
/// BlockMatch stores a valid canonical representative of the block, and tests
/// its block identity across every property state. Neither rule draws RNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OreRuleTest {
    BlockMatch(u32),
    TagMatch(OreTargetTag),
}
impl OreRuleTest {
    pub fn matches(&self, state: u32) -> Result<bool> {
        let actual = block_state_name(state)
            .context("ore target predicate received unknown canonical state")?;
        Ok(match *self {
            Self::BlockMatch(block) => {
                let expected = block_state_name(block)
                    .context("ore block predicate references unknown canonical block")?;
                actual == expected
            }
            Self::TagMatch(tag) => tag.contains(state)?,
        })
    }
    fn validate(&self) -> Result<()> {
        match *self {
            Self::BlockMatch(block) => {
                block_state_name(block)
                    .context("ore block predicate references unknown canonical block")?;
            }
            Self::TagMatch(_) => validate_builtin_tags()?,
        }
        Ok(())
    }
    fn parse(value: &Value) -> Result<Self> {
        let kind = canonical(
            value["predicate_type"]
                .as_str()
                .context("ore rule lacks predicate_type")?,
        );
        match kind.as_str() {
            "minecraft:block_match" => {
                fields(value, &["predicate_type", "block"])?;
                let name = value["block"]
                    .as_str()
                    .context("ore block rule lacks block")?;
                Ok(Self::BlockMatch(resolve_block_state(
                    &serde_json::json!({"Name":canonical(name)}),
                )?))
            }
            "minecraft:tag_match" => {
                fields(value, &["predicate_type", "tag"])?;
                Ok(Self::TagMatch(OreTargetTag::parse(
                    value["tag"].as_str().context("ore tag rule lacks tag")?,
                )?))
            }
            _ => bail!("unsupported ore target predicate {kind}"),
        }
    }
}

/// Preserve encounter order: the first matching target supplies the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetBlockState {
    pub target: OreRuleTest,
    pub state: u32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct OreConfiguration {
    pub targets: Vec<TargetBlockState>,
    pub size: i32,
    pub discard_chance_on_air_exposure: f32,
}
impl OreConfiguration {
    /// Validate public caller-constructed values before any host/RNG operation.
    /// Original codec permits zero size and an empty target list.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (0..=64).contains(&self.size),
            "ore size is outside original [0,64] codec range"
        );
        ensure!(
            self.discard_chance_on_air_exposure.is_finite()
                && (0.0..=1.0).contains(&self.discard_chance_on_air_exposure),
            "ore air-exposure discard probability is outside original [0,1] codec range"
        );
        for target in &self.targets {
            block_state_name(target.state)
                .context("ore target contains unknown canonical output state")?;
            target.target.validate()?;
        }
        Ok(())
    }
    fn parse(value: &Value) -> Result<Self> {
        fields(
            value,
            &["targets", "size", "discard_chance_on_air_exposure"],
        )?;
        let targets = value["targets"]
            .as_array()
            .context("ore config lacks targets")?
            .iter()
            .map(|target| {
                fields(target, &["target", "state"])?;
                fields(&target["state"], &["Name", "Properties"])?;
                Ok(TargetBlockState {
                    target: OreRuleTest::parse(&target["target"])?,
                    state: resolve_block_state(&target["state"])?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let size = i32::try_from(
            value["size"]
                .as_i64()
                .context("ore size must be an integer")?,
        )?;
        let discard = value["discard_chance_on_air_exposure"]
            .as_f64()
            .context("ore config lacks air-exposure discard probability")?
            as f32;
        let result = Self {
            targets,
            size,
            discard_chance_on_air_exposure: discard,
        };
        result.validate()?;
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfiguredOreFeature {
    Ore(OreConfiguration),
    ScatteredOre(OreConfiguration),
}
impl ConfiguredOreFeature {
    pub fn configuration(&self) -> &OreConfiguration {
        match self {
            Self::Ore(config) | Self::ScatteredOre(config) => config,
        }
    }
    fn parse(value: &Value) -> Result<Self> {
        fields(value, &["type", "config"])?;
        let kind = canonical(
            value["type"]
                .as_str()
                .context("configured ore lacks type")?,
        );
        let config = OreConfiguration::parse(&value["config"])?;
        Ok(match kind.as_str() {
            "minecraft:ore" => Self::Ore(config),
            "minecraft:scattered_ore" => Self::ScatteredOre(config),
            _ => bail!("unsupported configured ore feature type {kind}"),
        })
    }
}

struct Catalog {
    entries: BTreeMap<String, ConfiguredOreFeature>,
    snapshots: BTreeMap<String, Value>,
}
fn catalog() -> Result<&'static Catalog> {
    static CATALOG: OnceLock<std::result::Result<Catalog, String>> = OnceLock::new();
    CATALOG
        .get_or_init(|| load_catalog().map_err(|e| format!("{e:#}")))
        .as_ref()
        .map_err(|e| anyhow!("invalid packaged ore catalog: {e}"))
}
fn load_catalog() -> Result<Catalog> {
    let fixture: Value = serde_json::from_str(CONFIGS)?;
    ensure!(
        fixture["minecraft"] == "1.21"
            && fixture["protocol"] == 767
            && fixture["oracle_inner_sha256"] == INNER_SHA256,
        "packaged ore catalog has a different source identity"
    );
    validate_builtin_tags()?;
    let snapshots: BTreeMap<String, Value> =
        serde_json::from_value(fixture["configured_ores"].clone())?;
    let entries = snapshots
        .iter()
        .map(|(key, value)| Ok((key.clone(), ConfiguredOreFeature::parse(value)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    const NAMES: &[&str] = &[
        "ore_andesite",
        "ore_ancient_debris_large",
        "ore_ancient_debris_small",
        "ore_blackstone",
        "ore_clay",
        "ore_coal",
        "ore_coal_buried",
        "ore_copper_large",
        "ore_copper_small",
        "ore_diamond_buried",
        "ore_diamond_large",
        "ore_diamond_medium",
        "ore_diamond_small",
        "ore_diorite",
        "ore_dirt",
        "ore_emerald",
        "ore_gold",
        "ore_gold_buried",
        "ore_granite",
        "ore_gravel",
        "ore_gravel_nether",
        "ore_infested",
        "ore_iron",
        "ore_iron_small",
        "ore_lapis",
        "ore_lapis_buried",
        "ore_magma",
        "ore_nether_gold",
        "ore_quartz",
        "ore_redstone",
        "ore_soul_sand",
        "ore_tuff",
    ];
    ensure!(
        entries.len() == NAMES.len()
            && NAMES
                .iter()
                .all(|name| entries.contains_key(&canonical(name))),
        "packaged ore names differ from the original closed catalog"
    );
    ensure!(
        entries
            .values()
            .filter(|v| matches!(v, ConfiguredOreFeature::Ore(_)))
            .count()
            == 30,
        "packaged regular/scattered ore kinds differ"
    );
    Ok(Catalog { entries, snapshots })
}

/// Only the 32 packaged original configured ore holders are available. The
/// caller must establish unchanged actual registry/tag ownership. A resource
/// name by itself does not establish that certificate.
pub fn builtin_configured_ore(name: &str) -> Result<&'static ConfiguredOreFeature> {
    catalog()?
        .entries
        .get(&canonical(name))
        .with_context(|| format!("unsupported configured ore {name}"))
}

/// Verify actual decoded configured-feature data against this closed snapshot.
/// Overrides fail even if their holder retains a familiar original name. Tag
/// ownership still requires the caller's separate actual unchanged-tag fact.
pub fn builtin_configured_ore_from_snapshot(
    name: &str,
    actual: &Value,
) -> Result<&'static ConfiguredOreFeature> {
    let source = catalog()?;
    let name = canonical(name);
    let expected = source
        .snapshots
        .get(&name)
        .with_context(|| format!("unsupported configured ore {name}"))?;
    ensure!(
        actual == expected,
        "configured ore registry override is unsupported: {name}"
    );
    source
        .entries
        .get(&name)
        .context("packaged ore snapshot has no typed entry")
}

fn canonical(name: &str) -> String {
    match name.split_once(':') {
        Some(("", path)) => format!("minecraft:{path}"),
        Some(_) => name.to_owned(),
        None => format!("minecraft:{name}"),
    }
}
fn fields(value: &Value, allowed: &[&str]) -> Result<()> {
    let fields = value
        .as_object()
        .context("ore configuration component must be an object")?;
    ensure!(
        fields.keys().all(|key| allowed.contains(&key.as_str())),
        "unsupported extra ore configuration field"
    );
    Ok(())
}
fn validate_builtin_tags() -> Result<()> {
    #[derive(Deserialize)]
    struct Tags {
        minecraft: String,
        jar_sha256: String,
        tags: BTreeMap<String, Vec<u32>>,
    }
    static VALID: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    VALID
        .get_or_init(|| {
            let validate = || -> Result<()> {
                let data: Tags = serde_json::from_str(include_str!(
                    "../../../fixtures/placement-block-tags-1.21.json"
                ))?;
                ensure!(
                    data.minecraft == "1.21" && data.jar_sha256 == INNER_SHA256,
                    "ore tag source identity differs"
                );
                for tag in [
                    OreTargetTag::BaseStoneOverworld,
                    OreTargetTag::BaseStoneNether,
                    OreTargetTag::StoneOreReplaceables,
                    OreTargetTag::DeepslateOreReplaceables,
                ] {
                    let members = data
                        .tags
                        .get(tag.name())
                        .with_context(|| format!("missing ore target tag {}", tag.name()))?;
                    ensure!(!members.is_empty(), "empty packaged ore target tag");
                    for &state in members {
                        block_state_name(state)
                            .context("ore tag contains an unknown canonical block")?;
                    }
                }
                Ok(())
            };
            validate().map_err(|e| format!("{e:#}"))
        })
        .as_ref()
        .map(|_| ())
        .map_err(|e| anyhow!("invalid packaged ore tags: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_catalog_keeps_ordered_targets_and_property_states() -> Result<()> {
        let iron = builtin_configured_ore("ore_iron")?.configuration();
        assert_eq!(iron.size, 9);
        assert_eq!(iron.targets.len(), 2);
        assert_eq!(
            iron.targets[0].target,
            OreRuleTest::TagMatch(OreTargetTag::StoneOreReplaceables)
        );
        assert_eq!(
            iron.targets[1].target,
            OreRuleTest::TagMatch(OreTargetTag::DeepslateOreReplaceables)
        );
        assert_eq!(
            iron.targets[0].state,
            resolve_block_state(&json!({"Name":"minecraft:iron_ore"}))?
        );
        assert_eq!(
            iron.targets[1].state,
            resolve_block_state(&json!({"Name":"minecraft:deepslate_iron_ore"}))?
        );
        let redstone = builtin_configured_ore("ore_redstone")?.configuration();
        assert_eq!(
            redstone.targets[0].state,
            resolve_block_state(
                &json!({"Name":"minecraft:redstone_ore","Properties":{"lit":"false"}})
            )?
        );
        assert_ne!(
            redstone.targets[0].state,
            resolve_block_state(
                &json!({"Name":"minecraft:redstone_ore","Properties":{"lit":"true"}})
            )?
        );
        assert!(matches!(
            builtin_configured_ore("ore_ancient_debris_large")?,
            ConfiguredOreFeature::ScatteredOre(_)
        ));
        assert!(std::ptr::eq(
            builtin_configured_ore("ore_iron")?,
            builtin_configured_ore(":ore_iron")?
        ));
        assert!(builtin_configured_ore("example:ore_iron").is_err());
        assert!(builtin_configured_ore("minecraft:missing").is_err());
        Ok(())
    }

    #[test]
    fn source_rule_tests_match_block_identity_across_property_states() -> Result<()> {
        let axis_y =
            resolve_block_state(&json!({"Name":"minecraft:deepslate","Properties":{"axis":"y"}}))?;
        let axis_x =
            resolve_block_state(&json!({"Name":"minecraft:deepslate","Properties":{"axis":"x"}}))?;
        assert_ne!(axis_y, axis_x);
        assert!(OreRuleTest::BlockMatch(axis_y).matches(axis_x)?);
        assert!(OreTargetTag::DeepslateOreReplaceables.contains(axis_x)?);
        assert!(!OreTargetTag::StoneOreReplaceables.contains(axis_x)?);
        let stone = resolve_block_state(&json!({"Name":"minecraft:stone"}))?;
        assert!(OreTargetTag::BaseStoneOverworld.contains(stone)?);
        assert!(!OreTargetTag::BaseStoneNether.contains(stone)?);
        assert!(OreRuleTest::BlockMatch(u32::MAX).matches(stone).is_err());
        assert!(OreRuleTest::BlockMatch(stone).matches(u32::MAX).is_err());
        Ok(())
    }

    #[test]
    fn source_codec_boundaries_and_closed_scope_fail_before_execution() -> Result<()> {
        let mut config = OreConfiguration {
            targets: Vec::new(),
            size: 0,
            discard_chance_on_air_exposure: 0.0,
        };
        config.validate()?;
        config.size = 64;
        config.discard_chance_on_air_exposure = 1.0;
        config.validate()?;
        config.size = 65;
        assert!(config.validate().is_err());
        config.size = -1;
        assert!(config.validate().is_err());
        config.size = 0;
        config.discard_chance_on_air_exposure = f32::NAN;
        assert!(config.validate().is_err());
        config.discard_chance_on_air_exposure = f32::INFINITY;
        assert!(config.validate().is_err());
        config.discard_chance_on_air_exposure = -0.1;
        assert!(config.validate().is_err());
        config.discard_chance_on_air_exposure = 0.0;
        config.targets.push(TargetBlockState {
            target: OreRuleTest::BlockMatch(1),
            state: u32::MAX,
        });
        assert!(config.validate().is_err());
        assert!(OreConfiguration::parse(&json!({"targets":[],"size":0})).is_err());
        assert!(OreRuleTest::parse(&json!({"predicate_type":"minecraft:random_block_match","block":"minecraft:stone","probability":0.5})).is_err());
        assert!(OreRuleTest::parse(
            &json!({"predicate_type":"minecraft:tag_match","tag":"minecraft:missing"})
        )
        .is_err());
        assert!(OreRuleTest::parse(
            &json!({"predicate_type":"minecraft:block_match","block":"example:stone"})
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn known_name_does_not_accept_a_changed_registry_snapshot() -> Result<()> {
        let fixture: Value = serde_json::from_str(CONFIGS)?;
        let actual = &fixture["configured_ores"]["minecraft:ore_iron"];
        assert!(std::ptr::eq(
            builtin_configured_ore("ore_iron")?,
            builtin_configured_ore_from_snapshot("ore_iron", actual)?
        ));
        let mut changed = actual.clone();
        changed["config"]["size"] = json!(10);
        assert!(builtin_configured_ore_from_snapshot("ore_iron", &changed).is_err());
        assert!(builtin_configured_ore_from_snapshot("example:ore_iron", actual).is_err());
        Ok(())
    }
}
