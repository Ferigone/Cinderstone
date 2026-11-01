# Stonecutter Main composition contract

The workstation kernel owns temporary stonecutter input, selected recipe and
derived result. Main owns the current menu, canonical player inventory,
world/player locks, callbacks, RNG and publication. This contract records the
required composition points; the existence of the kernel alone does not prove
that production invokes them.

## Canonical owners

`main_stonecutter::StonecutterRuntime` stores each actual open menu and captured
block position. Its `inventory_snapshot` overlays the actual menu's carried
stack on the shared player inventory. It never inserts station input or the
derived result preview into player inventory. Recipe/stat/criterion callbacks,
owned drops and menu synchronization retain their ordered event positions.

The inspected `main_player_data.rs` contains native-record identity validation
and recipe-book serialization helpers. It contains no active-menu inventory
snapshot chain, so it requires no invented stonecutter hook. The real chain is
`Server::capture_player_locked` in `main.rs`.

## Required composition points

| Main owner/callsite | Required stonecutter behavior |
| --- | --- |
| `main.rs`: module declarations and `LivePlayers` construction | Install the four Main adapters and one canonical `StonecutterRuntime`; do not create another player inventory |
| `main.rs`: `capture_player_locked` | Compose `stonecutters.inventory_snapshot` with the existing external-menu snapshot chain; capture the actual player's statistic owner separately |
| PLAY block-use dispatch | Route actual stonecutter use through `use_native_stonecutter`, preserving hand/sneak, spectator, reach and feature gates |
| PLAY ContainerButtonClick (`0x0d`) | Decode the original two VarInts, route to the current menu and preserve the ignored wrong-window/spectator/invalid-access branches |
| PLAY ContainerClick (`0x0e`) | Dispatch the authoritative current stonecutter action before InventoryMenu fallback, retaining real state-ID/prediction synchronization |
| PLAY ContainerClose (`0x0f`) | Close the actual current server menu; the received window byte cannot select an older menu |
| `Server::tick_item_world` player-menu callback | Invoke the per-player stonecutter menu phase at that player's actual position in the shared entity tick order |
| `main_player_body.rs`: `PlayerBodyPhase::Pickup` | Supply immediate current-menu synchronization to `RuntimeInventoryHost` for a stonecutter, rather than delaying it until a later global tick |
| `main_rain_context.rs`: fallback and `external_menu` ownership | Include stonecutters in actual external-menu ownership; reject a fallback that cannot run the required player-menu phase |
| Existing container/furnace/crafting-table openers | Remove an existing stonecutter through its captured owner before allocating the next window; use the existing shared window counter |
| `main_block_entities.rs`: connected InventoryMenu cleanup guard | Recognize a still-active stonecutter; a normal InventoryMenu cleanup must not consume the current external menu's cursor |
| `Server::cleanup_player_admitted` | Preserve player-removal order: retained InventoryMenu removal first, then external stonecutter with `DeadOrDisconnected`, then persistence and actor removal |
| `Server::save` shutdown branch | Retire stonecutters alongside other external menus before final inventory capture; ordinary capture/autosave must not unexpectedly close a GUI |
| Reconfiguration cleanup | Reuse the real player-removal path; do not retain the old menu across actor replacement |

Immediate pickup synchronization must use the authoritative menu's callback and
packet owner. An independent packet-only shadow menu cannot replace it. Choose
one per-player tick path: calling both a whole-phase stonecutter loop and the
actual player callback would duplicate broadcasts, checks and callback work.

Equipment continues to use canonical player slots. A displayed stonecutter
result never becomes equipped, carried or persisted until a successful owned
take puts its output in an actual player slot or cursor.

## Source phase constraints

Direct `javap -p -c` inspection of the pinned original Java 1.21 server and
official mappings established these constraints; this is not an REA MCP or
graphical-client observation. Artifact identities are recorded in the linked
source ledgers.

- `ServerPlayerGameMode.useItemOn` (`aqw.a`) invokes the default-block-use
  criterion after the consuming ordinary main-hand block callback. The
  spectator MenuProvider path returns earlier.
- `ServerPlayer.openMenu` (`aqv.a(MenuProvider)`) closes a previous external
  menu, advances the shared counter, creates the new menu, sends OpenScreen,
  initializes listeners/synchronization and then installs the active menu.
  Opening does not remove the retained InventoryMenu cursor or 2x2 inputs.
- Container click/button handlers (`aru.a(agx)` / `aru.a(agw)`) reset actual
  last-action time before their window/spectator/validity gates. Invalid access
  returns without closing inline. A spectator click synchronizes the current
  state; a spectator button does nothing.
- `ServerPlayer.tick` (`aqv.l`) broadcasts the current menu before its validity
  check. A server closure sends CloseContainer before removed callbacks.
  `doCloseContainer` runs removed, transfers remote inventory state and then
  switches the current owner back to InventoryMenu.
- `StonecutterMenu.removed` (`crw.a(Player)`) delegates cursor removal first,
  discards the preview and returns/drops only the one owned input. It has no
  inventory-owning block entity to save or recreate.
- The captured validity query requires a stonecutter at the actual position and
  strict eye-position distance to its unit AABB using the real interaction
  range attribute plus the source's verification buffer of 4.

See [opening](stonecutter-opening.md),
[criterion ownership](stonecutter-criteria-owner.md) and the
[kernel/source ledger](workstations-stonecutter.md) for bounded APIs and
unsupported custom contexts. The integration coordinator records the final Main
source commit, compiler result and actual runtime flow separately. This contract
does not claim a completed aggregate checklist delivery.
