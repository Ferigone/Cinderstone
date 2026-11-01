# Lectern menu controller — Java 1.21

`src/container_workstation_lectern.rs` adds a bounded native controller over the
existing borrowed lectern kernel. It owns only UI synchronization, cursor and
UI-lifetime/drag state. The actual block entity retains its book, page,
pageCount and block state. The actual host retains the canonical player
inventory, world/scheduler, persistent dirty writes, callbacks and drop/RNG
owners. There is no production Main callsite or whole-checklist completion.

## Primary source evidence

Inspection used **JDK 21 `javap -p -c`** and official Java 1.21 mappings, not REA
or an executed Java menu oracle. The pinned inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.

- `LecternMenu` (`cqz`): constructor26–40, clickMenuButton44–73,
  quickMoveStack78, setData83–85, stillValid89, book/page getters93/97;
  `cqz$1.setChanged`34–36.
- `LecternBlockEntity` (`drk`): onBookItemRemove151–154, setBook157–161,
  setPage164–170, load/save209–229, createMenu238 and pageCount247–255.
  Actual book-access container `$1`: get/remove/removeNoUpdate55–78,
  ignored setItem84, maximum1 at88, setChanged93–94, validity98,
  canPlaceItem103 and clearContent108. Actual data-access `$2`: get114,
  set119–122, count126.
- `LecternBlock` (`dkj`): resetBookState188–192, signalPageChange195–198,
  changePowered201–203, updateBelow206–207, scheduled tick211–212,
  openScreen323–328.
- `Slot` (`crq`): base mayPlace49 returns true, setters63–81, remove97,
  tryRemove110–127, safeTake136–138, safeInsert152–168.
  `AbstractContainerMenu` (`cpu`): ordinary click310–528, validity/index,
  data/synchronization, removed558–569 and slotsChanged589–590.
- `ServerGamePacketListenerImpl` (`aru`): container click1693–1737 and
  button1757–1772. `ServerboundContainerButtonClickPacket` (`agw`) uses
  VAR_INT for both container and button IDs. `ServerPlayer.tick`522–564
  broadcasts the active menu before its validity/close decision.

## Book, page and button behavior

MenuType17 has one book slot0 and one page DataSlot0, with **zero appended
player inventory slots**. Shared MenuSync supplies this exact layout. Its
normal 15-bit state IDs, client prediction correction, slot/data listener order,
retained full synchronization and cursor packets are used. No book or page is
copied into a second mutable owner. Window0's retained cursor remains in the
borrowed player inventory; the external cursor is separate.

Buttons1/2 move previous/next, buttons>=100 jump to button-100. The actual stored
pageCount controls Java min(max(requested,0),pageCount-1), including page=-1
for a zero-page book. The controller does not recompute live pageCount from
components. A changed page writes the actual page, marks the block entity dirty,
sets POWERED=true with flags3 while preserving the rest of the actual state,
updates below using the old state, schedules the real block tick after2 ticks,
then emits level event1043/data0. A no-op page change skips those mutations.
The actual scheduler clears POWERED and updates below through the existing
kernel, not a private menu timer.

LecternMenu.setData broadcasts even when the page clamps to its old value.
The accepted-button packet handler broadcasts again. Page buttons therefore
retain two broadcast calls; accepted take-book retains one handler broadcast;
invalid or permission-refused buttons retain none. Direct prepare_set_data
uses only the source setData broadcast.

Button3 checks actual Player.mayBuild, not a creative/spectator guess. It moves
the real whole book with removeItemNoUpdate, invokes onBookItemRemove (page and
pageCount=0, POWERED=false/HAS_BOOK=false flags3, BLOCK_CHANGE with no source
and the actual new state, then below with the old state), marks the actual block
entity dirty, and invokes actual Inventory.add on the same canonical inventory.
Only an unsuccessful add drops its actual remainder, around=false/trace=false.
There are no invented item-crafted, recipe, XP or advancement callbacks.

The additive kernel hooks factor this same source onBookItemRemove and
removeItemNoUpdate sequence and expose actual raw split removal. Ordinary
bookAccess.removeItem splits its actual stored stack and invokes onBookItemRemove
only when it becomes empty. It does **not** add a container dirty callback.
The source Slot.setChanged subsequently marks the block entity dirty and invokes
menu.slotsChanged/broadcastChanges. Empty tryRemove also calls the ignored setter
and its Slot.setChanged before cursor installation; onTake and final PICKUP
dirty calls retain their later cursor phases.

## Ordinary click bounds and source aliases

All ordinary modes are dispatched with the actual one-slot rules. QUICK_MOVE
returns EMPTY. Creative CLONE copies up to the actual item's maximum stack size.
THROW and empty-cursor PICKUP remove from the actual book owner; ordinary slot
removal has no button3 mayBuild guard in source. PICKUP_ALL against this sole
occupied pickup-permitted slot is a source no-op. QUICK_CRAFT can select at most
one Slot, so no substitute Java HashSet encounter order is needed. A single
selected target completes through the source PICKUP behavior (creative type2
uses button2 and does no ordinary PICKUP). Negative target indices are rejected
for the drag target phase; no nonexistent Slot is fabricated.

An unusual original-server distinction is preserved: the base Slot.mayPlace
returns true, but actual lectern bookAccess.setItem is a **proved no-op**.
Container.canPlaceItem=false does not replace that Slot predicate. Supported
setters retain the no-op plus actual dirty/broadcast callbacks. Full matching
insertion still calls it for transfer0; signed overfull capacity is retained,
so stored book2 plus matching cursor1 becomes book1/cursor2 before callbacks.
A nonmatching cursor larger than the slot's cap1 is refused with the normal
final dirty callback. Outside-left drop observes the old cursor before clearing;
outside-right splits the cursor before the actual trace=true drop.

A nonmatching cursor of count<=1 would install the actual stored book object in
the cursor while the ignored setter leaves that same reference in the block.
Ordinary SWAP similarly aliases the real book across block/player owners, and
some overflow paths mutate that shared object through Inventory.add. This stage
has no real cross-owner ItemStack reference owner, so those alias-creating
ordinary contexts explicitly reject **before dependent mutation**. It never
fabricates two independently owned books to approximate those paths. Existing
cross-owner aliases, custom/reentrant listeners, unknown item overrides and
unsupported actual owners must fail host admission. Bundle overrides reject
where ordinary item-click behavior would require their real owner. Unresolved
explicit food-component identity also rejects before generic item comparison,
sync or prediction processing; stable proved prototype components remain usable.

## Transaction and production contracts

`LecternMenuHost` extends the real LecternHost/LecternContainerHost. It supplies
captured actual player/session/world/position/block-entity binding, a revision
covering all actual mutable dependencies (including permissions/abilities,
book/page/count/block, canonical inventory, callbacks/scheduler/RNG), detached
staging and atomic commit. stage must detach **all** those mutable owners;
cloning a shared mutable handle is insufficient. Actual block callbacks, page
pulses, Inventory.add, drop creation and synchronization/listener callbacks run
once against that detached stage, using the observed source phases. Returned
LecternEvents are observations only; replaying them would duplicate callbacks,
drops or packets. The menu wrapper owns the kernel broadcast callback and does
not also call an independent second host broadcast.

The host provides canonical inventory by borrow; existing Inventory.add and
cursor-return paths use that exact owner. No independent inventory snapshot is
committed beside the host. A plan validates full native UI state, a checked
nonreused UI lifetime/revision, actual binding/revision, book/page/count and
canonical inventory before consuming commit. Actions recheck actual
Container.stillValidBlockEntity (same block entity, world, reach+4) and canonical
book presence; close/broadcast can still execute after validity becomes false.
Failed staging or atomic host publication leaves actual owners and UI unchanged.
Packet queues can be sent only after native UI assignment completes and without
reentry. Main supplies actual creative abilities and handles active window,
spectator/thread/session admission around these packet methods.

Close has no LecternMenu override and no startOpen/stopOpen callback. It returns
or drops **only the external cursor**, preserving per-insertion raw
Inventory -2/state0 packets before overflow drops. Dead/disconnected cursor
removal drops with around=false/trace=false. The live book/page stay in the actual
block entity; no preview or persistent input is returned. After close, this UI
rejects further actions; a new same-window menu has a different lifetime.

The actual player tick broadcasts before its validity decision and can close
on a later phase after a take emptied the book. Main must keep that ordering,
retain/restore window0, bind block opening and its interaction stat, and use the
real scheduler, redstone, book component/command resolution and persistence
owners. Written-book resolution occurs in the actual book set/load path;
this controller does not create a command/component-resolution owner. Source
NBT saves Book and Page while nonempty, derives pageCount on load and clamps
loaded Page. No menu copy is a persistent save.

## Focused validation

Focused native flows cover page pulse/no-op and the two page-button broadcasts,
permission/refused take, real block reset before inventory add/drop, source
cursor/removal/dirty phases, persistent-book close, signed and zero insertion,
ignored setter, alias rejection, all admitted mode families, actual host failure
rollback, full-state/stale owner/UI guards and explicit component admission.
These use controlled detached native fixture owners; they are not production
Main or original-Java gameplay observations. On 1 November 2026,
`CARGO_TARGET_DIR=/workspace/cinderstone-workstations-20261101a/target cargo test
--lib lectern -- --nocapture` passed the five controller flows, two existing
kernel flows and the shared zero-player-layout flow. Actual fixture success and
failed Inventory.add paths, atomic host-publication failure, retained window0
cursor and identical same-window/book/page/sync reopen rejection are included.
The two owned Rust files passed rustfmt and owned changes passed whitespace
checks. Unrelated compiler warnings remain. Comprehensive conformance suites
remain deferred.
