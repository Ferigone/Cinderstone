# Shipped workstation recipe order and feature context

This answers the coordinator's [exact recipe/feature request](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6563366511).
It supplies native default-data metadata and a callable adapter, not a new
RecipeManager, Level, advancement owner or production Main callsite.

## Evidence and actual data

Original Java 1.21/protocol767 inner server SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`.
Methods were inspected with direct JDK21 `javap -p -c`, not REA.

A bounded helper then called unchanged `ServerPacksSource.createVanillaPackSource`,
`MultiPackResourceManager(SERVER_DATA, singleton vanilla pack)`, actual item
`TagLoader`/registry binding, `SimpleJsonResourceReloadListener.prepare` and
`RecipeManager.apply`. Original preparation returned a Java HashMap containing
the shipped recipes. Original `getAllRecipesFor` supplied encounter order;
Rust `RecipeBook::recipe_ids` is a BTreeMap order and was not substituted.
No server, Level or world was started or fabricated. Helpers, Java artifacts
and raw disassembly remain outside Git.

The ordinary [metadata file](../../src/workstations/default_recipe_context_1_21.json)
records exact IDs/order, descriptions, actual item required-feature names,
catalog/helper/observation hashes and precise method provenance. Two independent
JVM executions produced byte-identical observations. Its SHA-256 is
`8a7f635cd5c77596151720483b8059c6d8b0beae88a43b689cb0b534896a476b`.

- The 250 stonecutting results are actual BlockItems. Their description keys
  are `block.minecraft.<name>`; no equal-description tie was observed for any
  of the 57 actual default input items. The existing
  `PinnedStonecutterRecipes<F>` therefore uses the source sort without relying
  on arbitrary recipe-ID order; ties still fail explicitly.
- Smithing has 27 actual constructor recipes. The global result list is a
  stable sort of that captured list by actual `getResultItem(...).getDescriptionId()`.
  Filtering commutes with the stable key sort. All 18 trim previews are actual
  iron chestplates and tie at `item.minecraft.iron_chestplate`, so their captured
  encounter order matters. This global list is not described as a query matching
  all recipes. Separately, 27 unchanged `getRecipesFor` queries with actual
  singleton inputs each returned their expected recipe; those default methods
  do not access their null Level argument.
- Every observed stonecutting result, smithing preview and the 30 distinct
  smithing base items require exactly `minecraft:vanilla`. Original
  `FeatureFlags.DEFAULT_FLAGS` equals `VANILLA_SET`. Original
  `FeatureElement.isEnabled` tests required flags against actual enabled flags;
  `BlockItem` delegates requirements to its block. `ItemStack.EMPTY` short-circuits
  enabled=true even with empty flags; AIR itself requires vanilla.

These are restricted unchanged-class runtime observations, not Java gameplay,
an original-server menu session or native Main integration.

## Exact native API and owner contract

[`PinnedDefaultRecipeContext`](../../src/workstations/recipe_context.rs) exports:

```rust
pub fn new(book: &'a RecipeBook, enabled: &'a BTreeSet<String>) -> Result<Self>;
pub fn smithing_order(&self) -> Result<SmithingRecipeOrder>;
pub fn result_enabled(&self, result: &ItemStack) -> Result<bool>;
// Implements StonecutterRecipes using the existing pinned stonecutter adapter.
```

Construction requires the actual canonical `RecipeBook::vanilla()` object,
checks the compiled recipe/item catalogs against the observed SHA-256 values,
and requires the borrowed actual enabled-feature snapshot to equal exactly
`{minecraft:vanilla}`. It supplies the recorded constructor and stable-result
orders to `SmithingRecipeOrder::new`. It copies no recipe registry, inventory,
result preview or Level state. Required features are observed metadata, not an
always-enabled closure; uncertified nonempty result items fail explicitly.

Main must acquire its canonical world/player/menu owners and pass its real
immutable feature snapshot under the existing operation/lock contract.
The current native configuration fixture contains an actual feature-flags
packet whose decoded names are exactly `{minecraft:vanilla}`; its SHA-256 is
`4e862933c355b2adae68b9f57fb34bc652be104fb1cb929e53ea3b38213a5e37`.
Current `Fixtures` retains configuration packets, not a canonical live Level
feature owner. Central integration must bind/retain that actual profile state;
the adapter does not create it from a guessed set or silently replace reloads.

Custom packs, an independently deserialized recipe book, alternate registries,
experimental feature sets and resource reloads are outside this certified
snapshot. They require their actual manager/registry/features and ordering.
Stats, recipe-book awards, criteria, sound and drop callbacks remain mandatory
actual owners. No no-op implementation is supplied by this data context.

Focused native checks compose the observed context with real existing
stonecutting assembly and smithing transform state, verify enabled results and
empty-stack semantics, and reject alternate feature/catalog owners and an
uncertified result item. The independent code/data review found no concrete
defect within this bounded default profile; full Main validation is separate.
