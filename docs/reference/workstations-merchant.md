# Merchant: Java 1.21 costs, ownership and callback phases

Primary source is the pinned unchanged Java **1.21** inner server JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
and official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`.
This stage used **JDK 21 `javap -p -c`**, not REA, for `MerchantContainer`
(`crd`), `MerchantMenu` (`cre`), `MerchantResultSlot` (`crf`), `ItemCost` (`dbs`),
`MerchantOffer` (`dbu`), `MerchantOffers` (`dbv`), `Slot` (`crq`) and
`DataComponentPredicate` (`ko`). Java artifacts and raw disassembly remain
outside the source checkout. The native module is
`src/workstations/merchant.rs`; no Main callsite is added by this stage.

## Ownership and callable contract

`MerchantState<Binding, OfferKey>` owns two payments and a derived result preview,
selection hint, active offer handle and future merchant XP. It borrows ordered
canonical offer views from the actual merchant. Its generic binding must identify
the actual merchant **and** the trading player; it must not be a block coordinate
or an invented villager. An offer key identifies the actual offer object, rather
than only its current list index. No offer generation, entity, advancement,
reputation, gossip, restock clock or player inventory owner is fabricated.

`MerchantStage` represents a **private transaction of real owners**. Its methods
must read the actual staged offers and XP, execute item-crafted, merchant-trade,
trade-update, statistic, sound and XP callbacks into that stage, and publish it
atomically. Callbacks run synchronously: for example, `notify_trade` changes made
by the actual merchant must be visible to the following result recomputations.
The host supplies a unique stage key. Single-use plans capture that key and the
exact old native state; commit rejects another stage, merchant/player binding,
changed menu state or changed borrowed inventory. A rejected/private stage must
never publish partial statistics, offer uses, XP, sound, entities or destination
items. A common transaction lock must cover stage publication and the native
state/inventory replacement that immediately follows it.

`prepare_payment`, `prepare_remove_payment`, `prepare_selection`,
`prepare_refresh`, `prepare_take`, `prepare_selection_and_autofill` and
`prepare_close` produce such plans. `taken()` exposes reserved stacks for
preparing the actual destination; ownership transfers only when `commit`
succeeds. Closing removes only the two payments after the host handles the actual
menu cursor and clears the merchant's trading-player reference. The preview is
never returned as another item. Reentrant menu mutations during callbacks and
live owner replacement without rebinding the menu are unsupported.

## Cost and recipe facts

`ItemCost.test` checks the actual item and only its required component predicates.
Extra input components are allowed, and counts are tested separately. The native
implementation reuses the shared typed component comparison; expected food
component **20** is explicitly rejected because `FoodProperties` may contain an
`ItemStack` whose Java reference identity cannot be established from wire bytes.
Holder IDs must refer to the same actual registries as the owning merchant.
`ItemCost` counts use the source's positive-int domain, not an assumed stack-size
maximum. Payment slots themselves clamp to the item maximum and container **99**.

The modified first count is:

`clamp(base + max(0, floor((base * demand as Java int) as float * multiplier)) + specialPrice, 1, costStack.maxStackSize)`.

The multiplication before promotion and integer additions wrap. The product is
Java `float`; `Mth.floor(float)` includes Java cast/overflow behavior. The second
cost is not discounted. `updateDemand` adds `uses - (maxUses - uses)` with integer
wrapping. `MerchantOffer.take` tests and consumes costs but does **not** check
stock, increment uses, give experience or perform merchant callbacks.

`MerchantOffers.getRecipeFor` restricts to a hinted index only when the hint is
**positive** and below the list size. Hint zero, negative hints and oversized
hints scan from the first offer. The scan returns the first satisfying offer,
including an exhausted offer. `MerchantContainer.updateSellItem` then tries
reversed payments when the first result is absent or exhausted; it does not skip
an exhausted first result and resume the original-direction scan. A single
nonempty payment in slot one is normalized to the first cost for matching.

With no payment, the result/future XP clear without `notifyTradeUpdated`.
With a nonempty payment and a nonempty offer list, matching sets the actual active
offer, copied result and offer XP; failure clears the result/XP. Trade-update
notification follows. If the actual offer list becomes empty while payment
remains, Java leaves an old preview/XP while clearing the active offer. The kernel
records that source state but rejects taking a stale preview without a real
active offer owner; dynamic offer-list removal is not silently turned into free
owned output.

## Result take and input transaction phases

Menu type is **19**: payments **0/1**, result **2**, player inventory **3–29** and
hotbar **30–38**. There are no menu DataSlots; merchant level, XP, progress-bar and
restock metadata belong to the actual offers packet/merchant owner. Payments use
ordinary item acceptance. Result placement is denied. Pick-all collection is
disabled for the entire menu. Validity checks actual trading-player identity;
it does not substitute workstation block reach.

`MerchantContainer.removeItem(2, requested)` removes the **entire** nonempty
result, even when requested is smaller. `MerchantResultSlot.remove` nevertheless
accumulates only `min(requested, oldResultCount)` for its crafted callback.
`ResultTake::SafeTake` covers the ordinary `Slot.safeTake` path: removing the
whole result, `Slot.tryRemove` clearing the emptied slot, and `Slot.setChanged`
recomputing the preview before `MerchantResultSlot.onTake`.

`ResultTake::QuickMove` requires the actual destination to have accepted a
positive transfer amount. Source order is `onQuickCraft(originalResultCopy,
transferredAmount)`, item-crafted callback, the actual merchant trade sound,
`Slot.setChanged` result recomputation, then `onTake(remainingSourceStack)` with
crafted amount zero. The sound requires the actual server merchant entity,
position and sound event, neutral category and volume/pitch one. A non-entity
server merchant context must explicitly reject that callback rather than invent
an entity or silently omit it.

`onTake` first runs the item-crafted callback. With an active offer it tries
payment consumption in normal then reversed order. Success invokes the actual
`merchant.notifyTrade(offer)`, then `TRADED_WITH_VILLAGER`, then writes payment
zero and payment one, recomputing after each. Both payment stack objects have
already been shrunk before either write, so each recomputation sees both consumed
values. The host's notify-trade implementation owns offer-use increments, merchant
rewards, RNG and trade criteria; the menu does not duplicate them. Finally,
`overrideXp(currentMerchantXp + offer.xp)` runs outside the successful-take
condition. Arithmetic wraps as Java int. This XP is merchant progression, not an
immediate player-XP grant.

Selection/autofill first sets the hint and recomputes. It attempts to return each
old payment to player slots in reverse menu order. Already completed returns
remain when a later return cannot move. Only when both payments are empty does
it gather the selected offer's base first and optional second `ItemCost` from
ordinary inventory then hotbar. It gathers up to the item's maximum stack size,
not merely the required count, and only merges equal full components after the
first matching stack. The borrowed canonical `PlayerInventory` is staged once;
no second live inventory is retained by the menu.

## Verification and remaining integration

A temporary reflection harness called unchanged `MerchantOffer` and
`MerchantContainer` methods. The container received an explicitly labeled Java
Proxy merchant supplying actual canonical offers and no-op update notification;
this was an isolated environmental stub, not a vanilla villager or live gameplay.
Observed results:

| Operation | Original Java observation |
| --- | --- |
| Base 4, demand 3, multiplier 0.2, no special price | Cost 6 |
| Same counts, multiplier NaN / positive infinity / negative infinity | Cost 4 / 1 / 1 |
| Special price -20 with multiplier 0.2 | Cost 1 |
| Demand 3, uses 7, maximum uses 12, `updateDemand` | Demand 5 |
| First matching offer exhausted, hint 0 / hint 1 | Empty result / selected result count 3 |
| Remove result count 3 with requested count 1 | Removed count 3 |

Focused native checks exercise those facts plus component subsets, swapped
payments, callback ordering, positive/zero quick-craft callbacks, borrowed
inventory autofill, input-only closure and transaction guards. Their fixture
stage is explicitly not a production merchant owner. They do not establish a
PLAY client or live entity flow.

The central adapter still owns entity-use/open-screen dispatch, actual merchant
and offer lifetimes, the offers packet and selection packet dispatch, full menu
click modes, result-destination/cursor movement, state-ID/slot synchronization,
player validity phase, close/disconnect and real callbacks/world effects.
The implementation is a callable bounded kernel and completes neither
`inventory.4` nor `crafting.5`.
