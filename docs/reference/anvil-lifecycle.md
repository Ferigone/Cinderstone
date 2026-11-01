# Native Main anvil lifecycle contract

Target: Minecraft Java1.21, protocol767. Primary evidence is direct JDK21
`javap -p -c` against the unchanged inner server JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
with official mapping SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
This is actual bytecode inspection, not REA or an observed Java gameplay flow.
Inspected methods are ServerPlayer (`aqv`) tick/closeContainer/doCloseContainer,
AbstractContainerMenu (`cpu`) removed/clearContainer, and ItemCombinerMenu
(`cqx`) removed/stillValid. Raw Java artifacts are not redistributed.

## Source phase and real owners

`ServerPlayer.tick` broadcasts the actual current menu (`aqv.l`, offset45)
before checking its actual stillValid result (offset63) and closing an invalid
menu (offset70). `ServerPlayer.closeContainer` sends the current window's close
packet before `doCloseContainer`. The latter calls removed, transfers old menu
inventory synchronization baselines into the retained InventoryMenu, then makes
that retained InventoryMenu current. A fresh InventoryMenu is not constructed.

Anvil removal handles the source cursor first, then the two owned input stacks
in container order. Alive connected players use actual inventory return and
its overflow drops. Dead or disconnected players use actual Player.drop with
traceItem=false. The derived result preview is discarded: it is not a third
owned input or a second result. Returning/dropping inputs invokes no result
take, level cost, anvil wear or fabricated crafting callback.

`src/main_anvil_lifecycle.rs` owns only the lifetime orchestration. The runtime
in `main_anvil` owns the retained non-Clone `AnvilMenu<NativeAnvilBinding>` and
its real transaction. Context owns actual item/text/player/world effects;
dispatch owns actual anvil-tag/reach observations and packet entry; immediate
pickup owns its source synchronization phase. This module supplies no substitute
world, RNG, inventory, health flag or block entity. Anvils have no menu-input
block entity.

## Exact Root hooks

All locked methods take the existing `world -> players -> shared inventory`
sequence and do not reacquire world/player locks. Async close first takes the
existing chat sequencing guard, then those same actual owners.

- `main_anvil_lifecycle::connected_anvil_close_context(players, id)` is exported
  within Main for connected cross-open/current-close callers. It observes the
  same actual finite health and matching physical identity/removed owners as
  the lifecycle; it supplies no default when an owner is missing. The caller
  establishes connection lifetime, and disconnect/removal supplies its explicit
  `DeadOrDisconnected` context instead.
- `Server::close_native_anvil(id, window, send_close)` handles a connected-player
  close using the actual retained window. It reads actual metadata health and
  native physical removed state with matching entity ID/UUID; a missing or
  mismatched owner fails rather than assuming alive. The connected caller
  establishes the connection phase.
- `Server::close_anvil_with_context_locked(world, players, id, window,
  send_close, now, border, context)` closes only that captured actual menu.
  Disconnect/removal supplies `DeadOrDisconnected` explicitly **before**
  removing player/inventory/RNG owners, even while a registry record still
  exists. The client packet's window number is not an alternate state owner.
- `Server::tick_native_anvil_menu_for_player_locked(world, players, id, now,
  border)` runs once in that actual player's menu phase. It invokes the runtime
  Broadcast request first, then actual actor-profile admission, then borrows the
  retained position and real player for dispatch's actual anvil-tag/block/reach
  check. Unsupported profiles and invalid access use the real close owner.
  Root must not also add a second whole-runtime tick for the same menu.
- `Server::close_anvils_for_save_locked(world, players, now)` is shutdown-only
  retirement. It is deliberately a no-op during ordinary live autosave. During
  shutdown it follows actual player-registry save order, rejects orphan menu
  owners and returns/drops each actual cursor/input set before player capture.

After a successful actual Broadcast, the lifecycle also invokes dispatch's
actual `require_anvil_actor`. A later creative-to-survival/spectator, effect,
mount, removed-body or other admitted-profile change can leave block validity
true; this bounded runtime retires that unsupported actor through the same real
close transaction. This is explicit native profile retirement, not a fabricated
broader player implementation or a new spectator rename rule. Connected health
and entity lifetime select return versus drop; unavailable required close owners
propagate an error while the retained menu remains owned. Only the admission
predicate is caught here; arbitrary Broadcast/kernel/world errors still fail.
Root's central game-mode/effect/mount/death seams must preserve their actual
incoming owners and route their source close context before owner deletion.

A typed `main_stonecutter_dispatch::UnsupportedMenuReach` from an anvil
Broadcast identifies the existing bounded equipped-attribute profile boundary.
The lifecycle retires that actual menu through its normal close transaction;
it does not fabricate an attribute value, reset UI state or swallow unrelated
transaction errors. Changed captured windows fail the retirement guard.

The source close packet, inventory insertion/overflow packets, actual item
construction and RNG/entity-allocation work belong to
`Server::anvil_action_locked(AnvilRequest::Close { ... })`. That action must
stage and publish all actual owners atomically, retaining close-before-removal
and baseline-transfer-after-removal order. This lifecycle module does not
replay callbacks or perform a second item transfer after the action returns.

## Save snapshot, ordinary autosave and shutdown

`NativeAnvilRuntime::inventory_snapshot(id, inventory)` returns the existing
detached native player-save observation with the actual active anvil cursor.
The underlying canonical inventory slots, selected hotbar, retained window0
cursor and its2x2 grid are not mutated. This read-only snapshot is not a live
inventory owner, input transfer or menu-close operation. Root composes it into
`capture_player_locked` using the already captured canonical inventory.

Ordinary autosave leaves the actual anvil menu, cursor, inputs, preview and
sync state alive. It must not call an input drain or replace live inventory
with a snapshot. Temporary anvil inputs are not smuggled into retained window0
slots or represented by the result preview. This stage adds no durable
serialized temporary-menu checkpoint or restart reconstruction.

Shutdown retirement instead closes the actual external menu while real player,
world, item/RNG and connection owners are still available, before capturing the
player save. Root then runs the retained InventoryMenu's own source removal
phase. Its separately owned cursor/grid must not be reset by anvil open/close.
The disconnect caller supplies its real disconnected context; health is not a
substitute for connection ownership. Death handling similarly preserves the
actual dead/drop branch.

## Verification and remaining wiring

The lifecycle source is formatted and its API is coordinated with the runtime,
context and dispatch owners. The following narrow command compiled the actual
five new modules and executed the lifecycle regression successfully in Root's
external nonproduction composition `/tmp/ws-anvil-main-probe-20261101a`, based on
`41cd850e6cae926b168be211f26b393f510f7513`:

```text
CARGO_TARGET_DIR=/workspace/cinderstone-workstations-20261101a/target \
  cargo test --locked --bin mc121-rust main_anvil_lifecycle::tests -- --nocapture
```

That composition links these actual modules and supplies the proposed pure
`validate_active_player`/`preflight_inventory_slot_changes` gameplay methods in
its external copy. The reserved repository Main/gameplay files were not edited
by this owner. The executed regression uses actual native registry, lifecycle,
canonical inventory and physical-state constructors, with dimension facts from
the pinned public configuration capture. It checks creative/survival/spectator,
effect, mount, removed and zero-health admission inputs and real close-context
selection, including refusal with missing physical ownership. It does not
execute a Server tick or publish a close transaction. No mock world, menu clone,
direct XP increment or synthetic RNG fallback is used.

Root owns `main.rs`, LivePlayers.anvils, module exports, per-player body/rain
phases, cross-menu opens, close/disconnect/death dispatch, save snapshot/shutdown
calls and collective verification. No production Main invocation is claimed
from this lifecycle file alone. These bounded hooks do not complete the fixed
all-menu/workstation rows or establish full vanilla compatibility.
