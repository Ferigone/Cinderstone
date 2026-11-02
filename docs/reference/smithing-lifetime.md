# Native smithing menu lifetime

Target: Minecraft Java **1.21**, protocol **767**. This stage supplies
`src/main_smithing_lifecycle.rs` for the canonical `main_smithing` transaction
owner, plus its `main_player_body` pickup route. The coordinator owns exports,
central Main wiring and production evidence. Callable source alone is not
production proof.

## Pinned source evidence

The original inputs are pinned in `fixtures/source-lock.json`: server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The following facts were inspected directly with JDK21 `javap -p -c -l` and
official mappings. REA instructions were used, but no REA MCP operation or
original-Java runtime observation is claimed. Java artifacts and raw
disassembly remain outside the repository.

| Original method | Observed contract |
| --- | --- |
| `ServerPlayer.tick`, lines530–535 (`aqv.l`) | Calls current-menu broadcastChanges before testing stillValid and closing an invalid menu. |
| `ItemCombinerMenu.stillValid`, lines118–123 (`cqx.b` and its lambda) | Reads the block at its captured level/position, calls virtual isValidBlock, then checks the actual player with additional block-interaction range4.0. Block failure short-circuits reach. |
| `SmithingMenu.isValidBlock`, line58 (`crt.a`) | Tests canonical Blocks.SMITHING_TABLE identity. |
| `ItemCombinerMenu.removed`, lines112–114 (`cqx.a`) | Runs AbstractContainerMenu.removed first, then clears its input container through the captured level access. |
| `AbstractContainerMenu.removed`, lines558–569 (`cpu.a`) | Returns an alive connected ServerPlayer's cursor through the real inventory; a dead/disconnected player drops it through Player.drop. |
| `ServerPlayer.closeContainer` / `doCloseContainer`, lines1213–1223 (`aqv.s` / `aqv.t`) | Server close sends the actual current window's close packet before removal, then transfers synchronization to the retained InventoryMenu. |
| `ServerGamePacketListenerImpl.handleContainerClose`, lines1687–1689 (`aru.a`) | Closes the actual current menu; the received window ID does not select an older owner. |

The smithing input container owns template, base and addition in slots0/1/2.
The result is a derived preview and never a returned or dropped owned input.
The existing [smithing ledger](workstations-smithing.md) records the source
input-callback, result-recompute, recipe and level-event contracts.

## Actual owners and methods

`LivePlayers.smithing_tables` retains each actual `OpenSmithing`: its captured
position and one authoritative kernel menu. The Server retains the actual
world/level, player registry, shared inventory, player/entity/drop RNG and item
store. A smithing table has no inventory-owning block entity.

`tick_native_smithing_menu_for_player_locked(world, players, id, now, border)`
executes once in that player's actual menu phase. It delegates Broadcast to
`smithing_action_locked`, preserving phased synchronization callbacks and their
ordered publication. It then reads the current menu/player, obtains
`main_smithing_dispatch::smithing_access` for the captured position and invokes
the kernel menu's actual still_valid. Invalidity requests a server-side close
with `AliveConnected`. Broadcast must not perform an earlier validity rejection.

The access host supplies real captured block identity and the actual supported
player pose/attribute context. Reach is evaluated only after the block test.
The action owner must certify actual world identity and its supported player
capabilities; missing owners are errors, not default facts or silent item loss.

`close_native_smithing_table(id, window, send_close)` is a standalone connected
operation. It acquires chat_dispatch first, then obtains the clock/border and
world -> players locks. Its window argument must be selected from the current
server owner. A mismatched captured window returns false without mutation.

PLAY closes the actual current menu through `Server::close_current_player_menu`,
which selects and removes its owner under one chat_dispatch -> world -> players
operation. It ignores the received window selection and never crosses separate
async menu wrappers with an earlier remembered window. The existing locked
smithing helper runs without reacquiring those guards. Overlapping external
owners are an explicit error; window0 uses the actual InventoryMenu owner.

`close_smithing_with_context_locked(world, players, id, window, send_close,
now, border, context)` never reacquires those outer guards. It delegates Close
to the actual prepared transaction. The transaction owns cursor first, then
input0, input1 and input2 returns/drops, including intermediate raw inventory
packets, actual drop construction and RNG. The lifecycle does not flatten those
events or manipulate the preview. There is no smithing button request.

Disconnected/dead actor cleanup explicitly uses `DeadOrDisconnected`; it must
not borrow the connected-player inventory-return path. Main first removes the
retained InventoryMenu, then this actual external menu, before saving/removing
its actor, inventory/drop and RNG owners. Reconfiguration/connection removal
uses that actual context-aware player path. This does not describe native
SIGINT checkpoint retirement.

Ordinary autosave retains the actual current smithing menu and its three owned
inputs. It must not call `close_smithing_tables_for_save_locked`. The existing
active-menu inventory snapshot bridge supplies the actual cursor view without
turning input or result slots into additional owned player-inventory items.
Root owns these actual player-save call sites.

`close_smithing_tables_for_save_locked` is available only for an explicit native
checkpoint retirement of connected menus. This is a native checkpoint policy,
not a claim that vanilla's ordinary periodic save forcibly closes menus. Actual
disconnect/dead actor removal still uses its disconnected per-player context
after retained InventoryMenu removal. The current native SIGINT branch instead
calls Server::save while actors are still connected: it retires external menus
with AliveConnected before final capture, and does not first remove InventoryMenu.
This explicitly separate native shutdown policy is not vanilla dead-player
removal or evidence of periodic-save closure.

All locked methods rely on the caller's chat_dispatch -> world -> players ->
shared-container order. They acquire neither chat_dispatch nor player_admission
inside a locked callback. Menu operations do not mutate admission policy.

## Native body pickup synchronization

The `main_player_body` Item pickup path now selects the actual current smithing
menu alongside its existing stonecutter/furnace owners and rejects overlapping
active external-menu owners. It retains one actual `MainGameplayFrame`, the
real `PickupPlayer` identity and the canonical shared inventory transaction.
`with_external_menu_sync_and_hooks` dispatches smithing
`broadcast_for_inventory` at the existing ServerPlayer.take synchronization
point, after Collect and before private packet publication. The same borrowed
gameplay frame executes each phased inventory listener. There is no second
InventoryMenu broadcast, hook clone or menu-local player-inventory owner.

Stonecutter and furnace pickup routing retains its prior behavior when those
menus are current. Experience pickup remains with its actual XP owner. Ordinary
Player inventory tick/equipment/held-stack reads keep the canonical backing
inventory; smithing inputs and its result preview are not new held or equipped
items and are not added to that tick.

The Smithing runtime owns its phased pickup bridge; this lifecycle does not
replace it with a packet-only helper. The real phased `broadcast_full_state`
kernel wrapper is separately required for spectator resynchronization, under
the workstation team's ownership. A fabricated phase or final-state packet is
not a substitute for that API.

## Unsupported reach-profile transition

Only the typed `main_smithing_dispatch::UnsupportedMenuReach` result is handled
as a bounded native profile transition during Broadcast. That error denotes an
applicable equipped block-interaction-range modifier without an actual native
attribute evaluator. It can arise after a legal inventory/hotbar selection
change while the menu is already open.

The lifetime retires that actual current menu through the real connected-player
Close transaction, conserves its cursor/input ownership, publishes the server
close and logs `native_menu_profile_retired`. It then allows the level tick to
continue. A retirement failure or any other actor/owner/encoding error still
propagates. This does not implement vanilla custom-range attributes or invent
their resulting range. Supported Broadcast -> stillValid ordering is unchanged.

## Required central wiring and remaining proof

The coordinator owns all changes to existing Main/shared seams:

- Declare this module and the smithing transaction/dispatch modules, initialize
  the actual smithing_tables owner once, and retain it through ordinary phases.
- Invoke the per-player lifetime method once in the real level/player menu
  callback used for existing furnace and stonecutter menus, preserving that
  actor pass's actual ordering.
- Route container clicks and current-window close to the actual smithing menu.
  Ignore the close packet's untrusted window selection; a client close does not
  request a redundant server-close packet.
- Close smithing before opening another actual menu; opening smithing likewise
  retires the previous current menu. Include smithing in orphan checks and
  connected InventoryMenu-cleanup guards.
- Preserve retained-InventoryMenu-first, external-menu-second disconnect and
  shutdown removal. Ordinary autosave retains menus. Explicit retirement and
  inventory snapshots preserve actual cursor/input ownership without treating
  the derived result preview as owned inventory.

Changed smithing blocks need no invented block-entity retirement path. The next
actual player menu phase checks the captured block and closes invalid access.
Same-type replacement follows that actual block query.

The coordinator assembles the actual production owners and call sites before
recording focused Main compilation and an actual menu/input/result/close/drop/
restart flow. Those observations remain separate from this source-level
handoff. No broad suite, test-count progress, graphical-client proof or
whole-delivery completion is asserted by this stage.
