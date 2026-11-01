# Native AttributeInstance state and hash-order arithmetic

Instance: es-20261101-1425-a7c3; assigned issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Base: `47182b236135a295ab17a2d64aaedd425f960869`.
Owned: new `src/effects/attribute_instance.rs`, only its export in owned
`src/effects/mod.rs`, and this note.

## Original evidence

Minecraft Java1.21/protocol767 original server SHA1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1
`31c77994d96f05ba25a870ada70f47f315330437`; unchanged inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Direct JDK21 javap of original AttributeInstance (`bus`), AttributeModifier
(`buu`), ResourceLocation (`akr`) and packaged fastutil8.5.12. This is not an
REA/MCP execution. Binary/disassembly/probe artifacts stay outside Git.

| Original AttributeInstance methods | Source lines | Native API |
| --- | --- | --- |
| constructor, holder/default/fields | 26–38 | `AttributeInstance<D,K>::new` |
| getAttribute, getBaseValue, setBaseValue | 41,45,49–54 | `attribute`, `base_value`, `set_base_value` |
| getModifiers, getModifier, hasModifier | 62,67,71 | `modifiers`, `modifier`, `has_modifier` |
| addModifier; operation-map factory | 75–81; lambda58 | internal source-shaped insertion |
| addOrUpdateTransientModifier, addTransientModifier | 84–90,93–94 | `add_or_update_transient_modifier`, `add_transient_modifier` |
| addOrReplacePermanentModifier, addPermanentModifier | 97–100,103–105 | corresponding snake-case APIs |
| setDirty | 108–110 | `set_dirty` |
| removeModifier overloads, removeModifiers | 113–124,128–131 | `remove_modifier`, `remove_modifier_by_id`, `remove_modifiers` |
| getValue, calculateValue, getModifiersOrEmpty | 134–139,143–159,163 | `value`, source calculation/operation traversal |

## Callable state and callback contract

`AttributeDefinition` is the actual immutable attribute definition/holder,
retained by Arc. Its actual default and virtual sanitizeValue are mandatory;
there is no substitute range or fallback total. `AttributeInstance<D,K>` owns
base value, dirty flag, cached value and the source modifier indexes. No actor,
health, equipment or registry owner is allocated.

`ResourceLocationKey: Clone+Eq` must supply the actual immutable source ID and
Java hashCode. `resource_location_java_hash(namespace,path)` uses original
31*namespaceHash+pathHash with wrapping Java UTF16 String hash. It does not
parse, validate or register an ID. Canonical IDs are nonnull.

`Modifier<K>` retains ID, raw double amount and one of AddValue,
AddMultipliedBase, AddMultipliedTotal. Modifier Arc identity matters:
addOrUpdateTransient returns early only for the same object, not a new equal
record. The by-ID replacement happens before that identity check. A replacement
with a different operation leaves the old operation bucket entry, as Java does;
remove-by-ID removes the new operation entry and can leave that old ghost.
Do not silently strengthen those source invariants.

`DirtyListener<D,K>` is mandatory and receives the same instance synchronously.
setDirty sets dirty first, then calls it. It may read value (clearing dirty) or
mutate/reenter that same instance. addPermanent inserts by-ID/operation indexes,
calls dirty, then inserts the permanent index. A failing listener leaves the
already completed prefix; later permanent insertion does not run. Reentry may
also remove the just-added live modifier before the outer permanent insertion.
`MutationError` distinguishes duplicate-ID source exception from listener error.
No rollback or successful missing callback is supplied.

removeModifiers takes the original immutable encounter-order snapshot before
callbacks, so new reentrant modifiers are not appended to that traversal.
setBaseValue uses Java double equality: signed zeros compare equal, NaN always
invalidates. value calculates only while dirty, installs cache then clears dirty.

## Container and numerical order

By-ID and permanent indexes are Object2ObjectArrayMap-shaped insertion-order
maps, preserving original key handles on replacement and shifting on removal.
Each lazily allocated operation bucket is Object2ObjectOpenHashMap-shaped,
not an insertion-order map. It preserves fastutil mixed32bitJava hash, default
32 slots/minN32/.75fill, linear probing, descending old-table rehash, shift
removal, source growth after the 25th insertion and shrink conditions.
Iteration scans occupied slots descending. The nonnull ResourceLocation domain
does not need fastutil's Java null-key slot. Operations traverse in enum0/1/2
order; capacity and collision history affect arithmetic encounter order.

Calculation first adds AddValue amounts in that order, retains the resulting
base, adds base*amount for each AddMultipliedBase, multiplies sequentially by
(1+amount) for each AddMultipliedTotal, then invokes actual sanitizeValue.
No sorting or parallel reduction is used. The original probe inserts
`probe:a=1e16, probe:b=1, probe:c=-1e16`: actual table scan is `[a,c,b]`, giving
exact1.0; insertion-order summation would give0.0. The port preserves1.0.

## Validation and integration limits

The controlled original-Java probe uses unchanged bus/buu/akr, packaged
fastutil and original Attribute with identity sanitization; its Consumer supplies
surrounding observations/failures. Compiler and JVM exit0. Probe source SHA256:
`5d1c933e7eda948a06ea6159c45017f7ded51fd8e19bfc085dc9bf3fb6944a97`;
output SHA256:
`9c1d1fabfaff4329b1d6bedb3a6f6f86679fcf60421128c755abc3f5b0fbfa3f`.
Original observations establish nonassociative hash order, 24/25 growth/table
order, collision deletion, object shortcut, old-operation ghosts and permanent
callback/error prefixes. Focused native flows reproduce those results plus
cache, NaN/zero and synchronous same-instance reentry. These are controlled
source contexts, not real equipment/effect/player world behavior.

Independent fresh source review and full crate results are recorded in the PR
and canonical instance status. Existing manifests/lockfile are unchanged.

Unsupported: AttributeMap/supplier/registry/equipment/actor binding,
serialization, replaceFrom and mutable Java map views. Returned snapshots are
explicit read-only native observations, not mutable Java live views. No new
Main actor callsite or whole survival/entity delivery is claimed. Actual effect
attribute templates use addPermanentModifier and actual save/load must preserve
attributes and decoded effects together without replaying add/start callbacks.
See [the central collection/definition/attribute contract](entities-survival-effects-integration.md)
for exact proposed actor fields, dirty queues, canonical holder/hash owners,
metadata/packet phases and persistence requirements.
