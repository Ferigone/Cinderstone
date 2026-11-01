# Runtime chunk composition, Minecraft Java 1.21

`src/runtime_chunk_manager.rs` composes the separately compared ticket,
entity-range, holder-lifecycle and readiness cores. This is a bounded native
integration driver. It is not a differential oracle for the entire original
ChunkMap, priority mailbox, generation engine or server loop.

The original artifact is the signed 1.21 inner server JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
`tools/runtime_chunk_manager_bytecode.txt` contains the original `aqk` and
`aqs$b` javap output. In `aqk.a(aqb)`, natural-spawn/range/player-distance
trackers run before the loading graph. The nonempty holder set is traversed
twice: all generation passes, then all future updates. A nonempty holder set
returns true immediately. Only the settled branch attaches release callbacks
to each current concrete entity future. `aqs$b.B()` checks distance work before
each runnable, then calls the light scheduler before polling the superclass.
`aqs$b.ay()` always returns true, so production main execution queues work.

The native driver keeps one main FIFO shared by ticket admission, readiness
actions and holder confirmation. Source completion effects are applied in
order before the next driver operation. The host domain is queued and
serialized: synchronous callbacks may perform their required world/index
action, but may not reenter the readiness driver or enqueue untracked main
work. The lifecycle host bridge interleaves preparation and emitted effects;
it does not batch all stage preparation before publishing completions.

Every release registration captures a concrete readiness future and receives
its own observer. Neither repeated registrations for one chunk nor multiple
registrations for the same future are deduplicated. Full-status publication is
the sole route to entity visibility. Already queued successful promotions can
still publish after demotion, as the original holder proof demonstrates.
Consequently published `TICKING` alone is insufficient to authorize block or
fluid queries. `certify_query_region` requires each **current** FULL future plus
the backend's actual source-context facts over the complete query region.

`current_chunk_to_send_ready` follows the original holder's separate predicate
(`apz.e()` and `apz.d()`, in `tools/chunk_lifecycle_bytecode.txt`): sendSync
`isDone` short-circuits a current BLOCK future `getNow(UNLOADED).orElse(null)`.
Pending/failure/unloaded returns false; exceptional/cancelled BLOCK futures
error. Exceptional/cancelled sendSync is still done. This does not use current
FULL or published visibility. The manager's retained native holder map does
not yet reproduce ChunkMap's distinct visible-map publication/unload history;
the quiescent fresh-fixture slice has an explicit retained-membership scope.
The actual ready-to-send action runs before the BLOCK future completes and
must mark pending directly, without applying this getter a second time.

Two scheduling policies are explicit. `ObservedReference` accepts a supplied
permutation for a recorded reference replay. `NativeStablePolicy` uses stable
native holder identities; it does not reproduce Java HashSet identity order.
The priority driver preserves minimum current holder queue level, native
insertion-order ties and whole-batch acknowledgement. Its
`NativeStablePriorityScope` does not claim the complete original aqe/mailbox
interleaving. `has_scheduler_work()` must be rechecked after polls: an original
main poll can return idle after its distance pass has just queued sorter work.

## Concrete source/action domain

`RuntimeWorldSources` belongs to one level and one manager identity registry.
Its constructor requires explicit newly created flat-fixture facts. Known
empty inherited source entities and no active/pending block-entity states are
required for source readiness. All base and changed block states are checked
against the original block-entity factory table; `-2` (a null-returning factory)
is rejected as well as real and unknown factories.

The fresh native fixture representation has empty postprocessing, pending
block-entity NBT, UpgradeData and saved-tick data. Postprocessing additionally
requires an observed empty global upgrade-fixer history. The first saved-tick
unpack call records actual host game time; it does not fabricate nonempty
scheduled ticks. The sender action is a required real viewer-loop pending
enqueue callback, not a direct packet send. The light scheduler is also a
required host callback. A controlled empty native lighting registry may serve
the fresh, unedited fixture slice; a static full-light packet is not evidence
of the original threaded light engine. Unknown lighting work must error.

Anvil source conversion/postload remains explicit `Pending`, including when
full NBT and a loaded block cache exist. Neither storage status nor a network
packet completes a generation future. Nonempty postprocessing/upgrades,
block entities, imported entities, saved ticks or unknown global fixers are
not silently treated as empty. A previously pending generation slot also does
not auto-complete when a missing fact becomes available: a real source driver
is still required. Pending disabled slots complete with the original
`Unloaded chunk` error and reset; completed slots preserve their identities.
Any external `complete_source` caller must first update authoritative backend
source state to the same outcome; the current fixture backend has no async
loader and does not use this path.

## Verification

`tools/runtime_chunk_manager_check.py` runs nine native composition tests and
records their exact inputs, profile and log hash. It checks current-source
hashes of the separately established ticket/range/lifecycle/readiness reports.
Those reports compare 1,428 ticket states, 1,487 range states, 2,653 holder
operations and 4,521 readiness operations with unchanged original methods.
The composition tests are additional integration checks, not new original
global-scheduler differential vectors.

Tests cover concrete-future release duplication/order, distance repasses,
stale visibility after demotion, current-FULL query authorization, priority
minimum/batch ACK, explicit holder order, exceptional required sender actions,
source-slot lifetime, unknown fact and null-factory guards, real full Anvil
cache rejection, and poisoning after failed index publication. One settled
fresh-fixture scene then executes 80 actual native item ticks and compares
position/velocity IEEE bits, ground/fall state and item counters with the
frozen unchanged-original full-item corpus. This comparison covers those
fields, not all fields of the full-item vectors. Actual pending enqueue calls
are recorded for nine block-ticking chunks. This does not by itself prove a
live client, scheduler race, world generation, chunk unloading, whole-world
lighting, complete Player.tick or complete 1:1 server parity.
