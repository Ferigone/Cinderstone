# Actual Main anvil dispatch

The exact central grant is issue 3 comment `6564589182`. This module is new
`src/main_anvil_dispatch.rs` in the separate anvil-Main worktree, based on
RootStoneMain `dddc0e7`. It does not edit the existing main namespace, decoder,
player fields, competing menu open paths or deployment. Those exact callsites
remain central wiring until added and compiled together.

Primary evidence is the pinned original Minecraft Java 1.21 inner server JAR,
SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
with official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Direct JDK 21 `javap -p -c` inspection covered `AnvilBlock` (`dey`),
`ServerGamePacketListenerImpl.handleRenameItem` (`aru.a(ahr)`), the rename
packet (`ahr`), and the prior inspected `cpw`/`cqx` menu source. This is bytecode
evidence, not REA or original-Java gameplay execution. No Java artifacts or
raw disassembly are committed.

## Owned dispatch and actual owners

`use_native_anvil(id, hand, target)` executes under the existing chat-operation,
world, player and then canonical SharedContainer ownership interval. It checks
the actual target against the pinned native anvil tag, actual equipped reach
attributes, alive native creative Overworld/default-effect/unmounted body,
interaction reach, source main-hand and secondary-use suppression. It checks the
real gameplay empty-criteria capability and canonical inventory component
admission. It never synthesizes a player, recipe, advancement, XP or RNG owner.
Unsupported imported gameplay, abilities, body or active range modifiers remain
explicit errors before opening. No broader player-mode coverage is inferred.

The actual current external menu is selected with a uniqueness check and closed
through its real retained owner. The native shared window allocator supplies the
next window. If InventoryMenu is current, opening does **not** invoke its
`removed` callback: its window-0 cursor and 2x2 inputs remain distinct from the
new external cursor. The runtime's `open_anvil_locked` is the sole opening
publisher. It must enqueue OpenScreen type **8**, title `container.repair`, before
`initMenu`'s actual local listener/synchronizer sequence. Dispatch neither creates
a second menu nor emits a duplicate screen/content packet.

After successful runtime opening, dispatch increments the actual
`minecraft:interact_with_anvil` statistic, then queries the post-use block and
invokes the actual `DefaultBlockInteractionTrigger` empty-listener branch.
This preserves `dey.useWithoutItem`'s open-before-stat order and the enclosing
GameMode's post-use criterion phase. The registry certificate is the real
`MainItemGameplay::require_supported_floor`, held stable by the same locks; a
missing advancement owner is not treated as an empty registry.

`ActualAnvilAccess` borrows captured actual block, position, feet, pose and game
mode. The source menu validity calls the actual anvil tag predicate and
`player.canInteractWithBlock(pos, 4.0)`. Default creative/noncreative base ranges
come from the shared native geometry constants only after equipped range
modifiers are admitted. No constant eight-block centre-distance replacement is
used. The runtime/lifecycle can call `anvil_access` or `anvil_still_valid` on the
same captured position; no block entity is invented for anvils.

## Wire dispatch and retained recovery

`click_native_anvil(id, click)` executes only when the packet window equals the
actual retained anvil window. While an anvil is current, a wrong-window packet
is consumed as an ignored action and cannot fall through to mutate retained
InventoryMenu. It updates the actual connection last-action owner and delegates
matching actions to `AnvilRequest::Click`; the runtime owns validity,
spectator policy, slot bounds and authoritative state/cursor/prediction handling.
`rename_native_anvil(id, name)` routes only when an actual anvil is active and
passes a lossless JavaString to `AnvilRequest::Rename`. The rename packet has no
window or click state ID. `decode_anvil_rename` enforces the source wire
readUtf limit of 32767 UTF-16 units and complete payload consumption; core
`setItemName` later filters disallowed characters and applies the separate
50-unit rule. `aru.handleRenameItem` adds **no** last-action reset or unconditional
broadcast, so this wrapper adds neither.

`resync_native_anvil_locked` delegates to `AnvilRequest::Resync` on the real
retained owner. Recovery must invoke full synchronization while preserving
inputs, derived result, name, cursor, drag state and UI identity. A fake click,
replacement initialization or window-0 packet is not a recovery path. Actual
close remains the source removal transaction and must return/drop owned cursor
and inputs once, with preview excluded. Host callbacks, player counters, item
and world RNG, drop creation and source-ordered packet publication belong to
`main_anvil`/`main_anvil_context`, not these wire wrappers.

For a failed atomic click/rename candidate, the bounded native refusal policy
records the reason and invokes that retained full-state recovery. A changed
equipped reach profile retires the actual menu through the existing lifecycle,
using the real connected player's health/entity lifetime to choose return or
drop. If the required recovery/close owners are unavailable, the error remains
explicit and the failed candidate stays uncommitted. This is a native supported-
context boundary, not an invented successful vanilla take or error packet.

## Exact central wiring

The central owner must add these callsites before claiming production use:

- Declare `mod main_anvil_dispatch` alongside the granted runtime/context/
  lifecycle/pickup modules. Add `players.anvils: main_anvil::NativeAnvilRuntime`
  with its real default constructor, as owned by the central/runtime work.
- In actual use-on-block routing, after packet/teleport admission and before
  generic item/block fallback, call `use_native_anvil`; acknowledge the source
  sequence once when handled.
- In container-click routing, after shared prediction-component admission,
  try `click_native_anvil` before InventoryMenu fallback.
- Decode ServerboundRenameItem using `decode_anvil_rename` and call
  `rename_native_anvil`; there is no invented packet window/state or extra
  broadcast. The actual current menu determines whether it is handled.
- Add anvil to the current-menu uniqueness checks for close and retained
  full-state recovery, invoking `resync_native_anvil_locked` and the granted
  actual lifecycle close helper. Add actual anvil retirement to all other
  external-menu opening paths so two canonical current menus cannot coexist.
- Compose actual player-phase broadcast-before-validity, disconnect/death/save
  removal, save cursor observation and immediate pickup sync through the
  separately granted runtime/lifecycle/pickup modules. Do not reenter the world
  or player locks from a callback already inside that interval.

The callable runtime contract is `open_anvil_locked(world, players, id,
position, window, now, border)` and `anvil_action_locked(world, players, id,
AnvilRequest, now, border)`, using the actual `NativeAnvilRuntime` owner. The
request variants are Click, Rename, Broadcast, Resync and source-context Close.
The runtime must complete all fallible Item/XP/world/drop preparation before
its single canonical publication. Packet draining, callbacks that reenter or
saves cannot observe an uninstalled retained UI.

Compiler/runtime verification and the exact actually connected Root callsites
will be recorded after the companion runtime/context/namespace wiring is ready.
This dispatch slice does not complete aggregate `inventory.4`, `crafting.2` or
`crafting.5`, nor full native anvil compatibility.
