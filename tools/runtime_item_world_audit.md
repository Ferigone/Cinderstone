# Item runtime callback audit — original Java 1.21

Original inner server JAR SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Relevant unchanged bytecode and an original-class reflection declaration audit
are preserved in `tools/runtime_item_world_bytecode.txt`.

## Inside-block checks

`Entity.tryCheckInsideBlocks` (`bsr.aG`) calls `checkInsideBlocks` and wraps any
exception with entity collision crash context. It adds no gameplay operation.

`checkInsideBlocks` (`bsr.aT`) calculates integer endpoints by flooring each
component of `AABB.min + 1e-7` and `AABB.max - 1e-7`. It first invokes
`LevelReader.hasChunksAt(min,max)`. That returns false when `maxY < minBuildY`
or `minY >= maxBuildY`; otherwise it tests all touched chunks, with chunk X
outermost and chunk Z innermost. Any absent chunk skips the entire inside pass.

With available chunks, the cell loop is **X outermost, Y middle, Z innermost**.
This differs from the x-fastest collision cursor. Before each block-state fetch,
`isAlive()` is checked. ItemEntity inherits the base `!isRemoved()` result.
For each cell the order is:

1. Read the actual block state.
2. Invoke `BlockState.entityInside(world,pos,entity)`.
3. Invoke `Entity.onInsideBlock(state)` even if step 2 removed the entity.
4. Advance to the next cell, whose initial alive check can terminate the scan.

ItemEntity inherits the no-op `Entity.onInsideBlock`. Air, stone, dirt and grass
inherit the no-op `BlockBehaviour.entityInside` (`dtb.a`). Do not turn the
entire inside pass into a no-op for arbitrary blocks: cactus, fire, portals,
powder snow and other concrete overrides have actual effects.

## Falling and dry ground

ItemEntity inherits `Entity.checkFallDamage` (`bsr.a(double,boolean,dtc,jd)`):

- On ground: if `fallDistance > 0f`, invoke `block.fallOn`, then emit
  `GameEvent.HIT_GROUND`. Its affected block state comes from the current
  supporting-position state when that optional position exists; otherwise it
  uses the supplied ground state. Finally invoke `resetFallDistance`, including
  when the previous fall distance is zero, negative or NaN.
- In air with `movedY < 0d`: update using the exact float narrowing,
  `fallDistance = fallDistance - (float)movedY`.
- Otherwise leave the float unchanged.

Base `Block.fallOn` (`dfy.a`) calls
`entity.causeFallDamage(distance,1f,entity.damageSources().fall())`.
For an ordinary unmounted ItemEntity this reaches the base Entity method and
returns false without health loss. The base method still checks the entity-type
fall-immunity tag and would recurse into passengers if present. No RNG is used.

Base `Block.updateEntityAfterFallOn` multiplies the current velocity by
`(1d,0d,1d)` and stores it. This is **multiplication**, preserving a negative
zero Y from a negative incoming Y. Replacing it with literal positive zero
changes the original numeric result.

Base `Block.stepOn` is a no-op. Reflection against the original objects confirms
all three methods are inherited from `dfy` for stone, dirt and grass block
(and air). Their `entityInside` method is inherited from `dtb`.

`ItemEntity.getMovementEmission` returns `NONE`. This suppresses the normal
movement sound/event branch but does not suppress the separate HIT_GROUND event
inside `checkFallDamage`. ItemEntity's `resetFallDistance` is the base method,
which assigns `+0f` and has no player-statistic or impulse-context callback.

## Extinguishing sound and RNG

`Entity.playEntityOnFireExtinguishedSound` draws two entity `nextFloat` samples
before calling `playSound`, even when the entity is silent. It computes:

```text
volume = 0.7f
pitch = 1.6f + ((firstFloat - secondFloat) * 0.4f)
sound = GENERIC_EXTINGUISH_FIRE
```

All pitch operations remain float. `Entity.playSound` then checks `isSilent`.
When not silent it calls `Level.playSound`, which draws one **world** `nextLong`
for the sound seed before invoking the seeded sound method. ItemEntity's sound
source is **AMBIENT** (`avq.i`), not NEUTRAL.

## Water splash

ItemEntity inherits `Entity.doWaterSplashEffect`. An unmounted item uses its own
velocity and scale `0.2f`. Volume is the float minimum of 1f and:

```text
(float)sqrt(vx*vx*(double)0.2f + vy*vy + vz*vz*(double)0.2f) * 0.2f
```

The float cast occurs before the final float multiplication. Pitch draws two
entity floats and uses `1f + ((firstFloat-secondFloat)*0.4f)`. The normal and
high-speed splash getters both return GENERIC_SPLASH for the base Entity/item.
Non-silent sound playback then consumes one world long as above.

After the sound, two particle loops execute. Their bound is the float expression
`1f + dimensions.width()*20f`, compared to the integer loop index cast to float.
The normal item width of `0.25f` gives six bubble iterations followed by six
splash iterations:

- Each bubble draws three entity doubles: X offset, Z offset, then Y-velocity
  subtraction by `randomDouble * (double)0.2f`.
- Each splash draws two entity doubles for X and Z offset.
- Both loops compute offsets as `(randomDouble*2d-1d)*(double)width`.
- The particle Y coordinate is `(double)((float)floor(itemY)+1f)`.
- Finally emit `GameEvent.SPLASH`.

Thus a normal unmounted item splash consumes two entity floats and 30 entity
doubles, plus one world long when not silent. Silent status suppresses sound
playback/world sound-seed consumption, not the entity draws or game event.

Crucially, the called `Level.addParticle(ParticleOptions,DDDDDD)` is a no-op on
the server: original ServerLevel inherits it unchanged, as confirmed by
reflection. ServerLevel's separate `sendParticles` methods are not this call.
The server must consume these RNG samples but must not synthesize twelve
network particle packets from them.

This audit establishes callback behavior and ordering. It does not implement
arbitrary concrete block effects, vibration listeners, packet broadcasting,
passenger mechanics or the complete ItemEntity tick.

## Bounded damage provider

`RuntimeItemWorld` now delegates damage to the separately proved original
`ItemEntity.hurt` kernel in `item_damage.rs`. OnFire and Lava use the signed-JAR
fire tag and no-causing-entity facts. The synchronous host game-event callback
sees the new health and `hurtMarked=true`, before destruction and removal.
`hurtMarked` is independent of `hasImpulse` and does not dirty entity metadata.

Destruction executes the exact base Item return, or BlockItem's branch with an
absent effective CONTAINER component. That branch sets CONTAINER=EMPTY in place
before discard; it is not a no-op and does not dirty item metadata slot 8.
Present containers and Bundle remain explicit unsupported errors. The removal
callback requires no vehicle/passengers and runs synchronously with removed=true,
after stack mutation. It must remove the original level/index/tick registration;
it does not emit ENTITY_DIE. Original ItemEntity.tick may continue after removal.

The concrete provider comparison covers 440 original server hurt states,
including 84 destructions, plus 62 OnFire/Lava mapping comparisons. Damage-event
stack snapshots precede destruction; discarded stack bytes match the actual
original destruction oracle. All 33 deferred default destruction contexts and
unsupported host/riding callbacks are checked as errors. See
`evidence/runtime-item-world-conformance.json` and
`evidence/item-damage-conformance.json` for the distinct proof scopes.

## Explicit fluid shape history for the fall-reset ray

Original FlowingFluid owns four process-lifetime IdentityHashMaps, one for each
source/flowing water/lava singleton. The key is canonical FluidState identity,
including amount and falling. The first getShape query stores a box whose height
is the current f32 getHeight promoted to double. Later queries reuse that shape
without reading the world, even at other positions, worlds or above-fluid states.

`FluidShapeCache` imports explicit Unknown/Unqueried/CachedHeight observations.
There is no default empty history. `from_observed_empty_owners` is a controlled
initializer whose supplied owners must actually have been observed empty; the
oracle records this independently for all six fresh JVMs. Its 36 canonical states
exclude the original amount==9 early branch, which none of those states can reach.
World-query failures leave an Unqueried key unchanged.

`RuntimeItemWorld::with_fluid_shape_cache` borrows the shared cache. The original
fall-reset DDA now obtains the cached water shape and calls the proved VoxelShape
clip routine. A missing cache or Unknown history remains an explicit error.
Both block and fluid shapes are evaluated before choosing a hit. Native provider
tests exercise first-query shallow/full differences, later context changes,
cross-world/waterlogged aliases and separation of source/flowing owners.
The new oracle compares 237 shape queries, 119 world reads, 24 original returned
shape clip booleans and every key after every query. It is separate from a full
executable BlockGetter.clip DDA oracle; see `evidence/fluid-shape-conformance.json`.
