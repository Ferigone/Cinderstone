# Existing-target generation contract

`worldgen::generator::TargetGeneration<T>` is an additive session over the
caller's same `Box<T: GenerationTargetHost>`. The box, physical sections,
existing block/biome palettes, heightmaps and postprocessing queues are retained.
The session separately retains its exact `Arc<NoiseGenerator>`, wrapped density
cache and aquifer. It never replaces the input with a cloned `TerrainImage`.
This is native staged data, not a ChunkStatus, scheduler result or FULL chunk.

The original reference is the pinned Java 1.21/protocol 767 server, inner JAR
SHA256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence here is direct official-mapping/javap inspection, not REA execution.
Paired validation for this additive implementation is pending coordinator work;
earlier fresh-target or original-only upgrade observations do not validate this
new native path by themselves.

## Owners and accessors

Four domains remain separate:

- Logical build bounds govern ordinary ChunkAccess reads/writes.
- The actual section array can contain palette cells outside those logical
  bounds. Its capacity is not expanded to make a derived write succeed.
- `getHeightAccessorForGeneration` supplies the NOISE/BIOMES window. An actual
  BelowZeroRetrogen supplies the original `[-64,0)` accessor.
- The actual region accessor and generator define WorldGenerationContext
  anchors for SURFACE, independently of the target's generation window.

NoiseChunk.forChunk and aquifer construction clamp against the full build
accessor. `NoiseFillPlan` separately clamps NOISE traversal against the generation
accessor, retaining the unrounded minimum/height and Java floor division.
For H=8, min=1,height=127, direct palette writes cover Y=0..119 while old data in
Y=120..127 remains. This is not a request to round the logical accessor or fill
its remainder with air.

The target contract requires actual direct section leases, unprimed WG maps,
logical ProtoChunk writes, biome palette commits, postprocessing and registry
facts. `storage::VanillaWorld::set_block` and gameplay edits are different paths;
they are not implicit implementations of the generation host.

## Public session phases

`TargetGeneration::new(generator, target, blender, structure_beardifier)` stores
owners without querying the target, allocating a NoiseChunk, or assuming empty
blocks/maps/queues. Its local phase is `Prepared`.

`with_sampler` creates/reuses the full-build cache and passes a
`TargetClimateSampler` implementing the existing climate and raw-erosion traits.
It constructs no replacement graph/random/cache owner. Caller-owned biome
selectors and worker-local search history remain outside the session.

`fill_biomes` uses this sampler and the actual retained Blender, then the actual
retrogen resolver. The callback returns a `ResolvedBiome` containing separate
display identity and optional registered key. This preserves unbound-holder
behavior; the helper does not infer registry membership from a string. Actual
generation-accessor sections are committed after each successful 64-cell fill;
existing other sections remain. Failures retain prior section/cache/source
effects and poison the session.

`fill_from_noise` is the material-only generator boundary:

1. Read only the generation accessor and form the fill plan. A nonpositive cell
   count returns `TargetNoiseOutcome::NoCells` with no coordinate/build/cache,
   section, map or material access. Any previously created cache remains intact.
2. Acquire the actual sections top to bottom using the source lock span, then
   create/reuse the full-build NoiseChunk and aquifer.
3. Get/create unprimed OCEAN_FLOOR_WG, then WORLD_SURFACE_WG. An existing map is
   retained; an absent map is created as source zero-packed storage, not primed
   by scanning old blocks.
4. Run the shared source material kernel in the original traversal order.
   Exact canonical AIR skips the write and preserves the existing block. For
   other states the actual debug-void predicate can suppress the write.
5. Directly set the physical section state, update OCEAN_FLOOR_WG then
   WORLD_SURFACE_WG, and append a post only when the same aquifer's flag and
   actual generated fluid state require it. Prior queues/order/duplicates stay.

Owned section guards release on success or failure. The actual density/aquifer
prefix remains retained even when the visitor fails; the session is poisoned.

`finish_noise_task` runs the original successful-task continuation after either
material outcome. Actual retrogen replaces old bedrock at Y=0..4, then applies
the missing-bedrock mask through ordinary ProtoChunk writes. It does not clear
or filter existing posts. `generate_noise_task` composes both calls. Neither
method advances persisted status or clears retrogen via a fabricated completion.

`build_surface(phase_blender, region_height, zoom, neighbors, temperature)`
requires that continuation to have succeeded. The actual debug-void predicate
returns `TargetSurfaceOutcome::DebugVoidSkipped` before cache/region work.
Otherwise a missing NoiseChunk can be created using the recollected phase
Blender; an existing NoiseChunk keeps its original Blender without an Arc
equality requirement. The same seeded SurfaceSystem reads/scans the full target
and live WG maps, while its setter suppresses writes outside the generation
accessor before logical mutation or post enqueue. Real center palettes,
mandatory foreign palette owners and the persistent thermal owner supply biome
context. Errors leave the live mutation prefix and poison the session.

`target()` exposes only a shared reference. There is no mutable target escape,
clone/rebind/reset operation or cache replacement. Retained owners remain alive
after successful SURFACE. The local phase and outcome enums certify only these
invocations, never original generation status or dependency readiness.

## Original source anchors and remaining scope

- NoiseBasedChunkGenerator fillFromNoise/doFill: 291..413; separate generation
  clamp, section acquisition, AIR skip and direct writes/maps/posts.
- NoiseChunk.forChunk: 42..209; full-build cache and aquifer ownership.
- ChunkAccess getOrCreateNoiseChunk: 401..404.
- ChunkAccess/LevelChunkSection biome fills: 432..441 / 188..199.
- BelowZeroRetrogen resolver: 114..127; successful noise-task repairs: 78..98.
- ChunkStatusTasks generateNoise: 63..77; repairs precede step completion.
- ProtoChunk generation accessor: 340..343; persisted status: 235..240.
- NoiseBasedChunkGenerator public SURFACE: 236..243; SurfaceSystem: 82..195;
  its BlockColumn setter checks the generation accessor before mutation.

Nonempty/unavailable structure beardifiers still fail when a cache factory is
actually needed. Only the pinned built-in generator/registry profile is
constructed by NoiseGenerator. Runtime leases, world/dimension identity,
dependency scheduling, actual status advancement/retrogen clearing, persistence,
Main integration, existing-target CARVERS and complete FEATURES remain separate
owner work. No source archives, private saves or Java binaries belong in this
delivery.
