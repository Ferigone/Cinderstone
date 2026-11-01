# PlayerChunkSender original 1.21 audit

`arq` is invoked unchanged from the pinned signed original inner JAR. ASM
creates oracle-owned `oracle.*` host subclasses only. Original `dvi`, `eot`,
`adg`, `adf`, and `adk` objects retain their original methods. Ready chunks use
controlled original fields (empty section array, heightmaps and block-entity
maps); the original light engine has a zero-height context and no light layers.
Successful nonempty send paths therefore exercise original packet constructors
and final quota subtraction. This does not prove normal-world chunk payloads.

The fixture compares state after every operation, exact ready-query order,
selected actual chunk positions, and Begin/Chunk/End/Unload calls. Throwing at
Begin, a chunk send, or End independently proves the completed-prefix state:
all selected actual positions are removed before Begin; the unacknowledged
counter increments before Begin; quota is subtracted only after successful End.
A native additional test covers a fallible host readiness query.

The first outstanding-batch limit is one; any ACK changes it to ten. ACK always
decrements the signed counter, even unsolicited, with wrapping Java arithmetic.
NaN requested rate becomes .01; other rates clamp to .01..64. Quota becomes one
only when the new outstanding count is zero. Quota addition and limits use f32.
Public early gates do not ask for player position or ready chunks.

`ChunkBatchHost.player_chunk_position` is the lazy host acquisition boundary.
The original obtains ServerLevel, its chunk source/map, then player chunk
position, only after outstanding/quota/pending gates. A host may capture the
live world/map when constructed; it must report acquisition errors through this
callback before any readiness query. `send_chunk` includes the actual packet
construction/send and original debug hook. Every host method is required.
The original debug method `agf.sendChunkPacket` itself is return-only.

The memory-connection / pending<=floor(quota) branch queries readiness for every
pending key before stable sorting by **actual returned chunk** distance. The
limited branch first selects Guava's least(k) by **requested key** distance and
only then asks readiness. Nearest not-ready chunks can block farther ready
chunks. Both remove actual returned positions, by direct fastutil remove, only
after the entire ready list is constructed. Aliases and duplicate ready handles
are retained. Distances use wrapping signed i32 subtraction/multiplication.

fastutil's stream/spliterator scans zero first, then table slots ascending;
its separate Java iterator scans descending and is not used by `arq` streams.
Guava quickselect can reorder equal-distance candidates before its final stable
buffer sort. See the independent shipped-dependency corpus and report for both.

The actual original serverbound protocol-template visitor registers
`minecraft:chunk_batch_received` as ID **0x08**. Node's independent protocol-767
serializer emits `0841800000` for a rate of16. This is checked reproducibly.

Run `python3 tools/player_chunk_sender_check.py --release` for whole-crate Cargo
profiles. The first release attempt was blocked by rust-lld undefined hidden
symbols in unrelated rustls/ring thin-LTO dependencies, recorded separately in
`evidence/player-chunk-sender-cargo-release-link-failure.txt`. As an explicit isolated alternative,
`--optimized-standalone` compiles this unchanged sender module and all four tests
with rustc opt-level3 against its small anyhow/serde_json dependencies. The
report names this `optimized_standalone`; it does not substitute for Cargo release.
The final sender Cargo release was subsequently verified in the isolated
`../mc121-phase5-proof-target` directory; the earlier stale-target linker failure
is retained as historical evidence. This is the library sender test filter,
not an all-target project suite. Native providers,
normal-world chunk/light byte equality, and main integration require separate
checks; the helper does not claim complete server compatibility.
