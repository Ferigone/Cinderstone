# advancements-20261101-e31d — advancement requirements, progress and progress file (WP-E5 core)

Instance: advancements-20261101-e31d (same session as loot-/components-/saved-data-/scoreboard-/datapack-20261101-e31d);
team: lane E data-driven (no team issue). Claim:
[issue #26 comment](https://github.com/Ferigone/Cinderstone/issues/26#issuecomment-6675046688).
State: ready_for_integration once its pull request is green (library stage; no Main callsite)
Base: master `eaf93c3`; branch `ccr-e31d38f2-ovzstg`.
Capacity: one Claude Code session, no subordinate agents. Nothing continues in the background after this turn.

## Owned files

Stage 1 additions: `src/advancements/definition.rs`, `src/advancements/definition_tests.rs`,
`tools/advancement_parse_generate.py`, `fixtures/advancements-vanilla-1.21.json`,
`fixtures/advancement-parse-vectors.json`, the `advancement-parse` mode of `tools/loot_oracle.java`;
read-only reuse of `crate::loot::parse` (four of its predicate readers made `pub`).

New and exclusive: `src/advancements/**`, `fixtures/advancement-progress-vectors.json`,
`tools/advancement_generate.py`, `docs/reference/advancements.md`, this file. Shared edits: one
`pub mod advancements;` line in `src/lib.rs` (standing export exception); the `advancement-progress`
mode of `tools/loot_oracle.java` (this instance's harness); in code this instance already owns:
`src/components/lenient.rs` (a strict `parse_strict_prefix` and three corrections to the lenient
reader, listed in `docs/reference/advancements.md`). **Not touched:** `main_configuration.rs`, Main,
manifests, checklist, README, roadmap, journal and deployment files.

## Delivery IDs and bounded behavior

Work package WP-E5 on #26, **core only** (requirements, progress, progress file). No checklist point is
added or claimed; `scope_version: 1`, 100 items, **56/100 code, 51/100 Main** unchanged. Implemented:
`AdvancementRequirements`, `CriterionProgress`/`AdvancementProgress` and their codec, the
`advancements/<uuid>.json` document (strict Gson read, `SMART` date resolution, canonical UTC write,
duplicate ids, `DataVersion` rules) for files at DataVersion 3097 or later; stage 1 definitions (`definition::parse`: `Advancement.CODEC`
with all 56 trigger-instance codecs; 1,399 vanilla files and 44,195 mutants agree with the original).
Not implemented: trigger evaluation, tree/layout, `ServerAdvancementManager` loading, `PlayerAdvancements` runtime, the update
packet, data fixers for older files, file text rendering. Details:
[advancements.md](../../reference/advancements.md).

## Source evidence

Pinned server and official mappings; the **executed** unchanged original progress codec
(`DataFixTypes.ADVANCEMENTS.wrapCodec(PlayerAdvancements.Data.CODEC, ..)`, `AdvancementProgress.update`)
plus direct inspection of the decompiled classes and fixers (output not committed). Gson's reader
behaviour was probed directly against Gson 2.10.1 (the version the server bundles). No REA tool
execution is claimed.

## Validation

`cargo test --locked --lib advancements::` — 19,074 generated progress files agree with the executed
original (zero disagreements; 1,286 committed, the rest regenerable with `--full-out`). The lenient
reader change was rechecked against the full component (231,616 NBT and 142,489 JSON mutants),
scoreboard (load mutants) and tag-loader (6,000 scenarios) nets. Not run: a protocol client, Main
flow, deployment. No Railway change, no deployment claim.

## Blockers / requested contracts

None for the library. A caller (the future `PlayerAdvancements`) supplies each advancement's
requirements, reads the file as text it has already decoded as UTF-8, and treats
`ProgressError::DataFixerRequired` like the original's parse failure unless the fixers are ported.

## Next action / handoff

Owner: this instance for the next WP-E5 stages (the tree and `ServerAdvancementManager` loading, the
`Update Advancements` packet) only if claimed again; trigger evaluation and the runtime are unclaimed.
