# Remaining controllers and registry-context review

Target: original Java1.21/protocol767. PR76 source is
`edfaf836437ca9e15eef52c3140b021e74cdccfe`. This continues merged PR73,
`f39092468468701c6a5c11ad1c912921ad361e9f`, with ordinary source commits:

| Commit | Native behavior |
| --- | --- |
| `ca6eafe`, `5987d95` | Beacon/lectern layouts; dynamic horse layout with no registered MenuType; actual player-slot synchronization ranges |
| `e0d9f46` | Lectern borrowed book/page controller, source removal/pulse callbacks and cursor-only close |
| `b3d88b3` | Actual-owner anvil enchantment session, cost/compatibility/level/component semantics and snapshot guards |
| `0c60215` | Beacon live captured data, real effect setters, payment/cursor ordering, staged lock denial and close |
| `a605c7a` | Horse separate saddle/storage/BODY owners, dedicated opening, native transfer/close and capacity guards |
| `ed5d07f` | Exact requested transactional Grindstone retained fullresync and explicit publication phase |
| `0fa0f16` | Actual-registry repair/trim route and ordinary/default enchanted-book repair component correction |
| `ebc2c6c` | Exact central-requested same-owner Grindstone host staging capability |
| `7690150` | Mandatory detached Anvil host transaction and retained fullresync |
| `860b7cc` | Crafter nonmodifiable-slot capacity and matching-removal rules for admitted native imports |
| `edfaf83` | Retained Anvil inventory synchronization with exclusive validation and infallible adoption |

Evidence is the pinned original server JAR plus official mappings inspected
directly with JDK21 javap. The per-family ledgers retain methods, hashes, API
signatures and boundaries. No executed Java menu or modified-book observation
is claimed where inspection and source-derived native fixtures were used.

Independent cross-reviews checked every new controller, anvil combination and
special context against source. Horse review found the mandatory Slot.tryRemove
whole-removal/capacity guard for an occupied saddle. It was corrected before
commit, tested with actual max-stack-size component99/saddle2/cursor98 for
PICKUP and PICKUP_ALL, and independently rechecked. No concrete blocker
remained in the inspected paths. Team review is not independent human GitHub
approval or complete compatibility proof.

The Main proposal found a further Anvil integration gap: its previous
caller-staged API installed the UI before an external fallible publication.
Mandatory all-owner begin/publish methods now precede final UI installation,
without public Clone/reset. Direct failure tests after XP, inventory, RNG,
world, drop and outbox work preserve both actual fixture and UI. Independent
review checked that correction and the broadcast-only pickup capability.
Earlier green0fa verification is historical, not final approval. The Crafter
regression imports an occupied-disabled state through public native Serde;
normal vanilla restore/setters prevent that state, so no gameplay witness is
claimed for it.

Local composition `d65fa79d70461ed2d2223dc147987b02a2e0edad` combines actual
master `005e7592cd13d3d8edfa9fe0f875404bfa038b91` with that exact source.
Tree `3e7ec1030e9bb2ac056d14008badc8aec84dac5e`, source tree
`d38be01f2b607f7959969008b2c3c2df4be3abac`: repository hygiene,
`cargo check --locked --all-targets`, `cargo test --locked --lib` and
`cargo test --locked --bin mc121-rust` passed. Focused native controller/context
flows passed before freeze. Actual GitHub CI run41358613014 also succeeded on
source head edfaf83. Generated merge refs can be stale relative to concurrent
master merges, so they do not substitute for the independently tested
latest-master composition. Read actual final merge tree before claiming
that source was deployed. Later documentation-only heads have their own CI.

All external callback domains use actual borrowed or privately staged owners.
Atomic publication finishes fallible preparation before native UI adoption.
The caller then uses the existing ordered PlayerEvent FIFO under the actual
owner commit lock; socket draining is asynchronous. Returned observations are never replayed as
second packets/drops/callbacks. No fake mount, registry or block entity and no
second preview/inventory owner is supplied.

Limits remain explicit: real Main callsites/state owners, actual enchanted
holder/map order, unresolved component references, bundle/custom overrides,
reentry, uncertified drag ordering, lectern cross-owner book aliases and
take-time recipe/remainder alias ownership. The new repair/trim route requires
its real captured registry context without fallback; the legacy default
evaluator is documented separately. Central granted the five new Main-Anvil
files in issue3/6564589182; they use a separate branch. Existing Main/runtime/
cross-opener/body/save/decoder wiring remains central. An external new-source
composition with the modules, agreed field and two proposed pure gameplay
methods compiled and passed focused native flows. It did not edit reserved
repository files or prove production callers. Actual shared pure methods and
configured enchantment provider remain pending. Independent chunk readiness
and constructor entropy acquisition are not claimed rollback-capable by that
Main proposal; canonical gameplay state, player/Level streams and actual item
publication remain staged.

No whole inventory.4/crafting.2/crafting.5, fixed scope/count change or new
production menu invocation is claimed. Existing chest/barrel/furnace and
2x2/3x3 foundations are preserved. Subsequent Main integration and real owner
contracts are recorded in issue3 and the team status file.
