# level-data-20261101-e31d — `DragonFight` and `ScheduledEvents` members of `level.dat` (WP-G2 stage 1)

Instance: level-data-20261101-e31d (same session as loot-/components-/saved-data-/scoreboard-/datapack-/advancements-/bossbar-20261101-e31d);
team: lane G world/persistence (no team issue). Claim:
[issue #41 comment](https://github.com/Ferigone/Cinderstone/issues/41#issuecomment-6676208854) (next-from-this-instance note;
`ScheduledEvents` is also what WP-E7 [#28](https://github.com/Ferigone/Cinderstone/issues/28) needs for `/schedule`).
State: ready_for_integration once its pull request is green (library stage; no Main callsite)
Base: master `812d6fe`; branch `ccr-e31d38f2-ovzstg`.
Capacity: one Claude Code session, no subordinate agents. Nothing continues in the background after this turn.

## Owned files

New and exclusive: `src/level_data/**`, `fixtures/level-data-vectors.json`, `tools/level_data_generate.py`,
`docs/reference/level-data.md`, this file. Shared edits: one `pub mod level_data;` line in `src/lib.rs`
(standing export exception); the `dragon-fight` and `timer-queue` modes of `tools/loot_oracle.java` (this
instance's harness); `tools/saved_data_owners_generate.py` (SNBT string quoting only; its fixture is
unchanged). **Not touched:** `src/level.rs`, `world_rules.rs`, Main, manifests, checklist, README, roadmap,
journal and deployment files.

## Delivery IDs and bounded behavior

`world_storage.4` — **partial** (stage 1, the `DragonFight` and `ScheduledEvents` members only). Preserves
`scope_version: 1`, 100 items, **56/100 code, 51/100 Main** (unchanged; no point is added). Implemented:
`EndDragonFight.Data` parse/encode, the default gateway order from the world seed, and the `TimerQueue`
(schedule, remove, ids, store, load, the due-event step with the original's late-firing cell quirk). Not
implemented: running callbacks, `/schedule`, the dragon fight itself, the other `PrimaryLevelData` members,
Main wiring. Details: [level-data.md](../../reference/level-data.md).

## Source evidence

Pinned server and official mappings; the **executed** unchanged original classes through the local oracle
(a stub server records the function / tag a fired event asks for), plus direct inspection of the decompiled
classes (output not committed). No REA tool execution is claimed.

## Validation

`cargo test --locked --lib level_data::` — 1,640 dragon-fight payloads, 309 gateway seeds and 137 timer-queue
scenarios (2,887 steps, 394 fired callbacks) agree with the executed original with zero disagreements (547
payloads and 84 scenarios committed; the rest regenerable with `--full-out`). Eight deliberate mutations of
the port were each caught. Not run: a protocol client, Main flow, deployment. No Railway change, no deployment
claim.

## Blockers / requested contracts

For the integration coordinator (and WP-E7 for `/schedule`): read `DragonFight` with
`DragonFightData::from_nbt` and `ScheduledEvents` with `TimerQueue::load(level.get("ScheduledEvents"))`,
write `to_nbt()` / `store()` back in the `level.dat` save; run `pop_due(game_time)` in a loop each server
tick and run each fired callback (a function or a function tag) before asking for the next. The dragon
fight's gateway list for a world without saved data is `default_gateways(seed)`.

## Next action / handoff

Owner: this instance for further `level.dat` members only if claimed again (`WanderingTrader*`, `Version`,
difficulty/`GameType` fields); `/schedule` (WP-E7) and the dragon fight (WP-M5) are unclaimed by this instance.
