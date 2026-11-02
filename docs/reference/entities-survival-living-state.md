Living state owner — source/API/validation ledger, 2026-11-02
Instance es-20261101-1425-a7c3. Pre-edit issue4 comment6566884842.
Branch teams/entities-survival/es-20261101-1425-a7c3/living-state-owner.
Exact base96b4bad622155e2891d9d8a249f8e233721d34c8; fetched master f191.
Only repository write: src/living_entities/state.rs in living-state worktree.
No Git, shared export, manifest, Main, actor admission, docs or status edits.

SOURCE
Pinned original Java1.21 server protocol767, inner SHA256
8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
Official mappings server-1.21.txt: LivingEntity -> btn; WalkAnimationState -> bup.
Direct JDK21 javap (not REA/MCP): living-lifecycle.javap, removal-entity.javap,
living-walk-animation-state.javap. Raw disassembly remains scratch only.
btn constructor mapped187–284, instructions0–233: actual Entity ctor first,
then Living field initializers187–267, then constructor body272–284.
bup constructor5; setSpeed11–12; update15–18; speed21/25;
position29/33; isMoving37. Source f32 operations have no clamp or FMA.

EXCLUSIVE STATE OWNERSHIP
LivingEntityState<LivingObjects> is complementary state for a Bat lifetime.
It owns actual source btn.be dead, generic btn.ci absorptionAmount, nullable
reference fields and all remaining Living scalar fields. It does not derive an
alive/dead predicate from health or keep an alive flag. Generic absorption is
not Player metadata15 (Player override); Mob metadata15 is a different BYTE.
PhysicalState remains sole ID/position/velocity/bounds/tickCount/invulnerableTime/
oldYaw/oldPitch/fire/fluid/air/removal owner. Health remains actual metadata9.
Entity RNG is not created, copied or cached. No scheduler, level or entity registry.
Do NOT attach this complete Bat complement beside existing PlayerBodyState or
PlayerLifecycle. Player requires central projections/migration of existing fields.
Source invulnerableDuration(aV) final20 is distinct from physical invulnerableTime.
Source last-hand/armor stacks bX/bY are history, not Mob's current equipment.

PUBLIC API
LivingObjects binds canonical Actor, ItemStack, BlockPos, DamageSource,
FallLocation, attribute holder/instance/supplier and real effect-map/Brain/
location-enchantment objects. Actor/ItemStack Clone aliases same actual object
without callbacks; JavaObjectKey retains actual source equality/hash/identity.
LivingEntityState::jvm_zeroed() gives Java scalar +0/false and genuine reference
nulls, including attributes/Brain/walkAnimation/EMPTY refs. It must attach to the
same actor BEFORE Entity's constructor performs dynamic metadata callbacks.
No default constructor or completed-actor marker is supplied.
LivingCombatTracker stores actual actor reference and reused native CombatTracker.
Attributes reuse actual native AttributeMapState with retained immutable supplier;
no eagerly instantiated attributes, template-copy snapshot or second value cache.
EquipmentHistory owns fixed source NonNullList entries and same canonical EMPTY
default; set returns prior object and clear restores default references.
WalkAnimationState owns only bup's actual speedOld/speed/position fields.
AI/base field projection methods map source TickCount/InvulnerableTime and old
angles back into same PhysicalState. LivingTickProjection implements existing
LivingTickFields over short mutable references to actual Physical+Living state.
SourcePositionHistory requires borrowed actual Entity.xo/zo. IMPORTANT correction:
physical.old_position is xOld, NOT xo. Actual central f191 PlayerMotion uses its
previous_position as xo/yo/zo. Bat's actual history owner remains unclosed and
must cause the host to fail rather than alias tracker/base codec/xOld history.
No projection can span a source callback; reacquisition sees real mutations.

INITIALIZER ORDER AND BOUNDS
initialize_living_fields is ONLY the source post-Entity187–267 fragment:
1 native CombatTracker plus same Actor reference -> bV;
2 actual Maps.newHashMap factory -> bW;
3 canonical EMPTY + fixed NonNullList2 -> bX;
4 separate canonical EMPTY read + fixed NonNullList4 -> bY;
5 separate EMPTY read -> bZ, discardFriction=false, new WalkAnimationState,
  invulnerableDuration20, effectsDirty=true;
6 separate EMPTY read -> useItem, actual Optional.empty lastClimbablePos;
7 actual Reference2ObjectArrayMap factory -> activeLocationEnchantments;
  appliedScale=1. No other zero-initialized fields are reset.
The outer Option<Option<BlockPos>> distinguishes Java null before initializer
from initialized Optional.empty. Source autoSpinAttackItemStack remains null;
it is not silently replaced by ItemStack.EMPTY.

construct_living_after_entity includes that fragment then source272–284:
DefaultAttributes.getSupplier(RETAINED constructor EntityType argument), actual
new native AttributeMap attached before final getMaxHealth (virtual attribute
value dispatch), actual virtual setHealth, actual Entity.blocksBuilding=true,
ONE global Math.random -> rotA using promoted source float0.01 constant,
actual virtual reapplyPosition, second global draw castf32 BEFORE *12398f,
third global draw *promoted source2pi then actual virtual setYRot, fresh source
getYRot -> yHeadRot, empty-memories Dynamic -> actual virtual makeBrain -> bF.
Three draws are deliberately interleaved; no batch array or Entity RNG draws.
with_living_state releases the actual state borrow before every real callback.
Missing Brain/effect-map/enchantment-map/item/metadata/attribute supplier/Entity
field/virtual owner must error. No successful no-op or dummy owner is supplied.
Completed attachments/mutations/draws survive every error. Parent Entity ctor,
metadata, Mob/Bat construction, admission and packet position codec are not run.
Runtime manages actual source object-graph lifetime (including tracker actorref).

VALIDATION
Standalone library and test compilation: modern matching rustc, edition2021,
-Dwarnings. Harness scratch/living-state-harness.rs compiles actual current
state.rs, tracker.rs, attribute_instance.rs, attribute_map.rs, tick.rs, ai_step.rs,
base_tick.rs, Bat math/spawn helpers; unchanged existing physical/geometry via
prebuilt mc121_rust crate and its dependency dir. No shared Cargo target writes.
Focused tests cover allocation nulls before source init, shared EMPTY/tracker/
supplier references, actual field projections including physical timer aliasing,
distinct xo/xOld history, constructor callback reentry and live-ID replacement,
three global draws interleaved with real callbacks, late virtual yaw reread,
failure after tracker / after first draw / after all draws with prefix preserved,
fixed-list clear references, bup f32 rounding/interpolation and NaN threshold.
Initial decimal test expectations were corrected by exact source arithmetic:
rotA draw0.125 ->0x3c3851eb, not decimal0.01125f one ULP higher;
bup set0.7/update(-0.2,0.3) ->speed/position0x3edc28f5, not decimal0.43f;
speed(partial0.4)0x3f178d50; position(partial0.4)0x3e3020c4.
Peer unchanged-original bup runtime probe requested; no result yet.
Controlled native tests are not actual fully constructed Bat/world/Main proof.

REMAINING CENTRAL/ACTOR CONTRACTS
No production host is attached. Actual Entity construction, physical/metadata,
xo/yo/zo and VecDeltaCodec owner, canonical handles/lifetime and Java identity hash,
registered attribute supplier bindings, real active HashMap/Brain/reference map,
Mob/Bat controls/selectors/navigation/Sensing/current equipment, world queries,
Entity RNG/global Math owner and real scheduler/Main phases remain required.
No Player migration, whole entity/survival delivery, production tick/death proof,
spawn/persistence/tracking/loot/XP integration or checklist change is claimed.
