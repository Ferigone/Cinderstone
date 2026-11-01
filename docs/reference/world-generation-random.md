# World-generation random ownership

This stage uses **direct `javap` inspection**, official mappings, and a focused
Java reflection probe. No REA tool or REA Evidence record was available.

The source is the Minecraft Java **1.21** server pinned in
[`source-lock.json`](../../fixtures/source-lock.json): outer JAR SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`;
SHA-256: `c9d9fda3143c100fa0566099074fc5d912b8b7ef3cb5b84c87eab411bd5bfb51`.
Neither Java artifacts nor raw disassembly are part of this source change.

| Observed method | Native contract |
| --- | --- |
| `RandomState.<init>` (`dzm`, mapped source lines 36–122), bytecode 4–18 | Select the settings' algorithm, create its source with the world seed, then fork one positional root. |
| Same constructor, bytecode 27–69 | Independently hash `minecraft:aquifer` and `minecraft:ore` through that root, then fork each resulting sequential source once. |
| Same constructor, bytecode 94–115 | Pass the original root directly to `SurfaceSystem`; there is no separate `surface` seed or extra surface fork. |
| `RandomState.getOrCreateRandomFactory` (line 129), lambda bytecode 0–15 | Cache by resource location; create a factory by hashing the location through the root and then forking positional. |
| `RandomState.getOrCreateNoise` (line 125), `Noises.instantiate` (`dzh`, lines 96–97) | Resolve the registry noise key, hash its canonical location through the root and invoke modern `NormalNoise.create`, including for a Legacy root. |
| `SurfaceSystem.<init>` (`dzq`, lines 60–79), bytecode 31–47 | Generate clay bands from `root.fromHashOf(minecraft:clay_bands)` directly, without an intervening positional fork. |
| `ResourceLocation.parse` / `bySeparator` (`akr.a`) | Missing or empty namespace becomes `minecraft`; preserve case and reject invalid characters instead of silently changing the hash input. |

`RandomStateRandoms::from_graph` copies the immutable factory that bound the
`DensityGraph`. `new(seed, legacy)` is the equivalent standalone constructor.
`positional_factory`, `aquifer_random`, `ore_random`, and `surface_random` expose
the corresponding immutable owners. Every positional sample constructs a fresh
sequential source; it does not consume a different chunk's source or the live
Level gameplay RNG. The API accepts no Level RNG.

The named noise accessor uses the existing pinned noise-parameter fixture. It
supports built-in parameters only and rejects absent keys. Dynamic registry
overrides remain unsupported. The density visitor's special legacy climate
bindings remain in `DensityGraph`; applying those substitutions to generic
`getOrCreateNoise` would alter the original behavior. Graph and generator caches
may hold separate immutable noise instances with the same seeded numerical
identity; the graph's existing bindings are not replaced or reseeded.

## Focused observation

A temporary Java probe created actual `RandomState` objects through the built-in
registry provider for overworld and nether, using world seeds `0`, `123456789`,
and `-987654321`. It read the original root, `aquiferRandom`, `oreRandom`, and
`getOrCreateRandomFactory(minecraft:bedrock_floor)`, sampling each with
`at(-17, -4, 23).nextLong()`. It also sampled the clay-band hash source directly
and evaluated the original `minecraft:surface` noise at `(-13.25, 0, 87.5)`.
The probe exited successfully. Its raw results are the literals in
`owners_and_named_noise_match_original_random_state` in
[`random_state.rs`](../../src/worldgen/random_state.rs).

The temporary probe source SHA-256 was
`0217f268eeb52a41c542301cc12ed4dd0f7792715673ab7d9d11ae4dfc33099f`;
the result text SHA-256 was
`afbd31146e5477245b1566f0278998e3bf3265dada8cd06f745f42e356e4f10e`.
These observations cover the sampled ownership chains and named-noise bits.
The focused native golden comparison passed in a temporary Cargo harness that
linked the repository's `random`, `noise`, `density`, and `random_state` source
files directly. The command was `cargo test --manifest-path
/workspace/worldgen-rng-check/Cargo.toml
owners_and_named_noise_match_original_random_state -- --nocapture`.
It verifies the recorded sequential samples, raw surface-noise bits, graph
factory identity, and named-cache reuse. This is a focused module result;
compilation of the composed generator is recorded separately in the PR.
These observations do not establish a complete terrain generator, Main
integration, or a finished checklist delivery.
