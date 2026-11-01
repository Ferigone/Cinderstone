# Stonecutter production lifetime contract

Target: Minecraft Java **1.21**, protocol **767**. The actual menu owner is
`StonecutterRuntime` in `main_stonecutter`, holding a captured block position and
one authoritative `StonecutterMenu` for each admitted player. The native server
owns its actual level/world, player, shared inventory, drop entities and RNG.
A stonecutter does not have an inventory-owning block entity.

This stage adds `src/main_stonecutter_lifecycle.rs`. The integration coordinator
owns existing Main call sites, publication evidence and checklist decisions.
It does not claim a complete workstation delivery from one connected menu.

## Original evidence and order

The pinned original server and official mappings are identified by
`fixtures/source-lock.json`: outer server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The observations below came from direct JDK21 `javap -p -c -l` and the official
mapped methods. REA instructions were used; no REA MCP execution is claimed.
Raw disassembly and original Java artifacts remain outside the repository.

| Original method | Observed lifetime contract |
| --- | --- |
| `ServerPlayer.tick`, lines530–535 (`aqv.l`) | Calls the actual current menu's `broadcastChanges` first, then tests `stillValid` on the server and closes an invalid current menu. |
| `StonecutterMenu.stillValid`, line130 (`crw.b`) | Delegates through its captured `ContainerLevelAccess`, actual player and canonical stonecutter block identity. |
| `AbstractContainerMenu.stillValid`, lines81–86 (`cpu.a`) | Tests the captured block, then the actual player's `canInteractWithBlock` with additional range4.0; a guessed block entity or fixed-distance substitute is unnecessary. |
| `StonecutterMenu.removed`, lines252–256 (`crw.a`) | Runs parent removal first, discards the derived result preview, then clears its owned input through the captured level access. |
| `AbstractContainerMenu.removed`, lines558–569 (`cpu.a`) | Handles the actual cursor: dead/disconnected ServerPlayer drops it; an alive connected player returns it through the real inventory owner. |
| `ServerPlayer.closeContainer` / `doCloseContainer`, lines1213–1223 (`aqv.s` / `aqv.t`) | Server close sends the current container-close packet before removal; actual removal restores the retained InventoryMenu and transfers synchronization state. |
| `ServerGamePacketListenerImpl.handleContainerClose`, lines1687–1689 (`aru.a`) | Closes the actual current menu; its received window ID is not authority to select a different retained menu. |

Static source inspection establishes this order, not a graphical-client or Java
runtime observation. Authoritative input/result and ordered removal/drop details
are additionally documented in [workstations-stonecutter.md](workstations-stonecutter.md).

## Native methods and call phases

`Server::tick_native_stonecutter_menu_for_player_locked(world, players, id,
now, border)` runs once in the actual player's menu phase. It first submits
`StonecutterRequest::Broadcast` to the authoritative action owner. That owner
performs real shared-inventory synchronization, local listeners and ordered
packet publication. The lifecycle then queries the actual current player and
captured position through `main_stonecutter_dispatch::stonecutter_access`, and
invokes the current kernel menu's actual `still_valid` with that access host.
An invalid captured block or out-of-range player closes that captured server
window through the same authoritative transaction, with server-close output.
The Broadcast request must not itself perform an earlier validity rejection.

The shared validity host implements `WorkstationAccess`: canonical block state
at the captured level position, current player pose/eye position and the actual
supported interaction-range attribute profile, with the source's4.0 buffer.
The menu belongs to this Server's actual world; its action/callback owner must
certify the current native source and supported canonical player profile.
Missing actors, attributes or capability owners are errors, not fabricated
defaults or permission to discard an orphan menu's items.

`Server::close_native_stonecutter(id, window, send_close)` is the connected-player
wrapper. It obtains the actual game clock and border, then world -> players
locks. The window argument must come from the current server menu; Main ignores
an untrusted close packet's window ID when selecting that actual owner.
A mismatched captured server window returns false without altering ownership.

`Server::close_stonecutter_with_context_locked(world, players, id, window,
send_close, now, border, context)` composes with already locked operations.
It delegates to `StonecutterRequest::Close`; it does not clear input/cursor
directly. Connected close/replacement uses `AliveConnected`. Actual player
disconnect/removal uses `DeadOrDisconnected`, preserving real drop construction,
entity allocation, player/level RNG and publication through DropCommitCallbacks.
The preview is derived output and never becomes an owned returned/dropped item.
Errors leave the transaction owner's documented mutation/publication boundary;
the lifecycle does not retry a partially executed operation.

`Server::close_stonecutters_for_save_locked` retires each actual menu through the
connected-player path before native player snapshots. This follows Cinderstone's
existing external-menu checkpoint policy; it does not claim that vanilla's
ordinary periodic save forcibly closes menus. A save cannot silently omit the
cursor/input of an unpersisted live menu.

The optional `tick_native_stonecutters` adapter obtains the same locks for a
whole phase. Production must choose either the per-player call or that adapter;
calling both would broadcast twice. The real per-player level/menu phase is the
intended Main integration, matching the existing furnace lifetime call.

## Central wiring required

The integration coordinator owns these changes to existing files:

- Declare the lifecycle module and retain the actual `LivePlayers.stonecutters`
  owner. Initialize it once; do not replace it on ordinary broadcasts.
- In the actual level/player menu callback currently invoking
  `tick_native_furnace_menu_for_player_locked`, invoke the stonecutter per-player
  method exactly once, retaining that actor pass's actual order.
- Route container close to the actual current server window. Pass
  `send_close:false` for the client close operation; invalidity and server-side
  menu replacement use server-close output.
- Before opening another chest/barrel, crafting table or furnace, close the
  actual stonecutter with `AliveConnected` while already holding world/player
  locks. Opening a stonecutter likewise retires the previous actual menu.
- Actual player removal first cleans the retained InventoryMenu, then the active
  stonecutter with `DeadOrDisconnected`, before save/removal of actor/drop/RNG
  owners. Include stonecutters in orphan-menu checks and connected InventoryMenu
  cleanup guards.
- Apply the existing native checkpoint policy before save snapshots. Include
  `stonecutters.inventory_snapshot` wherever the canonical shared inventory is
  combined with the actual active menu's cursor for save or authoritative
  player snapshots; never serialize the result preview as owned inventory.

No block-entity retirement callback is needed when a stonecutter block changes.
The next actual menu phase observes the captured block through stillValid and
closes it. Querying the real replacement state preserves same-block replacement
semantics without inventing a block-entity identity.

## Ordered item and sound publication support

The narrowly added `PlayerInventory.snapshot_with_carried` returns a detached
view of the existing canonical inventory with its actual active menu cursor.
It uses the existing authoritative carried-stack normalization, preserving
slots, selected hotbar and synchronization state without moving live items.

`PreparedItemBatch.commit_with_world_random_ordered` retains the validated
exclusive store borrow and the existing world-RNG, entity-index, tracking,
tick-list and outbox commit operations. Its callback runs immediately before
each actual item insertion, then once after the last insertion; an empty batch
still has one final callback. The Stonecutter transaction can consequently
publish each indexed Drop and its surrounding packets/callbacks in order.
Older batch commit entry points use the same implementation with an empty
plumbing observer, adding no gameplay callback or random draw.

The callback receives only `ItemBatchPublication.next_sound_seed`, borrowing
the existing actual sound RNG. This is `Level.threadSafeRandom` (`dcw.f`) and
its `RandomSource.nextLong` (`ayw.g`), distinct from Level.random and the
player/entity drop streams. The capability exposes no entity mutation or RNG
replacement. The transaction must certify sound-owner availability before its
infallible commit; no fallback or cloned replacement sound stream is supplied.

The focused `cargo check --locked --lib` passed after these two additive native
APIs. It establishes compilation, not the Main menu/runtime flow below.

Full Main compilation and one actual protocol/menu/drop/restart flow belong to
the coordinator after these call sites are connected. No test count, isolated
kernel, graphical-client result or whole-Minecraft compatibility percentage is
claimed by this source note.
