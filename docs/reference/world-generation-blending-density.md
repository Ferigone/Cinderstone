# Original old-chunk density blending

The reference is the unchanged Java 1.21 server, protocol 767, inner SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The outer artifact and official mappings are pinned by `fixtures/source-lock.json`.
Inspection used direct Java 21 `javap`, not REA. No raw disassembly or proprietary
Java binaries are included in the repository.

`blending/density.rs` implements `Blender.blendDensity` over the common immutable
Blender and actual calculated BlendingData owners. The sampler performs no world
reads, seed construction or independent caching. Missing actual provider/chunk
owners must fail during construction; they must not become an empty map or a
raw-density fallback. Source-defined missing samples remain distinct from those
ownership failures.

| Original method | Preserved behavior |
| --- | --- |
| `Blender.blendDensity` (`eaa.a`, mapped lines 178–221) | Convert block X/Z to quart coordinates with arithmetic shifts. Convert block Y to eight-block density cells with integer division truncating toward zero, including negative Y. Try the direct old-density lookup before any weighted traversal. |
| `Blender.getBlendingDataValue` (`eaa.a`, lines 225–257) | Use the broad height-and-biome map even for density lookup. Try the current chunk, then northwest when both quart coordinates are chunk boundaries, then west, then north. Stop at the first value unequal to exactly `Double.MAX_VALUE`. The common Blender owns this shared lookup. |
| `Blender.lambda$blendDensity$3` (`eaa.a`, lines 192–212) | Traverse the density map in the original Fastutil encounter order. Decode each chunk key to its signed X/Z and convert to base quart coordinates, then request density cells in `[cellY - 1, cellY + 1)`. |
| `Blender.lambda$blendDensity$2` (`eaa.a`, lines 198–210) | Compute wrapped integer coordinate differences before widening; double the integer vertical difference. Use `sqrt((dx*dx + dy*dy) + dz*dz)`, accept radius 2 inclusively, update nearest distance and accumulate inverse fourth-power weights in source order. |
| `BlendingData.getDensity` (`eab.b`, lines 293–313) and `iterateDensities` (`eab.a`, lines 354–368) | The data owner supplies the source-scaled density values, nullable perimeter columns and exact vertical bounds. The direct bottom-cell special case returns 0.1 before column/XZ selection; unavailable direct samples retain the exact MAX sentinel. |

When a direct sample exists it is returned without combining it with the raw
density. Otherwise the weighted mean is combined using the source linear
`lerp(clamp(nearestDistance / 3, 0, 1), oldDensity, rawDensity)`. If traversal
finds no eligible point, the original returns the supplied raw density. This
is the only raw passthrough in this nonempty sampler. It adds no vertical
fractional interpolation, height-factor smoothstep, epsilon, zero-distance
substitution or nonfinite-value repair.

The common map owner must retain the original map encounter order; sorting old
chunks or samples changes floating-point accumulation. Each BlendingData owner
must already contain its actual calculated runtime density columns. Serialized
height data alone does not reconstruct those columns. Their construction and
cache lifetime belong to the common data/factory modules.

`DensityFunctions.BlendDensity` obtains its Blender from the actual supplied
FunctionContext. A NoiseChunk owner context exposes its retained Blender;
SinglePointContext inherits the original EMPTY Blender and therefore returns
raw density, including aquifer and preliminary-surface external samples. The
ChunkDensity adapter owns that distinction. Captured BlendAlpha/BlendOffset
wrappers have their own NoiseChunk cache contract and must not be inferred from
the density callback's context rule.

Focused original-runtime evidence should compare real calculated data and
source map ordering across direct boundary ownership, negative Y, radius limits,
weighted neighbors and source no-sample passthrough. An oracle constructed with
the original private Blender constructor establishes those arithmetic/data
paths only; it does not by itself establish WorldGenRegion factory dependencies,
old-chunk scheduling or cache ownership. Compiler/runtime results belong to the
coordinated validation ledger. This module alone establishes no Main callsite,
completed generation stage, upgraded-world parity or FULL chunk.
