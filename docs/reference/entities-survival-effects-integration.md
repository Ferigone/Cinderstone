# Native effect collection and attribute integration contract

This note specifies the requested central integration contract. It does not claim implemented collection or Main behavior. Evidence is direct javap
of the pinned original Java 1.21 jar, with official mappings. Source inspection
outputs are effect-contract-{but,bus,brx,aqv,absorption}.javap plus
 effects-damage-living.javap, effects-damage-player.javap and brz.javap in this
instance's scratch/oracle directory. HashMap iterator implementation was also
inspected directly from this installed JDK21 (21.0.12.1), in
 effect-contract-jdk-{hashiterator,keyiterator}.javap. That is JDK-library
evidence, distinct from the pinned game jar. No REA execution is claimed.

## Concrete state and existing authorities

Proposed central field in the existing NativePlayerMotion (same entity ID/UUID
lineage as its physical/body fields):

```
living: NativeLivingState {
    active_effects: EffectCollection,
    effects_dirty: bool, // constructor true, LivingEntity field239
    attributes: AttributeMapOwner,
    // existing/new source-backed hurt/death fields may be composed separately
}
```

EffectCollection owns a JavaHashMap-shaped holder-to-EffectObjectKey map, and
stable generational effect-object storage or a central shared arena. A retained
EffectObjectKey survives removal from the map until the current source call
returns. Keys are canonical bound holder identities, not registry integer IDs.
A source-compatible map representation needs table/bucket nodes, size,
threshold/load factor, wrapping mod_count and insertion/removal history; each
iterator retains next/current node, bucket index and expected_mod_count. Tree
bins/collision behavior cannot be dropped without a source-supported boundary.
Each effect object has its exact duration/amplifier/flags/hidden/blend fields.
Do not copy an effect object merely to give a callback access to the actor.

Keep these current owners authoritative:

- PlayerState.entity_data[9]: health; PlayerEntityData[15]: absorption. Player
  getAbsorptionAmount/internalSetAbsorptionAmount override uses synchronized
  metadata (Player2041-2046), whereas ordinary LivingEntity stores its ci field
  (LivingEntity3105-3114). No second permanent HealthState copy for players.
- PlayerState.entity_data[10]/[11]: effect particle list / all-ambient flag;
  actual entity flags carry invisibility/glowing. These are presentation, not
  proof that the authoritative effect collection is empty or alive.
- NativePlayerMotion.motion.physical.tick_count and position/physical owner;
  LivePlayers.player_random[id] is the current actual player entity RNG.
- Actual inventory remains the existing player/container inventory; equipment
  snapshots from PlayerEquipment are comparisons/presentation and cannot
  become a second authoritative set of mutable stacks.
- PlayerLifecycle remains hunger/abilities persistence authority. Motion's
  PlayerMotionAttributes must become a projection read from this one attribute
  map, not an independently modified second attribute system.

AttributeMapOwner must retain: immutable actual EntityType supplier binding;
instances (holder -> AttributeObjectKey); attributes_to_update and
attributes_to_sync; the actual instance arena. Source but24-31 uses fastutil
Object2ObjectOpenHashMap and ObjectOpenHashSet, not Java HashMap or BTreeMap.

An AttributeInstance object (bus26-38) owns: canonical attribute holder,
base_value f64, dirty bool, cached_value f64; EnumMap over three operations to
Object2ObjectOpenHashMap<ResourceLocation,ModifierObjectKey>; modifier_by_id and
permanent_modifiers Object2ObjectArrayMaps. Modifier objects need identity in
addition to {ResourceLocation id, amount f64, operation}, because
addOrUpdateTransientModifier84-90 returns early only on same object reference.
It does not clean an old operation bucket when a replacement changes operation;
do not repair source state into a stronger invariant. Per-operation fastutil hash iteration
order is part of arithmetic, not merely serialization.

## Actionable APIs requested from central adapter

Bind actor identity once to the real registry/body lifecycle. Proposed APIs:

```
add_effect(ctx: &mut LivingContext, actor: ActorKey,
           incoming: EffectObjectKey, cause: Option<ActorKey>) -> Result<bool>
force_add_effect(ctx, actor, incoming, cause) -> Result<()>
remove_effect_no_update(ctx, actor, holder: EffectHolderKey)
    -> Result<Option<EffectObjectKey>>
remove_effect(ctx, actor, holder) -> Result<bool>
remove_all_effects(ctx, actor) -> Result<bool>
tick_effects(ctx, actor) -> Result<()>
load_living_state(ctx, actor, nbt) -> Result<()>
refresh_dirty_attributes(ctx, actor) -> Result<()>
attribute_value(ctx, actor, holder: AttributeHolderKey) -> Result<f64>
```

LivingContext is a reborrow of actual existing central services, not a new
scheduler: real actor/metadata registry, actual effect/attribute stores,
canonical definition registry, world query/effect executor, player RNG,
passenger graph, packet FIFO and advancement/stat callbacks. Methods must
relookup actual owners by the identity key at callback boundaries.

Definition binding must identify canonical holder, protocol registry ID,
category/color, particle factory, blend duration, optional sound-on-added,
required feature set; its attribute templates are a fastutil
Object2ObjectOpenHashMap<AttributeHolderKey,{modifier id,amount,operation}>
(brx43-67). The actual definition method dispatch must supply shouldTick,
applyTick, onEffectStarted, onEffectAdded, onMobHurt, onMobRemoved and particle
creation. An inherited base no-op is permissible only when established by that
actual definition's source; missing dispatch must not become an empty callback.

Attribute operations must be callable by the definition executor on the same
actor-owned map: getInstance (lazy supplier copy, nullable if unsupported),
removeModifier(id), addPermanentModifier(handle), setBaseValue, getValue,
load/save and dirty propagation. Their callback is synchronous:
AttributeInstance.setDirty108-110 sets dirty then AttributeMap.onModified34-38
adds to toUpdate, then toSync only if actual Attribute.isClientSyncable.

## Exact add/update/remove/load sequencing

LivingEntity.addEffect980-1000:
1. Actual canBeAffected check; false returns without any add/start callback.
   Its tag tests1004-1013 are ordered else-if: IMMUNE_TO_INFESTED / INFESTED,
   then IMMUNE_TO_OOZING / OOZING, then IGNORES_POISON_AND_REGEN. Preserve order.
2. Look up existing live entry. If absent, put the exact incoming object into
   the map FIRST; call virtual actor.onEffectAdded(incoming,cause); then call
   incoming.onEffectAdded(actor) -> definition's onEffectAdded(amplifier).
3. If present, update that exact existing object in place. Only update()==true
   calls actor.onEffectUpdated(existing,true,cause). A false result can still
   change its hidden chain.
4. Call incoming.onEffectStarted(actor) ALWAYS after accepted canBeAffected,
   including unchanged/weaker updates. This is incoming's amplifier, not the
   active existing amplifier. Absorption onStarted22-24 refills absorption using
   Math.max(actual absorption, float(4*(1+amp))) then actual setAbsorptionAmount.

Virtual LivingEntity.onEffectAdded1049-1054: effectsDirty=true; server adds
actual definition attribute modifiers, then sends update packets to actual
direct ServerPlayer passengers. ServerPlayer override1471-1480 calls super
first, sends own update packet with blend=true, records levitation state when
applicable, then actual EFFECTS_CHANGED advancement criterion. Definition
onEffectAdded(sound etc.) happens only after this complete virtual callback.

onEffectUpdated1065-1075: effectsDirty=true; if reapply && server, remove
actual definition attribute modifiers, add at current amplifier, then
refreshDirtyAttributes. Then server sends direct-player-passenger update
packets. ServerPlayer1484-1488 calls super then own blend=false packet then
criterion. Every 600 remaining ticks uses reapply=false, null cause. Hidden
promotion uses reapply=true, null cause (LivingEntity lambda814).

forceAddEffect1017-1028 differs: map.put incoming FIRST; absent => virtual
onEffectAdded; present => incoming copies old blend, virtual
onEffectUpdated(incoming,true,cause). No instance definition onEffectAdded and
no onEffectStarted call on this path.

removeEffectNoUpdate1036 is map removal only. removeEffect1040-1045 removes
first then virtual onEffectRemoved. removeAllEffects944-955 instead iterates
values, calls virtual onEffectRemoved WHILE entry remains visible, then
iterator.remove. It returns false client-side and has no CME catch.

onEffectRemoved1078-1088: effectsDirty=true; server removes definition
modifiers, refreshes dirty attributes, then sends direct-player-passenger
remove packets. ServerPlayer1492-1500 calls super, sends own remove packet,
clears levitation state if applicable and triggers criterion with null cause.

Important persistence distinction: MobEffect.addAttributeModifiers176-184
removes each template ID, then addPermanentModifier (bus103-105). These
modifiers are included in the actual saved attribute instances. Living load
748-766 sets raw saved absorption via internalSetAbsorptionAmount, loads actual
attributes on server, decodes effects and directly puts into live map WITHOUT
add/update/start callbacks or modifier reapplication, THEN setHealth using the
loaded maximum. Constructor effectsDirty remains true. Replaying addEffect on
load duplicates effects or modifies state/packet/callback order. Save717-731
writes attributes before active_effects, effects in live map value order.

refreshDirtyAttributes1091-1096 iterates actual toUpdate set, calls
onAttributeUpdated for each then clears set. onAttributeUpdated1099-1110:
MAX_HEALTH -> if actual health exceeds current max, actual setHealth(max);
else MAX_ABSORPTION -> if actual absorption exceeds max, setAbsorption(max).
No duplicate health/absorption fields. AttributeInstance.calculateValue143-159
adds ADD_VALUE in its fastutil operation-map order; captures that result as base for each
ADD_MULTIPLIED_BASE addition; multiplies sequentially by (1+amount) for
ADD_MULTIPLIED_TOTAL; then actual Attribute.sanitizeValue. Keep f64 operation
order and source range/NaN handling.

## Exact tick and mutation semantics

Call tickEffects at LivingEntity.baseTick496: after hurt-time/death tick and
combat-reference expiry work, before previous animation/rotation snapshots.
Use actual actor tickCount, not connection ticks. It uses activeEffects.keySet
iterator and map.get(key) for each visited key, retaining that object through
MobEffectInstance.tick217-233. Do not move it out of map while ticking.

If instance.tick returns false: server iterator.remove FIRST, then virtual
onEffectRemoved(retained object). Otherwise remaining duration%600==0 calls
virtual onEffectUpdated(retained,false,null). Instance applyTick false calls
actor.removeEffect(holder) synchronously BEFORE recursive duration countdown;
it then continues ticking the retained object, including hidden promotion and
blending even if removed from the map.

Only tickEffects iterator loop810-827 catches ConcurrentModificationException;
it abandons the remaining loop and still performs dirty-state visibility/glow
updates and particle logic. HashMap.hasNext does not check modCount, next and
remove do. Existing-key replacement is not structural. A forceAdd replacing
current key therefore leaves the old retained object ticking; if old expires,
iterator.remove removes the replacement map value but onEffectRemoved receives
old retained object. Snapshotting/sorting keys, queueing reentrant mutations or
retrying an aborted loop changes behavior. Other source collection iteration
paths do not acquire this catch automatically.

If effectsDirty: server calls virtual updateInvisibilityStatus, then
updateGlowingStatus; then effectsDirty=false even client side. Nonempty synced
particle metadata then drives actual entity RNG: invisibility-dependent and
ambient-dependent nextInt bound, random particle selection, actor random
coordinates and world particle callback (tickEffects837-848). Even a server
world callback proved not to send a particle does not eliminate prior RNG.

Death removal is also distinct: triggerOnDeathMobEffects709-713 calls each
retained active instance.onMobRemoved(actor,reason) while map is live, then
clears map; do not substitute ordinary per-effect onEffectRemoved callbacks.

## Safe borrowing and remaining prerequisites

Preferred generic design: stable effect arena handles and source-call
continuations. Retain current handle; borrow only for each callback-free state
fragment; release actor/map/arena borrows; invoke callback with real context,
actor key and exact same effect handle; relookup/resume retained object. Nested
callbacks can mutate same entry or other entries synchronously. Retention keeps
removed objects valid to end of source call. Temporary locals are captured only
where Java captures them; other fields are reread after callbacks.

An alternative can keep monolithic instance.tick if a strictly split arena view
and active-object routing pass its supplied &mut actual current object to every
reentrant operation, including nested damage/collection/attribute calls. The
map entry stays intact and resolves that same handle. This is more complicated
than a step driver. Holding Rc<RefCell<EffectInstance>> mutably across callbacks
and treating recursive borrow failure as unsupported behavior is insufficient.

Hash/registry prerequisite must be explicit. Active-effects HashMap keys are
Holder.Reference objects with inherited Object identity equals/hashCode;
Reference has no equals/hash override in official mappings. Registry ID/name is
not that hash. Correct Java bucket/resize/iterator state needs stable canonical
holder identity, source identity hash policy and map insertion/removal history;
a guessed registry-ID order cannot prove vanilla effect order. Attribute
fastutil maps/sets have their own iteration rules and identity-key hashes.
Direct Holder is a record and has different equality semantics; .is(holder)
also must not be confused with map-key Object.equals. A canonical registry
binding/hash owner and actual library-container order contract are not present
in the current native sources. Do not claim all multi-effect/attribute order is
integrated until central provides or assigns these.

Current EffectInstance<K> uses owned Box hidden links and a monolithic mutable
API; it is a foundation for uniquely owned decoded chains, not proof of full
Java public-constructor alias graphs or unrestricted reentrancy. Full shared
instance/hidden identity requires handle-based object representation or an
explicit source-supported unique-ownership boundary.

Concrete central assignment requested: name exact authoritative actor field
location and initializer; canonical effect/attribute registry/holder/hash owner;
actual physical tick/body insertion point; metadata/health/absorption operations;
player RNG and world callback wiring; inventory/equipment access; passenger and
self packet FIFO/cause/criterion order; native save/load owner. Team can then
implement the handle-based collection, definition executor and attribute state
under new namespaces while central wires those reserved seams. This does not
by itself complete survival_combat.4 or enable full survival.

Companion source lifecycle: [death and respawn](entities-survival-death-respawn.md).
Instance: es-20261101-1425-a7c3; issue: https://github.com/Ferigone/Cinderstone/issues/4.
