# Chest and barrel removal reference

This ledger records static behavior of the shipped Minecraft Java **1.21**
server. It is not a runtime result or a claim of complete container support.

## Pinned source

The observations use the official mappings and JDK 21 `javap -p -c` against the
inner server JAR with SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The outer server JAR SHA-1 is `450698d1863ab5180c25d7c804ef0fe6369dd1ba` and
the matching mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Their pinned URLs are recorded in [`source-lock.json`](../../fixtures/source-lock.json).
Raw disassembly and Java artifacts are not redistributed.

| Mapped class | Shipped name | Relevant methods |
| --- | --- | --- |
| `ChestBlock` | `dhb` | `onRemove`, `updateShape`, `tick` |
| `BarrelBlock` | `dfe` | `onRemove`, `tick` |
| `AbstractChestBlock` | `des` | inherited base removal |
| `BaseEntityBlock` | `dfk` | inherited base removal |
| `BlockBehaviour` | `dtb` | `onRemove` |
| `Containers` | `bqn` | `dropContentsOnDestroy`, `dropContents`, `dropItemStack` |
| `ChestBlockEntity` | `dqo` | `getContainerSize`, opener lifecycle |
| `BarrelBlockEntity` | `dqa` | `getContainerSize`, `recheckOpen`, opener lifecycle |
| `RandomizableContainerBlockEntity` | `drn` | `getItem` |
| `ContainerOpenersCounter` | `dqu` | `recheckOpeners`, `scheduleRecheck` |

## Contents and comparator notification precede block-entity removal

Both `ChestBlock.onRemove` (mapped lines 185–187) and `BarrelBlock.onRemove`
(63–65) first invoke `Containers.dropContentsOnDestroy(oldState, newState,
level, pos)`, then delegate to their superclass removal method.

The shared helper (mapped lines 56–64):

1. Returns immediately when old and new states have the same block identity.
2. Reads the actual old block entity at the position.
3. If that object implements `Container`, calls `dropContents` on that object.
4. Calls `level.updateNeighbourForOutputSignal(pos, oldState.getBlock())`.

Only after this helper returns does the inherited `BlockBehaviour.onRemove`
remove the block entity, when the old state has a block entity and the block
identities differ. `BaseEntityBlock` and `AbstractChestBlock` do not replace
that removal behavior. Missing or non-container block entities do not produce
invented contents or a notification from this helper.

The observable order for an actual container is consequently **contents,
comparator notification, block-entity removal**. This differs from the
[furnace callback](furnace-removal.md), whose base removal precedes its
comparator notification. A general shared implementation must retain the
correct callback order for each block family. The level/chunk transition has
already changed its palette before calling the old block's removal callback;
the old block entity remains available during these operations.

This helper awards no recipe experience. Creative destruction skips the later
block-loot path, not the container-content callback. The applicable creative
item guards, `playerWillDestroy` effects and source replacement-fluid rules are
described in the [furnace removal ledger](furnace-removal.md); they are shared
game-mode/world operations rather than furnace-only behavior.

## Slot and random-source ownership

The actual chest and barrel block entities each report **27 slots**.
`Containers.dropContents` visits indices 0 through 26 in order and passes each
actual `Container.getItem(index)` stack to `dropItemStack`. It does not use a
merged double-chest menu inventory. Each removed half has its own block entity;
double-chest partner and shape updates are additional lifecycle work.

`RandomizableContainerBlockEntity.getItem` invokes `unpackLootTable(null)` before
delegating to ordinary slot access. An unresolved loot table therefore cannot
be treated as an empty or already materialized inventory by a native snapshot.

The item construction is the same `Containers` path already documented for
[furnaces](furnace-removal.md#container-stack-splitting-and-random-ownership)
and represented by `container_drops.rs`:

- Three actual level-random position doubles are consumed for every slot,
  including an empty one; an empty 27-slot container consumes 81 such draws.
- One position belongs to each slot. Each nonempty stack is split by
  `nextInt(21) + 10`, preserving components.
- Two constructor level-random velocity draws precede the actual entity's
  independent random-source factory. They remain consumed even though the
  final velocity is overwritten.
- The three triangular velocity samples use deviation
  `0.11485000171139836`, with means `0.0`, `0.2`, `0.0`.
- `addFreshEntity` occurs before the next split/slot. Container drops keep the
  item constructor's pickup delay zero; they are not player-thrown items.

Staging must retain the actual level RNG, entity RNG factories, entity IDs and
world-add ownership. Publishing an emptied live container as a shortcut would
misrepresent the callback's lifetime and expose an intermediate state.

## Openers and queued rechecks

Neither chest nor barrel `onRemove` calls `stopOpen`, closes viewing menus,
resets the opener counter or cancels scheduled opener checks. An open menu's
captured container lifetime and later validity/close phase require their own
implementation; closing every viewer synchronously is not this callback.

`ContainerOpenersCounter.scheduleRecheck` schedules the actual block after five
ticks. `BarrelBlock.tick` reads the current block entity and calls
`BarrelBlockEntity.recheckOpen` only when it is still a barrel block entity.
`recheckOpen` itself avoids a removed block entity. The chest has the analogous
current-block-entity recheck path. These scheduled operations are separate from
`onRemove`; removal does not justify fabricating a completed check or
arbitrarily deleting a pending scheduler entry.

## Bounded native integration requirements

The initial supported removal scope is an owned, closed **single dry chest** or
closed barrel in the existing creative-player profile. Before touching RNG or
publishing effects, the native host must verify the actual container identity,
all 27 canonical stacks, viewer/menu ownership, the original state and the
replacement. Open viewers, double-chest callbacks, unresolved loot tables and
waterlogged chest replacement remain explicit unsupported boundaries until
their real owners are connected.

Comparator notification still requires the source horizontal scan and real
world-query ownership. A supported empty-receiver path can establish that no
actual comparator callback exists; connected receivers must not be silently
ignored. The general comparator scan order and conductor behavior are recorded
in the [furnace ledger](furnace-removal.md#comparator-notification-boundary).

These facts do not validate native execution, networking, conservation across
restart, public gameplay, survival drops, full opener lifetime or performance.
Those observations must name their own exact source, binary and runtime scope.
