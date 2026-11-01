# SurfaceSystem top-material query

Target: unchanged Minecraft Java **1.21**, protocol 767, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Artifact and official mapping identities remain pinned in `fixtures/source-lock.json`.
Source inspection used direct Java 21 `javap`; no REA Evidence is claimed.

`SurfaceSystem.topMaterial` (`dzq.a`, mapped lines 223–234) creates a fresh
`SurfaceRules.Context` from its actual `CarvingContext`, biome function,
ProtoChunk and retained NoiseChunk. It applies the supplied RuleSource to that
context, initializes X/Z, then calls `updateY` with stone depths `1`, `1` and
water height `Y + 1` when the caller's boolean is true, otherwise `Integer.MIN_VALUE`.
It calls the resulting rule once and returns `Optional.ofNullable`.

The native query is:

```rust
SurfaceSystem::top_material(
    &self,
    host: &mut impl SurfaceHost,
    point: DensityPoint,
    has_fluid: bool,
) -> Result<Option<u32>>
```

This scoped API uses this exact seeded SurfaceSystem's compiled built-in settings
rule. The original method also permits an arbitrary separately supplied RuleSource;
custom/different rules are outside this API. Each call has fresh per-query rule
caches. It does not scan a column, read or replace the target block, run the
badlands/iceberg extensions, advance a generation stage or substitute a material
when the rule returns `None`. Lazy biome, thermal and preliminary-surface queries
can still update their actual owners' caches, as in the original.

The host must supply the actual CarvingContext generation bounds, live
WORLD_SURFACE_WG heightmap for steep checks, supplied block-biome function and
thermal predicate, and that same NoiseChunk's preliminary-surface owner. Other
SurfaceHost members are unused by this query. Existing rule short-circuiting
keeps unneeded dependencies unqueried; unavailable required owners return errors.

`CarvingContext.topMaterial` (`eaz.a`, line 38) delegates its stored RandomState,
NoiseChunk and RuleSource to the method above. In `WorldCarver.carveBlock`
(`ebe.a`, lines 133–167), the caller first writes the actual carved replacement.
When the reached-surface branch finds dirt below it, the query position is that
lower dirt position, while `has_fluid` comes from the carved replacement **above**.
The caller applies an optional result and handles resulting fluid postprocessing.
`None` leaves that dirt block untouched.

`NoiseBasedChunkGenerator.applyCarvers` (`dzb.a`, lines 257–287) supplies a
BiomeManager with a different source backed by the generator's seeded
NoiseBiomeSource and RandomState sampler. A future carving caller must retain
that actual source/zoom owner; this method does not select biomes or silently
reuse unrelated stored-chunk biome data. No CARVERS scheduling or carving loop
is implemented by this change.

## Controlled original execution

`src/worldgen/validation/WorldgenTopMaterialOracle.java` reuses the existing
surface oracle's original-registry bootstrap. It constructs actual original
ProtoChunk, retained NoiseChunk, RandomState, ImmutableRegistryAccess and
CarvingContext owners and invokes the unchanged original method. The input
canonical block arrays come from the existing source-generated NOISE fixtures.
Actual original fixed-biome BiomeManagers supply the controlled biome function;
this does not demonstrate natural biome selection or an executed carving stage.

`fixtures/world-generation-top-material.json` records nullable canonical results,
actual biome callback positions, required source WG/preliminary/thermal values,
input-image digests and source/transcript hashes. Original before/after canonical
block arrays were unchanged by these calls. Selected observations include:

- An overworld bedrock result with an unavailable biome callback never invoked.
- An empty Optional below the preliminary surface, also without a biome lookup.
- At overworld `(0,64,0)`, dry plains returns canonical grass-block state `9`,
  while the caller's fluid flag produces canonical dirt state `10`.
- A required biome callback failure propagates at the same position rather than
  selecting a fallback material.
- Desert, frozen-ocean, nether-wastes and End rules with their original owners;
  the End's unconditional rule does not call an unavailable biome function.

The native focused check consumes those source observations, compares results and
biome query order, and traps block reads/writes, inferred fluid flags, column
scans and iceberg-extension calls. It tests the top-material API's bounded rule
query and failure semantics, not a complete carver, Main integration or FULL chunk.
