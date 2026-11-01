# Native world-generation surface stage

Target: unchanged Minecraft Java **1.21**, protocol 767. The inner server SHA-256
is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Outer server and official mappings identities are pinned in
`fixtures/source-lock.json`. The surface settings fixture records SHA-256 for
each original packaged `data/minecraft/worldgen/noise_settings/*.json` member.
It retains the material flags, generation geometry and complete surface rules;
router and spawn-target definitions are omitted from this fixture.

The evidence below is **direct JVM bytecode inspection with javap**, using the
installed Java 21 `jdk.jdeps/com.sun.tools.javap.Main` module. No REA provider or
REA Evidence record was available. Private disassembly and the original Java
artifacts remain outside the repository. Static inspection does not establish
execution, Main integration or whole-generator compatibility.

| Original mapped owner | Inspected methods | Native implementation |
| --- | --- | --- |
| `SurfaceSystem` (`dzq`) | constructor lines 60–79; `getSurfaceDepth` 201–203; `getSurfaceSecondary` 210 | RandomState root factory, named noises and positional depth jitter |
| `SurfaceSystem` (`dzq`) | `buildSurface` 82–195 | X-major/Z-minor columns, live WORLD_SURFACE_WG heights, original air/fluid/solid run traversal and default-state-only replacement |
| `SurfaceSystem$1` (`dzq$1`) | `getBlock`, `setBlock` 91–103 | Host contract for build-height clipping, live chunk writes and fluid postprocessing |
| `SurfaceRules.Context` (`dzp$g`) | `updateXZ` 95–100, `updateY` 103–109, secondary 112–116, minimum surface 129–153 | Per-column and per-Y invalidation, biome memoization and shared four-corner preliminary-surface cache |
| `SurfaceRules` (`dzp`) and nested rules | state, sequence, test, biome, noise-threshold, inverted, stone-depth, water, Y, vertical-gradient, above-preliminary, hole, steep and temperature conditions | Complete rule types occurring in the seven pinned packaged settings; unknown types fail compilation |
| `SurfaceSystem` (`dzq`) | `generateBands` 324–356, `makeBands` 360–369, `getBand` 372–373 | Original 192 bands, draw order, neighboring gray bands and Java rounding/remainder behavior |
| `SurfaceSystem` (`dzq`) | eroded badlands 238–272; frozen ocean 276–321 | Source noise scaling, height limits, fluid/air predicates, snow cap and conditional random draws |
| `VerticalAnchor` nested records (`dzs$a/b/c`) | `resolveY` | Absolute coordinates and original generation-context bottom/top semantics |

`SurfaceSystem::from_settings` compiles original rules and resolves block-state
JSON through the caller's canonical registry. It receives `RandomStateRandoms`,
using the same root positional factory as the seeded density graph. Clay bands
use `root.fromHashOf("minecraft:clay_bands")` directly; vertical gradients use
RandomState's separately hashed/forked named positional factories. Neither path
borrows the gameplay Level RNG.

The source caller `NoiseBasedChunkGenerator.buildSurface` forwards the settings'
`useLegacyRandomSource()` flag (`dzd.n`, mapped line 21; `dzb` caller bytecodes
38–45) into `SurfaceSystem.buildSurface`. It controls the column's initial
BiomeManager lookup: legacy settings use block Y `0`, while modern settings use
the initial WORLD_SURFACE_WG height plus one. This is independent of later
per-block rule-context biome lookups, which use their actual Y. The native
generator forwards the same setting. The focused surface oracle now also uses
the actual original setting and observes its first selected quart-biome query;
the native probe compares that query and the initial block coordinate. Constant
biome output images alone cannot verify this caller contract.

`builtin_settings` follows the original `ResourceLocation.parse` /
`bySeparator` (`akr.a`, mapped lines 56 and 77–89) default-namespace rule:
`overworld`, `:overworld` and `minecraft:overworld` select the same packaged
settings owner. An explicit different namespace is retained and must identify
an actual packaged setting; it does not select the default namespace's setting.

`SurfaceSystem::build_surface` consumes an existing chunk through `SurfaceHost`.
The host must supply actual BiomeManager block-coordinate biome lookup, biome
cold-enough-to-snow and iceberg-temperature predicates, canonical state metadata,
WORLD_SURFACE_WG heightmaps, chunk build bounds, WorldGenerationContext anchor
bounds, and the same NoiseChunk preliminary-surface owner used during NOISE.
Missing owners must return errors. A caller must stage the mutable chunk until
the operation succeeds; this API does not claim transactional rollback after a
host error. It does not create a FULL chunk, perform biome selection or advance
runtime scheduler stages.

Details retained from the original include the directional steep predicate
(south at least four above north, or west at least four above east), unclamped
secondary-depth mapping, `nextFloat` for vertical gradients, Java integer
wrapping, first-fluid water-height tracking, default canonical state identity for
rule replacement, default block *type* for badlands interception, and the
`DimensionType.WAY_BELOW_MIN_Y` sentinel `-32512`.

Production Main wiring, actual runtime biome ownership and complete chunk
scheduler dependencies remain integration work. The available APIs and parsed
rules alone do not complete `world_generation.3`; validation and production
connection must be assessed with the other generator modules.

## Focused execution evidence

The unchanged pinned server was also executed locally through reflection on
actual `RandomState`, `SurfaceSystem`, `ProtoChunk`, `WorldGenerationContext`,
heightmaps, a registered original biome and a real `FixedBiomeSource`/
`BiomeManager`. There was no Unsafe-allocated surface/chunk owner. These are
controlled fixed-biome fixtures; they do not demonstrate native biome selection.
The original SURFACE fixture imports original NOISE block arrays and constructs
a fresh original NoiseChunk, so it does not verify original cross-stage cache
continuity. The native comparison runs the seeded generator's NOISE stage followed by its
SURFACE API. Temperature and iceberg callbacks read observations obtained from
the actual original biome methods for the explicit fixture coordinates; a
missing observation is an error.

For seed `123456789`, chunk `(0,0)`, the following full block-image digests were
identical after original and native SURFACE execution. Both live generation
heightmaps also matched, converting the native packed first-available values
to the original `getHeight` convention. Surface-only fluid-postprocessing count
deltas matched separately from the earlier NOISE postprocessing records. Those
selected surface counts are zero; positions and list order were not compared.

| Noise settings | Actual fixed biome | SHA-256 of final canonical state image |
| --- | --- | --- |
| overworld | plains | `71b848d52791b383444b9753315093f2f9ce3d5368b36311848f8e1d31ab1a30` |
| overworld | eroded_badlands | `609afd1fd205fe97c757e247d309f6ff59f72585b4e70177ce87a88e3511c3ec` |
| overworld | frozen_ocean | `922f52a14372499d2d5ee7af77531796d80e3dd6ca09517a587b8769d471e7d2` |
| overworld | swamp | `71b848d52791b383444b9753315093f2f9ce3d5368b36311848f8e1d31ab1a30` |
| nether | nether_wastes | `19e96d5669a512037dc3fc8aa2035da4ec5a6e6336b5ff8e9b8a8d8c04f8fa01` |
| end | the_end | `7987147699f5c140f45beec79ea9dd62072ef26abdecd8f5082efe6533037dc3` |

An additional selected **positive badlands pillar** case used overworld seed
`0`, chunk `(0,0)` and the actual fixed `eroded_badlands` biome. The original
SURFACE method added 321 air-to-nonair voxels and raised 37 columns; for example,
the highest occupied Y at local `(8,10)` increased from `62` to `80`. This
executes pillar filling beyond the earlier case's water guard. The complete
original and native images matched with SHA-256
`80285dcd4c48a015d4988cf2feb045400aae80d4e602de839ee8242fa49368b0`,
along with both WG heightmaps and the surface-only postprocessing count delta. The
original NOISE input also matched the native seeded NOISE output. The same
unchanged Java classes and public native stage probes described above were
used, with actual biome-temperature observations and no added terrain fallback.

The native wrapper also rejected an unavailable biome context while preserving
its NOISE image, heightmaps, postprocessing and stage. Repeating SURFACE with the
same available context returns the specific duplicate-stage error without
context callbacks or changes to the completed terrain. A separate
original/native numerical probe matched surface-depth
integers, raw secondary-noise IEEE bits and clay-band canonical state IDs for
modern and legacy settings at origin, negative, boundary and world-edge
coordinates. These observations support the implemented stage and its failure
boundaries, not Main integration or complete terrain-generation parity.
