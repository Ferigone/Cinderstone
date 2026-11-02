# Nonempty Blender observations

Calculated old-generation snapshots change native BIOMES and NOISE through the
retained Blender. In the selected controlled cases, the original and native
implementations agree on calculated columns, iterator/map order, double bits,
lazy biome resolution, wrapped bounds and fills, complete biome palettes and
complete NOISE block arrays, WG heightmaps and postprocessing lists. The same
matrix includes the actual EMPTY singleton and a constructed zero-map Blender
as distinct controls. These observations do not complete a server generation
status or Main integration.

The target is the unchanged Minecraft Java 1.21 server, protocol 767, inner
SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The checker verifies the outer server and official mappings against
`fixtures/source-lock.json`, then verifies the actually loaded Blender
CodeSource. Inspection used direct Java 21 `javap` and original execution. REA
tools were unavailable; no REA Evidence is claimed. Both reports identify the
native source/composed commits and hash the implementation, dependencies and
probes. A pinned-source run rejects changed production blobs before execution.

| Original source | Actual observed boundary |
| --- | --- |
| `BlendingData` (`eab`), constructor lines 100–113 and `calculateData` lines 142–180 | Construct original old-generation section bounds and calculate actual selected boundary columns once. A second calculation performs no material/biome reads. |
| `sideByGenerationAge` (`eab.a`, lines 129–138) | Query all eight actual supplied neighbor owners in Direction8 order through a rejecting WorldGenLevel interface proxy. Old age is the real BlendingData-slot presence on those owners. |
| Data height/density/biome helpers, lines 183–368 | Read original section palettes, primed WORLD_SURFACE_WG, collision shapes and generated biome columns. Capture raw column bits, valid local-quart getter results and ordered height/density/biome iteration. |
| `Blender` (`eaa`), map constructor lines 113–116 | Retain actual calculated data in actual Fastutil maps. Map entries follow the source-reviewed collection scan; this is not execution of `of(WorldGenRegion)`. |
| Height/offset and density functions, lines 121–257 | Compare raw doubles at direct, weighted, missing, negative-Y and range-boundary points. The private original `heightToOffset` is also executed on negative/remainder-boundary inputs. |
| Biome functions, lines 261–304 | Keep the actual nearest-old tie order and fixed source shift noise. Old success suppresses fallback; fallback receives unchanged quart coordinates and the same sampler. |
| `NoiseChunk.wrapNew` and blending wrappers (`dzc`, mapped lines 773–780, 783–857, 868–895) | Preserve singleton-sensitive substitution, prefilled quart caches, shared fallback pair, owner versus external density context, visited bounds and provider fills. |
| Actual cached climate sampler and public `fillFromNoise` | Fill full target palettes through the actual blended resolver, then fill NOISE using that job's retained NoiseChunk, aquifer and Blender. The original future returns the same target; its status stays EMPTY. |

The old owners are explicit controlled `ChunkAccess` snapshots. Their original
base storage contains real section palettes, heightmaps and canonical states.
Their biome palettes are filled by original biome-source methods. Most use the
same persistent raw multi-noise worker as later fallback queries. A few provide
explicit original FixedBiomeSource inputs with different canonical biomes to
make equal-distance ownership observable. Those fixed sources are declared
fixture inputs, not a fallback or an expected native answer supplied to Java.
The native fixture independently builds the corresponding palettes and preserves
that worker's encounter history across the recorded sequence.

Every snapshot reports EMPTY. Its base ChunkAccess biome getter reads the
already supplied palettes; no ProtoChunk is relabeled BIOMES to bypass its real
status guard. Unsupported snapshot operations throw. The material profiles are
explicit grass/dirt/stone columns with cavities and water, not a reconstruction
of historical natural terrain. Collision emptiness is queried from original
canonical states; the native fixture requires the corresponding real static
collision data and fails when that shape owner is unavailable.

The concrete public `Blender.of(WorldGenRegion)` and
`BlendingData.getOrUpdateBlendingData` entry points are **not executed** here.
WorldGenRegion requires a real ServerLevel, level data, RandomState and dimension
owner; an interface proxy is not that object. The probe instead executes actual
calculation and the original map constructor over the controlled snapshots.
Its collection adapter preserves the inspected X-major/Z-minor radius-seven
circular scan and radius-one density subset, but does not certify storage-age
scanning, dependency availability, highest-generated status or BelowZeroRetrogen.
Those remain actual host/central integration contracts. In particular, these
EMPTY snapshots would not themselves satisfy the public collector's BIOMES gate.

The corner case has equally near old plains and badlands samples. The original
Fastutil encounter order selects badlands; native selection matches. Multiple
old heights also exercise accumulation order rather than a sorted approximation.
A separate original Fastutil witness captures the full 193-key collection
stencil, growth from 32 through 512 slots, negative and zero keys, collisions and
replacement retaining slot order and the actual value owner. Its observations
are retained under `map_witnesses`. The native probe includes the exact
production `ordered_map.rs` by path and compares primitive Arc value owners at
the same insertion thresholds, including collisions and replacement identity.
Original capacity is observed directly; native capacity is not inspected or
guessed. The separate Blender comparisons use actual calculated data owners;
primitive map witness values are not substituted for terrain or biome data.

The component wrapper probe and the full generation job are separate real
owners sharing the same immutable Blender. Component queries intentionally warm
its cache; they cannot contaminate the independent full-job witness. That job
retains its own actual owner from blended BIOMES through NOISE. Original
NoiseChunk/aquifer/Blender identity is observed directly; the native prepared
Blender handle and actual callback owner are checked before consuming the typed
stage into NOISE.

A far-coordinate case makes the original fallback-key sentinel observable.
Its first query at `(1875066,0,1875066)` returns the initial `(1,0)` pair. After a
different fallback query, returning to that same point produces the nonempty
pair. Alpha and offset share that last-position/output owner. Grid edges also
exercise quart quantization and fallback outside the prefilled window. The
source's raw last-key field is retained as an original observation; the native
comparison uses observable values, not a manufactured private-field report.

Custom roots use the original DensityFunction codec and actual NoiseChunk
visitor, with native parsing of the same input descriptions: blend alpha,
blend offset, density around a constant, MIN with alpha, MAX with offset, and a
single-point alpha spline. Raw versus visited bounds, scalar samples, external
fills and provider-index encounter order must agree. A high old surface makes
MAX's visited offset bounds observable. Constructed zero maps still use wrapper
bounds and index visits, whereas EMPTY keeps raw constants. The owner-context
density result differs from the external SinglePointContext result even though
both receive the same coordinate and raw value. This also exercises the case
where a constant child still requires every transform provider index.

The full native output must equal the original complete 384×16×16 canonical
NOISE array, complete target biome palettes, both WG maps and every packed
postprocessing section list in insertion order. Nonempty west/high/corner cases
must change actual original terrain compared with the EMPTY control. Successful
identity controls, isolated function values or numeric test quantities cannot
replace those material-effect guards. Signed primitive values are compared by
raw double bits, including infinities in function bounds.

Run with Java 21 and Rust, keeping artifacts/classes/arrays outside the checkout:

```sh
python3 src/worldgen/validation/check_blending.py \
  --oracle-dir /path/to/pinned-oracle \
  --work-dir /path/outside/checkout/blending-check \
  --native-source-commit <verified-implementation-commit> \
  --runtime-report fixtures/world-generation-blending-runtime.json \
  --report fixtures/world-generation-blending-validation.json
```

The checker explicitly builds the current library before compiling its native
probe. `--skip-build` requires a separately completed `cargo build --lib` of the
same source and target; a compiler-only check or a stale top-level rlib is
insufficient. A child class loader contains the unsigned snapshot classes while
the signed original JAR remains unchanged in its parent. Published reports
contain sanitized observations and hashes; no Java binaries, private saves,
raw disassembly or machine paths are committed.

This matrix does not cover actual region collection, retrogen, existing target
blocks, structures, mixed-age SURFACE recollection, old/new carving-mask filters,
border ticks, complete FEATURES, lighting, persistence or FULL publication.
The public native generator's explicit blended context is callable; its real
region/status/dependency and Main owners remain separate integration work.
