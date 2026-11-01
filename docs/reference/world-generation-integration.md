# Native generation composition and production boundary

This stage composes seeded routers, stateful NoiseChunk traversal, aquifers,
ore veins and surface application. It returns **NOISE or SURFACE terrain**.
It does not return FULL chunks or complete a fixed checklist row by itself.

## Source identity and observations

The primary artifact is the original Java 1.21 server pinned by
`fixtures/source-lock.json`: outer SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official server mappings have SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`.

Inspection used direct `javap` through the installed `jdk.jdeps` module. No REA
provider or Evidence record was available. The following are static observations,
not claims that a Java server generated a complete playable world:

- `NoiseBasedChunkGenerator.createFluidPicker` (`dzb.a(dzd)`, mapped lines
  71–78, lambda lines 82–85) owns a lava status at level −54 and the settings'
  default-fluid status at the actual sea level. The selected status is lava
  when query Y is below `min(-54, seaLevel)`; otherwise the default status.
- `NoiseBasedChunkGenerator.doFill` (`dzb.a(eaa,ddq,dzm,duy,int,int)`, mapped
  lines 324–413) starts interpolation once, traverses cell X and Z forwards,
  cell Y and local Y downwards, then local X and Z forwards. After each update
  it asks the actual NoiseChunk block-state rule; null selects default block.
  Non-air writes update OCEAN_FLOOR_WG and WORLD_SURFACE_WG. A nonempty-fluid
  state is marked for postprocessing only when the aquifer requests it. Each
  X-cell swaps slices; the end stops interpolation.
- `NoiseChunk`'s constructor (`dzc`, mapped lines 108–209) builds the material
  rule list in aquifer-then-ore order, adding OreVeinifier only when the noise
  settings enable it. `OreVeinifier.create`/lambda (`dzi.a`, mapped lines
  42–80) preserves the positional ore factory, promoted float thresholds and
  short-circuit draw order. Copper uses Y 0…50, copper ore/raw copper/granite;
  iron uses Y −60…−8, deepslate iron ore/raw iron/tuff.
- `WorldGenerationContext` (`dzv`, mapped lines 10–13) separately computes
  `max(levelMinY, generatorMinY)` and `min(levelHeight, generatorDepth)` for
  vertical anchors. These are distinct from the chunk's build bounds.
- `NoiseBasedChunkGenerator.buildSurface` obtains the chunk's existing
  NoiseChunk and delegates to RandomState's SurfaceSystem. The supplied
  legacy-biome-height flag is false. Actual biome-manager and registry owners
  remain inputs, rather than converting raw climate values into invented biomes.

Existing canonical registry facts are reused: `items-1.21.json` resolves source
state JSON, `inventory::block_state_with_properties` resolves properties, and
`placement-1.21.json` supplies `isAir`, `blocksMotion` and fluid-nonempty flags.
Their existing extractor `tools/placement_catalog.java` directly calls the
original block-state methods. The staged heightmaps retain first-available Y;
the surface host exposes highest non-air Y as the source `getHeight` method does.

## Callable native API

`worldgen::NoiseGenerator::builtin(seed, settings)` builds per-world seeded
owners for any of the seven pinned built-in noise-settings records. It neither
consumes gameplay Level RNG nor creates a biome or scheduler owner.

`fill_noise(chunk_x, chunk_z, level_min_y, level_height, context)` returns a
`NoiseTerrain` borrowing that generator. `FreshGenerationContext` requires
explicit caller observations for initially empty block data, empty Blender and
empty structure beardifier. `Present` and `Unavailable` are errors. These
observations are caller contracts, not evidence that the production runtime
has already supplied them. Nonempty blending/structure influence is unsupported.
The retained graph borrow prevents replacing its node identities while chunk
caches are live.

The result exposes canonical states in ascending Y/local Z/local X order,
WORLD_SURFACE_WG and OCEAN_FLOOR_WG first-available heights, and ordered fluid
postprocessing positions. It carries `TerrainStage::Noise`, not ChunkStatus
FULL. No partial output is returned on failed material traversal. Noise cells
must form a nonempty aligned intersection with the level height accessor;
partial custom cells and an empty intersection are explicit unsupported paths.

`build_surface(&mut terrain, &mut SurfaceContext)` supplies actual block and
heightmap owners to the SurfaceSystem. `SurfaceContext` must supply zoomed
BiomeManager lookup and the original biome's snow/iceberg temperature predicates.
Writes, heightmap changes and postprocessing positions are privately staged;
unavailable context leaves the NOISE block image intact. Successful completion
changes only the native terrain stage to `Surface`. It does not perform carving,
features, structures, lighting, final heightmap conversion or runtime publication.

## Main integration contract still required

The inspected base is `643106e8eb60d970a8df67eb9acf0ab5f107c788`.
No production callsite invokes this module at that base:

- `src/main.rs` creates the explicit flat fixture or opens existing Anvil data.
- `VanillaWorld::chunk` in `src/storage.rs` reports `NotGenerated` when the
  region has no chunk; it does not call native generation.
- `MainChunkRuntime::new` creates `RuntimeWorldSources::fresh_flat_fixture`.
  Its source owner explicitly rejects Anvil FULL post-load/conversion. Disk or
  cache presence is not treated as a generation future.

The integration coordinator must supply the remaining production seams:

1. The mechanical `pub mod worldgen;` library export is included. The coordinator's
   published status at `46e5ca821b69a91e5555914dc82d03f57df6a507` explicitly
   permits this narrow change. It adds no Main generation callsite.
2. Own the real seed, noise-settings key, dimension and height accessor for one
   world. Construct and retain its generator independently of gameplay RNG.
3. Supply the original generation dependencies and status scheduler, including
   a real new ProtoChunk block image, BIOMES owner, empty/nonempty blending and
   structure influence. Unsupported dependency facts must remain pending/errors.
4. Invoke NOISE then SURFACE in their generation phases with actual biome
   manager, zoom seed and biome-temperature state. Retain cache and chunk identity.
5. Own the remaining CARVERS/FEATURES/LIGHT and final conversion phases,
   persistence, postprocessing, runtime readiness, and packet publication.
   A `NoiseTerrain` is never substituted for a completed resident FULL source.

`src/main.rs`, `src/main_*.rs`, shared runtime/storage seams, `src/lib.rs`,
manifests and checklist accounting are reserved to that coordinator. The only
shared source change is the explicitly permitted library export. The team's own
handoff status uses the separately assigned per-instance status path. Main integration remains pending; it is not
established by compiling or directly calling a generator stage in a probe.

Focused compiler and original-class runtime results are recorded in
[world-generation-validation.md](world-generation-validation.md) and the
generator-only validation fixture. NOISE arrays match the original class
traversal for all seven pinned settings in the selected cases. Controlled
fixed-biome SURFACE cases match actual original ProtoChunk output, both WG
heightmaps and surface postprocessing deltas. Those observations do not certify
natural biome selection or a Main missing-chunk generation path.
