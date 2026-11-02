# Beacon menu ordered/live effect selection

Target: Minecraft Java **1.21**, protocol **767**. This stage adds live
orchestration for `BeaconMenu.updateEffects` and the separate `setData` method.
It borrows the actual retained menu, payment Slot, native beacon, registry,
ContainerData, DataSlot list and captured ContainerLevelAccess. It constructs
none of those owners and adds no inventory, preview, player body or RNG.

The initial source base is published `b72f205` (PR120); Root's current-master
composition is recorded separately in the stage status. The earlier
[native beacon entity](workstations-beacon-entity.md) and
[ordered/live tick](workstations-beacon-live-contract.md) remain distinct
source stages. This addition does not install Main Beacon menu support.

## Primary source evidence

Evidence is direct **JDK 21 `javap -p -c`** inspection of the unchanged original
server with official mappings, not REA or execution of a Java menu/world flow.
Inner server JAR SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`.
No Java artifact or raw disassembly is committed.

| Original source | Observable order |
| --- | --- |
| `BeaconMenu` (`cpy`), `updateEffects` 166–172 | If the actual payment Slot has an item, encode/set primary, then encode/set secondary, remove one current payment, then execute the original captured access to call Level.blockEntityChanged. |
| `cpy.encodeEffect`, 147 | Actual built-in holder ID plus one with Java wrapping arithmetic; absent Optional encodes zero. |
| `cpy.setData`, 94–96 | Call the superclass menu DataSlot setter, then broadcastChanges. This is not the raw BE ContainerData setter. |
| `AbstractContainerMenu` (`cpu`), `setData` | Resolve the actual DataSlot list at the signed index and invoke its setter. A negative/out-of-range index fails before broadcast. |
| `Slot` (`crq`), `remove` | Delegate removeItem(slot, count) to the same actual container; no extra Slot.setChanged or Slot.onTake. |
| `SimpleContainer` (`bra`), `removeItem` | Split the current stack, then setChanged only when the removed stack is nonempty, retaining the real listener order. |
| `ContainerLevelAccess` (`cqg`) and update lambda | Use the access captured by the menu, not the beacon's possibly reattached Level. Genuine NULL access may do nothing; an unavailable world owner is not a NULL substitute. |

`updateEffects` writes its captured ContainerData directly. It does **not** call
the menu's `setData` wrapper or add a broadcast after either setter. Main's
current-menu-class and stillValid checks belong outside this method, at packet
dispatch. No second validity, tier/combination, payment-tag or creative-mode
test is invented inside the source operation. Two absent effects still consume
payment when the initial Slot has an item.

## Exact callable boundary

The new [beacon_menu_live.rs](../../src/workstations/beacon_menu_live.rs)
provides `BeaconMenuLiveHost`, `update_effects_live` and `set_menu_data_live`.
The host extends `BeaconEntityDataHost` to address the same native
`BeaconBlockEntity` and its existing live ContainerData setter.
`BeaconMenuLiveAction` identifies admission for effect selection or menu data
mutation. An initially empty payment returns `BeaconUpdate::IgnoredNoPayment`;
completed selection returns `Updated`, without claiming a packet was sent.

The menu owner must admit and retain the actual source menu/Slot/data/access
objects before the operation. Equality of payment bytes, coordinates, a reused
window ID or a caller Boolean cannot establish that lease. Closing/replacing
the current UI during a callback must not redirect the still-running source
operation to another menu, BE or payment container. The actual anonymous
ContainerData(this) capture remains required; this layer supplies no stand-in.

Registry encoding uses actual holders and the real holder ID map. It does not
construct an effect registry or reinterpret a tier number as a holder. A
read-only registry lease retains its genuine owner identity/generation; an
unavailable service remains an error. Ordinary queries must be nonreentrant.
Real data, sound, container-listener and access callbacks may reenter under
their retained-owner contract.
Absent selections encode zero without a registry query; a genuine getId result
of minus one also wraps to zero. The method does not add a registration or
effect-filter check at this encoding phase.

`set_menu_data_live` requires the actual menu DataSlot-list operation, including
its bounds and captured binding. The raw BE ContainerData's unknown-index
zero/no-op rule is not a menu-list bounds rule. After a successful DataSlot set,
the wrapper invokes the actual retained menu's broadcastChanges once. If list
lookup or its setter fails, that broadcast is not run. The host owns source
listener/remote-sync/state-ID/cursor/outbox effects; this layer does not emit
replacement packets or replay returned observations.

## Selection order, current payment and committed prefixes

After the initial actual hasItem observation, the operation performs:

1. Encode the requested primary through the actual registry.
2. Invoke live ContainerData index one on the same BE. Its source server/beam
   check and selection sound precede decode/filter/assignment, including zero,
   invalid or unchanged selection. The sound callback sees the old primary.
3. Only then encode the requested secondary and invoke live index two, silently.
4. Call remove(1) on the original payment Slot's **current** stack.
5. Execute the original menu access's Level.blockEntityChanged at its captured
   position.

The secondary value is not encoded up front. Payment is neither snapshotted nor
reserved as an independent stack before callbacks. If reentry replaced it, the
source removal acts on that replacement in the same retained payment container.
If reentry emptied it, remove still runs and yields empty: SimpleContainer
emits no changed callback for that empty result, but the final access operation
still follows. A nonempty removal runs only its actual container callbacks;
there is no added onTake, currency owner or refund path.

Errors stop at their actual boundary and retain the executed prefix. A sound
draw/output, earlier power assignment, reentrant mutation or payment removal
does not disappear when a later encode, setter, listener, access or broadcast
fails. The caller must not retry the whole invocation, replay callbacks, clone
the menu/BE/actors or roll back their live graph. Sound uses the existing actual
Level threadSafeRandom and audience/publication owner through the BE callback;
no new stream or duplicate sound is introduced here.

The old [staged beacon controller](workstations-beacon-menu.md) remains unchanged
and requires its own explicitly proven restricted domain. It cannot invoke these
live operations from an unproven detached stage. Conversely, this live boundary
does not certify the staged click/drag/close transaction or replace its owner.

## Validation and production integration

Root checked frozen Rust SHA-256
`67e998c7d72f7d294f3cdedb15e52c6e8f38ae7c0de5a7d7fc8ba24a7d711cb6`.
The clean composition with master `3da06b2` passed isolated locked offline
all-target compilation in 21.26s. The focused native flow passed, checking
primary callback changes before secondary-encoding failure, current payment
consumption retained after later access failure, invalid menu-index refusal
without broadcast and a valid setter followed by one broadcast. Separate
pinned-primary/API review found no concrete blocking defect. Formatting,
whitespace and staged-index hygiene passed.

The fixture uses genuine shared BE/base/EMPTY, existing BeaconState and tick
algorithms; its remaining owners are explicitly controlled descriptors. The
retained-menu marker points to the existing PAYMENT BeaconState, not a native
BeaconMenu object. It does not establish Menu/DataSlot/ContainerData allocation
or capture. That distinction remains an actual integration requirement.

Controlled recording hosts are not actual configured menus, registry holders,
Level/actor owners, source-Java execution or Main/gameplay evidence.

Central integration still supplies actual menu construction and retention,
ContainerData(this), the DataSlot list, canonical payment Slot/container and
listeners, BE arena/type registry, packet guard/decoder, world access and
ordered publication. The generic update-packet factory and native BE index,
ticker and checkpoint bindings remain separate pending contracts. Missing
services must fail rather than become empty listeners, a synthetic NULL access
or a payment clone.

This is a bounded source prerequisite, not Main Beacon gameplay or aggregate
completion of `inventory.4`, `crafting.2` or `crafting.5`. No shared Main/runtime,
checklist, journal, manifest or deployment configuration is changed.
