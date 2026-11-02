# Native SmithingMenu transaction owner

`src/main_smithing.rs` prepares the central Main adapter for the workstation
team's source-backed smithing kernel. This module is initially unexported until
the integration coordinator assembles its actual callsites. Callable source is
not evidence that a player can open it in production, and does not complete an
aggregate inventory/crafting checklist delivery.

## Canonical owners and API

`SmithingRuntime.menus` retains one `OpenSmithing { position, menu }` per active
player under the central `LivePlayers.smithing_tables` owner. The menu owns only
three temporary inputs and its external cursor; the result is a derived preview.
The same canonical player inventory is borrowed from `SharedContainer` for each
prepared operation. Its window-0 cursor remains separate. No block entity,
second player inventory or preview-owned item is constructed.

The runtime exports `window_id`, `has_open`, `pending_players` and
`inventory_snapshot`; the last is an observational/persistence clone, not a live
inventory replacement. `Server::smithing_action_locked(world, players, id,
request, now, border)` accepts `SmithingRequest::{Click,Close,Broadcast}` under
the already-held central operation/world/player locks. Smithing has no menu
button action, so no artificial button path is supplied.

Opening/access and wire dispatch belong to `main_smithing_dispatch`; real player
phase, close/disconnect/save belong to `main_smithing_lifecycle`; recipe/feature,
item/statistic/book callbacks belong to `main_smithing_recipes`; immediate pickup
sync belongs to `main_smithing_pickup`. The coordinator owns their Main assembly.

The actor check uses actual synchronized health, lifecycle dimension and the
existing admitted creative native-body contract, with its real unmounted body.
Spectators use the source's read-only menu/resync path. Actual applicable
equipped block-interaction-range modifiers are rejected until the real modifier
composition owner is available. Close remains possible to release owned stacks
without substituting a new actor/profile or abandoning them after a change.

## Source-specific smithing phases

Smithing binds both recipe orders from the actual `PinnedDefaultRecipeContext`:
the constructor input/shift-routing order and the separate result-selection
order. Its feature snapshot is the same configured snapshot used by Main.
Disabled or unknown result/registry/recipe/component contexts fail explicitly
before committing inventory. No invented recipe order, trim holder or enchantment
operation is supplied.

Every `SmithingMenuEvent` retains its `SmithingPhase`: all three temporary
inputs, current derived result, canonical player inventory observation and
separate external cursor. Source input changes broadcast the already-changed
inputs with the previous result before recomputing that result. The kernel
already converts `BeforeCreateResult` into those ordered Sync events; the host
must not perform a second recomputation or defer all callbacks to final state.
The callback bridge rejects an escaped `BeforeCreateResult` explicitly.

Result takes retain the source order: actual ItemStack crafted statistic,
recipe-crafted with the three pre-consumption inputs, recipe-book award, input
decrements in order, then level event 1044/data 0. Result quick-move invokes the
crafted callback with its emptied source stack and amount zero; it does not add
the stonecutter's separate preliminary Item callback. Callback snapshots are
observations, not additional owned stacks.

`PlayerInventoryChanged` calls `set_changed()` on the actual canonical
`PlayerInventory` through `SharedContainer.transact_with_menu_commit`, preserving
signed wrapping of its real transient counter. Player-backed slot listeners call
the actual `MainItemGameplay.inventory_slot_changed` with their own
`phase.player_inventory`, including intermediate input/output transfers. Every
needed backing-slot encoding/stack-size query is checked before infallible replay.
Top-input/result listeners do not impersonate player inventory. Actual recipe
book/statistic checkpoints are installed at their source event positions before
their ordered packets; unsupported registered criteria/reentrant callbacks remain
outside the certified native profile.

## Prepared commit and owned drops

The action plan checks actual menu instance/revision and canonical inventory
before staging, then revalidates after actual callback/drop preflight before
consumption. Canonical menu/inventory state remains inside the shared transaction
until successful commit. All drop indices are checked exactly once. Source stack
encoding, actual chunk/rain facts, actual gameplay/book owners and statistic
changes are checked before publishing actual items.

Nonempty owned drops are moved once into the existing
`spawn_player_drop_with_factory` path in event order. They use the actual player
RNG, Level RNG, shared entity-ID allocator and actual random factory. The
existing `RuntimeEntities.prepare_items` token retains the exclusive item-store
borrow. `commit_with_world_random_ordered` publishes real item additions between
the matching Sync/effect/drop events through the existing shared output FIFO.
Drop statistics advance after world-add. No second item outbox or connection
writer is introduced.

Server close publishes CloseScreen first. Its cursor is returned/dropped before
input 0, input 1 and input 2, preserving raw insertion/overflow interleaving.
The derived result never enters that owned return/drop sequence. The actual
caller supplies AliveConnected or DeadOrDisconnected, and synchronization
baselines transfer back to retained window 0.

## Level-event source proof

Direct JDK 21 `javap -p -c` inspected the pinned original 1.21 artifact and
official mappings from `fixtures/source-lock.json`; no REA MCP execution is
claimed. `SmithingMenu` (`crt`) dispatches `levelEvent(1044, blockPos, 0)` through
its captured `ContainerLevelAccess`. `LevelAccessor.levelEvent` (`dcx.c`) passes
a null excluded player. `ServerLevel.levelEvent` (993–994, `aqu.a`) calls actual
`PlayerList.broadcast` from the integer block X/Y/Z with radius 64, dimension
identity and `ClientboundLevelEventPacket(..., false)`.

The native event uses the existing protocol-767 level-event encoder (packet
`0x28`) and an actual same-dimension audience strictly within radius 64 of that
integer position, including the actor. This is not the stonecutter's centered
16-block sound audience. Smithing's server level event consumes no sound seed or
additional RNG draw: client event playback is separate from server sound-packet
generation.

Source packet-handler guards retain matching-window dispatch, spectator full
resynchronization before slot/validity handling, ignored invalid nonspectator
access/slot indices, and later player-phase broadcast-before-validity/close.
Main callsites, compiler/runtime results and publication remain separately owned
by the integration coordinator. No production or deployment proof is claimed by
this unexported source stage.
