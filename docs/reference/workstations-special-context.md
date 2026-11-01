# Workstation recipe context and ownership

This stage adds callable native adapters, not a saved-map owner or Main wiring.
The fixed `inventory.4`, `crafting.2` and `crafting.5` aggregate definitions remain
unchanged. Existing 2×2/3×3 recipe evaluation is reused.

## Primary evidence

The original Java **1.21** server was downloaded from Mojang's version manifest.
The outer JAR SHA-1 is `450698d1863ab5180c25d7c804ef0fe6369dd1ba`;
its inner JAR SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official server mappings SHA-1:
`31c77994d96f05ba25a870ada70f47f315330437`.

Behavior below was inspected with **JDK 21.0.12.1 `javap -p -c`**, invoked through
the installed `jdk.jdeps/com.sun.tools.javap.Main` module. No REA tool was
available for this stage. This is static bytecode inspection, not a Java server
runtime observation. Proprietary artifacts and raw disassembly remain outside
the repository.

## Recipe selection and smithing predicates

`RecipeManager` (`czd`), mapped lines 81–108, has distinct ordering contracts:

- `getRecipeFor` returns empty for an empty input, checks a supplied last holder
  first, then finds the first matching holder in the type's encounter order.
- `getAllRecipesFor` copies the type's holder collection without an extra sort.
- `getRecipesFor` filters matching holders and stably sorts by each recipe's
  `getResultItem(registryAccess).getDescriptionId()`. This is not recipe-ID order or merely raw
  manager encounter order.
- Loading/replacing recipes populates immutable multimaps in input encounter
  order. A BTreeMap of recipe IDs is not evidence of that original order.

`RecipeManagerView` requires a caller-supplied captured type list; it never
invents manager ordering from the existing recipe book's sorted storage.
`get_recipe_for` preserves last-holder preference and a matched recipe with
empty assembly, such as an already-applied smithing trim. The view models
`getRecipeFor`; it is not the stonecutter's sorted `getRecipesFor` operation.

The additive `RecipeBook::smithing_accepts_input` accessor exposes each captured
recipe's template/base/addition predicates. `SmithingTransformRecipe` (`czt`),
mapped lines 45–55, and `SmithingTrimRecipe` (`czu`), lines 74–84, test those
ingredients independently. The three aggregate helpers apply only to the pinned
default catalog; a menu with another captured recipe list must iterate that list
itself, retaining overlap and encounter-order semantics.

## Map recipe lookup

`MapExtendingRecipe` (`cyy`), mapped lines 32–56, first checks the shaped
paper/filled-map pattern, obtains the map's actual saved data, then rejects a
missing map, an exploration map or scale at least four. Locked status is not
an additional condition in this recipe. Assembly copies the filled map with
count one and sets `MAP_POST_PROCESSING` to SCALE; it does not allocate or alter
saved map data during preview.

`context_for_map_extension` performs the shaped check before asking the borrowed
`MapRecipeLookup` owner for the map ID. `None` certifies an actual absent map;
an unavailable owner must return an error. No dummy map is inserted. The
resulting `RecipeContext` snapshot feeds the existing evaluator.

## Taken-map postprocessing

`MapItem.onCraftedPostProcess` (`cvc`, lines 338–366) removes the pending
postprocessing component first. If no operation was pending, it returns. For
LOCK or SCALE it reads the actual source map; a missing map leaves the component
removed without allocating a replacement. When saved data exists, the Level
allocates a new map ID, stores the locked/scaled copy, then replaces the output
stack's map ID. It does not modify the source map in place.

`postprocess_crafted_map` handles only a taken filled-map result. It removes the
pending flag, delegates saved-data lookup/ID allocation/storage to a borrowed
`CraftedMapOwner`, and installs the returned ID. An absent map-ID component
skips the owner entirely. Errors leave the stack uncommitted; the enclosing
inventory transaction must also stage the real world owner. Preview must never
call this function. Item crafted/stat/recipe/advancement callbacks remain the
real player's responsibility and are not synthesized here.

## Integration boundary and validation

There are no new Main callsites in this stage. Production must supply actual
recipe-manager snapshots and map owners in the same transaction as the menu.
The existing crafting-table path deliberately rejects map postprocessing until
that world owner is connected. Dynamic recipe reload, custom trim registries,
map persistence/replication and advancement ownership are not supplied by this
module. Central integration retains existing player/container/shared-world
files.

Focused checks cover shaped-before-world-lookup ordering, certified missing-map
nonmatching, taken-result flag consumption/new-ID installation exactly once,
and captured manager order versus cached-holder preference. Compiler/runtime
results and exact commits belong to the issue/PR handoff after they run; this
source note does not itself claim they passed.
