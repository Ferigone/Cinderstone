# Seeded density router composition

This note uses direct `javap -c -p` inspection of the unchanged official
Minecraft Java 1.21 server, plus the bounded original-method observations below.
No REA tool or REA Evidence is claimed.
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

`Ap2.mapAll` (`dys$a.a(dyr$f)`) visits both arguments and calls
`TwoArgumentSimpleFunction.create` (`dys$y.a`) again. Once NoiseChunk has erased
the holder layers, this factory can recognize a direct `Constant` and replace
ADD/MUL with `MulOrAdd`. A marker or another function over a constant remains a
function; it must not be folded into a constant. Raw RandomState graphs retain
their holders and the original `Ap2` behavior.

The distinction affects arithmetic and lifecycle ordering. At `(0,-64,0)`, the
original raw `mul(0, y_clamped_gradient(-64,320,-1,1))` returns positive zero;
after NoiseChunk wrapping it evaluates the gradient first and returns negative
zero. For `mul(0, interpolated(gradient))`, sampling the wrapped function with
the chunk owner before interpolation throws the original outside-interpolation
error. Skipping the input because the coefficient is zero would hide that
stage error. External single-point and array sampling still use the rewritten
function, while marker caches preserve their external-context bypasses.

[`world-generation-density-rewrite.json`](../../fixtures/world-generation-density-rewrite.json)
records actual scalar/fill bits, bounds, marker-owner identity and the lifecycle
error from a constructed original NoiseChunk, with no Unsafe allocation. The
native owner retains a per-node arithmetic rewrite and canonicalizes the same
`MulOrAdd` shape. Raw evaluation does not use that metadata.

The recorded holder-rewrite fixture uses an empty Blender and an explicit raw
`BeardifierMarker`, rather than an actual empty Beardifier owner. In that
historical witness, holder erasure preserves child bounds. The original
two-argument factory recomputes bounds before detecting a constant and passes
them into `MulOrAdd`; the observed zero-coefficient witnesses retain exact
`[-0,+0]` bounds. Those fixture observations and their source pins remain
unchanged.

Native MIN/MAX use `ChunkDensity`'s reconstructed visited bounds. The
[actual empty Beardifier](world-generation-empty-beardifier.md) has infinite
bounds even with no selected structures, and its fill visits the active
provider. The current chunk owner applies those metadata and fill hooks;
Blender wrappers likewise supply their visited metadata. Direct raw graph
evaluation retains the marker's zero bounds and zero fill without provider
visits. The [captured Beardifier owner](world-generation-beardifier-integration.md)
now also evaluates actual ordered rigid-piece and junction contributions.
This data ingress does not implement structure generation or establish real
StructureManager collection and dependency ownership.

Resource locations with an empty namespace, such as `:overworld`, use the
original `minecraft` default namespace when loading density settings or names.

The packaged density fixture contains each router, its noise grid and the
legacy selection. It does not contain the complete generator settings,
surface-rule tree, biome source, sea level, default materials or scheduler
state. Those must come from their actual owners. Existing raw evaluation has
empty Blender and zero beardifier behavior; it does not certify an arbitrary
chunk as having no blending or structures. Production composition must require
an explicit supported empty context or reject an unsupported context. These
accessors alone do not complete a generator delivery or establish a Main
callsite.
