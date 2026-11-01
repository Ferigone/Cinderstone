# BiomeManager zoom over real quart-biome ownership

This stage ports **BiomeManager's block-to-quart lookup**, not biome selection,
chunk BIOMES generation or a ChunkStatus scheduler. Every lookup needs an actual
fallible `QuartBiomeProvider`. Missing neighbor chunks and unavailable BIOMES
state remain provider errors. No biome name, registry ID or fallback is built
into the helper.

The pinned Minecraft Java 1.21 inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings SHA-1:
`31c77994d96f05ba25a870ada70f47f315330437`.
Methods were inspected directly with `javap -p -c`; no REA tool or Evidence
record was available or used.

| Original mapped method | Preserved behavior |
| --- | --- |
| `BiomeManager` (`ddy`) constructor, lines 21–24 | Retain the already-obfuscated zoom seed and the actual quart-biome source. |
| `obfuscateSeed`, line 27 | Reuse native `world::obfuscate_biome_seed`; do not apply a second hash to an already-obfuscated seed. |
| `getBiome`, lines 35–79 | Subtract two using Java int wrapping, shift to quart coordinates, evaluate corners 0 through 7, retain the first strictly nearest corner, then query the source once. |
| `getFiddledDistance`, lines 101–120 | Mix signed X/Y/Z coordinates twice through the LCG, derive X/Y/Z jitter with the original zoom-seed salts, sum squared Z, Y and X offsets in that order. |
| `getFiddle`, lines 124–125 | Arithmetic right shift by 24, positive floor-modulo 1024, then `(fraction - 0.5) * 0.9`. |
| `LinearCongruentialGenerator.next` (`ayj`), lines 8–10 | Signed-long wrapping `seed * (seed * 6364136223846793005 + 1442695040888963407) + salt`. |

`BiomeZoom::from_world_seed` accepts the real world seed and calls the existing
seed-obfuscation implementation once. `from_zoom_seed` accepts the already
obfuscated seed from an actual owner. Neither constructor consumes the gameplay
RNG. `selected_quart` exposes only a selected coordinate; `get_biome` makes one
provider call and returns its result unchanged. Y clamping belongs to the
provider: the original manager forwards the selected quart Y to its source.

The controlled original-Java runtime probe behind
`fixtures/world-generation-biome-zoom.json` called the unchanged BiomeManager
with a test NoiseBiomeSource. That source returned an actual `Holder.direct`
containing a coordinate token, so the returned value identifies the exact
queried quart coordinate. It supplied no real terrain biomes. The probe
recorded negative coordinates, chunk boundaries, Java int overflow at the
block-minus-two boundary, world-seed obfuscation, already-obfuscated seeds,
raw fiddled-distance/fiddle bits, wrapping LCG outputs and one-call failure
propagation. The fixture pins the probe and output hashes.

Focused scratch validation compiled this module against the repository's real
`world` module and compared those recorded coordinates and raw bits. Native
provider witnesses covered successful and failing one-call lookup. Exact ties
were checked with a controlled equal-distance comparator input and the source
strict comparison; no naturally occurring exact tie is claimed as a runtime
observation. These checks validate this lookup contract, not biome generation,
Main integration or full terrain parity.

A production SurfaceContext adapter must obtain the real world/dimension zoom
seed, query actual generated quart-biome owners (including neighboring chunks),
and feed the selected canonical biome to its thermal owner. Constructing those
BIOMES records, meeting generation dependencies, completing lighting/features,
persistence and publishing FULL remain separate work.
