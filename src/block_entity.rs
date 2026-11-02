//! Native inherited BlockEntity state and its source serialization operations.
//!
//! These fields have concrete ownership. Level, registry-type and component-map
//! handles must come from their actual owners; this module creates none of them.
use crate::{native_block_entities::BlockPosition, nbt::{JavaString, Tag}};
use anyhow::{bail, ensure, Result};

pub const LOAD_COMPONENTS_WARNING: &str = "Failed to load components: {}";
pub const SAVE_COMPONENTS_WARNING: &str = "Failed to save components: {}";

/// Genuine CompoundTag mutation/encounter-order owner, not a list of fields
/// treated as a finished Java map. Compound retains the actual logical object
/// and its insertion/collision/resize history, including for an existing target
/// passed to a subclass hook. There is deliberately no default implementation.
///
/// A fresh root starts with the original new HashMap state. put and merge must
/// perform the real source operations without resetting that history; merge
/// also preserves actual recursive compound/copy behavior and source key
/// encounter order. An unavailable original history is an explicit error.
/// into_ordered_tag consumes that same root exactly once and emits its actual
/// Java HashMap encounter order, retaining actual nested codec ordering. A
/// lexical sort, input-vector order or assumed capacity is not a fallback.
pub trait BlockEntityCompoundOwner {
    type Compound;
    fn fresh_compound(&mut self) -> Result<Self::Compound>;
    fn put(
        &mut self, target: &mut Self::Compound, key: JavaString, value: Tag,
    ) -> Result<()>;
    fn merge_codec_compound(
        &mut self, target: &mut Self::Compound, actual_encoded: Tag,
    ) -> Result<()>;
    fn into_ordered_tag(&mut self, root: Self::Compound) -> Result<Tag>;
}

/// A retained actual DataComponentMap handle. Cloning retains its immutable
/// source value/identity; it must not substitute a freshly decoded empty map.
pub trait BlockEntityComponentMap: Clone {
    fn is_empty(&self) -> bool;
    fn is_empty_singleton(&self) -> bool;
}

/// Registry lookup of the actual retained BlockEntityType object. In source
/// saveId uses BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey, not a palette name.
pub trait BlockEntityTypeKeys<T> {
    fn block_entity_type_key(&self, actual_type: &T) -> Result<Option<JavaString>>;
}

/// Actual BlockEntity.ComponentHelper codec + registry ops + warning owner.
/// These methods implement DataResult.resultOrPartial: report source codec
/// diagnostics and return its actual full/partial value, or None. Encoded
/// compounds retain actual logical key iteration order and one value per key.
/// Actual diagnostics use LOAD_COMPONENTS_WARNING/SAVE_COMPONENTS_WARNING with
/// the genuine DataResult message and the real logger. Unsupported native
/// codecs/registries and failed warning callbacks remain Err, rather
/// than being relabelled as a successful Java codec failure.
pub trait BlockEntityComponentCodec<M> {
    type RegistryLookup: ?Sized;
    fn parse_with_source_diagnostics(
        &mut self, tag: &Tag, registries: &Self::RegistryLookup,
    ) -> Result<Option<M>>;
    fn encode_with_source_diagnostics(
        &mut self, components: &M, registries: &Self::RegistryLookup,
    ) -> Result<Option<Tag>>;
}

/// Source BlockEntity's actual inherited fields. The tuple copies the immutable
/// BlockPos value. L/T/M clones must retain their genuine owner handles. A
/// private world transaction may clone state, but must not clone class RNGs or
/// attach a fabricated Level to it.
#[derive(Clone, Debug)]
pub struct NativeBlockEntityBase<T, L, M: BlockEntityComponentMap> {
    actual_type: T,
    level: Option<L>,
    position: BlockPosition,
    removed: bool,
    state: u32,
    components: M,
}

impl<T, L, M: BlockEntityComponentMap> NativeBlockEntityBase<T, L, M> {
    /// Called by the real subclass factory, after its class initialization.
    /// Only this constructor requires the real EMPTY singleton. Subsequent
    /// imported or applied component maps are retained as supplied.
    pub fn construct(
        actual_type: T, position: BlockPosition, state: u32, actual_empty: M,
    ) -> Result<Self> {
        ensure!(actual_empty.is_empty() && actual_empty.is_empty_singleton(),
            "BlockEntity constructor requires the actual empty component-map singleton");
        Ok(Self {
            actual_type, level: None, position, removed: false, state,
            components: actual_empty,
        })
    }

    pub fn actual_type(&self) -> &T { &self.actual_type }
    pub fn block_position(&self) -> BlockPosition { self.position }
    pub fn block_state(&self) -> u32 { self.state }
    pub fn level(&self) -> Option<&L> { self.level.as_ref() }
    pub fn has_level(&self) -> bool { self.level.is_some() }
    pub fn is_removed(&self) -> bool { self.removed }
    pub fn components(&self) -> &M { &self.components }

    /// Source setLevel/setBlockState/setRemoved/clearRemoved/setComponents are
    /// plain assignments. They neither notify neighbors nor mutate inventory.
    pub fn set_level(&mut self, actual_level: Option<L>) { self.level = actual_level; }
    pub fn set_block_state(&mut self, actual_state: u32) { self.state = actual_state; }
    pub fn set_removed(&mut self) { self.removed = true; }
    pub fn clear_removed(&mut self) { self.removed = false; }
    pub fn set_components(&mut self, actual_components: M) { self.components = actual_components; }

    /// The subclass's loadAdditional must run BEFORE this operation. A failed
    /// codec without a partial result preserves the existing component map;
    /// this is not permission to substitute EMPTY for an unknown import.
    pub fn load_components<C: BlockEntityComponentCodec<M>>(
        &mut self, tag: &Tag, registries: &C::RegistryLookup, codec: &mut C,
    ) -> Result<()> {
        ensure!(matches!(tag, Tag::Compound(_)), "block-entity NBT must be compound");
        if let Some(actual_map) = codec.parse_with_source_diagnostics(tag, registries)? {
            self.components = actual_map;
        }
        Ok(())
    }

    /// The subclass's saveAdditional must have filled this new compound first.
    /// Source then encodes the actual stored map and CompoundTag.merge's the
    /// actual full/partial compound result. No codec result is invented.
    pub fn save_components_into<C: BlockEntityComponentCodec<M>, O: BlockEntityCompoundOwner>(
        &self, tag: &mut O::Compound, registries: &C::RegistryLookup, codec: &mut C,
        compounds: &mut O,
    ) -> Result<()> {
        if let Some(encoded) = codec.encode_with_source_diagnostics(&self.components, registries)? {
            compounds.merge_codec_compound(tag, encoded)?;
        }
        Ok(())
    }

    pub fn save_id<O: BlockEntityCompoundOwner>(
        &self, tag: &mut O::Compound, keys: &impl BlockEntityTypeKeys<T>, compounds: &mut O,
    ) -> Result<()> {
        let key = keys.block_entity_type_key(&self.actual_type)?;
        let Some(key) = key else { bail!("actual BlockEntityType is not registered during saveId"); };
        compounds.put(tag, "id".into(), Tag::String(key))
    }

    /// Source order is saveId, x, y, z, after subclass/component serialization.
    pub fn save_metadata<O: BlockEntityCompoundOwner>(
        &self, tag: &mut O::Compound, keys: &impl BlockEntityTypeKeys<T>, compounds: &mut O,
    ) -> Result<()> {
        self.save_id(tag, keys, compounds)?;
        for (name, value) in [("x", self.position.0), ("y", self.position.1), ("z", self.position.2)] {
            compounds.put(tag, name.into(), Tag::Int(value))?;
        }
        Ok(())
    }
}
