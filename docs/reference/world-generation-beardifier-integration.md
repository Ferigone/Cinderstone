# Actual structure influence in retained generation

Target: Minecraft Java 1.21, protocol 767, original inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The source references use official mappings and direct `javap` inspection.
They are not REA execution or a native Main callsite.

The additive live-target path accepts an actual immutable `Arc<Beardifier>`.
The selected rigid pieces and junctions keep their source encounter order,
coordinates, terrain adjustment and ground-level delta. The same captured
owner supplies scalar and provider-driven density evaluation before, during
and after the material fill. It is captured with the actual Blender before
router wrapping, independently of the full-build/generation-accessor split.

## APIs and owner lifetime

`TargetGeneration::new_with_influences(generator, target, blender, beardifier)`
takes `Arc<NoiseGenerator>`, the caller's same `Box<T: GenerationTargetHost>`,
`Option<Arc<Blender>>` and `Option<Arc<Beardifier>>`. Construction stores these
inputs without target queries, owner computation, selection or cache creation.
`None` preserves an unavailable owner and never becomes an empty influence.

The existing `new` and `new_with_optional_blender` APIs remain available.
Their `GenerationInfluence::ObservedEmpty` structure input creates a fresh
actual empty Beardifier only when the NoiseChunk factory is entered.
`Present` without its actual owner reports that the owner is required;
`Unavailable` reports unavailable context. The older fresh-image generator
APIs retain their explicit empty-structure profile. They are not generalized
by this live-target ingress.

`fill_biomes` uses the retained sampler/Blender and existing retrogen helper.
`fill_from_noise` still checks the current generation plan before entering any
cache factory: a NoCells outcome neither queries the structure input nor
replaces a previously retained owner. `finish_noise_task` separately applies
the successful-task retrogen repairs, including after NoCells. The shared
material kernel reads the actual captured structure contribution through the
same wrapped density and aquifer owner; it does not reseed or rebuild them.

`captured_beardifier() -> Result<Option<Arc<Beardifier>>>` reads only the actual
graph-bound cached owner. It returns `None` when no NoiseChunk exists. The
returned Arc is immutable and supplies no replacement or rebinding operation.

`build_surface_with_influences(phase_blender, phase_beardifier, region_height,
zoom, neighbors, temperature)` adds an explicit optional phase structure owner
to the existing SURFACE arguments. It shares the existing surface body and
error/poisoning behavior. A cache hit keeps the original beardifier even if
the phase argument is different or unavailable. A cache miss requires the
actual supplied owner. Existing `build_surface` keeps its constructor input
for that factory case. No equality of later influence Arcs is required.

The native session retains its same target sections, WG maps and ordered
postprocessing queues. Partial-cell physical writes, exact AIR skips,
retrogen generation windows and generation-clipped SURFACE writes keep their
existing contracts. Error prefixes remain owned by the poisoned session.

## Source selection and phase boundaries

`NoiseBasedChunkGenerator.createNoiseChunk` (mapped line 106) supplies the
actual `Beardifier.forStructuresInChunk`. `NoiseChunk` captures it before
mapping the router; `wrapNew` (872–895) replaces BeardifierMarker with that
owner. `Beardifier.compute` (81–144) evaluates rigid pieces and then junctions
in their retained order. The raw marker's zero bounds/fill remain different
from the actual owner's infinite bounds and provider visits, including for
an actual empty owner; see [the owner/marker note](world-generation-empty-beardifier.md).

Public phase adapters must preserve two different call phases. Blender
recollection happens before `getOrCreateNoiseChunk` even on a hit. Structure
selection happens only inside the cache-miss factory, before router wrapping.
An adapter must not query or reconstruct structure owners for a cached chunk.
Passing `None` to the new SURFACE structure argument supports that hit path.

`Beardifier::from_ordered_parts` is explicit data ingress. It does not certify
a StructureManager lookup, structure-start validity, region dependencies or
encounter order. A real selector must use the target STRUCTURE_REFERENCES map
and referenced STARTS chunks, collect valid starts in source order before
visiting pieces, and preserve the pool-piece projection/proximity/junction
filters. The source selection API and its actual host observations remain a
separate contract from constructing these immutable parts.

## Validation inputs and limits

The existing native replay probe accepts optional query data:
`beardifier.rigids` contains ordered `{box:[minX,minY,minZ,maxX,maxY,maxZ],
terrain_adjustment,ground_level_delta}` rows; `beardifier.junctions` contains
ordered `{source_x,source_ground_y,source_z}` rows. Adjustment names are
`none`, `bury`, `beard_thin`, `beard_box` and `encapsulate`. Missing data in
the explicit cache-absent NoCells profile remains unavailable; ordinary
baseline queries use an actual empty owner.

Optional `beardifier_points` supply block coordinates for scalar raw-double
bit observations. The separate `mode: "beardifier_kernel"` branch accepts
only `{query: ...}` and constructs no target or generation owner. Full-phase
queries still accept only the declared query and actual original INITIAL
storage; future original snapshots never become native input. Native SURFACE
supplies no later structure selection on its already-cached path. Immutable
Arc identity observations are separate from numeric equality and whole-image
comparison. These controlled parts are not naturally generated structures.

Focused native checks exercise actual nonempty owner capture, unchanged
cache allocations through a skipped fill, cached acceptance of different or
unavailable later inputs, lazy unavailable-owner NoCells, and failed positive
owner construction with released section leases. Original/native point and
full-phase validation passed on native source `1457a89`. The bounded original
process completed in 6.739 seconds and native in 2.070 seconds; both exited
zero and were reaped. Selected raw scalar bits and bounds agree for all five
adjustments, isolated junctions and ordered mixed contributions.

The empty control, rigid-only, junction-only and combined profiles agree at
every selected BIOMES/NOISE/task-continuation/SURFACE snapshot, including
complete physical blocks and biome palettes, both WG first-available heights,
section counters, queues/allocation and actual status/accessor metadata.
Nonempty profiles have actual material and surface image effects against the
empty control. Original retained Beardifier identity and the native captured
Arc identity match through each phase; the cached SURFACE path supplies no
new structure selection. These observations do not expose native private
aquifer allocation identity or original runtime lock order. Exact source,
probe, original artifact and execution records are published in the
[runtime fixture](../../fixtures/world-generation-beardifier-runtime.json)
and [validation fixture](../../fixtures/world-generation-beardifier-validation.json).

The full-phase original BIOMES body runs synchronously on one controlled
worker; its public asynchronous wrapper and actual scheduler are outside the
comparison. Original targets remain persisted EMPTY, with the declared
base-ChunkAccess palette view and explicit stored neighbors for SURFACE.
Neither those palettes nor direct ordered structure parts establish real
StructureManager selection, natural structure generation or dependency
readiness. The accepted pair validates this declared owner ingress and its
retained numeric pipeline.

This path does not advance persisted ChunkStatus or clear retrogen through a
fabricated completion. The central status-event/shared-live-owner adapter is
still required. A successful live-target SURFACE session remains
`PendingCarvers`; its existing-target CARVERS handoff is not implemented by
the image-backed `BiomeTerrain::carve_air` API. Structures/start generation,
their persistence and real region scheduling, complete FEATURES, lighting,
final conversion, Main integration and FULL publication remain separate work.
