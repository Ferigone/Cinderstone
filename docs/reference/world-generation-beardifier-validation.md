# Ordered Beardifier validation

Target: unchanged original Minecraft Java **1.21**, protocol **767**, inner
server SHA-256 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
The checker verifies the outer server and official mappings against the source
lock, verifies the bundled inner identity and hashes every declared library.
Source inspection uses direct Java 21 javap, not REA Evidence.

| Original mapped owner | Validation responsibility |
| --- | --- |
| Beardifier constructor (dyl, 81–84) and Rigid record (dyl$a, 33) | Construct actual original objects with the declared rigid and junction arrays in encounter order. |
| Beardifier.compute (dyl, 88–132) | Compare raw scalar bits, including iterator rewind and repeated positions, against native ordered parts. |
| Beardifier.minValue/maxValue (138, 144) | Record both original infinite bounds as IEEE bit strings. No constant-bound replacement stands in for a real empty owner. |
| Bury/beard helpers and kernel (149–186) | Selected point vectors cover all five actual TerrainAdjustment modes, box faces, vertical offsets, kernel cutoffs, a far-zero position and a repeated point. |
| JigsawJunction (eku, 14–31) | Supply actual original source coordinates; the Beardifier reads sourceX, sourceGroundY and sourceZ. |
| NoiseChunk constructor and beardifier field (dzc, 42–209) | Observe the actual installed owner through private field s after BIOMES, NOISE and SURFACE. |
| NoiseBasedChunkGenerator.doCreateBiomes/fillFromNoise/buildSurface (dzb, 98–103, 291–413, 247–250) | Continue the same controlled original target and cached owners through the existing full-phase oracle. |

The declared JSON contains a beardifier object with ordered rigids and junctions.
Each rigid has six box coordinates in minX/minY/minZ/maxX/maxY/maxZ order,
terrain_adjustment and ground_level_delta. Each junction has source_x,
source_ground_y and source_z. The original helper creates a real BoundingBox,
Rigid and JigsawJunction, then passes their actual ordered list iterators to
the unchanged Beardifier constructor. Bounding coordinates must be ordered
and integer fields fit signed 32-bit values. Adjustment names are resolved
against the original enum's serialized names.

Original junction deltaY zero and destination projection RIGID are explicit
fixture fields unused by Beardifier.compute. They do not represent completed
jigsaw placement or a reconstructed template. An absent beardifier payload
constructs an actual empty original owner; it does not supply the raw marker.
The report checks actual class dyl, its loaded JAR identity and the selected
ordered/empty input kind.

[WorldgenBeardifierOracle.java](../../src/worldgen/validation/WorldgenBeardifierOracle.java)
dispatches scalar queries to the shared original ingress and full phase queries
to the existing-target oracle. Scalar vectors separately exercise NONE, BURY,
BEARD_THIN, BEARD_BOX and ENCAPSULATE, junction-only data and ordered mixed data.
No expected native result is embedded in the inputs. Raw IEEE bits preserve
signed zero and avoid nonfinite JSON numbers. The repeated sample observes the
original iterators' rewind rather than assuming the owner is stateless.

[check_beardifier.py](../../src/worldgen/validation/check_beardifier.py)
adds four coherent full pipelines: actual empty control, rigid-only,
junction-only and combined. They share the same declared existing-target
blocks, old registered palettes, explicit foreign palettes and section queues.
The checker compares every physical block and biome array, WG first-available
values, section counters, statuses/accessors, unsaved/debug fields and ordered
posts/allocation at INITIAL, BIOMES, pre-continuation NOISE, NOISE and SURFACE.
It requires positive material/SURFACE work, and each nonempty source profile
must actually differ from the empty control in both final phase images.

Java executes first. Native scalar queries receive only declared inputs;
native full pipelines receive only their query and actual original INITIAL.
No later expected image is passed to native. Original retained-owner identity
and native captured-Arc identity are observed separately after each phase.
SURFACE keeps the previously installed owner. An uncached factory's structure
selection and unconditional public-phase Blender collection have distinct
source contracts; this fixture does not simulate a WorldGenRegion.

This evidence begins with already selected ordered data. It does not execute
Beardifier.forStructuresInChunk (dyl, 39–77), construct a StructureManager,
generate starts/references, or place templates/processors. Native structure
selection evidence belongs to its separate source-backed owner and report;
these phase comparisons alone do not establish a complete STRUCTURES stage.

The coordinator builds the current native library separately. A fresh external
replay uses:

    python3 src/worldgen/validation/check_beardifier.py \
      --oracle-dir /path/to/pinned-oracle \
      --work-dir /path/outside/checkout/new-empty-beardifier-work \
      --target-dir /path/to/explicit-native-build-target \
      --skip-build \
      --runtime-report /path/outside/checkout/new-beardifier-runtime.json \
      --report /path/outside/checkout/new-beardifier-validation.json

The runner reuses the existing artifact, process, snapshot, comparison and
sanitization helpers. It never invokes Cargo. Java 21 and the matching Rust
compiler are required; --java-home and --rustc select installed tools. Each
child has a 55-second timeout plus five seconds of TERM grace before KILL/reap.
Raw inputs, outputs and process records remain external. Final JSON is written
only after all selected comparisons and positive guards pass.

## Accepted original/native execution

The accepted run used native source **1457a89**. Original Java completed in
**6.739 seconds** and native execution in **2.070 seconds**; both exited zero,
stayed within their bounds and were reaped. Exact source, artifact, bootstrap,
compiled probe and raw transcript hashes are retained in the runtime and
validation records.

All selected scalar and min/max raw bits matched for NONE, BURY, BEARD_THIN,
BEARD_BOX, ENCAPSULATE, junction-only and ordered mixed inputs, including the
boundary and repeated-point observations. Each of the four complete pipelines
matched physical blocks, biome palettes, WG values, counters, statuses/accessors,
ordered post lists and allocation flags at all five snapshots. Original
NoiseChunk, aquifer, Blender, marker-cache and sampler identities were retained.
The actual original Beardifier and the separately observed native captured Arc
were retained after BIOMES, NOISE and SURFACE; native section leases were
released. These observations do not expose native private NoiseChunk/aquifer
pointer identity or original acquisition order.

| Declared ordered profile | NOISE mutations | Subsequent SURFACE mutations | NOISE differences against empty owner | Final SURFACE differences against empty owner |
| --- | --- | --- | --- | --- |
| Actual empty control | 28,627 | 15,749 | 0 | 0 |
| Rigid-only | 31,759 | 15,749 | 5,145 | 6,425 |
| Junction-only | 28,698 | 15,820 | 71 | 71 |
| Combined | 31,677 | 15,749 | 5,063 | 6,343 |

These are actual full-image differences, not inferred scalar effects. All
profiles also made actual foreign stored-biome queries during SURFACE. Every
selected SURFACE-only postprocessing delta was **zero**, with the earlier
complete queues and allocation states preserved. This does not prove a positive
SURFACE-only insertion branch.

The accepted profile remains controlled ordered-data ingress. It does not
execute source-manager selection, generate historical structures, establish
Main generation or scheduler behavior, persist chunks, or publish FULL data.
The separate selection and complete STRUCTURES limitations above are unchanged.
