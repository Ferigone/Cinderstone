# AttributeSupplier native owner — 2026-11-02

Claim issue4/6566884842; only authored repository file
`/workspace/cinderstone-attribute-owners-es-20261101-1425-a7c3/src/effects/supplier.rs`.
Branch attribute-owners base `96b4bad622155e2891d9d8a249f8e233721d34c8`.
Root owns exports, docs, Git/publication and every Main/shared seam. Companion
definitions.rs is exclusively effects_lifecycle's claim. No changes to existing
AttributeMapHost/AttributeInstance, registry, Main, status or checklist.

## Implemented source / current API

* `SupplierBuilderState<Holder,Instance>` is actual pending ordered Guava-style
  builder records (duplicates retained) and shared native frozen boolean.
  `FrozenSupplierListener` retains that same boolean and captured canonical
  holder after build/builder-handle release. A real arena must store its actual
  descriptor on the same newly allocated template instance.
* `RegisteredAttributeHolder: JavaObjectKey` certifies only nonnull canonical
  registered Reference object identity with stable real Object.hashCode. This
  deliberately does not claim arbitrary Direct/unbound/key-equals behavior.
  `SupplierKeyHost` supplies actual holder hash and getRegisteredName at their
  source observation points. No private registry, name interner, hash counter,
  protocol-ID identity or Rust-address identity is constructed.
* `AttributeSupplierHost` requires actual retained builder/instance arena short
  access, registered definition, actual NEW instance adoption with its consumer,
  real target Consumer handle/dispatch, and ResourceLocation.toString. No owner
  method has a successful missing-operation default. Definition stays generic
  `AttributeDefinition<Key=Holder>`, compatible with actual BoundAttribute from
  definitions.rs and without copying its shared value/sync/sentiment owner.
* `add` implements Builder81–82; `add_with_base`86–88. Both create NEW native
  AttributeInstance, append first. The latter runs actual native raw-base/dirty
  assignment then concrete frozen listener after ending the object access.
  Native set_base_value_fields is reused unchanged, so equal signed zeros skip
  listener and NaN does not. A frozen add with unchanged new default succeeds;
  changed base and appended object remain visible before name/exception.
* `build`92–93 stores frozen=true before snapshot/hash/construction. Native
  immutable-map construction uses source empty/singleton shortcut; regular
  reverse key hash/bucket scan, first-position/last-created-template duplicates,
  deduplicated-table-size reconstruction (and its additional reverse hashes),
  and the actual >8 failed-bucket-comparison overflow threshold. It implements
  the JDK-backed logical fallback rather than silently assuming good hashes.
  The fallback preserves canonical-reference behavior, forward delegate and
  duplicate-map hash observation phases and first-position/latest-value result.
  It does not claim a generic Java tree-bin/object implementation. Suppliers
  have immutable independent topology while template leases remain shared.
* `AttributeSupplierState::{template,has_attribute,entries}` expose actual
  source lookup plus encounter observation. `value`, `base_value`,
  `modifier_value`, `has_modifier` use those retained actual native instances;
  value reads/clears their actual cache, raw base is not sanitized. Missing
  required attribute resolves registered name then reports its source error;
  absent has queries are false. Missing modifier evaluates modifier.toString
  BEFORE holder.getRegisteredName. There is no numerical fallback.
* `create_instance`44–50 captures template, constructs/adopts NEW target with
  actual target consumer, reads current template replacement fields AFTER
  construction, installs native replacement (retaining Modifier Arc objects),
  then calls the actual consumer before returning. It inserts into no actor
  map. Missing returns None before construction/name/consumer. New object and
  completed consumer mutation prefix survive consumer errors. `notify_listener`
  routes retained descriptors to concrete frozen body or mandatory actual
  target consumer, without holding instance/builder access across callbacks.

Existing AttributeMapHost remains exactly intact. Its supplier_instance can
query this actual immutable state and return its same native template lease.
Existing AttributeMap.get_instance already composes source createInstance with
its JDK default computeIfAbsent final insertion and dirty registration; do not
call both executors or insert eagerly/materialize all supplier defaults.

## Primary evidence and exact source points

Direct JDK21 javap of original pinned Java1.21/mappings; no REA execution or
new Minecraft runtime/registry observation is claimed. Inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`.

Scratch/oracle `attribute-supplier-builder.javap`:

* buv$a.create71 new bus(holder, consumer(this,holder));76 builder.put;77 return.
* add81 create only;86 create then87 instance.setBaseValue;88 return builder.
* build92 writes b=true at bytecode0–2 before93 buildKeepingLast at13.
* lambda72 reads b at0–4; frozen branch73 evaluates holder.g at11–12 then
  UnsupportedOperationException constructor22 and throw25. No preflight add
  rejection and no getRegisteredName for unfrozen listener.

Scratch/oracle `attribute-map-supplier.javap`:

* buv.getAttributeInstance19 map.get,20 null branch,21 name then exception.
* getValue27 calls actual bus.getValue; getBaseValue31 actual raw getter.
* getModifierValue35 captures modifier;36 missing branch evaluates
  String.valueOf(modifier)17–19 before holder.g22–23;39 returns amount.
* create44 map.get;45 null guard;48 NEW bus(actual holder,target consumer);
 49 target.replaceFrom(captured template);50 return. No map insertion.
* hasAttribute58 containsKey; hasModifier62 map.get followed63 modifier!=null.

Packaged Guava32.1.2-jre inspected separately, raw scratch only:
`nov2-supplier-{ImmutableMap-Builder,RegularImmutableMap,JdkBackedImmutableMap,
SingletonImmutableBiMap,Hashing}.javap`.

* Builder.build has source size0/1 shortcuts, then Regular.fromEntryArray.
* Regular construction scans last->first, hash at90–95, bucket lookup110–120;
  shared latest duplicate entry is recorded and forward removeDuplicates
  retains its first encounter. Deduplicated table size difference recursively
  reconstructs at261–267. checkNoConflict's count>8 throws only after a failed
  equality; an equality match returns before increment/overflow.
* JdkBacked.create forward delegateMap.put59–69; duplicateMap.put136–147;
  second pass containsKey204–212, get216–225, then first occurrence put(null)
 248–258. Source canonical equality has no callback; logical native fallback
  emits the same extra actual hash observations and immutable encounter list.
* Singleton get/contains uses key.equals without hash; empty lookup no hash.

## Focused verification and limits

Standalone fixture `nov2/supplier-focused.rs` includes the actual unchanged
attribute_instance.rs, attribute_map.rs and new supplier.rs. Rust1.85 `-Dwarnings`
rlib and test compilation passed; focused supplier flow execution passed after
formatting. Log `nov2/supplier-focused.log`. Controlled context cases exercise
duplicate/rehash ordering and template identity, immutable snapshots, frozen
append/mutate/name failure and name-provider reentry, retained listener lifetime,
hash-error frozen prefix, empty/singleton no-hash, collision overflow fallback
and duplicate hash phases, fresh target/current template reads, shared modifier
objects, synchronous dirty consumer cache effects/errors and missing query
evaluation order. These are explicitly controlled arena/provider schedules,
not an actual registry, actor, Main, packet, world or persistence invocation.
No comprehensive full Cargo check/test was run by this subordinate; root owns
composed compiler/runtime gates.

A separate controlled probe executed the unchanged packaged Guava32.1.2
ImmutableMap.Builder with identity-equality keys and explicit hash values. It
independently confirmed the duplicate/reconstruction reverse hash sequence,
first-position/latest-value result, collision-overflow fallback's full extra
forward/duplicate hash sequence, and zero hashes for singleton build/lookup.
This executes the original library map, not Minecraft AttributeSupplier, a
native registry or gameplay. Probe and class/raw output stay only in scratch.
`oracle/AttributeSupplierGuavaProbe.java` SHA256
`c077d8620c9d35e677458583623f946f91f3478e12d9ebf1979a6ef1bbb320e5`;
`oracle/attribute-supplier-guava-probe.log` SHA256
`2ce85dafc90ae86152b513a56f2d6ce85b013d176a6c4be8e92b96282bbab143`.

Production binding still needs actual registered Attribute values/Reference
holders/ResourceKeys/Java hashes and registry IDs, real shared default supplier
arena and EntityType->supplier owner, true actor final AttributeMap/listeners,
equipment/effect modifier callbacks, packet tracker/registry codec/FIFO and
save/load phases. No Bat/default supplier table or actor constructor is created
here. Nothing claims entities or survival_combat whole-delivery completion.

Current pre-review formatted source SHA256:
`763289cf26858b8d6881f521406865f9621bf5db6c22604df4381db525a53422`.
