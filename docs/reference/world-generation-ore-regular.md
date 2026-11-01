# Regular ore feature leaf

Target: unchanged Minecraft Java **1.21**, protocol 767, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Artifacts and official mappings remain pinned in `fixtures/source-lock.json`.
Inspection used direct Java 21 `javap`; no REA Evidence is claimed.

`OreFeature` maps to `ecz`: `place` is mapped lines 25–57, `doPlace` 61–175,
`canPlaceOre` 179–185, and `shouldSkipAirCheck` 189–195. The internal native
`feature/ore.rs` leaf implements regular ore geometry and delegates ordered
target/exposure checks to the one shared ore material helper. Public callers use
`worldgen::feature::place_configured_ore` with a typed `ConfiguredOreFeature`, or
`place_builtin_configured_ore` with a certified pinned registry key. Both take
the actual `OreHost`, caller-owned `RandomSource`, and block origin. The
configured-feature dispatcher owns the
original `Feature.place` origin `ensureCanWrite` gate (mapped lines 167–170),
which runs before this leaf and before any leaf random draw.

The leaf consumes its supplied source directly. It does not create a seed,
fork, positional factory or local RNG. Decoration seed/index/step selection,
placement modifiers, neighboring-chunk scheduling and any preceding feature
draws remain with their actual callers. The typed configuration owner supplies
the pinned ordered targets, size and air-exposure discard chance; a name alone
does not certify that a datapack has left those registry objects unchanged.

## Geometry and observable ordering

The invocation draws a float angle multiplied by float pi, then computes the
two X/Z endpoints with `Math.sin/cos(double)` of that widened float angle. It
uses the existing native `hotspot_math::sin_cos` implementation for the pinned
x86_64 HotSpot results. The half-extent is float `size / 8`; the bounding padding
retains the original float expression and `Mth.ceil`. The two Y offsets each
consume `nextInt(3)` before any world query.

The inclusive eligibility scan visits bounding X first, then Z, querying real
`OCEAN_FLOOR_WG` first-available heights. At its first `minimumY <= height`
observation it returns the result of one `doPlace` call, including a false
result. It does not continue scanning in search of a more favorable height.
When no position qualifies, there are no sphere-double draws, bulk access or
target queries. Size zero retains the angle and both bounded draws, the height
scan, and an empty bulk-access lifetime when eligible; it returns false.

All `size` spheres are initialized before culling or material work. The
interpolation fraction is float `i / size`, widened for double lerp. Each sphere
consumes one double, and its radius uses the original float `Mth.sin` table
through `geometry::vanilla_sin`, not the endpoint's double trig path. Culling
uses strict radius-difference-squared containment. It continues the inner loop
after marking the outer sphere dead and rereads its modified radius, matching
the source rather than adding a break or snapshot.

Surviving spheres visit inclusive ascending X/Y/Z coordinates with the original
double normalized-distance comparisons. The upper coordinate bounds use
`Math.max`, as in the original. Outside-build-height rejection precedes the
visited-bit lookup. The bit index is
`(X-minX) + (Y-minY)*horizontalSpan + (Z-minZ)*horizontalSpan*verticalSpan`.
The source bit is set before write permission, section acquisition and targets;
a denied candidate remains visited and is not retried by an overlapping sphere.
The initial bit-set size is a capacity, so subsequent indices can grow it.

After permission succeeds, the leaf obtains the real section and captures the
current block state once. Ordered targets all test that captured state. Shared
`can_place_ore` preserves target matching, conditional exposure draws and lazy
Down/Up/North/South/West/East air reads. The first passing target performs one
direct section write and ends the target loop. The original result counts write
attempts, including a same-state replacement; it does not report changed voxels
or a completed generation status.

## Required section and region owners

The original constructs `BulkSectionAccess` (`duw`, mapped lines 15–60) after
sphere generation and culling, even with no spheres. The host must begin that
scope, cache the actual sections, acquire each on first access, and release all
acquired sections when it closes. The leaf closes once on normal returns and
propagated errors. If both placement and close fail, the original placement
error remains in the native error chain with the close failure recorded as
suppressed context. An unsuccessful construction has no close call.

`bulk_section_block` returning `None` means the actual section index lies outside
the accessor's section array. It does not mean a missing neighbor can be treated
as air. Section-array coverage and build-height bounds can differ for unaligned
accessors. Bulk neighbor reads return ordinary air for a null source section;
in-range missing chunks or unavailable owners return explicit errors. The
caller must provide real region dependency/write-radius and retrogen bounds
for `ensureCanWrite`, not derive permission from a convenient in-memory image.

The replacement is exactly `LevelChunkSection.setBlockState(..., false)`.
`direct_section_set` must mutate that acquired section and maintain its real
section counters, without calling `WorldGenRegion.setBlock`, updating ProtoChunk
heightmaps, or appending postprocessing positions. Scattered ore has a different
source write path and does not use this bulk scope. Existing generic terrain
writes that update WG heightmaps are not interchangeable with this API.

The implementation is a regular-ore leaf. Paired original/native execution,
source-backed host lifecycle checks and compiler validation are coordinated
with its enclosing dispatcher. This source translation alone does not prove
placed-feature composition, natural decoration, Main calls, persisted FEATURES
status or FULL chunk completion.
