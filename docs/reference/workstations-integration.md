# Workstation stage ownership and production contracts

Target: Minecraft Java 1.21, protocol 767. This team stage does not complete
`inventory.4` (all block menus), `crafting.2` (all special/trim contexts in Main)
or `crafting.5` (3x3 and all stations). Their fixed definitions and scope_version 1
remain unchanged. Existing chest/barrel, furnace and crafting foundations are
pre-existing work, not this team's delivery.

## Claim and communication

Instance `ws-20261101-7c2f`, isolated clone
`/workspace/cinderstone-workstations-20261101a`, branch
`teams/workstations/ws-20261101-7c2f/menus`, base origin/master
`643106e8eb60d970a8df67eb9acf0ab5f107c788`.

The session exposes seven simultaneous slots. The coordinator launched six
subordinates: stonecutter, smithing, anvil, grindstone, loom/cartography and
special context/evidence. This records actual observed capacity and launched
work, not any other external session or a compatibility measure.

At startup issues #1/#2/#4 had no comments and there were no open PRs; #3 assigns
the unused workstation namespace to one external instance. The startup claim
and integration request were prepared before implementation, but **both** the
GitHub connector comment and authenticated gh addComment were rejected with
`403 Resource not accessible by integration`. The user has been asked to grant
Issues, Contents and Pull requests write access. This note retains the exact
local claim; it is not a posted GitHub claim or an atomic lock. No prior claim
was treated as expired. Issue posting/publication status must be rechecked after
authentication changes.

Owned implementation: new `src/workstations/{mod,menu,stonecutter,smithing,
anvil,grindstone,loom,cartography,special_context,lectern,beacon,brewing}.rs`, new
`src/container_workstation_stonecutter.rs` and
`src/container_workstation_smithing.rs` for bounded inventory adapters; additive
`RecipeBook` smithing ingredient predicates only in `src/crafting.rs`.
Team source ledgers are `docs/reference/workstations-*.md`.
The only shared export edits are mechanical `pub mod` additions in `src/lib.rs`,
using the explicit standing exception in `docs/coordination/README.md`.
No Main, world/player/container/furnace core, manifest, status, checklist,
persistent journal or deployment files are assigned or changed.

## Shared synchronization/lifetime API

`workstations::menu::MenuSync` owns the external menu cursor, 15-bit state ID,
local listener baselines and remote slot/carried/data baselines. Station inputs
stay in their station, result previews stay derived, and actual
`PlayerInventory` is borrowed per operation. Its `snapshot(top, inventory)` uses
station slots then the existing canonical inventory indices 9..45 (main then
hotbar); offhand is not a visible workstation slot.

- `initialize(top, inventory, data)` observes local slots/data before sending
  initial contents and data, without preliminary slot-packet state increments.
- `broadcast_changes(...)` returns ordered `SyncEvent`s: local slot listener,
  that slot's remote packet, next slot, then cursor, then local/remote data.
  Predictions never suppress local inventory listeners. Data packets do not
  increment the menu state ID; contents/slot/carried packets do.
- `send_all_data(...)` updates only remote baselines, including under incremental
  suppression. `broadcast_full_state(...)` first runs local changes.
- The packet handler captures state before the action, suppresses incrementals
  during authoritative mutation, then calls `finish_click(click, old_state,
  top, inventory, data)`. Changed-slot/cursor predictions update only remote
  baselines. A stale click still executes and receives full state afterward.
- `close_inputs_ordered(inputs, inventory, MenuCloseContext)` stages cursor first and
  owned temporary inputs in source order. It calls the existing inventory-return
  API for an alive connected owner, or retains real Player.drop arguments for
  disconnected/dead owners. `MenuRemovalEvent` indexes insertion packets and
  actual drop callbacks in their exact order, including mixed alive overflow.
  The aggregate `close_inputs` compatibility method alone cannot express that
  interleaving. **Never pass derived result previews as inputs.** Beacon payment
  has its separate always-drop source rule; lectern books and brewing contents
  remain with the actual world owner.
- `transfer_inventory_state` preserves old listener/remote baselines only for
  the 36 actually shared inventory slots when returning to window0.

`SyncEvent` is an ordered observation contract for non-reentrant listeners.
The host must run actual ServerPlayer inventory criteria at their recorded
boundary and stage actual drops before publishing menu state/packets. These
observations do not implement advancement ownership or mutating/reentrant
listeners. `WorkstationAccess` delegates captured-block matching and the actual
player's `canInteractWithBlock(pos, 4.0)`; do not replace this attribute/eye/AABB
query with an old fixed eight-block sphere.

## Reserved production seams requested from the integration coordinator

No new Main callsite is claimed. The current base only routes the pre-existing
families. Required changes remain central-owned:

| Production phase | Current location / required contract |
| --- | --- |
| Block use/opening | Existing `main_crafting_table::Server::use_native_crafting_table` and storage/furnace paths are examples; route the actual workstation block use, allocate window, capture level/position and borrow real player inventory, send open before initial synchronization |
| PLAY click | `src/main.rs` PLAY `ContainerClick::decode` dispatch needs active workstation ownership, matching window, spectator remote-only resend, actual validity/slot checks, transactional action and ordered effects |
| Button/name packets | Add station-specific button selection and anvil name dispatch; use the captured recipe/pattern/registry owner, never a copied client recipe catalog |
| Player phase | Parallel the existing crafting-table/furnace lifetime phase: actual menu broadcast and validity checks at the source player phase; invalid menus close with the real lifecycle context |
| Close/disconnect/save | Existing `cleanup_player_admitted`, menu-close paths and save cleanup need cursor-before-input handling, actual disconnected Item/RNG/entity-ID transaction, close-before-return publication and old shared-baseline transfer |
| Item/recipe callbacks | Item.onCraftedBy vs ItemStack.onCraftedBy, actual stats, recipe-crafted criterion and recipe awards must execute with pre/post-consumption arguments and ordering specified in each ledger |
| XP/RNG/block callbacks | Grindstone uses actual Level RNG and XP-orb award at captured block; anvil charges real player levels and uses actual Player RNG for wear. Keep owners distinct |
| Registry/world context | Real recipe-manager ordering, enabled features, banner holders/tag order, enchantment holders/curse/min-cost rules and saved-map allocation/storage must be borrowed from their owning runtime |

All world-mutating result paths require a surrounding transaction that validates
result destination compatibility/capacity and real callback support before
publishing any staged menu/inventory/world/RNG state. In particular map
postprocessing can change stack components before insertion. Callback snapshots
are observations, not extra owned stacks. Pending traits are not fake production
owners and must not be filled with permissive no-op implementations.

## Source evidence and remaining families

Primary artifacts are the original outer server JAR SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01` and official
mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`. Direct JDK 21
`javap -p -c` inspection supplied bytecode facts. No REA tool was registered;
no REA evidence or Java gameplay observation is implied. Artifacts/raw
disassembly stay outside Git.

Shared facts were inspected in `AbstractContainerMenu` (`cpu`),
`ContainerLevelAccess` (`cqg`), `DataSlot` (`cqn`), `ServerPlayer$1` (`aqv$1`),
`ServerGamePacketListenerImpl.handleContainerClick` (`aru.a(agx)`) and
`MenuType` (`crc`). The packet IDs are contents 0x13, data 0x14, slot 0x15.
MenuType registrations are anvil 8, grindstone 15, loom 18, smithing 21,
cartography 23, stonecutter 24.

The initial six groups do not cover every menu. The bounded continuations also
port lectern book/page/pulse behavior through borrowed real owners, beacon
payment and live-data rules, and brewing tick/fuel/mix/remainder/persistence
rules. Their full menus and actual block/world processing remain pending.
Enchanting, crafter, hopper, dispenser/dropper, shulker-box, merchant and entity
containers still require their own complete owner/callback/menu work. Dynamic
recipes/registries, enchantment combining, complete recipe-book placement,
advancement rewards and all unsupported player profiles remain open. Main/world
ownership assignment and GitHub write access are outstanding integration/
publication blockers, not evidence of completion.

## Continuation claim

After the initial station cores, the same instance claims new
`src/workstations/lectern.rs`, `src/workstations/beacon.rs` and
`src/workstations/brewing.rs`, plus matching
`docs/reference/workstations-{lectern,beacon,brewing}.md` ledgers. These unused
default namespace files have no conflicting recorded owner. They are separate
bounded continuations, not retroactive claims that the first six groups cover
all menus. The same GitHub comment write blocker prevents posting this renewal;
no reserved Main/block-entity files are included. Brewing world ticking, lectern
book/page/redstone callbacks and beacon live effect/structure owners must remain
explicit host contracts wherever their actual runtime is unavailable.

## Handoff and verification

Stonecutter and smithing adapters provide all seven ordinary native click modes
and prepare/validate/commit transactions over the actual borrowed inventory.
Their events retain callback, listener, indexed drop and packet order. Real
production dispatch, owners and source phases are still pending; bundle/custom
item overrides and mutating/reentrant listeners remain explicit boundaries.

The final source review checked canonical ownership, stale/identity guards,
overstack rules and exact cursor/result/drop/close ordering against the pinned
bytecode. Brewing's post-consumption remainder quirk was separately verified
by invoking unchanged Java ItemStack methods; the final engine queries the
remainder after shrink, including AIR for an exhausted ingredient. This is
method-level evidence, not a Java server gameplay observation.

The per-instance [status record](../coordination/status/ws-20261101-7c2f.md)
retains exact commits/files, focused compiler/native-flow results, remaining
contracts and publication failure. It uses the explicit status-file fallback
added on current master `46e5ca821b69a91e5555914dc82d03f57df6a507`; no master
changes were merged. Both API writes and authenticated Git pushes fail in this
session, so a local status commit is not a published branch or remote handoff.
