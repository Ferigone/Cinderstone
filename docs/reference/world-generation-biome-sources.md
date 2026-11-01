# Climate RTree selection over original parameter lists

Target: Minecraft Java **1.21**, protocol **767**. The original inner server
SHA-256 is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
The source inspection used direct Java 21 `javap -c -p`, with no REA tool or
invented Evidence record. Original runtime witnesses are identified by the
parameter and search fixtures below.

| Original method | Required behavior |
| --- | --- |
| `Climate.ParameterPoint.parameterSpace` (`def$d`, line 375) | Seven intervals in order: temperature, humidity, continentalness, erosion, depth, weirdness, then the point interval `[offset,offset]`. A target contributes zero in its seventh coordinate. |
| `Climate.Parameter.distance(long)` (`def$b`, lines 419–424) and `RTree.Node.distance` (`def$e$b`, lines 85–89) | Measure each interval's distance with signed-long subtraction, square and sum with Java wrapping arithmetic. |
| `Climate.ParameterList` (`def$c`, lines 288–329) | Preserve the ordered values, build an RTree, and use that index for production `findValue`. The separate brute-force routine has different tie behavior and is not the production substitute. |
| `Climate.RTree.build/sort/bucketize/cost` (`def$e`, lines 160–249) | Preserve stable sorting, six-child grouping, the strict lowest-cost choice and recursive tree shape described below. |
| `Climate.RTree.search` (`def$e`, lines 270–273) | Read and replace this tree's worker-local previous leaf. No random generator or seed participates in selection. |
| `Climate.RTree.SubTree.search` (`def$e$c`, lines 126–141) | Visit a child only when the current best distance is strictly greater than its bound, and replace the best leaf only on strictly smaller fitness. Equal fitness retains the prior winner. |

The native `ClimateParameterList<T>` retains the original row order and generic
values. Construction rejects a ranged seventh interval: the original
`ParameterPoint` stores one scalar offset and exposes it as `[offset,offset]`.
It shares one immutable tree through an `Arc`; cloning the list shares
that identity. `worker()` creates a `ClimateSearch<T>` for that tree with no prior
result. Reuse this worker across its actual query sequence and chunk tasks.
The cursor is neither `Send` nor `Sync`, preserving the worker-local ownership
that the original tree implements with `ThreadLocal`. It cannot be retargeted
to an equal-but-distinct tree. The list is shareable when its values are
`Send + Sync`.

`find_index` returns the original ordered input row, and `find_value` returns
that row's supplied value. The previous leaf is observable on ties: after a
query chooses one of two equally distant candidates, a later tie retains that
candidate. A fresh worker instead uses the first candidate in the tree's
traversal order. Neither behavior is necessarily the first row in the input
list. No random tie-break or fallback value is introduced.

For at most six children, the original stable sort key is the sum of the
absolute centers of all seven intervals. Each center is the wrapped sum of its
endpoints divided by two with truncation toward zero. For larger nodes, each
possible starting axis defines a cyclic, lexicographic signed-center sort of
all seven dimensions. Sorted children are bucketed with width
`6^floor(log(size - 0.01) / log(6))`; for Java's integer list size, this is the
largest power of six strictly below the size. Native integer powers retain that
result without relying on platform logarithm rounding. Bucket cost is the
wrapped sum of absolute interval spans. Only a strictly lower total cost replaces
the chosen axis, so the first axis wins equal costs. The chosen buckets are
then stably sorted by cyclic absolute-center keys before recursively building
their children.

`fixtures/world-generation-multinoise-parameters.json` records the actual ordered
preset entries extracted from the original registry, including the quantized
long endpoints. The packaged preset JSON only identifies `minecraft:overworld`
or `minecraft:nether`; it does not contain these generated parameter vectors.
The original search witness in
`fixtures/world-generation-biome-source-vectors.json` uses a parallel original
`Climate.ParameterList<Integer>` with the same ordered parameter-point instances
and row-index tokens. It checks those indices against the actual holder-valued
preset's selected biome, avoiding assumptions about duplicate holder identity.
Controlled symmetric-point query sequences independently record cold-start and
warm tie behavior. Native focused checks consume those exact ordered sequences;
compiler and execution results belong in the coordinating validation record.

This module supplies parameter selection only. It does not construct a biome
registry, sample a replacement climate, populate BIOMES sections or publish a
chunk status. `MultiNoiseBiomeSource` must receive the actual seeded climate
sampler and the correct preset/registry owner. A complete generator path must
also preserve effective resolver ordering: base source, Blender wrapper, then
retrogen wrapper. Nonempty blending or retrogen needs existing biome owners;
absence is not an empty-context certificate. The End uses its separate
`TheEndBiomeSource` and erosion query, not this preset tree. Natural BIOMES
population, Main integration and FULL conversion remain separate deliveries.
