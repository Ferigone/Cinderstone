# Native GrindstoneMenu lifetime

Target: Java Minecraft1.21, protocol767. The native lifecycle delegates all
mutating menu/removal ownership to `main_grindstone` and its real mixed Item/XP
transaction. Main assembly and production runtime evidence remain separate.

## Original source

The original inner server SHA256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings SHA1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Direct JDK21 javap inspection, not a REA MCP call, establishes:

| Source method | Contract |
| --- | --- |
| ServerPlayer.tick (`aqv.l`), lines530–535 | Broadcast actual current menu before stillValid and server close. |
| GrindstoneMenu.stillValid (`cqt.b`), line225 | Captured actual level/position, canonical grindstone block, then actual player reach with additional range4.0. |
| GrindstoneMenu.removed (`cqt.a`), lines219–221 | AbstractContainerMenu.removed handles cursor first, then clears its real two-input container through captured level access. |
| AbstractContainerMenu.removed (`cpu.a`), lines558–569 | Actual alive connected inventory return versus dead/disconnected drop. |
| ServerPlayer.closeContainer/doCloseContainer | Server close packet precedes removed; synchronization then transfers to retained InventoryMenu. |
| Container-close packet handler | Incoming window ID does not select an older server menu. |

The result preview is derived and is never returned/dropped as another input.
Close alone awards no XP. Result-take integer XP and level event1042 belong to
the primary transaction; see `grindstone-transactions.md`. There is no block
entity or invented inventory at the captured grindstone position.

## Actual runtime and methods

`players.grindstones` retains one actual menu, captured position, binding and
configured enchantment/tag snapshot. `tick_native_grindstone_menu_for_player_locked`
calls the actual primary Broadcast first. That kernel transaction validates
the retained owner/registry under the same held nonreentrant operation. The
subsequent validity observation reads the fresh actual block/player access,
short-circuiting block identity before reach, then closes if invalid. It does
not construct a fake Host merely to supply a validity bool or read a stale
pickup access sample.

Only typed UnsupportedMenuReach is handled as an explicit bounded native
profile retirement. The actual menu is closed through its real ordered owner,
contents conserved and limitation logged. Other failures propagate. This does
not invent custom attribute results or establish arbitrary-range support.

`connected_grindstone_close_context(players,id)` checks actual metadata health,
matching physical entity/UUID and removed state. Connection-driven callers
already own their actual connected operation; registry presence alone is not
used to infer that an EOF connection remains connected. Explicit actor removal
uses DeadOrDisconnected after retained InventoryMenu removal.

`close_native_grindstone` owns the standalone chat/clock/world/player operation.
`close_grindstone_with_context_locked` never reacquires those guards and closes
only the matching actual current window through `grindstone_action_locked`.
Atomic Main current-menu selection ignores the packet's supplied old window;
client close does not request an extra server close packet.

## Saves, shutdown and cross-menu assembly

Ordinary autosave retains the actual menu and two owned inputs, using the
runtime's detached canonical inventory plus its actual separate cursor for
player snapshots. It must not serialize derived preview as owned inventory.

`close_grindstones_for_save_locked` is a no-op outside shutdown. During the
explicit connected native checkpoint policy it preserves real registry
encounter order, rejects orphan owners and preflights every actor's actual
AliveConnected branch before publishing any close. It does not perform an
early dead-player drop ahead of retained InventoryMenu cleanup. Actual EOF/dead
removal remains a separate InventoryMenu-first context-aware operation. This
checkpoint policy is not vanilla periodic-save forced closure.

The six existing native container/crafting/furnace/stonecutter/smithing/Loom
openers now close an actual Grindstone or Anvil before allocating/opening the
new menu, after their existing target admission. Loom's owner-exclusivity check
also includes both new owners. The retained InventoryMenu's Alive cleanup guard
includes both; its explicit DeadOrDisconnected-first exception is unchanged.

Root owns central runtime/mods, atomic close/resync, per-player tick, active
Body/legacy pickup selection, save/snapshot and orphan/removal wiring. These
source changes have passed whitespace/diff hygiene only; no Main runtime flow
or whole workstation/checklist completion is asserted by this handoff.
