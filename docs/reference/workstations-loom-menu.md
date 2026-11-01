# Loom menu controller — Java 1.21

This stage adds `src/container_workstation_loom.rs` over the existing single
`LoomState` in `src/workstations/loom.rs`. It implements ordinary PICKUP,
QUICK_MOVE, SWAP (including offhand), CLONE, THROW, QUICK_CRAFT and PICKUP_ALL,
pattern buttons, ordered synchronization and temporary-input removal. The
existing kernel remains the input/result owner. Its additive raw mutation and
source-observation hooks expose callback boundaries without owning more items.
There is no new Main callsite or aggregate checklist completion in this stage.

## Primary evidence

Inspected the pinned original server with **JDK 21 `javap -p -c`**, using the
official Java 1.21 mappings; this was not REA or execution of a Java menu oracle.
The inner-server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
the official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
The previous [kernel evidence](workstations-loom-cartography.md) still applies.

Source methods used:

- `LoomMenu` (`cra`): constructor lines 36–133, `stillValid` 137,
  `clickMenuButton` 142–147, `getSelectablePatterns` 151–157,
  `slotsChanged` 166–214, `quickMoveStack` 230–278, `removed` 283–285,
  `setupResultSlot` 288–303, and default update listener line 38.
- `LoomMenu$1/$2`: input `setChanged` 51–54 and output `setChanged` 60–62;
  `$3/$4/$5` input class predicates; `$6.onTake` 102–118 and its actual
  level-time/sound callback 109–114.
- `AbstractContainerMenu` (`cpu`): ordinary click modes, signed transfer,
  target admission, source/destination mutation and dirty order, actual Slot
  identity-set drag traversal, pickup-all, moveItemStackTo and removal.
- `Slot` (`crq`): `onTake` 45–46, setters 63–81, `remove` 97,
  `tryRemove` 110–127, `safeTake` 136–138 and `safeInsert` 152–168.
- `SimpleContainer` (`bra`) item setters/removal/listener behavior,
  `Inventory` (`cmw`) setters/`add`/actual dirty counter and cursor return,
  and `ServerGamePacketListenerImpl` (`aru`) accepted-button broadcast.

## Native behavior and ordering

Menu type is 18. Inputs are banner 0, dye 1, template 2; derived result is 3;
player main slots 4–30 map to canonical slots 9–35 and hotbar 31–39 to 36–44.
SWAP button 40 uses the actual canonical offhand 45. External carried state is
separate from the retained window0 inventory cursor. The one synchronized data
slot is the selected pattern index, initially zero as in DataSlot.standalone.

Default/template choices come only from the captured actual banner-pattern
registry and actual HolderSet/tag encounter order. Missing tags give no choices.
Selection survives a list change by the same actual registered holder, and one
choice selects automatically. Normal input callbacks enforce six existing
layers; source button handling only checks list bounds, including its unusual
ability to select a listed pattern after the input callback cleared a six-layer
result. Result components use the existing kernel's canonical banner layers,
actual dye color and holder IDs. The template is never consumed by a result take.

Input SimpleContainer.setChanged observes its superclass listener boundary,
then runs slotsChanged, then the update listener. Result SimpleContainer only
runs its superclass listener boundary and update listener. A Slot setter runs
both its container setter's changed callback and Slot.setChanged. Result setters
therefore produce two output/update observations. The missing-input branch sets
the output before clearing selection/list and returns without broadcast. Other
input recomputations broadcast after selection/result changes, including when
the result did not change. All observations and Sync events carry source-phase
inputs, preview, selection/list, actual player inventory snapshot and cursor.
They do not create item ownership or call the calculation a second time.

Result onTake removes one banner, then one dye, each with its own actual input
callback and possible broadcast, then clears unavailable selection, uses actual
captured-level game time for the once-per-tick sound, then invokes base result
Slot.onTake. The sound guard starts at zero. No crafted-item, recipe,
advancement or XP callback is invented. The ordinary pickup final slot-dirty
callback is retained. Output is a SimpleContainer, so its removal/empty setters
are observable before the cursor is installed; input-consumption callbacks see
the installed cursor. Quick-move mutates the real source before destination
callbacks and consumes result inputs only after insertion. Full matching inputs
have no player-range fallback. Empty quick-move targets retain the extra source
moveItemStackTo dirty callback. Signed safeInsert capacity includes zero and
negative transfers; an overfull dye99 plus cursor1 becomes slot64/cursor36.
Normal nonempty self-SWAP retains two actual player dirty callbacks, and an
overfull alias returns only the already-split old remainder.

Close returns the cursor first, then removes each owned input with
removeItemNoUpdate and returns/drops it in input order. It never returns or drops
the preview. Unlike some other workstation menus, Loom.removed does not first
clear its result, so close callbacks retain the old derived preview and selection
while observing each input's removal. Per-insertion raw inventory packets and
overflow/disconnected drops retain their source positions. Drop indices consume
one entry from the publication's drops; do not separately publish that vector.
Ordinary drops use around=false/trace=true; menu-close drops use false/false.

## Required production contract and explicit limits

`LoomMenuContext` requires the real pattern/tag resolver and WorkstationAccess,
plus a stable comparable binding containing actual player/world/block/registry
identity, tag revision, enabled-item admission and callback environment. A made-up
registry or local substitute counter is not an implementation. The host captures
this binding at menu construction and must retain it for that menu's lifetime.
Creation and ordinary clicks/buttons validate the real loom block and player's
reach with source additional range 4.0. Invalid reach still permits removal.
The actual Main player/session/window owner must serialize opening, publication
and closing and admit enabled items; this controller does not invent those owners.

The host explicitly certifies the actual default source update listener and
nonreentrant container/sync listeners; unknown/custom/reentrant contexts reject.
The source events mark the respective actual callback boundaries. They must not
rerun slotsChanged or broadcast on top of the already recorded Sync stream.
Actual listener/item/drop/world effects must be staged successfully against the
provided phases before consuming commit. No fallible or reentrant callback may
mutate live inventory/menu between validation and commit. Main owns Player.drop,
its RNG/entity creation, and actual level sound distribution/RNG. Loom itself
chooses no random recipe or random sound pitch.

Completed multi-target drag requires the real Slot identity HashSet encounter
order for the exact target set; missing/invalid order rejects preparation. The
BTreeSet only tracks membership, never substitutes a Java iteration order.
The supplied order is checked again at commit. Order matters for native input
recomputation, intermediate broadcast/state/listener observations and player
dirty callbacks as well as custom observers. An independence certification that
only covers custom listeners cannot replace this actual encounter-order owner.
Bundle item click overrides
reject explicitly where they would require the real bundle owner. Custom item
classes beyond the kernel's pinned vanilla banner/dye classification need a real
classification/color extension; none is fabricated here.

Explicit `minecraft:food` component20 patches reject before dependent mutations
or logical comparisons, including the station, external cursor, main/hotbar/
offhand inventory and client predictions. Nested ItemStack reference identity in
such a patch is not established by equal wire bytes or decoded values; this
adapter has no actual custom-identity resolver. Stable implicit prototype food
remains supported. The kernel's raw/input/result comparison hooks enforce the
same bound. Adding that identity owner is separate integration work.

Each UI has a checked nonreused native lifetime ID, preserved by cloning. Plans
validate the entire prior menu (including revision/sync/drag/cursor), canonical
inventory snapshot, captured binding, real click/button block/reach validity,
actual result-take game time and any used drag order. Commit is single-use. A plan from an old menu cannot replay after a
same-window/same-binding reopen. Ordinary menu state IDs and predictions remain
owned by MenuSync. After close this UI rejects further actions. Actual active
player/session binding, validity immediately before publication, listener/drop
staging, block opening, persistence and production dispatch remain Main work.

## Focused validation

`CARGO_TARGET_DIR=/workspace/cinderstone-workstations-20261101a/target cargo test
--lib container_workstation_loom::tests -- --nocapture` passed with six nonzero
focused native scenarios. These exercise canonical input routing, repeated result
shift insertion, one sound per actual fixture tick, template conservation,
result/cursor/input callback phases, missing-input old selection, signed and zero
safeInsert callbacks, actual self-SWAP dirty counts, retained window0 cursor,
registry/time/context/drag-order staleness, close ownership/preview and stale UI
replay, plus rollback/rejection for unresolved explicit food identity while
supporting stable prototype food. Fixtures own explicit controlled registry/tag and player/world inputs;
these are native tests, not Java oracle or production Main integration evidence.
Comprehensive conformance testing remains deferred.
