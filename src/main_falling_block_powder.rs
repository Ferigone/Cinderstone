//! Retained canonical ConcretePowderBlock bindings and real Main callbacks.
//!
//! The associations are the original final `dhk.b` Block fields captured by
//! PlacementCatalog, not a colour convention or a palette offset. WATER
//! membership is captured from the actual admitted configuration's fluid tag.
//! This file alone does not admit a new native class or install a Main owner.
use super::*;
use mc121_rust::{
    block_ticks,
    falling_block::block::{FallingBlockAction, FallingBlockHost, ShapeUpdate},
    falling_block::{concrete_powder::{self, ConcretePowderBlock}, FallingBlockState},
    fluid_physics::{self, FluidKind, FluidState},
    geometry::Direction,
    placement::{self, BlockPosition},
    tick::TickPriority,
};
use serde::Deserialize;
use super::{main_falling_block_runtime::ConstructedFallingBlock,
    main_falling_block_sequence::PreparedFallingEditSequence,
    main_falling_block_spawn::ActualFallingBlockEditServices};

#[derive(Deserialize)]
struct CapturedPowderCatalogue {
    minecraft: String,
    jar_sha256: String,
    blocks: Vec<CapturedPowderBlock>,
}
#[derive(Deserialize)]
struct CapturedPowderBlock {
    class: String,
    min: u32,
    max: u32,
    default: u32,
    hardened_state: Option<u32>,
    on_place_class: String,
    placement_class: String,
    falling_delay: Option<i32>,
}
struct CanonicalPowderBlocks {
    bindings: BTreeMap<u32, ConcretePowderBlock<u32>>,
    defaults: BTreeMap<u32, u32>,
    water_fluids: BTreeSet<i32>,
}

/// Install once from the actual outgoing Fixtures and retain this Arc owner.
/// A helper constructed from guessed state IDs is not this capability.
#[derive(Clone)]
pub(super) struct NativeConcretePowderOwner(Arc<CanonicalPowderBlocks>);

impl NativeConcretePowderOwner {
    pub(super) fn capture(fixtures: &Fixtures) -> Result<Self> {
        let pinned: Capture = serde_json::from_str(include_str!("../fixtures/configuration.json"))?;
        ensure!(pinned.version == "1.21" && pinned.protocol == 767 && pinned.complete,
            "foreign concrete-powder configuration capture");
        ensure!(pinned.packets.len() == fixtures.configuration.len(),
            "concrete-powder tag owner differs from the pinned native configuration");
        for (expected, actual) in pinned.packets.iter().zip(&fixtures.configuration) {
            ensure!(expected.id == actual.id && STANDARD.decode(&expected.payload)? == actual.payload,
                "concrete-powder requires the actual pinned configuration/tag snapshot");
        }
        let water_fluids = capture_water_fluids(&fixtures.configuration)?;
        let catalogue: CapturedPowderCatalogue = serde_json::from_str(include_str!(
            "../fixtures/placement-1.21.json"))?;
        ensure!(catalogue.minecraft == "1.21" && catalogue.jar_sha256 ==
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
            "foreign concrete-powder Block association catalogue");
        let mut defaults = BTreeMap::new();
        for block in &catalogue.blocks {
            ensure!(block.min <= block.default && block.default <= block.max
                && placement::placement_block_min_state(block.min)? == block.min
                && placement::placement_block_min_state(block.max)? == block.min
                && placement::placement_block_min_state(block.default)? == block.min
                && defaults.insert(block.min, block.default).is_none(),
                "concrete-powder canonical Block registry differs from its source capture");
            // The current admitted movement callbacks are inherited base
            // callbacks or pressure plates; neither adds velocity. The source
            // base speed-factor stage may only damp the preflight bound.
            for state in block.min..=block.max {
                let speed = mc121_rust::entity_move::block_facts(state)?.speed_factor();
                ensure!((0.0..=1.0).contains(&speed),
                    "powder bounded movement requires a nonaccelerating source speed factor");
            }
        }
        let mut bindings = BTreeMap::new();
        for powder in catalogue.blocks.iter().filter(|block| block.class == "dhk") {
            ensure!(powder.on_place_class == "diu" && powder.placement_class == "dhk"
                && powder.falling_delay == Some(2),
                "concrete-powder source inheritance differs from the pinned class");
            let hardened = powder.hardened_state.context(
                "actual ConcretePowderBlock final concrete association is absent")?;
            let concrete = catalogue.blocks.iter().find(|block|
                (block.min..=block.max).contains(&hardened)).context(
                "associated concrete default state lacks its canonical Block owner")?;
            ensure!(concrete.default == hardened && concrete.class == "dfy"
                && concrete.min != powder.min && concrete.on_place_class == "dtb",
                "actual ConcretePowderBlock association is not its captured concrete Block");
            ensure!(bindings.insert(powder.min, ConcretePowderBlock::from_source_association(
                powder.min, concrete.min)).is_none(), "duplicate actual powder singleton");
        }
        ensure!(bindings.len() == 16, "pinned ConcretePowderBlock association domain differs");
        Ok(Self(Arc::new(CanonicalPowderBlocks { bindings, defaults, water_fluids })))
    }

    pub(super) fn binding_for_state(&self, state: u32)
        -> Result<Option<&ConcretePowderBlock<u32>>> {
        let block = placement::placement_block_min_state(state)?;
        Ok(self.0.bindings.get(&block))
    }

    fn require_binding(&self, binding: &ConcretePowderBlock<u32>) -> Result<()> {
        let actual = self.0.bindings.get(binding.powder())
            .context("powder callback has no retained canonical association")?;
        ensure!(std::ptr::eq(actual, binding),
            "powder callback binding is not owned by this retained registry");
        Ok(())
    }

    fn default_state(&self, block: u32) -> Result<u32> {
        self.0.defaults.get(&block).copied().context("actual Block default state is absent")
    }

    fn fluid_is_water(&self, state: FluidState) -> bool {
        // Pinned Fluids.<clinit> registers EMPTY, FLOWING_WATER, WATER,
        // FLOWING_LAVA, LAVA in that order. Keep the actual configured tag set;
        // source and flowing FluidStates resolve to their genuine singleton.
        let fluid = match (state.kind, state.source) {
            (FluidKind::Empty, _) => 0,
            (FluidKind::Water, false) => 1,
            (FluidKind::Water, true) => 2,
            (FluidKind::Lava, false) => 3,
            (FluidKind::Lava, true) => 4,
        };
        self.0.water_fluids.contains(&fluid)
    }

    /// This bounded tick has no production COLLIDER/SOURCE_ONLY ray owner.
    /// Reject before begin_level_tick/time/gravity or movement. Current admitted
    /// movement callbacks only zero/damp velocity and source speed factors were
    /// verified at capture, so they cannot grow this value past the ray gate.
    /// This certifies this tick only, never the remainder of a falling path.
    pub(super) fn preflight_slow_tick(&self, actor: &FallingBlockState) -> Result<()> {
        self.binding_for_state(actor.block_state)?
            .context("powder trajectory lacks its retained canonical Block binding")?;
        let velocity = if actor.physical.no_gravity {actor.physical.velocity}
            else {actor.physical.velocity.plus(Vec3::new(0., -0.04, 0.))};
        ensure!(velocity.length_squared() <= 1.0,
            "fast concrete-powder trajectory requires the actual COLLIDER/SOURCE_ONLY water-ray owner");
        Ok(())
    }
}

fn bounded_count(reader: &mut Reader<'_>, label: &'static str) -> Result<usize> {
    let count = reader.varint()?;
    ensure!((0..=65_536).contains(&count), "invalid concrete-powder {label} count");
    Ok(count as usize)
}

fn capture_water_fluids(configuration: &[Packet]) -> Result<BTreeSet<i32>> {
    let mut water = None;
    let mut tags_packet = false;
    for packet in configuration.iter().filter(|packet| packet.id == 0x0d) {
        ensure!(!tags_packet, "duplicate actual tags packet for powder owner");
        tags_packet = true;
        let mut reader = Reader::new(&packet.payload);
        let registries = bounded_count(&mut reader, "tag registry")?;
        let mut names = BTreeSet::new();
        for _ in 0..registries {
            let registry = reader.string(32767)?;
            ensure!(names.insert(registry.clone()), "duplicate actual tag registry");
            let count = bounded_count(&mut reader, "tag")?;
            let mut tag_names = BTreeSet::new();
            for _ in 0..count {
                let name = reader.string(32767)?;
                ensure!(tag_names.insert(name.clone()), "duplicate actual fluid/block tag");
                let values = bounded_count(&mut reader, "tag entry")?;
                let is_water = registry == "minecraft:fluid" && name == "minecraft:water";
                let mut members = BTreeSet::new();
                for _ in 0..values {
                    let id = reader.varint()?;
                    ensure!(id >= 0, "negative actual tag member");
                    if is_water { ensure!(members.insert(id), "duplicate actual WATER member"); }
                }
                if is_water {
                    ensure!(water.replace(members).is_none(), "duplicate actual WATER tag");
                }
            }
        }
        reader.finish()?;
    }
    let water = water.context("actual configured fluid WATER tag is absent")?;
    ensure!(water == BTreeSet::from([1, 2]),
        "configured WATER tag differs from the pinned native fluid registry");
    Ok(water)
}

/// Exact cached FULL support is state-owned. An uncached state needs its live
/// dynamic shape provider; zero bits from an absent source cache are not false.
fn cached_face_sturdy(state: u32, _original_position: BlockPosition,
    face: Direction) -> Result<bool> {
    let facts = fluid_physics::block_facts(state)?;
    let faces = facts.cached_sturdy_faces.context(
        "powder hardening needs an actual uncached FULL support-shape provider")?;
    Ok(faces & (1 << face as u8) != 0)
}

enum PowderWorld<'a, 'level> {
    Actual {
        services: &'a mut ActualFallingBlockEditServices<'level>,
        landing: Option<&'a mut PreparedFallingEditSequence>,
    },
    /// A read-only algorithm preview over the projected first plan. It grants
    /// no source, scheduler, RNG, entity, write or publication capability.
    Preview(&'a mut WorldBackend),
}

struct MainConcretePowderHost<'a, 'level> {
    access: PowderWorld<'a, 'level>,
    owner: &'a NativeConcretePowderOwner,
    landing_position: Option<BlockPosition>,
}

impl MainConcretePowderHost<'_, '_> {
    fn world(&mut self) -> &mut WorldBackend {
        match &mut self.access {
            PowderWorld::Actual { services, .. } => &mut *services.world,
            PowderWorld::Preview(world) => world,
        }
    }
}

impl FallingBlockHost for MainConcretePowderHost<'_, '_> {
    type Block = u32;
    type BlockState = u32;
    type FallingEntity = ConstructedFallingBlock;

    fn preflight(&mut self, action: FallingBlockAction, block: &u32, state: &u32,
        position: BlockPosition) -> Result<()> {
        ensure!(self.owner.0.bindings.contains_key(block)
            && placement::placement_block_min_state(*state)? == *block,
            "powder inherited callback has a foreign canonical Block");
        ensure!(matches!(action, FallingBlockAction::OnPlace | FallingBlockAction::UpdateShape),
            "powder construction requires its genuine full retained-runtime owner");
        let PowderWorld::Actual { services, .. } = &mut self.access else {
            bail!("powder algorithm preview cannot admit a production callback")
        };
        services.context.dimension.require_runtime_layout()?;
        services.gameplay.require_supported_floor()?;
        services.block_runtime.require_healthy()?;
        services.source.as_ref().context("actual powder callback source is absent")?
            .require_owned_callback_world(services.world)?;
        ensure!(services.block_runtime.owns_falling_tick(*block, 2)?
            && services.block_runtime.queue.chunk(block_ticks::pos(position).chunk()).is_some(),
            "powder callback lacks its actual configured BLOCK callback/save owner");
        Ok(())
    }

    fn schedule_block_tick(&mut self, position: BlockPosition, block: &u32,
        delay: i32) -> Result<()> {
        ensure!(delay == 2 && self.owner.0.bindings.contains_key(block),
            "powder callback changed the captured Block/delay");
        let PowderWorld::Actual { services, .. } = &mut self.access else {
            bail!("powder preview cannot schedule a production tick")
        };
        services.block_runtime.mark_used();
        services.source.as_mut().context("actual powder source is absent")?
            .observe_block_scheduler_use();
        block_ticks::schedule(&mut services.block_runtime.queue, position, *block,
            services.now, delay, TickPriority::Normal, &mut services.fluid_runtime.sub_tick_order)?;
        Ok(())
    }

    fn superclass_update_shape(&mut self, update: ShapeUpdate<'_, u32>) -> Result<u32> {
        ensure!(self.owner.binding_for_state(*update.state)?.is_some(),
            "powder superclass shape callback has a foreign state");
        placement::placement_block_name(*update.neighbor_state)?;
        // dhk's dry branch invokes diu, which schedules before reaching
        // BlockBehaviour's literal unchanged-state return with these args.
        Ok(*update.state)
    }

    fn block_state(&mut self, position: BlockPosition) -> Result<u32> {
        self.world().block(position.0, position.1, position.2)
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
    fn minimum_build_height(&mut self) -> Result<i32> {
        match &self.access {
            PowderWorld::Actual { services, .. } => Ok(services.context.dimension.min_y),
            PowderWorld::Preview(_) => bail!("powder preview has no actual dimension owner"),
        }
    }
    fn fall_block(&mut self, _position: BlockPosition, _state: &u32)
        -> Result<ConstructedFallingBlock> {
        bail!("powder callback parts do not substitute for the actual Falling constructor/runtime")
    }
    fn set_hurts_entities(&mut self, _entity: &mut ConstructedFallingBlock,
        _amount: f32, _maximum: i32) -> Result<()> {
        bail!("ConcretePowderBlock does not invoke the Anvil-only hurt setter")
    }
}

impl concrete_powder::ConcretePowderHost for MainConcretePowderHost<'_, '_> {
    type FluidState = FluidState;
    type PlacementContext = placement::PlacementContext;
    type LandingEntity = FallingBlockState;

    fn preflight_concrete_powder(&mut self, action: concrete_powder::ConcretePowderAction,
        binding: &ConcretePowderBlock<u32>, position: BlockPosition) -> Result<()> {
        self.owner.require_binding(binding)?;
        let PowderWorld::Actual { services, landing } = &mut self.access else {
            bail!("powder preview cannot admit an actual callback")
        };
        services.context.dimension.require_runtime_layout()?;
        services.gameplay.require_supported_floor()?;
        services.source.as_ref().context("actual powder source is absent")?
            .require_owned_callback_world(services.world)?;
        match action {
            concrete_powder::ConcretePowderAction::OnLand => ensure!(landing.is_some()
                && self.landing_position == Some(position),
                "powder onLand requires its admitted original two-write phase"),
            concrete_powder::ConcretePowderAction::UpdateShape => {},
            concrete_powder::ConcretePowderAction::Placement =>
                bail!("powder placement needs the original actual Level-bound BlockPlaceContext"),
        }
        Ok(())
    }
    fn block_fluid_state(&mut self, state: &u32) -> Result<FluidState> {
        FluidState::from_block(*state)
    }
    fn fluid_is_water_tag(&mut self, state: &FluidState) -> Result<bool> {
        Ok(self.owner.fluid_is_water(*state))
    }
    fn is_face_sturdy(&mut self, state: &u32, original_position: BlockPosition,
        face: Direction) -> Result<bool> {
        cached_face_sturdy(*state, original_position, face)
    }
    fn default_block_state(&mut self, block: &u32) -> Result<u32> {
        self.owner.default_state(*block)
    }
    fn set_block(&mut self, position: BlockPosition, state: &u32, flags: i32) -> Result<bool> {
        let PowderWorld::Actual { services, landing: Some(sequence) } = &mut self.access else {
            bail!("powder hardening lacks its admitted actual sequential edit owner")
        };
        sequence.commit_next(services, position, *state, flags)
    }
    fn require_placement_level(&mut self, _context: &placement::PlacementContext) -> Result<()> {
        bail!("coordinate-only placement context does not prove its actual Level owner")
    }
    fn placement_position(&mut self, _context: &placement::PlacementContext)
        -> Result<BlockPosition> {
        bail!("powder placement has no admitted original Level-bound context")
    }
    fn superclass_state_for_placement(&mut self, _block: &u32,
        _context: &placement::PlacementContext) -> Result<u32> {
        bail!("powder placement has no admitted original Level-bound superclass call")
    }
}

/// Read the actual world through the exact pinned hardening kernel. This
/// helper grants no edit or scheduler capability and draws no random values.
pub(super) fn should_solidify(owner: &NativeConcretePowderOwner,
    world: &mut WorldBackend, position: BlockPosition, captured_state: u32) -> Result<bool> {
    let mut host = MainConcretePowderHost {
        access: PowderWorld::Preview(world), owner, landing_position: None,
    };
    concrete_powder::should_solidify(&mut host, position, &captured_state)
}

/// Same actual BLOCK/subtick owner, no separate onPlace hardening query.
pub(super) fn on_place(services: &mut ActualFallingBlockEditServices<'_>,
    owner: &NativeConcretePowderOwner, state: u32, position: BlockPosition) -> Result<()> {
    let binding = owner.binding_for_state(state)?.context("onPlace state is not actual powder")?;
    let mut host = MainConcretePowderHost {access: PowderWorld::Actual {
        services, landing: None}, owner, landing_position: None};
    concrete_powder::on_place(&mut host, binding, &state, position)
}

/// Return wet concrete without a Falling schedule; dry uses the source
/// schedule-before-superclass sequence. The real caller owns the returned edit.
pub(super) fn update_shape(services: &mut ActualFallingBlockEditServices<'_>,
    owner: &NativeConcretePowderOwner, update: ShapeUpdate<'_, u32>) -> Result<u32> {
    let binding = owner.binding_for_state(*update.state)?.context("updateShape state is not actual powder")?;
    let mut host = MainConcretePowderHost {access: PowderWorld::Actual {
        services, landing: None}, owner, landing_position: None};
    concrete_powder::update_shape(&mut host, binding, update)
}

/// One real landing's admitted source writes. The enclosing retained tick
/// keeps the same actual actor through first commit, broadcast and discard.
/// This token never constructs/copies an actor, Level, RNG or entity index.
pub(super) struct PowderLandingAdmission {
    owner: NativeConcretePowderOwner,
    original_block: u32,
    position: BlockPosition,
    carried_state: u32,
    replaced_state: u32,
    entity_id: i32,
    entity_uuid: [u8; 16],
    sequence: PreparedFallingEditSequence,
    first_committed: bool,
    first_changed: bool,
    callback_completed: bool,
}

pub(super) fn prepare_landing(services: &mut ActualFallingBlockEditServices<'_>,
    owner: &NativeConcretePowderOwner, actor: &FallingBlockState, original_block: u32,
    position: BlockPosition, carried_state: u32, replaced_state: u32, flags: i32)
    -> Result<PowderLandingAdmission> {
    ensure!(flags == 3 && !actor.physical.removed && actor.block_data.is_none(),
        "powder landing requires the actual pre-discard flags3 actor phase");
    let binding = owner.binding_for_state(carried_state)?
        .context("landing carried state lacks its actual ConcretePowderBlock owner")?;
    ensure!(*binding.powder() == original_block && actor.block_state == carried_state,
        "powder landing lost its captured original Block/carried state");
    ensure!(services.world.block(position.0, position.1, position.2)? == replaced_state,
        "powder landing captured replaced state differs before its first write");
    let first = services.preflight_block_edit(position, carried_state, flags)?;
    let WorldBackend::Fixture(flat) = &*services.world else {
        bail!("powder sequential admission requires the actual native world")
    };
    let mut projected = WorldBackend::Fixture(flat.clone());
    main_world_interactions::commit_fixture_plan(&mut projected, &first)?
        .context("powder projected admission needs its native ordered-write backend")?;
    let following = if should_solidify(owner, &mut projected, position, replaced_state)? {
        Some((position, owner.default_state(*binding.concrete())?, 3))
    } else {None};
    let sequence = PreparedFallingEditSequence::prepare(services, position, carried_state,
        flags, first, following)?;
    Ok(PowderLandingAdmission {owner: owner.clone(), original_block, position,
        carried_state, replaced_state, entity_id: actor.physical.entity_id,
        entity_uuid: actor.physical.uuid, sequence, first_committed: false,
        first_changed: false, callback_completed: false})
}

impl PowderLandingAdmission {
    pub(super) fn commit_first(&mut self, services: &mut ActualFallingBlockEditServices<'_>)
        -> Result<bool> {
        ensure!(!self.first_committed && !self.callback_completed,
            "powder first landing write was already attempted");
        let changed = self.sequence.commit_next(services, self.position, self.carried_state, 3)?;
        self.first_committed = true;
        self.first_changed = changed;
        if !changed {self.sequence.verify_complete()?;}
        Ok(changed)
    }

    /// Invoke after the original actual block-update broadcast and discard.
    /// The kernel ignores its same retained actor argument exactly as Java.
    pub(super) fn on_land(&mut self, services: &mut ActualFallingBlockEditServices<'_>,
        actor: &mut FallingBlockState, original_block: u32, position: BlockPosition,
        carried_state: u32, replaced_state: u32) -> Result<()> {
        ensure!(self.first_committed && self.first_changed && !self.callback_completed
            && actor.physical.removed && actor.physical.entity_id == self.entity_id
            && actor.physical.uuid == self.entity_uuid && original_block == self.original_block
            && position == self.position && carried_state == self.carried_state
            && replaced_state == self.replaced_state,
            "powder onLand is outside its admitted same-actor source phase");
        let binding = self.owner.binding_for_state(carried_state)?
            .context("admitted powder binding disappeared")?;
        let mut host = MainConcretePowderHost {access: PowderWorld::Actual {
            services, landing: Some(&mut self.sequence)}, owner: &self.owner,
            landing_position: Some(position)};
        concrete_powder::on_land(&mut host, binding, position, &carried_state,
            &replaced_state, actor)?;
        self.sequence.verify_complete()?;
        self.callback_completed = true;
        Ok(())
    }

    pub(super) fn verify_complete(&self) -> Result<()> {
        ensure!(self.first_committed && (!self.first_changed || self.callback_completed),
            "powder landing left its actual onLand phase incomplete");
        self.sequence.verify_complete()
    }
}
