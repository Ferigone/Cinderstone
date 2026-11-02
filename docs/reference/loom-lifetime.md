# Native LoomMenu lifetime

Target: Minecraft Java1.21, protocol767. `main_loom_lifecycle.rs` supplies the
actual per-player lifetime and context-aware closing entry points for the
retained `main_loom::LoomRuntime`. Central Main assembly and runtime evidence
belong to the integration coordinator. This unexported stage is not proof of
a production LoomMenu call or a completed checklist delivery.

## Original evidence

Pinned original inner server SHA256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings SHA1: `31c77994d96f05ba25a870ada70f47f315330437`.
The original methods were inspected directly using JDK21 `javap`. No REA MCP
execution or graphical-client observation is claimed; Java artifacts and raw
disassembly remain outside the repository.

| Original owner/method | Observed contract |
| --- | --- |
| `ServerPlayer.tick`, lines530–535 (`aqv.l`) | Broadcasts the actual current menu before testing validity and closing invalid access. |
| `LoomMenu.stillValid`, line137 (`cra.b`) | Uses captured ContainerLevelAccess with canonical Blocks.LOOM and the actual player. AbstractContainerMenu's helper checks the block before reach with additional range4.0. |
| `LoomMenu.removed`, lines283–285 (`cra.a`) | Calls AbstractContainerMenu.removed first, then clears the actual three-slot input container through captured Level access. |
| `AbstractContainerMenu.removed`, lines558–569 (`cpu.a`) | An alive connected player's cursor returns through the actual inventory; a dead/disconnected player's cursor is dropped. |
| `ServerPlayer.closeContainer`/`doCloseContainer`, lines1213–1223 | Server close sends the actual window close before removed, then transfers synchronization to retained InventoryMenu. |
| `ServerGamePacketListenerImpl.handleContainerClose`, lines1687–1689 | Closes the actual current menu; the incoming window does not select an old owner. |
| `LoomMenu$6.onTake`, lines102–118 | Consumes banner/dye, preserves the template, updates selection and invokes the time-gated actual take sound. It has no crafted, recipe or XP award callback. |

## Retained owners and API

`LivePlayers.looms` retains `OpenLoom { position, menu, context }`. The three
inputs are banner, dye and template. The result is derived; it is not another
owned input to return/drop. Cursor remains separate from window0's cursor.
There is no Loom inventory block entity. The actual world, player, inventory,
RNG, drop/statistic store and configured registry/tag/feature owners remain with
the existing Server/LivePlayers and primary native transaction.

`tick_native_loom_menu_for_player_locked(world,players,id,now,border)` executes
one actual menu phase: `LoomRequest::Broadcast`, fresh player/block access via
`main_loom_dispatch::loom_access`, kernel `still_valid`, then a real connected
server-side Close if invalid. It never uses the retained pickup context's old
access/time as a fresh validity observation. Block identity failure short-circuits
reach, including replacement and ordinary block removal.

Only typed `main_loom_dispatch::UnsupportedMenuReach` is retired as a bounded
native profile transition. An applicable equipped interaction-range modifier
can appear after opening. The helper conserves the actual cursor/inputs through
the primary ordered Close, sends the real server close, logs the explicit
native limitation and lets the level tick continue. Other errors propagate;
close must remain available without the unsupported attribute evaluator. This
does not implement arbitrary custom reach or invent a resulting range.

`close_native_loom(id,window,send_close)` acquires the outer chat operation guard
before clock/border, world and player ownership. The locked
`close_loom_with_context_locked(world,players,id,window,send_close,now,border,
context)` reacquires none of those guards and closes only the matching current
native owner. Cursor/input return, insertion packets, overflow/drop construction,
statistics and RNG belong exclusively to `loom_action_locked`.

## Central wiring and save/removal phases

Main must select a PLAY close's actual current menu atomically under one shared
operation, ignore the received window selection, and invoke the locked helper
with `send_close:false`. Cross-menu replacement uses the same real current owner
and connected context while retaining the already-held locks.

Actual dead/disconnected removal first removes retained InventoryMenu, then
closes the active Loom with `DeadOrDisconnected`, before saving/removing its
actor, RNG/drop/statistic or inventory owners. This preserves original cursor
and input drop order. The lifecycle does not silently substitute the connected
inventory-return path for that removal.

Ordinary autosave retains the current Loom and its owned inputs. Save/player
snapshots use `LoomRuntime::inventory_snapshot` to observe the actual external
cursor without treating the template, banner, dye or result as extra player
inventory. The existing explicit connected native checkpoint policy can call
`close_looms_for_save_locked` before a shutdown checkpoint. That policy is
distinct from ordinary autosave and from later dead/disconnected removal; it
is not a claim that vanilla periodic save forcibly closes its menus.

Required central changes are module/runtime assembly; exactly one menu tick;
click/button/current-close and cross-menu routing; central/legacy pickup
selection; save/snapshot/orphan checks; and real
InventoryMenu-first removal/shutdown wiring. The coordinator owns those files.
The Body pickup and real level-player external-owner suppression now include
Loom alongside the existing furnace/stonecutter/smithing owners. The fallback
requires a real menu callback; a captured pre-menu-tick owner prevents a second
window0 broadcast if validity closes the menu. No build or Main runtime flow
is asserted by this source-only handoff.
