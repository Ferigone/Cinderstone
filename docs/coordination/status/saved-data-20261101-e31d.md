# saved-data-20261101-e31d — `DimensionDataStorage` and the first saved-data owners (WP-G2 stage 2)

Instance: saved-data-20261101-e31d (same session as loot-20261101-e31d and components-20261101-e31d);
team: lane G world/persistence (no team issue). Claim:
[issue #41 comment](https://github.com/Ferigone/Cinderstone/issues/41#issuecomment-6567879423).
State: ready_for_integration once its pull request is green (library stage; no Main callsite)
Base: master `2b4d209`; branch `ccr-e31d38f2-ovzstg`.
Capacity: one Claude Code session, no subordinate agents. Nothing continues in the background after this turn.

## Owned files

New and exclusive: `src/saved_data/**`, `fixtures/saved-data-vectors.json`,
`fixtures/saved-data-mutants.json`, `tools/saved_data_generate.py`,
`docs/reference/saved-data.md`, this file. Shared edits: one `pub mod saved_data;` line in
`src/lib.rs` (standing export exception); the `saved-data` and `saved-data-mutants` modes of
`tools/loot_oracle.java` (this instance's harness); in code this instance already owns:
`src/loot/sequences.rs` (load fidelity, `reset`/`clear` no longer mark dirty, `set_dirty`) and
`src/components/java.rs` (`resource_location`, `numeric_as_int/long`, shared with the component
decoder). **Not touched:** `src/level.rs`, `src/world_rules.rs`, `src/storage.rs`, `src/nbt.rs`,
Main, manifests, checklist, README, roadmap, journal and deployment files.

## Delivery IDs and bounded behavior

`world_storage.4` — **partial** (stage 2 and the first owners of stage 3 only). Preserves
`scope_version: 1`, 100 items, **56/100 code, 51/100 Main** (unchanged; no point is added).
Implemented: the `data/<name>.dat` envelope (gzip or plain read, gzip write, atomic replace), lazy
load with a cache that remembers absence, dirty tracking, save-only-if-dirty, unknown files never
rewritten, exact data-fixer handling for the two owned types, and the owners `idcounts`
(`MapIndex`) and `random_sequences` (`RandomSequences`). Not implemented: `level.dat` field owners,
per-dimension storage, save ordering, any other owner, any other data fixer, Main wiring. Details:
[saved-data.md](../../reference/saved-data.md).

## Source evidence

Pinned server and official mappings (identities in the reference document); the **executed**
unchanged original `DimensionDataStorage`, `SavedData`, `RandomSequences` and `MapIndex` through the
local oracle, plus direct inspection of `DataFixers`/`DataFixTypes` to list the fixers of the two
types (decompiled text not committed). No REA tool execution is claimed.

## Validation

`cargo test --locked --lib saved_data`: 9 tests, including **60 scenarios / 195 steps** and **646
payload mutants** against the executed original, zero disagreements. A one-off check fed six
library-written files to the original, which loaded them with equal payloads. The oracle also
found four fidelity bugs in the already merged `loot::sequences` (numeric `salt`, id
canonicalization, list-of-number sources, `reset`/`clear` marking dirty); fixed here with the
loot tests unchanged and green. Not run: a protocol client, Main flow, deployment. No Railway
change, no deployment claim.

## Blockers / requested contracts

For the integration coordinator: construct one `DimensionDataStorage` per level over its `data`
folder, call `compute_if_absent(&random_sequences_factory(seed), "random_sequences")` and
`compute_if_absent(&MapIndex::factory(), "idcounts")` where Java does, and `save()` in the vanilla
save sequence (level data last). The `data` folder must exist (the library does not create it).

## Next action / handoff

Owner: this instance for further `world_storage.4` owners if no one else claims them (scoreboard
needs WP-E4, maps need the map item port). Stage 1 (`PrimaryLevelData` field owners, touching
`src/level.rs`) and stage 4 (save ordering) stay unclaimed on #41.
