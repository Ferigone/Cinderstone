# Java 1.21 effect-instance lifecycle

Team issue: [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Instance: `es-20261101-1425-a7c3`.
Implementation base: `692f9d2406ebd7fd2462f0c708c56d179b7da9de`.
Scope: callable active/hidden `MobEffectInstance` update, duration and blend
state. This is a partial `survival_combat.4` foundation. No entire entity,
damage/death/respawn, potion/attribute or mob delivery is complete.

## Evidence identity and method

The artifacts match `fixtures/source-lock.json`:

| Input | Verified identity |
| --- | --- |
| Original server, 51,623,779 bytes | SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba` |
| Official mappings, 7,454,609 bytes | SHA-1 `31c77994d96f05ba25a870ada70f47f315330437` |
| Extracted unchanged inner server | SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01` |

Analysis used JDK 21 direct `javap -c -p -l`, invoked through the installed
`jdk.jdeps/com.sun.tools.javap.Main` module. Source line numbers below come from
the official mappings and the original class line tables. REA tools were not
exposed; no REA run or Evidence ID is claimed. Java artifacts and raw JVM
disassembly remain outside Git. Static inspection does not establish execution.

Mapped classes: `MobEffectInstance` = `brz`, its `BlendState` = `brz$a`,
`MobEffect` = `brx`, `LivingEntity` = `btn`, `ServerPlayer` = `aqv`,
`ServerEntity` = `aqt`, `PlayerList` = `aur`, `Mth` = `ayo`.

## Implemented source behavior

`src/effects/instance.rs` keeps an opaque canonical holder key, duration,
amplifier, ambient/particle/icon flags, a hidden-instance chain and blend state.
It does not allocate actor identity, create a health owner or supply an AI,
scheduler, RNG or active-effect map.

- Constructors (Java 55–90): amplifier is clamped to 0–255; duration is retained
  as supplied. Exactly `-1` means infinite. Java's copy constructor copies
  details but omits hidden instances and resets blending. Native `Clone` is an
  explicitly distinct deep snapshot; `copy_instance` models the Java constructor.
- `update` (125–164): a stronger, shorter effect preserves the old details in
  front of the old hidden chain, then replaces active amplifier/duration. An
  equal-amplifier longer effect extends duration. A weaker longer effect updates
  or inserts the hidden chain and can return **false despite changing it**.
  Ambient changes obey the source's asymmetric rule; visibility and icon flags
  are updated separately. Mismatched keys warn synchronously and still merge,
  retaining the original key, as Java does.
- Duration predicates/mapping (168–183, 237–244): zero and infinite duration
  bypass mapping. Hidden durations count down recursively before active duration;
  Java integer subtraction wraps. No hidden promotion occurs if an instance
  enters `tick` with no remaining duration.
- `tick` (217–233): with remaining duration, select actor `tickCount` for an
  infinite instance or current duration otherwise. Call the definition predicate,
  then its effect callback when selected. A false effect result immediately calls
  the actor's `removeEffect`, **before countdown**. Continue on the retained
  instance, recursively age the chain, then promote hidden details when the
  active duration reaches zero. Advance the hidden link before the promotion
  callback. Finally tick blending even for an exhausted instance, then return
  whether duration remains. The return value alone does not describe map
  membership: a synchronous callback may already have removed the entry.
- Blend methods (372–426): keep previous/current factors; resolve registered
  blend duration, with zero duration setting current factor to one. Otherwise
  approach the post-countdown target by a clamped `1 / blendDuration` step.
  Resolve the target's duration lookup separately as Java does. `getBlendFactor`
  collapses previous to current when the actual actor is removed, then uses
  float subtract/multiply/add interpolation (`Mth.lerp`, 631). Copy and immediate
  blending are distinct operations.

`EffectTickHost<K>` has no default implementations. It requires the registered
definition predicate, actual effect execution, synchronous removal, promoted
instance update callback and registered blend-duration lookup. Effect execution
and removal receive the retained mutable instance, so state changes at those
callbacks precede countdown. Host errors stop at that point and preserve prior
mutations/side effects; callers must not retry the whole tick as a transaction.

## Actual actor integration contract — pending

Callable surface:

```rust
EffectInstance::<K>::new(effect: K, duration: i32, amplifier: i32)
instance.update(&incoming, |current_key, incoming_key| { /* real warning */ })
instance.tick(actor_tick_count: i32, host: &mut impl EffectTickHost<K>)
    -> Result<bool, HostError>
```

Additional constructors accept the source flags and hidden chain. Accessors,
`copy_instance`, `copy_blend_state`, `skip_blending` and `blend_factor` expose
the corresponding bounded instance operations. A publication follow-up adds only
`pub mod effects;` to `src/lib.rs` under the coordinator's narrow standing
mechanical exception. It introduces no actor owner or Main callsite and changes
no manifest. Actual actor integration remains with the coordinator.

The caller must own the actual canonical holder-keyed collection and retain the
same instance through synchronous map removal. A detached clone or buffered
action list is insufficient. Borrowing/retained-instance mechanics, actor
admission and error handling must be reviewed with the central owner before
installing a production host.

### Correct tick phases

`LivingEntity.baseTick` (395–506) runs server-side **enchantment** tick effects
at 403 before `Entity.baseTick` at 406. That phase is distinct from potion
effects. Environmental callbacks follow; hurt time decrements at 469–470,
invulnerability time at 472–473 except for `ServerPlayer`, death ticking at
475–476, attacker cleanup at 478–491. Potion `tickEffects` runs at 496 before
old animation/rotation snapshots. Placing it immediately after `Entity.baseTick`
would change callback order.

`LivingEntity.tickEffects` (808–848) iterates the actual Java HashMap key set;
there is no sorted-ID iteration guarantee. It calls the instance with a callback
to `onEffectUpdated(instance,true,null)`. Expiration removes the map entry before
`onEffectRemoved` on the server. Retained duration multiples of 600 cause
`onEffectUpdated(instance,false,null)`. The loop catches concurrent modification.
Dirty state then refreshes server invisibility followed by glowing, clears the
dirty flag, and finally performs particle selection using the actual actor RNG.
The instance implementation does not implement this outer lifecycle.

### Add, update, removal and attributes

`LivingEntity.addEffect` (980–1000) checks eligibility. A new instance is put
before `onEffectAdded` and its definition's added callback. A changed existing
instance calls `onEffectUpdated`. Every eligible addition, including an unchanged
merge, ends with the **incoming** instance's `onEffectStarted`. `forceAddEffect`
(1017–1028) has distinct replacement/blend-copy semantics.

Ordinary `removeEffect` removes before its callback (1040–1045), whereas
`removeAllEffects` calls each removal callback before iterator removal
(944–955). `removeEffectNoUpdate` removes without that callback (1036).

Living added/updated/removed callbacks (1049–1088) mark effects dirty and mutate
the actual server attribute map. Reapplying an updated effect removes modifiers,
adds them at the new amplifier, then refreshes dirty attributes. Removal removes
modifiers and refreshes attributes. `onAttributeUpdated` (1099–1110) clamps
current health and absorption to their actual maxima. These callbacks are not
substituted by instance duration state.

For Players, canonical health remains synchronized metadata field 9, absorption
field 15; reuse `EntityPhysicalState.invulnerable_time` and the real actor RNG.
Effect damage requires the genuine living damage-source/tag/callback executor.
The existing arithmetic and creative immunity return do not supply that owner.
See the coordinator's [damage contract](../coordination/damage-integration-contract.md).

### Packet and persistence owners

Living callbacks send update/removal packets to ServerPlayer passengers after
their attribute work. ServerPlayer overrides (1471–1500) call superclass first,
then send the self packet, then levitation bookkeeping and advancement callbacks.
Addition uses blend=true; updates use false. Update packet fields (Java 28–64)
are entity ID, canonical effect holder, amplifier, duration and flags
(ambient=1, particles=2, icon=4, blend=8).

`ServerEntity.sendPairingData` (271–325) does **not** replay active effects to
ordinary observers. `PlayerList.sendActiveEffects` (538–541) sends actual
instances with blend=false during login, respawn and living-vehicle mounting.
Unmounting removes that old vehicle's effects after the superclass operation
(`ServerPlayer.stopRiding`, 1993–2000). Do not invent an observer replay phase.

Living save writes attributes before nonempty `active_effects` (723–731).
Load restores attributes, directly decodes and puts instances into the map,
then loads health (750–766); it does not invoke runtime `addEffect` callbacks.
Native save must snapshot the real collection under its actor lock and restore
it before admission. Metadata particle/ambient fields cannot reconstruct it.

## Repository boundaries

At this base, Main has no active-effect map or potion-effect tick phase.
`src/player_body.rs` explicitly certifies an alive creative profile with no
active effects. `PlayerLifecycle` has no effect owner. Java player import
rejects nonempty `active_effects` and modifiers; export retains unowned original
NBT fields, so merely lifting this gate could preserve stale effect records.
Milk/honey and food callbacks currently rely on the empty-effect profile.
Those gates remain unchanged.

No native mob owner, living combat executor, effect definition catalog,
attribute lifecycle or effect persistence/packet adapter is introduced here.
The remaining instance forwarding callbacks, serialization/codecs, ordering,
equality/hash and presentation APIs are outside this bounded implementation.
No whole delivery or Main integration is claimed.

## Focused validation

The dependency-free namespace compiles directly with Rust 1.85.1 using
`rustc --edition=2021 --test src/effects/mod.rs`. Focused native checks pass for
stronger-shorter promotion, hidden-only update, Java copy versus snapshot,
infinite tick arguments, synchronous removal/error prefixes, exhausted blend
and post-countdown blending. This is callable component validation; there is no
connected-player, production Main, damage or mob runtime result.

The final status record supplies the exact implementation commit and any
validation output identity. Comprehensive conformance is deferred.
Compiler/test/agent counts are not progress measures.

A separate controlled probe loaded the unchanged signed inner Java JAR normally
through packaged reflection. It used finite vanilla FIRE_RESISTANCE instances;
inspection and runtime checks established that this holder's exact base
`MobEffect` has `shouldApplyEffectTickThisTick=false`. Thus finite
`tick(null, Runnable)` never reads the actor or executes effect application.
Infinite instances were updated but never ticked in this probe.

Observed original-Java results, also asserted by the focused native flow:

| Operation | Observed result |
| --- | --- |
| Update `20 ticks / amplifier 0` with `5 / 1`, then `3 / 2` | Active/hidden chain `3/2 -> 5/1 -> 20/0` |
| Three finite ticks | Promotion callback sees `2/1 -> 17/0` |
| Five total finite ticks | Next callback sees `15/0`, no hidden link |
| Final countdown | Duration zero; tick returns false |
| Update `5/2` with `20/0` | Returns false while inserting hidden `20/0` |
| Java copy constructor | Hidden chain omitted |
| Stronger finite over infinite | Old infinite duration retained hidden |
| Weaker infinite under finite | Hidden infinite inserted; update returns false |
| Equal-amplifier finite to infinite | Returns true; duration becomes `-1` |
| Equal-amplifier finite update of infinite | Returns false; infinite retained |

Probe compiler and JVM exit codes were zero. Probe source SHA-256:
`4a3e4badcce6c83b3e48d5bcd3571925b327d509c3e51c90e35b0655085a057a`;
output SHA-256:
`13e9c39bd0a7a5d754e55887f99d1a55fd24bb41ce5e9bb1ff9de6cc8e9d1033`.
These local artifacts remain in the instance's scratch oracle directory. This
probe covers update/copy/countdown/promotion/expiration and infinite update
relations only. It exercises no actor, world, damage callback, active map,
attributes, packets, persistence or production Main path. Other host callbacks
and blending have static source evidence plus native component checks, not an
original-Java actor runtime result.
