# Current native port status

Cinderstone targets Minecraft Java 1.21, protocol 767 and DataVersion 3953. It is
an experimental native Rust implementation. Full vanilla compatibility is not
complete. The current supported player profile is an alive, unmounted creative
player in the native flat world, without active status effects.

## Furnace menu lifetime after removal — integration in progress

The current porting stage targets creative destruction while one or more players
still have the furnace menu open. The removed block entity's actual container
must remain owned by those existing menus until their later validity/close phase.
Its dropped contents must not be available a second time, and a replacement
furnace at the same position must not become the old menu's container. Cursor
return, explicit close, disconnect and shutdown cleanup must release each menu
owner exactly once.

This stage is **IN PROGRESS**. There is no recorded compiler, runtime or hosted
verification result for this new lifetime path yet. The closed-menu processing
and removal results below remain evidence for their exact recorded versions.

## Earlier closed-menu furnace block removal — bounded runtime flow passed

The current implementation connects creative destruction of owned native
furnaces, smokers and blast furnaces to their real inventory contents and stored
recipe experience. It stages container stack splits and item-entity creation
before recipe XP, using the actual random and shared entity owners, then removes
the block entity and its ticker with the world change. A privately drained
staging image is never published as a live furnace state. Creative destruction
does not award a loot item for the furnace block itself.

The [focused local removal flow](validation/furnace-removal-flow.json) **passed**
on 1 November 2026 at 11:47:29 Europe/Warsaw, for source commit
`6eaf7487ffcca7856f233280a1e3e1b7d1ec192d` and debug binary SHA-256
`46a3514fd9906ed975e44c1e6577dc2fa021fcead1ca659a456e0d0053c50b8f`.
The recorded scope includes:

- Closed-menu destruction of all three furnace types, actual item entities and
  recipe-experience orbs emitted at the block center.
- Normal menu closure returning the cursor stack, plus the creative sword guard
  acknowledging the request without removing the furnace or spawning items/XP.
- Lit-state changes retaining the furnace contents and owner.
- Item conservation across player inventory and native entities before and after
  cold restart: 51 dirt, six coal, one diamond, one cooked beef, one netherite
  scrap and five quartz. The remaining item-entity IDs were preserved.
- Experience conservation across the same restart: one player XP point plus two
  points in saved orbs, totaling three both before and after restart.
- Removed blocks and block entities absent from the checkpoint, with clean
  SIGTERM shutdown after draining actual pending world/scheduler work before
  menu closure and checkpointing.

This is a local protocol-client result. The optimized hosted release and native
offline startup are verified separately below. The result does not establish a graphical vanilla-client
session, public-network gameplay, complete scheduler equivalence or a throughput
advantage. The earlier processing observations below retain their original
binary and scope.

This path is restricted to the supported native creative-player profile and
furnaces with no active menu viewers. Close every menu viewing the furnace before
breaking it. Removal while a menu is open, survival breaking and other removal
contexts remain unsupported. Comparator receivers, Mending repairs, custom
cooking datapacks and menu output-XP actions that also drop items remain open.

## Earlier furnace processing integration — bounded runtime flow passed

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
probe was a protocol client, not a graphical vanilla-client session. Hosted
startup is recorded separately below. Source behavior and
evidence boundaries are described in the
[furnace reference ledger](reference/furnaces.md).

Remaining furnace limits are explicit:

- The new bounded creative removal path passed its recorded local flow above.
  Open-menu, survival and other removal contexts remain unsupported.
- Comparator receivers and Mending experience repairs are unsupported.
- Menu actions combining furnace output experience and item drops are rejected
  before commit. The new block-removal transaction is a separate path.
- The captured default catalog does not establish support for custom cooking
  datapacks or dynamic recipe reload.

These are bounded implementation deliveries, not a claim that every furnace or
experience interaction is complete.

## Normal-rate server clock — exercised in the bounded flow

The earlier processing batch also replaced the unbounded timer-backlog behavior
with the original ordinary 20 TPS deadline and overload policy. The first tick is ready
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

The [deployment observation](validation/railway-preview.json) records the furnace
removal batch: optimized release build, successful automatic deployment,
one running replica and native offline listening on `0.0.0.0:25565` at
10:49:40 UTC on 1 November 2026. The persistent volume is retained. The current
release has no observed public PLAY join in that snapshot; the earlier offline
join remains explicitly historical. A fresh external status/pong probe failed
DNS resolution from the development environment. Local gameplay, hosted startup
and public gameplay are separate evidence scopes.

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
