# Native aquifer source ledger

This stage ports the built-in water/lava/air aquifer material decisions in
`src/worldgen/aquifer.rs`. It does not select biomes, publish chunks, schedule
runtime fluid ticks or advance generation status. The consumer must use the
returned fluid-update flag at the original postprocessing boundary.

The reference is the unchanged official Java 1.21 server from
`fixtures/source-lock.json`: outer SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
and official server mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`. Inspection used direct
`java --module jdk.jdeps/com.sun.tools.javap.Main -p -c`, not REA. No raw
disassembly or Java binaries are included in the repository.

| Source method and mapped class | Preserved native behavior |
| --- | --- |
| `Aquifer.createDisabled`, `Aquifer$1.computeSubstance` (`dyk`, `dyk$1`, source lines 61–73) | Positive density selects the default solid; nonpositive density uses the actual global picker; fluid updates remain false. |
| `Aquifer.FluidStatus.at` (`dyk$b`, 677–683) | Fluid exists strictly below its stored level; the level itself is air. |
| `NoiseBasedAquifer` constructor/getIndex (`dyk$c`, 156–192) | Chunk-local cache extent uses 16×12×16 grid separation plus the original border cells; fluid statuses and positional centers have separate caches. |
| `computeSubstance` (`dyk$c`, 201–359) | Source 2×3×2 center search order, bounded samples 10/9/10, equal-distance replacement order, three nearest statuses, density/pressure exits, global lava and water-over-lava exits, and the original update-similarity threshold. |
| `calculatePressure` (`dyk$c`, 388–472) | Water/lava pressure, vertical midpoint/shape arithmetic, lazy barrier evaluation shared among the three pair checks, and source multiplication order. |
| `getAquiferStatus`, `computeFluid` (`dyk$c`, 488–556) | Cached center fluid state and the ordered thirteen-column preliminary-surface probes, including early returns and the source integer-overflow behavior of the no-surface sentinel. |
| `computeSurfaceLevel` (`dyk$c`, 560–606) | Deep-dark rejection, floodedness thresholds, real nearby-fluid evidence, and randomized versus global/dry surface selection. |
| `computeRandomizedFluidSurfaceLevel`, `computeFluidType` (`dyk$c`, 615–662) | Floor-divided coordinates, 40-block fluid bands, three-block spread quantization, lava threshold and source dry-level sentinel. |
| `OverworldBiomeBuilder.isDeepDarkRegion` (`dem`, 608) | Original float constants widened to double and short-circuit erosion/depth evaluation. |
| `DimensionType` constants (`dwy`) | `WAY_BELOW_MIN_Y = -2032 << 4 = -32512`, distinct from the generator's minimum Y. |

`AquiferContext` deliberately separates the current NoiseChunk barrier query
from the other router samples. Source pressure receives the original current
function context. Floodedness, erosion/depth, spread and lava are sampled using
new `SinglePointContext` coordinates. The generator adapter binds these to
`ChunkDensity::sample` and `sample_external`, respectively, and supplies the
same chunk's actual `preliminary_surface_level` cache. An unknown surface query
must fail; a constant height is not an aquifer implementation.

The positional factory comes from the same RandomState root as the router:
`root.fromHashOf("minecraft:aquifer").forkPositional()`. Cache-center sampling
never consumes the gameplay Level RNG or another generator's random stream.
The production global picker remains a caller-owned dependency. The built-in
adapter mirrors `NoiseBasedChunkGenerator.createFluidPicker` (`dzb`, 71–85):
lava level −54 below `min(-54, seaLevel)`, otherwise the actual settings fluid
and sea level. The source's unused air status is not substituted for a picker.

Custom fluids outside canonical air/water/lava, inaccessible BlockPos ranges,
unknown dependency owners and production chunk publication are unsupported.
Aquifer results alone do not establish the complete `world_generation.3`
delivery or Main integration.

The focused aquifer probe executed the unchanged original `NoiseChunk` public
constructor with seed 12345, built-in Overworld settings, chunk (−1, 2), the
original empty Blender/beardifier marker and the original generator fluid
picker obtained by reflection. It then called the original aquifer with recorded
positive and negative densities and original `SinglePointContext` positions.
The native result matched every material and fluid-update flag in that ordered
probe, including pressure-induced solid results, air, water and global lava.
`fixtures/world-generation-aquifer-vectors.json` retains the source artifact and
probe hashes plus the ordered results. The in-module focused check replays
those observations with the same router and preliminary-surface queries. This
isolated oracle execution is separate from the generator's interpolation-loop
comparison and does not establish Main integration.

A targeted arithmetic audit found a difference outside the bounded built-in
noise values: Rust `floor()` followed by an integer cast differs from the
original `Mth.floor(double)` for values below the integer range. The original
casts first and then conditionally decrements the integer with wrapping. Spread
quantization now preserves that order. Direct reflection into the unchanged
original `Mth.quantize(double, int)` (`ayo.a`, divisor 3) observed −Infinity and
−1e20 produce `2147483645`, while floor-then-cast produced `−2147483648`.
`fixtures/world-generation-aquifer-quantization.json` records the actual source
results, including ordinary values, integer conversion limits, adjacent IEEE
values, signed zero, NaN and infinities. Inputs use bit strings so the fixture
contains no nonfinite JSON numbers. This correction preserves the public
context's Java arithmetic; it is not evidence of an error in the selected
bounded built-in terrain comparisons or a broader completion claim.
