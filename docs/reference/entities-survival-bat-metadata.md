# Bat synchronized data owner

Team4 instance `es-20261101-1425-a7c3`, issue4 claim
[6569404387](https://github.com/Ferigone/Cinderstone/issues/4#issuecomment-6569404387).
Implementation: `src/living_entities/metadata.rs`. Base inspected at
`e1471304cbd3e2cb4a30d0b522682084fded0d80`.

## Pinned original evidence

Minecraft Java1.21 server, protocol767, official mappings:

- Outer server SHA1: `450698d1863ab5180c25d7c804ef0fe6369dd1ba`.
- Official mappings SHA1: `31c77994d96f05ba25a870ada70f47f315330437`.
- Original inner-server SHA256:
  `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

Evidence is direct JDK21 `javap -c -p -l` inspection of that unchanged inner
server, interpreted with its official mappings. No REA/MCP run is claimed.
Original binaries and raw disassembly remain outside the repository.

`SynchedEntityData` (`aka`)65–73 obtains the actual DataItem, compares the
incoming object with its current object using `ObjectUtils.notEqual` unless
forced, writes the new value, invokes the retained holder's synchronous
`onSyncedDataUpdated(accessor)`, then marks that DataItem dirty and finally the
aggregate dirty flag. A callback failure keeps the completed value write and
does not execute the remaining dirty writes. A preexisting dirty bit survives.
Reentry can pack or change fields before the outer setter marks its item.

`packDirty`81–94 returns null if the aggregate flag is false. Otherwise it
clears that flag first, visits the array in ascending accessor order, clears
each dirty item before obtaining its DataValue, and returns the list.
`getNonDefaultValues`99–111 traverses the same array without changing dirty
flags and returns null when every field equals its registered default.
`DataItem`168–172 stores the same supplied initial object in both current and
initialValue;195 compares initialValue.equals(current). `DataValue`133–134
invokes the accessor serializer's copy while taking its snapshot. All
serializers used by Bat0–16 use `ForValueType.copy`, whose body returns the
same object (`EntityDataSerializer.ForValueType`18). Lists used by this
bounded actor are immutable source lists/replaced as a whole; Particle clones
retain the existing shared Particle Arc identities.

`LivingEntity` (`btn`)309–316 defines Living flags0, empty particles, ambient
false, arrow0, stinger0, health1, and empty sleeping position, in that definition
order. Accessor allocation establishes IDs8–14; snapshots use ascending IDs,
so health9 precedes particles10 despite their definition order. Mob
(`btp`)288–290 delegates to Living then defines byte0 at15. Bat (`cez`)62–64
delegates through AmbientCreature/Mob and defines byte0 at16. Player's float
absorption15 is a different schema and is never part of this owner.

Entity's constructor attaches the built virtual data hierarchy before Living
field initialization and constructor work. Registered health1 is distinct
from the later source setHealth(getMaxHealth), which writes6 for Bat. Server
Bat construction40–52 later calls setResting(true), mutating byte16 to1.
Neither constructor mutation is manufactured by `defined_bat_defaults`.

Bat and Mob do not override `onSyncedDataUpdated`. Living3202–3220 first calls
Entity3034–3037, whose only branch is accessor6 (Pose) invoking actual virtual
refreshDimensions3054–3067. Living's additional sleeping14 and flags8 branches
are gated by the actual Level.isClientSide field. Consequently this SERVER
Bat owner concretely invokes only the required Pose dimensions callback; it
does not install an arbitrary successful inherited-callback stub. Client
actors require their additional source callbacks and are unsupported here.

## Actual storage and exported calls

`BatEntityData::defined_bat_defaults` creates only the actual values8–16 and
their dirty state. It has no Clone implementation, duplicate health scalar,
Player schema, physical state, ID, RNG, actor lookup or scheduler. The concrete
actor frame must attach this value and the already canonical
`EntityPhysicalState`; fields0–7 and their dirty bits project directly from
that same physical owner. Item's separate physical metadata bit8 is not
consumed as Living metadata8.

`BatMetadataHost::Data` is the assembly's retained metadata receiver. It must
root the complete original actor through removal, including dimensions,
attributes and world owners, rather than merely retain a state-only value or
resolve a possibly reused entity ID. `with_metadata` supplies a callback-free
short projection of both canonical fields and releases its guards before
returning. `refresh_dimensions` must execute the actual source dimensions
operation on that receiver; no physical copy may be committed afterward.

The free callable functions are `get`, `set`, `is_dirty`, `pack_dirty_values`
and `non_default_values`. `set` owns the actual equality/write/callback/dirty
sequence. Invalid accessor/serializer/pose errors precede mutation. Its typed
representation preserves signed zero and Java boxed-float NaN equality via
the existing shared `MetadataValue`; it does not call payload encoding or
add wire budgets to the source setter. Snapshots are values, not packets.
The integration/tracker owner may subsequently use existing
`metadata::encode_values`; an encoding failure must not restore dirty bits
already consumed by packDirty.

Existing physical setters mark canonical physical dirty bits directly. The
aggregate query consumes those same bits without introducing a mirror.
Actual Bat source writes, especially Pose6, must enter through this callback
aware setter. Calling the existing physical `set_pose` directly would skip
the required dimensions callback and is not a valid substitute. The actor
must also keep all tracking drains attached to this data receiver rather than
independently clearing the physical portion before packDirty.

## Bounded behavior and validation

Nonempty custom-name writes explicitly return `UnsupportedComponentOwner`.
The existing physical field stores optional wire bytes, and shared metadata
documents incomplete rich-Component equality. This owner does not silently
substitute byte equality for Java Component ownership/equality. Clearing to
None works without such a comparison. Snapshotting an existing upstream Some
value retains its wire representation for the shared serializer, but does not
prove canonical Component aliasing or make named Bat lifecycle supported.
Source actor construction remains bounded to unnamed server Bats until that
reserved physical Component seam is integrated.

Focused Rust1.99.1 compilation uses this actual new source and the existing
compiled repository's real `EntityPhysicalState`, `MetadataValue`, Position
and Vec3 dependencies, with `--edition=2021 --test -Dwarnings`. It passed.
The focused runtime verifies constructor/default distinctions, Mob/Bat byte
schema, default reversion still being dirty, rejection before mutation,
synchronous Pose callback reentry, short-lock release, callback failure and
force prefixes, canonical physical projection, preservation of unrelated
physical bits, signed zero and boxed NaN equality. Scratch artifacts are
`nov2-bat-metadata-focused.rs` and `nov2-bat-metadata-tests` under the instance
scratch directory. These checks do not execute original Bat construction,
world callbacks, packets, tracking or Main. Whole-repository composition and
the real actor assembly are separate root validation/integration gates.

Incoming client `assignValues`, client-only callbacks, mutable metadata-list
aliasing, named Component ownership and generic non-Bat schemas are outside
this bounded server owner. No production admission/tick/Main proof or whole
entities/survival delivery completion is claimed by this module.
