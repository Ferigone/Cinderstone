# Cartography menu controller

This ports the Java 1.21 cartography controller as a callable native transaction.
Main opening, packet dispatch, saved-map persistence and world/player callbacks
remain integration work. It does not complete an all-menu checklist delivery.

## Primary evidence

The pinned original inner server JAR SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official server mappings SHA-1 is
`31c77994d96f05ba25a870ada70f47f315330437`. Evidence here is JDK21
`javap -p -c` inspection, not REA or original-server gameplay observation.

Inspected methods are `CartographyTableMenu` (`cqb`), `slotsChanged`111–120,
`setupResultSlot`123–153, `canTakeItemForPickAll`157,
`quickMoveStack`162–209 and `removed`214–218; input/result containers `$1/$2`
and result slot `$5.onTake`76–91; `ResultContainer` (`crl`), `Slot` (`crq`),
`AbstractContainerMenu` (`cpu`) click/transfer operations and its
`lambda$doClick$3/$4`420–422/439–441. Map mutation follows
`MapItem` (`cvc`)338–366, direct `Item.onCraftedBy`360–361 and actual
`MapItemSavedData.locked/scaled`296–316. `CartographyTableBlock` (`dgr`)
opens at its captured level/position and separately awards the interaction stat.
The bounded food-identity exclusion also follows `FoodProperties` (`cpr`), a
record whose `usingConvertsTo` field holds Optional<ItemStack>; `ItemStack`
(`cuq`) declares no equals/hashCode override. A decoded payload cannot certify
that nested reference identity from wire/value equality.

## Layout and result ownership

Protocol767 MenuType23 has map input0, paper/empty-map/glass-pane input1,
derived result2 and player slots3–38 (canonical inventory9–44). There are no
data slots. One kernel result owns the preview; observer snapshots confer no
items. Inputs and the external cursor are temporary owned items. Window0's
retained cursor remains inside the borrowed player inventory.

All seven ordinary click modes use the actual slot rules and item components.
Result placement and pick-all extraction are refused. Creative clone copies the
preview without consuming inputs or postprocessing maps. The explicit bundle
feature/item click-override boundary is rejected. Explicit FoodProperties
component20 overrides are also rejected before generic inventory equality or
sync comparisons: the current wire/value model cannot recover their nested
ItemStack Java reference identity. The guard covers canonical/retained inventory,
inputs, preview, both cursors, callback output and client predictions. Stable
prototype food components remain admitted; no substitute identity is fabricated.
Drag uses a stable native slot order only when the actual host certifies the complete source observations are
insensitive to Java's identity-set iteration order. Certification receives the
actual target set and starting inputs/preview/inventory/cursor phase. It covers
native input-triggered broadcasts, slot/data listeners, player counter phases
and cursor/remote synchronization as well as host callbacks. A no-op custom
observer does not make order-dependent native sync equivalent. Unknown or
order-dependent contexts reject; there is no permissive default.

`ResultContainer.removeItem` ignores its requested amount and extracts the whole
result. Consequently a right pickup or single-item throw of a two-map duplicate
still extracts two. Partial result shift transfers are preserved: the result
slot's onTake receives the actual remainder, including count1 or EMPTY.

Input container changes invoke menu result setup before its ordinary container
listeners. A valid setup broadcasts the previous preview before comparing the
new result; a changed result then receives a second broadcast. A genuine absent
saved-map record retains the previous result without either broadcast. Empty
inputs remove the preview without an immediate broadcast. The result
container's own setChanged also runs this setup; after raw result extraction,
Slot.setByPlayer(EMPTY) can therefore recreate a preview from unchanged inputs
before cursor installation and onTake. These are source boundaries, not a second
preview implementation.

## Actual map transaction and aliases

`CartographyMenuHost` is mandatory. Its key identifies the captured actual
level/table context and saved-map storage; its revision guards the real saved
records/ID owner. It supplies genuine lookup, source game time, validity,
supported callback context, detached staging and atomic publication. No native
map record, ID sequence, dirty owner or production no-op host is fabricated here.
The host's real `Item.onCraftedBy` removes the map-postprocessing marker, looks up
the old map, allocates/stores a scaled or locked map and updates the mutable
argument's ID in the source order. The existing special-context helper can serve
that component/callback sequence when backed by its real `CraftedMapOwner`.
Direct `Item.onCraftedBy` does not invent the separate ItemStack crafted-stat
wrapper, recipe unlock, criterion or advancement operation.

Quick-move calls this callback on the live result **before** attempting inventory
insertion. A full inventory can therefore still commit the allocated map ID and
processed result, with inputs unconsumed. Retry uses that processed result and
does not allocate again for its removed marker. A repeated shift can subsequently
reach another source callback prefix before discovering insufficient capacity.
This side effect is preserved; no destination reservation rewrites it.
Preview construction and creative clone alone never allocate IDs.

The mutable argument follows actual Java aliases. Empty-cursor pickup installs
the removed object before onTake, so postprocessing changes the cursor's map ID.
Empty-hotbar/offhand SWAP similarly aliases the installed inventory object.
Matching nonempty-cursor pickup first grows the existing cursor, then calls
onTake on the detached removed argument: its later component/ID mutation must
not replace the existing cursor's identity. Throw keeps the processed removed
argument for its subsequent actual drop. Shift's first callback aliases the live
result; its onTake argument is the remaining source object after transfer.

`ItemCraftedBy` events record before/after arguments and observer phases after
the real callback has executed against the detached host stage. They are
observations: publishing them must not execute the callback or allocate again.
Mandatory actual player-inventory change operations, sound, drops and ordered
sync/listener events carry their source inventory/input/preview/cursor phase.
The host must stage these actual operations successfully before plan commit.
The tick-guarded sound is at the captured block, BLOCKS category, volume/pitch1,
after input consumption and item callback, before base Slot.onTake.

A plan is non-Clone and validates the opaque nonreused UI instance, checked
revision, complete menu snapshot, canonical player inventory, actual owner
key/revision and game time. Closing marks this UI permanently closed. Further
clicks, initialization, broadcasts, repeated removal and old prepared actions
reject; a same-window reopen obtains a different lifetime identity. Baseline
transfer after removal remains available to the central lifecycle owner. The
plan checks context and drag certification before staging and again at commit. Click validity is rechecked; removal can proceed
after a block becomes invalid. The real host must detach all mutable state in
`stage` and publish it atomically or leave live state unchanged in
`commit_staged`. Cloning a mutable shared handle does not meet this contract.
Reentrant inventory/map/listener callbacks and unsupported world/owner contexts
must be rejected before admission.

## Close and production call phases

The server close packet precedes these effects when Main initiates closure.
The old menu's actual cursor return/drop runs first, while inputs and preview
still exist. Cartography then explicitly removes the derived preview, removes
one input at a time without container callbacks, and returns/drops only that
removed argument. Later inputs remain in their owner until their turn. Alive
returns preserve each raw -2/state0 insertion packet before its overflow drop;
dead/disconnected removal never inserts those items into inventory. The preview
never enters an owned return or drop. Window0 removal ordering and old shared
slot-baseline transfer remain actual central lifecycle responsibilities.

Main must bind this controller to the actual captured level/table position,
player/window/state-ID owner, real saved-map storage and MapIndex dirty/persist
transaction, direct item callback, game time, sound, Inventory.timesChanged,
listeners and actual Item/drop RNG/IDs. Open retains window0; packet actions
prepare on canonical inventory, stage actual operations, validate and commit
once, then publish the ordered stream. The player phase broadcasts before its
source validity/close decision. Opening's interaction stat and genuine
world/player lifecycle remain with their existing owners.

## Focused verification

Native flows exercise closed-UI/reopen and unresolved FoodProperties rejection,
stable prototype-food admission, real staged fixture map mutation before failed
shift, retry with the same processed ID, whole two-map extraction, mutable cursor versus
detached aliases, result SWAP, partial duplicate shift, cursor-first close,
drag/pick-all/clone/throw and stale owner/UI rejection. These fixtures model only
the map fields needed by each observation; they do not implement or certify a
production saved-map owner. With the assigned shared CARGO_TARGET_DIR,
`cargo test --lib cartography -- --nocapture` passed the six controller flows and
two existing kernel flows on 1 November 2026. The two owned Rust files passed
rustfmt checks; the owned diff passed whitespace checks. Existing unrelated
compiler warnings remain.
No protocol-client, graphical-client or original-server execution is claimed.
