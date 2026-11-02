# Living aiStep source implementation handoff

Explicit claim: issue4 comment6566386887. Instance es-20261101-1425-a7c3.
Worktree /workspace/cinderstone-living-tick-es-20261101-1425-a7c3, branch
teams/entities-survival/es-20261101-1425-a7c3/living-tick-travel, stacked base
ab410119874a261608f3e06b952af6fcb94db256. Root fetched newer master and owns
composition, exports, docs, Git, PRs and full compiler/runtime gate. This child
edited ONLY new src/living_entities/ai_step.rs. Other dirty files belong to
their explicitly assigned owners. No Main/shared/manifest/state edits made.

## Source and API

Evidence: direct JDK21 javap of unchanged original Java1.21 protocol767 inner
server SHA2568d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01,
official mappings SHA1 31c77994d96f05ba25a870ada70f47f315330437. No REA/MCP run.
Original private disassembly input oracle/living-lifecycle.javap (btn) plus
original Entity/Mob virtual dispatch evidence. No binaries/raw disassembly in Git.

Callable exports:
- ai_step<LivingAiStepHost>(actor,host): btn.m_2736–2859.
- travel_ridden<LivingRiddenHost>(actor,player,input:geometry::Vec3,host):
  private btn.c(Player,Vec3)2356–2369.
- update_fall_flying<LivingFallFlyingHost>(actor,host): private btn.I2869–2894.
- jump_from_ground<LivingJumpHost>(actor,host): default btn.ff2143–2160.
- go_down_in_water(actor,host): default btn.fg2163–2164.
- jump_in_liquid(actor,AiFluid,host): default btn.c(TagKey)2167–2168.
Selectors AiIntField/AiDoubleField/AiFloatField refer to actual existing fields;
AiFluid/AiEffect/AiDamage select actual canonical tag/holder/source operations.
AiProfileEvent preserves fresh profiler push/pop source calls.

All source callbacks are mandatory and fallible. Associated Actor/Level/
Profiler/BoundingBox/source/stack handles retain actual identity and original
lifetime across removal, callback mutation and reentry. No mutable borrow spans
a host callback. No new actor, timer, physical state, health, dead flag, effects,
equipment, RNG, world, AI scheduler or persistence owner is created.

Exact phase/reread ledger remains nov2-living-ai-step-preparation.md. Implemented
details include current controlled codec-base sync; postinterpolation counter
rereads; distinct vector versus component velocity virtual overloads; saved
velocity and literal positive-zero threshold writes; immobile Yya retention;
fresh Level/profiler for every profiling operation; repeated water/lava/onGround
jump predicates; exact float input damping; pretravel bbox/input locals before
effect/controller callbacks; separate ridden/ordinary travel dispatch; frozen
int wrapping and independent late freeze damage without a broad dead guard;
frost callbacks outside the freezing gate; autospin decrement before callback;
source old bbox retained through travel and freezing; unconditional push phase
and late sensitive-water hurt. Errors do not invent profiler pops or rollback.

No direct NoAI guard is added. Actual virtual Entity.isEffectiveAi and Mob's
parent-then-NoAI override remain authoritative. Source ServerAiStep, travel,
player controller, effects and damage may not be successful unavailable stubs.

The private ridden body retains adjusted input across tickRidden, then reads
current control. Controlled: getRiddenSpeed/setSpeed/virtual travel. Remote:
calculateEntityAnimation(false), setDeltaMovement(Vec3.ZERO), tryCheckInsideBlocks.
getRiddenInput/tickRidden/getRiddenSpeed remain mandatory actual virtual owners.

Ground jump uses actual source `power <= 1e-5f` return guard: NaN CONTINUES
(fcmpg/ifgt), writes NaN Y and then actual sprint/impulse work. Independent review
caught and corrected the initial negated-positive implementation before handoff.
Exact existing geometry::vanilla_sin/cos use the pinned original Mth float table;
f32 yaw*0.017453292 and fneg occur before float-to-double widening and double*0.2.
Source float jump power is widened exactly; default liquid add preserves all
three source double additions and ignores its tag only in that default body.

Fall flying retains actual CHEST stack and prospective wrapping fallFlyTicks+1
local, without incrementing the canonical counter. Real ELYTRA check,
isFlyEnabled, hurtAndBreak1/CHEST with actual equipment/enchantment/RNG owner,
ELYTRA_GLIDE game event and fresh final logical-side/metadata flag remain real
callbacks. Replacing equipment during durability cannot redirect retained stack.

## Validation and review

No Cargo command was run by this child. Standalone production -Dwarnings lib
compile and focused --test -Dwarnings compile/run pass. Harness imports actual
repository mc121_rust::geometry (not a test substitute) from the built library;
new ai_step.rs compiles as the actual source path. Scratch harness and executable:
living-ai-step-harness.rs and living-ai-step-tests. Rust1.99.1, unchanged manifests.

Controlled Rc<RefCell> hosts assert no actor borrow survives each callback. They
exercise codec sync, fresh profiler after world change, immutable bbox/input
locals across effect mutation and lethal/removed travel, damage/push continuation
after death, interpolation counter callback mutation, source NaN/zero velocity,
immobile Yya retention, repeated fluid/onGround predicates, jump callback partial
failure and source delay overwrite, frozen overflow, late-error prefix without
synthetic cleanup, recursive same-actor aiStep, ridden adjusted-input and control
reread, source float-table jump contribution and NaN jump continuation, exact
liquid additions, retained elytra stack after equipment/world replacement,
source flycounter local, ten/twenty-tick event/durability phases and late errors.
These are native controlled-host checks, not an original aiStep actor runtime,
world integration, full conformance or production/Main proof.

Independent living_lifecycle review compared every source body/helper against
original bytecode and approved after the NaN guard correction, with no further
material mismatch. Root still owns namespace/current-master build composition.

Actual admission/canonical object arena, existing fields, codec, profiler,
equipment/effect/attribute/enchantment definitions, jump-power producer, ordinary
travel body, riding overrides, world block/push/auto-spin queries, damage/death,
RNG/packet audiences and real tick phases remain mandatory production owners.
Source defaults do not silently replace subclass bodies. No whole survival,
entity, mob or checklist delivery is completed by this bounded component.

The independent original probe and native correction for the NaN jump guard are also recorded in entities-survival-living-tick.md.
