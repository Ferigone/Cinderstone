//! Actual world/BLOCK queue adapter for the original AnvilBlock callbacks.
//!
//! The generic entity capability is mandatory. Until Main supplies its genuine
//! falling runtime, callbacks are not registered as supported native work.
//! Source: merged `workstations::anvil_block`; pinned direct-javap evidence is
//! described in `docs/reference/falling-block-production-contract.md`.

use super::*;
use super::main_falling_block_world::MainFallingBlockWorld;
use mc121_rust::{
    placement::{self, BlockPosition},
    workstations::anvil_block::{self, AnvilBlockAction, AnvilBlockHost, ShapeUpdate},
};

/// Implement only over the actual retained falling actor/index/constructor
/// owners and real world callbacks. No successful unavailable implementation
/// exists. A failed addFreshEntity still returns the same constructed object,
/// so the associated capability need not be a registered EntityHandle.
pub(super) trait ActualFallingBlockOwner {
    type FallingEntity;

    /// Complete fallible capability checks before constructor entropy. This
    /// must certify origin fluid replacement/callbacks, real ID and entity RNG,
    /// actual add attempt, same-object hurt configuration, tracking/tick/save.
    fn preflight_fall(
        &mut self,
        world: &mut MainFallingBlockWorld<'_>,
        position: BlockPosition,
        original_state: u32,
    ) -> Result<()>;

    /// WATERLOGGED=false carried-state normalization; actual constructor at
    /// (x+.5,y,z+.5); origin ORIGINAL-fluid legacy replacement flags3; actual
    /// addFreshEntity attempt (boolean ignored); return SAME constructed owner.
    fn fall_block(
        &mut self,
        world: &mut MainFallingBlockWorld<'_>,
        position: BlockPosition,
        original_state: u32,
    ) -> Result<Self::FallingEntity>;

    /// Runs only after the actual add attempt has returned, including false.
    fn set_hurts_entities(
        &mut self,
        entity: &mut Self::FallingEntity,
        damage_per_distance: f32,
        damage_max: i32,
    ) -> Result<()>;

}

pub(super) struct MainAnvilBlockHost<'a, 'world, F: ActualFallingBlockOwner> {
    pub(super) frame: &'a mut MainFallingBlockWorld<'world>,
    pub(super) falling: &'a mut F,
}

impl<F: ActualFallingBlockOwner> AnvilBlockHost for MainAnvilBlockHost<'_, '_, F> {
    type Block = u32;
    type BlockState = u32;
    type FallingEntity = F::FallingEntity;

    fn preflight(&mut self, action: AnvilBlockAction, block: &u32, state: &u32,
        position: BlockPosition) -> Result<()> {
        self.frame.validate_callback(position, *block, *state)?;
        ensure!(placement::in_block_tag(*state, "minecraft:anvil"),
            "Anvil callback received a non-Anvil Block");
        if action == AnvilBlockAction::FallingEntity {
            self.falling.preflight_fall(self.frame, position, *state)?;
        }
        Ok(())
    }

    fn schedule_block_tick(&mut self, position: BlockPosition, block: &u32,
        delay: i32) -> Result<()> {
        self.frame.schedule(position, *block, delay)
    }

    fn superclass_update_shape(&mut self, update: ShapeUpdate<'_, u32>) -> Result<u32> {
        // Pinned BlockBehaviour.updateShape returns the invoked state unchanged.
        // The Anvil kernel has already performed the real schedule attempt.
        placement::placement_block_name(*update.state)?;
        Ok(*update.state)
    }

    fn block_state(&mut self, position: BlockPosition) -> Result<u32> {
        self.frame.block(position)
    }
    fn state_is_air(&mut self, state: &u32) -> Result<bool> {
        Ok(mc121_rust::lighting::state_properties(*state)?.is_air())
    }
    fn state_is_fire_tag(&mut self, state: &u32) -> Result<bool> {
        placement::placement_block_name(*state)?;
        Ok(placement::in_block_tag(*state, "minecraft:fire"))
    }
    fn state_is_liquid(&mut self, state: &u32) -> Result<bool> {
        super::main_falling_block_world::state_is_liquid(*state)
    }
    fn state_can_be_replaced(&mut self, state: &u32) -> Result<bool> {
        super::main_falling_block_world::state_can_be_replaced(*state)
    }
    fn minimum_build_height(&mut self) -> Result<i32> {
        Ok(self.frame.dimension.min_y)
    }
    fn fall_block(&mut self, position: BlockPosition, state: &u32)
        -> Result<F::FallingEntity> {
        self.falling.fall_block(self.frame, position, *state)
    }
    fn set_hurts_entities(&mut self, entity: &mut F::FallingEntity,
        damage_per_distance: f32, damage_max: i32) -> Result<()> {
        self.falling.set_hurts_entities(entity, damage_per_distance, damage_max)
    }
}

pub(super) fn on_place<F: ActualFallingBlockOwner>(
    frame: &mut MainFallingBlockWorld<'_>, falling: &mut F,
    position: BlockPosition, state: u32,
) -> Result<()> {
    let block = placement::placement_block_min_state(state)?;
    anvil_block::on_place(&mut MainAnvilBlockHost { frame, falling }, &block, &state, position)
}

pub(super) fn update_shape<F: ActualFallingBlockOwner>(
    frame: &mut MainFallingBlockWorld<'_>, falling: &mut F,
    update: ShapeUpdate<'_, u32>,
) -> Result<u32> {
    let block = placement::placement_block_min_state(*update.state)?;
    anvil_block::update_shape(&mut MainAnvilBlockHost { frame, falling }, &block, update)
}

/// Outer LevelTicks must first compare the current state's Block singleton to
/// the saved key. A facing-property change does not make the tick stale.
pub(super) fn scheduled_tick<F: ActualFallingBlockOwner>(
    frame: &mut MainFallingBlockWorld<'_>, falling: &mut F,
    position: BlockPosition, state: u32,
) -> Result<anvil_block::AnvilTickOutcome> {
    let block = placement::placement_block_min_state(state)?;
    anvil_block::scheduled_tick(&mut MainAnvilBlockHost { frame, falling }, &block, &state, position)
}
