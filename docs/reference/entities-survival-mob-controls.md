# Nov 2 ordinary Mob controls owner stage — subagent handoff

Instance es-20261101-1425-a7c3; exact root claim issue4 comment6566070764.
Base master26c4d2ec13b885e51025e105f1edeb0fe78f6309.
Worktree /workspace/cinderstone-mob-owners-es-20261101-1425-a7c3.
Only edits: new src/mobs/controls.rs and controls/{move_control,look_control,jump_control,body_rotation}.rs.
Root owns exports, reference docs, Git/GitHub and integration. No other file edited, no Git mutation, no Cargo invocation.

## Pinned original evidence

Official server1.21 outer SHA1 450698d1863ab5180c25d7c804ef0fe6369dd1ba; official mappings31c77994d96f05ba25a870ada70f47f315330437. Original inner jar SHA2568d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
Direct JDK21 javap, NOT REA/MCP, of bzp MoveControl28–163, bzo LookControl20–117 including lambdas, bzn JumpControl9–20, bzk BodyRotationControl16–75; ayo Mth clamp/rotateIfNecessary/sqrt/atan2 and official mappings.
Cached disassembly only outside repository: oracle/{move-control,look-control,jump-control,body-rotation-control,ayo}.javap and server-1.21.txt.
Existing source exact sine table reused through geometry::vanilla_sin/cos. Existing mobs::bat pub(super) Mth atan2/wrap/java_floor reused directly; NO Bat/math visibility edit or duplicate atan2 implementation.

## Actual source facts / implementation

Ordinary Mob constructor creates these four real controls in source order alongside actual selectors/sensing/GroundPathNavigation. Bat does not override them. Source native state has ONLY the original per-control fields. Actor reference is the permanently actor-bound host/lease contract, with no second body/health/RNG/NoAI/scheduler.

MoveControlState: wantedXYZ/speedModifier f64=0, strafeForwards/right f32=0, operation WAIT. All constructor/getters/setWantedPosition/strafe and four tick branches implemented. WAIT calls actual setZza(0), which matters after Bat customAI sets forward. STRAFE preserves f32 temporaries and two separate yaw calls, actual retained nav/evaluator/path type query, then actual current stored strafe inputs after callback; source sets operation WAIT last. MOVE_TO writes WAIT before actor queries, X/Z/Y deltas; source strict distance threshold; exact Mth yaw/rotlerp; modifier read before actual attribute callback; actual retained at/state/shape locals with fresh Level getter twice; actual max-step/width/shape/tag predicates and captured JumpControl callback before setting JUMPING. JUMPING uses modifier-before-attribute and actual onGround. setWantedPosition updates target/speed but preserves JUMPING. Null nav/evaluator are admitted ONLY when actual source getter is null, never an empty-navigation substitute.

LookControlState: original rotation rate floats0, cooldown int0, wantedXYZ doubles0. Ordinary base setLookAt(Vec3/coords/entity/default or explicit limits), getters and tick implemented. Nonliving target bbox getter occurs twice, minY retained across second getter. Default base resetXRotOnTick really sets pitch0; positive cooldown decrements before target computation; Optional yaw/pitch source locals preserved, fresh wanted field reads/rates around actual virtual eyeY/pitch callbacks; expired cooldown rotates head toward body10. Actual navigation.isDone always queried, active path clamps against real max head yaw. Mth.rotateIfNecessary remains target-relative and differs from rotateTowards. Derived protected overrides are NOT represented by this base driver.

JumpControlState: actual requestfalse field; jump sets true. Tick captures request, calls actual setJumping, clears request AFTER callback. A callback requesting again is cleared by successful outer tick; failure retains the request. No state borrow held during callback.

BodyRotationState: headStableTime0 and lastStableYHeadRot0. client_tick is source name, also required SERVER-side via Mob.tickHeadTurn. Actual xo/zo (Entity.L/N), not xOld/zOld. Moving sets body to actor yaw, clamps head, then captures fresh head and clears time. Stationary real getFirstPassenger instanceof Mob gate; strict raw delta15; wrapping i32 counter; narrowing begins only >10; target-relative source rotation and captured operands around maxHeadYaw callback. Java Math.min/max preserve NaN and signed zero; no Rust NaN-suppressing min/max substitution.

Entity.getX/getY/getZ, getBoundingBox/getBbWidth/getBlockY are FINAL in actual Java. API docs require callback-free canonical reads for these accessors. EyeY/yaw/pitch/setters, attribute, navigation/world methods retain actual source virtual callback semantics.

## Callable API / real owner contract

controls::ControlHost has Error and permanent one-actor lifetime contract. PositionHost/OrientationHost access actual source actor fields/methods; direct head/body field access is callback-free.
- move_control::{MoveControlState,Operation,MoveControlHost}; tick(&H::MoveLease,&mut H), set_wanted_position, strafe, rotlerp.
- look_control::{LookControlState,LookControlHost}; tick(&H::LookLease,&mut H), set_look_at, set_look_at_with_limits, entity overloads, rotate_towards.
- jump_control::{JumpControlState,JumpControlHost}; jump/tick(&H::JumpLease,&mut H).
- body_rotation::{BodyRotationState,BodyRotationHost}; client_tick(&H::BodyLease,&mut H).
Each host supplies a separate retained lease to original canonical control+actor lifetime. with_*_state runs short callback-free field closures, releases ALL actor/state borrows before returning, never moves state out/substitutes snapshot/current-ID lookup. Removal/control replacement cannot redirect a retained operation to another actor/control. Actual reference-like nav/evaluator/JumpControl/Level/state/shape/target/bbox handles similarly retain their source identity.
Mandatory owner methods have NO successful fake defaults. Test hosts are explicitly controlled callbacks, not implementation owners.

Actual constructor must create/bind these states to the original Mob fields; controls are transient source state (no new NBT schema supplied). Existing Mob.serverAiStep must call actual move→look→jump every permitted serverAI step after custom AI/navigation; original LivingEntity gating/NoAI is outside these controls. Mob.tickHeadTurn must call body client_tick at the original inherited phase. No new cadence/scheduler/gating created.

## Focused native validation

Rustc1.99.1 actual-file harness controls-harness.rs, edition2021 --test -Dwarnings. Imports actual current worktree Bat/spawning source and unchanged existing mc121_rust rlib for geometry/entity_move/random, not a replacement source math implementation. Exact test binary controls-test, compile/test logs controls-native-{compile,test}.log. Final compile exits0, focused runtime exits0 after rustfmt; no full Cargo/Main check by this subagent.
Meaningful cases: default WAIT and close MOVE_TO mutation prefix; real STRAFE nav/evaluator calls and current fields after setSpeed reentry; MOVE_TO retained local/call order, actual jump callback and operation prefix/failure; JUMPING preservation; attribute callback modifier load; NaN width; Look default pitch/head/navigation; same canonical RefCell control nested tick without held borrow, cooldown/failure prefixes; eyeY source-valid wantedZ reread; actual entity overload two bbox reads; body strict delay/counterwrap/xo and same-object max-head callback reentry; Jump same-object request reentry/error; signedzero/NaN math and distinct rotation forms. Independent Java eye-target-reread pitch bits0xc20d0ec8 asserted natively.

## Original Java runtime probe — controlled, NOT production

oracle/ControlOrderProbe.java loaded original bzp/bzo/bzn/bzk/ayo unchanged from pinned original jar. ASM only generated Mob/navigation/evaluator CALLBACK subclasses. Unsafe allocation bypassed actor/world constructors. Actual original Entity position/xo and Living head/body fields used. Actual original Mth computes numeric outputs. OpenJDK21.0.12.1 Debian, jdk.compiler javac with ASM export, -proc:none; java with same export. Scratch-only class output oracle/control-probe-classes; final compile/run exit0, oracle/control-order-{compile,probe}.log.
Observed and asserted: WAIT forward+0 only; STRAFE attr→two yaw→actual controlled nav/evaluator→path→speed0x3e333333→raw2/3 stored input, blocked1/0 fallback; close MOVE_TO WAIT earlyreturn; JUMPING modifier-before-attribute reentry and ground WAIT; default Look pitch0/head10/nav query; active-path target-relative clamp25; nested same Look cooldown0/head−60/pitch−40; failed eye cooldown1/head−30/pitch0 prefix; eye callback changes target then source wantedZ reread gives pitch0xc20d0ec8/cooldown2; Jump clears reentrant request AFTER successful setter and retains on failure; body strict delay11 max67.5, wrapping i32, xo notxOld; rotlerp wrap360/NaN/signedzero samples.
The full MOVE_TO block/state/collision/tag branch is bytecode-reviewed and controlled native-host tested, NOT exercised against a Java world in this probe. No production actor, actual nav/pathfinder construction, inherited living tick/world movement, network tracking/persistence/Main or complete Bat/survival delivery is proved.
An initial probe assertion incorrectly expected the enum's toString to be its obfuscated field name; source runtime actually prints WAIT. Assertion corrected to canonical enum reference identity and full final probe rerun passed. No native implementation mismatch found.

## Limits / next contracts

Root independent composition/compiler/Main work remains necessary. Real GoalSelector/sensing/navigation/NodeEvaluator/PathFinder owners, generic LivingEntity baseTick/tick/aiStep/travel, actor factory/admission/index/metadata/tracking/persistence/Main phases remain separate. Ordinary control support is NOT all mobs, navigation or a live Bat. No checklist/status metric changed, no completion percentage/test count used as behavioral progress.

## Exact SHA256 (final source + evidence)

- `/workspace/cinderstone-mob-owners-es-20261101-1425-a7c3/src/mobs/controls.rs`: `dcee2390f2069323251232862776337319e9bc1b299da5eff53d1f32c235affa`
- `/workspace/cinderstone-mob-owners-es-20261101-1425-a7c3/src/mobs/controls/body_rotation.rs`: `532bd5e04c0dee7ab402a5ebb439d5914dc804a2d38f3950a3fce3ca37108a3f`
- `/workspace/cinderstone-mob-owners-es-20261101-1425-a7c3/src/mobs/controls/jump_control.rs`: `86c9115834190ab3be2b12f4aee30c1898d8ef626a605e4d114ecb760e2a1533`
- `/workspace/cinderstone-mob-owners-es-20261101-1425-a7c3/src/mobs/controls/look_control.rs`: `d87557b46151c7a96e53f6915c5ee4487a6e95235a0276c04e7793f6f78d131e`
- `/workspace/cinderstone-mob-owners-es-20261101-1425-a7c3/src/mobs/controls/move_control.rs`: `6ca5d4c0f848df65d590c0133fafa3c062fd74e936a4941acd93581b7e590bad`
- `/workspace/scratch/es-20261101-1425-a7c3/controls-harness.rs`: `13349fe7b3c654e2d514c5f123781d148a8100d37c1c7f5673d33f66cbf93eef`
- `/workspace/scratch/es-20261101-1425-a7c3/controls-native-compile.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
- `/workspace/scratch/es-20261101-1425-a7c3/controls-native-test.log`: `f8c12b3386dc32bd22947a3462bf1202316ee2917c0b25b0a613ee4c03380a68`
- `/workspace/scratch/es-20261101-1425-a7c3/oracle/ControlOrderProbe.java`: `cfb30c059210515ff99f9d281133734e771288fe17cf04b0ca2faaf3cf7931d9`
- `/workspace/scratch/es-20261101-1425-a7c3/oracle/control-order-probe.log`: `aba796007744da436065ca272833398447656f740d6f34631036bf47a15132a2`
- `/workspace/scratch/es-20261101-1425-a7c3/oracle/server-inner-1.21.jar`: `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
- `original-jar:bzk.class`: `04a0ac94475e1c889227683dee8ec1598d9c15ae004dedb95ad3c62955cc4db9`
- `original-jar:bzn.class`: `52741113b4dce92ed1d1d909fbfd58a6882232e1c8e8b9c016c388f10f3cfede`
- `original-jar:bzo.class`: `f76dc897e8b8ae331ca041a9397318595ebbd1e0e3f306dee5e24917775eca4b`
- `original-jar:bzp.class`: `501ac34d6e3f3d2e1d9246b952a0b947ff380e882d65c2239bd579c9975c77e5`
- `original-jar:ayo.class`: `f9417695604144fcb88ae34ce68c47743f169cbbd72a3fccad94cdb5ea305ab1`
