# Configured ore host and material contract

The reference is the unchanged Java 1.21 server, protocol 767, inner SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The outer artifact and official mappings are pinned by `fixtures/source-lock.json`.
Inspection used direct Java 21 `javap`, not REA. No raw disassembly or proprietary
Java binaries are included in the repository.

`feature/host.rs` defines mandatory callbacks for actual world, chunk and section
owners. It supplies no production region, generated chunk, scheduler or fallback
terrain. The configured-leaf caller must establish the same world/dimension and
actual registry/tag snapshot, validate its configuration, and perform the
original `ConfiguredFeature.place` origin permission query before any leaf RNG.
Fresh `WorldGenRegion.ensureCanWrite` checks the center's permitted X/Z radius;
only upgrading centers add the source generation-height window. Ordinary
out-of-build-height origins must not acquire a new early rejection.

| Original method | Native contract |
| --- | --- |
| `OreFeature.canPlaceOre` (`ecz.a`, mapped lines 179–185) | Test the current ordered target against the leaf's captured state, then decide whether to skip exposure checking, then lazily query adjacent states. A failed target does not draw the exposure RNG or reread the voxel. |
| `OreFeature.shouldSkipAirCheck` (`ecz.a`, lines 189–195) | Discard chance at or below 0 skips air checks without a draw; at or above 1 always checks without a draw; otherwise consume exactly one `nextFloat`, skipping when it is at least the chance. |
| `Feature.isAdjacentToAir` and `checkNeighbors` (`ece.a`, lines 186–197) | Query DOWN, UP, NORTH, SOUTH, WEST, EAST in `Direction.values()` order and stop at the first state whose actual `isAir` is true. Native canonical air recognition includes air, cave air and void air; invalid state IDs fail. Coordinates preserve Java integer wrapping. |
| `BulkSectionAccess.getSection` (`duw.a`, lines 26–40) | Calculate the actual accessor's section index first. Only an index outside the section array returns no section. Otherwise resolve the real chunk and acquire/cache its actual section. Missing chunks or sections are errors. |
| `BulkSectionAccess.getBlockState` (`duw.b`, lines 44–52) | An actual absent section from that bounds test reads ordinary AIR before chunk lookup. An acquired section supplies its direct live state. This is distinct from a normal world read. |
| `BulkSectionAccess.close` (`duw.close`, lines 57–60) | Release every acquired section. The cache and acquired identities belong to this invocation, including after neighbor exposure queries switch the last-section cache. |
| `OreFeature.doPlace` (`ecz.a`, lines 61–175) | Construct bulk access after all sphere draws/culling; acquire sections lazily during traversal; write through the selected section with `setBlockState(..., false)`; close on normal return and exceptional exits. |
| `ScatteredOreFeature.place` (`edh.a`, lines 26–45) | Read the actual world state once per attempt, try targets in order with the normal world callback for air checks, then call region `setBlock(..., 2)` and ignore its boolean. It never constructs bulk access. |

`OreHost` exposes actual build minimum/exclusive maximum and the source
outside-build-height predicate. Bulk-section bounds remain separate: a custom
unaligned accessor can have section padding outside its build Y interval.
`bulk_section_block` returns `None` only for the source section-index bounds
case; `bulk_block` maps precisely that result to canonical ordinary AIR. An
in-range missing chunk or owner must never become `None`. The normal `block`
path resolves its actual chunk first, including at out-of-height positions;
an actual ProtoChunk then returns VOID_AIR outside its build bounds.

The regular vein path uses `ocean_floor_wg` with WorldGenRegion's first-available
Y convention (`ChunkAccess.getHeight` plus 1). Its direct section writes preserve
the section palette and nonempty/fluid counters but deliberately bypass
ChunkAccess heightmaps, lighting, block entity handling, ticks, on-place/remove
callbacks and automatic postprocessing marks. These writes must not be routed
through the scattered path's setter. `begin_bulk_section_access` and
`close_bulk_section_access` are mandatory; implementations retain actual cached
section identities and release them exactly once, including on errors.

The scattered setter preserves `WorldGenRegion.setBlock` (mapped lines 267–304):
permission check, actual chunk write, old/new POI notification when applicable,
block entity handling, then state-requested postprocessing. ProtoChunk writes
update the heightmaps appropriate to their real persisted status. Pending
ProtoChunk block entities use the source `DUMMY` NBT marker and must materialize
actual block entities when requested; that marker is not a substitute for the
owner. Deferred postprocessing preserves its actual section grouping and order.
WorldGenRegion itself ignores the flags/recursion arguments after entry, so
flags 2 do not imply runtime packet or neighbor notifications. Delegating to a
later-status ProtoChunk or LevelChunk still requires that owner's real lighting
and block callbacks. The returned bool records region acceptance, including
source-accepted no-op writes, rather than proof of a changed voxel.

The shared material helper accepts the actual caller RNG and a block-read
callback, so regular and scattered leaves share exposure logic without changing
their read ownership. The closed configured catalog uses only deterministic
BlockMatchTest/TagMatchTest rules; those rules draw no RNG. Other rule types,
registry overrides and missing source owners remain unsupported rather than
being converted to a default match or state.

Errors can occur after RNG draws, acquired sections or writes. Close the bulk
owner and consume or poison the surrounding job; restoring an image alone does
not establish rollback of its other owners. This contract and callable leaves
do not establish FEATURES completion, neighborhood scheduling, persisted chunk
statuses, Main invocation, lighting, spawning or FULL chunks. Focused material
checks cover lazy exposure order and RNG consumption; coordinated original
runtime comparisons are recorded separately, not inferred from these helpers.
