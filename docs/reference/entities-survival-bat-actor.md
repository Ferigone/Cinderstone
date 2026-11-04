# Retained ordinary Bat constructor and ownership

Minecraft Java 1.21, protocol767. Source evidence is direct `javap` analysis of
unchanged pinned original classes and official mappings, not an REA/MCP run.
Server SHA1: `450698d1863ab5180c25d7c804ef0fe6369dd1ba`.
Mappings SHA1: `31c77994d96f05ba25a870ada70f47f315330437`.
No Java binaries, archives, raw disassembly or original source are added here.

## Implemented native object

`src/mobs/actor.rs` owns one retained `NativeBatLease<T>` aggregate. Its single
field mutex contains the SAME actual `EntityPhysicalState`, complementary
Entity fields, `LivingEntityState`, `MobState`, synchronized Living/Mob/Bat
metadata, Entity RNG and Bat animation/target fields. Health exists only at
metadata9. Entity tick count, invulnerability, position, velocity, dimensions,
fluid/fire/frozen state and removal remain in the one physical owner. The
complementary Entity fields include source xo/yo/zo (distinct from physical
xOld/yOld/zOld), Entity's packet-position-codec base, ChunkPos and blocksBuilding.
No separate alive/dead approximation, health cache, scheduler or Main actor
factory is created.

Public `LivingObjects::Actor` is a genuine strong `NativeBatLease`, whose Clone
retains the SAME graph. The original tracker's self reference has a separate
storage encoding: a defaulted `LivingTrackerStorage` parameter preserves the
existing public Living state/constructor API, while the native aggregate uses
its private graph backlink. Both constructor entrypoints execute the SAME
factored initializer/constructor source body; no source sequence is copied.
The nominal default `RetainedLivingTracker` avoids recursively expanding
`Objects::Actor` when existing callers use recursive Actor aliases.

Internal controller/Sensing/navigation/tracker/attribute-consumer backlinks
have no strong self-edge. The opaque descriptor cannot be constructed or
resolved by external callers; resolution requires a context already retaining
the exact original graph. Public control, Sensing, navigation, tracker and
metadata leases ALWAYS carry that strong graph root. Controls/Sensing/navigation
also capture the exact component object, rather than looking up its current
field again. The source-final tracker remains inline and cannot be replaced
through this API. Removing an index entry or allocating another actor with the
same numeric ID cannot redirect any of these leases. When all external leases
are dropped, the internal backlinks do not keep the graph alive in an Arc cycle.

This is a bounded self-graph policy, not a general Java garbage collector.
Foreign-actor retention, arbitrary cyclic target/attacker/vehicle graphs and
escaped target AttributeInstances are not exposed by this stage. Future APIs
must retain their actual graph/object receivers. Brain is an actual peractor
inline owner, with no escaped Brain/replacement API yet; exposing those later
requires capturing the exact Brain object as well as the actor root.

## Source construction sequence

Evidence: Entity `bsr`169–315, Living `btn`187–295, Mob `btp`128–177,
AmbientCreature `cey`9–10, Bat `cez`40–64; navigation `cda`59–86 and `ccz`28–30.

The aggregate is attached before Entity's virtual metadata-definition callback,
with JVM-zero/null Living/Mob/Bat fields. Actual Entity construction remains an
explicit embedding precondition. It must obtain the shared atomic entity ID,
actual Entity RNG and UUID draws from that SAME RNG, initial dimensions,
metadata0–16, initial virtual setPos and final eye height in source order on
this SAME object. `EntityPhysicalState::new` alone is not that constructor.
`install_defined_synched_data` is the built metadata assignment point; the
subsequent `finish_entity_constructor` records the explicit owner assertion.
Neither method admits or ticks the actor.

`construct_after_entity` runs the following actual native tail once:

1. Shared Living initializers create the CombatTracker, native ActiveEffectMap,
   canonical EMPTY equipment histories, walk animation, source scalar defaults,
   native Reference2ObjectArrayMap location-enchantment owner, and AttributeMap from the shared registered
   EntityType default supplier. References are assigned at source points.
2. Native AttributeBinding reads MAX_HEALTH through supplier fallback, then the
   real setHealth body captures its data receiver and reads current MAX_HEALTH
   AGAIN before Java float clamp and metadata9 setter. No peractor default
   attributes are eagerly instantiated or placed in dirty sets.
3. blocksBuilding becomes true. Three separate global Math.random calls are
   interleaved with rotA assignment, actual inherited reapplyPosition, timeOffs,
   inherited finite yaw setter, fresh yaw read and head-yaw assignment. Entity
   RNG and Level RNG cannot substitute. A fresh real default Brain provider
   decodes the source empty-memories NbtOps input and returns a new Brain owner.
4. Shared Mob constructor creates its distinct current equipment/chance stores,
   path-malus EnumMap/restriction fields; obtains TWO separate profiler suppliers;
   allocates actual empty GoalSelectors and Look/Move/Jump/Body controls; runs
   real GroundPathNavigation construction against the same actor and retained
   Level argument. FOLLOW_RANGE uses the same inline native AttributeMap;
   WalkNodeEvaluator, canPassDoors and PathFinder are real retained objects.
   Sensing is allocated next. Armor, hand and body drop chances become0.085
   only after navigation/Sensing exist. Server registerGoals is the inherited
   literal empty Mob169 body.
5. AmbientCreature adds no further constructor work. Bat installs BOTH final
   animation fields before invoking the metadata setResting(true) operation on
   servers. The side check uses the RETAINED constructor Level argument, as
   original `cez` offset28 `aload_2` does, even if earlier callbacks change the
   actor's current Level. The frame therefore retains constructor Level
   separately from current runtime Level.

Metadata uses the SAME frame and source write → virtual callback → dirty order.
Server Bat inherits Entity/Living callbacks: pose6 invokes actual dimension
refresh; other supported source server data callbacks have their literal empty
body. No field lock survives that callback. Unsupported Component ownership or
missing real owners return errors with preceding mutation/callback prefixes
preserved. Constructor failures leave the object partial and prevent a retry
from silently recreating source-final objects or repeating RNG draws.

The inherited reapplyPosition uses `living_entities::retained_move`; raw position
updates the real chunk cache only when BlockPos changes, before the onMove
callback. Its bounding-box continuation reads current position/dimensions after
the callback. No field mutex spans onMove or another world operation.

## Exact embedding contracts and unsupported behavior

The public `NativeBatServices<T>` requires actual shared registered attribute
holders/definitions, lazy object-identity authority, static default supplier,
canonical ItemStack.EMPTY and BlockPos.ZERO, actual Enchantment VALUE and
location-effect reference/equality domains, global Math.random, retained-Level profiler/
side queries and world position/dimension/attribute callbacks. The constructor
owns ActiveEffectMap, AttributeMap, native LocationEnchantments map/sets, Brain
and Mob component allocation itself;
there is no opaque successful superclass-constructor callback.

No production implementation of those combined services or Entity constructor
seam is attached by this stage. No Main/index/tracking/persistence/removal/world
admission file is changed. There is deliberately no complete Bat tick entrypoint
or inherited_tick success stub. Full Entity/Living/Mob tick, damage/death/loot/XP,
effects, item/equipment callbacks, active navigation/goal algorithms, sound,
world spawning and persistent Mob saves remain unsupported integration work.
This milestone does not complete entities or survival_combat deliveries.

## Validation

Focused aggregate ownership fixtures cover retained removed actors after numeric
ID reuse, escaped control replacement identity, final-root drop without self
cycles, Send+Sync compatibility, explicit single-use Entity/data preconditions,
and the raw-position chunk-cache branch. These fixtures use uninhabited external
attribute/effect domains and claim no fabricated registry, actual Entity
construction or production runtime admission. Compiler/runtime results are
recorded by the team root after the composed stage is validated.
