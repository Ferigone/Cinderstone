# Default LivingEntity Brain ownership

This is the bounded base LivingEntity provider inherited by ordinary Bat. It
is not the complete Brain/sensor/behavior system and does not establish a Main
actor, scheduler or production tick callsite. The implementation is
`src/living_entities/brain.rs`.

## Pinned source and method evidence

Evidence uses the original Minecraft Java 1.21 server, protocol 767, official
mapping SHA1 `31c77994d96f05ba25a870ada70f47f315330437`, original server SHA1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, and extracted original server JAR
SHA256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Analysis used direct JDK 21 `javap` and reflection into unchanged original
classes. No REA/MCP analysis was performed. Original archives, binaries and
disassembly remain outside Git.

- LivingEntity (`btn`) 282–283 constructs an **NbtOps** Dynamic containing
  `{memories:{}}`, calls virtual `makeBrain`, and retains the returned `bF`.
  This is not a JSON Dynamic. `brainProvider` 291 supplies two empty immutable
  lists to `Brain.provider`; `makeBrain` 295 calls that provider. Ordinary Bat
  does not override those methods.
- Brain (`buq`) 69 creates the Provider. Provider 57–61 retains the memory
  collection, sensor collection and one newly constructed codec. Brain 73–110
  creates a recursive codec whose outer field is `memories`.
- Brain 115–159 allocates the memories HashMap, sensors LinkedHashMap, behavior
  priority TreeMap, activity-requirement HashMap, stop-memory HashMap, and core
  and active HashSets. It retains canonical `Schedule.EMPTY`, canonical
  `Activity.IDLE`, `lastScheduleUpdate = -9999` and its codec supplier. It then
  registers memory types, creates sensors, registers sensor-required memories,
  and applies decoded MemoryValues. These last three phases have genuinely
  empty inputs for this provider; there are no sensor RNG draws or AI owners
  to fabricate.
- Schedule (`cop`) 16 does not define an empty timeline table for EMPTY:
  `register("empty").changeActivityAt(0, Activity.IDLE).build()` creates an IDLE
  timeline with keyframe `(0, 1.0F)`. ScheduleBuilder (`coq`) 22–39 supplies
  that value. Timeline (`cor`) 11–12 and 42–62 retains the keyframes and
  `previousIndex`; its single-keyframe default returns IDLE for negative and
  positive timestamps. Activity (`com`) 37–71 retains the name `idle` and its
  Java String hash `3227604`; equality compares the activity name.
- Brain 162 calls its retained codec supplier and then `encodeStart`. Brain
  105/189 and MemoryValue 184 serialize actual registered memory entries. The
  supported default table has no entries, so the result is
  `success({memories:{}})`.
- Brain 200–203 clears memory **values at existing keys** without removing
  registrations. LivingEntity 700–706 calls virtual death effects where
  applicable, parent Entity removal, then reads the actor's current Brain and
  clears it. The caller must invoke `clear_memories` on that retained object
  after parent removal; this module does not invent another Brain.
- Brain 470–471 clears only its behavior-priority map. Mob 1551–1553 removes
  free will from its selector before calling this Brain operation. Brain
  336/362–370 activates the default activity only when explicitly requested;
  the constructor leaves the active set empty.

## Callable owner and identity

`DefaultBrainProvider::new()` allocates the provider/codec. Its
`make_from_constructor_memories()` constructs and decodes the exact source
empty-memories NBT, returning a new `DefaultLivingBrain` with its own fields.
The Brain retains a supplier of that same provider codec. Two Brains made by
one provider share the codec; independently created providers have distinct
codecs. `clear_memories`, `remove_all_behaviors`, `serialize_start_nbt`,
`use_default_activity` and callback-free state access operate on the retained
Brain.

`default_brain_bootstrap()` retains shared real default Activity and Schedule
objects. `idle_activity()` and `empty_schedule()` return references to those
same objects. Central canonical registry integration must reuse those
references for `minecraft:idle` and `minecraft:empty`; creating competing
objects from their names would not satisfy the identity contract. This
bootstrap covers only these required definitions, not complete registry
initialization or registry admission to Main. The schedule retains its actual
timeline and previous-index field; safe relaxed atomic access changes no
single-keyframe result or native callback ordering.

The default schema is enforced by uninhabited collection element types. It
cannot admit a manufactured sensor, behavior, memory module or memory value.
Those private collections remain actual per-Brain state. This owner is not a
generic Brain factory for mobs with different schemas.

## Saved input boundary and original runtime probe

`decode_empty_memories_nbt` accepts the supported compound with an empty
`memories` compound; source `fieldOf` ignores other outer fields. It explicitly
refuses noncompound roots, missing/wrong-type memories and nonempty memory
maps. These are **native support-boundary errors**, not an emulation of Java's
logged codec fallback. No arbitrary saved-state success is claimed.

The original decoder processes every incoming memory key through canonical
`BuiltInRegistries.MEMORY_MODULE_TYPE` and its individual codec, even for the
empty schema. It accumulates DataResult errors and logs before constructing a
Brain. Its constructor subsequently ignores decoded values whose memory
types are unregistered in this schema. Provider 64 separately calls
`parse(...).resultOrPartial(LOGGER::error).orElseGet(new Brain(...))` for outer
codec failures. Implementing those input paths requires the actual registry,
individual codecs and error/log ordering.

A focused JDK 21 reflection probe invoked the unchanged original provider,
NBT codec, built-in registry, canonical Activity/Schedule, clear and encode
operations and exited successfully. The observed facts were:

- `{memories:{}}` constructs the source field state described above and encodes
  exactly `{memories:{}}`, including after clear/removeAllBehaviors.
- `{}` logs `No key memories in MapLike[{}]` before fallback; `{memories:3}`
  logs `Not a map: 3` before fallback.
- An unknown memory key logs an unknown-registry-key error. ATTACK_TARGET
  input logs `No codec for memory: minecraft:attack_target`.
- Valid LAST_SLEPT input `{value:123L}` decodes without a log and is ignored
  by the default schema. Its malformed string value logs `Not a number`.
- Reading unregistered ATTACK_TARGET with native `getMemory` throws
  `IllegalStateException: Unregistered memory fetched: minecraft:attack_target`;
  this is not a successful absent-memory lookup.
- Brain objects retain the exact original canonical Schedule.EMPTY and
  Activity.IDLE references. EMPTY has one actual timeline and
  `previousIndex = 0`.

The Rust fixtures check independent per-Brain activity state, retained codec
and canonical-object identity, the original empty-state/NBT result, the real
default timeline including negative timestamps, and explicit refusal of
unsupported input. Main compilation/execution and focused Rust test results
belong to the root validation report; this probe itself runs no actor, world,
packet, Main loop or full survival delivery.

## Integration and unsupported behavior

The Living constructor host must return this real owner for an ordinary
default-provider actor and store it in that actor's actual Living field.
Removal must reread that field after parent removal, preserving replacement
and lifetime identity. NBT save must encode that actual current owner and
attach its result at LivingEntity's source `Brain` save phase. Loading saved
`Brain` replaces the field only at its source load phase, after a successful
supported decode. These callsites and central registry adoption remain an
integration responsibility until actual Main evidence is recorded.

Nondefault Brain schemas, memory registries/codecs, sensor creation/ticks,
behavior execution, general activity/schedule configuration, and arbitrary
saved inputs remain unsupported. `Brain.tick` is deliberately not exposed:
even empty behavior tables require the original world game-time calls in its
start/tick phases. A silent successful empty AI/scheduler would skip those
mandatory operations.
