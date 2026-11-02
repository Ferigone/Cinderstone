# Existing-target retrogen and BIOMES

Target: Minecraft Java **1.21**, protocol **767**. Original inner-server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings are pinned by `fixtures/source-lock.json`. Source analysis
uses direct Java 21 `javap` inspection, not REA execution. The paired runtime
observations below have a separate, controlled scope.

| Original source | Native requirement |
| --- | --- |
| `BelowZeroRetrogen` (`dym`), 59–73 | Keep the actual target status and complete Java BitSet. CODEC rejects EMPTY; direct NBT read of EMPTY returns no retrogen. |
| `dym`, 78–109 | Replace BEDROCK in local X/Z 0–15 and inclusive Y 0–4; apply the stored hole columns across the chunk's current generation accessor. |
| `dym`, 114–127 | Wrap Blender's resolver, evaluate its candidate first, retain three registered cave keys, otherwise read the same chunk at quart Y zero. |
| `ProtoChunk` (`dvs`), 235–247, 340–343 | Highest status is the maximum of persisted and retrogen target. Clear the slot only when actual persisted status reaches target. Retrogen generation height is `[-64,0)`. |
| `ChunkAccess` (`duy`), 418–441 | Queries clamp Y to the full level accessor; BIOMES fills only generation-accessor sections. |
| `LevelChunkSection` (`dvj`), 188–199 | Visit local quart X/Y/Z in that order; publish the completed section palette after all 64 calls. |
| `ChunkStatusTasks` (`dwb`), 63–77 | After successful fillFromNoise, run bedrock replacement, then the mask if it has any set bit, before outer stage completion. |

`retrogen.rs` reuses the central `chunk_readiness::ChunkStatus` identity. Its
payload and `RetrogenSlot` represent data and the actual observed slot; they do
not certify readiness. There is no Default slot. Unavailable observations error,
and a present slot retains the same immutable Arc. The status-clear helper must
be called only after the real owner applies a persisted-status update; it does
not apply that update itself.

BitSet words are not truncated to 256 bits. Trailing zero words are removed as
by `BitSet.valueOf`/`toLongArray`; all high nonzero words survive. `hasBedrockHoles`
observes the whole set, whereas a column uses bit `(z & 15) * 16 + (x & 15)`.
Thus a high-only bit still selects the source mask call without inventing a
matching column. NbtOps CODEC accepts its LONG_STREAM forms and ignores a
malformed optional mask; the direct CompoundTag reader accepts LongArray only
and otherwise reads an empty mask. Unknown or invalid target identifiers are
reported as unsupported/unavailable data, never converted to an absent slot.

Bedrock replacement uses X-fastest, then Y, then Z iteration, and compares
actual BEDROCK block identity. Only matching blocks become default DEEPSLATE.
The mask uses X outer, Z inner and ascending Y, writing default AIR without a
material predicate. All positions passed to the live logical ProtoChunk setter
are local X/Z. The setter is the source `setBlockState(..., false)` path, which
owns its actual section/count/heightmap effects. Helpers do not clear queues,
alter status, replace the slot or draw random values. Raw fillFromNoise and the
outer NOISE-task continuation are separate: the continuation also runs after a
successful no-cell early return.

The additive `biomes::fill_target_biomes` operates on `GenerationTargetHost`.
It retains existing outside palettes, fills only current generation sections,
and commits each completed section with canonical Y/Z/X slots. Retrogen rewrites
sections -4 through -1; existing zero-and-above palettes remain available for
fallback. It does not build a uniform replacement image or synthesize BIOMES
on an unknown chunk.

Resolver composition is source resolver, then Blender, then retrogen. Blender
may select an actual old biome without invoking its fallback. Retrogen always
evaluates that combined candidate once, preserving the retained sampler/RTree
history even when its result is discarded. Only actual bound registry keys
`minecraft:lush_caves`, `minecraft:dripstone_caves` and `minecraft:deep_dark`
survive. `ResolvedBiome` carries an explicit optional key; a matching unbound
display value does not pass this test. A selected stored Blender biome obtains
its key from the mandatory actual registry callback.

Other candidates use the same target's `get_noise_biome(qx,0,qz)`, including
its real highest-status guard and full-level quart-Y clamp. An actual old Ocean
value is preserved here; Ocean is not an invented default. The source's fresh
section default and ChunkHeightAndBiomeFix fallback are PLAINS. Neither can
replace unavailable old palettes or a missing registry owner.

The existing fresh `PreparedBiomes::fill` contract remains unchanged. Live-target
generation requires actual blocks, palettes, accessors, retrogen, registry and
stage owners from the separately owned target/session adapters. These helpers
alone establish no Main invocation, persistence compatibility, successful
ChunkStatus future or FULL publication.

The actual paired results in
[the validation fixture](../../fixtures/world-generation-existing-target-validation.json)
and [runtime fixture](../../fixtures/world-generation-existing-target-runtime.json)
match complete phase block images, palettes and metadata. The active-retrogen
case rewrites only the lower BIOMES sections, preserves all upper palettes and
positively exercises old quart-Y-zero Ocean/Plains fallback. It observes 1,280
BEDROCK-to-DEEPSLATE replacements and 115 masked-to-AIR changes; the NoCells
case still runs the continuation and observes 1,280 replacements plus six
masked changes, with no NoiseChunk cache created. Ordered existing queue
prefixes are preserved. The source retains its full-height 384-block NoiseChunk
while the retrogen generation accessor spans 64 blocks.

Both controlled retrogen cases keep persisted status EMPTY. Highest SPAWN
comes only from the supplied actual retrogen metadata and proves no SPAWN work
or status advancement. Registered cave retention, matching unbound cave
rejection and a high-only mask are not observed runtime branches; their
semantics have source/unit evidence only. The supplied payload retains a
nonzero high BitSet word. Nonempty source Beardifier influence remains
unsupported, and these cases establish no whole-checklist-item completion.
