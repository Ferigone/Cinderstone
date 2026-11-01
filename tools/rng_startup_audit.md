# Runtime RNG initialization — original Java 1.21

Read-only bytecode evidence, not a native startup-seed conformance claim.
The original inner JAR SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

`RandomSource.create()` (`ayw.a()`) calls `RandomSupport.generateUniqueSeed()`
and constructs `LegacyRandomSource` (`dyz`). `createThreadSafe()` (`ayw.b()`)
constructs `ThreadSafeLegacyRandomSource` (`dzr`) with another unique seed.
Both use the same 48-bit LCG and Java bit-stream sampling. The latter retries
atomic compare-and-set; the normal Legacy source rejects concurrent misuse.
Sequential sampling can use the native Legacy core when the host enforces
single ownership; that does not prove concurrent exception/CAS behavior.

`createNewThreadLocalInstance()` (`ayw.c()`) constructs
`SingleThreadedRandomSource` (`dzo`), seeded by
`io.netty.util.internal.ThreadLocalRandom.current().nextLong()`. It does not
construct a Minecraft class named ThreadLocalRandomSource. This path is not
the normal Entity or Level.random initializer. dzo's stored 48-bit state uses
the same LCG; Gaussian cache behavior is a separate concern.

`RandomSupport` (`dzn`) initializes one process-wide AtomicLong to
`8682522807148012`. Each unique-seed request atomically multiplies it by
`1181783497276652981`, wrapping modulo 2^64, returns the **new** value and XORs
it with `System.nanoTime()`. A native helper can expose this deterministic
transition with an explicitly supplied clock sample; it must not pretend that
an unrelated clock or default seed reproduces a particular original JVM run.
Matching unseeded cross-process trajectories requires captured clock samples
and the full shared uniquifier call order. World seed is not used here.

The original Level constructor (`dcw`) requests seeds in this order:

1. A temporary `RandomSource.create()`, consuming nextInt for `randValue`.
2. A fresh `RandomSource.create()` stored as Level.random (`z`).
3. `RandomSource.createThreadSafe()` stored as threadSafeRandom (`f`).

These are independent streams. The temporary random must not be reused as the
level random, and the level random must not be seeded from level.dat's world
seed. Other original bootstrap code can also advance the shared seed uniquifier.

Entity's constructor creates a fresh Legacy source (`bsr.ah`) and consumes two
nextLong calls for its UUID. ItemEntity's constructor then consumes one float
for bob phase and one float for visual yaw. The ordinary item constructor also
consumes two nextDouble calls from Level.random for default X/Z velocity before
player drop replaces that velocity. `spawn_player_drop` already checks these
draws in its independent 2,400-case original-class corpus. The host must retain
the same **post-construction entity RNG object/state** for subsequent physics,
separately from the player RNG and level RNG; recreating it from the initial
seed would repeat UUID/bob/yaw draws.

No `random.rs` edits were made by this audit. Suggested next boundary is a
seed-uniquifier transition with injected nanoTime, plus host-owned startup clock
and explicit stream objects. Do not implement a deterministic first seed by
XORing a freshly-created Instant's zero elapsed time.

Sources: `tools/rng_startup_ayw_bytecode.txt`,
`rng_startup_dzn_bytecode.txt`, `rng_startup_dzr_bytecode.txt`,
`rng_startup_dzo_bytecode.txt`, `rng_startup_level_bytecode.txt`,
`tools/item_runtime_entity_bytecode.txt`, and
`tools/item_runtime_item_bytecode.txt`.
