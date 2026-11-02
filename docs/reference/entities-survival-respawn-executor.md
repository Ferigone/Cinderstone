# Native PlayerList respawn and actual-owner contract

Instance: es-20261101-1425-a7c3; issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Source base: `c1baf5c70f07e53b6b76edaf84be5da57f6bd40b`.
Pinned original Minecraft Java1.21/protocol767 server SHA1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1
`31c77994d96f05ba25a870ada70f47f315330437`; unchanged inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence is direct JDK21 javap and specifically labeled controlled runtime
observations, not an unperformed REA run. Raw disassembly, Java binaries and
probe artifacts stay outside Git. No actual Main invocation is inferred.

Respawn reserved-central contract; read-only source audit 2026-11-01
Audited worktree: /workspace/cinderstone-living-next-es-20261101-1425-a7c3
This ledger is scratch evidence, not a published claim or production integration.

Source executor: new src/living_entities/respawn.rs; PlayerList.respawn469-530.
Actual ServerPlayer constructor193-314 + restoreFrom1415-1467 +
findRespawnPositionAndUseSpawnBlock827-841/857-885 remain mandatory full owners.
The executor does not allocate an actor/connection/RNG or implement those bodies.

Smallest real central milestone: source below-world lethal producer -> actual
ServerPlayer die -> Living tickDeath/removal -> client command action0 -> actual
new ServerPlayer generation with same connection + old wire entity ID, default
spawn/world queries, restoreFrom and ordered source insertion/output. No health
clamp, fabricated dead/alive owner or health-only respawn. Full lethal tail and
menu/item/XP ownership must be available before enabling this production path.

Reserved seams requiring explicit narrow central assignment:
1. src/main.rs:807 register_runtime_player discards the canonical EntityHandle
returned by item_runtime.register_player; runtime_entities.rs:588 returns it.
Expose/retain that actual identity for body lifetime, no new generation allocator.
At runtime_entities.rs:604 remove_player(id) resolves the current handle; supply
handle-qualified remove/move/query to reject stale old-generation callbacks.
2. src/main.rs:831 remove_runtime_player removes connection_admissions,
chunk_transports, observers/player_random and all native owners. Split actual
body retirement from session disconnect. src/main.rs:2221/2225 cleanup_player*
closes menus/saves/removes registry/chat/address/ACK/action/disconnect owner and
is never the death-tick/removal or respawn operation.
3. src/main.rs:3130+ PLAY captures container3158, gamemode3175/native_motion3178
once. Return/apply actual RespawnSessionUpdate after action0 (currently no-op
3575) replacing actor generation, menu/container, metadata/motion and position
references plus per-menu revision baseline. Preserve actual connection listener,
writer/codec/rate/inactivity/admission/chat/signature/ACK/cookie owners. Do not
reset login policy, perform LOGIN/CONFIGURATION or allocate another connection.
4. src/main.rs:3158-3161 menu publisher and3827 PlayerEvent::Container use only
numeric entityID+revision. Qualify publisher recipient with actual actor lifetime
and consume only current generation. New menu revision0 must not compare against
old menu's high revision. Preserve/rebind all menu callbacks and source inventory
owner at initInventoryMenu; old callback must not mutate/publish to new actor.
5. src/main_player_motion.rs:148 PlayerMotion contains physical and retained
MovementState(listener) together. main_player_motion_hooks.rs:88 register_native
constructs PlayerMotion::new and120-125 reserves initial teleportID1. Respawn must
replace constructor-owned physical/body/attributes/step state but retain the
same listener MovementState, then invoke the actual source connection.teleport
at PlayerList506. player_movement.rs:84-98 contains teleport sequence, pending
confirmation, movement counts/tick/firstGood/lastGood/floating/blockACK owner;
teleport393-449 mutates the existing sequence, never restart at1.
6. src/main.rs:1064 allocate_player_entity locks players internally. Actual new
constructor must consume global Entity counter and create_player_random even
though wireID is overwritten oldID at PlayerList480. A locked owner/helper is
needed under existing world->players operation; no recursive players lock, fake
counter, copied old RNG or skipped constructor Math.random work.
7. src/main.rs:793 replication_snapshot includes every registry player.
src/entity.rs:558 remove + replication.rs:364/533 remove_known emits BOTH destroy
body and RemovePlayerInfo. Death entity retirement needs canonical removal/body
generation projection so old body is not re-paired while PlayerList/session/tab
profile persists. Do not add an independently mutable removed/alive mirror.
At respawn514 insert replacement actual world/index/tracking lifetime before
PlayerList append515 and UUID-map replacement516; qualify old tracking callbacks.
8. src/main_chat.rs:166/204/210/419 registration_order is also current source
PlayerList audience order but connection owner includes signature cache. Source
respawn removes+appends actor in PlayerList: update actor/list ordering without
remove/register of the retained chat listener/cache.
9. src/main.rs:1007 capture_player_locked captures metadata health/absorption and
canonical PlayerLifecycle/inventory/stats. Do not save an arbitrary old lifecycle
clone for respawn. Dead restore uses current max health after attribute-base assignment and
constructor-owned food/effects; inventory
andXP retained only keepInventory||oldSpectator. Runtime dead/removal flags,
invulnerability/spawn timers/source history/CombatTracker are constructor/runtime
state, not blindly persisted player.dat fields. player_nbt.rs:101/188 handles
DeathTime short lineage; actual Living save717-744 also writes HurtTime and
HurtByTimestamp, currently unknown retained fields may go stale if unmodeled.
PlayerLifecycle.validate172 disallows negative deathTime although Java short
load can be negative; reconcile explicitly if import support expands. Current
MainPlayerBody health guard and dead_or_dying=false prevent lethal integration.

Output/call phase contract:
Run the whole server-thread client-command action0 operation at central operation
ordering boundary before world->players. Existing chat/admin uses chat_dispatch
-> player_admission; holding only players.write does not exclude interleaved
admin output while callbacks release it. No async guard acquisition from sync
callback holding world/players. Source output order is Respawn502 -> existing
connection teleport506 -> defaultspawn507 -> difficulty508 -> XP509 -> active
effects510 -> levelinfo511 -> permissions512 -> worldadd514/list515/UUID516 ->
menu518 -> setHealth(getHealth)519 -> dead-anchor sound526. LevelInfo714-728 itself
sends real border/time/defaultspawn/weather then ServerTickRateManager.
updateJoiningPlayer116-118. Ordered output must not route one sequence through
raw direct codec writes and other parts through independent event channels.
Terminal/rate-limited frame preemption retains already committed state/items/RNG;
normal death/removal does not trigger the disconnect watch or terminal cleanup.

Source locals/callback boundaries in executor:
Destination local5 comes from transition.level475; current actor level local9
and its leveldata10 are read500-501; defaultspawn/worldinfo/insertion/anchor query
andRNG use retained destination. All connection fields are re-read at each send.
Actor teleport coordinates are re-read AFTER Respawn send. Missing-block flag
is queried483 and493; transition.position is read491 and again522. Tags use the
actual old live iterator487-489. Anchor coordinates are integer block coords,
not centered or currentactor coords; only dead&&actual anchor advances destination
random.nextLong. Asynchronous output failures are real connection semantics;
executor callback Err stops only its synchronous completed-prefix model.

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
