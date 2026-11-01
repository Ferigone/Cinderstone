# NoiseChunk interpolation owner and traversal

The source is the pinned Minecraft Java **1.21**, protocol 767 artifact in
`fixtures/source-lock.json`. The inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
the official mappings SHA-1 is
`31c77994d96f05ba25a870ada70f47f315330437`.
Inspection used direct `javap -p -c` through Java's `jdk.jdeps` module. No REA
session or REA Evidence was available or executed.

| Original mapped method | Native responsibility |
| --- | --- |
| `NoiseChunk` (`dzc`) constructor, lines 42–209 | Register router markers in router order, then block-density cell cache; retain one owner per wrapped function. |
| `initializeForFirstCellX`, `fillSlice`, `advanceCellX`, lines 274–314 | Fill adjacent X slices through the original column provider; retain scalar/array counters and ordered interpolator owners. |
| `selectCellYZ`, `updateForY/X/Z`, `swapSlices`, `stopInterpolation`, lines 350–391 | Select corners and fill cell caches before block iteration; interpolate Y, then X, then Z; increment scalar counter at Z update. |
| `NoiseBasedChunkGenerator.doFill` (`dzb`), lines 324–413 | Visit cell X and Z forward; cell Y and local Y downward; local X and Z forward. Swap slices after all Z columns in one X cell. |
| `NoiseChunk.preliminarySurfaceLevel`, `computePreliminarySurfaceLevel`, lines 248–265 | Quantize X/Z to quart-block origins, cache by packed X/Z, scan from settings top to minimum in cell-height steps; first density strictly above `0.390625` wins; retain `Integer.MAX_VALUE` when none is found. |
| `DensityFunctions.HolderHolder.mapAll` (`dys$j`), line 397; `NoiseChunk.wrapNew`, lines 872–895 | Visit the held function, then erase the holder layer before parent wrapping and equality. |

`NoiseChunk<'g>` borrows its immutable `DensityGraph`. This keeps canonical node
IDs bound to the graph while its `ChunkDensity` caches live. `visit_blocks`
executes the source traversal and calls the material owner after each Z update.
The wrapper clamps noise settings against the actual level height accessor and
explicitly rejects unsupported partial cells. Callback failure stops
interpolation and prevents reusing a partially applied traversal; the material
owner must discard privately staged output. Finishing this traversal does not
publish a `ChunkStatus` or certify a FULL chunk.

The existing five marker implementations remain the sampling owner:
interpolated, flat cache, 2D cache, once cache and all-in-cell cache. A corrected
canonicalization removes `Reference`/holder layers before comparing parent
records. Registration delegates through the reference before marking its shared
identity, so a held marker is registered exactly once. Keeping an extra holder
layer would make direct and held copies allocate different cache owners.

Holder erasure also lets `Ap2.mapAll` rebuild ADD/MUL as `MulOrAdd` when a child
becomes a direct constant. `ChunkDensity` retains this arithmetic rewrite for
both scalar and array evaluation, including external point contexts; its
canonical marker keys use the same rewritten shape. A zero coefficient still
evaluates the held input, preserving negative zero and pre-interpolation
errors. The [density ledger](world-generation-density.md) and
`fixtures/world-generation-density-rewrite.json` record the source methods and
actual constructed-NoiseChunk witnesses. Raw density evaluation retains its
pre-wrapping semantics.

`ChunkDensity::preliminary_surface_level` samples the wrapped initial-density
root with a separate point context, including queries outside the chunk. It
preserves the current interpolation owner flag and never substitutes the
currently interpolated block density. The cache survives `stop_interpolation`
for the later SURFACE phase.

`fixtures/world-generation-noise-chunk.json` records direct runtime observations
from the unchanged Java classes. The full constructor and its real RandomState,
fluid picker, empty Blender and BeardifierMarker were used for two Overworld
chunks (including negative X), one Nether chunk and one End chunk. Original
interpolator/cell-cache owner counts were 8/1 for Overworld and 1/1 for Nether and
End. Preliminary levels were identical before and after traversal, including
the actual no-surface sentinel in Nether and End. These are original-method
observations, not a production Minecraft server-world run.

A separate small original-method identity probe used only wrapper-state fields
allocated with `Unsafe`: direct and held interpolated constants returned the
same interpolator object, with one owner. Its limited scope is recorded in the
fixture. The regression verifies that native holder canonicalization preserves
that observed ownership. Focused native guards cover callback rejection and
unsupported height-cell alignment; compiler/runtime results belong to the
team handoff and validation ledger.

Nonempty blending, structure terrain adjustment, chunk-status scheduling,
lighting, carving, features, persistence and production Main wiring remain
separate contracts. This interpolation delivery alone does not complete
`world_generation.3`.
