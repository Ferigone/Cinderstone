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
  `SmithingTrimRecipe` (`czu`). Raw disassembly and Java artifacts stay outside
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
   `SimpleContainer.setItem`, synchronously invoking `createResult` before the
   next decrement. Smithing does not use crafting recipe remainder items here.
4. Execute world level event 1044, data 0, at the captured block position.

`ItemCombinerMenu.quickMoveStack` transfers result into player inventory in
reverse order, then passes the now emptied result stack to `onTake`. With the
shipped one-item results, its crafted amount is **zero** on this branch. There is
no `ResultSlot` quick-craft counter. The native take plan records this explicitly;
normal pickup records the one-item output. Plans are one-shot and check their
original state before replacing it, so a second stale plan cannot consume a
newer input state. Callback arguments and the result preview are not owned drops.

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
