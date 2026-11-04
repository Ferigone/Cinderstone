# scoreboard-20261101-e31d — scoreboard model, packets and `scoreboard.dat` (WP-E4 stages 1–3)

Instance: scoreboard-20261101-e31d (same session as loot-20261101-e31d, components-20261101-e31d and
saved-data-20261101-e31d); team: lane E data-driven (no team issue). Claim:
[issue #25 comment](https://github.com/Ferigone/Cinderstone/issues/25#issuecomment-6568074930).
State: ready_for_integration once its pull request is green (library stage; no Main callsite)
Base: master `2ac2897`; branch `ccr-e31d38f2-ovzstg`.
Capacity: one Claude Code session, no subordinate agents. Nothing continues in the background after this turn.

## Owned files

New and exclusive: `src/scoreboard/**`, `fixtures/scoreboard-vectors.json`,
`fixtures/scoreboard-load-mutants.json`, `fixtures/scoreboard-facts-1.21.json`,
`tools/scoreboard_generate.py`, `docs/reference/scoreboard.md`, this file. Shared edits: one
`pub mod scoreboard;` line in `src/lib.rs` (standing export exception); `scoreboard`,
`scoreboard-load-mutants` and `scoreboard-facts` modes of `tools/loot_oracle.java` (this instance's
harness); in code this instance already owns: `src/components/**` (Gson text with the original's
member order, Java `Float/Double.toString`, a lenient JSON reader, a few `pub(crate)` seams, rustfmt
of the module) and `src/saved_data/storage.rs` (the `SavedDataScoreboard` fix type). **Not touched:**
`player_stats.rs`, chat, damage/survival modules, `src/level.rs`, `src/nbt.rs`, Main, manifests,
checklist, README, roadmap, journal and deployment files.

## Delivery IDs and bounded behavior

`commands_data.3` — **partial** (stages 1–3 only). Preserves `scope_version: 1`, 100 items,
**56/100 code, 51/100 Main** (unchanged; no point is added). Implemented: the scoreboard model with
the server layer's packet queue, objectives/criteria/number formats/scores/teams/display slots, the
five packets (bodies and framed), `scoreboard.dat` load/save on the `saved_data` framework, and the
fastutil open-hash order model the observable orders depend on. Not implemented: criterion hooks
(stage 4), team effects elsewhere (stage 5), commands (stage 6), formatted names, join-time delivery.
Details: [scoreboard.md](../../reference/scoreboard.md).

## Source evidence

Pinned server and official mappings; the **executed** unchanged original `ServerScoreboard`,
`ScoreboardSaveData`, `NumberFormatTypes` and packet classes (the server's player list is an
empty stub), plus direct inspection of the decompiled `Scoreboard`, `PlayerTeam`, fastutil usage
and `DataFixers` (output not committed). No REA tool execution is claimed.

## Validation

`cargo test --locked --lib scoreboard`: 6 tests, including **22 scenarios / 375 steps** (errors,
returns, every broadcast frame, full state dumps, saved NBT) and a **4003-mutant** `scoreboard.dat`
load net (3545 loaded, 458 rejected by the original) with zero disagreements. The earlier component
nets were re-run after the encoder changes (142,489 JSON mutants including `Component.Serializer.toJson`
text, 231,616 NBT mutants): zero disagreements. Not run: a protocol client, Main flow, deployment.
No Railway change, no deployment claim.

## Blockers / requested contracts

For the integration coordinator: own one `Scoreboard::new_server()` per server (through
`ScoreboardData::factory()` on the overworld's `DimensionDataStorage`, file `scoreboard`), drain
`take_outbox()` after each mutation (`Broadcast` to every player, `EachPlayer` to each online
player in order), send `start_tracking_packets` for every tracked objective at join, and call the
mutators from the command and stats owners (stages 4–6, unclaimed).

## Next action / handoff

Owner: this instance for stage 3 follow-ups; stages 4–6 stay unclaimed on #25 (stage 4 needs the
stats/health/kill owners, stage 6 needs WP-E3 selectors and the command tree). `Scoreboard`'s
`score` number provider and `entity_scores` condition in the loot port can now be wired to
`Scoreboard::player_score_info`.
