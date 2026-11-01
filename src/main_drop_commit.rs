//! Atomic application-side menu/drop commit over the proved core seam.
//! Caller owns the registry lock and its global unused entity-ID counter.
use super::*;
use mc121_rust::{
    container::DroppedStack,
    item_entity::{spawn_player_drop_with_factory, PlayerDrop},
    player_stats::{DropStatEvent, PlayerStats},
};

pub(super) struct DropCommitOwner<'a> {
    pub runtime: &'a mut RuntimeEntities,
    pub player_random: &'a mut LegacyRandomSource,
    pub stats: PlayerStats,
    pub entity_counter: &'a AtomicI32,
}

pub(super) struct DropCommitCallbacks<F, B, C> {
    pub prepare: F,
    pub before_spawn: B,
    pub commit: C,
}

pub(super) fn menu_action_with_items<J, F, B, C>(
    container: &SharedContainer,
    owner: DropCommitOwner<'_>,
    player: PlayerDrop,
    operation: impl FnOnce(&mut PlayerContainer) -> Result<ClickOutcome>,
    readiness: impl FnOnce(&[DroppedStack]) -> Result<()>,
    mut factory: impl FnMut() -> Result<LegacyRandomSource>,
    callbacks: DropCommitCallbacks<F, B, C>,
) -> Result<()>
where
    F: FnOnce(&mut PlayerContainer, PlayerStats) -> Result<J>,
    B: FnOnce(),
    C: FnOnce(J),
{
    let DropCommitCallbacks {
        prepare,
        before_spawn,
        commit,
    } = callbacks;
    let DropCommitOwner {
        runtime,
        player_random,
        stats,
        entity_counter,
    } = owner;
    let mut staged_player = player_random.clone();
    let mut staged_world = runtime.world_random_mut().clone();
    let mut staged_stats = stats;
    let first_id = entity_counter.load(Ordering::Relaxed);
    container.transact_with_commit(
        move |menu| {
            let outcome = operation(menu)?;
            let count = outcome
                .drops
                .iter()
                .filter(|drop| !drop.stack.is_empty())
                .count();
            let next_id = first_id
                .checked_add(i32::try_from(count)?)
                .context("entity ID space exhausted")?;
            // Check every stack and all world/gameplay capabilities before any
            // actual item factory or world/player random draw is requested.
            for drop in &outcome.drops {
                drop.stack.encode_canonical()?;
            }
            if count != 0 {
                readiness(&outcome.drops)?;
            }
            for drop in &outcome.drops {
                if !drop.stack.is_empty() {
                    // Supported native world-add has no stack-mutating listener.
                    staged_stats.award_drop(DropStatEvent {
                        constructed: true,
                        trace_item: drop.trace_item,
                        returned_item_id: Some(drop.stack.item_id),
                        original_count: drop.stack.count,
                    })?;
                }
            }
            let extra = prepare(menu, staged_stats)?;
            let mut entries = Vec::with_capacity(count);
            let mut id = first_id;
            for drop in outcome.drops {
                let context = PlayerDrop {
                    around: drop.around,
                    trace_item: drop.trace_item,
                    ..player
                };
                if let Some((state, random)) = spawn_player_drop_with_factory(
                    id,
                    context,
                    drop.stack,
                    &mut staged_player,
                    &mut staged_world,
                    &mut factory,
                )? {
                    entries.push((state, random));
                    id += 1;
                }
            }
            ensure!(
                id == next_id,
                "drop construction count changed after validation"
            );
            let prepared = runtime.prepare_items(entries)?;
            Ok((
                (prepared, staged_player, staged_world, extra, next_id),
                outcome.packets,
            ))
        },
        |(prepared, next_player, next_world, extra, next_id)| {
            *player_random = next_player;
            entity_counter.store(next_id, Ordering::Relaxed);
            before_spawn();
            prepared.commit_with_world_random(next_world);
            // ServerPlayer.drop awards after world-add and before the menu's
            // shared publisher. Dirty stats do not emit a packet automatically.
            commit(extra);
        },
    )
}

#[cfg(test)]
#[path = "../tools/main_drop_commit_tests.rs"]
mod tests;
