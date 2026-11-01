# Stonecutter recipe and callback production adapter

This adapter connects the workstation team's prepared stonecutter transactions
to the existing native recipe catalogue, player statistics, recipe-book and
sound owners. It is restricted to the current native flat-world profile and
does not introduce a block entity or another live inventory/RNG owner.

## Source evidence

Minecraft Java **1.21**, protocol **767**, uses the artifacts pinned in
[`source-lock.json`](../../fixtures/source-lock.json): outer server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
and official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Evidence is direct JDK 21 `javap -p -c` and a bounded original-bootstrap registry
query, not REA MCP or a Java gameplay session. Java artifacts and raw disassembly
remain private.

| Original method or owner | Source fact used |
| --- | --- |
| `StonecutterMenu` (`crw`) and its result slot (`crw$2`) | Input/result ownership, selected-recipe data and source take effects are supplied by the existing workstation kernel. |
| `ItemStack.onCraftedBy(Level, Player, int)` (`cuq.a`) | Calls `Player.awardStat(Stats.ITEM_CRAFTED.get(getItem()), amount)` before the virtual item callback, including amount zero. |
| `Item.onCraftedBy(ItemStack, Level, Player)` (`cul.b`) | Invokes `onCraftedPostProcess`; it does not separately award the crafted-item statistic. |
| `Item.onCraftedPostProcess` (`cul.a(cuq, dcw)`) | The base body returns without mutation. Actual original shipped stonecutting outputs and AIR inherit this body and the base `onCraftedBy`. |
| `FeatureFlagSet.isSubsetOf` / `FeatureFlags.VANILLA_SET` (`cpl.a`, `cpn.f`) | Actual original required feature sets for every shipped stonecutting output are subsets of `VANILLA_SET`. |
| `StonecutterMenu` constructor / default update listener (`crw`, `crw.q`) | Constructor `invokedynamic` bootstrap 0 binds its Runnable to `crw.q:()V`; that synthetic method contains only `return`. |
| Anonymous input container `setChanged` (`crw$1.e`) | Invokes `SimpleContainer.setChanged`, menu `slotsChanged`, then that captured Runnable, in order. |
| `Level.playSound` (`dcw`) | The BlockPos overload adds 0.5 to each coordinate; the double-coordinate overload obtains its packet seed from `Level.threadSafeRandom` (`dcw.f`), not `Level.random`. |

The fresh registry query loaded the unchanged pinned server and its bundled
libraries, bootstrapped its original built-in registries and inspected all
167 distinct outputs of its 250 shipped stonecutting recipes, plus AIR. It
verified required-feature subset membership and the actual declaring classes
of both item callbacks. The sorted numeric item IDs in
[`main_stonecutter_recipes.rs`](../../src/main_stonecutter_recipes.rs) are the
bounded production capability catalogue from that query. They are not an
assumed all-items capability. The same original registry resolved
`minecraft:ui.stonecutter.take_result` to sound ID **1431**, with no fixed-range
override.

## Recipes and enabled features

`NativeStonecutterRecipes::new` checks `MainItemGameplay.require_supported_floor`
and parses the **actual immutable feature-flags packet used by Main**. It
requires its unique flag set to be exactly `minecraft:vanilla`. An absent,
duplicate, malformed, experimental or otherwise different feature profile is
rejected before a live menu action.

Recipe matching, assembly and result-feature checks delegate to the team's
`PinnedDefaultRecipeContext`, borrowing `RecipeBook::vanilla` and the actual
parsed feature snapshot. That context verifies the native recipe/item catalogue
hashes against its original-class evidence and retains the observed default
recipe encounter order and stable description-ID ordering. The earlier
equal-description-key refusal in `PinnedStonecutterRecipes` is superseded by
that explicit ordering source. Results must also belong to the captured
original callback catalogue. Arbitrary dynamic datapacks are outside the
context's actual source boundary.

## Ordered callbacks and actual owners

`PreparedStonecutterCallbacks::prepare` observes the existing actual gameplay
registry and recipe book and creates detached transaction proposals. It never
changes live statistics, the recipe book, the inventory or randomness while
preparing. The caller holds the world/player/menu ownership locks, validates
the actual book again, and commits its proposals once.

- `ItemCraftedBeforeQuickMove` validates the real item's captured callback
  capability but awards **no crafted statistic**. The original callback still
  occurs when shift insertion accepts nothing.
- `CraftedBy` stages the actual `ITEM_CRAFTED` increment, including the
  zero-amount AIR entry reached when a full quick transfer passes an empty
  stack to `onTake`. Normal two-slab pickup awards two crafted slabs. No
  positive-count shortcut erases the zero-count source call.
- `AwardUsedRecipe` validates the original pre-consumption input against the
  actual pinned recipe and the `VanillaRecipeCatalogue` descriptor. It uses
  real `ServerRecipeBook.add_recipes`, retaining known/highlight changes and
  real Add packets. `book_at(index)` captures each source award boundary, so
  the caller installs that actual book state before publishing its packets.
  The workstation kernel owns clearing `recipeUsed` and consuming the input.
- Recipe-crafted and recipe-unlocked trigger contexts are admitted only after
  the existing gameplay owner proves the **actual fresh native empty-criteria
  profile**. No registered reward, predicate or reentrant advancement context
  is replaced with an empty callback; those profiles are rejected.
- `PlayTakeResultSound` remains an indexed source event after input consumption
  and nested synchronization. The kernel decides same-game-tick suppression.
  At that index the production transaction obtains one seed from the existing
  `RuntimeEntities.next_sound_seed` owner. `stonecutter_sound_packet` encodes
  the centered position, BLOCKS category, volume/pitch 1 and that seed. The
  original variable-range audience is 16 blocks, including the crafting player.

Every event now carries `StonecutterPhase`: the actual input, derived result,
selection, canonical player-inventory observation and separate external cursor.
`stonecutter_event_phase` exposes those arguments without creating another
live owner. Inventory criteria execute against **that event's** inventory;
pre-consumption recipe input and intermediate cursor/result observations are
not replaced with a final transaction snapshot. `stonecutter_preflight_phase`
resolves all fallible component/stack-limit reads before publication.

`InputChanged` is explicitly staged and consumed through
`complete_input_change_at`. The kernel already applies its SimpleContainer/menu
state callbacks, and fresh verbose bytecode resolves the constructor's update
listener to the original return-only `crw.q` body. Main constructs this default
menu and has no listener replacement path; a custom listener has no capability
here. `PlayerInventoryChanged` is a separate mandatory event for the **actual**
canonical `Inventory.setChanged`/transient wrapping `timesChanged` counter.
It must run at each recorded event, independently of slot-listener and packet
counts; the menu does not own a replacement counter.

The transaction owner interleaves these effects with `SyncEvent` and indexed
actual Item drops; it must not publish all drops or all recipe packets as a
separate final batch. Drop entity construction keeps its own existing player,
Level and entity-ID ownership. A sound event never obtains a replacement RNG
stream and never consumes a Level-random pitch draw.

`stonecutter_local_changes` maps the shared menu's player-backed slot listeners
to the actual canonical InventoryMenu/storage indices for the existing
`MainItemGameplay` callback path. `stonecutter_sync_packets` is only packet
extraction: it does **not** execute those listeners. Opening dispatch must run
the actual listener owner between Open Screen and initial synchronization.
Temporary input/result-container listeners do not become player-inventory
criteria, and the selected-recipe data listener has no server-player gameplay
callback.

## Immediate item-pickup synchronization

[`main_stonecutter_pickup.rs`](../../src/main_stonecutter_pickup.rs) adds
`StonecutterRuntime.broadcast_for_inventory` for the actual `ServerPlayer.take`
boundary. The existing item coordinator publishes Collect, then the canonical
`RuntimeInventoryHost` calls this bridge with the real picked-up player's
identity, the same shared player inventory and its actual `MainGameplayFrame`.
The bridge prepares `broadcast_changes` on the current stonecutter menu,
prevalidates slot mapping and each callback phase's component/stack-limit scans,
calls the existing inventory-criterion owner with its actual phase inventory
in source slot-listener order, then
commits the menu baselines and returns its ordered packets to the existing
shared publisher. It never synchronizes the inactive window0 in place of the
active menu.

The external synchronization hook receives the already borrowed gameplay frame
from `RuntimeInventoryHost`; it does not capture another mutable gameplay
owner, clone it or create an additional packet writer. The menu's actual cursor
remains in `StonecutterMenu` and is included by its ordinary broadcast. A later
player tick retains its independent validity/close obligation. Pickup does not
reinitialize menu state, send a delayed substitute full-state packet or move
that later validity check into `take`.

All fallible menu encodings and inventory stack queries complete before
callback observations or menu baselines are committed. The concrete gameplay
frame enforces the actual source/player guards before its first callback
mutation; the same world/player owner locks retain those facts through the
remaining source-ordered callbacks. This relies on the already declared native
empty-criteria profile, not support for mutating or reentrant advancement hooks.
The existing pickup publisher emits the returned private packet batch after
these callbacks. This preserves Collect-before-menu publication, callback
encounter order and packet order for the supported empty-criteria owner; it
does not establish interleaving for callbacks that themselves publish packets
or mutate gameplay. A broadcast containing a non-synchronization event is
rejected before callback execution instead of silently dropping a mandatory
owner operation.

## Validation boundary

The source query establishes original registry/callback facts. Compilation and
the focused native opening/click/close protocol flow are recorded by the
integration coordinator after Main wiring. This ledger itself is not runtime
Main proof, aggregate workstation completion or a compatibility percentage.
Imported/custom-feature worlds, dynamic recipes, nonempty criteria and
reentrant/custom item callbacks remain outside this bounded adapter.
