//! Pinned Java 1.21 `FallingBlockEntity.tick`, with real borrowed world owners.
//!
//! The owner must call `EntityPhysicalState::begin_level_tick` at the actual
//! Level tick-list entry. This body deliberately does not call `baseTick`:
//! neither does the original falling-block override. Required callbacks have
//! no successful defaults and must operate on the same retained actor.

use super::state::FallingBlockState;
use crate::{geometry::Vec3, nbt::{JavaString, Tag}, placement::BlockPosition};
use anyhow::{ensure, Result};

/// A Java exception thrown by the actual block-entity load is caught by this
/// source path. Host/registry/provenance failures instead remain outer errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockEntityLoadStatus {
    Loaded,
    /// The host has logged the actual caught load exception. `setChanged`
    /// still follows; an unavailable load owner is not this variant.
    CaughtAndLoggedException,
}

/// All handles resolve through the caller's actual Level/registry/actor owner.
/// No callback may substitute an ItemEntity for the falling actor, fabricate a
/// collision result, silently accept unknown predicates, or replace its RNG.
///
/// Preflight is a native capability/provenance check, not a Java world query.
/// It must not read future branch data, consume entropy or publish effects.
/// A staged native caller must include world, scheduler, index, actor, actual
/// Item drops, RNG and output owners in the same transaction. This body itself
/// neither clones those owners nor promises rollback after a host error.
pub trait FallingBlockTickHost {
    type Block;
    type BlockEntity;

    fn preflight_tick(&mut self, state: &FallingBlockState) -> Result<()>;
    fn state_is_air(&mut self, state: u32) -> Result<bool>;
    /// Return a stable handle to the actual Block singleton. The tick captures
    /// this BEFORE movement can damage/change the carried Anvil BlockState.
    fn block_of_state(&mut self, state: u32) -> Result<Self::Block>;
    /// Exact `Entity.move(SELF, velocity)`. Real collision/checkFallDamage and
    /// entityInside callbacks must retain this actor and its carried state.
    fn move_self(&mut self, state: &mut FallingBlockState, movement: Vec3) -> Result<()>;
    fn handle_portal(&mut self, state: &mut FallingBlockState) -> Result<()>;
    /// Query the actual Level after portal handling, not a cached origin world.
    fn is_client_side(&mut self, state: &FallingBlockState) -> Result<bool>;
    fn is_concrete_powder(&mut self, state: u32) -> Result<bool>;
    fn fluid_is_water(&mut self, position: BlockPosition) -> Result<bool>;
    /// Exact Level.clip: COLLIDER blocks, SOURCE_ONLY fluids, actual entity
    /// collision context, old position -> current position. None means MISS;
    /// return the real non-MISS hit position, even when its fluid is not water.
    fn clip_concrete_water(
        &mut self,
        state: &FallingBlockState,
        from: Vec3,
        to: Vec3,
    ) -> Result<Option<BlockPosition>>;
    fn block_state(&mut self, position: BlockPosition) -> Result<u32>;
    fn is_moving_piston(&mut self, state: u32) -> Result<bool>;
    /// Exact target.canBeReplaced(DirectionalPlaceContext(Level,position,
    /// Direction.DOWN, ItemStack.EMPTY, Direction.UP)).
    fn can_replace_for_falling(&mut self, target: u32, position: BlockPosition) -> Result<bool>;
    /// Exact FallingBlock.isFree for the queried below-state, preserving its
    /// air -> fire tag -> liquid -> canBeReplaced short circuit.
    fn falling_is_free(&mut self, state: u32) -> Result<bool>;
    fn can_survive(&mut self, carried: u32, position: BlockPosition) -> Result<bool>;
    fn has_waterlogged_property(&mut self, state: u32) -> Result<bool>;
    /// Identity with Fluids.WATER, deliberately stricter than the WATER tag.
    fn fluid_is_source_water(&mut self, position: BlockPosition) -> Result<bool>;
    fn with_waterlogged(&mut self, state: u32, value: bool) -> Result<u32>;
    /// Actual Level.setBlock with flags3, including real Block callbacks.
    fn set_block(
        &mut self,
        actor: &mut FallingBlockState,
        position: BlockPosition,
        state: u32,
        flags: i32,
    ) -> Result<bool>;
    /// Actual server ChunkMap.broadcast(actor, new BlockUpdatePacket(position,
    /// Level.getBlockState(position))). The reread is deliberately in tick().
    fn broadcast_block_update(
        &mut self,
        actor: &FallingBlockState,
        position: BlockPosition,
        actual_state: u32,
    ) -> Result<()>;
    /// Entity.discard at its literal source phase, preserving actual removal
    /// and index/tracking notifications. No delayed copied-actor retirement.
    fn discard(&mut self, actor: &mut FallingBlockState) -> Result<()>;
    /// Dispatch only if the captured original Block implements Fallable.
    fn on_land_if_fallable(
        &mut self,
        original_block: &Self::Block,
        actor: &mut FallingBlockState,
        position: BlockPosition,
        carried_state: u32,
        replaced_state: u32,
    ) -> Result<()>;
    /// Dispatch Fallable.onBrokenAfterFall only for that actual interface.
    fn on_broken_after_fall_if_fallable(
        &mut self,
        original_block: &Self::Block,
        actor: &mut FallingBlockState,
        position: BlockPosition,
    ) -> Result<()>;
    fn entity_drops_enabled(&mut self) -> Result<bool>;
    /// Exact Entity.spawnAtLocation(capturedOriginalBlock), retaining real
    /// ItemEntity constructor/ID/RNG/addFreshEntity ordering and its null gate.
    fn spawn_at_location(
        &mut self,
        actor: &mut FallingBlockState,
        original_block: &Self::Block,
    ) -> Result<()>;
    fn minimum_build_height(&mut self) -> Result<i32>;
    fn maximum_build_height(&mut self) -> Result<i32>;
    fn state_has_block_entity(&mut self, state: u32) -> Result<bool>;
    fn block_entity(&mut self, position: BlockPosition) -> Result<Option<Self::BlockEntity>>;
    fn save_block_entity_without_metadata(&mut self, entity: &Self::BlockEntity) -> Result<Tag>;
    /// Actual retained `blockData.getAllKeys()` HashMap encounter order. An
    /// NBT Vec/wire order is not that owner: the host must certify the actual
    /// map/history, including logical duplicate-key removal. No sorted/wire
    /// fallback is allowed when that source map owner is unavailable.
    fn block_data_key_order(&mut self, actor: &FallingBlockState) -> Result<Vec<JavaString>>;
    /// Exact actual loadWithComponents. Only an exception originating in that
    /// load may be logged/caught; unsupported ownership remains an Err.
    fn load_block_entity_with_components(
        &mut self,
        entity: &mut Self::BlockEntity,
        merged: Tag,
    ) -> Result<BlockEntityLoadStatus>;
    fn set_block_entity_changed(&mut self, entity: &mut Self::BlockEntity) -> Result<()>;
}

/// Full original tick body. Supported world/registry/actor capabilities are
/// supplied by the host; this callable kernel alone is not Main integration.
pub fn tick<H: FallingBlockTickHost>(
    state: &mut FallingBlockState,
    host: &mut H,
) -> Result<()> {
    host.preflight_tick(state)?;
    if host.state_is_air(state.block_state)? {
        host.discard(state)?;
        return Ok(());
    }
    let original_block = host.block_of_state(state.block_state)?;
    state.time = state.time.wrapping_add(1);
    // Entity.getGravity is final: noGravity ? 0 : getDefaultGravity(.04).
    // Preserve Vec3.add's x/z +0 operations, including signed zero behavior.
    if !state.physical.no_gravity {
        state.physical.velocity = state.physical.velocity.plus(Vec3::new(0., -0.04, 0.));
    }
    let movement = state.physical.velocity;
    host.move_self(state, movement)?;
    host.handle_portal(state)?;
    if !host.is_client_side(state)?
        && (!state.physical.removed || state.force_tick_after_teleport_to_duplicate)
    {
        server_after_move(state, host, &original_block)?;
    }
    state.physical.velocity = state.physical.velocity.scale(0.98);
    Ok(())
}

fn server_after_move<H: FallingBlockTickHost>(
    actor: &mut FallingBlockState,
    host: &mut H,
    original_block: &H::Block,
) -> Result<()> {
    let mut position = actor.physical.block_position;
    let concrete = host.is_concrete_powder(actor.block_state)?;
    let mut concrete_water = concrete && host.fluid_is_water(position)?;
    let speed_squared = actor.physical.velocity.length_squared();
    if concrete && speed_squared > 1. {
        if let Some(hit) = host.clip_concrete_water(
            actor,
            actor.physical.old_position,
            actor.physical.position_vec(),
        )? {
            if host.fluid_is_water(hit)? {
                position = hit;
                concrete_water = true;
            }
        }
    }
    if actor.physical.position.on_ground || concrete_water {
        let replaced = host.block_state(position)?;
        actor.physical.velocity = actor.physical.velocity.multiply(Vec3::new(0.7, -0.5, 0.7));
        if !host.is_moving_piston(replaced)? {
            if !actor.cancel_drop {
                let replaceable = host.can_replace_for_falling(replaced, position)?;
                // Both predicates are evaluated before the replaceable gate.
                // canSurvive itself is evaluated even when below is free.
                let below = host.block_state((position.0, position.1.wrapping_sub(1), position.2))?;
                let free_below = host.falling_is_free(below)? && !(concrete && concrete_water);
                let survives = host.can_survive(actor.block_state, position)? && !free_below;
                if replaceable && survives {
                    if host.has_waterlogged_property(actor.block_state)?
                        && host.fluid_is_source_water(position)?
                    {
                        actor.block_state = host.with_waterlogged(actor.block_state, true)?;
                    }
                    let carried_to_place = actor.block_state;
                    if host.set_block(actor, position, carried_to_place, 3)? {
                        let actual_state = host.block_state(position)?;
                        host.broadcast_block_update(actor, position, actual_state)?;
                        host.discard(actor)?;
                        let carried_after_discard = actor.block_state;
                        host.on_land_if_fallable(
                            original_block,
                            actor,
                            position,
                            carried_after_discard,
                            replaced,
                        )?;
                        transfer_block_data(actor, host, position)?;
                    } else if actor.drop_item && host.entity_drops_enabled()? {
                        // A failed setBlock without a permitted drop leaves
                        // the actor alive in the original source.
                        host.discard(actor)?;
                        host.on_broken_after_fall_if_fallable(original_block, actor, position)?;
                        host.spawn_at_location(actor, original_block)?;
                    }
                } else {
                    host.discard(actor)?;
                    if actor.drop_item && host.entity_drops_enabled()? {
                        host.on_broken_after_fall_if_fallable(original_block, actor, position)?;
                        host.spawn_at_location(actor, original_block)?;
                    }
                }
            } else {
                host.discard(actor)?;
                host.on_broken_after_fall_if_fallable(original_block, actor, position)?;
            }
        }
    } else {
        // The original repeats the actual Level's client flag here, after
        // preceding world callbacks, rather than caching the first result.
        if !host.is_client_side(actor)? {
            let outside_after_100 = actor.time > 100
                && (position.1 <= host.minimum_build_height()?
                    || position.1 > host.maximum_build_height()?);
            if outside_after_100 || actor.time > 600 {
                // Timeout differs from landing: spawn first, discard second;
                // no onBrokenAfterFall callback is made here.
                if actor.drop_item && host.entity_drops_enabled()? {
                    host.spawn_at_location(actor, original_block)?;
                }
                host.discard(actor)?;
            }
        }
    }
    Ok(())
}

fn transfer_block_data<H: FallingBlockTickHost>(
    actor: &FallingBlockState,
    host: &mut H,
    position: BlockPosition,
) -> Result<()> {
    if let Some(data) = &actor.block_data {
        if host.state_has_block_entity(actor.block_state)? {
            if let Some(mut entity) = host.block_entity(position)? {
                let mut merged = host.save_block_entity_without_metadata(&entity)?;
                let Tag::Compound(entries) = data else {
                    anyhow::bail!("falling block data is not a canonical CompoundTag");
                };
                ensure!(matches!(merged, Tag::Compound(_)), "block entity save is not CompoundTag");
                let keys = host.block_data_key_order(actor)?;
                // Validate the required owner's logical keyset without
                // imposing a replacement order on its actual HashMap walk.
                for (ordinal, key) in keys.iter().enumerate() {
                    ensure!(!keys[..ordinal].contains(key), "duplicate falling BlockData key owner entry");
                    ensure!(entries.iter().any(|(name, _)| name == key), "foreign falling BlockData key");
                }
                ensure!(entries.iter().all(|(key, _)| keys.contains(key)), "incomplete falling BlockData key owner");
                // Values are copied, not shared. All keys, including actual
                // coordinates, are copied; Java has no exclusion list here.
                for key in keys {
                    let value = &entries.iter().rev().find(|(name, _)| name == &key)
                        .expect("validated actual BlockData logical key").1;
                    merged.insert(key, value.clone())?;
                }
                let _status = host.load_block_entity_with_components(&mut entity, merged)?;
                host.set_block_entity_changed(&mut entity)?;
            }
        }
    }
    Ok(())
}
