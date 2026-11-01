# Native CraftingMenu1.21

Reference artifact: official inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Source mappings: `CraftingMenu -> cqm`, `ResultSlot -> crm`, `Slot -> crq`.
Read with JDK21 javap; original bytecode is kept temporarily in
`runtime/crafting-source/{cqm,crm,crq}.txt`.

Implemented source facts:

- Visible slots: result0, grid1..9, main10..36, hotbar37..45.
  Offhand remains the canonical player inventory's slot45, reachable through
  the swap button40 but absent from the table's visible slot list.
- `cqm.quickMoveStack`: result moves into10..46 in reverse. Inventory entries
  first try1..10; only a failed grid move exchanges main/hotbar ranges. Grid
  entries move into10..46 forwards. A partially transferred result consumes one
  recipe and drops its output remainder (not a second invisible output).
- `cqm.slotChangedCraftingGrid`: records the remote result baseline and sends
  an explicit result-slot packet on each actual grid notification, even for an
  unchanged result. State ID uses the original15bit increment.
- `crm.remove`: result removal returns the whole result stack; the crafted
  statistic records min(requested amount, source count). `crq.onQuickCraft`
  records the source-copy count difference before `crm.onTake`.
- `crm.onTake`: consume in the occupied rectangle row order, then return each
  actual recipe remainder to its grid slot, canonical inventory, or drop owner.
- AbstractContainerMenu click modes reuse the existing source-derived native
  implementation with table-specific slot restrictions and ranges.
- No canonical player inventory is retained in the menu. Every operation
  stages a temporary slot view from the current shared inventory and commits
  it only on success. The retained state is the9input grid, result, cursor,
  drag state, recipe and synchronization baseline.
- The Main bridge uses the shared menu/drop/RNG/statistics commit seam,
  common window counter, actual distance/block checks, and normal close at
  disconnect and shutdown.

Explicit remaining boundaries: limited-crafting recipe unlock ownership;
recipe-book recipe placement; dynamic map scaling/locking post-processing;
world map context when not supplied; dead/disconnected-player alternate close
semantics; advancement registrations beyond the actual native empty-criteria
owner; imported runtime sources. No full1:1 or full-station completion claim.
