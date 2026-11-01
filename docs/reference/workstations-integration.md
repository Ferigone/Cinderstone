# Workstation implementation and production contracts

Target: Minecraft Java 1.21, protocol767. These are bounded callable native
ports. They do not complete the fixed aggregate deliveries `inventory.4`,
`crafting.2` or `crafting.5`, and have no new production Main callsites. Existing
chest/barrel, furnace and2x2/3x3 foundations remain pre-existing work.

## Identity, scope and publication

Instance `ws-20261101-7c2f`; assigned issue
https://github.com/Ferigone/Cinderstone/issues/3; branch
`teams/workstations/ws-20261101-7c2f/menus`; original base
`643106e8eb60d970a8df67eb9acf0ab5f107c788`.

Owned source is the new `src/workstations/**` namespace, five new
`src/container_workstation_{stonecutter,smithing,storage,enchanting,crafter}.rs`
adapters, and additive smithing ingredient predicates only in `src/crafting.rs`.
The only shared export changes are mechanical `pub mod` declarations for
`workstations` and those five adapters, using the coordination guide's explicit
standing exception. Main/runtime/world/player/container/furnace core, manifests,
metrics/checklists, deployment and licensing files were not changed.

Six disjoint subordinate groups and the coordinator used the actual seven-slot
maximum. They completed implementation and cross-review. The
[per-instance status](../coordination/status/ws-20261101-7c2f.md) contains exact
commits, file claims, validation and dated publication history. Ordinary Git
push published the branch at `ab08813e34fa8ab458208b99a13c50dfdce769d5`;
[draft PR #5](https://github.com/Ferigone/Cinderstone/pull/5) and the
[integration queue request](https://github.com/Ferigone/Cinderstone/issues/1#issuecomment-6563262903)
were created and read back. The earlier403 publication blocker is resolved.
Actual Main/runtime owner integration remains pending; no shared credentials
or automatic cross-chat communication are assumed.

## Native APIs and actual owners

| Family | Callable implementation | Required real production owner |
| --- | --- | --- |
| Stonecutter | `StonecutterState`, `StonecutterMenu`, prepared actions, seven ordinary click modes | Captured recipe manager/description ordering, enabled features, actual player/block, crafting/recipe/stat/sound callbacks |
| Smithing | `SmithingState`, `SmithingRecipeOrder`, `SmithingMenu`, seven click modes | Actual input/result recipe orders, registry snapshot, player/world/reward owners; intermediate broadcasts before result recomputation |
| Anvil | `AnvilState`, result/name/repair/cost plans and `finish_anvil_take` | Actual repair/text/enchantment metadata, player levels, captured block and PLAYER RNG for wear |
| Grindstone | `GrindstoneState`, prepared result/XP transaction | Actual enchantment/curse/min-cost registry, LEVEL RNG and real XP orbs at captured block |
| Loom/cartography | `LoomState`, `CartographyState`, explicit registry/map callbacks | Actual banner holders/tag encounter order; actual saved maps/ID allocator and taken-result item callback |
| Special recipe context | `RecipeManagerView`, `context_for_map_extension`, `postprocess_crafted_map` | Actual recipe snapshot/order and saved-map storage; previews never allocate map IDs |
| Enchanting | `EnchantingState`, `EnchantingMenu<S,K>`, seven click modes | `EnchantmentRegistry`/`EnchantingBookshelves`/`EnchantingMenuHost`: actual player/table binding, registry holders/definitions/order, constructor RNG factory, same canonical inventory, XP/stat/criterion/PLAYER-seed/LEVEL-sound callbacks and actual drag order |
| Merchant | `ItemCost`, `MerchantOffer`, `MerchantState`, `MerchantPlan`, `MerchantStage` | Actual merchant/player binding, stable offer objects, private real stage, synchronous trade/use/stat/XP/sound callbacks and offers/selection packet transport |
| Crafter | `CrafterState`, keyed `CrafterPlan`, `CrafterMenu<K>`, seven click modes and slot-state packet | `CrafterOwnerIdentity`, `CrafterRecipes`, `CrafterOutputOwner`, `CrafterMenuCallbacks`: actual block key, shared recipe cache/manager, scheduler, loot, hopper destinations, output postprocessing, Item/RNG/IDs, criteria and callback-order certification |
| Hopper/dispenser/shulker menus | `StorageMenu`, `StorageContainer`, prepared open/actions and phased events, seven click modes | Actual canonical container/key/revision and player inventory, startOpen/stopOpen, source container operations, comparator/dirty and player-counter callbacks, validity and drag certification |
| Hopper/shulker blocks | Native hopper insert/push/pull/entity kernels; `ShulkerLifecycle` | Actual counterpart containers/sided permissions, world/entity lookup, cooldown/time, Level RNG, block events, collision ordering and `Entity.move(SHULKER_BOX)` |
| Dispenser/dropper | `scheduled_tick`, `neighbor_changed`, default spawn/velocity/analog helpers | `DispenserHost`/`DispenserTriggerHost`: actual loot, Level RNG, selected behavior registry, facing/state, destination, real Item/entity construction and scheduler |
| Ender chest | `EnderChestInventory<P,K>`, lifecycle/use/lid APIs | One actual personal owner per player, actual captured chest key, EnderItems persistence, opener/scheduler/world/piglin callbacks; reuse existing three-row GenericContainerMenu |
| Beacon/brewing/lectern | Payment/live-data, brewing tick/slot/remainder, borrowed book/page/pulse kernels | Actual beacon data/effect/lock/structure owners; actual potion recipe owner/block ticker; actual book/player/scheduler/redstone owner |

Per-family `workstations-*.md` source ledgers give exact signatures, slot rules,
callback arguments and supported boundaries. The host traits have no fabricated
world, enchantment, recipe, merchant, entity or advancement implementation.
Isolated recording fixtures are labeled as such and cannot substitute for
production owners. An unavailable owner must fail explicitly.

## Shared menu and callback phase contracts

`MenuSync` owns only the external cursor,15-bit state ID, local listener
baselines and remote slot/carried/data baselines. Canonical station/block and
player contents remain in their actual owners. Ordinary layouts append36
player slots after station slots. Crafter is different: block inputs0..8,
actual player slots9..44, then derived noninteractive preview45. `top` for that
kind contains nine inputs plus the preview; data has ten entries. Enchanting
has two input slots and ten data entries. Storage tops are5/9/27 with no data.

- `initialize` observes local slots/data before initial contents/data packets.
- `broadcast_changes` retains per-slot local-listener then remote-packet order,
  followed by carried, then local/remote data. Contents/slot/carried packets
  increment state; data packets do not. Remote predictions do not suppress
  local inventory listeners.
- `send_all_data` updates only remote baselines, including under incremental
  suppression. `broadcast_full_state` first observes actual local changes.
- Capture state before the authoritative click; suppress incrementals during
  mutation; finish with client predictions updating only remote baselines and
  stale-ID full resync after the action. Spectator/active-window/slot/validity
  dispatch guards remain the actual packet handler's responsibility.
- `has_local_slot_changes` reads the local baseline without mutation. Crafter's
  actual recipe-cache listener must not run merely because remote data resends.
- `close_inputs_ordered` returns indexed packet/drop events: cursor first, then
  actual temporary inputs. A cursor overflow can precede a later input's raw
  insertion packet. Never pass derived previews as owned returnable stacks.
- `transfer_inventory_state` transfers only the old shared player local/remote
  baselines back to window0. It does not move preview/data/cursor ownership.

Storage and crafter events retain actual intermediate block/player/cursor
observations. These are callback arguments, not additional live inventory or
owned stacks. The host must stage operations against its same real owners and
publish packets/drops exactly once. Source `Inventory.setChanged` changes the
actual player inventory's transient timesChanged counter; no menu-local counter
is invented. Even an unchanged self-hotbar swap can call it twice. Source dirty
multiplicity and callback-phase snapshots must not be batched into final values.

Java drag targets use object-identity HashSet iteration. Storage/crafter require
actual callback-order independence certification, without a permissive default,
and revalidate it before commit. Enchanting requires the actual encounter order.
An ascending native slot order alone cannot certify effectful receiver order.
Reentrant item/advancement/menu mutation, bundle overrides and unsupported
identity-sensitive components remain explicit limits. Crafter rejects the
unported overfull self-swap alias context before mutation; storage/enchanting
preserve the source's aliased remainder rather than duplicating its old stack.

Actual block validity uses the captured object/world and real player's
`canInteractWithBlock(pos,4.0)`, not a guessed fixed eight-block sphere. Plan
identity checks must reject a replacement even with identical coordinates and
contents. Main must preserve actual old/new menu lifetime and player identity.

## Reserved Main contracts requested from integration-root-20261101

No reserved runtime patch is implicitly assigned by these APIs.

1. Route actual block/entity use to correct opening, window allocation and
   captured owner. Preserve source constructor/startOpen, previous-menu closing,
   open-screen and initial-listener/synchronizer order. Personal ender inventory
   is the player's owner, not an inventory attached to the world block.
2. Route PLAY click/button/name/beacon/crafter-slot-state/merchant-selection
   packets to the actual active player menu. Accepted enchanting buttons have
   their immediate final broadcast after sound, not only a later player tick.
3. Borrow the same actual player inventory for callbacks and transfers. Stage
   source item/stat/recipe/advancement callbacks at their recorded phases.
   Implement the real inventory counter/criterion owners, not no-op substitutes.
4. Run actual player-phase broadcast/validity and source close/disconnect/save
   order. Window0 removal precedes the actual external menu on disconnect;
   close-before-return publication and actual Item/RNG/shared IDs remain owned.
   Beacon payment always drops. World storage contents, personal ender items
   and lectern books are not returned as temporary crafting inputs.
5. Bind actual shared registry/map/recipe/cache snapshots, native block-entity
   registration/persistence and scheduler phases. Preserve hopper push then pull,
   shulker deferred block-event animation/movement, dispenser4tick trigger,
   crafter4tick edge/6tick animation/output/remainders and real loot contexts.
6. Keep constructor/global factory, PLAYER RNG and LEVEL RNG owners distinct.
   Actual Item construction consumes two discarded level velocity draws before
   default-dispense velocity (nine level doubles total for that path), with
   independent entity factories/IDs and source pickup delay. Registered special
   dispenser behavior cannot be replaced by generic item ejection.
7. Provide actual merchant object/offer/stage bindings, update uses and XP in
   synchronous notifyTrade before input recomputation, and supply network offers.
   Keep native special-context helpers attached to actual saved-map and result
   postprocessing owners; derived previews never allocate or own another result.

## Source and validation

Primary outer server SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner
SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Direct JDK21 `javap -p -c` supplied bytecode facts. Selected unchanged Java
helper-method executions are separately labeled with restricted fixture/context
scope. No REA execution or Java gameplay observation is implied. Java artifacts
and raw disassembly remain outside Git.

Final continuation source `7147da02c737360682386df4d73792cb8b03ae00` passed
`cargo test --locked --lib workstation` and
`cargo build --locked --bin mc121-rust`. These focused native flows cover source
ownership, intermediate callbacks/counters/cursors, replacement/stale guards,
result consumption, drag bounds/certification, remainders, RNG and slot layouts.
The same commands also passed in a separate compatibility worktree at pinned
master `404a5dab9c1b161b3587aa24e11703458c5a8edd` with exactly that source diff
applied. Master and the feature branch were not merged; no Main file was edited.
This proves compiler/link compatibility, not invocation by Main or gameplay.

The original-base all-target check found stale shared persistence-smoke Options/
Server initializers; that shared test was not silently changed. No deployment,
protocol-client flow for these menus, complete registry/owner implementation,
full anvil enchantment combination, remaining family click adapters, horse/entity
inventories, complete survival/loot/world integration or fixed-row completion
is claimed. Comprehensive conformance remains deferred.

A later publication pass repeated both focused native flows and the full Main
build against master `f40d8bf066ecb446b46a66e1d6d9465ffb233d6e`, including its
chat/session changes, in a separate source-overlay worktree. Both passed; no
master merge or Main edit occurred. The source remains `7147da0`. The posted
[integration request](workstations-integration-request.md) aligns publication
with that coordinator's ordered connection output and exact owner contracts.
