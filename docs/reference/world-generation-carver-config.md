# Configured carvers and their sampled providers

Target: unchanged Minecraft Java **1.21**, protocol **767**, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mapping SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
The source inspection uses direct Java 21 `javap`, not REA. Packaged resource
digests and unchanged configuration JSON are recorded in
`fixtures/world-generation-configured-carvers.json`.

## Original configuration boundary

| Original method | Required behavior |
| --- | --- |
| `CarverConfiguration` (`eax`), mapped lines 16–23 | Probability in `[0,1]`, required `y`, `yScale`, `lava_level`, block-holder set `replaceable`, and optional debug settings. |
| `CaveCarverConfiguration` (`eba`), lines 12–17 | Horizontal/vertical radius providers and a floor provider constrained to `[-1,1]`. Nether uses this same configuration shape. |
| `CanyonCarverConfiguration` (`eav`), lines 40–44, and shape (`eav$a`), lines 14–21 | Vertical rotation plus distance, thickness, nonnegative width smoothness, horizontal radius, default vertical radius and center vertical radius factors. |
| `CarverDebugSettings` (`eay`), lines 9–23 | Missing debug mode is false. Presence of a debug-settings object does not enable carving debug mode. |
| `WorldGenerationContext` (`dzv`), lines 10–20 | Minimum is `max(accessor minimum, generator minimum)`; depth independently is `min(accessor height, generator depth)`. |
| `VerticalAnchor` implementations (`dzs$a/b/c`), lines 62, 76, 90 | Absolute Y, minimum plus offset, or depth minus one plus minimum minus offset, with Java int arithmetic. |
| `UniformHeight` (`eia`), lines 39–48 | Resolve both anchors against the actual context. A reversed range returns its minimum without sampling; otherwise draw an inclusive bounded integer. |
| `ConstantFloat` (`bps`), line 31; `UniformFloat` (`bqb`), line 41; `TrapezoidFloat` (`bqa`), lines 41–45 | Zero, one or two draws from the supplied random owner respectively; preserve each original float operation. |

The four original configured holders are:

| Holder | Probability | Inclusive Y anchors | yScale |
| --- | --- | --- | --- |
| `minecraft:cave` | 0.15 | above bottom 8 through absolute 180 | uniform 0.1 through 0.9 |
| `minecraft:cave_extra_underground` | 0.07 | above bottom 8 through absolute 47 | uniform 0.1 through 0.9 |
| `minecraft:nether_cave` | 0.2 | absolute 0 through below top 1 | constant 0.5 |
| `minecraft:canyon` | 0.01 | absolute 10 through absolute 67 | constant 3 |

Float uniform upper endpoints are configured as exclusive. Native sampling
preserves source float rounding and applies no additional endpoint clamp.
The two overworld cave configurations share radius providers `[0.7,1.4)` and
`[0.8,1.3)`, floor `[-1,-0.4)` and lava anchor above bottom 8. Nether has constant
radius multipliers 1, floor -0.7 and configured lava anchor above bottom 10.
That field does not replace NetherWorldCarver's separate block-carving override.
Canyon has rotation `[-0.125,0.125)`, distance and horizontal radius factors
`[0.75,1)`, trapezoid thickness `(0,6,2)`, width smoothness 3, vertical default
factor 1, center factor 0, and lava anchor above bottom 8.

The per-biome holder lists preserve packaged order: 53 built-in biomes specify
cave, extra-underground cave, then canyon; the five Nether biomes specify only
nether cave; the six End/void biomes have no configured carvers. These lists do
not determine which biome a neighboring chunk's actual `carverBiome` cache holds.

## Native API and ownership

`carver::config::builtin_configured_carver` returns one immutable typed
`ConfiguredCarver::{Cave,NetherCave,Canyon}`. `base()` exposes its
`BaseCarverConfig`. `builtin_biome_carvers` returns the recorded AIR holder order
for an actual pinned built-in biome. Unknown configured names and unknown biome
names are errors; a recorded empty list is distinct from an unavailable owner.
Both lookups use Minecraft's default namespace for omitted or empty namespaces.

These are closed pinned built-in lookups. Callers must establish that the actual
registry has not been changed by datapacks; a matching name is insufficient.
Custom registries, direct custom replacement sets and provider kinds outside
this implemented subset remain unsupported. This module does not select the
source biome, cache neighboring generation settings, seed a random owner or
execute a carving stage.

`HeightContext` carries the actual carving context's independent minimum and
depth. It is not a replacement dimension accessor or an inferred NOISE span.
`VerticalAnchor::resolve` retains signed-int wrapping arithmetic. `HeightProvider`
supports original constant anchors and inclusive uniform sampling. Valid
singleton uniform ranges still call `next_bounded_i32(1)`. Reversed resolved
ranges return the minimum without a draw, including clipped generation contexts.
The original provider's warn-once logging set is omitted as a diagnostic-only
effect; no different height, random sequence or terrain fallback is introduced.

`FloatProvider::sample` borrows the caller's `RandomSource`. Constants do not
consume it. Uniform sampling is `nextFloat * (max - min) + min` in float32.
Trapezoid sampling computes `range = max - min`, `slope = (range - plateau)/2`,
then `min + first * (range - slope) + second * slope`, preserving the two draws
and intermediate float rounding even for a degenerate range. There is no
forking, reseeding, double-precision substitute or fused multiply-add.

## Debug states and replacement holders

The original default debug owner contains disabled mode, default acacia-button,
candle, orange-stained-glass and glass states. Entirely missing debug settings
use that complete default. The original per-field optional codec instead uses
the default **air debug state** when an individual state field is omitted from
a present object; the private decoder preserves that source distinction.
The packaged overworld configurations explicitly provide all four debug states,
while Nether omits the entire debug object. Block-state properties are resolved
through the existing canonical state resolver.

`CarverReplaceable` permits only the original overworld and Nether block tags.
It validates source identity and tag presence in the existing pinned
`placement-block-tags-1.21.json`, then uses its recursively resolved block
membership. The source tags contain 49 and 24 block identities respectively;
membership includes every property state of a member block. Unknown canonical
states and unavailable tags are errors rather than a false membership result.
No second block-tag registry or silently empty set is created.

## Focused original observations

`WorldgenCarverOracle` in `providers` mode executes the original provider codecs
and classes with a real original generation context and LegacyRandomSource. The
fixture records sampled height values or raw float bits, followed by the next
long from the same random owner. The selected sequences cover constant, uniform
and trapezoid floats, constant and uniform heights, singleton inclusive bounds,
and a reversed anchor range in a short generation context. The next-long witness
distinguishes the singleton draw from the reversed range's absent draw. Native
focused tests consume these observations; compiler/runtime results belong in the
coordinator's validation record.

The same original probe reads the actual configured registry's debug settings.
All four modes are false. Cave, extra-underground cave and canyon use canonical
air-debug states 19109, 8620 and 19133; Nether uses its actual default owner with
air state 8716. All have water 20728, lava 5946 and barrier 519. Native checks
compare every observed field. These immutable configuration observations do not
establish a completed carving stage or production registry ownership.

This configuration/provider layer alone establishes no Main callsite, CARVERS
status, scheduler dependency, complete terrain delivery or FULL runtime chunk.
