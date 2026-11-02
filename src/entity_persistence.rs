//! Native entity checkpoints, not Java entity NBT import or chunk-future saves.
//!
//! NativeResume preserves retained RNGs and complete Item/XP/Falling bodies.
//! Restored entities start hidden: only new, confirmed chunk callbacks may make
//! them tick or track. Network sender baselines and live player sessions reset.
use crate::{
    random::LegacyRandomSource,
    runtime_entities::RuntimeEntities,
    runtime_world::RuntimeWorld,
    tick::{TickClock, WeatherState},
    world::BlockChange,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

#[path = "entity_persistence_dto.rs"]
mod dto;
use dto::SavedItem;

/// One authoritative non-player entity cohort, covered by the SAME atomic
/// EntityCheckpoint checksum as item bodies and source world/time. Legacy
/// SavedWorld XP is a migration input only when this payload is absent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeEntityCohort {
    pub experience_orbs: Vec<crate::experience::SavedExperienceOrb>,
    pub falling_blocks: Vec<crate::falling_block::persistence::FallingBlockSnapshot>,
    pub order: crate::runtime_entities::PersistentEntityOrder,
}

const FORMAT: &str = "mc121-rust-native-items";
const VERSION: u32 = 1;
const MAX_CHECKPOINT_BYTES: u64 = 256 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// No Gaussian cache is implemented by the native LegacyRandomSource. Restore
/// the existing sequential48-bit state by inverting its constructor xor; this
/// consumes no RNG draw and never uses the old initial seed as a new seed.
fn restore_random(state: u64) -> Result<LegacyRandomSource> {
    ensure!(
        state < (1u64 << 48),
        "invalid persisted LegacyRandomSource state"
    );
    Ok(LegacyRandomSource::new((state ^ 25_214_903_917) as i64))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldImage {
    base_states: [u32; 4],
    biome: u32,
    changes: Vec<BlockChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    containers: Vec<crate::native_block_entities::ContainerImage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    furnaces: Vec<crate::native_block_entities::FurnaceImage>,
}
impl WorldImage {
    fn capture(world: &RuntimeWorld) -> Result<Self> {
        let RuntimeWorld::Fixture(flat) = world else {
            anyhow::bail!("native item checkpoint cannot certify Anvil entity sources")
        };
        Ok(Self {
            base_states: flat.base_states(),
            biome: flat.biome_id(),
            changes: flat.changes(),
            containers: flat.native_block_entities().images(),
            furnaces: flat.native_block_entities().furnace_images(),
        })
    }
    fn is_canonical_unedited(&self) -> bool {
        self.base_states == [0, 1, 10, 9] && self.biome == 39 && self.changes.is_empty() && self.containers.is_empty() && self.furnaces.is_empty()
    }
    fn validate_native_base(&self) -> Result<()> {
        ensure!(
            self.base_states == [0, 1, 10, 9] && self.biome == 39,
            "source is outside the owned canonical native base"
        );
        let mut positions = BTreeSet::new();
        for change in &self.changes {
            ensure!(
                (crate::world::MIN_Y..=crate::world::MAX_Y).contains(&change.y),
                "source block outside build bounds"
            );
            ensure!(
                positions.insert((change.x, change.y, change.z)),
                "duplicate source block change"
            );
            crate::placement::placement_block_name(change.state)?;
        }
        Ok(())
    }
    fn has_fluid(&self) -> Result<bool> {
        for state in self
            .base_states
            .into_iter()
            .chain(self.changes.iter().map(|change| change.state))
        {
            if !crate::fluid_physics::FluidState::from_block(state)?.is_empty() {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn has_fire(&self) -> Result<bool> {
        for state in self
            .base_states
            .into_iter()
            .chain(self.changes.iter().map(|change| change.state))
        {
            if matches!(
                crate::placement::placement_block_name(state)?,
                "minecraft:fire" | "minecraft:soul_fire"
            ) {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn validate_item_runtime_scope(&self) -> Result<()> {
        self.validate_native_base()?;
        let states = self
            .base_states
            .into_iter()
            .chain(self.changes.iter().map(|change| change.state))
            .collect::<BTreeSet<_>>();
        for state in states {
            // Source classification is distinct from callback capability.
            // Pressure states are admitted only by their literal dfm/dlw/dpp
            // dispatch; a runtime lacking the actual host still fails at the
            // synchronous move callback, never receiving a successful default.
            ensure!(
                crate::runtime_item_world::supports_block_callbacks(state)?,
                "native item source contains an unsupported movement callback at state {state}"
            );
            ensure!(
                crate::geometry::static_block_collision_boxes(state).is_some(),
                "native item source contains an unowned dynamic collision shape at state {state}"
            );
        }
        Ok(())
    }
}

/// Owned source history. Constructed from actual absent native files or from a
/// validated complete checkpoint. It cannot be recreated from an empty diff.
/// Call invalidate for EVERY unsupported edit/load, including edit then revert.
#[derive(Debug)]
pub struct NativeEntitySource {
    history: SourceHistory,
    pending_world: Option<WorldImage>,
    pending_fluid_seen: bool,
    pending_fire_seen: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceHistory {
    origin: String,
    complete_native_items: bool,
    invalidated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    certified_world: Option<WorldImage>,
    #[serde(default, skip_serializing_if = "is_false")]
    requires_fluid_checkpoint: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    requires_fire_checkpoint: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    requires_block_checkpoint: bool,
}
fn is_false(value: &bool) -> bool {
    !*value
}
/// Prepared while the actual old world is locked. The target is calculated
/// from every ordered write, including its old state, rather than trusting a
/// caller-supplied summary or a final diff after an unobserved mutation.
pub struct PreparedNativeSourceEdit {
    before: WorldImage,
    after: WorldImage,
    requires_fire_queue: bool,
}
pub trait NativeBlockTickOwner {
    /// Claim only a tick whose actual configured scheduler, callback and save
    /// owner are implemented. The plan still has to execute its real enqueue.
    fn owns_native_block_tick(&self, tick: &crate::placement::PlannedBlockTick) -> Result<bool>;
    fn owns_native_placed_by(&self, _state: u32, _declaring_class: &str) -> Result<bool> { Ok(false) }
}
struct NoNativeBlockTicks;
impl NativeBlockTickOwner for NoNativeBlockTicks {
    fn owns_native_block_tick(&self, _: &crate::placement::PlannedBlockTick) -> Result<bool> {
        Ok(false)
    }
}
impl PreparedNativeSourceEdit {
    pub fn requires_fire_owner(&self) -> Result<bool> {
        Ok(self.requires_fire_queue || self.before.has_fire()? || self.after.has_fire()?)
    }
    pub fn require_fluid_history(
        &self,
        cache: &crate::fluid_physics::FluidShapeCache,
    ) -> Result<()> {
        if self.after.has_fluid()? {
            require_complete_fluid_shapes(cache)?;
        }
        Ok(())
    }
}
impl NativeEntitySource {
    pub fn observe_block_scheduler_use(&mut self) { self.history.requires_block_checkpoint = true; }
    pub fn requires_block_checkpoint(&self) -> bool { self.history.requires_block_checkpoint }
    /// Record a real menu transaction while the host retains the world/menu
    /// locks. Only the owned BE image may differ from the certified world.
    pub fn observe_owned_container_change(&mut self, world: &RuntimeWorld,
        before_images: &[crate::native_block_entities::ContainerImage]) -> Result<()> {
        ensure!(self.history.complete_native_items && self.history.invalidated.is_none(),
            "container transaction has no valid native source owner");
        let after=WorldImage::capture(world)?;
        let mut before=after.clone();
        before.containers=before_images.to_vec();
        let expected=self.pending_world.as_ref().or(self.history.certified_world.as_ref());
        ensure!(expected.is_some_and(|saved|saved == &before), "container transaction starts from unobserved world data");
        after.validate_item_runtime_scope()?;
        if self.pending_world.is_some() { self.pending_world=Some(after); }
        else { self.history.certified_world=Some(after); }
        Ok(())
    }
    /// Observe only an owned furnace inventory/timer transaction. A matching
    /// pre-mutation image is required even when no item entity currently exists.
    pub fn observe_owned_furnace_change(&mut self, world: &RuntimeWorld,
        before_images: &[crate::native_block_entities::FurnaceImage]) -> Result<()> {
        ensure!(self.history.complete_native_items && self.history.invalidated.is_none(),
            "furnace transaction has no valid native source owner");
        let after = WorldImage::capture(world)?;
        let mut before = after.clone();
        before.furnaces = before_images.to_vec();
        let expected = self.pending_world.as_ref().or(self.history.certified_world.as_ref());
        ensure!(expected.is_some_and(|saved| saved == &before),
            "furnace transaction starts from unobserved world data");
        after.validate_item_runtime_scope()?;
        if self.pending_world.is_some() { self.pending_world = Some(after); }
        else { self.history.certified_world = Some(after); }
        Ok(())
    }
    pub fn requires_fluid_checkpoint(&self) -> bool {
        self.history.requires_fluid_checkpoint || self.pending_fluid_seen
    }
    pub fn requires_fire_checkpoint(&self) -> bool {
        self.history.requires_fire_checkpoint || self.pending_fire_seen
    }
    pub fn require_fluid_history(
        &self,
        world: &RuntimeWorld,
        cache: &crate::fluid_physics::FluidShapeCache,
    ) -> Result<()> {
        if self.requires_fluid_checkpoint() {
            require_complete_fluid_shapes(cache)
        } else {
            require_native_fluid_history(world, cache)
        }
    }
    pub fn has_pending_edits(&self) -> bool {
        self.pending_world.is_some()
    }
    pub fn invalidate(&mut self, reason: impl Into<String>) {
        if self.history.invalidated.is_none() {
            self.history.invalidated = Some(reason.into());
        }
    }
    pub fn is_supported(&self) -> bool {
        self.history.complete_native_items
            && self.history.invalidated.is_none()
            && self.pending_world.is_none()
    }
    pub fn require_supported(&self) -> Result<()> {
        ensure!(
            self.is_supported(),
            "native item source history is unavailable: {:?}; pending owned edit: {}",
            self.history.invalidated,
            self.pending_world.is_some()
        );
        Ok(())
    }
    fn observe_world(&mut self, world: &WorldImage) {
        let expected = self
            .pending_world
            .as_ref()
            .or(self.history.certified_world.as_ref());
        let matches = expected.map_or_else(
            || world.is_canonical_unedited(),
            |expected| expected == world,
        );
        if !matches {
            self.invalidate("world changed outside the observed native transaction boundary");
        }
    }
    pub fn require_current_world(&self, world: &RuntimeWorld) -> Result<()> {
        self.require_supported()?;
        let actual = WorldImage::capture(world)?;
        actual.validate_item_runtime_scope()?;
        ensure!(
            self.history
                .certified_world
                .as_ref()
                .map_or_else(|| actual.is_canonical_unedited(), |saved| saved == &actual),
            "native source world has an unobserved edit"
        );
        Ok(())
    }
    /// A synchronous callback executes after an owned block commit and before
    /// complete_owned_edits certifies its final world. It may use that exact
    /// pending image, but cannot make pending callbacks disappear or grant the
    /// ordinary item-tick capability returned by require_current_world.
    pub fn require_owned_callback_world(&self, world: &RuntimeWorld) -> Result<()> {
        ensure!(
            self.history.complete_native_items && self.history.invalidated.is_none(),
            "native callback source history is unavailable: {:?}",
            self.history.invalidated
        );
        let actual = WorldImage::capture(world)?;
        actual.validate_item_runtime_scope()?;
        let expected = self.pending_world.as_ref()
            .or(self.history.certified_world.as_ref());
        ensure!(
            expected.map_or_else(|| actual.is_canonical_unedited(), |saved| saved == &actual),
            "native callback world differs from its owned pending/certified image"
        );
        Ok(())
    }
    pub fn prepare_owned_edit(
        &self,
        world: &RuntimeWorld,
        plan: &crate::world_interactions::InteractionPlan,
    ) -> Result<PreparedNativeSourceEdit> {
        self.prepare_owned_edit_with_block_ticks(world, plan, &NoNativeBlockTicks)
    }
    pub fn prepare_owned_edit_with_block_ticks(
        &self,
        world: &RuntimeWorld,
        plan: &crate::world_interactions::InteractionPlan,
        block_ticks: &impl NativeBlockTickOwner,
    ) -> Result<PreparedNativeSourceEdit> {
        use crate::world_interactions::InteractionEvent;
        ensure!(
            self.history.complete_native_items && self.history.invalidated.is_none(),
            "native source history is unavailable"
        );
        let before = WorldImage::capture(world)?;
        before.validate_item_runtime_scope()?;
        let expected = self
            .pending_world
            .as_ref()
            .or(self.history.certified_world.as_ref());
        ensure!(
            expected.map_or_else(|| before.is_canonical_unedited(), |saved| saved == &before),
            "native source transaction starts from an unobserved world"
        );
        for event in &plan.events {
            match event {
                InteractionEvent::RequiredHook { state, hook, declaring_class, .. } => {
                    ensure!(*hook == "setPlacedBy" && block_ticks.owns_native_placed_by(*state, declaring_class)?,
                        "native edit requires unsupported placement callback");
                }
                InteractionEvent::DropResources { .. } | InteractionEvent::BucketEvaporation { .. } =>
                    anyhow::bail!("native edit requires an unsupported loot/evaporation callback"),
                _ => {}
            }
            if let InteractionEvent::ScheduleBlockTick(tick) = event {
                ensure!(
                    block_ticks.owns_native_block_tick(tick)?,
                    "scheduled block tick has no actual native callback/save owner"
                );
            }
        }
        let RuntimeWorld::Fixture(flat) = world else {
            anyhow::bail!("native source requires actual fixture world");
        };
        let mut staged = flat.clone();
        for event in &plan.events {
            if let InteractionEvent::Write {
                position,
                old_state,
                state,
                ..
            } = event
            {
                ensure!(
                    staged.block(position.0, position.1, position.2) == *old_state,
                    "native source ordered old-state mismatch"
                );
                crate::placement::placement_block_name(*state)?;
                ensure!(
                    (crate::world::MIN_Y..=crate::world::MAX_Y).contains(&position.1),
                    "native source edit outside build height"
                );
                staged.set_block(position.0, position.1, position.2, *state);
                ensure!(staged.block(position.0, position.1, position.2) == *state,
                    "native container transition requires actual contents/drop owner");
            }
        }
        for event in &plan.events {
            if let InteractionEvent::CreateBlockEntity(be) = event {
                let expected_name = if let Some(kind) = crate::native_block_entities::furnace_kind(be.state) {
                    staged.native_block_entities().furnace_validate_at(be.position, be.state)?;
                    crate::native_block_entities::furnace_name(kind)
                } else {
                    staged.native_block_entities().validate_at(be.position, be.state)?;
                    crate::native_block_entities::container_kind(be.state)
                        .context("unsupported block entity type")?.name()
                };
                ensure!(be.type_name == expected_name, "block entity owner identity mismatch");
            }
        }
        let after = WorldImage::capture(&RuntimeWorld::Fixture(staged))?;
        after.validate_item_runtime_scope()?;
        let requires_fire_queue = plan.events.iter().any(|event| matches!(event,
            InteractionEvent::ScheduleBlockTick(tick) if crate::placement::placement_block_name(tick.state).ok() == Some("minecraft:fire")));
        Ok(PreparedNativeSourceEdit {
            before,
            after,
            requires_fire_queue,
        })
    }
    /// Prepare one actual furnace onRemove transaction without certifying an
    /// emptied live block entity. Only a private clone is drained to let the
    /// ordinary ordered-write validator calculate its final removed image.
    pub fn prepare_owned_furnace_removal_with_block_ticks(
        &self,
        world: &RuntimeWorld,
        at: (i32, i32, i32),
        plan: &crate::world_interactions::InteractionPlan,
        block_ticks: &impl NativeBlockTickOwner,
    ) -> Result<PreparedNativeSourceEdit> {
        self.prepare_owned_furnace_removal_impl(world, at, plan, block_ticks, None)
    }
    /// The dedicated menu owner retains exactly the removed object's viewers.
    /// Ordinary transitions and the closed-menu API keep their strict guards.
    pub fn prepare_owned_furnace_removal_retaining_viewers_with_block_ticks(
        &self,
        world: &RuntimeWorld,
        at: (i32, i32, i32),
        plan: &crate::world_interactions::InteractionPlan,
        block_ticks: &impl NativeBlockTickOwner,
        retention: &crate::native_block_entities::FurnaceViewerRetention,
    ) -> Result<PreparedNativeSourceEdit> {
        self.prepare_owned_furnace_removal_impl(world, at, plan, block_ticks, Some(retention))
    }
    fn prepare_owned_furnace_removal_impl(
        &self,
        world: &RuntimeWorld,
        at: (i32, i32, i32),
        plan: &crate::world_interactions::InteractionPlan,
        block_ticks: &impl NativeBlockTickOwner,
        retention: Option<&crate::native_block_entities::FurnaceViewerRetention>,
    ) -> Result<PreparedNativeSourceEdit> {
        use crate::world_interactions::InteractionEvent;
        self.require_owned_callback_world(world)?;
        let before = WorldImage::capture(world)?;
        let RuntimeWorld::Fixture(flat) = world else {
            anyhow::bail!("furnace removal requires the actual native world")
        };
        let writes = plan.events.iter().filter_map(|event| match event {
            InteractionEvent::Write { position, old_state, state, .. } if *position == at => Some((*old_state, *state)),
            _ => None,
        }).collect::<Vec<_>>();
        ensure!(writes.len() == 1, "furnace removal must own exactly one target write");
        let (old, replacement) = writes[0];
        ensure!(flat.block(at.0, at.1, at.2) == old, "furnace removal old state differs");
        let receipt = flat.native_block_entities().prepare_furnace_removal(at, old, replacement)?
            .context("furnace removal does not replace a furnace identity")?;
        let before_furnaces = flat.native_block_entities().furnace_images();
        let mut staged = flat.clone();
        if let Some(retention) = retention {
            staged.native_block_entities_mut().drain_furnace_removal_retaining_viewers(&receipt, retention)?;
        } else {
            staged.native_block_entities_mut().drain_furnace_removal(&receipt)?;
        }
        let staged = RuntimeWorld::Fixture(staged);
        let mut private_source = Self {
            history: self.history.clone(),
            pending_world: self.pending_world.clone(),
            pending_fluid_seen: self.pending_fluid_seen,
            pending_fire_seen: self.pending_fire_seen,
        };
        private_source.observe_owned_furnace_change(&staged, &before_furnaces)?;
        let mut token = private_source.prepare_owned_edit_with_block_ticks(&staged, plan, block_ticks)?;
        token.before = before;
        Ok(token)
    }
    /// Prepare a closed-viewer single-container onRemove transaction. The
    /// actual source retains its original full image; only a private staging
    /// owner is drained so ordered writes can remove that block entity.
    pub fn prepare_owned_container_removal_with_block_ticks(
        &self,
        world: &RuntimeWorld,
        at: (i32, i32, i32),
        plan: &crate::world_interactions::InteractionPlan,
        block_ticks: &impl NativeBlockTickOwner,
    ) -> Result<PreparedNativeSourceEdit> {
        use crate::world_interactions::InteractionEvent;
        self.require_owned_callback_world(world)?;
        let before = WorldImage::capture(world)?;
        let RuntimeWorld::Fixture(flat) = world else {
            anyhow::bail!("container removal requires the actual native world")
        };
        let writes = plan.events.iter().filter_map(|event| match event {
            InteractionEvent::Write { position, old_state, state, .. } if *position == at => {
                Some((*old_state, *state))
            }
            _ => None,
        }).collect::<Vec<_>>();
        ensure!(writes.len() == 1, "container removal must own exactly one target write");
        let (old, replacement) = writes[0];
        ensure!(flat.block(at.0, at.1, at.2) == old, "container removal old state differs");
        let receipt = flat.native_block_entities()
            .prepare_container_removal(at, old, replacement)?
            .context("container removal does not replace a container identity")?;
        let before_containers = flat.native_block_entities().images();
        let mut staged = flat.clone();
        staged.native_block_entities_mut().drain_container_removal(&receipt)?;
        let staged = RuntimeWorld::Fixture(staged);
        let mut private_source = Self {
            history: self.history.clone(),
            pending_world: self.pending_world.clone(),
            pending_fluid_seen: self.pending_fluid_seen,
            pending_fire_seen: self.pending_fire_seen,
        };
        private_source.observe_owned_container_change(&staged, &before_containers)?;
        let mut token = private_source.prepare_owned_edit_with_block_ticks(&staged, plan, block_ticks)?;
        // Commit still compares against the actual original source/history,
        // never the drained private image used to calculate final removal.
        token.before = before;
        Ok(token)
    }
    pub fn observe_owned_commit(
        &mut self,
        prepared: PreparedNativeSourceEdit,
        world: &RuntimeWorld,
    ) -> Result<()> {
        ensure!(
            self.history.complete_native_items && self.history.invalidated.is_none(),
            "native source was invalidated before commit"
        );
        let expected = self
            .pending_world
            .as_ref()
            .or(self.history.certified_world.as_ref());
        ensure!(
            expected.map_or_else(
                || prepared.before.is_canonical_unedited(),
                |saved| saved == &prepared.before
            ),
            "native source transaction order changed"
        );
        let actual = WorldImage::capture(world)?;
        if actual != prepared.after {
            self.invalidate("native atomic commit differs from its prepared ordered writes");
            anyhow::bail!("native source committed world mismatch");
        }
        self.pending_fluid_seen |= prepared.before.has_fluid()? || actual.has_fluid()?;
        self.pending_fire_seen |=
            prepared.requires_fire_queue || prepared.before.has_fire()? || actual.has_fire()?;
        self.pending_world = Some(actual);
        Ok(())
    }
    pub fn complete_owned_edits(
        &mut self,
        world: &RuntimeWorld,
        lighting: &crate::runtime_lighting::NativeRuntimeLighting,
        fluid_shapes: &crate::fluid_physics::FluidShapeCache,
        pending_callbacks: usize,
    ) -> Result<()> {
        ensure!(
            self.history.complete_native_items && self.history.invalidated.is_none(),
            "native source history is unavailable"
        );
        ensure!(
            pending_callbacks == 0,
            "native source has pending synchronous world callbacks"
        );
        let RuntimeWorld::Fixture(flat) = world else {
            anyhow::bail!("imported world cannot acquire native source provenance");
        };
        lighting.require_current(flat)?;
        let actual = WorldImage::capture(world)?;
        actual.validate_item_runtime_scope()?;
        if actual.has_fluid()? || self.requires_fluid_checkpoint() {
            require_complete_fluid_shapes(fluid_shapes)?;
        }
        self.observe_world(&actual);
        ensure!(
            self.history.invalidated.is_none(),
            "native source changed outside an observed transaction"
        );
        self.history.certified_world = Some(actual);
        self.history.requires_fluid_checkpoint |= self.pending_fluid_seen;
        self.history.requires_fire_checkpoint |= self.pending_fire_seen;
        self.pending_world = None;
        self.pending_fluid_seen = false;
        self.pending_fire_seen = false;
        Ok(())
    }
}

pub fn require_complete_fluid_shapes(cache: &crate::fluid_physics::FluidShapeCache) -> Result<()> {
    use crate::fluid_physics::{FluidShapeCache, FluidShapeHistory, FluidShapeOwner};
    let keys = FluidShapeCache::from_observed_empty_owners(FluidShapeOwner::ALL);
    for (state, _) in keys.observations() {
        ensure!(
            !matches!(cache.history(state)?, FluidShapeHistory::Unknown),
            "native fluid shape history is incomplete"
        );
    }
    Ok(())
}
pub fn require_native_fluid_history(
    world: &RuntimeWorld,
    cache: &crate::fluid_physics::FluidShapeCache,
) -> Result<()> {
    if WorldImage::capture(world)?.has_fluid()? {
        require_complete_fluid_shapes(cache)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeWorldTime {
    pub clock: TickClock,
    pub weather: WeatherState,
}

/// This checkpoint is a single atomic file. Coordinate writing the separate
/// block-world file with this capture; resume checks exact world-image equality
/// and rejects a torn pair instead of silently attaching items to another world.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityCheckpoint {
    format: String,
    version: u32,
    minecraft: String,
    protocol: i32,
    mode: String,
    source: SourceHistory,
    world: WorldImage,
    time: NativeWorldTime,
    next_entity_id: i32,
    world_random_state48: u64,
    sound_random_state48: u64,
    item_tick_order: Vec<i32>,
    items: Vec<SavedItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native_entities: Option<NativeEntityCohort>,
    /// Exact bytes written to the separate SavedWorld file in this capture.
    /// Block-image equality alone does not bind queues or other world fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    world_file_sha256: Option<String>,
}
pub struct RestoredEntities {
    pub runtime: RuntimeEntities,
    pub source: NativeEntitySource,
    pub next_entity_id: i32,
    pub time: NativeWorldTime,
    /// Saved scheduling order is retained as registration order. Chunk readiness
    /// is rebuilt, so this is not a claim to resume old executor/future order.
    pub item_tick_order: Vec<i32>,
    /// Some (including an empty cohort) is authoritative. None explicitly
    /// denotes the legacy item-only checkpoint and its narrower order proof.
    pub native_entities: Option<NativeEntityCohort>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointEnvelope {
    sha256: String,
    checkpoint: EntityCheckpoint,
}
fn digest(bytes: &[u8]) -> String {
    openssl::sha::sha256(bytes)
        .into_iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_cohort_identity(
    id: i32,
    uuid: [u8; 16],
    next_entity_id: i32,
    ids: &mut BTreeSet<i32>,
    uuids: &mut BTreeSet<[u8; 16]>,
) -> Result<()> {
    ensure!(id > 0 && id < next_entity_id, "native cohort exceeds shared entity counter");
    ensure!(ids.insert(id), "duplicate ID across native entity cohort");
    ensure!(uuids.insert(uuid), "duplicate UUID across native entity cohort");
    Ok(())
}

/// JSON must never silently replace a retained physical NaN/Inf with null.
/// Damage-specific float bits are encoded separately by FallingBlockSnapshot.
fn validate_persistent_physical(p: &crate::entity_state::EntityPhysicalState) -> Result<()> {
    ensure!(p.vehicle.is_none(), "native entity passenger/vehicle graph checkpoint is unsupported");
    let doubles = [
        p.position.x, p.position.y, p.position.z,
        p.velocity.x, p.velocity.y, p.velocity.z,
        p.bounds.min_x, p.bounds.min_y, p.bounds.min_z,
        p.bounds.max_x, p.bounds.max_y, p.bounds.max_z,
        p.old_position.x, p.old_position.y, p.old_position.z,
        p.stuck_speed_multiplier.x, p.stuck_speed_multiplier.y, p.stuck_speed_multiplier.z,
        p.piston_totals[0], p.piston_totals[1], p.piston_totals[2],
        p.fluid_heights[0], p.fluid_heights[1],
    ];
    let floats = [
        p.position.yaw, p.position.pitch, p.width, p.height, p.eye_height,
        p.old_yaw, p.old_pitch, p.walk_dist_old, p.walk_dist, p.move_dist,
        p.fly_dist, p.next_step, p.fall_distance,
    ];
    ensure!(doubles.into_iter().all(f64::is_finite) && floats.into_iter().all(f32::is_finite),
        "nonfinite native entity physical field cannot be persisted as JSON");
    ensure!(p.width >= 0. && p.height >= 0.
        && p.bounds.min_x <= p.bounds.max_x && p.bounds.min_y <= p.bounds.max_y && p.bounds.min_z <= p.bounds.max_z,
        "invalid native entity physical bounds");
    ensure!(p.metadata_dirty & !0x1ff == 0, "unknown native nonliving entity metadata fields");
    Ok(())
}

impl EntityCheckpoint {
    pub fn capture(
        runtime: &RuntimeEntities,
        source: &mut NativeEntitySource,
        world: &RuntimeWorld,
        next_entity_id: i32,
        time: NativeWorldTime,
    ) -> Result<Self> {
        ensure!(
            next_entity_id > runtime.highest_registered_entity_id().unwrap_or(0),
            "next entity ID would collide with a live item/player"
        );
        let world = WorldImage::capture(world)?;
        source.observe_world(&world);
        ensure!(
            source.pending_world.is_none(),
            "cannot checkpoint incomplete native edit callbacks"
        );
        let item_tick_order = runtime.persistent_item_tick_order();
        let mut entries = runtime
            .persistent_item_states()
            .map(|(state, random)| Ok((state.entity_id, SavedItem::capture(state, random)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut items = Vec::with_capacity(entries.len());
        for id in &item_tick_order {
            items.push(
                entries
                    .remove(id)
                    .context("tick-list item missing from persistent store")?,
            );
        }
        items.extend(entries.into_values());
        let value = Self {
            format: FORMAT.into(),
            version: VERSION,
            minecraft: "1.21".into(),
            protocol: 767,
            mode: "NativeResume".into(),
            source: source.history.clone(),
            world,
            time,
            next_entity_id,
            world_random_state48: runtime.world_random_state48(),
            sound_random_state48: runtime
                .sound_random_state48()
                .context("native resume requires actual level sound RNG owner")?,
            item_tick_order,
            items,
            native_entities: None,
            world_file_sha256: None,
        };
        value.validate_structure()?;
        Ok(value)
    }
    /// Capture all actual persistent entity owners while Main retains the same
    /// world/player registry lock. The caller supplies snapshots of the REAL
    /// external bodies; shared index/section/tick order is read from runtime.
    pub fn capture_with_native_entities(
        runtime: &RuntimeEntities,
        source: &mut NativeEntitySource,
        world: &RuntimeWorld,
        next_entity_id: i32,
        time: NativeWorldTime,
        experience_orbs: Vec<crate::experience::SavedExperienceOrb>,
        falling_blocks: Vec<crate::falling_block::persistence::FallingBlockSnapshot>,
    ) -> Result<Self> {
        let order = runtime.capture_persistent_entity_order()?;
        let mut value = Self::capture(runtime, source, world, next_entity_id, time)?;
        value.native_entities = Some(NativeEntityCohort { experience_orbs, falling_blocks, order });
        value.validate_structure()?;
        Ok(value)
    }

    pub fn has_native_entity_cohort(&self) -> bool { self.native_entities.is_some() }
    /// Bind a newly captured checkpoint to the exact world bytes Main is about
    /// to write. An existing proof cannot be rebound to another save interval.
    pub fn bind_world_file_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        ensure!(self.world_file_sha256.is_none(), "native world-file proof is already bound");
        self.world_file_sha256 = Some(digest(bytes));
        Ok(())
    }
    /// Validate the bytes actually read from disk, without parsing/reserializing
    /// them into a different representation. Every new atomic cohort, including
    /// EMPTY, requires this proof; older item-only checkpoints remain legacy.
    pub fn require_world_file_bytes(&self, bytes: &[u8]) -> Result<()> {
        match &self.world_file_sha256 {
            Some(expected) => ensure!(expected == &digest(bytes),
                "world/entity checkpoint byte mismatch; refusing torn scheduling/source state"),
            None => ensure!(self.native_entities.is_none(),
                "atomic native entity cohort lacks its exact world-file proof"),
        }
        Ok(())
    }
    fn require_persisted_world_proof(&self) -> Result<()> {
        ensure!(self.native_entities.is_none() || self.world_file_sha256.is_some(),
            "atomic native entity cohort lacks its exact world-file proof");
        Ok(())
    }
    pub fn persistent_entity_count(&self) -> usize {
        self.items.len() + self.native_entities.as_ref()
            .map_or(0, |entities| entities.experience_orbs.len() + entities.falling_blocks.len())
    }
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    pub fn next_entity_id(&self) -> i32 {
        self.next_entity_id
    }
    pub fn time(&self) -> NativeWorldTime {
        self.time
    }
    pub fn require_native_resume(&self, world: &RuntimeWorld) -> Result<()> {
        self.validate_structure()?;
        self.require_persisted_world_proof()?;
        ensure!(
            self.source.complete_native_items && self.source.invalidated.is_none(),
            "native item source was invalidated: {:?}",
            self.source.invalidated
        );
        ensure!(
            self.world == WorldImage::capture(world)?,
            "world/item checkpoint mismatch; refusing torn or foreign source"
        );
        self.world.validate_item_runtime_scope()?;
        ensure!(
            self.source.certified_world.as_ref().map_or_else(
                || self.world.is_canonical_unedited(),
                |saved| saved == &self.world
            ),
            "unsupported or unobserved native item source world"
        );
        Ok(())
    }
    fn validate_structure(&self) -> Result<()> {
        ensure!(
            self.format == FORMAT
                && self.version == VERSION
                && self.minecraft == "1.21"
                && self.protocol == 767,
            "unsupported native item checkpoint version"
        );
        ensure!(
            self.mode == "NativeResume",
            "Java NBT loading does not resume native RNG state; separate import is not implemented"
        );
        ensure!(
            self.source.origin == "ObservedFreshNativeFixture",
            "unknown native item source origin"
        );
        restore_random(self.world_random_state48)?;
        restore_random(self.sound_random_state48)?;
        if let Some(hash) = &self.world_file_sha256 {
            ensure!(hash.len() == 64 && hash.bytes().all(|byte|
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
                "invalid native world-file digest");
        }
        let mut ids = BTreeSet::new();
        let mut uuids = BTreeSet::new();
        for item in &self.items {
            ensure!(
                ids.insert(item.entity_id()),
                "duplicate saved item entity ID"
            );
            ensure!(uuids.insert(item.uuid()), "duplicate saved item UUID");
            ensure!(
                item.entity_id() > 0 && item.entity_id() < self.next_entity_id,
                "saved next entity ID would collide"
            );
        }
        let item_ids = ids.clone();
        if let Some(native) = &self.native_entities {
            let mut entries = self.items.iter().map(|item| {
                let (state, _) = item.clone().restore()?;
                validate_persistent_physical(&state.physical)?;
                Ok((state.entity_id, crate::runtime_entity_index::EntityKind::Item, state.position_vec()))
            }).collect::<Result<Vec<_>>>()?;
            for orb in &native.experience_orbs {
                orb.state.validate()?;
                orb.restore_random()?;
                validate_persistent_physical(&orb.state.physical)?;
                ensure!(!orb.state.removed, "removed XP orb in native cohort");
                validate_cohort_identity(orb.state.entity_id, orb.state.uuid, self.next_entity_id, &mut ids, &mut uuids)?;
                entries.push((orb.state.entity_id, crate::runtime_entity_index::EntityKind::ExperienceOrb, orb.state.position_vec()));
            }
            for saved in &native.falling_blocks {
                ensure!(saved.block_data.is_none(),
                    "native falling checkpoint lacks the actual retained block-data map owner");
                let falling = saved.clone().restore()?;
                validate_persistent_physical(&falling.physical)?;
                ensure!(!falling.removed, "removed falling block in native cohort");
                crate::placement::placement_block_name(falling.block_state)?;
                validate_cohort_identity(falling.entity_id, falling.uuid, self.next_entity_id, &mut ids, &mut uuids)?;
                entries.push((falling.entity_id, crate::runtime_entity_index::EntityKind::FallingBlock, falling.position_vec()));
            }
            native.order.validate_entries(&entries)?;
        }
        ensure!(self.next_entity_id > 0, "invalid saved next entity ID");
        let mut ticks = BTreeSet::new();
        for id in &self.item_tick_order {
            ensure!(
                item_ids.contains(id) && ticks.insert(*id),
                "invalid saved item tick order"
            );
        }
        ensure!(
            [
                self.time.weather.old_rain_level,
                self.time.weather.rain_level,
                self.time.weather.old_thunder_level,
                self.time.weather.thunder_level
            ]
            .into_iter()
            .all(f32::is_finite),
            "nonfinite native weather cannot be persisted as JSON"
        );
        Ok(())
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate_structure()?;
        self.require_persisted_world_proof()?;
        let body = serde_json::to_vec(self)?;
        let bytes = serde_json::to_vec(&CheckpointEnvelope {
            sha256: digest(&body),
            checkpoint: self.clone(),
        })?;
        ensure!(
            bytes.len() as u64 <= MAX_CHECKPOINT_BYTES,
            "native item checkpoint exceeds256MiB"
        );
        Ok(bytes)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() as u64 <= MAX_CHECKPOINT_BYTES,
            "native item checkpoint exceeds256MiB"
        );
        let file: CheckpointEnvelope =
            serde_json::from_slice(bytes).context("invalid native item checkpoint")?;
        ensure!(
            file.sha256 == digest(&serde_json::to_vec(&file.checkpoint)?),
            "native item checkpoint integrity mismatch"
        );
        file.checkpoint.validate_structure()?;
        file.checkpoint.require_persisted_world_proof()?;
        Ok(file.checkpoint)
    }
    /// Decode/validate every item before making a new store visible. No tick,
    /// tracker, packet, entropy factory or RNG draw runs during rehydration.
    pub fn restore(self, world: &RuntimeWorld) -> Result<RestoredEntities> {
        self.require_native_resume(world)?;
        let entries = self
            .items
            .into_iter()
            .map(SavedItem::restore)
            .collect::<Result<Vec<_>>>()?;
        let mut runtime = RuntimeEntities::new(restore_random(self.world_random_state48)?)
            .with_sound_random(restore_random(self.sound_random_state48)?);
        runtime.prepare_items(entries)?.commit();
        Ok(RestoredEntities {
            runtime,
            source: NativeEntitySource {
                history: self.source,
                pending_world: None,
                pending_fluid_seen: false,
                pending_fire_seen: false,
            },
            next_entity_id: self.next_entity_id,
            time: self.time,
            item_tick_order: self.item_tick_order,
            native_entities: self.native_entities,
        })
    }
}

#[derive(Clone, Debug)]
pub struct EntityDataStore {
    path: PathBuf,
    writes: Arc<Mutex<()>>,
}
impl EntityDataStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            writes: Arc::new(Mutex::new(())),
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Absence is a real new-world observation only if BOTH storage files are
    /// absent. Existing worlds without a complete entity file remain unknown.
    pub fn fresh_source(
        &self,
        native_world_path: &Path,
        world: &RuntimeWorld,
    ) -> Result<NativeEntitySource> {
        ensure!(
            !self.path.try_exists()? && !native_world_path.try_exists()?,
            "existing native world/checkpoint cannot be inferred to contain zero entities"
        );
        ensure!(
            WorldImage::capture(world)?.is_canonical_unedited(),
            "fresh native source is not canonical unedited fixture"
        );
        Ok(NativeEntitySource {
            history: SourceHistory {
                origin: "ObservedFreshNativeFixture".into(),
                complete_native_items: true,
                invalidated: None,
                certified_world: Some(WorldImage::capture(world)?),
                requires_fluid_checkpoint: false,
                requires_fire_checkpoint: false,
                requires_block_checkpoint: false,
            },
            pending_world: None,
            pending_fluid_seen: false,
            pending_fire_seen: false,
        })
    }
    pub fn load(&self) -> Result<Option<EntityCheckpoint>> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("open native item checkpoint"),
        };
        let mut bytes = Vec::new();
        file.take(MAX_CHECKPOINT_BYTES + 1)
            .read_to_end(&mut bytes)?;
        EntityCheckpoint::from_bytes(&bytes).map(Some)
    }
    pub fn save(&self, checkpoint: &EntityCheckpoint) -> Result<()> {
        let bytes = checkpoint.to_bytes()?;
        let _guard = self
            .writes
            .lock()
            .map_err(|_| anyhow::anyhow!("item save mutex poisoned"))?;
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut attempts = 0;
        let (temporary, mut file) = loop {
            let path = parent.join(format!(
                ".native-items.{}.{}.tmp",
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            match options.open(&path) {
                Ok(file) => break (path, file),
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists && attempts < 1024 =>
                {
                    attempts += 1
                }
                Err(error) => return Err(error).context("create atomic item checkpoint"),
            }
        };
        let result = (|| -> Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)?;
            File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.context("save atomic native item checkpoint")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        entity::Position, geometry::Vec3, inventory::ItemStack, item_entity::ItemState,
        world::FlatWorld,
    };
    #[test]
    fn owned_edit_waits_for_callbacks_and_light_then_resumes_exact_world() {
        use crate::{
            fluid_physics::{FluidShapeCache, FluidShapeOwner},
            runtime_lighting::NativeRuntimeLighting,
            world_interactions::{InteractionEvent, InteractionPlan},
        };
        let path = std::env::temp_dir().join(format!(
            "mc121-owned-edit-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let store = EntityDataStore::new(path.with_extension("items"));
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        let mut source = store.fresh_source(&path, &world).unwrap();
        let mut light = NativeRuntimeLighting::new(true);
        let cache = FluidShapeCache::from_observed_empty_owners(FluidShapeOwner::ALL);
        let RuntimeWorld::Fixture(flat) = &world else {
            unreachable!()
        };
        light.process_pending(flat).unwrap();
        let mut plan = InteractionPlan::default();
        plan.events.push(InteractionEvent::Write {
            position: (0, 80, 0),
            old_state: 0,
            state: 1,
            flags: 3,
        });
        let token = source.prepare_owned_edit(&world, &plan).unwrap();
        let RuntimeWorld::Fixture(flat) = &mut world else {
            unreachable!()
        };
        assert!(flat.set_block(0, 80, 0, 1));
        source.observe_owned_commit(token, &world).unwrap();
        assert!(source.require_supported().is_err());
        assert!(source
            .complete_owned_edits(&world, &light, &cache, 0)
            .is_err());
        let RuntimeWorld::Fixture(flat) = &world else {
            unreachable!()
        };
        light.process_pending(flat).unwrap();
        assert!(source
            .complete_owned_edits(&world, &light, &cache, 1)
            .is_err());
        source
            .complete_owned_edits(&world, &light, &cache, 0)
            .unwrap();
        let runtime = RuntimeEntities::new(LegacyRandomSource::new(1))
            .with_sound_random(LegacyRandomSource::new(2));
        let checkpoint = EntityCheckpoint::capture(
            &runtime,
            &mut source,
            &world,
            1,
            NativeWorldTime {
                clock: TickClock::default(),
                weather: WeatherState::default(),
            },
        )
        .unwrap();
        let restored = EntityCheckpoint::from_bytes(&checkpoint.to_bytes().unwrap())
            .unwrap()
            .restore(&world)
            .unwrap();
        restored.source.require_current_world(&world).unwrap();
        let RuntimeWorld::Fixture(flat) = &mut world else {
            unreachable!()
        };
        flat.set_block(1, 80, 0, 1);
        assert!(restored.source.require_current_world(&world).is_err());
    }
    #[test]
    fn native_item_restart_smoke() {
        let directory = std::env::temp_dir().join(format!(
            "mc121-items-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let store = EntityDataStore::new(directory.join("items.native.json"));
        let world_path = directory.join("world.json");
        let world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        let mut source = store.fresh_source(&world_path, &world).unwrap();
        let mut z = LegacyRandomSource::new(12345);
        z.next_i64();
        let mut f = LegacyRandomSource::new(67890);
        f.next_i64();
        let mut rng = LegacyRandomSource::new(712101);
        rng.next_i64();
        let mut item = ItemState::new(
            45,
            [7; 16],
            Position {
                x: 8.5,
                y: 65.,
                z: 8.5,
                yaw: 19.,
                pitch: -3.,
                on_ground: false,
            },
            Vec3::new(0.1, -0.2, 0.3),
            ItemStack::new(1, 4).unwrap(),
        )
        .unwrap();
        item.age = 81;
        item.pickup_delay = 12;
        item.health = 3;
        item.thrower = Some([8; 16]);
        item.remaining_fire_ticks = 10;
        let mut runtime = RuntimeEntities::new(z.clone()).with_sound_random(f.clone());
        runtime.insert_item(item, rng.clone()).unwrap();
        let time = NativeWorldTime {
            clock: TickClock::default(),
            weather: WeatherState::default(),
        };
        let checkpoint =
            EntityCheckpoint::capture(&runtime, &mut source, &world, 90, time).unwrap();
        store.save(&checkpoint).unwrap();
        let resumed = EntityDataStore::new(store.path())
            .load()
            .unwrap()
            .unwrap()
            .restore(&world)
            .unwrap();
        assert_eq!(resumed.next_entity_id, 90);
        assert_eq!(resumed.runtime.world_random_state48(), z.state48());
        assert_eq!(resumed.runtime.sound_random_state48(), Some(f.state48()));
        let (state, random) = resumed.runtime.persistent_item_states().next().unwrap();
        assert_eq!(
            (
                state.age,
                state.pickup_delay,
                state.health,
                state.remaining_fire_ticks
            ),
            (81, 12, 3, 10)
        );
        assert_eq!(state.stack.count, 4);
        assert_eq!(state.thrower, Some([8; 16]));
        assert_eq!(random.state48(), rng.state48());
        assert_eq!(resumed.runtime.ticking_item_states().count(), 0);
        assert!(store.fresh_source(&world_path, &world).is_err());
        source.invalidate("edit then revert");
        assert!(
            EntityCheckpoint::capture(&runtime, &mut source, &world, 90, time)
                .unwrap()
                .restore(&world)
                .is_err()
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
