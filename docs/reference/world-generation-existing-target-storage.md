# Existing native generation target storage

`worldgen::target` provides a mandatory live `GenerationTargetHost` and a
section-backed `NativeProtoTarget`. The caller moves its existing boxed
sections, canonical state and biome palettes, packed WG maps, per-section
postprocessing lists, persisted status, retrogen slot and unrelated owner data
into the target. Generation does not clone these allocations into a replacement
air image. This module supplies storage operations, not a runtime chunk, a
scheduler status transition, persistence or Main integration.

The public namespace remains `worldgen::target`. Its physical source directory
is `src/worldgen/chunk_target/`, selected by `#[path]` in `worldgen/mod.rs`, so
the Docker build-output exclusion `**/target/**` cannot discard these sources.
The earlier paired runtime fixtures retain their historical source paths and
native source commit `a28bfc2`; see [packaging evidence](world-generation-target-packaging.md).

The reference is the pinned unchanged Minecraft Java **1.21**, protocol **767**
server: inner SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
with official mappings pinned in `fixtures/source-lock.json`. Inspection used
direct Java 21 `javap`; no REA provider or REA Evidence was used. No proprietary
class files or raw disassembly are included.

| Original mapped boundary | Native storage responsibility |
| --- | --- |
| `ChunkAccess` (`duy`), constructor 83–115 and section access 161–165 | Full logical build bounds and physical section-array coverage remain distinct. Every supplied section retains its allocation. |
| `NoiseBasedChunkGenerator.fillFromNoise/doFill` (`dzb`), 291–413 | Acquire actual affected sections before writes. Direct section writes use `false`; WG acquisition is OCEAN_FLOOR_WG then WORLD_SURFACE_WG. The composition owner performs these operations in source order. |
| `LevelChunkSection.setBlockState` (`dvj`), 64–94 | Update the existing canonical palette and signed-short counters, including equal-state calls. Do not implicitly update heightmaps, light, block entities, ticks or posts. |
| `LevelChunkSection.recalcBlockCounts`, `BlockCounter.accept` (`dvj`, `dvj$a`), 138–145 and 121–135 | Palette recount counts non-air blocks and nonempty fluids separately in the nonempty counter, and counts only randomly ticking fluids in its fluid counter. Importing already maintained counters does not recount or normalize them. Incremental writes instead adjust the fluid counter for every nonempty fluid, exactly as the original method does. |
| `ChunkAccess.getOrCreateHeightmapUnprimed` (`duy`), 177 | Preserve an existing packed WG map. If absent, allocate zero packed storage; first-available values start at the full build minimum. Do not scan existing blocks here. |
| `Heightmap` (`dyy`), constructor 84–89, prime 92–124, update 127–154 | Retain SimpleBitStorage word layout and unused bits. Source updates raise a matching top or rescan the live full column downward when its top is removed. Lazy getHeight primes only an absent map; an existing unprimed map is not repaired. |
| `ProtoChunk.setBlockState` (`dvs`), 124–177 | Logical build-Y guard returns VOID_AIR outside the range. An empty section plus canonical AIR returns early. Otherwise perform its section write, prime missing status-required maps after the write, then update them in EnumSet order. This is separate from direct NOISE writes. |
| `ProtoChunk.markPosForPostprocessing` (`dvs`), 269–277 | Append packed local X/Y/Z to the actual full-build section list. Preserve prior entries, duplicates, empty-list versus absent-list state and insertion order. Outside logical build Y is ignored. |
| `ChunkAccess.getHighestGeneratedStatus/getNoiseBiome` and `ProtoChunk.getNoiseBiome` (`duy`, `dvs`), 276–282, 418–427, 244–247 | The real retrogen target contributes to highest-generated status. Stored biome reads require BIOMES. Their quart clamp uses `(minY >> 2) + (height >> 2) - 1`, including unaligned accessors, and reads the existing physical section palette. |
| `ProtoChunk.getHeightAccessorForGeneration` (`dvs`), 340–343; `BelowZeroRetrogen$1` (`dym$1`), 47–52 | A present actual retrogen slot selects minimum −64 and height 64. It does not change full build bounds, section coverage, NoiseChunk allocation or aquifer allocation. An unavailable slot is an error. |
| `ProtoChunk.setPersistedStatus` (`dvs`), 235–240 | Only an explicit actual status event changes persisted status, clears retrogen when the new status reaches its target, and marks the target unsaved. Material fill does not call this setter implicitly. |

`NoiseSectionLease` owns the lock cells of the actual acquired native sections.
Acquisition follows descending absolute section Y. A second overlapping lease
fails; direct writes require a matching live allocation token. Drop releases
all acquired sections on normal return or error. Logical Proto writes acquire
and release their own section access. These guards provide real exclusive
ownership; they do not claim the original ThreadingDetector's diagnostic text
or thread dump behavior. A failed generation session remains the composition
owner's poisoned session, not a rollback of already applied target mutations.

The native backend supports the packaged built-in registry and actual persisted
statuses EMPTY through SURFACE. The original switches to four final heightmaps
at CARVERS, before FEATURES. Those maps and initialized-light callbacks are not
implemented here; importing or assigning that later native status fails
explicitly. An external implementation of the mandatory host trait may supply
those actual owners. Unrelated caller-owned maps, block entities, ticks,
structures and metadata remain in the same `unrelated` owner without invented
callbacks. Stored built-in biome identities are validated; a generic live host
must separately report actual Holder registry-key ownership.

State facts come from
`fixtures/world-generation-target-state-facts.json`, an actual original-registry
export. It records `isAir`, both WG predicates, nonempty fluid and block/fluid
random-ticking predicates for each canonical state ID. Its artifact and exact
`placement-1.21.json` catalog identity are checked before using it. No opacity,
random-ticking or fluid default is guessed from a block name. The explicit
`debug_void_terrain` input is an observation from the actual source profile;
the backend does not infer false from a seed, empty block contents or absence
of another context.

Full build bounds, physical section coverage, current generation bounds, the
cached NoiseChunk's grid and the fill window are separate domains. In
particular, an old-target fill must keep its existing full NoiseChunk/aquifer
while traversing the narrowed retrogen window. Retrogen bedrock/hole repair is
a separate source continuation through logical Proto writes. SURFACE uses its
actual region context and current generation guard; it must not reuse the
retrogen fill window as a full-column scan range. These integrations and their
paired original observations are recorded by their respective owners.
