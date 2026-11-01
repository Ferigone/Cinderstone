# Retained NoiseChunk climate sampling before NOISE

The source is the pinned original Minecraft Java 1.21 server. Its inner JAR
SHA-256 is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
the official mappings SHA-1 is
`31c77994d96f05ba25a870ada70f47f315330437`.
The methods below were inspected directly with `javap -p -c`. No REA tools or
REA Evidence records were available or executed.

| Original mapped method | Relevant ownership and sampling behavior |
| --- | --- |
| `NoiseBasedChunkGenerator.doCreateBiomes` (`dzb`), lines 98–103 | Obtain the chunk's retained NoiseChunk, construct the effective biome resolver, then fill biomes using its cached climate sampler. |
| `ChunkAccess.getOrCreateNoiseChunk` (`duy`), lines 401–404 | Construct only when its NoiseChunk field is absent; later phases use that same object. |
| `NoiseChunk.cachedClimateSampler` (`dzc`), lines 212–218 | Map temperature, humidity, continentalness, erosion, depth and weirdness through this NoiseChunk's existing wrap map, then construct Climate.Sampler with the actual settings spawn-target list. |
| `Climate.Sampler.sample` (`def$f`), lines 457–469 | Convert quart X/Y/Z to block coordinates with wrapping int shifts; create a separate SinglePointContext; evaluate the six roots in order; narrow each double to float before climate quantization. |
| `NoiseChunk` constructor, lines 42–209 | Allocate marker/cache owners, the aquifer and the material-rule owner before climate sampling. |
| `NoiseBasedChunkGenerator.doFill`, lines 324–413 | Consume the retained interpolation/material owners in the original cell and block order. |

The native prepared owner must retain its actual seeded generator, density
caches, traversal state and aquifer through climate sampling into NOISE. Raw
`DensityGraph::sample_climate` evaluates before NoiseChunk marker wrapping; it
is not a replacement for this retained sampling context. Climate samples alone
neither select nor store a biome and do not establish BIOMES status.

`src/worldgen/validation/WorldgenPreparedClimateOracle.java` constructs an
actual unchanged original NoiseChunk, invokes its protected cached climate
sampler, samples explicit quart points, then traverses NOISE on that same
instance and samples again through the original retained climate sampler. It uses the original built-in settings, RandomState, global fluid
picker, empty Blender and BeardifierMarker. It verifies exact interpolator and
cell-cache object identities, unchanged interpolation counters during climate
sampling, and the same aquifer object after NOISE. No Unsafe allocation is used.
The original loaded NoiseChunk artifact is checked by its runtime CodeSource
SHA-256 as well as by the runner's pinned artifact checks.

The controlled inputs include Overworld origin and negative-X chunks, Nether
and End, neighboring quart columns, repeated points and int-endpoint quart
coordinates that exercise wrapping conversion to block coordinates. The
native `prepared_climate.rs` probe uses the prepared owner, samples the same
points, drops its outer generator binding and moves the prepared object to a
joined worker thread before consuming NOISE. Its output remains an intermediate
NOISE terrain image; post-NOISE samples use that output's same retained density
owner. The comparison checks the complete quantized parameter arrays before and
after NOISE, every canonical block state and the fluid-postprocessing count.

Reproduce with Java 21, Rust and the pinned server/mappings outside the checkout:

```bash
python src/worldgen/validation/check_prepared.py \
  --oracle-dir /path/to/mc121-oracle \
  --work-dir /path/outside-the-checkout/prepared-check \
  --report /path/outside-the-checkout/prepared-report.json
```

`fixtures/world-generation-prepared-climate.json` records the focused result
and hashes of its source inputs. Generated classes and binary terrain arrays
remain outside the repository. This controlled original-method observation is
not a natural-biome or production-server result. Real BIOMES resolution/storage,
Blender/structure dependencies, retrogen, ChunkStatus transitions, spawn search,
persistence and Main integration remain separate owners and deliveries.
