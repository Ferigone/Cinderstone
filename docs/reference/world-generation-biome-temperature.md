# Built-in biome temperature ownership

Target: Minecraft Java **1.21**, protocol 767. The original inner server SHA-256
is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
the outer artifact and official mappings are pinned in `fixtures/source-lock.json`.
This ledger uses direct Java 21 `javap` inspection and actual unchanged original
class execution, not REA Evidence.

`fixtures/world-generation-biome-climate.json` contains the climate fields from
every original packaged `data/minecraft/worldgen/biome/*.json` member, retaining
per-member SHA-256 identities. Extraction verified precipitation, modifier and
raw temperature/downfall float32 bits against the actual original built-in
registry's `Biome.ClimateSettings` objects. The explicit `none` modifier is the
original codec default when its JSON field is absent. Native temperature reads
those verified bits; unknown biome keys return an error. The caller must certify
that its provider uses the pinned built-in registry contents. String keys do not
detect datapack overrides under existing vanilla names; such an override passes
the name lookup and would use the packaged climate, so it requires another
thermal implementation.

| Original owner/method | Observed behavior retained natively |
| --- | --- |
| `Biome.<clinit>` (`ddw`, mapped lines 37–60) | Independent `WorldgenRandom<LegacyRandomSource>` seeds `1234` with octave `[0]`, `3456` with `[-2,-1,0]`, then `2345` with `[0]`. These are immutable class noises, independent of world seed and gameplay RNG. |
| `Biome.TemperatureModifier.FROZEN.modifyTemperature` (`ddw$d$2`, lines 96–106) | Frozen noise at double coordinate scales `0.05`, `0.2`, then conditionally `0.09`; original double comparisons and the `0.2f` replacement. |
| `Biome.getHeightAdjustedTemperature` (`ddw.e`, lines 175–182) | Above Y `80`, X/Z narrow to float before division by `8`; noise multiplied by double `8` narrows back to float. Subsequent additions, subtraction, `0.05f` multiplication and `40f` division retain float32 order. |
| `Biome.getTemperature` (`ddw.f`, lines 188–199) | Per-biome, per-worker packed-BlockPos cache; a NaN lookup is a miss. At size `1024`, remove the oldest inserted entry before `put`. Reads do not change insertion order. |
| `Biome.coldEnoughToSnow` / `warmEnoughToRain` (`ddw.b/c`, lines 229–233) | Snow predicate is the negation of `temperature >= 0.15f`, including the source NaN behavior. |
| `Biome.shouldMeltFrozenOceanIcebergSlightly` (`ddw.d`, line 237) | Original cached temperature comparison `> 0.1f`. |

`BiomeTemperature::builtin` creates one mutable thermal owner for the pinned
registry on one generation worker. Reuse that owner across chunks on that worker;
use a distinct owner for each other worker or registry identity. It is neither
`Send` nor `Sync`, keeping the original ThreadLocal cache prehistory with its
worker. Shared immutable noises and climate definitions do not carry that
prehistory. No world seed or gameplay RNG is accepted by this API.

`canonical_biome` is a pure name lookup in the pinned climate table, not an
automated registry-identity or same-key override check. It performs no thermal
or noise sample and touches no temperature cache.
`temperature`, `cold_enough_to_snow` and
`should_melt_frozen_ocean_iceberg_slightly` require the provider-selected biome
key and block coordinates. A BiomeManager/zoom owner must supply that key; raw
climate density samples are not used as a replacement biome. Resource locations
with omitted or explicitly empty namespace use the original `minecraft` default.

The original packed cache key intentionally aliases positions outside its
26-bit X/Z and 12-bit Y packing fields. The native cache keeps this behavior
rather than recomputing from the unpacked key or silently clearing old entries.
It starts empty just as the corresponding original biome's ThreadLocal cache
starts empty for that worker. Dynamic registry reload and restoring an unknown
previous worker/cache history remain unsupported.

## Focused original execution

A temporary reflection probe bootstrapped the actual original built-in registry,
read its climate fields, and invoked the unchanged `Biome.getTemperature`,
`coldEnoughToSnow` and `shouldMeltFrozenOceanIcebergSlightly` methods. Selected
positions exercise Y `80`/`81`, positive and negative coordinates, original world
edges, both frozen-modifier outcomes and the actual float temperature results.
The result fixture is `fixtures/world-generation-biome-temperature-vectors.json`.
It also records both query orders for packed-position aliases `(0,0,0)` and
`(0,4096,0)`, and the actual first/last keys after filling the original cache,
reading its oldest entry and inserting another position. This establishes FIFO
rather than read-touch eviction for the native cache.

The temporary probe source SHA-256 was
`a0f20e2c1c0adb8736098bc623995e954192e018da651261e505f057f79ca15a`;
its original result transcript SHA-256 was
`f9f22055ee0822cd59eae996554dfc2a942cdfb6802553cf5126876ab800f5d7`.
The original process exited successfully. Java artifacts and raw disassembly
remain outside the repository. Native focused comparisons assert raw float32
bits, both thermal predicates, packed-key prehistory, FIFO ownership and absence
of cache effects during pure name validation. These checks do not establish
natural biome selection, Main integration or complete dimension generation.

The unchanged portable reflection probe is committed at
`src/worldgen/validation/BiomeTemperatureOracle.java`, matching the source hash
above. Entry class `oracle.BiomeTemperatureOracle` consumes JSON lines with
`climates`, `clear`, `point` (biome/point), or `fifo` operations; its classpath
requires the verified original server and libraries. No original binaries are
packaged with these source probes.
