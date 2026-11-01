# Focused generation stage probes

These helpers call the unchanged official Java 1.21 classes and the public
native `worldgen` API. They are optional reference tools, not production Java
dependencies. All original artifacts remain outside the repository.

With Rust, Python and a Java 21 JDK on `PATH`, run from the repository root:

```sh
python tools/bootstrap_oracle.py --oracle-dir /tmp/cinderstone-oracle
python src/worldgen/validation/check.py --oracle-dir /tmp/cinderstone-oracle
```

`--java-home` selects a JDK. `--work-dir` retains generated probes, terrain arrays
and observed thermal tables in an external directory; otherwise a temporary
directory is removed on completion. `--report` saves the JSON comparison report.
The helper verifies the pinned server and mapping hashes, builds the native
library with the existing locked manifest, and compiles the probes externally.
It changes no manifest or runtime world.

The NOISE reference constructs a real original `NoiseChunk`, its actual seeded
`RandomState`, empty Blender/beardifier marker and original global fluid picker.
The harness invokes interpolation and block-state rules in the source `doFill`
order. It does not invoke the original `doFill` method or a server scheduler.

The SURFACE reference constructs a real original ProtoChunk, fixed-biome source,
BiomeManager, heightmaps and WorldGenerationContext. It invokes the original
SurfaceSystem with the original rule and biome objects. Native callbacks use
recorded temperature predicates from those actual biome owners; absent positions
fail. A fixed-biome fixture does not establish natural biome selection.

The report compares entire canonical block arrays, surface WG heightmaps and
surface-only postprocessing deltas. It also exercises native rejection of
unavailable surface context and a repeated SURFACE stage. These selected cases
do not prove Main integration, FULL chunks, every branch or full vanilla parity.

The selected cases also include the original default-namespace `:overworld`
alias and seed `0` with a fixed eroded-badlands biome, where original pillar
generation adds blocks and raises columns. The report records those original
growth observations alongside the complete state/heightmap comparison.

`WorldgenNoCellsOracle` invokes the original `fillFromNoise` entry point with
real sentinel-bearing ProtoChunks and unavailable generation dependencies.
For zero or negative clamped cell counts, the original future returns the same
unchanged chunk without constructing NoiseChunk state. `no_cells_probe.rs`
checks `NoiseFillOutcome::NoCells`, the exact clamped metadata and dependency
boundaries. These two helpers emit one JSON object without files or arguments.
An unchanged-input outcome is not a generated terrain image or FULL status.
