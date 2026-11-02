//! Actual configured banner-pattern holders/tags for native LoomMenu.
//!
//! The same immutable Fixtures supplies the client registry, tags and features.
//! Tag vectors preserve the original HolderSet encounter order. Template tags
//! were read from the unchanged original BannerPatternItem objects.
use super::*;
use mc121_rust::workstations::loom::{BannerPattern, LoomPatterns};
use std::sync::{Arc, OnceLock};

const BANNER_REGISTRY: &str = "minecraft:banner_pattern";
// Original avp.At is the exact registered ui.loom.take_result object. Its
// registry ID was observed, not copied from a later protocol's sound table.
pub(super) const LOOM_TAKE_SOUND_ID: i32 = 1429;
const DEFAULT_PATTERN_TAG: &str = "minecraft:no_item_required";
const VANILLA_FEATURE: &str = "minecraft:vanilla";
const SOURCE_ITEM_COUNT: i32 = 1333;
const MAX_CONFIGURATION_BYTES: usize = 2 * 1024 * 1024;
const MAX_COLLECTION: i32 = 65_536;

// Original bootstrap csm.getBannerPattern(), not a registry-name convention.
// Entries include the actual item name so the native ID contract is checked.
const TEMPLATE_TAGS: &[(i32, &str, &str)] = &[
    (1195, "flower_banner_pattern", "minecraft:pattern_item/flower"),
    (1196, "creeper_banner_pattern", "minecraft:pattern_item/creeper"),
    (1197, "skull_banner_pattern", "minecraft:pattern_item/skull"),
    (1198, "mojang_banner_pattern", "minecraft:pattern_item/mojang"),
    (1199, "globe_banner_pattern", "minecraft:pattern_item/globe"),
    (1200, "piglin_banner_pattern", "minecraft:pattern_item/piglin"),
    (1201, "flow_banner_pattern", "minecraft:pattern_item/flow"),
    (1202, "guster_banner_pattern", "minecraft:pattern_item/guster"),
];

// Source class inspection, not suffix-based item classification.
const SOURCE_BANNERS: &[i32] = &[
    1133, 1134, 1135, 1136, 1137, 1138, 1139, 1140,
    1141, 1142, 1143, 1144, 1145, 1146, 1147, 1148,
];
const SOURCE_DYES: &[i32] = &[
    944, 945, 946, 947, 948, 949, 950, 951,
    952, 953, 954, 955, 956, 957, 958, 959,
];

/// Source identity of this actual immutable configuration owner. The address
/// is that of Fixtures itself, not a menu-local registry/identity counter.
/// Exact packet bytes also detect a changed registry/tag/feature snapshot.
/// This binding is never persisted or substituted for a world/player binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LoomPatternBinding {
    fixture_owner: usize,
    registry_wire: Arc<[u8]>,
    tags_wire: Arc<[u8]>,
    features_wire: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub(super) struct CapturedBannerPattern {
    pub(super) registry_id: i32,
    pub(super) key: String,
    /// Preserve an actually omitted KnownPack entry as None. No replacement
    /// asset/translation data is fabricated by the native pattern resolver.
    pub(super) data: Option<Tag>,
}

#[derive(Clone, Debug)]
pub(super) struct NativeLoomPatterns {
    binding: LoomPatternBinding,
    registry: Arc<[CapturedBannerPattern]>,
    tags: Arc<BTreeMap<String, Vec<BannerPattern>>>,
    enabled: Arc<BTreeSet<String>>,
}

impl NativeLoomPatterns {
    pub(super) fn new(fixtures: &Fixtures) -> Result<Self> {
        let mut registry_packet = None;
        for packet in fixtures.configuration.iter().filter(|packet| packet.id == 0x07) {
            let mut fields = Reader::new(&packet.payload);
            if fields.string(32_767)? == BANNER_REGISTRY {
                ensure!(registry_packet.replace(packet).is_none(),
                    "loom requires a unique actual banner-pattern registry packet");
            }
        }
        let registry_packet = registry_packet.context("actual loom banner-pattern registry missing")?;
        let tags_packet = unique_packet(fixtures, 0x0d, "tags")?;
        let features_packet = unique_packet(fixtures, 0x0c, "enabled features")?;
        for packet in [registry_packet, tags_packet, features_packet] {
            ensure!(packet.payload.len() <= MAX_CONFIGURATION_BYTES,
                "loom configuration packet exceeds native admission size");
        }
        let enabled = decode_features(&features_packet.payload)?;
        // The item feature certificate below was obtained from this exact
        // unchanged original profile, not from an all-features-enabled guess.
        ensure!(enabled == BTreeSet::from([VANILLA_FEATURE.to_owned()]),
            "loom requires the captured original default enabled-feature profile");
        for &(id, name, _) in TEMPLATE_TAGS {
            ensure!(inventory::item_name(id) == Some(name),
                "loom template item IDs differ from the original class certificate");
        }
        ensure!(inventory::item_name(SOURCE_ITEM_COUNT - 1).is_some()
            && inventory::item_name(SOURCE_ITEM_COUNT).is_none(),
            "loom item registry differs from the captured original item profile");
        let registry = decode_registry(&registry_packet.payload)?;
        // Slot component49 already uses this pinned registry's holder domain.
        // Refuse a different ID owner instead of silently mixing two registries.
        let expected = native_banner_keys();
        ensure!(registry.len() == expected.len()
            && registry.iter().zip(expected).all(|(actual, expected)| actual.key.as_str() == expected.as_str()),
            "actual loom banner-pattern IDs differ from the native Slot registry owner");
        let tags = decode_tags(&tags_packet.payload, registry.len())?;
        Ok(Self {
            binding: LoomPatternBinding {
                fixture_owner: std::ptr::from_ref(fixtures) as usize,
                registry_wire: Arc::from(registry_packet.payload.as_slice()),
                tags_wire: Arc::from(tags_packet.payload.as_slice()),
                features_wire: Arc::from(features_packet.payload.as_slice()),
            },
            registry: Arc::from(registry),
            tags: Arc::new(tags),
            enabled: Arc::new(enabled),
        })
    }

    pub(super) fn binding(&self) -> LoomPatternBinding { self.binding.clone() }
    pub(super) fn enabled_features(&self) -> &BTreeSet<String> { &self.enabled }
    pub(super) fn registry(&self) -> &[CapturedBannerPattern] { &self.registry }

    /// Original lookup returns an empty list for a missing actual pattern tag.
    pub(super) fn patterns_for_tag(&self, tag: &str) -> &[BannerPattern] {
        self.tags.get(tag).map(Vec::as_slice).unwrap_or(&[])
    }

    pub(super) fn template_tag(&self, stack: &ItemStack) -> Option<&'static str> {
        if stack.is_empty() { return None; }
        TEMPLATE_TAGS.iter().find_map(|&(id, _, tag)| (id == stack.item_id).then_some(tag))
    }

    pub(super) fn is_banner_item(&self, stack: &ItemStack) -> bool {
        !stack.is_empty() && SOURCE_BANNERS.contains(&stack.item_id)
    }
    pub(super) fn is_dye_item(&self, stack: &ItemStack) -> bool {
        !stack.is_empty() && SOURCE_DYES.contains(&stack.item_id)
    }

    /// Source ItemStack.isItemEnabled short-circuits empty stacks. A bounded
    /// original bootstrap query covered every registered item: IDs0..1332 all
    /// have requiredFeatures contained in VANILLA_SET. This does not certify
    /// the item's callbacks, attributes, component equality or click overrides.
    pub(super) fn require_item_enabled(&self, stack: &ItemStack) -> Result<()> {
        if stack.is_empty() { return Ok(()); }
        ensure!((0..SOURCE_ITEM_COUNT).contains(&stack.item_id)
            && inventory::item_name(stack.item_id).is_some(),
            "loom item has no original enabled-feature certificate");
        ensure!(self.enabled.contains(VANILLA_FEATURE),
            "loom actual enabled features no longer admit the captured item profile");
        Ok(())
    }
}

fn native_banner_keys() -> &'static [String] {
    static KEYS: OnceLock<Vec<String>> = OnceLock::new();
    KEYS.get_or_init(|| {
        // Reuse the same original capture consumed by inventory's existing
        // registry-holder validator. This is admission evidence, not a fallback
        // registry or a source of selectable-tag ordering.
        let data: serde_json::Value = serde_json::from_str(include_str!("../tools/crafting_metadata.json"))
            .expect("captured original registry metadata");
        let registry = data["registries"][BANNER_REGISTRY].as_object()
            .expect("captured original banner-pattern registry");
        let mut keys = vec![String::new(); registry.len()];
        for (key, id) in registry {
            let id = id.as_u64().expect("captured banner-pattern numeric ID") as usize;
            assert!(id < keys.len() && keys[id].is_empty(), "unique captured banner-pattern ID");
            keys[id] = key.clone();
        }
        assert!(keys.iter().all(|key| !key.is_empty()), "dense captured banner-pattern IDs");
        keys
    })
}

impl LoomPatterns for NativeLoomPatterns {
    fn is_pattern_item(&self, stack: &ItemStack) -> bool {
        self.template_tag(stack).is_some()
    }
    fn selectable_patterns(&self, template: &ItemStack) -> Result<Vec<BannerPattern>> {
        let tag = if template.is_empty() {
            DEFAULT_PATTERN_TAG
        } else if let Some(tag) = self.template_tag(template) {
            tag
        } else {
            return Ok(Vec::new());
        };
        // Do not sort, deduplicate or regenerate this actual HolderSet order.
        Ok(self.patterns_for_tag(tag).to_vec())
    }
}

fn unique_packet<'a>(fixtures: &'a Fixtures, id: i32, label: &str) -> Result<&'a Packet> {
    let mut packets = fixtures.configuration.iter().filter(|packet| packet.id == id);
    let first = packets.next().with_context(|| format!("loom actual {label} packet missing"))?;
    ensure!(packets.next().is_none(), "loom requires unique actual {label} packet");
    Ok(first)
}

fn collection_count(fields: &mut Reader<'_>, label: &str) -> Result<usize> {
    let count = fields.varint()?;
    ensure!((0..=MAX_COLLECTION).contains(&count), "invalid loom {label} count");
    Ok(count as usize)
}

fn location(fields: &mut Reader<'_>) -> Result<String> {
    let value = fields.string(32_767)?;
    let (namespace, path) = value.split_once(':').context("loom resource location has no namespace")?;
    let namespace_char = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit()
        || matches!(byte, b'_' | b'-' | b'.');
    ensure!(!namespace.is_empty() && !path.is_empty()
        && namespace.bytes().all(namespace_char)
        && path.bytes().all(|byte| namespace_char(byte) || byte == b'/'),
        "invalid loom resource location");
    Ok(value)
}

fn decode_features(payload: &[u8]) -> Result<BTreeSet<String>> {
    let mut fields = Reader::new(payload);
    let count = collection_count(&mut fields, "enabled-feature")?;
    let mut flags = BTreeSet::new();
    for _ in 0..count {
        // The actual packet is a set. Its ordering has no pattern semantics.
        flags.insert(location(&mut fields)?);
    }
    fields.finish()?;
    Ok(flags)
}

fn decode_registry(payload: &[u8]) -> Result<Vec<CapturedBannerPattern>> {
    let mut fields = Reader::new(payload);
    ensure!(location(&mut fields)? == BANNER_REGISTRY, "not a loom banner-pattern registry");
    let count = collection_count(&mut fields, "banner-pattern entry")?;
    let mut entries = Vec::with_capacity(count);
    let mut keys = BTreeSet::new();
    for id in 0..count {
        let key = location(&mut fields)?;
        ensure!(keys.insert(key.clone()), "duplicate actual loom banner-pattern registry key");
        let data = if fields.bool()? {
            let (data, used) = Tag::decode_anonymous_prefix(fields.remaining())?;
            fields.bytes(used)?;
            Some(data)
        } else { None };
        entries.push(CapturedBannerPattern { registry_id: i32::try_from(id)?, key, data });
    }
    fields.finish()?;
    Ok(entries)
}

fn decode_tags(payload: &[u8], registry_size: usize) -> Result<BTreeMap<String, Vec<BannerPattern>>> {
    let mut fields = Reader::new(payload);
    let registry_count = collection_count(&mut fields, "tag registry")?;
    let mut actual_tags = None;
    for _ in 0..registry_count {
        let registry = location(&mut fields)?;
        let tag_count = collection_count(&mut fields, "tag")?;
        let mut tags = BTreeMap::new();
        for _ in 0..tag_count {
            let key = location(&mut fields)?;
            let value_count = collection_count(&mut fields, "tag holder")?;
            let mut holders = Vec::new();
            for _ in 0..value_count {
                let id = fields.varint()?;
                ensure!(id >= 0, "negative actual tag registry holder");
                if registry == BANNER_REGISTRY {
                    ensure!((id as usize) < registry_size,
                        "loom tag contains an unregistered banner-pattern holder");
                    holders.push(BannerPattern { registry_id: id });
                }
            }
            if registry == BANNER_REGISTRY {
                ensure!(tags.insert(key, holders).is_none(), "duplicate actual loom banner-pattern tag");
            }
        }
        if registry == BANNER_REGISTRY {
            ensure!(actual_tags.replace(tags).is_none(), "duplicate actual loom tag registry");
        }
    }
    fields.finish()?;
    // A missing registry/tag gives the source's empty optional tag lookup.
    Ok(actual_tags.unwrap_or_default())
}
