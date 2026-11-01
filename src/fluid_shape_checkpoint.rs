//! Exact native singleton fluid-shape cache history, including unknown owners.
use crate::fluid_physics::{FluidKind, FluidShapeCache, FluidShapeHistory, FluidState};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedFluidShapeCache {
    entries: Vec<SavedEntry>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedEntry {
    lava: bool,
    source: bool,
    amount: i32,
    falling: bool,
    history: SavedHistory,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
enum SavedHistory {
    Unknown,
    Unqueried,
    CachedHeight(u32),
}
impl SavedFluidShapeCache {
    pub fn capture(cache: &FluidShapeCache) -> Result<Self> {
        let entries = cache
            .observations()
            .map(|(state, history)| SavedEntry {
                lava: state.kind == FluidKind::Lava,
                source: state.source,
                amount: state.amount,
                falling: state.falling,
                history: match history {
                    FluidShapeHistory::Unknown => SavedHistory::Unknown,
                    FluidShapeHistory::Unqueried => SavedHistory::Unqueried,
                    FluidShapeHistory::CachedHeight(height) => {
                        SavedHistory::CachedHeight(height.to_bits())
                    }
                },
            })
            .collect();
        let saved = Self { entries };
        saved.restore()?;
        Ok(saved)
    }
    pub fn restore(&self) -> Result<FluidShapeCache> {
        ensure!(
            self.entries.len() <= 36,
            "too many singleton fluid-shape cache keys"
        );
        let entries = self.entries.iter().map(|entry| {
            let state = FluidState {
                kind: if entry.lava {
                    FluidKind::Lava
                } else {
                    FluidKind::Water
                },
                source: entry.source,
                amount: entry.amount,
                falling: entry.falling,
                own_height: entry.amount as f32 / 9_f32,
            };
            let history = match entry.history {
                SavedHistory::Unknown => FluidShapeHistory::Unknown,
                SavedHistory::Unqueried => FluidShapeHistory::Unqueried,
                SavedHistory::CachedHeight(bits) => {
                    FluidShapeHistory::CachedHeight(f32::from_bits(bits))
                }
            };
            (state, history)
        });
        FluidShapeCache::from_observed_history(entries)
    }
}
