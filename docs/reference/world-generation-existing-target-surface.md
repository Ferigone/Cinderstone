# SURFACE on an existing generation target

Target: unchanged Minecraft Java **1.21**, protocol **767**, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Outer artifact and official mappings are pinned in `fixtures/source-lock.json`.
This ledger uses official mappings and direct Java 21 `javap` inspection, not
REA Evidence. No target execution is claimed by this adapter's source alone.

`generator::target_surface::build_surface` is an internal implementation used
by the generation-session owner, not a separate public terrain generator. It
borrows the session's actual mutable `GenerationTargetHost`, retained
`ChunkDensity`, seeded `NoiseGenerator`, actual region height accessor,
`BiomeZoom`, foreign quart provider and persistent worker-local
`BiomeTemperature`. It reuses the existing `SurfaceSystem`; rule traversal,
badlands, iceberg and positional RNG algorithms are not duplicated.

| Original owner and mapped source | Adapter contract |
| --- | --- |
| `ChunkStatusTasks.generateSurface` (`dwb.g`, 82–85) | Call the actual surface stage; BelowZeroRetrogen does not skip the entire phase. |
| `NoiseBasedChunkGenerator.buildSurface` (`dzb.a`, 236–250) | Retain the chunk's existing NoiseChunk and use the settings' actual legacy-biome-height flag. Public debug-void handling and phase dependencies belong to the caller. |
| `WorldGenerationContext` (`dzv`, 10–13) | Anchor minimum is `max(region.minY, generator.minY)`; depth is independently `min(region.height, generator.genDepth)`, not an intersection height. |
| `SurfaceSystem.buildSurface` (`dzq.a`, 82–195) | Read and scan the full chunk build bounds, live WORLD_SURFACE_WG heights and retained preliminary-surface owner. |
| `SurfaceSystem$1.setBlock` (`dzq$1.a`, 91–103) | Obtain the current generation accessor before each attempted write; outside it, perform neither Proto write nor fluid postprocessing. |
| `ChunkAccess.getNoiseBiome` (`duy`, 418–427), `ProtoChunk.getNoiseBiome` (`dvs`, 244–247) | Center quart queries use the actual stored target palettes, source quart-Y clamp derived from build bounds and real highest-generated-status guard. Foreign queries retain their mandatory provider and coordinates. |
| `BiomeManager.getBiome` (`ddy`, 35–79) | Reuse the real zoom seed to select one quart query; temperature remains a later lazy rule callback. |

Chunk build bounds, the target's current generation accessor and region
anchor bounds have distinct owners. While BelowZeroRetrogen is present, its
generation accessor is `[-64,0)`. The adapter still scans the full chunk build
bounds and executes original queries/rules above zero; its setter suppresses
those writes. The guard precedes logical `proto_set_block`, WG-heightmap changes
inside that operation, and any fluid post. It is not a bedrock-correction guard:
the separate post-NOISE bedrock replacement intentionally uses logical Proto
writes at Y `0..4`. Physical section padding is retained and compared separately;
outside the logical build bounds, block reads return VOID_AIR.

An allowed write calls the target's actual logical Proto mutation with the
source `false` behavior, then marks the position only when the requested new
state contains fluid. It neither rebuilds nor filters existing postprocessing
queues. WORLD_SURFACE_WG comes from the target's lazy source-style heightmap
query; an unavailable map owner is not replaced with a constant or recomputed
stand-in. All state tests use the generator's existing canonical built-in
metadata. The actual imported target must use that same pinned registry.

The adapter routes the zoom-selected center coordinate directly to the live
target's stored palette. Foreign coordinates go unchanged to the supplied real
neighbor provider. Missing BIOMES/dependency owners propagate errors. The
thermal owner validates the selected built-in name without eager temperature
sampling, then retains its original worker-local FIFO cache through later
thermal callbacks. The caller must certify pinned built-in registry identity;
same-key datapack overrides cannot be detected from biome names.

There is no private image clone, rollback, replacement NoiseChunk, replacement
aquifer or reconstructed preliminary cache. A host or biome error preserves the
actual mutation/query prefix; the owning session must become poisoned and must
not publish a successful SURFACE result or silently reset owner histories.
Stage/status advancement belongs to that session. The existing SurfaceReport
counts attempted rule replacements, including writes that are clipped; it is
not a count of changed voxels or a completed-chunk certificate.

The public source phase separately recollects its actual Blender and supplies
dependencies. That contract must remain explicit in the session. The cached
NoiseChunk keeps its existing Blender owner; this helper neither requires Arc
equality against a newly collected result nor swaps the cached owner. No
structure, FEATURES, Main, persistence, scheduler or FULL support follows from
this internal adapter. Compiler and original/native runtime evidence will be
reported by the coordinating validation stage after it executes.
