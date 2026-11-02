//! Actual base FallingBlock scheduling for ordinary blocks and concrete powder.
//! Construction, flags3 source callbacks and world-add share the Anvil owner;
//! only the source virtual falling hook differs and does not enable damage.
use super::*;
use mc121_rust::{
    block_ticks,
    falling_block::block::{self, FallingBlockAction, FallingBlockHost,
        NativeFallingClass, OrdinaryFallingBlockHost, ShapeUpdate},
    falling_block::concrete_powder,
    placement::{self, BlockPosition}, tick::TickPriority,
};
use super::main_falling_block_runtime::ConstructedFallingBlock;

struct NativeBaseFallingHost<'a> {
    server: &'a Server,
    world: &'a mut WorldBackend,
    players: &'a mut LivePlayers,
    now: i64,
    border: CollisionBorder,
}
impl NativeBaseFallingHost<'_> {
    /// Pure source/association admission. Concrete powder requires the actual
    /// retained configuration owner before any constructor entropy or write.
    fn require_base_binding(&self, block: u32, state: u32) -> Result<()> {
        ensure!(placement::placement_block_min_state(state)? == block,
            "base falling callback received a foreign Block singleton");
        ensure!(block::inherits_base_falling_hook(state)?,
            "base falling callback lacks its literal inherited falling hook");
        match block::source_class(state)? {
            Some(NativeFallingClass::Ordinary) => Ok(()),
            Some(NativeFallingClass::ConcretePowder) => {
                let owner = self.players.block_runtime.concrete_powder_owner()?;
                let binding = owner.binding_for_state(state)?
                    .context("powder callback lacks its retained actual Block binding")?;
                let hardened = block::concrete_powder_hardened_state(state)?
                    .context("powder callback lacks its captured concrete association")?;
                ensure!(*binding.powder() == block
                    && *binding.concrete() == placement::placement_block_min_state(hardened)?,
                    "powder callback differs from its installed actual Block association");
                ensure!(self.players.block_runtime.owns_falling_tick(state, 2)?,
                    "powder callback lacks its configured shared BLOCK/save owner");
                Ok(())
            }
            _ => bail!("base falling hook has no actual inherited source implementation"),
        }
    }
}
impl FallingBlockHost for NativeBaseFallingHost<'_> {
    type Block = u32;
    type BlockState = u32;
    type FallingEntity = ConstructedFallingBlock;
    fn preflight(&mut self, action: FallingBlockAction, block: &u32, state: &u32,
        position: BlockPosition) -> Result<()> {
        self.players.require_current_native_source(self.world)?;
        self.players.block_runtime.falling_damage_types()?;
        self.require_base_binding(*block, *state)?;
        ensure!(self.players.block_runtime.queue.chunk(block_ticks::pos(position).chunk()).is_some(),
            "actual base FallingBlock queue is not loaded");
        if action == FallingBlockAction::FallingEntity {
            self.server.preflight_native_falling_block_locked(self.world, self.players,
                position, *state, self.now, self.border)?;
        }
        Ok(())
    }
    fn schedule_block_tick(&mut self, position: BlockPosition, block: &u32, delay: i32) -> Result<()> {
        self.players.block_runtime.mark_used();
        self.players.entity_source.as_mut().context("actual base falling source is absent")?
            .observe_block_scheduler_use();
        block_ticks::schedule(&mut self.players.block_runtime.queue, position, *block,
            self.now, delay, TickPriority::Normal, &mut self.players.fluid_runtime.sub_tick_order)?;
        Ok(())
    }
    fn superclass_update_shape(&mut self, update: ShapeUpdate<'_, u32>) -> Result<u32> {
        placement::placement_block_name(*update.state)?;
        // This is the inherited base delegate only. A powder updateShape must
        // enter its real hardening kernel before delegating the dry branch.
        Ok(*update.state)
    }
    fn block_state(&mut self, position: BlockPosition) -> Result<u32> {
        self.world.block(position.0, position.1, position.2)
    }
    fn state_is_air(&mut self, state: &u32) -> Result<bool> {
        Ok(mc121_rust::lighting::state_properties(*state)?.is_air())
    }
    fn state_is_fire_tag(&mut self, state: &u32) -> Result<bool> {
        placement::placement_block_name(*state)?;
        Ok(placement::in_block_tag(*state, "minecraft:fire"))
    }
    fn state_is_liquid(&mut self, state: &u32) -> Result<bool> {
        main_falling_block_world::state_is_liquid(*state)
    }
    fn state_can_be_replaced(&mut self, state: &u32) -> Result<bool> {
        main_falling_block_world::state_can_be_replaced(*state)
    }
    fn minimum_build_height(&mut self) -> Result<i32> { Ok(self.players.world_context.dimension.min_y) }
    fn fall_block(&mut self, position: BlockPosition, state: &u32) -> Result<ConstructedFallingBlock> {
        self.require_base_binding(placement::placement_block_min_state(*state)?, *state)?;
        self.server.construct_native_falling_block_locked(self.world, self.players,
            position, *state, self.now, self.border)
    }
    fn set_hurts_entities(&mut self, entity: &mut ConstructedFallingBlock,
        amount: f32, maximum: i32) -> Result<()> {
        // Shared capability exists on the SAME real FallingBlockEntity, but
        // the base scheduled body never invokes this Anvil-only hook.
        self.players.falling_blocks.object_state_mut(entity)?.set_hurts_entities(amount, maximum);
        Ok(())
    }
}
impl OrdinaryFallingBlockHost for NativeBaseFallingHost<'_> {
    fn inherited_falling(&mut self, block: &u32, state: &u32,
        entity: &mut ConstructedFallingBlock) -> Result<()> {
        self.require_base_binding(*block, *state)?;
        let actual = self.players.falling_blocks.object_state(entity)?;
        ensure!(actual.block_state == main_falling_block_world::carried_state_for_fall(*state)?,
            "ordinary falling hook did not resolve its actual constructed object");
        // Direct pinned dhg/dhk/diu inspection: inherited FallingBlock.falling is
        // literally return-only. No hurt2/40, RNG draw or second entity occurs.
        Ok(())
    }
}

impl Server {
    pub(super) fn tick_native_falling_block_callback_locked(&self, world: &mut WorldBackend,
        players: &mut LivePlayers, position: BlockPosition, state: u32,
        now: i64, border: CollisionBorder) -> Result<()> {
        match block::source_class(state)? {
            Some(NativeFallingClass::Anvil) =>
                self.tick_native_anvil_block_locked(world, players, position, state, now, border),
            Some(NativeFallingClass::Ordinary) => {
                let singleton = placement::placement_block_min_state(state)?;
                block::scheduled_tick(&mut NativeBaseFallingHost {
                    server: self, world, players, now, border,
                }, &singleton, &state, position)?;
                Ok(())
            }
            Some(NativeFallingClass::ConcretePowder) => {
                // Clone only the retained Arc handle, never a Block registry,
                // actor or RNG. This binding is the installed actual owner.
                let owner = players.block_runtime.concrete_powder_owner()?.clone();
                let binding = owner.binding_for_state(state)?
                    .context("scheduled powder lacks its retained actual Block association")?;
                concrete_powder::scheduled_tick(&mut NativeBaseFallingHost {
                    server: self, world, players, now, border,
                }, binding, &state, position)?;
                Ok(())
            }
            None => bail!("scheduled falling subclass has no actual native source owner"),
        }
    }
}
