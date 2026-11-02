# Native falling Anvil and actual menu witness

One combined actual Main flow passed: supported Anvil placement, real support
removal, a one-block FallingBlock flight and landing, followed by an ordinary
unenchanted creative Anvil rename/take and native save/restart ownership flow.
The earlier placement blocker was not bypassed with an edited world or invented
settled input. Every block and stack in this run came from real protocol actions
in a fresh native world. This is a bounded production witness, not complete
FallingBlock, Anvil or Minecraft compatibility and not a checklist delivery.

## Exact source and executable

| Identity | Value |
| --- | --- |
| Minecraft / protocol | Java 1.21 / 767 |
| Composed source | `a8cc5a766a0ad7ba6217fa56d73ef0760e3f5b5c` |
| Source tree | `e2e398445e35b958a713c24588a5691a96d9e770` |
| Actual executable SHA-256 | `e2c7950bf85c556e0f0a23004561fe038e894a87fbac6346088c29897f3da10e` |
| Combined harness SHA-256 | `09fda88cafe209184d230ae1525bcae368aa3f67a30b4c0993ea8a822fa2d2fb` |
| Preserved original Anvil harness SHA-256 | `4573ada0115a1ed275dbe0f1649339c4fc640fdb3c52e14c5b923d023d2e4200` |
| Reused menu / transport helper SHA-256 | `f9b5656cfef8907b9f054f0abe96d942b1711839363d44319155949e370e31d7` / `14f81108916913a825a7001d1a07ac9afcb07c3b92e28b20bc9de90f9ba17d94` |
| Actual run UTC | `2026-11-02T01:30:09.137436+00:00` to `2026-11-02T01:30:57.720749+00:00` |
| Sanitized report SHA-256 | `b4ee224b8701672d4f45ac591647ad30c1537783c2b7521c402d977140567263` |

The [actual report](../validation/falling-anvil-flow.json) is byte-identical to
the sanitized runtime record and retains exact packet ordinals and fixture
identity. Required all-target compilation passed in 23.94 seconds; the full
executable build passed in 37.04 seconds before this run. All three isolated
server processes completed genuine SIGINT shutdown with exit 0.

The composed source includes the actual shared-menu integration and corrected
player-seed source alongside the new Falling owner. It introduces no substitute
terrain generator or fabricated native history. Compiler success is separate
from the concrete runtime observations below.

## Actual source owners and call phases

Changed Anvil placement and shape callbacks retain their ordered delay-2 BLOCK
schedule attempts. `main_block_ticks.rs` rechecks the current Block singleton
and dispatches the original scheduled kernel through
`tick_native_anvil_block_locked`. The real loaded scheduler and shared sub-tick
counter are used; represented placement obligations are not enqueued a second
time. See the [central source contract](falling-block-production-contract.md)
and [actual state predicates](falling-block-state-predicates.md).

`main_falling_block_spawn.rs` constructs one actual `FallingBlockState` with the
shared entity counter and process constructor RNG. It writes the original
fluid replacement at the origin with flags 3, performs actual UUID/world-add
admission and configures that same object's Anvil hurt fields. An unsuccessful
add retains the original unregistered object rather than manufacturing a
registered handle or replacement item.

`MainFallingBlockRuntime` retains that canonical Box and its separate sender.
The actor uses the existing shared entity index, level tick list and visibility
owners. `main_falling_block_dispatch.rs` obtains actual current chunk/query
capabilities and invokes the literal physics/tick host while retaining the same
object. Landing executes actual block/callback, lighting and source-history
operations, publishes the tracked block update, discards the real index/tracker
owner and then emits Anvil on-land event 1031. Reaping happens after callbacks.
The native checkpoint contains the actual Item/XP/Falling cohort and captured
order; this run observes an empty Falling cohort after successful landing.

Actual menu entry goes through `use_native_anvil`, retained `NativeAnvilRuntime`,
its staged context and `AnvilMenu`, with the canonical shared player inventory.
Anvil owns its two inputs and cursor; slot 2 remains a derived result. PLAY
rename `0x2a` decodes only the actual name string, with no invented window/state
fields. Click and close select the actual current menu. Real source phase
images, local listener events, inventory counter boundaries, menu packets,
level events and owned Item drops retain their indexed commit order.

The context requires the genuine native creative/body/default-feature and
empty-criteria/nonreentrant floor. Nonempty normal/stored enchantment maps are
an explicit unsupported context, including nested stacks. Creative take avoids
the original noncreative XP and player-wear RNG branches and emits 1030. It does
not run those branches with dummy values. Actual close and disconnected removal
return/drop only cursor and inputs, never preview. Ordinary autosave retains
menus; forced connected retirement is confined to shutdown. These are source
contracts, not internal callback or counter instrumentation by this run.

## Observed falling sequence

The client placed a real stone support at `(10,64,8)` and an Anvil above it at
`(10,65,8)`. The captured placed state was 9109. Two actual time publications at
game times 19 and 39 bracketed a stable supported block, with no falling spawn.
It then sent genuine START_DESTROY_BLOCK, packet 36/action 0, to remove support.

| Actual packet observation | Wire ordinal |
| --- | --- |
| Support `(10,64,8)` becomes air | 91 |
| Original Anvil cell `(10,65,8)` becomes air | 94 |
| AddEntity: real entity 2, type 40, carried state 9109 at `(10.5,65,8.5)` | 104 |
| Metadata index 8 / serializer 10 retains start position `(10,65,8)` | 105 |
| Explicit initial motion packet, all three raw components 0 | 106 |
| Landing BlockUpdate: state 9109 at `(10,64,8)` | 109 |
| RemoveEntities for the same entity 2 | 114 |

Actual event 1031/data 0/global false arrived at the landing block. Further time
publications at 59 and 79 showed one stable landed Anvil and no second falling
spawn. The one-block case avoids a positive-damage wear outcome; the same state
was observed throughout. Neither constructor nor wear RNG sequences were
reconstructed from these packets. A pinned-original bootstrap separately
confirmed the used packet IDs and the original source supplied type 40; this
was direct JDK21 inspection/runtime extraction, not REA MCP execution.

## Observed menu and item ownership

The landed block opened actual MenuType 8 with 39 slots. Two individually legal,
unenchanted iron swords 833 were supplied in separate max-size-1 slots. One
input received literal name `Cinder Forge` through original rename packet 42,
body `0c43696e64657220466f726765`. Its genuine result contained custom-name
component 5 and cost 1. Taking it consumed that input, cleared the preview and
cost, owned one named cursor stack and emitted actual event 1030/data 0/global
false at `(10,64,8)`.

Connected close returned that cursor once. Native save and a real PLAY cold
restart retained one named sword and one plain sword, with empty cursor and
player XP 0. The second plain sword was put into the same real menu and a
`Cinder Pending` preview was left untaken. Disconnect dropped the owned plain
input, not that named preview. A further native restart with no PLAY actor
retained player named sword 1 plus world plain sword 1. Crafted statistics
remained empty. The final real world had exactly one Anvil at `(10,64,8)`, state
9109, and the actual native Falling owner list was empty.

Thus the two initial swords remain exactly two owned swords after one rename;
the two derived previews add no extra ownership. The no-PLAY item checkpoint
restart proves acceptance and count retention, not restored item tracking or
ticking. The successful PLAY restart reopened the genuinely placed landed
block; no virtual workstation or fake block entity was introduced.

## Remaining boundaries

Production falling admission is currently restricted to actual Anvil actors in
the certified native world and proven quiescent block neighborhoods. Other
falling families, concrete/water behavior, active fluid/fire/redstone callbacks,
block-data/vehicle ownership and arbitrary imported histories require their
actual owners. This run does not exercise long falls, victim damage, positive
wear/breakage, timeout or failed landing drops, mid-flight save/restore, unusual
observer/chunk transitions, full RNG sequences or multiple dimensions/clients.

The menu observation covers only ordinary literal rename under the supported
creative unenchanted profile. Survival XP/wear, enchantment merging, repair
materials, QuickMove, drag, custom names/components, pickup/refusal/full-resync,
reentrant callbacks, ordinary autosave timing, Java saves, public/graphical
clients and performance remain unexercised or explicitly unsupported by their
source owners. No full checklist row, percentage or denominator changes follow
from this bounded witness.
