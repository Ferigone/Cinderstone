# Special item recipe context: stew, decorated pot and armor dye

Target: Minecraft Java **1.21**, protocol **767**. This stage supplies an
additive, owner-backed recipe evaluation route for three special recipes.
Matching and assembly do not consume inputs, own a taken result or apply effects.
The existing captured-default recipe branches remain separate.

## Primary evidence

The unchanged inner server JAR has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings have SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Evidence is direct **JDK 21 `javap -p -c`** inspection. REA was unavailable.
This is static inspection, not execution of these recipes in Java. No Java
artifact or raw disassembly is committed.

| Mapped method | Original | Source rule |
| --- | --- | --- |
| `SuspiciousStewRecipe.matches`, 19–43 | `czx` | Row-major scan; brown mushroom, red mushroom, SMALL_FLOWERS and bowl are tested in that order with one flag per category; another nonempty input rejects |
| `SuspiciousStewRecipe.assemble`, 48–63 | `czx` | Fresh count-one stew, then first nonempty input whose Item resolves to a SuspiciousEffectHolder supplies its effects; assembly does not repeat the flower-tag predicate |
| `SuspiciousEffectHolder.tryGet`, 22–28 | `dob` | A BlockItem's actual block is checked first, then the actual Item itself |
| `DecoratedPotRecipe.matches/assemble`, 18–47 | `cys` | Exactly 3×3; slots 1/3/5/7 use DECORATED_POT_INGREDIENTS, every other slot must resolve to AIR; assembly passes those four Items in that order |
| `PotDecorations` constructors and `ordered`, 26–38/52 | `drm` | Order is back/left/right/front; actual BRICK identity becomes an absent decoration and absent entries read back as BRICK |
| `DecoratedPotBlockEntity.createDecoratedPotItem`, 98–100 | `dqx` | Obtain the actual pot Item's default instance, then set POT_DECORATIONS |
| `ArmorDyeRecipe.matches/assemble`, 20–72 | `cyi` | DYEABLE tag takes precedence over DyeItem classification; one dyeable input and at least one actual DyeItem; assembly copies the selected stack before applying dyes |
| `DyedItemColor.applyDyes`, 48–101 | `cxk` | Recheck DYEABLE, copy with count one, combine existing color and actual DyeColor texture colors, retain the existing tooltip flag or default true |
| `ItemStack.copy/copyWithCount`, 539–553 | `cuq` | Assembly's first copy preserves count; applyDyes performs another copy and sets count one |
| `Recipe.getRemainingItems`, 31–40 | `cyz` | Each actual Item's crafting-remainder property constructs its default remainder stack |

Stew's matching flag loop is significant when an actual custom tag overlaps
another category: it is not an independent set-membership/count calculation.
Assembly can also select a holder from an earlier ingredient than the matched
flower. A genuine absent holder leaves the fresh stew's effects unchanged;
unavailable owner access is an error rather than evidence of absence.

Armor color calculation sums red/green/blue channels and each color's maximum
channel using Java integer arithmetic. It divides channel sums by the color
count as integers, computes mean brightness and rescaling in `float`, then uses
Java float-to-int conversion. The final ARGB construction uses alpha **0**;
input alpha is not averaged. This uses DyeColor's actual texture diffuse color,
not a firework color, color-name heuristic or another fixed table.

## Actual context and component ownership

The new [special_item_context.rs](../../src/workstations/special_item_context.rs)
requires the actual recipe/item/tag/component owner and one captured snapshot.
Snapshot identity must distinguish the owner and its generation, not merely
equal registry contents. Validation binds the RecipeBook's decoded identities
and every queried Item/component to that same context. A changed or replaced
context must error before evaluation publishes its result.

```rust
let context = SpecialItemRecipeContext::new(&actual_owner)?;
let result: Option<CraftResult> = context.evaluate(&book, recipe_id, &input)?;
```

`None` means input mismatch; a matched recipe can retain an empty assembly in
`Some(CraftResult)`. An unsupported serializer or unavailable owner returns an
error, never a captured-default fallback. This entry point supports
only suspicious stew, decorated pot and armor dye. It follows each actual
`matches` method; the separate `canCraftInDimensions` advertisement does not
invent an extra matching restriction. For example, stew's flag loop can match
four ingredients in a 1×4 input, without claiming that a native menu opens such
a grid.

`SpecialItemRecipeOwner` is the mandatory provider interface. Its opaque
`Item`, `DyeItem` and `StewEffects` handles retain actual identity; its `Snapshot`
binds all those handles to the same captured context. Queries are read-only,
nonreentrant and must not consume RNG or run callbacks. Unsupported contexts error
instead of invoking the captured-default evaluator. The context uses the
RecipeBook's public recipe-type accessor; no existing crafting evaluator branch
is changed.

The provider owns actual SMALL_FLOWERS, DECORATED_POT_INGREDIENTS and DYEABLE
membership, Item/BlockItem class relationships, SuspiciousEffectHolder effects,
DyeItem/DyeColor values, Item defaults and Item crafting-remainder properties.
There is no production default registry implementation or static fallback on
this new route. A genuine negative predicate is different from a missing owner.
`is_item` resolves `SourceItem` constants; `is_in_tag` resolves `SpecialItemTag`.
`suspicious_effects`, `dye_texture_diffuse_argb` and `dyed_color` query those
actual typed objects. `fresh_output` distinguishes a fresh stew from the pot's
default instance. `encode_pot_decorations` runs before obtaining that pot
instance, and component 51 is then set even for four ordinary bricks.

SUSPICIOUS_STEW_EFFECTS is typed component **32**. Its ordered effect holders and
durations must be encoded against the actual captured effect registry; an equal
numeric ID in another snapshot does not certify the same holder. This component
is recipe data, not an instruction to mutate a player's live effect map.
DYED_COLOR is component **24** and POT_DECORATIONS is component **51**. Pot
decorations retain the four actual Item identities and their ordered optional
form, rather than treating four raw IDs as an independent registry.

Armor's `copy_stack` preserves the first source count; `copy_with_count_one`
performs the second count-one copy. Both require an actual copy-identity
certificate and a structural check. A structural check by itself cannot supply
that certificate. Copying must preserve unrelated typed components and their
actual copy/reference semantics. Equal encoded bytes alone do not prove
the identity of nested ItemStacks, direct holders or other referenced objects.
An unsupported copy context must fail explicitly; cloning a wire observation
does not supply those owners. Fresh stew and pot results likewise require the
actual Item default instance rather than an invented empty component profile.
The current shared item codec and prototype-normalization catalog remain an
additional admission boundary; an owner-supplied dynamic handle alone does not
make an arbitrary numeric Item ID or component form representable.
The shared component-context guard still rejects unresolved explicit
FoodProperties component **20** reference identity; it does not strip the
override or substitute a default to make evaluation succeed.

## Match, assembly and take are separate phases

A successful match permits assembly; it does not consume a recipe or transfer
the preview. Remainders have one entry per actual input slot and come from the
Item crafting-remainder property, not FoodProperties `usingConvertsTo`.
Evaluation leaves canonical inputs unchanged. The actual owning menu must
revalidate the retained snapshot and current inputs before committing a take.

The take owner still supplies destination/cursor capacity, current recipe-manager
remainder lookup, canonical input aliases, consumption, remainder insertion and
drop phases, plus the actual crafted/recipe/stat/advancement callbacks in source
order. This evaluator supplies none of those callbacks, no second result
inventory and no world or player effects. It does not replace the existing
2×2/3×3 menu lifecycle. The preview's remainder vector is not a take-time cache:
the actual manager remainder query belongs after the source result callbacks.

## Validation and remaining integration

Root validated frozen source SHA-256
`773584f063e0d2d743f6d4bf754e7c98fa3cf75c9e449985ec9b42cac02048bf`
against base `68fba409`: isolated all-target compilation passed, followed by
the focused native stew/pot/dye flows. These exercise ordered role overlap and
the first nonflower effect supplier, actual-AIR and pot direction queries, dye
tag precedence and both copy counts, retained tooltip/color, unavailable-copy
refusal and same-content owner replacement. An independent primary/API review
compared the same frozen source with the pinned bytecode and found no concrete
blocker. Formatting and whitespace checks passed.

Controlled native holder/tag/copy recorders are contract fixtures, not an
actual configured registry, Java recipe execution, Main call or gameplay
witness. Their success does not create an actual provider or take owner.

No actual registry provider, Main integration, dynamic resource-reload owner,
input consumption, result take or world/effect application is added here.
The existing captured-default paths are not upgraded implicitly by this new
context. `inventory.4`, `crafting.2` and `crafting.5` remain incomplete aggregate
deliveries; these three recipe serializers do not complete any all-menu row.
