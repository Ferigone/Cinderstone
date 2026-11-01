# Current native port status

Cinderstone targets Minecraft Java 1.21, protocol 767 and DataVersion 3953. It is
an experimental native Rust implementation. Full vanilla compatibility is not
complete. The current supported player profile is an alive, unmounted creative
player in the native flat world, without active status effects.

## Furnace Main integration — bounded runtime flow passed

The default cooking catalog was published in `872d638`; native furnace engines,
menus and persistent block-entity foundations were published in `2f5f716`.
The current Main integration connects:

- Furnace, smoker and blast-furnace processing with captured vanilla recipe and
  fuel data, canonical three-slot contents, counters and lit-state changes.
- Authoritative menus using the shared player inventory and cursor, with window
  contents and cooking/burn data synchronization.
- Owned block entities and native save/restart records. New native records retain
  registration order; legacy records restore their recorded order.
- Furnace experience rewards through real orbs, shared entity ticking, pickup
  and tracking, using the supported native player host.

The compiled batch passed a bounded local protocol-client flow on 1 November
2026 at 04:27:37 Europe/Warsaw. The
[machine-readable flow report](validation/furnace-preview-flow.json) identifies
binary SHA-256
`46340001f58f69500955c0f8e278b4670c707c18279a0cab1768e5c8044489b4`.
The observed scope was:

- Native PLAY join using Minecraft Java 1.21 protocol 767.
- Furnace processing at 200 native cooking ticks and smoker/blast-furnace
  processing at 100 ticks, with actual fuel, counters and menu synchronization.
- Normal and shift result transfers, lit-state changes and live lighting.
- Real XP-orb spawning/pickup and the player's experience packet.
- Shared chest-inventory behavior alongside furnace interactions.
- SIGTERM save and cold restart preserving partial burn/cooking state, output,
  used recipes, player XP and the correct chunk block-entity types.

The saved partial furnace had cooking progress 13/200 and 1,110 burn ticks
remaining. The report records two XP points before restart and four after the
continued flow. These observations apply to the recorded binary and path. The
probe was a protocol client, not a graphical vanilla-client session; Railway
deployment of this batch is not established by this result. Source behavior and
evidence boundaries are described in the
[furnace reference ledger](reference/furnaces.md).

Remaining furnace limits are explicit:

- Block destruction, including nonempty contents, item drops and stored recipe
  rewards, is unsupported.
- Comparator receivers and Mending experience repairs are unsupported.
- Transactions combining furnace output experience and item drops are rejected
  before commit.
- The captured default catalog does not establish support for custom cooking
  datapacks or dynamic recipe reload.

These are bounded implementation deliveries, not a claim that every furnace or
experience interaction is complete.

## Normal-rate server clock — exercised in the bounded flow

The current batch also replaces the unbounded timer-backlog behavior with the
original ordinary 20 TPS deadline and overload policy. The first tick is ready
immediately after initialization; the period is 50,000,000 nanoseconds. Modest
backlog is caught up. A lag greater than two seconds advances the deadline past
whole missed periods only when `nextTickTime - lastOverloadWarning` is at least
15 seconds. This is not an unconditional missed-tick skip policy.

The Linux implementation uses the absolute `CLOCK_MONOTONIC` origin, matching the
installed JDK21 reference runtime and the initially-zero warning field. The
non-Unix fallback uses a process elapsed-time epoch and does not claim the same
initial-warning behavior. Tick sprinting, freeze control and complete Java task
queue/draining semantics are outside this bounded clock change.

The successful furnace flow ran against the binary containing this clock,
including steady normal-rate world-time observations. It does not establish
complete scheduler/task-queue equivalence or fixed wall-clock cooking duration
under every load condition.

## Existing integrated scope and historical verification

The main runtime also includes native player body phases for the supported
creative profile; water/lava travel; food, milk and honey use; Chest/Barrel
inventories and persistence; 2×2 and 3×3 crafting; selected redstone components;
a shared BLOCK scheduler; and selected current-version Java `player.dat`/item
NBT codecs.

The earlier [offline flow report](validation/offline-preview-flow.json) records
public-bind startup, status/pong, an unauthenticated PLAY login, name-derived UUID
and graceful shutdown for its recorded binary. A live offline PLAY join was also
observed in Railway deployment logs.

The earlier [bounded gameplay report](validation/native-preview-flow.json)
records join/movement, a water-bucket roundtrip, chest transfer, 3×3 furnace-item
crafting, milk use, SIGTERM save and cold restart for its exact recorded binary.
Crafting a furnace item in that report did **not** validate furnace cooking.
Those historical results do not validate the current furnace integration binary.
Comprehensive conformance and equal-workload performance validation remain deferred.

## Publication and deployment

Normal source files are published in the public
[Ferigone/Cinderstone repository](https://github.com/Ferigone/Cinderstone) on
`master`, with an English README and roadmap. Source archives are not committed.
Railway follows that unpinned branch automatically, using one replica and a
persistent `/data` volume.

Preview address: **`hopper.proxy.rlwy.net:24418`**, for Minecraft Java **1.21**.
The preview accepts offline accounts with `MC_ONLINE_MODE=false`; nicknames are
not authenticated, and name-derived UUIDs do not automatically reuse existing
online-account data. The image default remains `MC_ONLINE_MODE=true`.

The [deployment observation](validation/railway-preview.json) describes its exact
recorded release and live join. The new local furnace flow passed, but that older
deployment observation does not establish Railway deployment of this batch.
Direct external status/pong
probing from the development environment previously failed DNS resolution;
live joins were observed through deployment logs. A successful join does not
establish complete gameplay compatibility.

## Remaining scope and checklist

Unsupported areas still include full survival/damage/death/respawn, active
effects, vehicles, elytra, mobs/AI, complete generation, remaining workstations and
block entities, complete redstone, TNT/explosions, commands/datapacks and full
Java-world interoperability. Unsupported owners and callbacks remain explicit
errors.

The fixed 100-item planning checklist remains **55/100 code deliveries** and
**50/100 main-server integrations** at `scope_version: 1`. These are checklist
indicators, not measures of functional compatibility, engineering effort or
remaining time. The broader inventory and simulation deliveries still require
all remaining menus and complete block-entity ticking; this bounded furnace
family does not complete those checklist items. The denominator and definitions
are unchanged.

Porting and integration remain the priority. See the [README](../README.md) and
[roadmap](ROADMAP.md) for the English project scope and remaining work.
