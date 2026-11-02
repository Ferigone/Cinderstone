# NoiseChunk Blender wrappers and density metadata

Target: unchanged Minecraft Java **1.21**, protocol **767**. The original inner
server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Evidence here is direct Java 21 `javap` inspection. No REA execution or raw
disassembly export is claimed. Runtime comparison results belong in the
coordinating Blender validation record.

| Original method | Required behavior |
| --- | --- |
| `NoiseChunk` (`dzc`), constructor lines 42–209 | Allocate two FlatCaches without their normal automatic fill. Fill their shared alpha/offset pairs directly from the actual Blender before visiting the router or constructing the aquifer. |
| `NoiseChunk.wrapNew`, lines 872–895 | Replace the BlendAlpha/BlendOffset singleton functions with those captured caches only when the actual Blender differs from the singleton `Blender.empty()`. |
| `NoiseChunk.FlatCache` (`dzc$g`), lines 423–469 | Quantize query X/Z to quart coordinates. Return stored values within the inclusive prefetched square; otherwise evaluate the captured underlying function at the original block coordinates. |
| `NoiseChunk.getOrComputeBlendingOutput`, lines 773–780 | Share one exact block-X/Z key and alpha/offset pair between fallback alpha and offset queries. The prefetched square does not warm this cache. |
| `NoiseChunk.BlendAlpha/BlendOffset` (`dzc$a/b`), lines 783–857 | Capture their NoiseChunk independently of the supplied FunctionContext. Scalar fallback uses its shared pair; array fill delegates to the supplied provider's direct fill. Bounds are `[0,1]` and `[-Infinity,+Infinity]`. |
| `DensityFunctions.BlendDensity` (`dys$e`), lines 840–867 | Evaluate its input, then call the supplied FunctionContext's Blender. An external SinglePointContext inherits the default empty Blender. |
| `DensityFunctions.Ap2` / `MulOrAdd` (`dys$a/y/n`) | Reconstruct parent bounds after visiting children. Ap2's factory calculates general binary bounds before selecting a newly exposed constant optimization; an existing MulOrAdd's mapAll uses its separate constant-sign calculation. |
| `CubicSpline.Multipoint` (`axp$e`), lines 51–138 and 213–216 | Visit the coordinate and nested values, then recompute bounds using the original float32 operations and linear extensions. |

## Captured cache owner

`ChunkDensity::new_with_blender(grid, origin_x, origin_z, cell_count_xz,
Arc<Blender>)` retains the actual constructed Blender. `new` continues to use
the actual empty singleton. There is no setter that can replace a Blender after
wrapping or interpolation, no self-reference, and no reconstruction between
generation phases. The existing move of ChunkDensity carries this owner and all
of its arrays, fallback state, marker caches and preliminary-surface map together.

The two arrays have `noiseSizeXZ + 1` rows and columns. Prefill visits X outside
Z, both inclusive, at block coordinates
`((origin >> 2) + quart_offset) << 2`, with Java int wrapping. It obtains one
pair for each position and writes alpha and offset to their respective arrays.
Even empty Blender construction allocates/prefills the arrays, as in the source;
the empty visitor leaves the original constant functions in use.

For a 16-block span, the inclusive last quart entry also answers block positions
16 through 19 relative to the first quart-aligned origin. An out-of-square query
uses its exact block X/Z as the fallback key. The initial key is the source
`ChunkPos.INVALID_CHUNK_POS`, `pack(1875066,1875066)`, with pair `(1,0)`.
That sentinel comparison is preserved rather than replaced by an initially
missing optional entry. Alpha and offset reuse the same fallback pair.

## FunctionContext and array phases

Wrapped alpha and offset capture this chunk's caches even for external
SinglePointContext sampling. BlendDensity follows a different source contract:
only the NoiseChunk owner context uses the real retained Blender. External
single-point, preliminary-surface and aquifer contexts use the default empty
Blender for that operation. Its input is still evaluated first.

Raw and empty alpha/offset array fills remain constant fills. They do not call
the provider's `forIndex` or advance interpolation counters. A nonempty wrapped
FlatCache delegates to the existing direct-fill implementation, preserving
column/cell/external traversal, counter changes and length errors. BlendDensity
array fill first fills the child array, then calls `forIndex` and applies the
context-selected Blender separately for each value.

The additive DensityContext hooks have raw defaults. DensityGraph's ordinary
raw evaluation therefore retains constant alpha 1, offset 0 and identity density
blending. The global graph and its original bounds remain immutable.

## Recomputed bounds

ChunkDensity keeps a node-indexed visited-bounds table. Reference and marker
bounds forward their visited child. Binary, existing constant-argument, mapped
and range-choice bounds retain their original arithmetic and Java min/max
behavior. Clamp declarations and noise, rarity, gradient and BlendDensity bounds
remain independent of those changed children. Custom nested spline coordinates
also receive exact recursive float32 bounds reconstruction; they do not use a
conservative substitute or the stale raw coordinate range.

DensityContext's bounds hook supplies this metadata to both scalar and array
MIN/MAX short circuits. For example, raw BlendAlpha has bounds `[1,1]`; a
nonempty NoiseChunk's wrapped function has bounds `[0,1]`. Retaining the raw
minimum could skip an alpha query that the original performs and select the
wrong value. BlendOffset's change from `[0,0]` to infinite bounds similarly
affects parent short circuits. `ChunkDensity::wrapped_bounds` exposes the visited
metadata after wrapping without changing RandomState's raw metadata.

The original Ap2, MulOrAdd and Mapped records include their reconstructed bounds
in equality. Their canonical cache keys therefore retain those bounds too,
including signed zero and Java's canonical NaN bits. Multiplication of a gradient
with range `[-2,1]` by `-0.0` illustrates why: an existing MulOrAdd's visitor
produces `[+0,-0]`, while a held Ap2 newly optimized by its factory produces
`[-0,+0]`. Those source records and any surrounding marker owners remain
distinct even though their scalar arithmetic agrees.

This file describes the density-wrapper boundary. Actual old-chunk data
collection, Blender algorithms, region status/availability and biome/mask hooks
have their separately assigned owners. These hooks alone establish no completed
terrain delivery, Main invocation, status transition or FULL runtime chunk.
