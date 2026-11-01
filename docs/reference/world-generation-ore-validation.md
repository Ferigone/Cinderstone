# Configured ore leaf observations

The native configured regular-ore and scattered-ore leaves produce the same
canonical block arrays, ordered host events, WG heightmap observations and
caller random states as the unchanged original in the selected controlled
material cases. These are leaf observations over actual original ProtoChunks
and section palettes. They do not execute WorldGenRegion, placed-feature
modifiers, decoration ordering, the complete FEATURES stage or Main generation.

The target is Minecraft Java 1.21, protocol 767. The loaded original OreFeature
CodeSource is checked against inner SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The outer server and official mappings are also checked against
`fixtures/source-lock.json`. Analysis used direct Java 21 `javap` and actual
original execution. REA tools were unavailable; no REA Evidence is claimed.
The unchanged ore implementation is commit
`2de9240eae9ffddeeab68e234ef2d542e788e411`; the validation reports also identify
the composed native source commit and hash the implementation, input catalogs,
shared dependencies and probes. Evidence must be regenerated when those change.

| Actual original entry | Observation |
| --- | --- |
| `ConfiguredFeature.place` (`ebq.a`, mapped line 28), through `Feature.place` (`ece.a`, lines 167–170) | Origin permission runs before every leaf random draw. Rejected regular/scattered origins consume no random values or material queries. |
| `OreFeature.place` (`ecz.a`, lines 25–57) | Height preflight can return false after the initial three draws without entering bulk access. A below-height geometry case performs source outside-Y rejection before candidate permission/section access. |
| `OreFeature.doPlace` (`ecz.a`, lines 61–175) | Regular veins write actual acquired sections, including cross-chunk material, and retain the selected write section when exposure queries acquire another section. Missing neighbors throw after earlier writes and still release acquired sections. |
| `OreFeature.canPlaceOre` (`ecz.a`, lines 179–185) | Ordered stone/deepslate iron targets both produce real replacements. Partial and full air-discard configurations preserve conditional random draws and actual lazy neighboring reads. |
| `BulkSectionAccess` (`duw`, lines 15–60) | First reads acquire/cache actual section owners. A source null-section neighbor below the accessor reads ordinary AIR. An unavailable in-range neighbor raises an error and never becomes air. |
| `ScatteredOreFeature.place` (`edh.a`, lines 26–45) | Actual nonzero attempts mutate real ProtoChunks through the controlled region-write callback. A candidate outside the selected write radius returns false; the leaf ignores that boolean and preserves its source target-loop behavior. |
| Same scattered method | An actual zero-attempt draw returns true with one wrapper draw and no material reads/writes. That return value is not placement or stage-completion evidence. |

The regular copper missing-neighbor case uses caller seed 1, origin
`(14,42,14)`, one available target chunk and write radius one. It performs 26
actual writes, acquires one section, then encounters an unavailable neighbor.
Original/native arrays, earlier events and post-error RNG states match; the
acquired section is released. This explicitly exercises cleanup after mutation,
not only an error during the preflight height query. It does not assert rollback
of those earlier changes.

The original reference also resolves every packaged configured ore from its
actual registry. It observes original size, raw float discard bits, ordered
canonical target states and actual BlockMatchTest/TagMatchTest decisions over
selected material states. Those observations match the native typed catalog.
This is the closed ore subset, not the full configured/placed feature registry.
Source resources are read from the original JAR; expected native values are
not supplied to the reference. Actual block/fluid tags are bound with the
original TagFile codec, TagLoader and Registry.bindTags. Basic bootstrap without
that step would leave holder memberships unbound and invalidate the comparison.

Each placement receives an explicitly seeded actual caller
`WorldgenRandom<XoroshiroRandomSource>`. Its wrapper count and underlying
128-bit state are compared after successful, rejected, zero-attempt and failing
calls. The harness does not pretend that this caller construction reproduces a
decoration factory, feature index/step, prior decorator draws or placement
modifier pipeline. Neither leaf constructs or reseeds its caller stream.

`WorldgenOreOracle.java` uses a rejecting WorldGenLevel interface proxy. It
routes allowed calls to real original ProtoChunks, real heightmaps and observing
LevelChunkSection subclasses whose storage/lock methods call the original
implementation. Unhandled interface methods throw. The explicit material
profiles are solid stone/deepslate or netherrack with a known top and an optional
air plane; they are not natural generated terrain. All ProtoChunk statuses
remain EMPTY. The source full block arrays are compared with an independently
implemented native fixture adapter in `ore_probe.rs`; that adapter is validation
code and is not a production OreHost or missing-world fallback.

Original scattered writes call actual ProtoChunk.setBlockState. EMPTY already
maintains both WG heightmaps. In these closed solid-to-solid replacements their
height predicates remain unchanged, so the actual primed arrays remain equal.
The native fixture verifies unchanged canonical height predicates and rejects
other transitions. It does not use a false claim that EMPTY has no heightmaps.
Regular direct section writes bypass ProtoChunk heightmap updates as required.
The controlled region adapter does not supply ServerLevel POI callbacks,
generalized block-entity handling or state-requested postprocessing; no parity
for those callbacks or queues is claimed by this report.

The event comparison covers origin/candidate permission, height preflight,
outside-Y checks, null-section Y queries, actual chunk/section acquisition,
bulk reads, direct section writes, world reads and region writes. It preserves
encounter order rather than sorting events or writes. Actual original section
acquire/release counters must balance on success and error. Native bulk sessions
must likewise close and release all retained sections; scattered leaves must
never begin one. The original BulkSectionAccess's internal close iteration order
is not represented by the normalized event stream.

The original JAR is signed and its Minecraft classes occupy Java's default
package. The checker keeps those classes in the parent loader and loads the
unsigned observing subclasses through a child URLClassLoader, generated by a
small bootstrap in the external work directory. This preserves original target
bytes and CodeSource identity. No rewritten JAR, Java binary or raw disassembly
is published.

Run from a Java 21/Rust environment with the pinned oracle files and extracted
libraries available externally:

```sh
python3 src/worldgen/validation/check_ores.py \
  --oracle-dir /path/to/pinned-oracle \
  --work-dir /path/outside/checkout/ore-check \
  --runtime-report fixtures/world-generation-ore-runtime.json \
  --report fixtures/world-generation-ore-validation.json
```

The checker builds the current native library explicitly before compiling its
probe. `--skip-build` is only for a separately completed `cargo build --lib` of
the same source and target directory; it must not select a stale rlib after a
compiler-only check. Private arrays and generated classes stay in the external
work directory. Committed reports retain hashes and sanitized observations,
not machine paths or original binaries.

Size-zero custom configurations and empty target lists have focused native and
static source checks, but are not configured overrides in this paired runtime
set. Random RuleTests, datapack overrides, arbitrary height predicates and
unaligned section fixtures are outside these observations. A production host,
actual neighborhood/status scheduler, complete FEATURES composition, persistence,
lighting, spawning and FULL publication remain separate delivery contracts.
