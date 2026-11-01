//! Server-side vanilla 1.21 statistic values, dirty deltas and AwardStats.
//! Gameplay callers own the literal trigger point. This module does not award
//! advancements or update active scoreboard objectives.
use crate::protocol::{Packet, Writer};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub const ITEM_PICKED_UP: i32 = 4;
pub const ITEM_DROPPED: i32 = 5;
pub const CUSTOM: i32 = 8;
pub const CUSTOM_DROP: i32 = 22;
pub const DATA_VERSION: i32 = 3953;
pub const AWARD_STATS_PACKET: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StatKey {
    pub type_id: i32,
    pub value_id: i32,
}
impl StatKey {
    pub fn new(type_id: i32, value_id: i32) -> Result<Self> {
        let key = Self { type_id, value_id };
        key.validate()?;
        Ok(key)
    }
    pub fn validate(self) -> Result<()> {
        ensure!(
            stat_names(self).is_some(),
            "unknown vanilla statistic ({},{})",
            self.type_id,
            self.value_id
        );
        Ok(())
    }
}
#[derive(Deserialize)]
struct Registry {
    award_stats_packet_id: i32,
    types: Vec<StatType>,
    drop_key: StatKey,
}
#[derive(Deserialize)]
struct StatType {
    type_id: i32,
    type_name: String,
    values: Vec<StatValue>,
}
#[derive(Deserialize)]
struct StatValue {
    value_id: i32,
    value_name: String,
}
fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry: Registry =
            serde_json::from_str(include_str!("../fixtures/player-stats-registry.json"))
                .expect("validated original statistic registry");
        registry.types.sort_by_key(|kind| kind.type_id);
        assert_eq!(registry.types.len(), 9);
        assert_eq!(registry.award_stats_packet_id, AWARD_STATS_PACKET);
        assert_eq!(
            registry.drop_key,
            StatKey {
                type_id: CUSTOM,
                value_id: CUSTOM_DROP
            }
        );
        for (index, kind) in registry.types.iter_mut().enumerate() {
            assert_eq!(kind.type_id, index as i32);
            kind.values.sort_by_key(|value| value.value_id);
            assert!(kind
                .values
                .iter()
                .enumerate()
                .all(|(index, value)| value.value_id == index as i32));
        }
        registry
    })
}
pub fn stat_names(key: StatKey) -> Option<(&'static str, &'static str)> {
    let kind = registry().types.get(usize::try_from(key.type_id).ok()?)?;
    let value = kind.values.get(usize::try_from(key.value_id).ok()?)?;
    Some((&kind.type_name, &value.value_name))
}
pub fn stat_key(type_name: &str, value_name: &str) -> Option<StatKey> {
    let kind = registry()
        .types
        .iter()
        .find(|kind| kind.type_name == type_name)?;
    let value = kind
        .values
        .iter()
        .find(|value| value.value_name == value_name)?;
    Some(StatKey {
        type_id: kind.type_id,
        value_id: value.value_id,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct DropStatEvent {
    /// super Player.drop returned a constructed entity, rather than null.
    pub constructed: bool,
    pub trace_item: bool,
    /// Actual returned stack's nonempty item after the world-add attempt.
    pub returned_item_id: Option<i32>,
    /// Count read from the original argument at the award call site.
    pub original_count: i32,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerStats {
    values: BTreeMap<StatKey, i32>,
    dirty: BTreeSet<StatKey>,
}
impl PlayerStats {
    pub fn get(&self, key: StatKey) -> i32 {
        *self.values.get(&key).unwrap_or(&0)
    }
    pub fn values(&self) -> &BTreeMap<StatKey, i32> {
        &self.values
    }
    pub fn dirty_keys(&self) -> &BTreeSet<StatKey> {
        &self.dirty
    }
    /// ServerStatsCounter marks every set dirty, even unchanged/zero values.
    pub fn set(&mut self, key: StatKey, value: i32) -> Result<()> {
        key.validate()?;
        self.values.insert(key, value);
        self.dirty.insert(key);
        Ok(())
    }
    pub fn increment(&mut self, key: StatKey, amount: i32) -> Result<i32> {
        // The long sum is clamped only above INT_MAX, then Java l2i wraps
        // values below INT_MIN. Saturating i32 addition would change this.
        let value = (i64::from(self.get(key)) + i64::from(amount)).min(i64::from(i32::MAX)) as i32;
        self.set(key, value)?;
        Ok(value)
    }
    pub fn mark_all_dirty(&mut self) {
        self.dirty.extend(self.values.keys().copied());
    }
    pub fn drain_dirty(&mut self) -> BTreeMap<StatKey, i32> {
        std::mem::take(&mut self.dirty)
            .into_iter()
            .map(|key| (key, self.get(key)))
            .collect()
    }
    /// Original sendStats clears dirty before the outer connection send and
    /// emits a packet even for an empty delta. Multi-entry map order is not a
    /// protocol semantic; this host uses canonical key order.
    pub fn send_stats_packet(&mut self) -> Result<Packet> {
        encode_award_stats(&self.drain_dirty().into_iter().collect::<Vec<_>>())
    }
    pub fn award_pickup(&mut self, item_id: i32, original_count: i32) -> Result<()> {
        self.increment(StatKey::new(ITEM_PICKED_UP, item_id)?, original_count)?;
        Ok(())
    }
    pub fn award_drop(&mut self, event: DropStatEvent) -> Result<()> {
        if !event.constructed || !event.trace_item {
            return Ok(());
        }
        if let Some(item_id) = event.returned_item_id {
            self.increment(StatKey::new(ITEM_DROPPED, item_id)?, event.original_count)?;
        }
        self.increment(
            StatKey {
                type_id: CUSTOM,
                value_id: CUSTOM_DROP,
            },
            1,
        )?;
        Ok(())
    }
    /// Canonical current-version file data. JSON member order is intentionally
    /// canonical; original HashMap/Gson member order is not promised.
    pub fn to_stat_file_json(&self) -> Result<String> {
        let mut types = Map::<String, Value>::new();
        for (key, value) in &self.values {
            let (type_name, value_name) =
                stat_names(*key).context("invalid stored statistic key")?;
            types
                .entry(type_name.to_owned())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .expect("stat type object")
                .insert(value_name.to_owned(), json!(value));
        }
        Ok(serde_json::to_string(
            &json!({"stats":types,"DataVersion":DATA_VERSION}),
        )?)
    }
    /// Merge canonical valid 3953 data without marking new entries dirty, just
    /// as original parseLocal writes directly into the value map. Older DFU
    /// upgrades, malformed numbers and unknown registry names are explicit
    /// unsupported inputs, never silently treated as a compatible empty file.
    pub fn load_current_stat_file_json(&mut self, input: &str) -> Result<()> {
        let file: Value = serde_json::from_str(input)?;
        ensure!(
            file["DataVersion"].as_i64() == Some(i64::from(DATA_VERSION)),
            "stat file datafixing requires unsupported version"
        );
        let Some(types) = file.get("stats") else {
            return Ok(());
        };
        let mut entries = Vec::new();
        for (type_name, values) in types.as_object().context("stat types must be an object")? {
            for (value_name, value) in values
                .as_object()
                .context("stat values must be an object")?
            {
                let key = stat_key(type_name, value_name).context("unknown vanilla stat name")?;
                let value = value
                    .as_i64()
                    .and_then(|value| i32::try_from(value).ok())
                    .context("stat file requires canonical int32 values")?;
                entries.push((key, value));
            }
        }
        self.values.extend(entries);
        Ok(())
    }
    pub fn from_current_stat_file_json(input: &str) -> Result<Self> {
        let mut result = Self::default();
        result.load_current_stat_file_json(input)?;
        Ok(result)
    }
}

/// Preserve explicit ordered input when the caller has an ordered map proof.
/// Keys must be unique, as in the original Object2IntMap packet constructor.
pub fn encode_award_stats(entries: &[(StatKey, i32)]) -> Result<Packet> {
    let mut seen = BTreeSet::new();
    let mut out = Writer::new();
    out.varint(i32::try_from(entries.len()).context("stat packet map too large")?);
    for (key, value) in entries {
        key.validate()?;
        ensure!(seen.insert(*key), "duplicate statistic map key");
        out.varint(key.type_id).varint(key.value_id).varint(*value);
    }
    Ok(Packet {
        id: AWARD_STATS_PACKET,
        payload: out.into_inner(),
    })
}

#[cfg(test)]
#[path = "../tools/player_stats_tests.rs"]
mod tests;
