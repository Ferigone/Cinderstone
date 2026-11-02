# Native smithing-table opening and dispatch

Target: Minecraft Java 1.21, protocol 767. The original server SHA-1 is
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA-1 is
`31c77994d96f05ba25a870ada70f47f315330437`; extracted inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
This stage used direct JDK21 `javap -p -c` and bootstrap-method inspection of
those original classes. No REA MCP execution is claimed; raw disassembly and
proprietary Java artifacts remain private.

## Original contracts

| Original method | Observed behavior |
| --- | --- |
| `SmithingTableBlock.getMenuProvider` (`dnf.b`, source line 33) and its provider lambda | Creates `SmithingMenu` using the actual level/position `ContainerLevelAccess`. Its title is **`container.upgrade`**. No block entity owns its temporary inputs. |
| `SmithingTableBlock.useWithoutItem` (`dnf.a`, lines 38–44) | Opens that provider, then awards `INTERACT_WITH_SMITHING_TABLE` on the server. |
| `SmithingMenu` constructor (`crt`, lines 41–44) | Supplies the actual SMITHING MenuType, retains its actual level and obtains the smithing recipe list from that level's RecipeManager. The native pinned-context owner supplies the captured corresponding input/result orders. |
| `ServerPlayerGameMode.useItemOn` (`aqw.a`, lines 327–380) | The spectator provider path precedes ordinary hand/sneak handling. Ordinary default-block interaction requires the main hand and is suppressed by secondary use when either hand holds an item. Its consumed result triggers the default-block-use criterion after the block callback. |
| `ServerPlayer.openMenu` and `initMenu` (`aqv.a`, lines 1153–1173 and 488–490) | Removes the current external menu, advances the shared window counter, constructs the new menu, sends OpenScreen, installs the local listener before the synchronizer, then makes the menu current. The retained InventoryMenu cursor/grid are not removed by opening an external menu. |
| `ServerPlayer$2.slotChanged` (`aqv$2.a`, lines 278–287) | Invokes the actual inventory-change criterion for non-result slots backed by the real player inventory. It is not restricted to InventoryMenu. |
| `EquipmentSlotGroup` (`bsz`, lines 13–29, 63) and `EquipmentSlot.isArmor` (`bsy.f`) | Apply attribute modifiers to their declared actual equipped slots. HAND includes both hands; ARMOR includes humanoid and animal body armor. The native decoded component strings use `main_hand`/`off_hand`. |

## Native API

`src/main_smithing_dispatch.rs` exports:

- `Server::use_native_smithing_table(id, hand, target) -> Result<bool>` resolves
  the actual block, actor, hands, secondary use and reach. It uses the actual
  native source/body/feature guards before closing an old menu or publishing
  OpenScreen. Captured recipe order comes from `NativeSmithingRecipes`, which
  delegates to `PinnedDefaultRecipeContext`; recipe IDs are not re-sorted here.
- Opening closes the actual current smithing/stonecutter/crafting/furnace or
  chest/barrel menu through its locked lifecycle owner. It allocates from the
  existing player window counter, uses kernel `WorkstationKind::Smithing`
  (protocol MenuType 21) and the source's `container.upgrade` title.
- OpenScreen is published before `SmithingMenu.initialize`. Each phased
  initialization listener executes through the actual gameplay owner with its
  own `SmithingPhase.player_inventory`, then the corresponding packet is
  published in event order. Unexpected mutation events in this initialization
  path fail instead of becoming an omitted callback.
- The initialized owner is installed before awarding the real
  `minecraft:interact_with_smithing_table` statistic and invoking the actual
  default-block-use bridge after reading the post-use block. Spectators take
  the readonly provider branch and receive neither of those block callbacks.
- `Server::click_native_smithing_table(id, click) -> Result<bool>` resets the
  actual last-action owner before matching the current window, then delegates
  to the canonical `smithing_action_locked` transaction owner. Smithing has no
  recipe-selection button handler in this adapter.
- `smithing_access(world, player, position)` implements the kernel's
  `WorkstationAccess` using the actual captured block and actor. Eye/range
  evaluation remains lazy so a replaced block short-circuits first.

The actual equipped `block_interaction_range` modifier (attribute 6) is an
explicit unsupported context until its attribute composition owner exists.
`require_smithing_reach_profile` checks canonical equipment and actual component
12 slot-group applicability before the initial default-range query. A modifier
on an unheld stack or an inapplicable equipped slot group does not fail this
guard; a matching equipped modifier is not assigned an invented resulting
attribute value. The primary actor guard reuses this capability check for later
menu operations. Only this applicable-modifier condition returns the typed
`UnsupportedMenuReach` error. The player-menu lifetime owner may retire that
actual menu through its ordered connected-player close path, conserving cursor
and inputs, when equipment changes after opening. Malformed state and other
transaction failures retain their distinct error paths. This retirement is an
explicit native unsupported-profile boundary, not vanilla attribute support.
The matching click wrapper catches only that same typed boundary under its
already-held operation/world/player guards, closes the actual menu with a server
close packet and preserves the PLAY session. It therefore covers a click queued
before the next player-menu tick without replacing another error with success.

Standalone wrappers acquire `chat_dispatch` before world → players → shared
inventory. Locked inner removal helpers never reacquire the operation guard.
The fresh native fixture's actual empty advancement/objective registry is the
supported callback policy; nonempty/reentrant/custom contexts are not executed
against snapshots or replaced by successful no-op callbacks.

## Integration boundary

The coordinator owns adding Main module exports, `LivePlayers.smithing_tables`,
block-use/ContainerClick routes, cross-opening closure from existing menus,
actual player tick and immediate pickup synchronization, persistence and
disconnect cleanup. This source module is prepared separately while the prior
stonecutter delivery is published. It does not itself prove these Main routes,
a socket flow or completion of an aggregate checklist item. Compiler/runtime
validation is recorded by the coordinator after the stage is composed.

The prepared cross-opening patch adds current-smithing retirement to the
crafting-table, furnace, chest/barrel and stonecutter openers, after their target
preflight and before a replacement window is allocated. It reuses the locked
context-aware smithing removal owner. Alive InventoryMenu cleanup also requires
the smithing owner to be closed, while disconnect removal preserves its original
retained-InventoryMenu-first exception. These patches do not close InventoryMenu
just to open another external menu.
