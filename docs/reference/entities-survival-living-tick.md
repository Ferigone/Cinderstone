# LivingEntity tick phases

ENTITIES-AND-SURVIVAL implemented component: LivingEntity.tick
Instance es-20261101-1425-a7c3; observed team capacity seven including root.
Pre-edit claim issue4comment6566386887. Exact stackedbase
ab410119874a261608f3e06b952af6fcb94db256 in cinderstone-living-tick checkout,
branch teams/entities-survival/es-20261101-1425-a7c3/living-tick-travel.
Owned edit ONLY NEW src/living_entities/tick.rs. No child Git/export/docs/Main/
shared state/manifest edits. Root separately widened existing canonical Bat
pure helper visibility without algorithm changes; this child did not edit Bat.

Evidence: unchanged pinned original Java1.21 protocol767, official mappings,
direct JDK21 javap -c -p -l. No REA exposed/invoked. Full source plan in sibling
nov2-living-tick-api-source-plan.txt; raw oracle/living-lifecycle.javap.
Pinned outer SHA1 450698d1863ab5180c25d7c804ef0fe6369dd1ba;
mapping SHA1 31c77994d96f05ba25a870ada70f47f315330437;
inner SHA256 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.

Callable APIs

tick(actor:&H::Actor,host:&mutH) -> Result<(),H::Error>
update_swim_amount(actor,host) -> Result<(),H::Error>
LivingTickHost mandatory retained actual Actor/Level/Profiler/CombatTracker
interfaces, source callbacks, and with_tick_fields callback-free projection.
LivingTickFields is a borrow/projection trait, NOT a state struct: reads/writes
actual TickIntField/ TickFloatField plus source Entity.xo/zo in existing actor
physical/body owners. No actor, field values, health, RNG or scheduler created.

Exact ported body: btn.l0-748 / LivingEntity.tick2468-2595

2468: literal inspected Entity.tick487-488 -> actual VIRTUAL baseTick callback.
No raw Entity.baseTick substitution, extra tickCount advance or second livingtick.
2469: actual private updatingUsingItem owner required; source3137-3146 tests
ItemStack.isSameItem (item only, NOT components) and refreshes canonical useItem
reference before actual item update/finish/stop callbacks.
2470: source private updateSwimAmount3166-3172 implemented here: snapshot old swim,
virtual visuallySwimming, fresh current swim, source f32 .09 and Java min/max.
2472-2502: one captured current-Level side decision; source arrow/stinger separate
getters, wrapping20*(30-count), decrement, reread expired<=0, capturedcount setter;
actual detectEquipmentUpdates; tickCount%20 recheck real captured CombatTracker;
sleep/checkBed/stop source branch. Counts are not clamped or captured together.
2506-2507: isRemoved gates ONLY virtual aiStep. No alive/dead/upfront removedreturn.
Source item/server/pre-AI and all post-AI phases still run on same retained actor.
2510-2538: double dx/dz from final coordinates minus exact xo/zo; sumcastto f32
before sqrt; desiredBodyYaw/run/step locals and oldRun snapshot; movementthreshold
.0025000002f, sqrt(f64(floatdistance)) castf32 *3; canonical Mth.atan2 castf32
BEFORE *57.295776f -90; wrap-yaw absolute95/265 strict bounds; fresh attackAnim
and getYRot override; onGround target reset; exact f32 run arithmetic.
2540-2574: each headTurn/rangeChecks profiler push/pop resolves current Level/
Profiler FRESH. Mandatory virtual headTurn may select Mob BodyRotationControl,
not generic math by default. Preserve returned step local through rangework.
Eight separate source while loops: yaw negative/positive, body negative/positive,
pitch negative/positive, head negative/positive; exact <-180 / >=180 boundaries,
virtual getters each condition, oldfield reread before +-360 writes. No modulo,
combined loop, finite guard or iteration cap; NaN exits as originalcomparisons.
2576: current animStep += retained returned step after all head/range callbacks.
2578-2585: fresh fallFlying -> wrapping actual fallFlyTicks+1 else0; NEW sleeping
read -> actual setXRot(0). No use of an earlier AI/travel/sleep predicate.
2588-2593: mandatory actual refreshDirtyAttributes1091-1096; actual getScale local;
fresh existing appliedScale != comparison; write capturedscale BEFORE actual
refreshDimensions. NaN always differs; signedzeros equal; failures keep prefix.
This inline tail binds effects/refresh.rs pieces INDIVIDUALLY. Do not also call
its optional tick_attribute_tail from refreshDirtyAttributes (doubleexecution).

Canonical pure helpers imported from mobs::bat::{mth_atan2,wrap_degrees}; root
made their visibility pub(crate). No reimplementation/libm approximation/parallel
numeric owner. Private Java min/max preserve NaN and signedzero semantics.

Actual-owner callback requirements remain explicit

Actor lease retains same actual lifetime through removal/ID reuse/reentry;
with_tick_fields closure operates callback-free on canonical fields in place,
never moving/copying state; releases ALL borrows before actor/world callbacks.
Missing owner returns Error before field fragment; completed fragments persist.
Actual updatingUsingItem/equipment/CombatTracker/sleep/AI/head/attribute/scale/
dimensions owners are mandatory and cannot return successful placeholders.
Host must preserve source subclass dispatch (e.g. Mob.aiStep/headTurn, Player
wrappers), packet/callback order, actual object identity and real metadata.

Focused validation (child only standalone compiler, no Cargo/shared target build)

Actual-file harness nov2-living-tick-harness.rs imports current worktree Bat,
spawning and new tick files, with existing prebuilt mc121_rust dependencies.
Modern stable rustc (matching prebuilt library) --edition2021 --test -Dwarnings
exit0 without diagnostics; native executable nov2-living-tick-tests
filtered living_tick::tests exit0. No claim to run fullcrate or Main.
Initial attempt with old1.85 compiler rejected ABI-incompatible existing rlib;
matching installed moderncompiler resolved that validation setup mismatch.

Meaningful flows: literal virtualbase/item/swim phase and postcallback swim reread;
captured servergate despite later worldchange; highcount/overflow timers and
arrowsetter mutation affecting next stingergetter; negative divisible20 tracker;
removed actor skips ONLY AI; AI removal+replacement numericregistry entry retains
old actual actor; nested sameRcRefCell actor tick releases every field borrow,
outer movement/step locals survive inner mutation and current Level/profiler
rereads; separate whileloops with yawgetter changes/failure keep oldfieldprefix;
NaN run/swim/rotations/scale, wrappingflytimer, late sleeping setPitch and committed
appliedScale on dimensionfailure; attackAnim override/ground target/headfailure
without synthetic pops. These are controlled callback recorders, NOT Main or
unchanged-original-Java actor runtime proof. Canonical numeric helpers retain
their previously documented original-Java probe evidence; no new oracle run here.

Source review of sibling ai_step.rs performed independently, no edits: found
ground-jump NaN guard fcmpg/ifgt mismatch; author fixed <=epsilon guard and added
focused flow. Exact correction re-inspected; all other productionai/ridden/fly/
jump helper paths reviewed against original. Final bounded approval sent root.

Remaining integration/unsupported scope

No actor constructor/admission, physical/body/living owner, world/metadata/AI
navigation, real item/equipment effects, attribute production, tracker, health,
damage/death, RNG, tracking/persistence, scheduler or Main adapter supplied here.
Actual aiStep/travel implementations live in separate claimed files; selecting
them and subtype overrides requires genuine runtime dispatch and real owners.
Existing creative body does not automatically call this method or satisfy its
contracts. Full entities/survival delivery and production integration remain
pending; standalone callback flow success is an explicit partial source milestone.


Bounded unchanged-original Java runtime evidence, living_lifecycle
Task: root requested original Living.jumpFromGround NaN/epsilon/Mth behavior and
final getMaxHealth/getMaxAbsorption virtual attribute dispatch; optional swim.
Only scratch files/classes/logs created; no repository or Git writes.

Original source/methods

Pinned Java1.21 protocol767 inner server jar remains SHA256
8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01,
17,634,864 bytes. Its original btn.ff2143-2160, btn.eN1791,
btn.eO1795, private btn.N3166-3172 and Mth are NOT transformed/replaced.
Original Entity vector getters/setters/addDeltaMovement bodies execute too:
generated tracing overrides invoke their actual bsr parent methods via
invokespecial. Actual immutable original Vec3 objects store results.
Reflection asserts jump method declaring class=btn, and health getters both
declaring class=btn and FINAL. Canonical actual buw.s/buw.r holders are passed
to the generated getAttributeValue override and compared by reference identity.

Probe mechanism and limits

oracle/LivingJumpAttributeProbe.java uses the ControlOrderProbe ASM callback
subclass pattern. Generated JumpAttributeActor extends ORIGINAL btn and supplies
controlled jumpPower, sprinting, yaw, getAttributeValue and visuallySwimming
callbacks. sun.misc.Unsafe.allocateInstance BYPASSES ALL actor constructors.
Actual original Entity velocity/hasImpulse and Living swim fields are used, but
there is NO admitted/constructed actual actor, AttributeMap, world, scheduler,
AI/travel, persistence/tracking/network or Main proof. Values in callbacks are
controlled facts, not a real attribute owner or a fabricated production default.
The .42 case means original default JUMP BODY with CONTROLLED power .42; it does
not prove default getJumpPower block/attribute/effect production calculation.
No REA tool was exposed/invoked; evidence is direct pinned JVM runtime plus
existing javap/mapping analysis. Full Living.tick/range loops were NOT probed.

Compiler/runtime

Actual installed OpenJDK21; compile using java --module
jdk.compiler/com.sun.tools.javac.Main with ASM export on BOTH compiler VM and
compiler arguments, -proc:none. Runtime uses same export. Classpath is original
server-inner-1.21.jar plus ALL sorted bundled library jars, and separate scratch
oracle/jump-attribute-probe-classes output. Compiler exit0, empty compile log;
full JVM probe exit0 with final PROBE PASS. No native/repository compiler run
claimed as part of this oracle-only task.

Exact observed/asserted cases

Jump power -1f(bf800000), +0f(00000000), nextDown(epsilon)(3727c5ab), and
epsilon1e-5f(3727c5ac): return after jumpPower callback ONLY. Velocity initial
[1,2,3] and hasImpulse=false remain unchanged.

nextUp(epsilon)(3727c5ad): jumpPower -> original getDeltaMovement -> source
setDeltaMovement(components) -> original vector setter -> sprinting(false),
then actual hasImpulse=true. Y doublebits3ee4f8b5a0000000.

Canonical Float.NaN(7fc00000): SAME source sequence continues, producing real
Y doubleNaN7ff8000000000000 and hasImpulse=true, retaining X1/Z3. Confirms source
fcmpg/ifgt means Java `power<=epsilon` returns; `!(power>epsilon)` is incorrect.
Native author already corrected that source mismatch before this observation.

Controlled power .42f(3ed70a3d), nonsprint, initial[1,2,3]: Y is REPLACED with
double(float.42)=3fdae147a0000000, even though oldY2 is greater. X/Z unchanged.

Controlled power .42, sprint true, source yaw90f, initial[0,2,0]: source float
Mth angle/sine/cosine and addDeltaMovement produce exact double velocity bits:
X bfc999999999999a (-.2), Y3fdae147a0000000,
Z3c7c3d0a00000000 (tiny positive NONZERO float-table cosine contribution).
Sequence after first vector write: sprinting -> yaw -> original getVector ->
original vector setter(addDeltaMovement result). hasImpulse=true at end.

Controlled sprint callback failure after initial .42 vector write: original
method throws actual IllegalStateException; already-written Y remains .42 but
hasImpulse=false because its final source write was not reached. No rollback.

Original FINAL eN/getMaxHealth and eO/getMaxAbsorption both call generated actual
VIRTUAL g/getAttributeValue override with canonical MAX_HEALTH/MAX_ABSORPTION
holders respectively. Controlled doubles37.2500004 and6.5000001 yield source
float casts42150000 and40d00000. Only those two ordered attribute callbacks occur.
Controlled NaN and -0d yield floatNaN7fc00000 and negativezero80000000; getters
insert no AttributeMap lookup, fallback or clamp. Source helpers must preserve
this virtual dispatch even though the outer getter itself is final.

Unchanged private updateSwimAmount N: initial actual swim=.2(3e4ccccd), virtual
visuallySwimming callback mutates current swim to .8, then returns true. Original
body saved old swim=.2 BEFORE callback and rereads current swim afterwards,
yielding .8f+.09f floatbits3f63d70a. With current canonical Float.NaN and false
swimming, both saved old and updated current stay NaN7fc00000 (Math.max propagates).

Exact artifact SHA256

oracle/LivingJumpAttributeProbe.java (13,333 bytes):
5b0e57e10d588754ccb15dbdc2f15ad4ae954def1c413647a39cc9aaf9124a1e
oracle/living-jump-attribute-compile.log (0 bytes):
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
oracle/living-jump-attribute-probe.log (3,314 bytes):
e01f80be13cf1a80470f5c546a43cc98ec4f782ca3e14a16085fbbc7e5788844

Handoff: original jump guard finding and exact sprint bits sent root/AI author;
final-getter virtual dispatch finding sent root/effects-refresh author; private
swim observation sent root. Root owns any code/docs/PR/export/merge changes.
