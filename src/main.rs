//! Native protocol-767 server under differential validation against vanilla 1.21.

use anyhow::{anyhow, bail, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use mc121_rust::{
    auth::{valid_login_name, Authenticator, CryptoRead, CryptoWrite, SessionVerifier},
    border::WorldBorder,
    chunk_lifecycle::FullStatus,
    container::{ClickOutcome, ContainerClick, PlayerContainer},
    crafting::RecipeContext,
    creative_drop::CreativeDropGate,
    entity::{
        self, EntityEvent, EntityRegistry, PlayerMetadata, PlayerMotionTracker, PlayerPoseInput,
        PlayerProfile, PlayerState, Position,
    },
    entity_persistence::{EntityCheckpoint, EntityDataStore, NativeEntitySource, NativeWorldTime},
    entity_spatial::Visibility,
    entity_tracking::{self, TrackingObserver},
    equipment::PlayerEquipment,
    geometry::{self, static_block_intersects, Aabb, EntityCollisionContext, Vec3},
    inventory::{self, CreativeSlotAction, ItemStack},
    item_entity::{ItemState, PlayerDrop},
    item_host::RuntimeInventoryHost,
    level::LevelData,
    metadata::MetadataValue,
    nbt::Tag,
    placement::{self, PlacementContext},
    player_data::{PlayerData, PlayerDataStore, PlayerLifecycle},
    player_inventory::{ContainerUpdate, SharedContainer},
    player_physics,
    player_stats::PlayerStats,
    protocol::{FrameCodec, Packet, Reader, Writer},
    random::LegacyRandomSource,
    random_seed,
    replication::{EntitySnapshot, EntityUpdate, GenericEntityTracker, PlayerTracker},
    runtime_collision::{CollisionActor, CollisionBorder, CollisionEntity, RuntimeCollision},
    runtime_entities::{PickupPlayer, RuntimeEntities, RuntimeEntityEvent},
    runtime_item_world::RainAvailability,
    runtime_world::{RuntimeWorld as WorldBackend, WorldChange as WorldUpdate},
    server_recipe_book::{
        RecipeBookSeenRecipe, RecipeBookSettingsChange, ServerRecipeBook, VanillaRecipeCatalogue,
    },
    session::{KeepAlive, KeepAliveAction, LatencyUpdateClock},
    storage::VanillaWorld,
    tick::{TickClock, WeatherChange, WeatherState, TICK_DURATION},
    world::{self, BlockChange, FlatWorld},
    world_context::{self, DimensionFacts, WorldContext},
    world_rules::WorldRules,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering},
        Arc, RwLock as StdRwLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs,
    io::AsyncWriteExt,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::{broadcast, mpsc, watch, Mutex, RwLock, Semaphore},
    task::JoinHandle,
    time::{self, Instant},
};

mod main_chunk_runtime;
mod main_chunk_transport;
mod main_drop_commit;
mod main_fluid_simulation;
mod main_fire_simulation;
pub mod main_item_gameplay;
mod main_player_data;
mod main_player_food;
mod main_item_use;
mod main_player_body;
mod main_block_entities;
mod main_crafting_table;
mod main_block_ticks;
mod main_redstone;
mod main_player_motion_hooks;
mod main_native_source;
mod main_rain_context;
mod main_random_block_ticks;
mod main_world_interactions;
mod main_bucket_interactions;
use main_chunk_runtime::MainChunkRuntime;
use main_chunk_transport::{ChunkTransportUpdate, SessionChunkTransport};
use main_drop_commit::{menu_action_with_items, DropCommitCallbacks, DropCommitOwner};
use main_item_gameplay::{GameplaySourceScope, MainItemGameplay};

const PROTOCOL: i32 = 767;
const MAX_PLAYERS: usize = 20;
const CONFIGURATION_DEADLINE: Duration = Duration::from_secs(15);
const WRITE_DEADLINE: Duration = Duration::from_secs(15);
type NetworkReader = CryptoRead<OwnedReadHalf>;
type NetworkWriter = CryptoWrite<OwnedWriteHalf>;

#[derive(Clone)]
struct Options {
    bind: IpAddr,
    port: u16,
    allow_offline: bool,
    allow_public_offline: bool,
    online_mode: bool,
    fixtures: PathBuf,
    world: PathBuf,
    anvil_world: Option<PathBuf>,
    view_distance: i32,
    difficulty: i32,
    max_chained_neighbor_updates: i32,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut options = Self {
            bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 25565,
            allow_offline: false,
            allow_public_offline: false,
            online_mode: false,
            fixtures: "fixtures/configuration.json".into(),
            world: "runtime/world.json".into(),
            anvil_world: None,
            view_distance: 2,
            difficulty: 1,
            max_chained_neighbor_updates: 1_000_000,
        };
        let mut args = std::env::args().skip(1);
        while let Some(argument) = args.next() {
            if argument == "--allow-offline" {
                options.allow_offline = true;
                continue;
            }
            if argument == "--allow-public-offline" {
                options.allow_public_offline = true;
                continue;
            }
            if argument == "--online-mode" {
                options.online_mode = true;
                continue;
            }
            if argument == "--help" {
                println!("mc121-rust (--online-mode | --allow-offline [--allow-public-offline]) [--bind 127.0.0.1] [--port 25565] [--fixtures fixtures/configuration.json] [--world runtime/world.json | --anvil-world /path/to/world] [--view-distance 2..6]");
                std::process::exit(0);
            }
            let value = args
                .next()
                .ok_or_else(|| anyhow!("missing value for {argument}"))?;
            match argument.as_str() {
                "--bind" => options.bind = value.parse().context("invalid bind IP")?,
                "--port" => options.port = value.parse().context("invalid port")?,
                "--fixtures" => options.fixtures = value.into(),
                "--world" => options.world = value.into(),
                "--max-chained-neighbor-updates" => options.max_chained_neighbor_updates = value.parse().context("invalid neighbor update limit")?,
                "--anvil-world" => options.anvil_world = Some(value.into()),
                "--difficulty" => options.difficulty = match value.as_str() {
                    "peaceful"|"0"=>0,"easy"|"1"=>1,"normal"|"2"=>2,"hard"|"3"=>3,
                    _=>bail!("difficulty must be peaceful, easy, normal or hard"),
                },
                "--view-distance" => {
                    options.view_distance = value.parse().context("invalid view distance")?
                }
                _ => bail!("unknown option {argument}"),
            }
        }
        if options.allow_public_offline && !options.allow_offline {
            bail!("--allow-public-offline requires --allow-offline");
        }
        if options.allow_offline == options.online_mode {
            bail!("select exactly one login mode: --online-mode or --allow-offline");
        }
        if options.allow_offline && !options.bind.is_loopback() && !options.allow_public_offline {
            bail!("public offline access requires explicit --allow-public-offline");
        }
        if !(2..=6).contains(&options.view_distance) {
            bail!("view distance must be 2..=6");
        }
        Ok(options)
    }
}

#[derive(Deserialize)]
struct CapturedPacket {
    name: String,
    id: i32,
    payload: String,
}
#[derive(Deserialize)]
struct Capture {
    version: String,
    protocol: i32,
    complete: bool,
    packets: Vec<CapturedPacket>,
}
struct Fixtures {
    configuration: Vec<Packet>,
    dimension: i32,
    dimension_facts: DimensionFacts,
    seed_hash: i64,
}

#[derive(Clone, Copy)]
struct ClientInformation {
    view_distance: i32,
    skin_parts: u8,
    main_hand: u8,
}

async fn fixtures(path: &Path) -> Result<Fixtures> {
    if fs::metadata(path)
        .await
        .context("configuration fixture is missing; capture the exact 1.21 oracle first")?
        .len()
        > 16 * 1024 * 1024
    {
        bail!("configuration fixture is too large");
    }
    let captured: Capture = serde_json::from_slice(&fs::read(path).await?)?;
    if captured.version != "1.21" || captured.protocol != PROTOCOL || !captured.complete {
        bail!("a complete exact-version 1.21/protocol-767 capture is required");
    }
    let mut configuration = Vec::new();
    let mut registry_count = 0;
    let mut has_flags = false;
    let mut has_tags = false;
    for packet in captured.packets {
        let expected = match packet.name.as_str() {
            "registry_data" => {
                registry_count += 1;
                0x07
            }
            "feature_flags" => {
                has_flags = true;
                0x0c
            }
            "tags" => {
                has_tags = true;
                0x0d
            }
            _ => bail!("unexpected packet in immutable configuration fixture"),
        };
        if packet.id != expected {
            bail!("fixture packet ID does not match protocol-767 packet name");
        }
        let payload = STANDARD
            .decode(packet.payload)
            .context("invalid captured packet base64")?;
        if payload.len() > 2 * 1024 * 1024 {
            bail!("fixture packet exceeds wire limit");
        }
        configuration.push(Packet {
            id: packet.id,
            payload,
        });
    }
    if registry_count == 0 || !has_flags || !has_tags {
        bail!("configuration fixture lacks registries, feature flags or tags");
    }
    let join_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("join.json");
    let join: serde_json::Value = serde_json::from_slice(
        &fs::read(join_path)
            .await
            .context("captured join.json is required")?,
    )?;
    let dimension =
        join.pointer("/worldState/dimension")
            .and_then(|v| v.as_i64())
            .filter(|value| (0..=1024).contains(value))
            .ok_or_else(|| anyhow!("captured join has invalid dimension ID"))? as i32;
    if join.pointer("/worldState/name").and_then(|v| v.as_str()) != Some("minecraft:overworld") {
        bail!("the creative fixture requires an overworld oracle capture");
    }
    let dimension_entries = configuration
        .iter()
        .filter(|packet| packet.id == 0x07)
        .find_map(|packet| {
            let mut reader = Reader::new(&packet.payload);
            match reader.string(32767) {
                Ok(name) if name == "minecraft:dimension_type" => {
                    Some(world_context::decode_dimension_registry(&packet.payload))
                }
                _ => None,
            }
        })
        .context("captured dimension registry is missing")??;
    let dimension_facts = dimension_entries
        .get(dimension as usize)
        .context("join dimension ID is absent from captured registry")?
        .facts
        .context("join dimension has no captured serialized facts")?;
    dimension_facts.require_runtime_layout()?;
    let seed_parts = join
        .pointer("/worldState/hashedSeed")
        .and_then(|value| value.as_array())
        .filter(|parts| parts.len() == 2)
        .context("captured join has no seed hash pair")?;
    let high = i32::try_from(
        seed_parts[0]
            .as_i64()
            .context("invalid seed hash high word")?,
    )?;
    let low = i32::try_from(
        seed_parts[1]
            .as_i64()
            .context("invalid seed hash low word")?,
    )?;
    let seed_hash = ((u64::from(high as u32) << 32) | u64::from(low as u32)) as i64;
    Ok(Fixtures {
        configuration,
        dimension,
        dimension_facts,
        seed_hash,
    })
}

#[derive(Serialize, Deserialize)]
struct SavedWorld {
    format: String,
    minecraft: String,
    changes: Vec<BlockChange>,
    #[serde(default)]
    clock: Option<TickClock>,
    #[serde(default)]
    weather: Option<WeatherState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fluid_ticks: Option<main_fluid_simulation::MainFluidCheckpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fluid_shapes: Option<mc121_rust::fluid_shape_checkpoint::SavedFluidShapeCache>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fire_ticks: Option<main_fire_simulation::SavedFireRuntime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native_block_entities: Option<mc121_rust::native_block_entities::SavedNativeBlockEntities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    block_ticks: Option<main_block_ticks::SavedBlockRuntime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    level_random_position_value: Option<i32>,
}
struct Server {
    options: Options,
    fixtures: Fixtures,
    seed_hash: i64,
    world: RwLock<WorldBackend>,
    save_lock: Mutex<()>,
    player_admission: Mutex<()>,
    online: Arc<AtomicUsize>,
    entities: AtomicI32,
    connection_sequence: AtomicU64,
    changes: broadcast::Sender<WorldUpdate>,
    authenticator: Option<Authenticator>,
    session_verifier: Option<SessionVerifier>,
    players: StdRwLock<LivePlayers>,
    entity_events: broadcast::Sender<PlayerEvent>,
    clock: StdRwLock<TickClock>,
    tick_events: watch::Sender<i64>,
    epoch: Instant,
    level: Option<Mutex<LevelData>>,
    border: StdRwLock<WorldBorder>,
    player_store: PlayerDataStore,
    entity_store: EntityDataStore,
    shutting_down: AtomicBool,
}

struct LivePlayers {
    registry: EntityRegistry,
    revision: u64,
    motion: BTreeMap<i32, PlayerMotionTracker>,
    player_motion_runtime: main_player_motion_hooks::MainPlayerMotionRuntime,
    player_food_runtime: main_player_food::MainPlayerFoodRuntime,
    item_use_runtime: main_item_use::MainItemUseRuntime,
    math_random: Option<mc121_rust::item_use::JdkMathRandom>,
    native_body_ready: BTreeSet<i32>,
    native_containers: main_block_entities::NativeContainerRuntime,
    crafting_tables: main_crafting_table::CraftingTableRuntime,
    block_runtime: main_block_ticks::MainBlockRuntime,
    redstone_runtime: main_redstone::MainRedstoneRuntime,
    containers: BTreeMap<i32, SharedContainer>,
    disconnects: BTreeMap<i32, watch::Sender<Option<&'static str>>>,
    pending_block_acks: BTreeMap<i32, i32>,
    flying: BTreeSet<i32>,
    unavailable_pose: BTreeMap<i32, String>,
    pending_metadata: BTreeMap<i32, PlayerMetadata>,
    item_runtime: RuntimeEntities,
    world_context: WorldContext,
    native_difficulty: mc121_rust::survival::Difficulty,
    rain_context: Option<mc121_rust::rain_physics::FreshFlatPlainsRain>,
    world_revision: u64,
    chunk_runtime: MainChunkRuntime,
    gameplay: MainItemGameplay,
    connection_admissions: BTreeMap<u64, i32>,
    player_chunks: BTreeMap<i32, (i32, i32)>,
    // Retained post-constructor stream, including Entity UUID draws.
    player_random: BTreeMap<i32, LegacyRandomSource>,
    creative_drop: BTreeMap<i32, CreativeDropGate>,
    confirmed_chunk_callbacks: Vec<((i32, i32), FullStatus)>,
    chunk_transports: BTreeMap<i32, SessionChunkTransport>,
    observers: BTreeMap<i32, TrackingObserver>,
    recipe_books: BTreeMap<[u8; 16], ServerRecipeBook>,
    player_lifecycle: BTreeMap<[u8; 16], PlayerLifecycle>,
    pending_player_saves: BTreeMap<[u8; 16], PlayerData>,
    entity_source: Option<NativeEntitySource>,
    world_interactions: main_world_interactions::WorldInteractionRuntime,
    fluid_runtime: main_fluid_simulation::MainFluidRuntime,
    fire_runtime: main_fire_simulation::MainFireRuntime,
    random_block_ticks: main_random_block_ticks::MainRandomBlockTicks,
    world_interaction_issue: Option<String>,
    level_random_position_value: Option<i32>,
}

struct LiveSnapshot {
    player_revision: u64,
    players: Vec<PlayerState>,
    player_baselines: BTreeMap<i32, Position>,
    item_revision: u64,
    items: Vec<EntitySnapshot<ItemState>>,
}

struct WorldSnapshot {
    revision: u64,
    time_payload: [u8; 16],
    weather: WeatherState,
}
fn weather_packet(reason: u8, value: f32) -> Packet {
    Packet {
        id: 0x22,
        payload: encoded(|writer| {
            writer.u8(reason).f32(value);
        }),
    }
}
impl WorldSnapshot {
    fn weather_packets(&self, recovery: bool) -> Vec<Packet> {
        if !recovery && !self.weather.is_raining() {
            return vec![];
        }
        vec![
            weather_packet(if self.weather.is_raining() { 1 } else { 2 }, 0.),
            weather_packet(7, self.weather.rain_level_at(1.)),
            weather_packet(8, self.weather.thunder_level_at(1.)),
        ]
    }
    #[cfg(test)]
    fn recovery_packets(&self) -> Vec<Packet> {
        let mut packets = vec![Packet {
            id: 0x64,
            payload: self.time_payload.to_vec(),
        }];
        packets.extend(self.weather_packets(true));
        packets
    }
}
struct ObserverReconcile {
    observer: TrackingObserver,
    snapshot: LiveSnapshot,
}

struct ClientTrackers {
    players: PlayerTracker,
    items: GenericEntityTracker<ItemState>,
    item_effect_revision: Option<u64>,
    world_revision: Option<u64>,
}
impl ClientTrackers {
    fn new(entity_id: i32) -> Self {
        Self {
            players: PlayerTracker::new_scheduled(entity_id),
            items: GenericEntityTracker::new(),
            item_effect_revision: None,
            world_revision: None,
        }
    }
    fn set_observer(&mut self, observer: TrackingObserver) -> Result<()> {
        self.players.set_observer(observer)?;
        self.items.set_observer(observer)
    }
    fn sync_snapshot(
        &mut self,
        snapshot: &LiveSnapshot,
        loaded: &BTreeSet<(i32, i32)>,
    ) -> Result<Vec<Packet>> {
        for (&id, &position) in &snapshot.player_baselines {
            self.players
                .set_spawn_position(snapshot.player_revision, id, position)?;
        }
        let mut packets = self
            .players
            .sync(snapshot.player_revision, &snapshot.players, loaded)?;
        packets.extend(
            self.items
                .sync(snapshot.item_revision, &snapshot.items, loaded)?,
        );
        Ok(packets)
    }
    /// Apply only a producer-captured frame received at its ordered FIFO point.
    fn apply_reconcile(
        &mut self,
        reconcile: &ObserverReconcile,
        loaded: &BTreeSet<(i32, i32)>,
    ) -> Result<Vec<Packet>> {
        self.set_observer(reconcile.observer)?;
        self.sync_snapshot(&reconcile.snapshot, loaded)
    }
    fn world_event(&mut self, revision: u64, packets: &[Packet]) -> Vec<Packet> {
        if self.world_revision.is_some_and(|seen| revision <= seen) {
            return vec![];
        }
        self.world_revision = Some(revision);
        packets.to_vec()
    }
    fn item_event(
        &mut self,
        observer_id: i32,
        event: &RuntimeEntityEvent,
        loaded: &BTreeSet<(i32, i32)>,
    ) -> Result<Vec<Packet>> {
        match event {
            RuntimeEntityEvent::Update {
                revision,
                snapshot,
                packets,
            } => self.items.update(*revision, snapshot, packets, loaded),
            RuntimeEntityEvent::Removed {
                revision,
                entity_id,
            } => self.items.remove(*revision, *entity_id),
            RuntimeEntityEvent::Collected { revision, pickup } => {
                Ok(self
                    .items
                    .transient(*revision, pickup.item_entity_id, &[pickup.packet()]))
            }
            RuntimeEntityEvent::Effect {
                revision,
                deliveries,
            } => {
                if self
                    .item_effect_revision
                    .is_some_and(|seen| *revision <= seen)
                {
                    return Ok(vec![]);
                }
                self.item_effect_revision = Some(*revision);
                Ok(deliveries
                    .iter()
                    .filter(|delivery| delivery.recipients.contains(&observer_id))
                    .map(|delivery| delivery.packet.clone())
                    .collect())
            }
        }
    }
}

#[derive(Clone)]
enum PlayerEvent {
    ChunksReady {
        recipient: i32,
        cutoff: u64,
        reconcile: Option<Arc<ObserverReconcile>>,
    },
    ObserverReconcile {
        recipient: i32,
        reconcile: Arc<ObserverReconcile>,
    },
    World {
        revision: u64,
        packets: Arc<Vec<Packet>>,
    },
    WorldAudience {
        recipients: Arc<Vec<i32>>,
        packets: Arc<Vec<Packet>>,
    },
    Item(Arc<RuntimeEntityEvent>),
    State(Arc<EntityUpdate>),
    Latency(Vec<PlayerState>),
    Container {
        recipient: i32,
        update: Arc<ContainerUpdate>,
    },
    Motion {
        revision: u64,
        entity_id: i32,
        position: Position,
        packets: Vec<Packet>,
    },
}

/// Lost ordered history is terminal: no fresh snapshot may invent its effects.
enum OrderedEntityInput {
    Event(PlayerEvent),
    DisconnectLostHistory { skipped: u64 },
    Closed,
}
fn ordered_entity_input(
    result: std::result::Result<PlayerEvent, broadcast::error::RecvError>,
) -> OrderedEntityInput {
    match result {
        Ok(event) => OrderedEntityInput::Event(event),
        Err(broadcast::error::RecvError::Lagged(skipped)) => {
            OrderedEntityInput::DisconnectLostHistory { skipped }
        }
        Err(broadcast::error::RecvError::Closed) => OrderedEntityInput::Closed,
    }
}

impl LivePlayers {
    fn new(
        world_random: LegacyRandomSource,
        sound_random: LegacyRandomSource,
        world_context: WorldContext,
        simulation_distance: i32,
        fresh_unedited: bool,
        sender: broadcast::Sender<PlayerEvent>,
    ) -> Self {
        let item_runtime = RuntimeEntities::new(world_random)
            .with_sound_random(sound_random)
            .with_publisher(Arc::new(move |event| {
                let _ = sender.send(PlayerEvent::Item(Arc::new(event)));
            }));
        Self {
            registry: EntityRegistry::default(),
            revision: 0,
            motion: BTreeMap::new(),
            player_motion_runtime: main_player_motion_hooks::MainPlayerMotionRuntime::new(),
            player_food_runtime: main_player_food::MainPlayerFoodRuntime::default(),
            item_use_runtime: main_item_use::MainItemUseRuntime::default(),
            math_random: None,
            native_body_ready: BTreeSet::new(),
            native_containers: main_block_entities::NativeContainerRuntime::default(),
            crafting_tables: main_crafting_table::CraftingTableRuntime::default(),
            block_runtime: main_block_ticks::MainBlockRuntime::new(fresh_unedited),
            redstone_runtime: main_redstone::MainRedstoneRuntime::new(fresh_unedited),
            containers: BTreeMap::new(),
            disconnects: BTreeMap::new(),
            pending_block_acks: BTreeMap::new(),
            flying: BTreeSet::new(),
            unavailable_pose: BTreeMap::new(),
            pending_metadata: BTreeMap::new(),
            item_runtime,
            world_context,
            native_difficulty: mc121_rust::survival::Difficulty::Easy,
            rain_context: fresh_unedited.then(
                mc121_rust::rain_physics::FreshFlatPlainsRain::from_observed_empty_temperature_history,
            ),
            world_revision: 0,
            chunk_runtime: MainChunkRuntime::new(
                simulation_distance,
                simulation_distance,
                fresh_unedited,
            ),
            gameplay: MainItemGameplay::new(if fresh_unedited {
                GameplaySourceScope::NativeFixtureEmptyCriteria
            } else {
                GameplaySourceScope::UnknownImported
            }),
            connection_admissions: BTreeMap::new(),
            player_chunks: BTreeMap::new(),
            player_random: BTreeMap::new(),
            creative_drop: BTreeMap::new(),
            confirmed_chunk_callbacks: vec![],
            chunk_transports: BTreeMap::new(),
            observers: BTreeMap::new(),
            recipe_books: BTreeMap::new(),
            player_lifecycle: BTreeMap::new(),
            pending_player_saves: BTreeMap::new(),
            entity_source: None,
            world_interactions: main_world_interactions::WorldInteractionRuntime::new(fresh_unedited),
            fluid_runtime: main_fluid_simulation::MainFluidRuntime::default(),
            fire_runtime: main_fire_simulation::MainFireRuntime::new(1).expect("native Easy difficulty ID is valid"),
            random_block_ticks: main_random_block_ticks::MainRandomBlockTicks::new(fresh_unedited),
            world_interaction_issue: None,
            level_random_position_value: None,
        }
    }
    fn replication_snapshot(&self) -> Result<LiveSnapshot> {
        let (item_revision, items) = self.item_runtime.snapshots()?;
        Ok(LiveSnapshot {
            player_revision: self.revision,
            players: self.registry.snapshot(),
            player_baselines: self
                .motion
                .iter()
                .map(|(id, motion)| (*id, motion.spawn_position()))
                .collect(),
            item_revision,
            items,
        })
    }
    fn register_runtime_player(
        &mut self,
        id: i32,
        position: Position,
        admission: u64,
        random: LegacyRandomSource,
        uuid: [u8; 16],
    ) -> Result<()> {
        self.gameplay.register_player(id, uuid)?;
        self.item_runtime
            .register_player(id, Vec3::new(position.x, position.y, position.z))?;
        let chunk = (
            (position.x.floor() as i32).div_euclid(16),
            (position.z.floor() as i32).div_euclid(16),
        );
        self.player_chunks.insert(id, chunk);
        self.chunk_runtime.manager.player_add(id, chunk, false)?;
        self.connection_admissions.insert(admission, id);
        self.player_random.insert(id, random);
        self.creative_drop.insert(id, CreativeDropGate::default());
        self.chunk_transports
            .insert(id, SessionChunkTransport::new());
        Ok(())
    }
    fn remove_runtime_player(&mut self, id: i32) -> Result<()> {
        self.player_motion_runtime.remove(id);
        self.player_food_runtime.remove(id);
        self.item_use_runtime.remove(id);
        self.native_body_ready.remove(&id);
        self.item_runtime.remove_player(id)?;
        self.gameplay.remove_player(id);
        if let Some(chunk) = self.player_chunks.remove(&id) {
            self.chunk_runtime.manager.player_remove(id, chunk)?;
        }
        self.connection_admissions.retain(|_, player| *player != id);
        self.chunk_transports.remove(&id);
        self.observers.remove(&id);
        self.player_random.remove(&id);
        self.creative_drop.remove(&id);
        Ok(())
    }
    fn apply_confirmed_chunk_status(
        &mut self,
        chunk: (i32, i32),
        status: FullStatus,
    ) -> Result<()> {
        let visibility = match status {
            FullStatus::Inaccessible => Visibility::Hidden,
            FullStatus::Full | FullStatus::BlockTicking => Visibility::Tracked,
            FullStatus::EntityTicking => Visibility::Ticking,
        };
        // This callback is literal, including an already queued stale promotion.
        // Ticket levels and network chunks never substitute for confirmation.
        self.item_runtime.set_chunk_visibility(chunk, visibility)
    }
    fn observer_for(
        &self,
        id: i32,
        position: Position,
        requested_view_distance: i32,
        server_view_distance: i32,
    ) -> Result<TrackingObserver> {
        let player = self.registry.get(id).context("absent tracking observer")?;
        Ok(TrackingObserver {
            entity_id: id,
            position: Vec3::new(position.x, position.y, position.z),
            requested_view_distance,
            server_view_distance,
            is_spectator: player.gamemode == 3,
            tracking_distance_percent: 100,
        })
    }
    fn capture_reconcile(&self, id: i32) -> Result<Arc<ObserverReconcile>> {
        Ok(Arc::new(ObserverReconcile {
            observer: *self.observers.get(&id).context("observer fact missing")?,
            snapshot: self.replication_snapshot()?,
        }))
    }
    fn publish_reconcile(&self, id: i32, sender: &broadcast::Sender<PlayerEvent>) -> Result<()> {
        let reconcile = self.capture_reconcile(id)?;
        let _ = sender.send(PlayerEvent::ObserverReconcile {
            recipient: id,
            reconcile,
        });
        Ok(())
    }
    fn queue_chunk_view(
        &mut self,
        id: i32,
        center: (i32, i32),
        radius: i32,
        send_center: bool,
        sender: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        let alive = match self.registry.get(id) {
            Some(player) => match player.entity_data.get(9) {
                Some(MetadataValue::Float(health)) => *health > 0.,
                _ => bail!("player health data unavailable"),
            },
            None => return Ok(()),
        };
        let Some(transport) = self.chunk_transports.get_mut(&id) else {
            return Ok(());
        };
        let view = entity_tracking::ChunkView {
            center,
            view_distance: radius,
        };
        if self.chunk_runtime.require_supported_lighting().is_ok() {
            transport.queue_view_ready(view, send_center, alive, |chunk| {
                self.chunk_runtime
                    .manager
                    .current_chunk_to_send_ready(chunk)
            })?;
        } else {
            transport.queue_view(view, send_center, alive)?;
        }
        let _ = sender.send(PlayerEvent::ChunksReady {
            recipient: id,
            cutoff: transport.latest_revision(),
            reconcile: None,
        });
        Ok(())
    }
    fn publish_world(
        &mut self,
        packets: Vec<Packet>,
        sender: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        if packets.is_empty() {
            return Ok(());
        }
        self.world_revision = self
            .world_revision
            .checked_add(1)
            .context("world revision exhausted")?;
        let _ = sender.send(PlayerEvent::World {
            revision: self.world_revision,
            packets: Arc::new(packets),
        });
        Ok(())
    }
    fn publish(&mut self, event: EntityEvent, sender: &broadcast::Sender<PlayerEvent>) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("entity revision exhausted");
        let _ = sender.send(PlayerEvent::State(Arc::new(EntityUpdate {
            revision: self.revision,
            event,
        })));
    }
}

#[derive(Debug)]
struct ItemRuntimeNotReady;
impl std::fmt::Display for ItemRuntimeNotReady {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "item runtime requires confirmed chunk lifecycle, world facts and gameplay hooks",
        )
    }
}
impl std::error::Error for ItemRuntimeNotReady {}

fn menu_without_world_drops(
    container: &SharedContainer,
    operation: impl FnOnce(&mut PlayerContainer) -> Result<ClickOutcome>,
) -> Result<()> {
    container.transact(|menu| {
        let outcome = operation(menu)?;
        if !outcome.drops.is_empty() {
            return Err(ItemRuntimeNotReady.into());
        }
        menu.take_local_slot_changes();
        Ok(((), outcome.packets))
    })
}
async fn finish_menu_action(
    result: Result<()>,
    container: &SharedContainer,
    codec: &FrameCodec,
    writer: &mut NetworkWriter,
    revision: &mut u64,
) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.downcast_ref::<ItemRuntimeNotReady>().is_some() => {
            // The staged menu was rolled back before any ID/RNG/world effect.
            // Repair the client's prediction without inventing a dropped item.
            let snapshot = container.snapshot()?;
            *revision = snapshot.revision;
            send_packets(codec, writer, vec![snapshot.packet]).await
        }
        Err(error) => Err(error),
    }
}

impl Server {
    fn capture_player_locked(
        &self,
        players: &mut LivePlayers,
        id: i32,
    ) -> Result<Option<PlayerData>> {
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(None);
        };
        let uuid = player.profile.uuid;
        let inventory = players
            .containers
            .get(&id)
            .context("saving player has no container")?
            .read(|menu| menu.inventory.clone())?;
        let inventory=players.native_containers.inventory_snapshot(id,&inventory);
        let inventory=players.crafting_tables.inventory_snapshot(id,&inventory);
        let stats = players.gameplay.snapshot_player_stats(uuid)?;
        let mut lifecycle = players
            .player_lifecycle
            .get(&uuid)
            .cloned()
            .context("saving player lifecycle missing")?;
        lifecycle.abilities.flying = players.flying.contains(&id);
        if let Some(velocity)=players.player_motion_runtime.velocity(id) { lifecycle.motion_velocity=Some(velocity); }
        if let Some(distance) = players.player_motion_runtime.fall_distance(id) {
            lifecycle.fall_distance = distance;
        }
        if let Some(book) = players.recipe_books.get(&uuid) {
            lifecycle.recipe_book_nbt = Some(main_player_data::save_recipe_book(book)?);
        }
        Ok(Some(PlayerData::capture(
            &player, &inventory, &stats, lifecycle,
        )?))
    }
    fn persist_player_data(&self, data: &PlayerData) -> Result<()> {
        self.player_store.save(data)?;
        if let Some(root)=&self.options.anvil_world {
            let uuid=mc121_rust::player_data::uuid_string(data.uuid);
            let path=root.join("playerdata").join(format!("{uuid}.dat"));
            let stats=root.join("stats").join(format!("{uuid}.json"));
            mc121_rust::player_nbt::save_player_file(&path,data,Some(&stats))?;
        }
        Ok(())
    }
    fn save_player_locked(&self, players: &mut LivePlayers, id: i32) -> Result<()> {
        let Some(data) = self.capture_player_locked(players, id)? else {
            return Ok(());
        };
        let uuid = data.uuid;
        // Retain the newest authoritative snapshot if disk I/O fails. A later
        // autosave or rejoin retries this owner rather than reading stale disk.
        players.pending_player_saves.insert(uuid, data);
        self.persist_player_data(players.pending_player_saves.get(&uuid).unwrap())?;
        players.pending_player_saves.remove(&uuid);
        Ok(())
    }
    fn allocate_player_entity(&self) -> Result<(i32, LegacyRandomSource)> {
        // Drop ID ranges are staged and committed under this same lock.
        let _players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let id = self
            .entities
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| anyhow!("entity ID space exhausted"))?;
        let random = random_seed::create_player_random()?;
        Ok((id, random))
    }
    async fn poll_chunk_runtime(&self) -> Result<()> {
        let game_time = self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?
            .game_time;
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        // Unknown imported lighting/source history leaves the earlier transport
        // slice active; it cannot manufacture a world-simulation capability.
        if !players.chunk_runtime.has_native_lighting_owner() { return Ok(()); }
        let light_updates = players.chunk_runtime.settle_lighting(&world)?;
        for update in light_updates { let _ = self.changes.send(update); }
        let LivePlayers {
            chunk_runtime,
            item_runtime,
            chunk_transports,
            ..
        } = &mut *players;
        let summary = chunk_runtime.poll(
            &mut world,
            item_runtime,
            chunk_transports,
            &self.entity_events,
            game_time,
        )?;
        for update in chunk_runtime.take_lighting_updates() { let _ = self.changes.send(update); }
        if !summary.quiescent || !summary.diagnostics.is_empty() {
            eprintln!(
                "{}",
                json!({"event":"chunk_runtime_progress","quiescent":summary.quiescent,"turns":summary.turns,"diagnostics":summary.diagnostics})
            );
        }
        Ok(())
    }
    async fn execute_menu_action(
        &self,
        id: i32,
        container: &SharedContainer,
        operation: impl FnOnce(&mut PlayerContainer) -> Result<ClickOutcome>,
        creative_attempt: Option<(i16, ItemStack)>,
    ) -> Result<()> {
        let game_time = self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?
            .game_time;
        let border_bounds = self
            .border
            .read()
            .map_err(|_| anyhow!("world border lock poisoned"))?
            .bounds(self.epoch.elapsed().as_millis() as i64);
        let border = CollisionBorder {
            min_x: border_bounds.min_x,
            min_z: border_bounds.min_z,
            max_x: border_bounds.max_x,
            max_z: border_bounds.max_z,
        };
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        ensure!(
            !self.shutting_down.load(Ordering::Acquire),
            "server is saving for shutdown"
        );
        let Some(player) = players.registry.get(id) else {
            return Ok(());
        };
        let (uuid, position, pose) = (player.profile.uuid, player.position, player.metadata.pose);
        let mut staged_gate = None;
        if let Some((slot, stack)) = &creative_attempt {
            stack.validate_creative()?;
            let mut gate = *players
                .creative_drop
                .get(&id)
                .context("creative gate missing")?;
            if !gate.allow_negative_slot_drop(*slot, stack)? {
                return Ok(());
            }
            if stack.is_empty() {
                *players.creative_drop.get_mut(&id).unwrap() = gate;
                return Ok(());
            }
            staged_gate = Some(gate);
        }
        if players.gameplay.require_supported_floor().is_err()
            || players.chunk_runtime.require_supported_lighting().is_err()
            || players.require_current_native_source(&world).is_err()
        {
            if creative_attempt.is_some() {
                return Err(ItemRuntimeNotReady.into());
            }
            return menu_without_world_drops(container, operation);
        }
        let facts = players.world_context.item_facts(game_time, border, false)?;
        let rain_available =
            matches!(facts.rain, RainAvailability::NotRaining) || players.rain_context.is_some();
        let dimensions = player_physics::player_dimensions(pose, 1.)?;
        let drop = PlayerDrop {
            player_uuid: uuid,
            position,
            eye_height: dimensions.eye_height,
            around: false,
            trace_item: false,
        };
        let floor = mc121_rust::entity_state::java_floor;
        let min = (
            floor(position.x - 18.).div_euclid(16),
            floor(position.z - 18.).div_euclid(16),
        );
        let max = (
            floor(position.x + 18.).div_euclid(16),
            floor(position.z + 18.).div_euclid(16),
        );
        let staged_stats = players.gameplay.stats_mut(uuid)?.clone();
        let LivePlayers {
            item_runtime,
            player_random,
            gameplay,
            chunk_runtime,
            chunk_transports,
            creative_drop,
            ..
        } = &mut *players;
        let random = player_random
            .get_mut(&id)
            .context("retained player random missing")?;
        let gate = creative_drop
            .get_mut(&id)
            .context("creative gate missing")?;
        menu_action_with_items(
            container,
            DropCommitOwner {
                runtime: item_runtime,
                player_random: random,
                stats: staged_stats,
                entity_counter: &self.entities,
            },
            drop,
            move |menu| {
                if let Some((_, stack)) = creative_attempt {
                    Ok(ClickOutcome {
                        drops: vec![mc121_rust::container::DroppedStack {
                            stack,
                            around: false,
                            trace_item: true,
                        }],
                        ..ClickOutcome::default()
                    })
                } else {
                    operation(menu)
                }
            },
            |_| {
                let readiness = (|| {
                    ensure!(rain_available, "item rain exposure provider unavailable");
                    ensure!(
                        (64.0..=319.0).contains(&position.y),
                        "drop is outside the supported ordinary-floor height slice"
                    );
                    chunk_runtime.certify_query_region(
                        &mut world,
                        chunk_transports,
                        &self.entity_events,
                        game_time,
                        min,
                        max,
                    )
                })();
                readiness.map_err(|error| {
                    anyhow::Error::new(ItemRuntimeNotReady).context(error.to_string())
                })
            },
            || random_seed::create_random().map_err(anyhow::Error::from),
            DropCommitCallbacks {
                prepare: move |menu: &mut PlayerContainer, next_stats| {
                    let gameplay = gameplay;
                    let changes = menu.take_local_slot_changes();
                    gameplay.prepare_menu_commit(uuid, &menu.inventory, changes, next_stats)
                },
                before_spawn: move || {
                    if let Some(next) = staged_gate {
                        *gate = next;
                    }
                },
                commit: |prepared: main_item_gameplay::PreparedGameplayMenu<'_>| prepared.commit(),
            },
        )
    }
    async fn drain_world_interaction_phase(&self) -> Result<()> {
        let now = self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?
            .game_time;
        let bounds = self
            .border
            .read()
            .map_err(|_| anyhow!("world border lock poisoned"))?
            .bounds(self.epoch.elapsed().as_millis() as i64);
        let border = CollisionBorder {
            min_x: bounds.min_x,
            min_z: bounds.min_z,
            max_x: bounds.max_x,
            max_z: bounds.max_z,
        };
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        // fixtures() verifies the selected join world key is minecraft:overworld.
        let report = players.drain_world_interactions(
            &mut world,
            now,
            border,
            true,
            &self.entity_events,
            1024,
        );
        if report.blocked != players.world_interaction_issue {
            if let Some(reason) = &report.blocked {
                eprintln!(
                    "{}",
                    json!({"event":"world_interaction_pending","reason":reason,"pending":report.pending,"processed":report.processed})
                );
            }
            players.world_interaction_issue = report.blocked;
        }
        Ok(())
    }
    async fn tick_item_world(&self) -> Result<()> {
        let game_time = self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?
            .game_time;
        let bounds = self
            .border
            .read()
            .map_err(|_| anyhow!("world border lock poisoned"))?
            .bounds(self.epoch.elapsed().as_millis() as i64);
        let border = CollisionBorder {
            min_x: bounds.min_x,
            min_z: bounds.min_z,
            max_x: bounds.max_x,
            max_z: bounds.max_z,
        };
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        players.simulate_item_world(&mut world, game_time, border, &self.entity_events)
    }

    async fn tick_connection_phase(&self) -> Result<()> {
        let game_time=self.clock.read().map_err(|_|anyhow!("clock poisoned"))?.game_time;
        let bounds=self.border.read().map_err(|_|anyhow!("border poisoned"))?.bounds(self.epoch.elapsed().as_millis()as i64);
        let border=CollisionBorder{min_x:bounds.min_x,min_z:bounds.min_z,max_x:bounds.max_x,max_z:bounds.max_z};
        let mut world=self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let admission_order = players
            .connection_admissions
            .values()
            .copied()
            .collect::<Vec<_>>();
        for id in admission_order {
            if players.player_motion_runtime.contains(id) {
                if players.native_body_ready.contains(&id) {
                    if let Err(error)=players.tick_native_player_body(id,&mut world,game_time,border,&self.entity_events) {
                        eprintln!("{}",json!({"event":"native_player_body_failed","entity_id":id,"reason":format!("{error:#}")}));
                    }
                } else {
                    players.advance_native_player_listener_clock(id,&self.entity_events)?;
                }
                if let Some(gate)=players.creative_drop.get_mut(&id){gate.tick();}
                continue;
            }
            self::LivePlayers::begin_native_food_slice(&mut players,id,&self.entity_events)?;
            // This supported item slice executes after levels in connection
            // admission order. It does not stand in for complete Player.aiStep.
            if players.item_runtime.item_count() != 0 {
                players.gameplay.connection_item_phase(id)?;
                let player = players
                    .registry
                    .get(id)
                    .context("connection player missing")?;
                let dimensions = player_physics::player_dimensions(player.metadata.pose, 1.)?;
                let alive = match player.entity_data.get(9) {
                    Some(MetadataValue::Float(health)) => *health > 0.,
                    _ => bail!("player health data unavailable"),
                };
                let pickup = PickupPlayer {
                    entity_id: id,
                    uuid: player.profile.uuid,
                    bounds: Aabb::player(
                        Vec3::new(player.position.x, player.position.y, player.position.z),
                        dimensions.width,
                        dimensions.height,
                    ),
                    alive,
                    spectator: player.gamemode == 3,
                    vehicle_bounds: None,
                };
                let LivePlayers {
                    item_runtime,
                    registry,
                    containers,
                    gameplay,
                    chunk_runtime,
                    ..
                } = &mut *players;
                let mut frame = gameplay.frame(registry, &chunk_runtime.manager, true);
                let mut host = RuntimeInventoryHost::new(registry, containers, &mut frame);
                item_runtime.player_touch(pickup, &mut host)?;
            }
            self::LivePlayers::tick_native_food_slice(&mut players,id,&self.entity_events)?;
            if let Some(gate) = players.creative_drop.get_mut(&id) {
                gate.tick();
            }
        }
        players.gameplay.drain_observations();
        Ok(())
    }
    fn queue_chunk_view(
        &self,
        id: i32,
        center: (i32, i32),
        radius: i32,
        send_center: bool,
    ) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        players.queue_chunk_view(id, center, radius, send_center, &self.entity_events)?;
        players.publish_reconcile(id, &self.entity_events)
    }
    /// Accept source state and its immutable observer transition in one lock.
    fn move_and_reconcile(
        &self,
        id: i32,
        position: Position,
        requested: i32,
        view: Option<((i32, i32), i32, bool)>,
    ) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        Self::move_player_locked(&mut players, id, position, &self.entity_events)?;
        if players.registry.get(id).is_none() {
            return Ok(());
        }
        let observer = players.observer_for(id, position, requested, self.options.view_distance)?;
        players.observers.insert(id, observer);
        if let Some((center, radius, send_center)) = view {
            players.queue_chunk_view(id, center, radius, send_center, &self.entity_events)?;
        }
        players.publish_reconcile(id, &self.entity_events)
    }
    fn acknowledge_chunk_batch(&self, id: i32, desired: f32) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        if let Some(transport) = players.chunk_transports.get_mut(&id) {
            transport.on_ack(desired);
        }
        Ok(())
    }
    fn drain_chunk_transport(&self, id: i32, cutoff: u64) -> Result<Vec<ChunkTransportUpdate>> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        Ok(players
            .chunk_transports
            .get_mut(&id)
            .map(|transport| transport.drain_through(cutoff))
            .unwrap_or_default())
    }
    async fn send_chunk_phase(&self) -> Result<()> {
        // Source MinecraftServer.tickChildren sends after connection/player-list
        // phases. Actual transport stays in a retained per-session outbox; a
        // broadcast notification may be lost without losing its chunk batch.
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let order = players
            .connection_admissions
            .values()
            .copied()
            .collect::<Vec<_>>();
        for id in order {
            let outcome = {
                let LivePlayers {
                    registry,
                    chunk_runtime,
                    chunk_transports,
                    ..
                } = &mut *players;
                let Some(player) = registry.get(id) else {
                    continue;
                };
                let center = (
                    (player.position.x.floor() as i32).div_euclid(16),
                    (player.position.z.floor() as i32).div_euclid(16),
                );
                let Some(transport) = chunk_transports.get_mut(&id) else {
                    continue;
                };
                if chunk_runtime.has_native_lighting_owner() {
                    let (light, manager) = chunk_runtime.native_lighting_parts()?;
                    transport.send_next_with_lighting(&mut world, Some(light), center, |chunk| manager.current_chunk_to_send_ready(chunk))
                } else {
                    transport.send_next(&mut world, center, |_| Ok(true))
                }
            };
            match outcome {
                Ok(0) => {}
                Ok(_) => {
                    let cutoff = players.chunk_transports[&id].latest_revision();
                    let reconcile = players.capture_reconcile(id)?;
                    let _ = self.entity_events.send(PlayerEvent::ChunksReady {
                        recipient: id,
                        cutoff,
                        reconcile: Some(reconcile),
                    });
                }
                Err(error) => {
                    eprintln!(
                        "{}",
                        json!({"event":"chunk_transport_unavailable","entity_id":id,"reason":format!("{error:#}")})
                    );
                    if let Some(disconnect) = players.disconnects.get(&id) {
                        disconnect.send_replace(Some("disconnect.disconnected"));
                    }
                }
            }
        }
        Ok(())
    }
    fn stats_packet(&self, id: i32) -> Result<Option<Packet>> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let Some(player) = players.registry.get(id) else {
            return Ok(None);
        };
        let uuid = player.profile.uuid;
        // The earlier imported-world checkpoint has no certified stat-file
        // source. Preserve that explicit unsupported path; never invent zero.
        if !players.gameplay.native_stats_owned(uuid)
            && players.gameplay.require_supported_floor().is_err()
        {
            return Ok(None);
        }
        Ok(Some(players.gameplay.stats_mut(uuid)?.send_stats_packet()?))
    }
    fn recipe_login_packets(&self, id: i32) -> Result<Vec<Packet>> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let uuid = players
            .registry
            .get(id)
            .context("recipe player missing")?
            .profile
            .uuid;
        let book = players.recipe_books.entry(uuid).or_default();
        Ok(vec![
            VanillaRecipeCatalogue::get().declarations_packet(),
            book.initial_packet().encode()?,
        ])
    }
    fn recipe_settings(&self, id: i32, payload: &[u8]) -> Result<()> {
        let change = RecipeBookSettingsChange::decode_exact(payload)?;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let uuid = players
            .registry
            .get(id)
            .context("recipe player missing")?
            .profile
            .uuid;
        players
            .recipe_books
            .entry(uuid)
            .or_default()
            .apply_settings_change(change);
        Ok(())
    }
    fn recipe_seen(&self, id: i32, payload: &[u8]) -> Result<()> {
        let seen = RecipeBookSeenRecipe::decode_exact(payload)?;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let uuid = players
            .registry
            .get(id)
            .context("recipe player missing")?
            .profile
            .uuid;
        let mut lookup = |id: &mc121_rust::server_recipe_book::RecipeId| {
            Ok(VanillaRecipeCatalogue::get().descriptor(id))
        };
        players
            .recipe_books
            .entry(uuid)
            .or_default()
            .seen_recipe(&seen.recipe, &mut lookup)
    }
    fn tracking_observer(
        &self,
        id: i32,
        position: Position,
        requested_view_distance: i32,
    ) -> Result<TrackingObserver> {
        let players = self
            .players
            .read()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        players.observer_for(
            id,
            position,
            requested_view_distance,
            self.options.view_distance,
        )
    }
    fn world_snapshot_locked(&self, players: &LivePlayers) -> Result<WorldSnapshot> {
        let time_payload = self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?
            .time_update_payload();
        Ok(WorldSnapshot {
            revision: players.world_revision,
            time_payload,
            weather: players.world_context.weather(),
        })
    }
    fn world_snapshot(&self) -> Result<WorldSnapshot> {
        let players = self
            .players
            .read()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        self.world_snapshot_locked(&players)
    }
    fn publish_time(&self) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let payload = self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?
            .time_update_payload();
        players.publish_world(
            vec![Packet {
                id: 0x64,
                payload: payload.to_vec(),
            }],
            &self.entity_events,
        )
    }
    fn tick_weather(&self) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let LivePlayers {
            world_context,
            item_runtime,
            ..
        } = &mut *players;
        let changes: Vec<WeatherChange> =
            world_context.advance_weather(item_runtime.world_random_mut());
        // This host has exactly one configured dimension; dimension-local and
        // global weather audiences both refer to its registered players.
        let packets = changes
            .into_iter()
            .map(|change| weather_packet(change.reason, change.value))
            .collect();
        players.publish_world(packets, &self.entity_events)
    }
    fn tick_border(&self) -> Result<()> {
        self.border
            .write()
            .map_err(|_| anyhow!("world border lock poisoned"))?
            .tick(self.epoch.elapsed().as_millis() as i64);
        Ok(())
    }

    async fn tick_player_poses(&self) -> Result<()> {
        let bounds = self
            .border
            .read()
            .map_err(|_| anyhow!("world border lock poisoned"))?
            .bounds(self.epoch.elapsed().as_millis() as i64);
        let border = CollisionBorder {
            min_x: bounds.min_x,
            min_z: bounds.min_z,
            max_x: bounds.max_x,
            max_z: bounds.max_z,
        };
        // Every operation that needs both locks uses world -> registry. Menu
        // transactions finish before accessing either one.
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        for player in players.registry.snapshot() {
            if players.native_body_ready.contains(&player.entity_id){continue;}
            let feet = Vec3::new(player.position.x, player.position.y, player.position.z);
            let shift = player.metadata.flags & 0x02 != 0;
            let leather_boots = players.containers[&player.entity_id].read(|menu| {
                menu.inventory.slot(8).is_some_and(|stack| {
                    !stack.is_empty()
                        && inventory::item_name(stack.item_id) == Some("leather_boots")
                })
            })?;
            let context = EntityCollisionContext {
                bottom_y: feet.y,
                descending: shift,
                fall_distance: players.player_motion_runtime.fall_distance(player.entity_id).unwrap_or(f32::NAN),
                is_falling_block: false,
                can_walk_on_powder_snow: leather_boots,
            };
            let input = PlayerPoseInput {
                shift_key_down: shift,
                flying: players.flying.contains(&player.entity_id),
                spectator: player.gamemode == 3,
                fall_flying: player.metadata.flags & 0x80 != 0,
                swimming: player.metadata.flags & 0x10 != 0,
                sleeping: matches!(
                    player.entity_data.get(14),
                    Some(MetadataValue::OptionalBlockPos(Some(_)))
                ),
                auto_spin_attack: matches!(player.entity_data.get(8),Some(MetadataValue::Byte(flags)) if flags&4!=0),
                passenger: false,
            };
            let mut collision = RuntimeCollision::new(
                &mut world,
                CollisionActor {
                    entity_id: player.entity_id,
                    position: feet,
                },
                &[],
                border,
            )
            .without_fall_distance();
            match player_physics::update_player_pose(
                &mut collision,
                player.entity_id,
                feet,
                player.metadata.pose,
                input,
                1.,
                context,
            ) {
                Ok(pose) => {
                    players.unavailable_pose.remove(&player.entity_id);
                    if pose != player.metadata.pose {
                        players.registry.set_entity_data(
                            player.entity_id,
                            6,
                            MetadataValue::Pose(i32::from(pose)),
                            false,
                        )?;
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    if players.unavailable_pose.get(&player.entity_id) != Some(&message) {
                        eprintln!(
                            "{}",
                            json!({"event":"pose_context_unavailable","entity_id":player.entity_id,"reason":message})
                        );
                        players.unavailable_pose.insert(player.entity_id, message);
                    }
                }
            }
        }
        Ok(())
    }

    fn set_flying(&self, id: i32, flying: bool) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let Some(player) = players.registry.get(id) else {
            return Ok(());
        };
        let uuid = player.profile.uuid;
        let may_fly = players
            .player_lifecycle
            .get(&uuid)
            .is_some_and(|state| state.abilities.may_fly);
        let flying = flying && may_fly;
        if players.player_motion_runtime.contains(id) {
            players.player_motion_runtime.set_flying(id,flying)?;
        }
        if let Some(lifecycle) = players.player_lifecycle.get_mut(&uuid) {
            lifecycle.abilities.flying = flying;
        }
        if flying {
            players.flying.insert(id);
        } else {
            players.flying.remove(&id);
        }
        Ok(())
    }

    fn current_metadata(&self, id: i32) -> Result<PlayerMetadata> {
        self.players
            .read()
            .map_err(|_| anyhow!("player registry lock poisoned"))?
            .registry
            .get(id)
            .map(|player| player.metadata)
            .context("absent player metadata")
    }

    fn acknowledge_block_changes_up_to(&self, entity_id: i32, sequence: i32) -> Result<()> {
        if sequence < 0 {
            bail!("negative block sequence");
        }
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        if players.player_motion_runtime.contains(entity_id) {
            players.player_motion_runtime.acknowledge_block_changes_up_to(entity_id,sequence)?;
        } else if players.registry.get(entity_id).is_some() {
            players
                .pending_block_acks
                .entry(entity_id)
                .and_modify(|pending| *pending = (*pending).max(sequence))
                .or_insert(sequence);
        }
        Ok(())
    }

    fn publish_pending_block_acknowledgments(&self) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        for (player, sequence) in std::mem::take(&mut players.pending_block_acks) {
            let _ = self
                .changes
                .send(WorldUpdate::Acknowledge { player, sequence });
        }
        Ok(())
    }

    fn publish_blocks(&self, changes: Vec<WorldUpdate>) {
        for change in changes {
            let _ = self.changes.send(change);
        }
    }

    fn snapshot(&self) -> Result<LiveSnapshot> {
        self.players
            .read()
            .map_err(|_| anyhow!("player registry lock poisoned"))?
            .replication_snapshot()
    }
    fn tick_players(&self) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let pending = std::mem::take(&mut players.pending_metadata);
        let mut metadata_events = BTreeMap::new();
        for (id, mut metadata) in pending {
            if players.registry.get(id).is_some() {
                metadata.pose = players
                    .registry
                    .get(id)
                    .expect("checked player")
                    .metadata
                    .pose;
                if let Some(event) = players.registry.update_metadata(id, metadata)? {
                    metadata_events.insert(id, event);
                }
            }
        }
        for event in players.registry.drain_entity_data()? {
            metadata_events.insert(event.entity_id(), event);
        }
        let mut equipment_events = BTreeMap::new();
        // Read current authoritative menus in this tick, rather than queueing
        // snapshots that can be published out of order by concurrent writers.
        // Container transactions never acquire the registry lock.
        for id in players.containers.keys().copied().collect::<Vec<_>>() {
            let equipment = players.containers[&id]
                .read(|menu| PlayerEquipment::from_inventory(&menu.inventory))??;
            if let Some(event) = players.registry.update_equipment_state(id, equipment)? {
                equipment_events.insert(id, event);
            }
        }
        let mut frames = BTreeMap::new();
        for player in players.registry.snapshot() {
            if players.player_motion_runtime.contains(player.entity_id) {
                players.player_motion_runtime.begin_level_tick(player.entity_id)?;
            }
            let motion = players
                .motion
                .get_mut(&player.entity_id)
                .context("registered player has no movement tracker")?;
            let packets = motion.tick(
                player.position,
                metadata_events.contains_key(&player.entity_id),
            )?;
            let position = motion.spawn_position();
            let (head, body): (Vec<_>, Vec<_>) =
                packets.into_iter().partition(|packet| packet.id == 0x48);
            frames.insert(
                player.entity_id,
                (
                    position,
                    body,
                    metadata_events.remove(&player.entity_id),
                    head,
                    equipment_events.remove(&player.entity_id),
                ),
            );
        }
        for (chunk, status) in std::mem::take(&mut players.confirmed_chunk_callbacks) {
            players.apply_confirmed_chunk_status(chunk, status)?;
        }
        let LivePlayers {
            item_runtime,
            chunk_runtime,
            revision,
            ..
        } = &mut *players;
        item_runtime.publish_tracking_with_players(
            |chunk| {
                chunk_runtime
                    .manager
                    .ticking_range()
                    .in_entity_ticking_range(chunk)
            },
            |handle| {
                let (position, body, metadata, head, equipment) = frames
                    .remove(&handle.id)
                    .context("shared sender references absent player frame")?;
                for action in [
                    Some((Some(body), None)),
                    metadata.map(|event| (None, Some(event))),
                    Some((Some(head), None)),
                    equipment.map(|event| (None, Some(event))),
                ]
                .into_iter()
                .flatten()
                {
                    let (packets, event) = action;
                    if let Some(packets) = packets {
                        if packets.is_empty() {
                            continue;
                        }
                        *revision = revision
                            .checked_add(1)
                            .context("entity revision exhausted")?;
                        let _ = self.entity_events.send(PlayerEvent::Motion {
                            revision: *revision,
                            entity_id: handle.id,
                            position,
                            packets,
                        });
                    } else if let Some(event) = event {
                        *revision = revision
                            .checked_add(1)
                            .context("entity revision exhausted")?;
                        let _ =
                            self.entity_events
                                .send(PlayerEvent::State(Arc::new(EntityUpdate {
                                    revision: *revision,
                                    event,
                                })));
                    }
                }
                Ok(())
            },
        )?;
        ensure!(frames.is_empty(), "player missing from shared sender map");
        Ok(())
    }

    fn move_player(&self, id: i32, position: Position) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        if players.player_motion_runtime.contains(id) {
            players.player_motion_runtime.sync_use_item_rotation(id,position.yaw,position.pitch)?;
        }
        Self::move_player_locked(&mut players, id, position, &self.entity_events)
    }
    fn move_player_locked(
        players: &mut LivePlayers,
        id: i32,
        position: Position,
        sender: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        if let Some(event) = players.registry.update_position(id, position)? {
            players
                .item_runtime
                .move_player(id, Vec3::new(position.x, position.y, position.z))?;
            let chunk = (
                (position.x.floor() as i32).div_euclid(16),
                (position.z.floor() as i32).div_euclid(16),
            );
            if players.player_chunks.get(&id) != Some(&chunk) {
                if let Some(previous) = players.player_chunks.insert(id, chunk) {
                    players
                        .chunk_runtime
                        .manager
                        .player_move(id, previous, chunk)?;
                }
            }
            players.publish(event, sender);
        }
        Ok(())
    }

    fn metadata(&self, id: i32, mut metadata: PlayerMetadata) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        metadata.pose = players
            .registry
            .get(id)
            .context("metadata for absent player")?
            .metadata
            .pose;
        players.pending_metadata.insert(id, metadata);
        Ok(())
    }

    fn animate(&self, id: i32, hand: i32) -> Result<()> {
        ensure!((0..=1).contains(&hand), "invalid animation hand");
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        if players.player_motion_runtime.contains(id) && !players.player_motion_runtime.swing(id,hand as u8)? {return Ok(());}
        let event = players.registry.animation(id, hand)?;
        players.publish(event, &self.entity_events);
        Ok(())
    }

    fn update_latency(&self, id: i32, latency: i32) -> Result<()> {
        self.players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?
            .registry
            .set_latency(id, latency)
    }

    async fn save(&self) -> Result<()> {
        let _save = self.save_lock.lock().await;
        let mut world = self.world.write().await;
        let clock = *self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?;
        let (weather, entity_checkpoint, fluid_checkpoint, fluid_shapes, fire_checkpoint, random_position_value) = {
            let mut players = self
                .players
                .write()
                .map_err(|_| anyhow!("player registry lock poisoned"))?;
            if self.shutting_down.load(Ordering::Acquire) {
                self.close_crafting_tables_for_save_locked(&mut world,&mut players,clock.game_time)?;
                self.close_native_containers_for_save_locked(&mut world,&mut players,clock.game_time)?;
                let b=self.border.read().map_err(|_|anyhow!("border poisoned"))?.bounds(self.epoch.elapsed().as_millis() as i64);
                let border=CollisionBorder{min_x:b.min_x,min_z:b.min_z,max_x:b.max_x,max_z:b.max_z};
                for player in players.registry.snapshot() {
                    self.close_inventory_menu_locked(&mut world,&mut players,player.entity_id,clock.game_time,border)?;
                }
            }
            let ids = players
                .registry
                .snapshot()
                .into_iter()
                .map(|player| player.entity_id)
                .collect::<Vec<_>>();
            for id in ids {
                self.save_player_locked(&mut players, id)?;
            }
            let pending = players
                .pending_player_saves
                .keys()
                .copied()
                .collect::<Vec<_>>();
            for uuid in pending {
                self.persist_player_data(players.pending_player_saves.get(&uuid).unwrap())?;
                players.pending_player_saves.remove(&uuid);
            }
            let weather = players.world_context.weather();
            ensure!(
                players.world_interactions.pending_count() == 0,
                "cannot checkpoint unexecuted ordered world interactions; fluid scheduling and other callbacks would be lost"
            );
            if players.chunk_runtime.has_native_lighting_owner() {
                for update in players.chunk_runtime.settle_lighting(&world)? {
                    let _ = self.changes.send(update);
                }
            }
            if players.entity_source.as_ref().is_some_and(|source| source.has_pending_edits()) {
                players.complete_native_source_edits(&world)?;
            }
            if players.item_runtime.item_count() != 0 || players.entity_source.as_ref().is_some_and(|source| source.is_supported()) {
                players.require_current_native_source(&world)?;
            }
            let fluid_checkpoint = players
                .fluid_runtime
                .checkpoint(clock.game_time, players.item_runtime.world_random_state48());
            let fluid_shapes = mc121_rust::fluid_shape_checkpoint::SavedFluidShapeCache::capture(
                &players.world_interactions.fluid_shapes,
            )?;
            let fire_checkpoint = players.fire_runtime.checkpoint(clock.game_time, players.fluid_runtime.sub_tick_order);
            let random_position_value = players.level_random_position_value;
            let LivePlayers {
                item_runtime,
                entity_source,
                ..
            } = &mut *players;
            let checkpoint = entity_source
                .as_mut()
                .map(|source| {
                    EntityCheckpoint::capture(
                        item_runtime,
                        source,
                        &world,
                        self.entities.load(Ordering::Relaxed),
                        NativeWorldTime { clock, weather },
                    )
                })
                .transpose()?;
            (weather, checkpoint, fluid_checkpoint, fluid_shapes, fire_checkpoint, random_position_value)
        };
        match &mut *world {
            WorldBackend::Anvil(world) => {
                world.save()?;
            }
            WorldBackend::Fixture(world) => {
                let saved = SavedWorld {
                    format: "mc121-flat-diffs-v1".into(),
                    minecraft: "1.21".into(),
                    changes: world.changes(),
                    clock: Some(clock),
                    weather: Some(weather),
                    fluid_ticks: Some(fluid_checkpoint),
                    fluid_shapes: Some(fluid_shapes),
                    fire_ticks: Some(fire_checkpoint),
                    native_block_entities: Some(world.native_block_entities().save(clock.game_time)?),
                    block_ticks: Some(self.players.read().map_err(|_|anyhow!("player registry lock poisoned"))?.block_runtime.checkpoint(clock.game_time)),
                    level_random_position_value: random_position_value,
                };
                let path = &self.options.world;
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    fs::create_dir_all(parent).await?;
                }
                let temporary = path.with_extension("json.tmp");
                fs::write(&temporary, serde_json::to_vec_pretty(&saved)?).await?;
                fs::rename(temporary, path)
                    .await
                    .context("atomically replacing fixture world diffs")?;
            }
        }
        // The world image cannot change between checkpoint capture and the
        // two writes. A torn pair is rejected by the checkpoint's exact image.
        if let Some(checkpoint) = entity_checkpoint {
            self.entity_store.save(&checkpoint)?;
        }
        drop(world);
        if let Some(level) = &self.level {
            let border = self
                .border
                .read()
                .map_err(|_| anyhow!("world border lock poisoned"))?
                .settings(self.epoch.elapsed().as_millis() as i64);
            let mut level = level.lock().await;
            level.set_border_settings(&border)?;
            let wall_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
            level.update(clock, Some(weather), wall_ms)?;
            level.save_atomic()?;
        }
        Ok(())
    }
}

struct PlayerGuard(Arc<AtomicUsize>);
impl PlayerGuard {
    fn reserve(count: Arc<AtomicUsize>) -> Result<Self> {
        let mut current = count.load(Ordering::Acquire);
        loop {
            if current >= MAX_PLAYERS {
                bail!("fixture server is full");
            }
            match count.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(Self(count)),
                Err(updated) => current = updated,
            }
        }
    }
}
impl Drop for PlayerGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
struct ReaderGuard(JoinHandle<()>);
impl Drop for ReaderGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct PresenceGuard {
    entity_id: i32,
    server: Arc<Server>,
}
impl Server {
    async fn cleanup_player(&self, id: i32, reason: Option<&'static str>) -> Result<()> {
        let admission = self.player_admission.lock().await;
        self.cleanup_player_admitted(id, reason, &admission).await
    }
    async fn cleanup_player_admitted(&self, id: i32, reason: Option<&'static str>, _admission: &tokio::sync::MutexGuard<'_, ()>) -> Result<()> {
        let now = self.clock.read().map_err(|_|anyhow!("clock poisoned"))?.game_time;
        let b = self.border.read().map_err(|_|anyhow!("border poisoned"))?.bounds(self.epoch.elapsed().as_millis() as i64);
        let border = CollisionBorder { min_x:b.min_x,min_z:b.min_z,max_x:b.max_x,max_z:b.max_z };
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_|anyhow!("player registry lock poisoned"))?;
        if players.registry.get(id).is_none() {
            ensure!(!players.native_containers.has_open(id), "orphan native menu retained");
            ensure!(!players.crafting_tables.has_open(id), "orphan crafting menu retained");
            return Ok(());
        }
        if let Some(reason)=reason {
            if let Some(disconnect)=players.disconnects.get(&id) { disconnect.send_replace(Some(reason)); }
        }
        if let Some(window)=players.crafting_tables.window_id(id) {
            self.crafting_table_action_locked(&mut world,&mut players,id,None,Some(window),false,now,border)?;
        }
        if let Some(window)=players.native_containers.window_id(id) {
            self.native_menu_action_locked(&mut world,&mut players,id,None,Some(window),false,now,border)?;
        }
        self.close_inventory_menu_locked(&mut world,&mut players,id,now,border)?;
        self.save_player_locked(&mut players,id)?;
        players.remove_runtime_player(id)?;
        if let Some(event)=players.registry.remove(id) {
            players.motion.remove(&id);
            players.pending_metadata.remove(&id);
            players.containers.remove(&id);
            players.disconnects.remove(&id);
            players.pending_block_acks.remove(&id);
            players.flying.remove(&id);
            players.unavailable_pose.remove(&id);
            players.publish(event,&self.entity_events);
        }
        Ok(())
    }
}
impl Drop for PresenceGuard {
    fn drop(&mut self) {
        let server=Arc::clone(&self.server);
        let id=self.entity_id;
        tokio::spawn(async move {
            let mut ticks=server.tick_events.subscribe();
            loop {
                match server.cleanup_player(id,None).await {
                    Ok(()) => return,
                    Err(error) => eprintln!("{}",json!({"event":"native_player_cleanup_failed","entity_id":id,"reason":format!("{error:#}"),"actor_retained":true})),
                }
                if server.shutting_down.load(Ordering::Acquire) || ticks.changed().await.is_err() { return; }
            }
        });
    }
}

/// Keep both signal subscriptions alive until shutdown; orchestration sends
/// SIGTERM while local development usually sends Ctrl-C.
async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate=tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result=tokio::signal::ctrl_c() => result,
            _=terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    { tokio::signal::ctrl_c().await }
}

#[tokio::main]
async fn main() -> Result<()> {
    let options = Options::parse()?;
    let fixtures = fixtures(&options.fixtures).await?;
    // Exact 1.21 minecraft-data defaults: air=0, stone=1, dirt=10,
    // grass_block=9 and minecraft:plains biome=39. This is a flat fixture.
    let mut fixture_world = FlatWorld::new(0, 1, 10, 9, 39);
    let mut initial_clock = TickClock::default();
    let mut initial_weather = WeatherState::default();
    let mut initial_fluid_runtime = main_fluid_simulation::MainFluidRuntime::default();
    let mut initial_fluid_random = None;
    let mut fluid_checkpoint_clock = None;
    let mut initial_fluid_shapes = None;
    let mut initial_fire_runtime = None;
    let mut had_fire_checkpoint = false;
    let mut initial_level_random_position_value = None;
    let mut initial_block_ticks = None;
    let mut had_block_checkpoint = false;
    let fresh_unedited = options.anvil_world.is_none() && !fs::try_exists(&options.world).await?;
    if options.anvil_world.is_none() && fs::try_exists(&options.world).await? {
        if fs::metadata(&options.world).await?.len() > 16 * 1024 * 1024 {
            bail!("saved fixture world exceeds 16 MiB");
        }
        let saved: SavedWorld = serde_json::from_slice(&fs::read(&options.world).await?)?;
        if saved.format != "mc121-flat-diffs-v1" || saved.minecraft != "1.21" {
            bail!("unsupported saved fixture world format");
        }
        fixture_world.apply_changes(&saved.changes)?;
        initial_clock = saved.clock.unwrap_or_default();
        initial_block_ticks = saved.block_ticks;
        had_block_checkpoint = initial_block_ticks.is_some();
        if let Some(records)=saved.native_block_entities {
            fixture_world.restore_native_block_entities(records,initial_clock.game_time)?;
        }
        initial_level_random_position_value = saved.level_random_position_value;
        let mut first_fluid = None;
        for change in fixture_world.changes() {
            if !mc121_rust::fluid_physics::FluidState::from_block(change.state)?.is_empty() {
                first_fluid = Some(change);
                break;
            }
        }
        let fluid_shapes = saved.fluid_shapes
            .map(|saved| saved.restore())
            .transpose()
            .context("restoring actual singleton fluid-shape history")?
            .unwrap_or_else(mc121_rust::fluid_physics::FluidShapeCache::unknown);
        let had_fluid_checkpoint = saved.fluid_ticks.is_some();
        if let Some(checkpoint) = saved.fluid_ticks {
            ensure!(saved.clock.is_some(), "native fluid checkpoint has no matching saved world clock");
            if first_fluid.is_some() {
                mc121_rust::entity_persistence::require_complete_fluid_shapes(&fluid_shapes)
                    .context("native_fluid_shape_source_unknown: existing fluid has incomplete singleton shape history")?;
            }
            initial_fluid_random = Some(checkpoint.world_random()?);
            fluid_checkpoint_clock = Some(initial_clock);
            initial_fluid_runtime = main_fluid_simulation::MainFluidRuntime::from_checkpoint(checkpoint)
                .context("restoring native scheduled fluid queues")?;
        } else {
            // A legacy dry fixture has no fluid callbacks to resume. Existing
            // water, lava, waterlogging and aquatic plants cannot prove that
            // their missing queue was empty; do not fabricate initial ticks.
            if let Some(change) = first_fluid {
                eprintln!("{}", json!({"event":"native_fluid_source_unknown","position":[change.x,change.y,change.z],"state":change.state,"reason":"existing fluid has no saved scheduled-tick checkpoint"}));
                bail!("native_fluid_source_unknown: legacy fixture contains fluid at {},{},{} without its scheduled-tick checkpoint", change.x, change.y, change.z);
            }
        }
        initial_fluid_shapes = Some(fluid_shapes);
        if let Some(checkpoint) = saved.fire_ticks {
            ensure!(saved.clock.is_some() && had_fluid_checkpoint, "native fire checkpoint requires paired clock/fluid shared-counter checkpoint");
            ensure!(checkpoint.shared_sub_tick_order == initial_fluid_runtime.sub_tick_order,
                "native fire/fluid shared sub-tick counter mismatch; refusing torn queue checkpoint");
            initial_fire_runtime = Some(main_fire_simulation::MainFireRuntime::from_checkpoint(checkpoint)?);
            had_fire_checkpoint = true;
        } else {
            for change in fixture_world.changes() {
                ensure!(mc121_rust::fire_simulation::fire_kind(change.state)? == 0,
                    "native fire source unknown: existing fire has no saved scheduled block queue");
            }
        }
        // Legacy v1 fixture diffs had no weather simulation. New captures
        // retain its actual timers/levels instead of resetting on restart.
        if let Some(weather) = saved.weather {
            // Only the persisted timers/flags survive a real Level reload;
            // interpolation fields are initialized again by prepareWeather.
            initial_weather = WeatherState {
                clear_weather_time: weather.clear_weather_time,
                rain_time: weather.rain_time,
                thunder_time: weather.thunder_time,
                raining: weather.raining,
                thundering: weather.thundering,
                ..WeatherState::default()
            };
            initial_weather.prepare_weather();
        }
    }
    let level = options
        .anvil_world
        .as_ref()
        .map(LevelData::open)
        .transpose()?;
    if let Some(level) = &level {
        initial_clock = level.clock()?;
    }
    let world = match &options.anvil_world {
        Some(path) => WorldBackend::Anvil(VanillaWorld::open(path)?),
        None => WorldBackend::Fixture(fixture_world),
    };
    let player_directory = options
        .anvil_world
        .as_ref()
        .map(|path| path.join("native-playerdata"))
        .unwrap_or_else(|| options.world.with_extension("players"));
    let entity_path = options
        .anvil_world
        .as_ref()
        .map(|path| path.join("native-entities.json"))
        .unwrap_or_else(|| options.world.with_extension("entities.native.json"));
    let player_store = PlayerDataStore::new(player_directory);
    let entity_store = EntityDataStore::new(entity_path);
    let mut restored_entities = None;
    let mut entity_source = None;
    if let Some(checkpoint) = entity_store.load()? {
        let item_count = checkpoint.item_count();
        match checkpoint.restore(&world) {
            Ok(restored) => {
                ensure!(!restored.source.requires_fire_checkpoint() || had_fire_checkpoint,
                    "native source history requires its saved fire queue, including stale scheduled ticks");
                ensure!(!restored.source.requires_block_checkpoint() || had_block_checkpoint,
                    "native source history requires its saved global BLOCK scheduler");
                ensure!(!restored.source.requires_fluid_checkpoint() || initial_fluid_random.is_some(),
                    "native source history requires its saved fluid queue, including stale scheduled ticks");
                if restored.source.requires_fluid_checkpoint() {
                    restored.source.require_fluid_history(&world, initial_fluid_shapes.as_ref()
                        .context("native source history requires its saved singleton fluid-shape cache")?)?;
                }
                if let Some(clock) = fluid_checkpoint_clock {
                    ensure!(restored.time.clock == clock,
                        "native fluid/entity checkpoint time mismatch; refusing a torn save pair");
                }
                if let Some(random) = &initial_fluid_random {
                    ensure!(restored.runtime.world_random_state48() == random.state48(),
                        "native fluid/entity Level.random mismatch; refusing a torn save pair");
                }
                initial_clock = restored.time.clock;
                initial_weather = restored.time.weather;
                restored_entities = Some(restored);
            }
            Err(error) if item_count == 0 => {
                eprintln!(
                    "{}",
                    json!({"event":"native_entity_resume_unavailable","scope":"legacy_profile_transport","reason":format!("{error:#}"),"saved_items":0})
                );
            }
            Err(error) => {
                return Err(error)
                    .context("refusing to discard a nonempty native entity checkpoint")
            }
        }
    } else if fresh_unedited {
        entity_source = Some(entity_store.fresh_source(&options.world, &world)?);
    } else {
        eprintln!(
            "{}",
            json!({"event":"native_entity_source_unknown","scope":"legacy_profile_transport","reason":"existing world has no complete native entity checkpoint"})
        );
    }
    let supported_native_source = entity_source.is_some()
        || restored_entities
            .as_ref()
            .is_some_and(|state| state.source.is_supported());
    let world_context = match &level {
        Some(level) => WorldContext::from_level(fixtures.dimension_facts, level)?,
        None => WorldContext::new(
            fixtures.dimension_facts,
            WorldRules::default(),
            initial_weather,
        ),
    };
    let border = WorldBorder::from_settings(
        level
            .as_ref()
            .map(LevelData::border_settings)
            .unwrap_or_default(),
        0,
    );
    let seed_hash = level
        .as_ref()
        .map(LevelData::seed)
        .transpose()?
        .map(world::obfuscate_biome_seed)
        .unwrap_or(fixtures.seed_hash);
    let listener = TcpListener::bind(SocketAddr::new(options.bind, options.port)).await?;
    let (changes, _) = broadcast::channel(4096);
    let (entity_events, _) = broadcast::channel(512);
    let (tick_events, _) = watch::channel(0);
    let authenticator = options.online_mode.then(Authenticator::new).transpose()?;
    let session_verifier = options.online_mode.then(SessionVerifier::new).transpose()?;
    let random_seed::LevelRandoms {
        rand_value,
        random: world_random,
        thread_safe_random: sound_random,
        ..
    } = random_seed::create_level_randoms()?;
    let mut players = LivePlayers::new(
        world_random,
        sound_random,
        world_context,
        options.view_distance,
        supported_native_source,
        entity_events.clone(),
    );
    let mut next_entity_id = 1;
    players.math_random = Some(mc121_rust::item_use::JdkMathRandom::fresh()?);
    players.redstone_runtime.configure_neighbor_limit(options.max_chained_neighbor_updates);
    players.native_difficulty = mc121_rust::survival::Difficulty::from_id(options.difficulty);
    players.fire_runtime = main_fire_simulation::MainFireRuntime::new(options.difficulty)?;
    // Saved native continuation can retain randValue. Legacy saves use the
    // value from the actual new Level constructor; vanilla does not store it
    // in level.dat. This is a cold level construction, not a VM suspension.
    players.level_random_position_value = initial_level_random_position_value.or(Some(rand_value));
    if let Some(restored) = restored_entities {
        next_entity_id = restored.next_entity_id;
        let publisher = entity_events.clone();
        players.item_runtime = restored.runtime.with_publisher(Arc::new(move |event| {
            let _ = publisher.send(PlayerEvent::Item(Arc::new(event)));
        }));
        entity_source = Some(restored.source);
    }
    players.entity_source = entity_source;
    if let Some(random) = initial_fluid_random {
        *players.item_runtime.world_random_mut() = random;
    }
    players.fluid_runtime = initial_fluid_runtime;
    if let Some(mut fire) = initial_fire_runtime {
        fire.set_difficulty(options.difficulty)?;
        players.fire_runtime = fire;
    }
    if let Some(saved)=initial_block_ticks {
        players.block_runtime=main_block_ticks::MainBlockRuntime::from_checkpoint(saved,supported_native_source)?;
    }
    players.block_runtime.adopt_legacy_fire(&mut players.fire_runtime,initial_clock.game_time)?;
    if let Some(cache) = initial_fluid_shapes {
        players.world_interactions.fluid_shapes = cache;
    }
    players
        .gameplay
        .restore_known_native_players(main_player_data::load_known_native_players(
            &player_store,
        )?)?;
    let server = Arc::new(Server {
        options,
        fixtures,
        seed_hash,
        world: RwLock::new(world),
        save_lock: Mutex::new(()),
        player_admission: Mutex::new(()),
        online: Arc::new(AtomicUsize::new(0)),
        entities: AtomicI32::new(next_entity_id),
        connection_sequence: AtomicU64::new(1),
        changes,
        authenticator,
        session_verifier,
        players: StdRwLock::new(players),
        entity_events,
        clock: StdRwLock::new(initial_clock),
        tick_events,
        epoch: Instant::now(),
        level: level.map(Mutex::new),
        border: StdRwLock::new(border),
        player_store,
        entity_store,
        shutting_down: AtomicBool::new(false),
    });
    let permits = Arc::new(Semaphore::new(64));
    println!(
        "{}",
        json!({"event":"listening","address":listener.local_addr()?.to_string(),
        "minecraft":"1.21","protocol":PROTOCOL,"mode":if server.options.online_mode {"online"} else if server.options.bind.is_loopback() {"offline-loopback"} else {"offline"}})
    );
    let mut ticks = time::interval_at(Instant::now() + TICK_DURATION, TICK_DURATION);
    ticks.set_missed_tick_behavior(time::MissedTickBehavior::Burst);
    let mut latency_clock = LatencyUpdateClock::default();
    let mut autosave_ticks = 0_u32;
    let mut server_tick_count = 0_i32;
    // Retain the registered signal listener while a tick/accept branch runs.
    // Recreating ctrl_c in select would drop its receiver after every winning
    // branch and could lose SIGINT delivered during the branch body.
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = ticks.tick() => {
                server_tick_count=server_tick_count.wrapping_add(1);
                // MinecraftServer.tickChildren timeSync precedes aqu.tick;
                // aqu then runs border, weather and tickTime in that order.
                if server_tick_count%20==0 {server.publish_time()?;}
                server.tick_border()?;
                server.tick_weather()?;
                server.clock.write().map_err(|_|anyhow!("world clock lock poisoned"))?.advance();
                server.poll_chunk_runtime().await?;
                server.tick_players()?;
                server.tick_native_containers().await?;
                server.tick_native_crafting_tables().await?;
                server.tick_player_poses().await?;
                server.drain_world_interaction_phase().await?;
                server.tick_block_world().await?;
                server.tick_fluid_world().await?;
                if server.options.anvil_world.is_none() {
                    server.tick_random_block_world().await?;
                }
                server.drain_world_interaction_phase().await?;
                server.tick_item_world().await?;
                server.tick_connection_phase().await?;
                server.publish_pending_block_acknowledgments()?;
                server.send_chunk_phase().await?;
                server.tick_events.send_replace(server.epoch.elapsed().as_millis() as i64);
                if latency_clock.tick() {
                    let snapshot=server.snapshot()?;
                    let _=server.entity_events.send(PlayerEvent::Latency(snapshot.players));
                }
                autosave_ticks+=1;
                if autosave_ticks==6000 {server.save().await?;autosave_ticks=0;}
            }
            accepted = listener.accept() => {
                let (socket, address) = accepted?;
                let admission=server.connection_sequence.try_update(Ordering::Relaxed,Ordering::Relaxed,|value|value.checked_add(1))
                    .map_err(|_|anyhow!("connection admission sequence exhausted"))?;
                let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else { drop(socket); continue; };
                let server = Arc::clone(&server);
                tokio::spawn(async move {
                    let _permit = permit;
                    if let Err(error) = connection(socket, server, admission).await {
                        eprintln!("{}", json!({"event":"connection_closed","peer":address.to_string(),"reason":error.to_string()}));
                    }
                });
            }
            signal = &mut shutdown => {
                signal?;
                server.shutting_down.store(true, Ordering::Release);
                server.save().await?;
                return Ok(());
            }
        }
    }
}

async fn send(
    codec: &FrameCodec,
    stream: &mut NetworkWriter,
    id: i32,
    payload: impl AsRef<[u8]> + Send + Sync,
) -> Result<()> {
    time::timeout(WRITE_DEADLINE, async {
        codec.write(stream, id, payload.as_ref()).await?;
        stream.flush().await?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("client write deadline")??;
    Ok(())
}
async fn send_packets(
    codec: &FrameCodec,
    writer: &mut NetworkWriter,
    packets: Vec<Packet>,
) -> Result<()> {
    for packet in packets {
        send(codec, writer, packet.id, packet.payload).await?;
    }
    Ok(())
}

async fn play_disconnect(
    codec: &FrameCodec,
    writer: &mut NetworkWriter,
    translation: &str,
) -> Result<()> {
    let reason = Tag::Compound(vec![("translate".into(), Tag::String(translation.into()))])
        .encode_network_anonymous()?;
    send(codec, writer, 0x1d, reason).await
}

async fn receive(codec: &FrameCodec, stream: &mut NetworkReader) -> Result<Packet> {
    time::timeout(CONFIGURATION_DEADLINE, codec.read(stream))
        .await
        .context("handshake/configuration deadline")??
        .ok_or_else(|| anyhow!("connection closed during negotiation"))
}
fn encoded(build: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut writer = Writer::new();
    build(&mut writer);
    writer.into_inner()
}

async fn connection(socket: TcpStream, server: Arc<Server>, admission: u64) -> Result<()> {
    socket.set_nodelay(true)?;
    let (reader, writer) = socket.into_split();
    let mut reader = CryptoRead::plain(reader);
    let mut writer = CryptoWrite::plain(writer);
    let mut codec = FrameCodec::default();
    let hello = receive(&codec, &mut reader).await?;
    if hello.id != 0 {
        bail!("expected handshake");
    }
    let mut fields = Reader::new(&hello.payload);
    let protocol = fields.varint()?;
    let _host = fields.string(255)?;
    let _port = fields.u16()?;
    let intent = fields.varint()?;
    fields.finish()?;
    if intent == 1 {
        let request = receive(&codec, &mut reader).await?;
        if request.id != 0 || !request.payload.is_empty() {
            bail!("invalid status request");
        }
        let response = json!({"version":{"name":"1.21","protocol":PROTOCOL},
            "players":{"max":MAX_PLAYERS,"online":server.players.read().map_err(|_| anyhow!("player registry lock poisoned"))?.registry.len(),"sample":[]},
            "description":{"text":"Rust compatibility checkpoint — creative fixture; vanilla parity incomplete"},
            "enforcesSecureChat":false});
        send(
            &codec,
            &mut writer,
            0,
            encoded(|w| {
                w.string(&response.to_string());
            }),
        )
        .await?;
        if let Some(ping) = time::timeout(Duration::from_secs(5), codec.read(&mut reader)).await?? {
            if ping.id != 1 || ping.payload.len() != 8 {
                bail!("invalid status ping");
            }
            send(&codec, &mut writer, 1, ping.payload).await?;
        }
        return Ok(());
    }
    if intent != 2 {
        bail!("only status and login handshakes are implemented");
    }
    if protocol != PROTOCOL {
        send(
            &codec,
            &mut writer,
            0,
            encoded(|w| {
                w.string(
                    &json!({"text":"This checkpoint requires Minecraft Java 1.21 (protocol 767)"})
                        .to_string(),
                );
            }),
        )
        .await?;
        bail!("unsupported client protocol {protocol}");
    }
    let start = receive(&codec, &mut reader).await?;
    if start.id != 0 {
        bail!("expected Login Start");
    }
    let mut fields = Reader::new(&start.payload);
    let username = fields.string(16)?;
    if !valid_login_name(&username) {
        bail!("invalid login username");
    }
    let _claimed_uuid = fields.uuid()?;
    fields.finish()?;
    let _player = PlayerGuard::reserve(Arc::clone(&server.online))?;
    let profile = if let Some(authenticator) = &server.authenticator {
        let challenge = authenticator.challenge();
        send(
            &codec,
            &mut writer,
            1,
            authenticator.encryption_request(&challenge),
        )
        .await?;
        let response = receive(&codec, &mut reader).await?;
        if response.id != 1 {
            bail!("expected encrypted login response");
        }
        let secret = authenticator.accept_response(&challenge, &response.payload)?;
        let session_hash = authenticator.session_hash(&secret);
        reader.enable(&secret)?;
        writer.enable(&secret)?;
        let verifier = server
            .session_verifier
            .as_ref()
            .context("online session verifier missing")?;
        let verified = match verifier.verify(&username, &session_hash, None).await {
            Ok(profile) => profile,
            Err(error) => {
                let message =
                    json!({"text":"Failed to verify username with the Minecraft session service"})
                        .to_string();
                send(
                    &codec,
                    &mut writer,
                    0,
                    encoded(|w| {
                        w.string(&message);
                    }),
                )
                .await?;
                return Err(error);
            }
        };
        PlayerProfile {
            uuid: verified.uuid,
            name: verified.name,
            properties: verified
                .properties
                .into_iter()
                .map(|property| entity::ProfileProperty {
                    name: property.name,
                    value: property.value,
                    signature: property.signature,
                })
                .collect(),
        }
    } else {
        PlayerProfile::offline(&username)?
    };
    send(
        &codec,
        &mut writer,
        3,
        encoded(|w| {
            w.varint(256);
        }),
    )
    .await?;
    codec.compression = Some(256);
    send(
        &codec,
        &mut writer,
        2,
        encoded(|w| {
            w.uuid(profile.uuid)
                .string(&profile.name)
                .varint(profile.properties.len() as i32);
            for property in &profile.properties {
                w.string(&property.name)
                    .string(&property.value)
                    .bool(property.signature.is_some());
                if let Some(signature) = &property.signature {
                    w.string(signature);
                }
            }
            w.bool(true);
        }),
    )
    .await?;
    let acknowledged = receive(&codec, &mut reader).await?;
    if acknowledged.id != 3 || !acknowledged.payload.is_empty() {
        bail!("expected Login Acknowledged");
    }
    for packet in &server.fixtures.configuration {
        send(&codec, &mut writer, packet.id, packet.payload.clone()).await?;
    }
    send(&codec, &mut writer, 3, Vec::new()).await?;
    let configuration_started = Instant::now();
    let mut client_information = None;
    let mut complete = false;
    for _ in 0..32 {
        if configuration_started.elapsed() > CONFIGURATION_DEADLINE {
            bail!("configuration negotiation deadline exceeded");
        }
        let packet = time::timeout_at(
            configuration_started + CONFIGURATION_DEADLINE,
            receive(&codec, &mut reader),
        )
        .await
        .context("configuration negotiation deadline exceeded")??;
        match packet.id {
            3 if packet.payload.is_empty() => {
                complete = true;
                break;
            }
            0 => {
                client_information = Some(read_client_information(&packet.payload)?);
            }
            2 if packet.payload.len() <= 32_768 => {} // brand/custom payload
            _ => bail!("unsupported configuration response {}", packet.id),
        }
    }
    if !complete {
        bail!("configuration was not acknowledged");
    }
    play(
        reader,
        writer,
        codec,
        server,
        profile,
        client_information,
        admission,
    )
    .await
}

fn read_client_information(payload: &[u8]) -> Result<ClientInformation> {
    let mut fields = Reader::new(payload);
    fields.string(16)?;
    let view_distance = i32::from(fields.i8()?);
    let chat = fields.varint()?;
    fields.bool()?;
    let skin = fields.u8()?;
    let hand = fields.varint()?;
    fields.bool()?;
    fields.bool()?;
    fields.finish()?;
    if !(0..=2).contains(&chat) || !(0..=1).contains(&hand) {
        bail!("invalid client information enum");
    }
    Ok(ClientInformation {
        view_distance,
        skin_parts: skin,
        main_hand: hand as u8,
    })
}

async fn play(
    mut reader: NetworkReader,
    mut writer: NetworkWriter,
    codec: FrameCodec,
    server: Arc<Server>,
    profile: PlayerProfile,
    client_information: Option<ClientInformation>,
    admission: u64,
) -> Result<()> {
    // Queue updates before the first snapshot. A concurrent player may edit an
    // already-sent chunk while the rest of this player's window is streaming.
    let mut changes = server.changes.subscribe();
    let mut entity_events = server.entity_events.subscribe();
    let mut tick_events = server.tick_events.subscribe();
    let (entity_id, player_random) = server.allocate_player_entity()?;
    let menu_sender = server.entity_events.clone();
    let container = SharedContainer::with_publisher(Arc::new(move |update| {
        let _ = menu_sender.send(PlayerEvent::Container {
            recipient: entity_id,
            update: Arc::new(update),
        });
    }));
    let (disconnect, mut disconnect_events) = watch::channel(None);
    let username = profile.name.clone();
    let mut position = Position {
        x: 8.5,
        y: 65.0,
        z: 8.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: false,
    };
    let mut metadata;
    let gamemode;
    let login_health;
    let login_lifecycle;
    let native_motion;
    let recipe_context = RecipeContext::default();
    let mut requested_view_distance = client_information
        .map(|value| value.view_distance)
        .unwrap_or(entity_tracking::DEFAULT_REQUESTED_VIEW_DISTANCE);
    let mut chunk_radius = entity_tracking::player_view_distance(
        requested_view_distance,
        server.options.view_distance,
    );
    let _presence=PresenceGuard { entity_id, server:Arc::clone(&server) };
    let admission_guard=server.player_admission.lock().await;
    ensure!(!server.shutting_down.load(Ordering::Acquire), "server shutting down");
    let previous={
        let players=server.players.read().map_err(|_|anyhow!("player registry lock poisoned"))?;
        players.registry.get_by_uuid(&profile.uuid).map(|p|p.entity_id)
    };
    if let Some(previous)=previous {
        server.cleanup_player_admitted(previous,Some("multiplayer.disconnect.duplicate_login"),&admission_guard).await?;
    }
    ensure!(!server.shutting_down.load(Ordering::Acquire), "server shutting down");
    {
        let mut players = server
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let uuid = profile.uuid;
        let mut saved = match players.pending_player_saves.get(&uuid) {
            Some(data) => Some(data.clone()),
            None => server.player_store.load(uuid)?,
        };
        if saved.is_none() {
            if let Some(root)=&server.options.anvil_world {
                let text=mc121_rust::player_data::uuid_string(uuid);
                let path=root.join("playerdata").join(format!("{text}.dat"));
                let stats=root.join("stats").join(format!("{text}.json"));
                if path.try_exists()? { saved=Some(mc121_rust::player_nbt::import_player_file(&path,uuid,Some(&stats))?); }
            }
        }
        let math=players.math_random.as_ref().context("actual global Math.random owner missing")?;
        let constructor_math=[math.next_f64()?,math.next_f64()?,math.next_f64()?];
        let mut player = PlayerState::new(entity_id, profile, position)?;
        let (inventory, stats, lifecycle) = if let Some(saved) = saved {
            saved.apply_to_player(&mut player)?;
            (saved.inventory, saved.stats, saved.lifecycle)
        } else {
            let mut stats = PlayerStats::default();
            if let Some(root) = &server.options.anvil_world {
                let uuid_text = mc121_rust::player_data::uuid_string(uuid);
                let path = root.join("stats").join(format!("{uuid_text}.json"));
                match std::fs::read_to_string(&path) {
                    Ok(json) => stats = PlayerStats::from_current_stat_file_json(&json)?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(error).context("read current-version Java player statistics")
                    }
                }

            }
            (
                inventory::PlayerInventory::new(),
                stats,
                PlayerLifecycle::new("minecraft:overworld", player.gamemode)?,
            )
        };
        ensure!(
            lifecycle.dimension == "minecraft:overworld",
            "saved player dimension is unsupported by this world host"
        );
        let book = main_player_data::load_recipe_book(&lifecycle)?;
        position = player.position;
        metadata = player.metadata;
        if let Some(info) = client_information {
            metadata.skin_parts = info.skin_parts;
            metadata.main_hand = info.main_hand;
        }
        player.metadata = metadata;
        gamemode = player.gamemode;
        login_health = match player.entity_data.get(9) {
            Some(MetadataValue::Float(health)) => *health,
            _ => bail!("restored player health missing"),
        };
        login_lifecycle = lifecycle;
        container.transact(|menu| {
            *menu = PlayerContainer::from_inventory(inventory);
            menu.refresh_crafting(&recipe_context)?;
            Ok(((), vec![]))
        })?;
        if let Some(velocity) = login_lifecycle.motion_velocity.filter(|_| players.gameplay.require_supported_floor().is_ok()) {
            players.player_motion_runtime.register_native(&player,&login_lifecycle,velocity,constructor_math)?;
            players.player_food_runtime.register_native(&player,&login_lifecycle)?;
            players.item_use_runtime.register_native(&player)?;
        }
        native_motion = players.player_motion_runtime.contains(entity_id);
        let event = players.registry.register(player)?;
        let uuid = players
            .registry
            .get(entity_id)
            .context("registered player absent")?
            .profile
            .uuid;
        if let Err(error) =
            players.register_runtime_player(entity_id, position, admission, player_random, uuid)
        {
            players.registry.remove(entity_id);
            return Err(error);
        }
        players.gameplay.install_player_stats(uuid, stats)?;
        players
            .player_lifecycle
            .insert(uuid, login_lifecycle.clone());
        players.recipe_books.insert(uuid, book);
        if login_lifecycle.abilities.flying && login_lifecycle.abilities.may_fly {
            players.flying.insert(entity_id);
        }
        players
            .motion
            .insert(entity_id, PlayerMotionTracker::new(entity_id, position)?);
        if players.gameplay.require_supported_floor().is_ok() {
            container.transact(|menu| Ok(((), menu.enable_local_slot_events()?)))?;
        }
        players.containers.insert(entity_id, container.clone());
        players.disconnects.insert(entity_id, disconnect);
        let observer = players.observer_for(
            entity_id,
            position,
            requested_view_distance,
            server.options.view_distance,
        )?;
        players.observers.insert(entity_id, observer);
        players.publish(event, &server.entity_events);
    }
    drop(admission_guard);
    let mut tracker = ClientTrackers::new(entity_id);
    tracker.set_observer(server.tracking_observer(
        entity_id,
        position,
        requested_view_distance,
    )?)?;
    let view = server.options.view_distance;
    send(
        &codec,
        &mut writer,
        0x2b,
        encoded(|w| {
            w.i32(entity_id)
                .bool(false)
                .varint(1)
                .string("minecraft:overworld")
                .varint(MAX_PLAYERS as i32)
                .varint(view)
                .varint(view)
                .bool(false)
                .bool(true)
                .bool(false)
                .varint(server.fixtures.dimension)
                .string("minecraft:overworld")
                .i64(server.seed_hash)
                .i8(gamemode as i8)
                .u8(255)
                .bool(false)
                .bool(true)
                .bool(false)
                .varint(0)
                .bool(false);
        }),
    )
    .await?;
    if server.options.anvil_world.is_none() {
        send(&codec,&mut writer,0x0b,encoded(|w| {w.u8(server.options.difficulty as u8).bool(false);})).await?;
    }
    let initial_inventory = container.snapshot()?;
    let mut container_revision = initial_inventory.revision;
    send_packets(&codec, &mut writer, vec![initial_inventory.packet]).await?;
    let selected_hotbar = container.read(|menu| menu.inventory.selected_hotbar())?;
    send(
        &codec,
        &mut writer,
        0x53,
        encoded(|w| {
            w.u8(selected_hotbar as u8);
        }),
    )
    .await?;
    let initial_world = server.world_snapshot()?;
    tracker.world_revision = Some(initial_world.revision);
    send(
        &codec,
        &mut writer,
        0x64,
        initial_world.time_payload.to_vec(),
    )
    .await?;
    send(
        &codec,
        &mut writer,
        0x56,
        encoded(|writer| {
            writer.i64(pack_position(8, 64, 8)).f32(0.);
        }),
    )
    .await?;
    send_packets(&codec, &mut writer, initial_world.weather_packets(false)).await?;
    send(
        &codec,
        &mut writer,
        0x38,
        encoded(|w| {
            let abilities = &login_lifecycle.abilities;
            let flags = u8::from(abilities.invulnerable)
                | (u8::from(abilities.flying) << 1)
                | (u8::from(abilities.may_fly) << 2)
                | (u8::from(abilities.instant_build) << 3);
            w.u8(flags)
                .f32(abilities.flying_speed)
                .f32(abilities.walking_speed);
        }),
    )
    .await?;
    send(
        &codec,
        &mut writer,
        0x22,
        encoded(|w| {
            w.u8(13).f32(0.0);
        }),
    )
    .await?;
    send(
        &codec,
        &mut writer,
        0x5d,
        encoded(|w| {
            w.f32(login_health)
                .varint(login_lifecycle.food.level)
                .f32(login_lifecycle.food.saturation);
        }),
    )
    .await?;
    send(
        &codec,
        &mut writer,
        0x5c,
        encoded(|w| {
            w.f32(login_lifecycle.experience.progress)
                .varint(login_lifecycle.experience.level)
                .varint(login_lifecycle.experience.total);
        }),
    )
    .await?;
    let mut loaded = BTreeSet::new();
    send_packets(&codec, &mut writer, server.recipe_login_packets(entity_id)?).await?;
    let login_center = (
        (position.x.floor() as i32).div_euclid(16),
        (position.z.floor() as i32).div_euclid(16),
    );
    server.queue_chunk_view(entity_id, login_center, chunk_radius, true)?;
    if native_motion {
        // Native placement waits for actual current FULL spawn sources before
        // inviting client motion. Normal level polling remains their owner.
        let feet=Vec3::new(position.x,position.y,position.z);
        let dimensions=player_physics::player_dimensions(metadata.pose,1.)?;
        let bounds=Aabb::player(feet,dimensions.width,dimensions.height).inflate(Vec3::new(17.,17.,17.));
        let min=((bounds.min_x.floor() as i32).div_euclid(16),(bounds.min_z.floor() as i32).div_euclid(16));
        let max=((bounds.max_x.floor() as i32).div_euclid(16),(bounds.max_z.floor() as i32).div_euclid(16));
        let deadline=Instant::now()+CONFIGURATION_DEADLINE;
        loop {
            let ready={
                let world=server.world.read().await;
                let players=server.players.read().map_err(|_|anyhow!("player registry lock poisoned"))?;
                players.chunk_runtime.certify_current_native_full(&world,min,max).is_ok()
            };
            if ready {break;}
            time::timeout_at(deadline,tick_events.changed()).await.context("native initial FULL chunks timed out")??;
        }
    }
    send(
        &codec,
        &mut writer,
        0x40,
        encoded(|w| {
            w.f64(position.x)
                .f64(position.y)
                .f64(position.z)
                .f32(position.yaw)
                .f32(position.pitch)
                .u8(0)
                .varint(1);
        }),
    )
    .await?;

    let (tx, mut rx) = mpsc::channel::<Result<Packet>>(32);
    if native_motion {
        server.players.write().map_err(|_|anyhow!("player registry lock poisoned"))?.native_body_ready.insert(entity_id);
    }
    let read_codec = codec.clone();
    let _reader = ReaderGuard(tokio::spawn(async move {
        loop {
            let packet =
                match time::timeout(Duration::from_secs(60), read_codec.read(&mut reader)).await {
                    Ok(Ok(Some(packet))) => Ok(packet),
                    Ok(Ok(None)) => break,
                    Ok(Err(error)) => {
                        let _ = tx.send(Err(error)).await;
                        break;
                    }
                    Err(_) => {
                        let _ = tx.send(Err(anyhow!("play read deadline"))).await;
                        break;
                    }
                };
            if tx.send(packet).await.is_err() {
                break;
            }
        }
    }));
    let mut keepalive = KeepAlive::new(server.epoch.elapsed().as_millis() as i64);
    let mut teleport_confirmed = false;
    let teleport_deadline = Instant::now() + Duration::from_secs(15);
    let mut center = login_center;
    println!(
        "{}",
        json!({"event":"play_join","username":username,"entity_id":entity_id})
    );
    loop {
        // Observe cancellation before another queued input, while retaining
        // fair selection between the connection's normal input/output queues.
        let cancellation = *disconnect_events.borrow_and_update();
        if let Some(reason) = cancellation {
            play_disconnect(&codec, &mut writer, reason).await?;
            return Ok(());
        }
        tokio::select! {
            changed = disconnect_events.changed() => {
                changed?;
                let reason=*disconnect_events.borrow_and_update();
                if let Some(reason)=reason {
                    play_disconnect(&codec,&mut writer,reason).await?;
                    return Ok(());
                }
            }
            tick = tick_events.changed() => {
                tick?;
                if !teleport_confirmed && Instant::now() >= teleport_deadline { bail!("initial teleport was not acknowledged"); }
                let now=*tick_events.borrow_and_update();
                if let Some(action)=keepalive.tick(now) {
                    match action {
                        KeepAliveAction::Challenge(id)=>send(&codec, &mut writer, 0x26, encoded(|w| { w.i64(id); })).await?,
                        KeepAliveAction::Timeout=>{ play_disconnect(&codec,&mut writer,"disconnect.timeout").await?; return Ok(()); },
                    }
                }
            }
            packet = rx.recv() => {
                let Some(packet) = packet else { return Ok(()); };
                let packet = packet?;
                if server.shutting_down.load(Ordering::Acquire) { return Ok(()); }
                let mut fields = Reader::new(&packet.payload);
                match packet.id {
                    0 => {
                        let ack=fields.varint()?; fields.finish()?;
                        if native_motion {
                            let dispatch=server.acknowledge_player_motion_teleport(entity_id,ack,requested_view_distance).await?;
                            position=dispatch.position; teleport_confirmed=dispatch.teleport_confirmed;
                        } else {
                            if ack!=1 {bail!("unexpected teleport acknowledgment");}
                            teleport_confirmed=true;
                        }
                    },
                    0x08 => { let desired=fields.f32()?; fields.finish()?; server.acknowledge_chunk_batch(entity_id,desired)?; },
                    0x09 => { let action=fields.varint()?; fields.finish()?; ensure!((0..=1).contains(&action),"unknown client command"); if action==1 {if let Some(packet)=server.stats_packet(entity_id)? {send_packets(&codec,&mut writer,vec![packet]).await?;}} },
                    0x28 => server.recipe_settings(entity_id, &packet.payload)?,
                    0x29 => server.recipe_seen(entity_id, &packet.payload)?,
                    0x18 => {
                        let id = fields.i64()?; fields.finish()?;
                        match keepalive.receive(server.epoch.elapsed().as_millis() as i64,id) {
                            Ok(Some(latency))=>server.update_latency(entity_id,latency)?,
                            Ok(None)=>{},
                            Err(_)=>{ play_disconnect(&codec,&mut writer,"disconnect.timeout").await?; return Ok(()); },
                        }
                    },
                    0x1a | 0x1b => {
                        let candidate = Vec3::new(fields.f64()?, fields.f64()?, fields.f64()?);
                        let angles = if packet.id == 0x1b {
                            Some((fields.f32()?, fields.f32()?))
                        } else { None };
                        let on_ground = fields.bool()?; fields.finish()?;
                        if native_motion {
                            let dispatch=server.handle_player_motion(entity_id,mc121_rust::player_movement::MovePacket {
                                position:Some(candidate),rotation:angles.map(|(yaw,pitch)|mc121_rust::player_movement::Rotation {yaw,pitch}),on_ground
                            },requested_view_distance).await?;
                            position=dispatch.position;teleport_confirmed=dispatch.teleport_confirmed;
                            center=((position.x.floor() as i32).div_euclid(16),(position.z.floor() as i32).div_euclid(16));
                            continue;
                        }
                        let (yaw,pitch)=angles.unwrap_or((0.0,0.0));
                        if player_physics::contains_invalid_values(candidate,yaw,pitch) {
                            play_disconnect(&codec,&mut writer,"multiplayer.disconnect.invalid_player_movement").await?;
                            return Ok(());
                        }
                        let candidate=player_physics::clamp_position(candidate);
                        if teleport_confirmed {
                            position.x = candidate.x; position.y = candidate.y; position.z = candidate.z; position.on_ground = on_ground;
                            if let Some((yaw, pitch)) = angles { position.yaw = player_physics::wrap_rotation(yaw); position.pitch = player_physics::wrap_rotation(pitch); }
                            let next_center=((position.x.floor() as i32).div_euclid(16),(position.z.floor() as i32).div_euclid(16));
                            let view=(next_center!=center).then_some((next_center,chunk_radius,true));
                            server.move_and_reconcile(entity_id,position,requested_view_distance,view)?;
                            center=next_center;
                        }
                    },
                    0x1c => {
                        let (yaw, pitch) = (fields.f32()?, fields.f32()?);
                        let on_ground = fields.bool()?; fields.finish()?;
                        if native_motion {
                            let dispatch=server.handle_player_motion(entity_id,mc121_rust::player_movement::MovePacket {
                                position:None,rotation:Some(mc121_rust::player_movement::Rotation {yaw,pitch}),on_ground
                            },requested_view_distance).await?;
                            position=dispatch.position;teleport_confirmed=dispatch.teleport_confirmed;
                            continue;
                        }
                        if player_physics::contains_invalid_values(Vec3::ZERO,yaw,pitch) {
                            play_disconnect(&codec,&mut writer,"multiplayer.disconnect.invalid_player_movement").await?;
                            return Ok(());
                        }
                        if teleport_confirmed { position.yaw=player_physics::wrap_rotation(yaw); position.pitch=player_physics::wrap_rotation(pitch); position.on_ground=on_ground; server.move_player(entity_id, position)?; }
                    },
                    0x1d => {
                        let on_ground=fields.bool()?; fields.finish()?;
                        if native_motion {
                            let dispatch=server.handle_player_motion(entity_id,mc121_rust::player_movement::MovePacket {
                                position:None,rotation:None,on_ground
                            },requested_view_distance).await?;
                            position=dispatch.position;teleport_confirmed=dispatch.teleport_confirmed;
                            continue;
                        }
                        if teleport_confirmed { position.on_ground=on_ground; server.move_player(entity_id, position)?; }
                    },
                    0x0a => {
                        let info=read_client_information(&packet.payload)?;
                        requested_view_distance=info.view_distance;
                        let radius=entity_tracking::player_view_distance(requested_view_distance,server.options.view_distance);
                        let view=(radius!=chunk_radius).then_some((center,radius,false));
                        server.move_and_reconcile(entity_id,position,requested_view_distance,view)?;
                        chunk_radius=radius;
                        metadata.skin_parts=info.skin_parts; metadata.main_hand=info.main_hand;
                        server.metadata(entity_id, metadata)?;
                    },
                    0x25 => {
                        let requested_id=fields.varint()?; let action=fields.varint()?; fields.varint()?; fields.finish()?;
                        if requested_id != entity_id { continue; }
                        match action { 0 => metadata.flags |= 0x02, 1 => metadata.flags &= !0x02,
                            3 => metadata.flags |= 0x08, 4 => metadata.flags &= !0x08, _=>{} }
                        server.metadata(entity_id, metadata)?;
                    },
                    0x23 => {
                        let flags=fields.u8()?;fields.finish()?;
                        // The current server game mode grants mayfly. The
                        // packet only requests the flying bit of Abilities.
                        server.set_flying(entity_id,flags&0x02!=0)?;
                    },
                    0x36 => { let hand=fields.varint()?; fields.finish()?; server.animate(entity_id, hand)?; },
                    0x0e => {
                        let click=ContainerClick::decode(&packet.payload)?;
                        if server.click_native_crafting_table(entity_id,&click).await? {continue;}
                        if server.click_native_container(entity_id,&click).await? {continue;}
                        let result=server.execute_menu_action(entity_id,&container,|menu|menu.click(&click,gamemode==1,&recipe_context),None).await;
                        finish_menu_action(result,&container,&codec,&mut writer,&mut container_revision).await?;
                    },
                    0x0f => {
                        let window=fields.u8()?; fields.finish()?;
                        if server.close_native_crafting_table(entity_id,window,false).await? {continue;}
                        if server.close_native_container(entity_id,window,false).await? {continue;}
                        if window==0 {
                            let result=server.execute_menu_action(entity_id,&container,PlayerContainer::close,None).await;
                            finish_menu_action(result,&container,&codec,&mut writer,&mut container_revision).await?;
                        }
                    },
                    0x2f => { let slot=fields.i16()?; fields.finish()?; container.transact(|menu| {menu.inventory.set_selected_hotbar(slot)?;Ok(((),vec![]))})?; },
                    0x32 => {
                        if gamemode != 1 { continue; }
                        let slot=fields.i16()?; let stack=ItemStack::decode_exact(fields.remaining())?;
                        if slot<0 {
                            let result=server.execute_menu_action(entity_id,&container,|_|Ok(ClickOutcome::default()),Some((slot,stack))).await;
                            finish_menu_action(result,&container,&codec,&mut writer,&mut container_revision).await?;
                            continue;
                        }
                        let result=server.execute_menu_action(entity_id,&container,|menu| {
                            let mut packets=Vec::new();
                            match menu.inventory.set_creative_slot(slot, stack)? {
                                CreativeSlotAction::Assigned{slot} => {
                                    packets.push(Packet{id:0x15,payload:menu.inventory.set_slot_payload(slot)?});
                                    menu.record_external_slot_sync(slot)?;
                                    if (1..=4).contains(&slot) {packets.extend(menu.crafting_changed(&recipe_context)?);}else{packets.extend(menu.broadcast_changes()?);}
                                },
                                CreativeSlotAction::Drop(_) | CreativeSlotAction::Ignored => {},
                            }
                            Ok(ClickOutcome{packets,..ClickOutcome::default()})
                        },None).await;
                        finish_menu_action(result,&container,&codec,&mut writer,&mut container_revision).await?;
                    },
                    0x24 => {
                        let action = fields.varint()?; let packed = fields.i64()?; let face = fields.i8()?; let sequence = fields.varint()?; fields.finish()?;
                        if sequence < 0 || !(0..=6).contains(&action) || !(0..=5).contains(&face) { bail!("invalid creative digging request"); }
                        if action==6 {
                            container.transact(|menu| Ok(((),menu.swap_hands()?)))?;
                            continue;
                        }
                        if action==3 || action==4 {
                            let result=server.execute_menu_action(entity_id,&container,|menu|menu.drop_selected(action==3),None).await;
                            finish_menu_action(result,&container,&codec,&mut writer,&mut container_revision).await?;
                            continue;
                        }
                        // Release-use returns before destroy-block acknowledgment.
                        if action==5 {server.release_item_use(entity_id).await?;continue;}
                        let (x, y, z) = unpack_position(packed);
                        let in_reach=can_reach_block(position,server.current_metadata(entity_id)?,(x,y,z));
                        if teleport_confirmed && matches!(action, 0 | 2) && in_reach {
                            let result = server.edit_world_block(entity_id,(x,y,z),0).await?;
                            if result == main_world_interactions::MainInteractionResult::Applied {server.drain_world_interaction_phase().await?;}
                        }
                        server.acknowledge_block_changes_up_to(entity_id,sequence)?;
                    },
                    0x38 => {
                        let hand=fields.varint()?; let target=unpack_position(fields.i64()?); let face=fields.varint()?;
                        let cursor=[fields.f32()?,fields.f32()?,fields.f32()?]; fields.bool()?;
                        let sequence=fields.varint()?; fields.finish()?;
                        if sequence<0 || !(0..=5).contains(&face) || !cursor.iter().all(|v| v.is_finite()) { bail!("invalid placement request"); }
                        if teleport_confirmed && server.use_native_crafting_table(entity_id,hand,target).await? {
                            server.acknowledge_block_changes_up_to(entity_id,sequence)?;
                            continue;
                        }
                        if teleport_confirmed && server.use_native_container(entity_id,hand,target).await? {
                            server.acknowledge_block_changes_up_to(entity_id,sequence)?;
                            continue;
                        }
                        if teleport_confirmed {
                            let direction=[geometry::Direction::Down,geometry::Direction::Up,geometry::Direction::North,geometry::Direction::South,geometry::Direction::West,geometry::Direction::East][face as usize];
                            let hit=Vec3::new(f64::from(target.0)+f64::from(cursor[0]),f64::from(target.1)+f64::from(cursor[1]),f64::from(target.2)+f64::from(cursor[2]));
                            let result=server.interact_world_block(entity_id,hand,target,direction,hit).await?;
                            if result == main_world_interactions::MainInteractionResult::Applied {server.drain_world_interaction_phase().await?;}
                            if result == main_world_interactions::MainInteractionResult::Unsupported {
                                eprintln!("{}",json!({"event":"world_interaction_unsupported","entity_id":entity_id,"target":target,"hand":hand}));
                            }
                        }
                        server.acknowledge_block_changes_up_to(entity_id,sequence)?;
                    },
                    0x39 => {
                        let hand=fields.varint()?; let sequence=fields.varint()?;
                        let yaw=fields.f32()?; let pitch=fields.f32()?; fields.finish()?;
                        if !(0..=1).contains(&hand) || sequence<0 || !yaw.is_finite() || !pitch.is_finite() { bail!("invalid use item request"); }
                        server.acknowledge_block_changes_up_to(entity_id,sequence)?;
                        if teleport_confirmed {
                            position.yaw=player_physics::wrap_rotation(yaw);
                            position.pitch=player_physics::wrap_rotation(pitch);
                            server.move_player(entity_id,position)?;
                            let result=server.use_bucket_item(entity_id,hand).await?;
                            if result == main_world_interactions::MainInteractionResult::Applied {server.drain_world_interaction_phase().await?;}
                            if result == main_world_interactions::MainInteractionResult::Pass {server.request_food_drink_use(entity_id,hand).await?;}
                        }
                    },
                    // Other vanilla messages are bounded by FrameCodec. Their
                    // mechanics are outside this creative compatibility checkpoint.
                    _ => {},
                }
            }
            change = changes.recv() => {
                match change {
                    Ok(WorldUpdate::Block(change)) if loaded.contains(&(change.x.div_euclid(16), change.z.div_euclid(16))) => {
                        send(&codec, &mut writer, 0x09, encoded(|w| { w.i64(pack_position(change.x, change.y, change.z)).varint(change.state as i32); })).await?;
                    },
                    Ok(WorldUpdate::Light{chunk,payload}) if loaded.contains(&chunk) => send(&codec,&mut writer,0x2a,payload).await?,
                    Ok(WorldUpdate::Acknowledge{player,sequence}) if player==entity_id => send(&codec,&mut writer,0x05,encoded(|w|{w.varint(sequence);})).await?,
                    Ok(_) => {},
                    Err(broadcast::error::RecvError::Lagged(_)) => bail!("client fell behind fixture world changes"),
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
            update = entity_events.recv() => {
                match ordered_entity_input(update) {
                    OrderedEntityInput::Event(PlayerEvent::ChunksReady{recipient,cutoff,reconcile}) if recipient==entity_id => flush_chunk_transport(&server,entity_id,(cutoff,reconcile.as_deref()),&mut tracker,&codec,&mut writer,&mut loaded).await?,
                    OrderedEntityInput::Event(PlayerEvent::ObserverReconcile{recipient,reconcile}) if recipient==entity_id => send_packets(&codec,&mut writer,tracker.apply_reconcile(&reconcile,&loaded)?).await?,
                    OrderedEntityInput::Event(PlayerEvent::ObserverReconcile{..})=>{},
                    OrderedEntityInput::Event(PlayerEvent::ChunksReady{..})=>{},
                    OrderedEntityInput::Event(PlayerEvent::World {revision,packets})=>send_packets(&codec,&mut writer,tracker.world_event(revision,&packets)).await?,
                    OrderedEntityInput::Event(PlayerEvent::WorldAudience {recipients,packets})=> {
                        if recipients.contains(&entity_id) { send_packets(&codec,&mut writer,packets.as_ref().clone()).await?; }
                    },
                    OrderedEntityInput::Event(PlayerEvent::Item(event)) => send_packets(&codec,&mut writer,tracker.item_event(entity_id,&event,&loaded)?).await?,
                    OrderedEntityInput::Event(PlayerEvent::State(update)) => send_packets(&codec, &mut writer, tracker.players.event(&update, &loaded)?).await?,
                    OrderedEntityInput::Event(PlayerEvent::Motion{revision,entity_id,position,packets}) => send_packets(&codec, &mut writer, tracker.players.motion(revision, entity_id, position, &packets)?).await?,
                    OrderedEntityInput::Event(PlayerEvent::Latency(players))=>send_packets(&codec,&mut writer,vec![entity::player_info_update(&players,0x10)?]).await?,
                    OrderedEntityInput::Event(PlayerEvent::Container{recipient,update}) if recipient==entity_id && update.revision>container_revision => {
                        container_revision=update.revision;
                        send_packets(&codec,&mut writer,update.packets.clone()).await?;
                    },
                    OrderedEntityInput::Event(PlayerEvent::Container{..}) => {},
                    OrderedEntityInput::DisconnectLostHistory{skipped} => {
                        // A snapshot cannot recreate an absent Collect history.
                        // Stop explicitly rather than send Destroy before it.
                        eprintln!("{}",json!({"event":"ordered_entity_history_lost","entity_id":entity_id,"skipped":skipped}));
                        play_disconnect(&codec,&mut writer,"disconnect.disconnected").await?;
                        return Ok(());
                    },
                    OrderedEntityInput::Closed => return Ok(()),
                }
            }
        }
    }
}

fn item_query_chunk_range(
    state: &mc121_rust::entity_state::EntityPhysicalState,
) -> Result<((i32, i32), (i32, i32))> {
    ensure!(
        [state.velocity.x, state.velocity.y, state.velocity.z]
            .into_iter()
            .all(|value| value.is_finite() && value.abs() <= 4.),
        "item velocity exceeds the supported ordinary-floor query envelope"
    );
    // One collision-neighbour block plus the real16-block game-event radius,
    // around the current swept bounds. This includes baseTick/support/merge
    // probes; this unedited dry source never enters the water ray-cache path.
    let bounds = state
        .bounds
        .expand_towards(state.velocity)
        .inflate(Vec3::new(17.000001, 0., 17.000001));
    let floor = mc121_rust::entity_state::java_floor;
    Ok((
        (
            floor(bounds.min_x).div_euclid(16),
            floor(bounds.min_z).div_euclid(16),
        ),
        (
            floor(bounds.max_x).div_euclid(16),
            floor(bounds.max_z).div_euclid(16),
        ),
    ))
}

fn can_reach_block(position: Position, metadata: PlayerMetadata, block: (i32, i32, i32)) -> bool {
    let eye_height = player_physics::player_dimensions(metadata.pose, 1.)
        .expect("validated player pose")
        .eye_height;
    let eyes =
        geometry::player_eye_position(Vec3::new(position.x, position.y, position.z), eye_height);
    geometry::can_interact_with_block(
        eyes,
        block,
        geometry::CREATIVE_BLOCK_INTERACTION_RANGE,
        geometry::INTERACTION_VERIFICATION_BUFFER,
    )
}

fn pack_position(x: i32, y: i32, z: i32) -> i64 {
    ((i64::from(x) & 0x3ffffff) << 38) | ((i64::from(z) & 0x3ffffff) << 12) | (i64::from(y) & 0xfff)
}
fn unpack_position(packed: i64) -> (i32, i32, i32) {
    (
        (packed >> 38) as i32,
        ((packed << 52) >> 52) as i32,
        ((packed << 26) >> 38) as i32,
    )
}

async fn flush_chunk_transport(
    server: &Server,
    entity_id: i32,
    delivery: (u64, Option<&ObserverReconcile>),
    tracker: &mut ClientTrackers,
    codec: &FrameCodec,
    writer: &mut NetworkWriter,
    loaded: &mut BTreeSet<(i32, i32)>,
) -> Result<()> {
    let (cutoff, reconcile) = delivery;
    let updates = server.drain_chunk_transport(entity_id, cutoff)?;
    for update in updates {
        send_packets(codec, writer, update.packets).await?;
        // Mark transport eligibility only after the real batch write finishes.
        // Neither this loaded set nor pending membership grants world ticking.
        *loaded = update.loaded;
        tracker.players.set_pending_chunks(update.pending.clone());
        tracker.items.set_pending_chunks(update.pending);
    }
    if let Some(reconcile) = reconcile {
        send_packets(codec, writer, tracker.apply_reconcile(reconcile, loaded)?).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tools/main_item_plumbing_tests.rs"]
mod item_plumbing_tests;

#[cfg(test)]
#[path = "../tools/observer_reconcile_tests.rs"]
mod observer_reconcile_tests;

#[cfg(test)]
#[path = "../tools/main_persistence_smoke.rs"]
mod persistence_smoke;
