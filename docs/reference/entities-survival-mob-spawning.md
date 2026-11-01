# Native base Mob spawn and despawn phases

Instance: `es-20261101-1425-a7c3`; issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Implementation base: `c1baf5c70f07e53b6b76edaf84be5da57f6bd40b`.
Exact new-module claim: [issue comment 6563958691](https://github.com/Ferigone/Cinderstone/issues/4#issuecomment-6563958691).
Implementation: `src/mobs/spawning.rs`.

Target: original Minecraft Java 1.21, protocol 767. Server SHA1:
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1:
`31c77994d96f05ba25a870ada70f47f315330437`; inner server SHA256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence is direct JDK21 javap and the controlled original runtime observation
below. No REA execution is claimed. Raw disassembly, Java binaries and probe
classes remain outside Git.

## Source and callable behavior

Base Mob.finalizeSpawn (`btp`, 1207–1215) obtains the supplied
ServerLevelAccessor.getRandom before FOLLOW_RANGE and requires that actual
attribute instance. If minecraft:random_spawn_bonus is absent, it adds a
permanent ADD_MULTIPLIED_BASE modifier using RandomSource.triangle with mean 0
and deviation 0.11485000000000001. It then always consumes nextFloat < 0.05 and
calls setLeftHanded. Difficulty and reason are unused; the same incoming group
identity/nullability is returned. The native runner inlines original
RandomSource (`ayw`, 75) triangle order:
mean + deviation * (first nextDouble - second nextDouble). It neither uses
Gaussian draws nor substitutes the actor RNG for the supplied Level RNG.

Static Mob.checkMobSpawnRules (892–893) computes below first, including Java
coordinate wrapping. Exact SPAWNER skips block lookup; TRIAL_SPAWNER does not.
Otherwise it reads the actual block below and calls its
isValidSpawn(level, below, type). Its random argument is unused. Base instance
checkSpawnRules (897) genuinely returns true; actual subtype dispatch and
SpawnPlacements are not bypassed by that base result.

Mob.checkSpawnObstruction (901) reads actual bounds and evaluates
!containsAnyLiquid(bounds) && isUnobstructed(actual actor), retaining the
liquid-query short circuit.

Mob.checkDespawn (747–775) preserves this order:

1. Peaceful difficulty and virtual shouldDespawnInPeaceful: discard and return.
2. Persistence-required or custom-persistence: set actual noActionTime to 0 and return.
3. Actual Level.getNearestPlayer(this,-1); no result returns without category/RNG work.
4. One retained player's distanceToSqr, category despawnDistance and wrapping Java
   int square. Far distance plus actual virtual removeWhenFarAway invokes discard.
5. That first discard does **not** return. Read category noDespawnDistance and its
   wrapping square; noActionTime > 600 then actor nextInt(800)==0 then far distance
   then virtual removeWhenFarAway can invoke discard again.
6. Otherwise strict distance<nearSquare resets noActionTime to 0. Equality does not.

The random draw precedes the near-distance condition, including for nearby
actors. No clock increment, scheduler or loot/XP is attached to this method;
discard must use the actual actor/removal/world owners.

Callable APIs are finalize_spawn, check_mob_spawn_rules, base_check_spawn_rules,
check_spawn_obstruction and check_despawn. Their mandatory host traits retain
actual RNG, attribute, type/reason, block/bounds and nearest-player handles, plus
actual state and removal callbacks. None has an unavailable-owner success default.
Earlier mutations/draws remain consumed after later errors. No actor, random
stream, queue, attribute definition or persistence owner is manufactured.

## Focused native validation

Standalone Rust 1.99.1 library compilation and formatting passed. Focused callback
flows passed using the actual public crate::random::LegacyRandomSource. An
external rustc harness supplies that module and the existing md5 dependency;
the repository tests do not recompile a nested copy of random.rs.

The flows cover exact Level RNG state/modifier bits/group identity, existing
modifier draw skipping, errors after consumed draws, exact SPAWNER-vs-TRIAL
branch/world-query order, liquid short circuit, separate Level/entity streams,
continued work and repeated discard after the first discard, nearby pre-distance
RNG consumption, retained removal after later failure, early exits, strict
thresholds and Java integer-square overflow. These are controlled native host
flows, not a production mob actor or world observation. Final actual-crate/Main
compiler checks and exact source commit belong to the coordinator's issue/PR.

## Unchanged-original finalize runtime observation

MobFinalizeSpawnProbe compiled and ran under the existing JDK21 with the pinned
jar/libraries, exit 0; successful output was recorded at
2026-11-01 21:22:39 UTC. Probe-source SHA256:
`fca61f2009a553cbf0e3d07a19aa8b6d12fd94e683f839aaa6740cb58787e244`.
Output SHA256:
`1b11012f78703b7d41e1e3eb4514c400b843a7e07148b3904a6478a9701325e6`.

No original class was transformed. A generated subclass supplies getAttribute
returning a real original AttributeInstance, and records setLeftHanded before
calling the original setter on real original SynchedEntityData. A LevelAccessor
Proxy supplies only getRandom, returning a retained original LegacyRandomSource.
Unsafe deliberately skips the actor constructor. Setup defines original Entity
base metadata defaults through the real Builder, then invokes original
Mob.defineSynchedData and Builder.build. The group is a controlled interface
object, retained by identity; null difficulty/reason corroborate unused base
parameters. Actual finalizeSpawn, attribute-map/dirty behavior, RNG and flag
setter bodies remain unchanged.

| Controlled case | Observed result |
| --- | --- |
| Seed 38, absent bonus | Modifier raw bits 3fb540f36eccd4e1; Level RNG state 7748939612830; entity RNG unchanged at 25214903690; left-handed true; metadata dirty; one permanent modifier. |
| Existing same bonus, reset Level seed 38 | Same modifier object retained; only nextFloat consumed; Level state 204891832807418; left-handed false; null group returned unchanged. |
| Actual AttributeInstance dirty Consumer throws | byId/operation insertion retained, permanent map empty; two nextDouble draws retained at Level state 237441308552447; no later handedness setter. |
| Missing FOLLOW_RANGE | getRandom precedes requireNonNull failure; no draws; Level state 25214903883 unchanged. |

The absent-bonus callback trace was getRandom -> getFollowRange ->
onDirty(byId 1, permanent 0) -> setLeftHanded(true). A separately retained original
comparison RNG ran original triangle then nextFloat and matched exact bits/state.
The original permanent map held the same modifier object and canonical bonus ID.
A nonnull incoming group was returned as the same object.

The first controlled setup attempt failed before finalizeSpawn because its
metadata Builder omitted Entity-constructor base entries. That setup error was
corrected and is not a gameplay observation. The successful probe verifies only
the described controlled method: no actual actor construction, complete actor
AttributeMap, world query, tick, registration, tracking, removal, loot/XP,
persistence or production gameplay was observed.

## Integration boundary

These are base-method executors. EntityType construction and its real subtype
finalize chain, spawn egg/command dispatch, natural spawning/caps, ordinary actor
state, AI/navigation, world add, tracking/packets, full damage/death/loot/XP,
complete save/restore and production Main calls remain pending central contracts.
Actual ID/UUID/generation, constructor and retained RNG state, source call phases,
EntityTickList registration/removal and packet order remain central ownership.
The separate loot team owns src/loot/**, untouched here.

Additional Bat virtual bodies and its light/date spawn predicate are documented
in [the AI/Bat ledger](entities-survival-mob-ai-step.md). Implemented predicate or
BatFlags NBT bodies do not establish spawning or persistence integration. The
retained Bat driver is source-reviewed and supports normal same-object virtual
reentry; neither it nor an isolated callback
host is recorded here as a complete actor. No NoAI or creative-only substitute,
whole mobs/entity delivery, checklist change or functional percentage is claimed.
