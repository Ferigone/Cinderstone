# PotionBrewing source and actual-owner boundary

This source stage supplies the brewing recipe algorithm and ordered vanilla
registration keys under [issue3/6566249731](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6566249731).
It does not replace the existing [brewing stand engine/controller](workstations-brewing.md),
register a block entity, supply its ticker or implement potion consumption,
projectiles, living effects or persistence.

## Evidence

Primary evidence is direct JDK21 `javap -p -c` inspection of original Java1.21
`PotionBrewing` (`cwt`), its Builder (`cwt$a`), `PotionContents` (`cwu`) and
`BrewingStandBlockEntity` (`dqk`), with official mappings. The source lock pins
outer server SHA1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inspected inner
server SHA256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
and mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`.
No REA execution or decompiled-source evidence is claimed.

A separate **OriginalRegistryObservation** executed unchanged original version
detection, bootstrap and `PotionBrewing.bootstrap(FeatureFlags.VANILLA_SET)`.
It observed the ordered three containers, two container mixes and63 potion
mixes. It also observed that all46 declared built-in Potion values had the same
original VANILLA_SET feature object. This is registry/bootstrap observation,
not an executed brewing-world tick, native production registry or gameplay test.
The temporary probe source SHA256 is
`d9c7b9491b21fbdabe30f51dc22dd03ec7e62f01a15a90bca2ee3ad517a3271d`;
raw observation SHA256 is
`2df3f380e30893b55ab6fdf9337112b5a26a8381de95ddf13f7c0854cf55c535`.
Its normalized ordered key-record SHA256 is
`935eba101336e5cfc5b7b7cd20cf84e5e980035b09a11ecaa0f8a6e58b1d9486`.
Java source, binaries, JARs and raw observation/disassembly remain outside Git.

## Original queries, registration and assembly

| Mapped method/source lines | Source behavior |
| --- | --- |
| `isIngredient`, 37 | Tests container ingredients before potion ingredients. These query the actual ordered Ingredient lists. |
| `isContainer`, 41–46 | Tests the admitted container Ingredient list in order. |
| `isBrewablePotion`, 68–73 | Looks for a matching **target** potion holder in potion mixes, using actual `Holder.is`. |
| `hasMix`, 77–81 | First requires an admitted container. Then tests container mixes before potion mixes. |
| `hasContainerMix`, 85–90 | Checks source item holder and ingredient; it does not require potion contents. |
| `hasPotionMix`, 94–103 | Reads component31 with `PotionContents.EMPTY` as its source default; absent optional potion returns false. First matching from-holder/ingredient wins. |
| `mix(ingredient,bottle)`, 107–128 | Has **no admitted-container gate**. Empty bottle, absent optional potion, or no matching recipe returns the same input ItemStack reference. With a present potion, container mixes precede potion mixes and the first match returns immediately. |
| `PotionContents.createItemStack`, 68–70 | Creates a fresh count-one target ItemStack and sets fresh component31 containing only the actual potion holder. Custom color and custom effects are empty. |

A successful mix does not copy any input component patch. It inherits the
**target item's actual prototype**, then sets its new PotionContents. Name,
count overrides, custom color/effects and other old patches do not migrate.
An unchanged result has the opposite ownership: it is the exact source stack,
including an empty stack, not a new equivalent value or another owned bottle.
Potion matches are directional `recipeHolder.is(actualHolder)` calls.
`Reference.is(other)` asks `other.is(this ResourceKey)`; reference-key comparison
uses actual key identity and a Direct holder does not match that key. Direct
`is(other)` compares actual potion values, whose class does not override Object
equality. This is not symmetric protocol-ID or name equality. Item matching is
different: `ItemStack.is(Holder<Item>)` compares its current item's built-in
holder to the supplied holder by reference equality, not Potion `Holder.is`.

Builder source237–277 preserves the following admission order:

- `addContainerRecipe`: from-item, ingredient-item, target-item feature checks
  short-circuit first; only then verify from/target are actual PotionItem
  instances and append the container mix.
- `addContainer`: check enabled features before the PotionItem type check.
- `addMix`: check source potion, ingredient item and target potion features in
  that order before appending the actual holder recipe.
- `addStartMix`: check the target potion first, then register water → mundane
  and awkward → target through `addMix`. Flattening this to unconditional rows
  would lose the outer target-feature gate.
- `build`: copies the three lists in their existing order; no sorting or
  deduplication supplies replacement recipe precedence.

The vanilla containers are potion, splash potion and lingering potion. The two
container recipes are potion + gunpowder → splash, then splash + dragon breath
→ lingering. The potion registration includes1.21 breeze rod, slime block,
stone and cobweb start mixes. Keys and source registration order are data;
they do not constitute item/potion holder instances or feature ownership.
`potion_brewing_vanilla::VANILLA_REGISTRATIONS` retains57 original calls:
three `Container`, two `ContainerRecipe`,41 `Mix` and11 `StartMix`. The latter
expand to the63 potion rows under original VANILLA_SET. `VANILLA_POTION_KEYS`
lists46 declaration keys, and `add_vanilla_mixes` resolves every registration
through the builder's required context. Flattened rows do not replace those
conditional registration calls.

## Existing block-entity consumption is separate

There is a deliberate source distinction between recognizing a brew and
constructing a changed result. For example, an admitted potion item with no
optional potion holder can satisfy the gunpowder container recipe in `hasMix`,
yet `mix` returns that same bottle. `dqk.isBrewable`159–178 can therefore permit
the stand to brew, and `doBrew`182–202 still consumes the ingredient. Do not add
a changed-result requirement or treat unavailable component ownership as a
known absent potion.

`doBrew` calls the actual Level PotionBrewing service for bottles0,1,2 in order,
including unchanged/empty bottles. It then shrinks ingredient3 by one, and only
afterward reads its current item and crafting remainder. In this pinned
artifact an exhausted final dragon breath now reports AIR and has no bottle
remainder. If dragon breath remains, its fresh static remainder is dropped at
integer block coordinates; the stand then stores the ingredient stack before
Level event1035/data0. These phases are already represented in `brewing.rs`;
this stage adds no second remainder, dirty, drop, RNG or event owner. Existing
engine/controller implementation and historical verification retain their
own provenance; source inspection here is not a new world-tick observation.

## Callable API

`potion_brewing::PotionBrewingContext` is mandatory. Its opaque `Snapshot`,
`Item`, `Potion` and `Ingredient` handles retain actual owners; snapshot capture
and validation precede/follow the source operations. It supplies real key
resolution, feature/type/Ingredient predicates, exact item-holder comparison,
directional `potion_is`, effective component31 `stack_potion`, source
`stack_item`, fresh `create_potion_stack`, and the separate unchanged-copy
identity certificate. No method has an owner-free default.

`PotionBrewingBuilder::new(&context)` binds that snapshot. Its
`add_container`, `add_container_recipe`, `add_mix` and `add_start_mix` methods
retain the three source lists. `potion_brewing_vanilla::add_vanilla_mixes`
replays the ordered key specification into that builder; `build` returns the
borrowed `PotionBrewing` service. `snapshot_key` and `registration_counts` are
inspection/binding APIs, not proof that an actual registry or Main caller exists.

`snapshot_key` identifies the registry/features/component context only. Different
builders can share that context while producing different PotionBrewing lists.
The existing `BrewingMenuOwner::ContextKey` must therefore also bind the real
Level-owned PotionBrewing object/list lifetime through its actual owner handle;
the snapshot key alone is insufficient. Do not invent another identity counter
or substitute list/content equality. Replacing that actual Level service must
invalidate captured menus/plans through the outer owner contract, even when the
context snapshot is unchanged. That production Level owner remains missing.

The service exposes the source container/ingredient/potion queries and
`mix_result(ingredient,bottle) -> MixResult`, whose variants are
`Unchanged(&original ItemStack)` and `Fresh(ItemStack)`. It also implements the
existing `brewing::PotionBrewingOwner` for use by `BrewingState::prepare_tick`.
That integration point remains callable source, not an installed Main service.

Reviewed frozen Rust source SHA256 values are
`c16cc75177e0974bedd36f2fc67316b9fbe4782a029880aa5d70bc99271055c0`
for `potion_brewing.rs` and
`bc54d8df98079706545cd8c3c1badb5290363faaab8bd41a1c583a16a265036a`
for `potion_brewing_vanilla.rs`. Provider checks use explicitly controlled native
owners; registry-table observation is separate from those checks. Root executed
the focused `workstations::potion_brewing::tests` filter: all three controlled
source cases passed in0.06s. Root's final all-target compiler check passed
in1m23s on the9b composition with those frozen source hashes unchanged;
formatting and diff checks also passed. These checks supply no real built-in
Potion registry, Level/Main caller or gameplay proof. The original bootstrap
observation above remains separate evidence.

## Required context and remaining production contracts

The algorithm must borrow actual item identities/PotionItem type checks,
Ingredient predicates, enabled feature sets, canonical built-in potion holders,
their `Holder.is` relation and component31 ownership. Reading arbitrary protocol
holder IDs or guessing None for an unavailable service is insufficient. The
actual input/component service must distinguish a known absent optional potion
from an unimplemented registry/component context.

The public mix boundary must preserve borrowed unchanged results. The existing
`PotionBrewingOwner::mix` interface returns a Rust ItemStack value; its adapter
may materialize an unchanged value only when a real source-identity certificate
proves that operation valid for the captured stack/component owners. Otherwise
it must fail. Comparing encoded bytes or cloning a value is not that proof.
Fresh results require the actual target prototype and canonical potion-only
component31 constructor; a fake registry ID or copied input patch cannot replace
either owner. The ordered keys are not a RecipeBook execution engine.

Persistent component31 translation is still missing in `item_nbt.rs` in both
directions. Slot stream parsing alone does not provide its persistent codec,
built-in potion lookup, holder identity or living-effect definitions. Before a
production stand can commit/store these outputs, central integration needs the
actual service, registry/feature binding, component validation/construction,
persistent codec, block-entity lifetime/revision, ticker and save owners.
`BREWED_POTION`, dirty/comparator callbacks, item remainder factories, event
publication and consumption/projectile effects remain with their actual outer
owners. No successful no-op callback or fabricated advancement/effect owner is
supplied here. No full brewing, workstation row or fixed-checklist completion is
claimed; `scope_version: 1` and the100-item definitions remain unchanged.
