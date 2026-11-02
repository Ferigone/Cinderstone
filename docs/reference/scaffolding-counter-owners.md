# Scaffolding interaction shape and native BLOCK counter provenance

This is a narrow source/Main follow-up to native powder PR122. It does not
complete a checklist delivery or admit adjacent-water player placement.

## Scaffolding interaction shape

Pinned Minecraft Java1.21 `ScaffoldingBlock` (`dmo`), mapped source80,
`getInteractionShape(BlockState, BlockGetter, BlockPos)`, returns
`Shapes.block()` directly. The method does not read state, world, position,
entity context or randomness. The original unit voxel grid can therefore be
provided for this exact declaring class without converting an approximate
outline or inventing a collision context.

The native provider is `bucket_interactions::static_interaction_shape`.
Production falling-block clipping calls the same provider after its real
collision capability checks. This extension supplies only the scaffolding
interaction shape. It does not widen cauldron, hopper, composter, animated
shulker/piston, unavailable FULL-region or unknown fluid-cache support.
Scaffolding movement/collision selection remains with the existing real
Falling actor context; unsupported landing/entityInside hooks stay explicit.
The current source-global item-runtime static-collision guard can reject a
scaffolding-containing world before this ray path. Bucket OUTLINE admission
also remains separate. This constant provider is a bounded callable native
prerequisite, not proof of newly admitted end-to-end scaffolding gameplay.

## Native BLOCK/shared-counter restore

Main's BLOCK and FLUID queues share the actual Level sub-tick counter held by
`MainFluidRuntime`; a BLOCK checkpoint does not own another counter. A dry
palette or empty retained queue does not prove that the counter was unused:
even duplicate scheduling attempts advance it before queue deduplication, and
stale scheduled work remains part of source history.

The source basis is pinned `Level.nextSubTickCount` (`dcw.G_`, mapped996) and
`ScheduledTickAccess.createTick` (`dcx`, mapped42/46). Native persistence is a
separate explicitly owned format, not a claim of Java save/VM suspension
interoperability. Existing cold `SavedTick.unpack` policy and relative delays
remain unchanged.

The restore admission must retain the real saved clock and FLUID shared-counter
checkpoint whenever the actual BLOCK checkpoint or entity-source history
requires that owner. It must reject missing provenance before live adoption;
it cannot synthesize counter0 from a dry world, current queue length or a
missing field. Genuine fresh/never-used legacy behavior remains a distinct
case. Current writers already save the paired owner.

The saved-owner requirement must be inspected without constructing a live
runtime, immediately after SavedWorld format decoding and before native block
entities, listener binding or Level/JdkMath RNG creation. Later source-history
and actual restored/adopted runtime checks remain independent. A review caught
that a final-only guard was too late for this pre-entropy condition; that first
compiled snapshot is superseded and has no runtime claim.

## Evidence and validation boundary

Original inspection uses direct JDK21 `javap` against the pinned inner server
SHA256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
and official mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`.
REA MCP is not exposed in this session. No Java binaries, raw disassembly,
private world/checkpoint or source archive is published.

Product source `c2c1b77cf3ee6edd29f3c4d12d2483c4b4dfac8b`, tree
`03e44471ba4620cb6d9e5d48f964f4285f9659c6`, passed isolated locked offline
all-target compilation5.63s and executable build15.46s after the review fix.
Binary SHA256 is
`4f8b5af1d882b6a9155dfc91b93652a6ddedf6b4d96e7c2390482d108d37d855`.
The [finite actual report](../validation/scaffold-counter-flow.json), SHA256
`fbc858959f2557af93d88acc7a02fd11813b9168a487b5227930f59f3641a963`,
records one genuine paired native restore: sticky BLOCK usage/empty queues,
shared counter4→4, STATUS1.21/767/online0/PING, normal SIGINT exit0. A separate
explicitly malformed copy omitted ONLY `fluid_ticks`, retaining fire/native
files without integrity repair; it exited1 with the exact early BLOCK pairing
diagnostic before listening. Original controlled inputs stayed byte-identical.

The exported interaction-provider probe used the actual canonical scaffold
state domain18372–18403 and public voxel-grid/clip APIs. It verifies that
constant provider only; no Level, bucket OUTLINE, high-speed Falling or new
source-world admission is claimed. Public report metadata contains no private
paths, save contents or key material; raw process logs remain local.

The earlier e2274b9 compiler snapshot passed but was superseded for guard
ordering and was never runtime-proven. The preceding real powder gameplay
flow remains bound to its unchanged original source/binary/report; it is not
relabeled as validation of this newer product. Composed fresh-master CI and
release verification are recorded with the issue/PR at publication.
