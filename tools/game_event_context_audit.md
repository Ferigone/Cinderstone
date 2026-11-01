# Empty game-event context, original 1.21

`src/game_event_context.rs` implements only the certified empty-listener path.
It does not replace a nonempty registry's dispatcher or vibration system.

## Host contract

Implement the required `GameEventRegistryWorld` methods over actual loaded
world state. `get_loaded_chunk(x,z)` corresponds to original
`ServerChunkCache.getChunkNow`, without loading/promoting a chunk. An absent
chunk is skipped; its registry state is not queried.

`registry_state(chunk,sectionY)` must perform the corresponding actual registry
lookup/creation. Original server `LevelChunk.getListenerRegistry` creates and
retains a Euclidean registry even for section Y outside the build-height range.
The registry map is not the physical block-section array and cannot be clamped
to that array's valid indices.

Return `RegistryState::Noop` only for the actual immutable NOOP registry case.
For Euclidean registries report all four current facts: active listener count,
pending-add count, pending-remove count, and processing flag. The supported
certificate requires all three collections empty and processing false.
`isEmpty()` alone checks only active listeners and is insufficient.

Any active/deferred/processing registry or `Unknown` returns an explicit error
at that section. The helper does not manufacture a silent listener result.
Earlier empty lookups may already have materialized their empty registries;
the operation is not an atomic world transaction.

The provider must know all actual listener sources and registration paths.
An observed empty block-entity list does not establish the absence of dynamic
entity listeners. Unread entity storage, unsupported source entities or missing
registration coverage must produce `Unknown`. No default provider is supplied.

`ItemGameEvent::{EntityDamage,HitGround,Splash}.at(position)` supplies the actual
vanilla item-event notification radius. Their registry IDs are 14, 26 and 40;
all have radius 16. These identifiers and radii were extracted from the actual
original GAME_EVENT registry, rather than inferred from packet names.

## Original execution

The oracle executes unchanged `dya.post`, original `dvi.getListenerRegistry`,
original `dxy.visitInRangeListeners`, and actual `dyc.NOOP`. ASM defines only
controlled classes under `oracle.*` for the world/chunk-source getters and a
chunk wrapper that logs then invokes the original registry lookup.

The corpus has 840 posts, including actual HIT_GROUND/SPLASH/ENTITY_DAMAGE
holders, custom GameEvent notification radii, absent/loaded chunks, negative
coordinates, section boundaries, unclamped Y, Java floor saturation and i32
wrap. It captures 22,900 world getter calls, 5,711 chunk queries, 11,478 section
visits and 5,900 materialized original Euclidean registries. Every final
registry is quiescent empty and each actual level RNG seed remains unchanged.

Traversal order is X, Z, then Y. The original empty path never reads
GameEvent.Context or listener/entity getters. Its visited flag remains false.
The conditional debug-event call is therefore absent; separately, the original
`DebugPackets.sendGameEvent` method itself has return-only bytecode. This last
fact is static evidence, not an instrumented debug-network observation.

Native early-return for an empty X/Z rectangle preserves all observed getters.
An empty Y interval still performs the X/Z loaded-chunk lookups. The edge corpus
tests this distinction, including wrapped intervals.

## Listener-source catalogue

The separate actual-factory extraction covers all 1,060 registered blocks,
26,684 states and 44 BE types. All 166 EntityBlock instances use the original
default `EntityBlock.getListener`, which delegates to a BE implementing
`GameEventListener.Provider`, otherwise returns null.

Only four registered block families return listeners: sculk sensor (radius 8),
calibrated sensor (16), sculk catalyst (8, BY_DISTANCE), and sculk shrieker (8).
Their block-position sources resolve to the BE center. The catalogue records
actual factory variants and state IDs, not just class-name matches.

ITEM and ServerPlayer inherit the return-only
`Entity.updateDynamicGameEventListener` method. Actual invocation registers
zero listeners for those two types. Other entities are not certified by this
fact. The catalogue alone is not a live loaded-registry certificate.

Reproduce the native/original comparison with
`python3 tools/game_event_context_check.py --release`. Re-extract factories with
`python3 tools/game_event_context_factories.py`. Reports preserve JAR/class and
source hashes. Main integration, source-world registry certification and all
nonempty dispatch remain separate work.
