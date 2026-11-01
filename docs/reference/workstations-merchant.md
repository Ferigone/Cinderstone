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

## Native menu controller continuation

`src/container_workstation_merchant.rs` now supplies the bounded server menu
controller: all seven ordinary click modes, select-trade/autofill, derived
preview/cursor ownership, borrowed player inventory, source dirty callbacks,
state-ID/client-prediction synchronization and ordered removal. This is callable
native behavior, not a registered merchant entity or a Main invocation.

Additional primary inspection used `AbstractContainerMenu.doClick` (`cpu`),
`Slot.tryRemove/safeTake/safeInsert` (`crq`) and
`ServerGamePacketListenerImpl.handleSelectTrade` (`aru`, mapped lines 756–766),
again through pinned JDK 21 `javap -p -c`. Source details retained by the adapter:

- Payments accept ordinary items; result insertion is denied. Player shift
  transfer goes between main inventory and hotbar rather than inventing
  automatic payment targeting. Payment shift return is forward and result
  shift return reverse. Offhand swap uses actual raw inventory slot 40.
- Ordinary result pickup/throw removes the whole result, while its crafted
  amount remains the requested removal capped by the old result count. Right
  pickup of three outputs obtains all three and reports crafted amount two.
  The actual cursor is installed after the source remove/setChanged phase and
  before `onTake`. A result swap into an empty hotbar/offhand uses
  `Slot.setByPlayer`, not `remove`; its crafted callback amount is **zero**.
- Result shift transfer can be partial. With three bread and only one available
  destination space, Java transfers one, runs the positive crafted callback on
  the original count-three copy, plays trade sound, regenerates the preview and
  calls `onTake` on the old count-two remainder with crafted amount zero. It
  consumes the offer once; the overwritten preview is not another owned item.
  Failed shift insertion invokes no trade/crafted/sound callback.
- Repeated `MerchantContainer.setItem/setChanged` recomputations are preserved,
  including repeated `notifyTradeUpdated` calls. Unlike smithing's superclass
  ordering, explicit `MerchantMenu.slotsChanged` refreshes the trade result
  **before** superclass broadcast. Ordinary container `setChanged` refreshes
  without inventing a menu broadcast. Player `Inventory.setChanged` callbacks
  keep source multiplicity and intermediate inventory/source-stack observations.
- `PickupAll` is a valid no-collection action for **every** menu slot, including
  the player's slots. Creative clone remains a real creative action. Drag
  insertion excludes result and requires the actual selected Slot identity
  HashSet encounter order from the host; ascending native indices are not a
  substitute for an effectful callback order. Signed ordinary `safeInsert`
  capacity and overfull self-swap remainders preserve the source behavior.
- SelectTrade carries only one VarInt index, without window or click state ID.
  The actual handler requires current `MerchantMenu` and its trading-player
  identity, then invokes `setSelectionHint` before `tryMoveItems`. It has no
  extra spectator gate or immediate broadcast. Invalid indices still take the
  selection-hint phase but do not autofill. A later player broadcast observes
  native state. Merchant offers/level/XP/restock metadata are not menu DataSlots.

Packet click/selection preparation checks actual trading-player validity.
Broadcast remains callable after that validity is lost: the real player phase
broadcasts first, then checks validity and closes. It must not skip that final
observation by applying the packet guard to a broadcast.

The controller exposes `prepare_initialize`, `prepare_broadcast`,
`prepare_slots_changed`, `prepare_click`, `prepare_select_trade` and
`prepare_close`. `MerchantMenuPlan::validate/commit` binds one private
`MerchantMenuHost` stage to the actual merchant/player, its stable offer objects,
the entire borrowed inventory snapshot and this UI's nonreused instance plus
checked revision. The stage key must remain constant throughout preparation;
an identical later UI or reused window ID cannot accept the old plan.

`MerchantMenuHost` extends the existing `MerchantStage`. Its required callbacks
supply source-phase observations, real inventory dirty/add behavior, component
identity validation, drag encounter order, actual drops and ordered sync. It
must preflight the actual private owner transaction. Source observations contain
read-only item/inventory values, not another live inventory, extra result or
permission to publish copied items. Trade callbacks execute synchronously so
actual use/XP changes affect following recomputations. Pricing, stable offer
handles, advancement/criteria, player/world/Item/RNG and sound owners remain real
host responsibilities; there is no default villager, offer book or no-op trade
receiver.

The narrow crate-private `MerchantState::menu_*` bridges expose source raw slot
changes, result removal, recomputation and `onTake` phases to this complete
private action. Existing prepared-kernel entry points remain available and use
the same evaluation/cost/trade logic. No second result preview or duplicated
trade evaluator is introduced. Several slot phases do not call the old public
kernel commit separately: the complete controller stage publishes **once**.

Host callbacks already stage actual effects/sync/drops during preparation.
`commit` revalidates and invokes that stage's `publish` once, then installs the
native menu/inventory snapshots under the same actual-owner lock. Do not replay
callbacks or drops again after commit. The central single output FIFO retains
real world-add/tracking and packet order; a later terminal transport close does
not roll back a completed source mutation or recreate a published drop.

Removal uses the shared indexed packet/drop schedule, cursor first. It then
clears the real merchant trading-player reference, removes payment zero without
update, returns/drops it, then does the same for payment one. Source observations
retain the later payment until its own removal. It discards the preview and
marks this UI inactive. Alive returns retain per-insertion raw inventory packets;
dead/disconnected stacks use `around=false, traceItem=false`. The actual close
packet, when called for by source, precedes these effects; old shared baseline
transfer follows removal. No universal death phase is inferred from the policy.

Unsupported component/reference identity, bundle overrides, non-entity trade
sound, stale output lacking an actual active offer, or unavailable owners must
error. The required `callbacks_preserve_menu_owners` certification is checked
before preparation, after callbacks and at commit. Real callback/criteria
contexts that would mutate the menu, its canonical inventory/cursor or crafted
output need a direct bridge and must reject this bounded controller; merely
queuing a callback that silently loses those mutations is invalid. In
particular an output-mutating `Item.onCraftedBy` is not implemented by the
immutable kernel callback. Custom FoodProperties equality cannot be certified
by matching wire bytes; `require_stack_context` must resolve actual identity or
reject, and ItemCost component-20 predicates remain explicitly unsupported.

Focused controller flows passed whole-result right pickup/throw, callback
cursor/payment observations, partial result shift and original/zero crafted
phases, select-trade without invented broadcast, drag/clone/pick-all exclusion,
offhand swap, ordinary player shift, ordered cursor/detach/payment close and
stale/replacement UI guards. Additional edge flow checks result-swap amount zero,
signed overfull insertion and rejection of an actual unavailable trade callback
without changing live menu/inventory. Receivers are explicit private fixtures,
not a vanilla merchant or a live PLAY result.

Main still owns entity-use/open-screen dispatch, merchant/offer lifetimes,
offers packet transport and selection/click routing, actual creative/player
state, player-phase validity/broadcast, close/disconnect/death integration and
all required real callbacks/world effects. This continuation completes neither
`inventory.4` nor `crafting.5`.
