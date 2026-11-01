//! Existing vanilla 1.21 overworld chunks, backed by lossless Anvil/NBT storage.
//!
//! No terrain is invented for missing chunks. Unknown disk fields are retained.
//! Block edits invalidate lighting until a light engine installs replacement
//! arrays. Per-type public blockentity snapshots select only update-tag data;
//! their world tick logic belongs to the corresponding simulation.
use crate::{
    anvil::{region_coordinates, ChunkCompression, RegionFile},
    inventory::block_state_with_properties,
    lighting::LightVolume,
    nbt::{NbtRoot, Tag},
    world::{MAX_Y, MIN_Y},
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

const MIN_SECTION: i32 = -4;
const SECTION_COUNT: usize = 24;
const LIGHT_SECTION_COUNT: usize = SECTION_COUNT + 2;
const NIBBLE_BYTES: usize = 2048;
const HEIGHTMAP_NAMES: [&str; 4] = [
    "WORLD_SURFACE",
    "OCEAN_FLOOR",
    "MOTION_BLOCKING",
    "MOTION_BLOCKING_NO_LEAVES",
];
const HEIGHTMAP_BITS: usize = 9;
const HEIGHTMAP_VALUES_PER_LONG: usize = 64 / HEIGHTMAP_BITS;
const HEIGHTMAP_LONGS: usize = 256_usize.div_ceil(HEIGHTMAP_VALUES_PER_LONG);

fn heightmap_matches(state_bits: u8, kind: usize) -> bool {
    match kind {
        0 => state_bits & 1 == 0,
        1 => state_bits & 2 != 0,
        2 => state_bits & 6 != 0,
        3 => state_bits & 6 != 0 && state_bits & 8 == 0,
        _ => unreachable!("only the four persisted overworld heightmaps"),
    }
}

/// Heightmap.update uses the first available Y, not the highest occupied Y.
/// The caller supplies the already-edited column and validated prior height.
/// Return its new first-available height and the original method's changed flag.
fn update_heightmap_column(
    first_available: i32,
    y: i32,
    new_state_bits: u8,
    kind: usize,
    mut column_bits: impl FnMut(i32) -> u8,
) -> (i32, bool) {
    if y <= first_available - 2 {
        return (first_available, false);
    }
    if heightmap_matches(new_state_bits, kind) {
        if y >= first_available {
            return (y + 1, true);
        }
    } else if y == first_available - 1 {
        for below in (MIN_Y..y).rev() {
            if heightmap_matches(column_bits(below), kind) {
                return (below + 1, true);
            }
        }
        return (MIN_Y, true);
    }
    (first_available, false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotGenerated {
    pub chunk_x: i32,
    pub chunk_z: i32,
}
impl std::fmt::Display for NotGenerated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "chunk {},{} is not generated",
            self.chunk_x, self.chunk_z
        )
    }
}
impl std::error::Error for NotGenerated {}

#[derive(Deserialize)]
struct RegistryFile {
    biomes: BTreeMap<String, u32>,
    block_entities: BTreeMap<String, u32>,
    air_states: Vec<u32>,
}
struct Catalog {
    blocks: BTreeMap<String, BlockDefinition>,
    registries: RegistryFile,
    states: Vec<Tag>,
    height_bits: Vec<u8>,
    block_entity_defaults: BTreeMap<String, Tag>,
    empty_update_types: BTreeSet<String>,
    block_entity_factory_states: Vec<i16>,
    block_entity_factory_templates: Vec<Tag>,
}
#[derive(Deserialize)]
struct BlockDefinition {
    min_state: u32,
    max_state: u32,
    default_state: u32,
    properties: Vec<Property>,
}
#[derive(Deserialize)]
struct Property {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    num_values: u32,
    #[serde(default)]
    values: Vec<serde_json::Value>,
}
fn property_value(property: &Property, index: u32) -> String {
    if property.kind == "bool" {
        return if index == 0 { "true" } else { "false" }.into();
    }
    match property.values.get(index as usize) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => index.to_string(),
    }
}
fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let generated: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/items-1.21.json"))
                .expect("verified generated catalog");
        let blocks: BTreeMap<String, BlockDefinition> =
            serde_json::from_value(generated["blocks"].clone()).expect("verified blocks");
        let registries: RegistryFile =
            serde_json::from_str(include_str!("../tools/storage_registries.json"))
                .expect("verified wire registries");
        let count = blocks.values().map(|b| b.max_state).max().unwrap() as usize + 1;
        let mut states = vec![Tag::Compound(Vec::new()); count];
        for (name, block) in &blocks {
            for id in block.min_state..=block.max_state {
                let mut remaining = id - block.min_state;
                let mut properties = Vec::new();
                for property in block.properties.iter().rev() {
                    let index = remaining % property.num_values;
                    remaining /= property.num_values;
                    properties.push((
                        property.name.as_str().into(),
                        Tag::String(property_value(property, index).into()),
                    ));
                }
                properties.reverse();
                let mut entries = vec![(
                    "Name".into(),
                    Tag::String(format!("minecraft:{name}").into()),
                )];
                if !properties.is_empty() {
                    entries.push(("Properties".into(), Tag::Compound(properties)));
                }
                states[id as usize] = Tag::Compound(entries);
            }
        }
        let light: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/light-1.21.json"))
                .expect("verified Java light facts");
        let height_bits = light["states"]
            .as_array()
            .expect("state row indices")
            .iter()
            .map(|row| {
                light["rows"][row.as_u64().expect("row index") as usize][2]
                    .as_u64()
                    .expect("height flags") as u8
            })
            .collect();
        let block_entity_records: serde_json::Value =
            serde_json::from_str(include_str!("../tools/storage_blockentity_defaults.json"))
                .expect("verified Java blockentity defaults");
        let mut block_entity_defaults = BTreeMap::new();
        let mut empty_update_types = BTreeSet::new();
        for record in block_entity_records
            .as_array()
            .expect("blockentity records")
        {
            let name = record["name"]
                .as_str()
                .expect("blockentity name")
                .to_owned();
            if record["update_declaring_class"] == "dqh" {
                empty_update_types.insert(name.clone());
            }
            let encoded = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                record["update_nbt"].as_str().expect("default updateNBT"),
            )
            .expect("defaultNBT base64");
            block_entity_defaults.insert(
                name,
                NbtRoot::decode(&encoded).expect("Java defaultNBT").tag,
            );
        }
        let factory: serde_json::Value =
            serde_json::from_str(include_str!("../tools/storage_blockentity_factories.json"))
                .expect("verified Java entityblock factories");
        let block_entity_factory_states = factory["states"]
            .as_array()
            .expect("allstatefactorymapping")
            .iter()
            .map(|v| v.as_i64().expect("factoryrow") as i16)
            .collect();
        let block_entity_factory_templates = factory["templates"]
            .as_array()
            .expect("factorytemplates")
            .iter()
            .map(|row| {
                let encoded = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    row["full_nbt"].as_str().expect("factoryNBT"),
                )
                .expect("factorybase64");
                NbtRoot::decode(&encoded).expect("actualJavafactoryNBT").tag
            })
            .collect();
        Catalog {
            blocks,
            registries,
            states,
            height_bits,
            block_entity_defaults,
            empty_update_types,
            block_entity_factory_states,
            block_entity_factory_templates,
        }
    })
}
fn string(tag: &Tag) -> Result<&str> {
    if let Tag::String(value) = tag {
        value.as_str().context("non-Unicode registry name")
    } else {
        bail!("expected NBT string")
    }
}
fn int(tag: Option<&Tag>, field: &str) -> Result<i32> {
    match tag {
        Some(Tag::Int(v)) => Ok(*v),
        _ => bail!("missing or invalid chunk {field}"),
    }
}
fn list(tag: &Tag) -> Result<&[Tag]> {
    match tag {
        Tag::List { values, .. } => Ok(values),
        _ => bail!("expected NBT list"),
    }
}
fn state_from_nbt(tag: &Tag) -> Result<u32> {
    let name = string(tag.get("Name").context("missing block palette Name")?)?;
    let block = catalog()
        .blocks
        .get(name.strip_prefix("minecraft:").unwrap_or(name))
        .ok_or_else(|| anyhow!("unknown 1.21 block {name}"))?;
    let mut properties = BTreeMap::new();
    if let Some(Tag::Compound(entries)) = tag.get("Properties") {
        for (key, value) in entries {
            properties.insert(
                key.as_str().context("invalid property name")?.to_owned(),
                string(value)?.to_owned(),
            );
        }
    }
    block_state_with_properties(block.default_state, &properties)
}
fn bits_for_palette(length: usize, minimum: usize) -> usize {
    if length <= 1 {
        0
    } else {
        ((usize::BITS - (length - 1).leading_zeros()) as usize).max(minimum)
    }
}
fn unpack_palette(
    container: &Tag,
    volume: usize,
    minimum: usize,
    decode: impl Fn(&Tag) -> Result<u32>,
) -> Result<Vec<u32>> {
    let palette = list(container.get("palette").context("missing disk palette")?)?
        .iter()
        .map(decode)
        .collect::<Result<Vec<_>>>()?;
    ensure!(!palette.is_empty(), "empty disk palette");
    if palette.len() == 1 {
        return Ok(vec![palette[0]; volume]);
    }
    let bits = bits_for_palette(palette.len(), minimum);
    ensure!(bits <= 32, "disk palette too large");
    let per_long = 64 / bits;
    let words = match container.get("data") {
        Some(Tag::LongArray(v)) => v,
        _ => bail!("multi-valued disk palette has no data"),
    };
    ensure!(
        words.len() == volume.div_ceil(per_long),
        "incorrect disk palette data length"
    );
    let mask = (1u64 << bits) - 1;
    (0..volume)
        .map(|i| {
            let index = ((words[i / per_long] as u64 >> ((i % per_long) * bits)) & mask) as usize;
            palette
                .get(index)
                .copied()
                .context("disk palette index out of range")
        })
        .collect()
}
fn palette(values: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut palette = Vec::new();
    let mut indices = Vec::with_capacity(values.len());
    let mut lookup = BTreeMap::new();
    for &value in values {
        let next = palette.len() as u32;
        let index = *lookup.entry(value).or_insert_with(|| {
            palette.push(value);
            next
        });
        indices.push(index);
    }
    (palette, indices)
}
fn pack(values: &[u32], bits: usize) -> Vec<i64> {
    let per_long = 64 / bits;
    let mut words = vec![0u64; values.len().div_ceil(per_long)];
    for (i, &value) in values.iter().enumerate() {
        words[i / per_long] |= u64::from(value) << ((i % per_long) * bits);
    }
    words.into_iter().map(|v| v as i64).collect()
}
fn disk_blocks(values: &[u32]) -> Tag {
    let (palette, indices) = palette(values);
    let bits = bits_for_palette(palette.len(), 4);
    let mut entries = vec![(
        "palette".into(),
        Tag::List {
            element_type: 10,
            values: palette
                .into_iter()
                .map(|id| catalog().states[id as usize].clone())
                .collect(),
        },
    )];
    if bits > 0 {
        entries.push(("data".into(), Tag::LongArray(pack(&indices, bits))));
    }
    Tag::Compound(entries)
}
fn varint(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let b = (value & 127) as u8;
        value >>= 7;
        out.push(if value == 0 { b } else { b | 128 });
        if value == 0 {
            break;
        }
    }
}
fn network_palette(
    values: &[u32],
    minimum: usize,
    maximum_indirect: usize,
    direct_bits: usize,
    out: &mut Vec<u8>,
) {
    let (palette, indices) = palette(values);
    let indirect = palette.len() <= maximum_indirect;
    let bits = bits_for_palette(palette.len(), minimum);
    if bits == 0 {
        out.push(0);
        varint(palette[0], out);
        varint(0, out);
        return;
    }
    let bits = if indirect { bits } else { direct_bits };
    out.push(bits as u8);
    if indirect {
        varint(palette.len() as u32, out);
        for value in palette {
            varint(value, out);
        }
    }
    let words = pack(if indirect { &indices } else { values }, bits);
    varint(words.len() as u32, out);
    for word in words {
        out.extend_from_slice(&word.to_be_bytes());
    }
}
fn bitset(bits: u32, out: &mut Vec<u8>) {
    if bits == 0 {
        varint(0, out);
    } else {
        varint(1, out);
        out.extend_from_slice(&u64::from(bits).to_be_bytes());
    }
}

/// Actual EntityBlock.newBlockEntity factory snapshot before worldticks.
/// A moving-piston state's factory returnsNone: piston simulation supplies its
/// specialized moving-block entity separately, as it does in Java.
pub fn block_entity_factory_snapshot(state: u32, x: i32, y: i32, z: i32) -> Result<Option<Tag>> {
    let row = *catalog()
        .block_entity_factory_states
        .get(state as usize)
        .context("unknown exact1.21stateID")?;
    if row < 0 {
        return Ok(None);
    }
    let mut tag = catalog().block_entity_factory_templates[row as usize].clone();
    for (name, value) in [("x", x), ("y", y), ("z", z)] {
        tag.insert(name, Tag::Int(value))?;
    }
    Ok(Some(tag))
}
fn entity_at(tag: &Tag, x: i32, y: i32, z: i32) -> bool {
    tag.get("x") == Some(&Tag::Int(x))
        && tag.get("y") == Some(&Tag::Int(y))
        && tag.get("z") == Some(&Tag::Int(z))
}
fn transition_block_entity(
    chunk: &mut CachedChunk,
    old: u32,
    new: u32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<()> {
    let block_changed =
        catalog().states[old as usize].get("Name") != catalog().states[new as usize].get("Name");
    let next = block_entity_factory_snapshot(new, x, y, z)?;
    let tag = chunk
        .root
        .tag
        .get_mut("block_entities")
        .context("missing chunkblockentities")?;
    let Tag::List {
        element_type,
        values,
    } = tag
    else {
        bail!("blockentities mustbelist")
    };
    ensure!(
        *element_type == 10 || values.is_empty(),
        "blockentities mustbecompoundlist"
    );
    if block_changed {
        values.retain(|tag| !entity_at(tag, x, y, z));
    }
    if !values.iter().any(|tag| entity_at(tag, x, y, z)) {
        if let Some(next) = next {
            *element_type = 10;
            values.push(next);
        }
    }
    Ok(())
}

/// Public update-tag projection of a canonical saved/live blockentity snapshot.
/// This reproduces the packetdata selection, not blockentity tick behavior.
/// Runtime-only fields (e.g. brushhitdirection) must be retained in the supplied
/// live snapshot by the corresponding simulation, since Java does not persist
/// all of them. Container inventories remain private for all28base-emptytypes.
pub fn block_entity_update_tag(snapshot: &Tag, state: u32) -> Result<Option<Tag>> {
    let name = string(snapshot.get("id").context("blockentity has noid")?)?;
    ensure!(
        catalog().registries.block_entities.contains_key(name),
        "unknown exact1.21blockentitytype"
    );
    if catalog().empty_update_types.contains(name) {
        return Ok(None);
    }
    let fields: &[&str] = match name.strip_prefix("minecraft:").unwrap_or(name) {
        "banner" => &["patterns", "CustomName", "components"],
        "beacon" => &[
            "primary_effect",
            "secondary_effect",
            "Levels",
            "CustomName",
            "Lock",
        ],
        "sign" | "hanging_sign" => &["front_text", "back_text", "is_waxed"],
        "mob_spawner" => &[
            "MaxNearbyEntities",
            "RequiredPlayerRange",
            "SpawnCount",
            "MaxSpawnDelay",
            "SpawnRange",
            "Delay",
            "MinSpawnDelay",
            "SpawnData",
        ],
        "piston" => &["blockState", "facing", "progress", "source", "extending"],
        "skull" => &["profile", "note_block_sound", "custom_name"],
        "structure_block" => &[
            "metadata",
            "mirror",
            "ignoreEntities",
            "powered",
            "seed",
            "author",
            "rotation",
            "posX",
            "mode",
            "posY",
            "sizeX",
            "posZ",
            "integrity",
            "showair",
            "name",
            "sizeY",
            "sizeZ",
            "showboundingbox",
        ],
        "end_gateway" => &["Age", "ExitPortal", "ExactTeleport"],
        "conduit" => &["Target"],
        "jigsaw" => &[
            "joint",
            "name",
            "pool",
            "final_state",
            "placement_priority",
            "selection_priority",
            "target",
        ],
        "campfire" => &["Items"],
        "brushable_block" => &["item", "hit_direction"],
        "decorated_pot" => &["sherds", "item", "LootTable", "LootTableSeed"],
        "trial_spawner" => &["spawn_data"],
        "vault" => &["shared_data"],
        _ => bail!("unsupported publicblockentitysnapshot codec{name}"),
    };
    let mut out = catalog().block_entity_defaults[name].clone();
    for &field in fields {
        if let Some(tag) = snapshot.get(field) {
            out.insert(field, tag.clone())?;
        }
    }
    if name == "minecraft:trial_spawner" {
        let state = catalog()
            .states
            .get(state as usize)
            .context("unknown blockstate")?;
        if state
            .get("Properties")
            .and_then(|t| t.get("trial_spawner_state"))
            .map(string)
            .transpose()?
            == Some("active")
        {
            out.insert(
                "next_mob_spawns_at",
                snapshot
                    .get("next_mob_spawns_at")
                    .cloned()
                    .unwrap_or(Tag::Long(0)),
            )?;
        }
    }
    ensure!(matches!(&out, Tag::Compound(_)), "updateTag mustbecompound");
    if matches!(&out,Tag::Compound(entries) if entries.is_empty()) {
        Ok(None)
    } else {
        Ok(Some(out))
    }
}
/// Clientbound BlockEntityData payload (protocol767packet0x07), excludingID.
/// The snapshot must describe the livepublic state; thismethod doesnot tickit.
pub fn block_entity_data_packet(snapshot: &Tag, state: u32) -> Result<Vec<u8>> {
    let x = int(snapshot.get("x"), "blockentity.x")?;
    let y = int(snapshot.get("y"), "blockentity.y")?;
    let z = int(snapshot.get("z"), "blockentity.z")?;
    let name = string(snapshot.get("id").context("blockentity has noid")?)?;
    let kind = *catalog()
        .registries
        .block_entities
        .get(name)
        .context("unknown blockentitytype")?;
    let mut out = crate::world::packed_position(x, y, z)?
        .to_be_bytes()
        .to_vec();
    varint(kind, &mut out);
    match block_entity_update_tag(snapshot, state)? {
        Some(tag) => out.extend_from_slice(&tag.encode_network_anonymous()?),
        None => out.push(0),
    }
    Ok(out)
}

fn network_block_entities(
    block_entities: &[Tag],
    chunk: &CachedChunk,
    cx: i32,
    cz: i32,
    out: &mut Vec<u8>,
) -> Result<()> {
    varint(block_entities.len() as u32, out);
    for snapshot in block_entities {
        let x = int(snapshot.get("x"), "blockentity.x")?;
        let y = int(snapshot.get("y"), "blockentity.y")?;
        let z = int(snapshot.get("z"), "blockentity.z")?;
        ensure!(
            x.div_euclid(16) == cx && z.div_euclid(16) == cz && (MIN_Y..=MAX_Y).contains(&y),
            "blockentity coordinates outside chunk"
        );
        let name = string(snapshot.get("id").context("blockentity has noid")?)?;
        let kind = *catalog()
            .registries
            .block_entities
            .get(name)
            .context("unknown blockentitytype")?;
        let tag = block_entity_update_tag(snapshot, chunk.block(x, y, z))?;
        out.push((((x & 15) << 4) | (z & 15)) as u8);
        out.extend_from_slice(&(y as i16).to_be_bytes());
        varint(kind, out);
        match tag {
            Some(tag) => out.extend_from_slice(&tag.encode_network_anonymous()?),
            None => out.push(0),
        }
    }
    Ok(())
}

#[derive(Clone)]
struct Section {
    states: Vec<u32>,
    biomes: Vec<u32>,
    non_air: usize,
}
struct CachedChunk {
    root: NbtRoot,
    sections: Vec<Section>,
    dirty: bool,
    lighting_dirty: bool,
    heightmaps_dirty: bool,
    // Disk maps are not trusted as incremental baselines until a native prime
    // has checked them against every column. This also repairs stale/invalid
    // imported maps on the first edit, retaining the previous rebuild behavior.
    heightmaps_validated: bool,
    /// Runtime version of canonical light arrays/allocation masks; unrelated
    /// block, blockentity and heightmap dirtiness never advance this value.
    light_revision: u64,
}
impl CachedChunk {
    fn decode(root: NbtRoot, cx: i32, cz: i32) -> Result<Self> {
        ensure!(
            int(root.tag.get("DataVersion"), "DataVersion")? == 3953,
            "only exact Minecraft 1.21 DataVersion3953 is supported"
        );
        ensure!(
            int(root.tag.get("xPos"), "xPos")? == cx && int(root.tag.get("zPos"), "zPos")? == cz,
            "chunk coordinates differ from region entry"
        );
        ensure!(
            root.tag
                .get("Status")
                .map(string)
                .transpose()?
                .is_some_and(|s| s == "minecraft:full" || s == "full"),
            "chunk is not full"
        );
        ensure!(
            int(root.tag.get("yPos"), "yPos")? == MIN_SECTION,
            "expected 24-section overworld"
        );
        let default_biome = *catalog()
            .registries
            .biomes
            .get("minecraft:plains")
            .context("missing plains wire ID")?;
        let mut sections = vec![
            Section {
                states: vec![0; 4096],
                biomes: vec![default_biome; 64],
                non_air: 0,
            };
            SECTION_COUNT
        ];
        let mut seen = BTreeSet::new();
        for section in list(root.tag.get("sections").context("missing chunk sections")?)? {
            let sy = match section.get("Y") {
                Some(Tag::Byte(y)) => i32::from(*y),
                _ => bail!("missing sectionY"),
            };
            ensure!(seen.insert(sy), "duplicate chunk sectionY");
            if !(MIN_SECTION..MIN_SECTION + SECTION_COUNT as i32).contains(&sy) {
                continue;
            }
            let index = (sy - MIN_SECTION) as usize;
            sections[index].states = match section.get("block_states") {
                Some(container) => unpack_palette(container, 4096, 4, state_from_nbt)?,
                None => vec![0; 4096],
            };
            sections[index].non_air = sections[index]
                .states
                .iter()
                .filter(|state| !catalog().registries.air_states.contains(state))
                .count();
            if let Some(container) = section.get("biomes") {
                sections[index].biomes = unpack_palette(container, 64, 1, |tag| {
                    catalog()
                        .registries
                        .biomes
                        .get(string(tag)?)
                        .copied()
                        .context("unknown biome in exact1.21 registry")
                })?;
            }
        }
        Ok(Self {
            root,
            sections,
            dirty: false,
            lighting_dirty: false,
            heightmaps_dirty: false,
            heightmaps_validated: false,
            light_revision: 0,
        })
    }
    fn section_tag_mut(&mut self, sy: i32) -> Result<&mut Tag> {
        let tag = self
            .root
            .tag
            .get_mut("sections")
            .context("missing chunksections")?;
        let Tag::List {
            element_type,
            values,
        } = tag
        else {
            bail!("sections is notlist")
        };
        ensure!(*element_type == 10, "sections notcompoundlist");
        if let Some(i) = values
            .iter()
            .position(|s| s.get("Y") == Some(&Tag::Byte(sy as i8)))
        {
            return Ok(&mut values[i]);
        }
        values.push(Tag::Compound(vec![("Y".into(), Tag::Byte(sy as i8))]));
        Ok(values.last_mut().unwrap())
    }
    fn block(&self, x: i32, y: i32, z: i32) -> u32 {
        if !(MIN_Y..=MAX_Y).contains(&y) {
            return 0;
        }
        self.sections[((y - MIN_Y) / 16) as usize].states
            [(((y & 15) << 8) | ((z & 15) << 4) | (x & 15)) as usize]
    }

    /// Update just this column's four packed heights after a state change.
    /// A false result requests a full prime because the imported maps have not
    /// yet been validated. All word checks precede mutations of any heightmap.
    fn update_heightmaps(&mut self, x: i32, y: i32, z: i32, state: u32) -> Result<bool> {
        if !self.heightmaps_validated {
            return Ok(false);
        }
        let index = ((z & 15) * 16 + (x & 15)) as usize;
        let word_index = index / HEIGHTMAP_VALUES_PER_LONG;
        let shift = (index % HEIGHTMAP_VALUES_PER_LONG) * HEIGHTMAP_BITS;
        let mut heights = [MIN_Y; 4];
        let Some(Tag::Compound(entries)) = self.root.tag.get("Heightmaps") else {
            self.heightmaps_validated = false;
            return Ok(false);
        };
        for (kind, name) in HEIGHTMAP_NAMES.into_iter().enumerate() {
            let Some(Tag::LongArray(words)) = entries
                .iter()
                .find_map(|(key, value)| (key == name).then_some(value))
            else {
                self.heightmaps_validated = false;
                return Ok(false);
            };
            if words.len() != HEIGHTMAP_LONGS {
                self.heightmaps_validated = false;
                return Ok(false);
            }
            let stored = (words[word_index] as u64 >> shift) & 0x1ff;
            if stored > (MAX_Y - MIN_Y + 1) as u64 {
                self.heightmaps_validated = false;
                return Ok(false);
            }
            heights[kind] = stored as i32 + MIN_Y;
        }
        let state_bits = &catalog().height_bits;
        let mut changed = [false; 4];
        for kind in 0..4 {
            (heights[kind], changed[kind]) = update_heightmap_column(
                heights[kind],
                y,
                state_bits[state as usize],
                kind,
                |below| state_bits[self.block(x, below, z) as usize],
            );
        }
        let Some(Tag::Compound(entries)) = self.root.tag.get_mut("Heightmaps") else {
            unreachable!("validated above without changing the root")
        };
        for (kind, name) in HEIGHTMAP_NAMES.into_iter().enumerate() {
            if changed[kind] {
                let Tag::LongArray(words) = entries
                    .iter_mut()
                    .find_map(|(key, value)| (key == name).then_some(value))
                    .expect("validated above")
                else {
                    unreachable!("validated above")
                };
                let mask = 0x1ff_u64 << shift;
                words[word_index] = ((words[word_index] as u64 & !mask)
                    | (((heights[kind] - MIN_Y) as u64) << shift))
                    as i64;
            }
        }
        self.heightmaps_dirty = false;
        Ok(true)
    }

    fn prime_heightmaps(&mut self, state_bits: &[u8]) -> Result<()> {
        ensure!(
            state_bits.len() == catalog().states.len(),
            "heightmap facts do not cover exact1.21states"
        );
        let mut entries = Vec::new();
        for (kind, name) in HEIGHTMAP_NAMES.into_iter().enumerate() {
            let mut heights = vec![0u32; 256];
            for z in 0..16 {
                for x in 0..16 {
                    for y in (MIN_Y..=MAX_Y).rev() {
                        let bits = state_bits[self.block(x, y, z) as usize];
                        if heightmap_matches(bits, kind) {
                            heights[(z * 16 + x) as usize] = (y - MIN_Y + 1) as u32;
                            break;
                        }
                    }
                }
            }
            entries.push((name.into(), Tag::LongArray(pack(&heights, HEIGHTMAP_BITS))));
        }
        self.root.tag.insert("Heightmaps", Tag::Compound(entries))?;
        self.heightmaps_dirty = false;
        self.heightmaps_validated = state_bits == catalog().height_bits;
        self.dirty = true;
        Ok(())
    }
}

/// One owner of an existing overworld directory. Lazy reads perform disk I/O;
/// server integrations should place this owner behind a blocking-I/O boundary.
struct CachedLighting {
    center: (i32, i32),
    volume: LightVolume,
}
pub struct VanillaWorld {
    path: PathBuf,
    regions: BTreeMap<(i32, i32), Option<RegionFile>>,
    chunks: BTreeMap<(i32, i32), CachedChunk>,
    light_cache: Option<CachedLighting>,
    light_edits: BTreeMap<(i32, i32, i32), u32>,
    last_relight_columns: usize,
    // Previous completed wire allocation masks, captured only on transitions
    // between an empty/nonempty block section. Values include the revision so
    // finish_lighting does not count an array change twice.
    pending_light_masks: BTreeMap<(i32, i32), (u32, u64)>,
}
impl VanillaWorld {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_owned();
        ensure!(path.join("region").is_dir(), "world has no regiondirectory");
        Ok(Self {
            path,
            regions: BTreeMap::new(),
            chunks: BTreeMap::new(),
            light_cache: None,
            light_edits: BTreeMap::new(),
            last_relight_columns: 0,
            pending_light_masks: BTreeMap::new(),
        })
    }
    fn region(&mut self, cx: i32, cz: i32) -> Result<Option<&mut RegionFile>> {
        let key = region_coordinates(cx, cz);
        if !self.regions.contains_key(&key) {
            let path = self
                .path
                .join("region")
                .join(format!("r.{}.{}.mca", key.0, key.1));
            let region = if path.exists() {
                Some(RegionFile::read(&path)?)
            } else {
                None
            };
            self.regions.insert(key, region);
        }
        Ok(self.regions.get_mut(&key).and_then(Option::as_mut))
    }
    pub fn has_chunk(&mut self, cx: i32, cz: i32) -> Result<bool> {
        match self.region(cx, cz)? {
            Some(region) => Ok(region.raw_sectors(cx, cz)?.is_some()),
            None => Ok(false),
        }
    }
    fn chunk(&mut self, cx: i32, cz: i32) -> Result<&mut CachedChunk> {
        if !self.chunks.contains_key(&(cx, cz)) {
            let root = self
                .region(cx, cz)?
                .and_then(|r| r.chunk(cx, cz).transpose())
                .transpose()?
                .ok_or(NotGenerated {
                    chunk_x: cx,
                    chunk_z: cz,
                })?;
            self.chunks
                .insert((cx, cz), CachedChunk::decode(root, cx, cz)?);
        }
        Ok(self.chunks.get_mut(&(cx, cz)).unwrap())
    }
    pub fn block(&mut self, x: i32, y: i32, z: i32) -> Result<u32> {
        if !(MIN_Y..=MAX_Y).contains(&y) {
            return Ok(0);
        }
        Ok(self
            .chunk(x.div_euclid(16), z.div_euclid(16))?
            .block(x, y, z))
    }
    pub fn loaded_block_state(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        if !(MIN_Y..=MAX_Y).contains(&y) {
            return Some(0);
        }
        self.chunks
            .get(&(x.div_euclid(16), z.div_euclid(16)))
            .map(|c| c.block(x, y, z))
    }
    pub fn cached_chunk_is_loaded(&self, cx: i32, cz: i32) -> bool {
        self.chunks.contains_key(&(cx, cz))
    }
    pub fn loaded_chunk_positions(&self) -> Vec<(i32, i32)> {
        self.chunks.keys().copied().collect()
    }
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: u32) -> Result<bool> {
        ensure!(
            (MIN_Y..=MAX_Y).contains(&y),
            "block outside overworld height"
        );
        ensure!(
            (state as usize) < catalog().states.len(),
            "unknown 1.21 blockstate"
        );
        let (cx, cz) = (x.div_euclid(16), z.div_euclid(16));
        let section_index = ((y - MIN_Y) / 16) as usize;
        let index = (((y & 15) << 8) | ((z & 15) << 4) | (x & 15)) as usize;
        let (old_state, non_air) = {
            let chunk = self.chunk(cx, cz)?;
            (
                chunk.sections[section_index].states[index],
                chunk.sections[section_index].non_air,
            )
        };
        if old_state == state {
            return Ok(false);
        }
        let old_air = catalog().registries.air_states.contains(&old_state);
        let new_air = catalog().registries.air_states.contains(&state);
        if (old_air && !new_air && non_air == 0) || (!old_air && new_air && non_air == 1) {
            self.remember_light_masks(cx, cz)?;
        }
        let chunk = self.chunk(cx, cz)?;
        transition_block_entity(chunk, old_state, state, x, y, z)?;
        match (old_air, new_air) {
            (true, false) => chunk.sections[section_index].non_air += 1,
            (false, true) => chunk.sections[section_index].non_air -= 1,
            _ => {}
        }
        chunk.sections[section_index].states[index] = state;
        let block_states = disk_blocks(&chunk.sections[section_index].states);
        chunk
            .section_tag_mut(y.div_euclid(16))?
            .insert("block_states", block_states)?;
        chunk.root.tag.insert("isLightOn", Tag::Byte(0))?;
        chunk.dirty = true;
        chunk.lighting_dirty = true;
        chunk.heightmaps_dirty = true;
        if !chunk.update_heightmaps(x, y, z, state)? {
            // Prime imported, missing or structurally invalid maps once. Only
            // this canonical result becomes an incremental baseline.
            chunk.prime_heightmaps(&catalog().height_bits)?;
        }
        self.light_edits.insert((x, y, z), state);
        Ok(true)
    }
    /// Existing canonical snapshot at a blockposition. Worldticklogic may
    /// subsequently update its public/runtime fields through its simulation.
    pub fn block_entity_snapshot(&mut self, x: i32, y: i32, z: i32) -> Result<Option<&Tag>> {
        if !(MIN_Y..=MAX_Y).contains(&y) {
            return Ok(None);
        }
        let chunk = self.chunk(x.div_euclid(16), z.div_euclid(16))?;
        Ok(list(
            chunk
                .root
                .tag
                .get("block_entities")
                .context("missingblockentities")?,
        )?
        .iter()
        .find(|tag| entity_at(tag, x, y, z)))
    }
    /// ClientboundBlockEntityData payload after a blockchange, if the new
    /// block has a registered entity. The blockupdate removes oldclientdata.
    pub fn block_entity_packet(&mut self, x: i32, y: i32, z: i32) -> Result<Option<Vec<u8>>> {
        let state = self.block(x, y, z)?;
        self.block_entity_snapshot(x, y, z)?
            .map(|tag| block_entity_data_packet(tag, state))
            .transpose()
    }
    /// Install a complete light-engine result. Both arrays must have2048bytes.
    pub fn set_section_light(
        &mut self,
        cx: i32,
        cz: i32,
        sy: i32,
        sky: Option<&[u8]>,
        block: Option<&[u8]>,
    ) -> Result<()> {
        ensure!(
            (MIN_SECTION - 1..=MIN_SECTION + SECTION_COUNT as i32).contains(&sy),
            "lightsection outside overworld boundary"
        );
        for data in [sky, block].into_iter().flatten() {
            ensure!(data.len() == NIBBLE_BYTES, "invalid lightarraylength");
        }
        let chunk = self.chunk(cx, cz)?;
        if sky.is_none() && block.is_none() {
            let present = list(chunk.root.tag.get("sections").context("missingsections")?)?
                .iter()
                .any(|s| s.get("Y") == Some(&Tag::Byte(sy as i8)));
            if !present {
                return Ok(());
            }
        }
        let section = chunk.section_tag_mut(sy)?;
        let mut changed = false;
        for (key, value) in [("SkyLight", sky), ("BlockLight", block)] {
            let next = value.map(|data| Tag::ByteArray(data.iter().map(|&v| v as i8).collect()));
            if section.get(key) == next.as_ref() {
                continue;
            }
            changed = true;
            if let Some(data) = next {
                section.insert(key, data)?;
            } else if let Tag::Compound(entries) = section {
                entries.retain(|(name, _)| name != key);
            }
        }
        if changed {
            chunk.light_revision = chunk
                .light_revision
                .checked_add(1)
                .context("light revision exhausted")?;
        }
        chunk.dirty |= changed;
        Ok(())
    }
    /// Current runtime version of canonical light arrays and allocated masks.
    /// Lazy chunk loading starts at zero; versions survive block edits/saves.
    pub fn light_revision(&mut self, cx: i32, cz: i32) -> Result<u64> {
        Ok(self.chunk(cx, cz)?.light_revision)
    }

    fn canonical_light_mask(&mut self, cx: i32, cz: i32) -> Result<u32> {
        let active = self.active_light_sections(cx, cz)?;
        let mut mask = 0u32;
        for sy in active {
            if (MIN_SECTION - 1..=MIN_SECTION + SECTION_COUNT as i32).contains(&sy) {
                mask |= 1 << ((sy - MIN_SECTION + 1) as u32);
            }
        }
        let chunk = self.chunk(cx, cz)?;
        for section in list(chunk.root.tag.get("sections").context("missing sections")?)? {
            let Some(Tag::Byte(sy)) = section.get("Y") else {
                continue;
            };
            let sy = i32::from(*sy);
            if !(MIN_SECTION - 1..=MIN_SECTION + SECTION_COUNT as i32).contains(&sy) {
                continue;
            }
            if matches!(section.get("SkyLight"), Some(Tag::ByteArray(_)))
                || matches!(section.get("BlockLight"), Some(Tag::ByteArray(_)))
            {
                mask |= 1 << ((sy - MIN_SECTION + 1) as u32);
            }
        }
        Ok(mask)
    }
    fn remember_light_masks(&mut self, cx: i32, cz: i32) -> Result<()> {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let x = cx.checked_add(dx).context("light mask X overflow")?;
                let z = cz.checked_add(dz).context("light mask Z overflow")?;
                if self.pending_light_masks.contains_key(&(x, z)) || !self.has_chunk(x, z)? {
                    continue;
                }
                let mask = self.canonical_light_mask(x, z)?;
                let revision = self.light_revision(x, z)?;
                self.pending_light_masks.insert((x, z), (mask, revision));
            }
        }
        Ok(())
    }
    /// Call only after the light engine has installed every affected section.
    pub fn finish_lighting(&mut self, cx: i32, cz: i32) -> Result<()> {
        let mask_change =
            if let Some(&(previous, revision)) = self.pending_light_masks.get(&(cx, cz)) {
                self.light_revision(cx, cz)? == revision
                    && self.canonical_light_mask(cx, cz)? != previous
            } else {
                false
            };
        let chunk = self.chunk(cx, cz)?;
        if mask_change {
            chunk.light_revision = chunk
                .light_revision
                .checked_add(1)
                .context("light revision exhausted")?;
        }
        let changed =
            chunk.root.tag.get("isLightOn") != Some(&Tag::Byte(1)) || chunk.lighting_dirty;
        if changed {
            chunk.root.tag.insert("isLightOn", Tag::Byte(1))?;
            chunk.lighting_dirty = false;
            chunk.dirty = true;
        }
        self.pending_light_masks.remove(&(cx, cz));
        Ok(())
    }
    /// Rebuild stable light for the3x3chunk neighborhood around an edit, using
    /// a complete5x5chunk domain as the16block dependencyhalo. Missing halo
    /// chunks fail explicitly; no closedboundary result replaces unknownlight.
    /// Returns the outputchunkcoordinates for networklight publication.
    /// A retained volume applies subsequent edits incrementally for the same
    /// domain. Initial construction performs blocking CPU and disk work.
    pub fn relight_around(&mut self, cx: i32, cz: i32) -> Result<Vec<(i32, i32)>> {
        let mut domain = Vec::new();
        for dz in -2..=2 {
            for dx in -2..=2 {
                let x = cx.checked_add(dx).context("lightcontextXoverflow")?;
                let z = cz.checked_add(dz).context("lightcontextZoverflow")?;
                if !self.has_chunk(x, z)? {
                    return Err(NotGenerated {
                        chunk_x: x,
                        chunk_z: z,
                    }
                    .into());
                }
                self.chunk(x, z)?;
                domain.push((x, z));
            }
        }
        let cache = self.light_cache.take();
        let (mut volume, rebuild) = match cache {
            Some(CachedLighting { center, volume }) if center == (cx, cz) => (volume, false),
            _ => (
                LightVolume::new(MIN_Y, MAX_Y - MIN_Y + 1, domain.iter().copied())?,
                true,
            ),
        };
        let mut active = BTreeSet::new();
        for &(x, z) in &domain {
            let chunk = &self.chunks[&(x, z)];
            for (index, section) in chunk.sections.iter().enumerate() {
                let sy = MIN_SECTION + index as i32;
                if section.non_air > 0 {
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            for dy in -1..=1 {
                                active.insert((x + dx, sy + dy, z + dz));
                            }
                        }
                    }
                }
                if rebuild {
                    for (i, &state) in section.states.iter().enumerate() {
                        if state != 0 {
                            volume.set_block(
                                x * 16 + (i & 15) as i32,
                                sy * 16 + (i >> 8) as i32,
                                z * 16 + ((i >> 4) & 15) as i32,
                                state,
                            )?;
                        }
                    }
                }
            }
        }
        if !rebuild {
            for (&(x, y, z), &state) in &self.light_edits {
                if volume.block(x, y, z).is_some() {
                    volume.set_block(x, y, z, state)?;
                }
            }
        }
        volume.relight()?;
        self.last_relight_columns = volume.last_rebuilt_columns();
        self.light_edits
            .retain(|&(x, _, z), _| !domain.contains(&(x.div_euclid(16), z.div_euclid(16))));
        let mut output = Vec::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                let x = cx + dx;
                let z = cz + dz;
                for sy in MIN_SECTION - 1..=MIN_SECTION + SECTION_COUNT as i32 {
                    if active.contains(&(x, sy, z)) {
                        let (sky, block) = volume.section_light(x, z, sy)?;
                        self.set_section_light(
                            x,
                            z,
                            sy,
                            sky.iter().any(|&v| v != 0).then_some(sky.as_slice()),
                            block.iter().any(|&v| v != 0).then_some(block.as_slice()),
                        )?;
                    } else {
                        self.set_section_light(x, z, sy, None, None)?;
                    }
                }
                self.finish_lighting(x, z)?;
                output.push((x, z));
            }
        }
        self.light_cache = Some(CachedLighting {
            center: (cx, cz),
            volume,
        });
        Ok(output)
    }
    /// Number of x/zblockcolumns rebuilt by the mostrecent relight.
    pub fn last_relight_rebuilt_columns(&self) -> usize {
        self.last_relight_columns
    }
    pub fn lighting_is_dirty(&mut self, cx: i32, cz: i32) -> Result<bool> {
        Ok(self.chunk(cx, cz)?.lighting_dirty)
    }
    /// Install all four vanilla heightmap predicates, indexed by globalstateID.
    /// Bits:0=isAir,1=blocksMotion,2=fluidNonempty,3=isLeavesBlock.
    pub fn recompute_heightmaps(&mut self, cx: i32, cz: i32, state_bits: &[u8]) -> Result<()> {
        self.chunk(cx, cz)?.prime_heightmaps(state_bits)
    }
    pub fn chunk_nbt(&mut self, cx: i32, cz: i32) -> Result<&NbtRoot> {
        Ok(&self.chunk(cx, cz)?.root)
    }
    pub fn chunk_packet(&mut self, cx: i32, cz: i32) -> Result<Vec<u8>> {
        let chunk = self.chunk(cx, cz)?;
        ensure!(
            !chunk.lighting_dirty,
            "chunk lighting needs recomputation afteredit"
        );
        ensure!(
            !chunk.heightmaps_dirty,
            "chunk heightmaps need recomputation afteredit"
        );
        ensure!(
            matches!(chunk.root.tag.get("isLightOn"), Some(Tag::Byte(1))),
            "chunk lacks completed vanilla lighting"
        );
        let block_entities = list(
            chunk
                .root
                .tag
                .get("block_entities")
                .context("missing blockentities")?,
        )?;

        let mut out = Vec::new();
        out.extend_from_slice(&cx.to_be_bytes());
        out.extend_from_slice(&cz.to_be_bytes());
        let mut heightmaps = Vec::new();
        for name in ["MOTION_BLOCKING", "WORLD_SURFACE"] {
            let value = chunk
                .root
                .tag
                .get("Heightmaps")
                .and_then(|t| t.get(name))
                .context("required network heightmap missing")?;
            ensure!(
                matches!(value,Tag::LongArray(v) if v.len()==37),
                "invalid overworld heightmap"
            );
            heightmaps.push((name.into(), value.clone()));
        }
        out.extend_from_slice(&Tag::Compound(heightmaps).encode_network_anonymous()?);
        let mut sections = Vec::new();
        for section in &chunk.sections {
            let non_air = section.non_air as i16;
            sections.extend_from_slice(&non_air.to_be_bytes());
            network_palette(&section.states, 4, 256, 15, &mut sections);
            network_palette(&section.biomes, 1, 8, 6, &mut sections);
        }
        varint(sections.len() as u32, &mut out);
        out.extend_from_slice(&sections);
        network_block_entities(block_entities, chunk, cx, cz, &mut out)?;
        out.extend_from_slice(&self.light_data(cx, cz)?);
        Ok(out)
    }
    fn active_light_sections(&mut self, cx: i32, cz: i32) -> Result<BTreeSet<i32>> {
        // LayerLightSectionStorage allocates bothlight layers for each
        // nonemptysection and its26sectionneighbors. Uniformzero layers are
        // omittedon disk but sent as empty masks; truly absentlayers have
        // neither the data nor the emptymaskbit set.
        let mut active_sections = BTreeSet::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                let nx = cx.checked_add(dx).context("neighborchunkX overflow")?;
                let nz = cz.checked_add(dz).context("neighborchunkZ overflow")?;
                if !self.has_chunk(nx, nz)? {
                    continue;
                }
                for (index, section) in self.chunk(nx, nz)?.sections.iter().enumerate() {
                    if section.non_air > 0 {
                        let sy = MIN_SECTION + index as i32;
                        active_sections.extend([sy - 1, sy, sy + 1]);
                    }
                }
            }
        }
        Ok(active_sections)
    }
    fn light_data(&mut self, cx: i32, cz: i32) -> Result<Vec<u8>> {
        let active_sections = self.active_light_sections(cx, cz)?;
        let chunk = self.chunk(cx, cz)?;
        ensure!(
            !chunk.lighting_dirty,
            "chunk lighting needs recomputation afteredit"
        );
        ensure!(
            matches!(chunk.root.tag.get("isLightOn"), Some(Tag::Byte(1))),
            "chunk lacks completed vanilla lighting"
        );
        let mut out = Vec::new();
        let mut sky = BTreeMap::new();
        let mut block = BTreeMap::new();
        for section in list(chunk.root.tag.get("sections").unwrap())? {
            let sy = match section.get("Y") {
                Some(Tag::Byte(y)) => i32::from(*y),
                _ => continue,
            };
            for (key, map) in [("SkyLight", &mut sky), ("BlockLight", &mut block)] {
                if let Some(Tag::ByteArray(data)) = section.get(key) {
                    ensure!(data.len() == NIBBLE_BYTES, "invalid disk lightarray");
                    map.insert(sy, data.iter().map(|&v| v as u8).collect::<Vec<_>>());
                }
            }
        }
        let mut sky_mask = 0u32;
        let mut block_mask = 0u32;
        let mut empty_sky = 0u32;
        let mut empty_block = 0u32;
        let mut sky_arrays = Vec::new();
        let mut block_arrays = Vec::new();
        // Missing arrays in allocatedlayers are uniformzero. Sections
        // outside the allocation neighborhood remain absent on the wire.
        for index in 0..LIGHT_SECTION_COUNT {
            let sy = MIN_SECTION - 1 + index as i32;
            if !active_sections.contains(&sy) && !sky.contains_key(&sy) && !block.contains_key(&sy)
            {
                continue;
            }
            let sky_data = sky
                .get(&sy)
                .cloned()
                .unwrap_or_else(|| vec![0; NIBBLE_BYTES]);
            let block_data = block
                .get(&sy)
                .cloned()
                .unwrap_or_else(|| vec![0; NIBBLE_BYTES]);
            if sky_data.iter().all(|&v| v == 0) {
                empty_sky |= 1 << index;
            } else {
                sky_mask |= 1 << index;
                sky_arrays.push(sky_data);
            }
            if block_data.iter().all(|&v| v == 0) {
                empty_block |= 1 << index;
            } else {
                block_mask |= 1 << index;
                block_arrays.push(block_data);
            }
        }
        for bits in [sky_mask, block_mask, empty_sky, empty_block] {
            bitset(bits, &mut out);
        }
        for arrays in [sky_arrays, block_arrays] {
            varint(arrays.len() as u32, &mut out);
            for data in arrays {
                varint(data.len() as u32, &mut out);
                out.extend_from_slice(&data);
            }
        }
        Ok(out)
    }
    /// Clientbound LightUpdate payload (protocol767packet0x2a), excludingID.
    pub fn light_packet(&mut self, cx: i32, cz: i32) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        varint(cx as u32, &mut out);
        varint(cz as u32, &mut out);
        out.extend_from_slice(&self.light_data(cx, cz)?);
        Ok(out)
    }
    /// Save only modified chunks, preserving every unmodified region sector.
    /// Java regenerates absent heightmaps and recomputes isLightOn=false data.
    pub fn save(&mut self) -> Result<usize> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32;
        let dirty = self
            .chunks
            .iter()
            .filter(|(_, c)| c.dirty)
            .map(|(&key, _)| key)
            .collect::<Vec<_>>();
        let mut regions = BTreeSet::new();
        for &(cx, cz) in &dirty {
            let chunk = &self.chunks[&(cx, cz)];
            let mut root = chunk.root.clone();
            // A Java relight can reuse supplied arrays even whenisLightOn is
            // false; retaining obsoletevalues can prevent decreases. Do not
            // persist arrays that thisedit has invalidated.
            if chunk.lighting_dirty {
                if let Some(Tag::List { values, .. }) = root.tag.get_mut("sections") {
                    for section in values {
                        if let Tag::Compound(entries) = section {
                            entries.retain(|(name, _)| name != "SkyLight" && name != "BlockLight");
                        }
                    }
                }
            }
            let region = self
                .region(cx, cz)?
                .context("edited chunk region disappeared")?;
            let compression = region
                .compression(cx, cz)?
                .unwrap_or(ChunkCompression::Zlib);
            region.insert_chunk(cx, cz, &root, compression, timestamp)?;
            regions.insert(region_coordinates(cx, cz));
        }
        for key in regions {
            let path = self
                .path
                .join("region")
                .join(format!("r.{}.{}.mca", key.0, key.1));
            self.regions
                .get_mut(&key)
                .and_then(Option::as_mut)
                .unwrap()
                .save_atomic(&path)?;
            for (&coordinates, chunk) in &mut self.chunks {
                if region_coordinates(coordinates.0, coordinates.1) == key {
                    chunk.dirty = false;
                }
            }
        }
        Ok(dirty.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_java_heightmap_updates_and_packed_columns() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/heightmap-update-vectors.json"))
                .unwrap();
        let masks = fixture["predicate_masks"].as_array().unwrap();
        assert_eq!(masks.len(), catalog().states.len());
        for (state, mask) in masks.iter().enumerate() {
            for kind in 0..4 {
                assert_eq!(
                    heightmap_matches(catalog().height_bits[state], kind),
                    mask.as_u64().unwrap() & (1 << kind) != 0,
                    "predicate state{state}kind{kind}"
                );
            }
        }
        let maps = |raw: &serde_json::Value| {
            Tag::Compound(
                HEIGHTMAP_NAMES
                    .into_iter()
                    .enumerate()
                    .map(|(kind, name)| {
                        let words = raw[kind]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|word| word.as_str().unwrap().parse::<u64>().unwrap() as i64)
                            .collect();
                        (name.into(), Tag::LongArray(words))
                    })
                    .collect(),
            )
        };
        let mut operations = 0_u64;
        let mut block_reads = 0_u64;
        let mut old_full_rebuild_reads = 0_u64;
        for (scene_index, scene) in fixture["scenes"].as_array().unwrap().iter().enumerate() {
            let x = scene["x"].as_i64().unwrap() as i32;
            let z = scene["z"].as_i64().unwrap() as i32;
            let mut chunk = CachedChunk {
                root: NbtRoot {
                    name: "".into(),
                    tag: Tag::Compound(vec![("Heightmaps".into(), maps(&scene["initial_raw"]))]),
                },
                sections: vec![
                    Section {
                        states: vec![0; 4096],
                        biomes: Vec::new(),
                        non_air: 0,
                    };
                    SECTION_COUNT
                ],
                dirty: false,
                lighting_dirty: false,
                heightmaps_dirty: false,
                heightmaps_validated: true,
                light_revision: 0,
            };
            for (offset, state) in scene["initial"].as_array().unwrap().iter().enumerate() {
                let y = offset as i32 + MIN_Y;
                let index = (((y & 15) << 8) | (z << 4) | x) as usize;
                chunk.sections[offset / 16].states[index] = state.as_u64().unwrap() as u32;
            }
            // Empty initial scenes share the same full prime. The remaining
            // scenes exercise unchanged Java priming for random real columns.
            if scene_index == 0
                || scene["initial"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|state| state.as_u64() != Some(0))
            {
                chunk.prime_heightmaps(&catalog().height_bits).unwrap();
                assert_eq!(
                    chunk.root.tag.get("Heightmaps").unwrap(),
                    &maps(&scene["initial_raw"]),
                    "prime scene{scene_index}"
                );
            }
            let mut heights = [MIN_Y; 4];
            for (kind, height) in heights.iter_mut().enumerate() {
                *height = scene["initial_heights"][kind].as_i64().unwrap() as i32;
            }
            for (step_index, step) in scene["steps"].as_array().unwrap().iter().enumerate() {
                let y = step[0].as_i64().unwrap() as i32;
                let state = step[1].as_u64().unwrap() as u32;
                chunk.sections[((y - MIN_Y) / 16) as usize].states
                    [(((y & 15) << 8) | (z << 4) | x) as usize] = state;
                let mut changed_mask = 0_u8;
                for (kind, height) in heights.iter_mut().enumerate() {
                    let mut reads = 0_u64;
                    let (new_height, changed) = update_heightmap_column(
                        *height,
                        y,
                        catalog().height_bits[state as usize],
                        kind,
                        |below| {
                            reads += 1;
                            catalog().height_bits[chunk.block(x, below, z) as usize]
                        },
                    );
                    assert_eq!(
                        new_height,
                        step[3][kind].as_i64().unwrap() as i32,
                        "height scene{scene_index}step{step_index}kind{kind}"
                    );
                    assert_eq!(
                        reads,
                        step[4][kind].as_u64().unwrap(),
                        "queries scene{scene_index}step{step_index}kind{kind}"
                    );
                    if changed {
                        changed_mask |= 1 << kind;
                    }
                    *height = new_height;
                    block_reads += reads;
                    // Exact former full-rebuild work for this oracle's one
                    // populated column and 255 entirely empty columns.
                    old_full_rebuild_reads += 255 * 384
                        + if new_height == MIN_Y {
                            384
                        } else {
                            (MAX_Y - new_height + 2) as u64
                        };
                }
                assert_eq!(
                    u64::from(changed_mask),
                    step[2].as_u64().unwrap(),
                    "changed mask scene{scene_index}step{step_index}"
                );
                assert!(chunk.update_heightmaps(x, y, z, state).unwrap());
                let index = (z * 16 + x) as usize;
                for (kind, name) in HEIGHTMAP_NAMES.into_iter().enumerate() {
                    let Some(Tag::LongArray(words)) =
                        chunk.root.tag.get("Heightmaps").unwrap().get(name)
                    else {
                        panic!("missing heightmap")
                    };
                    assert_eq!(
                        ((words[index / HEIGHTMAP_VALUES_PER_LONG] as u64
                            >> ((index % HEIGHTMAP_VALUES_PER_LONG) * HEIGHTMAP_BITS))
                            & 0x1ff) as i32
                            + MIN_Y,
                        heights[kind],
                        "packed height scene{scene_index}step{step_index}kind{kind}"
                    );
                }
                operations += 1;
            }
            assert_eq!(
                chunk.root.tag.get("Heightmaps").unwrap(),
                &maps(&scene["final_raw"]),
                "final packed maps scene{scene_index}"
            );
        }
        println!(
            "HEIGHTMAP_CONFORMANCE:{}",
            serde_json::json!({
                "ok":true,"minecraft":"1.21","scenes":fixture["scenes"].as_array().unwrap().len(),
                "edits":operations,"update_calls":operations*4,"predicate_tests":masks.len()*4,
                "incremental_column_block_queries":block_reads,
                "former_full_rebuild_block_queries_for_same_scenes":old_full_rebuild_reads,
                "differences":0,
                "scope":"actual Java four heightmap predicates, original update return values/heights/querycounts, priming, all256packedcolumnindices; workcounts exclude cold priming and are not wall-clock or full-server benchmarks"
            })
        );
    }

    #[test]
    fn imported_stale_or_invalid_heightmaps_are_primed_before_incremental_use() {
        let mut world = revision_test_world();
        let stale = vec![384_u32; 256];
        let chunk = world.chunks.get_mut(&(0, 0)).unwrap();
        chunk
            .root
            .tag
            .insert(
                "Heightmaps",
                Tag::Compound(
                    HEIGHTMAP_NAMES
                        .into_iter()
                        .map(|name| (name.into(), Tag::LongArray(pack(&stale, HEIGHTMAP_BITS))))
                        .collect(),
                ),
            )
            .unwrap();
        assert!(!chunk.heightmaps_validated);
        let stone = catalog().blocks["stone"].default_state;
        world.set_block(15, 0, 15, stone).unwrap();
        let chunk = world.chunks.get_mut(&(0, 0)).unwrap();
        assert!(chunk.heightmaps_validated);
        let mut expected = vec![0_u32; 256];
        expected[255] = 65;
        for name in HEIGHTMAP_NAMES {
            assert_eq!(
                chunk.root.tag.get("Heightmaps").unwrap().get(name),
                Some(&Tag::LongArray(pack(&expected, HEIGHTMAP_BITS)))
            );
        }
        // A later internal malformed-map mutation must also request a prime,
        // rather than allowing some maps to advance while another stays stale.
        chunk
            .root
            .tag
            .get_mut("Heightmaps")
            .unwrap()
            .insert(
                "WORLD_SURFACE",
                Tag::LongArray(vec![0; HEIGHTMAP_LONGS - 1]),
            )
            .unwrap();
        world.set_block(15, 1, 15, stone).unwrap();
        expected[255] = 66;
        let chunk = world.chunks.get(&(0, 0)).unwrap();
        assert!(chunk.heightmaps_validated);
        assert!(!chunk.heightmaps_dirty);
        for name in HEIGHTMAP_NAMES {
            assert_eq!(
                chunk.root.tag.get("Heightmaps").unwrap().get(name),
                Some(&Tag::LongArray(pack(&expected, HEIGHTMAP_BITS)))
            );
        }
    }

    fn revision_test_world() -> VanillaWorld {
        let mut world = VanillaWorld {
            path: PathBuf::new(),
            regions: BTreeMap::new(),
            chunks: BTreeMap::new(),
            light_cache: None,
            light_edits: BTreeMap::new(),
            last_relight_columns: 0,
            pending_light_masks: BTreeMap::new(),
        };
        for z in -2..=2 {
            for x in -2..=2 {
                let root = NbtRoot {
                    name: "".into(),
                    tag: Tag::Compound(vec![
                        ("DataVersion".into(), Tag::Int(3953)),
                        ("xPos".into(), Tag::Int(x)),
                        ("zPos".into(), Tag::Int(z)),
                        ("yPos".into(), Tag::Int(MIN_SECTION)),
                        ("Status".into(), Tag::String("minecraft:full".into())),
                        ("isLightOn".into(), Tag::Byte(1)),
                        (
                            "sections".into(),
                            Tag::List {
                                element_type: 10,
                                values: Vec::new(),
                            },
                        ),
                        (
                            "block_entities".into(),
                            Tag::List {
                                element_type: 10,
                                values: Vec::new(),
                            },
                        ),
                        ("Heightmaps".into(), Tag::Compound(Vec::new())),
                    ]),
                };
                let region_key = region_coordinates(x, z);
                let region = world
                    .regions
                    .entry(region_key)
                    .or_insert_with(|| Some(RegionFile::empty(region_key.0, region_key.1).unwrap()))
                    .as_mut()
                    .unwrap();
                region
                    .insert_chunk(x, z, &root, ChunkCompression::Uncompressed, 0)
                    .unwrap();
                world
                    .chunks
                    .insert((x, z), CachedChunk::decode(root, x, z).unwrap());
            }
        }
        world
    }
    #[test]
    fn light_revision_changes_only_for_actual_light_tags_and_arrays() {
        let mut world = revision_test_world();
        let bytes = vec![0xff; NIBBLE_BYTES];
        assert_eq!(world.light_revision(0, 0).unwrap(), 0);
        world.set_section_light(0, 0, 4, None, None).unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), 0);
        world
            .set_section_light(0, 0, 4, Some(&bytes), None)
            .unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), 1);
        world
            .set_section_light(0, 0, 4, Some(&bytes), None)
            .unwrap();
        world.finish_lighting(0, 0).unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), 1);
        world.set_section_light(0, 0, 4, None, None).unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), 2);
        world.set_section_light(0, 0, 4, None, None).unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), 2);
        let zero = vec![0; NIBBLE_BYTES];
        world.set_section_light(0, 0, 4, Some(&zero), None).unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), 3);
        world
            .recompute_heightmaps(0, 0, &catalog().height_bits)
            .unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), 3);
        world
            .set_block(0, 0, 0, catalog().blocks["stone"].default_state)
            .unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), 3);
    }
    #[test]
    fn unchanged_relighting_keeps_revision_but_empty_allocation_masks_advance_it() {
        let mut world = revision_test_world();
        let stone = catalog().blocks["stone"].default_state;
        // A solid roof makes the isolated bottom section's allocated light
        // layers uniformly zero, so its later removal changes masks alone.
        for chunk in world.chunks.values_mut() {
            chunk.sections[7].states.fill(stone);
            chunk.sections[7].non_air = 4096;
            let tag = disk_blocks(&chunk.sections[7].states);
            chunk
                .section_tag_mut(3)
                .unwrap()
                .insert("block_states", tag)
                .unwrap();
        }
        let center = world.chunks.get_mut(&(0, 0)).unwrap();
        center.sections[0].states[0] = stone;
        center.sections[0].non_air = 1;
        let tag = disk_blocks(&center.sections[0].states);
        center
            .section_tag_mut(-4)
            .unwrap()
            .insert("block_states", tag)
            .unwrap();
        let positions = world.relight_around(0, 0).unwrap();
        let before: Vec<_> = positions
            .iter()
            .map(|&(x, z)| world.light_revision(x, z).unwrap())
            .collect();
        world.relight_around(0, 0).unwrap();
        assert_eq!(
            positions
                .iter()
                .map(|&(x, z)| world.light_revision(x, z).unwrap())
                .collect::<Vec<_>>(),
            before
        );
        world
            .set_block(0, 63, 0, catalog().blocks["dirt"].default_state)
            .unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), before[4]);
        world.relight_around(0, 0).unwrap();
        assert_eq!(
            positions
                .iter()
                .map(|&(x, z)| world.light_revision(x, z).unwrap())
                .collect::<Vec<_>>(),
            before
        );
        let packet_before = world.light_packet(0, 0).unwrap();
        world.set_block(0, MIN_Y, 0, 0).unwrap();
        assert_eq!(world.light_revision(0, 0).unwrap(), before[4]);
        world.relight_around(0, 0).unwrap();
        for (index, &(x, z)) in positions.iter().enumerate() {
            assert_eq!(
                world.light_revision(x, z).unwrap(),
                before[index] + 1,
                "mask change in {x},{z}"
            );
        }
        assert_ne!(world.light_packet(0, 0).unwrap(), packet_before);
    }

    fn canonical(tag: Tag) -> Tag {
        match tag {
            Tag::Compound(entries) => {
                let mut entries = entries
                    .into_iter()
                    .map(|(k, v)| (k, canonical(v)))
                    .collect::<Vec<_>>();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                Tag::Compound(entries)
            }
            Tag::List {
                element_type,
                values,
            } => Tag::List {
                element_type,
                values: values.into_iter().map(canonical).collect(),
            },
            value => value,
        }
    }
    #[test]
    fn actual_java_all_block_entity_update_tags() {
        use base64::Engine;
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../tools/storage_blockentity_vectors.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let decode = |field: &str| {
                NbtRoot::decode(
                    &base64::engine::general_purpose::STANDARD
                        .decode(case[field].as_str().unwrap())
                        .unwrap(),
                )
                .unwrap()
                .tag
            };
            let mut snapshot = decode("full_nbt");
            // Brush direction is live state deliberately omitted by Java's
            // disk saveAdditional. Supplyit from the independently chosen
            // oracle input instead of pretending disk containsit.
            if case["name"] == "minecraft:brushable_block" {
                if let Some(direction) = decode("input_nbt").get("hit_direction") {
                    snapshot.insert("hit_direction", direction.clone()).unwrap();
                }
            }
            snapshot
                .insert("RustPrivateMarker", Tag::String("must not publish".into()))
                .unwrap();
            let actual =
                block_entity_update_tag(&snapshot, case["state_id"].as_u64().unwrap() as u32)
                    .unwrap()
                    .unwrap_or(Tag::Compound(Vec::new()));
            assert_eq!(
                canonical(actual),
                canonical(decode("update_nbt")),
                "{} {}",
                case["name"],
                case["label"]
            );
        }
    }
    #[test]
    fn block_entity_lifecycle_preserves_properties_and_resets_different_blocks() {
        let mut chunk = CachedChunk {
            root: NbtRoot {
                name: "".into(),
                tag: Tag::Compound(vec![(
                    "block_entities".into(),
                    Tag::List {
                        element_type: 10,
                        values: Vec::new(),
                    },
                )]),
            },
            sections: Vec::new(),
            dirty: false,
            lighting_dirty: false,
            heightmaps_dirty: false,
            heightmaps_validated: false,
            light_revision: 0,
        };
        let chest = catalog().blocks["chest"].default_state;
        let rotated =
            block_state_with_properties(chest, &BTreeMap::from([("facing".into(), "east".into())]))
                .unwrap();
        transition_block_entity(&mut chunk, 0, chest, -1, 65, -2).unwrap();
        let Tag::List { values, .. } = chunk.root.tag.get_mut("block_entities").unwrap() else {
            panic!()
        };
        assert_eq!(values.len(), 1);
        values[0]
            .insert("CustomName", Tag::String("preserve me".into()))
            .unwrap();
        values[0]
            .insert("UnknownSavedData", Tag::Long(98127))
            .unwrap();
        transition_block_entity(&mut chunk, chest, rotated, -1, 65, -2).unwrap();
        let values = list(chunk.root.tag.get("block_entities").unwrap()).unwrap();
        assert_eq!(values[0].get("UnknownSavedData"), Some(&Tag::Long(98127)));
        let trapped = catalog().blocks["trapped_chest"].default_state;
        transition_block_entity(&mut chunk, rotated, trapped, -1, 65, -2).unwrap();
        let values = list(chunk.root.tag.get("block_entities").unwrap()).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0].get("id"),
            Some(&Tag::String("minecraft:trapped_chest".into()))
        );
        assert!(values[0].get("UnknownSavedData").is_none());
        transition_block_entity(&mut chunk, trapped, 0, -1, 65, -2).unwrap();
        assert!(list(chunk.root.tag.get("block_entities").unwrap())
            .unwrap()
            .is_empty());
        let oak = catalog().blocks["oak_sign"].default_state;
        let birch = catalog().blocks["birch_sign"].default_state;
        transition_block_entity(&mut chunk, 0, oak, -1, 65, -2).unwrap();
        let Tag::List { values, .. } = chunk.root.tag.get_mut("block_entities").unwrap() else {
            panic!()
        };
        values[0].insert("UnknownSavedData", Tag::Long(1)).unwrap();
        transition_block_entity(&mut chunk, oak, birch, -1, 65, -2).unwrap();
        let values = list(chunk.root.tag.get("block_entities").unwrap()).unwrap();
        assert!(values[0].get("UnknownSavedData").is_none());
        assert_eq!(
            values[0].get("id"),
            Some(&Tag::String("minecraft:sign".into()))
        );
    }
    #[test]
    fn actual_factory_state_specific_defaults_and_null_piston() {
        let state = catalog().blocks["structure_block"].default_state;
        for mode in ["save", "load", "corner", "data"] {
            let state =
                block_state_with_properties(state, &BTreeMap::from([("mode".into(), mode.into())]))
                    .unwrap();
            let entity = block_entity_factory_snapshot(state, -100, 70, 304)
                .unwrap()
                .unwrap();
            assert_eq!(
                entity.get("mode"),
                Some(&Tag::String(mode.to_uppercase().into()))
            );
            for (name, value) in [("x", -100), ("y", 70), ("z", 304)] {
                assert_eq!(entity.get(name), Some(&Tag::Int(value)));
            }
        }
        for state in catalog().blocks["moving_piston"].min_state
            ..=catalog().blocks["moving_piston"].max_state
        {
            assert!(block_entity_factory_snapshot(state, 0, 0, 0)
                .unwrap()
                .is_none());
        }
        let chain = catalog().blocks["chain_command_block"].default_state;
        let impulse = catalog().blocks["command_block"].default_state;
        assert_eq!(
            block_entity_factory_snapshot(chain, 0, 0, 0)
                .unwrap()
                .unwrap()
                .get("auto"),
            Some(&Tag::Byte(1))
        );
        assert_eq!(
            block_entity_factory_snapshot(impulse, 0, 0, 0)
                .unwrap()
                .unwrap()
                .get("auto"),
            Some(&Tag::Byte(0))
        );
    }
    #[test]
    fn public_block_entity_packet_retains_network_nbt_nan_payload() {
        let state = catalog().blocks["structure_block"].default_state;
        let mut snapshot = block_entity_factory_snapshot(state, 0, 64, 0)
            .unwrap()
            .unwrap();
        snapshot
            .insert("integrity", Tag::Float(f32::from_bits(0x7fa1_2345)))
            .unwrap();
        let packet = block_entity_data_packet(&snapshot, state).unwrap();
        // Position is a long followed by a VarInt block-entity type.
        let mut nbt_start = 8;
        while packet[nbt_start] & 0x80 != 0 {
            nbt_start += 1;
        }
        nbt_start += 1;
        let update = Tag::decode_anonymous(&packet[nbt_start..]).unwrap();
        let Some(Tag::Float(integrity)) = update.get("integrity") else {
            panic!("missing public structure-block integrity")
        };
        assert_eq!(integrity.to_bits(), 0x7fa1_2345);
        // Named persisted snapshots retain the DataOutputStream disk contract.
        let persisted = NbtRoot {
            name: "".into(),
            tag: snapshot,
        };
        let decoded = NbtRoot::decode(&persisted.encode().unwrap()).unwrap();
        let Some(Tag::Float(integrity)) = decoded.tag.get("integrity") else {
            panic!("missing persisted structure-block integrity")
        };
        assert_eq!(integrity.to_bits(), 0x7fc0_0000);
    }
    #[test]
    fn all_states_roundtrip_names_and_properties() {
        for (id, tag) in catalog().states.iter().enumerate() {
            assert_eq!(state_from_nbt(tag).unwrap(), id as u32, "state{id}");
        }
    }
    #[test]
    fn packed_palettes_cover_sizes_and_boundaries() {
        for (volume, minimum) in [(4096, 4), (64, 1)] {
            for count in [1, 2, 3, 16, 17, 64, 257, 4096]
                .into_iter()
                .filter(|&n| n <= volume)
            {
                let values = (0..volume).map(|i| (i % count) as u32).collect::<Vec<_>>();
                let (palette, indices) = palette(&values);
                let bits = bits_for_palette(count, minimum);
                let mut entries = vec![(
                    "palette".into(),
                    Tag::List {
                        element_type: 3,
                        values: palette.into_iter().map(|v| Tag::Int(v as i32)).collect(),
                    },
                )];
                if bits > 0 {
                    entries.push(("data".into(), Tag::LongArray(pack(&indices, bits))));
                }
                let decoded = unpack_palette(&Tag::Compound(entries), volume, minimum, |t| {
                    if let Tag::Int(v) = t {
                        Ok(*v as u32)
                    } else {
                        bail!("notint")
                    }
                })
                .unwrap();
                assert_eq!(decoded, values);
            }
        }
    }
}
