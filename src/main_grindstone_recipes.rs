//! Actual configured default enchantment/curse queries for native grindstones.
//!
//! Grindstones do not use a recipe manager or invoke crafted/book callbacks.
//! This module certifies immutable registry data and source observations; the
//! transactional host retains the real Level RNG, inventory and XP-orb owners.
use super::{Fixtures, Packet};
use crate::{main_item_gameplay::MainItemGameplay, main_workstation_components};
use anyhow::{anyhow, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use mc121_rust::{
    container::LocalSlotChange,
    container_workstation_grindstone::GrindstonePhase,
    geometry::Vec3,
    nbt::Tag,
    protocol::Reader,
    workstations::{grindstone::GrindstoneEnchantments, menu::SyncEvent},
};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::{Arc, OnceLock}};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeGrindstoneRegistrySnapshot {
    enchantment_packet: Arc<[u8]>,
    tags_packet: Arc<[u8]>,
    feature_packet: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EnchantmentDefinition {
    key: String,
    minimum_base: i32,
    minimum_per_level: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeGrindstoneEnchantments {
    snapshot: NativeGrindstoneRegistrySnapshot,
    definitions: Arc<[EnchantmentDefinition]>,
    curses: BTreeSet<i32>,
}

impl NativeGrindstoneEnchantments {
    /// Bind the actual configured registry/tag/feature packets to the immutable
    /// original default capture also used for the native item's holder IDs.
    /// Alternate data packs or omitted known-pack definitions are unsupported;
    /// they do not receive a fabricated empty registry or hardcoded curse set.
    pub(super) fn new(fixtures: &Fixtures, gameplay: &MainItemGameplay) -> Result<Self> {
        gameplay.require_supported_floor()?;
        let snapshot = configured_snapshot(&fixtures.configuration)?;
        let pinned = pinned_context()?;
        ensure!(snapshot == pinned.snapshot,
            "grindstone requires the actual pinned default enchantment/tag/feature snapshot");
        let mut context = pinned.clone();
        context.snapshot = snapshot;
        Ok(context)
    }

    pub(super) fn registry_snapshot(&self) -> NativeGrindstoneRegistrySnapshot {
        self.snapshot.clone()
    }

    fn definition(&self, enchantment: i32) -> Result<&EnchantmentDefinition> {
        let index = usize::try_from(enchantment).context("negative enchantment holder ID")?;
        self.definitions.get(index).context("unresolved actual grindstone enchantment holder")
    }
}

impl GrindstoneEnchantments for NativeGrindstoneEnchantments {
    fn is_curse(&self, enchantment: i32) -> Result<bool> {
        self.definition(enchantment)?;
        Ok(self.curses.contains(&enchantment))
    }

    fn minimum_cost(&self, enchantment: i32, level: i32) -> Result<i32> {
        let definition = self.definition(enchantment)?;
        // Enchantment.Cost.calculate uses ordinary Java int subtraction,
        // multiplication and addition. Do not clamp to the declared max level.
        Ok(definition.minimum_base.wrapping_add(
            definition.minimum_per_level.wrapping_mul(level.wrapping_sub(1)),
        ))
    }
}

fn configured_snapshot(packets: &[Packet]) -> Result<NativeGrindstoneRegistrySnapshot> {
    let mut enchantment = None;
    for packet in packets.iter().filter(|packet| packet.id == 0x07) {
        let mut fields = Reader::new(&packet.payload);
        if fields.string(32_767)? == "minecraft:enchantment" {
            ensure!(enchantment.replace(Arc::from(packet.payload.as_slice())).is_none(),
                "duplicate configured enchantment registry");
        }
    }
    let unique = |id| -> Result<Arc<[u8]>> {
        let mut matching = packets.iter().filter(|packet| packet.id == id);
        let packet = matching.next().context("grindstone configuration packet absent")?;
        ensure!(matching.next().is_none(), "duplicate grindstone configuration packet");
        Ok(Arc::from(packet.payload.as_slice()))
    };
    Ok(NativeGrindstoneRegistrySnapshot {
        enchantment_packet: enchantment.context("actual enchantment registry absent")?,
        tags_packet: unique(0x0d)?,
        feature_packet: unique(0x0c)?,
    })
}

fn pinned_context() -> Result<&'static NativeGrindstoneEnchantments> {
    static CONTEXT: OnceLock<std::result::Result<NativeGrindstoneEnchantments, String>> = OnceLock::new();
    CONTEXT.get_or_init(|| load_pinned_context().map_err(|error| format!("{error:#}")))
        .as_ref().map_err(|error| anyhow!("invalid pinned grindstone context: {error}"))
}

#[derive(Deserialize)]
struct CapturedPacket { id: i32, payload: String }
#[derive(Deserialize)]
struct Capture {
    version: String,
    protocol: i32,
    complete: bool,
    packets: Vec<CapturedPacket>,
}

fn load_pinned_context() -> Result<NativeGrindstoneEnchantments> {
    let capture: Capture = serde_json::from_str(include_str!("../fixtures/configuration.json"))?;
    ensure!(capture.version == "1.21" && capture.protocol == 767 && capture.complete,
        "wrong pinned grindstone configuration version");
    let packets = capture.packets.into_iter().map(|packet| Ok(Packet {
        id: packet.id,
        payload: STANDARD.decode(packet.payload)?,
    })).collect::<Result<Vec<_>>>()?;
    let snapshot = configured_snapshot(&packets)?;
    let mut flags = Reader::new(&snapshot.feature_packet);
    ensure!(flags.varint()? == 1 && flags.string(32_767)? == "minecraft:vanilla",
        "unsupported pinned grindstone feature profile");
    flags.finish()?;

    let mut registry = Reader::new(&snapshot.enchantment_packet);
    ensure!(registry.string(32_767)? == "minecraft:enchantment", "wrong grindstone registry");
    let count = bounded_count(&mut registry)?;
    let mut definitions = Vec::with_capacity(count);
    let mut keys = BTreeSet::new();
    for _ in 0..count {
        let key = registry.string(32_767)?;
        ensure!(keys.insert(key.clone()), "duplicate enchantment registry key");
        ensure!(registry.bool()?, "grindstone enchantment definition omitted by known-pack negotiation");
        let (definition, used) = Tag::decode_anonymous_prefix(registry.remaining())?;
        registry.bytes(used)?;
        let cost = definition.get("min_cost").context("enchantment min_cost absent")?;
        definitions.push(EnchantmentDefinition {
            key,
            minimum_base: int_field(cost, "base")?,
            minimum_per_level: int_field(cost, "per_level_above_first")?,
        });
    }
    registry.finish()?;
    ensure!(!definitions.is_empty(), "empty actual enchantment registry");
    let mut tags = Reader::new(&snapshot.tags_packet);
    let mut curses = None;
    for _ in 0..bounded_count(&mut tags)? {
        let registry = tags.string(32_767)?;
        for _ in 0..bounded_count(&mut tags)? {
            let key = tags.string(32_767)?;
            let mut entries = BTreeSet::new();
            for _ in 0..bounded_count(&mut tags)? {
                let id = tags.varint()?;
                if registry == "minecraft:enchantment" && key == "minecraft:curse" {
                    ensure!(id >= 0 && (id as usize) < definitions.len(), "unresolved curse holder ID");
                    entries.insert(id);
                }
            }
            if registry == "minecraft:enchantment" && key == "minecraft:curse" {
                ensure!(curses.replace(entries).is_none(), "duplicate configured curse tag");
            }
        }
    }
    tags.finish()?;
    Ok(NativeGrindstoneEnchantments {
        snapshot,
        definitions: definitions.into(),
        curses: curses.context("actual configured curse tag absent")?,
    })
}

fn bounded_count(fields: &mut Reader<'_>) -> Result<usize> {
    let value = fields.varint()?;
    ensure!((0..=65_536).contains(&value), "excessive grindstone registry/tag count");
    Ok(value as usize)
}

fn int_field(tag: &Tag, name: &str) -> Result<i32> {
    match tag.get(name) {
        Some(Tag::Int(value)) => Ok(*value),
        _ => Err(anyhow!("enchantment cost field {name} is not an actual int")),
    }
}

/// Every actual source listener receives its own phase. Preflight all backing
/// stack-size queries before an infallible transaction replays these callbacks.
pub(super) fn grindstone_preflight_phase(phase: &GrindstonePhase) -> Result<()> {
    main_workstation_components::require_supported_inventory(&phase.player_inventory)?;
    for stack in phase.inputs.iter().chain([&phase.preview, &phase.carried]) {
        main_workstation_components::require_supported_stack(stack)?;
    }
    for slot in (36..45).chain(9..36).chain((5..9).rev()).chain(std::iter::once(45)) {
        let stack = phase.player_inventory.slot(slot).context("grindstone phase inventory slot absent")?;
        if !stack.is_empty() { stack.max_stack_size()?; }
    }
    Ok(())
}

/// Preserve the controller's actual SyncEvent order. The returned observations
/// authorize no packet publication or callback execution by themselves.
pub(super) fn grindstone_preflight_sync(events: &[SyncEvent], phase: &GrindstonePhase) -> Result<Vec<LocalSlotChange>> {
    grindstone_preflight_phase(phase)?;
    let mut changes = Vec::new();
    let mut packet_offset = 0;
    for event in events {
        match event {
            SyncEvent::Packet(_) => packet_offset += 1,
            SyncEvent::DataListener { .. } => anyhow::bail!("grindstone has no menu data listener slots"),
            SyncEvent::SlotListener { menu_slot, player_slot, stack } => {
                main_workstation_components::require_supported_stack(stack)?;
                if let Some(slot) = player_slot {
                    ensure!((9..45).contains(slot) && *menu_slot + 6 == *slot,
                        "grindstone listener lost its canonical player inventory mapping");
                    changes.push(LocalSlotChange {
                        menu_slot: *slot,
                        inventory_slot: Some(if *slot < 36 { *slot } else { *slot - 36 }),
                        stack: stack.clone(), packet_offset,
                    });
                } else {
                    ensure!(*menu_slot < 3, "unknown grindstone backing container");
                }
            }
        }
    }
    Ok(changes)
}

/// Only the actual integer-orb award owner may consume this source position.
/// Do not replace ExperienceOrb.award with a player XP increment or a floating
/// furnace reward. The kernel already owns its one positive-bound Level draw.
pub(super) fn grindstone_experience_position(position: (i32, i32, i32)) -> Vec3 {
    Vec3::new(f64::from(position.0) + 0.5, f64::from(position.1) + 0.5, f64::from(position.2) + 0.5)
}

pub(super) fn grindstone_level_event_packet(position: (i32, i32, i32), event: i32, data: i32) -> Result<Packet> {
    ensure!(event == 1042 && data == 0, "unknown grindstone level event");
    Ok(mc121_rust::item_effects::level_event_packet(event, position, data, false))
}
