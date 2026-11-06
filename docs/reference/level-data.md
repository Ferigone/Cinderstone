# `level.dat` members — the Ender Dragon fight data and the scheduled events

Target: Minecraft Java **1.21**, protocol **767**, DataVersion **3953**. Primary evidence is the
unchanged original `EndDragonFight.Data.CODEC` (with `Util.shuffle` and `RandomSource.create` for the
gateway order) and `TimerQueue<MinecraftServer>` with `TimerCallbacks.SERVER_CALLBACKS` and the real
`FunctionCallback` / `FunctionTagCallback`, run through the local oracle (`tools/loot_oracle.java
dragon-fight` and `timer-queue`; the second uses a stub server whose function manager records which
function or tag a fired event asked for). Pinned artifacts: bundler SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner server jar SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437` ([`fixtures/source-lock.json`](../../fixtures/source-lock.json)).
Java artifacts and decompiled text stay outside Git.

Scope: `src/level_data/**`, work package WP-G2 **stage 1** (the `DragonFight` and `ScheduledEvents` members
of `level.dat`, delivery `world_storage.4`, **partial**; the scheduled-event queue is also what WP-E7's
`/schedule` needs). It claims **no** checklist point and **no** Main integration: nothing in `src/` outside
the module calls it, and `src/level.rs` is untouched (the boss bars are in [bossbar.md](bossbar.md)).

## What is implemented

| Java | Rust |
| --- | --- |
| `EndDragonFight.Data.CODEC` over `NbtOps` (parse and encode), `Data.DEFAULT` | `level_data::DragonFightData` (`from_nbt`, `to_nbt`, `DEFAULT`) |
| the gateway list `EndDragonFight`'s constructor draws: `0..20` shuffled with `Util.shuffle(.., RandomSource.create(worldSeed))` | `level_data::default_gateways(world_seed)` |
| `TimerQueue<MinecraftServer>`: constructor from `ScheduledEvents`, `schedule`, `remove`, `getEventsIds`, `store`, `tick` | `level_data::TimerQueue` (`load`, `schedule`, `remove`, `events_ids`, `store`, `pop_due`) |
| `TimerCallbacks.SERVER_CALLBACKS`: `minecraft:function`, `minecraft:function_tag` | `level_data::TimerCallback::{Function, FunctionTag}` |

The caller owns the `level.dat` compound: it passes the `DragonFight` / `ScheduledEvents` member in and writes
the value back, and it runs the callbacks (`pop_due` hands back the next due event; call it again after
running one, as the original's loop does, because a callback may schedule more).

Behaviours the oracle showed (each pinned by scenarios or mutants):

- **Dragon fight codec.** `NeedsStateScanning` (default true), `DragonKilled` and `PreviouslyKilled`
  (default false) are `fieldOf(..).orElse(default)`: a missing member **or any value the boolean codec
  rejects** is the default. Through `NbtOps` a boolean is any numeric tag narrowed to a byte (`256s` and
  `0.4f` are false, `-1b` true); a string, list or compound is "not a boolean". `IsRespawning`, `Dragon`,
  `ExitPortalLocation` and `Gateways` are `lenientOptionalFieldOf`: an unreadable value is simply absent.
  `Dragon` and `ExitPortalLocation` are an `INT_STREAM` of exactly 4 / 3 numbers: an int array, but also
  a byte or long array or a list of any numeric tags, whose elements are narrowed with `Number.intValue`
  (a long wraps, a float or double is **cast**, truncating and saturating — not floored), and a list of
  compounds is unwrapped from `{"": x}`.
- **Encoding.** `IsRespawning` is written only when true, the optional members only when present, and a
  list of ints is written as an **int array** (`NbtOps`' list collector), an empty one as an empty list; the
  compound iterates in `HashMap` order of its keys.
- **Gateways.** The fight draws its gateway order from the world seed with a legacy `Random`: for `i` from 20
  down to 2, swap element `i - 1` with element `nextInt(i)`.
- **Scheduled events.** The queue is ordered by trigger time and then by the order events were scheduled
  in (a counter that restarts at 0 on load, so a loaded list keeps its stored order for equal times).
  `schedule` ignores an event whose `(name, trigger time)` cell already exists — **the same name at
  another time is a new event** — and `remove(name)` drops every event of that name and returns the
  number of *cells* in the row.
- **A cell can outlive its event.** `tick` takes a due event off the queue and then removes the cell
  `(name, now)`: the current game time, not the event's trigger time. An event that fires late (after
  `now > trigger time`) therefore leaves its cell behind: its name stays in `getEventsIds()`, the same
  name and time can no longer be scheduled, and a later `remove(name)` counts it. Only an event ticked at
  exactly its trigger time cleans up after itself. (`getEventsIds` is in order of each name's first
  insertion; a name whose last cell is removed leaves and joins the end if it comes back.)
- **Loading** reads `ScheduledEvents` as `Dynamic.get(..).asStream()`: a list is read in order, anything else
  is an empty queue. An element that is not a compound is skipped (a list of compounds is first unwrapped
  from `{"": x}` wrappers); an event is skipped when its `Callback` has a `Type` that is not
  `function`/`function_tag` (with or without `minecraft:`; `Function`, `:function`, `x:function` are not) or a
  `Name` that `ResourceLocation.parse` rejects. `Name` defaults to the empty string and `TriggerTime`
  to 0; the time is read with `getLong` (a float is cast, a double **floored**, a string is 0).
- **Saved form.** `store()` is the list sorted by (trigger time, order scheduled), each event `{Name,
  TriggerTime, Callback: {Name, Type}}` with the callback id as `namespace:path` and the type as
  `minecraft:function` / `minecraft:function_tag`; the compounds iterate in `HashMap` order. An empty queue is
  an empty list of type "end".

## Evidence

`tools/level_data_generate.py` records the original's answers.

- `fixtures/level-data-vectors.json`: **1,640 `DragonFight` payloads** generated (12 bases — all members,
  none, every array and list type, numeric forms, wrappers, wrong types, extreme ints — and every one-step
  mutant; 1,368 accepted, 547 committed) compared on every field and the re-encoded NBT bytes;
  **309 world seeds** for the gateway order (including `0`, `-1`, the `int` and `long` extremes and 300
  random ones); **137 timer-queue scenarios** with 2,887 steps and 394 fired callbacks (84 committed): hand
  scenarios for duplicates, late firing and stale cells, removal, ordering at equal times, loading of
  good and broken callbacks, type/name spellings, time encodings, non-list and wrong-shaped lists, store
  and reload; and 80 random sequences of schedule / remove / tick / ids / store / reload over a small pool
  of names and times (including `Long.MIN_VALUE` and `Long.MAX_VALUE`).
- **All agree with zero disagreements.** Eight deliberate mutations of the port (cell removed at the trigger
  time, no duplicate-cell check, `remove` counting queued events, a double time truncated instead of
  floored, the store ordered by sequence only, `IsRespawning` always written, the gateway shuffle in the
  other direction, a float floored instead of cast) were each caught.

`cargo test --locked --lib level_data::` runs the fixture in under a second. The full set:
`LEVEL_DATA_FILE=<json> cargo test --lib level_data::tests::explore -- --ignored`.

## Not implemented (explicit)

- Running a callback (`FunctionCallback.handle` needs the function manager), the `/schedule` command and
  `tick` as a server step: `pop_due` only hands back the due event.
- The other members of `PrimaryLevelData` (`Version`, `WorldGenSettings`, difficulty, `GameType`,
  `WanderingTrader*`, …) and `src/level.rs` wiring.
- `EndDragonFight` itself (the fight, the respawn animation, the gateway placement): only its saved data
  and the default gateway order.

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git      # JDK 21 required
python3 tools/level_data_generate.py --oracle-dir /path/outside/git --sample 3 [--full-out FILE]
cargo test --locked --lib level_data::
```
