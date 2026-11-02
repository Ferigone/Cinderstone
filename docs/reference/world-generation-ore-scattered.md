# Scattered ore placement

This leaf implements the original Minecraft Java **1.21**
`ScatteredOreFeature.place` (`edh.a`, mapped lines 26–45),
`offsetTargetPos` (lines 49–53), and
`getRandomPlacementInOneAxisRelativeToOrigin` (line 56). Evidence is direct
Java 21 `javap -p -c` inspection of the pinned inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
with official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
No REA tools or Evidence records were used, and raw disassembly is not published.

The internal `feature::scattered_ore::place(host, config, random, origin)` leaf
requires the actual caller RNG and world host. External callers use
`feature::place_configured_ore` or `feature::place_builtin_configured_ore`. Their
ConfiguredFeature dispatcher performs the source origin `ensureCanWrite` check
before invoking this leaf and before any leaf draw.
The leaf creates no random source, takes no seed, forks nothing, and does not
clone or reseed the caller's source. It works with the caller's actual sampling
semantics, including a WorldgenRandom wrapper and its continuing draw counter.

The preserved sequence is:

1. Draw `nextInt(size + 1)`, even when size is zero and the bound is one.
2. For each attempt `i`, use spread `min(i, 7)`. Draw two floats for X, then two
   for Y, then two for Z. Each axis computes the float32 subtraction and float32
   multiplication before `Math.round(float)`. All six draws occur at `i = 0`,
   even though its spread is zero.
3. Add the three integer offsets to the origin with Java int wrapping. Read the
   actual block state once before iterating the target list, including when that
   list is empty. No visited mask removes repeated positions or repeated reads.
4. Evaluate targets in their original list order through the shared
   `OreFeature.canPlaceOre` helper. The same initial block state is supplied to
   each rule, while required exposure checks read actual neighboring blocks
   lazily. A failed rule or exposure check proceeds to the next target.
5. The first successful target calls the actual `setBlock(point, state, 2)`.
   Its boolean is ignored and target iteration stops even when that boolean is
   false. No extra candidate `ensureCanWrite` or build-height prefilter is added;
   the world read/set owners retain their own source behavior.

Java float rounding uses ties toward positive infinity: `-0.5f` becomes zero,
while the closest float below `+0.5f` remains zero. The native code widens the
already-computed float32 offset to double before adding one half and taking the
floor. Adding the half in float32, or using Rust's ties-away-from-zero `round`,
would change these source boundaries.

The return value is the original unconditional `true` after the loop, including
zero attempts, empty targets and zero successful writes. Host/provider errors
propagate; no world state or successful placement is fabricated. The shared
configuration/rule helper owns the closed pinned built-in target rules and their
conditional exposure RNG/query order. Unknown providers or registry overrides
are not inferred from matching names.

Compiler results and paired unchanged-original placement observations belong to
the coordinating validation stage. Explicitly seeded leaf probes are controlled
caller-RNG observations; they do not establish decoration seeding, natural ore
placement, Main integration, complete FEATURES generation or FULL publication.
