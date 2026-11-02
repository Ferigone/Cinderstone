# Nonempty Blender and retained NoiseChunk composition

Target: Minecraft Java 1.21, protocol767, pinned by `fixtures/source-lock.json`.
The original inner JAR SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Source evidence is direct Java21 `javap` with official mappings and separately
identified original runtime observations. No REA execution is claimed.

The source boundary is `Blender` (`eaa`), `BlendingData` (`eab`),
`NoiseChunk` (`dzc`) and `NoiseBasedChunkGenerator` (`dzb`). Their native
counterparts retain calculated old data through a real Blender Arc, the paired
alpha/offset caches and the same prepared density/aquifer job. The fresh entry
continues to require actual observed-empty inputs. Nonempty blending is an
additive entry requiring an actual retained Blender; neither a missing owner
nor a familiar world seed establishes that observation.

## Old-source collection and ownership

`Blender::from_region(&mut impl BlendingRegionHost)` ports `Blender.of`, mapped
lines76–110. The mandatory old-around-radius7 guard belongs to the real
ServerLevel/chunk-storage/IOWorker path. It cannot be inferred from loaded
BlendingData map size. The source cached on-disk scan identifies old saved data
from DataVersion/blending metadata; that storage owner remains external.

After a positive guard the source scans X then Z, both -7 through7, keeping
`dx*dx+dz*dz<=64`. These are193 candidate positions. Each source chunk/data slot
is read before its actual highest-generated-status check; a missing data slot
skips that status observation. Qualifying data queries all eight Direction8
neighbors in enum order, including when its calculated flag was already set.
These reads can require radius8 dependencies. Actual ChunkAccess old-generation
membership is its BlendingData-slot presence, distinct from the storage guard.

The host then calculates the same actual old-data owner and returns its retained
immutable Arc. Native `is_calculated` checks calculation state; it is not a
certificate of storage provenance, status readiness or absent retrogen. Height/
biome and density maps share that same Arc. The source density map contains only
the center radius1 square; direct lookup for both heights and density instead
uses the broad height/biome map. Lookup precedence is current, northwest when
both quart boundaries match, west, then north, stopping at the first value other
than exact Double.MAX_VALUE. Missing actual dependencies fail, separately from
source-observed absent slots, out-of-range samples and absent WG maps.

The maps preserve Fastutil8.5.12 default long-object map hashing, linear probing,
zero-key treatment, resize threshold, descending rehash and zero-first/descending
forEach order. This preserves weighted double accumulation and first strict
nearest-biome ties. Sorted maps or insertion-order vectors are not equivalent.
`from_calculated_data` accepts original insertion-order `(packedChunk, Arc<Data>)`
entries for the actual map constructor. Even zero entries retain a non-singleton
Blender identity. `from_region` has the source's separate both-maps-empty rule,
returning the actual `Blender::empty()` singleton. The constructor validates
calculated owners without inventing sample values or world observations.

## Callable generation entries

`BlendedGenerationContext` requires observed-empty new target blocks, an
observed-empty structure beardifier and the actual Blender Arc. Existing target
blocks and present/unavailable target BelowZeroRetrogen remain unsupported. The
original source can also handle additional contexts; this boundary does not
manufacture those owners.

The additive generator entries are `prepare_chunk_with_blender`,
`fill_noise_with_blender`, `fill_from_noise_with_blender` and
`fill_from_noise_owned_with_blender`. The fillFromNoise variants preserve the
source early no-cell return before checking dependency observations, and return
no new image or completed status then. Positive aligned fills use
`NoiseChunk::new_with_blender` and the same actual aquifer construction.

The NoiseChunk pair cache is initialized before router wrapping and aquifer
creation. Its two FlatCache grids share source quart positions; outside that
grid they share a last exact-block-X/Z key and pair. Prefill does not warm the
fallback cache. The invalid packed-coordinate sentinel and initial `(1,0)` pair
remain as in the source. Wrapper replacement checks actual EMPTY identity.
Nonempty wrappers also change function bounds and array-provider behavior;
those are ported in `density.rs`/`density/chunk.rs`, not approximated by replacing
scalar values alone. See `world-generation-blending-wrappers.md`.

`PreparedGeneration::blender()` clones only the retained handle after checking
graph identity. `PreparedBiomes::fill` wraps the actual supplied resolver with
that owner. Selecting an old stored biome avoids the fallback climate/source
call; otherwise coordinates and the same prepared sampler are forwarded once.
The helper's fixed seed42 shift noise is separate from the process-global seed
factory and world/Level streams. BIOMES, NOISE, aquifer/preliminary reads and
SURFACE keep the same density cache owner when the job moves. SinglePointContext
density blending remains source EMPTY, while captured alpha/offset wrappers
continue to use the actual retained Blender.

Public source phase entry points also have region-side collection effects.
In particular, public `buildSurface` recollects `Blender.of(region)` before
getOrCreateNoiseChunk even if the chunk already retains its first NoiseChunk.
The retained numeric owner is then reused. A production phase adapter must
perform those actual region/storage observations in source order; invoking
the owned-data SURFACE method alone is not proof of that public callback path.
The same applies when an already-calculated old data owner still requires the
collector's eight neighbor-age reads. The current runtime comparisons identify
these collection/phase limits separately.

## Evidence and production limits

Focused native compilation/checks and paired original execution are recorded in
the dedicated blending validation note and fixtures. Their controlled original
ChunkAccess snapshots retain actual palettes, metadata and heightmaps without
marking an EMPTY ProtoChunk as BIOMES. Actual BlendingData calculation, Blender
map construction/functions, NoiseChunk wrappers and full NOISE images can be
observed through those owners. The public concrete WorldGenRegion constructor
requires a genuine initialized ServerLevel; no such harness is supplied by an
interface proxy or fabricated object. Public `Blender.of(region)` runtime
coverage remains explicit, with native collection source and mandatory host
ordering reviewed separately.

The production Main job store, level/storage guard, actual chunk dependency
providers, retrogen and structure owners, carving-mask/border callbacks,
FEATURES/LIGHT/SPAWN/FULL conversion, persistence and readiness remain separate
contracts on issue43 and the relevant work packages. Successful numeric/image
comparison does not certify those stages or change the fixed checklist. No
flat fallback, fabricated completed chunk, duplicate RNG or shared runtime owner
is introduced here.
