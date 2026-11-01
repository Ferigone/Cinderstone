# SURFACE context from real biome owners

`BuiltinBiomeSurfaceContext` connects the existing `SurfaceContext` callbacks to
an explicit `BiomeZoom`, a mandatory fallible `QuartBiomeProvider<Biome = String>`,
and a borrowed worker-local `BiomeTemperature`. It generates neither BIOMES data
nor a substitute biome. The provider remains responsible for actual quart-biome
ownership, neighboring chunk availability, generation dependencies and quart-Y
clamping. A provider failure or unsupported biome name propagates as an error.

`new` accepts an existing zoom owner. `from_world_seed` accepts the known real
world seed and applies the zoom-seed obfuscation once. Neither constructor
chooses a seed or consumes gameplay RNG. Both borrow the supplied provider and
thermal owner; the adapter retains the thermal owner's `!Send`/`!Sync` boundary
and belongs in the synchronous generation worker, outside the shared generator.
Reuse the same thermal owner for successive chunk tasks on that worker and
registry identity.

The callback sequence follows the pinned Minecraft Java **1.21** server,
protocol **767**, inner JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
The underlying source inspections used direct `javap` and original runtime
execution, with no REA tools or invented Evidence IDs.

| Original contract | Adapter behavior |
| --- | --- |
| `BiomeManager.getBiome` (`ddy`, mapped lines 35–79) selects one quart coordinate and queries its real source once. | `biome(x,y,z)` calls `BiomeZoom.get_biome` once with the mandatory provider. |
| `SurfaceSystem.buildSurface` (`dzq`, lines 82–195) and `SurfaceRules.Context.updateY` (`dzp$g`, lines 103–109) obtain the biome without evaluating its temperature. | The returned provider name is validated through pure `BiomeTemperature.canonical_biome`; this callback performs no thermal sample or cache insertion. |
| `SurfaceRules.Context` temperature condition (`dzp$g$d`) and frozen-ocean extension call the selected biome's actual thermal methods when needed. | Thermal callbacks delegate the supplied biome and exact block coordinates to `BiomeTemperature`, without another provider query. |

The [zoom ledger](world-generation-biomes.md) records the selected-quart source
behavior. The [temperature ledger](world-generation-biome-temperature.md)
records the original built-in climate owners, noise seeds, thresholds and
worker-local packed-position cache. This adapter adds no new climate parameters,
noise definitions or cache ownership.

Focused adapter checks use a clearly controlled quart-provider witness. They
connect the original observed world-seed-0 lookup at block `(0,0,0)` to its single
quart query `(-1,0,-1)`, canonicalize the actual supplied name, compare the returned
biome's thermal predicates with the existing original frozen-ocean observations,
and verify that thermal calls add no provider queries. Failure cases preserve the
provider error and reject an unknown biome.

A separate lazy-callback check exploits the original packed-position alias
between `(0,4096,0)` and `(0,0,0)`: looking up the biome at the high position must
not cache its cold temperature before the later low-position thermal callback.
That check uses the original ordered-alias observation in
`fixtures/world-generation-biome-temperature-vectors.json`, making an accidental
eager temperature lookup observable.

Compiler and focused execution results belong in the coordinating PR after the
new module export is added. These checks establish adapter ownership and call
ordering only. They do not establish natural biome selection, a production
quart-biome provider, Main integration, ChunkStatus scheduling or FULL terrain.
