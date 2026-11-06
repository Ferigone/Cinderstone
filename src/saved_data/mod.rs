//! `DimensionDataStorage` and `SavedData` (WP-G2 stage 2): the `data/<name>.dat` files of a
//! level, as a library. Nothing here is called by Main.
//!
//! A saved-data file is a gzip-compressed NBT document `{data: {...}, DataVersion: int}`.
//! [`DimensionDataStorage`] loads it lazily, caches the result (a missing or unreadable file is
//! cached as absent, as in Java), tracks dirty state per object and writes only dirty objects.
//! Evidence and limits: `docs/reference/saved-data.md`.

pub mod command_storage;
pub mod file;
pub mod forced_chunks;
pub mod java_collections;
pub mod owners;
pub mod storage;

#[cfg(test)]
mod owners_tests;
#[cfg(test)]
mod tests;

pub use command_storage::{CommandStorage, CommandStorageContainer};
pub use forced_chunks::ForcedChunks;
pub use owners::{random_sequences_factory, MapIndex, RANDOM_SEQUENCES};
pub use storage::{DataFixType, DimensionDataStorage, Factory, SavedData, SavedDataError};

/// `SharedConstants.getCurrentVersion().getDataVersion().getVersion()` for Minecraft 1.21.
pub const DATA_VERSION: i32 = 3953;
