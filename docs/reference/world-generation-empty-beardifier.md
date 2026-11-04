# Empty Beardifier owner and raw marker

Target: Minecraft Java **1.21**, protocol **767**, original inner JAR SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Source analysis uses the pinned official mappings and direct Java 21 `javap`;
REA tools were unavailable. This stage supports the existing observed-empty
structure context. It does not generate structures or implement nonempty
Beardifier contributions.

`DensityFunctions.BeardifierMarker` (`dys$b`, mapped lines 353–373) computes
zero, fills zero with `Arrays.fill`, and declares bounds `[0,0]`. Actual
`Beardifier` (`dyl`, 81–144), including an instance with empty piece and
junction iterators, computes zero but declares `[-Infinity,+Infinity]`. It
inherits `DensityFunction.SimpleFunction.fillArray`, which calls the supplied
provider's `fillAllDirectly` and therefore visits its indices.

`NoiseBasedChunkGenerator.createNoiseChunk` (106) constructs an actual
`Beardifier.forStructuresInChunk`. `NoiseChunk` retains this owner before
mapping its router, then maps `cache_all_in_cell(add(finalDensity,
BeardifierMarker))` through the same visitor. `NoiseChunk.wrapNew` (872–895)
replaces the marker with that retained owner. Its visited metadata and fill
behavior differ from the raw marker even when the actual structure lists are
empty. Parent arithmetic bounds must be reconstructed from that replacement.

The native `DensityGraph` keeps raw marker bounds and raw fill behavior.
`ChunkDensity` reconstructs the actual empty owner's infinite bounds and
delegates its fill to the current provider. This applies to column, cell and
external-point providers. An external SinglePoint context still bypasses a
NoiseChunk interpolator in the original way; it does not revert the visited
function's bounds or fill behavior to those of the raw marker.

The source-only [probe](../../src/worldgen/validation/EmptyBeardifierProbe.java)
and [bootstrap](../../src/worldgen/validation/EmptyBeardifierBootstrap.java)
were executed against the unchanged pinned JAR. They construct actual
NoiseChunks with identical original settings/RandomState and either the
deliberate marker control or an actual zero-list Beardifier. The original
CODEC and `mapAll`/NoiseChunk visitor process this expression:

```text
min(-1, add(beardifier, interpolated(0)))
```

The raw and marker-control bounds are `[-1,-1]`; actual-empty visited bounds
are `[-Infinity,-1]`. Before interpolation begins, the owner-context marker
control returns `-1`, while the actual empty owner evaluates the interpolator
and throws `Trying to sample interpolator outside the interpolation loop`.
Both external SinglePoint evaluations return `-1`. Direct three-value fill
produces three zero values in both cases, but the marker has no `forIndex`
calls and the actual owner visits `[0,1,2]`. The focused native regression
checks this lifecycle distinction, unchanged raw metadata and the observable
column-provider visits.

The original component execution exited zero and was reaped in 4.946 seconds,
with a 768 MiB maximum heap and 60-second wall bound. Its observation JSON
SHA256 is `373a8eee0fa87a65abfc03e2537d4dcfcc26d63ddd683b8c3ca2b7f8bdc1f766`.
Probe source SHA256 is
`25ccdfec495012204f41662c05ee72ddc629c6bcd12576fe60977bfcef53b7af`;
bootstrap SHA256 is
`c0ffa598b56ccd63f659e8244a732c70303a7cf3c07b9f5edd06667d0853bac6`.
No original class, world, status or version was changed. This component
witness establishes no structure selection, full-terrain replay, saved-world
compatibility, ChunkStatus completion or Main integration. Historical paired
reports remain evidence for their own pinned source and declared marker
inputs; this correction does not rewrite those observations.

To reproduce, provide the verified inner JAR and its official bundled-library
classpath, use Java 21, compile `WorldgenSurfaceOracle.java` and the bootstrap
into a parent output directory, and compile the probe into a separate child
directory. Run `oracle.EmptyBeardifierBootstrap <child-directory>` with the
parent output, original JAR and libraries on the parent classpath, under the
same heap and wall bounds. The parent/child split avoids mixing unsigned probe
classes into the original signed default package. The helper performs registry
bootstrap only; it does not start a server.
