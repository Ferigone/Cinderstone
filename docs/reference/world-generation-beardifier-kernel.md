# Beardifier scalar kernel

This port follows the official Minecraft Java 1.21 server, protocol 767, inner
JAR SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The evidence is direct Java 21 `javap` inspection and an actual original-class
numeric export. REA tools were unavailable. No original class or saved world
was modified.

`Beardifier` maps to `dyl`. Its scalar method, mapped lines 88–132, visits
rigid pieces in encounter order and then junctions in encounter order. The
native `kernel::compute` borrows the immutable ordered owner and preserves
duplicates and addition order. All source integer subtraction/addition uses
Java wrapping semantics before conversion to double. The kernel consumes no
RNG and constructs no seed or random owner.

For a rigid piece, X and Z distances are the maximum of zero and the two
signed distances outside the inclusive box. The ground plane is
`box.minY + groundLevelDelta`; its signed Y distance is retained separately
from the distance used to index the kernel.

| Terrain adjustment | Contribution |
| --- | --- |
| NONE | Zero, retaining the original addition |
| BURY | Bury contribution at `(dx, groundDy / 2, dz)` |
| BEARD_THIN | Beard contribution at `(dx, groundDy, dz)`, with signed `groundDy`, multiplied by 0.8 |
| BEARD_BOX | Beard contribution with Y=`max(0, groundPlane-y, y-box.maxY)`, still using signed `groundDy`, multiplied by 0.8 |
| ENCAPSULATE | Bury contribution at half the X/Z distances and half `max(0, box.minY-y, y-box.maxY)`, multiplied by 0.8 |

The bury contribution, mapped lines 149–150, computes the original ordered
sum of squares and `Math.sqrt`, then
`Mth.clampedMap(distance, 0, 6, 1, 0)`. A point on the ground plane inside the
X/Z box therefore contributes exactly 1 for BURY.

A junction uses the three signed differences from its source X, source
ground Y and source Z. It contributes the beard value with that same signed
Y as its fourth argument, multiplied by 0.4. Junction contributions are not
interleaved with rigid pieces.

The beard contribution, mapped lines 160–170, rejects any lookup coordinate
outside `[-12,11]`. The table index is `(z+12)*24*24 + (x+12)*24 + (y+12)`.
Separately, `halfY = groundDy + 0.5` controls the sign and inverse-square-root
scale, including when BEARD_BOX selected a different table Y coordinate:

```text
square = (x*x + halfY*halfY) + z*z
scale = (-halfY * fastInvSqrt(square / 2)) / 2
result = scale * double(kernel[index])
```

The original `Mth.fastInvSqrt(double)` (`ayo.g`, mapped lines 491–496) uses
raw double bits, a signed shift and magic integer `6910469410427058090`, then
one Newton step. Its multiplication order is preserved. This is not replaced
with a division by `sqrt`.

The original static initializer, mapped lines 23–30 and 179–186, stores a
24×24×24 float table. Each entry is the final float cast of
`Math.pow(2.718281828459045, -((x*x + (y+0.5)*(y+0.5)) + z*z)/16)`.
The native table uses the same operation order, E constant and final f32
rounding, with lazy immutable initialization. It does not substitute `exp`,
promote the stored table entries to an independently computed double table,
or change the half-block offset.

The actual original `dyl.g` array was exported by reflection from the
unchanged pinned JAR after registry bootstrap, using the Java 21 source
launcher, a 512 MiB heap, two active processors and a 55-second bound. The
process exited 0, was reaped and had empty stderr. Elapsed time was not
recorded. The export contains 13,824 raw float-bit values, available as
[numeric generator data](../../fixtures/world-generation-beardifier-kernel.json),
with the original inner-JAR identity. The original numeric export SHA256 is
`44b10fc96f9eab7faa1bfbcdcd1db4674f052ad6bdbd82a41f64c9e75fe9ca58`;
the SHA256 of those bits as contiguous little-endian u32 values is
`6c069f1bd5824e7addd4db3ffd7a90e96a1368d8f51170f8f78d6311063a6849`.

The native regression
`native_kernel_rounds_to_every_actual_original_float` compares every generated
table entry's raw bits with that original export. It passed for the complete
13,824-entry array. On native source commit
`1457a89`, the composed original/native scalar comparison also matched every
selected raw double sample and declared bound, covering all five terrain
adjustments, junctions, ordered mixtures, boundary points and repeated points.
The [paired validation note](world-generation-beardifier-validation.md) records
the controlled inputs and retained phase comparison. Its whole profile
completed with original/native exit 0 in 6.739/2.070 seconds, respectively;
both processes were reaped. These are bounded execution observations, not a
performance comparison.

The temporary original exporter source SHA256 was
`234a344a4bd17d2c55418cd7d7e00b3c1cf2adc7c4fa92cef944df86ec1511ff`.
Only its observed numeric output is needed by the kernel regression. No
original JAR, class, source archive, raw disassembly or private save is
published. Scalar evaluation does not establish actual StructureManager
selection, structure generation, chunk readiness, Main integration or FULL
publication; those require their real caller owners and separate evidence.
No runtime RNG stream or draw-counter telemetry was collected. The RNG-free
statement concerns the inspected scalar and kernel initializer methods;
registry bootstrap is not a measured global RNG-stream witness.
