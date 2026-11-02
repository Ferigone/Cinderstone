# Process-seeded Xoroshiro construction

Target: unchanged Minecraft Java **1.21**, protocol **767**, pinned by
`fixtures/source-lock.json`. The inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Inspection used direct Java 21 `javap`; no REA execution is claimed.

`RandomSupport.generateUniqueSeed` (`dzn.a`, mapped line 50) first performs the
shared `AtomicLong.updateAndGet`, multiplying by `1181783497276652981` with
64-bit wrapping. It then samples `System.nanoTime` and XORs the two full-width
values. The initial shared counter is `8682522807148012`. Original bytecode
offsets 0–15 establish the update-before-clock order.

`XoroshiroRandomSource(long)` (`dzz`, mapped lines 21–25) passes the complete
long to `RandomSupport.upgradeSeedTo128bit` and constructs its Xoroshiro owner
without drawing an output. The seed must not first pass through Legacy's
48-bit state. `ChunkGenerator.applyBiomeDecoration` (`duz.a`, mapped lines
280–383), offsets 90–107, constructs
`WorldgenRandom(new XoroshiroRandomSource(generateUniqueSeed()))` before its
decoration reseed at offsets 109–127. That later reseed does not remove the
earlier shared-counter and clock observations.

The additive native API is
`random_seed::create_xoroshiro_random() -> io::Result<XoroshiroRandomSource>`.
It uses the existing process-wide uniquifier and Linux monotonic-clock owner,
preserves the full signed 64-bit seed, and makes no sampler, fork or extra
unique-seed call. An unavailable clock propagates its error after the existing
atomic update. The helper returns the underlying source; the caller remains
responsible for the original `WorldgenRandom` wrapper and subsequent reseeds.

This change does not implement biome decoration, feature scheduling, placement
modifiers or a Main call. Focused compiler and factory-flow validation belongs
to the stage coordinator's validation record.
