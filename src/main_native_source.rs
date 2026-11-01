//! Commit boundary for observed native block history. A pending transition
//! cannot make items tick until its actual callbacks and light work complete.
use super::*;
use mc121_rust::{
    entity_persistence::{NativeBlockTickOwner, PreparedNativeSourceEdit},
    placement::{BlockPosition, PlacementWorld},
    world_interactions::{self, InteractionEvent, InteractionPlan},
};
impl NativeBlockTickOwner for super::main_block_ticks::MainBlockRuntime {
    fn owns_native_placed_by(&self,state:u32,class:&str)->Result<bool>{self.require_healthy()?;Ok(class=="dia"&&mc121_rust::redstone::owns_placed_by(state)?)}
    fn owns_native_block_tick(
        &self,
        tick: &mc121_rust::placement::PlannedBlockTick,
    ) -> Result<bool> {
        self.require_healthy()?;
        Ok((super::main_fire_simulation::MainFireRuntime::supports_scheduled_state(tick.state)&&(30..=39).contains(&tick.delay))||mc121_rust::redstone::owns_scheduled_callback(tick.state,tick.delay)?||(mc121_rust::native_block_entities::container_kind(tick.state).is_some()&&tick.delay==5))
    }
}

struct CallbackQuery<'a>(&'a mut WorldBackend);
impl PlacementWorld for CallbackQuery<'_> {
    fn block(&mut self, position: BlockPosition) -> Result<u32> {
        self.0.block(position.0, position.1, position.2)
    }
    fn has_neighbor_signal(&mut self, position: BlockPosition) -> Result<bool> {
        mc121_rust::redstone::runtime_has_neighbor_signal(self.0, position)
    }
}
pub(super) fn callbacks_can_complete(world: &WorldBackend, plan: &InteractionPlan) -> Result<()> {
    callbacks_can_complete_except_furnace_removal(world, plan, None)
}
fn callbacks_can_complete_except_furnace_removal(world: &WorldBackend, plan: &InteractionPlan, furnace: Option<(i32,i32,i32)>) -> Result<()> {
    let WorldBackend::Fixture(flat) = world else {
        bail!("imported source has no native callback owner");
    };
    let mut staged = WorldBackend::Fixture(flat.clone());
    main_world_interactions::commit_fixture_plan(&mut staged, plan)?
        .context("native callback staging requires fixture")?;
    // These proofs only read states and redstone. Entity-placement facts are
    // deliberately unavailable in this callback-only query.
    let mut query = CallbackQuery(&mut staged);
    // Main appends every ordered Write as a FluidBlockChanged effect; its
    // drain executes the real fluid callback before redstone. Fluid-owned
    // LiquidBlock.onPlace must therefore not be claimed by redstone twice.
    for event in &plan.events {
        if let InteractionEvent::Write{position,old_state,state,..}=event {
            let own_removal = furnace == Some(*position)
                && mc121_rust::native_block_entities::furnace_kind(*old_state).is_some()
                && mc121_rust::native_block_entities::furnace_kind(*old_state) != mc121_rust::native_block_entities::furnace_kind(*state);
            // The dedicated removal host executes the old furnace callback.
            // New-state onPlace still needs its actual fluid/redstone owner.
            let old = if own_removal { 0 } else { *old_state };
            ensure!(mc121_rust::redstone::owns_change_callbacks_after_fluid(old,*state)?,"unported native onPlace/onRemove owner");
        }
        if let InteractionEvent::NeighbourNotice {
            position,
            source_state,
        } = event
        {
            ensure!(
                world_interactions::neighbour_notice_nonfluid_is_quiescent(
                    *position,
                    *source_state,
                    &mut query
                )?,
                "native edit reaches an active unported neighbour callback"
            );
        }
    }
    Ok(())
}
impl LivePlayers {
    pub(super) fn prepare_required_native_furnace_removal_source_edit(
        &mut self,
        world: &WorldBackend,
        at: (i32,i32,i32),
        plan: &InteractionPlan,
    ) -> Result<PreparedNativeSourceEdit> {
        self.require_current_native_source(world)?;
        let WorldBackend::Fixture(flat) = world else { bail!("native furnace removal requires fixture backend") };
        let replacement = plan.events.iter().find_map(|e| match e {
            InteractionEvent::Write { position, state, .. } if *position == at => Some(*state),
            _ => None,
        }).context("furnace removal write missing")?;
        let receipt = flat.native_block_entities().prepare_furnace_removal(at, flat.block(at.0,at.1,at.2), replacement)?
            .context("furnace removal owner absent")?;
        let mut drained = flat.clone();
        drained.native_block_entities_mut().drain_furnace_removal(&receipt)?;
        callbacks_can_complete_except_furnace_removal(&WorldBackend::Fixture(drained), plan, Some(at))?;
        let token = self.entity_source.as_ref().context("native furnace source absent")?
            .prepare_owned_furnace_removal_with_block_ticks(world, at, plan, &self.block_runtime)?;
        if token.requires_fire_owner()? { self.fire_runtime.require_healthy()?; }
        token.require_fluid_history(&self.world_interactions.fluid_shapes)?;
        Ok(token)
    }
    pub(super) fn prepare_native_source_edit(
        &mut self,
        world: &WorldBackend,
        plan: &InteractionPlan,
    ) -> Result<Option<PreparedNativeSourceEdit>> {
        let validation = self.prepare_required_native_source_edit(world, plan);
        match validation {
            Ok(token) => Ok(Some(token)),
            Err(error) if self.item_runtime.item_count() != 0
                || (self.entity_source.is_some() && self.gameplay.require_supported_floor().is_ok()) => Err(error),
            Err(_) => Ok(None),
        }
    }
    /// A player mutation must not publish a changed world after source
    /// validation failed just because no item entity currently exists.
    pub(super) fn prepare_required_native_source_edit(
        &mut self,
        world: &WorldBackend,
        plan: &InteractionPlan,
    ) -> Result<PreparedNativeSourceEdit> {
        self.gameplay.require_supported_floor()?;
        self.fluid_runtime.require_healthy()?;
        self.require_native_fire_source_owner()?;
        let WorldBackend::Fixture(flat) = world else {
            bail!("native source requires fixture backend");
        };
        self.chunk_runtime
            .native_lighting_mut()?
            .require_current(flat)?;
        self.entity_source
            .as_ref()
            .context("complete native item source unavailable")?
            .require_fluid_history(world, &self.world_interactions.fluid_shapes)?;
        callbacks_can_complete(world, plan)?;
        let token = self
            .entity_source
            .as_ref()
            .context("complete native item source unavailable")?
            .prepare_owned_edit_with_block_ticks(world, plan, &self.block_runtime)?;
        if token.requires_fire_owner()? {
            self.fire_runtime.require_healthy()?;
        }
        token.require_fluid_history(&self.world_interactions.fluid_shapes)?;
        Ok(token)
    }
    pub(super) fn observe_native_source_edit(
        &mut self,
        world: &WorldBackend,
        token: Option<PreparedNativeSourceEdit>,
        changed: bool,
    ) -> Result<()> {
        if let Some(source) = &mut self.entity_source {
            match token {
                Some(token) => source.observe_owned_commit(token, world)?,
                None if changed => source.invalidate(
                    "block mutation has no complete native callback/light/cache transaction",
                ),
                None => {}
            }
        }
        Ok(())
    }
    pub(super) fn complete_native_source_edits(&mut self, world: &WorldBackend) -> Result<()> {
        self.gameplay.require_supported_floor()?;
        self.fluid_runtime.require_healthy()?;
        self.require_native_fire_source_owner()?;
        let pending = self.world_interactions.pending_count();
        let Self {
            entity_source,
            chunk_runtime,
            world_interactions,
            ..
        } = self;
        entity_source
            .as_mut()
            .context("complete native item source unavailable")?
            .complete_owned_edits(
                world,
                chunk_runtime.native_lighting_mut()?,
                &world_interactions.fluid_shapes,
                pending,
            )
    }
    pub(super) fn require_current_native_source(&mut self, world: &WorldBackend) -> Result<()> {
        self.gameplay.require_supported_floor()?;
        self.fluid_runtime.require_healthy()?;
        self.require_native_fire_source_owner()?;
        ensure!(
            self.world_interactions.pending_count() == 0,
            "native world callbacks are still pending"
        );
        let WorldBackend::Fixture(flat) = world else {
            bail!("native item source cannot certify imported world");
        };
        self.chunk_runtime
            .native_lighting_mut()?
            .require_current(flat)?;
        self.entity_source
            .as_ref()
            .context("complete native item source unavailable")?
            .require_fluid_history(world, &self.world_interactions.fluid_shapes)?;
        self.entity_source
            .as_ref()
            .context("complete native item source unavailable")?
            .require_current_world(world)
    }
    fn require_native_fire_source_owner(&self) -> Result<()> {
        if self.block_runtime.pending_count()!=0||self.entity_source.as_ref().is_some_and(|s|s.requires_block_checkpoint()){self.block_runtime.require_healthy()?;self.redstone_runtime.require_healthy()?;}

        if self.fire_runtime.pending_count() != 0
            || self
                .entity_source
                .as_ref()
                .is_some_and(|source| source.requires_fire_checkpoint())
        {
            self.fire_runtime.require_healthy()?;
        }
        Ok(())
    }
}
