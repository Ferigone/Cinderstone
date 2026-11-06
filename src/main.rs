//! Native protocol-767 server under differential validation against vanilla 1.21.

use anyhow::{anyhow, bail, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use mc121_rust::{
    auth::{valid_login_name, Authenticator, CryptoRead, CryptoWrite, SessionVerifier},
    border::WorldBorder,
    chunk_lifecycle::FullStatus,
    connection_cookies::{CookieAction, CookieConnection, CookiePhase, UNEXPECTED_COOKIE_RESPONSE_REASON},
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
    item_host::{ExternalMenuPickupResult, RuntimeInventoryHost},
    level::LevelData,
    metadata::MetadataValue,
    nbt::Tag,
    placement::{self, PlacementContext},
    player_data::{PlayerData, PlayerDataStore, PlayerLifecycle},
    player_inventory::{ContainerUpdate, SharedContainer},
    player_physics,
    player_stats::PlayerStats,
    protocol::{FrameCodec, Packet, Reader, Writer},
    reconfiguration::Reconfiguration,
    resource_packs::{ResourcePackOffer, ResourcePackPhase, ResourcePackSession},
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
        atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering},
        Arc, RwLock as StdRwLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs,
    io::AsyncWriteExt,
    net::{
        tcp::OwnedWriteHalf,
        TcpListener, TcpStream,
    },
    sync::{broadcast, mpsc, watch, Mutex, RwLock, Semaphore},
    task::JoinHandle,
    time::{self, Instant},
};

mod main_chunk_runtime;
mod main_border_login;
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
mod main_furnaces;
mod main_furnace_removal;
mod main_container_removal;
mod main_shutdown_chunks;
mod main_furnace_notifications;
mod main_server_timing;
mod main_experience;
mod main_falling_block;
mod main_falling_block_generic;
mod main_falling_block_powder;
mod main_concrete_powder_placement;
mod main_falling_block_sequence;
mod main_falling_block_clip;
mod main_falling_block_clip_admission;
mod main_block_state_warnings;
mod main_native_enchantments;
mod main_block_state_registry;
mod main_falling_block_world;
mod main_falling_block_runtime;
mod main_falling_block_persistence;
mod main_falling_block_spawn;
mod main_falling_block_physics;
mod main_falling_block_redstone;
mod main_pressure_plate_entities;
mod main_falling_block_damage;
mod main_falling_block_dispatch;
mod main_block_ticks;
mod main_redstone;
mod main_player_motion_hooks;
mod main_native_source;
mod main_rain_context;
mod main_random_block_ticks;
mod main_world_interactions;
mod main_bucket_interactions;
mod main_configuration;
mod main_administration;
mod main_admin_dispatch;
mod main_scoreboard;
mod main_chat;
mod main_chat_dispatch;
mod main_chat_keys;
mod main_connection;
mod main_connection_rate;
mod main_connection_decode;
mod main_stonecutter;
mod main_stonecutter_recipes;
mod main_stonecutter_dispatch;
mod main_stonecutter_lifecycle;
mod main_stonecutter_pickup;
mod main_workstation_components;
mod main_menu_resync;
mod main_smithing;
mod main_smithing_recipes;
mod main_smithing_dispatch;
mod main_smithing_lifecycle;
mod main_smithing_pickup;
mod main_loom;
mod main_loom_patterns;
mod main_loom_callbacks;
mod main_loom_dispatch;
mod main_loom_lifecycle;
mod main_loom_pickup;
mod main_grindstone;
mod main_grindstone_recipes;
mod main_grindstone_dispatch;
mod main_grindstone_lifecycle;
mod main_grindstone_pickup;
mod main_anvil;
mod main_anvil_context;
mod main_anvil_dispatch;
mod main_anvil_lifecycle;
mod main_anvil_pickup;
use main_chunk_runtime::MainChunkRuntime;
use main_chunk_transport::{ChunkTransportUpdate, SessionChunkTransport};
use main_drop_commit::{menu_action_with_items, DropCommitCallbacks, DropCommitOwner};
use main_item_gameplay::{GameplaySourceScope, MainItemGameplay};

const PROTOCOL: i32 = 767;
const CONFIGURATION_DEADLINE: Duration = Duration::from_secs(15);
const WRITE_DEADLINE: Duration = Duration::from_secs(15);
type NetworkReader = CryptoRead<mc121_rust::connection_transport::RawReadStream>;
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
    resource_pack: Option<ResourcePackOffer>,
    resource_pack_required: bool,
    server_directory: Option<PathBuf>,
    max_players: i32,
    enforce_whitelist: bool,
    op_permission_level: i32,
    enforce_secure_profile: bool,
    secure_profile_override: Option<bool>,
    rate_limit: Option<i32>,
    compression_threshold: Option<i32>,
    loopback_service_keys: Option<PathBuf>,
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
            resource_pack: None,
            resource_pack_required: false,
            server_directory: None,
            max_players: 20,
            enforce_whitelist: false,
            op_permission_level: 4,
            enforce_secure_profile: true,
            secure_profile_override: None,
            rate_limit: None,
            compression_threshold: None,
            loopback_service_keys: None,
        };
        let mut pack_url = String::new();
        let mut pack_hash = String::new();
        let mut pack_id = None;
        let mut pack_prompt = None;
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
            if argument == "--require-resource-pack" {
                options.resource_pack_required = true;
                continue;
            }
            if argument == "--enforce-whitelist" {
                options.enforce_whitelist = true;
                continue;
            }
            if argument == "--help" {
                println!("mc121-rust (--online-mode | --allow-offline [--allow-public-offline]) [--bind 127.0.0.1] [--port 25565] [--fixtures fixtures/configuration.json] [--world runtime/world.json | --anvil-world /path/to/world] [--view-distance 2..6] [--loopback-service-keys PATH (offline loopback only)]");
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
                "--server-directory" => options.server_directory = Some(value.into()),
                "--max-players" => options.max_players = value.parse().context("invalid maximum players")?,
                "--op-permission-level" => options.op_permission_level = value.parse().context("invalid operator permission level")?,
                "--enforce-secure-profile" => options.secure_profile_override = Some(value.parse().context("enforce-secure-profile must be true or false")?),
                "--rate-limit" => options.rate_limit = Some(value.parse().context("invalid packet rate limit")?),
                "--compression-threshold" => options.compression_threshold = Some(value.parse().context("invalid compression threshold")?),
                "--loopback-service-keys" => options.loopback_service_keys = Some(value.into()),
                "--max-chained-neighbor-updates" => options.max_chained_neighbor_updates = value.parse().context("invalid neighbor update limit")?,
                "--anvil-world" => options.anvil_world = Some(value.into()),
                "--resource-pack-url" => pack_url = value,
                "--resource-pack-sha1" => pack_hash = value,
                "--resource-pack-id" => pack_id = Some(parse_pack_id(&value)?),
                "--resource-pack-prompt" => pack_prompt = Some(value),
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
        main_chat_keys::validate_source_options(&options)?;
        if !(2..=6).contains(&options.view_distance) {
            bail!("view distance must be 2..=6");
        }
        ensure!((0..=4).contains(&options.op_permission_level), "operator permission level must be 0..4");
        if !pack_url.is_empty() {
            options.resource_pack = Some(ResourcePackOffer::from_server_config(
                pack_id, pack_url, pack_hash, options.resource_pack_required, pack_prompt,
            )?);
        }
        // MinecraftServer.isResourcePackRequired filters the actual optional
        // offer; a required property without a URL does not require a pack.
        options.resource_pack_required = options.resource_pack.as_ref().is_some_and(|pack| pack.required);
        Ok(options)
    }
}

fn parse_pack_id(value: &str) -> Result<[u8; 16]> {
    let compact = value.replace('-', "");
    ensure!(compact.len() == 32 && compact.bytes().all(|b| b.is_ascii_hexdigit()), "invalid resource-pack UUID");
    let mut uuid = [0; 16];
    for (index, byte) in uuid.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&compact[index * 2..index * 2 + 2], 16)?;
    }
    Ok(uuid)
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
    chat_type: i32,
    configuration: Vec<Packet>,
    block_state_registry: main_block_state_registry::NativeBlockStateRegistry,
    dimension: i32,
    dimension_facts: DimensionFacts,
    seed_hash: i64,
}

#[derive(Clone, Copy)]
struct ClientInformation {
    chat_visibility: i32,
    text_filtering: bool,
    view_distance: i32,
    skin_parts: u8,
    main_hand: u8,
}

enum PlayExit {
    Disconnected,
    Reconfigure(Option<ClientInformation>),
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
    let chat_type = main_chat_keys::captured_chat_type(&configuration)?;
    let block_state_registry = main_block_state_registry::NativeBlockStateRegistry::capture(
        &configuration,
        inventory::block_state_catalog(),
        mc121_rust::placement::canonical_placement_catalog(),
    )?;
    Ok(Fixtures {
        chat_type,
        configuration,
        block_state_registry,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    experience_orbs: Vec<mc121_rust::experience::SavedExperienceOrb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    block_ticks: Option<main_block_ticks::SavedBlockRuntime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    level_random_position_value: Option<i32>,
}
struct Server {
    options: Options,
    connection_settings: mc121_rust::connection_settings::DedicatedConnectionSettings,
    connection_rates: StdRwLock<BTreeMap<u64, main_connection_rate::ConnectionRate>>,
    fixtures: Fixtures,
    seed_hash: i64,
    world: RwLock<WorldBackend>,
    save_lock: Mutex<()>,
    player_admission: Mutex<()>,
    administration: StdRwLock<main_administration::Administration>,
    connection_ticks: watch::Sender<u64>,
    chat_keys: watch::Sender<Arc<mc121_rust::chat_crypto::ServiceKeySet>>,
    chat_keys_attempted: watch::Sender<bool>,
    chat_dispatch: Mutex<()>,
    entities: AtomicI32,
    connection_sequence: AtomicU64,
    configuration_requests: broadcast::Sender<i32>,
    changes: broadcast::Sender<WorldUpdate>,
    authenticator: Option<Authenticator>,
    session_verifier: Option<SessionVerifier>,
    players: StdRwLock<LivePlayers>,
    entity_events: broadcast::Sender<PlayerEvent>,
    clock: StdRwLock<TickClock>,
    tick_events: watch::Sender<i64>,
    epoch: Instant,
    level: Option<Mutex<LevelData>>,
    difficulty_locked: bool,
    border: StdRwLock<WorldBorder>,
    player_store: PlayerDataStore,
    entity_store: EntityDataStore,
    shutting_down: AtomicBool,
}

struct LivePlayers {
    scoreboard_revision: u64,
    scoreboard_trackers: BTreeMap<i32, mc121_rust::scoreboard::hooks::PlayerCriteriaTracker>,
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
    furnaces: main_furnaces::FurnaceRuntime,
    stonecutters: main_stonecutter::StonecutterRuntime,
    smithing_tables: main_smithing::SmithingRuntime,
    looms: main_loom::LoomRuntime,
    grindstones: main_grindstone::GrindstoneRuntime,
    anvils: main_anvil::NativeAnvilRuntime,
    experience_runtime: main_experience::MainExperienceRuntime,
    falling_blocks: main_falling_block_runtime::MainFallingBlockRuntime,
    block_runtime: main_block_ticks::MainBlockRuntime,
    redstone_runtime: main_redstone::MainRedstoneRuntime,
    containers: BTreeMap<i32, SharedContainer>,
    disconnects: BTreeMap<i32, watch::Sender<Option<&'static str>>>,
    connection_addresses: BTreeMap<i32, SocketAddr>,
    chat: main_chat::MainChatRuntime,
    player_last_actions: BTreeMap<i32, i64>,
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
    player_removal_progress: BTreeMap<i32, PlayerRemovalProgress>,
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
    Scoreboard { revision: u64, recipients: Arc<Vec<i32>>, packets: Arc<Vec<Packet>> },
    AdminDisconnect { recipient: i32, reason: Vec<u8> },
    ChatDisconnect { recipient: i32, reason: &'static str },
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
            scoreboard_revision: 0,
            scoreboard_trackers: BTreeMap::new(),
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
            furnaces: main_furnaces::FurnaceRuntime::default(),
            stonecutters: main_stonecutter::StonecutterRuntime::default(),
            smithing_tables: main_smithing::SmithingRuntime::default(),
            looms: main_loom::LoomRuntime::default(),
            grindstones: main_grindstone::GrindstoneRuntime::default(),
            anvils: main_anvil::NativeAnvilRuntime::default(),
            experience_runtime: main_experience::MainExperienceRuntime::default(),
            falling_blocks: main_falling_block_runtime::MainFallingBlockRuntime::default(),
            block_runtime: main_block_ticks::MainBlockRuntime::new(fresh_unedited),
            redstone_runtime: main_redstone::MainRedstoneRuntime::new(fresh_unedited),
            containers: BTreeMap::new(),
            disconnects: BTreeMap::new(),
            connection_addresses: BTreeMap::new(),
            chat: main_chat::MainChatRuntime::default(),
            player_last_actions: BTreeMap::new(),
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
            player_removal_progress: BTreeMap::new(),
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
        self.experience_runtime.initialize_player_cache(id);
        self.creative_drop.insert(id, CreativeDropGate::default());
        self.chunk_transports
            .insert(id, SessionChunkTransport::new());
        Ok(())
    }
    fn remove_runtime_player(&mut self, id: i32) -> Result<()> {
        self.scoreboard_trackers.remove(&id);
        self.experience_runtime.forget_player(id);
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

#[derive(Clone, Copy)]
enum PlayerInventorySaveView {
    NativeResume,
    SourcePlayerInventory,
}

#[derive(Clone, Copy)]
enum RemovalLeaveGameOutcome { Awarded, LegacyUnsupported }

/// One actual removal operation, retained across the existing error retries.
/// The save proposal is immutable serialization data, not another live menu.
struct PlayerRemovalProgress {
    uuid: [u8; 16],
    handle: Option<mc121_rust::runtime_entity_index::EntityHandle>,
    leave_game: Option<RemovalLeaveGameOutcome>,
    source_save: Option<PlayerData>,
    inventory_removed: bool,
    external_removed: bool,
    retirement_started: bool,
}

impl Server {
    fn capture_player_locked(
        &self,
        players: &mut LivePlayers,
        id: i32,
    ) -> Result<Option<PlayerData>> {
        if let Some(removal) = players.player_removal_progress.get(&id) {
            let player = players.registry.get(id).context("removal save actor is absent")?;
            ensure!(player.profile.uuid == removal.uuid, "removal save actor UUID changed");
            return Ok(Some(removal.source_save.clone()
                .context("actual player removal has no completed source-save proposal")?));
        }
        self.capture_player_with_view_locked(players, id, PlayerInventorySaveView::NativeResume)
    }
    fn capture_player_with_view_locked(
        &self,
        players: &mut LivePlayers,
        id: i32,
        view: PlayerInventorySaveView,
    ) -> Result<Option<PlayerData>> {
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(None);
        };
        let uuid = player.profile.uuid;
        let inventory = players
            .containers
            .get(&id)
            .context("saving player has no container")?
            .read(|menu| match view {
                PlayerInventorySaveView::NativeResume => menu.inventory.clone(),
                PlayerInventorySaveView::SourcePlayerInventory => menu.inventory.snapshot_for_player_save(),
            })?;
        let inventory = match view {
            PlayerInventorySaveView::NativeResume => {
                let inventory=players.native_containers.inventory_snapshot(id,&inventory);
                let inventory=players.crafting_tables.inventory_snapshot(id,&inventory);
                let inventory=players.furnaces.inventory_snapshot(id,&inventory);
                let inventory=players.stonecutters.inventory_snapshot(id,&inventory);
                let inventory=players.smithing_tables.inventory_snapshot(id,&inventory);
                let inventory=players.looms.inventory_snapshot(id,&inventory);
                let inventory=players.grindstones.inventory_snapshot(id,&inventory);
                players.anvils.inventory_snapshot(id,&inventory)
            }
            PlayerInventorySaveView::SourcePlayerInventory => inventory,
        };
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
        self.persist_player_proposal_locked(players, data)
    }
    fn persist_player_proposal_locked(&self, players: &mut LivePlayers, data: PlayerData) -> Result<()> {
        let uuid = data.uuid;
        // Retain the newest authoritative snapshot if disk I/O fails. A later
        // autosave or rejoin retries this owner rather than reading stale disk.
        players.pending_player_saves.insert(uuid, data);
        self.persist_player_data(players.pending_player_saves.get(&uuid).unwrap())?;
        players.pending_player_saves.remove(&uuid);
        Ok(())
    }

    fn player_runtime_handle(players: &LivePlayers, id: i32)
        -> Option<mc121_rust::runtime_entity_index::EntityHandle> {
        // Registration and generation survive hidden/non-ticking readiness.
        // A tick-list scan must not turn the same real Player into None here.
        players.item_runtime.entity_handle(id).filter(|handle|
            players.item_runtime.entity_kind(*handle)
                == Some(mc121_rust::runtime_entity_index::EntityKind::Player))
    }

    /// PlayerList.remove awards leave_game and saves BEFORE Player.remove.
    /// Store each completed source phase so I/O/removal retries cannot repeat
    /// the award or replace this snapshot with a post-drop menu capture.
    fn save_player_before_removal_locked(&self, players: &mut LivePlayers, id: i32) -> Result<()> {
        let uuid = players.registry.get(id).context("removal player is absent")?.profile.uuid;
        let handle = Self::player_runtime_handle(players, id);
        if let Some(removal) = players.player_removal_progress.get(&id) {
            ensure!(removal.uuid == uuid, "removal actor UUID changed");
            ensure!(removal.handle == handle || (removal.retirement_started && handle.is_none()),
                "removal actor generation changed");
        } else {
            // This marks the actual leaving actor before save/removal. The
            // close calls below carry its disconnected source context even
            // for EOF and same-TCP reconfiguration without a reason packet.
            players.player_removal_progress.insert(id, PlayerRemovalProgress {
                uuid, handle, leave_game: None, source_save: None,
                inventory_removed: false, external_removed: false, retirement_started: false,
            });
        }
        if players.player_removal_progress.get(&id).expect("actual removal marker").leave_game.is_none() {
            let outcome = match players.gameplay.require_supported_floor() {
                Ok(()) => {
                    let key = mc121_rust::player_stats::stat_key("minecraft:custom", "minecraft:leave_game")
                        .context("source leave_game statistic is unavailable")?;
                    players.gameplay.award_player_stat(id, uuid, key, 1)?;
                    RemovalLeaveGameOutcome::Awarded
                }
                Err(error) => {
                    // Preserve the existing transport-only cleanup when the
                    // actual imported/legacy callback owner is unavailable.
                    // This is not a successful vanilla statistic callback.
                    eprintln!("{}", json!({"event":"native_player_removal_stat_unsupported",
                        "entity_id":id,"reason":format!("{error:#}"),"award_completed":false}));
                    RemovalLeaveGameOutcome::LegacyUnsupported
                }
            };
            players.player_removal_progress.get_mut(&id).expect("actual removal marker").leave_game = Some(outcome);
        }
        if players.player_removal_progress.get(&id).expect("actual removal marker").source_save.is_none() {
            let data = self.capture_player_with_view_locked(players, id, PlayerInventorySaveView::SourcePlayerInventory)?
                .context("actual removal player disappeared before source save")?;
            players.player_removal_progress.get_mut(&id).expect("actual removal marker").source_save = Some(data);
        }
        let data = players.player_removal_progress.get(&id).expect("actual removal marker")
            .source_save.as_ref().expect("captured source-removal save").clone();
        self.persist_player_proposal_locked(players, data)
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
    /// handleContainerClose targets the actual current server menu. Select and
    /// remove that owner in one operation, never across separate async guards
    /// after remembering an earlier window ID.
    async fn close_current_player_menu(&self, id: i32) -> Result<()> {
        let _operation = self.chat_dispatch.lock().await;
        let now = self.clock.read().map_err(|_| anyhow!("clock poisoned"))?.game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        enum CurrentMenu {
            Furnace(u8),
            Stonecutter(u8),
            Smithing(u8),
            Loom(u8),
            Grindstone(u8),
            Anvil(u8),
            CraftingTable(u8),
            NativeContainer(u8),
        }
        let mut current = [
            players.furnaces.window_id(id).map(CurrentMenu::Furnace),
            players.stonecutters.window_id(id).map(CurrentMenu::Stonecutter),
            players.smithing_tables.window_id(id).map(CurrentMenu::Smithing),
            players.looms.window_id(id).map(CurrentMenu::Loom),
            players.grindstones.window_id(id).map(CurrentMenu::Grindstone),
            players.anvils.window_id(id).map(CurrentMenu::Anvil),
            players.crafting_tables.window_id(id).map(CurrentMenu::CraftingTable),
            players.native_containers.window_id(id).map(CurrentMenu::NativeContainer),
        ].into_iter().flatten();
        let selected = current.next();
        ensure!(current.next().is_none(), "multiple current player menus during close");
        let closed = match selected {
            Some(CurrentMenu::Furnace(window)) => self.furnace_menu_action_locked(
                &mut world, &mut players, id, None, Some(window), false, now, border,
            )?,
            Some(CurrentMenu::Stonecutter(window)) => self.close_stonecutter_with_context_locked(
                &mut world, &mut players, id, window, false, now, border,
                mc121_rust::container::MenuCloseContext::AliveConnected,
            )?,
            Some(CurrentMenu::Loom(window)) => self.close_loom_with_context_locked(
                &mut world, &mut players, id, window, false, now, border,
                mc121_rust::container::MenuCloseContext::AliveConnected,
            )?,
            Some(CurrentMenu::Grindstone(window)) => self.close_grindstone_with_context_locked(
                &mut world, &mut players, id, window, false, now, border,
                mc121_rust::container::MenuCloseContext::AliveConnected,
            )?,
            Some(CurrentMenu::Anvil(window)) => {
                let context = main_anvil_lifecycle::connected_anvil_close_context(&players, id)?;
                self.close_anvil_with_context_locked(
                    &mut world, &mut players, id, window, false, now, border, context,
                )?
            }
            Some(CurrentMenu::Smithing(window)) => self.close_smithing_with_context_locked(
                &mut world, &mut players, id, window, false, now, border,
                mc121_rust::container::MenuCloseContext::AliveConnected,
            )?,
            Some(CurrentMenu::CraftingTable(window)) => self.crafting_table_action_locked(
                &mut world, &mut players, id, None, Some(window), false, now, border,
            )?,
            Some(CurrentMenu::NativeContainer(window)) => self.native_menu_action_locked(
                &mut world, &mut players, id, None, Some(window), false, now, border,
            )?,
            None => return self.close_inventory_menu_locked(
                &mut world, &mut players, id, now, border,
            ),
        };
        ensure!(closed, "actual current player menu changed during close");
        Ok(())
    }

    async fn execute_menu_action(
        &self,
        id: i32,
        container: &SharedContainer,
        operation: impl FnOnce(&mut PlayerContainer) -> Result<ClickOutcome>,
        creative_attempt: Option<(i16, ItemStack)>,
    ) -> Result<()> {
        self.execute_menu_action_with_candidate(
            id, container, operation, creative_attempt, None,
        ).await
    }

    async fn execute_menu_action_with_candidate(
        &self,
        id: i32,
        container: &SharedContainer,
        operation: impl FnOnce(&mut PlayerContainer) -> Result<ClickOutcome>,
        creative_attempt: Option<(i16, ItemStack)>,
        candidate: Option<ItemStack>,
    ) -> Result<()> {
        let _operation = self.chat_dispatch.lock().await;
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
        if let Some(candidate) = &candidate {
            if players.anvils.has_open(id) {
                if let Err(error) = main_anvil_context::require_anvil_stack_context(candidate) {
                    if !error.is::<main_anvil_context::UnsupportedAnvilEnchantmentContext>() {
                        return Err(error);
                    }
                    eprintln!("{}", json!({
                        "event":"native_component_admission_refused", "entity_id":id,
                        "packet_id":0x32, "reason":format!("{error:#}"),
                    }));
                    // Selection, refusal and genuine retained-state recovery
                    // share the same actual operation/world/player interval.
                    self.resync_native_anvil_locked(
                        &mut world, &mut players, id, game_time, border,
                    )?;
                    return Ok(());
                }
            }
        }
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
        let staged_stats = players.gameplay.stats_mut(uuid)?.for_transaction();
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
        let _operation = self.chat_dispatch.lock().await;
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
        players.simulate_native_entities_with_player_menus(&mut world, game_time, border, &self.entity_events, Some(&self.changes),
            |players, world, id| {
                self.tick_native_furnace_menu_for_player_locked(world, players, id, game_time, border)?;
                self.tick_native_stonecutter_menu_for_player_locked(world, players, id, game_time, border)?;
                self.tick_native_smithing_menu_for_player_locked(world, players, id, game_time, border)?;
                self.tick_native_loom_menu_for_player_locked(world, players, id, game_time, border)?;
                self.tick_native_grindstone_menu_for_player_locked(world, players, id, game_time, border)?;
                self.tick_native_anvil_menu_for_player_locked(world, players, id, game_time, border)
            },
            |players, world, handle| {
                self.tick_native_falling_block_locked(world, players, handle, game_time, border)
            })
    }

    async fn tick_connection_phase(&self) -> Result<()> {
        let _operation = self.chat_dispatch.lock().await;
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
                    match players.tick_native_player_body(id,&mut world,game_time,border,&self.entity_events) {
                        Ok(()) => players.finish_native_experience_player_tick(id, &self.entity_events)?,
                        Err(error) => eprintln!("{}",json!({"event":"native_player_body_failed","entity_id":id,"reason":format!("{error:#}")})),
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
                    furnaces,
                    stonecutters,
                    smithing_tables,
                    looms,
                    grindstones,
                    anvils,
                    ..
                } = &mut *players;
                let active_furnace = furnaces.has_open(id);
                let active_stonecutter = stonecutters.has_open(id);
                let active_smithing = smithing_tables.has_open(id);
                let active_loom = looms.has_open(id);
                let active_grindstone = grindstones.has_open(id);
                let active_anvil = anvils.has_open(id);
                ensure!(u8::from(active_furnace) + u8::from(active_stonecutter)
                    + u8::from(active_smithing) + u8::from(active_loom)
                    + u8::from(active_grindstone) + u8::from(active_anvil) <= 1,
                    "multiple current external menus during connection pickup");
                let mut frame = gameplay.frame(registry, &chunk_runtime.manager, true);
                let mut host = RuntimeInventoryHost::new(registry, containers, &mut frame);
                let mut furnace_sync = |inventory: &mc121_rust::inventory::PlayerInventory| {
                    furnaces.broadcast_for_inventory(&*world, id, inventory)
                };
                let mut stonecutter_sync = |hooks: &mut main_item_gameplay::MainGameplayFrame<'_>,
                                           inventory: &mc121_rust::inventory::PlayerInventory| {
                    stonecutters.broadcast_for_inventory(id, pickup, inventory, hooks)
                };
                let mut smithing_sync = |hooks: &mut main_item_gameplay::MainGameplayFrame<'_>,
                                        inventory: &mc121_rust::inventory::PlayerInventory| {
                    smithing_tables.broadcast_for_inventory(id, pickup, inventory, hooks)
                };
                let mut loom_sync = |hooks: &mut main_item_gameplay::MainGameplayFrame<'_>,
                                     inventory: &mc121_rust::inventory::PlayerInventory| {
                    looms.broadcast_for_inventory(id, pickup, inventory, hooks)
                };
                let mut grindstone_sync = |hooks: &mut main_item_gameplay::MainGameplayFrame<'_>,
                                           inventory: &mc121_rust::inventory::PlayerInventory| {
                    grindstones.broadcast_for_inventory(id, pickup, inventory, hooks)
                };
                let mut anvil_sync = |hooks: &mut main_item_gameplay::MainGameplayFrame<'_>,
                                      inventory: &mc121_rust::inventory::PlayerInventory,
                                      candidate: Option<&ItemStack>| {
                    if let Some(candidate) = candidate {
                        match anvils.preflight_inventory_pickup(id, pickup, inventory, candidate, hooks) {
                            Ok(()) => Ok(ExternalMenuPickupResult::Admit),
                            Err(error) if error.is::<main_anvil_context::UnsupportedAnvilEnchantmentContext>()
                                || error.is::<main_workstation_components::UnsupportedFoodIdentity>() =>
                                Ok(ExternalMenuPickupResult::Refuse),
                            Err(error) => Err(error),
                        }
                    } else {
                        anvils.broadcast_for_inventory(id, pickup, inventory, hooks)
                            .map(ExternalMenuPickupResult::Packets)
                    }
                };
                if active_anvil {
                    host = host.with_external_menu_sync_and_pickup_preflight(pickup, &mut anvil_sync);
                } else if active_grindstone {
                    host = host.with_external_menu_sync_and_hooks(pickup, &mut grindstone_sync);
                } else if active_loom {
                    host = host.with_external_menu_sync_and_hooks(pickup, &mut loom_sync);
                } else if active_smithing {
                    host = host.with_external_menu_sync_and_hooks(pickup, &mut smithing_sync);
                } else if active_stonecutter {
                    host = host.with_external_menu_sync_and_hooks(pickup, &mut stonecutter_sync);
                } else if active_furnace {
                    host = host.with_external_menu_sync(id, &mut furnace_sync);
                }
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
        main_scoreboard::tick(&mut players, &self.entity_events)?;
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
            registry,
            observers,
            chunk_transports,
            experience_runtime,
            falling_blocks,
            ..
        } = &mut *players;
        item_runtime.publish_tracking_with_external(
            |chunk| {
                chunk_runtime
                    .manager
                    .ticking_range()
                    .in_entity_ticking_range(chunk)
            },
            |kind, handle| {
                if kind == mc121_rust::runtime_entity_index::EntityKind::FallingBlock {
                    return main_falling_block_dispatch::publish_falling_tracking(
                        falling_blocks, handle, &chunk_runtime.manager,
                        registry, observers, chunk_transports, &self.entity_events,
                    );
                }
                if kind == mc121_rust::runtime_entity_index::EntityKind::ExperienceOrb {
                    return experience_runtime.publish_one(handle, registry, observers, chunk_transports, &self.entity_events);
                }
                ensure!(kind == mc121_rust::runtime_entity_index::EntityKind::Player, "unknown external sender owner");
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
        let _operation = self.chat_dispatch.lock().await;
        let _save = self.save_lock.lock().await;
        let mut world = self.world.write().await;
        let clock = *self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?;
        let (weather, mut entity_checkpoint, fluid_checkpoint, fluid_shapes, fire_checkpoint, block_checkpoint, random_position_value) = {
            let mut players = self
                .players
                .write()
                .map_err(|_| anyhow!("player registry lock poisoned"))?;
            if self.shutting_down.load(Ordering::Acquire) {
                self.finish_shutdown_chunk_tasks_locked(&mut world, &mut players, clock.game_time)?;
                self.close_furnaces_for_save_locked(&mut world,&mut players,clock.game_time)?;
                self.close_stonecutters_for_save_locked(&mut world,&mut players,clock.game_time)?;
                self.close_smithing_tables_for_save_locked(&mut world,&mut players,clock.game_time)?;
                self.close_looms_for_save_locked(&mut world,&mut players,clock.game_time)?;
                self.close_grindstones_for_save_locked(&mut world,&mut players,clock.game_time)?;
                self.close_anvils_for_save_locked(&mut world,&mut players,clock.game_time)?;
                self.close_crafting_tables_for_save_locked(&mut world,&mut players,clock.game_time)?;
                self.close_native_containers_for_save_locked(&mut world,&mut players,clock.game_time)?;
                let b=self.border.read().map_err(|_|anyhow!("border poisoned"))?.bounds(self.epoch.elapsed().as_millis() as i64);
                let border=CollisionBorder{min_x:b.min_x,min_z:b.min_z,max_x:b.max_x,max_z:b.max_z};
                for player in players.registry.snapshot() {
                    self.close_inventory_menu_locked(&mut world,&mut players,player.entity_id,clock.game_time,border)?;
                }
                self.finish_shutdown_chunk_tasks_locked(&mut world, &mut players, clock.game_time)?;
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
            if players.item_runtime.item_count() != 0
                || players.experience_runtime.count() != 0
                || players.falling_blocks.count() != 0
                || players.entity_source.as_ref().is_some_and(|source| source.is_supported())
            {
                players.require_current_native_source(&world)?;
            }
            let fluid_checkpoint = players
                .fluid_runtime
                .checkpoint(clock.game_time, players.item_runtime.world_random_state48());
            let fluid_shapes = mc121_rust::fluid_shape_checkpoint::SavedFluidShapeCache::capture(
                &players.world_interactions.fluid_shapes,
            )?;
            let fire_checkpoint = players.fire_runtime.checkpoint(clock.game_time, players.fluid_runtime.sub_tick_order);
            let block_checkpoint = players.block_runtime.checkpoint(clock.game_time);
            let random_position_value = players.level_random_position_value;
            // Capture the actual external bodies under the SAME registry/world
            // locks as the shared item/index/order owners. They belong to the
            // atomic entity checkpoint, never a second unanchored world array.
            let experience_orbs = players.experience_runtime.snapshot()?;
            let falling_blocks = main_falling_block_persistence::snapshot_native_falling_blocks(&players, &world)?;
            ensure!(players.entity_source.is_some() || (experience_orbs.is_empty() && falling_blocks.is_empty()),
                "cannot persist XP/falling bodies without the actual native entity source");
            let LivePlayers {
                item_runtime,
                entity_source,
                ..
            } = &mut *players;
            let checkpoint = entity_source
                .as_mut()
                .map(|source| {
                    if source.is_supported() {
                        EntityCheckpoint::capture_with_native_entities(
                            item_runtime,
                            source,
                            &world,
                            self.entities.load(Ordering::Relaxed),
                            NativeWorldTime { clock, weather },
                            experience_orbs,
                            falling_blocks,
                        )
                    } else {
                        // Preserve the existing empty, unsupported transport
                        // profile as explicitly legacy. Do not certify it as
                        // a newly complete empty native entity cohort.
                        ensure!(item_runtime.item_count() == 0
                            && experience_orbs.is_empty() && falling_blocks.is_empty(),
                            "unsupported source has persistent native entities");
                        EntityCheckpoint::capture(
                            item_runtime, source, &world,
                            self.entities.load(Ordering::Relaxed),
                            NativeWorldTime { clock, weather },
                        )
                    }
                })
                .transpose()?;
            (weather, checkpoint, fluid_checkpoint, fluid_shapes, fire_checkpoint, block_checkpoint, random_position_value)
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
                    // Legacy field retained for read compatibility only. The
                    // atomic EntityCheckpoint now owns XP, including EMPTY.
                    experience_orbs: Vec::new(),
                    block_ticks: Some(block_checkpoint),
                    level_random_position_value: random_position_value,
                };
                // Serialize ONCE. Bind and write these identical bytes; a later
                // parsed/pretty-printed equivalent is not this pairing proof.
                let saved_bytes = serde_json::to_vec_pretty(&saved)?;
                if let Some(checkpoint) = entity_checkpoint.as_mut() {
                    checkpoint.bind_world_file_bytes(&saved_bytes)?;
                }
                let path = &self.options.world;
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    fs::create_dir_all(parent).await?;
                }
                let temporary = path.with_extension("json.tmp");
                fs::write(&temporary, &saved_bytes).await?;
                fs::rename(temporary, path)
                    .await
                    .context("atomically replacing fixture world diffs")?;
            }
        }
        // The world image cannot change between checkpoint capture and the
        // two writes. New checkpoints additionally bind the exact world bytes,
        // including scheduling data not represented by the block WorldImage.
        if let Some(checkpoint) = entity_checkpoint {
            self.entity_store.save(&checkpoint)?;
        }
        drop(world);
        {
            let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
            main_scoreboard::save(&main_scoreboard::path(&self.options), &mut players.gameplay.scoreboard)?;
        }
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
    /// PlayerList sends the current ServerLevel shared spawn on admission.
    /// Imported worlds own it in level.dat; native flat worlds retain their
    /// existing default until a native spawn-setting owner is implemented.
    async fn default_spawn_packet(&self) -> Result<Packet> {
        let (x, y, z, angle) = match &self.level {
            Some(level) => level.lock().await.spawn()?,
            None => (8, 64, 8, 0.),
        };
        Ok(Packet {
            id: 0x56,
            payload: encoded(|writer| {
                writer.i64(pack_position(x, y, z)).f32(angle);
            }),
        })
    }

    async fn cleanup_player(&self, id: i32, reason: Option<&'static str>) -> Result<()> {
        let _operation = self.chat_dispatch.lock().await;
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
            ensure!(!players.furnaces.has_open(id), "orphan furnace menu retained");
            ensure!(!players.stonecutters.has_open(id), "orphan stonecutter menu retained");
            ensure!(!players.smithing_tables.has_open(id), "orphan smithing menu retained");
            ensure!(!players.looms.has_open(id), "orphan loom menu retained");
            ensure!(!players.grindstones.has_open(id), "orphan grindstone menu retained");
            ensure!(!players.anvils.has_open(id), "orphan anvil menu retained");
            if let Some(removal) = players.player_removal_progress.get(&id) {
                ensure!(removal.retirement_started && Self::player_runtime_handle(&players, id).is_none(),
                    "orphan player removal marker retained");
                players.player_removal_progress.remove(&id);
            }
            return Ok(());
        }
        if let Some(reason)=reason {
            if let Some(disconnect)=players.disconnects.get(&id) { disconnect.send_replace(Some(reason)); }
        }
        let external_owners = usize::from(players.native_containers.has_open(id))
            + usize::from(players.crafting_tables.has_open(id))
            + usize::from(players.furnaces.has_open(id))
            + usize::from(players.stonecutters.has_open(id))
            + usize::from(players.smithing_tables.has_open(id))
            + usize::from(players.looms.has_open(id))
            + usize::from(players.grindstones.has_open(id))
            + usize::from(players.anvils.has_open(id));
        ensure!(external_owners <= 1, "multiple current external menus during player cleanup");
        // PlayerList.remove saves persistent inventory/stats BEFORE Player.remove.
        // The source view excludes menu temporaries; saving the native resume
        // view here would duplicate the real drops that follow.
        self.save_player_before_removal_locked(&mut players, id)?;
        // Player.remove first invokes InventoryMenu.removed, then removes the
        // actual external menu. Keep that order for carried/grid drop RNG and
        // shared entity allocation even when a block menu remains current.
        if !players.player_removal_progress.get(&id).expect("actual removal marker").inventory_removed {
            self.close_inventory_menu_with_context_locked(&mut world,&mut players,id,now,border,mc121_rust::container::MenuCloseContext::DeadOrDisconnected)?;
            players.player_removal_progress.get_mut(&id).expect("actual removal marker").inventory_removed = true;
        }
        if !players.player_removal_progress.get(&id).expect("actual removal marker").external_removed {
            if let Some(window)=players.anvils.window_id(id) {
                ensure!(self.close_anvil_with_context_locked(&mut world,&mut players,id,window,false,now,border,mc121_rust::container::MenuCloseContext::DeadOrDisconnected)?,
                    "actual anvil owner changed during player cleanup");
            }
            if let Some(window)=players.grindstones.window_id(id) {
                ensure!(self.close_grindstone_with_context_locked(&mut world,&mut players,id,window,false,now,border,mc121_rust::container::MenuCloseContext::DeadOrDisconnected)?,
                    "actual grindstone owner changed during player cleanup");
            }
            if let Some(window)=players.looms.window_id(id) {
                self.close_loom_with_context_locked(&mut world,&mut players,id,window,false,now,border,mc121_rust::container::MenuCloseContext::DeadOrDisconnected)?;
            }
            if let Some(window)=players.smithing_tables.window_id(id) {
                self.close_smithing_with_context_locked(&mut world,&mut players,id,window,false,now,border,mc121_rust::container::MenuCloseContext::DeadOrDisconnected)?;
            }
            if let Some(window)=players.stonecutters.window_id(id) {
                self.close_stonecutter_with_context_locked(&mut world,&mut players,id,window,false,now,border,mc121_rust::container::MenuCloseContext::DeadOrDisconnected)?;
            }
            if let Some(window)=players.furnaces.window_id(id) {
                // This actor is leaving after EOF, disconnect, or duplicate-login
                // replacement. Its carried stack follows Player.drop(stack,false),
                // after the source save and before actor removal, instead of return.
                self.furnace_menu_action_with_close_context_locked(&mut world,&mut players,id,None,Some(window),false,now,border,mc121_rust::container_furnace::FurnaceMenuCloseContext::DeadOrDisconnected)?;
            }
            if let Some(window)=players.crafting_tables.window_id(id) {
                self.crafting_table_action_with_close_context_locked(&mut world,&mut players,id,None,Some(window),false,now,border,mc121_rust::container::MenuCloseContext::DeadOrDisconnected)?;
            }
            if let Some(window)=players.native_containers.window_id(id) {
                self.native_menu_action_with_close_context_locked(&mut world,&mut players,id,None,Some(window),false,now,border,mc121_rust::container::MenuCloseContext::DeadOrDisconnected)?;
            }
            players.player_removal_progress.get_mut(&id).expect("actual removal marker").external_removed = true;
        }
        players.player_removal_progress.get_mut(&id).context("actual removal marker is absent")?
            .retirement_started = true;
        players.remove_runtime_player(id)?;
        if let Some(event)=players.registry.remove(id) {
            players.motion.remove(&id);
            players.pending_metadata.remove(&id);
            players.containers.remove(&id);
            players.disconnects.remove(&id);
            players.connection_addresses.remove(&id);
            players.chat.remove(id);
            players.player_last_actions.remove(&id);
            players.pending_block_acks.remove(&id);
            players.flying.remove(&id);
            players.unavailable_pose.remove(&id);
            players.publish(event,&self.entity_events);
        }
        ensure!(players.registry.get(id).is_none() && Self::player_runtime_handle(&players, id).is_none(),
            "player retirement did not release its actual owners");
        players.player_removal_progress.remove(&id);
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
    let mut options = Options::parse()?;
    let fixtures = fixtures(&options.fixtures).await?;
    // Exact 1.21 minecraft-data defaults: air=0, stone=1, dirt=10,
    // grass_block=9 and minecraft:plains biome=39. This is a flat fixture.
    let mut fixture_world = FlatWorld::new(0, 1, 10, 9, 39);
    let mut initial_clock = TickClock::default();
    let mut initial_experience_orbs = Vec::new();
    let mut initial_native_entities = None;
    let mut initial_world_file_bytes = None;
    let mut initial_weather = WeatherState::default();
    let mut initial_fluid_runtime = main_fluid_simulation::MainFluidRuntime::default();
    let mut initial_fluid_random = None;
    let mut had_fluid_checkpoint = false;
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
        let saved_bytes = fs::read(&options.world).await?;
        let saved: SavedWorld = serde_json::from_slice(&saved_bytes)?;
        initial_world_file_bytes = Some(saved_bytes);
        if saved.format != "mc121-flat-diffs-v1" || saved.minecraft != "1.21" {
            bail!("unsupported saved fixture world format");
        }
        // Inspect actual saved provenance before restoring blocks/BEs or
        // binding/listening and creating the cold Level/Math RNG owners.
        if saved.block_ticks.as_ref().is_some_and(main_block_ticks::SavedBlockRuntime::requires_shared_counter) {
            ensure!(saved.fluid_ticks.is_some() && saved.clock.is_some(),
                "saved BLOCK use or pending/stale ticks require a paired clock/FLUID shared-counter checkpoint before world restoration");
        }
        fixture_world.apply_changes(&saved.changes)?;
        initial_clock = saved.clock.unwrap_or_default();
        initial_experience_orbs = saved.experience_orbs;
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
        had_fluid_checkpoint = saved.fluid_ticks.is_some();
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
    let difficulty_locked = level.as_ref().is_some_and(LevelData::difficulty_locked);
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
        let persistent_count = checkpoint.persistent_entity_count();
        // An atomic cohort is authoritative even when empty. If its source
        // proof fails, the older split world XP field must not resurrect an
        // entity from another save interval through the legacy fallback.
        let has_native_cohort = checkpoint.has_native_entity_cohort();
        let restored = (|| {
            if let Some(bytes) = initial_world_file_bytes.as_deref() {
                checkpoint.require_world_file_bytes(bytes)?;
            } else {
                ensure!(!has_native_cohort,
                    "atomic native entity cohort requires its actual saved world file");
            }
            checkpoint.restore(&world)
        })();
        match restored {
            Ok(restored) => {
                // Include hidden items before adopting either restored time
                // or the live runtime. Failure cannot enter legacy fallback.
                for state in restored.runtime.item_states() {
                    main_workstation_components::require_supported_stack(&state.stack)
                        .with_context(|| format!("refusing unsupported native checkpoint item {}", state.entity_id))?;
                }
                ensure!(!restored.source.requires_fire_checkpoint() || had_fire_checkpoint,
                    "native source history requires its saved fire queue, including stale scheduled ticks");
                ensure!(!restored.source.requires_block_checkpoint() || had_block_checkpoint,
                    "native source history requires its saved global BLOCK scheduler");
                // BLOCK shares the FLUID checkpoint's level sub-tick counter,
                // including historically used queues that are now empty.
                ensure!(!restored.source.requires_block_checkpoint() || had_fluid_checkpoint,
                    "native BLOCK source history requires its paired clock/FLUID shared-counter checkpoint");
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
            Err(error) if persistent_count == 0 && !has_native_cohort => {
                eprintln!(
                    "{}",
                    json!({"event":"native_entity_resume_unavailable","scope":"legacy_profile_transport","reason":format!("{error:#}"),"saved_items":0})
                );
            }
            Err(error) => {
                return Err(error)
                    .context("refusing to discard an authoritative or nonempty native entity checkpoint")
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
    let administration_directory = options.server_directory.clone()
        .unwrap_or_else(|| options.world.parent().unwrap_or_else(|| Path::new(".")).to_path_buf());
    let property_bytes = match fs::read(administration_directory.join("server.properties")).await {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{}",json!({"event":"connection_properties_load_failed","file":administration_directory.join("server.properties"),"reason":error.to_string(),"using_defaults":true}));
            Vec::new()
        }
    };
    let mut connection_settings = mc121_rust::connection_settings::DedicatedConnectionSettings::try_from_lookup(
        |key| main_administration::property_value_from_bytes(&property_bytes, key),
    )?;
    if let Some(value) = options.rate_limit { connection_settings.rate_limit = value; }
    if let Some(value) = options.compression_threshold { connection_settings.network_compression_threshold = value; }
    if let Some(value) = options.secure_profile_override { connection_settings.enforce_secure_profile = value; }
    options.enforce_secure_profile = connection_settings.enforce_secure_profile;
    // Resolve the explicit local source after effective settings, before any
    // listener or auth/key network owner can start. Local failure is terminal.
    let service_key_source = main_chat_keys::prepare_key_source(&options).await?;
    let mut administration = main_administration::Administration::load(
        administration_directory,
        main_administration::AdminSettings {
            max_players: options.max_players,
            whitelist_enabled: false,
            enforce_whitelist: options.enforce_whitelist,
            op_permission_level: options.op_permission_level,
        },
        main_admin_dispatch::wall_time_ms()?,
    )?;
    for warning in administration.take_startup_warnings() {
        if let main_administration::AdminEffect::PersistenceWarning { file, message } = warning {
            eprintln!("{}", json!({"event":"administration_persistence_warning","file":file,"reason":message}));
        }
    }
    let listener = TcpListener::bind(SocketAddr::new(options.bind, options.port)).await?;
    service_key_source.require_bound_address(listener.local_addr()?)?;
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
        initial_native_entities = restored.native_entities;
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
    // Check the actual final owner too: this covers queued/stale records and
    // legacy migration without an entity source, never a queue-derived counter.
    // A present FLUID checkpoint already passed its saved-clock validation.
    if players.block_runtime.was_used() || players.block_runtime.pending_count() != 0 {
        ensure!(had_fluid_checkpoint,
            "used or pending native BLOCK scheduler requires its paired clock/FLUID shared-counter checkpoint");
    }
    if supported_native_source {
        let falling_owner = main_falling_block_damage::NativeFallingDamageTypes::capture(&fixtures)?;
        let powder_owner = main_falling_block_powder::NativeConcretePowderOwner::capture(&fixtures)?;
        players.block_runtime.install_falling_owner(falling_owner)?;
        players.block_runtime.install_concrete_powder_owner(powder_owner)?;
    }
    main_falling_block_persistence::restore_native_entity_cohort(
        &mut players, &world, initial_native_entities, initial_experience_orbs, next_entity_id,
    )?;
    if let Some(cache) = initial_fluid_shapes {
        players.world_interactions.fluid_shapes = cache;
    }
    players
        .gameplay
        .restore_known_native_players(main_player_data::load_known_native_players(
            &player_store,
        )?)?;
    players.gameplay.scoreboard = main_scoreboard::load(&main_scoreboard::path(&options))?;
    let server = Arc::new(Server {
        options,
        connection_settings,
        connection_rates: StdRwLock::new(BTreeMap::new()),
        fixtures,
        seed_hash,
        world: RwLock::new(world),
        save_lock: Mutex::new(()),
        player_admission: Mutex::new(()),
        administration: StdRwLock::new(administration),
        connection_ticks: watch::channel(0).0,
        chat_keys: watch::channel(Arc::new(mc121_rust::chat_crypto::ServiceKeySet::empty())).0,
        chat_keys_attempted: watch::channel(false).0,
        chat_dispatch: Mutex::new(()),
        entities: AtomicI32::new(next_entity_id),
        connection_sequence: AtomicU64::new(1),
        configuration_requests: broadcast::channel(64).0,
        changes,
        authenticator,
        session_verifier,
        players: StdRwLock::new(players),
        entity_events,
        clock: StdRwLock::new(initial_clock),
        tick_events,
        epoch: Instant::now(),
        level: level.map(Mutex::new),
        difficulty_locked,
        border: StdRwLock::new(border),
        player_store,
        entity_store,
        shutting_down: AtomicBool::new(false),
    });
    main_chat_keys::start_key_refresh(Arc::clone(&server), service_key_source);
    let permits = Arc::new(Semaphore::new(64));
    println!(
        "{}",
        json!({"event":"listening","address":listener.local_addr()?.to_string(),
        "minecraft":"1.21","protocol":PROTOCOL,"mode":if server.options.online_mode {"online"} else if server.options.bind.is_loopback() {"offline-loopback"} else {"offline"}})
    );
    let mut console = main_admin_dispatch::console_input();
    let mut console_open = true;
    let mut connection_tick_sequence = 0_u64;
    let mut ticks = main_server_timing::NativeServerTiming::new()?;
    let mut latency_clock = LatencyUpdateClock::default();
    let mut autosave_ticks = 0_u32;
    let mut server_tick_count = 0_i32;
    // Retain the registered signal listener while a tick/accept branch runs.
    // Recreating ctrl_c in select would drop its receiver after every winning
    // branch and could lose SIGINT delivered during the branch body.
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    #[cfg(unix)]
    let mut reconfigure_signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?;
    loop {
        tokio::select! {
            line = console.recv(), if console_open => {
                match line {
                    Some(line) => {
                        if let Err(error) = server.execute_administration(&line, None).await {
                            eprintln!("{}", json!({"event":"administration_command_failed","reason":error.to_string()}));
                        }
                    }
                    None => console_open = false,
                }
            }
            _ = async {
                #[cfg(unix)]
                { reconfigure_signal.recv().await; }
                #[cfg(not(unix))]
                { std::future::pending::<()>().await; }
            } => {
                // An explicit native operator transport reconfiguration. This
                // does not claim to reload datapacks or regenerate registries.
                let ids = server.players.read().map_err(|_|anyhow!("players poisoned"))?
                    .registry.snapshot().into_iter().map(|p|p.entity_id).collect::<Vec<_>>();
                for id in ids { let _ = server.configuration_requests.send(id); }
            }
            overload = ticks.next_tick() => {
                if let Some(overload) = overload? {
                    eprintln!("{}", json!({"event":"server_tick_overload", "behind_ms":overload.behind_ms, "skipped_deadline_ticks":overload.skipped_ticks}));
                }
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
                // XP and Item bodies share the actual level tick order above.
                server.tick_native_furnaces().await?;
                server.tick_connection_phase().await?;
                // Sample actual connection ticks without replaying coalesced
                // watch updates into the wrong packet-count window.
                for (connection, rate) in server.connection_rates.read().map_err(|_|anyhow!("connection registry poisoned"))?.iter() {
                    if let Some(action) = rate.tick_connection()? {
                        eprintln!("{}",json!({"event":"packet_rate_exceeded","connection":connection,"average_received_packets":action.average_received_packets,"configured_limit":action.configured_limit}));
                    }
                }
                {
                    let _chat_sequence=server.chat_dispatch.lock().await;
                    server.players.write().map_err(|_|anyhow!("players poisoned"))?.chat.tick();
                }
                connection_tick_sequence=connection_tick_sequence.checked_add(1).context("connection tick counter exhausted")?;
                server.connection_ticks.send_replace(connection_tick_sequence);
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
    if main_connection::terminal().is_some() {
        main_connection::disconnect_rate(codec, stream).await?;
        bail!("{}", mc121_rust::login_limits::EXCEEDED_PACKET_RATE_REASON);
    }
    main_connection::rate().packet_send_dispatched()?;
    let _write = main_connection::begin_write();
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

async fn receive_optional(codec: &FrameCodec, stream: &mut NetworkReader, writer: &mut NetworkWriter) -> Result<Option<Packet>> {
    tokio::select! {
        result = codec.read(stream) => result,
        _ = main_connection::wait_terminal() => {
            main_connection::disconnect_rate(codec,writer).await?;
            bail!("{}",mc121_rust::login_limits::EXCEEDED_PACKET_RATE_REASON);
        }
    }
}
async fn receive(codec: &FrameCodec, stream: &mut NetworkReader, writer: &mut NetworkWriter) -> Result<Packet> {
    receive_optional(codec,stream,writer).await?.context("connection closed during negotiation")
}
async fn receive_login(
    codec: &FrameCodec,
    reader: &mut NetworkReader,
    writer: &mut NetworkWriter,
    cookies: &mut CookieConnection,
) -> Result<Packet> {
    let packet = tokio::select! {
        result = codec.read(reader) => result?.context("connection closed during login")?,
        _ = main_connection::wait_terminal() => {
            main_connection::disconnect_rate(codec, writer).await?;
            bail!("{}",mc121_rust::login_limits::EXCEEDED_PACKET_RATE_REASON);
        }
    };
    if cookies.handle_packet(&packet)? == CookieAction::DisconnectUnexpectedQuery {
        main_connection::accepted()?;
        let reason = json!({"translate":UNEXPECTED_COOKIE_RESPONSE_REASON}).to_string();
        send(codec, writer, 0x00, encoded(|w| { w.string(&reason); })).await?;
        bail!("{UNEXPECTED_COOKIE_RESPONSE_REASON}");
    }
    Ok(packet)
}
fn start_packet_reader(mut reader: NetworkReader, codec: FrameCodec) -> (ReaderGuard, mpsc::Receiver<Result<Packet>>) {
    let (tx, rx) = mpsc::channel(32);
    let guard = ReaderGuard(tokio::spawn(async move {
        loop {
            let packet = match codec.read(&mut reader).await {
                Ok(Some(packet)) => Ok(packet),
                Ok(None) => break,
                Err(error) => Err(error),
            };
            let failed = packet.is_err();
            if tx.send(packet).await.is_err() || failed { break; }
        }
    }));
    (guard, rx)
}
fn encoded(build: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut writer = Writer::new();
    build(&mut writer);
    writer.into_inner()
}

async fn negotiate_login(
    mut reader: &mut NetworkReader,
    mut writer: &mut NetworkWriter,
    codec: &mut FrameCodec,
    server: &Server,
    address: SocketAddr,
) -> Result<Option<PlayerProfile>> {
    let mut login_cookies = CookieConnection::new(CookiePhase::Login);
    let start = receive_login(&codec, &mut reader, &mut writer, &mut login_cookies).await?;
    if start.id != 0 {
        bail!("expected Login Start");
    }
    let mut fields = Reader::new(&start.payload);
    let username = fields.string(16)?;
    let _claimed_uuid = fields.uuid()?;
    fields.finish()?;
    main_connection::accepted()?;
    if !valid_login_name(&username) {
        bail!("invalid login username");
    }
    let profile = if let Some(authenticator) = &server.authenticator {
        let challenge = authenticator.challenge();
        send(
            &codec,
            &mut writer,
            1,
            authenticator.encryption_request(&challenge),
        )
        .await?;
        let response = receive_login(&codec, &mut reader, &mut writer, &mut login_cookies).await?;
        if response.id != 1 {
            bail!("expected encrypted login response");
        }
        main_connection_decode::validate_login_key(&response.payload)?;
        main_connection::accepted()?;
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
    let admission_guard = server.player_admission.lock().await;
    if !server.check_initial_admission(&profile, address, &codec, &mut writer, &admission_guard).await? {
        return Ok(None);
    }
    let threshold = server.connection_settings.network_compression_threshold;
    if threshold >= 0 {
        send(&codec, &mut writer, 3, encoded(|w| { w.varint(threshold); })).await?;
        codec.compression = Some(threshold);
    }
    server.evict_duplicate_for_login(&profile, &admission_guard).await?;
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
    drop(admission_guard);
    let acknowledged = receive_login(&codec, &mut reader, &mut writer, &mut login_cookies).await?;
    if acknowledged.id != 3 || !acknowledged.payload.is_empty() {
        bail!("expected Login Acknowledged");
    }
    main_connection::accepted()?;
    Ok(Some(profile))
}

async fn connection(socket: TcpStream, server: Arc<Server>, admission: u64) -> Result<()> {
    socket.set_nodelay(true)?;
    let address = socket.peer_addr()?;
    let (reader, writer) = socket.into_split();
    let (raw_pump, reader, mut idle) = mc121_rust::connection_transport::spawn_raw_read_transport(reader);
    let rate = main_connection_rate::ConnectionRate::new(server.connection_settings.rate_limit);
    server.connection_rates.write().map_err(|_|anyhow!("connection registry poisoned"))?.insert(admission, rate.clone());
    let _registration = main_connection::ConnectionRegistration { server: Arc::clone(&server), admission };
    let context = main_connection::ConnectionContext::new(rate, raw_pump.abort_handle());
    let session = main_connection::CONNECTION.scope(context, connection_session(
        CryptoRead::plain(reader), CryptoWrite::plain(writer), server, admission, address,
    ));
    tokio::pin!(session);
    tokio::select! {
        result = &mut session => result,
        timeout = idle.wait() => Err(timeout.into()),
    }
}

async fn connection_session(
    mut reader: NetworkReader,
    mut writer: NetworkWriter,
    server: Arc<Server>,
    admission: u64,
    address: SocketAddr,
) -> Result<()> {
    let mut codec = FrameCodec::default();
    let hello = receive(&codec, &mut reader, &mut writer).await?;
    if hello.id != 0 {
        bail!("expected handshake");
    }
    let mut fields = Reader::new(&hello.payload);
    let protocol = fields.varint()?;
    let _host = fields.string(255)?;
    let _port = fields.u16()?;
    let intent = fields.varint()?;
    fields.finish()?;
    main_connection::accepted()?;
    if intent == 1 {
        main_connection::phase(main_connection::OutboundPhase::Status);
        let request = receive(&codec, &mut reader, &mut writer).await?;
        if request.id != 0 || !request.payload.is_empty() {
            bail!("invalid status request");
        }
        main_connection::accepted()?;
        server.wait_secure_chat_enforcement().await?;
        let response = json!({"version":{"name":"1.21","protocol":PROTOCOL},
            "players":{"max":server.maximum_players()?,"online":server.players.read().map_err(|_| anyhow!("player registry lock poisoned"))?.registry.len(),"sample":[]},
            "description":{"text":"Rust compatibility checkpoint — creative fixture; vanilla parity incomplete"},
            "enforcesSecureChat":server.current_secure_chat_enforcement()});
        send(
            &codec,
            &mut writer,
            0,
            encoded(|w| {
                w.string(&response.to_string());
            }),
        )
        .await?;
        if let Some(ping) = receive_optional(&codec, &mut reader, &mut writer).await? {
            if ping.id != 1 || ping.payload.len() != 8 {
                bail!("invalid status ping");
            }
            main_connection::accepted()?;
            send(&codec, &mut writer, 1, ping.payload).await?;
        }
        return Ok(());
    }
    if intent != 2 {
        bail!("only status and login handshakes are implemented");
    }
    main_connection::phase(main_connection::OutboundPhase::Login);
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
    let outcome = {
        let mut ticks = server.connection_ticks.subscribe();
        let mut timeout = mc121_rust::login_limits::ObservedLoginTimeout::new(*ticks.borrow_and_update());
        let negotiation = negotiate_login(&mut reader, &mut writer, &mut codec, &server, address);
        tokio::pin!(negotiation);
        let mut expired = false;
        loop {
            tokio::select! {
                biased;
                result = &mut negotiation => break if expired { None } else { Some(result) },
                changed = ticks.changed() => {
                    changed?;
                    let current=*ticks.borrow_and_update();
                    expired |= timeout.observe(current).is_some();
                    // Keep the terminal action while an already-dispatched
                    // packet finishes; cancel-then-append could corrupt a
                    // partial frame and its advanced encryption stream.
                    if expired && !main_connection::writing() { break None; }
                }
            }
        }
    };
    let profile = match outcome {
        Some(result) => match result? { Some(profile) => profile, None => return Ok(()) },
        None => {
            let reason=json!({"translate":mc121_rust::login_limits::SLOW_LOGIN_REASON}).to_string();
            send(&codec, &mut writer, 0x00, encoded(|w| { w.string(&reason); })).await?;
            return Ok(());
        }
    };
    // One raw framing reader owns the encrypted stream through every listener
    // transition. Aborting/restarting a reader could discard a partial frame.
    let (_packet_reader, mut packets) = start_packet_reader(reader, codec.clone());
    main_connection::phase(main_connection::OutboundPhase::Configuration);
    let mut protocol = Reconfiguration::initial_configuration(server.epoch.elapsed().as_millis() as i64, 0);
    let mut packs = ResourcePackSession::new(server.options.resource_pack_required);
    let mut client_information = main_configuration::configure(
        &mut packets, &mut writer, &codec, &server, &mut protocol, &mut packs, None, true,
    ).await?;
    loop {
        server.wait_secure_chat_enforcement().await?;
        main_connection::phase(main_connection::OutboundPhase::Play);
        match play(
            &mut packets, &mut writer, codec.clone(), Arc::clone(&server), profile.clone(),
            client_information, admission, address, &mut protocol, &mut packs,
        ).await? {
            PlayExit::Disconnected => {
                // Retain the actual framing reader while half-closing output.
                // Dropping a socket with unread client input can turn an already
                // written disconnect reason into TCP RST. Late packets are
                // drained here without dispatching any more gameplay callbacks.
                let _ = time::timeout(WRITE_DEADLINE, writer.shutdown()).await;
                let _ = time::timeout(Duration::from_secs(1), async {
                    while let Some(packet) = packets.recv().await {
                        if packet.is_err() { break; }
                    }
                }).await;
                return Ok(());
            },
            PlayExit::Reconfigure(info) => {
                client_information = main_configuration::configure(
                    &mut packets, &mut writer, &codec, &server, &mut protocol, &mut packs, info, false,
                ).await?;
            }
        }
    }
}

fn read_client_information(payload: &[u8]) -> Result<ClientInformation> {
    let mut fields = Reader::new(payload);
    fields.string(16)?;
    let view_distance = i32::from(fields.i8()?);
    let chat = fields.varint()?;
    fields.bool()?;
    let skin = fields.u8()?;
    let hand = fields.varint()?;
    let text_filtering = fields.bool()?;
    fields.bool()?;
    fields.finish()?;
    if !(0..=2).contains(&chat) || !(0..=1).contains(&hand) {
        bail!("invalid client information enum");
    }
    Ok(ClientInformation {
        chat_visibility: chat,
        text_filtering,
        view_distance,
        skin_parts: skin,
        main_hand: hand as u8,
    })
}

async fn play(
    rx: &mut mpsc::Receiver<Result<Packet>>,
    mut writer: &mut NetworkWriter,
    codec: FrameCodec,
    server: Arc<Server>,
    profile: PlayerProfile,
    client_information: Option<ClientInformation>,
    admission: u64,
    address: SocketAddr,
    protocol: &mut Reconfiguration,
    packs: &mut ResourcePackSession,
) -> Result<PlayExit> {
    // Queue updates before the first snapshot. A concurrent player may edit an
    // already-sent chunk while the rest of this player's window is streaming.
    let mut changes = server.changes.subscribe();
    let mut entity_events = server.entity_events.subscribe();
    let mut tick_events = server.tick_events.subscribe();
    let mut configuration_requests = server.configuration_requests.subscribe();
    let mut current_information = client_information;
    let mut cookies = CookieConnection::new(CookiePhase::Play);
    let admission_guard=server.player_admission.lock().await;
    ensure!(!server.shutting_down.load(Ordering::Acquire), "server shutting down");
    if let Some(rejection) = server.check_final_admission(&profile, address)? {
        send(&codec, &mut writer, 0x1d, rejection.network_component()?).await?;
        return Ok(PlayExit::Disconnected);
    }
    let (entity_id, mut player_random) = server.allocate_player_entity()?;
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
    {
        let mut players = server
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let uuid = profile.uuid;
        let mut seed_admission = main_player_data::EnchantmentSeedAdmission::SavedData;
        // Pending writes are saved-data proposals for this new Player too.
        // Reconfiguration does not retain/copy the old Player object.
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
        if let Some(data) = saved.as_ref() {
            // One admission boundary for pending, native and imported saves,
            // before apply/refresh can canonicalize an unsupported component.
            main_workstation_components::require_supported_inventory(&data.inventory)
                .context("refusing unsupported saved/pending/imported player inventory before admission")?;
        }
        let math=players.math_random.as_ref().context("actual global Math.random owner missing")?;
        let constructor_math=[math.next_f64()?,math.next_f64()?,math.next_f64()?];
        let mut player = PlayerState::new(entity_id, profile, position)?;
        let (inventory, stats, mut lifecycle) = if let Some(saved) = saved {
            saved.apply_to_player(&mut player)?;
            (saved.inventory, saved.stats, saved.lifecycle)
        } else {
            seed_admission = main_player_data::EnchantmentSeedAdmission::NewPlayer;
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
        // This is the one real local Player.random created for this admission.
        // Every actual saved-data admission, including reconfiguration and a
        // pending native save, loads seed0 once. A genuinely new player keeps
        // constructor zero. Move this advanced RNG into runtime below.
        main_player_data::admit_enchantment_seed(&mut lifecycle, seed_admission, &mut player_random)?;
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
        let chat_profile = player.profile.clone();
        let event = players.registry.register(player)?;
        let chat_settings = match client_information {
            Some(info) => main_chat::MainChatSettings::from_client_information(info.chat_visibility, info.text_filtering)?,
            None => main_chat::MainChatSettings::default(),
        };
        players.chat.register(entity_id, &chat_profile, chat_settings)?;
        players.player_last_actions.insert(entity_id, server.epoch.elapsed().as_millis() as i64);
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
            players.chat.remove(entity_id);
            players.player_last_actions.remove(&entity_id);
            return Err(error);
        }
        players.gameplay.scoreboard_names.insert(uuid, chat_profile.name.clone());
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
        players.connection_addresses.insert(entity_id, address);
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
    let maximum_players = server.maximum_players()?;
    let enforce_secure_chat = server.current_secure_chat_enforcement();
    let (reduced_debug_info, show_death_screen) = {
        let players = server.players.read().map_err(|_| anyhow!("player registry lock poisoned"))?;
        let rules = &players.world_context.rules;
        (rules.boolean("reducedDebugInfo").context("missing reducedDebugInfo rule")?,
         !rules.boolean("doImmediateRespawn").context("missing doImmediateRespawn rule")?)
    };
    send(
        &codec,
        &mut writer,
        0x2b,
        encoded(|w| {
            w.i32(entity_id)
                .bool(false)
                .varint(1)
                .string("minecraft:overworld")
                .varint(maximum_players)
                .varint(view)
                .varint(view)
                .bool(reduced_debug_info)
                .bool(show_death_screen)
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
                .bool(enforce_secure_chat);
        }),
    )
    .await?;
    send(&codec,&mut writer,0x0b,encoded(|w| {w.u8(server.options.difficulty as u8).bool(server.difficulty_locked);})).await?;
    send_packets(&codec, &mut writer, server.administration_permission_packets(entity_id)?).await?;
    let (mut scoreboard_revision, scoreboard_packets) = server.scoreboard_join_packets()?;
    send_packets(&codec, &mut writer, scoreboard_packets).await?;
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
    let border_packet = {
        let border = server.border.read().map_err(|_| anyhow!("world border lock poisoned"))?;
        main_border_login::initialize(&border, server.epoch.elapsed().as_millis() as i64)
    };
    send(&codec, &mut writer, border_packet.id, border_packet.payload).await?;
    let initial_world = server.world_snapshot()?;
    tracker.world_revision = Some(initial_world.revision);
    send(
        &codec,
        &mut writer,
        0x64,
        initial_world.time_payload.to_vec(),
    )
    .await?;
    let spawn_packet = server.default_spawn_packet().await?;
    send(&codec, &mut writer, spawn_packet.id, spawn_packet.payload).await?;
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

    if native_motion {
        server.players.write().map_err(|_|anyhow!("player registry lock poisoned"))?.native_body_ready.insert(entity_id);
    }
    protocol.complete_play_admission(server.epoch.elapsed().as_millis() as i64)?;
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
            return Ok(PlayExit::Disconnected);
        }
        tokio::select! {
            _ = main_connection::wait_terminal() => {
                main_connection::disconnect_rate(&codec, &mut writer).await?;
                return Ok(PlayExit::Disconnected);
            }
            request = configuration_requests.recv() => {
                match request {
                    Ok(requested) if requested == entity_id => {
                        // Set waiting-for-configuration before source save/removal.
                        // Re-entry constructs and loads a NEW Player; no restoreFrom.
                        let terminal = protocol.begin_reconfiguration(server.epoch.elapsed().as_millis() as i64)?;
                        server.cleanup_player(entity_id, None).await?;
                        send(&codec, &mut writer, terminal.id, terminal.payload).await?;
                        return Ok(PlayExit::Reconfigure(current_information));
                    }
                    Ok(_) => {},
                    Err(broadcast::error::RecvError::Lagged(_)) => bail!("configuration control history lost"),
                    Err(broadcast::error::RecvError::Closed) => return Ok(PlayExit::Disconnected),
                }
            }
            changed = disconnect_events.changed() => {
                changed?;
                let reason=*disconnect_events.borrow_and_update();
                if let Some(reason)=reason {
                    play_disconnect(&codec,&mut writer,reason).await?;
                    return Ok(PlayExit::Disconnected);
                }
            }
            tick = tick_events.changed() => {
                tick?;
                if !teleport_confirmed && Instant::now() >= teleport_deadline { bail!("initial teleport was not acknowledged"); }
                let now=*tick_events.borrow_and_update();
                if let Some(action)=protocol.keep_alive_mut().tick(now) {
                    match action {
                        KeepAliveAction::Challenge(id)=>send(&codec, &mut writer, 0x26, encoded(|w| { w.i64(id); })).await?,
                        KeepAliveAction::Timeout=>{ play_disconnect(&codec,&mut writer,"disconnect.timeout").await?; return Ok(PlayExit::Disconnected); },
                    }
                }
            }
            packet = rx.recv() => {
                let Some(packet) = packet else { return Ok(PlayExit::Disconnected); };
                let packet = packet?;
                if server.shutting_down.load(Ordering::Acquire) { return Ok(PlayExit::Disconnected); }
                main_connection_decode::validate_play(&packet)?;
                main_connection::accepted()?;
                if cookies.handle_packet(&packet)? == CookieAction::DisconnectUnexpectedQuery {
                    play_disconnect(&codec, &mut writer, UNEXPECTED_COOKIE_RESPONSE_REASON).await?;
                    return Ok(PlayExit::Disconnected);
                }
                let mut fields = Reader::new(&packet.payload);
                match packet.id {
                    0x03..=0x07 => {
                        server.handle_chat_packet(entity_id, &packet).await?;
                    },
                    0x0b => {
                        let request=mc121_rust::command_protocol::decode_suggestion_request(&packet.payload)?;
                        let packet=server.administration_suggestions(entity_id, &request)?;
                        send(&codec, &mut writer, packet.id, packet.payload).await?;
                    },
                    0x0c => bail!("client acknowledged configuration, but none was requested"),
                    0x12 => { let _ = mc121_rust::connection_payload::decode_custom_payload(&packet.payload)?; },
                    0x27 => { let _ = mc121_rust::connection_payload::decode_pong(&packet.payload)?; },
                    0x21 => {
                        let time = fields.i64()?; fields.finish()?;
                        send(&codec,&mut writer,0x36,encoded(|out| { out.i64(time); })).await?;
                    },
                    0x2b => {
                        let outcome = packs.handle_response(ResourcePackPhase::Play, &packet.payload)?;
                        if let Some(reason) = outcome.disconnect_reason {
                            play_disconnect(&codec, &mut writer, reason).await?;
                            return Ok(PlayExit::Disconnected);
                        }
                    },
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
                    0x2a => {
                        let name=main_anvil_dispatch::decode_anvil_rename(&packet.payload)?;
                        let _=server.rename_native_anvil(entity_id,&name).await?;
                    },
                    0x18 => {
                        let id = fields.i64()?; fields.finish()?;
                        match protocol.keep_alive_mut().receive(server.epoch.elapsed().as_millis() as i64,id) {
                            Ok(Some(latency))=>server.update_latency(entity_id,latency)?,
                            Ok(None)=>{},
                            Err(_)=>{ play_disconnect(&codec,&mut writer,"disconnect.timeout").await?; return Ok(PlayExit::Disconnected); },
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
                            return Ok(PlayExit::Disconnected);
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
                            return Ok(PlayExit::Disconnected);
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
                        current_information=Some(info);
                        server.players.write().map_err(|_|anyhow!("players poisoned"))?.chat.update_client_information(
                            entity_id, main_chat::MainChatSettings::from_client_information(info.chat_visibility, info.text_filtering)?
                        )?;
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
                    0x0d => {
                        let window=fields.varint()?; let button=fields.varint()?; fields.finish()?;
                        if server.loom_button(entity_id,window,button).await? { continue; }
                        let _=server.stonecutter_button(entity_id,window,button).await?;
                    },
                    0x0e => {
                        let click=ContainerClick::decode(&packet.payload)?;
                        if let Err(error) = main_workstation_components::require_supported_prediction(&click) {
                            // Refuse before adopting predictions or mutating
                            // actual stacks, then sync the retained current owner.
                            eprintln!("{}", json!({"event":"native_component_admission_refused","entity_id":entity_id,"packet_id":packet.id,"reason":format!("{error:#}")}));
                            server.resync_current_player_menu(entity_id).await?;
                            continue;
                        }
                        if server.click_native_anvil(entity_id,&click).await? {continue;}
                        if server.click_native_grindstone(entity_id,&click).await? {continue;}
                        if server.click_native_loom(entity_id,&click).await? {continue;}
                        if server.click_native_smithing_table(entity_id,&click).await? {continue;}
                        if server.click_native_stonecutter(entity_id,&click).await? {continue;}
                        if server.click_native_furnace(entity_id,&click).await? {continue;}
                        if server.click_native_crafting_table(entity_id,&click).await? {continue;}
                        if server.click_native_container(entity_id,&click).await? {continue;}
                        let result=server.execute_menu_action(entity_id,&container,|menu|menu.click(&click,gamemode==1,&recipe_context),None).await;
                        finish_menu_action(result,&container,&codec,&mut writer,&mut container_revision).await?;
                    },
                    0x0f => {
                        let _requested_window=fields.u8()?; fields.finish()?;
                        // Selection and removal share one actual operation;
                        // the client's window never chooses a retained owner.
                        let result=server.close_current_player_menu(entity_id).await;
                        finish_menu_action(result,&container,&codec,&mut writer,&mut container_revision).await?;
                    },
                    0x2f => { let slot=fields.i16()?; fields.finish()?; container.transact(|menu| {menu.inventory.set_selected_hotbar(slot)?;Ok(((),vec![]))})?; },
                    0x32 => {
                        if gamemode != 1 { continue; }
                        let slot=fields.i16()?; let stack=ItemStack::decode_exact(fields.remaining())?;
                        if let Err(error) = main_workstation_components::require_supported_stack(&stack) {
                            // Preserve actual slot/cursor ownership and repair
                            // through the real current menu's full-state path.
                            eprintln!("{}", json!({"event":"native_component_admission_refused","entity_id":entity_id,"packet_id":packet.id,"reason":format!("{error:#}")}));
                            server.resync_current_player_menu(entity_id).await?;
                            continue;
                        }
                        if slot<0 {
                            let result=server.execute_menu_action(entity_id,&container,|_|Ok(ClickOutcome::default()),Some((slot,stack))).await;
                            finish_menu_action(result,&container,&codec,&mut writer,&mut container_revision).await?;
                            continue;
                        }
                        // A detached admission observation of the actual packet
                        // stack; mutation still owns the decoded stack below.
                        let candidate=stack.clone();
                        let result=server.execute_menu_action_with_candidate(entity_id,&container,|menu| {
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
                        },None,Some(candidate)).await;
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
                        if !(0..=1).contains(&hand) || sequence<0 || !(0..=5).contains(&face) || !cursor.iter().all(|v| v.is_finite()) { bail!("invalid placement request"); }
                        let direction=[geometry::Direction::Down,geometry::Direction::Up,geometry::Direction::North,geometry::Direction::South,geometry::Direction::West,geometry::Direction::East][face as usize];
                        let hit=Vec3::new(f64::from(target.0)+f64::from(cursor[0]),f64::from(target.1)+f64::from(cursor[1]),f64::from(target.2)+f64::from(cursor[2]));
                        // Java returns before interaction and resync for these invalid hits.
                        if !can_reach_block(position,server.current_metadata(entity_id)?,target)
                            || !geometry::valid_use_item_hit_location(target,hit) {
                            server.acknowledge_block_changes_up_to(entity_id,sequence)?;
                            continue;
                        }
                        if teleport_confirmed && server.use_native_furnace(entity_id,hand,target).await? {
                            server.finish_use_item_on(entity_id,target,direction,sequence).await?;
                            continue;
                        }
                        if teleport_confirmed && server.use_native_stonecutter(entity_id,hand,target).await? {
                            server.finish_use_item_on(entity_id,target,direction,sequence).await?;
                            continue;
                        }
                        if teleport_confirmed && server.use_native_grindstone(entity_id,hand,target).await? {
                            server.finish_use_item_on(entity_id,target,direction,sequence).await?;
                            continue;
                        }
                        if teleport_confirmed && server.use_native_anvil(entity_id,hand,target).await? {
                            server.finish_use_item_on(entity_id,target,direction,sequence).await?;
                            continue;
                        }
                        if teleport_confirmed && server.use_native_loom(entity_id,hand,target).await? {
                            server.finish_use_item_on(entity_id,target,direction,sequence).await?;
                            continue;
                        }
                        if teleport_confirmed && server.use_native_smithing_table(entity_id,hand,target).await? {
                            server.finish_use_item_on(entity_id,target,direction,sequence).await?;
                            continue;
                        }
                        if teleport_confirmed && server.use_native_crafting_table(entity_id,hand,target).await? {
                            server.finish_use_item_on(entity_id,target,direction,sequence).await?;
                            continue;
                        }
                        if teleport_confirmed && server.use_native_container(entity_id,hand,target).await? {
                            server.finish_use_item_on(entity_id,target,direction,sequence).await?;
                            continue;
                        }
                        let result = if teleport_confirmed {
                            let result=server.interact_world_block(entity_id,hand,target,direction,hit).await?;
                            if result == main_world_interactions::MainInteractionResult::Applied {server.drain_world_interaction_phase().await?;}
                            if result == main_world_interactions::MainInteractionResult::Unsupported {
                                eprintln!("{}",json!({"event":"world_interaction_unsupported","entity_id":entity_id,"target":target,"hand":hand}));
                            }
                            result
                        } else { main_world_interactions::MainInteractionResult::Pass };
                        if result == main_world_interactions::MainInteractionResult::Applied {
                            // The commit queued the correction pair before its ordinary block updates.
                            server.acknowledge_block_changes_up_to(entity_id,sequence)?;
                        } else {
                            server.finish_use_item_on(entity_id,target,direction,sequence).await?;
                        }
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
                    Ok(WorldUpdate::BlockResync{player,blocks}) if player==entity_id => {
                        for change in blocks {
                            send(&codec,&mut writer,0x09,encoded(|w| {
                                w.i64(pack_position(change.x,change.y,change.z)).varint(change.state as i32);
                            })).await?;
                        }
                    },
                    Ok(WorldUpdate::Light{chunk,payload}) if loaded.contains(&chunk) => send(&codec,&mut writer,0x2a,payload).await?,
                    Ok(WorldUpdate::Acknowledge{player,sequence}) if player==entity_id => send(&codec,&mut writer,0x05,encoded(|w|{w.varint(sequence);})).await?,
                    Ok(_) => {},
                    Err(broadcast::error::RecvError::Lagged(_)) => bail!("client fell behind fixture world changes"),
                    Err(broadcast::error::RecvError::Closed) => return Ok(PlayExit::Disconnected),
                }
            }
            update = entity_events.recv() => {
                match ordered_entity_input(update) {
                    OrderedEntityInput::Event(PlayerEvent::ChunksReady{recipient,cutoff,reconcile}) if recipient==entity_id => flush_chunk_transport(&server,entity_id,(cutoff,reconcile.as_deref()),&mut tracker,&codec,&mut writer,&mut loaded).await?,
                    OrderedEntityInput::Event(PlayerEvent::ObserverReconcile{recipient,reconcile}) if recipient==entity_id => send_packets(&codec,&mut writer,tracker.apply_reconcile(&reconcile,&loaded)?).await?,
                    OrderedEntityInput::Event(PlayerEvent::ObserverReconcile{..})=>{},
                    OrderedEntityInput::Event(PlayerEvent::ChunksReady{..})=>{},
                    OrderedEntityInput::Event(PlayerEvent::World {revision,packets})=>send_packets(&codec,&mut writer,tracker.world_event(revision,&packets)).await?,
                    OrderedEntityInput::Event(PlayerEvent::Scoreboard {revision,recipients,packets})=> {
                        if revision > scoreboard_revision && recipients.contains(&entity_id) {
                            send_packets(&codec,&mut writer,packets.as_ref().clone()).await?;
                            scoreboard_revision=revision;
                        }
                    },
                    OrderedEntityInput::Event(PlayerEvent::WorldAudience {recipients,packets})=> {
                        if recipients.contains(&entity_id) { send_packets(&codec,&mut writer,packets.as_ref().clone()).await?; }
                    },
                    OrderedEntityInput::Event(PlayerEvent::Item(event)) => send_packets(&codec,&mut writer,tracker.item_event(entity_id,&event,&loaded)?).await?,
                    OrderedEntityInput::Event(PlayerEvent::State(update)) => send_packets(&codec, &mut writer, tracker.players.event(&update, &loaded)?).await?,
                    OrderedEntityInput::Event(PlayerEvent::AdminDisconnect { recipient, reason }) => {
                        if recipient == entity_id {
                            send(&codec, &mut writer, 0x1d, reason).await?;
                            return Ok(PlayExit::Disconnected);
                        }
                    },
                    OrderedEntityInput::Event(PlayerEvent::ChatDisconnect { recipient, reason }) => {
                        if recipient == entity_id {
                            play_disconnect(&codec, &mut writer, reason).await?;
                            return Ok(PlayExit::Disconnected);
                        }
                    },
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
                        return Ok(PlayExit::Disconnected);
                    },
                    OrderedEntityInput::Closed => return Ok(PlayExit::Disconnected),
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
