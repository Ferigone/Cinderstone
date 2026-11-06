//! Data-pack plane (WP-E1). Currently: the tag loader (`TagLoader`, `TagFile`, `TagEntry`,
//! `DependencySorter`). Evidence and limits: `docs/reference/datapack-tags.md`.

pub mod tags;

#[cfg(test)]
mod tests;
