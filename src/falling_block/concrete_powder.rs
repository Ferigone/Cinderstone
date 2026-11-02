//! Pinned Java 1.21 ConcretePowderBlock callbacks, without a second world.
//!
//! Direct JDK21 javap of ConcretePowderBlock (`dhk`) supplies source30–85;
//! FallingBlock (`diu`) supplies inherited onPlace32–33/tick44–50. This is a
//! callable source kernel, not a canonical palette binding or Main delivery.

use super::block::{self, FallingBlockHost, OrdinaryFallingBlockHost, ShapeUpdate};
use crate::{geometry::Direction, placement::BlockPosition};
use anyhow::Result;

/// The actual invoked powder singleton and its final `concrete` Block field.
/// Capture these handles from the real registry/source association once. A
/// color-name convention, adjacent state ID or arbitrary default is not that
/// association. Host preflight must verify both identities against its owner.
/// There is deliberately no Default, palette lookup or replacement state here.
pub struct ConcretePowderBlock<B> {
    powder: B,
    concrete: B,
}

impl<B> ConcretePowderBlock<B> {
    pub fn from_source_association(powder: B, concrete: B) -> Self {
        Self { powder, concrete }
    }

    pub fn powder(&self) -> &B {
        &self.powder
    }

    pub fn concrete(&self) -> &B {
        &self.concrete
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConcretePowderAction {
    OnLand,
    Placement,
    UpdateShape,
}

/// Borrow the same actual BlockGetter/Level, state/fluid registries, shape
/// context, BLOCK scheduler and edit/publication owners as the enclosing call.
/// Methods have no permissive defaults. Missing capabilities return errors.
///
/// Native preflight is not an extra Java callback: it verifies the retained
/// source association and capability provenance without world queries, RNG,
/// schedules, writes or output. Finish fallible edit admission before a write;
/// set_block must execute genuine flags3 callbacks/light/schedule/publication
/// in their source order, not merely replace a palette entry or report success.
/// A hardening return from placement/updateShape is only a returned state; the
/// enclosing real placement/update owner performs any resulting world edit.
pub trait ConcretePowderHost: FallingBlockHost {
    type FluidState;
    type PlacementContext;
    /// The actual retained actor (or a capability borrowing that actor), not
    /// FallingBlockHost's constructor/admission token. onLand does not read it.
    type LandingEntity: ?Sized;

    fn preflight_concrete_powder(
        &mut self,
        action: ConcretePowderAction,
        binding: &ConcretePowderBlock<Self::Block>,
        position: BlockPosition,
    ) -> Result<()>;

    /// Read this actual BlockState.getFluidState(), including waterlogged and
    /// source/flowing states; do not replace it with a block-name predicate.
    fn block_fluid_state(&mut self, state: &Self::BlockState) -> Result<Self::FluidState>;

    /// Actual FluidState.is(FluidTags.WATER) from this world's tag owner. Lava,
    /// fluid height, rain and the dimension's ultrawarm flag are not substitutes.
    fn fluid_is_water_tag(&mut self, state: &Self::FluidState) -> Result<bool>;

    /// Actual candidateState.isFaceSturdy(the SAME BlockGetter, supplied
    /// position, face), using its source full-support/default SupportType path.
    /// The supplied position intentionally remains the powder's original pos.
    fn is_face_sturdy(
        &mut self,
        state: &Self::BlockState,
        position: BlockPosition,
        face: Direction,
    ) -> Result<bool>;

    /// Real canonical Block.defaultBlockState() for the retained concrete field.
    fn default_block_state(&mut self, block: &Self::Block) -> Result<Self::BlockState>;

    fn set_block(
        &mut self,
        position: BlockPosition,
        state: &Self::BlockState,
        flags: i32,
    ) -> Result<bool>;

    /// Observe BlockPlaceContext.getLevel() before getClickedPos(), requiring
    /// that it is this host's actual Level rather than creating a detached one.
    /// This performs no block query, mutation or dimension-dependent fallback.
    fn require_placement_level(&mut self, context: &Self::PlacementContext) -> Result<()>;
    fn placement_position(&mut self, context: &Self::PlacementContext) -> Result<BlockPosition>;

    /// Actual super.getStateForPlacement(context), invoked only on the dry
    /// branch with the original context and invoked powder singleton.
    fn superclass_state_for_placement(
        &mut self,
        block: &Self::Block,
        context: &Self::PlacementContext,
    ) -> Result<Self::BlockState>;
}

/// ConcretePowderBlock.canSolidify, source76 (`dhk.n`, bytecode0–10).
pub fn can_solidify<H: ConcretePowderHost>(
    host: &mut H,
    state: &H::BlockState,
) -> Result<bool> {
    let fluid = host.block_fluid_state(state)?;
    host.fluid_is_water_tag(&fluid)
}

/// ConcretePowderBlock.touchesLiquid, source58–72. Preserve the literal
/// MutableBlockPos cursor and every read: at the start of each direction the
/// source reads the cursor left by the previous iteration. DOWN is first and
/// is skipped unless the original-position state can solidify. A successful
/// neighbor must be water-tagged AND not sturdy on its opposite face, queried
/// at the ORIGINAL powder position (not the neighbor position).
pub fn touches_liquid<H: ConcretePowderHost>(
    host: &mut H,
    position: BlockPosition,
) -> Result<bool> {
    let mut cursor = position;
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ] {
        let cursor_state = host.block_state(cursor)?;
        if direction == Direction::Down && !can_solidify(host, &cursor_state)? {
            continue;
        }
        let (dx, dy, dz) = direction.step();
        cursor = (
            position.0.wrapping_add(dx),
            position.1.wrapping_add(dy),
            position.2.wrapping_add(dz),
        );
        let neighbor = host.block_state(cursor)?;
        if can_solidify(host, &neighbor)?
            && !host.is_face_sturdy(&neighbor, position, direction.opposite())?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// ConcretePowderBlock.shouldSolidify, source54. Test the supplied captured
/// state BEFORE querying neighbors; this is not a fresh read at `position`.
pub fn should_solidify<H: ConcretePowderHost>(
    host: &mut H,
    position: BlockPosition,
    state: &H::BlockState,
) -> Result<bool> {
    if can_solidify(host, state)? {
        return Ok(true);
    }
    touches_liquid(host, position)
}

/// ConcretePowderBlock.onLand, source36–39 (`dhk.a`, bytecode0–24).
/// The fourth original argument is the captured replaced state. Neither the
/// carried state nor the actual falling entity is queried by this body. Keep
/// that real entity argument: this call must not construct a substitute actor.
/// The original flags3 setBlock boolean is intentionally ignored.
pub fn on_land<H: ConcretePowderHost>(
    host: &mut H,
    binding: &ConcretePowderBlock<H::Block>,
    position: BlockPosition,
    _falling_state: &H::BlockState,
    replaced_state: &H::BlockState,
    _falling_entity: &mut H::LandingEntity,
) -> Result<()> {
    host.preflight_concrete_powder(ConcretePowderAction::OnLand, binding, position)?;
    if should_solidify(host, position, replaced_state)? {
        let concrete = host.default_block_state(binding.concrete())?;
        let _ = host.set_block(position, &concrete, 3)?;
    }
    Ok(())
}

/// ConcretePowderBlock.getStateForPlacement, source43–50. Query the actual
/// current target before shouldSolidify; hardening returns the retained
/// concrete default state, otherwise delegate with the original context.
pub fn get_state_for_placement<H: ConcretePowderHost>(
    host: &mut H,
    binding: &ConcretePowderBlock<H::Block>,
    context: &H::PlacementContext,
) -> Result<H::BlockState> {
    host.require_placement_level(context)?;
    let position = host.placement_position(context)?;
    host.preflight_concrete_powder(ConcretePowderAction::Placement, binding, position)?;
    let replaced_state = host.block_state(position)?;
    if should_solidify(host, position, &replaced_state)? {
        host.default_block_state(binding.concrete())
    } else {
        host.superclass_state_for_placement(binding.powder(), context)
    }
}

/// ConcretePowderBlock.updateShape, source81–85. Unlike shouldSolidify this
/// calls touchesLiquid directly. A wet return does NOT schedule FallingBlock's
/// tick or call its superclass. A dry delegation retains every original shape
/// argument and schedules the same powder Block at delay2 before superclass.
pub fn update_shape<H: ConcretePowderHost>(
    host: &mut H,
    binding: &ConcretePowderBlock<H::Block>,
    update: ShapeUpdate<'_, H::BlockState>,
) -> Result<H::BlockState> {
    host.preflight_concrete_powder(
        ConcretePowderAction::UpdateShape,
        binding,
        update.position,
    )?;
    if touches_liquid(host, update.position)? {
        host.default_block_state(binding.concrete())
    } else {
        block::update_shape(host, binding.powder(), update)
    }
}

/// No ConcretePowderBlock onPlace override exists. Preserve inherited
/// FallingBlock's unconditional delay2 schedule; no hardening query is added.
pub fn on_place<H: FallingBlockHost>(
    host: &mut H,
    binding: &ConcretePowderBlock<H::Block>,
    state: &H::BlockState,
    position: BlockPosition,
) -> Result<()> {
    block::on_place(host, binding.powder(), state, position)
}

/// Reuse the source inherited FallingBlock tick and literal empty falling hook.
/// This never calls Anvil.setHurtsEntities. It still requires the same actual
/// constructor, replacement, addFreshEntity and returned-entity capability.
pub fn scheduled_tick<H: OrdinaryFallingBlockHost>(
    host: &mut H,
    binding: &ConcretePowderBlock<H::Block>,
    state: &H::BlockState,
    position: BlockPosition,
) -> Result<block::FallingTickOutcome> {
    block::scheduled_tick(host, binding.powder(), state, position)
}
