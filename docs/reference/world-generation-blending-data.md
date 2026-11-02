# Original BlendingData ownership and columns

Target: pinned Minecraft Java 1.21, protocol 767. Evidence is direct `javap`
and official mappings, not REA. The original inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The official mapping SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Original `BlendingData` is `eab`; its public collection and leaf calculations
are different source boundaries.

## Actual region collection and persistent owner

Original `getOrUpdateBlendingData` (117–125) asks the real WorldGenRegion for
the target chunk and reads its actual BlendingData slot. It returns absent if
that slot is absent or the chunk's highest generated status is before BIOMES.
Otherwise it queries all eight neighbor chunks, in Direction8 enum order,
selecting those for which `isOldNoiseGeneration == false`, then invokes
`calculateData` on that same target data object. ChunkAccess defines old-noise
generation as BlendingData being present. This is not inferred from available
heightmaps, seed, biome names or an invented old/new flag default.

The eight neighbor reads precede calculateData even if its one-time calculated
flag is already true. `Direction8::ALL` preserves NORTH, NORTH_EAST, EAST,
SOUTH_EAST, SOUTH, SOUTH_WEST, WEST, NORTH_WEST. Missing dependencies must error;
observed absent data and actual insufficient status remain distinct source
facts. The integration owner performs this collection and retains the same
calculated object through `Arc<BlendingData>`; reconstructing a fresh object per
request would discard its source caches and lifecycle.

The data leaf requires actual old-chunk material/biome/collision owners. A
private original leaf executed against a controlled ChunkAccess snapshot is
useful bounded evidence, but does not establish a real ServerLevel,
WorldGenRegion, status scheduler or completed loaded legacy chunk. In
particular, an actual fresh ProtoChunk at EMPTY rejects biome queries; assigning
it a fabricated BIOMES/FULL status is not a valid substitute.

## Persistent fields and runtime fields

The original constructor (100–113) derives its old-generation height accessor
from `min_section * 16` and `max_section * 16`, preserving Java integer wrapping.
The CODEC (86–97) stores required `min_section`, required `max_section`, and a
**lenient optional** double-list `heights`. A missing or malformed optional
field initializes 16 `Double.MAX_VALUE` entries. A decoded double-list with the
wrong length fails the outer array-size validation. Native `from_json` accepts
the normal integer field profile and reproduces that optional/list boundary.
Typed `from_serialized` preserves all doubles, including nonfinite values;
there is no finite-value cutoff or NaN-as-missing convention.

Only heights persist. `serialized()` omits them when every entry equals exact
`f64::MAX`; otherwise it includes the whole 16-entry array. Biome columns,
density columns and `hasCalculatedData` are runtime state and are never saved.
Decoding saved heights therefore starts with absent runtime columns and a false
calculated flag. Saved non-sentinel heights skip height rescanning, while
selected density/biome columns are still calculated from actual owners.

The 16 boundary positions, in iterator/index order, are:

```text
(3,0) (2,0) (1,0) (0,0) (0,1) (0,2) (0,3)
(0,4) (1,4) (2,4) (3,4) (4,4) (4,3) (4,2) (4,1) (4,0)
```

These are local quart coordinates. Outside-border quart 4 samples old block
coordinate 15 rather than asking for a fabricated next chunk. Collection
selects the north/west edges, east/south edges and conditional corners exactly
as original calculateData (142–180); calculation order differs from iterator
index order. Even an actual selection with no columns sets calculated true.
Errors retain earlier height/column mutations and leave the flag false. The
leaf supplies no transactional rollback or cloned replacement owner.

## Actual height, density and biome reads

Height scanning (193–207) first checks the actual existing WORLD_SURFACE_WG map.
If present, its first-available Y is clipped to the old-generation maximum; if
observed absent, scanning starts at that maximum. It moves down before each
block read and recognizes only podzol, gravel, grass block, stone, coarse dirt,
sand, red sand, mycelium, snow block, terracotta and dirt. Reaching the old
minimum returns that minimum. An unavailable map is not an observed absent map.

Density calculation (211–248) moves one mutable local position downward through
7 samples, then repeated 1 + 7 samples. Ground contributes +1 and other material
-1; adjacent groups combine with the original addition association and divisor
15. The final array entry remains -1 before possible surface correction. That
correction preserves floor division by 8 and signed double remainder for
negative heights. Getter/iterator output scales stored density by 0.1.

Ground classification (262–279) rejects canonical air states, actual leaves and
logs tag members, brown/red mushroom blocks, then asks whether the actual
collision shape is empty. Native tag source/version/members are validated
before the existing tag helper is used. The host must query collision using
**ChunkAccess as BlockGetter and the original local position**, including its
neighbor-state wrapping behavior; a world-region query or solid/full-cube flag
is not equivalent. Unknown states, missing tag data or unavailable collision
owners fail.

The direct density getter's old bottom-cell case returns 0.1 **before** X/Z or
column checks, even for an interior point with no column. Other missing/outside
samples return exact `f64::MAX`. Runtime density columns start one cell above
that bottom and contain two cells per old section. Iteration preserves column
index order, then increasing cell Y, clipped to the supplied exclusive window.

Biome columns (252–258) call the actual chunk's stored noise-biome lookup at
local quart X/Z and each old-area quart Y. Native `B` is the actual provided
biome token; default `String` follows the existing closed canonical-biome-key
profile and does not select or invent a biome. Iteration checks vertical bounds
first and skips absent columns, preserving column order. It does not query a
raw generator or replace unavailable old biomes with a fallback.

## Native contract and checks

`OldChunkDataHost` has four mandatory fallible callbacks: canonical block state,
optional observed WORLD_SURFACE_WG first-available height, actual stored noise
biome, and collision-shape emptiness. `calculate_data` consumes the observed
new-generation Direction8Set and populates this existing mutable owner once.
The parent owns availability/status checks and all neighbor-age reads.

`get_height`, `get_density`, `iterate_heights`, `iterate_densities` and
`iterate_biomes` are read-only and infallible once the actual owner is available.
No callback allocates a random source or consumes RNG. Parent map traversal
must independently preserve original Fastutil encounter order; the data leaf
only preserves its own column order.

Focused checks cover persistent ordering and signed coordinates, lenient
optional versus wrong-length heights, exact sentinel/nonfinite retention,
saved-height reuse, one-time calculation, genuine biome failure after partial
mutations, and the legitimate no-selected-edge case. Runtime comparison of
original calculated columns and same-owner nonempty NoiseChunk output is the
separate behavioral evidence. This source module alone does not complete
legacy-world loading, public region collection, retrogen, status publication or
world-generation checklist delivery.
