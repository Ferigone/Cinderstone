# LivingEntity base tick and air supply

ENTITIES-AND-SURVIVAL source milestone: LivingEntity.baseTick
Instance: es-20261101-1425-a7c3, observed team capacity seven including root.
Claim: issue4 comment6566070764. Base26c4d2ec13b885e51025e105f1edeb0fe78f6309.
Owned edit: ONLY src/living_entities/base_tick.rs in cinderstone-living-native checkout.
No child Git, exports, Main, existing actor, shared state or status/doc edits.

Evidence: direct JDK21 javap -c -p -l of ORIGINAL Java1.21 protocol767 btn;
official server-1.21.txt mapping. No REA tools exposed or invoked.
Oracle: /workspace/scratch/es-20261101-1425-a7c3/oracle/living-lifecycle.javap.
Pinned outer server SHA1 450698d1863ab5180c25d7c804ef0fe6369dd1ba;
mapping SHA1 31c77994d96f05ba25a870ada70f47f315330437;
inner SHA256 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.

Callable exports:
- base_tick(actor:&H::Actor, host:&mut H) -> Result<(),LivingBaseTickError<H::Error>>
- on_below_world(actor,host) -> Result<(),H::Error>
- decrease_air_supply(actor,air,host) -> Result<i32,H::Error>
- increase_air_supply(actor,air,host) -> Result<i32,H::Error>
- LivingBaseTickHost mandatory actual-owner interface, and LivingAirSupplyHost
  extension for real OXYGEN_BONUS AttributeInstance/default air implementation.
- Exact field selectors LivingFloatField/LivingIntField; no field state created.
- EnvironmentalDamage selects canonical source from actual DamageSources.
- LivingBaseTickError distinguishes host error and Java nullable dereference of
  second vehicle/lastHurtMob/lastHurtByMob field read; no successful fallback.

Exact source ledger:
btn.aw395-398: oAttackAnim=attackAnim; firstTick sleeping Optional, method reference
this::setPosToBed3480-3481 dispatched through mandatory actual position operation.
401-406: retain current Level, instanceof ServerLevel, EnchantmentHelper.tickEffects
with that receiver BEFORE invokespecial Entity.baseTick (with real virtual callbacks).
408: reread current Level/profiler, push livingEntityBaseTick.
410-411: fireImmune shortcircuit current Level client-side; actual clearFire.
414-425: initial isAlive gate and captured instanceof Player local; server inWall
hurt1 else player outside-border; each Level/Border reread exact, first Border retained
before boundingBox argument. Distance + separately read safeZone; <0 then actual
damagePerBlock>0; floor(-distance*damagePerBlock) max1 converts i32->f32, virtual hurt.
432-445: isEyeInFluid(WATER), captured Level before X/eyeY/Z and BlockPos.containing,
actual BlockState.is(BUBBLE_COLUMN); breath/real effect utility/player abilities
shortcircuit; virtual decreaseAirSupply, real air write then reread ==-20; set0;
retain one Vec3 deltaMovement before loop; exactly6 entity nextDouble per each of8
bubbles (X difference, Y difference, Z difference), then reread Level/X/Y/Z and
actual addParticle(BUBBLE,...saved velocity), then canonical drown hurt2.
449-453: server passenger, vehicle null test and NEW vehicle read before actual
dismountsUnderwater; actual stopRiding; otherwise air<maxAir, air reread then virtual
increaseAirSupply and real setAirSupply.
456-460: current ServerLevel retained, current BlockPos retained, Objects.equal old
lastPos; store SAME lastPos before virtual onChangedBlock(retained serverlevel,pos).
465-466: NEW isAlive read; actual wet condition || isInPowderSnow -> extinguishFire.
469-476: hurtTime>0 reread/decrement; Entity.invulnerableTime>0 and NOT ServerPlayer
reread/decrement; actual isDeadOrDying then currentLevel.shouldTickDeath -> tickDeath.
478-491: playerTime>0 reread/decrement, ELSE raw lastHurtByPlayer=null (1->0 retains
player this tick); dead lastHurtMob raw clear; lastHurtByMob null/dead/elapsed strict
>100 with wrapping tick-timestamp, virtual setLastHurtByMob(null) retains timestamp.
496-505: actual virtual tickEffects; animStepO, yBodyRotO, yHeadRotO, getYRot/yRotO,
getXRot/xRotO order; current Level/profiler reread and pop. No finally cleanup.
587-600: default decrease reads actual optional OXYGEN_BONUS instance/value; positive
bonus only consumes real entity nextDouble; >=1/(bonus+1) preserves air, otherwise
wrapping air-1. Attribute absence/NaN/nonpositive consumes no RNG.
604: default increase wraps air+4 before reading current virtual maxAir and min.
1953-1954: canonical fellOutOfWorld DamageSource, actual virtual hurt4, bool ignored.

Focused validation:
Standalone original file rustc --edition=2021 --crate-type lib exit0.
Standalone original file rustc --edition=2021 --test exit0 (no diagnostics).
/workspace/scratch/es-20261101-1425-a7c3/nov2-base-tick-tests exit0.
Meaningful recorders exercise lethal suffocation continuing drown bubbles/block
change/death/effects, exact48 entity RNG calls and preloop velocity despite RNG
mutation, enchantment-before-parent and inherited virtual void dispatch, captured
border query and subsequent current-Level rereads, virtual air
shortcircuit and null second vehicle read/error prefix, wrapping attacker expiration,
same canonical Rc<RefCell> actor reentry, effect-error committed counters/no later
snapshots or profiler cleanup, OXYGEN thresholds/NaN/RNG gate/integer wrapping.
Recorders are controlled hosts, not original-Java runtime or production/Main proof.

Remaining actual owners/integration:
No Actor constructor/admission, canonical object arena, Entity.baseTick body,
physical/metadata fields, health/dead flag, RNG, effects map, enchantment engine,
actual world/border/block queries, real hurt dispatch, parent callbacks, removal,
packet audience, scheduler or Main adapter is supplied by this module.
Mandatory virtual calls must use real existing owners; missing owners must error.
Host must retain actor lifetime and each captured receiver, release borrows before
callbacks, preserve normal reentry, and never redirect an actor on ID reuse.
Source helper bodies are defaults ONLY; actual subclass air/void/tickEffects/death
dispatch remains authoritative. No full survival/entities delivery claim.

Independent source review: damage_contracts reviewed the complete original method bodies and found no material mismatch. Controlled-host validation is distinct from original-Java probes and production execution. Root owns namespace exports and repository checks.
