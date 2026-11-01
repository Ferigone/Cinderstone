# Hopper, dispenser/dropper and shulker storage menus

Target: Minecraft Java 1.21, protocol 767. Primary evidence is the unmodified
inner server JAR with SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
and official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
The inspection method was JDK 21 `javap -p -c` directly. No REA execution,
Java gameplay observation or source reconstruction archive is claimed.
Java artifacts and raw disassembly are not committed.

| Official source | Mapped class | Source facts used |
| --- | --- | --- |
| `HopperMenu` | `cqu` | Five actual top slots; constructor startOpen, container stillValid, reverse transfer to player, forward transfer to top, removed stopOpen |
| `DispenserMenu` | `cqo` | Nine actual top slots; same transfer directions; successful quickMove additionally calls base Slot.onTake |
| `ShulkerBoxMenu` | `crn` | Twenty-seven actual top slots; start/stop and transfer behavior; ShulkerBoxSlot insertion rule |
| `ShulkerBoxSlot` | `cro` | Delegates mayPlace to Item.canFitInsideContainerItems |
| `Item`, `BlockItem` | `cul`, `cso` | Base item fits; BlockItem rejects only an actual ShulkerBoxBlock |
| `AbstractContainerMenu`, `Slot` | `cpu`, `crq` | All seven click modes, source placement/transfer order, dirty callbacks, cursor and drag rules |
| `Container`, `Inventory` | `bqk`, `cmw` | Default maximum 99, per-stack minimum; actual Inventory.setChanged increments timesChanged |

## Ported menu contract

`src/container_workstation_storage.rs` implements these three menu families.
Dispenser and dropper share the original nine-slot DispenserMenu. Existing
chest/barrel GenericContainerMenu is neither modified nor counted as new work.

| Family | Menu registry ID | Actual block slots | Main inventory menu slots | Hotbar menu slots |
| --- | ---: | --- | --- | --- |
| Hopper | 16 | 0–4 | 5–31 | 32–40 |
| Dispenser/dropper | 6 | 0–8 | 9–35 | 36–44 |
| Shulker box | 20 | 0–26 | 27–53 | 54–62 |

All player slots borrow the canonical inventory's main indices 9–35 and hotbar
indices 36–44. Offhand swap uses actual player slot 45. No block-container
contents live in the retained menu; it stores only the captured actual owner
key, cursor, drag/synchronization state and a checked menu revision.

The `StorageContainer` trait requires the real block object's key, kind,
content revision, actual slot slice and maximum size, actual-player validity,
and an atomic install of a prepared contents image. It does not construct a
private `ContainerOwnerKey`, generate substitute block identities or invent a
world/player/RNG owner. The central host must implement this trait on its actual
native container owner. A replacement at the same coordinates cannot satisfy a
captured key. A prepared action checks that key, source contents/revision,
player inventory and retained menu state before committing once.

Normal pickup, repeated shift transfer, hotbar/offhand swap, creative clone,
throw, quick-craft and pick-all follow the inspected source methods. Transfers
from the top scan the player's slots in reverse order; transfers into the top
scan forward. The actual container and item component limits are used; there
is no hardcoded 64-item cap. Source-stack shrink is recorded before each
destination callback, so callback observations conserve the actual moved count.
The source merge pass omits mayPlace, while the empty-destination pass checks it.

Shulker placement uses the actual item-class predicate supplied by the host.
The pinned default implementation recognizes the uncolored and sixteen colored
ShulkerBoxBlock items through canonical block-state identities; component
contents do not turn another Item class into a ShulkerBoxBlock. Unknown item
classes/custom registries require the actual registry owner. Refused inserts
remain owned by the cursor or player slot. Pickup and pick-all remain available
for the actual stored stacks, following Slot's removal rules.

## Ordered source callbacks and lifecycle

`StorageMenuEvent` retains lifecycle, canonical container operations, local
listeners, packets and indexed drops in source order. A `Container` event
contains the real captured key, the source operation and an intermediate slot
observation. `SetItem`, in-place `MutateStack`, `RemoveItem` and explicit
`SetChanged` are separate operations: the host invokes its real container
methods, preserving any dirty/comparator behavior inside those methods. It must
not replace them with a fake observer or silently discard unsupported receivers.
`PlayerInventoryChanged` separately requires the actual Inventory.timesChanged
owner; a packet/listener update is not a substitute for that source callback.

The source constructors call container.startOpen before Main's OpenScreen and
initial synchronization. `prepare_open` emits the mandatory `StartOpen` marker;
the host stages it with its actual container/player/world owners before commit.
The actual hopper/dispenser callback may be the inherited no-op, but the source
invocation is still represented. Shulker opening count, block event, animation,
world RNG/sound and collision movement belong to the real shulker lifecycle
host; the menu does not fabricate those states.

All three menus delegate stillValid to their actual container. That owner must
check the captured object/world and actual player interaction range; the menu
does not infer validity from coordinates or install a substitute distance test.
Opening locks, unresolved loot and shulker obstruction must be handled by the
actual block/menu provider before construction. A removed captured owner can
still run close; a new same-position owner cannot take its place.

Removal follows AbstractContainerMenu.removed's cursor return/drop, then the
actual container.stopOpen callback. No world slot is returned as temporary menu
input. Close uses the shared ordered removal helper and retains insertion
packet/overflow-drop order. `Drop { index }` references the one owned entry in
`outcome.drops`; the host executes it at that position, without independently
draining drops. Ordered packet events are the sole packet publication owner.

The host must prepare the actual owner/world/RNG/drop callbacks before consuming
a plan, then publish the ordered events with that single commit. The atomic
contents installer must not replay already staged callbacks. Java quick-craft iterates an identity-based HashSet of Slot objects. The native
batch uses slot-index order only when the actual host certifies
`quickcraft_callbacks_are_order_independent`; this mandatory method has no
permissive implementation. Certification is checked both before preparing the
slot batch and immediately before committing it. Multi-slot
drag affecting the block container rejects unknown or order-sensitive
comparator/listener contexts rather than inventing a JVM identity-hash owner.
The current bounded native profile can certify an actual empty receiver set.
Reentrant custom listeners and bundle item-click overrides are explicit
unsupported contexts.
There is no fabricated recipe, component, advancement or reward owner.

## Integration and validation scope

These are callable native menu transactions. No Main opening, click dispatch,
container lifetime registration, persistent block owner, drop spawning or
production callback implementation is claimed here. Hopper transfer ticking,
dispenser/dropper behavior and shulker world animation are independent owner
modules coordinated with this menu; this file does not duplicate their slots.
No complete inventory.4 or crafting.5 checklist row is claimed.

`cargo test --lib container_workstation_storage` passed the focused native
transaction flows. The in-memory test owner owns the actual test slot vector;
it is explicitly not a production-world substitute. The flows cover all three
families, stale client state resynchronization, normal/drag/swap/pick-all/shift/
throw/creative-plan behavior, canonical offhand contents, content-revision and
same-position replacement rejection, and closing without returning world slots.
They conserve eight input stone as four in the actual offhand, one deliberate
throw and three cursor-close drops while retaining separately updated quartz
in the block owner. The second flow checks all seventeen shulker item classes
and an actual max-stack-size component of 99. It also rejects a multi-slot
drag when callback-order certification is absent, and rejects an already
prepared drag if that receiver context changes before commit, without changing
canonical contents or cursor. These are native transaction observations, not a Java oracle run, network-client flow or Main integration.
Comprehensive conformance remains deferred.

The empty-target branch of `cpu.moveItemStackTo` invokes Slot.setByPlayer and
then an additional Slot.setChanged. Because setByPlayer itself calls the
container setter and setChanged, the native trace retains both explicit dirty
calls. The focused checks assert both calls for a real top destination and both
Inventory.timesChanged callbacks for an empty player destination. SWAP has no
same-hotbar-slot shortcut in the source: a nonempty self-swap retains its
setByPlayer and onTake callbacks, producing two player inventory change calls
even though item contents are unchanged. This multiplicity is asserted directly.

## Cross-owner phase observations and aliasing

Every lifecycle, container, player-change, sync and drop event now carries a
read-only `StoragePhase`: the actual staged block contents, canonical player
inventory and external menu cursor at that source boundary. The external cursor
is separate from the retained window0 carried value inside PlayerInventory.
These observations are not extra owned inventories or drops. A real host stages
its source callbacks against those intermediate owners; replaying all callbacks
against only the final inventory would be incorrect. Opening now borrows and
validates the actual player inventory as well as the block owner. Initial and
incremental sync entrypoints return phase-tagged storage events.

Safe insertion splits/shrinks the live cursor before destination setter/dirty
callbacks. Shift merging shrinks the source owner before destination callbacks;
its later empty-target insertion can observe a different source count. The
focused flow checks a source count of 100 becoming 99 at one container dirty
boundary, then zero at the next two boundaries, while conserving 198 total stone.
It also checks the right-click insertion callback observes the five remaining
cursor emeralds, rather than the old count six. Right-click outside drops observe
the already split cursor. Cursor-only closure reconstructs each insertion phase
from the ordered raw inventory-slot packets; its overflow drop sees the actual
remaining cursor before stopOpen observes the empty cursor.

A nonempty ordinary same-hotbar SWAP retains both dirty callbacks. An overfull
same-hotbar SWAP also preserves source aliasing: the hovered stack and held stack
refer to the same original object, so splitting it leaves only the remainder
for Inventory.add. The native code detects that actual canonical slot alias and
returns only that remainder. A focused two-iron-sword fixture becomes one sword
in the original hotbar slot and one in the next free slot, with two inventory
change callbacks and no duplicated item. All focused storage flows passed after
these corrections. No Main or network-client observation is implied.
