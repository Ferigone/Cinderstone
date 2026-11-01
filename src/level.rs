//! Lossless preservation of existing 1.21 level.dat with clock persistence.
//! This does not generate world settings or replace the complete LevelData API.
use crate::{
    border::BorderSettings,
    nbt::{NbtRoot, Tag},
    tick::{TickClock, WeatherState},
    world_rules::WorldRules,
};
use anyhow::{bail, ensure, Context, Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_LEVEL_BYTES: u64 = 16 * 1024 * 1024;

pub struct LevelData {
    path: PathBuf,
    root: NbtRoot,
}

fn long(data: &Tag, name: &str, default: i64) -> Result<i64> {
    match data.get(name) {
        Some(Tag::Long(value)) => Ok(*value),
        None => Ok(default),
        _ => bail!("invalid level.dat {name}"),
    }
}
fn int(data: &Tag, name: &str, default: i32) -> Result<i32> {
    match data.get(name) {
        Some(Tag::Int(value)) => Ok(*value),
        None => Ok(default),
        _ => bail!("invalid level.dat {name}"),
    }
}
fn flag(data: &Tag, name: &str, default: bool) -> Result<bool> {
    match data.get(name) {
        Some(Tag::Byte(value)) => Ok(*value != 0),
        None => Ok(default),
        _ => bail!("invalid level.dat {name}"),
    }
}

impl LevelData {
    pub fn open(world: impl AsRef<Path>) -> Result<Self> {
        let path = world.as_ref().join("level.dat");
        ensure!(
            fs::metadata(&path)?.len() <= MAX_LEVEL_BYTES,
            "level.dat exceeds compressed limit"
        );
        let mut bytes = Vec::new();
        GzDecoder::new(File::open(&path)?)
            .take(MAX_LEVEL_BYTES + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= MAX_LEVEL_BYTES,
            "level.dat exceeds decompressed limit"
        );
        let root = NbtRoot::decode(&bytes)?;
        let data = root
            .tag
            .get("Data")
            .context("level.dat has no Data compound")?;
        ensure!(
            matches!(data, Tag::Compound(_)),
            "level.dat Data must be a compound"
        );
        ensure!(
            int(data, "DataVersion", 0)? == 3953,
            "only exact Minecraft 1.21 level.dat is supported"
        );
        Ok(Self { path, root })
    }
    fn data(&self) -> &Tag {
        self.root.tag.get("Data").expect("validated Data compound")
    }
    pub fn clock(&self) -> Result<TickClock> {
        let daylight_cycle = self
            .rules()
            .boolean("doDaylightCycle")
            .expect("original rule catalog");
        let game_time = long(self.data(), "Time", 0)?;
        Ok(TickClock::new(
            game_time,
            long(self.data(), "DayTime", game_time)?,
            daylight_cycle,
        ))
    }
    pub fn rules(&self) -> WorldRules {
        WorldRules::read(self.data().get("GameRules"))
    }
    pub fn weather(&self) -> Result<WeatherState> {
        let mut weather = WeatherState {
            clear_weather_time: int(self.data(), "clearWeatherTime", 0)?,
            rain_time: int(self.data(), "rainTime", 0)?,
            thunder_time: int(self.data(), "thunderTime", 0)?,
            raining: flag(self.data(), "raining", false)?,
            thundering: flag(self.data(), "thundering", false)?,
            ..WeatherState::default()
        };
        weather.prepare_weather();
        Ok(weather)
    }
    pub fn spawn(&self) -> Result<(i32, i32, i32, f32)> {
        let angle = match self.data().get("SpawnAngle") {
            Some(Tag::Float(value)) => *value,
            None => 0.0,
            _ => bail!("invalid SpawnAngle"),
        };
        Ok((
            int(self.data(), "SpawnX", 0)?,
            int(self.data(), "SpawnY", 0)?,
            int(self.data(), "SpawnZ", 0)?,
            angle,
        ))
    }
    pub fn seed(&self) -> Result<i64> {
        self.data()
            .get("WorldGenSettings")
            .map(|settings| long(settings, "seed", 0))
            .transpose()
            .map(|value| value.unwrap_or(0))
    }
    /// Exact WorldBorder.Settings.read semantics, including numeric tag
    /// conversions and the official per-field defaults for missing values.
    pub fn border_settings(&self) -> BorderSettings {
        BorderSettings::read(self.data(), BorderSettings::default())
    }
    /// Persist the current interpolation state without replacing unrelated
    /// level.dat fields. save_clock/save_atomic performs the actual disk write.
    pub fn set_border_settings(&mut self, settings: &BorderSettings) -> Result<()> {
        settings.write(
            self.root
                .tag
                .get_mut("Data")
                .expect("validated Data compound"),
        )
    }
    /// Preserve every unrelated named tag and its original scalar type.
    /// Callers supply the persisted wall clock for deterministic tests.
    pub fn update(
        &mut self,
        clock: TickClock,
        weather: Option<WeatherState>,
        last_played_ms: i64,
    ) -> Result<()> {
        let data = self
            .root
            .tag
            .get_mut("Data")
            .expect("validated Data compound");
        data.insert("Time", Tag::Long(clock.game_time))?;
        data.insert("DayTime", Tag::Long(clock.day_time))?;
        data.insert("LastPlayed", Tag::Long(last_played_ms))?;
        if let Some(weather) = weather {
            for (name, value) in [
                ("clearWeatherTime", weather.clear_weather_time),
                ("rainTime", weather.rain_time),
                ("thunderTime", weather.thunder_time),
            ] {
                data.insert(name, Tag::Int(value))?;
            }
            data.insert("raining", Tag::Byte(i8::from(weather.raining)))?;
            data.insert("thundering", Tag::Byte(i8::from(weather.thundering)))?;
        }
        Ok(())
    }
    pub fn save_clock(&mut self, clock: TickClock) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
        self.update(clock, None, now)?;
        self.save_atomic()
    }
    /// Write and sync the replacement before touching either existing file.
    /// The primary rename is atomic; primary+backup aren't one transaction.
    pub fn save_atomic(&self) -> Result<()> {
        let parent = self.path.parent().context("level.dat has no directory")?;
        let temporary = parent.join("level.dat_new");
        let backup = parent.join("level.dat_old");
        let backup_temporary = parent.join("level.dat_old.tmp");
        let bytes = self.root.encode()?;
        ensure!(
            bytes.len() as u64 <= MAX_LEVEL_BYTES,
            "level.dat exceeds write limit"
        );
        let mut encoder = GzEncoder::new(File::create(&temporary)?, Compression::default());
        encoder.write_all(&bytes)?;
        encoder.finish()?.sync_all()?;
        if self.path.exists() {
            fs::copy(&self.path, &backup_temporary)?;
            File::open(&backup_temporary)?.sync_all()?;
            fs::rename(&backup_temporary, &backup)?;
        }
        fs::rename(&temporary, &self.path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    }
    pub fn document(&self) -> &NbtRoot {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> LevelData {
        LevelData {
            path: PathBuf::from("unused/level.dat"),
            root: NbtRoot {
                name: "".into(),
                tag: Tag::Compound(vec![(
                    "Data".into(),
                    Tag::Compound(vec![
                        ("DataVersion".into(), Tag::Int(3953)),
                        ("Time".into(), Tag::Long(11)),
                        ("DayTime".into(), Tag::Long(-1)),
                        (
                            "GameRules".into(),
                            Tag::Compound(vec![(
                                "doDaylightCycle".into(),
                                Tag::String("false".into()),
                            )]),
                        ),
                        ("opaque".into(), Tag::ByteArray(vec![-128, 127, 0])),
                    ]),
                )]),
            },
        }
    }
    #[test]
    fn update_keeps_unknown_tags_and_original_rules() {
        let mut level = fixture();
        let old = level.document().clone();
        let mut clock = level.clock().unwrap();
        clock.advance();
        level.update(clock, None, 123).unwrap();
        let data = level.data();
        assert_eq!(data.get("Time"), Some(&Tag::Long(12)));
        assert_eq!(data.get("DayTime"), Some(&Tag::Long(-1)));
        assert_eq!(data.get("LastPlayed"), Some(&Tag::Long(123)));
        for name in ["opaque", "GameRules", "DataVersion"] {
            assert_eq!(data.get(name), old.tag.get("Data").unwrap().get(name));
        }
    }
    #[test]
    fn clock_uses_original_gamerule_string_and_wrong_type_semantics() {
        let mut level = fixture();
        for (tag, expected) in [
            (Tag::String("TrUe".into()), true),
            (Tag::String(" true".into()), false),
            (Tag::Int(0), true),
        ] {
            level
                .root
                .tag
                .get_mut("Data")
                .unwrap()
                .get_mut("GameRules")
                .unwrap()
                .insert("doDaylightCycle", tag)
                .unwrap();
            assert_eq!(level.clock().unwrap().daylight_cycle, expected);
        }
    }
    #[test]
    fn weather_prepare_uses_persisted_flags() {
        let mut level = fixture();
        let data = level.root.tag.get_mut("Data").unwrap();
        data.insert("raining", Tag::Byte(1)).unwrap();
        data.insert("thundering", Tag::Byte(1)).unwrap();
        let weather = level.weather().unwrap();
        assert_eq!(weather.rain_level, 1.0);
        assert_eq!(weather.thunder_level, 1.0);
        assert_eq!(weather.old_rain_level, 0.0);
        assert_eq!(weather.old_thunder_level, 0.0);
    }
    #[test]
    fn border_checkpoint_preserves_unknown_fields_and_interpolation() {
        let mut level = fixture();
        let before = level.document().clone();
        let settings = BorderSettings {
            center_x: -18.5,
            size: 256.,
            size_lerp_time: 8765,
            size_lerp_target: 32.,
            warning_blocks: 9,
            ..BorderSettings::default()
        };
        level.set_border_settings(&settings).unwrap();
        assert_eq!(level.border_settings(), settings);
        for name in ["opaque", "GameRules", "DataVersion", "Time", "DayTime"] {
            assert_eq!(
                level.data().get(name),
                before.tag.get("Data").unwrap().get(name)
            );
        }
        assert_eq!(
            level.data().get("BorderWarningBlocks"),
            Some(&Tag::Double(9.))
        );
        assert_eq!(
            level.data().get("BorderSizeLerpTime"),
            Some(&Tag::Long(8765))
        );
    }
}
