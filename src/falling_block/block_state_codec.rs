//! Concrete pinned 1.21 NbtUtils BlockState codec, with actual borrowed owners.
//!
//! Catalogue lookup and parsing are native. The registry binding, input
//! CompoundTag encounter order and warning sink are required real capabilities;
//! this module never constructs them from a guessed empty/default context.

use super::persistence::FallingBlockStateCodec;
use crate::nbt::{JavaString, Tag};
use anyhow::{anyhow, ensure, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const INNER_SHA256: &str = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
const MAPPINGS_SHA1: &str = "31c77994d96f05ba25a870ada70f47f315330437";
const STATE_COUNT: usize = 26_684;

pub const INVALID_PROPERTY_WARNING: &str =
    "Unable to read property: {} with value: {} for blockstate: {}";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockPropertyKind { Boolean, Integer, Enum }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceLocationPart { Namespace, Path }

/// Source parsing failure, distinct from unavailable native capabilities.
#[derive(Clone, Debug)]
pub struct InvalidBlockResourceLocation {
    pub value: JavaString,
    pub part: ResourceLocationPart,
}
impl std::fmt::Display for InvalidBlockResourceLocation {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "invalid block ResourceLocation {:?}", self.part)
    }
}
impl std::error::Error for InvalidBlockResourceLocation {}

/// Actual StateDefinition order, not an invented registry. Values use
/// Property.getName; StateHolder write encounter order is queried separately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundBlockProperty {
    pub name: String,
    pub kind: BlockPropertyKind,
    pub values: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundBlockDefinition {
    pub key: String,
    pub minimum_state: u32,
    pub maximum_state: u32,
    pub default_state: u32,
    pub properties: Vec<BoundBlockProperty>,
}

/// Implement over the actual native Level's retained block registry and tags.
/// Every method is mandatory. An identity covers registry/tag lifetime; it must
/// change if the underlying definitions or configured tag bindings change.
pub trait ActualBlockRegistry {
    type Identity: Clone + Eq;
    fn identity(&self) -> Result<Self::Identity>;
    fn minecraft_version(&self) -> Result<&str>;
    fn protocol_version(&self) -> Result<i32>;
    fn original_inner_sha256(&self) -> Result<&str>;
    fn official_mappings_sha1(&self) -> Result<&str>;
    fn block_definitions(&self) -> Result<Vec<BoundBlockDefinition>>;
    fn state_properties(&self, state: u32) -> Result<BTreeMap<String, String>>;
    /// Actual StateHolder.getValues().entrySet order for this same state.
    /// It is not inferred from a BTreeMap or packet property order.
    fn state_property_order(&self, state: u32) -> Result<Vec<String>>;
    fn state_is_air(&self, state: u32) -> Result<bool>;
    /// Actual current BlockTags.ANVIL membership. No compiled-tag fallback.
    fn state_is_anvil(&self, state: u32) -> Result<bool>;
}

/// Borrow the actual original input-map order and logger. No default/no-op
/// implementation is provided. Tag's stored/wire order alone is not a Java
/// HashMap history certificate. Callbacks must not mutate registry/input tags.
pub trait BlockStateNbtContext {
    fn property_key_order(&mut self, properties: &Tag) -> Result<Vec<JavaString>>;
    fn warn_invalid_property(
        &mut self,
        message: &'static str,
        property: &JavaString,
        value: &JavaString,
        original_block_state: &Tag,
    ) -> Result<()>;
}

pub struct PinnedBlockStateCodec<'a, R: ActualBlockRegistry, C: BlockStateNbtContext> {
    registry: &'a R,
    context: &'a mut C,
    identity: R::Identity,
    blocks: BTreeMap<String, BoundBlockDefinition>,
    by_state: Vec<String>,
}

impl<'a, R: ActualBlockRegistry, C: BlockStateNbtContext> PinnedBlockStateCodec<'a, R, C> {
    /// Certify the complete actual block/property/state binding ONCE, before
    /// lookup may use the compiled pinned catalogue. This is not a constructor
    /// accepting a bare state number or caller-created "supported=true" flag.
    pub fn capture(registry: &'a R, context: &'a mut C) -> Result<Self> {
        let identity = registry.identity()?;
        ensure!(registry.minecraft_version()? == "1.21" && registry.protocol_version()? == 767,
            "block-state codec requires the actual supported 1.21 registry");
        ensure!(registry.original_inner_sha256()? == INNER_SHA256
            && registry.official_mappings_sha1()? == MAPPINGS_SHA1,
            "block-state codec registry has foreign source provenance");
        let expected = pinned_definitions()?;
        let mut actual = BTreeMap::new();
        for block in registry.block_definitions()? {
            ensure!(actual.insert(block.key.clone(), block).is_none(), "duplicate actual block key");
        }
        ensure!(actual == expected, "actual block/property registry differs from the pinned catalogue");
        let mut by_state = vec![String::new(); STATE_COUNT];
        for (key, block) in &actual {
            for state in block.minimum_state..=block.maximum_state {
                let slot = by_state.get_mut(state as usize).ok_or_else(|| anyhow!("foreign block state ID"))?;
                ensure!(slot.is_empty(), "overlapping actual block states");
                ensure!(registry.state_properties(state)? == properties_for(block, state)?,
                    "actual property/state binding differs at state {state}");
                *slot = key.clone();
            }
        }
        ensure!(by_state.iter().all(|key| !key.is_empty()), "actual registry omits a pinned block state");
        ensure!(registry.identity()? == identity, "block registry changed during codec capture");
        Ok(Self { registry, context, identity, blocks: actual, by_state })
    }

    fn require_current(&self) -> Result<()> {
        ensure!(self.registry.identity()? == self.identity, "block-state codec registry/tag snapshot changed");
        Ok(())
    }
    fn block(&self, state: u32) -> Result<&BoundBlockDefinition> {
        let key = self.by_state.get(state as usize).ok_or_else(|| anyhow!("unknown block-state ID {state}"))?;
        self.blocks.get(key).ok_or_else(|| anyhow!("captured block definition is absent"))
    }
    fn air(&self) -> Result<u32> {
        self.blocks.get("minecraft:air").map(|block| block.default_state)
            .ok_or_else(|| anyhow!("actual AIR block is unavailable"))
    }
}

impl<R: ActualBlockRegistry, C: BlockStateNbtContext> FallingBlockStateCodec
    for PinnedBlockStateCodec<'_, R, C>
{
    fn write_block_state(&mut self, state: u32) -> Result<Tag> {
        self.require_current()?;
        let block = self.block(state)?;
        let values = properties_for(block, state)?;
        let mut puts = vec![(JavaString::from("Name"), Tag::String(block.key.clone().into()))];
        if !block.properties.is_empty() {
            let order = self.registry.state_property_order(state)?;
            ensure!(order.len() == values.len() && order.iter().collect::<BTreeSet<_>>()
                == values.keys().collect::<BTreeSet<_>>(), "actual StateHolder property order differs from its values");
            let property_puts = order.into_iter().map(|name| {
                Ok((name.clone().into(), Tag::String(values.get(&name)
                    .ok_or_else(|| anyhow!("captured property value is absent"))?.clone().into())))
            }).collect::<Result<Vec<_>>>()?;
            puts.push(("Properties".into(), fresh_compound(property_puts)?));
        }
        fresh_compound(puts)
    }

    fn read_block_state(&mut self, input: &Tag) -> Result<u32> {
        self.require_current()?;
        ensure!(matches!(input, Tag::Compound(_)), "NbtUtils.readBlockState requires an actual compound");
        let Some(Tag::String(name)) = input.get("Name") else { return self.air(); };
        // NbtUtils calls the THROWING ResourceLocation.parse, not tryParse.
        let key = parse_resource_location(name)?;
        let Some(block) = self.blocks.get(&key) else { return self.air(); };
        let mut state = block.default_state;
        let Some(properties @ Tag::Compound(entries)) = input.get("Properties") else { return Ok(state); };
        if entries.is_empty() { return Ok(state); }
        let block = block.clone();
        let order = self.context.property_key_order(properties)?;
        validate_key_order(entries, &order)?;
        for name in order {
            let Some(property) = block.properties.iter().find(|property| name == property.name.as_str()) else { continue; };
            let value = match last_value(entries, &name) {
                Some(Tag::String(value)) => value.clone(),
                // CompoundTag.getString returns EMPTY for every non-string.
                _ => JavaString::default(),
            };
            if let Some(canonical) = parse_property(property, &value) {
                state = set_property(&block, state, &property.name, canonical)?;
            } else {
                self.context.warn_invalid_property(INVALID_PROPERTY_WARNING, &name, &value, input)?;
                self.require_current()?;
            }
        }
        Ok(state)
    }

    fn state_is_air(&mut self, state: u32) -> Result<bool> {
        self.require_current()?; self.block(state)?;
        self.registry.state_is_air(state)
    }
    fn state_is_anvil(&mut self, state: u32) -> Result<bool> {
        self.require_current()?; self.block(state)?;
        self.registry.state_is_anvil(state)
    }
    fn default_sand_state(&mut self) -> Result<u32> {
        self.require_current()?;
        self.blocks.get("minecraft:sand").map(|block| block.default_state)
            .ok_or_else(|| anyhow!("actual SAND block is unavailable"))
    }
}

fn parse_resource_location(value: &JavaString) -> Result<String> {
    let units = value.units();
    let default_namespace = "minecraft".encode_utf16().collect::<Vec<_>>();
    let (namespace, path) = match units.iter().position(|unit| *unit == u16::from(b':')) {
        Some(0) => (default_namespace.as_slice(), &units[1..]),
        Some(index) => (&units[..index], &units[index + 1..]),
        None => (default_namespace.as_slice(), units.as_ref()),
    };
    let valid = |unit: &u16, path: bool| matches!(*unit, 0x61..=0x7a | 0x30..=0x39 | 0x5f | 0x2e | 0x2d)
        || (path && *unit == 0x2f);
    if !namespace.iter().all(|unit| valid(unit, false)) {
        return Err(InvalidBlockResourceLocation { value: value.clone(), part: ResourceLocationPart::Namespace }.into());
    }
    if !path.iter().all(|unit| valid(unit, true)) {
        return Err(InvalidBlockResourceLocation { value: value.clone(), part: ResourceLocationPart::Path }.into());
    }
    let namespace = namespace.iter().map(|unit| *unit as u8 as char).collect::<String>();
    let path = path.iter().map(|unit| *unit as u8 as char).collect::<String>();
    Ok(format!("{namespace}:{path}"))
}

fn last_value<'a>(entries: &'a [(JavaString, Tag)], name: &JavaString) -> Option<&'a Tag> {
    entries.iter().rev().find(|(key, _)| key == name).map(|(_, value)| value)
}
fn validate_key_order(entries: &[(JavaString, Tag)], order: &[JavaString]) -> Result<()> {
    let expected = entries.iter().map(|(key, _)| key.clone()).collect::<BTreeSet<_>>();
    let actual = order.iter().cloned().collect::<BTreeSet<_>>();
    ensure!(actual.len() == order.len() && actual == expected,
        "actual CompoundTag key order differs from its logical input");
    Ok(())
}

fn parse_property<'a>(property: &'a BoundBlockProperty, value: &JavaString) -> Option<&'a str> {
    match property.kind {
        BlockPropertyKind::Integer => {
            let number = java_integer(value)?;
            property.values.iter().find(|candidate| candidate.parse::<i32>() == Ok(number)).map(String::as_str)
        }
        BlockPropertyKind::Boolean | BlockPropertyKind::Enum =>
            property.values.iter().find(|candidate| value == candidate.as_str()).map(String::as_str),
    }
}

// Same pinned JDK21 BMP Character.digit(char,10) ranges already observed for
// world_rules/connection_settings. Supplementary digits are two surrogate
// chars and therefore rejected by the source Integer.parseInt loop.
fn java_integer(value: &JavaString) -> Option<i32> {
    const ZEROES: &[u16] = &[
        0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6,
        0xc66, 0xce6, 0xd66, 0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090,
        0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90, 0x1b50, 0x1bb0,
        0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50,
        0xabf0, 0xff10,
    ];
    let mut units = value.iter_units().peekable();
    let negative = match units.peek().copied() {
        Some(0x2d) => { units.next(); true },
        Some(0x2b) => { units.next(); false },
        _ => false,
    };
    units.peek()?;
    let limit = if negative { i32::MIN } else { -i32::MAX };
    let mut result = 0_i32;
    for unit in units {
        let at = ZEROES.partition_point(|zero| *zero <= unit);
        let digit = unit.checked_sub(*ZEROES.get(at.checked_sub(1)?)?)?;
        if digit >= 10 || result < limit / 10 { return None; }
        result *= 10;
        if result < limit + i32::from(digit) { return None; }
        result -= i32::from(digit);
    }
    Some(if negative { result } else { -result })
}

fn properties_for(block: &BoundBlockDefinition, state: u32) -> Result<BTreeMap<String, String>> {
    ensure!((block.minimum_state..=block.maximum_state).contains(&state), "state belongs to another block");
    let mut offset = state - block.minimum_state;
    let mut result = BTreeMap::new();
    for property in block.properties.iter().rev() {
        let count = u32::try_from(property.values.len())?;
        ensure!(count != 0, "empty block property domain");
        let value = (offset % count) as usize;
        offset /= count;
        result.insert(property.name.clone(), property.values[value].clone());
    }
    ensure!(offset == 0, "block state cardinality differs from property definitions");
    Ok(result)
}
fn set_property(block: &BoundBlockDefinition, state: u32, name: &str, value: &str) -> Result<u32> {
    let mut values = properties_for(block, state)?;
    *values.get_mut(name).ok_or_else(|| anyhow!("actual property disappeared"))? = value.to_owned();
    let mut offset = 0_u32;
    for property in &block.properties {
        let index = property.values.iter().position(|candidate| Some(candidate) == values.get(&property.name))
            .ok_or_else(|| anyhow!("property value is outside the actual domain"))?;
        offset = offset.checked_mul(u32::try_from(property.values.len())?)
            .and_then(|old| old.checked_add(index as u32)).ok_or_else(|| anyhow!("block state offset overflow"))?;
    }
    let state = block.minimum_state.checked_add(offset).ok_or_else(|| anyhow!("block state overflow"))?;
    ensure!(state <= block.maximum_state, "resulting block state is outside its actual block");
    Ok(state)
}

/// CompoundTag() -> Maps.newHashMap() -> fresh JDK21 HashMap. Pinned writes
/// contain <=7 unique properties (outer compound <=2 keys), so table16 never
/// resizes/treeifies. Bucket iteration and in-bucket insertion order are exact;
/// this is NOT a fallback for an arbitrary existing input compound's history.
fn fresh_compound(mut puts: Vec<(JavaString, Tag)>) -> Result<Tag> {
    ensure!(puts.len() <= 7, "foreign fresh block-state compound cardinality");
    let unique = puts.iter().map(|(key, _)| key.clone()).collect::<BTreeSet<_>>();
    ensure!(unique.len() == puts.len(), "duplicate source block-state puts");
    puts.sort_by_key(|(key, _)| {
        let hash = key.iter_units().fold(0_u32, |hash, unit| hash.wrapping_mul(31).wrapping_add(u32::from(unit)));
        (hash ^ (hash >> 16)) & 15
    });
    Ok(Tag::Compound(puts))
}

#[derive(Deserialize)]
struct ItemCatalogue { version: String, protocol: i32, blocks: BTreeMap<String, JsonBlock> }
#[derive(Deserialize)]
struct JsonBlock { min_state: u32, max_state: u32, default_state: u32, properties: Vec<JsonProperty> }
#[derive(Deserialize)]
struct JsonProperty { name: String, #[serde(rename = "type")] kind: String, num_values: usize, values: Option<Vec<String>> }
#[derive(Deserialize)]
struct PlacementCatalogue { minecraft: String, jar_sha256: String, blocks: Vec<PlacementBlock> }
#[derive(Deserialize)]
struct PlacementBlock { name: String, min: u32, max: u32, default: u32 }

fn pinned_definitions() -> Result<BTreeMap<String, BoundBlockDefinition>> {
    let items: ItemCatalogue = serde_json::from_str(include_str!("../../fixtures/items-1.21.json"))?;
    let placement: PlacementCatalogue = serde_json::from_str(include_str!("../../fixtures/placement-1.21.json"))?;
    ensure!(items.version == "1.21" && items.protocol == 767 && placement.minecraft == "1.21"
        && placement.jar_sha256 == INNER_SHA256 && items.blocks.len() == 1060 && placement.blocks.len() == 1060,
        "foreign compiled block catalogue");
    let mut result = BTreeMap::new();
    for row in placement.blocks {
        let short = row.name.strip_prefix("minecraft:").ok_or_else(|| anyhow!("foreign pinned block key"))?;
        let data = items.blocks.get(short).ok_or_else(|| anyhow!("pinned state catalogue block missing"))?;
        ensure!((row.min, row.max, row.default) == (data.min_state, data.max_state, data.default_state),
            "pinned original/state-catalogue binding differs");
        ensure!(data.properties.len() <= 7, "foreign property cardinality");
        let mut properties = Vec::with_capacity(data.properties.len());
        let mut names = BTreeSet::new();
        let mut cardinality = 1_u32;
        for property in &data.properties {
            ensure!(names.insert(property.name.clone()), "duplicate pinned property name");
            let kind = match property.kind.as_str() {
                "bool" => BlockPropertyKind::Boolean,
                "int" => BlockPropertyKind::Integer,
                "enum" => BlockPropertyKind::Enum,
                _ => return Err(anyhow!("unported block property class")),
            };
            let values = if kind == BlockPropertyKind::Boolean {
                vec!["true".to_owned(), "false".to_owned()]
            } else { property.values.clone().ok_or_else(|| anyhow!("pinned property values missing"))? };
            ensure!(values.len() == property.num_values && !values.is_empty()
                && values.iter().collect::<BTreeSet<_>>().len() == values.len(), "invalid pinned property domain");
            if kind == BlockPropertyKind::Integer {
                let numbers = values.iter().map(|value| value.parse::<i32>()).collect::<std::result::Result<Vec<_>, _>>()?;
                ensure!(numbers.windows(2).all(|pair| pair[0].checked_add(1) == Some(pair[1])),
                    "pinned IntegerProperty is not its contiguous source range");
            }
            cardinality = cardinality.checked_mul(u32::try_from(values.len())?).ok_or_else(|| anyhow!("property cardinality overflow"))?;
            properties.push(BoundBlockProperty { name: property.name.clone(), kind, values });
        }
        ensure!(row.max.checked_sub(row.min).and_then(|n| n.checked_add(1)) == Some(cardinality)
            && (row.min..=row.max).contains(&row.default), "invalid pinned state/default domain");
        let key = row.name.clone();
        ensure!(result.insert(key, BoundBlockDefinition { key: row.name, minimum_state: row.min,
            maximum_state: row.max, default_state: row.default, properties }).is_none(), "duplicate pinned block");
    }
    Ok(result)
}
