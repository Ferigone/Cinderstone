# Native AnvilMenu synchronization during item pickup

This source stage is authorized by
[issue3 comment6564589182](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564589182).
It owns only `main_anvil_pickup.rs`; Root owns its actual item-body, fallback,
current-menu recovery and export wiring. Production callsites remain pending
until that separate wiring and its focused runtime proof are present.

Primary evidence is the original Java1.21 inner server JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, and official
server mappings, SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`. The source
facts below were inspected directly with JDK21 `javap -p -c`; no REA inspection
or original Java gameplay execution is claimed. No raw disassembly or Java
artifact belongs in the repository.

## Source call phase and actual owners

`ServerPlayer.take(Entity,int)` (`aqv.a(bsr,int)`, mapped lines996–998) invokes
its superclass at bytecode offsets0–3, then invokes `broadcastChanges` on the
actual current `containerMenu` at offsets6–10. It does not validate the current
block, recompute anvil results, rename an input, charge XP or obtain an RNG.
Its later player-tick validity/close phase remains a separate operation.

The actual `ServerPlayer$2.slotChanged` listener (`aqv$2.a(cpu,int,cuq)`, mapped
lines278–287) first excludes `ResultSlot`, then compares the slot's backing
container by reference to that player's real Inventory (offsets16–28). Only
that owner invokes the inventory-changed criterion at31–46; its data listener
returns immediately. Thus anvil input/result/data/cursor synchronization cannot
manufacture an inventory listener or a dirty-counter change.

`ItemEntity.playerTouch(Player)` (`cjh.b_(cmx)`, mapped lines378–396) checks
client-side/delay/target, calls actual `Inventory.add` at offsets55–63, then
calls `take` at66–70. If the stack became empty, discard and count restoration
follow at73–87; pickup statistic and `onItemPickup` follow at90–105. Thus
pickup admission after `take` is too late to protect the canonical inventory.

Main's existing `RuntimeInventoryHost.add_to_inventory` mutates the actual
`SharedContainer` inventory through `add_pickup_to_inventory` without emitting
window0 updates. Its collection callback already publishes Collect and then
calls the external-menu synchronization closure with that same immutable
`PlayerInventory` and the real borrowed `MainGameplayFrame`. The anvil bridge
must consume that seam. It must retain the current `AnvilMenu`'s two inputs,
single derived result, external cursor, cost and local/remote baselines; it must
not open a replacement menu or borrow an invented anvil world/repair/XP host.

## Admission before canonical mutation

`preflight_anvil_pickup(players,id,candidate)` requires the actual active anvil
and captured binding to agree on player ID/UUID, position, dimension and window.
It certifies the alive creative Overworld, unmounted admitted native-body and
default-effect profile against actual player/lifecycle/body owners, together
with Main's supported empty-criteria gameplay floor. It deliberately performs
no current-block/reach check: even a removed anvil receives the immediate
source take broadcast before the later tick closes it.

It inspects the actual canonical inventory (including the inactive retained
InventoryMenu's cursor), current anvil inputs/result/cursor and candidate with
the central recursive FOOD component20 admission policy. Stable prototype food
is allowed. An unresolved explicit FoodProperties/nested ItemStack identity
fails before `Inventory.add`; the guard never strips components, replaces a
stack or changes `Inventory.timesChanged`. The actual context's initial
unenchanted profile also rejects nonempty ordinary/stored enchantment maps
(components9/23, recursively through item contents) with the typed
`UnsupportedAnvilEnchantmentContext`; preserved protocol IDs do not establish
the unavailable actual configured holder identity. Empty maps remain allowed.
The guard additionally prepares and drops a read-only current-menu sync plan
to validate active lifetime, hidden baselines and checked revision, and resolves
the actual inventory/candidate stack-limit queries before pickup. Root must call it for the actual
candidate before `player_touch_item` (or before the fallback `player_touch`
scan mutates any admitted item). Any rejected predicted UI operation must use
the actual current-menu recovery helper, never an unconditional window0 resend.

## Retained synchronization contract

`NativeAnvilRuntime.broadcast_for_inventory` composes the assigned core
`prepare_inventory_broadcast` bridge with the actual borrowed canonical
inventory. Its nonreplayable plan exposes the ordered `SyncEvent` and
`AnvilPhase` observations without recomputing a preview or calling unavailable
item/world/XP callbacks. `into_validated` checks the same nonreused UI token,
binding, retained state/drag/name/sync and checked next revision, then holds an
exclusive borrow of that exact menu through callbacks and infallible adoption.
No public `AnvilMenu` clone or permissive full-host substitute is used.

Before committing either listeners or UI baselines, the adapter finishes
every fallible packet encoding, component/slot-limit scan and the actual
`MainGameplayFrame.preflight_inventory_slot_changes` identity/empty-criteria/
statistics preflight. The central preflight validates the actual player and
prepares/drops the existing actual gameplay menu token without observations.
`MainItemGameplay.validate_active_player` also validates that registration
before canonical pickup. Those central APIs are requested in
[comment6564847678](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564847678);
their source grant/implementation is pending at this writing, so this adapter
does not yet claim compilation or production wiring. It then invokes the existing
inventory-slot listener owner for player-backed slots in source encounter
order and installs the prepared baselines infallibly under the same exclusive
owner locks. Top/result/data/cursor listeners do not become player-inventory
criteria. The existing inventory mutation owns its dirty counter; broadcasting
does not manufacture another `Inventory.setChanged` callback.

## Focused verification and limits

The admission regression covers unresolved candidate FOOD identity, the actual
retained external cursor, accepted stable-prototype food, unavailable incoming
enchantment-holder rejection and observational
nonmutation of the candidate and canonical inventory counter. The focused flow
uses real `MenuSync` encoding and `MainGameplayFrame` criteria against one
borrowed inventory: failed foreign-player preflight produces no observation,
the admitted actual slot produces one inventory scan/criterion, and window7's
slot3 packet preserves the external cursor, input/result preview and dirty
counter. A mapping regression checks result-listener exclusion, main/hotbar
backing and actual packet offsets. These tests are written but have not run
until the requested core/central dependencies and exports are present.
Compilation, the retained sync-plan flow and Root's actual callsite proof remain
to be recorded after those dependencies freeze. This file does not claim Main wiring,
broader player profiles or a complete inventory/checklist delivery.
