//! Binary-load-issued immutable NBT and genuine CompoundTag history views.
//!
//! Public constructors decode actual bytes. No Tag/pointer/value-equality
//! constructor can certify a prior in-memory map. The raw nbt API is unchanged.
//! A bound getCompound call can also issue its genuine source fresh-empty result.
use crate::{
    block_entity::BlockEntityCompoundOwner,
    block_entity_compound::{NativeCompoundView, NativeFreshCompoundOwner, NativeOwnedNbt},
    nbt::{JavaString, Limits, NbtRoot, Tag},
};
use anyhow::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceNbtFraming { Named, Anonymous }

/// One immutable native binary-load result. Its normalized logical Tag and
/// associated actual compound histories are private and retained together.
/// Neither public Clone nor extraction of a mutable owned Tag is provided.
pub struct SourceDecodedNbt {
    name: Option<JavaString>,
    framing: SourceNbtFraming,
    consumed: usize,
    tag: Box<Tag>,
    actual: NativeOwnedNbt,
}

impl SourceDecodedNbt {
    pub fn decode_named(bytes: &[u8]) -> Result<Self> {
        Self::decode_named_with_limits(bytes, Limits::default())
    }
    pub fn decode_named_with_limits(bytes: &[u8], limits: Limits) -> Result<Self> {
        let root = NbtRoot::decode_with_limits(bytes, limits)?;
        Self::from_wire(root.tag, Some(root.name), SourceNbtFraming::Named, bytes.len())
    }
    pub fn decode_anonymous(bytes: &[u8]) -> Result<Self> {
        Self::decode_anonymous_with_limits(bytes, Limits::default())
    }
    pub fn decode_anonymous_with_limits(bytes: &[u8], limits: Limits) -> Result<Self> {
        let tag = Tag::decode_anonymous_with_limits(bytes, limits)?;
        Self::from_wire(tag, None, SourceNbtFraming::Anonymous, bytes.len())
    }
    pub fn decode_anonymous_prefix(bytes: &[u8]) -> Result<(Self, usize)> {
        Self::decode_anonymous_prefix_with_limits(bytes, Limits::default())
    }
    pub fn decode_anonymous_prefix_with_limits(bytes: &[u8], limits: Limits) -> Result<(Self, usize)> {
        let (tag, consumed) = Tag::decode_anonymous_prefix_with_limits(bytes, limits)?;
        let decoded = Self::from_wire(tag, None, SourceNbtFraming::Anonymous, consumed)?;
        Ok((decoded, consumed))
    }

    // Only the actual byte-decoder constructors above can reach this issuer.
    // Reader preserves wire entries and duplicate occurrences. Replaying that
    // exact decoded stream recursively is source CompoundTag binary load, NOT
    // HashMap(Map)/CompoundTag.copy and not a blessing of arbitrary Tag input.
    fn from_wire(tag: Tag, name: Option<JavaString>, framing: SourceNbtFraming, consumed: usize) -> Result<Self> {
        let mut owner = NativeFreshCompoundOwner::new();
        let actual = load_wire_value(tag, &mut owner)?;
        let tag = Box::new(owner.observe_owned_tag(&actual));
        Ok(Self { name, framing, consumed, tag, actual })
    }

    pub fn tag(&self) -> &Tag { &self.tag }
    pub fn name(&self) -> Option<&JavaString> { self.name.as_ref() }
    pub fn framing(&self) -> SourceNbtFraming { self.framing }
    pub fn consumed(&self) -> usize { self.consumed }

    /// Actual root type decides absence; this never constructs an empty/root
    /// substitute. The private issuer establishes Tag/history correspondence.
    pub fn root_compound(&self) -> Option<SourceCompoundView<'_>> {
        if !matches!(&*self.tag, Tag::Compound(_)) { return None; }
        let history = self.actual.compound_view()
            .expect("immutable decoded root lost its actual compound history");
        Some(SourceCompoundView { tag: &self.tag, history })
    }
}

/// Borrowed capability for THIS actual immutable source-issued compound node.
/// Fields/constructors are private. Its lifetime retains the genuine decoder
/// owner or parent-bound getCompound result; no pointer/vector can issue it.
#[derive(Clone, Copy)]
pub struct SourceCompoundView<'a> {
    tag: &'a Tag,
    history: NativeCompoundView<'a>,
}
impl<'a> SourceCompoundView<'a> {
    pub fn tag(&self) -> &'a Tag { self.tag }
    pub fn key_order(&self) -> Vec<JavaString> { self.history.key_order() }

    pub fn child_compound(&self, name: &str) -> Option<Self> {
        self.child_compound_key(&name.into())
    }
    pub fn child_compound_key(&self, name: &JavaString) -> Option<Self> {
        let Tag::Compound(entries) = self.tag else {
            unreachable!("source-issued compound view is not a compound");
        };
        let tag = &entries.iter().find(|(key, _)| key == name)?.1;
        if !matches!(tag, Tag::Compound(_)) { return None; }
        let history = self.history.child_compound_key(name)
            .expect("immutable decoded child lost its actual compound history");
        Some(Self { tag, history })
    }

    /// Actual CompoundTag.getCompound, not child-only lookup. A missing or
    /// wrong-type logical value constructs a NEW genuine empty compound for
    /// this call, without inserting it into or changing the parent. The opaque
    /// result retains its parent/key and keeps a fresh result alive for views.
    pub fn get_compound(&self, name: &str) -> Result<SourceCompoundLookup<'a>> {
        self.get_compound_key(&name.into())
    }
    pub fn get_compound_key(&self, name: &JavaString) -> Result<SourceCompoundLookup<'a>> {
        let value = match self.child_compound_key(name) {
            Some(actual) => CompoundLookupValue::Borrowed(actual),
            None => {
                let mut owner = NativeFreshCompoundOwner::new();
                let fresh = owner.fresh_compound()?;
                let actual = owner.owned_compound(fresh);
                let tag = Box::new(owner.observe_owned_tag(&actual));
                CompoundLookupValue::Fresh { tag, actual }
            }
        };
        Ok(SourceCompoundLookup { parent: *self, key: name.clone(), value })
    }

    /// Identity VALIDATION under already retained genuine source ownership.
    /// This cannot create provenance: the view was privately issued first,
    /// keeps its immutable node alive, and a cloned/equal Tag does not match.
    pub fn matches_node(&self, candidate: &Tag) -> bool {
        std::ptr::eq(self.tag, candidate)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceCompoundLookupKind { Borrowed, FreshEmpty }

/// Privately issued result of one actual parent-bound getCompound call. There
/// is no public constructor or extraction/replacement of its fresh node.
pub struct SourceCompoundLookup<'a> {
    parent: SourceCompoundView<'a>,
    key: JavaString,
    value: CompoundLookupValue<'a>,
}
enum CompoundLookupValue<'a> {
    Borrowed(SourceCompoundView<'a>),
    Fresh { tag: Box<Tag>, actual: NativeOwnedNbt },
}
impl<'a> SourceCompoundLookup<'a> {
    pub fn parent(&self) -> SourceCompoundView<'a> { self.parent }
    pub fn key(&self) -> &JavaString { &self.key }
    pub fn kind(&self) -> SourceCompoundLookupKind {
        match &self.value {
            CompoundLookupValue::Borrowed(_) => SourceCompoundLookupKind::Borrowed,
            CompoundLookupValue::Fresh { .. } => SourceCompoundLookupKind::FreshEmpty,
        }
    }
    /// Borrowing this lookup retains the exact fresh object across a complete
    /// source state read/warning phase. Each default call has distinct identity;
    /// an actually present empty child is the original borrowed object.
    pub fn view(&self) -> SourceCompoundView<'_> {
        match &self.value {
            CompoundLookupValue::Borrowed(actual) => *actual,
            CompoundLookupValue::Fresh { tag, actual } => SourceCompoundView {
                tag,
                history: actual.compound_view()
                    .expect("source getCompound result lost its actual fresh history"),
            },
        }
    }
}

fn load_wire_value(tag: Tag, owner: &mut NativeFreshCompoundOwner) -> Result<NativeOwnedNbt> {
    match tag {
        Tag::Compound(entries) => {
            let mut actual = owner.fresh_compound()?;
            for (key, value) in entries {
                // Original fully loads this child BEFORE Map.put. Duplicate
                // names replace the old value while retaining existing node
                // position/key, capacity and all prior resize history.
                let value = load_wire_value(value, owner)?;
                owner.put_owned_value(&mut actual, key, value)?;
            }
            Ok(owner.owned_compound(actual))
        }
        Tag::List { element_type, values } => {
            let values = values.into_iter().map(|tag| load_wire_value(tag, owner)).collect::<Result<_>>()?;
            owner.owned_list(element_type, values)
        }
        scalar => owner.owned_noncompound(scalar),
    }
}
