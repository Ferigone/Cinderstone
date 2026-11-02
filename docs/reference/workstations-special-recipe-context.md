# Registry-aware repair and trim recipe context

Target: Minecraft Java **1.21**, protocol **767**.
[Exact team claim](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564372851).
This is a callable context layer and a bounded repair-component correction.
It does not create a recipe manager, registry, Level, advancement owner, extra
result inventory or new Main callsite.

## Pinned primary evidence

The unchanged inner server JAR has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings have SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
The method evidence here is direct **JDK 21 `javap -p -c`**, not REA.
No Java artifact or raw disassembly is committed.

| Actual mapped method | Original | Source fact |
| --- | --- | --- |
| `RepairItemRecipe.getItemsToCombine/canCombine`, 22–47 | `czg` | Exactly two nonempty count-one stacks, same Item, both MAX_DAMAGE and DAMAGE present; this is not an isDamageable/unbreakable check |
| `RepairItemRecipe.assemble`, 57–87, and its lambdas 79–84 | `czg` | Fresh output ItemStack; larger maximum damage, summed remaining durability plus five-percent integer bonus; actual enchantment-registry holders filtered by CURSE, maximum input level retained |
| `EnchantmentHelper.getEnchantmentsForCrafting/getComponentType`, 77–81; `updateEnchantments`, 56–65 | `dae` | Only actual enchanted books select STORED_ENCHANTMENTS; other items select ENCHANTMENTS; update the new output component and clamp mutable levels |
| `SmithingTrimRecipe.matches/assemble`, 36–58 | `czu` | Ingredient match is separate from assembly; missing material/pattern or an identical existing trim yields empty assembly |
| `TrimMaterials.getFromIngredient`, 47; `TrimPatterns.getFromTemplate`, 60 | `cwz/cxb` | Actual provider registry streams, first matching holder in encounter order; both lookups precede testing the two Optionals |
| `ArmorTrim.hasPatternAndMaterial`, 83 | `cwx` | Actual pattern/material holder equality, ignoring tooltip visibility |
| `Recipe.getRemainingItems`, 31–40 | `cyz` | Each Item's hasCraftingRemainingItem/getCraftingRemainingItem, constructing its default count-one stack |

## Native API and actual owners

New [special_recipe_context.rs](../../src/workstations/special_recipe_context.rs)
exports `SpecialRecipeRegistry`, `SpecialRecipeContext<'a, R>` and
`CraftingEnchantments::{Ordinary, Stored}`. The additive
[RecipeBook entry point](../../src/crafting.rs) is:

```rust
pub fn evaluate_with_special_registry<R: SpecialRecipeRegistry>(
    &self,
    id: &str,
    input: &RecipeInput,
    context: &SpecialRecipeContext<'_, R>,
) -> Result<Option<CraftResult>>;
```

This entry point supports **repair-item and smithing-trim serializers only**.
An unsupported serializer errors instead of falling back to captured metadata.
`None` means ingredient/input mismatch. `Some(CraftResult)` with an empty stack
retains a successful trim match whose actual assembly is empty.

Construct the context with `SpecialRecipeContext::new(&actual_registry_owner)`.
Its required registry interface has no default implementation. It supplies:

- An actual snapshot key plus validation that the RecipeBook's ingredient/item
  identities belong to the same captured recipe/item context.
- Actual enchantment holders, CURSE membership, selected typed-component levels
  and real holder/map component encoding.
- Actual trim material and pattern holder encounter lists and their Item
  predicates. Native loops select the first matching holders. Existing-trim
  equality and trim component encoding remain with that actual holder owner.
- Actual Item crafting-remainder properties and default result stacks.
  Food `usingConvertsTo` is a consumption rule, not a crafting remainder.

Registry queries are read-only and consume no player/Level RNG, callbacks or
world effects. Genuine absent matching holders produce empty trim assembly.
An unavailable provider, unknown item/component/holder, replaced context or
uncertified owner must return an error. Actual CURSE membership may differ from
the two shipped curse assignments; no hardcoded curse list backs the new route.

The context checks its captured snapshot before binding/queries and after the
computed output/remainders. It does not publish a result if the snapshot changes
during evaluation. The key must identify actual owner identity and generation;
identical registry contents do not certify a replacement owner. A later menu
transaction must retain that key and revalidate before committing, rather than
treating a returned CraftResult as a self-validating ownership token.

Native assembly performs the source arithmetic and component selection. Repair
creates a fresh item and does not copy unrelated input patches. It reads
component **23** for enchanted books and **9** otherwise, filters actual registry
holders by CURSE and keeps maximum positive levels through the actual component
encoder. The fresh output's tooltip setting is true. Trim copies the base with
count one, preserves unrelated components, uses owner equality and installs the
new trim with tooltip true. An existing identical trim with tooltip false still
assembles empty.

Remainders preserve one position per supplied input slot. Nonempty Item-property
remainders must have count one; empty/AIR inputs cannot invent a remainder.
No input is consumed and no output becomes owned merely by evaluating. The
owning menu still has to reserve the actual destination and run its source
callbacks/consumption transaction.

Input encoding uses the current shared item codec. Unknown holder IDs or
component forms outside that codec's supported registry/context still fail
explicitly; this change does not implement arbitrary datapack loading or replace
the shared codec registry. Surviving custom FoodProperties overrides remain
unsupported because wire values do not retain nested ItemStack reference
identity through a copy. Stream-codec-proved prototype elisions remain admitted.
Actual trim reference/direct equality is resolved by the mandatory trim owner,
not inferred from equal numeric IDs or equal wire bytes.

## Existing pinned-default entry points

`RecipeBook::evaluate` and `evaluate_with_context` remain compatibility entry
points using captured shipped-default curse and trim metadata (plus the existing
caller map context). They do not gain a live registry owner or reload support.
Their existing repair branch now also chooses stored component **23** for
enchanted books, rather than always reading/writing normal component **9**.
All unrelated matching/serializers, player/container foundations and Main files
remain outside this patch.

The specific correction is source-derived: two count-one enchanted books with
MAX_DAMAGE 100, DAMAGE 60 and stored curse levels one/two meet `czg.canCombine`.
Java's method structure requires damage 15 and stored curse level two, ignoring
an unrelated normal-enchantment component. This stage did **not** execute that
modified-book case in Java and does not describe it as an oracle observation.

## Focused native verification and remaining integration

The explicit native fixture owns its test holder handles, tag assignment,
snapshot and encounter order. It is not a production registry or Java world.
Its repair flow exercises the modified-book correction, a changed actual CURSE
assignment, selected-component/tooltip behavior, unchanged inputs and real
Item-property milk-bucket remainders. Its trim flow gives two matching holders
in deliberate encounter order, checks first selection and preserved base data,
and distinguishes nonmatch, missing-holder empty assembly and identical trim
with different tooltip visibility. Unavailable registry access and a snapshot
change error instead of using static defaults; other serializers are rejected.

These are source-derived native checks. The older captured special-recipe
vectors retain their separately recorded provenance; no new original-Java
execution or live client/server result is implied here.

No Main caller, actual registry implementation, recipe-manager reload owner,
dynamic tag-backed stew/dye/decorated-pot context or complete recipe delivery is
claimed. The central [take-phase/remainder request](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564373039)
remains separate: ResultSlot's actual crafted/recipe callbacks, then current
manager remainder query, canonical input aliases and mutation/drop phases must
be integrated by their real owners. This evaluator does not rewrite that
lifecycle or fabricate its advancement/recipe-book owner. `inventory.4`,
`crafting.2` and `crafting.5` remain aggregate incomplete.
