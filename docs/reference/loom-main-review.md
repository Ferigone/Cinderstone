# Loom Main integration source review

This is a read-only review of `main_loom_dispatch`, `main_loom_patterns`,
`container_workstation_loom` and `workstations::loom`. The provider/pattern
helpers are source-only; no native Loom transaction, production open/take or
checklist completion is established by their presence. No concrete critical
defect was found in the reviewed provider gates, tag-order resolver or result
callback order. The obligations below remain with the actual Main owner.

## Original callback evidence

Direct JDK21 `javap -p -c` inspected the unchanged original Java1.21 inner
server SHA256`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
and official mappings SHA1`31c77994d96f05ba25a870ada70f47f315330437`.
No REA MCP execution, new server launch or broad test pass is claimed here.
Raw disassembly and Java binaries remain outside the repository.

| Original owner | Verified behavior | Native responsibility |
| --- | --- | --- |
| `BannerPatternItem` / `csm`, constructor and `b()` | Stores and returns its actual `TagKey`; no name-derived tag is calculated. | Retain the source-certified template item/tag mapping and actual configured holder domain. |
| `LoomMenu$1` / `cra$1.setChanged` | Calls `SimpleContainer.setChanged`, then the real `LoomMenu.slotsChanged`, then its update Runnable. | Consume the kernel's input/recompute/broadcast observations in order; do not recompute again in Main. |
| `LoomMenu$2` / `cra$2.setChanged` | Calls the superclass and the update Runnable; it does not invoke input recomputation. | Preserve output notification events without impersonating an input change. |
| `LoomMenu$6` / `cra$6.onTake`, lines102–118 | Removes one banner, then one dye; resets selection if either is exhausted; invokes captured sound callback; finally calls base `Slot.onTake`. | Keep every intermediate input/result/selection/cursor/inventory phase. The template is retained. |
| Result sound callback, lines109–114 | Compares actual game time with retained `lastSoundTime`; calls sound only when unequal, then stores the time. | The initial long is0: tick0 and repeated same-time takes consume no sound seed. |

There is no Item/ItemStack crafted callback, crafted statistic, recipe-book
award, recipe-crafted criterion or XP reward in this result `onTake`.
The kernel's quick-move comment correctly preserves the base empty
`onQuickCraft` callback rather than introducing the Stonecutter callback.
Its source event order matches the original input notifications, take sound
and final output/base-slot change boundary.

The configured pattern resolver preserves actual tag vectors; it does not sort
the default or template patterns by registry ID or key. Its original item-class
and feature certificates do not confer custom component-equality, advancement,
attribute, bundle-click or callback capabilities. The future context must bind
the real actor/Level/position/menu lifetime to the same immutable configuration
owner and exact registry/tag/feature snapshot.

## Sound and real publication

The original block-position Level sound overload adds0.5 to each coordinate.
The floating-coordinate overload directly reads `dcw.f`, mapped to
`Level.threadSafeRandom`, and calls `RandomSource.nextLong` once before seeded
sound dispatch. This is separate from `Level.random` and the player's RNG.
It adds no random pitch draw. Original `avp.At` is the registered
`ui.loom.take_result`, whose agreed native constant is
`main_loom_patterns::LOOM_TAKE_SOUND_ID`1429.

The actual sound owner must be preflighted before a prepared menu commits.
Draw its seed exactly at each emitted `TakeSound` source event using the
existing narrow ordered item-publication capability. Do not draw at every
result take, during phase preflight or once per generic tick. An empty audience
still consumes the source draw. The audience uses the actual same dimension,
centered block origin, null excluded player and strict squared distance below256
(volume1, dynamic range16). Smithing's raw-integer level-event audience is not
the Loom sound audience. Existing indexed drop additions and their actual
player/Level RNG draws keep their source positions around sound publication.

## Required Main checks

Before calling any prepared action, validate actual canonical inventory,
retained window0 cursor, menu inputs/preview/external cursor and accepted
client predictions with the recursive source-identity admission helper. The
kernel's shallow FOOD guard alone does not certify nested item components or
historical synchronization baselines. Admission must prevent unsupported
values from entering canonical state; a later tick error must not become a
whole-server abort or a fabricated dead close.

Replay `PlayerInventoryChanged` on the actual canonical inventory's transient
counter at the indexed source event. Player-backed `SyncEvent::SlotListener`
must call the genuine gameplay owner with that event's `LoomPhase` inventory;
input/result listeners have no player-inventory backing. Preflight every
needed backing-slot stack-size query before infallible replay. The default
constructor Runnable is source-proven return-only; a replacement/reentrant
callback context must fail admission rather than be silently ignored.

Use each indexed `Drop` once, through the existing real item constructor,
shared ID/index/RNG owners and ordered publisher. Closing returns/drops only
the cursor and three owned inputs; the result remains a preview. Retain the
inactive window0 cursor/grid when opening an external menu: source
`ServerPlayer.openMenu` removes only a prior external menu. Use the genuine
retained full-state API for spectator/refused-prediction recovery, and retire
only the actual current external menu when a supported-profile boundary
requires its real ordered close.

Keep multi-target drag explicitly unsupported until an actual original
Slot-identity HashSet encounter owner is available; a sorted `BTreeSet` is not
that owner. Compiler/runtime verification and deployment must refer to the
eventual exported Main composition, not these helper modules alone.
