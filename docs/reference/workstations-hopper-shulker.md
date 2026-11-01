# Hopper transfers and shulker lifecycle reference

These are callable native kernels and explicit real-owner contracts. They do not
create native block entities in Main, replace the existing chest/barrel/furnace
owners, or complete the remaining all-menu/all-station checklist rows.

## Pinned primary evidence

The original Java 1.21 inner server JAR has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official server mappings have SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`. This stage used JDK 21
`javap -p -c` against those artifacts. REA was unavailable. This is static
bytecode inspection, not an original-Java gameplay observation. No Java binary
or raw disassembly is redistributed.

Inspected methods:

- `HopperBlockEntity` (`drh`), constructor 37–45, `removeItem`/`setItem`
  76–88, `pushItemsTick` 102–109, `tryMoveItems` 112–133,
  `inventoryFull` 137–143, `ejectItems` 147–177, slot/fullness helpers
  181–213, `suckInItems` 217–240, extraction 244–261, item-entity
  absorption 265–279, `addItem` 283–296, permissions 300–310,
  `tryMoveInItem` 314–347, container/entity lookups 352–408,
  `canMergeItems` 412 and `entityInside` 458–463.
- `Hopper` (`drg`) suction AABB initialization; `ItemStack.getMaxStackSize`
  (`cuq.j`) and original empty-stack component fallback.
- `ShulkerBoxBlockEntity` (`drr`), `updateAnimation` 79–107,
  `getBoundingBox` 114, `moveCollidedEntities` 118–141,
  `triggerEvent` 150–161, neighbor updates 165–167,
  `startOpen` 171–182, `stopOpen` 186–194 and sided admission 237–247.
- `ShulkerBoxBlock.canOpen` (`dmy`) 131–136 and
  `Shulker.getProgressAabb/getProgressDeltaAabb` (`ckj`) 318–332.

## Hopper native transfer phase

`HopperContainer` borrows an actual canonical container owner. It must expose
its real slots, ordinary/sided admission, source `canTakeItem` receiving the
actual destination object, loot-unpack behavior, stack setters, dirty callbacks
and (only for actual hopper owners) real cooldown/ticked-game-time fields.
There is no second five-slot image hidden in this engine. `push_items_tick`
requires an actual five-slot owner and errors without its actual timing owner.
The storage menu adapter must borrow these same slots.

The ticker decrements cooldown with Java integer wrapping, records the actual
level time, normalizes a nonpositive cooldown to zero, and then attempts transfer
only on the server when the actual block is enabled. It pushes before pulling.
Pull runs even after a successful push because Java uses a bitwise OR. A move
sets cooldown 8 and invokes the real static block-entity changed callback. A
positive cooldown blocks the entire transfer phase.

Push examines hopper slots in index order and tries one item from each until a
successful transfer. It queries destination fullness using actual sided slots,
then inserts using generic and sided placement rules. Pull examines the actual
source's down-face slots and checks generic `canTakeItem(destination,slot,item)`
before the down-face extraction rule. A failed one-item attempt restores the
existing stack's original count; original count-one removal uses the source's
actual setter to restore its now-empty slot. No fake item or destination is used.

Insertion preserves source slot encounter order. Empty destination slots receive
the whole item through the actual setter, including that owner's own cap and
callbacks. Merging changes the existing count directly, uses actual item/component
equality and the incoming item's maximum, and invokes `setChanged` only after a
positive move. An empty receiving hopper without a custom cooldown greater than
8 receives cooldown 7 if it has already ticked at or after the sending hopper;
otherwise 8. The source-timing comparison uses the actual counterpart fields.
A successful push also invokes the extra destination `setChanged`; a successful
pull invokes the source `setChanged` before the enclosing hopper callback.

The shared native inventory helper reports maximum zero for empty stacks,
where the original `ItemStack` falls back to one. The hopper fullness helper
explicitly handles empty stacks as nonfull, preserving the source branch.

## Actual world, entities and persistence required

`HopperWorld` must bind to one actual position/state/level. Output lookup uses
the attached position, insertion face opposite the actual facing; input lookup
uses the block above. Lookup precedence is actual block `ContainerHolder`,
actual container block entity, combined chest with obstruction ignored, then
actual entity-container query. The entity-container query uses the real level
RNG to select among encountered candidates, including the one-candidate draw.
An unresolved/unsupported owner must error; `None` certifies actual absence.
A stale claim or palette block alone cannot create container contents.

A present source container that yields no transfer does not fall through to
loose items. Only genuine source absence reaches the solid-above/actual
`DOES_NOT_BLOCK_HOPPERS` tag test and live item query. The source suction AABB is
`[0,11/16,0]..[1,2,1]` translated to the actual hopper block position; the native
`suction_aabb` helper and `entity_inside` intersection use that shape. Item query
encounter order and actual shared Item entity IDs remain with the level owner.

Partial entity absorption updates its actual remaining stack and returns false.
Only complete absorption sets the actual entity stack empty, discards that same
identity and returns true. Thus a partial first entity does not stop subsequent
candidates and is not falsely counted as a complete ingestion. The entity-inside
callback uses the same move phase but does not decrement the ticker cooldown.

`TransferCooldown` is persisted by the source; `tickedGameTime` is runtime state.
Loot tables, RNG, entity container lifetimes, chunk load/access and callback
ordering require their real owners. The host must stage all source/destination
inventories, counters, entities and callback/RNG effects in one transaction and
roll them back together on failure. This module cannot make disconnected world
owners atomic and supplies no dummy empty world or unconditional dirty receiver.

## Shulker opener, event and animation phases

`ShulkerLifecycle` owns only the original open count, animation status and two
progress floats. The real 27-slot container, color/components, lock, loot and
block-entity identity remain with the existing/future actual native owner.
`start_open` and `stop_open` require actual player identity and spectator/removed
checks from `ShulkerHost`. Neither a removed owner nor a spectator changes the
counter or produces callbacks.

Start clamps a negative count to zero, increments, and enqueues block event
`1/count`. On first open it emits the source container-open game event, draws one
actual level random float and plays the shulker-open sound at volume 0.5 and
pitch `nextFloat()*0.1f+0.9f`. Stop decrements, enqueues the event, and emits the
close game event/randomized sound when count is nonpositive. Other viewers do
not manufacture another opening sound or RNG draw.

Starting/stopping does not directly animate. Only actual delivery of block event
1 to that same block owner sets the open count and selects Closing for zero or
Opening for one; other counts retain status. Unknown events return unhandled so
the caller delegates to its actual base block-entity event implementation. A
same-position replacement must not receive an old owner's event by accident.

The block-entity ticker snapshots old progress and applies Java float `+0.1f` or
`-0.1f`, clamping at endpoints. Initial/final transition callbacks invoke block
neighbor-shape updates with flags 3, then `Level.updateNeighborsAt` in that order.
Opening also queries entities in the actual swept lid slab and moves each
non-`PushReaction.IGNORE` entity, in query order, through actual
`Entity.move(MoverType.SHULKER_BOX,delta)`. Delta is swept-slab size plus 0.01 in
the facing direction. Direct entity-position assignment would bypass required
collision callbacks. Closing does not perform these push queries.

The swept AABB uses raw old/current progress, while the physical box bounding
AABB uses half of interpolated progress. Both preserve the original
`expandTowards` then `contract` geometry. A fully closed box requires the real
world's `noCollision` over the opening slab from 0 to 0.5, translated to the real
block and deflated by `1e-6`; an already opening/open/closing box skips that
initial obstruction test. An empty-world collision shortcut is not supplied.

Actual shulker block items, uncolored or any color, are rejected by the native
`may_place` helper, matching ShulkerBoxSlot and sided insertion. Other slot
permissions, shared cursor, inventory, state IDs, menu opening and close order
belong to the storage adapter plus actual block/world/player owners.

## Production boundary

These engines have no Main block-entity registration, ticker, persistence,
collision, menu or removal callsite yet. The coordinator's storage adapter is a
separate delivery. Real item/drop/XP/entity/RNG ownership, block history, block
events, comparator receivers and player phases must be wired by the central
integrator. Unsupported owner contexts remain explicit errors; no all-menu or
all-station completion is implied.

## Focused native verification

`cargo test --locked --lib workstations::hopper::tests` passed push-before-pull,
actual counterpart timing adjustment, sided slot order, failed-source restoration
and partial-vs-complete entity absorption flows. The focused
`workstations::shulker::tests` flow passed event-delayed animation, source-ordered
callbacks, endpoint transitions, ignored-push entities, spectator guards and
nested shulker admission. These use explicit native test owners; they are not
original-Java execution, protocol gameplay or Main integration evidence.
