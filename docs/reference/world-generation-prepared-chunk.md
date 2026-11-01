# Prepared chunk sampling and retained generation owners

`PreparedGeneration` creates the supported chunk's density and aquifer owners
before climate sampling, then carries those same owners through NOISE and
SURFACE. It is an additive native ownership contract. It does not select or
store biomes, create a BIOMES status, search for spawn, schedule chunks, perform
retrogen, or publish a FULL chunk.

## Original source boundary

The reference is the unchanged Java 1.21 server pinned by
`fixtures/source-lock.json`, inner SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence uses direct JDK21 `javap` and separately identified original-class
execution, not REA.

`NoiseBasedChunkGenerator.doCreateBiomes` obtains the chunk's existing
NoiseChunk and calls its cached climate sampler. The NoiseChunk constructor
creates the wrapped density functions and actual settings-selected aquifer
before that sampling. `NoiseChunk.cachedClimateSampler` (`dzc.a(dze,List)`,
mapped lines 212–218) wraps the six router fields in order: temperature,
vegetation/humidity, continents, erosion, depth and ridges/weirdness. Repeated
wrapping resolves to the existing chunk cache owners.

`Climate.Sampler.sample` (mapped lines 457–469) converts each quart coordinate
to blocks through Java's wrapping int shift by two, constructs a separate
SinglePointContext, and evaluates the six functions in that order. Each double
is narrowed to float before the existing float32 multiplication by 10,000 and
long narrowing. A climate target is not a selected biome or a stored biome
palette. The settings' spawn targets also serve other methods of the original
sampler; this API does not implement those methods.

The [prepared-climate evidence](world-generation-prepared-climate.md) records
the original constructor/sampler/traversal observations and their bounds.
The older [NoiseChunk ledger](world-generation-noise-chunk.md) and
[integration contract](world-generation-integration.md) retain the interpolation,
material and production boundaries.

## Additive API

`NoiseGenerator::prepare_chunk(self: &Arc<Self>, chunk_x, chunk_z, level_min_y,
level_height, context) -> Result<PreparedGeneration>` is strict. It requires a
positive, aligned intersection with the pinned noise grid and explicit actual
observations of initially empty block data, empty Blender and empty structure
beardifier. Existing coordinate and dimension-height limits remain enforced.
Preparation allocates neither a terrain image nor a completed generation status.

The private prepared value retains the exact `Arc<NoiseGenerator>`, the already
wrapped `ChunkDensity`, its grid/origin/traversal phase, and the real aquifer in
a movable Box. Noise-based aquifer caches or the settings' disabled aquifer are
constructed using the existing source algorithm and positional factory. There
is no stand-in fluid owner. Preparation does not consume gameplay Level RNG.

`PreparedGeneration::sample_climate(&mut self, quart_x, quart_y, quart_z)` returns
`ClimateTargetPoint` using the retained wrapped roots with external single-point
sampling and the existing float32 quantization. It does not start interpolation
or advance the NOISE traversal. Source marker caches can answer or retain their
normal external-context queries. The prepared value can outlive its local
caller binding and move into an owned job or another thread.

`PreparedGeneration::fill_noise(self) -> Result<OwnedNoiseTerrain>` consumes the
prepared value. A private restore operation checks exact graph identity and
moves the existing grid, origin, phase and density caches into the borrowing
traversal view. It does not call wrapping or density construction again. The
operation then runs the same material kernel used by the pre-existing
`NoiseGenerator::fill_noise` API, including interpolation, aquifer/ore/default
material order, WG heightmaps and fluid postprocessing. Success returns only
`TerrainStage::Noise`. Failure returns no partially generated image and the
consumed prepared job cannot be retried as a fresh traversal.

The existing borrowed and owned fill entry points retain their output semantics.
Their aquifer now remains with the returned terrain rather than being dropped
at the end of the material loop. The prepared path moves its original Box into
the result. SURFACE does not query the aquifer, but the owner and its actual
caches remain alive with the chunk stage.

`OwnedNoiseTerrain::sample_climate` samples through that same retained density
owner after NOISE, and can also sample after SURFACE. It is still only the six
climate functions. Owned SURFACE keeps its existing transactional image updates,
actual `SurfaceContext` requirement and duplicate-stage guard. Neither operation
creates a BIOMES record or another chunk owner.

`fill_from_noise` and `fill_from_noise_owned` retain the original early-return
`NoCells` path. Callers wanting that behavior continue to use those entry points
and keep their actual input ProtoChunk unchanged. `prepare_chunk` rejects a
no-cell range instead of constructing an empty prepared or terrain owner.

## Identity, phases and validation

All restored-state fields are private. The temporary graph identity is only an
in-memory equality check while the prepared Arc keeps that exact graph alive;
it is never dereferenced, persisted, or exposed as a public rebinding API.
Restoration preserves Ready, completed and failed traversal phases rather than
resetting a stopped or failed owner. There is no `unsafe` code, self-reference,
cache cloning, reseeding, or replacement aquifer construction between climate
sampling and the material traversal.

Focused native checks exercise warmed cache/preliminary-map identity, unchanged
interpolation counters during climate sampling, preserved aquifer allocation,
climate values before and after NOISE, equality with the existing material
kernel, thread/lifetime movement, graph rejection, strict preparation failures,
transactional SURFACE failure and duplicate rejection. Retained-state checks
also reject a second traversal after a completed or failed fill. Original/native
prepared-climate comparisons separately establish selected source behavior;
these checks do not turn fixture contexts into production BIOMES owners.

## Remaining production and stage contracts

The central coordinator still owns actual new ProtoChunk/BIOMES storage,
BiomeSource/BiomeManager selection and zoom seed, dependency and status
scheduling, dimension identity, persistence, lighting and runtime readiness.
Callers cannot infer an empty structure or blending context from a climate
sample or from the ability to construct this type. Native preparation is not a
new Main missing-chunk generation callsite.

The original CARVERS phase reuses this aquifer but passes a new SinglePointContext
to its material query. The current `MaterialContext` remains the source NOISE
context, including current-owner barrier sampling. Retaining the aquifer does
not implement the external carver material adapter, real carving/neighbor hosts,
features or structures. Those contexts remain unsupported until ported with
their actual owners and source call phases. This stage claims no FULL chunk,
complete world-generation delivery, checklist change or deployment.
