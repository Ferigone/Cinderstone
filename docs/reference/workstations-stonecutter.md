# Stonecutter source ledger and integration boundary

Target: Minecraft Java 1.21, protocol 767. Primary artifact: unmodified inner
server JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official server mappings SHA-1:
`31c77994d96f05ba25a870ada70f47f315330437`.
This stage used JDK 21 `javap -p -c` directly, not REA and not a Java gameplay
observation. Neither Java artifacts nor raw disassembly are committed.

## Inspected methods

| Official owner | Mapped class/method | Fact used by the native core |
| --- | --- | --- |
| `StonecutterMenu` | `crw`, constructor and `slotsChanged`, `setupRecipeList`, `setupResultSlot`, `clickMenuButton` | One owned input, one derived result, selected recipe DataSlot, result feature check |
| Anonymous input container | `crw$1.setChanged` | Container change, menu input identity check, then UI update listener |
| Anonymous result slot | `crw$2.onTake`, `getRelevantItems` | Crafted callback, recipe rewards with the actual pre-consumption input, shrink one input, rebuild result, world sound, base slot callback |
| `RecipeManager` | `czd.getRecipesFor` / `b`, comparison lambda | Match recipes, then sort by result stack description ID |
| `ResultContainer` | `crl.removeItem` / `a(int,int)` | Removes the entire result through `ContainerHelper.takeItem`, ignoring requested amount |
| `RecipeCraftingHolder` | `crk.awardUsedRecipes` / `a(Player,List)` | Recipe-crafted criterion before non-special recipe-book award and used-recipe clear |
| `MenuType` | `crc` static registration | Stonecutter menu registry ID 24 |

## Ported bounded behavior

`src/workstations/stonecutter.rs` retains only the canonical temporary input,
recipe choice/list, result preview, used recipe and last sound time. It borrows
recipe and enabled-feature access for each transaction. It never retains a
second player inventory or treats callback snapshots as additional owned items.

Slots are input 0, result 1, main inventory 2–28 and hotbar 29–37. Result inserts
and pick-all collection from the result container are disallowed. The standalone
selected DataSlot starts at zero; changing the input's **item identity** resets
selection to -1 and clears the result. Count and component changes on the same
item do not reset selection. Invalid recipe buttons are ignored but accepted.

The pinned adapter uses the existing recipe evaluator and the shipped 250
stonecutting recipes. All their outputs are block items, so the native adapter
uses the corresponding `block.minecraft.<item>` description keys. Equal keys
are rejected because their tie ordering needs the real manager encounter order.
Custom recipe managers use the explicit provider contract instead. Result
availability is checked against a caller-supplied enabled-feature query;
production must supply the actual level owner.

A normal take removes the entire result preview, including right-click takes.
It consumes one input regardless of result stack count. A prepared transaction
contains the sole owned output, next state and ordered callback descriptions:

1. `ItemStack.onCraftedBy`, with the actual callback count.
2. Recipe-crafted criterion and used-recipe award, observing input before shrink.
3. One input consumed and the derived result recomputed.
4. The stonecutter take-result sound through the captured level/block position,
   at most once per world game tick. Initial `lastSoundTime` is zero, so game
   tick zero itself does not play the sound.

The host must stage those effects with the real player/world owners before
committing the prepared state/output. Failed owner preparation leaves the old
state intact. Quick-move additionally invokes `Item.onCraftedBy` **before** the
inventory insertion attempt, even if insertion fails. After a successful partial
or full insertion, `prepare_quick_take` gives `onTake` the actual remaining
preview stack, including its count-zero callback after a full insertion. The
recipe still consumes one input. The consumed plan validates its expected state and revision when committed;
replayed/stale plans fail.

Closing handles the actual cursor first via `AbstractContainerMenu.removed`,
then discards the result preview and returns/drops only the owned input. Block
validity remains the actual `ContainerLevelAccess`/player owner: stonecutter at
the captured block position and `player.canInteractWithBlock(pos, 4.0)`. The
actual player adds its block-interaction-range attribute and checks strict
eye-position distance to the block AABB; this is not a hardcoded center-distance
test.
There is no RNG draw in these stonecutter callbacks.

## Production integration and verification limits

`src/container_workstation_stonecutter.rs` adds a callable authoritative menu
adapter over the shared `MenuSync` owner. It supports normal left/right pickup,
input/main/hotbar transfers, repeated shift-result crafting, deliberate throws,
creative clone, hotbar/offhand swap, quick-craft drag distribution, pick-all,
selection buttons, lifecycle removal and shared synchronization. Pick-all and
drag distribution exclude the derived result slot. Bundle click overrides
require their real owner. Its prepared action validates the captured
menu identity/revision and complete canonical inventory baseline before a single
commit. Local synchronization callbacks occur at the source result-recompute
boundary; predicted remote state cannot erase those local callbacks.

Main opening/click/button dispatch, captured validity/close publication and
actual recipe/stat/advancement/sound callbacks remain with the integrator. The source contract is recorded above; no Main
callsite is claimed by this stage. Replacing an input or preparing an output does
not itself establish a complete production menu or a complete checklist row.

The focused native check covers stone-to-slab selection, same-item count-change
selection retention, one-input/two-slab ownership, sound suppression during a
second take in the same tick, exhaustion and close avoiding a duplicate preview.
Compiler/test results are recorded by the team coordinator after the module is
exported. Comprehensive conformance and graphical/server gameplay are deferred.

The adapter flow additionally moves the actual three-stone inventory stack into
the menu, selects slabs and shift-crafts exactly six owned slabs into the shared
inventory, with one same-tick sound callback. A previously prepared selection
transaction is rejected after the crafting action commits. This is a native
transaction check, not an unchanged Java server or graphical-client observation.

Validation observed in this session: `cargo test --lib stonecutter` passed the
core ownership flow; `cargo test --lib container_workstation_` passed the
stonecutter authoritative adapter flow along with the smithing adapter flows.
These observations are native unit/transaction execution, not production Main
callsite or Minecraft network-client evidence.

## Remaining native click-mode stage

The adapter additionally follows `AbstractContainerMenu` (`cpu.doClick`,
`resetQuickCraft`, `canItemQuickReplace` and its two pickup lambdas), inspected
with the same direct `javap -p -c` method. Ordinary result pickup installs or
grows the cursor before result `onTake`. Swap into an empty hotbar/offhand
installs the actual output there before `onTake`. Result swaps into occupied
held slots are refused by `mayPlace`; they never exchange owned inputs for a
preview. Oversized held-stack displacement uses the canonical inventory-add
order and actual drop request, retaining the creative remainder branch.

Quick-craft retains its mode when status/slot selection resets, supports ordinary
split, right-drag single placement and creative fill, and cancels an active drag
when a different click arrives. Single-slot drag follows the original pickup
fallback. The menu uses slot-index order for an otherwise unobservable server
input-container update batch; the default server update listener is a no-op.
Reentrant custom listeners and custom item click overrides remain unsupported.
Pick-all performs both source passes and excludes `ResultContainer`, even when
the cursor item equals the displayed result. Actual offhand mutations use the
canonical player's slot 45, not a synthetic menu slot.

The focused native transaction flow for this stage passed with
`cargo test --lib container_workstation_stonecutter`: it distributes eight real
stone between the input and player inventory, swaps the two-slab result into
the hotbar with its inventory listener before the sound boundary, proves
pick-all leaves the same-item preview unowned, then moves the remaining real
input into the actual offhand. The earlier ownership/shift/result-capacity flow
also passed. This does not establish Main integration or a full checklist row.
