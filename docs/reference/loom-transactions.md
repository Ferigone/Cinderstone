# Native LoomMenu transaction owner

Target: original Minecraft Java 1.21, protocol 767. Original outer server SHA-1:
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA-1:
`31c77994d96f05ba25a870ada70f47f315330437`; extracted inner JAR SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Source evidence is direct JDK21 `javap` of the pinned `cra`, `cra$6`, `dkq` and
`cpu` classes, together with the actual generic adapter in Team3 tree
`b4bccba:src/container_workstation_loom.rs`. No REA MCP run is claimed.
Raw disassembly and original binaries remain private.

## Source boundaries and real owners

| Boundary | Native owner/behavior |
| --- | --- |
| Level/player/configuration identity | `NativeLoomBinding` holds the actual stable `Server.world` lock address, entity ID and UUID, captured dimension/block position and exact `LoomPatternBinding`. No local counter replaces a level or registry owner. |
| Actual access/time | `NativeLoomContext` captures the real block and player pose/position/gamemode while locked, plus the actual `now` supplied from Main's world clock. Actions refresh those observations; sync-only pickup retains the real old context without querying reach/time. |
| Default listener environment | Private native factory certifies the original constructor-default `cra.r()` sole-return Runnable and native nonreentrant listeners. Original Java `registerUpdateListener(Runnable)` exists; arbitrary replacement listeners are unsupported. The source floor is rechecked before mutable transactions. |
| Current menu and canonical inventory | `LivePlayers.looms: LoomRuntime` is the required central field. `OpenLoom` retains exactly one real bound `LoomMenu`, position and context. `SharedContainer::transact_with_menu_commit` stages the real inventory and commits under its existing mutex. |
| Prepared actions | Actual `prepare_click`, `prepare_button` and `prepare_close` validate the unchanged menu/inventory/binding. Every source phase/callback and owned drop is admitted before the plan's detached commit is consumed. |
| Local listeners | Actual player-backed menu slots 4–39 map to canonical slots 9–44. Each real `inventory_slot_changed` sees its event's historical inventory, never a substituted final snapshot. Original data listener index zero observes the selected pattern and has no extra server callback body. |
| Source container callbacks | Kernel source phases already own input/result mutation, recomputation and nested broadcasts. The bridge explicitly certifies the known boundaries and constructor-default Runnable; it does not rerun recomputation or add a generic fallback listener. |
| Inventory dirty counter | Every mandatory `PlayerInventoryChanged` executes the real canonical `Inventory.set_changed()` at that exact event position. |
| Menu packets | The retained menu owns its synchronization baselines. Packets publish through the existing ordered `PlayerEvent` stream, interleaved with actual listeners and effects. `ClickOutcome.packets` must be empty to prevent a second packet owner. |
| Removed items | Only indexed `outcome.drops` confer ownership. Prepared actual ItemEntities are committed through `PreparedItemBatch` at their source positions. Input/result/cursor phase snapshots are observations, and the result preview is never dropped as an extra owned item. |
| Drop randomness/statistics | Actual per-player RNG and actual Level.random are staged using the existing player-drop factory. Real entity IDs are reserved under the existing player lock. Drop statistics advance after each actual item-world addition. |
| Take sound | Actual emitted TakeSound draws one seed from the independent Level.threadSafeRandom at its source event, even with no recipients. The packet uses the original registered sound, BLOCKS, centered block position and volume/pitch one. See the independent source ledger below. |

The loom has **no item-crafted, recipe-book, recipe-criterion or XP callback** in
the inspected result-take path. This owner creates none. Actual block opening
still awards `interact_with_loom` and invokes the post-use default-block
criterion in its certified empty-criteria profile; spectator opening skips
those ordinary block-use callbacks.

## Callable APIs

`main_loom` supplies:

- `LoomRuntime` with actual current-window lookup, player enumeration and an
  active-cursor inventory snapshot for persistence. This view does not move
  any stack or overwrite the retained InventoryMenu cursor/grid.
- `OpenLoom { position, menu, context }`, with the retained context available to
  the real immediate pickup synchronization owner.
- `open_loom_menu_locked(world, players, id, position, window, now, border)`.
  The provider must first complete real preflight, close its previous external
  owner and allocate the actual shared window. Construction precedes
  OpenScreen; initialization/listeners precede installing current ownership.
- `loom_action_locked(world, players, id, request, now, border)` with
  `LoomRequest::Click`, `Button`, `Close`, `Broadcast` and `Resync`.

Wrong click/close windows are ignored. The packet wrappers reset actual last
action time before matching. A matching spectator click uses retained genuine
full-state synchronization before a validity gate; spectator buttons are
ignored. Mutable invalid access is ignored by the packet action and left for
the real player phase's broadcast -> fresh stillValid -> ordered close.
Broadcast does not prematurely test block validity. Close is exempt from the
equipped-reach mutation gate so the exact typed unsupported-profile retirement
can return/drop the actual cursor and inputs safely.

Full-state repair calls the real retained `broadcast_full_state`; it does not
initialize another menu, fabricate a click or publish window-zero content.
The source snapshot wrapping those sync events comes from the same actual menu
and canonical inventory. The close plan preserves cursor-first then input
return/drop ordering and excludes the derived preview.

## Sound and ordering evidence

[loom-sound-source.md](loom-sound-source.md) and
`fixtures/loom-production-sound.json` independently verify the original sound
object, raw registry ID 1429, variable range, category and default listener.
The single Rust constant belongs to `main_loom_patterns` and packet encoding to
`main_loom_callbacks`; this owner supplies only the actual seed and audience.

The kernel compares actual signed game time with retained lastSoundTime using
inequality, with initial value zero. A suppressed equal-time take emits no
sound event and draws no seed; a different time invokes the actual sound path
even when no player is in range. Audience selection uses the captured level's
actual dimension and strict squared distance below 256 from XYZ + 0.5.
Independent sound draws occur between indexed item additions through the
existing prepared store's restricted publication capability, rather than
through a second RNG or item outbox.

## Supported boundary and remaining integration

This is a bounded native default creative/readonly spectator and fresh native
fixture/empty-criteria profile. Imported-world contexts, custom/reentrant
listeners, altered registry ID ownership/features, unsupported component
identity and applicable equipped custom block-range modifiers are not claimed
as vanilla-compatible. The existing typed reach boundary retires the real menu
through its normal ordered removal; it does not invent a modified attribute.

Quick-craft with zero/one target has one possible encounter order. Several
targets require the real Java Slot-identity HashSet owner; sorted native slot
indices are explicitly unsupported instead of silently changing that order.
Actual configured tag vectors remain in packet encounter order, and missing
tags remain empty. No made-up pattern registry, flat fallback, fake block entity,
recipe order, preview owner or replacement listener is provided.

At this source handoff the working tree contains the actual generic kernel and
the central integrators' Main state/dispatch, player-phase lifetime, same-frame
pickup and persistence/removal source assembly. The provider/runtime assignment
did not edit those existing files. Root must finish reverse cross-menu
replacement and compile/exercise the combined bounded path. No compiler result,
live connection, successful deployment or checklist completion is asserted here.
This assignment ran no Cargo suite, changed no Git state and changed no progress
metrics.
