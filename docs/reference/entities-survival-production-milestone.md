# First production Player damage, death and respawn milestone

This is a concrete central integration request for issue [#4](https://github.com/Ferigone/Cinderstone/issues/4),
instance `es-20261101-1425-a7c3`, based on master
`c1baf5c70f07e53b6b76edaf84be5da57f6bd40b`. It grants no reserved-file ownership.
This note's claim is [issue #4 comment 6564157625](https://github.com/Ferigone/Cinderstone/issues/4#issuecomment-6564157625).
The audited Main, body, motion, lifecycle/NBT, entity-index and replication
sources did not change between `3e924b2` and this base. The workstation merge
therefore does not resolve the lifecycle contracts below.

The first bounded production path should use the existing below-world query:
actual `LivingEntity.onBelowWorld` damage, amount 4, through the full
ServerPlayer damage/death path, entity retirement and same-connection respawn.
It must permit a lethal result. A creative void flow can establish that bounded
native exception; it cannot establish survival behavior or formula branches
bypassed by its source. Survival admission requires its actual game mode,
abilities, food, attributes and callbacks, rather than retaining a creative
approximation under a survival label. Unsupported contexts must be recognized
before damage starts; clamping lethal damage or skipping its tail is invalid.

Evidence is the pinned Java 1.21/protocol 767 server and official mappings,
inspected directly with `javap`; no REA run is claimed. Original server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Detailed source facts remain in the linked team reference notes.

## Callable source and current publication boundary

These namespaces are public through `src/lib.rs`. “Merged” means present at
the audited master, not called by Main. The additional lifecycle modules are
mechanically exported in the current feature checkout; their publication and
integration must be established by actual later commits/PRs on issue #4.

| Exported API | State at this base | Required composition |
| --- | --- | --- |
| `player_damage::{server_player_hurt,hurt,actually_hurt}`, `living_damage::hurt` | merged #8 | ServerPlayer → Player → LivingEntity, with virtual Player actual damage |
| `combat::reduction::{get_damage_after_armor_absorb,get_damage_after_magic_absorb}`, `combat::totem::check_totem_death_protection` | merged #8 | real source/tags/equipment/definition/stat/effect owners |
| `combat::tracker::CombatTracker::{new,record_damage,recheck_status}` | merged #10 | one transient tracker per actual actor; death-message selection is still external |
| `effects::EffectInstance`, `effects::attribute_instance::AttributeInstance`, `effects::retained::tick_retained` | merged #7/#11/#12 | real collection, holder/definition, attribute-map and retained-object owners |
| `living_entities::{server_player_death::die,death_tick::tick_death}` | merged #15 | actual virtual death selection and LivingEntity base-tick death phase |
| `removal::{remove_player,remove_living,trigger_on_death_effects,notify_effect_instance_removed}` | feature stage | actual Entity parent removal, effect collection, Brain and menu owners |
| `death_loot::{living_drop_all_death_loot,living_drop_experience,player_drop_equipment,destroy_vanishing_cursed_items,inventory_drop_all,player_base_experience_reward}` | feature stage | real loot, inventory, enchantment, item and XP-orb ownership |
| `respawn::respawn`, `restore::restore_from`, `client_command::handle_client_command` | feature stage | real constructor, replacement actor, retained listener and ordered output |

`death_loot::living_drop_equipment` and
`death_loot::player_is_always_experience_dropper` are inspected base bodies,
not defaults for unknown overrides. None of these exports supplies a production
actor, scheduler, active-effect map, world query or packet owner. Required host
callbacks must execute their complete source bodies; successful no-ops do not
make the path native. See [damage](entities-survival-damage.md),
[combat history](entities-survival-combat-tracker.md),
[effects/attributes](entities-survival-effects-integration.md) and
[death/respawn source](entities-survival-death-respawn.md).

## Exact central state and initialization request

Keep `PlayerState.entity_data[9]` health and `[15]` absorption authoritative.
Reuse `EntityPhysicalState` position, velocity, tick count and physical
`invulnerable_time`; reuse saved `PlayerLifecycle.death_time`, food/abilities and
the actual container inventory. Bind the missing mapped hurt fields, attacker
identities/timestamps, last source/stamp, combat tracker, effect collection and
attribute map to that same actor lifetime. Add no permanent `HealthState` mirror,
second death clock or synthetic alive/dead flag. Java's generic LivingEntity
`dead` field is distinct from health/removal; ServerPlayer.die does not set it
or invoke either parent death body. Removal reason must have one real owner,
with `isRemoved`/physical presentation derived consistently from it.

Actual constructors initialize the new actor, including default-zero/null hurt
history, fresh tracker/effects, attributes, body, menus and FoodData. ServerPlayer
constructor 193–314 initializes spawn protection to 60 and obtains its own
`Util.getMillis` action timestamp. That spawn timer is distinct from physical
invulnerability. The factory must consume the existing global entity-ID and
player/entity RNG constructor work even though PlayerList later overwrites the
new numeric ID with the old one. `main.rs:1064::allocate_player_entity` currently
locks players itself: provide a correctly locked internal owner rather than
recursively acquiring it, copying old RNG or inventing a counter.

Use the actual generational identity returned by
`runtime_entities.rs:588::register_player`; `main.rs:807::register_runtime_player`
currently discards it. Extend current canonical identity handling for actor
replacement; do not create another generation allocator. Require
handle-qualified removal/movement/query APIs: current `remove_player(id)` at
604 resolves whichever actor presently owns the numeric ID.

`restore_from(new,old,alive,host)` must perform the source-selected transfer,
not clone `PlayerLifecycle`. Alive restoration shares the old FoodData object,
copies inventory through its real method and effects through the native copy
constructor/addEffect, then XP/score/portal state. Dead restoration retains
inventory/XP only for new-level keepInventory or old spectator; FoodData and
effects otherwise remain constructor-owned. The unconditional restore
prefix assigns attribute base values then sets health to the resulting current
maximum; the dead branch does not copy old health. Preserve object identities,
attribute base-value assignment and callbacks. Last-sent XP, health and food
become -1 in that order, rather than using fresh-constructor synchronization
sentinels unchanged; `main_player_food.rs:50::register_native` needs that explicit
respawn distinction.

## Source phases, retirement and retained session

`entity_base.rs:298` already performs the below-world query, but
`player_body_world.rs:83::on_below_world` explicitly errors. Implement the
LivingEntity override through the original DamageSource and wrapper chain;
never use the generic Entity discard default. Complete the source damage tail,
including totem selection, ServerPlayer death dispatch and real loot/stat/event
work, before enabling the producer.

Keep source ServerPlayer.tick timers (522–530) separate from its connection
doTick/Player body phase. The existing connection-phase entry is
`main.rs:1341/1358`; its adapter needs actual surrounding Player/Living phases,
not an additional invented scheduler or tick counter. LivingEntity.baseTick
395–506 executes server EnchantmentHelper.tickEffects at403 before the
parent Entity.baseTick406. Hurt-time decrement follows at469–470, physical
invulnerability decrement except ServerPlayer (472–473), health-gated death
ticks (475–476), attacker expiry (478–491), then effects (496). A lethal
below-world callback can therefore reach the first death-time increment during
that same enclosing base tick.

`main_player_body.rs:629` currently detaches its motion entry, rejects nonpositive
health at 659 and supplies `dead_or_dying=false` at 724. Its completion at
756–790 projects physical state and reinserts by numeric ID. Replace these
assumptions with canonical actor access and generation-qualified completion:
nested removal/replacement must not reinsert an old body over its successor.
Retained effect leases and source callbacks require the original actor and
selected object to remain addressable during synchronous reentry.

Death-time 20 publishes event 60 before virtual KILLED removal. Compose Player
removal → Living removal → actual Entity.remove, including effects/Brain/menu
callbacks, even when an earlier removal already set a reason. Inventory/menu
cleanup is distinct from death equipment drops. Do not call
`main.rs:831::remove_runtime_player` or `2221::cleanup_player`: they remove
connection admission, transport, RNG, chat/ACK/address owners and save/cleanup
the entire session. A dead removed body still has its PlayerList/session owner.

Body retirement must also differ from `entity.rs:558::remove` plus
`replication.rs:533::remove_known`, which currently destroy the entity and remove
PlayerInfo. Preserve the online profile, retire the actual body generation and
exclude it from `main.rs:793::replication_snapshot` until replacement. Project
canonical removal state; do not maintain an independent visibility alive flag.

The proposed central command result is an actual `RespawnSessionUpdate`:
replacement actor identity and current container/menu, metadata, motion and
position references with a new menu revision baseline. This is a requested
adapter result, not an existing export or new session owner. PLAY currently
captures container/gamemode/native-motion at `main.rs:3158–3178`, ignores respawn
action 0 at 3575, and accepts container publications using only numeric ID and
revision at 3827. Qualify both publisher and consumer with the real generation;
old menu events must not target the replacement or suppress its lower revision.

`PlayerMotion` combines physical state and listener `MovementState`
(`main_player_motion.rs:148`). Respawn replaces actor physical/body state while
retaining this connection listener's movement counts, clock, teleport sequence,
pending confirmation and block ACK owner. Do not call the fresh-login initializer
at `main_player_motion_hooks.rs:88`, which reserves teleport ID 1. Invoke the
existing listener teleport at its source respawn callsite instead. Preserve
chat/signature/rate/inactivity/cookie/transport owners. PlayerList removes and
appends the actor; update `main_chat.rs` audience registration order without
removing/recreating its retained signature-cache owner.

## Output, save contract and smallest assignment

Enter the existing central operation boundary in `chat_dispatch` →
`player_admission` order before world → players. Holding only the player write
lock does not serialize callbacks that release it against chat/admin output.
No async lock acquisition belongs inside a synchronous callback holding those
locks. Use one actual ordered connection output path, retaining committed state
and consumed RNG if terminal delivery preempts later frames.

PlayerList.respawn 469–530 requires: remove old list/world body; resolve spawn;
construct actor; retain connection; restore; reuse old wire ID; move; optional
missing-spawn event; Respawn → teleport → default spawn → difficulty → XP →
active effects → level info → permissions; destination insertion → PlayerList
append → UUID replacement → inventory-menu initialization → setHealth(getHealth);
then dead-anchor sound. Retain destination separately from later current-level
and level-data locals, and reread connection/actor fields at source callsites.
Level info uses Overworld border, destination time/spawn/weather,
LEVEL_CHUNKS_LOAD_START and tick-rate update. The anchor seed comes from retained
destination RNG; item drops, XP awards, loot named streams, entity RNG and global
Math.random remain with their original owners. An empty player loot table is
not permission to omit registry/context/random-stream lifecycle.

`main.rs:1007::capture_player_locked` must capture current canonical health,
inventory, stats and the source saved subset. `player_nbt.rs:101/188` already
handles DeathTime; model supported HurtTime/HurtByTimestamp, last-death location
and spawn dimension/position/angle/forced fields rather than preserving stale
unknown copies after mutation. Do not persist transient combat history,
removal/dead flags or spawn/cooldown timers as arbitrary player.dat fields.
Reconcile Java signed-short DeathTime loading with the current nonnegative
validation. A saved health-zero player cannot silently receive fresh health.

The minimum useful central assignment has two reviewable patches before the
producer is enabled: (1) canonical actor initialization/identity, handle-qualified
body retirement and retained-listener/session replacement seams; (2) actual
damage/death/tick/removal/loot/restore/command host composition, ordered output
and source save mutations in those seams. Name exact Main/body/motion/food,
entity/index/replication and persistence functions in the grant. Team source
executors alone do not authorize editing those files.

Acceptance needs a real retained connection through lethal damage, full death
side effects, death-time removal and command-triggered new generation with the
same wire ID; source-ordered packets, real inventory/XP changes and save/reload
must agree. Include stale old-generation/menu callback rejection and preserved
teleport progression. Existing compiler checks, source reviews and controlled
probes support the exported fragments; none supplies that Main proof. Generic
LivingEntity/Player death, arbitrary damage/effect/AI contexts, mobs and whole
entities/survival deliveries remain outside this bounded milestone.
