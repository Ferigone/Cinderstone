# Furnace removal reference ledger

This records static behavior from the shipped Minecraft Java **1.21** server,
not a claim that Cinderstone has executed or fully implemented each path. The
integration status belongs in the roadmap and runtime reports.

## Reference identity

The pinned inner server JAR SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The observations below use JDK 21 `javap -p -c` and its matching Mojang mappings.
Private disassembly is retained outside this repository. The earlier REA archive
inventory Evidence
`ev_8062f8049307794f7a4fdb13f2a8e90b1c3c3a9f596760ee21b9aa77832b9e09`
establishes archive contents only; it is not the source of these JVM behavior
claims and did not execute furnace removal.

| Mapped class | Shipped name | Relevant mapped methods |
| --- | --- | --- |
| `Containers` | `bqn` | `dropContents`, `dropItemStack` |
| `ItemEntity` | `cjh` | constructors |
| `AbstractFurnaceBlock` | `det` | `onRemove` |
| `AbstractFurnaceBlockEntity` | `dpv` | `getRecipesToAwardAndPopExperience`, `createExperience` |
| `ServerPlayerGameMode` | `aqw` | `destroyBlock` |
| `LevelChunk` | `dvi` | `setBlockState` |
| `Level` | `dcw` | `removeBlock`, `updateNeighbourForOutputSignal` |
| `BlockBehaviour` | `dtb` | `onRemove` |

## Creative destruction and lifecycle order

`ServerPlayerGameMode.destroyBlock` first checks the selected item's
`canAttackBlock`, obtains the actual block entity, checks the special
administrative-block restriction and the player's block-action restriction.
It then invokes `playerWillDestroy`, followed by `removeBlock(pos, false)`.
When removal succeeds it invokes the block's `destroy` callback. Only after
these callbacks does the creative-mode branch return.

The matching mappings contain `canAttackBlock` overrides only on
`DebugStickItem`, `MaceItem`, `SwordItem` and `TridentItem`. The ordinary `Item`
implementation returns true. Mace, sword and trident return false for a creative
player. A debug stick invokes its special server-side state-selection handler
and then returns false; accepting its input as an ordinary break would skip an
observable callback. Unsupported debug-stick handling must remain explicit.

Consequently creative mode skips `mineBlock` and `playerDestroy` (the later
block-loot path), but does **not** skip a furnace's `onRemove` inventory and
recorded-experience effects. The base `playerWillDestroy` emits break level
event 2001 and a `BLOCK_DESTROY` game event before removal. Its level event
excludes the initiating player.

`Level.removeBlock(pos, false)` replaces the state with the actual fluid
state's legacy block, using flags 3. A general implementation must not replace
this with an unconditional air write. Furnace blocks do not have a waterlogged
property, but the fluid/world owner still determines the replacement.

`LevelChunk.setBlockState` writes the section palette and updates height/light
sources before calling the old state's `onRemove`. Thus a removal callback
observes the new world palette while the old furnace block entity is still
available. New-state `onPlace` follows the old removal callback and its state
identity check.

For `AbstractFurnaceBlock.onRemove`:

1. If the new state has the same block identity, return immediately. Changing
   `lit` or facing retains the block entity and produces no destruction drops.
2. Obtain the actual old block entity. For an owned furnace on a server level,
   invoke `Containers.dropContents(level, pos, furnace)` first.
3. Invoke `getRecipesToAwardAndPopExperience` at the block center
   `(x + 0.5, y + 0.5, z + 0.5)`.
4. Invoke base `onRemove`. `BlockBehaviour.onRemove` removes the block entity
   when the old state has one and the block identities differ.
5. Request `updateNeighbourForOutputSignal(pos, oldBlock)`.

The missing/falsely typed block-entity branch only delegates to base removal;
it does not invent contents or experience.

## Container stack splitting and random ownership

`dropContents` visits actual slots in increasing index order. For a furnace
this is input, fuel, then result. Each call to `dropItemStack` computes its
position **before** checking whether that stack is empty. Empty slots therefore
still consume three draws from `Level.random`.

The item entity's width is read as a Java float and widened to double. For the
shipped item type it is `0.25`, making the spread `0.75` and half-width `0.125`.
The position for one slot is:

- X: `floor(x) + nextDouble() * 0.75 + 0.125`.
- Y: `floor(y) + nextDouble() * 0.75`.
- Z: `floor(z) + nextDouble() * 0.75 + 0.125`.

This position is computed once per slot and retained for all its split entities.
While the actual stack is nonempty, the operation is:

1. Split off `nextInt(21) + 10` items, clamped by the remaining stack count.
   The original container stack is mutated, and components remain on its split.
2. Construct `ItemEntity(level, x, y, z, split)`. That convenience constructor
   consumes two additional `Level.random.nextDouble()` draws for its initial
   X/Z velocity before delegating to the base entity constructor.
3. The item entity's own random source supplies bob phase and yaw through two
   `nextFloat()` calls after base-entity construction. Its UUID/base constructor
   state belongs to the actual entity owner, not `Level.random`.
4. Overwrite velocity using three `Level.random.triangle(mean, deviation)`
   calls, in X/Y/Z order. The means are `0.0`, `0.2`, `0.0`; the deviation is
   the exact double `0.11485000171139836`.
5. Call `addFreshEntity` before processing the next split.

The convenience constructor's initial X/Z velocity draws remain observable in
the RNG stream even though the velocity is subsequently overwritten. These
constructor calls do not use `Math.random()`. `Containers` does not call
`setDefaultPickUpDelay`; the new item entity's pickup delay remains its default
zero. A player nearby may therefore collect the dropped contents immediately.

All three furnace slots are visited, so an entirely empty furnace still consumes
nine level-random double draws through this container callback. Experience
processing follows those draws and all item entity insertions.

## Recorded recipe experience

`getRecipesToAwardAndPopExperience` iterates the actual Fastutil recipe-counter
encounter order. For each recipe still present in the recipe manager it appends
the recipe to a result list and invokes `createExperience`.

This method does **not** clear `recipesUsed`, award a player's recipe book or
trigger a player's recipe-crafted criteria. The caller discards its returned
list during destruction. These effects differ from taking a result through a
furnace menu; clearing the recipe map is part of the separate result-slot path.

Experience uses Java float multiplication of the recorded count and recipe XP,
its floor and fractional part. A nonzero fraction consumes the actual shared
`Math.random()` owner and may add one point. The resulting integer enters the
real experience-orb award path at the block center. The level/entity random
state at this point includes every earlier container-drop operation.

## Comparator notification boundary

`Level.updateNeighbourForOutputSignal` visits horizontal directions in the
source order **north, east, south, west**. It checks the adjacent position's
chunk availability. A direct comparator receives the neighbor-change callback.
Otherwise, if that adjacent state is a redstone conductor, it reads the next
state in the same direction and notifies it when it is a comparator. The
second read is not preceded by another explicit chunk-availability check in
this method. Vertical positions are not scanned here.

Unsupported comparator receivers, game-event listeners, world-query providers,
survival/tool contexts or combined item/XP transactions must remain explicit
integration boundaries. Static ordering evidence alone does not validate the
native transaction, networking, persistence or public gameplay.
