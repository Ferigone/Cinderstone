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

## Ordered drop publication

`StonecutterMenuEvent::Drop { index }` references exactly one owned entry in
`StonecutterPublication.outcome.drops`. The host dispatches this event in place;
it must not drain the drop vector independently of the ordered event sequence.
A result THROW therefore emits crafted/recipe callbacks, nested local sync,
sound, the actual player-drop request, then the final click synchronization.
The drop is not moved to the beginning or end of the whole menu transaction.

Close consumes `MenuSync.close_inputs_ordered`: cursor first, then actual input,
retaining each insertion packet or overflow drop in its original position.
Packet events are the sole publication owner and `outcome.packets` is cleared,
preventing a host from sending an aggregate duplicate packet batch. Main still
owns the outer server-close packet, which precedes these removed callbacks.
The focused adapter flow now verifies result-throw sound/drop/final-sync order
and a mixed alive close where the cursor overflow drops before a later input
insertion packet and that input's remaining overflow drop.

## Actual inventory callbacks and source phases

The adapter now exposes `StonecutterMenuEvent::PlayerInventoryChanged` for the
actual `Inventory.setChanged`/timesChanged owner, distinct from slot listeners
and packets. It does not create a counterfeit counter in the menu. The host
must stage that actual player callback or reject the unsupported context before
commit. Temporary-input callbacks are separately represented by `InputChanged`:
SimpleContainer's setter/remover callback, Slot.setChanged and Slot.onTake retain
their source multiplicity. The default server update listener is the shipped
no-op; reentrant custom listeners remain unsupported.

Every effect, input/player change, sync and indexed drop carries read-only
`StonecutterPhase` observations of the actual temporary input, derived result,
selected data index, canonical player inventory and external cursor. They are
not additional owners. Initial/incremental sync entrypoints also return these
phase-tagged events. A host must execute callbacks against the supplied source
phase rather than replaying all callbacks against only final inventory contents.

In-place quick-move mutations use private working overlays until the source
container's changed/recompute boundary. Thus shifting the last input can show
an empty live input alongside the old selection/result at the destination's
inventory callback, then clear the selection/result at the input callback.
Crafted/recipe callbacks observe pre-consumption input; input removal observes
the actual result remainder; subsequent synchronization and sound observe the
recomputed preview. Cursor-only removal precedes result discard and actual input
return. Close reconstructs per-insertion phases from the shared ordered raw
inventory-slot packets, retaining whether the owned return belongs to the cursor
or input. Actual world/drop construction still belongs to Main's real host.

The ordinary nonempty self-hotbar SWAP preserves its two inventory-change calls.
An overfull self-swap preserves held/source aliasing, so only the split remainder
enters Inventory.add. `Slot.safeInsert` uses signed capacity: an imported/raw
99-count default stone target plus one cursor stone becomes target64/cursor36.
Zero-capacity insertion still executes the setter and dirty callbacks. Empty
quick-move destinations retain the setter's callback and the extra source
Slot.setChanged; merged destinations retain one change callback. Java wrapping
count arithmetic is used at these accepted overfull boundaries.

Focused native regressions passed for self-swap callback multiplicity, a
conserved two-sword overfull self-swap, signed and zero capacity, temporary-input
callback counts, destination-versus-input shift phases, crafted/input/sound
phases and cursor-versus-input close observations. This is callable native source
verification. Main still must supply actual player counters, item-crafted,
recipe/advancement, feature-set, block validity, sound and real drop owners.
Mutating item callbacks/custom recipe contexts outside the pinned block-result
profile require an explicit integration contract rather than a no-op substitute.
No Main callsite or completed aggregate checklist delivery is claimed.

## Retained full-state integration API

The [central integration request](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6563646842)
adds exactly
`StonecutterMenu::broadcast_full_state(&mut self, &PlayerInventory) -> Result<Vec<SyncEvent>>`
for spectator/full-state resynchronization. It clones the retained menu, calls
`MenuSync.broadcast_full_state` with its actual top slots, canonical player
inventory and selected data, advances the checked action revision, and installs
that clone. It does not call initialize, replace the menu instance, consume
inputs, clear the cursor/drag state or emit authoritative dirty callbacks.
The existing phased initialize, incremental and click APIs remain separate.

The focused native regression preserves the input, derived result, selected
recipe, external cursor, menu instance and active drag across full resync,
checks the complete content/cursor/data packets, and rejects an action prepared
before the revision change. Completing the retained drag afterwards transfers
exactly its existing cursor once. This verifies the requested API seam; central
Main wiring and its runtime evidence remain the integrator's responsibility.
