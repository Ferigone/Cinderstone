# Cave and Nether cave geometry

This stage ports the original **Minecraft Java 1.21** CaveWorldCarver and its
Nether geometry overrides into `worldgen::carver::{cave,nether}`. It borrows the
actual invocation RNG and dispatches ellipsoids to the shared WorldCarver
kernel. It does not create a world, choose carvers from substitute biomes,
construct an aquifer or certify CARVERS/FULL status.

Evidence is direct Java 21 `javap -p -c` inspection using the pinned inner server
SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
and official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
No REA tool or Evidence record was available. Raw disassembly and Java binaries
remain outside the repository.

| Original mapped method | Preserved native behavior |
| --- | --- |
| `CaveWorldCarver.isStartChunk` (`ebb.a`, line 24) | One invocation-source `nextFloat`, with the inclusive `<= probability` comparison. |
| `CaveWorldCarver.carve` (`ebb.a`, lines 29–63) | Nested bounded system-count draws; per-system X, height-provider Y, Z, horizontal/vertical multipliers and floor-level samples; optional room and its tunnel-count draw; per-tunnel yaw, pitch, thickness, end and seed draws in the original order. |
| `getCaveBound` / `getThickness` / `getYScale` (lines 67–79) | Bound `15`; thickness `nextFloat()*2 + nextFloat()`, then the conditional `nextInt(10)` enlargement; initial tunnel Y scale `1`. |
| `createRoom` (lines 83–89) | Radius `1.5 + double(Mth.sin(float(pi/2))*thickness)`, supplied room Y scale, and center X offset `+1`. No independent room RNG is constructed. |
| `createTunnel` (lines 92–135) | Independent Legacy RNG from the supplied long seed; branch-index and gentle-pitch draws before stepping; float32 angle/velocity evolution; independent recursive children in left-then-right order; random step suppression before reachability and kernel invocation. |
| `shouldSkip` (lines 139–143) | Normalized Y at or below the sampled floor is skipped; otherwise the left-associated double squared-distance comparison skips values at or outside radius one. |
| `RandomSource.create(long)` (`ayw.a`) | A fresh `LegacyRandomSource(seed)`, without a WorldgenRandom wrapper or positional fork. |
| `NetherWorldCarver` (`ebd`, lines 29–39) | Bound `10`; thickness `(nextFloat()*2 + nextFloat())*2`, with no extra enlargement draw; initial tunnel Y scale `5`. |

The initial cave length is `sectionToBlockCoord(getRange()*2-1)`, or `112` for the
original range `4`. A tunnel's end subtracts `nextInt(28)`. The source samples
height through its actual CarvingContext bounds; it does not clamp all tunnel
centers to the target chunk or replace an unavailable height provider.

All Mth sine/cosine calls use the existing native `geometry::vanilla_sin` and
`vanilla_cos` float32 lookup implementation. Angle constants retain the source
float32 values of pi, pi/2 and tau. Products of float samples, angle updates and
motion components narrow as float32 before promotion into double positions or
radii. The radius multipliers and captured floor level are promoted from their
actual float-provider samples to double exactly where the original does so.

Random ownership is deliberately separate. The enclosing applyCarvers owner
supplies its real `WorldgenRandom<LegacyRandomSource>`; these modules neither
reseed it nor invent its construction seed. Each top-level tunnel seed is drawn
from that invocation source, while the tunnel's internal draw stream is a newly
constructed local Legacy source. Recursive child seeds and thicknesses come from
that local parent source. The left child finishes before the parent draws the
right child's seed and thickness. Child tunnels start at the parent's current
step and use Y scale `1`, including children of a Nether tunnel whose initial
scale was `5`. Flattening these local streams into the invocation RNG would
change both terrain and the invocation wrapper's draw counter.

Nether replacement behavior is dispatched through the same shared kernel's
`WorldCarverKind::Nether` branch. It is not an alternate geometry kernel and
must not consume or fabricate an aquifer result for the original Nether block
override. Registry replaceability, target block mutations, masks, fluid handling,
heightmaps and surface repair remain the shared kernel and actual host owners.

The public module contract is `is_start_chunk(config, random)` and
`carve(host, config, random, source_chunk, mask)`. The latter returns the original
boolean `true` after its geometry loops, even when no system or block is carved;
that boolean is not chunk completion evidence. Host/provider errors propagate.
The accepted configuration scope is the separately decoded pinned built-in
carver definitions and their supported providers, not arbitrary datapack carvers.
Compiler and unchanged-original geometry/material observations are recorded by
the coordinating validation stage after these modules are composed.
