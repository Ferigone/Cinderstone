# Falling entity pressure-plate callbacks

This is a future source stage, separate from the built Anvil and ordinary
sand/gravel observations. It does not extend those runtime claims or complete a
checklist row.

Primary evidence is direct JDK21 javap inspection of the pinned Java 1.21 inner
server SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
and mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`. This is not REA
execution or an original Java gameplay observation. Raw disassembly is private.

| Original source | Native contract |
| --- | --- |
| Entity.isIgnoringBlockTriggers2895, bsr.r_ | Literal false. Current native Player, ItemEntity, ExperienceOrb and FallingBlockEntity classes inherit it. Future indexed classes must port their own override explicitly. |
| BasePressurePlateBlock.entityInside79–87, dfm | Server side only; invoke checkPressed only when the actual captured signal is zero. Keep the genuine triggering entity for the block game-event context. |
| BasePressurePlateBlock.getEntityCount151 | Actual class query, then NO_SPECTATORS and !isIgnoringBlockTriggers. Health is not a predicate. A dead but still-indexed Player is not replaced by a fake nonliving/zero-health actor. |
| BasePressurePlateBlock.TOUCH_AABB27–29 | Actual x/z range1/16..15/16 and y0..1/4, translated by the real BlockPos. The previous Main range1/8..7/8 was incorrect. |
| PressurePlateBlock.getSignalStrength | EVERYTHING queries Entity.class; MOBS queries LivingEntity.class. A Falling trigger is nonliving, but its callback must still query real Players already on a stone plate. |
| WeightedPressurePlateBlock | Count actual entities, clamp to configured maximum and apply the original positive f32 ratio-times15 ceiling. Preserve actual weighted delay10; base binary plates use delay20. |
| BasePressurePlateBlock.checkPressed90–112 | Real signal comparison/setBlock(flags2), plate and below-neighbor notifications, inherited empty ServerLevel setBlocksDirty, transition sound then game event, then one actual schedule attempt whenever resulting power is positive. |

## Actual owners

`main_falling_block_redstone` supplies the finite source predicate/class facts
and the precise touch bounds, then invokes the real shared redstone kernel
wrapper. It never substitutes a Player, fabricated health, an invented empty
query or an early no-op for a nonliving stone-plate trigger.

The movement host now borrows the one ActualFallingBlockEditServices throughout
Entity.move with the real physical field and disjoint damage/publication fields.
Rain/light, fluid-shape and gameplay-frame borrows are created only within their
actual method calls. A pressure callback can therefore use the same actual
BLOCK, fluid, redstone, source, lighting, sound and publication owners without
aliasing a separately held manager/cache borrow or detaching the actor Box.

The real native entity-section query determines encounter order and current
membership. Current Player/Item owners are queried directly; the same-lock
immutable XP/other-Falling body view is captured before borrowing the retained
Box. The current trigger always resolves through its genuine physical argument,
not its earlier captured bounds. Spectators and source trigger-ignore predicates
remain distinct from LivingEntity filtering.

## Sound and event evidence

The pinned pressure source invokes the four-argument LevelAccessor.playSound.
Its actual dcx.a(Player,BlockPos,SoundEvent,SoundSource) default, source95–96,
passes volume1 and pitch1 to the six-argument method. Level then centers all
coordinates by0.5 and consumes exactly one actual threadSafeRandom nextLong.
The category is BLOCKS4; the triggering entity's silent flag does not suppress a
block sound. Historical lower-volume pressure implementations are not evidence
for this artifact.

Block activation/deactivation retains the actual triggering entity and null
block-state context; a scheduled release has the source null entity context.
The actual GameEvent registry registers both events with radius16. The
LevelAccessor BlockPos overload posts at Vec3.atCenterOf(position), not the
falling body's location or the integer block corner. Only the certified real
empty-listener registry path is supported. Missing or active listener owners are not converted
into successful no-ops.

## Remaining lifecycle boundary

A Falling actor can press a plate and then fail to replace that nonreplaceable
block, producing its real Item drop. The pre-existing Item/XP movement adapter
still rejects pressure-plate entityInside callbacks. Its callback-support/source
certificate also needs the real shared bridge before this can be claimed as a
complete playable falling/plate/drop lifecycle. A powered plate does not justify
silently ignoring that missing owner. This stage does not claim Item/XP plate
support, active custom listeners or broad redstone compatibility.

## Item/XP bridge source contract

This section records the pinned source contract for the separately owned live
Item/XP bridge; it does not assert that its changing implementation has compiled
or executed.

| Actual source phase | Required bridge behavior |
| --- | --- |
| ItemEntity.tick133–212, cjh.l | An empty stack discards and returns before base tick or pressure. Otherwise base tick and pickup-delay handling precede movement. Invoke pressure only within the actual Entity.move call, preserving the grounded low-horizontal-speed modulo4 skip. Do not add an every-tick pressure scan. |
| Item movement continuation | Pressure inside is before movement's speed factor/fire/rain tail and before Item friction/bounce. Neighbor-item merge, age, final water/lava update and despawn follow; preserve them and any synchronous removals at their original locations. |
| ExperienceOrb.tick77–138, btc.l | Base Entity tick, water/gravity/lava/escape, scanning/merge and attraction precede its ordinary SELF movement. Pressure stays inside that real move; friction, bounce, age and expiry follow. Do not add another base tick or entity counter increment. |
| Entity.move | NoPhysics returns before inside-block traversal. The actual position callback updates the index before AABB refresh; the later inside callback receives the refreshed real bounds. Traverse x, then y, then z with the original inward epsilon and check removed before each cell. |
| Entity predicates | Player, Item, XP and Falling inherit false for isIgnoringBlockTriggers. Spectator filtering is separate; a nonplayer saved physical flag does not create a new Java virtual override. Class filtering must preserve genuine LivingEntity membership and must not introduce a health test. |

The current Item tick deliberately loans the real entry out of its map while
keeping its shared index handle. Therefore the pressure query must resolve that
current handle from the actual physical reference passed into the callback.
`tracking_context` is a replication template; its cloned ItemState must not be
used as the canonical current item or as a reason to omit it. Other Item state
comes from the actual TickParts map. No fabricated Player, health value, missing
item or shadow RuntimeEntities/index is valid.

TickParts owns the actual index, remaining Item map, shared sound RNG, tick and
outbox references. A parts bridge must split/reborrow these real fields and
avoid recursive borrowing of the same RefCell. A pressure callback draws no
Item RNG, XP RNG or Level.random. A transition sound draws exactly one actual
shared sound RNG long at the source sound call; do not batch it at tick end or
reseed a replacement stream.

The existing generic provider can retain mutable fluid-cache and rain-exposure
borrows throughout a tick. A real pressure world callback cannot alias the
containing interactions/light owners. Use genuine short owner reborrows/parts,
or declare and certify a narrower actual context where those capabilities are
not acquired or queried; never clone/swap a scratch cache or fake a lighting
owner. Preserve initial base-tick water/lava/eyes/fire/rain handling and Item's
separate final water/lava update around the pressure call. The source certificate
must be bound to an actually registered callback capability; compiling a dfm
branch does not grant every provider pressure support.
