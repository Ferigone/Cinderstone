# Lectern menu and canonical book ownership

Minecraft Java **1.21**, protocol **767**. Inspected the unchanged server with
**JDK 21 `javap -p -c`**, using the official server mappings. This is direct
bytecode inspection, not REA execution or an observed Java gameplay run.
Inner server JAR SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official server mappings SHA-1:
`31c77994d96f05ba25a870ada70f47f315330437`.
Java artifacts and raw disassembly remain outside the repository.

## Source facts

- `LecternMenu` (`cqz`) has protocol menu ID **17**, **one book slot**, and **one
  page DataSlot**. It does not append the player's 36 inventory slots.
  `getBook` and `getPage` read the supplied actual container/data owner.
  `quickMoveStack` always returns empty. The book belongs to the block entity;
  closing the menu does not return it to inventory or create a second stack.
- `clickMenuButton`: ID 1 requests previous page, ID 2 next page, ID 3 takes the
  book, IDs at least 100 request page `id - 100`. Unknown IDs return false.
  Only book-taking checks the actual `Player.mayBuild()` permission. Page
  navigation does not derive this permission from a guessed game mode.
- `LecternBlockEntity` (`drk`) `setPage` uses
  `min(max(requested, 0), pageCount - 1)`. A zero-page book consequently clamps
  to **-1**. Replacing that expression with an assumed nonnegative clamp would
  change the source behavior. A changed page is stored, marks the block entity
  changed, then invokes `LecternBlock.signalPageChange`; an unchanged page does
  not pulse. `LecternMenu.setData` broadcasts changes even for that no-op.
- `LecternBlock` (`dkj`) `signalPageChange` sets POWERED true with block flags 3,
  updates neighbors below, schedules this actual block for **two ticks** and
  emits level event **1043**, data 0. The actual scheduled block callback clears
  POWERED with flags 3 and again updates neighbors below. This is a real BLOCK
  scheduler callback, not a menu-owned timer.
- `LecternBlockEntity$1.removeItemNoUpdate(0)` moves the actual book out, empties
  its slot, then `onBookItemRemove` resets page and pageCount to 0. It resets
  POWERED and HAS_BOOK false with flags 3, emits BLOCK_CHANGE with **null source
  entity** and the new state, then updates neighbors below using the old state.
  The menu then calls container `setChanged`, invokes actual `Inventory.add`
  with that owned book, and only on false calls `Player.drop(remainder, false)`.
  The insertion mutates the same stack to its actual remainder; neither a full
  original-book drop nor a cloned preview is a substitute.
- `LecternBlockEntity$1.stillValid` requires the actual block entity still be
  present **by identity** in its world, the player's
  `canInteractWithBlock(pos, 4.0)`, and `hasBook`. The last predicate checks the
  canonical writable-book or written-book items. Coordinates alone do not
  transfer an old menu to a replacement lectern.
- `getPageCount` uses written-book content when present, otherwise writable-book
  content, otherwise zero. `getRedstoneSignal` computes in Java float:
  `floor((pageCount > 1 ? page / (pageCount - 1.0F) : 1.0F) * 14.0F) + hasBook`.
  `LecternBlock.getAnalogOutputSignal` first checks actual HAS_BOOK and actual
  lectern block-entity ownership, returning zero otherwise. Calling the inner
  formula indiscriminately for an empty lectern is incorrect.

## Native API and integration contract

`src/workstations/lectern.rs` implements stateless button dispatch, page
clamping, page-pulse callbacks and scheduled pulse ending, book taking and
remainder dispatch, validity, page-count decoding and comparator arithmetic.
It owns no book/page/inventory copy. `LecternHost` borrows the actual captured
block entity, world and player transaction. Every owner/callback method is
required; none silently succeeds as an unsupported no-op.

`preflight(LecternAction)` must establish the actual world, block/entity
identity, player permission/inventory/drop, scheduler/neighbor and sync
capabilities needed for the action before publishing mutations. Callbacks
operate on a transaction staged by those real owners. An error discards that
whole transaction, including book moves and Item/RNG effects. Cloning a menu
alone cannot provide that guarantee. The raw book/page methods deliberately do
not repeat callbacks already ported by the helper.

`set_page` invokes dirty, world pulse, scheduler, event and broadcast in source
order. `click_menu_button` takes the actual book only once and leaves Inventory
insertion and Item drop effects with their existing canonical owners. The
scheduler passes its actual current state to `scheduled_page_pulse_end`; no
menu-local block state or timer substitutes for it.

Main block-use opening, captured persistent lectern block-entity ownership,
button packet dispatch, ordinary slot/cursor click handling, listener/state-ID
synchronization, redstone receivers and actual scheduled BLOCK integration are
**pending central integration**. Lectern book placement, written-book command
resolution, persistence and block-removal book drops are not delivered here.
The shared synchronization adapter must use one top slot and one DataSlot,
without appending player inventory. Its base cursor lifecycle remains separate
from the persistent book. No complete checklist row is claimed.

Focused native flows cover bounded/no-op page navigation and source callback
ordering, the zero-page clamp, comparator arithmetic, permission-denied book
taking, and actual remainder/drop ordering in a recording test host. These are
native contract checks, not Java/runtime or Main integration observations.
