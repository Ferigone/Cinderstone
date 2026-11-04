//! Canonical native DataComponentMap.EMPTY and the source empty-only BE codec.
//!
//! EMPTY is a genuine singleton, not an unknown/nonempty component fallback.
//! Real registry-ops and diagnostic capabilities remain required at the call.
use crate::{
    block_entity::{BlockEntityComponentCodec, BlockEntityComponentMap, LOAD_COMPONENTS_WARNING},
    nbt::{JavaString, Tag},
};
use anyhow::{ensure, Result};
use std::{fmt, marker::PhantomData, sync::{Arc, OnceLock}};

#[derive(Debug)]
struct EmptyObject;

fn empty_object() -> &'static Arc<EmptyObject> {
    static EMPTY: OnceLock<Arc<EmptyObject>> = OnceLock::new();
    EMPTY.get_or_init(|| Arc::new(EmptyObject))
}

/// Source km$1: get(any type)=null, empty keySet and empty iterator. Only this
/// canonical immutable object is representable here. There is no from-map,
/// from-bytes or nonempty constructor and no implicit Default conversion.
#[derive(Clone, Debug)]
pub struct NativeEmptyComponentMap {
    actual: Arc<EmptyObject>,
}
impl NativeEmptyComponentMap {
    /// Source constructor/default/empty-builder branch only. A caller must
    /// never call this to replace an unsupported component map or failed owner.
    pub fn singleton() -> Self { Self { actual: Arc::clone(empty_object()) } }
    pub fn same_identity(&self, other: &Self) -> bool { Arc::ptr_eq(&self.actual, &other.actual) }
    pub fn size(&self) -> usize { 0 }
}
impl PartialEq for NativeEmptyComponentMap {
    fn eq(&self, other: &Self) -> bool { self.same_identity(other) }
}
impl Eq for NativeEmptyComponentMap {}
impl BlockEntityComponentMap for NativeEmptyComponentMap {
    fn is_empty(&self) -> bool { self.size() == 0 }
    fn is_empty_singleton(&self) -> bool { Arc::ptr_eq(&self.actual, empty_object()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedEmptyComponentCodec {
    NonemptyComponents,
    NoncanonicalEmptyOwner,
}
impl fmt::Display for UnsupportedEmptyComponentCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonemptyComponents => f.write_str("nonempty BE components require their actual component type/value registry codecs"),
            Self::NoncanonicalEmptyOwner => f.write_str("BE empty codec requires the actual canonical EMPTY singleton"),
        }
    }
}
impl std::error::Error for UnsupportedEmptyComponentCodec {}

/// The actual HolderLookup.Provider.createSerializationContext(NbtOps) call.
/// Its real ops object is retained through this operation. Empty maps perform
/// no component-key/value/holder lookup, but this source prefix is not omitted
/// or replaced by a made-up successful registry owner.
pub trait EmptyComponentRegistryContext {
    type Ops;
    fn create_component_nbt_ops(&self) -> Result<Self::Ops>;
}

/// Required real source diagnostic owners; deliberately no no-op/default.
/// nbt_to_source_string is Tag.toString/StringTagVisitor, including actual
/// nested formatting and UTF16. Rust Debug, JSON and lossy text are not that
/// owner. warn_load_components must invoke the actual logger with the supplied
/// source template and genuine DataResult message; callback failures propagate.
pub trait EmptyComponentDiagnostics {
    fn nbt_to_source_string(&mut self, actual_input: &Tag) -> Result<JavaString>;
    fn warn_load_components(
        &mut self, template: &'static str, actual_message: &JavaString,
    ) -> Result<()>;
}

pub struct NativeEmptyComponentCodec<R: EmptyComponentRegistryContext + ?Sized, D: EmptyComponentDiagnostics> {
    diagnostics: D,
    registry: PhantomData<fn(&R)>,
}
impl<R: EmptyComponentRegistryContext + ?Sized, D: EmptyComponentDiagnostics> NativeEmptyComponentCodec<R, D> {
    pub fn new(diagnostics: D) -> Self { Self { diagnostics, registry: PhantomData } }
    pub fn diagnostics(&self) -> &D { &self.diagnostics }

    fn warn_not_map(&mut self, actual_input: &Tag) -> Result<()> {
        let source_text = self.diagnostics.nbt_to_source_string(actual_input)?;
        let mut units: Vec<u16> = "Not a map: ".encode_utf16().collect();
        units.extend(source_text.iter_units());
        let message = JavaString::from_utf16_lossless(units);
        self.diagnostics.warn_load_components(LOAD_COMPONENTS_WARNING, &message)
    }
}

impl<R: EmptyComponentRegistryContext + ?Sized, D: EmptyComponentDiagnostics>
    BlockEntityComponentCodec<NativeEmptyComponentMap> for NativeEmptyComponentCodec<R, D>
{
    type RegistryLookup = R;

    fn parse_with_source_diagnostics(
        &mut self, tag: &Tag, registries: &R,
    ) -> Result<Option<NativeEmptyComponentMap>> {
        let _actual_ops = registries.create_component_nbt_ops()?;
        let Tag::Compound(_) = tag else {
            // MapCodec.codec fails before OptionalFieldCodec can supply any
            // partial/default. No value means the existing BE map is retained.
            self.warn_not_map(tag)?;
            return Ok(None);
        };
        match tag.get("components") {
            // optionalFieldOf(..., EMPTY), and builder.buildFromMapTrusted's
            // actual empty-map branch, both return the SAME canonical object.
            None => Ok(Some(NativeEmptyComponentMap::singleton())),
            Some(Tag::Compound(entries)) if entries.is_empty() => {
                Ok(Some(NativeEmptyComponentMap::singleton()))
            }
            Some(Tag::Compound(_)) => {
                Err(UnsupportedEmptyComponentCodec::NonemptyComponents.into())
            }
            Some(wrong_type) => {
                // Strict optionalFieldOf is NOT lenient. DFU OptionalFieldCodec
                // sets partial Optional.empty on a failed element decode; its
                // default xmap makes that a partial canonical EMPTY. Source
                // resultOrPartial logs BEFORE exposing this actual partial.
                self.warn_not_map(wrong_type)?;
                Ok(Some(NativeEmptyComponentMap::singleton()))
            }
        }
    }

    fn encode_with_source_diagnostics(
        &mut self, components: &NativeEmptyComponentMap, registries: &R,
    ) -> Result<Option<Tag>> {
        let _actual_ops = registries.create_component_nbt_ops()?;
        ensure!(components.is_empty_singleton(), UnsupportedEmptyComponentCodec::NoncanonicalEmptyOwner);
        // Objects.equals(actual, EMPTY) -> Optional.empty. No element/map
        // codec is invoked, no "components" field or source warning is added.
        // MapCodec.encodeStart over NbtOps returns a genuine empty compound.
        Ok(Some(Tag::Compound(Vec::new())))
    }
}
