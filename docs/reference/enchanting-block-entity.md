# Native EnchantingTableBlockEntity server state

Target: Minecraft Java1.21, protocol767. `enchanting_block_entity.rs` ports the
actual server-side subclass state, constructor/name/NBT/component operations,
native inherited state/serialization bodies and explicit actual owner
capabilities. It supplies no inventory or invented server animation tick.
`block_entity.rs` supplies concrete inherited fields and lifecycle operations;
`CapturedEnchantingType::construct` supplies the pinned canonical factory
boundary. Concrete NativeBlockEntities registration, actual Level/component
and registry assembly, imports/checkpoint binding and Main opening remain
pending central integration; a palette block alone is not a block entity or
menu provider.

## Source evidence

Original inner server SHA256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings SHA1: `31c77994d96f05ba25a870ada70f47f315330437`.
The following source was inspected directly with JDK21 `javap`, not a REA MCP
operation. Raw disassembly and Java artifacts stay outside the repository.

| Original source | Exact observed behavior |
| --- | --- |
| `EnchantingTableBlockEntity` (`drc`), ctor35–36 | Calls `BlockEntity(ENCHANTING_TABLE,pos,state)`. Name is null and all animation fields start at JVM zero. It implements Nameable, not Container or MenuProvider. |
| `BlockEntity` (`dqh`), ctor46–52 | Sets the actual empty DataComponentMap, canonical type, immutable position copy and actual block state. Level initially null and removal flag false. |
| `dqh.setLevel`, lines64–65; `setRemoved`,232–233; `clearRemoved`,236–237; `setBlockState`,269–270; `setComponents`,336–337 | Assign the real field only. They do not invent dirty notifications, packet delivery or inventory. |
| `dqh.loadWithComponents`, lines75–80 | Runs the actual subclass loadAdditional first; parses ComponentHelper codec with actual registry NbtOps; logs source DataResult errors and assigns its available full/partial map. No result preserves the previous map. |
| `dqh.saveWithoutMetadata`, lines113–120 | New compound; actual subclass saveAdditional first; encode stored component map with actual registry NbtOps; source diagnostics; merge its available full/partial compound. |
| `dqh.saveCustomOnly`,124–126; `saveCustomAndMetadata`,130–132 | Custom-only excludes inherited component-map encoding. Custom-and-metadata then appends actual type id and x/y/z. |
| `dqh.saveId`,136–141; `saveMetadata`,156–160 | Resolve the actual type object's registered key, then write id and x/y/z; absent registered key fails rather than inferring a palette name. |
| `drc.saveAdditional`, lines40–44 | Calls the fixed empty base hook, then writes CustomName as Component.Serializer JSON using the actual registry lookup only when its current name is nonnull. It does not clear a pre-existing target key when no name is present. |
| `drc.loadAdditional`, lines48–52 | Calls the fixed empty base hook, then assigns parseCustomNameSafe only if CustomName is a String. Missing/wrong-typed keys preserve an already assigned name. |
| `dqh.parseCustomNameSafe`, lines342–345 | Parses with actual HolderLookup.Provider; an actual Java Exception logs the original failed-name warning and returns null. |
| `drc.getName`, lines115–118; `Nameable.getDisplayName`, line15 | Returns the actual custom component reference, otherwise constructs `Component.translatable("container.enchant")`. Display name delegates to getName. |
| `drc.setCustomName`, lines122–123 | Assigns the nullable component only; no setChanged, block event or packet. |
| `drc.applyImplicitComponents`, lines133–135 | Fixed base hook returns; CUSTOM_NAME input is assigned unconditionally. An absent component clears the name. |
| `drc.collectImplicitComponents`, lines139–141 | Fixed base hook returns; calls the actual builder's set(CUSTOM_NAME,name). Builder `km$a` removes that key for null. |
| `drc.removeComponentsFromTag`, lines145–146 | Removes CustomName, without an additional base mutation. |
| `EnchantingTableBlock.getTicker`, line84 (`dik.a`) | Server side returns null regardless of requested type. Only client side resolves the matching-type book-animation ticker. |
| `drc.<clinit>`, line30 | Calls RandomSource.create once and stores one private static class RNG. It is not a per-table or Level.random stream. |
| `dik.getMenuProvider`, lines100–106 | Reads the actual block entity and creates its provider only for an actual drc. Title comes from that object's getDisplayName. An absent/other-class BE returns null; it does not create a fallback screen. |

The server never invokes `bookAnimationTick` through this block's ticker. Its
client-only nearest-player/book motion algorithm is not supplied by this
server port. The real animation fields are retained at their source initial
values, are not subclass NBT fields and receive no artificial server updates.

## Implemented state and required concrete owners

`EnchantingTableBlockEntity<B,C>` retains one actual inherited base object B,
one nullable canonical Component handle C, and its real animation field state.
There is no separate menu inventory or fabricated world. Base factory and state
traits require actual position/state, attached Level, removal and component-map
observations with no permissive defaults. Construction validates the real
initial base contract. Later runtime attachment uses the same base owner;
initial null Level is the original constructor state, not a missing-BE fallback.

`EnchantingTableClass<R>::construct` first initializes the one real class RNG
through the supplied actual RandomFactory, then invokes the required actual
base factory. The class owner is deliberately not Clone. Successful class
initialization survives a subsequent failed base construction, and a failed
initializer is not silently retried with another stream. Rust host failures
remain explicit; no full JVM exception-object/classloader-failure equivalence
is claimed. The runtime must retain this owner for one class lifetime, shared
across actual tables and worlds. It must not deep-clone it into staged world
images or create a class state per placement. Server behavior never draws
book-animation samples from it.

`EnchantingNameCodec<C>` requires the actual Component serializer/parser,
translatable constructor and registry lookup. C clones preserve canonical
component identity. Default names are constructed through that genuine codec
at getName/getDisplayName; a hardcoded OpenScreen title is not a replacement.
An unavailable codec/registry is an explicit error. Only a real source parse
failure produces the safe parser's logged/null result. This module neither
implements a permissive JSON fallback nor swallows missing registry ownership.

NBT uses the existing lossless JavaString/Tag primitives, including UTF16 code
units and logical last-key lookup. Removing CustomName removes every decoded
duplicate occurrence so that the logical key is absent. Only the actual
CustomName subclass field is saved/loaded by the generic additional methods.
The native-base specialization below adds source save/load wrapper bodies,
requiring actual component/type/registry codecs. Item-stack application,
saveToItem and update wrappers remain outside that narrow specialization.

`EnchantingComponentInput` and `EnchantingComponentBuilder` require the real
CUSTOM_NAME component owners. Applying absence clears the old name; collecting
absence removes the builder key. Neither method invents another component map,
calls setChanged or mutates inventory. `has_server_ticker` is false by the
original server-side branch, not because an unimplemented server tick was
replaced with an empty function.

## Native inherited prerequisite and canonical factory

`NativeBlockEntityBase<T,L,M>` now owns actual type handle, nullable Level
handle, immutable position value, removal flag, current state and stored
component-map handle. Its constructor requires the genuine EMPTY singleton
from the actual component owner. Imported/applied maps are assigned as
supplied; there is no unknown-import-to-empty conversion. Cloned L/T/M handles
must retain their genuine owner identity. No class RNG is a base/world field.

`BlockEntityComponentCodec<M>` is the required actual ComponentHelper codec,
HolderLookup/NbtOps and logger capability. The native bodies preserve source
subclass-before-component ordering and actual DataResult full/partial/no-result
behavior. Source diagnostics are `Failed to load components: {}` and
`Failed to save components: {}` with the real codec message. Missing native
codec, registry or warning ownership remains an error. Component encoding must
return actual logical CompoundTag iteration order with unique keys. The required
CompoundTag owner performs actual source merge operations, including recursive
compound/copy behavior; it cannot fabricate a map for unknown input history.
The source save variants retain their distinct metadata/components boundaries.

`CapturedEnchantingType::capture(&actual_registry)` validates actual registry
identity, Minecraft1.21/protocol767, pinned original/mapping provenance,
canonical type key, the catalogue's actual enchanting block state (7389) and
the actual type's valid-state set. A foreign dynamic registry is refused rather
than using the compiled state number as proof. Construction/save key lookup
checks that the actual registry identity has not changed. Capture does not
initialize a class or draw RNG.

`CapturedEnchantingType::construct::<L,M,C>(position,state,&mut actual_empty)`
uses one private process/class owner of
`EnchantingTableClass<LegacyRandomSource>` through a retained static mutex.
Its first genuine construction invokes existing
`random_seed::create_random`: the actual shared process uniquifier and
CLOCK_MONOTONIC owner, with source update-before-clock ordering. Initialization
precedes the real base constructor; later tables/cloned worlds do not allocate
another class stream. Registry/source preflight remains an explicit native
admission boundary before construction. Initializer, clock or poisoned-owner
failures remain errors; Java classloader/exception-object parity is not claimed.

The instantiated type is
`EnchantingTableBlockEntity<NativeBlockEntityBase<R::Type,L,M>,C>`. C remains a
canonical component handle supplied by the actual name codec, not raw JSON
declared equivalent to a Component. Full source inherited implicit-component
patch filtering, saveToItem, registry loadStatic and update wrappers are still
outside this narrow prerequisite. No absent owner is silently defaulted.

## Required central factory and production assembly

The next narrow integration must supply actual Level, immutable component map,
canonical Component and registry/name/component-codec bindings; register real
construction/import/replacement/removal/Level attachment and source
save/checkpoint handling; and expose lookup of the actual retained BE to the
block provider. It must include the owned BE in certified world images rather
than infer it from matching palette blocks. The existing context-free
NativeBlockEntities map and cloned FlatWorld are intentionally unchanged.

Exact assembly call phases are:

1. Retain `CapturedEnchantingType::capture` over the real supported immutable
   registry; perform actual source/world admission before calling its factory.
2. Construct with the actual EMPTY-map capability, then attach the actual Level
   using `body.base_mut().set_level(Some(actual_level_handle))`. On import call
   `body.load_with_components(tag,registries,names,components)`; no component
   codec may be replaced with a successful empty result.
3. Preserve the actual body through same-block state changes; use its base
   lifecycle methods for actual attachment/removal. Central replacement and
   certified world/save ownership remain required.
4. Actual typed Level lookup must return this retained object. Only then call
   `body.get_menu_provider(actual_lookup_level,actual_lookup_position,names)`.
   This invokes its actual getDisplayName immediately and captures the real
   Level/pos as original SimpleMenuProvider does. Missing/other-type lookup must
   follow the original absent branch, never construct a substitute object.
5. `NativeEnchantingMenuProvider::create_menu(menu_id,actual_inventory,factory)`
   invokes required `NativeEnchantingMenuFactory::create_enchantment_menu` with
   that same actual inventory and captured location. The factory must bind
   actual ContainerLevelAccess/menu random/source owners. There is no owned
   duplicate inventory, permanent None provider, empty menu or fallback title.
   The captured BE identity is not an extra menu stillValid condition in source.
6. Supply an actual `BlockEntityCompoundOwner`, then use the appropriate
   `save_custom_only`, `save_custom_and_metadata`,
   `save_without_metadata`, `save_with_id` or `save_with_full_metadata` body with
   actual codecs/type keys and the required `compounds` argument. No server
   book ticker is registered.

## Concrete assembly audit and next minimal stage

This audit reads the future worktree's actual implementation. Separately,
fresh master `4594a3edb2a7a0087d3ed06398b1507241f84e50` was read through GitHub
for `src/components/text.rs` and `docs/reference/components-text.md` (PR109).
Subsequently `src/components/decode.rs` was inspected at current master
`2b4d20994b6b8cc8ce3b609fabceda830d09450a`: the shared model now has genuine
`Component::from_nbt`. Those newer independent files were not copied over this
worktree. Use that shared text model, not another parser or duplicate Component
definition.

| Existing owner/seam | Concrete next required assembly |
| --- | --- |
| `main.rs::Server.world: RwLock<WorldBackend>`, held by the actual `Arc<Server>`; `RuntimeWorld::{Fixture(FlatWorld),Anvil(VanillaWorld)}` | There is no retained library Level handle yet. Central assembly must bind L to this actual owner and lifecycle before calling `set_level`. A dimension string, copied WorldContext or integer pointer observation alone is not a replacement Level. Existing `NativeLoomBinding.level_owner` is a same-lock observation, not an independently reusable Level object. |
| `main.rs::Fixtures.configuration`; `main_native_enchantments.rs::NativeEnchantmentRegistry` | The configured enchantment holder owner can later supply the existing EnchantingMenu host. It is not the builtin BlockEntityType registry and does not implement the name/component-map capability. |
| `placement-1.21.json`; `tools/storage_registries.json`; `tools/storage_blockentity_factories.json` | Existing original-derived facts bind state7389, canonical key, type12 and fresh factory NBT. Central native builtin registry ownership must validate and retain these facts as canonical type objects for `ActualEnchantingBlockEntityRegistry`. Calling `storage::block_entity_factory_snapshot` only clones NBT and does not construct a live native BE. |
| Fresh-master `components::text::{Component,Contents,Style}` | Use a retained `Arc<Component>` for C, so getName/provider clones retain the same component instead of deep-cloning it. Build the default actual Translatable contents through this shared model. Its `from_json`, `to_json`, `to_network_nbt` are the name/parser/title serialization boundary. Keep `ComponentError::Codec` distinct from `Unsupported`; only genuine source parse rejection invokes the safe-name warning/null behavior. |
| `NativeBlockEntities::{entries,furnaces}`, `transition`, `validate_owned_at` | Add one typed table map only after L/M/type/name assembly exists. Actual same-block updates retain the same body; replacement/removal must execute the inherited source lifecycle. Current maps have no table body, generic component map or typed table lookup. |
| `world.rs::FlatWorld::{set_block,apply_changes,restore_native_block_entities,chunk_packet}` | Add the explicit admitted creation/attachment phase and authoritative restore path. Context-free setters and private clone proofs must not initialize a class or fabricate a table from its palette state. Chunk data must enumerate actual installed table owners with type12 and the inherited empty update tag; custom names belong to provider/NBT, not an invented update packet. |
| `entity_persistence.rs::WorldImage`, `prepare_owned_edit`; `main_world_interactions.rs::{commit_fixture_plan,drain_world_interactions}` | Add actual table images/type/state validation to both CreateBlockEntity checks, source after-image and certified/pending image comparison. Pure proofs must project an already owned operation without invoking another constructor/RNG. Do not create a body only when the deferred CreateBlockEntity verifier finally runs. |
| `NativeBlockEntities::{save,restore,load_nbt}`; `main.rs::SavedWorld`, startup and save | Add typed authoritative table records and actual codec arguments. Startup currently applies palette changes before restoring BE records. It must not thereby create a new table and silently use that as evidence for a missing legacy record. Save under the same world/player operation interval as the certified WorldImage; retain the existing exact SavedWorld-byte binding to EntityCheckpoint. |
| `main.rs` use-block/click/button/close/current-menu/removal/save dispatch; `container_workstation_enchanting.rs::EnchantingMenu` | There is currently no Main enchanting runtime/host or dispatcher. After real typed BE lookup and provider name, implement `NativeEnchantingMenuFactory` over the canonical player inventory, existing menu kernel, configured enchantment holder owner and actual source callbacks/RNG. Route all lifecycle/close/resync paths; adding only an OpenScreen packet is insufficient. |

The smallest first production slice is a genuinely constructed/retained native
table in the certified fixture world, with its actual default Component name,
real constructor EMPTY component owner and a fully bound save/restore lifecycle.
This requires implementing the missing canonical bindings, not adding successful
empty callbacks. Imported/nonempty component maps, unsupported item component
application and unavailable name contexts must remain explicit errors. Existing
Anvil stores raw chunk NBT and has no native table/Level/source assembly; it is
a separate actual import/lifecycle adapter, not equivalent evidence of this
fixture slice. A working EnchantingMenu additionally requires the final row.

The table's new save schema must distinguish newly authoritative empty records
from absent legacy data, validate duplicates/type/position/state against actual
blocks, and restore real objects before exposure. An old file that contains a
table palette state without its owned record does not prove an empty/default
body. No class RNG, Level handle, removal flag, book animation, session/menu
sender baseline or duplicate inventory belongs in source table NBT. Source
loads reconstruct the actual body and then attach its genuine Level. A name or
stored-component mutation without a block-state change must also update the
owned source image using its real pre-mutation image, analogous to existing
`observe_owned_container_change`/`observe_owned_furnace_change`; rehashing an
unobserved world is not an acceptable certification path.

### Source construction/attachment phase

Direct `javap` of original `LevelChunk` (`dvi`) confirms that
`setBlockState223–289` writes the section at offsets64–76, runs old onRemove at
319–328, checks the current block, runs new onPlace at385–394, then performs BE
lookup and actual EntityBlock.newBlockEntity at419–430. An existing body receives
setBlockState at447–456. `setBlockEntity371–402` validates the current state/type,
calls setLevel146–151 and clearRemoved154–155, inserts the actual object158–172,
then marks a different replaced object removed188–190. This evidence uses the
same pinned original identities above; it is direct javap, not REA.

Current FlatWorld calls `native_block_entities.transition` before writing block
storage. `NativeEntitySource::prepare_owned_edit`, callback proofs and
`commit_fixture_plan` each use private clones. Registering a table constructor
inside that old transition would therefore put its first class RNG creation in
proof work and before the source write/callback phase. Central assembly must
give the admitted operation an explicit source-ordered factory/attachment phase,
retain that actual constructed body for adoption, and give pure clone proofs
only its corresponding projection. It must not invoke another source constructor
from each clone, move class state into FlatWorld, or silently roll back/reseed
an already performed class initialization when a later native owner fails.

### Remaining codec boundaries

PR109 implements text Component, not DataComponentMap/ComponentHelper. Its NBT
decoder was absent at4594; current master2b4d209 adds the shared genuine
`Component::from_nbt` decoder. show_item/show_entity hover forms remain
unsupported and unpaired UTF16 is explicitly Unsupported. Existing exact
numeric JSON formatting limitations are not resolved by merely adding the
decoder.
Do not treat those valid-but-unsupported names as Java parse exceptions or
replace them with the default title. CustomName is source JSON-in-NBT, so its
supported JSON path can reuse the shared model. Item CUSTOM_NAME NBT should use
that shared decoder; DataComponentMap/ComponentHelper and registry bindings are
still separate actual owner requirements.

The required `BlockEntityCompoundOwner` corrects the previously missing root
map ownership boundary. The existing Tag writer emits retained vector order,
whereas a Java CompoundTag emits HashMap encounter order. Save bodies therefore
no longer create a Vec-backed root or finalize a partial root and append later
metadata. Every exported source save wrapper requires this actual capability:

| Required operation | Concrete implementation obligation |
| --- | --- |
| `type Compound` | Retain the same actual logical root and its insertion/collision/resize history, including an existing target passed to saveAdditional. A finished Tag alone does not reconstruct an unknown history. |
| `fresh_compound()` | Allocate the actual original fresh-root map state; no copied imported root or already finalized field vector. |
| `put(target,JavaString,Tag)` | Execute the source field write against that same root, retaining Java key identity/hash and actual overwrite/encounter behavior. Name serialization and type-key resolution happen before their corresponding write. |
| `merge_codec_compound(target,actual_encoded)` | Execute the actual ComponentHelper result's source key iteration and CompoundTag.merge recursion/copy rules. Preserve nested codec ordering and actual existing-target history; unsupported history/context is an error. |
| `into_ordered_tag(root)` | Consume that same root exactly once, after all source fields/components/metadata, and produce its real Java HashMap encounter order. Do not finalize then reconstruct/append metadata. |

`save_additional` now takes `&mut O::Compound` and required `&mut O`. The inherited
`save_components_into`, `save_id`, `save_metadata` take the same owner. Private
`fresh_custom`/`fresh_without_metadata` retain O::Compound through each full
variant; the five public save wrappers finalize once. Name/load/provider/class
RNG behavior is unchanged. There is deliberately no default implementation and
the misleading local vector-based merge has been removed. Concrete original
HashMap/CompoundTag ownership now has the concrete admitted implementation below;
central assembly must actually supply it and the real codecs. PR109's
`components::java` is reused, not permission to sort fields lexically or infer
unknown imported-map history.

## Concrete native fresh CompoundTag owner

`block_entity_compound.rs::NativeFreshCompoundOwner` implements the required
capability. Its private `NativeFreshCompound` retains actual native JavaHashMap
table slots, list-node order, stored spread hashes, size and threshold. It does
not publicly implement Clone or accept a field vector as reconstructed history.
The fresh root is genuinely source-created native state: no table allocation
until first put, default first capacity16 and threshold12. Overwrite retains
the existing key/node position, size and capacity; remove preserves remaining
node order and does not shrink/reset a map.

The concrete API is `NativeFreshCompoundOwner::new()` plus required trait
`fresh_compound/put/merge_codec_compound/into_ordered_tag`. Additional
`copy_known(&actual_root)` executes the real source deep copy, and
`put_owned_compound(target,key,actual_child)` transfers an actual exclusively
owned native child with its genuine history. Direct put of bare compounds,
including inside a ListTag, returns `DirectNestedHistory`; callers must supply
the actual retained child or use the genuine codec-copy path. Shared/cyclic
mutable Tag graphs are outside this exclusive-owner save profile. No new Level,
component model, random source or class owner is created by these operations.

`adopt_imported(tag)` returns `ImportedHistory` explicitly: decoded entry order
does not establish old capacity/deletion/tree/insertion history. Actual source
codec results are a different boundary: their logical encounter order is
required by `BlockEntityComponentCodec`. For a replacement value, source
CompoundTag.merge invokes Tag.copy. Therefore the actual source key encounter
order and size suffice to reconstruct the NEW copy map, even though they do not
reconstruct the original source map's history. This distinction never admits an
unknown imported target as an existing native owner.

Direct pinned-original `javap` confirms:

| Source | Concrete implementation |
| --- | --- |
| `CompoundTag` (`ub`) ctor186–187 | Guava Maps.newHashMap, original fresh state. |
| `ub.put`,229; `remove`,464–465 | Actual Map.put/Map.remove; preserve surviving list-node history. |
| `ub.copy`,493–494 | Maps.transformValues(Tag.copy), then Maps.newHashMap(Map). It is not a clone of the old bucket table. Lists/compounds/arrays copy recursively according to their actual value kinds. |
| `ub.merge`,548–563 | Iterate source keySet; compound+compound recursively merges into existing actual target; otherwise put sourceValue.copy. The source input's actual encounter order is retained as the copy input, rather than replaced with lexical order. |
| Pinned oracle JDK21.0.13 `HashMap.putMapEntries` | For a new map, threshold is tableSizeFor(ceil(size/0.75)). The older float(size/.75)+1 formula is not used. Native admission sizes make integer ceil(4*size/3) equivalent. |
| JDK21 `HashMap.putVal`, `resize`, `treeifyBin` | UTF16 String.hashCode and spread; list append; real low/high stable split on resize; post-insert size/load gate. Appending after eight existing list nodes calls treeifyBin; below64 it resizes first, at64 tree support would be required. |

Representable keys reuse shared `components::java::string_hash`; lossless
JavaString UTF16 keys use the same fold over their exact code units, without
replacement characters. The shared field-order helper alone cannot represent
overwrite/removal/existing-target history or collision-triggered resize, hence
the concrete retained table/node state here. Output walks actual table slots
and their node chains. No field sorting is used.

Nested actual codec maps are copied in their genuine source encounter order
through the exact HashMap(Map) sizing/insertion algorithm. Their resulting copy
order may legitimately differ from their original map order; source copy, not
blind vector retention, determines that. Recursive merges preserve the actual
existing native nested map's history. Duplicate logical codec keys, noncompound
merge results and invalid ListTag element types are errors, not fabricated
successful codec outputs.

Tree bins are not implemented. A put/merge/copy needing native treeification at
capacity64+ returns `TreeBin` before mutating the actual target. Collision-driven
resizes below64 and ordinary load-factor resizes are implemented. The explicit
native resource profile permits up to4096keys/map, capacity8192, depth128 and
262144operation visits. These are native admission limits, not Minecraft NBT
rules. Existing roots/children are actual known state; a private admission
shadow copies their bucket history without executing source constructors/RNG.
Unsupported boundary detection occurs there before adoption, leaving the real
target untouched. No general arbitrary-import or tree-bin byte parity is
claimed. No Rust build/runtime check was run by this owner; Root handles focused
compilation and subsequent source-runtime verification.

No Rust build or runtime check was run by this owner; the coordinator handles
focused assembly/compilation. The local `rustfmt` executable was unavailable.
These native inherited/subclass algorithms and actual-owner factory/provider
bodies are source prerequisites, not proof of production registration, a working
enchanting BE/menu, complete block-entity support or functional Minecraft parity.
