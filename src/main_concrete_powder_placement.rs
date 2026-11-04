//! Concrete-powder planning over the actual Main operation and planner reads.
//! No Level, entity, scheduler, source history or association registry is copied.
//! Shape planning returns ordered tick obligations; Main executes them later.
use super::*;
use super::main_falling_block_powder::{self, NativeConcretePowderOwner};
use mc121_rust::{
    falling_block::{block::{FallingBlockAction, FallingBlockHost, ShapeUpdate},
        concrete_powder::{self, ConcretePowderAction, ConcretePowderBlock, ConcretePowderHost}},
    fluid_physics::FluidState,
    geometry::Direction,
    placement::{self, BlockPosition, PlacementContext, PlacementResult, PlacementWorld, PlannedBlockTick, SupportType},
    world_interactions::{self, ConcretePowderShapePlan, InteractionPlan, InteractionWorld, RuntimeInteractionWorld},
};
use std::cell::RefCell;

/// Minted only after the genuine locked Main player/source/inventory checks.
/// The Level binding itself is the actual mutable query borrowed below.
pub(super) struct PlayerPowderOperation {
    actor: i32,
    uuid: [u8; 16],
    item: i32,
    hand: i32,
}
pub(super) fn prepare_player_operation(world: &WorldBackend, source: &NativeEntitySource,
    registry: &EntityRegistry, actor: i32, hand: i32, stack: &ItemStack,
) -> Result<PlayerPowderOperation> {
    source.require_owned_callback_world(world)?;
    ensure!(matches!(world, WorldBackend::Fixture(_)), "powder placement lacks its actual native Level");
    let player = registry.get(actor).context("powder placement's actual player is absent")?;
    ensure!(player.gamemode == 1 && (0..=1).contains(&hand) && !stack.is_empty(),
        "powder placement lacks the actual admitted creative player/item operation");
    Ok(PlayerPowderOperation { actor, uuid: player.profile.uuid, item: stack.item_id, hand })
}

/// Borrow the SAME real RuntimeInteractionWorld used by the enclosing action.
/// None is an explicit edit/preview operation, never a BlockPlaceContext Level.
pub(super) struct PowderPlanningWorld<'a, W> {
    world: &'a mut W,
    owner: &'a NativeConcretePowderOwner,
    player: Option<PlayerPowderOperation>,
    powder_used: bool,
}
impl<'a, W: InteractionWorld> PowderPlanningWorld<'a, W> {
    pub(super) fn for_player(world: &'a mut W, owner: &'a NativeConcretePowderOwner,
        operation: PlayerPowderOperation,
    ) -> Self { Self { world, owner, player: Some(operation), powder_used: false } }
    pub(super) fn for_edit(world: &'a mut W, owner: &'a NativeConcretePowderOwner) -> Self {
        Self { world, owner, player: None, powder_used: false }
    }
    pub(super) fn used_powder(&self) -> bool { self.powder_used }
}
impl<W: InteractionWorld> PlacementWorld for PowderPlanningWorld<'_, W> {
    fn block(&mut self, p: BlockPosition) -> Result<u32> { self.world.block(p) }
    fn min_build_height(&mut self) -> Result<i32> { self.world.min_build_height() }
    fn max_build_height(&mut self) -> Result<i32> { self.world.max_build_height() }
    fn has_neighbor_signal(&mut self, p: BlockPosition) -> Result<bool> { self.world.has_neighbor_signal(p) }
    fn inside_world_border(&mut self, p: BlockPosition) -> Result<bool> { self.world.inside_world_border(p) }
    fn raw_brightness(&mut self, p: BlockPosition, d: i32) -> Result<i32> { self.world.raw_brightness(p, d) }
    fn can_see_sky(&mut self, p: BlockPosition) -> Result<bool> { self.world.can_see_sky(p) }
    fn support(&mut self, p: BlockPosition, d: Direction, kind: SupportType) -> Result<bool> {
        self.world.support(p, d, kind)
    }
    fn collision_face_nonempty(&mut self, p: BlockPosition, d: Direction) -> Result<bool> {
        self.world.collision_face_nonempty(p, d)
    }
    fn collision_face_full(&mut self, p: BlockPosition, d: Direction) -> Result<bool> {
        self.world.collision_face_full(p, d)
    }
    fn collision_full(&mut self, p: BlockPosition) -> Result<bool> { self.world.collision_full(p) }
}
impl<W: InteractionWorld> InteractionWorld for PowderPlanningWorld<'_, W> {
    fn unobstructed(&mut self, state: u32, p: BlockPosition) -> Result<bool> {
        self.world.unobstructed(state, p)
    }
    fn plan_concrete_powder_placement(&mut self, placing: u32, context: &PlacementContext,
        overlay: &BTreeMap<BlockPosition, u32>, read_set: &mut BTreeMap<BlockPosition, u32>,
    ) -> Result<PlacementResult> {
        self.powder_used = true;
        let operation = self.player.as_ref().context("preview/edit has no actual player placement Level")?;
        ensure!((0..=1).contains(&operation.hand)
            && inventory::item_default_block_state(operation.item) == Some(placing),
            "powder selection differs from the actual original player/item operation");
        let _actual_player_identity = (operation.actor, operation.uuid);
        let binding = self.owner.binding_for_state(placing)?
            .context("selected powder has no installed canonical association")?;
        let level = PlanningReads { world: RefCell::new(&mut *self.world), overlay,
            read_set: RefCell::new(read_set) };
        let bound = LevelBoundPlacement { level: &level, coordinates: context, operation };
        let mut host = PowderPlanHost { level: &level, owner: self.owner,
            player: Some(operation), ticks: Vec::new() };
        let state = concrete_powder::get_state_for_placement(&mut host, binding, &bound)?;
        ensure!(host.ticks.is_empty(), "getStateForPlacement unexpectedly scheduled a tick");
        Ok(PlacementResult::State(state))
    }
    fn plan_concrete_powder_shape(&mut self, update: ShapeUpdate<'_, u32>,
        overlay: &BTreeMap<BlockPosition, u32>, read_set: &mut BTreeMap<BlockPosition, u32>,
    ) -> Result<ConcretePowderShapePlan> {
        self.powder_used = true;
        let binding = self.owner.binding_for_state(*update.state)?
            .context("shape powder has no installed canonical association")?;
        let level = PlanningReads { world: RefCell::new(&mut *self.world), overlay,
            read_set: RefCell::new(read_set) };
        let mut host = PowderPlanHost { level: &level, owner: self.owner,
            player: None, ticks: Vec::new() };
        let state = concrete_powder::update_shape(&mut host, binding, update)?;
        Ok(ConcretePowderShapePlan { state, scheduled_ticks: host.ticks })
    }
}

/// A scoped borrow of the planner's existing read view, not a second Level.
/// Method-local RefCell loans let the original bound context and kernel host
/// refer to this SAME actual query without a duplicated mutable world owner.
struct PlanningReads<'a> {
    world: RefCell<&'a mut dyn PlacementWorld>,
    overlay: &'a BTreeMap<BlockPosition, u32>,
    read_set: RefCell<&'a mut BTreeMap<BlockPosition, u32>>,
}
impl PlanningReads<'_> {
    fn block(&self, p: BlockPosition) -> Result<u32> {
        if let Some(&state) = self.overlay.get(&p) { return Ok(state); }
        let state = self.world.borrow_mut().block(p)?;
        self.read_set.borrow_mut().entry(p).or_insert(state);
        Ok(state)
    }
}
struct LevelBoundPlacement<'scope, 'world> {
    level: &'scope PlanningReads<'world>,
    coordinates: &'scope PlacementContext,
    operation: &'scope PlayerPowderOperation,
}
struct PowderPlanHost<'scope, 'world> {
    level: &'scope PlanningReads<'world>,
    owner: &'scope NativeConcretePowderOwner,
    player: Option<&'scope PlayerPowderOperation>,
    ticks: Vec<PlannedBlockTick>,
}
impl FallingBlockHost for PowderPlanHost<'_, '_> {
    type Block = u32;
    type BlockState = u32;
    type FallingEntity = std::convert::Infallible;
    fn preflight(&mut self, action: FallingBlockAction, block: &u32, state: &u32, _: BlockPosition) -> Result<()> {
        ensure!(action == FallingBlockAction::UpdateShape
            && placement::placement_block_min_state(*state)? == *block,
            "powder planning cannot execute a scheduled/spawn/onPlace callback");
        let binding = self.owner.binding_for_state(*state)?.context("unbound powder shape")?;
        self.owner.require_binding(binding)
    }
    fn schedule_block_tick(&mut self, p: BlockPosition, block: &u32, delay: i32) -> Result<()> {
        ensure!(delay == 2 && self.owner.binding_for_state(*block)?.is_some(), "foreign powder schedule obligation");
        self.ticks.push(PlannedBlockTick { position: p, state: *block, delay });
        Ok(())
    }
    fn superclass_update_shape(&mut self, update: ShapeUpdate<'_, u32>) -> Result<u32> {
        placement::placement_block_name(*update.neighbor_state)?;
        Ok(*update.state)
    }
    fn block_state(&mut self, p: BlockPosition) -> Result<u32> { self.level.block(p) }
    fn state_is_air(&mut self, _: &u32) -> Result<bool> { bail!("planner does not execute Falling tick") }
    fn state_is_fire_tag(&mut self, _: &u32) -> Result<bool> { bail!("planner does not execute Falling tick") }
    fn state_is_liquid(&mut self, _: &u32) -> Result<bool> { bail!("planner does not execute Falling tick") }
    fn state_can_be_replaced(&mut self, _: &u32) -> Result<bool> { bail!("planner does not execute Falling tick") }
    fn minimum_build_height(&mut self) -> Result<i32> { bail!("planner does not execute Falling tick") }
    fn fall_block(&mut self, _: BlockPosition, _: &u32) -> Result<Self::FallingEntity> { bail!("planner cannot create an actor") }
    fn set_hurts_entities(&mut self, entity: &mut Self::FallingEntity, _: f32, _: i32) -> Result<()> { match *entity {} }
}
impl<'scope, 'world> ConcretePowderHost for PowderPlanHost<'scope, 'world> {
    type FluidState = FluidState;
    type PlacementContext = LevelBoundPlacement<'scope, 'world>;
    type LandingEntity = std::convert::Infallible;
    fn preflight_concrete_powder(&mut self, action: ConcretePowderAction,
        binding: &ConcretePowderBlock<u32>, _: BlockPosition,
    ) -> Result<()> {
        self.owner.require_binding(binding)?;
        ensure!(action != ConcretePowderAction::OnLand, "planner has no actual landing actor/edit owner");
        if action == ConcretePowderAction::Placement {
            ensure!(self.player.is_some(), "powder placement lacks an actual player operation");
        }
        Ok(())
    }
    fn block_fluid_state(&mut self, state: &u32) -> Result<FluidState> { FluidState::from_block(*state) }
    fn fluid_is_water_tag(&mut self, state: &FluidState) -> Result<bool> { Ok(self.owner.fluid_is_water(*state)) }
    fn is_face_sturdy(&mut self, state: &u32, p: BlockPosition, face: Direction) -> Result<bool> {
        main_falling_block_powder::cached_face_sturdy(*state, p, face)
    }
    fn default_block_state(&mut self, block: &u32) -> Result<u32> { self.owner.default_state(*block) }
    fn set_block(&mut self, _: BlockPosition, _: &u32, _: i32) -> Result<bool> { bail!("planner cannot commit a Level write") }
    fn require_placement_level(&mut self, context: &Self::PlacementContext) -> Result<()> {
        ensure!(std::ptr::eq(context.level, self.level)
            && self.player.is_some_and(|player| std::ptr::eq(player, context.operation)),
            "BlockPlaceContext is not bound to the actual Main operation's Level query");
        Ok(())
    }
    fn placement_position(&mut self, context: &Self::PlacementContext) -> Result<BlockPosition> {
        Ok(context.coordinates.position)
    }
    fn superclass_state_for_placement(&mut self, block: &u32, context: &Self::PlacementContext) -> Result<u32> {
        self.require_placement_level(context)?;
        self.owner.binding_for_state(*block)?.context("powder superclass has a foreign Block")?;
        // FallingBlock inherits Block's literal defaultBlockState return.
        self.owner.default_state(*block)
    }
}

pub(super) fn plan_edit_with_powder(world: &mut WorldBackend, source: &NativeEntitySource,
    block_runtime: &main_block_ticks::MainBlockRuntime, position: BlockPosition, state: u32,
    flags: u8, border: CollisionBorder,
) -> Result<InteractionPlan> {
    source.require_owned_callback_world(world)?;
    let owner = block_runtime.concrete_powder_owner()?;
    let (plan, powder) = plan_edit_inner(world, owner, position, state, flags, border)?;
    if powder { require_bounded_commit(&plan)?; }
    Ok(plan)
}

/// The existing Main committer applies planned writes before callbacks. Until
/// an interleaved callback committer owns cascades, a powder operation may
/// contain at most one changed write. This is checked before any live write.
pub(super) fn require_bounded_commit(plan: &InteractionPlan) -> Result<()> {
    ensure!(plan.writes().count() <= 1,
        "powder cascade requires an actual interleaved multi-write callback owner");
    Ok(())
}

/// The player FIFO executes the complete fluid callback before its following
/// explicit onPlace obligation. Until that phase is split, admit powder only
/// when both the original and projected local fluid callbacks are quiescent.
/// The separately ordered retained-actor water executor does NOT use this gate.
pub(super) fn require_player_commit(world: &mut WorldBackend, plan: &InteractionPlan) -> Result<()> {
    use world_interactions::InteractionEvent;
    require_bounded_commit(plan)?;
    ensure!(!plan.events.iter().any(|event| matches!(event, InteractionEvent::ScheduleWaterTick { .. })),
        "player powder needs an actual interleaved fluid scheduling phase");
    for event in &plan.events {
        let InteractionEvent::Write { position, old_state, state, .. } = event else { continue };
        ensure!(FluidState::from_block(*old_state)?.is_empty() && FluidState::from_block(*state)?.is_empty(),
            "player powder target needs an actual interleaved fluid callback owner");
        for delta in [(0, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)] {
            let at = (position.0.wrapping_add(delta.0), position.1.wrapping_add(delta.1),
                position.2.wrapping_add(delta.2));
            let original = world.block(at.0, at.1, at.2)?;
            let projected = if at == *position { *state } else { original };
            ensure!(FluidState::from_block(original)?.is_empty() && FluidState::from_block(projected)?.is_empty(),
                "player powder neighborhood needs an actual interleaved fluid callback owner at {at:?}");
        }
    }
    Ok(())
}

/// Read-only projection only: this API returns obligations/read-set and grants
/// no source, Level-bound placement, constructor, scheduler or commit owner.
pub(super) fn plan_projected_edit_with_powder(world: &mut WorldBackend,
    owner: &NativeConcretePowderOwner, position: BlockPosition, state: u32, flags: u8,
    border: CollisionBorder,
) -> Result<InteractionPlan> {
    plan_edit_inner(world, owner, position, state, flags, border).map(|(plan, _)| plan)
}

fn plan_edit_inner(world: &mut WorldBackend, owner: &NativeConcretePowderOwner,
    position: BlockPosition, state: u32, flags: u8, border: CollisionBorder,
) -> Result<(InteractionPlan, bool)> {
    let mut base = RuntimeInteractionWorld::new(world, world::MIN_Y, world::MAX_Y + 1,
        border, |_, _| bail!("powder edit planning does not place an entity"))?;
    let mut query = PowderPlanningWorld::for_edit(&mut base, owner);
    let plan = world_interactions::plan_edit(position, state, flags, &mut query)?;
    Ok((plan, query.used_powder()))
}
