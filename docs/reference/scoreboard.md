# Scoreboard — objectives, scores, teams, packets and `scoreboard.dat`

Target: Minecraft Java **1.21**, protocol **767**. Primary evidence is the unchanged original
`Scoreboard`, `ServerScoreboard`, `ScoreboardSaveData`, `NumberFormatTypes` and the five
scoreboard packets, run through the local oracle (`tools/loot_oracle.java scoreboard` and
`scoreboard-load-mutants`). Pinned artifacts: bundler SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`,
inner server jar SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`
([`fixtures/source-lock.json`](../../fixtures/source-lock.json)). Java artifacts and decompiled
text stay outside Git.

Scope: `src/scoreboard/**`, work package WP-E4 stages 1–3 (delivery `commands_data.3`,
**partial**). It claims **no** checklist point and **no** Main integration; nothing outside the
module calls it. Criterion updates from statistics/health/kills (stage 4), name-tag/collision/chat
effects (stage 5) and `/scoreboard`, `/team`, `/trigger` (stage 6) are not implemented.

## What is implemented

| Java | Rust | Notes |
| --- | --- | --- |
| `Scoreboard`, `ServerScoreboard` | `scoreboard::Scoreboard` (`new` / `new_server`) | one struct plays both: with the server layer on it queues broadcast packets, tracks objectives and marks itself dirty |
| `Objective`, `Score`, `ScoreAccess`, `PlayerScoreEntry` | `Objective`, `Score`, `ScoreAccess` | read-only criteria, the `created` flag, locked scores, display/number-format overrides, `displayAutoUpdate` |
| `PlayerTeam`, `Team.Visibility`, `Team.CollisionRule` | `PlayerTeam`, `Visibility`, `CollisionRule` | members iterate in `HashSet` order |
| `DisplaySlot` (19 slots) | `DisplaySlot` | incl. the 16 team-colour sidebar slots |
| `ObjectiveCriteria`, `RenderType` | `ObjectiveCriteria`, `RenderType` | 43 built-in criteria and stat criteria (`minecraft.mined:minecraft.stone`), validated against the registries |
| `ChatFormatting` | `ChatFormatting` | ordinals (what `writeEnum` sends), `getByName`, ids |
| `NumberFormat` (blank, fixed, styled) | `NumberFormat` | codec both ways (JSON/NBT), `format(int)`, network form |
| `ClientboundSetObjectivePacket`, `SetScore`, `ResetScore`, `SetDisplayObjective`, `SetPlayerTeam` | `ScoreboardPacket` | body and framed (`id` + body) encoders |
| `Object2ObjectOpenHashMap` | `scoreboard::fastutil::OpenHashMap` | probing, growth, shrink, back-shift deletion and iteration order |
| `ScoreboardSaveData` | `scoreboard::ScoreboardData` (`SavedData`), `save_scoreboard`, `load_scoreboard` | on the `saved_data` framework, name `scoreboard` |

Behaviours the oracle showed (each a real mismatch before it was fixed):

- **Iteration orders are observable and differ per call site.** Objectives, teams and score
  holders live in `Object2ObjectOpenHashMap`s (`16, 0.5` for objectives and holders). `forEach`,
  `values()` and iterators visit the table from its last slot to its first; `getTrackedPlayers()`
  goes through `keySet().stream()`, which goes from the first slot to the last. A `HashSet`
  (team members) keeps its table size when members leave.
- **One holder's scores are keyed by object identity** (`Reference2ObjectOpenHashMap`), so their
  order in `scoreboard.dat` differs between runs of the original itself. The port keeps creation
  order and the tests compare the `PlayerScores` list as a set.
- **`Component.Serializer.toJson` text.** `display`, `DisplayName`, `MemberNamePrefix/Suffix`
  hold Gson text in the original's member order (`text`, `extra`, `hoverEvent`, `insertion`,
  `font`, `obfuscated`, `clickEvent`, `italic`, `underlined`, `strikethrough`, `color`, `bold`;
  `hoverEvent` writes `contents` before `action`). `Component::to_json_string` produces it.
- **Loading is lenient and not transactional.** `Component.Serializer.fromJson(String)` uses Gson's
  lenient reader (unquoted and single-quoted strings, `=` and `;`); an empty string throws. An
  exception part-way through `load` leaves the objects and teams loaded before it in the
  scoreboard, and the next `computeIfAbsent` binds a fresh save-data object to that same
  scoreboard. `load_scoreboard` returns `Err` for the whole load instead of exposing the partial
  state; a caller that wants Java's behaviour keeps the scoreboard it already has.
- **A team colour can be `null`.** `ChatFormatting.getByName` of an unknown name sets it; the
  `SetPlayerTeam` encoder then throws, so the broadcast never leaves. `TeamParameters.color` is an
  `Option` and encoding it fails.
- **`ServerScoreboard.setDisplayObjective` is virtual** and runs mid-way through
  `removeObjective`; the port runs the same logic at the same point, so the packets and the tracked
  set evolve as in Java. `addPlayerToTeam` removes the player from the old team first, sending
  that `REMOVE` before the `ADD`.
- **Loading marks the data dirty** (the server scoreboard's setters call `setDirty`), so a loaded
  file is rewritten at the next save.

## Evidence

`tools/scoreboard_generate.py` records the original's result for every step.

- `fixtures/scoreboard-vectors.json`: **22 scenarios, 375 steps**: criteria (valid, stat, unknown,
  invalid characters), objective changes with and without tracking, scores (read-only, forced,
  locked, display and number-format overrides, holder display names with auto-update), 46 holders
  (growth and deletion shifting of the open-hash maps), 30 objectives, display slots and
  tracking, fixed/styled/blank number formats, teams (colours, prefixes, all options, 19 members,
  25 teams), start/stop tracking packets, and reloads of what the original saved. Compared per
  step: error or not, return value, the frames of every `broadcastAll`, the full state dump
  (objective, score, team and slot order included) and the saved NBT.
- `fixtures/scoreboard-load-mutants.json`: a sample of **201** of **4003** one-step mutants (member
  removed, node replaced by 19 wrong-typed tags) of the saved payloads up to 3000 hex digits.
  3545 loaded and 458 were rejected by the original; **the full set agrees with zero
  disagreements** on accept/reject, the resulting state dump and the re-saved NBT (compound order
  aside, the `PlayerScores` list as a set).
- `fixtures/scoreboard-facts-1.21.json`: the stat-type and custom-stat registries behind stat
  criteria, exported from the same oracle (`scoreboard-facts`).

`cargo test --locked --lib scoreboard` runs these plus focused tests (criteria names, the
open-hash map, packet ids, a round trip through the `saved_data` storage) in about a tenth of a
second. The full mutant set is run with
`SCOREBOARD_MUTANTS_FILE=<json> cargo test --lib scoreboard::tests::explore_load_mutants -- --ignored`
after `python3 tools/scoreboard_generate.py --sample 1`.

## Not implemented / limits

- **Stages 4–6** of WP-E4 (criterion hooks, team effects in tracking/chat/damage, commands).
- **`join-time` delivery.** The module returns the packets (`start_tracking_packets`, the
  `Outbound` queue); sending them to a player is the connection owner's job. Packets to online
  players (`EachPlayer`) are not observed by the oracle (its player list is empty); their content
  is the checked `getStartTrackingPackets` / `getStopTrackingPackets`.
- **Formatted names** (`getFormattedDisplayName`, `PlayerTeam.getFormattedName`,
  `ScoreHolder.getFeedbackDisplayName`) need `Component.append/withStyle`; not ported.
- **Gson leniency** is implemented for the forms listed above, not every corner of
  `JsonReader` (`)]}'` prefix aside, malformed numbers, `\u` edge cases).
- **Data fixers.** Only data already at version 3953 (or newer) is accepted for `scoreboard`; older
  files report `DataFixerRequired` because several fixers rewrite this type.
- **Identity-ordered per-holder scores**, see above; `Scoreboard.entityRemoved` (needs entities).

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git      # JDK 21 required
python3 tools/scoreboard_generate.py --oracle-dir /path/outside/git
cargo test --locked --lib scoreboard
```
