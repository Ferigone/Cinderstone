//! Java1.21 ChunkMap.TrackedEntity observer policy.
//!
//! Sent/loaded chunks alone do not establish entity visibility: vanilla also
//! checks horizontal block distance, the observer's requested view cap, server
//! range scaling, indirect passengers and entity.broadcastToPlayer. The chunk
//! gate is the current tracking view AND absence from the pending-send set.
use crate::geometry::Vec3;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::sync::OnceLock;

pub const DEFAULT_REQUESTED_VIEW_DISTANCE: i32 = 2;
#[derive(Clone, Copy, Debug)]
pub struct TrackingObserver {
    pub entity_id: i32,
    pub position: Vec3,
    pub requested_view_distance: i32,
    pub server_view_distance: i32,
    pub is_spectator: bool,
    pub tracking_distance_percent: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct TrackingTarget<'a> {
    pub entity_id: i32,
    pub position: Vec3,
    /// TrackedEntity.range is already measured in blocks, before server scaling.
    pub base_range_blocks: i32,
    /// Ranges of ALL indirect passengers, including nested riders, in chunks.
    pub indirect_passenger_ranges_chunks: &'a [i32],
    pub broadcast_allowed: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct ChunkTrackingGate {
    pub chunk_in_tracking_view: bool,
    pub pending_to_send: bool,
}
impl ChunkTrackingGate {
    pub fn allows(self) -> bool {
        self.chunk_in_tracking_view && !self.pending_to_send
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackingDecision {
    Unchanged,
    Pair,
    Unpair,
}
/// DedicatedServer.getScaledTrackingDistance uses i32 multiply then truncating
/// integer division. It does not use floating point or saturating arithmetic.
pub fn scaled_range(range_blocks: i32, percent: i32) -> i32 {
    percent.wrapping_mul(range_blocks) / 100
}
pub fn effective_range(
    base_range_blocks: i32,
    indirect_passenger_ranges_chunks: &[i32],
    percent: i32,
) -> i32 {
    let mut range = base_range_blocks;
    for &chunks in indirect_passenger_ranges_chunks {
        let passenger = chunks.wrapping_mul(16);
        if passenger > range {
            range = passenger;
        }
    }
    scaled_range(range, percent)
}
pub fn player_view_distance(requested: i32, server: i32) -> i32 {
    requested.max(2).min(server)
}
pub fn tracking_radius(observer: TrackingObserver, target: TrackingTarget<'_>) -> f64 {
    effective_range(
        target.base_range_blocks,
        target.indirect_passenger_ranges_chunks,
        observer.tracking_distance_percent,
    )
    .min(
        player_view_distance(
            observer.requested_view_distance,
            observer.server_view_distance,
        )
        .wrapping_mul(16),
    ) as f64
}
pub fn within_tracking_range(observer: TrackingObserver, target: TrackingTarget<'_>) -> bool {
    let delta = observer.position.subtract(target.position);
    let radius = tracking_radius(observer, target);
    delta.x * delta.x + delta.z * delta.z <= radius * radius
}
/// Captures the original lazy call order for providers with live broadcast/chunk
/// gates. Self returns immediately and does not alter an existing seenBy entry.
pub fn tracking_decision_with_gates(
    observer: TrackingObserver,
    target: TrackingTarget<'_>,
    previously_visible: bool,
    broadcast: impl FnOnce() -> bool,
    chunk_gate: impl FnOnce() -> bool,
) -> TrackingDecision {
    if observer.entity_id == target.entity_id {
        return TrackingDecision::Unchanged;
    }
    let allowed = within_tracking_range(observer, target) && broadcast() && chunk_gate();
    match (previously_visible, allowed) {
        (false, true) => TrackingDecision::Pair,
        (true, false) => TrackingDecision::Unpair,
        _ => TrackingDecision::Unchanged,
    }
}
pub fn tracking_decision(
    observer: TrackingObserver,
    target: TrackingTarget<'_>,
    gate: ChunkTrackingGate,
    previously_visible: bool,
) -> TrackingDecision {
    tracking_decision_with_gates(
        observer,
        target,
        previously_visible,
        || target.broadcast_allowed,
        || gate.allows(),
    )
}
/// Original ServerPlayer.broadcastToPlayer. Spectator observers see a player
/// only while that player's camera is itself; ordinary observers cannot see a
/// spectator player. Other vanilla Entity implementations use the base true.
pub fn player_broadcast_to_observer(
    observer_is_spectator: bool,
    target_is_spectator: bool,
    target_camera_is_self: bool,
) -> bool {
    if observer_is_spectator {
        target_camera_is_self
    } else {
        !target_is_spectator
    }
}
/// Original ChunkTrackingView.Positioned.contains shape. include_extra=true is
/// contains(), while false is isInViewDistance(). Intermediate i32 operations
/// intentionally wrap, including distance²; the two squared deltas use i64.
pub fn chunk_within_distance(
    center: (i32, i32),
    view_distance: i32,
    chunk: (i32, i32),
    include_extra: bool,
) -> bool {
    let x = chunk
        .0
        .wrapping_sub(center.0)
        .wrapping_abs()
        .wrapping_sub(1)
        .max(0);
    let z = chunk
        .1
        .wrapping_sub(center.1)
        .wrapping_abs()
        .wrapping_sub(1)
        .max(0);
    let max = x.max(z).wrapping_sub(i32::from(include_extra)).max(0) as i64;
    let min = x.min(z) as i64;
    min.wrapping_mul(min).wrapping_add(max.wrapping_mul(max))
        < (view_distance.wrapping_mul(view_distance) as i64)
}
pub fn chunk_view_contains(center: (i32, i32), view_distance: i32, chunk: (i32, i32)) -> bool {
    chunk_within_distance(center, view_distance, chunk, true)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkView {
    pub center: (i32, i32),
    pub view_distance: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkViewChange {
    Add((i32, i32)),
    Remove((i32, i32)),
}
impl ChunkView {
    fn bounds(self) -> Result<(i32, i32, i32, i32)> {
        anyhow::ensure!(
            (0..=32).contains(&self.view_distance),
            "unsupported chunk view radius"
        );
        let extent = self.view_distance + 1;
        Ok((
            self.center
                .0
                .checked_sub(extent)
                .ok_or_else(|| anyhow!("chunk view coordinate overflow"))?,
            self.center
                .1
                .checked_sub(extent)
                .ok_or_else(|| anyhow!("chunk view coordinate overflow"))?,
            self.center
                .0
                .checked_add(extent)
                .ok_or_else(|| anyhow!("chunk view coordinate overflow"))?,
            self.center
                .1
                .checked_add(extent)
                .ok_or_else(|| anyhow!("chunk view coordinate overflow"))?,
        ))
    }
    pub fn contains(self, chunk: (i32, i32)) -> bool {
        chunk_view_contains(self.center, self.view_distance, chunk)
    }
    /// Positioned.forEach: inclusive center±(radius+1), X outer / Z inner, then
    /// contains(include_extra=true). The radius is limited to vanilla's bound.
    pub fn positions(self) -> Result<Vec<(i32, i32)>> {
        let (min_x, min_z, max_x, max_z) = self.bounds()?;
        let mut out = Vec::new();
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                if self.contains((x, z)) {
                    out.push((x, z));
                }
            }
        }
        Ok(out)
    }
}
pub fn chunk_view_difference(
    old: Option<ChunkView>,
    new: Option<ChunkView>,
) -> Result<Vec<ChunkViewChange>> {
    if old == new {
        return Ok(Vec::new());
    }
    let mut changes = Vec::new();
    if let (Some(old), Some(new)) = (old, new) {
        let (a, b, c, d) = old.bounds()?;
        let (e, f, g, h) = new.bounds()?;
        if a <= g && c >= e && b <= h && d >= f {
            for x in a.min(e)..=c.max(g) {
                for z in b.min(f)..=d.max(h) {
                    let before = old.contains((x, z));
                    let after = new.contains((x, z));
                    if before != after {
                        changes.push(if after {
                            ChunkViewChange::Add((x, z))
                        } else {
                            ChunkViewChange::Remove((x, z))
                        });
                    }
                }
            }
            return Ok(changes);
        }
    }
    if let Some(old) = old {
        changes.extend(old.positions()?.into_iter().map(ChunkViewChange::Remove));
    }
    if let Some(new) = new {
        changes.extend(new.positions()?.into_iter().map(ChunkViewChange::Add));
    }
    Ok(changes)
}
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct EntityTypeTrackingFacts {
    pub id: u32,
    pub range_chunks: i32,
    pub update_interval: i32,
    pub track_deltas: bool,
}
#[derive(Deserialize)]
struct TypeFacts {
    rows: Vec<EntityTypeTrackingFacts>,
    default_requested_view_distance: i32,
}
fn facts() -> &'static TypeFacts {
    static FACTS: OnceLock<TypeFacts> = OnceLock::new();
    FACTS.get_or_init(|| {
        let data: TypeFacts =
            serde_json::from_str(include_str!("../tools/entity_tracking_facts.json"))
                .expect("verified exact1.21 entity tracking facts");
        assert_eq!(
            data.default_requested_view_distance,
            DEFAULT_REQUESTED_VIEW_DISTANCE
        );
        data
    })
}
pub fn entity_type_facts(id: u32) -> Result<&'static EntityTypeTrackingFacts> {
    facts()
        .rows
        .iter()
        .find(|r| r.id == id)
        .ok_or_else(|| anyhow!("unknown exact1.21 entity type{id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::cell::RefCell;
    fn d(v: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
    }
    fn vector(v: &Value) -> Vec3 {
        Vec3::new(d(&v[0]), d(&v[1]), d(&v[2]))
    }
    fn integer(v: &Value, n: &str, default: i32) -> i32 {
        v[n].as_i64().map(|v| v as i32).unwrap_or(default)
    }
    fn flag(v: &Value, n: &str) -> bool {
        v[n].as_bool().unwrap_or(false)
    }
    fn flatten(v: &Value, result: &mut Vec<i32>) {
        if let Some(rows) = v.as_array() {
            for child in rows {
                result.push(
                    entity_type_facts(child["type"].as_u64().unwrap() as u32)
                        .unwrap()
                        .range_chunks,
                );
                flatten(&child["passengers"], result);
            }
        }
    }
    fn java_floor(v: f64) -> i32 {
        let i = v as i32;
        if v < (i as f64) {
            i.wrapping_sub(1)
        } else {
            i
        }
    }
    fn run(q: &Value) -> Value {
        let mut visible = flag(q, "previous");
        let mut result = Vec::new();
        let fallback = vec![json!({})];
        let steps = q["steps"].as_array().unwrap_or(&fallback);
        for step in steps {
            let mut context = q.clone();
            for (k, v) in step.as_object().unwrap() {
                context
                    .as_object_mut()
                    .unwrap()
                    .insert(k.clone(), v.clone());
            }
            let q = &context;
            let same = flag(q, "same");
            let observer = TrackingObserver {
                entity_id: 1,
                position: if same {
                    vector(&q["entity"])
                } else {
                    vector(&q["observer"])
                },
                requested_view_distance: integer(q, "requested", 10),
                server_view_distance: integer(q, "server_view", 10),
                is_spectator: flag(q, "observer_spectator"),
                tracking_distance_percent: integer(q, "percent", 100),
            };
            let mut passengers = Vec::new();
            flatten(&q["passengers"], &mut passengers);
            let target = TrackingTarget {
                entity_id: if same { 1 } else { 2 },
                position: vector(&q["entity"]),
                base_range_blocks: integer(q, "range", 96),
                indirect_passenger_ranges_chunks: &passengers,
                broadcast_allowed: if flag(q, "player_target") {
                    player_broadcast_to_observer(
                        observer.is_spectator,
                        flag(q, "target_spectator"),
                        flag(q, "camera_self"),
                    )
                } else {
                    flag(q, "broadcast")
                },
            };
            let chunk = q
                .get("entity_chunk")
                .map(|v| (v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32))
                .unwrap_or_else(|| {
                    (
                        java_floor(target.position.x) >> 4,
                        java_floor(target.position.z) >> 4,
                    )
                });
            let contains = chunk_view_contains(
                (
                    q["center"][0].as_i64().unwrap() as i32,
                    q["center"][1].as_i64().unwrap() as i32,
                ),
                integer(q, "view_radius", 10),
                chunk,
            );
            let events = RefCell::new(Vec::new());
            let decision = tracking_decision_with_gates(
                observer,
                target,
                visible,
                || {
                    events.borrow_mut().push("broadcast");
                    target.broadcast_allowed
                },
                || {
                    events.borrow_mut().push("chunk_view");
                    contains && !flag(q, "pending")
                },
            );
            let mut events = events.into_inner();
            match decision {
                TrackingDecision::Pair => {
                    visible = true;
                    events.push("pair");
                }
                TrackingDecision::Unpair => {
                    visible = false;
                    events.push("unpair");
                }
                TrackingDecision::Unchanged => {}
            }
            result.push(json!({"events":events,"visible":visible,"effective_range":effective_range(target.base_range_blocks,&passengers,observer.tracking_distance_percent),"player_view":player_view_distance(observer.requested_view_distance,observer.server_view_distance),"view_contains":contains}));
        }
        json!(result)
    }
    #[test]
    fn actual_original_tracked_entity_policy_and_pairing_transitions() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/entity_tracking_vectors.json")).unwrap();
        assert_eq!(
            fixture["oracle_inner_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let mut diffs = Vec::new();
        for (i, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
            let actual = run(&row["query"]);
            if actual != row["expected"] {
                if diffs.len() < 6 {
                    eprintln!(
                        "case{i} {}: {}\nnative={actual}\njava={}",
                        row["category"], row["query"], row["expected"]
                    );
                }
                diffs.push(i);
            }
        }
        assert!(
            diffs.is_empty(),
            "{} original tracking differences,first{:?}",
            diffs.len(),
            &diffs[..diffs.len().min(6)]
        );
    }

    #[test]
    fn actual_original_chunk_view_iteration_and_difference_order() {
        let data: Value =
            serde_json::from_str(include_str!("../fixtures/entity_tracking_views.json")).unwrap();
        for row in data["iterations"].as_array().unwrap() {
            let view = ChunkView {
                center: (
                    row["center"][0].as_i64().unwrap() as i32,
                    row["center"][1].as_i64().unwrap() as i32,
                ),
                view_distance: row["radius"].as_i64().unwrap() as i32,
            };
            assert_eq!(json!(view.positions().unwrap()), row["expected"]);
        }
        for row in data["differences"].as_array().unwrap() {
            let v = row["query"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_i64().unwrap() as i32)
                .collect::<Vec<_>>();
            let old = ChunkView {
                center: (v[0], v[1]),
                view_distance: v[2],
            };
            let new = ChunkView {
                center: (v[3], v[4]),
                view_distance: v[5],
            };
            let changes = chunk_view_difference(Some(old), Some(new))
                .unwrap()
                .into_iter()
                .map(|change| match change {
                    ChunkViewChange::Add((x, z)) => json!(["add", x, z]),
                    ChunkViewChange::Remove((x, z)) => json!(["remove", x, z]),
                })
                .collect::<Vec<_>>();
            assert_eq!(json!(changes), row["expected"]);
        }
    }
}
