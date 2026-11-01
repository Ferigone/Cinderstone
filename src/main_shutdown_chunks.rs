//! Drain actual runnable chunk work before shutdown menu cleanup/checkpoints.
//!
//! A signal may arrive between the final player ticket mutation and the next
//! ordinary server poll. Menu-close drops still need the real current query
//! owners. Running their existing executors does not advance game time, tick
//! entities or fabricate completion of pending external generation futures.
use super::*;

impl Server {
    /// Retains the caller's world -> players locks; never re-enters an async
    /// server lock or lets a connection mutate tickets between drain and save.
    pub(super) fn finish_shutdown_chunk_tasks_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        game_time: i64,
    ) -> Result<()> {
        ensure!(
            self.shutting_down.load(Ordering::Acquire),
            "shutdown chunk drain requires stopped server admission"
        );
        // Imported/legacy transport profiles have no native world-action or
        // stable-light owner. Preserve their existing scope without claiming a
        // native scheduler certificate for that unsupported source.
        if !players.chunk_runtime.has_native_lighting_owner() {
            return Ok(());
        }
        let had_work = players.chunk_runtime.manager.has_scheduler_work();
        let LivePlayers {
            chunk_runtime,
            item_runtime,
            chunk_transports,
            ..
        } = players;
        // poll has its own finite MAX_POLL_TURNS budget and executes the same
        // sorter, priority, distance, readiness and visibility owners as the
        // ordinary main loop. Do not clear their queues or mark futures ready.
        let summary = chunk_runtime.poll(
            world,
            item_runtime,
            chunk_transports,
            &self.entity_events,
            game_time,
        )?;
        for update in chunk_runtime.take_lighting_updates() {
            let _ = self.changes.send(update);
        }
        ensure!(
            summary.quiescent && !chunk_runtime.manager.has_scheduler_work(),
            "shutdown chunk scheduler exceeded its runnable-work budget after {} turns; work remains retained",
            summary.turns
        );
        if had_work || !summary.diagnostics.is_empty() {
            eprintln!(
                "{}",
                json!({
                    "event": "shutdown_chunk_runtime_drained",
                    "turns": summary.turns,
                    "main_tasks": summary.main_tasks,
                    "sorter_tasks": summary.sorter_tasks,
                    "priority_batches": summary.priority_batches,
                    "visibility_updates": summary.visibility_updates,
                    "generation_pending": summary.generation_pending,
                    "diagnostics": summary.diagnostics,
                })
            );
        }
        Ok(())
    }
}
