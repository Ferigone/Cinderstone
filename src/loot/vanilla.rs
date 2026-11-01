//! The 1178 loot tables of the vanilla 1.21 data pack.
//!
//! `fixtures/loot-tables-1.21.json` holds the shipped JSON verbatim (key: table
//! ID). Parsing happens once, on first use, through the same codec rules the
//! server applies; a parse failure here means the port rejects a table Java
//! accepts, which `tests` guards against.

use super::host::LootResolver;
use super::{entry::LootTable, parse, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub struct VanillaTables {
    tables: BTreeMap<String, LootTable>,
}

impl VanillaTables {
    /// Parse the embedded data pack.
    pub fn load() -> Result<Self> {
        let document: Value =
            serde_json::from_str(include_str!("../../fixtures/loot-tables-1.21.json"))
                .map_err(|error| super::LootError::Codec(error.to_string()))?;
        let tables = document["tables"]
            .as_object()
            .ok_or_else(|| super::LootError::Codec("fixture has no tables object".to_owned()))?;
        let mut parsed = BTreeMap::new();
        for (key, body) in tables {
            parsed.insert(
                key.clone(),
                parse::table(body).map_err(|error| {
                    super::LootError::Codec(format!("{key}: {error}"))
                })?,
            );
        }
        Ok(Self { tables: parsed })
    }

    pub fn get(&self, key: &str) -> Option<&LootTable> {
        self.tables.get(&super::facts::canonical(key))
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.tables.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.tables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }
}

impl LootResolver for VanillaTables {
    fn loot_table(&self, key: &str) -> Option<&LootTable> {
        self.get(key)
    }
}

/// The shared parsed vanilla tables.
pub fn vanilla() -> &'static VanillaTables {
    static TABLES: OnceLock<VanillaTables> = OnceLock::new();
    TABLES.get_or_init(|| VanillaTables::load().expect("embedded vanilla loot tables parse"))
}
