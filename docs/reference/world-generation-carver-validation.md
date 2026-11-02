# Original AIR-carver material validation

Target: the pinned Minecraft Java 1.21 server, protocol 767. This is direct
`javap` inspection plus execution of the original Java classes, not REA output.
The portable original/native comparison is
`src/worldgen/validation/check_carvers.py`; its recorded result is
`fixtures/world-generation-carver-runtime.json` when the paired check passes.

## Source boundaries and actual owners

Original `NoiseBasedChunkGenerator.applyCarvers` (`dzb.a`, official lines
257–287) first captures `BiomeManager.withDifferentSource` over the generator's
real biome source and **raw `RandomState.sampler()`**. This differs from the
wrapped cached chunk sampler used to fill BIOMES. It creates one real
`WorldgenRandom(LegacyRandomSource(RandomSupport.generateUniqueSeed()))`, then
acquires the target's retained NoiseChunk/aquifer, constructs CarvingContext,
and gets the actual AIR CarvingMask. Neighbor loops visit X offsets -8..8,
then Z offsets -8..8. Each neighbor's actual `ChunkAccess.carverBiome` cache
suppresses the raw source and generation-settings supplier on a hit. For every
ordered configured holder, the invocation RNG reseeds with `worldSeed + index`,
neighbor X/Z, tests `isStartChunk`, and calls that actual configured carver when
selected. The index restarts at zero for each neighbor.

The Java probe reproduces that source loop over 289 actual ProtoChunk cache
owners; it does not construct WorldGenRegion or call ChunkStep. Its center
already has real source BIOMES, public NOISE and actual SurfaceSystem output.
It uses the same installed NoiseChunk, interpolators, cell caches and aquifer
through those phases and passes the original aquifer into actual configured
carver calls. The source worker's persistent MultiNoise parameter-tree cursor
is reused across stored BIOMES, raw neighbor-settings and raw zoom-selected
surface-repair queries, in their actual interleaved order. End source lookup
also uses the original source. No fixed biome replaces the real selection.

SURFACE uses an explicit view of the actual 3x3 stored section palettes with
source X/Z routing and Y clamp. CARVERS instead changes that manager's source
to the raw generator lookup, as the original caller does. Missing stored
neighbors fail. The fixture's explicit fresh 17x17 availability is not evidence
that a scheduler produced structures or reached STRUCTURE_STARTS. All 289
original ProtoChunk statuses remain their real EMPTY values; no status is set
by the probe. Actual EMPTY/PRE heightmap behavior is used. There is no FULL
status, dummy aquifer, dummy surface owner or supplied expected material image.

## Original block/fluid tags must be bound

`pd`/`akt` bootstrap alone leaves built-in block-holder tags unbound. Actual
protected `WorldCarver.canReplaceBlock` then returns false for stone/dirt in
caves and netherrack in Nether caves. Comparing that setup would test mask
geometry while suppressing real carving material changes.

The probe therefore reads the pinned original JAR's single built-in block and
fluid tag pack. It decodes actual `TagFile.CODEC` (`awt.a`), resolves the
original `TagEntry` objects with original `TagLoader.build` (`awv.a(Map)`),
then calls actual `Registry.bindTags` (`jz.a(Map)`). Every tag must resolve;
source file SHA-256 digests are recorded in the result. This is a single
controlled original pack, not a claim about external pack overrides or reload
scheduling. The original protected replacement witness for canonical
stone/dirt/netherrack is cave `[true,true,false]`, Nether cave
`[true,true,true]` after binding. Native predicates are not used in the Java
reference setup.

## Compared data and RNG observations

The paired runner compares every original/native canonical block state before
and after AIR carving, actual full 3x3 BIOMES palettes and resolver query order,
the complete trimmed stored AIR-mask long array, both WG heightmaps, and every
postprocessing short/decoded position in actual section storage order. Original
null/empty queue allocation is diagnostic only; both mean no queued contents.
Section lists do not reconstruct a global order across different sections.

It also compares all 289 neighbor cache observations, configured identities and
ordering, every reseed/start/carve observation and invocation `next(bits)` count,
the final underlying Legacy 48-bit state after deterministic reseeding, and all
interleaved raw biome queries with their role and returned real biome. Local
cave/canyon tunnel Legacy draws do not belong to WorldgenRandom's invocation
count. Original second accesses use throwing suppliers to witness actual cache
hits; the native owners must remain cached.

`RandomSupport.generateUniqueSeed` advances its actual process-global atomic
uniquifier once, multiplying by `1181783497276652981`, and combines it with the
real monotonic clock. The Java probe checks the actual one-step atomic advance,
even in the real empty End AIR-list case. Clock-derived numeric seeds and the
unreseeded End final RNG state are not expected to match between processes.
Deterministic geometry starts only after source `setLargeFeatureSeed`.

Cases use seed 123456789: Overworld (-640,320), Overworld (-638,324), Nether
(0,0), and End (65,0). The second Overworld position was selected from an
actual source-observed canyon start to obtain nonempty target canyon mask
contributions. The runner requires nonempty target-mask evidence from all four
actual configured carvers across the cases; an `isStartChunk` count alone does
not prove the target's geometry was visited. Nether has actual level height
256 while its noise/carving generation depth is 128.

## Reproduction and limits

With the pinned oracle folder, Java 21 tools, and Rust available:

```sh
python3 src/worldgen/validation/check_carvers.py \
  --oracle-dir /path/to/pinned-oracle \
  --work-dir /path/to/external-work \
  --write-report fixtures/world-generation-carver-runtime.json
```

Use a dedicated worktree Cargo target. `--skip-build` uses a library already
built by the coordinator from that exact worktree. Java classes, raw arrays,
disassembly and original binaries stay outside the repository.

The selected material cases cover the original built-in `debug_mode=false`
configurations, ordinary fresh generation, observed-empty structure/blender
influence, unchanged AIR filter, and initially absent neighbor carver caches.
They do not prove debug remapping, an installed additional mask, upgrading,
retrogen, arbitrary resource packs or LIQUID carving. Runtime Main scheduling,
real neighbor readiness, persisted status changes and publication remain
integration work. The fixture is bounded path evidence, not checklist or
production-readiness completion.
