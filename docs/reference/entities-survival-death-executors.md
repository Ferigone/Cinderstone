# Native ServerPlayer death and living death-tick executors

Instance: es-20261101-1425-a7c3; assigned issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Claim posted before edit: issue4comment6563750981. Owned new namespace files
`src/living_entities/{mod,death_tick,server_player_death}.rs`, only its mechanical
`pub mod living_entities` in `src/lib.rs`, this team ledger and canonical status.
Implementation started at mastere292dc3 in an isolated player-death worktree.

## Original evidence and callable scope

Pinned original Minecraft Java1.21/protocol767 server SHA1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1
`31c77994d96f05ba25a870ada70f47f315330437`; inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Direct JDK21 javap, original line tables and official mappings; no exposed REA
run is claimed. Binaries/raw disassembly/probes stay outside Git.

| Native API | Original method/lines | Actual state/callback owner |
| --- | --- | --- |
| `death_tick::tick_death<H:DeathTickHost>` | LivingEntity (`btn`).tickDeath571–576 | actual saved deathTime, current level, removed state, event and virtual KILLED removal |
| `server_player_death::die<H:ServerPlayerDeathHost>` | ServerPlayer (`aqv`).die694–748 | same original actor, source, message/team/killer/world/score/connection owners |
| required deferred failure supplier | aqv fallback701–705 | actual connection PacketSendListener.exceptionallySend completion owner |
| required neutral-mob notification | aqv.tellNeutralMobsThatIDied751–755 | actual world query/predicate/Mob/NeutralMob and playerDied operations |

These functions add source control flow and ordered virtual operations. They
own no actor, health/alive/dead flag, death counter, connection, inventory, RNG,
world, AI, scheduler, registry or persistence. Every host operation is mandatory
and synchronous at its source callsite; absence of an owner cannot return
successful empty behavior. A later error preserves all earlier mutations and
side effects. Canonical handles retain object identity/lifetime/generation,
including after removal or reuse of numeric entity IDs.

## Death-tick sequence

Read the actual saved LivingEntity.deathTime, add one with Java int wrap, write,
then reread the field. Only if it is at least20 query the current level's logical
side; only server-side query actual isRemoved. If unremoved, query the current
level again inside the broadcast callback, send event60, then invoke virtual
remove(KILLED). Source has no health predicate here, no fresh eligibility check
after event60, and no substitution with DISCARD or network disconnect.

LivingEntity.baseTick's health gate owns whether to call tickDeath. A lethal
out-of-world hit from Entity.baseTick can make this happen in the same enclosing
baseTick. Central must not schedule a later first increment. It must reuse the
actual PlayerLifecycle saved counter rather than add a second timer.

Virtual removal must implement the actual LivingEntity/Player chain: death-only
onMobRemoved(reason) effect dispatch and collection clear, Entity.remove,
Brain memories, inventory-menu/external menu cleanup and tracking/index lifetime.
That is not ordinary removeAllEffects nor complete connection cleanup. A dead
ServerPlayer retains its actual connection/listener for later respawn.

## Separate ServerPlayer override

ServerPlayer.die calls neither parent death body. This executor deliberately
adds no generic removed/dead guard, generic dead write, health setter or DYING
pose. Its actual virtual selection happens in the existing living damage host.

Ordered operations: ENTITY_DIE game event; showDeathMessages rule; actual combat
death Component and kill packet with deferred failure listener, or actual EMPTY
Component packet when disabled; real team routing with each source visibility
reread; shoulders; forgiveDeadPlayers and actual ordered neutral-mob query;
spectator gate and current ServerLevel death loot; DEATH_COUNT objectives;
retained kill-credit/type/victim statistic then current deathScore (including
negative values), killer score and rose; event3; DEATHS and time statistics;
clear fire, frozen0, shared firefalse; combat recheck; then-current dimension
and block position into lastDeathLocation. Each host call preserves the source
location of the read and its preceding side effects.

Team routing receives the actual victim; the source PlayerList operation can
read its current team independently of the captured Team used for visibility.
The captured killer handle similarly remains stable across later callbacks;
deathScore is read after the victim statistic callback, not precomputed.

The packet host must register the original exceptionallySend supplier on the
real connection completion owner, retaining original actor and Component.
Only if that supplier later runs may it truncate the original message, build
message-too-long/even-more-magic/hover Components and reread the original actor's
current display name/entity ID. It cannot prebuild the fallback or invent a
synchronous completion scheduler. Result::Err means synchronous dispatch/owner
failure; later asynchronous send failure executes its retained supplier while
the death method may already have continued/returned. No delayed error may
retroactively abort or roll back the completed death tail.

Loot, XP, scores, criteria, Components, neutral query/AI callbacks and actual
packet publication remain mandatory implementations, not implemented world
bodies in this module. Creative mode is not a source spectator/loot exemption.
Neither executor manufactures an empty world/equipment/effect context.

## Focused validation

Two separate pinned-source reviews found no material mismatch in ServerPlayer
callback ordering or lifetime boundaries. Standalone native flows cover full
message/loot/killer/tail order, repeated visibility reads, disabled-message and
spectator branches, post-stat negative deathScore, and early/late owner failure
prefixes. They are controlled callback recorders, not native world gameplay.
The real async fallback body/Components/neutral AI/loot/XP/scoreboard/Main were
not executed by those checks.

Independent death-tick review plus an unchanged-original-Java probe confirm
18->19,19->20,20->21, int wrap, client/removed short circuit, second current-world
lookup for event60, event before remove(KILLED), and errors. Broadcast callback
may change deathTime/removal state; source still calls remove without recheck.
Original actual deathTime, Level.isClientSide and final Entity.isRemoved are
used, while generated world lookup/broadcast/virtual remove bodies provide
controlled surrounding contexts. This is not proof of actual removal body or
removal-reason overwrite semantics.
Probe source SHA256:
`df90442515ead3a5fd86fe7b5e89f6dfb1f9a70e28436b130d773aca610106a5`;
output SHA256:
`3884611feb17f13a4cf1b508c651ed348c0f93808ed283eeae25508f3f5481e5`.
Native focused checks include the same event mutation/error prefixes.
Full crate/current-master composition results are recorded in the canonical
instance status and issue-linked PR; counts do not measure progress.

## Integration still required

Bind the same canonical ServerPlayer actor to hurt->die, then baseTick->tickDeath
at actual source phases. Required real owners include combat history/message,
health/absorption metadata9/15, equipment/effects/attributes, source/tag/attacker,
physical/spawn timers, source RNG, menus/inventory/loot/XP, death/removal reason,
lastDeathLocation save/load, packet FIFO and connection completion behavior.
No such production Main binding is created by these namespace exports.

PlayerList.respawn remains unimplemented: new actor generation with old wire ID
and actual retained connection, exact restore rules, real bed/anchor/world/default
spawn queries, source packet/insertion/menu/health order. It must not repeat
fresh login/configuration/admission or disconnect cleanup. Details are in
[the death/respawn contract](entities-survival-death-respawn.md) and
[effects/attributes ownership](entities-survival-effects-integration.md).

Generic LivingEntity.die, Player.die, actual death loot/removal bodies, full
respawn, mobs/AI/spawning/tracking/persistence and whole survival/entities
remain unsupported. This bounded executor stage does not complete any whole
fixed checklist delivery or establish native lethal survival in Main.
