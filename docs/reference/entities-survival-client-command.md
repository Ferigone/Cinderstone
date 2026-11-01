# Native client respawn and statistics command dispatch

Instance: es-20261101-1425-a7c3; issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Source base: `c1baf5c70f07e53b6b76edaf84be5da57f6bd40b`.
Pinned original Minecraft Java1.21/protocol767 server SHA1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1
`31c77994d96f05ba25a870ada70f47f315330437`; unchanged inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence is direct JDK21 javap and specifically labeled controlled runtime
observations, not an unperformed REA run. Raw disassembly, Java binaries and
probe artifacts stay outside Git. No actual Main invocation is inferred.

Client command source/API ledger — es-20261101-1425-a7c3
Only implementation: /workspace/cinderstone-living-next-es-20261101-1425-a7c3/src/living_entities/client_command.rs
Claim root reports issue4comment6564074170. No shared/Main/Git edits by author.

Evidence: direct original Java1.21 javap and official mappings, not REA.
Original inner SHA2568d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
Raw scratch oracle/aru-death.javap.txt method public void a(agt), source1659–1683;
oracle/aqv-tracking.javap.txt resetLastActionTime1689–1690;
oracle/client-command-stats.javap.txt original avv.sendStats171–178.
Mappings identify agt=ServerboundClientCommandPacket; action a=PERFORM_RESPAWN,
b=REQUEST_STATS; bsr$c.e=CHANGED_DIMENSION, a=KILLED; an.w=CHANGED_DIMENSION;
dcw.j=END,h=OVERWORLD; dcs.r=RULE_SPECTATORSGENERATECHUNKS; ad.c=Util.getMillis137.

Callable API:
ClientCommandAction::{PerformRespawn,RequestStats}, decoded typed enum only.
RespawnRemovalReason::{ChangedDimension,Killed}, exact source enum choices.
ClientCommandHost requires Error,Packet,Player,Level,PlayerList,GameRules,
BooleanRule,Stats associated native owner/identity types, no default callbacks.
pub fn handle_client_command<H:ClientCommandHost+?Sized>(&mut H,&H::Packet)
 -> Result<(),H::Error>.
Packet is the actual already decoded command; host.action reads its field after
reset, not another wire decoder. Player identifies actual lifetime/generation;
all host.player calls reread the real listener.player field at the source point.

Exact source ordered facts:
1659 current listenerplayer -> serverLevel -> actual PacketUtils.
ensureRunningOnSameThread(originalPacket,thisListener,level), including its real
reschedule/stop control flow; no reset/action branch executes on callback error.
1660 reread listenerplayer -> resetLastActionTime (actual Util.getMillis clock).
1661 access alreadydecoded packet action;1662 switch.
1664 PERFORM_RESPAWN reads current wonGame. If true:1665 reread player then set
wonGamefalse;1666 actual server.getPlayerList FIRST, then reread old player,
respawn(old,true,CHANGED_DIMENSION), assign returned actual actor to listener.
1667 reread listenerplayer for actual CHANGED_DIMENSION.trigger(player,END,
OVERWORLD). Won-game branch does not query health or hardcore.
1669 ordinary path rereads player then health; health>0 returns1670. This is
strict Java float comparison, so ±0,negative and NaN do not take the earlyreturn.
1672 actual PlayerList obtained before reread old player; respawnfalseKILLED,
assign returned actual actor.1673 actual existing server.isHardcore query.
If hardcore1674 reread currentplayer then actual setGameMode(SPECTATOR), ignoring
boolean result;1675 reread listenerplayer again -> actual current Entity.level ->
GameRules -> RULE_SPECTATORSGENERATECHUNKS BooleanValue -> set(false,server),
retaining actual rule callbacks. No fake bool/game-mode mirror is supplied.
1680 REQUEST_STATS captures currentplayer.getStats() counter, THEN REREADS
listener.player as sendStats recipient. First Stats owner remains retained even
if its lookup callback replaces current listenerplayer; never relookup stats for
that second actor. Original sendStats171–178 drains real dirty set before send
and sends even an empty delta; no invented throttling or all-stats snapshot.
1683 return. Synchronous Err leaves all prior source mutations/output committed;
cleared wonGame and successful actor replacement are not rolled back/retried.

Mandatory integration boundaries/limits:
No actual PlayerList/newactor factory, restoreFrom, source spawn/world queries,
menu/loot/XP/removal, thread scheduler, clock, actor, listener, connection, RNG,
stats or game-rule owner is created here. Full host.respawn can compose existing
respawn.rs only with all actual native owners. Runtime no-op callbacks must not
be presented as production respawn. Same listener/connection, new generation and
same old numeric entityID; source returned actor must replace PLAY's captured
container/metadata/motion/position/menu revisions and generationqualified old
callbacks. No login policy readmission, CONFIGURATION or new transport/listener.

Existing Main reserved seam: main.rs3575 decodes0x09, ignores action0, action1
uses stats_packet1535 -> actual canonical send_stats_packet1551 and output but
has no native client-command executor/currentactor replacement here. Decode and
accepted ingress count must remain once. Main player_last_actions491/3268 is
existing actor-action timestamp map; avoid a second clock or treating source
resetLastActionTime as transport inactivity reset. NewServerPlayer constructor
still owns the replacement actor's own initial fields/time; no wholesale clone.
Run source command at actual central server operation phase using outer ordered
chat_dispatch -> player_admission before world -> players. Hold/order against
chat/admin/disconnect while source callbacks produce outputs; do not add async
lock acquisition inside sync callbacks holding world/player locks. Actual central
thread scheduling and actor/persistence/packet contracts remain unassigned to
this file. See scratch/respawn-main-contract-ledger.txt for exact patch locations.

Validation: rustfmt applied; standalone rustc1.85 -Dwarnings test and ordinary
rlib compile passed. Controlled focused flows cover currentactor mutation around
thread/reset/won/playerlist/assign/spectator/stats callbacks; credits/deadhealth/
NaN/hardcore/stats branches; error prefixes before reset and after wonGameclear
or actorreplacement. These are mock source-call tests, not actual Main/JVM actor
respawn/thread/stats runtime observations. FullCargo intentionally left to root.

## Integration and validation boundary

These callable source bodies require actual canonical actor, world, item,
attribute/effect, inventory/menu, entity ID/generation, packet, RNG, persistence
and connection-lifetime owners at their source callsites. Missing owners cannot
return successful no-op behavior, manufacture an empty world or reset a listener.
Synchronous errors retain completed mutation/output prefixes; they are not
rollback transactions. A later async transport failure belongs to the actual
connection completion owner and cannot retroactively stop a completed body.

Independent source reviews and focused controlled native flows cover the
bounded implementations; final exact compiler/crate results and source SHAs
are recorded on the issue-linked PR and canonical per-instance status. The
selected actual milestone is a lethal ServerPlayer lifecycle. Shared Main/body/
index/session/menu-generation/save seams still require central narrow assignment.
No whole entities or survival delivery, public gameplay or checklist change is
claimed. [Concrete production contract](entities-survival-production-milestone.md)
and [original lifecycle facts](entities-survival-death-respawn.md) describe the
remaining work.
