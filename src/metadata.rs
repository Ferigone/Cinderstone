//! Java 1.21 player SynchedEntityData fields, defaults and dirty-value packing.
//!
//! Each player inherits 21 fields from Entity, LivingEntity and Player. Full
//! snapshots omit values equal to their registered defaults; dirty snapshots
//! include changed values even when reverting to a default. Complex particle
//! options retain Java identity through Arc, rather than equating wire bytes.

use crate::{
    inventory::ItemStack,
    nbt::Tag,
    protocol::{Packet, Reader, Writer},
};
use anyhow::{bail, ensure, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const PLAYER_SERIALIZERS: [i32; 21] = [
    0, 1, 6, 8, 8, 8, 21, 1, 0, 3, 18, 8, 1, 1, 11, 3, 1, 0, 0, 16, 16,
];

fn float_bits(value: f32) -> u32 {
    if value.is_nan() {
        0x7fc00000
    } else {
        value.to_bits()
    }
}
fn position_bits(x: i32, y: i32, z: i32) -> i64 {
    ((i64::from(x) & 0x3ffffff) << 38) | ((i64::from(z) & 0x3ffffff) << 12) | (i64::from(y) & 0xfff)
}

/// A decoded vanilla particle option. Clones share identity exactly as the
/// immutable ParticleOptions objects in Java's copied metadata lists do.
#[derive(Debug, Clone)]
pub struct Particle {
    id: i32,
    wire: Arc<Vec<u8>>,
}
impl PartialEq for Particle {
    fn eq(&self, other: &Self) -> bool {
        if self.id != other.id {
            return false;
        }
        if self.id == 35 {
            // SculkChargeParticleOptions is a Java float record.
            let left = f32::from_be_bytes(self.wire[self.wire.len() - 4..].try_into().unwrap());
            let right = f32::from_be_bytes(other.wire[other.wire.len() - 4..].try_into().unwrap());
            return float_bits(left) == float_bits(right);
        }
        if !matches!(self.id, 1 | 2 | 13 | 14 | 20 | 28 | 35 | 44 | 45 | 99 | 105) {
            return true;
        }
        Arc::ptr_eq(&self.wire, &other.wire)
    }
}
impl Particle {
    /// Decode all 109 vanilla particle types, preserving item components and
    /// validating the complete option payload. Dust scale uses vanilla clamp.
    pub fn from_wire(bytes: &[u8]) -> Result<Self> {
        let (particle, used) = Self::decode(bytes)?;
        ensure!(used == bytes.len(), "trailing particle option data");
        Ok(particle)
    }
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize)> {
        ensure!(
            bytes.len() <= 1_048_576,
            "particle option exceeds packet budget"
        );
        let mut input = Reader::new(bytes);
        let id = input.varint()?;
        ensure!(
            (0..=108).contains(&id),
            "unknown Minecraft1.21 particle type"
        );
        let mut out = Writer::new();
        out.varint(id);
        match id {
            1 | 2 | 28 | 105 => {
                let state = input.varint()?;
                ensure!((0..26684).contains(&state), "invalid particle block state");
                out.varint(state);
            }
            13 | 14 => {
                for field in 0..if id == 13 { 4 } else { 7 } {
                    let value = input.f32()?;
                    out.f32(if field == 3 {
                        value.clamp(0.01, 4.0)
                    } else {
                        value
                    });
                }
            }
            20 => {
                out.i32(input.i32()?);
            }
            35 => {
                out.f32(input.f32()?);
            }
            44 => {
                let (stack, used) = ItemStack::decode(input.remaining())?;
                input.bytes(used)?;
                out.raw(&stack.encode());
            }
            45 => {
                let source = input.varint()?;
                out.varint(source);
                match source {
                    0 => {
                        out.i64(input.i64()?);
                    }
                    1 => {
                        out.varint(input.varint()?).f32(input.f32()?);
                    }
                    _ => bail!("unknown vibration position source"),
                }
                out.varint(input.varint()?);
            }
            99 => {
                out.varint(input.varint()?);
            }
            _ => {}
        }
        let used = bytes.len() - input.remaining().len();
        Ok((
            Self {
                id,
                wire: Arc::new(out.into_inner()),
            },
            used,
        ))
    }
    pub fn wire(&self) -> &[u8] {
        &self.wire
    }
    pub fn id(&self) -> i32 {
        self.id
    }
}

#[derive(Debug, Clone)]
pub enum MetadataValue {
    Byte(u8),
    Int(i32),
    Float(f32),
    ItemStack(Arc<ItemStack>),
    OptionalComponent(Option<Vec<u8>>),
    Boolean(bool),
    Pose(i32),
    Particles(Vec<Particle>),
    OptionalBlockPos(Option<(i32, i32, i32)>),
    CompoundTag(Arc<Tag>),
}
pub fn normalize_literal_component_payload(bytes: &[u8]) -> Result<Vec<u8>> {
    fn literal(tag: &Tag) -> Option<crate::nbt::JavaString> {
        match tag {
            Tag::String(text) => Some(text.clone()),
            Tag::List { values, .. } if values.len() == 1 => literal(&values[0]),
            Tag::Compound(fields) => {
                if let Some((_, kind)) = fields.iter().rev().find(|(key, _)| key == "type") {
                    if !matches!(kind,Tag::String(name) if name=="text") {
                        return None;
                    }
                }
                let text = match fields.iter().rev().find(|(key, _)| key == "text") {
                    Some((_, Tag::String(text))) => text.clone(),
                    _ => return None,
                };
                for (key, value) in fields {
                    if [
                        "color",
                        "bold",
                        "italic",
                        "underlined",
                        "strikethrough",
                        "obfuscated",
                        "insertion",
                        "clickEvent",
                        "hoverEvent",
                        "font",
                    ]
                    .iter()
                    .any(|name| key == *name)
                    {
                        return None;
                    }
                    if key == "extra" && !matches!(value,Tag::List{values,..} if values.is_empty())
                    {
                        return None;
                    }
                }
                Some(text)
            }
            _ => None,
        }
    }
    fn validate_lists(tag: &Tag) -> Result<()> {
        match tag {
            Tag::List { values, .. } => {
                ensure!(!values.is_empty(), "component list must have contents");
                for value in values {
                    validate_lists(value)?;
                }
            }
            Tag::Compound(fields) => {
                if let Some((_, extra)) = fields.iter().rev().find(|(key, _)| key == "extra") {
                    let Tag::List { values, .. } = extra else {
                        bail!("component extra must be a list");
                    };
                    ensure!(
                        !values.is_empty(),
                        "component extra list must have contents"
                    );
                    for value in values {
                        validate_lists(value)?;
                    }
                }
            }
            Tag::String(_) => {}
            _ => bail!("invalid component representation"),
        }
        Ok(())
    }
    let tag = Tag::decode_anonymous(bytes)?;
    validate_lists(&tag)?;
    if let Some(text) = literal(&tag) {
        Tag::String(text).encode_network_anonymous()
    } else {
        tag.encode_network_anonymous()
    }
}

impl PartialEq for MetadataValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Byte(a), Self::Byte(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => float_bits(*a) == float_bits(*b),
            (Self::ItemStack(a), Self::ItemStack(b)) => Arc::ptr_eq(a, b),
            (Self::Boolean(a), Self::Boolean(b)) => a == b,
            (Self::Pose(a), Self::Pose(b)) => a == b,
            (Self::Particles(a), Self::Particles(b)) => a == b,
            (Self::OptionalBlockPos(a), Self::OptionalBlockPos(b)) => a == b,
            // Literal shorthand is normalized exactly as the Java component
            // codec does. General rich-component logical equality remains a
            // separately documented gap (styles, translated arguments, etc.).
            (Self::OptionalComponent(a), Self::OptionalComponent(b)) => match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => match (
                    normalize_literal_component_payload(a),
                    normalize_literal_component_payload(b),
                ) {
                    (Ok(a), Ok(b)) => a == b,
                    _ => false,
                },
                _ => false,
            },
            (Self::CompoundTag(a), Self::CompoundTag(b)) => Arc::ptr_eq(a, b) || nbt_equal(a, b),
            _ => false,
        }
    }
}
fn nbt_equal(a: &Tag, b: &Tag) -> bool {
    match (a, b) {
        (Tag::Compound(left), Tag::Compound(right)) => {
            let mut lhs = BTreeMap::new();
            let mut rhs = BTreeMap::new();
            for (k, v) in left {
                lhs.insert(k.units().into_owned(), v);
            }
            for (k, v) in right {
                rhs.insert(k.units().into_owned(), v);
            }
            lhs.len() == rhs.len()
                && lhs
                    .iter()
                    .all(|(key, value)| rhs.get(key).is_some_and(|other| nbt_equal(value, other)))
        }
        (Tag::List { values: a, .. }, Tag::List { values: b, .. }) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| nbt_equal(a, b))
        }
        _ => a == b,
    }
}
impl MetadataValue {
    pub fn serializer(&self) -> i32 {
        match self {
            Self::Byte(_) => 0,
            Self::Int(_) => 1,
            Self::Float(_) => 3,
            Self::ItemStack(_) => 7,
            Self::OptionalComponent(_) => 6,
            Self::Boolean(_) => 8,
            Self::OptionalBlockPos(_) => 11,
            Self::CompoundTag(_) => 16,
            Self::Particles(_) => 18,
            Self::Pose(_) => 21,
        }
    }
    /// Decode one complete value using the exact registered player's field
    /// serializer, useful for persisted data and independent oracle vectors.
    pub fn decode_player_field(id: u8, bytes: &[u8]) -> Result<Self> {
        let serializer = *PLAYER_SERIALIZERS
            .get(usize::from(id))
            .ok_or_else(|| anyhow::anyhow!("unknown player metadata field"))?;
        let mut input = Reader::new(bytes);
        let value = match serializer {
            0 => Self::Byte(input.u8()?),
            1 => Self::Int(input.varint()?),
            3 => Self::Float(input.f32()?),
            6 => Self::OptionalComponent(if input.bool()? {
                let remaining = input.remaining().len();
                let bytes = input.bytes(remaining)?.to_vec();
                Tag::decode_anonymous(&bytes)?;
                Some(bytes)
            } else {
                None
            }),
            8 => Self::Boolean(input.bool()?),
            11 => Self::OptionalBlockPos(if input.bool()? {
                let bits = input.i64()?;
                Some((
                    (bits >> 38) as i32,
                    (bits << 52 >> 52) as i32,
                    (bits << 26 >> 38) as i32,
                ))
            } else {
                None
            }),
            16 => {
                let remaining = input.remaining().len();
                let tag = Tag::decode_anonymous(input.bytes(remaining)?)?;
                ensure!(
                    matches!(&tag, Tag::Compound(_)),
                    "shoulder metadata requires a compound"
                );
                Self::CompoundTag(Arc::new(tag))
            }
            18 => {
                let count = input.varint()?;
                ensure!(
                    count >= 0 && count as usize <= input.remaining().len(),
                    "invalid particle list size"
                );
                let mut particles = Vec::new();
                for _ in 0..count {
                    let (particle, used) = Particle::decode(input.remaining())?;
                    input.bytes(used)?;
                    particles.push(particle);
                }
                Self::Particles(particles)
            }
            21 => Self::Pose(input.varint()?),
            _ => unreachable!("registered player metadata serializer"),
        };
        input.finish()?;
        value.payload()?;
        Ok(value)
    }

    pub fn payload(&self) -> Result<Vec<u8>> {
        let mut out = Writer::new();
        match self {
            Self::Byte(v) => {
                out.u8(*v);
            }
            Self::Int(v) => {
                out.varint(*v);
            }
            Self::Float(v) => {
                out.f32(*v);
            }
            Self::ItemStack(stack) => {
                out.raw(&stack.encode());
            }
            Self::OptionalComponent(v) => {
                out.bool(v.is_some());
                if let Some(bytes) = v {
                    ensure!(
                        bytes.len() <= 1_048_576,
                        "metadata component exceeds budget"
                    );
                    out.raw(&normalize_literal_component_payload(bytes)?);
                }
            }
            Self::Boolean(v) => {
                out.bool(*v);
            }
            Self::Pose(v) => {
                ensure!((0..=17).contains(v), "unknown Minecraft1.21 pose");
                out.varint(*v);
            }
            Self::Particles(values) => {
                ensure!(values.len() <= 16_384, "too many metadata particles");
                out.varint(values.len() as i32);
                for value in values {
                    out.raw(value.wire());
                }
            }
            Self::OptionalBlockPos(v) => {
                out.bool(v.is_some());
                if let Some((x, y, z)) = v {
                    out.i64(position_bits(*x, *y, *z));
                }
            }
            Self::CompoundTag(tag) => {
                ensure!(
                    matches!(&**tag, Tag::Compound(_)),
                    "shoulder metadata requires a compound"
                );
                out.raw(&tag.encode_network_anonymous()?);
            }
        }
        Ok(out.into_inner())
    }
}

/// Generic metadata packet builder for non-player entity types. Field order
/// matches SynchedEntityData's ascending index order; duplicates are invalid.
pub fn encode_values(entity_id: i32, entries: &[(u8, MetadataValue)]) -> Result<Option<Packet>> {
    ensure!(entity_id > 0, "entity ID must be positive");
    let mut sorted = BTreeMap::new();
    for (id, value) in entries {
        ensure!(*id < 255, "metadata field255 is a terminator");
        ensure!(
            sorted.insert(*id, value).is_none(),
            "duplicate metadata field"
        );
    }
    if sorted.is_empty() {
        return Ok(None);
    }
    let mut out = Writer::new();
    out.varint(entity_id);
    for (id, value) in sorted {
        out.u8(id).varint(value.serializer()).raw(&value.payload()?);
    }
    out.u8(255);
    Ok(Some(Packet {
        id: 0x58,
        payload: out.into_inner(),
    }))
}

#[derive(Debug, Clone)]
pub struct PlayerEntityData {
    defaults: Vec<MetadataValue>,
    values: Vec<MetadataValue>,
    dirty: BTreeSet<u8>,
}
impl PartialEq for PlayerEntityData {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}
impl Default for PlayerEntityData {
    fn default() -> Self {
        let empty = || MetadataValue::CompoundTag(Arc::new(Tag::Compound(Vec::new())));
        let defaults = vec![
            MetadataValue::Byte(0),
            MetadataValue::Int(300),
            MetadataValue::OptionalComponent(None),
            MetadataValue::Boolean(false),
            MetadataValue::Boolean(false),
            MetadataValue::Boolean(false),
            MetadataValue::Pose(0),
            MetadataValue::Int(0),
            MetadataValue::Byte(0),
            MetadataValue::Float(1.0),
            MetadataValue::Particles(Vec::new()),
            MetadataValue::Boolean(false),
            MetadataValue::Int(0),
            MetadataValue::Int(0),
            MetadataValue::OptionalBlockPos(None),
            MetadataValue::Float(0.0),
            MetadataValue::Int(0),
            MetadataValue::Byte(0),
            MetadataValue::Byte(1),
            empty(),
            empty(),
        ];
        Self {
            values: defaults.clone(),
            defaults,
            dirty: BTreeSet::new(),
        }
    }
}
impl PlayerEntityData {
    /// Registered Player defaults plus the constructor's health=20 mutation.
    pub fn new_player() -> Self {
        let mut data = Self::default();
        data.set(9, MetadataValue::Float(20.0), false)
            .expect("valid player health");
        data
    }
    pub fn clear_dirty(&mut self) {
        self.dirty.clear();
    }
    /// Snapshot recovery compares every inherited field, while retaining the
    /// shared identity of particle options across ordinary state clones.
    pub fn difference_packet(&self, entity_id: i32, previous: &Self) -> Result<Option<Packet>> {
        self.encode(
            entity_id,
            self.values
                .iter()
                .zip(&previous.values)
                .enumerate()
                .filter_map(|(id, (a, b))| (a != b).then_some(id as u8)),
        )
    }
    pub fn get(&self, id: u8) -> Option<&MetadataValue> {
        self.values.get(usize::from(id))
    }
    pub fn is_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }
    pub fn set(&mut self, id: u8, value: MetadataValue, force: bool) -> Result<bool> {
        let index = usize::from(id);
        ensure!(index < 21, "unknown player metadata field");
        ensure!(
            value.serializer() == PLAYER_SERIALIZERS[index],
            "wrong serializer for player metadata field"
        );
        value.payload()?;
        let changed = force || self.values[index] != value;
        if changed {
            self.values[index] = value;
            self.dirty.insert(id);
        }
        Ok(changed)
    }
    fn encode(&self, entity_id: i32, ids: impl Iterator<Item = u8>) -> Result<Option<Packet>> {
        ensure!(entity_id > 0, "player entity ID must be positive");
        let mut out = Writer::new();
        out.varint(entity_id);
        let mut count = 0;
        for id in ids {
            let value = &self.values[usize::from(id)];
            out.u8(id).varint(value.serializer()).raw(&value.payload()?);
            count += 1;
        }
        if count == 0 {
            return Ok(None);
        }
        out.u8(0xff);
        Ok(Some(Packet {
            id: 0x58,
            payload: out.into_inner(),
        }))
    }
    pub fn packet_non_defaults(&self, entity_id: i32) -> Result<Option<Packet>> {
        self.encode(
            entity_id,
            self.values
                .iter()
                .zip(&self.defaults)
                .enumerate()
                .filter_map(|(id, (a, b))| (a != b).then_some(id as u8)),
        )
    }
    pub fn pack_dirty(&mut self, entity_id: i32) -> Result<Option<Packet>> {
        let packet = self.encode(entity_id, self.dirty.iter().copied())?;
        self.dirty.clear();
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    #[test]
    fn registered_player_defaults_and_constructor_health_are_distinct() {
        let mut defaults = PlayerEntityData::default();
        assert!(!defaults.is_dirty());
        assert!(defaults.packet_non_defaults(17).unwrap().is_none());
        assert!(defaults.pack_dirty(17).unwrap().is_none());
        let mut player = PlayerEntityData::new_player();
        let snapshot = player.packet_non_defaults(17).unwrap().unwrap();
        assert_eq!(snapshot.payload, vec![17, 9, 3, 0x41, 0xa0, 0, 0, 255]);
        assert!(player.pack_dirty(17).unwrap().is_some());
        assert!(!player.is_dirty());
    }
    #[test]
    fn float_and_particle_equality_match_java_object_semantics() {
        assert_ne!(MetadataValue::Float(0.0), MetadataValue::Float(-0.0));
        assert_eq!(
            MetadataValue::Float(f32::from_bits(0x7fc00001)),
            MetadataValue::Float(f32::from_bits(0xffc00000))
        );
        let a = Particle::from_wire(&[20, 0, 0xff, 0, 0xff]).unwrap();
        let b = Particle::from_wire(a.wire()).unwrap();
        assert_ne!(a, b);
        assert_eq!(a, a.clone());
        assert_eq!(
            Particle::from_wire(&[0]).unwrap(),
            Particle::from_wire(&[0]).unwrap()
        );
        let sculk = Particle::from_wire(&[35, 0x3f, 0x80, 0, 0]).unwrap();
        assert_eq!(sculk, Particle::from_wire(sculk.wire()).unwrap());
    }
    #[test]
    fn compound_metadata_retains_network_nbt_nan_payloads() {
        let input = vec![
            10, 5, 0, 1, b'f', 0x7f, 0xa1, 0x23, 0x45, 6, 0, 1, b'd', 0xff, 0xf8, 0x12, 0x34, 0x56,
            0x78, 0x9a, 0xbc, 0,
        ];
        let tag = Tag::decode_anonymous(&input).unwrap();
        let metadata = MetadataValue::CompoundTag(Arc::new(tag));
        assert_eq!(metadata.payload().unwrap(), input);
        let decoded = MetadataValue::decode_player_field(19, &input).unwrap();
        assert_eq!(decoded.payload().unwrap(), input);
    }
    #[test]
    fn invalid_player_fields_do_not_mutate_or_clear_dirty_state() {
        let mut data = PlayerEntityData::new_player();
        assert!(data.set(21, MetadataValue::Byte(0), false).is_err());
        assert!(data.set(9, MetadataValue::Int(20), false).is_err());
        assert!(data.set(6, MetadataValue::Pose(18), false).is_err());
        assert!(data.is_dirty());
        assert!(data.pack_dirty(0).is_err());
        assert!(data.is_dirty());
        assert!(data.pack_dirty(17).unwrap().is_some());
    }
    #[test]
    fn dirty_and_non_default_packets_match_actual_player_metadata_classes() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tools/metadata-player.json")).unwrap();
        assert_eq!(
            fixture["jar_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 803);
        let mut data = PlayerEntityData::default();
        let mut previous = None;
        for (index, case) in cases.iter().enumerate() {
            let mut changed = false;
            let op = case["op"].as_str().unwrap();
            match op {
                "new" => {
                    data = PlayerEntityData::default();
                    previous = None;
                }
                "set" | "reuse" => {
                    let id = case["id"].as_u64().unwrap() as u8;
                    let value = if op == "set" {
                        MetadataValue::decode_player_field(
                            id,
                            &STANDARD.decode(case["payload"].as_str().unwrap()).unwrap(),
                        )
                        .unwrap()
                    } else {
                        previous.clone().unwrap()
                    };
                    previous = Some(value.clone());
                    changed = data
                        .set(id, value, case["force"].as_bool().unwrap())
                        .unwrap();
                }
                "drain" => {}
                _ => panic!("unknown metadata oracle operation"),
            }
            let expected = &case["expected"];
            assert_eq!(
                changed,
                expected["changed"].as_bool().unwrap(),
                "metadata case{index}: {case}"
            );
            assert_eq!(
                data.is_dirty(),
                expected["is_dirty"].as_bool().unwrap(),
                "metadata case{index}"
            );
            let compare = |observed: Option<Packet>, key: &str| {
                let wanted = expected[key]
                    .as_str()
                    .map(|text| STANDARD.decode(text).unwrap());
                assert_eq!(
                    observed.map(|p| p.payload),
                    wanted,
                    "metadata case{index} {key}: {case}"
                );
            };
            compare(data.packet_non_defaults(17).unwrap(), "snapshot");
            if op == "drain" {
                compare(data.pack_dirty(17).unwrap(), "dirty");
            }
            if op == "new" {
                for field in expected["schema"].as_array().unwrap() {
                    let id = field["id"].as_u64().unwrap() as usize;
                    assert_eq!(
                        PLAYER_SERIALIZERS[id],
                        field["serializer"].as_i64().unwrap() as i32
                    );
                }
            }
        }
    }
}
