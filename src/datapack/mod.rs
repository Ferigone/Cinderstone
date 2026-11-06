//! Data-pack plane (WP-E1). Currently: the tag loader (`TagLoader`, `TagFile`, `TagEntry`,
//! `DependencySorter`). Evidence and limits: `docs/reference/datapack-tags.md`.

pub mod config;
pub mod packs;
pub mod tags;
pub mod zip;

#[cfg(test)]
mod tests;
