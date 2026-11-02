# Native Mob state and constructor attachments

Issue: <https://github.com/Ferigone/Cinderstone/issues/4>. Instance:
`es-20261101-1425-a7c3`; resumed base
`e1471304cbd3e2cb4a30d0b522682084fded0d80`. This stage owns only
`src/mobs/state.rs`, `src/mobs/construction.rs`, and this reference note.
Namespace exports, concrete actor assembly and central admission remain other
explicit owners. This source fragment alone does not complete entities or
survival/combat and does not establish a production Main callsite.

## Original source evidence

Evidence is direct `javap` inspection of the unchanged original Minecraft Java
1.21 server, protocol767, using its official mappings. REA/MCP was not exposed
and no REA run is claimed. Pinned outer server SHA1:
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; mappings SHA1:
`31c77994d96f05ba25a870ada70f47f315330437`; inner server SHA256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Raw JARs and disassembly remain outside the repository.

Mapped `Mob` is `btp`. Its constructor128–166 invokes complete Living first.
Offsets6–68 then assign hand NonNullList2 using canonical ItemStack.EMPTY,
float[2] zeros, armor NonNullList4 using the same EMPTY object, float[4] zeros,
body EMPTY, an empty PathType EnumMap, canonical BlockPos.ZERO, and radius−1.
The three EMPTY reads are separate source field reads; no ItemStack is copied
or newly constructed here. Living's last-equipment histories are separate
objects and must not substitute for these current-equipment lists.

Constructor150–157, offsets71–163, constructs and assigns goalSelector from
the first `Level.getProfilerSupplier()` result, targetSelector from a second
getter call, LookControl, MoveControl, JumpControl, virtual createBodyControl,
virtual createNavigation, then Sensing. Each control/Sensing refers to the same
actual Mob. Body and navigation fields are assigned only after their virtual
factory calls return. A navigation factory can observe already assigned
selectors/controls, null Mob.navigation/Sensing, and zero equipment chances.

Only afterward,159–161, offsets166–187, the constructor fills armor chances
with0.085F, then hand chances, then body chance. Finally163–164 checks the
retained constructor Level's client field and invokes virtual registerGoals
on the server. The earlier supplier getters already dereference Level; a null
Level is not a supported constructor path. Mob169 registerGoals is literally
empty, inherited by AmbientCreature/Bat. It does not authorize replacing actual
selectors with dummy functions. Bat uses ordinary GroundPathNavigation and
ordinary controls, rather than FlyingMoveControl or FlyingMob travel.

Other mapped Mob fields retain their JVM allocation defaults: ambientSoundTime
and xpReward0; target/lootTable/leashData null; lootTableSeed0; canPickUpLoot and
persistenceRequired false. RestrictRadius is JVM0 before its initializer sets−1.
No synthetic dead/alive field or duplicate health/RNG/identity owner belongs here.
Mob metadata15 is BYTE flags, Bat16 is BYTE flags, and Living health is metadata9;
these fields remain exclusively the real SynchedEntityData owner.

Mapped `PathType` is `epv`, source3–39. Its26 exact enum constants use source
ordinal0–25. Source default maluses are−1 for BLOCKED, POWDER_SNOW, FENCE, LAVA,
UNPASSABLE_RAIL, DAMAGE_OTHER, DOOR_WOOD_CLOSED, DOOR_IRON_CLOSED and LEAVES;
8 for WATER, WATER_BORDER, DANGER_FIRE, DANGER_OTHER and STICKY_HONEY;16 for
DAMAGE_FIRE;4 for BREACH;0 for the remaining constants. The actual Mob EnumMap
starts empty. Its get/put observations need no randomized hash or registry.
Mob.getPathfindingMalus198–205 selects a qualifying vehicle/Mob first, then
checks that selected object's map and falls back to PathType.getMalus.
The provided map getter implements only that final selected-map portion.

Mob.getItemBySlot966–969 maps MAINHAND/OFFHAND to hand indexes0/1, armor
FEET/LEGS/CHEST/HEAD to0/1/2/3, and BODY to its scalar reference. Primitive
replacement is only the actual store mutation; existing
`equipment_drops::set_item_slot` still must verify first, replace, then run real
onEquipItem callbacks. Source death drops, XP, loot override, persistence,
targeting and despawn callers must bind to these same stores and actor lifetime.

## Callable implementation and ownership

`MobState<O: MobObjects>` adds complementary actual Mob fields to the existing
Living/Entity owners. `jvm_zeroed()` leaves every source object reference null.
`MobItemList` owns a fixed current-equipment list and its retained canonical
EMPTY default; clear aliases that same object into every slot. Equipment
primitive reads/replacements retain ItemStack identity. `PathfindingMalus`
owns the closed26-key map and preserves arbitrary raw f32 override bits.

`construct_mob_after_living(actor, retained_level, host)` runs only after the
complete real Living constructor. It constructs actual GoalSelectorState,
ordinary control fields, and SensingState; mandatory allocation callbacks place
those objects in the embedding canonical object graph. Factories and field
attachments run in source order with short callback-free borrows. Completed
assignments survive any later error. Missing chance arrays are explicit typed
failures, never silently rebuilt defaults. This is not an actor allocator,
superclass success callback, scheduler, metadata substitute or world admission.

`MobConstructionHost` binds real EMPTY/ZERO objects, Level/profiler suppliers,
native object allocators, virtual body/navigation dispatch, side lookup and
registerGoals. `base_body_control` supplies the literal original base control
object. For Bat, virtual navigation must call the existing real
`navigation::construct_ground`; source FOLLOW_RANGE/evaluator/pathfinder owners
remain mandatory. Every object lease escaping to game code must root its
original actor graph through removal or ID reuse. Private internal self-reference
encoding must avoid reference-counting cycles without exposing weak-only leases.

The subsequent Bat constructor still must attach its fly and rest AnimationState
objects before the server setResting(true) metadata callback. That narrow Bat
initializer/concrete assembly is owned separately. Main entity registration,
world callbacks, packet ordering, loot/XP, full persistence, damage/death/respawn,
nonempty AI and active pathfinding are not supplied by these two modules.

## Focused validation

Rust1.99.1 `rustfmt` was applied only to these owned source files. A standalone
`rustc --test` harness compiled both new modules with the actual existing
GoalSelector, control, Sensing, Bat, RNG and geometry modules. Its shell provides
only extracted LivingObjects/EquipmentSlot declarations and a minimal
MovementEmission type, so this is a focused compile rather than a whole-crate
integration result. The actual whole-crate compiler gate belongs to the root.

Focused runtime checks passed for constructor callback visibility and same-item
identity; two supplier reads and separate selectors; armor→hand→body chance-fill
order; failures retaining completed assignments; client construction skipping
only registerGoals; and raw NaN malus overrides without populating defaults.
Controlled test factories record required calls and do not prove real Level,
navigation, canonical item services, actor construction or Main production use.
