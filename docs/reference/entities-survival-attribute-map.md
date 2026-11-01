# Native AttributeMap state and exact AttributeInstance copying

Instance: es-20261101-1425-a7c3; issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Base: `c1baf5c70f07e53b6b76edaf84be5da57f6bd40b`.
Owned source: new `src/effects/attribute_map.rs`, source copy helpers/tests only
in owned `src/effects/attribute_instance.rs`, one owned effects/mod.rs export.
Pinned original server SHA1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`;
official mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`.

Implementation ready in /workspace/cinderstone-attribute-map-es-20261101-1425-a7c3:
only new src/effects/attribute_map.rs and narrow copy/replace helpers plus focused
checks in existing src/effects/attribute_instance.rs. Root owns mod exports,
reference notes, Git and full-crate validation. No actor/Main integration claimed.

Source is pinned unchanged original server inner jar SHA256
8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01, official mappings.
Evidence is direct javap, original packaged fastutil and controlled original-Java
probes. No REA execution. Source artifacts remain scratch/oracle:
attribute-map-supplier.javap (buv), effect-contract-but.javap,
effect-contract-bus.javap, attribute-map-fastutil{,-interface}.javap,
attribute-map-jdk-map.javap, attribute-map-dirty-iterator.javap.

## Source facts and corrected copy premise

- bus.replaceFrom167-180 copies base, clears+putAll byID, clears+putAll permanent,
  clears the operation EnumMap, then EnumMap.forEach invokes lambda177:
  getModifiers(op).putAll(sourceOperationMap), finally setDirty once.
- That lambda does NOT use the fastutil Map-copy constructor. getModifiers
  creates a fresh default operation bucket n/minN32. putAll preallocates using
  tryCapacity(current size + source size), actual f32 quotient -> f64 ceil,
  capped at1<<30, then inserts in source map iteration order. Source size25
  retained atn128 is copied into n64/minN32, not n128 or minN64.
- The initial constructor premise was caught and corrected before commit by
  root's unchanged-original AttributeCopyProbe + direct lambda inspection.
  Final helper and assertions follow the corrected source.
- Modifier Arc identities, operation ghosts and empty operation buckets survive
  copying. Target definition/object/listener and old cached value remain; dirty
  becomes true before terminal listener. Listener failure preserves copied prefix.
- Java replaceFrom(this) is real: clearing each receiver map clears its source
  before putAll/forEach. Explicit replace_from_self clears all modifier indexes,
  retains base/cache/definition, then invokes actual dirty listener.

Original-Java AttributeCopyProbe.java SHA256
97a18fe8139656b1331129353aac2a30f69a79dc4897f41686a7e763c89cee27;
attribute-copy-probe.log SHA256
3933d83a9d19109fd08839a610f6401e840622ce02c2357f725cf68e9746efe4.
Root ran this unchanged bus body with actual packaged fastutil and a controlled
Consumer. This proves the recorded component/callback facts, not actor/Main.

## Concrete native APIs and real owners

attribute_map.rs exports JavaObjectKey (actual equality/hashCode/identity,
clone retains canonical object), MapAttributeDefinition (existing immutable
AttributeDefinition plus actual isClientSyncable field), AttributeMapState,
DirtySet, DirtyCursor and typed source-condition errors.

AttributeMapState owns exactly its default fastutil-shaped holder->instance
object table, toUpdate object set, toSync object set and real supplier handle.
It creates no actor, registry, holder hash policy, synthetic scalar attributes,
health/absorption owner, RNG or scheduler. Canonical holder/instance keys must
come from actual runtime owners; registry IDs/arena indices are not default
Object hashCode substitutes.

AttributeMapHost supplies retained actual Map, Actor, Instance and Supplier
handles, canonical definitions, source supplier lookup, actual retained object
allocation, callback-free short with_map/with_instance access, and actual
onAttributeUpdated(actor,holder). Handles cannot retarget on ID reuse.

get_instance implements but54 and buv44-50: current table lookup; supplier
lookup; NEW existing AttributeInstance(actual definition); allocate canonical
object; source replacement data/dirty installation; onAttributeModified;
then final current-table put. Dirty listener registration precedes map visibility.
The real supplier callback is not a fake numerical fallback: it must read the
actual immutable bound default instance, whose value cache is retained.

JDK overload is material: but54 invokes java.util.Map.computeIfAbsent(Object,
java.util.function.Function). Packaged fastutil specializes the distinct
Object2ObjectFunction overload only. Actual JDK default is get -> function ->
nonnull result put -> return result. Thus creation reentry can put an inner
instance, then outer put replaces it and outer is returned; both created objects
may remain in actual dirty sets. No stale captured table slot or invented CME.

Other exported runners: on_attribute_modified; has_attribute; value;
base_value; has_modifier; modifier_value; refresh_dirty_attributes. Query fallback
uses actual supplier without materializing actor copies. Existing-instance
missing modifier is distinct from supplier's IllegalArgumentException case.
AttributeMapListener binds the exact canonical instance to the source map's
fixed synchronous listener body; it does not queue notifications or defer until
permanent insertion. Runtime can split its actual arena and map to use existing
AttributeInstance mutation APIs safely.

## Live dirty sets, borrowing and exception prefixes

but24-31 uses default Object2ObjectOpenHashMap for instances and ObjectOpenHashSet
for both sets. onModified34-38 adds toUpdate FIRST, reads actual definition's
syncable field, then adds toSync conditionally. None of these fixed source
operations invent an actor callback. Actor callbacks occur in the separate
refresh loop, after all map/instance access closures have returned.

LivingEntity.refreshDirtyAttributes1091-1096 iterates the live toUpdate set,
invokes actual onAttributeUpdated for each holder, then clear after SUCCESS.
Native cursor captures source pos and remaining count but reads current table
on each next step. Reentrant addition does not increase captured count; a new
entry in a later scan position can be visited instead of an old entry. Final
clear removes even new/unvisited entries, as source does. Callback error does
not clear. No snapshot, drain, secondary deferred set or fail-fast CME is added.

Fastutil SetIterator has no fail-fast count. Reentrant deletion can exhaust
slots with remaining count positive, then dereference absent wrapped list
(source NPE); shrinking can cause source array bounds failure. Native explicit
DirtyIterationError preserves these source conditions/prefixes rather than
silently rejecting all reentry. Null keys/elements and iterator.remove/raw
mutable collection views are outside this bounded API.

The source array has n+1 slots: reserved key[n] is a VALID empty slot in nonnull
domain. Native cursor models that implicitly. Independent reviewer caught the
premature bounds error at index==n; it was corrected before commit and verified
against actual packaged-fastutil DirtyCursorShrinkProbe. Its sequence grows50
keys to n128, removes25, visits slot65, removes2 -> n64, then next scans valid
null key64 and returns slot48. Native check reproduces this actual history.
DirtyCursorShrinkProbe.java SHA256
4dcc4a2b6c927580ab764702ce5d6939aa0f997e73cc0ce7700cb87f62710698;
dirty-cursor-shrink-probe.log SHA256
b191e2004495ed95613142e5ae06234b421ac69f9ceda0c2213c3970b44ed02b.

## Validation and limits

Standalone actual-file rustc --edition=2021 -D warnings --test using
scratch/attribute-map-test-harness.rs passes. Existing attribute arithmetic/
identity/callback checks remain; added checks match original copy dimensions,
exact copy25 hash order, shared ghosts/permanent/cache, terminal error and self
alias. Map checks exercise real native state + retained mock object arena,
lazy fallback/cache, dirty registration before put, JDK outer/inner creation
reentry, same actual modifier identities, live dirty refresh/reentry/error/clear
and original shrink boundary. All owner borrows are checked available at host
callback boundaries. These are component checks, not production actor evidence.

Not implemented: NBT save/load and source warning/registry resolution, supplier
builder freeze rules, mutable raw collection views/null elements/iterator.remove,
getSyncableAttributes collection export, multimap bulk add/remove,
assignAllValues/assignBaseValues, concrete actor/registry/hash/supplier bindings,
effect definitions and Main/persistence wiring. The native module has no
unsupported-body stubs pretending these methods completed. VM allocation/OOM
exception equivalence is not established. This stage does NOT complete whole
AttributeMap/effects/survival or any fixed checklist row.

## Root validation and central integration

Ordinary lib+Main compiler check PASSED for this exact source composition at
c1baf5c with unchanged manifests/lockfile using Rust1.99.1. Focused actual-crate
effect results and exact reviewed source/merge SHA are recorded on the PR.
Two independent complete source/container/borrow reviews found no material
mismatch after the documented precommit corrections. Runtime probes use the
original unchanged AttributeInstance and packaged fastutil with controlled
callbacks; no actual actor, registry, packet, persistence or Main observation
is inferred. The copied source supplier is immutable, while retained actual
instances and map remain real mutable runtime objects.

Actual holder/instance object identity hashes and canonical arena/supplier
bindings must be assigned centrally before integration. Player metadata9/15
remain health/absorption; onAttributeUpdated callbacks must clamp those actual
values at original MAX_HEALTH/MAX_ABSORPTION sites. Do not maintain a duplicate
numeric total or defer dirty notifications into a second queue. No whole
survival_combat.4 or entity delivery is complete from this bounded map state.
