//! Current configuration block tags bound once to the real native ID owner.
//!
//! Main retains this owner directly in Fixtures. Capturing equal packet bytes
//! creates a different identity, never a replacement for the current Fixtures
//! owner. Static block IDs are supplied by the canonical inventory catalogue;
//! there is no block RegistryData packet in the default 1.21 configuration.
//! The SAME canonical inventory owner has validated the original bootstrap
//! capture of every raw Block ID, Property class/domain and per-state property
//! entry order before this capability can bind configured tag holders.
use super::{Fixtures, Packet};
use anyhow::{anyhow, ensure, Context, Result};
use mc121_rust::{
    falling_block::block_state_registry::ConfiguredBlockTags,
    inventory::{self, BlockStateCatalog},
    placement::{self, CanonicalPlacementCatalog},
    protocol::Reader,
};
use std::{collections::{BTreeMap, BTreeSet}, fmt, sync::Arc};

const BLOCK_REGISTRY: &str = "minecraft:block";
const MAX_PACKET_BYTES: usize = 2 * 1024 * 1024;
const MAX_COLLECTION: i32 = 65_536;

struct BlockRegistryOwner {
    catalog: BlockStateCatalog,
    placement: CanonicalPlacementCatalog,
    blocks: BTreeMap<u32, NativeBlockId>,
    tags: BTreeMap<String, Vec<u32>>,
    // Retain the admitted source, not a digest-only or byte-equal substitute.
    tags_packet: Arc<[u8]>,
    features_packet: Arc<[u8]>,
}

#[derive(Debug)]
struct NativeBlockId {
    key: String,
    minimum_state: u32,
    maximum_state: u32,
}

/// Retained registry/tag lifetime. Equality is actual owner identity, not data
/// equality or an address of a movable/temporarily borrowed Fixtures value.
#[derive(Clone)]
pub(super) struct NativeBlockStateRegistryBinding(Arc<BlockRegistryOwner>);
impl PartialEq for NativeBlockStateRegistryBinding {
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
}
impl Eq for NativeBlockStateRegistryBinding {}
impl fmt::Debug for NativeBlockStateRegistryBinding {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("NativeBlockStateRegistryBinding").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub(super) struct NativeBlockStateRegistry {
    binding: NativeBlockStateRegistryBinding,
}

impl NativeBlockStateRegistry {
    /// Called exactly once by the actual Fixtures admission owner. Runtime
    /// reload is unsupported; a new Fixtures capture has a new identity.
    pub(super) fn capture(configuration: &[Packet], catalog: BlockStateCatalog,
        placement: CanonicalPlacementCatalog,
    ) -> Result<Self> {
        ensure!(catalog == inventory::block_state_catalog(),
            "configured block tags have a foreign canonical catalogue owner");
        ensure!(catalog.minecraft_version() == Some("1.21") && catalog.protocol_version() == Some(767),
            "configured block tags require the actual native 1.21/protocol-767 catalogue");
        ensure!(placement.identity() == placement::canonical_placement_catalog().identity()
            && placement.minecraft_version() == "1.21"
            && placement.original_inner_sha256()
                == "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
            "configured block tags have a foreign canonical placement owner");
        let metadata = catalog.source_metadata();
        ensure!(metadata.pointer("/oracle/minecraft").and_then(|v| v.as_str()) == Some("1.21")
            && metadata.pointer("/oracle/oracle_inner_sha256").and_then(|v| v.as_str())
                == Some("8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"),
            "configured block ID domain lacks the pinned original source binding");
        let original = catalog.original_owner_facts_metadata();
        ensure!(original["minecraft"] == "1.21" && original["protocol"] == 767
            && original["method"] == "original_1.21_bootstrap_registry_property_and_all_state_capture_v1"
            && original["original_inner_sha256"]
                == "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
            && original["official_mappings_sha1"] == "31c77994d96f05ba25a870ada70f47f315330437"
            && original["block_count"] == 1060 && original["state_count"] == 26_684,
            "configured block IDs lack the actual canonical original owner certificate");

        let mut blocks = BTreeMap::new();
        let mut names = BTreeSet::new();
        let mut states = BTreeSet::new();
        for definition in catalog.block_definitions() {
            let definition = definition?;
            ensure!(definition.minimum_state <= definition.default_state
                && definition.default_state <= definition.maximum_state,
                "native block default lies outside its actual state range");
            let key = format!("minecraft:{}", definition.name);
            ensure!(names.insert(key.clone()), "duplicate native block registry key");
            for state in definition.minimum_state..=definition.maximum_state {
                ensure!(states.insert(state), "overlapping native block state ranges");
            }
            ensure!(blocks.insert(definition.registry_id, NativeBlockId {
                key, minimum_state: definition.minimum_state, maximum_state: definition.maximum_state,
            }).is_none(), "duplicate explicit native raw Block ID");
        }
        ensure!(blocks.len() == 1060 && blocks.keys().copied().eq(0..1060)
            && states.len() == 26_684 && states.iter().copied().eq(0..26_684),
            "configured block tags lack the complete native raw-ID/state domain");
        let mut placed = BTreeSet::new();
        for definition in placement.blocks() {
            ensure!(placed.insert(definition.key), "duplicate canonical placement block key");
            let block = blocks.values().find(|block| block.key == definition.key)
                .context("canonical placement block is absent from actual raw-ID domain")?;
            let native = catalog.state_definition(block.minimum_state)?
                .context("actual canonical block definition is absent")?;
            ensure!(block.minimum_state == definition.minimum_state
                && block.maximum_state == definition.maximum_state
                && native.default_state == definition.default_state,
                "actual placement and explicit raw-ID block domains differ");
            if let Some(raw) = definition.raw_registry_id {
                ensure!(u32::try_from(raw)? == native.registry_id,
                    "canonical placement raw ID differs from actual inventory raw ID");
            }
        }
        ensure!(placed.len() == blocks.len() && placement.state_count() == states.len(),
            "canonical placement omits actual native blocks/states");

        let tags_packet = unique_packet(configuration, 0x0d)?;
        let features_packet = unique_packet(configuration, 0x0c)?;
        let mut features = Reader::new(&features_packet.payload);
        ensure!(features.varint()? == 1 && features.string(32_767)? == "minecraft:vanilla",
            "native block registry requires the actual default enabled-feature profile");
        features.finish()?;
        let tags = decode_block_tags(&tags_packet.payload, &blocks)?;
        Ok(Self { binding: NativeBlockStateRegistryBinding(Arc::new(BlockRegistryOwner {
            catalog, placement, blocks, tags, tags_packet: tags_packet.payload.as_slice().into(),
            features_packet: features_packet.payload.as_slice().into(),
        })) })
    }

    pub(super) fn binding(&self) -> NativeBlockStateRegistryBinding { self.binding.clone() }
    pub(super) fn require_binding(&self, binding: &NativeBlockStateRegistryBinding) -> Result<()> {
        ensure!(&self.binding == binding, "block registry/tag owner was replaced");
        ensure!(self.binding.0.catalog == inventory::block_state_catalog(),
            "block registry's actual catalogue owner changed");
        ensure!(self.binding.0.placement.identity() == placement::canonical_placement_catalog().identity(),
            "block registry's actual placement owner changed");
        Ok(())
    }
    fn require_configuration(&self, configuration: &[Packet]) -> Result<()> {
        // Identity is still the retained owner above. These comparisons only
        // reject in-place mutation of its admitted configuration; equal bytes
        // cannot attach a separately constructed owner to the current view.
        ensure!(unique_packet(configuration, 0x0d)?.payload.as_slice() == &*self.binding.0.tags_packet
            && unique_packet(configuration, 0x0c)?.payload.as_slice() == &*self.binding.0.features_packet,
            "actual block-tag configuration changed without a new retained owner");
        Ok(())
    }
    /// Borrow the field of the actual admitted Fixtures. No independently
    /// captured equal-byte registry can be supplied as this current view.
    pub(super) fn current(fixtures: &Fixtures) -> CurrentBlockTags<'_> {
        CurrentBlockTags { fixtures, binding: fixtures.block_state_registry.binding() }
    }
    pub(super) fn catalog(&self) -> BlockStateCatalog { self.binding.0.catalog }
    pub(super) fn block_key(&self, raw_id: i32) -> Result<&str> {
        Ok(&self.block(raw_id)?.key)
    }
    pub(super) fn raw_block_id(&self, state: u32) -> Result<i32> {
        let definition = self.binding.0.catalog.state_definition(state)?
            .context("state is absent from the actual native block registry")?;
        let raw = i32::try_from(definition.registry_id)?;
        let block = self.block(raw)?;
        ensure!(state >= block.minimum_state && state <= block.maximum_state
            && block.key.strip_prefix("minecraft:") == Some(definition.name),
            "actual state/raw Block ID binding changed");
        Ok(raw)
    }
    pub(super) fn tag_exists(&self, key: &str) -> Result<bool> {
        self.require_binding(&self.binding)?;
        Ok(self.binding.0.tags.contains_key(key))
    }
    pub(super) fn block_in_tag(&self, raw_id: i32, key: &str) -> Result<bool> {
        self.block(raw_id)?;
        let holders = self.binding.0.tags.get(key).context("actual configured block tag is absent")?;
        Ok(holders.contains(&u32::try_from(raw_id)?))
    }
    pub(super) fn state_in_tag(&self, state: u32, key: &str) -> Result<bool> {
        self.block_in_tag(self.raw_block_id(state)?, key)
    }
    fn block(&self, raw_id: i32) -> Result<&NativeBlockId> {
        self.require_binding(&self.binding)?;
        self.binding.0.blocks.get(&u32::try_from(raw_id)?).context("unknown actual raw Block ID")
    }
}

pub(super) struct CurrentBlockTags<'a> {
    fixtures: &'a Fixtures,
    binding: NativeBlockStateRegistryBinding,
}
impl CurrentBlockTags<'_> {
    fn owner(&self) -> Result<&NativeBlockStateRegistry> {
        self.fixtures.block_state_registry.require_binding(&self.binding)?;
        self.fixtures.block_state_registry.require_configuration(&self.fixtures.configuration)?;
        Ok(&self.fixtures.block_state_registry)
    }
}
impl ConfiguredBlockTags for CurrentBlockTags<'_> {
    type Identity = NativeBlockStateRegistryBinding;
    fn identity(&self) -> Result<Self::Identity> { Ok(self.owner()?.binding()) }
    fn raw_block_key(&self, raw: u32) -> Result<&str> {
        self.owner()?.block_key(i32::try_from(raw)?)
    }
    fn raw_block_id(&self, state: u32) -> Result<u32> {
        Ok(u32::try_from(self.owner()?.raw_block_id(state)?)?)
    }
    fn tag_exists(&self, key: &str) -> Result<bool> { self.owner()?.tag_exists(key) }
    fn block_in_tag(&self, raw: u32, key: &str) -> Result<bool> {
        self.owner()?.block_in_tag(i32::try_from(raw)?, key)
    }
}

fn unique_packet(configuration: &[Packet], id: i32) -> Result<&Packet> {
    let mut packets = configuration.iter().filter(|packet| packet.id == id);
    let packet = packets.next().context("actual block-tag configuration packet is absent")?;
    ensure!(packets.next().is_none(), "duplicate actual block-tag configuration packet");
    ensure!(packet.payload.len() <= MAX_PACKET_BYTES, "block-tag configuration packet exceeds native limit");
    Ok(packet)
}
fn count(reader: &mut Reader<'_>) -> Result<usize> {
    let count = reader.varint()?;
    ensure!((0..=MAX_COLLECTION).contains(&count), "invalid block-tag collection count");
    Ok(count as usize)
}
fn decode_block_tags(bytes: &[u8], blocks: &BTreeMap<u32, NativeBlockId>)
    -> Result<BTreeMap<String, Vec<u32>>> {
    let mut reader = Reader::new(bytes);
    let mut seen_registries = BTreeSet::new();
    let mut result = None;
    for _ in 0..count(&mut reader)? {
        let registry = reader.string(32_767)?;
        ensure!(seen_registries.insert(registry.clone()), "duplicate actual tag registry");
        let mut tags = BTreeMap::new();
        for _ in 0..count(&mut reader)? {
            let key = reader.string(32_767)?;
            let mut holders = Vec::new();
            for _ in 0..count(&mut reader)? {
                let raw = reader.varint()?;
                if registry == BLOCK_REGISTRY {
                    let raw = u32::try_from(raw).context("negative actual block-tag raw ID")?;
                    ensure!(blocks.contains_key(&raw), "configured block tag has an unknown raw Block ID");
                    holders.push(raw);
                }
            }
            ensure!(tags.insert(key, holders).is_none(), "duplicate actual configured block tag");
        }
        if registry == BLOCK_REGISTRY { result = Some(tags); }
    }
    reader.finish()?;
    result.ok_or_else(|| anyhow!("actual minecraft:block tag registry is absent"))
}
