# Blender height and offset calculation

Target: unchanged Minecraft Java **1.21**, protocol 767, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Artifacts and official mappings remain pinned by `fixtures/source-lock.json`.
Inspection used direct Java 21 `javap`; no REA Evidence is claimed. Raw JVM
disassembly and proprietary binaries are not published.

`Blender` maps to `eaa`. Its `blendOffsetAndFactor` is mapped lines 121–165,
`heightToOffset` 170–174, and the height accumulation lambdas 135–153 and
139–151. `blending/blender.rs` implements that pure height/factor calculation
against the common immutable, calculated Blender data owner. It supplies the
internal `blend_offset_and_factor(&Blender, blockX, blockZ)` calculation and
`height_to_offset(height)` helper. The common owner controls exported methods,
data construction, map order and direct lookup; this module creates no RNG,
world callback, per-query cache, fabricated old column or generation status.

## Direct samples and weighted calculation

Block X/Z become quart coordinates using arithmetic right shift by two, so
negative block coordinates preserve `QuartPos.fromBlock`. The shared
`getBlendingDataValue` helper (mapped lines 225–257) first checks the owning
chunk's data. At quart boundaries it then checks northwest, west and north in
that order as applicable, stopping at the first value unequal to
`Double.MAX_VALUE`. These direct queries use the broad height/biome map.
`BlendingData.getHeight` ignores the supplied zero Y argument. A direct height
returns alpha zero and `heightToOffset(height)` immediately.

If direct lookup finds no height, the source iterates the broad height/biome
map and each data owner's height array in its original order. Chunk keys decode
as signed low-32-bit X and high-32-bit Z, then shift left two for the world quart
origin. Sample-coordinate subtraction takes place as wrapping Java integers
before widening to double. Distance is `sqrt(dx*dx + dz*dz)`, not a rescaled
hypotenuse calculation. Samples at distance **27 quart cells or less** are
included, with weight `1 / (distance * distance * distance * distance)`.

The nearest-distance update uses a strict comparison. For each accepted sample,
the code adds `height * weight` to the height sum, then adds weight to the weight
sum. It preserves the source expression order without regrouping sums or
replacing products with powers. There is no extra zero-distance or finite-value
fallback. Absence of accepted samples returns the original `(alpha=1, offset=0)`
identity result. Otherwise the weighted height is `heightSum / weightSum`.
For `t = clamp(nearestDistance / 28, 0, 1)`, alpha is evaluated as
`3*t*t - 2*t*t*t`, preserving the original left-associated products. Rewriting
this as `t*t*(3-2*t)` can change the final double bits.

`heightToOffset` first evaluates `d = height + 0.5`, then the original
`Mth.positiveModulo(d, 8)` as `((d % 8) + 8) % 8`. It retains the complete source
expression and operation order:

```text
1 * (32*(d-128) - 3*(d-120)*m + 3*m*m) / (128*(32-3*m))
```

The sentinel is the finite `Double.MAX_VALUE`, not NaN. Height-array availability,
stored values, sample coordinates and iteration belong to the actual
`BlendingData` owner; this calculation does not invent values for absent cells.

## Shared map and cache ownership

`Blender.of` (mapped lines 76–110) uses actual region/old-generation owners. It
scans offsets with X outermost and Z innermost, each from -7 through 7, retaining
coordinates whose squared offset distance is at most 64. Every non-null updated
BlendingData owner enters the broad map; the central -1 through 1 square also
enters the density map. Both maps retain the same data objects. The constructor
(113–116) stores these maps rather than copying their calculated samples into
per-query fallback state.

Those maps are original Fastutil 8.5.12 `Long2ObjectOpenHashMap` objects. Its
`Long2ObjectMap.forEach` uses `MapEntrySet.fastForEach`: key zero first if present,
then occupied hash slots in descending index order. Default sizing, hash mixing,
linear probing and rehash order therefore affect weighted addition order.
The parent owner's `height_and_biome_entries` must preserve that order; sorted
coordinates or insertion order are not interchangeable. Each data owner's
height iterator then visits its actual 16 stored boundary indices in ascending
index order.

Blender height math has no last-query cache. Original NoiseChunk's
`getOrComputeBlendingOutput` (mapped lines 773–780) owns its separate coordinate
cache, while old chunk BlendingData owns the persistent calculated-data state.
Those lifetimes and any density/router integration belong to their respective
owners. Focused compiler and original/native runtime validation are coordinated
at the composed stage. This module alone does not establish nonempty Blender
Main integration, complete old-world boundary processing or FULL chunks.
