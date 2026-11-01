# Production StonecutterMenu transaction owner

`src/main_stonecutter.rs` binds the workstation team's prepared stonecutter
kernel to the existing player menu, gameplay, recipe book and ItemEntity owners.
This is a bounded native flat-world integration stage, not completion of every
workstation, an imported-world capability or a fixed-checklist increment.

## Actual owners and entry points

`LivePlayers.stonecutters` holds one `OpenStonecutter { position, menu }` per
active player. The position is the actual captured block in the same level; the
menu's canonical input and external cursor belong to that retained menu. The
36 player slots are borrowed from the player's existing `SharedContainer` for
every transaction. Window 0 retains its own cursor and 2×2 inputs.

`StonecutterRuntime::{window_id,has_open,pending_players,inventory_snapshot}`
serve the central dispatch, lifetime and persistence owners. The inventory
snapshot is only a clone of the canonical inventory with the retained external
cursor for observation/save; it creates no second live inventory.

`Server::stonecutter_action_locked` accepts `StonecutterRequest::{Click,Button,
Close,Broadcast}` while the central level/player locks and operation sequencing
are held. Opening/wire dispatch belongs to `main_stonecutter_dispatch`; player
phase and close/save lifetime belong to `main_stonecutter_lifecycle`. Source
recipe/feature/statistic/book callbacks belong to `main_stonecutter_recipes`.

The action owner checks the current source image, actual certified native
gameplay/criteria floor, real recipe book, player and current feature snapshot.
Mutable actions reuse the existing native creative-overworld body contract,
actual health and unmounted physical owner. Spectators retain the source's
read-only open/resynchronization path. Close remains available for actual owner
cleanup rather than abandoning stacks after a profile change.
Unsupported recipe, component, callback, imported source or missing owner errors
remain explicit. It never installs a fake block entity or a no-op callback for
an unknown context. Stonecutters do not own an inventory-bearing block entity.

## Transaction and publication order

1. Match the actual active menu/window and apply the source handler guards.
   Prepare the kernel plan against the canonical player inventory and real
   game time. Validate its menu identity/revision and expected inventory.
2. Stage source callbacks against the real certified gameplay/book owners and
   validate every owned drop stack. Revalidate the plan before consuming it.
   Its new menu/inventory remain inside the shared transaction until commit.
3. Validate event/drop indices, source inventory-listener mappings, callback
   statistic changes, actual sound RNG presence and required chunk/rain facts.
   Every nonempty owned drop has exactly one event and one actual staged item.
4. Construct drops in their event order through the existing
   `spawn_player_drop_with_factory`, actual player RNG, Level RNG, global entity
   ID counter and actual random factory. Consume each owned drop stack once;
   result previews and callback snapshots never become additional item stacks.
5. Prepare the real `RuntimeEntities` batch. On successful shared commit,
   install the active-menu state, staged player RNG and reserved entity IDs.
   For a requested server close, publish CloseScreen before return/drop events.
6. Use `PreparedItemBatch::commit_with_world_random_ordered` to publish each
   actual item through its existing item store/outbox at the matching indexed
   Drop event. Between insertions, replay menu packets, real inventory listener
   callbacks, recipe-book checkpoints/packets and sound at their source indices.
   Drop statistics advance after actual world-add. Crafted statistics preserve
   zero-count/AIR callbacks. No separate connection writer or item outbox exists.

The callback replay uses prevalidated statistic/book proposals while retaining
the actual exclusive owners. Every event retains the workstation team's
`StonecutterPhase`: actual input, derived result, selection, canonical inventory
observation and the separate external cursor. Inventory callbacks call
`MainItemGameplay.inventory_slot_changed` with that event's inventory phase,
rather than substituting the transaction's final inventory. All backing slots
needed by each source inventory scan are checked for canonical encoding and
supported stack-size lookup before an infallible commit. Top input/result slots
do not masquerade as player inventory.

Mandatory `PlayerInventoryChanged` events call `set_changed()` on the actual
canonical `PlayerInventory`, retaining Java signed-int wrapping of its transient
`timesChanged` counter. `SharedContainer.transact_with_menu_commit` lends that
actual menu under its existing lock; no menu-local substitute counter or
reentrant container read is used. Mandatory `InputChanged` events explicitly
complete the certified original constructor-default input update callback. The
kernel already computed `SimpleContainer`/`slotsChanged` state at that boundary;
the original default server update listener returns without additional state.
Unknown/custom/reentrant listeners are not replaced with that default.
Registered criteria, scoreboard or reentrant item contexts outside the existing
certified profile remain rejected. The precise Item versus ItemStack crafted
callback distinction is documented in the recipe/callback source ledger.

The assembled Main target passed `cargo check --locked --bin mc121-rust` after
the phased API, actual inventory counter and ordered item-publication seams
were connected. This is compiler evidence only. The integration coordinator
owns the focused real-client flow, exact published commits and deployment
verification; no additional comprehensive suite is claimed here.

## Sound and RNG

The source stonecutter result effect has fixed pitch/volume and source game-time
sound suppression. The enclosing `Level.playSound` nevertheless draws one sound
seed from the actual independent `Level.threadSafeRandom` owner. The ordered
prepared-item capability exposes only `next_sound_seed`; it cannot mutate the
validated item store. The draw occurs at the Sound event even if no player is
within audible range. It does not consume Level.random or the player's stream.

The actual same-level audience includes the actor and players strictly within
16 blocks of the captured block center. Packet publication uses the existing
ordered `PlayerEvent::WorldAudience` stream. Recipe packets and slot/cursor
packets use their actor audience within that same stream.

## Source handler and lifetime boundaries

The pinned original Java 1.21 server and official mappings remain identified by
`fixtures/source-lock.json`. Fresh direct JDK 21 `javap -p -c` inspected
`ServerGamePacketListenerImpl.handleContainerClick` (1693–1737, `aru.a(agx)`),
`handleContainerButtonClick` (1757–1772, `aru.a(agw)`) and
`AbstractContainerMenu.isValidSlotIndex` (111, `cpu.a(int)`). This is static
source evidence; no REA MCP execution or Java gameplay flow is claimed here.

- Matching spectator clicks send a full state resynchronization before checking
  block validity; they do not execute the requested click. Spectator buttons
  return without mutation.
- Nonspectator clicks/buttons with invalid captured block/reach are ignored by
  the packet handler. They do not close the menu immediately. Its later actual
  player phase first broadcasts, then checks validity and closes when required.
- Slot indices at or above 38 are ignored. The original validity predicate
  permits negative indices below its size; the prepared kernel retains their
  source click handling, including outside-window `-999`.
- `Broadcast` deliberately bypasses the action validity gate. `Close` retains
  the actual caller's AliveConnected or DeadOrDisconnected context and transfers
  shared player synchronization baselines back to window 0.

The central dispatch also owns the actual last-action timestamp and resets it
before the source packet handler's window/permission gates. Real Main callsites,
compiler/runtime results and publication commits must be recorded by the
integration coordinator after assembly; this source file alone is not a live
menu/protocol-client or deployment observation.
