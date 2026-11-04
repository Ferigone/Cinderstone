# Existing target source and native validation

This stage uses the pinned official Minecraft Java 1.21 server (protocol 767),
inner JAR SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The harness uses direct original-class calls and direct `javap` inspection.
REA tools were unavailable. It does not run a saved world or a server.

`src/worldgen/validation/WorldgenExistingTargetOracle.java` exports
`fixtures/world-generation-target-state-facts.json` with
`--export-state-facts <placement-catalog> <output-json>`. The original signed
JAR stays unchanged: load the default-package harness through a child
`URLClassLoader`, with the original JAR and packaged helper classes in its
parent, as in the existing Blender checker.

The exported `states` array is indexed by actual `Block.getId(state)`. Its
26,684 entries were obtained from the actual `Block.BLOCK_STATE_REGISTRY`;
the harness checks every existing placement catalog block name, default ID,
canonical range and block identity against that registry. `catalog_sha256`
means the exact bytes of `fixtures/placement-1.21.json`, whose SHA256 is
`e044d4bdf90cfcae62043b630b1c673bb3582025b684a7fbe5878fe6747e8a50`.
The observed default IDs are AIR `0`, VOID_AIR `12958`, BEDROCK `79` and
DEEPSLATE `24905`.

Each row contains six actual source predicates:

| Fixture field | Original owner and mapped method |
| --- | --- |
| `is_air` | `BlockBehaviour.BlockStateBase.isAir`, mapped line 961, `dtc.i()` |
| `has_fluid` | Negated `FluidState.isEmpty`, mapped line 50, `!state.u().c()` |
| `randomly_ticking_block` | `BlockStateBase.isRandomlyTicking`, mapped line 1266, `state.v()` |
| `randomly_ticking_fluid` | `FluidState.isRandomlyTicking`, mapped line 87, `state.u().f()` |
| `world_surface_wg_opaque` | Actual `Heightmap.Types.WORLD_SURFACE_WG.isOpaque().test(state)`, `dyy.a.a.e()` |
| `ocean_floor_wg_opaque` | Actual `Heightmap.Types.OCEAN_FLOOR_WG.isOpaque().test(state)`, `dyy.a.c.e()` |

The heightmap predicates come from the actual enum's predicate objects, rather
than a substitute material classification. This table supplies no light,
shape, final heightmap, entity, tick-list, readiness or saved-status facts.
Its observed export SHA256 is
`29c1100e93f0b5c4a247d6e54f66ea459d39d0352c83accbab6adc1aedc29e3c`.

The same harness accepts `<output-directory>` and JSON lines on stdin. Each
query supplies `id`, `settings`, `biome_source`, `seed`, chunk `x`/`z`,
`level_min_y`, `level_height`, `blocks_file`, `biomes_file` and `posts`.
Block input covers the complete physical section array, including padding
outside logical build bounds, as canonical little-endian u32 IDs in Y/X/Z
order. Biome input is a JSON array of canonical registered names in ascending
section Y, quart Y, quart Z, quart X order. `posts` has one entry per physical
section: null preserves an unallocated queue, an empty array creates an
allocated empty queue, and other arrays contain packed signed or unsigned
16-bit positions, preserving order and duplicates.

Optional `retrogen` is decoded by the actual `BelowZeroRetrogen.CODEC`.
`target_status` is explicit; each `missing_bedrock` word accepts a signed or
unsigned decimal long string and is converted without changing its bits.
This is controlled current metadata, rather than evidence of natural saved
chunk history. The actual persisted status starts and remains EMPTY. An
active SPAWN retrogen target legitimately supplies the original highest
generated status used by the old-palette biome guard; it does not certify
SPAWN work.

For SURFACE, `surface_neighbors` supplies the eight neighboring stored-biome
owners as `{x,z,level_min_y,level_height,biomes_file}`. A strict fixture palette
adapter applies the original logical quart Y clamp and rejects unavailable
neighbors. The center always reads its same actual ProtoChunk palette after
BIOMES. This adapter does not claim WorldGenRegion, scheduler or persisted
BIOMES publication. Omitting the neighbor input explicitly skips SURFACE.

The harness exports `initial`, `biomes`, `noise_before_retrogen`, `noise` and,
when requested, `surface` snapshots. Each snapshot includes complete physical
block and biome images, the actual WORLD_SURFACE_WG and OCEAN_FLOOR_WG first
available heights in Z/X order, ordered per-section postprocessing queues,
queue allocation, and the actual three section counter shorts. It records
logical generation bounds, persisted/highest status, retrogen presence,
unsaved state and the original `SharedConstants.debugVoidTerrain` result.
The source primes WG maps once from the explicit existing input before
exporting INITIAL. The native probe receives only that INITIAL source state
as its starting storage; later source snapshots are comparison outputs.

NOISE calls the actual public `NoiseBasedChunkGenerator.fillFromNoise`, then
the actual original `ChunkStatusTasks.lambda$generateNoise$0`
(`dwb.b(duy)`, mapped lines 66–77). The latter performs bedrock replacement
and hole masking even when the fill has no cells. This is an original task
callback observation, not an execution of the complete ChunkStep. SURFACE
uses the public generator overload with an actual retained NoiseChunk,
generation context and biome manager. StructureManager is absent only where
the installed NoiseChunk makes the source creation callback unused; the
empty Blender and BeardifierMarker were explicit controlled inputs in the
recorded `a28bfc2`/`8ac66497` run. The current oracle instead supplies an actual
empty `Beardifier` owner; its new paired observations are recorded separately
in [zero-overlap validation](world-generation-zero-overlap.md). Historical
fixture hashes and results are retained unchanged.

BIOMES invokes the actual original private `doCreateBiomes` body
(`dzb.b(eaa,dzm,ddq,duy)`) synchronously on one controlled worker. The public
asynchronous wrapper is not claimed here: arbitrary background executor
selection would change which original ThreadLocal climate-search cursor
receives the next case. The actual builtin source persists across cases on
this controlled worker, with the real NoiseChunk-wrapped sampler supplied by
the original body. The harness preinstalls the actual full-accessor
NoiseChunk, then runs the body. This controlled factory installation is not
an initial pointer-equality claim about the native owner.

The optional `skip_biomes:true` profile is restricted to an actual source
clamp with no vertical cells and excludes SURFACE. It creates no RandomState,
NoiseChunk or aquifer. The public fill receives null generation dependencies
and returns the original ProtoChunk before using them; the actual task
continuation still applies the supplied retrogen. That profile exports no
BIOMES snapshot or fabricated aquifer identity, and directly observes that
the chunk's NoiseChunk slot remains absent. Its missing `biome_source` also
passes actual null to the original generator constructor. The source stores
that argument for lazy use and does not dereference it on this path. This is
a controlled null-argument early-return probe, not a production generator
configuration.

The harness observes actual NoiseChunk, aquifer, Blender and cache owner
identity. A sampler factory returns a new record on each invocation, so
record identity across factory calls is not treated as source cache
ownership. Instead a pure delegated biome-source observer records the
sampler actually delivered to BIOMES callbacks, with no extra climate
sampling or NoiseChunk prewarming.

The final paired run passed the declared five cases. The
[runtime record](../../fixtures/world-generation-existing-target-runtime.json)
preserves the declared inputs, actual original/native observations, artifact
and source hashes, execution metadata and raw transcript digests. The
[comparison record](../../fixtures/world-generation-existing-target-validation.json)
records complete phase block/palette equality, both WG maps, section counts,
queue values/allocation/order, status metadata and ordered biome callbacks.
The original execution exited 0 in 6.076 seconds and the native execution
exited 0 in 1.469 seconds; both processes were reaped without timeout. These
are bounded probe execution observations, not a performance comparison.

| Controlled case | Logical build bounds | Observed NOISE and task behavior | Actual SURFACE block changes |
| --- | --- | --- | --- |
| `aligned_existing` | `[-64,320)` | 28,627 changed blocks; 92 gold markers preserved inside the fill span by source AIR skips | 15,749 |
| `partial_positive` | `[1,128)` | 15,365 changed blocks; 256 writes at physical Y=0; 69 unwritten tail gold markers preserved | 2,064 |
| `partial_negative` | `[-63,64)` | 29,763 changed blocks; 256 writes at physical Y=-64; 69 unwritten tail gold markers preserved | 18,079 |
| `retrogen_active` | `[-64,320)` | 12,505 fill changes, then 1,280 BEDROCK-to-DEEPSLATE and 115 hole-mask-to-AIR changes | 13,158 |
| `no_cells_retrogen` | `[-64,320)` | No fill changes or NoiseChunk; the actual continuation still makes 1,280 bedrock replacements and 6 hole-mask changes | Skipped explicitly |

The partial cases compare all 128 physical block layers, including the layer
below logical build bounds and the unwritten cell tail. The source's direct
NOISE section writes deliberately reach the lower physical padding; a
logical-height write filter would change this behavior. Every cell outside
the source fill span was compared, not only the gold witnesses.

The active retrogen case retains the full 384-height NoiseChunk/aquifer while
its material fill and BIOMES generation accessor covers `[-64,0)`. BIOMES
leaves the upper section palettes unchanged. The actual post-NOISE callback
uses the supplied full BitSet, with low selected columns and a retained high
word. SURFACE reads full build-bound storage and live WG heights while its
actual generation-accessor write clipping leaves Y>=0 unchanged. It keeps
all stored biome palettes. The NoCells case calls the public fill with
unavailable biome, RandomState, Blender and structure dependencies, observes
the absent NoiseChunk before and after, then executes the real continuation.
Neither case advances the original EMPTY persisted status or removes the
controlled SPAWN retrogen metadata.

The four SURFACE cases compare every physical block, both WG heightmaps,
the section counters and complete postprocessing queues, as well as actual
seeded zoom queries against center and supplied foreign stored palettes.
NOISE appends source postprocessing entries in the selected ordinary cases;
SURFACE adds zero entries in all selected cases. Preserving these queues
proves their values, allocation and existing prefixes for this input, without
claiming the positive SURFACE insertion branch or a global inter-section
queue order.

Original identity observations concern the actual NoiseChunk, aquifer,
Blender, cache owners and callback sampler. Native observations separately
show the retained target Box, section/state storage, map objects and post
array, with released section leases. The native private aquifer pointer and
cache allocation identity are not exposed or measured. No original runtime
lock/acquire-order trace is inferred from storage equality or released
native leases.

The run used native source commit
`a28bfc2ef8b11581b9a09f76a9d54f25e8afc890` and source-fact commit
`fd21a1f`, based on master `2ac289776ea884d6b9ad308844b9e402a5647658`.
The exact source pins are:

| Input | SHA256 |
| --- | --- |
| Original oracle | `8ac6649749066b4311f3d2003234173503851be37d8e510c43a53efdd3b58b71` |
| Native probe source | `675f1feee97a107a30969cf3998701ac41190eb00d81a346d7394cb25e3a72f2` |
| Checker | `528db88ade6d578de0bb2727aac80bc9e1513d483bf24869bb74cccd035045bb` |
| Published runtime record | `bcb93c31212aa721c9529c36ae51919fd0770f713c293220defb2ad10962cb04` |
| Published comparison record | `38a4e56ff2ccadf8d29ab9fcba1b27dc351e449d87326503929d3b2e7e2af312` |

The runtime record also pins the original outer JAR, official mappings, every
original dependency JAR, the packaged bootstrap and helper sources, the
native library/dependencies and the compiled native probe. No original JAR,
compiled class, saved world or raw disassembly is published.

To reproduce with Java 21, Rust and the pinned original artifacts available,
choose a new empty work directory outside every checkout:

```sh
python3 src/worldgen/validation/check_existing_target.py \
  --oracle-dir "$MC121_ORACLE" \
  --work-dir "$EXTERNAL_EMPTY_WORK" \
  --runtime-report "$EXTERNAL_EMPTY_WORK/runtime.json" \
  --report "$EXTERNAL_EMPTY_WORK/validation.json"
```

The default command builds the locked native library. `--skip-build` is only
for an explicitly completed library build of the same source and target;
`--target-dir` and `--java-home` select existing toolchain locations. The
runner compiles original helpers in a parent class loader and the oracle in
a child, preserving the signed original JAR unchanged. It generates the
controlled inputs, runs Java first, and sends native stdin exactly the query
plus original INITIAL storage. No later original phase output becomes native
input. It captures each process separately, bounds execution and reaps its
process group without an implicit retry.

The selected runtime cases do not exercise cave-biome preservation or an
unbound cave holder, a high-only hole mask, initially absent WG maps, unused
packed heightmap bits, or positive SURFACE-only postprocessing insertion.
They do not observe private native aquifer/cache pointer identity or the
original lock trace. They also do not establish saved-world import/DFU,
concrete WorldGenRegion dependencies, nonempty beardifier behavior, every
retrogen target status, datapack/registry overrides, actual ChunkStep status
completion, Main integration, FULL publication or the whole checklist row.
