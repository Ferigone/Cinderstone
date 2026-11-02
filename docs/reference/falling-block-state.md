# FallingBlockEntity canonical state and persistence

This stage targets unchanged Minecraft Java 1.21, protocol767. Reference is the
pinned inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
and official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`.
Evidence below is direct JDK21 `javap -p -c` inspection, not REA execution or a
Java gameplay observation. Proprietary binaries and raw disassembly stay private.

## Source-owned fields and construction

`FallingBlockEntity` maps to `cjg`. Constructor source lines58–73 initialize
SAND, DropItem=true and FallHurtMax=40; Java zero/null initialization supplies
time, cancellation/hurt booleans, damage amount, block data and teleport flag.
Its source lines76–89 positioned constructor assigns the carried state,
Entity.blocksBuilding=true, actual position, zero motion, previous position and
the block-floor start position. Neither constructor adds an entity to Level.

`FallingBlockState` retains one `EntityPhysicalState`, the actual retained
`LegacyRandomSource`, carried state and all additional fields. The caller passes
the actual allocated entity ID and freshly acquired constructor random owner.
UUID construction consumes exactly two nextLong calls, with UUID version4 and
variant bits. No Level/player/sound RNG or replacement seed is used. There are
no FallingBlockEntity-specific random samples in these constructors.

`new` is the positioned constructor. `new_unpositioned` accepts the actual
registry's default SAND state for the EntityType factory/load path and leaves
blocksBuilding=false. The start position's registered default is ZERO; only an
unequal set dirties metadata slot8, including a later reversion to ZERO.

Original `EntityType` (`bsx`) registration of `falling_block` supplies width and
height0.98f, tracking range10 and update interval20. `EntityDimensions` (`bsu`)
source line12 computes eye height as height*0.85f. The inherited physical
constructor uses those source float values. EntityType numeric identity and its
tracking delta policy remain actual-registry/runtime inputs, not guessed IDs.

The pure source predicates preserve isAttackable=false, isPickable=!removed,
displayFireAnimation=false and onlyOpCanSetNbt=true. `disable_drop` sets the
CancelDrop field, not DropItem=false. `set_hurts_entities` sets hurt=true and
retains the supplied amount/max without invented bounds. NBT can legitimately
load zero, negative or nonfinite damage values; this module does not clamp them.

## Packet data

Source getAddEntityPacket (line350) uses global carried BlockState ID as the
AddEntity data field. `spawn_packet_at` requires the actual certified EntityType
ID and the real ServerEntity-captured position and velocity. It encodes UUID,
source angle/motion quantization and state ID; it does not invent a tracker or
publish itself. Pairing's further metadata/motion packets remain runtime work.

`metadata_packet` and `dirty_metadata_packet` combine inherited fields0–7 with
start-position slot8. Slot8 uses mandatory BLOCK_POS serializer10, not optional
serializer11. Dirty encoding consumes a caller-drained mask without clearing
another observer's data; nondefault encoding omits ZERO start position.

## Java NBT versus native checkpoints

Source addAdditionalSaveData lines272–282 writes BlockState, Time, DropItem,
HurtEntities, FallHurtAmount, FallHurtMax, optional TileEntityData and CancelDrop.
It does not write StartPos, forceTickAfterTeleportToDuplicate or RNG state.
The outer Entity save/load owner is still responsible for physical state,
UUID, dimension and the superclass fields.

Source readAdditionalSaveData lines286–311 is implemented in literal order:

- Read BlockState using the actual Level Block registry; wrong/missing compound
  becomes CompoundTag.getCompound's empty compound for that actual decoder.
- Read Time with NumericTag conversion. Numeric HurtEntities also reads amount
  and maximum; absent/wrong HurtEntities only forces true for actual anvil tag.
- Numeric DropItem overwrites the field; absent/wrong type retains it.
- Only compound TileEntityData overwrites with a deep copy; otherwise retain it.
- Read CancelDrop, then replace an actual AIR carried state with default SAND.

`FallingBlockStateCodec` is a required borrowed owner for BlockState NBT,
AIR/anvil predicates and default SAND. It has no permissive implementation.
Numeric booleans use NumericTag.getAsByte's low byte, not `int != 0`; float and
double integer reads use the original Mth.floor behavior including narrowing,
NaN and out-of-range wrapping after the Java cast.

`FallingBlockSnapshot` is explicitly a richer **native** checkpoint. It captures
the physical/UUID state, exact retained48-bit RNG, carried state and fields,
start-position metadata and teleport/blocksBuilding flags. Restore consumes no
entropy and creates no new ID/handle. Damage amount bits are explicit; optional
block NBT uses the repository's bounded anonymous-NBT codec. An enclosing actual
checkpoint must certify registry identity, world history, index/generation,
shared tick order, tracking and source lifetime. Retained block-data key-set
encounter order is a separate original CompoundTag HashMap owner: the Tag's
serialized field order is not that certificate. A world with blockData requires
the tick host's actual key-order capability or remains explicitly unsupported;
this snapshot alone cannot manufacture the original map capacity/history.
This helper does not certify those owners or make an imported world admissible.

## Integration boundary

FallingBlockEntity.fall (lines92–96) first normalizes WATERLOGGED=false, constructs
at x+.5/y/z+.5, replaces the origin with the ORIGINAL state's fluid legacy state
using flags3 and attempts addFreshEntity. Both source booleans are ignored and
the SAME actual constructed object is returned even if addition returned false.
The positioned constructor here performs only its own state phase; actual world
callbacks, registration, ticking, tracking, damage, landing and save admission
must use the owners supplied by the central integration runtime. No fabricated
item entity or completed falling actor is substituted. This source stage alone
does not establish Main Anvil placement, socket gameplay or checklist completion.

## Atomic native entity cohort integration

The additive EntityCheckpoint `native_entities` payload contains the actual XP
snapshots, FallingBlock snapshots and shared persistent index/section/tick order.
It is covered by the SAME existing checksum and atomic entity-file replacement
as item snapshots, actual world image, source history, time, Level RNG and shared
entity counter. Main captures all these owners under its existing world write
and player-registry locks. There is no second FallingBlock array in SavedWorld.
The world-image/source-history checks are retained without rewriting provenance.
The same checkpoint also binds the SHA-256 of the exact SavedWorld bytes written
by that capture. Main serializes the world once, binds and writes those bytes,
then atomically writes the normal checksummed entity file. Startup validates
the bytes actually read before native restore. This binds BLOCK/fluid queues,
clock and other world-file fields even when two saves have identical block
images. A torn pair is rejected; an existing bound proof cannot be rebound.

Checkpoint validation rejects cross-kind duplicate IDs/UUIDs, out-of-counter
identities, unknown carried states, nonfinite physical JSON fields and an order
certificate that differs from the actual complete body cohort. Restore creates
the retained FallingBlock Box directly from its checkpoint; it does not rerun
the source constructor, draw another UUID or reseed the retained RNG. Actual
shared registrations produce actual new handles, then the runtime restores
the certified mixed order over those handles before the Server is published.
Chunk visibility/readiness and observer sender baselines still start fresh;
this does not claim suspension/resumption of an old VM executor.

An atomic payload that is present, including EMPTY, is authoritative over the
old SavedWorld XP field. A missing payload denotes the old split-file migration
path explicitly: its historical XP/item mixed ordering and cross-file atomicity
were not certified. That path is logged as such rather than described as an
original-order reconstruction. New saves leave the legacy XP field empty only
after capturing the actual XP owner in the atomic checkpoint. A new atomic
cohort, including EMPTY, can no longer be discarded by the historical
item-count-zero fallback or replaced by stale legacy XP. Only an actually
legacy item-only checkpoint retains that narrower fallback. Every nonempty
Item/XP/Falling owner requires the actual supported source at capture.
The pre-existing empty, unsupported-source transport profile is not relabeled
as a complete new cohort: only when every entity store is actually empty may
it keep a legacy item-only capture. Its exact newly written world bytes are
still bound. A later stale legacy empty proof may enter the explicitly logged
legacy fallback; a new atomic cohort or any nonempty payload cannot.

The new atomic FallingBlock profile rejects any present blockData (even an
empty compound) until the genuine retained CompoundTag map owner can be
checkpointed. It also rejects a persistent cohort with vehicle references;
an actual passenger graph/restore owner is not available. Those fields are
never cleared to make a checkpoint pass. The actual Anvil constructor's absent
blockData and unmounted state meet these bounds. The broader callable source
state/NBT APIs do not remove these Main continuation restrictions.

Main contains the shared level-entry and tracking dispatch callsites, and
installs the actual configured damage-holder owner once after BLOCK restore.
These are implemented integration contracts; compilation and runtime validation
are reported by the integration coordinator against the exact composed stage.
They do not establish that FallingBlock creation/physical ticking or ordinary
Anvil placement/menu gameplay has passed a runtime flow.
