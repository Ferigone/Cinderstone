# Enchanting menu, seeded offers and actual owners

Target: Minecraft Java **1.21**, protocol **767**. Primary evidence is direct
**JDK 21 `javap -p -c`** inspection of the original classes using official
mappings. No REA execution is claimed. Inner server JAR SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official server mappings SHA-1:
`31c77994d96f05ba25a870ada70f47f315330437`.
Java artifacts, helper binaries and raw disassembly remain outside Git.

## Source ledger

- `EnchantmentMenu` (`cqp`) has MenuType ID **13**, item input slot 0 with
  maximum count 1, lapis-only slot 1, then 36 borrowed player-inventory slots.
  It has **no result slot or additional preview item**. Its ten DataSlots are
  costs 0/1/2, actual enchantment seed, enchantment clues 0/1/2, level clues
  0/1/2, in that order.
- `slotsChanged` consults the input's actual item `isEnchantable` callback and
  the effective ordinary enchantment component's presence/emptiness. An empty
  or unenchantable input clears costs to 0 and clues to -1 without broadcasting
  inside that branch. An enchantable input evaluates the captured Level and
  broadcasts after generating all offers.
- `EnchantmentTableBlock` (`dik`) scans all 32 outer-ring positions from
  `BlockPos.betweenClosed(-2,0,-2,2,1,2)`, x-fastest, then y, then z, retaining
  offsets with `abs(x)==2 || abs(z)==2`. For each, it requires actual block tag
  ENCHANTMENT_POWER_PROVIDER, then ENCHANTMENT_POWER_TRANSMITTER at offset
  `(x/2,y,z/2)`, using Java integer division. Cost generation caps that actual
  count at 15; the world scan itself is not truncated.
- `EnchantmentHelper.getEnchantmentCost` (`dae`) returns zero without draws
  for a nonpositive actual item enchantment value. Otherwise it draws
  `nextInt(8)+1+(shelves>>1)+nextInt(shelves+1)` independently for each option.
  Costs are `max(base/3,1)`, `base*2/3+1`, and `max(base,shelves*2)`; a menu
  cost lower than `option+1` becomes zero. The menu RNG is reseeded from the
  **actual captured player enchantment seed** before these three computations.
- `getEnchantmentList` reseeds that same menu LegacyRandomSource with the
  Java-int-wrapped `seed+option`, sign-extended to long. It resolves the actual
  enchantment registry's IN_ENCHANTING_TABLE HolderSet; absent tag means empty
  list, not a substituted default. HolderSet encounter order is preserved.
- `EnchantmentHelper.selectEnchantment` adjusts level using two bounded item
  enchantability draws and two Java floats, then rounds with `Math.round` and
  clamps to at least one. Candidate filtering uses actual primary-item rules
  (plain books bypass a false result), then the highest matching level between
  each definition's min/max costs. Weighted selection preserves encounter order;
  additional choices draw `nextInt(50) <= level`, filter compatibility against
  the last choice, select again and halve level. Compatibility excludes equal
  holders and both actual exclusive sets. `WeightedRandom` (`bpe`) sums weights
  as long and rejects totals greater than Integer.MAX_VALUE.
- A plain book with multiple selected enchantments removes one random entry.
  Offer display then performs another random selection from the remaining list
  for the actual registry protocol clue and level. Selecting an offer regenerates
  its list from the same seed; it does not enchant from just the displayed clue.
- `clickMenuButton` accepts only indices 0–2. Required lapis/levels spent are
  `option+1`, while minimum player level also includes the displayed cost.
  Instabuild bypasses the lapis/level gate. A gated, nonempty option whose actual
  enchantment list is empty returns true but performs no payment/reward/sound.
- A successful action first invokes actual
  `Player.onEnchantmentPerformed(originalItem, option+1)` (`cmx`), **including
  for instabuild players**. The source subtracts that many experience levels;
  a negative level clamps to zero and resets experience progress/total. It then
  refreshes enchantmentSeed from the **player's own RNG**. `ServerPlayer`
  (`aqv`) delegates and invalidates its experience synchronization cache.
- Plain-book conversion uses `ItemStack.transmuteCopy(ENCHANTED_BOOK)`, retaining
  the count and component **patch**, rather than copying the old prototype's
  entire effective component map. Its setItem callback invokes container
  listeners and menu slotsChanged before enchantments are added. Each actual
  holder is then applied through `ItemStack.enchant`/`updateEnchantments`.
- Lapis consumption uses actual player infinite-material rules. Empty lapis is
  set to EMPTY and triggers its container callback **even when it was already
  empty for instabuild**. Then the actual ENCHANT_ITEM statistic is awarded;
  server players trigger ENCHANTED_ITEM with the changed input and payment.
  Container setChanged runs, menu seed reads the newly refreshed player seed,
  and slotsChanged runs again. Only afterward does Level RNG draw the sound
  pitch `nextFloat()*0.1F+0.9F`, followed by
  `playSound(null, pos, ENCHANTMENT_TABLE_USE, BLOCKS, 1, pitch)`.
- `EnchantmentMenu$1` first invokes SimpleContainer.setChanged listeners, then
  menu slotsChanged. This nested ordering is retained during book conversion,
  lapis clearing and the final callback. `stillValid` checks the actual table
  and player reach with additional range 4.0. Closing returns only the two
  actual inputs through the shared cursor/clearContainer lifecycle.
- `quickMoveStack` moves either top slot to reversed player slots `[2,38)`,
  player lapis to `[1,2)`, or exactly one other player item into empty slot 0.
  It does not fall back to a main-inventory/hotbar shuffle when slot 0 is occupied.

## Native API and integration boundary

`src/workstations/enchanting.rs` implements cost calculation, source bookshelf
query order, definition/cost matching, weighted/exclusive selection, seed/list
regeneration, book removal, clues, slot rules and transfer routing, component
patch conversion, and ordered successful-action callbacks. `EnchantingState<S>`
owns only the two temporary inputs and menu-local offers/RNG/seed. Its snapshot
identity `S` comes from the actual registry owner; a changed snapshot requires an
explicit owner handoff rather than silently using old clue IDs with new data.

`EnchantmentRegistry` requires actual holder identity/encounter order, item
callbacks, definitions, primary-item/exclusive-set and protocol mappings, and
actual component-map upgrade ownership. It has no production default or fake
selection table. `EnchantingBookshelves` resolves real world tags and chunks.
`EnchantingHost` requires actual player XP/seed/RNG, statistics, advancements,
container listeners, MenuSync broadcasts and Level RNG/sound callbacks.

`set_input` and `click_menu_button` stage native menu state. `preflight` must
establish all real owner/callback capabilities before mutation; callbacks run
against the host's staged transaction, which must also be discarded on error.
Cloning only a menu cannot roll back a consumed player/Level RNG or awarded
advancement. Unsupported hosts must fail explicitly during preflight. Broadcast
callbacks receive each actual intermediate two-slot state and all ten data
values; they must use the shared MenuSync and borrowed player inventory without
publishing a separate input owner or appending an invented result slot.

`src/container_workstation_enchanting.rs` adds `EnchantingMenu<S,K>` over the
same core and shared `MenuSync`. `K` is an actual captured player, Level/position
and menu-lifetime binding supplied by the host, checked on every action; equal
contents or seeds do not identify another owner. Opening requires
`create_menu_random_source()` from the actual `RandomSource.create` factory and
seed-uniquifier owner, then reads the actual player seed. The pure kernel alone
does not claim that constructor factory side effect.

The controller implements Pickup, QuickMove, Swap, Clone, Throw, QuickCraft and
PickupAll using the same canonical player inventory exposed to advancement
rewards. Intermediate broadcasts read that owner, including the already-shrunk
source during a shift transfer. All source Slot/SimpleContainer dirty boundaries
are retained, including double setByPlayer callbacks and a full matching slot
accepting zero items. The actual Inventory.setChanged and Inventory.add owners
remain mandatory host callbacks. Same-backing overfull SWAP preserves the
post-split displaced remainder instead of duplicating the original full stack.

QuickCraft membership lives in the controller; its HashSet encounter order
must come from the actual slot-identity owner, with no invented production
sorting. A missing owner rejects that context. A one-slot creative drag resets
then attempts PICKUP(button2), which is a no-op under the source dispatch guard.
Bundle pickup overrides and reentrant listeners remain explicit unsupported
contexts. Shared prediction reconciliation, state-ID increments and cursor sync
run against the actual authoritative mutations.

The host stages SyncEvents inline with player/world callbacks rather than
collecting all packets after advancement and sound effects. On an accepted
button the controller performs the packet handler's final broadcast **after**
the sound; rejected buttons do not get that extra broadcast. Closing stages
cursor then input0/input1 returns or real drops in the shared removal order,
then deactivates this controller. The central server-initiated close packet
must precede those effects; the caller transfers shared local/remote baselines
back to window0 afterward.

Main opening, actual registry/component-holder/player/advancement/world hosts,
actual factory/slot-identity contexts, packet dispatch and publication are
**pending central integration**. Production phases are block-use opening and
initial listener-before-content synchronization; authoritative click/button
packet handling; the real player menu broadcast/validity phase; and explicit
close/disconnect cleanup. No Main file is changed here. These callable native
modules do not establish complete enchanting gameplay or an aggregate checklist
row.

## Focused verification

The unchanged original JAR was executed directly through a local reflection
helper (JDK **21.0.12.1**), calling `EnchantmentHelper.getEnchantmentCost` with
an actual plain-book ItemStack and actual LegacyRandomSource. Fifteen-shelf
costs and the following unbounded draw were:

| Seed | Costs 0/1/2 | Following nextInt |
| --- | --- | ---: |
| 0 | 8 / 13 / 30 | -1930858313 |
| 123456789 | 8 / 11 / 30 | -458096230 |
| -123456789 | 7 / 18 / 30 | -619273243 |

The native focused check matches those values and draw order. The unchanged
`selectEnchantment` helper was also invoked with the actual bootstrap Unbreaking
holder in a deliberately restricted single-holder Stream, an actual book,
seeds `123456789+option` and costs 8/11/30. Native selection matches the observed
levels and the following unbounded draw:

| Option | Selected Unbreaking level | Following nextInt |
| --- | ---: | ---: |
| 0 | 1 | -458096230 |
| 1 | 2 | -1947763183 |
| 2 | 3 | 1412103999 |

This is a restricted-stream helper observation, not a full-default-tag menu
observation. Native contract flows additionally exercise a clearly restricted
recording registry fixture containing the pinned Unbreaking definition (not a default vanilla registry),
book conversion, lapis clearing, player callback/stat/criterion/seed/sound order,
instabuild payment callback, absent tag behavior and rejected unsupported owners.
These observations establish neither full Java menu gameplay nor Main callsites.

The bounded controller flow initializes a 38-slot, ten-DataSlot menu, shifts the actual
book/lapis sources before nested offers, selects an enchantment, observes the
source sound-before-final-sync order, then disconnects with exactly one owned
book dropped. Focused alias/drag checks cover overfull self-SWAP conservation,
one-slot creative-drag no-op, zero-accept full-slot callbacks and owner-binding
rejection. The recording insertion/order/factory hosts are test fixtures, not
production substitutes for the actual owners.
