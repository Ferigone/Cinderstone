//! Real XP-orb owner using the shared native entity index and inherited physics.
use super::*;
use mc121_rust::{
    entity::{EntityMotionTracker, MotionTrackingOptions},
    entity_base,
    entity_move::{self, EntityMoveContext, MovementEmission, MoverType, ProfileEvent},
    entity_state::EntityPhysicalState,
    equipment::EquipmentSlot,
    experience::{self, ExperienceOrbState, ExperienceOrbWorld, FollowingPlayer, SavedExperienceOrb},
    fluid_physics::{FluidKind, FluidWorld, RuntimeFluidWorld},
    item_entity::FullItemWorld,
    item_host::RuntimeGameplayHooks,
    item_use::JdkMathRandom,
    runtime_entity_index::{EntityHandle, EntityKind, RuntimeEntityIndex},
    runtime_item_world::{ItemWorldFacts, RuntimeItemEffect, RuntimeItemHost, RuntimeItemWorld},
};

#[derive(Clone)]
struct NativeOrb {
    state: ExperienceOrbState,
    random: LegacyRandomSource,
    handle: Option<EntityHandle>,
    tracker: EntityMotionTracker,
    metadata: EntityPhysicalState,
    seen: BTreeSet<i32>,
    insertion: u64,
}
#[derive(Clone, Default)]
pub(super) struct MainExperienceRuntime {
    orbs: BTreeMap<i32, NativeOrb>,
    next_insertion: u64,
    revision: u64,
    pickup_delay: BTreeMap<i32, i32>,
    last_level_up: BTreeMap<i32, i32>,
    last_sent_experience: BTreeMap<i32, i32>,
}
impl MainExperienceRuntime {
    pub(super) fn snapshot(&self) -> Result<Vec<SavedExperienceOrb>> {
        let mut values = self.orbs.values().filter(|o| !o.state.removed).collect::<Vec<_>>();
        values.sort_by_key(|o| o.insertion);
        values.into_iter().map(|o| {
            o.state.validate()?;
            let mut state = o.state.clone();
            state.following_player = None;
            Ok(SavedExperienceOrb { state, random_state48: o.random.state48() })
        }).collect()
    }
    pub(super) fn restore(saved: Vec<SavedExperienceOrb>, items: &mut RuntimeEntities) -> Result<Self> {
        items.validate_experience_orb_registration_batch(saved.iter().map(|o| o.state.entity_id))?;
        let mut runtime = Self::default();
        for record in saved {
            record.state.validate()?;
            let random = record.restore_random()?;
            let mut state = record.state;
            state.following_player = None;
            let handle = items.register_experience_orb(state.entity_id, state.position_vec())?;
            runtime.insert(state, random, Some(handle))?;
        }
        Ok(runtime)
    }
    fn insert(&mut self, state: ExperienceOrbState, random: LegacyRandomSource, handle: Option<EntityHandle>) -> Result<()> {
        ensure!(!self.orbs.contains_key(&state.entity_id), "duplicate native XP orb ID");
        let tracker = EntityMotionTracker::with_options(state.entity_id, state.position, state.velocity,
            MotionTrackingOptions { update_interval: 20, track_delta: true, track_head: false })?;
        self.next_insertion = self.next_insertion.checked_add(1).context("XP insertion order exhausted")?;
        self.orbs.insert(state.entity_id, NativeOrb { metadata: state.physical.clone(), state, random,
            handle, tracker, seen: BTreeSet::new(), insertion: self.next_insertion });
        Ok(())
    }
    pub(super) fn max_entity_id(&self) -> Option<i32> { self.orbs.keys().next_back().copied() }
    pub(super) fn bounds(&self, handle: EntityHandle) -> Option<Aabb> {
        self.orbs.get(&handle.id).filter(|o| o.handle == Some(handle) && !o.state.removed).map(|o| o.state.bounds)
    }
    pub(super) fn physical(&self, handle: EntityHandle) -> Option<&EntityPhysicalState> {
        self.orbs.get(&handle.id).filter(|o|o.handle==Some(handle)&&!o.state.removed).map(|o|&o.state.physical)
    }
    pub(super) fn count(&self) -> usize { self.orbs.len() }
    pub(super) fn physical_states(&self) -> impl Iterator<Item = &EntityPhysicalState> {
        self.orbs.values().filter(|o| !o.state.removed).map(|o| &o.state.physical)
    }
    pub(super) fn begin_player_tick(&mut self, id: i32) {
        if let Some(delay) = self.pickup_delay.get_mut(&id) { if *delay > 0 { *delay -= 1; } }
    }
    pub(super) fn forget_player(&mut self, id: i32) {
        self.pickup_delay.remove(&id); self.last_level_up.remove(&id); self.last_sent_experience.remove(&id);
        for orb in self.orbs.values_mut() { orb.seen.remove(&id); }
    }
    pub(super) fn publish_one(
        &mut self, handle: EntityHandle, registry: &EntityRegistry,
        observers: &BTreeMap<i32, TrackingObserver>, transports: &BTreeMap<i32, SessionChunkTransport>,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        let Some(orb) = self.orbs.get_mut(&handle.id).filter(|o| o.handle == Some(handle)) else { return Ok(()); };
        let p = orb.state.position_vec();
        let chunk = ((p.x.floor() as i32).div_euclid(16), (p.z.floor() as i32).div_euclid(16));
        let mut next = BTreeSet::new();
        for (&id, observer) in observers {
            let target = entity_tracking::TrackingTarget { entity_id: handle.id, position: p,
                base_range_blocks: 6 * 16, indirect_passenger_ranges_chunks: &[], broadcast_allowed: true };
            if registry.get(id).is_some()
                && transports.get(&id).is_some_and(|t| t.entity_chunk_sent(chunk))
                && entity_tracking::within_tracking_range(*observer, target) { next.insert(id); }
        }
        for id in orb.seen.difference(&next) { emit(events, vec![*id], vec![experience::remove_packet(&[handle.id])]); }
        for id in next.difference(&orb.seen) {
            let mut packets = vec![orb.state.spawn_packet(orb.tracker.spawn_position())?];
            let mask = orb.metadata.generic_non_default_mask();
            if let Some(packet) = mc121_rust::metadata::encode_values(handle.id, &orb.metadata.generic_metadata_values(mask))? { packets.push(packet); }
            packets.push(entity::velocity_packet(handle.id, orb.tracker.spawn_velocity())?);
            emit(events, vec![*id], packets);
        }
        orb.seen = next;
        let dirty = orb.state.metadata_dirty;
        let mut packets = orb.tracker.tick_with_velocity(orb.state.position, orb.state.velocity, dirty != 0, orb.state.has_impulse)?;
        if dirty != 0 {
            if let Some(packet) = mc121_rust::metadata::encode_values(handle.id, &orb.state.generic_metadata_values(dirty))? { packets.push(packet); }
            orb.metadata = orb.state.physical.clone();
        }
        if orb.state.hurt_marked { packets.push(entity::velocity_packet(handle.id, orb.state.velocity)?); }
        orb.state.metadata_dirty = 0; orb.state.has_impulse = false; orb.state.hurt_marked = false;
        emit(events, orb.seen.iter().copied().collect(), packets);
        Ok(())
    }
    fn remove(&mut self, id: i32, items: &mut RuntimeEntities, events: &broadcast::Sender<PlayerEvent>) -> Result<()> {
        if let Some(orb) = self.orbs.remove(&id) {
            if let Some(handle) = orb.handle { items.remove_experience_orb(handle)?; }
            emit(events, orb.seen.into_iter().collect(), vec![experience::remove_packet(&[id])]);
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }
}

fn emit(events: &broadcast::Sender<PlayerEvent>, recipients: Vec<i32>, packets: Vec<Packet>) {
    if !recipients.is_empty() && !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience { recipients: Arc::new(recipients), packets: Arc::new(packets) });
    }
}

pub(super) struct PreparedFurnaceExperience {
    staged: MainExperienceRuntime,
    expected_revision: u64,
    new_ids: Vec<i32>,
    world_before: u64,
    world_after: LegacyRandomSource,
    math_before: u64,
    math_after: LegacyRandomSource,
    deliveries: Vec<(Vec<i32>, Vec<Packet>)>,
}
impl PreparedFurnaceExperience {
    /// Combined container-removal preflight includes these IDs after the staged
    /// ItemEntity IDs, before committing either native owner.
    pub(super) fn registration_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.new_ids.iter().copied()
    }
    pub(super) fn commit(self, players: &mut LivePlayers, events: &broadcast::Sender<PlayerEvent>) -> Result<()> {
        self.commit_parts(&mut players.experience_runtime, &mut players.item_runtime,
            players.math_random.as_ref().context("actual Math.random owner missing")?, events)
    }
    pub(super) fn commit_parts(mut self, runtime: &mut MainExperienceRuntime, items: &mut RuntimeEntities,
        math: &JdkMathRandom, events: &broadcast::Sender<PlayerEvent>) -> Result<()> {
        ensure!(runtime.revision == self.expected_revision, "XP state changed after reward preparation");
        ensure!(items.world_random_state48() == self.world_before, "Level RNG changed after XP preparation");
        items.validate_experience_orb_registration_batch(self.new_ids.iter().copied())?;
        math.with_random(|actual| {
            ensure!(actual.state48() == self.math_before, "Math.random changed after XP preparation");
            for id in &self.new_ids {
                let orb = self.staged.orbs.get_mut(id).context("prepared XP entity missing")?;
                orb.handle = Some(items.register_experience_orb(*id, orb.state.position_vec())?);
            }
            *items.world_random_mut() = self.world_after;
            *actual = self.math_after;
            self.staged.revision = runtime.revision.wrapping_add(1);
            *runtime = self.staged;
            for (recipients, packets) in self.deliveries { emit(events, recipients, packets); }
            Ok(())
        })
    }
}

fn require_native_experience_world(world: &WorldBackend, players: &LivePlayers) -> Result<()> {
    players.gameplay.require_supported_floor()?;
    ensure!(matches!(world, WorldBackend::Fixture(_)), "native XP requires an owned native world");
    players.entity_source.as_ref().context("native XP source missing")?.require_owned_callback_world(world)
}

pub(super) fn prepare_furnace_experience(
    world: &mut WorldBackend, players: &mut LivePlayers, id: i32,
    rewards: &[mc121_rust::furnace::FurnaceRecipeReward], entities: &AtomicI32,
) -> Result<PreparedFurnaceExperience> {
    require_native_experience_world(world, players)?;
    let player = players.registry.get(id).context("XP actor missing")?;
    require_no_mending(players.containers.get(&id).context("XP actor inventory missing")?)?;
    let at = Vec3::new(player.position.x, player.position.y, player.position.z);
    ensure!(players.native_body_ready.contains(&id), "XP actor body is not FULL-ready");
    let index = players.item_runtime.prepare_experience_index()?;
    let level_random = players.item_runtime.world_random_mut().clone();
    let math_random = players.math_random.as_ref().context("actual Math.random owner missing")?
        .with_random(|r| Ok(r.clone()))?;
    prepare_furnace_experience_at(players, at, rewards,
        || Ok(entities.fetch_add(1, Ordering::Relaxed)), index, level_random, math_random)
}

/// AbstractFurnaceBlock.onRemove invokes getRecipesToAwardAndPopExperience at
/// Vec3.atCenterOf(blockPos), after Containers.dropContents. Unlike result-slot
/// reward collection, it has no actor, recipe unlock or Mending precondition.
/// The caller retains recipesUsed until the actual block entity is removed.
///
/// `index_after_drops` is a transaction-local copy of the actual shared index
/// with every staged ItemEntity registered in source order. Both RNG inputs are
/// their post-drop states; commit_parts checks against those states only after
/// the caller commits the staged drops. `next_entity_id` is the same staged
/// counter used for those Items; the caller reserves the complete range only
/// after all fallible preflight. This token does not clear recipesUsed.
pub(super) fn prepare_furnace_block_experience_after_drops(
    world: &mut WorldBackend, players: &mut LivePlayers, position: (i32, i32, i32),
    rewards: &[mc121_rust::furnace::FurnaceRecipeReward], next_entity_id: &mut i32,
    index_after_drops: RuntimeEntityIndex,
    level_after_drops: &LegacyRandomSource, math_after_drops: &LegacyRandomSource,
) -> Result<PreparedFurnaceExperience> {
    require_native_experience_world(world, players)?;
    let at = Vec3::new(f64::from(position.0) + 0.5, f64::from(position.1) + 0.5,
        f64::from(position.2) + 0.5);
    prepare_furnace_experience_at(players, at, rewards, || {
        let id = *next_entity_id;
        ensure!(id > 0 && id < i32::MAX, "entity ID exhausted");
        *next_entity_id = id + 1;
        Ok(id)
    }, index_after_drops, level_after_drops.clone(), math_after_drops.clone())
}

fn prepare_furnace_experience_at(
    players: &mut LivePlayers, at: Vec3,
    rewards: &[mc121_rust::furnace::FurnaceRecipeReward],
    mut next_entity_id: impl FnMut() -> Result<i32>, mut index: RuntimeEntityIndex, mut level_random: LegacyRandomSource,
    mut math_random: LegacyRandomSource,
) -> Result<PreparedFurnaceExperience> {
    ensure!(at.x.is_finite() && at.y.is_finite() && at.z.is_finite(), "nonfinite XP award position");
    let mut staged = players.experience_runtime.clone();
    let world_before = level_random.state48();
    let math_before = math_random.state48();
    let mut new_ids = Vec::new();
    let mut deliveries = Vec::new();
    for reward in rewards {
        let amount = experience::recipe_experience(reward.count, reward.experience, || Ok(math_random.next_f64()))?;
        prepare_native_experience_amount(players, at, amount, &mut staged, &mut next_entity_id,
            &mut index, &mut level_random, &mut new_ids, &mut deliveries)?;
    }
    players.item_runtime.validate_experience_orb_registration_batch(new_ids.iter().copied())?;
    Ok(PreparedFurnaceExperience { staged, expected_revision: players.experience_runtime.revision,
        new_ids, world_before, world_after: level_random, math_before, math_after: math_random, deliveries })
}

/// One genuine integer ExperienceOrb.award call. Furnace rounding stays at its
/// existing caller; this shared path never reads or draws global Math.random.
fn prepare_native_experience_amount(
    players: &LivePlayers, at: Vec3, mut remaining: i32,
    staged: &mut MainExperienceRuntime, next_entity_id: &mut impl FnMut() -> Result<i32>,
    index: &mut RuntimeEntityIndex, level_random: &mut LegacyRandomSource,
    new_ids: &mut Vec<i32>, deliveries: &mut Vec<(Vec<i32>, Vec<Packet>)>,
) -> Result<()> {
    ensure!(at.x.is_finite() && at.y.is_finite() && at.z.is_finite(), "nonfinite XP award position");
    while remaining > 0 {
        let value = experience::experience_value(remaining); remaining -= value;
        let group = level_random.next_bounded_i32(40)?;
        let candidates = index.query(experience::award_bounds(at), Some(EntityKind::ExperienceOrb), |h| {
            Ok(staged.orbs.get(&h.id).filter(|o| !o.state.removed && o.state.can_merge_with(group, value)).map(|o| o.state.bounds))
        })?;
        if let Some(handle) = candidates.first() {
            let orb = staged.orbs.get_mut(&handle.id).context("XP merge owner missing")?;
            orb.state.count = orb.state.count.wrapping_add(1); orb.state.age = 0;
        } else {
            let entity_id = next_entity_id()?;
            ensure!(entity_id > 0 && entity_id < i32::MAX, "entity ID exhausted");
            let mut random = random_seed::create_random()?;
            let state = ExperienceOrbState::new(entity_id, at, value, &mut random)?;
            index.register(entity_id, EntityKind::ExperienceOrb, at, false,
                |_| players.item_runtime.experience_visibility_at(at))?;
            staged.insert(state, random, None)?;
            let orb = staged.orbs.get_mut(&entity_id).context("prepared new XP orb missing")?;
            let chunk = ((at.x.floor() as i32).div_euclid(16), (at.z.floor() as i32).div_euclid(16));
            for (&viewer, observer) in &players.observers {
                let target = entity_tracking::TrackingTarget { entity_id, position: at,
                    base_range_blocks: 96, indirect_passenger_ranges_chunks: &[], broadcast_allowed: true };
                if players.chunk_transports.get(&viewer).is_some_and(|t|t.entity_chunk_sent(chunk))
                    && entity_tracking::within_tracking_range(*observer,target) { orb.seen.insert(viewer); }
            }
            let packets = vec![orb.state.spawn_packet(orb.tracker.spawn_position())?,
                entity::velocity_packet(entity_id, orb.tracker.spawn_velocity())?];
            deliveries.push((orb.seen.iter().copied().collect(),packets));
            new_ids.push(entity_id);
        }
    }
    Ok(())
}

/// One integer award at its source event, backed by the actual XP/index owners.
/// There is no furnace reward identity and no global-Math owner in this token.
#[derive(Clone)]
pub(super) struct PreparedNativeExperience {
    staged: MainExperienceRuntime,
    expected_revision: u64,
    new_ids: Vec<i32>,
    expected_handles: BTreeMap<i32, EntityHandle>,
    registered_handles: BTreeMap<i32, EntityHandle>,
    world_before: u64,
    world_after: LegacyRandomSource,
    deliveries: Vec<(Vec<i32>, Vec<Packet>)>,
}
impl PreparedNativeExperience {
    pub(super) fn registration_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.new_ids.iter().copied()
    }
    pub(super) fn registration_states(&self) -> impl Iterator<Item = &ExperienceOrbState> + '_ {
        self.new_ids.iter().map(|id| &self.staged.orbs.get(id)
            .expect("prepared native XP state retained").state)
    }
    pub(super) fn expected_registration_handle(&self, id: i32) -> Option<EntityHandle> {
        self.expected_handles.get(&id).copied()
    }
    /// Bind only the real Registered{kind:ExperienceOrb} event from the mixed
    /// native batch. Its index generation must equal the staged source order.
    pub(super) fn attach_registered(&mut self, id: i32, handle: EntityHandle) -> Result<()> {
        ensure!(self.expected_handles.get(&id).copied() == Some(handle),
            "native XP registration differs from its prepared source handle");
        ensure!(!self.registered_handles.contains_key(&id), "native XP registration repeated");
        self.registered_handles.insert(id, handle);
        Ok(())
    }
    fn rebind_existing_handles(&mut self, runtime: &MainExperienceRuntime) -> Result<()> {
        for (id, orb) in &mut self.staged.orbs {
            if !self.expected_handles.contains_key(id) {
                orb.handle = runtime.orbs.get(id).context("prior native XP owner missing")?.handle;
            }
        }
        Ok(())
    }
    /// Mixed-batch publication owns actual index/tick registration and the one
    /// final Level RNG. Install this award's XP state at its source boundary,
    /// after binding every one of its new registered handles. Merge-only and
    /// zero awards still have a real checkpoint and ordered delivery boundary.
    pub(super) fn commit_registered_parts(mut self, runtime: &mut MainExperienceRuntime,
        events: &broadcast::Sender<PlayerEvent>) -> Result<()> {
        ensure!(runtime.revision == self.expected_revision, "XP state changed before mixed award checkpoint");
        ensure!(self.registered_handles == self.expected_handles,
            "mixed XP award has unregistered source entities");
        self.rebind_existing_handles(runtime)?;
        for (id, handle) in &self.registered_handles {
            self.staged.orbs.get_mut(id).context("registered native XP state missing")?.handle = Some(*handle);
        }
        self.staged.revision = runtime.revision.wrapping_add(1);
        *runtime = self.staged;
        for (recipients, packets) in self.deliveries { emit(events, recipients, packets); }
        Ok(())
    }
    /// Commit at this exact award boundary, after only its source predecessors.
    /// The caller owns the shared entity counter and has applied the preceding
    /// getExperienceAmount draw/drop state to the actual Level RNG. Later
    /// drops cannot be committed first without changing index encounter order.
    pub(super) fn commit_parts(mut self, runtime: &mut MainExperienceRuntime,
        items: &mut RuntimeEntities, events: &broadcast::Sender<PlayerEvent>) -> Result<()> {
        ensure!(runtime.revision == self.expected_revision, "XP state changed after integer award preparation");
        ensure!(items.world_random_state48() == self.world_before, "Level RNG changed before integer XP award");
        items.validate_experience_orb_registration_batch(self.new_ids.iter().copied())?;
        self.rebind_existing_handles(runtime)?;
        let mut index = items.prepare_experience_index()?;
        for id in &self.new_ids {
            let at = self.staged.orbs.get(id).context("prepared integer XP entity missing")?.state.position_vec();
            let handle = index.register(*id, EntityKind::ExperienceOrb, at, false,
                |_| items.experience_visibility_at(at))?;
            ensure!(self.expected_handles.get(id).copied() == Some(handle),
                "shared index source order changed before integer XP award");
        }
        for id in self.new_ids.clone() {
            let at = self.staged.orbs.get(&id).expect("validated integer XP entity").state.position_vec();
            let handle = items.register_experience_orb(id, at)?;
            self.attach_registered(id, handle)?;
        }
        *items.world_random_mut() = self.world_after.clone();
        self.commit_registered_parts(runtime, events)
    }
}

/// Cloneable transaction-local XP proposals for repeated synchronous onTake.
/// This creates no additional live inventory/index/RNG owner. Award updates
/// the caller's one staged index/Level RNG; tokens retain its read-only states.
#[derive(Clone)]
pub(super) struct NativeMenuExperienceStage {
    staged: MainExperienceRuntime,
    awards: Vec<PreparedNativeExperience>,
}
impl NativeMenuExperienceStage {
    pub(super) fn new(players: &LivePlayers) -> Result<Self> {
        players.gameplay.require_supported_floor()?;
        Ok(Self { staged: players.experience_runtime.clone(), awards: Vec::new() })
    }
    pub(super) fn award_count(&self) -> usize { self.awards.len() }
    pub(super) fn registration_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.awards.iter().flat_map(|award| award.registration_ids())
    }
    pub(super) fn awards(&self) -> &[PreparedNativeExperience] { &self.awards }
    /// Use the actual locked players for visibility, observers and sent chunks.
    /// On failure discard this stage together with the passed staging owners;
    /// no real orb/index/RNG has been installed or published.
    pub(super) fn award(&mut self, players: &LivePlayers, amount: i32, at: Vec3,
        next_entity_id: &mut i32, index: &mut RuntimeEntityIndex,
        level_random: &mut LegacyRandomSource) -> Result<()> {
        players.gameplay.require_supported_floor()?;
        let expected_revision = self.staged.revision;
        let world_before = level_random.state48();
        let mut new_ids = Vec::new();
        let mut deliveries = Vec::new();
        prepare_native_experience_amount(players, at, amount, &mut self.staged, &mut || {
            let id = *next_entity_id;
            ensure!(id > 0 && id < i32::MAX, "entity ID exhausted");
            *next_entity_id = id + 1;
            Ok(id)
        }, index, level_random, &mut new_ids, &mut deliveries)?;
        players.item_runtime.validate_experience_orb_registration_batch(new_ids.iter().copied())?;
        let expected_handles = new_ids.iter().map(|id| {
            let entry = index.by_id(*id).context("prepared XP index entry missing")?;
            ensure!(entry.kind == EntityKind::ExperienceOrb, "prepared XP index kind changed");
            Ok((*id, entry.handle))
        }).collect::<Result<BTreeMap<_, _>>>()?;
        self.staged.revision = expected_revision.wrapping_add(1);
        self.awards.push(PreparedNativeExperience { staged: self.staged.clone(), expected_revision,
            new_ids, expected_handles, registered_handles: BTreeMap::new(),
            world_before, world_after: level_random.clone(), deliveries });
        Ok(())
    }
    /// Consume each token at its recorded synchronous source award event.
    /// This is deliberately not an end-of-click batch publication API.
    pub(super) fn into_awards(self) -> Vec<PreparedNativeExperience> { self.awards }
}

/// Single award after genuine preceding drops and the grindstone's already
/// sampled integer amount. Only predecessor Items belong in index_after_drops;
/// a later result THROW must not be moved ahead of its onTake XP award.
pub(super) fn prepare_grindstone_experience_after_drops(
    world: &mut WorldBackend, players: &mut LivePlayers, position: (i32, i32, i32),
    amount: i32, next_entity_id: &mut i32, mut index_after_drops: RuntimeEntityIndex,
    level_after_drops: &LegacyRandomSource,
) -> Result<PreparedNativeExperience> {
    require_native_experience_world(world, players)?;
    let mut stage = NativeMenuExperienceStage::new(players)?;
    let mut level_random = level_after_drops.clone();
    stage.award(players, amount,
        Vec3::new(f64::from(position.0) + 0.5, f64::from(position.1) + 0.5, f64::from(position.2) + 0.5),
        next_entity_id, &mut index_after_drops, &mut level_random)?;
    Ok(stage.into_awards().pop().expect("one actual integer award was prepared"))
}

/// Mending requires the actual equipped-item/enchantment effect owner. The
/// bounded host rejects that path before consuming a reward or pickup delay.
fn require_no_mending(container: &SharedContainer) -> Result<()> {
    let id = mending_id()?;
    container.read(|menu| -> Result<()> {
        let equipment = mc121_rust::equipment::PlayerEquipment::from_inventory(&menu.inventory)?;
        for slot in EquipmentSlot::ALL {
            let stack = equipment.get(slot);
            if stack.is_empty() || stack.component_value(3)?.and_then(|v|v.as_i64()).unwrap_or(0) <= 0 { continue; }
            if let Some(enchantments) = stack.component_value(9)? {
                ensure!(!enchantments["enchantments"].as_array().context("invalid item enchantments")?.iter()
                    .any(|e|e["id"].as_i64()==Some(i64::from(id))&&e["level"].as_i64().unwrap_or(0)>0),
                    "XP repair requires the actual Mending effect owner");
            }
        }
        Ok(())
    })?
}
fn mending_id() -> Result<i32> {
    static ID: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    if let Some(id) = ID.get() { return Ok(*id); }
    let capture: serde_json::Value = serde_json::from_str(include_str!("../fixtures/configuration.json"))?;
    for packet in capture["packets"].as_array().context("configuration packets missing")? {
        if packet["name"] != "registry_data" { continue; }
        let bytes = STANDARD.decode(packet["payload"].as_str().context("registry payload missing")?)?;
        let mut r = Reader::new(&bytes);
        if r.string(32767)? != "minecraft:enchantment" { continue; }
        for index in 0..r.varint()? {
            let name = r.string(32767)?;
            if r.bool()? { let (_,n)=Tag::decode_anonymous_prefix(r.remaining())?;r.bytes(n)?; }
            if name == "minecraft:mending" { let _=ID.set(index);return Ok(index); }
        }
    }
    bail!("pinned Mending definition missing")
}

struct OrbServices<'a> {
    items: &'a mut RuntimeEntities,
    gameplay: &'a mut MainItemGameplay,
    registry: &'a EntityRegistry,
    manager: &'a mc121_rust::runtime_chunk_manager::RuntimeChunkManager,
    handle: EntityHandle,
    events: &'a broadcast::Sender<PlayerEvent>,
}
impl RuntimeItemHost for OrbServices<'_> {
    fn position_changed(&mut self, state: &EntityPhysicalState) -> Result<()> {
        if !state.removed { self.items.move_experience_orb(self.handle, state.position_vec())?; }
        Ok(())
    }
    fn profile(&mut self, event: ProfileEvent) {
        let mut frame = self.gameplay.frame(self.registry, self.manager, true);
        RuntimeGameplayHooks::profile(&mut frame, event);
    }
    fn effect(&mut self, state: &EntityPhysicalState, effect: RuntimeItemEffect) -> Result<()> {
        match effect {
            RuntimeItemEffect::HitGround { affected_block } => {
                let mut frame = self.gameplay.frame(self.registry, self.manager, true);
                frame.hit_ground(state, affected_block)
            }
            RuntimeItemEffect::Sound { sound, volume, pitch } => {
                let packet = mc121_rust::item_effects::sound_packet(sound, state.position_vec(), volume, pitch, self.items.next_sound_seed()?);
                let recipients = self.registry.snapshot().into_iter().filter(|p| packet.in_range(Vec3::new(p.position.x,p.position.y,p.position.z),true)).map(|p|p.entity_id).collect();
                emit(self.events, recipients, vec![packet.packet]); Ok(())
            }
            RuntimeItemEffect::Base(entity_base::BaseEffect::DiscardBelowWorld) => Ok(()),
            RuntimeItemEffect::Base(entity_base::BaseEffect::MeltFrozen { position, event, data }) => {
                let origin=Vec3::new(f64::from(position.0),f64::from(position.1),f64::from(position.2));
                let recipients=self.registry.snapshot().iter().filter(|p|Vec3::new(p.position.x,p.position.y,p.position.z).subtract(origin).length_squared()<4096.).map(|p|p.entity_id).collect();
                emit(self.events, recipients,
                    vec![mc121_rust::item_effects::level_event_packet(event,position,data,false)]); Ok(())
            }
            _ => bail!("unported native XP base effect"),
        }
    }
    fn entity_damage(&mut self, _: &EntityPhysicalState, _: &ItemStack, _: i32, _: Option<i32>) -> Result<()> { bail!("XP must not use ItemEntity damage hooks") }
    fn discarded(&mut self, _: &EntityPhysicalState, _: &ItemStack, _: i32) -> Result<()> { bail!("XP must not use ItemEntity discard hooks") }
}

struct OrbWorld<'a> {
    world: &'a mut WorldBackend,
    services: OrbServices<'a>,
    others: &'a mut BTreeMap<i32, NativeOrb>,
    facts: ItemWorldFacts,
    shapes: &'a mut mc121_rust::fluid_physics::FluidShapeCache,
    rain: Option<mc121_rust::native_rain::NativeFlatPlainsRainFrame<'a>>,
}
impl<'w> OrbWorld<'w> {
    fn provider<'a>(&'a mut self, s: &EntityPhysicalState, random: &'a mut LegacyRandomSource) -> Result<RuntimeItemWorld<'a, LegacyRandomSource, OrbServices<'w>>> {
        let provider = RuntimeItemWorld::new_experience_orb(self.world, s, random, &mut self.services, self.facts, &[])?
            .with_fluid_shape_cache(self.shapes);
        Ok(if let Some(rain) = self.rain.as_mut() { provider.with_rain_exposure(rain) } else { provider })
    }
    fn following(&self, id: i32) -> Option<FollowingPlayer> {
        let p = self.services.registry.get(id)?;
        let dimensions = player_physics::player_dimensions(p.metadata.pose, 1.).ok()?;
        Some(FollowingPlayer { entity_id: id, position: Vec3::new(p.position.x,p.position.y,p.position.z),
            eye_height: dimensions.eye_height, spectator: p.gamemode == 3,
            dead: !matches!(p.entity_data.get(9),Some(MetadataValue::Float(v))if *v>0.) })
    }
}
impl ExperienceOrbWorld for OrbWorld<'_> {
    fn base_tick(&mut self, orb: &mut ExperienceOrbState, random: &mut LegacyRandomSource) -> Result<()> {
        let mut provider = self.provider(&orb.physical, random)?;
        let health = &mut orb.health;
        entity_base::base_tick(&mut orb.physical, &mut provider, |s,_,_,amount| {
            s.hurt_marked = true; *health = (*health as f32 - amount) as i32;
            if *health <= 0 { s.removed = true; } Ok(true)
        })
    }
    fn lava_at_block(&mut self, orb: &ExperienceOrbState) -> Result<bool> {
        Ok(RuntimeFluidWorld { world: self.world }.fluid(orb.block_position)?.kind == FluidKind::Lava)
    }
    fn no_collision(&mut self, orb: &ExperienceOrbState) -> Result<bool> {
        let mut collision = RuntimeCollision::new(self.world, CollisionActor { entity_id: orb.entity_id, position: orb.position_vec() }, &[], self.facts.border);
        player_physics::no_collision(&mut collision, player_physics::CollisionQuery { actor_id: orb.entity_id,
            bounds: orb.bounds, context: EntityCollisionContext { bottom_y:orb.position.y,descending:false,
                fall_distance:orb.fall_distance,is_falling_block:false,can_walk_on_powder_snow:false } })
    }
    fn move_towards_closest_space(&mut self, orb: &mut ExperienceOrbState, random: &mut LegacyRandomSource) -> Result<()> {
        let point = Vec3::new(orb.position.x,(orb.bounds.min_y+orb.bounds.max_y)/2.,orb.position.z);
        let mut provider = self.provider(&orb.physical, random)?;
        mc121_rust::item_entity::move_towards_closest_space(&mut orb.physical, point, &mut provider)
    }
    fn nearest_player(&mut self, orb: &ExperienceOrbState, radius: f64) -> Result<Option<FollowingPlayer>> {
        let mut distance = radius*radius; let mut result = None;
        for p in self.services.registry.snapshot() {
            if let Some(p) = self.following(p.entity_id).filter(|p| !p.spectator && !p.dead) {
                let d = p.position.subtract(orb.position_vec()).length_squared();
                if d < distance { distance=d; result=Some(p); }
            }
        }
        Ok(result)
    }
    fn player(&mut self, id: i32) -> Result<Option<FollowingPlayer>> { Ok(self.following(id)) }
    fn merge_neighbours(&mut self, orb: &mut ExperienceOrbState) -> Result<()> {
        let others = &*self.others;
        let candidates = self.services.items.query_experience_orbs(orb.bounds.inflate(Vec3::new(0.5,0.5,0.5)), |h| {
            Ok(others.get(&h.id).filter(|o|o.handle==Some(h)&&o.state.can_merge_with(orb.entity_id,orb.value)).map(|o|o.state.bounds))
        })?;
        for handle in candidates {
            if let Some(mut other) = self.others.remove(&handle.id) {
                orb.merge_from(&mut other.state);
                self.services.items.remove_experience_orb(handle)?;
                emit(self.services.events, other.seen.into_iter().collect(), vec![experience::remove_packet(&[handle.id])]);
            }
        }
        Ok(())
    }
    fn move_orb(&mut self, orb: &mut ExperienceOrbState, random: &mut LegacyRandomSource, movement: Vec3) -> Result<()> {
        let mut provider = self.provider(&orb.physical, random)?.with_experience_orb_health(&mut orb.health)?;
        entity_move::move_entity(&mut orb.physical, MoverType::SelfMove, movement,
            EntityMoveContext { step_height:0.,descending:false,is_falling_block:false,
                can_walk_on_powder_snow:false,movement_emission:MovementEmission::None }, &mut provider)?;
        Ok(())
    }
    fn ground_friction(&mut self, orb: &ExperienceOrbState, random: &mut LegacyRandomSource) -> Result<f32> {
        let mut provider = self.provider(&orb.physical, random)?;
        let p = entity_move::on_position(&orb.physical,0.999999f32,&mut provider)?;
        mc121_rust::player_body::friction(provider.world.block(p.0,p.1,p.2)?)
    }
}

impl MainExperienceRuntime {
    pub(super) fn tick_one(
        &mut self, handle: EntityHandle, world: &mut WorldBackend, items: &mut RuntimeEntities,
        gameplay: &mut MainItemGameplay, registry: &EntityRegistry, chunks: &mut MainChunkRuntime,
        shapes: &mut mc121_rust::fluid_physics::FluidShapeCache,
        rain: Option<&mut mc121_rust::rain_physics::FreshFlatPlainsRain>, context: &WorldContext,
        game_time: i64, border: CollisionBorder, events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        if items.entity_kind(handle) != Some(EntityKind::ExperienceOrb) { return Ok(()); }
        ensure!(self.orbs.get(&handle.id).is_some_and(|o|o.handle==Some(handle)), "registered XP owner/generation missing");
        let facts = context.item_facts(game_time,border,false)?;
        let (light,manager)=chunks.native_lighting_parts()?;
        let mut orb = self.orbs.remove(&handle.id).context("registered XP owner disappeared")?;
        if orb.state.removed { items.remove_experience_orb(handle)?; return Ok(()); }
        let result = {
            let rain=rain.map(|owner|mc121_rust::native_rain::NativeFlatPlainsRainFrame::new(owner,light,manager));
            let services = OrbServices { items, gameplay, registry, manager, handle, events };
            let mut host = OrbWorld { world, services, others:&mut self.orbs, facts, shapes, rain };
            experience::tick(&mut orb.state,&mut orb.random,&mut host)
        };
        if orb.state.removed {
            items.remove_experience_orb(handle)?;
            emit(events,orb.seen.into_iter().collect(),vec![experience::remove_packet(&[handle.id])]);
        } else { self.orbs.insert(handle.id,orb); }
        self.revision = self.revision.wrapping_add(1);
        result
    }
    pub(super) fn pickup_one(
        &mut self, handle:EntityHandle, id:i32, physical:&EntityPhysicalState,
        items:&mut RuntimeEntities, registry:&mut EntityRegistry,
        lifecycles:&mut BTreeMap<[u8;16],PlayerLifecycle>, containers:&BTreeMap<i32,SharedContainer>,
        events:&broadcast::Sender<PlayerEvent>,
    )->Result<()> {
        if self.pickup_delay.get(&id).copied().unwrap_or(0)!=0 { return Ok(()); }
        let Some(orb)=self.orbs.get(&handle.id).filter(|o|o.handle==Some(handle)&&!o.state.removed) else { return Ok(()); };
        if !physical.bounds.inflate(Vec3::new(1.,0.5,1.)).intersects(&orb.state.bounds) { return Ok(()); }
        let player=registry.get(id).context("XP player missing")?;
        if player.gamemode==3 || !matches!(player.entity_data.get(9),Some(MetadataValue::Float(h))if *h>0.) {return Ok(());}
        require_no_mending(containers.get(&id).context("XP player container missing")?)?;
        let uuid=player.profile.uuid;
        let score=match player.entity_data.get(16){Some(MetadataValue::Int(v))=>*v,_=>bail!("XP score owner missing")};
        let amount=orb.state.value;
        let audience=orb.seen.iter().copied().collect::<Vec<_>>();
        let lifecycle=lifecycles.get_mut(&uuid).context("XP lifecycle missing")?;
        let mut xp=lifecycle.experience.clone();let mut levels=Vec::new();
        experience::give_experience_points(&mut xp,amount,|level|{levels.push(level);Ok(())})?;
        self.pickup_delay.insert(id,2);
        emit(events,audience,vec![experience::take_packet(handle.id,id)]);
        registry.set_entity_data(id,16,MetadataValue::Int(score.wrapping_add(amount)),false)?;
        lifecycle.experience=xp;
        for level in levels {
            let last=self.last_level_up.get(&id).copied().unwrap_or(0);
            if level>0 && level%5==0 && (last as f32)<(physical.tick_count as f32-100.) {
                let volume=if level>30 {1.} else {level as f32/30.}*0.75f32;
                let seed=items.next_sound_seed()?;
                let p=physical.position_vec();let mut w=Writer::new();
                w.varint(1114+1).varint(7).i32((p.x*8.)as i32).i32((p.y*8.)as i32).i32((p.z*8.)as i32).f32(volume).f32(1.).i64(seed);
                let recipients=registry.snapshot().iter().filter(|q|Vec3::new(q.position.x,q.position.y,q.position.z).subtract(p).length_squared()<256.).map(|q|q.entity_id).collect();
                emit(events,recipients,vec![Packet{id:0x68,payload:w.into_inner()}]);
                self.last_level_up.insert(id,physical.tick_count);
            }
        }
        // The embedding Player.doTick sends SetExperience from finish_player_tick.
        let orb=self.orbs.get_mut(&handle.id).context("XP pickup owner disappeared")?;
        orb.state.count=orb.state.count.wrapping_sub(1);
        if orb.state.count==0 {self.remove(handle.id,items,events)?;}
        self.revision=self.revision.wrapping_add(1);
        Ok(())
    }
}

impl LivePlayers {
    pub(super) fn finish_native_experience_player_tick(&mut self,id:i32,events:&broadcast::Sender<PlayerEvent>)->Result<()> {
        let player=self.registry.get(id).context("XP player missing")?;
        let xp=&self.player_lifecycle.get(&player.profile.uuid).context("XP player lifecycle missing")?.experience;
        if self.experience_runtime.last_sent_experience.get(&id).copied().unwrap_or(-1)!=xp.total {
            self.experience_runtime.last_sent_experience.insert(id,xp.total);
            emit(events,vec![id],vec![experience::experience_packet(xp)]);
        }
        Ok(())
    }
    pub(super) fn begin_native_experience_player_tick(&mut self,id:i32) {
        self.experience_runtime.begin_player_tick(id);
    }
    pub(super) fn pickup_one_native_experience(&mut self,handle:EntityHandle,id:i32,physical:&EntityPhysicalState,
        events:&broadcast::Sender<PlayerEvent>)->Result<()> {
        self.gameplay.require_supported_floor()?;
        self.experience_runtime.pickup_one(handle,id,physical,&mut self.item_runtime,&mut self.registry,
            &mut self.player_lifecycle,&self.containers,events)
    }
    pub(super) fn pickup_native_experience_for_player(&mut self,id:i32,physical:&EntityPhysicalState,
        events:&broadcast::Sender<PlayerEvent>)->Result<()> {
        let runtime=&self.experience_runtime;
        let candidates=self.item_runtime.query_experience_orbs(physical.bounds.inflate(Vec3::new(1.,0.5,1.)),|h|Ok(runtime.bounds(h)))?;
        for handle in candidates {self.pickup_one_native_experience(handle,id,physical,events)?;}
        Ok(())
    }
}
