# Original fresh-world region boundary

An unchanged original Minecraft Java **1.21**, protocol **767**, normal world
executed a real ServerChunkCache NOISE request. A separately constructed concrete
WorldGenRegion then executed public `Blender.of` and
`BlendingData.getOrUpdateBlendingData`, observing the EMPTY singleton and an
absent data slot. This is original-server evidence for the fresh, empty boundary.
The paired native nonempty snapshot observations remain separate evidence in
[world-generation-blending-validation.md](world-generation-blending-validation.md).

The inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
outer artifact and official mappings are pinned in `fixtures/source-lock.json`.
Inspection used official mappings, direct Java 21 `javap` and original execution.
No REA provider or REA Evidence was used. The archived successful second attempt
used harness source SHA-256
`f0fe4515ce68481084be57c235e92981694c958cd0d03d946a6533dd7405a4a5`.
The repository harness is
[RealGameTestServer121.java](../../src/worldgen/validation/RealGameTestServer121.java);
the successful counted replay used its separate source SHA-256
`32ec3d049de352805a613e68a79e72e4293029dfa477481c727826059a00735c`.
The repository runner
[check_blending_region.py](../../src/worldgen/validation/check_blending_region.py)
has SHA-256
`43a0a12f2d5bc21d01b24da0f6b35f02fe183ad2e2d5863d10e0240490d4ad2f`.
Its original runtime-report bytes hash to
`172bd8c38fa1724205e7519ae0a145877f4c025d1446d8e0251ea0f85a38799e`.
Sanitized observations and provenance are recorded in the
[runtime fixture](../../fixtures/world-generation-blending-region-runtime.json)
and [validation fixture](../../fixtures/world-generation-blending-region-validation.json).

The original WorldLoader loaded vanilla resources and tags. Original
`WorldPresets.NORMAL`, PrimaryLevelData and WorldStem supplied the world with
seed `123456789`; structures and bonus chest were disabled. Spawn-chunk radius
zero was selected through the original game-rule API before creating the new
world. The harness invokes the original GameTestServer constructor with that
normal WorldStem, avoiding its public factory's fixed flat preset. It does not
replace a flat world's metadata. `MinecraftServer.spin` creates the actual
server thread, levels, chunk cache and scheduler.

The evidence preserves two initial attempts and two separately pinned counted
repository replays:

| Attempt | Actual result | Evidence boundary |
| --- | --- | --- |
| Original GameTestTicker route | Timed out after **115.065 s**, exit **143**, terminated and reaped | A legitimate normal server booted, but the intended callback never ran. No requested-NOISE or public-region success is attributed to this attempt. |
| Historical server executor route (`f0fe…`) | Finished after **15.296 s**, exit **0**, reaped | After original public `isReady`, the report identified the executor entry on the actual server thread. Independent callback counts were not recorded by this revision. |
| First counted repository replay (driver `5271…`) | Finished after **14.386 s**, exit **0**, reaped | Actual observer executor count **1**, passive GameTest callback count **0**, original development flag **false**, and actual running-server-thread identity all passed under the earlier driver source pin. |
| Final published runner replay (driver `43a0…`) | Finished after **14.513 s**, exit **0**, reaped | The same counted observations passed with the exact published Java and final runner hashes above. |

The first counted replay used runner SHA-256
`527119daca0ab88683b457e50e261604b2e8dc8bd3fa67b878d3b4ea1da9894d`.
Removing one extra EOF blank line changed that driver hash. The final published
runner received its own fresh execution; the earlier successful run remains
separate history rather than acceptance evidence for the final driver pin.

The first failure exposed an original production boundary:
`MinecraftServer.tickChildren` only ticks GameTestTicker when
`SharedConstants.IS_RUNNING_IN_IDE` is true. The pinned production artifact leaves
it false. No development flag was changed to obtain a callback. The historical
`f0fe…` source registered its observer through both the GameTest callback and
server executor; its report identified the executor entry, with inactive ticker
behavior explained by the pinned source. The counted revision makes the
GameTest consumer a passive counter and permits the observer only from a
one-shot executor task. It checks the entry counts before observing the world
and again before writing its report. The measured counts belong to this new
replay, not the historical source. GameTest counts and process exit alone are
not completion witnesses.

On that server thread, `ServerChunkCache.getChunk(0,0,NOISE,true)` entered the
normal original cache/scheduler path. The returned chunk reported both persisted
and highest-generated status `minecraft:noise`, and its installed original
NoiseChunk retained the actual EMPTY Blender singleton. Observed classes were
ServerLevel `aqu`, ServerChunkCache `aqs`, RandomState `dzm`,
NoiseBasedChunkGenerator `dzb` and NoiseChunk `dzc`. The harness neither completed
holder futures nor assigned chunk statuses. The scheduler path is backed by
the inspected unchanged source; no internal public-Blender call counter was
instrumented or reported.

The explicit public-region observation is a separate operation. It acquires
**289 actual GenerationChunkHolder references** from that world's ChunkMap,
stores them in original StaticCache2D, and constructs an actual `WorldGenRegion`
(`arb`) with the original BIOMES ChunkStep descriptor and actual center chunk.
The harness calls the original release method for each acquired holder in
`finally`; reference counts and release counters are not independently measured.
Holder acquisition does not certify every neighbor's BIOMES completion. This construction does not execute
`ChunkStep.apply`, nor does it establish an outer BIOMES or SURFACE invocation
through that separately constructed region.

Within that concrete region, the unchanged public `Blender.of(region)` executes
and returns the original EMPTY singleton. Public
`BlendingData.getOrUpdateBlendingData(region,0,0)` also executes and returns null
for the center's actual absent BlendingData slot. These explicit calls establish
the public empty/absent results without an interface proxy or invented data.
They do not reach the nonempty collection loop, its eight-neighbor calculation,
or a positive old-storage scan. Those branches require genuine legacy owners
and remain outside this fresh run.

The repository replay used maximum heap **1536 MiB** and an enforced
**120-second** wall limit. Its original listener list was observed empty; the
harness invokes no listener-binding method. No EULA file was created or accepted,
and the Main/Eula route was not called. No Unsafe, target-field writes, manual
future completion, debug/version/status patches or handwritten blending metadata
were used. Original world generation and saving did write the new isolated
engine world. Original shutdown logs record saving Overworld, End and Nether,
then all dimensions saved and process shutdown. Generated saves and logs remain
outside the repository. Original GameTest setup can prepare unrelated far
template chunks; they are not the requested `(0,0)` NOISE witness.

The harness retains a fixed external controlled root and an
`--execute-approved` invocation gate. Fresh mode refuses an existing `level.dat`;
it is not a general portable launcher. Source/configuration hashes, bounded
execution, required runtime-report fields and termination/reaping records
identify the actual run. Portability or execution changes require their own
source hash and actual replay. The `upgrade` branch is unexecuted. Its
provenance-marker check only checks file presence. It does not authenticate a genuinely old save, establish supported
legacy formats, or supply a legacy result. This evidence records no successful
legacy generation/load or nonempty public-region execution.

The repository runner compiles a captured source copy and performs a passive
metadata audit by default. Its executed profile additionally verifies the pinned
outer/inner server, official mappings, every declared bundled library, loaded
server CodeSource and the required runtime fields. It records Java versions,
source/configuration/class/log hashes, timeout status and process reaping.
An explicit fresh replay uses Java 21, POSIX process groups and new external paths:

```sh
python3 src/worldgen/validation/check_blending_region.py \
  --oracle-dir /path/to/pinned-oracle \
  --work-dir /path/outside/checkout/new-empty-work-directory \
  --expected-source-sha256 32ec3d049de352805a613e68a79e72e4293029dfa477481c727826059a00735c \
  --run-original \
  --report /path/outside/checkout/new-result.json
```

The work directory must be empty and the external report must not exist. The
runner generates a unique new world name under the source's controlled root;
it does not retry implicitly, reuse a save or overwrite a prior observation.
All generated worlds, classes and logs stay external. Compilation/passive-audit
success without `--run-original` does not establish server execution.

This stage does not compare native full images against this original server run,
prove native scheduler parity, or establish Main integration, complete FEATURES,
persistence compatibility or FULL publication. The earlier native comparisons
cover calculated controlled nonempty snapshots and retained BIOMES→NOISE→SURFACE;
this fresh original run establishes a different concrete server/region boundary.
