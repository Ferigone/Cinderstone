# Retained NoiseChunk at zero vertical overlap

The target is unchanged Minecraft Java **1.21**, protocol **767**, inner server
SHA-256 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
The outer JAR, official mappings and every declared bundled library are checked
against fixtures/source-lock.json and the original outer archive. Source
inspection uses direct Java 21 javap, not REA Evidence.

NoiseBasedChunkGenerator.doCreateBiomes (dzb, 98–103) obtains the target's
cached NoiseChunk before filling its generation-accessor palettes.
NoiseChunk.forChunk (dzc, 108–111) clamps settings against the target build
accessor. A touching vertical boundary produces height zero while the target
still has positive build height and existing section palettes.
NoiseBasedChunkGenerator.fillFromNoise (dzb, 291–319) independently clamps
against the generation accessor and returns the same input when its cell count
is nonpositive. This does not discard a cache created by BIOMES or skip the
following SURFACE invocation.

The focused cases are Nether build **[-64,0)** against settings starting at
zero, and Overworld build **[320,384)** against settings ending at 320. Each
target has four real sections. Declared input is the canonical setting default
material, with gold-block markers on the second-highest layer where
(localX*3+localZ)%11 equals zero. Full registered ocean/plains palettes, eight
explicit foreign palette owners and queues containing null, allocated-empty
and duplicate entries are controlled input. These are not historical saves
or naturally generated neighbor palettes.

An earlier **original-only RAW BeardifierMarker** observation used the
existing-target oracle and compiled helpers from its prior paired run. That
bounded Java process exited zero and was reaped after **5.747 seconds**, with
768 MiB heap. It observed 256 BIOMES resolver calls per target, retained grids
(minY=0,height=0) and (minY=320,height=0), unchanged target data through the
skipped NOISE fill and its actual task continuation. Original SURFACE changed
**16,361** and **730** voxels respectively. Its private ledger SHA-256 is
4d10845ac9ffffa39583c14594660dcaa3b0043c86389636f87e50230530494a.
That record pins the original source, compiled classes, input files and JAR.
It is historical context, not acceptance evidence for the new actual-empty-owner
paired profile and not a native comparison.

[check_zero_overlap.py](../../src/worldgen/validation/check_zero_overlap.py)
reuses the existing-target oracle, native probe and shared checker helpers.
The two zero-overlap queries and one aligned Overworld positive-NOISE control
require the original **actual empty Beardifier** owner, checking both its
reported input kind and actual class dyl. The current oracle constructs this
real empty owner. Historical raw-marker fixtures retain their older source hash;
this new original context does not rewrite that history. All executed source
hashes are recorded by the new runner.

Java executes first. Native receives only each query and its actual original
INITIAL snapshot, never expected BIOMES, NOISE or SURFACE output. Comparisons
cover complete physical block and biome artifacts, WG first-available values,
section counters, status/retrogen/generation fields, unsaved/debug flags and
every ordered section queue with its allocation state. The checker requires
unchanged complete target data through NOISE and 256 actual BIOMES queries in
each zero-overlap case, positive SURFACE mutations and retained native cache
presence. The single aligned control is reused from the existing checker's
declared inputs and must execute positive NOISE material writes as well as
positive SURFACE. It covers the material loop skipped by the two boundary cases.

Original NoiseChunk, aquifer, Blender, marker-cache and sampler identities are
observed separately from native target/section/state/map/post-array ownership
and released leases. Native private grid and aquifer identities are not runtime
observations. No original acquisition order, unused packed WG bits or global
inter-section queue order is inferred. Original targets remain persisted EMPTY,
with the existing declared base-ChunkAccess palette view for controlled SURFACE;
no ProtoChunk status is fabricated. SURFACE scans logical chunk build bounds
and live WG heights; physical section images are compared separately.

The runner never invokes Cargo. The coordinator first builds the exact current
native library, then selects a new external work directory:

    python3 src/worldgen/validation/check_zero_overlap.py \
      --oracle-dir /path/to/pinned-oracle \
      --work-dir /path/outside/checkout/new-empty-zero-overlap-work \
      --target-dir /path/to/explicit-native-build-target \
      --skip-build \
      --runtime-report /path/outside/checkout/new-zero-overlap-runtime.json \
      --report /path/outside/checkout/new-zero-overlap-validation.json

Java 21 and the matching Rust compiler must be available; --java-home and
--rustc select installed tools. Each compilation/execution has a 55-second
timeout plus the shared helper's five-second termination grace. Children are
killed and reaped on failure; raw transcripts remain external. Outputs are
written only after every selected comparison and positive guard passes, with
artifact paths sanitized relative to the external work area.

The initial handoff preceded paired execution; the accepted run is recorded
below. It does not establish negative-overlap support, arbitrary dimensions,
nonempty structures, scheduler behavior, saved-world import, Main, persistence
or FULL delivery.

## Current catalog dependency

Central PR114 corrected30 flowing-water flags in the placement catalog, changing
its SHA256 frome044d4bd to97d79958. The earlier native state-facts capture still
pinned the old catalog, so the first actual-empty-owner native replay correctly
failed its strict registry guard. The original phase run alone had succeeded;
that partial attempt is not a successful pair. An earlier helper compile also
failed because the original obfuscated class `it` shadows a qualified package
name; an explicit ObjectArrayList import fixes the helper without target edits.
Both failed attempts and process records remain external.

The actual unchanged original registry exporter was rerun against the current
catalog:4.999s, exit0, reaped. All26684 state predicate rows and defaults match
the previous capture exactly; the verified catalog SHA is the only changed
field. The published fixture is byte-identical to that actual export, SHA256
`3ba567d241f2387f85c5444adf312218090f3d64f93306ba7b192e54e0e282d3`.
The strict native guard is unchanged. Source-only WorldgenTargetFactsBootstrap
and the components fixture record reproducible invocation and exact artifact,
source, execution and capture hashes. The full library/server compiler check
and explicit library rebuild passed again after this data dependency changed.
Historical paired reports retain their old source/catalog/facts pins; they are
not relabeled as observations of the current catalog.

## Accepted actual-empty-owner pair

Native source `dbbc19d` passed all three controlled cases. Original Java took
4.989s and native took0.917s; both exited0 and were reaped. Every source owner
was the actual empty Beardifier (`dyl`), rather than its marker. Complete
physical states, stored biome arrays, WG first-available values, counters,
ordered post lists/allocation and actual status/accessor metadata match at all
five snapshots. Exact source/artifact/probe hashes and process records are in
the zero-overlap runtime and validation fixtures.

| Controlled profile | Observed NOISE | Actual SURFACE changes |
| --- | --- | --- |
| Nether build[-64,0), retained grid[0,0] |256 BIOMES samples, same retained cache, no target mutation |16361 cells |
| Overworld build[320,384), retained grid[320,0] |256 BIOMES samples, same retained cache, no target mutation |730 cells |
| Aligned existing Overworld control |28627 changes,92 AIR-skipped gold markers preserved,1250 added posts |15749 cells |

The two zero cases retain all23 tail gold markers. Foreign stored biome reads
are positive in all three SURFACE cases. No case adds a SURFACE-only post;
that positive insertion branch is not runtime proof here. Original private
NC/AQ/Blender/cache identities are observed. Native public cache presence and
actual Box/section/state/map/post-array retention are observed separately;
native private NC/AQ/grid pointer identity or acquisition-order traces are not.
All original targets stay actual persisted EMPTY. These are controlled phase
invocations with declared palettes, not WorldGenRegion/scheduler/save/Main/FULL
delivery or nonempty structure support.
