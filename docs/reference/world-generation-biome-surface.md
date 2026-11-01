# Natural biome palettes through NOISE and SURFACE

This focused comparison targets the unchanged Java **1.21**, protocol 767 server.
Its inner SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
outer artifact and official mappings remain pinned in `fixtures/source-lock.json`.
The source bindings use direct `javap` inspection and actual original-class
execution, not REA Evidence.

The paired fixture computes natural biome palettes for a 3×3 neighborhood from
actual original MultiNoise or End biome sources. It fills real ProtoChunk section
containers through `ChunkAccess.fillBiomesFromNoise`, retaining each chunk's
cached climate sampler, NoiseChunk and aquifer. Neighbor jobs are visited in
ascending delta X, then ascending delta Z; resolver calls keep original section
and local-coordinate order. The same worker/preset search history persists
across that explicit sequence.

Only the center chunk then invokes the original public
`NoiseBasedChunkGenerator.fillFromNoise`. Its completed future must retain the
same ProtoChunk and the previously installed NoiseChunk. After NOISE, the actual
original `SurfaceSystem.buildSurface` consumes that same center owner, original
BiomeManager zoom and original biome thermal predicates. It receives the actual
settings' `useLegacyRandomSource` flag, including the initial Y-zero lookup for
legacy settings.

## Controlled palette provider boundary

SURFACE quart lookup is an **explicit controlled snapshot adapter** over the
nine actual section palettes. It routes the selected quart X/Z to its real
chunk, clamps quart Y against that selected chunk's full build bounds using the
original `ChunkAccess.getNoiseBiome` formula, then reads the actual section
container. It fails for missing snapshots. There is no fixed-biome return,
inferred replacement biome or generator-direct fallback.

The original ProtoChunks deliberately keep their actual persisted status
**EMPTY**, including after the low-level BIOMES, public NOISE and SURFACE calls.
The fixture neither constructs WorldGenRegion/ChunkStep nor invents their
status transitions. Calling ProtoChunk's guarded public biome reader would
require the missing scheduler status; direct palette access is disclosed rather
than presented as that runtime path. Successful selected neighbor reads do not
certify the complete server's BIOMES-radius-1 or structure dependencies.

The native side computes each of those palettes with `PreparedBiomes` and its
actual prepared density owner. It consumes the center through `fill_noise`,
retains the other real prepared neighbor images, and invokes
`BiomeTerrain::build_surface` using `BiomeZoom`, those images and one persistent
worker-local `BiomeTemperature`. The outer generator binding is dropped while
all prepared jobs retain the shared immutable generator. Stored palettes stay
unchanged through NOISE and SURFACE.

## Compared observations

The selected source cases are mixed overworld chunk `(-640,320)`, Nether chunk
`(0,0)` with its actual 256-block build height, and outer End chunk `(65,0)`, all
with seed `123456789`. The checker compares:

- Every canonical biome name in all nine palettes and the complete resolver
  query order used to fill them.
- Ordered selected quart reads that actually cross into a neighboring image,
  including raw Y before the source reader's clamp.
- Complete center canonical block arrays before and after SURFACE and both
  live WG heightmaps.
- Actual postprocessing positions, duplicates and insertion order inside every
  source section list before and after SURFACE. The earlier NOISE prefix must
  remain intact. Source null/allocated-empty flags are recorded separately;
  native global call storage does not expose the same allocation representation.
- Original center NoiseChunk, marker-cache and aquifer identity, unchanged
  stored biomes, and the actual unchanged EMPTY statuses.

Source section queues do not expose a global order between different sections;
this evidence does not claim one. A selected case adding zero SURFACE entries
proves preservation of its actual queues, not a positive SURFACE-only fluid
branch. The primary BIOMES/NOISE fixture separately checks nonempty original
NOISE queue ordering.

The optional paired checker is reproducible with original artifacts and toolchains:

```sh
python src/worldgen/validation/check_biome_surface.py \
  --oracle-dir /path/to/pinned-oracle \
  --work-dir /tmp/cinderstone-biome-surface \
  --report /tmp/world-generation-biome-surface.json
```

Generated Java classes, canonical arrays and intermediate palette observations
remain outside the checkout. The sanitized report records source hashes and
explicit comparison results. This bounded pipeline does not establish actual
WorldGenRegion availability, ChunkStep completion, Main scheduling, nonempty
blending/structures, retrogen, custom biome registries, carving/features,
lighting, persistence or FULL chunk publication.
