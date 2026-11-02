# Existing target and partial-cell NOISE geometry

The target is the pinned Minecraft Java 1.21 server, protocol 767, in
`fixtures/source-lock.json`. Its inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The source evidence here is direct Java 21 `javap -p -c` inspection with the
official mappings. No REA execution or partial-cell runtime result is claimed
by this note.

## Two actual accessors

`NoiseBasedChunkGenerator.fillFromNoise` (`dzb`), mapped lines 291–319, clamps
the original `NoiseSettings` against `ChunkAccess.getHeightAccessorForGeneration`.
`NoiseSettings.clampToHeightAccessor` (`dzg`), lines 97–99, computes the maximum
of the two minima and the minimum of the two exclusive tops, then subtracts.
It constructs that exact minimum and height without rounding either one. A
disjoint range can retain a negative clamped height.

The fill entry takes `floorDiv(clampedMinY, cellHeight)` and
`floorDiv(clampedHeight, cellHeight)`. A nonpositive count returns the same
input chunk before NoiseChunk creation or material work.

In contrast, `NoiseChunk.forChunk` (`dzc`), lines 108–111, clamps its constructor
settings against the chunk itself. `NoiseChunk` retains this build grid and
derives its own minimum cell Y and full cell count. `ProtoChunk` (`dvs`), lines
340–343, returns the chunk for the generation accessor when not upgrading; an
actual BelowZeroRetrogen instead supplies its separate generation accessor.
The two grids must therefore remain distinct even when their minima happen
to match.

## Traversal and physical writes

`NoiseBasedChunkGenerator.doFill`, lines 324–413, selects cell Y relative to
zero in the retained NoiseChunk. Its emitted world Y instead uses the fill
entry's minimum cell Y. Slice filling, interpolation arrays and aquifer creation
still use the full NoiseChunk build grid. Rebuilding that owner for the shorter
generation window would change caches and preliminary-surface queries.

The source writes a non-AIR material directly through
`LevelChunkSection.setBlockState(..., false)`. It does not call
`ProtoChunk.setBlockState`, whose logical Y guard would discard out-of-range
writes. Existing AIR results are skipped. Untouched physical palette cells,
heightmaps and postprocessing queues belong to the existing target and must
not be replaced with a new AIR image.

For the built-in Overworld cell height 8, these are source-derived coordinate
examples, not runtime observations:

| Actual accessor minimum/height | Traversed block Y | Untouched logical tail | Descending acquired section Ys |
| --- | --- | --- | --- |
| `1 / 127` | `0..119` | `120..127` | `7..0` |
| `-63 / 127` | `-64..55` | `56..63` | `3..-4` |
| `7 / 17` | `0..15` | `16..23` | `1..0` |

The source async fill lambda, lines 303–317, acquires sections using the
unrounded interval from `clampedMinY` through
`clampedMinY + cellCountY * cellHeight - 1`, descending. That interval can differ
from the actual traversed block interval. It releases the same acquired owners
on success or failure. The native plan exposes absolute section Ys; the target
owner converts them to its actual section indices. It must neither expand nor
clip physical storage to hide a source-invalid index.

The unrounded minimum and height also remain in preliminary-surface metadata.
For `1 / 127`, its scan starts at Y 128 and decreases by 8 while at least 1.
Rounding the grid to `0 / 120` would change that scan and aquifer inputs.

## Native contract

`NoiseFillPlan::new(original_settings, generation_min_y, generation_height)`
preserves the unrounded generation clamp, signed floor division and source
section acquisition span. Its `grid()` is the generation clamp, not the
NoiseChunk build grid. It allocates no chunk, caches, blocks or status.

`NoiseChunk::new_with_blender` keeps the unrounded chunk-build grid.
`visit_blocks_with_plan` uses the separate generation window without replacing
that owner. Full slices and relative cell-cache selection remain unchanged;
world Y uses the plan. An actual unavailable cache/target index fails rather
than shortening the requested traversal. Material or sampling failure stops
interpolation and poisons the native one-shot attempt.

The existing `visit_blocks` entry delegates using the full retained build grid.
A no-cell plan makes no density or phase-state query, calls no material owner
and leaves an existing NoiseChunk unchanged. The outer actual-target entry
must perform the source early return before constructing or obtaining a
NoiseChunk at all; a chunk that already cached one retains it unchanged.

Physical target storage, its existing section owners/maps/queues and phase
integration are separate from this geometry module. These APIs do not fabricate
target contents, round a logical height accessor, certify retrogen provenance,
complete a ChunkStatus, or establish production Main integration.
