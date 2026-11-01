//! Bounded Java-compatible NBT primitives for disk and anonymous network documents.
//!
//! Strings use Java's Modified UTF-8 and retain every UTF-16 code unit,
//! including isolated surrogates that cannot be represented by a Rust String.
//! Compound order and duplicate names are retained; lookups use the last name,
//! matching the logical result of Java CompoundTag insertion.
//! Float/Double tag factories normalize either signed zero to positive zero,
//! including binary reads, matching Minecraft FloatTag/DoubleTag.valueOf. Other
//! payload bits are retained on decode. Disk encoding canonicalizes every NaN
//! like Java DataOutputStream; explicit network encoding retains NaN payloads
//! like Netty ByteBufOutputStream. Nonzero finite values retain their exact bits.
//! Limits bound encoded bytes, estimated retained allocation, container nesting,
//! and total tags/array cells before allocating attacker-controlled lengths.

use anyhow::{anyhow, bail, Context, Result};
use std::{
    borrow::Cow,
    cmp::Ordering,
    hash::{Hash, Hasher},
    mem::size_of,
};

/// A Java String with no lossy conversion of isolated UTF-16 surrogates.
#[derive(Clone, Debug)]
pub enum JavaString {
    Utf8(String),
    Utf16(Vec<u16>),
}

impl JavaString {
    pub fn from_utf16_lossless(units: Vec<u16>) -> Self {
        match String::from_utf16(&units) {
            Ok(text) => Self::Utf8(text),
            Err(_) => Self::Utf16(units),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Utf8(text) => Some(text),
            Self::Utf16(_) => None,
        }
    }

    pub fn units(&self) -> Cow<'_, [u16]> {
        match self {
            Self::Utf8(text) => Cow::Owned(text.encode_utf16().collect()),
            Self::Utf16(units) => Cow::Borrowed(units),
        }
    }

    pub fn iter_units(&self) -> JavaUnits<'_> {
        match self {
            Self::Utf8(text) => JavaUnits::Utf8(text.encode_utf16()),
            Self::Utf16(units) => JavaUnits::Utf16(units.iter()),
        }
    }

    pub fn len_utf16(&self) -> usize {
        self.iter_units().count()
    }
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Utf8(text) => text.is_empty(),
            Self::Utf16(units) => units.is_empty(),
        }
    }

    /// For diagnostics only. Encoding never calls this lossy conversion.
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        match self {
            Self::Utf8(text) => Cow::Borrowed(text),
            Self::Utf16(units) => Cow::Owned(String::from_utf16_lossy(units)),
        }
    }
}

pub enum JavaUnits<'a> {
    Utf8(std::str::EncodeUtf16<'a>),
    Utf16(std::slice::Iter<'a, u16>),
}
impl Iterator for JavaUnits<'_> {
    type Item = u16;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Utf8(iter) => iter.next(),
            Self::Utf16(iter) => iter.next().copied(),
        }
    }
}
impl From<String> for JavaString {
    fn from(value: String) -> Self {
        Self::Utf8(value)
    }
}
impl From<&str> for JavaString {
    fn from(value: &str) -> Self {
        Self::Utf8(value.into())
    }
}
impl Default for JavaString {
    fn default() -> Self {
        Self::Utf8(String::new())
    }
}
impl PartialEq for JavaString {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Utf8(left), Self::Utf8(right)) => left == right,
            _ => self.iter_units().eq(other.iter_units()),
        }
    }
}
impl Eq for JavaString {}
impl PartialEq<str> for JavaString {
    fn eq(&self, other: &str) -> bool {
        self.iter_units().eq(other.encode_utf16())
    }
}
impl PartialEq<&str> for JavaString {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl PartialEq<String> for JavaString {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}
impl PartialOrd for JavaString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for JavaString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter_units().cmp(other.iter_units())
    }
}
impl Hash for JavaString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for unit in self.iter_units() {
            unit.hash(state);
        }
        // All representations use the same unit sequence and terminator.
        0x1_0000_u32.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(JavaString),
    List { element_type: u8, values: Vec<Tag> },
    Compound(Vec<(JavaString, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    pub fn tag_id(&self) -> u8 {
        match self {
            Self::Byte(_) => 1,
            Self::Short(_) => 2,
            Self::Int(_) => 3,
            Self::Long(_) => 4,
            Self::Float(_) => 5,
            Self::Double(_) => 6,
            Self::ByteArray(_) => 7,
            Self::String(_) => 8,
            Self::List { .. } => 9,
            Self::Compound(_) => 10,
            Self::IntArray(_) => 11,
            Self::LongArray(_) => 12,
        }
    }

    pub fn get(&self, name: &str) -> Option<&Tag> {
        match self {
            Self::Compound(entries) => entries
                .iter()
                .rev()
                .find(|(key, _)| key == name)
                .map(|(_, tag)| tag),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Tag> {
        match self {
            Self::Compound(entries) => entries
                .iter_mut()
                .rev()
                .find(|(key, _)| key == name)
                .map(|(_, tag)| tag),
            _ => None,
        }
    }

    pub fn insert(&mut self, name: impl Into<JavaString>, tag: Tag) -> Result<Option<Tag>> {
        let name = name.into();
        let Self::Compound(entries) = self else {
            bail!("NBT insertion requires a compound");
        };
        if let Some((_, old)) = entries.iter_mut().rev().find(|(key, _)| key == &name) {
            return Ok(Some(std::mem::replace(old, tag)));
        }
        entries.push((name, tag));
        Ok(None)
    }

    pub fn decode_anonymous(bytes: &[u8]) -> Result<Self> {
        Self::decode_anonymous_with_limits(bytes, Limits::default())
    }

    /// Decode one anonymous network tag embedded in a larger packet. Bytes
    /// following that tag belong to the caller and are not consumed or charged.
    pub fn decode_anonymous_prefix(bytes: &[u8]) -> Result<(Self, usize)> {
        Self::decode_anonymous_prefix_with_limits(bytes, Limits::default())
    }
    pub fn decode_anonymous_prefix_with_limits(
        bytes: &[u8],
        limits: Limits,
    ) -> Result<(Self, usize)> {
        let limits = limits.validate()?;
        let mut reader = Reader::new(&bytes[..bytes.len().min(limits.max_bytes)], limits)?;
        let id = reader.byte()?;
        let tag = reader.payload(id, 0)?;
        Ok((tag, reader.position))
    }

    pub fn decode_anonymous_with_limits(bytes: &[u8], limits: Limits) -> Result<Self> {
        let mut reader = Reader::new(bytes, limits)?;
        let id = reader.byte()?;
        let tag = reader.payload(id, 0)?;
        reader.finish()?;
        Ok(tag)
    }

    pub fn encode_anonymous(&self) -> Result<Vec<u8>> {
        self.encode_anonymous_with_limits(Limits::default())
    }

    pub fn encode_anonymous_with_limits(&self, limits: Limits) -> Result<Vec<u8>> {
        self.encode_anonymous_in_mode(limits, WriterMode::Disk)
    }

    /// Encode the anonymous NBT used by FriendlyByteBuf/Netty. Unlike the disk
    /// DataOutputStream path, floating NaN payload bits are retained. Both paths
    /// apply Minecraft's FloatTag/DoubleTag factory normalization of signed zero.
    pub fn encode_network_anonymous(&self) -> Result<Vec<u8>> {
        self.encode_network_anonymous_with_limits(Limits::default())
    }

    pub fn encode_network_anonymous_with_limits(&self, limits: Limits) -> Result<Vec<u8>> {
        self.encode_anonymous_in_mode(limits, WriterMode::Network)
    }

    fn encode_anonymous_in_mode(&self, limits: Limits, mode: WriterMode) -> Result<Vec<u8>> {
        let mut writer = Writer::new(limits, mode)?;
        writer.byte(self.tag_id())?;
        writer.payload(self, 0)?;
        Ok(writer.bytes)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NbtRoot {
    pub name: JavaString,
    pub tag: Tag,
}

impl NbtRoot {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Self::decode_with_limits(bytes, Limits::default())
    }

    pub fn decode_with_limits(bytes: &[u8], limits: Limits) -> Result<Self> {
        let mut reader = Reader::new(bytes, limits)?;
        let id = reader.byte()?;
        if id == 0 {
            bail!("TAG_End cannot be a named root");
        }
        let name = reader.string()?;
        let tag = reader.payload(id, 0)?;
        reader.finish()?;
        Ok(Self { name, tag })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.encode_with_limits(Limits::default())
    }

    pub fn encode_with_limits(&self, limits: Limits) -> Result<Vec<u8>> {
        let mut writer = Writer::new(limits, WriterMode::Disk)?;
        writer.byte(self.tag.tag_id())?;
        writer.string(&self.name)?;
        writer.payload(&self.tag, 0)?;
        Ok(writer.bytes)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum nested List/Compound containers; primitives do not add depth.
    pub max_depth: usize,
    /// Ceiling on encoded input/output and estimated retained decode allocation.
    pub max_bytes: usize,
    /// Maximum aggregate tags and primitive array cells.
    pub max_elements: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 512,
            max_bytes: 64 * 1024 * 1024,
            max_elements: 16 * 1024 * 1024,
        }
    }
}
impl Limits {
    fn validate(self) -> Result<Self> {
        if self.max_depth > 512 {
            bail!("NBT depth may not exceed the Java 1.21 limit of 512");
        }
        if self.max_bytes == 0 || self.max_elements == 0 {
            bail!("NBT byte and element budgets must be positive");
        }
        Ok(self)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    limits: Limits,
    allocation_left: usize,
    elements_left: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        let limits = limits.validate()?;
        if bytes.len() > limits.max_bytes {
            bail!("NBT input exceeds its byte budget");
        }
        Ok(Self {
            bytes,
            position: 0,
            limits,
            allocation_left: limits.max_bytes,
            elements_left: limits.max_elements,
        })
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| anyhow!("NBT byte range overflow"))?;
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| anyhow!("truncated NBT document"))?;
        self.position = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn count(&mut self) -> Result<usize> {
        let count = i32::from_be_bytes(self.take(4)?.try_into()?);
        if count < 0 {
            bail!("negative NBT array/list length");
        }
        Ok(count as usize)
    }
    fn charge_elements(&mut self, count: usize) -> Result<()> {
        self.elements_left = self
            .elements_left
            .checked_sub(count)
            .ok_or_else(|| anyhow!("NBT element budget exceeded"))?;
        Ok(())
    }
    fn charge_allocation(&mut self, bytes: usize) -> Result<()> {
        self.allocation_left = self
            .allocation_left
            .checked_sub(bytes)
            .ok_or_else(|| anyhow!("NBT retained-allocation budget exceeded"))?;
        Ok(())
    }
    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>> {
        self.charge_allocation(
            count
                .checked_mul(size_of::<T>())
                .ok_or_else(|| anyhow!("NBT allocation length overflow"))?,
        )?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .context("allocating bounded NBT collection")?;
        Ok(values)
    }
    fn string(&mut self) -> Result<JavaString> {
        let length = u16::from_be_bytes(self.take(2)?.try_into()?) as usize;
        let bytes = self.take(length)?;
        let stats = mutf8_stats(bytes)?;
        if stats.valid_unicode {
            self.charge_allocation(stats.utf8_bytes)?;
            let mut text = String::new();
            text.try_reserve_exact(stats.utf8_bytes)
                .context("allocating bounded NBT string")?;
            let mut units = Mutf8Reader::new(bytes);
            while let Some(unit) = units.next()? {
                let codepoint = if (0xd800..=0xdbff).contains(&unit) {
                    let low = units
                        .next()?
                        .ok_or_else(|| anyhow!("internal MUTF-8 pair mismatch"))?;
                    0x1_0000 + (((unit as u32 - 0xd800) << 10) | (low as u32 - 0xdc00))
                } else {
                    unit as u32
                };
                text.push(
                    char::from_u32(codepoint)
                        .ok_or_else(|| anyhow!("internal MUTF-8 scalar mismatch"))?,
                );
            }
            Ok(JavaString::Utf8(text))
        } else {
            let mut values = self.reserve::<u16>(stats.units)?;
            let mut units = Mutf8Reader::new(bytes);
            while let Some(unit) = units.next()? {
                values.push(unit);
            }
            Ok(JavaString::Utf16(values))
        }
    }
    fn finish(&self) -> Result<()> {
        if self.position != self.bytes.len() {
            bail!("trailing bytes after NBT document");
        }
        Ok(())
    }
    fn container_depth(&self, depth: usize) -> Result<usize> {
        if depth >= self.limits.max_depth {
            bail!("NBT container depth budget exceeded");
        }
        Ok(depth + 1)
    }
    fn array_count(&mut self, width: usize) -> Result<usize> {
        let count = self.count()?;
        self.charge_elements(count)?;
        let bytes = count
            .checked_mul(width)
            .ok_or_else(|| anyhow!("NBT array byte length overflow"))?;
        if bytes > self.bytes.len() - self.position {
            bail!("truncated NBT primitive array");
        }
        Ok(count)
    }
    fn payload(&mut self, id: u8, depth: usize) -> Result<Tag> {
        self.charge_elements(1)?;
        Ok(match id {
            1 => Tag::Byte(self.byte()? as i8),
            2 => Tag::Short(i16::from_be_bytes(self.take(2)?.try_into()?)),
            3 => Tag::Int(i32::from_be_bytes(self.take(4)?.try_into()?)),
            4 => Tag::Long(i64::from_be_bytes(self.take(8)?.try_into()?)),
            5 => {
                let value = f32::from_bits(u32::from_be_bytes(self.take(4)?.try_into()?));
                Tag::Float(if value == 0.0 { 0.0 } else { value })
            }
            6 => {
                let value = f64::from_bits(u64::from_be_bytes(self.take(8)?.try_into()?));
                Tag::Double(if value == 0.0 { 0.0 } else { value })
            }
            7 => {
                let count = self.array_count(1)?;
                let mut values = self.reserve::<i8>(count)?;
                values.extend(self.take(count)?.iter().map(|byte| *byte as i8));
                Tag::ByteArray(values)
            }
            8 => Tag::String(self.string()?),
            9 => {
                let nested = self.container_depth(depth)?;
                let element_type = self.byte()?;
                let count = self.count()?;
                if element_type > 12 || (element_type == 0 && count != 0) {
                    bail!("invalid NBT list element type");
                }
                if count > self.elements_left {
                    bail!("NBT list exceeds element budget");
                }
                // Each nonempty payload occupies at least one encoded byte.
                if count > self.bytes.len() - self.position {
                    bail!("truncated NBT list");
                }
                let mut values = self.reserve::<Tag>(count)?;
                for _ in 0..count {
                    values.push(self.payload(element_type, nested)?);
                }
                Tag::List {
                    element_type,
                    values,
                }
            }
            10 => {
                let nested = self.container_depth(depth)?;
                let mut values = Vec::new();
                loop {
                    let kind = self.byte()?;
                    if kind == 0 {
                        break;
                    }
                    if kind > 12 {
                        bail!("invalid NBT compound tag type {kind}");
                    }
                    if self.elements_left == 0 {
                        bail!("NBT compound exceeds element budget");
                    }
                    if values.len() == values.capacity() {
                        let width = size_of::<(JavaString, Tag)>();
                        let available = self.allocation_left / width;
                        if available == 0 {
                            bail!("NBT compound exceeds retained-allocation budget");
                        }
                        let extra = values.capacity().max(4).min(available);
                        self.charge_allocation(extra * width)?;
                        values
                            .try_reserve_exact(extra)
                            .context("allocating bounded NBT compound")?;
                    }
                    let name = self.string()?;
                    values.push((name, self.payload(kind, nested)?));
                }
                Tag::Compound(values)
            }
            11 => {
                let count = self.array_count(4)?;
                let mut values = self.reserve::<i32>(count)?;
                for _ in 0..count {
                    values.push(i32::from_be_bytes(self.take(4)?.try_into()?));
                }
                Tag::IntArray(values)
            }
            12 => {
                let count = self.array_count(8)?;
                let mut values = self.reserve::<i64>(count)?;
                for _ in 0..count {
                    values.push(i64::from_be_bytes(self.take(8)?.try_into()?));
                }
                Tag::LongArray(values)
            }
            _ => bail!("invalid NBT tag type {id}"),
        })
    }
}

#[derive(Clone, Copy)]
enum WriterMode {
    Disk,
    Network,
}

struct Writer {
    bytes: Vec<u8>,
    limits: Limits,
    elements_left: usize,
    mode: WriterMode,
}
impl Writer {
    fn new(limits: Limits, mode: WriterMode) -> Result<Self> {
        let limits = limits.validate()?;
        Ok(Self {
            bytes: Vec::new(),
            limits,
            elements_left: limits.max_elements,
            mode,
        })
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<()> {
        let required = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| anyhow!("NBT output byte count overflow"))?;
        if required > self.limits.max_bytes {
            bail!("NBT output exceeds byte budget");
        }
        if required > self.bytes.capacity() {
            let capacity = required
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limits.max_bytes);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .context("allocating bounded NBT output")?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn byte(&mut self, value: u8) -> Result<()> {
        self.raw(&[value])
    }
    fn count(&mut self, length: usize) -> Result<()> {
        let count = i32::try_from(length).context("NBT collection length exceeds signed i32")?;
        self.raw(&count.to_be_bytes())
    }
    fn charge_elements(&mut self, count: usize) -> Result<()> {
        self.elements_left = self
            .elements_left
            .checked_sub(count)
            .ok_or_else(|| anyhow!("NBT element budget exceeded"))?;
        Ok(())
    }
    fn string(&mut self, value: &JavaString) -> Result<()> {
        let mut encoded_length = 0_usize;
        for unit in value.iter_units() {
            encoded_length += match unit {
                1..=0x7f => 1,
                0..=0x7ff => 2,
                _ => 3,
            };
            if encoded_length > u16::MAX as usize {
                bail!("NBT Modified UTF-8 string exceeds 65535 encoded bytes");
            }
        }
        self.raw(&(encoded_length as u16).to_be_bytes())?;
        for unit in value.iter_units() {
            match unit {
                1..=0x7f => self.byte(unit as u8)?,
                0..=0x7ff => {
                    self.raw(&[(0xc0 | (unit >> 6)) as u8, (0x80 | (unit & 0x3f)) as u8])?
                }
                _ => self.raw(&[
                    (0xe0 | (unit >> 12)) as u8,
                    (0x80 | ((unit >> 6) & 0x3f)) as u8,
                    (0x80 | (unit & 0x3f)) as u8,
                ])?,
            }
        }
        Ok(())
    }
    fn container_depth(&self, depth: usize) -> Result<usize> {
        if depth >= self.limits.max_depth {
            bail!("NBT container depth budget exceeded");
        }
        Ok(depth + 1)
    }
    fn payload(&mut self, tag: &Tag, depth: usize) -> Result<()> {
        self.charge_elements(1)?;
        match tag {
            Tag::Byte(value) => self.byte(*value as u8)?,
            Tag::Short(value) => self.raw(&value.to_be_bytes())?,
            Tag::Int(value) => self.raw(&value.to_be_bytes())?,
            Tag::Long(value) => self.raw(&value.to_be_bytes())?,
            Tag::Float(value) => self.raw(
                &if *value == 0.0 {
                    0
                } else if matches!(self.mode, WriterMode::Disk) && value.is_nan() {
                    0x7fc0_0000_u32
                } else {
                    value.to_bits()
                }
                .to_be_bytes(),
            )?,
            Tag::Double(value) => self.raw(
                &if *value == 0.0 {
                    0
                } else if matches!(self.mode, WriterMode::Disk) && value.is_nan() {
                    0x7ff8_0000_0000_0000_u64
                } else {
                    value.to_bits()
                }
                .to_be_bytes(),
            )?,
            Tag::ByteArray(values) => {
                self.charge_elements(values.len())?;
                self.count(values.len())?;
                for value in values {
                    self.byte(*value as u8)?;
                }
            }
            Tag::String(value) => self.string(value)?,
            Tag::List {
                element_type,
                values,
            } => {
                let nested = self.container_depth(depth)?;
                if *element_type > 12 || (*element_type == 0 && !values.is_empty()) {
                    bail!("invalid NBT list element type");
                }
                if values.len() > self.elements_left {
                    bail!("NBT list exceeds element budget");
                }
                if values.iter().any(|value| value.tag_id() != *element_type) {
                    bail!("heterogeneous NBT list");
                }
                self.byte(*element_type)?;
                self.count(values.len())?;
                for value in values {
                    self.payload(value, nested)?;
                }
            }
            Tag::Compound(entries) => {
                let nested = self.container_depth(depth)?;
                for (name, value) in entries {
                    self.byte(value.tag_id())?;
                    self.string(name)?;
                    self.payload(value, nested)?;
                }
                self.byte(0)?;
            }
            Tag::IntArray(values) => {
                self.charge_elements(values.len())?;
                self.count(values.len())?;
                for value in values {
                    self.raw(&value.to_be_bytes())?;
                }
            }
            Tag::LongArray(values) => {
                self.charge_elements(values.len())?;
                self.count(values.len())?;
                for value in values {
                    self.raw(&value.to_be_bytes())?;
                }
            }
        }
        Ok(())
    }
}

struct Mutf8Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> Mutf8Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }
    fn next(&mut self) -> Result<Option<u16>> {
        let Some(&first) = self.bytes.get(self.cursor) else {
            return Ok(None);
        };
        self.cursor += 1;
        match first {
            0..=0x7f => Ok(Some(first as u16)),
            0xc0..=0xdf => {
                let second = self.continuation()?;
                Ok(Some((((first & 0x1f) as u16) << 6) | second as u16))
            }
            0xe0..=0xef => {
                let second = self.continuation()?;
                let third = self.continuation()?;
                Ok(Some(
                    (((first & 0x0f) as u16) << 12) | ((second as u16) << 6) | third as u16,
                ))
            }
            _ => bail!("invalid Modified UTF-8 leading byte"),
        }
    }
    fn continuation(&mut self) -> Result<u8> {
        let &byte = self
            .bytes
            .get(self.cursor)
            .ok_or_else(|| anyhow!("truncated Modified UTF-8 sequence"))?;
        self.cursor += 1;
        if byte & 0xc0 != 0x80 {
            bail!("invalid Modified UTF-8 continuation byte");
        }
        Ok(byte & 0x3f)
    }
}
struct Mutf8Stats {
    units: usize,
    utf8_bytes: usize,
    valid_unicode: bool,
}
fn mutf8_stats(bytes: &[u8]) -> Result<Mutf8Stats> {
    let mut reader = Mutf8Reader::new(bytes);
    let mut stats = Mutf8Stats {
        units: 0,
        utf8_bytes: 0,
        valid_unicode: true,
    };
    let mut high = false;
    while let Some(unit) = reader.next()? {
        stats.units += 1;
        if high {
            if (0xdc00..=0xdfff).contains(&unit) {
                stats.utf8_bytes += 4;
                high = false;
                continue;
            }
            stats.valid_unicode = false;
        }
        high = (0xd800..=0xdbff).contains(&unit);
        if !high {
            if (0xdc00..=0xdfff).contains(&unit) {
                stats.valid_unicode = false;
            }
            stats.utf8_bytes += match unit {
                0..=0x7f => 1,
                0x80..=0x7ff => 2,
                _ => 3,
            };
        }
    }
    if high {
        stats.valid_unicode = false;
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        text.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn all_tag_types_named_and_anonymous_preserve_wire_payloads() {
        let tag = Tag::Compound(vec![
            ("byte".into(), Tag::Byte(-128)),
            ("short".into(), Tag::Short(i16::MIN)),
            ("int".into(), Tag::Int(i32::MIN)),
            ("long".into(), Tag::Long(i64::MIN)),
            ("float".into(), Tag::Float(-0.0)),
            ("double".into(), Tag::Double(f64::INFINITY)),
            ("bytes".into(), Tag::ByteArray(vec![-128, -1, 0, 1, 127])),
            ("string".into(), Tag::String("A\0😀".into())),
            (
                "list".into(),
                Tag::List {
                    element_type: 3,
                    values: vec![Tag::Int(-1), Tag::Int(0), Tag::Int(i32::MAX)],
                },
            ),
            (
                "compound".into(),
                Tag::Compound(vec![("nested".into(), Tag::Byte(1))]),
            ),
            (
                "ints".into(),
                Tag::IntArray(vec![i32::MIN, -1, 0, i32::MAX]),
            ),
            (
                "longs".into(),
                Tag::LongArray(vec![i64::MIN, -1, 0, i64::MAX]),
            ),
        ]);
        let root = NbtRoot {
            name: "Root\0😀".into(),
            tag: tag.clone(),
        };
        let encoded = root.encode().unwrap();
        assert_eq!(NbtRoot::decode(&encoded).unwrap(), root);
        assert_eq!(
            NbtRoot::decode(&encoded).unwrap().encode().unwrap(),
            encoded
        );
        let anonymous = tag.encode_anonymous().unwrap();
        assert_eq!(Tag::decode_anonymous(&anonymous).unwrap(), tag);
    }

    #[test]
    fn explicit_big_endian_named_compound_vector() {
        let bytes = hex("0a000001000178ff030001798000000000");
        let parsed = NbtRoot::decode(&bytes).unwrap();
        assert_eq!(parsed.name, "");
        assert_eq!(parsed.tag.get("x"), Some(&Tag::Byte(-1)));
        assert_eq!(parsed.tag.get("y"), Some(&Tag::Int(i32::MIN)));
        assert_eq!(parsed.encode().unwrap(), bytes);
    }

    #[test]
    fn compound_order_duplicates_and_last_wins_are_preserved() {
        let mut tag = Tag::Compound(vec![
            ("x".into(), Tag::Int(1)),
            ("z".into(), Tag::Byte(3)),
            ("x".into(), Tag::Int(2)),
        ]);
        assert_eq!(tag.get("x"), Some(&Tag::Int(2)));
        assert_eq!(tag.insert("x", Tag::Int(7)).unwrap(), Some(Tag::Int(2)));
        assert_eq!(tag.get("x"), Some(&Tag::Int(7)));
        let bytes = tag.encode_anonymous().unwrap();
        assert_eq!(Tag::decode_anonymous(&bytes).unwrap(), tag);
        let Tag::Compound(entries) = tag else {
            panic!()
        };
        assert_eq!(entries[0].1, Tag::Int(1));
        assert_eq!(entries[2].1, Tag::Int(7));
    }

    #[test]
    fn modified_utf8_preserves_isolated_surrogates_and_utf16_equality() {
        let values = [
            vec![0xd800],
            vec![0xdc00],
            vec![0xd800, 65, 0xdc00],
            vec![0xd83d, 0xde00],
            vec![0, 0xffff],
        ];
        for units in values {
            let text = JavaString::from_utf16_lossless(units.clone());
            let tag = Tag::String(text);
            let bytes = tag.encode_anonymous().unwrap();
            let Tag::String(decoded) = Tag::decode_anonymous(&bytes).unwrap() else {
                panic!()
            };
            assert_eq!(&*decoded.units(), units);
            assert_eq!(Tag::String(decoded).encode_anonymous().unwrap(), bytes);
        }
        assert_eq!(
            JavaString::Utf16(vec![65, 0xd83d, 0xde00]),
            JavaString::from("A😀")
        );
        assert_eq!(
            Tag::String(JavaString::Utf16(vec![0xd800]))
                .encode_anonymous()
                .unwrap(),
            hex("080003eda080")
        );
    }

    #[test]
    fn actual_jdk_modified_utf8_vectors_match_unit_for_unit() {
        let vectors: serde_json::Value =
            serde_json::from_str(include_str!("../tools/nbt_utf_vectors.json")).unwrap();
        for sample in vectors["writes"].as_array().unwrap() {
            let units: Vec<u16> = sample["units"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect();
            let expected = hex(sample["hex"].as_str().unwrap());
            let encoded = Tag::String(JavaString::from_utf16_lossless(units.clone()))
                .encode_anonymous()
                .unwrap();
            assert_eq!(&encoded[1..], expected);
            let Tag::String(decoded) = Tag::decode_anonymous(&encoded).unwrap() else {
                panic!()
            };
            assert_eq!(&*decoded.units(), units);
        }
        for sample in vectors["reads"].as_array().unwrap() {
            let mut bytes = vec![8];
            bytes.extend(hex(sample["hex"].as_str().unwrap()));
            let parsed = Tag::decode_anonymous(&bytes);
            if sample.get("error").is_some() {
                assert!(parsed.is_err());
            } else {
                let expected: Vec<u16> = sample["units"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u16)
                    .collect();
                let Tag::String(decoded) = parsed.unwrap() else {
                    panic!()
                };
                assert_eq!(&*decoded.units(), expected);
            }
        }
    }

    #[test]
    fn encoded_string_length_is_unsigned_u16_modified_utf8_bytes() {
        let maximum = JavaString::from("\0".repeat(32767) + "x");
        assert_eq!(
            Tag::String(maximum).encode_anonymous().unwrap().len(),
            65538
        );
        assert!(Tag::String("\0".repeat(32768).into())
            .encode_anonymous()
            .is_err());
        assert!(Tag::String(JavaString::Utf16(vec![0xd800; 21846]))
            .encode_anonymous()
            .is_err());
    }

    #[test]
    fn malformed_documents_reject_before_collection_allocations() {
        for bytes in [
            vec![],
            hex("00"),
            hex("0d"),
            hex("0700000002ff"),
            hex("07ffffffff"),
            hex("0b7fffffff"),
            hex("090000000001"),
            hex("090d00000000"),
            hex("0a0d000000"),
        ] {
            assert!(Tag::decode_anonymous(&bytes).is_err(), "{bytes:?}");
        }
        assert!(NbtRoot::decode(&hex("0a00000000")).is_err());
        assert!(Tag::decode_anonymous(&hex("080004f0908080")).is_err());
        assert!(Tag::decode_anonymous(&hex("080002c241")).is_err());
        assert!(Tag::decode_anonymous(&hex("080001c2")).is_err());
    }

    #[test]
    fn container_depth_element_and_allocation_limits_are_independent() {
        let nested = Tag::Compound(vec![(
            "a".into(),
            Tag::List {
                element_type: 10,
                values: vec![Tag::Compound(vec![])],
            },
        )]);
        let encoded = nested.encode_anonymous().unwrap();
        assert!(Tag::decode_anonymous_with_limits(
            &encoded,
            Limits {
                max_depth: 2,
                ..Limits::default()
            }
        )
        .is_err());
        assert_eq!(
            Tag::decode_anonymous_with_limits(
                &encoded,
                Limits {
                    max_depth: 3,
                    ..Limits::default()
                }
            )
            .unwrap(),
            nested
        );
        assert!(nested
            .encode_anonymous_with_limits(Limits {
                max_depth: 2,
                ..Limits::default()
            })
            .is_err());
        let array = Tag::IntArray(vec![1, 2, 3]);
        let bytes = array.encode_anonymous().unwrap();
        assert!(Tag::decode_anonymous_with_limits(
            &bytes,
            Limits {
                max_elements: 3,
                ..Limits::default()
            }
        )
        .is_err());
        assert!(array
            .encode_anonymous_with_limits(Limits {
                max_elements: 3,
                ..Limits::default()
            })
            .is_err());
        assert!(Tag::decode_anonymous_with_limits(
            &bytes,
            Limits {
                max_bytes: bytes.len() - 1,
                ..Limits::default()
            }
        )
        .is_err());
        let dense = Tag::List {
            element_type: 1,
            values: vec![Tag::Byte(0); 100],
        }
        .encode_anonymous()
        .unwrap();
        assert!(Tag::decode_anonymous_with_limits(
            &dense,
            Limits {
                max_bytes: 256,
                ..Limits::default()
            }
        )
        .is_err());
    }

    #[test]
    fn empty_list_type_is_retained_but_nonempty_end_or_heterogeneous_is_invalid() {
        let empty = Tag::List {
            element_type: 0,
            values: vec![],
        };
        assert_eq!(
            Tag::decode_anonymous(&empty.encode_anonymous().unwrap()).unwrap(),
            empty
        );
        let bad = Tag::List {
            element_type: 3,
            values: vec![Tag::Int(1), Tag::Byte(2)],
        };
        assert!(bad.encode_anonymous().is_err());
    }

    #[test]
    fn jdk_float_vectors_with_minecraft_factory_zero_normalization() {
        let vectors: serde_json::Value =
            serde_json::from_str(include_str!("../tools/nbt_utf_vectors.json")).unwrap();
        for sample in vectors["floats"].as_array().unwrap() {
            let bits = u32::from_str_radix(sample["input_bits"].as_str().unwrap(), 16).unwrap();
            let mut bytes = vec![5];
            bytes.extend(bits.to_be_bytes());
            let Tag::Float(decoded) = Tag::decode_anonymous(&bytes).unwrap() else {
                panic!()
            };
            assert_eq!(
                decoded.to_bits(),
                if bits == 0x8000_0000 { 0 } else { bits },
                "Minecraft factories normalize signed zero; other decoded bits remain"
            );
            let encoded = Tag::Float(decoded).encode_anonymous().unwrap();
            let mut expected = hex(sample["hex"].as_str().unwrap());
            if decoded == 0.0 {
                expected.fill(0);
            }
            assert_eq!(&encoded[1..], expected);
        }
        for sample in vectors["doubles"].as_array().unwrap() {
            let bits = u64::from_str_radix(sample["input_bits"].as_str().unwrap(), 16).unwrap();
            let mut bytes = vec![6];
            bytes.extend(bits.to_be_bytes());
            let Tag::Double(decoded) = Tag::decode_anonymous(&bytes).unwrap() else {
                panic!()
            };
            assert_eq!(
                decoded.to_bits(),
                if bits == 0x8000_0000_0000_0000 {
                    0
                } else {
                    bits
                },
                "Minecraft factories normalize signed zero; other decoded bits remain"
            );
            let encoded = Tag::Double(decoded).encode_anonymous().unwrap();
            let mut expected = hex(sample["hex"].as_str().unwrap());
            if decoded == 0.0 {
                expected.fill(0);
            }
            assert_eq!(&encoded[1..], expected);
        }
    }
    #[test]
    fn original_minecraft_nbt_io_factory_vectors() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../fixtures/survival-nbt-factory-vectors.json"
        ))
        .unwrap();
        for sample in vectors["cases"].as_array().unwrap() {
            let input = hex(sample["input"].as_str().unwrap());
            let expected = hex(sample["expected"].as_str().unwrap());
            let decoded = NbtRoot::decode(&input).unwrap();
            assert_eq!(
                decoded.encode().unwrap(),
                expected,
                "actual Java uo roundtrip: {}",
                sample["input"]
            );
        }
        assert_eq!(
            Tag::Float(-0.0).encode_anonymous().unwrap(),
            vec![5, 0, 0, 0, 0]
        );
        assert_eq!(
            Tag::Double(-0.0).encode_anonymous().unwrap(),
            vec![6, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn original_minecraft_disk_and_netty_floating_nbt_vectors() {
        let fixture = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/nbt-float-vectors.json"
        ))
        .unwrap();
        let vectors: serde_json::Value = serde_json::from_str(&fixture).unwrap();
        let cases = vectors["cases"].as_array().unwrap();
        assert!(!cases.is_empty());
        for sample in cases {
            let bits = sample["bits"].as_str().unwrap();
            let tag_type = sample["tag_type"].as_u64().unwrap() as u8;
            let tag = match tag_type {
                5 => Tag::Float(f32::from_bits(u32::from_str_radix(bits, 16).unwrap())),
                6 => Tag::Double(f64::from_bits(u64::from_str_radix(bits, 16).unwrap())),
                _ => panic!("expected an original floating-tag vector"),
            };
            let mut input = vec![tag_type];
            input.extend(hex(bits));
            let decoded = Tag::decode_anonymous(&input).unwrap();
            let expected_bits =
                u64::from_str_radix(sample["decoded_bits"].as_str().unwrap(), 16).unwrap();
            let decoded_bits = match &decoded {
                Tag::Float(value) => u64::from(value.to_bits()),
                Tag::Double(value) => value.to_bits(),
                _ => panic!("unexpected decoded tag"),
            };
            assert_eq!(decoded_bits, expected_bits, "original factory: {bits}");
            for value in [tag, decoded] {
                assert_eq!(
                    value.encode_anonymous().unwrap(),
                    hex(sample["disk_anonymous"].as_str().unwrap()),
                    "original DataOutputStream anonymous: {bits}"
                );
                assert_eq!(
                    value.encode_network_anonymous().unwrap(),
                    hex(sample["network_anonymous"].as_str().unwrap()),
                    "original ByteBufOutputStream anonymous: {bits}"
                );
                assert_eq!(
                    NbtRoot {
                        name: "".into(),
                        tag: value,
                    }
                    .encode()
                    .unwrap(),
                    hex(sample["disk_named"].as_str().unwrap()),
                    "original DataOutputStream named: {bits}"
                );
            }
        }
        let nested = vectors["nested"].as_array().unwrap();
        assert!(!nested.is_empty());
        for (index, sample) in nested.iter().enumerate() {
            let tag =
                Tag::decode_anonymous(&hex(sample["input_network"].as_str().unwrap())).unwrap();
            assert_eq!(
                tag.encode_anonymous().unwrap(),
                hex(sample["disk_anonymous"].as_str().unwrap()),
                "nested disk {index}"
            );
            assert_eq!(
                tag.encode_network_anonymous().unwrap(),
                hex(sample["network_anonymous"].as_str().unwrap()),
                "nested Netty {index}"
            );
            assert_eq!(
                NbtRoot {
                    name: "".into(),
                    tag,
                }
                .encode()
                .unwrap(),
                hex(sample["disk_named"].as_str().unwrap()),
                "nested named disk {index}"
            );
        }
    }

    #[test]
    fn network_nbt_encoding_enforces_all_budgets() {
        let tag = Tag::Compound(vec![(
            "a".into(),
            Tag::List {
                element_type: 5,
                values: vec![Tag::Float(f32::from_bits(0xffa1_2345))],
            },
        )]);
        let encoded = tag.encode_network_anonymous().unwrap();
        for limits in [
            Limits {
                max_depth: 1,
                ..Limits::default()
            },
            Limits {
                max_bytes: encoded.len() - 1,
                ..Limits::default()
            },
            Limits {
                max_elements: 2,
                ..Limits::default()
            },
        ] {
            assert!(tag.encode_network_anonymous_with_limits(limits).is_err());
        }
        assert_eq!(
            tag.encode_network_anonymous_with_limits(Limits {
                max_depth: 2,
                max_bytes: encoded.len(),
                max_elements: 3,
            })
            .unwrap(),
            encoded
        );
    }
}
