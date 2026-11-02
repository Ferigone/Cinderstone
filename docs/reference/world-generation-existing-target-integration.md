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
Bounded original/native paired validation passed for native source commit
`a28bfc2ef8b11581b9a09f76a9d54f25e8afc890`. The
[validation note](world-generation-existing-target-validation.md),
[runtime fixture](../../fixtures/world-generation-existing-target-runtime.json)
and [validation fixture](../../fixtures/world-generation-existing-target-validation.json)
record the exact source, probe and artifact hashes and the limits of that result.

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

`new_with_optional_blender(generator, target, blender, structure_beardifier)`
also accepts `None` for an actually unavailable Blender, without substituting
EMPTY. A no-cell material fill returns before accessing it or the structure
beardifier. A positive cache factory requires `Some(actual_owner)` and an
observed-empty structure beardifier; a real observed EMPTY Blender is supplied
as `Some(Blender::empty())`. Existing cached owners remain authoritative and
are not recreated from the constructor argument. The no-cell paired profile
uses unavailable generation dependencies and still executes the separate
successful-task retrogen continuation.

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

## Bounded validation

The paired profiles cover aligned existing Overworld data, positive and
negative partial-cell accessors, active below-zero retrogen, and a no-cell
retrogen fill. Complete physical block images and biome palettes, live WG
first-available heights, section counters, queue allocation/order/duplicates,
status and retrogen metadata match at every selected phase. Actual writes
below the logical minimum, preserved partial-cell tails and AIR-skipped old
blocks, bedrock/mask repairs, and clipped SURFACE mutations supply positive
behavior witnesses. The selected SURFACE runs add no new posts; they do not
prove its positive post-insertion branch.

The original runs first on immutable controlled input. Native receives only
that query and the original INITIAL snapshot, never future source phase data.
Original BIOMES invokes the actual private `doCreateBiomes` body on one
controlled worker with its persistent builtin selector; it does not exercise
the public asynchronous wrapper. NOISE invokes the public original fill and
its actual successful-task continuation. SURFACE invokes the original
generator overload with the retained owners and declared stored-biome
neighbors. Actual ProtoChunk status remains EMPTY. The probes expose the
declared base-ChunkAccess palette view for SURFACE without fabricating BIOMES
status, a WorldGenRegion or ChunkStep completion.

Original NoiseChunk/aquifer/Blender/cache continuity and the BIOMES sampler
delivered to callbacks are observed. Native target-box, section/state/map and
post-array identity and released leases are separately checked. Native private
aquifer/cache allocation identity and original runtime lock order are not
observed. WG first-available values are compared; unused packed-word bits are
outside this evidence. The selected profiles do not cover absent WG-map
creation, cave-key/unbound-cave resolver branches or a high-only missing-bedrock
mask.

An external compile assertion also passed for
`TargetGeneration<NativeProtoTarget<()>>: Send + 'static` (imported in that
assertion as `NativeTargetGeneration`). This establishes that the owned native
session can outlive a local binding and move to a worker; selector and thermal
owners still follow their separate worker contracts. It supplies no Main or
scheduler integration proof.

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
