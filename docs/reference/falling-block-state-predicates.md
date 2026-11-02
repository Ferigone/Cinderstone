# Original 1.21 falling-block state predicates

[The compact certificate](../../fixtures/falling-block-state-predicates.json)
contains actual original `BlockStateBase.liquid()` and no-context
`canBeReplaced()` observations for the complete **26684-state** registry.
These facts support a real native falling-block owner. They are not a scheduler,
entity simulation, world-integration claim or compatibility/progress metric.

## Pinned actual acquisition

The original inner-server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
They match [source-lock.json](../../fixtures/source-lock.json). Actual bootstrap
execution used the verified Microsoft OpenJDK 21.0.13.0.2+1 and exited 0. Source
method inspection used direct JDK 21 `javap`, not an REA MCP tool.

The bounded generator bootstrapped the unchanged original classes, obtained
`Block.BLOCK_STATE_REGISTRY` (`dfy.q`) and queried its actual `IdMapper.size()`.
For every ID 0..size-1 it required a nonnull original state, then directly called
`liquid()` and `canBeReplaced()`. All 26684 IDs were observed; no holes, unknown
IDs, class inference or absent-state false values were inserted. The generator
and raw observation digests are in fixture provenance. Private query source,
raw output/disassembly and Java binaries are not published here.

The comparison independently loaded each existing placement catalogue block's
min/max state range and recorded replaceable flag. It required every original
state ID to be covered exactly once and compared the actual state predicate to
that flag. The observed mismatch count was **0**, with no mismatch IDs. This
verifies the current catalogue for this predicate; it does not assume all future
catalogues or new versions behave identically. Its exact SHA-256 is bound by
`placement_catalog_sha256` in the certificate.

## Source predicates and the liquid-class trap

| Original method | Obfuscated owner/method | Official line | Meaning |
| --- | --- | --- | --- |
| `BlockStateBase.liquid()` | `dtb$a.k()` |971| Actual stored liquid property of this state |
| `BlockStateBase.canBeReplaced()` | `dtb$a.r()` |1205| Actual no-context replaceability property |
| `FallingBlock.isFree(BlockState)` | `diu.m(dtc)` |61| Source OR of AIR, actual FIRE tag, liquid and no-context replaceability |

The actual liquid=true states are:

| Original block | Inclusive state IDs |
| --- | --- |
| Water |80..95|
| Lava |96..111|
| Bubble column |12960..12961|

The compact fixture merges the adjacent water/lava intervals into 80..111.
**LiquidBlock class membership is insufficient:** the all-state original query
found exactly two mismatches, the bubble-column states. All other states,
including waterlogged solids, observed liquid=false. Conversely, querying
whether a state's fluid is nonempty would incorrectly include waterlogged
solids. Neither substitute preserves this source predicate.

The original no-context replaceable=true ranges are stored separately; do not
conflate them with liquid. Their all-state values happen to match this exact
placement catalogue's per-block flags. Contextual placement replacement through
`BlockPlaceContext` is a different method and has no certificate here. The
actual configured FIRE tag and AIR predicate remain separate owners in
`FallingBlock.isFree`; these two arrays do not impersonate that complete query.

## Consumer contract

`schema_version:1` binds the data layout; `minecraft:1.21`, protocol 767 and the
original hashes bind its source. The two top-level range arrays contain
inclusive, sorted, nonoverlapping true intervals. The actual complete state
count is 26684. False is certified only for other IDs inside that observed
domain, never for an unknown or foreign ID.

A consumer must validate source/catalogue digests and range ordering/bounds,
resolve the requested state through the same known placement state domain and
reject IDs outside 0..26683 explicitly. A missing/invalid fixture or changed
registry has no class-based, fluid-based or permissive fallback. Consumers own
the actual configured-world/holder binding and their errors.

No source block callback, pending tick, Item/FallingBlockEntity owner, unknown
world history, block-entity state or RNG capability is supplied by this file.
No native gameplay campaign or new graphical/public client validation was run
for this source-data capture.
