//! Owned gameplay callbacks for the native item fixture slice.
//!
//! NativeFixtureEmptyCriteria is an explicit fixture policy: no unported
//! advancement criterion is registered. The vanilla recipe advancements
//! (`recipes/**`, #27) are the one loaded tree; their listeners are evaluated
//! here and their awards are applied by `main_recipe_advancements`. Player
//! phases below implement only this item's callback slice, never full
//! ServerPlayer.tick.
use anyhow::{ensure, Context, Result};
use mc121_rust::{
    container::LocalSlotChange,
    entity::{EntityRegistry, PlayerState},
    entity_move::ProfileEvent,
    entity_state::EntityPhysicalState,
    game_event_context::{self, GameEventRegistryWorld, ItemGameEvent, RegistryState},
    inventory::{ItemStack, PlayerInventory},
    item_entity::{ItemState, Pickup},
    item_host::RuntimeGameplayHooks,
    player_stats::{PlayerStats, StatKey},
    runtime_chunk_manager::RuntimeChunkManager,
    runtime_entities::PickupPlayer,
    runtime_entity_index::EntityHandle,
};
use mc121_rust::advancements::player::{inventory_from_menu, Listener, PlayerAdvancements, RecipeAdvancements};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type Uuid = [u8; 16];
type Section = (i32, i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameplaySourceScope {
    /// Fresh native fixture, no imported entities/listeners/objectives and no
    /// advancement registrations. All later mutations must pass this owner.
    NativeFixtureEmptyCriteria,
    UnknownImported,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerPhaseScope {
    NativeItemCallbacksOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameplayCriterion {
    ThrownItemPickedUpByEntity,
    ThrownItemPickedUpByPlayer,
    InventoryChanged,
    Tick,
    DefaultBlockUse,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventorySlotCounts {
    pub full: i32,
    pub empty: i32,
    pub occupied: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameplayObservation {
    AwardPickup {
        player: Uuid,
        item_id: i32,
        count: i32,
    },
    EmptyScoreboard {
        player: Uuid,
        key: StatKey,
    },
    EmptyCriterion {
        player: Uuid,
        criterion: GameplayCriterion,
    },
    InventoryScan {
        player: Uuid,
        counts: InventorySlotCounts,
    },
    Discarded {
        entity_id: i32,
    },
}

/// This owner is the registry. An empty value is created only by the explicit
/// fresh-fixture constructor, never inferred from missing imported files.
pub struct MainItemGameplay {
    pub scoreboard: mc121_rust::scoreboard::Scoreboard,
    pub scoreboard_names: BTreeMap<Uuid, String>,
    source_scope: GameplaySourceScope,
    invalid_reason: Option<String>,
    active_players: BTreeMap<i32, Uuid>,
    known_player_uuids: BTreeSet<Uuid>,
    stats: BTreeMap<Uuid, PlayerStats>,
    // Independent native player-store provenance, not a world/gameplay fact.
    native_stats: BTreeSet<Uuid>,
    criteria: BTreeMap<(Uuid, GameplayCriterion), BTreeSet<String>>,
    objectives: BTreeMap<StatKey, BTreeSet<String>>,
    sections: BTreeMap<Section, RegistryState>,
    listener_blocks: BTreeMap<(i32, i32, i32), u32>,
    observations: VecDeque<GameplayObservation>,
    level_player_callbacks: u64,
    connection_callbacks: u64,
    profile_counts: [u64; 4],
    /// Per-player progress of the loaded recipe advancements. Kept by UUID
    /// after disconnect so the save path still reads it.
    recipe_advancements: BTreeMap<Uuid, PlayerAdvancements>,
    /// Matched criteria, in trigger order, awaiting `award` by the owner of
    /// the recipe books.
    pending_awards: Vec<(Uuid, Listener)>,
    /// Known recipes already reported to RecipeUnlockedTrigger, so additions
    /// made by other recipe-book owners (furnace, smithing, stonecutter
    /// outputs) still fire it.
    recipe_trigger_baseline: BTreeMap<Uuid, BTreeSet<String>>,
    /// Players whose recipe-book INIT has been built. Awards wait for it: the
    /// original sends INIT in placeNewPlayer, before the player's first tick.
    recipe_book_ready: BTreeSet<Uuid>,
}
impl MainItemGameplay {
    pub fn new(source_scope: GameplaySourceScope) -> Self {
        Self {
            scoreboard: mc121_rust::scoreboard::Scoreboard::new_server(),
            scoreboard_names: BTreeMap::new(),
            source_scope,
            invalid_reason: None,
            active_players: BTreeMap::new(),
            known_player_uuids: BTreeSet::new(),
            stats: BTreeMap::new(),
            native_stats: BTreeSet::new(),
            criteria: BTreeMap::new(),
            objectives: BTreeMap::new(),
            sections: BTreeMap::new(),
            listener_blocks: BTreeMap::new(),
            observations: VecDeque::new(),
            level_player_callbacks: 0,
            connection_callbacks: 0,
            profile_counts: [0; 4],
            recipe_advancements: BTreeMap::new(),
            pending_awards: Vec::new(),
            recipe_trigger_baseline: BTreeMap::new(),
            recipe_book_ready: BTreeSet::new(),
        }
    }
    /// PlayerAdvancements construction for an admitted player (load or new).
    pub fn install_recipe_advancements(&mut self, uuid: Uuid, advancements: PlayerAdvancements, known_recipes: BTreeSet<String>) {
        self.recipe_advancements.insert(uuid, advancements);
        self.recipe_trigger_baseline.insert(uuid, known_recipes);
        self.recipe_book_ready.remove(&uuid);
    }
    pub fn recipe_book_initialized(&mut self, uuid: Uuid) {
        self.recipe_book_ready.insert(uuid);
    }
    pub fn recipe_book_ready(&self, uuid: Uuid) -> bool {
        self.recipe_book_ready.contains(&uuid)
    }
    pub fn recipe_trigger_baseline_mut(&mut self, uuid: Uuid) -> &mut BTreeSet<String> {
        self.recipe_trigger_baseline.entry(uuid).or_default()
    }
    pub fn recipe_advancements(&self, uuid: Uuid) -> Option<&PlayerAdvancements> {
        self.recipe_advancements.get(&uuid)
    }
    pub fn recipe_advancements_mut(&mut self, uuid: Uuid) -> Option<&mut PlayerAdvancements> {
        self.recipe_advancements.get_mut(&uuid)
    }
    pub fn take_pending_awards(&mut self) -> Vec<(Uuid, Listener)> {
        std::mem::take(&mut self.pending_awards)
    }
    pub fn requeue_pending_awards(&mut self, awards: Vec<(Uuid, Listener)>) {
        let later = std::mem::replace(&mut self.pending_awards, awards);
        self.pending_awards.extend(later);
    }
    pub fn active_player_ids(&self) -> Vec<(i32, Uuid)> {
        self.active_players.iter().map(|(id, uuid)| (*id, *uuid)).collect()
    }
    /// The recipe advancements' InventoryChangeTrigger listeners for one
    /// slotChanged callback, against the inventory at that callback.
    fn match_inventory_changed(&self, uuid: Uuid, inventory: &PlayerInventory, changed: &ItemStack) -> Result<Vec<(Uuid, Listener)>> {
        let Some(advancements) = self.recipe_advancements.get(&uuid) else { return Ok(Vec::new()) };
        let slots = inventory_from_menu(inventory.slots());
        Ok(advancements
            .inventory_changed(RecipeAdvancements::vanilla(), &slots, changed)?
            .into_iter()
            .map(|listener| (uuid, listener))
            .collect())
    }
    /// ServerPlayer.onInsideBlock: EnterBlockTrigger for one block state.
    pub fn enter_block(&mut self, uuid: Uuid, state: u32) -> Result<()> {
        self.require_supported_floor()?;
        let catalogue = RecipeAdvancements::vanilla();
        if let Some(advancements) = self.recipe_advancements.get(&uuid) {
            if advancements.has_enter_block_listeners(catalogue) {
                let matched = advancements.enter_block(catalogue, state)?;
                self.pending_awards.extend(matched.into_iter().map(|listener| (uuid, listener)));
            }
        }
        Ok(())
    }
    /// Login: the first broadcastChanges of the InventoryMenu reports every
    /// non-empty inventory-backed slot to the listener.
    pub fn login_inventory_changes(&mut self, uuid: Uuid, inventory: &PlayerInventory, changes: Vec<LocalSlotChange>) -> Result<()> {
        for change in changes {
            if change.inventory_slot.is_some() {
                self.inventory_slot_changed(uuid, change.menu_slot, inventory, &change.stack)?;
            }
        }
        Ok(())
    }
    pub fn phase_scope(&self) -> PlayerPhaseScope {
        PlayerPhaseScope::NativeItemCallbacksOnly
    }
    pub fn register_player(&mut self, id: i32, uuid: Uuid) -> Result<()> {
        ensure!(
            !self.active_players.contains_key(&id),
            "gameplay player ID already registered"
        );
        ensure!(
            !self.active_players.values().any(|value| *value == uuid),
            "gameplay UUID already active"
        );
        self.active_players.insert(id, uuid);
        self.known_player_uuids.insert(uuid);
        // Original PlayerList.placeNewPlayer always marks all stored values
        // dirty at bytecode494, including when this UUID reconnects after a
        // prior sendStats drain. Missing keys remain absent, rather than zeros.
        self.stats.entry(uuid).or_default().mark_all_dirty();
        Ok(())
    }
    /// Pure admission of the actual native callback/statistic owner. This
    /// neither registers another player nor constructs default counters.
    pub(super) fn validate_active_player(&self, id: i32, uuid: Uuid) -> Result<()> {
        self.require_supported_floor()?;
        ensure!(self.active_players.get(&id) == Some(&uuid),
            "gameplay active player identity changed");
        ensure!(self.known_player_uuids.contains(&uuid) && self.stats.contains_key(&uuid),
            "gameplay active player statistics are not registered");
        Ok(())
    }
    pub fn remove_player(&mut self, id: i32) {
        if let Some(uuid) = self.active_players.remove(&id) {
            self.scoreboard_names.remove(&uuid);
            // Original stopListening removes this PlayerAdvancements identity.
            // Values survive this connection's lifetime under the UUID owner.
            self.criteria.retain(|(player, _), _| *player != uuid);
        }
    }
    pub fn stats_mut(&mut self, uuid: Uuid) -> Result<&mut PlayerStats> {
        if !self.native_stats_owned(uuid) {
            self.require_supported_floor()?;
        }
        self.stats
            .get_mut(&uuid)
            .context("gameplay statistics UUID not registered")
    }
    /// ServerPlayer.awardStat for an actually admitted actor. The real native
    /// floor owns the empty objective/criterion maps; native save provenance
    /// alone does not authorize omitting these callbacks.
    pub fn award_player_stat(&mut self, id: i32, uuid: Uuid, key: StatKey, amount: i32) -> Result<()> {
        self.require_supported_floor()?;
        ensure!(self.active_players.get(&id) == Some(&uuid), "statistic actor identity changed");
        key.validate()?;
        self.stats.get_mut(&uuid).context("statistic counter owner is absent")?
            .increment(key, amount)?;
        self.award_scoreboard_stat(uuid, key, amount)?;
        if !self.has_stat_objective(key)? {
            self.observe(GameplayObservation::EmptyScoreboard { player: uuid, key });
        }
        Ok(())
    }
    fn stat_criterion(key: StatKey) -> Result<String> {
        let (kind, value) = mc121_rust::player_stats::stat_names(key).context("statistic name missing")?;
        Ok(format!("{}:{}", kind.replace(':', "."), value.replace(':', ".")))
    }
    fn has_stat_objective(&self, key: StatKey) -> Result<bool> {
        let criteria = Self::stat_criterion(key)?;
        Ok(self.scoreboard.objectives().iter().any(|o| o.criteria.name() == criteria))
    }
    pub fn award_scoreboard_stat(&mut self, uuid: Uuid, key: StatKey, amount: i32) -> Result<()> {
        if self.has_stat_objective(key)? {
            let name = self.scoreboard_names.get(&uuid).context("scoreboard statistic actor name is missing")?;
            let holder=mc121_rust::scoreboard::names::player_holder(&self.scoreboard,name,uuid);
            mc121_rust::scoreboard::hooks::award_stat(&mut self.scoreboard,
                &holder, &Self::stat_criterion(key)?, amount)?;
        }
        Ok(())
    }
    /// Transactional workstation/drop counters publish their statistic criteria at commit.
    pub fn replace_player_stats(&mut self, uuid: Uuid, mut stats: PlayerStats) -> Result<()> {
        let cursor = self.stats.get(&uuid).context("statistics owner missing")?.award_cursor();
        let awards = stats.awards_after(cursor).collect::<Vec<_>>();
        for (key, amount) in awards { self.award_scoreboard_stat(uuid, key, amount)?; }
        stats.finish_transaction();
        self.stats.insert(uuid, stats);
        Ok(())
    }
    /// Install a verified native player-store counter after admission, or an
    /// explicit fresh native counter after that store reports no file. Native
    /// statistics provenance is independent of imported-world capabilities.
    /// This replaces rather than merges values and marks every value dirty.
    pub fn install_player_stats(&mut self, uuid: Uuid, mut stats: PlayerStats) -> Result<()> {
        ensure!(
            self.known_player_uuids.contains(&uuid)
                && self.active_players.values().any(|active| *active == uuid),
            "persisted statistics require an admitted known player UUID"
        );
        stats.finish_transaction();
        stats.mark_all_dirty();
        self.stats.insert(uuid, stats);
        self.native_stats.insert(uuid);
        Ok(())
    }
    pub fn native_stats_owned(&self, uuid: Uuid) -> bool {
        self.native_stats.contains(&uuid)
    }
    /// Clone the authoritative complete counters for one native save
    /// transaction. Offline known players retain values after removal.
    pub fn snapshot_player_stats(&self, uuid: Uuid) -> Result<PlayerStats> {
        if !self.native_stats_owned(uuid) {
            self.require_supported_floor()?;
        }
        ensure!(
            self.known_player_uuids.contains(&uuid),
            "statistics snapshot requires a known player UUID"
        );
        self.stats
            .get(&uuid)
            .map(PlayerStats::for_transaction)
            .context("gameplay statistics UUID has no counter owner")
    }
    /// Preload UUID identities only from fully validated native player saves.
    /// This does not admit players, create counters, register criteria or
    /// invent live thrower entities. This independent native identity source
    /// grants no imported-world gameplay capability or empty counter fallback.
    pub fn restore_known_native_players(
        &mut self,
        verified_native_uuids: impl IntoIterator<Item = Uuid>,
    ) -> Result<()> {
        let verified = verified_native_uuids.into_iter().collect::<BTreeSet<_>>();
        self.known_player_uuids.extend(verified);
        Ok(())
    }
    pub fn require_supported_floor(&self) -> Result<()> {
        ensure!(
            self.source_scope == GameplaySourceScope::NativeFixtureEmptyCriteria,
            "gameplay source coverage is unknown for imported data"
        );
        ensure!(
            self.invalid_reason.is_none(),
            "gameplay source coverage invalidated: {:?}",
            self.invalid_reason
        );
        ensure!(
            self.criteria.values().all(BTreeSet::is_empty),
            "registered advancement criteria require actual predicate/award execution"
        );
        ensure!(
            self.objectives.values().all(BTreeSet::is_empty),
            "registered statistic objectives require actual score dispatch"
        );
        ensure!(
            self.listener_blocks.is_empty(),
            "registered block game-event listeners require actual dispatch"
        );
        Ok(())
    }
    pub fn invalidate_world(&mut self, reason: impl Into<String>) {
        self.invalid_reason = Some(reason.into());
    }
    /// Record unsupported live registrations instead of presenting an empty
    /// registry. A later supported-slice guard fails before item mutation.
    pub fn register_unported_criterion(
        &mut self,
        uuid: Uuid,
        criterion: GameplayCriterion,
        identity: String,
    ) {
        self.criteria
            .entry((uuid, criterion))
            .or_default()
            .insert(identity);
    }
    pub fn register_unported_objective(&mut self, key: StatKey, identity: String) -> Result<()> {
        key.validate()?;
        self.objectives.entry(key).or_default().insert(identity);
        Ok(())
    }
    /// All fresh-world block mutations must pass here, including neighbour
    /// changes. The actual factory catalogue certifies the four listener block
    /// families; their nonempty dispatch remains explicitly unsupported.
    pub fn observe_block_change(&mut self, position: (i32, i32, i32), state: u32) -> Result<()> {
        let facts = listener_facts();
        ensure!(
            state < facts.state_count,
            "uncovered block state for listener source guard"
        );
        if facts.states.contains(&state) {
            self.listener_blocks.insert(position, state);
        } else {
            self.listener_blocks.remove(&position);
        }
        Ok(())
    }
    /// Dispatch the source DefaultBlockInteractionTrigger for an actual player.
    /// The caller must first query the post-use target through its real world
    /// owner, while retaining the world/player lock. This registry owns only
    /// the explicitly empty-listener branch: no location predicate consumes
    /// the target and no synthetic LootContext or advancement is created.
    pub(super) fn default_block_use(
        &mut self,
        uuid: Uuid,
        _target: (i32, i32, i32),
    ) -> Result<()> {
        self.require_supported_floor()?;
        ensure!(
            self.active_players.values().any(|active| *active == uuid),
            "default block-use criterion player is not active"
        );
        self.empty_criterion(uuid, GameplayCriterion::DefaultBlockUse)
    }
    /// Call only at the actual ContainerListener.slotChanged boundary for the
    /// player's InventoryMenu. It has a separate local baseline from remote
    /// prediction. Slots0..4 and carried are not inventory-backed listeners.
    pub fn inventory_slot_changed(
        &mut self,
        uuid: Uuid,
        menu_slot: usize,
        inventory: &PlayerInventory,
        changed_stack: &ItemStack,
    ) -> Result<Option<InventorySlotCounts>> {
        self.require_supported_floor()?;
        ensure!(
            self.stats.contains_key(&uuid),
            "inventory listener player is not registered"
        );
        ensure!(menu_slot < 46, "not an InventoryMenu slot");
        if menu_slot < 5 {
            return Ok(None);
        }
        let counts = inventory_counts(inventory)?;
        self.observe(GameplayObservation::InventoryScan {
            player: uuid,
            counts,
        });
        self.empty_criterion(uuid, GameplayCriterion::InventoryChanged)?;
        let matched = self.match_inventory_changed(uuid, inventory, changed_stack)?;
        self.pending_awards.extend(matched);
        Ok(Some(counts))
    }
    /// Prepare the immutable empty-criteria observations and staged counter
    /// before an atomic menu/drop commit. The caller must hold its player/menu
    /// ownership lock. These listeners cannot mutate inventory or publish an
    /// advancement, so every scan sees the same final authoritative inventory.
    /// No real statistic, listener state or outbox is mutated here.
    pub fn prepare_menu_commit<'a>(
        &'a mut self,
        uuid: Uuid,
        inventory: &PlayerInventory,
        local_changes: Vec<LocalSlotChange>,
        next_stats: PlayerStats,
    ) -> Result<PreparedGameplayMenu<'a>> {
        self.require_supported_floor()?;
        ensure!(
            self.active_players.values().any(|player| *player == uuid),
            "prepared menu player is not active"
        );
        ensure!(
            self.stats.contains_key(&uuid),
            "prepared menu statistics missing"
        );
        let mut counts = None;
        let mut observations = Vec::new();
        let mut awards = Vec::new();
        for change in local_changes {
            ensure!(
                change.menu_slot < 46,
                "prepared callback is not an InventoryMenu slot"
            );
            let expected_inventory_slot = match change.menu_slot {
                5..=8 => Some(44 - change.menu_slot),
                9..=35 => Some(change.menu_slot),
                36..=44 => Some(change.menu_slot - 36),
                45 => Some(40),
                _ => None,
            };
            ensure!(
                change.inventory_slot == expected_inventory_slot,
                "prepared callback backing inventory identity is inconsistent"
            );
            if expected_inventory_slot.is_none() {
                continue;
            }
            let scan = match counts {
                Some(counts) => counts,
                None => {
                    let scan = inventory_counts(inventory)?;
                    counts = Some(scan);
                    scan
                }
            };
            observations.push(GameplayObservation::InventoryScan {
                player: uuid,
                counts: scan,
            });
            observations.push(GameplayObservation::EmptyCriterion {
                player: uuid,
                criterion: GameplayCriterion::InventoryChanged,
            });
            awards.extend(self.match_inventory_changed(uuid, inventory, &change.stack)?);
        }
        Ok(PreparedGameplayMenu {
            gameplay: self,
            uuid,
            next_stats,
            observations,
            block_changes: Vec::new(),
            awards,
        })
    }
    /// Validate native listener-source changes before an atomic menu/world
    /// commit. The prepared token owns all later listener observations, so the
    /// world commit never needs a second mutable borrow of this registry.
    pub fn prepare_menu_block_commit<'a>(
        &'a mut self,
        uuid: Uuid,
        inventory: &PlayerInventory,
        local_changes: Vec<LocalSlotChange>,
        next_stats: PlayerStats,
        block_changes: Vec<mc121_rust::world::BlockChange>,
    ) -> Result<PreparedGameplayMenu<'a>> {
        let state_count = listener_facts().state_count;
        ensure!(
            block_changes
                .iter()
                .all(|change| change.state < state_count),
            "uncovered block state for prepared listener-source change"
        );
        let mut prepared = self.prepare_menu_commit(uuid, inventory, local_changes, next_stats)?;
        prepared.block_changes = block_changes;
        Ok(prepared)
    }
    pub fn connection_item_phase(&mut self, player_id: i32) -> Result<()> {
        self.require_supported_floor()?;
        ensure!(
            self.active_players.contains_key(&player_id),
            "connection gameplay player missing"
        );
        // Instrumentation of the explicitly bounded item callback phase. The
        // caller executes actual item-touch/menu work; no full doTick is claimed.
        self.connection_callbacks = self.connection_callbacks.wrapping_add(1);
        Ok(())
    }
    // Bounded diagnostics; these are not game packets or authoritative events.
    fn observe(&mut self, observation: GameplayObservation) {
        if self.observations.len() == 1024 {
            self.observations.pop_front();
        }
        self.observations.push_back(observation);
    }
    pub fn drain_observations(&mut self) -> Vec<GameplayObservation> {
        self.observations.drain(..).collect()
    }
    pub fn frame<'a>(
        &'a mut self,
        players: &'a EntityRegistry,
        manager: &'a RuntimeChunkManager,
        in_overworld: bool,
    ) -> MainGameplayFrame<'a> {
        MainGameplayFrame {
            gameplay: self,
            players,
            manager,
            in_overworld,
            current_region: None,
        }
    }
    /// Positive loaded-region capability acquired under the same world/player
    /// lock as this frame. Pending ticket input may coexist with unchanged
    /// current FULL futures; no scheduler work is executed by these queries.
    pub(super) fn frame_current<'a>(
        &'a mut self,
        players: &'a EntityRegistry,
        manager: &'a RuntimeChunkManager,
        in_overworld: bool,
        region: super::main_chunk_runtime::CurrentChunkQueryRegion,
    ) -> MainGameplayFrame<'a> {
        MainGameplayFrame {
            gameplay: self,
            players,
            manager,
            in_overworld,
            current_region: Some(region),
        }
    }
    fn empty_criterion(&mut self, player: Uuid, criterion: GameplayCriterion) -> Result<()> {
        ensure!(
            self.criteria
                .get(&(player, criterion))
                .is_none_or(BTreeSet::is_empty),
            "nonempty criterion registry requires actual execution"
        );
        self.observe(GameplayObservation::EmptyCriterion { player, criterion });
        Ok(())
    }
}
/// Exclusive owner borrow keeps all checked source/criterion facts stable up
/// to commit. Dropping this token leaves real stats and observations untouched.
pub struct PreparedGameplayMenu<'a> {
    gameplay: &'a mut MainItemGameplay,
    uuid: Uuid,
    next_stats: PlayerStats,
    observations: Vec<GameplayObservation>,
    block_changes: Vec<mc121_rust::world::BlockChange>,
    awards: Vec<(Uuid, Listener)>,
}
impl PreparedGameplayMenu<'_> {
    pub fn commit(self) {
        self.gameplay.pending_awards.extend(self.awards);
        for change in self.block_changes {
            self.gameplay
                .observe_block_change((change.x, change.y, change.z), change.state)
                .expect("prepared listener-source states were validated before world commit");
        }
        self.gameplay.replace_player_stats(self.uuid, self.next_stats).expect("prepared native scoreboard/statistic commit");
        for observation in self.observations {
            self.gameplay.observe(observation);
        }
    }
}
fn inventory_counts(inventory: &PlayerInventory) -> Result<InventorySlotCounts> {
    let mut counts = InventorySlotCounts::default();
    // Actual Inventory order: main hotbar+main, armour, offhand. Native storage
    // uses InventoryMenu indices, which are a distinct mapping.
    let slots = inventory.slots();
    for index in (36..45)
        .chain(9..36)
        .chain((5..9).rev())
        .chain(std::iter::once(45))
    {
        let stack = &slots[index];
        if stack.is_empty() {
            counts.empty += 1;
        } else {
            counts.occupied += 1;
            if stack.count >= stack.max_stack_size()? {
                counts.full += 1;
            }
        }
    }
    Ok(counts)
}
struct ListenerFacts {
    state_count: u32,
    states: BTreeSet<u32>,
}
fn listener_facts() -> &'static ListenerFacts {
    static FACTS: std::sync::OnceLock<ListenerFacts> = std::sync::OnceLock::new();
    FACTS.get_or_init(|| {
        let facts: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/main-item-gameplay-facts.json"))
                .expect("original listener-source facts");
        assert_eq!(facts["dynamic_item_and_player_listener_registrations"], 0);
        assert_eq!(facts["default_vanilla_recipe_unlocks_implemented"], true);
        ListenerFacts {
            state_count: facts["block_states"].as_u64().unwrap() as u32,
            states: facts["listener_state_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect(),
        }
    })
}
pub struct MainGameplayFrame<'a> {
    gameplay: &'a mut MainItemGameplay,
    players: &'a EntityRegistry,
    manager: &'a RuntimeChunkManager,
    in_overworld: bool,
    current_region: Option<super::main_chunk_runtime::CurrentChunkQueryRegion>,
}
impl MainGameplayFrame<'_> {
    fn validate_player(&self, player: PickupPlayer) -> Result<()> {
        ensure!(
            self.players
                .get(player.entity_id)
                .is_some_and(|state| state.profile.uuid == player.uuid),
            "gameplay picker identity changed"
        );
        ensure!(
            self.gameplay.active_players.get(&player.entity_id) == Some(&player.uuid),
            "gameplay picker not registered"
        );
        Ok(())
    }
    /// Preflight the same actual callback owner and historical inventory used
    /// by the subsequent source listener replay. Dropping the prepared token
    /// executes no observation/statistic mutation or packet publication.
    pub(super) fn preflight_inventory_slot_changes(
        &mut self,
        player: PickupPlayer,
        inventory: &PlayerInventory,
        changes: &[LocalSlotChange],
    ) -> Result<()> {
        self.validate_player(player)?;
        self.gameplay.validate_active_player(player.entity_id, player.uuid)?;
        let stats = self.gameplay.snapshot_player_stats(player.uuid)?;
        drop(self.gameplay.prepare_menu_commit(player.uuid, inventory, changes.to_vec(), stats)?);
        Ok(())
    }
    fn post(&mut self, state: &EntityPhysicalState, event: ItemGameEvent) -> Result<()> {
        self.gameplay.require_supported_floor()?;
        game_event_context::post_to_empty_registries(self, event.at(state.position_vec()))?;
        Ok(())
    }
}
impl GameEventRegistryWorld for MainGameplayFrame<'_> {
    type Chunk = (i32, i32);
    fn get_loaded_chunk(&mut self, x: i32, z: i32) -> Result<Option<Self::Chunk>> {
        self.gameplay.require_supported_floor()?;
        // Main must first certify_query_region over the complete swept item
        // bounds plus the event radius, after driving its source manager to
        // quiescence. Accessibility FULL alone is not a general getChunkNow
        // implementation: original also has a four-entry cache and visible
        // holder generation futures. Never turn an unproved state into None.
        if let Some(region) = self.current_region {
            ensure!(
                region.contains((x, z)),
                "game-event query exceeds certified current region"
            );
        } else {
            ensure!(
                !self.manager.has_scheduler_work(),
                "game-event query requires a quiescent certified source region"
            );
        }
        self.manager.certify_full_region((x, z), (x, z))?;
        Ok(Some((x, z)))
    }
    fn registry_state(&mut self, chunk: Self::Chunk, y: i32) -> Result<RegistryState> {
        self.gameplay.require_supported_floor()?;
        let registry = self
            .gameplay
            .sections
            .entry((chunk.0, y, chunk.1))
            .or_insert(RegistryState::Euclidean {
                listeners: 0,
                pending_add: 0,
                pending_remove: 0,
                processing: false,
            });
        Ok(*registry)
    }
}
impl RuntimeGameplayHooks for MainGameplayFrame<'_> {
    fn inventory_slot_changed(
        &mut self,
        player: PickupPlayer,
        menu_slot: usize,
        inventory: &PlayerInventory,
        changed_stack: &ItemStack,
    ) -> Result<()> {
        self.validate_player(player)?;
        self.gameplay
            .inventory_slot_changed(player.uuid, menu_slot, inventory, changed_stack)?;
        Ok(())
    }
    fn level_player_tick(&mut self, player: EntityHandle) -> Result<()> {
        self.gameplay.require_supported_floor()?;
        let state = self
            .players
            .get(player.id)
            .context("level gameplay player missing")?;
        ensure!(
            self.gameplay.active_players.get(&player.id) == Some(&state.profile.uuid),
            "level gameplay identity changed"
        );
        self.gameplay.level_player_callbacks = self.gameplay.level_player_callbacks.wrapping_add(1);
        Ok(())
    }
    fn profile(&mut self, event: ProfileEvent) {
        let index = match event {
            ProfileEvent::PushMove => 0,
            ProfileEvent::PopMove => 1,
            ProfileEvent::PushRest => 2,
            ProfileEvent::PopRest => 3,
        };
        self.gameplay.profile_counts[index] = self.gameplay.profile_counts[index].wrapping_add(1);
    }
    fn player_in_overworld(&self, player: &PlayerState) -> Result<bool> {
        ensure!(
            self.gameplay.active_players.get(&player.entity_id) == Some(&player.profile.uuid),
            "sound recipient gameplay identity missing"
        );
        Ok(self.in_overworld)
    }
    fn hit_ground(&mut self, state: &EntityPhysicalState, _affected_block: u32) -> Result<()> {
        self.post(state, ItemGameEvent::HitGround)
    }
    fn splash(&mut self, state: &EntityPhysicalState) -> Result<()> {
        self.post(state, ItemGameEvent::Splash)
    }
    fn entity_damage(
        &mut self,
        state: &EntityPhysicalState,
        _stack: &ItemStack,
        _health: i32,
        _causing_entity: Option<i32>,
    ) -> Result<()> {
        self.post(state, ItemGameEvent::EntityDamage)
    }
    fn discarded(
        &mut self,
        state: &EntityPhysicalState,
        _stack: &ItemStack,
        _health: i32,
    ) -> Result<()> {
        self.gameplay.require_supported_floor()?;
        ensure!(
            state.removed,
            "item discarded callback requires removed state"
        );
        self.gameplay.observe(GameplayObservation::Discarded {
            entity_id: state.entity_id,
        });
        Ok(())
    }
    fn award_pickup_stat(
        &mut self,
        player: PickupPlayer,
        original_item_id: i32,
        original_count: i32,
    ) -> Result<()> {
        self.gameplay.require_supported_floor()?;
        self.validate_player(player)?;
        self.gameplay
            .stats_mut(player.uuid)?
            .award_pickup(original_item_id, original_count)?;
        self.gameplay.observe(GameplayObservation::AwardPickup {
            player: player.uuid,
            item_id: original_item_id,
            count: original_count,
        });
        self.gameplay.award_scoreboard_stat(player.uuid,
            StatKey::new(mc121_rust::player_stats::ITEM_PICKED_UP, original_item_id)?, original_count)?;
        let key = StatKey::new(mc121_rust::player_stats::ITEM_PICKED_UP, original_item_id)?;
        if !self.gameplay.has_stat_objective(key)? {
            self.gameplay.observe(GameplayObservation::EmptyScoreboard { player: player.uuid, key });
        }
        Ok(())
    }
    fn on_item_pickup(
        &mut self,
        player: PickupPlayer,
        state: &ItemState,
        _pickup: Pickup,
    ) -> Result<()> {
        self.gameplay.require_supported_floor()?;
        self.validate_player(player)?;
        if let Some(uuid) = state.thrower {
            ensure!(
                self.gameplay.known_player_uuids.contains(&uuid),
                "uncertified item owner UUID requires actual Entity.getOwner resolution"
            );
        }
        let owner = state
            .thrower
            .and_then(|uuid| self.players.get_by_uuid(&uuid));
        if let Some(owner) = owner {
            ensure!(
                self.gameplay.active_players.get(&owner.entity_id) == Some(&owner.profile.uuid),
                "resolved item owner has no authoritative gameplay registrations"
            );
            // dl.trigger creates a context before checking its registered set.
            // For these owned empty sets its construction only reads context
            // facts and references the existing level RNG, consuming no draws.
            self.gameplay.empty_criterion(
                owner.profile.uuid,
                GameplayCriterion::ThrownItemPickedUpByEntity,
            )?;
            self.gameplay
                .empty_criterion(player.uuid, GameplayCriterion::ThrownItemPickedUpByPlayer)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tools/main_item_gameplay_tests.rs"]
mod tests;
