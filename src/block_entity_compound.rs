//! Actual native fresh CompoundTag roots for BlockEntity source saves.
//!
//! Retains Java HashMap bucket/node/threshold history, not field-vector order.
//! Codec merge input MUST be the actual codec's logical key encounter order as
//! required by BlockEntityComponentCodec. No imported Tag becomes a known map.
use crate::{
    block_entity::BlockEntityCompoundOwner,
    components::java::string_hash,
    nbt::{JavaString, Tag},
};
use anyhow::{bail, ensure, Result};
use std::fmt;

const MAX_KEYS: usize = 4096;
const MAX_CAPACITY: usize = 8192;
const MAX_DEPTH: usize = 128;
const MAX_VISITS: usize = 262_144;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsupportedCompoundTag {
    ImportedHistory,
    DirectNestedHistory,
    TreeBin { capacity: usize },
    ResourceBudget,
}
impl fmt::Display for UnsupportedCompoundTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImportedHistory => f.write_str("imported CompoundTag has no reconstructed HashMap history"),
            Self::DirectNestedHistory => f.write_str("put requires an actual retained nested CompoundTag owner"),
            Self::TreeBin { capacity } => write!(f, "CompoundTag requires an unported HashMap tree bin at capacity {capacity}"),
            Self::ResourceBudget => f.write_str("CompoundTag exceeds the admitted native save resource budget"),
        }
    }
}
impl std::error::Error for UnsupportedCompoundTag {}

/// Actual source-created root. Fields are private; there is no Tag-to-owner
/// reconstruction or public Clone. Explicit source copy is copy_known below.
#[derive(Debug)]
pub struct NativeFreshCompound {
    map: JavaMap,
}

/// Exclusively owned native tag tree. Compound nodes retain their actual
/// history, including when nested inside lists; a bare Tag cannot mint that
/// history. This is an owned value, not a binary-decoder provenance token.
#[derive(Debug)]
pub struct NativeOwnedNbt {
    value: Value,
}
impl NativeOwnedNbt {
    pub fn compound_view(&self) -> Option<NativeCompoundView<'_>> {
        match &self.value {
            Value::Compound(map) => Some(NativeCompoundView { map }),
            _ => None,
        }
    }
}

/// Borrow of an actual retained native map, never a reconstructed Tag Vec.
#[derive(Clone, Copy)]
pub struct NativeCompoundView<'a> {
    map: &'a JavaMap,
}
impl<'a> NativeCompoundView<'a> {
    pub fn key_order(&self) -> Vec<JavaString> {
        self.map.table.iter().flatten().map(|node| node.key.clone()).collect()
    }
    pub fn child_compound(&self, name: &str) -> Option<Self> {
        self.child_compound_key(&name.into())
    }
    pub fn child_compound_key(&self, name: &JavaString) -> Option<Self> {
        match self.map.get(name) {
            Some(Value::Compound(map)) => Some(Self { map }),
            _ => None,
        }
    }
}

/// One concrete algorithm/capability, with no world or random-stream ownership.
pub struct NativeFreshCompoundOwner;

impl NativeFreshCompoundOwner {
    pub fn new() -> Self { Self }

    /// A real scalar/array/list value with no bare nested compound history.
    pub fn owned_noncompound(&mut self, tag: Tag) -> Result<NativeOwnedNbt> {
        Ok(NativeOwnedNbt { value: Value::direct(tag, 0, &mut Budget::default())? })
    }

    pub fn owned_compound(&mut self, actual: NativeFreshCompound) -> NativeOwnedNbt {
        NativeOwnedNbt { value: Value::Compound(Box::new(actual.map)) }
    }

    /// Native source ListTag construction over its already retained children.
    /// No child compound is re-imported, copied, normalized or re-ordered.
    pub fn owned_list(&mut self, element_type: u8, values: Vec<NativeOwnedNbt>) -> Result<NativeOwnedNbt> {
        ensure!(element_type <= 12 && values.iter().all(|value| value.value.tag_id() == element_type),
            "actual owned ListTag has an invalid element type");
        let value = Value::List { element_type, values: values.into_iter().map(|value| value.value).collect() };
        value.require_budget(0, &mut Budget::default())?;
        Ok(NativeOwnedNbt { value })
    }

    pub fn put_owned_value(
        &mut self, target: &mut NativeFreshCompound, key: JavaString, value: NativeOwnedNbt,
    ) -> Result<()> {
        value.value.require_budget(1, &mut Budget::default())?;
        let mut staged = target.map.clone();
        staged.put(key.clone(), value.value.clone())?;
        target.map.put(key, value.value)
    }

    /// Immutable logical observation in the actual retained maps' encounter
    /// order. This does NOT invoke source CompoundTag.copy/HashMap(Map), consume
    /// or replace those owners, or issue a decoder/history capability.
    pub fn observe_owned_tag(&self, actual: &NativeOwnedNbt) -> Tag {
        actual.value.clone().into_tag()
    }

    /// A decoded entry sequence does not reveal the old capacity, deletions,
    /// tree structure or insertion history. Never certify it as a fresh map.
    pub fn adopt_imported(&mut self, _tag: Tag) -> Result<NativeFreshCompound> {
        Err(UnsupportedCompoundTag::ImportedHistory.into())
    }

    /// Source CompoundTag.copy: transformValues(Tag.copy), then HashMap(Map),
    /// not a clone of the old bucket capacity. Real source encounter order and
    /// size are available because this is an actual retained native owner.
    pub fn copy_known(&mut self, source: &NativeFreshCompound) -> Result<NativeFreshCompound> {
        let mut budget = Budget::default();
        Ok(NativeFreshCompound { map: source.map.source_copy(0, &mut budget)? })
    }

    /// Source put of another actual native CompoundTag retains that object's
    /// map history. A bare nested Tag is deliberately not accepted by put.
    pub fn put_owned_compound(
        &mut self, target: &mut NativeFreshCompound, key: JavaString,
        value: NativeFreshCompound,
    ) -> Result<()> {
        value.map.require_budget(1, &mut Budget::default())?;
        let mut staged = target.map.clone();
        staged.put(key.clone(), Value::Compound(Box::new(value.map.clone())))?;
        target.map.put(key, Value::Compound(Box::new(value.map)))
    }

    /// Source HashMap.remove does not shrink/reset the table or reorder the
    /// surviving list nodes. This supports genuine existing-target hooks.
    pub fn remove(&mut self, target: &mut NativeFreshCompound, key: &JavaString) {
        target.map.remove(key);
    }
}

impl BlockEntityCompoundOwner for NativeFreshCompoundOwner {
    type Compound = NativeFreshCompound;
    fn fresh_compound(&mut self) -> Result<Self::Compound> {
        Ok(NativeFreshCompound { map: JavaMap::fresh() })
    }
    fn put(&mut self, target: &mut Self::Compound, key: JavaString, value: Tag) -> Result<()> {
        // Pure admission shadow retains exact history; failure cannot partially
        // mutate the real target at an unsupported tree/nesting boundary.
        let mut budget = Budget::default();
        let value = Value::direct(value, 1, &mut budget)?;
        let mut staged = target.map.clone();
        staged.put(key.clone(), value.clone())?;
        target.map.put(key, value)
    }
    fn merge_codec_compound(&mut self, target: &mut Self::Compound, actual_encoded: Tag) -> Result<()> {
        let mut staged = target.map.clone();
        staged.merge_codec(actual_encoded.clone(), 0, &mut Budget::default())?;
        // Deterministic admission has succeeded. Apply to the SAME actual map,
        // retaining existing key/value/node ownership rather than adopting its
        // private shadow as a replacement source object.
        target.map.merge_codec(actual_encoded, 0, &mut Budget::default())
    }
    fn into_ordered_tag(&mut self, root: Self::Compound) -> Result<Tag> {
        Ok(root.map.into_tag())
    }
}

#[derive(Default)]
struct Budget { visits: usize }
impl Budget {
    fn visit(&mut self, depth: usize) -> Result<()> {
        self.visits += 1;
        ensure!(depth <= MAX_DEPTH && self.visits <= MAX_VISITS, UnsupportedCompoundTag::ResourceBudget);
        Ok(())
    }
}

#[derive(Clone, Debug)]
enum Value {
    Atom(Tag),
    List { element_type: u8, values: Vec<Value> },
    Compound(Box<JavaMap>),
}
impl Value {
    fn tag_id(&self) -> u8 {
        match self { Self::Atom(tag) => tag.tag_id(), Self::List { .. } => 9, Self::Compound(_) => 10 }
    }
    fn direct(tag: Tag, depth: usize, budget: &mut Budget) -> Result<Self> {
        budget.visit(depth)?;
        match tag {
            Tag::Compound(_) => Err(UnsupportedCompoundTag::DirectNestedHistory.into()),
            Tag::List { element_type, values } => {
                validate_list(element_type, &values)?;
                let values = values.into_iter().map(|v| Self::direct(v, depth + 1, budget)).collect::<Result<_>>()?;
                Ok(Self::List { element_type, values })
            }
            atom => Ok(Self::Atom(atom)),
        }
    }
    /// Actual codec CompoundTag.copy can be reconstructed from its real key
    /// encounter order and size: the NEW HashMap(Map) loses the source table's
    /// old capacity but inserts in that encounter order with a new threshold.
    fn copy_codec(tag: Tag, depth: usize, budget: &mut Budget) -> Result<Self> {
        budget.visit(depth)?;
        match tag {
            Tag::Compound(entries) => {
                validate_codec_entries(&entries)?;
                let mut copy = JavaMap::copy_target(entries.len())?;
                for (key, tag) in entries {
                    copy.put(key, Self::copy_codec(tag, depth + 1, budget)?)?;
                }
                Ok(Self::Compound(Box::new(copy)))
            }
            Tag::List { element_type, values } => {
                validate_list(element_type, &values)?;
                let values = values.into_iter().map(|v| Self::copy_codec(v, depth + 1, budget)).collect::<Result<_>>()?;
                Ok(Self::List { element_type, values })
            }
            atom => Ok(Self::Atom(atom)),
        }
    }
    fn source_copy(&self, depth: usize, budget: &mut Budget) -> Result<Self> {
        budget.visit(depth)?;
        Ok(match self {
            Self::Atom(tag) => Self::Atom(tag.clone()),
            Self::List { element_type, values } => Self::List {
                element_type: *element_type,
                values: values.iter().map(|v| v.source_copy(depth + 1, budget)).collect::<Result<_>>()?,
            },
            Self::Compound(map) => Self::Compound(Box::new(map.source_copy(depth, budget)?)),
        })
    }
    fn require_budget(&self, depth: usize, budget: &mut Budget) -> Result<()> {
        budget.visit(depth)?;
        match self {
            Self::Compound(map) => map.require_budget(depth, budget),
            Self::List { values, .. } => {
                for value in values { value.require_budget(depth + 1, budget)?; }
                Ok(())
            }
            Self::Atom(_) => Ok(()),
        }
    }
    fn into_tag(self) -> Tag {
        match self {
            Self::Atom(tag) => tag,
            Self::List { element_type, values } => Tag::List {
                element_type, values: values.into_iter().map(Self::into_tag).collect(),
            },
            Self::Compound(map) => map.into_tag(),
        }
    }
}

#[derive(Clone, Debug)]
struct Node { hash: u32, key: JavaString, value: Value }

/// Each bucket is the original linked-list chain in encounter order. Vectors
/// here represent actual node chains/table slots, not field-order output.
/// Tree bins are rejected before mutation of the real owner.
#[derive(Clone, Debug)]
struct JavaMap {
    table: Vec<Vec<Node>>,
    threshold: usize,
    size: usize,
}
impl JavaMap {
    fn fresh() -> Self { Self { table: Vec::new(), threshold: 0, size: 0 } }
    fn copy_target(size: usize) -> Result<Self> {
        ensure!(size <= MAX_KEYS, UnsupportedCompoundTag::ResourceBudget);
        let mut result = Self::fresh();
        if size != 0 {
            // Pinned JDK21 putMapEntries uses ceil(s / (double)loadFactor).
            // This differs from the older float(s/.75)+1 implementation.
            result.threshold = ((size * 4 + 2) / 3).next_power_of_two();
        }
        Ok(result)
    }
    fn get_mut(&mut self, key: &JavaString) -> Option<&mut Value> {
        if self.table.is_empty() { return None; }
        let hash = key_hash(key);
        let index = hash as usize & (self.table.len() - 1);
        self.table[index].iter_mut().find(|node| node.hash == hash && node.key == *key).map(|node| &mut node.value)
    }
    fn get(&self, key: &JavaString) -> Option<&Value> {
        if self.table.is_empty() { return None; }
        let hash = key_hash(key);
        let index = hash as usize & (self.table.len() - 1);
        self.table[index].iter().find(|node| node.hash == hash && node.key == *key).map(|node| &node.value)
    }
    fn put(&mut self, key: JavaString, value: Value) -> Result<()> {
        if self.table.is_empty() { self.resize()?; }
        let hash = key_hash(&key);
        let index = hash as usize & (self.table.len() - 1);
        if let Some(node) = self.table[index].iter_mut().find(|node| node.hash == hash && node.key == key) {
            // Java overwrites value in the EXISTING node; it keeps its key
            // object, chain position, capacity, size and threshold.
            node.value = value;
            return Ok(());
        }
        ensure!(self.size < MAX_KEYS, UnsupportedCompoundTag::ResourceBudget);
        let treeify = self.table[index].len() >= 8;
        if treeify && self.table.len() >= 64 {
            return Err(UnsupportedCompoundTag::TreeBin { capacity: self.table.len() }.into());
        }
        self.table[index].push(Node { hash, key, value });
        // putVal's binCount >=7 occurs when appending after eight old list
        // nodes. treeifyBin below64 resizes BEFORE size's load-factor gate.
        if treeify { self.resize()?; }
        self.size += 1;
        if self.size > self.threshold { self.resize()?; }
        Ok(())
    }
    fn resize(&mut self) -> Result<()> {
        let old_capacity = self.table.len();
        let capacity = if old_capacity != 0 { old_capacity * 2 }
            else if self.threshold != 0 { self.threshold } else { 16 };
        ensure!(capacity <= MAX_CAPACITY, UnsupportedCompoundTag::ResourceBudget);
        let mut next: Vec<Vec<Node>> = (0..capacity).map(|_| Vec::new()).collect();
        // Source low/high split preserves each old chain's relative order.
        for bucket in std::mem::take(&mut self.table) {
            for node in bucket {
                next[node.hash as usize & (capacity - 1)].push(node);
            }
        }
        self.table = next;
        self.threshold = capacity * 3 / 4;
        Ok(())
    }
    fn remove(&mut self, key: &JavaString) {
        if self.table.is_empty() { return; }
        let hash = key_hash(key);
        let index = hash as usize & (self.table.len() - 1);
        if let Some(node) = self.table[index].iter().position(|node| node.hash == hash && node.key == *key) {
            self.table[index].remove(node);
            self.size -= 1;
        }
    }
    fn merge_codec(&mut self, tag: Tag, depth: usize, budget: &mut Budget) -> Result<()> {
        budget.visit(depth)?;
        let Tag::Compound(entries) = tag else { bail!("actual ComponentHelper codec result is not a compound"); };
        validate_codec_entries(&entries)?;
        for (key, value) in entries {
            if matches!(&value, Tag::Compound(_)) {
                if let Some(Value::Compound(existing)) = self.get_mut(&key) {
                    existing.merge_codec(value, depth + 1, budget)?;
                    continue;
                }
            }
            // Source merge copies every replacement value, including compounds
            // and lists, rather than adopting/resetting the source map's order.
            self.put(key, Value::copy_codec(value, depth + 1, budget)?)?;
        }
        Ok(())
    }
    fn source_copy(&self, depth: usize, budget: &mut Budget) -> Result<Self> {
        budget.visit(depth)?;
        let mut copy = Self::copy_target(self.size)?;
        for node in self.table.iter().flatten() {
            copy.put(node.key.clone(), node.value.source_copy(depth + 1, budget)?)?;
        }
        Ok(copy)
    }
    fn require_budget(&self, depth: usize, budget: &mut Budget) -> Result<()> {
        budget.visit(depth)?;
        for node in self.table.iter().flatten() {
            node.value.require_budget(depth + 1, budget)?;
        }
        Ok(())
    }
    fn into_tag(self) -> Tag {
        Tag::Compound(self.table.into_iter().flatten().map(|node| (node.key, node.value.into_tag())).collect())
    }
}

fn key_hash(key: &JavaString) -> u32 {
    let original = if let Some(text) = key.as_str() {
        string_hash(text) as u32
    } else {
        // The existing helper accepts Rust str only. Java/NBT permits lone
        // UTF16 surrogates; hash those exact code units without replacement.
        key.iter_units().fold(0u32, |hash, unit| hash.wrapping_mul(31).wrapping_add(u32::from(unit)))
    };
    original ^ (original >> 16)
}
fn validate_codec_entries(entries: &[(JavaString, Tag)]) -> Result<()> {
    ensure!(entries.len() <= MAX_KEYS, UnsupportedCompoundTag::ResourceBudget);
    for (index, (key, _)) in entries.iter().enumerate() {
        ensure!(!entries[..index].iter().any(|(previous, _)| previous == key),
            "actual codec CompoundTag has duplicate logical keys");
    }
    Ok(())
}
fn validate_list(element_type: u8, values: &[Tag]) -> Result<()> {
    ensure!(element_type <= 12 && values.iter().all(|value| value.tag_id() == element_type),
        "actual codec ListTag has an invalid element type");
    Ok(())
}
