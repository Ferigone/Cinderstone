# Beacon menu reference ledger

This stage ports the temporary payment and borrowed-data contract of the Java
1.21 beacon menu. It does not implement a beacon block entity, structure/beam
scanning, potion effects or a Main callsite, and does not complete an aggregate
workstation checklist delivery.

## Pinned primary evidence

Inspected with JDK 21 `javap -p -c` and the official mappings:

- Original inner server JAR SHA-256:
  `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
- Official server mappings SHA-1:
  `31c77994d96f05ba25a870ada70f47f315330437`.
- `BeaconMenu` (`cpy`), constructor 30–72, `removed` 76–85,
  `setData` 94–96, `quickMoveStack` 100–139, effect encode/decode
  147–152 and `updateEffects` 166–172; payment slot `$a` and temporary
  container `$1`.
- `BeaconBlockEntity.createMenu` 354–357,
  `BaseContainerBlockEntity.canUnlock` (`dqb`) 83–89.
- `BeaconBlockEntity$1` (`dqc$1`), actual ContainerData getter/setter
  92–116, and `BeaconBlockEntity.filterEffect` 122.
- `ServerGamePacketListenerImpl.handleSetBeaconPacket` (`aru.a(ahu)`)
  654–662 and the corresponding packet's optional effect-holder codec.
- `Player.drop(ItemStack,boolean)` (`cmx.a(cuq,boolean)`) 747.
- The shipped `data/minecraft/tags/item/beacon_payment_items.json` lists
  netherite ingot, emerald, diamond, gold ingot and iron ingot. Production
  admission still queries the actual captured tag owner, not a local whitelist.

`javap -p -v` additionally resolves BeaconMenu's update lambda to
`Level.blockEntityChanged` (`dcw.q`). REA was unavailable. This is static
inspection, not original-Java menu execution. Raw disassembly and Java artifacts
are not redistributed.

## Native ownership and ordered behavior

`BeaconState` owns only one temporary payment stack. Both its payment container
and payment slot have capacity one. Levels and effect choices stay in the real
opened block entity through `BeaconHost`, which borrows its actual three-value
ContainerData owner. A synchronization snapshot reads levels/data 0, primary/data
1 and secondary/data 2 from that owner each time.

`BeaconState::open` first delegates the actual create-menu lock check through
`BeaconOpenAccess`. Source `canUnlock` admits a spectator or an actual main-hand
item satisfying the beacon's LockCode; otherwise it sends `container.isLocked`
in the action bar, plays the chest-locked sound and denies menu creation. Those
state and message/sound owners remain with the host.

The actual mob-effect registry encodes an absent effect as zero and a registered
holder as its registry ID plus one. Unresolved data IDs decode to absent.
`BeaconRegistry` requires the real effect registry and payment item tag; an
unavailable owner is an error, distinct from a negative membership result.

The serverbound handler first requires the player's actual active menu to be a
beacon menu, then calls its actual `stillValid`. This module validates the host's
captured block/player access; the production dispatcher must establish active
menu identity and the correct server thread. There is no additional spectator/creative or operator bypass in the selection
handler; the distinct opening lock check above retains its source spectator
branch.

If payment is present, the source update performs, in order:

1. Actual ContainerData setter for primary/data 1.
2. Actual ContainerData setter for secondary/data 2.
3. Remove one payment.
4. `Level.blockEntityChanged` at the captured block.

The source does **not** validate structure level tiers, primary/secondary
combinations, or require any particular effect choice at this point. Even both
absent choices consume payment. The real block entity setters filter against
`VALID_EFFECTS`: setting primary also emits the beacon selection sound on the
server if beam sections are present, before decoding/filtering the new holder.
This port delegates these callbacks to that owner instead of fabricating its
beam state or applying effects itself. It does not convert every registered
effect into a valid beacon power.

`BeaconHost` must participate in the enclosing transaction and roll back its own
staged changes on error. The menu publishes consumed payment only after all
callbacks succeed. A failed callback leaves the menu's canonical payment
unchanged; it does not by itself roll back an incorrectly implemented host.
`set_data` delegates the actual setter and yields a fresh actual data snapshot;
the adapter must broadcast immediately afterward, matching BeaconMenu.setData.

## Transfers and removal

Menu type ID is 9. Payment is slot 0; main inventory is 1–27 and hotbar 28–36.
Shift-moving payment targets shared player slots 1–36 in reverse order. A player
stack enters payment only when the payment slot is empty, its tag accepts the
item and the entire player stack's count equals one. A larger valid-payment
stack moves between main inventory and hotbar; it is not split automatically.

Removal first executes the actual base menu cursor callback. On the server,
BeaconMenu then removes its payment and calls `Player.drop(stack,false)`, which
expands to `around=false, trace_item=false`. This occurs even for an alive,
connected player. It is **not** the ordinary temporary-container inventory-return
path. `prepare_close` creates a non-cloneable, revision-checked plan; after the
host stages the real player drop, commit drains payment once and yields the
existing `DroppedStack` contract. No preview exists and no result is duplicated.

## Integration boundary

Main must provide block-use opening/lock checks, actual beacon block-entity
lifetime and data, the actual player/cursor/inventory, packet dispatch and
prediction, slot/state-ID/data synchronization, validity and close/disconnect
phases. Payment drop needs the real player Item/RNG/ID transaction; menu data
updates need actual block-entity sound/filter/dirty callbacks. These owners are
not supplied by default mocks or unsupported no-op implementations.

Beacon base/beam computation, effect ticking, persistence, sound publication,
block-entity network updates and full menus remain separate production work.

## Focused native verification

`cargo test --locked --lib workstations::beacon::tests` passed the ordered real-
owner callback contract, payment consumption without an invented level-tier
check, invalid-menu retention, failed-host payment retention and source-specific
close-drop ownership. These are native module flows with explicit test hosts,
not original-Java execution, protocol-client gameplay or Main integration.
