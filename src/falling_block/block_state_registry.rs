//! Block-state registry adapters over the retained native catalogue owners.
//!
//! Configured tags must borrow the admitted registry/tag owner. Equal packet
//! bytes or a caller-created catalogue are not owner-identity certificates.

use super::block_state_codec::{
    ActualBlockRegistry, BlockPropertyKind, BlockStateNbtContext, BoundBlockDefinition,
    BoundBlockProperty, PinnedBlockStateCodec, INVALID_PROPERTY_WARNING,
};
use super::persistence::FallingBlockStateCodec;
use crate::{
    inventory::{self, BlockStateCatalog, BlockStateDefinitionRef, CatalogBlockPropertyKind},
    lighting,
    nbt::{JavaString, Tag},
    placement::{self, CanonicalPlacementCatalog, CanonicalPlacementCatalogIdentity},
    source_nbt_decode::SourceCompoundView,
};
use anyhow::{anyhow, ensure, Context, Result};
use std::collections::{BTreeMap, BTreeSet};

const INNER_SHA256: &str = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
const MAPPINGS_SHA1: &str = "31c77994d96f05ba25a870ada70f47f315330437";
const STATE_COUNT: usize = 26_684;
const BLOCK_COUNT: usize = 1_060;
const ANVIL_TAG: &str = "minecraft:anvil";

/// The actual current configuration's block-ID and tag owner.
/// Unknown raw IDs and absent tags fail explicitly; a known empty tag may
/// return false. Identity is the retained owner, never value equality.
pub trait ConfiguredBlockTags {
    type Identity: Clone + Eq;
    fn identity(&self) -> Result<Self::Identity>;
    fn raw_block_key(&self, raw: u32) -> Result<&str>;
    fn raw_block_id(&self, state: u32) -> Result<u32>;
    fn tag_exists(&self, key: &str) -> Result<bool>;
    fn block_in_tag(&self, raw: u32, key: &str) -> Result<bool>;
}

/// Opaque composite identity of the SAME two canonical catalogues and actual
/// current configured-tag owner. Its fields cannot bless a replacement owner.
#[derive(Clone, PartialEq, Eq)]
pub struct CanonicalBlockStateRegistryIdentity<I: Clone + Eq> {
    placement: CanonicalPlacementCatalogIdentity,
    inventory: BlockStateCatalog,
    tags: I,
}

/// Concrete codec registry borrowing the real retained native owners.
/// No second catalogue is parsed, allocated or reconstructed here. The
/// inventory OnceLock already verifies original raw IDs, Property classes,
/// serialized domains and every state's StateHolder entry order against the
/// original bootstrap certificate and this SAME placement owner.
pub struct CanonicalBlockStateRegistry<'a, T: ConfiguredBlockTags + ?Sized> {
    placement: CanonicalPlacementCatalog,
    inventory: BlockStateCatalog,
    tags: &'a T,
    identity: CanonicalBlockStateRegistryIdentity<T::Identity>,
}

impl<'a, T: ConfiguredBlockTags + ?Sized> CanonicalBlockStateRegistry<'a, T> {
    pub fn capture(tags: &'a T) -> Result<Self> {
        let placement = placement::canonical_placement_catalog();
        let inventory = inventory::block_state_catalog();
        let identity = CanonicalBlockStateRegistryIdentity {
            placement: placement.identity(), inventory, tags: tags.identity()?,
        };
        let registry = Self { placement, inventory, tags, identity };
        registry.require_current()?;
        ensure!(inventory.minecraft_version() == Some("1.21")
            && inventory.protocol_version() == Some(767)
            && placement.minecraft_version() == "1.21"
            && placement.original_inner_sha256() == INNER_SHA256
            && placement.state_count() == STATE_COUNT,
            "native block-state registry has a foreign canonical source domain");
        let proof = inventory.original_owner_facts_metadata();
        ensure!(proof["minecraft"] == "1.21" && proof["protocol"] == 767
            && proof["method"] == "original_1.21_bootstrap_registry_property_and_all_state_capture_v1"
            && proof["original_inner_sha256"] == INNER_SHA256
            && proof["official_mappings_sha1"] == MAPPINGS_SHA1
            && proof["block_count"] == BLOCK_COUNT && proof["state_count"] == STATE_COUNT,
            "native block-state registry lacks its actual original owner certificate");
        // Compare retained owner rows, not a parse of the same fixture into an
        // unrelated registry. Raw Block IDs come ONLY from original-verified
        // inventory rows; placement rows intentionally do not invent them.
        let mut placement_rows = BTreeMap::new();
        for row in placement.blocks() {
            ensure!(placement_rows.insert(row.key, row).is_none(), "duplicate canonical placement block");
        }
        let mut raw_ids = BTreeSet::new();
        let mut covered = vec![false; STATE_COUNT];
        let mut block_count = 0;
        for definition in inventory.block_definitions() {
            let definition = definition?;
            let key = qualified_key(definition.name);
            let row = placement_rows.remove(key.as_str())
                .with_context(|| format!("canonical placement omits {key}"))?;
            ensure!(row.minimum_state == definition.minimum_state
                && row.maximum_state == definition.maximum_state
                && row.default_state == definition.default_state,
                "canonical block owners disagree for {key}");
            if let Some(raw) = row.raw_registry_id {
                ensure!(u32::try_from(raw)? == definition.registry_id,
                    "canonical placement raw Block ID differs for {key}");
            }
            ensure!(raw_ids.insert(definition.registry_id), "duplicate actual raw Block ID");
            ensure!(tags.raw_block_key(definition.registry_id)? == key,
                "current configured raw Block ID differs for {key}");
            ensure!(definition.minimum_state <= definition.default_state
                && definition.default_state <= definition.maximum_state,
                "actual default state is outside its block domain");
            for state in definition.minimum_state..=definition.maximum_state {
                let slot = covered.get_mut(state as usize)
                    .ok_or_else(|| anyhow!("foreign canonical block-state ID"))?;
                ensure!(!*slot, "overlapping canonical block-state domains");
                *slot = true;
                ensure!(tags.raw_block_id(state)? == definition.registry_id,
                    "current configured state/raw-Block binding differs at {state}");
            }
            block_count += 1;
        }
        ensure!(block_count == BLOCK_COUNT && placement_rows.is_empty()
            && covered.into_iter().all(|covered| covered),
            "canonical block-state registry is incomplete");
        ensure!(tags.tag_exists(ANVIL_TAG)?, "current configured ANVIL tag is absent");
        registry.require_current()?;
        Ok(registry)
    }

    fn require_current(&self) -> Result<()> {
        ensure!(self.placement.identity() == self.identity.placement
            && self.identity.placement == placement::canonical_placement_catalog().identity()
            && self.inventory == self.identity.inventory
            && self.inventory == inventory::block_state_catalog()
            && self.tags.identity()? == self.identity.tags,
            "native block-state registry or configured-tag owner changed");
        Ok(())
    }

    fn state_definition(&self, state: u32) -> Result<BlockStateDefinitionRef> {
        self.require_current()?;
        let definition = self.inventory.state_definition(state)?
            .with_context(|| format!("unknown canonical block-state ID {state}"))?;
        let placement = self.placement.state_definition(state)?;
        let key = qualified_key(definition.name);
        ensure!(placement.key == key
            && placement.minimum_state == definition.minimum_state
            && placement.maximum_state == definition.maximum_state
            && placement.default_state == definition.default_state
            && self.tags.raw_block_id(state)? == definition.registry_id
            && self.tags.raw_block_key(definition.registry_id)? == key,
            "current block-state/raw-Block/catalogue binding differs at {state}");
        Ok(definition)
    }

    fn original_metadata_str(&self, key: &str) -> Result<&'static str> {
        self.require_current()?;
        self.inventory.original_owner_facts_metadata()[key].as_str()
            .with_context(|| format!("actual block-state source metadata lacks {key}"))
    }
}

impl<T: ConfiguredBlockTags + ?Sized> ActualBlockRegistry for CanonicalBlockStateRegistry<'_, T> {
    type Identity = CanonicalBlockStateRegistryIdentity<T::Identity>;
    fn identity(&self) -> Result<Self::Identity> {
        self.require_current()?;
        Ok(self.identity.clone())
    }
    fn minecraft_version(&self) -> Result<&str> { self.original_metadata_str("minecraft") }
    fn protocol_version(&self) -> Result<i32> {
        self.require_current()?;
        i32::try_from(self.inventory.original_owner_facts_metadata()["protocol"].as_i64()
            .context("actual block-state source metadata lacks protocol")?).map_err(Into::into)
    }
    fn original_inner_sha256(&self) -> Result<&str> { self.original_metadata_str("original_inner_sha256") }
    fn official_mappings_sha1(&self) -> Result<&str> { self.original_metadata_str("official_mappings_sha1") }
    fn block_definitions(&self) -> Result<Vec<BoundBlockDefinition>> {
        self.require_current()?;
        self.inventory.block_definitions().map(|definition| {
            let definition = definition?;
            let properties = definition.properties().map(|property| {
                let property = property?;
                let kind = match property.kind {
                    CatalogBlockPropertyKind::Boolean => {
                        ensure!(property.original_class == "dtt" && property.original_value_class == "java.lang.Boolean",
                            "actual BooleanProperty class binding differs");
                        BlockPropertyKind::Boolean
                    },
                    CatalogBlockPropertyKind::Integer => {
                        ensure!(property.original_class == "duc" && property.original_value_class == "java.lang.Integer",
                            "actual IntegerProperty class binding differs");
                        BlockPropertyKind::Integer
                    },
                    CatalogBlockPropertyKind::Enum => {
                        ensure!(matches!(property.original_class, "dua" | "dtw")
                            && !property.original_value_class.is_empty(), "actual EnumProperty class binding differs");
                        BlockPropertyKind::Enum
                    },
                };
                let values = (0..property.values.len()).map(|ordinal| {
                    property.values.get(ordinal).map(str::to_owned)
                        .context("actual Property serialized value is absent")
                }).collect::<Result<Vec<_>>>()?;
                Ok(BoundBlockProperty { name: property.name.to_owned(), kind, values })
            }).collect::<Result<Vec<_>>>()?;
            Ok(BoundBlockDefinition { key: qualified_key(definition.name),
                minimum_state: definition.minimum_state, maximum_state: definition.maximum_state,
                default_state: definition.default_state, properties })
        }).collect()
    }
    fn state_properties(&self, state: u32) -> Result<BTreeMap<String, String>> {
        self.state_definition(state)?;
        self.inventory.state_properties(state)
            .with_context(|| format!("actual canonical property values unavailable for {state}"))
    }
    fn state_property_order(&self, state: u32) -> Result<Vec<String>> {
        let definition = self.state_definition(state)?;
        let order = definition.state_holder_property_entry_order()
            .context("actual StateHolder property entry order is unavailable")?;
        Ok(order.iter().map(str::to_owned).collect())
    }
    fn state_is_air(&self, state: u32) -> Result<bool> {
        self.state_definition(state)?;
        // Same canonical original per-state predicate used by Main's falling
        // host; this includes cave_air/void_air, unlike an AIR-key comparison.
        Ok(lighting::state_properties(state)?.is_air())
    }
    fn state_is_anvil(&self, state: u32) -> Result<bool> {
        let definition = self.state_definition(state)?;
        ensure!(self.tags.tag_exists(ANVIL_TAG)?, "current configured ANVIL tag is absent");
        self.tags.block_in_tag(definition.registry_id, ANVIL_TAG)
    }
}

fn qualified_key(name: &str) -> String { format!("minecraft:{name}") }

/// Mandatory actual warning owner. The caller supplies its genuine formatter
/// and logger; this module neither discards warnings nor fabricates SNBT text.
/// The final argument remains a decoder-issued compound capability, allowing
/// a source formatter to observe the SAME original object's actual key order.
pub trait BlockStateWarningSink {
    fn warn_invalid_property(
        &mut self,
        message: &'static str,
        property: &JavaString,
        value: &JavaString,
        original: SourceCompoundView<'_>,
    ) -> Result<()>;
}

/// Concrete input-map context issued ONLY from an immutable binary-loaded
/// compound view and an explicit warning sink. Cloning/equal Tags or arbitrary
/// vector entries cannot construct the source ownership capability.
pub struct DecodedBlockStateNbtContext<'d, W: BlockStateWarningSink + ?Sized> {
    original: SourceCompoundView<'d>,
    warnings: &'d mut W,
}
impl<'d, W: BlockStateWarningSink + ?Sized> DecodedBlockStateNbtContext<'d, W> {
    pub fn new(original: SourceCompoundView<'d>, warnings: &'d mut W) -> Self {
        Self { original, warnings }
    }
}
impl<W: BlockStateWarningSink + ?Sized> BlockStateNbtContext for DecodedBlockStateNbtContext<'_, W> {
    fn property_key_order(&mut self, properties: &Tag) -> Result<Vec<JavaString>> {
        let actual = self.original.child_compound("Properties")
            .context("source-decoded BlockState has no actual Properties compound")?;
        ensure!(actual.matches_node(properties), "BlockState Properties is not the retained source-decoded node");
        Ok(actual.key_order())
    }
    fn warn_invalid_property(&mut self, message: &'static str, property: &JavaString,
        value: &JavaString, original_block_state: &Tag,
    ) -> Result<()> {
        ensure!(self.original.matches_node(original_block_state),
            "BlockState warning input is not the retained source-decoded node");
        ensure!(message == INVALID_PROPERTY_WARNING, "foreign BlockState property warning template");
        self.warnings.warn_invalid_property(message, property, value, self.original)
    }
}

/// Enforces decoder-issued input identity BEFORE every read, including missing
/// Name/Properties cases where the core algorithm performs no map callback.
/// The caller must pass the actual `SourceCompoundView::tag()`; an equal clone
/// is rejected. Output remains the core source fresh-compound implementation.
pub struct SourceBoundBlockStateCodec<'a, 'd: 'a, R: ActualBlockRegistry,
    W: BlockStateWarningSink + ?Sized,
> {
    original: SourceCompoundView<'d>,
    codec: PinnedBlockStateCodec<'a, R, DecodedBlockStateNbtContext<'d, W>>,
}
impl<'a, 'd: 'a, R: ActualBlockRegistry, W: BlockStateWarningSink + ?Sized>
    SourceBoundBlockStateCodec<'a, 'd, R, W>
{
    pub fn capture(registry: &'a R, context: &'a mut DecodedBlockStateNbtContext<'d, W>) -> Result<Self> {
        let original = context.original;
        Ok(Self { original, codec: PinnedBlockStateCodec::capture(registry, context)? })
    }
}
impl<R: ActualBlockRegistry, W: BlockStateWarningSink + ?Sized> FallingBlockStateCodec
    for SourceBoundBlockStateCodec<'_, '_, R, W>
{
    fn write_block_state(&mut self, state: u32) -> Result<Tag> { self.codec.write_block_state(state) }
    fn read_block_state(&mut self, input: &Tag) -> Result<u32> {
        ensure!(self.original.matches_node(input), "BlockState read input is not the retained source-decoded node");
        self.codec.read_block_state(input)
    }
    fn state_is_air(&mut self, state: u32) -> Result<bool> { self.codec.state_is_air(state) }
    fn state_is_anvil(&mut self, state: u32) -> Result<bool> { self.codec.state_is_anvil(state) }
    fn default_sand_state(&mut self) -> Result<u32> { self.codec.default_sand_state() }
}
