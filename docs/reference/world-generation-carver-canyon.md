# Canyon geometry and random ownership

Target: unchanged Minecraft Java **1.21**, protocol 767, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Artifact and official mappings are pinned in `fixtures/source-lock.json`.
Inspection used direct Java 21 `javap`; no REA Evidence is claimed.

`CanyonWorldCarver` maps to `eaw`. The native
`worldgen::carver::canyon::{is_start_chunk, carve}` functions implement its
start test (mapped line 24), invocation (29–43), path (47–87), width array
(90–99), vertical radius (103–105) and skip predicate (110–111). They use the
single shared `world_carver` ellipsoid/material/mask kernel. Canyon does not
have a separate material loop, aquifer implementation or surface callback.

The start test always consumes one invocation-source float and compares it
inclusively with the configured probability. `carve` samples source-chunk X,
height, Z, yaw, vertical rotation, Y scale, thickness and distance in that order,
then consumes one invocation-source long. The source chunk passed to `carve`
may differ from the target chunk supplied by the live host. Integer chunk
coordinate arithmetic wraps as Java integer arithmetic does.

That long seeds `RandomSource.create(long)`, which the pinned original
implements with `LegacyRandomSource`. It is a separate local path owner: width
initialization, horizontal-radius samples, vertical jitter, direction changes
and path skips do not advance the invocation `WorldgenRandom`. No world-seed
factory or positional random is substituted. The caller retains ownership of
start-test sequencing, per-carver seed initialization and neighboring-origin
traversal.

Before any path step, the local owner initializes exactly `getGenDepth()` width
entries. Entry zero draws two floats without a bounded-int draw. Each following
entry first draws `nextInt(width_smoothness)`; only zero draws two replacement
floats. A replacement width is `1 + first * second`, and its square is stored.
The ellipsoid skip predicate reads entry **`blockY - minGenY - 1`** and evaluates
`(normalizedX² + normalizedZ²) * width + normalizedY² / 6 >= 1`. This relies on
the shared source kernel's strict lower-Y clip; no invented boundary width or
clamped index is used.

Path motion and radius calculations retain the original float operations before
widening to double. Sine and cosine use the existing pinned Minecraft `Mth`
float lookup table through `geometry::{vanilla_sin, vanilla_cos}`. Horizontal
radius sampling precedes vertical jitter. Each step then advances the center,
updates pitch/yaw and their float deltas, draws the one-in-four skip, and only
then checks reachability. A skipped step performs no reachability or ellipsoid
call. A failed reachability check ends this local path. The original public
`carve` result remains `true` even when a path changes no blocks; callers must
not interpret that boolean as a completed stage or a changed-block witness.

The pinned packaged `minecraft:canyon` definition uses width smoothness `3`,
uniform distance and horizontal-radius factors in `[0.75, 1)`, trapezoid
thickness with minimum `0`, maximum `6`, plateau `2`, uniform vertical rotation
in `[-0.125, 0.125)`, Y scale `3`, and absolute height `10..=67`. These values
come from the actual packaged configured-carver registry, rather than geometry
constants substituted by Canyon. Typed configuration/provider owners resolve
the values and replaceable block tag; the live host provides real generation
bounds and retained material owners. Custom registry/provider support is
limited by that typed configuration owner's declared scope.

This module implements Canyon's native geometry contract. Compiler/runtime
validation and the enclosing CARVERS stage comparison belong to the coordinated
composed-stage check; this source translation alone does not prove Main
integration, neighborhood scheduling, a persisted CARVERS status or FULL chunk
completion.
