# Ordinary falling: bounded runtime record

Actual sand, red sand and gravel falls passed, and a separate previously-unrun
water path passed using a real bucket and stone basin. The grouped record stays
**partial / `ok: false`** because a private harness `deepcopy` error interrupted
the full removal/re-entry observation. Its successful prefix is preserved;
neither that error nor the separate water result turns it into a complete PASS.
See the [grouped record](../validation/ordinary-falling-local-keys.json).

| Identity | Exact value |
| --- | --- |
| Minecraft / protocol | Java 1.21 / 767 |
| Frozen source / tree | `a2b020f934cfbe00abca260f1a33593c31873518` / `64f645097fb0e287e33e65cb6e3cabe4c7ae227e` |
| Actual executable SHA-256 | `39adbead1c345ee5b9870a7adbef5c1cc58c47f60bdda9b1e4421edb66796395` |
| Grouped harness / failed raw report SHA-256 | `dc2806f9c89b2967d5a2c8211f72107fbc2205945e9cb102d9ff881571c36b39` / `815ae50f353ccb071f5678da28f1247716447c35979eb0a702960ac0ca7a44d2` |
| Water-only harness / actual raw report SHA-256 | `c29ea3fe3cf47a7ab5c95830bafbd8de08ae3dba9cbd522e12df02a3d343d301` / `e32332d2b5fe3383898260738dacb04c2f3fec3e27d29a0ae06730579c8ccade` |
| Published grouped JSON SHA-256 | `7dcfcd9770adf4a22afc52a405d6542adba51451664ef07a5265cc531f83a5f7` |

The grouped attempt ran `2026-11-02T02:49:35.222712+00:00` to
`02:50:14.446747+00:00`; the authorized water-only path ran
`02:53:30.697004+00:00` to `02:54:10.672642+00:00`. Its setup did not replay the
ordinary/removal scene. Every isolated server completed normal SIGINT shutdown
with exit 0; the grouped harness itself failed and remains recorded that way.
No saved scene, positive checkpoint, history, RNG or integrity value was edited.

All processes explicitly selected offline loopback plus
`--loopback-service-keys`. Each actual pre-listening event selected
`source: loopback-file`, `network_refresh: false`; the parsed empty key lists
gave zero trust roots. No official-source event appeared. The immutable local
source and absence of HTTP fallback are documented separately in
[the local-key source ledger](falling-midflight-local-keys.md). This does not
claim signed-chat/certificate validation or a network-interface capture audit.

## Ordinary falling and water

The pinned placement catalogue identifies exactly sand, red sand and gravel as
the ordinary `ColoredFallingBlock` family (`dhg`).
`falling_block::block::scheduled_tick` retains inherited delay-2 scheduling and
isFree queries, constructs the genuine shared FallingBlock, and invokes the
literal inherited falling hook. It does not apply Anvil's hurt `(2,40)` setter.
The ordinary onLand/onBroken callbacks are the actual inherited return-only
bodies and do not emit Anvil level events. `hurtEntities=false` is source
ownership evidence here, not runtime damage instrumentation.

| Actual path | Observation |
| --- | --- |
| Sand item 57 / state 112 | Type 40/entity 2 spawn ordinal 115, start-position metadata 116, real landing block 120, same-actor removal 123 |
| Red sand item 60 / state 117 | Entity 3 spawn 129, metadata 130, landing 134, removal 137 |
| Gravel item 61 / state 118 | Entity 4 spawn 144, metadata 145, landing 149, removal 152 |
| Ordinary callbacks | No level 1029, 1030 or 1031 in these actual fall intervals |

Each real block first remained stable on a real stone support. Destroying that
support produced an actual FallingBlock and a one-block landing, retaining the
carried state and metadata start position. Nothing substituted a dropped item
for a falling actor.

For water, normal placement made a stone floor and walls; actual BucketItem
use packet 57 created source-water state 80 at `(10,65,8)`. Creative mode retained
the genuine water-bucket stack. Further actual supports held sand at
`(10,68,8)` before support destruction. Type 40/entity 2 spawned at ordinal 148,
metadata followed at 149, actual sand 112 replaced the water cell at ordinal
153, and the same actor was removed at 156. No Anvil event appeared. Native save
contained one sand block at `(10,65,8)`, no Falling owner and no Item owner.

This reaches the real `main_falling_block_callbacks` water profile and existing
`MainFluidHost`/FluidSimulation owner, with source callbacks and actual scheduler
ownership. The stone floor/walls keep neighboring classes within its declared
base Block/Air/LiquidBlock boundary. Lava, fire, waterlogged or other active
neighbor callbacks are not silently accepted. The result does not establish
concrete-powder hardening, arbitrary water networks or other falling subclasses.

## Source-removal save prefix and explicit harness failure

The actual InventoryMenu retained cobblestone 2 in its 2x2 grid. A real crafting
table separately owned logs 3 in its input grid and dirt 4 on its cursor, with a
derived four-plank preview left untaken. Canonical persistent slot 12 held stone
7. Genuine disconnect produced this actual first checkpoint:

| Owner | Stone | Cobblestone | Logs | Dirt | Planks |
| --- | --- | --- | --- | --- | --- |
| Saved player | 7 | 0 | 0 | 0 | 0 |
| Real world Items | 0 | 2 | 3 | 4 | 0 |

Cursor was empty and saved `leave_game` was 1. This witnesses the new
`SourcePlayerInventory` projection: PlayerList.remove's award/save precedes
Player.remove, while menu-owned cursor/grid stacks are dropped by their actual
removal owners rather than also serialized as player inventory. The detached
`snapshot_for_player_save` clears menu slots 0..4 and carried state; native
resume/autosave uses its separate view.

On genuine re-entry the raw initial full window-0 packet was already captured
before the private observer attempted `copy.deepcopy` of the imported immutable
Stack tuple subtype. That Python reconstruction raised `TypeError`; the live
AwardStats request and subsequent assertions were not executed. Read-only
decoding of the existing packet shows 46 slots with only slot 12/stone 7, empty
grid and cursor. Actual final save after cleanup still conserved the table above
and recorded `leave_game: 2`. Those are retained packet/save facts, not a
completed live-statistics or reconfiguration check. No complete removal flow
is claimed, and the original raw report remains byte-for-byte failed.

The original protocol bootstrap separately confirmed serverbound client-command
9, click 14, close 15, block action 36, creative slot 50 and use-item 57, plus
clientbound AwardStats 4. Actual custom `leave_game` is fixture type 8/value 0.
This was direct pinned JDK21 bootstrap extraction, not REA MCP execution. The
source identities, fixture/capture digests and exact packet facts are in JSON.

These narrow witnesses do not certify all menu owners, reconfiguration,
survival, damage victims, RNG streams, Java saves, mixed cohorts, public or
graphical clients, performance, or a full fixed-checklist row. Checklist scope
and percentages are unchanged.
