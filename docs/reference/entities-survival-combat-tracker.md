# Native CombatTracker state and callback order

Instance: es-20261101-1425-a7c3; assigned issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Base: `47182b236135a295ab17a2d64aaedd425f960869`.
Owned source: `src/combat/tracker.rs`, its mechanical export in
`src/combat/mod.rs`, this note and the per-instance status.

## Original artifact and source facts

Minecraft Java 1.21, protocol 767; official server and official mappings.
Server SHA1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`;
mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`;
inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Direct JDK21 javap analysis of original CombatTracker (`brh`) and CombatEntry
(`brf`), not an REA/MCP run. CombatRules is `brg`, a distinct class.

| Original method | Source lines | Native API |
| --- | --- | --- |
| CombatTracker constructor / fields | 27–37 | `CombatTracker<S,F>::new` |
| recordDamage | 40–55 | `record_damage` |
| shouldEnterCombat | 58 | actual causing-living host query at its callsite |
| getCombatDuration | 163–166 | `get_combat_duration` |
| recheckStatus | 171–184 | `recheck_status` |
| CombatEntry constructor and getters | original `brf`, line5 | `CombatEntry<S,F>` and getters |

The tracker owns its original entries, timestamps, inCombat and takingDamage.
Those flags describe combat history; they do not establish actor aliveness.
Actual LivingEntity.isAlive, physical tickCount, fall location/distance and
canonical DamageSource stay with their actual owners. `S: Clone` must retain
source object identity, direct/causing entities, holder and source position.
It must not construct a source from a damage registry ID. `F` is the actual
FallLocation value, including a normal absent value.

## Implemented lifecycle

recordDamage first rechecks status, then resolves current fall location and
fall distance, appends the immutable entry, reads actor tickCount into
lastDamageTime and sets takingDamage. If not already in combat, the real actor
must still be alive and the source's causing entity must be a LivingEntity.
Only then set inCombat, reread tickCount for combatStartTime, copy it to
combatEndTime and invoke actual onEnterCombat. Raw damage, signed zero and NaN
are not normalized by this method, matching its lack of an amount guard.

FallLocation resolution must query the actual last-climbable-position block
first; water is checked only when no last-climbable position is present. The
tracker delegates this actual world/entity operation, with no invented ladder
or water classification.

recheckStatus queries the actor only while takingDamage. Death short-circuits
the timeout tick read. Otherwise expiration uses strict `>` and wrapped Java
int subtraction, with 100 ticks outside combat and 300 in combat. Expiration
clears both flags, rereads tickCount into combatEndTime, invokes onLeaveCombat
only if previously in combat, then clears entries. The callback sees the new
flags/end timestamp and the old entries. Callback reentry can record new hits;
the outer final clear also clears those entries without undoing reentrant
flags/timestamps. Errors preserve completed state and callback prefixes.

getCombatDuration never rechecks status. It reads the actor tick only while
in combat; otherwise it returns end minus start. Both subtract with Java int
wrap. Querying this getter does not expire combat.

## Owner and Main integration contract

Attach exactly one tracker to the real living actor identity/generation; it
is transient and not an independent persisted actor or scheduler. Implement
all `CombatTrackerHost<S,F>` methods synchronously on the actual associated
actor/world, including onEnterCombat/onLeaveCombat and their packet/other
callbacks. The supplied callback receives the same mutable tracker to support
source reentry; central borrowing must route that exact object rather than
allocate a second tracker or reject ordinary reentry.

The existing Player actuallyHurt executor calls recordDamage after exhaustion
and before its subsequent health read/subtraction. Generic LivingEntity and
ServerPlayer death paths recheck at their distinct original positions; neither
can be replaced by this tracker. The central damage host must compose this
owner at those sites under the shared outer operation guard. Main, native
lethal/death/respawn, menus, inventory/loot/XP, save/load and packet publication
remain pending real-owner contracts. This source component has no Main actor
invocation. [Death/respawn facts](entities-survival-death-respawn.md) and
[effect ownership](entities-survival-effects-integration.md) describe the
related integration requirements.

Unsupported here: death-message Components, most-significant-fall selection,
kill-credit/death results, world callbacks themselves, mobs and their AI,
production tracking/persistence and full survival. Entry retention is not a
substitute for implementing those operations. No fixed checklist delivery is
changed by this bounded source component.

## Focused validation

Independent original-source review found no material mismatch in scope.
A controlled original-Java probe runs unchanged `brh` and `brf`; generated
surrounding actor/source subclasses supply alive, water/climbable and callback
facts, while actual actor fields supply tick/fall. It establishes source
ordering under those contexts, not production world behavior.

Probe source SHA256:
`0e30026c9014d4f2d0d7aeb6755b7d3d3247f81e76d4e79f8dc387b57b2560f3`.
Observed output SHA256:
`efd843d55f81721ff9e0ca70bf7690e1b3d081e4bcc95f0d5afa0ba2a69a6478`.
The probe and native focused checks cover retained identity/raw values,
fall/water ordering, strict timeout boundaries, absence of getter recheck,
death short circuit, callback-visible entries/flags, failure prefixes, Java
int wrap and synchronous leave/reenter behavior. Binaries, raw disassembly and
controlled probe artifacts stay in external scratch, outside Git.

Full crate/compiler results are recorded in the canonical
[instance status](../coordination/status/es-20261101-1425-a7c3.md).
