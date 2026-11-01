# Smithing menu source ledger

Target: Minecraft Java 1.21, protocol 767. Primary evidence is direct JDK 21
`javap -p -c` inspection of the unchanged shipped server and the matching official
Mojang mappings. No REA execution, original-server menu execution or live native
Main call is claimed by this ledger.

- Outer server SHA-1: `450698d1863ab5180c25d7c804ef0fe6369dd1ba`.
- Inner server SHA-256: `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
- Official server mappings SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`.
- Inspected classes: `SmithingMenu` (`crt`), `ItemCombinerMenu` (`cqx`, including
  anonymous result/input containers), `RecipeCraftingHolder` (`crk`),
  `ResultContainer` (`crl`), `SmithingTransformRecipe` (`czt`) and
  `SmithingTrimRecipe` (`czu`), `AbstractContainerMenu` (`cpu`), `Slot` (`crq`)
  and player `Inventory` (`cmw`). Raw disassembly and Java artifacts stay outside
  the repository.

## Source facts and bounded implementation

`SmithingMenu.createInputSlotDefinitions` supplies template, base and addition
slots 0, 1 and 2 and a non-insertable result slot 3. Menu type is 21; player main
inventory/hotbar occupy menu slots 4 through 39. Slot predicates test any recipe
in the constructor's captured `getAllRecipesFor(SMITHING)` list.

`createResult` uses the first matching holder from `getRecipesFor`; that method
has a different ordering contract from the constructor's list. The native API
requires both ordered ID lists from the real recipe manager. It does not infer
Java order from the recipe fixture's sorted map. `SmithingRecipeOrder` resolves
only known shipped serializers and requires both lists to describe the same
snapshot. Unknown recipes fail explicitly. Dynamic recipe reload, custom recipe
components, alternate feature sets and alternate trim registries require their
real owners and are outside this pinned default catalog implementation.

`mayPickup` checks the selected recipe against current inputs and ignores its
`hasItem` argument. Identical existing trim still matches a recipe but assembles
an empty stack. The native implementation keeps that distinction and additionally
requires nonempty result contents to prepare a take. On no match the original
clears only result contents; stale selected-holder fields are harmless because
pickup rechecks the inputs.

Transform assembly uses `transmuteCopy` followed by the recipe result component
patch. The shipped transform results carry no additional patches; the existing
recipe evaluator therefore preserves the base patch and changes its item
prototype/count. Trim assembly resolves material and pattern from actual registry
entries, rejects an identical pattern/material, and otherwise copies one base
item and replaces its trim component. This stage uses the existing evaluator's
captured default registry data; it does not create a live registry owner.

`onTake` orders its effects as follows:

1. Call output `ItemStack.onCraftedBy(level, player, stack.getCount())`.
2. Trigger recipe-crafted using the three actual pre-consumption inputs, award
   the non-special recipe, and clear `recipeUsed`.
3. Shrink input 0, then 1, then 2 by one. Every nonempty input is installed with
   `SimpleContainer.setItem`, synchronously invoking its `setChanged` callback.
   `ItemCombinerMenu.slotsChanged` first calls superclass
   `AbstractContainerMenu.slotsChanged`, which **broadcasts changes**, and only
   then invokes `createResult` before the next decrement. The broadcast sees
   already-decremented inputs and the **previous** derived result. Smithing
   does not use crafting recipe remainder items here.
4. Execute world level event 1044, data 0, at the captured block position.

`ItemCombinerMenu.quickMoveStack` transfers result into player inventory in
reverse order, then passes the now emptied result stack to `onTake`. With the
shipped one-item results, its crafted amount is **zero** on this branch. There is
no `ResultSlot` quick-craft counter. The native take plan records this explicitly;
normal pickup records the one-item output. Plans are one-shot and check their
original state before replacing it, so a second stale plan cannot consume a
newer input state. Callback arguments and the result preview are not owned drops.

`SmithingEffect::BeforeCreateResult` records those exact input/previous-preview
observations. The inventory adapter consumes them through shared `MenuSync`
at their source positions rather than batching all input/result observations
after consumption. For ordinary `Slot.set`, the container's setItem callback
and the following `Slot.setChanged` both invoke this path. `moveItemStackTo`
merges into an existing slot with just `Slot.setChanged`; insertion into an
empty slot uses `Slot.set`. Input removal likewise preserves the remove callback,
empty-slot installation, ordinary input onTake and final pickup slot callback.

`findSlotToQuickMoveTo` visits captured recipes in order. Within each recipe it
checks template before base before addition, then accepts only an **empty**
matching input slot. An occupied matching slot is not a quick-move merge target.
`canTakeItemForPickAll` excludes the result container.

`ItemCombinerMenu.removed` first handles the actual cursor via the base callback,
then returns/drops only the three input slots. The result is a derived preview.
`stillValid` checks the real smithing block and
`player.canInteractWithBlock(blockPos, 4.0)` through captured level access.
The native state exposes input draining; cursor, real returns/drops, reach,
server-player validity phase and state-ID/listener synchronization belong to the
shared menu adapter and the central Main integration.

## Validation and production boundary

Focused native checks exercise patch-preserving transform consumption, the
pre-consumption criterion inputs, the zero-count quick-move callback, stale-plan
rejection and identical-trim/no-result behavior. Compiler/check results and exact
commits are recorded by the workstation coordinator. These checks do not claim
graphical client, original-server oracle execution or production Main integration.
The broad `inventory.4`, `crafting.2` and `crafting.5` deliveries remain partial.

## Bounded packet adapter contract

`src/container_workstation_smithing.rs` connects the state to borrowed canonical
`PlayerInventory` and shared `MenuSync`. `SmithingMenu.prepare_click` accepts
all seven protocol click modes and the actual player creative flag, prepares
cursor/slot changes and ordered smithing callbacks,
and computes prediction/state-ID synchronization without mutating live state.
Its plan checks the complete menu snapshot, nonreused UI instance identity, checked
monotonic revision and borrowed inventory before staging and again before commit.
The host inspects the single ordered effect/sync event stream and drops, stages actual item/player/world owners,
commits once, and dispatches the ordered sync events. The adapter never awards
recipes, advances criteria, spawns items or selects a world position itself.

The adapter supports left/right pickup, cursor outside drops, ingredient shift
routing, repeated main/hotbar transfers, reverse output insertion and repeated
smithing while the same result item remains available. Full player inventory
leaves inputs and callbacks untouched. SWAP covers hotbar and actual offhand,
including input slot limits and inventory-return/overflow behavior; output swap
to an empty hotbar/offhand installs that output before `onTake`. Creative CLONE
copies the preview without ingredient consumption or smithing callbacks; the
actual creative flag gates it. THROW consumes a result normally, then records
its actual drop after the crafted/recipe/world callbacks. QUICK_CRAFT preserves
the source drag state machine, single-target pickup branch and creative fill
mode. PICKUP_ALL uses two passes, skips overstacked sources and excludes the
derived result container. Bundle item-click overrides still require their real
feature/component owner and fail before live mutation.

Pickup installs/grows the actual menu cursor before result `onTake`; output
hotbar/offhand swap installs its destination first. Input insertion and swaps
also shrink/change their actual cursor or player source before the nested
input callbacks. Intermediate local listeners therefore observe the source
inventory at the correct phase, without treating a transfer proposal as a
second owned stack. Remote packets remain suppressed during authoritative
click execution; client predictions only update the remote baselines afterward.

`prepare_close` stages the cursor first and owned input slots in order, using the
actual alive/disconnected context supplied by Main. It excludes the preview.
The host must publish a server-initiated close packet before those return/drop
effects and transfer the old shared inventory baselines afterwards. The adapter
prepares cursor and each input separately so raw inventory insertion packets and
overflow drops retain their actual per-stack interleaving. The one event stream
includes `Drop { index }` references into the prepared drop list; each actual
drop is staged/executed once at that position.

Focused adapter checks cover actual native menu actions against a borrowed
inventory, output placement and zero-count callback, no-capacity refusal,
disconnected cursor/input conservation, stale inventory rejection and stale-ID
full-state synchronization. Later focused flows exercise drag/pick-all item
conservation, offhand and result hotbar swap, creative clone gating, ordered
result throw, repeated overstacked main/hotbar transfers, result exclusion from
pick-all, and mixed alive raw insertions/overflow drops. A focused ordering flow
also observes the previous result as EMPTY, recreated output, then EMPTY across
the three input decrements, between recipe awards and level event1044; it checks
that an ingredient shift has already emptied its actual player source before
the input/result listeners run. This is an in-process native transaction check,
not a protocol socket/client or original-server gameplay observation. The
central Main block-open, packet dispatch, callback owners and player-phase
validity/close callsites remain pending the integration contract.

The adapter's callback contract is bounded to actual owners whose recipe and
item callbacks do not reenter and mutate this inventory/menu during an action.
The host must reject unsupported advancement-reward/reentrant contexts before
staging; it must not ignore their effects. `ItemCombinerMenu.createResult` itself
does not broadcast, but its preceding superclass slotsChanged **does**. Local
slot/listener observations are retained at every source boundary, including
between input decrements, while suppressed remote changes wait for final click
synchronization. These observations support non-reentrant actual owners; they
do not execute mutating advancement or custom listener code against a later
final-state snapshot. The ingredient quick-move transfer range is the selected starting
input through the exclusive result slot, exactly as in the source base menu.
