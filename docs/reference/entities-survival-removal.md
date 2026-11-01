# Native living and Player removal

Instance: es-20261101-1425-a7c3; issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Source base: `c1baf5c70f07e53b6b76edaf84be5da57f6bd40b`.
Pinned original Minecraft Java1.21/protocol767 server SHA1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1
`31c77994d96f05ba25a870ada70f47f315330437`; unchanged inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence is direct JDK21 javap and specifically labeled controlled runtime
observations, not an unperformed REA run. Raw disassembly, Java binaries and
probe artifacts stay outside Git. No actual Main invocation is inferred.

Instance: es-20261101-1425-a7c3; exact new file: src/living_entities/removal.rs
Worktree: /workspace/cinderstone-living-next-es-20261101-1425-a7c3
Base: c1baf5c70f07e53b6b76edaf84be5da57f6bd40b
Claim: Team #4 comment6563958691. No Git/module/docs/Main changes by this agent.

Public API:
- RemovalReason::{Killed,Discarded,UnloadedToChunk,UnloadedWithPlayer,ChangedDimension}.
- LivingRemovalHost {Error,Actor}; remove_living(actor,reason,host)->Result.
- DeathEffectsHost {Error,Actor,Cursor,Lease}; trigger_on_death_effects(actor,reason,host)->Result.
- DeathEffectInstanceHost<K> {Error,Actor,Lease,Definition}; notify_effect_instance_removed<K:Clone>(actor,retained,reason,host)->Result.
- PlayerRemovalHost {Error,Actor,InventoryMenu}; remove_player(actor,reason,host)->Result.
All actual operations mandatory/fallible; no default successful stubs. No actor/collection/menu/Brain/removal state owned by executor.

Pinned source (direct JDK21 javap + official mappings; no REA run):
- LivingEntity btn.remove700-706: passed reason KILLED or DISCARDED => virtual triggerOnDeathMobEffects; invokespecial Entity.remove; current Brain.clearMemories.
- LivingEntity btn.triggerOnDeathMobEffects709-713: actual getActiveEffects values iterator; hasNext/next; canonical selected instance.onMobRemoved(actor,reason); after completed iteration, current activeEffects.clear. No removeAllEffects, onEffectRemoved, attribute callbacks or ConcurrentModificationException catch.
- JDK HashMap$HashIterator: hasNext tests prefetched next only; next checks modCount then reads selected node value and prefetches successor. Collection callback changes can therefore fail at next, while structural mutation after final next can still reach clear. Cursor is actual-owner contract, never snapshot/sorted plan.
- MobEffectInstance brz.onMobRemoved252-253: canonical holder field -> holder.value resolution -> retained instance amplifier reread -> actual definition.onMobRemoved(actor,amplifier,reason). Short state borrows end before callbacks; original selected object remains retained through map replacement/removal. Trigger preserves virtual dispatch; separate helper implements base body only when selected.
- Entity bsr.remove415-416 invokes setRemoved3702-3710. Parent remains mandatory actual-owner operation: install first removalReason only if absent, consult stored reason.shouldDestroy for stopRiding, stop passengers, in-level removal callback with passed reason. No bool or registry deletion substitution; repeated calls do not suppress source parent body.
- Player cmx.remove1450-1456: invokespecial LivingEntity.remove; current inventoryMenu.removed(actor); reread current containerMenu nullness; virtual hasContainerOpen; virtual doCloseContainer.
- Player cmx.hasContainerOpen1485: current containerMenu != current inventoryMenu object identity, not window ID/opener count. Parent/removed callbacks may reenter and change fields before later reads.

Focused native validation:
Harness: /workspace/scratch/es-20261101-1425-a7c3/removal-focused.rs (only existing effect instance + new module; no Cargo/full Main).
Standalone rustc --edition=2021 --test compilation passed. Filter removal::tests:: passed.
Meaningful checks: reason-specific effect gate and map-clear/parent/Brain order; live-cursor concurrent modification failure vs structural mutation after last node; retained instance identity across map replacement + amplifier reread after definition resolution; synchronous nested removal with all mutable context available; effect/parent callback errors preserve prefixes; player menu identity/nullness read after actual inventory removed callback.
The cursor/actor/menu tests use explicit controlled recorders, not an implemented native HashMap or production actor. Harness emits only an unused existing effect-instance copy_blend_state warning.

Limits:
No actual active-effect map/cursor implementation, effect definitions, generic Entity.setRemoved body, Brain, menu/drop/packet adapters, tracking/persistence or Main callsites provided. These remain actual-owner contracts, and no full entity/survival delivery is claimed. A new actor must not be fabricated to satisfy them. No physical removed/alive flag, reason field, health, timer, RNG, connection, scheduler or map introduced here. Missing owners must error at their own callsite and earlier effects must not be rolled back or retried atomically.

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
