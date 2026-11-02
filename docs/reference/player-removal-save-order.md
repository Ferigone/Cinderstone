# Player removal: source save before menu cleanup

This stage changes the native disconnect/reconfiguration save slice to follow
the original Java 1.21 save-before-removal order. It does not implement the
complete inherited Player/LivingEntity removal lifecycle or imported callbacks.
The normal native autosave/resume view remains a separate existing policy.

## Pinned source evidence

Evidence is direct JDK21 `javap -p -c` of original classes, not REA MCP or a
new Java gameplay execution. Original server SHA-1:
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; inner SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mapping SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`.
Raw disassembly and binaries remain private.

| Original call | Relevant source order |
| --- | --- |
| `ServerGamePacketListenerImpl.switchToConfig`, source1578–1582, `aru.n` | Sets waitingForSwitchToConfig first, calls removal, then sends StartConfiguration and changes outbound protocol |
| `removePlayerFromWorld`, source1245–1253, `aru.q` | Calls ServerPlayer.disconnect before PlayerList.remove |
| `ServerPlayer.disconnect`, source1374–1379, `aqv.v` | Sets hasDisconnected, ejects passengers and exits sleeping state when needed |
| `PlayerList.remove`, source381–406, `aur.c` | Awards LEAVE_GAME at bytecode5–9; calls save at12–14, before world removal and Player.remove |
| `PlayerList.save`, source364–373, `aur.b` | Saves PlayerDataStorage, actual statistics and advancements owners |
| `Player.remove`, source1450–1456, `cmx.a(RemovalReason)` | After its inherited removal call, invokes InventoryMenu.removed, then actual doCloseContainer for the disconnected player |
| `ServerPlayer.doCloseContainer`, source1219–1223, `aqv.t` | Removes the actual current menu, transfers menu state, then restores InventoryMenu as current |
| `Inventory.save`, source404–425, `cmw.a(ListTag)` | Serializes actual items, armor and offhand; menu carried/grid/result are not Inventory fields |
| `ServerPlayer.awardStat`, source1334–1336 | Updates the actual statistics counter, then dispatches matching scoreboard objectives |

The existing native profile is unmounted with no active passenger/sleep owner;
this stage does not invent ejection or sleep callbacks for broader contexts.
It keeps the existing actual native body/session retirement host rather than
claiming a new implementation of every inherited removal callback.

## Explicit persistent inventory view

Moving the old `save_player_locked` call upward would be wrong. Its native
resume capture can retain InventoryMenu grid/result state and overlays the
current external cursor. Saving those temporaries and later dropping them
would create two restart owners for the same items.

`PlayerInventory::snapshot_for_player_save` is a detached serialization view
of the actual persistent slots5..45 and actual selected hotbar. It excludes
carried and slots0..4. It preserves synchronization/change counters in the view
and does not mutate live slots, validate a creative action, execute a menu,
publish packets, allocate an entity or draw randomness.

Main uses `PlayerInventorySaveView::SourcePlayerInventory` only for the actual
pre-removal save. This branch does not overlay any external cursor or temporary
inputs. PlayerData captures the actual player, lifecycle and statistics with
that view. The existing `NativeResume` branch and its overlays remain unchanged
for actors that are not undergoing removal; that policy is not labeled vanilla
Player.save semantics.

## Actual statistic and retry ownership

`MainItemGameplay::award_player_stat` requires the existing real native
empty-criterion/objective floor, verifies actual active actor ID/UUID and
updates the canonical PlayerStats counter using its existing saturation and
dirty-state operation. Its empty-scoreboard observation records the verified
actual owner. Native save provenance by itself does not waive callback checks.

When that callback floor is unknown/imported or otherwise unsupported, cleanup
records `native_player_removal_stat_unsupported` with award_completed=false.
It preserves the earlier transport-only cleanup rather than trapping an actor
solely on the absent inherited callback capability. It does not pretend that
the missing leave-game award, objectives or advancements were executed.
Unexpected identity, counter, capture, persistence and removal errors propagate.

The retained `PlayerRemovalProgress` is bound to actual actor ID, UUID and the
available real runtime generation. It records the actual leave-game outcome,
one immutable pre-removal save proposal and completed removal phases. The
existing cleanup retry loop therefore cannot award leave_game again after
capture/disk failure, recapture already dropped temporaries, or rerun completed
menu callbacks after retirement has begun. This is serialization/finalization
progress, not another live inventory or XP owner.

Pending player-save proposals use that same source snapshot. Autosave of a
removing actor reuses the already captured proposal; if the source proposal
has not completed, it errors rather than silently switching to a post-removal
view. The marker is cleared only with actual actor retirement, never by time.
No new persistent marker is written to a player save.

## Current native sequence and limits

The reconfiguration state transition now occurs before cleanup. Its returned
StartConfiguration packet is still sent only after source save and actual
removal complete, with one transition and one send. Disconnect reason signaling
and the actual leaving-actor marker precede save/menu work.

Cleanup then awards the supported source leave-game statistic once, captures
and persists the source inventory-only proposal, removes retained InventoryMenu
temporaries first, removes the actual external menu next and retires the
existing native body/session. Item constructor/drop IDs and RNG remain in their
real close callbacks; the save-view helper consumes none.

This is source implementation in the future worktree. Compiler/runtime checks,
current-master composition and publication are reported separately by the
central integrator. It does not establish all imported/advancement/scoreboard,
passenger, sleeping, superclass removal or full Minecraft compatibility.
