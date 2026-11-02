//! Shared source-bound configured enchantment metadata and item mutation.
//! No world, player, effect evaluator, XP owner or table menu is created here.
use super::{Fixtures, Packet};
use anyhow::{anyhow, ensure, Context, Result};
use mc121_rust::{
    inventory::{self, Component, ItemStack},
    nbt::Tag,
    protocol::{Reader, Writer},
    workstations::enchanting::{EnchantmentCost, EnchantmentDefinition, EnchantmentInstance, EnchantmentRegistry},
};
use serde::Deserialize;
use std::{collections::{BTreeMap, BTreeSet}, fmt::Debug, sync::Arc};

const ORIGINAL_INNER_SHA256: &str = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
const ORIGINAL_MAPPINGS_SHA1: &str = "31c77994d96f05ba25a870ada70f47f315330437";
const ITEM_CERTIFICATE_SHA256: &str = "828c99f328a9dc684e66582ed07c26c2417a99f3df5dc364b6a0710d0f5e8da5";
const ENCHANTMENT_REGISTRY: &str = "minecraft:enchantment";
const ITEM_REGISTRY: &str = "minecraft:item";

#[derive(Debug)]
struct NativeEnchantmentOwner {
    registry: Arc<[u8]>,
    tags: Arc<[u8]>,
    features: Arc<[u8]>,
    item_facts_sha256: String,
}

/// The retained decoded registry owns holder identity. A borrowed Fixtures
/// address can move or be reused, and equal packet bytes do not make two
/// separately constructed registry owners interchangeable.
#[derive(Clone)]
pub(super) struct NativeEnchantmentSnapshot(Arc<NativeEnchantmentOwner>);
impl PartialEq for NativeEnchantmentSnapshot {
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
}
impl Eq for NativeEnchantmentSnapshot {}
impl Debug for NativeEnchantmentSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeEnchantmentSnapshot")
            .field("item_certificate", &self.0.item_facts_sha256)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeEnchantmentHolder {
    owner: NativeEnchantmentSnapshot,
    id: i32,
}

/// An explicit actual reference-map owner may implement this capability. The
/// order must follow Mutable's default Object2IntOpenHashMap + putAll + upgrade
/// for the SAME real holder references, not sorted IDs or the input wire order.
/// No such implementation is supplied here. The owner must belong to the same
/// genuine host transaction as the item; preparation cannot mutate canonical
/// map history or publish a new component before that transaction commits.
pub(super) trait NativeEnchantmentMapOrder: Debug + Send + Sync {
    fn snapshot(&self) -> NativeEnchantmentSnapshot;
    /// Require the SAME actual component-map reference/history for this stack
    /// in its canonical or genuinely staged owner. Equality of wire bytes or
    /// level values cannot certify this binding.
    fn require_component_owner(&self, item: &ItemStack, component: i32) -> Result<()>;
    fn existing_entries_order(&self, item: &ItemStack, component: i32,
        values: &BTreeMap<i32, i32>) -> Result<Vec<i32>>;
    fn mutable_upgrade_order(&self, item: &ItemStack, component: i32,
        before_values: &BTreeMap<i32, i32>, upgraded: &NativeEnchantmentHolder,
        requested_level: i32,
        resulting_levels: &BTreeMap<i32, i32>) -> Result<Vec<i32>>;
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum ItemPredicate {
    #[serde(rename = "default_max_stack_one_and_max_damage_present")]
    Default,
    #[serde(rename = "count_one")]
    CountOne,
    #[serde(rename = "never")]
    Never,
}
#[derive(Clone, Debug, Deserialize)]
struct ItemFact {
    id: i32,
    key: String,
    enchantment_value: i32,
    predicate: ItemPredicate,
    predicate_owner: String,
}
#[derive(Deserialize)]
struct ItemProvenance {
    inner_server_sha256: String,
    server_mappings_sha1: String,
    native_catalog_sha256: String,
    original_item_registry_size: usize,
    all_native_item_id_keys_match: bool,
}
#[derive(Deserialize)]
struct ItemCertificateFile {
    schema_version: i32,
    minecraft: String,
    protocol: i32,
    provenance: ItemProvenance,
    items: Vec<ItemFact>,
}

/// REQUIRED original-class certificate; there is no missing-item zero/value
/// fallback and no inference from an item's suffix or prototype boolean.
#[derive(Clone, Debug)]
pub(super) struct NativeItemEnchantmentCertificate {
    facts: Arc<[ItemFact]>,
    sha256: String,
}
impl NativeItemEnchantmentCertificate {
    pub(super) fn pinned() -> Result<Self> {
        let bytes = include_bytes!("../fixtures/native-item-enchantability.json");
        let certificate_digest = openssl::sha::sha256(bytes).iter()
            .map(|byte| format!("{byte:02x}")).collect::<String>();
        ensure!(certificate_digest == ITEM_CERTIFICATE_SHA256,
            "item enchantment facts differ from the inspected original-class certificate");
        let file: ItemCertificateFile = serde_json::from_slice(bytes)?;
        ensure!(file.schema_version == 1 && file.minecraft == "1.21" && file.protocol == 767
            && file.provenance.inner_server_sha256 == ORIGINAL_INNER_SHA256
            && file.provenance.server_mappings_sha1 == ORIGINAL_MAPPINGS_SHA1,
            "item enchantment certificate has no matching original source provenance");
        ensure!(!file.items.is_empty(), "missing item enchantment facts");
        let catalog_digest = openssl::sha::sha256(include_bytes!("../fixtures/items-1.21.json"))
            .iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        ensure!(file.provenance.native_catalog_sha256 == catalog_digest
            && file.provenance.original_item_registry_size == file.items.len()
            && file.provenance.all_native_item_id_keys_match,
            "item enchantment certificate lacks the matching full original/native item observation");
        for (index, fact) in file.items.iter().enumerate() {
            ensure!(fact.id == i32::try_from(index)?
                && inventory::item_name(fact.id).map(|name| format!("minecraft:{name}")) == Some(fact.key.clone()),
                "item enchantment certificate differs from the pinned item ID owner");
            ensure!(fact.enchantment_value >= 0, "invalid original item enchantment value");
            let owner = match fact.predicate {
                ItemPredicate::Default => fact.predicate_owner == "net.minecraft.world.item.Item",
                ItemPredicate::CountOne => fact.predicate_owner == "net.minecraft.world.item.BookItem",
                ItemPredicate::Never => matches!(fact.predicate_owner.as_str(),
                    "net.minecraft.world.item.AnimalArmorItem" | "net.minecraft.world.item.EnchantedBookItem"),
            };
            ensure!(owner, "item enchantability callback lacks its source-proven declaring owner");
        }
        ensure!(inventory::item_name(i32::try_from(file.items.len())?).is_none(),
            "incomplete original item enchantment certificate");
        Ok(Self {
            facts: file.items.into(),
            sha256: certificate_digest,
        })
    }

    fn fact(&self, stack: &ItemStack) -> Result<&ItemFact> {
        // Source getItem dispatches empty stacks to AIR. Its effective component
        // view is empty too; no raw empty tool/book callback is substituted.
        let id = if stack.is_empty() { 0 } else { stack.item_id };
        self.facts.get(usize::try_from(id)?).filter(|fact| fact.id == id)
            .context("item has no actual enchantment/class certificate")
    }
}

#[derive(Clone, Debug)]
pub(super) struct NativeEnchantmentDefinition {
    pub(super) key: String,
    pub(super) table: EnchantmentDefinition,
    pub(super) anvil_cost: i32,
    pub(super) supported_items: Vec<i32>,
    pub(super) primary_items: Option<Vec<i32>>,
    pub(super) exclusive_set: Vec<i32>,
}

#[derive(Clone, Debug)]
pub(super) struct NativeEnchantmentRegistry {
    owner: NativeEnchantmentSnapshot,
    definitions: Arc<[NativeEnchantmentDefinition]>,
    enchantment_tags: Arc<BTreeMap<String, Vec<i32>>>,
    items: NativeItemEnchantmentCertificate,
    map_order: Option<Arc<dyn NativeEnchantmentMapOrder>>,
}

impl NativeEnchantmentRegistry {
    pub(super) fn new(fixtures: &Fixtures, items: NativeItemEnchantmentCertificate) -> Result<Self> {
        let registry = actual_registry_packet(fixtures)?;
        let tags = unique_packet(fixtures, 0x0d)?;
        let features = unique_packet(fixtures, 0x0c)?;
        let mut flags = Reader::new(&features.payload);
        ensure!(flags.varint()? == 1 && flags.string(32_767)? == "minecraft:vanilla",
            "native enchantments require the actual default enabled-feature profile");
        flags.finish()?;
        let owner = NativeEnchantmentSnapshot(Arc::new(NativeEnchantmentOwner {
            registry: Arc::from(registry.payload.as_slice()), tags: Arc::from(tags.payload.as_slice()),
            features: Arc::from(features.payload.as_slice()), item_facts_sha256: items.sha256.clone(),
        }));
        let metadata: serde_json::Value = serde_json::from_str(include_str!("../tools/crafting_metadata.json"))?;
        let ids = metadata["registries"][ENCHANTMENT_REGISTRY].as_object().context("native enchantment holder domain absent")?;
        let mut expected = vec![String::new(); ids.len()];
        for (key, id) in ids {
            let id = usize::try_from(id.as_u64().context("invalid pinned enchantment ID")?)?;
            ensure!(id < expected.len() && expected[id].is_empty(), "invalid pinned enchantment holder domain");
            expected[id] = key.clone();
        }
        let item_ids = items.facts.iter().map(|fact| (fact.key.clone(), fact.id)).collect::<BTreeMap<_, _>>();
        let (item_tags, enchantment_tags) = decode_tags(&tags.payload, items.facts.len(), expected.len())?;
        let mut fields = Reader::new(&registry.payload);
        ensure!(fields.string(32_767)? == ENCHANTMENT_REGISTRY, "wrong actual enchantment registry");
        let length = count(&mut fields)?;
        ensure!(length == expected.len() && length == 42, "unsupported native enchantment holder domain");
        let mut encoded = Vec::with_capacity(length);
        for (id, expected_key) in expected.iter().enumerate() {
            let key = fields.string(32_767)?;
            ensure!(&key == expected_key, "configured/native enchantment holder IDs differ");
            ensure!(fields.bool()?, "actual enchantment definition omitted; no known-pack definition owner supplied");
            let (data, used) = Tag::decode_anonymous_prefix(fields.remaining())?;
            fields.bytes(used)?;
            encoded.push((i32::try_from(id)?, key, data));
        }
        fields.finish()?;
        let enchantment_ids = encoded.iter().map(|(id, key, _)| (key.clone(), *id)).collect::<BTreeMap<_, _>>();
        let mut definitions = Vec::with_capacity(length);
        for (_, key, data) in encoded {
            let supported = holder_set(data.get("supported_items").context("enchantment supported_items absent")?, &item_ids, &item_tags)?;
            let primary = data.get("primary_items").map(|value| holder_set(value, &item_ids, &item_tags)).transpose()?;
            let exclusive = data.get("exclusive_set").map(|value| holder_set(value, &enchantment_ids, &enchantment_tags))
                .transpose()?.unwrap_or_default(); // original optionalFieldOf(HolderSet.direct())
            let definition = EnchantmentDefinition {
                weight: int(&data, "weight")?, max_level: int(&data, "max_level")?,
                minimum_cost: cost(&data, "min_cost")?, maximum_cost: cost(&data, "max_cost")?,
            };
            ensure!((1..=1024).contains(&definition.weight) && (1..=255).contains(&definition.max_level),
                "unsupported configured enchantment weight/level");
            definitions.push(NativeEnchantmentDefinition { key, table: definition, anvil_cost: int(&data, "anvil_cost")?,
                supported_items: supported, primary_items: primary, exclusive_set: exclusive });
        }
        Ok(Self { owner, definitions: definitions.into(), enchantment_tags: Arc::new(enchantment_tags), items, map_order: None })
    }

    pub(super) fn with_map_order(mut self, order: Arc<dyn NativeEnchantmentMapOrder>) -> Result<Self> {
        ensure!(order.snapshot() == self.owner, "enchantment map order belongs to another actual holder owner");
        self.map_order = Some(order);
        Ok(self)
    }
    pub(super) fn binding(&self) -> NativeEnchantmentSnapshot { self.owner.clone() }
    /// Check the relevant CURRENT Main configuration without reconstructing
    /// this registry or accepting another registry's equal-looking holders.
    pub(super) fn require_configuration(&self, fixtures: &Fixtures) -> Result<()> {
        ensure!(actual_registry_packet(fixtures)?.payload.as_slice() == &*self.owner.0.registry
            && unique_packet(fixtures, 0x0d)?.payload.as_slice() == &*self.owner.0.tags
            && unique_packet(fixtures, 0x0c)?.payload.as_slice() == &*self.owner.0.features,
            "actual enchantment configuration changed after registry capture");
        Ok(())
    }
    /// Full ItemStack.isEnchantable gate. The trait's item callback below is
    /// only the declaring Item class predicate; this additionally requires
    /// the actual normal ENCHANTMENTS component to exist and be logically empty.
    pub(super) fn stack_is_enchantable(&self, item: &ItemStack) -> Result<bool> {
        if item.is_empty() || !self.item_is_enchantable(item)? { return Ok(false); }
        let Some(bytes) = item.effective_component_bytes(9) else { return Ok(false); };
        Ok(self.read_map(bytes)?.0.is_empty())
    }
    pub(super) fn holder(&self, key: &str) -> Result<NativeEnchantmentHolder> {
        let id = self.definitions.iter().position(|definition| definition.key == key)
            .context("unresolved configured enchantment key")?;
        self.holder_by_id(i32::try_from(id)?)
    }
    pub(super) fn holder_by_id(&self, id: i32) -> Result<NativeEnchantmentHolder> {
        self.definitions.get(usize::try_from(id)?).context("unresolved configured enchantment ID")?;
        Ok(NativeEnchantmentHolder { owner: self.owner.clone(), id })
    }
    pub(super) fn details(&self, holder: &NativeEnchantmentHolder) -> Result<&NativeEnchantmentDefinition> {
        ensure!(holder.owner == self.owner, "enchantment holder belongs to another actual registry snapshot");
        self.definitions.get(usize::try_from(holder.id)?).context("unresolved actual enchantment holder")
    }
    /// Preserve the actual configured tag's encounter vector; absence is
    /// distinct from a present empty named set.
    pub(super) fn tag_holders(&self, key: &str) -> Result<Option<Vec<NativeEnchantmentHolder>>> {
        self.enchantment_tags.get(key).map(|ids|
            ids.iter().map(|id| self.holder_by_id(*id)).collect()).transpose()
    }
    pub(super) fn holder_is_in_tag(&self, holder: &NativeEnchantmentHolder, key: &str) -> Result<bool> {
        self.details(holder)?;
        Ok(self.enchantment_tags.get(key).is_some_and(|ids| ids.contains(&holder.id)))
    }
    pub(super) fn is_supported_item(&self, holder: &NativeEnchantmentHolder, item: &ItemStack) -> Result<bool> {
        // ItemStack.is(HolderSet) queries its effective item holder, AIR when
        // empty. Do not replace that source membership query with a guessed
        // empty=false rule for an actual configured set which can contain AIR.
        Ok(self.details(holder)?.supported_items.contains(&self.items.fact(item)?.id))
    }
    /// Values only. Its map ordering is deliberately not an authorization to
    /// iterate source ItemEnchantments for an order-dependent anvil operation.
    pub(super) fn enchantment_values(&self, item: &ItemStack) -> Result<BTreeMap<i32, i32>> {
        let component = crafting_component(item);
        let Some(bytes) = item.effective_component_bytes(component) else { return Ok(BTreeMap::new()); };
        let (levels, _) = self.read_map(bytes)?;
        Ok(levels.into_iter().collect())
    }
    pub(super) fn enchantments_in_source_order(&self, item: &ItemStack) -> Result<Vec<EnchantmentInstance<NativeEnchantmentHolder>>> {
        let component = crafting_component(item);
        let Some(bytes) = item.effective_component_bytes(component) else { return Ok(Vec::new()); };
        let levels = self.read_map(bytes)?.0.into_iter().collect::<BTreeMap<_, _>>();
        let order = if levels.len() <= 1 { levels.keys().copied().collect::<Vec<_>>() }
        else {
            let owner = self.map_order.as_ref().context(
                "multi-holder enchantment iteration requires the actual Holder-reference OpenHashMap encounter owner")?;
            ensure!(owner.snapshot() == self.owner, "enchantment reference-map owner changed");
            owner.require_component_owner(item, component)?;
            owner.existing_entries_order(item, component, &levels)?
        };
        validate_order(&order, &levels)?;
        order.into_iter().map(|id| Ok(EnchantmentInstance { holder: self.holder_by_id(id)?, level: levels[&id] })).collect()
    }
    fn read_map(&self, bytes: &[u8]) -> Result<(Vec<(i32, i32)>, bool)> {
        let mut fields = Reader::new(bytes);
        let mut values: Vec<(i32, i32)> = Vec::new();
        for _ in 0..count(&mut fields)? {
            let id = fields.varint()?; self.holder_by_id(id)?;
            let level = fields.varint()?;
            ensure!((0..=255).contains(&level), "invalid actual enchantment component level");
            if let Some((_, value)) = values.iter_mut().find(|(existing, _)| *existing == id) { *value = level; }
            else { values.push((id, level)); }
        }
        let tooltip = fields.bool()?;
        fields.finish()?;
        Ok((values, tooltip))
    }
}

impl EnchantmentRegistry for NativeEnchantmentRegistry {
    type Holder = NativeEnchantmentHolder;
    type Snapshot = NativeEnchantmentSnapshot;
    fn snapshot(&self) -> Self::Snapshot { self.owner.clone() }
    fn item_is_enchantable(&self, item: &ItemStack) -> Result<bool> {
        let fact = self.items.fact(item)?;
        if item.is_empty() { return Ok(false); }
        Ok(match fact.predicate {
            ItemPredicate::Default => item.max_stack_size()? == 1 && item.effective_component_bytes(2).is_some(),
            ItemPredicate::CountOne => item.count == 1,
            ItemPredicate::Never => false,
        })
    }
    fn item_enchantment_value(&self, item: &ItemStack) -> Result<i32> { Ok(self.items.fact(item)?.enchantment_value) }
    fn in_enchanting_table(&self) -> Result<Option<Vec<Self::Holder>>> {
        self.tag_holders("minecraft:in_enchanting_table")
    }
    fn definition(&self, holder: &Self::Holder) -> Result<EnchantmentDefinition> { Ok(self.details(holder)?.table) }
    fn is_primary_item(&self, holder: &Self::Holder, item: &ItemStack) -> Result<bool> {
        let definition = self.details(holder)?;
        let id = self.items.fact(item)?.id;
        Ok(definition.supported_items.contains(&id)
            && definition.primary_items.as_ref().is_none_or(|primary| primary.contains(&id)))
    }
    fn are_compatible(&self, first: &Self::Holder, second: &Self::Holder) -> Result<bool> {
        let a = self.details(first)?; let b = self.details(second)?;
        Ok(first != second && !a.exclusive_set.contains(&second.id) && !b.exclusive_set.contains(&first.id))
    }
    fn protocol_id(&self, holder: &Self::Holder) -> Result<i32> { self.details(holder)?; Ok(holder.id) }
    fn enchant_item(&self, item: &mut ItemStack, enchantment: &EnchantmentInstance<Self::Holder>) -> Result<()> {
        self.details(&enchantment.holder)?;
        let component = crafting_component(item);
        let Some(bytes) = item.effective_component_bytes(component) else { return Ok(()); };
        let (before, tooltip) = self.read_map(bytes)?;
        let before_values = before.iter().copied().collect::<BTreeMap<_, _>>();
        let mut levels = before_values.clone();
        if enchantment.level > 0 {
            let entry = levels.entry(enchantment.holder.id).or_insert(0);
            *entry = (*entry).max(enchantment.level.min(255));
        }
        let order = if levels.len() <= 1 { levels.keys().copied().collect::<Vec<_>>() }
        else {
            let owner = self.map_order.as_ref().context(
                "multi-holder enchantment mutation requires the actual Holder-reference OpenHashMap encounter owner")?;
            ensure!(owner.snapshot() == self.owner, "enchantment reference-map owner changed");
            owner.require_component_owner(item, component)?;
            owner.mutable_upgrade_order(item, component, &before_values,
                &enchantment.holder, enchantment.level, &levels)?
        };
        validate_order(&order, &levels)?;
        let mut fields = Writer::new(); fields.varint(i32::try_from(order.len())?);
        for id in order { fields.varint(id).varint(levels[&id]); }
        fields.bool(tooltip);
        let mut next = item.clone();
        next.components.retain(|entry| entry.type_id != component);
        next.removed_components.retain(|id| *id != component);
        next.components.push(Component { type_id: component, data: fields.into_inner() });
        // Only the original-stream equality certificate may elide a prototype.
        next.normalize_patch();
        next.encode_canonical()?;
        *item = next;
        Ok(())
    }
}

fn crafting_component(item: &ItemStack) -> i32 {
    if !item.is_empty() && inventory::item_name(item.item_id) == Some("enchanted_book") { 23 } else { 9 }
}
fn validate_order(order: &[i32], levels: &BTreeMap<i32, i32>) -> Result<()> {
    let unique = order.iter().copied().collect::<BTreeSet<_>>();
    ensure!(order.len() == levels.len() && unique.len() == order.len()
        && unique == levels.keys().copied().collect(), "actual enchantment map owner returned an invalid permutation");
    Ok(())
}
fn actual_registry_packet(fixtures: &Fixtures) -> Result<&Packet> {
    let mut result = None;
    for packet in fixtures.configuration.iter().filter(|packet| packet.id == 0x07) {
        if Reader::new(&packet.payload).string(32_767)? == ENCHANTMENT_REGISTRY {
            ensure!(result.replace(packet).is_none(), "duplicate actual enchantment registry");
        }
    }
    result.context("actual configured enchantment registry missing")
}
fn unique_packet(fixtures: &Fixtures, id: i32) -> Result<&Packet> {
    let mut packets = fixtures.configuration.iter().filter(|packet| packet.id == id);
    let packet = packets.next().context("actual enchantment configuration packet absent")?;
    ensure!(packets.next().is_none(), "duplicate enchantment configuration packet");
    Ok(packet)
}
fn count(fields: &mut Reader<'_>) -> Result<usize> {
    let value = fields.varint()?;
    ensure!((0..=65_536).contains(&value), "excessive enchantment registry/tag/map collection");
    Ok(value as usize)
}
fn int(data: &Tag, key: &str) -> Result<i32> {
    match data.get(key) { Some(Tag::Int(value)) => Ok(*value), _ => Err(anyhow!("actual enchantment int field {key} absent")) }
}
fn cost(data: &Tag, key: &str) -> Result<EnchantmentCost> {
    let data = data.get(key).with_context(|| format!("enchantment {key} absent"))?;
    Ok(EnchantmentCost { base: int(data, "base")?, per_level_above_first: int(data, "per_level_above_first")? })
}
fn holder_set(data: &Tag, ids: &BTreeMap<String, i32>, tags: &BTreeMap<String, Vec<i32>>) -> Result<Vec<i32>> {
    let resolve = |value: &mc121_rust::nbt::JavaString| -> Result<Vec<i32>> {
        let value = value.as_str().context("invalid holder-set key Unicode")?;
        if let Some(tag) = value.strip_prefix('#') { return tags.get(tag).cloned().context("unresolved actual holder-set tag"); }
        Ok(vec![*ids.get(value).context("unresolved actual holder-set key")?])
    };
    match data {
        Tag::String(value) => resolve(value),
        Tag::List { element_type: 8, values } => {
            let mut out = Vec::new();
            for value in values {
                let Tag::String(value) = value else { return Err(anyhow!("non-string actual holder-set element")); };
                let value = value.as_str().context("invalid holder key Unicode")?;
                ensure!(!value.starts_with('#'), "nested holder-set tag is not a direct holder");
                out.push(*ids.get(value).context("unresolved actual direct holder-set entry")?);
            }
            Ok(out)
        }
        Tag::List { element_type: 0, values } if values.is_empty() => Ok(Vec::new()),
        _ => Err(anyhow!("unsupported actual holder-set codec shape")),
    }
}
type Tags = BTreeMap<String, Vec<i32>>;
fn decode_tags(bytes: &[u8], items: usize, enchantments: usize) -> Result<(Tags, Tags)> {
    let mut fields = Reader::new(bytes); let mut item_tags = None; let mut enchantment_tags = None;
    for _ in 0..count(&mut fields)? {
        let registry = fields.string(32_767)?; let mut tags = BTreeMap::new();
        for _ in 0..count(&mut fields)? {
            let key = fields.string(32_767)?; let mut values = Vec::new();
            for _ in 0..count(&mut fields)? {
                let id = fields.varint()?;
                if matches!(registry.as_str(), ITEM_REGISTRY | ENCHANTMENT_REGISTRY) {
                    let limit = if registry == ITEM_REGISTRY { items } else { enchantments };
                    ensure!(id >= 0 && (id as usize) < limit, "unresolved actual tag holder ID");
                    values.push(id);
                }
            }
            ensure!(tags.insert(key, values).is_none(), "duplicate actual enchantment/item tag key");
        }
        if registry == ITEM_REGISTRY { ensure!(item_tags.replace(tags).is_none(), "duplicate actual item tag registry"); }
        else if registry == ENCHANTMENT_REGISTRY { ensure!(enchantment_tags.replace(tags).is_none(), "duplicate actual enchantment tag registry"); }
    }
    fields.finish()?;
    // Missing tag registries remain absent/empty for actual optional lookups.
    // Referenced definition tags still fail holder_set resolution explicitly.
    Ok((item_tags.unwrap_or_default(), enchantment_tags.unwrap_or_default()))
}
