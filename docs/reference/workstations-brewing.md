# Brewing stand: Java 1.21 state machine and owner contracts

Primary evidence is the unchanged Java **1.21** inner server JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
with official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`.
This stage directly inspected **JDK 21 `javap -p -c`** output for
`BrewingStandBlockEntity` (`dqk`), its data adapter (`dqk$1`),
`BrewingStandMenu` (`cqa`), its fuel/ingredient/potion slots (`cqa$a/b/c`),
`PotionBrewing` (`cwt`) and `BaseContainerBlockEntity` (`dqb`). No REA inspection,
Java server-tick execution or gameplay observation is claimed. A separate small
direct item-method observation is labeled below. Raw disassembly
and Java artifacts remain outside the source checkout.

## Tick ordering

The native `BrewingState` owns all five real slots: bottles 0–2, ingredient 3,
fuel 4. There is no derived result inventory or second potion owner.
`ContainerData` indices are brew time **0** and remaining fuel uses **1**.

`dqk.serverTick` performs these operations in order:

1. If fuel is nonpositive and slot four is blaze powder, consume one powder,
   set fuel to **20**, and call `setChanged`. This runs even without a recipe.
2. Ask the actual Level's `PotionBrewing` whether the ingredient is recognized
   and any nonempty bottle has a mix.
3. An active brew first decrements its time. At zero, a currently valid recipe
   completes. Otherwise, losing brewability or changing the ingredient **item**
   cancels it. Component or count changes alone do not change the cached item.
   The completion branch precedes identity cancellation: switching to a different
   valid ingredient on the final tick can complete using that ingredient.
4. Every tick that began active calls `setChanged`, including ordinary countdown
   ticks. An idle, brewable stand with fuel consumes one fuel use, sets time to
   **400**, captures the ingredient item, and calls `setChanged`.
5. Compare all three nonempty bottle bits with the transient previous bits. On
   change, cache them and, only if the captured block is a brewing stand, update
   all `HAS_BOTTLE` properties with flags **2**. This also runs on the first tick
   after construction/load because the previous bits are initially absent.

Completion calls the actual `PotionBrewing.mix(ingredient, bottle)` for all three
slots, including empty or unchanged bottles. It then consumes one ingredient.
The remainder query occurs **after shrinking**: `ItemStack.getItem` returns AIR
when the count is zero. Consequently, exhausting the final dragon breath yields
no glass-bottle remainder in this exact artifact. When dragon breath remains,
its fresh static `Item.craftingRemainingItem` enters real
`Containers.dropItemStack` at integer block x/y/z before Level event **1035**,
data **0**. The engine reuses the existing pinned static remainder catalog
through `smelting::fuel_remainder`, queried against the post-shrink stack; that
helper accepts all registered items, despite its furnace-oriented name. It
transfers no input component patch to the remainder.

This quirk is supported by `dqk.doBrew` offsets 52–78 and `cuq.getItem`'s
empty-stack branch. A temporary reflection harness additionally called the
unchanged `ItemStack.shrink/getItem/Item.hasCraftingRemainingItem` methods:
starting with one dragon breath returned count zero, empty true, remainder false;
starting with two returned count one, empty false, remainder true. This is a
small direct item-method observation, not an executed brewing-world tick.

The mix owner must reproduce the source's assembly. In particular, `cwt.mix`
creates a fresh result with the new base potion for successful container/potion
mixes; arbitrary input custom color/effects/components are not automatically
copied. This engine delegates assembly to the actual owner instead of inventing
potion holder IDs, a default recipe catalog or feature-flag state.

## Slot, automation and persistence facts

Menu type is **11**. The five block slots precede player inventory indices 5–31
and hotbar indices 32–40. Potion slots accept potion, splash potion, lingering
potion or glass bottle, with maximum **one**. The ingredient slot asks the real
`PotionBrewing.isIngredient`; the fuel slot accepts blaze powder. The block
entity additionally requires a bottle slot to be empty for placement. The base
container mutation clamps to the item's maximum and the container maximum **99**;
it does not apply the menu predicate itself.

Up exposes `[3]`, down `[0, 1, 2, 3]`, sides `[0, 1, 2, 4]`. The face list is a
separate transfer-owner check. Extraction from ingredient slot three only accepts
a glass bottle; other exposed slots have no additional extraction predicate.
These rules do not implement a hopper transfer/scheduler owner.

On taking a bottle, a present potion holder triggers `BREWED_POTION` for an
actual `ServerPlayer`, before ordinary `Slot.onTake`. The helper
`potion_take_criterion` extracts that holder for the owning advancement adapter;
it does not invent or award an advancement.

Java saves brew time as a signed short and fuel as a signed byte, alongside
actual container slots. Load restores those widths, reconstructs the ingredient
item cache only when time is positive, and does not restore previous bottle bits.
`BrewingSaved`/`from_saved`/`save_values` preserve these facts; they are not wired
to the native block-entity registry or checkpoint format in this stage.

## Callable native boundary and verification

`prepare_tick(actual_mix_owner, captured_block_is_brewing_stand)` computes a
single-use before/after plan and ordered `BrewingEffect`s. Dirty effects retain
the slots/counters observed at that exact callback. `commit_tick` rejects a
changed state and asks `BrewingTickHost` to atomically commit the real world
effects before publishing the new state. Its contract requires captured block-entity
identity, actual item/RNG/entity owners for a remainder drop, real dirty and
comparator callbacks, event publication and property updates in source order.
A failed transaction must publish neither the effects nor the new state.
The caller must bind each plan to the actual captured block-entity owner key or
epoch; structural state equality alone cannot distinguish an identical newly
placed container at the same coordinates.

Focused native scenarios cover idle refueling, the 400-tick countdown, ordinary
ingredient abort, final-tick ingredient switching, the remaining-versus-exhausted ingredient
remainder distinction, signed save values, stale plans and rejected world transactions.
Their explicitly labeled fixture mix and recording host test the state engine;
they establish neither a production potion recipe owner nor live world behavior.

Main, native block-entity registration/ticking, menu clicks and synchronization,
close/disconnect, real potion recipes, feature flags, actual remainder Items,
advancements, comparator receivers and restart integration remain central-owner
contracts. Brewing menu closure returns its cursor; its five persistent contents
remain with the captured block entity. This bounded stage completes neither
`inventory.4` nor `crafting.5`.
