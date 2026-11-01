# Native retained-object effect tick

Instance: es-20261101-1425-a7c3; issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Implementation base: `47182b236135a295ab17a2d64aaedd425f960869`.
Owned: new `src/effects/retained.rs`, internal helpers in owned
`src/effects/instance.rs`, its export in owned `src/effects/mod.rs`, this note.

Pinned Minecraft Java1.21/protocol767 original server SHA1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1
`31c77994d96f05ba25a870ada70f47f315330437`; inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

Evidence: direct javap of pinned unchanged Java 1.21 server inner jar `brz`
and `brz$a`, bound by official mappings to MobEffectInstance/BlendState.
Inputs/outputs remain in scratch/oracle/brz.javap and brz-blend.javap. No REA
execution or new original-Java runtime execution is claimed for this stage.
The existing original-Java nested-hidden-chain probe remains separate evidence.

- MobEffectInstance.tick217-218, bytecode0-25: hasRemainingDuration (-1 or >0);
  if infinite, actual actor tickCount field; otherwise current duration; store
  that predicate argument in local3. The actor field is not read for finite or
  exhausted instances.
- tick219, bytes26-46: resolve actual effect holder value and read current
  amplifier for shouldApplyEffectTickThisTick. This call can complete before
  further state is read.
- tick220-221, bytes49-80: resolve effect definition again and read amplifier
  again for applyEffectTick(actor,amplifier). If false, immediately call actor
  removeEffect(actual holder), not removal by retained object identity.
- tick224-228, bytes81-120: recursively decrement hidden durations before own
  duration, preserving -1 and0; if own becomes0 and hidden exists, copy its
  details then advance the hidden link, then run the captured promotion
  callback. The retained original object continues after map removal/replacement.
- tick232-233, bytes125-137: tick blend state even when entry began exhausted;
  then re-evaluate hasRemainingDuration for the returned boolean.
- BlendState.tick394-407: previousFactor=factor before first definition lookup;
  save first blend duration. Zero => factor=1 and return. Otherwise target()
  resolves definition blend duration AGAIN (410-415) and tests the instance's
  current duration. Only after this second lookup use current factor and target
  for step clamp, with step=1f/first saved duration. Source float order remains
  shared with existing monolithic tick.

Implementation: `tick_retained(actor,lease,host)` uses callback-free
`with_instance` closure fragments. Every closure returns before a definition or
actor callback executes. The host supplies an actual object-retention lease;
lookup through the mutable current effect map is explicitly forbidden. K::clone
retains canonical holder identity. No actor, map, registry, RNG, scheduler or
attribute owner is created here.

Focused native evidence: compile actual instance.rs + retained.rs via the
external scratch/retained-test-harness.rs using rustc --test, then execute the
result. Existing instance checks and retained checks pass. Retained checks use
real strong-reference object leases and assert every object can be mutably
borrowed inside each callback; they update and recursively tick the same actual instance, replace
and remove the current map entry, resume the old retained instance and its
promotion, preserve committed mutation/promotion/blend prefixes on failures,
check finite/infinite tickCount reads, and compare callback trace + state to the
existing monolithic source runner for nested hidden effects. The mock collection
has one entry and makes no Java HashMap iteration-order claim.

Production: the namespace export is present; real host wiring remains central work.
This is one source-backed retained instance primitive, not tickEffects collection
ownership, effect definitions, full Java hidden-object alias graphs, or a whole
survival_combat/entities checklist delivery. No new Main proof is claimed.

## Canonical actor/collection contract

`RetainedEffectTickHost<K>` requires actual `Actor`, retained `Lease` and all
source callbacks. Actor keys must retain the same original actor identity and
lifetime through nested calls/removal, not retarget a reused entity ID. A lease
must resolve the same effect object independently of the current live map.
The API creates neither owner. There are no successful missing-operation defaults.

A source-shaped host may use a real arena with retained generational handles;
it borrows only within `with_instance` fragments, returns, then reborrows its
whole actual context for callbacks. Recursive tick of the same retained object
is supported and checked; Rust RefCell in the test context is never borrowed
across a callback. Holding an actual actor/object borrow across callbacks would
violate the contract. Lookup failure must occur before executing a fragment;
completed fragments and callbacks are never rolled back on later error.

The helper extraction shares countdown/promotion/blend arithmetic with the
existing public `EffectInstance::tick`; its API stays compatible. This avoids
two independently drifting arithmetic implementations. Existing unique-owned
Box hidden links remain a bounded representation: arbitrary Java constructor
alias graphs/shared hidden instances are unsupported.

Independent pinned-source review and separate actual-file compilation/flows
found no blocking mismatch. Existing and retained focused flows cover nested
hidden promotion, finite/infinite/exhausted tick reads, callback mutation,
recursive ticks, replacement/removal and failures. These are controlled native
contexts, not a real world, effect definition, attribute or packet invocation.
Full crate results are recorded on the PR/canonical instance status.

See [the full collection/definition/attribute contract](entities-survival-effects-integration.md)
for exact add/update/remove/iterator/persistence/Main prerequisites. In
particular, tickEffects removes expired entries before virtual removal;
applyTick false immediately removes the current holder map value, yet execution
resumes the old retained object. Those are different removal paths. This
primitive does not replace either actual collection owner or its callbacks.
