# Staged BIOMES image over the prepared chunk owner

This module computes actual quart-biome data using the retained prepared sampler,
keeps it with that owner, and moves both into NOISE/SURFACE terrain. It does not
create an original ProtoChunk status, satisfy generation dependency radii, write
Anvil or packets, or publish a FULL chunk. Nonempty Blender and BelowZeroRetrogen
remain unsupported.

## Original source and representation

The reference is the pinned unchanged Java 1.21 server, inner SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
with official mappings from `fixtures/source-lock.json`. Direct `javap` supplied
the source observations; actual original-class pipeline observations are
separately recorded by the BIOMES validation tooling. No REA execution is claimed.

`ChunkAccess.fillBiomesFromNoise` (`duy`, mapped lines 432–441) iterates the actual
generation accessor's sections in ascending section Y. Each
`LevelChunkSection.fillBiomesFromNoise` (`dvj`, lines 188–199) calls the resolver
with local quart X outermost, local quart Y next, and local quart Z innermost.
Each query receives global quart coordinates and the actual cached climate
sampler. The section's completed palette replaces its prior palette after its
64 successful queries.

Palette layout differs from that callback order. The original
`PalettedContainer.Strategy.SECTION_BIOMES` index is
`localY * 16 + localZ * 4 + localX`. Native `BiomeImage::values()` uses ascending
stored quart Y, then local quart Z and X. Its full section span is:

- `minSection = floorDiv(levelMinY, 16)`;
- `maxSection = floorDiv(levelMinY + levelHeight - 1, 16) + 1` (exclusive);
- `storageMinQuartY = minSection * 4`;
- `storageQuartHeight = (maxSection - minSection) * 4`.

The image therefore includes every cell of every intersecting section, even for
an unaligned accessor. It never truncates a partial section or invents its cells.
All cells come from successful resolver calls; unresolved private construction
slots are never exposed or replaced with a constant biome.

The image uses the full level height supplied to the prepared owner, not the
clipped noise grid. A Nether or End level accessor of height 256 has sixteen
BIOMES sections even when its noise settings fill only the lower 128 blocks.
Retrogen absence makes the generation accessor the ordinary full chunk accessor;
its absence must still be an actual caller observation.

## Resolver and ownership API

`BiomeResolver::get_noise_biome(qx, qy, qz, &mut PreparedGeneration)` receives the
same actual mutable sampler on every query. The direct implementation for
`BuiltinMultiNoiseBiomeSource` preserves the worker's existing RTree search
cursor across chunks. `BuiltinBiomeResolver::MultiNoise(&mut source)` makes that
borrow explicit; `BuiltinBiomeResolver::End` uses the retained raw erosion path.
Neither variant infers a preset from the dimension or noise-settings key. Pinned
built-in registry contents remain a caller contract; a familiar registry key
alone does not detect a datapack override.

`PreparedBiomes::fill(prepared, RetrogenContext::ObservedAbsent, &mut resolver)`
consumes a single generation attempt and returns a private pairing of the
prepared sampler/aquifer owner and complete `BiomeImage`. `Present` and
`Unavailable` retrogen are errors before the first resolver query. Height or
source failure returns no partial image. Source cursor and density-cache effects
are not rolled back, cloned or reset: a failed attempt does not silently restart
with a fresh search owner. The staged native image is private until success;
this is not a claim to reproduce reentrant custom-resolver access to partially
committed original palettes.

`PreparedBiomes::biomes()` borrows that computed image, including while a neighbor
job remains before NOISE. Consuming `PreparedBiomes::fill_noise()` moves the same
prepared owner through the existing one-shot material traversal and returns
`BiomeTerrain`. The image is moved with it, and `BiomeTerrain::terrain()` exposes
the resulting NOISE/SURFACE data read-only. Private fields prevent replacing
only the image or only the terrain with another chunk's data. No sampler,
aquifer, seed sequence or cache is reconstructed between these phases.

## Reads, neighbors and SURFACE

Original `ChunkAccess.getNoiseBiome` (`duy`, lines 418–427) clamps quart Y using
full chunk build bounds:

`minQuartY = levelMinY >> 2`

`maxQuartY = minQuartY + (levelHeight >> 2) - 1`.

After that clamp, its section/local indices select the stored palette. Native
`BiomeImage::get_noise_biome` applies the same Y clamp to the full section-span
image. Thus stored rows outside an unaligned accessor's readable quart range
still exist, but a read clamps to the original readable edge.

XZ ownership is a higher routing contract. Raw ChunkAccess masks local X/Z with
`& 3`; the original world/region first selects the chunk using floor division by
four. The native global image view rejects foreign X/Z instead of masking them
into this image. `BiomeImage::neighborhood(&mut actual_neighbors)` implements a
fallible quart provider: own queries read this image, foreign queries reach the
mandatory neighbor provider unchanged. Each selected neighbor applies its own
full-height clamp. Missing BIOMES data is an error; no selector is rerun to
invent an adjacent image and no fixed, wrapped or nearest-chunk fallback exists.

`BiomeTerrain::build_surface(zoom, &mut neighbors, &mut temperature)` constructs
the zoomed surface context from this actual center image, actual supplied
neighbors and the caller's persistent worker-local `BiomeTemperature`. The zoom
seed is explicit and is not hashed or inferred again. No thermal owner or
neighbor is fabricated. This bounded helper requires pinned built-in biome
climates; custom climates require their actual context and remain unsupported
by this convenience path. The underlying owned SURFACE transaction and duplicate
application guard remain in force.

Successful reads establish only the data actually queried. The scheduler must
still establish the complete BIOMES-radius-1 and STRUCTURE_STARTS-radius-8/status
preconditions, world/dimension identity, and all required neighbors before the
real phase runs. A data image is not the original ProtoChunk
`highestGeneratedStatus` witness: original ProtoChunk guards and persisted or
retrogen-target status transitions remain central integration work. Direct
palette observations on a still-EMPTY original ProtoChunk must be labeled as
such, rather than presented as a runtime-ready BIOMES/FULL chunk.

## Validation and limits

Focused native checks distinguish callback order from palette layout using a
persistent actual source and varied selected biomes. Additional indexing-only
fixtures clearly label their coordinate strings as controlled metadata, not
natural or production biomes. They cover full-height and partial-section storage,
exact Y clamping, negative XZ ownership, failure before publication, persistent
resolver effects on error, unchanged image allocation through NOISE, and actual
borrowed neighbor images. The bounded SURFACE check constructs actual selected
neighbor images; unavailable neighbors leave NOISE data intact.

Original/native pipeline probes separately compare actual selected palettes,
resolver query traces and the same owner's subsequent terrain. Those source
observations do not establish scheduler/status readiness or a Main callsite.
Carving, features, structures, retrogen, nonempty blending, persistence,
conversion, lighting and runtime publication remain outside this stage. The
fixed checklist and its scope are unchanged.
