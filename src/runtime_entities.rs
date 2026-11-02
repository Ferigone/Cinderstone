//! Authoritative item storage joining the proved tick, spatial and sender cores.
//!
//! Tracking, level simulation and connection-phase pickup are separate calls.
//! The host owns ticket visibility, player lifecycle, inventory and gameplay
//! listeners. Unsupported world callbacks remain errors from RuntimeItemWorld.
use crate::{
    entity_base::BaseEffect,
    entity_move::ProfileEvent,
    entity_spatial::Visibility,
    entity_state::EntityPhysicalState,
    entity_tick::EntityTickList,
    entity_tracking_order::TrackingEntityMap,
    fluid_physics::FluidShapeCache,
    geometry::{Aabb, Vec3},
    inventory::ItemStack,
    item_entity::{self, ItemPickupHost, ItemState, Pickup},
    item_replication::ItemReplication,
    protocol::Packet,
    rain_physics::RainExposure,
    random::LegacyRandomSource,
    replication::EntitySnapshot,
    runtime_collision::CollisionEntity,
    runtime_entity_index::{EntityHandle, EntityKind, PersistentEntityIndexOrder, RuntimeEntityIndex},
    runtime_item_world::{ItemWorldFacts, RuntimeItemEffect, RuntimeItemHost, RuntimeItemWorld},
    runtime_world::RuntimeWorld,
};
use anyhow::{ensure, Context, Result};
use std::{cell::RefCell, collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub enum RuntimeEntityEvent {
    Update {
        revision: u64,
        snapshot: Box<EntitySnapshot<ItemState>>,
        packets: Vec<Packet>,
    },
    Removed {
        revision: u64,
        entity_id: i32,
    },
    Collected {
        revision: u64,
        pickup: Pickup,
    },
    Effect {
        revision: u64,
        deliveries: Vec<EffectDelivery>,
    },
}

#[derive(Clone, Debug)]
pub struct EffectDelivery {
    /// Audience captured synchronously at the original world callback.
    pub recipients: Vec<i32>,
    pub packet: Packet,
}

/// Native-only checkpoint cohort. Historical sender hash layout and chunk
/// readiness are deliberately absent: native resume creates fresh senders.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistentEntityOrder {
    pub index: PersistentEntityIndexOrder,
    pub ticking: Vec<EntityHandle>,
}
impl PersistentEntityOrder {
    pub fn validate_entries(&self, entries: &[(i32, EntityKind, Vec3)]) -> Result<()> {
        self.index.validate_entries(entries)?;
        let by_id = self.index.registrations.iter().map(|entry| (entry.handle.id, entry.handle)).collect::<BTreeMap<_, _>>();
        let mut seen = std::collections::BTreeSet::new();
        for handle in &self.ticking {
            ensure!(by_id.get(&handle.id) == Some(handle) && seen.insert(*handle),
                "invalid persistent tick-order reference");
        }
        Ok(())
    }
}

/// Facts from the actual player's connection-phase Player.aiStep call.
#[derive(Clone, Copy, Debug)]
pub struct PickupPlayer {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub bounds: Aabb,
    pub alive: bool,
    pub spectator: bool,
    /// None means unmounted or the actual vehicle has already been removed.
    pub vehicle_bounds: Option<Aabb>,
}

pub trait RuntimeEntityHost {
    /// Optional instrumentation only; never substitutes a gameplay callback.
    fn observe_item_tick(&mut self, _state: &ItemState, _random: &LegacyRandomSource) {}
    /// External entities retain their actual Level.tickNonPassenger callback.
    fn level_player_tick(&mut self, player: EntityHandle) -> Result<()>;
    fn profile(&mut self, event: ProfileEvent);
    /// Sound has already consumed one nextLong from the separately owned
    /// Level.threadSafeRandom; Level.random is unchanged. Other effects
    /// carry no invented seed. Gameplay listeners execute synchronously here.
    fn effect(
        &mut self,
        state: &EntityPhysicalState,
        effect: RuntimeItemEffect,
        sound_seed: Option<i64>,
    ) -> Result<Vec<EffectDelivery>>;
    fn entity_damage(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
        causing_entity: Option<i32>,
    ) -> Result<()>;
    fn discarded(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
    ) -> Result<()>;
    /// Vanilla Inventory.add. On success the host must put TakeItemEntity
    /// before its private menu synchronization, using original_count.
    fn add_to_inventory(
        &mut self,
        player: PickupPlayer,
        item_entity_id: i32,
        original_count: i32,
        stack: &mut ItemStack,
    ) -> Result<bool>;
    /// Player.take at its literal call site, before discard/count restoration.
    /// The Collected event has already entered the shared outbox, so menu
    /// broadcastChanges can follow it without crossing independent streams.
    fn collect_item(
        &mut self,
        player: PickupPlayer,
        state: &ItemState,
        original_count: i32,
        accepted_count: i32,
    ) -> Result<()>;
    fn collected(
        &mut self,
        player: PickupPlayer,
        state: &ItemState,
        pickup: Pickup,
        original_item_id: i32,
    ) -> Result<()>;
}

struct RuntimeItem {
    handle: EntityHandle,
    state: ItemState,
    random: LegacyRandomSource,
}
struct TrackedItem {
    handle: EntityHandle,
    sender: ItemReplication,
    last_section: crate::entity_spatial::SectionKey,
}

struct PreparedItem {
    state: ItemState,
    random: LegacyRandomSource,
    tracking: Option<(ItemReplication, EntitySnapshot<ItemState>)>,
}

/// Read-only registration facts captured from the actual external XP owner.
/// The physical state, per-entity RNG and packet sender stay with that owner;
/// this token does not create another orb or expose an arbitrary index write.
#[derive(Clone, Copy, Debug)]
pub struct ExperienceOrbRegistration {
    entity_id: i32,
    position: Vec3,
}

impl ExperienceOrbRegistration {
    pub fn from_state(state: &crate::experience::ExperienceOrbState) -> Result<Self> {
        state.validate()?;
        // Validate the actual outgoing representations before a menu/world
        // transaction can commit. Encoding does not drain the owner's dirties.
        state.spawn_packet(state.position)?;
        crate::entity::velocity_packet(state.entity_id, state.velocity)?;
        ensure!(!state.removed, "cannot register a removed experience orb");
        Ok(Self {
            entity_id: state.entity_id,
            position: state.position_vec(),
        })
    }
}

/// One actual source-ordered native addition. Cloning this proposal preserves
/// captured identities; only consuming a prepared batch registers live owners.
#[derive(Clone, Debug)]
pub enum NativeEntityAddition {
    Item(ItemState, LegacyRandomSource),
    ExperienceOrb(ExperienceOrbRegistration),
}

enum PreparedNativeEntity {
    Item(PreparedItem),
    ExperienceOrb(ExperienceOrbRegistration),
}

/// The before/registered pair surrounds each real insertion, including the
/// item's tracking outbox publication or the XP section/tracking/tick entry.
/// Source effects without an insertion run in Before or Complete. A callback
/// receives no mutable store and cannot add an unvalidated identity.
#[derive(Clone, Copy, Debug)]
pub enum NativeEntityBatchEvent {
    Before { ordinal: usize },
    Registered { ordinal: usize, handle: EntityHandle, kind: EntityKind },
    Complete { len: usize },
}

/// A complete mixed Item/XP registration plan retaining the actual store's
/// exclusive borrow. Dropping it changes no index, outbox, tick owner or RNG.
pub struct PreparedNativeEntityBatch<'a> {
    store: &'a mut RuntimeEntities,
    entries: Vec<PreparedNativeEntity>,
}

impl PreparedNativeEntityBatch<'_> {
    /// Callbacks must use previously validated canonical owners and infallible
    /// prepared actions. XP state/merge/delivery checkpoints stay with the XP
    /// owner and are installed at the corresponding source event; Registered
    /// supplies the handle from this actual insertion, never a guessed one.
    /// Level RNG was consumed during synchronous staging and is installed once
    /// under this exclusive borrow. The independent sound owner remains live.
    pub fn commit_with_world_random_ordered(
        self,
        random: LegacyRandomSource,
        mut publish: impl FnMut(NativeEntityBatchEvent, &mut ItemBatchPublication<'_>),
    ) -> Vec<EntityHandle> {
        let Self { store, entries } = self;
        store.world_random = random;
        let len = entries.len();
        let mut handles = Vec::with_capacity(len);
        for (ordinal, entry) in entries.into_iter().enumerate() {
            publish(NativeEntityBatchEvent::Before { ordinal }, &mut ItemBatchPublication {
                sound_random: &mut store.sound_random,
            });
            let (handle, kind) = match entry {
                PreparedNativeEntity::Item(item) => (store.insert_prepared_item(item), EntityKind::Item),
                PreparedNativeEntity::ExperienceOrb(registration) => (
                    store.register_experience_orb(registration.entity_id, registration.position)
                        .expect("exclusive mixed preparation preserves validated XP identities and capacity"),
                    EntityKind::ExperienceOrb,
                ),
            };
            publish(NativeEntityBatchEvent::Registered { ordinal, handle, kind },
                &mut ItemBatchPublication { sound_random: &mut store.sound_random });
            handles.push(handle);
        }
        publish(NativeEntityBatchEvent::Complete { len }, &mut ItemBatchPublication {
            sound_random: &mut store.sound_random,
        });
        handles
    }
}

/// Validated items held with an exclusive store borrow until the menu commits.
/// Dropping this token publishes nothing and consumes no store generation/RNG.
pub struct PreparedItemBatch<'a> {
    store: &'a mut RuntimeEntities,
    items: Vec<PreparedItem>,
}

/// Restricted publication capability borrowed from the actual prepared store.
/// It exposes only Level.threadSafeRandom sound-seed consumption; callers
/// cannot inspect, insert, remove or otherwise mutate prepared/live entities.
pub struct ItemBatchPublication<'a> {
    sound_random: &'a mut Option<LegacyRandomSource>,
}

impl ItemBatchPublication<'_> {
    /// The same independent level sound owner as RuntimeEntities.next_sound_seed.
    /// A caller must certify its availability before an infallible batch commit.
    pub fn next_sound_seed(&mut self) -> Result<i64> {
        Ok(self
            .sound_random
            .as_mut()
            .context("level sound random owner unavailable")?
            .next_i64())
    }
}

impl PreparedItemBatch<'_> {
    pub fn commit(self) -> Vec<EntityHandle> {
        self.commit_inner(None, |_, _| {})
    }
    pub fn commit_with_world_random(self, random: LegacyRandomSource) -> Vec<EntityHandle> {
        self.commit_inner(Some(random), |_, _| {})
    }
    /// Publish interleaved effects at their source position around actual item
    /// additions. The callback runs before each insertion (0..len), then once
    /// at len after all insertions, including once at zero for an empty batch.
    ///
    /// Preparation retains the exclusive store borrow and all existing entity,
    /// tracking and outbox validation. Callers must preflight callback owners;
    /// this commit cannot roll back already published items or consumed sounds.
    pub fn commit_with_world_random_ordered(
        self,
        random: LegacyRandomSource,
        before_item: impl FnMut(usize, &mut ItemBatchPublication<'_>),
    ) -> Vec<EntityHandle> {
        self.commit_inner(Some(random), before_item)
    }
    fn commit_inner(
        self,
        random: Option<LegacyRandomSource>,
        mut before_item: impl FnMut(usize, &mut ItemBatchPublication<'_>),
    ) -> Vec<EntityHandle> {
        let Self { store, items } = self;
        if let Some(random) = random {
            store.world_random = random;
        }
        let count = items.len();
        let mut handles = Vec::with_capacity(count);
        for (ordinal, item) in items.into_iter().enumerate() {
            before_item(
                ordinal,
                &mut ItemBatchPublication {
                    sound_random: &mut store.sound_random,
                },
            );
            let handle = store.insert_prepared_item(item);
            handles.push(handle);
        }
        before_item(
            count,
            &mut ItemBatchPublication {
                sound_random: &mut store.sound_random,
            },
        );
        handles
    }
}

pub struct RuntimeEntities {
    index: RuntimeEntityIndex,
    ticks: EntityTickList<EntityHandle>,
    items: BTreeMap<i32, RuntimeItem>,
    tracking: BTreeMap<i32, TrackedItem>,
    tracking_order: TrackingEntityMap<EntityHandle>,
    chunk_visibility: BTreeMap<(i32, i32), Visibility>,
    world_random: LegacyRandomSource,
    sound_random: Option<LegacyRandomSource>,
    outbox: EntityOutbox,
    native_tick_resume_order: Vec<EntityHandle>,
    native_tick_resume_seen: std::collections::BTreeSet<EntityHandle>,
}
type EntityPublisher = Arc<dyn Fn(RuntimeEntityEvent) + Send + Sync>;
#[derive(Default)]
struct EntityOutbox {
    revision: u64,
    events: Vec<RuntimeEntityEvent>,
    publisher: Option<EntityPublisher>,
}
impl EntityOutbox {
    fn send(&mut self, event: RuntimeEntityEvent) {
        if let Some(publish) = &self.publisher {
            publish(event);
        } else {
            self.events.push(event);
        }
    }
}
impl RuntimeEntities {
    /// Send synchronously into a shared ordered outbox with menu/world packets.
    /// The sink must not re-enter this store. Tests can retain the local queue.
    pub fn with_publisher(
        mut self,
        publisher: Arc<dyn Fn(RuntimeEntityEvent) + Send + Sync>,
    ) -> Self {
        self.outbox.publisher = Some(publisher);
        self
    }
    /// Supply the level RNG state; this constructor does not invent a seed.
    pub fn new(world_random: LegacyRandomSource) -> Self {
        Self {
            index: RuntimeEntityIndex::new(),
            ticks: EntityTickList::new(),
            items: BTreeMap::new(),
            tracking: BTreeMap::new(),
            tracking_order: TrackingEntityMap::new(),
            chunk_visibility: BTreeMap::new(),
            world_random,
            sound_random: None,
            outbox: EntityOutbox::default(),
            native_tick_resume_order: Vec::new(),
            native_tick_resume_seen: std::collections::BTreeSet::new(),
        }
    }
    /// Consume the actual independent Level.threadSafeRandom stream. The host
    /// serializes this owner with its level/registry lock; no concurrent CAS
    /// semantics or fallback to Level.random are implied by this builder.
    pub fn with_sound_random(mut self, sound_random: LegacyRandomSource) -> Self {
        self.sound_random = Some(sound_random);
        self
    }
    pub fn sound_random_state48(&self) -> Option<u64> {
        self.sound_random.as_ref().map(LegacyRandomSource::state48)
    }
    /// Level.playSound uses the level's distinct ThreadSafeLegacy stream.
    pub fn next_sound_seed(&mut self) -> Result<i64> {
        Ok(self
            .sound_random
            .as_mut()
            .context("level sound random owner unavailable")?
            .next_i64())
    }
    pub fn world_random_mut(&mut self) -> &mut LegacyRandomSource {
        &mut self.world_random
    }
    pub fn item(&self, id: i32) -> Option<&ItemState> {
        self.items.get(&id).map(|entry| &entry.state)
    }
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    pub fn entity_kind(&self, handle: EntityHandle) -> Option<EntityKind> {
        self.index.get(handle).map(|entry| entry.kind)
    }
    /// Resolve the actual current generation; callers must not manufacture a
    /// handle from an entity ID or a persisted generation.
    pub fn entity_handle(&self, id: i32) -> Option<EntityHandle> {
        self.index.by_id(id).map(|entry| entry.handle)
    }
    pub fn capture_persistent_entity_order(&self) -> Result<PersistentEntityOrder> {
        let index = self.index.capture_persistent_order()?;
        let ticking = self.ticks.ordered_entries().filter_map(|(_, handle)|
            self.index.get(handle).filter(|entry| entry.kind != EntityKind::Player).map(|_| handle)).collect();
        let order = PersistentEntityOrder { index, ticking };
        let by_id = order.index.registrations.iter().map(|entry| (entry.handle.id, entry.handle)).collect::<BTreeMap<_, _>>();
        let mut seen = std::collections::BTreeSet::new();
        for handle in &order.ticking {
            ensure!(by_id.get(&handle.id) == Some(handle) && seen.insert(*handle), "invalid actual native tick-order owner");
        }
        Ok(order)
    }
    /// All source-cohort validation in validate_entries must also run against
    /// checkpoint physical payloads before registering any restored bodies.
    /// Here the complete actual fresh cohort is checked before index mutation.
    pub fn restore_persistent_entity_order(&mut self, order: &PersistentEntityOrder) -> Result<()> {
        let prepared = self.index.prepare_persistent_order(&order.index)?;
        let by_id = order.index.registrations.iter().map(|entry| (entry.handle.id, entry.handle)).collect::<BTreeMap<_, _>>();
        let mut seen = std::collections::BTreeSet::new();
        let mut ticking = Vec::with_capacity(order.ticking.len());
        for handle in &order.ticking {
            ensure!(by_id.get(&handle.id) == Some(handle) && seen.insert(*handle), "invalid saved native tick order");
            let actual = self.index.by_id(handle.id).context("actual restored tick owner missing")?;
            ticking.push(actual.handle);
        }
        ensure!(self.ticks.is_empty() && self.tracking_order.is_empty()
            && self.index.entries().all(|entry| self.index.visibility(entry.handle) == Some(Visibility::Hidden)),
            "native order restore requires fresh hidden tick/sender owners");
        self.index.apply_persistent_order(prepared)?;
        self.native_tick_resume_order = ticking;
        self.native_tick_resume_seen.clear();
        Ok(())
    }

    /// Only actual confirmed ticking callbacks activate saved members. Restore
    /// their native subsequence without moving new Players' positions or
    /// invalidating an in-progress EntityTickList copy-on-write pass. Once all
    /// saved members first activate/disappear, ordinary source ordering resumes.
    fn reconcile_native_tick_resume_order(&mut self) {
        if self.native_tick_resume_order.is_empty() { return; }
        // A captured member can be removed before its chunk ever activates.
        // Retire that source reference before applying a later activation's
        // priority; otherwise an already completed cohort could reorder again.
        for &handle in &self.native_tick_resume_order {
            if self.index.get(handle).is_none() { self.native_tick_resume_seen.insert(handle); }
        }
        if self.native_tick_resume_seen.len() == self.native_tick_resume_order.len() {
            self.native_tick_resume_order.clear();
            self.native_tick_resume_seen.clear();
            return;
        }
        let mut current = self.ticks.ordered_entries().collect::<Vec<_>>();
        let cohort = self.native_tick_resume_order.iter().copied().collect::<std::collections::BTreeSet<_>>();
        let slots = current.iter().enumerate().filter_map(|(slot, (_, handle))| cohort.contains(handle).then_some(slot)).collect::<Vec<_>>();
        let active = self.native_tick_resume_order.iter().copied()
            .filter(|handle| self.ticks.get(handle.id) == Some(*handle)).collect::<Vec<_>>();
        for (slot, handle) in slots.into_iter().zip(active) { current[slot] = (handle.id, handle); }
        let before = self.ticks.ordered_entries().collect::<Vec<_>>();
        if current != before {
            for (id, _) in &before { self.ticks.remove(*id); }
            for (id, handle) in current { self.ticks.add(id, handle); }
        }
        for &handle in &self.native_tick_resume_order {
            if self.ticks.get(handle.id) == Some(handle) || self.index.get(handle).is_none() {
                self.native_tick_resume_seen.insert(handle);
            }
        }
        if self.native_tick_resume_seen.len() == self.native_tick_resume_order.len() {
            self.native_tick_resume_order.clear();
            self.native_tick_resume_seen.clear();
        }
    }
    /// A temporary preflight copy contains index ownership facts only. The
    /// caller must discard it and commit against the actual exclusive owner.
    pub fn prepare_experience_index(&self) -> Result<RuntimeEntityIndex> {
        self.index.snapshot_for_preparation()
    }
    /// Validate the whole mixed Item/XP insertion before either owner commits.
    pub fn validate_entity_registration_batch(&self, ids: impl IntoIterator<Item = i32>) -> Result<()> {
        self.index.validate_registration_batch(ids)
    }
    /// Temporary shared index after the planned Item additions. The caller
    /// commits those exact items first, so subsequent XP gets the same section
    /// encounter order and generation sequence as this preparation.
    pub fn prepare_index_after_items(&self, entries: &[(ItemState, LegacyRandomSource)]) -> Result<RuntimeEntityIndex> {
        let mut index = self.index.snapshot_for_preparation()?;
        index.validate_registration_batch(entries.iter().map(|(state, _)| state.entity_id))?;
        for (state, _) in entries {
            state.validate()?;
            ensure!(!state.removed, "cannot stage a removed item");
            index.register(state.entity_id, EntityKind::Item, state.position_vec(), false,
                |chunk| self.experience_chunk_visibility(chunk))?;
        }
        Ok(index)
    }
    pub fn experience_chunk_visibility(&self, chunk: (i32, i32)) -> Visibility {
        self.chunk_visibility
            .get(&chunk)
            .copied()
            .unwrap_or(Visibility::Hidden)
    }
    pub fn experience_visibility_at(&self, position: Vec3) -> Visibility {
        let (x, _, z) =
            crate::entity_spatial::section_coordinates(crate::entity_spatial::section_at(position));
        self.experience_chunk_visibility((x, z))
    }
    /// XP merge/search uses the same live class index as Item/Player queries.
    pub fn query_experience_orbs(
        &mut self,
        bounds: Aabb,
        xp_bounds: impl FnMut(EntityHandle) -> Result<Option<Aabb>>,
    ) -> Result<Vec<EntityHandle>> {
        self.index
            .query(bounds, Some(EntityKind::ExperienceOrb), xp_bounds)
    }
    /// Validate an external XP owner's IDs before its inventory/RNG commit.
    /// This is a shared-index capacity check, not an identity reservation.
    pub fn validate_experience_orb_registration_batch(
        &self,
        ids: impl IntoIterator<Item = i32>,
    ) -> Result<()> {
        self.index.validate_registration_batch(ids)
    }
    /// The host keeps the physical state, RNG and replication owner. Only the
    /// immutable generation handle and actual section membership live here.
    pub fn register_experience_orb(&mut self, id: i32, position: Vec3) -> Result<EntityHandle> {
        ensure!(
            position.x.is_finite() && position.y.is_finite() && position.z.is_finite(),
            "nonfinite experience orb position"
        );
        let visibility = &self.chunk_visibility;
        let handle =
            self.index
                .register(id, EntityKind::ExperienceOrb, position, false, |chunk| {
                    visibility
                        .get(&chunk)
                        .copied()
                        .unwrap_or(Visibility::Hidden)
                })?;
        let visibility = self
            .index
            .visibility(handle)
            .context("registered experience orb section missing")?;
        if visibility.accessible() {
            self.tracking_order.insert(id, handle);
        }
        if visibility.ticking() {
            self.ticks.add(id, handle);
        }
        Ok(handle)
    }
    /// Call at the XP entity's actual setPosRaw/onMove boundary. Bounds are
    /// supplied by its physical owner to queries; no stale bounds copy exists.
    pub fn move_experience_orb(&mut self, handle: EntityHandle, position: Vec3) -> Result<()> {
        ensure!(
            position.x.is_finite() && position.y.is_finite() && position.z.is_finite(),
            "nonfinite experience orb position"
        );
        let entry = self
            .index
            .get(handle)
            .context("experience orb handle is not registered")?;
        ensure!(
            entry.kind == EntityKind::ExperienceOrb,
            "expected experience orb handle"
        );
        let visibility = &self.chunk_visibility;
        if let Some(transition) = self.index.position_changed(handle, position, |chunk| {
            visibility
                .get(&chunk)
                .copied()
                .unwrap_or(Visibility::Hidden)
        })? {
            if transition.was_ticking() != transition.is_ticking() {
                if transition.is_ticking() {
                    self.ticks.add(handle.id, handle);
                } else {
                    self.ticks.remove(handle.id);
                }
            }
            if transition.previous.accessible() != transition.current.accessible() {
                if transition.current.accessible() {
                    self.tracking_order.insert(handle.id, handle);
                } else {
                    self.tracking_order.remove(handle.id);
                }
            }
        }
        Ok(())
    }
    /// A stale generation cannot remove an entity which reused the same ID.
    /// Packet removal and XP gameplay callbacks belong to the external owner.
    pub fn remove_experience_orb(&mut self, handle: EntityHandle) -> Result<()> {
        let Some(entry) = self.index.get(handle) else {
            return Ok(());
        };
        ensure!(
            entry.kind == EntityKind::ExperienceOrb,
            "expected experience orb handle"
        );
        self.index.remove(handle);
        self.ticks.remove(handle.id);
        self.tracking_order.remove(handle.id);
        Ok(())
    }
    pub fn experience_orb_visibility(&self, handle: EntityHandle) -> Option<Visibility> {
        self.index
            .get(handle)
            .filter(|entry| entry.kind == EntityKind::ExperienceOrb)
            .and_then(|_| self.index.visibility(handle))
    }
    pub fn experience_orb_is_ticking(&self, handle: EntityHandle) -> bool {
        self.entity_kind(handle) == Some(EntityKind::ExperienceOrb)
            && self.ticks.get(handle.id) == Some(handle)
    }
    /// A falling-block owner retains its canonical state/RNG/sender separately;
    /// only the real shared section, generation, tracking and tick owners live
    /// here. Call only after the actual addFreshEntity admission accepts it.
    pub fn register_falling_block(&mut self, id: i32, position: Vec3) -> Result<EntityHandle> {
        ensure!(position.x.is_finite() && position.y.is_finite() && position.z.is_finite(),
            "nonfinite falling block position");
        let visibility = &self.chunk_visibility;
        let handle = self.index.register(id, EntityKind::FallingBlock, position, false, |chunk| {
            visibility.get(&chunk).copied().unwrap_or(Visibility::Hidden)
        })?;
        let visibility = self.index.visibility(handle).context("registered falling block section missing")?;
        if visibility.accessible() { self.tracking_order.insert(id, handle); }
        if visibility.ticking() { self.ticks.add(id, handle); }
        Ok(handle)
    }
    /// Invoke synchronously at the actual actor's setPosRaw/onMove boundary,
    /// not once after its entire movement/landing callback sequence.
    pub fn move_falling_block(&mut self, handle: EntityHandle, position: Vec3) -> Result<()> {
        ensure!(position.x.is_finite() && position.y.is_finite() && position.z.is_finite(),
            "nonfinite falling block position");
        let entry = self.index.get(handle).context("falling block handle is not registered")?;
        ensure!(entry.kind == EntityKind::FallingBlock, "expected falling block handle");
        let visibility = &self.chunk_visibility;
        if let Some(transition) = self.index.position_changed(handle, position, |chunk| {
            visibility.get(&chunk).copied().unwrap_or(Visibility::Hidden)
        })? {
            if transition.was_ticking() != transition.is_ticking() {
                if transition.is_ticking() { self.ticks.add(handle.id, handle); }
                else { self.ticks.remove(handle.id); }
            }
            if transition.previous.accessible() != transition.current.accessible() {
                if transition.current.accessible() { self.tracking_order.insert(handle.id, handle); }
                else { self.tracking_order.remove(handle.id); }
            }
        }
        Ok(())
    }
    /// The actual discard callback calls this immediately, even when source
    /// landing/drop callbacks continue to use the same now-removed object.
    pub fn remove_falling_block(&mut self, handle: EntityHandle) -> Result<()> {
        let Some(entry) = self.index.get(handle) else { return Ok(()); };
        ensure!(entry.kind == EntityKind::FallingBlock, "expected falling block handle");
        self.index.remove(handle);
        self.ticks.remove(handle.id);
        self.tracking_order.remove(handle.id);
        Ok(())
    }
    pub fn falling_block_visibility(&self, handle: EntityHandle) -> Option<Visibility> {
        self.index.get(handle).filter(|entry| entry.kind == EntityKind::FallingBlock)
            .and_then(|_| self.index.visibility(handle))
    }
    pub fn falling_block_is_ticking(&self, handle: EntityHandle) -> bool {
        self.entity_kind(handle) == Some(EntityKind::FallingBlock)
            && self.ticks.get(handle.id) == Some(handle)
    }
    /// The same insertion-ordered level list used by item/player simulation.
    /// The external XP owner must retain this sequence rather than sort by ID.
    pub fn level_tick_entries(&self) -> impl Iterator<Item = (EntityHandle, EntityKind)> + '_ {
        self.ticks
            .ordered_entries()
            .filter_map(|(_, handle)| self.index.get(handle).map(|entry| (handle, entry.kind)))
    }
    pub fn ticking_experience_orbs(&self) -> impl Iterator<Item = EntityHandle> + '_ {
        self.level_tick_entries()
            .filter_map(|(handle, kind)| (kind == EntityKind::ExperienceOrb).then_some(handle))
    }
    /// Preserve EntitySectionStorage query order for the Player-only push
    /// predicate. The caller supplies current authoritative Player AABBs.
    pub fn native_body_query(
        &mut self,
        bounds: Aabb,
        except: i32,
        mut current_bounds: impl FnMut(EntityHandle) -> Result<Option<Aabb>>,
    ) -> Result<Vec<EntityHandle>> {
        self.index
            .query(bounds, Some(EntityKind::Player), |handle| {
                if handle.id == except {
                    Ok(None)
                } else {
                    current_bounds(handle)
                }
            })
    }
    /// One real shared section query for source entity predicates. Physical
    /// bounds remain with the actual class owners supplied by the caller.
    pub fn query_native_entities(&mut self, bounds: Aabb, except: i32,
        mut inspect: impl FnMut(EntityHandle, EntityKind) -> Result<Option<Aabb>>,
    ) -> Result<Vec<EntityHandle>> {
        self.index.query_with_kind(bounds, None, |handle, kind| {
            if handle.id == except { Ok(None) } else { inspect(handle, kind) }
        })
    }
    /// Query the same section/class owner without an excluded-entity sentinel.
    /// Item bounds come from their real internal body and matching generation;
    /// external classes retain their actual host-owned body lookup. Trigger,
    /// spectator, health or other gameplay predicates belong to the caller.
    pub fn query_native_entities_with_items(
        &mut self,
        bounds: Aabb,
        kind: Option<EntityKind>,
        mut external: impl FnMut(EntityHandle, EntityKind) -> Result<Option<Aabb>>,
    ) -> Result<Vec<EntityHandle>> {
        let items = &self.items;
        self.index.query_with_kind(bounds, kind, |handle, kind| {
            if kind == EntityKind::Item {
                let item = items.get(&handle.id).context("indexed query Item body is unavailable")?;
                ensure!(item.handle == handle, "indexed query Item generation changed");
                Ok((!item.state.removed).then_some(item.state.bounds))
            } else {
                external(handle, kind)
            }
        })
    }
    /// The current level tick list, including stale-but-still-scheduled items.
    /// Tracking visibility and published snapshots are not a simulation gate.
    pub fn ticking_item_states(&self) -> impl Iterator<Item = &ItemState> {
        self.ticks.ordered_entries().filter_map(|(_, handle)| {
            self.items
                .get(&handle.id)
                .filter(|entry| {
                    entry.handle == handle
                        && !entry.state.removed
                        && self.index.get(handle).is_some()
                })
                .map(|entry| &entry.state)
        })
    }
    pub fn item_states(&self) -> impl Iterator<Item = &ItemState> {
        self.items.values().map(|entry| &entry.state)
    }
    /// Native persistence sees the actual retained RNG, without drawing or
    /// reconstructing a stream from the entity's original constructor seed.
    pub fn persistent_item_states(
        &self,
    ) -> impl Iterator<Item = (&ItemState, &LegacyRandomSource)> {
        self.items
            .values()
            .map(|entry| (&entry.state, &entry.random))
    }
    pub fn persistent_item_tick_order(&self) -> Vec<i32> {
        self.ticks
            .ordered_entries()
            .filter_map(|(id, handle)| {
                self.items
                    .get(&id)
                    .filter(|entry| entry.handle == handle)
                    .map(|_| id)
            })
            .collect()
    }
    pub fn world_random_state48(&self) -> u64 {
        self.world_random.state48()
    }
    pub fn highest_registered_entity_id(&self) -> Option<i32> {
        self.index.entries().map(|entry| entry.handle.id).max()
    }
    fn prepare_item(&self, state: ItemState, random: LegacyRandomSource) -> Result<PreparedItem> {
        state.validate()?;
        ensure!(!state.removed, "cannot register a removed item");
        let sender = ItemReplication::new(&state)?;
        // Hidden entities satisfy the same sender contract before a later
        // visibility transition can create their actual live tracker.
        let snapshot = sender.snapshot(&state)?;
        state.spawn_packets()?;
        let section = crate::entity_spatial::section_coordinates(
            crate::entity_spatial::section_at(state.position_vec()),
        );
        let accessible = self.chunk_visibility.get(&(section.0, section.2))
            .copied().unwrap_or(Visibility::Hidden).accessible();
        let tracking = if accessible { Some((sender, snapshot)) } else { None };
        Ok(PreparedItem { state, random, tracking })
    }

    fn insert_prepared_item(&mut self, item: PreparedItem) -> EntityHandle {
        let PreparedItem {
            state,
            random,
            tracking,
        } = item;
        let visibility = &self.chunk_visibility;
        let handle = self
            .index
            .register(
                state.entity_id,
                EntityKind::Item,
                state.position_vec(),
                false,
                |chunk| {
                    visibility
                        .get(&chunk)
                        .copied()
                        .unwrap_or(Visibility::Hidden)
                },
            )
            .expect("exclusive prepared insertion preserves validated identities and capacity");
        self.items.insert(
            handle.id,
            RuntimeItem {
                handle,
                state,
                random,
            },
        );
        if let Some((sender, snapshot)) = tracking {
            self.tracking.insert(
                handle.id,
                TrackedItem {
                    handle,
                    sender,
                    last_section: crate::entity_spatial::section_at(
                        snapshot.entity.position_vec(),
                    ),
                },
            );
            self.tracking_order.insert(handle.id, handle);
            let revision = next_revision(&mut self.outbox.revision)
                .expect("prepared tracking publication capacity");
            self.outbox.send(RuntimeEntityEvent::Update {
                revision,
                snapshot: Box::new(snapshot),
                packets: vec![],
            });
        }
        // onEntityAdded starts tracking before ticking, matching onMove.
        if self
            .index
            .visibility(handle)
            .is_some_and(Visibility::ticking)
        {
            self.ticks.add(handle.id, handle);
        }
        handle
    }

    pub fn prepare_items(
        &mut self,
        entries: Vec<(ItemState, LegacyRandomSource)>,
    ) -> Result<PreparedItemBatch<'_>> {
        self.index
            .validate_registration_batch(entries.iter().map(|(state, _)| state.entity_id))?;
        let mut items = Vec::with_capacity(entries.len());
        let mut publications = 0_u64;
        for (state, random) in entries {
            let item = self.prepare_item(state, random)?;
            if item.tracking.is_some() {
                publications += 1;
            }
            items.push(item);
        }
        self.outbox
            .revision
            .checked_add(publications)
            .context("item revision exhausted")?;
        Ok(PreparedItemBatch { store: self, items })
    }

    /// Preflight the whole original mixed insertion sequence before any menu,
    /// RNG, shared index or sender state changes. The actual exclusive borrow
    /// prevents intervening registrations or visibility changes until commit.
    /// XP's physical/sender owner must retain the exact state used to create
    /// its registration and preflight all corresponding merge/packet phases.
    pub fn prepare_native_entities(
        &mut self,
        additions: Vec<NativeEntityAddition>,
    ) -> Result<PreparedNativeEntityBatch<'_>> {
        self.index.validate_registration_batch(additions.iter().map(|entry| match entry {
            NativeEntityAddition::Item(state, _) => state.entity_id,
            NativeEntityAddition::ExperienceOrb(registration) => registration.entity_id,
        }))?;
        let mut entries = Vec::with_capacity(additions.len());
        let mut publications = 0_u64;
        for addition in additions {
            entries.push(match addition {
                NativeEntityAddition::Item(state, random) => {
                    let item = self.prepare_item(state, random)?;
                    if item.tracking.is_some() { publications += 1; }
                    PreparedNativeEntity::Item(item)
                }
                NativeEntityAddition::ExperienceOrb(registration) => {
                    ensure!(registration.position.x.is_finite()
                        && registration.position.y.is_finite()
                        && registration.position.z.is_finite(),
                        "nonfinite prepared experience orb position");
                    PreparedNativeEntity::ExperienceOrb(registration)
                }
            });
        }
        self.outbox.revision.checked_add(publications)
            .context("mixed item revision exhausted")?;
        Ok(PreparedNativeEntityBatch { store: self, entries })
    }
    pub fn revision(&self) -> u64 {
        self.outbox.revision
    }
    pub fn drain_events(&mut self) -> Vec<RuntimeEntityEvent> {
        std::mem::take(&mut self.outbox.events)
    }
    pub fn snapshots(&self) -> Result<(u64, Vec<EntitySnapshot<ItemState>>)> {
        Ok((
            self.outbox.revision,
            self.tracking_order
                .values()
                .filter_map(|handle| self.tracking.get(&handle.id))
                .filter_map(|tracked| {
                    self.items
                        .get(&tracked.handle.id)
                        .filter(|entry| entry.handle == tracked.handle && !entry.state.removed)
                        .map(|entry| (tracked, entry))
                })
                .map(|(tracked, entry)| tracked.sender.snapshot(&entry.state))
                .collect::<Result<_>>()?,
        ))
    }
    pub fn register_player(&mut self, id: i32, position: Vec3) -> Result<EntityHandle> {
        let visibility = &self.chunk_visibility;
        let handle = self
            .index
            .register(id, EntityKind::Player, position, true, |chunk| {
                visibility
                    .get(&chunk)
                    .copied()
                    .unwrap_or(Visibility::Hidden)
            })?;
        self.ticks.add(id, handle);
        // ChunkMap's map includes players. Their hash slots affect the order
        // of item senders even when this layer delegates player publication.
        self.tracking_order.insert(id, handle);
        Ok(handle)
    }
    pub fn remove_player(&mut self, id: i32) -> Result<()> {
        if let Some(entry) = self.index.by_id(id) {
            ensure!(
                entry.kind == EntityKind::Player,
                "expected registered player"
            );
            self.index.remove(entry.handle);
            self.ticks.remove(id);
            self.tracking_order.remove(id);
        }
        Ok(())
    }
    pub fn move_player(&mut self, id: i32, position: Vec3) -> Result<()> {
        if let Some(entry) = self.index.by_id(id) {
            ensure!(
                entry.kind == EntityKind::Player,
                "expected registered player"
            );
            self.index
                .position_changed(entry.handle, position, |chunk| {
                    self.chunk_visibility
                        .get(&chunk)
                        .copied()
                        .unwrap_or(Visibility::Hidden)
                })?;
        }
        Ok(())
    }
    /// Ticket manager input. Loading a network chunk alone must not imply this.
    pub fn set_chunk_visibility(
        &mut self,
        chunk: (i32, i32),
        visibility: Visibility,
    ) -> Result<()> {
        self.chunk_visibility.insert(chunk, visibility);
        for transition in self.index.set_chunk_visibility(chunk, visibility)? {
            if transition.was_ticking() != transition.is_ticking() {
                if transition.is_ticking() {
                    self.ticks
                        .add(transition.entity.handle.id, transition.entity.handle);
                } else {
                    self.ticks.remove(transition.entity.handle.id);
                }
            }
            if transition.entity.kind == EntityKind::Item {
                let handle = transition.entity.handle;
                let state = &self
                    .items
                    .get(&handle.id)
                    .context("indexed item missing")?
                    .state;
                change_tracking(
                    &mut self.tracking,
                    &mut self.tracking_order,
                    &mut self.outbox,
                    handle,
                    state,
                    (transition.previous, transition.current),
                )?;
            } else if matches!(transition.entity.kind, EntityKind::ExperienceOrb | EntityKind::FallingBlock)
                && transition.previous.accessible() != transition.current.accessible()
            {
                let handle = transition.entity.handle;
                if transition.current.accessible() {
                    self.tracking_order.insert(handle.id, handle);
                } else {
                    self.tracking_order.remove(handle.id);
                }
            }
        }
        self.reconcile_native_tick_resume_order();
        Ok(())
    }
    pub fn insert_item(
        &mut self,
        state: ItemState,
        random: LegacyRandomSource,
    ) -> Result<EntityHandle> {
        Ok(self
            .prepare_items(vec![(state, random)])?
            .commit()
            .pop()
            .expect("one prepared item"))
    }
    /// ChunkMap's shared sender phase, before level entity simulation.
    pub fn publish_tracking(
        &mut self,
        in_entity_ticking_range: impl FnMut((i32, i32)) -> bool,
    ) -> Result<()> {
        self.publish_tracking_with_players(in_entity_ticking_range, |_| {
            anyhow::bail!("registered players require their shared tracking callback")
        })
    }
    pub fn publish_tracking_with_players(
        &mut self,
        in_entity_ticking_range: impl FnMut((i32, i32)) -> bool,
        mut publish_player: impl FnMut(EntityHandle) -> Result<()>,
    ) -> Result<()> {
        self.publish_tracking_with_external(in_entity_ticking_range, |kind, handle| {
            ensure!(
                kind == EntityKind::Player,
                "experience orb sender owner must use publish_tracking_with_external"
            );
            publish_player(handle)
        })
    }
    /// Traverse the actual shared ChunkMap sender order. The callback handles
    /// externally owned Players and ExperienceOrbs at their original positions
    /// among Item senders; it must not re-enter this RuntimeEntities borrow.
    pub fn publish_tracking_with_external(
        &mut self,
        mut in_entity_ticking_range: impl FnMut((i32, i32)) -> bool,
        mut publish_external: impl FnMut(EntityKind, EntityHandle) -> Result<()>,
    ) -> Result<()> {
        for handle in self.tracking_order.values() {
            let Some(tracked) = self.tracking.get_mut(&handle.id) else {
                let entry = self
                    .index
                    .get(handle)
                    .context("external tracker handle is stale")?;
                ensure!(
                    entry.kind != EntityKind::Item,
                    "indexed item tracker is missing"
                );
                publish_external(entry.kind, handle)?;
                continue;
            };
            ensure!(tracked.handle == handle, "stale tracked item handle");
            let entry = self
                .items
                .get_mut(&tracked.handle.id)
                .context("tracked item missing")?;
            if entry.state.removed {
                continue;
            }
            let section = crate::entity_spatial::section_at(entry.state.position_vec());
            let (x, _, z) = crate::entity_spatial::section_coordinates(section);
            if section == tracked.last_section && !in_entity_ticking_range((x, z)) {
                continue;
            }
            let publication = tracked.sender.send_changes(&mut entry.state)?;
            tracked.last_section = section;
            let revision = next_revision(&mut self.outbox.revision)?;
            self.outbox.send(RuntimeEntityEvent::Update {
                revision,
                snapshot: Box::new(publication.snapshot),
                packets: publication.packets,
            });
        }
        Ok(())
    }
    pub fn simulate_level(
        &mut self,
        world: &mut RuntimeWorld,
        facts: ItemWorldFacts,
        colliders: &[CollisionEntity],
        host: &mut impl RuntimeEntityHost,
    ) -> Result<()> {
        self.simulate_level_with_context(world, facts, colliders, None, None, host)
    }
    /// Reuse the server-wide observed history across all items and dimensions.
    pub fn simulate_level_with_fluid_cache(
        &mut self,
        world: &mut RuntimeWorld,
        facts: ItemWorldFacts,
        colliders: &[CollisionEntity],
        cache: &mut FluidShapeCache,
        host: &mut impl RuntimeEntityHost,
    ) -> Result<()> {
        self.simulate_level_with_context(world, facts, colliders, Some(cache), None, host)
    }
    /// Share the actual fluid-shape history and source-validated rain exposure
    /// across every item in this level tick. Neither capability is fabricated.
    pub fn simulate_level_with_context(
        &mut self,
        world: &mut RuntimeWorld,
        facts: ItemWorldFacts,
        colliders: &[CollisionEntity],
        cache: Option<&mut FluidShapeCache>,
        rain: Option<&mut dyn RainExposure>,
        host: &mut impl RuntimeEntityHost,
    ) -> Result<()> {
        self.simulate_level_inner(world, facts, colliders, cache, rain, host)
    }
    fn simulate_level_inner(
        &mut self,
        world: &mut RuntimeWorld,
        facts: ItemWorldFacts,
        colliders: &[CollisionEntity],
        mut cache: Option<&mut FluidShapeCache>,
        mut rain: Option<&mut dyn RainExposure>,
        host: &mut impl RuntimeEntityHost,
    ) -> Result<()> {
        let Self {
            index,
            ticks,
            items,
            tracking,
            tracking_order,
            chunk_visibility,
            world_random: _,
            sound_random,
            outbox,
            ..
        } = self;
        ticks
            .try_for_each(|ticks, handle| {
                let rain = rain
                    .as_mut()
                    .map(|exposure| &mut **exposure as &mut dyn RainExposure);
                simulate_level_entry_inner(
                    TickParts {
                        index: &mut *index,
                        ticks,
                        items: &mut *items,
                        tracking: &mut *tracking,
                        tracking_order: &mut *tracking_order,
                        chunk_visibility: &*chunk_visibility,
                        sound_random: &mut *sound_random,
                        outbox: &mut *outbox,
                        host: &mut *host,
                    },
                    handle,
                    world,
                    facts,
                    colliders,
                    cache.as_deref_mut(),
                    rain,
                )
            })
            .map_err(|error| anyhow::anyhow!("entity level tick: {error}"))
    }

    /// Begin the actual shared Item/Player/XP EntityTickList pass. The marker
    /// preserves copy-on-write and nested-iteration rejection across separately
    /// borrowed native body hosts. End this pass on error and unwind as well.
    pub fn begin_level_tick_pass(&mut self) -> Result<Vec<EntityHandle>> {
        self.ticks.begin_snapshot_pass().map_err(Into::into)
    }
    pub fn finish_level_tick_pass(&mut self) {
        self.ticks.finish_snapshot_pass();
    }
    /// Run exactly the original body for a captured pass entry. Current tick
    /// membership is deliberately not rechecked: a surviving generation removed
    /// from ticking during the pass still belongs to its captured iteration.
    pub fn simulate_level_entry_with_context(
        &mut self,
        handle: EntityHandle,
        world: &mut RuntimeWorld,
        facts: ItemWorldFacts,
        colliders: &[CollisionEntity],
        cache: Option<&mut FluidShapeCache>,
        rain: Option<&mut dyn RainExposure>,
        host: &mut impl RuntimeEntityHost,
    ) -> Result<()> {
        ensure!(
            self.ticks.is_iterating(),
            "single entity tick requires the actual shared level pass"
        );
        let Self {
            index,
            ticks,
            items,
            tracking,
            tracking_order,
            chunk_visibility,
            world_random: _,
            sound_random,
            outbox,
            ..
        } = self;
        simulate_level_entry_inner(
            TickParts {
                index,
                ticks,
                items,
                tracking,
                tracking_order,
                chunk_visibility,
                sound_random,
                outbox,
                host,
            },
            handle,
            world,
            facts,
            colliders,
            cache,
            rain,
        )
        .map_err(|error| anyhow::anyhow!("entity level tick: {error}"))
    }
    /// Run for each actual connection-phase Player.aiStep, after world ticks.
    /// The caller preserves its real connection iteration order.
    pub fn player_touch(
        &mut self,
        player: PickupPlayer,
        host: &mut impl RuntimeEntityHost,
    ) -> Result<()> {
        if !player.alive || player.spectator {
            return Ok(());
        }
        let bounds = if let Some(vehicle) = player.vehicle_bounds {
            player
                .bounds
                .minmax(vehicle)
                .inflate(Vec3::new(1.0, 0.0, 1.0))
        } else {
            player.bounds.inflate(Vec3::new(1.0, 0.5, 1.0))
        };
        let items = &self.items;
        let candidates = self.index.query(bounds, Some(EntityKind::Item), |handle| {
            Ok(items
                .get(&handle.id)
                .filter(|entry| entry.handle == handle && !entry.state.removed)
                .map(|entry| entry.state.bounds))
        })?;
        for handle in candidates {
            self.player_touch_item(handle, player, host)?;
        }
        Ok(())
    }
    /// Player.aiStep's actual Item/XP search in section and insertion order.
    /// The caller supplies XP AABBs directly from its authoritative store and
    /// dispatches each returned handle synchronously before taking the next.
    pub fn native_pickup_candidates(
        &mut self,
        player: PickupPlayer,
        mut xp_bounds: impl FnMut(EntityHandle) -> Result<Option<Aabb>>,
    ) -> Result<Vec<EntityHandle>> {
        if !player.alive || player.spectator {
            return Ok(Vec::new());
        }
        let bounds = pickup_search_bounds(player);
        let items = &self.items;
        self.index
            .query_with_kind(bounds, None, |handle, kind| match kind {
                // FallingBlockEntity inherits the no-op playerTouch; it is
                // never represented as an Item or awarded as an XP pickup.
                EntityKind::Player | EntityKind::FallingBlock => Ok(None),
                EntityKind::Item => Ok(items
                    .get(&handle.id)
                    .filter(|entry| entry.handle == handle && !entry.state.removed)
                    .map(|entry| entry.state.bounds)),
                EntityKind::ExperienceOrb => xp_bounds(handle),
            })
    }
    /// The existing ItemEntity.playerTouch callback for one handle from the
    /// mixed search. This preserves inventory/collection/discard ordering.
    pub fn player_touch_item(
        &mut self,
        handle: EntityHandle,
        player: PickupPlayer,
        host: &mut impl RuntimeEntityHost,
    ) -> Result<()> {
        if !player.alive || player.spectator {
            return Ok(());
        }
        let Some(indexed) = self.index.get(handle) else {
            return Ok(());
        };
        ensure!(
            indexed.kind == EntityKind::Item,
            "expected item pickup handle"
        );
        let Some(entry) = self.items.get_mut(&handle.id) else {
            return Ok(());
        };
        if entry.handle != handle || entry.state.removed {
            return Ok(());
        }
        let mut callbacks = PickupCallbacks {
            player,
            handle,
            index: &mut self.index,
            ticks: &mut self.ticks,
            tracking: &mut self.tracking,
            tracking_order: &mut self.tracking_order,
            outbox: &mut self.outbox,
            host,
        };
        let result =
            entry
                .state
                .try_pickup_with_callbacks(player.uuid, player.entity_id, &mut callbacks);
        if entry.state.removed {
            self.items.remove(&handle.id);
        }
        result?;
        Ok(())
    }
}
fn pickup_search_bounds(player: PickupPlayer) -> Aabb {
    if let Some(vehicle) = player.vehicle_bounds {
        player
            .bounds
            .minmax(vehicle)
            .inflate(Vec3::new(1.0, 0.0, 1.0))
    } else {
        player.bounds.inflate(Vec3::new(1.0, 0.5, 1.0))
    }
}
struct PickupCallbacks<'a, H> {
    player: PickupPlayer,
    handle: EntityHandle,
    index: &'a mut RuntimeEntityIndex,
    ticks: &'a mut EntityTickList<EntityHandle>,
    tracking: &'a mut BTreeMap<i32, TrackedItem>,
    tracking_order: &'a mut TrackingEntityMap<EntityHandle>,
    outbox: &'a mut EntityOutbox,
    host: &'a mut H,
}
impl<H: RuntimeEntityHost> ItemPickupHost for PickupCallbacks<'_, H> {
    fn is_client_side(&self) -> bool {
        false
    }
    fn add_to_inventory(&mut self, stack: &mut ItemStack) -> Result<bool> {
        self.host
            .add_to_inventory(self.player, self.handle.id, stack.count, stack)
    }
    fn collect(
        &mut self,
        state: &ItemState,
        original_count: i32,
        accepted_count: i32,
    ) -> Result<()> {
        let pickup = Pickup {
            item_entity_id: state.entity_id,
            player_entity_id: self.player.entity_id,
            packet_count: original_count,
            accepted_count,
            removed: state.removed,
        };
        let revision = next_revision(&mut self.outbox.revision)?;
        self.outbox
            .send(RuntimeEntityEvent::Collected { revision, pickup });
        self.host
            .collect_item(self.player, state, original_count, accepted_count)
    }
    fn discarded(&mut self, state: &ItemState) -> Result<()> {
        self.index.remove(self.handle);
        self.ticks.remove(self.handle.id);
        if self.tracking.remove(&self.handle.id).is_some() {
            self.tracking_order.remove(self.handle.id);
            let revision = next_revision(&mut self.outbox.revision)?;
            self.outbox.send(RuntimeEntityEvent::Removed {
                revision,
                entity_id: self.handle.id,
            });
        }
        self.host
            .discarded(&state.physical, &state.stack, state.health)
    }
    fn after(&mut self, state: &ItemState, pickup: Pickup, original_item_id: i32) -> Result<()> {
        self.host
            .collected(self.player, state, pickup, original_item_id)
    }
}

fn next_revision(revision: &mut u64) -> Result<u64> {
    *revision = revision.checked_add(1).context("item revision exhausted")?;
    Ok(*revision)
}
fn start_tracking(
    tracking: &mut BTreeMap<i32, TrackedItem>,
    tracking_order: &mut TrackingEntityMap<EntityHandle>,
    outbox: &mut EntityOutbox,
    handle: EntityHandle,
    state: &ItemState,
) -> Result<()> {
    ensure!(
        !tracking.contains_key(&handle.id),
        "item is already tracked"
    );
    let sender = ItemReplication::new(state)?;
    let snapshot = sender.snapshot(state)?;
    tracking.insert(
        handle.id,
        TrackedItem {
            handle,
            sender,
            last_section: crate::entity_spatial::section_at(state.position_vec()),
        },
    );
    tracking_order.insert(handle.id, handle);
    let revision = next_revision(&mut outbox.revision)?;
    outbox.send(RuntimeEntityEvent::Update {
        revision,
        snapshot: Box::new(snapshot),
        packets: vec![],
    });
    Ok(())
}
fn change_tracking(
    tracking: &mut BTreeMap<i32, TrackedItem>,
    tracking_order: &mut TrackingEntityMap<EntityHandle>,
    outbox: &mut EntityOutbox,
    handle: EntityHandle,
    state: &ItemState,
    visibility: (Visibility, Visibility),
) -> Result<()> {
    let (previous, current) = visibility;
    if previous.accessible() == current.accessible() {
        return Ok(());
    }
    if current.accessible() {
        start_tracking(tracking, tracking_order, outbox, handle, state)?;
    } else if tracking.remove(&handle.id).is_some() {
        tracking_order.remove(handle.id);
        let revision = next_revision(&mut outbox.revision)?;
        outbox.send(RuntimeEntityEvent::Removed {
            revision,
            entity_id: handle.id,
        });
    }
    Ok(())
}

fn simulate_level_entry_inner<H: RuntimeEntityHost>(
    mut parts: TickParts<'_, H>,
    handle: EntityHandle,
    world: &mut RuntimeWorld,
    facts: ItemWorldFacts,
    colliders: &[CollisionEntity],
    cache: Option<&mut FluidShapeCache>,
    rain: Option<&mut dyn RainExposure>,
) -> Result<()> {
    let Some(indexed) = parts.index.get(handle) else {
        return Ok(());
    };
    match indexed.kind {
        EntityKind::Player => return parts.host.level_player_tick(handle),
        // An external owner must dispatch this entry in the same pass. The
        // item adapter never fabricates an ItemState for an experience orb.
        EntityKind::ExperienceOrb | EntityKind::FallingBlock => return Ok(()),
        EntityKind::Item => {}
    }
    let Some(mut current) = parts.items.remove(&handle.id) else {
        return Ok(());
    };
    ensure!(current.handle == handle, "stale item tick handle");
    if current.state.removed {
        parts.unregister(handle)?;
        return Ok(());
    }
    current.state.begin_level_tick();
    // onMove overlays current physical fields on this pre-movement template.
    let tracking_context = RefCell::new(current.state.clone());
    let parts = RefCell::new(parts);
    let mut callbacks = ItemCallbacks {
        parts: &parts,
        handle,
        tracking_context: &tracking_context,
    };
    let outcome = (|| {
        let mut provider = RuntimeItemWorld::new(
            world,
            &current.state.physical,
            &mut current.random,
            &mut callbacks,
            facts,
            colliders,
        )?;
        if let Some(cache) = cache {
            provider = provider.with_fluid_shape_cache(cache);
        }
        if let Some(rain) = rain {
            provider = provider.with_rain_exposure(rain);
        }
        current
            .state
            .tick_full_body(&mut provider, |state| parts.borrow_mut().merge(state))?;
        Ok::<(), anyhow::Error>(())
    })();
    {
        let mut parts = parts.borrow_mut();
        parts
            .host
            .observe_item_tick(&current.state, &current.random);
        // Retain completed synchronous effects and actual post-error state.
        if current.state.removed {
            parts.remove_item(handle, &current.state)?;
        } else {
            parts.items.insert(handle.id, current);
        }
    }
    outcome
}

struct TickParts<'a, H> {
    index: &'a mut RuntimeEntityIndex,
    ticks: &'a mut EntityTickList<EntityHandle>,
    items: &'a mut BTreeMap<i32, RuntimeItem>,
    tracking: &'a mut BTreeMap<i32, TrackedItem>,
    tracking_order: &'a mut TrackingEntityMap<EntityHandle>,
    chunk_visibility: &'a BTreeMap<(i32, i32), Visibility>,
    sound_random: &'a mut Option<LegacyRandomSource>,
    outbox: &'a mut EntityOutbox,
    host: &'a mut H,
}
impl<H: RuntimeEntityHost> TickParts<'_, H> {
    fn unregister(&mut self, handle: EntityHandle) -> Result<bool> {
        if !self.index.remove(handle) {
            return Ok(false);
        }
        self.ticks.remove(handle.id);
        if self.tracking.remove(&handle.id).is_some() {
            self.tracking_order.remove(handle.id);
            let revision = next_revision(&mut self.outbox.revision)?;
            self.outbox.send(RuntimeEntityEvent::Removed {
                revision,
                entity_id: handle.id,
            });
        }
        Ok(true)
    }
    fn remove_item(&mut self, handle: EntityHandle, state: &ItemState) -> Result<()> {
        if self.unregister(handle)? {
            self.host
                .discarded(&state.physical, &state.stack, state.health)?;
        }
        Ok(())
    }
    fn merge(&mut self, current: &mut ItemState) -> Result<()> {
        let items = &self.items;
        let bounds = current.bounds.inflate(Vec3::new(0.5, 0.0, 0.5));
        let candidates = self.index.query(bounds, Some(EntityKind::Item), |handle| {
            if handle.id == current.entity_id {
                return Ok(None);
            }
            let Some(entry) = items.get(&handle.id).filter(|entry| entry.handle == handle) else {
                return Ok(None);
            };
            Ok(entry.state.is_mergeable()?.then_some(entry.state.bounds))
        })?;
        for handle in candidates {
            if current.removed {
                break;
            }
            let Some(mut other) = self.items.remove(&handle.id) else {
                continue;
            };
            let result = item_entity::merge_pair(current, &mut other.state);
            if other.state.removed {
                self.remove_item(handle, &other.state)?;
            } else {
                self.items.insert(handle.id, other);
            }
            result?;
            if current.removed {
                let current_handle = self
                    .index
                    .by_id(current.entity_id)
                    .context("merge source is not indexed")?
                    .handle;
                self.remove_item(current_handle, current)?;
            }
        }
        Ok(())
    }
}
struct ItemCallbacks<'a, 'b, H> {
    parts: &'a RefCell<TickParts<'b, H>>,
    handle: EntityHandle,
    tracking_context: &'a RefCell<ItemState>,
}
impl<H: RuntimeEntityHost> RuntimeItemHost for ItemCallbacks<'_, '_, H> {
    fn position_changed(&mut self, state: &EntityPhysicalState) -> Result<()> {
        let mut parts = self.parts.borrow_mut();
        let visibility = parts.chunk_visibility;
        if let Some(transition) =
            parts
                .index
                .position_changed(self.handle, state.position_vec(), |chunk| {
                    visibility
                        .get(&chunk)
                        .copied()
                        .unwrap_or(Visibility::Hidden)
                })?
        {
            // onMove updates tracking before ticking. The chunk-status path
            // deliberately uses the reverse ordering above.
            if transition.previous.accessible() != transition.current.accessible() {
                let mut context = self.tracking_context.borrow().clone();
                context.physical = state.clone();
                let TickParts {
                    tracking,
                    tracking_order,
                    outbox,
                    ..
                } = &mut *parts;
                change_tracking(
                    tracking,
                    tracking_order,
                    outbox,
                    self.handle,
                    &context,
                    (transition.previous, transition.current),
                )?;
            }
            if transition.was_ticking() != transition.is_ticking() {
                if transition.is_ticking() {
                    parts.ticks.add(self.handle.id, self.handle);
                } else {
                    parts.ticks.remove(self.handle.id);
                }
            }
        }
        Ok(())
    }
    fn profile(&mut self, event: ProfileEvent) {
        self.parts.borrow_mut().host.profile(event);
    }
    fn effect(&mut self, state: &EntityPhysicalState, effect: RuntimeItemEffect) -> Result<()> {
        let mut parts = self.parts.borrow_mut();
        if matches!(
            effect,
            RuntimeItemEffect::Base(BaseEffect::DiscardBelowWorld)
        ) && parts.unregister(self.handle)?
        {
            let context = self.tracking_context.borrow();
            parts
                .host
                .discarded(state, &context.stack, context.health)?;
        }
        let seed = if matches!(effect, RuntimeItemEffect::Sound { .. }) {
            Some(
                parts
                    .sound_random
                    .as_mut()
                    .context("actual Level.threadSafeRandom sound owner unavailable")?
                    .next_i64(),
            )
        } else {
            None
        };
        let deliveries = parts.host.effect(state, effect, seed)?;
        if !deliveries.is_empty() {
            let revision = next_revision(&mut parts.outbox.revision)?;
            parts.outbox.send(RuntimeEntityEvent::Effect {
                revision,
                deliveries,
            });
        }
        Ok(())
    }
    fn entity_damage(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
        causing_entity: Option<i32>,
    ) -> Result<()> {
        {
            let mut context = self.tracking_context.borrow_mut();
            context.stack = stack.clone();
            context.health = health;
        }
        self.parts
            .borrow_mut()
            .host
            .entity_damage(state, stack, health, causing_entity)
    }
    fn discarded(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
    ) -> Result<()> {
        let mut parts = self.parts.borrow_mut();
        if parts.unregister(self.handle)? {
            parts.host.discarded(state, stack, health)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tools/runtime_entities_tests.rs"]
mod original_corpus_tests;

#[cfg(test)]
#[path = "../tools/runtime_entities_rain_tests.rs"]
mod rain_context_tests;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::{
        entity::Position, runtime_collision::CollisionBorder, runtime_item_world::RainAvailability,
        world::FlatWorld,
    };
    struct Host {
        visits: Vec<i32>,
        removed: Vec<i32>,
    }
    impl RuntimeEntityHost for Host {
        fn observe_item_tick(&mut self, state: &ItemState, _: &LegacyRandomSource) {
            self.visits.push(state.entity_id);
        }
        fn level_player_tick(&mut self, player: EntityHandle) -> Result<()> {
            self.visits.push(player.id);
            Ok(())
        }
        fn profile(&mut self, _: ProfileEvent) {}
        fn effect(
            &mut self,
            _: &EntityPhysicalState,
            effect: RuntimeItemEffect,
            _: Option<i64>,
        ) -> Result<Vec<EffectDelivery>> {
            ensure!(
                matches!(effect, RuntimeItemEffect::HitGround { .. }),
                "unexpected test-world effect"
            );
            Ok(vec![])
        }
        fn entity_damage(
            &mut self,
            _: &EntityPhysicalState,
            _: &ItemStack,
            _: i32,
            _: Option<i32>,
        ) -> Result<()> {
            anyhow::bail!("unexpected damage")
        }
        fn discarded(&mut self, state: &EntityPhysicalState, _: &ItemStack, _: i32) -> Result<()> {
            assert!(state.removed);
            self.removed.push(state.entity_id);
            Ok(())
        }
        fn add_to_inventory(
            &mut self,
            _: PickupPlayer,
            _: i32,
            _: i32,
            _: &mut ItemStack,
        ) -> Result<bool> {
            anyhow::bail!("unexpected pickup")
        }
        fn collect_item(&mut self, _: PickupPlayer, _: &ItemState, _: i32, _: i32) -> Result<()> {
            anyhow::bail!("unexpected pickup")
        }
        fn collected(&mut self, _: PickupPlayer, _: &ItemState, _: Pickup, _: i32) -> Result<()> {
            anyhow::bail!("unexpected pickup")
        }
    }
    fn item(id: i32, count: i32) -> ItemState {
        ItemState::new(
            id,
            [id as u8; 16],
            Position {
                x: 8.5,
                y: 64.,
                z: 8.5,
                yaw: 0.,
                pitch: 0.,
                on_ground: true,
            },
            Vec3::ZERO,
            ItemStack::new(1, count).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn tracker_lifecycle_and_distance_gate_preserve_dirty_state_and_reset_only_global_tracking() {
        let mut runtime = RuntimeEntities::new(LegacyRandomSource::new(90));
        runtime
            .insert_item(item(17, 1), LegacyRandomSource::new(91))
            .unwrap();
        assert!(runtime.snapshots().unwrap().1.is_empty());
        assert!(runtime.drain_events().is_empty());
        runtime
            .set_chunk_visibility((0, 0), Visibility::Tracked)
            .unwrap();
        runtime.publish_tracking(|_| false).unwrap();
        assert_eq!(runtime.tracking[&17].sender.tracking_tick_count(), 0);
        assert_ne!(runtime.item(17).unwrap().metadata_dirty, 0);
        runtime.publish_tracking(|_| true).unwrap();
        assert_eq!(runtime.tracking[&17].sender.tracking_tick_count(), 1);
        assert_eq!(runtime.item(17).unwrap().metadata_dirty, 0);
        runtime
            .set_chunk_visibility((0, 0), Visibility::Ticking)
            .unwrap();
        assert_eq!(runtime.tracking[&17].sender.tracking_tick_count(), 1);
        runtime
            .set_chunk_visibility((0, 0), Visibility::Hidden)
            .unwrap();
        assert!(runtime.snapshots().unwrap().1.is_empty());
        runtime
            .items
            .get_mut(&17)
            .unwrap()
            .state
            .set_stack_count_in_place(5);
        runtime
            .set_chunk_visibility((0, 0), Visibility::Tracked)
            .unwrap();
        let snapshot = runtime.snapshots().unwrap().1.pop().unwrap();
        assert_eq!(snapshot.entity.stack.count, 5);
        assert_eq!(runtime.tracking[&17].sender.tracking_tick_count(), 0);
    }
    #[test]
    fn merge_removal_mutates_the_actual_tick_list_without_reordering_external_players() {
        let mut runtime = RuntimeEntities::new(LegacyRandomSource::new(92));
        runtime
            .set_chunk_visibility((0, 0), Visibility::Ticking)
            .unwrap();
        let mut first = item(17, 4);
        first.tick_count = 39;
        runtime
            .insert_item(first, LegacyRandomSource::new(93))
            .unwrap();
        runtime
            .register_player(99, Vec3::new(8.5, 64., 8.5))
            .unwrap();
        let mut source = item(3, 1);
        source.tick_count = 39;
        runtime
            .insert_item(source, LegacyRandomSource::new(94))
            .unwrap();
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        let mut host = Host {
            visits: vec![],
            removed: vec![],
        };
        runtime
            .simulate_level(
                &mut world,
                ItemWorldFacts {
                    game_time: 1,
                    minimum_build_height: -64,
                    ultrawarm: false,
                    border: CollisionBorder::default(),
                    rain: RainAvailability::NotRaining,
                    has_passengers: false,
                },
                &[],
                &mut host,
            )
            .unwrap();
        assert_eq!(host.visits, vec![17, 99]);
        assert_eq!(host.removed, vec![3]);
        assert_eq!(runtime.item(17).unwrap().stack.count, 5);
        assert!(runtime.item(3).is_none());
        assert!(runtime.index.by_id(3).is_none());
        assert!(!runtime.ticks.contains(3));
        assert!(!runtime.tracking_order.contains_key(3));
    }

    #[test]
    fn pickup_uses_one_ordered_outbox_and_preserves_the_discarded_stack_object() {
        use crate::player_inventory::SharedContainer;
        use std::sync::Mutex;
        struct PickupHost {
            menu: SharedContainer,
            calls: Vec<&'static str>,
        }
        impl RuntimeEntityHost for PickupHost {
            fn level_player_tick(&mut self, _: EntityHandle) -> Result<()> {
                anyhow::bail!("not a level tick")
            }
            fn profile(&mut self, _: ProfileEvent) {}
            fn effect(
                &mut self,
                _: &EntityPhysicalState,
                _: RuntimeItemEffect,
                _: Option<i64>,
            ) -> Result<Vec<EffectDelivery>> {
                anyhow::bail!("unexpected effect")
            }
            fn entity_damage(
                &mut self,
                _: &EntityPhysicalState,
                _: &ItemStack,
                _: i32,
                _: Option<i32>,
            ) -> Result<()> {
                anyhow::bail!("unexpected damage")
            }
            fn discarded(
                &mut self,
                state: &EntityPhysicalState,
                stack: &ItemStack,
                _: i32,
            ) -> Result<()> {
                assert!(state.removed);
                assert_eq!(stack.count, 0);
                assert_eq!(stack.item_id, 1);
                self.calls.push("discard");
                Ok(())
            }
            fn add_to_inventory(
                &mut self,
                _: PickupPlayer,
                _: i32,
                original: i32,
                stack: &mut ItemStack,
            ) -> Result<bool> {
                assert_eq!(original, 3);
                self.calls.push("inventory_add");
                self.menu
                    .transact(|menu| Ok((menu.add_pickup_to_inventory(stack, false)?, vec![])))
            }
            fn collect_item(
                &mut self,
                _: PickupPlayer,
                state: &ItemState,
                original: i32,
                accepted: i32,
            ) -> Result<()> {
                assert!(!state.removed);
                assert_eq!(state.stack.count, 0);
                assert_eq!((original, accepted), (3, 3));
                self.calls.push("take");
                self.menu
                    .transact(|menu| Ok(((), menu.broadcast_changes()?)))
            }
            fn collected(
                &mut self,
                _: PickupPlayer,
                state: &ItemState,
                pickup: Pickup,
                original_item_id: i32,
            ) -> Result<()> {
                assert!(state.removed);
                assert_eq!((state.stack.item_id, state.stack.count), (1, 3));
                assert_eq!(original_item_id, 1);
                assert!(pickup.removed);
                self.calls.push("stats_then_on_pickup");
                Ok(())
            }
        }
        let order = Arc::new(Mutex::new(Vec::new()));
        let menu_order = Arc::clone(&order);
        let menu = SharedContainer::with_publisher(Arc::new(move |update| {
            if !update.packets.is_empty() {
                menu_order.lock().unwrap().push("menu");
            }
        }));
        let entity_order = Arc::clone(&order);
        let mut runtime = RuntimeEntities::new(LegacyRandomSource::new(95)).with_publisher(
            Arc::new(move |event| {
                let kind = match event {
                    RuntimeEntityEvent::Update { .. } => "update",
                    RuntimeEntityEvent::Collected { .. } => "collect",
                    RuntimeEntityEvent::Removed { .. } => "remove",
                    RuntimeEntityEvent::Effect { .. } => "effect",
                };
                entity_order.lock().unwrap().push(kind);
            }),
        );
        runtime
            .set_chunk_visibility((0, 0), Visibility::Ticking)
            .unwrap();
        runtime
            .insert_item(item(17, 3), LegacyRandomSource::new(96))
            .unwrap();
        order.lock().unwrap().clear();
        let mut host = PickupHost {
            menu,
            calls: vec![],
        };
        runtime
            .player_touch(
                PickupPlayer {
                    entity_id: 99,
                    uuid: [99; 16],
                    bounds: Aabb::player(Vec3::new(8.5, 64., 8.5), 0.6, 1.8),
                    alive: true,
                    spectator: false,
                    vehicle_bounds: None,
                },
                &mut host,
            )
            .unwrap();
        assert_eq!(*order.lock().unwrap(), ["collect", "menu", "remove"]);
        assert_eq!(
            host.calls,
            ["inventory_add", "take", "discard", "stats_then_on_pickup"]
        );
        assert_eq!(
            host.menu
                .read(|menu| menu.inventory.slot(36).unwrap().count)
                .unwrap(),
            3
        );
        assert!(runtime.item(17).is_none());
        assert!(runtime.index.by_id(17).is_none());
    }

    #[test]
    fn menu_and_prepared_drop_commit_together_before_the_menu_outbox() {
        use crate::player_inventory::SharedContainer;
        use std::sync::Mutex;
        let order = Arc::new(Mutex::new(Vec::new()));
        let menu_order = Arc::clone(&order);
        let menu = SharedContainer::with_publisher(Arc::new(move |update| {
            if !update.packets.is_empty() {
                menu_order.lock().unwrap().push("menu");
            }
        }));
        menu.transact(|menu| {
            menu.inventory
                .set_slot_authoritative(36, ItemStack::new(1, 3)?);
            menu.record_external_sync();
            Ok(((), vec![]))
        })
        .unwrap();
        let entity_order = Arc::clone(&order);
        let mut runtime = RuntimeEntities::new(LegacyRandomSource::new(97)).with_publisher(
            Arc::new(move |event| {
                if matches!(event, RuntimeEntityEvent::Update { .. }) {
                    entity_order.lock().unwrap().push("spawn");
                }
            }),
        );
        runtime
            .set_chunk_visibility((0, 0), Visibility::Ticking)
            .unwrap();
        let state_before = menu.read(|menu| menu.inventory.state_id()).unwrap();
        let failed: Result<Vec<EntityHandle>> = {
            let runtime = &mut runtime;
            menu.transact_with_commit(
                move |menu| {
                    menu.inventory
                        .set_slot_authoritative(36, ItemStack::empty());
                    let mut invalid = item(18, 1);
                    invalid.vehicle = Some(99);
                    let prepared = runtime.prepare_items(vec![
                        (item(17, 3), LegacyRandomSource::new(98)),
                        (invalid, LegacyRandomSource::new(99)),
                    ])?;
                    Ok((prepared, menu.broadcast_changes()?))
                },
                |prepared| prepared.commit(),
            )
        };
        assert!(failed.is_err());
        assert_eq!(runtime.item_count(), 0);
        assert!(runtime.index.by_id(17).is_none());
        assert!(order.lock().unwrap().is_empty());
        assert_eq!(
            menu.read(|menu| (
                menu.inventory.slot(36).unwrap().count,
                menu.inventory.state_id()
            ))
            .unwrap(),
            (3, state_before)
        );
        let rng = LegacyRandomSource::new(100);
        let expected_rng = rng.state48();
        let handles = {
            let runtime = &mut runtime;
            menu.transact_with_commit(
                move |menu| {
                    let stack = menu.inventory.slot(36).unwrap().clone();
                    menu.inventory
                        .set_slot_authoritative(36, ItemStack::empty());
                    let mut state = item(17, 3);
                    state.stack = stack;
                    let prepared =
                        runtime.prepare_items(vec![(state, LegacyRandomSource::new(101))])?;
                    Ok((prepared, menu.broadcast_changes()?))
                },
                move |prepared| prepared.commit_with_world_random(rng),
            )
            .unwrap()
        };
        assert_eq!(handles[0].generation, 1);
        assert_eq!(*order.lock().unwrap(), ["spawn", "menu"]);
        assert!(menu
            .read(|menu| menu.inventory.slot(36).unwrap().is_empty())
            .unwrap());
        assert_eq!(runtime.item(17).unwrap().stack.count, 3);
        assert_eq!(runtime.world_random.state48(), expected_rng);
    }

    #[test]
    fn dropping_a_prepared_batch_reserves_no_entity_identity_or_rng_state() {
        let mut runtime = RuntimeEntities::new(LegacyRandomSource::new(102));
        let original_rng = runtime.world_random.state48();
        let mut invalid = item(18, 1);
        invalid.metadata_dirty = 0x200;
        assert!(runtime
            .prepare_items(vec![(invalid, LegacyRandomSource::new(105))])
            .is_err());
        drop(
            runtime
                .prepare_items(vec![(item(17, 3), LegacyRandomSource::new(103))])
                .unwrap(),
        );
        assert!(runtime.index.by_id(17).is_none());
        assert_eq!(runtime.item_count(), 0);
        assert!(runtime.drain_events().is_empty());
        assert_eq!(runtime.world_random.state48(), original_rng);
        assert_eq!(
            runtime
                .insert_item(item(17, 3), LegacyRandomSource::new(104))
                .unwrap()
                .generation,
            1
        );
    }
}
