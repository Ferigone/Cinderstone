# Bounded retained Bat physical bodies

This stage binds selected original Entity bodies directly to the concrete
`mobs::actor::NativeBatLease<T>`. There is one canonical physical field, owned
by the allocated Bat lifetime. A short field access releases its lock before
the next world operation or movement callback. No actor, physical mirror, RNG,
tick scheduler, world, collision list or fluid-shape cache is created here.

The files are `src/living_entities/retained_move.rs` and
`src/living_entities/retained_base.rs`. They are selected source bodies, not
complete Entity.move, Entity.baseTick, LivingEntity.travel or Main admission.

## Original source and actual selection

The target is Java1.21, protocol767, DataVersion3953. Source analysis used the
original server outer SHA1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, official
mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`, and inner JAR SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence is direct JDK21 `javap` and an original-method Java runtime probe;
no REA/MCP run is claimed. Proprietary binaries, raw disassembly and probe
classes remain outside the repository.

`Bat` maps to `cez`, Entity to `bsr`, and LivingEntity to `btn`:

- Entity.setPos451–458 invokes setPosRaw3587–3602 before rebuilding its AABB.
  setPosRaw updates block position, clears the cached inside state, updates the
  distinct ChunkPos cache and invokes EntityInLevelCallback.onMove. The native
  lease owns the complementary ChunkPos field beside its existing physical.
  Its current API exposes chunk coordinates as a value tuple; original ChunkPos
  allocation/object identity is not exposed. A later source caller retaining
  that object requires its real object owner before claiming that API.
  An onMove callback therefore sees new coordinates with the old AABB. Nested
  same-actor movement can change coordinates or dimensions; outer continuation
  rebuilds the box from the current canonical fields. A callback failure keeps
  raw-position/index changes and skips that rebuild. ReapplyPosition466–467
  selects this same virtual setter. Equal numeric coordinates preserve their
  existing signed-zero bits but still rebuild the AABB.
- Entity.collide1028–1068 is geometric clipping. The native body uses existing
  `player_physics::collide_motion` and actual `RuntimeCollision` world, border
  and original ordered candidates. The caller supplies real current step-height
  attribute and powder-snow equipment results. A collision result alone does
  not execute movement, position, fall, contact, fire or game-event effects.
- Entity.setOnGroundWithMovement650–652/checkSupportingBlock659–687 uses the
  current AABB's bottom1e-6 probe, actual BlockCollisions/support ordering and
  its horizontal backwards fallback. `on_ground_no_blocks` is read after the
  first query, and the actual source onGround field has one physical owner.
- Entity.getOnPos934–956 retains its support position while querying that
  block's fence/wall/gate facts. Source `fcmpl` skips the query branch for a NaN
  offset; the native comparison preserves that detail. The selected Entity
  block-speed body976–981 is separate from Living510, which first reads actual
  MOVEMENT_EFFICIENCY and interpolates the Entity result toward1.
- Entity.updateFluidHeightAndDoFluidPushing3385–3450 uses the existing native
  fluid solver. The actual Bat is nonplayer and inherits isPushedByFluid=true
  (Entity2934). Its pushing currents therefore take nonplayer normalization.
  Unavailable-chunk early return writes neither velocity nor tag depth. This
  selected body does not clear the whole depth map or perform splash/fire work.
- Entity.updateFluidOnEyes1461–1478 first copies the previous eye-water tag
  into wasEyeInWater, clears current eye tags, retains eyeY, then samples the
  current real fluid. Surface arithmetic adds float blockY and float height
  before widening to double. The current helper is explicitly unmounted and
  uses native vanilla water/lava tags; a mounted Bat requires its retained
  vehicle/Boat owner before execution.
- Entity.updateSwimming1427–1432 uses isUnderWater1423: previous eye-water
  state AND current water contact. It does not use the newly sampled eye tags
  as an immediate replacement. The selected inherited setter changes bit4 of
  metadata0 shared flags through the existing canonical dirty owner.
- Selected Entity.setRemainingFireTicks621–622 and resetFallDistance2673–2674
  act on the actual Bat physical. Player's abilities-dependent fire clamp is a
  different override and is not used for this actual Bat.

## Callable API and source boundaries

`retained_move::set_position` and `reapply_position` require the real onMove
callback as an explicit fallible closure receiving the same retained lease.
No actor lock spans that callback. They are immediately useful to the actual
Living constructor's reapplyPosition boundary; source NULL level callback
before registration is distinct from an invented successful runtime callback.

`collide`, `set_on_ground_with_movement`, `on_position` and
`entity_block_speed_factor` use concrete existing RuntimeWorld/RuntimeCollision
adapters. The caller must borrow those from actual current Level owners and
retain original candidate/query order. Missing shapes/chunks remain errors.
Candidates must exclude this actor by source object identity before entering
the numeric collision adapter. A remaining candidate with the same numeric ID
would be another lifetime after ID reuse; the helper explicitly rejects it
rather than silently exclude it through RuntimeCollision's numeric filter.

`retained_base` exposes `update_fluid_height_and_do_fluid_pushing`,
`update_fluid_on_eyes_unmounted`, `update_swimming`, `set_remaining_fire_ticks`
and `reset_fall_distance`. Numeric query locals are source values; they never
replace the canonical physical body. The concrete runtime fluid adapter has no
actor callbacks, so no source reentry is hidden inside the fluid solver.

## Integration and remaining behavior

The integration coordinator still owns Main, actor-store/index admission,
Level tick membership, source current-world ownership, tracking/persistence,
packet order and shared entity files. No existing Item/XP/Player/Falling actor
or creative body is installed as a Bat proxy. This stage changes no checklist.

Full source parents still require actual profiler calls; retained vehicle and
portal handling; splash/particle/sound/RNG owners; canonical damage/effects and
Living void/death dispatch; fall/block/inside callbacks; attribute reads;
movement emissions and game events; removal and tracking. Those operations are
not silently skipped by these helpers, and there is no top-level baseTick or
move function advertising their completion.

Bat inherits ordinary Living travel and requires MovementEmission.EVENTS.
Its checkFallDamage208, doPush104 and pushEntities109 are literally empty;
that does not make afterFall, stepOn or entityInside empty. Inherited
processFlappingMovement889–895 consults Bat.isFlapping57 and emits the actual
FLAP game event. Bat.isIgnoringBlockTriggers212 is true, excluding its body
from pressure counts while preserving the actual block callback/query phase.
The central Falling same-physical callback loan pattern remains useful, but
its NONE emission and restricted callbacks do not implement these Bat owners.

## Validation

The original runtime probe invokes unchanged original Entity position methods
on an actual Bat-class object with controlled EntityInLevelCallback. Its Unsafe
allocation and explicit initial fields bypass constructors and actual Level
admission; this is source-method ordering evidence, not production lifecycle
or Main proof. It passed nested onMove/current-AABB continuation, failed-callback
prefix, equal signed-zero/no-onMove/AABB-refresh, and NaN getOnPos support
short-circuit cases on the pinned JAR. The original NaN case returns its retained
support with a null Level; a finite positive offset instead queries that Level.

Focused native checks use the actual NativeBatLease storage with an explicitly
unfinished Entity prefix and uninhabited unused external object domains. They
exercise same-physical callback reentry, immediate complementary ChunkPos,
error-prefix/AABB ordering, NaN support query suppression and explicit reused-ID
collision refusal. They do not certify a completed constructor or Main actor.
Native compiler/runtime results are recorded after the concrete actor module
and root's mechanical exports are assembled.
