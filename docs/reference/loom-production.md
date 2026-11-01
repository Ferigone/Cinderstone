# Native Loom production boundary

**Runtime status: focused actual Main flow passed.** LoomMenu is connected to
Main at source `17f1fa6f73c3fed1688d8403e03a0507f6bcb549`, tree
`6e1931f9640fbba0946988846dccb22a199eeb11`, targeting Minecraft Java **1.21**,
protocol **767**. The full Main executable build passed. Actual offline loopback
execution completed at `2026-11-01T22:27:32.519206+00:00` on binary SHA-256
`8af216dc629a0b953fd087db3c2ed8074d7fa9f5ba1d9453aa1c32340adac115`.
This records the bounded production paths and observations below. It does not
complete an aggregate inventory/crafting checklist row or establish full vanilla
compatibility. Earlier source ledgers' unexported-stage wording is historical.

The later composition recheck also passed at source
`972cf8b9fb43e3f1a266e176d1eb37789b438c4b`, tree
`1e702a720ff44cf462d52dae3af641e828e136d2`, using binary SHA-256
`0008b1ac57e5e84a5e9d0a1e5b9e6d71c9b95ca666e62931dd04d1498a715e27`.
It includes refreshed shared MenuSync layouts, the actual Loom wrong-window
click guard and the source-phase counter projection described below. The
initial `17f1fa6` source/binary/report remain separate historical evidence.

## Actual Main entries and retained ownership

| Production entry | Actual owner and behavior |
| --- | --- |
| `main.rs`, `LivePlayers.looms` | Retain one `OpenLoom { position, menu, context }` per current Loom user |
| PLAY block use `0x38` | `use_native_loom` checks the real block, actor, hand/sneak/reach and configured source, retires a prior external menu, then opens the actual Loom |
| Container button `0x0d` | Decode two VarInts and dispatch `loom_button` against the actual matching window and selected configured pattern index |
| Container click `0x0e` | Validate component predictions, then dispatch `click_native_loom`; when Loom is current, a mismatched wire window is handled by ignoring it before inactive InventoryMenu fallback |
| Container close `0x0f` | Capture and retire the sole current server menu under one operation/world/player interval; the wire window does not choose the owner |
| Real player menu phase | `tick_native_loom_menu_for_player_locked` broadcasts, observes fresh block/reach validity, then closes invalid access |
| Actual native-body and connection pickup paths | Select `broadcast_for_inventory` for this current menu with the same canonical inventory and gameplay frame |
| Player capture | `inventory_snapshot` observes canonical inventory with the Loom's separate carried stack |
| Removal/reconfiguration | Remove retained InventoryMenu first, then Loom with actual `DeadOrDisconnected`, save, and remove the player |
| `Server::save` | Ordinary autosave retains menus; only its `shutting_down` branch retires external menus before the final checkpoint |
| Unsupported candidate recovery | `resync_current_player_menu` selects the retained Loom and its actual full-state operation |

The existing `SharedContainer` owns canonical player inventory. Loom owns its
banner, dye and template inputs plus a separate external cursor. Result slot 3
is a derived preview. MenuType is 18 with 40 slots: inputs 0/1/2, result 3,
main inventory 4..30 and hotbar 31..39. The player mappings are canonical 9..35 and
36..44; offhand remains canonical 45. The one synchronized data slot is the
selected pattern index. Retained window 0 keeps its own cursor and 2x2 inputs.
There is no Loom block entity, second player inventory, alternate writer or
independent entity-ID allocator.

## Supported native source and pattern holders

Mutable operations require the existing certified native flat-world source and
an actual alive, unmounted creative Overworld actor under the supported body
and lifecycle contracts. The source spectator branch is read-only: clicks use
full synchronization and buttons do not mutate. Spectator, survival, mounted,
unsupported effect and other custom actor profiles are not established by this
creative flow.

`NativeLoomPatterns` consumes the actual immutable configuration fixture that
Main sends: the unique banner-pattern registry packet, tag packet and default
feature packet. It retains registry holder IDs and each actual HolderSet's
encounter order. The ID-to-key domain is validated against the existing pinned
Slot component 49 registry owner; selectable choices are taken from tag bytes,
not regenerated from that metadata or sorted keys.

Empty template uses actual `minecraft:no_item_required`. A genuine configured
BannerPatternItem resolves its captured original item-to-tag association. The
template item is retained after taking, rather than consumed like banner/dye.
Missing tags yield no choices; other nonempty item classes do not acquire a
fabricated pattern tag. The retained snapshot has no live-reload capability; unsupported feature/item
profiles and mismatched holder domains fail explicitly. The runtime below uses no template;
these template contracts have source evidence without a Main runtime witness.

The retained `NativeLoomBinding` binds the actual native level-lock owner,
entity ID/UUID, dimension, captured position, configured pattern bytes and
constructor-default callback capability. Actions refresh actual block/time
observations under the shared locks and validate that binding. It is not a
persisted Java level identity or a menu-local replacement registry counter.
Pure synchronization reuses its owner binding without inventing a new validity
or game-time observation.

The native gameplay owner must certify its empty-criteria floor before callback
preparation. Unsupported registered advancement predicates/rewards, statistic
objectives, container/item listeners or reentrant hooks are not silently
ignored; missing imported files alone cannot prove empty registries. Ordinary
block opening uses the actual interaction statistic and default block-use
criterion owner. That is separate from the absence of crafted/recipe/XP effects
on taking a Loom result.

## Source phases, counters and sealed default callbacks

Each `LoomMenuEvent` retains its own `LoomPhase`: input stacks, derived preview,
selection, actual selectable holder list, canonical inventory observation and
external cursor. Intermediate input/result callbacks are not replaced with the
transaction's final image. The original input change order is superclass
change handling, `slotsChanged`, then update Runnable; output change handling
runs its superclass and that Runnable without an input recomputation.

The original constructor Runnable targets `cra.r()`, whose body is a sole
return. Main's private factory seals `OriginalServerDefault` together with the
actual configured owner and certified nonreentrant source profile. It does not
accept an arbitrary caller's permissive boolean as listener evidence. Java can
replace that Runnable through `registerUpdateListener`; such custom listener
ownership is outside this capability. The callback bridge completes the emitted
source boundary, without running result recomputation or broadcasts twice.

Player-backed slot listeners use their own phase's canonical inventory and
changed stack. Input/result listeners keep their actual backing owners and do
not impersonate player inventory. `PlayerInventoryChanged` calls `set_changed`
on the actual shared `PlayerInventory`, preserving the transient signed
wrapping counter independently of protocol state IDs and criteria. Pure pickup
broadcasts add no artificial counter, recomputation, sound or item take.

At source `972cf8b`, Main also projects the transient counter through each
historical phase. It first verifies that this kernel handoff left the prepared
inventory and all emitted phase counters at the original counter value. Each
phase receives only the preceding `PlayerInventoryChanged` increments; that
marker's own phase is its before-image. Commit installs the actual event's
inventory image into the same canonical owner before callbacks, then executes
that marker's `set_changed`. Later events therefore see the source after-image.
The final counter is checked against the prepared final inventory before that
final image is installed. A kernel that starts advancing these counters itself
requires an explicit new handoff, rather than silently doubling increments.
These are source-code/precondition contracts. The protocol flow below does not
independently instrument `Inventory.timesChanged` or every internal phase;
protocol state IDs are not substituted as evidence for that transient counter.

The refreshed shared `MenuSync` retains explicit layout fields for top slots,
player range and data slots. Existing Loom remains its actual registered kind,
with top 4, player slots 4..39 and one data slot; other new layouts do not alter
Loom's 40-slot contract. The composed runtime observation below checks that
actual Loom layout, not the correctness of every newly available menu layout.

Normal result taking removes one banner and then one dye with each real input
callback, adjusts unavailable selection, dispatches the captured-level take
sound when required, then invokes base Slot.onTake. The source introduces no
crafted-item statistic, recipe-book award or XP effect here. These are source
contracts, not a runtime instrumentation trace of every internal callback.

Main preflights historical stacks, actual feature/holder domains, native
criteria, callback binding, encodings, plan/menu/inventory identity and revision,
real time/chunk/rain facts and every drop index before commit. Unknown contexts
fail rather than receiving a fake result or callback. Its ordered commit
retains listener/counter/packet/source/drop positions under the empty-criteria
profile. Arbitrary exceptional or reentrant Java callback behavior remains
unsupported.

## Actual items, sound RNG and publication

Owned nonempty drops move once into the existing real ItemEntity preparation
path with actual player RNG, Level.random, random factory and shared entity-ID
owner. `PreparedItemBatch::commit_with_world_random_ordered` adds each entity at
its indexed Drop event, retaining callback/menu-packet publication between
additions through the existing FIFO. Drop statistics advance after world-add.
There is no second item outbox. Removal handles owned cursor before inputs 0/1/2;
the result preview is never an additional return, drop or persisted item.

`TakeSound` retains the source's actual captured-level game time. The kernel's
last-sound long begins at 0 and suppresses equal times, including first take at
time 0. An unequal time emits the event and records that time after the original
sound call. At that indexed event Main draws one seed from actual independent
**Level.threadSafeRandom**, through `ItemBatchPublication.next_sound_seed`,
even when its recipient list is empty. It draws no random pitch and uses no
player/drop Level.random state for that sound. Preparation or a later generic
tick does not consume this seed.

`LOOM_TAKE_SOUND_ID` has one Rust owner in `main_loom_patterns`, backed by the
original observed raw ID 1429. The existing sound packet 0x68 uses the registered
holder codec, BLOCKS 4, volume/pitch 1, and captured block X/Y/Z each plus 0.5.
The actual same-dimension audience uses strict squared distance below 256
(radius 16), without excluding the actor. The runtime observed the actor's
correct sound packet; it did not compare the complete RNG sequence, verify
same-time suppression or exercise empty/multi-player/audience-edge cases.
See [original sound/default-listener proof](loom-sound-source.md) and its
[public source facts](../../fixtures/loom-production-sound.json).

Immediate pickup preserves Collect-before-menu handling, actual callback
encounter order and returned packet order. Its existing private packet batch is
published after callbacks. This relies on the empty-criteria capability; it does
not prove packet/listener interleaving for hooks that publish packets or mutate
and reenter gameplay. The later player phase still owns its separate validity
obligation. Pickup was not executed in this runtime flow.

## Lifecycle, admission and actual resynchronization

Connected close returns owned cursor/inputs to canonical inventory or creates
actual overflow drops. Disconnection uses its actual lifecycle fact and drops
only owned stacks. Source Loom removal retains its old derived preview during
input-removal observations; that observation grants no additional ownership.
Server CloseScreen, when requested, precedes the removal publication. Client
close targets the actual current server menu, even when the wire says window 0.

Ordinary autosave retains the GUI and observes canonical inventory with the
external cursor. Shutdown retirement is restricted to `shutting_down`. This is
Cinderstone's current native checkpoint policy, not Java-save interoperability
or proof of arbitrary crash recovery for live temporary menus.

PLAY CreativeSlot/ContainerClick admission validates candidate components before
canonical mutation or prediction adoption. Surviving explicit FOOD20 retains an
unresolved source reference-identity requirement and is refused; inherited
prototypes remain admissible. The guard also visits nested Slot components 29,
30 and 52. Loom phase preflight validates its actual input/preview/cursor and
canonical inventory observations against that same native boundary.

On refusal, Main calls `resync_current_player_menu` under the actual sole-owner
selection interval. Loom's `Resync` uses genuine `broadcast_full_state` on the
retained menu and its real revision, inputs, preview, player slots and cursor.
It executes no dummy click or menu reinitialization and does not synchronize an
inactive window 0 for an external menu. Applicable equipped custom reach is
another typed unsupported boundary: tick, click/button or refusal recovery can
retire the actual connected menu. Other internal failures are not relabeled as
reach. These refusal, resync and reach paths are source-wired but were not
runtime-exercised by this no-template flow.

Main permits only the unambiguous empty/single-target drag order until the real
Slot-identity HashSet encounter owner is available. Multi-target drag remains
explicitly unsupported; this runtime exercised no drag mode.

## Focused actual protocol observations

The [sanitized actual Main report](../validation/loom-flow.json) pins the source,
tree and binary above. Its private independent raw framing/compression/Slot/NBT
harness SHA-256 is
`7e66156976e27161612199cd06370fd7d6a5198e3cc277b13691fb045286e1f2`.
It observed actual spawn-chunk publication and batch acknowledgement before
block interaction. The scenario began with fresh synthetic offline data and
reused only its own native checkpoints for cold restarts.

| Actual production observation | Result |
| --- | --- |
| Place/use a real Loom | OpenScreen type 18, 40 slots |
| Load white banner 1133 and red dye 958, no template | Actual inputs 2/2/0; button index 0 selected the actual configured first HolderSet entry |
| Preview identity | Component 49 retained raw holder 26, `minecraft:square_bottom_left`, color 14; those IDs came from actual configured registry/tag and original dye observation |
| Take once | Inputs became 1/1/0; cursor owned patterned banner 1; remaining patterned preview 1 stayed derived |
| Actual sound | Raw 1429, BLOCKS 4 at 10.5,64.5,8.5, volume/pitch 1 with signed seed present |
| Client close with wire window 0 | Actual Loom closed; player retained plain banner 1, patterned banner 1, dye 1 and empty cursor |
| Native save | Same player counts, no corresponding world drops; crafted entries empty, recipe awards/highlights empty and XP level/total/progress 0 |
| Native cold restart with PLAY actor | Restored the same counts and actual component 49 identity |
| Reopen with remaining inputs 1/1/0 and disconnect without taking | Player retained patterned banner 1; actual item checkpoint contained plain banner 1 and dye 1, patterned banner 0 |
| Native restart without PLAY actor | Accepted the nonempty item checkpoint and retained exact counts without duplicating preview |

The initial two banners and two dyes became one owned patterned banner plus one
remaining banner and dye. Every isolated process completed the actual SIGINT
path with exit 0. The saved crafted/book/experience observations do not by
themselves instrument every internal callback. The no-PLAY restart proves
checkpoint acceptance/count retention, not restored item tracking/ticking
readiness. Private player/world files, raw traces, logs and binaries are not in
the public report.

Templates, alternate patterns, drag/click-mode variants, custom components,
custom reach, immediate pickup, refusal/full resync, ordinary autosave timing,
full RNG sequences, sound audience edges and internal callback instrumentation
remain unexercised. Custom/reentrant/unknown source profiles and multi-target
drag remain unsupported. Java saves, public/graphical clients, deployment
behavior, complete workstations, performance and aggregate checklist completion
are not established by this local witness.

## Later composition and wrong-window regression

The [separate composed report](../validation/loom-stale-click-flow.json) completed
at `2026-11-01T23:46:20.279010+00:00` on source
`972cf8b9fb43e3f1a266e176d1eb37789b438c4b`, tree
`1e702a720ff44cf462d52dae3af641e828e136d2`, binary SHA-256
`0008b1ac57e5e84a5e9d0a1e5b9e6d71c9b95ca666e62931dd04d1498a715e27`.
Its distinct private harness SHA-256 is
`286d2e28c24d9ccad709c3eed64ee9f21bfe08a13e91cdb473043b523859cfda`.
The original `17f1fa6` flow/harness were preserved unchanged.

When Loom is current, `click_native_loom` now returns handled for an unequal
wire window, without dispatching a click to that menu or falling through to the
inactive InventoryMenu. With no retained Loom, it returns unhandled so actual
other-owner dispatch can proceed. The runtime scenario placed 7 stone in
canonical slot 11, then opened the real Loom with its actual banner/dye inputs.
It sent a well-formed window 0, slot 11 PICKUP payload `0006000b00000000` while the
current Loom window was 1. A wrong fallback would have picked up those stones
through the inactive InventoryMenu.

The scenario used actual normal close and a fresh Loom full-state reopen as its
observation barrier, requiring canonical slot 11 still to contain 7 stone and
both the actual cursor and retained window 0 cursor to remain empty. This is an
ordered protocol/state observation, not a sleep-based race campaign or a
fabricated resync call. The reopened actual Loom used window 2 and continued the
original no-template index 0→holder 26/color 14 flow.

The complete original ownership expectations passed again: one patterned
banner taken, one banner/dye consumed, actual sound 1429/BLOCKS 4, connected close,
PLAY cold restart, open-menu disconnect and final no-PLAY checkpoint retention.
Every native save retained all 7 stone in the player with none in world items or
cursor. Patterned/banner/dye ownership remained unchanged from the initial
scenario; crafted entries and recipe awards/highlights stayed empty, XP stayed 0,
and every isolated process exited 0 through its actual SIGINT path.

This runtime witness covers only the current Loom wrong-window case. Analogous
changes in the other four menu families have no new runtime witness here. It
does not instrument the internal counter/phase timeline, prove every new
MenuSync layout, exercise template/drag/reach/pickup/refusal-resync cases or
establish full sound RNG/audience behavior. The final no-PLAY restart retains
its native checkpoint-count boundary, without restored item tracking/ticking,
Java-save or public/graphical-client claims. No aggregate checklist completion
or progress percentage is changed by this composition recheck.

Source inspection is direct JDK 21 `javap` and bounded original bootstrap/registry
execution, not REA MCP. The original 1.21 artifacts and mappings are pinned in
[source-lock.json](../../fixtures/source-lock.json). See
[configured patterns](loom-patterns-production.md),
[opening/access](loom-opening.md),
[native menu source phases](workstations-loom-menu.md),
[kernel ownership](workstations-loom-cartography.md) and
[sound/default-listener proof](loom-sound-source.md) for source method evidence.
