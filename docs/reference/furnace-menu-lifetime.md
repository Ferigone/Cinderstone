# Furnace menu lifetime reference

This ledger describes Minecraft Java **1.21** behavior used for removing a
furnace while its menu is open. It establishes static source behavior, not a
successful execution of the native implementation or full vanilla parity.

## Pinned reference

- Outer server JAR SHA-1: `450698d1863ab5180c25d7c804ef0fe6369dd1ba`.
- Inner server JAR SHA-256:
  `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
- Official server mappings SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`.
- The artifacts were recovered from the exact URLs in
  [`source-lock.json`](../../fixtures/source-lock.json), their hashes checked,
  and methods read with JDK 21 `javap -p -c`. Java binaries and raw disassembly
  are not redistributed in the repository.
- Relevant classes: `ServerPlayer` (`aqv`), `ServerGamePacketListenerImpl`
  (`aru`), `AbstractContainerMenu` (`cpu`), `AbstractFurnaceMenu` (`cpv`),
  `Container` (`bqk`), `BaseContainerBlockEntity` (`dqb`), `Inventory` (`cmw`),
  and `AbstractFurnaceBlock` (`det`).

## Ownership survives removal from the level

`AbstractFurnaceMenu` holds the actual `Container` and `ContainerData` objects.
The block's removal first drops the container contents, then emits recorded
recipe XP, then removes the world block entity and notifies comparator output.
It does not close every viewing menu synchronously.

`Containers.dropContents` splits the existing stacks. It does not call
`AbstractFurnaceBlockEntity.setItem`, reset cooking or fuel counters, or clear
`RecipesUsed`. Therefore an existing menu still references the removed old
container, whose three stacks have become empty while its counters and used
recipes remain. This object lifetime is distinct from world ownership.

`AbstractFurnaceMenu.stillValid` delegates to its actual container.
`BaseContainerBlockEntity.stillValid` uses `Container.stillValidBlockEntity`:

1. A null level returns false.
2. `level.getBlockEntity(position)` must be the **same object** as the captured
   block entity. Matching coordinates, block type or contents are insufficient.
3. The player must pass `canInteractWithBlock(position, 4.0)`.

The last check uses the player's actual block-interaction-range attribute plus
four, eye position, squared distance to the block AABB and a strict `<` limit.
Replacing a removed furnace at the same position cannot revive an old menu.

## The normal close phase is the level's player tick

`ServerPlayer.tick` (`aqv.l`, mapped source lines 522–564) performs the menu
portion in this order:

1. `containerMenu.broadcastChanges()`.
2. On the server, evaluate `containerMenu.stillValid(player)`.
3. When invalid, call `closeContainer()` and select the inventory menu.

This is the player's **level entity tick**, after its game-mode/warden tracker
updates and counter decrements. It is not the later connection-phase
`ServerPlayer.doTick` (`aqv.m`), and it is not the furnace block-entity tick.
Within the ordinary level lifecycle the entity ticks precede block-entity
ticks. The final emptied slots can consequently be broadcast before the close
packet, using the old object's retained counter values.

## Closing returns each menu's own cursor once

`ServerPlayer.closeContainer` (`aqv.s`) sends the clientbound close packet
first, then invokes `doCloseContainer` (`aqv.t`). That method:

1. Calls `oldMenu.removed(player)`.
2. Transfers shared-slot synchronization state from the old menu to the
   inventory menu.
3. Assigns the inventory menu as the current menu.

`AbstractContainerMenu.removed` returns only its carried stack. For an alive,
connected server player it calls `Inventory.placeItemBackInInventory`; for a
dead or disconnected player it uses the actual player-drop path. It then sets
the old menu's carried stack to empty. The furnace's three slots are not
returned a second time. A full inventory can make the cursor remainder enter
the real player-drop path.

`Inventory.placeItemBackInInventory(stack)` enables its packet flag. For each
successful insertion it sends `ClientboundContainerSetSlotPacket` with window
`-2`, state ID `0`, the raw inventory slot index and that slot's current stack.
The slot-baseline transfer matches the actual container object and slot index;
it does not copy the old menu's cursor into the inventory menu.

`doCloseContainer` returns after that transfer and assignment. It does not
immediately call the inventory menu's `broadcastChanges`. The transferred old
local and remote slot baselines can therefore still differ from cursor-return
contents until the next actual broadcast invocation. `broadcastFullState`
updates local slot/listener state before calling `sendAllDataToRemote`; the
latter alone updates only the remote slot, carried and data baselines.

## Item pickup broadcasts the current menu immediately

`ItemEntity.playerTouch` calls the player's `take(entity, originalCount)` after
a successful `Inventory.add`. `ServerPlayer.take` first invokes the inherited
collect-packet behavior, then immediately calls the **current**
`containerMenu.broadcastChanges`. This precedes the item's discard path,
picked-up statistic and `onItemPickup` callback. When a furnace menu is current,
that broadcast belongs to the furnace menu; it is not an unconditional window-0
inventory broadcast. Once the furnace menu has closed, the current inventory
menu is the appropriate broadcast owner.

## Late packets

`ServerGamePacketListenerImpl.handleContainerClick` (`aru.a(agx)`, mapped
source lines 1693–1737) first ignores a mismatched window ID. A spectator is
resynchronized and returns before ordinary validity checking. For a normal
player whose current menu is no longer valid, it logs and **returns without
clicking, closing or resynchronizing**. The later normal player level tick
performs the invalid-menu close. Slot-index and state-ID handling are reached
only after validity succeeds.

`handleContainerClose` (`aru.a(agy)`, mapped lines 1687–1689) invokes the current
player's `doCloseContainer`; it does not compare the packet's window ID. This
is a separate client-requested close path and does not send a server-initiated
close packet first.

These observations require native integration to retain the old container
identity and each viewer's cursor independently, reject late invalid clicks,
and run broadcast/validity/close at the actual level player phase. They do not
establish complete support for survival, disconnected-player drops, all
workstations, arbitrary callbacks or every packet race.
