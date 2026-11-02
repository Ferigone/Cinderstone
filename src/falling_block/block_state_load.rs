//! Owned source BlockState read and the separate Falling load finalization.
//!
//! This is not an Entity.load/Java-save importer. Its input is a retained
//! source-owned parent compound. CompoundTag.getCompound either borrows its
//! actual BlockState child or owns the genuinely fresh source EMPTY result;
//! no caller-created Tag or literal fallback is accepted by the codec.

use super::{
    block_state_codec::ActualBlockRegistry,
    block_state_registry::{
        BlockStateWarningSink, DecodedBlockStateNbtContext, SourceBoundBlockStateCodec,
    },
    persistence::FallingBlockStateCodec,
};
use crate::source_nbt_decode::SourceCompoundView;
use anyhow::{ensure, Context, Result};

/// The successfully parsed source value, before FallingBlockEntity's final
/// AIR-to-SAND correction. Its registry identity is retained across the
/// intervening Time/HurtEntities/DropItem/TileEntityData/CancelDrop phases.
/// Construction is private; an arbitrary numeric state is not this result.
pub struct SourceBlockStateLoad<I: Clone + Eq> {
    state: u32,
    registry: I,
}

/// Call at the original initial BlockState-read phase of
/// FallingBlockEntity.readAdditionalSaveData. The parent must be its actual
/// decoded/owned input compound, not a reconstructed logical Tag.
/// ResourceLocation errors and required property warnings propagate exactly
/// through the pinned codec. Missing/wrong-type BlockState uses an actual
/// parent-bound source getCompound EMPTY owner, alive for the entire read.
pub fn read_block_state_from_parent<R: ActualBlockRegistry, W: BlockStateWarningSink + ?Sized>(
    parent: SourceCompoundView<'_>,
    registry: &R,
    warnings: &mut W,
) -> Result<SourceBlockStateLoad<R::Identity>> {
    let identity = registry.identity()?;
    let lookup = parent.get_compound("BlockState")?;
    let original = lookup.view();
    let mut context = DecodedBlockStateNbtContext::new(original, warnings);
    let mut codec = SourceBoundBlockStateCodec::capture(registry, &mut context)?;
    let state = codec.read_block_state(original.tag())?;
    ensure!(registry.identity()? == identity, "BlockState load registry changed during source read");
    Ok(SourceBlockStateLoad { state, registry: identity })
}

impl<I: Clone + Eq> SourceBlockStateLoad<I> {
    /// Assign this exact value at the initial source read phase. AIR is still
    /// AIR here, so intervening source ANVIL checks see the parsed value.
    pub fn parsed_state(&self) -> u32 { self.state }

    /// Available for the original absent-HurtEntities ANVIL branch, after
    /// Time has been read and before final AIR correction. Current configured
    /// tag membership comes from the SAME retained registry identity.
    pub fn source_is_anvil<R: ActualBlockRegistry<Identity = I>>(&self, registry: &R) -> Result<bool> {
        self.require_registry(registry)?;
        let result = registry.state_is_anvil(self.state)?;
        self.require_registry(registry)?;
        Ok(result)
    }

    /// Invoke ONLY at the final source phase, after all other additional-load
    /// fields. Consuming this result prevents accidentally finalizing it twice.
    /// This does not mutate a Player/world/entity or invent an import caller.
    pub fn finish_after_additional_fields<R: ActualBlockRegistry<Identity = I>>(
        self, registry: &R,
    ) -> Result<u32> {
        self.require_registry(registry)?;
        if !registry.state_is_air(self.state)? {
            self.require_registry(registry)?;
            return Ok(self.state);
        }
        let mut sand = None;
        for block in registry.block_definitions()? {
            if block.key == "minecraft:sand" {
                ensure!(sand.is_none(), "actual registry has duplicate SAND definitions");
                ensure!(block.minimum_state <= block.default_state
                    && block.default_state <= block.maximum_state,
                    "actual SAND default is outside its state domain");
                sand = Some(block.default_state);
            }
        }
        self.require_registry(registry)?;
        sand.context("actual SAND default unavailable at final Falling load phase")
    }

    fn require_registry<R: ActualBlockRegistry<Identity = I>>(&self, registry: &R) -> Result<()> {
        ensure!(registry.identity()? == self.registry,
            "BlockState load registry/tag owner changed between source phases");
        Ok(())
    }
}
