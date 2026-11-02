# Crafting result take: production owner contract

This is a source/production-contract note for the pending central
[request6564373039](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564373039).
It changes no crafting core, Main adapter or ownership implementation. The
existing 2x2/3x3 foundations already have bounded support; this note does not
claim them as new work, complete an aggregate delivery or certify full ResultSlot
behavior.

## Pinned source phase

Evidence is JDK21 `javap -p -c` inspection of the original Java1.21/protocol767
server and official mappings, not REA or Java gameplay execution. Inner JAR
SHA-256: `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
mappings SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`.

`ResultSlot` (`crm`) has its own removeCount. remove28–31 adds the requested
amount bounded by result count; onQuickCraft36–38 adds the transferred amount
and checks achievements; onSwapCraft42–43 adds its amount. Its exact take order
is:

1. `checkTakeAchievements`47–54. When removeCount>0, call the actual taken
   ItemStack's `onCraftedBy(Level,Player,count)`. This wrapper first awards the
   crafted-item statistic and then invokes the actual Item callback
   (`ItemStack`/`cuq`657–659). The Item callback can delegate actual crafted
   postprocessing; map flags require the real saved-map/ID owner.
2. Still in checkTakeAchievements, invoke actual
   `RecipeCraftingHolder.awardUsedRecipes(player,craftSlots.getItems())`, then
   reset removeCount. This award happens even when the count is zero. The
   holder (`crk`21–29) invokes the player's crafted-recipe callback and, for a
   non-special recipe, unlocks it and clears recipeUsed. Special recipes follow
   their source branch. Recipe-book/criterion callbacks are real owners, not
   optional fabricated no-ops.
3. **After those callbacks**, `onTake`58–93 obtains the actual grid's
   `asPositionedCraftInput()`: cropped width/height and left/top offsets, holding
   the then-current ItemStack references. It calls the **current Level's**
   `RecipeManager.getRemainingItemsFor(CRAFTING,input,Level)`.
4. The manager (`czd`117–126) performs a fresh `getRecipeFor` with no preferred
   preview holder. A match uses that current recipe's getRemainingItems. No
   match creates an appropriately sized list and assigns each original
   `input.getItem(i)` reference to it; it does not copy those stacks.
5. Iterate the captured cropped rectangle in row-major order, including holes.
   For each original grid coordinate, read the actual current grid stack and
   captured remainder reference. If the current stack is nonempty, remove one
   through the real TransientCraftingContainer, allowing its synchronous
   slotsChanged/preview/sync callbacks, then reread that grid coordinate. If
   the remainder is still nonempty: assign it into an empty grid slot; otherwise
   compare the actual stacks and grow the remainder by current count before
   assigning it; otherwise call actual Inventory.add and drop only its rejected
   remainder with around=false/trace=false. Each setter/removal retains its
   source callback phase.

`CraftingInput` (`cyp`16–81) retains ItemStack references, including when it
builds a cropped list; `CraftingContainer` (`cql`18/22) supplies the actual
positioned view. `TransientCraftingContainer` (`crx`) remove/set callbacks
notify the menu after their mutations. `ResultContainer` (`crl`39) removes the
whole result regardless of requested amount. Ordinary cursor placement and
ResultSlot callbacks must therefore keep the actual taken argument/alias for
each mode, not substitute a fresh preview copy.

The external `CraftingMenu` (`cqm`135–179) also calls direct Item.onCraftedBy on
its live result before a result quick-move insertion attempt, then
onQuickCraft/checkTakeAchievements, source cleanup and onTake. A failed
insertion can retain the earlier item callback's real effects. The retained
`InventoryMenu` (`cqw`151–209) has its own source quick-move sequence; do not
add that external-menu prefix to it. Base Slot/cpu cursor, quick-craft and
onSwapCraft paths determine the actual removeCount, arguments and callback
positions.

## Current concrete native and Main paths

These are existing callsites, not newly supplied integrations:

| Existing path | Current behavior relevant to this contract |
| --- | --- |
| `src/main.rs:3179`, `:3252`, `:3673`–`:3675`, `:3712` | Creates a connection-local `RecipeContext::default()`, refreshes the retained PlayerContainer, dispatches external menus first, then window0 `PlayerContainer::click`; creative grid updates refresh the same 2x2 owner. |
| `src/container.rs:490`, `:750`, `:772`, `:850` | `refresh_crafting` clones the 2x2 inputs and enumerates canonical `RecipeBook::vanilla()` by ID, rejecting ambiguous matches. It caches `(recipe ID,CraftResult)`; result take/quick-move consumes cloned cached `result.remaining` and refreshes afterwards. `ClickOutcome.crafted` records IDs. |
| `src/main_crafting_table.rs:59`, `:105`, `:137`, `:176`, `:245`, `:346` | Opens the actual table position, requires doLimitedCrafting=false and the supported native gameplay source, uses one borrowed canonical player inventory, checks table validity and calls `CraftingTableMenu::click(...,&RecipeContext::default())` under the existing world/player ownership locks. |
| `src/crafting_table.rs:80`, `:377`, `:596`, `:620`, `:720` | Its engine borrows the real inventory for each action and retains only grid/cursor/result state. Matching caches a CraftResult and rejects ambiguous recipe order. Consumption uses cached remainder copies over the occupied rectangle, with intermediate refreshes. Map extension requires supplied real saved-map data; map crafted postprocessing contexts reject explicitly. |
| `src/main_crafting_table.rs:382` | After the core click, drains take_crafted_items and stages crafted statistic increments before the gameplay menu commit. This is not the source's complete pre-remainder ItemStack/item/recipe callback sequence. |
| `src/main.rs:1121`, `:1259`; `src/main_drop_commit.rs:24` | Existing shared-container/drop transaction stages canonical menu, actual player/world RNG, item entities/IDs, drop statistics and prepared gameplay commit. It supplies real drop ownership; it does not currently call the Level recipe manager or take-phase recipe awards. |
| `src/main_item_gameplay.rs:192`, `:285` | The real NativeFixtureEmptyCriteria scope checks no active criterion/objective/block listener registrations and prepares readonly menu observations. Imported/registered effectful contexts are not certified. |
| `src/main.rs:973`, `:1169` | The window0 fallback can transact a no-world-drop menu operation when gameplay/world support is unavailable, rejecting only outcomes with drops and clearing local observations. This is **not** certification of crafted/recipe callbacks or cached take remainders. |
| `src/main.rs:1553`; `src/server_recipe_book.rs:497`, `:580` | Real recipe-book login state and synchronous add_recipes/RecipeBookHost APIs exist. Current crafting take paths do not consume ClickOutcome.crafted to execute the source recipe-used award/unlock chain. No production consumer of those IDs was found in the inspected source. |

`CraftResult.remaining` in `src/crafting.rs:65` is one owned value per original
untrimmed input position, computed during evaluation. `RecipeContext` currently
contains map metadata values, not a live manager, manager revision/order,
feature/registry owner, recipe-used holder or callback certificate. The newer
special/registry evaluators and [shipped metadata](workstations-recipe-context.md)
do not implicitly replace those Main arguments or implement ResultSlot's take
lifecycle.

## When cached remainder values can be certified

A bounded default-data profile can justify using the cached **matching-recipe
remainder values** only when its real owners certify all of these conditions:

- Actual manager/recipe/tag/registry/feature/rule snapshots are the supported
  unchanged originals and remain fixed across preview, callbacks and take.
  The current grid has a unique matching recipe with the same remainder
  semantics; relying on BTreeMap ID order as the manager's encounter order is
  insufficient. Limited crafting, recipe availability and result-feature gates
  need their actual owners; default context construction does not prove them.
- Real crafted-stat/item callbacks, recipeUsed awards/unlocks/criteria, menu
  listeners and any item postprocessing execute at their source points. Their
  certified effects cannot change the grid, its captured objects, the current
  match/remainders, manager, tags or queried Level context. Empty registered
  criteria/objectives may be established by the actual existing native gameplay
  owner; merely omitting the callbacks or using an always-empty substitute
  does not establish this condition. Actual recipe-book effects still execute.
- Real grid/cursor/player/stack identities, component equality and remainder
  creation/copy behavior are supported; unresolved nested ItemStack identity
  (including explicit food-component20 patches) or cross-owner aliases cannot
  be replaced by decoded-value equality. Real inventory insertion/drop/RNG and
  source callbacks must be supported, not inferred from item conservation.
- The guarantee holds after the actual achievement callbacks at the remainder
  query boundary and throughout the source consumption loop. A failure rejects
  and rolls back the detached take; each repeated quick-move iteration needs
  its own valid phase, not one blanket certificate for the outer packet.

Such a certificate would justify this cache substitution only. It would not
make today's deferred/missing callback phases or all menu behavior complete.
Current APIs do not carry this certificate; existing Main default calls must
not be described as already proving it. Effectful/reentrant callbacks, changed
recipes/reloads, changed inputs/availability and no-match cases require the
actual current manager query and stack-reference semantics, or explicit refusal.

## No-match references require an actual object owner

The manager's no-match fallback is not a vector of copied input values and not
`InputSlot(index)` resolved later. It captures the **same ItemStack objects** in
that positioned input at query time. A later remove splits/mutates those objects;
a later grid replacement can detach one without changing the captured reference.
Actual handles must therefore keep object identity beyond its current grid slot.

For example, in a context where an actual crafted callback changes a formerly
matching input into one diamond stack of count3 and the fresh manager finds no
recipe, the fallback remainder references that same diamond object. Removal
reduces it to2; the subsequent same-object grow-by-current-count makes it4.
A copied count3 remainder or a slot-index substitute has different semantics.
This is a source-derived alias example, not a production/Java gameplay test.
The bounded readonly/default certificate can rule this context out; a general
implementation must preserve the real alias rather than invent that certificate.

## Proposed additive API, not an implemented owner

The central owner can add a take-aware entry point to both existing engines,
without changing the legacy bounded evaluator's return type. One exact proposed
contract is below; names are proposals and **none of these new types/traits or
callsites is implemented by this note**.

```rust
pub struct PositionedTakeInput<S> {
    pub left: usize, pub top: usize,
    pub width: usize, pub height: usize,
    pub items: Vec<S>, // captured actual references, including empty objects
}
pub enum CurrentRemainders<R, S> {
    Matched { recipe: R, items: Vec<S> },
    NoMatch { items: Vec<S> },
}
pub trait CraftingTakeOwner {
    type Binding: Clone + Eq;
    type Revision: Clone + Eq;
    type StackRef: Clone + Eq; // real object identity, never a grid-slot ID
    type RecipeRef: Clone + Eq; // real current manager/holder lifetime
    type CacheCertificate;    // opaque actual-owner proof; no public constructor

    fn binding(&self) -> &Self::Binding;
    fn revision(&self) -> Self::Revision;
    fn stack(&self, object: &Self::StackRef) -> Result<&ItemStack>;
    fn remove_count(&self) -> i32;
    fn add_remove_count(&mut self, amount: i32) -> Result<()>;
    fn reset_remove_count(&mut self) -> Result<()>; // after the award callback
    fn item_on_crafted_by(&mut self, actual_argument: &Self::StackRef)
        -> Result<()>; // external quick-move prefix; no statistic wrapper
    fn item_stack_on_crafted_by(
        &mut self, actual_argument: &Self::StackRef, remove_count: i32,
    ) -> Result<()>;
    // Reads actual result container recipeUsed; owns special/non-special clear
    // and actual book/criteria. Never fabricates that holder from a cached ID.
    fn award_used_recipes(&mut self, grid: &[Self::StackRef]) -> Result<()>;
    fn positioned_input(&self) -> Result<PositionedTakeInput<Self::StackRef>>;
    fn get_remaining_items_for(
        &mut self, input: &PositionedTakeInput<Self::StackRef>,
    ) -> Result<CurrentRemainders<Self::RecipeRef, Self::StackRef>>;
    fn certify_cached_matching_remainders(
        &mut self, input: &PositionedTakeInput<Self::StackRef>, cached: &CraftResult,
    ) -> Result<Option<Self::CacheCertificate>>;
    fn cached_matching_remainders(
        &mut self, proof: &Self::CacheCertificate, cached: &CraftResult,
    ) -> Result<Vec<Self::StackRef>>; // actual allocations/copy semantics proved
    fn grid_ref(&self, slot: usize) -> Result<Self::StackRef>;
    fn remove_grid_item(&mut self, slot: usize, amount: i32)
        -> Result<Self::StackRef>; // actual split and synchronous menu callbacks
    fn same_item_components(&self, a: &Self::StackRef, b: &Self::StackRef)
        -> Result<bool>; // actual component/reference owner
    fn grow(&mut self, object: &Self::StackRef, amount: i32) -> Result<()>;
    fn set_grid_ref(&mut self, slot: usize, object: Self::StackRef) -> Result<()>;
    fn inventory_add(&mut self, object: &Self::StackRef) -> Result<bool>;
    fn drop_remainder(&mut self, object: Self::StackRef) -> Result<()>;
}
```

The owner must bind the actual Level/current RecipeManager, the actual result
container's recipeUsed and ResultSlot removeCount, canonical grid/inventory,
callback argument/cursor aliases, statistics/recipe book/criteria, component
identities, world RNG/drops and source sync phases in **one detached transaction**.
Its no-match return must use input.items' exact handles. Matched entries preserve
the actual recipe's returned fresh/copy/reference behavior. grow and setters
operate on the referenced object graph; a grid index only locates the current
slot when grid_ref is called. It never retroactively identifies captured objects.
Legacy ItemStack value copies and scalar snapshots do not supply this owner.

A later, explicitly assigned core patch can add
`PlayerContainer::click_with_take_owner(..., owner: &mut impl CraftingTakeOwner)`
and `CraftingTableMenu::click_with_take_owner(...)`, using scoped borrows of
those existing canonical owners. It must also expose the direct external
quick-move Item callback and exact removeCount/recipeUsed phases, not merge
those into a generic final callback. Actual begin-stage/precommit validation
and atomic publish must cover **all** these owners; callback records are
observations, never an instruction to execute effects twice. If native stack
object lifetime/alias ownership does not yet exist, the general handle API
remains unsupported; allocating fake IDs for copied slots is not integration.

Main integration belongs at the existing window0 dispatch and locked 3x3 action
above, inside their real canonical transaction before consumption and packet
publication. The existing drop/RNG and gameplay seams may be reused only with
their actual supported-owner guarantees. The pending central request must
resolve exact module assignments and runtime owner binding first. No core,
Main, manifest, checklist or persistent journal was edited; no tests were run
for this read-only source/contract task.
