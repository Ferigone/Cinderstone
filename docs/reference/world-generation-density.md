# Seeded density router composition

This note describes direct `javap -c -p` inspection of the unchanged official
Minecraft Java 1.21 server. No REA tool or Java execution observation is claimed.
The artifact URLs and outer SHA-1 values are pinned by
[`fixtures/source-lock.json`](../../fixtures/source-lock.json). The inspected
inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings identify the classes and source line spans below.

| Original method | Observation and native consequence |
| --- | --- |
| `RandomState.<init>` (`dzm`, lines 36–122) | Creates the settings-selected sequential algorithm from the world seed, then calls `forkPositional` once for its root. Aquifer and ore roots come from that root's resource-location hashes and their own positional forks. The density graph now exposes its actual immutable positional root, seed and legacy selection for composition; copying the root consumes no sequential RNG. |
| `RandomState.NoiseWiringHelper.visitNoise` (`dzm$a.a(dyr$c)`, lines 56–72) | Legacy temperature uses `LegacyRandomSource(seed)` with octave `-7` and amplitudes `[1,1]`; vegetation uses `seed+1`; offset gets its special zero-amplitude normal-noise binding. Other holders use `RandomState.getOrCreateNoise`. The existing graph's special router wiring remains separate from the generic named noise cache used by surfaces. |
| `RandomState.NoiseWiringHelper.wrapNew` (`dzm$a.a(dyr)`, lines 76–83) | Terrain blended noise uses the legacy instance at offset zero or the root's `minecraft:terrain` stream; End-island density uses the world seed. This stage does not change those existing bindings. |
| `NoiseRouter` (`dze`, record constructor and accessors at source line 11; `mapAll`, lines 63–78) | All fifteen fields survive seeding and visitor composition. `DensityRouter` exposes their original record order and names: barrier, fluid floodedness/spread, lava, six climate functions, initial density, final density, and three vein functions. `DensityGraph::router()` rejects incomplete graphs rather than filling missing fields with constants. |

`DensityRouter` node IDs belong to their originating `DensityGraph`; the router
is a view of seeded graph roots, not a chunk or sampling owner. Interpolation
and marker caches remain owned by `ChunkDensity`, and the generation traversal
must use that owner. Raw `DensityGraph::eval` preserves pre-NoiseChunk marker
pass-through semantics.

The packaged density fixture contains each router, its noise grid and the
legacy selection. It does not contain the complete generator settings,
surface-rule tree, biome source, sea level, default materials or scheduler
state. Those must come from their actual owners. Existing raw evaluation has
empty Blender and zero beardifier behavior; it does not certify an arbitrary
chunk as having no blending or structures. Production composition must require
an explicit supported empty context or reject an unsupported context. These
accessors alone do not complete a generator delivery or establish a Main
callsite.
