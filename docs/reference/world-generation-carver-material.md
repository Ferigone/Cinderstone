# Shared carver material and mask ownership

The reference is the unchanged Java 1.21 server, protocol 767, inner SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The outer artifact and official mappings are pinned by `fixtures/source-lock.json`.
Inspection used direct Java 21 `javap`, not REA. No raw disassembly or proprietary
Java binaries are included in the repository.

`carver/world_carver.rs` is the single native `WorldCarver` ellipsoid and material
kernel used by cave, Nether cave and canyon geometry. It owns no sequential RNG:
the original `carveEllipsoid` and `carveBlock` receive no random argument. The
configured geometry owners retain their actual per-invocation/tunnel streams.
`GENERATION_RANGE` is the source `WorldCarver.getRange()` value 4. `can_reach`
preserves the float additions of thickness, 2 and 16 before widening, followed by
the original double squared-distance/subtraction association.

| Original method | Preserved behavior |
| --- | --- |
| `WorldCarver.carveEllipsoid` (`ebe.a`, mapped lines 70–129) | Actual target chunk center, horizontal distance rejection, source floor/int wrapping and clipping, upgrading-dependent top margin, X-major/Z-minor/Y-descending traversal, X/Z `+0.5` and Y `−0.5` normalized coordinates, and strict exclusion of the lower clipped Y. |
| `WorldCarver.carveEllipsoid`, same method | The actual geometry skip checker runs before mask lookup. Mask lookup runs before a debug-mode bypass. A selected bit is set before material callbacks, including attempts that later return false or no material. The success accumulation does not short-circuit later callbacks. |
| `WorldCarver.carveBlock` (`ebe.a`, lines 133–167) | Read the live state, mark the per-column reached-surface flag for grass/mycelium before replaceability, evaluate the real configured HolderSet, decide material, write it, then handle the source fluid and ground-repair callbacks. |
| `WorldCarver.getCarveState` (`ebe.a`, lines 176–187) | At or below the resolved configured lava anchor, return canonical lava without an aquifer call or debug remapping. Otherwise use the retained aquifer with density 0 and an external SinglePointContext. An empty decision remains empty outside debug mode. |
| `WorldCarver.getDebugState` (`ebe.b`, lines 191–202) | Remap actual air/water/lava block types to the supplied canonical debug states, setting WATERLOGGED when the chosen debug water state has that property. Other states retain their identity. |
| `NetherWorldCarver.carveBlock` (`ebd.a`, lines 44–54) | Separate override: require replaceability, write lava at or below `minGenY + 31`, otherwise cave air. It bypasses aquifer, debug material mapping, reached-surface tracking, ground repair and fluid postprocessing. |
| `CarvingMask` (`dux`, lines 13–55) | Independent stored BitSet and additional mask, original packed index, filter-before-bit lookup, trimmed long-array persistence and ascending stored-bit streaming. |

The base material callback writes the replacement before reading the aquifer's
update flag and, only when that flag is true, testing its fluid state and marking
that position for postprocessing. The direct lava-anchor branch deliberately
retains the aquifer's preceding update flag. Reconstructing or clearing that
owner would change this callback sequence.

Each ellipsoid X/Z column has one reached-surface flag, initially false. Once
set it remains true during that column's descending visits, including failed
replacement attempts. After a successful primary write, a reached column checks
the actual block below for dirt. It calls the same seeded SurfaceSystem's
`top_material` with that lower position and the fluid state of the replacement
above. An optional result is written below, then marked whenever that result
contains fluid, independently of the aquifer flag. `None` leaves the dirt alone.
The [top-material ledger](world-generation-top-material.md) records that query.

`CarverHost` requires the actual image and generation bounds, upgrading flag,
canonical state fluids, retained aquifer, actual source biome function and
postprocessing owner. Its block writes update the live heightmaps without
implicitly marking fluid positions: the shared material kernel supplies those
marks in source order. Original ProtoChunk storage groups packed postprocessing
positions by section; duplicates and insertion order within each section must
survive publication. A queued postprocessing position is not an immediate
runtime fluid tick.

The original `CarvingMask` belongs to the actual ProtoChunk and carving step.
Its index is `(x & 15) | ((z & 15) << 4) | ((y - minY) << 8)`, with source integer
wrapping. Its `minY` is the chunk build minimum, which may differ from the
CarvingContext generation minimum. Capacity does not clip later BitSet writes.
The additional filter receives the original supplied coordinates first; a true
result bypasses even an otherwise invalid negative stored-bit index. Setting a
bit does not query that filter. Serialization and streaming contain only the
stored bits, never the additional filter. Fresh construction has the original
false additional predicate; loaded/old-generation contexts must retain their
real mask/filter owners rather than substitute that fresh state.

`NoiseBasedChunkGenerator.applyCarvers` (`dzb.a`, lines 257–287) reuses the chunk's
NoiseChunk and aquifer. Its biome function is a BiomeManager with the generator's
NoiseBiomeSource and raw RandomState sampler, not an unrelated stored-biome
lookup. Future/production adapters must retain that actual source and zoom
identity. The existing NOISE aquifer adapter is insufficient here because every
carver density query, including lazy barrier noise, uses SinglePointContext.

Kernel errors can occur after a mask bit, write, aquifer flag or provider cache
has changed. The surrounding generation job must be consumed or poisoned on
failure. Restoring only an image does not restore all those source owners. This
kernel alone does not assert successful CARVERS scheduling, completion of all
neighbor/configured-carver passes, persistence, Main integration or FULL status.

Focused native mask checks cover additional-filter order, stored bits beneath
that filter, coordinate aliases, trimmed persistence and ascending stored-bit
positions. Original configured-carver and end-to-end staged observations belong
to the coordinated validation report; no compiler/runtime result is inferred
from the presence of this source or its checks.
