# Native Stonecutter production boundary

Initial source stage: `5c496b2aba781944210bf518c5a3f447fe45baf6`, targeting
Minecraft Java **1.21**, protocol **767**. This stage connects the native
Stonecutter kernel to Main. It does not complete all workstations or establish
full vanilla compatibility.

**Runtime status: focused local Main flow passed.** The full Main compiler
check and executable build passed. Actual offline loopback protocol execution
completed at `2026-11-01T21:13:56.558189+00:00` on binary SHA-256
`09f662444521fb4df9535fd05cf9947c2f0ed820c693c3d38502ec0778907cf9`.
This records the bounded observations below, not all Stonecutter behavior.
The latest bounded execution used publication source
`5553015344f939f6bb255f414439e1c3b022af5a`, tree
`adf8b3920e3a75b60fa7d19451602112539d1178`, and binary SHA-256
`526842c949dd1ab039e0b71a017cb8ce794e0241d526c1344002ad6e4c922539`.
The positive flow and genuine FOOD-refusal/full-state recovery passed. Queued
button retirement was exercised; queued click-handler coverage remains
unresolved, as detailed below. Earlier `5c496b2`, `e18ad37` and `57e90e6`
witnesses retain their separate source/binary attributions.

## Actual Main call paths

| Production entry | Bound owner and behavior |
| --- | --- |
| `main.rs`, `LivePlayers.stonecutters` | One `StonecutterRuntime` owns each captured block position and active menu; the existing `SharedContainer` owns canonical player inventory |
| PLAY block use | `use_native_stonecutter` checks the real world block, actor, hand/sneak/reach and feature context, closes a preceding external menu and allocates the existing shared window counter |
| Container button `0x0d` | Decode the original two VarInts; route selection through the actual current Stonecutter window and handler gates |
| Container click `0x0e` | Decode actual Slot/prediction data and dispatch `click_native_stonecutter` before inactive InventoryMenu fallback |
| Container close `0x0f` | Capture and retire the actual current server menu under one operation/world/player ownership interval; the received window byte cannot revive an older menu |
| Unsupported FOOD candidate in PLAY `0x32` / `0x0e` | Refuse surviving explicit FOOD20 before canonical mutation, then call `resync_current_player_menu` for the actual retained owner |
| Actual per-player menu phase | `tick_native_stonecutter_menu_for_player_locked` broadcasts first, then checks the captured block/reach and closes an invalid menu |
| `PlayerBodyPhase::Pickup` and existing fallback pickup owner | `broadcast_for_inventory` synchronizes the actual current Stonecutter immediately after Collect; it does not replace it with window0 or defer it to the later validity phase |
| Other block-menu openers | Container, furnace and crafting-table paths remove the captured Stonecutter before creating the replacement menu |
| `capture_player_locked` | Observe canonical inventory with the external menu's separate carried stack and actual player-statistic owner |
| `cleanup_player_admitted` | Remove retained InventoryMenu first, then Stonecutter with `DeadOrDisconnected`, save, and remove the actual actor |
| `Server::save` shutdown branch | Retire external menus before final capture; ordinary autosave retains live menus |

Reconfiguration uses the existing player-removal path. There is no Stonecutter
block entity, second player inventory, private packet writer or independent
entity-ID allocator. Its 38 menu slots comprise input slot 0, derived result slot 1 and the
36 actual player slots; MenuType is 24. Window0 retains its own cursor and 2x2
inputs when an external menu opens.

## Supported source and actor profile

Mutable actions require the existing certified native flat-world source and
actual alive, unmounted creative Overworld actor, with the supported body and
lifecycle profile. The source spectator branch is read-only: clicks synchronize
the current state and buttons do not mutate it. These paths are not a claim
that all spectator, survival, vehicle or effect behavior is implemented.

`NativeStonecutterRecipes` reads the same unique immutable feature-flags packet
that Main sends. Its `PinnedDefaultRecipeContext` binds the actual packaged
recipe/item catalogue and captured default encounter order, with the supported
`minecraft:vanilla` feature set. Dynamic recipes/features and unknown imported
source data are explicit unsupported contexts.

Reach uses actual actor feet/pose, the captured block's unit AABB and supported
default interaction range, including the source verification buffer. An
applicable equipped block-interaction-range modifier is rejected until its
actual attribute owner is implemented. An inactive modifier in another slot
group is not treated as an active reach override.

When a later accepted inventory mutation activates that unsupported range while
a Stonecutter is open, the native player phase or a queued button/click handler
retires the actual menu through its connected removal path. The handlers catch
only typed `UnsupportedMenuReach`, capture the real current window and close it
with `AliveConnected` under the same ownership interval. Internal transaction or
encoding failures are not silently classified as unsupported reach. This
preserves the world process and owned items without pretending to implement
custom interaction-range behavior. The queued button handler has runtime
evidence below; the analogous click handler has source evidence but no isolated
runtime witness.

The gameplay registry must prove its native empty-criteria floor: no unsupported
registered advancement predicates/rewards, statistic objectives, block listeners
or reentrant item hooks. Missing imported files do not prove empty registries.
The ordinary block callback awards the interaction statistic, reads the actual
post-use world block and dispatches the existing default-use criterion owner.
The spectator provider branch precedes those ordinary-use effects.


### Surviving FOOD identity boundary and recovery

Well-formed protocol data does not establish the native ownership semantics of
an explicit FOOD20 override. `main_workstation_components` first applies the
existing proven component normalization; any surviving explicit FOOD20 requires
its original nested ItemStack reference owner and is rejected with typed
`UnsupportedFoodIdentity`. Inherited FOOD prototypes remain admitted. The guard
also visits actual nested Slot values in components 29, 30 and 52; this runtime
flow does not exercise that entire nesting boundary.

The PLAY CreativeSlot and ContainerClick admission paths log
`native_component_admission_refused` before installing the rejected stack or
prediction. Recovery calls `resync_current_player_menu`, which captures the
actual sole current owner under the normal operation/world/player interval and
asks that retained owner for genuine full state. For Stonecutter this uses its
real menu, revision, input, derived preview, player slots and separate cursor.
It does not generate a dummy click or publish inactive window0 as the recovery
for an external menu. The shared dispatcher has other current-owner branches;
only actual Stonecutter recovery was exercised in this flow.

## Phase callbacks, output and ownership

`StonecutterMenuEvent` carries a `StonecutterPhase` containing that event's actual
input, derived preview, selection, canonical inventory observation and separate
external cursor. These snapshots are callback arguments, not additional live
owners. Pre-consumption recipe input and intermediate inventory/cursor phases
are not replaced with the transaction's final image.

`PreparedStonecutterCallbacks` validates the existing gameplay and recipe-book
owners before the shared transaction commits. Normal result pickup awards its
actual output count to `ITEM_CRAFTED`; a two-slab result awards two. The
quick-move pre-callback is `Item.onCraftedBy`, which does not award another
crafted statistic. After a full transfer the source onTake callback receives an
empty stack, preserving the zero-count AIR crafted-stat call.

Actual recipe awards pass through `ServerRecipeBook.add_recipes`, retaining
known/highlight state and Add packets at their source event positions.
`InputChanged` completes only the constructor-default input listener: direct
original bytecode identifies `crw.q()` as a return-only Runnable after the real
SimpleContainer/menu state callbacks. Main exposes no replacement-listener
capability. Custom listeners are not silently substituted with that default.

`PlayerInventoryChanged` is a separate mandatory operation. At its recorded
event, the commit calls `canonical_menu.inventory.set_changed()` on the real
wrapping transient `timesChanged` counter, independently of packet or listener
counts. Player-backed slot criteria use each event's phase inventory; temporary
input/result listeners do not impersonate player inventory.

The result preview remains derived. Only successful taking owns output.
Connected removal returns owned cursor/input to inventory or creates actual
overflow drops. Actual disconnection drops the owned cursor and input through
`Player.drop`; it never returns or persists an additional preview stack.

## RNG, publication and lifetime

Prepared actions validate canonical inventory/menu identity and revision,
callback phases, component encodings, actual chunk/rain facts and drop indices.
Every nonempty drop consumes one owned stack and becomes one prepared real
ItemEntity. Construction uses the existing player RNG, Level.random, random
factory and serialized global entity-ID owner.

`PreparedItemBatch::commit_with_world_random_ordered` adds each item at its
indexed Drop event. Between additions, Main replays phase callbacks, recipe-book
checkpoints, slots/cursor packets and sounds through the existing ordered event
stream. Drop statistics follow world-add. A requested server closure publishes
CloseScreen before removal events. Unsupported fallible contexts are rejected
at the native preparation boundary; this is not support for arbitrary Java
exception/reentrant behavior.

The kernel suppresses repeated take sounds within the same actual game time.
At the indexed sound event, Main draws one seed from the actual independent
Level.threadSafeRandom owner, even if the audible audience is empty. It does
not draw a pitch or seed from Level.random. The packet uses source sound 1431,
BLOCKS, centered block coordinates, volume/pitch 1 and the source variable-range
audience.

Immediate item pickup preserves Collect-before-menu publication, callback
encounter order and packet order. Its existing publisher returns the private
packet batch after callbacks. This is supported by the actual empty-criteria
owner; it does **not** establish listener/packet interleaving for callbacks that
publish packets or mutate/reenter gameplay. Such contexts remain unsupported.
The later real player phase still owns its separate validity/close obligation.

Ordinary autosave captures the live canonical state without closing a GUI.
Shutdown retirement runs only when `shutting_down` is true. The recorded cold
restart flow, when available, concerns Cinderstone's native player/item formats;
it is not proof of Java player.dat interoperability or arbitrary crash recovery
for live temporary menus.

## Focused validation

The [recorded actual Main flow](../validation/stonecutter-flow.json) uses the
above exact source/binary and independent raw protocol framing, compression,
Slot and NBT readers. Its harness SHA-256 is
`f9b5656cfef8907b9f054f0abe96d942b1711839363d44319155949e370e31d7`.
The client applies the local cursor prediction it declares before observing
corrections; matching authoritative cursor state need not emit a redundant
packet. Fixtures supply the actual stone item 1, slab 264 and recipe selection 5.

| Actual production observation | Result |
| --- | --- |
| Creative block placement and ordinary use | Real Stonecutter OpenScreen, type 24 and 38 slots |
| Select and take a two-slab result | Input 8 became 7; cursor owned 2 slabs while the remaining result 2 stayed a derived preview |
| Client close with received window 0 while Stonecutter 1 was current | Actual current menu closed; inventory retained 7 stone + 2 slabs and empty cursor |
| Another normal take followed by opening a real crafting table | Stonecutter removal returned its owned state once; crafting MenuType 12 opened; subsequent inventory contained 6 stone + 4 slabs |
| Native player cold restart | Actual PLAY inventory restored 6 stone + 4 slabs |
| Normal take then disconnect with active Stonecutter | Player saved 4 slabs; actual item checkpoint contained 5 stone + 2 slabs, without an additional preview item |
| Normal crafted callback statistics | Saved crafted-slab statistic 6 matched three ordinary two-slab takes |
| Cold restart with that nonempty item checkpoint | Server accepted/restored the checkpoint and preserved those counts through another save, with no PLAY actor created |
| Isolated one-input full QuickMove | Input/result/cursor became empty, inventory owned 2 slabs, AIR crafted-stat entry 0 was present and slab crafted-stat remained 0 |

The primary scenario conserved its initial 8 stone as 5 remaining stone and 6 slabs;
the independent QuickMove fixture did not change that inventory. Private
synthetic player/world files, raw wire traces and logs are excluded from the
public report. All isolated servers stopped through their real SIGINT/shutdown
path. The initial harness attempt stopped on a stale client cursor observation;
the corrected client prediction passed against the same unchanged Main binary.

This flow does not exercise graphical/public clients, Java player.dat imports,
ordinary autosave timing, same-game-time sound suppression, custom range
modifiers or immediate item pickup while a Stonecutter is open. The final
no-PLAY-actor restart proves nonempty native checkpoint acceptance and count
retention; it does not prove restored item tracking/ticking readiness.

### Valid equipped-range retirement regression

The [separate focused regression](../validation/stonecutter-reach-flow.json)
passed at `2026-11-01T21:23:52.732915+00:00` on fix source
`e18ad3781c2a94884925996e011466acc3dac5cf`, binary SHA-256
`515a8a33b9b8ac3d77a8e0e0f67315a17a9e02c4c70daa182fb1d0f6d46c4b93`.
Its harness SHA-256 is
`f72303996d6462af09524800c9b0a14ae01a66b3927ba5d92d978a45d7986ae2`.
The earlier positive flow remains tied to its original source/binary above.

With actual input 7 and an owned cursor of 2 slabs, the client sent a valid
CreativeSlot update to selected hotbar slot 36: stick 848 with component 12,
attribute 6, ResourceLocation `cinderstone:reach_probe`, finite additive amount 1,
operation 0, equipment group `main_hand` and the real tooltip boolean. The wire
fields come from the pinned protocol schema; malformed bytes did not trigger
this case. The server echoed the actual attribute component.

The current Stonecutter window 1 received CloseScreen and returned to window 0
with empty cursor. Actual inventory retained 7 stone, 2 slabs and the one modified
stick; no result preview was duplicated and no world-item overflow was created.
After another 1.1 seconds the same PLAY session received its real `/list` reply,
and an independent status request reported one online player. The native
`native_menu_profile_retired` event named `unsupported_block_interaction_range`.
Graceful shutdown exited 0 and its actual saved player/item owners retained the
same counts.

This is a safe explicit native-profile retirement, not full custom attribute
compatibility. The valid CreativeSlot activation path was exercised; activating
a preloaded modifier through a later selected-hotbar packet was not separately
exercised. No broad source-suite, graphical or public-network result is claimed.

### Composed publication candidate

Both bounded flows passed again on publication candidate
`57e90e63e9dcd4c103761a909854886334a702c7` atop refreshed master. Its full Main
build and actual execution used binary SHA-256
`a0169096d316d28ab7f5568cb6fe665b70947c2333d6b46c4374f2d17266072b`.

- The [composed positive flow](../validation/stonecutter-composed-flow.json)
  completed at `2026-11-01T21:36:48.365201+00:00`. It retained the original
  block/menu/callback, removal, restart and QuickMove expectations, including
  primary conservation of 8 stone as 5 stone plus 6 slabs.
- The [composed reach regression](../validation/stonecutter-reach-composed-flow.json)
  completed at `2026-11-01T21:36:08.076414+00:00`. The valid equipped modifier
  again retired only the unsupported Stonecutter menu, conserved input/cursor
  and retained the actual PLAY session and world process.

Each recheck used a fresh private runtime and the same unchanged corresponding
harness hash recorded above. All isolated processes completed graceful SIGINT
shutdown with exit0. The earlier `5c496b2` positive and `e18ad37` reach witnesses
remain separate. These composition checks do not extend their declared coverage.


### Latest composed source: positive, queued reach and FOOD recovery

The final frozen publication source `5553015344f939f6bb255f414439e1c3b022af5a`
(tree `adf8b3920e3a75b60fa7d19451602112539d1178`) passed the full Main build.
All observations in this subsection use binary SHA-256
`526842c949dd1ab039e0b71a017cb8ce794e0241d526c1344002ad6e4c922539`
and separate fresh private worlds. They preserve the earlier witnesses above.

| Focused execution | Actual outcome |
| --- | --- |
| [Final positive flow](../validation/stonecutter-final-flow.json), completed `2026-11-01T21:56:22.857194+00:00` | Passed the same ordinary take, stale wire-close, cross-menu replacement, native cold restart, disconnect ownership and full QuickMove expectations. Initial 8 stone remained equivalent to 5 stone plus 6 slabs; the isolated QuickMove retained the source AIR0 crafted callback. Harness SHA-256 remained `f9b5656cfef8907b9f054f0abe96d942b1711839363d44319155949e370e31d7`. |
| [Queued reach flow](../validation/stonecutter-queued-reach-flow.json), completed `2026-11-01T21:55:33.692366+00:00` | The valid reach CreativeSlot and Button were sent in one TCP write. The actual `container_button` handler retired the menu, conserving 7 stone, 2 slabs and one modified stick, empty cursor and continued PLAY. Its later queued click was safely retired by the player tick before handler dispatch: the client retained 6 stone, 4 slabs and one stick and received `/list` with status online1. The strict handler assertion failed, so this report remains `ok: false`; it does not establish click-handler runtime coverage or its unreached final saved-state assertions. |
| [Single isolated queued-click retry](../validation/stonecutter-queued-click-flow.json), completed `2026-11-01T21:58:02.499401+00:00` | Setup timed out awaiting block-change/acknowledgement before the queued packets were sent. The saved world did contain the placed Stonecutter, state18467 at10,64,8; no internal server error was observed. A missing visibility/acknowledgement precondition is an inference, not a proven cause or a demonstrated Main defect. This report remains `ok: false`; no further timing retries were performed. |
| [FOOD admission and actual current-menu recovery](../validation/stonecutter-food-flow.json), completed `2026-11-01T21:55:33.420526+00:00` | Passed. While input7 and cursor2 slabs were owned by the current Stonecutter, a valid explicit FOOD20 mushroom-stew override was refused before replacing player slot10's dirt3. A new full content packet for that same actual Stonecutter restored the client's prediction and retained input/cursor. Normal close/reopen independently conserved the actual inventory; an inherited mushroom stew without an explicit override remained admitted. PLAY `/list`, online1 and graceful native saved ownership all remained valid. |

The FOOD case uses the captured prototype payload for item849 from
`fixtures/item-components-1.21.json`, SHA-256
`a0e45f91fd300767f95ea88690fd8401406b7ff9941f4bde08d836ec9ac62ab6`.
Its harness SHA-256 is
`2cf75eedc09a27b85abc33269017e02293a9a681ae3fdfe52b33b2fece346393`.
The final saved player held 7 stone, 2 slabs, 3 dirt and one inherited mushroom
stew, with empty carried state and none of those items in the world checkpoint;
shutdown exited0. This is a defined unsupported custom-identity admission and
recovery boundary. It does not implement custom FOOD reference identity,
exercise the other surviving prototype overrides or certify all-menu recovery.

Queued reach harnesses have SHA-256
`50d04c569428f151ad54939bf21dcc6a26824ce8daa38ba37c64547b7fd53a0d`
(two-path run) and
`993480b2d0a86649cd5585588998e42c426bb55acc2958f90a6f69dec3d652d7`
(single isolated click retry). Their partial/unexercised outcomes are retained
without converting safe tick handling into proof of a queued click handler.

Source inspection is direct JDK21 `javap` and bounded original-class/registry
evidence, not an REA MCP execution. The original server and official mappings
are pinned in [source-lock.json](../../fixtures/source-lock.json). See
[Main composition](stonecutter-main-wiring.md),
[transactions](stonecutter-transactions.md),
[recipes and callbacks](stonecutter-recipes-production.md),
[opening](stonecutter-opening.md),
[criterion ownership](stonecutter-criteria-owner.md) and
[lifetime](stonecutter-lifetime.md) for source methods and narrower contracts.

No public-network, graphical-client, complete advancement, all-workstation,
performance or checklist-delivery result is established by this document.
