# Retained native attribute bindings and Bat defaults

Issue4, instance `es-20261101-1425-a7c3`, claim
[6569404387](https://github.com/Ferigone/Cinderstone/issues/4#issuecomment-6569404387).
Base `e1471304cbd3e2cb4a30d0b522682084fded0d80`. Owned new files:
`src/effects/binding.rs`, `src/effects/default_attributes.rs`, this reference.
Root owns exports, Git, Main and shared registry/runtime integration.

## Source evidence

Minecraft Java1.21, protocol767. Original inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official server mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`.
Direct JDK21 `javap -c -p -l` analysis, not REA/MCP execution. Raw disassembly
and Java artifacts remain scratch-only.

`LivingEntity.createLivingAttributes` (`btn.dV`,319–337) creates a builder and
adds these18 actual static Attributes references in this exact order:
MAX_HEALTH, KNOCKBACK_RESISTANCE, MOVEMENT_SPEED, ARMOR, ARMOR_TOUGHNESS,
MAX_ABSORPTION, STEP_HEIGHT, SCALE, GRAVITY, SAFE_FALL_DISTANCE,
FALL_DAMAGE_MULTIPLIER, JUMP_STRENGTH, OXYGEN_BONUS, BURNING_TIME,
EXPLOSION_KNOCKBACK_RESISTANCE, WATER_MOVEMENT_EFFICIENCY,
MOVEMENT_EFFICIENCY, ATTACK_KNOCKBACK.

`Mob.createMobAttributes` (`btp.C`,172–173) calls this factory then adds the
actual FOLLOW_RANGE holder with raw base16. `Bat.createAttributes` (`cez.s`,
112–113) calls Mob's factory then adds MAX_HEALTH again with raw base6. Both
add overloads allocate a NEW AttributeInstance; the Bat builder therefore
holds20 records/objects, and Guava buildKeepingLast produces19 distinct keys,
keeping the first health position and last health instance. Bat has neither
FLYING_SPEED nor ATTACK_DAMAGE in its source supplier.

`DefaultAttributes.<clinit>` (`bux`,88–175) calls `cez.s` at bytecode60,
Builder.build at63 and inserts that supplier at66 into the real global
EntityType-to-supplier map. `getSupplier`178 is a map lookup. The new factory
ports this actual Bat supplier construction; it does not construct another
DefaultAttributes global map or registry.

The existing source supplier implements Builder freeze-before-build, native
Guava key/hash/duplicate phases and the actual frozen listener, and the existing
AttributeMap implements computeIfAbsent's dirty registration before final
insertion. These executors are reused directly, not duplicated or double-run.
See [supplier evidence](entities-survival-attribute-supplier.md) for its pinned
source facts, if present in the current checkout.

## Callable implementation and ownership

`NativeAttributeInstance<H,O,M>` is a strong lease of one actual native
AttributeInstance with its real installed frozen/map-target consumer. Clone
retains that same Arc object, Eq compares its source native object identity,
and Object.hashCode delegates to the SAME externally supplied actual lazy
identity token O. No registry ID, arena slot, Rust address, private hash counter
or RNG is used as a Java identity hash.

`NativeAttributeBuilder` owns the original mutable builder state and ordered
records. `NativeAttributeSupplier` owns the immutable native supplier and
retains that same builder. The actual default-supplier bootstrap owner MUST
retain this shared supplier through shutdown, matching DefaultAttributes'
static lifetime. This preserves the20-template construction graph for the
supported Bat bootstrap. This is not a general tracing-GC implementation:
a template escaping alone after destruction of every supplier/builder would
retain the frozen control but not the full builder records. That destruction
pattern is outside this static default-supplier binding.

`NativeAttributeMap` is an alias of the existing AttributeMapState with these
actual retained types. It is intended to remain the SAME inline
LivingEntityState.attributes field, with no second actor map.
`NativeAttributeBinding<C>` implements both existing native supplier and map
host contracts. `C: AttributeBindingOwner` supplies exact canonical holder,
registered BoundAttribute, real new-object identity token, source name/hash
providers, short access to the actual map and the real Living callback.
No external operation has a successful default implementation.

The binding concretely allocates and retains instances, accesses templates,
performs supplier lookup, installs/dispatches the actual native map dirty
consumer, and forwards real attribute-updated callbacks after ending state
loans. Inherent get_instance binds the exact allocation map for that call.
set_base_value and replace_from perform native state fragments followed by the
SAME installed consumer, including source self-copy, frozen failure prefix and
Java signed-zero equality. Modifier mutation methods are not newly wrapped by
this binding; existing source executors and their required real listeners
remain separate. Never run callback-bearing arbitrary code under with_instance
or with_map access closures.

`create_living_attributes`, `create_mob_attributes`, `create_bat_attributes`
return the actual retained native builder at their source stage. Call
`build_bat_supplier` once from the actual default-supplier bootstrap owner and
share its result. Fresh Bat map construction is
`NativeAttributeMap::new(shared_supplier.clone())`; constructor max-health and
follow-range reads use `attribute_map::value` with the native binding. They
query the supplier's6/16 fallback while the actor map remains empty. They do
not call getInstance or pre-materialize/synchronize19 defaults.

## Actual external owners still required

At the inspected base, Main's NativeFallingDamageTypes,
NativeBlockStateRegistryBinding and NativeEnchantmentSnapshot have retained
identity for their own narrow registries. None is the global ATTRIBUTE or
MOB_EFFECT registered-reference/ResourceKey owner or a Java VM object-hash
service. saved_data and source NBT likewise provide no such authority.

The real integration owner must supply the canonical31 registered Attribute
Reference objects and shared AttributeValue bindings from original bootstrap,
ResourceKey identity/interner and register/freeze phases, registered names,
and one shared lazy Java object-identity authority for each NEW native
AttributeInstance. Hash initialization happens at actual source hashCode
observations, not eagerly when the native instance is allocated. Equivalent
canonical effect references/definitions remain their own required owner.
The binder does not certify an arbitrary provider as an implemented production
registry merely because it satisfies a Rust trait.

Actor map descriptors must address and retain the exact source map/lifetime
through a rooted invocation, including callbacks after removal. A weak internal
backreference is only valid when the real actor invocation/escaped-instance
root separately retains the actor. This module creates no actor self-cycle or
replacement entity-ID lifetime authority.

## Validation and integration state

Focused stable rustc1.99.1 compilation with `-D warnings` succeeds against the
actual native NBT dependency and current attribute source modules. A native
binding fixture runs the actual factories/supplier/map executors, checking
20 allocations versus19 final keys, no eager instance hash observations,
6-health/16-range fallback with zero actor instances, lazy canonical instance
reuse and dirty/sync sets, installed map consumer, actual update-error prefix,
frozen template mutation-before-error and retained displaced template.
Existing source supplier/instance/map/persistence checks also pass in that
same focused executable. Fixture canonical references and hashes are controlled
inputs, not a claimed production registry/hash implementation.

No Main callsite, production actor tick, packet, loot, XP, persistence, spawn or
whole entities/survival delivery is proved by this binding check. Root performs
full crate compilation and integrates the actor constructor separately.
