//! Ordered landing edits admitted before the first actual world mutation.
//!
//! Projection below is planning only. Production callbacks, source history,
//! BLOCK/fluid queues, light, random counters and publication continue through
//! the retained ActualFallingBlockEditServices and its real commit_plan.
use super::*;
use super::main_falling_block_spawn::ActualFallingBlockEditServices;
use mc121_rust::{
    entity_persistence::PreparedNativeSourceEdit,
    fluid_physics::FluidState,
    placement::BlockPosition,
    world_interactions::{self, InteractionEvent, InteractionPlan, RuntimeInteractionWorld},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EditRequest {
    position: BlockPosition,
    state: u32,
    flags: i32,
}

struct AdmittedEdit {
    request: EditRequest,
    plan: InteractionPlan,
}

/// A private, single-use receipt, not a replacement native source owner. Its
/// source admission is the existing real PreparedNativeSourceEdit calculated
/// from the complete ordered event stream while the actual source is locked.
/// The caller retains those same services and the actual landing actor across
/// the first setBlock, broadcast/discard, and subsequent onLand callback.
pub(super) struct PreparedFallingEditSequence {
    _source_admission: PreparedNativeSourceEdit,
    expected_revision: u64,
    first: Option<AdmittedEdit>,
    following: Option<AdmittedEdit>,
    first_result: Option<bool>,
    stopped: bool,
}

impl PreparedFallingEditSequence {
    pub(super) fn prepare(
        services: &mut ActualFallingBlockEditServices<'_>,
        position: BlockPosition,
        first_state: u32,
        flags: i32,
        first: InteractionPlan,
        following: Option<(BlockPosition, u32, i32)>,
    ) -> Result<Self> {
        let first_request = EditRequest { position, state: first_state, flags };
        revalidate_first(services, &first, first_request)?;
        // A false first setBlock never invokes onLand in FallingBlockEntity.
        let following = if first.events.is_empty() {
            None
        } else {
            following.map(|(position, state, flags)| {
                let plan = services.preflight_following_block_edit(&first, position, state, flags)?;
                Ok::<_, anyhow::Error>(AdmittedEdit {
                    request: EditRequest { position, state, flags }, plan,
                })
            }).transpose()?
        };
        let combined = combine_ordered_plans(services.world, &first,
            following.as_ref().map(|edit| &edit.plan))?;
        let source_admission = services.source.as_ref().context("falling sequence source is absent")?
            .prepare_owned_edit_with_block_ticks(services.world, &combined, services.block_runtime)?;
        ensure!(!source_admission.requires_fire_owner()?,
            "falling sequence requires an actual fire callback owner");
        source_admission.require_fluid_history(&services.interactions.fluid_shapes)?;
        let revision_steps = u64::from(!first.events.is_empty())
            + u64::from(following.as_ref().is_some_and(|edit| !edit.plan.events.is_empty()));
        services.world_revision.checked_add(revision_steps)
            .context("falling sequence world revision exhausted")?;
        // Only the real counter owner prepares its initial section image.
        // It observes the two actual writes later, at their original commits.
        services.random_ticks.prepare_plan(services.world, &combined)?;
        Ok(Self {
            _source_admission: source_admission,
            expected_revision: *services.world_revision,
            first: Some(AdmittedEdit { request: first_request, plan: first }),
            following,
            first_result: None,
            stopped: false,
        })
    }

    /// Consume precisely the next source request through the real services.
    /// A failed commit poisons this receipt: completed effects cannot be
    /// retried as though they were an unobserved planning operation.
    pub(super) fn commit_next(
        &mut self,
        services: &mut ActualFallingBlockEditServices<'_>,
        position: BlockPosition,
        state: u32,
        flags: i32,
    ) -> Result<bool> {
        ensure!(!self.stopped, "falling sequence stopped after an actual commit error");
        ensure!(*services.world_revision == self.expected_revision,
            "actual world revision changed between falling sequence phases");
        services.source.as_ref().context("falling sequence source is absent")?
            .require_current_world(services.world)?;
        let is_first = self.first.is_some();
        let next = if is_first { self.first.as_ref() } else { self.following.as_ref() }
            .context("falling sequence has no remaining actual edit")?;
        ensure!(next.request == EditRequest { position, state, flags },
            "falling sequence request differs from its admitted source phase");
        let next_revision = self.expected_revision.checked_add(u64::from(!next.plan.events.is_empty()))
            .context("falling sequence world revision exhausted")?;
        let next = if is_first { self.first.take() } else { self.following.take() }
            .expect("validated falling sequence phase");
        self.stopped = true;
        // commit_plan checks the current real read set/source again, executes
        // its callbacks, and records its own genuine per-stage source token.
        let changed = services.commit_plan(next.plan)?;
        ensure!(*services.world_revision == next_revision,
            "falling commit did not retain its actual revision phase");
        self.expected_revision = next_revision;
        if is_first {
            self.first_result = Some(changed);
            if !changed { self.following = None; }
        }
        self.stopped = false;
        Ok(changed)
    }

    pub(super) fn first_result(&self) -> Option<bool> { self.first_result }

    /// The real onLand branch must consume a pre-admitted following write.
    /// If the first setBlock returned false, that callback is never required.
    pub(super) fn verify_complete(&self) -> Result<()> {
        ensure!(!self.stopped && self.first.is_none() && self.following.is_none(),
            "falling sequence has an unfinished or failed actual source phase");
        Ok(())
    }
}

impl ActualFallingBlockEditServices<'_> {
    /// Plan against the first write's projected state, without substituting
    /// that private copy for this service's Level, source, light or queues.
    /// Initial scope admits only a dry, single-target, quiescent following
    /// transition; active adjacent water/redstone/BE/loot remains unsupported.
    pub(super) fn preflight_following_block_edit(
        &mut self,
        first: &InteractionPlan,
        position: BlockPosition,
        state: u32,
        flags: i32,
    ) -> Result<InteractionPlan> {
        let first_request = primary_request(first)?;
        ensure!(first_request.position == position,
            "following falling edit must retain the original landing position");
        revalidate_first(self, first, first_request)?;
        ensure!(flags == 3, "following falling setBlock must retain source flags3");
        let mut projected = project(self.world, first)?;
        // This SAME installed association is borrowed only for projected
        // planning. The helper grants no actual Level/source/callback owner;
        // combined admission below still uses the real world and source.
        let owner = self.block_runtime.concrete_powder_owner()?;
        let plan = super::main_concrete_powder_placement::plan_projected_edit_with_powder(
            &mut projected, owner, position, state, 3, self.border)?;
        validate_dry_following(&mut projected, &plan, position, self.border)?;
        // This is the real source/BLOCK admission for BOTH ordered writes,
        // not a source object constructed or advanced on the projection.
        let combined = combine_ordered_plans(self.world, first, Some(&plan))?;
        let token = self.source.as_ref().context("falling sequence source is absent")?
            .prepare_owned_edit_with_block_ticks(self.world, &combined, self.block_runtime)?;
        ensure!(!token.requires_fire_owner()?, "falling sequence needs an actual fire owner");
        token.require_fluid_history(&self.interactions.fluid_shapes)?;
        self.world_revision.checked_add(u64::from(!first.events.is_empty())
            + u64::from(!plan.events.is_empty())).context("falling sequence world revision exhausted")?;
        self.random_ticks.prepare_plan(self.world, &combined)?;
        Ok(plan)
    }
}

fn primary_request(first: &InteractionPlan) -> Result<EditRequest> {
    ensure!(first.events.iter().filter(|event| matches!(event, InteractionEvent::Write { .. })).count() == 1,
        "following falling profile requires one actual first target write");
    let Some(InteractionEvent::Write { position, state, flags, .. }) = first.events.first() else {
        bail!("following falling edit lacks its actual first write phase")
    };
    Ok(EditRequest { position: *position, state: *state, flags: i32::from(*flags) })
}

fn revalidate_first(services: &mut ActualFallingBlockEditServices<'_>,
    first: &InteractionPlan, request: EditRequest) -> Result<()> {
    ensure!(services.preflight_block_edit(request.position, request.state, request.flags)? == *first,
        "actual first falling plan changed before sequence admission");
    Ok(())
}

fn project(world: &WorldBackend, plan: &InteractionPlan) -> Result<WorldBackend> {
    let WorldBackend::Fixture(flat) = world else {
        bail!("falling sequence projection requires the actual native backend")
    };
    let mut projected = WorldBackend::Fixture(flat.clone());
    main_world_interactions::commit_fixture_plan(&mut projected, plan)?
        .context("falling sequence projection has no native planning backend")?;
    Ok(projected)
}

fn validate_dry_following(projected: &mut WorldBackend, plan: &InteractionPlan,
    position: BlockPosition, border: CollisionBorder) -> Result<()> {
    let mut after = project(projected, plan)?;
    let mut writes = 0;
    let mut notices = 0;
    for event in &plan.events {
        match event {
            InteractionEvent::Write { position: at, old_state, state, flags } => {
                writes += 1;
                ensure!(*at == position && *flags == 3,
                    "following falling profile needs its one actual flags3 target write");
                let old = mc121_rust::redstone::block(*old_state)?;
                let new = mc121_rust::redstone::block(*state)?;
                ensure!((old.min == new.min || old.on_remove == "dtb") && new.on_place == "dtb",
                    "following falling edit needs an active actual block callback owner");
                for value in [*old_state, *state] {
                    ensure!(FluidState::from_block(value)?.is_empty()
                        && !placement::in_block_tag(value, "minecraft:fire")
                        && !mc121_rust::fire_simulation::has_block_entity(value)?,
                        "following falling edit needs actual fluid/fire/block-entity lifecycle");
                }
            }
            InteractionEvent::NeighbourNotice { position: at, source_state } => {
                notices += 1;
                ensure!(*at == position, "following falling notice changed its actual target");
                for direction in mc121_rust::redstone::NEIGHBOR_ORDER {
                    let at = mc121_rust::redstone::step(*at, direction);
                    let value = after.block(at.0, at.1, at.2)?;
                    ensure!(mc121_rust::redstone::block(value)?.neighbor == "dtb"
                        && FluidState::from_block(value)?.is_empty()
                        && !placement::in_block_tag(value, "minecraft:fire")
                        && !mc121_rust::fire_simulation::has_block_entity(value)?,
                        "following falling edit reaches an active neighboring owner");
                }
                let mut query = RuntimeInteractionWorld::new(&mut after, world::MIN_Y,
                    world::MAX_Y + 1, border,
                    |_, _| bail!("following callback proof has no entity placement"))?;
                ensure!(world_interactions::neighbour_notice_is_quiescent(*at, *source_state, &mut query)?,
                    "following falling edit needs an active actual shape callback owner");
            }
            _ => bail!("following falling edit needs an additional actual effect/scheduler owner"),
        }
    }
    ensure!((writes == 0 && plan.events.is_empty()) || (writes == 1 && notices == 1),
        "following falling edit exceeds its single quiescent target phase");
    Ok(())
}

fn combine_ordered_plans(world: &mut WorldBackend, first: &InteractionPlan,
    following: Option<&InteractionPlan>) -> Result<InteractionPlan> {
    let mut combined = first.clone();
    if let Some(following) = following {
        let mut projected = project(world, first)?;
        for (position, expected) in &following.read_set {
            ensure!(projected.block(position.0, position.1, position.2)? == *expected,
                "following falling read no longer matches its first-plan projection");
            // A combined initial read receipt uses the real initial value;
            // ordered Write.old_state still retains the intermediate value.
            let initial = world.block(position.0, position.1, position.2)?;
            if let Some(existing) = combined.read_set.insert(*position, initial) {
                ensure!(existing == initial, "first falling read changed during sequence planning");
            }
        }
        combined.events.extend(following.events.iter().cloned());
        combined.changed_positions.extend(following.changed_positions.iter().copied());
    }
    Ok(combined)
}
