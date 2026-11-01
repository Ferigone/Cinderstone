# Java 1.21 death, removal and respawn source contract

Team issue: [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Instance: `es-20261101-1425-a7c3`.

This is a **source-analysis and integration-contract note**. It adds no native
death/respawn executor, actor state, Main callsite or gameplay result. The
[damage dispatch ledger](entities-survival-damage.md) records the separate
callable hurt/reduction/totem implementation. Calling its `die` host method
does not establish that the bodies described here have run. No complete
`survival_combat.3`, entity, mob or survival delivery is claimed.

## Pinned evidence

The original inputs match `fixtures/source-lock.json`: Minecraft Java 1.21,
protocol 767, DataVersion 3953; server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, size 51,623,779 bytes;
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`, size
7,454,609 bytes; unchanged extracted inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

Evidence is direct JDK 21 `javap -c -p -l` inspection, interpreted with the
official mappings and original line tables. REA tools were not exposed; no REA
run, Evidence ID or death/respawn runtime observation is claimed. Artifacts and
raw JVM disassembly remain outside Git.

| Mapped class | Original class | Methods inspected, Java source lines |
| --- | --- | --- |
| LivingEntity | `btn` | `baseTick` 395–506; `tickDeath` 571–576; experience methods 579–616; `remove` 700–706; death-effect removal 709–713; save/load 717–805; health/liveness 1128, 1609; `die` 1377–1405; rose/loot/XP 1408–1486; kill credit 1781–1787 |
| Player | `cmx` | `die` 692–715; equipment/vanishing drops 719–733; `remove` 1450–1456; base XP/always-drop 1861–1875 |
| ServerPlayer | `aqv` | constructor 193–314; spawn adjustment 318–358; `tick` 522–564; `doTick` 587–654; `die` 694–748; neutral-mob notification 751–755; respawn-position methods 827–885; `restoreFrom` 1415–1467; death-message fallback 701–705 |
| Inventory | `cmw` | compartments constructor 36–48; `dropAll` 497–506 |
| PlayerList | `aur` | `respawn` 469–530; active-effect replay 534–541 |
| ServerGamePacketListenerImpl | `aru` | `handleClientCommand` 1659–1683 |
| AttributeMap | `but` | `assignBaseValues` 112–118 |

## Health, death guards and virtual dispatch

These source fields must not be collapsed into one synthetic alive/dead flag:

- `isDeadOrDying` reads synchronized health (`getHealth() <= 0`), at 1128.
- `isAlive` requires both unremoved state and health greater than zero, at 1609.
- `LivingEntity.dead` (`be`) is a separate guard used by its generic `die` body.
- `deathTime` (`aQ`) is the death-tick counter; it is not the hurt cooldown.
- Entity removal reason/state is separate from death health and connection
  membership. `KILLED` and `DISCARDED` have specific effect-removal callbacks.

The class hierarchy does **not** imply that all three death bodies execute:

| Selected virtual body | Parent dispatch and its own work |
| --- | --- |
| `LivingEntity.die` 1377–1405 | Generic implementation, with removed/dead guard, mapped dead-field mutation and final DYING pose |
| `Player.die` 692–715 | Calls `LivingEntity.die`, then performs its own position, loot, motion, statistics and fire/death-location work |
| `ServerPlayer.die` 694–748 | Separate override; calls neither parent death body. It has no generic removed/dead guard, dead-field assignment or explicit DYING-pose call in this method |

Production ServerPlayer damage must dispatch to the ServerPlayer body. Adding
the generic dead guard, invoking all parent bodies, or using a local flag in
place of their effects changes the source behavior. A health setter is not a
death executor.

## Generic living and Player death bodies

`LivingEntity.die` first returns for removed or already-dead state. It then
captures the causing entity and resolves kill credit: credited Player first,
otherwise last-hurt-by-mob (1781–1787). When death score is nonnegative and kill
credit exists, it awards the killer score before sleeping cleanup and the
server's named-entity death log. It sets the actual `dead` field, rechecks the
CombatTracker and, on ServerLevel, invokes the causing entity's `killedEntity`
callback when present. Only a missing causing entity or a true callback result
enters death game event, loot and wither-rose work. Event 3 follows that branch;
the DYING pose follows the ServerLevel block.

`Player.die` calls that body, reapplies position, then on the server and while
not spectator invokes its loot path. With a source, its death velocity uses
the actual hurt-direction and yaw, source trigonometry and float-to-double
operations; its null-source branch sets a different explicit velocity. It
awards DEATHS, resets TIME_SINCE_DEATH and TIME_SINCE_REST, clears fire and the
shared on-fire flag, then stores the actual dimension/block-position death
location. This body must not be substituted for the ServerPlayer override.

## ServerPlayer death order

`ServerPlayer.die` 694–748 performs the following source sequence:

1. Emit `ENTITY_DIE`, then read `showDeathMessages`.
2. When enabled, obtain the actual CombatTracker death Component, dispatch
   `ClientboundPlayerCombatKillPacket` to the victim with the source
   `PacketSendListener.exceptionallySend` fallback, then broadcast the death
   message according to the actual team's death-message visibility. No team or
   ALWAYS broadcasts to all; HIDE_FOR_OTHER_TEAMS routes to the team;
   HIDE_FOR_OWN_TEAM routes outside it; NEVER broadcasts none. When disabled,
   send the victim the combat-kill packet with an empty Component.
3. Remove actual shoulder entities. If `forgiveDeadPlayers`, notify the actual
   nearby neutral mobs, using the source box around the block position expanded
   by 32 horizontally and 10 vertically, its Mob query/predicate, NeutralMob
   filter and `playerDied` callback (751–755). A certified query with no mobs
   differs from omitting the query or inventing an AI owner.
4. Unless spectator, run `dropAllDeathLoot` with the actual ServerLevel/source.
5. Increment actual DEATH_COUNT objectives. Resolve kill credit; when present,
   award the victim's ENTITY_KILLED_BY statistic, call the killer's
   `awardKillScore(victim, deathScore, source)`, then create a wither rose. This
   override does not insert the generic body's nonnegative-score guard.
6. Broadcast entity event 3. Award DEATHS; reset TIME_SINCE_DEATH and
   TIME_SINCE_REST; clear fire, set frozen ticks to zero and clear shared fire.
7. Recheck CombatTracker status and store the actual last-death dimension and
   block position.

The failed-send fallback constructs a combat-kill packet with the source
`death.attack.even_more_magic` message and hover text based on
`death.attack.message_too_long`, using at most 256 characters of the original
message (701–705). A plain queued Component is not this completion callback.
The existing damage pipeline makes a fresh-hit death sound **before** calling
virtual `die`; the death body must not add a second sound.

## Loot, canonical inventory and XP

`LivingEntity.dropAllDeathLoot` 1431–1439 snapshots
`lastHurtByPlayerTime > 0`. If `shouldDropLoot` and `doMobLoot`, it calls the
loot-table path and then custom death loot. It then calls virtual
`dropEquipment` and `dropExperience(source.getEntity())` regardless of that
table/custom gate. Wither-rose placement/spawning has its own actual killer,
mobGriefing, block/world and ItemEntity callbacks (1408–1428).

`Player.dropEquipment` 719–724 calls the base implementation, then, unless
`keepInventory`, destroys actual items with the applicable vanishing effect
and invokes `Inventory.dropAll`. Vanishing removal scans the real full
inventory and removes matching nonempty stacks (727–733).

Inventory compartments are ordered items (36), armor (4), offhand (1).
`dropAll` 497–506 visits that order and, for each nonempty canonical stack,
calls `Player.drop(stack, true, false)` **before** replacing that slot with
`ItemStack.EMPTY`. Actual item construction, entity IDs, per-entity/world RNG,
world addition and source listener/stat behavior remain attached to the existing
owners. A privately cleared snapshot or rollback after a later failure does
not reproduce this mutation prefix. Carried stacks and temporary menu grids
have separate removal callbacks; they are not extra Inventory compartments.

`dropExperience` 1445–1448 requires ServerLevel and unconsumed experience, then
either `isAlwaysExperienceDropper` or the ordinary recently-hit/
`shouldDropExperience`/`doMobLoot` conjunction. Player's always-drop predicate
is true (1875). Its **base** reward is zero for keepInventory or spectator;
otherwise Java integer `experienceLevel * 7`, capped only above 100
(1861–1869). Final `getExperienceReward` 608 passes server, actual causing
entity, victim and base reward through the real enchantment evaluator before
`ExperienceOrb.award` at the actual position.

Creative mode is not an inventory or XP exemption in these death branches.
Empty inventory, zero XP, absent applicable enchantments and empty listener
registries must be established by actual ownership when used to bound a profile.

## Death ticks, removal and menus

`LivingEntity.baseTick` runs server enchantment tick effects at 403, then
`Entity.baseTick` at 406, followed by environmental callbacks. Hurt time
decrements at 469–470; physical invulnerability time decrements at 472–473
except for ServerPlayer. The health-based death gate calls `tickDeath` at
475–476, before attacker expiry and potion `tickEffects` at 496.

Consequently a lethal below-world hit during `Entity.baseTick` reaches its
first death-time increment later in that **same** base tick. A hit after the
base-tick phase has a different first-increment boundary. ServerPlayer's separate
`tick` 522–530 owns its spawn-protection decrement and positive physical
invulnerability decrement; do not introduce side timers.

`tickDeath` 571–576 increments the actual `deathTime`. At at least 20, on the
server and while unremoved, it broadcasts event 60 and then calls virtual
`remove(KILLED)`. Living removal 700–713 invokes each actual active instance's
`onMobRemoved(reason)` for KILLED/DISCARDED, then clears that map, calls parent
Entity removal and clears Brain memories. This is **not** `removeAllEffects`:
do not substitute its per-effect `onEffectRemoved` callbacks.

Player removal 1450–1456 calls its parent removal, then
`inventoryMenu.removed(this)` and, when a container exists and is open,
`doCloseContainer`. Removal reached by death tick or earlier respawn must run
these real menu callbacks once in their source order. Inventory death drops at
the lethal call and menu cursor/grid removal are distinct phases.

Entity removal is not connection disconnection. A dead ServerPlayer retains its
session and PlayerList membership while the client may request respawn. Current
Main disconnect cleanup closes menus/saves and removes chat/connection owners;
using it wholesale for death would destroy the wrong lifetime. Native tracking
and the shared entity index must retire the old body without allowing a later
registry snapshot to pair that removed body again.

## Respawn: new actor, retained connection and numeric ID

`handleClientCommand` 1659–1683 first ensures server-thread execution and resets
last-action time. PERFORM_RESPAWN has a separate won-game path: clear wonGame,
respawn with `alive=true` and CHANGED_DIMENSION, then run the dimension-change
criterion. Otherwise positive health returns without respawn; dead health enters
`respawn(alive=false, KILLED)` and assigns the returned actor to the listener.
Hardcore then switches to spectator and changes spectator-generates-chunks.
REQUEST_STATS remains a separate branch.

`PlayerList.respawn` 469–530 orders actual state replacement and output:

1. Remove the old object from the PlayerList list and old ServerLevel with the
   supplied reason. Resolve `findRespawnPositionAndUseSpawnBlock` next.
2. Construct a new ServerPlayer in the resolved destination with the old profile
   and client information, using the real constructor/spawn/RNG paths. Assign
   its connection from the old actor, run `restoreFrom`, then set its numeric
   entity ID to the **old ID** and copy the main arm. Copy valid respawn position
   data and entity tags, then move to the resolved position/yaw/pitch.
3. Emit the missing-respawn-block game event when applicable. Send Respawn with
   common player spawn info and keep-data byte `alive ? 1 : 0` (502), connection
   teleport (506), default spawn position (507), difficulty (508), experience
   (509), active effects (510), level info (511) and permission level (512).
4. Add the new actor to the destination (514), append to PlayerList (515),
   replace its UUID map entry (516), initialize the new inventory menu (518),
   then call `setHealth(getHealth())` (519).
5. For a dead respawn at a respawn anchor, dispatch the anchor-deplete sound,
   with its actual destination-level random seed (521–526).

The new object has a new native lifetime/generation even though the wire ID is
reused. Constructor RNG/identity work cannot be skipped because that ID is
overwritten later. Old handles/callbacks must not resolve to the replacement.
This is the existing connection/listener, not LOGIN or CONFIGURATION: source
respawn does not rerun login admission policy or reset connection rate,
inactivity, encryption, signature-cache or acknowledgment owners.

`restoreFrom` 1415–1467 reuses WardenSpawnTracker and RemoteChatSession, restores
current/previous game mode and sends abilities, assigns attribute **base values**
and sets health to actual maximum. `alive=true` copies inventory, old health,
the actual FoodData object, copy-constructed active effects, XP/score and portal
process. For `alive=false`, keepInventory or the old spectator state controls
inventory/XP/score retention; food and effects otherwise retain the new
constructor state, while health already equals the current maximum from the
unconditional attribute-base/health prefix. The dead branch omits old health. It always carries enchantment seed, EnderChest owner, model
customization, recipe book, credits/nether/tracking state, shoulder compounds
and last-death location. It resets last-sent XP, health and food caches to -1.
Neither a wholesale old lifecycle clone nor a health-only reset models this.

## Required native integration and next bounded milestone

The first real actor milestone can use the existing below-world producer and
the actual out-of-world source, amount 4, but must include its full lethal
ServerPlayer path and respawn. That is a bounded native creative void exception,
not complete survival behavior or evidence for formula branches bypassed by
that source. Unsupported contexts require admission/capability checks before
damage begins; never clamp an ordinary lethal hit to one health.

The concrete integration still requires:

- Canonical metadata health/absorption (9/15), physical invulnerability time,
  actual actor identity/RNG and existing saved `PlayerLifecycle.death_time`.
  Assign mapped hurt/death/attacker/combat fields to one actor owner; do not add
  a competing persistent health, death counter or synthetic alive flag.
- The complete ServerPlayer death body above, canonical inventory/vanishing/
  item/XP operations, source death-message completion callback, scoreboard and
  criterion owners, actual game events and world queries. Missing work is not
  supplied by a successful callback default or a fake empty inventory.
- Real post-lethal body ticks, removal/effect/menu ordering, tracked-body
  retirement while keeping the session, and the same-connection/new-generation
  respawn replacement and packet order. Main's current alive-only body and
  action-zero no-op are not that implementation.
- Native and Java save/export updates for actual hurt timestamps/death counters,
  last-death location and respawn dimension/position/angle/forced state. Unknown
  retained source NBT cannot be a runtime owner and can become stale after
  mutations. Resolve source default spawn or supported bed/anchor queries;
  Main's login coordinate is not a substitute respawn algorithm.
- The centrally owned operation guard before world/player locks, actual
  tracked-plus-self transient FIFO publication, ordinary metadata and later
  private health delivery. Keep normal death distinct from disconnect watches
  and transport rate/inactivity termination. A safe frame boundary or queued
  packet does not make an entire death sequence atomically delivered.

Main, shared entity/index/tracking/persistence seams and active menu work remain
with the integration coordinator. Native implementation and a focused connected
lethal/death/respawn flow are pending. This static ledger supplies exact source
requirements; it is not their implementation or validation.
