# Minecraft Java 1.21: runtime entity order audit

This is a read-only audit of the unchanged official server, not a claim that the
current Rust runtime reproduces the whole server tick. The inner server JAR was
rehashed during the audit:

`SHA-256 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`

Names and source line ranges come from the matching official mappings. Bytecode
offsets below were read with JDK 21 `javap -p -c` against this JAR.

## Global ordering contract

For an ordinary server tick, `MinecraftServer.tickChildren` visits all levels
before the connection phase. In each level:

1. `ServerLevel.tick` performs its earlier border/weather/time/scheduled phases.
2. At bytecode 292–298 it invokes `ServerChunkCache.tick(supplier, true)`.
3. `ServerChunkCache.tick`, bytecode 59–71, calls `tickChunks()` and then
   `ChunkMap.tick()`.
4. `ChunkMap.tick()` updates chunk tracking and calls eligible shared
   `ServerEntity.sendChanges()` instances.
5. The level then runs block events. Its `EntityTickList.forEach` invocation is
   later, at `ServerLevel.tick` bytecode 435–447. Entity movement, item aging and
   merging therefore occur **after** this tick's ordinary sender phase.
6. Block-entity ticking and entity-management work follow the entity phase.
7. After **all** levels finish, `MinecraftServer.tickChildren`, bytecode
   199–214, calls `ServerConnectionListener.tick()`.
8. Each game connection's listener tick calls `ServerPlayer.doTick()`. This
   invokes the full `Player.tick()` / `Player.aiStep()` path, including pickup.
9. The player-list tick, GUI tickables and chunk sending follow the connection
   phase.

`ServerPlayer.tick()` (`aqv.l`, the method called from the level entity tick) is
not the full `Player.tick()`: it does not call `super Player.tick`. The separate
`ServerPlayer.doTick()` (`aqv.m`, source 587–654) calls `cmx.l` at bytecode 15.
`ServerGamePacketListenerImpl.tick()` (`aru.d`, source 259–320) calls this
`aqv.m` at bytecode 78. This distinction determines item pickup ordering.

Consequently, an item merge in the entity phase precedes that tick's connection
pickup, while ordinary metadata/motion publication by its existing sender is
normally deferred until the next eligible sender phase. Immediate events such as
pairing, collection and removal retain their own call-site order.

## Sender iteration and gating

`ChunkMap.entityMap` is constructed as a default
`fastutil Int2ObjectOpenHashMap` (`aqb` constructor bytecode 116–124). The sender
loop uses `entityMap.values().iterator()`; this is neither entity-ID sorting nor
insertion order.

`ChunkMap.tick` (`aqb.l`, source 1136–1166):

- First it calls `updateChunkTracking` for players from its player map.
- For each tracked entity it compares the current section with the stored
  section. A change invokes `TrackedEntity.updatePlayers(allLevelPlayers)` and
  updates the stored section. Moved players are accumulated.
- `sendChanges` is called if the section changed **or** the distance manager
  says the entity's chunk is in entity-ticking range (bytecode 166–194).
- After the loop, if players changed section, every tracked entity updates its
  viewers against that moved-player list.

A viewer update can pair/unpair an entity before that entity's sender call, or in
the final moved-player pass after sender calls. Do not replace this with one
unqualified publish-every-item operation.

`ServerChunkCache.tick` invokes `ChunkMap.tick` when its `tickChunks` argument is
true, even when `TickRateManager.runsNormally()` is false. The ordinary
`ServerLevel.tick` passes true. `tickChunks()` has its own freeze/debug gates;
those do not erase the subsequent ChunkMap call. Network-loaded chunks are not a
substitute for the distance manager's entity-ticking fact.

## Connection iteration and admission

`ServerConnectionListener` (`art`) constructs `connections` as
`Collections.synchronizedList(ArrayList)` at constructor bytecode 14–21.
Its entire `tick()` iteration holds that list's monitor (bytecode 0–167):

1. Visit connections in list order.
2. If `isConnecting()` is true, leave the entry in place and skip it.
   `Connection.isConnecting()` means `channel == null` (`vt.j`).
3. Otherwise, if `isConnected()` is true, call `Connection.tick()`.
   Here connected means a non-null, open channel (`vt.i`).
4. Otherwise remove the entry through the iterator and call
   `handleDisconnection()`.

TCP `art$1.initChannel` adds the new connection directly to this same list at
bytecode 129–143; memory `art$2.initChannel` does so at 30–43. A Netty thread
attempting that add while the tick holds the monitor waits until the iteration
ends. Its appended entry can be visited on the next connection phase. An entry
added before iteration is still skipped if its channel has not become available.

The official `art` has no separate pending-connections queue and no connection
shuffle/randomize option or RNG call. This statement is specific to the pinned
official server, not server forks.

`Connection.pendingActions` is different: it is a queue of
`Consumer<Connection>`. `Connection.tick()` (`vt.b`, source 398–418) drains this
queue first, then invokes `TickablePacketListener.tick`, handles an observed
disconnection, flushes the channel and updates connection statistics. This queue
does not define a second player-admission order.

For competing ordinary players, full player ticks/pickups thus follow active
connection order, not player UUID, entity ID, player-list sorting or
`EntityTickList` order. Incoming packet tasks and Netty admission timing have
their own scheduling; this audit does not prove every asynchronous interleaving.

## Player pickup query, iteration and RNG

`Player.aiStep` (`cmx.m_`, source 575–640), bytecode 240–422, enters the nearby
entity-touch tail when `health > 0` and the player is not a spectator. There is
no additional creative/survival, inventory-capacity or radial-distance test in
this tail.

The query AABB is:

| Context | Exact query bounds |
|---|---|
| Passenger with a non-removed vehicle | union of player and vehicle AABBs, inflated by `(1, 0, 1)` |
| All other cases | player AABB inflated by `(1, 0.5, 1)` |

The mounted path is at bytecode 256–294; the ordinary path is 297–309. Supporting
only the ordinary box is an explicit mounted-player compatibility gap.

The call to `EntityGetter.getEntities(excludedPlayer, bounds)` uses
`EntitySelector.NO_SPECTATORS` (`dcn.a_` → `bsw.f`). The returned order is the
world's spatial-query order. It is not restricted to ItemEntity and must not be
re-sorted by ID.

For each result, in that order:

- An entity whose type is exactly `EXPERIENCE_ORB` is saved in a separate list.
- Every other entity is touched immediately if it is not removed at that moment.
  `Player.touch(entity)` directly calls `entity.playerTouch(player)`.
- After the complete loop, if the orb list is nonempty, exactly one orb is chosen
  with `player.random.nextInt(orbList.size())` and touched. Even a singleton list
  invokes `nextInt(1)`. Orb entries are collected before the ordinary removed
  check; their own `playerTouch` still has its own guards.

The ordinary item-only iteration performs no RNG draw itself. Whole
`Player.aiStep`, movement, inventory hooks, advancement rewards, orb pickup and
other touched entity classes are separate potential RNG users. Do not infer
that the complete player tick is RNG-free.

`ItemEntity.playerTouch` (`cjh.b_(cmx)`, source 378–396) requires, in order:

1. Server-side level.
2. `pickupDelay == 0` (not merely `<= 0`).
3. Target/owner UUID is absent or equals this player's UUID. Thrower UUID is not
   the pickup-permission check.
4. `player.inventory.add(theCurrentStack)` returns true.

It saves the original count before the inventory callback. On success:
`take(item, originalCount)` occurs first; if the mutable current stack is empty,
the item is discarded and that stack's count is restored to the saved count;
then the item-pickup statistic and `onItemPickup` callback execute. A partial
pickup mutates the same stack and does not call `setItem` or dirty Slot8.

The original `LivingEntity.take` broadcasts the collection packet before that
discard, if the source entity is not removed, the level is server-side and the
source is an ItemEntity, AbstractArrow or ExperienceOrb. Item sender metadata
and collection/removal therefore must not be coalesced into a post-tick snapshot.

## Global tracking lifecycle

For ordinary items, PersistentEntitySectionManager visibility is independent of
per-viewer visibility:

| Global section visibility | Visible lookup / ChunkMap sender | EntityTickList |
|---|---|---|
| HIDDEN | absent | absent |
| TRACKED | present | absent |
| TICKING | present | present |

Always-ticking entities have their effective status promoted to TICKING by
`dxr.getEffectiveStatus`; ordinary ItemEntity does not use this exception.

`startTracking` (`dxr.e`) adds to the visible lookup, then invokes
`LevelCallback.onTrackingStart`. `ServerLevel.EntityCallbacks.onTrackingStart`
(`aqu$a.e`) calls `ServerChunkCache.addEntity`, then `ChunkMap.addEntity`.
For an item this constructs a **new TrackedEntity and a new ServerEntity**,
inserts it into `entityMap`, and updates viewers. The new sender starts with
current position/velocity, rotation and a fresh non-default metadata snapshot;
its counters start anew. Construction does not clear pending metadata dirtiness.

`stopTracking` (`dxr.f`) calls `onTrackingEnd` first. This removes the tracked
entry from ChunkMap and removes pairings from its viewers. Only afterwards does
the visible lookup remove the entity.

Thus global HIDDEN → accessible recreates the sender/cache. A single viewer
leaving and re-entering while the entity stays globally accessible preserves the
existing sender/cache. TRACKED ↔ TICKING also preserves that sender.

Callback order depends on the transition entry point:

| Original entry point | Relevant order |
|---|---|
| `addEntity` | section add; install callback; onCreated if new; startTracking; startTicking |
| `updateChunkStatus` section callback | stopTicking if needed; stop/startTracking if needed; startTicking if needed; always-ticking entities excluded from these status changes |
| `Callback.onMove` / `updateStatus` | transfer section; stop/startTracking if needed; stop/startTicking if needed; onSectionChange if newly accessible; same effective accessible status still invokes onSectionChange |
| `Callback.onRemove` | remove from section; stopTicking; stopTracking; onDestroyed only for destroying removal reasons; remove known UUID; replace entity callback with NULL; remove empty section |

These are different original paths, not interchangeable normalized event lists.

## Evidence and verification limits

The call graph, list/map types, branch guards and lifecycle ordering above were
independently read from the pinned original bytecode during this audit. This
audit did **not** launch a complete Java/Rust server scenario spanning every
level, Netty listener and pickup callback.

Existing direct-original differential suites available at audit time:

| Report | Executed coverage relevant here |
|---|---|
| `tools/simulation_conformance.json` | 3562 queries, 26206 phase callbacks; original Level/server phase gates with injected callbacks |
| `evidence/entity-tick-conformance.json` | 131 scenes, 9201 recorded operations, 8118 visits; original EntityTickList and linked-map mutation/iteration semantics |
| `evidence/entity-spatial-conformance.json` | 97 scenes, 8830 operations, 4564 queries; original section/class-list spatial ordering and visibility |
| `evidence/item-replication-conformance.json` | 94 cases, 5208 states, 5292 packets; actual item sender, cache, merge/pickup mutation and pairing bytes with controlled inventory callbacks |
| `evidence/player-movement-conformance.json` | 3579 handler queries and 2877 bookkeeping queries; original listener handler/tick bookkeeping with controlled player effects |

The spatial suite does not by itself prove PersistentEntitySectionManager's
whole tracking lifecycle. The item-sender suite does not by itself prove that
runtime executes its sender, item simulation and player pickup in the global
order above.

Before claiming integrated parity, a combined oracle must observe at least:
sender-before-item-tick; merge-before-connection-pickup; two players competing in
connection order; admission while the connection-list monitor is held; mounted
pickup bounds; no item-only pickup RNG draw versus one orb-selection draw;
global Hidden re-entry rebuilding the sender versus viewer-only re-entry
preserving it; and exact distance-manager/ticket eligibility. These remain
separate integration checks until executed and recorded.

## Reproducible bytecode anchors

Run, for example:

```sh
javap -classpath ../mc121-oracle/server-inner-1.21.jar -p -c aqu
javap -classpath ../mc121-oracle/server-inner-1.21.jar -p -c aqs
javap -classpath ../mc121-oracle/server-inner-1.21.jar -p -c aqb
javap -classpath ../mc121-oracle/server-inner-1.21.jar -p -c art
javap -classpath ../mc121-oracle/server-inner-1.21.jar -p -c vt
javap -classpath ../mc121-oracle/server-inner-1.21.jar -p -c cmx
javap -classpath ../mc121-oracle/server-inner-1.21.jar -p -c cjh
javap -classpath ../mc121-oracle/server-inner-1.21.jar -p -c 'dxr$a'
```

Selected original class-entry hashes, independently read from the JAR:

| Class | SHA-256 |
|---|---|
| MinecraftServer | `8ce674270a27e02aae69bd6c5a3cd2fad8f2181fbb92349d882251e9db65faef` |
| aqu / ServerLevel | `ec29f2c2add8452dbd7ffe49d47577f4c4fdea6234afb8de61584508e1baae56` |
| aqs / ServerChunkCache | `22dc5072068ba8379446c8e94bffb9f6c24787e2fdfe3bebc228e653f2b8ab82` |
| aqb / ChunkMap | `1df3f6aab0cf614188a612bfaea98fd812028d2e0766860bb865bd564f91baa3` |
| art / ServerConnectionListener | `cf9284bf0ca82ef1e5e84d4c48d2de00d712010075f313efc3e22232858e542c` |
| cmx / Player | `cf857b1a2a716131f78eadec2ee5be22167e3dcf648ee42e2036edc1935edab5` |
| cjh / ItemEntity | `2d13aeaa443560da9d7969c3e515f0577a68010900b48999d476355736f8669d` |
| dxr / PersistentEntitySectionManager | `7908ee20d7c1ffd8f475378214a6f2588dfe483a9d5667ec1f037d1be05cb8fd` |
