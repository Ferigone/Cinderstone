# Retained AIR carver invocation

This stage composes the native carver modules with the retained BIOMES → NOISE →
SURFACE job. `BiomeTerrain::carve_air(AirCarvingContext)` consumes that job and
returns `CarvedBiomeTerrain` only after the complete AIR invocation succeeds.
The result retains the same biome image, terrain, density/cache allocation,
aquifer and actual AIR mask. Its `TerrainStage::Carvers` describes native data;
it is not a ProtoChunk status, scheduler dependency certificate or FULL chunk.
Main does not acquire these owners or call this API in this stage.

The reference is the unchanged Minecraft Java 1.21 server, protocol 767, inner
SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Artifact and official mappings are pinned in `fixtures/source-lock.json`.
Inspection used direct Java 21 `javap -p -c`, not REA. Raw disassembly and Java
binaries remain outside the repository.

| Original method | Retained contract |
| --- | --- |
| `NoiseBasedChunkGenerator.applyCarvers` (`dzb.a`, lines 257–287), bytecode 0–32 | Construct the raw-source BiomeManager view, then exactly one WorldgenRandom wrapping a LegacyRandomSource seeded by the real RandomSupport process uniquifier and nanoTime. This occurs even with no configured carvers. |
| Same method, bytecode 38–118 | Reuse the target NoiseChunk/aquifer and actual generation-height accessor, then acquire the AIR mask before neighbor traversal. |
| Same method, bytecode 120–327 | Visit neighbor offsets X outer, Z inner, both −8 through 8. Select each actual region chunk before consulting its persistent carverBiome cache. |
| `ChunkAccess.carverBiome` (`duy.a`, lines 409–412) and generator lambda line 273 | Only a cache miss queries the generator source at neighbor minimum-block quart X/Z, quart Y zero, using the raw RandomState sampler, then resolves actual generation settings. A hit suppresses both callbacks. |
| `applyCarvers`, bytecode 200–318 | Iterate the actual AIR holder order, reset index to zero per neighbor, reseed with world seed plus index, test start, and share the post-start invocation stream with carve. Increment the index for every entry. |
| `WorldGenerationContext` (`dzv`, lines 10–20) | Generation minimum is max(accessor minimum, generator minimum); depth independently is min(accessor height, generator depth). These differ from the full chunk bounds used by the mask and image. |
| `ChunkStatusTasks.generateCarvers` (`dwb.h`, lines 89–95) | Install the actual Blender mask action before invoking AIR. The original task does not perform a second LIQUID pass. |

`AirCarvingContext` requires the actual level seed, zoom seed, worker-local biome
source and temperature owners, region neighbors, configuration registry,
generation-accessor/upgrade facts and AIR mask slot. The retained generator
checks the level seed against its own owner. Its private raw sampler evaluates
the same graph's raw router climate/erosion functions; it does not substitute the
wrapped chunk climate sampler or stored biome palettes. The existing persistent
multi-noise selector is borrowed from the actual worker. Its preceding search
history remains that worker's history, including BIOMES work actually run there.
A moved job does not carry or reconstruct another worker's selector.

Neighbor settings cache misses and zoomed carver top-material biome requests use
that same source/worker cursor in their actual interleaved order. The closed
`BuiltinCarverRegistry` is available only when the caller establishes that its
actual biome/configuration/tag registry is the pinned original registry. A
matching resource name does not establish that fact. Unknown configurations and
missing neighbors fail; they never become empty lists. An empty original End
list still visits all 289 neighbors and creates the actual process RNG. The
reported final 48-bit state is consequently clock dependent when no reseed
occurs; it is not replaced by zero.

`AirMaskOwner` binds the actual slot to the target chunk and full build minimum.
It distinguishes an observed absent slot from an unavailable owner. Existing
bits and the actual additional predicate move into the result. A Blender
`ObservedNoUpdate` performs no setter and preserves that predicate. An actual
`Install` action sets its provided filter before invocation RNG creation, and
may create the observed-absent mask at that point. Without an installation,
applyCarvers acquires/creates the observed-absent mask after RNG/context setup.
Neither route replaces unavailable state with a fresh false filter.

The target adapter mutates the same live terrain image and WG heightmaps. It
keeps postprocessing separate from writes so the shared material kernel applies
fluid marks in source order. It uses the retained aquifer with density zero and
external SinglePointContext sampling for every router query, including the
lazy barrier. The direct lava-anchor branch does not clear the aquifer's last
update flag. Ground repair queries the same seeded SurfaceSystem's top-material
rule against that live image, retained preliminary cache, raw-source biome view
and actual thermal owner. See the [material/mask ledger](world-generation-carver-material.md)
and [top-material ledger](world-generation-top-material.md).

Failure consumes the target job. Earlier mask bits, writes, aquifer flags,
neighbor settings caches, worker selector/thermal history and process RNG
creation may already have changed. No image-only rollback or retryable SURFACE
result is promised. Success advances only after all 289 neighbors finish;
duplicate SURFACE or AIR application is rejected by the private native stage.

The current supported boundary remains a fresh prepared chunk with observed
empty Blender/structure influence and observed absent retrogen. The CARVERS
adapter requires the actual generation accessor to match its retained full
chunk bounds and rejects upgrading/retrogen. Built-in biome selection, thermal
data, configured carvers, providers and replaceable tags are pinned subsets;
custom registry overrides and nonempty blending/retrogen are unsupported.
Actual scheduler requirements remain central integration work: parent SURFACE
at radius zero, STRUCTURE_STARTS dependencies through radius eight, source phase
ownership, actual region/cache/mask persistence, cancellation and publication.
Successful queried neighbor reads do not certify those prerequisites. Features,
lighting, spawn work, persisted statuses and Main/FULL delivery are not provided.

Focused native checks cover cache hit suppression and failed-miss history,
additional-mask preservation and install ordering, exact all-neighbor traversal
with real empty End lists, and dependency failure after earlier cache effects.
A retained End owner check observes unchanged density/aquifer/image allocations
through the empty invocation and rejects duplicate stages. Its surface setup is
an explicit source-derived test context, not evidence of natural BIOMES staging.
Compiler and unchanged-original material/pipeline results are recorded by the
coordinating validation stage; test quantities are not completion evidence.
