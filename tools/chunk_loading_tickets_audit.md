# Minecraft Java 1.21 loading-ticket contract

The unchanged official inner JAR is pinned at SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The official outer JAR SHA1 is `450698d1863ab5180c25d7c804ef0fe6369dd1ba`.
Names below come from its official mappings; the oracle records per-class hashes.

## Separate graphs and states

| Original class | Published state | Meaning |
|---|---|---|
| `ara`, TickingTracker | Default 33; stored levels below 33 | DistanceManager entity/block ticking predicates at <=31 / <=32 |
| `aqk$c`, PlayerTicketTracker | Default 34; stored distances <=32 | Distance from active player source chunks; eligibility for deferred loading PLAYER tickets |
| `aqk$a`, ChunkTicketTracker | Default 45; scheduled ticket levels <=44 | Loading/generation scheduling; not confirmed accessibility |
| `apz`, ChunkHolder | FULL/BLOCK_TICKING/ENTITY_TICKING futures and confirmation | Lifecycle's actual confirmed full status |

A loading level <=33 can request FULL, while FULL visibility still depends on
successful future promotion and its executor callback. An entity can therefore
leave `ara`'s entity-ticking range while remaining globally tracked in a confirmed
FULL chunk. The set of packets requested by an individual network chunk view is
a separate concern.

## Original source rules

* `DistanceManager.addPlayer(SectionPos, ServerPlayer)` updates the shared player
  set, updates the natural-spawn and PlayerTicketTracker source graphs at level
  zero, and adds an immediate **separate ara PLAYER ticket** at
  `max(0, 31 - simulationDistance)` with Java int wrapping.
* Player membership is shared by chunk. Final removal invalidates the distance
  sources and removes that ara ticket. Duplicate add still invokes the source
  updates. Spectator admission policy is upstream in ChunkMap.
* `PlayerTicketTracker.haveTicketFor(distance)` is `distance <= viewDistance`.
  `ChunkMap.setServerViewDistance(int)` clamps to 2..32 and passes that exact
  value; no extra radius is added at this call site.
* Into-view transitions submit a **primary loading PLAYER ticket at level 31**.
  They do not add that loading ticket to ara. The separate immediate ara source
  already represents simulation distance.
* Raw DistanceManager add/removeTicket affects only the primary map/loading
  graph. Region wrappers use `33 - regionDistance`, wrapping as an int, and
  transfer the same logical ticket to ara. FORCED uses level 31 in both graphs.
* START, PLAYER and FORCED have timeout zero. Duplicate primary tickets refresh
  their creation clock. Other types with nonzero timeout are outside the native
  three-kind module's scope.

`aqk$c.updateViewDistance(int)` scans the original fastutil player-distance map
in its actual iteration order, compares old/new eligibility, then stores the new
view distance. A graph drain gathers changed chunks in a fastutil LongOpenHashSet;
that set's iteration order controls the subsequent sorter messages. The native
module preserves those layouts and exact graph FIFO queues.

## Deferred sorter and main work

The actual DistanceManager constructor creates `ChunkTaskPriorityQueueSorter`
with acquisition limit **four**. Its actual `ProcessorMailbox` uses this priority
order, with FIFO within each priority:

| Priority | Operation |
|---|---|
| 0 | Queue-level changes / initial processor setup |
| 1 | Release or cancellation |
| 2 | Submit chunk task |
| 3 | Poll next chunk task |

Each task-priority bucket is a linked map from chunk to a list of optional tasks.
Insertion at an existing chunk appends to the list. A task and an empty acquire
marker are submitted together. A poll refuses work when four chunks are already
acquired; otherwise it removes the first chunk in the lowest occupied priority.
The marker adds that chunk to the acquired set. Completion of the processor
runnable, rather than chunk readiness, allows the next poll to be posted.
Chunk readiness releases the acquired chunk later.

Admission has **two queued main-executor steps**:

1. The processor dispatch runs the submitted runnable. That runnable posts a
   separate admission commit to the main executor. The processor acknowledgement
   already permits the next sorter poll.
2. The admission commit re-reads the current *published* player distance and
   current view-distance threshold. If still eligible, it adds the primary
   PLAYER31 ticket and records the chunk in `ticketsToRelease`. If no longer
   eligible, it releases the acquisition without adding a ticket.

This is not merely an assumption of the test executor. The production
`ServerChunkCache.MainThreadExecutor.scheduleExecutables`, `aqs$b.ay()`, is
`iconst_1; ireturn`, so execute calls always queue. `bpk.run()` processes exactly
one mailbox runnable per executor dispatch; its runAll method is separate.

A leave-view transition submits release(clearQueue=true). It cancels all queued
messages for that chunk, releases acquisition, and posts a main-executor removal
commit. A normal readiness release(clearQueue=false) removes acquire markers
while preserving task lists and their linked-map positions.

## Holder futures and release registration

Actual `DistanceManager.runAllUpdates(ChunkMap)` runs natural-spawn graph, ara,
PlayerTicketTracker, then the loading graph. If changed ChunkHolders exist, it
schedules their generation/future updates and returns early. A later settled
pass iterates `ticketsToRelease`, checks whether a primary PLAYER ticket still
exists, and attaches `thenAccept` to the current **entity-ticking** ChunkHolder
future. It clears `ticketsToRelease` after that pass.

The callback ignores the ChunkResult's success value. A future completing
normally with success, failure or UNLOADED therefore posts the main release
callback. Exceptional or cancelled futures do not execute `thenAccept`.
The main callback in turn posts the sorter release(clearQueue=false).

The original oracle calls the real registration path on controlled pending
CompletableFutures after the host boundary explicitly clears already-applied
`chunksToUpdateFutures`. It proves normal versus exceptional/cancelled callbacks;
it does not execute ChunkMap generation or manufacture a FULL confirmation.

Production main scheduling has another required boundary:
`aqs$b.pollTask/B()` first invokes `runDistanceManagerUpdates`. If that reports
work, the poll returns without running the next queued main Runnable. Otherwise
it schedules lighting work and runs one main Runnable. A main host reproducing
this policy must service distance/future work **between** main tasks. Draining
all admission commits before updating the loading graph is a distinct schedule.
The offline oracle deliberately exposes executor ordering as input, not as a
claim about a complete integrated production tick.

## Native API and ownership

`src/chunk_loading_tickets.rs` owns membership, primary sources, both loading
and player-distance graphs, exact sorter mailbox/task queues, main task queue,
and acquisition bookkeeping. The frozen `EntityTickingRange` stays separate.

* `add_player`, `remove_player`, `update_simulation_distance`, region/forced
  wrappers emit ordered `TickingTransfer` values. `take_ticking_transfers()`
  drains them; `TickingTransfer::apply` applies them to EntityTickingRange.
* `set_view_distance` receives DistanceManager's threshold; the caller performs
  ChunkMap's upstream clamp when applying server configuration.
* `run_player_distance_updates()` drains the player graph and posts sorter work.
* `run_loading_updates(budget)` returns the original remaining budget.
  `take_scheduling_changes()` yields ordered `{chunk, old_level, level}` values
  for the host's actual ChunkHolder scheduling/lifecycle provider.
* `run_sorter_tasks(budget)` and `run_main_tasks(budget)` advance explicit queued
  executor dispatches. Budget one preserves production polling boundaries.
* `take_release_registrations()` is called on the next settled pass after the
  host has applied changed-holder futures. The host attaches each returned
  chunk's current entity-ticking future.
* `normal_entity_future_completion(chunk)` is the callback for normal completion
  only. It posts the main release task; it never confirms accessibility.

`take_events()` drains the instrumentation/effect log; the host must drain it
alongside transfers and scheduling changes to avoid accumulating log entries.

`loading_level` and `player_distance` are observations, not visibility APIs.
Lifecycle callbacks alone should update global entity visibility. There is no
network-view heuristic or implicit whole-world readiness in this module.

## Objective verification scope

The Java oracle uses the **real DistanceManager constructor** and unchanged
`aqk$a`, `aqk$b`, `aqk$c`, `aqe`, `aqd`, `bpk`, `ara`, and actual source methods.
ASM only implements abstract world scheduling/get-holder boundaries. The
controlled scheduling callback records each original request and stores its
new ticket level in an actual ChunkHolder; it supplies no generation result.
Two controlled executors expose every task dispatch.

The fixture preserves events, both graph states and all their computed/FIFO
queues, primary tickets and clocks, sorter task lists/acquisition/sleep state,
main tasks, and original ara propagation. Repeated immutable JSON values are
interned without changing their values. The native tests resolve those pool
references before comparison. ara ticket creation timestamps are not compared
by the native tracker because none of its three supported ticket kinds expire;
its full propagated/pending graph is compared instead.

Remaining separate work includes ChunkMap generation/readiness, actual
ChunkHolder confirmations, natural-spawn behavior, nonzero-timeout ticket kinds,
network chunk sending, and the complete integrated host scheduler. Passing these
fixtures does not establish full Minecraft parity.
