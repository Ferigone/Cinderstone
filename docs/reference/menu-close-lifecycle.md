# Menu close and disconnect reference

This ledger records static Minecraft Java **1.21** source behavior for inventory,
crafting-table and chest-style menu removal. It does not report a native runtime
result, complete disconnect lifecycle support or full vanilla compatibility.

## Pinned source and inspection

The artifacts were recovered from the URLs pinned in
[`source-lock.json`](../../fixtures/source-lock.json), with these hashes checked:

- Outer server JAR SHA-1: `450698d1863ab5180c25d7c804ef0fe6369dd1ba`.
- Inner server JAR SHA-256:
  `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
- Official server mappings SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`.

The relevant methods were inspected with JDK 21 `javap -p -c` and the official
mappings. This is direct bytecode inspection, separate from the earlier REA
archive inventory. Proprietary binaries and raw disassembly are not published.

| Mapped class | Original name | Relevant mapped source lines |
| --- | --- | --- |
| `AbstractContainerMenu` | `cpu` | `removed` 558–569; `clearContainer` 572–586 |
| `InventoryMenu` | `cqw` | `removed` 133–142 |
| `CraftingMenu` | `cqm` | `removed` 124–126; its lambda at 125 |
| `ChestMenu` | `cqc` | `removed` 108–110 |
| `Inventory` | `cmw` | `placeItemBackInInventory` 302–323 |
| `ServerPlayer` | `aqv` | `disconnect` 1374–1379; `hasDisconnected` 1382 |
| `Player` | `cmx` | `remove` 1450–1456; `hasContainerOpen` 1485 |
| `PlayerList` | `aur` | `save` 364–373; `remove` 381–406 |

## The cursor belongs to the removed menu

`AbstractContainerMenu.removed(player)` handles a nonempty cursor for a
`ServerPlayer`. It evaluates `!player.isAlive() || player.hasDisconnected()`.
The disconnected state is the explicit `ServerPlayer.db` flag read by `w()`
and set by `v()`; neither a client menu-close packet nor suppression of an
outgoing close packet supplies that state.

When dead or disconnected, it calls `Player.drop(cursor, false)`. That overload
forwards `around=false` and `traceItem=false`. Empty stacks create no Item
entity. For an alive, connected server player, it instead calls
`Inventory.placeItemBackInInventory(cursor)`. Finally it clears the old cursor.
This callback does not itself broadcast the inventory menu.

## Temporary crafting inputs and owned block inventories differ

`InventoryMenu.removed` first invokes the inherited cursor callback, then clears
its result container. On the server it calls `clearContainer` for its **four
input slots**. The result is a derived preview, not a fifth owned input stack.

`CraftingMenu.removed` first invokes the inherited cursor callback, then uses
its `ContainerLevelAccess` callback to clear the **nine input slots**. It does
not return or drop its result preview as another item.

`clearContainer` visits the actual temporary container indices in ascending
order, removing each stack with `removeItemNoUpdate`. For a dead player, or a
disconnected server player, each stack follows `Player.drop(stack, false)`.
For an alive, connected server player each stack follows
`Inventory.placeItemBackInInventory`. The cursor is therefore handled before
the inputs of the menu whose `removed` callback is currently executing.

`ChestMenu.removed` calls the inherited cursor callback and then the actual
container's `stopOpen(player)`. It does **not** clear, return or drop the
chest-style container contents. Closing a menu and destroying its owning block
are separate actions.

## Returning a stack to inventory is an explicit packet-producing operation

The one-argument `Inventory.placeItemBackInInventory` enables its packet flag.
While the stack is nonempty, it searches for remaining merge space in this order:

1. The selected hotbar slot.
2. Raw offhand slot **40**.
3. Raw main-inventory indices **0–35**, ascending.

Merge candidates must be nonempty, stackable, match item and components, and
have remaining capacity. If none matches, it selects the first empty raw
main-inventory slot, again from **0–35**. Armor is not searched for free slots.
If no destination exists, the remaining stack uses `Player.drop(stack, false)`.

Otherwise it splits according to the source stack's maximum size minus the
destination's current count and performs `Inventory.add(rawIndex, split)`.
After a successful insertion, with packet delivery enabled and a server-player
owner, it sends `ClientboundContainerSetSlotPacket` with window **-2**, state ID
**0**, the **raw inventory index**, and that destination's current stack. It
then repeats until the original stack is empty. An ordinary menu-slot update or
a single final full-menu synchronization is not this insertion packet sequence.

## Full disconnect ordering remains a separate integration boundary

`ServerGamePacketListenerImpl.removePlayerFromWorld` sets the player's explicit
disconnected state before calling `PlayerList.remove`. The latter saves the
player before its later removal from the level.

The inherited `Player.remove` invokes `inventoryMenu.removed(player)` first.
If an additional current menu is open, it subsequently invokes
`doCloseContainer` for that actual menu. Retained 2×2 inputs therefore precede
the current external menu's cursor/grid in this disconnect sequence. This
matters for Item construction, shared random consumption and publication order.

A bounded cursor/grid disposition fix does not by itself establish the complete
Java player-save order, every asynchronous disconnect race, player death and
respawn, or all workstation callbacks. Live menu closure and live-player save
cleanup must not be reclassified as disconnected merely because they omit an
outgoing close packet or run during shutdown. See also the
[furnace menu lifetime reference](furnace-menu-lifetime.md).

## Opening a block menu does not remove the inventory menu

`ServerPlayer.openMenu(MenuProvider)` (`aqv.a(bqv)`, mapped lines 1153–1173)
compares the current `containerMenu` object with `inventoryMenu`. It invokes
`closeContainer` only when they are different. If the inventory menu is current,
opening a block menu does **not** invoke `InventoryMenu.removed`, clear its 2×2
inputs, return its carried stack or publish cursor-return insertion packets.
The old inventory menu can consequently retain its own temporary inputs and
cursor while the newly created external menu owns a separate cursor.

When a previous external menu is current, `closeContainer` sends its close
packet before `doCloseContainer` invokes its removed callback, transfers shared
slot synchronization state to the inventory menu and selects the inventory
menu. This is followed by allocation of the next container ID and invocation of
the new provider. For a successfully created menu, `openMenu` sends its open
screen packet, initializes its synchronization/listeners and then assigns it as
the current menu. A provider returning no menu does not restore the old external
menu. A null provider returns immediately before the close comparison.

An unconditional inventory-menu close before opening a block menu changes this
ownership lifecycle. In particular it prevents the retained 2×2-input and old
inventory-cursor case that the later `Player.remove` ordering must handle.
