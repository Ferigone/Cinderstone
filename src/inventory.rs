//! Java 1.21 inventory wire data and creative inventory state.
//!
//! All 57 protocol-767 component schemas are loaded from the pinned generated
//! data. Component payload bytes are retained; the original-stream equality
//! policy removes proved redundant prototype overrides and absent removals.
//! Typed constructors validate and normalize the proved component subset; raw
//! payloads remain available for identity-sensitive component comparisons.

use std::{collections::BTreeMap, sync::OnceLock};

use crate::nbt::Tag;
use anyhow::{anyhow, bail, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::protocol::{Reader, Writer};

const MAX_SLOT_BYTES: usize = 2 * 1024 * 1024;
const MAX_ARRAY: usize = 65_536;
const MAX_VISITS: usize = 262_144;
const MAX_DEPTH: usize = 128;
pub const INVENTORY_SLOTS: usize = 46;
pub const HOTBAR_START: usize = 36;
pub const OFFHAND_SLOT: usize = 45;

fn catalog() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut data: Value = serde_json::from_str(include_str!("../fixtures/items-1.21.json"))
            .expect("generated protocol-767 item data is valid JSON");
        // The original 1.21 STREAM codecs differ from later Prismarine schemas.
        data["types"]["InstrumentData"] = json!(["container",[
            {"name":"soundEvent","type":"ItemSoundHolder"},
            {"name":"useDuration","type":"varint"},
            {"name":"range","type":"f32"}
        ]]);
        data["types"]["ArmorTrimMaterial"] = json!(["container",[
            {"name":"assetName","type":"string"},
            {"name":"ingredientId","type":"varint"},
            {"name":"itemModelIndex","type":"f32"},
            {"name":"overrideArmorAssets","type":["array",{"countType":"varint","type":["container",[
                {"name":"key","type":"varint"},{"name":"value","type":"string"}
            ]]}]},
            {"name":"description","type":"anonymousNbt"}
        ]]);
        data
    })
}

fn prototype_wire() -> &'static BTreeMap<i32, BTreeMap<i32, Vec<u8>>> {
    static DATA: OnceLock<BTreeMap<i32, BTreeMap<i32, Vec<u8>>>> = OnceLock::new();
    DATA.get_or_init(|| {
        let value: Value =
            serde_json::from_str(include_str!("../fixtures/item-components-1.21.json"))
                .expect("exact Java component prototypes");
        value["prototypes"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(item, components)| {
                (
                    item.parse().unwrap(),
                    components
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(id, data)| {
                            (
                                id.parse().unwrap(),
                                STANDARD
                                    .decode(data.as_str().unwrap())
                                    .expect("Java prototype base64"),
                            )
                        })
                        .collect(),
                )
            })
            .collect()
    })
}

fn prototype_elision_masks() -> &'static BTreeMap<i32, u64> {
    static MASKS: OnceLock<BTreeMap<i32, u64>> = OnceLock::new();
    MASKS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Policy {
            removable_prototypes: Vec<[i32; 2]>,
        }
        let policy: Policy = serde_json::from_str(include_str!(
            "../fixtures/inventory_normalization_policy.json"
        ))
        .expect("original-stream prototype equality policy");
        let mut masks = BTreeMap::new();
        for [item, component] in policy.removable_prototypes {
            assert!((0..57).contains(&component), "known1.21 component");
            *masks.entry(item).or_insert(0) |= 1_u64 << component;
        }
        masks
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub type_id: i32,
    /// Wire payload following the component ID, including nested Slot/NBT data.
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub count: i32,
    pub item_id: i32,
    pub components: Vec<Component>,
    pub removed_components: Vec<i32>,
}

impl ItemStack {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn new(item_id: i32, count: i32) -> Result<Self> {
        if count <= 0 || item_id == 0 {
            return Ok(Self::empty());
        }
        ensure!(
            catalog()["items"].get(item_id.to_string()).is_some(),
            "unknown protocol-767 item ID"
        );
        Ok(Self {
            count,
            item_id,
            components: Vec::new(),
            removed_components: Vec::new(),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.item_id == 0
    }

    /// Return the actual Slot prefix length; a caller may have more fields.
    /// Java's OPTIONAL_STREAM_CODEC treats every count <= 0 as empty.
    pub fn decode(input: &[u8]) -> Result<(Self, usize)> {
        let limited = &input[..input.len().min(MAX_SLOT_BYTES)];
        let mut reader = Reader::new(limited);
        let mut budget = Budget::default();
        let stack = read_stack(&mut reader, &mut budget, 0)?;
        Ok((stack, limited.len() - reader.remaining().len()))
    }

    pub fn decode_exact(input: &[u8]) -> Result<Self> {
        let (stack, used) = Self::decode(input)?;
        ensure!(used == input.len(), "trailing bytes after Slot");
        Ok(stack)
    }

    pub fn encode(&self) -> Vec<u8> {
        let raw = self.encode_patch_raw();
        if self.components.is_empty() && self.removed_components.is_empty() {
            return raw;
        }
        // Direct native constructors historically allowed unchecked payloads.
        // Keep that API; validated stacks use the exact typed encoder below.
        canonical_stack_wire(&raw).unwrap_or(raw)
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>> {
        canonical_stack_wire(&self.encode_patch_raw())
    }

    fn encode_patch_raw(&self) -> Vec<u8> {
        let mut out = Writer::new();
        if self.is_empty() {
            out.varint(0);
            return out.into_inner();
        }
        let prototype = prototype_wire().get(&self.item_id);
        let mask = prototype_elision_masks()
            .get(&self.item_id)
            .copied()
            .unwrap_or(0);
        let components = self
            .components
            .iter()
            .filter(|component| !redundant_prototype_component(prototype, mask, component));
        let removed = self
            .removed_components
            .iter()
            .filter(|id| !absent_prototype_removal(prototype, **id));
        out.varint(self.count)
            .varint(self.item_id)
            .varint(components.clone().count() as i32)
            .varint(removed.clone().count() as i32);
        for component in components {
            out.varint(component.type_id).raw(&component.data);
        }
        for id in removed {
            out.varint(*id);
        }
        out.into_inner()
    }

    /// Apply the prototype normalization proved by the original stream codec.
    /// Component values whose equality depends on object identity need separate
    /// evidence; identical bytes alone are not sufficient to discard them.
    pub fn normalize_patch(&mut self) {
        let prototype = prototype_wire().get(&self.item_id);
        let mask = prototype_elision_masks()
            .get(&self.item_id)
            .copied()
            .unwrap_or(0);
        self.components
            .retain(|component| !redundant_prototype_component(prototype, mask, component));
        self.removed_components
            .retain(|id| !absent_prototype_removal(prototype, *id));
    }

    /// Effective component bytes, including the actual item's Java prototype.
    pub fn effective_component_bytes(&self, type_id: i32) -> Option<&[u8]> {
        if self.is_empty() || self.removed_components.contains(&type_id) {
            return None;
        }
        self.components
            .iter()
            .rev()
            .find(|c| c.type_id == type_id)
            .map(|c| c.data.as_slice())
            .or_else(|| {
                prototype_wire()
                    .get(&self.item_id)?
                    .get(&type_id)
                    .map(Vec::as_slice)
            })
    }
    /// Logical ItemStack.isSameItemSameComponents semantics. Counts are ignored;
    /// default components and map/NBT ordering participate as Java values.
    pub fn same_item_same_components(&self, other: &Self) -> Result<bool> {
        if self.is_empty() || other.is_empty() {
            return Ok(self.is_empty() && other.is_empty());
        }
        if self.item_id != other.item_id {
            return Ok(false);
        }
        for id in 0..57 {
            if self.component_value(id)? != other.component_value(id)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub fn component_value(&self, type_id: i32) -> Result<Option<Value>> {
        let Some(bytes) = self.effective_component_bytes(type_id) else {
            return Ok(None);
        };
        let schema = &catalog()["component_types"][type_id.to_string()]["schema"];
        let mut reader = Reader::new(bytes);
        let mut budget = Budget {
            semantic: true,
            component_id: Some(type_id),
            ..Budget::default()
        };
        let mut value = skip_schema(schema, &mut reader, &Map::new(), &mut budget, 0)?;
        reader.finish()?;
        if matches!(type_id, 9 | 23) {
            if let Some(entries) = value.get_mut("enchantments").and_then(Value::as_array_mut) {
                let mut map = BTreeMap::new();
                for entry in entries.drain(..) {
                    map.insert(entry["id"].as_i64().unwrap(), entry["level"].clone());
                }
                *entries = map
                    .into_iter()
                    .map(|(id, level)| json!({"id":id,"level":level}))
                    .collect();
            }
        }
        if type_id == 35 {
            if let Some(entries) = value
                .get_mut("material")
                .and_then(|holder| holder.get_mut("inline"))
                .and_then(|material| material.get_mut("overrideArmorAssets"))
                .and_then(Value::as_array_mut)
            {
                // Map equality ignores fastutil iteration order. Registry holder
                // object identity affects Java's wire order, not key/value equality.
                let mut map = BTreeMap::new();
                for entry in entries.drain(..) {
                    map.insert(entry["key"].as_i64().unwrap(), entry["value"].clone());
                }
                *entries = map
                    .into_iter()
                    .map(|(key, value)| json!({"key":key,"value":value}))
                    .collect();
            }
        }
        if type_id == 51 {
            if let Some(entries) = value.get_mut("decorations").and_then(Value::as_array_mut) {
                for entry in entries.iter_mut() {
                    if entry
                        .as_i64()
                        .is_some_and(|id| catalog()["items"].get(id.to_string()).is_none())
                    {
                        *entry = json!(0);
                    }
                }
                entries.resize(4, json!(921));
            }
        }
        if type_id == 53 {
            if let Some(entries) = value.get_mut("properties").and_then(Value::as_array_mut) {
                let mut map = BTreeMap::new();
                for entry in entries.drain(..) {
                    map.insert(
                        entry["property"].as_str().unwrap().to_owned(),
                        entry["value"].clone(),
                    );
                }
                *entries = map
                    .into_iter()
                    .map(|(property, value)| json!({"property":property,"value":value}))
                    .collect();
            }
        }
        Ok(Some(value))
    }
    fn semantic_value(&self) -> Result<Value> {
        if self.is_empty() {
            return Ok(Value::Null);
        }
        let mut components = Map::new();
        for id in 0..57 {
            if let Some(value) = self.component_value(id)? {
                components.insert(id.to_string(), value);
            }
        }
        Ok(json!({"id":self.item_id,"count":self.count,"components":components}))
    }
    pub fn max_stack_size(&self) -> Result<i32> {
        if self.is_empty() {
            return Ok(0);
        }
        if self.removed_components.contains(&1) {
            return Ok(1);
        }
        if let Some(component) = self.components.iter().rev().find(|c| c.type_id == 1) {
            let mut fields = Reader::new(&component.data);
            let value = fields.varint()?;
            fields.finish()?;
            return Ok(value);
        }
        catalog()["items"][self.item_id.to_string()]["stack_size"]
            .as_i64()
            .map(|value| value as i32)
            .ok_or_else(|| anyhow!("unknown item prototype"))
    }

    pub fn is_damageable(&self) -> Result<bool> {
        Ok(!self.is_empty()
            && self.effective_component_bytes(2).is_some()
            && self.effective_component_bytes(3).is_some()
            && self.effective_component_bytes(4).is_none())
    }
    pub fn damage_value(&self) -> Result<i32> {
        let damage = self
            .effective_component_bytes(3)
            .map_or(Ok(0), |data| Reader::new(data).varint())?;
        let maximum = self
            .effective_component_bytes(2)
            .map_or(Ok(0), |data| Reader::new(data).varint())?;
        Ok(damage.max(0).min(maximum))
    }
    pub fn is_stackable(&self) -> Result<bool> {
        Ok(self.max_stack_size()? > 1 && (!self.is_damageable()? || self.damage_value()? <= 0))
    }
    pub fn has_max_damage(&self) -> bool {
        if self.is_empty() || self.removed_components.contains(&2) {
            return false;
        }
        self.components.iter().any(|c| c.type_id == 2)
            || catalog()["items"][self.item_id.to_string()]["max_damage"].is_number()
    }

    /// Count checks performed by ItemStack.CODEC in the creative packet's
    /// validatedStreamCodec wrapper. That wrapper uses CODEC, not STRICT_CODEC:
    /// it accepts a damageable item with a max-stack-size component above one.
    /// Component-specific codec bounds and world-dependent block entity data
    /// replacement are separate validation work.
    pub fn validate_creative(&self) -> Result<()> {
        ensure!(
            self.is_empty() || self.count <= 99,
            "item count exceeds codec maximum 99"
        );
        for component in &self.components {
            if !matches!(component.type_id, 29 | 30 | 52) {
                continue;
            }
            let mut reader = Reader::new(&component.data);
            let length = count(reader.varint()?)?;
            for _ in 0..length {
                let (nested, used) = Self::decode(reader.remaining())?;
                nested.validate_creative()?;
                reader.bytes(used)?;
            }
            reader.finish()?;
        }
        Ok(())
    }
}

fn redundant_prototype_component(
    prototype: Option<&BTreeMap<i32, Vec<u8>>>,
    mask: u64,
    component: &Component,
) -> bool {
    // All 8,314 original prototype payloads were decoded by the real stream
    // codec. Four FoodProperties overrides retain nested ItemStack identity.
    if !(0..57).contains(&component.type_id) || mask & (1_u64 << component.type_id) == 0 {
        return false;
    }
    let Some(default) = prototype.and_then(|values| values.get(&component.type_id)) else {
        return false;
    };
    if default == &component.data {
        return true;
    }
    // Original STREAM also proves boxed primitive equality across noncanonical
    // VarInts and nonzero Boolean bytes. Do not generalize this to object graphs.
    fn scalar(id: i32, bytes: &[u8]) -> Result<i32> {
        let mut input = Reader::new(bytes);
        let value = match id {
            1 | 2 | 3 | 8 | 16 | 41 => input.varint()?,
            18 => i32::from(input.bool()?),
            25 => input.i32()?,
            _ => bail!("unproved component equality"),
        };
        input.finish()?;
        Ok(value)
    }
    match (
        scalar(component.type_id, default),
        scalar(component.type_id, &component.data),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}
fn absent_prototype_removal(prototype: Option<&BTreeMap<i32, Vec<u8>>>, id: i32) -> bool {
    (0..57).contains(&id) && prototype.is_some_and(|values| !values.contains_key(&id))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreativeSlotAction {
    Assigned {
        slot: usize,
    },
    /// Caller owns entity spawning and vanilla's creative-drop rate limiter.
    Drop(ItemStack),
    Ignored,
}

#[derive(Clone, Debug)]
pub struct PlayerInventory {
    slots: Vec<ItemStack>,
    selected_hotbar: usize,
    carried: ItemStack,
    state_id: i32,
    // Java Inventory.timesChanged is transient. Existing native/Java restore
    // reconstructs through new(), while detached views preserve its value.
    times_changed: i32,
}

impl Default for PlayerInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerInventory {
    pub fn new() -> Self {
        Self {
            slots: vec![ItemStack::empty(); INVENTORY_SLOTS],
            selected_hotbar: 0,
            carried: ItemStack::empty(),
            state_id: 0,
            times_changed: 0,
        }
    }
    pub fn slots(&self) -> &[ItemStack] {
        &self.slots
    }
    pub fn carried(&self) -> &ItemStack {
        &self.carried
    }
    /// Detached view of the canonical inventory with its actual active menu's
    /// cursor. Slots, selected hotbar and synchronization state are unchanged;
    /// this snapshot neither moves a stack nor creates another live owner.
    pub fn snapshot_with_carried(&self, carried: &ItemStack) -> Self {
        let mut snapshot = self.clone();
        snapshot.set_carried_authoritative(carried.clone());
        snapshot
    }
    pub(crate) fn set_carried_authoritative(&mut self, stack: ItemStack) {
        self.carried = if stack.is_empty() {
            ItemStack::empty()
        } else {
            stack
        };
    }
    pub(crate) fn set_slot_authoritative(&mut self, slot: usize, stack: ItemStack) {
        self.slots[slot] = if stack.is_empty() {
            ItemStack::empty()
        } else {
            stack
        };
    }
    pub(crate) fn increment_state_id(&mut self) -> i32 {
        self.state_id = (self.state_id + 1) & 32767;
        self.state_id
    }
    pub fn state_id(&self) -> i32 {
        self.state_id
    }
    /// Java Inventory.getTimesChanged, distinct from protocol state IDs.
    pub fn get_times_changed(&self) -> i32 {
        self.times_changed
    }
    /// Java Inventory.setChanged increments only at its actual source calls.
    /// Generic slot/cursor setters do not implicitly repeat that callback.
    pub fn set_changed(&mut self) {
        self.times_changed = self.times_changed.wrapping_add(1);
    }
    pub fn selected_hotbar(&self) -> usize {
        self.selected_hotbar
    }
    pub fn slot(&self, slot: usize) -> Option<&ItemStack> {
        self.slots.get(slot)
    }
    /// Oracle replay of a mutation to the same live ItemStack object. Normal
    /// gameplay uses the checked inventory operations above.
    #[cfg(test)]
    pub(crate) fn slot_mut(&mut self, slot: usize) -> Option<&mut ItemStack> {
        self.slots.get_mut(slot)
    }
    pub fn set_selected_hotbar(&mut self, selected: i16) -> Result<()> {
        ensure!(
            (0..9).contains(&selected),
            "selected hotbar slot must be 0..8"
        );
        self.selected_hotbar = selected as usize;
        Ok(())
    }
    pub fn selected_stack(&self, hand: i32) -> Result<&ItemStack> {
        match hand {
            0 => Ok(&self.slots[HOTBAR_START + self.selected_hotbar]),
            1 => Ok(&self.slots[OFFHAND_SLOT]),
            _ => bail!("invalid interaction hand"),
        }
    }
    pub fn set_creative_slot(&mut self, slot: i16, stack: ItemStack) -> Result<CreativeSlotAction> {
        stack.validate_creative()?;
        // The packet is valid, but vanilla ignores assignments/drops whose
        // count exceeds the effective stack limit. It does not disconnect.
        if !stack.is_empty() && stack.count > stack.max_stack_size()? {
            return Ok(CreativeSlotAction::Ignored);
        }
        if slot < 0 {
            return Ok(CreativeSlotAction::Drop(stack));
        }
        // Actual ServerGamePacketListenerImpl: only 1..45 inclusive; slot 0
        // is the crafting result and must not be assigned by this packet.
        if !(1..=45).contains(&slot) {
            return Ok(CreativeSlotAction::Ignored);
        }
        self.slots[slot as usize] = stack;
        self.state_id = (self.state_id + 1) & 32767;
        Ok(CreativeSlotAction::Assigned {
            slot: slot as usize,
        })
    }
    pub fn window_items_payload(&self) -> Vec<u8> {
        let mut out = Writer::new();
        out.u8(0)
            .varint(self.state_id)
            .varint(INVENTORY_SLOTS as i32);
        for slot in &self.slots {
            out.raw(&slot.encode());
        }
        out.raw(&self.carried.encode());
        out.into_inner()
    }
    pub fn set_slot_payload(&self, slot: usize) -> Result<Vec<u8>> {
        let stack = self
            .slots
            .get(slot)
            .ok_or_else(|| anyhow!("inventory slot out of range"))?;
        let mut out = Writer::new();
        out.i8(0)
            .varint(self.state_id)
            .i16(slot as i16)
            .raw(&stack.encode());
        Ok(out.into_inner())
    }
}

/// Official default component value from the exact-version item reports.
pub fn prototype_component(item_id: i32, component_name: &str) -> Option<&'static Value> {
    catalog()["items"][item_id.to_string()]["prototype_components"].get(component_name)
}

pub fn item_default_block_state(item_id: i32) -> Option<u32> {
    catalog()["items"][item_id.to_string()]["block_default_state"]
        .as_u64()
        .map(|v| v as u32)
}
pub fn item_name(item_id: i32) -> Option<&'static str> {
    catalog()["items"][item_id.to_string()]["name"].as_str()
}

/// Change selected properties while retaining the other properties of the
/// supplied state. This is a state-ID operation; block-specific placement
/// rules (stairs/neighbours/slabs/multi-blocks) belong to their own simulation.
pub fn block_state_with_properties(
    state: u32,
    overrides: &BTreeMap<String, String>,
) -> Result<u32> {
    let block = catalog()["blocks"]
        .as_object()
        .expect("generated blocks object")
        .values()
        .find(|block| {
            block["min_state"]
                .as_u64()
                .is_some_and(|v| v <= u64::from(state))
                && block["max_state"]
                    .as_u64()
                    .is_some_and(|v| v >= u64::from(state))
        })
        .ok_or_else(|| anyhow!("unknown block state"))?;
    let props = block["properties"]
        .as_array()
        .expect("generated properties array");
    let min = block["min_state"].as_u64().expect("generated min_state") as u32;
    let mut remaining = state - min;
    let mut values = vec![0_u32; props.len()];
    for (i, property) in props.iter().enumerate().rev() {
        let count = property["num_values"]
            .as_u64()
            .expect("generated property cardinality") as u32;
        values[i] = remaining % count;
        remaining /= count;
    }
    for (name, value) in overrides {
        let (i, property) = props
            .iter()
            .enumerate()
            .find(|(_, p)| p["name"].as_str() == Some(name))
            .ok_or_else(|| anyhow!("block does not have property {name}"))?;
        let index = match property["type"].as_str() {
            Some("bool") => match value.as_str() {
                "true" => 0,
                "false" => 1,
                _ => bail!("invalid boolean block property"),
            },
            Some("int") => {
                if let Some(options) = property["values"].as_array() {
                    options
                        .iter()
                        .position(|v| {
                            v.as_str() == Some(value)
                                || v.as_i64().is_some_and(|n| value.parse::<i64>() == Ok(n))
                        })
                        .ok_or_else(|| anyhow!("invalid integer block property"))?
                        as u32
                } else {
                    value
                        .parse::<u32>()
                        .context("invalid integer block property")?
                }
            }
            Some("enum") => property["values"]
                .as_array()
                .expect("enum values")
                .iter()
                .position(|v| v.as_str() == Some(value))
                .ok_or_else(|| anyhow!("invalid enum block property"))?
                as u32,
            _ => bail!("unsupported generated block property kind"),
        };
        ensure!(
            index
                < property["num_values"]
                    .as_u64()
                    .expect("property cardinality") as u32,
            "block property index out of range"
        );
        values[i] = index;
    }
    let offset = props.iter().zip(values).fold(0_u32, |acc, (p, v)| {
        acc * p["num_values"].as_u64().unwrap() as u32 + v
    });
    Ok(min + offset)
}

/// Exact-version block-state catalog lookup shared with placement/physics.
fn block_state_catalog_entry(state: u32) -> Option<(&'static str, &'static Value)> {
    static INDEX: OnceLock<BTreeMap<u32, (&'static str, &'static Value)>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        catalog()["blocks"]
            .as_object()
            .expect("block catalog")
            .iter()
            .map(|(name, block)| {
                (
                    block["min_state"].as_u64().expect("minimum block state") as u32,
                    (name.as_str(), block),
                )
            })
            .collect()
    });
    let (_, entry) = index.range(..=state).next_back()?;
    (state <= entry.1["max_state"].as_u64()? as u32).then_some(*entry)
}
pub fn block_state_name(state: u32) -> Option<&'static str> {
    block_state_catalog_entry(state).map(|(name, _)| name)
}
pub fn block_default_state(state: u32) -> Option<u32> {
    block_state_catalog_entry(state)?.1["default_state"]
        .as_u64()
        .map(|n| n as u32)
}
pub fn block_state_properties(state: u32) -> Option<BTreeMap<String, String>> {
    let (_, block) = block_state_catalog_entry(state)?;
    let mut offset = state - block["min_state"].as_u64()? as u32;
    let mut values = BTreeMap::new();
    for property in block["properties"].as_array()?.iter().rev() {
        let count = property["num_values"].as_u64()? as u32;
        let value = offset % count;
        offset /= count;
        let text = match property["type"].as_str()? {
            "bool" => {
                if value == 0 {
                    "true".to_owned()
                } else {
                    "false".to_owned()
                }
            }
            "int" | "enum" => match property["values"]
                .as_array()
                .and_then(|a| a.get(value as usize))
            {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => value.to_string(),
            },
            _ => return None,
        };
        values.insert(property["name"].as_str()?.to_owned(), text);
    }
    Some(values)
}
pub fn block_state_property(state: u32, name: &str) -> Option<String> {
    block_state_properties(state)?.remove(name)
}

#[derive(Default)]
struct Budget {
    visits: usize,
    semantic: bool,
    canonical: bool,
    component_id: Option<i32>,
    rewrites: Vec<WireRewrite>,
}
struct WireRewrite {
    before: usize,
    after: usize,
    bytes: Vec<u8>,
}
impl Budget {
    fn rewrite(&mut self, before: usize, after: usize, bytes: Vec<u8>) {
        if self.canonical {
            self.rewrites.push(WireRewrite {
                before,
                after,
                bytes,
            });
        }
    }
}
fn rewritten_payload(raw: &[u8], before: usize, rewrites: &[WireRewrite]) -> Result<Vec<u8>> {
    let after = before - raw.len();
    let mut out = Vec::new();
    let mut cursor = 0;
    for rewrite in rewrites
        .iter()
        .filter(|entry| entry.before <= before && entry.after >= after)
    {
        let start = before - rewrite.before;
        let end = before - rewrite.after;
        ensure!(
            start >= cursor && end <= raw.len(),
            "overlapping item wire normalization"
        );
        out.extend_from_slice(&raw[cursor..start]);
        out.extend_from_slice(&rewrite.bytes);
        cursor = end;
    }
    out.extend_from_slice(&raw[cursor..]);
    Ok(out)
}
fn canonical_stack_wire(raw: &[u8]) -> Result<Vec<u8>> {
    let mut budget = Budget {
        canonical: true,
        ..Budget::default()
    };
    let mut reader = Reader::new(raw);
    read_stack(&mut reader, &mut budget, 0)?;
    reader.finish()?;
    ensure!(budget.rewrites.len() == 1, "missing canonical item wire");
    Ok(budget.rewrites.pop().unwrap().bytes)
}
impl Budget {
    fn visit(&mut self, depth: usize) -> Result<()> {
        self.visits += 1;
        ensure!(
            depth <= MAX_DEPTH && self.visits <= MAX_VISITS,
            "item component nesting/work limit exceeded"
        );
        Ok(())
    }
}
fn count(value: i32) -> Result<usize> {
    ensure!(
        (0..=MAX_ARRAY as i32).contains(&value),
        "invalid or excessive item array length"
    );
    Ok(value as usize)
}

fn read_stack(r: &mut Reader<'_>, budget: &mut Budget, depth: usize) -> Result<ItemStack> {
    budget.visit(depth)?;
    let stack_before = r.remaining().len();
    let rewrite_start = budget.rewrites.len();
    let item_count = r.varint()?;
    if item_count <= 0 {
        budget.rewrite(stack_before, r.remaining().len(), vec![0]);
        return Ok(ItemStack::empty());
    }
    let item_id = r.varint()?;
    ensure!(
        catalog()["items"].get(item_id.to_string()).is_some(),
        "unknown protocol-767 item ID"
    );
    let added = count(r.varint()?)?;
    let removed = count(r.varint()?)?;
    let mut patch: Vec<(i32, Option<Component>)> = Vec::new();
    let mut component_positions = BTreeMap::new();
    for _ in 0..added {
        let type_id = r.varint()?;
        let component = catalog()["component_types"]
            .get(type_id.to_string())
            .ok_or_else(|| anyhow!("unknown protocol-767 component ID {type_id}"))?;
        let start = r.remaining();
        if type_id == 52 {
            ensure!(
                count(Reader::new(start).varint()?)? <= 256,
                "container item list exceeds 256 slots"
            );
        }
        let component_rewrite_start = budget.rewrites.len();
        let previous_id = budget.component_id.replace(type_id);
        if type_id == 51 {
            ensure!(
                count(Reader::new(start).varint()?)? <= 4,
                "pot decorations exceed four slots"
            );
        }
        let value = skip_schema(&component["schema"], r, &Map::new(), budget, depth + 1);
        budget.component_id = previous_id;
        let value = value?;
        validate_component_value(type_id, &value)?;
        let used = start.len() - r.remaining().len();
        if type_id == 51 && budget.canonical {
            let mut input = Reader::new(&start[..used]);
            let length = count(input.varint()?)?;
            let mut out = Writer::new();
            out.varint(4);
            for index in 0..4 {
                let item = if index < length { input.varint()? } else { 921 };
                out.varint(if catalog()["items"].get(item.to_string()).is_some() {
                    item
                } else {
                    0
                });
            }
            budget.rewrites.truncate(component_rewrite_start);
            budget.rewrite(start.len(), r.remaining().len(), out.into_inner());
        }
        component_positions.insert(type_id, start.len());
        let value = Some(Component {
            type_id,
            data: start[..used].to_vec(),
        });
        if let Some((_, current)) = patch.iter_mut().find(|(id, _)| *id == type_id) {
            *current = value;
        } else {
            patch.push((type_id, value));
        }
    }
    for _ in 0..removed {
        let type_id = r.varint()?;
        ensure!(
            catalog()["component_types"]
                .get(type_id.to_string())
                .is_some(),
            "unknown removed component ID"
        );
        if let Some((_, current)) = patch.iter_mut().find(|(id, _)| *id == type_id) {
            *current = None;
        } else {
            patch.push((type_id, None));
        }
    }
    if item_id == 0 {
        budget.rewrites.truncate(rewrite_start);
        budget.rewrite(stack_before, r.remaining().len(), vec![0]);
        return Ok(ItemStack::empty());
    }
    let mut components = Vec::new();
    let mut removed_components = Vec::new();
    for (id, value) in patch {
        if let Some(value) = value {
            components.push(value);
        } else {
            removed_components.push(id);
        }
    }
    let mut stack = ItemStack {
        count: item_count,
        item_id,
        components,
        removed_components,
    };
    stack.normalize_patch();
    if budget.canonical {
        let mut out = Writer::new();
        out.varint(stack.count)
            .varint(stack.item_id)
            .varint(stack.components.len() as i32)
            .varint(stack.removed_components.len() as i32);
        for component in &stack.components {
            let payload = rewritten_payload(
                &component.data,
                component_positions[&component.type_id],
                &budget.rewrites[rewrite_start..],
            )?;
            out.varint(component.type_id).raw(&payload);
        }
        for id in &stack.removed_components {
            out.varint(*id);
        }
        budget.rewrites.truncate(rewrite_start);
        budget.rewrite(stack_before, r.remaining().len(), out.into_inner());
    }
    Ok(stack)
}

fn validate_component_value(type_id: i32, value: &Value) -> Result<()> {
    if type_id == 35 {
        if let Some(entries) = value["material"]["inline"]["overrideArmorAssets"].as_array() {
            for entry in entries {
                ensure!(
                    (0..8).contains(&entry["key"].as_i64().unwrap_or(-1)),
                    "unknown armor material registry holder"
                );
            }
        }
    }
    Ok(())
}

fn validate_text_component(tag: &Tag) -> Result<()> {
    match tag {
        Tag::String(_) => {}
        Tag::List { values, .. } => {
            ensure!(!values.is_empty(), "component list must have contents");
            for value in values {
                validate_text_component(value)?;
            }
        }
        Tag::Compound(_) => {
            let kind = match tag.get("type") {
                Some(Tag::String(value)) => Some(
                    value
                        .as_str()
                        .ok_or_else(|| anyhow!("invalid component type"))?,
                ),
                Some(_) => bail!("invalid component type"),
                None => None,
            };
            let fields = [
                ("text", "text"),
                ("translatable", "translate"),
                ("keybind", "keybind"),
                ("score", "score"),
                ("selector", "selector"),
                ("nbt", "nbt"),
            ];
            let field = fields
                .iter()
                .find(|(name, field)| match kind {
                    Some(kind) => kind == *name,
                    None => tag.get(field).is_some(),
                })
                .map(|(_, field)| *field)
                .ok_or_else(|| anyhow!("unknown component contents"))?;
            if field == "score" {
                ensure!(
                    matches!(tag.get(field), Some(Tag::Compound(_))),
                    "score contents must be a map"
                );
            } else {
                ensure!(
                    matches!(tag.get(field), Some(Tag::String(_))),
                    "component contents must be a string"
                );
            }
            if let Some(extra) = tag.get("extra") {
                let Tag::List { values, .. } = extra else {
                    bail!("component extra must be a list")
                };
                ensure!(!values.is_empty(), "component extra must have contents");
                for child in values {
                    validate_text_component(child)?;
                }
            }
        }
        _ => bail!("invalid component representation"),
    }
    Ok(())
}

fn component_nbt_wire(type_id: Option<i32>, tag: &Tag, raw: &[u8]) -> Result<Vec<u8>> {
    if matches!(type_id, Some(5 | 6 | 7 | 34 | 35 | 42)) {
        validate_text_component(tag)?;
        return crate::metadata::normalize_literal_component_payload(raw);
    }
    if matches!(
        type_id,
        Some(0 | 10 | 11 | 19 | 22 | 27 | 36 | 37 | 38 | 39 | 54 | 56)
    ) {
        ensure!(
            matches!(tag, Tag::Compound(_)),
            "expected component compound tag"
        );
    }
    match type_id {
        Some(19) => return Tag::Compound(Vec::new()).encode_network_anonymous(),
        Some(27) => {
            if let Tag::Compound(entries) = tag {
                for (_, value) in entries {
                    ensure!(
                        matches!(value, Tag::Compound(_)),
                        "map decoration must be a map"
                    );
                    ensure!(
                        matches!(value.get("type"), Some(Tag::String(_))),
                        "map decoration type is missing"
                    );
                    for coordinate in ["x", "z", "rot"] {
                        ensure!(
                            matches!(
                                value.get(coordinate),
                                Some(
                                    Tag::Byte(_)
                                        | Tag::Short(_)
                                        | Tag::Int(_)
                                        | Tag::Long(_)
                                        | Tag::Float(_)
                                        | Tag::Double(_)
                                )
                            ),
                            "map decoration coordinate is missing"
                        );
                    }
                }
            }
        }
        Some(36) => {
            if let Tag::Compound(entries) = tag {
                for (name, value) in entries {
                    let key = name
                        .as_str()
                        .ok_or_else(|| anyhow!("invalid block registry name"))?;
                    let key = key.strip_prefix("minecraft:").unwrap_or(key);
                    let block = catalog()["blocks"]
                        .get(key)
                        .ok_or_else(|| anyhow!("unknown block registry key"))?;
                    let Tag::String(property) = value else {
                        bail!("debug stick property must be a string")
                    };
                    ensure!(
                        block["properties"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|entry| property.as_str() == entry["name"].as_str()),
                        "unknown block property"
                    );
                }
            }
        }
        Some(43) => {
            let Tag::List { values, .. } = tag else {
                bail!("recipe component must be a list")
            };
            ensure!(
                values.iter().all(|value| matches!(value, Tag::String(_))),
                "recipe key must be a string"
            );
        }
        Some(55) => ensure!(matches!(tag, Tag::String(_)), "lock must be a string"),
        Some(56) => ensure!(
            matches!(tag.get("loot_table"), Some(Tag::String(_))),
            "loot table key is missing"
        ),
        _ => {}
    }
    tag.encode_network_anonymous()
}

fn captured_registry_size(name: &str) -> usize {
    static REGISTRIES: OnceLock<Value> = OnceLock::new();
    let data = REGISTRIES.get_or_init(|| {
        serde_json::from_str(include_str!("../tools/crafting_metadata.json"))
            .expect("captured registry IDs")
    });
    data["registries"][name]
        .as_object()
        .expect("known captured registry")
        .len()
}
fn default_block_tag_exists(name: &str) -> bool {
    static TAGS: OnceLock<Value> = OnceLock::new();
    let tags = TAGS.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/placement-block-tags-1.21.json"))
            .expect("original block tags")
    });
    let key = if name.contains(':') {
        name.to_owned()
    } else {
        format!("minecraft:{name}")
    };
    tags["tags"].get(key).is_some()
}

fn skip_schema(
    schema: &Value,
    r: &mut Reader<'_>,
    environment: &Map<String, Value>,
    budget: &mut Budget,
    depth: usize,
) -> Result<Value> {
    budget.visit(depth)?;
    if let Some(name) = schema.as_str() {
        return match name {
            "varint" => Ok(json!(r.varint()?)),
            "i8" => Ok(json!(r.i8()?)),
            "u8" => Ok(json!(r.u8()?)),
            "i16" => Ok(json!(r.i16()?)),
            "u16" => Ok(json!(r.u16()?)),
            "i32" => Ok(json!(r.i32()?)),
            "i64" => Ok(json!(r.i64()?)),
            "f32" => {
                let bits = r.i32()? as u32;
                Ok(json!(if f32::from_bits(bits).is_nan() {
                    0x7fc00000
                } else {
                    bits
                }))
            }
            "f64" => {
                let bits = r.i64()? as u64;
                Ok(json!(if f64::from_bits(bits).is_nan() {
                    0x7ff8000000000000
                } else {
                    bits
                }))
            }
            "bool" => Ok(json!(r.bool()?)),
            "string" => Ok(json!(r.string(32767)?)),
            "void" => Ok(Value::Null),
            "UUID" => Ok(json!(r.bytes(16)?)),
            "position" => Ok(json!(r.i64()?)),
            "Slot" => {
                let stack = read_stack(r, budget, depth + 1)?;
                if budget.semantic {
                    stack.semantic_value()
                } else {
                    Ok(Value::Null)
                }
            }
            "NonEmptySlot" => {
                let stack = read_stack(r, budget, depth + 1)?;
                ensure!(!stack.is_empty(), "empty item in required Slot");
                if budget.semantic {
                    stack.semantic_value()
                } else {
                    Ok(Value::Null)
                }
            }
            "anonymousNbt" | "anonOptionalNbt" => {
                let start = r.remaining();
                let tag = r.u8()?;
                skip_nbt(tag, r, 0)?;
                let raw = &start[..start.len() - r.remaining().len()];
                if tag == 0 {
                    ensure!(
                        name == "anonOptionalNbt" && budget.component_id != Some(7),
                        "expected non-null component NBT"
                    );
                    Ok(Value::Null)
                } else {
                    let tag = Tag::decode_anonymous(raw)?;
                    let wire = component_nbt_wire(budget.component_id, &tag, raw)?;
                    let semantic = Tag::decode_anonymous(&wire)?;
                    budget.rewrite(start.len(), r.remaining().len(), wire);
                    Ok(nbt_value(&semantic))
                }
            }
            "ItemEffectDetail" if matches!(budget.component_id, Some(20 | 31)) => {
                let before = r.remaining().len();
                let mark = budget.rewrites.len();
                let amplifier = r.varint()?.clamp(0, 255);
                let duration = r.varint()?;
                let ambient = r.bool()?;
                let particles = r.bool()?;
                let icon = r.bool()?;
                let hidden = if r.bool()? {
                    skip_schema(
                        &json!("ItemEffectDetail"),
                        r,
                        environment,
                        budget,
                        depth + 1,
                    )?
                } else {
                    Value::Null
                };
                let mut out = Writer::new();
                out.varint(amplifier)
                    .varint(duration)
                    .bool(ambient)
                    .bool(particles)
                    .bool(icon)
                    .bool(false);
                budget.rewrites.truncate(mark);
                budget.rewrite(before, r.remaining().len(), out.into_inner());
                Ok(
                    json!({"amplifier":amplifier,"duration":duration,"ambient":ambient,
                    "showParticles":particles,"showIcon":icon,"hiddenEffect":hidden}),
                )
            }
            _ => {
                let next = catalog()["types"]
                    .get(name)
                    .ok_or_else(|| anyhow!("unknown item schema type {name}"))?;
                ensure!(
                    next.as_str() != Some("native"),
                    "unsupported native item schema {name}"
                );
                skip_schema(next, r, environment, budget, depth + 1)
            }
        };
    }
    let tuple = schema
        .as_array()
        .ok_or_else(|| anyhow!("invalid generated item schema"))?;
    let kind = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("invalid schema constructor"))?;
    let opts = tuple.get(1).unwrap_or(&Value::Null);
    match kind {
        "container" => {
            let mut local = environment.clone();
            for field in opts
                .as_array()
                .ok_or_else(|| anyhow!("container fields missing"))?
            {
                let value = skip_schema(&field["type"], r, &local, budget, depth + 1)?;
                if let Some(name) = field["name"].as_str() {
                    local.insert(name.into(), value);
                } else if let Some(values) = value.as_object() {
                    local.extend(values.clone());
                }
            }
            Ok(Value::Object(local))
        }
        "switch" => {
            let key = opts["compareTo"]
                .as_str()
                .ok_or_else(|| anyhow!("switch field missing"))?;
            let value = environment
                .get(key)
                .ok_or_else(|| anyhow!("switch context {key} missing"))?;
            let key = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            let branch = opts["fields"]
                .get(&key)
                .or_else(|| opts.get("default"))
                .ok_or_else(|| anyhow!("unknown switch branch {key}"))?;
            skip_schema(branch, r, environment, budget, depth + 1)
        }
        "mapper" => {
            let value = skip_schema(&opts["type"], r, environment, budget, depth + 1)?;
            let key = value.to_string();
            opts["mappings"]
                .get(key)
                .cloned()
                .ok_or_else(|| anyhow!("unrecognized item enum value"))
        }
        "array" => {
            let length = if let Some(ty) = opts.get("countType") {
                skip_schema(ty, r, environment, budget, depth + 1)?
                    .as_i64()
                    .ok_or_else(|| anyhow!("non-integer array length"))?
            } else if let Some(name) = opts["count"].as_str() {
                environment
                    .get(name)
                    .and_then(Value::as_i64)
                    .ok_or_else(|| anyhow!("array count field missing"))?
            } else {
                opts["count"]
                    .as_i64()
                    .ok_or_else(|| anyhow!("array count missing"))?
            };
            ensure!(
                (0..=MAX_ARRAY as i64).contains(&length),
                "invalid item array length"
            );
            let mut values = Vec::with_capacity(length as usize);
            for _ in 0..length {
                values.push(skip_schema(
                    &opts["type"],
                    r,
                    environment,
                    budget,
                    depth + 1,
                )?);
            }
            Ok(Value::Array(values))
        }
        "option" => {
            if r.bool()? {
                skip_schema(opts, r, environment, budget, depth + 1)
            } else {
                Ok(Value::Null)
            }
        }
        "registryEntryHolder" => {
            let id = r.varint()?;
            ensure!(id >= 0, "negative registry holder");
            if id == 0 {
                Ok(
                    json!({"inline":skip_schema(&opts["otherwise"]["type"],r,environment,budget,depth+1)?}),
                )
            } else {
                let registry = match opts["baseName"].as_str() {
                    Some("materialId") => Some("minecraft:trim_material"),
                    Some("patternId") if opts["otherwise"]["type"] == "ArmorTrimPattern" => {
                        Some("minecraft:trim_pattern")
                    }
                    Some("patternId") => Some("minecraft:banner_pattern"),
                    Some("songId") => Some("minecraft:jukebox_song"),
                    _ => None,
                };
                if let Some(name) = registry {
                    ensure!(
                        ((id - 1) as usize) < captured_registry_size(name),
                        "unknown registry holder"
                    );
                }
                if opts["baseName"] == "instrumentId" {
                    ensure!(id <= 8, "unknown instrument registry holder");
                }
                Ok(json!({"id":id-1}))
            }
        }
        "registryEntryHolderSet" => {
            let size = r.varint()?;
            ensure!(size >= 0, "negative registry holder set");
            if size == 0 {
                let name = skip_schema(&opts["base"]["type"], r, environment, budget, depth + 1)?;
                if opts["otherwise"]["name"] == "blockIds" || budget.component_id == Some(22) {
                    ensure!(
                        name.as_str().is_some_and(default_block_tag_exists),
                        "unknown block tag"
                    );
                }
                Ok(json!({"tag":name}))
            } else {
                let mut values = Vec::new();
                for _ in 0..count(size - 1)? {
                    values.push(skip_schema(
                        &opts["otherwise"]["type"],
                        r,
                        environment,
                        budget,
                        depth + 1,
                    )?);
                }
                Ok(json!({"values":values}))
            }
        }
        "pstring" => Ok(json!(r.string(32767)?)),
        "bitfield" => {
            let bits = opts
                .as_array()
                .ok_or_else(|| anyhow!("bitfield layout missing"))?
                .iter()
                .map(|field| field["size"].as_u64().unwrap_or(0))
                .sum::<u64>();
            ensure!(
                bits > 0 && bits <= 128 && bits % 8 == 0,
                "unsupported item bitfield width"
            );
            Ok(json!(r.bytes((bits / 8) as usize)?))
        }
        _ => bail!("unsupported item schema constructor {kind}"),
    }
}

fn nbt_value(tag: &Tag) -> Value {
    let data = match tag {
        Tag::Byte(v) => json!(v),
        Tag::Short(v) => json!(v),
        Tag::Int(v) => json!(v),
        Tag::Long(v) => json!(v),
        Tag::Float(v) => json!(if *v == 0.0 {
            0
        } else if v.is_nan() {
            0x7fc00000
        } else {
            v.to_bits()
        }),
        Tag::Double(v) => json!(if *v == 0.0 {
            0
        } else if v.is_nan() {
            0x7ff8000000000000
        } else {
            v.to_bits()
        }),
        Tag::ByteArray(v) => json!(v),
        Tag::IntArray(v) => json!(v),
        Tag::LongArray(v) => json!(v),
        Tag::String(v) => json!(v.iter_units().collect::<Vec<_>>()),
        Tag::List { values, .. } => Value::Array(values.iter().map(nbt_value).collect()),
        Tag::Compound(entries) => {
            let mut map = BTreeMap::new();
            for (key, value) in entries {
                map.insert(key.iter_units().collect::<Vec<_>>(), nbt_value(value));
            }
            Value::Array(
                map.into_iter()
                    .map(|(key, value)| json!([key, value]))
                    .collect(),
            )
        }
    };
    json!([tag.tag_id(), data])
}

// Prefix-only NBT scanner. It retains raw bytes; it does not reinterpret or
// normalize Modified UTF-8 strings. Full semantic NBT belongs to nbt.rs.
fn skip_nbt(tag: u8, r: &mut Reader<'_>, depth: usize) -> Result<()> {
    ensure!(depth <= 512, "NBT nesting exceeds 512");
    match tag {
        0 => {}
        1 => {
            r.bytes(1)?;
        }
        2 => {
            r.bytes(2)?;
        }
        3 | 5 => {
            r.bytes(4)?;
        }
        4 | 6 => {
            r.bytes(8)?;
        }
        7 => {
            let n = count(r.i32()?)?;
            r.bytes(n)?;
        }
        8 => {
            let n = r.u16()? as usize;
            r.bytes(n)?;
        }
        9 => {
            let kind = r.u8()?;
            let n = count(r.i32()?)?;
            ensure!(kind != 0 || n == 0, "nonempty TAG_End list");
            for _ in 0..n {
                skip_nbt(kind, r, depth + 1)?;
            }
        }
        10 => loop {
            let kind = r.u8()?;
            if kind == 0 {
                break;
            }
            let length = r.u16()? as usize;
            r.bytes(length)?;
            skip_nbt(kind, r, depth + 1)?;
        },
        11 => {
            let n = count(r.i32()?)?;
            r.bytes(
                n.checked_mul(4)
                    .ok_or_else(|| anyhow!("NBT array overflow"))?,
            )?;
        }
        12 => {
            let n = count(r.i32()?)?;
            r.bytes(
                n.checked_mul(8)
                    .ok_or_else(|| anyhow!("NBT array overflow"))?,
            )?;
        }
        _ => bail!("unknown NBT tag {tag}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;

    #[test]
    fn actual_java_nested_required_slots_and_container256_limit() {
        let data: Value = serde_json::from_str(include_str!(
            "../tools/container_component_wire_vectors.json"
        ))
        .unwrap();
        for case in data["cases"].as_array().unwrap() {
            let bytes = STANDARD.decode(case["bytes"].as_str().unwrap()).unwrap();
            assert_eq!(
                ItemStack::decode_exact(&bytes).is_ok(),
                case["accepted"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
        }
        assert_eq!(data["cases"].as_array().unwrap().len(), 16);
    }
    #[test]
    fn actual_java_default_component_payloads_decode_exactly() {
        let mut errors = Vec::new();
        let mut checked = 0;
        for (item, components) in prototype_wire() {
            if *item == 0 {
                continue;
            }
            for (id, data) in components {
                let stack = ItemStack {
                    count: 1,
                    item_id: *item,
                    components: vec![Component {
                        type_id: *id,
                        data: data.clone(),
                    }],
                    removed_components: vec![],
                };
                if let Err(error) = ItemStack::decode_exact(&stack.encode()) {
                    errors.push(format!("{item}/{id}: {error:#}"));
                }
                checked += 1;
            }
        }
        assert!(errors.is_empty(), "{}errors: {:?}", errors.len(), errors);
        assert!(checked > 8000);
    }
    #[test]
    fn actual_java_stream_prototype_and_removal_normalization() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../fixtures/inventory_normalization_vectors.json"
        ))
        .unwrap();
        let mut groups = BTreeMap::<String, Value>::new();
        let mut gaps = Vec::new();
        let mut strict_cases = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let input = STANDARD
                .decode(case["query"]["bytes"].as_str().unwrap())
                .unwrap();
            let group = case["group"].as_str().unwrap();
            let original = &case["expected"];
            let observed = match ItemStack::decode(&input) {
                Ok((stack, consumed)) => json!({"accepted":true,"consumed":consumed,
                    "remaining":input.len()-consumed,"canonical":STANDARD.encode(stack.encode())}),
                Err(_) => json!({"accepted":false}),
            };
            let matches = observed["accepted"] == original["accepted"]
                && (!observed["accepted"].as_bool().unwrap()
                    || (observed["consumed"] == original["consumed"]
                        && observed["remaining"] == original["remaining"]
                        && observed["canonical"] == original["canonical"]));
            let counts = groups
                .entry(group.into())
                .or_insert(json!({"cases":0,"matches":0,"mismatches":0}));
            counts["cases"] = json!(counts["cases"].as_u64().unwrap() + 1);
            let key = if matches { "matches" } else { "mismatches" };
            counts[key] = json!(counts[key].as_u64().unwrap() + 1);
            if matches!(group, "prototype" | "removal" | "ordering" | "smoke") {
                assert!(
                    matches,
                    "{} expected {}, observed {}",
                    case["name"], original, observed
                );
                strict_cases += 1;
            } else if !matches {
                gaps.push(json!({"name":case["name"],"group":group,
                    "expected":original,"observed":observed}));
            }
        }
        assert_eq!(strict_cases, 8840);
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 9145);
        assert!(gaps.is_empty(), "original stream differences: {gaps:?}");
        println!(
            "INVENTORY_NORMALIZATION_REPORT {}",
            json!({"groups":groups,"strict_cases":strict_cases,"gaps":gaps})
        );
    }

    #[test]
    fn independent_node_vectors_match_original_java_acceptance_and_canonical_wire() {
        let data: Value =
            serde_json::from_str(include_str!("../tools/inventory_vectors.json")).unwrap();
        let original: Value = serde_json::from_str(include_str!(
            "../fixtures/inventory_normalization_vectors.json"
        ))
        .unwrap();
        assert_eq!(data["component_count"], 57);
        let mut exact_accepted = 0;
        let mut prefix_only = 0;
        let mut rejected = 0;
        for vector in data["vectors"].as_array().unwrap() {
            let bytes = STANDARD
                .decode(vector["slot_base64"].as_str().unwrap())
                .unwrap();
            let golden = &original["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["group"] == "independent117" && case["name"] == vector["name"])
                .unwrap()["expected"];
            if !golden["accepted"].as_bool().unwrap() {
                assert!(ItemStack::decode(&bytes).is_err(), "{}", vector["name"]);
                rejected += 1;
                continue;
            }
            let (stack, consumed) = ItemStack::decode(&bytes)
                .unwrap_or_else(|error| panic!("{}: {error:#}", vector["name"]));
            assert_eq!(
                consumed as u64,
                golden["consumed"].as_u64().unwrap(),
                "{}",
                vector["name"]
            );
            assert_eq!(
                stack.encode_canonical().unwrap(),
                STANDARD
                    .decode(golden["canonical"].as_str().unwrap())
                    .unwrap(),
                "{}",
                vector["name"]
            );
            if golden["remaining"] == 0 {
                assert!(
                    ItemStack::decode_exact(&bytes).is_ok(),
                    "{}",
                    vector["name"]
                );
                exact_accepted += 1;
            } else {
                assert!(
                    ItemStack::decode_exact(&bytes).is_err(),
                    "{}",
                    vector["name"]
                );
                prefix_only += 1;
            }
        }
        assert_eq!((exact_accepted, prefix_only, rejected), (93, 1, 23));
    }

    #[test]
    fn actual_java_stream_scalar_semantic_prototype_equality() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../fixtures/inventory_normalization_scalar_vectors.json"
        ))
        .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 46);
        for case in cases {
            let bytes = STANDARD
                .decode(case["query"]["bytes"].as_str().unwrap())
                .unwrap();
            let expected = &case["expected"];
            assert_eq!(expected["prototype_equals_decoded"], true);
            assert_eq!(expected["redundant_removed"], true);
            let (stack, consumed) = ItemStack::decode(&bytes).unwrap();
            assert_eq!(
                consumed as u64,
                expected["consumed"].as_u64().unwrap(),
                "{}",
                case["name"]
            );
            assert!(stack.components.is_empty(), "{}", case["name"]);
            assert_eq!(
                stack.encode_canonical().unwrap(),
                STANDARD
                    .decode(expected["canonical"].as_str().unwrap())
                    .unwrap(),
                "{}",
                case["name"]
            );
        }
    }

    #[test]
    fn actual_java_trim_map_identity_hash_changes_wire_but_preserves_value() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../fixtures/inventory_normalization_map_vectors.json"
        ))
        .unwrap();
        let mut checked = 0;
        let mut orders = BTreeMap::<String, std::collections::BTreeSet<String>>::new();
        for run in fixture["runs"].as_array().unwrap() {
            for case in run["cases"].as_array().unwrap() {
                let expected = &case["expected"];
                for key in ["trim_equals", "material_equals", "map_equals"] {
                    assert_eq!(expected[key], true);
                }
                let left = ItemStack::decode_exact(
                    &STANDARD.decode(case["input"].as_str().unwrap()).unwrap(),
                )
                .unwrap();
                let right = ItemStack::decode_exact(
                    &STANDARD
                        .decode(case["reference"].as_str().unwrap())
                        .unwrap(),
                )
                .unwrap();
                assert!(
                    left.same_item_same_components(&right).unwrap(),
                    "{}",
                    case["name"]
                );
                for key in ["input_roundtrip", "reference_roundtrip"] {
                    assert_eq!(expected[key]["accepted"], true);
                    assert_eq!(expected[key]["remaining"], 0);
                    let canonical = expected[key]["canonical"].as_str().unwrap();
                    orders
                        .entry(case["name"].as_str().unwrap().into())
                        .or_default()
                        .insert(canonical.into());
                    let decoded =
                        ItemStack::decode_exact(&STANDARD.decode(canonical).unwrap()).unwrap();
                    assert!(left.same_item_same_components(&decoded).unwrap());
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 21);
        assert_eq!(orders.len(), 7);
        assert!(orders.values().all(|orders| orders.len() >= 2));
    }

    fn only_trim_override_iteration_order_differs(left: &[u8], right: &[u8]) -> bool {
        let left = ItemStack::decode_exact(left).unwrap();
        let right = ItemStack::decode_exact(right).unwrap();
        if left.count != right.count
            || left.item_id != right.item_id
            || left.removed_components != right.removed_components
            || left.components.len() != right.components.len()
        {
            return false;
        }
        let mut found = false;
        for (a, b) in left.components.iter().zip(&right.components) {
            if a.type_id != b.type_id {
                return false;
            }
            if a.data == b.data {
                continue;
            }
            if a.type_id != 35
                || left.component_value(35).unwrap() != right.component_value(35).unwrap()
            {
                return false;
            }
            found = true;
        }
        found
    }

    #[test]
    fn actual_java_stream_component_constructors_and_nested_canonical_wire() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../fixtures/inventory_normalization_constructor_vectors.json"
        ))
        .unwrap();
        let mut groups = BTreeMap::<String, Value>::new();
        let mut gaps = Vec::new();
        let mut semantic_map_matches = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let input = STANDARD
                .decode(case["query"]["bytes"].as_str().unwrap())
                .unwrap();
            let original = &case["expected"];
            let observed = match ItemStack::decode(&input) {
                Ok((stack, consumed)) => match stack.encode_canonical() {
                    Ok(wire) => json!({"accepted":true,"consumed":consumed,
                        "remaining":input.len()-consumed,"canonical":STANDARD.encode(wire)}),
                    Err(error) => json!({"accepted":true,"encode_error":format!("{error:#}")}),
                },
                Err(_) => json!({"accepted":false}),
            };
            let matches = observed["accepted"] == original["accepted"]
                && (!observed["accepted"].as_bool().unwrap()
                    || (observed["consumed"] == original["consumed"]
                        && observed["remaining"] == original["remaining"]
                        && observed["canonical"] == original["canonical"]));
            let group = case["group"].as_str().unwrap();
            let counts = groups
                .entry(group.into())
                .or_insert(json!({"cases":0,"matches":0,"mismatches":0}));
            counts["cases"] = json!(counts["cases"].as_u64().unwrap() + 1);
            let key = if matches { "matches" } else { "mismatches" };
            counts[key] = json!(counts[key].as_u64().unwrap() + 1);
            if !matches {
                let semantic_map_match = original["accepted"] == true
                    && observed["accepted"] == true
                    && original["consumed"] == observed["consumed"]
                    && original["remaining"] == observed["remaining"]
                    && only_trim_override_iteration_order_differs(
                        &STANDARD
                            .decode(observed["canonical"].as_str().unwrap())
                            .unwrap(),
                        &STANDARD
                            .decode(original["canonical"].as_str().unwrap())
                            .unwrap(),
                    );
                assert!(
                    semantic_map_match,
                    "{} expected {}, observed {}",
                    case["name"], original, observed
                );
                semantic_map_matches += 1;
                gaps.push(json!({"name":case["name"],"group":group,"expected":original,"observed":observed,
                    "semantic_map_match":semantic_map_match}));
            }
        }
        println!(
            "INVENTORY_CONSTRUCTOR_REPORT {}",
            json!({"groups":groups,"gaps":gaps,"semantic_map_matches":semantic_map_matches})
        );
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 452);
        assert_eq!(semantic_map_matches, 7);
        assert_eq!(gaps.len(), 7);
    }

    #[test]
    fn java_negative_count_is_empty_and_does_not_consume_following_fields() {
        for n in [-1, i32::MIN, 0] {
            let mut w = Writer::new();
            w.varint(n).u8(123);
            let bytes = w.into_inner();
            let (stack, used) = ItemStack::decode(&bytes).unwrap();
            assert!(stack.is_empty());
            assert_eq!(bytes[used], 123);
            assert_eq!(stack.encode(), vec![0]);
        }
    }

    #[test]
    fn invalid_id_unknown_components_and_truncation_are_rejected() {
        assert!(ItemStack::new(99999, 1).is_err());
        let bytes = vec![1, 1, 1, 0, 57];
        assert!(ItemStack::decode_exact(&bytes).is_err());
        let data: Value =
            serde_json::from_str(include_str!("../tools/inventory_vectors.json")).unwrap();
        let original: Value = serde_json::from_str(include_str!(
            "../fixtures/inventory_normalization_vectors.json"
        ))
        .unwrap();
        for vector in data["vectors"].as_array().unwrap().iter().skip(1) {
            let golden = &original["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["group"] == "independent117" && case["name"] == vector["name"])
                .unwrap()["expected"];
            let bytes = STANDARD
                .decode(vector["slot_base64"].as_str().unwrap())
                .unwrap();
            for cut in 0..bytes.len() {
                let accepted_prefix =
                    golden["accepted"] == true && golden["consumed"].as_u64() == Some(cut as u64);
                assert_eq!(
                    ItemStack::decode_exact(&bytes[..cut]).is_ok(),
                    accepted_prefix,
                    "{} prefix {cut}",
                    vector["name"]
                );
            }
        }
    }

    #[test]
    fn creative_slots_hotbar_offhand_and_oversized_stacks_match_java_ranges() {
        let mut inv = PlayerInventory::new();
        let stone = ItemStack::new(1, 64).unwrap();
        assert_eq!(
            inv.set_creative_slot(0, stone.clone()).unwrap(),
            CreativeSlotAction::Ignored
        );
        assert!(inv.slot(0).unwrap().is_empty());
        assert!(inv.set_creative_slot(36, stone.clone()).is_ok());
        assert_eq!(inv.selected_stack(0).unwrap(), &stone);
        inv.set_selected_hotbar(8).unwrap();
        assert!(inv.selected_stack(0).unwrap().is_empty());
        assert!(inv.set_selected_hotbar(9).is_err());
        assert_eq!(inv.selected_hotbar(), 8);
        assert!(inv.set_creative_slot(45, stone.clone()).is_ok());
        assert_eq!(inv.selected_stack(1).unwrap(), &stone);
        assert_eq!(
            inv.set_creative_slot(46, stone.clone()).unwrap(),
            CreativeSlotAction::Ignored
        );
        assert_eq!(
            inv.set_creative_slot(-1, stone.clone()).unwrap(),
            CreativeSlotAction::Drop(stone)
        );
        assert_eq!(
            inv.set_creative_slot(36, ItemStack::new(1, 65).unwrap())
                .unwrap(),
            CreativeSlotAction::Ignored
        );
        assert_eq!(inv.state_id(), 2);
    }

    #[test]
    fn patch_duplicates_last_value_and_removal_override_match_java_map() {
        // count1,itemstone,threeadded,two removals; damage overwritten then removed.
        let input = [1, 1, 3, 2, 3, 0, 3, 1, 1, 2, 3, 5];
        let stack = ItemStack::decode_exact(&input).unwrap();
        assert_eq!(
            stack.components,
            vec![Component {
                type_id: 1,
                data: vec![2]
            }]
        );
        assert!(stack.removed_components.is_empty());
        assert_eq!(stack.max_stack_size().unwrap(), 2);
        let oracle: Value = serde_json::from_str(include_str!(
            "../fixtures/inventory_normalization_regressions.json"
        ))
        .unwrap();
        let golden = STANDARD
            .decode(
                oracle["cases"][0]["expected"]["canonical"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(stack.encode(), golden);
    }

    #[test]
    fn block_catalog_resolves_item_and_oriented_mixed_radix_state() {
        let log_id = catalog()["items"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, v)| v["name"] == "oak_log")
            .unwrap()
            .0
            .parse()
            .unwrap();
        let state = item_default_block_state(log_id).unwrap();
        assert_eq!(state, 131);
        assert_eq!(
            block_state_with_properties(state, &BTreeMap::from([("axis".into(), "x".into())]))
                .unwrap(),
            130
        );
        assert_eq!(
            block_state_with_properties(state, &BTreeMap::from([("axis".into(), "z".into())]))
                .unwrap(),
            132
        );
        assert!(block_state_with_properties(
            state,
            &BTreeMap::from([("axis".into(), "invalid".into())])
        )
        .is_err());
        assert_eq!(
            block_state_with_properties(9, &BTreeMap::from([("snowy".into(), "true".into())]))
                .unwrap(),
            8
        );
    }

    #[test]
    fn independent_all_block_boundary_states_decode_property_order() {
        let data: Value =
            serde_json::from_str(include_str!("../tools/inventory_vectors.json")).unwrap();
        let samples = data["states"].as_array().unwrap();
        assert!(samples.len() > 1500);
        for sample in samples {
            let props = sample["properties"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                .collect::<BTreeMap<_, _>>();
            let state =
                block_state_with_properties(sample["base"].as_u64().unwrap() as u32, &props)
                    .unwrap();
            assert_eq!(u64::from(state), sample["state"].as_u64().unwrap());
        }
    }

    #[test]
    fn independent_window_and_slot_packets_match_native_inventory() {
        let data: Value =
            serde_json::from_str(include_str!("../tools/inventory_vectors.json")).unwrap();
        let mut inventory = PlayerInventory::new();
        inventory
            .set_creative_slot(36, ItemStack::new(1, 64).unwrap())
            .unwrap();
        inventory
            .set_creative_slot(45, ItemStack::new(1, 64).unwrap())
            .unwrap();
        assert_eq!(
            inventory.window_items_payload(),
            STANDARD
                .decode(data["packets"][0]["payload_base64"].as_str().unwrap())
                .unwrap()
        );
        assert_eq!(
            inventory.set_slot_payload(36).unwrap(),
            STANDARD
                .decode(data["packets"][1]["payload_base64"].as_str().unwrap())
                .unwrap()
        );
    }

    #[test]
    fn creative_ordinary_codec_accepts_stackable_damage_and_nested_non_strict_count() {
        let mut custom = ItemStack::new(1, 99).unwrap();
        custom.components.push(Component {
            type_id: 1,
            data: vec![99],
        });
        assert!(custom.validate_creative().is_ok());
        custom.count = 100;
        assert!(custom.validate_creative().is_err());
        custom.count = 1;
        custom.components.push(Component {
            type_id: 2,
            data: vec![1],
        });
        assert!(custom.validate_creative().is_ok());
        custom.components[0].data = vec![1];
        assert!(custom.validate_creative().is_ok());
        let mut nested = ItemStack::new(1, 1).unwrap();
        let mut w = Writer::new();
        w.varint(1).raw(&ItemStack::new(1, 65).unwrap().encode());
        nested.components.push(Component {
            type_id: 52,
            data: w.into_inner(),
        });
        assert!(nested.validate_creative().is_ok());
        let mut w = Writer::new();
        w.varint(1).raw(&ItemStack::new(1, 64).unwrap().encode());
        nested.components[0].data = w.into_inner();
        assert!(nested.validate_creative().is_ok());
    }
}
