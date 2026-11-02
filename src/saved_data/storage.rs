//! `DimensionDataStorage`, `SavedData` and `SavedData.Factory`.

use super::file;
use super::DATA_VERSION;
use crate::nbt::{JavaString, Tag};
use std::any::Any;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavedDataError {
    /// The file could not be read or written.
    Io(String),
    /// The file is not a valid NBT document (Java logs this and treats the data as absent).
    Corrupt(String),
    /// Upgrading the file needs data fixers this library does not have.
    DataFixerRequired { fix: &'static str, from: i32 },
    /// The cached object under this name is of another type.
    TypeMismatch(String),
    /// The name would leave the data folder.
    InvalidName(String),
}

impl std::fmt::Display for SavedDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(what) => write!(f, "saved data I/O error: {what}"),
            Self::Corrupt(what) => write!(f, "corrupt saved data: {what}"),
            Self::DataFixerRequired { fix, from } => {
                write!(
                    f,
                    "{fix} data of version {from} needs data fixers that are not ported"
                )
            }
            Self::TypeMismatch(name) => write!(f, "saved data {name} is cached as another type"),
            Self::InvalidName(name) => write!(f, "invalid saved data name {name:?}"),
        }
    }
}

impl std::error::Error for SavedDataError {}

/// `DataFixTypes` for the saved data this library owns. Each variant lists every fixer the
/// original registers for that type (found by inspecting `DataFixers`), so `update` is exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFixType {
    /// `SAVED_DATA_MAP_INDEX`: no fixer is registered for this type.
    SavedDataMapIndex,
    /// `SAVED_DATA_RANDOM_SEQUENCES`: `RandomSequenceSettingsFix` at data version 3565.
    SavedDataRandomSequences,
    /// `SAVED_DATA_SCOREBOARD`: several fixers rewrite it (display slots, text components), so only
    /// data already at the target version is accepted.
    SavedDataScoreboard,
}

impl DataFixType {
    /// `DataFixTypes.update(fixer, tag, from, to)` for a whole file document. Data newer than
    /// `to` is returned unchanged.
    pub fn update(self, root: Tag, from: i32, to: i32) -> Result<Tag, SavedDataError> {
        match self {
            Self::SavedDataMapIndex => Ok(root),
            Self::SavedDataScoreboard => {
                if from >= to {
                    Ok(root)
                } else {
                    Err(SavedDataError::DataFixerRequired {
                        fix: "saved_data/scoreboard",
                        from,
                    })
                }
            }
            Self::SavedDataRandomSequences => {
                const SETTINGS_FIX: i32 = 3565;
                if from < SETTINGS_FIX && SETTINGS_FIX <= to {
                    Ok(random_sequence_settings_fix(root))
                } else {
                    Ok(root)
                }
            }
        }
    }
}

/// `RandomSequenceSettingsFix`: `data` becomes `{sequences: data}` when present.
fn random_sequence_settings_fix(root: Tag) -> Tag {
    let Tag::Compound(mut entries) = root else {
        return root;
    };
    for (name, value) in entries.iter_mut() {
        if name == "data" {
            let old = std::mem::replace(value, Tag::Compound(Vec::new()));
            *value = Tag::Compound(vec![(JavaString::from("sequences"), old)]);
        }
    }
    Tag::Compound(entries)
}

/// `SavedData`: an owner of one `data/<name>.dat` payload.
pub trait SavedData: Any {
    /// `save(CompoundTag, Provider)`: the payload stored under `data`.
    fn save(&self) -> Tag;
    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, dirty: bool);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// `SavedData.Factory`.
pub struct Factory<T> {
    pub constructor: Box<dyn Fn() -> T>,
    /// Receives `data` (an empty compound when the file has none). `Err` is Java's thrown
    /// exception: logged, and the data is treated as absent.
    pub deserializer: Box<dyn Fn(&Tag) -> Result<T, String>>,
    pub fix: DataFixType,
}

/// `DimensionDataStorage` over one `data` folder.
pub struct DimensionDataStorage {
    folder: PathBuf,
    cache: HashMap<String, Option<Box<dyn SavedData>>>,
    log: Vec<String>,
}

impl DimensionDataStorage {
    pub fn new(folder: impl Into<PathBuf>) -> Self {
        Self {
            folder: folder.into(),
            cache: HashMap::new(),
            log: Vec::new(),
        }
    }

    /// Messages Java would have logged at error level (unreadable file, failed write).
    pub fn diagnostics(&self) -> &[String] {
        &self.log
    }

    fn data_file(&self, name: &str) -> Result<PathBuf, SavedDataError> {
        if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
            return Err(SavedDataError::InvalidName(name.to_owned()));
        }
        Ok(self.folder.join(format!("{name}.dat")))
    }

    /// `readTagFromDisk`: the whole document after the data fixers up to `version`.
    pub fn read_tag_from_disk(
        &self,
        name: &str,
        fix: DataFixType,
        version: i32,
    ) -> Result<Tag, SavedDataError> {
        let bytes = fs::read(self.data_file(name)?)
            .map_err(|error| SavedDataError::Io(error.to_string()))?;
        let root = file::read_root(&bytes)?;
        let from = file::data_version(&root, 1343);
        fix.update(root, from, version)
    }

    fn read_saved_data<T: SavedData>(
        &mut self,
        factory: &Factory<T>,
        name: &str,
    ) -> Result<Option<T>, SavedDataError> {
        let path = self.data_file(name)?;
        if !path.exists() {
            return Ok(None);
        }
        match self.read_tag_from_disk(name, factory.fix, DATA_VERSION) {
            Ok(root) => match (factory.deserializer)(&file::compound_member(&root, "data")) {
                Ok(data) => Ok(Some(data)),
                Err(error) => {
                    self.log
                        .push(format!("Error loading saved data: {name}: {error}"));
                    Ok(None)
                }
            },
            // Needing a fixer we lack is not a read failure: treating it as absent would let a
            // later save replace the file with a fresh object.
            Err(error @ SavedDataError::DataFixerRequired { .. }) => Err(error),
            Err(error) => {
                self.log
                    .push(format!("Error loading saved data: {name}: {error}"));
                Ok(None)
            }
        }
    }

    /// `get`: the cached object, loading it on first use. A missing or unreadable file is cached
    /// as absent, so a file that appears later is not seen until [`Self::set`] replaces it.
    pub fn get<T: SavedData>(
        &mut self,
        factory: &Factory<T>,
        name: &str,
    ) -> Result<Option<&mut T>, SavedDataError> {
        if !self.cache.contains_key(name) {
            let loaded = self.read_saved_data(factory, name)?;
            self.cache.insert(
                name.to_owned(),
                loaded.map(|data| Box::new(data) as Box<dyn SavedData>),
            );
        }
        match self.cache.get_mut(name).and_then(Option::as_mut) {
            None => Ok(None),
            Some(data) => data
                .as_any_mut()
                .downcast_mut::<T>()
                .map(Some)
                .ok_or_else(|| SavedDataError::TypeMismatch(name.to_owned())),
        }
    }

    /// `computeIfAbsent`: a new object from the factory is cached but not marked dirty.
    pub fn compute_if_absent<T: SavedData>(
        &mut self,
        factory: &Factory<T>,
        name: &str,
    ) -> Result<&mut T, SavedDataError> {
        if self.get(factory, name)?.is_none() {
            self.set(name, (factory.constructor)());
        }
        self.get(factory, name)?
            .ok_or_else(|| SavedDataError::TypeMismatch(name.to_owned()))
    }

    /// `set`: replaces the cached object; dirty state is the object's own.
    pub fn set<T: SavedData>(&mut self, name: &str, data: T) {
        self.cache.insert(name.to_owned(), Some(Box::new(data)));
    }

    /// `save`: writes every dirty cached object. A failed write is logged and leaves the object
    /// dirty (Java clears the flag regardless), so the next save retries.
    pub fn save(&mut self) {
        let mut names: Vec<String> = self.cache.keys().cloned().collect();
        names.sort();
        for name in names {
            let Some(Some(data)) = self.cache.get(&name) else {
                continue;
            };
            if !data.is_dirty() {
                continue;
            }
            let outcome = self.write_file(&name, data.as_ref());
            match outcome {
                Ok(()) => {
                    if let Some(Some(data)) = self.cache.get_mut(&name) {
                        data.set_dirty(false);
                    }
                }
                Err(error) => self
                    .log
                    .push(format!("Could not save data {name}: {error}")),
            }
        }
    }

    fn write_file(&self, name: &str, data: &dyn SavedData) -> Result<(), SavedDataError> {
        let path = self.data_file(name)?;
        let root = Tag::Compound(vec![
            (JavaString::from("data"), data.save()),
            (JavaString::from("DataVersion"), Tag::Int(DATA_VERSION)),
        ]);
        let bytes = file::write_root(&root)?;
        // Write a sibling file, flush it to disk and rename it over the target, so a crash
        // leaves the old or the new file, never a torn one (Java truncates in place).
        let temporary = self.folder.join(format!("{name}.dat_new"));
        let io = |error: std::io::Error| SavedDataError::Io(error.to_string());
        let mut handle = fs::File::create(&temporary).map_err(io)?;
        handle.write_all(&bytes).map_err(io)?;
        handle.sync_all().map_err(io)?;
        fs::rename(&temporary, &path).map_err(io)
    }
}
