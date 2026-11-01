# Crafter reference ledger

Target: Minecraft Java 1.21, protocol 767. Primary method evidence is direct
JDK21 `javap -p -c` inspection of the unchanged server using official Mojang
mappings. No REA execution, Java gameplay oracle or native Main integration is
claimed. Artifacts/disassembly remain outside the repository.

- Outer server SHA-1: `450698d1863ab5180c25d7c804ef0fe6369dd1ba`.
- Inner server SHA-256: `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
- Official mappings SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`.
- Inspected classes: `CrafterMenu` (`cqj`), `CrafterSlot` (`cqk`),
  `NonInteractiveResultSlot` (`crg`), `CrafterBlockEntity` (`dqv`, including its
  ContainerData implementation), `CrafterBlock` (`dhr`), `RecipeCache` (`cza`,
  including Entry), `ItemStack` (`cuq`), `DefaultDispenseItemBehavior` (`kw`),
  `ServerboundContainerSlotStateChangedPacket` (`agz`) and its actual server
  handler in `ServerGamePacketListenerImpl` (`aru`). `BaseContainerBlockEntity` (`dqb`), `AbstractContainerMenu` (`cpu`)
  and `Slot` (`crq`), plus `CraftingInput` (`cyp`), supply the inherited mutation/broadcast boundaries.

## Actual state and layout

The block owns nine input stacks. Data indices0–8 contain1 for disabled and0 for
enabled; index9 is triggered. Menu type7 has block slots0–8, actual player main
inventory/hotbar9–44 and a **derived noninteractive result at45**. The result is
not an owned inventory output and closing the menu returns only its cursor.
Block inputs remain in their captured block entity. The native common sync
adapter explicitly handles this noncontiguous layout.

`CrafterBlockEntity.setSlotState(index, enabled)` accepts only valid empty slots,
even when enabling. The source packet carries slot VarInt, container VarInt and
the enabled boolean; it has no click state ID. Its handler rejects spectators and
wrong windows and invokes the actual crafter block entity. It does not mutate
only a menu-private data copy, nor broadcast immediately. The native adapter
records the real dirty callback and leaves data publication to the actual player
broadcast phase.

`setItem` auto-enables an empty disabled slot before installing a stack. Native
UI clicks avoid calling it for an empty cursor/empty slot, which would otherwise
incorrectly enable a disabled slot. `CrafterSlot.mayPlace` checks only whether the
slot is enabled. Automation's `canPlaceItem` additionally rejects full current
stacks and prefers a later enabled empty slot or a later smaller same-component
current stack. Its proposed incoming stack argument is not used; the actual
Hopper insertion owner separately checks item compatibility. Comparator output
counts each nonempty **or disabled** slot, from0 through9; it is not fractional
inventory fullness.

Saved fields include actual items, disabled indices, `crafting_ticks_remaining`
and raw integer `triggered`. Restore ignores invalid or occupied disabled indices,
as Java does; triggered equals true only for integer1. Loot-table loading,
physical identity, dirty/comparator callbacks and actual native persistence
registration remain with the central block-entity owner.

## Triggering, recipes and output order

A neighbor rising edge schedules a real block tick after4 ticks, writes
TRIGGERED=true with flags2, then sets the block entity's triggered data. A falling
edge clears TRIGGERED and CRAFTING with flags2 and clears triggered data; it does
not cancel the queued tick. `CrafterBlock.tick` attempts crafting without another
power check. Successful craft starts a6-tick animation. The block-entity tick
decrements that counter, and at zero clears CRAFTING with flags3.

The real world's shared ten-entry RecipeCache selects crafting recipes. Its keys
use the cropped input width/height and same-item/same-components comparisons,
ignoring counts; hits retain positive or negative holders without rerunning
matches. A changed RecipeManager identity clears the cache. The native
`CrafterRecipes` contract requires that actual cache/recipe/registry owner; it
never guesses order from sorted fixture IDs or invents a replacement cache
identity. The supplied native checks use explicit fixture recipes and do not
establish production cache integration.

`prepare_auto_craft` preserves the inspected sequence:

1. Resolve potential recipe and assemble. Missing/empty results emit level
   event1050, with no consumption or animation start.
2. Set crafting counter6 and CRAFTING=true with flags2.
3. Invoke output `ItemStack.onCraftedBySystem` using its actual item/world owners,
   including map postprocessing when required.
4. Dispense the main output, then request recipe remainders and dispense each
   nonempty remainder in source list order.
5. Shrink every nonempty actual input by one, then mark the block entity changed.

Dispensing captures the actual container ahead of the block's orientation front.
A neighbor crafter, or output exceeding the destination's actual stack maximum,
is inserted one item at a time. Other containers receive whole remaining stacks,
repeating while progress occurs. Every insertion uses the source/destination
Hopper transfer owners and the opposite face. Uninserted items enter the actual
DefaultDispenseItemBehavior spawn path. Only that spill branch triggers nearby
ServerPlayers' CRAFTER_RECIPE_CRAFTED criteria using the nine pre-consumption
inputs, then level events1049 and2010. Full successful container insertion has
neither those spill criteria nor those effects. Each remainder uses the same
source sequence.

Default spawn accuracy is6; the position is block center plus facing*0.7,
adjusted downward0.125 for vertical facing or0.15625 otherwise. The actual
five-argument Item constructor first evaluates **two shared-level
nextDouble velocity arguments**, then creates its independent entity RNG and
UUID/bob/yaw. The initial motion is replaced later but those two draws remain.
After construction the shared level RNG samples speed `nextDouble()*0.1+0.2`
and three triangle deviations of
`0.0172275*6`, with X/Z means facing step times speed and Y mean0.2. These calls
therefore consume nine level nextDouble calls in total. Pickup delay stays at
the constructor's zero, with absent target/thrower. Existing native dispenser
position/velocity helpers apply after the actual constructor; there is no
Player.drop statistic/thrower/delay path here. These are an explicit real
Item/RNG/ID host contract; this module creates no synthetic
entities or random generator. Actual container lookup, insertion callbacks,
nearby-player AABB selection and advancement rewards must likewise be staged
by their real runtime owners. Reentrant owner callbacks remain unsupported and
must fail before publication.

## Native boundary and focused checks

`CrafterState` is callable native block state with borrowed recipe/output owner
traits. Prepared plans capture the actual `CrafterOwnerIdentity::Key` supplied by the
host and check it together with the complete expected state. A same-position
replacement with identical slots cannot consume an old plan. Production must
supply the real physical key and commit the real output/world staging
transaction with it; no synthetic/default identity is provided. The menu adapter borrows that state and
the canonical player inventory, with one-shot complete-snapshot guards and a
nonreused UI identity/revision. `CrafterMenu<K>::new(window, callbacks)` captures
the actual physical key; initialize, broadcast, prepare, validate and commit
check the current owner. Its mandatory actual-player validity callback must use
Container.stillValidBlockEntity on that captured owner. It supports all seven protocol click modes and
slot-state packets; bundle item-click overrides remain an explicit pre-mutation
boundary. NonInteractiveResultSlot forbids normal extraction/insertion,
quick-move, swap, throw and pick-all. The source CLONE branch deliberately does
not consult mayPickup: an actual creative player can clone its displayed
preview to the cursor without consuming inputs. This is an explicit creative
creation action; ordinary result ownership and close handling never duplicate it.

Focused native flows exercise cake output and three bucket remainders before
input consumption, pre-consumption criterion inputs, animation completion,
disabled-slot equalization, rising/falling edges, cropped recipes, one-item
neighbor-crafter insertion, stale-plan rejection, preview slot45 wire placement,
shared input retention on close and actual packet toggle routing. Compiler
results and commits are recorded by the coordinator. These are in-process native
flows, not socket/client or original-server gameplay observations. Main's block
registration, scheduler/ticker, physical owner, packet dispatch, loot/persistence,
Hopper transfer, real drops/RNG and criteria callsites remain pending integration;
the broad inventory/crafting checklist deliveries remain partial.

## Intermediate source broadcasts and callbacks

`AbstractContainerMenu.slotsChanged` calls `broadcastChanges`; it is not a no-op.
For a grid slot, `Slot.set` invokes actual `CrafterBlockEntity.setItem` (which
marks changed), then `CrafterSlot.setChanged` marks changed again and invokes
that menu broadcast. `moveItemStackTo` merges by mutating the actual source and
target stacks before target `setChanged`; an empty-target insertion also invokes
an extra target `setChanged`. Removal, empty-slot installation, `onTake` and the
final ordinary PICKUP slot change retain their separate source boundaries. The
adapter emits ordered block-entity dirty observations with their state snapshots
and the intermediate local-listener broadcasts. Remote updates stay suppressed
until packet predictions are applied and the final incremental/full state is
sent. Dirty observations contain callback data, not substitute physical owners.

CrafterMenu's self-listener refreshes the result on changed local slots before
slot45 is traversed. The adapter uses the shared actual local-baseline change
gate to avoid touching the global recipe cache on unchanged or data-only
broadcasts. Ordinary nonreentrant listeners see the same final input key;
subsequent same-key cache hits have no additional LRU effect. Arbitrary reentrant
listeners, advancement reward mutations and comparator receiver effects require
the actual runtime integration contract; they must not be silently ignored.
Multi-slot drag uses a stable native traversal only after the actual
`CrafterMenuCallbacks` host certifies dirty/comparator/listener callbacks are
independent of target encounter order. The plan rechecks that certification
before commit; missing, unknown or order-sensitive contexts fail before
publication. Exact JVM identity-HashSet traversal is not manufactured.

Later focused flows exercise actual creative-preview clone gating, forbidden
result swap, enabled/disabled drag targets, pick-all conservation, offhand swap,
owned-input throw, intermediate local listeners before final remote packets,
separate remove/set/onTake/final dirty boundaries, and unchanged/data-only cache
lookup gating. These remain in-process native flows and do not close the full
inventory/crafting deliveries or demonstrate production invocation.

The automatic-craft host receives source snapshots at the two observable owner
boundaries: counter6 before the CRAFTING flags2 write and item/system callbacks,
then consumed inputs before final block-entity setChanged/comparator callbacks.
It must install those phase snapshots in the actual captured staging owner
before the corresponding callback, rather than consulting the original live
pre-craft state. This is native transaction preparation, not an additional Java
world callback or a second block entity.

The focused drag flow also rejects an uncertified actual-callback context and
rejects a prepared action when certification changes before commit. There is no
permissive production default for that capability. Physical source identity and
player/world transaction binding remain required integration contracts.

Core neighbor/ticker/automatic-craft plans likewise capture the real owner's
associated key, and commit rejects a replacement even when state is structurally
identical. One remaining alias boundary is explicit: an overfull SWAP targeting
the same canonical player slot as the selected hotbar item is rejected before
mutation. Java shares that source ItemStack object during split/add; cloning the
original stack would duplicate items. The native adapter does not manufacture
that alias or claim this context works.

Every menu callback, drop and synchronization event now carries a read-only
`CrafterPhase`: actual block state, canonical player inventory (including its
retained window0 carried value), external menu cursor and derived preview. This
lets the actual host observe the state at each source callback instead of
reading only the final transaction. Snapshots confer no item ownership.
`PlayerInventoryChanged` is a mandatory typed phase operation for actual
Inventory.setChanged/timesChanged, separate from inventory slot listeners.
Slot.set and onTake preserve their separate counter calls; a normal nonempty
self-hotbar SWAP produces two even when contents stay identical. Overfull
self-alias remains rejected before mutation. Source close raw insertion packets
carry reconstructed per-insertion inventory/cursor phases, followed by the
remaining overflow drop phase; the block's inputs are retained throughout.


## Matching full-slot pickup correction

`Slot.safeInsert` (`crq.b(ItemStack,int)`, bytecode offsets 22–87) retains the
signed `slotMaximum - existingCount` capacity. For equal components it invokes
`setByPlayer` even when the accepted amount is zero. The crafter controller now
retains the actual container setter's dirty callback, the first
`CrafterSlot.setChanged` dirty/broadcast, and the ordinary PICKUP final
`setChanged` dirty/broadcast as three separate source dirty boundaries. It does
not collapse a full matching target into only the final callback.

For a loaded overfull target, the same signed calculation returns excess items
to the actual carried stack before those callbacks: 99 stone in a maximum-64
slot plus one carried stone becomes 64 in the slot and 36 carried. Clamping the
transfer amount to zero before calling the real setter would lose those 35
excess items. The focused native regression checks both zero-acceptance and
this overfull case, their three phase snapshots, a pending authoritative player
inventory listener between the second and final dirty boundaries, the unchanged
derived preview and exact stone conservation. This is an in-process native
source-phase check, not a Main or network gameplay observation.


Outside PICKUP retains the distinct cursor phases from `AbstractContainerMenu`
(`cpu`, offsets 619–651): left click drops the whole actual cursor before
clearing it; right click splits one from that cursor before invoking the drop.
The native Drop phase now exposes the original carried stack for left click and
the reduced carried stack for right click. A focused regression verifies those
snapshots, the exact owned drop, unchanged block inputs/derived preview and item
conservation for both buttons. The actual Item/RNG/world drop owner still belongs
to central integration.
