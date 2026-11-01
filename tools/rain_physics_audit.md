# Rain exposure, original Java 1.21

The original inner server JAR is pinned to SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The containing official artifact was inventoried by REA in
`evidence/rea-artifact.json`, Evidence
`ev_01dc3f66ca2191406947da9b3c4ab86f1ee130e356d200f79f3958263a1ab77b`.
That archive Evidence establishes artifact identity/inventory, not JVM behavior.
The bytecode files `tools/rain_*_bytecode.txt` and the unchanged-original JVM
oracles provide the separate static and execution evidence for this port.

## Exact predicates

`Level.isRainingAt` first calls `isRaining`: original f32 rain interpolation at
partial tick1, widened to f64 and compared with the f64 constant0.2. Next it
calls `canSeeSky`, whose order is SKY brightness then maximum light level,
with an integer `>=` comparison. Only then does it query the MOTION_BLOCKING
heightmap position. A height above the query Y rejects exposure. Finally it
asks the actual biome's precipitation at that position and accepts only RAIN.

`Entity.isInRain` first uses the cached entity BlockPos. On false it calls the
same level predicate at `(cachedBlockX, floor(AABB.maxY), cachedBlockZ)`.
It does not use raw entity X/Z or eye height, and does not deduplicate equal
body/top points. Rust's existing Java floor helper retains the original NaN,
infinity, integer-cast and wrapping behavior for the predicate oracle.

`tools/rain_physics_oracle.java` leaves original Entity/ItemEntity/Level methods
unchanged. A generated world supplies lower brightness, height and actual
registered plains/snowy-plains/desert holders. A rain-level wrapper invokes the
unchanged original interpolation and records its use. Faults belong to these
controlled lower getters. The actual ItemEntity's inherited **private bsr.w**
is invoked explicitly: the unrelated obfuscated `cjh.w` is not the predicate.
The frozen corpus has1,000 cases,4,011 traced calls and69 controlled faults.
Actual biome results and cache history are recorded as world facts; the generic
predicate replay does not invent a new empty biome cache for each case.

## Concrete fresh native source

`rain_fixture_oracle` constructs nine genuine LevelChunks and actual sections
with canonical air0/stone1/dirt10/grass9, primes original heightmaps and sky
sources, and settles the actual SKY light engine. All256 central columns have
highest occupied63, Level height64, lowest sky source64. Its106,752 actual
brightness/visibility evaluations cover Y−80..336 and yield one uniform row.
These physical source facts are separate from the old fixture's static
full-light **network packet**, which is not used as rain evidence.

`FreshFlatPlainsRain` rejects Anvil, noncanonical base states, other biome IDs,
any current sparse world edit and points outside the measured Y window or
supported in-world horizontal coordinates. Uniform translation to other flat
columns is an inference from the strict canonical unedited source invariant,
not an observation of every possible chunk. A required host capability also
checks the persistent settled/unedited witness and current FULL futures. There
is no blanket implementation for RuntimeChunkManager: FULL alone cannot prove
lighting history, and edit→restore cannot recreate a lost witness.

The actual registered plains climate, temperature-noise permutation/gradients
and exact F2/G2 bits are frozen by the original oracle. Native code ports the
single octave simplex arithmetic, integer→f32 division before widening, and
the original f32 altitude expression without reassociation. It matches1,542
actual temperature bit patterns. Precipitation is calculated, not assumed to
remain RAIN for arbitrary altitude. A persistent1024-entry insertion cache
retains BlockPos packing and eviction behavior. Its constructor explicitly
requires observed empty temperature history; no Default manufactures missing
prehistory. Keep this owner across all items/ticks of the certified one-world,
one-thread slice. General shared-biome/multiple-thread ownership is unproved.

## Runtime bridge and integration contract

`RuntimeItemWorld::with_rain_exposure(&mut dyn RainExposure)` is additive.
`was_touching_water` returns first. Known actual non-raining weather preserves
the earlier path. `ExposureUnavailable` now uses the supplied exposure; absence
still errors. False exposure then checks the actual bubble-column block.
Exposure capabilities are called only after actual global rain is established;
the concrete frame's known-true global predicate does not change weather.

The separate full-item rain probe also exposed an integration dependency:
original `Level.playSound` draws its sound seed from `dcw.f/threadSafeRandom`,
not `dcw.z/level.random`. Exposure evaluation consumes neither stream. The
coordinator must retain the actual separate sound source; changing the main
level RNG for a sound is incorrect. Packet helpers take an explicit captured
seed and do not choose or advance a source themselves.

The host owns one `FreshFlatPlainsRain`, creates a borrowed frame with a real
`RainChunkAvailability`, and forwards it to every item provider. It must still
certify the complete motion/event halo and actual backend facts before item
mutation. This module does not edit main, world_context, weather timers, level
RNG or the existing no-rain/water physics. It does not itself make default
vanilla advancements, arbitrary lighting edits, generation, imported sources,
multiple worlds or complete Player.tick supported.

`rain_physics_check.py` records targeted native tests, the concrete item-provider
regression suite and Clippy. Unless explicitly run with `--release`, its report
is debug evidence only; checkpointv5 intentionally excludes this later work.
