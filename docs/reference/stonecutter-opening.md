# Native stonecutter opening and dispatch

Target: the pinned original Minecraft Java 1.21 server, protocol 767. Server
SHA-1 is `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, official mappings SHA-1
is `31c77994d96f05ba25a870ada70f47f315330437`, and extracted inner-server
SHA-256 is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
This stage used direct JDK21 `javap -p -c` and the official mappings, without
claiming an REA MCP execution. Raw disassembly stays outside the repository.

## Original contracts

| Original method | Observed contract |
| --- | --- |
| `StonecutterBlock.useWithoutItem` (`dnw.a`, source lines 52–59) | On the server, opens the actual block's menu provider, then awards `INTERACT_WITH_STONECUTTER`. This is not a block entity inventory. |
| `StonecutterBlock.getMenuProvider` (`dnw.b`, line 65) | Creates `StonecutterMenu` with the actual level/position `ContainerLevelAccess` and the `container.stonecutter` title. |
| `ServerPlayerGameMode.useItemOn` (`aqw.a`, lines 327–380) | Spectators take the menu-provider branch before ordinary hand/sneak suppression. Ordinary default-block interaction is reached only from the main hand and is suppressed by secondary use when either hand is nonempty. A consumed default-block interaction triggers `DefaultBlockUseTrigger` after the block callback. |
| `ServerPlayer.openMenu` (`aqv.a`, lines 1153–1173) | Closes the current external menu, increments the player's window counter, constructs the menu, sends OpenScreen, initializes its listener/synchronizer and then installs it as the current menu. It does not remove the retained InventoryMenu just to open an external menu. |
| `ServerPlayer.initMenu` (`aqv.a`, lines 488–490) | Installs the local container listener before the synchronizer. |
| `ServerPlayer$2.slotChanged` (`aqv$2.a`, lines 278–287) | Skips result slots, then invokes the inventory-change criterion only when the slot's backing owner is the actual player inventory. This includes inventory-backed slots in an external menu. `dataChanged` has no behavior in this listener. |
| `AbstractContainerMenu.stillValid` default access and `StonecutterMenu.stillValid` | Tests the captured stonecutter block and the actual player's block-interaction range with an additional four blocks. |

## Native ownership

`src/main_stonecutter_dispatch.rs` provides these Server entry points:

- `use_native_stonecutter(id, hand, target)` resolves the actual world block,
  live actor, hands, crouch state and initial reach. It closes existing external
  menus through their real locked removal owners and allocates a window from the
  existing shared player window counter. The retained InventoryMenu cursor/grid
  remain with that menu.
- Opening publishes OpenScreen before calling the new menu's initializer. The
  initializer's inventory-backed slot callbacks are mapped into the actual
  gameplay owner using each `StonecutterMenuEvent` phase's observed inventory,
  prepared and committed in event order before the corresponding packet
  publication. Initialization is synchronization-only; an unexpected mutation
  event is rejected instead of silently ignored. The initialized menu retains
  its real baselines and cursor.
- Ordinary opening then increments the actual
  `minecraft:interact_with_stonecutter` statistic. Spectator opening uses the
  provider path, with neither that block statistic nor a default-block-use
  callback. Its click behavior is owned by the shared action module.
- `click_native_stonecutter(id, click)` and
  `stonecutter_button(id, window, button)` select only the real current matching
  window and delegate the actual transaction to `stonecutter_action_locked`.
- `stonecutter_access(world, player, position)` captures the real block, pose
  eye position and interaction range while the owning locks are held. It
  implements the kernel's `WorkstationAccess`; no dummy block entity, player or
  world is constructed. `stonecutter_still_valid` exposes the same predicate for
  the lifetime owner.

The recipe/callback bridge is in `main_stonecutter_recipes`, the transaction
owner in `main_stonecutter`, and close/tick adapters in
`main_stonecutter_lifecycle`. World → players → shared inventory remains the lock
order within an operation. The standalone stonecutter opening/click/button
wrappers and existing crafting-table/furnace/chest-barrel openers acquire the
shared `chat_dispatch` operation guard before those locks. Locked inner removal
helpers never reacquire it. Opening checks the actual actor through the shared
native-body/lifecycle guard before closing an old menu or publishing OpenScreen.
No opening wrapper reacquires the async world/player locks while an old menu is
being removed.

The supported source remains the actual fresh native fixture with its certified
empty advancement/objective registry. The `DefaultBlockUseTrigger` boundary is
invoked through `MainItemGameplay.default_block_use` after reading the actual
post-use block and awarding the block interaction statistic. That owner verifies
the active UUID and its empty registrations; nonempty/custom advancement
registrations are unsupported. Recipe feature
checks, crafted-result callbacks, actual item drops and sound publication belong
to the transaction/callback owners and are not implied by menu opening alone.

## Main routing contract

The integration coordinator owns module declarations, `LivePlayers.stonecutters`
and the existing Main dispatch functions. Required routes are actual block use
to `use_native_stonecutter`, PLAY ContainerClick to
`click_native_stonecutter`, and the two-VarInt ContainerButtonClick payload to
`stonecutter_button`. Actual server-window close, cross-menu opening, player
tick, pickup, persistence and disconnect routes are separate lifetime contracts.
This source stage alone does not claim those Main routes or a whole checklist
delivery. Focused compiler/runtime evidence is recorded by the coordinator after
the shared modules are composed.
