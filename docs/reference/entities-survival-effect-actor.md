# Actor effect collection operations — Nov 2

Instance es-20261101-1425-a7c3; root reports issue4 claim6566070764.
Only source written: /workspace/cinderstone-effects-state-es-20261101-1425-a7c3/src/effects/actor_ops.rs.
Root owns exports, docs, Git, full Cargo and publication. Implementation base is refreshed master26c4d2ec13b885e51025e105f1edeb0fe78f6309.

Evidence: unchanged original Java1.21/protocol767; server SHA1 450698d1863ab5180c25d7c804ef0fe6369dd1ba; official mappings SHA1 31c77994d96f05ba25a870ada70f47f315330437; inner server SHA256 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
Direct javap/official mappings, not an REA run or new original-Java runtime observation.
Raw evidence remains scratch/oracle/btn-tracking.javap.txt + brz.javap and existing effects reference notes.

## Exact APIs

InstancePair<'a,K>::Same(&'a mut EffectInstance<K>) or Distinct{target:&'a mut EffectInstance<K>,source:&'a EffectInstance<K>}.
ActorEffectHost<K>: required Error,Actor:?Sized,Entity:?Sized,Lease,ValuesCursor.
Short with_instance read and with_instance_pair update/blend fragments; actual synchronous mismatch diagnostic passed separately into the latter. Every actor/definition callback runs after these accesses return. Canonical identity selects Same; never holder/value equality or map-key lookup.

add_effect<K:Clone+PartialEq,H:ActorEffectHost<K>+?Sized>(actor:&H::Actor,incoming:&H::Lease,cause:Option<&H::Entity>,host:&mut H)->Result<bool,H::Error>.
force_add_effect<K:Clone,H:ActorEffectHost<K>+?Sized>(same args)->Result<(),H::Error>.
remove_effect_no_update<K,H:ActorEffectHost<K>+?Sized>(actor,holder:&K,host)->Result<Option<H::Lease>,H::Error>.
remove_effect<K,H:ActorEffectHost<K>+?Sized>(actor,holder,host)->Result<bool,H::Error>.
remove_all_effects<K,H:ActorEffectHost<K>+?Sized>(actor,host)->Result<bool,H::Error>.

## Pinned source facts

LivingEntity.removeAllEffects944–955: current Level client test returnsfalse before iteration; obtain actual activeEffects.values iterator; hasNext/next -> actual virtual onEffectRemoved(selected) WHILE entry visible -> iterator.remove -> changedtrue. No CME catch. Returnfalse when no visits. Next/remove retain their source structural modification checks; hasNext does not acquire one.

addEffect980–1000: virtual canBeAffected first; false stops all map/start work. Read incoming holder for get; retain selected existing instance. Absent: read incoming holder AGAIN, put exact incoming FIRST (discard previous return), virtual actor.onEffectAdded(incoming,cause), changedtrue, actual virtual instance.onEffectAdded(actor). Existing: native existing.update(incoming); only true invokes actor.onEffectUpdated(existing,true,cause), changedtrue. BOTH accepted branches then instance.onEffectStarted(INCOMING), even if update returnedfalse and mutated hidden chain. Return flag. Callback replacement/removal does not retarget retained existing or incoming.

forceAdd1017–1028: virtual canBeAffected; put exact incoming, retaining actual previous value. Absent->virtual onEffectAdded only. Present->incoming.copyBlendState(previous), virtual onEffectUpdated(incoming,true,cause). Neither instance.onEffectAdded nor onEffectStarted on this source path.

removeEffectNoUpdate1036 base body only: activeEffects.remove(holder), retaining returned exact object; no callbacks.
removeEffect1040–1045: actual VIRTUAL removeEffectNoUpdate dispatch; nonnull returned object -> onEffectRemoved(returned),true; null->false. It does not bypass an override by directly calling map_remove.

Update/copy blend: actual native EffectInstance methods invoked in Distinct short access. Same-object update is provablyfalse without writes: identical effect, amplifier, duration, flags make all source comparisons false, no warning/hidden update. Same blend self-copy leaves both fields unchanged. Same branch borrows canonical object once: no RefCell double-borrow, deepCloneHidden or move-out. Existing top-level alias support does not claim arbitrary public Java hidden-object alias graphs supported by current Box representation.

## Validation and explicit limits

rustfmt applied. Standalone actual-file rustc1.85 --edition2021 -Dwarnings --test and ordinary rlib compile passed using scratch/actor-ops-focused.rs. Focused actor_ops filter passed. The scratch harness allows unrelated dead-code lints from the isolated existing instance module; repository source receives no such lint allowance.

Controlled real EffectInstance/Rc retained-object tests cover insertion visibility and reentrant force replacement; source original incoming callbacks after replacement; false update mutating hidden chain with incomingStarted; true same-object alias, retained hidden allocation and no borrow conflict; force blend copy/no start callbacks; canBeAffected rejection; virtual remove-before-notify; live iterator callback-before-removal and nonstructural replacement removal; no structural CME catch; client/empty exits; update/installation retained after later callback errors. Every actor callback checks that all test instance objects can be mutably borrowed.

Test iteration is a controlled supplied key schedule/structural-change fixture, not an implemented Java HashMap or iteration-order proof. No production actor/map/cursor/holder hash policy/definition/attributes/packet/RNG/session/scheduler is allocated. All missing owners remain required fallible host callbacks. No concrete effect callbacks or tickEffects/baseTick are supplied here; root owns tick.rs. No Main/whole effects/survival completion or original-Java actor runtime proof is claimed. Earlier mutations survive errors, with no rollback or whole-operation retry.


# Root tick.rs independent source review and assigned end-only checks

Read-only production review against pinned btn.tickEffects808–848 and private updateGlowingStatus873–877 found no material mismatch. Exception table15..108 covers only loop, iterator creation0..14 outside. Retained instance tick, expiration logical-side gate, iterator.remove before old-instance onEffectRemoved, else duration%600 updatefalse/null all match. Dirty server invisibility/privateglow then clear even client; private glow captures current result before shared flagread. Particle metadata list retained once; empty check before ambient/invisibility and RNG; (15 or4)*(5 or1) actor RNG bound. On success source captures Level before Util.getRandom list/RNG arguments; actual RNG captured before list.size; list.get before X/Y/Z calls; velocities1,1,1. No root production-body edits.

Root granted only an appended #[cfg(test)] module to src/effects/tick.rs. Module begins after final production function; exact production prefix preserved against scratch snapshot (normalized terminal newline only). Test module formatted in a separate scratch file, without formatting root's production body. Only actor_ops.rs and this assigned test suffix were changed by this worker.

Actual-file harness nov2/tick-focused.rs includes repository EffectInstance + retained.rs + tick.rs. rustc1.85 -Dwarnings test and rlib compilation pass; focused effects::tick::tests filter passes. Actual retained tick runs on strong Rc object leases. Every actor/definition callback checks all selected instances are mutably borrowable, preventing crossed source callbacks with held object borrows.

Controlled checks: apply callback replaces same map key nonstructurally, then old object expires; iterator removes replacement while onEffectRemoved gets retained old object. Client expiration leaves entry but still decrements old duration and clears dirty without server visibility/glow. Hidden601->600 countdown/promotion on retained object precedes periodic600 callback. CME thrown by nested periodic callback inside loop is swallowed and dirty/tail continue; iterator-creation CME, dirty visibility CME, tail CME, null map dereference and other loop error all stop at source boundary. Dirty clear follows visibility and captured-glow/sharedflag/set order despite callback mutation. Particle test mutates actor Level/list/RNG after capture: original Level/list and second captured RNG are used, coordinates use then-current random owner; exact gate/selection/X/Y/Z/add order and all ambient/invisibility bounds/empty-list draw skip checked.

The map/cursor, metadata, Level, RNG and callbacks are controlled supplied fixtures, not implementations/proof of actual HashMap order, native world/actor/definition or Main behavior. No source pin/REA/runtime-overall claim beyond pinned static review and described native component composition. Full Cargo/export/docs/Git remain root-owned.

## Root callable collection tick and production boundary

NEW effects/tick.rs exports LivingEffectsTickHost, LivingEffectsTickError,
tick_effects and update_glowing_status. It directly composes the existing native
tick_retained with a real live key iterator, exact loop-only CME catch, retained
old-instance expiration/periodic callbacks, then dirty visibility/glowing and
source metadata-list/RNG/Level particle dispatch. It creates no active map,
canonical holder/hash, actor, definition, attribute, RNG, world or scheduler.
Every missing source owner is a mandatory fallible host method.

Pinned original: server1.21 SHA1 450698d1863ab5180c25d7c804ef0fe6369dd1ba,
official mappings SHA1 31c77994d96f05ba25a870ada70f47f315330437, inner JAR SHA256
8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.
Source btn808–848 and873–877; direct JDK21 javap only for these method bodies,
not an REA/MCP or original-Java actor runtime. Independent tick body reviews
by tracking_persistence and mob_navigation found no material mismatch. Root
reviewed actor_ops against original add/force/remove/removeAll methods.

Actual concrete onEffect* handlers, definition callbacks, passenger/self packet
audiences and source updateInvisibilityStatus overrides remain required actual
virtual operations. The source Added handler applies attributes and sends only
to direct ServerPlayer passengers without immediate attribute refresh; Updated
and Removed do refresh at their real source points. Those callbacks must not be
implemented as successful empty closures. Current Main has no callsite into
these collection runners and still rejects an actively effected player body.
Beacon source modules can request native addEffect but do not create that owner.

Root combined source compiler/runtime results, exact commits, CI/merge state
and pending actual Main contracts are tracked in the instance status and PR.
Controlled native callback tests establish these source algorithms under their
explicit owner contract; they do not complete whole effects/entities/survival.
