# Structure selection for Beardifier

Target: Minecraft Java **1.21**, protocol **767**. The unchanged original inner
JAR has SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence here is official mappings and direct Java 21 `javap` inspection;
no REA tools were exposed. This note describes source analysis and native
selection code, without claiming an original runtime or Main observation.

`Beardifier.forStructuresInChunk` (`dyl.a(ddq,dcd)`, mapped lines39–77) calls
`StructureManager.startsForStructure(ChunkPos,Predicate)` (`ddq.a`,51–61).
The manager reads the target chunk at **STRUCTURE_REFERENCES**, traverses its
actual reference-map entry order, and skips map keys whose terrain adjustment
is `NONE`. For each remaining actual LongSet reference, in its iterator order,
`fillStartsForStructure` (77–84) decodes the referenced ChunkPos, observes the
level's minimum section, requests that chunk at **STRUCTURE_STARTS**, and reads
its start for the same structure key. Null and invalid starts are skipped.
`StructureStart.isValid` (`ejz.b`,125) means its actual PiecesContainer is not
empty. Missing chunk/status dependencies are not equivalent to null starts.

The manager builds its complete ordered list before Beardifier traverses any
pieces. The native
[`select_beardifier`](../../src/worldgen/beardifier/selection.rs) preserves
this boundary and repeated starts. Its mandatory `BeardifierSelectionHost`
callbacks acquire the reference and STARTS owners and expose their actual
ordered contents. Opaque handles must belong to the same live manager/level;
unavailable owner, status, registry or piece observations return errors.
There are no default empty callbacks or successful provenance certificates.
Ordered snapshots of stable owners are supported; sorted registry names or
reconstructed neighbor lists do not supply the original iteration order.

For each valid start, Beardifier reads the adjustment from **that start's
structure**, then traverses its actual piece list. This is separate from the
reference-map key used by the earlier predicate. The native collector does
not silently substitute the key's adjustment or reapply the `NONE` filter.

`StructurePiece.isCloseToChunk` (`ejv.a(dcd,int)`,144–147) tests the piece's
BoundingBox against the chunk's horizontal block interval expanded by12.
`BoundingBox.intersects(int,int,int,int)` (`ejj.a`,106) includes both edges;
there is no vertical selection condition. The original block coordinates and
integer additions wrap, including at extreme chunk coordinates. The native
collector uses the same signed wrapping operations and comparisons.

An ordinary piece contributes one Rigid with its box, the start's adjustment
and ground-level delta0. A PoolElementStructurePiece (`ejn`) contributes a
Rigid only when its element projection is **RIGID**, using the actual piece's
ground-level delta. Both RIGID and TERRAIN_MATCHING pool pieces still traverse
their actual junction lists. A junction is retained only when both source X/Z
are strictly greater than `chunkMin-12` and strictly less than
`chunkMin+15+12`. Its source ground Y is retained without a vertical filter;
destination projection and delta Y do not participate in this selection.
Piece and junction order, as well as duplicates across starts, are preserved.

The result is a fresh immutable Beardifier containing the selected ordered
parts. Errors publish no result; actual host reads/acquisitions performed
before an error are not rolled back. The collector creates no structures,
references, replacement chunks or statuses and draws no random values. The
caller still needs actual structure generation/reference storage, registry
identity, stage dependencies and live owner adapters. A controlled host can
exercise the collector but does not establish WorldGenRegion, scheduler,
saved-world or production Main integration.

Focused native regressions use explicitly controlled owners to check the
source-derived lookup trace, map-key/start adjustment distinction, duplicate
start retention, both inclusive piece edges, strict junction edges and
terrain-matching junctions without a vertical filter. A missing STARTS owner
errors before any piece traversal; an observed null start is skipped. These
assertions are distinct from executing the original StructureManager. No
original selection runtime or compiler result is asserted by this note.
