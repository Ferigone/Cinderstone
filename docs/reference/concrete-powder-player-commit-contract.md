# Concrete Powder player commit: source contract

This is a contract for a **future synchronous, single-placement committer**.
It does not implement that committer, remove the current player-fluid guard,
expand production admission or establish Java-save/client compatibility.
The existing [kernel](concrete-powder.md) and
[runtime evidence](concrete-powder-runtime.md) retain their own narrower scope.

Evidence is direct JDK21 `javap -p -c -l` inspection of the unchanged Minecraft
Java 1.21 server, protocol767. The inner-server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`, matching
[source-lock.json](../../fixtures/source-lock.json). This was not REA MCP or
gameplay execution. No Java binary or raw disassembly is distributed here.

| Original mapped method | Required behavior |
| --- | --- |
| `ConcretePowderBlock.getStateForPlacement`, `dhk.a`,43–50 | Read the actual target, then the exact hardening kernel. Wet selection returns the retained concrete default; dry selection delegates to the powder default. |
| `BlockItem.placeBlock`, `cso.a`,153; `Level.setBlock`, `dcw.a`,217 | Preserve player flags11 and the default recursion depth512. Do not substitute the retained Falling landing profile's flags3. |
| `LevelChunk.setBlockState`, `dvi.a`,223–289 | An actual changed write and old removal precede new-state `onPlace`; that call is at bytecode394/source273, before returning to Level publication. |
| `FallingBlock.onPlace`, `diu.b`,32–33 | Powder inherits exactly one delay2 schedule attempt. Direct wet selection writes concrete, whose base onPlace schedules nothing. |
| `Level.setBlock`, `dcw.a`,222–270 | Reread and check actual requested-state identity; dirty/publication; flag1 `blockUpdated(oldBlock)`; conditional new-state analog-output update; old indirect shapes; new direct shapes; new indirect shapes; `onBlockStateChange`. |
| `BlockBehaviour.updateIndirectNeighbourShapes`, `dtb.a`,141; `hasAnalogOutputSignal`, `dtb.c_`,252 | For the finite classes below, indirect calls inherit the literal return body. Powder/concrete inherit false analog output, so the output-signal branch is genuinely skipped. |
| `NeighborUpdater.UPDATE_ORDER`, `eqi`,18 | Neighbor callbacks: WEST, EAST, DOWN, UP, NORTH, SOUTH. |
| `BlockStateBase.updateNeighbourShapes`, `dtb$a.a`,1121–1126; `BlockBehaviour.UPDATE_SHAPE_ORDER`,88 | Direct shapes: WEST, EAST, NORTH, SOUTH, DOWN, UP. The actual neighbor receives the opposite direction and original changed position; preserve reduced flags10/depth511. |
| `LiquidBlock.neighborChanged`, `dko.a`,145–148; `updateShape`,136–140 | Ordinary WATER neighbor callbacks attempt a tick for source and flowing fluids. Shape callbacks schedule only when either participating fluid is a source. |
| `LevelAccessor.createTick`, `dcx.a`,42/46; `Level.nextSubTickCount`, `dcw.G_`,996 | Both BLOCK and FLUID consume the same Level post-increment counter before queue deduplication. Keep every attempt in source order. |

The DOWN-only water case matters: the powder placement kernel initially skips
DOWN when the actual target is not water. Powder above source water therefore
keeps its delay2 onPlace attempt, followed by a DOWN water neighbor attempt and
a DOWN water shape attempt. The latter may deduplicate in the fluid queue but
still consumes the shared counter. Flowing water below has the neighbor attempt
without that source-fluid shape attempt. Side/target water can instead select
concrete directly, with no powder onPlace tick.

The proposed first scope admits **one changed nonliquid powder/concrete target**
with certified inert old/new callbacks and only ordinary base Block, AIR or
LiquidBlock WATER neighbors. It excludes lava, fire, waterlogged/custom shapes,
block entities, active redstone, adjacent powder and recursive write cascades.
The admitted classes inherit the indirect return body; default neighborChanged
calls only `DebugPackets.sendNeighborsUpdatePacket` (`agf.a`,198), itself a
literal return. These are source proofs, not assumed empty callbacks.

Before mutation, retain the actual locked World/Level, native source history,
configured powder-to-concrete association, current FULL query region, lighting,
BLOCK and FLUID queues, fluid-shape cache and shared sub-tick owner. Validate the
complete read set and all callback capabilities before either write or output.
The committer must retain the genuine `PlayerPowderOperation` minted by the
checked Main registered-player/hand/held-stack/source flow and its Level-bound
context. The planner's `used_powder` boolean is only a routing marker; it is
**not authority** to commit, replace the player, or promote a projection into
the real Level.

Execute actual write/removal, consume represented onPlace once, then publish.
At the source neighbor barrier, the existing actual WATER aggregate can cover
neighbor and liquid-shape phases only while all intervening non-water and
indirect callbacks are proven inert as above. Pass flags11 unchanged. Finish
the genuine placement sound/event tail afterwards. Do not enqueue/replay the
already executed write/onPlace prefix through the old FIFO, fabricate empty
entity queries or reuse the flags3 landing committer unchanged. General mixed
contexts require real per-direction callback phases beyond this contract.

One separate inherited limit remains explicit: original LiquidBlock.onPlace
129–132 has no old/new Block-identity gate. The native fluid helper's
`name(old) != name(new)` guard is not a general source-equivalence certificate.
The proposed new powder/concrete state is nonliquid, so this does not justify
extending this contract to arbitrary liquid writes or property changes.
