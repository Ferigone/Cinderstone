# Native generation stage validation

Initial pipeline implementation source commit:
`6c0b18c50abcee153705675e8d900757400b1e72`, based on
`643106e8eb60d970a8df67eb9acf0ab5f107c788`.
The subsequent arithmetic, settings-alias and no-cell corrections are listed in
the [team handoff](../coordination/status/worldgen-20261101-c4e7.md).
The checked-in generator-only report records the current exact source and probe hashes:
[`world-generation-stage-validation.json`](../../fixtures/world-generation-stage-validation.json).

Reference inspection used direct `javap` through Java 21's `jdk.jdeps` module.
No REA tools were exposed, and no REA Evidence IDs are claimed. The original
server and official mappings were verified against `fixtures/source-lock.json`;
the inner server SHA-256 was
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

The real Main compiler check passed:

```sh
cargo check --locked --bin mc121-rust
cargo test --locked --lib worldgen::
cargo test --locked --lib density::chunk::tests
```

The focused native checks cover original RNG/factory observations, original
aquifer decisions and fluid-update flags, cache-owner identity, and failed
traversal cleanup. Existing unrelated compiler warnings remain. The Main check
includes the mechanically exported namespace; it proves compilation, not a
production generation callsite.

The exact generator source/fixture diff was also applied to a separate detached
worktree at current pinned master
`f40d8bf066ecb446b46a66e1d6d9465ffb233d6e`. The full Main build and the same
focused native checks passed there. This source-overlay validation performed no
master merge and changed no reserved Main implementation. The
[compatibility report](../../fixtures/world-generation-main-compatibility.json)
records the branch/base/master identities, exact patch/source hashes and
sanitized log digest. This confirms compiler/native-flow compatibility with
the later admission/chat code; production generation remains unconnected.

The portable probe sources and invocation are documented in
[`src/worldgen/validation/README.md`](../../src/worldgen/validation/README.md).
The NOISE comparison constructs the original NoiseChunk and calls its real
interpolation/block-state methods in source `doFill` order. It compares complete
canonical state arrays for seed `123456789` in all seven built-in noise settings,
plus an Overworld chunk at negative X. Each selected original/native array agrees.
The current replay also checks the original `:overworld` default-namespace alias
and seed `0` for the selected active badlands case.
This is direct original-class runtime execution, not execution of the original
server's generation scheduler or `doFill` itself.

The SURFACE comparison invokes the actual original SurfaceSystem on a real
ProtoChunk containing imported original NOISE block arrays, with real
FixedBiomeSource, BiomeManager and WorldGenerationContext owners. It constructs
a fresh original NoiseChunk for SURFACE; the native probe retains its NOISE
owner. Original cross-stage cache-owner continuity is not compared. The
controlled original biomes
are plains, eroded badlands, frozen ocean, swamp, nether wastes and the End.
Native NOISE-to-SURFACE output agrees on complete state arrays, both WG
heightmaps and surface postprocessing-count deltas. The selected surface counts
are zero; positions and list order are not verified by that comparison. Native
unavailable-biome rejection preserves the NOISE image/stage. A repeated SURFACE
call reuses the available context that succeeded, receives the specific
duplicate-stage error, invokes no biome/thermal callbacks and preserves the
completed image, both heightmaps, postprocessing and stage.
Frozen-ocean terrain includes active iceberg growth. The additional seed `0`
eroded-badlands case demonstrates positive pillar filling: the original adds
321 non-air voxels and raises 37 columns, matching the native output.
Thermal callbacks replay actual original biome observations and reject unknown
coordinates, rather than assuming a constant temperature.

The original `fillFromNoise` entry-point probe covers zero, negative and sub-cell
clamped heights. It uses actual stone-bearing ProtoChunks, with generation
dependencies unavailable, and verifies completed-future identity, untouched
blocks/status and no NoiseChunk allocation. Native `fill_from_noise` agrees on
clamped metadata and returns `NoCells` without a replacement terrain image.
Positive overlap still requires the real dependency observations.

The focused density fixture separately records the original visitor's ADD/MUL
reconstruction after holder removal. It preserves scalar/fill signed zeros and
input lifecycle errors that a stale raw multiply-by-zero could hide. The raw
pre-NoiseChunk evaluator remains unchanged. The aquifer quantization fixture
records Java's narrow-then-wrap conversion at ordinary and extreme inputs.

Separate source notes describe the
[density router](world-generation-density.md),
[random owners](world-generation-random.md),
[NoiseChunk lifecycle](world-generation-noise-chunk.md),
[aquifer](world-generation-aquifer.md) and
[surface rules](world-generation-surface.md).
The [integration contract](world-generation-integration.md) lists the missing
production owners and call phases. Main generation, natural multi-biome lookup,
nonempty blending/beardifier, full dimension generation, carving, features,
lighting, persistence and FULL publication remain unverified or unsupported.
No checklist row or progress metric is changed by these observations.
