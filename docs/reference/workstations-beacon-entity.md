# Native BeaconBlockEntity state and lifecycle

Target: Minecraft Java **1.21**, protocol **767**. This stage supplies the
Beacon subclass state around the shared native BlockEntity base, with source
name/lock, serialization and live ContainerData boundaries. It reuses the
existing scan and [ordered/live tick](workstations-beacon-live-contract.md)
instead of introducing another beam, power, payment or result owner.

The source base is `e1471304cbd3e2cb4a30d0b522682084fded0d80`;
the exact pre-edit assignment is
[issue #3 comment 6569390389](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6569390389).
No Main beacon object, world index, ticker or gameplay execution is established
by this subclass port.

The implementation is split between
[subclass state](../../src/workstations/beacon_entity.rs),
[NBT wrappers](../../src/workstations/beacon_entity_nbt.rs) and
[live data](../../src/workstations/beacon_entity_data.rs).

## Primary source evidence

Evidence is direct **JDK 21 `javap -p -c`** inspection of the unchanged original
server and official mappings, not REA or execution of Java beacon gameplay.
The inner server JAR has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
the official mappings have SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`.
No Java artifact or raw disassembly is committed.

| Original method | Source behavior |
| --- | --- |
| `BeaconBlockEntity` (`dqc`), constructor 73–127 | Construct the inherited base, two distinct empty beam lists, actual `NO_LOCK` and a ContainerData bound to this object; levels and last-check height start at zero, powers and custom name at null. |
| `dqc.<clinit>`, 55–68; `getName`, 367–370 | Effect tiers and valid effects precede creation of the one static `container.beacon` component. Getters return that same class-owned component when no custom name is present. |
| `setLevel`, 419–421 | Attach the actual Level through the inherited setter first, then set lastCheckY to its minimum build height minus one. |
| `setRemoved`, 245–247 | Play deactivation sound unconditionally before setting the inherited removal flag. No beam/level predicate suppresses this sound. |
| `loadAdditional`, 314–324 | Load primary, then secondary; conditionally parse a String CustomName; assign LockCode.fromTag unconditionally. The saved Levels value is never read. |
| `saveAdditional`, 328–339 | Save keyed primary, keyed secondary, raw Levels, optional CustomName and nonempty Lock in that order. Omitted values do not remove existing target keys. |
| `getUpdateTag`, 289; `getUpdatePacket`, 284 | Update tag uses saveCustomOnly, excluding metadata and the stored component map. Packet creation delegates to the actual generic BlockEntity packet owner. |
| `applyImplicitComponents`, 375–378 | Assign nullable CUSTOM_NAME and LOCK with the actual NO_LOCK default; absence clears the old custom name and replaces the lock. |
| `collectImplicitComponents`, 382–387 | Always set nullable CUSTOM_NAME; set LOCK only when unequal to NO_LOCK. Skipping an empty lock does not remove an existing builder lock. |
| `removeComponentsFromTag`, 391–393 | Remove CustomName and Lock only. |
| `LockCode` (`bqu`), 18–35 | Empty key unlocks immediately. Otherwise compare its UTF-16 key to the actual custom-name component's getString result. String Lock creates a record even when empty; missing/wrong-typed Lock returns the singleton NO_LOCK. |
| `dqc$1`, get 92–96, set 102–112, count 116 | Three signed-int data fields; primary selection sounds before lookup/filter/assignment, secondary is silent, and unknown indices are zero/no-op. |

The [earlier runtime ledger](workstations-beacon-runtime.md) remains the source
record for incremental beam/pyramid scanning, effect application, criteria and
Level sound RNG. Those existing algorithms are not a new delivery in this stage.

## Bounded original-class observation

Root executed the unchanged original constructor, ContainerData index-zero
setter and additional-NBT methods with the real bootstrap and built-in registry
lookup. The helper exited normally. No Level, server or world was created;
this is original-class runtime evidence, not beacon gameplay or full persistence.

Two original beacons shared the same DEFAULT_NAME reference, had distinct data
objects capturing their respective beacon, and began at levels zero, null
powers/name, empty beams and the same NO_LOCK singleton. Saving an empty beacon
into a seeded compound preserved existing primary/secondary/name/lock sentinel
keys and wrote Levels zero. Its SNBT rendering listed CustomName, Levels,
Lock, primary_effect, secondary_effect; that rendering is not a separate
observation of the underlying HashMap encounter order.

After actual data index zero set levels to seven, loading Levels 99 plus valid
speed/regeneration, name and lock assigned those fields but retained levels
seven. A later load with wrong-typed CustomName and missing powers/lock retained
the same name object, cleared powers and restored the NO_LOCK singleton. A
present empty String Lock created a distinct record equal to NO_LOCK.

External helper `/tmp/WsBeaconEntityObservation.java` SHA-256:
`c33473b93a8f2771019fa84d88e0e6b976b4e4364e2e90a715f29d7f9b0c402c`.
Normalized observation `/tmp/ws-beacon-entity-original-observation.json`
SHA-256: `d7fc376871853fd94f4c3c72d3306b7ae5e2e662c1ba55b32fbf62276736f478`.
The helper, Java artifacts and raw logs remain outside Git. This bounded
observation does not certify the new native implementation by itself.

## One concrete subclass owner

`BeaconBlockEntity<T,L,M,E,C>` composes
`NativeBlockEntityBase<T,L,M>` with the existing `BeaconScanState`, raw levels,
canonical primary/secondary effect handles, nullable custom name and native
LockCode. Its inherited type, nullable Level, immutable position, block state,
removal flag and stored component map remain in the shared base.

`BeaconClass<E,C>` retains the actual effect-constant service and class
default-name handle. `BeaconClassInitializer` supplies those constants before
constructing the default name. Initialization failures are sticky rather than
silently retried; full JVM classloader/exception-object equivalence is not
claimed. Component and effect clones preserve their canonical objects; a title
string, reconstructed numeric holder or per-getter translatable component is
not an implementation. The class owner must survive placements and world-image
operations rather than being recreated for every beacon.

`BeaconBlockEntity::construct` requires an initialized shared class and a real
`BeaconBaseFactory`. Fresh-field checks supplement that actual factory contract;
equal fields cannot certify a reused object as newly allocated. The object is
not Clone. Its source anonymous `ContainerData(this)` capture is not allocated
by this port: the actual arena/menu factory must retain that capture and the
same BE. No stand-in identity token supplies it.

The immutable LockCode stores its real JavaString key, with UTF-16 equality.
An explicitly loaded empty key may be a fresh record while remaining equal to
NO_LOCK. Unlocking a nonempty key requires the actual named-item/component
getString service; text guesses or lossy conversion are not substitutes.
Menu construction delegates the shared source canUnlock behavior and actual
menu factory. This port supplies no player, message, sound or menu owner.
The shared source check bypasses for an actual spectator, not for creative mode.
Refusal displays the real locked-container actionbar message, then plays the
actual player's locked-chest notification sound before returning no menu.
Core entry points include `set_level`, `set_removed_live`, `create_menu`,
`apply_implicit_components`, `collect_implicit_components` and
`remove_components_from_tag`. The generic update-packet factory remains an
outer integration requirement; it must resolve the real attached Level's
registry access before capturing position/type and producing the update tag.

Raw subclass setters are field assignments, not dirty callbacks, menu resets or
sound requests. The actual tick/menu adapter must borrow these same fields and
scan object. It must not copy them into another live beacon record or reset scan
state when a menu opens.

## Native NBT and source failure prefixes

`beacon_entity_nbt.rs` adds `load_additional`, `load_custom_only`,
`load_with_components`, `save_additional`, `save_custom_only`,
`save_custom_and_metadata`, `save_without_metadata`, `save_with_id`,
`save_with_full_metadata` and `get_update_tag` over that same subclass.

Loads take a decoder-issued `SourceCompoundView`. The enclosing
`SourceDecodedNbt` keeps the immutable logical tag and its genuine compound
history together; a cloned or equal Tag cannot issue this capability.
`BeaconPowerRegistry<E>` supplies actual holder lookup, key/id and validity
operations under its retained registry identity. Name serialization and safe
parsing reuse the shared `EnchantingNameCodec<C>` contract and actual registry
lookup. Its generic name operations do not create another Component codec.

Missing or wrong-typed CustomName preserves the old reference. A genuine source
parse exception logs through the real safe parser and assigns null; absent codec
or registry ownership remains an error. Loading powers and Lock does not call
the ContainerData setter, emit selection sounds or reset levels/scan state.
If a later operation fails, earlier assigned fields remain assigned.

Saves use the actual `BlockEntityCompoundOwner`, including
`NativeFreshCompoundOwner`, for new compounds and source mutations. The shared
base appends stored-component encoding and metadata only in their respective
wrapper phases. Codec full/partial/no-result and warning behavior remain with
the shared actual codec. Unknown compound histories, required tree bins and
unsupported component forms are explicit errors, not permission to clone a
field vector or substitute EMPTY. The currently concrete component-map codec
supports genuine EMPTY only.

Saving null/unkeyed powers, no custom name or an empty lock leaves any old
matching key in an existing target untouched. Serialization is ordered and
retains completed target mutations on later failure; no general rollback is
claimed. Update tags contain subclass data only. A working update-packet or
Main publication path must still be supplied by its actual owner.

## Live data, removal and callbacks

`beacon_entity_data.rs` supplies `data_count`, `data_get`, `data_values` and
`set_data_live` through `BeaconEntityDataHost` over this retained subclass.
The count is three; values are raw levels and primary/secondary built-in holder
IDs plus one, with Java wrapping arithmetic. Zero means no effect;
invalid/unavailable holders are distinguished by the actual registry service
and source filter. Unknown setter indices do not acquire a Level or registry;
index zero assigns only raw levels. Secondary selection also needs no Level.

Primary selection reads the actual attached Level's server side before the raw
committed beam list; a detached beacon cannot fabricate a server-side default.
It then sounds before decoding/filtering/assigning even for zero, invalid or
unchanged selection. Sound callbacks therefore observe the old primary. A
later error preserves completed sound/RNG/publication and reentrant field
changes; no stage, clone, rollback or retry is introduced. Secondary selection
does not sound. Unknown signed indices perform no source mutation.

Removal likewise retains the deactivation callback prefix and sets the base
flag only after that callback succeeds. Actual callbacks must retain the same
BE across removal, replacement and reattachment, never redirect to a current
position/index entry or a reused wire ID. A mutable counter or caller Boolean
does not establish that lifetime capability. Live data/removal admission is
separate from the old restricted staged menu/data APIs.

## Validation and remaining production binding

Root checked frozen core SHA-256
`74e3a438d664a8f6483fed01474baf1d0b7cf9201e37bf5cf698e0cbad275e36`,
NBT `c3aeb4167ccfdb7d996d69ee1224ae4c29b3f36eb49301fd121768e875384abb`
and live data
`0ee2914fe2cf8c59d68d5934b2ff1a6404b6af8731f957aa2fab56d8c34f20a7`.
Locked offline all-target compilation passed in 1m26s. Focused native class,
lifecycle, NBT and live-data flows passed on those exact files. They cover the
same-object/default-name and Lock semantics, earlier assignments surviving
lookup/component failures, omitted-key preservation and custom-only update
tags, plus actual retained-field callback prefixes. Separate primary bytecode
and API/integration reviews found no concrete blocking defect; class constants
have safe Send + Sync bounds. Formatting and whitespace checks passed.

NBT registry/name/component queries are admitted only as read-only and
nonreentrant. Arbitrary codec/logger-driven BE mutation is outside those
ordinary borrows. Live data and removal use their separate retained mutable
callback contracts. The runtime must call the concrete subclass set_level and
set_removed_live phases, rather than bypassing them through raw base setters.

Controlled native class/holder/name/compound/callback fixtures are not an actual
configured registry, Level, actor arena or Main flow. They are separate from
the bounded original-class observation recorded above.

Central integration still owns the canonical Beacon type/class factory,
retained world BE index, ticker, checkpoint lifecycle, effect definitions and
Team4 actor arena, actual criteria, sound/RNG/audience and packet publication.
Nonempty component codecs and the actual name/Lock/menu services must be
available in every admitted context. Missing services must error, not become
empty players/effects or no-op callbacks.

There is no Main Beacon integration or complete workstation delivery in this
source stage. `inventory.4`, `crafting.2` and `crafting.5` remain aggregate
incomplete; no checklist, denominator, public status, journal or deployment
configuration change follows.
