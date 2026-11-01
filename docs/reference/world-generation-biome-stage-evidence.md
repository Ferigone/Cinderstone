# Actual seeded BIOMES data followed by same-owner NOISE

The target is the pinned Minecraft Java **1.21**, protocol 767 server:
inner SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Source inspection used direct `javap`; no REA Evidence is claimed.

| Original method | Contract exercised |
| --- | --- |
| `ChunkAccess.getOrCreateNoiseChunk` (`duy`), lines 401–404 | Install one real NoiseChunk and retain it across biome sampling and material generation. |
| `NoiseChunk.forChunk` / `cachedClimateSampler` (`dzc`), lines 108–111 / 212–218 | Clamp the noise grid against the actual generation accessor, retain wrapped density owners and expose their climate sampler. |
| `MultiNoiseBiomeSource.createFromPreset` / `getNoiseBiome` (`dej`), lines 46 / 73 | Use the actual pinned registry preset and worker-local search history with the supplied actual sampler. |
| `TheEndBiomeSource.getNoiseBiome` (`den`), lines 60–87 | Use the original central-island check and retained raw erosion sampler outside it. |
| `ChunkAccess.fillBiomesFromNoise` (`duy`), lines 432–441 | Fill every section of the actual generation height accessor, ascending section Y. |
| `LevelChunkSection.fillBiomesFromNoise` (`dvj`), lines 188–199 | Call the biome resolver in local quart X, then Y, then Z order and publish the resulting section palette. |
| `SECTION_BIOMES` indexing (`dvq$d`), mapped line 387 | Store local quart cells as `Y * 16 + Z * 4 + X`; query order and storage order differ. |
| `NoiseBasedChunkGenerator.fillFromNoise` (`dzb`), lines 291–319 | Run the actual public asynchronous method, including section/material writes, WG heightmaps and fluid-postprocessing marks, using the already retained NoiseChunk. |

The oracle creates actual original ProtoChunks, their canonical biome registry,
RandomState and explicit built-in MultiNoise or End source. The reused surface
oracle contributes only original registry/bootstrap code: no FixedBiomeSource
or fixed-biome run is used here. A transparent recording BiomeResolver delegates
every call to the actual seeded source and verifies that the sampler passed by
`fillBiomesFromNoise` is the retained cached sampler. After the actual fill,
every canonical biome is read back from the real section palettes.

NOISE then runs through the original public `fillFromNoise` future on that same
ProtoChunk and NoiseChunk. The fixture supplies the actual empty Blender and
BeardifierMarker. StructureManager is absent but source-cache-branch-proved
unused: the already installed NoiseChunk prevents the factory callback that
would access it. Successful execution and exact retained identities are checked.
This controlled absence does not establish real-world structure dependencies.

The constructor's persisted status remains actual EMPTY throughout these direct
method calls. No ChunkStep continuation is supplied or fabricated. Reading the
real section palettes does not bypass that distinction to claim runtime BIOMES,
NOISE or FULL readiness. Actual status completion and server scheduling remain
central integration work.

The paired native probe supplies the explicit matching source and retained
PreparedGeneration to PreparedBiomes. It records every actual resolver call,
retains one search cursor per preset on the BIOMES worker across chunks, then
moves only the owned biome image and prepared material job to a joined NOISE
worker. The outer generator binding is dropped first. It compares:

- Every canonical biome in section Y / local quart Y-Z-X storage order.
- Every resolver coordinate in section Y / local quart X-Y-Z callback order.
- Every canonical NOISE block, both WG heightmaps and every queued NOISE
  fluid-postprocessing position in its actual per-section insertion order.
- Original retained sampler, cache, aquifer and chunk identities and unchanged
  persisted status; native biome-image continuity and owned worker lifetime.

For postprocessing, the original probe reads every actual
`ChunkAccess.getPostProcessing` section list, including null/empty sections.
`ProtoChunk.packOffsetCoordinates` (mapped lines 251–257) packs low X, Y and Z
nibbles as `X | Y << 4 | Z << 8`. The original unpack method (lines 261–264)
reconstructs each absolute position from its actual section Y and ChunkPos; an
original pack/unpack roundtrip is checked. `markPosForPostprocessing` (269–272)
appends to the list selected by the actual chunk section index.

The native probe groups its actual global `terrain.postprocessing()` callback
vector by that same section index, preserving order and duplicates. Both packed
shorts and absolute positions are compared. Null and allocated-empty source lists
normalize only to no queued entries; original allocation flags are recorded
separately. At least one actual nonempty source section is required by the
runner before recording an order result. The selected Overworld cases contain
real queued fluid positions. This proves NOISE positions and order **within each
section list**; original section storage does not expose global inter-section
callback order. No SURFACE postprocessing comparison is claimed here.

Controlled cases include Overworld origin and negative coordinates, an actual
mixed Overworld chunk with dark forest/deep dark/lush caves, Nether and End with
actual level height **256** but noise height **128**, and an outer End chunk.
Thus BIOMES data spans the whole level, including the upper half left empty by
NOISE in Nether and End. No default biome substitutes for those upper cells.

Reproduce using Java 21, Rust and the pinned external artifacts:

```bash
python src/worldgen/validation/check_biome_stage.py \
  --oracle-dir /path/to/mc121-oracle \
  --work-dir /path/outside-the-checkout/biome-stage-check \
  --report /path/outside-the-checkout/biome-stage-report.json
```

`fixtures/world-generation-biome-stage.json` records the paired result and exact
source hashes. Terrain arrays, complete generated biome palettes, classes and
query traces remain outside the repository. These observations do not establish
WorldGenRegion dependency availability, nonempty blending or structures,
retrogen, datapack overrides, Main integration, carving/features, lighting,
persistence or FULL chunks. A SURFACE comparison backed by actual neighboring
BIOMES images is a separate verification boundary.
