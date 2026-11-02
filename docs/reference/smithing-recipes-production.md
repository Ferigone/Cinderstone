# Smithing recipe and callback production adapter

This bounded adapter supplies the current Main integration with actual default
recipe ordering, enabled-feature checks and source-phased item/statistic/recipe
callbacks. It does not introduce a recipe manager, inventory, advancement
registry, attribute catalogue or RNG owner.

## Original source and item capabilities

Target: Minecraft Java **1.21**, protocol **767**. Original inputs remain pinned
in [`source-lock.json`](../../fixtures/source-lock.json): outer server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Evidence is direct JDK 21 `javap` plus bounded queries of unchanged original
bootstrap registries, not REA MCP or a Java gameplay session. Raw Java artifacts
and disassembly remain private.

Fresh original-registry queries covered the captured smithing base items,
Netherite transform outputs and trim-result item types, plus AIR. Every queried
item's actual required features were a subset of the original
`FeatureFlags.VANILLA_SET`; both `onCraftedBy` and `onCraftedPostProcess` were
declared by the original base `Item` class (`cul`). The latter body returns
without mutation. The sorted item-ID capability list in
[`main_smithing_recipes.rs`](../../src/main_smithing_recipes.rs) is limited to
those observed types; it is not an all-items callback implementation.

| Original source | Behavior retained |
| --- | --- |
| `SmithingMenu` (`crt`), `ItemCombinerMenu` (`cqx`) | Item take, actual pre-consumption recipe callbacks, ordered input updates and world event. |
| `ItemStack.onCraftedBy` (`cuq.a(Level, Player, int)`) | Awards ITEM_CRAFTED before invoking the virtual item callback, even for an empty stack and zero amount. |
| `ItemStack.getItem` (`cuq.g`) | An empty stack exposes AIR to that crafted-statistic lookup. |
| `Item.onCraftedBy` / postprocess (`cul.b`, `cul.a`) | The observed base/result item types inherit the actual default callback bodies. |
| `SmithingTransformRecipe.assemble` (`czt`) | Existing native evaluator retains the base patch while selecting the new result prototype. |
| `SmithingTrimRecipe.assemble` (`czu`) | Existing evaluator uses captured actual material/pattern registry data and preserves the base item's other components. |
| Smithing world-event lambda (`crt.a(dcw, jd)`) | Calls `Level.levelEvent(1044, position, 0)`, without a server sound-seed or pitch draw. |
| `ServerLevel.levelEvent` (`aqu.a(cmx, int, jd, int)`) | Broadcasts around raw block X/Y/Z converted to doubles with radius 64 and the current dimension; the level-event packet has `global=false`. |

No helper constructs replacement enchantments or attribute modifiers. Assembly
remains with the existing actual `RecipeBook` evaluator and canonical item
prototype/component codecs. Unsupported component identities or dynamic recipe
contexts must fail before the real owners commit.

## Actual recipe and feature snapshot

`NativeSmithingRecipes::new` checks the existing
`MainItemGameplay.require_supported_floor` owner and parses the same unique
feature-flags packet that Main sends. It passes that actual immutable flag
snapshot and the canonical `RecipeBook::vanilla` to
`PinnedDefaultRecipeContext`.

`order()` returns that context's distinct observed constructor/input encounter
order and stable description-ordered result selection. `result_enabled()`
uses its certified actual base/result required-feature data. The context checks
the recipe/item catalogue hashes and rejects another catalogue, experimental
feature profile or arbitrary dynamic owner. No lexicographic substitute order
or default-true feature predicate is installed.

## Phased callback contract

`PreparedSmithingCallbacks::prepare` reads the actual gameplay registry and
recipe book and creates detached proposals. Callers retain the real owner
locks, validate the book again and consume the proposals once. Each
`SmithingMenuEvent` carries `SmithingPhase`: three actual inputs, derived result,
canonical `player_inventory` observation and separate external cursor.

- `CraftedBy` stages the actual ITEM_CRAFTED increment with the original amount
  and item lookup. A normal default output awards one crafted result; a full
  quick transfer passes the emptied result and awards the zero-count AIR entry.
- `RecipeCrafted` validates the event's exact pre-consumption inputs, recipe ID
  and actual nonempty assembly. Its callback is admitted only when the existing
  gameplay registry proves the actual native empty-criteria profile. Registered
  reward/predicate/reentrant advancement contexts are rejected.
- `AwardRecipe` must follow its matching recipe-crafted callback. Real
  `ServerRecipeBook.add_recipes` stages known/highlight changes and actual Add
  packets. `book_at(index)` lets the transaction install each award checkpoint
  before publishing `packets_at(index)`. The kernel owns recipe-used clearing.
- `PlayerInventoryChanged` remains a separate mandatory event for the actual
  canonical inventory's transient wrapping `timesChanged` counter; packet or
  slot-listener counts do not replace its source multiplicity.
- `LevelEvent` stages only the original `1044` / data `0` obligation. The caller
  publishes the existing `item_effects::level_event_packet` at that source
  position, with the captured integer block coordinates as the audience origin,
  `global=false` and the source 64-block radius. Unlike a block sound, this
  audience origin does not add 0.5. It includes the crafting player
  and consumes no server sound RNG.

The actual menu adapter already converts `BeforeCreateResult` into intermediate
Sync events **before** recomputing the result. An escaped effect marker is
rejected rather than being acknowledged as another completed callback, replayed
after the result or replaced with an initializer.

`smithing_event_phase`, `smithing_preflight_phase`, `smithing_local_changes` and
`smithing_sync_packets` preserve the event's actual canonical inventory/cursor
arguments and validate all fallible component/stack-limit reads before
publication. Player-backed menu slots map back to the real InventoryMenu
indices. Smithing has no DataSlot callbacks. Packet extraction itself does not
execute listeners or dirty counters. The caller invokes the actual
`MainGameplayFrame`/`MainItemGameplay` listener with **phase.player_inventory**,
interleaving the source effects, indexed real Item drops and existing ordered
packet publication.

## Validation and remaining boundary

The resumed contract review compared working Main stage `e18ad37` with
`origin/master` at `75513cb5c5908b3685f3951b409850c0db56aa34` and read the
published workstation branch at
`a688aa77e9b438384fe2635b4174aec86c1d803f`. The recipe-context, SmithingState
and phased SmithingMenu contracts are unchanged between those checked trees;
the helper needs no source rewrite for that snapshot. `SmithingPhase` still
uses `player_inventory`, with external cursor ownership kept separately.

The checked SmithingMenu does not yet export `broadcast_full_state`. Root's
[exact team-owned API request](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6563857281)
is the pending spectator-resync integration contract. This recipe/callback
helper does not replace that operation with `initialize`, reset retained menu
baselines or edit the workstation team's kernel to work around the contract.

The bounded original-bootstrap queries and bytecode inspection establish source
facts. A focused metadata compiler check covers this helper against the current
native kernel; its tiny surrounding Fixtures/gameplay definitions are compile
scaffolding, not runtime or production authenticity evidence.

The module is initially unexported while the central coordinator finishes its
preceding stage. Root owns Main export/wiring, full compiler/runtime validation,
publication and deployment. No Main callsite, completed workstation delivery,
full Minecraft compatibility or performance advantage is established by this
ledger. Full registry reloads, nonempty advancements, custom item callbacks and
remaining identity-sensitive component contexts stay explicit boundaries.
