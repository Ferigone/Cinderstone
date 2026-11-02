# Original region boundary, legacy provenance and upgrade

An unchanged original Minecraft Java **1.21**, protocol **767**, normal world
executed a real ServerChunkCache NOISE request. A separately constructed concrete
WorldGenRegion then executed public `Blender.of` and
`BlendingData.getOrUpdateBlendingData`, observing the EMPTY singleton and an
absent data slot. A separate run loaded a complete genuine 1.17.1 save copy into
unchanged original 1.21, retained a nonempty Blender on its requested NOISE chunk,
and observed nonempty public collection and a calculated old BlendingData owner.
These are original-server observations of the fresh and legacy boundaries.
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
source hash and actual replay. The Java `upgrade` branch's provenance-marker
check alone only checks file presence. The separately pinned upgrade runner
below verifies the actual historical run record, marker and every raw input
file before invoking that branch. The fresh executions above establish only
the empty boundary; the later upgrade execution establishes the selected
legacy observations.

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

The historical-save reference is unchanged original **1.17.1**, protocol **756**.
The port target remains **1.21/protocol 767**. The official historical server
SHA-256 is
`e8c211b41317a9f5a780c98a89592ecb72eb39a6e475d4ac9657e5bc9ffaf55f`.
The successful generation uses these frozen validation sources:

| Source | SHA-256 |
| --- | --- |
| [Legacy117Bootstrap.java](../../src/worldgen/validation/Legacy117Bootstrap.java) | `67b6ff6424985692ff079db97daf9dacfbe50b070ac07d74e22daf9583ab13a8` |
| [Legacy117World.java](../../src/worldgen/validation/Legacy117World.java) | `17ca3125c64364639ff7124fadf1a7c8849cbe75dc680eaa0cf310ddfc4cc1b9` |
| [build_legacy117_compile_api.py](../../src/worldgen/validation/build_legacy117_compile_api.py) | `3ba84df6a41f27677f376971592593cca1f0dcbbf76351769ba7cb48a362cd0e` |

The helper generates a compile-only MinecraftServer ABI header outside the
checkout. It preserves original public/protected JVM descriptors and raw
superclass/interfaces, omits the class-level generic Signature, and replaces
retained concrete bodies with unconditional throws. Abstract methods remain
abstract. The header supplies compiler declarations, not engine behavior.
The subclass preserves the original bridge casts/delegation without overriding
their typed target methods. At runtime, Bootstrap permits exactly the compiled
harness directory and original server JAR on the classpath, rejects a header in
that directory, checks original CodeSources, and verifies the original
MinecraftServer class bytes against SHA-256
`4846a20513148027eaf124a51427df69b34560924e6471c8913613648e741f71`.
The generated header is excluded from runtime and publication; the original
archive is unchanged.

This is an embedded original MinecraftServer using loaded vanilla resources,
actual registries/WorldData and the normal NoiseBasedChunkGenerator. It invokes
original `createLevels`, then requests `(0,0)` at FULL through the real
ServerChunkCache. For the new uninitialized world, `createLevels` selects
original `setInitialSpawn` before marking level data initialized. That initial
spawn path is established by source control flow, without a separate entry
counter. Outer `loadLevel` and `prepareLevels` spawn-region warm-up are omitted;
the evidence does not claim DedicatedServer or GameTestServer startup parity.

Two earlier post-close provenance failures remain recorded:

| Attempt | Actual outcome |
| --- | --- |
| `legacy-normal-1171` | Original generation and all-dimension save logs were observed, but the scanner rejected an original zero-byte region placeholder. Exit 1, reaped, no success marker. |
| `legacy-normal-1172` | Original generation/save occurred, but the guard compared raw saved `full` with registry-qualified `minecraft:full`. Exit 1, reaped, no success marker. |
| `legacy-normal-1173` | A new run completed original generation, save/close and post-close provenance. Exit 0, reaped, marker written after successful checks. |

The successful scanner records zero-byte placeholders in the raw inventory and
skips their nonexistent chunk slots; malformed short nonempty regions still
fail. It records both original status forms: registry `minecraft:full` and
serialized `full`, comparing the saved raw value with the original serialized
name. The failed worlds were preserved. Neither their metadata nor their
success records were retrofitted to pass; the successful observation uses a
separate new world and source revision.

The successful 1.17.1 run observed seed `123456789`, saved generator
`minecraft:noise`, and actual saved DataVersion **2730**. The requested target
returned a real LevelChunk (`cmx`) at FULL with generator `cpu`. Post-close raw
inspection found two saved FULL chunks, `(0,-1)` and `(0,0)`, each with raw
`Status="full"` and DataVersion 2730. The extra chunk is consistent with the
source initial-spawn path, not a separately measured FULL callback. All **11**
original raw files were inventoried before harness metadata and rechecked.
No version/status/blending tags were hand-authored to create this provenance.

The provenance artifact SHA-256 is
`3dc1c2f7fa2deb3f94ff809d4b7254b76e1f9420ca72b758d50344e1e074a801`;
the successful run-result SHA-256 is
`ef18808ee923dbf0dff377760e3a77993808714754109b5d332191949da2630a`.
Both fixtures retain these observations under `legacy_reference`, including the
separate prelaunch build audit, failed-attempt hashes and raw file/chunk hashes.
The build audit alone is compile evidence; generation acceptance comes from the
successful run and post-close artifact. Original stopServer returned before
observation, the server thread was joined, and logs recorded all three dimensions
saved. No close-error lines were found. Listener counts before/after FULL and
after stop were zero; EULA files were absent before and after.

The run used maximum heap 1536 MiB, TERM at 110 seconds with KILL after a further
five seconds, and two active processors. The host compiler/runtime was Java
21.0.12.1. Compilation used `-source 16 -target 16 -proc:none`; the host lacked
the `--release 16` symbols. This does not establish Java 16 runtime compatibility.
A reproduction must use new external build/world paths and exclude the header
from its runtime classpath:

```sh
WG117_ROOT=/workspace/blending-region-harness
WG117_JAR="$WG117_ROOT/artifacts/1.17.1/server-1.17.1.jar"
WG117_BUILD="$WG117_ROOT/new-external-build-UNIQUE"
python3 src/worldgen/validation/build_legacy117_compile_api.py \
  --server-jar "$WG117_JAR" --out-dir "$WG117_BUILD/compile-only"
mkdir -p "$WG117_BUILD/runtime-classes"
javac -source 16 -target 16 -proc:none \
  -cp "$WG117_BUILD/compile-only:$WG117_JAR" -d "$WG117_BUILD/runtime-classes" \
  src/worldgen/validation/Legacy117World.java src/worldgen/validation/Legacy117Bootstrap.java
cd "$WG117_ROOT/runs"
timeout --signal=TERM --kill-after=5s 110s \
  java -Xms128m -Xmx1536m -XX:ActiveProcessorCount=2 \
  -cp "$WG117_BUILD/runtime-classes:$WG117_JAR" Legacy117Bootstrap \
  "$WG117_JAR" "$WG117_ROOT/runs/controlled-worlds/legacy-normal-117-UNIQUE" 123456789 0 0
```

The fixed controlled-world root and refusal to reuse an existing world remain
operational limits. Generated headers, classes, original binaries, worlds and
logs remain external. This historical reference establishes actual 1.17.1
generation and saved provenance for the separate original 1.21 run below.

The completed upgrade used
[check_blending_region_upgrade.py](../../src/worldgen/validation/check_blending_region_upgrade.py),
SHA-256
`3ed8045bb56d1b098b15b97907dd792dc260db77ebd2bb74d86357fc72a7c1c8`,
with the unchanged counted Java `32ec…` and fresh-runner utility `43a0…` pins
above. It verifies the official historical artifact and original superclass,
the successful `1173` result and build audit, the pinned post-close marker,
and all **11** raw files plus that marker. Missing/extra files, changed hashes,
symlinks and malformed provenance reject the run. The marker supplies the
previous original NBT observations; this runner verifies its exact bytes rather
than rewriting or independently decoding its tags.

Only a newly named complete copy was opened by 1.21. Before launch, the complete
copy inventory equaled the closed original inventory, including the marker.
The closed original was rechecked before and after execution and remained
unchanged. No input file was rewritten to prepare the upgrade. The original
1.21 level-data read/datafix and existing-world WorldLoader supplier decoded
the saved options and dimensions; the fresh-world seed, generation flags and
spawn-radius setting were not substituted into this branch.

The actual bounded run finished in **22.242 s**, exit **0**, and was reaped.
It used maximum heap **2048 MiB**, four active processors and a **120-second**
wall limit. The measured executor count was **1**, passive GameTest callback
count **0**, original development flag **false**, and the observer ran on the
actual server thread after original public `isReady`. The world retained seed
`123456789`, normal NoiseBasedChunkGenerator `dzb`, RandomState `dzm`,
ServerLevel `aqu` and ServerChunkCache `aqs`.

The real cache/scheduler request targeted `(6,0)` at NOISE. Its returned chunk
reported persisted and highest-generated status **`minecraft:noise`**, with an
installed NoiseChunk `dzc` whose Blender was **not** the EMPTY singleton. This
is a post-request observation; no pre-request status was measured. The harness
did not complete holder futures or assign statuses. Original GameTestServer
spawn/weather initialization and any template preparation remain part of this
original profile, so the run does not isolate every generation task to the
explicit request.

After that request, the separate concrete `WorldGenRegion` `arb` observation
acquired **289** real generation holders and invoked public `Blender.of`.
Its result was also **not** the EMPTY singleton. Public
`BlendingData.getOrUpdateBlendingData(region,0,0)` returned an actual non-null
old owner with the original calculated flag **true**. The flag is observed
after the call; it does not prove that this explicit call performed the first
calculation. The retained NoiseChunk Blender and this separate public result
were independently nonempty; their object identities and maps were not compared.
This region again uses the original BIOMES descriptor without
executing its outer `ChunkStep.apply`. Source release calls run in `finally`;
holder reference counts and release counters were not independently measured.

These runtime observations establish the selected public nonempty and calculated-owner
branches, without measuring collection traversal, map keys, old vertical bounds,
retrogen state, or numeric blend values. They do not establish that both maps
are populated: the height/biome map may be nonempty while density owners are
absent at this target. No internal public-Blender invocation counter was added.
The earlier native calculated-snapshot comparisons remain separate from this
original concrete-region run.

The actual listener count was **0**, EULA files were absent in both the external
work area and copied world, and no listener-binding or Main/Eula route was
invoked. Original shutdown logs recorded Overworld, End and Nether saves and
then `All dimensions are saved`. The runner inventoried the resulting copied
world after shutdown; the original closed legacy source still matched its
complete pre-run inventory. The raw original runtime-report SHA-256 is
`64fd14efbe122bed2545bbdaa59f7f4f710bfb9f5b05543448d901859eae3868`;
the complete external upgrade-result SHA-256 is
`2a20be8642cac6b0fd7469d04aa3be13e1da42da4edab8c4759e9af43db43459`.

A separate external Python NBT reader inspected the actual closed input and
output without running Java or modifying either save. In both old chunks
`(0,0)` and `(0,-1)`, input DataVersion **2730**/raw `full` became saved
DataVersion **3953**/`minecraft:full`, with serialized `blending_data` section
bounds **0..16** and **16** height entries. The target `(6,0)` had no input Anvil
location entry and was saved at DataVersion **3953**/`minecraft:noise`, without
serialized blending data. None of these three closed output chunks contained
`below_zero_retrogen`; this final absence does not record an intermediate
retrogen transition. Seed `123456789`, generation flags and the three vanilla
dimension types/settings were retained through the migrated level encoding.
Original GameTestServer initialization changed saved spawn `(0,88,-16)` to
`(0,80,0)` and serialized spawn-chunk radius from absent to `2`.

The passive reader's raw facts hash is
`a72a23911a59651d8b6aad6c5160e7d74cd5b7c42152e4697a745f0a06384530`.
Both the [runtime fixture](../../fixtures/world-generation-blending-region-runtime.json)
and [validation fixture](../../fixtures/world-generation-blending-region-validation.json)
separate `upgrade_reference.actual_runner`, `upgrade_reference.identity_scope`
and `upgrade_reference.passive_saved_nbt`. The passive fields are not assertions
made by the Java report or published upgrade runner. Saved heights and section
bounds do not establish runtime density/biome caches, calculated flags, map
membership, iteration order, or numeric blending behavior.

To reproduce this exact provenance profile, retain the verified original `1173`
world read-only and select new external output paths:

```sh
python3 src/worldgen/validation/check_blending_region_upgrade.py \
  --oracle-dir /path/to/pinned-oracle \
  --legacy-world /workspace/blending-region-harness/runs/controlled-worlds/legacy-normal-1173 \
  --legacy-result /workspace/blending-region-harness/legacy-run-result-1173.json \
  --legacy-build-audit /workspace/blending-region-harness/legacy-build-audit.json \
  --legacy-server-jar /workspace/blending-region-harness/artifacts/1.17.1/server-1.17.1.jar \
  --work-dir /path/outside/checkout/new-empty-upgrade-work-directory \
  --run-original \
  --report /path/outside/checkout/new-upgrade-result.json
```

The runner is pinned to this successful legacy source record, not arbitrary
legacy worlds or formats. It creates a unique complete copy beneath the Java
source's fixed external controlled root; outputs inside the original input
world are rejected. The work directory must be empty and the report absent.
Without `--run-original`, compile and provenance checks do not establish an
upgrade execution. This evidence does not claim general legacy compatibility.

These observations do not compare native full images against these server runs,
prove native scheduler parity, or establish Main integration, complete FEATURES,
persistence compatibility or FULL publication. The earlier native comparisons
cover calculated controlled nonempty snapshots and retained BIOMES→NOISE→SURFACE;
the original runs establish separate concrete fresh/legacy server-region and
historical-save boundaries. No native image comparison or new checklist
completion follows from the original upgrade observation alone.
