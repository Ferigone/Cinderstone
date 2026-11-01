# Dispenser and dropper control, Java 1.21

This stage adds callable control over a captured nine-slot block entity. It does
not supply a second inventory, a behavior registry, projectiles, a hopper owner,
Item entities or a Main block-tick adapter. The storage-menu adapter is separate.
No aggregate `inventory.4` or `crafting.5` completion is claimed.

## Primary evidence

Inspected the unchanged Java **1.21** server with official mappings and **JDK 21
`javap -p -c`**. Inner JAR SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings SHA-1:
`31c77994d96f05ba25a870ada70f47f315330437`.

Methods: `DispenserBlock` (`did`), `dispenseFrom` lines 92–118,
`neighborChanged` 123–132 and `tick` 136–137; `DropperBlock` (`dij`),
`dispenseFrom` 50–87; `DispenserBlockEntity` (`dra`), `getRandomSlot` 35–45;
`DefaultDispenseItemBehavior` (`kw`), `dispense`, `execute`, `spawnItem` and
sound/animation helpers 17–75; `BaseContainerBlockEntity.setItem` (`dqb`)
127–130; `RandomizableContainerBlockEntity.setItem` (`drn`) 75–77;
`HopperBlockEntity.addItem/tryMoveInItem` (`drh`) 283–347; and
`AbstractContainerMenu.getRedstoneSignalFromContainer` (`cpu`).

No REA tool was available. Raw disassembly and Java artifacts remain outside
Git. A separate reflection program called unchanged original `dyz`/`ayw`
LegacyRandomSource bounded-int, int, double and triangle methods to capture
the small RNG facts below. Those are method-level observations, not original
Java dispensing, item spawning or gameplay validation.

## Selection, behavior and source inventory

`getRandomSlot` first unpacks the actual loot table with a null player. It scans
all nine slots in ascending order and reservoir-samples nonempty slots with
`nextInt(1)`, `nextInt(2)`, etc. Even bound one advances the actual Level RNG.
Item count does not weight selection. An unavailable loot owner cannot be
replaced by an assumed empty inventory.

The dispenser first requests the exact dispenser block-entity type at its
position. A missing matching owner returns without sampling or dispensing.
An empty dispenser emits Level event **1001**, data **0**, then **BLOCK_ACTIVATE**
with no entity and its actual block entity's stored state as context. An empty
dropper emits only **1001**, with no BLOCK_ACTIVATE event.

For a selected dispenser item, disabled features explicitly choose the default
ejection behavior. Enabled items consult the actual dispenser registry and its
default return value. Registered NOOP performs no behavior or final setItem.
Other registered behaviors must use their real world/entity/item implementation;
an unported behavior is an error, not a silent ordinary Item fallback.

Default behavior splits one item from the canonical source stack, constructs
and adds its actual Item entity, emits **1000**, data **0**, then **2000** with
the captured facing's 3-D direction index, and finally writes the returned
source stack through actual `setItem`. That setter unpacks loot, replaces and
limits the slot, then invokes real dirty/comparator callbacks. There is no
invented extra sound, reward or advancement callback.

The default spawn point is block center plus facing × **0.7**, then Y is reduced
by **0.125** for a vertical facing or **0.15625** otherwise. After the actual Item
constructor, velocity consumes the same Level RNG: one double gives
`speed = nextDouble()*0.1 + 0.2`, followed by three triangle samples, each using
two doubles. Means are `(stepX*speed, 0.2, stepZ*speed)` and deviation is
`0.0172275 * accuracy`, with default accuracy **6**. Downward dispensing still
uses Y mean 0.2. `addFreshEntity`'s Boolean result is ignored by Java; it is not
a source inventory refund. `default_item_velocity` must run after real Item
construction, preserving its actual independent Item RNG/ID work.

## Dropper destination path

Dropper selects from the same real nine slots but bypasses the dispenser's
registered/feature selection. It reads the current Level block state's facing
to find the container in front with actual `HopperBlockEntity.getContainerAt`.
That lookup includes block/entity containers and their real lookup RNG. The
scheduled state's facing remains separate: fallback default ejection uses that
captured state through its BlockSource.

If a destination exists, source attempts to insert a count-one copy from the
selected stack through `HopperBlockEntity.addItem`, passing the **opposite**
current facing. Sided exposed-slot order, placement predicates, actual slot
capacity, hopper cooldown and destination dirty/comparator callbacks remain
with the real insertion owner. Acceptance decrements the source by one;
refusal preserves its count. Both paths perform the final source setItem.
They emit none of default ejection's sound/animation events. When no destination
exists, dropper uses ordinary default ejection.

The proposed count-one stack is part of the same staged source/destination
transaction. It is not a second item that may be independently committed or
spawned. The helper validates the returned remainder as empty or the unchanged
one-item remainder, never a fabricated replacement.

## Redstone and callbacks

`neighborChanged` checks actual neighbor signal at the source, then above it
only if needed. Rising power schedules the actual block for **four ticks**
before writing TRIGGERED true with block flags **2**. Falling power writes
TRIGGERED false with flags 2; it does not cancel that scheduled tick. The
scheduled callback dispenses without rechecking power or TRIGGERED.

`analog_output` uses the actual nine slots and Java float fullness intermediates:
sum `count/min(containerMaximum99,itemMaximum)`, divide by nine, multiply by 14,
floor, then add one if any slot is nonempty. It does not notify a comparator or
certify an unsupported receiver. Actual `setChanged` and destination insertion
callbacks must preserve their real dirty/comparator effects.

## API, verification and remaining scope

`DispenserHost` binds actual block-entity identity, loot, Level RNG, captured and
current facing, registry, source/destination inventories, Item construction and
callbacks. `scheduled_tick` mutates only that borrowed staged host, never a
menu-local inventory or fresh RNG. `DispenserTriggerHost` binds the real state
and scheduler for `neighbor_changed`. All methods are required; unavailable
owners or registered behaviors return errors. The enclosing transaction must
discard all staged owner mutations, RNG draws and effects on failure and verify
the same captured owner before commit. It must not call these helpers against
unprotected live owners and try to roll back only their slots.

Focused native checks use the actual native LegacyRandomSource and explicit
recording fixture owners. For seed **55**, unchanged original bounded draws
at bounds 1,2,3 are **0,1,1**, selecting occupied slot 1 from `[1,4,7]`; the next
int is **-742418317**. Default eastward velocity from a fresh seed 55 is
`(0.27177007744713355, 0.17748820205937174, -0.02673956673299945)` and the next
int is **559611923**. Checks also cover falling-power scheduled ejection,
split/spawn/event/dirty ordering, dropper acceptance/refusal without an extra
spawn, empty dispenser/dropper differences and explicit unsupported registry
handling. Compiler/native-flow results are recorded by the team handoff after
they run, not inferred from this source ledger.

Main registration/ticking, source block lifetime/loot persistence and removal,
the complete actual dispenser registry (projectiles, containers, equipment and
other special behaviors), real Item spawning, destination/hopper integration,
comparators and socket/gameplay validation remain pending. This is a bounded
source/control milestone, not a complete dispenser/dropper implementation.
