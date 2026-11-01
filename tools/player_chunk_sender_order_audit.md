# PlayerChunkSender pending-order dependencies, Java 1.21

The helper is a private sibling of `player_chunk_sender.rs`. It implements the
default fastutil 8.5.12 LongOpenHashSet and the sequential Guava 32.1.2-jre
Comparators.least collector used by the unchanged original `arq.collectChunks`.
Both dependency JAR hashes are checked against the pinned official server's
META-INF/libraries.list. ChunkPos distance comparisons invoke the original inner
Minecraft JAR, SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

## Stream order differs from iterator order

The actual default set starts with n=32, minN=32 and load factor0.75. Zero has a
separate presence flag/slot. Nonzero keys use original HashCommon.mix(long),
linear probing and the original shiftKeys removal/descending rehash traversal.
The 25th insertion grows a capacity32 table. Removal shrinks only when n>minN,
n>16 and the new size is strictly below maxFill/4.

The **Java Iterator** returns zero first and scans table slots in descending
order. The **spliterator** returns zero first and scans slots in ascending order.
Both boxed stream() and primitive longStream() use that spliterator order.
Original arq uses these streams in both collection branches, so the native
`PendingChunks.iter()` deliberately exposes their ascending encounter order.
Using the earlier tracked-entity map's descending iteration would change ties.

The oracle records complete key slots, zero presence, size/capacity/minN/maxFill,
mutation return/contains values, Java Iterator and both stream orders after each
mutation. All three sequences are recorded separately rather than assuming their
relationship. Their differences occur in 2,189 of the 2,235 mutation states.

## Integer distance and TopK selection

Original ChunkPos.distanceSquared computes dx, dz, dx*dx+dz*dz as Java int
operations. Every subtraction/multiplication/addition wraps; a negative overflow
result sorts before nonnegative values in comparingInt. Packed chunks use X in
the low32 bits and Z in the high32 bits.

Guava buffers at most 2*k values. After the first k, a candidate is accepted only
when it compares **strictly less** than the current threshold. Filling 2*k runs
the original quickselect partition sequence; partition uses strict-less swaps,
which can reorder equal-distance chunks. The iteration bound is
3*ceil(log2(2*k-1)); its fallback stably sorts only the remaining subrange.
The threshold is recomputed over the retained k prefix. Finishing stably sorts
the current buffer and retains at most k values. A stable sort of the complete
pending input is therefore not an equivalent implementation.

The corpus includes 189 selections with 232,476 offered values. The original
TopKSelector's private fields are observed through reflection to count 675 actual
trim resets. The collector result is checked against directly invoking that
unchanged selector. Two adversarial equal-distance bands exercise the bounded
quickselect fallback: the comparator observes actual Java Arrays.sort stack
frames inside TopKSelector.trim, without rewriting any class or method. Native
tests verify that the matching fallback branch is exercised. There are 29 final
selections whose tie order differs from a stable full-sort prefix.

Repeated immutable arrays are interned in the fixture. Native runtime code does
not load any fixture or expected output; tests resolve pool references before
comparing the source input, every original distance, layout and ordered result.

## API and scope

`PendingChunks::{new,insert,remove,contains,len,is_empty,iter}` has the required
private sibling visibility. `distance_squared(center,chunk)` returns wrapped i32.
`least_by_distance(iterator,k,center)` implements the actual sequential collector;
k obeys Guava's 0..Integer.MAX_VALUE/2 constructor precondition. k=0 still consumes
the iterator, while accepting no candidates and invoking no comparisons.

The proof covers only these pending-order dependencies. Original arq public
quota/ACK gates, readiness filtering and sending belong to the separate sender
oracle. Parallel stream combining, iterator mutation, set serialization/trim,
nondefault constructors/load factors, allocation failure and custom comparators
are outside this helper's scope.

Reproduce with `python3 tools/player_chunk_sender_order_check.py`. The optional
`--standalone` mode compiles the same private source via a minimal rustc test
harness, using already-built serde_json test dependencies.
