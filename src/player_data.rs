//! Durable native player saves. This is deliberately not Java player.dat NBT.
//!
//! Slot/component and metadata payloads retain their network codecs, including
//! raw NBT/UTF-16 data, until native equivalents of every persistent NBT codec
//! exist. Inventory, carried stack, lifecycle and current-version statistics
//! share one atomic UUID file, so a crash cannot mix two inventory/stat saves.
use crate::{
    entity::{self, PlayerState, Position},
    equipment::PlayerEquipment,
    geometry::Vec3,
    inventory::{ItemStack, PlayerInventory, INVENTORY_SLOTS},
    metadata::{MetadataValue, PlayerEntityData},
    player_stats::PlayerStats,
};
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

const FORMAT: &str = "mc121-rust-native-player";
const VERSION: u32 = 1;
const MAX_SAVE_BYTES: u64 = 64 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoodData {
    pub level: i32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub tick_timer: i32,
}
impl Default for FoodData {
    fn default() -> Self {
        Self {
            level: 20,
            saturation: 5.,
            exhaustion: 0.,
            tick_timer: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilitiesData {
    pub invulnerable: bool,
    pub flying: bool,
    pub may_fly: bool,
    pub instant_build: bool,
    pub may_build: bool,
    pub flying_speed: f32,
    pub walking_speed: f32,
}
impl Default for AbilitiesData {
    fn default() -> Self {
        Self {
            invulnerable: false,
            flying: false,
            may_fly: false,
            instant_build: false,
            may_build: true,
            flying_speed: 0.05,
            walking_speed: 0.1,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperienceData {
    pub level: i32,
    pub total: i32,
    pub progress: f32,
}

/// Facts not represented in PlayerState. The host retains this owner and
/// updates it when food/abilities/experience/lifecycle mechanics change.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerLifecycle {
    pub dimension: String,
    /// Authoritative native motion state. Missing in older saves; absence must
    /// never be interpreted as a newly constructed stationary player.
    #[serde(default)]
    pub motion_velocity: Option<Vec3>,
    pub food: FoodData,
    pub abilities: AbilitiesData,
    pub experience: ExperienceData,
    pub remaining_fire_ticks: i32,
    pub fall_distance: f32,
    pub death_time: i32,
    pub sleep_timer: i32,
    /// Complete anonymous recipe-book compound, without JSON text conversion.
    #[serde(default)]
    pub recipe_book_nbt: Option<Vec<u8>>,
    /// Original current-version Java player.dat for lossless unknown-field
    /// retention. This is source data, not a substitute runtime state owner.
    #[serde(default)]
    pub java_source_nbt: Option<Vec<u8>>,
    /// Original Java statistics JSON, including unrecognized extension keys.
    #[serde(default)]
    pub java_stats_json: Option<String>,
}
impl PlayerLifecycle {
    /// Explicit new-player state; do not use this to replace an existing save.
    pub fn new(dimension: impl Into<String>, gamemode: i32) -> Result<Self> {
        ensure!((0..=3).contains(&gamemode), "invalid new-player gamemode");
        let abilities = AbilitiesData {
            invulnerable: matches!(gamemode, 1 | 3),
            may_fly: matches!(gamemode, 1 | 3),
            flying: gamemode == 3,
            instant_build: gamemode == 1,
            may_build: matches!(gamemode, 0 | 1),
            ..AbilitiesData::default()
        };
        let value = Self {
            dimension: dimension.into(),
            motion_velocity: Some(Vec3::ZERO),
            food: FoodData::default(),
            abilities,
            experience: ExperienceData::default(),
            remaining_fire_ticks: -1,
            fall_distance: 0.,
            death_time: 0,
            sleep_timer: 0,
            recipe_book_nbt: None,
            java_source_nbt: None,
            java_stats_json: None,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if let Some(velocity) = self.motion_velocity {
            ensure!(velocity.x.is_finite() && velocity.y.is_finite() && velocity.z.is_finite(),
                "invalid saved player motion velocity");
        }
        ensure!(
            !self.dimension.is_empty() && self.dimension.len() <= 32767,
            "invalid saved dimension key"
        );
        ensure!(
            (0..=20).contains(&self.food.level) && self.food.tick_timer >= 0,
            "invalid saved food state"
        );
        ensure!(
            [
                self.food.saturation,
                self.food.exhaustion,
                self.abilities.flying_speed,
                self.abilities.walking_speed,
                self.experience.progress,
                self.fall_distance,
            ]
            .into_iter()
            .all(|n| n.is_finite() && n >= 0.),
            "invalid saved lifecycle float"
        );
        ensure!(
            self.experience.progress <= 1.
                && self.experience.level >= 0
                && self.experience.total >= 0
                && self.death_time >= 0
                && self.sleep_timer >= 0,
            "invalid saved lifecycle counter"
        );
        if let Some(bytes) = &self.recipe_book_nbt {
            ensure!(
                matches!(
                    crate::nbt::Tag::decode_anonymous(bytes)?,
                    crate::nbt::Tag::Compound(_)
                ),
                "saved recipe book must be an anonymous compound"
            );
        }
        ensure!(self.java_source_nbt.as_ref().map_or(true, |v| v.len() as u64 <= MAX_SAVE_BYTES),
            "original Java player source exceeds64MiB");
        ensure!(self.java_stats_json.as_ref().map_or(true, |v| v.len() as u64 <= MAX_SAVE_BYTES),
            "original Java statistics source exceeds64MiB");
        Ok(())
    }
}

/// A complete native save transaction. Entity IDs, authenticated profiles,
/// latency, menu prediction/revisions and RNG identities are not persisted.
#[derive(Clone, Debug)]
pub struct PlayerData {
    pub uuid: [u8; 16],
    pub position: Position,
    pub gamemode: i32,
    pub entity_data: PlayerEntityData,
    pub inventory: PlayerInventory,
    pub stats: PlayerStats,
    pub lifecycle: PlayerLifecycle,
}
impl PlayerData {
    pub fn capture(
        player: &PlayerState,
        inventory: &PlayerInventory,
        stats: &PlayerStats,
        lifecycle: PlayerLifecycle,
    ) -> Result<Self> {
        player.validate()?;
        let value = Self {
            uuid: player.profile.uuid,
            position: player.position,
            gamemode: player.gamemode,
            entity_data: entity::prepared_entity_data(player)?,
            inventory: inventory.clone(),
            stats: stats.clone(),
            lifecycle,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        self.position.validate()?;
        ensure!((0..=3).contains(&self.gamemode), "invalid saved gamemode");
        self.lifecycle.validate()?;
        match self.entity_data.get(9) {
            Some(MetadataValue::Float(health)) => {
                ensure!(health.is_finite() && *health >= 0., "invalid saved health")
            }
            _ => anyhow::bail!("missing saved player health"),
        }
        ensure!(
            matches!(self.entity_data.get(6), Some(MetadataValue::Pose(0..=17))),
            "invalid saved pose"
        );
        ensure!(
            entity::basic_metadata(&self.entity_data)?.main_hand <= 1,
            "invalid saved main hand"
        );
        ensure!(
            self.inventory.slots().len() == INVENTORY_SLOTS && self.inventory.selected_hotbar() < 9,
            "invalid saved player inventory layout"
        );
        Ok(())
    }
    /// Apply before registry admission. Keep the fresh authenticated profile and
    /// allocated runtime entity ID. Container initialization is separate:
    /// PlayerContainer::from_inventory(saved.inventory.clone()).
    pub fn apply_to_player(&self, player: &mut PlayerState) -> Result<()> {
        self.validate()?;
        ensure!(
            player.profile.uuid == self.uuid,
            "saved player UUID does not match login"
        );
        let mut staged = player.clone();
        staged.position = self.position;
        staged.gamemode = self.gamemode;
        staged.entity_data = self.entity_data.clone();
        staged.entity_data.set(8, MetadataValue::Byte(0), false)?;
        staged.entity_data.clear_dirty();
        staged.metadata = entity::basic_metadata(&staged.entity_data)?;
        staged.equipment = Box::new(PlayerEquipment::from_inventory(&self.inventory)?);
        staged.validate()?;
        *player = staged;
        Ok(())
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let file = SaveFile {
            format: FORMAT.into(),
            version: VERSION,
            minecraft: "1.21".into(),
            protocol: 767,
            uuid: self.uuid,
            position: self.position,
            gamemode: self.gamemode,
            lifecycle: self.lifecycle.clone(),
            slots: self
                .inventory
                .slots()
                .iter()
                .map(encode_stack)
                .collect::<Result<_>>()?,
            carried: encode_stack(self.inventory.carried())?,
            selected_hotbar: self.inventory.selected_hotbar(),
            metadata: (0..21)
                .map(|id| {
                    Ok(STANDARD.encode(
                        self.entity_data
                            .get(id)
                            .context("missing saved metadata")?
                            .payload()?,
                    ))
                })
                .collect::<Result<_>>()?,
            stats: serde_json::from_str(&self.stats.to_stat_file_json()?)?,
        };
        let bytes = serde_json::to_vec(&file)?;
        ensure!(
            bytes.len() as u64 <= MAX_SAVE_BYTES,
            "native player save exceeds64MiB"
        );
        Ok(bytes)
    }
    pub fn from_bytes(bytes: &[u8], uuid: [u8; 16]) -> Result<Self> {
        ensure!(
            bytes.len() as u64 <= MAX_SAVE_BYTES,
            "native player save exceeds64MiB"
        );
        let file: SaveFile = serde_json::from_slice(bytes).context("invalid native player save")?;
        ensure!(
            file.format == FORMAT
                && file.version == VERSION
                && file.minecraft == "1.21"
                && file.protocol == 767,
            "unsupported native player save version; Java player.dat import is not implemented"
        );
        ensure!(
            file.uuid == uuid,
            "native player filename/content UUID mismatch"
        );
        ensure!(
            file.slots.len() == INVENTORY_SLOTS && file.metadata.len() == 21,
            "incomplete native player inventory/metadata"
        );
        let mut inventory = PlayerInventory::new();
        for (slot, encoded) in file.slots.iter().enumerate() {
            inventory.set_slot_authoritative(slot, decode_stack(encoded)?);
        }
        inventory.set_carried_authoritative(decode_stack(&file.carried)?);
        ensure!(file.selected_hotbar < 9, "invalid saved hotbar selection");
        inventory.set_selected_hotbar(file.selected_hotbar as i16)?;
        let mut entity_data = PlayerEntityData::default();
        for (id, encoded) in file.metadata.iter().enumerate() {
            entity_data.set(
                id as u8,
                MetadataValue::decode_player_field(id as u8, &STANDARD.decode(encoded)?)?,
                false,
            )?;
        }
        entity_data.clear_dirty();
        let value = Self {
            uuid,
            position: file.position,
            gamemode: file.gamemode,
            entity_data,
            inventory,
            stats: PlayerStats::from_current_stat_file_json(&serde_json::to_string(&file.stats)?)?,
            lifecycle: file.lifecycle,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveFile {
    format: String,
    version: u32,
    minecraft: String,
    protocol: i32,
    uuid: [u8; 16],
    position: Position,
    gamemode: i32,
    lifecycle: PlayerLifecycle,
    slots: Vec<String>,
    carried: String,
    selected_hotbar: usize,
    metadata: Vec<String>,
    stats: serde_json::Value,
}
fn encode_stack(stack: &ItemStack) -> Result<String> {
    Ok(STANDARD.encode(stack.encode_canonical()?))
}
fn decode_stack(encoded: &str) -> Result<ItemStack> {
    ItemStack::decode_exact(&STANDARD.decode(encoded)?)
        .context("invalid saved Slot/component payload")
}

/// Synchronous disk store. Serialize lifecycle saves per player; use a blocking
/// worker when embedding in an async server. Clone shares the write mutex.
#[derive(Clone, Debug)]
pub struct PlayerDataStore {
    directory: PathBuf,
    writes: Arc<Mutex<()>>,
}
impl PlayerDataStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            writes: Arc::new(Mutex::new(())),
        }
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn path(&self, uuid: [u8; 16]) -> PathBuf {
        self.directory
            .join(format!("{}.native.json", uuid_string(uuid)))
    }
    pub fn load(&self, uuid: [u8; 16]) -> Result<Option<PlayerData>> {
        let path = self.path(uuid);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| format!("open player save {}", path.display()))
            }
        };
        let mut bytes = Vec::new();
        file.take(MAX_SAVE_BYTES + 1).read_to_end(&mut bytes)?;
        PlayerData::from_bytes(&bytes, uuid)
            .map(Some)
            .with_context(|| format!("load player save {}", path.display()))
    }
    pub fn save(&self, data: &PlayerData) -> Result<()> {
        // Finish validation/encoding before touching any existing save.
        let bytes = data.to_bytes()?;
        let _guard = self
            .writes
            .lock()
            .map_err(|_| anyhow::anyhow!("player save mutex poisoned"))?;
        fs::create_dir_all(&self.directory)?;
        let path = self.path(data.uuid);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut attempts = 0;
        let (temporary, mut file) = loop {
            let temporary = self.directory.join(format!(
                ".{}.{}.{}.tmp",
                uuid_string(data.uuid),
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            match options.open(&temporary) {
                Ok(file) => break (temporary, file),
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists && attempts < 1024 =>
                {
                    attempts += 1;
                }
                Err(error) => return Err(error).context("create atomic player save replacement"),
            }
        };
        let result = (|| -> Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, &path)?;
            File::open(&self.directory)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.with_context(|| format!("save player data {}", path.display()))
    }
}
pub fn uuid_string(uuid: [u8; 16]) -> String {
    let hex: String = uuid.into_iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{entity::PlayerProfile, player_stats::StatKey};
    #[test]
    fn native_player_restart_smoke() {
        let directory = std::env::temp_dir().join(format!(
            "mc121-player-save-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let store = PlayerDataStore::new(&directory);
        let mut player = PlayerState::new(
            4,
            PlayerProfile::offline("SaveSmoke").unwrap(),
            Position {
                x: 11.5,
                y: 72.,
                z: -9.25,
                yaw: 37.,
                pitch: -12.,
                on_ground: true,
            },
        )
        .unwrap();
        player.gamemode = 0;
        player
            .entity_data
            .set(9, MetadataValue::Float(7.5), false)
            .unwrap();
        let mut inventory = PlayerInventory::new();
        inventory.set_slot_authoritative(0, ItemStack::new(1, 2).unwrap());
        inventory.set_slot_authoritative(44, ItemStack::new(1, 17).unwrap());
        inventory.set_carried_authoritative(ItemStack::new(1, 3).unwrap());
        inventory.set_selected_hotbar(8).unwrap();
        let mut stats = PlayerStats::default();
        stats.set(StatKey::new(4, 1).unwrap(), 29).unwrap();
        let mut lifecycle = PlayerLifecycle::new("minecraft:overworld", 0).unwrap();
        lifecycle.food.level = 9;
        let data = PlayerData::capture(&player, &inventory, &stats, lifecycle).unwrap();
        assert!(store.load(player.profile.uuid).unwrap().is_none());
        store.save(&data).unwrap();
        // New owner models a process restart, not an in-memory cache lookup.
        let restored = PlayerDataStore::new(&directory)
            .load(player.profile.uuid)
            .unwrap()
            .unwrap();
        assert_eq!(
            restored.inventory.window_items_payload(),
            inventory.window_items_payload()
        );
        assert_eq!(restored.inventory.selected_hotbar(), 8);
        assert_eq!(restored.stats.values(), stats.values());
        assert_eq!(restored.lifecycle.food.level, 9);
        let mut login = PlayerState::new(
            99,
            player.profile.clone(),
            Position {
                x: 0.,
                y: 65.,
                z: 0.,
                yaw: 0.,
                pitch: 0.,
                on_ground: false,
            },
        )
        .unwrap();
        restored.apply_to_player(&mut login).unwrap();
        assert_eq!(login.entity_id, 99);
        assert_eq!(login.position, player.position);
        assert!(matches!(
            login.entity_data.get(9),
            Some(MetadataValue::Float(7.5))
        ));
        fs::write(store.path(player.profile.uuid), b"broken").unwrap();
        assert!(store.load(player.profile.uuid).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
