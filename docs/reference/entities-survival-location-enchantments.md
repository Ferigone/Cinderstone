# Native location-dependent enchantment collection owner

Issue4, instance `es-20261101-1425-a7c3`, claim
[6569589745](https://github.com/Ferigone/Cinderstone/issues/4#issuecomment-6569589745).
Base `e1471304cbd3e2cb4a30d0b522682084fded0d80`. New owned source
`src/effects/location_map.rs` and this reference. Root owns exports, Git and
Main; actor assembly owns its concrete constructor attachment.

## Original evidence

Minecraft Java1.21/protocol767, original inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, official
mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`. Direct JDK21 javap
analysis of the unchanged original server and bundled fastutil8.5.12, not an
REA/MCP execution. Raw output and binaries stay in scratch.

LivingEntity's final field `activeLocationDependentEnchantments` (`btn.cq`)
has exact generic signature
`Reference2ObjectMap<Enchantment, Set<EnchantmentLocationBasedEffect>>`.
Constructor bytecode95–103 allocates a NEW Reference2ObjectArrayMap, directly
before `appliedScale=1`. Getter1554 (`btn.eG`) returns this SAME mutable map.
The keys are Enchantment VALUE object references, not Holder references,
registry names/IDs, or Enchantment record equality.

Enchantment.runLocationChangedEffects434–462 (`dac.a(ServerLevel,int,
EnchantedItemInUse,LivingEntity)`) selects/removes/puts entries with `this` as
the key. The invalid-slot branch removes its map entry before invoking
stopped-effect callbacks on the captured set. The ordinary branch reads the
existing set and tests each actual effect with Set.contains BEFORE constructing
its location context and checking the condition. On the first newly applicable
effect with no set, bytecode169–176 allocates NEW ObjectArraySet, then180–191
inserts it into the actor map, then192–201 adds the effect, then202–229 invokes
the real effect callback. It does not allocate a HashSet. A no-longer-matching
active effect is removed from that set BEFORE its stop callback. An empty set
is removed from the map only after the entire source effect traversal.

Enchantment.stopLocationBasedEffects465–473 first removes the map entry, then
iterates the captured actual set and invokes each stop callback. It does not
clear that set or iterate a newly looked-up replacement map value. These
callback/evaluator bodies are source evidence for required collection ownership
and ordering; this module does not claim to implement them.

Bundled Reference2ObjectArrayMap uses zero size and the shared empty-array
initial domain. Lookup scans used keys in reverse using source reference `==`.
A replacement changes the value only, preserving the original key/position.
The first insertion allocates two separate arrays of length2; full arrays grow
to2*size and copy existing references backwards. It writes new key, then value,
then size. Removal captures the old value, shifts the key array, then value
array, decrements size and nulls both trailing slots. Clear nulls used key/value
slots in reverse, then writes size0. Neither lookup nor mutation hashes keys.

Bundled ObjectArraySet also begins with zero size/empty array, scans in reverse
with `Objects.equals(STORED,query)`, preserves the first stored reference for
an equal duplicate, and grows0→2→2*size. Identity short-circuits stored.equals.
Removal shifts the live array forward, then decrements size/nulls the tail;
clear nulls the old used range then sets size0. Its iterator stores only a next
index: hasNext reads CURRENT size, next reads CURRENT array and increments that
index. No modCount, snapshot or fail-fast behavior exists. Set.forEach inherits
Iterable.forEach and uses this same live iterator; callback removals can skip
a shifted successor and callback appends can be visited. Iterator.remove's
unusual invalid-use behavior is outside this module's source-used API.

## Callable owner and domain

`LocationEnchantments<K,E>::new()` constructs the real native reference-key
array-map state internally. It is a retained mutable object; Clone aliases the
SAME map and does not clone its state or fabricate another actor. The actual
Bat Living constructor attaches it directly instead of calling an opaque
successful external map factory. The existing generic Living host contract
remains unchanged for other embeddings.

`EnchantmentValueReference` requires canonical actual value-object reference
identity. `LocationEffectReference` requires actual nonnull source effect
references plus their source virtual equals behavior. Neither trait has an
implementation using protocol IDs, strings, generated identities or a new
registry. Effect equality is restricted to the pinned callback-free value
object domain; arbitrary callback-bearing equals needs a separate retained
runner. Clone and identity comparison retain/check the same actual object.

Map get/put/remove preserve actual retained `LocationEffectSet<E>` handles;
those sets own the native ObjectArraySet state and methods. Captured sets and
live cursors survive removal/replacement of their map entries. Every state
loan ends before a caller can execute a real gameplay callback. The module
exposes exact native allocation/growth/lookup/replacement/removal/clear,
set contains/add/remove and live cursor operations. Null keys/values, custom
default-return values, arbitrary map views, cloning/serialization and unsupported
iterator.remove are not silently approximated. Native allocation/poison errors
are explicit capability failures, not fabricated Java exceptions.

## Integration and limits

Current Main NativeEnchantmentRegistry retains configured owner/holder identity
and limited native definition metadata. Its definition records cover key,
costs, supported/primary items and exclusive IDs, but not the full original
Enchantment object/effects-component graph or location-effect evaluator.
A Holder from that registry is not itself this map's Enchantment value key.
Actual canonical value references/effect equality must be supplied when a
nonempty equipped execution is admitted. An unequipped Bat constructor needs
no key/effect objects and can allocate this exact local map now.

The actor assembly uses external `EnchantmentValue`/`LocationEffect` reference
domains and the concrete native collection; it no longer delegates peractor
map creation to NativeBatServices. This closes one constructor allocation,
not production equipped-enchantment behavior, RNG/loot contexts, effect world
callbacks, full actor ticking, persistence, Main admission or a whole delivery.

## Focused validation

Stable rustc1.99.1 `--edition 2021 -D warnings` compiles the actual standalone
source, and its focused runtime checks pass. They cover equal-but-distinct
Enchantment value references, replacement preserving original position,
capacity0/2/4 and no shrink on clear, retained set lifetime after map removal,
reverse stored-receiver equality and identity shortcut, and live iteration
through current-element removal, append and clear. The fixtures contain
controlled reference/value objects only; they do not claim an enchantment
registry, evaluator, Actor/Level owner or production Main proof. Full actor
compilation/construction validation belongs to Root's combined stage.
