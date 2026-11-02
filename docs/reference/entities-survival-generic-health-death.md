# Nov 2 generic living health/death stage

Claim: issue4 comment6566070764. Base master26c4d2ec13b885e51025e105f1edeb0fe78f6309.
Only owned repository files: src/combat/living_health.rs and src/living_entities/death.rs.
No Main, exports, manifest, status/checklist or Git changes by this subagent.

Evidence is direct JDK21 javap on the original Java1.21 protocol767 inner server
JAR, not an REA/MCP run. Original inner SHA256:
8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
Official mapping source server-1.21.txt SHA1:
31c77994d96f05ba25a870ada70f47f315330437.
Raw private source inputs: oracle/living-lifecycle.javap (btn),
oracle/player-damage.javap (cmx); raw output and binaries are outside Git.

## Callable APIs and facts

combat::living_health::actually_hurt<LivingActuallyHurtHost> is the generic
btn.f(brk,float), source1750–1773. It first invokes virtual invulnerability,
armor and magic reductions. Save reduced before absorption; max(reduced-current
absorption,+0), then reread absorption and set absorption-(reduced-healthDamage).
Absorbed strictly >0 and <3.4028235e37f triggers causing entity lookup and actual
ServerPlayer DAMAGE_DEALT_ABSORBED(round(absorbed*10f)). No direct-entity shortcut.
Both signed zeros return before tracker; NaN proceeds. RecordDamage precedes
fresh health read/virtual setHealth; another fresh absorption read/subtract/set
follows health. ENTITY_DAMAGE game event last. Player.actuallyHurt deliberately
has different stats/food and no final absorption subtraction: do not compose it
by selecting this generic method for Player.

heal<LivingHealHost> is btn.c(float), source1113–1117: current health local,
strict positive health gate, virtual setHealth(current+amount). No amount gate,
health mirror or revival/death dispatch is inserted.

kill_credit<LivingKillCreditHost> is btn.eM(), source1781–1787: check actual
lastHurtByPlayer field then reread it for return; else analogous lastHurtByMob;
else null. No timer/source/alive query belongs in this getter. Actual expiration
is baseTick478–491, separately owned.

living_entities::death::living_die<LivingDeathHost> is btn.a(brk),
source1377–1405. Removed short-circuits actual dead field access; either guard
returns. Null source fails at its getEntity dereference AFTER those guards.
Causing entity and getKillCredit results are retained locals. DeathScore is read
for >=0 before killer-null gate and reread for awardKillScore. Sleeping stop,
nonclient named entity death-message/getString/logger execution, actual dead=true,
and tracker recheck precede the captured current ServerLevel read. Causer-null
or actual causingEntity.killedEntity(level,victim)==true permits ENTITY_DIE,
virtual dropAllDeathLoot(capturedLevel,source), virtual createWitherRose(saved
killer). Veto still broadcasts status3 on freshly read CURRENT level. Nonserver
skips entire branch. Actual DYING pose is always the successful final action.

player_die<PlayerDeathHost> is cmx.a(brk), source692–715. Direct invokespecial
parent is the living_die body above, then reapplyPosition. !spectator and actual
current ServerLevel execute virtual death loot again, even when parent already
did it or returned for removed/dead. Nullable source passes through that real
virtual callback (its source failure must not be moved before earlier loot/
equipment prefix). Nonnull source uses independently reread float hurtDir+yRot
for x and z, f32 factor0.017453292 then actual Mth.cos/sin, fneg and f32*0.1,
finally widening each to double; y is widened float0.1. Null source uses x/z+0
and actual double0.1. Actual DEATHS stat, reset TIME_SINCE_DEATH/REST, clearFire,
onFire flagfalse, current dimension then block position then lastDeath complete
the tail. No inserted freeze reset, menus, removal, timers or health write.

ServerPlayer.die override is a separate existing executor and bypasses both
parents. These two bodies must never be called as its parent fallback.

## Ownership, validation and limits

All associated handles retain actual actor/entity/source/level identity across
callbacks; no mutable borrow survives callback dispatch. All callbacks mandatory,
no default successful stubs. Canonical health9/absorption15 (when the actual
actor is Player), real max-attribute clamp, item/effect/enchantment reductions,
tracker, source dead field, score, logger/Component, world/loot/RNG, pose,
stats and last-death state remain actual-owner obligations. Private health send
is later, not eagerly emitted here. Missing owners must return failure with
already executed prefix intact. Native menu cleanup belongs later removal and
is not synthesized into these original bodies.

Focused standalone `rustc --edition=2021 --test -Dwarnings` commands compile and
run each actual owned file. Checks exercise actual callback trace/order,
retained source/causer/killer/level identities, health/absorption changes during
callbacks, final absorption reread, stat and late-event error prefixes, fully
absorbed/invulnerable/NaN and healing/killcredit branches; generic death veto,
negative score, nonserver branch, named log, captured level vs changed current
level, partial failure before Player tail, double loot, repeated float reads,
null source after guards and Player tail after parent early return. Their hosts
are controlled callback recorders, not real actors/worlds or Main validation.
Full Cargo and namespace exports remain root-owned. No actual production damage,
lethal actor, loot producer, packet audience or respawn integration is claimed.

Independent source review: living_lifecycle reviewed both files against original LivingEntity/Player bodies and found no material mismatch. Root reviewed the callable body and namespace composition; this approval covers these bounded source methods only.
